//! Reward Calculation and Distribution

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct RewardClaim {
    pub node_id: String,
    pub amount: f64,
    pub source: RewardSource,
    pub claimed_at: std::time::Instant,
    pub tx_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RewardSource {
    PatternValidation,
    ConsensusParticipation,
    PatternContribution,
    Staking,
}

impl std::fmt::Display for RewardSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RewardSource::PatternValidation => write!(f, "pattern_validation"),
            RewardSource::ConsensusParticipation => write!(f, "consensus_participation"),
            RewardSource::PatternContribution => write!(f, "pattern_contribution"),
            RewardSource::Staking => write!(f, "staking"),
        }
    }
}

pub struct RewardCalculator {
    base_reward: f64,
    pattern_bonus: f64,
    staking_apy: f64,
    pending_rewards: HashMap<String, f64>,
}

impl RewardCalculator {
    pub fn new(base_reward: f64, pattern_bonus: f64, staking_apy: f64) -> Self {
        Self {
            base_reward,
            pattern_bonus,
            staking_apy,
            pending_rewards: HashMap::new(),
        }
    }

    /// Calculate reward for pattern validation
    pub fn pattern_validation_reward(&self, stake_weight: f64, pattern_quality: f64) -> f64 {
        self.base_reward * stake_weight * pattern_quality
    }

    /// Calculate reward for pattern contribution
    pub fn pattern_contribution_reward(&self, pattern_quality: f64, usage_count: usize) -> f64 {
        let usage_factor = (usage_count as f64).ln_1p();
        self.pattern_bonus * pattern_quality * usage_factor
    }

    /// Calculate staking reward for a period
    pub fn staking_reward(&self, stake_amount: f64, days: f64) -> f64 {
        // Daily rate from APY
        let daily_rate = (1.0 + self.staking_apy).powf(1.0 / 365.0) - 1.0;
        stake_amount * daily_rate * days
    }

    /// Add pending reward
    pub fn add_pending(&mut self, node_id: &str, amount: f64, _source: RewardSource) {
        *self
            .pending_rewards
            .entry(node_id.to_string())
            .or_insert(0.0) += amount;
    }

    /// Get pending rewards for a node
    pub fn pending_rewards(&self, node_id: &str) -> f64 {
        self.pending_rewards.get(node_id).copied().unwrap_or(0.0)
    }

    /// Claim rewards
    pub fn claim(&mut self, node_id: &str) -> Option<RewardClaim> {
        let amount = self.pending_rewards.remove(node_id)?;

        if amount <= 0.0 {
            return None;
        }

        Some(RewardClaim {
            node_id: node_id.to_string(),
            amount,
            source: RewardSource::Staking, // Simplified
            claimed_at: std::time::Instant::now(),
            tx_hash: format!("reward_tx_{}", rand::random::<u64>()),
        })
    }

    /// Get total pending rewards across all nodes
    pub fn total_pending(&self) -> f64 {
        self.pending_rewards.values().sum()
    }
}

impl Default for RewardCalculator {
    fn default() -> Self {
        Self::new(
            1.0,  // base_reward
            10.0, // pattern_bonus
            0.05, // 5% APY
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_validation_reward() {
        let calc = RewardCalculator::default();
        let reward = calc.pattern_validation_reward(1.0, 0.9);
        assert_eq!(reward, 0.9); // 1.0 * 1.0 * 0.9
    }

    #[test]
    fn test_pattern_contribution_reward() {
        let calc = RewardCalculator::default();
        let reward = calc.pattern_contribution_reward(1.0, 100);
        assert!(reward > 0.0);
        // Higher usage should give more reward
        let higher = calc.pattern_contribution_reward(1.0, 1000);
        assert!(higher > reward);
    }

    #[test]
    fn test_staking_reward() {
        let calc = RewardCalculator::default();
        let reward = calc.staking_reward(100.0, 365.0);
        // With 5% APY, should be close to 5.0
        assert!(reward > 4.8 && reward < 5.2);
    }

    #[test]
    fn test_pending_rewards() {
        let mut calc = RewardCalculator::default();

        calc.add_pending("node1", 5.0, RewardSource::Staking);
        calc.add_pending("node1", 3.0, RewardSource::PatternValidation);

        assert_eq!(calc.pending_rewards("node1"), 8.0);
        assert_eq!(calc.total_pending(), 8.0);

        let claim = calc.claim("node1").unwrap();
        assert_eq!(claim.amount, 8.0);
        assert_eq!(calc.pending_rewards("node1"), 0.0);
    }

    #[test]
    fn test_reward_source_display() {
        assert_eq!(RewardSource::Staking.to_string(), "staking");
        assert_eq!(
            RewardSource::PatternValidation.to_string(),
            "pattern_validation"
        );
    }
}
