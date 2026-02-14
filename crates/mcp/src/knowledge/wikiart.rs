use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::HashMap;

use super::{KnowledgeEntry, KnowledgeProvider, LookupOptions, LookupResult};

const WIKIART_API: &str = "https://www.wikiart.org/en/api/2";

pub struct WikiArtProvider {
    client: Client,
}

#[derive(Debug, Deserialize)]
struct ArtistItem {
    #[serde(rename = "artistName")]
    artist_name: Option<String>,
    #[serde(rename = "birthDay")]
    birth_day: Option<String>,
    #[serde(rename = "deathDay")]
    death_day: Option<String>,
    biography: Option<String>,
    image: Option<String>,
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PaintingSearchResponse {
    data: Option<Vec<PaintingItem>>,
}

#[derive(Debug, Deserialize)]
struct PaintingItem {
    title: String,
    #[serde(rename = "artistName")]
    artist_name: Option<String>,
    year: Option<String>,
    image: Option<String>,
    #[serde(rename = "artistUrl")]
    artist_url: Option<String>,
}

impl WikiArtProvider {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("vault-tree-mcp/0.1 (https://github.com/Hydepwns/vault-tree)")
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }

    fn search_artists(&self, query: &str, limit: usize) -> Result<Vec<KnowledgeEntry>, String> {
        let url = format!(
            "{}/App/Search/ArtistByName?searchParameter={}",
            WIKIART_API,
            urlencoding::encode(query)
        );

        let response = self.client.get(&url).send().map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Ok(Vec::new());
        }

        let data: Vec<ArtistItem> = response.json().map_err(|e| e.to_string())?;

        Ok(data
            .into_iter()
            .take(limit)
            .filter_map(|artist| {
                let name = artist.artist_name.as_ref()?;

                let years = match (&artist.birth_day, &artist.death_day) {
                    (Some(b), Some(d)) => format!(" ({} - {})", b, d),
                    (Some(b), None) => format!(" ({} - )", b),
                    (None, Some(d)) => format!(" (? - {})", d),
                    (None, None) => String::new(),
                };

                let summary = artist.biography
                    .as_ref()
                    .map(|b| b.chars().take(500).collect::<String>())
                    .unwrap_or_else(|| format!("Artist{}", years));

                let mut metadata = HashMap::new();
                metadata.insert("type".to_string(), serde_json::json!("artist"));
                if let Some(img) = &artist.image {
                    metadata.insert("image".to_string(), serde_json::json!(img));
                }
                if let Some(b) = &artist.birth_day {
                    metadata.insert("birthDay".to_string(), serde_json::json!(b));
                }
                if let Some(d) = &artist.death_day {
                    metadata.insert("deathDay".to_string(), serde_json::json!(d));
                }

                Some(KnowledgeEntry {
                    title: name.clone(),
                    summary,
                    url: artist.url.map(|u| format!("https://www.wikiart.org{}", u)),
                    source: "wikiart".to_string(),
                    metadata: Some(metadata),
                })
            })
            .collect())
    }

    fn search_paintings(&self, query: &str, limit: usize) -> Result<Vec<KnowledgeEntry>, String> {
        let url = format!(
            "{}/App/Search/PaintingsByText?searchParameter={}",
            WIKIART_API,
            urlencoding::encode(query)
        );

        let response = self.client.get(&url).send().map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Ok(Vec::new());
        }

        let data: PaintingSearchResponse = response.json().map_err(|e| e.to_string())?;

        Ok(data
            .data
            .unwrap_or_default()
            .into_iter()
            .take(limit)
            .map(|painting| {
                let artist = painting.artist_name.as_deref().unwrap_or("Unknown");
                let year = painting.year.as_ref().map(|y| format!(" ({})", y)).unwrap_or_default();

                let mut metadata = HashMap::new();
                metadata.insert("type".to_string(), serde_json::json!("painting"));
                if let Some(a) = &painting.artist_name {
                    metadata.insert("artist".to_string(), serde_json::json!(a));
                }
                if let Some(y) = &painting.year {
                    metadata.insert("year".to_string(), serde_json::json!(y));
                }
                if let Some(img) = &painting.image {
                    metadata.insert("image".to_string(), serde_json::json!(img));
                }

                KnowledgeEntry {
                    title: painting.title,
                    summary: format!("{}{}", artist, year),
                    url: painting.artist_url.map(|u| format!("https://www.wikiart.org{}", u)),
                    source: "wikiart".to_string(),
                    metadata: Some(metadata),
                }
            })
            .collect())
    }
}

impl Default for WikiArtProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeProvider for WikiArtProvider {
    fn name(&self) -> &'static str {
        "wikiart"
    }

    fn is_available(&self) -> bool {
        self.client
            .get(format!("{}/App/Artist/AlphabetJson", WIKIART_API))
            .send()
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    fn lookup(&self, query: &str, options: &LookupOptions) -> LookupResult {
        let limit = options.max_results.unwrap_or(5);

        let mut entries = match self.search_artists(query, limit) {
            Ok(e) => e,
            Err(e) => return LookupResult::error(self.name(), e),
        };

        if entries.len() < limit {
            let remaining = limit - entries.len();
            match self.search_paintings(query, remaining) {
                Ok(paintings) => entries.extend(paintings),
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
    fn wikiart_lookup() {
        let provider = WikiArtProvider::new();
        let result = provider.lookup("Monet", &LookupOptions::default());
        assert!(result.success);
    }
}
