//! Pipeline Parallelism for Multi-ESP32 Inference

use heapless::Vec as HVec;
use super::protocol::{ChipId, FederationMessage};

pub const MAX_LAYERS_PER_CHIP: usize = 4;
pub const MAX_PIPELINE_DEPTH: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PipelineRole { Head, Middle, Tail, Standalone }

#[derive(Debug, Clone)]
pub struct PipelineConfig {
    pub num_chips: usize,
    pub position: usize,
    pub layer_start: usize,
    pub layer_count: usize,
    pub total_layers: usize,
    pub embed_dim: usize,
    pub micro_batch_size: usize,
}

impl PipelineConfig {
    pub fn for_chip(chip_pos: usize, num_chips: usize, total_layers: usize, embed_dim: usize) -> Self {
        let layers_per_chip = (total_layers + num_chips - 1) / num_chips;
        let layer_start = chip_pos * layers_per_chip;
        let layer_count = layers_per_chip.min(total_layers - layer_start);
        Self { num_chips, position: chip_pos, layer_start, layer_count, total_layers, embed_dim, micro_batch_size: 1 }
    }

    pub fn role(&self) -> PipelineRole {
        if self.num_chips == 1 { PipelineRole::Standalone }
        else if self.position == 0 { PipelineRole::Head }
        else if self.position == self.num_chips - 1 { PipelineRole::Tail }
        else { PipelineRole::Middle }
    }

    pub fn prev_chip(&self) -> Option<ChipId> {
        if self.position > 0 { Some(ChipId((self.position - 1) as u8)) } else { None }
    }

    pub fn next_chip(&self) -> Option<ChipId> {
        if self.position + 1 < self.num_chips { Some(ChipId((self.position + 1) as u8)) } else { None }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PipelineState { WaitingInput, Processing, WaitingSend, Idle }

#[derive(Debug, Clone)]
pub struct InFlightToken {
    pub seq_pos: u16,
    pub token_id: u16,
    pub current_layer: u8,
    pub activation: HVec<i8, 128>,
}

pub struct PipelineNode {
    config: PipelineConfig,
    state: PipelineState,
    chip_id: ChipId,
    seq_counter: u16,
    in_flight: HVec<InFlightToken, MAX_PIPELINE_DEPTH>,
    output_queue: HVec<InFlightToken, MAX_PIPELINE_DEPTH>,
    barrier_counter: u16,
}

impl PipelineNode {
    pub fn new(config: PipelineConfig) -> Self {
        Self {
            chip_id: ChipId(config.position as u8),
            config,
            state: PipelineState::Idle,
            seq_counter: 0,
            in_flight: HVec::new(),
            output_queue: HVec::new(),
            barrier_counter: 0,
        }
    }

    pub fn state(&self) -> PipelineState { self.state }
    pub fn handles_embedding(&self) -> bool { matches!(self.config.role(), PipelineRole::Head | PipelineRole::Standalone) }
    pub fn handles_output(&self) -> bool { matches!(self.config.role(), PipelineRole::Tail | PipelineRole::Standalone) }

    pub fn start_token(&mut self, token_id: u16) -> crate::Result<()> {
        if !self.handles_embedding() { return Err(crate::Error::UnsupportedFeature("Not head chip")); }
        if self.in_flight.len() >= MAX_PIPELINE_DEPTH { return Err(crate::Error::BufferOverflow); }

        let token = InFlightToken { seq_pos: self.seq_counter, token_id, current_layer: 0, activation: HVec::new() };
        self.in_flight.push(token).map_err(|_| crate::Error::BufferOverflow)?;
        self.seq_counter += 1;
        self.state = PipelineState::Processing;
        Ok(())
    }

    pub fn receive_activation(&mut self, msg: &FederationMessage) -> crate::Result<()> {
        let (layer_idx, position, data) = msg.get_activation_data()
            .ok_or(crate::Error::InvalidModel("Invalid activation"))?;

        let mut activation = HVec::new();
        for &d in data { activation.push(d as i8).map_err(|_| crate::Error::BufferOverflow)?; }

        let token = InFlightToken { seq_pos: position, token_id: 0, current_layer: layer_idx, activation };
        self.in_flight.push(token).map_err(|_| crate::Error::BufferOverflow)?;
        self.state = PipelineState::Processing;
        Ok(())
    }

    pub fn process_step<F>(&mut self, mut layer_fn: F) -> crate::Result<bool>
    where F: FnMut(usize, &mut [i8]) -> crate::Result<()>
    {
        if self.in_flight.is_empty() {
            self.state = PipelineState::WaitingInput;
            return Ok(false);
        }

        let token = &mut self.in_flight[0];
        let relative_layer = token.current_layer as usize - self.config.layer_start;

        if relative_layer < self.config.layer_count {
            let layer_idx = self.config.layer_start + relative_layer;
            layer_fn(layer_idx, &mut token.activation)?;
            token.current_layer += 1;
        }

        let next = token.current_layer as usize;
        if next >= self.config.layer_start + self.config.layer_count {
            if let Some(completed) = self.in_flight.pop() {
                self.output_queue.push(completed).map_err(|_| crate::Error::BufferOverflow)?;
            }
            self.state = PipelineState::WaitingSend;
        }
        Ok(true)
    }

    pub fn get_output(&mut self) -> Option<FederationMessage> {
        if self.output_queue.is_empty() { return None; }
        let token = self.output_queue.pop()?;
        let next_chip = self.config.next_chip()?;
        let data: heapless::Vec<i8, 128> = token.activation.iter().cloned().collect();
        FederationMessage::activation(self.chip_id, next_chip, token.seq_pos, token.current_layer, token.seq_pos, &data).ok()
    }

    pub fn has_final_output(&self) -> bool { self.handles_output() && !self.output_queue.is_empty() }

    pub fn get_final_output(&mut self) -> Option<HVec<i8, 128>> {
        if !self.handles_output() { return None; }
        self.output_queue.pop().map(|t| t.activation)
    }

    pub fn stats(&self) -> PipelineStats {
        PipelineStats {
            in_flight_count: self.in_flight.len(),
            output_queue_len: self.output_queue.len(),
            tokens_processed: self.seq_counter as usize,
            current_state: self.state,
        }
    }

    pub fn create_barrier(&mut self) -> FederationMessage {
        self.barrier_counter += 1;
        FederationMessage::barrier(self.chip_id, self.barrier_counter)
    }
}

#[derive(Debug, Clone)]
pub struct PipelineStats {
    pub in_flight_count: usize,
    pub output_queue_len: usize,
    pub tokens_processed: usize,
    pub current_state: PipelineState,
}

pub fn calculate_pipeline_efficiency(num_chips: usize, tokens: usize) -> f32 {
    if tokens <= num_chips {
        tokens as f32 / (num_chips as f32 * tokens as f32)
    } else {
        tokens as f32 / (tokens as f32 + (num_chips - 1) as f32)
    }
}
