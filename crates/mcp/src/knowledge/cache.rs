use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::LookupResult;

struct CacheEntry {
    value: LookupResult,
    expires_at: Instant,
}

pub struct LruCache {
    cache: HashMap<String, CacheEntry>,
    max_size: usize,
    ttl: Duration,
    order: Vec<String>,
}

impl LruCache {
    pub fn new(max_size: usize, ttl_minutes: u64) -> Self {
        Self {
            cache: HashMap::new(),
            max_size,
            ttl: Duration::from_secs(ttl_minutes * 60),
            order: Vec::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<LookupResult> {
        let entry = self.cache.get(key)?;

        if Instant::now() > entry.expires_at {
            self.cache.remove(key);
            self.order.retain(|k| k != key);
            return None;
        }

        // Move to end (most recently used)
        self.order.retain(|k| k != key);
        self.order.push(key.to_string());

        Some(entry.value.clone())
    }

    pub fn set(&mut self, key: String, value: LookupResult) {
        // Evict oldest if at capacity
        if self.cache.len() >= self.max_size && !self.cache.contains_key(&key) {
            if let Some(oldest_key) = self.order.first().cloned() {
                self.cache.remove(&oldest_key);
                self.order.remove(0);
            }
        }

        self.order.retain(|k| k != &key);
        self.order.push(key.clone());

        self.cache.insert(
            key,
            CacheEntry {
                value,
                expires_at: Instant::now() + self.ttl,
            },
        );
    }

    pub fn clear(&mut self) {
        self.cache.clear();
        self.order.clear();
    }

    pub fn size(&self) -> usize {
        self.cache.len()
    }
}

pub fn create_cache_key(provider: &str, query: &str, max_results: Option<usize>) -> String {
    match max_results {
        Some(n) => format!("{}:{}:{}", provider, query, n),
        None => format!("{}:{}", provider, query),
    }
}

impl Default for LruCache {
    fn default() -> Self {
        Self::new(100, 15)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::KnowledgeEntry;

    fn test_result() -> LookupResult {
        LookupResult::success(
            "test",
            vec![KnowledgeEntry {
                title: "Test".to_string(),
                summary: "Test summary".to_string(),
                url: None,
                source: "test".to_string(),
                metadata: None,
            }],
        )
    }

    #[test]
    fn cache_stores_and_retrieves() {
        let mut cache = LruCache::new(10, 15);
        cache.set("key1".to_string(), test_result());
        assert!(cache.get("key1").is_some());
    }

    #[test]
    fn cache_evicts_oldest() {
        let mut cache = LruCache::new(2, 15);
        cache.set("key1".to_string(), test_result());
        cache.set("key2".to_string(), test_result());
        cache.set("key3".to_string(), test_result());

        assert!(cache.get("key1").is_none());
        assert!(cache.get("key2").is_some());
        assert!(cache.get("key3").is_some());
    }

    #[test]
    fn cache_key_format() {
        assert_eq!(create_cache_key("wiki", "rust", None), "wiki:rust");
        assert_eq!(create_cache_key("wiki", "rust", Some(5)), "wiki:rust:5");
    }
}
