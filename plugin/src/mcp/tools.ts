import { App, TFile } from "obsidian";
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
  const output = formatTreeOutput(result);

  return {
    content: [{ type: "text", text: output }],
  };
}

async function handleVaultSearch(
  app: App,
  args: Record<string, unknown>
): Promise<ToolCallResult> {
  const pattern = args.pattern as string;
  const caseInsensitive = args.case_insensitive !== false;
  const maxResults = typeof args.max_results === "number" ? args.max_results : 100;

  const regex = new RegExp(pattern, caseInsensitive ? "i" : "");
  const results: string[] = [];
  let matchCount = 0;

  const files = app.vault.getMarkdownFiles();

  for (const file of files) {
    if (matchCount >= maxResults) break;

    try {
      const content = await app.vault.cachedRead(file);
      const lines = content.split("\n");
      const fileMatches: string[] = [];

      for (let i = 0; i < lines.length; i++) {
        if (matchCount >= maxResults) break;

        if (regex.test(lines[i])) {
          fileMatches.push(`  ${i + 1}: ${lines[i]}`);
          matchCount++;
        }
      }

      if (fileMatches.length > 0) {
        results.push(`## ${file.path}\n${fileMatches.join("\n")}`);
      }
    } catch {
      // Skip files that can't be read
    }
  }

  const output = results.length > 0 ? results.join("\n\n") : "No matches found.";

  return {
    content: [{ type: "text", text: output }],
  };
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
    return {
      content: [{
        type: "text",
        text: "Error: API token not configured. Set it in Vault Tree plugin settings.",
      }],
    };
  }

  const file = app.vault.getAbstractFileByPath(filePath);
  if (!file || !(file instanceof TFile)) {
    return {
      content: [{
        type: "text",
        text: `Error: File not found: ${filePath}`,
      }],
    };
  }

  const content = await app.vault.read(file);

  const metadata = extractMetadataFromFrontmatter(content);
  if (!metadata) {
    return {
      content: [{
        type: "text",
        text: "Error: No frontmatter found in file. Add YAML frontmatter with title, date, description, tags, and slug.",
      }],
    };
  }

  const validation = validateMetadata(metadata);
  if (!validation.valid) {
    const errorList = validation.errors.map((e) => `- ${e.field}: ${e.message}`).join("\n");
    return {
      content: [{
        type: "text",
        text: `Validation failed:\n${errorList}`,
      }],
    };
  }

  const postMetadata = metadata as PostMetadata;
  const options = {
    apiToken: settings.apiToken,
    apiUrl: settings.apiUrl || undefined,
    dryRun,
  };

  const exists = await checkPostExists(postMetadata.slug, options);

  if (exists && !updateIfExists) {
    return {
      content: [{
        type: "text",
        text: `Error: Post with slug "${postMetadata.slug}" already exists. Set update_if_exists=true to update it.`,
      }],
    };
  }

  const result = exists
    ? await updatePost(postMetadata.slug, content, postMetadata, options)
    : await publishPost(content, postMetadata, options);

  if (result.success) {
    const action = exists ? "Updated" : "Published";
    const mode = dryRun ? " (dry run)" : "";
    return {
      content: [{
        type: "text",
        text: `${action}${mode}: ${result.url}`,
      }],
    };
  } else {
    return {
      content: [{
        type: "text",
        text: `Error: ${result.error}`,
      }],
    };
  }
}

