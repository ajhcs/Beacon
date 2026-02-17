use beacon_core::mcp::{handle_request, McpState};
use std::sync::Arc;

fn make_state() -> Arc<McpState> {
    Arc::new(McpState::new())
}

fn make_request(method: &str, params: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params,
    })
}

#[test]
fn test_initialize() {
    let state = make_state();
    let req = make_request("initialize", serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": { "name": "test", "version": "1.0" }
    }));
    let resp = handle_request(&req, &state);
    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 1);
    assert!(resp["result"]["serverInfo"]["name"].is_string());
    assert!(resp["result"]["capabilities"]["tools"].is_object());
}

#[test]
fn test_tools_list() {
    let state = make_state();
    let req = make_request("tools/list", serde_json::json!({}));
    let resp = handle_request(&req, &state);
    let tools = resp["result"]["tools"].as_array().unwrap();
    assert!(!tools.is_empty());

    // Should include beacon_compile and beacon_status
    let tool_names: Vec<&str> = tools
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(tool_names.contains(&"beacon_compile"));
    assert!(tool_names.contains(&"beacon_status"));
}

#[test]
fn test_tools_call_beacon_compile_success() {
    let state = make_state();
    let ir_json = include_str!("../../beacon-ir/tests/fixtures/document_lifecycle.json");
    let req = make_request("tools/call", serde_json::json!({
        "name": "beacon_compile",
        "arguments": {
            "ir_json": ir_json
        }
    }));
    let resp = handle_request(&req, &state);
    assert!(resp["error"].is_null(), "Unexpected error: {}", resp["error"]);

    let content = &resp["result"]["content"][0];
    assert_eq!(content["type"], "text");
    let text: serde_json::Value = serde_json::from_str(content["text"].as_str().unwrap()).unwrap();
    assert_eq!(text["result"], "pass");
    assert!(text["campaign_id"].is_string());
    assert!(text["budget"]["min_iterations"].is_number());
}

#[test]
fn test_tools_call_beacon_compile_error() {
    let state = make_state();
    let req = make_request("tools/call", serde_json::json!({
        "name": "beacon_compile",
        "arguments": {
            "ir_json": "not json"
        }
    }));
    let resp = handle_request(&req, &state);
    // MCP tools return isError=true in content for tool-level errors
    let content = &resp["result"]["content"][0];
    let text: serde_json::Value = serde_json::from_str(content["text"].as_str().unwrap()).unwrap();
    assert_eq!(text["result"], "errors");
    assert!(text["errors"].is_array());
}

#[test]
fn test_tools_call_beacon_status() {
    let state = make_state();

    // Status when empty
    let req = make_request("tools/call", serde_json::json!({
        "name": "beacon_status",
        "arguments": {}
    }));
    let resp = handle_request(&req, &state);
    let content = &resp["result"]["content"][0];
    let text: serde_json::Value = serde_json::from_str(content["text"].as_str().unwrap()).unwrap();
    assert_eq!(text["state"], "idle");
    assert_eq!(text["active_campaigns"], 0);

    // Compile something, then check status
    let ir_json = include_str!("../../beacon-ir/tests/fixtures/document_lifecycle.json");
    let compile_req = make_request("tools/call", serde_json::json!({
        "name": "beacon_compile",
        "arguments": { "ir_json": ir_json }
    }));
    handle_request(&compile_req, &state);

    let resp = handle_request(&req, &state);
    let content = &resp["result"]["content"][0];
    let text: serde_json::Value = serde_json::from_str(content["text"].as_str().unwrap()).unwrap();
    assert_eq!(text["active_campaigns"], 1);
}

#[test]
fn test_unknown_tool() {
    let state = make_state();
    let req = make_request("tools/call", serde_json::json!({
        "name": "nonexistent_tool",
        "arguments": {}
    }));
    let resp = handle_request(&req, &state);
    assert!(resp["result"]["isError"].as_bool().unwrap_or(false));
}

#[test]
fn test_unknown_method() {
    let state = make_state();
    let req = make_request("nonexistent/method", serde_json::json!({}));
    let resp = handle_request(&req, &state);
    assert!(resp["error"].is_object());
    assert_eq!(resp["error"]["code"], -32601); // Method not found
}
