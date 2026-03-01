//! CRDT-based Credit Ledger
//!
//! Implements a conflict-free replicated data type (CRDT) ledger for P2P consistency.
//! Uses G-Counters for earnings (monotonically increasing) and PN-Counters for spending.

use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use wasm_bindgen::prelude::*;

use crate::curve::ContributionCurve;

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

/// Credit event reasons for audit trail
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum CreditReason {
    /// Earned from completing a task
    TaskCompleted { task_id: String },
    /// Earned from uptime bonus
    UptimeReward { hours: f32 },
    /// Earned from referral
    Referral { referee: String },
    /// Staked for participation
    Stake { amount: u64, locked: bool },
    /// Transferred between nodes
    Transfer {
        from: String,
        to: String,
        memo: String,
    },
    /// Penalty for invalid work
    Penalty { reason: String },
}

/// CRDT-based credit ledger for P2P consistency
///
/// The ledger uses two types of counters:
/// - G-Counter (grow-only) for credits earned - safe for concurrent updates
/// - PN-Counter (positive-negative) for credits spent - supports disputes
///
/// ```text
/// Earned (G-Counter):     Spent (PN-Counter):
/// +----------------+      +--------------------+
/// | event_1: 100   |      | event_a: (50, 0)   |  <- (positive, negative)
/// | event_2: 200   |      | event_b: (30, 10)  |  <- disputed 10 returned
/// | event_3: 150   |      +--------------------+
/// +----------------+
///
/// Balance = sum(earned) - sum(spent.positive - spent.negative) - staked
/// ```
#[wasm_bindgen]
#[derive(Clone)]
pub struct CreditLedger {
    /// Node identifier
    node_id: String,

    /// G-Counter: monotonically increasing credits earned
    /// Key: event_id, Value: amount credited
    earned: FxHashMap<String, u64>,

    /// PN-Counter: credits spent/penalized
    /// Key: event_id, Value: (positive_spent, negative_refund)
    spent: FxHashMap<String, (u64, u64)>,

    /// Merkle root of current state for quick verification
    state_root: [u8; 32],

    /// Total network compute hours (for multiplier calculation)
    network_compute: f64,

    /// Staked credits (locked for participation)
    staked: u64,

    /// Last sync timestamp (Unix ms)
    last_sync: u64,

    /// Event counter for generating unique IDs
    event_counter: u64,
}

#[wasm_bindgen]
impl CreditLedger {
    /// Create a new credit ledger for a node
    #[wasm_bindgen(constructor)]
    pub fn new(node_id: String) -> Result<CreditLedger, JsValue> {
        if node_id.is_empty() {
            return Err(JsValue::from_str("Node ID cannot be empty"));
        }

        Ok(CreditLedger {
            node_id,
            earned: FxHashMap::default(),
            spent: FxHashMap::default(),
            state_root: [0u8; 32],
            network_compute: 0.0,
            staked: 0,
            last_sync: 0,
            event_counter: 0,
        })
    }

    /// Get the node ID
    #[wasm_bindgen(js_name = nodeId)]
    pub fn node_id(&self) -> String {
        self.node_id.clone()
    }

    /// Get current available balance (earned - spent - staked)
    #[wasm_bindgen]
    pub fn balance(&self) -> u64 {
        let total_earned: u64 = self.earned.values().sum();
        let total_spent: u64 = self
            .spent
            .values()
            .map(|(pos, neg)| pos.saturating_sub(*neg))
            .sum();

        total_earned
            .saturating_sub(total_spent)
            .saturating_sub(self.staked)
    }

    /// Get total credits ever earned (before spending)
    #[wasm_bindgen(js_name = totalEarned)]
    pub fn total_earned(&self) -> u64 {
        self.earned.values().sum()
    }

    /// Get total credits spent
    #[wasm_bindgen(js_name = totalSpent)]
    pub fn total_spent(&self) -> u64 {
        self.spent
            .values()
            .map(|(pos, neg)| pos.saturating_sub(*neg))
            .sum()
    }

    /// Get staked amount
    #[wasm_bindgen(js_name = stakedAmount)]
    pub fn staked_amount(&self) -> u64 {
        self.staked
    }

    /// Get network compute hours
    #[wasm_bindgen(js_name = networkCompute)]
    pub fn network_compute(&self) -> f64 {
        self.network_compute
    }

    /// Get current contribution multiplier
    #[wasm_bindgen(js_name = currentMultiplier)]
    pub fn current_multiplier(&self) -> f32 {
        ContributionCurve::current_multiplier(self.network_compute)
    }

    /// Get the state root (Merkle root of ledger state)
    #[wasm_bindgen(js_name = stateRoot)]
    pub fn state_root(&self) -> Vec<u8> {
        self.state_root.to_vec()
    }

    /// Get state root as hex string
    #[wasm_bindgen(js_name = stateRootHex)]
    pub fn state_root_hex(&self) -> String {
        self.state_root
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect()
    }

