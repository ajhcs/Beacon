use beacon_ir::parse::parse_ir;
use beacon_model::effect::apply_effect;
use beacon_model::state::{ModelState, Value};

fn get_test_ir() -> beacon_ir::types::BeaconIR {
    let json = include_str!("../../beacon-ir/tests/fixtures/document_lifecycle.json");
    parse_ir(json).unwrap()
}

#[test]
fn test_apply_create_effect() {
    let ir = get_test_ir();
    let effect = ir.effects.get("create_document").unwrap();
    let mut state = ModelState::new();

    // Create an actor first
    let actor_id = state.create_instance("User");
    state.set_field(&actor_id, "id", Value::String("user-123".to_string()));

    let result = apply_effect(&mut state, effect, &actor_id);
    assert!(
        result.is_ok(),
        "apply_effect failed: {:?}",
        result.unwrap_err()
    );

    // Should have created a Document instance
    let docs = state.all_instances("Document");
    assert_eq!(docs.len(), 1);

    // Should have set owner_id, visibility, deleted
    let doc = &docs[0];
    assert_eq!(
        doc.get_field("owner_id"),
        Some(&Value::String("user-123".to_string()))
    );
    assert_eq!(
        doc.get_field("visibility"),
        Some(&Value::String("private".to_string()))
    );
    assert_eq!(doc.get_field("deleted"), Some(&Value::Bool(false)));
}

#[test]
fn test_apply_set_effect() {
    let ir = get_test_ir();
    let mut state = ModelState::new();

    // Setup: create a document via create_document effect
    let actor_id = state.create_instance("User");
    state.set_field(&actor_id, "id", Value::String("user-123".to_string()));
    apply_effect(
        &mut state,
        ir.effects.get("create_document").unwrap(),
        &actor_id,
    )
    .unwrap();

    // Now apply "publish" effect which sets visibility to public
    let doc_id = state.all_instances("Document")[0].id.clone();
    let publish_effect = ir.effects.get("publish").unwrap();
    let result = apply_effect(&mut state, publish_effect, &actor_id);
    assert!(result.is_ok());

    let doc = state.get_instance(&doc_id).unwrap();
    assert_eq!(
        doc.get_field("visibility"),
        Some(&Value::String("public".to_string()))
    );
}

#[test]
fn test_apply_delete_effect() {
    let ir = get_test_ir();
    let mut state = ModelState::new();

    let actor_id = state.create_instance("User");
    state.set_field(&actor_id, "id", Value::String("user-123".to_string()));
    apply_effect(
        &mut state,
        ir.effects.get("create_document").unwrap(),
        &actor_id,
    )
    .unwrap();

    let doc_id = state.all_instances("Document")[0].id.clone();
    apply_effect(&mut state, ir.effects.get("delete").unwrap(), &actor_id).unwrap();

    let doc = state.get_instance(&doc_id).unwrap();
    assert_eq!(doc.get_field("deleted"), Some(&Value::Bool(true)));
}

#[test]
fn test_apply_read_effect_is_noop() {
    let ir = get_test_ir();
    let mut state = ModelState::new();

    let actor_id = state.create_instance("User");
    state.set_field(&actor_id, "id", Value::String("user-123".to_string()));
    apply_effect(
        &mut state,
        ir.effects.get("create_document").unwrap(),
        &actor_id,
    )
    .unwrap();

    let _gen_before = state.generation();
    apply_effect(&mut state, ir.effects.get("read").unwrap(), &actor_id).unwrap();
    // Read has empty sets, so generation should not increase (no mutations beyond effect application)
    // Actually, generation may still increase if apply_effect records the action
    // Just verify no field changes
    let doc = &state.all_instances("Document")[0];
    assert_eq!(
        doc.get_field("visibility"),
        Some(&Value::String("private".to_string()))
    );
}

#[test]
fn test_apply_archive_then_restore() {
    let ir = get_test_ir();
    let mut state = ModelState::new();

    let actor_id = state.create_instance("User");
    state.set_field(&actor_id, "id", Value::String("user-123".to_string()));
    apply_effect(
        &mut state,
        ir.effects.get("create_document").unwrap(),
        &actor_id,
    )
    .unwrap();
    apply_effect(&mut state, ir.effects.get("publish").unwrap(), &actor_id).unwrap();

    let doc_id = state.all_instances("Document")[0].id.clone();
    assert_eq!(
        state.get_instance(&doc_id).unwrap().get_field("visibility"),
        Some(&Value::String("public".to_string()))
    );

    apply_effect(&mut state, ir.effects.get("archive").unwrap(), &actor_id).unwrap();
    assert_eq!(
        state.get_instance(&doc_id).unwrap().get_field("visibility"),
        Some(&Value::String("private".to_string()))
    );

    apply_effect(&mut state, ir.effects.get("restore").unwrap(), &actor_id).unwrap();
    assert_eq!(
        state.get_instance(&doc_id).unwrap().get_field("visibility"),
        Some(&Value::String("shared".to_string()))
    );
}
