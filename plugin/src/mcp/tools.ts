import { App, TFile, CachedMetadata } from "obsidian";
import type { ToolDefinition, ToolCallResult } from "./types";
import { buildVaultTree } from "../tree/builder";
import { formatTreeOutput } from "../tree/renderer";
import { validateMetadata, extractMetadataFromFrontmatter, calculatePostStats } from "../publish/validator";
import { publishPost, updatePost, checkPostExists } from "../publish/bridge";
import type { PostMetadata } from "../publish/types";
import type { VaultTreeSettings } from "../settings";
import { triageInbox, formatTriageSuggestions, applyTriageDecisions } from "../organize/triage";
import { ingestFiles, formatIngestResult } from "../organize/ingest";
import { findDuplicates, buildHashIndex } from "../organize/fingerprint";
import { DEFAULT_ORGANIZE_SETTINGS } from "../organize/types";
import { getKnowledgeRegistry, type KnowledgeProviderType } from "../knowledge/registry";
import { OllamaProvider } from "../knowledge/providers/ai/ollama";
import { OpenAIProvider } from "../knowledge/providers/ai/openai";
import type { VaultContext, LinkSuggestion } from "../knowledge/types";
import { insertLinks, previewChanges } from "../knowledge/linker";
import { generateNoteFromEntry, formatNoteContent, type GeneratorOptions } from "../knowledge/generator";
import {
  collectFilesFromFolder,
  batchSuggestLinks,
  batchApplyLinks,
  formatBatchResult,
  formatBatchApplyResult,
} from "../knowledge/batch";

const errorResult = (message: string): ToolCallResult => ({
  content: [{ type: "text", text: `Error: ${message}` }],
});

const textResult = (text: string): ToolCallResult => ({
  content: [{ type: "text", text }],
});

const getMarkdownFile = (app: App, path: string): TFile | null => {
  const file = app.vault.getAbstractFileByPath(path);
  return file instanceof TFile ? file : null;
};

