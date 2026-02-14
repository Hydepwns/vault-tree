import type { App, TFile } from "obsidian";
import type { ToolDefinition, ToolCallResult } from "./types";
import type { VaultTreeSettings } from "../../settings";
import { textResult, errorResult } from "./types";
import { triageInbox, formatTriageSuggestions, applyTriageDecisions } from "../../organize/triage";
import { ingestFiles, formatIngestResult } from "../../organize/ingest";
import { findDuplicates, buildHashIndex } from "../../organize/fingerprint";
import { DEFAULT_ORGANIZE_SETTINGS } from "../../organize/types";

export const organizeDefinitions: ToolDefinition[] = [
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
];

export async function handleOrganizeTriage(
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

export async function handleOrganizeIngest(
  app: App,
  _settings: VaultTreeSettings,
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

export async function handleFindDuplicates(
  app: App,
  _settings: VaultTreeSettings,
  _args: Record<string, unknown>
): Promise<ToolCallResult> {
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
