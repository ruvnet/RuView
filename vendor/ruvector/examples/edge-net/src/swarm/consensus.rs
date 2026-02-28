//! Entropy-Based Consensus for Swarm Intelligence
//!
//! Implements entropy-minimizing negotiation between swarm nodes.
//! Consensus is achieved when belief entropy falls below threshold,
//! indicating the swarm has converged to a shared decision.
//!
//! ## Theory
//!
//! Shannon entropy measures uncertainty in a probability distribution:
//!   H = -SUM(p_i * log2(p_i))
//!
//! Low entropy = high certainty = convergence
//! High entropy = uncertainty = negotiation needed
//!
//! ## Algorithm
//!
//! 1. Each node maintains belief probabilities for decisions
//! 2. Nodes exchange beliefs with peers (gossip)
//! 3. Beliefs are averaged: p_new = 0.5 * p_local + 0.5 * p_peer
//! 4. Convergence when H < threshold (e.g., 0.1)
//!
//! ## References
//!
//! - Degroot consensus model
//! - Entropy-based stopping criteria

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use rustc_hash::FxHashMap;
use std::sync::RwLock;

// ============================================================================
// Decision Types
// ============================================================================

/// A decision that the swarm can make
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum Decision {
    /// Accept a proposed action
    Accept(u64),
    /// Reject a proposed action
    Reject(u64),
    /// Route task to specific node
    RouteToNode(u32),
    /// Allocate resources
    Allocate(u32),
    /// Elect a coordinator
    ElectCoordinator(u32),
    /// Custom decision with ID
    Custom(u64),
}

impl Decision {
    /// Get decision ID for hashing
    pub fn id(&self) -> u64 {
        match self {
            Decision::Accept(id) => *id,
            Decision::Reject(id) => *id | 0x8000_0000_0000_0000,
            Decision::RouteToNode(node) => *node as u64 | 0x1000_0000_0000_0000,
            Decision::Allocate(amount) => *amount as u64 | 0x2000_0000_0000_0000,
            Decision::ElectCoordinator(node) => *node as u64 | 0x3000_0000_0000_0000,
            Decision::Custom(id) => *id | 0x4000_0000_0000_0000,
        }
    }
}

// ============================================================================
// Entropy-Based Consensus
// ============================================================================

/// Configuration for entropy consensus
#[derive(Clone, Debug)]
pub struct EntropyConsensusConfig {
    /// Entropy threshold for convergence (lower = stricter)
    pub entropy_threshold: f32,
    /// Maximum negotiation rounds before timeout
    pub max_negotiation_rounds: usize,
    /// Mixing weight for local beliefs (0.0-1.0)
    pub local_weight: f32,
    /// Minimum probability to consider (prevents log(0))
    pub min_probability: f32,
    /// Enable temperature-based annealing
    pub enable_annealing: bool,
    /// Initial temperature for annealing
    pub initial_temperature: f32,
}

impl Default for EntropyConsensusConfig {
    fn default() -> Self {
        Self {
            entropy_threshold: 0.1,
            max_negotiation_rounds: 50,
            local_weight: 0.5,
            min_probability: 1e-6,
            enable_annealing: true,
            initial_temperature: 1.0,
        }
    }
}

/// Entropy-based consensus engine for swarm decisions
#[wasm_bindgen]
pub struct EntropyConsensus {
    /// Belief probabilities for each decision
    beliefs: RwLock<FxHashMap<u64, f32>>,
    /// Entropy threshold for convergence
    entropy_threshold: f32,
    /// Completed negotiation rounds
    negotiation_rounds: RwLock<usize>,
    /// Maximum rounds allowed
    max_rounds: usize,
    /// Mixing weight for local beliefs
    local_weight: f32,
    /// Minimum probability (prevents log(0))
    min_prob: f32,
    /// Current temperature for annealing
    temperature: RwLock<f32>,
    /// Initial temperature
    initial_temperature: f32,
    /// Enable annealing
    enable_annealing: bool,
    /// History of entropy values (for monitoring convergence)
    entropy_history: RwLock<Vec<f32>>,
}

