//! Federation Coordinator - Cluster Management
//!
//! Manages the multi-chip cluster with self-learning optimization.
//! Integrates MicroLoRA for distributed fine-tuning.

use super::protocol::{ChipId, FederationMessage, MessageType, CommStats};
use super::{FederationConfig, FederationMode, FederationSpeedup, estimate_speedup};
use crate::optimizations::micro_lora::{MicroLoRA, LoRAConfig, LoRAStack};

/// Maximum chips in cluster
pub const MAX_CLUSTER_SIZE: usize = 8;

/// Cluster topology
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClusterTopology {
    /// Linear pipeline: 0 -> 1 -> 2 -> 3 -> 4
    Linear,
    /// Ring: 0 -> 1 -> 2 -> 3 -> 4 -> 0
    Ring,
    /// Star: 0 <-> all others
    Star,
    /// Mesh: all-to-all
    Mesh,
}

/// Chip status in cluster
#[derive(Debug, Clone)]
pub struct ChipStatus {
    /// Chip ID
    pub id: ChipId,
    /// Is chip active
    pub active: bool,
    /// Last heartbeat time (in ticks)
    pub last_heartbeat: u32,
    /// Current load (0-255)
    pub load: u8,
    /// Memory used (KB)
    pub memory_used_kb: u16,
    /// Tokens processed
    pub tokens_processed: u32,
}

/// Self-learning state for optimization
#[derive(Debug, Clone)]
pub struct SelfLearningState {
    /// Learning rate for LoRA updates
    pub learning_rate: i8,
    /// Gradient accumulation counter
    pub gradient_steps: u32,
    /// Average loss (fixed-point)
    pub avg_loss: i32,
    /// Best loss seen
    pub best_loss: i32,
    /// Adaptation enabled
    pub enabled: bool,
}

impl Default for SelfLearningState {
    fn default() -> Self {
        Self {
            learning_rate: 4,
            gradient_steps: 0,
            avg_loss: i32::MAX,
            best_loss: i32::MAX,
            enabled: false,
        }
    }
}

/// Federation coordinator
pub struct FederationCoordinator {
    /// This coordinator's chip ID
    chip_id: ChipId,
    /// Is this the master coordinator
    is_master: bool,
    /// Cluster configuration
    config: FederationConfig,
    /// Topology
    topology: ClusterTopology,
    /// Status of all chips
    chip_status: [Option<ChipStatus>; MAX_CLUSTER_SIZE],
    /// Communication stats
    comm_stats: CommStats,
    /// Self-learning state
    learning: SelfLearningState,
    /// Distributed LoRA adapters (one per layer shard)
    lora_stack: Option<LoRAStack<4>>,
    /// Current tick (for timeouts)
    current_tick: u32,
    /// Sequence counter
    seq_counter: u16,
}

impl FederationCoordinator {
    /// Create new coordinator
    pub fn new(config: FederationConfig, is_master: bool) -> Self {
        let chip_status = core::array::from_fn(|i| {
            if i < config.num_chips {
                Some(ChipStatus {
                    id: ChipId(i as u8),
                    active: i == config.chip_id.0 as usize,
                    last_heartbeat: 0,
                    load: 0,
                    memory_used_kb: 0,
                    tokens_processed: 0,
                })
            } else {
                None
            }
        });

        Self {
            chip_id: config.chip_id,
            is_master,
            topology: Self::optimal_topology(&config),
            config,
            chip_status,
            comm_stats: CommStats::default(),
            learning: SelfLearningState::default(),
            lora_stack: None,
            current_tick: 0,
            seq_counter: 0,
        }
    }

    /// Determine optimal topology for config
    fn optimal_topology(config: &FederationConfig) -> ClusterTopology {
        match config.mode {
            FederationMode::Pipeline => ClusterTopology::Linear,
            FederationMode::TensorParallel => ClusterTopology::Star,
            FederationMode::Speculative => ClusterTopology::Star,
            FederationMode::MixtureOfExperts => ClusterTopology::Mesh,
            _ => ClusterTopology::Linear,
        }
    }

