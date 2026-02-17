//! Per-epoch weight decay, normalization, and coverage floor enforcement.
//!
//! Each epoch applies:
//! 1. Global decay to prevent fixation on explored paths.
//! 2. Boost on finding-yielding branches proportional to severity.
//! 3. Normalization of alt-block branches to sum to 100.
//! 4. Coverage floor enforcement ensuring uncovered targets maintain
//!    minimum probability mass.

use crate::traversal::weight_table::WeightTable;

/// Configuration for per-epoch decay behavior.
#[derive(Debug, Clone)]
pub struct DecayConfig {
    /// Decay factor applied to all state-conditioned weights per epoch.
    /// Prevents fixation. Typical: 0.95.
    pub global_decay: f64,
    /// Minimum weight — prevents complete suppression.
    /// Only overridden by provable unreachability.
    pub min_weight: f64,
}

impl Default for DecayConfig {
    fn default() -> Self {
        Self {
            global_decay: 0.95,
            min_weight: 0.1,
        }
    }
}

/// Apply per-epoch decay to all state-conditioned weights.
///
/// After decay, no weight drops below `min_weight` unless it was
/// already at 0.0 (provably unreachable).
pub fn apply_epoch_decay(weight_table: &mut WeightTable, config: &DecayConfig) {
    weight_table.decay_all(config.global_decay);
    weight_table.clamp_min(config.min_weight);
}

/// Enforce coverage floor: ensure branches reaching uncovered targets
/// maintain at least `threshold` fraction of total weight budget.
///
/// If the total weight of uncovered-target-reaching branches drops below
/// `threshold * total_weight_budget`, boost them proportionally.
pub fn enforce_coverage_floor(
    weight_table: &mut WeightTable,
    uncovered_branches: &[String],
    threshold: f64,
) {
    if uncovered_branches.is_empty() || threshold <= 0.0 {
        return;
    }

    // Compute current total weight for uncovered branches (at default state hash 0).
    let uncovered_total: f64 = uncovered_branches
        .iter()
        .map(|b| weight_table.get(b, 0))
        .sum();

    // Compute total weight budget (all known branches).
    // We use the uncovered branches' weights + a baseline.
    // In practice, this would sum all branches in the alt block.
    let total_budget = 100.0; // Normalized target

    let floor = threshold * total_budget;

    if uncovered_total < floor && uncovered_total > 0.0 {
        // Boost uncovered branches to meet the floor.
        let boost = floor / uncovered_total;
        for branch in uncovered_branches {
            weight_table.adjust(branch, 0, boost);
        }
    } else if uncovered_total == 0.0 && !uncovered_branches.is_empty() {
        // All uncovered branches at zero — restore to minimum.
        let per_branch = floor / uncovered_branches.len() as f64;
        for branch in uncovered_branches {
            weight_table.set(branch, 0, per_branch);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decay_reduces_weights() {
        let mut wt = WeightTable::new();
        wt.set("b1", 0, 100.0);
        wt.set("b2", 0, 50.0);

        apply_epoch_decay(
            &mut wt,
            &DecayConfig {
                global_decay: 0.9,
                min_weight: 0.1,
            },
        );

        assert!((wt.get("b1", 0) - 90.0).abs() < 0.01);
        assert!((wt.get("b2", 0) - 45.0).abs() < 0.01);
    }

    #[test]
    fn test_decay_respects_minimum() {
        let mut wt = WeightTable::new();
        wt.set("b1", 0, 0.2);

        apply_epoch_decay(
            &mut wt,
            &DecayConfig {
                global_decay: 0.1,
                min_weight: 0.1,
            },
        );

        // 0.2 * 0.1 = 0.02, but clamped to 0.1
        assert!((wt.get("b1", 0) - 0.1).abs() < 0.01);
    }

    #[test]
    fn test_decay_preserves_zero_for_unreachable() {
        let mut wt = WeightTable::new();
        wt.set("unreachable", 0, 0.0);

        apply_epoch_decay(
            &mut wt,
            &DecayConfig {
                global_decay: 0.95,
                min_weight: 0.1,
            },
        );

        // Zero stays zero (provably unreachable).
        assert_eq!(wt.get("unreachable", 0), 0.0);
    }

    #[test]
    fn test_coverage_floor_boosts_uncovered() {
        let mut wt = WeightTable::new();
        wt.set("covered", 0, 90.0);
        wt.set("uncovered_a", 0, 1.0);
        wt.set("uncovered_b", 0, 1.0);

        // Floor = 5% of 100 = 5.0
        // Uncovered total = 2.0 < 5.0 → boost
        enforce_coverage_floor(&mut wt, &["uncovered_a".into(), "uncovered_b".into()], 0.05);

        let uncovered_total = wt.get("uncovered_a", 0) + wt.get("uncovered_b", 0);
        assert!(
            uncovered_total >= 5.0,
            "uncovered total {uncovered_total} should be >= 5.0"
        );
    }

    #[test]
    fn test_coverage_floor_restores_from_zero() {
        let mut wt = WeightTable::new();
        wt.set("dead_a", 0, 0.0);
        wt.set("dead_b", 0, 0.0);

        enforce_coverage_floor(&mut wt, &["dead_a".into(), "dead_b".into()], 0.05);

        // Both should be restored to at least something.
        assert!(wt.get("dead_a", 0) > 0.0);
        assert!(wt.get("dead_b", 0) > 0.0);
    }

    #[test]
    fn test_coverage_floor_no_op_when_above_threshold() {
        let mut wt = WeightTable::new();
        wt.set("healthy", 0, 50.0);

        let before = wt.get("healthy", 0);
        enforce_coverage_floor(&mut wt, &["healthy".into()], 0.05);
        let after = wt.get("healthy", 0);

        assert_eq!(before, after);
    }

    #[test]
    fn test_coverage_floor_empty_branches_no_op() {
        let mut wt = WeightTable::new();
        wt.set("b1", 0, 10.0);

        let before = wt.get("b1", 0);
        enforce_coverage_floor(&mut wt, &[], 0.05);
        assert_eq!(wt.get("b1", 0), before);
    }
}
