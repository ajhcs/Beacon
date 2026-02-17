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

fn parse_tool_response(resp: &serde_json::Value) -> serde_json::Value {
    let content = &resp["result"]["content"][0];
    serde_json::from_str(content["text"].as_str().unwrap()).unwrap()
}

#[test]
fn test_initialize() {
    let state = make_state();
    let req = make_request(
        "initialize",
        serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test", "version": "1.0" }
        }),
    );
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

    let tool_names: Vec<&str> = tools
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(tool_names.contains(&"beacon_compile"));
    assert!(tool_names.contains(&"beacon_status"));
    assert!(tool_names.contains(&"beacon_fuzz_start"));
    assert!(tool_names.contains(&"beacon_fuzz_status"));
    assert!(tool_names.contains(&"beacon_findings"));
    assert!(tool_names.contains(&"beacon_coverage"));
    assert!(tool_names.contains(&"beacon_abort"));
    assert!(tool_names.contains(&"beacon_analytics"));
}

#[test]
fn test_tools_call_beacon_compile_success() {
    let state = make_state();
    let ir_json = include_str!("../../beacon-ir/tests/fixtures/document_lifecycle.json");
    let req = make_request(
        "tools/call",
        serde_json::json!({
            "name": "beacon_compile",
            "arguments": {
                "ir_json": ir_json
            }
        }),
    );
    let resp = handle_request(&req, &state);
    assert!(
        resp["error"].is_null(),
        "Unexpected error: {}",
        resp["error"]
    );

    let text = parse_tool_response(&resp);
    assert_eq!(text["result"], "pass");
    assert!(text["campaign_id"].is_string());
    assert!(text["budget"]["min_iterations"].is_number());
}

#[test]
fn test_tools_call_beacon_compile_error() {
    let state = make_state();
    let req = make_request(
        "tools/call",
        serde_json::json!({
            "name": "beacon_compile",
            "arguments": {
                "ir_json": "not json"
            }
        }),
    );
    let resp = handle_request(&req, &state);
    let text = parse_tool_response(&resp);
    assert_eq!(text["result"], "errors");
    assert!(text["errors"].is_array());
}

#[test]
fn test_tools_call_beacon_status() {
    let state = make_state();

    // Status when empty
    let req = make_request(
        "tools/call",
        serde_json::json!({
            "name": "beacon_status",
            "arguments": {}
        }),
    );
    let resp = handle_request(&req, &state);
    let text = parse_tool_response(&resp);
    assert_eq!(text["state"], "idle");
    assert_eq!(text["active_campaigns"], 0);

    // Compile something, then check status
    let ir_json = include_str!("../../beacon-ir/tests/fixtures/document_lifecycle.json");
    let compile_req = make_request(
        "tools/call",
        serde_json::json!({
            "name": "beacon_compile",
            "arguments": { "ir_json": ir_json }
        }),
    );
    handle_request(&compile_req, &state);

    let resp = handle_request(&req, &state);
    let text = parse_tool_response(&resp);
    assert_eq!(text["active_campaigns"], 1);
}

#[test]
fn test_unknown_tool() {
    let state = make_state();
    let req = make_request(
        "tools/call",
        serde_json::json!({
            "name": "nonexistent_tool",
            "arguments": {}
        }),
    );
    let resp = handle_request(&req, &state);
    assert!(resp["result"]["isError"].as_bool().unwrap_or(false));
}

#[test]
fn test_unknown_method() {
    let state = make_state();
    let req = make_request("nonexistent/method", serde_json::json!({}));
    let resp = handle_request(&req, &state);
    assert!(resp["error"].is_object());
    assert_eq!(resp["error"]["code"], -32601);
}

// --- New MCP tool tests ---

fn compile_campaign(state: &McpState) -> String {
    let ir_json = include_str!("../../beacon-ir/tests/fixtures/document_lifecycle.json");
    let req = make_request(
        "tools/call",
        serde_json::json!({
            "name": "beacon_compile",
            "arguments": { "ir_json": ir_json }
        }),
    );
    let resp = handle_request(&req, state);
    let text = parse_tool_response(&resp);
    text["campaign_id"].as_str().unwrap().to_string()
}

#[test]
fn test_fuzz_start() {
    let state = McpState::new();
    let campaign_id = compile_campaign(&state);

    let req = make_request(
        "tools/call",
        serde_json::json!({
            "name": "beacon_fuzz_start",
            "arguments": { "campaign_id": campaign_id }
        }),
    );
    let resp = handle_request(&req, &state);
    let text = parse_tool_response(&resp);
    assert_eq!(text["status"], "started");
    assert_eq!(text["campaign_id"], campaign_id);
}

