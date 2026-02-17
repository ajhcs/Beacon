use beacon_ir::parse::parse_ir;

#[test]
fn test_full_compilation_pipeline() {
    let json = include_str!("../../beacon-ir/tests/fixtures/document_lifecycle.json");
    let ir = parse_ir(json).unwrap();
    let result = beacon_compiler::compile(&ir);
    assert!(result.is_ok(), "Compilation failed: {:?}", result.unwrap_err());
    let compiled = result.unwrap();
    assert!(!compiled.graphs.is_empty());
    assert!(!compiled.predicates.is_empty());
}

#[test]
fn test_compilation_rejects_invalid_ir() {
    let json = r#"{
        "entities": {},
        "refinements": {
            "BadRef": { "base": "Ghost", "predicate": true }
        },
        "functions": {},
        "protocols": {},
        "effects": {},
        "properties": {},
        "generators": {},
        "exploration": { "weights": { "scope": "per_alt_branch_and_model_state", "initial": "from_protocol", "decay": "per_epoch" }, "directives_allowed": [], "adaptation_signals": [], "strategy": { "initial": "pseudo_random_traversal", "fallback": "targeted_on_violation" }, "epoch_size": 100, "coverage_floor_threshold": 0.05, "concurrency": { "mode": "deterministic_interleaving", "threads": 4 } },
        "inputs": { "domains": {}, "constraints": [], "coverage": { "targets": [], "seed": 42, "reproducible": true } },
        "bindings": { "runtime": "wasm", "entry": "main.wasm", "actions": {}, "event_hooks": { "mode": "function_intercept", "observe": [], "capture": [] } }
    }"#;
    let ir = parse_ir(json).unwrap();
    let result = beacon_compiler::compile(&ir);
    assert!(result.is_err());
}

#[test]
fn test_compiled_ir_has_correct_graphs() {
    let json = include_str!("../../beacon-ir/tests/fixtures/document_lifecycle.json");
    let ir = parse_ir(json).unwrap();
    let compiled = beacon_compiler::compile(&ir).unwrap();
    // Should have graphs for both protocols: document_lifecycle and idle
    assert!(compiled.graphs.contains_key("document_lifecycle"));
    assert!(compiled.graphs.contains_key("idle"));
}

#[test]
fn test_compiled_ir_has_predicates_from_refinements() {
    let json = include_str!("../../beacon-ir/tests/fixtures/document_lifecycle.json");
    let ir = parse_ir(json).unwrap();
    let compiled = beacon_compiler::compile(&ir).unwrap();
    // Should have compiled predicates for refinements
    assert!(compiled.predicates.contains_key("AuthenticatedUser"));
    assert!(compiled.predicates.contains_key("OwnedDocument"));
    assert!(compiled.predicates.contains_key("AccessibleDocument"));
}

#[test]
fn test_compiled_ir_has_property_predicates() {
    let json = include_str!("../../beacon-ir/tests/fixtures/document_lifecycle.json");
    let ir = parse_ir(json).unwrap();
    let compiled = beacon_compiler::compile(&ir).unwrap();
    // Should have compiled predicates for invariant properties
    assert!(compiled.predicates.contains_key("property:ownership_isolation"));
}
