//! Distributed Coherence State Machine
//!
//! State machine for replicated coherence state across the cluster.

use super::adapter::CoherenceCommand;
use std::collections::{HashMap, HashSet};

/// Node state in the distributed system
#[derive(Debug, Clone)]
pub struct NodeState {
    /// Node identifier
    pub node_id: u64,
    /// State vector
    pub state: Vec<f32>,
    /// Last update timestamp
    pub last_update: u64,
}

/// Edge energy state
#[derive(Debug, Clone)]
pub struct EdgeEnergy {
    /// Source node
    pub source: u64,
    /// Target node
    pub target: u64,
    /// Current energy value
    pub energy: f32,
    /// History of recent energies (for trend analysis)
    pub history: Vec<f32>,
}

impl EdgeEnergy {
    /// Create new edge energy
    pub fn new(source: u64, target: u64, energy: f32) -> Self {
        Self {
            source,
            target,
            energy,
            history: vec![energy],
        }
    }

    /// Update energy value
    pub fn update(&mut self, energy: f32) {
        self.energy = energy;
        self.history.push(energy);
        // Keep only last 10 values
        if self.history.len() > 10 {
            self.history.remove(0);
        }
    }

    /// Get energy trend (positive = increasing, negative = decreasing)
    pub fn trend(&self) -> f32 {
        if self.history.len() < 2 {
            return 0.0;
        }
        let n = self.history.len();
        let first_half: f32 = self.history[..n / 2].iter().sum::<f32>() / (n / 2) as f32;
        let second_half: f32 = self.history[n / 2..].iter().sum::<f32>() / (n - n / 2) as f32;
        second_half - first_half
    }

    /// Check if energy is stable
    pub fn is_stable(&self, threshold: f32) -> bool {
        if self.history.len() < 2 {
            return true;
        }
        let mean: f32 = self.history.iter().sum::<f32>() / self.history.len() as f32;
        let variance: f32 = self.history.iter().map(|e| (e - mean).powi(2)).sum::<f32>()
            / self.history.len() as f32;
        variance.sqrt() < threshold
    }
}

/// Incoherent region tracking
#[derive(Debug, Clone)]
pub struct IncoherentRegion {
    /// Region identifier
    pub region_id: u64,
    /// Nodes in this region
    pub nodes: HashSet<u64>,
    /// When the region was marked incoherent
    pub marked_at: u64,
    /// Whether region is currently flagged
    pub active: bool,
}

/// Checkpoint of coherence state
#[derive(Debug, Clone)]
pub struct Checkpoint {
    /// Checkpoint index
    pub index: u64,
    /// Total energy at checkpoint
    pub total_energy: f32,
    /// Timestamp
    pub timestamp: u64,
    /// Number of edges
    pub num_edges: usize,
    /// Number of incoherent regions
    pub num_incoherent: usize,
}

/// Replicated coherence state machine
#[derive(Debug)]
pub struct CoherenceStateMachine {
    /// Node states (node_id -> state)
    node_states: HashMap<u64, NodeState>,
    /// Edge energies ((src, dst) -> energy)
    edge_energies: HashMap<(u64, u64), EdgeEnergy>,
    /// Incoherent regions
    incoherent_regions: HashMap<u64, IncoherentRegion>,
    /// Checkpoints
    checkpoints: Vec<Checkpoint>,
    /// Current applied index
    applied_index: u64,
    /// Configuration dimension
    dimension: usize,
}

impl CoherenceStateMachine {
    /// Create a new state machine
    pub fn new(dimension: usize) -> Self {
        Self {
            node_states: HashMap::new(),
            edge_energies: HashMap::new(),
            incoherent_regions: HashMap::new(),
            checkpoints: Vec::new(),
            applied_index: 0,
            dimension,
        }
    }

    /// Apply a command to the state machine
    pub fn apply(&mut self, command: &CoherenceCommand, index: u64) -> ApplyResult {
        self.applied_index = index;

        match command {
            CoherenceCommand::UpdateEnergy { edge_id, energy } => {
                self.apply_update_energy(*edge_id, *energy)
            }
            CoherenceCommand::SetNodeState { node_id, state } => {
                self.apply_set_node_state(*node_id, state.clone())
            }
            CoherenceCommand::Checkpoint {
                total_energy,
                timestamp,
            } => self.apply_checkpoint(*total_energy, *timestamp),
            CoherenceCommand::MarkIncoherent { region_id, nodes } => {
                self.apply_mark_incoherent(*region_id, nodes.clone())
            }
            CoherenceCommand::ClearIncoherent { region_id } => {
                self.apply_clear_incoherent(*region_id)
            }
        }
    }

