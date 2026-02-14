import { requestUrl } from "obsidian";
import type { KnowledgeProvider, LookupOptions, LookupResult, KnowledgeEntry } from "../types";

const CRATES_API = "https://crates.io/api/v1";

interface CrateSearchResponse {
  crates?: Crate[];
}

interface CrateResponse {
  crate: Crate;
}

interface Crate {
  name: string;
  description?: string;
  max_version?: string;
  max_stable_version?: string;
  downloads: number;
  repository?: string;
  documentation?: string;
  keywords?: string[];
}

export class CratesIoProvider implements KnowledgeProvider {
  readonly name = "crates.io";

  async isAvailable(): Promise<boolean> {
    return true;
  }

  async lookup(query: string, options?: LookupOptions): Promise<LookupResult> {
    const maxResults = options?.maxResults ?? 5;

    try {
      // Check if query looks like a crate name (no spaces, valid chars)
      if (!query.includes(" ") && /^[\w-]+$/.test(query)) {
        const entry = await this.lookupCrate(query);
        if (entry) {
          return {
            success: true,
            provider: this.name,
            entries: [entry],
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
        error: `crates.io lookup failed: ${message}`,
      };
    }
  }

  private formatDownloads(n: number): string {
    if (n >= 1_000_000) {
      return `${(n / 1_000_000).toFixed(1)}M`;
    } else if (n >= 1_000) {
      return `${(n / 1_000).toFixed(1)}k`;
    }
    return String(n);
  }

  private crateToEntry(crate: Crate): KnowledgeEntry {
    const version = crate.max_stable_version ?? crate.max_version ?? "unknown";
    const downloads = this.formatDownloads(crate.downloads);

    const lines: string[] = [];
    if (crate.description) lines.push(crate.description);
    lines.push(`Version: ${version} | Downloads: ${downloads}`);
    if (crate.keywords && crate.keywords.length > 0) {
      lines.push(`Keywords: ${crate.keywords.slice(0, 5).join(", ")}`);
    }

    return {
      title: crate.name,
      summary: lines.join("\n"),
      url: `https://crates.io/crates/${crate.name}`,
      source: "crates.io",
      metadata: {
        name: crate.name,
        version,
        downloads: crate.downloads,
        description: crate.description,
        repository: crate.repository,
        documentation: crate.documentation,
      },
    };
  }

  private async search(query: string, limit: number): Promise<KnowledgeEntry[]> {
    const response = await requestUrl({
      url: `${CRATES_API}/crates?q=${encodeURIComponent(query)}&per_page=${limit}&sort=downloads`,
      method: "GET",
      headers: {
        "User-Agent": "VaultTree/0.1.0",
      },
      throw: false,
    });

    if (response.status !== 200) {
      throw new Error(`search failed: ${response.status}`);
    }

    const data = response.json as CrateSearchResponse;
    return (data.crates ?? []).map((c) => this.crateToEntry(c));
  }

  private async lookupCrate(name: string): Promise<KnowledgeEntry | null> {
    const response = await requestUrl({
      url: `${CRATES_API}/crates/${encodeURIComponent(name)}`,
      method: "GET",
      headers: {
        "User-Agent": "VaultTree/0.1.0",
      },
      throw: false,
    });

    if (response.status === 404) {
      return null;
    }

    if (response.status !== 200) {
      throw new Error(`lookup failed: ${response.status}`);
    }

    const data = response.json as CrateResponse;
    return this.crateToEntry(data.crate);
  }
}