    /// Initialize distributed LoRA for self-learning
    pub fn init_distributed_lora(&mut self, dim: usize, seed: u32) -> crate::Result<()> {
        let lora_config = LoRAConfig {
            rank: 1, // Minimal rank for distributed
            dim,
            scale: 8,
            frozen: false,
        };

        let mut stack = LoRAStack::new();

        // Each chip gets LoRA for its assigned layers
        let layers_per_chip = self.config.layers_per_chip;
        for i in 0..layers_per_chip.min(4) {
            let layer_seed = seed.wrapping_add(i as u32 * 1000);
            let adapter = MicroLoRA::new(lora_config, layer_seed)?;
            stack.add_adapter(i, adapter)?;
        }

        self.lora_stack = Some(stack);
        self.learning.enabled = true;

        Ok(())
    }

    /// Process tick (call regularly)
    pub fn tick(&mut self) {
        self.current_tick += 1;

        // Check for timeouts
        for status in self.chip_status.iter_mut().flatten() {
            if self.current_tick - status.last_heartbeat > 1000 {
                status.active = false;
            }
        }
    }

    /// Handle received message
    pub fn handle_message(&mut self, msg: &FederationMessage) -> Option<FederationMessage> {
        self.comm_stats.messages_received += 1;
        self.comm_stats.bytes_received += msg.payload.len() as u32;

        let msg_type = MessageType::from(msg.header.msg_type);

        match msg_type {
            MessageType::Heartbeat => {
                // Update chip status
                let src = msg.header.src as usize;
                if let Some(status) = self.chip_status.get_mut(src).and_then(|s| s.as_mut()) {
                    status.active = true;
                    status.last_heartbeat = self.current_tick;
                }
                None
            }

            MessageType::Discovery => {
                // Respond with our status
                Some(self.create_heartbeat())
            }

            MessageType::Barrier => {
                // Acknowledge barrier
                Some(FederationMessage::new(
                    MessageType::Ack,
                    self.chip_id,
                    ChipId(msg.header.src),
                    msg.header.seq,
                ))
            }

            _ => None,
        }
    }

    /// Create heartbeat message
    pub fn create_heartbeat(&mut self) -> FederationMessage {
        self.seq_counter += 1;
        let mut msg = FederationMessage::new(
            MessageType::Heartbeat,
            self.chip_id,
            ChipId::BROADCAST,
            self.seq_counter,
        );

        // Add load info to payload
        if let Some(status) = &self.chip_status[self.chip_id.0 as usize] {
            let _ = msg.payload.push(status.load);
            let _ = msg.payload.push((status.memory_used_kb & 0xFF) as u8);
            let _ = msg.payload.push((status.memory_used_kb >> 8) as u8);
        }
        msg.header.payload_len = msg.payload.len() as u16;
        msg.update_checksum();

        self.comm_stats.messages_sent += 1;
        msg
    }

    /// Get number of active chips
    pub fn active_chip_count(&self) -> usize {
        self.chip_status.iter().filter(|s| s.as_ref().is_some_and(|s| s.active)).count()
    }

    /// Estimate current speedup based on active chips
    pub fn current_speedup(&self) -> FederationSpeedup {
        let active = self.active_chip_count();
        let mut effective_config = self.config.clone();
        effective_config.num_chips = active;
        estimate_speedup(&effective_config)
    }

    /// Update learning state with loss
    pub fn update_learning(&mut self, loss: i32) {
        if !self.learning.enabled {
            return;
        }

        self.learning.gradient_steps += 1;

        // Exponential moving average of loss
        if self.learning.avg_loss == i32::MAX {
            self.learning.avg_loss = loss;
        } else {
            self.learning.avg_loss = (self.learning.avg_loss * 15 + loss) / 16;
        }

        // Track best
        if loss < self.learning.best_loss {
            self.learning.best_loss = loss;
        }

        // Adaptive learning rate
        if self.learning.gradient_steps % 100 == 0 {
            if self.learning.avg_loss < self.learning.best_loss * 11 / 10 {
                // Good progress, increase LR
                self.learning.learning_rate = (self.learning.learning_rate + 1).min(16);
            } else {
                // Slow progress, decrease LR
                self.learning.learning_rate = (self.learning.learning_rate - 1).max(1);
            }
        }
    }

