import { App, TFile } from "obsidian";
import { getWasm } from "../wasm/loader";

export async function hashFile(app: App, file: TFile): Promise<string> {
  const wasm = getWasm();

  const content = await app.vault.readBinary(file);
  const bytes = new Uint8Array(content);

  if (wasm) {
    return wasm.compute_hash(bytes);
  }

  // Fallback: simple hash if WASM not available
  return simpleHash(bytes);
}

export async function hashContent(content: string): Promise<string> {
  const wasm = getWasm();
  const bytes = new TextEncoder().encode(content);

  if (wasm) {
    return wasm.compute_hash(bytes);
  }

  return simpleHash(bytes);
}

function simpleHash(bytes: Uint8Array): string {
  // Simple FNV-1a hash as fallback
  let hash = 2166136261;
  for (let i = 0; i < bytes.length; i++) {
    hash ^= bytes[i];
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0).toString(16).padStart(8, "0");
}

export interface HashIndex {
  byHash: Map<string, string[]>;
  byFile: Map<string, string>;
}

export async function buildHashIndex(app: App): Promise<HashIndex> {
  const index: HashIndex = {
    byHash: new Map(),
    byFile: new Map(),
  };

  const files = app.vault.getMarkdownFiles();

  for (const file of files) {
    try {
      const hash = await hashFile(app, file);
      index.byFile.set(file.path, hash);

      const existing = index.byHash.get(hash) || [];
      existing.push(file.path);
      index.byHash.set(hash, existing);
    } catch {
      // Skip files that can't be hashed
    }
  }

  return index;
}

export function findDuplicates(index: HashIndex): Map<string, string[]> {
  const duplicates = new Map<string, string[]>();

  for (const [hash, files] of index.byHash) {
    if (files.length > 1) {
      duplicates.set(hash, files);
    }
  }

  return duplicates;
}
