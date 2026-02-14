use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;
use vault_tree_core::{generate_tree, render_tree, search_vault, SearchOptions, TreeOptions};

use super::ToolDefinition;

pub fn definitions() -> Vec<ToolDefinition> {
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

pub fn call(name: &str, arguments: Value) -> Result<Value, String> {
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
        _ => Err(format!("unknown vault tool: {}", name)),
    }
}
