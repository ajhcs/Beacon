//! Parallel fracture/solve/abort pipeline.
//!
//! The full pipeline from the patents:
//! 1. Encode input space domains as SAT variables
//! 2. Encode constraints as SAT clauses
//! 3. Fracture by first variable -> subspaces
//! 4. Solve subspaces in parallel (rayon)
//! 5. Abort UNSAT subspaces immediately
//! 6. Search for unique vectors in SAT subspaces
//! 7. Hierarchical: fracture further if coverage insufficient
//! 8. Collect all vectors into the pool

use std::collections::{BTreeMap, HashSet};

use rayon::prelude::*;

use fresnel_fir_ir::types::InputSpace;

use super::constraint::{encode_constraints, CnfClauses};
use super::domain::{encode_input_space, EncodedInputSpace};
use super::fracture::{fracture_by_variable, Subspace};
use super::search::{find_many, is_sat, SearchError};
use super::{DomainValue, TestVector};

/// Configuration for the pipeline.
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Global RNG seed for reproducibility.
    pub seed: u64,
    /// Maximum vectors to search per leaf subspace.
    /// 0 = exhaustive.
    pub max_vectors_per_leaf: usize,
    /// Variables to fracture by, in order.
    /// If empty, just solve the whole space.
    pub fracture_variables: Vec<String>,
}

/// Result of running the full pipeline.
#[derive(Debug)]
pub struct PipelineResult {
    /// All unique test vectors generated.
    pub vectors: Vec<TestVector>,
    /// Number of subspaces that were SAT.
    pub sat_count: usize,
    /// Number of subspaces that were UNSAT (aborted).
    pub unsat_count: usize,
}

/// Run the full parallel fracture/solve/abort pipeline.
///
/// This is the top-level entry point for vector generation.
pub fn run_pipeline(
    input_space: &InputSpace,
    config: &PipelineConfig,
) -> Result<PipelineResult, SearchError> {
    let encoded = encode_input_space(input_space)?;
    let constraint_clauses = encode_constraints(&input_space.constraints, &encoded)?;

    if config.fracture_variables.is_empty() {
        // No fracturing — solve the whole space directly.
        let vectors = find_many(
            &encoded,
            &constraint_clauses,
            &vec![],
            config.max_vectors_per_leaf,
        )?;
        return Ok(PipelineResult {
            sat_count: if vectors.is_empty() { 0 } else { 1 },
            unsat_count: if vectors.is_empty() { 1 } else { 0 },
            vectors,
        });
    }

    let mut all_vectors = Vec::new();
    let mut sat_count = 0usize;
    let mut unsat_count = 0usize;

    parallel_fracture_recursive(
        &encoded,
        &constraint_clauses,
        &config.fracture_variables,
        0,
        &BTreeMap::new(),
        &vec![],
        0,
        config.max_vectors_per_leaf,
        &mut all_vectors,
        &mut sat_count,
        &mut unsat_count,
    )?;

    // Deduplicate vectors.
    let mut seen = HashSet::new();
    all_vectors.retain(|v| seen.insert(v.clone()));

    Ok(PipelineResult {
        vectors: all_vectors,
        sat_count,
        unsat_count,
    })
}

/// Recursive parallel fracture/solve.
///
/// At each depth, fractures by the current variable, then uses rayon
/// to solve all subspaces in parallel. UNSAT subspaces are aborted.
/// SAT subspaces are either recursed into (if more variables remain)
/// or searched for vectors (leaf level).
#[allow(clippy::too_many_arguments)]
fn parallel_fracture_recursive(
    encoded: &EncodedInputSpace,
    constraint_clauses: &CnfClauses,
    variables: &[String],
    depth: usize,
    fixed: &BTreeMap<String, DomainValue>,
    base_clauses: &CnfClauses,
    stage_id: u64,
    max_vectors_per_leaf: usize,
    results: &mut Vec<TestVector>,
    sat_count: &mut usize,
    unsat_count: &mut usize,
) -> Result<(), SearchError> {
    if depth >= variables.len() {
        // Leaf level: solve for vectors.
        if is_sat(encoded, constraint_clauses, base_clauses)? {
            *sat_count += 1;
            let vectors = find_many(
                encoded,
                constraint_clauses,
                base_clauses,
                max_vectors_per_leaf,
            )?;
            results.extend(vectors);
        } else {
            *unsat_count += 1;
        }
        return Ok(());
    }

    let variable = &variables[depth];
    let subspaces = fracture_by_variable(encoded, variable, fixed, base_clauses, stage_id)?;

    // Parallel SAT check across all subspaces.
    let sat_results: Vec<(usize, bool)> = subspaces
        .par_iter()
        .enumerate()
        .map(|(i, subspace)| {
            let sat =
                is_sat(encoded, constraint_clauses, &subspace.fixing_clauses).unwrap_or(false);
            (i, sat)
        })
        .collect();

    // Process results: abort UNSAT, recurse into SAT.
    for (i, is_satisfiable) in sat_results {
        if !is_satisfiable {
            *unsat_count += 1;
            continue; // Abort UNSAT subspace.
        }

        let subspace = &subspaces[i];
        parallel_fracture_recursive(
            encoded,
            constraint_clauses,
            variables,
            depth + 1,
            &subspace.fixed,
            &subspace.fixing_clauses,
            subspace.stage_id,
            max_vectors_per_leaf,
            results,
            sat_count,
            unsat_count,
        )?;
    }

    Ok(())
}

