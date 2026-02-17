//! SAT-based search for satisfying assignments.
//!
//! Given encoded domains + constraints, finds satisfying test vectors.
//! Supports finding a single solution, multiple unique solutions,
//! and bounded search with a maximum count.

use std::collections::HashSet;

use varisat::{ExtendFormula, Lit, Var, solver::Solver};

use super::domain::{EncodedInputSpace, Encoding, decode_model};
use super::constraint::{CnfClauses, encode_constraints};
use super::TestVector;
use beacon_ir::types::InputSpace;

/// Errors during search.
#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error("domain encoding error: {0}")]
    Encoding(#[from] super::domain::EncodingError),

    #[error("constraint encoding error: {0}")]
    Constraint(#[from] super::constraint::ConstraintError),

    #[error("solver error: {0}")]
    Solver(String),
}

/// Result of a satisfiability check.
#[derive(Debug, Clone)]
pub enum SatResult {
    /// Satisfiable — at least one solution exists.
    Sat(TestVector),
    /// Unsatisfiable — no solution exists.
    Unsat,
}

/// Configuration for searching multiple vectors.
#[derive(Debug, Clone)]
pub struct SearchConfig {
    /// Maximum number of unique vectors to find (0 = find all).
    pub max_vectors: usize,
    /// Additional clauses to add beyond structural + constraint clauses.
    /// Used by fracture to fix variables.
    pub extra_clauses: CnfClauses,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            max_vectors: 0,
            extra_clauses: vec![],
        }
    }
}

/// Collect all SAT variables used in the encoding.
fn all_domain_vars(encoded: &EncodedInputSpace) -> Vec<Var> {
    let mut vars = Vec::new();
    for enc in encoded.domains.values() {
        match &enc.encoding {
            Encoding::Bool { var } => vars.push(*var),
            Encoding::OneHot { variants } => {
                for (_, var) in variants {
                    vars.push(*var);
                }
            }
        }
    }
    vars
}

/// Build a domain-specific blocking clause from a model.
///
/// Only includes literals for variables that belong to our encoded domains.
/// This avoids issues with solver-internal variables and ensures proper blocking.
fn domain_blocking_clause(encoded: &EncodedInputSpace, model: &[Lit]) -> Vec<Lit> {
    let domain_var_set: HashSet<usize> = all_domain_vars(encoded)
        .iter()
        .map(|v| v.index())
        .collect();

    model
        .iter()
        .filter(|l| domain_var_set.contains(&l.var().index()))
        .map(|l| !*l)
        .collect()
}

/// Initialize a solver with all domain variables registered and all clauses added.
fn init_solver<'a>(
    encoded: &EncodedInputSpace,
    constraint_clauses: &CnfClauses,
    extra_clauses: &CnfClauses,
) -> Solver<'a> {
    let mut solver = Solver::new();

    // Ensure all domain variables are known to the solver by adding
    // tautological clauses [v, !v] for each variable. This guarantees
    // the solver tracks all variables even if no real clause mentions them.
    for var in all_domain_vars(encoded) {
        solver.add_clause(&[var.positive(), var.negative()]);
    }

    // Add structural clauses (exactly-one for enums/ints).
    for clause in &encoded.structural_clauses {
        solver.add_clause(clause);
    }

    // Add constraint clauses.
    for clause in constraint_clauses {
        solver.add_clause(clause);
    }

    // Add extra clauses (e.g., variable fixings from fracture).
    for clause in extra_clauses {
        solver.add_clause(clause);
    }

    solver
}

/// Find a single satisfying assignment for the given input space.
pub fn find_one(
    encoded: &EncodedInputSpace,
    constraint_clauses: &CnfClauses,
    extra_clauses: &CnfClauses,
) -> Result<SatResult, SearchError> {
    let mut solver = init_solver(encoded, constraint_clauses, extra_clauses);

    match solver.solve() {
        Ok(true) => {
            let model = solver.model().ok_or_else(|| {
                SearchError::Solver("SAT but no model returned".to_string())
            })?;
            let assignments = decode_model(encoded, &model);
            Ok(SatResult::Sat(TestVector { assignments }))
        }
        Ok(false) => Ok(SatResult::Unsat),
        Err(e) => Err(SearchError::Solver(e.to_string())),
    }
}

