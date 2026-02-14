use serde::Deserialize;
use serde_json::{json, Value};

use super::ToolDefinition;
use crate::knowledge::{KnowledgeRegistry, LookupOptions};

pub fn definitions() -> Vec<ToolDefinition> {
    vec![ToolDefinition {
        name: "knowledge_lookup".to_string(),
        description: "Look up information from external knowledge sources (Wikipedia, DBpedia, arXiv, OpenLibrary, etc.)".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query"
                },
                "provider": {
                    "type": "string",
                    "description": "Knowledge provider (auto tries providers in order)",
                    "enum": ["auto", "wikipedia", "dbpedia", "wikidata", "github", "sourceforge", "npm", "crates.io", "stackoverflow", "reddit", "openlibrary", "arxiv", "musicbrainz", "wikiart", "defillama", "shodan"]
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of results (default 5)"
                },
                "language": {
                    "type": "string",
                    "description": "Language code for Wikipedia (default 'en')"
                }
            },
            "required": ["query", "provider"]
        }),
    }]
}

#[derive(Debug, Deserialize)]
struct KnowledgeLookupArgs {
    query: String,
    provider: String,
    max_results: Option<usize>,
    language: Option<String>,
}

pub fn call(name: &str, arguments: Value) -> Result<Value, String> {
    match name {
        "knowledge_lookup" => {
            let args: KnowledgeLookupArgs = serde_json::from_value(arguments)
                .map_err(|e| format!("invalid arguments: {}", e))?;

            let registry = KnowledgeRegistry::new();
            let options = LookupOptions {
                max_results: args.max_results,
                language: args.language,
            };

            let result = if args.provider == "auto" {
                registry.auto_lookup(&args.query, &options)
            } else {
                registry
                    .lookup(&args.provider, &args.query, &options)
                    .ok_or_else(|| format!("unknown provider: {}", args.provider))?
            };

            if !result.success {
                return Err(result.error.unwrap_or_else(|| "lookup failed".to_string()));
            }

            let mut output = format!(
                "Found {} results from {}:\n\n",
                result.entries.len(),
                args.provider
            );

            for entry in &result.entries {
                output.push_str(&format!("## {}\n", entry.title));
                output.push_str(&entry.summary);
                if let Some(url) = &entry.url {
                    output.push_str(&format!("\nURL: {}", url));
                }
                output.push_str("\n\n");
            }

            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": output
                }],
                "metadata": {
                    "provider": args.provider,
                    "results_count": result.entries.len()
                }
            }))
        }
        _ => Err(format!("unknown knowledge tool: {}", name)),
    }
}
