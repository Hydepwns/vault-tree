import { requestUrl } from "obsidian";
import type { KnowledgeProvider, LookupOptions, LookupResult, KnowledgeEntry } from "../types";

const WIKIPEDIA_API = "https://en.wikipedia.org";

interface WikiSearchResult {
  query?: {
    search?: Array<{
      title: string;
      snippet: string;
      pageid: number;
    }>;
  };
}

interface WikiSummaryResult {
  title: string;
  extract: string;
  content_urls?: {
    desktop?: {
      page?: string;
    };
  };
  description?: string;
}

export class WikipediaProvider implements KnowledgeProvider {
  readonly name = "wikipedia";

  async isAvailable(): Promise<boolean> {
    try {
      const response = await requestUrl({
        url: `${WIKIPEDIA_API}/api/rest_v1/`,
        method: "GET",
        throw: false,
      });
      return response.status === 200;
    } catch {
      return false;
    }
  }

  async lookup(query: string, options?: LookupOptions): Promise<LookupResult> {
    const maxResults = options?.maxResults ?? 5;
    const language = options?.language ?? "en";
    const baseUrl = `https://${language}.wikipedia.org`;

    try {
      const searchResults = await this.search(query, maxResults, baseUrl);

      if (searchResults.length === 0) {
        return {
          success: true,
          provider: this.name,
          entries: [],
        };
      }

      const entries = await Promise.all(
        searchResults.map((title) => this.getSummary(title, baseUrl))
      );

      return {
        success: true,
        provider: this.name,
        entries: entries.filter((e): e is KnowledgeEntry => e !== null),
      };
    } catch (error) {
      const message = error instanceof Error ? error.message : "Unknown error";
      return {
        success: false,
        provider: this.name,
        entries: [],
        error: `Wikipedia lookup failed: ${message}`,
      };
    }
  }

  private async search(query: string, limit: number, baseUrl: string): Promise<string[]> {
    const params = new URLSearchParams({
      action: "query",
      list: "search",
      srsearch: query,
      srlimit: String(limit),
      format: "json",
      origin: "*",
    });

    const response = await requestUrl({
      url: `${baseUrl}/w/api.php?${params}`,
      method: "GET",
      throw: false,
    });

    if (response.status !== 200) {
      throw new Error(`Search request failed: ${response.status}`);
    }

    const data = response.json as WikiSearchResult;
    return data.query?.search?.map((r) => r.title) ?? [];
  }

  private async getSummary(title: string, baseUrl: string): Promise<KnowledgeEntry | null> {
    const encodedTitle = encodeURIComponent(title.replace(/ /g, "_"));

    const response = await requestUrl({
      url: `${baseUrl}/api/rest_v1/page/summary/${encodedTitle}`,
      method: "GET",
      throw: false,
    });

    if (response.status !== 200) {
      return null;
    }

    const data = response.json as WikiSummaryResult;

    return {
      title: data.title,
      summary: data.extract,
      url: data.content_urls?.desktop?.page,
      source: "wikipedia",
      metadata: {
        description: data.description,
      },
    };
  }
}
