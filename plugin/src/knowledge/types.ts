export interface LookupOptions {
  maxResults?: number;
  language?: string;
}

export interface KnowledgeEntry {
  title: string;
  summary: string;
  url?: string;
  source: string;
  metadata?: Record<string, unknown>;
}

export interface LookupResult {
  success: boolean;
  provider: string;
  entries: KnowledgeEntry[];
  error?: string;
  cached?: boolean;
}

export interface KnowledgeProvider {
  readonly name: string;
  isAvailable(): Promise<boolean>;
  lookup(query: string, options?: LookupOptions): Promise<LookupResult>;
}

export interface LinkSuggestion {
  targetNote: string;
  confidence: number;
  reason: string;
  suggestedText?: string;
}

export interface SuggestLinksResult {
  success: boolean;
  provider: string;
  suggestions: LinkSuggestion[];
  error?: string;
}

export interface AIProvider {
  readonly name: string;
  isAvailable(): Promise<boolean>;
  suggestLinks(
    noteContent: string,
    notePath: string,
    vaultContext: VaultContext
  ): Promise<SuggestLinksResult>;
}

export interface VaultContext {
  noteTitles: string[];
  notePaths: string[];
  tags: string[];
}