/// Find multiple unique satisfying assignments.
///
/// Uses blocking clauses to ensure each found vector is unique.
/// Stops when either:
/// - `max_vectors` unique vectors have been found (0 = find all)
/// - The solver reports UNSAT (all solutions exhausted)
pub fn find_many(
    encoded: &EncodedInputSpace,
    constraint_clauses: &CnfClauses,
    extra_clauses: &CnfClauses,
    max_vectors: usize,
) -> Result<Vec<TestVector>, SearchError> {
    let mut solver = init_solver(encoded, constraint_clauses, extra_clauses);

    let mut vectors = Vec::new();
    let mut seen = HashSet::new();

    loop {
        if max_vectors > 0 && vectors.len() >= max_vectors {
            break;
        }

        match solver.solve() {
            Ok(true) => {
                let model = solver.model().ok_or_else(|| {
                    SearchError::Solver("SAT but no model returned".to_string())
                })?;
                let assignments = decode_model(encoded, &model);
                let vector = TestVector { assignments };

                // Check uniqueness via hash.
                if seen.insert(vector.clone()) {
                    vectors.push(vector);
                }

                // Add blocking clause — only for domain-relevant variables.
                let blocking = domain_blocking_clause(encoded, &model);
                if blocking.is_empty() {
                    break; // No variables to block — degenerate case.
                }
                solver.add_clause(&blocking);
            }
            Ok(false) => break, // UNSAT — no more solutions.
            Err(e) => return Err(SearchError::Solver(e.to_string())),
        }
    }

    Ok(vectors)
}

/// Check if the given encoded space (with constraints + extras) is satisfiable.
pub fn is_sat(
    encoded: &EncodedInputSpace,
    constraint_clauses: &CnfClauses,
    extra_clauses: &CnfClauses,
) -> Result<bool, SearchError> {
    match find_one(encoded, constraint_clauses, extra_clauses)? {
        SatResult::Sat(_) => Ok(true),
        SatResult::Unsat => Ok(false),
    }
}

