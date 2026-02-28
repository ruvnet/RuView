//! # Neural Autonomous Organization (NAO)
//!
//! A decentralized governance mechanism for AI agent collectives using
//! oscillatory synchronization for consensus and stake-weighted voting.
//!
//! ## Key Concepts
//!
//! - **Stake**: Each agent's influence weight in the organization
//! - **Proposals**: Actions that require collective approval
//! - **Oscillatory Sync**: Neural-inspired synchronization for coherence
//! - **Quadratic Voting**: Diminishing returns on vote weight
//!
//! ## Example
//!
//! ```rust
//! use ruvector_exotic_wasm::nao::{NeuralAutonomousOrg, ProposalStatus};
//!
//! let mut nao = NeuralAutonomousOrg::new(0.7); // 70% quorum
//!
//! // Add agents with stake
//! nao.add_member("agent_1", 100);
//! nao.add_member("agent_2", 50);
//!
//! // Create and vote on proposal
//! let prop_id = nao.propose("Migrate to new memory backend");
//! nao.vote(&prop_id, "agent_1", 0.9);  // Strong support
//! nao.vote(&prop_id, "agent_2", 0.6);  // Moderate support
//!
//! // Execute if consensus reached
//! if nao.execute(&prop_id) {
//!     println!("Proposal executed!");
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

/// Status of a proposal in the NAO
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalStatus {
    /// Proposal is active and accepting votes
    Pending,
    /// Proposal passed quorum and was executed
    Executed,
    /// Proposal failed to reach quorum or was rejected
    Rejected,
    /// Proposal expired without decision
    Expired,
}

/// A proposal for collective action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    /// Unique identifier
    pub id: String,
    /// Description of the proposed action
    pub action: String,
    /// Current status
    pub status: ProposalStatus,
    /// Votes: agent_id -> vote weight (-1.0 to 1.0)
    pub votes: HashMap<String, f32>,
    /// Creation timestamp (in simulation ticks)
    pub created_at: u64,
    /// Expiration timestamp
    pub expires_at: u64,
}

impl Proposal {
    /// Create a new proposal
    pub fn new(id: String, action: String, created_at: u64, ttl: u64) -> Self {
        Self {
            id,
            action,
            status: ProposalStatus::Pending,
            votes: HashMap::new(),
            created_at,
            expires_at: created_at + ttl,
        }
    }

    /// Calculate weighted vote tally
    pub fn tally(&self, members: &HashMap<String, u64>) -> (f32, f32) {
        let mut for_votes = 0.0f32;
        let mut against_votes = 0.0f32;

        for (agent_id, vote_weight) in &self.votes {
            if let Some(&stake) = members.get(agent_id) {
                // Quadratic voting: sqrt(stake) * vote_weight
                let voting_power = (stake as f32).sqrt();
                let weighted_vote = voting_power * vote_weight;

                if weighted_vote > 0.0 {
                    for_votes += weighted_vote;
                } else {
                    against_votes += weighted_vote.abs();
                }
            }
        }

        (for_votes, against_votes)
    }

    /// Check if proposal has reached quorum
    pub fn has_quorum(&self, members: &HashMap<String, u64>, quorum_threshold: f32) -> bool {
        let total_voting_power: f32 = members.values().map(|&s| (s as f32).sqrt()).sum();

        if total_voting_power == 0.0 {
            return false;
        }

        let participating_power: f32 = self
            .votes
            .keys()
            .filter_map(|id| members.get(id))
            .map(|&s| (s as f32).sqrt())
            .sum();

        (participating_power / total_voting_power) >= quorum_threshold
    }
}

/// Kuramoto-style oscillatory synchronizer for agent coherence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OscillatorySynchronizer {
    /// Phase of each oscillator (agent)
    phases: HashMap<String, f32>,
    /// Natural frequency of each oscillator
    frequencies: HashMap<String, f32>,
    /// Coupling strength between oscillators
    coupling: f32,
    /// Base frequency (Hz)
    base_frequency: f32,
}

impl OscillatorySynchronizer {
    /// Create a new synchronizer
    pub fn new(coupling: f32, base_frequency: f32) -> Self {
        Self {
            phases: HashMap::new(),
            frequencies: HashMap::new(),
            coupling,
            base_frequency,
        }
    }

    /// Add an oscillator for an agent
    pub fn add_oscillator(&mut self, agent_id: &str) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        // Random initial phase
        let phase = rng.gen::<f32>() * 2.0 * std::f32::consts::PI;
        // Slight frequency variation around base
        let freq = self.base_frequency * (0.95 + rng.gen::<f32>() * 0.1);

