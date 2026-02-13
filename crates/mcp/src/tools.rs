use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;
use vault_tree_core::{generate_tree, render_tree, search_vault, SearchOptions, TreeOptions};

use lib_organizer::{
    classify_file, find_duplicates, format_size, scan_directory, scan_files, Config, FileType,
    IngestOptions, Manifest, Organizer, ScanOptions, Topic,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

pub fn list_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "vault_tree".to_string(),
            description: "Generate an annotated tree of an Obsidian vault showing file structure, tags, dates, and link counts".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "vault_path": {
                        "type": "string",
                        "description": "Path to the Obsidian vault directory"
                    },
                    "depth": {
                        "type": "integer",
                        "description": "Maximum depth to traverse (optional, default unlimited)"
                    }
                },
                "required": ["vault_path"]
            }),
        },
        ToolDefinition {
            name: "vault_search".to_string(),
            description: "Search for a pattern across all markdown files in an Obsidian vault".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "vault_path": {
                        "type": "string",
                        "description": "Path to the Obsidian vault directory"
                    },
                    "pattern": {
                        "type": "string",
                        "description": "Regex pattern to search for"
                    },
                    "file_pattern": {
                        "type": "string",
                        "description": "Regex pattern to filter file names (optional)"
                    },
                    "case_insensitive": {
                        "type": "boolean",
                        "description": "Whether to perform case-insensitive search (default false)"
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Maximum number of matches to return (optional)"
                    }
                },
                "required": ["vault_path", "pattern"]
            }),
        },
        ToolDefinition {
            name: "lib_scan".to_string(),
            description: "Scan directories for books and documents (PDF, EPUB, etc.)".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "paths": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Directories to scan"
                    },
                    "recursive": {
                        "type": "boolean",
                        "description": "Scan subdirectories (default true)"
                    }
                },
                "required": ["paths"]
            }),
        },
        ToolDefinition {
            name: "lib_duplicates".to_string(),
            description: "Find duplicate files by content hash".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "paths": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Directories to scan for duplicates"
                    }
                },
                "required": ["paths"]
            }),
        },
        ToolDefinition {
            name: "lib_classify".to_string(),
            description: "Get topic classification suggestions for files".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "files": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Files to classify"
                    },
                    "library_path": {
                        "type": "string",
                        "description": "Library path for keyword rules (optional)"
                    }
                },
                "required": ["files"]
            }),
        },
        ToolDefinition {
            name: "lib_ingest".to_string(),
            description: "Ingest files into the library".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "library_path": {
                        "type": "string",
                        "description": "Path to the library"
                    },
                    "files": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Files to ingest"
                    },
                    "topic": {
                        "type": "string",
                        "description": "Topic to assign (optional, auto-classified if not provided)"
                    },
                    "subtopic": {
                        "type": "string",
                        "description": "Subtopic to assign (optional)"
                    },
                    "compress": {
                        "type": "boolean",
                        "description": "Compress files with zstd (default false)"
                    },
                    "copy": {
                        "type": "boolean",
                        "description": "Copy instead of move (default false)"
                    },
                    "commit_message": {
                        "type": "string",
                        "description": "Git commit message (optional)"
                    }
                },
                "required": ["library_path", "files"]
            }),
        },
        ToolDefinition {
            name: "lib_search".to_string(),
            description: "Search the library for files".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "library_path": {
                        "type": "string",
                        "description": "Path to the library"
                    },
                    "query": {
                        "type": "string",
                        "description": "Search query"
                    }
                },
                "required": ["library_path", "query"]
            }),
        },
        ToolDefinition {
            name: "lib_status".to_string(),
            description: "Get library status and statistics".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "library_path": {
                        "type": "string",
                        "description": "Path to the library"
                    }
                },
                "required": ["library_path"]
            }),
        },
        ToolDefinition {
            name: "lib_init".to_string(),
            description: "Initialize a new library".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to create the library"
                    }
                },
                "required": ["path"]
            }),
        },
    ]
}

