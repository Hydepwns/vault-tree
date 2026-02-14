import type { App } from "obsidian";
import type { ToolDefinition, ToolCallResult } from "./types";
import type { VaultTreeSettings } from "../../settings";
import { textResult, errorResult, getMarkdownFile } from "./types";
import { validateMetadata, extractMetadataFromFrontmatter, calculatePostStats } from "../../publish/validator";
import { publishPost, updatePost, checkPostExists } from "../../publish/bridge";
import type { PostMetadata } from "../../publish/types";

export const publishDefinitions: ToolDefinition[] = [
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
];

export async function handlePublishPost(
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

export async function handleValidatePost(
  app: App,
  _settings: VaultTreeSettings,
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
