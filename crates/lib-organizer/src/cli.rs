use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use indicatif::{ProgressBar, ProgressStyle};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

const TICK_MS: u64 = 80;

fn spinner_style() -> ProgressStyle {
    ProgressStyle::default_spinner()
        .template(" {spinner} {msg}")
        .unwrap()
        .tick_chars("▏▎▍▌▋▊▉█▉▋▌▍▎")
}

fn bar_style() -> ProgressStyle {
    ProgressStyle::default_bar()
        .template(":: {spinner} {msg:<16} ━{bar:30}━ {pos}/{len} | ETA {eta}")
        .unwrap()
        .tick_chars("▏▎▍▌▋▊▉█▉▋▌▍▎")
        .progress_chars("━━░")
}

use lib_organizer::{
    classify_file, find_duplicates, format_search_results, format_secrets_results, format_size,
    scan_directory, scan_for_secrets, FileType, Manifest, Organizer, ScanOptions, SearchIndex,
    SearchOptions, SecretsScanOptions, Topic,
};

#[derive(Parser)]
#[command(name = "lib-organizer")]
#[command(version)]
#[command(about = "Organize books and manuals into a structured library")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init {
        #[arg(help = "Path to create the library")]
        path: PathBuf,
    },
    Scan {
        #[arg(short, long, help = "Directories to scan [default: current dir]")]
        dirs: Vec<PathBuf>,
        #[arg(short, long, help = "Top-level only, skip subdirectories")]
        flat: bool,
    },
    Duplicates {
        #[arg(short, long, help = "Directories to scan [default: current dir]")]
        dirs: Vec<PathBuf>,
    },
    Classify {
        #[arg(help = "File to classify")]
        file: PathBuf,
        #[arg(short, long, help = "Library for keyword rules")]
        library: Option<PathBuf>,
    },
    Ingest {
        #[arg(help = "Files to ingest")]
        files: Vec<PathBuf>,
        #[arg(short, long, help = "Topic (e.g. programming, research, security)")]
        topic: Option<String>,
        #[arg(short, long, help = "Subtopic within topic")]
        subtopic: Option<String>,
        #[arg(short, long, help = "Compress with zstd")]
        compress: bool,
        #[arg(short, long, help = "Library path")]
        library: PathBuf,
        #[arg(long, help = "Copy instead of move")]
        copy: bool,
        #[arg(short, long, help = "Git commit message")]
        message: Option<String>,
    },
    Search {
        #[arg(help = "Search query")]
        query: String,
        #[arg(short, long, help = "Library path")]
        library: PathBuf,
        #[arg(short, long, help = "Search PDF/EPUB content (not just metadata)")]
        fulltext: bool,
        #[arg(long, help = "Rebuild search index from scratch")]
        rebuild_index: bool,
        #[arg(
            short = 'n',
            long,
            default_value = "20",
            help = "Max results [default: 20]"
        )]
        limit: usize,
        #[arg(long, help = "Allow typos in search terms")]
        fuzzy: bool,
    },
    Status {
        #[arg(short, long, help = "Library path")]
        library: PathBuf,
    },
    Secrets {
        #[arg(help = "Directories to scan [default: current dir]")]
        dirs: Vec<PathBuf>,
        #[arg(long, help = "Also check file contents")]
        content: bool,
        #[arg(long, help = "Exit with error if secrets found")]
        strict: bool,
    },
    Index {
        #[arg(short, long, help = "Library path")]
        library: PathBuf,
        #[arg(long, help = "Show index statistics")]
        stats: bool,
        #[arg(long, help = "Rebuild index from scratch")]
        rebuild: bool,
    },
    /// Watch directories and auto-ingest new files
    Watch {
        #[arg(help = "Directories to watch")]
        dirs: Vec<PathBuf>,
        #[arg(short, long, help = "Library path")]
        library: PathBuf,
        #[arg(short, long, help = "Topic to assign")]
        topic: Option<String>,
        #[arg(long, help = "Copy instead of move")]
        copy: bool,
    },
    /// Generate shell completions
    Completions {
        #[arg(help = "Shell to generate for (bash, zsh, fish, powershell)")]
        shell: Shell,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { path } => cmd_init(&path),
        Commands::Scan { dirs, flat } => cmd_scan(&dirs, flat),
        Commands::Duplicates { dirs } => cmd_duplicates(&dirs),
        Commands::Classify { file, library } => cmd_classify(&file, library.as_deref()),
        Commands::Ingest {
            files,
            topic,
            subtopic,
            compress,
            library,
            copy,
            message,
        } => cmd_ingest(&files, topic, subtopic, compress, &library, copy, message),
        Commands::Search {
            query,
            library,
            fulltext,
            rebuild_index,
            limit,
            fuzzy,
        } => cmd_search(&query, &library, fulltext, rebuild_index, limit, fuzzy),
        Commands::Status { library } => cmd_status(&library),
        Commands::Secrets {
            dirs,
            content,
            strict,
        } => cmd_secrets(&dirs, content, strict),
        Commands::Index {
            library,
            stats,
            rebuild,
        } => cmd_index(&library, stats, rebuild),
        Commands::Watch {
            dirs,
            library,
            topic,
            copy,
        } => cmd_watch(&dirs, &library, topic.as_deref(), copy),
        Commands::Completions { shell } => {
            generate(
                shell,
                &mut Cli::command(),
                "lib-organizer",
                &mut io::stdout(),
            );
            Ok(())
        }
    }
}

