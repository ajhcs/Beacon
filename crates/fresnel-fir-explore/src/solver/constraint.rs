//! Constraint encoding: translates IR constraint predicates into SAT clauses.
//!
//! The IR constraint language supports:
//! - `eq(domain_var, value)` — domain variable equals a specific value
//! - `neq(domain_var, value)` — domain variable does not equal a specific value
//! - `implies(A, B)` — if A then B
//! - `and(A, B, ...)` — conjunction
//! - `or(A, B, ...)` — disjunction
//! - `not(A)` — negation
//!
//! These are compiled into CNF clauses suitable for the SAT solver.

use fresnel_fir_ir::expr::{Expr, Literal, OpKind};
use varisat::Lit;

use super::domain::{lit_for_not_value, lit_for_value, EncodedInputSpace, Encoding};
use super::DomainValue;

/// Errors during constraint encoding.
#[derive(Debug, thiserror::Error)]
pub enum ConstraintError {
    #[error("unknown domain variable '{0}' in constraint")]
    UnknownDomain(String),

    #[error("cannot encode expression as SAT constraint: {0}")]
    UnsupportedExpr(String),

    #[error("domain '{domain}' has no value '{value}'")]
    InvalidValue { domain: String, value: String },
}

/// Result of encoding a constraint expression: a set of CNF clauses.
/// Each inner Vec<Lit> is a disjunctive clause; the set is conjunctive (AND of ORs).
pub type CnfClauses = Vec<Vec<Lit>>;

/// Encode all IR constraints into SAT clauses.
pub fn encode_constraints(
    constraints: &[fresnel_fir_ir::types::InputConstraint],
    encoded_space: &EncodedInputSpace,
) -> Result<CnfClauses, ConstraintError> {
    let mut all_clauses = Vec::new();
    for constraint in constraints {
        let clauses = encode_expr(&constraint.rule, encoded_space)?;
        all_clauses.extend(clauses);
    }
    Ok(all_clauses)
}

/// Encode a single expression into CNF clauses.
///
/// The encoding uses Tseitin-like transformation where possible:
/// - Atomic propositions (eq, neq) become unit or short clauses.
/// - `and(A, B)` concatenates the clauses of A and B.
/// - `implies(A, B)` becomes `or(not(A), B)`.
/// - `or(A, B)` and `not(A)` require auxiliary handling.
fn encode_expr(expr: &Expr, space: &EncodedInputSpace) -> Result<CnfClauses, ConstraintError> {
    match expr {
        // eq(domain_var_name, literal_value)
        // Encoded as: the SAT literal for that value must be true.
        Expr::Op {
            op: OpKind::Eq,
            args,
        } if args.len() == 2 => encode_eq(&args[0], &args[1], space, false),

        // neq(domain_var_name, literal_value)
        // Encoded as: the SAT literal for that value must be false.
        Expr::Op {
            op: OpKind::Neq,
            args,
        } if args.len() == 2 => encode_eq(&args[0], &args[1], space, true),

        // implies(A, B) => for each conjunction clause of A, create (not_A_clause OR B)
        // Simplified: implies(A, B) where A is atomic => not(A) OR B
        Expr::Op {
            op: OpKind::Implies,
            args,
        } if args.len() == 2 => encode_implies(&args[0], &args[1], space),

        // and(A, B, ...) => concatenate clauses of each operand.
        Expr::Op {
            op: OpKind::And,
            args,
        } => {
            let mut all = Vec::new();
            for arg in args {
                all.extend(encode_expr(arg, space)?);
            }
            Ok(all)
        }

        // or(A, B, ...) => requires combining clause sets disjunctively.
        Expr::Op {
            op: OpKind::Or,
            args,
        } => encode_or(args, space),

        // not(A) => negate. Only works for atomic propositions.
        Expr::Op {
            op: OpKind::Not,
            args,
        } if args.len() == 1 => encode_not(&args[0], space),

        // Literal true is trivially satisfied (no clauses needed).
        Expr::Literal(Literal::Bool(true)) => Ok(vec![]),

        // Literal false is unsatisfiable (empty clause).
        Expr::Literal(Literal::Bool(false)) => Ok(vec![vec![]]),

        other => Err(ConstraintError::UnsupportedExpr(format!("{:?}", other))),
    }
}

