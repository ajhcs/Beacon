use beacon_ir::parse::parse_ir;

#[test]
fn test_parse_full_ir_from_file() {
    let json_str = include_str!("fixtures/document_lifecycle.json");
    let ir = parse_ir(json_str).unwrap();
    assert_eq!(ir.entities.len(), 2); // User, Document
    assert!(ir.entities.contains_key("User"));
    assert!(ir.entities.contains_key("Document"));
    assert!(!ir.refinements.is_empty());
    assert!(!ir.protocols.is_empty());
    assert!(!ir.effects.is_empty());
    assert!(!ir.properties.is_empty());
}

#[test]
fn test_parse_invalid_json() {
    let result = parse_ir("not json at all");
    assert!(result.is_err());
}

#[test]
fn test_parse_empty_sections() {
    let json = r#"{
        "entities": {},
        "refinements": {},
        "functions": {},
        "protocols": {},
        "effects": {},
        "properties": {},
        "generators": {},
        "exploration": {
            "weights": { "scope": "per_alt_branch_and_model_state", "initial": "from_protocol", "decay": "per_epoch" },
            "directives_allowed": [],
            "adaptation_signals": [],
            "strategy": { "initial": "pseudo_random_traversal", "fallback": "targeted_on_violation" },
            "epoch_size": 100,
            "coverage_floor_threshold": 0.05,
            "concurrency": { "mode": "deterministic_interleaving", "threads": 4 }
        },
        "inputs": {
            "domains": {},
            "constraints": [],
            "coverage": { "targets": [], "seed": 42, "reproducible": true }
        },
        "bindings": {
            "runtime": "wasm",
            "entry": "main.wasm",
            "actions": {},
            "event_hooks": { "mode": "function_intercept", "observe": [], "capture": [] }
        }
    }"#;
    let ir = parse_ir(json).unwrap();
    assert!(ir.entities.is_empty());
}
