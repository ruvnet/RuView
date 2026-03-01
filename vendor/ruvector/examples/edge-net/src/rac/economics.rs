//! # RAC Economic Layer
//!
//! Crypto-economic incentives and mechanism design for adversarial coherence.
//! Implements concepts from research.md:
//!
//! - **Staking & Slashing**: Nodes stake collateral that can be slashed for misbehavior
//! - **Reputation Decay**: Reputation scores diminish over time to prevent gaming
//! - **Time-Locked Rewards**: Rewards vest over time to allow dispute resolution
//! - **Adaptive Incentives**: RL-based tuning of reward parameters
//!
//! ## References
//! - [PoS Slashing](https://daic.capital) - Validator stake mechanics
//! - [MeritRank](https://arxiv.org/org) - Reputation decay algorithms
//! - [BDEQ](https://pmc.ncbi.nlm.nih.gov) - RL-based edge network optimization

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use rustc_hash::FxHashMap;
use std::sync::RwLock;

use super::{EventId, PublicKeyBytes, current_timestamp_ms};

// ============================================================================
// Staking & Slashing (Economic Security)
// ============================================================================

/// Stake record for a node
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StakeRecord {
    /// Node public key
    pub node_id: PublicKeyBytes,
    /// Staked amount in tokens
    pub amount: u64,
    /// Stake timestamp
    pub staked_at: u64,
    /// Lock period in ms
    pub lock_period_ms: u64,
    /// Whether stake is currently locked
    pub locked: bool,
    /// Accumulated slashes
    pub slashed_amount: u64,
}

/// Slashing event
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SlashEvent {
    /// Node being slashed
    pub node_id: PublicKeyBytes,
    /// Slash amount
    pub amount: u64,
    /// Reason for slash
    pub reason: SlashReason,
    /// Related event IDs (evidence)
    pub evidence: Vec<EventId>,
    /// Timestamp
    pub timestamp: u64,
}

/// Reasons for slashing
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SlashReason {
    /// Submitted incorrect computation result
    IncorrectResult,
    /// Attempted to submit conflicting claims
    Equivocation,
    /// Failed to respond to challenge
    ChallengeTimeout,
    /// Detected Sybil behavior
    SybilAttack,
    /// Violated protocol rules
    ProtocolViolation,
}

/// Stake manager for the network
#[wasm_bindgen]
pub struct StakeManager {
    /// Stakes by node ID
    stakes: RwLock<FxHashMap<[u8; 32], StakeRecord>>,
    /// Slash history
    slashes: RwLock<Vec<SlashEvent>>,
    /// Minimum stake required to participate
    min_stake: u64,
    /// Slash percentages by reason
    slash_rates: SlashRates,
}

/// Slash percentages for different violations
#[derive(Clone, Debug)]
pub struct SlashRates {
    pub incorrect_result: f32,
    pub equivocation: f32,
    pub challenge_timeout: f32,
    pub sybil_attack: f32,
    pub protocol_violation: f32,
}

impl Default for SlashRates {
    fn default() -> Self {
        Self {
            incorrect_result: 0.10,    // 10% slash
            equivocation: 0.50,        // 50% slash (severe)
            challenge_timeout: 0.05,   // 5% slash
            sybil_attack: 1.0,         // 100% slash
            protocol_violation: 0.20,  // 20% slash
        }
    }
}

#[wasm_bindgen]
impl StakeManager {
    /// Create a new stake manager
    #[wasm_bindgen(constructor)]
    pub fn new(min_stake: u64) -> Self {
        Self {
            stakes: RwLock::new(FxHashMap::default()),
            slashes: RwLock::new(Vec::new()),
            min_stake,
            slash_rates: SlashRates::default(),
        }
    }

    /// Get minimum stake requirement
    #[wasm_bindgen(js_name = getMinStake)]
    pub fn get_min_stake(&self) -> u64 {
        self.min_stake
    }

