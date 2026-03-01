//! Stake/Slash Mechanics
//!
//! Implements participation requirements and penalty system:
//! - Minimum stake to participate in network
//! - Slash conditions for bad behavior
//! - Stake delegation support
//! - Lock periods for stability

use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// Get current timestamp in milliseconds (works in both WASM and native)
fn current_timestamp_ms() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now() as u64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

/// Reasons for slashing stake
#[wasm_bindgen]
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum SlashReason {
    /// Invalid task result
    InvalidResult = 0,
    /// Double-spending attempt
    DoubleSpend = 1,
    /// Sybil attack detected
    SybilAttack = 2,
    /// Excessive downtime
    Downtime = 3,
    /// Spam/flooding
    Spam = 4,
    /// Malicious behavior
    Malicious = 5,
}

impl SlashReason {
    /// Get slash percentage for this reason
    pub fn slash_percentage(&self) -> f32 {
        match self {
            SlashReason::InvalidResult => 0.05, // 5% for errors
            SlashReason::DoubleSpend => 1.0,    // 100% for fraud
            SlashReason::SybilAttack => 0.5,    // 50% for sybil
            SlashReason::Downtime => 0.01,      // 1% for downtime
            SlashReason::Spam => 0.1,           // 10% for spam
            SlashReason::Malicious => 0.75,     // 75% for malicious
        }
    }
}

/// Stake entry for a node
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct StakeEntry {
    /// Staked amount
    pub amount: u64,
    /// Lock timestamp (Unix ms) - cannot unstake before this
    pub locked_until: u64,
    /// Delegated stake (from other nodes)
    pub delegated: u64,
    /// Nodes that delegated to this one
    pub delegators: Vec<String>,
    /// Slash history
    pub slashes: Vec<SlashEvent>,
}

/// Record of a slash event
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SlashEvent {
    /// Amount slashed
    pub amount: u64,
    /// Reason for slash
    pub reason: SlashReason,
    /// Timestamp
    pub timestamp: u64,
    /// Evidence (task ID, etc.)
    pub evidence: String,
}

/// Stake manager for the network
#[wasm_bindgen]
pub struct StakeManager {
    /// Stakes by node ID
    stakes: FxHashMap<String, StakeEntry>,
    /// Minimum stake to participate
    min_stake: u64,
    /// Default lock period in milliseconds
    default_lock_period: u64,
    /// Total staked across network
    total_staked: u64,
    /// Total slashed
    total_slashed: u64,
}

#[wasm_bindgen]
impl StakeManager {
    /// Create a new stake manager
    #[wasm_bindgen(constructor)]
    pub fn new() -> StakeManager {
        StakeManager {
            stakes: FxHashMap::default(),
            min_stake: 100,                  // 100 credits minimum
            default_lock_period: 86_400_000, // 24 hours in ms
            total_staked: 0,
            total_slashed: 0,
        }
    }

    /// Create with custom parameters
    #[wasm_bindgen(js_name = newWithParams)]
    pub fn new_with_params(min_stake: u64, lock_period_ms: u64) -> StakeManager {
        StakeManager {
            stakes: FxHashMap::default(),
            min_stake,
            default_lock_period: lock_period_ms,
            total_staked: 0,
            total_slashed: 0,
        }
    }

    /// Get minimum stake requirement
    #[wasm_bindgen(js_name = minStake)]
    pub fn min_stake(&self) -> u64 {
        self.min_stake
    }

    /// Get total network staked
    #[wasm_bindgen(js_name = totalStaked)]
    pub fn total_staked(&self) -> u64 {
        self.total_staked
    }

    /// Get total slashed
    #[wasm_bindgen(js_name = totalSlashed)]
    pub fn total_slashed(&self) -> u64 {
        self.total_slashed
    }

    /// Get stake for a node
    #[wasm_bindgen(js_name = getStake)]
    pub fn get_stake(&self, node_id: &str) -> u64 {
        self.stakes.get(node_id).map(|s| s.amount).unwrap_or(0)
    }

    /// Get effective stake (own + delegated)
    #[wasm_bindgen(js_name = getEffectiveStake)]
    pub fn get_effective_stake(&self, node_id: &str) -> u64 {
        self.stakes
            .get(node_id)
            .map(|s| s.amount + s.delegated)
            .unwrap_or(0)
    }

