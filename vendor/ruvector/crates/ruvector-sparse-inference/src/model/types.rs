//! Core types for model inference

use std::collections::HashMap;

/// Generic tensor representation
#[derive(Debug, Clone)]
pub struct Tensor {
    pub data: Vec<f32>,
    pub shape: Vec<u64>,
    pub name: String,
}

impl Tensor {
    pub fn new(data: Vec<f32>, shape: Vec<u64>, name: String) -> Self {
        Self { data, shape, name }
    }

    pub fn zeros(shape: Vec<u64>, name: String) -> Self {
        let size = shape.iter().product::<u64>() as usize;
        Self {
            data: vec![0.0; size],
            shape,
            name,
        }
    }

    pub fn size(&self) -> usize {
        self.data.len()
    }

    pub fn reshape(&mut self, new_shape: Vec<u64>) {
        let new_size = new_shape.iter().product::<u64>() as usize;
        assert_eq!(
            new_size,
            self.size(),
            "Reshape size mismatch: {} vs {}",
            new_size,
            self.size()
        );
        self.shape = new_shape;
    }
}

/// Model input configuration
#[derive(Debug, Clone)]
pub struct ModelInput {
    pub input_ids: Vec<u64>,
    pub attention_mask: Option<Vec<u8>>,
    pub position_ids: Option<Vec<u64>>,
}

impl ModelInput {
    pub fn new(input_ids: Vec<u64>) -> Self {
        Self {
            input_ids,
            attention_mask: None,
            position_ids: None,
        }
    }

    pub fn with_attention_mask(mut self, mask: Vec<u8>) -> Self {
        self.attention_mask = Some(mask);
        self
    }

    pub fn with_position_ids(mut self, positions: Vec<u64>) -> Self {
        self.position_ids = Some(positions);
        self
    }

    pub fn sequence_length(&self) -> usize {
        self.input_ids.len()
    }
}

/// Model output
#[derive(Debug, Clone)]
pub struct ModelOutput {
    pub logits: Vec<f32>,
    pub hidden_states: Option<Vec<Vec<f32>>>,
    pub attentions: Option<Vec<Vec<f32>>>,
}

impl ModelOutput {
    pub fn new(logits: Vec<f32>) -> Self {
        Self {
            logits,
            hidden_states: None,
            attentions: None,
        }
    }

    pub fn with_hidden_states(mut self, states: Vec<Vec<f32>>) -> Self {
        self.hidden_states = Some(states);
        self
    }
}

/// Inference configuration
#[derive(Debug, Clone)]
pub struct InferenceConfig {
    /// Sparsity level (0.0 = dense, 1.0 = maximum sparsity)
    pub sparsity: f32,

    /// Sparsity threshold for neuron activation
    pub sparsity_threshold: f32,

    /// Temperature for sampling
    pub temperature: f32,

    /// Top-k sampling
    pub top_k: Option<usize>,

    /// Top-p (nucleus) sampling
    pub top_p: Option<f32>,

    /// Use sparse FFN computation
    pub use_sparse_ffn: bool,

    /// Number of active neurons per layer
    pub active_neurons_per_layer: Option<usize>,

    /// Return hidden states
    pub output_hidden_states: bool,

    /// Return attention weights
    pub output_attentions: bool,
}

impl Default for InferenceConfig {
    fn default() -> Self {
        Self {
            sparsity: 0.9,
            sparsity_threshold: 0.01,
            temperature: 1.0,
            top_k: None,
            top_p: None,
            use_sparse_ffn: true,
            active_neurons_per_layer: None,
            output_hidden_states: false,
            output_attentions: false,
        }
    }
}

/// Calibration statistics
#[derive(Debug, Clone)]
pub struct CalibrationStats {
    pub num_samples: usize,
    pub average_sparsity: f32,
    pub layer_stats: HashMap<usize, LayerStats>,
}

#[derive(Debug, Clone)]
pub struct LayerStats {
    pub active_neurons: usize,
    pub total_neurons: usize,
    pub sparsity: f32,
}