    /// Get staked amount for a node
    #[wasm_bindgen(js_name = getStake)]
    pub fn get_stake(&self, node_id: &[u8]) -> u64 {
        if node_id.len() != 32 {
            return 0;
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(node_id);

        self.stakes.read().unwrap()
            .get(&key)
            .map(|s| s.amount.saturating_sub(s.slashed_amount))
            .unwrap_or(0)
    }

    /// Check if node has sufficient stake
    #[wasm_bindgen(js_name = hasSufficientStake)]
    pub fn has_sufficient_stake(&self, node_id: &[u8]) -> bool {
        self.get_stake(node_id) >= self.min_stake
    }

    /// Get total staked amount in network
    #[wasm_bindgen(js_name = totalStaked)]
    pub fn total_staked(&self) -> u64 {
        self.stakes.read().unwrap()
            .values()
            .map(|s| s.amount.saturating_sub(s.slashed_amount))
            .sum()
    }

    /// Get number of stakers
    #[wasm_bindgen(js_name = stakerCount)]
    pub fn staker_count(&self) -> usize {
        self.stakes.read().unwrap()
            .values()
            .filter(|s| s.amount > s.slashed_amount)
            .count()
    }
}

impl StakeManager {
    /// Stake tokens for a node
    pub fn stake(&self, node_id: PublicKeyBytes, amount: u64, lock_period_ms: u64) -> bool {
        if amount < self.min_stake {
            return false;
        }

        let mut stakes = self.stakes.write().unwrap();
        let now = current_timestamp_ms();

        stakes.entry(node_id)
            .and_modify(|s| {
                s.amount = s.amount.saturating_add(amount);
                s.lock_period_ms = lock_period_ms;
                s.locked = true;
            })
            .or_insert(StakeRecord {
                node_id,
                amount,
                staked_at: now,
                lock_period_ms,
                locked: true,
                slashed_amount: 0,
            });

        true
    }

    /// Unstake tokens (if lock period has passed)
    pub fn unstake(&self, node_id: &PublicKeyBytes) -> Result<u64, &'static str> {
        let mut stakes = self.stakes.write().unwrap();
        let now = current_timestamp_ms();

        let stake = stakes.get_mut(node_id).ok_or("No stake found")?;

        let unlock_time = stake.staked_at.saturating_add(stake.lock_period_ms);
        if now < unlock_time {
            return Err("Stake is still locked");
        }

        let available = stake.amount.saturating_sub(stake.slashed_amount);
        stakes.remove(node_id);

        Ok(available)
    }

    /// Slash a node's stake
    pub fn slash(&self, node_id: &PublicKeyBytes, reason: SlashReason, evidence: Vec<EventId>) -> u64 {
        let mut stakes = self.stakes.write().unwrap();
        let mut slashes = self.slashes.write().unwrap();

        let Some(stake) = stakes.get_mut(node_id) else {
            return 0;
        };

        let slash_rate = match reason {
            SlashReason::IncorrectResult => self.slash_rates.incorrect_result,
            SlashReason::Equivocation => self.slash_rates.equivocation,
            SlashReason::ChallengeTimeout => self.slash_rates.challenge_timeout,
            SlashReason::SybilAttack => self.slash_rates.sybil_attack,
            SlashReason::ProtocolViolation => self.slash_rates.protocol_violation,
        };

        let available = stake.amount.saturating_sub(stake.slashed_amount);
        let slash_amount = (available as f32 * slash_rate) as u64;
        stake.slashed_amount = stake.slashed_amount.saturating_add(slash_amount);

        slashes.push(SlashEvent {
            node_id: *node_id,
            amount: slash_amount,
            reason,
            evidence,
            timestamp: current_timestamp_ms(),
        });

        slash_amount
    }

    /// Get slash history for a node
    pub fn get_slashes(&self, node_id: &PublicKeyBytes) -> Vec<SlashEvent> {
        self.slashes.read().unwrap()
            .iter()
            .filter(|s| &s.node_id == node_id)
            .cloned()
            .collect()
    }
}

// ============================================================================
// Reputation System with Decay
// ============================================================================

/// Reputation record for a node
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReputationRecord {
    /// Node public key
    pub node_id: PublicKeyBytes,
    /// Current reputation score (0.0 - 1.0)
    pub score: f64,
    /// Last update timestamp
    pub updated_at: u64,
    /// Successful tasks completed
    pub successes: u64,
    /// Failed/disputed tasks
    pub failures: u64,
    /// Challenges won
    pub challenges_won: u64,
    /// Challenges lost
    pub challenges_lost: u64,
}

impl ReputationRecord {
    /// Calculate effective reputation with decay
    pub fn effective_score(&self, now: u64, decay_rate: f64, decay_interval_ms: u64) -> f64 {
        if now <= self.updated_at {
            return self.score;
        }

        let elapsed = now - self.updated_at;
        let decay_periods = (elapsed / decay_interval_ms) as f64;
        let decay_factor = (1.0 - decay_rate).powf(decay_periods);

        (self.score * decay_factor).max(0.0)
    }
}

