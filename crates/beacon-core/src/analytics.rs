//! Campaign analytics and telemetry.
//!
//! Tracks coverage curves, finding rates, adaptation effectiveness,
//! and per-epoch statistics for campaign-level observability.

use std::time::Instant;

use serde::{Deserialize, Serialize};

/// A timestamped data point in a coverage curve.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoveragePoint {
    /// Step number when this measurement was taken.
    pub step: u64,
    /// Number of distinct coverage targets hit so far.
    pub targets_hit: u32,
    /// Total coverage targets.
    pub targets_total: u32,
    /// Coverage percentage (0.0-1.0).
    pub percent: f64,
}

/// A timestamped finding rate measurement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindingRatePoint {
    /// Epoch number.
    pub epoch: u64,
    /// Findings discovered in this epoch.
    pub findings_in_epoch: u32,
    /// Cumulative findings so far.
    pub cumulative_findings: u32,
}

/// Per-epoch adaptation statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpochStats {
    /// Epoch number.
    pub epoch: u64,
    /// Number of signals processed in this epoch.
    pub signals_processed: u32,
    /// Number of directives emitted.
    pub directives_emitted: u32,
    /// Coverage delta rate (new targets / step).
    pub coverage_delta_rate: f64,
    /// Number of guard failures.
    pub guard_failures: u32,
    /// Number of timeouts.
    pub timeouts: u32,
}

/// Campaign-level analytics aggregator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignAnalytics {
    /// Coverage curve over time.
    pub coverage_curve: Vec<CoveragePoint>,
    /// Finding rate per epoch.
    pub finding_rates: Vec<FindingRatePoint>,
    /// Per-epoch adaptation statistics.
    pub epoch_stats: Vec<EpochStats>,
    /// Total steps executed.
    pub total_steps: u64,
    /// Total findings discovered.
    pub total_findings: u32,
    /// Peak coverage percentage.
    pub peak_coverage: f64,
    /// Wall-clock elapsed seconds.
    pub elapsed_secs: f64,
    /// Campaign state.
    pub state: CampaignPhase,
}

/// Campaign lifecycle phase.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CampaignPhase {
    Compiled,
    DutLoaded,
    Running,
    Complete,
    Aborted,
}

impl CampaignAnalytics {
    pub fn new() -> Self {
        Self {
            coverage_curve: Vec::new(),
            finding_rates: Vec::new(),
            epoch_stats: Vec::new(),
            total_steps: 0,
            total_findings: 0,
            peak_coverage: 0.0,
            elapsed_secs: 0.0,
            state: CampaignPhase::Compiled,
        }
    }

    /// Record a coverage measurement.
    pub fn record_coverage(&mut self, step: u64, targets_hit: u32, targets_total: u32) {
        let percent = if targets_total > 0 {
            targets_hit as f64 / targets_total as f64
        } else {
            0.0
        };

        self.coverage_curve.push(CoveragePoint {
            step,
            targets_hit,
            targets_total,
            percent,
        });

        if percent > self.peak_coverage {
            self.peak_coverage = percent;
        }
    }

    /// Record finding rate for an epoch.
    pub fn record_finding_rate(&mut self, epoch: u64, findings_in_epoch: u32) {
        self.total_findings += findings_in_epoch;
        self.finding_rates.push(FindingRatePoint {
            epoch,
            findings_in_epoch,
            cumulative_findings: self.total_findings,
        });
    }

    /// Record epoch statistics.
    pub fn record_epoch(&mut self, stats: EpochStats) {
        self.epoch_stats.push(stats);
    }

    /// Update total steps.
    pub fn set_total_steps(&mut self, steps: u64) {
        self.total_steps = steps;
    }

    /// Update elapsed time.
    pub fn set_elapsed(&mut self, secs: f64) {
        self.elapsed_secs = secs;
    }

    /// Compute finding rate (findings per 1000 steps).
    pub fn finding_rate_per_k_steps(&self) -> f64 {
        if self.total_steps == 0 {
            0.0
        } else {
            (self.total_findings as f64 / self.total_steps as f64) * 1000.0
        }
    }

    /// Compute coverage velocity (coverage % gained per 1000 steps).
    pub fn coverage_velocity(&self) -> f64 {
        if self.coverage_curve.len() < 2 || self.total_steps == 0 {
            return 0.0;
        }
        let first = &self.coverage_curve[0];
        let last = self.coverage_curve.last().unwrap();
        let coverage_gained = last.percent - first.percent;
        (coverage_gained / self.total_steps as f64) * 1000.0
    }

