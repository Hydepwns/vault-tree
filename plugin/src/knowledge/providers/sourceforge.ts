import { requestUrl } from "obsidian";
import type { KnowledgeProvider, LookupOptions, LookupResult, KnowledgeEntry } from "../types";

const SOURCEFORGE_API = "https://sourceforge.net/api";

interface SourceForgeSearchResult {
  projects?: Array<{
    projectid: number;
    name: string;
    url: string;
    shortname: string;
    summary: string;
    categories: {
      topic: Array<{ fullname: string }>;
      os: Array<{ fullname: string }>;
      language: Array<{ fullname: string }>;
      license: Array<{ fullname: string }>;
    };
  }>;
}

interface SourceForgeProject {
  name: string;
  shortname: string;
  summary: string;
  description?: string;
  url: string;
  created: string;
  homepage?: string;
  external_homepage?: string;
  download_page?: string;
  categories: {
    topic?: Array<{ fullname: string }>;
    os?: Array<{ fullname: string }>;
    language?: Array<{ fullname: string }>;
    license?: Array<{ fullname: string }>;
  };
  developers?: Array<{
    username: string;
    name: string;
  }>;
  stats?: {
    downloads?: number;
  };
}

export class SourceForgeProvider implements KnowledgeProvider {
  readonly name = "sourceforge";

  async isAvailable(): Promise<boolean> {
    try {
      const response = await requestUrl({
        url: `${SOURCEFORGE_API}/project/name/test/json`,
        method: "GET",
        throw: false,
      });
      // 404 is ok, means API is responding
      return response.status === 200 || response.status === 404;
    } catch {
      return false;
    }
  }

  async lookup(query: string, options?: LookupOptions): Promise<LookupResult> {
    const maxResults = options?.maxResults ?? 5;

    try {
      // Check if query looks like a project name
      if (!query.includes(" ") && query.length < 50) {
        const project = await this.lookupProject(query);
        if (project) {
          return {
            success: true,
            provider: this.name,
            entries: [project],
          };
        }
      }

      // Otherwise search
      const entries = await this.search(query, maxResults);

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
        error: `SourceForge lookup failed: ${message}`,
      };
    }
  }

  private async lookupProject(name: string): Promise<KnowledgeEntry | null> {
    const response = await requestUrl({
      url: `${SOURCEFORGE_API}/project/name/${encodeURIComponent(name)}/json`,
      method: "GET",
      throw: false,
    });

    if (response.status === 404) {
      return null;
    }

    if (response.status !== 200) {
      throw new Error(`Project lookup failed: ${response.status}`);
    }

    const data = response.json as { Project?: SourceForgeProject };
    const project = data.Project;

    if (!project) {
      return null;
    }

    return this.projectToEntry(project);
  }

  private async search(query: string, limit: number): Promise<KnowledgeEntry[]> {
    // SourceForge doesn't have a great search API, use their sitemap/allura search
    const response = await requestUrl({
      url: `https://sourceforge.net/directory/?q=${encodeURIComponent(query)}`,
      method: "GET",
      throw: false,
    });

    if (response.status !== 200) {
      // Fallback: try direct project lookup
      const project = await this.lookupProject(query.replace(/\s+/g, "").toLowerCase());
      return project ? [project] : [];
    }

    // Parse HTML for project links (basic extraction)
    const projects = this.extractProjectsFromHtml(response.text, limit);

    // Fetch details for each project
    const entries: KnowledgeEntry[] = [];
    for (const shortname of projects) {
      const entry = await this.lookupProject(shortname);
      if (entry) {
        entries.push(entry);
        if (entries.length >= limit) break;
      }
    }

    return entries;
  }

  private extractProjectsFromHtml(html: string, limit: number): string[] {
    const projects: string[] = [];
    const regex = /href="\/projects\/([a-zA-Z0-9_-]+)\/"/g;
    let match;

    while ((match = regex.exec(html)) !== null && projects.length < limit * 2) {
      const shortname = match[1];
      if (!projects.includes(shortname) && shortname !== "directory") {
        projects.push(shortname);
      }
    }

    return projects.slice(0, limit);
  }

  private projectToEntry(project: SourceForgeProject): KnowledgeEntry {
    const lines: string[] = [];

    if (project.summary) {
      lines.push(project.summary);
    }

    const languages = project.categories.language?.map((l) => l.fullname).slice(0, 3);
    if (languages && languages.length > 0) {
      lines.push(`Languages: ${languages.join(", ")}`);
    }

    const licenses = project.categories.license?.map((l) => l.fullname).slice(0, 2);
    if (licenses && licenses.length > 0) {
      lines.push(`License: ${licenses.join(", ")}`);
    }

    const os = project.categories.os?.map((o) => o.fullname).slice(0, 3);
    if (os && os.length > 0) {
      lines.push(`Platforms: ${os.join(", ")}`);
    }

    return {
      title: project.name,
      summary: lines.join("\n"),
      url: project.url || `https://sourceforge.net/projects/${project.shortname}/`,
      source: "sourceforge",
      metadata: {
        shortname: project.shortname,
        description: project.description,
        homepage: project.external_homepage || project.homepage,
        downloadPage: project.download_page,
        topics: project.categories.topic?.map((t) => t.fullname),
        languages: project.categories.language?.map((l) => l.fullname),
        licenses: project.categories.license?.map((l) => l.fullname),
        platforms: project.categories.os?.map((o) => o.fullname),
        developers: project.developers?.map((d) => d.name || d.username),
        downloads: project.stats?.downloads,
        created: project.created,
      },
    };
  }
}
