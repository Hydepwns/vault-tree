use serde_json::json;

use crate::tools::{call_tool, list_tools};
use crate::transport::{
    JsonRpcRequest, JsonRpcResponse, INTERNAL_ERROR, INVALID_PARAMS, METHOD_NOT_FOUND, PARSE_ERROR,
};

const SERVER_NAME: &str = "vault-tree-mcp";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
const PROTOCOL_VERSION: &str = "2024-11-05";

pub struct McpServer {
    initialized: bool,
}

impl McpServer {
    pub fn new() -> Self {
        Self { initialized: false }
    }

    pub fn handle_request(&mut self, input: &str) -> Option<String> {
        let request: JsonRpcRequest = match serde_json::from_str(input) {
            Ok(r) => r,
            Err(_) => {
                let resp = JsonRpcResponse::error(None, PARSE_ERROR, "Parse error");
                return Some(serde_json::to_string(&resp).unwrap());
            }
        };

        let response = match request.method.as_str() {
            "initialize" => self.handle_initialize(&request),
            "initialized" => {
                self.initialized = true;
                return None;
            }
            "tools/list" => self.handle_tools_list(&request),
            "tools/call" => self.handle_tools_call(&request),
            "ping" => JsonRpcResponse::success(request.id, json!({})),
            _ => JsonRpcResponse::error(
                request.id,
                METHOD_NOT_FOUND,
                format!("Method not found: {}", request.method),
            ),
        };

        Some(serde_json::to_string(&response).unwrap())
    }

    fn handle_initialize(&mut self, request: &JsonRpcRequest) -> JsonRpcResponse {
        JsonRpcResponse::success(
            request.id.clone(),
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": SERVER_NAME,
                    "version": SERVER_VERSION
                }
            }),
        )
    }

    fn handle_tools_list(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let tools = list_tools();
        JsonRpcResponse::success(request.id.clone(), json!({ "tools": tools }))
    }

    fn handle_tools_call(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        let params = match &request.params {
            Some(p) => p,
            None => {
                return JsonRpcResponse::error(request.id.clone(), INVALID_PARAMS, "Missing params")
            }
        };

        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

        match call_tool(name, arguments) {
            Ok(result) => JsonRpcResponse::success(request.id.clone(), result),
            Err(e) => JsonRpcResponse::error(request.id.clone(), INTERNAL_ERROR, e),
        }
    }
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}
