use fresnel_fir_model::state::TraceEntry;
use fresnel_fir_model::temporal::{check_temporal, TemporalRule};

fn make_trace(actions: &[(&str, &[(&str, &str)])]) -> Vec<TraceEntry> {
    actions
        .iter()
        .enumerate()
        .map(|(i, (action, args))| TraceEntry {
            action: action.to_string(),
            args: args
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            generation: i as u64,
        })
        .collect()
}

#[test]
fn test_auth_before_mutation_passes() {
    // All mutations have authenticated=true in their args
    let trace = make_trace(&[
        ("create_document", &[("actor_authenticated", "true")]),
        ("read", &[("actor_authenticated", "true")]),
        ("publish", &[("actor_authenticated", "true")]),
        ("delete", &[("actor_authenticated", "true")]),
    ]);

    let rules = vec![TemporalRule::BeforeMutation {
        name: "auth_before_mutation".to_string(),
        mutating_actions: vec![
            "create_document".to_string(),
            "publish".to_string(),
            "delete".to_string(),
            "archive".to_string(),
            "restore".to_string(),
        ],
        required_arg: "actor_authenticated".to_string(),
        required_value: "true".to_string(),
    }];

    let violations = check_temporal(&trace, &rules);
    assert!(
        violations.is_empty(),
        "Expected no violations, got: {:?}",
        violations
    );
}

#[test]
fn test_auth_before_mutation_fails() {
    // Mutation without authentication
    let trace = make_trace(&[
        ("create_document", &[("actor_authenticated", "true")]),
        ("publish", &[("actor_authenticated", "false")]), // violation!
    ]);

    let rules = vec![TemporalRule::BeforeMutation {
        name: "auth_before_mutation".to_string(),
        mutating_actions: vec![
            "create_document".to_string(),
            "publish".to_string(),
            "delete".to_string(),
        ],
        required_arg: "actor_authenticated".to_string(),
        required_value: "true".to_string(),
    }];

    let violations = check_temporal(&trace, &rules);
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].rule_name, "auth_before_mutation");
    assert!(violations[0].message.contains("publish"));
}

#[test]
fn test_delete_is_permanent_passes() {
    // Delete then no restore on the same entity
    let trace = make_trace(&[
        ("create_document", &[("entity_id", "doc1")]),
        ("read", &[("entity_id", "doc1")]),
        ("delete", &[("entity_id", "doc1")]),
        ("read", &[("entity_id", "doc2")]), // different entity, OK
    ]);

    let rules = vec![TemporalRule::AfterNever {
        name: "delete_is_permanent".to_string(),
        trigger_action: "delete".to_string(),
        forbidden_action: "restore".to_string(),
        same_entity_key: "entity_id".to_string(),
    }];

    let violations = check_temporal(&trace, &rules);
    assert!(violations.is_empty());
}

#[test]
fn test_delete_is_permanent_fails() {
    // Delete then restore on the same entity
    let trace = make_trace(&[
        ("create_document", &[("entity_id", "doc1")]),
        ("delete", &[("entity_id", "doc1")]),
        ("restore", &[("entity_id", "doc1")]), // violation!
    ]);

    let rules = vec![TemporalRule::AfterNever {
        name: "delete_is_permanent".to_string(),
        trigger_action: "delete".to_string(),
        forbidden_action: "restore".to_string(),
        same_entity_key: "entity_id".to_string(),
    }];

    let violations = check_temporal(&trace, &rules);
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].rule_name, "delete_is_permanent");
    assert!(violations[0].message.contains("restore"));
    assert!(violations[0].message.contains("doc1"));
}

#[test]
fn test_delete_different_entity_ok() {
    // Delete doc1, then restore doc2 — allowed
    let trace = make_trace(&[
        ("delete", &[("entity_id", "doc1")]),
        ("restore", &[("entity_id", "doc2")]), // different entity, OK
    ]);

    let rules = vec![TemporalRule::AfterNever {
        name: "delete_is_permanent".to_string(),
        trigger_action: "delete".to_string(),
        forbidden_action: "restore".to_string(),
        same_entity_key: "entity_id".to_string(),
    }];

    let violations = check_temporal(&trace, &rules);
    assert!(violations.is_empty());
}

#[test]
fn test_empty_trace_no_violations() {
    let trace = vec![];
    let rules = vec![
        TemporalRule::BeforeMutation {
            name: "auth_before_mutation".to_string(),
            mutating_actions: vec!["create_document".to_string()],
            required_arg: "actor_authenticated".to_string(),
            required_value: "true".to_string(),
        },
        TemporalRule::AfterNever {
            name: "delete_is_permanent".to_string(),
            trigger_action: "delete".to_string(),
            forbidden_action: "restore".to_string(),
            same_entity_key: "entity_id".to_string(),
        },
    ];

    let violations = check_temporal(&trace, &rules);
    assert!(violations.is_empty());
}