    /// Check if node meets minimum stake
    #[wasm_bindgen(js_name = meetsMinimum)]
    pub fn meets_minimum(&self, node_id: &str) -> bool {
        self.get_effective_stake(node_id) >= self.min_stake
    }

    /// Stake credits for a node
    #[wasm_bindgen]
    pub fn stake(&mut self, node_id: &str, amount: u64) -> Result<(), JsValue> {
        if amount == 0 {
            return Err(JsValue::from_str("Amount must be positive"));
        }

        let now = current_timestamp_ms();
        let locked_until = now + self.default_lock_period;

        let entry = self
            .stakes
            .entry(node_id.to_string())
            .or_insert_with(|| StakeEntry {
                amount: 0,
                locked_until: 0,
                delegated: 0,
                delegators: Vec::new(),
                slashes: Vec::new(),
            });

        entry.amount += amount;
        entry.locked_until = locked_until;
        self.total_staked += amount;

        Ok(())
    }

    /// Unstake credits (if lock period has passed)
    #[wasm_bindgen]
    pub fn unstake(&mut self, node_id: &str, amount: u64) -> Result<u64, JsValue> {
        let now = current_timestamp_ms();

        let entry = self
            .stakes
            .get_mut(node_id)
            .ok_or_else(|| JsValue::from_str("No stake found"))?;

        if now < entry.locked_until {
            return Err(JsValue::from_str("Stake is locked"));
        }

        if amount > entry.amount {
            return Err(JsValue::from_str("Insufficient stake"));
        }

        entry.amount -= amount;
        self.total_staked -= amount;

        Ok(amount)
    }

    /// Slash stake for bad behavior
    #[wasm_bindgen]
    pub fn slash(
        &mut self,
        node_id: &str,
        reason: SlashReason,
        evidence: &str,
    ) -> Result<u64, JsValue> {
        let now = current_timestamp_ms();

        let entry = self
            .stakes
            .get_mut(node_id)
            .ok_or_else(|| JsValue::from_str("No stake found"))?;

        // Calculate slash amount
        let slash_pct = reason.slash_percentage();
        let slash_amount = ((entry.amount as f64) * (slash_pct as f64)) as u64;

        // Apply slash
        entry.amount = entry.amount.saturating_sub(slash_amount);
        self.total_staked -= slash_amount;
        self.total_slashed += slash_amount;

        // Record event
        entry.slashes.push(SlashEvent {
            amount: slash_amount,
            reason,
            timestamp: now,
            evidence: evidence.to_string(),
        });

        Ok(slash_amount)
    }

    /// Delegate stake to another node
    #[wasm_bindgen]
    pub fn delegate(&mut self, from_node: &str, to_node: &str, amount: u64) -> Result<(), JsValue> {
        // Verify from_node has sufficient stake
        let from_entry = self
            .stakes
            .get_mut(from_node)
            .ok_or_else(|| JsValue::from_str("Delegator has no stake"))?;

        if from_entry.amount < amount {
            return Err(JsValue::from_str("Insufficient stake to delegate"));
        }

        // Reduce from_node stake
        from_entry.amount -= amount;

        // Add to to_node delegated
        let to_entry = self
            .stakes
            .entry(to_node.to_string())
            .or_insert_with(|| StakeEntry {
                amount: 0,
                locked_until: 0,
                delegated: 0,
                delegators: Vec::new(),
                slashes: Vec::new(),
            });

        to_entry.delegated += amount;
        if !to_entry.delegators.contains(&from_node.to_string()) {
            to_entry.delegators.push(from_node.to_string());
        }

        Ok(())
    }

    /// Undelegate stake
    #[wasm_bindgen]
    pub fn undelegate(
        &mut self,
        from_node: &str,
        to_node: &str,
        amount: u64,
    ) -> Result<(), JsValue> {
        // Reduce delegated from to_node
        let to_entry = self
            .stakes
            .get_mut(to_node)
            .ok_or_else(|| JsValue::from_str("Target node not found"))?;

        if to_entry.delegated < amount {
            return Err(JsValue::from_str("Insufficient delegated amount"));
        }

        to_entry.delegated -= amount;

        // Return to from_node
        let from_entry = self
            .stakes
            .entry(from_node.to_string())
            .or_insert_with(|| StakeEntry {
                amount: 0,
                locked_until: 0,
                delegated: 0,
                delegators: Vec::new(),
                slashes: Vec::new(),
            });

        from_entry.amount += amount;

        Ok(())
    }

