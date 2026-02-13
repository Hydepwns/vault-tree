import type { App, TFile, TFolder } from "obsidian";
import type { LinkSuggestion, VaultContext, AIProvider } from "./types";
import { insertLinks } from "./linker";

export interface BatchItem {
  file: TFile;
  content: string;
}

export interface BatchSuggestion {
  filePath: string;
  suggestions: LinkSuggestion[];
  error?: string;
}

export interface BatchResult {
  processed: number;
  successful: number;
  failed: number;
  totalSuggestions: number;
  items: BatchSuggestion[];
}

export interface BatchApplyResult {
  processed: number;
  modified: number;
  skipped: number;
  totalInserted: number;
  items: Array<{
    filePath: string;
    inserted: number;
    skipped: number;
    error?: string;
  }>;
}

export interface BatchOptions {
  maxSuggestions?: number;
  minConfidence?: number;
  concurrency?: number;
  excludePatterns?: string[];
  includePatterns?: string[];
}

const matchGlob = (path: string, pattern: string): boolean => {
  const regexPattern = pattern
    .replace(/\./g, "\\.")
    .replace(/\*\*/g, "{{DOUBLESTAR}}")
    .replace(/\*/g, "[^/]*")
    .replace(/{{DOUBLESTAR}}/g, ".*");
  return new RegExp(`^${regexPattern}$`).test(path);
};

const matchesAnyPattern = (path: string, patterns: string[]): boolean =>
  patterns.some((pattern) => matchGlob(path, pattern));

const isMarkdownFile = (child: unknown): child is TFile =>
  "extension" in (child as TFile) && (child as TFile).extension === "md";

const isFolder = (child: unknown): child is TFolder =>
  "children" in (child as TFolder);

const collectFilesRecursive = async (
  app: App,
  folder: TFolder,
  excludePatterns: string[],
  includePatterns: string[]
): Promise<BatchItem[]> => {
  const results = await Promise.all(
    folder.children.map(async (child): Promise<BatchItem[]> => {
      if (isFolder(child)) {
        return matchesAnyPattern(child.path, excludePatterns)
          ? []
          : collectFilesRecursive(app, child, excludePatterns, includePatterns);
      }

      if (!isMarkdownFile(child)) return [];
      if (matchesAnyPattern(child.path, excludePatterns)) return [];
      if (includePatterns.length > 0 && !matchesAnyPattern(child.path, includePatterns)) return [];

      const content = await app.vault.cachedRead(child);
      return [{ file: child, content }];
    })
  );

  return results.flat();
};

export const collectFilesFromFolder = async (
  app: App,
  folderPath: string,
  options?: BatchOptions
): Promise<BatchItem[]> => {
  const folder = app.vault.getAbstractFileByPath(folderPath);
  if (!folder || !isFolder(folder)) {
    throw new Error(`Folder not found: ${folderPath}`);
  }

  return collectFilesRecursive(
    app,
    folder,
    options?.excludePatterns ?? [],
    options?.includePatterns ?? ["*.md"]
  );
};

const chunk = <T>(arr: T[], size: number): T[][] =>
  arr.reduce<T[][]>(
    (chunks, item, i) =>
      i % size === 0
        ? [...chunks, [item]]
        : chunks.map((c, j) => (j === chunks.length - 1 ? [...c, item] : c)),
    []
  );

const filterByConfidence = <T extends { confidence: number }>(
  items: T[],
  minConfidence: number,
  maxResults: number
): T[] =>
  items.filter((s) => s.confidence >= minConfidence).slice(0, maxResults);

const processBatchItem = async (
  item: BatchItem,
  provider: AIProvider,
  vaultContext: VaultContext,
  maxSuggestions: number,
  minConfidence: number
): Promise<BatchSuggestion> => {
  try {
    const result = await provider.suggestLinks(item.content, item.file.path, vaultContext);

    if (!result.success) {
      return { filePath: item.file.path, suggestions: [], error: result.error };
    }

    return {
      filePath: item.file.path,
      suggestions: filterByConfidence(result.suggestions, minConfidence, maxSuggestions),
    };
  } catch (error) {
    const message = error instanceof Error ? error.message : "Unknown error";
    return { filePath: item.file.path, suggestions: [], error: message };
  }
};

const aggregateBatchResults = (items: BatchSuggestion[]): Omit<BatchResult, "items"> =>
  items.reduce(
    (acc, item) => ({
      processed: acc.processed + 1,
      successful: acc.successful + (item.error ? 0 : 1),
      failed: acc.failed + (item.error ? 1 : 0),
      totalSuggestions: acc.totalSuggestions + item.suggestions.length,
    }),
    { processed: 0, successful: 0, failed: 0, totalSuggestions: 0 }
  );

const processChunksSequentially = async <T, R>(
  chunks: T[][],
  processFn: (item: T) => Promise<R>
): Promise<R[]> => {
  const results: R[] = [];
  for (const batch of chunks) {
    const batchResults = await Promise.all(batch.map(processFn));
    results.push(...batchResults);
  }
  return results;
};