async function handleValidatePost(
  app: App,
  args: Record<string, unknown>
): Promise<ToolCallResult> {
  const filePath = args.file_path as string;

  const file = app.vault.getAbstractFileByPath(filePath);
  if (!file || !(file instanceof TFile)) {
    return {
      content: [{
        type: "text",
        text: `Error: File not found: ${filePath}`,
      }],
    };
  }

  const content = await app.vault.read(file);

  const metadata = extractMetadataFromFrontmatter(content);
  if (!metadata) {
    return {
      content: [{
        type: "text",
        text: "Error: No frontmatter found in file.",
      }],
    };
  }

  const validation = validateMetadata(metadata);
  const stats = calculatePostStats(content);

  const lines: string[] = [];

  lines.push(`File: ${filePath}`);
  lines.push(`Status: ${validation.valid ? "Valid" : "Invalid"}`);
  lines.push("");
  lines.push("## Metadata");
  lines.push(`- Title: ${metadata.title || "(missing)"}`);
  lines.push(`- Slug: ${metadata.slug || "(missing)"}`);
  lines.push(`- Date: ${metadata.date || "(missing)"}`);
  lines.push(`- Tags: ${(metadata.tags || []).join(", ") || "(missing)"}`);
  lines.push(`- Description: ${metadata.description || "(missing)"}`);
  if (metadata.author) lines.push(`- Author: ${metadata.author}`);
  if (metadata.series) lines.push(`- Series: ${metadata.series}`);
  if (metadata.series_order) lines.push(`- Series Order: ${metadata.series_order}`);

  lines.push("");
  lines.push("## Stats");
  lines.push(`- Word count: ${stats.wordCount}`);
  lines.push(`- Reading time: ${stats.estimatedReadingTime}`);
  lines.push(`- Links: ${stats.linkCount}`);
  lines.push(`- Images: ${stats.imageCount}`);

  if (validation.errors.length > 0) {
    lines.push("");
    lines.push("## Errors");
    for (const error of validation.errors) {
      lines.push(`- ${error.field}: ${error.message}`);
    }
  }

  if (validation.warnings.length > 0) {
    lines.push("");
    lines.push("## Warnings");
    for (const warning of validation.warnings) {
      lines.push(`- ${warning.field}: ${warning.message}`);
    }
  }

  return {
    content: [{ type: "text", text: lines.join("\n") }],
  };
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
    return {
      content: [{
        type: "text",
        text: `No files found in inbox folder: ${inboxFolder}`,
      }],
    };
  }

  if (autoApply) {
    // Auto-accept items above confidence threshold
    for (const item of items) {
      if (item.suggestion.confidence >= minConfidence && item.suggestion.suggestedFolder) {
        item.status = "accepted";
      }
    }

    const result = await applyTriageDecisions(app, items);

    return {
      content: [{
        type: "text",
        text: `Auto-triage complete:\n- Processed: ${result.processed}\n- Accepted: ${result.accepted}\n- Skipped (low confidence): ${items.length - result.processed}`,
      }],
    };
  }

  // Return suggestions for review
  const output = formatTriageSuggestions(items);

  return {
    content: [{ type: "text", text: output }],
  };
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

  // Get files from source folder
  const folder = app.vault.getAbstractFileByPath(sourceFolder);
  if (!folder) {
    return {
      content: [{
        type: "text",
        text: `Error: Source folder not found: ${sourceFolder}`,
      }],
    };
  }

  const files: string[] = [];
  if ("children" in folder) {
    for (const child of (folder as any).children) {
      if (child instanceof TFile) {
        files.push(child.path);
      }
    }
  }

  if (files.length === 0) {
    return {
      content: [{
        type: "text",
        text: `No files found in source folder: ${sourceFolder}`,
      }],
    };
  }

  const result = await ingestFiles(app, files, {
    targetFolder,
    detectDuplicates,
    autoGenerateFrontmatter: autoFrontmatter,
    dryRun,
  });

  const output = formatIngestResult(result);
  const mode = dryRun ? " (dry run)" : "";

  return {
    content: [{
      type: "text",
      text: `Ingest complete${mode}:\n\n${output}`,
    }],
  };
}

async function handleFindDuplicates(app: App): Promise<ToolCallResult> {
  const index = await buildHashIndex(app);
  const duplicates = findDuplicates(index);

  if (duplicates.size === 0) {
    return {
      content: [{
        type: "text",
        text: "No duplicate files found.",
      }],
    };
  }

  const lines: string[] = [];
  lines.push(`## Duplicate Files Found (${duplicates.size} groups)`);
  lines.push("");

  for (const [hash, files] of duplicates) {
    lines.push(`### Hash: ${hash.slice(0, 8)}...`);
    for (const file of files) {
      lines.push(`- ${file}`);
    }
    lines.push("");
  }

  return {
    content: [{ type: "text", text: lines.join("\n") }],
  };
}
