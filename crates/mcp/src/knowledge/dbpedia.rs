use reqwest::blocking::Client;
use serde::Deserialize;

use super::{KnowledgeEntry, KnowledgeProvider, LookupOptions, LookupResult};

const DBPEDIA_LOOKUP: &str = "https://lookup.dbpedia.org/api/search";

pub struct DBpediaProvider {
    client: Client,
}

#[derive(Debug, Deserialize)]
struct LookupResponse {
    docs: Option<Vec<Doc>>,
}

#[derive(Debug, Deserialize)]
struct Doc {
    resource: Option<Vec<String>>,
    label: Option<Vec<String>>,
    comment: Option<Vec<String>>,
    category: Option<Vec<String>>,
    #[serde(rename = "type")]
    types: Option<Vec<String>>,
}

impl DBpediaProvider {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("vault-tree-mcp/0.1 (https://github.com/Hydepwns/vault-tree)")
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }
}

impl Default for DBpediaProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeProvider for DBpediaProvider {
    fn name(&self) -> &'static str {
        "dbpedia"
    }

    fn is_available(&self) -> bool {
        self.client
            .get(&format!("{}?query=test&maxResults=1&format=json", DBPEDIA_LOOKUP))
            .send()
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    fn lookup(&self, query: &str, options: &LookupOptions) -> LookupResult {
        let limit = options.max_results.unwrap_or(5);

        let url = format!(
            "{}?query={}&maxResults={}&format=json",
            DBPEDIA_LOOKUP,
            urlencoding::encode(query),
            limit
        );

        let response = match self.client.get(&url).header("Accept", "application/json").send() {
            Ok(r) => r,
            Err(e) => return LookupResult::error(self.name(), e.to_string()),
        };

        if !response.status().is_success() {
            return LookupResult::error(
                self.name(),
                format!("lookup request failed: {}", response.status()),
            );
        }

        let data: LookupResponse = match response.json() {
            Ok(d) => d,
            Err(e) => return LookupResult::error(self.name(), e.to_string()),
        };

        let entries: Vec<KnowledgeEntry> = data
            .docs
            .unwrap_or_default()
            .into_iter()
            .filter_map(|doc| {
                let label = doc.label.as_ref()?.first()?.clone();
                let resource = doc.resource.as_ref().and_then(|r| r.first().cloned());
                let comment = doc.comment.as_ref().and_then(|c| c.first().cloned());

                let types: Vec<String> = doc
                    .types
                    .unwrap_or_default()
                    .into_iter()
                    .take(3)
                    .filter_map(|t| t.rsplit('/').next().map(String::from))
                    .collect();

                let categories: Vec<String> = doc
                    .category
                    .unwrap_or_default()
                    .into_iter()
                    .take(5)
                    .collect();

                let mut metadata = std::collections::HashMap::new();
                if !types.is_empty() {
                    metadata.insert("types".to_string(), serde_json::json!(types));
                }
                if !categories.is_empty() {
                    metadata.insert("categories".to_string(), serde_json::json!(categories));
                }
                if let Some(ref uri) = resource {
                    metadata.insert("resourceUri".to_string(), serde_json::json!(uri));
                }

                Some(KnowledgeEntry {
                    title: label,
                    summary: comment.unwrap_or_default(),
                    url: resource,
                    source: "dbpedia".to_string(),
                    metadata: if metadata.is_empty() {
                        None
                    } else {
                        Some(metadata)
                    },
                })
            })
            .collect();

        LookupResult::success(self.name(), entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires network
    fn dbpedia_lookup() {
        let provider = DBpediaProvider::new();
        let result = provider.lookup("Rust programming language", &LookupOptions::default());
        assert!(result.success);
        assert!(!result.entries.is_empty());
    }
}
