use fresnel_fir_ir::types::FresnelFirIR;

#[test]
fn test_parse_minimal_ir() {
    let json = serde_json::json!({
        "entities": {
            "User": {
                "fields": {
                    "id": { "type": "string", "format": "uuid" },
                    "role": { "type": "enum", "values": ["admin", "guest"] }
                }
            }
        },
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
    });
    let ir: FresnelFirIR = serde_json::from_value(json).unwrap();
    assert_eq!(ir.entities.len(), 1);
    assert!(ir.entities.contains_key("User"));
}

#[test]
fn test_parse_entity_fields() {
    let json = serde_json::json!({
        "type": "enum",
        "values": ["admin", "member", "guest"]
    });
    let field: fresnel_fir_ir::types::FieldDef = serde_json::from_value(json).unwrap();
    assert!(matches!(
        field.field_type,
        fresnel_fir_ir::types::FieldType::Enum { .. }
    ));
}

#[test]
fn test_parse_protocol_with_grammar_constructs() {
    let json = serde_json::json!({
        "root": {
            "type": "seq",
            "children": [
                { "type": "call", "action": "create" },
                {
                    "type": "alt",
                    "branches": [
                        { "id": "a", "weight": 50, "body": { "type": "call", "action": "read" } },
                        { "id": "b", "weight": 50, "body": { "type": "call", "action": "delete" } }
                    ]
                }
            ]
        }
    });
    let proto: fresnel_fir_ir::types::Protocol = serde_json::from_value(json).unwrap();
    assert!(matches!(
        proto.root,
        fresnel_fir_ir::types::ProtocolNode::Seq { .. }
    ));
}

#[test]
fn test_parse_effect() {
    let json = serde_json::json!({
        "creates": { "entity": "Document", "assign": "doc" },
        "sets": [
            { "target": ["doc", "visibility"], "value": "private" }
        ]
    });
    let effect: fresnel_fir_ir::types::Effect = serde_json::from_value(json).unwrap();
    assert!(effect.creates.is_some());
    assert_eq!(effect.sets.len(), 1);
}

#[test]
fn test_parse_action_binding() {
    let json = serde_json::json!({
        "function": "create_document",
        "args": ["actor_id"],
        "returns": { "type": "Document" },
        "mutates": true,
        "idempotent": false,
        "reads": [],
        "writes": ["Document"]
    });
    let binding: fresnel_fir_ir::types::ActionBinding = serde_json::from_value(json).unwrap();
    assert!(binding.mutates);
    assert!(!binding.idempotent);
}
