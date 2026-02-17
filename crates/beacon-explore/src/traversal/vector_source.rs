use crate::solver::{DomainValue, TestVector};

/// Abstract source of input vectors for action execution.
///
/// Abstracted behind a trait so we can:
/// - Use MockVectorSource for testing (returns predefined vectors)
/// - Plug in the real solver's vector pool later
pub trait VectorSource {
    /// Get the next input vector for the given action.
    /// Returns None if no vectors are available.
    fn next_vector(&mut self, action: &str) -> Option<TestVector>;
}

/// Mock vector source for testing — returns predefined or default vectors.
pub struct MockVectorSource {
    /// Predefined vectors per action. Pops from front.
    vectors: std::collections::HashMap<String, Vec<TestVector>>,
    /// Default vector returned when no predefined vectors remain.
    default_args: Vec<i32>,
}

impl MockVectorSource {
    pub fn new() -> Self {
        Self {
            vectors: std::collections::HashMap::new(),
            default_args: vec![1],
        }
    }

    /// Add predefined vectors for a specific action.
    pub fn add_vectors(&mut self, action: &str, vectors: Vec<TestVector>) {
        self.vectors.insert(action.to_string(), vectors);
    }

    /// Set default args returned when no predefined vectors remain.
    pub fn set_default_args(&mut self, args: Vec<i32>) {
        self.default_args = args.clone();
    }

    /// Create a test vector from simple i32 args.
    pub fn vector_from_args(args: &[(&str, i32)]) -> TestVector {
        let mut tv = TestVector::new();
        for (name, val) in args {
            tv.assignments
                .insert(name.to_string(), DomainValue::Int(*val as i64));
        }
        tv
    }
}

impl Default for MockVectorSource {
    fn default() -> Self {
        Self::new()
    }
}

impl VectorSource for MockVectorSource {
    fn next_vector(&mut self, action: &str) -> Option<TestVector> {
        if let Some(queue) = self.vectors.get_mut(action) {
            if !queue.is_empty() {
                return Some(queue.remove(0));
            }
        }
        // Return a default vector
        let mut tv = TestVector::new();
        for (i, val) in self.default_args.iter().enumerate() {
            tv.assignments
                .insert(format!("arg{}", i), DomainValue::Int(*val as i64));
        }
        Some(tv)
    }
}
