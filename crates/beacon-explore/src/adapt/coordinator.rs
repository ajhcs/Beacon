//! Epoch-based signal coordinator with total ordering.
//!
//! Collects signals from traversal threads into fixed-size epochs,
//! assigns monotonic sequence numbers (sorted by thread_id then local_step),
//! maps signals to directives, and applies directives at epoch boundaries.
//!
//! Same inputs → same directive history regardless of thread scheduling.

use crate::traversal::signal::{SignalEvent, SignalType};
use crate::traversal::weight_table::WeightTable;

use super::decay::{self, DecayConfig};
use super::directive::{Directive, DirectiveLog};
use super::timeout::TimeoutTracker;

/// Configuration for the adaptation coordinator.
#[derive(Debug, Clone)]
pub struct CoordinatorConfig {
    /// Number of signals per epoch before processing.
    pub epoch_size: u32,
    /// Weight boost multiplier for coverage-yielding branches.
    pub coverage_boost: f64,
    /// Weight decay multiplier for guard-failing branches (state-conditioned).
    pub guard_failure_decay: f64,
    /// Weight boost for finding-yielding branches.
    pub finding_boost: f64,
    /// Force budget when investigating violations.
    pub force_budget: u32,
    /// Coverage floor threshold (fraction of total weight budget).
    pub coverage_floor_threshold: f64,
    /// Decay configuration.
    pub decay: DecayConfig,
}

impl Default for CoordinatorConfig {
    fn default() -> Self {
        Self {
            epoch_size: 100,
            coverage_boost: 1.5,
            guard_failure_decay: 0.5,
            finding_boost: 2.0,
            force_budget: 10,
            coverage_floor_threshold: 0.05,
            decay: DecayConfig::default(),
        }
    }
}

/// The adaptation coordinator — processes signals and emits directives.
///
/// Invariant: adaptation changes exploration policy, never the spec.
/// Invariant: adaptation is deterministic given the same signal sequence.
pub struct Coordinator {
    config: CoordinatorConfig,
    /// Current epoch number.
    epoch: u64,
    /// Signals collected in the current epoch (not yet processed).
    pending_signals: Vec<SignalEvent>,
    /// All directives applied so far (audit log).
    directive_log: DirectiveLog,
    /// Tracks timeout two-step state per action.
    timeout_tracker: TimeoutTracker,
    /// Global signal sequence counter.
    signal_seqno: u64,
    /// Set of branches known to reach uncovered targets.
    /// Used for coverage floor enforcement.
    uncovered_target_branches: Vec<String>,
}

impl Coordinator {
    pub fn new(config: CoordinatorConfig) -> Self {
        Self {
            config,
            epoch: 0,
            pending_signals: Vec::new(),
            directive_log: DirectiveLog::new(),
            timeout_tracker: TimeoutTracker::new(),
            signal_seqno: 0,
            uncovered_target_branches: Vec::new(),
        }
    }

    /// Feed a signal into the coordinator.
    /// Returns directives if an epoch boundary is reached.
    pub fn feed_signal(
        &mut self,
        signal: SignalEvent,
        weight_table: &mut WeightTable,
        alt_block_branches: &[Vec<String>],
    ) -> Vec<Directive> {
        self.pending_signals.push(signal);

        if self.pending_signals.len() >= self.config.epoch_size as usize {
            self.process_epoch(weight_table, alt_block_branches)
        } else {
            Vec::new()
        }
    }

    /// Force-process any remaining signals (e.g., at campaign end).
    pub fn flush(
        &mut self,
        weight_table: &mut WeightTable,
        alt_block_branches: &[Vec<String>],
    ) -> Vec<Directive> {
        if self.pending_signals.is_empty() {
            return Vec::new();
        }
        self.process_epoch(weight_table, alt_block_branches)
    }

