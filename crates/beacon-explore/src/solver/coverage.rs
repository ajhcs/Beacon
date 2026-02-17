//! Coverage-driven vector generation.
//!
//! Generates test vectors targeting specific coverage points:
//! - **all-pairs**: For N variables, ensure every pair of values is covered.
//! - **boundary**: Boundary values for integer domains (min, max, min+1, max-1).
//! - **each-transition**: Each transition in a state machine (delegated to traversal).

use std::collections::HashSet;

use beacon_ir::types::{CoverageTarget, DomainType, InputSpace};

use super::constraint::{encode_constraints, CnfClauses};
use super::domain::{encode_input_space, EncodedInputSpace, lit_for_value};
use super::search::{find_many, SearchError};
use super::{DomainValue, TestVector};

/// A coverage point — a specific combination that must be exercised.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CoveragePoint {
    /// A pair of (var1=val1, var2=val2) that must appear in some vector.
    Pair {
        var1: String,
        val1: DomainValue,
        var2: String,
        val2: DomainValue,
    },
    /// A boundary value for a domain.
    Boundary {
        var: String,
        value: DomainValue,
    },
}

/// Result of coverage-driven generation.
#[derive(Debug)]
pub struct CoverageResult {
    /// Generated test vectors.
    pub vectors: Vec<TestVector>,
    /// Coverage points that were hit.
    pub covered: HashSet<CoveragePoint>,
    /// Coverage points that could not be covered (UNSAT).
    pub uncoverable: HashSet<CoveragePoint>,
    /// Total coverage points targeted.
    pub total_targets: usize,
}

/// Generate all-pairs coverage targets for the given variables.
pub fn all_pairs_targets(
    input_space: &InputSpace,
    variables: &[String],
) -> Vec<CoveragePoint> {
    let mut targets = Vec::new();

    for i in 0..variables.len() {
        for j in (i + 1)..variables.len() {
            let var1 = &variables[i];
            let var2 = &variables[j];

            let vals1 = domain_values(input_space, var1);
            let vals2 = domain_values(input_space, var2);

            for v1 in &vals1 {
                for v2 in &vals2 {
                    targets.push(CoveragePoint::Pair {
                        var1: var1.clone(),
                        val1: v1.clone(),
                        var2: var2.clone(),
                        val2: v2.clone(),
                    });
                }
            }
        }
    }

    targets
}

/// Generate boundary value targets for a domain.
pub fn boundary_targets(
    input_space: &InputSpace,
    domain_name: &str,
    explicit_values: &[serde_json::Value],
) -> Vec<CoveragePoint> {
    let mut targets = Vec::new();

    // Add explicit boundary values from the IR.
    for val in explicit_values {
        if let Some(i) = val.as_i64() {
            targets.push(CoveragePoint::Boundary {
                var: domain_name.to_string(),
                value: DomainValue::Int(i),
            });
        } else if let Some(s) = val.as_str() {
            targets.push(CoveragePoint::Boundary {
                var: domain_name.to_string(),
                value: DomainValue::Enum(s.to_string()),
            });
        } else if let Some(b) = val.as_bool() {
            targets.push(CoveragePoint::Boundary {
                var: domain_name.to_string(),
                value: DomainValue::Bool(b),
            });
        }
    }

    // For integer domains, also add automatic boundary values.
    if let Some(domain) = input_space.domains.get(domain_name) {
        if let DomainType::Int { min, max } = &domain.domain_type {
            let mut auto = vec![*min, *max];
            if max - min > 1 {
                auto.push(min + 1);
                auto.push(max - 1);
            }
            for val in auto {
                let point = CoveragePoint::Boundary {
                    var: domain_name.to_string(),
                    value: DomainValue::Int(val),
                };
                if !targets.contains(&point) {
                    targets.push(point);
                }
            }
        }
    }

    targets
}

