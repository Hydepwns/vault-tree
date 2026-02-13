use std::cmp::Ordering;
use std::path::Path;
use walkdir::{DirEntry, WalkDir};

/// Compare two tree entries: directories first, then alphabetically by name.
pub fn compare_tree_entries<T, F, G>(a: &T, b: &T, is_dir: F, get_name: G) -> Ordering
where
    F: Fn(&T) -> bool,
    G: Fn(&T) -> &str,
{
    match (is_dir(a), is_dir(b)) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => get_name(a).cmp(get_name(b)),
    }
}

/// Compare two DirEntry items: directories first, then alphabetically.
pub fn compare_dir_entries(a: &std::fs::DirEntry, b: &std::fs::DirEntry) -> Ordering {
    let a_is_dir = a.path().is_dir();
    let b_is_dir = b.path().is_dir();
    match (a_is_dir, b_is_dir) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => a.file_name().cmp(&b.file_name()),
    }
}

/// Format annotation string for a file node showing tags, date, and link counts.
pub fn format_file_annotation(
    tags: &[String],
    date: Option<&str>,
    incoming_links: usize,
    outgoing_links: usize,
) -> String {
    let mut parts = Vec::new();

    if !tags.is_empty() {
        parts.push(format!("[{}]", tags.join(",")));
    }
    if let Some(d) = date {
        parts.push(d.to_string());
    }
    parts.push(format!("<-{} ->{}", incoming_links, outgoing_links));

    format!("  {}", parts.join(" "))
}

/// Generate annotation for a tree node based on whether it's a directory or file.
pub fn node_annotation(
    is_dir: bool,
    note_count: usize,
    has_children: bool,
    tags: &[String],
    date: Option<&str>,
    incoming_links: usize,
    outgoing_links: usize,
) -> String {
    if is_dir {
        if note_count > 0 && !has_children {
            format!(" ({} notes)", note_count)
        } else {
            String::new()
        }
    } else {
        format_file_annotation(tags, date, incoming_links, outgoing_links)
    }
}

/// Count notes in a list of children nodes.
pub fn sum_child_notes<T, F>(children: &[T], is_dir: F, note_count: impl Fn(&T) -> usize) -> usize
where
    F: Fn(&T) -> bool,
{
    children
        .iter()
        .map(|c| if is_dir(c) { note_count(c) } else { 1 })
        .sum()
}

/// Count total notes and directories in a tree.
pub fn count_totals<T: TreeRenderable>(node: &T) -> (usize, usize) {
    let mut notes = 0;
    let mut dirs = 0;

    if node.is_dir() {
        dirs += 1;
        for child in node.children() {
            let (n, d) = count_totals(child);
            notes += n;
            dirs += d;
        }
    } else {
        notes += 1;
    }

    (notes, dirs)
}

/// Trait for tree nodes that can be rendered as ASCII trees.
pub trait TreeRenderable {
    fn name(&self) -> &str;
    fn is_dir(&self) -> bool;
    fn children(&self) -> &[Self]
    where
        Self: Sized;
    fn annotation(&self) -> String;
}

/// Render a tree node and its children as an ASCII tree.
pub fn render_tree_ascii<T: TreeRenderable>(node: &T, prefix: &str, is_last: bool) -> String {
    let mut output = String::new();

    let connector = if prefix.is_empty() {
        ""
    } else if is_last {
        "`-- "
    } else {
        "|-- "
    };

    let display_name = if node.is_dir() {
        format!("{}/", node.name())
    } else {
        node.name().to_string()
    };

    output.push_str(&format!(
        "{}{}{}{}\n",
        prefix,
        connector,
        display_name,
        node.annotation()
    ));

    let child_prefix = if prefix.is_empty() {
        String::new()
    } else if is_last {
        format!("{}    ", prefix)
    } else {
        format!("{}|   ", prefix)
    };

    let children = node.children();
    let child_count = children.len();
    for (i, child) in children.iter().enumerate() {
        output.push_str(&render_tree_ascii(
            child,
            &child_prefix,
            i == child_count - 1,
        ));
    }

    output
}

/// Returns true if the path should be excluded from vault operations.
/// Excludes .obsidian, .git, and node_modules directories.
pub fn is_excluded(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|name| name == ".obsidian" || name == ".git" || name == "node_modules")
        .unwrap_or(false)
}

/// Returns an iterator over markdown files in the given path,
/// excluding .obsidian, .git, and node_modules directories.
pub fn walk_markdown_files(path: &Path) -> impl Iterator<Item = DirEntry> {
    WalkDir::new(path)
        .into_iter()
        .filter_entry(|e| !is_excluded(e.path()))
        .filter_map(|e| e.ok())
        .filter(is_markdown_file)
}

/// Returns true if the entry is a markdown file.
pub fn is_markdown_file(entry: &DirEntry) -> bool {
    entry.file_type().is_file()
        && entry
            .path()
            .extension()
            .map(|ext| ext == "md")
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn excludes_obsidian_dir() {
        let path = Path::new("/vault/.obsidian");
        assert!(is_excluded(path));
    }

    #[test]
    fn excludes_git_dir() {
        let path = Path::new("/vault/.git");
        assert!(is_excluded(path));
    }

    #[test]
    fn excludes_node_modules() {
        let path = Path::new("/vault/node_modules");
        assert!(is_excluded(path));
    }

    #[test]
    fn includes_regular_dirs() {
        let path = Path::new("/vault/notes");
        assert!(!is_excluded(path));
    }

    #[test]
    fn walks_markdown_files() {
        let dir = TempDir::new().unwrap();

        fs::write(dir.path().join("note1.md"), "# Note 1").unwrap();
        fs::write(dir.path().join("note2.md"), "# Note 2").unwrap();
        fs::write(dir.path().join("readme.txt"), "readme").unwrap();

        fs::create_dir(dir.path().join(".obsidian")).unwrap();
        fs::write(dir.path().join(".obsidian/config.json"), "{}").unwrap();

        let files: Vec<_> = walk_markdown_files(dir.path()).collect();

        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|f| f.path().extension().unwrap() == "md"));
    }
}
