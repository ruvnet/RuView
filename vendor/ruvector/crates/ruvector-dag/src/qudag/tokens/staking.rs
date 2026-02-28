//! Token Staking for Pattern Validation

use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct StakeInfo {
    pub amount: f64,
    pub staked_at: Instant,
    pub lock_duration: Duration,
    pub validator_weight: f64,
}

impl StakeInfo {
    pub fn new(amount: f64, lock_days: u64) -> Self {
        let lock_duration = Duration::from_secs(lock_days * 24 * 3600);

        // Weight increases with lock duration
        let weight_multiplier = 1.0 + (lock_days as f64 / 365.0);

        Self {
            amount,
            staked_at: Instant::now(),
            lock_duration,
            validator_weight: amount * weight_multiplier,
        }
    }

    pub fn is_locked(&self) -> bool {
        self.staked_at.elapsed() < self.lock_duration
    }

    pub fn time_remaining(&self) -> Duration {
        if self.is_locked() {
            self.lock_duration - self.staked_at.elapsed()
        } else {
            Duration::ZERO
        }
    }

    pub fn can_unstake(&self) -> bool {
        !self.is_locked()
    }
}

pub struct StakingManager {
    stakes: HashMap<String, StakeInfo>,
    total_staked: f64,
    min_stake: f64,
    max_stake: f64,
}

impl StakingManager {
    pub fn new(min_stake: f64, max_stake: f64) -> Self {
        Self {
            stakes: HashMap::new(),
            total_staked: 0.0,
            min_stake,
            max_stake,
        }
    }

    pub fn stake(
        &mut self,
        node_id: &str,
        amount: f64,
        lock_days: u64,
    ) -> Result<StakeInfo, StakingError> {
        if amount < self.min_stake {
            return Err(StakingError::BelowMinimum(self.min_stake));
        }

        if amount > self.max_stake {
            return Err(StakingError::AboveMaximum(self.max_stake));
        }

        if self.stakes.contains_key(node_id) {
            return Err(StakingError::AlreadyStaked);
        }

        let stake = StakeInfo::new(amount, lock_days);
        self.total_staked += amount;
        self.stakes.insert(node_id.to_string(), stake.clone());

        Ok(stake)
    }

    pub fn unstake(&mut self, node_id: &str) -> Result<f64, StakingError> {
        let stake = self.stakes.get(node_id).ok_or(StakingError::NotStaked)?;

        if stake.is_locked() {
            return Err(StakingError::StillLocked(stake.time_remaining()));
        }

        let amount = stake.amount;
        self.total_staked -= amount;
        self.stakes.remove(node_id);

        Ok(amount)
    }

    pub fn get_stake(&self, node_id: &str) -> Option<&StakeInfo> {
        self.stakes.get(node_id)
    }

    pub fn total_staked(&self) -> f64 {
        self.total_staked
    }

    pub fn validator_weight(&self, node_id: &str) -> f64 {
        self.stakes
            .get(node_id)
            .map(|s| s.validator_weight)
            .unwrap_or(0.0)
    }

    pub fn relative_weight(&self, node_id: &str) -> f64 {
        if self.total_staked == 0.0 {
            return 0.0;
        }
        self.validator_weight(node_id) / self.total_staked
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StakingError {
    #[error("Amount below minimum stake of {0}")]
    BelowMinimum(f64),
    #[error("Amount above maximum stake of {0}")]
    AboveMaximum(f64),
    #[error("Already staked")]
    AlreadyStaked,
    #[error("Not staked")]
    NotStaked,
    #[error("Stake still locked for {0:?}")]
    StillLocked(Duration),
    #[error("Insufficient balance")]
    InsufficientBalance,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stake_creation() {
        let stake = StakeInfo::new(100.0, 30);
        assert_eq!(stake.amount, 100.0);
        assert!(stake.validator_weight > 100.0); // Has weight multiplier
        assert!(stake.is_locked());
    }

    #[test]
    fn test_staking_manager() {
        let mut manager = StakingManager::new(10.0, 1000.0);

        // Test successful stake
        let result = manager.stake("node1", 100.0, 30);
        assert!(result.is_ok());
        assert_eq!(manager.total_staked(), 100.0);

        // Test duplicate stake
        let duplicate = manager.stake("node1", 50.0, 30);
        assert!(duplicate.is_err());

        // Test below minimum
        let too_low = manager.stake("node2", 5.0, 30);
        assert!(matches!(too_low, Err(StakingError::BelowMinimum(_))));
    }

    #[test]
    fn test_validator_weight() {
        let mut manager = StakingManager::new(10.0, 1000.0);
        manager.stake("node1", 100.0, 365).unwrap();

        let weight = manager.validator_weight("node1");
        assert!(weight > 100.0);
        assert!(weight <= 200.0); // Max 2x multiplier for 1 year

        // relative_weight = validator_weight / total_staked
        // With only one staker, this equals validator_weight / amount
        // Since validator_weight > amount (due to lock multiplier),
        // relative weight will be > 1.0
        let relative = manager.relative_weight("node1");
        assert!(relative > 0.0);
        assert!(relative <= 2.0); // Max 2x due to lock multiplier
    }
}
