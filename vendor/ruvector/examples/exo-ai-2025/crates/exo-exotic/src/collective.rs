//! # Collective Consciousness (Hive Mind)
//!
//! Implementation of distributed consciousness across multiple cognitive
//! substrates, creating emergent group awareness and collective intelligence.
//!
//! ## Key Concepts
//!
//! - **Distributed Φ**: Integrated information across multiple substrates
//! - **Swarm Intelligence**: Emergent behavior from simple rules
//! - **Collective Memory**: Shared memory pool across substrates
//! - **Consensus Mechanisms**: Agreement protocols for collective decisions
//!
//! ## Theoretical Basis
//!
//! Inspired by:
//! - IIT extended to multi-agent systems
//! - Swarm intelligence (ant colonies, bee hives)
//! - Global Workspace Theory (Baars)

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Collective consciousness spanning multiple substrates
#[derive(Debug)]
pub struct CollectiveConsciousness {
    /// Individual substrates in the collective
    substrates: Vec<Substrate>,
    /// Inter-substrate connections
    connections: Vec<Connection>,
    /// Shared memory pool
    shared_memory: Arc<DashMap<String, SharedMemoryItem>>,
    /// Global workspace for broadcast
    global_workspace: GlobalWorkspace,
    /// Collective phi (Φ) computation
    collective_phi: f64,
}

