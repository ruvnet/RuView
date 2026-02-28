//! # Reputation Bonding Curves
//!
//! Economic mechanisms for reputation-based pricing and allocation.
//! Implements bonding curves that reward high-reputation nodes with:
//!
//! - **Price Discounts**: Up to 20% discount for high-reputation nodes
//! - **Priority Allocation**: Superlinear advantage for task allocation
//! - **Stake Requirements**: Bonding curve for reputation-stake relationship
//!
//! ## Bonding Curve Model
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                   REPUTATION BONDING CURVE                       │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                                                                 │
//! │  Discount │                            ╭────────────────────    │
//! │    20% ───┤                         ╭──╯                        │
//! │           │                      ╭──╯                           │
//! │    15% ───┤                   ╭──╯                              │
//! │           │                ╭──╯                                 │
//! │    10% ───┤             ╭──╯                                    │
//! │           │          ╭──╯                                       │
//! │     5% ───┤       ╭──╯                                          │
//! │           │    ╭──╯                                             │
//! │     0% ───┴────╯────┬────┬────┬────┬────┬────┬────┬────┬────    │
//! │           0    10   20   30   40   50   60   70   80   90  100  │
//! │                          Reputation Score                       │
//! │                                                                 │
//! │  Curve: discount = (reputation/100)^1.5 * 0.2                   │
//! │                                                                 │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Task Allocation Priority
//!
//! Higher reputation nodes get superlinear advantage in task allocation:
//! - Reputation 50: weight = 50^1.5 = 353
//! - Reputation 100: weight = 100^1.5 = 1000
//!
//! This creates strong incentives for maintaining good behavior.

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use std::sync::RwLock;
use rustc_hash::FxHashMap;

/// Default base price for stake calculations
pub const DEFAULT_BASE_PRICE: u64 = 100;

/// Default curve exponent for moderate bonding
pub const DEFAULT_CURVE_EXPONENT: f32 = 1.5;

/// Maximum discount percentage (20%)
pub const MAX_DISCOUNT: f32 = 0.20;

/// Reputation tier thresholds
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ReputationTier {
    /// New or low reputation (0-25)
    Bronze,
    /// Moderate reputation (25-50)
    Silver,
    /// Good reputation (50-75)
    Gold,
    /// Excellent reputation (75-100)
    Platinum,
}

impl ReputationTier {
    /// Get tier from reputation score
    pub fn from_score(reputation: f32) -> Self {
        match reputation {
            r if r >= 75.0 => ReputationTier::Platinum,
            r if r >= 50.0 => ReputationTier::Gold,
            r if r >= 25.0 => ReputationTier::Silver,
            _ => ReputationTier::Bronze,
        }
    }

    /// Get tier name
    pub fn name(&self) -> &str {
        match self {
            ReputationTier::Bronze => "Bronze",
            ReputationTier::Silver => "Silver",
            ReputationTier::Gold => "Gold",
            ReputationTier::Platinum => "Platinum",
        }
    }

    /// Get tier multiplier for rewards
    pub fn reward_multiplier(&self) -> f32 {
        match self {
            ReputationTier::Bronze => 1.0,
            ReputationTier::Silver => 1.1,
            ReputationTier::Gold => 1.25,
            ReputationTier::Platinum => 1.5,
        }
    }
}

/// Reputation bonding curve configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReputationCurveConfig {
    /// Base price for stake calculations
    pub base_price: u64,
    /// Curve exponent (1.5 for moderate bonding)
    pub curve_exponent: f32,
    /// Maximum discount percentage (0.0 - 1.0)
    pub max_discount: f32,
    /// Minimum reputation to participate
    pub min_reputation: f32,
    /// Decay rate per epoch (0.0 - 1.0)
    pub decay_rate: f32,
}

impl Default for ReputationCurveConfig {
    fn default() -> Self {
        Self {
            base_price: DEFAULT_BASE_PRICE,
            curve_exponent: DEFAULT_CURVE_EXPONENT,
            max_discount: MAX_DISCOUNT,
            min_reputation: 10.0,
            decay_rate: 0.01, // 1% decay per epoch
        }
    }
}

