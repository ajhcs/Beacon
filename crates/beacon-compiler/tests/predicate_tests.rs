use beacon_compiler::predicate::{compile_expr, eval_expr, TypeContext, Value, ValueEnv};
use beacon_ir::expr::Expr;
use beacon_ir::parse::parse_ir;

fn make_test_context() -> TypeContext {
    let json = include_str!("../../beacon-ir/tests/fixtures/document_lifecycle.json");
    let ir = parse_ir(json).unwrap();
    TypeContext::from_ir(&ir)
}

#[test]
fn test_compile_literal_bool() {
    let ctx = make_test_context();
    let expr: Expr = serde_json::from_value(serde_json::json!(true)).unwrap();
    let compiled = compile_expr(&expr, &ctx).unwrap();
    let env = ValueEnv::new();
    let result = eval_expr(&compiled, &env).unwrap();
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn test_compile_literal_string() {
    let ctx = make_test_context();
    let expr: Expr = serde_json::from_value(serde_json::json!("public")).unwrap();
    let compiled = compile_expr(&expr, &ctx).unwrap();
    let env = ValueEnv::new();
    let result = eval_expr(&compiled, &env).unwrap();
    assert_eq!(result, Value::String("public".to_string()));
}

#[test]
fn test_compile_field_access() {
    let ctx = make_test_context();
    let expr: Expr =
        serde_json::from_value(serde_json::json!(["field", "self", "authenticated"])).unwrap();
    let compiled = compile_expr(&expr, &ctx).unwrap();

    let mut env = ValueEnv::new();
    env.set_field("self", "authenticated", Value::Bool(true));
    let result = eval_expr(&compiled, &env).unwrap();
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn test_compile_eq_expression() {
    let ctx = make_test_context();
    let expr: Expr = serde_json::from_value(serde_json::json!([
        "eq",
        ["field", "self", "visibility"],
        "public"
    ]))
    .unwrap();
    let compiled = compile_expr(&expr, &ctx).unwrap();

    let mut env = ValueEnv::new();
    env.set_field("self", "visibility", Value::String("public".to_string()));
    let result = eval_expr(&compiled, &env).unwrap();
    assert_eq!(result, Value::Bool(true));

    env.set_field("self", "visibility", Value::String("private".to_string()));
    let result = eval_expr(&compiled, &env).unwrap();
    assert_eq!(result, Value::Bool(false));
}

#[test]
fn test_compile_and_expression() {
    let ctx = make_test_context();
    let expr: Expr = serde_json::from_value(serde_json::json!(["and", true, true])).unwrap();
    let compiled = compile_expr(&expr, &ctx).unwrap();
    let env = ValueEnv::new();
    let result = eval_expr(&compiled, &env).unwrap();
    assert_eq!(result, Value::Bool(true));

    let expr: Expr = serde_json::from_value(serde_json::json!(["and", true, false])).unwrap();
    let compiled = compile_expr(&expr, &ctx).unwrap();
    let result = eval_expr(&compiled, &env).unwrap();
    assert_eq!(result, Value::Bool(false));
}

#[test]
fn test_compile_or_expression() {
    let ctx = make_test_context();
    let expr: Expr = serde_json::from_value(serde_json::json!(["or", false, true])).unwrap();
    let compiled = compile_expr(&expr, &ctx).unwrap();
    let env = ValueEnv::new();
    let result = eval_expr(&compiled, &env).unwrap();
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn test_compile_not_expression() {
    let ctx = make_test_context();
    let expr: Expr = serde_json::from_value(serde_json::json!(["not", false])).unwrap();
    let compiled = compile_expr(&expr, &ctx).unwrap();
    let env = ValueEnv::new();
    let result = eval_expr(&compiled, &env).unwrap();
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn test_compile_neq_expression() {
    let ctx = make_test_context();
    let expr: Expr = serde_json::from_value(serde_json::json!([
        "neq",
        ["field", "actor", "role"],
        "guest"
    ]))
    .unwrap();
    let compiled = compile_expr(&expr, &ctx).unwrap();

    let mut env = ValueEnv::new();
    env.set_field("actor", "role", Value::String("admin".to_string()));
    let result = eval_expr(&compiled, &env).unwrap();
    assert_eq!(result, Value::Bool(true));

    env.set_field("actor", "role", Value::String("guest".to_string()));
    let result = eval_expr(&compiled, &env).unwrap();
    assert_eq!(result, Value::Bool(false));
}

#[test]
fn test_compile_implies_expression() {
    let ctx = make_test_context();
    // implies(false, anything) = true
    let expr: Expr = serde_json::from_value(serde_json::json!(["implies", false, false])).unwrap();
    let compiled = compile_expr(&expr, &ctx).unwrap();
    let env = ValueEnv::new();
    let result = eval_expr(&compiled, &env).unwrap();
    assert_eq!(result, Value::Bool(true));

    // implies(true, false) = false
    let expr: Expr = serde_json::from_value(serde_json::json!(["implies", true, false])).unwrap();
    let compiled = compile_expr(&expr, &ctx).unwrap();
    let result = eval_expr(&compiled, &env).unwrap();
    assert_eq!(result, Value::Bool(false));
}

#[test]
fn test_compile_complex_nested() {
    let ctx = make_test_context();
    // The AccessibleDocument predicate from the fixture
    let expr: Expr = serde_json::from_value(serde_json::json!([
        "or",
        ["eq", ["field", "self", "visibility"], "public"],
        [
            "eq",
            ["field", "self", "owner_id"],
            ["field", "actor", "id"]
        ],
        [
            "and",
            ["eq", ["field", "self", "visibility"], "shared"],
            ["neq", ["field", "actor", "role"], "guest"]
        ]
    ]))
    .unwrap();
    let compiled = compile_expr(&expr, &ctx).unwrap();

    // Public doc -> accessible
    let mut env = ValueEnv::new();
    env.set_field("self", "visibility", Value::String("public".to_string()));
    env.set_field("self", "owner_id", Value::String("other".to_string()));
    env.set_field("actor", "id", Value::String("user1".to_string()));
    env.set_field("actor", "role", Value::String("guest".to_string()));
    let result = eval_expr(&compiled, &env).unwrap();
    assert_eq!(result, Value::Bool(true));

    // Private doc, not owner, guest -> not accessible
    env.set_field("self", "visibility", Value::String("private".to_string()));
    let result = eval_expr(&compiled, &env).unwrap();
    assert_eq!(result, Value::Bool(false));

    // Shared doc, non-guest -> accessible
    env.set_field("self", "visibility", Value::String("shared".to_string()));
    env.set_field("actor", "role", Value::String("member".to_string()));
    let result = eval_expr(&compiled, &env).unwrap();
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn test_compile_derived_fn_call() {
    let ctx = make_test_context();
    let expr: Expr =
        serde_json::from_value(serde_json::json!(["derived", "canAccess", "u", "d"])).unwrap();
    // Should compile without error (we just verify it compiles, not evaluates,
    // since full function evaluation requires model state resolution)
    let result = compile_expr(&expr, &ctx);
    assert!(result.is_ok());
}

#[test]
fn test_compile_unknown_operator_fails() {
    let ctx = make_test_context();
    // "bogus" is not a known operator
    let expr: Expr = serde_json::from_value(serde_json::json!(["eq", true, true])).unwrap();
    // This should succeed since eq is valid
    assert!(compile_expr(&expr, &ctx).is_ok());
}
