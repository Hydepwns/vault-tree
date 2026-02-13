use rayon::prelude::*;
use std::path::{Path, PathBuf};
use tantivy::{
    directory::MmapDirectory,
    schema::{Field, Schema, STORED, STRING, TEXT},
    Index, IndexReader, IndexSettings, IndexWriter, TantivyDocument,
};
use thiserror::Error;

use super::extractor::{extract_epub_text, extract_pdf_text, ExtractedText};
use crate::types::FileType;

#[derive(Debug, Error)]
pub enum IndexError {
    #[error("failed to create index directory: {0}")]
    CreateDir(#[from] std::io::Error),
    #[error("tantivy error: {0}")]
    Tantivy(#[from] tantivy::TantivyError),
    #[error("failed to open directory: {0}")]
    OpenDir(#[from] tantivy::directory::error::OpenDirectoryError),
    #[error("index not found at {0}")]
    NotFound(PathBuf),
    #[error("extraction failed: {0}")]
    Extract(#[from] super::extractor::ExtractError),
}

pub struct SearchIndex {
    index: Index,
    reader: IndexReader,
    writer: IndexWriter,
    schema: SearchSchema,
    index_path: PathBuf,
}

const SNIPPET_MAX_CHARS: usize = 1000;

fn truncate_to_char_boundary(s: &str, max_chars: usize) -> &str {
    if s.len() <= max_chars {
        return s;
    }
    let mut end = max_chars;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

#[derive(Clone)]
pub struct SearchSchema {
    pub file_hash: Field,
    pub file_path: Field,
    pub title: Field,
    pub author: Field,
    pub content: Field,
    pub content_preview: Field,
}

impl SearchSchema {
    fn build() -> (Schema, Self) {
        let mut schema_builder = Schema::builder();

        let file_hash = schema_builder.add_text_field("file_hash", STRING | STORED);
        let file_path = schema_builder.add_text_field("file_path", STORED);
        let title = schema_builder.add_text_field("title", TEXT | STORED);
        let author = schema_builder.add_text_field("author", TEXT | STORED);
        let content = schema_builder.add_text_field("content", TEXT);
        let content_preview = schema_builder.add_text_field("content_preview", STORED);

        let schema = schema_builder.build();
        let fields = Self {
            file_hash,
            file_path,
            title,
            author,
            content,
            content_preview,
        };

        (schema, fields)
    }
}

impl SearchIndex {
    pub fn open_or_create(library_path: &Path) -> Result<Self, IndexError> {
        let index_path = library_path.join(".search-index");

        let (schema, fields) = SearchSchema::build();

        let index = if index_path.exists() {
            let dir = MmapDirectory::open(&index_path)?;
            Index::open(dir)?
        } else {
            std::fs::create_dir_all(&index_path)?;
            let dir = MmapDirectory::open(&index_path)?;
            Index::create(dir, schema.clone(), IndexSettings::default())?
        };

        let reader = index.reader()?;
        let writer = index.writer(50_000_000)?; // 50MB heap

        Ok(Self {
            index,
            reader,
            writer,
            schema: fields,
            index_path,
        })
    }

    pub fn schema(&self) -> &SearchSchema {
        &self.schema
    }

    pub fn reader(&self) -> &IndexReader {
        &self.reader
    }

    pub fn index(&self) -> &Index {
        &self.index
    }

    pub fn index_path(&self) -> &Path {
        &self.index_path
    }

    pub fn add_document(
        &mut self,
        file_hash: &str,
        file_path: &Path,
        title: Option<&str>,
        author: Option<&str>,
        content: &str,
    ) -> Result<(), IndexError> {
        let mut doc = TantivyDocument::new();
        doc.add_text(self.schema.file_hash, file_hash);
        doc.add_text(self.schema.file_path, file_path.to_string_lossy());
        if let Some(t) = title {
            doc.add_text(self.schema.title, t);
        }
        if let Some(a) = author {
            doc.add_text(self.schema.author, a);
        }
        doc.add_text(self.schema.content, content);

        let preview = truncate_to_char_boundary(content, SNIPPET_MAX_CHARS);
        doc.add_text(self.schema.content_preview, preview);

        self.writer.add_document(doc)?;
        Ok(())
    }

    pub fn add_pdf(
        &mut self,
        file_hash: &str,
        file_path: &Path,
        title: Option<&str>,
        author: Option<&str>,
    ) -> Result<ExtractedText, IndexError> {
        let extracted = extract_pdf_text(file_path)?;

        if !extracted.is_empty() {
            self.add_document(file_hash, file_path, title, author, &extracted.content)?;
        }

        Ok(extracted)
    }

    pub fn add_epub(
        &mut self,
        file_hash: &str,
        file_path: &Path,
        title: Option<&str>,
        author: Option<&str>,
    ) -> Result<ExtractedText, IndexError> {
        let extracted = extract_epub_text(file_path)?;

        if !extracted.is_empty() {
            self.add_document(file_hash, file_path, title, author, &extracted.content)?;
        }

        Ok(extracted)
    }

    pub fn remove_document(&mut self, file_hash: &str) -> Result<(), IndexError> {
        let term = tantivy::Term::from_field_text(self.schema.file_hash, file_hash);
        self.writer.delete_term(term);
        Ok(())
    }

    pub fn commit(&mut self) -> Result<(), IndexError> {
        self.writer.commit()?;
        self.reader.reload()?;
        Ok(())
    }

    pub fn clear(&mut self) -> Result<(), IndexError> {
        self.writer.delete_all_documents()?;
        self.commit()
    }

    pub fn document_count(&self) -> u64 {
        let searcher = self.reader.searcher();
        searcher.num_docs()
    }

    pub fn contains_hash(&self, file_hash: &str) -> bool {
        let searcher = self.reader.searcher();
        let term = tantivy::Term::from_field_text(self.schema.file_hash, file_hash);
        let query = tantivy::query::TermQuery::new(term, tantivy::schema::IndexRecordOption::Basic);

        searcher
            .search(&query, &tantivy::collector::Count)
            .map(|count| count > 0)
            .unwrap_or(false)
    }

    pub fn indexed_hashes(&self) -> Vec<String> {
        let searcher = self.reader.searcher();
        let mut hashes = Vec::new();

        for segment_reader in searcher.segment_readers() {
            if let Ok(inverted_index) = segment_reader.inverted_index(self.schema.file_hash) {
                let mut terms = inverted_index.terms().stream().unwrap();
                while let Some((bytes, _)) = terms.next() {
                    if let Ok(hash) = std::str::from_utf8(bytes) {
                        hashes.push(hash.to_string());
                    }
                }
            }
        }

        hashes
    }

    pub fn prune_stale(
        &mut self,
        valid_hashes: &std::collections::HashSet<String>,
    ) -> Result<usize, IndexError> {
        let indexed = self.indexed_hashes();
        let mut removed = 0;

        for hash in indexed {
            if !valid_hashes.contains(&hash) {
                self.remove_document(&hash)?;
                removed += 1;
            }
        }

        if removed > 0 {
            self.commit()?;
        }

        Ok(removed)
    }

    pub fn stats(&self) -> IndexStats {
        let searcher = self.reader.searcher();
        let doc_count = searcher.num_docs();

        let mut total_size = 0u64;
        if let Ok(entries) = std::fs::read_dir(&self.index_path) {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    total_size += meta.len();
                }
            }
        }

        let segment_count = searcher.segment_readers().len();

        IndexStats {
            document_count: doc_count,
            index_size_bytes: total_size,
            segment_count,
            index_path: self.index_path.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct IndexStats {
    pub document_count: u64,
    pub index_size_bytes: u64,
    pub segment_count: usize,
    pub index_path: PathBuf,
}

impl IndexStats {
    pub fn format_size(&self) -> String {
        let bytes = self.index_size_bytes;
        if bytes < 1024 {
            format!("{} B", bytes)
        } else if bytes < 1024 * 1024 {
            format!("{:.1} KB", bytes as f64 / 1024.0)
        } else if bytes < 1024 * 1024 * 1024 {
            format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExtractionJob {
    pub hash: String,
    pub path: PathBuf,
    pub file_type: FileType,
    pub title: Option<String>,
    pub author: Option<String>,
}

#[derive(Debug)]
pub struct ExtractionResult {
    pub hash: String,
    pub path: PathBuf,
    pub title: Option<String>,
    pub author: Option<String>,
    pub content: String,
}

pub fn extract_parallel(jobs: Vec<ExtractionJob>) -> Vec<ExtractionResult> {
    jobs.into_par_iter()
        .filter_map(|job| {
            let extracted = match job.file_type {
                FileType::Pdf => extract_pdf_text(&job.path).ok(),
                FileType::Epub => extract_epub_text(&job.path).ok(),
                _ => None,
            };

            extracted.and_then(|e| {
                if e.is_empty() {
                    None
                } else {
                    Some(ExtractionResult {
                        hash: job.hash,
                        path: job.path,
                        title: job.title,
                        author: job.author,
                        content: e.content,
                    })
                }
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn create_and_open_index() {
        let temp = TempDir::new().unwrap();
        let lib_path = temp.path();

        {
            let index = SearchIndex::open_or_create(lib_path).unwrap();
            assert_eq!(index.document_count(), 0);
        }

        {
            let index = SearchIndex::open_or_create(lib_path).unwrap();
            assert_eq!(index.document_count(), 0);
        }
    }

    #[test]
    fn add_and_remove_document() {
        let temp = TempDir::new().unwrap();
        let mut index = SearchIndex::open_or_create(temp.path()).unwrap();

        index
            .add_document(
                "hash123",
                Path::new("test.pdf"),
                Some("Test Title"),
                Some("Test Author"),
                "test content here",
            )
            .unwrap();
        index.commit().unwrap();

        assert!(index.contains_hash("hash123"));
        assert!(!index.contains_hash("nonexistent"));

        index.remove_document("hash123").unwrap();
        index.commit().unwrap();

        assert!(!index.contains_hash("hash123"));
    }

    #[test]
    fn clear_index() {
        let temp = TempDir::new().unwrap();
        let mut index = SearchIndex::open_or_create(temp.path()).unwrap();

        index
            .add_document("h1", Path::new("a.pdf"), None, None, "content a")
            .unwrap();
        index
            .add_document("h2", Path::new("b.pdf"), None, None, "content b")
            .unwrap();
        index.commit().unwrap();

        assert_eq!(index.document_count(), 2);

        index.clear().unwrap();
        assert_eq!(index.document_count(), 0);
    }
}
