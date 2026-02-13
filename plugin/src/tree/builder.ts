import { App, TFile, TFolder, TAbstractFile } from "obsidian";
import { getWasm } from "../wasm/loader";
import type { FileEntry, TreeOptions, TreeResult } from "../wasm/types";
import { isExcluded } from "../utils/vault";

export interface BuildTreeOptions {
  depth?: number;
  includeContent?: boolean;
}

export async function buildVaultTree(
  app: App,
  options: BuildTreeOptions = {}
): Promise<TreeResult> {
  const wasm = getWasm();
  if (!wasm) {
    throw new Error("WASM module not initialized");
  }

  const files = await collectFiles(app, options.includeContent ?? true);
  const vaultName = app.vault.getName();

  const treeOptions: TreeOptions = {
    depth: options.depth,
    root_name: vaultName,
  };

  return wasm.build_tree(files, treeOptions);
}

async function collectFiles(
  app: App,
  includeContent: boolean
): Promise<FileEntry[]> {
  const entries: FileEntry[] = [];
  const root = app.vault.getRoot();

  await collectFilesRecursive(app, root, "", entries, includeContent);

  return entries;
}

async function collectFilesRecursive(
  app: App,
  folder: TFolder,
  basePath: string,
  entries: FileEntry[],
  includeContent: boolean
): Promise<void> {
  for (const child of folder.children) {
    if (isExcluded(child.name)) {
      continue;
    }

    const childPath = basePath ? `${basePath}/${child.name}` : child.name;

    if (child instanceof TFolder) {
      entries.push({
        path: childPath,
        name: child.name,
        is_dir: true,
        content: null,
      });

      await collectFilesRecursive(app, child, childPath, entries, includeContent);
    } else if (child instanceof TFile && child.extension === "md") {
      let content: string | null = null;

      if (includeContent) {
        try {
          content = await app.vault.cachedRead(child);
        } catch {
          content = null;
        }
      }

      entries.push({
        path: childPath,
        name: child.name,
        is_dir: false,
        content,
      });
    }
  }
}

export function buildTreeWithObsidianLinks(
  app: App,
  options: BuildTreeOptions = {}
): Promise<TreeResult> {
  // For future enhancement: use app.metadataCache.resolvedLinks
  // to get more accurate link counts from Obsidian's cache
  return buildVaultTree(app, options);
}
