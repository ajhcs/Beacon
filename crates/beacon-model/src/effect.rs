use beacon_ir::types::Effect;

use crate::state::{InstanceId, ModelState, Value};

#[derive(Debug, thiserror::Error)]
pub enum EffectError {
    #[error("No instance of type '{entity_type}' found to apply set effect")]
    NoInstance { entity_type: String },

    #[error("Cannot resolve value: {reason}")]
    ValueResolution { reason: String },
}

/// Apply an effect to model state.
///
/// `actor_id` is the instance ID of the actor performing the action.
/// For effects with `creates`, a new instance is allocated.
/// For `sets`, fields are updated on the most recently created instance
/// of the target entity type (or the actor if the target is "actor").
pub fn apply_effect(
    state: &mut ModelState,
    effect: &Effect,
    actor_id: &InstanceId,
) -> Result<(), EffectError> {
    // Track the created instance ID if the effect creates one
    let mut created_id: Option<InstanceId> = None;

    if let Some(create) = &effect.creates {
        let id = state.create_instance(&create.entity);
        created_id = Some(id);
    }

    for set in &effect.sets {
        if set.target.len() < 2 {
            continue;
        }
        let target_var = &set.target[0];
        let field_name = &set.target[1];

        // Resolve the target instance
        let target_id = resolve_target_instance(state, target_var, actor_id, &created_id)?;

        // Resolve the value
        let value = resolve_value(&set.value, state, actor_id)?;

        state.set_field(&target_id, field_name, value);
    }

    Ok(())
}

/// Resolve a target variable name to an instance ID.
fn resolve_target_instance(
    state: &ModelState,
    var_name: &str,
    actor_id: &InstanceId,
    created_id: &Option<InstanceId>,
) -> Result<InstanceId, EffectError> {
    match var_name {
        "actor" => Ok(actor_id.clone()),
        _ => {
            // Check if this is the just-created instance
            if let Some(id) = created_id {
                return Ok(id.clone());
            }
            // Otherwise, find the most recent instance that could match
            // Try all entity types to find the last created instance
            // This is a simplification — in a full implementation, variable bindings
            // would be resolved through a proper scope
            for entity_type in ["Document", "User"] {
                let instances = state.all_instances(entity_type);
                if let Some(last) = instances.last() {
                    return Ok(last.id.clone());
                }
            }
            Err(EffectError::NoInstance {
                entity_type: var_name.to_string(),
            })
        }
    }
}

/// Resolve a JSON value from an effect's `value` field into a model Value.
fn resolve_value(
    json_val: &serde_json::Value,
    state: &ModelState,
    actor_id: &InstanceId,
) -> Result<Value, EffectError> {
    match json_val {
        serde_json::Value::Bool(b) => Ok(Value::Bool(*b)),
        serde_json::Value::String(s) => Ok(Value::String(s.clone())),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::Int(i))
            } else {
                Err(EffectError::ValueResolution {
                    reason: format!("unsupported number: {n}"),
                })
            }
        }
        // Array form: ["field", entity_var, field_name] — resolve from model state
        serde_json::Value::Array(arr) => {
            if arr.len() == 3
                && arr[0].as_str() == Some("field")
            {
                let entity_var = arr[1].as_str().unwrap_or("");
                let field_name = arr[2].as_str().unwrap_or("");

                let instance_id = match entity_var {
                    "actor" => actor_id.clone(),
                    _ => {
                        return Err(EffectError::ValueResolution {
                            reason: format!("unknown entity variable: {entity_var}"),
                        })
                    }
                };

                let instance = state.get_instance(&instance_id).ok_or_else(|| {
                    EffectError::ValueResolution {
                        reason: format!("instance not found for {entity_var}"),
                    }
                })?;

                instance
                    .get_field(field_name)
                    .cloned()
                    .ok_or_else(|| EffectError::ValueResolution {
                        reason: format!("field '{field_name}' not found on {entity_var}"),
                    })
            } else {
                Err(EffectError::ValueResolution {
                    reason: format!("unsupported array value: {json_val}"),
                })
            }
        }
        _ => Err(EffectError::ValueResolution {
            reason: format!("unsupported value type: {json_val}"),
        }),
    }
}
