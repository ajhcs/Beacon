use fresnel_fir_compiler::graph::BranchEdge;
use rand::Rng;
use rand_chacha::ChaCha8Rng;

use super::weight_table::WeightTable;

/// Strategy decision at an alt node — which branch to take.
#[derive(Debug, Clone)]
pub struct BranchDecision {
    pub branch_index: usize,
    pub branch_id: String,
    pub weight_used: f64,
}

/// Strategy decision at a repeat node — how many iterations.
#[derive(Debug, Clone)]
pub struct RepeatDecision {
    pub iterations: u32,
}

/// A traversal strategy — the "brain" that makes decisions at branch/loop points.
pub trait Strategy {
    /// Select a branch at an alt node, given the current model state hash.
    fn select_branch(
        &mut self,
        branches: &[BranchEdge],
        model_state_hash: u64,
        weight_table: &WeightTable,
    ) -> BranchDecision;

    /// Choose iteration count at a repeat node.
    fn choose_iterations(&mut self, min: u32, max: u32) -> RepeatDecision;

    /// Name of this strategy (for tracing).
    fn name(&self) -> &str;
}

/// Pseudo-random traversal strategy — the default.
///
/// Uses weighted random selection at alt branches and random iteration
/// counts at repeat nodes, seeded for reproducibility.
pub struct PseudoRandomStrategy {
    rng: ChaCha8Rng,
}

impl PseudoRandomStrategy {
    pub fn new(rng: ChaCha8Rng) -> Self {
        Self { rng }
    }
}

impl Strategy for PseudoRandomStrategy {
    fn select_branch(
        &mut self,
        branches: &[BranchEdge],
        model_state_hash: u64,
        weight_table: &WeightTable,
    ) -> BranchDecision {
        // Collect state-conditioned weights
        let weights: Vec<f64> = branches
            .iter()
            .map(|b| weight_table.get(&b.id, model_state_hash).max(0.0))
            .collect();

        let total: f64 = weights.iter().sum();
        if total <= 0.0 {
            // Fallback: pick first branch
            return BranchDecision {
                branch_index: 0,
                branch_id: branches[0].id.clone(),
                weight_used: 0.0,
            };
        }

        // Weighted random selection
        let mut roll: f64 = self.rng.gen::<f64>() * total;
        for (i, (branch, &weight)) in branches.iter().zip(weights.iter()).enumerate() {
            roll -= weight;
            if roll <= 0.0 {
                return BranchDecision {
                    branch_index: i,
                    branch_id: branch.id.clone(),
                    weight_used: weight,
                };
            }
        }

        // Shouldn't reach here, but fallback to last
        let last = branches.len() - 1;
        BranchDecision {
            branch_index: last,
            branch_id: branches[last].id.clone(),
            weight_used: weights[last],
        }
    }

    fn choose_iterations(&mut self, min: u32, max: u32) -> RepeatDecision {
        let iterations = if min == max {
            min
        } else {
            self.rng.gen_range(min..=max)
        };
        RepeatDecision { iterations }
    }

    fn name(&self) -> &str {
        "pseudo_random"
    }
}

/// Strategy stack — supports push/pop for nested strategy changes.
/// Depth limit prevents unbounded growth.
pub struct StrategyStack {
    stack: Vec<Box<dyn Strategy>>,
    depth_limit: usize,
}

impl StrategyStack {
    pub fn new(base: Box<dyn Strategy>, depth_limit: usize) -> Self {
        Self {
            stack: vec![base],
            depth_limit,
        }
    }

    /// Get the current (top) strategy.
    pub fn current(&mut self) -> &mut dyn Strategy {
        self.stack
            .last_mut()
            .expect("strategy stack is never empty")
            .as_mut()
    }

    /// Push a new strategy. If depth limit exceeded, pop the oldest.
    pub fn push(&mut self, strategy: Box<dyn Strategy>) {
        if self.stack.len() >= self.depth_limit {
            // Remove the second-oldest (keep base)
            if self.stack.len() > 1 {
                self.stack.remove(1);
            }
        }
        self.stack.push(strategy);
    }

    /// Pop the current strategy, returning to the previous one.
    /// Never pops the base strategy.
    pub fn pop(&mut self) -> Option<Box<dyn Strategy>> {
        if self.stack.len() > 1 {
            self.stack.pop()
        } else {
            None
        }
    }

    /// Current stack depth.
    pub fn depth(&self) -> usize {
        self.stack.len()
    }
}
