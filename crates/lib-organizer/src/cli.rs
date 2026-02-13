use anyhow::Result;
use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use std::path::{Path, PathBuf};

use lib_organizer::{
    classify_file, find_duplicates, format_size, scan_directory, Manifest, Organizer, ScanOptions,
    Topic,
};

#[derive(Parser)]
#[command(name = "lib-organizer")]
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
        #[arg(short, long, help = "Directories to scan")]
        dirs: Vec<PathBuf>,
        #[arg(short, long, help = "Non-recursive scan")]
        flat: bool,
    },
    Duplicates {
        #[arg(short, long, help = "Directories to scan for duplicates")]
        dirs: Vec<PathBuf>,
    },
    Classify {
        #[arg(help = "File to classify")]
        file: PathBuf,
        #[arg(short, long, help = "Library path for keyword rules")]
        library: Option<PathBuf>,
    },
    Ingest {
        #[arg(help = "Files to ingest")]
        files: Vec<PathBuf>,
        #[arg(short, long, help = "Topic to assign")]
        topic: Option<String>,
        #[arg(short, long, help = "Subtopic to assign")]
        subtopic: Option<String>,
        #[arg(short, long, help = "Compress files")]
        compress: bool,
        #[arg(short, long, help = "Library path")]
        library: PathBuf,
        #[arg(long, help = "Copy instead of move")]
        copy: bool,
        #[arg(short, long, help = "Commit message")]
        message: Option<String>,
    },
    Search {
        #[arg(help = "Search query")]
        query: String,
        #[arg(short, long, help = "Library path")]
        library: PathBuf,
    },
    Status {
        #[arg(short, long, help = "Library path")]
        library: PathBuf,
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
        Commands::Search { query, library } => cmd_search(&query, &library),
        Commands::Status { library } => cmd_status(&library),
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
        vec![std::env::current_dir()?]
    } else {
        dirs.to_vec()
    };

    let options = ScanOptions {
        recursive: !flat,
        ..Default::default()
    };

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );

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
            "  {:>10}  {:?}  {}",
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
        lib_organizer::Config::new("/tmp/lib")
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
    println!("Confidence: {:?}", result.confidence);

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
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );

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

fn cmd_search(query: &str, library: &Path) -> Result<()> {
    let manifest = Manifest::load(&library.join("manifest.json"))?;
    let results = manifest.search(query);

    if results.is_empty() {
        println!("No matches for '{}'", query);
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