export function getToolDefinitions(): ToolDefinition[] {
  return [
    {
      name: "vault_tree",
      description:
        "Generate an annotated tree of the Obsidian vault showing file structure, tags, dates, and link counts",
      inputSchema: {
        type: "object",
        properties: {
          depth: {
            type: "integer",
            description: "Maximum depth to traverse (optional, default unlimited)",
          },
        },
      },
    },
    {
      name: "vault_search",
      description: "Search for a pattern across all markdown files in the vault",
      inputSchema: {
        type: "object",
        properties: {
          pattern: {
            type: "string",
            description: "Text pattern to search for",
          },
          case_insensitive: {
            type: "boolean",
            description: "Whether to perform case-insensitive search (default true)",
          },
          max_results: {
            type: "integer",
            description: "Maximum number of matches to return (optional)",
          },
        },
        required: ["pattern"],
      },
    },
    {
      name: "publish_post",
      description: "Publish a markdown file to droo.foo. Validates frontmatter and publishes the post.",
      inputSchema: {
        type: "object",
        properties: {
          file_path: {
            type: "string",
            description: "Path to the markdown file to publish (relative to vault root)",
          },
          dry_run: {
            type: "boolean",
            description: "If true, validate but don't actually publish (default false)",
          },
          update_if_exists: {
            type: "boolean",
            description: "If true, update existing post instead of failing (default true)",
          },
        },
        required: ["file_path"],
      },
    },
    {
      name: "validate_post",
      description: "Validate a markdown file's frontmatter for publishing without actually publishing",
      inputSchema: {
        type: "object",
        properties: {
          file_path: {
            type: "string",
            description: "Path to the markdown file to validate (relative to vault root)",
          },
        },
        required: ["file_path"],
      },
    },
    {
      name: "organize_triage",
      description: "Analyze inbox folder and suggest placements for notes based on content, tags, and links",
      inputSchema: {
        type: "object",
        properties: {
          inbox_folder: {
            type: "string",
            description: "Folder to triage (default: '999 Review')",
          },
          min_confidence: {
            type: "number",
            description: "Minimum confidence threshold for suggestions (0-1, default 0.5)",
          },
          auto_apply: {
            type: "boolean",
            description: "If true, automatically apply suggestions above min_confidence (default false)",
          },
        },
      },
    },
    {
      name: "organize_ingest",
      description: "Ingest files from a folder into the vault, detecting duplicates and categorizing by type",
      inputSchema: {
        type: "object",
        properties: {
          source_folder: {
            type: "string",
            description: "Source folder containing files to ingest",
          },
          target_folder: {
            type: "string",
            description: "Target folder in vault for ingested files",
          },
          detect_duplicates: {
            type: "boolean",
            description: "Check for duplicate files using content hashing (default true)",
          },
          auto_frontmatter: {
            type: "boolean",
            description: "Auto-generate frontmatter for markdown files without it (default true)",
          },
          dry_run: {
            type: "boolean",
            description: "If true, analyze but don't actually move files (default false)",
          },
        },
        required: ["source_folder", "target_folder"],
      },
    },
    {
      name: "find_duplicates",
      description: "Find duplicate files in the vault based on content hashing",
      inputSchema: {
        type: "object",
        properties: {},
      },
    },
    {
      name: "knowledge_lookup",
      description: "Look up information from Wikipedia, DBpedia, GitHub, OpenLibrary, arXiv, MusicBrainz, WikiArt, or Shodan",
      inputSchema: {
        type: "object",
        properties: {
          query: {
            type: "string",
            description: "Search query, Wikidata QID (e.g., Q42), or arXiv search terms",
          },
          provider: {
            type: "string",
            enum: ["wikipedia", "dbpedia", "wikidata", "github", "sourceforge", "openlibrary", "arxiv", "musicbrainz", "wikiart", "defillama", "shodan", "auto"],
            description: "Provider to use (default: auto)",
          },
          max_results: {
            type: "integer",
            description: "Maximum number of results (default: 5)",
          },
          language: {
            type: "string",
            description: "Language code (default: en)",
          },
        },
        required: ["query"],
      },
    },
    {
      name: "suggest_links",
      description: "Use AI to suggest internal links for a note based on content",
      inputSchema: {
        type: "object",
        properties: {
          file_path: {
            type: "string",
            description: "Path to the note file (relative to vault root)",
          },
          max_suggestions: {
            type: "integer",
            description: "Maximum number of suggestions (default: 10)",
          },
          min_confidence: {
            type: "number",
            description: "Minimum confidence threshold 0-1 (default: 0.5)",
          },
          apply: {
            type: "boolean",
            description: "If true, insert the suggested links into the note (default: false)",
          },
          first_match_only: {
            type: "boolean",
            description: "When applying, only link the first occurrence of each target (default: true)",
          },
        },
        required: ["file_path"],
      },
    },
    {
      name: "apply_links",
      description: "Insert internal links into a note at positions where target note titles appear",
      inputSchema: {
        type: "object",
        properties: {
          file_path: {
            type: "string",
            description: "Path to the note file (relative to vault root)",
          },
          targets: {
            type: "array",
            items: { type: "string" },
            description: "List of note titles to link to",
          },
          first_match_only: {
            type: "boolean",
            description: "Only link the first occurrence of each target (default: true)",
          },
          dry_run: {
            type: "boolean",
            description: "If true, preview changes without modifying the file (default: false)",
          },
        },
        required: ["file_path", "targets"],
      },
    },
    {
      name: "create_note",
      description: "Create a note from a knowledge lookup result (Wikipedia, arXiv, OpenLibrary, etc.)",
      inputSchema: {
        type: "object",
        properties: {
          query: {
            type: "string",
            description: "Search query to look up",
          },
          provider: {
            type: "string",
            enum: ["wikipedia", "dbpedia", "wikidata", "github", "sourceforge", "openlibrary", "arxiv", "musicbrainz", "wikiart", "defillama", "shodan", "auto"],
            description: "Provider to use (default: auto)",
          },
          result_index: {
            type: "integer",
            description: "Which result to use (0-indexed, default: 0)",
          },
          template_style: {
            type: "string",
            enum: ["minimal", "standard", "detailed"],
            description: "Note template style (default: standard)",
          },
          target_folder: {
            type: "string",
            description: "Folder to create note in (default: based on source)",
          },
          dry_run: {
            type: "boolean",
            description: "If true, preview without creating file (default: false)",
          },
        },
        required: ["query"],
      },
    },
    {
      name: "batch_suggest_links",
      description: "Use AI to suggest internal links for all notes in a folder",
      inputSchema: {
        type: "object",
        properties: {
          folder_path: {
            type: "string",
            description: "Folder to process (relative to vault root)",
          },
          max_suggestions: {
            type: "integer",
            description: "Max suggestions per file (default: 5)",
          },
          min_confidence: {
            type: "number",
            description: "Minimum confidence threshold 0-1 (default: 0.5)",
          },
          apply: {
            type: "boolean",
            description: "If true, insert the suggested links (default: false)",
          },
          dry_run: {
            type: "boolean",
            description: "If true with apply, preview without modifying (default: false)",
          },
          exclude_patterns: {
            type: "array",
            items: { type: "string" },
            description: "Glob patterns to exclude (e.g., ['templates/*'])",
          },
        },
        required: ["folder_path"],
      },
    },
  ];
}