/// Extract all coverage targets from an InputSpace's coverage config.
pub fn extract_targets(input_space: &InputSpace) -> Vec<CoveragePoint> {
    let mut targets = Vec::new();

    for target in &input_space.coverage.targets {
        match target {
            CoverageTarget::AllPairs { over } => {
                targets.extend(all_pairs_targets(input_space, over));
            }
            CoverageTarget::Boundary { domain, values } => {
                targets.extend(boundary_targets(input_space, domain, values));
            }
            CoverageTarget::EachTransition { .. } => {
                // Transition coverage is delegated to the traversal engine.
                // The solver doesn't handle it directly.
            }
        }
    }

    targets
}

/// Check which coverage points a set of vectors covers.
pub fn check_coverage(
    vectors: &[TestVector],
    targets: &[CoveragePoint],
) -> HashSet<CoveragePoint> {
    let mut covered = HashSet::new();

    for target in targets {
        match target {
            CoveragePoint::Pair {
                var1,
                val1,
                var2,
                val2,
            } => {
                if vectors.iter().any(|v| {
                    v.assignments.get(var1.as_str()) == Some(val1)
                        && v.assignments.get(var2.as_str()) == Some(val2)
                }) {
                    covered.insert(target.clone());
                }
            }
            CoveragePoint::Boundary { var, value } => {
                if vectors
                    .iter()
                    .any(|v| v.assignments.get(var.as_str()) == Some(value))
                {
                    covered.insert(target.clone());
                }
            }
        }
    }

    covered
}

/// Generate vectors to cover specific uncovered points.
///
/// For each uncovered point, generates a vector that satisfies
/// the point's requirements plus all domain/constraint constraints.
pub fn generate_for_targets(
    encoded: &EncodedInputSpace,
    constraint_clauses: &CnfClauses,
    uncovered: &[CoveragePoint],
) -> Result<Vec<TestVector>, SearchError> {
    let mut result = Vec::new();

    for point in uncovered {
        let extra_clauses = point_to_clauses(point, encoded)?;
        let vectors = find_many(encoded, constraint_clauses, &extra_clauses, 1)?;
        result.extend(vectors);
    }

    Ok(result)
}

/// Convert a coverage point into extra SAT clauses that force it.
fn point_to_clauses(
    point: &CoveragePoint,
    encoded: &EncodedInputSpace,
) -> Result<CnfClauses, SearchError> {
    let mut clauses = Vec::new();

    match point {
        CoveragePoint::Pair {
            var1,
            val1,
            var2,
            val2,
        } => {
            let enc1 = encoded.domains.get(var1).ok_or_else(|| {
                SearchError::Solver(format!("unknown domain '{var1}' in coverage target"))
            })?;
            let enc2 = encoded.domains.get(var2).ok_or_else(|| {
                SearchError::Solver(format!("unknown domain '{var2}' in coverage target"))
            })?;

            let lit1 = lit_for_value(enc1, val1).ok_or_else(|| {
                SearchError::Solver(format!("no SAT literal for {val1} in {var1}"))
            })?;
            let lit2 = lit_for_value(enc2, val2).ok_or_else(|| {
                SearchError::Solver(format!("no SAT literal for {val2} in {var2}"))
            })?;

            clauses.push(vec![lit1]);
            clauses.push(vec![lit2]);
        }
        CoveragePoint::Boundary { var, value } => {
            let enc = encoded.domains.get(var).ok_or_else(|| {
                SearchError::Solver(format!("unknown domain '{var}' in coverage target"))
            })?;

            let lit = lit_for_value(enc, value).ok_or_else(|| {
                SearchError::Solver(format!("no SAT literal for {value} in {var}"))
            })?;

            clauses.push(vec![lit]);
        }
    }

    Ok(clauses)
}

