use std::collections::HashMap;

use fresnel_fir_compiler::predicate::CompiledExpr;
use fresnel_fir_ir::expr::{OpKind, QuantifierKind};

use crate::state::{InstanceId, ModelState, Value};

#[derive(Debug, thiserror::Error)]
pub enum ModelEvalError {
    #[error("Field not found: {entity}.{field}")]
    FieldNotFound { entity: String, field: String },

    #[error("Unbound variable: '{var}'")]
    UnboundVariable { var: String },

    #[error("Type error: expected {expected}, got {actual}")]
    TypeError { expected: String, actual: String },

    #[error("Cannot evaluate: {reason}")]
    Unsupported { reason: String },
}

/// Variable bindings mapping variable names to entity instance IDs.
pub type Bindings = HashMap<String, InstanceId>;

/// Evaluate a compiled expression against model state.
///
/// `bindings` maps variable names (like "self", "actor", "u", "d") to
/// concrete entity instance IDs in the model.
pub fn eval_in_model(
    expr: &CompiledExpr,
    state: &ModelState,
    bindings: &Bindings,
) -> Result<Value, ModelEvalError> {
    match expr {
        CompiledExpr::Literal(v) => Ok(compiler_value_to_model_value(v)),

        CompiledExpr::Field { entity, field } => {
            let instance_id =
                bindings
                    .get(entity)
                    .ok_or_else(|| ModelEvalError::UnboundVariable {
                        var: entity.clone(),
                    })?;
            let instance =
                state
                    .get_instance(instance_id)
                    .ok_or_else(|| ModelEvalError::FieldNotFound {
                        entity: entity.clone(),
                        field: field.clone(),
                    })?;
            instance
                .get_field(field)
                .cloned()
                .ok_or_else(|| ModelEvalError::FieldNotFound {
                    entity: entity.clone(),
                    field: field.clone(),
                })
        }

        CompiledExpr::Op { op, args } => eval_op(op, args, state, bindings),

        CompiledExpr::Quantifier {
            kind,
            var,
            domain,
            body,
        } => {
            let instances = state.all_instances(domain);
            match kind {
                QuantifierKind::Forall => {
                    for inst in instances {
                        let mut new_bindings = bindings.clone();
                        new_bindings.insert(var.clone(), inst.id.clone());
                        let result = eval_in_model(body, state, &new_bindings)?;
                        if result == Value::Bool(false) {
                            return Ok(Value::Bool(false));
                        }
                    }
                    Ok(Value::Bool(true))
                }
                QuantifierKind::Exists => {
                    for inst in instances {
                        let mut new_bindings = bindings.clone();
                        new_bindings.insert(var.clone(), inst.id.clone());
                        let result = eval_in_model(body, state, &new_bindings)?;
                        if result == Value::Bool(true) {
                            return Ok(Value::Bool(true));
                        }
                    }
                    Ok(Value::Bool(false))
                }
            }
        }

        CompiledExpr::FnCall { name, .. } => {
            // For now, derived function calls require the function body to be
            // inlined at compile time. Return unsupported for runtime calls.
            Err(ModelEvalError::Unsupported {
                reason: format!(
                    "Direct function call evaluation for '{name}' not yet implemented; \
                     function bodies should be inlined during compilation"
                ),
            })
        }

        CompiledExpr::Is { .. } => Err(ModelEvalError::Unsupported {
            reason: "Is expression requires refinement predicate resolution".to_string(),
        }),
    }
}

fn eval_op(
    op: &OpKind,
    args: &[CompiledExpr],
    state: &ModelState,
    bindings: &Bindings,
) -> Result<Value, ModelEvalError> {
    match op {
        OpKind::Eq => {
            let left = eval_in_model(&args[0], state, bindings)?;
            let right = eval_in_model(&args[1], state, bindings)?;
            Ok(Value::Bool(left == right))
        }
        OpKind::Neq => {
            let left = eval_in_model(&args[0], state, bindings)?;
            let right = eval_in_model(&args[1], state, bindings)?;
            Ok(Value::Bool(left != right))
        }
        OpKind::And => {
            for arg in args {
                let val = eval_in_model(arg, state, bindings)?;
                if val == Value::Bool(false) {
                    return Ok(Value::Bool(false));
                }
            }
            Ok(Value::Bool(true))
        }
        OpKind::Or => {
            for arg in args {
                let val = eval_in_model(arg, state, bindings)?;
                if val == Value::Bool(true) {
                    return Ok(Value::Bool(true));
                }
            }
            Ok(Value::Bool(false))
        }
        OpKind::Not => {
            let val = eval_in_model(&args[0], state, bindings)?;
            match val {
                Value::Bool(b) => Ok(Value::Bool(!b)),
                other => Err(ModelEvalError::TypeError {
                    expected: "bool".to_string(),
                    actual: format!("{other:?}"),
                }),
            }
        }
        OpKind::Implies => {
            let antecedent = eval_in_model(&args[0], state, bindings)?;
            match antecedent {
                Value::Bool(false) => Ok(Value::Bool(true)),
                Value::Bool(true) => eval_in_model(&args[1], state, bindings),
                other => Err(ModelEvalError::TypeError {
                    expected: "bool".to_string(),
                    actual: format!("{other:?}"),
                }),
            }
        }
        OpKind::Lt => eval_int_cmp(args, state, bindings, |a, b| a < b),
        OpKind::Lte => eval_int_cmp(args, state, bindings, |a, b| a <= b),
        OpKind::Gt => eval_int_cmp(args, state, bindings, |a, b| a > b),
        OpKind::Gte => eval_int_cmp(args, state, bindings, |a, b| a >= b),
    }
}

fn eval_int_cmp(
    args: &[CompiledExpr],
    state: &ModelState,
    bindings: &Bindings,
    cmp: fn(i64, i64) -> bool,
) -> Result<Value, ModelEvalError> {
    let left = eval_in_model(&args[0], state, bindings)?;
    let right = eval_in_model(&args[1], state, bindings)?;
    match (&left, &right) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(cmp(*a, *b))),
        _ => Err(ModelEvalError::TypeError {
            expected: "int".to_string(),
            actual: format!("{left:?}, {right:?}"),
        }),
    }
}

/// Convert from fresnel-fir-compiler's Value to fresnel-fir-model's Value.
fn compiler_value_to_model_value(v: &fresnel_fir_compiler::predicate::Value) -> Value {
    match v {
        fresnel_fir_compiler::predicate::Value::Bool(b) => Value::Bool(*b),
        fresnel_fir_compiler::predicate::Value::Int(i) => Value::Int(*i),
        fresnel_fir_compiler::predicate::Value::String(s) => Value::String(s.clone()),
    }
}
