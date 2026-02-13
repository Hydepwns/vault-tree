use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FrontmatterError {
    #[error("no frontmatter delimiters found")]
    NoDelimiters,
    #[error("yaml parse error: {0}")]
    YamlError(#[from] serde_yaml::Error),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Frontmatter {
    pub title: Option<String>,
    pub date: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub slug: Option<String>,
    pub description: Option<String>,
}

pub fn extract_frontmatter(content: &str) -> Result<Frontmatter, FrontmatterError> {
    let content = content.trim_start();

    if !content.starts_with("---") {
        return Err(FrontmatterError::NoDelimiters);
    }

    let after_first = &content[3..];
    let end_pos = after_first
        .find("\n---")
        .or_else(|| after_first.find("\r\n---"))
        .ok_or(FrontmatterError::NoDelimiters)?;

    let yaml_content = &after_first[..end_pos].trim();
    let fm: Frontmatter = serde_yaml::from_str(yaml_content)?;

    Ok(fm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_frontmatter() {
        let content = r#"---
title: Test Post
date: 2025-01-18
tags:
  - rust
  - mcp
slug: test-post
description: A test post
---

# Content here
"#;
        let fm = extract_frontmatter(content).unwrap();
        assert_eq!(fm.title, Some("Test Post".to_string()));
        assert_eq!(fm.date, Some("2025-01-18".to_string()));
        assert_eq!(fm.tags, vec!["rust", "mcp"]);
        assert_eq!(fm.slug, Some("test-post".to_string()));
    }

    #[test]
    fn handles_missing_frontmatter() {
        let content = "# Just a heading\n\nSome content";
        assert!(extract_frontmatter(content).is_err());
    }

    #[test]
    fn handles_partial_frontmatter() {
        let content = r#"---
title: Only Title
---

Content
"#;
        let fm = extract_frontmatter(content).unwrap();
        assert_eq!(fm.title, Some("Only Title".to_string()));
        assert_eq!(fm.date, None);
        assert!(fm.tags.is_empty());
    }

    #[test]
    fn handles_inline_tags() {
        let content = r#"---
title: Inline Tags
tags: [one, two, three]
---
"#;
        let fm = extract_frontmatter(content).unwrap();
        assert_eq!(fm.tags, vec!["one", "two", "three"]);
    }
}
