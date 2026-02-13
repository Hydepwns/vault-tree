use git2::{IndexAddOption, Repository, Signature, Status};
use std::path::{Path, PathBuf};

pub struct GitOps {
    repo: Repository,
    root: PathBuf,
}

impl GitOps {
    pub fn init(path: &Path) -> anyhow::Result<Self> {
        let repo = Repository::init(path)?;
        Ok(Self {
            repo,
            root: path.to_path_buf(),
        })
    }

    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let repo = Repository::open(path)?;
        Ok(Self {
            repo,
            root: path.to_path_buf(),
        })
    }

    pub fn add_paths(&self, paths: &[PathBuf]) -> anyhow::Result<()> {
        let mut index = self.repo.index()?;

        paths
            .iter()
            .map(|p| p.strip_prefix(&self.root).unwrap_or(p))
            .try_for_each(|relative| index.add_path(relative))?;

        index.write().map_err(Into::into)
    }

    pub fn add_all(&self) -> anyhow::Result<()> {
        let mut index = self.repo.index()?;
        index.add_all(["*"].iter(), IndexAddOption::DEFAULT, None)?;
        index.write().map_err(Into::into)
    }

    pub fn commit(&self, message: &str) -> anyhow::Result<()> {
        let mut index = self.repo.index()?;
        let oid = index.write_tree()?;
        let tree = self.repo.find_tree(oid)?;
        let sig = self.default_signature()?;

        let parents: Vec<_> = self
            .repo
            .head()
            .ok()
            .and_then(|h| h.peel_to_commit().ok())
            .into_iter()
            .collect();

        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

        self.repo
            .commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)?;

        Ok(())
    }

    fn default_signature(&self) -> anyhow::Result<Signature<'static>> {
        self.repo
            .signature()
            .ok()
            .and_then(|sig| {
                Signature::now(
                    sig.name().unwrap_or("lib-organizer"),
                    sig.email().unwrap_or("lib-organizer@local"),
                )
                .ok()
            })
            .map(Ok)
            .unwrap_or_else(|| {
                Signature::now("lib-organizer", "lib-organizer@local").map_err(Into::into)
            })
    }

    pub fn status_summary(&self) -> String {
        self.repo
            .statuses(None)
            .map(|statuses| summarize_statuses(&statuses))
            .unwrap_or_else(|_| "error getting status".to_string())
    }

    pub fn has_uncommitted_changes(&self) -> anyhow::Result<bool> {
        self.repo
            .statuses(None)
            .map(|s| !s.is_empty())
            .map_err(Into::into)
    }

    pub fn head_commit_message(&self) -> Option<String> {
        self.repo
            .head()
            .ok()?
            .peel_to_commit()
            .ok()?
            .message()
            .map(String::from)
    }
}

#[derive(Default)]
struct StatusCounts {
    staged: usize,
    modified: usize,
    new: usize,
}

fn summarize_statuses(statuses: &git2::Statuses) -> String {
    if statuses.is_empty() {
        return "clean".to_string();
    }

    let counts = statuses
        .iter()
        .fold(StatusCounts::default(), |mut acc, entry| {
            let status = entry.status();
            if status.intersects(Status::INDEX_NEW | Status::INDEX_MODIFIED) {
                acc.staged += 1;
            }
            if status.contains(Status::WT_MODIFIED) {
                acc.modified += 1;
            }
            if status.contains(Status::WT_NEW) {
                acc.new += 1;
            }
            acc
        });

    let parts: Vec<String> = [
        (counts.staged, "staged"),
        (counts.modified, "modified"),
        (counts.new, "new"),
    ]
    .into_iter()
    .filter(|(count, _)| *count > 0)
    .map(|(count, label)| format!("{} {}", count, label))
    .collect();

    if parts.is_empty() {
        "clean".to_string()
    } else {
        parts.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn init_creates_repo() {
        let dir = TempDir::new().unwrap();
        GitOps::init(dir.path()).unwrap();
        assert!(dir.path().join(".git").exists());
    }

    #[test]
    fn add_and_commit() {
        let dir = TempDir::new().unwrap();
        let git = GitOps::init(dir.path()).unwrap();

        fs::write(dir.path().join("test.txt"), "content").unwrap();

        git.add_paths(&[dir.path().join("test.txt")]).unwrap();
        git.commit("initial commit").unwrap();

        let msg = git.head_commit_message().unwrap();
        assert_eq!(msg, "initial commit");
    }

    #[test]
    fn status_clean_after_commit() {
        let dir = TempDir::new().unwrap();
        let git = GitOps::init(dir.path()).unwrap();

        fs::write(dir.path().join("test.txt"), "content").unwrap();
        git.add_all().unwrap();
        git.commit("commit").unwrap();

        assert_eq!(git.status_summary(), "clean");
    }

    #[test]
    fn status_shows_changes() {
        let dir = TempDir::new().unwrap();
        let git = GitOps::init(dir.path()).unwrap();

        fs::write(dir.path().join("test.txt"), "content").unwrap();

        let status = git.status_summary();
        assert!(status.contains("new"));
    }
}