export async function callTool(
  app: App,
  settings: VaultTreeSettings,
  name: string,
  args: Record<string, unknown>
): Promise<ToolCallResult> {
  switch (name) {
    case "vault_tree":
      return handleVaultTree(app, args);
    case "vault_search":
      return handleVaultSearch(app, args);
    case "publish_post":
      return handlePublishPost(app, settings, args);
    case "validate_post":
      return handleValidatePost(app, args);
    case "organize_triage":
      return handleOrganizeTriage(app, settings, args);
    case "organize_ingest":
      return handleOrganizeIngest(app, args);
    case "find_duplicates":
      return handleFindDuplicates(app);
    case "knowledge_lookup":
      return handleKnowledgeLookup(settings, args);
    case "suggest_links":
      return handleSuggestLinks(app, settings, args);
    case "apply_links":
      return handleApplyLinks(app, args);
    case "create_note":
      return handleCreateNote(app, settings, args);
    case "batch_suggest_links":
      return handleBatchSuggestLinks(app, settings, args);
    default:
      throw new Error(`Unknown tool: ${name}`);
  }
}

async function handleVaultTree(
  app: App,
  args: Record<string, unknown>
): Promise<ToolCallResult> {
  const depth = typeof args.depth === "number" ? args.depth : undefined;
  const result = await buildVaultTree(app, { depth });
  return textResult(formatTreeOutput(result));
}

type LineMatch = { lineNum: number; content: string };
type FileMatches = { path: string; matches: LineMatch[] };

const findMatchingLines = (content: string, regex: RegExp, limit: number): LineMatch[] =>
  content
    .split("\n")
    .map((line, i) => ({ lineNum: i + 1, content: line }))
    .filter(({ content }) => regex.test(content))
    .slice(0, limit);

const formatFileMatches = ({ path, matches }: FileMatches): string =>
  `## ${path}\n${matches.map((m) => `  ${m.lineNum}: ${m.content}`).join("\n")}`;

async function handleVaultSearch(
  app: App,
  args: Record<string, unknown>
): Promise<ToolCallResult> {
  const pattern = args.pattern as string;
  const caseInsensitive = args.case_insensitive !== false;
  const maxResults = typeof args.max_results === "number" ? args.max_results : 100;

  const regex = new RegExp(pattern, caseInsensitive ? "i" : "");
  const files = app.vault.getMarkdownFiles();

  const searchFile = async (file: TFile): Promise<FileMatches | null> => {
    try {
      const content = await app.vault.cachedRead(file);
      const matches = findMatchingLines(content, regex, maxResults);
      return matches.length > 0 ? { path: file.path, matches } : null;
    } catch {
      return null;
    }
  };

  const allMatches = await Promise.all(files.map(searchFile));
  const validMatches = allMatches.filter((m): m is FileMatches => m !== null);

  const truncated = validMatches.reduce<{ results: FileMatches[]; count: number }>(
    (acc, fileMatch) => {
      if (acc.count >= maxResults) return acc;
      const remaining = maxResults - acc.count;
      const truncatedMatches = fileMatch.matches.slice(0, remaining);
      return {
        results: [...acc.results, { ...fileMatch, matches: truncatedMatches }],
        count: acc.count + truncatedMatches.length,
      };
    },
    { results: [], count: 0 }
  );

  const output = truncated.results.length > 0
    ? truncated.results.map(formatFileMatches).join("\n\n")
    : "No matches found.";

  return textResult(output);
}

