import { requestUrl } from "obsidian";
import type { KnowledgeProvider, LookupOptions, LookupResult, KnowledgeEntry } from "../types";

const NPM_REGISTRY = "https://registry.npmjs.org";

interface NpmSearchResponse {
  objects?: Array<{
    package: {
      name: string;
      version: string;
      description?: string;
      keywords?: string[];
      author?: { name?: string };
      links: {
        npm?: string;
        homepage?: string;
        repository?: string;
      };
    };
  }>;
}

interface NpmPackageInfo {
  name: string;
  description?: string;
  "dist-tags"?: { latest?: string };
  keywords?: string[];
  license?: string;
}

export class NpmProvider implements KnowledgeProvider {
  readonly name = "npm";

  async isAvailable(): Promise<boolean> {
    return true;
  }

  async lookup(query: string, options?: LookupOptions): Promise<LookupResult> {
    const maxResults = options?.maxResults ?? 5;

    try {
      // Check if query looks like a package name (no spaces)
      if (!query.includes(" ")) {
        const entry = await this.lookupPackage(query);
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
        error: `npm lookup failed: ${message}`,
      };
    }
  }

  private async search(query: string, limit: number): Promise<KnowledgeEntry[]> {
    const response = await requestUrl({
      url: `${NPM_REGISTRY}/-/v1/search?text=${encodeURIComponent(query)}&size=${limit}`,
      method: "GET",
      throw: false,
    });

    if (response.status !== 200) {
      throw new Error(`search failed: ${response.status}`);
    }

    const data = response.json as NpmSearchResponse;

    return (data.objects ?? []).map((obj) => {
      const pkg = obj.package;
      const lines: string[] = [];

      if (pkg.description) lines.push(pkg.description);
      lines.push(`Version: ${pkg.version}`);
      if (pkg.author?.name) lines.push(`Author: ${pkg.author.name}`);
      if (pkg.keywords && pkg.keywords.length > 0) {
        lines.push(`Keywords: ${pkg.keywords.slice(0, 5).join(", ")}`);
      }

      const url = pkg.links.npm ?? `https://www.npmjs.com/package/${pkg.name}`;

      return {
        title: pkg.name,
        summary: lines.join("\n"),
        url,
        source: "npm",
        metadata: {
          name: pkg.name,
          version: pkg.version,
          description: pkg.description,
          keywords: pkg.keywords,
        },
      };
    });
  }

  private async lookupPackage(name: string): Promise<KnowledgeEntry | null> {
    const response = await requestUrl({
      url: `${NPM_REGISTRY}/${encodeURIComponent(name)}`,
      method: "GET",
      throw: false,
    });

    if (response.status === 404) {
      return null;
    }

    if (response.status !== 200) {
      throw new Error(`lookup failed: ${response.status}`);
    }

    const pkg = response.json as NpmPackageInfo;
    const version = pkg["dist-tags"]?.latest ?? "unknown";

    const lines: string[] = [];
    if (pkg.description) lines.push(pkg.description);
    lines.push(`Latest: ${version}`);
    if (pkg.license) lines.push(`License: ${pkg.license}`);
    if (pkg.keywords && pkg.keywords.length > 0) {
      lines.push(`Keywords: ${pkg.keywords.slice(0, 5).join(", ")}`);
    }

    return {
      title: pkg.name,
      summary: lines.join("\n"),
      url: `https://www.npmjs.com/package/${name}`,
      source: "npm",
      metadata: {
        name: pkg.name,
        version,
        description: pkg.description,
        license: pkg.license,
      },
    };
  }
}
