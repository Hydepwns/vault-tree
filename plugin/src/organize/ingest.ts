import { App, TFile, TFolder, Notice, normalizePath } from "obsidian";
import type { FileInfo, FileCategory, IngestResult, DuplicateInfo } from "./types";
import { hashFile, buildHashIndex } from "./fingerprint";
import { getTodayDate, generateSlug } from "../publish/validator";
import { ensureFolder } from "../utils/vault";

export interface IngestOptions {
  targetFolder: string;
  detectDuplicates: boolean;
  autoGenerateFrontmatter: boolean;
  dryRun: boolean;
}

type ProcessedFile =
  | { status: "moved"; info: FileInfo }
  | { status: "duplicate"; info: FileInfo; existingFile: string }
  | { status: "skipped"; info: FileInfo };

const buildHashLookup = async (
  app: App,
  detectDuplicates: boolean
): Promise<Map<string, string>> => {
  if (!detectDuplicates) return new Map();
  const index = await buildHashIndex(app);
  return new Map(Array.from(index.byHash).map(([hash, files]) => [hash, files[0]]));
};

const summarizeByCategory = (files: FileInfo[]): Record<FileCategory, number> =>
  files.reduce(
    (acc, f) => ({ ...acc, [f.category]: acc[f.category] + 1 }),
    { markdown: 0, image: 0, pdf: 0, other: 0 }
  );

const processFile = async (
  app: App,
  sourcePath: string,
  hashIndex: Map<string, string>,
  options: IngestOptions
): Promise<ProcessedFile> => {
  const info = analyzeFile(sourcePath);

  if (options.dryRun) {
    return { status: "skipped", info };
  }

  const existingFile = app.vault.getAbstractFileByPath(sourcePath);
  if (!existingFile || !(existingFile instanceof TFile)) {
    return { status: "skipped", info };
  }

  if (options.detectDuplicates) {
    const hash = await hashFile(app, existingFile);
    info.hash = hash;

    const existingPath = hashIndex.get(hash);
    if (existingPath && existingPath !== sourcePath) {
      return { status: "duplicate", info, existingFile: existingPath };
    }
  }

  const targetPath = normalizePath(`${options.targetFolder}/${info.name}`);

  if (targetPath !== sourcePath) {
    await app.vault.rename(existingFile, targetPath);
  }

  if (info.category === "markdown" && options.autoGenerateFrontmatter) {
    const movedFile = app.vault.getAbstractFileByPath(targetPath);
    if (movedFile instanceof TFile) {
      await addFrontmatterIfMissing(app, movedFile);
    }
  }

  return { status: "moved", info };
};

const toIngestResult = (results: ProcessedFile[]): IngestResult => {
  const files = results.map((r) => r.info);
  const duplicates = results
    .filter((r): r is Extract<ProcessedFile, { status: "duplicate" }> => r.status === "duplicate")
    .map((r) => ({ newFile: r.info.path, existingFile: r.existingFile, hash: r.info.hash ?? "" }));

  return {
    total: files.length,
    byCategory: summarizeByCategory(files),
    files,
    duplicates,
  };
};

export async function ingestFiles(
  app: App,
  sourcePaths: string[],
  options: IngestOptions
): Promise<IngestResult> {
  const hashIndex = await buildHashLookup(app, options.detectDuplicates);

  if (!options.dryRun) {
    await ensureFolder(app, options.targetFolder);
  }

  const results: ProcessedFile[] = [];
  for (const sourcePath of sourcePaths) {
    results.push(await processFile(app, sourcePath, hashIndex, options));
  }

  return toIngestResult(results);
}

export async function scanExternalDirectory(
  _app: App,
  _directoryPath: string
): Promise<FileInfo[]> {
  // Note: Obsidian's API doesn't provide direct filesystem access
  // This would need to be implemented via a native plugin or electron APIs
  // For now, return empty array - the feature works with files already in vault
  return [];
}

type ImportResult = "imported" | "skipped";

const importSingleFile = async (
  app: App,
  fileInfo: FileInfo,
  targetFolder: string,
  autoGenerateFrontmatter: boolean
): Promise<ImportResult> => {
  try {
    const targetPath = normalizePath(`${targetFolder}/${fileInfo.name}`);

    if (app.vault.getAbstractFileByPath(targetPath)) {
      return "skipped";
    }

    const existingFile = app.vault.getAbstractFileByPath(fileInfo.path);
    if (!(existingFile instanceof TFile)) {
      return "skipped";
    }

    await app.vault.rename(existingFile, targetPath);

    if (fileInfo.category === "markdown" && autoGenerateFrontmatter) {
      const movedFile = app.vault.getAbstractFileByPath(targetPath);
      if (movedFile instanceof TFile) {
        await addFrontmatterIfMissing(app, movedFile);
      }
    }

    return "imported";
  } catch (error) {
    console.warn(`[ingest] Failed to import ${fileInfo.path}:`, error);
    return "skipped";
  }
};

const countResults = (results: ImportResult[]): { imported: number; skipped: number } => ({
  imported: results.filter((r) => r === "imported").length,
  skipped: results.filter((r) => r === "skipped").length,
});

export async function importToVault(
  app: App,
  files: FileInfo[],
  targetFolder: string,
  options: { autoGenerateFrontmatter: boolean }
): Promise<{ imported: number; skipped: number }> {
  await ensureFolder(app, targetFolder);

  const results: ImportResult[] = [];
  for (const fileInfo of files) {
    results.push(await importSingleFile(app, fileInfo, targetFolder, options.autoGenerateFrontmatter));
  }

  return countResults(results);
}

function analyzeFile(path: string): FileInfo {
  const name = path.split("/").pop() || path;
  const extension = name.includes(".") ? name.split(".").pop()?.toLowerCase() || "" : "";

  return {
    path,
    name,
    extension,
    size: 0, // Would need filesystem access
    category: categorizeFile(extension),
  };
}

function categorizeFile(extension: string): FileCategory {
  switch (extension) {
    case "md":
    case "markdown":
      return "markdown";
    case "png":
    case "jpg":
    case "jpeg":
    case "gif":
    case "webp":
    case "svg":
    case "bmp":
      return "image";
    case "pdf":
      return "pdf";
    default:
      return "other";
  }
}

async function addFrontmatterIfMissing(app: App, file: TFile): Promise<void> {
  const content = await app.vault.read(file);

  if (content.trim().startsWith("---")) {
    return; // Already has frontmatter
  }

  // Generate frontmatter
  const title = file.basename;
  const date = getTodayDate();
  const slug = generateSlug(title);

  const frontmatter = `---
title: "${title}"
date: ${date}
description: ""
tags: []
slug: ${slug}
---

`;

  await app.vault.modify(file, frontmatter + content);
}

export function formatIngestResult(result: IngestResult): string {
  const header = [
    `## Ingest Summary`,
    `Total files: ${result.total}`,
    ``,
    `### By Category`,
    `- Markdown: ${result.byCategory.markdown}`,
    `- Images: ${result.byCategory.image}`,
    `- PDFs: ${result.byCategory.pdf}`,
    `- Other: ${result.byCategory.other}`,
  ];

  const duplicatesSection =
    result.duplicates.length > 0
      ? [
          ``,
          `### Duplicates Found (${result.duplicates.length})`,
          ...result.duplicates.map((d) => `- ${d.newFile} -> ${d.existingFile}`),
        ]
      : [];

  return [...header, ...duplicatesSection].join("\n");
}
