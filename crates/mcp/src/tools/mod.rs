mod knowledge;
mod library;
mod secrets;
mod vault;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

pub fn list_tools() -> Vec<ToolDefinition> {
    let mut tools = Vec::new();
    tools.extend(vault::definitions());
    tools.extend(library::definitions());
    tools.extend(knowledge::definitions());
    tools.extend(secrets::definitions());
    tools
}

pub fn call_tool(name: &str, arguments: Value) -> Result<Value, String> {
    match name {
        "vault_tree" | "vault_search" => vault::call(name, arguments),
        n if n.starts_with("lib_") => library::call(name, arguments),
        "knowledge_lookup" => knowledge::call(name, arguments),
        "secrets_scan" => secrets::call(name, arguments),
        _ => Err(format!("unknown tool: {}", name)),
    }
}