async function handlePublishPost(
  app: App,
  settings: VaultTreeSettings,
  args: Record<string, unknown>
): Promise<ToolCallResult> {
  const filePath = args.file_path as string;
  const dryRun = args.dry_run === true;
  const updateIfExists = args.update_if_exists !== false;

  if (!settings.apiToken && !dryRun) {
    return errorResult("API token not configured. Set it in Vault Tree plugin settings.");
  }

  const file = getMarkdownFile(app, filePath);
  if (!file) {
    return errorResult(`File not found: ${filePath}`);
  }

  const content = await app.vault.read(file);

  const metadata = extractMetadataFromFrontmatter(content);
  if (!metadata) {
    return errorResult("No frontmatter found in file. Add YAML frontmatter with title, date, description, tags, and slug.");
  }

  const validation = validateMetadata(metadata);
  if (!validation.valid) {
    const errorList = validation.errors.map((e) => `- ${e.field}: ${e.message}`).join("\n");
    return textResult(`Validation failed:\n${errorList}`);
  }

  const postMetadata = metadata as PostMetadata;
  const options = {
    apiToken: settings.apiToken,
    apiUrl: settings.apiUrl || undefined,
    dryRun,
  };

  const exists = await checkPostExists(postMetadata.slug, options);

  if (exists && !updateIfExists) {
    return errorResult(`Post with slug "${postMetadata.slug}" already exists. Set update_if_exists=true to update it.`);
  }

  const result = exists
    ? await updatePost(postMetadata.slug, content, postMetadata, options)
    : await publishPost(content, postMetadata, options);

  if (!result.success) {
    return errorResult(result.error ?? "Unknown error");
  }

  const action = exists ? "Updated" : "Published";
  const mode = dryRun ? " (dry run)" : "";
  return textResult(`${action}${mode}: ${result.url}`);
}

async function handleValidatePost(
  app: App,
  args: Record<string, unknown>
): Promise<ToolCallResult> {
  const filePath = args.file_path as string;

  const file = getMarkdownFile(app, filePath);
  if (!file) {
    return errorResult(`File not found: ${filePath}`);
  }

  const content = await app.vault.read(file);

  const metadata = extractMetadataFromFrontmatter(content);
  if (!metadata) {
    return errorResult("No frontmatter found in file.");
  }

  const validation = validateMetadata(metadata);
  const stats = calculatePostStats(content);

  const optionalFields = [
    metadata.author && `- Author: ${metadata.author}`,
    metadata.series && `- Series: ${metadata.series}`,
    metadata.series_order && `- Series Order: ${metadata.series_order}`,
  ].filter(Boolean);

  const errors = validation.errors.length > 0
    ? ["", "## Errors", ...validation.errors.map((e) => `- ${e.field}: ${e.message}`)]
    : [];

  const warnings = validation.warnings.length > 0
    ? ["", "## Warnings", ...validation.warnings.map((w) => `- ${w.field}: ${w.message}`)]
    : [];

  const output = [
    `File: ${filePath}`,
    `Status: ${validation.valid ? "Valid" : "Invalid"}`,
    "",
    "## Metadata",
    `- Title: ${metadata.title || "(missing)"}`,
    `- Slug: ${metadata.slug || "(missing)"}`,
    `- Date: ${metadata.date || "(missing)"}`,
    `- Tags: ${(metadata.tags || []).join(", ") || "(missing)"}`,
    `- Description: ${metadata.description || "(missing)"}`,
    ...optionalFields,
    "",
    "## Stats",
    `- Word count: ${stats.wordCount}`,
    `- Reading time: ${stats.estimatedReadingTime}`,
    `- Links: ${stats.linkCount}`,
    `- Images: ${stats.imageCount}`,
    ...errors,
    ...warnings,
  ].join("\n");

  return textResult(output);
}

