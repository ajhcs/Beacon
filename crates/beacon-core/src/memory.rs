//! Cross-campaign learning memory.
//!
//! Persists learned knowledge across campaigns (same IR hash):
//! - Replay capsules (full finding reproduction state)
//! - Learned state-conditioned weights (with 0.8 decay per campaign)
//! - Hot regions with reproduction tracking
//! - Effective generator shortcuts
//!
//! Resets per campaign: model state, WASM instance, traversal traces, finding list.
//!
//! Re-regression priority on campaign start:
//! 1. Replay all previous finding capsules (confirm fixes, catch regressions)
//! 2. Explore hot regions with boosted weights
//! 3. Resume coverage-driven exploration

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A replay capsule — everything needed to reproduce a finding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayCapsule {
    /// Hash of the IR that produced this finding.
    pub ir_hash: String,
    /// Hash of the WASM module that was under test.
    pub wasm_hash: String,
    /// RNG seed used in the campaign.
    pub seed: u64,
    /// Description of the finding.
    pub finding_description: String,
    /// Action that triggered the finding.
    pub trigger_action: String,
    /// Step number in the traversal trace.
    pub trace_step: u64,
    /// Model generation at finding time.
    pub model_generation: u64,
    /// Input vector assignments (serialized).
    pub input_vector: HashMap<String, String>,
}

/// A hot region — a part of the search space that frequently produces findings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotRegion {
    /// Branch ID that leads to findings.
    pub branch_id: String,
    /// Model state hash when findings occur.
    pub model_state_hash: u64,
    /// Number of findings from this region.
    pub finding_count: u32,
    /// Boost factor for this region on campaign start.
    pub boost_factor: f64,
}

/// A learned weight entry (serializable).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnedWeight {
    pub branch_id: String,
    pub model_state_hash: u64,
    pub weight: f64,
}

/// Cross-campaign memory for a specific IR hash.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignMemory {
    /// IR hash this memory applies to.
    pub ir_hash: String,
    /// Replay capsules from previous campaigns.
    pub replay_capsules: Vec<ReplayCapsule>,
    /// Learned state-conditioned weights.
    pub learned_weights: Vec<LearnedWeight>,
    /// Hot regions that frequently produce findings.
    pub hot_regions: Vec<HotRegion>,
    /// Consecutive non-reproduction counts per capsule index.
    /// When this exceeds `invalidation_threshold`, aggressive decay applies.
    pub non_reproduction_counts: HashMap<usize, u32>,
    /// Number of campaigns run against this IR.
    pub campaign_count: u32,
}

/// Configuration for cross-campaign memory behavior.
#[derive(Debug, Clone)]
pub struct MemoryConfig {
    /// Weight decay factor applied to learned weights per campaign.
    /// Typical: 0.8 (gentle decay). Design doc: 0.8.
    pub cross_campaign_decay: f64,
    /// Aggressive decay for non-reproducing findings.
    /// Applied after `invalidation_threshold` consecutive failures.
    /// Design doc: 0.2.
    pub aggressive_decay: f64,
    /// Consecutive non-reproduction campaigns before aggressive decay.
    /// Design doc: 3.
    pub invalidation_threshold: u32,
    /// Boost factor for hot regions on campaign start.
    pub hot_region_boost: f64,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            cross_campaign_decay: 0.8,
            aggressive_decay: 0.2,
            invalidation_threshold: 3,
            hot_region_boost: 2.0,
        }
    }
}

impl CampaignMemory {
    /// Create empty memory for a new IR.
    pub fn new(ir_hash: String) -> Self {
        Self {
            ir_hash,
            replay_capsules: Vec::new(),
            learned_weights: Vec::new(),
            hot_regions: Vec::new(),
            non_reproduction_counts: HashMap::new(),
            campaign_count: 0,
        }
    }

    /// Record a finding's replay capsule.
    pub fn add_capsule(&mut self, capsule: ReplayCapsule) {
        self.replay_capsules.push(capsule);
    }

    /// Record a hot region.
    pub fn add_hot_region(&mut self, region: HotRegion) {
        // Merge with existing if same branch + state.
        if let Some(existing) = self.hot_regions.iter_mut().find(|r| {
            r.branch_id == region.branch_id && r.model_state_hash == region.model_state_hash
        }) {
            existing.finding_count += region.finding_count;
            existing.boost_factor = existing.boost_factor.max(region.boost_factor);
        } else {
            self.hot_regions.push(region);
        }
    }

    /// Save current weight table state as learned weights.
    pub fn save_learned_weights(&mut self, weights: Vec<LearnedWeight>) {
        self.learned_weights = weights;
    }

