export interface FileEntry {
  path: string;
  name: string;
  is_dir: boolean;
  content: string | null;
}

export interface TreeOptions {
  depth?: number;
  root_name?: string;
}

export interface TreeResult {
  tree: string;
  total_notes: number;
  total_dirs: number;
}

export interface Frontmatter {
  title?: string;
  date?: string;
  tags: string[];
  slug?: string;
  description?: string;
}

export interface Link {
  target: string;
  link_type: "Wikilink" | "Markdown";
  display_text?: string;
}

export interface WasmModule {
  parse_frontmatter(content: string): Frontmatter;
  parse_links(content: string): Link[];
  normalize_link(target: string): string;
  compute_hash(content: Uint8Array): string;
  build_tree(files: FileEntry[], options: TreeOptions): TreeResult;
}
