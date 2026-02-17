use std::collections::HashMap;

use fresnel_fir_ir::types::{ActionBinding, Bindings};
use fresnel_fir_sandbox::sandbox::{SandboxError, SandboxInstance, WasmVal};

/// The result of executing a single action against the DUT.
#[derive(Debug)]
pub struct ActionResult {
    /// The action name that was executed.
    pub action: String,
    /// The WASM function that was called.
    pub function: String,
    /// The arguments passed (as i32 values).
    pub args: Vec<i32>,
    /// The return value (None for void functions).
    pub return_value: Option<i32>,
    /// Whether the call trapped/panicked.
    pub trapped: bool,
    /// Fuel consumed during execution (if metering enabled).
    pub fuel_consumed: Option<u64>,
    /// Error message if the call failed.
    pub error: Option<String>,
}

/// Observer result — explicitly tagged to never be confused with model truth.
#[derive(Debug)]
pub struct ObserverResult {
    /// The observer function name.
    pub observer: String,
    /// The WASM function called.
    pub function: String,
    /// The observed return value.
    pub value: Option<i32>,
    /// Whether the observation failed.
    pub error: Option<String>,
}

/// The verification adapter — the ONLY interface between FresnelFir and the DUT.
///
/// Auto-generated from IR bindings. Bridges abstract action names to concrete
/// WASM function calls. Handles argument serialization and return deserialization.
pub struct VerificationAdapter {
    /// Maps action name -> binding definition.
    action_bindings: HashMap<String, ActionBinding>,
    /// Maps observer function name -> binding info (for observer calls).
    observer_bindings: HashMap<String, ObserverBinding>,
}

#[derive(Debug, Clone)]
struct ObserverBinding {
    function: String,
}

impl VerificationAdapter {
    /// Create a new adapter from IR bindings.
    pub fn from_bindings(bindings: &Bindings) -> Self {
        let action_bindings = bindings.actions.clone();
        let observer_bindings = HashMap::new(); // Observers added via register_observer

        Self {
            action_bindings,
            observer_bindings,
        }
    }

    /// Register an observer binding (from IR functions section).
    pub fn register_observer(&mut self, name: &str, function: &str, _params: &[String]) {
        self.observer_bindings.insert(
            name.to_string(),
            ObserverBinding {
                function: function.to_string(),
            },
        );
    }

    /// Execute a single action against the DUT.
    ///
    /// This is the forward direction: FresnelFir -> DUT.
    /// Serializes arguments, calls the WASM export, deserializes the return value.
    pub fn execute_action(
        &self,
        instance: &mut SandboxInstance,
        action: &str,
        args: &[i32],
    ) -> ActionResult {
        let binding = match self.action_bindings.get(action) {
            Some(b) => b,
            None => {
                return ActionResult {
                    action: action.to_string(),
                    function: String::new(),
                    args: args.to_vec(),
                    return_value: None,
                    trapped: false,
                    fuel_consumed: None,
                    error: Some(format!("No binding for action '{action}'")),
                };
            }
        };

        let func_name = &binding.function;

        // Serialize args as i32 WasmVals
        let wasm_args: Vec<WasmVal> = args.iter().map(|&a| WasmVal::from(a)).collect();

        // call_func resets fuel before executing, so measure AFTER the call
        // by checking remaining fuel (call_func sets it to fuel_per_action, then
        // the WASM consumes some)
        match instance.call_func(func_name, &wasm_args) {
            Ok(results) => {
                let fuel_after = instance.remaining_fuel();
                // fuel_per_action was set at start of call_func, remaining is what's left
                let fuel_consumed = fuel_after.map(|after| {
                    // The sandbox resets to fuel_per_action before each call
                    // So consumed = fuel_per_action - remaining
                    let budget = instance.fuel_budget().unwrap_or(0);
                    budget.saturating_sub(after)
                });

                let return_value = results.first().and_then(|v| v.i32());

                ActionResult {
                    action: action.to_string(),
                    function: func_name.clone(),
                    args: args.to_vec(),
                    return_value,
                    trapped: false,
                    fuel_consumed,
                    error: None,
                }
            }
            Err(SandboxError::FuelExhausted) => ActionResult {
                action: action.to_string(),
                function: func_name.clone(),
                args: args.to_vec(),
                return_value: None,
                trapped: true,
                fuel_consumed: instance.fuel_budget(),
                error: Some("Fuel exhausted".to_string()),
            },
            Err(e) => ActionResult {
                action: action.to_string(),
                function: func_name.clone(),
                args: args.to_vec(),
                return_value: None,
                trapped: true,
                fuel_consumed: None,
                error: Some(e.to_string()),
            },
        }
    }

    /// Query an observer in the DUT.
    ///
    /// Observer results are explicitly tagged as ObserverResult — they represent
    /// what the DUT *claims* about its state, never model truth.
    pub fn query_observer(
        &self,
        instance: &mut SandboxInstance,
        observer: &str,
        args: &[i32],
    ) -> ObserverResult {
        let binding = match self.observer_bindings.get(observer) {
            Some(b) => b,
            None => {
                return ObserverResult {
                    observer: observer.to_string(),
                    function: String::new(),
                    value: None,
                    error: Some(format!("No observer binding for '{observer}'")),
                };
            }
        };

        let func_name = &binding.function;
        let wasm_args: Vec<WasmVal> = args.iter().map(|&a| WasmVal::from(a)).collect();

        match instance.call_func(func_name, &wasm_args) {
            Ok(results) => {
                let value = results.first().and_then(|v| v.i32());
                ObserverResult {
                    observer: observer.to_string(),
                    function: func_name.clone(),
                    value,
                    error: None,
                }
            }
            Err(e) => ObserverResult {
                observer: observer.to_string(),
                function: func_name.clone(),
                value: None,
                error: Some(e.to_string()),
            },
        }
    }

    /// Check if a binding exists for the given action.
    pub fn has_action(&self, action: &str) -> bool {
        self.action_bindings.contains_key(action)
    }

    /// Check if a binding exists for the given observer.
    pub fn has_observer(&self, observer: &str) -> bool {
        self.observer_bindings.contains_key(observer)
    }

    /// Get the WASM function name for an action.
    pub fn function_for_action(&self, action: &str) -> Option<&str> {
        self.action_bindings
            .get(action)
            .map(|b| b.function.as_str())
    }

    /// Get all registered action names.
    pub fn action_names(&self) -> Vec<&str> {
        self.action_bindings.keys().map(|s| s.as_str()).collect()
    }

    /// Get the ActionBinding for a given action.
    pub fn get_binding(&self, action: &str) -> Option<&ActionBinding> {
        self.action_bindings.get(action)
    }
}
