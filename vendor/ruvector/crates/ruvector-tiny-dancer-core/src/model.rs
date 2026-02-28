//! FastGRNN model implementation
//!
//! Lightweight Gated Recurrent Neural Network optimized for inference

use crate::error::{Result, TinyDancerError};
use ndarray::{Array1, Array2};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// FastGRNN model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastGRNNConfig {
    /// Input dimension
    pub input_dim: usize,
    /// Hidden dimension
    pub hidden_dim: usize,
    /// Output dimension
    pub output_dim: usize,
    /// Gate non-linearity parameter
    pub nu: f32,
    /// Hidden non-linearity parameter
    pub zeta: f32,
    /// Rank constraint for low-rank factorization
    pub rank: Option<usize>,
}

impl Default for FastGRNNConfig {
    fn default() -> Self {
        Self {
            input_dim: 5, // 5 features from feature engineering
            hidden_dim: 8,
            output_dim: 1,
            nu: 1.0,
            zeta: 1.0,
            rank: Some(4),
        }
    }
}

/// FastGRNN model for neural routing
pub struct FastGRNN {
    config: FastGRNNConfig,
    /// Weight matrix for reset gate (U_r)
    w_reset: Array2<f32>,
    /// Weight matrix for update gate (U_u)
    w_update: Array2<f32>,
    /// Weight matrix for candidate (U_c)
    w_candidate: Array2<f32>,
    /// Recurrent weight matrix (W)
    w_recurrent: Array2<f32>,
    /// Output weight matrix
    w_output: Array2<f32>,
    /// Bias for reset gate
    b_reset: Array1<f32>,
    /// Bias for update gate
    b_update: Array1<f32>,
    /// Bias for candidate
    b_candidate: Array1<f32>,
    /// Bias for output
    b_output: Array1<f32>,
    /// Whether the model is quantized
    quantized: bool,
}

impl FastGRNN {
    /// Create a new FastGRNN model with the given configuration
    pub fn new(config: FastGRNNConfig) -> Result<Self> {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        // Xavier initialization
        let w_reset = Array2::from_shape_fn((config.hidden_dim, config.input_dim), |_| {
            rng.gen_range(-0.1..0.1)
        });
        let w_update = Array2::from_shape_fn((config.hidden_dim, config.input_dim), |_| {
            rng.gen_range(-0.1..0.1)
        });
        let w_candidate = Array2::from_shape_fn((config.hidden_dim, config.input_dim), |_| {
            rng.gen_range(-0.1..0.1)
        });
        let w_recurrent = Array2::from_shape_fn((config.hidden_dim, config.hidden_dim), |_| {
            rng.gen_range(-0.1..0.1)
        });
        let w_output = Array2::from_shape_fn((config.output_dim, config.hidden_dim), |_| {
            rng.gen_range(-0.1..0.1)
        });

        let b_reset = Array1::zeros(config.hidden_dim);
        let b_update = Array1::zeros(config.hidden_dim);
        let b_candidate = Array1::zeros(config.hidden_dim);
        let b_output = Array1::zeros(config.output_dim);

        Ok(Self {
            config,
            w_reset,
            w_update,
            w_candidate,
            w_recurrent,
            w_output,
            b_reset,
            b_update,
            b_candidate,
            b_output,
            quantized: false,
        })
    }

    /// Load model from a file (safetensors format)
    pub fn load<P: AsRef<Path>>(_path: P) -> Result<Self> {
        // TODO: Implement safetensors loading
        // For now, return a default model
        Self::new(FastGRNNConfig::default())
    }

    /// Save model to a file (safetensors format)
    pub fn save<P: AsRef<Path>>(&self, _path: P) -> Result<()> {
        // TODO: Implement safetensors saving
        Ok(())
    }

