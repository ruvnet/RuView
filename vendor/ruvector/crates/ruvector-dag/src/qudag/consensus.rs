//! Consensus Validation

#[derive(Debug, Clone)]
pub struct ConsensusResult {
    pub round: u64,
    pub proposal_id: String,
    pub accepted: bool,
    pub stake_weight: f64,
    pub validator_count: usize,
}

#[derive(Debug, Clone)]
pub struct Vote {
    pub voter_id: String,
    pub proposal_id: String,
    pub approve: bool,
    pub stake_weight: f64,
    pub signature: Vec<u8>, // ML-DSA signature
}

impl Vote {
    pub fn new(voter_id: String, proposal_id: String, approve: bool, stake_weight: f64) -> Self {
        Self {
            voter_id,
            proposal_id,
            approve,
            stake_weight,
            signature: Vec::new(),
        }
    }

    pub fn sign(&mut self, _private_key: &[u8]) {
        // Would use ML-DSA to sign
        self.signature = vec![0u8; 64];
    }

    pub fn verify(&self, _public_key: &[u8]) -> bool {
        // Would verify ML-DSA signature
        !self.signature.is_empty()
    }
}

#[allow(dead_code)]
pub struct ConsensusTracker {
    proposals: std::collections::HashMap<String, Vec<Vote>>,
    threshold: f64, // Stake threshold for acceptance (e.g., 0.67)
}

#[allow(dead_code)]
impl ConsensusTracker {
    pub fn new(threshold: f64) -> Self {
        Self {
            proposals: std::collections::HashMap::new(),
            threshold,
        }
    }

    pub fn add_vote(&mut self, vote: Vote) {
        self.proposals
            .entry(vote.proposal_id.clone())
            .or_default()
            .push(vote);
    }

    pub fn check_consensus(&self, proposal_id: &str) -> Option<ConsensusResult> {
        let votes = self.proposals.get(proposal_id)?;

        let total_stake: f64 = votes.iter().map(|v| v.stake_weight).sum();
        let approve_stake: f64 = votes
            .iter()
            .filter(|v| v.approve)
            .map(|v| v.stake_weight)
            .sum();

        let accepted = approve_stake / total_stake > self.threshold;

        Some(ConsensusResult {
            round: 0,
            proposal_id: proposal_id.to_string(),
            accepted,
            stake_weight: total_stake,
            validator_count: votes.len(),
        })
    }
}
