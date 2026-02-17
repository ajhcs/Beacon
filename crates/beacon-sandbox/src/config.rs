/// Sandbox configuration — memory limits, fuel metering, isolation guarantees.
use serde::{Deserialize, Serialize};

/// Configuration for the WASM sandbox environment.
///
/// Controls isolation guarantees: no filesystem, no network, no clock,
/// plus resource limits (memory cap, fuel metering for CPU budget).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// Maximum memory in bytes (default: 256 MB).
    pub memory_limit_bytes: u64,
    /// Fuel budget per action invocation. None = unlimited (not recommended).
    pub fuel_per_action: Option<u64>,
    /// Whether to enable WASI (should be false for isolation).
    pub enable_wasi: bool,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            memory_limit_bytes: 256 * 1024 * 1024, // 256 MB
            fuel_per_action: Some(1_000_000),
            enable_wasi: false,
        }
    }
}
