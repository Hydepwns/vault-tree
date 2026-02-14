import type { App, TFile, CachedMetadata } from "obsidian";
import type { VaultTreeSettings } from "../../settings";

export interface ToolDefinition {
  name: string;
  description: string;
  inputSchema: {
    type: "object";
    properties: Record<string, unknown>;
    required?: string[];
  };
}

export interface ToolCallResult {
  content: Array<{
    type: "text";
    text: string;
  }>;
}

export type ToolHandler = (
  app: App,
  settings: VaultTreeSettings,
  args: Record<string, unknown>
) => Promise<ToolCallResult>;

export const errorResult = (message: string): ToolCallResult => ({
  content: [{ type: "text", text: `Error: ${message}` }],
});

export const textResult = (text: string): ToolCallResult => ({
  content: [{ type: "text", text }],
});

export const getMarkdownFile = (app: App, path: string): TFile | null => {
  const file = app.vault.getAbstractFileByPath(path);
  return file instanceof TFile ? file : null;
};

export const extractTagsFromCache = (cache: CachedMetadata | null): string[] => {
  const inlineTags = cache?.tags?.map((t) => t.tag) ?? [];
  const frontmatterTags = Array.isArray(cache?.frontmatter?.tags)
    ? cache.frontmatter.tags.map((t: unknown) =>
        typeof t === "string" ? `#${t}` : String(t)
      )
    : [];
  return [...inlineTags, ...frontmatterTags];
};
