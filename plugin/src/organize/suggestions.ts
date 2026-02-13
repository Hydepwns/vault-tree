import { App, TFile, TFolder } from "obsidian";
import type { PlacementSuggestion, FolderStats } from "./types";
import { extractTags, extractKeywords } from "../utils/metadata";

type ScoredFolder = { folder: string; score: number; reasons: string[] };

type ScoringContext = {
  app: App;
  fileKeywords: Record<string, number>;
  fileTags: string[];
  fileLinks: string[];
  fileName: string;
};

type ScoringFactor = {
  weight: number;
  calculate: (stats: FolderStats, ctx: ScoringContext) => number;
  describe: (rawScore: number) => string;
};

const scoringFactors: ScoringFactor[] = [
  {
    weight: 0.4,
    calculate: (stats, ctx) => calculateKeywordOverlap(ctx.fileKeywords, stats.keywords),
    describe: (s) => `Keywords match: ${(s * 100).toFixed(0)}%`,
  },
  {
    weight: 0.3,
    calculate: (stats, ctx) => calculateTagOverlap(ctx.fileTags, stats.tags),
    describe: (s) => `Tags match: ${(s * 100).toFixed(0)}%`,
  },
  {
    weight: 0.3,
    calculate: (stats, ctx) => calculateLinkScore(ctx.app, ctx.fileLinks, stats.path),
    describe: (s) => `Links to ${Math.round(s * 10)} notes in folder`,
  },
  {
    weight: 0.2,
    calculate: (stats, ctx) => calculateFolderNameMatch(ctx.fileName, stats.path),
    describe: () => `Folder name relevant`,
  },
];

const scoreFolder = (stats: FolderStats, ctx: ScoringContext): ScoredFolder => {
  const contributions = scoringFactors
    .map((factor) => {
      const rawScore = factor.calculate(stats, ctx);
      return { rawScore, weighted: rawScore * factor.weight, describe: factor.describe };
    })
    .filter(({ rawScore }) => rawScore > 0);

  return {
    folder: stats.path,
    score: contributions.reduce((sum, c) => sum + c.weighted, 0),
    reasons: contributions.map((c) => c.describe(c.rawScore)),
  };
};

const isCandidate = (
  stats: FolderStats,
  excludeFolders: string[],
  currentPath: string | undefined
): boolean =>
  !excludeFolders.some((ex) => stats.path.startsWith(ex)) &&
  stats.path !== currentPath;

const noSuggestion = (filePath: string): PlacementSuggestion => ({
  file: filePath,
  suggestedFolder: "",
  confidence: 0,
  reasons: ["No suitable folder found"],
  alternativeFolders: [],
});

const toPlacementSuggestion = (
  filePath: string,
  [best, ...alternatives]: ScoredFolder[]
): PlacementSuggestion =>
  best
    ? {
        file: filePath,
        suggestedFolder: best.folder,
        confidence: Math.min(best.score, 1),
        reasons: best.reasons,
        alternativeFolders: alternatives.slice(0, 3).map((a) => ({
          folder: a.folder,
          confidence: Math.min(a.score, 1),
        })),
      }
    : noSuggestion(filePath);

export async function suggestPlacement(
  app: App,
  file: TFile,
  folderStats: FolderStats[],
  excludeFolders: string[]
): Promise<PlacementSuggestion> {
  const content = await app.vault.cachedRead(file);

  const ctx: ScoringContext = {
    app,
    fileKeywords: extractKeywords(content),
    fileTags: extractTags(content),
    fileLinks: extractLinkTargets(content),
    fileName: file.basename,
  };

  const scored = folderStats
    .filter((stats) => isCandidate(stats, excludeFolders, file.parent?.path))
    .map((stats) => scoreFolder(stats, ctx))
    .filter((s) => s.score > 0)
    .sort((a, b) => b.score - a.score);

  return toPlacementSuggestion(file.path, scored);
}

export async function buildFolderStats(
  app: App,
  excludeFolders: string[]
): Promise<FolderStats[]> {
  const folders = getAllFolders(app);

  const filtered = folders.filter(
    (folder) => !excludeFolders.some((ex) => folder.path.startsWith(ex))
  );

  const analyzed = await Promise.all(
    filtered.map((folder) => analyzeFolderContent(app, folder))
  );

  return analyzed.filter((stats) => stats.noteCount > 0);
}

const collectFolders = (folder: TFolder): TFolder[] => [
  folder,
  ...folder.children
    .filter((child): child is TFolder => child instanceof TFolder)
    .flatMap(collectFolders),
];

function getAllFolders(app: App): TFolder[] {
  return collectFolders(app.vault.getRoot());
}

type ContentAnalysis = {
  tags: Record<string, number>;
  keywords: Record<string, number>;
};

const toFrequencyMap = (items: string[]): Record<string, number> =>
  items.reduce<Record<string, number>>(
    (acc, item) => ({ ...acc, [item]: (acc[item] ?? 0) + 1 }),
    {}
  );

const mergeFrequencyMap = (
  a: Record<string, number>,
  b: Record<string, number>
): Record<string, number> =>
  Object.entries(b).reduce((acc, [k, v]) => ({ ...acc, [k]: (acc[k] ?? 0) + v }), a);

const mergeAnalysis = (a: ContentAnalysis, b: ContentAnalysis): ContentAnalysis => ({
  tags: mergeFrequencyMap(a.tags, b.tags),
  keywords: mergeFrequencyMap(a.keywords, b.keywords),
});

const analyzeContent = (content: string): ContentAnalysis => ({
  tags: toFrequencyMap(extractTags(content)),
  keywords: extractKeywords(content),
});

const safeRead = async (app: App, file: TFile): Promise<string | null> => {
  try {
    return await app.vault.cachedRead(file);
  } catch (error) {
    console.warn(`[suggestions] Failed to read ${file.path}:`, error);
    return null;
  }
};

async function analyzeFolderContent(app: App, folder: TFolder): Promise<FolderStats> {
  const mdFiles = folder.children.filter(
    (child): child is TFile => child instanceof TFile && child.extension === "md"
  );

  const contents = await Promise.all(mdFiles.map((f) => safeRead(app, f)));
  const validContents = contents.filter((c): c is string => c !== null);

  const combined = validContents
    .map(analyzeContent)
    .reduce(mergeAnalysis, { tags: {}, keywords: {} });

  return {
    path: folder.path,
    noteCount: mdFiles.length,
    ...combined,
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
  const fileEntries = Object.entries(fileKeywords);
  const folderKeys = new Set(Object.keys(folderKeywords));

  if (fileEntries.length === 0 || folderKeys.size === 0) {
    return 0;
  }

  const { overlap, total } = fileEntries.reduce(
    (acc, [key, count]) => ({
      total: acc.total + count,
      overlap: acc.overlap + (folderKeys.has(key) ? count : 0),
    }),
    { overlap: 0, total: 0 }
  );

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
  const matches = linkTargets.filter((target) => {
    const file = app.metadataCache.getFirstLinkpathDest(target, "");
    return file?.parent?.path === folderPath;
  }).length;

  return matches / 10;
}

function calculateFolderNameMatch(fileName: string, folderPath: string): number {
  const fileWords = fileName.toLowerCase().split(/[-_\s]+/);
  const folderName = folderPath.split("/").pop() || "";
  const folderWords = folderName.toLowerCase().split(/[-_\s]+/);

  const matches = fileWords.filter((w) => folderWords.includes(w)).length;
  return matches > 0 ? 0.2 * matches : 0;
}