    /// Credit the ledger (earn credits)
    ///
    /// This updates the G-Counter which is monotonically increasing.
    /// Safe for concurrent P2P updates.
    #[wasm_bindgen]
    pub fn credit(&mut self, amount: u64, _reason: &str) -> Result<String, JsValue> {
        if amount == 0 {
            return Err(JsValue::from_str("Amount must be positive"));
        }

        // Generate unique event ID
        self.event_counter += 1;
        let event_id = format!("{}:{}", self.node_id, self.event_counter);

        // Update G-Counter
        self.earned.insert(event_id.clone(), amount);

        // Update state root
        self.recompute_state_root();

        Ok(event_id)
    }

    /// Credit with multiplier applied (for task rewards)
    #[wasm_bindgen(js_name = creditWithMultiplier)]
    pub fn credit_with_multiplier(
        &mut self,
        base_amount: u64,
        reason: &str,
    ) -> Result<String, JsValue> {
        let multiplier = self.current_multiplier();
        let amount = (base_amount as f32 * multiplier) as u64;
        self.credit(amount, reason)
    }

    /// Deduct from the ledger (spend credits)
    ///
    /// This updates the PN-Counter positive side.
    /// Spending can be disputed/refunded by updating the negative side.
    #[wasm_bindgen]
    pub fn deduct(&mut self, amount: u64) -> Result<String, JsValue> {
        if self.balance() < amount {
            return Err(JsValue::from_str("Insufficient balance"));
        }

        // Generate unique event ID
        self.event_counter += 1;
        let event_id = format!("{}:{}", self.node_id, self.event_counter);

        // Update PN-Counter (positive side)
        self.spent.insert(event_id.clone(), (amount, 0));

        // Update state root
        self.recompute_state_root();

        Ok(event_id)
    }

    /// Refund a previous deduction (dispute resolution)
    ///
    /// This updates the PN-Counter negative side for the given event.
    #[wasm_bindgen]
    pub fn refund(&mut self, event_id: &str, amount: u64) -> Result<(), JsValue> {
        let entry = self
            .spent
            .get_mut(event_id)
            .ok_or_else(|| JsValue::from_str("Event not found"))?;

        if entry.1 + amount > entry.0 {
            return Err(JsValue::from_str("Refund exceeds original spend"));
        }

        entry.1 += amount;
        self.recompute_state_root();

        Ok(())
    }

    /// Stake credits for participation
    #[wasm_bindgen]
    pub fn stake(&mut self, amount: u64) -> Result<(), JsValue> {
        if self.balance() < amount {
            return Err(JsValue::from_str("Insufficient balance for stake"));
        }

        self.staked += amount;
        self.recompute_state_root();

        Ok(())
    }

    /// Unstake credits
    #[wasm_bindgen]
    pub fn unstake(&mut self, amount: u64) -> Result<(), JsValue> {
        if self.staked < amount {
            return Err(JsValue::from_str("Insufficient staked amount"));
        }

        self.staked -= amount;
        self.recompute_state_root();

        Ok(())
    }

    /// Slash staked credits (penalty for bad behavior)
    ///
    /// Returns the actual amount slashed (may be less if stake is insufficient)
    #[wasm_bindgen]
    pub fn slash(&mut self, amount: u64) -> Result<u64, JsValue> {
        let slash_amount = amount.min(self.staked);
        self.staked -= slash_amount;
        self.recompute_state_root();

        Ok(slash_amount)
    }

    /// Update network compute hours (from P2P sync)
    #[wasm_bindgen(js_name = updateNetworkCompute)]
    pub fn update_network_compute(&mut self, hours: f64) {
        self.network_compute = hours;
    }

    /// Merge with another ledger (CRDT merge operation)
    ///
    /// This is the core CRDT operation - associative, commutative, and idempotent.
    /// Safe to apply in any order with any number of concurrent updates.
    #[wasm_bindgen]
    pub fn merge(&mut self, other_earned: &[u8], other_spent: &[u8]) -> Result<u32, JsValue> {
        let mut merged_count = 0u32;

        // Deserialize and merge earned counter (G-Counter: take max)
        let earned_map: FxHashMap<String, u64> = serde_json::from_slice(other_earned)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse earned: {}", e)))?;

        for (key, value) in earned_map {
            let entry = self.earned.entry(key).or_insert(0);
            if value > *entry {
                *entry = value;
                merged_count += 1;
            }
        }

        // Deserialize and merge spent counter (PN-Counter: take max of each component)
        let spent_map: FxHashMap<String, (u64, u64)> = serde_json::from_slice(other_spent)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse spent: {}", e)))?;

        for (key, (pos, neg)) in spent_map {
            let entry = self.spent.entry(key).or_insert((0, 0));
            if pos > entry.0 || neg > entry.1 {
                entry.0 = entry.0.max(pos);
                entry.1 = entry.1.max(neg);
                merged_count += 1;
            }
        }

        // Update state and timestamp
        self.recompute_state_root();
        self.last_sync = current_timestamp_ms();

        Ok(merged_count)
    }

