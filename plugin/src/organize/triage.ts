import { App, TFile, TFolder, normalizePath } from "obsidian";
import type { TriageItem, TriageResult, PlacementSuggestion, OrganizeSettings } from "./types";
import { suggestPlacement, buildFolderStats } from "./suggestions";
import { ensureFolder } from "../utils/vault";

export async function triageInbox(
  app: App,
  settings: OrganizeSettings
): Promise<TriageItem[]> {
  const inboxFolder = app.vault.getAbstractFileByPath(settings.inboxFolder);

  if (!inboxFolder || !(inboxFolder instanceof TFolder)) {
    return [];
  }

  const folderStats = await buildFolderStats(app, settings.excludeFolders);
  const items: TriageItem[] = [];

  for (const child of inboxFolder.children) {
    if (child instanceof TFile && child.extension === "md") {
      const suggestion = await suggestPlacement(
        app,
        child,
        folderStats,
        settings.excludeFolders
      );

      items.push({
        file: child.path,
        currentFolder: settings.inboxFolder,
        suggestion,
        status: "pending",
      });
    }
  }

  // Sort by confidence (highest first)
  items.sort((a, b) => b.suggestion.confidence - a.suggestion.confidence);

  return items;
}

export async function applyTriageDecisions(
  app: App,
  items: TriageItem[]
): Promise<TriageResult> {
  const result: TriageResult = {
    items,
    processed: 0,
    accepted: 0,
    rejected: 0,
    modified: 0,
  };

  for (const item of items) {
    if (item.status === "pending") {
      continue;
    }

    result.processed++;

    if (item.status === "rejected") {
      result.rejected++;
      continue;
    }

    const targetFolder =
      item.status === "modified" && item.modifiedFolder
        ? item.modifiedFolder
        : item.suggestion.suggestedFolder;

    if (!targetFolder) {
      result.rejected++;
      continue;
    }

    try {
      const file = app.vault.getAbstractFileByPath(item.file);
      if (file instanceof TFile) {
        await moveFile(app, file, targetFolder);

        if (item.status === "accepted") {
          result.accepted++;
        } else {
          result.modified++;
        }
      }
    } catch {
      result.rejected++;
    }
  }

  return result;
}

async function moveFile(app: App, file: TFile, targetFolder: string): Promise<void> {
  // Ensure target folder exists
  await ensureFolder(app, targetFolder);

  const newPath = normalizePath(`${targetFolder}/${file.name}`);

  // Check for name collision
  let finalPath = newPath;
  let counter = 1;

  while (app.vault.getAbstractFileByPath(finalPath)) {
    const ext = file.extension;
    const baseName = file.basename;
    finalPath = normalizePath(`${targetFolder}/${baseName}-${counter}.${ext}`);
    counter++;
  }

  await app.vault.rename(file, finalPath);
}

export function formatTriageSuggestions(items: TriageItem[]): string {
  const lines: string[] = [];

  lines.push(`## Triage Suggestions (${items.length} files)`);
  lines.push(``);

  for (const item of items) {
    const confidence = (item.suggestion.confidence * 100).toFixed(0);
    lines.push(`### ${item.file}`);
    lines.push(`Suggested: **${item.suggestion.suggestedFolder || "(none)"}** (${confidence}% confidence)`);

    if (item.suggestion.reasons.length > 0) {
      lines.push(`Reasons:`);
      for (const reason of item.suggestion.reasons) {
        lines.push(`- ${reason}`);
      }
    }

    if (item.suggestion.alternativeFolders.length > 0) {
      lines.push(`Alternatives:`);
      for (const alt of item.suggestion.alternativeFolders) {
        const altConf = (alt.confidence * 100).toFixed(0);
        lines.push(`- ${alt.folder} (${altConf}%)`);
      }
    }

    lines.push(``);
  }

  return lines.join("\n");
}

export function formatTriageResult(result: TriageResult): string {
  const lines: string[] = [];

  lines.push(`## Triage Result`);
  lines.push(`- Processed: ${result.processed}`);
  lines.push(`- Accepted: ${result.accepted}`);
  lines.push(`- Modified: ${result.modified}`);
  lines.push(`- Rejected: ${result.rejected}`);

  return lines.join("\n");
}
