import { requestUrl } from "obsidian";
import type { KnowledgeProvider, LookupOptions, LookupResult, KnowledgeEntry } from "../types";

const ARXIV_API = "https://export.arxiv.org/api/query";

interface ArxivEntry {
  id: string;
  title: string;
  summary: string;
  authors: string[];
  published: string;
  updated: string;
  categories: string[];
  pdfLink?: string;
  doi?: string;
}

export class ArxivProvider implements KnowledgeProvider {
  readonly name = "arxiv";

  async isAvailable(): Promise<boolean> {
    try {
      const response = await requestUrl({
        url: `${ARXIV_API}?search_query=all:test&max_results=1`,
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

    try {
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
        error: `arXiv lookup failed: ${message}`,
      };
    }
  }

  private async search(query: string, maxResults: number): Promise<KnowledgeEntry[]> {
    // Build search query - search in title, abstract, and author
    const searchQuery = `all:${encodeURIComponent(query)}`;

    const params = new URLSearchParams({
      search_query: searchQuery,
      start: "0",
      max_results: String(maxResults),
      sortBy: "relevance",
      sortOrder: "descending",
    });

    const response = await requestUrl({
      url: `${ARXIV_API}?${params}`,
      method: "GET",
      throw: false,
    });

    if (response.status !== 200) {
      throw new Error(`arXiv request failed: ${response.status}`);
    }

    const entries = this.parseAtomFeed(response.text);

    return entries.map((entry) => {
      const year = entry.published.slice(0, 4);
      const authorList =
        entry.authors.length > 3
          ? `${entry.authors.slice(0, 3).join(", ")} et al.`
          : entry.authors.join(", ");

      return {
        title: entry.title,
        summary: `${authorList} (${year})\n\n${entry.summary.slice(0, 400)}...`,
        url: entry.id,
        source: "arxiv",
        metadata: {
          authors: entry.authors,
          published: entry.published,
          updated: entry.updated,
          categories: entry.categories,
          arxivId: extractArxivId(entry.id),
          pdfLink: entry.pdfLink,
          doi: entry.doi,
        },
      };
    });
  }

  private parseAtomFeed(xml: string): ArxivEntry[] {
    const entries: ArxivEntry[] = [];

    // Simple XML parsing for Atom feed
    const entryRegex = /<entry>([\s\S]*?)<\/entry>/g;
    let match;

    while ((match = entryRegex.exec(xml)) !== null) {
      const entryXml = match[1];

      const id = extractTag(entryXml, "id");
      const title = extractTag(entryXml, "title")?.replace(/\s+/g, " ").trim();
      const summary = extractTag(entryXml, "summary")?.replace(/\s+/g, " ").trim();
      const published = extractTag(entryXml, "published");
      const updated = extractTag(entryXml, "updated");

      if (!id || !title) continue;

      // Extract authors
      const authors: string[] = [];
      const authorRegex = /<author>[\s\S]*?<name>([\s\S]*?)<\/name>[\s\S]*?<\/author>/g;
      let authorMatch;
      while ((authorMatch = authorRegex.exec(entryXml)) !== null) {
        authors.push(authorMatch[1].trim());
      }

      // Extract categories
      const categories: string[] = [];
      const categoryRegex = /<category[^>]*term="([^"]+)"/g;
      let categoryMatch;
      while ((categoryMatch = categoryRegex.exec(entryXml)) !== null) {
        categories.push(categoryMatch[1]);
      }

      // Extract PDF link
      const pdfMatch = /<link[^>]*title="pdf"[^>]*href="([^"]+)"/i.exec(entryXml);
      const pdfLink = pdfMatch?.[1];

      // Extract DOI
      const doiMatch = /<arxiv:doi[^>]*>([\s\S]*?)<\/arxiv:doi>/i.exec(entryXml);
      const doi = doiMatch?.[1]?.trim();

      entries.push({
        id,
        title,
        summary: summary ?? "",
        authors,
        published: published ?? "",
        updated: updated ?? "",
        categories,
        pdfLink,
        doi,
      });
    }

    return entries;
  }
}

function extractTag(xml: string, tag: string): string | undefined {
  const regex = new RegExp(`<${tag}[^>]*>([\\s\\S]*?)<\\/${tag}>`, "i");
  const match = regex.exec(xml);
  return match?.[1]?.trim();
}

function extractArxivId(url: string): string {
  // Extract ID from URL like http://arxiv.org/abs/2301.12345v1
  const match = /arxiv\.org\/abs\/(.+)/.exec(url);
  return match?.[1] ?? url;
}
