import type { KnowledgeProvider, LookupOptions, LookupResult, AIProvider, SuggestLinksResult, VaultContext } from "./types";
import { WikipediaProvider } from "./providers/wikipedia";
import { WikidataProvider } from "./providers/wikidata";
import { WikiArtProvider } from "./providers/wikiart";
import { OpenLibraryProvider } from "./providers/openlibrary";
import { MusicBrainzProvider } from "./providers/musicbrainz";
import { DBpediaProvider } from "./providers/dbpedia";
import { ArxivProvider } from "./providers/arxiv";
import { ShodanProvider } from "./providers/shodan";
import { GitHubProvider } from "./providers/github";
import { SourceForgeProvider } from "./providers/sourceforge";
import { DefiLlamaProvider } from "./providers/defillama";
import { GitHubProvider } from "./providers/github";
import { LRUCache, createCacheKey } from "./cache";

export type KnowledgeProviderType = "wikipedia" | "wikidata" | "wikiart" | "openlibrary" | "musicbrainz" | "dbpedia" | "arxiv" | "shodan" | "github" | "sourceforge" | "defillama" | "auto";
export type AIProviderType = "none" | "ollama" | "openai";

export interface RegistryOptions {
  cacheSize?: number;
  cacheTtlMinutes?: number;
  enableCache?: boolean;
}

const PROVIDER_ORDER: readonly string[] = [
  "wikipedia",
  "dbpedia",
  "wikidata",
  "github",
  "sourceforge",
  "openlibrary",
  "arxiv",
  "musicbrainz",
  "wikiart",
  "defillama",
  "shodan",
] as const;

const createDefaultProviders = (): KnowledgeProvider[] => [
  new WikipediaProvider(),
  new WikidataProvider(),
  new WikiArtProvider(),
  new OpenLibraryProvider(),
  new MusicBrainzProvider(),
  new DBpediaProvider(),
  new ArxivProvider(),
  new GitHubProvider(),
  new SourceForgeProvider(),
  new DefiLlamaProvider(),
];

const createErrorResult = (provider: string, error: string): LookupResult => ({
  success: false,
  provider,
  entries: [],
  error,
});

const createEmptyResult = (provider: string): LookupResult => ({
  success: true,
  provider,
  entries: [],
});

const withCacheFlag = (result: LookupResult): LookupResult => ({
  ...result,
  cached: true,
});

export class KnowledgeRegistry {
  private readonly providers: ReadonlyMap<string, KnowledgeProvider>;
  private readonly aiProviders: Map<string, AIProvider> = new Map();
  private readonly cache: LRUCache<LookupResult>;
  private readonly cacheEnabled: boolean;
  private readonly shodanProvider: ShodanProvider;
  private readonly githubProvider: GitHubProvider;

  constructor(options?: RegistryOptions) {
    this.cache = new LRUCache<LookupResult>(
      options?.cacheSize ?? 100,
      options?.cacheTtlMinutes ?? 15
    );
    this.cacheEnabled = options?.enableCache !== false;

    this.shodanProvider = new ShodanProvider();
    this.githubProvider = new GitHubProvider();
    const defaultProviders = createDefaultProviders().filter(
      (p) => p.name !== "github"
    );

    this.providers = new Map([
      ...defaultProviders.map((p): [string, KnowledgeProvider] => [p.name, p]),
      [this.shodanProvider.name, this.shodanProvider],
      [this.githubProvider.name, this.githubProvider],
    ]);
  }

  configureShodan(apiKey: string): void {
    this.shodanProvider.configure({ apiKey });
  }

  configureGitHub(token: string): void {
    this.githubProvider.configure({ token });
  }

  registerAIProvider(provider: AIProvider): void {
    this.aiProviders.set(provider.name, provider);
  }

  getProvider(name: string): KnowledgeProvider | undefined {
    return this.providers.get(name);
  }

  getAIProvider(name: string): AIProvider | undefined {
    return this.aiProviders.get(name);
  }

  private getCached(key: string): LookupResult | undefined {
    if (!this.cacheEnabled) return undefined;
    const cached = this.cache.get(key);
    return cached ? withCacheFlag(cached) : undefined;
  }

  private setCached(key: string, result: LookupResult): void {
    if (this.cacheEnabled && result.success) {
      this.cache.set(key, result);
    }
  }

  private async lookupWithProvider(
    provider: KnowledgeProvider,
    query: string,
    options?: LookupOptions
  ): Promise<LookupResult> {
    const available = await provider.isAvailable();
    if (!available) {
      return createErrorResult(provider.name, `Provider ${provider.name} is not available`);
    }
    return provider.lookup(query, options);
  }

  async lookup(
    query: string,
    providerType: KnowledgeProviderType,
    options?: LookupOptions & { skipCache?: boolean }
  ): Promise<LookupResult> {
    if (providerType === "auto") {
      return this.autoLookup(query, options);
    }

    const cacheKey = createCacheKey(providerType, query, options);

    if (!options?.skipCache) {
      const cached = this.getCached(cacheKey);
      if (cached) return cached;
    }

    const provider = this.providers.get(providerType);
    if (!provider) {
      return createErrorResult(providerType, `Unknown provider: ${providerType}`);
    }

    const result = await this.lookupWithProvider(provider, query, options);
    this.setCached(cacheKey, result);
    return result;
  }

  private async autoLookup(
    query: string,
    options?: LookupOptions & { skipCache?: boolean }
  ): Promise<LookupResult> {
    const cacheKey = createCacheKey("auto", query, options);

    if (!options?.skipCache) {
      const cached = this.getCached(cacheKey);
      if (cached) return cached;
    }

    // Try providers in order, return first successful result with entries
    const tryProvider = async (
      providerNames: readonly string[]
    ): Promise<LookupResult> => {
      if (providerNames.length === 0) {
        return createEmptyResult("auto");
      }

      const [name, ...rest] = providerNames;
      const provider = this.providers.get(name);

      if (!provider) {
        return tryProvider(rest);
      }

      const available = await provider.isAvailable();
      if (!available) {
        return tryProvider(rest);
      }

      const result = await provider.lookup(query, options);
      if (result.success && result.entries.length > 0) {
        this.setCached(cacheKey, result);
        return result;
      }

      return tryProvider(rest);
    };

    return tryProvider(PROVIDER_ORDER);
  }

  clearCache(): void {
    this.cache.clear();
  }

  getCacheStats(): { size: number; enabled: boolean } {
    return {
      size: this.cache.size(),
      enabled: this.cacheEnabled,
    };
  }

  async suggestLinks(
    providerName: string,
    noteContent: string,
    notePath: string,
    vaultContext: VaultContext
  ): Promise<SuggestLinksResult> {
    const provider = this.aiProviders.get(providerName);

    if (!provider) {
      return {
        success: false,
        provider: providerName,
        suggestions: [],
        error: `Unknown AI provider: ${providerName}`,
      };
    }

    const available = await provider.isAvailable();
    if (!available) {
      return {
        success: false,
        provider: providerName,
        suggestions: [],
        error: `AI provider ${providerName} is not available`,
      };
    }

    return provider.suggestLinks(noteContent, notePath, vaultContext);
  }

  listProviders(): string[] {
    return Array.from(this.providers.keys());
  }

  listAIProviders(): string[] {
    return Array.from(this.aiProviders.keys());
  }
}

let registry: KnowledgeRegistry | null = null;

export const getKnowledgeRegistry = (): KnowledgeRegistry => {
  if (!registry) {
    registry = new KnowledgeRegistry();
  }
  return registry;
};
