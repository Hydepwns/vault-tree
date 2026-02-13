use std::collections::HashMap;
use std::path::Path;

use crate::config::Config;
use crate::types::{FileMetadata, FileType, Topic};

/// Split a string on camelCase and PascalCase boundaries.
/// "eigenlayerWhitepaper" -> ["eigenlayer", "Whitepaper"]
/// "PDFDocument" -> ["PDF", "Document"]
fn split_camel_case(s: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();

    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        let next_is_lower = chars.get(i + 1).map(|n| n.is_lowercase()).unwrap_or(false);

        if c.is_uppercase() && !current.is_empty() {
            // Start new word if: lowercase->uppercase OR uppercase->uppercase followed by lowercase
            let prev_is_lower = current
                .chars()
                .last()
                .map(|p| p.is_lowercase())
                .unwrap_or(false);
            if prev_is_lower || next_is_lower {
                words.push(std::mem::take(&mut current));
            }
        }
        current.push(c);
    }

    if !current.is_empty() {
        words.push(current);
    }

    words
}

#[derive(Debug, Clone)]
pub struct ClassificationResult {
    pub topic: Topic,
    pub subtopic: Option<String>,
    pub confidence: Confidence,
    pub metadata: FileMetadata,
    pub matched_keywords: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    High,
    Medium,
    Low,
}

impl Confidence {
    fn from_match_count(count: usize) -> Self {
        match count {
            0 => Self::Low,
            1 => Self::Medium,
            _ => Self::High,
        }
    }
}

pub trait Classifier {
    fn classify(&self, path: &Path, file_type: FileType) -> anyhow::Result<ClassificationResult>;
}

pub struct RuleBasedClassifier {
    keyword_rules: HashMap<String, Topic>,
}

impl RuleBasedClassifier {
    pub fn new(config: &Config) -> Self {
        Self {
            keyword_rules: config.keyword_rules.clone(),
        }
    }

