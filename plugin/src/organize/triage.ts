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

  const mdFiles = inboxFolder.children.filter(
    (child): child is TFile => child instanceof TFile && child.extension === "md"
  );

  const items = await Promise.all(
    mdFiles.map(async (file) => ({
      file: file.path,
      currentFolder: settings.inboxFolder,
      suggestion: await suggestPlacement(app, file, folderStats, settings.excludeFolders),
      status: "pending" as const,
    }))
  );

  return [...items].sort((a, b) => b.suggestion.confidence - a.suggestion.confidence);
}

type MoveOutcome = "accepted" | "modified" | "rejected" | "skipped";

const classifyItem = (item: TriageItem): { outcome: MoveOutcome; targetFolder: string | null } => {
  if (item.status === "pending") {
    return { outcome: "skipped", targetFolder: null };
  }
  if (item.status === "rejected") {
    return { outcome: "rejected", targetFolder: null };
  }
  const targetFolder =
    item.status === "modified" && item.modifiedFolder
      ? item.modifiedFolder
      : item.suggestion.suggestedFolder;
  if (!targetFolder) {
    return { outcome: "rejected", targetFolder: null };
  }
  return { outcome: item.status as "accepted" | "modified", targetFolder };
};

const processItem = async (
  app: App,
  item: TriageItem
): Promise<MoveOutcome> => {
  const { outcome, targetFolder } = classifyItem(item);
  if (outcome === "skipped" || outcome === "rejected" || !targetFolder) {
    return outcome;
  }
  const file = app.vault.getAbstractFileByPath(item.file);
  if (!(file instanceof TFile)) {
    return "rejected";
  }
  try {
    await moveFile(app, file, targetFolder);
    return outcome;
  } catch {
    return "rejected";
  }
};

const tallyOutcomes = (outcomes: MoveOutcome[]) =>
  outcomes.reduce(
    (acc, outcome) => ({
      ...acc,
      processed: acc.processed + (outcome === "skipped" ? 0 : 1),
      [outcome]: acc[outcome] + 1,
    }),
    { processed: 0, accepted: 0, modified: 0, rejected: 0, skipped: 0 }
  );

export async function applyTriageDecisions(
  app: App,
  items: TriageItem[]
): Promise<TriageResult> {
  const outcomes = await Promise.all(items.map((item) => processItem(app, item)));
  const { processed, accepted, modified, rejected } = tallyOutcomes(outcomes);
  return { items, processed, accepted, modified, rejected };
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

const formatItemSuggestion = (item: TriageItem): string => {
  const confidence = (item.suggestion.confidence * 100).toFixed(0);
  const reasons = item.suggestion.reasons.length > 0
    ? ["Reasons:", ...item.suggestion.reasons.map((r) => `- ${r}`)]
    : [];
  const alternatives = item.suggestion.alternativeFolders.length > 0
    ? [
        "Alternatives:",
        ...item.suggestion.alternativeFolders.map(
          (alt) => `- ${alt.folder} (${(alt.confidence * 100).toFixed(0)}%)`
        ),
      ]
    : [];

  return [
    `### ${item.file}`,
    `Suggested: **${item.suggestion.suggestedFolder || "(none)"}** (${confidence}% confidence)`,
    ...reasons,
    ...alternatives,
    "",
  ].join("\n");
};

export function formatTriageSuggestions(items: TriageItem[]): string {
  return [
    `## Triage Suggestions (${items.length} files)`,
    "",
    ...items.map(formatItemSuggestion),
  ].join("\n");
}

export function formatTriageResult(result: TriageResult): string {
  return [
    "## Triage Result",
    `- Processed: ${result.processed}`,
    `- Accepted: ${result.accepted}`,
    `- Modified: ${result.modified}`,
    `- Rejected: ${result.rejected}`,
  ].join("\n");
}
