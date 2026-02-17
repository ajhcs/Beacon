use std::collections::HashSet;

use beacon_ir::types::{BeaconIR, ProtocolNode};

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Dangling entity reference: refinement '{refinement}' references entity '{entity}' which doesn't exist")]
    DanglingEntityRef { refinement: String, entity: String },

    #[error("Missing effect: action '{action}' used in protocol but no effect defined")]
    MissingEffect { action: String },

    #[error("Missing binding: action '{action}' used in protocol but no binding defined")]
    MissingBinding { action: String },

    #[error("Dangling protocol ref: '{from}' references protocol '{target}' which doesn't exist")]
    DanglingProtocolRef { from: String, target: String },

    #[error("All zero weights in alt block at '{location}'")]
    AllZeroWeights { location: String },

    #[error("Invalid repeat bounds at '{location}': min ({min}) > max ({max})")]
    InvalidRepeatBounds {
        location: String,
        min: u32,
        max: u32,
    },
}

pub fn validate_ir(ir: &BeaconIR) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();
    validate_entity_refs(ir, &mut errors);
    validate_protocol_actions(ir, &mut errors);
    validate_protocol_refs(ir, &mut errors);
    validate_protocol_structure(ir, &mut errors);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Check that all refinement base types reference declared entities.
fn validate_entity_refs(ir: &BeaconIR, errors: &mut Vec<ValidationError>) {
    for (name, refinement) in &ir.refinements {
        if !ir.entities.contains_key(&refinement.base) {
            errors.push(ValidationError::DanglingEntityRef {
                refinement: name.clone(),
                entity: refinement.base.clone(),
            });
        }
    }
}

/// Check that all actions referenced in protocols have matching effects and bindings.
fn validate_protocol_actions(ir: &BeaconIR, errors: &mut Vec<ValidationError>) {
    let mut actions = HashSet::new();
    for protocol in ir.protocols.values() {
        collect_actions(&protocol.root, &mut actions);
    }
    for action in &actions {
        if !ir.effects.contains_key(action.as_str()) {
            errors.push(ValidationError::MissingEffect {
                action: action.clone(),
            });
        }
        if !ir.bindings.actions.contains_key(action.as_str()) {
            errors.push(ValidationError::MissingBinding {
                action: action.clone(),
            });
        }
    }
}

/// Check that all protocol refs target existing protocols.
fn validate_protocol_refs(ir: &BeaconIR, errors: &mut Vec<ValidationError>) {
    for (proto_name, protocol) in &ir.protocols {
        collect_refs(&protocol.root, proto_name, &ir.protocols, errors);
    }
}

/// Check structural constraints: alt weights, repeat bounds.
fn validate_protocol_structure(ir: &BeaconIR, errors: &mut Vec<ValidationError>) {
    for (proto_name, protocol) in &ir.protocols {
        check_structure(&protocol.root, proto_name, errors);
    }
}

/// Recursively collect all action names from Call nodes.
fn collect_actions(node: &ProtocolNode, actions: &mut HashSet<String>) {
    match node {
        ProtocolNode::Call { action } => {
            actions.insert(action.clone());
        }
        ProtocolNode::Seq { children } => {
            for child in children {
                collect_actions(child, actions);
            }
        }
        ProtocolNode::Alt { branches } => {
            for branch in branches {
                collect_actions(&branch.body, actions);
            }
        }
        ProtocolNode::Repeat { body, .. } => {
            collect_actions(body, actions);
        }
        ProtocolNode::Ref { .. } => {}
    }
}

/// Recursively check for dangling protocol refs.
fn collect_refs(
    node: &ProtocolNode,
    from: &str,
    protocols: &std::collections::HashMap<String, beacon_ir::types::Protocol>,
    errors: &mut Vec<ValidationError>,
) {
    match node {
        ProtocolNode::Ref { protocol } => {
            if !protocols.contains_key(protocol) {
                errors.push(ValidationError::DanglingProtocolRef {
                    from: from.to_string(),
                    target: protocol.clone(),
                });
            }
        }
        ProtocolNode::Seq { children } => {
            for child in children {
                collect_refs(child, from, protocols, errors);
            }
        }
        ProtocolNode::Alt { branches } => {
            for branch in branches {
                collect_refs(&branch.body, from, protocols, errors);
            }
        }
        ProtocolNode::Repeat { body, .. } => {
            collect_refs(body, from, protocols, errors);
        }
        ProtocolNode::Call { .. } => {}
    }
}

/// Recursively check structural constraints in protocol nodes.
fn check_structure(node: &ProtocolNode, proto_name: &str, errors: &mut Vec<ValidationError>) {
    match node {
        ProtocolNode::Alt { branches } => {
            if !branches.is_empty() && branches.iter().all(|b| b.weight == 0) {
                errors.push(ValidationError::AllZeroWeights {
                    location: proto_name.to_string(),
                });
            }
            for branch in branches {
                check_structure(&branch.body, proto_name, errors);
            }
        }
        ProtocolNode::Repeat { min, max, body } => {
            if min > max {
                errors.push(ValidationError::InvalidRepeatBounds {
                    location: proto_name.to_string(),
                    min: *min,
                    max: *max,
                });
            }
            check_structure(body, proto_name, errors);
        }
        ProtocolNode::Seq { children } => {
            for child in children {
                check_structure(child, proto_name, errors);
            }
        }
        ProtocolNode::Call { .. } | ProtocolNode::Ref { .. } => {}
    }
}
