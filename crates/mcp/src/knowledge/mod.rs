mod arxiv;
mod cache;
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
use std::sync::Mutex;

use cache::{create_cache_key, LruCache};

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
    cache: Mutex<LruCache>,
    cache_enabled: bool,
}

impl KnowledgeRegistry {
    pub fn new() -> Self {
        Self::with_cache(true, 100, 15)
    }

    pub fn with_cache(enabled: bool, max_size: usize, ttl_minutes: u64) -> Self {
        let mut registry = Self {
            providers: HashMap::new(),
            cache: Mutex::new(LruCache::new(max_size, ttl_minutes)),
            cache_enabled: enabled,
        };
        registry.register(Box::new(WikipediaProvider::new()));
        registry.register(Box::new(DBpediaProvider::new()));
        registry.register(Box::new(WikidataProvider::new()));

        let github = match std::env::var("GITHUB_TOKEN") {
            Ok(token) if !token.is_empty() => GitHubProvider::with_token(token),
            _ => GitHubProvider::new(),
        };
        registry.register(Box::new(github));

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
        let cache_key = create_cache_key(provider, query, options.max_results);

        // Check cache first
        if self.cache_enabled {
            if let Ok(mut cache) = self.cache.lock() {
                if let Some(cached) = cache.get(&cache_key) {
                    return Some(cached);
                }
            }
        }

        let result = self.providers.get(provider).map(|p| p.lookup(query, options))?;

        // Cache successful results
        if self.cache_enabled && result.success {
            if let Ok(mut cache) = self.cache.lock() {
                cache.set(cache_key, result.clone());
            }
        }

        Some(result)
    }

    pub fn available_providers(&self) -> Vec<&str> {
        self.providers
            .iter()
            .filter(|(_, p)| p.is_available())
            .map(|(name, _)| name.as_str())
            .collect()
    }

    pub fn auto_lookup(&self, query: &str, options: &LookupOptions) -> LookupResult {
        let cache_key = create_cache_key("auto", query, options.max_results);

        // Check cache first
        if self.cache_enabled {
            if let Ok(mut cache) = self.cache.lock() {
                if let Some(cached) = cache.get(&cache_key) {
                    return cached;
                }
            }
        }

        for &provider_name in PROVIDER_ORDER {
            if let Some(provider) = self.providers.get(provider_name) {
                if !provider.is_available() {
                    continue;
                }

                let result = provider.lookup(query, options);
                if result.success && !result.entries.is_empty() {
                    // Cache the result
                    if self.cache_enabled {
                        if let Ok(mut cache) = self.cache.lock() {
                            cache.set(cache_key, result.clone());
                        }
                    }
                    return result;
                }
            }
        }

        LookupResult::success("auto", Vec::new())
    }

    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.clear();
        }
    }

    pub fn cache_size(&self) -> usize {
        self.cache.lock().map(|c| c.size()).unwrap_or(0)
    }
}

impl Default for KnowledgeRegistry {
    fn default() -> Self {
        Self::new()
    }
}
