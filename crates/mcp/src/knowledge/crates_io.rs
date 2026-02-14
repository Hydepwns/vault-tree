use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::HashMap;

use super::{KnowledgeEntry, KnowledgeProvider, LookupOptions, LookupResult};

const CRATES_API: &str = "https://crates.io/api/v1";

pub struct CratesIoProvider {
    client: Client,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    crates: Option<Vec<Crate>>,
}

#[derive(Debug, Deserialize)]
struct CrateResponse {
    #[serde(rename = "crate")]
    krate: Crate,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Crate {
    name: String,
    description: Option<String>,
    max_version: Option<String>,
    max_stable_version: Option<String>,
    downloads: u64,
    repository: Option<String>,
    documentation: Option<String>,
    homepage: Option<String>,
    keywords: Option<Vec<String>>,
    categories: Option<Vec<String>>,
}

impl CratesIoProvider {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("vault-tree-mcp/0.1 (https://github.com/Hydepwns/vault-tree)")
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }

    fn format_downloads(n: u64) -> String {
        if n >= 1_000_000 {
            format!("{:.1}M", n as f64 / 1_000_000.0)
        } else if n >= 1_000 {
            format!("{:.1}k", n as f64 / 1_000.0)
        } else {
            n.to_string()
        }
    }

    fn crate_to_entry(&self, krate: &Crate) -> KnowledgeEntry {
        let version = krate
            .max_stable_version
            .as_ref()
            .or(krate.max_version.as_ref())
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());

        let downloads = Self::format_downloads(krate.downloads);

        let mut lines = Vec::new();
        if let Some(desc) = &krate.description {
            lines.push(desc.clone());
        }
        lines.push(format!("Version: {} | Downloads: {}", version, downloads));
        if let Some(keywords) = &krate.keywords {
            if !keywords.is_empty() {
                lines.push(format!(
                    "Keywords: {}",
                    keywords.iter().take(5).cloned().collect::<Vec<_>>().join(", ")
                ));
            }
        }

        let url = format!("https://crates.io/crates/{}", krate.name);

        let mut metadata = HashMap::new();
        metadata.insert("name".to_string(), serde_json::json!(krate.name));
        metadata.insert("version".to_string(), serde_json::json!(version));
        metadata.insert("downloads".to_string(), serde_json::json!(krate.downloads));
        if let Some(desc) = &krate.description {
            metadata.insert("description".to_string(), serde_json::json!(desc));
        }
        if let Some(repo) = &krate.repository {
            metadata.insert("repository".to_string(), serde_json::json!(repo));
        }
        if let Some(docs) = &krate.documentation {
            metadata.insert("documentation".to_string(), serde_json::json!(docs));
        }

        KnowledgeEntry {
            title: krate.name.clone(),
            summary: lines.join("\n"),
            url: Some(url),
            source: "crates.io".to_string(),
            metadata: Some(metadata),
        }
    }

    fn search(&self, query: &str, limit: usize) -> Result<Vec<KnowledgeEntry>, String> {
        let url = format!(
            "{}/crates?q={}&per_page={}&sort=downloads",
            CRATES_API,
            urlencoding::encode(query),
            limit
        );

        let response = self.client.get(&url).send().map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Err(format!("search failed: {}", response.status()));
        }

        let data: SearchResponse = response.json().map_err(|e| e.to_string())?;

        Ok(data
            .crates
            .unwrap_or_default()
            .iter()
            .map(|c| self.crate_to_entry(c))
            .collect())
    }

    fn lookup_crate(&self, name: &str) -> Result<Option<KnowledgeEntry>, String> {
        let url = format!("{}/crates/{}", CRATES_API, urlencoding::encode(name));

        let response = self.client.get(&url).send().map_err(|e| e.to_string())?;

        if response.status().as_u16() == 404 {
            return Ok(None);
        }

        if !response.status().is_success() {
            return Err(format!("lookup failed: {}", response.status()));
        }

        let data: CrateResponse = response.json().map_err(|e| e.to_string())?;
        Ok(Some(self.crate_to_entry(&data.krate)))
    }
}

impl Default for CratesIoProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeProvider for CratesIoProvider {
    fn name(&self) -> &'static str {
        "crates.io"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn lookup(&self, query: &str, options: &LookupOptions) -> LookupResult {
        let limit = options.max_results.unwrap_or(5);

        // Check if query looks like a crate name (no spaces, valid chars)
        if !query.contains(' ') && query.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            match self.lookup_crate(query) {
                Ok(Some(entry)) => return LookupResult::success(self.name(), vec![entry]),
                Ok(None) => {}
                Err(e) => return LookupResult::error(self.name(), e),
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
    fn crates_io_search() {
        let provider = CratesIoProvider::new();
        let result = provider.lookup("serde", &LookupOptions::default());
        assert!(result.success);
        assert!(!result.entries.is_empty());
    }
}