/// Run pipeline with parallel leaf solving.
///
/// Like `run_pipeline`, but at the leaf level, solves all SAT subspaces
/// concurrently using rayon. Better for workloads with many leaf subspaces.
pub fn run_pipeline_parallel_leaves(
    input_space: &InputSpace,
    config: &PipelineConfig,
) -> Result<PipelineResult, SearchError> {
    let encoded = encode_input_space(input_space)?;
    let constraint_clauses = encode_constraints(&input_space.constraints, &encoded)?;

    if config.fracture_variables.is_empty() {
        let vectors = find_many(
            &encoded,
            &constraint_clauses,
            &vec![],
            config.max_vectors_per_leaf,
        )?;
        return Ok(PipelineResult {
            sat_count: if vectors.is_empty() { 0 } else { 1 },
            unsat_count: if vectors.is_empty() { 1 } else { 0 },
            vectors,
        });
    }

    // Collect all leaf subspaces first.
    let mut leaves = Vec::new();
    let mut pruned_count = 0usize;
    collect_leaves(
        &encoded,
        &constraint_clauses,
        &config.fracture_variables,
        0,
        &BTreeMap::new(),
        &vec![],
        0,
        &mut leaves,
        &mut pruned_count,
    )?;

    // Solve all leaves in parallel.
    let leaf_results: Vec<Result<(Vec<TestVector>, bool), SearchError>> = leaves
        .par_iter()
        .map(|subspace| {
            if !is_sat(&encoded, &constraint_clauses, &subspace.fixing_clauses)? {
                return Ok((vec![], false));
            }
            let vectors = find_many(
                &encoded,
                &constraint_clauses,
                &subspace.fixing_clauses,
                config.max_vectors_per_leaf,
            )?;
            Ok((vectors, true))
        })
        .collect();

    let mut all_vectors = Vec::new();
    let mut sat_count = 0;
    let mut unsat_count = 0;

    for result in leaf_results {
        let (vectors, is_sat_result) = result?;
        if is_sat_result {
            sat_count += 1;
            all_vectors.extend(vectors);
        } else {
            unsat_count += 1;
        }
    }

    // Include pruned subspaces in the UNSAT count.
    unsat_count += pruned_count;

    // Deduplicate.
    let mut seen = HashSet::new();
    all_vectors.retain(|v| seen.insert(v.clone()));

    Ok(PipelineResult {
        vectors: all_vectors,
        sat_count,
        unsat_count,
    })
}

