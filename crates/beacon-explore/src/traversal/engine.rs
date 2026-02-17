use std::collections::HashMap;

use beacon_compiler::graph::{GraphNode, NdaGraph, NodeId};
use beacon_ir::types::BeaconIR;
use beacon_model::effect::apply_effect;
use beacon_model::invariant::{check_invariants, CompiledProperty};
use beacon_model::state::{InstanceId, ModelState, Value};

use super::signal::{Finding, SignalEvent, SignalType};
use super::strategy::StrategyStack;
use super::trace::{TraceStepKind, TraversalTrace};
use super::vector_source::VectorSource;
use super::weight_table::WeightTable;
use crate::solver::{DomainValue, TestVector};

/// Result of executing a single DUT action.
#[derive(Debug)]
pub struct ActionOutcome {
    /// Return value from the DUT (None for model-only or void).
    pub return_value: Option<i32>,
    /// Whether the call trapped/panicked.
    pub trapped: bool,
    /// Fuel consumed during execution.
    pub fuel_consumed: Option<u64>,
    /// Error message if the call failed.
    pub error: Option<String>,
}

/// Trait abstracting action execution against the DUT (or model-only).
///
/// This lets the traversal engine work in model-only mode (no DUT) for testing,
/// and with a real WASM sandbox for production use.
pub trait ActionExecutor {
    fn execute(&mut self, action: &str, vector: Option<&TestVector>) -> ActionOutcome;
}

/// Model-only executor — no DUT calls, just returns success.
/// Used for testing the traversal engine and model-level verification.
pub struct ModelOnlyExecutor;

impl ActionExecutor for ModelOnlyExecutor {
    fn execute(&mut self, _action: &str, _vector: Option<&TestVector>) -> ActionOutcome {
        ActionOutcome {
            return_value: None,
            trapped: false,
            fuel_consumed: None,
            error: None,
        }
    }
}

/// Sandbox executor — calls into a real WASM sandbox via the verification adapter.
pub struct SandboxExecutor<'a> {
    pub instance: &'a mut beacon_sandbox::sandbox::SandboxInstance,
    pub adapter: &'a beacon_vif::adapter::VerificationAdapter,
}

impl<'a> ActionExecutor for SandboxExecutor<'a> {
    fn execute(&mut self, action: &str, vector: Option<&TestVector>) -> ActionOutcome {
        let args = vector_to_i32_args(vector);
        let result = self.adapter.execute_action(self.instance, action, &args);
        ActionOutcome {
            return_value: result.return_value,
            trapped: result.trapped,
            fuel_consumed: result.fuel_consumed,
            error: result.error,
        }
    }
}

/// Result of a single traversal pass through the graph.
#[derive(Debug)]
pub struct TraversalResult {
    pub findings: Vec<Finding>,
    pub signals: Vec<SignalEvent>,
    pub actions_executed: u64,
    pub guards_failed: u64,
    pub nodes_visited: u64,
    pub coverage: CoverageReport,
    pub trace: TraversalTrace,
}

/// Coverage information from a traversal run.
#[derive(Debug, Clone, Default)]
pub struct CoverageReport {
    /// Actions executed and their counts.
    pub action_counts: HashMap<String, u64>,
    /// Branch IDs selected and their counts.
    pub branch_counts: HashMap<String, u64>,
}

impl CoverageReport {
    pub fn unique_actions(&self) -> usize {
        self.action_counts.len()
    }

    pub fn total_actions(&self) -> u64 {
        self.action_counts.values().sum()
    }
}

/// The traversal engine — walks an NDA graph, executing actions.
///
/// Implements the object stack + strategy stack pattern from the 2008 patent.
/// The engine is "dumb pipes". The strategy is the brain.
/// Guard failures are reported to the strategy, never auto-resolved.
pub struct TraversalEngine<'a, V: VectorSource, E: ActionExecutor> {
    graph: &'a NdaGraph,
    model: &'a mut ModelState,
    executor: E,
    ir: &'a BeaconIR,
    invariants: &'a [CompiledProperty],
    actor_id: InstanceId,
    strategy_stack: &'a mut StrategyStack,
    vector_source: &'a mut V,
    weight_table: &'a mut WeightTable,
    trace: TraversalTrace,
    signals: Vec<SignalEvent>,
    findings: Vec<Finding>,
    coverage: CoverageReport,
    visited_nodes: std::collections::HashSet<NodeId>,
    step_counter: u64,
    finding_counter: u64,
    actions_executed: u64,
    guards_failed: u64,
}

