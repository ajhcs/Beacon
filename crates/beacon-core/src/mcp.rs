use serde_json::{json, Value};

use crate::analytics::CampaignPhase;
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
            },
            {
                "name": "beacon_fuzz_start",
                "description": "Start a fuzzing campaign against a compiled specification",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "campaign_id": {
                            "type": "string",
                            "description": "Campaign ID from beacon_compile"
                        },
                        "extra_iterations": {
                            "type": "integer",
                            "description": "Additional iterations beyond computed minimum (optional)"
                        }
                    },
                    "required": ["campaign_id"]
                }
            },
            {
                "name": "beacon_fuzz_status",
                "description": "Get the current status of a fuzzing campaign",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "campaign_id": {
                            "type": "string",
                            "description": "Campaign ID"
                        }
                    },
                    "required": ["campaign_id"]
                }
            },
            {
                "name": "beacon_findings",
                "description": "Get findings from a campaign, optionally since a sequence number for incremental polling",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "campaign_id": {
                            "type": "string",
                            "description": "Campaign ID"
                        },
                        "since_seqno": {
                            "type": "integer",
                            "description": "Only return findings after this sequence number (for incremental polling)"
                        }
                    },
                    "required": ["campaign_id"]
                }
            },
            {
                "name": "beacon_coverage",
                "description": "Get coverage data for a campaign",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "campaign_id": {
                            "type": "string",
                            "description": "Campaign ID"
                        }
                    },
                    "required": ["campaign_id"]
                }
            },
            {
                "name": "beacon_abort",
                "description": "Abort a running campaign and get final status",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "campaign_id": {
                            "type": "string",
                            "description": "Campaign ID"
                        }
                    },
                    "required": ["campaign_id"]
                }
            },
            {
                "name": "beacon_analytics",
                "description": "Get detailed analytics for a campaign including coverage curves, finding rates, and adaptation effectiveness",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "campaign_id": {
                            "type": "string",
                            "description": "Campaign ID"
                        }
                    },
                    "required": ["campaign_id"]
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
        "beacon_fuzz_start" => tool_beacon_fuzz_start(&arguments, state),
        "beacon_fuzz_status" => tool_beacon_fuzz_status(&arguments, state),
        "beacon_findings" => tool_beacon_findings(&arguments, state),
        "beacon_coverage" => tool_beacon_coverage(&arguments, state),
        "beacon_abort" => tool_beacon_abort(&arguments, state),
        "beacon_analytics" => tool_beacon_analytics(&arguments, state),
        _ => tool_error(&format!("Unknown tool: {tool_name}")),
    }
}

fn tool_beacon_compile(args: &Value, state: &McpState) -> Value {
    let ir_json = args.get("ir_json").and_then(|v| v.as_str()).unwrap_or("");

    match state.manager.compile(ir_json) {
        Ok(campaign_id) => {
            let campaign = state.manager.get_campaign(&campaign_id);
            let budget = campaign
                .map(|c| {
                    json!({
                        "min_iterations": c.budget.min_iterations,
                        "min_timeout_secs": c.budget.min_timeout_secs,
                    })
                })
                .unwrap_or(json!(null));

            tool_success(json!({
                "result": "pass",
                "campaign_id": campaign_id,
                "budget": budget,
            }))
        }
        Err(e) => tool_success(json!({
            "result": "errors",
            "errors": [e.to_string()],
        })),
    }
}

fn tool_beacon_status(state: &McpState) -> Value {
    let count = state.manager.active_campaign_count();
    let engine_state = if count > 0 { "active" } else { "idle" };

    tool_success(json!({
        "state": engine_state,
        "active_campaigns": count,
    }))
}

fn tool_beacon_fuzz_start(args: &Value, state: &McpState) -> Value {
    let campaign_id = match args.get("campaign_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => return tool_error("Missing required parameter: campaign_id"),
    };

    let campaign = match state.manager.get_campaign(campaign_id) {
        Some(c) => c,
        None => return tool_error(&format!("Campaign not found: {campaign_id}")),
    };

    // Validate campaign is in correct phase.
    if campaign.phase != CampaignPhase::Compiled && campaign.phase != CampaignPhase::DutLoaded {
        return tool_error(&format!(
            "Campaign {} is in {:?} phase, expected Compiled or DutLoaded",
            campaign_id, campaign.phase
        ));
    }

    // Transition to Running.
    if let Err(e) = state
        .manager
        .set_phase(campaign_id, CampaignPhase::Running)
    {
        return tool_error(&e.to_string());
    }

    let _extra = args
        .get("extra_iterations")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    tool_success(json!({
        "status": "started",
        "campaign_id": campaign_id,
        "budget": {
            "min_iterations": campaign.budget.min_iterations,
            "min_timeout_secs": campaign.budget.min_timeout_secs,
        },
    }))
}

