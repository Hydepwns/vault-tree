use serde_json::{json, Value};
use std::fs;
use tempfile::TempDir;
use vault_tree_mcp::server::McpServer;

fn request(method: &str, params: Option<Value>) -> String {
    let req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params
    });
    serde_json::to_string(&req).unwrap()
}

fn tool_call(name: &str, arguments: Value) -> String {
    request(
        "tools/call",
        Some(json!({
            "name": name,
            "arguments": arguments
        })),
    )
}

fn parse_response(response: &str) -> Value {
    serde_json::from_str(response).unwrap()
}

fn get_text_content(response: &Value) -> &str {
    response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or("")
}

fn create_test_vault() -> TempDir {
    let dir = TempDir::new().unwrap();

    fs::write(
        dir.path().join("note1.md"),
        "---\ntitle: Note 1\ntags: [rust, programming]\ndate: 2025-01-18\n---\n\n# Hello World\n\nContent with [[note2]]\n",
    )
    .unwrap();

    fs::write(
        dir.path().join("note2.md"),
        "---\ntitle: Note 2\ntags: [mcp]\n---\n\n# Another Note\n\nHello again!\n",
    )
    .unwrap();

    fs::create_dir(dir.path().join("subdir")).unwrap();
    fs::write(
        dir.path().join("subdir/nested.md"),
        "# Nested\n\nLinks to [[note1]]",
    )
    .unwrap();

    fs::create_dir(dir.path().join(".obsidian")).unwrap();
    fs::write(dir.path().join(".obsidian/config.json"), "{}").unwrap();

    dir
}

// ============================================================================
// MCP Protocol Tests
// ============================================================================

#[test]
fn initialize_returns_server_info() {
    let mut server = McpServer::new();
    let resp = server
        .handle_request(&request("initialize", Some(json!({}))))
        .unwrap();
    let json: Value = parse_response(&resp);

    assert_eq!(json["jsonrpc"], "2.0");
    assert!(json["result"]["serverInfo"]["name"].as_str().is_some());
    assert!(json["result"]["serverInfo"]["version"].as_str().is_some());
    assert!(json["result"]["protocolVersion"].as_str().is_some());
    assert!(json["result"]["capabilities"]["tools"].is_object());
}

#[test]
fn initialized_notification_returns_nothing() {
    let mut server = McpServer::new();
    let resp = server.handle_request(&request("initialized", None));
    assert!(resp.is_none());
}

#[test]
fn ping_returns_empty_object() {
    let mut server = McpServer::new();
    let resp = server.handle_request(&request("ping", None)).unwrap();
    let json: Value = parse_response(&resp);

    assert_eq!(json["result"], json!({}));
}

#[test]
fn tools_list_returns_all_tools() {
    let mut server = McpServer::new();
    let resp = server.handle_request(&request("tools/list", None)).unwrap();
    let json: Value = parse_response(&resp);

    let tools = json["result"]["tools"].as_array().unwrap();
    // vault_tree, vault_search, knowledge_lookup = 3 tools minimum
    assert!(tools.len() >= 3);

    let tool_names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(tool_names.contains(&"vault_tree"));
    assert!(tool_names.contains(&"vault_search"));
    assert!(tool_names.contains(&"knowledge_lookup"));
}

#[test]
fn unknown_method_returns_error() {
    let mut server = McpServer::new();
    let resp = server
        .handle_request(&request("unknown/method", None))
        .unwrap();
    let json: Value = parse_response(&resp);

    assert!(json["error"].is_object());
    assert_eq!(json["error"]["code"], -32601);
}

#[test]
fn invalid_json_returns_parse_error() {
    let mut server = McpServer::new();
    let resp = server.handle_request("not valid json").unwrap();
    let json: Value = parse_response(&resp);

    assert!(json["error"].is_object());
    assert_eq!(json["error"]["code"], -32700);
}

// ============================================================================
// Vault Tools Tests
// ============================================================================

#[test]
fn vault_tree_generates_tree() {
    let vault = create_test_vault();
    let mut server = McpServer::new();

    let resp = server
        .handle_request(&tool_call(
            "vault_tree",
            json!({ "vault_path": vault.path().to_str().unwrap() }),
        ))
        .unwrap();

    let json = parse_response(&resp);
    let text = get_text_content(&json);

    assert!(text.contains("note1.md"));
    assert!(text.contains("note2.md"));
    assert!(text.contains("nested.md"));
    assert!(!text.contains(".obsidian"));
}

#[test]
fn vault_tree_respects_depth() {
    let vault = create_test_vault();
    let mut server = McpServer::new();

    let resp = server
        .handle_request(&tool_call(
            "vault_tree",
            json!({ "vault_path": vault.path().to_str().unwrap(), "depth": 1 }),
        ))
        .unwrap();

    let json = parse_response(&resp);
    let text = get_text_content(&json);

    assert!(text.contains("note1.md"));
    assert!(text.contains("subdir"));
    assert!(!text.contains("nested.md"));
}

#[test]
fn vault_search_finds_matches() {
    let vault = create_test_vault();
    let mut server = McpServer::new();

    let resp = server
        .handle_request(&tool_call(
            "vault_search",
            json!({
                "vault_path": vault.path().to_str().unwrap(),
                "pattern": "Hello"
            }),
        ))
        .unwrap();

    let json = parse_response(&resp);
    let text = get_text_content(&json);

    assert!(text.contains("note1.md") || text.contains("note2.md"));
    assert!(text.contains("Hello"));
}

#[test]
fn vault_search_case_insensitive() {
    let vault = create_test_vault();
    let mut server = McpServer::new();

    let resp = server
        .handle_request(&tool_call(
            "vault_search",
            json!({
                "vault_path": vault.path().to_str().unwrap(),
                "pattern": "hello",
                "case_insensitive": true
            }),
        ))
        .unwrap();

    let json = parse_response(&resp);
    let text = get_text_content(&json);

    assert!(text.contains("Hello"));
}

#[test]
fn vault_search_no_matches() {
    let vault = create_test_vault();
    let mut server = McpServer::new();

    let resp = server
        .handle_request(&tool_call(
            "vault_search",
            json!({
                "vault_path": vault.path().to_str().unwrap(),
                "pattern": "xyznonexistent"
            }),
        ))
        .unwrap();

    let json = parse_response(&resp);
    let text = get_text_content(&json);

    assert!(text.contains("No matches"));
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn tool_call_missing_params_returns_error() {
    let mut server = McpServer::new();
    let resp = server.handle_request(&request("tools/call", None)).unwrap();
    let json = parse_response(&resp);

    assert!(json["error"].is_object());
    assert_eq!(json["error"]["code"], -32602);
}

#[test]
fn unknown_tool_returns_error() {
    let mut server = McpServer::new();
    let resp = server
        .handle_request(&tool_call("nonexistent_tool", json!({})))
        .unwrap();
    let json = parse_response(&resp);

    assert!(json["error"].is_object());
    assert!(json["error"]["message"]
        .as_str()
        .unwrap()
        .contains("unknown tool"));
}

#[test]
fn vault_tree_invalid_path_returns_error() {
    let mut server = McpServer::new();
    let resp = server
        .handle_request(&tool_call(
            "vault_tree",
            json!({ "vault_path": "/nonexistent/path/xyz" }),
        ))
        .unwrap();
    let json = parse_response(&resp);

    assert!(json["error"].is_object());
}
