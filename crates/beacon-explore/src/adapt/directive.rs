//! Adaptation directives — the vocabulary of exploration policy changes.
//!
//! Directives are the output of the adaptation coordinator. Each directive
//! represents a single atomic change to exploration policy (weights, strategy,
//! or action handling). Directives never modify the spec/constraints — only
//! how the engine explores the search space.

use crate::traversal::signal::SignalType;

/// A directive modifying exploration policy.
#[derive(Debug, Clone, PartialEq)]
pub enum Directive {
    /// Adjust weight on an alt branch (state-conditioned).
    AdjustWeight {
        branch_id: String,
        model_state_hash: u64,
        multiplier: f64,
    },
    /// Force a specific action to be taken next (highest priority).
    Force {
        action: String,
        /// Number of times to force before reverting.
        budget: u32,
    },
    /// Skip a branch for a bounded number of iterations (state-conditioned).
    Skip {
        branch_id: String,
        model_state_hash: u64,
        remaining: u32,
    },
    /// Adjust repeat bounds within declared min/max.
    LoopLimit {
        loop_node_id: u32,
        new_min: u32,
        new_max: u32,
    },
    /// Set weight to permanent zero with proof artifact.
    PermanentZero {
        branch_id: String,
        proof: UnreachabilityProof,
    },
}

/// Proof that a branch is provably unreachable.
#[derive(Debug, Clone, PartialEq)]
pub enum UnreachabilityProof {
    /// No path from entry to this branch in the graph.
    StaticUnreachable { path_description: String },
    /// Solver returned UNSAT for the guard predicate.
    SolverUnsat { constraint_description: String },
}

/// A directive with metadata for logging and replay.
#[derive(Debug, Clone)]
pub struct DirectiveEntry {
    /// The directive itself.
    pub directive: Directive,
    /// The signal that triggered this directive.
    pub triggered_by: SignalType,
    /// Epoch in which this directive was applied.
    pub epoch: u64,
    /// Monotonic sequence number (total order).
    pub seqno: u64,
}

/// Log of all directives applied during a campaign.
#[derive(Debug, Clone, Default)]
pub struct DirectiveLog {
    entries: Vec<DirectiveEntry>,
    next_seqno: u64,
}

impl DirectiveLog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, directive: Directive, triggered_by: SignalType, epoch: u64) {
        self.entries.push(DirectiveEntry {
            directive,
            triggered_by,
            epoch,
            seqno: self.next_seqno,
        });
        self.next_seqno += 1;
    }

    pub fn entries(&self) -> &[DirectiveEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_directive_log_records_in_order() {
        let mut log = DirectiveLog::new();

        log.record(
            Directive::AdjustWeight {
                branch_id: "b1".into(),
                model_state_hash: 42,
                multiplier: 1.5,
            },
            SignalType::CoverageDelta {
                node_id: 1,
                action: "a".into(),
            },
            0,
        );

        log.record(
            Directive::Force {
                action: "test".into(),
                budget: 5,
            },
            SignalType::PropertyViolation {
                property: "p1".into(),
                details: "violated".into(),
            },
            0,
        );

        assert_eq!(log.len(), 2);
        assert_eq!(log.entries()[0].seqno, 0);
        assert_eq!(log.entries()[1].seqno, 1);
        assert_eq!(log.entries()[0].epoch, 0);
    }

    #[test]
    fn test_directive_log_empty() {
        let log = DirectiveLog::new();
        assert!(log.is_empty());
        assert_eq!(log.len(), 0);
    }

    #[test]
    fn test_permanent_zero_with_proof() {
        let d = Directive::PermanentZero {
            branch_id: "dead_branch".into(),
            proof: UnreachabilityProof::SolverUnsat {
                constraint_description: "guard requires x > 10 but x domain is [0, 5]".into(),
            },
        };

        if let Directive::PermanentZero { proof, .. } = &d {
            assert!(matches!(proof, UnreachabilityProof::SolverUnsat { .. }));
        }
    }
}