fn tool_beacon_fuzz_status(args: &Value, state: &McpState) -> Value {
    let campaign_id = match args.get("campaign_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => return tool_error("Missing required parameter: campaign_id"),
    };

    let campaign = match state.manager.get_campaign(campaign_id) {
        Some(c) => c,
        None => return tool_error(&format!("Campaign not found: {campaign_id}")),
    };

    let state_str = match campaign.phase {
        CampaignPhase::Compiled | CampaignPhase::DutLoaded => "pending",
        CampaignPhase::Running => "running",
        CampaignPhase::Complete => "complete",
        CampaignPhase::Aborted => "aborted",
    };

    let coverage_percent = if campaign.coverage_total > 0 {
        (campaign.coverage_hit as f64 / campaign.coverage_total as f64) * 100.0
    } else {
        0.0
    };

    tool_success(json!({
        "state": state_str,
        "progress": {
            "iterations_done": campaign.steps_executed,
            "iterations_total": campaign.budget.min_iterations,
        },
        "coverage": {
            "targets_hit": campaign.coverage_hit,
            "targets_total": campaign.coverage_total,
            "percent": coverage_percent,
        },
        "findings_count": campaign.findings_count,
        "stop_reason": campaign.stop_reason.as_ref().map(|r| format!("{:?}", r)),
    }))
}

fn tool_beacon_findings(args: &Value, state: &McpState) -> Value {
    let campaign_id = match args.get("campaign_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => return tool_error("Missing required parameter: campaign_id"),
    };

    if state.manager.get_campaign(campaign_id).is_none() {
        return tool_error(&format!("Campaign not found: {campaign_id}"));
    }

    let since_seqno = args.get("since_seqno").and_then(|v| v.as_u64());
    let findings = state.manager.get_findings(campaign_id, since_seqno);

    let next_seqno = findings.last().map(|f| f.seqno + 1).unwrap_or(0);

    tool_success(json!({
        "findings": findings,
        "next_seqno": next_seqno,
        "total_findings": findings.len(),
    }))
}

fn tool_beacon_coverage(args: &Value, state: &McpState) -> Value {
    let campaign_id = match args.get("campaign_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => return tool_error("Missing required parameter: campaign_id"),
    };

    let campaign = match state.manager.get_campaign(campaign_id) {
        Some(c) => c,
        None => return tool_error(&format!("Campaign not found: {campaign_id}")),
    };

    let targets = state.manager.get_coverage(campaign_id);
    let hit = targets.iter().filter(|t| t.status == "hit").count();
    let pending = targets.iter().filter(|t| t.status == "pending").count();
    let unreachable = targets
        .iter()
        .filter(|t| t.status == "unreachable")
        .count();

    let percent = if campaign.coverage_total > 0 {
        (campaign.coverage_hit as f64 / campaign.coverage_total as f64) * 100.0
    } else {
        0.0
    };

    tool_success(json!({
        "targets": targets,
        "summary": {
            "hit": hit,
            "pending": pending,
            "unreachable": unreachable,
            "percent": percent,
        },
    }))
}

fn tool_beacon_abort(args: &Value, state: &McpState) -> Value {
    let campaign_id = match args.get("campaign_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => return tool_error("Missing required parameter: campaign_id"),
    };

    match state.manager.abort(campaign_id) {
        Ok(final_state) => tool_success(json!({
            "campaign_id": campaign_id,
            "final_status": format!("{:?}", final_state.phase),
            "findings_count": final_state.findings_count,
            "steps_executed": final_state.steps_executed,
        })),
        Err(e) => tool_error(&e.to_string()),
    }
}

fn tool_beacon_analytics(args: &Value, state: &McpState) -> Value {
    let campaign_id = match args.get("campaign_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => return tool_error("Missing required parameter: campaign_id"),
    };

    match state.manager.get_analytics(campaign_id) {
        Some(analytics) => {
            let summary = analytics.summary();
            tool_success(json!({
                "campaign_id": campaign_id,
                "summary": {
                    "total_steps": summary.total_steps,
                    "total_findings": summary.total_findings,
                    "peak_coverage": summary.peak_coverage,
                    "elapsed_secs": summary.elapsed_secs,
                    "finding_rate_per_k": summary.finding_rate_per_k,
                    "coverage_velocity": summary.coverage_velocity,
                    "adaptation_effectiveness": summary.adaptation_effectiveness,
                    "epochs_completed": summary.epochs_completed,
                    "state": format!("{:?}", summary.state),
                },
                "coverage_curve_points": analytics.coverage_curve.len(),
                "epoch_stats_count": analytics.epoch_stats.len(),
            }))
        }
        None => tool_error(&format!("Campaign not found: {campaign_id}")),
    }
}

/// Build a successful MCP tool response.
fn tool_success(data: Value) -> Value {
    json!({
        "content": [{
            "type": "text",
            "text": data.to_string()
        }]
    })
}

/// Build an MCP tool error response.
fn tool_error(message: &str) -> Value {
    json!({
        "isError": true,
        "content": [{
            "type": "text",
            "text": json!({"error": message}).to_string()
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
