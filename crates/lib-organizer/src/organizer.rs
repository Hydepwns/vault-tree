use std::path::{Path, PathBuf};

use crate::classifier::{classify_file, ClassificationResult};
use crate::compression::{compress_file, compressed_path};
use crate::config::Config;
use crate::git::GitOps;
use crate::manifest::Manifest;
use crate::scanner::ScannedFile;
use crate::types::{LibEntry, Topic};

/// Immutable library handle for creating sessions
pub struct Library {
    config: Config,
    git: GitOps,
}

impl Library {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        Ok(Self {
            config: Config::new(path),
            git: GitOps::open(path)?,
        })
    }

    pub fn init(path: &Path) -> anyhow::Result<Self> {
        std::fs::create_dir_all(path)?;

        let config = Config::new(path);

        config
            .default_topics
            .iter()
            .map(|topic| config.topic_path(topic))
            .try_for_each(std::fs::create_dir_all)?;

        let manifest = Manifest::new(config.manifest_path());
        manifest.save_to(&config.manifest_path())?;

        let git = GitOps::init(path)?;

        let gitignore_path = path.join(".gitignore");
        if !gitignore_path.exists() {
            std::fs::write(&gitignore_path, "# Library gitignore\n*.tmp\n")?;
        }

        git.add_paths(&[config.manifest_path(), gitignore_path])?;
        git.commit("Initialize library")?;

        Ok(Self { config, git })
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn load_manifest(&self) -> anyhow::Result<Manifest> {
        Manifest::load_or_create(&self.config.manifest_path())
    }

    pub fn session(&self) -> anyhow::Result<IngestSession> {
        let manifest = self.load_manifest()?;
        Ok(IngestSession::new(&self.config, manifest))
    }

    pub fn commit_session(
        &self,
        session: IngestSession,
        message: &str,
    ) -> anyhow::Result<Manifest> {
        session.commit(&self.git, &self.config, message)
    }

    pub fn status(&self) -> anyhow::Result<LibraryStatus> {
        let manifest = self.load_manifest()?;
        let by_topic = manifest.count_by_topic();
        let topics = by_topic
            .into_iter()
            .map(|(t, c)| (t.to_string(), c))
            .collect();

        Ok(LibraryStatus {
            total_files: manifest.count(),
            total_size: manifest.total_size(),
            topics,
            git_status: self.git.status_summary(),
        })
    }
}

/// Accumulated ingest operations, consumed on commit
pub struct IngestSession {
    manifest: Manifest,
    pending: Vec<PendingIngest>,
}