    /// Compute adaptation effectiveness: ratio of finding-producing epochs
    /// to total epochs.
    pub fn adaptation_effectiveness(&self) -> f64 {
        if self.finding_rates.is_empty() {
            return 0.0;
        }
        let productive = self
            .finding_rates
            .iter()
            .filter(|r| r.findings_in_epoch > 0)
            .count();
        productive as f64 / self.finding_rates.len() as f64
    }

    /// Generate a summary for MCP tool responses.
    pub fn summary(&self) -> AnalyticsSummary {
        AnalyticsSummary {
            total_steps: self.total_steps,
            total_findings: self.total_findings,
            peak_coverage: self.peak_coverage,
            elapsed_secs: self.elapsed_secs,
            finding_rate_per_k: self.finding_rate_per_k_steps(),
            coverage_velocity: self.coverage_velocity(),
            adaptation_effectiveness: self.adaptation_effectiveness(),
            epochs_completed: self.epoch_stats.len() as u64,
            state: self.state.clone(),
        }
    }
}

impl Default for CampaignAnalytics {
    fn default() -> Self {
        Self::new()
    }
}

/// Compact analytics summary for MCP responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsSummary {
    pub total_steps: u64,
    pub total_findings: u32,
    pub peak_coverage: f64,
    pub elapsed_secs: f64,
    pub finding_rate_per_k: f64,
    pub coverage_velocity: f64,
    pub adaptation_effectiveness: f64,
    pub epochs_completed: u64,
    pub state: CampaignPhase,
}

/// A simple wall-clock timer for campaign duration.
#[derive(Debug)]
pub struct CampaignTimer {
    start: Instant,
}

impl CampaignTimer {
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    pub fn elapsed_secs(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_analytics_is_empty() {
        let a = CampaignAnalytics::new();
        assert_eq!(a.total_steps, 0);
        assert_eq!(a.total_findings, 0);
        assert_eq!(a.peak_coverage, 0.0);
        assert_eq!(a.state, CampaignPhase::Compiled);
    }

    #[test]
    fn test_coverage_recording() {
        let mut a = CampaignAnalytics::new();
        a.record_coverage(100, 5, 20);
        a.record_coverage(200, 12, 20);

        assert_eq!(a.coverage_curve.len(), 2);
        assert!((a.coverage_curve[0].percent - 0.25).abs() < 0.01);
        assert!((a.coverage_curve[1].percent - 0.60).abs() < 0.01);
        assert!((a.peak_coverage - 0.60).abs() < 0.01);
    }

    #[test]
    fn test_finding_rate_accumulates() {
        let mut a = CampaignAnalytics::new();
        a.record_finding_rate(0, 3);
        a.record_finding_rate(1, 1);
        a.record_finding_rate(2, 0);

        assert_eq!(a.total_findings, 4);
        assert_eq!(a.finding_rates.len(), 3);
        assert_eq!(a.finding_rates[1].cumulative_findings, 4);
    }

    #[test]
    fn test_finding_rate_per_k_steps() {
        let mut a = CampaignAnalytics::new();
        a.total_findings = 10;
        a.total_steps = 5000;

        assert!((a.finding_rate_per_k_steps() - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_coverage_velocity() {
        let mut a = CampaignAnalytics::new();
        a.record_coverage(0, 0, 100);
        a.record_coverage(1000, 50, 100);
        a.total_steps = 1000;

        // 0.5 coverage / 1000 steps * 1000 = 0.5
        assert!((a.coverage_velocity() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_adaptation_effectiveness() {
        let mut a = CampaignAnalytics::new();
        a.record_finding_rate(0, 2); // productive
        a.record_finding_rate(1, 0); // unproductive
        a.record_finding_rate(2, 1); // productive
        a.record_finding_rate(3, 0); // unproductive

        assert!((a.adaptation_effectiveness() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_summary_generation() {
        let mut a = CampaignAnalytics::new();
        a.total_steps = 10000;
        a.total_findings = 5;
        a.peak_coverage = 0.85;
        a.elapsed_secs = 12.5;
        a.state = CampaignPhase::Complete;

        let s = a.summary();
        assert_eq!(s.total_steps, 10000);
        assert_eq!(s.total_findings, 5);
        assert_eq!(s.state, CampaignPhase::Complete);
    }

    #[test]
    fn test_zero_steps_rates() {
        let a = CampaignAnalytics::new();
        assert_eq!(a.finding_rate_per_k_steps(), 0.0);
        assert_eq!(a.coverage_velocity(), 0.0);
        assert_eq!(a.adaptation_effectiveness(), 0.0);
    }
}
