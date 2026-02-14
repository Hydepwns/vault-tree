import { requestUrl } from "obsidian";
import type { KnowledgeProvider, LookupOptions, LookupResult, KnowledgeEntry } from "../types";

const REDDIT_API = "https://www.reddit.com";

interface SearchResponse {
  data: {
    children: Array<{
      data: Post;
    }>;
  };
}

interface Post {
  id: string;
  title: string;
  subreddit: string;
  author: string;
  score: number;
  num_comments: number;
  permalink: string;
  selftext?: string;
  url?: string;
  is_self: boolean;
}

export class RedditProvider implements KnowledgeProvider {
  readonly name = "reddit";

  async isAvailable(): Promise<boolean> {
    return true;
  }

  async lookup(query: string, options?: LookupOptions): Promise<LookupResult> {
    const maxResults = options?.maxResults ?? 5;

    try {
      // Check if query specifies a subreddit (r/subreddit query)
      if (query.startsWith("r/")) {
        const rest = query.slice(2);
        const spaceIndex = rest.indexOf(" ");
        if (spaceIndex !== -1) {
          const sub = rest.slice(0, spaceIndex);
          const search = rest.slice(spaceIndex + 1);
          const entries = await this.searchSubreddit(sub, search, maxResults);
          return {
            success: true,
            provider: this.name,
            entries,
          };
        }
      }

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
        error: `Reddit lookup failed: ${message}`,
      };
    }
  }

  private formatCount(n: number): string {
    const abs = Math.abs(n);
    if (abs >= 1_000_000) {
      return `${(n / 1_000_000).toFixed(1)}M`;
    } else if (abs >= 1_000) {
      return `${(n / 1_000).toFixed(1)}k`;
    }
    return String(n);
  }

  private postToEntry(post: Post): KnowledgeEntry {
    const score = this.formatCount(post.score);
    const comments = this.formatCount(post.num_comments);

    const lines: string[] = [];
    lines.push(`r/${post.subreddit} | Score: ${score} | Comments: ${comments}`);
    lines.push(`Posted by u/${post.author}`);

    if (post.selftext && post.is_self) {
      const preview = post.selftext.slice(0, 200);
      const truncated = post.selftext.length > 200 ? `${preview}...` : preview;
      if (truncated) lines.push(truncated);
    }

    const permalink = `https://www.reddit.com${post.permalink}`;

    return {
      title: post.title,
      summary: lines.join("\n"),
      url: permalink,
      source: "reddit",
      metadata: {
        id: post.id,
        subreddit: post.subreddit,
        author: post.author,
        score: post.score,
        numComments: post.num_comments,
        isSelf: post.is_self,
        externalUrl: !post.is_self ? post.url : undefined,
      },
    };
  }

  private async search(query: string, limit: number): Promise<KnowledgeEntry[]> {
    const response = await requestUrl({
      url: `${REDDIT_API}/search.json?q=${encodeURIComponent(query)}&sort=relevance&limit=${limit}&type=link`,
      method: "GET",
      headers: {
        "User-Agent": "VaultTree/0.1.0",
      },
      throw: false,
    });

    if (response.status !== 200) {
      throw new Error(`search failed: ${response.status}`);
    }

    const data = response.json as SearchResponse;
    return data.data.children.map((wrapper) => this.postToEntry(wrapper.data));
  }

  private async searchSubreddit(subreddit: string, query: string, limit: number): Promise<KnowledgeEntry[]> {
    const response = await requestUrl({
      url: `${REDDIT_API}/r/${subreddit}/search.json?q=${encodeURIComponent(query)}&restrict_sr=on&sort=relevance&limit=${limit}`,
      method: "GET",
      headers: {
        "User-Agent": "VaultTree/0.1.0",
      },
      throw: false,
    });

    if (response.status === 404) {
      return [];
    }

    if (response.status !== 200) {
      throw new Error(`search failed: ${response.status}`);
    }

    const data = response.json as SearchResponse;
    return data.data.children.map((wrapper) => this.postToEntry(wrapper.data));
  }
}
