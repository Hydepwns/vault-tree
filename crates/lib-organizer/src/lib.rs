pub mod classifier;
pub mod compression;
pub mod config;
pub mod git;
pub mod indexing;
pub mod manifest;
pub mod organizer;
pub mod scanner;
pub mod search;
pub mod secrets;
pub mod types;

pub use classifier::{classify_file, ClassificationResult, Classifier, Confidence};
pub use compression::{compress_file, decompress_file};
pub use config::Config;
pub use git::GitOps;
pub use manifest::Manifest;
pub use organizer::{
    IngestOptions, IngestResult, IngestSession, Library, LibraryStatus, Organizer,
};
pub use scanner::{
    find_duplicates, format_size, scan_directory, scan_files, ScanOptions, ScannedFile,
};
pub use search::{
    extract_epub_text, extract_parallel, extract_pdf_text, format_search_results, ExtractedText,
    ExtractionJob, ExtractionResult, IndexStats, SearchIndex, SearchOptions, SearchResult,
};
pub use secrets::{
    format_results as format_secrets_results, scan_files_for_secrets, scan_for_secrets,
    ScanOptions as SecretsScanOptions, SecretType, SensitiveFile, Severity,
};
pub use types::{FileType, LibEntry, Topic};
