use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::types::{LibEntry, Topic};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub version: u32,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    pub entries: Vec<LibEntry>,
}

impl Manifest {
    pub fn new<P: AsRef<Path>>(_path: P) -> Self {
        let now = Utc::now();
        Self {
            version: 1,
            created: now,
            updated: now,
            entries: Vec::new(),
        }
    }

    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        serde_json::from_str(&content).map_err(Into::into)
    }

    pub fn load_or_create(path: &Path) -> anyhow::Result<Self> {
        if path.exists() {
            Self::load(path)
        } else {
            Ok(Self::new(path))
        }
    }

    pub fn save_to(&self, path: &Path) -> anyhow::Result<()> {
        let updated = Self {
            updated: Utc::now(),
            ..self.clone()
        };
        let content = serde_json::to_string_pretty(&updated)?;
        std::fs::write(path, content).map_err(Into::into)
    }

    /// Returns new Manifest with entry added (immutable)
    pub fn with_entry(&self, entry: LibEntry) -> Self {
        let entries = self
            .entries
            .iter()
            .cloned()
            .chain(std::iter::once(entry))
            .collect();

        Self {
            entries,
            ..self.clone()
        }
    }

    /// Returns new Manifest with entry removed (immutable)
    pub fn without_hash(&self, hash: &str) -> Self {
        let entries = self
            .entries
            .iter()
            .filter(|e| e.hash != hash)
            .cloned()
            .collect();

        Self {
            entries,
            ..self.clone()
        }
    }

    pub fn find_by_hash(&self, hash: &str) -> Option<&LibEntry> {
        self.entries.iter().find(|e| e.hash == hash)
    }

    pub fn find_by_path(&self, path: &Path) -> Option<&LibEntry> {
        self.entries.iter().find(|e| e.path == path)
    }

    pub fn contains_hash(&self, hash: &str) -> bool {
        self.entries.iter().any(|e| e.hash == hash)
    }

    pub fn by_topic(&self) -> HashMap<&Topic, Vec<&LibEntry>> {
        self.entries.iter().fold(HashMap::new(), |mut acc, entry| {
            acc.entry(&entry.topic).or_default().push(entry);
            acc
        })
    }

    pub fn search(&self, query: &str) -> Vec<&LibEntry> {
        let query_lower = query.to_lowercase();
        let matches_query = |s: &str| s.to_lowercase().contains(&query_lower);

        self.entries
            .iter()
            .filter(|e| {
                e.path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(matches_query)
                    .unwrap_or(false)
                    || e.title.as_ref().map(|t| matches_query(t)).unwrap_or(false)
                    || e.author.as_ref().map(|a| matches_query(a)).unwrap_or(false)
                    || e.tags.iter().any(|t| matches_query(t))
            })
            .collect()
    }

    pub fn total_size(&self) -> u64 {
        self.entries.iter().map(|e| e.size).sum()
    }

    pub fn count(&self) -> usize {
        self.entries.len()
    }

    pub fn count_by_topic(&self) -> HashMap<&Topic, usize> {
        self.entries.iter().fold(HashMap::new(), |mut acc, entry| {
            *acc.entry(&entry.topic).or_default() += 1;
            acc
        })
    }

    // Mutable methods for backwards compatibility
    pub fn add(&mut self, entry: LibEntry) {
        self.entries.push(entry);
    }

    pub fn remove(&mut self, hash: &str) -> Option<LibEntry> {
        self.entries
            .iter()
            .position(|e| e.hash == hash)
            .map(|pos| self.entries.remove(pos))
    }

    pub fn mark_indexed(&mut self, hash: &str) {
        let now = Utc::now();
        if let Some(entry) = self.entries.iter_mut().find(|e| e.hash == hash) {
            entry.indexed_at = Some(now);
        }
    }

    pub fn mark_indexed_batch(&mut self, hashes: &[String]) {
        let now = Utc::now();
        for entry in &mut self.entries {
            if hashes.contains(&entry.hash) {
                entry.indexed_at = Some(now);
            }
        }
    }

    pub fn save(&mut self) -> anyhow::Result<()> {
        self.updated = Utc::now();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::FileType;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn test_entry(hash: &str, topic: &str) -> LibEntry {
        LibEntry::new(
            PathBuf::from(format!("{}/test.pdf", topic)),
            PathBuf::from("/original/test.pdf"),
            hash.to_string(),
            FileType::Pdf,
            1024,
            Topic::new(topic),
        )
    }

    #[test]
    fn manifest_with_entry_is_immutable() {
        let manifest = Manifest::new("/lib/manifest.json");
        let entry = test_entry("abc123", "programming");

        let new_manifest = manifest.with_entry(entry);

        assert_eq!(manifest.count(), 0);
        assert_eq!(new_manifest.count(), 1);
        assert!(new_manifest.contains_hash("abc123"));
    }

    #[test]
    fn manifest_without_hash_is_immutable() {
        let manifest = Manifest::new("/lib/manifest.json")
            .with_entry(test_entry("abc123", "programming"))
            .with_entry(test_entry("def456", "electronics"));

        let new_manifest = manifest.without_hash("abc123");

        assert_eq!(manifest.count(), 2);
        assert_eq!(new_manifest.count(), 1);
        assert!(!new_manifest.contains_hash("abc123"));
        assert!(new_manifest.contains_hash("def456"));
    }

    #[test]
    fn manifest_save_load() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("manifest.json");

        let manifest = Manifest::new(&path).with_entry(test_entry("abc123", "programming"));
        manifest.save_to(&path).unwrap();

        let loaded = Manifest::load(&path).unwrap();
        assert_eq!(loaded.count(), 1);
        assert!(loaded.contains_hash("abc123"));
    }

    #[test]
    fn manifest_by_topic_uses_fold() {
        let manifest = Manifest::new("/lib/manifest.json")
            .with_entry(test_entry("a", "programming"))
            .with_entry(test_entry("b", "programming"))
            .with_entry(test_entry("c", "electronics"));

        let by_topic = manifest.by_topic();
        assert_eq!(by_topic.get(&Topic::new("programming")).unwrap().len(), 2);
        assert_eq!(by_topic.get(&Topic::new("electronics")).unwrap().len(), 1);
    }

    #[test]
    fn manifest_search() {
        let mut entry = test_entry("a", "programming");
        entry.title = Some("Rust Programming".to_string());

        let manifest = Manifest::new("/lib/manifest.json").with_entry(entry);
        let results = manifest.search("rust");

        assert_eq!(results.len(), 1);
    }
}
