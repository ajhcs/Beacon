//! Resource limits and graceful degradation.
//!
//! Provides configurable caps on wall-time, iterations, memory, and
//! concurrent campaigns. When limits are hit, the engine produces
//! partial results rather than failing.

use serde::{Deserialize, Serialize};

/// Resource limits for a single campaign.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum wall-clock seconds before forced stop.
    pub max_wall_secs: u64,
    /// Maximum total iterations (steps) across all passes.
    pub max_iterations: u64,
    /// Maximum memory usage in bytes (advisory — checked periodically).
    pub max_memory_bytes: u64,
    /// Maximum concurrent traversal threads.
    pub max_threads: u32,
    /// Maximum findings before stopping (prevents runaway on catastrophically broken DUT).
    pub max_findings: u32,
    /// Maximum passes per campaign.
    pub max_passes: u32,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_wall_secs: 300,        // 5 minutes
            max_iterations: 1_000_000, // 1M steps
            max_memory_bytes: 512 * 1024 * 1024, // 512 MB
            max_threads: 4,
            max_findings: 1000,
            max_passes: 100,
        }
    }
}

/// Global engine limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineLimits {
    /// Maximum concurrent campaigns.
    pub max_concurrent_campaigns: u32,
    /// Maximum WASM module size in bytes.
    pub max_wasm_module_bytes: u64,
    /// Maximum IR JSON size in bytes.
    pub max_ir_json_bytes: u64,
}

impl Default for EngineLimits {
    fn default() -> Self {
        Self {
            max_concurrent_campaigns: 8,
            max_wasm_module_bytes: 64 * 1024 * 1024, // 64 MB
            max_ir_json_bytes: 16 * 1024 * 1024,     // 16 MB
        }
    }
}

/// Reason a campaign was stopped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StopReason {
    /// All passes completed normally.
    Complete,
    /// Wall-clock time limit exceeded.
    WallTimeExceeded,
    /// Iteration limit exceeded.
    IterationLimitExceeded,
    /// Finding limit exceeded (DUT too broken to continue).
    FindingLimitExceeded,
    /// User-requested abort.
    UserAborted,
    /// Memory limit exceeded (advisory).
    MemoryLimitExceeded,
}

/// Check resource usage against limits.
pub struct ResourceChecker {
    limits: ResourceLimits,
    start_time: std::time::Instant,
}

impl ResourceChecker {
    pub fn new(limits: ResourceLimits) -> Self {
        Self {
            limits,
            start_time: std::time::Instant::now(),
        }
    }

    /// Check if any limit has been exceeded.
    /// Returns None if all ok, or the reason for stopping.
    pub fn check(
        &self,
        iterations: u64,
        findings: u32,
        passes: u32,
    ) -> Option<StopReason> {
        let elapsed = self.start_time.elapsed().as_secs();

        if elapsed >= self.limits.max_wall_secs {
            return Some(StopReason::WallTimeExceeded);
        }
        if iterations >= self.limits.max_iterations {
            return Some(StopReason::IterationLimitExceeded);
        }
        if findings >= self.limits.max_findings {
            return Some(StopReason::FindingLimitExceeded);
        }
        if passes >= self.limits.max_passes {
            return Some(StopReason::Complete);
        }
        None
    }

    /// Check only the wall-time limit (cheap, no other args needed).
    pub fn wall_time_exceeded(&self) -> bool {
        self.start_time.elapsed().as_secs() >= self.limits.max_wall_secs
    }

    /// Elapsed seconds since start.
    pub fn elapsed_secs(&self) -> f64 {
        self.start_time.elapsed().as_secs_f64()
    }

    /// Get the configured limits.
    pub fn limits(&self) -> &ResourceLimits {
        &self.limits
    }
}

