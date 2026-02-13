pub mod fingerprint;
pub mod frontmatter;
pub mod links;
pub mod search;
#[cfg(test)]
mod testutils;
pub mod tree;
pub mod utils;

pub use fingerprint::{hash_content, hash_file};
pub use frontmatter::{extract_frontmatter, Frontmatter};
pub use links::{extract_links, normalize_link_target, Link, LinkIndex, LinkType};
pub use search::{search_vault, SearchMatch, SearchOptions, SearchResult};
pub use tree::{generate_tree, render_tree, TreeOptions, VaultNode, VaultTree};
pub use utils::{
    compare_dir_entries, compare_tree_entries, count_totals, format_file_annotation, is_excluded,
    is_markdown_file, node_annotation, render_tree_ascii, sum_child_notes, walk_markdown_files,
    TreeRenderable,
};