impl<'a, V: VectorSource, E: ActionExecutor> TraversalEngine<'a, V, E> {
    pub fn new(
        graph: &'a NdaGraph,
        model: &'a mut ModelState,
        executor: E,
        ir: &'a BeaconIR,
        invariants: &'a [CompiledProperty],
        actor_id: InstanceId,
        strategy_stack: &'a mut StrategyStack,
        vector_source: &'a mut V,
        weight_table: &'a mut WeightTable,
    ) -> Self {
        Self {
            graph,
            model,
            executor,
            ir,
            invariants,
            actor_id,
            strategy_stack,
            vector_source,
            weight_table,
            trace: TraversalTrace::new(),
            signals: Vec::new(),
            findings: Vec::new(),
            coverage: CoverageReport::default(),
            visited_nodes: std::collections::HashSet::new(),
            step_counter: 0,
            finding_counter: 0,
            actions_executed: 0,
            guards_failed: 0,
        }
    }

    /// Run one traversal pass through the graph (entry to exit).
    ///
    /// Uses an explicit object stack (not recursion):
    /// - Pop node from stack
    /// - Terminal (call) -> execute action pipeline
    /// - Branch (alt) -> strategy picks a branch, push target
    /// - LoopEntry -> strategy picks iteration count, push body N times
    /// - Start/End -> trace only, push successors
    pub fn run_pass(mut self, max_steps: u64) -> TraversalResult {
        let mut object_stack: Vec<NodeId> = vec![self.graph.entry];

        while let Some(node_id) = object_stack.pop() {
            if self.step_counter >= max_steps {
                break;
            }

            self.visited_nodes.insert(node_id);
            let node = self.graph.nodes[node_id as usize].clone();

            match node {
                GraphNode::Start => {
                    self.trace.record(node_id, TraceStepKind::Start);
                    self.push_successors(node_id, &mut object_stack);
                }

                GraphNode::End => {
                    self.trace.record(node_id, TraceStepKind::End);
                }

                GraphNode::Terminal { action, guard } => {
                    self.step_counter += 1;

                    // Action pipeline step 1-2: Check guard against model state
                    let guard_passed = if let Some(ref guard_expr) = guard {
                        let bindings = self.make_bindings();
                        match beacon_model::eval::eval_in_model(
                            guard_expr,
                            self.model,
                            &bindings,
                        ) {
                            Ok(Value::Bool(true)) => true,
                            _ => false,
                        }
                    } else {
                        true
                    };

                    if !guard_passed {
                        self.guards_failed += 1;
                        self.trace.record(
                            node_id,
                            TraceStepKind::GuardFailed {
                                action: action.clone(),
                            },
                        );
                        self.emit_signal(SignalType::GuardFailure {
                            branch_id: String::new(),
                            action,
                        });
                        // Push successors so traversal continues past this node
                        self.push_successors(node_id, &mut object_stack);
                        continue;
                    }

                    // Step 3: Get input vector
                    let vector = self.vector_source.next_vector(&action);

                    // Step 4: Execute against DUT (or model-only)
                    let outcome = self.executor.execute(&action, vector.as_ref());

                    // Step 5: Check for traps/crashes
                    if outcome.trapped {
                        if let Some(ref err) = outcome.error {
                            if err.contains("Fuel") || err.contains("fuel") {
                                self.emit_signal(SignalType::Timeout {
                                    action: action.clone(),
                                    fuel_consumed: outcome.fuel_consumed,
                                });
                            } else {
                                self.emit_signal(SignalType::Crash {
                                    action: action.clone(),
                                    message: err.clone(),
                                });
                                self.add_finding();
                            }
                        }
                    }

                    // Step 6: Apply effects to model state
                    if let Some(effect) = self.ir.effects.get(&action) {
                        let _ = apply_effect(self.model, effect, &self.actor_id);
                    }

                    // Record in model trace
                    self.model.record_action(&action, &[]);

                    // Step 7: Check invariants
                    let violations = check_invariants(self.model, self.invariants);
                    for violation in &violations {
                        self.emit_signal(SignalType::PropertyViolation {
                            property: violation.property_name.clone(),
                            details: violation.message.clone(),
                        });
                        self.add_finding();
                    }

                    // Step 8: Coverage tracking
                    *self
                        .coverage
                        .action_counts
                        .entry(action.clone())
                        .or_insert(0) += 1;
                    self.actions_executed += 1;

                    // Step 9: Coverage delta signal on first hit
                    if self.coverage.action_counts[&action] == 1 {
                        self.emit_signal(SignalType::CoverageDelta {
                            node_id,
                            action: action.clone(),
                        });
                    }

                    self.trace.record(
                        node_id,
                        TraceStepKind::ActionExecuted {
                            action,
                            guard_passed: true,
                            return_value: outcome.return_value,
                            fuel_consumed: outcome.fuel_consumed,
                        },
                    );

                    self.push_successors(node_id, &mut object_stack);
                }

                GraphNode::Branch { alternatives } => {
                    let model_hash = self.compute_model_state_hash(&alternatives);
                    let decision = self.strategy_stack.current().select_branch(
                        &alternatives,
                        model_hash,
                        self.weight_table,
                    );

                    *self
                        .coverage
                        .branch_counts
                        .entry(decision.branch_id.clone())
                        .or_insert(0) += 1;

                    self.trace.record(
                        node_id,
                        TraceStepKind::BranchSelected {
                            branch_id: decision.branch_id.clone(),
                            weight_used: decision.weight_used,
                        },
                    );

                    // Coverage delta if branch target not visited before
                    let target_node = alternatives[decision.branch_index].target;
                    if !self.visited_nodes.contains(&target_node) {
                        self.emit_signal(SignalType::CoverageDelta {
                            node_id: target_node,
                            action: decision.branch_id,
                        });
                    }

                    object_stack.push(target_node);
                }

                GraphNode::LoopEntry {
                    body_start,
                    min,
                    max,
                } => {
                    let decision =
                        self.strategy_stack.current().choose_iterations(min, max);

                    self.trace.record(
                        node_id,
                        TraceStepKind::LoopEnter {
                            iterations_chosen: decision.iterations,
                        },
                    );

                    // Push loop exit first (processed after all iterations)
                    self.push_loop_exit_successors(node_id, &mut object_stack);

                    // Push body N times (last iteration pushed first = LIFO)
                    for _ in 0..decision.iterations {
                        object_stack.push(body_start);
                    }
                }

                GraphNode::LoopExit => {
                    self.trace.record(node_id, TraceStepKind::LoopExit);
                    self.push_successors(node_id, &mut object_stack);
                }
            }
        }

        TraversalResult {
            findings: self.findings,
            signals: self.signals,
            actions_executed: self.actions_executed,
            guards_failed: self.guards_failed,
            nodes_visited: self.visited_nodes.len() as u64,
            coverage: self.coverage,
            trace: self.trace,
        }
    }