#[wasm_bindgen]
impl EntropyConsensus {
    /// Create new entropy consensus with default configuration
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::with_config(EntropyConsensusConfig::default())
    }

    /// Create with custom entropy threshold
    #[wasm_bindgen(js_name = withThreshold)]
    pub fn with_threshold(threshold: f32) -> Self {
        let mut config = EntropyConsensusConfig::default();
        config.entropy_threshold = threshold.clamp(0.01, 2.0);
        Self::with_config(config)
    }

    /// Get current entropy of belief distribution
    #[wasm_bindgen]
    pub fn entropy(&self) -> f32 {
        let beliefs = self.beliefs.read().unwrap();
        self.compute_entropy(&beliefs)
    }

    /// Check if consensus has been reached
    #[wasm_bindgen]
    pub fn converged(&self) -> bool {
        self.entropy() < self.entropy_threshold
    }

    /// Get the winning decision (if converged)
    #[wasm_bindgen(js_name = getDecision)]
    pub fn get_decision(&self) -> Option<u64> {
        if !self.converged() {
            return None;
        }

        let beliefs = self.beliefs.read().unwrap();
        beliefs.iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(&id, _)| id)
    }

    /// Get number of negotiation rounds completed
    #[wasm_bindgen(js_name = getRounds)]
    pub fn get_rounds(&self) -> usize {
        *self.negotiation_rounds.read().unwrap()
    }

    /// Get the entropy threshold for convergence
    #[wasm_bindgen(js_name = getEntropyThreshold)]
    pub fn get_entropy_threshold(&self) -> f32 {
        self.entropy_threshold
    }

    /// Check if negotiation has timed out
    #[wasm_bindgen(js_name = hasTimedOut)]
    pub fn has_timed_out(&self) -> bool {
        *self.negotiation_rounds.read().unwrap() >= self.max_rounds
    }

    /// Get belief probability for a decision
    #[wasm_bindgen(js_name = getBelief)]
    pub fn get_belief(&self, decision_id: u64) -> f32 {
        self.beliefs.read().unwrap()
            .get(&decision_id)
            .copied()
            .unwrap_or(0.0)
    }

    /// Set initial belief for a decision
    #[wasm_bindgen(js_name = setBelief)]
    pub fn set_belief(&self, decision_id: u64, probability: f32) {
        let prob = probability.clamp(self.min_prob, 1.0);
        self.beliefs.write().unwrap().insert(decision_id, prob);
        self.normalize_beliefs();
    }

    /// Set belief without normalizing (for batch updates)
    /// Call normalize_beliefs() after all set_belief_raw calls
    pub fn set_belief_raw(&self, decision_id: u64, probability: f32) {
        let prob = probability.clamp(self.min_prob, 1.0);
        self.beliefs.write().unwrap().insert(decision_id, prob);
    }

    /// Manually trigger normalization (for use after set_belief_raw)
    pub fn finalize_beliefs(&self) {
        self.normalize_beliefs();
    }

    /// Get number of decision options
    #[wasm_bindgen(js_name = optionCount)]
    pub fn option_count(&self) -> usize {
        self.beliefs.read().unwrap().len()
    }

    /// Get current temperature (for annealing)
    #[wasm_bindgen(js_name = getTemperature)]
    pub fn get_temperature(&self) -> f32 {
        *self.temperature.read().unwrap()
    }

    /// Get entropy history as JSON
    #[wasm_bindgen(js_name = getEntropyHistory)]
    pub fn get_entropy_history(&self) -> String {
        let history = self.entropy_history.read().unwrap();
        serde_json::to_string(&*history).unwrap_or_else(|_| "[]".to_string())
    }

    /// Get consensus statistics as JSON
    #[wasm_bindgen(js_name = getStats)]
    pub fn get_stats(&self) -> String {
        let entropy = self.entropy();
        let rounds = *self.negotiation_rounds.read().unwrap();
        let converged = entropy < self.entropy_threshold;
        let temp = *self.temperature.read().unwrap();
        let options = self.beliefs.read().unwrap().len();

        format!(
            r#"{{"entropy":{:.4},"rounds":{},"converged":{},"temperature":{:.4},"options":{},"threshold":{:.4}}}"#,
            entropy, rounds, converged, temp, options, self.entropy_threshold
        )
    }

    /// Reset consensus state for new decision
    #[wasm_bindgen]
    pub fn reset(&self) {
        *self.beliefs.write().unwrap() = FxHashMap::default();
        *self.negotiation_rounds.write().unwrap() = 0;
        *self.temperature.write().unwrap() = self.initial_temperature;
        self.entropy_history.write().unwrap().clear();
    }
}

impl Default for EntropyConsensus {
    fn default() -> Self {
        Self::new()
    }
}

impl EntropyConsensus {
    /// Create with full configuration
    pub fn with_config(config: EntropyConsensusConfig) -> Self {
        Self {
            beliefs: RwLock::new(FxHashMap::default()),
            entropy_threshold: config.entropy_threshold,
            negotiation_rounds: RwLock::new(0),
            max_rounds: config.max_negotiation_rounds,
            local_weight: config.local_weight,
            min_prob: config.min_probability,
            temperature: RwLock::new(config.initial_temperature),
            initial_temperature: config.initial_temperature,
            enable_annealing: config.enable_annealing,
            entropy_history: RwLock::new(Vec::with_capacity(config.max_negotiation_rounds)),
        }
    }

