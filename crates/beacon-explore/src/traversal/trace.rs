use beacon_compiler::graph::NodeId;

/// A single step in the traversal trace, for replay capsule construction.
#[derive(Debug, Clone)]
pub struct TraceStep {
    /// The graph node that was visited.
    pub node_id: NodeId,
    /// What kind of step this was.
    pub kind: TraceStepKind,
    /// Step number (monotonic within the traversal).
    pub step_number: u64,
}

/// The kind of traversal step taken.
#[derive(Debug, Clone)]
pub enum TraceStepKind {
    /// Entered a start node.
    Start,
    /// Reached an end node.
    End,
    /// Selected a branch in an alt block.
    BranchSelected { branch_id: String, weight_used: f64 },
    /// Entered a loop.
    LoopEnter { iterations_chosen: u32 },
    /// Exited a loop.
    LoopExit,
    /// Executed an action (call terminal).
    ActionExecuted {
        action: String,
        guard_passed: bool,
        return_value: Option<i32>,
        fuel_consumed: Option<u64>,
    },
    /// Guard check failed — action not executed.
    GuardFailed { action: String },
}

/// Full traversal trace for a campaign run.
#[derive(Debug, Clone, Default)]
pub struct TraversalTrace {
    steps: Vec<TraceStep>,
    next_step: u64,
}

impl TraversalTrace {
    pub fn new() -> Self {
        Self {
            steps: Vec::new(),
            next_step: 0,
        }
    }

    pub fn record(&mut self, node_id: NodeId, kind: TraceStepKind) {
        self.steps.push(TraceStep {
            node_id,
            kind,
            step_number: self.next_step,
        });
        self.next_step += 1;
    }

    pub fn steps(&self) -> &[TraceStep] {
        &self.steps
    }

    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}
