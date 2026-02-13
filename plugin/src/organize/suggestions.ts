import { App, TFile, TFolder } from "obsidian";
import type { PlacementSuggestion, FolderStats } from "./types";
import { extractTags, extractKeywords } from "../utils/metadata";

export async function suggestPlacement(
  app: App,
  file: TFile,
  folderStats: FolderStats[],
  excludeFolders: string[]
): Promise<PlacementSuggestion> {
  const content = await app.vault.cachedRead(file);
  const fileKeywords = extractKeywords(content);
  const fileTags = extractTags(content);
  const fileLinks = extractLinkTargets(content);

  const scores: Array<{ folder: string; score: number; reasons: string[] }> = [];

  for (const stats of folderStats) {
    if (excludeFolders.some((ex) => stats.path.startsWith(ex))) {
      continue;
    }

    if (stats.path === file.parent?.path) {
      continue; // Skip current folder
    }

    const reasons: string[] = [];
    let score = 0;

    // Keyword matching
    const keywordScore = calculateKeywordOverlap(fileKeywords, stats.keywords);
    if (keywordScore > 0) {
      score += keywordScore * 0.4;
      reasons.push(`Keywords match: ${(keywordScore * 100).toFixed(0)}%`);
    }

    // Tag matching
    const tagScore = calculateTagOverlap(fileTags, stats.tags);
    if (tagScore > 0) {
      score += tagScore * 0.3;
      reasons.push(`Tags match: ${(tagScore * 100).toFixed(0)}%`);
    }

    // Link matching - check if file links to notes in this folder
    const linkScore = calculateLinkScore(app, fileLinks, stats.path);
    if (linkScore > 0) {
      score += linkScore * 0.3;
      reasons.push(`Links to ${Math.round(linkScore * 10)} notes in folder`);
    }

    // Folder name matching
    const folderNameScore = calculateFolderNameMatch(file.basename, stats.path);
    if (folderNameScore > 0) {
      score += folderNameScore * 0.2;
      reasons.push(`Folder name relevant`);
    }

    if (score > 0) {
      scores.push({ folder: stats.path, score, reasons });
    }
  }

  // Sort by score descending
  scores.sort((a, b) => b.score - a.score);

  const best = scores[0];
  const alternatives = scores.slice(1, 4);

  if (!best) {
    return {
      file: file.path,
      suggestedFolder: "",
      confidence: 0,
      reasons: ["No suitable folder found"],
      alternativeFolders: [],
    };
  }

  return {
    file: file.path,
    suggestedFolder: best.folder,
    confidence: Math.min(best.score, 1),
    reasons: best.reasons,
    alternativeFolders: alternatives.map((a) => ({
      folder: a.folder,
      confidence: Math.min(a.score, 1),
    })),
  };
}

export async function buildFolderStats(
  app: App,
  excludeFolders: string[]
): Promise<FolderStats[]> {
  const stats: FolderStats[] = [];
  const folders = getAllFolders(app);

  for (const folder of folders) {
    if (excludeFolders.some((ex) => folder.path.startsWith(ex))) {
      continue;
    }

    const folderStat = await analyzeFolderContent(app, folder);
    if (folderStat.noteCount > 0) {
      stats.push(folderStat);
    }
  }

  return stats;
}

function getAllFolders(app: App): TFolder[] {
  const folders: TFolder[] = [];

  function collect(folder: TFolder) {
    folders.push(folder);
    for (const child of folder.children) {
      if (child instanceof TFolder) {
        collect(child);
      }
    }
  }

  collect(app.vault.getRoot());
  return folders;
}

async function analyzeFolderContent(app: App, folder: TFolder): Promise<FolderStats> {
  const tags: Record<string, number> = {};
  const keywords: Record<string, number> = {};
  let noteCount = 0;

  for (const child of folder.children) {
    if (child instanceof TFile && child.extension === "md") {
      noteCount++;

      try {
        const content = await app.vault.cachedRead(child);

        // Extract tags
        for (const tag of extractTags(content)) {
          tags[tag] = (tags[tag] || 0) + 1;
        }

        // Extract keywords
        for (const [keyword, count] of Object.entries(extractKeywords(content))) {
          keywords[keyword] = (keywords[keyword] || 0) + count;
        }
      } catch {
        // Skip files that can't be read
      }
    }
  }

  return {
    path: folder.path,
    noteCount,
    tags,
    keywords,
  };
}


function extractLinkTargets(content: string): string[] {
  const wikiLinks = content.match(/\[\[([^\]|#]+)/g) || [];
  return wikiLinks.map((l) => l.slice(2).toLowerCase());
}

function calculateKeywordOverlap(
  fileKeywords: Record<string, number>,
  folderKeywords: Record<string, number>
): number {
  const fileKeys = Object.keys(fileKeywords);
  const folderKeys = new Set(Object.keys(folderKeywords));

  if (fileKeys.length === 0 || folderKeys.size === 0) {
    return 0;
  }

  let overlap = 0;
  let total = 0;

  for (const key of fileKeys) {
    total += fileKeywords[key];
    if (folderKeys.has(key)) {
      overlap += fileKeywords[key];
    }
  }

  return total > 0 ? overlap / total : 0;
}

function calculateTagOverlap(
  fileTags: string[],
  folderTags: Record<string, number>
): number {
  if (fileTags.length === 0 || Object.keys(folderTags).length === 0) {
    return 0;
  }

  const folderTagSet = new Set(Object.keys(folderTags).map((t) => t.toLowerCase()));
  const matches = fileTags.filter((t) => folderTagSet.has(t.toLowerCase()));

  return matches.length / fileTags.length;
}

function calculateLinkScore(app: App, linkTargets: string[], folderPath: string): number {
  let matches = 0;

  for (const target of linkTargets) {
    const file = app.metadataCache.getFirstLinkpathDest(target, "");
    if (file && file.parent?.path === folderPath) {
      matches++;
    }
  }

  return matches / 10; // Normalize
}

function calculateFolderNameMatch(fileName: string, folderPath: string): number {
  const fileWords = fileName.toLowerCase().split(/[-_\s]+/);
  const folderName = folderPath.split("/").pop() || "";
  const folderWords = folderName.toLowerCase().split(/[-_\s]+/);

  const matches = fileWords.filter((w) => folderWords.includes(w)).length;
  return matches > 0 ? 0.2 * matches : 0;
}

