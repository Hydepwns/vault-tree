import type { App } from "obsidian";
import type { ToolDefinition, ToolCallResult } from "./types";
import type { VaultTreeSettings } from "../../settings";
import { textResult, errorResult, getMarkdownFile } from "./types";
import { getKnowledgeRegistry, type KnowledgeProviderType } from "../../knowledge/registry";
import { generateNoteFromEntry, formatNoteContent, type GeneratorOptions } from "../../knowledge/generator";

export const knowledgeDefinitions: ToolDefinition[] = [
  {
    name: "knowledge_lookup",
    description: "Look up information from Wikipedia, DBpedia, GitHub, npm, crates.io, StackOverflow, Reddit, OpenLibrary, arXiv, MusicBrainz, WikiArt, or Shodan",
    inputSchema: {
      type: "object",
      properties: {
        query: {
          type: "string",
          description: "Search query, Wikidata QID (e.g., Q42), or arXiv search terms",
        },
        provider: {
          type: "string",
          enum: ["wikipedia", "dbpedia", "wikidata", "github", "sourceforge", "npm", "crates.io", "stackoverflow", "reddit", "openlibrary", "arxiv", "musicbrainz", "wikiart", "defillama", "shodan", "auto"],
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
    name: "create_note",
    description: "Create a note from a knowledge lookup result (Wikipedia, arXiv, OpenLibrary, npm, crates.io, etc.)",
    inputSchema: {
      type: "object",
      properties: {
        query: {
          type: "string",
          description: "Search query to look up",
        },
        provider: {
          type: "string",
          enum: ["wikipedia", "dbpedia", "wikidata", "github", "sourceforge", "npm", "crates.io", "stackoverflow", "reddit", "openlibrary", "arxiv", "musicbrainz", "wikiart", "defillama", "shodan", "auto"],
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
];

export async function handleKnowledgeLookup(
  _app: App,
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

export async function handleCreateNote(
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
