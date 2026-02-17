use std::collections::HashMap;

/// Key for the weight table: (AltBranchId, AbstractModelStateId).
///
/// Weights are state-conditioned — "branch B is unproductive WHEN model is in
/// state S" not "branch B is globally unproductive."
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WeightKey {
    pub branch_id: String,
    pub model_state_hash: u64,
}

/// State-conditioned weight table for alt branch selection.
///
/// Maps (AltBranchId, AbstractModelStateId) -> weight.
/// Initial weights come from the protocol definition.
#[derive(Debug, Clone)]
pub struct WeightTable {
    weights: HashMap<WeightKey, f64>,
    /// Default weights per branch ID (from protocol definition).
    defaults: HashMap<String, f64>,
}

impl WeightTable {
    pub fn new() -> Self {
        Self {
            weights: HashMap::new(),
            defaults: HashMap::new(),
        }
    }

    /// Set the default weight for a branch (from protocol definition).
    pub fn set_default(&mut self, branch_id: &str, weight: f64) {
        self.defaults.insert(branch_id.to_string(), weight);
    }

    /// Get the weight for a branch in a given model state.
    /// Falls back to the default weight if no state-specific weight exists.
    pub fn get(&self, branch_id: &str, model_state_hash: u64) -> f64 {
        let key = WeightKey {
            branch_id: branch_id.to_string(),
            model_state_hash,
        };
        if let Some(&w) = self.weights.get(&key) {
            w
        } else {
            self.defaults.get(branch_id).copied().unwrap_or(1.0)
        }
    }

    /// Set a state-conditioned weight.
    pub fn set(&mut self, branch_id: &str, model_state_hash: u64, weight: f64) {
        let key = WeightKey {
            branch_id: branch_id.to_string(),
            model_state_hash,
        };
        self.weights.insert(key, weight);
    }

    /// Adjust a weight by a multiplier.
    pub fn adjust(&mut self, branch_id: &str, model_state_hash: u64, multiplier: f64) {
        let current = self.get(branch_id, model_state_hash);
        self.set(branch_id, model_state_hash, current * multiplier);
    }

    /// Normalize all weights for branches sharing the same alt block.
    /// Branch IDs within the same alt block should share a common prefix.
    /// Takes a set of branch IDs to normalize together, target sum defaults to 100.
    pub fn normalize(&mut self, branch_ids: &[&str], model_state_hash: u64) {
        let total: f64 = branch_ids
            .iter()
            .map(|id| self.get(id, model_state_hash))
            .sum();

        if total <= 0.0 {
            return;
        }

        for id in branch_ids {
            let current = self.get(id, model_state_hash);
            self.set(id, model_state_hash, (current / total) * 100.0);
        }
    }

    /// Apply per-epoch decay to all state-conditioned weights.
    pub fn decay_all(&mut self, factor: f64) {
        for weight in self.weights.values_mut() {
            *weight *= factor;
        }
    }
}

impl Default for WeightTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute an abstract model state hash from field values relevant to guards.
/// This is a simplified hash — the full implementation would hash only fields
/// referenced by guards in the alt block.
pub fn compute_model_state_hash(field_values: &[(&str, &str)]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    for (field, value) in field_values {
        field.hash(&mut hasher);
        value.hash(&mut hasher);
    }
    hasher.finish()
}
