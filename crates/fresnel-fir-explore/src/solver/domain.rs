//! Domain encoding: maps IR input domains to SAT boolean variables.
//!
//! Encoding strategy:
//! - **Bool**: 1 SAT variable. `true` = domain true, `false` = domain false.
//! - **Enum**: One-hot encoding. N SAT variables for N variants.
//!   Exactly-one constraint added (at-least-one + pairwise at-most-one).
//! - **Int [min, max]**: Treated as enum over the range `min..=max`.
//!   Range must be bounded and reasonably small (max 1024 values).

use std::collections::BTreeMap;

use fresnel_fir_ir::types::{Domain, DomainType, InputSpace};
#[cfg(test)]
use varisat::ExtendFormula;
use varisat::{Lit, Var};

use super::DomainValue;

/// Maximum number of values allowed in a single integer domain.
const MAX_INT_RANGE: i64 = 1024;

/// Maps a domain variable name to its SAT encoding.
#[derive(Debug, Clone)]
pub struct EncodedDomain {
    /// The domain variable name from the IR.
    pub name: String,
    /// The encoding variant.
    pub encoding: Encoding,
}

/// How a single domain variable is encoded in SAT.
#[derive(Debug, Clone)]
pub enum Encoding {
    /// Single boolean variable.
    Bool { var: Var },
    /// One-hot: one SAT variable per value.
    OneHot {
        /// Ordered list of (value_label, SAT_variable).
        variants: Vec<(String, Var)>,
    },
}

/// All encoded domains plus their structural constraints (exactly-one for enums).
#[derive(Debug)]
pub struct EncodedInputSpace {
    /// Domain name -> encoding.
    pub domains: BTreeMap<String, EncodedDomain>,
    /// Structural clauses (exactly-one constraints for one-hot encodings).
    pub structural_clauses: Vec<Vec<Lit>>,
    /// Next free variable index.
    pub next_var: usize,
}

/// Errors during domain encoding.
#[derive(Debug, thiserror::Error)]
pub enum EncodingError {
    #[error("integer domain '{name}' has empty range: min={min}, max={max}")]
    EmptyIntRange { name: String, min: i64, max: i64 },

    #[error("integer domain '{name}' range too large: {size} values (max {MAX_INT_RANGE})")]
    IntRangeTooLarge { name: String, size: i64 },

    #[error("enum domain '{name}' has no values")]
    EmptyEnum { name: String },
}

/// Encode all domains from an IR InputSpace into SAT variables.
pub fn encode_input_space(input_space: &InputSpace) -> Result<EncodedInputSpace, EncodingError> {
    let mut domains = BTreeMap::new();
    let mut structural_clauses = Vec::new();
    let mut next_var: usize = 0;

    // Process domains in sorted order for determinism.
    let sorted_names: Vec<&String> = {
        let mut names: Vec<_> = input_space.domains.keys().collect();
        names.sort();
        names
    };

    for name in sorted_names {
        let domain = &input_space.domains[name];
        let encoded = encode_domain(name, domain, &mut next_var, &mut structural_clauses)?;
        domains.insert(name.clone(), encoded);
    }

    Ok(EncodedInputSpace {
        domains,
        structural_clauses,
        next_var,
    })
}

