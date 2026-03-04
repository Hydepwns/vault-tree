//! Sync library to remote MinIO storage.
//!
//! Uploads files from the local library to a MinIO bucket and generates
//! an import manifest compatible with droodotfoo's Library context.

use crate::types::{FileType, LibEntry};
use crate::Manifest;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::Path;

/// Configuration for syncing to MinIO.
#[derive(Debug, Clone)]
pub struct SyncConfig {
    /// MinIO endpoint URL (e.g., "http://mini-axol.tail9b2ce8.ts.net:9000")
    pub endpoint: String,
    /// MinIO bucket name
    pub bucket: String,
    /// MinIO access key
    pub access_key: String,
    /// MinIO secret key
    pub secret_key: String,
    /// Prefix for uploaded files in the bucket
    pub prefix: String,
    /// Whether to use path-style URLs (required for MinIO)
    pub path_style: bool,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:9000".to_string(),
            bucket: "droo-library".to_string(),
            access_key: String::new(),
            secret_key: String::new(),
            prefix: "documents".to_string(),
            path_style: true,
        }
    }
}

/// Result of syncing a single file.
#[derive(Debug, Clone)]
pub struct SyncResult {
    pub entry: LibEntry,
    pub s3_key: String,
    pub sha256_hash: String,
    pub uploaded: bool,
    pub skipped_reason: Option<String>,
}

/// Summary of sync operation.
#[derive(Debug, Clone, Default)]
pub struct SyncSummary {
    pub uploaded: usize,
    pub skipped: usize,
    pub failed: usize,
    pub total_bytes: u64,
}

/// Import manifest entry compatible with droodotfoo's Library.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportEntry {
    pub title: String,
    pub slug: String,
    pub content_type: String,
    pub file_key: String,
    pub file_size: i64,
    pub content_hash: String,
    pub tags: Vec<String>,
    pub metadata: ImportMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ImportMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtopic: Option<String>,
}

/// Import manifest for droodotfoo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportManifest {
    pub version: u32,
    pub source: String,
    pub entries: Vec<ImportEntry>,
}

impl ImportManifest {
    pub fn new() -> Self {
        Self {
            version: 1,
            source: "lib-organizer".to_string(),
            entries: Vec::new(),
        }
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

impl Default for ImportManifest {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert FileType to MIME type.
pub fn file_type_to_mime(file_type: FileType) -> &'static str {
    match file_type {
        FileType::Pdf => "application/pdf",
        FileType::Epub => "application/epub+zip",
        FileType::Djvu => "image/vnd.djvu",
        FileType::Mobi => "application/x-mobipocket-ebook",
        FileType::Chm => "application/vnd.ms-htmlhelp",
        FileType::Unknown => "application/octet-stream",
    }
}

/// Generate a slug from a title.
pub fn slugify(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
        .chars()
        .take(100)
        .collect()
}

/// Compute SHA256 hash of file content.
pub fn compute_sha256(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    format!("{:x}", hasher.finalize())
}

/// Compute SHA256 hash of a file.
pub fn compute_sha256_file(path: &Path) -> Result<String> {
    let content = std::fs::read(path).context("Failed to read file")?;
    Ok(compute_sha256(&content))
}

/// Convert a LibEntry to an ImportEntry.
pub fn lib_entry_to_import(entry: &LibEntry, s3_key: &str, sha256_hash: &str) -> ImportEntry {
    let title = entry
        .title
        .clone()
        .unwrap_or_else(|| {
            entry
                .path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("untitled")
                .to_string()
        });

    let slug = slugify(&title);

    let mut tags = entry.tags.clone();
    tags.push(entry.topic.to_string());
    if let Some(ref subtopic) = entry.subtopic {
        tags.push(subtopic.clone());
    }
    tags.sort();
    tags.dedup();

    ImportEntry {
        title,
        slug,
        content_type: file_type_to_mime(entry.file_type).to_string(),
        file_key: s3_key.to_string(),
        file_size: entry.size as i64,
        content_hash: sha256_hash.to_string(),
        tags,
        metadata: ImportMetadata {
            author: entry.author.clone(),
            original_path: Some(entry.original_path.display().to_string()),
            topic: Some(entry.topic.to_string()),
            subtopic: entry.subtopic.clone(),
        },
    }
}

/// Generate S3 key for a library entry.
pub fn generate_s3_key(entry: &LibEntry, prefix: &str) -> String {
    let slug = entry
        .title
        .as_ref()
        .map(|t| slugify(t))
        .unwrap_or_else(|| {
            entry
                .path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(slugify)
                .unwrap_or_else(|| entry.hash[..16].to_string())
        });

    let ext = entry.file_type.extension();
    let ext_suffix = if ext.is_empty() {
        String::new()
    } else {
        format!(".{}", ext)
    };

    format!("{}/{}/{}{}", prefix, slug, slug, ext_suffix)
}

/// Sync client for MinIO operations.
pub struct SyncClient {
    #[allow(dead_code)]
    config: SyncConfig,
    bucket: Box<s3::Bucket>,
}

impl SyncClient {
    /// Create a new sync client.
    pub fn new(config: SyncConfig) -> Result<Self> {
        let region = s3::Region::Custom {
            region: "us-east-1".to_string(),
            endpoint: config.endpoint.clone(),
        };

        let credentials = s3::creds::Credentials::new(
            Some(&config.access_key),
            Some(&config.secret_key),
            None,
            None,
            None,
        )?;

        let mut bucket = s3::Bucket::new(&config.bucket, region, credentials)?;

        if config.path_style {
            bucket = bucket.with_path_style();
        }

        Ok(Self { config, bucket })
    }

    /// List existing objects in the bucket with the configured prefix.
    pub fn list_existing(&self) -> Result<HashSet<String>> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let mut existing = HashSet::new();
            let mut continuation_token: Option<String> = None;

            loop {
                let results = self
                    .bucket
                    .list(self.config.prefix.clone(), None)
                    .await
                    .context("Failed to list bucket")?;

                for result in &results {
                    for obj in &result.contents {
                        existing.insert(obj.key.clone());
                    }

                    if result.is_truncated {
                        continuation_token = result.next_continuation_token.clone();
                    } else {
                        continuation_token = None;
                    }
                }

                if continuation_token.is_none() {
                    break;
                }
            }

            Ok(existing)
        })
    }

    /// Check if an object exists in the bucket.
    pub fn object_exists(&self, key: &str) -> Result<bool> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            match self.bucket.head_object(key).await {
                Ok((_head, code)) => Ok(code == 200),
                Err(e) => {
                    let err_str = format!("{:?}", e);
                    // Check for 404 in the error - rust-s3 may return different error types
                    if err_str.contains("404") || err_str.contains("NoSuchKey") || err_str.contains("NotFound") {
                        Ok(false)
                    } else {
                        Err(e.into())
                    }
                }
            }
        })
    }

    /// Upload a file to the bucket.
    pub fn upload(&self, key: &str, content: &[u8], content_type: &str) -> Result<()> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            self.bucket
                .put_object_with_content_type(key, content, content_type)
                .await
                .context("Failed to upload to S3")?;
            Ok(())
        })
    }

    /// Sync a single library entry.
    pub fn sync_entry(&self, entry: &LibEntry, library_path: &Path) -> Result<SyncResult> {
        let s3_key = generate_s3_key(entry, &self.config.prefix);

        // Read file content
        let file_path = library_path.join(&entry.path);
        let content = std::fs::read(&file_path)
            .with_context(|| format!("Failed to read {}", file_path.display()))?;

        // Compute SHA256 for droodotfoo compatibility
        let sha256_hash = compute_sha256(&content);

        // Check if already exists
        if self.object_exists(&s3_key)? {
            return Ok(SyncResult {
                entry: entry.clone(),
                s3_key,
                sha256_hash,
                uploaded: false,
                skipped_reason: Some("already exists".to_string()),
            });
        }

        // Upload
        let content_type = file_type_to_mime(entry.file_type);
        self.upload(&s3_key, &content, content_type)?;

        Ok(SyncResult {
            entry: entry.clone(),
            s3_key,
            sha256_hash,
            uploaded: true,
            skipped_reason: None,
        })
    }
}

