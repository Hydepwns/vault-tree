import type { App, TFile } from "obsidian";
import type { ToolDefinition, ToolCallResult } from "./types";
import { textResult } from "./types";
import { buildVaultTree } from "../../tree/builder";
import { formatTreeOutput } from "../../tree/renderer";

export const vaultDefinitions: ToolDefinition[] = [
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
];

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

export async function handleVaultTree(
  app: App,
  _settings: unknown,
  args: Record<string, unknown>
): Promise<ToolCallResult> {
  const depth = typeof args.depth === "number" ? args.depth : undefined;
  const result = await buildVaultTree(app, { depth });
  return textResult(formatTreeOutput(result));
}

export async function handleVaultSearch(
  app: App,
  _settings: unknown,
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