/// Full coverage-driven generation pipeline.
///
/// 1. Extract coverage targets from the IR.
/// 2. Generate base vectors (e.g., from fracture pipeline).
/// 3. Check which targets are covered.
/// 4. For uncovered targets, generate targeted vectors.
/// 5. Return combined vectors + coverage report.
pub fn coverage_driven_generation(
    input_space: &InputSpace,
) -> Result<CoverageResult, SearchError> {
    let encoded = encode_input_space(input_space)?;
    let constraint_clauses = encode_constraints(&input_space.constraints, &encoded)?;
    let targets = extract_targets(input_space);

    if targets.is_empty() {
        // No coverage targets — just solve for all vectors.
        let vectors = find_many(&encoded, &constraint_clauses, &vec![], 0)?;
        return Ok(CoverageResult {
            vectors,
            covered: HashSet::new(),
            uncoverable: HashSet::new(),
            total_targets: 0,
        });
    }

    // First pass: generate targeted vectors for each coverage point.
    let mut vectors = Vec::new();
    let mut uncoverable = HashSet::new();

    for target in &targets {
        let extra = point_to_clauses(target, &encoded)?;
        let found = find_many(&encoded, &constraint_clauses, &extra, 1)?;
        if found.is_empty() {
            uncoverable.insert(target.clone());
        } else {
            vectors.extend(found);
        }
    }

    // Deduplicate vectors.
    let mut seen = HashSet::new();
    vectors.retain(|v| seen.insert(v.clone()));

    let covered = check_coverage(&vectors, &targets);

    Ok(CoverageResult {
        vectors,
        covered,
        uncoverable,
        total_targets: targets.len(),
    })
}

