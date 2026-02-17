use serde_json::{json, Value};

use crate::campaign::CampaignManager;

/// Server state shared across MCP request handling.
pub struct McpState {
    pub manager: CampaignManager,
}

impl McpState {
    pub fn new() -> Self {
        Self {
            manager: CampaignManager::new(),
        }
    }
}

impl Default for McpState {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle a single JSON-RPC request and return a JSON-RPC response.
pub fn handle_request(req: &Value, state: &McpState) -> Value {
    let id = req.get("id").cloned().unwrap_or(Value::Null);
    let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");

    match method {
        "initialize" => json_rpc_result(id, handle_initialize()),
        "tools/list" => json_rpc_result(id, handle_tools_list()),
        "tools/call" => {
            let params = req.get("params").cloned().unwrap_or(json!({}));
            json_rpc_result(id, handle_tools_call(&params, state))
        }
        _ => json_rpc_error(id, -32601, "Method not found"),
    }
}

fn handle_initialize() -> Value {
    json!({
        "protocolVersion": "2024-11-05",
        "serverInfo": {
            "name": "beacon-core",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "capabilities": {
            "tools": {}
        }
    })
}

fn handle_tools_list() -> Value {
    json!({
        "tools": [
            {
                "name": "beacon_compile",
                "description": "Compile a Beacon IR specification and create a verification campaign",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "ir_json": {
                            "type": "string",
                            "description": "JSON string of the Beacon IR specification"
                        }
                    },
                    "required": ["ir_json"]
                }
            },
            {
                "name": "beacon_status",
                "description": "Get the current status of the Beacon verification engine",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            }
        ]
    })
}

fn handle_tools_call(params: &Value, state: &McpState) -> Value {
    let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    match tool_name {
        "beacon_compile" => tool_beacon_compile(&arguments, state),
        "beacon_status" => tool_beacon_status(state),
        _ => json!({
            "isError": true,
            "content": [{
                "type": "text",
                "text": json!({"error": format!("Unknown tool: {tool_name}")}).to_string()
            }]
        }),
    }
}

fn tool_beacon_compile(args: &Value, state: &McpState) -> Value {
    let ir_json = args.get("ir_json").and_then(|v| v.as_str()).unwrap_or("");

    match state.manager.compile(ir_json) {
        Ok(campaign_id) => {
            let campaign = state.manager.get_campaign(&campaign_id);
            let budget = campaign.map(|c| json!({
                "min_iterations": c.budget.min_iterations,
                "min_timeout_secs": c.budget.min_timeout_secs,
            })).unwrap_or(json!(null));

            json!({
                "content": [{
                    "type": "text",
                    "text": json!({
                        "result": "pass",
                        "campaign_id": campaign_id,
                        "budget": budget,
                    }).to_string()
                }]
            })
        }
        Err(e) => {
            json!({
                "content": [{
                    "type": "text",
                    "text": json!({
                        "result": "errors",
                        "errors": [e.to_string()],
                    }).to_string()
                }]
            })
        }
    }
}

fn tool_beacon_status(state: &McpState) -> Value {
    let count = state.manager.active_campaign_count();
    let engine_state = if count > 0 { "active" } else { "idle" };

    json!({
        "content": [{
            "type": "text",
            "text": json!({
                "state": engine_state,
                "active_campaigns": count,
            }).to_string()
        }]
    })
}

fn json_rpc_result(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
}

fn json_rpc_error(id: Value, code: i32, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message,
        }
    })
}