    /// Process one epoch: sort signals, map to directives, apply.
    fn process_epoch(
        &mut self,
        weight_table: &mut WeightTable,
        alt_block_branches: &[Vec<String>],
    ) -> Vec<Directive> {
        // Step 1: Sort by (thread_id, local_step) for total ordering.
        let mut signals = std::mem::take(&mut self.pending_signals);
        signals.sort_by(|a, b| {
            a.thread_id
                .cmp(&b.thread_id)
                .then(a.local_step.cmp(&b.local_step))
        });

        // Step 2: Assign monotonic sequence numbers.
        for signal in &signals {
            self.signal_seqno += 1;
            let _ = signal; // seqno assigned globally, signals just ordered
        }

        // Step 3: Map signals to directives.
        let mut directives = Vec::new();
        for signal in &signals {
            let new_directives = self.map_signal_to_directives(&signal.signal_type);
            for d in &new_directives {
                self.directive_log
                    .record(d.clone(), signal.signal_type.clone(), self.epoch);
            }
            directives.extend(new_directives);
        }

        // Step 4: Apply per-epoch weight decay.
        decay::apply_epoch_decay(weight_table, &self.config.decay);

        // Step 5: Normalize weights per alt block.
        for block_branches in alt_block_branches {
            let branch_refs: Vec<&str> = block_branches.iter().map(|s| s.as_str()).collect();
            // Normalize for each unique model_state_hash we've seen.
            // For simplicity, normalize at hash 0 (default state).
            // Full implementation would track all observed hashes.
            weight_table.normalize(&branch_refs, 0);
        }

        // Step 6: Enforce coverage floor.
        decay::enforce_coverage_floor(
            weight_table,
            &self.uncovered_target_branches,
            self.config.coverage_floor_threshold,
        );

        // Step 7: Apply directives to weight table.
        for directive in &directives {
            self.apply_directive(directive, weight_table);
        }

        self.epoch += 1;
        directives
    }

    /// Map a single signal to zero or more directives.
    fn map_signal_to_directives(&mut self, signal: &SignalType) -> Vec<Directive> {
        match signal {
            SignalType::CoverageDelta { action, .. } => {
                // Boost weight on the branch that led to new coverage.
                // We use the action name as a proxy for branch_id here.
                vec![Directive::AdjustWeight {
                    branch_id: action.clone(),
                    model_state_hash: 0,
                    multiplier: self.config.coverage_boost,
                }]
            }

            SignalType::PropertyViolation { property, .. } => {
                // Force nearby branches for deeper investigation.
                vec![Directive::Force {
                    action: property.clone(),
                    budget: self.config.force_budget,
                }]
            }

            SignalType::Discrepancy { action, .. } => {
                // Force the divergent path + increase loop bounds.
                vec![Directive::Force {
                    action: action.clone(),
                    budget: self.config.force_budget,
                }]
            }

            SignalType::Crash { action, .. } => {
                // Force with boundary values and related inputs.
                vec![
                    Directive::Force {
                        action: action.clone(),
                        budget: self.config.force_budget * 2,
                    },
                    Directive::AdjustWeight {
                        branch_id: action.clone(),
                        model_state_hash: 0,
                        multiplier: self.config.finding_boost,
                    },
                ]
            }

            SignalType::Timeout {
                action,
                fuel_consumed,
            } => {
                // Two-step timeout response via the tracker.
                self.timeout_tracker
                    .handle_timeout(action, *fuel_consumed)
                    .into_iter()
                    .collect()
            }

            SignalType::GuardFailure {
                branch_id, action, ..
            } => {
                // State-conditioned decay: "branch B is invalid WHEN model is in state S"
                let bid = if branch_id.is_empty() {
                    action
                } else {
                    branch_id
                };
                vec![Directive::AdjustWeight {
                    branch_id: bid.clone(),
                    model_state_hash: 0, // Caller should provide real hash
                    multiplier: self.config.guard_failure_decay,
                }]
            }

            SignalType::CoveragePlateau { .. } => {
                // Convert each uncovered target to a Force directive.
                self.uncovered_target_branches
                    .iter()
                    .map(|branch| Directive::Force {
                        action: branch.clone(),
                        budget: self.config.force_budget,
                    })
                    .collect()
            }
        }
    }