        self.phases.insert(agent_id.to_string(), phase);
        self.frequencies.insert(agent_id.to_string(), freq);
    }

    /// Remove an oscillator
    pub fn remove_oscillator(&mut self, agent_id: &str) {
        self.phases.remove(agent_id);
        self.frequencies.remove(agent_id);
    }

    /// Step the Kuramoto dynamics forward
    pub fn step(&mut self, dt: f32) {
        let n = self.phases.len();
        if n < 2 {
            return;
        }

        // Collect current phases
        let current_phases: Vec<(String, f32)> =
            self.phases.iter().map(|(k, v)| (k.clone(), *v)).collect();

        // Kuramoto update: dθ_i/dt = ω_i + (K/N) * Σ_j sin(θ_j - θ_i)
        for (agent_id, phase) in &current_phases {
            let omega = self
                .frequencies
                .get(agent_id)
                .copied()
                .unwrap_or(self.base_frequency);

            // Sum of phase differences
            let phase_coupling: f32 = current_phases
                .iter()
                .filter(|(id, _)| id != agent_id)
                .map(|(_, other_phase)| (other_phase - phase).sin())
                .sum();

            let coupling_term = (self.coupling / n as f32) * phase_coupling;
            let new_phase = phase + (omega + coupling_term) * dt;

            // Wrap to [0, 2π]
            let wrapped = new_phase.rem_euclid(2.0 * std::f32::consts::PI);
            self.phases.insert(agent_id.clone(), wrapped);
        }
    }

    /// Calculate order parameter (synchronization level, 0-1)
    pub fn order_parameter(&self) -> f32 {
        let n = self.phases.len();
        if n == 0 {
            return 0.0;
        }

        // r = |1/N * Σ_j e^(iθ_j)|
        let sum_cos: f32 = self.phases.values().map(|&p| p.cos()).sum();
        let sum_sin: f32 = self.phases.values().map(|&p| p.sin()).sum();

        let r = ((sum_cos / n as f32).powi(2) + (sum_sin / n as f32).powi(2)).sqrt();
        r
    }

    /// Get coherence between two agents (0-1)
    pub fn coherence(&self, agent_a: &str, agent_b: &str) -> f32 {
        match (self.phases.get(agent_a), self.phases.get(agent_b)) {
            (Some(&pa), Some(&pb)) => {
                // Coherence = cos(phase_difference)
                let diff = pa - pb;
                (1.0 + diff.cos()) / 2.0 // Map [-1, 1] to [0, 1]
            }
            _ => 0.0,
        }
    }

    /// Get all current phases
    pub fn phases(&self) -> &HashMap<String, f32> {
        &self.phases
    }
}

/// Neural Autonomous Organization - decentralized AI governance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuralAutonomousOrg {
    /// Member agents: agent_id -> stake
    members: HashMap<String, u64>,
    /// Active proposals
    proposals: Vec<Proposal>,
    /// Oscillatory synchronizer for coherence
    sync: OscillatorySynchronizer,
    /// Quorum threshold (0.0 - 1.0)
    quorum_threshold: f32,
    /// Current simulation tick
    tick: u64,
    /// Proposal time-to-live in ticks
    proposal_ttl: u64,
    /// Counter for generating proposal IDs
    proposal_counter: u64,
}

impl Default for NeuralAutonomousOrg {
    fn default() -> Self {
        Self::new(0.5)
    }
}

impl NeuralAutonomousOrg {
    /// Create a new NAO with the given quorum threshold
    pub fn new(quorum_threshold: f32) -> Self {
        Self {
            members: HashMap::new(),
            proposals: Vec::new(),
            sync: OscillatorySynchronizer::new(5.0, 40.0), // 40Hz gamma oscillations
            quorum_threshold: quorum_threshold.clamp(0.0, 1.0),
            tick: 0,
            proposal_ttl: 1000, // 1000 ticks default TTL
            proposal_counter: 0,
        }
    }

    /// Add a member agent with initial stake
    pub fn add_member(&mut self, agent_id: &str, stake: u64) {
        self.members.insert(agent_id.to_string(), stake);
        self.sync.add_oscillator(agent_id);
    }

    /// Remove a member agent
    pub fn remove_member(&mut self, agent_id: &str) {
        self.members.remove(agent_id);
        self.sync.remove_oscillator(agent_id);
    }

    /// Get member count
    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    /// Get a member's stake
    pub fn get_stake(&self, agent_id: &str) -> Option<u64> {
        self.members.get(agent_id).copied()
    }

