use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

use lib_organizer::{format_secrets_results, scan_for_secrets, SecretsScanOptions, Severity};

use super::ToolDefinition;

pub fn definitions() -> Vec<ToolDefinition> {
    vec![ToolDefinition {
        name: "secrets_scan".to_string(),
        description: "Scan directories for sensitive files like private keys, passwords, and recovery kits".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "paths": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Directories to scan for secrets"
                },
                "check_content": {
                    "type": "boolean",
                    "description": "Check file contents for secrets (default false)"
                }
            },
            "required": ["paths"]
        }),
    }]
}

#[derive(Debug, Deserialize)]
struct SecretsScanArgs {
    paths: Vec<String>,
    #[serde(default)]
    check_content: bool,
}

pub fn call(name: &str, arguments: Value) -> Result<Value, String> {
    match name {
        "secrets_scan" => {
            let args: SecretsScanArgs = serde_json::from_value(arguments)
                .map_err(|e| format!("invalid arguments: {}", e))?;

            let options = SecretsScanOptions {
                check_content: args.check_content,
                max_file_size: 1024 * 1024,
                include_hidden: true,
            };

            let results: Vec<_> = args
                .paths
                .iter()
                .flat_map(|p| scan_for_secrets(Path::new(p), &options))
                .collect();

            let output = format_secrets_results(&results);

            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": output
                }],
                "metadata": {
                    "secrets_found": results.len(),
                    "critical_count": results.iter().filter(|r| r.severity() == Severity::Critical).count()
                }
            }))
        }
        _ => Err(format!("unknown secrets tool: {}", name)),
    }
}