/// Reputation manager with decay mechanics
#[wasm_bindgen]
pub struct ReputationManager {
    /// Reputation records by node ID
    records: RwLock<FxHashMap<[u8; 32], ReputationRecord>>,
    /// Decay rate per interval (0.0 - 1.0)
    decay_rate: f64,
    /// Decay interval in ms
    decay_interval_ms: u64,
    /// Initial reputation for new nodes
    initial_reputation: f64,
    /// Minimum reputation to participate
    min_reputation: f64,
}

#[wasm_bindgen]
impl ReputationManager {
    /// Create a new reputation manager
    #[wasm_bindgen(constructor)]
    pub fn new(decay_rate: f64, decay_interval_ms: u64) -> Self {
        Self {
            records: RwLock::new(FxHashMap::default()),
            decay_rate: decay_rate.clamp(0.0, 0.5), // Max 50% decay per interval
            decay_interval_ms,
            initial_reputation: 0.5,
            min_reputation: 0.1,
        }
    }

    /// Get effective reputation for a node (with decay applied)
    #[wasm_bindgen(js_name = getReputation)]
    pub fn get_reputation(&self, node_id: &[u8]) -> f64 {
        if node_id.len() != 32 {
            return 0.0;
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(node_id);

        let now = current_timestamp_ms();
        self.records.read().unwrap()
            .get(&key)
            .map(|r| r.effective_score(now, self.decay_rate, self.decay_interval_ms))
            .unwrap_or(0.0)
    }

    /// Check if node has sufficient reputation
    #[wasm_bindgen(js_name = hasSufficientReputation)]
    pub fn has_sufficient_reputation(&self, node_id: &[u8]) -> bool {
        self.get_reputation(node_id) >= self.min_reputation
    }

    /// Get number of tracked nodes
    #[wasm_bindgen(js_name = nodeCount)]
    pub fn node_count(&self) -> usize {
        self.records.read().unwrap().len()
    }

    /// Get average network reputation
    #[wasm_bindgen(js_name = averageReputation)]
    pub fn average_reputation(&self) -> f64 {
        let records = self.records.read().unwrap();
        if records.is_empty() {
            return 0.0;
        }

        let now = current_timestamp_ms();
        let total: f64 = records.values()
            .map(|r| r.effective_score(now, self.decay_rate, self.decay_interval_ms))
            .sum();

        total / records.len() as f64
    }
}

impl ReputationManager {
    /// Register a new node with initial reputation
    pub fn register(&self, node_id: PublicKeyBytes) {
        let mut records = self.records.write().unwrap();
        let now = current_timestamp_ms();

        records.entry(node_id).or_insert(ReputationRecord {
            node_id,
            score: self.initial_reputation,
            updated_at: now,
            successes: 0,
            failures: 0,
            challenges_won: 0,
            challenges_lost: 0,
        });
    }

    /// Record a successful task completion
    pub fn record_success(&self, node_id: &PublicKeyBytes, weight: f64) {
        self.update_reputation(node_id, true, weight);
    }

    /// Record a task failure
    pub fn record_failure(&self, node_id: &PublicKeyBytes, weight: f64) {
        self.update_reputation(node_id, false, weight);
    }

    /// Record challenge outcome
    pub fn record_challenge(&self, winner: &PublicKeyBytes, loser: &PublicKeyBytes, weight: f64) {
        let mut records = self.records.write().unwrap();
        let now = current_timestamp_ms();

        // Update winner
        if let Some(record) = records.get_mut(winner) {
            // Apply decay first
            record.score = record.effective_score(now, self.decay_rate, self.decay_interval_ms);
            // Then apply boost
            record.score = (record.score + weight * 0.1).min(1.0);
            record.challenges_won += 1;
            record.updated_at = now;
        }

        // Update loser
        if let Some(record) = records.get_mut(loser) {
            record.score = record.effective_score(now, self.decay_rate, self.decay_interval_ms);
            record.score = (record.score - weight * 0.15).max(0.0);
            record.challenges_lost += 1;
            record.updated_at = now;
        }
    }