    fn extract_keywords_from_filename(&self, path: &Path) -> Vec<String> {
        path.file_stem()
            .and_then(|s| s.to_str())
            .map(|name| {
                // First split on non-alphanumeric chars
                name.replace(|c: char| !c.is_alphanumeric(), " ")
                    .split_whitespace()
                    // Then split each word on camelCase boundaries
                    .flat_map(split_camel_case)
                    .map(|s| s.to_lowercase())
                    .filter(|w| w.len() > 2)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn match_keywords(&self, keywords: &[String]) -> Option<(Topic, Vec<String>)> {
        keywords
            .iter()
            .filter_map(|kw| {
                self.keyword_rules
                    .get(kw)
                    .map(|topic| (topic.clone(), kw.clone()))
            })
            .fold(
                HashMap::<Topic, Vec<String>>::new(),
                |mut acc, (topic, keyword)| {
                    acc.entry(topic).or_default().push(keyword);
                    acc
                },
            )
            .into_iter()
            .max_by_key(|(_, kws)| kws.len())
    }

    fn infer_subtopic(&self, topic: &Topic, keywords: &[String]) -> Option<String> {
        const PROGRAMMING_LANGUAGES: &[&str] = &[
            "rust",
            "python",
            "go",
            "java",
            "javascript",
            "typescript",
            "c",
            "cpp",
            "ruby",
            "elixir",
            "haskell",
            "scala",
            "kotlin",
            "swift",
            "lua",
        ];

        (topic.as_str() == "programming")
            .then(|| {
                PROGRAMMING_LANGUAGES
                    .iter()
                    .find(|&&lang| keywords.iter().any(|k| k == lang))
                    .map(|&s| s.to_string())
            })
            .flatten()
    }
}

impl Classifier for RuleBasedClassifier {
    fn classify(&self, path: &Path, file_type: FileType) -> anyhow::Result<ClassificationResult> {
        let metadata = extract_metadata(path, file_type).unwrap_or_default();

        let all_keywords = collect_keywords(path, &metadata, self);

        let (topic, matched_keywords, confidence) = self
            .match_keywords(&all_keywords)
            .map(|(topic, matched)| {
                let confidence = Confidence::from_match_count(matched.len());
                (topic, matched, confidence)
            })
            .unwrap_or_else(|| (Topic::new("other"), vec![], Confidence::Low));

        let subtopic = self.infer_subtopic(&topic, &all_keywords);

        Ok(ClassificationResult {
            topic,
            subtopic,
            confidence,
            metadata,
            matched_keywords,
        })
    }
}

fn extract_metadata(path: &Path, file_type: FileType) -> Option<FileMetadata> {
    match file_type {
        FileType::Pdf => extract_pdf_metadata(path).ok(),
        FileType::Epub => extract_epub_metadata(path).ok(),
        _ => None,
    }
}

fn collect_keywords(
    path: &Path,
    metadata: &FileMetadata,
    classifier: &RuleBasedClassifier,
) -> Vec<String> {
    let filename_keywords = classifier.extract_keywords_from_filename(path);

    let title_keywords = metadata
        .title
        .as_ref()
        .map(|t| {
            t.to_lowercase()
                .split_whitespace()
                .filter(|w| w.len() > 2)
                .map(String::from)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let meta_keywords = metadata.keywords.iter().map(|k| k.to_lowercase());

    filename_keywords
        .into_iter()
        .chain(title_keywords)
        .chain(meta_keywords)
        .collect()
}

pub fn classify_file(
    path: &Path,
    file_type: FileType,
    config: &Config,
) -> anyhow::Result<ClassificationResult> {
    RuleBasedClassifier::new(config).classify(path, file_type)
}

fn extract_pdf_metadata(path: &Path) -> anyhow::Result<FileMetadata> {
    let doc = lopdf::Document::load(path)?;

    let info_dict = doc
        .trailer
        .get(b"Info")
        .ok()
        .and_then(|info| info.as_reference().ok())
        .and_then(|info_ref| doc.get_dictionary(info_ref).ok());

    let get_string = |dict: &lopdf::Dictionary, key: &[u8]| -> Option<String> {
        dict.get(key)
            .ok()
            .and_then(|v| v.as_str().ok())
            .map(|s| String::from_utf8_lossy(s).to_string())
            .filter(|s| !s.is_empty())
    };

    let (title, author, subject, keywords) = info_dict
        .map(|dict| {
            (
                get_string(dict, b"Title"),
                get_string(dict, b"Author"),
                get_string(dict, b"Subject"),
                get_string(dict, b"Keywords").map(|s| {
                    s.split([',', ';', ' '])
                        .filter(|k| !k.is_empty())
                        .map(String::from)
                        .collect::<Vec<_>>()
                }),
            )
        })
        .unwrap_or_default();

    let page_count = doc.get_pages().len().try_into().ok();

    Ok(FileMetadata {
        title,
        author,
        subject,
        keywords: keywords.unwrap_or_default(),
        language: None,
        page_count,
    })
}

fn extract_epub_metadata(path: &Path) -> anyhow::Result<FileMetadata> {
    let doc = epub::doc::EpubDoc::new(path)?;

    Ok(FileMetadata {
        title: doc.mdata("title").map(|m| m.value.clone()),
        author: doc.mdata("creator").map(|m| m.value.clone()),
        subject: doc.mdata("subject").map(|m| m.value.clone()),
        language: doc.mdata("language").map(|m| m.value.clone()),
        keywords: Vec::new(),
        page_count: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_keywords_from_filename() {
        let config = Config::new("/lib");
        let classifier = RuleBasedClassifier::new(&config);

        let keywords = classifier
            .extract_keywords_from_filename(Path::new("/path/to/Rust_Programming_Book.pdf"));

        assert!(keywords.contains(&"rust".to_string()));
        assert!(keywords.contains(&"programming".to_string()));
        assert!(keywords.contains(&"book".to_string()));
    }

    #[test]
    fn classify_by_filename() {
        let config = Config::new("/lib");
        let result =
            classify_file(Path::new("/path/to/rust_book.pdf"), FileType::Pdf, &config).unwrap();

        assert_eq!(result.topic, Topic::new("programming"));
        assert_eq!(result.subtopic, Some("rust".to_string()));
    }

    #[test]
    fn unknown_file_gets_other() {
        let config = Config::new("/lib");
        let result = classify_file(
            Path::new("/path/to/random_stuff.pdf"),
            FileType::Pdf,
            &config,
        )
        .unwrap();

        assert_eq!(result.topic, Topic::new("other"));
        assert_eq!(result.confidence, Confidence::Low);
    }

    #[test]
    fn confidence_from_match_count() {
        assert_eq!(Confidence::from_match_count(0), Confidence::Low);
        assert_eq!(Confidence::from_match_count(1), Confidence::Medium);
        assert_eq!(Confidence::from_match_count(2), Confidence::High);
        assert_eq!(Confidence::from_match_count(10), Confidence::High);
    }

    #[test]
    fn split_camel_case_works() {
        assert_eq!(
            split_camel_case("eigenlayerWhitepaper"),
            vec!["eigenlayer", "Whitepaper"]
        );
        assert_eq!(split_camel_case("BitcoinCore"), vec!["Bitcoin", "Core"]);
        assert_eq!(split_camel_case("PDFDocument"), vec!["PDF", "Document"]);
        assert_eq!(split_camel_case("simpleword"), vec!["simpleword"]);
        // Consecutive uppercase splits before the last uppercase before lowercase
        assert_eq!(split_camel_case("ABCdef"), vec!["AB", "Cdef"]);
    }

    #[test]
    fn camel_case_filename_extracts_keywords() {
        let config = Config::new("/lib");
        let classifier = RuleBasedClassifier::new(&config);

        let keywords = classifier
            .extract_keywords_from_filename(Path::new("/path/to/eigenlayerWhitepaper.pdf"));

        assert!(keywords.contains(&"eigenlayer".to_string()));
        assert!(keywords.contains(&"whitepaper".to_string()));
    }

    #[test]
    fn classify_arxiv_paper() {
        let config = Config::new("/lib");
        let result = classify_file(
            Path::new("/path/to/2309.04269-arxiv-paper.pdf"),
            FileType::Pdf,
            &config,
        )
        .unwrap();

        assert_eq!(result.topic, Topic::new("research"));
        assert!(result.matched_keywords.contains(&"arxiv".to_string()));
    }

    #[test]
    fn classify_crypto_camel_case() {
        let config = Config::new("/lib");
        let result = classify_file(
            Path::new("/path/to/eigenlayerWhitepaper.pdf"),
            FileType::Pdf,
            &config,
        )
        .unwrap();

        assert_eq!(result.topic, Topic::new("crypto"));
        assert!(result.matched_keywords.contains(&"eigenlayer".to_string()));
    }
}