/// Encode `eq(a, b)` or `neq(a, b)`.
/// One of the args should be a domain variable reference (as a string literal matching a domain name),
/// and the other should be a literal value.
fn encode_eq(
    lhs: &Expr,
    rhs: &Expr,
    space: &EncodedInputSpace,
    negate: bool,
) -> Result<CnfClauses, ConstraintError> {
    // Try both orderings: (domain_name, value) or (value, domain_name).
    if let Some((domain_name, value)) = extract_domain_value_pair(lhs, rhs, space) {
        let enc = space
            .domains
            .get(&domain_name)
            .ok_or_else(|| ConstraintError::UnknownDomain(domain_name.clone()))?;

        let domain_val = literal_to_domain_value(&value, &enc.encoding)?;

        let lit = if negate {
            lit_for_not_value(enc, &domain_val)
        } else {
            lit_for_value(enc, &domain_val)
        };

        match lit {
            Some(l) => Ok(vec![vec![l]]),
            None => Err(ConstraintError::InvalidValue {
                domain: domain_name,
                value: format!("{:?}", value),
            }),
        }
    } else {
        // Both sides might be domain references — compare equality between two domains.
        // For now, we only support domain vs literal comparisons.
        Err(ConstraintError::UnsupportedExpr(
            "eq/neq between two non-literal expressions is not yet supported".to_string(),
        ))
    }
}

/// Try to extract a (domain_name, literal_value) pair from two expressions.
fn extract_domain_value_pair(
    lhs: &Expr,
    rhs: &Expr,
    space: &EncodedInputSpace,
) -> Option<(String, Literal)> {
    // Case 1: lhs is a domain name string, rhs is a literal.
    if let (Expr::Literal(Literal::String(name)), lit) = (lhs, rhs) {
        if space.domains.contains_key(name) {
            if let Some(l) = expr_to_literal(lit) {
                return Some((name.clone(), l));
            }
        }
    }
    // Case 2: rhs is a domain name string, lhs is a literal.
    if let (lit, Expr::Literal(Literal::String(name))) = (lhs, rhs) {
        if space.domains.contains_key(name) {
            if let Some(l) = expr_to_literal(lit) {
                return Some((name.clone(), l));
            }
        }
    }
    None
}

/// Convert an Expr to a Literal if possible.
fn expr_to_literal(expr: &Expr) -> Option<Literal> {
    match expr {
        Expr::Literal(l) => Some(l.clone()),
        _ => None,
    }
}

/// Convert an IR Literal to a DomainValue appropriate for the encoding.
fn literal_to_domain_value(
    lit: &Literal,
    encoding: &Encoding,
) -> Result<DomainValue, ConstraintError> {
    match (lit, encoding) {
        (Literal::Bool(b), Encoding::Bool { .. }) => Ok(DomainValue::Bool(*b)),
        (Literal::String(s), Encoding::OneHot { .. }) => Ok(DomainValue::Enum(s.clone())),
        (Literal::Int(i), Encoding::OneHot { .. }) => Ok(DomainValue::Int(*i)),
        (Literal::Bool(b), Encoding::OneHot { .. }) => Ok(DomainValue::Bool(*b)),
        _ => Err(ConstraintError::UnsupportedExpr(format!(
            "cannot convert literal {:?} for encoding {:?}",
            lit, encoding
        ))),
    }
}

