use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;

static WIKILINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[([^\]|#]+)(?:#[^\]|]*)?(?:\|[^\]]+)?\]\]").unwrap());

static MDLINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").unwrap());

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    pub target: String,
    pub link_type: LinkType,
    pub display_text: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkType {
    Wikilink,
    Markdown,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LinkIndex {
    pub outgoing: HashMap<String, Vec<String>>,
    pub incoming: HashMap<String, Vec<String>>,
}

impl LinkIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_link(&mut self, from: &str, to: &str) {
        self.outgoing
            .entry(from.to_string())
            .or_default()
            .push(to.to_string());

        self.incoming
            .entry(to.to_string())
            .or_default()
            .push(from.to_string());
    }

    pub fn outgoing_count(&self, file: &str) -> usize {
        self.outgoing.get(file).map_or(0, Vec::len)
    }

    pub fn incoming_count(&self, file: &str) -> usize {
        self.incoming.get(file).map_or(0, Vec::len)
    }
}

pub fn extract_links(content: &str) -> Vec<Link> {
    let mut links = Vec::new();

    for cap in WIKILINK_RE.captures_iter(content) {
        let target = cap
            .get(1)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        links.push(Link {
            target,
            link_type: LinkType::Wikilink,
            display_text: None,
        });
    }

    for cap in MDLINK_RE.captures_iter(content) {
        let display = cap.get(1).map(|m| m.as_str().to_string());
        let target = cap
            .get(2)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();

        if target.ends_with(".md") || !target.contains('.') {
            links.push(Link {
                target,
                link_type: LinkType::Markdown,
                display_text: display,
            });
        }
    }

    links
}

pub fn normalize_link_target(target: &str) -> String {
    let target = target.trim();
    let target = target.strip_suffix(".md").unwrap_or(target);
    target.to_lowercase().replace(' ', "-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_wikilinks() {
        let content = "Check out [[My Note]] and [[Another Note|alias]] for more.";
        let links = extract_links(content);

        assert_eq!(links.len(), 2);
        assert_eq!(links[0].target, "My Note");
        assert_eq!(links[0].link_type, LinkType::Wikilink);
        assert_eq!(links[1].target, "Another Note");
    }

    #[test]
    fn extracts_wikilinks_with_heading() {
        let content = "See [[Note#Section]] for details.";
        let links = extract_links(content);

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "Note");
    }

    #[test]
    fn extracts_markdown_links_to_md_files() {
        let content = "Read [the docs](./docs/readme.md) and [external](https://example.com).";
        let links = extract_links(content);

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "./docs/readme.md");
        assert_eq!(links[0].link_type, LinkType::Markdown);
    }

    #[test]
    fn link_index_tracks_bidirectional() {
        let mut index = LinkIndex::new();
        index.add_link("note-a", "note-b");
        index.add_link("note-a", "note-c");
        index.add_link("note-b", "note-c");

        assert_eq!(index.outgoing_count("note-a"), 2);
        assert_eq!(index.outgoing_count("note-b"), 1);
        assert_eq!(index.incoming_count("note-c"), 2);
        assert_eq!(index.incoming_count("note-a"), 0);
    }

    #[test]
    fn normalizes_link_targets() {
        assert_eq!(normalize_link_target("My Note.md"), "my-note");
        assert_eq!(normalize_link_target("Another Note"), "another-note");
        assert_eq!(normalize_link_target("  spaced  "), "spaced");
    }
}
