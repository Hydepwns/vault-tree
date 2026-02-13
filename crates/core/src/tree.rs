use crate::frontmatter::{extract_frontmatter, Frontmatter};
use crate::links::{extract_links, normalize_link_target, LinkIndex};
use crate::utils::{
    compare_dir_entries, count_totals, is_excluded, node_annotation, render_tree_ascii,
    sum_child_notes, walk_markdown_files, TreeRenderable,
};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TreeError {
    #[error("vault path does not exist: {0}")]
    VaultNotFound(String),
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub frontmatter: Option<Frontmatter>,
    pub outgoing_links: usize,
    pub incoming_links: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultNode {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub metadata: Option<FileMetadata>,
    #[serde(default)]
    pub children: Vec<VaultNode>,
    #[serde(default)]
    pub note_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultTree {
    pub root: VaultNode,
    pub total_notes: usize,
    pub total_dirs: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TreeOptions {
    pub depth: Option<usize>,
}

impl TreeRenderable for VaultNode {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_dir(&self) -> bool {
        self.is_dir
    }

    fn children(&self) -> &[Self] {
        &self.children
    }

    fn annotation(&self) -> String {
        let (tags, date, incoming, outgoing) = self
            .metadata
            .as_ref()
            .map(|meta| {
                let (tags, date) = meta
                    .frontmatter
                    .as_ref()
                    .map(|fm| (fm.tags.as_slice(), fm.date.as_deref()))
                    .unwrap_or((&[], None));
                (tags, date, meta.incoming_links, meta.outgoing_links)
            })
            .unwrap_or((&[], None, 0, 0));

        node_annotation(
            self.is_dir,
            self.note_count,
            !self.children.is_empty(),
            tags,
            date,
            incoming,
            outgoing,
        )
    }
}

pub fn generate_tree(vault_path: &Path, options: &TreeOptions) -> Result<VaultTree, TreeError> {
    if !vault_path.exists() {
        return Err(TreeError::VaultNotFound(vault_path.display().to_string()));
    }

    let md_files = collect_markdown_files(vault_path);
    let link_index = build_link_index(vault_path, &md_files);
    let metadata_map = build_metadata_map(&md_files, &link_index);

    let root = build_tree_node(vault_path, vault_path, options, 0, &metadata_map)?;

    let (total_notes, total_dirs) = count_totals(&root);

    Ok(VaultTree {
        root,
        total_notes,
        total_dirs,
    })
}

fn collect_markdown_files(vault_path: &Path) -> Vec<PathBuf> {
    walk_markdown_files(vault_path)
        .map(|e| e.path().to_path_buf())
        .collect()
}

fn build_link_index(vault_path: &Path, files: &[PathBuf]) -> LinkIndex {
    let file_links: Vec<(String, Vec<String>)> = files
        .par_iter()
        .filter_map(|path| {
            let content = fs::read_to_string(path).ok()?;
            let links = extract_links(&content);
            let from = path
                .strip_prefix(vault_path)
                .ok()?
                .to_string_lossy()
                .to_string();
            let targets: Vec<String> = links
                .iter()
                .map(|l| normalize_link_target(&l.target))
                .collect();
            Some((from, targets))
        })
        .collect();

    let mut index = LinkIndex::new();
    for (from, targets) in file_links {
        let from_normalized = normalize_link_target(&from);
        for target in targets {
            index.add_link(&from_normalized, &target);
        }
    }
    index
}

fn build_metadata_map(files: &[PathBuf], link_index: &LinkIndex) -> HashMap<PathBuf, FileMetadata> {
    files
        .par_iter()
        .filter_map(|path| {
            let content = fs::read_to_string(path).ok()?;
            let frontmatter = extract_frontmatter(&content).ok();
            let normalized = normalize_link_target(path.file_stem()?.to_str()?);

            let metadata = FileMetadata {
                frontmatter,
                outgoing_links: link_index.outgoing_count(&normalized),
                incoming_links: link_index.incoming_count(&normalized),
            };

            Some((path.clone(), metadata))
        })
        .collect()
}

fn build_tree_node(
    vault_path: &Path,
    current_path: &Path,
    options: &TreeOptions,
    depth: usize,
    metadata_map: &HashMap<PathBuf, FileMetadata>,
) -> Result<VaultNode, TreeError> {
    let name = current_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| current_path.to_string_lossy().to_string());

    let relative_path = current_path
        .strip_prefix(vault_path)
        .unwrap_or(current_path)
        .to_string_lossy()
        .to_string();

    if current_path.is_file() {
        let metadata = metadata_map.get(current_path).cloned();
        return Ok(VaultNode {
            path: relative_path,
            name,
            is_dir: false,
            metadata,
            children: vec![],
            note_count: 0,
        });
    }

    if let Some(max_depth) = options.depth {
        if depth >= max_depth {
            let note_count = count_notes_recursive(current_path);
            return Ok(VaultNode {
                path: relative_path,
                name,
                is_dir: true,
                metadata: None,
                children: vec![],
                note_count,
            });
        }
    }

    let mut entries: Vec<_> = fs::read_dir(current_path)?
        .filter_map(|e| e.ok())
        .filter(|e| !is_excluded(&e.path()))
        .filter(|e| e.path().is_dir() || e.path().extension().is_some_and(|ext| ext == "md"))
        .collect();

    entries.sort_by(compare_dir_entries);

    let children: Vec<VaultNode> = entries
        .into_iter()
        .filter_map(|entry| {
            build_tree_node(vault_path, &entry.path(), options, depth + 1, metadata_map).ok()
        })
        .collect();

    let note_count = sum_child_notes(&children, |c| c.is_dir, |c| c.note_count);

    Ok(VaultNode {
        path: relative_path,
        name,
        is_dir: true,
        metadata: None,
        children,
        note_count,
    })
}

fn count_notes_recursive(path: &Path) -> usize {
    walk_markdown_files(path).count()
}

pub fn render_tree(tree: &VaultTree) -> String {
    let mut output = render_tree_ascii(&tree.root, "", true);
    output.push_str(&format!(
        "\n{} notes, {} directories\n",
        tree.total_notes, tree.total_dirs
    ));
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutils::create_test_vault;

    #[test]
    fn generates_tree() {
        let vault = create_test_vault();
        let tree = generate_tree(vault.path(), &TreeOptions::default()).unwrap();

        assert_eq!(tree.total_notes, 3);
        assert_eq!(tree.total_dirs, 2);
    }

    #[test]
    fn respects_depth_limit() {
        let vault = create_test_vault();
        let tree = generate_tree(vault.path(), &TreeOptions { depth: Some(1) }).unwrap();

        let subdir = tree
            .root
            .children
            .iter()
            .find(|c| c.name == "subdir")
            .unwrap();

        assert!(subdir.children.is_empty());
        assert_eq!(subdir.note_count, 1);
    }

    #[test]
    fn excludes_obsidian_dir() {
        let vault = create_test_vault();
        let tree = generate_tree(vault.path(), &TreeOptions::default()).unwrap();

        let has_obsidian = tree.root.children.iter().any(|c| c.name == ".obsidian");

        assert!(!has_obsidian);
    }

    #[test]
    fn renders_tree_output() {
        let vault = create_test_vault();
        let tree = generate_tree(vault.path(), &TreeOptions::default()).unwrap();
        let output = render_tree(&tree);

        assert!(output.contains("note1.md"));
        assert!(output.contains("subdir/"));
        assert!(output.contains("3 notes"));
    }
}