struct PendingIngest {
    #[allow(dead_code)]
    entry: LibEntry,
    final_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct IngestOptions {
    pub topic: Option<Topic>,
    pub subtopic: Option<String>,
    pub compress: bool,
    pub move_file: bool,
}

impl Default for IngestOptions {
    fn default() -> Self {
        Self {
            topic: None,
            subtopic: None,
            compress: false,
            move_file: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IngestResult {
    pub entry: LibEntry,
    pub classification: ClassificationResult,
    pub compressed_size: Option<u64>,
}

impl IngestSession {
    fn new(_config: &Config, manifest: Manifest) -> Self {
        Self {
            manifest,
            pending: Vec::new(),
        }
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Plan and execute an ingest, returning updated session
    pub fn ingest(
        self,
        file: &ScannedFile,
        config: &Config,
        options: &IngestOptions,
    ) -> anyhow::Result<(Self, IngestResult)> {
        let plan = plan_ingest(file, &self.manifest, config, options)?;
        let (final_path, compressed_size) = execute_ingest(&plan)?;

        let entry = build_entry(&plan, &final_path, compressed_size, config);

        let manifest = self.manifest.with_entry(entry.clone());
        let pending = self
            .pending
            .into_iter()
            .chain(std::iter::once(PendingIngest {
                entry: entry.clone(),
                final_path,
            }))
            .collect();

        let result = IngestResult {
            entry,
            classification: plan.classification,
            compressed_size,
        };

        Ok((Self { manifest, pending }, result))
    }

    fn commit(self, git: &GitOps, config: &Config, message: &str) -> anyhow::Result<Manifest> {
        self.manifest.save_to(&config.manifest_path())?;

        let paths: Vec<PathBuf> = std::iter::once(config.manifest_path())
            .chain(self.pending.into_iter().map(|p| p.final_path))
            .collect();

        git.add_paths(&paths)?;
        git.commit(message)?;

        Ok(self.manifest)
    }
}

/// Pure planning: determines what should happen without side effects
struct IngestPlan {
    file: ScannedFile,
    classification: ClassificationResult,
    topic: Topic,
    subtopic: Option<String>,
    target_path: PathBuf,
    compress: bool,
    move_file: bool,
}

fn plan_ingest(
    file: &ScannedFile,
    manifest: &Manifest,
    config: &Config,
    options: &IngestOptions,
) -> anyhow::Result<IngestPlan> {
    if manifest.contains_hash(&file.hash) {
        anyhow::bail!("file already in library: {}", file.hash);
    }

    let classification = classify_file(&file.path, file.file_type, config)?;

    let topic = options
        .topic
        .clone()
        .unwrap_or_else(|| classification.topic.clone());

    let subtopic = options
        .subtopic
        .clone()
        .or_else(|| classification.subtopic.clone());

    let target_dir = subtopic
        .as_ref()
        .map(|sub| config.subtopic_path(&topic, sub))
        .unwrap_or_else(|| config.topic_path(&topic));

    let filename = file
        .path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("no filename"))?;

    let target_path = target_dir.join(filename);

    let compress = options.compress && file.size >= config.compression.min_size_bytes;

    Ok(IngestPlan {
        file: file.clone(),
        classification,
        topic,
        subtopic,
        target_path,
        compress,
        move_file: options.move_file,
    })
}

/// Execute the plan: performs file operations
fn execute_ingest(plan: &IngestPlan) -> anyhow::Result<(PathBuf, Option<u64>)> {
    let target_dir = plan
        .target_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("no parent directory"))?;

    std::fs::create_dir_all(target_dir)?;

    if plan.compress {
        let compressed_target = compressed_path(&plan.target_path);
        let size = compress_file(&plan.file.path, &compressed_target, 3)?;

        if plan.move_file {
            std::fs::remove_file(&plan.file.path)?;
        }

        Ok((compressed_target, Some(size)))
    } else if plan.move_file {
        std::fs::rename(&plan.file.path, &plan.target_path)?;
        Ok((plan.target_path.clone(), None))
    } else {
        std::fs::copy(&plan.file.path, &plan.target_path)?;
        Ok((plan.target_path.clone(), None))
    }
}

fn build_entry(
    plan: &IngestPlan,
    final_path: &Path,
    compressed_size: Option<u64>,
    config: &Config,
) -> LibEntry {
    let relative_path = final_path
        .strip_prefix(&config.library_path)
        .unwrap_or(final_path)
        .to_path_buf();

    let entry = LibEntry::new(
        relative_path,
        plan.file.path.clone(),
        plan.file.hash.clone(),
        plan.file.file_type,
        plan.file.size,
        plan.topic.clone(),
    )
    .with_compression(compressed_size.is_some());

    let entry = plan
        .subtopic
        .as_ref()
        .map(|s| entry.clone().with_subtopic(s))
        .unwrap_or(entry);

    let entry = plan
        .classification
        .metadata
        .title
        .as_ref()
        .map(|t| entry.clone().with_title(t))
        .unwrap_or(entry);

    plan.classification
        .metadata
        .author
        .as_ref()
        .map(|a| entry.clone().with_author(a))
        .unwrap_or(entry)
}

#[derive(Debug, Clone)]
pub struct LibraryStatus {
    pub total_files: usize,
    pub total_size: u64,
    pub topics: Vec<(String, usize)>,
    pub git_status: String,
}

// Backwards compatibility: Organizer wraps Library with stateful session
pub struct Organizer {
    library: Library,
    session: Option<IngestSession>,
}

impl Organizer {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let library = Library::open(path)?;
        let session = Some(library.session()?);
        Ok(Self { library, session })
    }

    pub fn init(path: &Path) -> anyhow::Result<Self> {
        let library = Library::init(path)?;
        let session = Some(library.session()?);
        Ok(Self { library, session })
    }

    pub fn config(&self) -> &Config {
        self.library.config()
    }

    pub fn manifest(&self) -> &Manifest {
        self.session
            .as_ref()
            .map(|s| s.manifest())
            .unwrap_or_else(|| panic!("session consumed"))
    }

    pub fn ingest(
        &mut self,
        file: &ScannedFile,
        options: &IngestOptions,
    ) -> anyhow::Result<IngestResult> {
        let session = self
            .session
            .take()
            .ok_or_else(|| anyhow::anyhow!("session consumed"))?;
        let (new_session, result) = session.ingest(file, self.library.config(), options)?;
        self.session = Some(new_session);
        Ok(result)
    }

    pub fn save(&mut self) -> anyhow::Result<()> {
        if let Some(ref session) = self.session {
            session
                .manifest()
                .save_to(&self.library.config().manifest_path())?;
        }
        Ok(())
    }

    pub fn commit(&mut self, message: &str) -> anyhow::Result<()> {
        let session = self
            .session
            .take()
            .ok_or_else(|| anyhow::anyhow!("session consumed"))?;
        let _manifest = self.library.commit_session(session, message)?;
        self.session = Some(self.library.session()?);
        Ok(())
    }

    pub fn status(&self) -> LibraryStatus {
        self.library.status().unwrap_or_else(|_| LibraryStatus {
            total_files: 0,
            total_size: 0,
            topics: vec![],
            git_status: "error".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::scan_directory;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn init_creates_structure() {
        let dir = TempDir::new().unwrap();
        let lib_path = dir.path().join("lib");

        Library::init(&lib_path).unwrap();

        assert!(lib_path.join("manifest.json").exists());
        assert!(lib_path.join(".git").exists());
        assert!(lib_path.join("programming").exists());
        assert!(lib_path.join("electronics").exists());
    }

    #[test]
    fn session_ingest_file() {
        let dir = TempDir::new().unwrap();
        let lib_path = dir.path().join("lib");
        let source_dir = dir.path().join("source");
        fs::create_dir(&source_dir).unwrap();
        fs::write(source_dir.join("rust_book.pdf"), b"PDF content").unwrap();

        let library = Library::init(&lib_path).unwrap();
        let session = library.session().unwrap();

        let files = scan_directory(&source_dir, &Default::default()).unwrap();
        let options = IngestOptions {
            move_file: false,
            ..Default::default()
        };

        let (session, result) = session
            .ingest(&files[0], library.config(), &options)
            .unwrap();

        assert_eq!(result.entry.topic, Topic::new("programming"));
        assert!(session.manifest().contains_hash(&files[0].hash));
    }

    #[test]
    fn session_ingest_with_explicit_topic() {
        let dir = TempDir::new().unwrap();
        let lib_path = dir.path().join("lib");
        let source_dir = dir.path().join("source");
        fs::create_dir(&source_dir).unwrap();
        fs::write(source_dir.join("random.pdf"), b"PDF content").unwrap();

        let library = Library::init(&lib_path).unwrap();
        let session = library.session().unwrap();
        let files = scan_directory(&source_dir, &Default::default()).unwrap();

        let options = IngestOptions {
            topic: Some(Topic::new("philosophy")),
            move_file: false,
            ..Default::default()
        };

        let (_session, result) = session
            .ingest(&files[0], library.config(), &options)
            .unwrap();

        assert_eq!(result.entry.topic, Topic::new("philosophy"));
    }

    #[test]
    fn session_rejects_duplicate() {
        let dir = TempDir::new().unwrap();
        let lib_path = dir.path().join("lib");
        let source_dir = dir.path().join("source");
        fs::create_dir(&source_dir).unwrap();
        fs::write(source_dir.join("book.pdf"), b"PDF content").unwrap();

        let library = Library::init(&lib_path).unwrap();
        let session = library.session().unwrap();
        let files = scan_directory(&source_dir, &Default::default()).unwrap();

        let options = IngestOptions {
            move_file: false,
            ..Default::default()
        };

        let (session, _) = session
            .ingest(&files[0], library.config(), &options)
            .unwrap();
        let result = session.ingest(&files[0], library.config(), &options);

        assert!(result.is_err());
    }

    // Backwards compatibility tests
    #[test]
    fn organizer_ingest_file() {
        let dir = TempDir::new().unwrap();
        let lib_path = dir.path().join("lib");
        let source_dir = dir.path().join("source");
        fs::create_dir(&source_dir).unwrap();
        fs::write(source_dir.join("rust_book.pdf"), b"PDF content").unwrap();

        let mut organizer = Organizer::init(&lib_path).unwrap();
        let files = scan_directory(&source_dir, &Default::default()).unwrap();

        let options = IngestOptions {
            move_file: false,
            ..Default::default()
        };

        let result = organizer.ingest(&files[0], &options).unwrap();

        assert_eq!(result.entry.topic, Topic::new("programming"));
        assert!(organizer.manifest().contains_hash(&files[0].hash));
    }
}
