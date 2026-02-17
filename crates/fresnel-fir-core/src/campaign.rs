use std::collections::HashMap;
use std::sync::Mutex;

use fresnel_fir_compiler::compile;
use fresnel_fir_compiler::compile::CompiledIR;
use fresnel_fir_ir::parse::parse_ir;

use crate::analytics::{CampaignAnalytics, CampaignPhase};
use crate::limits::{EngineLimits, ResourceLimits, StopReason};

#[derive(Debug, thiserror::Error)]
pub enum CampaignError {
    #[error("IR parse error: {0}")]
    Parse(#[from] fresnel_fir_ir::parse::ParseError),

    #[error("Compilation error: {0}")]
    Compile(#[from] fresnel_fir_compiler::compile::CompileError),

    #[error("Campaign not found: {0}")]
    NotFound(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Limit exceeded: {0}")]
    LimitExceeded(String),
}

/// Budget estimates computed from IR complexity.
#[derive(Debug, Clone)]
pub struct Budget {
    pub min_iterations: u64,
    pub min_timeout_secs: u64,
}

/// State for a single campaign.
#[derive(Debug, Clone)]
pub struct CampaignState {
    pub id: String,
    pub compiled: CompiledIR,
    pub budget: Budget,
    pub resource_limits: ResourceLimits,
    pub phase: CampaignPhase,
    /// Total findings discovered so far.
    pub findings_count: u32,
    /// Total steps executed so far.
    pub steps_executed: u64,
    /// Coverage targets hit / total.
    pub coverage_hit: u32,
    pub coverage_total: u32,
    /// Stop reason (if finished).
    pub stop_reason: Option<StopReason>,
}

/// A finding record for MCP tool responses.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FindingRecord {
    pub id: u64,
    pub seqno: u64,
    pub finding_type: String,
    pub action: String,
    pub details: String,
    pub model_generation: u64,
}

/// Coverage target status.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CoverageTarget {
    pub target: String,
    pub status: String,
    pub hit_count: u64,
}

/// Manages all active campaigns.
pub struct CampaignManager {
    campaigns: Mutex<HashMap<String, CampaignState>>,
    findings: Mutex<HashMap<String, Vec<FindingRecord>>>,
    coverage: Mutex<HashMap<String, Vec<CoverageTarget>>>,
    analytics: Mutex<HashMap<String, CampaignAnalytics>>,
    next_id: Mutex<u64>,
    engine_limits: EngineLimits,
}

impl CampaignManager {
    pub fn new() -> Self {
        Self {
            campaigns: Mutex::new(HashMap::new()),
            findings: Mutex::new(HashMap::new()),
            coverage: Mutex::new(HashMap::new()),
            analytics: Mutex::new(HashMap::new()),
            next_id: Mutex::new(1),
            engine_limits: EngineLimits::default(),
        }
    }

    /// Compile IR JSON and create a new campaign.
    pub fn compile(&self, ir_json: &str) -> Result<String, CampaignError> {
        // Check engine limits.
        let campaign_count = self.campaigns.lock().unwrap().len();
        if campaign_count as u32 >= self.engine_limits.max_concurrent_campaigns {
            return Err(CampaignError::LimitExceeded(format!(
                "Too many concurrent campaigns ({}/{})",
                campaign_count, self.engine_limits.max_concurrent_campaigns
            )));
        }
        if ir_json.len() as u64 > self.engine_limits.max_ir_json_bytes {
            return Err(CampaignError::LimitExceeded(format!(
                "IR JSON too large ({} bytes, max {})",
                ir_json.len(),
                self.engine_limits.max_ir_json_bytes
            )));
        }

        let ir = parse_ir(ir_json)?;
        let compiled = compile(&ir)?;
        let budget = estimate_budget(&ir);

        let campaign_id = {
            let mut next = self.next_id.lock().unwrap();
            let id = format!("campaign-{:04}", *next);
            *next += 1;
            id
        };

        let state = CampaignState {
            id: campaign_id.clone(),
            compiled,
            budget,
            resource_limits: ResourceLimits::default(),
            phase: CampaignPhase::Compiled,
            findings_count: 0,
            steps_executed: 0,
            coverage_hit: 0,
            coverage_total: 0,
            stop_reason: None,
        };

        self.campaigns
            .lock()
            .unwrap()
            .insert(campaign_id.clone(), state);
        self.findings
            .lock()
            .unwrap()
            .insert(campaign_id.clone(), Vec::new());
        self.coverage
            .lock()
            .unwrap()
            .insert(campaign_id.clone(), Vec::new());
        self.analytics
            .lock()
            .unwrap()
            .insert(campaign_id.clone(), CampaignAnalytics::new());

        Ok(campaign_id)
    }

    /// Get a clone of a campaign's state.
    pub fn get_campaign(&self, id: &str) -> Option<CampaignState> {
        self.campaigns.lock().unwrap().get(id).cloned()
    }

    /// Number of active campaigns.
    pub fn active_campaign_count(&self) -> usize {
        self.campaigns.lock().unwrap().len()
    }

