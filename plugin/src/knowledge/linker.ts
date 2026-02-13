import type { LinkSuggestion } from "./types";

export interface LinkMatch {
  suggestion: LinkSuggestion;
  matches: Array<{
    index: number;
    length: number;
    text: string;
    line: number;
    column: number;
  }>;
}

export interface InsertResult {
  originalContent: string;
  newContent: string;
  insertedLinks: number;
  skippedLinks: number;
  changes: Array<{
    targetNote: string;
    line: number;
    originalText: string;
    linkedText: string;
  }>;
}

interface LineInfo {
  lineNum: number;
  text: string;
  offset: number;
}

const escapeRegex = (str: string): string =>
  str.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

const findFrontmatterEnd = (lines: string[]): number =>
  lines[0] !== "---" ? -1 : lines.findIndex((line, i) => i > 0 && line === "---");

type Range = { start: number; end: number };

const extractRanges = (line: string, pattern: RegExp): Range[] =>
  [...line.matchAll(pattern)].map((m) => ({
    start: m.index!,
    end: m.index! + m[0].length,
  }));

const WIKI_LINK_PATTERN = /\[\[[^\]]+\]\]/g;
const CODE_SPAN_PATTERN = /`[^`]+`/g;

const extractLinks = (line: string): Range[] => extractRanges(line, WIKI_LINK_PATTERN);
const extractCodeSpans = (line: string): Range[] => extractRanges(line, CODE_SPAN_PATTERN);

const isPositionInRanges = (
  position: number,
  ranges: Array<{ start: number; end: number }>
): boolean => ranges.some((r) => position >= r.start && position < r.end);

const isPositionInLink = (line: string, position: number): boolean =>
  isPositionInRanges(position, [...extractLinks(line), ...extractCodeSpans(line)]);

const lineContainsExistingLink = (line: string, target: string): boolean =>
  [...line.matchAll(/\[\[([^\]|]+)(\|[^\]]+)?\]\]/g)].some(
    (m) => m[1].toLowerCase() === target.toLowerCase()
  );

const buildLineInfo = (lines: string[], frontmatterEnd: number): LineInfo[] =>
  lines.reduce<{ infos: LineInfo[]; offset: number }>((acc, text, lineNum) => {
    const info: LineInfo = { lineNum, text, offset: acc.offset };
    const nextOffset = acc.offset + text.length + 1;

    // Skip frontmatter lines
    if (lineNum <= frontmatterEnd) {
      return { infos: acc.infos, offset: nextOffset };
    }

    return { infos: [...acc.infos, info], offset: nextOffset };
  }, { infos: [], offset: 0 }).infos;

const findMatchesInLine = (
  line: LineInfo,
  pattern: RegExp,
  targetNote: string
): LinkMatch["matches"] => {
  if (lineContainsExistingLink(line.text, targetNote)) {
    return [];
  }

  return [...line.text.matchAll(pattern)]
    .filter((m) => !isPositionInLink(line.text, m.index!))
    .map((m) => ({
      index: line.offset + m.index!,
      length: m[0].length,
      text: m[0],
      line: line.lineNum + 1,
      column: m.index! + 1,
    }));
};

export const findLinkableMatches = (
  content: string,
  suggestions: LinkSuggestion[]
): LinkMatch[] => {
  const lines = content.split("\n");
  const frontmatterEnd = findFrontmatterEnd(lines);
  const lineInfos = buildLineInfo(lines, frontmatterEnd);

  return suggestions
    .map((suggestion) => {
      const pattern = new RegExp(`\\b${escapeRegex(suggestion.targetNote)}\\b`, "gi");
      const matches = lineInfos.flatMap((line) =>
        findMatchesInLine(line, pattern, suggestion.targetNote)
      );
      return { suggestion, matches };
    })
    .filter((result) => result.matches.length > 0);
};

const formatLink = (target: string, matchedText: string, displayText?: string): string =>
  displayText && displayText !== target
    ? `[[${target}|${displayText}]]`
    : matchedText !== target
      ? `[[${target}|${matchedText}]]`
      : `[[${target}]]`;

interface MatchWithSuggestion {
  suggestion: LinkSuggestion;
  match: LinkMatch["matches"][0];
}

const selectMatches = (
  linkMatches: LinkMatch[],
  firstMatchOnly: boolean,
  maxPerNote: number
): { selected: MatchWithSuggestion[]; skipped: number } =>
  linkMatches.reduce(
    (acc, linkMatch) => {
      const limit = firstMatchOnly ? 1 : maxPerNote;
      const selected = linkMatch.matches.slice(0, limit);
      const skipped = linkMatch.matches.length - selected.length;

      return {
        selected: [
          ...acc.selected,
          ...selected.map((match) => ({ suggestion: linkMatch.suggestion, match })),
        ],
        skipped: acc.skipped + skipped,
      };
    },
    { selected: [] as MatchWithSuggestion[], skipped: 0 }
  );

const applyReplacements = (
  content: string,
  matches: MatchWithSuggestion[],
  useDisplayText: boolean
): { newContent: string; changes: InsertResult["changes"] } => {
  // Sort descending by index to preserve positions during replacement
  const sorted = [...matches].sort((a, b) => b.match.index - a.match.index);

  return sorted.reduce(
    (acc, { suggestion, match }) => {
      const linkedText = formatLink(
        suggestion.targetNote,
        match.text,
        useDisplayText ? suggestion.suggestedText : undefined
      );

      return {
        newContent:
          acc.newContent.slice(0, match.index) +
          linkedText +
          acc.newContent.slice(match.index + match.length),
        changes: [
          {
            targetNote: suggestion.targetNote,
            line: match.line,
            originalText: match.text,
            linkedText,
          },
          ...acc.changes,
        ],
      };
    },
    { newContent: content, changes: [] as InsertResult["changes"] }
  );
};

export const insertLinks = (
  content: string,
  suggestions: LinkSuggestion[],
  options?: {
    maxPerNote?: number;
    firstMatchOnly?: boolean;
    useDisplayText?: boolean;
  }
): InsertResult => {
  const maxPerNote = options?.maxPerNote ?? 1;
  const firstMatchOnly = options?.firstMatchOnly ?? true;
  const useDisplayText = options?.useDisplayText ?? false;

  const linkMatches = findLinkableMatches(content, suggestions);
  const { selected, skipped } = selectMatches(linkMatches, firstMatchOnly, maxPerNote);
  const { newContent, changes } = applyReplacements(content, selected, useDisplayText);

  return {
    originalContent: content,
    newContent,
    insertedLinks: selected.length,
    skippedLinks: skipped,
    changes,
  };
};

export const previewChanges = (result: InsertResult): string =>
  result.changes.length === 0
    ? "No links to insert."
    : [
        `## Link Changes Preview (${result.insertedLinks} insertions)`,
        "",
        ...result.changes.map(
          (c) => `- Line ${c.line}: "${c.originalText}" -> ${c.linkedText}`
        ),
        ...(result.skippedLinks > 0
          ? ["", `(${result.skippedLinks} additional matches skipped)`]
          : []),
      ].join("\n");
