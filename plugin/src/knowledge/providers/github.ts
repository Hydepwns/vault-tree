import { requestUrl } from "obsidian";
import type { KnowledgeProvider, LookupOptions, LookupResult, KnowledgeEntry } from "../types";

const GITHUB_API = "https://api.github.com";

interface GitHubRepoResult {
  items?: Array<{
    id: number;
    name: string;
    full_name: string;
    description: string | null;
    html_url: string;
    stargazers_count: number;
    forks_count: number;
    language: string | null;
    topics?: string[];
    owner: {
      login: string;
      avatar_url: string;
    };
    updated_at: string;
    license?: {
      spdx_id: string;
    };
  }>;
  total_count?: number;
}

interface GitHubUserResult {
  items?: Array<{
    id: number;
    login: string;
    avatar_url: string;
    html_url: string;
    type: string;
  }>;
}

interface GitHubUser {
  login: string;
  name: string | null;
  bio: string | null;
  html_url: string;
  public_repos: number;
  followers: number;
  following: number;
  company: string | null;
  location: string | null;
  blog: string | null;
  twitter_username: string | null;
}

interface GitHubRepo {
  name: string;
  full_name: string;
  description: string | null;
  html_url: string;
  stargazers_count: number;
  forks_count: number;
  open_issues_count: number;
  language: string | null;
  topics?: string[];
  license?: { spdx_id: string };
  created_at: string;
  updated_at: string;
  owner: { login: string };
}

interface GitHubConfig {
  token?: string;
}

export class GitHubProvider implements KnowledgeProvider {
  readonly name = "github";
  private token?: string;

  configure(config: GitHubConfig): void {
    this.token = config.token;
  }

  private getHeaders(): Record<string, string> {
    const headers: Record<string, string> = {
      Accept: "application/vnd.github.v3+json",
      "User-Agent": "VaultTree/0.1.0",
    };
    if (this.token) {
      headers["Authorization"] = `Bearer ${this.token}`;
    }
    return headers;
  }

  async isAvailable(): Promise<boolean> {
    try {
      const response = await requestUrl({
        url: `${GITHUB_API}/rate_limit`,
        method: "GET",
        headers: this.getHeaders(),
        throw: false,
      });
      return response.status === 200;
    } catch {
      return false;
    }
  }

  async lookup(query: string, options?: LookupOptions): Promise<LookupResult> {
    const maxResults = options?.maxResults ?? 5;

    try {
      // Check if query is a GitHub URL
      const urlMatch = this.parseGitHubUrl(query);
      if (urlMatch) {
        const entry = await this.lookupByUrl(urlMatch);
        return {
          success: true,
          provider: this.name,
          entries: entry ? [entry] : [],
        };
      }

      // Check if query looks like owner/repo
      if (query.includes("/") && !query.includes(" ")) {
        const entry = await this.lookupRepo(query);
        if (entry) {
          return {
            success: true,
            provider: this.name,
            entries: [entry],
          };
        }
      }

      // Otherwise, search repos
      const entries = await this.searchRepos(query, maxResults);

      return {
        success: true,
        provider: this.name,
        entries,
      };
    } catch (error) {
      const message = error instanceof Error ? error.message : "Unknown error";
      return {
        success: false,
        provider: this.name,
        entries: [],
        error: `GitHub lookup failed: ${message}`,
      };
    }
  }

  private parseGitHubUrl(query: string): { type: "repo" | "user"; path: string } | null {
    const repoMatch = /github\.com\/([^\/]+\/[^\/]+)\/?/.exec(query);
    if (repoMatch) {
      return { type: "repo", path: repoMatch[1] };
    }

    const userMatch = /github\.com\/([^\/]+)\/?$/.exec(query);
    if (userMatch) {
      return { type: "user", path: userMatch[1] };
    }

    return null;
  }

  private async lookupByUrl(match: { type: "repo" | "user"; path: string }): Promise<KnowledgeEntry | null> {
    if (match.type === "repo") {
      return this.lookupRepo(match.path);
    } else {
      return this.lookupUser(match.path);
    }
  }

  private async lookupRepo(fullName: string): Promise<KnowledgeEntry | null> {
    const response = await requestUrl({
      url: `${GITHUB_API}/repos/${fullName}`,
      method: "GET",
      headers: this.getHeaders(),
      throw: false,
    });

    if (response.status === 404) {
      return null;
    }

    if (response.status !== 200) {
      throw new Error(`Repo lookup failed: ${response.status}`);
    }

    const repo = response.json as GitHubRepo;
    return this.repoToEntry(repo);
  }

  private async lookupUser(username: string): Promise<KnowledgeEntry | null> {
    const response = await requestUrl({
      url: `${GITHUB_API}/users/${username}`,
      method: "GET",
      headers: this.getHeaders(),
      throw: false,
    });

    if (response.status === 404) {
      return null;
    }

    if (response.status !== 200) {
      throw new Error(`User lookup failed: ${response.status}`);
    }

    const user = response.json as GitHubUser;

    const lines: string[] = [];
    if (user.bio) lines.push(user.bio);
    lines.push(`Repos: ${user.public_repos} | Followers: ${user.followers}`);
    if (user.company) lines.push(`Company: ${user.company}`);
    if (user.location) lines.push(`Location: ${user.location}`);

    return {
      title: user.name || user.login,
      summary: lines.join("\n"),
      url: user.html_url,
      source: "github",
      metadata: {
        type: "user",
        login: user.login,
        name: user.name,
        bio: user.bio,
        publicRepos: user.public_repos,
        followers: user.followers,
        following: user.following,
        company: user.company,
        location: user.location,
        blog: user.blog,
        twitter: user.twitter_username,
      },
    };
  }

  private async searchRepos(query: string, limit: number): Promise<KnowledgeEntry[]> {
    const params = new URLSearchParams({
      q: query,
      sort: "stars",
      order: "desc",
      per_page: String(limit),
    });

    const response = await requestUrl({
      url: `${GITHUB_API}/search/repositories?${params}`,
      method: "GET",
      headers: this.getHeaders(),
      throw: false,
    });

    if (response.status !== 200) {
      throw new Error(`Search failed: ${response.status}`);
    }

    const data = response.json as GitHubRepoResult;

    if (!data.items) {
      return [];
    }

    return data.items.map((repo) => this.repoToEntry(repo));
  }

  private repoToEntry(repo: GitHubRepo | GitHubRepoResult["items"][0]): KnowledgeEntry {
    const stars = repo.stargazers_count >= 1000
      ? `${(repo.stargazers_count / 1000).toFixed(1)}k`
      : String(repo.stargazers_count);

    const forks = repo.forks_count >= 1000
      ? `${(repo.forks_count / 1000).toFixed(1)}k`
      : String(repo.forks_count);

    const lines: string[] = [];
    if (repo.description) lines.push(repo.description);
    lines.push(`Stars: ${stars} | Forks: ${forks}`);
    if (repo.language) lines.push(`Language: ${repo.language}`);
    if (repo.topics && repo.topics.length > 0) {
      lines.push(`Topics: ${repo.topics.slice(0, 5).join(", ")}`);
    }

    return {
      title: repo.full_name,
      summary: lines.join("\n"),
      url: repo.html_url,
      source: "github",
      metadata: {
        type: "repo",
        name: repo.name,
        fullName: repo.full_name,
        owner: repo.owner.login,
        description: repo.description,
        stars: repo.stargazers_count,
        forks: repo.forks_count,
        language: repo.language,
        topics: repo.topics,
        license: repo.license?.spdx_id,
        updatedAt: repo.updated_at,
      },
    };
  }
}
