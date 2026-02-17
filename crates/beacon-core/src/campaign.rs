use std::collections::HashMap;
use std::sync::Mutex;

use beacon_compiler::compile;
use beacon_compiler::compile::CompiledIR;
use beacon_ir::parse::parse_ir;

#[derive(Debug, thiserror::Error)]
pub enum CampaignError {
    #[error("IR parse error: {0}")]
    Parse(#[from] beacon_ir::parse::ParseError),

    #[error("Compilation error: {0}")]
    Compile(#[from] beacon_compiler::compile::CompileError),
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
}

/// Manages all active campaigns.
pub struct CampaignManager {
    campaigns: Mutex<HashMap<String, CampaignState>>,
    next_id: Mutex<u64>,
}

impl CampaignManager {
    pub fn new() -> Self {
        Self {
            campaigns: Mutex::new(HashMap::new()),
            next_id: Mutex::new(1),
        }
    }

    /// Compile IR JSON and create a new campaign.
    pub fn compile(&self, ir_json: &str) -> Result<String, CampaignError> {
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
        };

        self.campaigns
            .lock()
            .unwrap()
            .insert(campaign_id.clone(), state);

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
}

impl Default for CampaignManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Estimate fuzzing budget from IR complexity.
fn estimate_budget(ir: &beacon_ir::types::BeaconIR) -> Budget {
    let entity_count = ir.entities.len() as u64;
    let protocol_count = ir.protocols.len() as u64;
    let effect_count = ir.effects.len() as u64;
    let input_domain_size: u64 = ir
        .inputs
        .domains
        .values()
        .map(|d| match &d.domain_type {
            beacon_ir::types::DomainType::Bool => 2,
            beacon_ir::types::DomainType::Enum { values } => values.len() as u64,
            beacon_ir::types::DomainType::Int { min, max } => (max - min + 1) as u64,
        })
        .product::<u64>()
        .max(1);
    let coverage_target_count = ir.inputs.coverage.targets.len() as u64;

    // min_iterations = entities * transitions * input_domains * coverage_targets
    // with a floor of 100
    let min_iterations = (entity_count
        * effect_count.max(1)
        * protocol_count.max(1)
        * input_domain_size.min(1000) // cap to prevent explosion
        * coverage_target_count.max(1))
    .max(100);

    // Timeout: 1 second per 100 iterations, minimum 10 seconds
    let min_timeout_secs = (min_iterations / 100).max(10);

    Budget {
        min_iterations,
        min_timeout_secs,
    }
}
