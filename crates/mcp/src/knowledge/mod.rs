mod wikipedia;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub use wikipedia::WikipediaProvider;

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
}

impl Default for KnowledgeRegistry {
    fn default() -> Self {
        Self::new()
    }
}
