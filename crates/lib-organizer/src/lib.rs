pub mod classifier;
pub mod compression;
pub mod config;
pub mod git;
pub mod manifest;
pub mod organizer;
pub mod scanner;
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
pub use types::{FileType, LibEntry, Topic};