    /// Apply distributed LoRA update
    #[cfg(not(feature = "frozen"))]
    pub fn apply_lora_gradient(
        &mut self,
        layer_idx: usize,
        input: &[i8],
        grad_output: &[i32],
    ) {
        if let Some(ref mut stack) = self.lora_stack {
            if let Some(lora) = stack.get(layer_idx) {
                lora.update(input, grad_output, self.learning.learning_rate);
            }
        }
    }

    /// Get LoRA adapter for a layer
    pub fn get_lora(&mut self, layer_idx: usize) -> Option<&mut MicroLoRA> {
        self.lora_stack.as_mut()?.get(layer_idx)
    }

    /// Get cluster statistics
    pub fn stats(&self) -> ClusterStats {
        let total_tokens: u32 = self.chip_status.iter()
            .filter_map(|s| s.as_ref())
            .map(|s| s.tokens_processed)
            .sum();

        let total_memory: u32 = self.chip_status.iter()
            .filter_map(|s| s.as_ref())
            .map(|s| s.memory_used_kb as u32)
            .sum();

        ClusterStats {
            active_chips: self.active_chip_count(),
            total_chips: self.config.num_chips,
            total_tokens_processed: total_tokens,
            total_memory_kb: total_memory,
            messages_sent: self.comm_stats.messages_sent,
            messages_received: self.comm_stats.messages_received,
            current_speedup: self.current_speedup(),
            learning_enabled: self.learning.enabled,
            learning_rate: self.learning.learning_rate,
            avg_loss: self.learning.avg_loss,
        }
    }

    /// Update chip's token count
    pub fn record_tokens(&mut self, count: u32) {
        if let Some(status) = self.chip_status.get_mut(self.chip_id.0 as usize).and_then(|s| s.as_mut()) {
            status.tokens_processed += count;
        }
    }

    /// Update chip's memory usage
    pub fn update_memory_usage(&mut self, kb: u16) {
        if let Some(status) = self.chip_status.get_mut(self.chip_id.0 as usize).and_then(|s| s.as_mut()) {
            status.memory_used_kb = kb;
        }
    }
}

/// Cluster statistics
#[derive(Debug, Clone)]
pub struct ClusterStats {
    /// Active chips
    pub active_chips: usize,
    /// Total chips configured
    pub total_chips: usize,
    /// Total tokens processed
    pub total_tokens_processed: u32,
    /// Total memory used (KB)
    pub total_memory_kb: u32,
    /// Messages sent
    pub messages_sent: u32,
    /// Messages received
    pub messages_received: u32,
    /// Current speedup estimate
    pub current_speedup: FederationSpeedup,
    /// Self-learning enabled
    pub learning_enabled: bool,
    /// Current learning rate
    pub learning_rate: i8,
    /// Average loss
    pub avg_loss: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordinator_creation() {
        let config = FederationConfig::default();
        let coord = FederationCoordinator::new(config, true);

        assert_eq!(coord.active_chip_count(), 1); // Only self is active initially
    }

    #[test]
    fn test_distributed_lora() {
        let config = FederationConfig::default();
        let mut coord = FederationCoordinator::new(config, true);

        coord.init_distributed_lora(32, 42).unwrap();

        assert!(coord.learning.enabled);
        assert!(coord.get_lora(0).is_some());
    }

    #[test]
    fn test_learning_update() {
        let config = FederationConfig::default();
        let mut coord = FederationCoordinator::new(config, true);
        coord.learning.enabled = true;

        coord.update_learning(1000);
        coord.update_learning(900);
        coord.update_learning(800);

        assert!(coord.learning.avg_loss < 1000);
        assert_eq!(coord.learning.best_loss, 800);
    }
}