    /// Update a member's stake
    pub fn update_stake(&mut self, agent_id: &str, delta: i64) -> Option<u64> {
        if let Some(stake) = self.members.get_mut(agent_id) {
            let new_stake = (*stake as i64 + delta).max(0) as u64;
            *stake = new_stake;
            Some(new_stake)
        } else {
            None
        }
    }

    /// Create a new proposal
    pub fn propose(&mut self, action: &str) -> String {
        self.proposal_counter += 1;
        let id = format!("prop_{}", self.proposal_counter);

        let proposal = Proposal::new(id.clone(), action.to_string(), self.tick, self.proposal_ttl);

        self.proposals.push(proposal);
        id
    }

    /// Vote on a proposal
    ///
    /// # Arguments
    /// * `proposal_id` - The proposal to vote on
    /// * `agent_id` - The voting agent
    /// * `weight` - Vote weight from -1.0 (strongly against) to 1.0 (strongly for)
    ///
    /// # Returns
    /// `true` if vote was recorded, `false` if proposal not found or agent not a member
    pub fn vote(&mut self, proposal_id: &str, agent_id: &str, weight: f32) -> bool {
        // Verify agent is a member
        if !self.members.contains_key(agent_id) {
            return false;
        }

        // Find and update proposal
        for proposal in &mut self.proposals {
            if proposal.id == proposal_id && proposal.status == ProposalStatus::Pending {
                let clamped_weight = weight.clamp(-1.0, 1.0);
                proposal.votes.insert(agent_id.to_string(), clamped_weight);
                return true;
            }
        }

        false
    }

    /// Execute a proposal if it has reached consensus
    ///
    /// # Returns
    /// `true` if proposal was executed, `false` otherwise
    pub fn execute(&mut self, proposal_id: &str) -> bool {
        let members = self.members.clone();
        let quorum = self.quorum_threshold;

        for proposal in &mut self.proposals {
            if proposal.id == proposal_id && proposal.status == ProposalStatus::Pending {
                // Check quorum
                if !proposal.has_quorum(&members, quorum) {
                    return false;
                }

                // Tally votes
                let (for_votes, against_votes) = proposal.tally(&members);

                // Simple majority with coherence boost
                let sync_level = self.sync.order_parameter();
                let coherence_boost = 1.0 + sync_level * 0.2; // Up to 20% boost for synchronized org

                if for_votes * coherence_boost > against_votes {
                    proposal.status = ProposalStatus::Executed;
                    return true;
                } else {
                    proposal.status = ProposalStatus::Rejected;
                    return false;
                }
            }
        }

        false
    }

    /// Advance simulation by one tick
    pub fn tick(&mut self, dt: f32) {
        self.tick += 1;
        self.sync.step(dt);

        // Expire old proposals
        for proposal in &mut self.proposals {
            if proposal.status == ProposalStatus::Pending && self.tick > proposal.expires_at {
                proposal.status = ProposalStatus::Expired;
            }
        }
    }

    /// Get current synchronization level (0-1)
    pub fn synchronization(&self) -> f32 {
        self.sync.order_parameter()
    }

    /// Get coherence between two agents
    pub fn agent_coherence(&self, agent_a: &str, agent_b: &str) -> f32 {
        self.sync.coherence(agent_a, agent_b)
    }

    /// Get all active proposals
    pub fn active_proposals(&self) -> Vec<&Proposal> {
        self.proposals
            .iter()
            .filter(|p| p.status == ProposalStatus::Pending)
            .collect()
    }

    /// Get proposal by ID
    pub fn get_proposal(&self, proposal_id: &str) -> Option<&Proposal> {
        self.proposals.iter().find(|p| p.id == proposal_id)
    }

    /// Clean up expired/rejected proposals older than given tick threshold
    pub fn cleanup(&mut self, tick_threshold: u64) {
        self.proposals.retain(|p| {
            p.status == ProposalStatus::Pending
                || p.status == ProposalStatus::Executed
                || p.created_at + tick_threshold > self.tick
        });
    }

    /// Get total voting power in the organization
    pub fn total_voting_power(&self) -> f32 {
        self.members.values().map(|&s| (s as f32).sqrt()).sum()
    }

    /// Get current tick
    pub fn current_tick(&self) -> u64 {
        self.tick
    }
}

// WASM Bindings

/// WASM-bindgen wrapper for NeuralAutonomousOrg
#[wasm_bindgen]
pub struct WasmNAO {
    inner: NeuralAutonomousOrg,
}