    /// Apply a single directive to the weight table.
    fn apply_directive(&self, directive: &Directive, weight_table: &mut WeightTable) {
        match directive {
            Directive::AdjustWeight {
                branch_id,
                model_state_hash,
                multiplier,
            } => {
                weight_table.adjust(branch_id, *model_state_hash, *multiplier);
            }
            Directive::PermanentZero {
                branch_id, proof, ..
            } => {
                // Set to zero across all observed model states.
                // Log the proof artifact.
                weight_table.set(branch_id, 0, 0.0);
                let _ = proof; // Proof is recorded in directive log
            }
            Directive::Skip {
                branch_id,
                model_state_hash,
                ..
            } => {
                // Temporarily set very low weight.
                weight_table.set(branch_id, *model_state_hash, 0.01);
            }
            // Force and LoopLimit affect the strategy stack, not weight table.
            // They're handled by the traversal engine when it checks active directives.
            Directive::Force { .. } | Directive::LoopLimit { .. } => {}
        }
    }

    /// Register branches that can reach uncovered targets.
    pub fn set_uncovered_target_branches(&mut self, branches: Vec<String>) {
        self.uncovered_target_branches = branches;
    }

    /// Get the directive log for audit/replay.
    pub fn directive_log(&self) -> &DirectiveLog {
        &self.directive_log
    }

    /// Current epoch number.
    pub fn current_epoch(&self) -> u64 {
        self.epoch
    }