async function handleOrganizeTriage(
  app: App,
  settings: VaultTreeSettings,
  args: Record<string, unknown>
): Promise<ToolCallResult> {
  const inboxFolder = (args.inbox_folder as string) || settings.inboxFolder || DEFAULT_ORGANIZE_SETTINGS.inboxFolder;
  const minConfidence = typeof args.min_confidence === "number" ? args.min_confidence : DEFAULT_ORGANIZE_SETTINGS.minConfidence;
  const autoApply = args.auto_apply === true;

  const organizeSettings = {
    inboxFolder,
    excludeFolders: settings.excludeFolders || DEFAULT_ORGANIZE_SETTINGS.excludeFolders,
    minConfidence,
    autoGenerateFrontmatter: DEFAULT_ORGANIZE_SETTINGS.autoGenerateFrontmatter,
  };

  const items = await triageInbox(app, organizeSettings);

  if (items.length === 0) {
    return textResult(`No files found in inbox folder: ${inboxFolder}`);
  }

  if (!autoApply) {
    return textResult(formatTriageSuggestions(items));
  }

  const markedItems = items.map((item) =>
    item.suggestion.confidence >= minConfidence && item.suggestion.suggestedFolder
      ? { ...item, status: "accepted" as const }
      : item
  );

  const result = await applyTriageDecisions(app, markedItems);

  return textResult([
    "Auto-triage complete:",
    `- Processed: ${result.processed}`,
    `- Accepted: ${result.accepted}`,
    `- Skipped (low confidence): ${items.length - result.processed}`,
  ].join("\n"));
}

async function handleOrganizeIngest(
  app: App,
  args: Record<string, unknown>
): Promise<ToolCallResult> {
  const sourceFolder = args.source_folder as string;
  const targetFolder = args.target_folder as string;
  const detectDuplicates = args.detect_duplicates !== false;
  const autoFrontmatter = args.auto_frontmatter !== false;
  const dryRun = args.dry_run === true;

  const folder = app.vault.getAbstractFileByPath(sourceFolder);
  if (!folder) {
    return errorResult(`Source folder not found: ${sourceFolder}`);
  }

  const files = "children" in folder
    ? (folder as { children: unknown[] }).children
        .filter((child): child is TFile => child instanceof TFile)
        .map((f) => f.path)
    : [];

  if (files.length === 0) {
    return textResult(`No files found in source folder: ${sourceFolder}`);
  }

  const result = await ingestFiles(app, files, {
    targetFolder,
    detectDuplicates,
    autoGenerateFrontmatter: autoFrontmatter,
    dryRun,
  });

  const mode = dryRun ? " (dry run)" : "";
  return textResult(`Ingest complete${mode}:\n\n${formatIngestResult(result)}`);
}

async function handleFindDuplicates(app: App): Promise<ToolCallResult> {
  const index = await buildHashIndex(app);
  const duplicates = findDuplicates(index);

  if (duplicates.size === 0) {
    return textResult("No duplicate files found.");
  }

  const formatGroup = ([hash, files]: [string, string[]]): string =>
    [`### Hash: ${hash.slice(0, 8)}...`, ...files.map((f) => `- ${f}`), ""].join("\n");

  const output = [
    `## Duplicate Files Found (${duplicates.size} groups)`,
    "",
    ...Array.from(duplicates.entries()).map(formatGroup),
  ].join("\n");

  return textResult(output);
}

async function handleKnowledgeLookup(
  settings: VaultTreeSettings,
  args: Record<string, unknown>
): Promise<ToolCallResult> {
  const query = args.query as string;
  const providerType = (args.provider as KnowledgeProviderType) || settings.knowledgeDefaultProvider || "auto";
  const maxResults = typeof args.max_results === "number" ? args.max_results : 5;
  const language = (args.language as string) || "en";

  const registry = getKnowledgeRegistry();

  if (settings.shodanApiKey) {
    registry.configureShodan(settings.shodanApiKey);
  }

  const result = await registry.lookup(query, providerType, { maxResults, language });

  if (!result.success) {
    return errorResult(result.error ?? "Unknown error");
  }

  if (result.entries.length === 0) {
    return textResult(`No results found for "${query}" using ${result.provider}`);
  }

  const formatEntry = (entry: typeof result.entries[0]): string =>
    [`### ${entry.title}`, entry.summary, entry.url ? `URL: ${entry.url}` : "", ""]
      .filter(Boolean)
      .join("\n");

  const cacheNote = result.cached ? " [cached]" : "";
  const output = [
    `## Results from ${result.provider} (${result.entries.length})${cacheNote}`,
    "",
    ...result.entries.map(formatEntry),
  ].join("\n");

  return textResult(output);
}

