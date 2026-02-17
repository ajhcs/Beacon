use beacon_compiler::predicate::{compile_expr, TypeContext};
use beacon_ir::parse::parse_ir;
use beacon_model::effect::apply_effect;
use beacon_model::invariant::{check_invariants, CompiledProperty};
use beacon_model::state::{ModelState, Value};

fn setup() -> (beacon_ir::types::BeaconIR, TypeContext, Vec<CompiledProperty>) {
    let json = include_str!("../../beacon-ir/tests/fixtures/document_lifecycle.json");
    let ir = parse_ir(json).unwrap();
    let ctx = TypeContext::from_ir(&ir);

    let mut properties = Vec::new();
    for (name, prop) in &ir.properties {
        if let Some(pred) = &prop.predicate {
            let compiled = compile_expr(pred, &ctx).unwrap();
            properties.push(CompiledProperty {
                name: name.clone(),
                expr: compiled,
            });
        }
    }

    (ir, ctx, properties)
}

#[test]
fn test_invariant_passes_on_valid_state() {
    let (ir, _ctx, properties) = setup();
    let mut state = ModelState::new();

    // Create a user who owns a private document
    let user_id = state.create_instance("User");
    state.set_field(&user_id, "id", Value::String("u1".to_string()));
    state.set_field(&user_id, "role", Value::String("member".to_string()));
    state.set_field(&user_id, "authenticated", Value::Bool(true));

    apply_effect(
        &mut state,
        ir.effects.get("create_document").unwrap(),
        &user_id,
    )
    .unwrap();

    let violations = check_invariants(&state, &properties);
    assert!(violations.is_empty(), "Expected no violations, got: {:?}", violations);
}

#[test]
fn test_invariant_passes_with_multiple_users() {
    let (_ir, ctx, _properties) = setup();
    let mut state = ModelState::new();

    // Use a custom invariant that doesn't require derived function resolution:
    // forall u in User: implies(eq(u.role, "admin"), eq(u.authenticated, true))
    // "All admins are authenticated"
    let expr: beacon_ir::expr::Expr = serde_json::from_value(serde_json::json!(
        ["forall", "u", "User",
            ["implies",
                ["eq", ["field", "u", "role"], "admin"],
                ["eq", ["field", "u", "authenticated"], true]
            ]
        ]
    )).unwrap();
    let compiled = compile_expr(&expr, &ctx).unwrap();
    let custom_props = vec![CompiledProperty {
        name: "admin_is_authenticated".to_string(),
        expr: compiled,
    }];

    // User 1 is admin and authenticated (satisfies)
    let u1 = state.create_instance("User");
    state.set_field(&u1, "id", Value::String("u1".to_string()));
    state.set_field(&u1, "role", Value::String("admin".to_string()));
    state.set_field(&u1, "authenticated", Value::Bool(true));

    // User 2 is guest and not authenticated (vacuously true — not admin)
    let u2 = state.create_instance("User");
    state.set_field(&u2, "id", Value::String("u2".to_string()));
    state.set_field(&u2, "role", Value::String("guest".to_string()));
    state.set_field(&u2, "authenticated", Value::Bool(false));

    let violations = check_invariants(&state, &custom_props);
    assert!(violations.is_empty(), "Expected no violations, got: {:?}", violations);
}

#[test]
fn test_invariant_empty_model_passes() {
    let (_ir, _ctx, properties) = setup();
    let state = ModelState::new();

    // Empty model — forall over empty set is vacuously true
    let violations = check_invariants(&state, &properties);
    assert!(violations.is_empty());
}

#[test]
fn test_simple_invariant_violation() {
    let (_ir, _ctx, _properties) = setup();

    // Create a custom property that we know will fail
    let expr: beacon_ir::expr::Expr = serde_json::from_value(serde_json::json!(
        ["forall", "u", "User", ["eq", ["field", "u", "authenticated"], true]]
    ))
    .unwrap();
    let ctx_for_compile = _ctx;
    let compiled = compile_expr(&expr, &ctx_for_compile).unwrap();
    let custom_props = vec![CompiledProperty {
        name: "all_authenticated".to_string(),
        expr: compiled,
    }];

    let mut state = ModelState::new();
    let u1 = state.create_instance("User");
    state.set_field(&u1, "authenticated", Value::Bool(true));
    let u2 = state.create_instance("User");
    state.set_field(&u2, "authenticated", Value::Bool(false)); // violates

    let violations = check_invariants(&state, &custom_props);
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].property_name, "all_authenticated");
}
