use beacon_core::campaign::{CampaignManager, CampaignState};

#[test]
fn test_new_manager_is_empty() {
    let manager = CampaignManager::new();
    assert_eq!(manager.active_campaign_count(), 0);
}

#[test]
fn test_compile_valid_ir_creates_campaign() {
    let manager = CampaignManager::new();
    let json = include_str!("../../beacon-ir/tests/fixtures/document_lifecycle.json");
    let result = manager.compile(json);
    assert!(result.is_ok(), "Compile failed: {:?}", result.unwrap_err());

    let campaign_id = result.unwrap();
    assert_eq!(manager.active_campaign_count(), 1);

    let state = manager.get_campaign(&campaign_id);
    assert!(state.is_some());
}

#[test]
fn test_compile_invalid_ir_returns_error() {
    let manager = CampaignManager::new();
    let result = manager.compile("not json");
    assert!(result.is_err());
    assert_eq!(manager.active_campaign_count(), 0);
}

#[test]
fn test_compile_structurally_invalid_ir_returns_error() {
    let manager = CampaignManager::new();
    let json = r#"{
        "entities": {},
        "refinements": { "Bad": { "base": "Ghost", "predicate": true } },
        "functions": {},
        "protocols": {},
        "effects": {},
        "properties": {},
        "generators": {},
        "exploration": { "weights": { "scope": "per_alt_branch_and_model_state", "initial": "from_protocol", "decay": "per_epoch" }, "directives_allowed": [], "adaptation_signals": [], "strategy": { "initial": "pseudo_random_traversal", "fallback": "targeted_on_violation" }, "epoch_size": 100, "coverage_floor_threshold": 0.05, "concurrency": { "mode": "deterministic_interleaving", "threads": 4 } },
        "inputs": { "domains": {}, "constraints": [], "coverage": { "targets": [], "seed": 42, "reproducible": true } },
        "bindings": { "runtime": "wasm", "entry": "main.wasm", "actions": {}, "event_hooks": { "mode": "function_intercept", "observe": [], "capture": [] } }
    }"#;
    let result = manager.compile(json);
    assert!(result.is_err());
    assert_eq!(manager.active_campaign_count(), 0);
}

#[test]
fn test_multiple_campaigns() {
    let manager = CampaignManager::new();
    let json = include_str!("../../beacon-ir/tests/fixtures/document_lifecycle.json");

    let id1 = manager.compile(json).unwrap();
    let id2 = manager.compile(json).unwrap();

    assert_ne!(id1, id2);
    assert_eq!(manager.active_campaign_count(), 2);
}

#[test]
fn test_campaign_has_compiled_ir() {
    let manager = CampaignManager::new();
    let json = include_str!("../../beacon-ir/tests/fixtures/document_lifecycle.json");
    let campaign_id = manager.compile(json).unwrap();

    let state = manager.get_campaign(&campaign_id).unwrap();
    assert!(!state.compiled.graphs.is_empty());
    assert!(!state.compiled.predicates.is_empty());
}

#[test]
fn test_campaign_budget_estimates() {
    let manager = CampaignManager::new();
    let json = include_str!("../../beacon-ir/tests/fixtures/document_lifecycle.json");
    let campaign_id = manager.compile(json).unwrap();

    let state = manager.get_campaign(&campaign_id).unwrap();
    assert!(state.budget.min_iterations > 0);
    assert!(state.budget.min_timeout_secs > 0);
}