const createAIProvider = (settings: VaultTreeSettings) => {
  if (settings.aiProvider === "ollama") {
    return new OllamaProvider({
      baseUrl: settings.ollamaUrl,
      model: settings.aiModel,
    });
  }
  return new OpenAIProvider({
    apiKey: settings.aiApiKey,
    baseUrl: settings.aiApiUrl,
    model: settings.aiModel,
  });
};

async function handleSuggestLinks(
  app: App,
  settings: VaultTreeSettings,
  args: Record<string, unknown>
): Promise<ToolCallResult> {
  const filePath = args.file_path as string;
  const maxSuggestions = typeof args.max_suggestions === "number" ? args.max_suggestions : 10;
  const minConfidence = typeof args.min_confidence === "number" ? args.min_confidence : 0.5;
  const shouldApply = args.apply === true;
  const firstMatchOnly = args.first_match_only !== false;

  if (settings.aiProvider === "none") {
    return errorResult("AI provider not configured. Enable Ollama or OpenAI in plugin settings.");
  }

  const file = getMarkdownFile(app, filePath);
  if (!file) {
    return errorResult(`File not found: ${filePath}`);
  }

  const content = await app.vault.read(file);
  const vaultContext = buildVaultContext(app);

  const registry = getKnowledgeRegistry();
  registry.registerAIProvider(createAIProvider(settings));

  const result = await registry.suggestLinks(settings.aiProvider, content, filePath, vaultContext);

  if (!result.success) {
    return errorResult(result.error ?? "Unknown error");
  }

  const filtered = result.suggestions
    .filter((s) => s.confidence >= minConfidence)
    .slice(0, maxSuggestions);

  if (filtered.length === 0) {
    return textResult(`No link suggestions above ${minConfidence} confidence for "${filePath}"`);
  }

  if (shouldApply) {
    const insertResult = insertLinks(content, filtered, { firstMatchOnly });

    if (insertResult.insertedLinks > 0) {
      await app.vault.modify(file, insertResult.newContent);
    }

    const output = [
      `## Links Applied to ${filePath}`,
      `Provider: ${result.provider}`,
      "",
      previewChanges(insertResult),
    ].join("\n");

    return textResult(output);
  }

  const formatSuggestion = (s: LinkSuggestion): string => [
    `### [[${s.targetNote}]] (${Math.round(s.confidence * 100)}%)`,
    s.reason,
    s.suggestedText ? `Suggested text: "${s.suggestedText}"` : "",
    "",
  ].filter(Boolean).join("\n");

  const output = [
    `## Link Suggestions for ${filePath}`,
    `Provider: ${result.provider}`,
    "",
    ...filtered.map(formatSuggestion),
    "---",
    "Use `apply: true` to insert these links into the note.",
  ].join("\n");

  return textResult(output);
}

async function handleApplyLinks(
  app: App,
  args: Record<string, unknown>
): Promise<ToolCallResult> {
  const filePath = args.file_path as string;
  const targets = args.targets as string[];
  const firstMatchOnly = args.first_match_only !== false;
  const dryRun = args.dry_run === true;

  if (!Array.isArray(targets) || targets.length === 0) {
    return errorResult("targets must be a non-empty array of note titles");
  }

  const file = getMarkdownFile(app, filePath);
  if (!file) {
    return errorResult(`File not found: ${filePath}`);
  }

  const content = await app.vault.read(file);

  const suggestions: LinkSuggestion[] = targets.map((target) => ({
    targetNote: target,
    confidence: 1.0,
    reason: "Manual link request",
  }));

  const insertResult = insertLinks(content, suggestions, { firstMatchOnly });

  if (insertResult.insertedLinks === 0) {
    return textResult(`No matches found for the specified targets in "${filePath}"`);
  }

  if (!dryRun) {
    await app.vault.modify(file, insertResult.newContent);
  }

  const output = [
    `## Links ${dryRun ? "Preview" : "Applied"}${dryRun ? " (dry run)" : ""}`,
    `File: ${filePath}`,
    "",
    previewChanges(insertResult),
  ].join("\n");

  return textResult(output);
}

const extractTagsFromCache = (cache: CachedMetadata | null): string[] => {
  const inlineTags = cache?.tags?.map((t) => t.tag) ?? [];
  const frontmatterTags = Array.isArray(cache?.frontmatter?.tags)
    ? cache.frontmatter.tags.map((t: unknown) =>
        typeof t === "string" ? `#${t}` : String(t)
      )
    : [];
  return [...inlineTags, ...frontmatterTags];
};