/// Recursively collect all leaf subspaces without solving them.
/// Tracks how many subspaces were pruned as UNSAT during collection.
#[allow(clippy::too_many_arguments)]
fn collect_leaves(
    encoded: &EncodedInputSpace,
    constraint_clauses: &CnfClauses,
    variables: &[String],
    depth: usize,
    fixed: &BTreeMap<String, DomainValue>,
    base_clauses: &CnfClauses,
    stage_id: u64,
    leaves: &mut Vec<Subspace>,
    pruned_count: &mut usize,
) -> Result<(), SearchError> {
    if depth >= variables.len() {
        leaves.push(Subspace {
            fixed: fixed.clone(),
            fixing_clauses: base_clauses.clone(),
            stage_id,
        });
        return Ok(());
    }

    let variable = &variables[depth];
    let subspaces = fracture_by_variable(encoded, variable, fixed, base_clauses, stage_id)?;

    // Quick parallel SAT check to prune early.
    let sat_checks: Vec<bool> = subspaces
        .par_iter()
        .map(|s| is_sat(encoded, constraint_clauses, &s.fixing_clauses).unwrap_or(false))
        .collect();

    for (i, subspace) in subspaces.iter().enumerate() {
        if !sat_checks[i] {
            *pruned_count += 1;
            continue; // Prune UNSAT.
        }
        collect_leaves(
            encoded,
            constraint_clauses,
            variables,
            depth + 1,
            &subspace.fixed,
            &subspace.fixing_clauses,
            subspace.stage_id,
            leaves,
            pruned_count,
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fresnel_fir_ir::expr::{Expr, Literal, OpKind};
    use fresnel_fir_ir::types::*;
    use std::collections::HashMap;

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
    fn test_pipeline_no_fracture() {
        let mut domains = HashMap::new();
        domains.insert(
            "flag".to_string(),
            Domain {
                domain_type: DomainType::Bool,
            },
        );
        let input_space = make_input_space(domains, vec![]);

        let config = PipelineConfig {
            seed: 42,
            max_vectors_per_leaf: 0,
            fracture_variables: vec![],
        };

        let result = run_pipeline(&input_space, &config).unwrap();
        assert_eq!(result.vectors.len(), 2);
    }

    #[test]
    fn test_pipeline_single_fracture() {
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

        let config = PipelineConfig {
            seed: 42,
            max_vectors_per_leaf: 0,
            fracture_variables: vec!["role".into()],
        };

        let result = run_pipeline(&input_space, &config).unwrap();
        assert_eq!(result.vectors.len(), 6); // 3 roles x 2 auth
        assert_eq!(result.sat_count, 3);
        assert_eq!(result.unsat_count, 0);
    }

    #[test]
    fn test_pipeline_with_constraint_and_abort() {
        // role = admin forced -> guest subspace is UNSAT
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

        let config = PipelineConfig {
            seed: 42,
            max_vectors_per_leaf: 0,
            fracture_variables: vec!["role".into()],
        };

        let result = run_pipeline(&input_space, &config).unwrap();
        assert_eq!(result.vectors.len(), 1);
        assert_eq!(result.sat_count, 1);
        assert_eq!(result.unsat_count, 1); // guest was aborted
    }

    #[test]
    fn test_pipeline_hierarchical() {
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
        domains.insert(
            "vis".to_string(),
            Domain {
                domain_type: DomainType::Enum {
                    values: vec!["private".into(), "public".into()],
                },
            },
        );

        let input_space = make_input_space(domains, vec![]);

        let config = PipelineConfig {
            seed: 42,
            max_vectors_per_leaf: 0,
            fracture_variables: vec!["role".into(), "auth".into(), "vis".into()],
        };

        let result = run_pipeline(&input_space, &config).unwrap();
        // 2 roles x 2 auth x 2 vis = 8
        assert_eq!(result.vectors.len(), 8);
    }

    #[test]
    fn test_pipeline_parallel_leaves() {
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

        let config = PipelineConfig {
            seed: 42,
            max_vectors_per_leaf: 0,
            fracture_variables: vec!["role".into(), "auth".into()],
        };

        let result = run_pipeline_parallel_leaves(&input_space, &config).unwrap();
        // admin+true, admin+false, member+true, member+false, guest+false = 5
        // guest+true is UNSAT
        assert_eq!(result.vectors.len(), 5);
        assert_eq!(result.unsat_count, 1); // guest+true
        assert_eq!(result.sat_count, 5);
    }

    #[test]
    fn test_pipeline_reproduces_same_vectors() {
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

        let input_space = make_input_space(domains, vec![]);

        let config = PipelineConfig {
            seed: 42,
            max_vectors_per_leaf: 0,
            fracture_variables: vec!["role".into()],
        };

        let result1 = run_pipeline(&input_space, &config).unwrap();
        let result2 = run_pipeline(&input_space, &config).unwrap();

        // Both runs should produce the same set of vectors.
        let set1: HashSet<_> = result1.vectors.iter().collect();
        let set2: HashSet<_> = result2.vectors.iter().collect();
        assert_eq!(set1, set2);
    }

    #[test]
    fn test_pipeline_design_doc_example() {
        // Full example from the design doc inputs section.
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

        let input_space = make_input_space(domains, constraints);

        let config = PipelineConfig {
            seed: 42,
            max_vectors_per_leaf: 0,
            fracture_variables: vec!["actor_role".into(), "doc_visibility".into()],
        };

        let result = run_pipeline_parallel_leaves(&input_space, &config).unwrap();

        // 3 roles x 3 vis = 9 leaf subspaces, all SAT (constraint is trivially true).
        // Each leaf has 2 auth x 2 owner x 8 concurrent_actors = 32 vectors.
        // Total: 9 * 32 = 288 vectors.
        assert_eq!(result.vectors.len(), 288);
        assert_eq!(result.unsat_count, 0);

        // Verify all vectors have 5 assignments.
        for v in &result.vectors {
            assert_eq!(v.assignments.len(), 5);
        }
    }
}