/// Node reputation record
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeReputation {
    /// Node ID
    pub node_id: String,
    /// Current reputation score (0-100)
    pub reputation: f32,
    /// Total tasks completed
    pub tasks_completed: u64,
    /// Successful tasks
    pub tasks_successful: u64,
    /// Total compute contributed (seconds)
    pub compute_contributed: u64,
    /// Total stake locked
    pub stake_locked: u64,
    /// Last update timestamp
    pub last_updated: u64,
    /// Reputation tier
    pub tier: ReputationTier,
}

impl NodeReputation {
    /// Calculate success rate
    pub fn success_rate(&self) -> f32 {
        if self.tasks_completed == 0 {
            return 0.0;
        }
        self.tasks_successful as f32 / self.tasks_completed as f32
    }
}

/// Reputation bonding curve for economic incentives
#[wasm_bindgen]
pub struct ReputationCurve {
    /// Configuration
    config: ReputationCurveConfig,
    /// Node reputations
    reputations: RwLock<FxHashMap<String, NodeReputation>>,
    /// Epoch counter for decay
    epoch: RwLock<u64>,
}

#[wasm_bindgen]
impl ReputationCurve {
    /// Create a new reputation curve with default configuration
    #[wasm_bindgen(constructor)]
    pub fn new() -> ReputationCurve {
        ReputationCurve {
            config: ReputationCurveConfig::default(),
            reputations: RwLock::new(FxHashMap::default()),
            epoch: RwLock::new(0),
        }
    }

    /// Create with custom base price and exponent
    #[wasm_bindgen(js_name = withConfig)]
    pub fn with_config(base_price: u64, curve_exponent: f32) -> ReputationCurve {
        ReputationCurve {
            config: ReputationCurveConfig {
                base_price,
                curve_exponent,
                ..Default::default()
            },
            reputations: RwLock::new(FxHashMap::default()),
            epoch: RwLock::new(0),
        }
    }

    /// Calculate discount for a given reputation score
    /// Returns a multiplier (0.8 = 20% discount, 1.0 = no discount)
    #[wasm_bindgen]
    pub fn discount(&self, reputation: f32) -> f32 {
        let normalized = (reputation / 100.0).clamp(0.0, 1.0);
        let discount_amount = normalized.powf(self.config.curve_exponent) * self.config.max_discount;
        1.0 - discount_amount
    }

    /// Calculate absolute discount amount for a given price
    #[wasm_bindgen(js_name = discountAmount)]
    pub fn discount_amount(&self, base_price: u64, reputation: f32) -> u64 {
        let discount_rate = 1.0 - self.discount(reputation);
        (base_price as f32 * discount_rate) as u64
    }

    /// Calculate final price after reputation discount
    #[wasm_bindgen(js_name = finalPrice)]
    pub fn final_price(&self, base_price: u64, reputation: f32) -> u64 {
        let multiplier = self.discount(reputation);
        (base_price as f32 * multiplier) as u64
    }

    /// Reputation-weighted task allocation priority
    /// Returns a weight for weighted random selection
    #[wasm_bindgen(js_name = allocationWeight)]
    pub fn allocation_weight(&self, reputation: f32) -> f32 {
        if reputation <= 0.0 {
            return 0.0;
        }
        // Superlinear advantage for high-reputation nodes
        reputation.powf(self.config.curve_exponent)
    }

    /// Stake required to achieve a target reputation level
    #[wasm_bindgen(js_name = stakeForReputation)]
    pub fn stake_for_reputation(&self, target_rep: f32) -> u64 {
        if target_rep <= 0.0 {
            return 0;
        }
        // Bonding curve: stake = base * rep^exponent
        (self.config.base_price as f32 * target_rep.powf(self.config.curve_exponent)) as u64
    }

