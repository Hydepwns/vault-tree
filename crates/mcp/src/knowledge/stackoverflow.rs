use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::HashMap;

use super::{KnowledgeEntry, KnowledgeProvider, LookupOptions, LookupResult};

const STACKEXCHANGE_API: &str = "https://api.stackexchange.com/2.3";

pub struct StackOverflowProvider {
    client: Client,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    items: Option<Vec<Question>>,
}

#[derive(Debug, Deserialize)]
struct Question {
    question_id: u64,
    title: String,
    link: String,
    score: i32,
    answer_count: u32,
    is_answered: bool,
    view_count: u32,
    tags: Option<Vec<String>>,
    owner: Option<Owner>,
}

#[derive(Debug, Deserialize)]
struct Owner {
    display_name: Option<String>,
    reputation: Option<u32>,
}

impl StackOverflowProvider {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("vault-tree-mcp/0.1 (https://github.com/Hydepwns/vault-tree)")
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }

    fn format_count(n: u32) -> String {
        if n >= 1_000_000 {
            format!("{:.1}M", n as f64 / 1_000_000.0)
        } else if n >= 1_000 {
            format!("{:.1}k", n as f64 / 1_000.0)
        } else {
            n.to_string()
        }
    }

    fn search(&self, query: &str, limit: usize) -> Result<Vec<KnowledgeEntry>, String> {
        let url = format!(
            "{}/search/advanced?order=desc&sort=relevance&site=stackoverflow&q={}&pagesize={}&filter=withbody",
            STACKEXCHANGE_API,
            urlencoding::encode(query),
            limit
        );

        let response = self.client.get(&url).send().map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Err(format!("search failed: {}", response.status()));
        }

        let data: SearchResponse = response.json().map_err(|e| e.to_string())?;

        Ok(data
            .items
            .unwrap_or_default()
            .into_iter()
            .map(|q| {
                let views = Self::format_count(q.view_count);
                let answered_icon = if q.is_answered { "[+]" } else { "[-]" };

                let mut lines = Vec::new();
                lines.push(format!(
                    "Score: {} | Answers: {} {} | Views: {}",
                    q.score, q.answer_count, answered_icon, views
                ));
                if let Some(tags) = &q.tags {
                    if !tags.is_empty() {
                        lines.push(format!(
                            "Tags: {}",
                            tags.iter().take(5).cloned().collect::<Vec<_>>().join(", ")
                        ));
                    }
                }
                if let Some(owner) = &q.owner {
                    if let Some(name) = &owner.display_name {
                        let rep = owner
                            .reputation
                            .map(|r| format!(" ({})", Self::format_count(r)))
                            .unwrap_or_default();
                        lines.push(format!("Asked by: {}{}", name, rep));
                    }
                }

                let mut metadata = HashMap::new();
                metadata.insert("questionId".to_string(), serde_json::json!(q.question_id));
                metadata.insert("score".to_string(), serde_json::json!(q.score));
                metadata.insert("answerCount".to_string(), serde_json::json!(q.answer_count));
                metadata.insert("isAnswered".to_string(), serde_json::json!(q.is_answered));
                metadata.insert("viewCount".to_string(), serde_json::json!(q.view_count));
                if let Some(tags) = &q.tags {
                    metadata.insert("tags".to_string(), serde_json::json!(tags));
                }

                KnowledgeEntry {
                    title: q.title,
                    summary: lines.join("\n"),
                    url: Some(q.link),
                    source: "stackoverflow".to_string(),
                    metadata: Some(metadata),
                }
            })
            .collect())
    }
}

impl Default for StackOverflowProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeProvider for StackOverflowProvider {
    fn name(&self) -> &'static str {
        "stackoverflow"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn lookup(&self, query: &str, options: &LookupOptions) -> LookupResult {
        let limit = options.max_results.unwrap_or(5);

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
    fn stackoverflow_search() {
        let provider = StackOverflowProvider::new();
        let result = provider.lookup("rust ownership", &LookupOptions::default());
        assert!(result.success);
        assert!(!result.entries.is_empty());
    }
}