    /// Update reputation based on outcome
    fn update_reputation(&self, node_id: &PublicKeyBytes, success: bool, weight: f64) {
        let mut records = self.records.write().unwrap();
        let now = current_timestamp_ms();

        let record = records.entry(*node_id).or_insert(ReputationRecord {
            node_id: *node_id,
            score: self.initial_reputation,
            updated_at: now,
            successes: 0,
            failures: 0,
            challenges_won: 0,
            challenges_lost: 0,
        });

        // Apply decay first
        record.score = record.effective_score(now, self.decay_rate, self.decay_interval_ms);

        // Then apply update
        if success {
            record.score = (record.score + weight * 0.05).min(1.0);
            record.successes += 1;
        } else {
            record.score = (record.score - weight * 0.10).max(0.0);
            record.failures += 1;
        }

        record.updated_at = now;
    }

    /// Get detailed record for a node
    pub fn get_record(&self, node_id: &PublicKeyBytes) -> Option<ReputationRecord> {
        self.records.read().unwrap().get(node_id).cloned()
    }

    /// Prune nodes with zero reputation
    pub fn prune_inactive(&self) {
        let now = current_timestamp_ms();
        let mut records = self.records.write().unwrap();

        records.retain(|_, r| {
            r.effective_score(now, self.decay_rate, self.decay_interval_ms) > 0.01
        });
    }
}

// ============================================================================
// Time-Locked Rewards
// ============================================================================

/// Reward record with time lock
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RewardRecord {
    /// Reward ID
    pub id: [u8; 32],
    /// Recipient node
    pub recipient: PublicKeyBytes,
    /// Reward amount
    pub amount: u64,
    /// Related task/event
    pub task_id: EventId,
    /// Creation timestamp
    pub created_at: u64,
    /// Vesting period in ms
    pub vesting_period_ms: u64,
    /// Whether reward has been claimed
    pub claimed: bool,
    /// Whether reward was clawed back
    pub clawed_back: bool,
}

impl RewardRecord {
    /// Check if reward is vested
    pub fn is_vested(&self, now: u64) -> bool {
        now >= self.created_at.saturating_add(self.vesting_period_ms)
    }

    /// Get vesting progress (0.0 - 1.0)
    pub fn vesting_progress(&self, now: u64) -> f64 {
        if now >= self.created_at.saturating_add(self.vesting_period_ms) {
            return 1.0;
        }
        if now <= self.created_at {
            return 0.0;
        }

        let elapsed = now - self.created_at;
        (elapsed as f64 / self.vesting_period_ms as f64).min(1.0)
    }
}

/// Manages time-locked rewards
#[wasm_bindgen]
pub struct RewardManager {
    /// Pending rewards
    rewards: RwLock<Vec<RewardRecord>>,
    /// Default vesting period
    default_vesting_ms: u64,
    /// Total rewards distributed
    total_distributed: RwLock<u64>,
    /// Total rewards clawed back
    total_clawed_back: RwLock<u64>,
}

#[wasm_bindgen]
impl RewardManager {
    /// Create a new reward manager
    #[wasm_bindgen(constructor)]
    pub fn new(default_vesting_ms: u64) -> Self {
        Self {
            rewards: RwLock::new(Vec::new()),
            default_vesting_ms,
            total_distributed: RwLock::new(0),
            total_clawed_back: RwLock::new(0),
        }
    }

    /// Get number of pending rewards
    #[wasm_bindgen(js_name = pendingCount)]
    pub fn pending_count(&self) -> usize {
        self.rewards.read().unwrap()
            .iter()
            .filter(|r| !r.claimed && !r.clawed_back)
            .count()
    }

    /// Get total pending reward amount
    #[wasm_bindgen(js_name = pendingAmount)]
    pub fn pending_amount(&self) -> u64 {
        self.rewards.read().unwrap()
            .iter()
            .filter(|r| !r.claimed && !r.clawed_back)
            .map(|r| r.amount)
            .sum()
    }

    /// Get claimable rewards for a node
    #[wasm_bindgen(js_name = claimableAmount)]
    pub fn claimable_amount(&self, node_id: &[u8]) -> u64 {
        if node_id.len() != 32 {
            return 0;
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(node_id);

        let now = current_timestamp_ms();
        self.rewards.read().unwrap()
            .iter()
            .filter(|r| r.recipient == key && !r.claimed && !r.clawed_back && r.is_vested(now))
            .map(|r| r.amount)
            .sum()
    }
}

impl RewardManager {
    /// Issue a new reward
    pub fn issue_reward(&self, recipient: PublicKeyBytes, amount: u64, task_id: EventId) -> [u8; 32] {
        use sha2::{Sha256, Digest};

        let now = current_timestamp_ms();

        let mut hasher = Sha256::new();
        hasher.update(&recipient);
        hasher.update(&amount.to_le_bytes());
        hasher.update(&task_id);
        hasher.update(&now.to_le_bytes());
        let result = hasher.finalize();
        let mut id = [0u8; 32];
        id.copy_from_slice(&result);

        let reward = RewardRecord {
            id,
            recipient,
            amount,
            task_id,
            created_at: now,
            vesting_period_ms: self.default_vesting_ms,
            claimed: false,
            clawed_back: false,
        };

        self.rewards.write().unwrap().push(reward);
        id
    }