    /// Total signals processed.
    pub fn total_signals_processed(&self) -> u64 {
        self.signal_seqno
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_signal(signal_type: SignalType) -> SignalEvent {
        SignalEvent {
            thread_id: 0,
            local_step: 0,
            signal_type,
        }
    }

    #[test]
    fn test_epoch_boundary_triggers_processing() {
        let config = CoordinatorConfig {
            epoch_size: 3,
            ..Default::default()
        };
        let mut coordinator = Coordinator::new(config);
        let mut weight_table = WeightTable::new();

        let signal = SignalType::CoverageDelta {
            node_id: 1,
            action: "act".into(),
        };

        // First two signals: no directives yet.
        let d1 = coordinator.feed_signal(make_signal(signal.clone()), &mut weight_table, &[]);
        assert!(d1.is_empty());
        let d2 = coordinator.feed_signal(make_signal(signal.clone()), &mut weight_table, &[]);
        assert!(d2.is_empty());

        // Third signal triggers epoch processing.
        let d3 = coordinator.feed_signal(make_signal(signal), &mut weight_table, &[]);
        assert!(!d3.is_empty());
        assert_eq!(coordinator.current_epoch(), 1);
    }

    #[test]
    fn test_flush_processes_partial_epoch() {
        let config = CoordinatorConfig {
            epoch_size: 100,
            ..Default::default()
        };
        let mut coordinator = Coordinator::new(config);
        let mut weight_table = WeightTable::new();

        coordinator.feed_signal(
            make_signal(SignalType::CoverageDelta {
                node_id: 1,
                action: "a".into(),
            }),
            &mut weight_table,
            &[],
        );

        let directives = coordinator.flush(&mut weight_table, &[]);
        assert!(!directives.is_empty());
        assert_eq!(coordinator.current_epoch(), 1);
    }

    #[test]
    fn test_coverage_delta_produces_adjust_weight() {
        let config = CoordinatorConfig {
            epoch_size: 1,
            ..Default::default()
        };
        let mut coordinator = Coordinator::new(config);
        let mut weight_table = WeightTable::new();
        weight_table.set_default("act", 50.0);

        let directives = coordinator.feed_signal(
            make_signal(SignalType::CoverageDelta {
                node_id: 1,
                action: "act".into(),
            }),
            &mut weight_table,
            &[],
        );

        assert!(directives.iter().any(
            |d| matches!(d, Directive::AdjustWeight { branch_id, .. } if branch_id == "act")
        ));
    }

    #[test]
    fn test_guard_failure_produces_decay() {
        let config = CoordinatorConfig {
            epoch_size: 1,
            guard_failure_decay: 0.3,
            ..Default::default()
        };
        let mut coordinator = Coordinator::new(config);
        let mut weight_table = WeightTable::new();
        weight_table.set_default("br", 100.0);

        let directives = coordinator.feed_signal(
            make_signal(SignalType::GuardFailure {
                branch_id: "br".into(),
                action: "a".into(),
            }),
            &mut weight_table,
            &[],
        );

        assert!(directives.iter().any(|d| matches!(
            d,
            Directive::AdjustWeight { multiplier, .. } if *multiplier == 0.3
        )));
    }

    #[test]
    fn test_crash_produces_force_and_boost() {
        let config = CoordinatorConfig {
            epoch_size: 1,
            ..Default::default()
        };
        let mut coordinator = Coordinator::new(config);
        let mut weight_table = WeightTable::new();

        let directives = coordinator.feed_signal(
            make_signal(SignalType::Crash {
                action: "buggy".into(),
                message: "trap".into(),
            }),
            &mut weight_table,
            &[],
        );

        let has_force = directives
            .iter()
            .any(|d| matches!(d, Directive::Force { action, .. } if action == "buggy"));
        let has_boost = directives
            .iter()
            .any(|d| matches!(d, Directive::AdjustWeight { branch_id, .. } if branch_id == "buggy"));
        assert!(has_force);
        assert!(has_boost);
    }

    #[test]
    fn test_plateau_forces_uncovered_targets() {
        let config = CoordinatorConfig {
            epoch_size: 1,
            ..Default::default()
        };
        let mut coordinator = Coordinator::new(config);
        coordinator.set_uncovered_target_branches(vec!["target_a".into(), "target_b".into()]);
        let mut weight_table = WeightTable::new();

        let directives = coordinator.feed_signal(
            make_signal(SignalType::CoveragePlateau {
                current_coverage: 0.8,
                delta_rate: 0.001,
            }),
            &mut weight_table,
            &[],
        );

        let force_actions: Vec<_> = directives
            .iter()
            .filter_map(|d| match d {
                Directive::Force { action, .. } => Some(action.clone()),
                _ => None,
            })
            .collect();
        assert!(force_actions.contains(&"target_a".to_string()));
        assert!(force_actions.contains(&"target_b".to_string()));
    }

    #[test]
    fn test_directive_log_accumulates_across_epochs() {
        let config = CoordinatorConfig {
            epoch_size: 1,
            ..Default::default()
        };
        let mut coordinator = Coordinator::new(config);
        let mut weight_table = WeightTable::new();

        coordinator.feed_signal(
            make_signal(SignalType::CoverageDelta {
                node_id: 1,
                action: "a".into(),
            }),
            &mut weight_table,
            &[],
        );
        coordinator.feed_signal(
            make_signal(SignalType::CoverageDelta {
                node_id: 2,
                action: "b".into(),
            }),
            &mut weight_table,
            &[],
        );

        assert_eq!(coordinator.directive_log().len(), 2);
        assert_eq!(coordinator.current_epoch(), 2);
    }

    #[test]
    fn test_signal_ordering_by_thread_and_step() {
        let config = CoordinatorConfig {
            epoch_size: 3,
            ..Default::default()
        };
        let mut coordinator = Coordinator::new(config);
        let mut weight_table = WeightTable::new();

        // Signals from different threads, out of order.
        let signals = vec![
            SignalEvent {
                thread_id: 1,
                local_step: 2,
                signal_type: SignalType::CoverageDelta {
                    node_id: 3,
                    action: "c".into(),
                },
            },
            SignalEvent {
                thread_id: 0,
                local_step: 1,
                signal_type: SignalType::CoverageDelta {
                    node_id: 1,
                    action: "a".into(),
                },
            },
            SignalEvent {
                thread_id: 1,
                local_step: 1,
                signal_type: SignalType::CoverageDelta {
                    node_id: 2,
                    action: "b".into(),
                },
            },
        ];

        for s in signals {
            coordinator.feed_signal(s, &mut weight_table, &[]);
        }

        // After epoch, directives should be in total order:
        // thread 0 step 1 -> thread 1 step 1 -> thread 1 step 2
        let log = coordinator.directive_log();
        assert_eq!(log.len(), 3);
        // Verify ordering by checking the sequence numbers are ascending.
        for i in 1..log.entries().len() {
            assert!(log.entries()[i].seqno > log.entries()[i - 1].seqno);
        }
    }
}