    /// Calculate reputation from current stake (inverse of stake_for_reputation)
    #[wasm_bindgen(js_name = reputationFromStake)]
    pub fn reputation_from_stake(&self, stake: u64) -> f32 {
        if stake == 0 || self.config.base_price == 0 {
            return 0.0;
        }
        // Inverse: rep = (stake / base)^(1/exponent)
        let ratio = stake as f32 / self.config.base_price as f32;
        ratio.powf(1.0 / self.config.curve_exponent).min(100.0)
    }

    /// Get reputation tier for a score
    #[wasm_bindgen(js_name = getTier)]
    pub fn get_tier(&self, reputation: f32) -> String {
        ReputationTier::from_score(reputation).name().to_string()
    }

    /// Get reward multiplier for a tier
    #[wasm_bindgen(js_name = getRewardMultiplier)]
    pub fn get_reward_multiplier(&self, reputation: f32) -> f32 {
        ReputationTier::from_score(reputation).reward_multiplier()
    }

    /// Get node count
    #[wasm_bindgen(js_name = getNodeCount)]
    pub fn get_node_count(&self) -> usize {
        self.reputations.read().unwrap().len()
    }

    /// Get average reputation
    #[wasm_bindgen(js_name = getAverageReputation)]
    pub fn get_average_reputation(&self) -> f32 {
        let reps = self.reputations.read().unwrap();
        if reps.is_empty() {
            return 0.0;
        }
        let total: f32 = reps.values().map(|r| r.reputation).sum();
        total / reps.len() as f32
    }

    /// Get reputation for a specific node
    #[wasm_bindgen(js_name = getReputation)]
    pub fn get_reputation(&self, node_id: &str) -> f32 {
        self.reputations.read().unwrap()
            .get(node_id)
            .map(|r| r.reputation)
            .unwrap_or(0.0)
    }

    /// Get current epoch
    #[wasm_bindgen(js_name = getEpoch)]
    pub fn get_epoch(&self) -> u64 {
        *self.epoch.read().unwrap()
    }

    /// Get tier distribution as JSON
    #[wasm_bindgen(js_name = getTierDistribution)]
    pub fn get_tier_distribution(&self) -> String {
        let reps = self.reputations.read().unwrap();
        let mut bronze = 0;
        let mut silver = 0;
        let mut gold = 0;
        let mut platinum = 0;

        for rep in reps.values() {
            match rep.tier {
                ReputationTier::Bronze => bronze += 1,
                ReputationTier::Silver => silver += 1,
                ReputationTier::Gold => gold += 1,
                ReputationTier::Platinum => platinum += 1,
            }
        }

        let dist = serde_json::json!({
            "bronze": bronze,
            "silver": silver,
            "gold": gold,
            "platinum": platinum,
            "total": reps.len(),
        });
        serde_json::to_string(&dist).unwrap_or_else(|_| "{}".to_string())
    }

    /// Get curve configuration as JSON
    #[wasm_bindgen(js_name = getConfig)]
    pub fn get_config(&self) -> String {
        serde_json::to_string(&self.config).unwrap_or_else(|_| "{}".to_string())
    }
}

impl ReputationCurve {
    /// Register a new node with initial reputation
    pub fn register_node(&self, node_id: &str, initial_stake: u64) {
        let now = js_sys::Date::now() as u64;
        let initial_rep = self.reputation_from_stake(initial_stake).min(50.0); // Cap initial rep

        let mut reps = self.reputations.write().unwrap();
        reps.entry(node_id.to_string()).or_insert(NodeReputation {
            node_id: node_id.to_string(),
            reputation: initial_rep,
            tasks_completed: 0,
            tasks_successful: 0,
            compute_contributed: 0,
            stake_locked: initial_stake,
            last_updated: now,
            tier: ReputationTier::from_score(initial_rep),
        });
    }

