use beacon_compiler::predicate::TypeContext;
use beacon_ir::parse::parse_ir;
use beacon_model::simulate::{simulate, SimulationConfig};
use beacon_model::state::{ModelState, Value};

fn setup() -> (beacon_ir::types::BeaconIR, TypeContext) {
    let json = include_str!("../../beacon-ir/tests/fixtures/document_lifecycle.json");
    let ir = parse_ir(json).unwrap();
    let ctx = TypeContext::from_ir(&ir);
    (ir, ctx)
}

#[test]
fn test_simulate_single_step() {
    let (ir, ctx) = setup();
    let mut state = ModelState::new();

    // Pre-create an actor
    let actor_id = state.create_instance("User");
    state.set_field(&actor_id, "id", Value::String("u1".to_string()));
    state.set_field(&actor_id, "role", Value::String("admin".to_string()));
    state.set_field(&actor_id, "authenticated", Value::Bool(true));

    let config = SimulationConfig {
        max_steps: 1,
        seed: 42,
        protocol_name: "document_lifecycle".to_string(),
    };

    let result = simulate(&ir, &ctx, &mut state, &config);
    assert!(
        result.is_ok(),
        "Simulation failed: {:?}",
        result.unwrap_err()
    );
    let report = result.unwrap();

    // Should have executed at least one action
    assert!(report.steps_executed > 0);
    assert!(!report.actions_executed.is_empty());
}

#[test]
fn test_simulate_multiple_steps() {
    let (ir, ctx) = setup();
    let mut state = ModelState::new();

    let actor_id = state.create_instance("User");
    state.set_field(&actor_id, "id", Value::String("u1".to_string()));
    state.set_field(&actor_id, "role", Value::String("admin".to_string()));
    state.set_field(&actor_id, "authenticated", Value::Bool(true));

    let config = SimulationConfig {
        max_steps: 10,
        seed: 42,
        protocol_name: "document_lifecycle".to_string(),
    };

    let result = simulate(&ir, &ctx, &mut state, &config).unwrap();

    // Should have executed multiple actions
    assert!(result.steps_executed > 1);
    // Should have created a document (first action is always create_document)
    assert!(result
        .actions_executed
        .contains(&"create_document".to_string()));
}

#[test]
fn test_simulate_produces_coverage_report() {
    let (ir, ctx) = setup();
    let mut state = ModelState::new();

    let actor_id = state.create_instance("User");
    state.set_field(&actor_id, "id", Value::String("u1".to_string()));
    state.set_field(&actor_id, "role", Value::String("admin".to_string()));
    state.set_field(&actor_id, "authenticated", Value::Bool(true));

    let config = SimulationConfig {
        max_steps: 20,
        seed: 123,
        protocol_name: "document_lifecycle".to_string(),
    };

    let result = simulate(&ir, &ctx, &mut state, &config).unwrap();

    // Coverage should track unique actions hit
    assert!(!result.unique_actions.is_empty());
    assert!(result.unique_actions.contains("create_document"));
}

#[test]
fn test_simulate_detects_no_violations_on_valid_spec() {
    let (ir, ctx) = setup();
    let mut state = ModelState::new();

    let actor_id = state.create_instance("User");
    state.set_field(&actor_id, "id", Value::String("u1".to_string()));
    state.set_field(&actor_id, "role", Value::String("admin".to_string()));
    state.set_field(&actor_id, "authenticated", Value::Bool(true));

    let config = SimulationConfig {
        max_steps: 10,
        seed: 42,
        protocol_name: "document_lifecycle".to_string(),
    };

    let result = simulate(&ir, &ctx, &mut state, &config).unwrap();
    // With a valid spec and single authenticated admin, no contradictions expected
    assert!(
        result.violations.is_empty(),
        "Unexpected violations: {:?}",
        result.violations
    );
}

#[test]
fn test_simulate_idle_protocol() {
    let (ir, ctx) = setup();
    let mut state = ModelState::new();

    let actor_id = state.create_instance("User");
    state.set_field(&actor_id, "id", Value::String("u1".to_string()));
    state.set_field(&actor_id, "role", Value::String("member".to_string()));
    state.set_field(&actor_id, "authenticated", Value::Bool(true));

    // Also need a document for read to target
    let doc_id = state.create_instance("Document");
    state.set_field(&doc_id, "id", Value::String("d1".to_string()));
    state.set_field(&doc_id, "visibility", Value::String("public".to_string()));

    let config = SimulationConfig {
        max_steps: 5,
        seed: 42,
        protocol_name: "idle".to_string(),
    };

    let result = simulate(&ir, &ctx, &mut state, &config).unwrap();
    assert!(result.steps_executed >= 1);
    assert!(result.actions_executed.contains(&"read".to_string()));
}
