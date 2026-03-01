//! Governance Voting System

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Proposal {
    pub id: String,
    pub title: String,
    pub description: String,
    pub proposer: String,
    pub created_at: std::time::Instant,
    pub voting_ends: std::time::Duration,
    pub proposal_type: ProposalType,
    pub status: ProposalStatus,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProposalType {
    ParameterChange,
    PatternPolicy,
    RewardAdjustment,
    ProtocolUpgrade,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProposalStatus {
    Active,
    Passed,
    Failed,
    Executed,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct GovernanceVote {
    pub voter: String,
    pub proposal_id: String,
    pub vote: VoteChoice,
    pub weight: f64,
    pub timestamp: std::time::Instant,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VoteChoice {
    For,
    Against,
    Abstain,
}

pub struct GovernanceSystem {
    proposals: HashMap<String, Proposal>,
    votes: HashMap<String, Vec<GovernanceVote>>,
    quorum_threshold: f64,   // Minimum participation (e.g., 0.1 = 10%)
    approval_threshold: f64, // Minimum approval (e.g., 0.67 = 67%)
}

impl GovernanceSystem {
    pub fn new(quorum_threshold: f64, approval_threshold: f64) -> Self {
        Self {
            proposals: HashMap::new(),
            votes: HashMap::new(),
            quorum_threshold,
            approval_threshold,
        }
    }

    pub fn create_proposal(
        &mut self,
        title: String,
        description: String,
        proposer: String,
        proposal_type: ProposalType,
        voting_duration: std::time::Duration,
    ) -> String {
        let id = format!("prop_{}", rand::random::<u64>());

        let proposal = Proposal {
            id: id.clone(),
            title,
            description,
            proposer,
            created_at: std::time::Instant::now(),
            voting_ends: voting_duration,
            proposal_type,
            status: ProposalStatus::Active,
        };

        self.proposals.insert(id.clone(), proposal);
        self.votes.insert(id.clone(), Vec::new());

        id
    }

    pub fn vote(
        &mut self,
        voter: String,
        proposal_id: &str,
        choice: VoteChoice,
        stake_weight: f64,
    ) -> Result<(), GovernanceError> {
        let proposal = self
            .proposals
            .get(proposal_id)
            .ok_or(GovernanceError::ProposalNotFound)?;

        if proposal.status != ProposalStatus::Active {
            return Err(GovernanceError::ProposalNotActive);
        }

        if proposal.created_at.elapsed() > proposal.voting_ends {
            return Err(GovernanceError::VotingEnded);
        }

        // Check if already voted
        let votes = self.votes.get_mut(proposal_id).unwrap();
        if votes.iter().any(|v| v.voter == voter) {
            return Err(GovernanceError::AlreadyVoted);
        }

        votes.push(GovernanceVote {
            voter,
            proposal_id: proposal_id.to_string(),
            vote: choice,
            weight: stake_weight,
            timestamp: std::time::Instant::now(),
        });

        Ok(())
    }

    pub fn tally(&self, proposal_id: &str, total_stake: f64) -> Option<VoteTally> {
        let votes = self.votes.get(proposal_id)?;

        let mut for_weight = 0.0;
        let mut against_weight = 0.0;
        let mut abstain_weight = 0.0;

        for vote in votes {
            match vote.vote {
                VoteChoice::For => for_weight += vote.weight,
                VoteChoice::Against => against_weight += vote.weight,
                VoteChoice::Abstain => abstain_weight += vote.weight,
            }
        }

        let total_voted = for_weight + against_weight + abstain_weight;
        let participation = total_voted / total_stake;
        let approval = if for_weight + against_weight > 0.0 {
            for_weight / (for_weight + against_weight)
        } else {
            0.0
        };

        let quorum_met = participation >= self.quorum_threshold;
        let approved = approval >= self.approval_threshold && quorum_met;

        Some(VoteTally {
            for_weight,
            against_weight,
            abstain_weight,
            participation,
            approval,
            quorum_met,
            approved,
        })
    }

    pub fn finalize(
        &mut self,
        proposal_id: &str,
        total_stake: f64,
    ) -> Result<ProposalStatus, GovernanceError> {
        // First, validate the proposal without holding a mutable borrow
        {
            let proposal = self
                .proposals
                .get(proposal_id)
                .ok_or(GovernanceError::ProposalNotFound)?;

            if proposal.status != ProposalStatus::Active {
                return Err(GovernanceError::ProposalNotActive);
            }

            if proposal.created_at.elapsed() < proposal.voting_ends {
                return Err(GovernanceError::VotingNotEnded);
            }
        }

        // Calculate tally (immutable borrow)
        let tally = self
            .tally(proposal_id, total_stake)
            .ok_or(GovernanceError::ProposalNotFound)?;

        let new_status = if tally.approved {
            ProposalStatus::Passed
        } else {
            ProposalStatus::Failed
        };

        // Now update the status (mutable borrow)
        let proposal = self.proposals.get_mut(proposal_id).unwrap();
        proposal.status = new_status;
        Ok(new_status)
    }

    pub fn get_proposal(&self, proposal_id: &str) -> Option<&Proposal> {
        self.proposals.get(proposal_id)
    }

    pub fn active_proposals(&self) -> Vec<&Proposal> {
        self.proposals
            .values()
            .filter(|p| p.status == ProposalStatus::Active)
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct VoteTally {
    pub for_weight: f64,
    pub against_weight: f64,
    pub abstain_weight: f64,
    pub participation: f64,
    pub approval: f64,
    pub quorum_met: bool,
    pub approved: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum GovernanceError {
    #[error("Proposal not found")]
    ProposalNotFound,
    #[error("Proposal not active")]
    ProposalNotActive,
    #[error("Voting has ended")]
    VotingEnded,
    #[error("Voting has not ended")]
    VotingNotEnded,
    #[error("Already voted")]
    AlreadyVoted,
    #[error("Insufficient stake to propose")]
    InsufficientStake,
}

impl Default for GovernanceSystem {
    fn default() -> Self {
        Self::new(0.1, 0.67) // 10% quorum, 67% approval
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_proposal_creation() {
        let mut gov = GovernanceSystem::default();
        let id = gov.create_proposal(
            "Test".to_string(),
            "Description".to_string(),
            "proposer1".to_string(),
            ProposalType::ParameterChange,
            Duration::from_secs(86400),
        );

        let proposal = gov.get_proposal(&id).unwrap();
        assert_eq!(proposal.title, "Test");
        assert_eq!(proposal.status, ProposalStatus::Active);
    }

    #[test]
    fn test_voting() {
        let mut gov = GovernanceSystem::default();
        let id = gov.create_proposal(
            "Test".to_string(),
            "Description".to_string(),
            "proposer1".to_string(),
            ProposalType::ParameterChange,
            Duration::from_secs(86400),
        );

        // First vote succeeds
        assert!(gov
            .vote("voter1".to_string(), &id, VoteChoice::For, 100.0)
            .is_ok());

        // Duplicate vote fails
        assert!(matches!(
            gov.vote("voter1".to_string(), &id, VoteChoice::For, 50.0),
            Err(GovernanceError::AlreadyVoted)
        ));
    }

    #[test]
    fn test_tally() {
        let mut gov = GovernanceSystem::new(0.1, 0.5);
        let id = gov.create_proposal(
            "Test".to_string(),
            "Description".to_string(),
            "proposer1".to_string(),
            ProposalType::ParameterChange,
            Duration::from_secs(86400),
        );

        gov.vote("voter1".to_string(), &id, VoteChoice::For, 700.0)
            .unwrap();
        gov.vote("voter2".to_string(), &id, VoteChoice::Against, 300.0)
            .unwrap();

        let tally = gov.tally(&id, 10000.0).unwrap();
        assert_eq!(tally.for_weight, 700.0);
        assert_eq!(tally.against_weight, 300.0);
        assert_eq!(tally.participation, 0.1); // 1000/10000
        assert_eq!(tally.approval, 0.7); // 700/1000
        assert!(tally.quorum_met);
        assert!(tally.approved);
    }

    #[test]
    fn test_quorum_not_met() {
        let mut gov = GovernanceSystem::new(0.5, 0.67);
        let id = gov.create_proposal(
            "Test".to_string(),
            "Description".to_string(),
            "proposer1".to_string(),
            ProposalType::ParameterChange,
            Duration::from_secs(86400),
        );

        gov.vote("voter1".to_string(), &id, VoteChoice::For, 100.0)
            .unwrap();

        let tally = gov.tally(&id, 10000.0).unwrap();
        assert!(!tally.quorum_met); // Only 1% participation
        assert!(!tally.approved);
    }
}
