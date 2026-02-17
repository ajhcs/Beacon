use std::collections::HashMap;

use fresnel_fir_ir::types::Bindings;

/// Errors discovered during interface validation.
#[derive(Debug, thiserror::Error)]
pub enum InterfaceError {
    #[error(
        "Missing export: action '{action}' expects WASM export '{function}', but it was not found"
    )]
    MissingExport { action: String, function: String },

    #[error("Wrong kind: action '{action}' expects a function export '{function}', but found '{found_kind}'")]
    WrongExportKind {
        action: String,
        function: String,
        found_kind: String,
    },

    #[error("Signature mismatch: action '{action}' function '{function}' expects {expected_params} params, found {found_params}")]
    ParamCountMismatch {
        action: String,
        function: String,
        expected_params: usize,
        found_params: usize,
    },

    #[error("Return mismatch: action '{action}' function '{function}' expects {expected_returns} return values, found {found_returns}")]
    ReturnCountMismatch {
        action: String,
        function: String,
        expected_returns: usize,
        found_returns: usize,
    },
}

/// Validate that a WASM module's exports match the IR bindings.
///
/// `exports` is a list of (name, kind) pairs from the loaded module.
/// `bindings` is the IR bindings section.
///
/// Returns Ok(()) if all bindings are satisfied, or a list of interface errors.
pub fn validate_interface(
    exports: &[(String, String)],
    bindings: &Bindings,
) -> Result<(), Vec<InterfaceError>> {
    let mut errors = Vec::new();

    // Build a lookup map of exports
    let export_map: HashMap<&str, &str> = exports
        .iter()
        .map(|(name, kind)| (name.as_str(), kind.as_str()))
        .collect();

    for (action_name, binding) in &bindings.actions {
        let func_name = &binding.function;

        match export_map.get(func_name.as_str()) {
            None => {
                errors.push(InterfaceError::MissingExport {
                    action: action_name.clone(),
                    function: func_name.clone(),
                });
            }
            Some(&kind) if kind != "func" => {
                errors.push(InterfaceError::WrongExportKind {
                    action: action_name.clone(),
                    function: func_name.clone(),
                    found_kind: kind.to_string(),
                });
            }
            Some(_) => {
                // Export exists and is a function — basic validation passes.
                // Full signature validation requires the wasmtime FuncType,
                // which is checked separately in validate_signatures().
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Result of a detailed signature check.
#[derive(Debug)]
pub struct SignatureReport {
    pub action: String,
    pub function: String,
    pub expected_params: usize,
    pub actual_params: usize,
    pub expected_returns: usize,
    pub actual_returns: usize,
    pub matches: bool,
}

/// Validate WASM function signatures against IR bindings.
///
/// `func_signatures` maps function name to (param_count, result_count).
/// This is extracted from the WASM module at load time.
pub fn validate_signatures(
    func_signatures: &HashMap<String, (usize, usize)>,
    bindings: &Bindings,
) -> Result<Vec<SignatureReport>, Vec<InterfaceError>> {
    let mut errors = Vec::new();
    let mut reports = Vec::new();

    for (action_name, binding) in &bindings.actions {
        let func_name = &binding.function;
        let expected_params = binding.args.len();
        let expected_returns = if is_void_return(&binding.returns) {
            0
        } else {
            1
        };

        if let Some(&(actual_params, actual_returns)) = func_signatures.get(func_name.as_str()) {
            let matches = actual_params == expected_params && actual_returns == expected_returns;

            if actual_params != expected_params {
                errors.push(InterfaceError::ParamCountMismatch {
                    action: action_name.clone(),
                    function: func_name.clone(),
                    expected_params,
                    found_params: actual_params,
                });
            }
            if actual_returns != expected_returns {
                errors.push(InterfaceError::ReturnCountMismatch {
                    action: action_name.clone(),
                    function: func_name.clone(),
                    expected_returns,
                    found_returns: actual_returns,
                });
            }

            reports.push(SignatureReport {
                action: action_name.clone(),
                function: func_name.clone(),
                expected_params,
                actual_params,
                expected_returns,
                actual_returns,
                matches,
            });
        }
    }

    if errors.is_empty() {
        Ok(reports)
    } else {
        Err(errors)
    }
}

/// Check if a return type descriptor indicates void.
fn is_void_return(returns: &serde_json::Value) -> bool {
    match returns {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::String(t)) = map.get("type") {
                t == "void"
            } else {
                false
            }
        }
        serde_json::Value::String(s) => s == "void",
        serde_json::Value::Null => true,
        _ => false,
    }
}
