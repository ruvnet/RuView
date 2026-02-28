//! Pipeline Parallelism for Multi-ESP32 Inference
//!
//! Distributes layers across chips for linear scaling with model size.
//! Each chip processes its assigned layers and passes activations to the next.
//!
//! # 5-Chip Pipeline Example
//!
//! ```text
//! Token 0: [C0:embed+L0] → [C1:L1-2] → [C2:L3-4] → [C3:L5-6] → [C4:L7+head]
//! Token 1:    idle        [C0:embed]  [C1:L1-2]  [C2:L3-4]  [C3:L5-6]
//! Token 2:    idle           idle     [C0:embed] [C1:L1-2]  [C2:L3-4]
//! ...
//! ```

use heapless::Vec as HVec;
use super::protocol::{ChipId, FederationMessage};

/// Maximum layers per chip
pub const MAX_LAYERS_PER_CHIP: usize = 4;
/// Pipeline depth (tokens in flight)
pub const MAX_PIPELINE_DEPTH: usize = 8;

/// Role in the pipeline
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PipelineRole {
    /// First chip: handles embedding + first layers
    Head,
    /// Middle chip: processes middle layers
    Middle,
    /// Last chip: final layers + output head
    Tail,
    /// Single chip mode (no pipeline)
    Standalone,
}

/// Pipeline configuration
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Total chips in pipeline
    pub num_chips: usize,
    /// This chip's position (0 = head)
    pub position: usize,
    /// Layers assigned to this chip
    pub layer_start: usize,
    /// Number of layers on this chip
    pub layer_count: usize,
    /// Total layers in model
    pub total_layers: usize,
    /// Embedding dimension
    pub embed_dim: usize,
    /// Enable micro-batching
    pub micro_batch_size: usize,
}

impl PipelineConfig {
    /// Create config for a specific chip in the pipeline
    pub fn for_chip(
        chip_pos: usize,
        num_chips: usize,
        total_layers: usize,
        embed_dim: usize,
    ) -> Self {
        let layers_per_chip = (total_layers + num_chips - 1) / num_chips;
        let layer_start = chip_pos * layers_per_chip;
        let layer_count = layers_per_chip.min(total_layers - layer_start);

        Self {
            num_chips,
            position: chip_pos,
            layer_start,
            layer_count,
            total_layers,
            embed_dim,
            micro_batch_size: 1,
        }
    }

    /// Get role of this chip
    pub fn role(&self) -> PipelineRole {
        if self.num_chips == 1 {
            PipelineRole::Standalone
        } else if self.position == 0 {
            PipelineRole::Head
        } else if self.position == self.num_chips - 1 {
            PipelineRole::Tail
        } else {
            PipelineRole::Middle
        }
    }

    /// Previous chip in pipeline (if any)
    pub fn prev_chip(&self) -> Option<ChipId> {
        if self.position > 0 {
            Some(ChipId((self.position - 1) as u8))
        } else {
            None
        }
    }

    /// Next chip in pipeline (if any)
    pub fn next_chip(&self) -> Option<ChipId> {
        if self.position + 1 < self.num_chips {
            Some(ChipId((self.position + 1) as u8))
        } else {
            None
        }
    }
}

/// Pipeline state for a chip
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PipelineState {
    /// Waiting for input from previous chip
    WaitingInput,
    /// Processing layers
    Processing,
    /// Waiting to send output
    WaitingSend,
    /// Idle (pipeline bubble)
    Idle,
}

/// In-flight token tracking
#[derive(Debug, Clone)]
pub struct InFlightToken {
    /// Sequence position
    pub seq_pos: u16,
    /// Token ID
    pub token_id: u16,
    /// Current layer being processed
    pub current_layer: u8,
    /// Activation data (INT8)
    pub activation: HVec<i8, 128>,
}

/// Pipeline node managing this chip's portion
pub struct PipelineNode {
    /// Configuration
    config: PipelineConfig,
    /// Current state
    state: PipelineState,
    /// Chip ID
    chip_id: ChipId,
    /// Sequence counter
    seq_counter: u16,
    /// Tokens in flight in the pipeline
    in_flight: HVec<InFlightToken, MAX_PIPELINE_DEPTH>,
    /// Completed tokens waiting to send
    output_queue: HVec<InFlightToken, MAX_PIPELINE_DEPTH>,
    /// Input buffer for receiving activations
    input_buffer: HVec<i8, 256>,
    /// Barrier counter for synchronization
    barrier_counter: u16,
}

impl PipelineNode {
    /// Create new pipeline node
    pub fn new(config: PipelineConfig) -> Self {
        Self {
            chip_id: ChipId(config.position as u8),
            config,
            state: PipelineState::Idle,
            seq_counter: 0,
            in_flight: HVec::new(),
            output_queue: HVec::new(),
            input_buffer: HVec::new(),
            barrier_counter: 0,
        }
    }

    /// Get current pipeline state
    pub fn state(&self) -> PipelineState {
        self.state
    }

    /// Check if this chip should handle embedding
    pub fn handles_embedding(&self) -> bool {
        self.config.role() == PipelineRole::Head ||
        self.config.role() == PipelineRole::Standalone
    }

    /// Check if this chip should handle output head
    pub fn handles_output(&self) -> bool {
        self.config.role() == PipelineRole::Tail ||
        self.config.role() == PipelineRole::Standalone
    }

