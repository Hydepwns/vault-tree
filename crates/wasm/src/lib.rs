use serde::{Deserialize, Serialize};
use serde_wasm_bindgen::{from_value, to_value};
use vault_tree_core::{
    compare_tree_entries, count_totals, extract_frontmatter, extract_links, hash_content,
    node_annotation, normalize_link_target, render_tree_ascii, sum_child_notes, Frontmatter,
    LinkIndex, TreeRenderable,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn parse_frontmatter(content: &str) -> Result<JsValue, JsError> {
    let fm = extract_frontmatter(content).map_err(|e| JsError::new(&e.to_string()))?;
    to_value(&fm).map_err(|e| JsError::new(&e.to_string()))
}

#[wasm_bindgen]
pub fn parse_links(content: &str) -> Result<JsValue, JsError> {
    let links = extract_links(content);
    to_value(&links).map_err(|e| JsError::new(&e.to_string()))
}

#[wasm_bindgen]
pub fn normalize_link(target: &str) -> String {
    normalize_link_target(target)
}

#[wasm_bindgen]
pub fn compute_hash(content: &[u8]) -> String {
    hash_content(content)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeNode {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub tags: Vec<String>,
    pub date: Option<String>,
    pub incoming_links: usize,
    pub outgoing_links: usize,
    pub children: Vec<TreeNode>,
    pub note_count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TreeResult {
    pub tree: String,
    pub total_notes: usize,
    pub total_dirs: usize,
}

impl TreeRenderable for TreeNode {
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
        node_annotation(
            self.is_dir,
            self.note_count,
            !self.children.is_empty(),
            &self.tags,
            self.date.as_deref(),
            self.incoming_links,
            self.outgoing_links,
        )
    }
}

#[wasm_bindgen]
pub fn build_tree(files_js: JsValue, options_js: JsValue) -> Result<JsValue, JsError> {
    let files: Vec<FileEntry> =
        from_value(files_js).map_err(|e| JsError::new(&format!("invalid files: {}", e)))?;

    let options: TreeOptions = from_value(options_js).unwrap_or_default();

    let mut link_index = LinkIndex::new();
    let mut file_metadata: std::collections::HashMap<String, (Option<Frontmatter>, usize)> =
        std::collections::HashMap::new();

    for file in &files {
        if file.is_dir {
            continue;
        }
        if let Some(ref content) = file.content {
            let links = extract_links(content);
            let normalized_from = normalize_link_target(&file.name);

            let outgoing_count = links.len();
            for link in &links {
                let normalized_to = normalize_link_target(&link.target);
                link_index.add_link(&normalized_from, &normalized_to);
            }

            let fm = extract_frontmatter(content).ok();
            file_metadata.insert(file.path.clone(), (fm, outgoing_count));
        }
    }

    let root = build_tree_structure(&files, &file_metadata, &link_index, &options);
    let rendered = render_tree_ascii(&root, "", true);

    let (total_notes, total_dirs) = count_totals(&root);

    let result = TreeResult {
        tree: rendered,
        total_notes,
        total_dirs,
    };

    to_value(&result).map_err(|e| JsError::new(&e.to_string()))
}

#[derive(Debug, Default, Deserialize)]
struct TreeOptions {
    #[serde(default)]
    depth: Option<usize>,
    #[serde(default)]
    root_name: Option<String>,
}

fn build_tree_structure(
    files: &[FileEntry],
    metadata: &std::collections::HashMap<String, (Option<Frontmatter>, usize)>,
    link_index: &LinkIndex,
    options: &TreeOptions,
) -> TreeNode {
    let mut root = TreeNode {
        path: String::new(),
        name: options
            .root_name
            .clone()
            .unwrap_or_else(|| "vault".to_string()),
        is_dir: true,
        tags: vec![],
        date: None,
        incoming_links: 0,
        outgoing_links: 0,
        children: vec![],
        note_count: 0,
    };

    let mut dir_map: std::collections::HashMap<String, Vec<TreeNode>> =
        std::collections::HashMap::new();

    for file in files {
        let parts: Vec<&str> = file.path.split('/').collect();
        let depth = parts.len();

        if let Some(max_depth) = options.depth {
            if depth > max_depth {
                continue;
            }
        }

        let normalized_name = normalize_link_target(&file.name);
        let (fm, outgoing) = metadata.get(&file.path).cloned().unwrap_or((None, 0));

        let node = TreeNode {
            path: file.path.clone(),
            name: file.name.clone(),
            is_dir: file.is_dir,
            tags: fm.as_ref().map(|f| f.tags.clone()).unwrap_or_default(),
            date: fm.as_ref().and_then(|f| f.date.clone()),
            incoming_links: link_index.incoming_count(&normalized_name),
            outgoing_links: outgoing,
            children: vec![],
            note_count: 0,
        };

        let parent_path = if parts.len() > 1 {
            parts[..parts.len() - 1].join("/")
        } else {
            String::new()
        };

        dir_map.entry(parent_path).or_default().push(node);
    }

    fn collect_children(
        path: &str,
        dir_map: &std::collections::HashMap<String, Vec<TreeNode>>,
    ) -> Vec<TreeNode> {
        let mut children = dir_map.get(path).cloned().unwrap_or_default();

        for child in &mut children {
            if child.is_dir {
                child.children = collect_children(&child.path, dir_map);
                child.note_count = count_notes(&child.children);
            }
        }

        children.sort_by(|a, b| compare_tree_entries(a, b, |n| n.is_dir, |n| &n.name));

        children
    }

    root.children = collect_children("", &dir_map);
    root.note_count = count_notes(&root.children);

    root
}

fn count_notes(children: &[TreeNode]) -> usize {
    sum_child_notes(children, |c| c.is_dir, |c| c.note_count)
}