    /// Transition a campaign to a new phase.
    pub fn set_phase(&self, id: &str, phase: CampaignPhase) -> Result<(), CampaignError> {
        {
            let mut campaigns = self.campaigns.lock().unwrap();
            let state = campaigns
                .get_mut(id)
                .ok_or_else(|| CampaignError::NotFound(id.to_string()))?;
            state.phase = phase.clone();
        }
        // Lock released before acquiring analytics lock to prevent deadlock.
        if let Some(analytics) = self.analytics.lock().unwrap().get_mut(id) {
            analytics.state = phase;
        }
        Ok(())
    }

    /// Record a finding for a campaign.
    pub fn add_finding(&self, campaign_id: &str, finding: FindingRecord) {
        if let Some(findings) = self.findings.lock().unwrap().get_mut(campaign_id) {
            findings.push(finding);
        }
        if let Some(state) = self.campaigns.lock().unwrap().get_mut(campaign_id) {
            state.findings_count += 1;
        }
    }

    /// Get findings for a campaign, optionally since a sequence number.
    pub fn get_findings(&self, campaign_id: &str, since_seqno: Option<u64>) -> Vec<FindingRecord> {
        let findings = self.findings.lock().unwrap();
        match findings.get(campaign_id) {
            Some(list) => match since_seqno {
                Some(seqno) => list.iter().filter(|f| f.seqno > seqno).cloned().collect(),
                None => list.clone(),
            },
            None => Vec::new(),
        }
    }

    /// Update coverage data for a campaign.
    pub fn update_coverage(&self, campaign_id: &str, targets: Vec<CoverageTarget>) {
        let hit = targets.iter().filter(|t| t.status == "hit").count() as u32;
        let total = targets.len() as u32;

        if let Some(state) = self.campaigns.lock().unwrap().get_mut(campaign_id) {
            state.coverage_hit = hit;
            state.coverage_total = total;
        }
        if let Some(cov) = self.coverage.lock().unwrap().get_mut(campaign_id) {
            *cov = targets;
        }
    }

    /// Get coverage data for a campaign.
    pub fn get_coverage(&self, campaign_id: &str) -> Vec<CoverageTarget> {
        self.coverage
            .lock()
            .unwrap()
            .get(campaign_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Abort a campaign.
    pub fn abort(&self, campaign_id: &str) -> Result<CampaignState, CampaignError> {
        let result = {
            let mut campaigns = self.campaigns.lock().unwrap();
            let state = campaigns
                .get_mut(campaign_id)
                .ok_or_else(|| CampaignError::NotFound(campaign_id.to_string()))?;

            state.phase = CampaignPhase::Aborted;
            state.stop_reason = Some(StopReason::UserAborted);
            state.clone()
        };
        // Lock released before acquiring analytics lock to prevent deadlock.
        if let Some(analytics) = self.analytics.lock().unwrap().get_mut(campaign_id) {
            analytics.state = CampaignPhase::Aborted;
        }

        Ok(result)
    }

    /// Get analytics for a campaign.
    pub fn get_analytics(&self, campaign_id: &str) -> Option<CampaignAnalytics> {
        self.analytics.lock().unwrap().get(campaign_id).cloned()
    }

    /// Remove a completed/aborted campaign.
    pub fn remove_campaign(&self, campaign_id: &str) {
        self.campaigns.lock().unwrap().remove(campaign_id);
        self.findings.lock().unwrap().remove(campaign_id);
        self.coverage.lock().unwrap().remove(campaign_id);
        self.analytics.lock().unwrap().remove(campaign_id);
    }
}

impl Default for CampaignManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Estimate fuzzing budget from IR complexity.
fn estimate_budget(ir: &fresnel_fir_ir::types::FresnelFirIR) -> Budget {
    let entity_count = ir.entities.len() as u64;
    let protocol_count = ir.protocols.len() as u64;
    let effect_count = ir.effects.len() as u64;
    let input_domain_size: u64 = ir
        .inputs
        .domains
        .values()
        .map(|d| match &d.domain_type {
            fresnel_fir_ir::types::DomainType::Bool => 2u64,
            fresnel_fir_ir::types::DomainType::Enum { values } => values.len().max(1) as u64,
            fresnel_fir_ir::types::DomainType::Int { min, max } => {
                if max >= min {
                    ((max - min) as u64).saturating_add(1)
                } else {
                    1 // Invalid range, treat as single value
                }
            }
        })
        .try_fold(1u64, |acc, x| acc.checked_mul(x))
        .unwrap_or(u64::MAX)
        .max(1);
    let coverage_target_count = ir.inputs.coverage.targets.len() as u64;

    // min_iterations = entities * transitions * input_domains * coverage_targets
    // with a floor of 100 and cap on input_domain_size to prevent explosion
    let min_iterations = entity_count
        .saturating_mul(effect_count.max(1))
        .saturating_mul(protocol_count.max(1))
        .saturating_mul(input_domain_size.min(1000))
        .saturating_mul(coverage_target_count.max(1))
        .max(100);

    // Timeout: 1 second per 100 iterations, minimum 10 seconds
    let min_timeout_secs = (min_iterations / 100).max(10);

    Budget {
        min_iterations,
        min_timeout_secs,
    }
}