fn cmd_init(path: &Path) -> Result<()> {
    let mut organizer = Organizer::init(path)?;
    organizer.commit("Initialize library")?;

    println!("Initialized library at {}", path.display());
    println!("Created topics:");
    for topic in organizer.config().default_topics.iter() {
        println!("  - {}", topic);
    }

    Ok(())
}

fn cmd_scan(dirs: &[PathBuf], flat: bool) -> Result<()> {
    let dirs = if dirs.is_empty() {
        eprintln!("Scanning current directory...");
        vec![std::env::current_dir()?]
    } else {
        dirs.to_vec()
    };

    let options = ScanOptions {
        recursive: !flat,
        ..Default::default()
    };

    let pb = ProgressBar::new_spinner();
    pb.set_style(spinner_style());
    pb.enable_steady_tick(Duration::from_millis(TICK_MS));

    let mut all_files = Vec::new();
    for dir in &dirs {
        pb.set_message(format!("Scanning {}", dir.display()));
        let files = scan_directory(dir, &options)?;
        all_files.extend(files);
    }

    pb.finish_and_clear();

    println!("Found {} files:\n", all_files.len());

    let mut total_size = 0u64;
    for file in &all_files {
        let filename = file.filename().unwrap_or("?");
        println!(
            "  {:>10}  {:>4}  {}",
            format_size(file.size),
            file.file_type,
            filename
        );
        total_size += file.size;
    }

    println!(
        "\nTotal: {} in {} files",
        format_size(total_size),
        all_files.len()
    );

    Ok(())
}

fn cmd_duplicates(dirs: &[PathBuf]) -> Result<()> {
    let dirs = if dirs.is_empty() {
        eprintln!("Scanning current directory...");
        vec![std::env::current_dir()?]
    } else {
        dirs.to_vec()
    };

    let options = ScanOptions::default();

    let mut all_files = Vec::new();
    for dir in &dirs {
        let files = scan_directory(dir, &options)?;
        all_files.extend(files);
    }

    let dupes = find_duplicates(&all_files);

    if dupes.is_empty() {
        println!("No duplicates found.");
        return Ok(());
    }

    println!("Found {} duplicate groups:\n", dupes.len());

    for (i, group) in dupes.iter().enumerate() {
        println!("Group {} ({}):", i + 1, format_size(group[0].size));
        for file in group {
            println!("  {}", file.path.display());
        }
        println!();
    }

    Ok(())
}

