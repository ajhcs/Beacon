use std::collections::HashSet;

use crate::state::TraceEntry;

/// A temporal rule to check against an action trace.
#[derive(Debug, Clone)]
pub enum TemporalRule {
    /// "Before any mutating action, a required condition must hold."
    /// Checks that every mutating action in the trace has the required arg=value.
    BeforeMutation {
        name: String,
        mutating_actions: Vec<String>,
        required_arg: String,
        required_value: String,
    },

    /// "After trigger_action on an entity, forbidden_action must never occur on the same entity."
    AfterNever {
        name: String,
        trigger_action: String,
        forbidden_action: String,
        same_entity_key: String,
    },
}

/// A temporal violation.
#[derive(Debug, Clone)]
pub struct TemporalViolation {
    pub rule_name: String,
    pub message: String,
    pub trace_index: usize,
}

/// Check temporal properties against an action trace.
pub fn check_temporal(trace: &[TraceEntry], rules: &[TemporalRule]) -> Vec<TemporalViolation> {
    let mut violations = Vec::new();

    for rule in rules {
        match rule {
            TemporalRule::BeforeMutation {
                name,
                mutating_actions,
                required_arg,
                required_value,
            } => {
                check_before_mutation(
                    trace,
                    name,
                    mutating_actions,
                    required_arg,
                    required_value,
                    &mut violations,
                );
            }
            TemporalRule::AfterNever {
                name,
                trigger_action,
                forbidden_action,
                same_entity_key,
            } => {
                check_after_never(
                    trace,
                    name,
                    trigger_action,
                    forbidden_action,
                    same_entity_key,
                    &mut violations,
                );
            }
        }
    }

    violations
}

fn check_before_mutation(
    trace: &[TraceEntry],
    rule_name: &str,
    mutating_actions: &[String],
    required_arg: &str,
    required_value: &str,
    violations: &mut Vec<TemporalViolation>,
) {
    for (i, entry) in trace.iter().enumerate() {
        if mutating_actions.contains(&entry.action) {
            let has_required = entry
                .args
                .iter()
                .any(|(k, v)| k == required_arg && v == required_value);
            if !has_required {
                violations.push(TemporalViolation {
                    rule_name: rule_name.to_string(),
                    message: format!(
                        "Action '{}' at trace index {} requires {}={} but condition not met",
                        entry.action, i, required_arg, required_value,
                    ),
                    trace_index: i,
                });
            }
        }
    }
}

fn check_after_never(
    trace: &[TraceEntry],
    rule_name: &str,
    trigger_action: &str,
    forbidden_action: &str,
    same_entity_key: &str,
    violations: &mut Vec<TemporalViolation>,
) {
    // Track which entities have been triggered (e.g., deleted)
    let mut triggered_entities: HashSet<String> = HashSet::new();

    for (i, entry) in trace.iter().enumerate() {
        // Extract entity ID from args
        let entity_id = entry
            .args
            .iter()
            .find(|(k, _)| k == same_entity_key)
            .map(|(_, v)| v.clone());

        if entry.action == trigger_action {
            if let Some(eid) = &entity_id {
                triggered_entities.insert(eid.clone());
            }
        }

        if entry.action == forbidden_action {
            if let Some(eid) = &entity_id {
                if triggered_entities.contains(eid) {
                    violations.push(TemporalViolation {
                        rule_name: rule_name.to_string(),
                        message: format!(
                            "Action '{}' on entity '{}' at trace index {} is forbidden after '{}'",
                            forbidden_action, eid, i, trigger_action,
                        ),
                        trace_index: i,
                    });
                }
            }
        }
    }
}