/// Convenience: encode + find all unique vectors from an InputSpace.
pub fn solve_input_space(
    input_space: &InputSpace,
    max_vectors: usize,
) -> Result<Vec<TestVector>, SearchError> {
    let encoded = super::domain::encode_input_space(input_space)?;
    let constraint_clauses = encode_constraints(&input_space.constraints, &encoded)?;
    find_many(&encoded, &constraint_clauses, &vec![], max_vectors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use beacon_ir::expr::{Expr, Literal, OpKind};
    use beacon_ir::types::*;
    use std::collections::HashMap;

    use crate::solver::DomainValue;

    fn make_input_space(
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

    #[test]
    fn test_find_one_simple() {
        let mut domains = HashMap::new();
        domains.insert(
            "flag".to_string(),
            Domain { domain_type: DomainType::Bool },
        );
        let input_space = make_input_space(domains, vec![]);
        let vectors = solve_input_space(&input_space, 1).unwrap();
        assert_eq!(vectors.len(), 1);
        assert!(matches!(vectors[0].assignments["flag"], DomainValue::Bool(_)));
    }

    #[test]
    fn test_find_all_bool() {
        let mut domains = HashMap::new();
        domains.insert(
            "flag".to_string(),
            Domain { domain_type: DomainType::Bool },
        );
        let input_space = make_input_space(domains, vec![]);
        let vectors = solve_input_space(&input_space, 0).unwrap();
        assert_eq!(vectors.len(), 2); // true and false
    }

    #[test]
    fn test_find_all_enum() {
        let mut domains = HashMap::new();
        domains.insert(
            "role".to_string(),
            Domain {
                domain_type: DomainType::Enum {
                    values: vec!["admin".into(), "member".into(), "guest".into()],
                },
            },
        );
        let input_space = make_input_space(domains, vec![]);
        let vectors = solve_input_space(&input_space, 0).unwrap();
        assert_eq!(vectors.len(), 3);

        let values: HashSet<&DomainValue> = vectors.iter().map(|v| &v.assignments["role"]).collect();
        assert!(values.contains(&DomainValue::Enum("admin".into())));
        assert!(values.contains(&DomainValue::Enum("member".into())));
        assert!(values.contains(&DomainValue::Enum("guest".into())));
    }

    #[test]
    fn test_find_all_with_constraint() {
        // 2 roles x 2 bools = 4, minus guest+true = 3.
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
            Domain { domain_type: DomainType::Bool },
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
                            Expr::Literal(Literal::String("auth".into())),
                            Expr::Literal(Literal::Bool(false)),
                        ],
                    },
                ],
            },
        }];

        let input_space = make_input_space(domains, constraints);
        let vectors = solve_input_space(&input_space, 0).unwrap();
        assert_eq!(vectors.len(), 3); // admin+true, admin+false, guest+false

        for v in &vectors {
            if v.assignments["role"] == DomainValue::Enum("guest".into()) {
                assert_eq!(v.assignments["auth"], DomainValue::Bool(false));
            }
        }
    }

    #[test]
    fn test_find_many_with_limit() {
        let mut domains = HashMap::new();
        domains.insert(
            "role".to_string(),
            Domain {
                domain_type: DomainType::Enum {
                    values: vec!["admin".into(), "member".into(), "guest".into()],
                },
            },
        );
        domains.insert(
            "flag".to_string(),
            Domain { domain_type: DomainType::Bool },
        );
        // Total: 3 x 2 = 6 possible vectors.
        let input_space = make_input_space(domains, vec![]);
        let vectors = solve_input_space(&input_space, 3).unwrap();
        assert_eq!(vectors.len(), 3);
    }

    #[test]
    fn test_uniqueness_guaranteed() {
        let mut domains = HashMap::new();
        domains.insert(
            "role".to_string(),
            Domain {
                domain_type: DomainType::Enum {
                    values: vec!["admin".into(), "member".into(), "guest".into()],
                },
            },
        );
        domains.insert(
            "auth".to_string(),
            Domain { domain_type: DomainType::Bool },
        );

        let input_space = make_input_space(domains, vec![]);
        let vectors = solve_input_space(&input_space, 0).unwrap();

        // Verify all vectors are unique.
        let set: HashSet<&TestVector> = vectors.iter().collect();
        assert_eq!(set.len(), vectors.len());
        assert_eq!(vectors.len(), 6); // 3 x 2
    }

    #[test]
    fn test_unsat_returns_empty() {
        // Contradictory constraints: role must be both admin and guest.
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

        let input_space = make_input_space(domains, constraints);
        let vectors = solve_input_space(&input_space, 0).unwrap();
        assert!(vectors.is_empty());
    }

    #[test]
    fn test_is_sat() {
        let mut domains = HashMap::new();
        domains.insert(
            "flag".to_string(),
            Domain { domain_type: DomainType::Bool },
        );
        let input_space = make_input_space(domains, vec![]);
        let encoded = super::super::domain::encode_input_space(&input_space).unwrap();
        let constraint_clauses = encode_constraints(&input_space.constraints, &encoded).unwrap();

        assert!(is_sat(&encoded, &constraint_clauses, &vec![]).unwrap());
    }

    #[test]
    fn test_find_one_with_extra_clauses() {
        let mut domains = HashMap::new();
        domains.insert(
            "role".to_string(),
            Domain {
                domain_type: DomainType::Enum {
                    values: vec!["admin".into(), "member".into(), "guest".into()],
                },
            },
        );
        let input_space = make_input_space(domains, vec![]);
        let encoded = super::super::domain::encode_input_space(&input_space).unwrap();
        let constraint_clauses = encode_constraints(&input_space.constraints, &encoded).unwrap();

        // Force role = "admin" via extra clause.
        let admin_lit = super::super::domain::lit_for_value(
            &encoded.domains["role"],
            &DomainValue::Enum("admin".into()),
        )
        .unwrap();
        let extra = vec![vec![admin_lit]];

        let result = find_one(&encoded, &constraint_clauses, &extra).unwrap();
        match result {
            SatResult::Sat(v) => {
                assert_eq!(v.assignments["role"], DomainValue::Enum("admin".into()));
            }
            SatResult::Unsat => panic!("expected SAT"),
        }
    }
}