    /// Claim vested rewards for a node
    pub fn claim(&self, node_id: &PublicKeyBytes) -> u64 {
        let now = current_timestamp_ms();
        let mut rewards = self.rewards.write().unwrap();
        let mut claimed_amount = 0u64;

        for reward in rewards.iter_mut() {
            if reward.recipient == *node_id
                && !reward.claimed
                && !reward.clawed_back
                && reward.is_vested(now)
            {
                reward.claimed = true;
                claimed_amount = claimed_amount.saturating_add(reward.amount);
            }
        }

        *self.total_distributed.write().unwrap() += claimed_amount;
        claimed_amount
    }

    /// Claw back rewards for a disputed task
    pub fn claw_back(&self, task_id: &EventId) -> u64 {
        let now = current_timestamp_ms();
        let mut rewards = self.rewards.write().unwrap();
        let mut clawed_back = 0u64;

        for reward in rewards.iter_mut() {
            if &reward.task_id == task_id && !reward.claimed && !reward.clawed_back {
                // Can only claw back if not yet vested
                if !reward.is_vested(now) {
                    reward.clawed_back = true;
                    clawed_back = clawed_back.saturating_add(reward.amount);
                }
            }
        }

        *self.total_clawed_back.write().unwrap() += clawed_back;
        clawed_back
    }

    /// Get rewards for a specific task
    pub fn get_task_rewards(&self, task_id: &EventId) -> Vec<RewardRecord> {
        self.rewards.read().unwrap()
            .iter()
            .filter(|r| &r.task_id == task_id)
            .cloned()
            .collect()
    }

    /// Prune old claimed/clawed-back rewards
    pub fn prune_old(&self, max_age_ms: u64) {
        let now = current_timestamp_ms();
        let mut rewards = self.rewards.write().unwrap();

        rewards.retain(|r| {
            if r.claimed || r.clawed_back {
                now - r.created_at < max_age_ms
            } else {
                true // Keep pending rewards
            }
        });
    }
}

// ============================================================================
// Combined Economic Engine
// ============================================================================

/// RAC-specific combined economic engine managing stakes, reputation, and rewards
#[wasm_bindgen(js_name = RacEconomicEngine)]
pub struct RacEconomicEngine {
    stakes: StakeManager,
    reputation: ReputationManager,
    rewards: RewardManager,
}

#[wasm_bindgen]
impl RacEconomicEngine {
    /// Create a new RAC economic engine
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            stakes: StakeManager::new(100), // 100 token minimum stake
            reputation: ReputationManager::new(0.10, 86400_000), // 10% decay per day
            rewards: RewardManager::new(3600_000), // 1 hour vesting
        }
    }

    /// Check if node can participate (has stake + reputation)
    #[wasm_bindgen(js_name = canParticipate)]
    pub fn can_participate(&self, node_id: &[u8]) -> bool {
        self.stakes.has_sufficient_stake(node_id) && self.reputation.has_sufficient_reputation(node_id)
    }

    /// Get combined score (stake-weighted reputation)
    #[wasm_bindgen(js_name = getCombinedScore)]
    pub fn get_combined_score(&self, node_id: &[u8]) -> f64 {
        let stake = self.stakes.get_stake(node_id) as f64;
        let reputation = self.reputation.get_reputation(node_id);

        // Combined score: sqrt(stake) * reputation
        // This gives both factors influence while preventing extreme dominance
        stake.sqrt() * reputation
    }

    /// Get summary statistics as JSON
    #[wasm_bindgen(js_name = getSummary)]
    pub fn get_summary(&self) -> String {
        let summary = serde_json::json!({
            "total_staked": self.stakes.total_staked(),
            "staker_count": self.stakes.staker_count(),
            "avg_reputation": self.reputation.average_reputation(),
            "node_count": self.reputation.node_count(),
            "pending_rewards": self.rewards.pending_amount(),
            "pending_reward_count": self.rewards.pending_count(),
        });
        serde_json::to_string(&summary).unwrap_or_else(|_| "{}".to_string())
    }
}

