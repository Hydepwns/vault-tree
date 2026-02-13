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

export async function ingestFiles(
  app: App,
  sourcePaths: string[],
  options: IngestOptions
): Promise<IngestResult> {
  const result: IngestResult = {
    total: 0,
    byCategory: { markdown: 0, image: 0, pdf: 0, other: 0 },
    files: [],
    duplicates: [],
  };

  // Build hash index for duplicate detection
  let hashIndex: Map<string, string> | null = null;
  if (options.detectDuplicates) {
    const index = await buildHashIndex(app);
    hashIndex = new Map();
    for (const [hash, files] of index.byHash) {
      hashIndex.set(hash, files[0]);
    }
  }

  // Ensure target folder exists
  if (!options.dryRun) {
    await ensureFolder(app, options.targetFolder);
  }

  for (const sourcePath of sourcePaths) {
    const fileInfo = analyzeFile(sourcePath);
    result.total++;
    result.byCategory[fileInfo.category]++;
    result.files.push(fileInfo);

    if (options.dryRun) {
      continue;
    }

    // Read file content (for external files, this would need filesystem access)
    // For now, we assume files are already in the vault or accessible
    const existingFile = app.vault.getAbstractFileByPath(sourcePath);
    if (!existingFile || !(existingFile instanceof TFile)) {
      continue;
    }

    // Check for duplicates
    if (hashIndex && options.detectDuplicates) {
      const hash = await hashFile(app, existingFile);
      fileInfo.hash = hash;

      const existingPath = hashIndex.get(hash);
      if (existingPath && existingPath !== sourcePath) {
        result.duplicates.push({
          newFile: sourcePath,
          existingFile: existingPath,
          hash,
        });
        continue; // Skip duplicates
      }
    }

    // Move/copy file to target folder
    const targetPath = normalizePath(`${options.targetFolder}/${fileInfo.name}`);

    if (targetPath !== sourcePath) {
      await app.vault.rename(existingFile, targetPath);
    }

    // Add frontmatter if markdown and missing
    if (fileInfo.category === "markdown" && options.autoGenerateFrontmatter) {
      const movedFile = app.vault.getAbstractFileByPath(targetPath);
      if (movedFile instanceof TFile) {
        await addFrontmatterIfMissing(app, movedFile);
      }
    }
  }

  return result;
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

export async function importToVault(
  app: App,
  files: FileInfo[],
  targetFolder: string,
  options: { autoGenerateFrontmatter: boolean }
): Promise<{ imported: number; skipped: number }> {
  let imported = 0;
  let skipped = 0;

  await ensureFolder(app, targetFolder);

  for (const fileInfo of files) {
    try {
      const targetPath = normalizePath(`${targetFolder}/${fileInfo.name}`);

      // Check if file already exists
      if (app.vault.getAbstractFileByPath(targetPath)) {
        skipped++;
        continue;
      }

      // For files already in vault, move them
      const existingFile = app.vault.getAbstractFileByPath(fileInfo.path);
      if (existingFile instanceof TFile) {
        await app.vault.rename(existingFile, targetPath);

        if (fileInfo.category === "markdown" && options.autoGenerateFrontmatter) {
          const movedFile = app.vault.getAbstractFileByPath(targetPath);
          if (movedFile instanceof TFile) {
            await addFrontmatterIfMissing(app, movedFile);
          }
        }

        imported++;
      } else {
        skipped++;
      }
    } catch {
      skipped++;
    }
  }

  return { imported, skipped };
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
  const lines: string[] = [];

  lines.push(`## Ingest Summary`);
  lines.push(`Total files: ${result.total}`);
  lines.push(``);
  lines.push(`### By Category`);
  lines.push(`- Markdown: ${result.byCategory.markdown}`);
  lines.push(`- Images: ${result.byCategory.image}`);
  lines.push(`- PDFs: ${result.byCategory.pdf}`);
  lines.push(`- Other: ${result.byCategory.other}`);

  if (result.duplicates.length > 0) {
    lines.push(``);
    lines.push(`### Duplicates Found (${result.duplicates.length})`);
    for (const dup of result.duplicates) {
      lines.push(`- ${dup.newFile} -> ${dup.existingFile}`);
    }
  }

  return lines.join("\n");
}