/// Encode a single domain variable.
fn encode_domain(
    name: &str,
    domain: &Domain,
    next_var: &mut usize,
    clauses: &mut Vec<Vec<Lit>>,
) -> Result<EncodedDomain, EncodingError> {
    let encoding = match &domain.domain_type {
        DomainType::Bool => {
            let var = Var::from_index(*next_var);
            *next_var += 1;
            Encoding::Bool { var }
        }

        DomainType::Enum { values } => {
            if values.is_empty() {
                return Err(EncodingError::EmptyEnum {
                    name: name.to_string(),
                });
            }
            let variants: Vec<(String, Var)> = values
                .iter()
                .map(|v| {
                    let var = Var::from_index(*next_var);
                    *next_var += 1;
                    (v.clone(), var)
                })
                .collect();

            // Exactly-one constraint:
            // 1) At-least-one: (v1 OR v2 OR ... OR vN)
            let at_least_one: Vec<Lit> = variants.iter().map(|(_, v)| v.positive()).collect();
            clauses.push(at_least_one);

            // 2) At-most-one: pairwise (!vi OR !vj) for all i < j
            for i in 0..variants.len() {
                for j in (i + 1)..variants.len() {
                    clauses.push(vec![variants[i].1.negative(), variants[j].1.negative()]);
                }
            }

            Encoding::OneHot { variants }
        }

        DomainType::Int { min, max } => {
            let (min, max) = (*min, *max);
            if min > max {
                return Err(EncodingError::EmptyIntRange {
                    name: name.to_string(),
                    min,
                    max,
                });
            }
            let size = max - min + 1;
            if size > MAX_INT_RANGE {
                return Err(EncodingError::IntRangeTooLarge {
                    name: name.to_string(),
                    size,
                });
            }

            // Encode as one-hot over the integer range.
            let variants: Vec<(String, Var)> = (min..=max)
                .map(|i| {
                    let var = Var::from_index(*next_var);
                    *next_var += 1;
                    (i.to_string(), var)
                })
                .collect();

            // Exactly-one constraint (same as enum).
            let at_least_one: Vec<Lit> = variants.iter().map(|(_, v)| v.positive()).collect();
            clauses.push(at_least_one);

            for i in 0..variants.len() {
                for j in (i + 1)..variants.len() {
                    clauses.push(vec![variants[i].1.negative(), variants[j].1.negative()]);
                }
            }

            Encoding::OneHot { variants }
        }
    };

    Ok(EncodedDomain {
        name: name.to_string(),
        encoding,
    })
}

/// Decode a SAT model (variable assignments) back to domain values.
pub fn decode_model(encoded: &EncodedInputSpace, model: &[Lit]) -> BTreeMap<String, DomainValue> {
    let mut assignments = BTreeMap::new();

    // Build a quick lookup: var_index -> is_true
    let mut var_assignment: BTreeMap<usize, bool> = BTreeMap::new();
    for lit in model {
        var_assignment.insert(lit.var().index(), lit.is_positive());
    }

    for (name, enc) in &encoded.domains {
        let value = decode_single(&enc.encoding, &var_assignment, name);
        if let Some(v) = value {
            assignments.insert(name.clone(), v);
        }
    }

    assignments
}

/// Decode a single domain variable from the model.
fn decode_single(
    encoding: &Encoding,
    var_assignment: &BTreeMap<usize, bool>,
    _domain_name: &str,
) -> Option<DomainValue> {
    match encoding {
        Encoding::Bool { var } => {
            let is_true = var_assignment.get(&var.index()).copied().unwrap_or(false);
            Some(DomainValue::Bool(is_true))
        }
        Encoding::OneHot { variants } => {
            // Find which variant is true.
            for (label, var) in variants {
                if var_assignment.get(&var.index()).copied().unwrap_or(false) {
                    // Determine if this is an int or enum domain.
                    // Try parsing as int first.
                    if let Ok(i) = label.parse::<i64>() {
                        return Some(DomainValue::Int(i));
                    }
                    return Some(DomainValue::Enum(label.clone()));
                }
            }
            // Fallback: if no variant is true in the model, pick the first one.
            // This shouldn't happen with correct exactly-one constraints.
            let label = &variants[0].0;
            if let Ok(i) = label.parse::<i64>() {
                Some(DomainValue::Int(i))
            } else {
                Some(DomainValue::Enum(label.clone()))
            }
        }
    }
}

/// Get the SAT literal for a specific domain value.
/// Returns `None` if the value doesn't exist in the domain.
pub fn lit_for_value(encoded: &EncodedDomain, value: &DomainValue) -> Option<Lit> {
    match (&encoded.encoding, value) {
        (Encoding::Bool { var }, DomainValue::Bool(true)) => Some(var.positive()),
        (Encoding::Bool { var }, DomainValue::Bool(false)) => Some(var.negative()),
        (Encoding::OneHot { variants }, DomainValue::Enum(s)) => variants
            .iter()
            .find(|(label, _)| label == s)
            .map(|(_, var)| var.positive()),
        (Encoding::OneHot { variants }, DomainValue::Int(i)) => {
            let label = i.to_string();
            variants
                .iter()
                .find(|(l, _)| *l == label)
                .map(|(_, var)| var.positive())
        }
        _ => None,
    }
}

