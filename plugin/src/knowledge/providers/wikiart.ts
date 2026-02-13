import { requestUrl } from "obsidian";
import type { KnowledgeProvider, LookupOptions, LookupResult, KnowledgeEntry } from "../types";

const WIKIART_API = "https://www.wikiart.org/en/api/2";

interface WikiArtSearchResult {
  data?: Array<{
    id: string;
    title: string;
    artistName?: string;
    year?: string;
    image?: string;
    artistUrl?: string;
  }>;
}

interface WikiArtArtistResult {
  id?: string;
  artistName?: string;
  birthDay?: string;
  deathDay?: string;
  biography?: string;
  image?: string;
  url?: string;
}

export class WikiArtProvider implements KnowledgeProvider {
  readonly name = "wikiart";

  async isAvailable(): Promise<boolean> {
    try {
      const response = await requestUrl({
        url: `${WIKIART_API}/App/Artist/AlphabetJson`,
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

      const artistResults = await this.searchArtists(query, maxResults);
      entries.push(...artistResults);

      if (entries.length < maxResults) {
        const paintingResults = await this.searchPaintings(query, maxResults - entries.length);
        entries.push(...paintingResults);
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
        error: `WikiArt lookup failed: ${message}`,
      };
    }
  }

  private async searchArtists(query: string, limit: number): Promise<KnowledgeEntry[]> {
    const response = await requestUrl({
      url: `${WIKIART_API}/App/Search/ArtistByName?searchParameter=${encodeURIComponent(query)}`,
      method: "GET",
      throw: false,
    });

    if (response.status !== 200) {
      return [];
    }

    const data = response.json as WikiArtArtistResult[];

    if (!Array.isArray(data)) {
      return [];
    }

    return data.slice(0, limit).map((artist) => {
      const years =
        artist.birthDay || artist.deathDay
          ? ` (${artist.birthDay ?? "?"} - ${artist.deathDay ?? ""})`
          : "";

      return {
        title: artist.artistName ?? "Unknown Artist",
        summary: artist.biography?.slice(0, 500) ?? `Artist${years}`,
        url: artist.url ? `https://www.wikiart.org${artist.url}` : undefined,
        source: "wikiart",
        metadata: {
          type: "artist",
          image: artist.image,
          birthDay: artist.birthDay,
          deathDay: artist.deathDay,
        },
      };
    });
  }

  private async searchPaintings(query: string, limit: number): Promise<KnowledgeEntry[]> {
    const response = await requestUrl({
      url: `${WIKIART_API}/App/Search/PaintingsByText?searchParameter=${encodeURIComponent(query)}`,
      method: "GET",
      throw: false,
    });

    if (response.status !== 200) {
      return [];
    }

    const data = response.json as WikiArtSearchResult;

    if (!data.data || !Array.isArray(data.data)) {
      return [];
    }

    return data.data.slice(0, limit).map((painting) => ({
      title: painting.title,
      summary: `${painting.artistName ?? "Unknown"}${painting.year ? ` (${painting.year})` : ""}`,
      url: painting.artistUrl
        ? `https://www.wikiart.org${painting.artistUrl}`
        : undefined,
      source: "wikiart",
      metadata: {
        type: "painting",
        artist: painting.artistName,
        year: painting.year,
        image: painting.image,
      },
    }));
  }
}