    /// Record task completion and update reputation
    pub fn record_task(&self, node_id: &str, success: bool, compute_seconds: u64) {
        let now = js_sys::Date::now() as u64;
        let mut reps = self.reputations.write().unwrap();

        if let Some(rep) = reps.get_mut(node_id) {
            rep.tasks_completed += 1;
            rep.compute_contributed += compute_seconds;
            rep.last_updated = now;

            if success {
                rep.tasks_successful += 1;
                // Increase reputation for success (diminishing returns)
                let increase = (1.0 / (1.0 + rep.reputation / 50.0)).max(0.1);
                rep.reputation = (rep.reputation + increase).min(100.0);
            } else {
                // Decrease reputation for failure
                let decrease = 2.0; // Failures hurt more than successes help
                rep.reputation = (rep.reputation - decrease).max(0.0);
            }

            rep.tier = ReputationTier::from_score(rep.reputation);
        }
    }

    /// Update stake for a node
    pub fn update_stake(&self, node_id: &str, new_stake: u64) {
        let now = js_sys::Date::now() as u64;
        let mut reps = self.reputations.write().unwrap();

        if let Some(rep) = reps.get_mut(node_id) {
            rep.stake_locked = new_stake;
            rep.last_updated = now;
        }
    }

    /// Apply decay to all reputations (call once per epoch)
    pub fn apply_decay(&self) {
        let mut epoch = self.epoch.write().unwrap();
        *epoch += 1;

        let mut reps = self.reputations.write().unwrap();
        let decay_factor = 1.0 - self.config.decay_rate;

        for rep in reps.values_mut() {
            // Apply decay
            rep.reputation *= decay_factor;

            // Minimum reputation from stake
            let stake_rep = self.reputation_from_stake(rep.stake_locked);
            rep.reputation = rep.reputation.max(stake_rep * 0.5); // Stake provides floor

            rep.tier = ReputationTier::from_score(rep.reputation);
        }
    }

    /// Get node reputation record
    pub fn get_node_reputation(&self, node_id: &str) -> Option<NodeReputation> {
        self.reputations.read().unwrap().get(node_id).cloned()
    }

    /// Get top nodes by reputation
    pub fn get_top_nodes(&self, limit: usize) -> Vec<NodeReputation> {
        let reps = self.reputations.read().unwrap();
        let mut nodes: Vec<_> = reps.values().cloned().collect();
        nodes.sort_by(|a, b| b.reputation.partial_cmp(&a.reputation).unwrap());
        nodes.into_iter().take(limit).collect()
    }

    /// Select nodes for task allocation using weighted random selection
    pub fn select_nodes_for_task(&self, count: usize, excluded: &[String]) -> Vec<String> {
        let reps = self.reputations.read().unwrap();

        // Filter eligible nodes and calculate weights
        let eligible: Vec<_> = reps.values()
            .filter(|r| {
                r.reputation >= self.config.min_reputation
                    && !excluded.contains(&r.node_id)
            })
            .collect();

        if eligible.is_empty() {
            return Vec::new();
        }

        // Calculate total weight
        let total_weight: f32 = eligible.iter()
            .map(|r| self.allocation_weight(r.reputation))
            .sum();

        if total_weight <= 0.0 {
            return Vec::new();
        }

        // Simple proportional selection (not true weighted random for simplicity)
        let mut selected: Vec<_> = eligible.iter()
            .map(|r| (r.node_id.clone(), self.allocation_weight(r.reputation) / total_weight))
            .collect();

        selected.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        selected.into_iter().take(count).map(|(id, _)| id).collect()
    }

    /// Slash reputation for misbehavior
    pub fn slash_reputation(&self, node_id: &str, amount: f32, reason: &str) {
        let now = js_sys::Date::now() as u64;
        let mut reps = self.reputations.write().unwrap();

        if let Some(rep) = reps.get_mut(node_id) {
            rep.reputation = (rep.reputation - amount).max(0.0);
            rep.last_updated = now;
            rep.tier = ReputationTier::from_score(rep.reputation);
        }
    }

    /// Prune inactive nodes with zero reputation
    pub fn prune_inactive(&self) {
        let mut reps = self.reputations.write().unwrap();
        reps.retain(|_, r| r.reputation > 0.1 || r.stake_locked > 0);
    }
}