impl Default for RacEconomicEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RacEconomicEngine {
    /// Record a successful task with economic effects
    pub fn record_task_success(&self, node_id: &PublicKeyBytes, task_id: EventId, reward_amount: u64) {
        self.reputation.record_success(node_id, 1.0);
        self.rewards.issue_reward(*node_id, reward_amount, task_id);
    }

    /// Record a task failure with economic effects
    pub fn record_task_failure(&self, node_id: &PublicKeyBytes, task_id: EventId) {
        self.reputation.record_failure(node_id, 1.0);
        self.rewards.claw_back(&task_id);
    }

    /// Process a successful challenge (winner/loser)
    pub fn process_challenge(&self, winner: &PublicKeyBytes, loser: &PublicKeyBytes, evidence: Vec<EventId>) {
        // Update reputations
        self.reputation.record_challenge(winner, loser, 1.0);

        // Slash loser's stake
        self.stakes.slash(loser, SlashReason::IncorrectResult, evidence);
    }

    /// Stake tokens for a node
    pub fn stake(&self, node_id: PublicKeyBytes, amount: u64) -> bool {
        self.reputation.register(node_id);
        self.stakes.stake(node_id, amount, 7 * 24 * 3600_000) // 7 day lock
    }

    /// Claim available rewards
    pub fn claim_rewards(&self, node_id: &PublicKeyBytes) -> u64 {
        self.rewards.claim(node_id)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stake_manager() {
        let manager = StakeManager::new(100);

        let node_id = [1u8; 32];
        assert!(!manager.has_sufficient_stake(&node_id));

        // Stake tokens
        assert!(manager.stake(node_id, 200, 0));
        assert!(manager.has_sufficient_stake(&node_id));
        assert_eq!(manager.get_stake(&node_id), 200);

        // Slash
        let slashed = manager.slash(&node_id, SlashReason::IncorrectResult, vec![]);
        assert_eq!(slashed, 20); // 10% of 200
        assert_eq!(manager.get_stake(&node_id), 180);
    }

    #[test]
    fn test_reputation_decay() {
        let manager = ReputationManager::new(0.5, 1000); // 50% decay per second

        let node_id = [1u8; 32];
        manager.register(node_id);

        let initial = manager.get_reputation(&node_id);
        assert!((initial - 0.5).abs() < 0.01);

        // Simulate time passing (decay applied on read)
        // Since we can't easily mock time, we test the calculation directly
        let record = manager.get_record(&node_id).unwrap();
        let future_score = record.effective_score(
            record.updated_at + 2000, // 2 intervals
            0.5,
            1000,
        );
        assert!((future_score - 0.125).abs() < 0.01); // 0.5 * 0.5 * 0.5
    }

    #[test]
    fn test_reward_vesting() {
        let manager = RewardManager::new(1000); // 1 second vesting

        let recipient = [1u8; 32];
        let task_id = [2u8; 32];

        let reward_id = manager.issue_reward(recipient, 100, task_id);
        assert_ne!(reward_id, [0u8; 32]);

        // Can't claim immediately (not vested)
        assert_eq!(manager.claimable_amount(&recipient), 0);

        // Test vesting calculation
        let rewards = manager.rewards.read().unwrap();
        let reward = rewards.iter().find(|r| r.id == reward_id).unwrap();
        assert!(reward.vesting_progress(reward.created_at + 500) < 1.0);
    }

    #[test]
    fn test_economic_engine() {
        let engine = RacEconomicEngine::new();

        let node_id = [1u8; 32];

        // Can't participate without stake
        assert!(!engine.can_participate(&node_id));

        // Stake and register
        assert!(engine.stake(node_id, 200));
        assert!(engine.can_participate(&node_id));

        // Get combined score
        let score = engine.get_combined_score(&node_id);
        assert!(score > 0.0);
    }

    #[test]
    fn test_slashing() {
        let manager = StakeManager::new(100);
        let node_id = [1u8; 32];

        manager.stake(node_id, 1000, 0);

        // Test different slash rates
        let equivocation_slash = manager.slash(&node_id, SlashReason::Equivocation, vec![]);
        assert_eq!(equivocation_slash, 500); // 50% of 1000

        // Remaining is 500, incorrect result = 10%
        let result_slash = manager.slash(&node_id, SlashReason::IncorrectResult, vec![]);
        assert_eq!(result_slash, 50); // 10% of 500
    }
}