/// Get all possible values for a domain variable from the InputSpace.
fn domain_values(input_space: &InputSpace, var: &str) -> Vec<DomainValue> {
    if let Some(domain) = input_space.domains.get(var) {
        match &domain.domain_type {
            DomainType::Bool => vec![DomainValue::Bool(false), DomainValue::Bool(true)],
            DomainType::Enum { values } => {
                values.iter().map(|v| DomainValue::Enum(v.clone())).collect()
            }
            DomainType::Int { min, max } => (*min..=*max).map(DomainValue::Int).collect(),
        }
    } else {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use beacon_ir::types::*;
    use std::collections::HashMap;

    fn make_input_space(
        domains: HashMap<String, Domain>,
        constraints: Vec<InputConstraint>,
        coverage_targets: Vec<CoverageTarget>,
    ) -> InputSpace {
        InputSpace {
            domains,
            constraints,
            coverage: CoverageConfig {
                targets: coverage_targets,
                seed: 42,
                reproducible: true,
            },
        }
    }

    #[test]
    fn test_all_pairs_targets_count() {
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
            "vis".to_string(),
            Domain {
                domain_type: DomainType::Enum {
                    values: vec!["private".into(), "public".into()],
                },
            },
        );
        domains.insert(
            "owner".to_string(),
            Domain { domain_type: DomainType::Bool },
        );

        let input_space = make_input_space(domains, vec![], vec![]);

        let targets = all_pairs_targets(
            &input_space,
            &["role".into(), "vis".into(), "owner".into()],
        );

        // role x vis = 3*3 = 9
        // role x owner = 3*2 = 6
        // vis x owner = 3*2 = 6
        // Total: 21
        assert_eq!(targets.len(), 21);
    }

    #[test]
    fn test_boundary_targets() {
        let mut domains = HashMap::new();
        domains.insert(
            "count".to_string(),
            Domain {
                domain_type: DomainType::Int { min: 1, max: 8 },
            },
        );

        let input_space = make_input_space(domains, vec![], vec![]);

        let targets = boundary_targets(
            &input_space,
            "count",
            &[serde_json::json!(1), serde_json::json!(2), serde_json::json!(8)],
        );

        // Explicit: 1, 2, 8
        // Auto: min=1, max=8, min+1=2, max-1=7 (1,8 already present, 2 already present)
        // Unique: 1, 2, 8, 7
        assert_eq!(targets.len(), 4);
    }

    #[test]
    fn test_check_coverage() {
        let mut v1 = TestVector::new();
        v1.assignments
            .insert("role".into(), DomainValue::Enum("admin".into()));
        v1.assignments
            .insert("vis".into(), DomainValue::Enum("private".into()));

        let mut v2 = TestVector::new();
        v2.assignments
            .insert("role".into(), DomainValue::Enum("guest".into()));
        v2.assignments
            .insert("vis".into(), DomainValue::Enum("public".into()));

        let targets = vec![
            CoveragePoint::Pair {
                var1: "role".into(),
                val1: DomainValue::Enum("admin".into()),
                var2: "vis".into(),
                val2: DomainValue::Enum("private".into()),
            },
            CoveragePoint::Pair {
                var1: "role".into(),
                val1: DomainValue::Enum("admin".into()),
                var2: "vis".into(),
                val2: DomainValue::Enum("public".into()),
            },
            CoveragePoint::Pair {
                var1: "role".into(),
                val1: DomainValue::Enum("guest".into()),
                var2: "vis".into(),
                val2: DomainValue::Enum("public".into()),
            },
        ];

        let covered = check_coverage(&[v1, v2], &targets);
        assert_eq!(covered.len(), 2); // admin+private and guest+public
    }

    #[test]
    fn test_coverage_driven_generation_all_pairs() {
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
            "vis".to_string(),
            Domain {
                domain_type: DomainType::Enum {
                    values: vec!["private".into(), "shared".into(), "public".into()],
                },
            },
        );
        domains.insert(
            "owner".to_string(),
            Domain { domain_type: DomainType::Bool },
        );

        let coverage_targets = vec![CoverageTarget::AllPairs {
            over: vec!["role".into(), "vis".into(), "owner".into()],
        }];

        let input_space = make_input_space(domains, vec![], coverage_targets);
        let result = coverage_driven_generation(&input_space).unwrap();

        // All 21 pairs should be covered.
        assert_eq!(result.total_targets, 21);
        assert_eq!(result.covered.len(), 21);
        assert!(result.uncoverable.is_empty());
    }

    #[test]
    fn test_coverage_driven_generation_boundary() {
        let mut domains = HashMap::new();
        domains.insert(
            "count".to_string(),
            Domain {
                domain_type: DomainType::Int { min: 1, max: 8 },
            },
        );

        let coverage_targets = vec![CoverageTarget::Boundary {
            domain: "count".to_string(),
            values: vec![
                serde_json::json!(1),
                serde_json::json!(2),
                serde_json::json!(8),
            ],
        }];

        let input_space = make_input_space(domains, vec![], coverage_targets);
        let result = coverage_driven_generation(&input_space).unwrap();

        // All boundary values should be covered.
        assert!(result.uncoverable.is_empty());
        assert_eq!(result.covered.len(), result.total_targets);
    }

    #[test]
    fn test_coverage_with_constraint_makes_pair_uncoverable() {
        // role=guest and auth=true should be uncoverable
        // if constraint says implies(guest, auth=false).
        use beacon_ir::expr::{Expr, Literal, OpKind};

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

        let coverage_targets = vec![CoverageTarget::AllPairs {
            over: vec!["role".into(), "auth".into()],
        }];

        let input_space = make_input_space(domains, constraints, coverage_targets);
        let result = coverage_driven_generation(&input_space).unwrap();

        // 4 pairs total: admin+true, admin+false, guest+true, guest+false
        assert_eq!(result.total_targets, 4);
        // guest+true is uncoverable
        assert_eq!(result.uncoverable.len(), 1);
        assert!(result.uncoverable.contains(&CoveragePoint::Pair {
            var1: "role".into(),
            val1: DomainValue::Enum("guest".into()),
            var2: "auth".into(),
            val2: DomainValue::Bool(true),
        }));
        assert_eq!(result.covered.len(), 3);
    }
}
