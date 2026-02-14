use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::HashMap;

use super::{KnowledgeEntry, KnowledgeProvider, LookupOptions, LookupResult};

const GITHUB_API: &str = "https://api.github.com";

pub struct GitHubProvider {
    client: Client,
    token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RepoSearchResponse {
    items: Option<Vec<RepoItem>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct RepoItem {
    full_name: String,
    name: String,
    description: Option<String>,
    html_url: String,
    stargazers_count: u64,
    forks_count: u64,
    language: Option<String>,
    topics: Option<Vec<String>>,
    license: Option<License>,
    updated_at: Option<String>,
    owner: Owner,
}

#[derive(Debug, Deserialize)]
struct Owner {
    login: String,
}

#[derive(Debug, Deserialize)]
struct License {
    spdx_id: Option<String>,
}

impl GitHubProvider {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("vault-tree-mcp/0.1 (https://github.com/Hydepwns/vault-tree)")
                .build()
                .unwrap_or_else(|_| Client::new()),
            token: None,
        }
    }

    pub fn with_token(token: impl Into<String>) -> Self {
        Self {
            client: Client::builder()
                .user_agent("vault-tree-mcp/0.1 (https://github.com/Hydepwns/vault-tree)")
                .build()
                .unwrap_or_else(|_| Client::new()),
            token: Some(token.into()),
        }
    }

    fn add_auth(&self, request: reqwest::blocking::RequestBuilder) -> reqwest::blocking::RequestBuilder {
        match &self.token {
            Some(token) => request.header("Authorization", format!("Bearer {}", token)),
            None => request,
        }
    }

    fn format_count(n: u64) -> String {
        if n >= 1000 {
            format!("{:.1}k", n as f64 / 1000.0)
        } else {
            n.to_string()
        }
    }

    fn repo_to_entry(&self, repo: &RepoItem) -> KnowledgeEntry {
        let stars = Self::format_count(repo.stargazers_count);
        let forks = Self::format_count(repo.forks_count);

        let mut lines = Vec::new();
        if let Some(desc) = &repo.description {
            lines.push(desc.clone());
        }
        lines.push(format!("Stars: {} | Forks: {}", stars, forks));
        if let Some(lang) = &repo.language {
            lines.push(format!("Language: {}", lang));
        }
        if let Some(topics) = &repo.topics {
            if !topics.is_empty() {
                lines.push(format!("Topics: {}", topics.iter().take(5).cloned().collect::<Vec<_>>().join(", ")));
            }
        }

        let mut metadata = HashMap::new();
        metadata.insert("type".to_string(), serde_json::json!("repo"));
        metadata.insert("name".to_string(), serde_json::json!(repo.name));
        metadata.insert("fullName".to_string(), serde_json::json!(repo.full_name));
        metadata.insert("owner".to_string(), serde_json::json!(repo.owner.login));
        if let Some(desc) = &repo.description {
            metadata.insert("description".to_string(), serde_json::json!(desc));
        }
        metadata.insert("stars".to_string(), serde_json::json!(repo.stargazers_count));
        metadata.insert("forks".to_string(), serde_json::json!(repo.forks_count));
        if let Some(lang) = &repo.language {
            metadata.insert("language".to_string(), serde_json::json!(lang));
        }
        if let Some(topics) = &repo.topics {
            metadata.insert("topics".to_string(), serde_json::json!(topics));
        }
        if let Some(lic) = &repo.license {
            if let Some(id) = &lic.spdx_id {
                metadata.insert("license".to_string(), serde_json::json!(id));
            }
        }

        KnowledgeEntry {
            title: repo.full_name.clone(),
            summary: lines.join("\n"),
            url: Some(repo.html_url.clone()),
            source: "github".to_string(),
            metadata: Some(metadata),
        }
    }

    fn search_repos(&self, query: &str, limit: usize) -> Result<Vec<KnowledgeEntry>, String> {
        let url = format!(
            "{}/search/repositories?q={}&sort=stars&order=desc&per_page={}",
            GITHUB_API,
            urlencoding::encode(query),
            limit
        );

        let request = self.client
            .get(&url)
            .header("Accept", "application/vnd.github.v3+json");

        let response = self.add_auth(request)
            .send()
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Err(format!("search failed: {}", response.status()));
        }

        let data: RepoSearchResponse = response.json().map_err(|e| e.to_string())?;

        Ok(data
            .items
            .unwrap_or_default()
            .iter()
            .map(|r| self.repo_to_entry(r))
            .collect())
    }

    fn lookup_repo(&self, full_name: &str) -> Result<Option<KnowledgeEntry>, String> {
        let url = format!("{}/repos/{}", GITHUB_API, full_name);

        let request = self.client
            .get(&url)
            .header("Accept", "application/vnd.github.v3+json");

        let response = self.add_auth(request)
            .send()
            .map_err(|e| e.to_string())?;

        if response.status().as_u16() == 404 {
            return Ok(None);
        }

        if !response.status().is_success() {
            return Err(format!("repo lookup failed: {}", response.status()));
        }

        let repo: RepoItem = response.json().map_err(|e| e.to_string())?;
        Ok(Some(self.repo_to_entry(&repo)))
    }
}

impl Default for GitHubProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeProvider for GitHubProvider {
    fn name(&self) -> &'static str {
        "github"
    }

    fn is_available(&self) -> bool {
        let request = self.client
            .get(format!("{}/rate_limit", GITHUB_API))
            .header("Accept", "application/vnd.github.v3+json");

        self.add_auth(request)
            .send()
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    fn lookup(&self, query: &str, options: &LookupOptions) -> LookupResult {
        let limit = options.max_results.unwrap_or(5);

        // Check if query looks like owner/repo
        if query.contains('/') && !query.contains(' ') {
            match self.lookup_repo(query) {
                Ok(Some(entry)) => return LookupResult::success(self.name(), vec![entry]),
                Ok(None) => {}
                Err(e) => return LookupResult::error(self.name(), e),
            }
        }

        // Search repos
        match self.search_repos(query, limit) {
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
    fn github_lookup() {
        let provider = GitHubProvider::new();
        let result = provider.lookup("rust-lang/rust", &LookupOptions::default());
        assert!(result.success);
        assert!(!result.entries.is_empty());
    }
}
