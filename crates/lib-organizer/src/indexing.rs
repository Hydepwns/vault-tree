use rayon::prelude::*;
use std::collections::HashSet;
use std::path::Path;

use crate::search::{ExtractionJob, ExtractionResult, IndexError, SearchIndex};
use crate::types::FileType;
use crate::Manifest;

pub fn build_extraction_jobs(
    manifest: &Manifest,
    library: &Path,
    force_reindex: bool,
) -> Vec<ExtractionJob> {
    manifest
        .entries
        .iter()
        .filter(|e| {
            matches!(e.file_type, FileType::Pdf | FileType::Epub)
                && (force_reindex || e.indexed_at.is_none())
        })
        .filter_map(|e| {
            let path = library.join(&e.path);
            if path.exists() {
                Some(ExtractionJob {
                    hash: e.hash.clone(),
                    path,
                    file_type: e.file_type,
                    title: e.title.clone(),
                    author: e.author.clone(),
                })
            } else {
                None
            }
        })
        .collect()
}

pub fn extract_with_progress<F>(jobs: Vec<ExtractionJob>, on_progress: F) -> Vec<ExtractionResult>
where
    F: Fn() + Sync,
{
    use crate::search::{extract_epub_text, extract_pdf_text};

    jobs.into_par_iter()
        .inspect(|_| on_progress())
        .filter_map(|job| {
            let extracted = match job.file_type {
                FileType::Pdf => extract_pdf_text(&job.path).ok(),
                FileType::Epub => extract_epub_text(&job.path).ok(),
                _ => return None,
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

pub fn prune_stale_entries(
    manifest: &Manifest,
    index: &mut SearchIndex,
) -> Result<usize, IndexError> {
    let valid_hashes: HashSet<String> = manifest.entries.iter().map(|e| e.hash.clone()).collect();
    index.prune_stale(&valid_hashes)
}

pub fn index_extracted_documents(
    index: &mut SearchIndex,
    manifest: &mut Manifest,
    manifest_path: &Path,
    docs: Vec<ExtractionResult>,
) -> anyhow::Result<Vec<String>> {
    let mut indexed_hashes = Vec::new();

    for doc in docs {
        if index
            .add_document(
                &doc.hash,
                &doc.path,
                doc.title.as_deref(),
                doc.author.as_deref(),
                &doc.content,
            )
            .is_ok()
        {
            indexed_hashes.push(doc.hash);
        }
    }

    if !indexed_hashes.is_empty() {
        index.commit()?;
        manifest.mark_indexed_batch(&indexed_hashes);
        manifest.save_to(manifest_path)?;
    }

    Ok(indexed_hashes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn build_jobs_empty_manifest() {
        let temp = TempDir::new().unwrap();
        let manifest = Manifest::new(temp.path());
        let jobs = build_extraction_jobs(&manifest, temp.path(), false);
        assert!(jobs.is_empty());
    }
}
