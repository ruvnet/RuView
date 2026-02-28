//! Adapter to ruvector-raft
//!
//! Wraps Raft consensus for coherence state replication.

use super::config::NodeRole;
use super::{DistributedCoherenceConfig, DistributedError, Result};

/// Command types for coherence state machine
#[derive(Debug, Clone)]
pub enum CoherenceCommand {
    /// Update energy for an edge
    UpdateEnergy { edge_id: (u64, u64), energy: f32 },
    /// Set node state vector
    SetNodeState { node_id: u64, state: Vec<f32> },
    /// Record coherence checkpoint
    Checkpoint { total_energy: f32, timestamp: u64 },
    /// Mark region as incoherent
    MarkIncoherent { region_id: u64, nodes: Vec<u64> },
    /// Clear incoherence flag
    ClearIncoherent { region_id: u64 },
}

impl CoherenceCommand {
    /// Serialize command to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        // Simple serialization format
        let mut bytes = Vec::new();
        match self {
            Self::UpdateEnergy { edge_id, energy } => {
                bytes.push(0);
                bytes.extend(edge_id.0.to_le_bytes());
                bytes.extend(edge_id.1.to_le_bytes());
                bytes.extend(energy.to_le_bytes());
            }
            Self::SetNodeState { node_id, state } => {
                bytes.push(1);
                bytes.extend(node_id.to_le_bytes());
                bytes.extend((state.len() as u32).to_le_bytes());
                for &v in state {
                    bytes.extend(v.to_le_bytes());
                }
            }
            Self::Checkpoint {
                total_energy,
                timestamp,
            } => {
                bytes.push(2);
                bytes.extend(total_energy.to_le_bytes());
                bytes.extend(timestamp.to_le_bytes());
            }
            Self::MarkIncoherent { region_id, nodes } => {
                bytes.push(3);
                bytes.extend(region_id.to_le_bytes());
                bytes.extend((nodes.len() as u32).to_le_bytes());
                for &n in nodes {
                    bytes.extend(n.to_le_bytes());
                }
            }
            Self::ClearIncoherent { region_id } => {
                bytes.push(4);
                bytes.extend(region_id.to_le_bytes());
            }
        }
        bytes
    }

    /// Deserialize command from bytes
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.is_empty() {
            return None;
        }

        let cmd_type = bytes[0];
        let data = &bytes[1..];

        match cmd_type {
            0 if data.len() >= 20 => {
                let src = u64::from_le_bytes(data[0..8].try_into().ok()?);
                let dst = u64::from_le_bytes(data[8..16].try_into().ok()?);
                let energy = f32::from_le_bytes(data[16..20].try_into().ok()?);
                Some(Self::UpdateEnergy {
                    edge_id: (src, dst),
                    energy,
                })
            }
            1 if data.len() >= 12 => {
                let node_id = u64::from_le_bytes(data[0..8].try_into().ok()?);
                let len = u32::from_le_bytes(data[8..12].try_into().ok()?) as usize;
                if data.len() < 12 + len * 4 {
                    return None;
                }
                let state: Vec<f32> = (0..len)
                    .map(|i| {
                        let offset = 12 + i * 4;
                        f32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
                    })
                    .collect();
                Some(Self::SetNodeState { node_id, state })
            }
            2 if data.len() >= 12 => {
                let total_energy = f32::from_le_bytes(data[0..4].try_into().ok()?);
                let timestamp = u64::from_le_bytes(data[4..12].try_into().ok()?);
                Some(Self::Checkpoint {
                    total_energy,
                    timestamp,
                })
            }
            3 if data.len() >= 12 => {
                let region_id = u64::from_le_bytes(data[0..8].try_into().ok()?);
                let len = u32::from_le_bytes(data[8..12].try_into().ok()?) as usize;
                if data.len() < 12 + len * 8 {
                    return None;
                }
                let nodes: Vec<u64> = (0..len)
                    .map(|i| {
                        let offset = 12 + i * 8;
                        u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap())
                    })
                    .collect();
                Some(Self::MarkIncoherent { region_id, nodes })
            }
            4 if data.len() >= 8 => {
                let region_id = u64::from_le_bytes(data[0..8].try_into().ok()?);
                Some(Self::ClearIncoherent { region_id })
            }
            _ => None,
        }
    }
}

/// Result of applying a command
#[derive(Debug, Clone)]
pub struct CommandResult {
    /// Log index where command was applied
    pub index: u64,
    /// Term when command was applied
    pub term: u64,
    /// Whether command was successful
    pub success: bool,
}

/// Adapter wrapping ruvector-raft for coherence coordination
#[derive(Debug)]
pub struct RaftAdapter {
    /// Configuration
    config: DistributedCoherenceConfig,
    /// Current role (simulated without actual Raft)
    role: NodeRole,
    /// Current term
    current_term: u64,
    /// Current leader ID
    current_leader: Option<String>,
    /// Log index
    log_index: u64,
    /// Pending commands (for simulation)
    pending_commands: Vec<CoherenceCommand>,
}

impl RaftAdapter {
    /// Create a new Raft adapter
    pub fn new(config: DistributedCoherenceConfig) -> Self {
        let is_leader = config.is_single_node();
        Self {
            role: if is_leader {
                NodeRole::Leader
            } else {
                NodeRole::Follower
            },
            current_term: 1,
            current_leader: if is_leader {
                Some(config.node_id.clone())
            } else {
                None
            },
            log_index: 0,
            pending_commands: Vec::new(),
            config,
        }
    }

    /// Get current role
    pub fn role(&self) -> NodeRole {
        self.role
    }

    /// Get current term
    pub fn current_term(&self) -> u64 {
        self.current_term
    }

