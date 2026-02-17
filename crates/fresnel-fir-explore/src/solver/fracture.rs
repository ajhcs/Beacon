//! Input space fracturing by variable.
//!
//! Implements the fracture/solve/abort pipeline from the patents:
//! 1. Fracture input space by a variable → produces subspaces
//! 2. For each subspace, check SAT/UNSAT
//! 3. If UNSAT → abort (prune this subspace)
//! 4. If SAT → search for unique satisfying vectors
//! 5. Hierarchical: fracture subspaces further if coverage insufficient

use std::collections::BTreeMap;

use super::constraint::CnfClauses;
use super::domain::{lit_for_value, EncodedInputSpace, Encoding};
use super::search::{find_many, is_sat, SearchError};
use super::{DomainValue, TestVector};

/// A subspace created by fixing one or more domain variables.
#[derive(Debug, Clone)]
pub struct Subspace {
    /// The fixed variable assignments that define this subspace.
    pub fixed: BTreeMap<String, DomainValue>,
    /// Extra SAT clauses that enforce the fixings.
    pub fixing_clauses: CnfClauses,
    /// A unique identifier for this subspace (used for RNG seeding).
    pub stage_id: u64,
}

/// Result of fracturing and solving a subspace.
#[derive(Debug)]
pub enum SubspaceResult {
    /// Subspace is satisfiable — contains test vectors.
    Sat {
        subspace: Subspace,
        vectors: Vec<TestVector>,
    },
    /// Subspace is unsatisfiable — no valid assignments exist.
    Unsat { subspace: Subspace },
}

/// Fracture an input space by a single variable.
///
/// Given a variable name, produces one `Subspace` for each value
/// in the variable's domain, with that variable fixed and all
/// other variables free.
pub fn fracture_by_variable(
    encoded: &EncodedInputSpace,
    variable: &str,
    base_fixed: &BTreeMap<String, DomainValue>,
    base_clauses: &CnfClauses,
    base_stage_id: u64,
) -> Result<Vec<Subspace>, SearchError> {
    let domain_enc = encoded.domains.get(variable).ok_or_else(|| {
        SearchError::Solver(format!("unknown domain variable '{variable}' for fracture"))
    })?;

    let values = domain_values(&domain_enc.encoding);
    let mut subspaces = Vec::new();

    for (i, value) in values.iter().enumerate() {
        let lit = lit_for_value(domain_enc, value).ok_or_else(|| {
            SearchError::Solver(format!(
                "no SAT literal for value {value} in domain {variable}"
            ))
        })?;

        let mut fixed = base_fixed.clone();
        fixed.insert(variable.to_string(), value.clone());

        let mut fixing_clauses = base_clauses.clone();
        fixing_clauses.push(vec![lit]);

        let stage_id = base_stage_id * 1000 + i as u64;

        subspaces.push(Subspace {
            fixed,
            fixing_clauses,
            stage_id,
        });
    }

    Ok(subspaces)
}

/// Get all possible values for a domain encoding.
fn domain_values(encoding: &Encoding) -> Vec<DomainValue> {
    match encoding {
        Encoding::Bool { .. } => vec![DomainValue::Bool(false), DomainValue::Bool(true)],
        Encoding::OneHot { variants } => variants
            .iter()
            .map(|(label, _)| {
                if let Ok(i) = label.parse::<i64>() {
                    DomainValue::Int(i)
                } else {
                    DomainValue::Enum(label.clone())
                }
            })
            .collect(),
    }
}

/// Solve a single subspace: check SAT, then search for unique vectors.
pub fn solve_subspace(
    encoded: &EncodedInputSpace,
    constraint_clauses: &CnfClauses,
    subspace: &Subspace,
    max_vectors: usize,
) -> Result<SubspaceResult, SearchError> {
    // First check SAT.
    if !is_sat(encoded, constraint_clauses, &subspace.fixing_clauses)? {
        return Ok(SubspaceResult::Unsat {
            subspace: subspace.clone(),
        });
    }

    // SAT — search for unique vectors.
    let vectors = find_many(
        encoded,
        constraint_clauses,
        &subspace.fixing_clauses,
        max_vectors,
    )?;

    Ok(SubspaceResult::Sat {
        subspace: subspace.clone(),
        vectors,
    })
}

/// Fracture and solve: fracture by a variable, then solve each subspace.
///
/// Returns results for all subspaces (both SAT and UNSAT).
pub fn fracture_and_solve(
    encoded: &EncodedInputSpace,
    constraint_clauses: &CnfClauses,
    variable: &str,
    base_fixed: &BTreeMap<String, DomainValue>,
    base_clauses: &CnfClauses,
    base_stage_id: u64,
    max_vectors_per_subspace: usize,
) -> Result<Vec<SubspaceResult>, SearchError> {
    let subspaces =
        fracture_by_variable(encoded, variable, base_fixed, base_clauses, base_stage_id)?;

    let mut results = Vec::new();
    for subspace in &subspaces {
        results.push(solve_subspace(
            encoded,
            constraint_clauses,
            subspace,
            max_vectors_per_subspace,
        )?);
    }

    Ok(results)
}

