use beacon_compiler::graph::NdaGraph;
use beacon_ir::types::BeaconIR;
use beacon_model::invariant::CompiledProperty;
use beacon_model::state::{InstanceId, ModelState};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use super::engine::{ActionExecutor, TraversalEngine};
use super::signal::Finding;
use super::strategy::{PseudoRandomStrategy, StrategyStack};
use super::vector_source::VectorSource;
use super::weight_table::WeightTable;

/// Configuration for a campaign run.
#[derive(Debug, Clone)]
pub struct CampaignConfig {
    /// Number of traversal passes to run.
    pub max_passes: u32,
    /// RNG seed for reproducibility.
    pub seed: u64,
    /// Strategy stack depth limit.
    pub strategy_depth_limit: usize,
    /// Max steps per pass (prevents infinite loops).
    pub max_steps_per_pass: u64,
}

impl Default for CampaignConfig {
    fn default() -> Self {
        Self {
            max_passes: 10,
            seed: 42,
            strategy_depth_limit: 4,
            max_steps_per_pass: 10_000,
        }
    }
}

/// Result of a complete campaign run.
#[derive(Debug)]
pub struct CampaignResult {
    /// All findings across all passes.
    pub findings: Vec<Finding>,
    /// Total actions executed.
    pub total_actions: u64,
    /// Total passes completed.
    pub passes_completed: u32,
    /// Total unique nodes visited.
    pub unique_nodes_visited: u64,
    /// Total guard failures.
    pub total_guard_failures: u64,
}

/// Run a single-threaded campaign: create engine per pass, aggregate results.
pub fn run_campaign<V: VectorSource, E: ActionExecutor>(
    graph: &NdaGraph,
    model: &mut ModelState,
    executor: &mut E,
    ir: &BeaconIR,
    invariants: &[CompiledProperty],
    actor_id: InstanceId,
    vector_source: &mut V,
    config: &CampaignConfig,
) -> CampaignResult {
    let rng = ChaCha8Rng::seed_from_u64(config.seed);
    let base_strategy = Box::new(PseudoRandomStrategy::new(rng));
    let mut strategy_stack = StrategyStack::new(base_strategy, config.strategy_depth_limit);
    let mut weight_table = WeightTable::new();

    let mut all_findings = Vec::new();
    let mut total_actions = 0u64;
    let mut total_guard_failures = 0u64;
    let mut max_nodes_visited = 0u64;

    for _pass in 0..config.max_passes {
        let engine = TraversalEngine::new(
            graph,
            model,
            ExecutorRef(executor),
            ir,
            invariants,
            actor_id.clone(),
            &mut strategy_stack,
            vector_source,
            &mut weight_table,
        );

        let result = engine.run_pass(config.max_steps_per_pass);

        total_actions += result.actions_executed;
        total_guard_failures += result.guards_failed;
        if result.nodes_visited > max_nodes_visited {
            max_nodes_visited = result.nodes_visited;
        }

        all_findings.extend(result.findings);
    }

    CampaignResult {
        findings: all_findings,
        total_actions,
        passes_completed: config.max_passes,
        unique_nodes_visited: max_nodes_visited,
        total_guard_failures,
    }
}

/// Wrapper to delegate ActionExecutor through a mutable reference.
/// This lets run_campaign reuse a single executor across passes.
struct ExecutorRef<'a, E: ActionExecutor>(&'a mut E);

impl<'a, E: ActionExecutor> ActionExecutor for ExecutorRef<'a, E> {
    fn execute(
        &mut self,
        action: &str,
        vector: Option<&crate::solver::TestVector>,
    ) -> super::engine::ActionOutcome {
        self.0.execute(action, vector)
    }
}
