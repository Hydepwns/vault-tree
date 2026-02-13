export interface FileInfo {
  path: string;
  name: string;
  extension: string;
  size: number;
  category: FileCategory;
  hash?: string;
}

export type FileCategory = "markdown" | "image" | "pdf" | "other";

export interface IngestResult {
  total: number;
  byCategory: Record<FileCategory, number>;
  files: FileInfo[];
  duplicates: DuplicateInfo[];
}

export interface DuplicateInfo {
  newFile: string;
  existingFile: string;
  hash: string;
}

export interface PlacementSuggestion {
  file: string;
  suggestedFolder: string;
  confidence: number;
  reasons: string[];
  alternativeFolders: Array<{
    folder: string;
    confidence: number;
  }>;
}

export interface TriageItem {
  file: string;
  currentFolder: string;
  suggestion: PlacementSuggestion;
  status: "pending" | "accepted" | "rejected" | "modified";
  modifiedFolder?: string;
}

export interface TriageResult {
  items: TriageItem[];
  processed: number;
  accepted: number;
  rejected: number;
  modified: number;
}

export interface FolderStats {
  path: string;
  noteCount: number;
  tags: Record<string, number>;
  keywords: Record<string, number>;
}

export interface OrganizeSettings {
  inboxFolder: string;
  excludeFolders: string[];
  minConfidence: number;
  autoGenerateFrontmatter: boolean;
}

export const DEFAULT_ORGANIZE_SETTINGS: OrganizeSettings = {
  inboxFolder: "999 Review",
  excludeFolders: [".obsidian", ".git", "node_modules", "templates"],
  minConfidence: 0.5,
  autoGenerateFrontmatter: true,
};
