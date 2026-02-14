mod arxiv;
mod dbpedia;
mod defillama;
mod github;
mod musicbrainz;
mod openlibrary;
mod shodan;
mod sourceforge;
mod wikiart;
mod wikidata;
mod wikipedia;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub use arxiv::ArxivProvider;
pub use dbpedia::DBpediaProvider;
pub use defillama::DefiLlamaProvider;
pub use github::GitHubProvider;
pub use musicbrainz::MusicBrainzProvider;
pub use openlibrary::OpenLibraryProvider;
pub use shodan::ShodanProvider;
pub use sourceforge::SourceForgeProvider;
pub use wikiart::WikiArtProvider;
pub use wikidata::WikidataProvider;
pub use wikipedia::WikipediaProvider;

const PROVIDER_ORDER: &[&str] = &[
    "wikipedia",
    "dbpedia",
    "wikidata",
    "github",
    "sourceforge",
    "openlibrary",
    "arxiv",
    "musicbrainz",
    "wikiart",
    "defillama",
    "shodan",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEntry {
    pub title: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Default)]
pub struct LookupOptions {
    pub max_results: Option<usize>,
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupResult {
    pub success: bool,
    pub provider: String,
    pub entries: Vec<KnowledgeEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl LookupResult {
    pub fn success(provider: &str, entries: Vec<KnowledgeEntry>) -> Self {
        Self {
            success: true,
            provider: provider.to_string(),
            entries,
            error: None,
        }
    }

    pub fn error(provider: &str, error: impl Into<String>) -> Self {
        Self {
            success: false,
            provider: provider.to_string(),
            entries: Vec::new(),
            error: Some(error.into()),
        }
    }
}

pub trait KnowledgeProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn is_available(&self) -> bool;
    fn lookup(&self, query: &str, options: &LookupOptions) -> LookupResult;
}

pub struct KnowledgeRegistry {
    providers: HashMap<String, Box<dyn KnowledgeProvider>>,
}

impl KnowledgeRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            providers: HashMap::new(),
        };
        registry.register(Box::new(WikipediaProvider::new()));
        registry.register(Box::new(DBpediaProvider::new()));
        registry.register(Box::new(WikidataProvider::new()));
        registry.register(Box::new(GitHubProvider::new()));
        registry.register(Box::new(SourceForgeProvider::new()));
        registry.register(Box::new(OpenLibraryProvider::new()));
        registry.register(Box::new(ArxivProvider::new()));
        registry.register(Box::new(MusicBrainzProvider::new()));
        registry.register(Box::new(WikiArtProvider::new()));
        registry.register(Box::new(DefiLlamaProvider::new()));

        let shodan = match std::env::var("SHODAN_API_KEY") {
            Ok(key) if !key.is_empty() => ShodanProvider::with_api_key(key),
            _ => ShodanProvider::new(),
        };
        registry.register(Box::new(shodan));

        registry
    }

    pub fn register(&mut self, provider: Box<dyn KnowledgeProvider>) {
        self.providers.insert(provider.name().to_string(), provider);
    }

    pub fn lookup(
        &self,
        provider: &str,
        query: &str,
        options: &LookupOptions,
    ) -> Option<LookupResult> {
        self.providers
            .get(provider)
            .map(|p| p.lookup(query, options))
    }

    pub fn available_providers(&self) -> Vec<&str> {
        self.providers
            .iter()
            .filter(|(_, p)| p.is_available())
            .map(|(name, _)| name.as_str())
            .collect()
    }

    pub fn auto_lookup(&self, query: &str, options: &LookupOptions) -> LookupResult {
        for &provider_name in PROVIDER_ORDER {
            if let Some(provider) = self.providers.get(provider_name) {
                if !provider.is_available() {
                    continue;
                }

                let result = provider.lookup(query, options);
                if result.success && !result.entries.is_empty() {
                    return result;
                }
            }
        }

        LookupResult::success("auto", Vec::new())
    }
}

impl Default for KnowledgeRegistry {
    fn default() -> Self {
        Self::new()
    }
}
