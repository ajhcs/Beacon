use fresnel_fir_compiler::graph::{BranchEdge, GraphNode, NdaGraph};
use fresnel_fir_explore::traversal::engine::{ModelOnlyExecutor, TraversalEngine};
use fresnel_fir_explore::traversal::runner::{run_campaign, CampaignConfig};
use fresnel_fir_explore::traversal::signal::SignalType;
use fresnel_fir_explore::traversal::strategy::{PseudoRandomStrategy, StrategyStack};
use fresnel_fir_explore::traversal::trace::TraceStepKind;
use fresnel_fir_explore::traversal::vector_source::MockVectorSource;
use fresnel_fir_explore::traversal::weight_table::WeightTable;
use fresnel_fir_ir::types::FresnelFirIR;
use fresnel_fir_model::state::{InstanceId, ModelState, Value};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

fn minimal_ir() -> FresnelFirIR {
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

/// Build a simple linear graph: Start -> action_a -> End
fn build_linear_graph() -> NdaGraph {
    let mut g = NdaGraph::new();
    let terminal = g.add_node(GraphNode::Terminal {
        action: "create_document".to_string(),
        guard: None,
    });
    g.add_edge(g.entry, terminal);
    g.add_edge(terminal, g.exit);
    g
}

/// Build a branching graph: Start -> Alt(create, read) -> End
fn build_branching_graph() -> NdaGraph {
    let mut g = NdaGraph::new();
    let create = g.add_node(GraphNode::Terminal {
        action: "create_document".to_string(),
        guard: None,
    });
    let read = g.add_node(GraphNode::Terminal {
        action: "read".to_string(),
        guard: None,
    });
    let branch = g.add_node(GraphNode::Branch {
        alternatives: vec![
            BranchEdge {
                id: "create_path".to_string(),
                weight: 60.0,
                target: create,
                guard: None,
            },
            BranchEdge {
                id: "read_path".to_string(),
                weight: 40.0,
                target: read,
                guard: None,
            },
        ],
    });
    g.add_edge(g.entry, branch);
    g.add_edge(create, g.exit);
    g.add_edge(read, g.exit);
    g
}

/// Build a graph with a loop: Start -> Loop(create, 1..3) -> End
fn build_loop_graph() -> NdaGraph {
    let mut g = NdaGraph::new();
    let body = g.add_node(GraphNode::Terminal {
        action: "create_document".to_string(),
        guard: None,
    });
    let loop_exit = g.add_node(GraphNode::LoopExit);
    let loop_entry = g.add_node(GraphNode::LoopEntry {
        body_start: body,
        min: 1,
        max: 3,
    });
    g.add_edge(g.entry, loop_entry);
    g.add_edge(loop_entry, loop_exit);
    g.add_edge(loop_exit, g.exit);
    g
}

/// Build a sequence graph: Start -> create -> read -> delete -> End
fn build_sequence_graph() -> NdaGraph {
    let mut g = NdaGraph::new();
    let create = g.add_node(GraphNode::Terminal {
        action: "create_document".to_string(),
        guard: None,
    });
    let read = g.add_node(GraphNode::Terminal {
        action: "read".to_string(),
        guard: None,
    });
    let delete = g.add_node(GraphNode::Terminal {
        action: "delete".to_string(),
        guard: None,
    });
    g.add_edge(g.entry, create);
    g.add_edge(create, read);
    g.add_edge(read, delete);
    g.add_edge(delete, g.exit);
    g
}

#[test]
fn test_linear_traversal_executes_action() {
    let graph = build_linear_graph();
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

    assert_eq!(result.actions_executed, 1);
    assert_eq!(result.guards_failed, 0);
    assert!(result.findings.is_empty());
    assert!(result.nodes_visited > 0);

    // Should have Start, ActionExecuted, End
    let has_action = result.trace.steps().iter().any(|s| {
        matches!(&s.kind, TraceStepKind::ActionExecuted { action, .. } if action == "create_document")
    });
    assert!(has_action, "trace should contain create_document action");
}

#[test]
fn test_branching_traversal_selects_branch() {
    let graph = build_branching_graph();
    let mut model = ModelState::new();
    let ir = minimal_ir();
    let mut strategy_stack = make_strategy_stack();
    let mut vector_source = MockVectorSource::new();
    let mut weight_table = WeightTable::new();
    weight_table.set_default("create_path", 60.0);
    weight_table.set_default("read_path", 40.0);

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

    // Exactly one action should be executed (either create or read)
    assert_eq!(result.actions_executed, 1);

    // Trace should show branch selection
    let branch_step = result
        .trace
        .steps()
        .iter()
        .find(|s| matches!(&s.kind, TraceStepKind::BranchSelected { .. }));
    assert!(
        branch_step.is_some(),
        "trace should contain branch selection"
    );
}

#[test]
fn test_loop_traversal_iterates() {
    let graph = build_loop_graph();
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

    // Should execute 1-3 actions (loop iterations)
    assert!(
        result.actions_executed >= 1 && result.actions_executed <= 3,
        "expected 1-3 actions, got {}",
        result.actions_executed
    );

    // Trace should show loop entry
    let has_loop = result
        .trace
        .steps()
        .iter()
        .any(|s| matches!(&s.kind, TraceStepKind::LoopEnter { .. }));
    assert!(has_loop, "trace should contain loop entry");
}

#[test]
fn test_sequence_traversal_executes_all_actions() {
    let graph = build_sequence_graph();
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

    // Should execute 3 actions in sequence
    assert_eq!(result.actions_executed, 3);

    // Trace should have all 3 actions
    let actions: Vec<String> = result
        .trace
        .steps()
        .iter()
        .filter_map(|s| match &s.kind {
            TraceStepKind::ActionExecuted { action, .. } => Some(action.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(actions.len(), 3);
}

#[test]
fn test_model_records_actions() {
    let graph = build_sequence_graph();
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

    engine.run_pass(10_000);

    // Model should have recorded actions in its trace
    let trace = model.trace();
    assert_eq!(trace.len(), 3);
    assert_eq!(trace[0].action, "create_document");
    assert_eq!(trace[1].action, "read");
    assert_eq!(trace[2].action, "delete");
}

#[test]
fn test_coverage_delta_signals() {
    let graph = build_branching_graph();
    let mut model = ModelState::new();
    let ir = minimal_ir();
    let mut strategy_stack = make_strategy_stack();
    let mut vector_source = MockVectorSource::new();
    let mut weight_table = WeightTable::new();
    weight_table.set_default("create_path", 60.0);
    weight_table.set_default("read_path", 40.0);

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

    // First traversal should emit at least one CoverageDelta
    let coverage_signals: Vec<_> = result
        .signals
        .iter()
        .filter(|s| matches!(&s.signal_type, SignalType::CoverageDelta { .. }))
        .collect();
    assert!(
        !coverage_signals.is_empty(),
        "should emit coverage delta on first visit"
    );
}

#[test]
fn test_campaign_runner_multi_pass() {
    let graph = build_linear_graph();
    let mut model = ModelState::new();
    let ir = minimal_ir();
    let mut vector_source = MockVectorSource::new();
    let mut executor = ModelOnlyExecutor;

    let campaign_config = CampaignConfig {
        max_passes: 5,
        seed: 42,
        strategy_depth_limit: 4,
        max_steps_per_pass: 10_000,
    };

    let result = run_campaign(
        &graph,
        &mut model,
        &mut executor,
        &ir,
        &[],
        actor_id(),
        &mut vector_source,
        &campaign_config,
    );

    assert_eq!(result.passes_completed, 5);
    assert_eq!(result.total_actions, 5); // 1 action per pass * 5 passes
    assert!(result.findings.is_empty());
}

#[test]
fn test_weight_table_state_conditioned() {
    let mut wt = WeightTable::new();
    wt.set_default("branch_a", 60.0);
    wt.set_default("branch_b", 40.0);

    // Default weights
    assert_eq!(wt.get("branch_a", 0), 60.0);
    assert_eq!(wt.get("branch_b", 0), 40.0);

    // State-conditioned override
    wt.set("branch_a", 123, 10.0);
    assert_eq!(wt.get("branch_a", 123), 10.0);
    assert_eq!(wt.get("branch_a", 0), 60.0); // different state still uses default

    // Adjust
    wt.adjust("branch_b", 0, 0.5);
    assert_eq!(wt.get("branch_b", 0), 20.0); // 40 * 0.5

    // Normalize
    wt.normalize(&["branch_a", "branch_b"], 0);
    let a = wt.get("branch_a", 0);
    let b = wt.get("branch_b", 0);
    assert!((a + b - 100.0).abs() < 0.01, "should normalize to 100");
}

#[test]
fn test_strategy_stack_depth_limit() {
    let rng = ChaCha8Rng::seed_from_u64(42);
    let mut stack = StrategyStack::new(Box::new(PseudoRandomStrategy::new(rng)), 3);

    assert_eq!(stack.depth(), 1);

    // Push 2 more
    let rng2 = ChaCha8Rng::seed_from_u64(43);
    stack.push(Box::new(PseudoRandomStrategy::new(rng2)));
    assert_eq!(stack.depth(), 2);

    let rng3 = ChaCha8Rng::seed_from_u64(44);
    stack.push(Box::new(PseudoRandomStrategy::new(rng3)));
    assert_eq!(stack.depth(), 3);

    // Push one more — should evict oldest non-base
    let rng4 = ChaCha8Rng::seed_from_u64(45);
    stack.push(Box::new(PseudoRandomStrategy::new(rng4)));
    assert_eq!(stack.depth(), 3); // still 3, oldest was evicted

    // Pop should work
    assert!(stack.pop().is_some());
    assert_eq!(stack.depth(), 2);

    // Pop again
    assert!(stack.pop().is_some());
    assert_eq!(stack.depth(), 1);

    // Can't pop base
    assert!(stack.pop().is_none());
    assert_eq!(stack.depth(), 1);
}

#[test]
fn test_deterministic_traversal() {
    let ir = minimal_ir();
    let graph = build_branching_graph();

    let run = |seed: u64| -> Vec<String> {
        let mut model = ModelState::new();
        let rng = ChaCha8Rng::seed_from_u64(seed);
        let mut strategy_stack = StrategyStack::new(Box::new(PseudoRandomStrategy::new(rng)), 4);
        let mut vector_source = MockVectorSource::new();
        let mut weight_table = WeightTable::new();
        weight_table.set_default("create_path", 60.0);
        weight_table.set_default("read_path", 40.0);

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

        result
            .trace
            .steps()
            .iter()
            .filter_map(|s| match &s.kind {
                TraceStepKind::ActionExecuted { action, .. } => Some(action.clone()),
                TraceStepKind::BranchSelected { branch_id, .. } => {
                    Some(format!("branch:{}", branch_id))
                }
                _ => None,
            })
            .collect()
    };

    let run1 = run(42);
    let run2 = run(42);
    assert_eq!(run1, run2, "same seed should produce identical traces");
}

#[test]
fn test_effects_applied_during_traversal() {
    // Create an IR with a "create_document" effect that creates a Document
    let ir: FresnelFirIR = serde_json::from_str(
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

// ── Integration test: Full document_lifecycle protocol traversal ────────

/// Load the full document_lifecycle fixture, compile it, and run a model-only
/// campaign traversal through the compiled NDA graph.
///
/// This is the comprehensive integration test: real IR -> real compiler ->
/// real NDA graph -> real traversal engine -> real effects -> real coverage.
#[test]
fn test_document_lifecycle_model_only_campaign() {
    // 1. Load the full document_lifecycle IR fixture
    let ir_json = include_str!("../../fresnel-fir-ir/tests/fixtures/document_lifecycle.json");
    let ir: FresnelFirIR = serde_json::from_str(ir_json).expect("fixture should parse");

    // 2. Compile the IR into NDA graphs
    let compiled = fresnel_fir_compiler::compile(&ir).expect("fixture should compile");
    let graph = compiled
        .graphs
        .get("document_lifecycle")
        .expect("document_lifecycle graph should exist");

    // 3. Set up model state with an authenticated User actor
    let mut model = ModelState::new();
    let actor = model.create_instance("User");
    model.set_field(&actor, "authenticated", Value::Bool(true));
    model.set_field(&actor, "role", Value::String("admin".to_string()));
    model.set_field(&actor, "id", Value::String("user-1".to_string()));

    // 4. Run a model-only campaign
    let mut vector_source = MockVectorSource::new();
    let config = CampaignConfig {
        max_passes: 50,
        max_steps_per_pass: 200,
        seed: 42,
        strategy_depth_limit: 4,
    };

    let mut executor = ModelOnlyExecutor;
    let result = run_campaign(
        graph,
        &mut model,
        &mut executor,
        &ir,
        &[], // No compiled invariants for now (they use quantifiers which need model iteration)
        actor,
        &mut vector_source,
        &config,
    );

    // 5. Verify the campaign completed
    assert_eq!(result.passes_completed, 50);
    assert!(
        result.total_actions > 0,
        "should have executed actions, got 0"
    );

    // 6. Verify coverage — the create_document action must have been hit
    // (it's the first action in the seq, always executed)
    let model_trace = model.trace();
    let create_count = model_trace
        .iter()
        .filter(|t| t.action == "create_document")
        .count();
    assert!(
        create_count >= 50,
        "each pass should execute create_document at least once, got {}",
        create_count
    );

    // 7. Verify that the loop body was entered and various branches taken.
    // With 50 passes the traversal should have explored multiple action types.
    let unique_actions: std::collections::HashSet<&str> =
        model_trace.iter().map(|t| t.action.as_str()).collect();
    assert!(
        unique_actions.len() >= 2,
        "should have explored at least 2 unique actions across 50 passes, got {:?}",
        unique_actions
    );

    // 8. Verify that Document instances were created by effects
    let docs = model.all_instances("Document");
    assert!(
        docs.len() >= 50,
        "each pass creates a document, expected >= 50, got {}",
        docs.len()
    );

    // 9. Verify determinism — same seed, same result
    let mut model2 = ModelState::new();
    let actor2 = model2.create_instance("User");
    model2.set_field(&actor2, "authenticated", Value::Bool(true));
    model2.set_field(&actor2, "role", Value::String("admin".to_string()));
    model2.set_field(&actor2, "id", Value::String("user-1".to_string()));
    let mut vs2 = MockVectorSource::new();
    let mut executor2 = ModelOnlyExecutor;

    let result2 = run_campaign(
        graph,
        &mut model2,
        &mut executor2,
        &ir,
        &[],
        actor2,
        &mut vs2,
        &config,
    );

    assert_eq!(
        result.total_actions, result2.total_actions,
        "same seed should produce same action count"
    );
}
