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

const fnv1a = (bytes: Uint8Array): number =>
  bytes.reduce((hash, byte) => Math.imul(hash ^ byte, 16777619), 2166136261);

const simpleHash = (bytes: Uint8Array): string =>
  (fnv1a(bytes) >>> 0).toString(16).padStart(8, "0");

export interface HashIndex {
  byHash: Map<string, string[]>;
  byFile: Map<string, string>;
}

type FileHash = { path: string; hash: string };

const safeHashFile = async (app: App, file: TFile): Promise<FileHash | null> => {
  try {
    const hash = await hashFile(app, file);
    return { path: file.path, hash };
  } catch (error) {
    console.warn(`[fingerprint] Failed to hash ${file.path}:`, error);
    return null;
  }
};

export async function buildHashIndex(app: App): Promise<HashIndex> {
  const files = app.vault.getMarkdownFiles();

  const results = await Promise.all(files.map((f) => safeHashFile(app, f)));
  const validResults = results.filter((r): r is FileHash => r !== null);

  return validResults.reduce<HashIndex>(
    (index, { path, hash }) => ({
      byFile: new Map(index.byFile).set(path, hash),
      byHash: new Map(index.byHash).set(hash, [...(index.byHash.get(hash) ?? []), path]),
    }),
    { byFile: new Map(), byHash: new Map() }
  );
}

export function findDuplicates(index: HashIndex): Map<string, string[]> {
  return new Map(
    Array.from(index.byHash.entries()).filter(([, files]) => files.length > 1)
  );
}
