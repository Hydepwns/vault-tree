export type {
  KnowledgeProvider,
  LookupOptions,
  LookupResult,
  KnowledgeEntry,
  AIProvider,
  LinkSuggestion,
  SuggestLinksResult,
  VaultContext,
} from "./types";

export {
  KnowledgeRegistry,
  getKnowledgeRegistry,
  type KnowledgeProviderType,
  type AIProviderType,
  type RegistryOptions,
} from "./registry";

export { WikipediaProvider } from "./providers/wikipedia";
export { WikidataProvider } from "./providers/wikidata";
export { WikiArtProvider } from "./providers/wikiart";
export { OpenLibraryProvider } from "./providers/openlibrary";
export { MusicBrainzProvider } from "./providers/musicbrainz";
export { DBpediaProvider } from "./providers/dbpedia";
export { ArxivProvider } from "./providers/arxiv";
export { ShodanProvider, type ShodanConfig } from "./providers/shodan";
export { GitHubProvider } from "./providers/github";
export { SourceForgeProvider } from "./providers/sourceforge";
export { DefiLlamaProvider } from "./providers/defillama";
export { OllamaProvider, type OllamaConfig } from "./providers/ai/ollama";
export { OpenAIProvider, type OpenAIConfig } from "./providers/ai/openai";

export { LRUCache, createCacheKey } from "./cache";
export {
  findLinkableMatches,
  insertLinks,
  previewChanges,
  type LinkMatch,
  type InsertResult,
} from "./linker";

export {
  generateNoteFromEntry,
  generateNotesFromEntries,
  formatNoteContent,
  type NoteTemplate,
  type GeneratorOptions,
} from "./generator";

export {
  collectFilesFromFolder,
  batchSuggestLinks,
  batchApplyLinks,
  formatBatchResult,
  formatBatchApplyResult,
  type BatchItem,
  type BatchSuggestion,
  type BatchResult,
  type BatchApplyResult,
  type BatchOptions,
} from "./batch";