    /// Negotiate with peer beliefs to minimize entropy
    ///
    /// Updates local beliefs by averaging with peer beliefs:
    ///   p_new = local_weight * p_local + (1 - local_weight) * p_peer
    ///
    /// This implements a weighted averaging consensus protocol.
    pub fn negotiate(&self, peer_beliefs: &FxHashMap<u64, f32>) {
        let peer_weight = 1.0 - self.local_weight;

        // Apply temperature-scaled mixing if annealing is enabled
        let effective_peer_weight = if self.enable_annealing {
            let temp = *self.temperature.read().unwrap();
            peer_weight * temp
        } else {
            peer_weight
        };

        let effective_local_weight = 1.0 - effective_peer_weight;

        {
            let mut beliefs = self.beliefs.write().unwrap();

            // Update beliefs for all known decisions
            for (decision_id, peer_prob) in peer_beliefs {
                let my_prob = beliefs.get(decision_id).copied().unwrap_or(0.5);
                let new_prob = effective_local_weight * my_prob + effective_peer_weight * peer_prob;
                beliefs.insert(*decision_id, new_prob.max(self.min_prob));
            }

            // Also consider local-only beliefs (peer may not know about)
            let local_only: Vec<u64> = beliefs.keys()
                .filter(|k| !peer_beliefs.contains_key(*k))
                .copied()
                .collect();

            for decision_id in local_only {
                if let Some(prob) = beliefs.get_mut(&decision_id) {
                    // Decay beliefs not shared by peer
                    *prob = (*prob * effective_local_weight).max(self.min_prob);
                }
            }
        }

        self.normalize_beliefs();

        // Update negotiation round count
        {
            let mut rounds = self.negotiation_rounds.write().unwrap();
            *rounds += 1;
        }

        // Update temperature (simulated annealing)
        if self.enable_annealing {
            let mut temp = self.temperature.write().unwrap();
            *temp = (*temp * 0.95).max(0.01); // Exponential cooling
        }

        // Record entropy history
        {
            let entropy = self.entropy();
            let mut history = self.entropy_history.write().unwrap();
            history.push(entropy);
        }
    }

    /// Negotiate with peer beliefs (HashMap variant for convenience)
    pub fn negotiate_map(&self, peer_beliefs: &std::collections::HashMap<Decision, f32>) {
        let fx_map: FxHashMap<u64, f32> = peer_beliefs.iter()
            .map(|(d, p)| (d.id(), *p))
            .collect();
        self.negotiate(&fx_map);
    }

    /// Add a decision option with initial belief
    pub fn add_option(&self, decision: Decision, initial_belief: f32) {
        let prob = initial_belief.clamp(self.min_prob, 1.0);
        self.beliefs.write().unwrap().insert(decision.id(), prob);
        self.normalize_beliefs();
    }

    /// Get the best decision with its probability
    pub fn decision(&self) -> Option<(u64, f32)> {
        if !self.converged() {
            return None;
        }

        let beliefs = self.beliefs.read().unwrap();
        beliefs.iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(&id, &prob)| (id, prob))
    }

    /// Get all beliefs as a map
    pub fn get_all_beliefs(&self) -> FxHashMap<u64, f32> {
        self.beliefs.read().unwrap().clone()
    }

    /// Set multiple beliefs at once (normalized together)
    /// This avoids the issue where individual set_belief calls normalize prematurely
    pub fn set_beliefs(&self, new_beliefs: &[(u64, f32)]) {
        let mut beliefs = self.beliefs.write().unwrap();
        for (decision_id, probability) in new_beliefs {
            let prob = probability.clamp(self.min_prob, 1.0);
            beliefs.insert(*decision_id, prob);
        }
        drop(beliefs);
        self.normalize_beliefs();
    }

    /// Compute Shannon entropy of belief distribution
    fn compute_entropy(&self, beliefs: &FxHashMap<u64, f32>) -> f32 {
        if beliefs.is_empty() {
            return 0.0;
        }

        // H = -SUM(p_i * log2(p_i))
        -beliefs.values()
            .filter(|&&p| p > self.min_prob)
            .map(|&p| {
                let p_clamped = p.clamp(self.min_prob, 1.0);
                p_clamped * p_clamped.log2()
            })
            .sum::<f32>()
    }

    /// Normalize beliefs to sum to 1.0
    fn normalize_beliefs(&self) {
        let mut beliefs = self.beliefs.write().unwrap();
        let sum: f32 = beliefs.values().sum();

        if sum > 0.0 && sum != 1.0 {
            for prob in beliefs.values_mut() {
                *prob /= sum;
            }
        } else if sum == 0.0 && !beliefs.is_empty() {
            // Uniform distribution if all zeros
            let uniform = 1.0 / beliefs.len() as f32;
            for prob in beliefs.values_mut() {
                *prob = uniform;
            }
        }
    }
}