export const batchSuggestLinks = async (
  app: App,
  items: BatchItem[],
  provider: AIProvider,
  vaultContext: VaultContext,
  options?: BatchOptions
): Promise<BatchResult> => {
  const maxSuggestions = options?.maxSuggestions ?? 5;
  const minConfidence = options?.minConfidence ?? 0.5;
  const concurrency = options?.concurrency ?? 3;

  const batches = chunk(items, concurrency);
  const allResults = await processChunksSequentially(batches, (item) =>
    processBatchItem(item, provider, vaultContext, maxSuggestions, minConfidence)
  );

  return { ...aggregateBatchResults(allResults), items: allResults };
};

interface ApplyItemResult {
  filePath: string;
  inserted: number;
  skipped: number;
  error?: string;
}

const applyLinksToItem = async (
  app: App,
  item: BatchSuggestion,
  firstMatchOnly: boolean,
  dryRun: boolean
): Promise<ApplyItemResult> => {
  if (item.suggestions.length === 0) {
    return { filePath: item.filePath, inserted: 0, skipped: 0 };
  }

  const file = app.vault.getAbstractFileByPath(item.filePath);
  if (!file || !isMarkdownFile(file)) {
    return { filePath: item.filePath, inserted: 0, skipped: 0, error: "File not found" };
  }

  try {
    const content = await app.vault.read(file);
    const insertResult = insertLinks(content, item.suggestions, { firstMatchOnly });

    if (insertResult.insertedLinks > 0 && !dryRun) {
      await app.vault.modify(file, insertResult.newContent);
    }

    return {
      filePath: item.filePath,
      inserted: insertResult.insertedLinks,
      skipped: insertResult.skippedLinks,
    };
  } catch (error) {
    const message = error instanceof Error ? error.message : "Unknown error";
    return { filePath: item.filePath, inserted: 0, skipped: 0, error: message };
  }
};

const aggregateApplyResults = (items: ApplyItemResult[]): Omit<BatchApplyResult, "items"> =>
  items.reduce(
    (acc, item) => ({
      processed: acc.processed + 1,
      modified: acc.modified + (item.inserted > 0 ? 1 : 0),
      skipped: acc.skipped + (item.inserted === 0 && !item.error ? 1 : 0),
      totalInserted: acc.totalInserted + item.inserted,
    }),
    { processed: 0, modified: 0, skipped: 0, totalInserted: 0 }
  );

export const batchApplyLinks = async (
  app: App,
  batchResult: BatchResult,
  options?: { firstMatchOnly?: boolean; dryRun?: boolean }
): Promise<BatchApplyResult> => {
  const firstMatchOnly = options?.firstMatchOnly !== false;
  const dryRun = options?.dryRun === true;

  const results = await Promise.all(
    batchResult.items.map((item) => applyLinksToItem(app, item, firstMatchOnly, dryRun))
  );

  return { ...aggregateApplyResults(results), items: results };
};

export const formatBatchResult = (result: BatchResult): string => {
  const header = [
    "## Batch Link Suggestions",
    "",
    `- Processed: ${result.processed} files`,
    `- Successful: ${result.successful}`,
    `- Failed: ${result.failed}`,
    `- Total suggestions: ${result.totalSuggestions}`,
    "",
  ];

  const withSuggestions = result.items.filter((i) => i.suggestions.length > 0);

  if (withSuggestions.length === 0) {
    return [...header, "No link suggestions found."].join("\n");
  }

  const suggestions = withSuggestions.flatMap((item) => [
    `### ${item.filePath}`,
    ...item.suggestions.map(
      (s) => `- [[${s.targetNote}]] (${Math.round(s.confidence * 100)}%) - ${s.reason}`
    ),
    "",
  ]);

  const errors = result.items.filter((i) => i.error);
  const errorSection =
    errors.length > 0
      ? ["### Errors", ...errors.map((i) => `- ${i.filePath}: ${i.error}`)]
      : [];

  return [...header, ...suggestions, ...errorSection].join("\n");
};

export const formatBatchApplyResult = (result: BatchApplyResult, dryRun: boolean): string => {
  const mode = dryRun ? " (dry run)" : "";

  const header = [
    `## Batch Link Application${mode}`,
    "",
    `- Processed: ${result.processed} files`,
    `- Modified: ${result.modified}`,
    `- Skipped: ${result.skipped}`,
    `- Total links inserted: ${result.totalInserted}`,
    "",
  ];

  const modified = result.items.filter((i) => i.inserted > 0);
  const modifiedSection =
    modified.length > 0
      ? [
          "### Modified Files",
          ...modified.map((i) => `- ${i.filePath}: ${i.inserted} links inserted`),
          "",
        ]
      : [];

  const errors = result.items.filter((i) => i.error);
  const errorSection =
    errors.length > 0
      ? ["### Errors", ...errors.map((i) => `- ${i.filePath}: ${i.error}`)]
      : [];

  return [...header, ...modifiedSection, ...errorSection].join("\n");
};
