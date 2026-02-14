use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::HashMap;

use super::{KnowledgeEntry, KnowledgeProvider, LookupOptions, LookupResult};

const REDDIT_API: &str = "https://www.reddit.com";

pub struct RedditProvider {
    client: Client,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    data: SearchData,
}

#[derive(Debug, Deserialize)]
struct SearchData {
    children: Vec<PostWrapper>,
}

#[derive(Debug, Deserialize)]
struct PostWrapper {
    data: Post,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Post {
    id: String,
    title: String,
    subreddit: String,
    author: String,
    score: i32,
    num_comments: u32,
    permalink: String,
    selftext: Option<String>,
    url: Option<String>,
    created_utc: f64,
    is_self: bool,
}

impl RedditProvider {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("vault-tree-mcp/0.1 (https://github.com/Hydepwns/vault-tree)")
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }

    fn format_count(n: i32) -> String {
        let abs = n.abs();
        if abs >= 1_000_000 {
            format!("{:.1}M", n as f64 / 1_000_000.0)
        } else if abs >= 1_000 {
            format!("{:.1}k", n as f64 / 1_000.0)
        } else {
            n.to_string()
        }
    }

    fn search(&self, query: &str, limit: usize) -> Result<Vec<KnowledgeEntry>, String> {
        let url = format!(
            "{}/search.json?q={}&sort=relevance&limit={}&type=link",
            REDDIT_API,
            urlencoding::encode(query),
            limit
        );

        let response = self.client.get(&url).send().map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Err(format!("search failed: {}", response.status()));
        }

        let data: SearchResponse = response.json().map_err(|e| e.to_string())?;

        Ok(data
            .data
            .children
            .into_iter()
            .map(|wrapper| {
                let post = wrapper.data;
                let score = Self::format_count(post.score);
                let comments = Self::format_count(post.num_comments as i32);

                let mut lines = Vec::new();
                lines.push(format!("r/{} | Score: {} | Comments: {}", post.subreddit, score, comments));
                lines.push(format!("Posted by u/{}", post.author));

                // Add preview of selftext if available
                if let Some(text) = &post.selftext {
                    if !text.is_empty() && post.is_self {
                        let preview: String = text.chars().take(200).collect();
                        let truncated = if text.len() > 200 {
                            format!("{}...", preview)
                        } else {
                            preview
                        };
                        lines.push(truncated);
                    }
                }

                let permalink = format!("https://www.reddit.com{}", post.permalink);

                let mut metadata = HashMap::new();
                metadata.insert("id".to_string(), serde_json::json!(post.id));
                metadata.insert("subreddit".to_string(), serde_json::json!(post.subreddit));
                metadata.insert("author".to_string(), serde_json::json!(post.author));
                metadata.insert("score".to_string(), serde_json::json!(post.score));
                metadata.insert("numComments".to_string(), serde_json::json!(post.num_comments));
                metadata.insert("isSelf".to_string(), serde_json::json!(post.is_self));
                if let Some(url) = &post.url {
                    if !post.is_self {
                        metadata.insert("externalUrl".to_string(), serde_json::json!(url));
                    }
                }

                KnowledgeEntry {
                    title: post.title,
                    summary: lines.join("\n"),
                    url: Some(permalink),
                    source: "reddit".to_string(),
                    metadata: Some(metadata),
                }
            })
            .collect())
    }

    fn search_subreddit(&self, subreddit: &str, query: &str, limit: usize) -> Result<Vec<KnowledgeEntry>, String> {
        let url = format!(
            "{}/r/{}/search.json?q={}&restrict_sr=on&sort=relevance&limit={}",
            REDDIT_API,
            subreddit,
            urlencoding::encode(query),
            limit
        );

        let response = self.client.get(&url).send().map_err(|e| e.to_string())?;

        if response.status().as_u16() == 404 {
            return Ok(Vec::new());
        }

        if !response.status().is_success() {
            return Err(format!("search failed: {}", response.status()));
        }

        let data: SearchResponse = response.json().map_err(|e| e.to_string())?;

        Ok(data
            .data
            .children
            .into_iter()
            .map(|wrapper| {
                let post = wrapper.data;
                let score = Self::format_count(post.score);
                let comments = Self::format_count(post.num_comments as i32);

                let mut lines = Vec::new();
                lines.push(format!("Score: {} | Comments: {}", score, comments));
                lines.push(format!("Posted by u/{}", post.author));

                let permalink = format!("https://www.reddit.com{}", post.permalink);

                let mut metadata = HashMap::new();
                metadata.insert("id".to_string(), serde_json::json!(post.id));
                metadata.insert("subreddit".to_string(), serde_json::json!(post.subreddit));
                metadata.insert("author".to_string(), serde_json::json!(post.author));
                metadata.insert("score".to_string(), serde_json::json!(post.score));
                metadata.insert("numComments".to_string(), serde_json::json!(post.num_comments));

                KnowledgeEntry {
                    title: post.title,
                    summary: lines.join("\n"),
                    url: Some(permalink),
                    source: "reddit".to_string(),
                    metadata: Some(metadata),
                }
            })
            .collect())
    }
}

impl Default for RedditProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeProvider for RedditProvider {
    fn name(&self) -> &'static str {
        "reddit"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn lookup(&self, query: &str, options: &LookupOptions) -> LookupResult {
        let limit = options.max_results.unwrap_or(5);

        // Check if query specifies a subreddit (r/subreddit query)
        if let Some(rest) = query.strip_prefix("r/") {
            if let Some((sub, search)) = rest.split_once(' ') {
                return match self.search_subreddit(sub, search, limit) {
                    Ok(entries) => LookupResult::success(self.name(), entries),
                    Err(e) => LookupResult::error(self.name(), e),
                };
            }
        }

        match self.search(query, limit) {
            Ok(entries) => LookupResult::success(self.name(), entries),
            Err(e) => LookupResult::error(self.name(), e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires network
    fn reddit_search() {
        let provider = RedditProvider::new();
        let result = provider.lookup("rust programming", &LookupOptions::default());
        assert!(result.success);
        assert!(!result.entries.is_empty());
    }
}