fn cmd_classify(file: &Path, library: Option<&Path>) -> Result<()> {
    let config = if let Some(lib) = library {
        let organizer = Organizer::open(lib)?;
        organizer.config().clone()
    } else {
        lib_organizer::Config::default()
    };

    let file_type = file
        .extension()
        .and_then(|e| e.to_str())
        .map(lib_organizer::FileType::from_extension)
        .unwrap_or(lib_organizer::FileType::Unknown);

    let result = classify_file(file, file_type, &config)?;

    println!("File: {}", file.display());
    println!("Topic: {}", result.topic);
    if let Some(sub) = &result.subtopic {
        println!("Subtopic: {}", sub);
    }
    println!("Confidence: {}", result.confidence);

    if !result.matched_keywords.is_empty() {
        println!("Matched keywords: {}", result.matched_keywords.join(", "));
    }

    if let Some(title) = &result.metadata.title {
        println!("Title: {}", title);
    }
    if let Some(author) = &result.metadata.author {
        println!("Author: {}", author);
    }

    Ok(())
}

fn cmd_ingest(
    files: &[PathBuf],
    topic: Option<String>,
    subtopic: Option<String>,
    compress: bool,
    library: &Path,
    copy: bool,
    message: Option<String>,
) -> Result<()> {
    let mut organizer = Organizer::open(library)?;

    let scanned = lib_organizer::scan_files(files)?;

    let pb = ProgressBar::new(scanned.len() as u64);
    pb.set_style(bar_style());
    pb.enable_steady_tick(Duration::from_millis(TICK_MS));

    let options = lib_organizer::IngestOptions {
        topic: topic.map(Topic::from),
        subtopic,
        compress,
        move_file: !copy,
    };

    let mut ingested = 0;
    for file in &scanned {
        pb.set_message(file.filename().unwrap_or("?").to_string());

        match organizer.ingest(file, &options) {
            Ok(result) => {
                ingested += 1;
                let size_info = if let Some(compressed) = result.compressed_size {
                    format!(" (compressed: {})", format_size(compressed))
                } else {
                    String::new()
                };
                pb.println(format!(
                    "  [+] {} -> {}/{}{}",
                    file.filename().unwrap_or("?"),
                    result.entry.topic,
                    result.entry.subtopic.as_deref().unwrap_or(""),
                    size_info
                ));
            }
            Err(e) => {
                pb.println(format!("  [!] {}: {}", file.filename().unwrap_or("?"), e));
            }
        }

        pb.inc(1);
    }

    pb.finish_and_clear();

    if ingested > 0 {
        let msg = message.unwrap_or_else(|| format!("Ingest {} files", ingested));
        organizer.commit(&msg)?;
        println!("Ingested {} files, committed: {}", ingested, msg);
    } else {
        println!("No files ingested.");
    }

    Ok(())
}

fn cmd_search(
    query: &str,
    library: &Path,
    fulltext: bool,
    rebuild_index: bool,
    limit: usize,
    fuzzy: bool,
) -> Result<()> {
    if fulltext {
        cmd_fulltext_search(query, library, rebuild_index, limit, fuzzy)
    } else {
        cmd_metadata_search(query, library)
    }
}

fn cmd_metadata_search(query: &str, library: &Path) -> Result<()> {
    let manifest = Manifest::load(&library.join("manifest.json"))?;
    let results = manifest.search(query);

    if results.is_empty() {
        println!("No matches for '{}'.", query);
        return Ok(());
    }

    println!("Found {} matches for '{}':\n", results.len(), query);

    for entry in results {
        println!("  {}", entry.path.display());
        if let Some(title) = &entry.title {
            println!("    Title: {}", title);
        }
        if let Some(author) = &entry.author {
            println!("    Author: {}", author);
        }
        println!("    Topic: {}", entry.topic);
        println!("    Size: {}", format_size(entry.size));
        println!();
    }

    Ok(())
}