    fn apply_update_energy(&mut self, edge_id: (u64, u64), energy: f32) -> ApplyResult {
        let edge = self
            .edge_energies
            .entry(edge_id)
            .or_insert_with(|| EdgeEnergy::new(edge_id.0, edge_id.1, 0.0));

        let old_energy = edge.energy;
        edge.update(energy);

        ApplyResult::EnergyUpdated {
            edge_id,
            old_energy,
            new_energy: energy,
        }
    }

    fn apply_set_node_state(&mut self, node_id: u64, state: Vec<f32>) -> ApplyResult {
        let truncated_state: Vec<f32> = state.into_iter().take(self.dimension).collect();

        let node = self
            .node_states
            .entry(node_id)
            .or_insert_with(|| NodeState {
                node_id,
                state: vec![0.0; self.dimension],
                last_update: 0,
            });

        node.state = truncated_state;
        node.last_update = self.applied_index;

        ApplyResult::NodeStateSet { node_id }
    }

    fn apply_checkpoint(&mut self, total_energy: f32, timestamp: u64) -> ApplyResult {
        let checkpoint = Checkpoint {
            index: self.applied_index,
            total_energy,
            timestamp,
            num_edges: self.edge_energies.len(),
            num_incoherent: self
                .incoherent_regions
                .values()
                .filter(|r| r.active)
                .count(),
        };

        self.checkpoints.push(checkpoint.clone());

        // Keep only last 100 checkpoints
        if self.checkpoints.len() > 100 {
            self.checkpoints.remove(0);
        }

        ApplyResult::CheckpointCreated { checkpoint }
    }

    fn apply_mark_incoherent(&mut self, region_id: u64, nodes: Vec<u64>) -> ApplyResult {
        let region = self
            .incoherent_regions
            .entry(region_id)
            .or_insert_with(|| IncoherentRegion {
                region_id,
                nodes: HashSet::new(),
                marked_at: self.applied_index,
                active: false,
            });

        region.nodes = nodes.into_iter().collect();
        region.marked_at = self.applied_index;
        region.active = true;

        ApplyResult::RegionMarkedIncoherent {
            region_id,
            node_count: region.nodes.len(),
        }
    }

    fn apply_clear_incoherent(&mut self, region_id: u64) -> ApplyResult {
        if let Some(region) = self.incoherent_regions.get_mut(&region_id) {
            region.active = false;
            ApplyResult::RegionCleared { region_id }
        } else {
            ApplyResult::RegionNotFound { region_id }
        }
    }

    /// Get node state
    pub fn get_node_state(&self, node_id: u64) -> Option<&NodeState> {
        self.node_states.get(&node_id)
    }

    /// Get edge energy
    pub fn get_edge_energy(&self, edge_id: (u64, u64)) -> Option<f32> {
        self.edge_energies.get(&edge_id).map(|e| e.energy)
    }

    /// Get total energy
    pub fn total_energy(&self) -> f32 {
        self.edge_energies.values().map(|e| e.energy).sum()
    }

    /// Get number of incoherent regions
    pub fn num_incoherent_regions(&self) -> usize {
        self.incoherent_regions
            .values()
            .filter(|r| r.active)
            .count()
    }

    /// Get all incoherent node IDs
    pub fn incoherent_nodes(&self) -> HashSet<u64> {
        self.incoherent_regions
            .values()
            .filter(|r| r.active)
            .flat_map(|r| r.nodes.iter().copied())
            .collect()
    }

    /// Check if a node is in an incoherent region
    pub fn is_node_incoherent(&self, node_id: u64) -> bool {
        self.incoherent_regions
            .values()
            .any(|r| r.active && r.nodes.contains(&node_id))
    }

    /// Get latest checkpoint
    pub fn latest_checkpoint(&self) -> Option<&Checkpoint> {
        self.checkpoints.last()
    }

    /// Get state summary
    pub fn summary(&self) -> StateSummary {
        StateSummary {
            applied_index: self.applied_index,
            num_nodes: self.node_states.len(),
            num_edges: self.edge_energies.len(),
            total_energy: self.total_energy(),
            num_incoherent_regions: self.num_incoherent_regions(),
            num_checkpoints: self.checkpoints.len(),
        }
    }

    /// Create snapshot data
    pub fn snapshot(&self) -> StateSnapshot {
        StateSnapshot {
            applied_index: self.applied_index,
            node_states: self.node_states.clone(),
            edge_energies: self.edge_energies.clone(),
            incoherent_regions: self.incoherent_regions.clone(),
        }
    }

    /// Restore from snapshot
    pub fn restore(&mut self, snapshot: StateSnapshot) {
        self.applied_index = snapshot.applied_index;
        self.node_states = snapshot.node_states;
        self.edge_energies = snapshot.edge_energies;
        self.incoherent_regions = snapshot.incoherent_regions;
    }
}

