use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::HashMap;

use super::{KnowledgeEntry, KnowledgeProvider, LookupOptions, LookupResult};

const MUSICBRAINZ_API: &str = "https://musicbrainz.org/ws/2";

pub struct MusicBrainzProvider {
    client: Client,
}

#[derive(Debug, Deserialize)]
struct ArtistSearchResponse {
    artists: Option<Vec<Artist>>,
}

#[derive(Debug, Deserialize)]
struct Artist {
    id: String,
    name: String,
    #[serde(rename = "type")]
    artist_type: Option<String>,
    country: Option<String>,
    #[serde(rename = "life-span")]
    life_span: Option<LifeSpan>,
    disambiguation: Option<String>,
    tags: Option<Vec<Tag>>,
}

#[derive(Debug, Deserialize)]
struct LifeSpan {
    begin: Option<String>,
    end: Option<String>,
    ended: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
struct Tag {
    name: String,
    count: i32,
}

#[derive(Debug, Deserialize)]
struct ReleaseSearchResponse {
    releases: Option<Vec<Release>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Release {
    id: String,
    title: String,
    date: Option<String>,
    country: Option<String>,
    #[serde(rename = "artist-credit")]
    artist_credit: Option<Vec<ArtistCredit>>,
    #[serde(rename = "release-group")]
    release_group: Option<ReleaseGroup>,
}

#[derive(Debug, Deserialize)]
struct ArtistCredit {
    artist: ArtistRef,
}

#[derive(Debug, Deserialize)]
struct ArtistRef {
    name: String,
}

#[derive(Debug, Deserialize)]
struct ReleaseGroup {
    #[serde(rename = "primary-type")]
    primary_type: Option<String>,
}

impl MusicBrainzProvider {
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
            "{}/artist?query={}&limit={}&fmt=json",
            MUSICBRAINZ_API,
            urlencoding::encode(query),
            limit
        );

        let response = self.client.get(&url).send().map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Ok(Vec::new());
        }

        let data: ArtistSearchResponse = response.json().map_err(|e| e.to_string())?;

        Ok(data
            .artists
            .unwrap_or_default()
            .into_iter()
            .map(|artist| {
                let lifespan = artist.life_span.as_ref();
                let years = match (lifespan.and_then(|l| l.begin.as_ref()), lifespan.and_then(|l| l.end.as_ref())) {
                    (Some(b), Some(e)) => format!(" ({} - {})", b, e),
                    (Some(b), None) => {
                        if lifespan.map(|l| l.ended.unwrap_or(false)).unwrap_or(false) {
                            format!(" ({} - ?)", b)
                        } else {
                            format!(" ({} - present)", b)
                        }
                    }
                    _ => String::new(),
                };

                let artist_type = artist.artist_type.as_deref().unwrap_or("Artist");
                let country = artist.country.as_ref().map(|c| format!(", {}", c)).unwrap_or_default();
                let disambiguation = artist.disambiguation.as_ref().map(|d| format!(" - {}", d)).unwrap_or_default();

                let top_tags: String = artist.tags
                    .as_ref()
                    .map(|tags| {
                        let mut sorted = tags.clone();
                        sorted.sort_by(|a, b| b.count.cmp(&a.count));
                        sorted.iter().take(3).map(|t| t.name.clone()).collect::<Vec<_>>().join(", ")
                    })
                    .filter(|s| !s.is_empty())
                    .map(|s| format!(". Genres: {}", s))
                    .unwrap_or_default();

                let mut metadata = HashMap::new();
                metadata.insert("type".to_string(), serde_json::json!("artist"));
                if let Some(t) = &artist.artist_type {
                    metadata.insert("artistType".to_string(), serde_json::json!(t));
                }
                if let Some(c) = &artist.country {
                    metadata.insert("country".to_string(), serde_json::json!(c));
                }
                if let Some(ls) = &artist.life_span {
                    if let Some(b) = &ls.begin {
                        metadata.insert("beginDate".to_string(), serde_json::json!(b));
                    }
                    if let Some(e) = &ls.end {
                        metadata.insert("endDate".to_string(), serde_json::json!(e));
                    }
                }

                KnowledgeEntry {
                    title: artist.name,
                    summary: format!("{}{}{}{}{}", artist_type, country, years, disambiguation, top_tags),
                    url: Some(format!("https://musicbrainz.org/artist/{}", artist.id)),
                    source: "musicbrainz".to_string(),
                    metadata: Some(metadata),
                }
            })
            .collect())
    }

    fn search_releases(&self, query: &str, limit: usize) -> Result<Vec<KnowledgeEntry>, String> {
        let url = format!(
            "{}/release?query={}&limit={}&fmt=json",
            MUSICBRAINZ_API,
            urlencoding::encode(query),
            limit
        );

        let response = self.client.get(&url).send().map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Ok(Vec::new());
        }

        let data: ReleaseSearchResponse = response.json().map_err(|e| e.to_string())?;

        Ok(data
            .releases
            .unwrap_or_default()
            .into_iter()
            .map(|release| {
                let artists = release.artist_credit
                    .as_ref()
                    .map(|ac| ac.iter().map(|a| a.artist.name.clone()).collect::<Vec<_>>().join(", "))
                    .unwrap_or_else(|| "Unknown artist".to_string());

                let year = release.date
                    .as_ref()
                    .map(|d| format!(" ({})", d.get(..4).unwrap_or(d)))
                    .unwrap_or_default();

                let release_type = release.release_group
                    .as_ref()
                    .and_then(|rg| rg.primary_type.as_ref())
                    .map(|t| t.as_str())
                    .unwrap_or("Release");

                let mut metadata = HashMap::new();
                metadata.insert("type".to_string(), serde_json::json!("release"));
                if let Some(rg) = &release.release_group {
                    if let Some(pt) = &rg.primary_type {
                        metadata.insert("releaseType".to_string(), serde_json::json!(pt));
                    }
                }
                if let Some(d) = &release.date {
                    metadata.insert("date".to_string(), serde_json::json!(d));
                }

                KnowledgeEntry {
                    title: release.title,
                    summary: format!("{} by {}{}", release_type, artists, year),
                    url: Some(format!("https://musicbrainz.org/release/{}", release.id)),
                    source: "musicbrainz".to_string(),
                    metadata: Some(metadata),
                }
            })
            .collect())
    }
}

impl Default for MusicBrainzProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeProvider for MusicBrainzProvider {
    fn name(&self) -> &'static str {
        "musicbrainz"
    }

    fn is_available(&self) -> bool {
        self.client
            .get(format!("{}/artist?query=test&limit=1&fmt=json", MUSICBRAINZ_API))
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
            match self.search_releases(query, remaining) {
                Ok(releases) => entries.extend(releases),
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
    fn musicbrainz_lookup() {
        let provider = MusicBrainzProvider::new();
        let result = provider.lookup("Beatles", &LookupOptions::default());
        assert!(result.success);
        assert!(!result.entries.is_empty());
    }
}
