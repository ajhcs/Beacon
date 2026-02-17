use std::collections::HashSet;

use fresnel_fir_compiler::predicate::TypeContext;
use fresnel_fir_ir::types::{FresnelFirIR, ProtocolNode};

use crate::effect::apply_effect;
use crate::state::{InstanceId, ModelState};

#[derive(Debug)]
pub struct SimulationConfig {
    pub max_steps: usize,
    pub seed: u64,
    pub protocol_name: String,
}

#[derive(Debug)]
pub struct SimulationResult {
    pub steps_executed: usize,
    pub actions_executed: Vec<String>,
    pub unique_actions: HashSet<String>,
    pub violations: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum SimulationError {
    #[error("Protocol '{name}' not found")]
    ProtocolNotFound { name: String },

    #[error("Effect error: {0}")]
    Effect(#[from] crate::effect::EffectError),

    #[error("No actor available in model state")]
    NoActor,
}

/// Simulate a protocol against model state without a DUT.
///
/// Walks the protocol grammar, applying effects at each Call node,
/// making random choices at Alt branches and Repeat counts.
pub fn simulate(
    ir: &FresnelFirIR,
    _ctx: &TypeContext,
    state: &mut ModelState,
    config: &SimulationConfig,
) -> Result<SimulationResult, SimulationError> {
    let protocol = ir.protocols.get(&config.protocol_name).ok_or_else(|| {
        SimulationError::ProtocolNotFound {
            name: config.protocol_name.clone(),
        }
    })?;

    let mut result = SimulationResult {
        steps_executed: 0,
        actions_executed: Vec::new(),
        unique_actions: HashSet::new(),
        violations: Vec::new(),
    };

    // Simple PRNG (xorshift64)
    let mut rng_state = config.seed;

    // Find the actor (first User instance, or error)
    let actor_id = state
        .all_instances("User")
        .first()
        .map(|inst| inst.id.clone())
        .ok_or(SimulationError::NoActor)?;

    walk_node(
        &protocol.root,
        ir,
        state,
        &actor_id,
        &mut result,
        &mut rng_state,
        config.max_steps,
    )?;

    Ok(result)
}

fn walk_node(
    node: &ProtocolNode,
    ir: &FresnelFirIR,
    state: &mut ModelState,
    actor_id: &InstanceId,
    result: &mut SimulationResult,
    rng: &mut u64,
    max_steps: usize,
) -> Result<(), SimulationError> {
    if result.steps_executed >= max_steps {
        return Ok(());
    }

    match node {
        ProtocolNode::Call { action } => {
            // Apply the effect for this action
            if let Some(effect) = ir.effects.get(action) {
                apply_effect(state, effect, actor_id)?;
            }
            // Record in trace so temporal properties can observe the action sequence.
            state.record_action(action, &[]);
            result.steps_executed += 1;
            result.actions_executed.push(action.clone());
            result.unique_actions.insert(action.clone());
            Ok(())
        }

        ProtocolNode::Seq { children } => {
            for child in children {
                if result.steps_executed >= max_steps {
                    break;
                }
                walk_node(child, ir, state, actor_id, result, rng, max_steps)?;
            }
            Ok(())
        }

        ProtocolNode::Alt { branches } => {
            if branches.is_empty() {
                return Ok(());
            }

            // Weighted random selection
            let total_weight: u32 = branches.iter().map(|b| b.weight).sum();
            if total_weight == 0 {
                return Ok(());
            }

            let roll = xorshift64(rng) % (total_weight as u64);
            let mut cumulative = 0u64;
            let mut selected = &branches[0];
            for branch in branches {
                cumulative += branch.weight as u64;
                if roll < cumulative {
                    selected = branch;
                    break;
                }
            }

            walk_node(&selected.body, ir, state, actor_id, result, rng, max_steps)
        }

        ProtocolNode::Repeat { min, max, body } => {
            // Choose a random repeat count in [min, max]
            let range = (*max as u64).saturating_sub(*min as u64) + 1;
            let count = if range > 0 {
                *min as u64 + (xorshift64(rng) % range)
            } else {
                *min as u64
            };

            for _ in 0..count {
                if result.steps_executed >= max_steps {
                    break;
                }
                walk_node(body, ir, state, actor_id, result, rng, max_steps)?;
            }
            Ok(())
        }

        ProtocolNode::Ref { protocol } => {
            if let Some(proto) = ir.protocols.get(protocol) {
                walk_node(&proto.root, ir, state, actor_id, result, rng, max_steps)
            } else {
                Ok(()) // Already validated, shouldn't happen
            }
        }
    }
}

/// Simple xorshift64 PRNG for deterministic simulation.
fn xorshift64(state: &mut u64) -> u64 {
    // Ensure state is never zero
    if *state == 0 {
        *state = 1;
    }
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}