fn cmd_fulltext_search(
    query: &str,
    library: &Path,
    rebuild_index: bool,
    limit: usize,
    fuzzy: bool,
) -> Result<()> {
    let manifest_path = library.join("manifest.json");
    let mut manifest = Manifest::load(&manifest_path)?;
    let mut index = SearchIndex::open_or_create(library)?;

    if rebuild_index {
        index.clear()?;
    }

    let pruned = lib_organizer::indexing::prune_stale_entries(&manifest, &mut index)?;
    if pruned > 0 {
        println!("Removed {} stale index entries.\n", pruned);
    }

    let jobs = lib_organizer::indexing::build_extraction_jobs(&manifest, library, rebuild_index);

    let indexed_count = if !jobs.is_empty() {
        let pb = ProgressBar::new(jobs.len() as u64);
        pb.set_style(bar_style());
        pb.set_message("Extracting");
        pb.enable_steady_tick(Duration::from_millis(TICK_MS));

        let results = lib_organizer::indexing::extract_with_progress(jobs, || pb.inc(1));
        pb.finish_and_clear();

        let indexed = lib_organizer::indexing::index_extracted_documents(
            &mut index,
            &mut manifest,
            &manifest_path,
            results,
        )?;
        indexed.len()
    } else {
        0
    };

    if indexed_count > 0 {
        println!("Indexed {} new documents.\n", indexed_count);
    }

    let options = SearchOptions {
        limit,
        fuzzy,
        ..Default::default()
    };

    let results = index.search(query, &options)?;
    print!("{}", format_search_results(&results, query));

    Ok(())
}

fn cmd_status(library: &Path) -> Result<()> {
    let organizer = Organizer::open(library)?;
    let status = organizer.status();

    println!("Library: {}", library.display());
    println!("Total files: {}", status.total_files);
    println!("Total size: {}", format_size(status.total_size));
    println!("Git status: {}", status.git_status);
    println!("\nBy topic:");

    let mut topics = status.topics;
    topics.sort_by(|a, b| b.1.cmp(&a.1));

    for (topic, count) in topics {
        println!("  {}: {}", topic, count);
    }

    Ok(())
}

fn cmd_secrets(dirs: &[PathBuf], check_content: bool, strict: bool) -> Result<()> {
    let dirs = if dirs.is_empty() {
        eprintln!("Scanning current directory...");
        vec![std::env::current_dir()?]
    } else {
        dirs.to_vec()
    };

    let options = SecretsScanOptions {
        check_content,
        max_file_size: 1024 * 1024,
        include_hidden: true,
    };

    let mut all_results = Vec::new();

    for dir in &dirs {
        println!("Scanning {}...", dir.display());
        let results = scan_for_secrets(dir, &options);
        all_results.extend(results);
    }

    if all_results.is_empty() {
        println!("\nNo sensitive files detected.");
        return Ok(());
    }

    println!("\n{}", format_secrets_results(&all_results));

    let critical_count = all_results
        .iter()
        .filter(|r| r.severity() == lib_organizer::Severity::Critical)
        .count();

    if strict && !all_results.is_empty() {
        anyhow::bail!(
            "Found {} sensitive file(s) ({} critical). Remove --strict to continue.",
            all_results.len(),
            critical_count
        );
    }

    Ok(())
}

