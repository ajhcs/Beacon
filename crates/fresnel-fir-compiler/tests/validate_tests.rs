use fresnel_fir_compiler::validate::{validate_ir, ValidationError};
use fresnel_fir_ir::parse::parse_ir;

/// Helper to build a minimal valid IR JSON with overrides for specific sections.
fn minimal_ir_json(overrides: &str) -> String {
    // Start with the base template, then apply overrides
    let base = r#"{
        "entities": {},
        "refinements": {},
        "functions": {},
        "protocols": {},
        "effects": {},
        "properties": {},
        "generators": {},
        "exploration": { "weights": { "scope": "per_alt_branch_and_model_state", "initial": "from_protocol", "decay": "per_epoch" }, "directives_allowed": [], "adaptation_signals": [], "strategy": { "initial": "pseudo_random_traversal", "fallback": "targeted_on_violation" }, "epoch_size": 100, "coverage_floor_threshold": 0.05, "concurrency": { "mode": "deterministic_interleaving", "threads": 4 } },
        "inputs": { "domains": {}, "constraints": [], "coverage": { "targets": [], "seed": 42, "reproducible": true } },
        "bindings": { "runtime": "wasm", "entry": "main.wasm", "actions": {}, "event_hooks": { "mode": "function_intercept", "observe": [], "capture": [] } }
    }"#;
    // Merge by parsing both as serde_json::Value
    let mut base_val: serde_json::Value = serde_json::from_str(base).unwrap();
    let overrides_val: serde_json::Value = serde_json::from_str(overrides).unwrap();
    if let (Some(base_obj), Some(over_obj)) = (base_val.as_object_mut(), overrides_val.as_object())
    {
        for (k, v) in over_obj {
            base_obj.insert(k.clone(), v.clone());
        }
    }
    serde_json::to_string(&base_val).unwrap()
}

#[test]
fn test_valid_ir_passes() {
    let json = include_str!("../../fresnel-fir-ir/tests/fixtures/document_lifecycle.json");
    let ir = parse_ir(json).unwrap();
    let result = validate_ir(&ir);
    assert!(
        result.is_ok(),
        "Expected valid IR to pass validation, got: {:?}",
        result.unwrap_err()
    );
}

#[test]
fn test_dangling_entity_ref_in_refinement() {
    let json = minimal_ir_json(
        r#"{
        "refinements": {
            "BadRef": {
                "base": "Ghost",
                "predicate": true
            }
        }
    }"#,
    );
    let ir = parse_ir(&json).unwrap();
    let result = validate_ir(&ir);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors
        .iter()
        .any(|e| matches!(e, ValidationError::DanglingEntityRef { .. })));
}

#[test]
fn test_action_without_effect() {
    let json = minimal_ir_json(
        r#"{
        "entities": {
            "User": { "fields": { "id": { "type": "string" } } }
        },
        "protocols": {
            "test_proto": {
                "root": { "type": "call", "action": "publish" }
            }
        },
        "effects": {},
        "bindings": {
            "runtime": "wasm",
            "entry": "main.wasm",
            "actions": {
                "publish": {
                    "function": "publish",
                    "args": [],
                    "returns": { "type": "void" },
                    "mutates": true,
                    "idempotent": false,
                    "reads": [],
                    "writes": []
                }
            },
            "event_hooks": { "mode": "function_intercept", "observe": [], "capture": [] }
        }
    }"#,
    );
    let ir = parse_ir(&json).unwrap();
    let result = validate_ir(&ir);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors
        .iter()
        .any(|e| matches!(e, ValidationError::MissingEffect { .. })));
}

#[test]
fn test_action_without_binding() {
    let json = minimal_ir_json(
        r#"{
        "entities": {
            "User": { "fields": { "id": { "type": "string" } } }
        },
        "protocols": {
            "test_proto": {
                "root": { "type": "call", "action": "publish" }
            }
        },
        "effects": {
            "publish": { "sets": [] }
        },
        "bindings": {
            "runtime": "wasm",
            "entry": "main.wasm",
            "actions": {},
            "event_hooks": { "mode": "function_intercept", "observe": [], "capture": [] }
        }
    }"#,
    );
    let ir = parse_ir(&json).unwrap();
    let result = validate_ir(&ir);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors
        .iter()
        .any(|e| matches!(e, ValidationError::MissingBinding { .. })));
}

#[test]
fn test_dangling_protocol_ref() {
    let json = minimal_ir_json(
        r#"{
        "protocols": {
            "test_proto": {
                "root": { "type": "ref", "protocol": "nonexistent" }
            }
        }
    }"#,
    );
    let ir = parse_ir(&json).unwrap();
    let result = validate_ir(&ir);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors
        .iter()
        .any(|e| matches!(e, ValidationError::DanglingProtocolRef { .. })));
}

#[test]
fn test_alt_all_zero_weights() {
    let json = minimal_ir_json(
        r#"{
        "protocols": {
            "test_proto": {
                "root": {
                    "type": "alt",
                    "branches": [
                        { "id": "a", "weight": 0, "body": { "type": "call", "action": "read" } },
                        { "id": "b", "weight": 0, "body": { "type": "call", "action": "write" } }
                    ]
                }
            }
        },
        "effects": {
            "read": { "sets": [] },
            "write": { "sets": [] }
        },
        "bindings": {
            "runtime": "wasm",
            "entry": "main.wasm",
            "actions": {
                "read": { "function": "read", "args": [], "returns": { "type": "void" }, "mutates": false, "idempotent": true, "reads": [], "writes": [] },
                "write": { "function": "write", "args": [], "returns": { "type": "void" }, "mutates": true, "idempotent": false, "reads": [], "writes": [] }
            },
            "event_hooks": { "mode": "function_intercept", "observe": [], "capture": [] }
        }
    }"#,
    );
    let ir = parse_ir(&json).unwrap();
    let result = validate_ir(&ir);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors
        .iter()
        .any(|e| matches!(e, ValidationError::AllZeroWeights { .. })));
}

#[test]
fn test_repeat_min_exceeds_max() {
    let json = minimal_ir_json(
        r#"{
        "protocols": {
            "test_proto": {
                "root": {
                    "type": "repeat",
                    "min": 10,
                    "max": 5,
                    "body": { "type": "call", "action": "read" }
                }
            }
        },
        "effects": {
            "read": { "sets": [] }
        },
        "bindings": {
            "runtime": "wasm",
            "entry": "main.wasm",
            "actions": {
                "read": { "function": "read", "args": [], "returns": { "type": "void" }, "mutates": false, "idempotent": true, "reads": [], "writes": [] }
            },
            "event_hooks": { "mode": "function_intercept", "observe": [], "capture": [] }
        }
    }"#,
    );
    let ir = parse_ir(&json).unwrap();
    let result = validate_ir(&ir);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors
        .iter()
        .any(|e| matches!(e, ValidationError::InvalidRepeatBounds { .. })));
}
