import { requestUrl } from "obsidian";
import type { KnowledgeProvider, LookupOptions, LookupResult, KnowledgeEntry } from "../types";

const MUSICBRAINZ_API = "https://musicbrainz.org/ws/2";
const USER_AGENT = "VaultTree/0.1.0 (https://github.com/drooamor/vault-tree)";

interface MusicBrainzArtistResult {
  artists?: Array<{
    id: string;
    name: string;
    type?: string;
    country?: string;
    "life-span"?: {
      begin?: string;
      end?: string;
      ended?: boolean;
    };
    disambiguation?: string;
    tags?: Array<{ name: string; count: number }>;
  }>;
}

interface MusicBrainzReleaseResult {
  releases?: Array<{
    id: string;
    title: string;
    date?: string;
    country?: string;
    "artist-credit"?: Array<{
      artist: { name: string };
    }>;
    "release-group"?: {
      "primary-type"?: string;
    };
  }>;
}

interface MusicBrainzRecordingResult {
  recordings?: Array<{
    id: string;
    title: string;
    length?: number;
    "artist-credit"?: Array<{
      artist: { name: string };
    }>;
    releases?: Array<{
      title: string;
      date?: string;
    }>;
  }>;
}

export class MusicBrainzProvider implements KnowledgeProvider {
  readonly name = "musicbrainz";

  async isAvailable(): Promise<boolean> {
    try {
      const response = await requestUrl({
        url: `${MUSICBRAINZ_API}/artist?query=test&limit=1&fmt=json`,
        method: "GET",
        headers: { "User-Agent": USER_AGENT },
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

      // Search artists first
      const artistResults = await this.searchArtists(query, maxResults);
      entries.push(...artistResults);

      // If few results, search releases (albums)
      if (entries.length < maxResults) {
        const releaseResults = await this.searchReleases(query, maxResults - entries.length);
        entries.push(...releaseResults);
      }

      // If still few results, search recordings (songs)
      if (entries.length < maxResults) {
        const recordingResults = await this.searchRecordings(query, maxResults - entries.length);
        entries.push(...recordingResults);
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
        error: `MusicBrainz lookup failed: ${message}`,
      };
    }
  }

  private async searchArtists(query: string, limit: number): Promise<KnowledgeEntry[]> {
    const response = await requestUrl({
      url: `${MUSICBRAINZ_API}/artist?query=${encodeURIComponent(query)}&limit=${limit}&fmt=json`,
      method: "GET",
      headers: { "User-Agent": USER_AGENT },
      throw: false,
    });

    if (response.status !== 200) {
      return [];
    }

    const data = response.json as MusicBrainzArtistResult;

    if (!data.artists || !Array.isArray(data.artists)) {
      return [];
    }

    return data.artists.map((artist) => {
      const lifespan = artist["life-span"];
      const years =
        lifespan?.begin || lifespan?.end
          ? ` (${lifespan.begin ?? "?"} - ${lifespan.ended ? lifespan.end ?? "" : "present"})`
          : "";

      const type = artist.type ? `${artist.type}` : "Artist";
      const country = artist.country ? `, ${artist.country}` : "";
      const disambiguation = artist.disambiguation ? ` - ${artist.disambiguation}` : "";

      const topTags = artist.tags
        ?.sort((a, b) => b.count - a.count)
        .slice(0, 3)
        .map((t) => t.name)
        .join(", ");

      return {
        title: artist.name,
        summary: `${type}${country}${years}${disambiguation}${topTags ? `. Genres: ${topTags}` : ""}`,
        url: `https://musicbrainz.org/artist/${artist.id}`,
        source: "musicbrainz",
        metadata: {
          type: "artist",
          artistType: artist.type,
          country: artist.country,
          beginDate: lifespan?.begin,
          endDate: lifespan?.end,
          tags: artist.tags?.map((t) => t.name),
        },
      };
    });
  }

  private async searchReleases(query: string, limit: number): Promise<KnowledgeEntry[]> {
    const response = await requestUrl({
      url: `${MUSICBRAINZ_API}/release?query=${encodeURIComponent(query)}&limit=${limit}&fmt=json`,
      method: "GET",
      headers: { "User-Agent": USER_AGENT },
      throw: false,
    });

    if (response.status !== 200) {
      return [];
    }

    const data = response.json as MusicBrainzReleaseResult;

    if (!data.releases || !Array.isArray(data.releases)) {
      return [];
    }

    return data.releases.map((release) => {
      const artists =
        release["artist-credit"]?.map((ac) => ac.artist.name).join(", ") ?? "Unknown artist";
      const year = release.date ? ` (${release.date.slice(0, 4)})` : "";
      const type = release["release-group"]?.["primary-type"] ?? "Release";

      return {
        title: release.title,
        summary: `${type} by ${artists}${year}`,
        url: `https://musicbrainz.org/release/${release.id}`,
        source: "musicbrainz",
        metadata: {
          type: "release",
          releaseType: release["release-group"]?.["primary-type"],
          artists: release["artist-credit"]?.map((ac) => ac.artist.name),
          date: release.date,
          country: release.country,
        },
      };
    });
  }

  private async searchRecordings(query: string, limit: number): Promise<KnowledgeEntry[]> {
    const response = await requestUrl({
      url: `${MUSICBRAINZ_API}/recording?query=${encodeURIComponent(query)}&limit=${limit}&fmt=json`,
      method: "GET",
      headers: { "User-Agent": USER_AGENT },
      throw: false,
    });

    if (response.status !== 200) {
      return [];
    }

    const data = response.json as MusicBrainzRecordingResult;

    if (!data.recordings || !Array.isArray(data.recordings)) {
      return [];
    }

    return data.recordings.map((recording) => {
      const artists =
        recording["artist-credit"]?.map((ac) => ac.artist.name).join(", ") ?? "Unknown artist";

      const duration = recording.length
        ? ` [${formatDuration(recording.length)}]`
        : "";

      const album = recording.releases?.[0]?.title
        ? ` from "${recording.releases[0].title}"`
        : "";

      return {
        title: recording.title,
        summary: `Recording by ${artists}${duration}${album}`,
        url: `https://musicbrainz.org/recording/${recording.id}`,
        source: "musicbrainz",
        metadata: {
          type: "recording",
          artists: recording["artist-credit"]?.map((ac) => ac.artist.name),
          duration: recording.length,
          releases: recording.releases?.map((r) => r.title),
        },
      };
    });
  }
}

function formatDuration(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;
  return `${minutes}:${remainingSeconds.toString().padStart(2, "0")}`;
}