    /// Forward pass through the FastGRNN model
    ///
    /// # Arguments
    /// * `input` - Input vector (sequence of features)
    /// * `initial_hidden` - Optional initial hidden state
    ///
    /// # Returns
    /// Output score (typically between 0.0 and 1.0 after sigmoid)
    pub fn forward(&self, input: &[f32], initial_hidden: Option<&[f32]>) -> Result<f32> {
        if input.len() != self.config.input_dim {
            return Err(TinyDancerError::InvalidInput(format!(
                "Expected input dimension {}, got {}",
                self.config.input_dim,
                input.len()
            )));
        }

        let x = Array1::from_vec(input.to_vec());
        let mut h = if let Some(hidden) = initial_hidden {
            Array1::from_vec(hidden.to_vec())
        } else {
            Array1::zeros(self.config.hidden_dim)
        };

        // FastGRNN cell computation
        // r_t = sigmoid(W_r * x_t + b_r)
        let r = sigmoid(&(self.w_reset.dot(&x) + &self.b_reset), self.config.nu);

        // u_t = sigmoid(W_u * x_t + b_u)
        let u = sigmoid(&(self.w_update.dot(&x) + &self.b_update), self.config.nu);

        // c_t = tanh(W_c * x_t + W * (r_t ⊙ h_{t-1}) + b_c)
        let c = tanh(
            &(self.w_candidate.dot(&x) + self.w_recurrent.dot(&(&r * &h)) + &self.b_candidate),
            self.config.zeta,
        );

        // h_t = u_t ⊙ h_{t-1} + (1 - u_t) ⊙ c_t
        h = &u * &h + &((Array1::<f32>::ones(u.len()) - &u) * &c);

        // Output: y = W_out * h_t + b_out
        let output = self.w_output.dot(&h) + &self.b_output;

        // Apply sigmoid to get probability
        Ok(sigmoid_scalar(output[0]))
    }

    /// Batch inference for multiple inputs
    pub fn forward_batch(&self, inputs: &[Vec<f32>]) -> Result<Vec<f32>> {
        inputs
            .iter()
            .map(|input| self.forward(input, None))
            .collect()
    }

    /// Quantize the model to INT8
    pub fn quantize(&mut self) -> Result<()> {
        // TODO: Implement INT8 quantization
        self.quantized = true;
        Ok(())
    }

    /// Apply magnitude-based pruning
    pub fn prune(&mut self, sparsity: f32) -> Result<()> {
        if !(0.0..=1.0).contains(&sparsity) {
            return Err(TinyDancerError::InvalidInput(
                "Sparsity must be between 0.0 and 1.0".to_string(),
            ));
        }

        // TODO: Implement magnitude-based pruning
        Ok(())
    }

    /// Get model size in bytes
    pub fn size_bytes(&self) -> usize {
        let params = self.w_reset.len()
            + self.w_update.len()
            + self.w_candidate.len()
            + self.w_recurrent.len()
            + self.w_output.len()
            + self.b_reset.len()
            + self.b_update.len()
            + self.b_candidate.len()
            + self.b_output.len();

        params * if self.quantized { 1 } else { 4 } // 1 byte for INT8, 4 bytes for f32
    }

    /// Get configuration
    pub fn config(&self) -> &FastGRNNConfig {
        &self.config
    }
}

/// Sigmoid activation with scaling parameter
fn sigmoid(x: &Array1<f32>, scale: f32) -> Array1<f32> {
    x.mapv(|v| sigmoid_scalar(v * scale))
}

/// Scalar sigmoid with numerical stability
fn sigmoid_scalar(x: f32) -> f32 {
    if x > 0.0 {
        1.0 / (1.0 + (-x).exp())
    } else {
        let ex = x.exp();
        ex / (1.0 + ex)
    }
}

/// Tanh activation with scaling parameter
fn tanh(x: &Array1<f32>, scale: f32) -> Array1<f32> {
    x.mapv(|v| (v * scale).tanh())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fastgrnn_creation() {
        let config = FastGRNNConfig::default();
        let model = FastGRNN::new(config).unwrap();
        assert!(model.size_bytes() > 0);
    }

    #[test]
    fn test_forward_pass() {
        let config = FastGRNNConfig {
            input_dim: 10,
            hidden_dim: 8,
            output_dim: 1,
            ..Default::default()
        };
        let model = FastGRNN::new(config).unwrap();
        let input = vec![0.5; 10];
        let output = model.forward(&input, None).unwrap();
        assert!(output >= 0.0 && output <= 1.0);
    }

    #[test]
    fn test_batch_inference() {
        let config = FastGRNNConfig {
            input_dim: 10,
            ..Default::default()
        };
        let model = FastGRNN::new(config).unwrap();
        let inputs = vec![vec![0.5; 10], vec![0.3; 10], vec![0.8; 10]];
        let outputs = model.forward_batch(&inputs).unwrap();
        assert_eq!(outputs.len(), 3);
    }
}