    /// Export earned counter for P2P sync
    #[wasm_bindgen(js_name = exportEarned)]
    pub fn export_earned(&self) -> Result<Vec<u8>, JsValue> {
        serde_json::to_vec(&self.earned)
            .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
    }

    /// Export spent counter for P2P sync
    #[wasm_bindgen(js_name = exportSpent)]
    pub fn export_spent(&self) -> Result<Vec<u8>, JsValue> {
        serde_json::to_vec(&self.spent)
            .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
    }

    /// Get event count
    #[wasm_bindgen(js_name = eventCount)]
    pub fn event_count(&self) -> usize {
        self.earned.len() + self.spent.len()
    }

    /// Verify state root matches current state
    #[wasm_bindgen(js_name = verifyStateRoot)]
    pub fn verify_state_root(&self, expected_root: &[u8]) -> bool {
        if expected_root.len() != 32 {
            return false;
        }

        let mut expected = [0u8; 32];
        expected.copy_from_slice(expected_root);

        self.state_root == expected
    }

    /// Recompute the Merkle state root
    fn recompute_state_root(&mut self) {
        let mut hasher = Sha256::new();

        // Hash earned entries (sorted for determinism)
        let mut earned_keys: Vec<_> = self.earned.keys().collect();
        earned_keys.sort();
        for key in earned_keys {
            hasher.update(key.as_bytes());
            hasher.update(&self.earned[key].to_le_bytes());
        }

        // Hash spent entries (sorted for determinism)
        let mut spent_keys: Vec<_> = self.spent.keys().collect();
        spent_keys.sort();
        for key in spent_keys {
            let (pos, neg) = self.spent[key];
            hasher.update(key.as_bytes());
            hasher.update(&pos.to_le_bytes());
            hasher.update(&neg.to_le_bytes());
        }

        // Hash staked amount
        hasher.update(&self.staked.to_le_bytes());

        // Finalize
        self.state_root = hasher.finalize().into();
    }
}

#[cfg(test)]
mod tests {
    // All ledger tests require JsValue which only works in WASM
    // Native tests are in curve.rs and reputation.rs

    #[cfg(target_arch = "wasm32")]
    mod wasm_tests {
        use super::super::*;
        use wasm_bindgen_test::*;

        #[wasm_bindgen_test]
        fn test_ledger_creation() {
            let ledger = CreditLedger::new("node-1".to_string()).unwrap();
            assert_eq!(ledger.node_id(), "node-1");
            assert_eq!(ledger.balance(), 0);
        }

        #[wasm_bindgen_test]
        fn test_empty_node_id_rejected() {
            let result = CreditLedger::new("".to_string());
            assert!(result.is_err());
        }

        #[wasm_bindgen_test]
        fn test_credit_and_deduct() {
            let mut ledger = CreditLedger::new("node-1".to_string()).unwrap();

            ledger.credit(100, "task:1").unwrap();
            assert_eq!(ledger.balance(), 100);
            assert_eq!(ledger.total_earned(), 100);

            ledger.deduct(30).unwrap();
            assert_eq!(ledger.balance(), 70);
            assert_eq!(ledger.total_spent(), 30);
        }

        #[wasm_bindgen_test]
        fn test_insufficient_balance() {
            let mut ledger = CreditLedger::new("node-1".to_string()).unwrap();
            ledger.credit(50, "task:1").unwrap();

            let result = ledger.deduct(100);
            assert!(result.is_err());
        }

        #[wasm_bindgen_test]
        fn test_stake_and_slash() {
            let mut ledger = CreditLedger::new("node-1".to_string()).unwrap();
            ledger.credit(200, "task:1").unwrap();

            ledger.stake(100).unwrap();
            assert_eq!(ledger.balance(), 100);
            assert_eq!(ledger.staked_amount(), 100);

            let slashed = ledger.slash(30).unwrap();
            assert_eq!(slashed, 30);
            assert_eq!(ledger.staked_amount(), 70);
        }

        #[wasm_bindgen_test]
        fn test_refund() {
            let mut ledger = CreditLedger::new("node-1".to_string()).unwrap();
            ledger.credit(100, "task:1").unwrap();

            let event_id = ledger.deduct(50).unwrap();
            assert_eq!(ledger.balance(), 50);

            ledger.refund(&event_id, 20).unwrap();
            assert_eq!(ledger.balance(), 70);
        }

        #[wasm_bindgen_test]
        fn test_state_root_changes() {
            let mut ledger = CreditLedger::new("node-1".to_string()).unwrap();
            let initial_root = ledger.state_root();

            ledger.credit(100, "task:1").unwrap();
            let after_credit = ledger.state_root();

            assert_ne!(initial_root, after_credit);
        }
    }
}
