use std::collections::HashMap;

use beacon_ir::types::{ActionBinding, Bindings, EventHooks};
use beacon_vif::validate::{validate_interface, validate_signatures, InterfaceError};

fn make_bindings(actions: Vec<(&str, &str, Vec<&str>, &str)>) -> Bindings {
    let mut action_map = HashMap::new();
    for (action_name, func_name, args, ret_type) in actions {
        action_map.insert(
            action_name.to_string(),
            ActionBinding {
                function: func_name.to_string(),
                args: args.into_iter().map(|s| s.to_string()).collect(),
                returns: serde_json::json!({ "type": ret_type }),
                mutates: true,
                idempotent: false,
                reads: vec![],
                writes: vec![],
            },
        );
    }
    Bindings {
        runtime: "wasm".to_string(),
        entry: "main.wasm".to_string(),
        actions: action_map,
        event_hooks: EventHooks {
            mode: "function_intercept".to_string(),
            observe: vec![],
            capture: vec![],
        },
    }
}

#[test]
fn test_valid_interface() {
    let exports = vec![
        ("create_document".to_string(), "func".to_string()),
        ("get_document".to_string(), "func".to_string()),
        ("delete_document".to_string(), "func".to_string()),
    ];
    let bindings = make_bindings(vec![
        ("create_document", "create_document", vec!["actor_id"], "Document"),
        ("read", "get_document", vec!["actor_id", "doc_id"], "Document"),
        ("delete", "delete_document", vec!["actor_id", "doc_id"], "void"),
    ]);

    let result = validate_interface(&exports, &bindings);
    assert!(result.is_ok());
}

#[test]
fn test_missing_export() {
    let exports = vec![
        ("create_document".to_string(), "func".to_string()),
        // get_document is MISSING
    ];
    let bindings = make_bindings(vec![
        ("create_document", "create_document", vec!["actor_id"], "Document"),
        ("read", "get_document", vec!["actor_id", "doc_id"], "Document"),
    ]);

    let result = validate_interface(&exports, &bindings);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 1);
    assert!(matches!(
        &errors[0],
        InterfaceError::MissingExport { action, function }
        if action == "read" && function == "get_document"
    ));
}

#[test]
fn test_wrong_export_kind() {
    let exports = vec![
        ("create_document".to_string(), "func".to_string()),
        ("get_document".to_string(), "memory".to_string()), // wrong kind!
    ];
    let bindings = make_bindings(vec![
        ("create_document", "create_document", vec!["actor_id"], "Document"),
        ("read", "get_document", vec!["actor_id", "doc_id"], "Document"),
    ]);

    let result = validate_interface(&exports, &bindings);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| matches!(
        e,
        InterfaceError::WrongExportKind { action, .. } if action == "read"
    )));
}

#[test]
fn test_multiple_missing_exports() {
    let exports = vec![];
    let bindings = make_bindings(vec![
        ("create_document", "create_document", vec!["actor_id"], "Document"),
        ("read", "get_document", vec!["actor_id", "doc_id"], "Document"),
        ("delete", "delete_document", vec!["actor_id", "doc_id"], "void"),
    ]);

    let result = validate_interface(&exports, &bindings);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 3);
}

#[test]
fn test_signature_validation_matches() {
    let mut sigs = HashMap::new();
    sigs.insert("create_document".to_string(), (1usize, 1usize));
    sigs.insert("get_document".to_string(), (2, 1));
    sigs.insert("delete_document".to_string(), (2, 0));

    let bindings = make_bindings(vec![
        ("create_document", "create_document", vec!["actor_id"], "Document"),
        ("read", "get_document", vec!["actor_id", "doc_id"], "Document"),
        ("delete", "delete_document", vec!["actor_id", "doc_id"], "void"),
    ]);

    let result = validate_signatures(&sigs, &bindings);
    assert!(result.is_ok());
    let reports = result.unwrap();
    assert!(reports.iter().all(|r| r.matches));
}

#[test]
fn test_signature_param_mismatch() {
    let mut sigs = HashMap::new();
    sigs.insert("create_document".to_string(), (2usize, 1usize)); // expects 1 param, has 2

    let bindings = make_bindings(vec![
        ("create_document", "create_document", vec!["actor_id"], "Document"),
    ]);

    let result = validate_signatures(&sigs, &bindings);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| matches!(
        e,
        InterfaceError::ParamCountMismatch { expected_params: 1, found_params: 2, .. }
    )));
}

#[test]
fn test_signature_return_mismatch() {
    let mut sigs = HashMap::new();
    sigs.insert("delete_document".to_string(), (2usize, 1usize)); // expects 0 returns (void), has 1

    let bindings = make_bindings(vec![
        ("delete", "delete_document", vec!["actor_id", "doc_id"], "void"),
    ]);

    let result = validate_signatures(&sigs, &bindings);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| matches!(
        e,
        InterfaceError::ReturnCountMismatch { expected_returns: 0, found_returns: 1, .. }
    )));
}
