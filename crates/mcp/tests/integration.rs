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

fn create_test_source_dir() -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("rust_book.pdf"), b"fake pdf content").unwrap();
    fs::write(dir.path().join("electronics_manual.epub"), b"fake epub").unwrap();
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
    assert!(tools.len() >= 9);

    let tool_names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(tool_names.contains(&"vault_tree"));
    assert!(tool_names.contains(&"vault_search"));
    assert!(tool_names.contains(&"lib_scan"));
    assert!(tool_names.contains(&"lib_init"));
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
// Library Tools Tests
// ============================================================================

#[test]
fn lib_init_creates_library() {
    let dir = TempDir::new().unwrap();
    let lib_path = dir.path().join("library");
    let mut server = McpServer::new();

    let resp = server
        .handle_request(&tool_call(
            "lib_init",
            json!({ "path": lib_path.to_str().unwrap() }),
        ))
        .unwrap();

    let json = parse_response(&resp);
    let text = get_text_content(&json);

    assert!(text.contains("Initialized library"));
    assert!(text.contains("programming"));
    assert!(text.contains("electronics"));
    assert!(lib_path.join("manifest.json").exists());
    assert!(lib_path.join(".git").exists());
}

#[test]
fn lib_scan_finds_files() {
    let source = create_test_source_dir();
    let mut server = McpServer::new();

    let resp = server
        .handle_request(&tool_call(
            "lib_scan",
            json!({ "paths": [source.path().to_str().unwrap()] }),
        ))
        .unwrap();

    let json = parse_response(&resp);
    let text = get_text_content(&json);

    assert!(text.contains("Found 2 files"));
    assert!(text.contains("rust_book.pdf"));
    assert!(text.contains("electronics_manual.epub"));
}

#[test]
fn lib_duplicates_detects_dupes() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("file1.pdf"), b"same content").unwrap();
    fs::write(dir.path().join("file2.pdf"), b"same content").unwrap();
    fs::write(dir.path().join("file3.pdf"), b"different").unwrap();

    let mut server = McpServer::new();

    let resp = server
        .handle_request(&tool_call(
            "lib_duplicates",
            json!({ "paths": [dir.path().to_str().unwrap()] }),
        ))
        .unwrap();

    let json = parse_response(&resp);
    let text = get_text_content(&json);

    assert!(text.contains("duplicate"));
    assert!(text.contains("file1.pdf"));
    assert!(text.contains("file2.pdf"));
}

#[test]
fn lib_duplicates_no_dupes() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("file1.pdf"), b"content1").unwrap();
    fs::write(dir.path().join("file2.pdf"), b"content2").unwrap();

    let mut server = McpServer::new();

    let resp = server
        .handle_request(&tool_call(
            "lib_duplicates",
            json!({ "paths": [dir.path().to_str().unwrap()] }),
        ))
        .unwrap();

    let json = parse_response(&resp);
    let text = get_text_content(&json);

    assert!(text.contains("No duplicates"));
}

#[test]
fn lib_classify_suggests_topic() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("rust_programming_guide.pdf");
    fs::write(&file, b"fake pdf").unwrap();

    let mut server = McpServer::new();

    let resp = server
        .handle_request(&tool_call(
            "lib_classify",
            json!({ "files": [file.to_str().unwrap()] }),
        ))
        .unwrap();

    let json = parse_response(&resp);
    let text = get_text_content(&json);

    assert!(text.contains("Topic:"));
    assert!(text.contains("programming") || text.contains("Confidence"));
}

#[test]
fn lib_status_shows_info() {
    let dir = TempDir::new().unwrap();
    let lib_path = dir.path().join("library");

    let mut server = McpServer::new();

    server
        .handle_request(&tool_call(
            "lib_init",
            json!({ "path": lib_path.to_str().unwrap() }),
        ))
        .unwrap();

    let resp = server
        .handle_request(&tool_call(
            "lib_status",
            json!({ "library_path": lib_path.to_str().unwrap() }),
        ))
        .unwrap();

    let json = parse_response(&resp);
    let text = get_text_content(&json);

    assert!(text.contains("Library:"));
    assert!(text.contains("Total files:"));
    assert!(text.contains("Git status:"));
}

#[test]
fn lib_ingest_adds_file_to_library() {
    let dir = TempDir::new().unwrap();
    let lib_path = dir.path().join("library");
    let source_dir = dir.path().join("source");
    fs::create_dir(&source_dir).unwrap();
    fs::write(source_dir.join("test.pdf"), b"pdf content").unwrap();

    let mut server = McpServer::new();

    server
        .handle_request(&tool_call(
            "lib_init",
            json!({ "path": lib_path.to_str().unwrap() }),
        ))
        .unwrap();

    let resp = server
        .handle_request(&tool_call(
            "lib_ingest",
            json!({
                "library_path": lib_path.to_str().unwrap(),
                "files": [source_dir.join("test.pdf").to_str().unwrap()],
                "copy": true
            }),
        ))
        .unwrap();

    let json = parse_response(&resp);
    let text = get_text_content(&json);

    assert!(text.contains("[+]") || text.contains("test.pdf"));
    assert!(text.contains("Committed"));
}

#[test]
fn lib_search_finds_entries() {
    let dir = TempDir::new().unwrap();
    let lib_path = dir.path().join("library");
    let source_dir = dir.path().join("source");
    fs::create_dir(&source_dir).unwrap();
    fs::write(source_dir.join("rust_book.pdf"), b"pdf content").unwrap();

    let mut server = McpServer::new();

    server
        .handle_request(&tool_call(
            "lib_init",
            json!({ "path": lib_path.to_str().unwrap() }),
        ))
        .unwrap();

    server
        .handle_request(&tool_call(
            "lib_ingest",
            json!({
                "library_path": lib_path.to_str().unwrap(),
                "files": [source_dir.join("rust_book.pdf").to_str().unwrap()],
                "copy": true
            }),
        ))
        .unwrap();

    let resp = server
        .handle_request(&tool_call(
            "lib_search",
            json!({
                "library_path": lib_path.to_str().unwrap(),
                "query": "rust"
            }),
        ))
        .unwrap();

    let json = parse_response(&resp);
    let text = get_text_content(&json);

    assert!(text.contains("rust") || text.contains("Found"));
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