#[test]
fn test_fuzz_start_missing_campaign() {
    let state = McpState::new();
    let req = make_request(
        "tools/call",
        serde_json::json!({
            "name": "beacon_fuzz_start",
            "arguments": { "campaign_id": "nonexistent" }
        }),
    );
    let resp = handle_request(&req, &state);
    assert!(resp["result"]["isError"].as_bool().unwrap_or(false));
}

#[test]
fn test_fuzz_status() {
    let state = McpState::new();
    let campaign_id = compile_campaign(&state);

    let req = make_request(
        "tools/call",
        serde_json::json!({
            "name": "beacon_fuzz_status",
            "arguments": { "campaign_id": campaign_id }
        }),
    );
    let resp = handle_request(&req, &state);
    let text = parse_tool_response(&resp);
    assert_eq!(text["state"], "pending"); // Not started yet
    assert!(text["progress"]["iterations_total"].is_number());
}

#[test]
fn test_findings_empty() {
    let state = McpState::new();
    let campaign_id = compile_campaign(&state);

    let req = make_request(
        "tools/call",
        serde_json::json!({
            "name": "beacon_findings",
            "arguments": { "campaign_id": campaign_id }
        }),
    );
    let resp = handle_request(&req, &state);
    let text = parse_tool_response(&resp);
    assert_eq!(text["total_findings"], 0);
    assert_eq!(text["findings"].as_array().unwrap().len(), 0);
}

#[test]
fn test_coverage_empty() {
    let state = McpState::new();
    let campaign_id = compile_campaign(&state);

    let req = make_request(
        "tools/call",
        serde_json::json!({
            "name": "beacon_coverage",
            "arguments": { "campaign_id": campaign_id }
        }),
    );
    let resp = handle_request(&req, &state);
    let text = parse_tool_response(&resp);
    assert!(text["summary"]["hit"].is_number());
}

#[test]
fn test_abort_campaign() {
    let state = McpState::new();
    let campaign_id = compile_campaign(&state);

    let req = make_request(
        "tools/call",
        serde_json::json!({
            "name": "beacon_abort",
            "arguments": { "campaign_id": campaign_id }
        }),
    );
    let resp = handle_request(&req, &state);
    let text = parse_tool_response(&resp);
    assert_eq!(text["campaign_id"], campaign_id);
    assert_eq!(text["final_status"], "Aborted");
}

#[test]
fn test_analytics() {
    let state = McpState::new();
    let campaign_id = compile_campaign(&state);

    let req = make_request(
        "tools/call",
        serde_json::json!({
            "name": "beacon_analytics",
            "arguments": { "campaign_id": campaign_id }
        }),
    );
    let resp = handle_request(&req, &state);
    let text = parse_tool_response(&resp);
    assert_eq!(text["campaign_id"], campaign_id);
    assert!(text["summary"]["total_steps"].is_number());
    assert!(text["summary"]["state"].is_string());
}

#[test]
fn test_fuzz_lifecycle() {
    let state = McpState::new();
    let campaign_id = compile_campaign(&state);

    // Start fuzzing.
    let start_req = make_request(
        "tools/call",
        serde_json::json!({
            "name": "beacon_fuzz_start",
            "arguments": { "campaign_id": campaign_id }
        }),
    );
    let resp = handle_request(&start_req, &state);
    let text = parse_tool_response(&resp);
    assert_eq!(text["status"], "started");

    // Check status — should be running.
    let status_req = make_request(
        "tools/call",
        serde_json::json!({
            "name": "beacon_fuzz_status",
            "arguments": { "campaign_id": campaign_id }
        }),
    );
    let resp = handle_request(&status_req, &state);
    let text = parse_tool_response(&resp);
    assert_eq!(text["state"], "running");

    // Abort.
    let abort_req = make_request(
        "tools/call",
        serde_json::json!({
            "name": "beacon_abort",
            "arguments": { "campaign_id": campaign_id }
        }),
    );
    let resp = handle_request(&abort_req, &state);
    let text = parse_tool_response(&resp);
    assert_eq!(text["final_status"], "Aborted");

    // Verify aborted status.
    let resp = handle_request(&status_req, &state);
    let text = parse_tool_response(&resp);
    assert_eq!(text["state"], "aborted");
}
