//! Two-step timeout response handler.
//!
//! When an action times out:
//! 1. First occurrence: halve fuel, schedule retry.
//!    If retry completes → emit performance finding (not a bug, just slow).
//! 2. Second timeout (or retry also times out): bounded skip + timeout finding.
//!
//! This prevents prematurely marking slow-but-correct code as broken,
//! while still detecting genuinely infinite loops.

use std::collections::HashMap;

use super::directive::Directive;

/// State of a timeout-tracked action.
#[derive(Debug, Clone, PartialEq)]
enum TimeoutState {
    /// Retry scheduled — waiting for retry result.
    RetryScheduled {
        reduced_fuel: u64,
    },
    /// Retry also timed out — skip this action.
    PermanentSkip {
        skip_remaining: u32,
    },
}

/// Tracks timeout two-step state per action.
#[derive(Debug, Clone)]
pub struct TimeoutTracker {
    states: HashMap<String, TimeoutState>,
    /// Default skip budget when permanently skipping.
    default_skip_budget: u32,
}

/// Result of processing a timeout event.
#[derive(Debug, Clone)]
pub struct TimeoutResponse {
    /// Directive to apply (if any).
    pub directive: Option<Directive>,
    /// Whether this is the first timeout (retry possible) or permanent.
    pub is_retry: bool,
    /// Whether this action should be reported as a finding.
    pub is_finding: bool,
    /// Reduced fuel for retry (if applicable).
    pub reduced_fuel: Option<u64>,
}

impl TimeoutTracker {
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
            default_skip_budget: 50,
        }
    }

    /// Handle a timeout event for an action.
    /// Returns directives to apply based on the two-step protocol.
    pub fn handle_timeout(
        &mut self,
        action: &str,
        fuel_consumed: Option<u64>,
    ) -> Option<Directive> {
        let state = self.states.get(action).cloned();

        match state {
            None => {
                // First timeout: halve fuel, schedule retry.
                let reduced = fuel_consumed.map(|f| f / 2).unwrap_or(500_000);
                self.states.insert(
                    action.to_string(),
                    TimeoutState::RetryScheduled {
                        reduced_fuel: reduced,
                    },
                );
                // No directive yet — the caller should retry with reduced fuel.
                None
            }

            Some(TimeoutState::RetryScheduled { .. }) => {
                // Retry also timed out → permanent skip.
                self.states.insert(
                    action.to_string(),
                    TimeoutState::PermanentSkip {
                        skip_remaining: self.default_skip_budget,
                    },
                );
                // Emit a skip directive.
                Some(Directive::Skip {
                    branch_id: action.to_string(),
                    model_state_hash: 0,
                    remaining: self.default_skip_budget,
                })
            }

            Some(TimeoutState::PermanentSkip { skip_remaining }) => {
                // Already skipping — decrement remaining.
                if skip_remaining > 0 {
                    self.states.insert(
                        action.to_string(),
                        TimeoutState::PermanentSkip {
                            skip_remaining: skip_remaining.saturating_sub(1),
                        },
                    );
                } else {
                    // Skip expired — reset, allow retry.
                    self.states.remove(action);
                }
                None
            }
        }
    }

    /// Report that a retry succeeded (action completed at reduced fuel).
    /// This means the action is slow but not broken.
    pub fn report_retry_success(&mut self, action: &str) {
        self.states.remove(action);
    }

    /// Check if an action is in retry state (should be retried with reduced fuel).
    pub fn needs_retry(&self, action: &str) -> Option<u64> {
        match self.states.get(action) {
            Some(TimeoutState::RetryScheduled { reduced_fuel }) => Some(*reduced_fuel),
            _ => None,
        }
    }

    /// Check if an action is permanently skipped.
    pub fn is_skipped(&self, action: &str) -> bool {
        matches!(
            self.states.get(action),
            Some(TimeoutState::PermanentSkip { .. })
        )
    }

    /// Number of actions currently tracked.
    pub fn tracked_count(&self) -> usize {
        self.states.len()
    }
}

impl Default for TimeoutTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_first_timeout_schedules_retry() {
        let mut tracker = TimeoutTracker::new();

        let directive = tracker.handle_timeout("slow_fn", Some(1_000_000));
        assert!(directive.is_none()); // No directive yet, retry scheduled.
        assert_eq!(tracker.needs_retry("slow_fn"), Some(500_000));
    }

    #[test]
    fn test_second_timeout_produces_skip() {
        let mut tracker = TimeoutTracker::new();

        // First timeout.
        tracker.handle_timeout("slow_fn", Some(1_000_000));

        // Second timeout (retry failed).
        let directive = tracker.handle_timeout("slow_fn", Some(500_000));
        assert!(directive.is_some());
        assert!(matches!(directive.unwrap(), Directive::Skip { .. }));
        assert!(tracker.is_skipped("slow_fn"));
    }

    #[test]
    fn test_retry_success_clears_state() {
        let mut tracker = TimeoutTracker::new();

        tracker.handle_timeout("slow_fn", Some(1_000_000));
        assert!(tracker.needs_retry("slow_fn").is_some());

        tracker.report_retry_success("slow_fn");
        assert!(tracker.needs_retry("slow_fn").is_none());
        assert!(!tracker.is_skipped("slow_fn"));
    }

    #[test]
    fn test_skip_expires_after_budget() {
        let mut tracker = TimeoutTracker {
            states: HashMap::new(),
            default_skip_budget: 2,
        };

        // First timeout.
        tracker.handle_timeout("fn", Some(100));
        // Second timeout → skip with budget 2.
        tracker.handle_timeout("fn", Some(50));
        assert!(tracker.is_skipped("fn"));

        // Decrement skip.
        tracker.handle_timeout("fn", None);
        assert!(tracker.is_skipped("fn"));

        // Decrement to 0.
        tracker.handle_timeout("fn", None);
        assert!(tracker.is_skipped("fn"));

        // Skip expired → removed.
        tracker.handle_timeout("fn", None);
        assert!(!tracker.is_skipped("fn"));
    }

    #[test]
    fn test_unknown_action_starts_fresh() {
        let tracker = TimeoutTracker::new();
        assert!(tracker.needs_retry("unknown").is_none());
        assert!(!tracker.is_skipped("unknown"));
    }

    #[test]
    fn test_no_fuel_uses_default() {
        let mut tracker = TimeoutTracker::new();

        tracker.handle_timeout("fn", None);
        assert_eq!(tracker.needs_retry("fn"), Some(500_000));
    }
}