/// Get the SAT literal that forces a domain to NOT take a specific value.
pub fn lit_for_not_value(encoded: &EncodedDomain, value: &DomainValue) -> Option<Lit> {
    lit_for_value(encoded, value).map(|l| !l)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fresnel_fir_ir::types::{CoverageConfig, InputSpace};
    use std::collections::HashMap;
    use varisat::solver::Solver;

    fn make_input_space(domains: HashMap<String, Domain>) -> InputSpace {
        InputSpace {
            domains,
            constraints: vec![],
            coverage: CoverageConfig {
                targets: vec![],
                seed: 42,
                reproducible: true,
            },
        }
    }

    #[test]
    fn test_encode_bool_domain() {
        let mut domains = HashMap::new();
        domains.insert(
            "flag".to_string(),
            Domain {
                domain_type: DomainType::Bool,
            },
        );
        let input_space = make_input_space(domains);
        let encoded = encode_input_space(&input_space).unwrap();

        assert_eq!(encoded.domains.len(), 1);
        let flag = &encoded.domains["flag"];
        assert!(matches!(flag.encoding, Encoding::Bool { .. }));
        // Bool domains produce no structural clauses.
        assert!(encoded.structural_clauses.is_empty());
    }

    #[test]
    fn test_encode_enum_domain() {
        let mut domains = HashMap::new();
        domains.insert(
            "role".to_string(),
            Domain {
                domain_type: DomainType::Enum {
                    values: vec!["admin".into(), "member".into(), "guest".into()],
                },
            },
        );
        let input_space = make_input_space(domains);
        let encoded = encode_input_space(&input_space).unwrap();

        let role = &encoded.domains["role"];
        match &role.encoding {
            Encoding::OneHot { variants } => {
                assert_eq!(variants.len(), 3);
                assert_eq!(variants[0].0, "admin");
                assert_eq!(variants[1].0, "member");
                assert_eq!(variants[2].0, "guest");
            }
            _ => panic!("expected OneHot encoding for enum"),
        }
        // 1 at-least-one + 3 pairwise at-most-one = 4 clauses.
        assert_eq!(encoded.structural_clauses.len(), 4);
    }

    #[test]
    fn test_encode_int_domain() {
        let mut domains = HashMap::new();
        domains.insert(
            "count".to_string(),
            Domain {
                domain_type: DomainType::Int { min: 1, max: 4 },
            },
        );
        let input_space = make_input_space(domains);
        let encoded = encode_input_space(&input_space).unwrap();

        let count = &encoded.domains["count"];
        match &count.encoding {
            Encoding::OneHot { variants } => {
                assert_eq!(variants.len(), 4); // 1, 2, 3, 4
                assert_eq!(variants[0].0, "1");
                assert_eq!(variants[3].0, "4");
            }
            _ => panic!("expected OneHot encoding for int range"),
        }
        // 1 at-least-one + 6 pairwise at-most-one = 7 clauses.
        assert_eq!(encoded.structural_clauses.len(), 7);
    }

    #[test]
    fn test_roundtrip_bool() {
        let mut domains = HashMap::new();
        domains.insert(
            "flag".to_string(),
            Domain {
                domain_type: DomainType::Bool,
            },
        );
        let input_space = make_input_space(domains);
        let encoded = encode_input_space(&input_space).unwrap();

        // Create a solver, add structural clauses, solve.
        let mut solver = Solver::new();
        for clause in &encoded.structural_clauses {
            solver.add_clause(clause);
        }
        assert!(solver.solve().unwrap());

        let model = solver.model().unwrap();
        let decoded = decode_model(&encoded, &model);
        assert_eq!(decoded.len(), 1);
        assert!(matches!(decoded["flag"], DomainValue::Bool(_)));
    }

    #[test]
    fn test_roundtrip_enum() {
        let mut domains = HashMap::new();
        domains.insert(
            "role".to_string(),
            Domain {
                domain_type: DomainType::Enum {
                    values: vec!["admin".into(), "member".into(), "guest".into()],
                },
            },
        );
        let input_space = make_input_space(domains);
        let encoded = encode_input_space(&input_space).unwrap();

        let mut solver = Solver::new();
        for clause in &encoded.structural_clauses {
            solver.add_clause(clause);
        }
        assert!(solver.solve().unwrap());

        let model = solver.model().unwrap();
        let decoded = decode_model(&encoded, &model);
        assert_eq!(decoded.len(), 1);
        match &decoded["role"] {
            DomainValue::Enum(v) => {
                assert!(["admin", "member", "guest"].contains(&v.as_str()));
            }
            other => panic!("expected Enum value, got {:?}", other),
        }
    }

    #[test]
    fn test_roundtrip_int() {
        let mut domains = HashMap::new();
        domains.insert(
            "count".to_string(),
            Domain {
                domain_type: DomainType::Int { min: 1, max: 8 },
            },
        );
        let input_space = make_input_space(domains);
        let encoded = encode_input_space(&input_space).unwrap();

        let mut solver = Solver::new();
        for clause in &encoded.structural_clauses {
            solver.add_clause(clause);
        }
        assert!(solver.solve().unwrap());

        let model = solver.model().unwrap();
        let decoded = decode_model(&encoded, &model);
        assert_eq!(decoded.len(), 1);
        match &decoded["count"] {
            DomainValue::Int(i) => {
                assert!((1..=8).contains(i));
            }
            other => panic!("expected Int value, got {:?}", other),
        }
    }

    #[test]
    fn test_roundtrip_multi_domain() {
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
            "authenticated".to_string(),
            Domain {
                domain_type: DomainType::Bool,
            },
        );
        domains.insert(
            "count".to_string(),
            Domain {
                domain_type: DomainType::Int { min: 1, max: 3 },
            },
        );
        let input_space = make_input_space(domains);
        let encoded = encode_input_space(&input_space).unwrap();

        let mut solver = Solver::new();
        for clause in &encoded.structural_clauses {
            solver.add_clause(clause);
        }
        assert!(solver.solve().unwrap());

        let model = solver.model().unwrap();
        let decoded = decode_model(&encoded, &model);
        assert_eq!(decoded.len(), 3);
        assert!(decoded.contains_key("role"));
        assert!(decoded.contains_key("authenticated"));
        assert!(decoded.contains_key("count"));
    }

    #[test]
    fn test_empty_enum_rejected() {
        let mut domains = HashMap::new();
        domains.insert(
            "bad".to_string(),
            Domain {
                domain_type: DomainType::Enum { values: vec![] },
            },
        );
        let input_space = make_input_space(domains);
        let result = encode_input_space(&input_space);
        assert!(result.is_err());
    }

    #[test]
    fn test_inverted_int_range_rejected() {
        let mut domains = HashMap::new();
        domains.insert(
            "bad".to_string(),
            Domain {
                domain_type: DomainType::Int { min: 10, max: 5 },
            },
        );
        let input_space = make_input_space(domains);
        let result = encode_input_space(&input_space);
        assert!(result.is_err());
    }

    #[test]
    fn test_lit_for_value_enum() {
        let mut domains = HashMap::new();
        domains.insert(
            "role".to_string(),
            Domain {
                domain_type: DomainType::Enum {
                    values: vec!["admin".into(), "guest".into()],
                },
            },
        );
        let input_space = make_input_space(domains);
        let encoded = encode_input_space(&input_space).unwrap();
        let role_enc = &encoded.domains["role"];

        let lit = lit_for_value(role_enc, &DomainValue::Enum("admin".into()));
        assert!(lit.is_some());
        assert!(lit.unwrap().is_positive());

        let lit_bad = lit_for_value(role_enc, &DomainValue::Enum("nonexistent".into()));
        assert!(lit_bad.is_none());
    }

    #[test]
    fn test_lit_for_value_bool() {
        let mut domains = HashMap::new();
        domains.insert(
            "flag".to_string(),
            Domain {
                domain_type: DomainType::Bool,
            },
        );
        let input_space = make_input_space(domains);
        let encoded = encode_input_space(&input_space).unwrap();
        let flag_enc = &encoded.domains["flag"];

        let lit_true = lit_for_value(flag_enc, &DomainValue::Bool(true));
        assert!(lit_true.is_some());
        assert!(lit_true.unwrap().is_positive());

        let lit_false = lit_for_value(flag_enc, &DomainValue::Bool(false));
        assert!(lit_false.is_some());
        assert!(lit_false.unwrap().is_negative());
    }
}