// ============================================================================
// Multi-Phase Consensus
// ============================================================================

/// Phase of consensus protocol
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ConsensusPhase {
    /// Proposing options
    Proposal,
    /// Negotiating beliefs
    Negotiation,
    /// Final voting
    Voting,
    /// Consensus reached
    Committed,
    /// Failed to reach consensus
    Aborted,
}

/// Multi-phase consensus coordinator
pub struct ConsensusCoordinator {
    /// Current phase
    phase: RwLock<ConsensusPhase>,
    /// Active consensus instances by topic
    instances: RwLock<FxHashMap<String, EntropyConsensus>>,
    /// Phase transition timestamps
    phase_times: RwLock<Vec<u64>>,
    /// Quorum requirement (fraction of nodes)
    quorum: f32,
}

impl ConsensusCoordinator {
    /// Create new coordinator with quorum requirement
    pub fn new(quorum: f32) -> Self {
        Self {
            phase: RwLock::new(ConsensusPhase::Proposal),
            instances: RwLock::new(FxHashMap::default()),
            phase_times: RwLock::new(Vec::new()),
            quorum: quorum.clamp(0.5, 1.0),
        }
    }

    /// Start consensus for a topic
    pub fn start_consensus(&self, topic: &str, config: EntropyConsensusConfig) {
        let mut instances = self.instances.write().unwrap();
        instances.insert(topic.to_string(), EntropyConsensus::with_config(config));
        *self.phase.write().unwrap() = ConsensusPhase::Proposal;
    }

    /// Get consensus instance for topic
    pub fn get_instance(&self, topic: &str) -> Option<EntropyConsensus> {
        self.instances.read().unwrap().get(topic).map(|c| {
            // Return a new instance with same state
            let config = EntropyConsensusConfig {
                entropy_threshold: c.entropy_threshold,
                max_negotiation_rounds: c.max_rounds,
                local_weight: c.local_weight,
                min_probability: c.min_prob,
                enable_annealing: c.enable_annealing,
                initial_temperature: c.initial_temperature,
            };
            EntropyConsensus::with_config(config)
        })
    }

    /// Advance phase based on state
    pub fn advance_phase(&self, topic: &str) -> ConsensusPhase {
        let instances = self.instances.read().unwrap();

        if let Some(consensus) = instances.get(topic) {
            let mut phase = self.phase.write().unwrap();

            match *phase {
                ConsensusPhase::Proposal => {
                    if consensus.option_count() > 0 {
                        *phase = ConsensusPhase::Negotiation;
                    }
                }
                ConsensusPhase::Negotiation => {
                    if consensus.converged() {
                        *phase = ConsensusPhase::Voting;
                    } else if consensus.has_timed_out() {
                        *phase = ConsensusPhase::Aborted;
                    }
                }
                ConsensusPhase::Voting => {
                    // Check if quorum reached
                    if consensus.converged() {
                        *phase = ConsensusPhase::Committed;
                    }
                }
                ConsensusPhase::Committed | ConsensusPhase::Aborted => {
                    // Terminal states
                }
            }

            *phase
        } else {
            ConsensusPhase::Aborted
        }
    }

    /// Get current phase
    pub fn current_phase(&self) -> ConsensusPhase {
        *self.phase.read().unwrap()
    }
}