/// Hierarchical fracture: fracture by multiple variables in sequence.
///
/// Fractures by the first variable, then for each SAT subspace,
/// fractures further by the next variable, and so on.
/// Returns all unique test vectors found across all leaf subspaces.
pub fn hierarchical_fracture(
    encoded: &EncodedInputSpace,
    constraint_clauses: &CnfClauses,
    variables: &[String],
    max_vectors_per_leaf: usize,
) -> Result<Vec<TestVector>, SearchError> {
    if variables.is_empty() {
        // No variables to fracture — just solve the whole space.
        return super::search::find_many(
            encoded,
            constraint_clauses,
            &vec![],
            max_vectors_per_leaf,
        );
    }

    let mut all_vectors = Vec::new();
    hierarchical_fracture_inner(
        encoded,
        constraint_clauses,
        variables,
        0,
        &BTreeMap::new(),
        &vec![],
        0,
        max_vectors_per_leaf,
        &mut all_vectors,
    )?;

    Ok(all_vectors)
}

#[allow(clippy::too_many_arguments)]
fn hierarchical_fracture_inner(
    encoded: &EncodedInputSpace,
    constraint_clauses: &CnfClauses,
    variables: &[String],
    depth: usize,
    fixed: &BTreeMap<String, DomainValue>,
    base_clauses: &CnfClauses,
    stage_id: u64,
    max_vectors_per_leaf: usize,
    results: &mut Vec<TestVector>,
) -> Result<(), SearchError> {
    if depth >= variables.len() {
        // Leaf level: solve this subspace for vectors.
        let vectors = find_many(
            encoded,
            constraint_clauses,
            base_clauses,
            max_vectors_per_leaf,
        )?;
        results.extend(vectors);
        return Ok(());
    }

    let variable = &variables[depth];
    let subspaces = fracture_by_variable(encoded, variable, fixed, base_clauses, stage_id)?;

    for subspace in &subspaces {
        // Quick SAT check before recursing.
        if !is_sat(encoded, constraint_clauses, &subspace.fixing_clauses)? {
            continue; // Abort UNSAT subspace.
        }

        hierarchical_fracture_inner(
            encoded,
            constraint_clauses,
            variables,
            depth + 1,
            &subspace.fixed,
            &subspace.fixing_clauses,
            subspace.stage_id,
            max_vectors_per_leaf,
            results,
        )?;
    }

    Ok(())
}

/// Collect all vectors from a list of subspace results.
pub fn collect_vectors(results: &[SubspaceResult]) -> Vec<TestVector> {
    results
        .iter()
        .filter_map(|r| match r {
            SubspaceResult::Sat { vectors, .. } => Some(vectors.clone()),
            SubspaceResult::Unsat { .. } => None,
        })
        .flatten()
        .collect()
}