/// Encode `implies(A, B)`.
///
/// For atomic A (produces a single unit clause [lit_a]):
///   implies(A, B) = not(A) OR B
///   For each clause `c` in B, we add `(!lit_a) ++ c`.
///
/// For conjunctive A (produces multiple unit clauses):
///   implies(A1 AND A2 AND ..., B) = not(A1) OR not(A2) OR ... OR B
///   For each clause `c` in B, we add `(!a1 OR !a2 OR ... OR c)`.
fn encode_implies(
    antecedent: &Expr,
    consequent: &Expr,
    space: &EncodedInputSpace,
) -> Result<CnfClauses, ConstraintError> {
    let ante_clauses = encode_expr(antecedent, space)?;
    let cons_clauses = encode_expr(consequent, space)?;

    // Collect all antecedent unit literals (negated for implication).
    let mut ante_negated_lits: Vec<Lit> = Vec::new();
    for clause in &ante_clauses {
        if clause.len() == 1 {
            ante_negated_lits.push(!clause[0]);
        } else {
            // Non-unit antecedent clause: more complex encoding needed.
            // For now, we handle the common case where antecedent is a conjunction
            // of atomic propositions (each producing a unit clause).
            return Err(ConstraintError::UnsupportedExpr(
                "implies with non-atomic antecedent clause is not yet supported".to_string(),
            ));
        }
    }

    if cons_clauses.is_empty() {
        // Consequent is trivially true: implication is trivially true.
        return Ok(vec![]);
    }

    // For each consequent clause, prepend the negated antecedent literals.
    let mut result = Vec::new();
    for cons_clause in &cons_clauses {
        let mut new_clause = ante_negated_lits.clone();
        new_clause.extend_from_slice(cons_clause);
        result.push(new_clause);
    }

    Ok(result)
}

/// Encode `or(A, B, ...)`.
///
/// When each sub-expression produces only unit clauses, we can combine
/// them into a single disjunctive clause. For more complex cases,
/// we use auxiliary variables (Tseitin transformation).
fn encode_or(args: &[Expr], space: &EncodedInputSpace) -> Result<CnfClauses, ConstraintError> {
    // Collect the encoding of each argument.
    let mut arg_clauses: Vec<CnfClauses> = Vec::new();
    for arg in args {
        arg_clauses.push(encode_expr(arg, space)?);
    }

    // Simple case: each argument produces exactly one unit clause.
    let all_unit = arg_clauses
        .iter()
        .all(|cs| cs.len() == 1 && cs[0].len() == 1);
    if all_unit {
        let combined: Vec<Lit> = arg_clauses.iter().map(|cs| cs[0][0]).collect();
        return Ok(vec![combined]);
    }

    // General case: at least one argument has multiple clauses.
    // For `or(A, B)` where A = {c1 AND c2} and B = {d1 AND d2}:
    // We need: for every combination (one clause from A, one clause from B),
    // create a clause that is their union.
    // This can be exponential, but for small constraint sets it's fine.
    let mut result: Vec<Vec<Lit>> = vec![vec![]];
    for arg_cs in &arg_clauses {
        if arg_cs.is_empty() {
            // This disjunct is trivially true => entire OR is true.
            return Ok(vec![]);
        }
        let mut new_result = Vec::new();
        for existing in &result {
            for clause in arg_cs {
                let mut combined = existing.clone();
                combined.extend_from_slice(clause);
                new_result.push(combined);
            }
        }
        result = new_result;
    }

    Ok(result)
}