    /// Get lock timestamp for a node
    #[wasm_bindgen(js_name = getLockTimestamp)]
    pub fn get_lock_timestamp(&self, node_id: &str) -> u64 {
        self.stakes
            .get(node_id)
            .map(|s| s.locked_until)
            .unwrap_or(0)
    }

    /// Check if stake is locked
    #[wasm_bindgen(js_name = isLocked)]
    pub fn is_locked(&self, node_id: &str) -> bool {
        let now = current_timestamp_ms();
        self.stakes
            .get(node_id)
            .map(|s| now < s.locked_until)
            .unwrap_or(false)
    }

    /// Get slash count for a node
    #[wasm_bindgen(js_name = getSlashCount)]
    pub fn get_slash_count(&self, node_id: &str) -> usize {
        self.stakes
            .get(node_id)
            .map(|s| s.slashes.len())
            .unwrap_or(0)
    }

    /// Get total amount slashed from a node
    #[wasm_bindgen(js_name = getNodeTotalSlashed)]
    pub fn get_node_total_slashed(&self, node_id: &str) -> u64 {
        self.stakes
            .get(node_id)
            .map(|s| s.slashes.iter().map(|e| e.amount).sum())
            .unwrap_or(0)
    }

    /// Get delegator count
    #[wasm_bindgen(js_name = getDelegatorCount)]
    pub fn get_delegator_count(&self, node_id: &str) -> usize {
        self.stakes
            .get(node_id)
            .map(|s| s.delegators.len())
            .unwrap_or(0)
    }

    /// Get number of stakers
    #[wasm_bindgen(js_name = stakerCount)]
    pub fn staker_count(&self) -> usize {
        self.stakes.len()
    }

    /// Export stake data as JSON
    #[wasm_bindgen(js_name = exportJson)]
    pub fn export_json(&self) -> String {
        serde_json::to_string(&self.stakes).unwrap_or_else(|_| "{}".to_string())
    }
}

impl Default for StakeManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slash_percentages() {
        assert!((SlashReason::InvalidResult.slash_percentage() - 0.05).abs() < 0.001);
        assert!((SlashReason::DoubleSpend.slash_percentage() - 1.0).abs() < 0.001);
        assert!((SlashReason::SybilAttack.slash_percentage() - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_default_params() {
        let manager = StakeManager::new();
        assert_eq!(manager.min_stake(), 100);
    }

    // Tests that use JsValue-returning functions must be gated for WASM
    #[cfg(target_arch = "wasm32")]
    mod wasm_tests {
        use super::*;
        use wasm_bindgen_test::*;

        #[wasm_bindgen_test]
        fn test_stake_and_unstake() {
            let mut manager = StakeManager::new();

            manager.stake("node-1", 500).unwrap();
            assert_eq!(manager.get_stake("node-1"), 500);
            assert_eq!(manager.total_staked(), 500);
            assert!(manager.meets_minimum("node-1"));

            // Cannot unstake immediately (locked)
            assert!(manager.unstake("node-1", 100).is_err());
        }

        #[wasm_bindgen_test]
        fn test_slash() {
            let mut manager = StakeManager::new();
            manager.stake("node-1", 1000).unwrap();

            // Slash for invalid result (5%)
            let slashed = manager
                .slash("node-1", SlashReason::InvalidResult, "task:123")
                .unwrap();
            assert_eq!(slashed, 50);
            assert_eq!(manager.get_stake("node-1"), 950);
            assert_eq!(manager.total_slashed(), 50);
        }

        #[wasm_bindgen_test]
        fn test_delegation() {
            let mut manager = StakeManager::new();

            manager.stake("node-1", 1000).unwrap();
            manager.delegate("node-1", "node-2", 300).unwrap();

            assert_eq!(manager.get_stake("node-1"), 700);
            assert_eq!(manager.get_effective_stake("node-2"), 300);
            assert_eq!(manager.get_delegator_count("node-2"), 1);
        }
    }
}
