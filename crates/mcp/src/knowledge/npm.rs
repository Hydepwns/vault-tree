use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::HashMap;

use super::{KnowledgeEntry, KnowledgeProvider, LookupOptions, LookupResult};

const NPM_REGISTRY: &str = "https://registry.npmjs.org";

pub struct NpmProvider {
    client: Client,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    objects: Option<Vec<SearchObject>>,
}

#[derive(Debug, Deserialize)]
struct SearchObject {
    package: Package,
}

#[derive(Debug, Deserialize)]
struct Package {
    name: String,
    version: String,
    description: Option<String>,
    keywords: Option<Vec<String>>,
    author: Option<Author>,
    links: Links,
}

#[derive(Debug, Deserialize)]
struct Author {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Links {
    npm: Option<String>,
    homepage: Option<String>,
    repository: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct PackageInfo {
    name: String,
    description: Option<String>,
    #[serde(rename = "dist-tags")]
    dist_tags: Option<DistTags>,
    keywords: Option<Vec<String>>,
    author: Option<Author>,
    license: Option<String>,
    homepage: Option<String>,
    repository: Option<Repository>,
}

#[derive(Debug, Deserialize)]
struct DistTags {
    latest: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Repository {
    url: Option<String>,
}

impl NpmProvider {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("vault-tree-mcp/0.1 (https://github.com/Hydepwns/vault-tree)")
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }

    fn search(&self, query: &str, limit: usize) -> Result<Vec<KnowledgeEntry>, String> {
        let url = format!(
            "{}/-/v1/search?text={}&size={}",
            NPM_REGISTRY,
            urlencoding::encode(query),
            limit
        );

        let response = self.client.get(&url).send().map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Err(format!("search failed: {}", response.status()));
        }

        let data: SearchResponse = response.json().map_err(|e| e.to_string())?;

        Ok(data
            .objects
            .unwrap_or_default()
            .into_iter()
            .map(|obj| {
                let pkg = obj.package;
                let mut lines = Vec::new();

                if let Some(desc) = &pkg.description {
                    lines.push(desc.clone());
                }
                lines.push(format!("Version: {}", pkg.version));
                if let Some(author) = &pkg.author {
                    if let Some(name) = &author.name {
                        lines.push(format!("Author: {}", name));
                    }
                }
                if let Some(keywords) = &pkg.keywords {
                    if !keywords.is_empty() {
                        lines.push(format!(
                            "Keywords: {}",
                            keywords.iter().take(5).cloned().collect::<Vec<_>>().join(", ")
                        ));
                    }
                }

                let url = pkg.links.npm.unwrap_or_else(|| {
                    format!("https://www.npmjs.com/package/{}", pkg.name)
                });

                let mut metadata = HashMap::new();
                metadata.insert("name".to_string(), serde_json::json!(pkg.name));
                metadata.insert("version".to_string(), serde_json::json!(pkg.version));
                if let Some(desc) = &pkg.description {
                    metadata.insert("description".to_string(), serde_json::json!(desc));
                }
                if let Some(keywords) = &pkg.keywords {
                    metadata.insert("keywords".to_string(), serde_json::json!(keywords));
                }

                KnowledgeEntry {
                    title: pkg.name,
                    summary: lines.join("\n"),
                    url: Some(url),
                    source: "npm".to_string(),
                    metadata: Some(metadata),
                }
            })
            .collect())
    }

    fn lookup_package(&self, name: &str) -> Result<Option<KnowledgeEntry>, String> {
        let url = format!("{}/{}", NPM_REGISTRY, urlencoding::encode(name));

        let response = self.client.get(&url).send().map_err(|e| e.to_string())?;

        if response.status().as_u16() == 404 {
            return Ok(None);
        }

        if !response.status().is_success() {
            return Err(format!("lookup failed: {}", response.status()));
        }

        let pkg: PackageInfo = response.json().map_err(|e| e.to_string())?;

        let version = pkg
            .dist_tags
            .as_ref()
            .and_then(|dt| dt.latest.clone())
            .unwrap_or_else(|| "unknown".to_string());

        let mut lines = Vec::new();
        if let Some(desc) = &pkg.description {
            lines.push(desc.clone());
        }
        lines.push(format!("Latest: {}", version));
        if let Some(license) = &pkg.license {
            lines.push(format!("License: {}", license));
        }
        if let Some(keywords) = &pkg.keywords {
            if !keywords.is_empty() {
                lines.push(format!(
                    "Keywords: {}",
                    keywords.iter().take(5).cloned().collect::<Vec<_>>().join(", ")
                ));
            }
        }

        let mut metadata = HashMap::new();
        metadata.insert("name".to_string(), serde_json::json!(pkg.name));
        metadata.insert("version".to_string(), serde_json::json!(version));
        if let Some(desc) = &pkg.description {
            metadata.insert("description".to_string(), serde_json::json!(desc));
        }
        if let Some(license) = &pkg.license {
            metadata.insert("license".to_string(), serde_json::json!(license));
        }

        Ok(Some(KnowledgeEntry {
            title: pkg.name,
            summary: lines.join("\n"),
            url: Some(format!("https://www.npmjs.com/package/{}", name)),
            source: "npm".to_string(),
            metadata: Some(metadata),
        }))
    }
}

impl Default for NpmProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeProvider for NpmProvider {
    fn name(&self) -> &'static str {
        "npm"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn lookup(&self, query: &str, options: &LookupOptions) -> LookupResult {
        let limit = options.max_results.unwrap_or(5);

        // Check if query looks like a package name (no spaces)
        if !query.contains(' ') {
            match self.lookup_package(query) {
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
    fn npm_search() {
        let provider = NpmProvider::new();
        let result = provider.lookup("react", &LookupOptions::default());
        assert!(result.success);
        assert!(!result.entries.is_empty());
    }
}