    /// Prepare for a new campaign: apply cross-campaign decay to weights,
    /// apply invalidation to non-reproducing capsules, increment campaign count.
    pub fn prepare_new_campaign(&mut self, config: &MemoryConfig) {
        self.campaign_count += 1;

        // Decay learned weights.
        for w in &mut self.learned_weights {
            w.weight *= config.cross_campaign_decay;
        }

        // Decay hot region boosts.
        for r in &mut self.hot_regions {
            r.boost_factor *= config.cross_campaign_decay;
        }

        // Apply invalidation for non-reproducing capsules.
        for (idx, count) in &self.non_reproduction_counts {
            if *count >= config.invalidation_threshold {
                // Aggressive decay on weights associated with this capsule's region.
                if let Some(capsule) = self.replay_capsules.get(*idx) {
                    let trigger = &capsule.trigger_action;
                    for w in &mut self.learned_weights {
                        if w.branch_id == *trigger {
                            w.weight *= config.aggressive_decay;
                        }
                    }
                }
            }
        }
    }

    /// Record that a capsule was replayed successfully (finding reproduced).
    pub fn record_reproduction(&mut self, capsule_index: usize) {
        self.non_reproduction_counts.remove(&capsule_index);
    }

    /// Record that a capsule replay failed to reproduce the finding.
    pub fn record_non_reproduction(&mut self, capsule_index: usize) {
        *self
            .non_reproduction_counts
            .entry(capsule_index)
            .or_insert(0) += 1;
    }

    /// Get capsules ordered for re-regression (replay) on campaign start.
    /// Returns (capsule_index, capsule) pairs.
    pub fn regression_order(&self) -> Vec<(usize, &ReplayCapsule)> {
        let mut indexed: Vec<(usize, &ReplayCapsule)> =
            self.replay_capsules.iter().enumerate().collect();

        // Sort by: non-reproduction count (ascending — most reliable first),
        // then by finding severity (we use trigger_action as proxy).
        indexed.sort_by(|(idx_a, _), (idx_b, _)| {
            let count_a = self.non_reproduction_counts.get(idx_a).unwrap_or(&0);
            let count_b = self.non_reproduction_counts.get(idx_b).unwrap_or(&0);
            count_a.cmp(count_b)
        });

        indexed
    }

    /// Get hot regions ordered by finding frequency (descending).
    pub fn hot_region_order(&self) -> Vec<&HotRegion> {
        let mut regions: Vec<&HotRegion> = self.hot_regions.iter().collect();
        regions.sort_by(|a, b| b.finding_count.cmp(&a.finding_count));
        regions
    }

