use beacon_model::state::{ModelState, Value};

#[test]
fn test_new_model_state_is_empty() {
    let state = ModelState::new();
    assert_eq!(state.generation(), 0);
    assert!(state.all_instances("User").is_empty());
}

#[test]
fn test_create_entity_instance() {
    let mut state = ModelState::new();
    let id = state.create_instance("User");
    assert_eq!(state.all_instances("User").len(), 1);
    assert!(state.get_instance(&id).is_some());
    assert_eq!(state.generation(), 1);
}

#[test]
fn test_set_and_get_field() {
    let mut state = ModelState::new();
    let id = state.create_instance("User");
    state.set_field(&id, "role", Value::String("admin".to_string()));
    let inst = state.get_instance(&id).unwrap();
    assert_eq!(
        inst.get_field("role"),
        Some(&Value::String("admin".to_string()))
    );
}

#[test]
fn test_generation_increments_on_mutation() {
    let mut state = ModelState::new();
    assert_eq!(state.generation(), 0);
    let id = state.create_instance("Document");
    assert_eq!(state.generation(), 1);
    state.set_field(&id, "visibility", Value::String("private".to_string()));
    assert_eq!(state.generation(), 2);
}

#[test]
fn test_fork_creates_independent_copy() {
    let mut state = ModelState::new();
    let id = state.create_instance("User");
    state.set_field(&id, "role", Value::String("admin".to_string()));

    let mut forked = state.fork();
    forked.set_field(&id, "role", Value::String("guest".to_string()));

    // Original unchanged
    let orig_inst = state.get_instance(&id).unwrap();
    assert_eq!(
        orig_inst.get_field("role"),
        Some(&Value::String("admin".to_string()))
    );

    // Fork has the new value
    let fork_inst = forked.get_instance(&id).unwrap();
    assert_eq!(
        fork_inst.get_field("role"),
        Some(&Value::String("guest".to_string()))
    );
}

#[test]
fn test_snapshot_and_rollback() {
    let mut state = ModelState::new();
    let id = state.create_instance("User");
    state.set_field(&id, "role", Value::String("admin".to_string()));

    let snapshot = state.snapshot();

    // Mutate after snapshot
    state.set_field(&id, "role", Value::String("guest".to_string()));
    state.create_instance("Document");
    assert_eq!(state.all_instances("Document").len(), 1);

    // Rollback
    state.rollback(snapshot);
    let inst = state.get_instance(&id).unwrap();
    assert_eq!(
        inst.get_field("role"),
        Some(&Value::String("admin".to_string()))
    );
    assert!(state.all_instances("Document").is_empty());
}

#[test]
fn test_multiple_entity_types() {
    let mut state = ModelState::new();
    let user_id = state.create_instance("User");
    let doc_id = state.create_instance("Document");
    state.set_field(&user_id, "role", Value::String("admin".to_string()));
    state.set_field(&doc_id, "visibility", Value::String("private".to_string()));

    assert_eq!(state.all_instances("User").len(), 1);
    assert_eq!(state.all_instances("Document").len(), 1);
    assert!(state.get_instance(&user_id).is_some());
    assert!(state.get_instance(&doc_id).is_some());
}

#[test]
fn test_fork_shares_data_until_write() {
    let mut state = ModelState::new();
    for _ in 0..10 {
        state.create_instance("User");
    }

    // Fork should be cheap (shares underlying data)
    let forked = state.fork();
    assert_eq!(forked.all_instances("User").len(), 10);
    // Generation resets are independent
    assert_eq!(state.generation(), forked.generation());
}

#[test]
fn test_trace_records_actions() {
    let mut state = ModelState::new();
    state.record_action("create_document", &[("actor_id", "user1")]);
    state.record_action("read", &[("actor_id", "user1"), ("doc_id", "doc1")]);

    let trace = state.trace();
    assert_eq!(trace.len(), 2);
    assert_eq!(trace[0].action, "create_document");
    assert_eq!(trace[1].action, "read");
}
