use beacon_compiler::predicate::{compile_expr, TypeContext};
use beacon_ir::parse::parse_ir;
use beacon_model::effect::apply_effect;
use beacon_model::eval::eval_in_model;
use beacon_model::state::{ModelState, Value};

fn setup() -> (beacon_ir::types::BeaconIR, TypeContext) {
    let json = include_str!("../../beacon-ir/tests/fixtures/document_lifecycle.json");
    let ir = parse_ir(json).unwrap();
    let ctx = TypeContext::from_ir(&ir);
    (ir, ctx)
}

#[test]
fn test_eval_literal_in_model() {
    let (_ir, ctx) = setup();
    let expr: beacon_ir::expr::Expr = serde_json::from_value(serde_json::json!(true)).unwrap();
    let compiled = compile_expr(&expr, &ctx).unwrap();
    let state = ModelState::new();
    let result = eval_in_model(&compiled, &state, &Default::default()).unwrap();
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn test_eval_field_access_on_entity() {
    let (_ir, ctx) = setup();
    let mut state = ModelState::new();
    let user_id = state.create_instance("User");
    state.set_field(&user_id, "role", Value::String("admin".to_string()));
    state.set_field(&user_id, "authenticated", Value::Bool(true));
    state.set_field(&user_id, "id", Value::String("u1".to_string()));

    // Compile: ["field", "self", "authenticated"]
    let expr: beacon_ir::expr::Expr =
        serde_json::from_value(serde_json::json!(["field", "self", "authenticated"])).unwrap();
    let compiled = compile_expr(&expr, &ctx).unwrap();

    // Evaluate with "self" bound to the user instance
    let mut bindings = std::collections::HashMap::new();
    bindings.insert("self".to_string(), user_id.clone());
    let result = eval_in_model(&compiled, &state, &bindings).unwrap();
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn test_eval_eq_with_field_and_literal() {
    let (_ir, ctx) = setup();
    let mut state = ModelState::new();
    let doc_id = state.create_instance("Document");
    state.set_field(&doc_id, "visibility", Value::String("public".to_string()));

    let expr: beacon_ir::expr::Expr = serde_json::from_value(serde_json::json!(
        ["eq", ["field", "self", "visibility"], "public"]
    ))
    .unwrap();
    let compiled = compile_expr(&expr, &ctx).unwrap();

    let mut bindings = std::collections::HashMap::new();
    bindings.insert("self".to_string(), doc_id.clone());
    let result = eval_in_model(&compiled, &state, &bindings).unwrap();
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn test_eval_forall_quantifier_passes() {
    let (_ir, ctx) = setup();
    let mut state = ModelState::new();

    // Create two users, both authenticated
    for i in 0..2 {
        let uid = state.create_instance("User");
        state.set_field(&uid, "authenticated", Value::Bool(true));
        state.set_field(&uid, "id", Value::String(format!("u{i}")));
    }

    // forall u in User: authenticated(u) == true
    let expr: beacon_ir::expr::Expr = serde_json::from_value(serde_json::json!(
        ["forall", "u", "User", ["eq", ["field", "u", "authenticated"], true]]
    ))
    .unwrap();
    let compiled = compile_expr(&expr, &ctx).unwrap();

    let result = eval_in_model(&compiled, &state, &Default::default()).unwrap();
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn test_eval_forall_quantifier_fails() {
    let (_ir, ctx) = setup();
    let mut state = ModelState::new();

    let u1 = state.create_instance("User");
    state.set_field(&u1, "authenticated", Value::Bool(true));

    let u2 = state.create_instance("User");
    state.set_field(&u2, "authenticated", Value::Bool(false)); // This one breaks the forall

    let expr: beacon_ir::expr::Expr = serde_json::from_value(serde_json::json!(
        ["forall", "u", "User", ["eq", ["field", "u", "authenticated"], true]]
    ))
    .unwrap();
    let compiled = compile_expr(&expr, &ctx).unwrap();

    let result = eval_in_model(&compiled, &state, &Default::default()).unwrap();
    assert_eq!(result, Value::Bool(false));
}

#[test]
fn test_eval_exists_quantifier() {
    let (_ir, ctx) = setup();
    let mut state = ModelState::new();

    let u1 = state.create_instance("User");
    state.set_field(&u1, "role", Value::String("guest".to_string()));

    let u2 = state.create_instance("User");
    state.set_field(&u2, "role", Value::String("admin".to_string()));

    // exists u in User: role(u) == "admin"
    let expr: beacon_ir::expr::Expr = serde_json::from_value(serde_json::json!(
        ["exists", "u", "User", ["eq", ["field", "u", "role"], "admin"]]
    ))
    .unwrap();
    let compiled = compile_expr(&expr, &ctx).unwrap();

    let result = eval_in_model(&compiled, &state, &Default::default()).unwrap();
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn test_eval_nested_forall_ownership_isolation() {
    let (ir, ctx) = setup();
    let mut state = ModelState::new();

    // Create user
    let u1 = state.create_instance("User");
    state.set_field(&u1, "id", Value::String("u1".to_string()));
    state.set_field(&u1, "role", Value::String("member".to_string()));
    state.set_field(&u1, "authenticated", Value::Bool(true));

    // Create document owned by u1, private
    let actor_id = u1.clone();
    apply_effect(
        &mut state,
        ir.effects.get("create_document").unwrap(),
        &actor_id,
    )
    .unwrap();

    // The ownership_isolation property from the fixture:
    // forall d in Document, forall u in User:
    //   (d.visibility == "private" AND d.owner_id != u.id) implies NOT canAccess(u, d)
    //
    // With a single user who IS the owner, the antecedent (owner_id != u.id) is false,
    // so implies is vacuously true. This should pass.
    let prop = ir.properties.get("ownership_isolation").unwrap();
    if let Some(pred) = &prop.predicate {
        let compiled = compile_expr(pred, &ctx).unwrap();
        let result = eval_in_model(&compiled, &state, &Default::default()).unwrap();
        assert_eq!(result, Value::Bool(true));
    }
}