fn cmd_index(library: &Path, stats: bool, rebuild: bool) -> Result<()> {
    let manifest_path = library.join("manifest.json");
    let mut manifest = Manifest::load(&manifest_path)?;
    let mut index = SearchIndex::open_or_create(library)?;

    if stats {
        let index_stats = index.stats();
        println!("Search Index Statistics");
        println!("-----------------------");
        println!("Path: {}", index_stats.index_path.display());
        println!("Documents: {}", index_stats.document_count);
        println!("Size: {}", index_stats.format_size());
        println!("Segments: {}", index_stats.segment_count);

        let indexed_count = manifest
            .entries
            .iter()
            .filter(|e| e.indexed_at.is_some())
            .count();
        let indexable_count = manifest
            .entries
            .iter()
            .filter(|e| matches!(e.file_type, FileType::Pdf | FileType::Epub))
            .count();

        println!("\nManifest Status");
        println!("---------------");
        println!("Indexed entries: {}/{}", indexed_count, indexable_count);

        return Ok(());
    }

    if rebuild {
        println!("Rebuilding index...");
        index.clear()?;
    }

    let pruned = lib_organizer::indexing::prune_stale_entries(&manifest, &mut index)?;
    if pruned > 0 {
        println!("Removed {} stale entries.", pruned);
    }

    let jobs = lib_organizer::indexing::build_extraction_jobs(&manifest, library, rebuild);

    if jobs.is_empty() {
        println!("Index is up to date.");
        return Ok(());
    }

    let pb = ProgressBar::new(jobs.len() as u64);
    pb.set_style(bar_style());
    pb.set_message("Extracting");
    pb.enable_steady_tick(Duration::from_millis(TICK_MS));

    let results = lib_organizer::indexing::extract_with_progress(jobs, || pb.inc(1));
    pb.finish_and_clear();

    if results.is_empty() {
        println!("No text extracted from files.");
        return Ok(());
    }

    println!("Adding {} documents to index...", results.len());

    let indexed = lib_organizer::indexing::index_extracted_documents(
        &mut index,
        &mut manifest,
        &manifest_path,
        results,
    )?;

    if !indexed.is_empty() {
        println!("Indexed {} documents.", indexed.len());
    }

    Ok(())
}

fn cmd_watch(dirs: &[PathBuf], library: &Path, topic: Option<&str>, copy: bool) -> Result<()> {
    let dirs = if dirs.is_empty() {
        eprintln!("Watching current directory...");
        vec![std::env::current_dir()?]
    } else {
        dirs.to_vec()
    };

    let mut organizer = Organizer::open(library)?;

    let (tx, rx) = mpsc::channel();

    let mut watcher = RecommendedWatcher::new(tx, Config::default())?;

    for dir in &dirs {
        println!("Watching: {}", dir.display());
        watcher.watch(dir, RecursiveMode::Recursive)?;
    }

    println!("\nWaiting for new files... (Ctrl+C to stop)\n");

    let options = lib_organizer::IngestOptions {
        topic: topic.map(Topic::from),
        subtopic: None,
        compress: false,
        move_file: !copy,
    };

    for res in rx {
        match res {
            Ok(event) => {
                if !matches!(
                    event.kind,
                    notify::EventKind::Create(_) | notify::EventKind::Modify(_)
                ) {
                    continue;
                }

                for path in event.paths {
                    if !path.is_file() {
                        continue;
                    }

                    let ext = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|s| s.to_lowercase());

                    let is_supported = matches!(
                        ext.as_deref(),
                        Some("pdf") | Some("epub") | Some("djvu") | Some("mobi") | Some("chm")
                    );

                    if !is_supported {
                        continue;
                    }

                    // Small delay to ensure file is fully written
                    std::thread::sleep(Duration::from_millis(500));

                    let scanned = match lib_organizer::scan_files(std::slice::from_ref(&path)) {
                        Ok(files) => files,
                        Err(e) => {
                            eprintln!("[!] Failed to scan {}: {}", path.display(), e);
                            continue;
                        }
                    };

                    for file in &scanned {
                        match organizer.ingest(file, &options) {
                            Ok(result) => {
                                println!(
                                    "[+] {} -> {}/{}",
                                    file.filename().unwrap_or("?"),
                                    result.entry.topic,
                                    result.entry.subtopic.as_deref().unwrap_or("")
                                );
                                if let Err(e) = organizer.commit(&format!(
                                    "Ingest {}",
                                    file.filename().unwrap_or("file")
                                )) {
                                    eprintln!("[!] Commit failed: {}", e);
                                }
                            }
                            Err(e) => {
                                eprintln!("[!] {}: {}", file.filename().unwrap_or("?"), e);
                            }
                        }
                    }
                }
            }
            Err(e) => eprintln!("[!] Watch error: {}", e),
        }
    }

    Ok(())
}
