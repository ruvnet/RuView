//! rUv Token Integration for QuDAG

mod governance;
mod rewards;
mod staking;

pub use governance::{
    GovernanceError, GovernanceSystem, GovernanceVote, Proposal, ProposalStatus, ProposalType,
    VoteChoice,
};
pub use rewards::{RewardCalculator, RewardClaim, RewardSource};
pub use staking::{StakeInfo, StakingError, StakingManager};

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_staking_integration() {
        let mut manager = StakingManager::new(10.0, 1000.0);
        let stake = manager.stake("node1", 100.0, 30).unwrap();
        assert_eq!(stake.amount, 100.0);
        assert_eq!(manager.total_staked(), 100.0);
    }

    #[test]
    fn test_rewards_calculation() {
        let calculator = RewardCalculator::default();
        let reward = calculator.pattern_validation_reward(1.0, 0.9);
        assert!(reward > 0.0);
    }

    #[test]
    fn test_governance_voting() {
        let mut gov = GovernanceSystem::default();
        let proposal_id = gov.create_proposal(
            "Test Proposal".to_string(),
            "Test Description".to_string(),
            "proposer1".to_string(),
            ProposalType::ParameterChange,
            Duration::from_secs(86400),
        );

        gov.vote("voter1".to_string(), &proposal_id, VoteChoice::For, 100.0)
            .unwrap();
        let tally = gov.tally(&proposal_id, 1000.0).unwrap();
        assert_eq!(tally.for_weight, 100.0);
    }
}