function buildVaultContext(app: App): VaultContext {
  const files = app.vault.getMarkdownFiles();

  return {
    notePaths: files.map((f) => f.path),
    noteTitles: files.map((f) => f.basename),
    tags: [
      ...new Set(
        files.flatMap((f) => extractTagsFromCache(app.metadataCache.getFileCache(f)))
      ),
    ],
  };
}

async function handleCreateNote(
  app: App,
  settings: VaultTreeSettings,
  args: Record<string, unknown>
): Promise<ToolCallResult> {
  const query = args.query as string;
  const providerType = (args.provider as KnowledgeProviderType) || settings.knowledgeDefaultProvider || "auto";
  const resultIndex = typeof args.result_index === "number" ? args.result_index : 0;
  const templateStyle = (args.template_style as "minimal" | "standard" | "detailed") || "standard";
  const targetFolder = args.target_folder as string | undefined;
  const dryRun = args.dry_run === true;

  const registry = getKnowledgeRegistry();
  const result = await registry.lookup(query, providerType, { maxResults: resultIndex + 1 });

  if (!result.success) {
    return errorResult(result.error ?? "Unknown error");
  }

  if (result.entries.length === 0) {
    return textResult(`No results found for "${query}"`);
  }

  if (resultIndex >= result.entries.length) {
    return errorResult(`result_index ${resultIndex} out of range (${result.entries.length} results)`);
  }

  const entry = result.entries[resultIndex];
  const options: GeneratorOptions = {
    templateStyle,
    folderMapping: targetFolder ? { [entry.source]: targetFolder } : undefined,
  };

  const template = generateNoteFromEntry(entry, options);
  const noteContent = formatNoteContent(template);
  const notePath = targetFolder
    ? `${targetFolder}/${template.title.replace(/[<>:"/\\|?*]/g, "")}.md`
    : template.suggestedPath ?? `${template.title}.md`;

  if (dryRun) {
    const output = [
      "## Note Preview (dry run)",
      `Path: ${notePath}`,
      "",
      "```markdown",
      noteContent,
      "```",
    ].join("\n");

    return textResult(output);
  }

  const folderPath = notePath.substring(0, notePath.lastIndexOf("/"));
  if (folderPath && !app.vault.getAbstractFileByPath(folderPath)) {
    await app.vault.createFolder(folderPath);
  }

  if (app.vault.getAbstractFileByPath(notePath)) {
    return errorResult(`File already exists: ${notePath}`);
  }

  await app.vault.create(notePath, noteContent);

  return textResult(`Created note: ${notePath}`);
}

async function handleBatchSuggestLinks(
  app: App,
  settings: VaultTreeSettings,
  args: Record<string, unknown>
): Promise<ToolCallResult> {
  const folderPath = args.folder_path as string;
  const maxSuggestions = typeof args.max_suggestions === "number" ? args.max_suggestions : 5;
  const minConfidence = typeof args.min_confidence === "number" ? args.min_confidence : 0.5;
  const shouldApply = args.apply === true;
  const dryRun = args.dry_run === true;
  const excludePatterns = (args.exclude_patterns as string[]) || [];

  if (settings.aiProvider === "none") {
    return errorResult("AI provider not configured. Enable Ollama or OpenAI in plugin settings.");
  }

  let items;
  try {
    items = await collectFilesFromFolder(app, folderPath, { excludePatterns });
  } catch (error) {
    return errorResult(error instanceof Error ? error.message : "Unknown error");
  }

  if (items.length === 0) {
    return textResult(`No markdown files found in folder: ${folderPath}`);
  }

  const vaultContext = buildVaultContext(app);
  const registry = getKnowledgeRegistry();
  const provider = createAIProvider(settings);
  registry.registerAIProvider(provider);

  const batchResult = await batchSuggestLinks(app, items, provider, vaultContext, {
    maxSuggestions,
    minConfidence,
  });

  if (!shouldApply) {
    return textResult(formatBatchResult(batchResult));
  }

  const applyResult = await batchApplyLinks(app, batchResult, {
    firstMatchOnly: true,
    dryRun,
  });

  return textResult(formatBatchApplyResult(applyResult, dryRun));
}
