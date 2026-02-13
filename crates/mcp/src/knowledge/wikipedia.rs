use reqwest::blocking::Client;
use serde::Deserialize;

use super::{KnowledgeEntry, KnowledgeProvider, LookupOptions, LookupResult};

pub struct WikipediaProvider {
    client: Client,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    query: Option<SearchQuery>,
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    search: Vec<SearchResult>,
}

#[derive(Debug, Deserialize)]
struct SearchResult {
    title: String,
}

#[derive(Debug, Deserialize)]
struct SummaryResponse {
    title: String,
    extract: Option<String>,
    content_urls: Option<ContentUrls>,
}

#[derive(Debug, Deserialize)]
struct ContentUrls {
    desktop: Option<DesktopUrl>,
}

#[derive(Debug, Deserialize)]
struct DesktopUrl {
    page: Option<String>,
}

impl WikipediaProvider {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("vault-tree-mcp/0.1 (https://github.com/Hydepwns/vault-tree)")
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }

    fn search(&self, query: &str, lang: &str, limit: usize) -> Result<Vec<String>, String> {
        let url = format!(
            "https://{}.wikipedia.org/w/api.php?action=query&list=search&srsearch={}&srlimit={}&format=json",
            lang,
            urlencoding::encode(query),
            limit
        );

        let response: SearchResponse = self
            .client
            .get(&url)
            .send()
            .map_err(|e| e.to_string())?
            .json()
            .map_err(|e| e.to_string())?;

        Ok(response
            .query
            .map(|q| q.search.into_iter().map(|r| r.title).collect())
            .unwrap_or_default())
    }

    fn get_summary(&self, title: &str, lang: &str) -> Result<Option<KnowledgeEntry>, String> {
        let url = format!(
            "https://{}.wikipedia.org/api/rest_v1/page/summary/{}",
            lang,
            urlencoding::encode(title)
        );

        let response = self.client.get(&url).send().map_err(|e| e.to_string())?;

        if response.status() == 404 {
            return Ok(None);
        }

        let summary: SummaryResponse = response.json().map_err(|e| e.to_string())?;

        let url = summary
            .content_urls
            .and_then(|c| c.desktop)
            .and_then(|d| d.page);

        Ok(Some(KnowledgeEntry {
            title: summary.title,
            summary: summary.extract.unwrap_or_default(),
            url,
            source: "wikipedia".to_string(),
            metadata: None,
        }))
    }
}

impl Default for WikipediaProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeProvider for WikipediaProvider {
    fn name(&self) -> &'static str {
        "wikipedia"
    }

    fn is_available(&self) -> bool {
        self.client
            .get("https://en.wikipedia.org/api/rest_v1/")
            .send()
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    fn lookup(&self, query: &str, options: &LookupOptions) -> LookupResult {
        let lang = options.language.as_deref().unwrap_or("en");
        let limit = options.max_results.unwrap_or(5);

        let titles = match self.search(query, lang, limit) {
            Ok(t) => t,
            Err(e) => return LookupResult::error(self.name(), e),
        };

        let mut entries = Vec::new();
        for title in titles {
            match self.get_summary(&title, lang) {
                Ok(Some(entry)) => entries.push(entry),
                Ok(None) => continue,
                Err(e) => return LookupResult::error(self.name(), e),
            }
        }

        LookupResult::success(self.name(), entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires network
    fn wikipedia_lookup() {
        let provider = WikipediaProvider::new();
        let result = provider.lookup("Rust programming language", &LookupOptions::default());
        assert!(result.success);
        assert!(!result.entries.is_empty());
        assert!(result.entries[0].title.contains("Rust"));
    }
}
