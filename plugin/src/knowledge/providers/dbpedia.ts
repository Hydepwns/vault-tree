import { requestUrl } from "obsidian";
import type { KnowledgeProvider, LookupOptions, LookupResult, KnowledgeEntry } from "../types";

const DBPEDIA_SPARQL = "https://dbpedia.org/sparql";
const DBPEDIA_LOOKUP = "https://lookup.dbpedia.org/api/search";

interface DBpediaLookupResult {
  docs?: Array<{
    resource?: string[];
    label?: string[];
    comment?: string[];
    category?: string[];
    type?: string[];
  }>;
}

interface SparqlResult {
  results?: {
    bindings?: Array<{
      [key: string]: { value: string; type?: string };
    }>;
  };
}

export class DBpediaProvider implements KnowledgeProvider {
  readonly name = "dbpedia";

  async isAvailable(): Promise<boolean> {
    try {
      const response = await requestUrl({
        url: `${DBPEDIA_LOOKUP}?query=test&maxResults=1&format=json`,
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
      // Use lookup API for search
      const entries = await this.searchLookup(query, maxResults);

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
        error: `DBpedia lookup failed: ${message}`,
      };
    }
  }

  private async searchLookup(query: string, maxResults: number): Promise<KnowledgeEntry[]> {
    const params = new URLSearchParams({
      query,
      maxResults: String(maxResults),
      format: "json",
    });

    const response = await requestUrl({
      url: `${DBPEDIA_LOOKUP}?${params}`,
      method: "GET",
      headers: {
        Accept: "application/json",
      },
      throw: false,
    });

    if (response.status !== 200) {
      throw new Error(`Lookup request failed: ${response.status}`);
    }

    const data = response.json as DBpediaLookupResult;

    if (!data.docs || !Array.isArray(data.docs)) {
      return [];
    }

    return data.docs
      .filter((doc) => doc.label?.[0])
      .map((doc) => {
        const resource = doc.resource?.[0] ?? "";
        const types = doc.type?.slice(0, 3).map((t) => t.split("/").pop()) ?? [];

        return {
          title: doc.label?.[0] ?? "Unknown",
          summary: doc.comment?.[0] ?? "",
          url: resource,
          source: "dbpedia",
          metadata: {
            types,
            categories: doc.category?.slice(0, 5),
            resourceUri: resource,
          },
        };
      });
  }

  async getResourceDetails(resourceUri: string): Promise<KnowledgeEntry | null> {
    const sparqlQuery = `
      SELECT ?label ?abstract ?thumbnail ?type WHERE {
        <${resourceUri}> rdfs:label ?label .
        OPTIONAL { <${resourceUri}> dbo:abstract ?abstract . FILTER(lang(?abstract) = "en") }
        OPTIONAL { <${resourceUri}> dbo:thumbnail ?thumbnail }
        OPTIONAL { <${resourceUri}> rdf:type ?type . FILTER(STRSTARTS(STR(?type), "http://dbpedia.org/ontology/")) }
        FILTER(lang(?label) = "en")
      }
      LIMIT 1
    `;

    const params = new URLSearchParams({
      query: sparqlQuery,
      format: "json",
    });

    const response = await requestUrl({
      url: `${DBPEDIA_SPARQL}?${params}`,
      method: "GET",
      headers: {
        Accept: "application/sparql-results+json",
      },
      throw: false,
    });

    if (response.status !== 200) {
      return null;
    }

    const data = response.json as SparqlResult;
    const binding = data.results?.bindings?.[0];

    if (!binding?.label?.value) {
      return null;
    }

    return {
      title: binding.label.value,
      summary: binding.abstract?.value?.slice(0, 500) ?? "",
      url: resourceUri,
      source: "dbpedia",
      metadata: {
        type: binding.type?.value?.split("/").pop(),
        thumbnail: binding.thumbnail?.value,
      },
    };
  }
}