/// Count SAT and UNSAT subspaces.
pub fn count_results(results: &[SubspaceResult]) -> (usize, usize) {
    let sat = results
        .iter()
        .filter(|r| matches!(r, SubspaceResult::Sat { .. }))
        .count();
    let unsat = results
        .iter()
        .filter(|r| matches!(r, SubspaceResult::Unsat { .. }))
        .count();
    (sat, unsat)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fresnel_fir_ir::expr::{Expr, Literal, OpKind};
    use fresnel_fir_ir::types::*;
    use std::collections::{HashMap, HashSet};

    use crate::solver::constraint::encode_constraints;
    use crate::solver::domain::encode_input_space;

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
    fn test_fracture_by_bool() {
        let mut domains = HashMap::new();
        domains.insert(
            "flag".to_string(),
            Domain {
                domain_type: DomainType::Bool,
            },
        );
        let input_space = make_input_space(domains, vec![]);
        let encoded = encode_input_space(&input_space).unwrap();

        let subspaces =
            fracture_by_variable(&encoded, "flag", &BTreeMap::new(), &vec![], 0).unwrap();

        assert_eq!(subspaces.len(), 2); // true and false
        assert_eq!(subspaces[0].fixed["flag"], DomainValue::Bool(false));
        assert_eq!(subspaces[1].fixed["flag"], DomainValue::Bool(true));
    }

    #[test]
    fn test_fracture_by_enum() {
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
        let encoded = encode_input_space(&input_space).unwrap();

        let subspaces =
            fracture_by_variable(&encoded, "role", &BTreeMap::new(), &vec![], 0).unwrap();

        assert_eq!(subspaces.len(), 3);
        assert_eq!(
            subspaces[0].fixed["role"],
            DomainValue::Enum("admin".into())
        );
        assert_eq!(
            subspaces[1].fixed["role"],
            DomainValue::Enum("member".into())
        );
        assert_eq!(
            subspaces[2].fixed["role"],
            DomainValue::Enum("guest".into())
        );
    }

    #[test]
    fn test_fracture_and_solve_with_constraint() {
        // Constraint: implies(role="guest", auth=false)
        // Fracture by "role":
        //   admin: SAT (auth can be true or false) -> 2 vectors
        //   guest: SAT (auth must be false) -> 1 vector
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
        let encoded = encode_input_space(&input_space).unwrap();
        let constraint_clauses = encode_constraints(&input_space.constraints, &encoded).unwrap();

        let results = fracture_and_solve(
            &encoded,
            &constraint_clauses,
            "role",
            &BTreeMap::new(),
            &vec![],
            0,
            0, // find all
        )
        .unwrap();

        let (sat, unsat) = count_results(&results);
        assert_eq!(sat, 2); // admin and guest are both SAT
        assert_eq!(unsat, 0);

        let vectors = collect_vectors(&results);
        assert_eq!(vectors.len(), 3); // admin+true, admin+false, guest+false
    }

    #[test]
    fn test_fracture_with_unsat_subspace() {
        // Constraint: role == "admin"
        // Fracture by "role":
        //   admin: SAT
        //   guest: UNSAT (contradicts constraint)
        let mut domains = HashMap::new();
        domains.insert(
            "role".to_string(),
            Domain {
                domain_type: DomainType::Enum {
                    values: vec!["admin".into(), "guest".into()],
                },
            },
        );

        let constraints = vec![InputConstraint {
            name: "must_admin".to_string(),
            rule: Expr::Op {
                op: OpKind::Eq,
                args: vec![
                    Expr::Literal(Literal::String("role".into())),
                    Expr::Literal(Literal::String("admin".into())),
                ],
            },
        }];

        let input_space = make_input_space(domains, constraints);
        let encoded = encode_input_space(&input_space).unwrap();
        let constraint_clauses = encode_constraints(&input_space.constraints, &encoded).unwrap();

        let results = fracture_and_solve(
            &encoded,
            &constraint_clauses,
            "role",
            &BTreeMap::new(),
            &vec![],
            0,
            0,
        )
        .unwrap();

        let (sat, unsat) = count_results(&results);
        assert_eq!(sat, 1); // only admin is SAT
        assert_eq!(unsat, 1); // guest is UNSAT

        let vectors = collect_vectors(&results);
        assert_eq!(vectors.len(), 1);
        assert_eq!(
            vectors[0].assignments["role"],
            DomainValue::Enum("admin".into())
        );
    }

    #[test]
    fn test_hierarchical_fracture() {
        // Fracture by role, then by auth.
        // No constraints -> 3 roles x 2 auth = 6 total vectors.
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
            Domain {
                domain_type: DomainType::Bool,
            },
        );

        let input_space = make_input_space(domains, vec![]);
        let encoded = encode_input_space(&input_space).unwrap();
        let constraint_clauses = encode_constraints(&input_space.constraints, &encoded).unwrap();

        let vectors = hierarchical_fracture(
            &encoded,
            &constraint_clauses,
            &["role".into(), "auth".into()],
            0,
        )
        .unwrap();

        // Should find all 6 combinations.
        let unique: HashSet<&TestVector> = vectors.iter().collect();
        assert_eq!(unique.len(), 6);
    }

    #[test]
    fn test_hierarchical_fracture_with_constraint() {
        // implies(guest, auth=false)
        // Fracture by role, then by auth.
        // admin: auth=true, auth=false -> 2
        // guest: auth=false (auth=true is UNSAT) -> 1
        // Total: 3
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
        let encoded = encode_input_space(&input_space).unwrap();
        let constraint_clauses = encode_constraints(&input_space.constraints, &encoded).unwrap();

        let vectors = hierarchical_fracture(
            &encoded,
            &constraint_clauses,
            &["role".into(), "auth".into()],
            0,
        )
        .unwrap();

        assert_eq!(vectors.len(), 3);

        for v in &vectors {
            if v.assignments["role"] == DomainValue::Enum("guest".into()) {
                assert_eq!(v.assignments["auth"], DomainValue::Bool(false));
            }
        }
    }

    #[test]
    fn test_stage_ids_are_unique() {
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
        let encoded = encode_input_space(&input_space).unwrap();

        let subspaces =
            fracture_by_variable(&encoded, "role", &BTreeMap::new(), &vec![], 1).unwrap();

        let ids: HashSet<u64> = subspaces.iter().map(|s| s.stage_id).collect();
        assert_eq!(ids.len(), 3); // All stage IDs are unique.
    }
}