/// Validate engine-level limits before accepting a request.
pub fn validate_engine_limits(
    engine_limits: &EngineLimits,
    current_campaigns: usize,
    ir_json_size: usize,
) -> Result<(), LimitViolation> {
    if current_campaigns as u32 >= engine_limits.max_concurrent_campaigns {
        return Err(LimitViolation::TooManyCampaigns {
            current: current_campaigns as u32,
            max: engine_limits.max_concurrent_campaigns,
        });
    }
    if ir_json_size as u64 > engine_limits.max_ir_json_bytes {
        return Err(LimitViolation::IrTooLarge {
            size: ir_json_size as u64,
            max: engine_limits.max_ir_json_bytes,
        });
    }
    Ok(())
}

/// A limit violation error.
#[derive(Debug, Clone, PartialEq)]
pub enum LimitViolation {
    TooManyCampaigns { current: u32, max: u32 },
    IrTooLarge { size: u64, max: u64 },
    WasmTooLarge { size: u64, max: u64 },
}

impl std::fmt::Display for LimitViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooManyCampaigns { current, max } => {
                write!(f, "Too many concurrent campaigns ({current}/{max})")
            }
            Self::IrTooLarge { size, max } => {
                write!(f, "IR JSON too large ({size} bytes, max {max})")
            }
            Self::WasmTooLarge { size, max } => {
                write!(f, "WASM module too large ({size} bytes, max {max})")
            }
        }
    }
}

impl std::error::Error for LimitViolation {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_limits() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.max_wall_secs, 300);
        assert_eq!(limits.max_iterations, 1_000_000);
        assert_eq!(limits.max_threads, 4);
        assert_eq!(limits.max_findings, 1000);
    }

    #[test]
    fn test_resource_checker_all_ok() {
        let limits = ResourceLimits {
            max_wall_secs: 60,
            max_iterations: 10_000,
            max_findings: 100,
            max_passes: 50,
            ..Default::default()
        };
        let checker = ResourceChecker::new(limits);

        assert!(checker.check(100, 5, 2).is_none());
    }

    #[test]
    fn test_iteration_limit() {
        let limits = ResourceLimits {
            max_iterations: 100,
            ..Default::default()
        };
        let checker = ResourceChecker::new(limits);

        assert_eq!(
            checker.check(100, 0, 0),
            Some(StopReason::IterationLimitExceeded)
        );
    }

    #[test]
    fn test_finding_limit() {
        let limits = ResourceLimits {
            max_findings: 5,
            ..Default::default()
        };
        let checker = ResourceChecker::new(limits);

        assert_eq!(
            checker.check(50, 5, 0),
            Some(StopReason::FindingLimitExceeded)
        );
    }

    #[test]
    fn test_pass_limit_returns_complete() {
        let limits = ResourceLimits {
            max_passes: 10,
            ..Default::default()
        };
        let checker = ResourceChecker::new(limits);

        assert_eq!(checker.check(500, 3, 10), Some(StopReason::Complete));
    }

    #[test]
    fn test_engine_limits_validation() {
        let engine = EngineLimits {
            max_concurrent_campaigns: 2,
            max_ir_json_bytes: 1024,
            ..Default::default()
        };

        // OK
        assert!(validate_engine_limits(&engine, 1, 512).is_ok());

        // Too many campaigns
        assert_eq!(
            validate_engine_limits(&engine, 2, 512),
            Err(LimitViolation::TooManyCampaigns {
                current: 2,
                max: 2
            })
        );

        // IR too large
        assert_eq!(
            validate_engine_limits(&engine, 0, 2048),
            Err(LimitViolation::IrTooLarge {
                size: 2048,
                max: 1024
            })
        );
    }

    #[test]
    fn test_limit_violation_display() {
        let v = LimitViolation::TooManyCampaigns {
            current: 8,
            max: 8,
        };
        assert!(v.to_string().contains("8/8"));
    }

    #[test]
    fn test_elapsed_secs() {
        let checker = ResourceChecker::new(ResourceLimits::default());
        // Should be very small right after creation.
        assert!(checker.elapsed_secs() < 1.0);
    }
}
