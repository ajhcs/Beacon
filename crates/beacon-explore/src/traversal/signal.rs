/// Signals emitted by the traversal engine during action execution.
/// These drive the adaptation layer (signal -> directive mappings).

/// All signal types the engine can emit.
#[derive(Debug, Clone, PartialEq)]
pub enum SignalType {
    /// A new state or transition was covered for the first time.
    CoverageDelta {
        node_id: u32,
        action: String,
    },
    /// An invariant or temporal property was violated.
    PropertyViolation {
        property: String,
        details: String,
    },
    /// Model truth diverged from DUT observation.
    Discrepancy {
        action: String,
        model_value: String,
        observed_value: String,
    },
    /// DUT panicked or trapped in WASM.
    Crash {
        action: String,
        message: String,
    },
    /// DUT action exceeded time/fuel budget.
    Timeout {
        action: String,
        fuel_consumed: Option<u64>,
    },
    /// Guard prevented transition from current state.
    GuardFailure {
        branch_id: String,
        action: String,
    },
    /// Coverage delta rate approaching zero.
    CoveragePlateau {
        current_coverage: f64,
        delta_rate: f64,
    },
}

/// A signal event with metadata for replay capsule construction.
#[derive(Debug, Clone)]
pub struct SignalEvent {
    /// Thread ID that emitted this signal (0 for single-threaded).
    pub thread_id: u32,
    /// Monotonic step counter within the thread.
    pub local_step: u64,
    /// The signal itself.
    pub signal_type: SignalType,
}

/// A finding — a significant signal that should be reported to the user.
#[derive(Debug, Clone)]
pub struct Finding {
    /// Unique ID for this finding within the campaign.
    pub id: u64,
    /// The signal that triggered this finding.
    pub signal: SignalEvent,
    /// The action trace leading to this finding.
    pub trace_indices: Vec<usize>,
    /// Model generation at the time of the finding.
    pub model_generation: u64,
}