/// Encode `not(A)`.
///
/// For atomic A (unit clause [lit]): not(A) = [!lit].
/// For conjunctions: not(A AND B) = or(not(A), not(B)) — De Morgan.
fn encode_not(expr: &Expr, space: &EncodedInputSpace) -> Result<CnfClauses, ConstraintError> {
    let clauses = encode_expr(expr, space)?;

    if clauses.is_empty() {
        // not(true) = false => empty clause (unsatisfiable).
        return Ok(vec![vec![]]);
    }

    // For unit clauses, negate each literal.
    // For a conjunction of unit clauses (AND of lits): not(l1 AND l2) = (!l1 OR !l2).
    let all_unit = clauses.iter().all(|c| c.len() == 1);
    if all_unit {
        // De Morgan: not(l1 AND l2 AND ...) = (!l1 OR !l2 OR ...)
        let negated: Vec<Lit> = clauses.iter().map(|c| !c[0]).collect();
        return Ok(vec![negated]);
    }

    Err(ConstraintError::UnsupportedExpr(
        "not() over complex (non-unit) clauses is not yet supported".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fresnel_fir_ir::types::*;
    use std::collections::HashMap;
    use varisat::{solver::Solver, ExtendFormula};

    use crate::solver::domain::{decode_model, encode_input_space};

    fn make_input_space_with_constraints(
        domains: HashMap<String, Domain>,
        constraints: Vec<InputConstraint>,
    ) -> InputSpace {
        InputSpace {
            domains,
            constraints,
            coverage: CoverageConfig {
                targets: vec![],
                seed: 42,
                reproducible: true,
            },
        }
    }

    fn make_solver_with_space(
        input_space: &InputSpace,
    ) -> (Solver<'_>, super::super::domain::EncodedInputSpace) {
        let encoded = encode_input_space(input_space).unwrap();
        let mut solver = Solver::new();
        for clause in &encoded.structural_clauses {
            solver.add_clause(clause);
        }
        let constraint_clauses = encode_constraints(&input_space.constraints, &encoded).unwrap();
        for clause in &constraint_clauses {
            solver.add_clause(clause);
        }
        (solver, encoded)
    }

    #[test]
    fn test_eq_constraint_forces_value() {
        // Constraint: role == "admin"
        let mut domains = HashMap::new();
        domains.insert(
            "role".to_string(),
            Domain {
                domain_type: DomainType::Enum {
                    values: vec!["admin".into(), "member".into(), "guest".into()],
                },
            },
        );
        let constraints = vec![InputConstraint {
            name: "force_admin".to_string(),
            rule: Expr::Op {
                op: OpKind::Eq,
                args: vec![
                    Expr::Literal(Literal::String("role".into())),
                    Expr::Literal(Literal::String("admin".into())),
                ],
            },
        }];

        let input_space = make_input_space_with_constraints(domains, constraints);
        let (mut solver, encoded) = make_solver_with_space(&input_space);

        assert!(solver.solve().unwrap());
        let model = solver.model().unwrap();
        let decoded = decode_model(&encoded, &model);
        assert_eq!(decoded["role"], DomainValue::Enum("admin".into()));
    }

    #[test]
    fn test_neq_constraint_excludes_value() {
        // Constraint: role != "guest"
        let mut domains = HashMap::new();
        domains.insert(
            "role".to_string(),
            Domain {
                domain_type: DomainType::Enum {
                    values: vec!["admin".into(), "member".into(), "guest".into()],
                },
            },
        );
        let constraints = vec![InputConstraint {
            name: "no_guest".to_string(),
            rule: Expr::Op {
                op: OpKind::Neq,
                args: vec![
                    Expr::Literal(Literal::String("role".into())),
                    Expr::Literal(Literal::String("guest".into())),
                ],
            },
        }];

        let input_space = make_input_space_with_constraints(domains, constraints);
        let (mut solver, encoded) = make_solver_with_space(&input_space);

        // Enumerate all solutions and verify guest never appears.
        let mut count = 0;
        while solver.solve().unwrap() {
            let model = solver.model().unwrap();
            let decoded = decode_model(&encoded, &model);
            assert_ne!(decoded["role"], DomainValue::Enum("guest".into()));
            count += 1;

            // Add blocking clause to find next solution.
            let blocking: Vec<Lit> = model.iter().map(|l| !*l).collect();
            solver.add_clause(&blocking);
        }
        // Should have exactly 2 solutions: admin and member.
        assert_eq!(count, 2);
    }

    #[test]
    fn test_implies_constraint() {
        // implies(eq(role, "guest"), eq(authenticated, false))
        // "If guest, then not authenticated"
        let mut domains = HashMap::new();
        domains.insert(
            "role".to_string(),
            Domain {
                domain_type: DomainType::Enum {
                    values: vec!["admin".into(), "guest".into()],
                },
            },
        );
        domains.insert(
            "authenticated".to_string(),
            Domain {
                domain_type: DomainType::Bool,
            },
        );

        let constraints = vec![InputConstraint {
            name: "guest_not_auth".to_string(),
            rule: Expr::Op {
                op: OpKind::Implies,
                args: vec![
                    Expr::Op {
                        op: OpKind::Eq,
                        args: vec![
                            Expr::Literal(Literal::String("role".into())),
                            Expr::Literal(Literal::String("guest".into())),
                        ],
                    },
                    Expr::Op {
                        op: OpKind::Eq,
                        args: vec![
                            Expr::Literal(Literal::String("authenticated".into())),
                            Expr::Literal(Literal::Bool(false)),
                        ],
                    },
                ],
            },
        }];

        let input_space = make_input_space_with_constraints(domains, constraints);
        let (mut solver, encoded) = make_solver_with_space(&input_space);

        // Enumerate all solutions and verify the implication holds.
        let mut solutions = Vec::new();
        while solver.solve().unwrap() {
            let model = solver.model().unwrap();
            let decoded = decode_model(&encoded, &model);
            solutions.push(decoded.clone());

            // Verify: if role=guest then authenticated=false.
            if decoded["role"] == DomainValue::Enum("guest".into()) {
                assert_eq!(decoded["authenticated"], DomainValue::Bool(false));
            }

            let blocking: Vec<Lit> = model.iter().map(|l| !*l).collect();
            solver.add_clause(&blocking);
        }

        // Should have valid solutions (admin+true, admin+false, guest+false = 3).
        assert_eq!(solutions.len(), 3);
    }

    #[test]
    fn test_and_constraint() {
        // and(eq(role, "admin"), eq(auth, true))
        let mut domains = HashMap::new();
        domains.insert(
            "role".to_string(),
            Domain {
                domain_type: DomainType::Enum {
                    values: vec!["admin".into(), "guest".into()],
                },
            },
        );
        domains.insert(
            "auth".to_string(),
            Domain {
                domain_type: DomainType::Bool,
            },
        );

        let constraints = vec![InputConstraint {
            name: "admin_and_auth".to_string(),
            rule: Expr::Op {
                op: OpKind::And,
                args: vec![
                    Expr::Op {
                        op: OpKind::Eq,
                        args: vec![
                            Expr::Literal(Literal::String("role".into())),
                            Expr::Literal(Literal::String("admin".into())),
                        ],
                    },
                    Expr::Op {
                        op: OpKind::Eq,
                        args: vec![
                            Expr::Literal(Literal::String("auth".into())),
                            Expr::Literal(Literal::Bool(true)),
                        ],
                    },
                ],
            },
        }];

        let input_space = make_input_space_with_constraints(domains, constraints);
        let (mut solver, encoded) = make_solver_with_space(&input_space);

        assert!(solver.solve().unwrap());
        let model = solver.model().unwrap();
        let decoded = decode_model(&encoded, &model);
        assert_eq!(decoded["role"], DomainValue::Enum("admin".into()));
        assert_eq!(decoded["auth"], DomainValue::Bool(true));

        // Should be the only solution.
        let blocking: Vec<Lit> = model.iter().map(|l| !*l).collect();
        solver.add_clause(&blocking);
        assert!(!solver.solve().unwrap());
    }

    #[test]
    fn test_or_constraint() {
        // or(eq(role, "admin"), eq(role, "member"))
        // Excludes "guest".
        let mut domains = HashMap::new();
        domains.insert(
            "role".to_string(),
            Domain {
                domain_type: DomainType::Enum {
                    values: vec!["admin".into(), "member".into(), "guest".into()],
                },
            },
        );

        let constraints = vec![InputConstraint {
            name: "admin_or_member".to_string(),
            rule: Expr::Op {
                op: OpKind::Or,
                args: vec![
                    Expr::Op {
                        op: OpKind::Eq,
                        args: vec![
                            Expr::Literal(Literal::String("role".into())),
                            Expr::Literal(Literal::String("admin".into())),
                        ],
                    },
                    Expr::Op {
                        op: OpKind::Eq,
                        args: vec![
                            Expr::Literal(Literal::String("role".into())),
                            Expr::Literal(Literal::String("member".into())),
                        ],
                    },
                ],
            },
        }];

        let input_space = make_input_space_with_constraints(domains, constraints);
        let (mut solver, encoded) = make_solver_with_space(&input_space);

        let mut solutions = Vec::new();
        while solver.solve().unwrap() {
            let model = solver.model().unwrap();
            let decoded = decode_model(&encoded, &model);
            assert_ne!(decoded["role"], DomainValue::Enum("guest".into()));
            solutions.push(decoded);

            let blocking: Vec<Lit> = model.iter().map(|l| !*l).collect();
            solver.add_clause(&blocking);
        }
        assert_eq!(solutions.len(), 2);
    }

    #[test]
    fn test_guest_never_admin_from_design_doc() {
        // From the design doc: implies(eq("actor_role", "guest"), neq("actor_role", "admin"))
        // This is trivially true since role can only be one value at a time,
        // but it tests the full encoding pipeline.
        let mut domains = HashMap::new();
        domains.insert(
            "actor_role".to_string(),
            Domain {
                domain_type: DomainType::Enum {
                    values: vec!["admin".into(), "member".into(), "guest".into()],
                },
            },
        );
        domains.insert(
            "actor_authenticated".to_string(),
            Domain {
                domain_type: DomainType::Bool,
            },
        );
        domains.insert(
            "doc_visibility".to_string(),
            Domain {
                domain_type: DomainType::Enum {
                    values: vec!["private".into(), "shared".into(), "public".into()],
                },
            },
        );
        domains.insert(
            "actor_is_owner".to_string(),
            Domain {
                domain_type: DomainType::Bool,
            },
        );
        domains.insert(
            "concurrent_actors".to_string(),
            Domain {
                domain_type: DomainType::Int { min: 1, max: 8 },
            },
        );

        let constraints = vec![InputConstraint {
            name: "guest_never_admin".to_string(),
            rule: Expr::Op {
                op: OpKind::Implies,
                args: vec![
                    Expr::Op {
                        op: OpKind::Eq,
                        args: vec![
                            Expr::Literal(Literal::String("actor_role".into())),
                            Expr::Literal(Literal::String("guest".into())),
                        ],
                    },
                    Expr::Op {
                        op: OpKind::Neq,
                        args: vec![
                            Expr::Literal(Literal::String("actor_role".into())),
                            Expr::Literal(Literal::String("admin".into())),
                        ],
                    },
                ],
            },
        }];

        let input_space = make_input_space_with_constraints(domains, constraints);
        let (mut solver, encoded) = make_solver_with_space(&input_space);

        assert!(solver.solve().unwrap());
        let model = solver.model().unwrap();
        let decoded = decode_model(&encoded, &model);

        // Every domain should have a value.
        assert_eq!(decoded.len(), 5);
    }

    #[test]
    fn test_unsatisfiable_constraint() {
        // Contradictory: role == "admin" AND role == "guest"
        let mut domains = HashMap::new();
        domains.insert(
            "role".to_string(),
            Domain {
                domain_type: DomainType::Enum {
                    values: vec!["admin".into(), "guest".into()],
                },
            },
        );

        let constraints = vec![
            InputConstraint {
                name: "must_admin".to_string(),
                rule: Expr::Op {
                    op: OpKind::Eq,
                    args: vec![
                        Expr::Literal(Literal::String("role".into())),
                        Expr::Literal(Literal::String("admin".into())),
                    ],
                },
            },
            InputConstraint {
                name: "must_guest".to_string(),
                rule: Expr::Op {
                    op: OpKind::Eq,
                    args: vec![
                        Expr::Literal(Literal::String("role".into())),
                        Expr::Literal(Literal::String("guest".into())),
                    ],
                },
            },
        ];

        let input_space = make_input_space_with_constraints(domains, constraints);
        let (mut solver, _encoded) = make_solver_with_space(&input_space);

        // Should be UNSAT.
        assert!(!solver.solve().unwrap());
    }
}