/// A single cognitive substrate in the collective
#[derive(Debug, Clone)]
pub struct Substrate {
    pub id: Uuid,
    /// Local Φ value
    pub local_phi: f64,
    /// Current state vector
    pub state: Vec<f64>,
    /// Processing capacity
    pub capacity: f64,
    /// Specialization type
    pub specialization: SubstrateSpecialization,
    /// Activity level (0-1)
    pub activity: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SubstrateSpecialization {
    Perception,
    Processing,
    Memory,
    Integration,
    Output,
    General,
}

/// Connection between substrates
#[derive(Debug, Clone)]
pub struct Connection {
    pub from: Uuid,
    pub to: Uuid,
    pub strength: f64,
    pub delay: u32,
    pub bidirectional: bool,
}

/// Hive mind coordinating the collective
#[derive(Debug)]
pub struct HiveMind {
    /// Central coordination state
    coordination_state: CoordinationState,
    /// Decision history
    decisions: Vec<CollectiveDecision>,
    /// Consensus threshold
    consensus_threshold: f64,
}

#[derive(Debug, Clone)]
pub enum CoordinationState {
    Distributed,
    Coordinated,
    Emergency,
    Dormant,
}

#[derive(Debug, Clone)]
pub struct CollectiveDecision {
    pub id: Uuid,
    pub proposal: String,
    pub votes: HashMap<Uuid, f64>,
    pub result: Option<bool>,
    pub consensus_level: f64,
}

/// Distributed Φ computation
#[derive(Debug)]
pub struct DistributedPhi {
    /// Per-substrate Φ values
    local_phis: HashMap<Uuid, f64>,
    /// Inter-substrate integration
    integration_matrix: Vec<Vec<f64>>,
    /// Global Φ estimate
    global_phi: f64,
}

/// Global workspace for information broadcast
#[derive(Debug)]
pub struct GlobalWorkspace {
    /// Current broadcast content
    broadcast: Option<BroadcastContent>,
    /// Workspace capacity
    capacity: usize,
    /// Competition threshold
    threshold: f64,
    /// Broadcast history
    history: Vec<BroadcastContent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BroadcastContent {
    pub source: Uuid,
    pub content: Vec<f64>,
    pub salience: f64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedMemoryItem {
    pub content: Vec<f64>,
    pub owner: Uuid,
    pub access_count: usize,
    pub importance: f64,
}

impl CollectiveConsciousness {
    /// Create a new collective consciousness
    pub fn new() -> Self {
        Self {
            substrates: Vec::new(),
            connections: Vec::new(),
            shared_memory: Arc::new(DashMap::new()),
            global_workspace: GlobalWorkspace::new(10),
            collective_phi: 0.0,
        }
    }

    /// Add a substrate to the collective
    pub fn add_substrate(&mut self, specialization: SubstrateSpecialization) -> Uuid {
        let id = Uuid::new_v4();
        let substrate = Substrate {
            id,
            local_phi: 0.0,
            state: vec![0.0; 8],
            capacity: 1.0,
            specialization,
            activity: 0.5,
        };
        self.substrates.push(substrate);
        id
    }

    /// Connect two substrates
    pub fn connect(&mut self, from: Uuid, to: Uuid, strength: f64, bidirectional: bool) {
        self.connections.push(Connection {
            from,
            to,
            strength,
            delay: 1,
            bidirectional,
        });

        if bidirectional {
            self.connections.push(Connection {
                from: to,
                to: from,
                strength,
                delay: 1,
                bidirectional: false,
            });
        }
    }

    /// Compute global Φ across all substrates
    pub fn compute_global_phi(&mut self) -> f64 {
        if self.substrates.is_empty() {
            return 0.0;
        }

        // Compute local Φ for each substrate (collect state first to avoid borrow issues)
        let local_phis: Vec<f64> = self
            .substrates
            .iter()
            .map(|s| {
                let entropy = self.compute_entropy(&s.state);
                let integration = s.activity * s.capacity;
                entropy * integration
            })
            .collect();

        // Update local phi values
        for (substrate, phi) in self.substrates.iter_mut().zip(local_phis.iter()) {
            substrate.local_phi = *phi;
        }

        // Compute integration across substrates
        let integration = self.compute_integration();

        // Global Φ = sum of local Φ weighted by integration
        let local_sum: f64 = self
            .substrates
            .iter()
            .map(|s| s.local_phi * s.activity)
            .sum();

        self.collective_phi = local_sum * integration;
        self.collective_phi
    }

    #[allow(dead_code)]
    fn compute_local_phi(&self, substrate: &Substrate) -> f64 {
        // Simplified IIT Φ computation
        let entropy = self.compute_entropy(&substrate.state);
        let integration = substrate.activity * substrate.capacity;

        entropy * integration
    }

    fn compute_entropy(&self, state: &[f64]) -> f64 {
        let sum: f64 = state.iter().map(|x| x.abs()).sum();
        if sum == 0.0 {
            return 0.0;
        }

        let normalized: Vec<f64> = state.iter().map(|x| x.abs() / sum).collect();
        -normalized
            .iter()
            .filter(|&&p| p > 1e-10)
            .map(|&p| p * p.ln())
            .sum::<f64>()
    }

    fn compute_integration(&self) -> f64 {
        if self.connections.is_empty() || self.substrates.len() < 2 {
            return 0.0;
        }

        // Integration based on connection density and strength
        let max_connections = self.substrates.len() * (self.substrates.len() - 1);
        let connection_density = self.connections.len() as f64 / max_connections as f64;

        let avg_strength: f64 = self.connections.iter().map(|c| c.strength).sum::<f64>()
            / self.connections.len() as f64;

        (connection_density * avg_strength).min(1.0)
    }

    /// Share memory item across collective
    pub fn share_memory(&self, key: &str, content: Vec<f64>, owner: Uuid) {
        self.shared_memory.insert(
            key.to_string(),
            SharedMemoryItem {
                content,
                owner,
                access_count: 0,
                importance: 0.5,
            },
        );
    }

    /// Access shared memory
    pub fn access_memory(&self, key: &str) -> Option<Vec<f64>> {
        self.shared_memory.get_mut(key).map(|mut item| {
            item.access_count += 1;
            item.content.clone()
        })
    }

    /// Broadcast to global workspace
    pub fn broadcast(&mut self, source: Uuid, content: Vec<f64>, salience: f64) -> bool {
        self.global_workspace.try_broadcast(BroadcastContent {
            source,
            content,
            salience,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        })
    }

    /// Get current broadcast
    pub fn current_broadcast(&self) -> Option<&BroadcastContent> {
        self.global_workspace.current()
    }

    /// Propagate state through network
    pub fn propagate(&mut self) {
        let substrate_map: HashMap<Uuid, usize> = self
            .substrates
            .iter()
            .enumerate()
            .map(|(i, s)| (s.id, i))
            .collect();

        let mut updates: Vec<(usize, Vec<f64>)> = Vec::new();

        for conn in &self.connections {
            if let (Some(&from_idx), Some(&to_idx)) =
                (substrate_map.get(&conn.from), substrate_map.get(&conn.to))
            {
                let from_state = &self.substrates[from_idx].state;
                let influence: Vec<f64> = from_state.iter().map(|&v| v * conn.strength).collect();
                updates.push((to_idx, influence));
            }
        }

        for (idx, influence) in updates {
            for (i, inf) in influence.iter().enumerate() {
                if i < self.substrates[idx].state.len() {
                    self.substrates[idx].state[i] += inf * 0.1;
                    self.substrates[idx].state[i] = self.substrates[idx].state[i].clamp(-1.0, 1.0);
                }
            }
        }
    }

    /// Get substrate count
    pub fn substrate_count(&self) -> usize {
        self.substrates.len()
    }

    /// Get connection count
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    /// Get collective health metrics
    pub fn health_metrics(&self) -> CollectiveHealth {
        let avg_activity = if self.substrates.is_empty() {
            0.0
        } else {
            self.substrates.iter().map(|s| s.activity).sum::<f64>() / self.substrates.len() as f64
        };

        CollectiveHealth {
            substrate_count: self.substrates.len(),
            connection_density: if self.substrates.len() > 1 {
                self.connections.len() as f64
                    / (self.substrates.len() * (self.substrates.len() - 1)) as f64
            } else {
                0.0
            },
            average_activity: avg_activity,
            collective_phi: self.collective_phi,
            shared_memory_size: self.shared_memory.len(),
        }
    }
}

impl Default for CollectiveConsciousness {
    fn default() -> Self {
        Self::new()
    }
}

impl HiveMind {
    /// Create a new hive mind coordinator
    pub fn new(consensus_threshold: f64) -> Self {
        Self {
            coordination_state: CoordinationState::Distributed,
            decisions: Vec::new(),
            consensus_threshold,
        }
    }

    /// Propose a collective decision
    pub fn propose(&mut self, proposal: &str) -> Uuid {
        let id = Uuid::new_v4();
        self.decisions.push(CollectiveDecision {
            id,
            proposal: proposal.to_string(),
            votes: HashMap::new(),
            result: None,
            consensus_level: 0.0,
        });
        id
    }

    /// Vote on a proposal
    pub fn vote(&mut self, decision_id: Uuid, voter: Uuid, confidence: f64) -> bool {
        if let Some(decision) = self.decisions.iter_mut().find(|d| d.id == decision_id) {
            decision.votes.insert(voter, confidence.clamp(-1.0, 1.0));
            true
        } else {
            false
        }
    }

    /// Resolve a decision
    pub fn resolve(&mut self, decision_id: Uuid) -> Option<bool> {
        if let Some(decision) = self.decisions.iter_mut().find(|d| d.id == decision_id) {
            if decision.votes.is_empty() {
                return None;
            }

            let avg_vote: f64 = decision.votes.values().sum::<f64>() / decision.votes.len() as f64;

            decision.consensus_level = decision
                .votes
                .values()
                .map(|&v| 1.0 - (v - avg_vote).abs())
                .sum::<f64>()
                / decision.votes.len() as f64;

            let result = avg_vote > 0.0 && decision.consensus_level >= self.consensus_threshold;
            decision.result = Some(result);
            Some(result)
        } else {
            None
        }
    }

    /// Get coordination state
    pub fn state(&self) -> &CoordinationState {
        &self.coordination_state
    }

    /// Set coordination state
    pub fn set_state(&mut self, state: CoordinationState) {
        self.coordination_state = state;
    }
}

impl DistributedPhi {
    /// Create a new distributed Φ calculator
    pub fn new(num_substrates: usize) -> Self {
        Self {
            local_phis: HashMap::new(),
            integration_matrix: vec![vec![0.0; num_substrates]; num_substrates],
            global_phi: 0.0,
        }
    }

    /// Update local Φ for a substrate
    pub fn update_local(&mut self, substrate_id: Uuid, phi: f64) {
        self.local_phis.insert(substrate_id, phi);
    }

    /// Set integration strength between substrates
    pub fn set_integration(&mut self, i: usize, j: usize, strength: f64) {
        if i < self.integration_matrix.len() && j < self.integration_matrix[i].len() {
            self.integration_matrix[i][j] = strength;
        }
    }

    /// Compute global Φ
    pub fn compute(&mut self) -> f64 {
        let local_sum: f64 = self.local_phis.values().sum();

        let mut integration_sum = 0.0;
        for row in &self.integration_matrix {
            integration_sum += row.iter().sum::<f64>();
        }

        let n = self.integration_matrix.len() as f64;
        let avg_integration = if n > 1.0 {
            integration_sum / (n * (n - 1.0))
        } else {
            0.0
        };

        self.global_phi = local_sum * (1.0 + avg_integration);
        self.global_phi
    }

    /// Get global Φ
    pub fn global_phi(&self) -> f64 {
        self.global_phi
    }
}

impl GlobalWorkspace {
    /// Create a new global workspace
    pub fn new(capacity: usize) -> Self {
        Self {
            broadcast: None,
            capacity,
            threshold: 0.5,
            history: Vec::new(),
        }
    }

    /// Try to broadcast content (competes with current broadcast)
    pub fn try_broadcast(&mut self, content: BroadcastContent) -> bool {
        match &self.broadcast {
            None => {
                self.broadcast = Some(content);
                true
            }
            Some(current) if content.salience > current.salience + self.threshold => {
                // Save current to history
                if self.history.len() < self.capacity {
                    self.history.push(current.clone());
                }
                self.broadcast = Some(content);
                true
            }
            _ => false,
        }
    }

    /// Get current broadcast
    pub fn current(&self) -> Option<&BroadcastContent> {
        self.broadcast.as_ref()
    }

    /// Clear the workspace
    pub fn clear(&mut self) {
        if let Some(broadcast) = self.broadcast.take() {
            if self.history.len() < self.capacity {
                self.history.push(broadcast);
            }
        }
    }
}

/// Health metrics for the collective
#[derive(Debug, Clone)]
pub struct CollectiveHealth {
    pub substrate_count: usize,
    pub connection_density: f64,
    pub average_activity: f64,
    pub collective_phi: f64,
    pub shared_memory_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collective_creation() {
        let collective = CollectiveConsciousness::new();
        assert_eq!(collective.substrate_count(), 0);
    }

    #[test]
    fn test_add_substrates() {
        let mut collective = CollectiveConsciousness::new();
        let id1 = collective.add_substrate(SubstrateSpecialization::Processing);
        let id2 = collective.add_substrate(SubstrateSpecialization::Memory);

        assert_eq!(collective.substrate_count(), 2);
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_connect_substrates() {
        let mut collective = CollectiveConsciousness::new();
        let id1 = collective.add_substrate(SubstrateSpecialization::Processing);
        let id2 = collective.add_substrate(SubstrateSpecialization::Memory);

        collective.connect(id1, id2, 0.8, true);
        assert_eq!(collective.connection_count(), 2); // Bidirectional = 2 connections
    }

    #[test]
    fn test_compute_global_phi() {
        let mut collective = CollectiveConsciousness::new();

        for _ in 0..4 {
            collective.add_substrate(SubstrateSpecialization::Processing);
        }

        // Connect all pairs
        let ids: Vec<Uuid> = collective.substrates.iter().map(|s| s.id).collect();
        for i in 0..ids.len() {
            for j in i + 1..ids.len() {
                collective.connect(ids[i], ids[j], 0.5, true);
            }
        }

        let phi = collective.compute_global_phi();
        assert!(phi >= 0.0);
    }

    #[test]
    fn test_shared_memory() {
        let collective = CollectiveConsciousness::new();
        let owner = Uuid::new_v4();

        collective.share_memory("test_key", vec![1.0, 2.0, 3.0], owner);
        let retrieved = collective.access_memory("test_key");

        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_hive_mind_voting() {
        let mut hive = HiveMind::new(0.6);

        let decision_id = hive.propose("Should we expand?");

        let voter1 = Uuid::new_v4();
        let voter2 = Uuid::new_v4();
        let voter3 = Uuid::new_v4();

        hive.vote(decision_id, voter1, 0.9);
        hive.vote(decision_id, voter2, 0.8);
        hive.vote(decision_id, voter3, 0.7);

        let result = hive.resolve(decision_id);
        assert!(result.is_some());
    }

    #[test]
    fn test_global_workspace() {
        let mut workspace = GlobalWorkspace::new(5);

        let content1 = BroadcastContent {
            source: Uuid::new_v4(),
            content: vec![1.0],
            salience: 0.5,
            timestamp: 0,
        };

        assert!(workspace.try_broadcast(content1));
        assert!(workspace.current().is_some());

        // Lower salience should fail
        let content2 = BroadcastContent {
            source: Uuid::new_v4(),
            content: vec![2.0],
            salience: 0.3,
            timestamp: 1,
        };

        assert!(!workspace.try_broadcast(content2));
    }

    #[test]
    fn test_distributed_phi() {
        let mut dphi = DistributedPhi::new(3);

        dphi.update_local(Uuid::new_v4(), 0.5);
        dphi.update_local(Uuid::new_v4(), 0.6);
        dphi.update_local(Uuid::new_v4(), 0.4);

        dphi.set_integration(0, 1, 0.8);
        dphi.set_integration(1, 2, 0.7);

        let phi = dphi.compute();
        assert!(phi > 0.0);
    }
}
