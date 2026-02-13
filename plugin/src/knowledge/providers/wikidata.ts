import { requestUrl } from "obsidian";
import type { KnowledgeProvider, LookupOptions, LookupResult, KnowledgeEntry } from "../types";

const WIKIDATA_SPARQL = "https://query.wikidata.org/sparql";
const WIKIDATA_API = "https://www.wikidata.org/w/api.php";

interface WikidataSearchResult {
  search?: Array<{
    id: string;
    label: string;
    description?: string;
    url?: string;
  }>;
}

interface SparqlResult {
  results?: {
    bindings?: Array<{
      item?: { value: string };
      itemLabel?: { value: string };
      itemDescription?: { value: string };
    }>;
  };
}

export class WikidataProvider implements KnowledgeProvider {
  readonly name = "wikidata";

  async isAvailable(): Promise<boolean> {
    try {
      const response = await requestUrl({
        url: WIKIDATA_API,
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

    try {
      if (this.isQID(query)) {
        const entry = await this.getEntityById(query, language);
        return {
          success: true,
          provider: this.name,
          entries: entry ? [entry] : [],
        };
      }

      const entries = await this.searchEntities(query, maxResults, language);

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
        error: `Wikidata lookup failed: ${message}`,
      };
    }
  }

  private isQID(query: string): boolean {
    return /^Q\d+$/i.test(query.trim());
  }

  private async searchEntities(
    query: string,
    limit: number,
    language: string
  ): Promise<KnowledgeEntry[]> {
    const params = new URLSearchParams({
      action: "wbsearchentities",
      search: query,
      language,
      limit: String(limit),
      format: "json",
      origin: "*",
    });

    const response = await requestUrl({
      url: `${WIKIDATA_API}?${params}`,
      method: "GET",
      throw: false,
    });

    if (response.status !== 200) {
      throw new Error(`Search request failed: ${response.status}`);
    }

    const data = response.json as WikidataSearchResult;

    return (data.search ?? []).map((item) => ({
      title: item.label,
      summary: item.description ?? "",
      url: `https://www.wikidata.org/wiki/${item.id}`,
      source: "wikidata",
      metadata: {
        qid: item.id,
      },
    }));
  }

  private async getEntityById(qid: string, language: string): Promise<KnowledgeEntry | null> {
    const sparqlQuery = `
      SELECT ?item ?itemLabel ?itemDescription WHERE {
        BIND(wd:${qid.toUpperCase()} AS ?item)
        SERVICE wikibase:label { bd:serviceParam wikibase:language "${language},en". }
      }
      LIMIT 1
    `;

    const params = new URLSearchParams({
      query: sparqlQuery,
      format: "json",
    });

    const response = await requestUrl({
      url: `${WIKIDATA_SPARQL}?${params}`,
      method: "GET",
      headers: {
        Accept: "application/sparql-results+json",
      },
      throw: false,
    });

    if (response.status !== 200) {
      throw new Error(`SPARQL query failed: ${response.status}`);
    }

    const data = response.json as SparqlResult;
    const binding = data.results?.bindings?.[0];

    if (!binding?.itemLabel?.value) {
      return null;
    }

    return {
      title: binding.itemLabel.value,
      summary: binding.itemDescription?.value ?? "",
      url: binding.item?.value,
      source: "wikidata",
      metadata: {
        qid: qid.toUpperCase(),
      },
    };
  }
}
