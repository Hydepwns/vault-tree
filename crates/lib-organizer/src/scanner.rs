use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::types::FileType;

#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub recursive: bool,
    pub include_hidden: bool,
    pub file_types: Vec<FileType>,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            recursive: true,
            include_hidden: false,
            file_types: vec![
                FileType::Pdf,
                FileType::Epub,
                FileType::Djvu,
                FileType::Mobi,
                FileType::Chm,
            ],
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScannedFile {
    pub path: PathBuf,
    pub file_type: FileType,
    pub size: u64,
    pub hash: String,
}

impl ScannedFile {
    pub fn filename(&self) -> Option<&str> {
        self.path.file_name().and_then(|n| n.to_str())
    }
}

pub fn scan_directory(path: &Path, options: &ScanOptions) -> anyhow::Result<Vec<ScannedFile>> {
    let walker = match options.recursive {
        true => WalkDir::new(path),
        false => WalkDir::new(path).max_depth(1),
    };

    let is_candidate = |entry: &walkdir::DirEntry| -> bool {
        entry.file_type().is_file() && (options.include_hidden || !is_hidden(entry.path())) && {
            let ft = file_type_from_path(entry.path());
            ft.is_supported() && (options.file_types.is_empty() || options.file_types.contains(&ft))
        }
    };

    let paths: Vec<PathBuf> = walker
        .into_iter()
        .filter_map(Result::ok)
        .filter(is_candidate)
        .map(|e| e.into_path())
        .collect();

    Ok(paths.par_iter().filter_map(|p| scan_file(p).ok()).collect())
}

pub fn scan_files(paths: &[PathBuf]) -> anyhow::Result<Vec<ScannedFile>> {
    Ok(paths.par_iter().filter_map(|p| scan_file(p).ok()).collect())
}

fn scan_file(path: &Path) -> anyhow::Result<ScannedFile> {
    let metadata = std::fs::metadata(path)?;
    let hash = vault_tree_core::hash_file(path)?;
    let file_type = file_type_from_path(path);

    Ok(ScannedFile {
        path: path.to_path_buf(),
        file_type,
        size: metadata.len(),
        hash,
    })
}

fn file_type_from_path(path: &Path) -> FileType {
    path.extension()
        .and_then(|e| e.to_str())
        .map(FileType::from_extension)
        .unwrap_or(FileType::Unknown)
}

fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.starts_with('.'))
        .unwrap_or(false)
}

pub fn find_duplicates(files: &[ScannedFile]) -> Vec<Vec<&ScannedFile>> {
    files
        .iter()
        .fold(
            HashMap::<&str, Vec<&ScannedFile>>::new(),
            |mut acc, file| {
                acc.entry(&file.hash).or_default().push(file);
                acc
            },
        )
        .into_values()
        .filter(|group| group.len() > 1)
        .collect()
}

pub fn format_size(bytes: u64) -> String {
    const UNITS: &[(u64, &str)] = &[
        (1024 * 1024 * 1024, "GB"),
        (1024 * 1024, "MB"),
        (1024, "KB"),
    ];

    UNITS
        .iter()
        .find(|(threshold, _)| bytes >= *threshold)
        .map(|(threshold, unit)| format!("{:.2} {}", bytes as f64 / *threshold as f64, unit))
        .unwrap_or_else(|| format!("{} B", bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn scan_finds_pdfs() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("book.pdf"), b"PDF content").unwrap();
        fs::write(dir.path().join("readme.txt"), b"text").unwrap();

        let files = scan_directory(dir.path(), &ScanOptions::default()).unwrap();

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].file_type, FileType::Pdf);
    }

    #[test]
    fn scan_excludes_hidden() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("book.pdf"), b"PDF content").unwrap();
        fs::write(dir.path().join(".hidden.pdf"), b"hidden").unwrap();

        let files = scan_directory(dir.path(), &ScanOptions::default()).unwrap();

        assert_eq!(files.len(), 1);
    }

    #[test]
    fn find_duplicates_groups_by_hash() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.pdf"), b"same content").unwrap();
        fs::write(dir.path().join("b.pdf"), b"same content").unwrap();
        fs::write(dir.path().join("c.pdf"), b"different").unwrap();

        let files = scan_directory(dir.path(), &ScanOptions::default()).unwrap();
        let dupes = find_duplicates(&files);

        assert_eq!(dupes.len(), 1);
        assert_eq!(dupes[0].len(), 2);
    }

    #[test]
    fn format_size_display() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(1024 * 1024), "1.00 MB");
        assert_eq!(format_size(1024 * 1024 * 1024), "1.00 GB");
    }
}