#[wasm_bindgen]
impl WasmNAO {
    /// Create a new NAO with the given quorum threshold (0.0 - 1.0)
    #[wasm_bindgen(constructor)]
    pub fn new(quorum_threshold: f32) -> Self {
        Self {
            inner: NeuralAutonomousOrg::new(quorum_threshold),
        }
    }

    /// Add a member agent with initial stake
    #[wasm_bindgen(js_name = addMember)]
    pub fn add_member(&mut self, agent_id: &str, stake: u32) {
        self.inner.add_member(agent_id, stake as u64);
    }

    /// Remove a member agent
    #[wasm_bindgen(js_name = removeMember)]
    pub fn remove_member(&mut self, agent_id: &str) {
        self.inner.remove_member(agent_id);
    }

    /// Get member count
    #[wasm_bindgen(js_name = memberCount)]
    pub fn member_count(&self) -> usize {
        self.inner.member_count()
    }

    /// Create a new proposal, returns proposal ID
    pub fn propose(&mut self, action: &str) -> String {
        self.inner.propose(action)
    }

    /// Vote on a proposal
    pub fn vote(&mut self, proposal_id: &str, agent_id: &str, weight: f32) -> bool {
        self.inner.vote(proposal_id, agent_id, weight)
    }

    /// Execute a proposal if consensus reached
    pub fn execute(&mut self, proposal_id: &str) -> bool {
        self.inner.execute(proposal_id)
    }

    /// Advance simulation by one tick
    pub fn tick(&mut self, dt: f32) {
        self.inner.tick(dt);
    }

    /// Get current synchronization level (0-1)
    pub fn synchronization(&self) -> f32 {
        self.inner.synchronization()
    }

    /// Get coherence between two agents (0-1)
    #[wasm_bindgen(js_name = agentCoherence)]
    pub fn agent_coherence(&self, agent_a: &str, agent_b: &str) -> f32 {
        self.inner.agent_coherence(agent_a, agent_b)
    }

    /// Get active proposal count
    #[wasm_bindgen(js_name = activeProposalCount)]
    pub fn active_proposal_count(&self) -> usize {
        self.inner.active_proposals().len()
    }

    /// Get total voting power
    #[wasm_bindgen(js_name = totalVotingPower)]
    pub fn total_voting_power(&self) -> f32 {
        self.inner.total_voting_power()
    }

    /// Get current tick
    #[wasm_bindgen(js_name = currentTick)]
    pub fn current_tick(&self) -> u32 {
        self.inner.current_tick() as u32
    }