    fn emit_signal(&mut self, signal_type: SignalType) {
        self.signals.push(SignalEvent {
            thread_id: 0,
            local_step: self.step_counter,
            signal_type,
        });
    }

    fn add_finding(&mut self) {
        let finding = Finding {
            id: self.finding_counter,
            signal: self.signals.last().unwrap().clone(),
            trace_indices: vec![self.trace.len().saturating_sub(1)],
            model_generation: self.model.generation(),
        };
        self.findings.push(finding);
        self.finding_counter += 1;
    }

    fn push_successors(&self, node_id: NodeId, stack: &mut Vec<NodeId>) {
        for &(from, to) in &self.graph.edges {
            if from == node_id {
                stack.push(to);
            }
        }
    }

    /// Push only LoopExit successors from a LoopEntry node.
    fn push_loop_exit_successors(&self, node_id: NodeId, stack: &mut Vec<NodeId>) {
        for &(from, to) in &self.graph.edges {
            if from == node_id {
                if matches!(self.graph.nodes[to as usize], GraphNode::LoopExit) {
                    stack.push(to);
                }
            }
        }
    }

    /// Build variable bindings for guard evaluation.
    fn make_bindings(&self) -> HashMap<String, InstanceId> {
        let mut bindings = HashMap::new();
        bindings.insert("actor".to_string(), self.actor_id.clone());

        // Bind "doc" and "self" to the most recently created Document instance
        let docs = self.model.all_instances("Document");
        if let Some(last_doc) = docs.last() {
            bindings.insert("doc".to_string(), last_doc.id.clone());
            bindings.insert("self".to_string(), last_doc.id.clone());
        }

        bindings
    }