    /// Serialize memory to JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize memory from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_capsule(action: &str) -> ReplayCapsule {
        ReplayCapsule {
            ir_hash: "abc123".into(),
            wasm_hash: "def456".into(),
            seed: 42,
            finding_description: format!("crash in {action}"),
            trigger_action: action.into(),
            trace_step: 10,
            model_generation: 5,
            input_vector: HashMap::new(),
        }
    }

    #[test]
    fn test_new_memory_is_empty() {
        let mem = CampaignMemory::new("hash".into());
        assert!(mem.replay_capsules.is_empty());
        assert!(mem.learned_weights.is_empty());
        assert!(mem.hot_regions.is_empty());
        assert_eq!(mem.campaign_count, 0);
    }

    #[test]
    fn test_add_and_retrieve_capsules() {
        let mut mem = CampaignMemory::new("hash".into());
        mem.add_capsule(make_capsule("fn_a"));
        mem.add_capsule(make_capsule("fn_b"));

        assert_eq!(mem.replay_capsules.len(), 2);
        assert_eq!(mem.replay_capsules[0].trigger_action, "fn_a");
    }

    #[test]
    fn test_cross_campaign_decay() {
        let mut mem = CampaignMemory::new("hash".into());
        mem.learned_weights.push(LearnedWeight {
            branch_id: "b1".into(),
            model_state_hash: 0,
            weight: 100.0,
        });

        mem.prepare_new_campaign(&MemoryConfig::default());

        assert!((mem.learned_weights[0].weight - 80.0).abs() < 0.01);
        assert_eq!(mem.campaign_count, 1);
    }

    #[test]
    fn test_invalidation_after_threshold() {
        let config = MemoryConfig {
            invalidation_threshold: 2,
            aggressive_decay: 0.2,
            ..Default::default()
        };

        let mut mem = CampaignMemory::new("hash".into());
        mem.add_capsule(make_capsule("buggy_fn"));
        mem.learned_weights.push(LearnedWeight {
            branch_id: "buggy_fn".into(),
            model_state_hash: 0,
            weight: 100.0,
        });

        // Two consecutive non-reproductions.
        mem.record_non_reproduction(0);
        mem.record_non_reproduction(0);
        assert_eq!(*mem.non_reproduction_counts.get(&0).unwrap(), 2);

        // Prepare new campaign — should apply aggressive decay.
        mem.prepare_new_campaign(&config);

        // Weight should be 100 * 0.8 (cross-campaign) * 0.2 (aggressive) = 16
        assert!(mem.learned_weights[0].weight < 20.0);
    }

    #[test]
    fn test_reproduction_clears_non_reproduction_count() {
        let mut mem = CampaignMemory::new("hash".into());
        mem.add_capsule(make_capsule("fn_a"));

        mem.record_non_reproduction(0);
        mem.record_non_reproduction(0);
        assert_eq!(*mem.non_reproduction_counts.get(&0).unwrap(), 2);

        mem.record_reproduction(0);
        assert!(!mem.non_reproduction_counts.contains_key(&0));
    }

    #[test]
    fn test_hot_region_merging() {
        let mut mem = CampaignMemory::new("hash".into());

        mem.add_hot_region(HotRegion {
            branch_id: "b1".into(),
            model_state_hash: 42,
            finding_count: 3,
            boost_factor: 1.5,
        });
        mem.add_hot_region(HotRegion {
            branch_id: "b1".into(),
            model_state_hash: 42,
            finding_count: 2,
            boost_factor: 2.0,
        });

        assert_eq!(mem.hot_regions.len(), 1);
        assert_eq!(mem.hot_regions[0].finding_count, 5);
        assert_eq!(mem.hot_regions[0].boost_factor, 2.0);
    }

    #[test]
    fn test_regression_order_most_reliable_first() {
        let mut mem = CampaignMemory::new("hash".into());
        mem.add_capsule(make_capsule("unreliable"));
        mem.add_capsule(make_capsule("reliable"));

        mem.record_non_reproduction(0);
        mem.record_non_reproduction(0);
        // capsule 1 has no non-reproductions

        let order = mem.regression_order();
        // Reliable (0 failures) should come first.
        assert_eq!(order[0].1.trigger_action, "reliable");
        assert_eq!(order[1].1.trigger_action, "unreliable");
    }

    #[test]
    fn test_hot_region_order_by_frequency() {
        let mut mem = CampaignMemory::new("hash".into());

        mem.add_hot_region(HotRegion {
            branch_id: "low".into(),
            model_state_hash: 0,
            finding_count: 1,
            boost_factor: 1.0,
        });
        mem.add_hot_region(HotRegion {
            branch_id: "high".into(),
            model_state_hash: 0,
            finding_count: 10,
            boost_factor: 1.0,
        });

        let order = mem.hot_region_order();
        assert_eq!(order[0].branch_id, "high");
        assert_eq!(order[1].branch_id, "low");
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut mem = CampaignMemory::new("hash123".into());
        mem.add_capsule(make_capsule("fn_a"));
        mem.learned_weights.push(LearnedWeight {
            branch_id: "b1".into(),
            model_state_hash: 42,
            weight: 75.0,
        });
        mem.add_hot_region(HotRegion {
            branch_id: "hot".into(),
            model_state_hash: 0,
            finding_count: 5,
            boost_factor: 2.0,
        });
        mem.campaign_count = 3;

        let json = mem.to_json().unwrap();
        let restored = CampaignMemory::from_json(&json).unwrap();

        assert_eq!(restored.ir_hash, "hash123");
        assert_eq!(restored.replay_capsules.len(), 1);
        assert_eq!(restored.learned_weights.len(), 1);
        assert_eq!(restored.hot_regions.len(), 1);
        assert_eq!(restored.campaign_count, 3);
        assert!((restored.learned_weights[0].weight - 75.0).abs() < 0.01);
    }

    #[test]
    fn test_multiple_campaign_decay_compounds() {
        let config = MemoryConfig::default();
        let mut mem = CampaignMemory::new("hash".into());
        mem.learned_weights.push(LearnedWeight {
            branch_id: "b1".into(),
            model_state_hash: 0,
            weight: 100.0,
        });

        // Run 3 campaigns: 100 * 0.8^3 = 51.2
        mem.prepare_new_campaign(&config);
        mem.prepare_new_campaign(&config);
        mem.prepare_new_campaign(&config);

        assert!((mem.learned_weights[0].weight - 51.2).abs() < 0.1);
        assert_eq!(mem.campaign_count, 3);
    }
}