    /// Get current leader
    pub fn current_leader(&self) -> Option<&str> {
        self.current_leader.as_deref()
    }

    /// Check if this node is the leader
    pub fn is_leader(&self) -> bool {
        self.role.is_leader()
    }

    /// Submit a command for replication
    pub fn submit_command(&mut self, command: CoherenceCommand) -> Result<CommandResult> {
        if !self.is_leader() {
            return Err(DistributedError::NotLeader {
                leader: self.current_leader.clone(),
            });
        }

        // In a real implementation, this would go through Raft
        self.log_index += 1;
        self.pending_commands.push(command);

        Ok(CommandResult {
            index: self.log_index,
            term: self.current_term,
            success: true,
        })
    }

    /// Update energy for an edge
    pub fn update_energy(&mut self, edge_id: (u64, u64), energy: f32) -> Result<CommandResult> {
        let command = CoherenceCommand::UpdateEnergy { edge_id, energy };
        self.submit_command(command)
    }

    /// Set node state
    pub fn set_node_state(&mut self, node_id: u64, state: Vec<f32>) -> Result<CommandResult> {
        let command = CoherenceCommand::SetNodeState { node_id, state };
        self.submit_command(command)
    }

    /// Record checkpoint
    pub fn checkpoint(&mut self, total_energy: f32, timestamp: u64) -> Result<CommandResult> {
        let command = CoherenceCommand::Checkpoint {
            total_energy,
            timestamp,
        };
        self.submit_command(command)
    }

    /// Mark region as incoherent
    pub fn mark_incoherent(&mut self, region_id: u64, nodes: Vec<u64>) -> Result<CommandResult> {
        let command = CoherenceCommand::MarkIncoherent { region_id, nodes };
        self.submit_command(command)
    }

    /// Clear incoherence flag
    pub fn clear_incoherent(&mut self, region_id: u64) -> Result<CommandResult> {
        let command = CoherenceCommand::ClearIncoherent { region_id };
        self.submit_command(command)
    }

    /// Get pending commands (for state machine application)
    pub fn take_pending_commands(&mut self) -> Vec<CoherenceCommand> {
        std::mem::take(&mut self.pending_commands)
    }

    /// Simulate leader election (for testing)
    pub fn become_leader(&mut self) {
        self.role = NodeRole::Leader;
        self.current_term += 1;
        self.current_leader = Some(self.config.node_id.clone());
    }

    /// Simulate stepping down
    pub fn step_down(&mut self) {
        self.role = NodeRole::Follower;
        self.current_leader = None;
    }

    /// Get cluster status
    pub fn cluster_status(&self) -> ClusterStatus {
        ClusterStatus {
            node_id: self.config.node_id.clone(),
            role: self.role,
            term: self.current_term,
            leader: self.current_leader.clone(),
            cluster_size: self.config.cluster_members.len(),
            quorum_size: self.config.quorum_size(),
            log_index: self.log_index,
        }
    }
}

/// Status of the Raft cluster
#[derive(Debug, Clone)]
pub struct ClusterStatus {
    /// This node's ID
    pub node_id: String,
    /// Current role
    pub role: NodeRole,
    /// Current term
    pub term: u64,
    /// Current leader (if known)
    pub leader: Option<String>,
    /// Total cluster size
    pub cluster_size: usize,
    /// Quorum size
    pub quorum_size: usize,
    /// Current log index
    pub log_index: u64,
}

impl ClusterStatus {
    /// Check if cluster is healthy (has leader)
    pub fn is_healthy(&self) -> bool {
        self.leader.is_some()
    }

    /// Check if this node can accept writes
    pub fn can_write(&self) -> bool {
        self.role.is_leader()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_creation() {
        let config = DistributedCoherenceConfig::single_node("node1");
        let adapter = RaftAdapter::new(config);

        assert!(adapter.is_leader());
        assert_eq!(adapter.current_term(), 1);
    }

    #[test]
    fn test_command_serialization() {
        let cmd = CoherenceCommand::UpdateEnergy {
            edge_id: (1, 2),
            energy: 0.5,
        };

        let bytes = cmd.to_bytes();
        let recovered = CoherenceCommand::from_bytes(&bytes).unwrap();

        if let CoherenceCommand::UpdateEnergy { edge_id, energy } = recovered {
            assert_eq!(edge_id, (1, 2));
            assert!((energy - 0.5).abs() < 1e-6);
        } else {
            panic!("Wrong command type");
        }
    }

    #[test]
    fn test_submit_command() {
        let config = DistributedCoherenceConfig::single_node("node1");
        let mut adapter = RaftAdapter::new(config);

        let result = adapter.update_energy((1, 2), 0.5).unwrap();
        assert!(result.success);
        assert_eq!(result.index, 1);

        let pending = adapter.take_pending_commands();
        assert_eq!(pending.len(), 1);
    }

    #[test]
    fn test_not_leader_error() {
        let config = DistributedCoherenceConfig {
            node_id: "node1".to_string(),
            cluster_members: vec![
                "node1".to_string(),
                "node2".to_string(),
                "node3".to_string(),
            ],
            ..Default::default()
        };
        let mut adapter = RaftAdapter::new(config);
        adapter.step_down();

        let result = adapter.update_energy((1, 2), 0.5);
        assert!(result.is_err());
    }

    #[test]
    fn test_cluster_status() {
        let config = DistributedCoherenceConfig::single_node("node1");
        let adapter = RaftAdapter::new(config);

        let status = adapter.cluster_status();
        assert!(status.is_healthy());
        assert!(status.can_write());
        assert_eq!(status.cluster_size, 1);
    }
}
