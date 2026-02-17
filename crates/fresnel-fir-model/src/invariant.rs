use fresnel_fir_compiler::predicate::CompiledExpr;

use crate::eval::eval_in_model;
use crate::state::ModelState;

/// A compiled property ready for checking.
#[derive(Debug, Clone)]
pub struct CompiledProperty {
    pub name: String,
    pub expr: CompiledExpr,
}

/// A violation found during invariant checking.
#[derive(Debug, Clone)]
pub struct Violation {
    pub property_name: String,
    pub message: String,
}

/// Check all invariant properties against the current model state.
///
/// Returns a list of violations (empty if all invariants hold).
pub fn check_invariants(state: &ModelState, properties: &[CompiledProperty]) -> Vec<Violation> {
    let mut violations = Vec::new();
    let bindings = Default::default();

    for prop in properties {
        match eval_in_model(&prop.expr, state, &bindings) {
            Ok(crate::state::Value::Bool(true)) => {
                // Invariant holds
            }
            Ok(crate::state::Value::Bool(false)) => {
                violations.push(Violation {
                    property_name: prop.name.clone(),
                    message: format!("Invariant '{}' violated", prop.name),
                });
            }
            Ok(other) => {
                violations.push(Violation {
                    property_name: prop.name.clone(),
                    message: format!(
                        "Invariant '{}' evaluated to non-boolean: {:?}",
                        prop.name, other
                    ),
                });
            }
            Err(e) => {
                violations.push(Violation {
                    property_name: prop.name.clone(),
                    message: format!("Invariant '{}' evaluation error: {}", prop.name, e),
                });
            }
        }
    }

    violations
}