    /// Compute abstract model state hash for state-conditioned weights.
    /// Uses model generation as a simplified hash.
    fn compute_model_state_hash(
        &self,
        _alternatives: &[beacon_compiler::graph::BranchEdge],
    ) -> u64 {
        self.model.generation()
    }
}

/// Convert a TestVector to i32 args for WASM function calls.
fn vector_to_i32_args(vector: Option<&TestVector>) -> Vec<i32> {
    match vector {
        Some(v) => v
            .assignments
            .values()
            .map(|dv| match dv {
                DomainValue::Bool(b) => {
                    if *b {
                        1
                    } else {
                        0
                    }
                }
                DomainValue::Int(i) => *i as i32,
                DomainValue::Enum(_) => 0,
            })
            .collect(),
        None => vec![1],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traversal::strategy::PseudoRandomStrategy;
    use crate::traversal::vector_source::MockVectorSource;
    use beacon_compiler::graph::{BranchEdge, GraphNode, NdaGraph};
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    fn minimal_ir() -> BeaconIR {
        serde_json::from_str(
            r#"{
                "entities": {},
                "refinements": {},
                "functions": {},
                "protocols": {},
                "effects": {},
                "properties": {},
                "generators": {},
                "exploration": {
                    "weights": { "scope": "test", "initial": "from_protocol", "decay": "per_epoch" },
                    "directives_allowed": [],
                    "adaptation_signals": [],
                    "strategy": { "initial": "pseudo_random_traversal", "fallback": "targeted_on_violation" },
                    "epoch_size": 100,
                    "coverage_floor_threshold": 0.05,
                    "concurrency": { "mode": "deterministic_interleaving", "threads": 1 }
                },
                "inputs": {
                    "domains": {},
                    "constraints": [],
                    "coverage": { "targets": [], "seed": 42, "reproducible": true }
                },
                "bindings": {
                    "runtime": "wasm",
                    "entry": "test.wasm",
                    "actions": {},
                    "event_hooks": { "mode": "function_intercept", "observe": [], "capture": [] }
                }
            }"#,
        )
        .unwrap()
    }

    fn make_strategy_stack() -> StrategyStack {
        let rng = ChaCha8Rng::seed_from_u64(42);
        let strategy = PseudoRandomStrategy::new(rng);
        StrategyStack::new(Box::new(strategy), 4)
    }

    fn actor_id() -> InstanceId {
        InstanceId {
            entity_type: "User".to_string(),
            index: 0,
        }
    }

    #[test]
    fn test_simple_linear_traversal() {
        // Graph: Start -> action_a -> action_b -> End
        let mut graph = NdaGraph::new();
        let a = graph.add_node(GraphNode::Terminal {
            action: "action_a".to_string(),
            guard: None,
        });
        let b = graph.add_node(GraphNode::Terminal {
            action: "action_b".to_string(),
            guard: None,
        });
        graph.add_edge(graph.entry, a);
        graph.add_edge(a, b);
        graph.add_edge(b, graph.exit);

        let mut model = ModelState::new();
        let ir = minimal_ir();
        let mut strategy_stack = make_strategy_stack();
        let mut vector_source = MockVectorSource::new();
        let mut weight_table = WeightTable::new();

        let engine = TraversalEngine::new(
            &graph,
            &mut model,
            ModelOnlyExecutor,
            &ir,
            &[],
            actor_id(),
            &mut strategy_stack,
            &mut vector_source,
            &mut weight_table,
        );

        let result = engine.run_pass(10_000);
        assert_eq!(result.actions_executed, 2);
        assert_eq!(result.coverage.unique_actions(), 2);
        assert!(result.coverage.action_counts.contains_key("action_a"));
        assert!(result.coverage.action_counts.contains_key("action_b"));
        assert!(result.findings.is_empty());
    }

    #[test]
    fn test_branch_traversal() {
        // Graph: Start -> Branch(A|B) -> join -> End
        let mut graph = NdaGraph::new();
        let term_a = graph.add_node(GraphNode::Terminal {
            action: "branch_a".to_string(),
            guard: None,
        });
        let term_b = graph.add_node(GraphNode::Terminal {
            action: "branch_b".to_string(),
            guard: None,
        });
        let join = graph.add_node(GraphNode::Start); // join placeholder
        graph.add_edge(term_a, join);
        graph.add_edge(term_b, join);
        graph.add_edge(join, graph.exit);

        let branch = graph.add_node(GraphNode::Branch {
            alternatives: vec![
                BranchEdge {
                    id: "a".to_string(),
                    weight: 50.0,
                    target: term_a,
                    guard: None,
                },
                BranchEdge {
                    id: "b".to_string(),
                    weight: 50.0,
                    target: term_b,
                    guard: None,
                },
            ],
        });
        graph.add_edge(graph.entry, branch);

        let mut model = ModelState::new();
        let ir = minimal_ir();
        let mut strategy_stack = make_strategy_stack();
        let mut vector_source = MockVectorSource::new();
        let mut weight_table = WeightTable::new();
        weight_table.set_default("a", 50.0);
        weight_table.set_default("b", 50.0);

        let engine = TraversalEngine::new(
            &graph,
            &mut model,
            ModelOnlyExecutor,
            &ir,
            &[],
            actor_id(),
            &mut strategy_stack,
            &mut vector_source,
            &mut weight_table,
        );

        let result = engine.run_pass(10_000);
        assert_eq!(result.actions_executed, 1);
        assert_eq!(result.coverage.unique_actions(), 1);
        let took_a = result.coverage.action_counts.contains_key("branch_a");
        let took_b = result.coverage.action_counts.contains_key("branch_b");
        assert!(took_a || took_b);
    }

    #[test]
    fn test_loop_traversal_fixed_count() {
        // Graph: Start -> Loop(body=action, min=3, max=3) -> End
        // No back-edge from action to loop_entry — the LoopEntry node
        // handles iteration by pushing body_start N times onto the stack.
        let mut graph = NdaGraph::new();
        let action = graph.add_node(GraphNode::Terminal {
            action: "loop_action".to_string(),
            guard: None,
        });
        let loop_exit = graph.add_node(GraphNode::LoopExit);
        let loop_entry = graph.add_node(GraphNode::LoopEntry {
            body_start: action,
            min: 3,
            max: 3,
        });
        graph.add_edge(graph.entry, loop_entry);
        graph.add_edge(loop_entry, loop_exit);
        graph.add_edge(loop_exit, graph.exit);

        let mut model = ModelState::new();
        let ir = minimal_ir();
        let mut strategy_stack = make_strategy_stack();
        let mut vector_source = MockVectorSource::new();
        let mut weight_table = WeightTable::new();

        let engine = TraversalEngine::new(
            &graph,
            &mut model,
            ModelOnlyExecutor,
            &ir,
            &[],
            actor_id(),
            &mut strategy_stack,
            &mut vector_source,
            &mut weight_table,
        );

        let result = engine.run_pass(10_000);
        assert_eq!(result.actions_executed, 3);
        assert_eq!(
            *result.coverage.action_counts.get("loop_action").unwrap(),
            3
        );
    }

    #[test]
    fn test_max_steps_limit() {
        // Loop with 100 iterations but max_steps=5
        let mut graph = NdaGraph::new();
        let action = graph.add_node(GraphNode::Terminal {
            action: "repeated".to_string(),
            guard: None,
        });
        let loop_exit = graph.add_node(GraphNode::LoopExit);
        let loop_entry = graph.add_node(GraphNode::LoopEntry {
            body_start: action,
            min: 100,
            max: 100,
        });
        graph.add_edge(graph.entry, loop_entry);
        graph.add_edge(loop_entry, loop_exit);
        graph.add_edge(loop_exit, graph.exit);

        let mut model = ModelState::new();
        let ir = minimal_ir();
        let mut strategy_stack = make_strategy_stack();
        let mut vector_source = MockVectorSource::new();
        let mut weight_table = WeightTable::new();

        let engine = TraversalEngine::new(
            &graph,
            &mut model,
            ModelOnlyExecutor,
            &ir,
            &[],
            actor_id(),
            &mut strategy_stack,
            &mut vector_source,
            &mut weight_table,
        );

        let result = engine.run_pass(5);
        assert_eq!(result.actions_executed, 5);
    }

    #[test]
    fn test_coverage_delta_signals() {
        // Two different actions should emit two CoverageDelta signals
        let mut graph = NdaGraph::new();
        let a = graph.add_node(GraphNode::Terminal {
            action: "first".to_string(),
            guard: None,
        });
        let b = graph.add_node(GraphNode::Terminal {
            action: "second".to_string(),
            guard: None,
        });
        graph.add_edge(graph.entry, a);
        graph.add_edge(a, b);
        graph.add_edge(b, graph.exit);

        let mut model = ModelState::new();
        let ir = minimal_ir();
        let mut strategy_stack = make_strategy_stack();
        let mut vector_source = MockVectorSource::new();
        let mut weight_table = WeightTable::new();

        let engine = TraversalEngine::new(
            &graph,
            &mut model,
            ModelOnlyExecutor,
            &ir,
            &[],
            actor_id(),
            &mut strategy_stack,
            &mut vector_source,
            &mut weight_table,
        );

        let result = engine.run_pass(10_000);
        let coverage_signals: Vec<_> = result
            .signals
            .iter()
            .filter(|s| matches!(s.signal_type, SignalType::CoverageDelta { .. }))
            .collect();
        assert_eq!(coverage_signals.len(), 2);
    }

    #[test]
    fn test_empty_graph_start_to_end() {
        // Just Start -> End (via default edges in NdaGraph::new)
        let mut graph = NdaGraph::new();
        graph.add_edge(graph.entry, graph.exit);

        let mut model = ModelState::new();
        let ir = minimal_ir();
        let mut strategy_stack = make_strategy_stack();
        let mut vector_source = MockVectorSource::new();
        let mut weight_table = WeightTable::new();

        let engine = TraversalEngine::new(
            &graph,
            &mut model,
            ModelOnlyExecutor,
            &ir,
            &[],
            actor_id(),
            &mut strategy_stack,
            &mut vector_source,
            &mut weight_table,
        );

        let result = engine.run_pass(10_000);
        assert_eq!(result.actions_executed, 0);
        assert!(result.findings.is_empty());
    }

    #[test]
    fn test_effects_applied_during_traversal() {
        // Create an IR with a "create_document" effect that creates a Document
        let ir: BeaconIR = serde_json::from_str(
            r#"{
                "entities": {
                    "Document": {
                        "fields": {
                            "visibility": { "type": "enum", "values": ["private", "public"] }
                        }
                    }
                },
                "refinements": {},
                "functions": {},
                "protocols": {},
                "effects": {
                    "create_document": {
                        "creates": { "entity": "Document", "assign": "doc" },
                        "sets": [
                            { "target": ["doc", "visibility"], "value": "private" }
                        ]
                    }
                },
                "properties": {},
                "generators": {},
                "exploration": {
                    "weights": { "scope": "test", "initial": "from_protocol", "decay": "per_epoch" },
                    "directives_allowed": [],
                    "adaptation_signals": [],
                    "strategy": { "initial": "pseudo_random_traversal", "fallback": "targeted_on_violation" },
                    "epoch_size": 100,
                    "coverage_floor_threshold": 0.05,
                    "concurrency": { "mode": "deterministic_interleaving", "threads": 1 }
                },
                "inputs": {
                    "domains": {},
                    "constraints": [],
                    "coverage": { "targets": [], "seed": 42, "reproducible": true }
                },
                "bindings": {
                    "runtime": "wasm",
                    "entry": "test.wasm",
                    "actions": {},
                    "event_hooks": { "mode": "function_intercept", "observe": [], "capture": [] }
                }
            }"#,
        )
        .unwrap();

        let mut graph = NdaGraph::new();
        let a = graph.add_node(GraphNode::Terminal {
            action: "create_document".to_string(),
            guard: None,
        });
        graph.add_edge(graph.entry, a);
        graph.add_edge(a, graph.exit);

        let mut model = ModelState::new();
        // Create an actor (User instance)
        let actor = model.create_instance("User");
        let mut strategy_stack = make_strategy_stack();
        let mut vector_source = MockVectorSource::new();
        let mut weight_table = WeightTable::new();

        let engine = TraversalEngine::new(
            &graph,
            &mut model,
            ModelOnlyExecutor,
            &ir,
            &[],
            actor,
            &mut strategy_stack,
            &mut vector_source,
            &mut weight_table,
        );

        let result = engine.run_pass(10_000);
        assert_eq!(result.actions_executed, 1);

        // Verify the effect was applied: a Document instance should exist
        let docs = model.all_instances("Document");
        assert_eq!(docs.len(), 1);
        assert_eq!(
            docs[0].get_field("visibility"),
            Some(&Value::String("private".to_string()))
        );
    }

    #[test]
    fn test_trace_records_all_steps() {
        let mut graph = NdaGraph::new();
        let a = graph.add_node(GraphNode::Terminal {
            action: "step1".to_string(),
            guard: None,
        });
        let b = graph.add_node(GraphNode::Terminal {
            action: "step2".to_string(),
            guard: None,
        });
        graph.add_edge(graph.entry, a);
        graph.add_edge(a, b);
        graph.add_edge(b, graph.exit);

        let mut model = ModelState::new();
        let ir = minimal_ir();
        let mut strategy_stack = make_strategy_stack();
        let mut vector_source = MockVectorSource::new();
        let mut weight_table = WeightTable::new();

        let engine = TraversalEngine::new(
            &graph,
            &mut model,
            ModelOnlyExecutor,
            &ir,
            &[],
            actor_id(),
            &mut strategy_stack,
            &mut vector_source,
            &mut weight_table,
        );

        let result = engine.run_pass(10_000);
        // Should have: Start, action_a executed, action_b executed, End
        assert!(result.trace.len() >= 4);

        // First step should be Start
        let steps = result.trace.steps();
        assert!(matches!(steps[0].kind, TraceStepKind::Start));
        // Last step should be End
        assert!(matches!(steps.last().unwrap().kind, TraceStepKind::End));
    }

    #[test]
    fn test_deterministic_traversal_same_seed() {
        // Running with the same seed should produce identical results
        let build_graph = || {
            let mut graph = NdaGraph::new();
            let term_a = graph.add_node(GraphNode::Terminal {
                action: "a".to_string(),
                guard: None,
            });
            let term_b = graph.add_node(GraphNode::Terminal {
                action: "b".to_string(),
                guard: None,
            });
            let join = graph.add_node(GraphNode::Start);
            graph.add_edge(term_a, join);
            graph.add_edge(term_b, join);
            graph.add_edge(join, graph.exit);

            let branch = graph.add_node(GraphNode::Branch {
                alternatives: vec![
                    BranchEdge {
                        id: "a".to_string(),
                        weight: 50.0,
                        target: term_a,
                        guard: None,
                    },
                    BranchEdge {
                        id: "b".to_string(),
                        weight: 50.0,
                        target: term_b,
                        guard: None,
                    },
                ],
            });
            graph.add_edge(graph.entry, branch);
            graph
        };

        let ir = minimal_ir();

        // Run 1
        let graph1 = build_graph();
        let mut model1 = ModelState::new();
        let mut ss1 = make_strategy_stack();
        let mut vs1 = MockVectorSource::new();
        let mut wt1 = WeightTable::new();
        wt1.set_default("a", 50.0);
        wt1.set_default("b", 50.0);

        let engine1 = TraversalEngine::new(
            &graph1,
            &mut model1,
            ModelOnlyExecutor,
            &ir,
            &[],
            actor_id(),
            &mut ss1,
            &mut vs1,
            &mut wt1,
        );
        let result1 = engine1.run_pass(10_000);

        // Run 2 (same seed)
        let graph2 = build_graph();
        let mut model2 = ModelState::new();
        let mut ss2 = make_strategy_stack();
        let mut vs2 = MockVectorSource::new();
        let mut wt2 = WeightTable::new();
        wt2.set_default("a", 50.0);
        wt2.set_default("b", 50.0);

        let engine2 = TraversalEngine::new(
            &graph2,
            &mut model2,
            ModelOnlyExecutor,
            &ir,
            &[],
            actor_id(),
            &mut ss2,
            &mut vs2,
            &mut wt2,
        );
        let result2 = engine2.run_pass(10_000);

        // Same seed -> same branch chosen
        assert_eq!(result1.coverage.action_counts, result2.coverage.action_counts);
    }

    /// Custom executor that simulates crashes for testing.
    struct CrashingExecutor {
        crash_on: String,
    }

    impl ActionExecutor for CrashingExecutor {
        fn execute(&mut self, action: &str, _vector: Option<&TestVector>) -> ActionOutcome {
            if action == self.crash_on {
                ActionOutcome {
                    return_value: None,
                    trapped: true,
                    fuel_consumed: None,
                    error: Some("WASM trap: unreachable".to_string()),
                }
            } else {
                ActionOutcome {
                    return_value: None,
                    trapped: false,
                    fuel_consumed: None,
                    error: None,
                }
            }
        }
    }

    #[test]
    fn test_crash_produces_finding() {
        let mut graph = NdaGraph::new();
        let a = graph.add_node(GraphNode::Terminal {
            action: "safe_action".to_string(),
            guard: None,
        });
        let b = graph.add_node(GraphNode::Terminal {
            action: "crashing_action".to_string(),
            guard: None,
        });
        graph.add_edge(graph.entry, a);
        graph.add_edge(a, b);
        graph.add_edge(b, graph.exit);

        let mut model = ModelState::new();
        let ir = minimal_ir();
        let mut strategy_stack = make_strategy_stack();
        let mut vector_source = MockVectorSource::new();
        let mut weight_table = WeightTable::new();

        let executor = CrashingExecutor {
            crash_on: "crashing_action".to_string(),
        };

        let engine = TraversalEngine::new(
            &graph,
            &mut model,
            executor,
            &ir,
            &[],
            actor_id(),
            &mut strategy_stack,
            &mut vector_source,
            &mut weight_table,
        );

        let result = engine.run_pass(10_000);
        assert_eq!(result.actions_executed, 2);
        assert_eq!(result.findings.len(), 1);
        assert!(matches!(
            result.findings[0].signal.signal_type,
            SignalType::Crash { .. }
        ));
    }

    /// Custom executor that simulates timeouts for testing.
    struct TimeoutExecutor {
        timeout_on: String,
    }

    impl ActionExecutor for TimeoutExecutor {
        fn execute(&mut self, action: &str, _vector: Option<&TestVector>) -> ActionOutcome {
            if action == self.timeout_on {
                ActionOutcome {
                    return_value: None,
                    trapped: true,
                    fuel_consumed: Some(1_000_000),
                    error: Some("Fuel exhausted".to_string()),
                }
            } else {
                ActionOutcome {
                    return_value: None,
                    trapped: false,
                    fuel_consumed: None,
                    error: None,
                }
            }
        }
    }

    #[test]
    fn test_timeout_emits_signal_not_finding() {
        let mut graph = NdaGraph::new();
        let a = graph.add_node(GraphNode::Terminal {
            action: "slow_action".to_string(),
            guard: None,
        });
        graph.add_edge(graph.entry, a);
        graph.add_edge(a, graph.exit);

        let mut model = ModelState::new();
        let ir = minimal_ir();
        let mut strategy_stack = make_strategy_stack();
        let mut vector_source = MockVectorSource::new();
        let mut weight_table = WeightTable::new();

        let executor = TimeoutExecutor {
            timeout_on: "slow_action".to_string(),
        };

        let engine = TraversalEngine::new(
            &graph,
            &mut model,
            executor,
            &ir,
            &[],
            actor_id(),
            &mut strategy_stack,
            &mut vector_source,
            &mut weight_table,
        );

        let result = engine.run_pass(10_000);
        // Timeout emits a signal but NOT a finding (per the two-step timeout policy)
        let timeout_signals: Vec<_> = result
            .signals
            .iter()
            .filter(|s| matches!(s.signal_type, SignalType::Timeout { .. }))
            .collect();
        assert_eq!(timeout_signals.len(), 1);
        // No crash findings for timeouts
        let crash_findings: Vec<_> = result
            .findings
            .iter()
            .filter(|f| matches!(f.signal.signal_type, SignalType::Crash { .. }))
            .collect();
        assert!(crash_findings.is_empty());
    }
}