    /// Start processing a new token (head chip only)
    pub fn start_token(&mut self, token_id: u16) -> crate::Result<()> {
        if !self.handles_embedding() {
            return Err(crate::Error::UnsupportedFeature("Not head chip"));
        }

        if self.in_flight.len() >= MAX_PIPELINE_DEPTH {
            return Err(crate::Error::BufferOverflow);
        }

        let token = InFlightToken {
            seq_pos: self.seq_counter,
            token_id,
            current_layer: 0,
            activation: HVec::new(),
        };

        self.in_flight.push(token).map_err(|_| crate::Error::BufferOverflow)?;
        self.seq_counter += 1;
        self.state = PipelineState::Processing;

        Ok(())
    }

    /// Receive activation from previous chip
    pub fn receive_activation(&mut self, msg: &FederationMessage) -> crate::Result<()> {
        let (layer_idx, position, data) = msg.get_activation_data()
            .ok_or(crate::Error::InvalidModel("Invalid activation message"))?;

        // Create in-flight token from received data
        let mut activation = HVec::new();
        for &d in data {
            activation.push(d as i8).map_err(|_| crate::Error::BufferOverflow)?;
        }

        let token = InFlightToken {
            seq_pos: position,
            token_id: 0, // Not needed for middle/tail chips
            current_layer: layer_idx,
            activation,
        };

        self.in_flight.push(token).map_err(|_| crate::Error::BufferOverflow)?;
        self.state = PipelineState::Processing;

        Ok(())
    }

    /// Process one step (one layer for one token)
    /// Returns true if there's work to do
    pub fn process_step<F>(&mut self, mut layer_fn: F) -> crate::Result<bool>
    where
        F: FnMut(usize, &mut [i8]) -> crate::Result<()>,
    {
        if self.in_flight.is_empty() {
            self.state = PipelineState::WaitingInput;
            return Ok(false);
        }

        // Process first token in queue
        let token = &mut self.in_flight[0];

        // Determine which layer to process
        let relative_layer = token.current_layer as usize - self.config.layer_start;

        if relative_layer < self.config.layer_count {
            // Process this layer
            let layer_idx = self.config.layer_start + relative_layer;
            layer_fn(layer_idx, &mut token.activation)?;
            token.current_layer += 1;
        }

        // Check if done with this chip's layers
        let next_layer = token.current_layer as usize;
        if next_layer >= self.config.layer_start + self.config.layer_count {
            // Move to output queue
            if let Some(completed) = self.in_flight.pop() {
                self.output_queue.push(completed).map_err(|_| crate::Error::BufferOverflow)?;
            }
            self.state = PipelineState::WaitingSend;
        }

        Ok(true)
    }

    /// Get activation to send to next chip
    pub fn get_output(&mut self) -> Option<FederationMessage> {
        if self.output_queue.is_empty() {
            return None;
        }

        let token = self.output_queue.pop()?;
        let next_chip = self.config.next_chip()?;

        // Convert activation to bytes
        let data: Vec<i8> = token.activation.iter().cloned().collect();

        FederationMessage::activation(
            self.chip_id,
            next_chip,
            token.seq_pos,
            token.current_layer,
            token.seq_pos,
            &data,
        ).ok()
    }

    /// Check if output is available (for tail chip)
    pub fn has_final_output(&self) -> bool {
        self.handles_output() && !self.output_queue.is_empty()
    }

    /// Get final output logits (tail chip only)
    pub fn get_final_output(&mut self) -> Option<HVec<i8, 128>> {
        if !self.handles_output() {
            return None;
        }

        let token = self.output_queue.pop()?;
        Some(token.activation)
    }

    /// Get pipeline statistics
    pub fn stats(&self) -> PipelineStats {
        PipelineStats {
            in_flight_count: self.in_flight.len(),
            output_queue_len: self.output_queue.len(),
            tokens_processed: self.seq_counter as usize,
            current_state: self.state,
        }
    }

    /// Create synchronization barrier
    pub fn create_barrier(&mut self) -> FederationMessage {
        self.barrier_counter += 1;
        FederationMessage::barrier(self.chip_id, self.barrier_counter)
    }
}

/// Pipeline statistics
#[derive(Debug, Clone)]
pub struct PipelineStats {
    /// Tokens currently in pipeline
    pub in_flight_count: usize,
    /// Tokens waiting to send
    pub output_queue_len: usize,
    /// Total tokens processed
    pub tokens_processed: usize,
    /// Current state
    pub current_state: PipelineState,
}

/// Calculate pipeline efficiency
pub fn calculate_pipeline_efficiency(
    num_chips: usize,
    tokens_generated: usize,
) -> f32 {
    // Pipeline efficiency = useful work / total work
    // With N chips, first N-1 tokens have bubble overhead
    if tokens_generated <= num_chips {
        tokens_generated as f32 / (num_chips as f32 * tokens_generated as f32)
    } else {
        // After warmup, efficiency approaches 100%
        let warmup_overhead = (num_chips - 1) as f32;
        let useful_work = tokens_generated as f32;
        useful_work / (useful_work + warmup_overhead)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_config() {
        // 5 chips, 10 layers
        let config = PipelineConfig::for_chip(0, 5, 10, 64);
        assert_eq!(config.role(), PipelineRole::Head);
        assert_eq!(config.layer_start, 0);
        assert_eq!(config.layer_count, 2);

        let config = PipelineConfig::for_chip(2, 5, 10, 64);
        assert_eq!(config.role(), PipelineRole::Middle);
        assert_eq!(config.layer_start, 4);

        let config = PipelineConfig::for_chip(4, 5, 10, 64);
        assert_eq!(config.role(), PipelineRole::Tail);
    }

    #[test]
    fn test_pipeline_efficiency() {
        // After 100 tokens, efficiency should be high
        let eff = calculate_pipeline_efficiency(5, 100);
        assert!(eff > 0.95);

        // During warmup, efficiency is lower
        let eff_warmup = calculate_pipeline_efficiency(5, 5);
        assert!(eff_warmup < 0.5);
    }
}
