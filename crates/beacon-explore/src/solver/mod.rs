pub mod domain;
pub mod constraint;
pub mod search;
pub mod fracture;
pub mod coverage;
pub mod pool;
pub mod rng;
pub mod pipeline;

use std::collections::BTreeMap;

/// A concrete assignment of values to input domain variables.
/// Uses BTreeMap for deterministic ordering and Hash support.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TestVector {
    /// Variable name -> assigned value (sorted for determinism)
    pub assignments: BTreeMap<String, DomainValue>,
}

impl TestVector {
    pub fn new() -> Self {
        Self {
            assignments: BTreeMap::new(),
        }
    }
}

impl Default for TestVector {
    fn default() -> Self {
        Self::new()
    }
}

/// A concrete value from a domain.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DomainValue {
    Bool(bool),
    Int(i64),
    Enum(String),
}

impl std::fmt::Display for DomainValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DomainValue::Bool(b) => write!(f, "{b}"),
            DomainValue::Int(i) => write!(f, "{i}"),
            DomainValue::Enum(s) => write!(f, "{s}"),
        }
    }
}