/// Build an import manifest from sync results.
pub fn build_import_manifest(results: &[SyncResult]) -> ImportManifest {
    let mut manifest = ImportManifest::new();

    for result in results {
        if result.skipped_reason.is_none() || result.uploaded {
            let import_entry =
                lib_entry_to_import(&result.entry, &result.s3_key, &result.sha256_hash);
            manifest.entries.push(import_entry);
        }
    }

    manifest
}

/// Dry-run: plan what would be synced without uploading.
pub fn plan_sync(manifest: &Manifest, _library_path: &Path, prefix: &str) -> Vec<(LibEntry, String)> {
    manifest
        .entries
        .iter()
        .map(|entry| {
            let s3_key = generate_s3_key(entry, prefix);
            (entry.clone(), s3_key)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Topic;
    use std::path::PathBuf;

    fn test_entry() -> LibEntry {
        LibEntry::new(
            PathBuf::from("programming/rust/the-rust-book.pdf"),
            PathBuf::from("/home/user/Downloads/rust-book.pdf"),
            "abc123".to_string(),
            FileType::Pdf,
            1024 * 1024,
            Topic::new("programming"),
        )
        .with_subtopic("rust")
        .with_title("The Rust Programming Language")
        .with_author("Steve Klabnik")
    }

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("The Rust Book"), "the-rust-book");
        assert_eq!(slugify("Hello, World!"), "hello-world");
        assert_eq!(slugify("  spaces  "), "spaces");
    }

    #[test]
    fn test_file_type_to_mime() {
        assert_eq!(file_type_to_mime(FileType::Pdf), "application/pdf");
        assert_eq!(file_type_to_mime(FileType::Epub), "application/epub+zip");
    }

    #[test]
    fn test_generate_s3_key() {
        let entry = test_entry();
        let key = generate_s3_key(&entry, "documents");
        assert_eq!(key, "documents/the-rust-programming-language/the-rust-programming-language.pdf");
    }

    #[test]
    fn test_lib_entry_to_import() {
        let entry = test_entry();
        let import = lib_entry_to_import(&entry, "documents/test/test.pdf", "sha256hash");

        assert_eq!(import.title, "The Rust Programming Language");
        assert_eq!(import.content_type, "application/pdf");
        assert!(import.tags.contains(&"programming".to_string()));
        assert!(import.tags.contains(&"rust".to_string()));
        assert_eq!(import.metadata.author, Some("Steve Klabnik".to_string()));
    }

    #[test]
    fn test_compute_sha256() {
        let hash = compute_sha256(b"hello world");
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }
}
