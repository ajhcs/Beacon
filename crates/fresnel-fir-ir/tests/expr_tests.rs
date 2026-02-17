use fresnel_fir_ir::expr::Expr;

#[test]
fn test_parse_literal_bool() {
    let json = serde_json::json!(true);
    let expr: Expr = serde_json::from_value(json).unwrap();
    assert!(matches!(
        expr,
        Expr::Literal(fresnel_fir_ir::expr::Literal::Bool(true))
    ));
}

#[test]
fn test_parse_literal_string() {
    let json = serde_json::json!("public");
    let expr: Expr = serde_json::from_value(json).unwrap();
    assert!(matches!(expr, Expr::Literal(fresnel_fir_ir::expr::Literal::String(s)) if s == "public"));
}

#[test]
fn test_parse_eq_expression() {
    let json = serde_json::json!(["eq", ["field", "self", "authenticated"], true]);
    let expr: Expr = serde_json::from_value(json).unwrap();
    assert!(matches!(expr, Expr::Op { .. }));
}

#[test]
fn test_parse_nested_and_or() {
    let json = serde_json::json!([
        "or",
        ["eq", ["field", "self", "visibility"], "public"],
        [
            "and",
            ["eq", ["field", "self", "visibility"], "shared"],
            ["neq", ["field", "actor", "role"], "guest"]
        ]
    ]);
    let expr: Expr = serde_json::from_value(json).unwrap();
    assert!(matches!(expr, Expr::Op { .. }));
}

#[test]
fn test_parse_field_access() {
    let json = serde_json::json!(["field", "self", "owner_id"]);
    let expr: Expr = serde_json::from_value(json).unwrap();
    assert!(matches!(expr, Expr::Field { .. }));
}

#[test]
fn test_parse_forall_quantifier() {
    let json = serde_json::json!([
        "forall",
        "d",
        "Document",
        ["not", ["derived", "canAccess", "u", "d"]]
    ]);
    let expr: Expr = serde_json::from_value(json).unwrap();
    assert!(matches!(expr, Expr::Quantifier { .. }));
}