impl Default for ReputationCurve {
    fn default() -> Self {
        Self::new()
    }
}

/// Combined reputation and pricing engine
#[wasm_bindgen]
pub struct ReputationPricing {
    curve: ReputationCurve,
}

#[wasm_bindgen]
impl ReputationPricing {
    /// Create a new reputation pricing engine
    #[wasm_bindgen(constructor)]
    pub fn new() -> ReputationPricing {
        ReputationPricing {
            curve: ReputationCurve::new(),
        }
    }

    /// Calculate task price for a node based on reputation
    #[wasm_bindgen(js_name = calculateTaskPrice)]
    pub fn calculate_task_price(&self, base_price: u64, node_id: &str) -> u64 {
        let reputation = self.curve.get_reputation(node_id);
        self.curve.final_price(base_price, reputation)
    }

    /// Get priority score for task allocation
    #[wasm_bindgen(js_name = getPriorityScore)]
    pub fn get_priority_score(&self, node_id: &str) -> f32 {
        let reputation = self.curve.get_reputation(node_id);
        self.curve.allocation_weight(reputation)
    }

    /// Get minimum stake for target reputation
    #[wasm_bindgen(js_name = getMinimumStake)]
    pub fn get_minimum_stake(&self, target_reputation: f32) -> u64 {
        self.curve.stake_for_reputation(target_reputation)
    }
}

impl Default for ReputationPricing {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discount_calculation() {
        let curve = ReputationCurve::new();

        // Zero reputation = no discount
        let discount = curve.discount(0.0);
        assert!((discount - 1.0).abs() < 0.001);

        // Max reputation = max discount
        let discount = curve.discount(100.0);
        assert!((discount - 0.8).abs() < 0.01); // 20% discount = 0.8 multiplier

        // Mid reputation
        let discount = curve.discount(50.0);
        assert!(discount > 0.8 && discount < 1.0);
    }

    #[test]
    fn test_allocation_weight() {
        let curve = ReputationCurve::new();

        // Superlinear: higher rep = disproportionately higher weight
        let weight_50 = curve.allocation_weight(50.0);
        let weight_100 = curve.allocation_weight(100.0);

        // weight_100 should be more than 2x weight_50 (superlinear)
        assert!(weight_100 > weight_50 * 2.0);
    }

    #[test]
    fn test_stake_reputation_relationship() {
        let curve = ReputationCurve::new();

        // Stake for reputation 50
        let stake_50 = curve.stake_for_reputation(50.0);

        // Reputation from that stake should be 50
        let rep = curve.reputation_from_stake(stake_50);
        assert!((rep - 50.0).abs() < 1.0);
    }

    #[test]
    fn test_reputation_tiers() {
        assert_eq!(ReputationTier::from_score(10.0), ReputationTier::Bronze);
        assert_eq!(ReputationTier::from_score(30.0), ReputationTier::Silver);
        assert_eq!(ReputationTier::from_score(60.0), ReputationTier::Gold);
        assert_eq!(ReputationTier::from_score(80.0), ReputationTier::Platinum);
    }

    #[test]
    fn test_final_price() {
        let curve = ReputationCurve::new();

        // Base price 1000, high reputation
        let price = curve.final_price(1000, 100.0);
        assert_eq!(price, 800); // 20% discount

        // Base price 1000, zero reputation
        let price = curve.final_price(1000, 0.0);
        assert_eq!(price, 1000); // No discount
    }

    #[test]
    fn test_reward_multiplier() {
        let curve = ReputationCurve::new();

        assert_eq!(curve.get_reward_multiplier(10.0), 1.0);   // Bronze
        assert_eq!(curve.get_reward_multiplier(30.0), 1.1);   // Silver
        assert_eq!(curve.get_reward_multiplier(60.0), 1.25);  // Gold
        assert_eq!(curve.get_reward_multiplier(90.0), 1.5);   // Platinum
    }
}