#[derive(Debug, Deserialize)]
struct VaultTreeArgs {
    vault_path: String,
    depth: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct VaultSearchArgs {
    vault_path: String,
    pattern: String,
    file_pattern: Option<String>,
    #[serde(default)]
    case_insensitive: bool,
    max_results: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct LibScanArgs {
    paths: Vec<String>,
    #[serde(default = "default_true")]
    recursive: bool,
}

#[derive(Debug, Deserialize)]
struct LibDuplicatesArgs {
    paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct LibClassifyArgs {
    files: Vec<String>,
    library_path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LibIngestArgs {
    library_path: String,
    files: Vec<String>,
    topic: Option<String>,
    subtopic: Option<String>,
    #[serde(default)]
    compress: bool,
    #[serde(default)]
    copy: bool,
    commit_message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LibSearchArgs {
    library_path: String,
    query: String,
}

#[derive(Debug, Deserialize)]
struct LibStatusArgs {
    library_path: String,
}

#[derive(Debug, Deserialize)]
struct LibInitArgs {
    path: String,
}

fn default_true() -> bool {
    true
}

pub fn call_tool(name: &str, arguments: Value) -> Result<Value, String> {
    match name {
        "vault_tree" => {
            let args: VaultTreeArgs = serde_json::from_value(arguments)
                .map_err(|e| format!("invalid arguments: {}", e))?;

            let options = TreeOptions { depth: args.depth };

            let tree = generate_tree(Path::new(&args.vault_path), &options)
                .map_err(|e| format!("failed to generate tree: {}", e))?;

            let output = render_tree(&tree);

            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": output
                }]
            }))
        }
        "vault_search" => {
            let args: VaultSearchArgs = serde_json::from_value(arguments)
                .map_err(|e| format!("invalid arguments: {}", e))?;

            let options = SearchOptions {
                file_pattern: args.file_pattern,
                case_insensitive: args.case_insensitive,
                max_results: args.max_results,
            };

            let results = search_vault(Path::new(&args.vault_path), &args.pattern, &options)
                .map_err(|e| format!("search failed: {}", e))?;

            let mut output = String::new();
            for result in &results {
                output.push_str(&format!("## {}\n", result.file_path));
                for m in &result.matches {
                    output.push_str(&format!(
                        "  {}:{} {}\n",
                        m.line_number, m.match_start, m.line_content
                    ));
                }
                output.push('\n');
            }

            if results.is_empty() {
                output = "No matches found.".to_string();
            }

            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": output
                }]
            }))
        }
        "lib_scan" => {
            let args: LibScanArgs = serde_json::from_value(arguments)
                .map_err(|e| format!("invalid arguments: {}", e))?;

            let options = ScanOptions {
                recursive: args.recursive,
                ..Default::default()
            };

            let mut all_files = Vec::new();
            for path in &args.paths {
                let files = scan_directory(Path::new(path), &options)
                    .map_err(|e| format!("scan failed: {}", e))?;
                all_files.extend(files);
            }

            let mut output = format!("Found {} files:\n\n", all_files.len());
            let mut total_size = 0u64;

            for file in &all_files {
                let filename = file.filename().unwrap_or("?");
                output.push_str(&format!(
                    "{:>10}  {:?}  {}\n",
                    format_size(file.size),
                    file.file_type,
                    filename
                ));
                total_size += file.size;
            }

            output.push_str(&format!(
                "\nTotal: {} in {} files",
                format_size(total_size),
                all_files.len()
            ));

            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": output
                }]
            }))
        }
        "lib_duplicates" => {
            let args: LibDuplicatesArgs = serde_json::from_value(arguments)
                .map_err(|e| format!("invalid arguments: {}", e))?;

            let options = ScanOptions::default();

            let mut all_files = Vec::new();
            for path in &args.paths {
                let files = scan_directory(Path::new(path), &options)
                    .map_err(|e| format!("scan failed: {}", e))?;
                all_files.extend(files);
            }

            let dupes = find_duplicates(&all_files);

            if dupes.is_empty() {
                return Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": "No duplicates found."
                    }]
                }));
            }

            let mut output = format!("Found {} duplicate groups:\n\n", dupes.len());

            for (i, group) in dupes.iter().enumerate() {
                output.push_str(&format!(
                    "Group {} ({}):\n",
                    i + 1,
                    format_size(group[0].size)
                ));
                for file in group {
                    output.push_str(&format!("  {}\n", file.path.display()));
                }
                output.push('\n');
            }

            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": output
                }]
            }))
        }
        "lib_classify" => {
            let args: LibClassifyArgs = serde_json::from_value(arguments)
                .map_err(|e| format!("invalid arguments: {}", e))?;

            let config = if let Some(lib) = args.library_path {
                let organizer = Organizer::open(Path::new(&lib))
                    .map_err(|e| format!("failed to open library: {}", e))?;
                organizer.config().clone()
            } else {
                Config::new("/tmp/lib")
            };

            let mut output = String::new();

            for file_path in &args.files {
                let path = Path::new(file_path);
                let file_type = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(FileType::from_extension)
                    .unwrap_or(FileType::Unknown);

                match classify_file(path, file_type, &config) {
                    Ok(result) => {
                        output.push_str(&format!("## {}\n", file_path));
                        output.push_str(&format!("Topic: {}\n", result.topic));
                        if let Some(sub) = &result.subtopic {
                            output.push_str(&format!("Subtopic: {}\n", sub));
                        }
                        output.push_str(&format!("Confidence: {:?}\n", result.confidence));
                        if !result.matched_keywords.is_empty() {
                            output.push_str(&format!(
                                "Matched keywords: {}\n",
                                result.matched_keywords.join(", ")
                            ));
                        }
                        if let Some(title) = &result.metadata.title {
                            output.push_str(&format!("Title: {}\n", title));
                        }
                        if let Some(author) = &result.metadata.author {
                            output.push_str(&format!("Author: {}\n", author));
                        }
                        output.push('\n');
                    }
                    Err(e) => {
                        output.push_str(&format!("## {}\nError: {}\n\n", file_path, e));
                    }
                }
            }

            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": output
                }]
            }))
        }
        "lib_ingest" => {
            let args: LibIngestArgs = serde_json::from_value(arguments)
                .map_err(|e| format!("invalid arguments: {}", e))?;

            let mut organizer = Organizer::open(Path::new(&args.library_path))
                .map_err(|e| format!("failed to open library: {}", e))?;

            let paths: Vec<_> = args.files.iter().map(std::path::PathBuf::from).collect();
            let scanned = scan_files(&paths).map_err(|e| format!("scan failed: {}", e))?;

            let options = IngestOptions {
                topic: args.topic.map(Topic::from),
                subtopic: args.subtopic,
                compress: args.compress,
                move_file: !args.copy,
            };

            let mut output = String::new();
            let mut ingested = 0;

            for file in &scanned {
                match organizer.ingest(file, &options) {
                    Ok(result) => {
                        ingested += 1;
                        let size_info = if let Some(compressed) = result.compressed_size {
                            format!(" (compressed: {})", format_size(compressed))
                        } else {
                            String::new()
                        };
                        output.push_str(&format!(
                            "[+] {} -> {}/{}{}\n",
                            file.filename().unwrap_or("?"),
                            result.entry.topic,
                            result.entry.subtopic.as_deref().unwrap_or(""),
                            size_info
                        ));
                    }
                    Err(e) => {
                        output.push_str(&format!(
                            "[!] {}: {}\n",
                            file.filename().unwrap_or("?"),
                            e
                        ));
                    }
                }
            }

            if ingested > 0 {
                let msg = args
                    .commit_message
                    .unwrap_or_else(|| format!("Ingest {} files", ingested));
                organizer
                    .commit(&msg)
                    .map_err(|e| format!("commit failed: {}", e))?;
                output.push_str(&format!("\nCommitted: {}", msg));
            } else {
                output.push_str("\nNo files ingested.");
            }

            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": output
                }]
            }))
        }
        "lib_search" => {
            let args: LibSearchArgs = serde_json::from_value(arguments)
                .map_err(|e| format!("invalid arguments: {}", e))?;

            let manifest_path = Path::new(&args.library_path).join("manifest.json");
            let manifest = Manifest::load(&manifest_path)
                .map_err(|e| format!("failed to load manifest: {}", e))?;

            let results = manifest.search(&args.query);

            if results.is_empty() {
                return Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": format!("No matches for '{}'", args.query)
                    }]
                }));
            }

            let mut output = format!("Found {} matches for '{}':\n\n", results.len(), args.query);

            for entry in results {
                output.push_str(&format!("{}\n", entry.path.display()));
                if let Some(title) = &entry.title {
                    output.push_str(&format!("  Title: {}\n", title));
                }
                if let Some(author) = &entry.author {
                    output.push_str(&format!("  Author: {}\n", author));
                }
                output.push_str(&format!("  Topic: {}\n", entry.topic));
                output.push_str(&format!("  Size: {}\n", format_size(entry.size)));
                output.push('\n');
            }

            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": output
                }]
            }))
        }
        "lib_status" => {
            let args: LibStatusArgs = serde_json::from_value(arguments)
                .map_err(|e| format!("invalid arguments: {}", e))?;

            let organizer = Organizer::open(Path::new(&args.library_path))
                .map_err(|e| format!("failed to open library: {}", e))?;

            let status = organizer.status();

            let mut output = format!(
                "Library: {}\nTotal files: {}\nTotal size: {}\nGit status: {}\n\nBy topic:\n",
                args.library_path,
                status.total_files,
                format_size(status.total_size),
                status.git_status
            );

            let mut topics = status.topics;
            topics.sort_by(|a, b| b.1.cmp(&a.1));

            for (topic, count) in topics {
                output.push_str(&format!("  {}: {}\n", topic, count));
            }

            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": output
                }]
            }))
        }
        "lib_init" => {
            let args: LibInitArgs = serde_json::from_value(arguments)
                .map_err(|e| format!("invalid arguments: {}", e))?;

            let mut organizer = Organizer::init(Path::new(&args.path))
                .map_err(|e| format!("init failed: {}", e))?;

            organizer
                .commit("Initialize library")
                .map_err(|e| format!("commit failed: {}", e))?;

            let mut output = format!("Initialized library at {}\nCreated topics:\n", args.path);

            for topic in organizer.config().default_topics.iter() {
                output.push_str(&format!("  - {}\n", topic));
            }

            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": output
                }]
            }))
        }
        _ => Err(format!("unknown tool: {}", name)),
    }
}