/// Result of applying a command
#[derive(Debug, Clone)]
pub enum ApplyResult {
    /// Energy was updated
    EnergyUpdated {
        edge_id: (u64, u64),
        old_energy: f32,
        new_energy: f32,
    },
    /// Node state was set
    NodeStateSet { node_id: u64 },
    /// Checkpoint was created
    CheckpointCreated { checkpoint: Checkpoint },
    /// Region was marked incoherent
    RegionMarkedIncoherent { region_id: u64, node_count: usize },
    /// Region was cleared
    RegionCleared { region_id: u64 },
    /// Region was not found
    RegionNotFound { region_id: u64 },
}

/// Summary of state machine state
#[derive(Debug, Clone)]
pub struct StateSummary {
    /// Last applied log index
    pub applied_index: u64,
    /// Number of nodes
    pub num_nodes: usize,
    /// Number of edges
    pub num_edges: usize,
    /// Total energy
    pub total_energy: f32,
    /// Number of active incoherent regions
    pub num_incoherent_regions: usize,
    /// Number of checkpoints
    pub num_checkpoints: usize,
}

/// Snapshot of state machine
#[derive(Debug, Clone)]
pub struct StateSnapshot {
    /// Applied index at snapshot time
    pub applied_index: u64,
    /// Node states
    pub node_states: HashMap<u64, NodeState>,
    /// Edge energies
    pub edge_energies: HashMap<(u64, u64), EdgeEnergy>,
    /// Incoherent regions
    pub incoherent_regions: HashMap<u64, IncoherentRegion>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_machine_creation() {
        let sm = CoherenceStateMachine::new(64);
        assert_eq!(sm.total_energy(), 0.0);
        assert_eq!(sm.num_incoherent_regions(), 0);
    }

    #[test]
    fn test_update_energy() {
        let mut sm = CoherenceStateMachine::new(64);

        let cmd = CoherenceCommand::UpdateEnergy {
            edge_id: (1, 2),
            energy: 0.5,
        };
        sm.apply(&cmd, 1);

        assert!((sm.get_edge_energy((1, 2)).unwrap() - 0.5).abs() < 1e-6);
        assert!((sm.total_energy() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_set_node_state() {
        let mut sm = CoherenceStateMachine::new(4);

        let cmd = CoherenceCommand::SetNodeState {
            node_id: 1,
            state: vec![1.0, 2.0, 3.0, 4.0],
        };
        sm.apply(&cmd, 1);

        let state = sm.get_node_state(1).unwrap();
        assert_eq!(state.state, vec![1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_mark_incoherent() {
        let mut sm = CoherenceStateMachine::new(64);

        let cmd = CoherenceCommand::MarkIncoherent {
            region_id: 1,
            nodes: vec![10, 20, 30],
        };
        sm.apply(&cmd, 1);

        assert_eq!(sm.num_incoherent_regions(), 1);
        assert!(sm.is_node_incoherent(10));
        assert!(sm.is_node_incoherent(20));
        assert!(!sm.is_node_incoherent(40));
    }

    #[test]
    fn test_clear_incoherent() {
        let mut sm = CoherenceStateMachine::new(64);

        sm.apply(
            &CoherenceCommand::MarkIncoherent {
                region_id: 1,
                nodes: vec![10],
            },
            1,
        );
        assert_eq!(sm.num_incoherent_regions(), 1);

        sm.apply(&CoherenceCommand::ClearIncoherent { region_id: 1 }, 2);
        assert_eq!(sm.num_incoherent_regions(), 0);
    }

    #[test]
    fn test_checkpoint() {
        let mut sm = CoherenceStateMachine::new(64);

        sm.apply(
            &CoherenceCommand::Checkpoint {
                total_energy: 1.5,
                timestamp: 1000,
            },
            1,
        );

        let cp = sm.latest_checkpoint().unwrap();
        assert!((cp.total_energy - 1.5).abs() < 1e-6);
        assert_eq!(cp.timestamp, 1000);
    }

    #[test]
    fn test_edge_energy_trend() {
        let mut edge = EdgeEnergy::new(1, 2, 1.0);
        edge.update(1.1);
        edge.update(1.2);
        edge.update(1.3);
        edge.update(1.4);

        let trend = edge.trend();
        assert!(
            trend > 0.0,
            "Trend should be positive for increasing energy"
        );
    }

    #[test]
    fn test_snapshot_restore() {
        let mut sm = CoherenceStateMachine::new(64);

        sm.apply(
            &CoherenceCommand::UpdateEnergy {
                edge_id: (1, 2),
                energy: 0.5,
            },
            1,
        );
        sm.apply(
            &CoherenceCommand::SetNodeState {
                node_id: 1,
                state: vec![1.0; 64],
            },
            2,
        );

        let snapshot = sm.snapshot();

        let mut sm2 = CoherenceStateMachine::new(64);
        sm2.restore(snapshot);

        assert!((sm2.get_edge_energy((1, 2)).unwrap() - 0.5).abs() < 1e-6);
        assert!(sm2.get_node_state(1).is_some());
    }
}