impl Default for ConsensusCoordinator {
    fn default() -> Self {
        Self::new(0.67)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entropy_calculation() {
        let consensus = EntropyConsensus::new();

        // Use set_beliefs to set multiple beliefs at once (avoids intermediate normalization)
        consensus.set_beliefs(&[(1, 0.5), (2, 0.5)]);
        let uniform_entropy = consensus.entropy();
        assert!((uniform_entropy - 1.0).abs() < 0.01, "Uniform entropy should be 1.0, got {}", uniform_entropy); // log2(2) = 1

        // Reset and test concentrated distribution
        consensus.reset();
        consensus.set_beliefs(&[(1, 0.99), (2, 0.01)]);
        let concentrated_entropy = consensus.entropy();
        assert!(concentrated_entropy < 0.1, "Concentrated entropy should be < 0.1, got {}", concentrated_entropy); // Very low entropy
    }

    #[test]
    fn test_convergence() {
        let config = EntropyConsensusConfig {
            entropy_threshold: 0.35, // Entropy of 0.95:0.05 is ~0.286, so use threshold > 0.286
            ..Default::default()
        };
        let consensus = EntropyConsensus::with_config(config);

        // Start with concentrated belief using set_beliefs to avoid intermediate normalization
        // H(-0.95*log2(0.95) - 0.05*log2(0.05)) ~= 0.286
        consensus.set_beliefs(&[(1, 0.95), (2, 0.05)]);

        assert!(consensus.converged(), "Should be converged with entropy {}", consensus.entropy());
        assert!(consensus.get_decision().is_some());
        assert_eq!(consensus.get_decision().unwrap(), 1);
    }

    #[test]
    fn test_negotiation() {
        let consensus = EntropyConsensus::new();

        // Local: prefer option 1
        consensus.set_belief(1, 0.8);
        consensus.set_belief(2, 0.2);

        // Peer: prefers option 2
        let mut peer_beliefs = FxHashMap::default();
        peer_beliefs.insert(1, 0.2);
        peer_beliefs.insert(2, 0.8);

        // Negotiate - should move toward middle
        consensus.negotiate(&peer_beliefs);

        let belief_1 = consensus.get_belief(1);
        let belief_2 = consensus.get_belief(2);

        // After negotiation, beliefs should be closer to 0.5
        assert!(belief_1 < 0.8 && belief_1 > 0.2);
        assert!(belief_2 < 0.8 && belief_2 > 0.2);
    }

    #[test]
    fn test_repeated_negotiation_converges() {
        let config = EntropyConsensusConfig {
            entropy_threshold: 0.3, // Threshold for convergence
            local_weight: 0.5,
            enable_annealing: false, // Disable annealing for predictable convergence
            ..Default::default()
        };
        let consensus = EntropyConsensus::with_config(config);

        // Start uniform using set_beliefs
        consensus.set_beliefs(&[(1, 0.5), (2, 0.5)]);

        // Peer strongly prefers option 1
        let mut peer_beliefs = FxHashMap::default();
        peer_beliefs.insert(1, 0.95);
        peer_beliefs.insert(2, 0.05);

        // Negotiate multiple times
        for _ in 0..50 {
            consensus.negotiate(&peer_beliefs);
        }

        // Should have converged toward peer's preference
        let belief1 = consensus.get_belief(1);
        assert!(belief1 > 0.7, "Belief 1 should be > 0.7, got {}", belief1);
        assert!(consensus.converged(), "Should be converged with entropy {}", consensus.entropy());
    }

    #[test]
    fn test_timeout() {
        let config = EntropyConsensusConfig {
            max_negotiation_rounds: 5,
            ..Default::default()
        };
        let consensus = EntropyConsensus::with_config(config);

        consensus.set_belief(1, 0.5);
        consensus.set_belief(2, 0.5);

        // Both parties have same beliefs - no convergence
        let peer_beliefs = consensus.get_all_beliefs();

        for _ in 0..6 {
            consensus.negotiate(&peer_beliefs);
        }

        assert!(consensus.has_timed_out());
    }

    #[test]
    fn test_decision_types() {
        let d1 = Decision::Accept(42);
        let d2 = Decision::Reject(42);
        let d3 = Decision::RouteToNode(5);

        assert_ne!(d1.id(), d2.id());
        assert_ne!(d1.id(), d3.id());

        let consensus = EntropyConsensus::new();
        consensus.add_option(d1, 0.7);
        consensus.add_option(d2, 0.3);

        assert_eq!(consensus.option_count(), 2);
    }

    #[test]
    fn test_temperature_annealing() {
        let config = EntropyConsensusConfig {
            enable_annealing: true,
            initial_temperature: 1.0,
            ..Default::default()
        };
        let consensus = EntropyConsensus::with_config(config);

        consensus.set_belief(1, 0.6);
        consensus.set_belief(2, 0.4);

        let initial_temp = consensus.get_temperature();
        assert!((initial_temp - 1.0).abs() < 0.01);

        let peer_beliefs = consensus.get_all_beliefs();
        for _ in 0..10 {
            consensus.negotiate(&peer_beliefs);
        }

        let final_temp = consensus.get_temperature();
        assert!(final_temp < initial_temp); // Temperature should decrease
    }

    #[test]
    fn test_consensus_coordinator() {
        let coordinator = ConsensusCoordinator::new(0.67);

        let config = EntropyConsensusConfig::default();
        coordinator.start_consensus("task-routing", config);

        assert_eq!(coordinator.current_phase(), ConsensusPhase::Proposal);
    }
}
