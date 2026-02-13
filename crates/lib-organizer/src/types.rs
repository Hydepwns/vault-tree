use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileType {
    Pdf,
    Epub,
    Djvu,
    Mobi,
    Chm,
    Unknown,
}

impl FileType {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "pdf" => Self::Pdf,
            "epub" => Self::Epub,
            "djvu" => Self::Djvu,
            "mobi" => Self::Mobi,
            "chm" => Self::Chm,
            _ => Self::Unknown,
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Epub => "epub",
            Self::Djvu => "djvu",
            Self::Mobi => "mobi",
            Self::Chm => "chm",
            Self::Unknown => "",
        }
    }

    pub fn is_supported(&self) -> bool {
        !matches!(self, Self::Unknown)
    }
}

impl fmt::Display for FileType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Pdf => "pdf",
            Self::Epub => "epub",
            Self::Djvu => "djvu",
            Self::Mobi => "mobi",
            Self::Chm => "chm",
            Self::Unknown => "?",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Topic(String);

impl Topic {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into().to_lowercase().replace(' ', "-"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl std::fmt::Display for Topic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for Topic {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for Topic {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibEntry {
    pub path: PathBuf,
    pub original_path: PathBuf,
    pub hash: String,
    pub file_type: FileType,
    pub size: u64,
    pub compressed: bool,
    pub topic: Topic,
    pub subtopic: Option<String>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub ingest_date: DateTime<Utc>,
    pub tags: Vec<String>,
    #[serde(default)]
    pub indexed_at: Option<DateTime<Utc>>,
}

impl LibEntry {
    pub fn new(
        path: PathBuf,
        original_path: PathBuf,
        hash: String,
        file_type: FileType,
        size: u64,
        topic: Topic,
    ) -> Self {
        Self {
            path,
            original_path,
            hash,
            file_type,
            size,
            compressed: false,
            topic,
            subtopic: None,
            title: None,
            author: None,
            ingest_date: Utc::now(),
            tags: Vec::new(),
            indexed_at: None,
        }
    }

    pub fn with_compression(mut self, compressed: bool) -> Self {
        self.compressed = compressed;
        self
    }

    pub fn with_subtopic(mut self, subtopic: impl Into<String>) -> Self {
        self.subtopic = Some(subtopic.into());
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct FileMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Vec<String>,
    pub language: Option<String>,
    pub page_count: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_type_from_extension() {
        assert_eq!(FileType::from_extension("pdf"), FileType::Pdf);
        assert_eq!(FileType::from_extension("PDF"), FileType::Pdf);
        assert_eq!(FileType::from_extension("epub"), FileType::Epub);
        assert_eq!(FileType::from_extension("txt"), FileType::Unknown);
    }

    #[test]
    fn topic_normalization() {
        let topic = Topic::new("Programming Languages");
        assert_eq!(topic.as_str(), "programming-languages");
    }

    #[test]
    fn lib_entry_builder() {
        let entry = LibEntry::new(
            PathBuf::from("programming/rust/book.pdf"),
            PathBuf::from("/home/user/Downloads/book.pdf"),
            "abc123".to_string(),
            FileType::Pdf,
            1024,
            Topic::new("programming"),
        )
        .with_subtopic("rust")
        .with_title("The Rust Book")
        .with_compression(true);

        assert!(entry.compressed);
        assert_eq!(entry.subtopic, Some("rust".to_string()));
        assert_eq!(entry.title, Some("The Rust Book".to_string()));
    }
}
