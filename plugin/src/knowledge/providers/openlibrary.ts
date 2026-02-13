import { requestUrl } from "obsidian";
import type { KnowledgeProvider, LookupOptions, LookupResult, KnowledgeEntry } from "../types";

const OPENLIBRARY_API = "https://openlibrary.org";

interface OpenLibrarySearchResult {
  docs?: Array<{
    key: string;
    title: string;
    author_name?: string[];
    first_publish_year?: number;
    isbn?: string[];
    subject?: string[];
    cover_i?: number;
  }>;
  numFound?: number;
}

interface OpenLibraryAuthorResult {
  docs?: Array<{
    key: string;
    name: string;
    birth_date?: string;
    death_date?: string;
    top_work?: string;
    work_count?: number;
  }>;
}

interface OpenLibraryWorkResult {
  title?: string;
  description?: string | { value: string };
  subjects?: string[];
  authors?: Array<{ author: { key: string } }>;
}

export class OpenLibraryProvider implements KnowledgeProvider {
  readonly name = "openlibrary";

  async isAvailable(): Promise<boolean> {
    try {
      const response = await requestUrl({
        url: `${OPENLIBRARY_API}/search.json?q=test&limit=1`,
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
      const entries: KnowledgeEntry[] = [];

      // Search books first
      const bookResults = await this.searchBooks(query, maxResults);
      entries.push(...bookResults);

      // If few results, also search authors
      if (entries.length < maxResults) {
        const authorResults = await this.searchAuthors(query, maxResults - entries.length);
        entries.push(...authorResults);
      }

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
        error: `OpenLibrary lookup failed: ${message}`,
      };
    }
  }

  private async searchBooks(query: string, limit: number): Promise<KnowledgeEntry[]> {
    const params = new URLSearchParams({
      q: query,
      limit: String(limit),
      fields: "key,title,author_name,first_publish_year,isbn,subject,cover_i",
    });

    const response = await requestUrl({
      url: `${OPENLIBRARY_API}/search.json?${params}`,
      method: "GET",
      throw: false,
    });

    if (response.status !== 200) {
      return [];
    }

    const data = response.json as OpenLibrarySearchResult;

    if (!data.docs || !Array.isArray(data.docs)) {
      return [];
    }

    return data.docs.map((book) => {
      const authors = book.author_name?.join(", ") ?? "Unknown author";
      const year = book.first_publish_year ? ` (${book.first_publish_year})` : "";

      return {
        title: book.title,
        summary: `${authors}${year}`,
        url: `https://openlibrary.org${book.key}`,
        source: "openlibrary",
        metadata: {
          type: "book",
          authors: book.author_name,
          year: book.first_publish_year,
          isbn: book.isbn?.[0],
          subjects: book.subject?.slice(0, 5),
          coverId: book.cover_i,
        },
      };
    });
  }

  private async searchAuthors(query: string, limit: number): Promise<KnowledgeEntry[]> {
    const params = new URLSearchParams({
      q: query,
      limit: String(limit),
    });

    const response = await requestUrl({
      url: `${OPENLIBRARY_API}/search/authors.json?${params}`,
      method: "GET",
      throw: false,
    });

    if (response.status !== 200) {
      return [];
    }

    const data = response.json as OpenLibraryAuthorResult;

    if (!data.docs || !Array.isArray(data.docs)) {
      return [];
    }

    return data.docs.map((author) => {
      const years =
        author.birth_date || author.death_date
          ? ` (${author.birth_date ?? "?"} - ${author.death_date ?? ""})`
          : "";
      const works = author.work_count ? `, ${author.work_count} works` : "";
      const topWork = author.top_work ? `. Notable: "${author.top_work}"` : "";

      return {
        title: author.name,
        summary: `Author${years}${works}${topWork}`,
        url: `https://openlibrary.org/authors/${author.key.replace("/authors/", "")}`,
        source: "openlibrary",
        metadata: {
          type: "author",
          birthDate: author.birth_date,
          deathDate: author.death_date,
          workCount: author.work_count,
          topWork: author.top_work,
        },
      };
    });
  }

  async getWorkDetails(workKey: string): Promise<KnowledgeEntry | null> {
    const response = await requestUrl({
      url: `${OPENLIBRARY_API}${workKey}.json`,
      method: "GET",
      throw: false,
    });

    if (response.status !== 200) {
      return null;
    }

    const data = response.json as OpenLibraryWorkResult;

    const description =
      typeof data.description === "string"
        ? data.description
        : data.description?.value ?? "";

    return {
      title: data.title ?? "Unknown",
      summary: description.slice(0, 500),
      url: `https://openlibrary.org${workKey}`,
      source: "openlibrary",
      metadata: {
        type: "work",
        subjects: data.subjects?.slice(0, 10),
      },
    };
  }
}
