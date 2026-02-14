import type { App } from "obsidian";
import type { VaultTreeSettings } from "../../settings";
import type { ToolDefinition, ToolCallResult } from "./types";
import { vaultDefinitions, handleVaultTree, handleVaultSearch } from "./vault";
import { publishDefinitions, handlePublishPost, handleValidatePost } from "./publish";
import { organizeDefinitions, handleOrganizeTriage, handleOrganizeIngest, handleFindDuplicates } from "./organize";
import { knowledgeDefinitions, handleKnowledgeLookup, handleCreateNote } from "./knowledge";
import { linksDefinitions, handleSuggestLinks, handleApplyLinks, handleBatchSuggestLinks } from "./links";

export type { ToolDefinition, ToolCallResult } from "./types";

export function getToolDefinitions(): ToolDefinition[] {
  return [
    ...vaultDefinitions,
    ...publishDefinitions,
    ...organizeDefinitions,
    ...knowledgeDefinitions,
    ...linksDefinitions,
  ];
}

type Handler = (
  app: App,
  settings: VaultTreeSettings,
  args: Record<string, unknown>
) => Promise<ToolCallResult>;

const handlers: Record<string, Handler> = {
  vault_tree: handleVaultTree,
  vault_search: handleVaultSearch,
  publish_post: handlePublishPost,
  validate_post: handleValidatePost,
  organize_triage: handleOrganizeTriage,
  organize_ingest: handleOrganizeIngest,
  find_duplicates: handleFindDuplicates,
  knowledge_lookup: handleKnowledgeLookup,
  create_note: handleCreateNote,
  suggest_links: handleSuggestLinks,
  apply_links: handleApplyLinks,
  batch_suggest_links: handleBatchSuggestLinks,
};

export async function callTool(
  app: App,
  settings: VaultTreeSettings,
  name: string,
  args: Record<string, unknown>
): Promise<ToolCallResult> {
  const handler = handlers[name];
  if (!handler) {
    throw new Error(`Unknown tool: ${name}`);
  }
  return handler(app, settings, args);
}
