import { requestUrl } from "obsidian";
import type { KnowledgeProvider, LookupOptions, LookupResult, KnowledgeEntry } from "../types";

const STACKEXCHANGE_API = "https://api.stackexchange.com/2.3";

interface SearchResponse {
  items?: Question[];
}

interface Question {
  question_id: number;
  title: string;
  link: string;
  score: number;
  answer_count: number;
  is_answered: boolean;
  view_count: number;
  tags?: string[];
  owner?: {
    display_name?: string;
    reputation?: number;
  };
}

export class StackOverflowProvider implements KnowledgeProvider {
  readonly name = "stackoverflow";

  async isAvailable(): Promise<boolean> {
    return true;
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
        error: `StackOverflow lookup failed: ${message}`,
      };
    }
  }

  private formatCount(n: number): string {
    if (n >= 1_000_000) {
      return `${(n / 1_000_000).toFixed(1)}M`;
    } else if (n >= 1_000) {
      return `${(n / 1_000).toFixed(1)}k`;
    }
    return String(n);
  }

  private async search(query: string, limit: number): Promise<KnowledgeEntry[]> {
    const params = new URLSearchParams({
      order: "desc",
      sort: "relevance",
      site: "stackoverflow",
      q: query,
      pagesize: String(limit),
      filter: "withbody",
    });

    const response = await requestUrl({
      url: `${STACKEXCHANGE_API}/search/advanced?${params}`,
      method: "GET",
      throw: false,
    });

    if (response.status !== 200) {
      throw new Error(`search failed: ${response.status}`);
    }

    const data = response.json as SearchResponse;

    return (data.items ?? []).map((q) => {
      const views = this.formatCount(q.view_count);
      const answeredIcon = q.is_answered ? "[+]" : "[-]";

      const lines: string[] = [];
      lines.push(`Score: ${q.score} | Answers: ${q.answer_count} ${answeredIcon} | Views: ${views}`);
      if (q.tags && q.tags.length > 0) {
        lines.push(`Tags: ${q.tags.slice(0, 5).join(", ")}`);
      }
      if (q.owner?.display_name) {
        const rep = q.owner.reputation ? ` (${this.formatCount(q.owner.reputation)})` : "";
        lines.push(`Asked by: ${q.owner.display_name}${rep}`);
      }

      return {
        title: q.title,
        summary: lines.join("\n"),
        url: q.link,
        source: "stackoverflow",
        metadata: {
          questionId: q.question_id,
          score: q.score,
          answerCount: q.answer_count,
          isAnswered: q.is_answered,
          viewCount: q.view_count,
          tags: q.tags,
        },
      };
    });
  }
}
