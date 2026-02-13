import { App, TFolder, normalizePath } from "obsidian";

/**
 * Ensure a folder exists, creating it and any parent folders if necessary.
 */
export async function ensureFolder(app: App, folderPath: string): Promise<void> {
  const normalized = normalizePath(folderPath);
  const existing = app.vault.getAbstractFileByPath(normalized);

  if (existing instanceof TFolder) {
    return;
  }

  if (existing) {
    throw new Error(`Path exists but is not a folder: ${normalized}`);
  }

  const parts = normalized.split("/");
  let current = "";

  for (const part of parts) {
    current = current ? `${current}/${part}` : part;
    const folder = app.vault.getAbstractFileByPath(current);

    if (!folder) {
      await app.vault.createFolder(current);
    } else if (!(folder instanceof TFolder)) {
      throw new Error(`Path exists but is not a folder: ${current}`);
    }
  }
}

/**
 * Check if a file or folder name should be excluded from vault operations.
 * Excludes .obsidian, .git, and node_modules.
 */
export function isExcluded(name: string): boolean {
  return name === ".obsidian" || name === ".git" || name === "node_modules";
}
