use crate::utils::walk_markdown_files;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SearchError {
    #[error("invalid regex pattern: {0}")]
    InvalidPattern(#[from] regex::Error),
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchMatch {
    pub line_number: usize,
    pub line_content: String,
    pub match_start: usize,
    pub match_end: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub file_path: String,
    pub matches: Vec<SearchMatch>,
}

#[derive(Debug, Clone, Default)]
pub struct SearchOptions {
    pub file_pattern: Option<String>,
    pub case_insensitive: bool,
    pub max_results: Option<usize>,
}

pub fn search_vault(
    vault_path: &Path,
    pattern: &str,
    options: &SearchOptions,
) -> Result<Vec<SearchResult>, SearchError> {
    let regex = if options.case_insensitive {
        Regex::new(&format!("(?i){}", pattern))?
    } else {
        Regex::new(pattern)?
    };

    let file_regex = options
        .file_pattern
        .as_ref()
        .map(|p| Regex::new(p))
        .transpose()?;

    let entries = walk_markdown_files(vault_path).filter(|entry| {
        file_regex.as_ref().is_none_or(|re| {
            entry
                .path()
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|name| re.is_match(name))
        })
    });

    let mut results = Vec::new();
    let mut total_matches = 0;

    for entry in entries {
        let path = entry.path();
        let Ok(content) = fs::read_to_string(path) else {
            continue;
        };

        let file_matches: Vec<SearchMatch> = content
            .lines()
            .enumerate()
            .filter_map(|(line_num, line)| {
                regex.find(line).map(|mat| SearchMatch {
                    line_number: line_num + 1,
                    line_content: line.to_string(),
                    match_start: mat.start(),
                    match_end: mat.end(),
                })
            })
            .take_while(|_| {
                options
                    .max_results
                    .is_none_or(|max| total_matches < max)
                    .then(|| total_matches += 1)
                    .is_some()
            })
            .collect();

        if !file_matches.is_empty() {
            results.push(SearchResult {
                file_path: path.to_string_lossy().to_string(),
                matches: file_matches,
            });

            if options.max_results.is_some_and(|max| total_matches >= max) {
                break;
            }
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutils::create_test_vault;

    #[test]
    fn finds_matches_in_vault() {
        let vault = create_test_vault();
        let results = search_vault(vault.path(), "Hello", &SearchOptions::default()).unwrap();

        assert_eq!(results.len(), 2);
    }

    #[test]
    fn case_insensitive_search() {
        let vault = create_test_vault();
        let results = search_vault(
            vault.path(),
            "hello",
            &SearchOptions {
                case_insensitive: true,
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(results.len(), 2);
    }

    #[test]
    fn excludes_obsidian_dir() {
        let vault = create_test_vault();
        let results = search_vault(vault.path(), "config", &SearchOptions::default()).unwrap();

        assert!(results.is_empty());
    }
}