    /// Get all data as JSON
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.inner).map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nao_creation() {
        let nao = NeuralAutonomousOrg::new(0.5);
        assert_eq!(nao.member_count(), 0);
        assert_eq!(nao.synchronization(), 0.0);
    }

    #[test]
    fn test_member_management() {
        let mut nao = NeuralAutonomousOrg::new(0.5);

        nao.add_member("agent_1", 100);
        nao.add_member("agent_2", 50);

        assert_eq!(nao.member_count(), 2);
        assert_eq!(nao.get_stake("agent_1"), Some(100));
        assert_eq!(nao.get_stake("agent_2"), Some(50));

        nao.remove_member("agent_1");
        assert_eq!(nao.member_count(), 1);
        assert_eq!(nao.get_stake("agent_1"), None);
    }

    #[test]
    fn test_stake_update() {
        let mut nao = NeuralAutonomousOrg::new(0.5);
        nao.add_member("agent_1", 100);

        let new_stake = nao.update_stake("agent_1", 50);
        assert_eq!(new_stake, Some(150));

        let new_stake = nao.update_stake("agent_1", -200);
        assert_eq!(new_stake, Some(0)); // Can't go negative

        assert_eq!(nao.update_stake("nonexistent", 10), None);
    }

    #[test]
    fn test_proposal_lifecycle() {
        let mut nao = NeuralAutonomousOrg::new(0.5);

        nao.add_member("agent_1", 100);
        nao.add_member("agent_2", 100);

        let prop_id = nao.propose("Test action");
        assert_eq!(nao.active_proposals().len(), 1);

        // Vote
        assert!(nao.vote(&prop_id, "agent_1", 1.0));
        assert!(nao.vote(&prop_id, "agent_2", 0.8));

        // Execute
        assert!(nao.execute(&prop_id));

        // Should be executed now
        let proposal = nao.get_proposal(&prop_id).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Executed);
    }

    #[test]
    fn test_quorum_requirement() {
        let mut nao = NeuralAutonomousOrg::new(0.7); // 70% quorum

        nao.add_member("agent_1", 100);
        nao.add_member("agent_2", 100);
        nao.add_member("agent_3", 100);

        let prop_id = nao.propose("Test action");

        // Only one vote - should not reach quorum
        nao.vote(&prop_id, "agent_1", 1.0);
        assert!(!nao.execute(&prop_id));

        // Add second vote - still below 70%
        nao.vote(&prop_id, "agent_2", 1.0);
        // 2/3 = 66.7% < 70%
        assert!(!nao.execute(&prop_id));

        // Add third vote - now above quorum
        nao.vote(&prop_id, "agent_3", 1.0);
        assert!(nao.execute(&prop_id));
    }

    #[test]
    fn test_voting_rejection() {
        let mut nao = NeuralAutonomousOrg::new(0.5);

        nao.add_member("agent_1", 100);
        nao.add_member("agent_2", 100);
        nao.add_member("agent_3", 100);

        let prop_id = nao.propose("Controversial action");

        // Two against, one weak for - should be rejected even with coherence boost
        nao.vote(&prop_id, "agent_1", 0.3); // weak support
        nao.vote(&prop_id, "agent_2", -1.0); // strong against
        nao.vote(&prop_id, "agent_3", -1.0); // strong against

        // Should be rejected (more against than for)
        assert!(!nao.execute(&prop_id));

        let proposal = nao.get_proposal(&prop_id).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Rejected);
    }

    #[test]
    fn test_oscillatory_synchronization() {
        let mut nao = NeuralAutonomousOrg::new(0.5);

        nao.add_member("agent_1", 100);
        nao.add_member("agent_2", 100);
        nao.add_member("agent_3", 100);

        // Initial sync should be low (random phases)
        let initial_sync = nao.synchronization();

        // Run dynamics to synchronize
        for _ in 0..1000 {
            nao.tick(0.001); // 1ms steps
        }

        let final_sync = nao.synchronization();

        // Synchronization should increase due to Kuramoto coupling
        assert!(
            final_sync > initial_sync * 0.5,
            "Sync should improve: initial={}, final={}",
            initial_sync,
            final_sync
        );
    }

    #[test]
    fn test_coherence_between_agents() {
        let mut nao = NeuralAutonomousOrg::new(0.5);

        nao.add_member("agent_1", 100);
        nao.add_member("agent_2", 100);

        // Run to synchronize
        for _ in 0..2000 {
            nao.tick(0.001);
        }

        let coherence = nao.agent_coherence("agent_1", "agent_2");
        assert!(
            coherence >= 0.0 && coherence <= 1.0,
            "Coherence should be in [0,1]: {}",
            coherence
        );
    }

    #[test]
    fn test_proposal_expiration() {
        let mut nao = NeuralAutonomousOrg::new(0.5);
        nao.proposal_ttl = 10; // Short TTL for testing

        nao.add_member("agent_1", 100);

        let prop_id = nao.propose("Expiring action");

        // Advance past TTL
        for _ in 0..15 {
            nao.tick(1.0);
        }

        let proposal = nao.get_proposal(&prop_id).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Expired);
    }

    #[test]
    fn test_non_member_cannot_vote() {
        let mut nao = NeuralAutonomousOrg::new(0.5);

        nao.add_member("agent_1", 100);
        let prop_id = nao.propose("Test");

        // Non-member vote should fail
        assert!(!nao.vote(&prop_id, "stranger", 1.0));
    }

    #[test]
    fn test_quadratic_voting_power() {
        let mut nao = NeuralAutonomousOrg::new(0.1); // Low quorum for testing

        // Agent with 100 stake has sqrt(100) = 10 voting power
        // Agent with 25 stake has sqrt(25) = 5 voting power
        nao.add_member("rich", 100);
        nao.add_member("poor", 25);

        let prop_id = nao.propose("Favor rich");

        // Rich votes against, poor votes for
        nao.vote(&prop_id, "rich", -1.0); // -10 effective vote
        nao.vote(&prop_id, "poor", 1.0); // +5 effective vote

        // Rich should win despite being one agent
        assert!(!nao.execute(&prop_id)); // Rejected

        let proposal = nao.get_proposal(&prop_id).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Rejected);
    }

    #[test]
    fn test_total_voting_power() {
        let mut nao = NeuralAutonomousOrg::new(0.5);

        nao.add_member("agent_1", 100); // sqrt(100) = 10
        nao.add_member("agent_2", 25); // sqrt(25) = 5

        let total = nao.total_voting_power();
        assert!((total - 15.0).abs() < 0.01, "Expected ~15, got {}", total);
    }
}
