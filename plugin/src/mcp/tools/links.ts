import type { App } from "obsidian";
import type { ToolDefinition, ToolCallResult } from "./types";
import type { VaultTreeSettings } from "../../settings";
import { textResult, errorResult, getMarkdownFile, extractTagsFromCache } from "./types";
import { getKnowledgeRegistry } from "../../knowledge/registry";
import { OllamaProvider } from "../../knowledge/providers/ai/ollama";
import { OpenAIProvider } from "../../knowledge/providers/ai/openai";
import type { VaultContext, LinkSuggestion } from "../../knowledge/types";
import { insertLinks, previewChanges } from "../../knowledge/linker";
import {
  collectFilesFromFolder,
  batchSuggestLinks,
  batchApplyLinks,
  formatBatchResult,
  formatBatchApplyResult,
} from "../../knowledge/batch";

export const linksDefinitions: ToolDefinition[] = [
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

export async function handleSuggestLinks(
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

export async function handleApplyLinks(
  app: App,
  _settings: VaultTreeSettings,
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

export async function handleBatchSuggestLinks(
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
