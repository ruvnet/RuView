//! Neural network layer implementations optimized for temporal prediction

use crate::error::{Result, TemporalNeuralError};
use nalgebra::{DMatrix, DVector};
use serde::{Deserialize, Serialize};
use rand::{Rng, distributions::{Distribution, Uniform}};
use std::f64::consts::PI;

/// GRU (Gated Recurrent Unit) layer optimized for micro-networks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GruLayer {
    /// Input size
    pub input_size: usize,
    /// Hidden state size
    pub hidden_size: usize,
    /// Reset gate weights (input)
    pub weight_ir: DMatrix<f64>,
    /// Reset gate weights (hidden)
    pub weight_hr: DMatrix<f64>,
    /// Reset gate bias
    pub bias_r: DVector<f64>,
    /// Update gate weights (input)
    pub weight_iz: DMatrix<f64>,
    /// Update gate weights (hidden)
    pub weight_hz: DMatrix<f64>,
    /// Update gate bias
    pub bias_z: DVector<f64>,
    /// New gate weights (input)
    pub weight_in: DMatrix<f64>,
    /// New gate weights (hidden)
    pub weight_hn: DMatrix<f64>,
    /// New gate bias
    pub bias_n: DVector<f64>,
    /// Current hidden state
    hidden_state: Option<DVector<f64>>,
}

impl GruLayer {
    /// Create a new GRU layer
    pub fn new(input_size: usize, hidden_size: usize) -> Self {
        let mut layer = Self {
            input_size,
            hidden_size,
            weight_ir: DMatrix::zeros(hidden_size, input_size),
            weight_hr: DMatrix::zeros(hidden_size, hidden_size),
            bias_r: DVector::zeros(hidden_size),
            weight_iz: DMatrix::zeros(hidden_size, input_size),
            weight_hz: DMatrix::zeros(hidden_size, hidden_size),
            bias_z: DVector::zeros(hidden_size),
            weight_in: DMatrix::zeros(hidden_size, input_size),
            weight_hn: DMatrix::zeros(hidden_size, hidden_size),
            bias_n: DVector::zeros(hidden_size),
            hidden_state: None,
        };

        layer.initialize_weights();
        layer
    }

    /// Initialize weights using Xavier/Glorot initialization
    pub fn initialize_weights(&mut self) {
        let mut rng = rand::thread_rng();

        // Xavier initialization scale
        let input_scale = (6.0 / (self.input_size + self.hidden_size) as f64).sqrt();
        let hidden_scale = (6.0 / (2.0 * self.hidden_size) as f64).sqrt();

        // Initialize input weights
        let uniform = Uniform::new(-input_scale, input_scale);
        self.weight_ir = DMatrix::from_fn(self.hidden_size, self.input_size, |_, _| {
            uniform.sample(&mut rng)
        });
        self.weight_iz = DMatrix::from_fn(self.hidden_size, self.input_size, |_, _| {
            uniform.sample(&mut rng)
        });
        self.weight_in = DMatrix::from_fn(self.hidden_size, self.input_size, |_, _| {
            uniform.sample(&mut rng)
        });

        // Initialize hidden weights
        let hidden_uniform = Uniform::new(-hidden_scale, hidden_scale);
        self.weight_hr = DMatrix::from_fn(self.hidden_size, self.hidden_size, |_, _| {
            hidden_uniform.sample(&mut rng)
        });
        self.weight_hz = DMatrix::from_fn(self.hidden_size, self.hidden_size, |_, _| {
            hidden_uniform.sample(&mut rng)
        });
        self.weight_hn = DMatrix::from_fn(self.hidden_size, self.hidden_size, |_, _| {
            hidden_uniform.sample(&mut rng)
        });

        // Initialize biases to small positive values for forget gates
        self.bias_z.fill(1.0); // Update gate bias - helps with gradient flow
    }

    /// Forward pass through GRU layer
    pub fn forward(&mut self, input: &DVector<f64>) -> Result<DVector<f64>> {
        if input.len() != self.input_size {
            return Err(TemporalNeuralError::ModelError {
                component: "GruLayer".to_string(),
                message: format!(
                    "Input size mismatch: expected {}, got {}",
                    self.input_size, input.len()
                ),
                context: vec![],
            });
        }

        // Initialize hidden state if needed
        if self.hidden_state.is_none() {
            self.hidden_state = Some(DVector::zeros(self.hidden_size));
        }

        let h_prev = self.hidden_state.as_ref().unwrap().clone();

        // Reset gate: r = sigmoid(W_ir @ x + W_hr @ h + b_r)
        let r = sigmoid(&(&self.weight_ir * input + &self.weight_hr * &h_prev + &self.bias_r));

        // Update gate: z = sigmoid(W_iz @ x + W_hz @ h + b_z)
        let z = sigmoid(&(&self.weight_iz * input + &self.weight_hz * &h_prev + &self.bias_z));

        // New gate: n = tanh(W_in @ x + W_hn @ (r ⊙ h) + b_n)
        let r_h = r.component_mul(&h_prev);
        let n = tanh(&(&self.weight_in * input + &self.weight_hn * &r_h + &self.bias_n));

        // Hidden state: h = (1 - z) ⊙ n + z ⊙ h_prev
        let one_minus_z = DVector::from_element(self.hidden_size, 1.0) - &z;
        let h_new = one_minus_z.component_mul(&n) + z.component_mul(&h_prev);

        self.hidden_state = Some(h_new.clone());
        Ok(h_new)
    }

    /// Process a sequence of inputs
    pub fn forward_sequence(&mut self, inputs: &DMatrix<f64>) -> Result<DMatrix<f64>> {
        let seq_len = inputs.ncols();
        let mut outputs = DMatrix::zeros(self.hidden_size, seq_len);

        for t in 0..seq_len {
            let input = inputs.column(t);
            let output = self.forward(&input.into())?;
            outputs.set_column(t, &output);
        }

        Ok(outputs)
    }

    /// Reset hidden state
    pub fn reset_state(&mut self) {
        self.hidden_state = None;
    }

    /// Get current hidden state
    pub fn get_state(&self) -> Option<&DVector<f64>> {
        self.hidden_state.as_ref()
    }

    /// Set hidden state (for initialization or transfer)
    pub fn set_state(&mut self, state: DVector<f64>) -> Result<()> {
        if state.len() != self.hidden_size {
            return Err(TemporalNeuralError::ModelError {
                component: "GruLayer".to_string(),
                message: format!(
                    "State size mismatch: expected {}, got {}",
                    self.hidden_size, state.len()
                ),
                context: vec![],
            });
        }
        self.hidden_state = Some(state);
        Ok(())
    }

    /// Get parameter count
    pub fn parameter_count(&self) -> usize {
        self.weight_ir.len() + self.weight_hr.len() + self.bias_r.len() +
        self.weight_iz.len() + self.weight_hz.len() + self.bias_z.len() +
        self.weight_in.len() + self.weight_hn.len() + self.bias_n.len()
    }
}

/// Temporal Convolutional Network (TCN) layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcnLayer {
    /// Number of input channels
    pub input_channels: usize,
    /// Number of output channels
    pub output_channels: usize,
    /// Kernel size
    pub kernel_size: usize,
    /// Dilation factor
    pub dilation: usize,
    /// Convolution weights
    pub weight: DMatrix<f64>,
    /// Bias terms
    pub bias: DVector<f64>,
    /// Whether to use residual connections
    pub residual: bool,
    /// Residual projection weights (if needed)
    pub residual_weight: Option<DMatrix<f64>>,
}

impl TcnLayer {
    /// Create a new TCN layer
    pub fn new(
        input_channels: usize,
        output_channels: usize,
        kernel_size: usize,
        dilation: usize,
        residual: bool,
    ) -> Self {
        let weight_size = output_channels * input_channels * kernel_size;
        let mut layer = Self {
            input_channels,
            output_channels,
            kernel_size,
            dilation,
            weight: DMatrix::zeros(output_channels, input_channels * kernel_size),
            bias: DVector::zeros(output_channels),
            residual,
            residual_weight: None,
        };

        // Create residual projection if channel sizes don't match
        if residual && input_channels != output_channels {
            layer.residual_weight = Some(DMatrix::zeros(output_channels, input_channels));
        }

        layer.initialize_weights();
        layer
    }

    /// Initialize weights
    pub fn initialize_weights(&mut self) {
        let mut rng = rand::thread_rng();
        let fan_in = self.input_channels * self.kernel_size;
        let fan_out = self.output_channels * self.kernel_size;
        let scale = (2.0 / (fan_in + fan_out) as f64).sqrt();

        let uniform = Uniform::new(-scale, scale);
        self.weight = DMatrix::from_fn(self.output_channels, self.input_channels * self.kernel_size, |_, _| {
            uniform.sample(&mut rng)
        });

        if let Some(ref mut res_weight) = self.residual_weight {
            let res_scale = (2.0 / (self.input_channels + self.output_channels) as f64).sqrt();
            let res_uniform = Uniform::new(-res_scale, res_scale);
            *res_weight = DMatrix::from_fn(self.output_channels, self.input_channels, |_, _| {
                res_uniform.sample(&mut rng)
            });
        }
    }

    /// Forward pass with causal convolution
    pub fn forward(&self, input: &DMatrix<f64>) -> Result<DMatrix<f64>> {
        let (channels, seq_len) = (input.nrows(), input.ncols());

        if channels != self.input_channels {
            return Err(TemporalNeuralError::ModelError {
                component: "TcnLayer".to_string(),
                message: format!(
                    "Input channel mismatch: expected {}, got {}",
                    self.input_channels, channels
                ),
                context: vec![],
            });
        }

        // Calculate output sequence length (causal convolution doesn't reduce length)
        let output_len = seq_len;
        let mut output = DMatrix::zeros(self.output_channels, output_len);

        // Perform causal dilated convolution
        for t in 0..output_len {
            for out_ch in 0..self.output_channels {
                let mut sum = self.bias[out_ch];

                for k in 0..self.kernel_size {
                    let input_t = t as i64 - (k * self.dilation) as i64;
                    if input_t >= 0 {
                        let input_t = input_t as usize;
                        for in_ch in 0..self.input_channels {
                            let weight_idx = in_ch * self.kernel_size + k;
                            sum += self.weight[(out_ch, weight_idx)] * input[(in_ch, input_t)];
                        }
                    }
                }

                output[(out_ch, t)] = sum;
            }
        }

        // Apply residual connection if enabled
        if self.residual {
            if let Some(ref res_weight) = self.residual_weight {
                // Project input to match output channels
                let residual = res_weight * input;
                output += residual;
            } else if self.input_channels == self.output_channels {
                // Direct residual connection
                output += input;
            }
        }

        Ok(output)
    }

    /// Get parameter count
    pub fn parameter_count(&self) -> usize {
        let mut count = self.weight.len() + self.bias.len();
        if let Some(ref res_weight) = self.residual_weight {
            count += res_weight.len();
        }
        count
    }
}

/// Dense (fully connected) layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DenseLayer {
    /// Input dimension
    pub input_dim: usize,
    /// Output dimension
    pub output_dim: usize,
    /// Weight matrix
    pub weight: DMatrix<f64>,
    /// Bias vector
    pub bias: DVector<f64>,
    /// Activation function
    pub activation: ActivationFunction,
}

/// Activation function types
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ActivationFunction {
    /// Linear (no activation)
    Linear,
    /// ReLU activation
    Relu,
    /// Tanh activation
    Tanh,
    /// Sigmoid activation
    Sigmoid,
    /// GELU activation
    Gelu,
}

impl DenseLayer {
    /// Create a new dense layer
    pub fn new(input_dim: usize, output_dim: usize, activation: ActivationFunction) -> Self {
        let mut layer = Self {
            input_dim,
            output_dim,
            weight: DMatrix::zeros(output_dim, input_dim),
            bias: DVector::zeros(output_dim),
            activation,
        };

        layer.initialize_weights();
        layer
    }

    /// Initialize weights
    pub fn initialize_weights(&mut self) {
        let mut rng = rand::thread_rng();
        let scale = (2.0 / (self.input_dim + self.output_dim) as f64).sqrt();

        let uniform = Uniform::new(-scale, scale);
        self.weight = DMatrix::from_fn(self.output_dim, self.input_dim, |_, _| {
            uniform.sample(&mut rng)
        });
    }

    /// Forward pass
    pub fn forward(&self, input: &DVector<f64>) -> Result<DVector<f64>> {
        if input.len() != self.input_dim {
            return Err(TemporalNeuralError::ModelError {
                component: "DenseLayer".to_string(),
                message: format!(
                    "Input dimension mismatch: expected {}, got {}",
                    self.input_dim, input.len()
                ),
                context: vec![],
            });
        }

        let linear_output = &self.weight * input + &self.bias;
        let activated_output = apply_activation(&linear_output, self.activation);

        Ok(activated_output)
    }

    /// Get parameter count
    pub fn parameter_count(&self) -> usize {
        self.weight.len() + self.bias.len()
    }
}

// Activation functions
fn sigmoid(x: &DVector<f64>) -> DVector<f64> {
    x.map(|val| 1.0 / (1.0 + (-val).exp()))
}

fn tanh(x: &DVector<f64>) -> DVector<f64> {
    x.map(|val| val.tanh())
}

fn relu(x: &DVector<f64>) -> DVector<f64> {
    x.map(|val| val.max(0.0))
}

fn gelu(x: &DVector<f64>) -> DVector<f64> {
    x.map(|val| 0.5 * val * (1.0 + (val * (2.0 / PI).sqrt()).tanh()))
}

fn apply_activation(x: &DVector<f64>, activation: ActivationFunction) -> DVector<f64> {
    match activation {
        ActivationFunction::Linear => x.clone(),
        ActivationFunction::Relu => relu(x),
        ActivationFunction::Tanh => tanh(x),
        ActivationFunction::Sigmoid => sigmoid(x),
        ActivationFunction::Gelu => gelu(x),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gru_forward() {
        let mut gru = GruLayer::new(4, 8);
        let input = DVector::from_vec(vec![1.0, 2.0, 3.0, 4.0]);

        let output = gru.forward(&input).unwrap();
        assert_eq!(output.len(), 8);

        // Test state persistence
        let output2 = gru.forward(&input).unwrap();
        assert_ne!(output, output2); // Should be different due to state
    }

    #[test]
    fn test_gru_sequence() {
        let mut gru = GruLayer::new(2, 4);
        let inputs = DMatrix::from_row_slice(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);

        let outputs = gru.forward_sequence(&inputs).unwrap();
        assert_eq!(outputs.shape(), (4, 3));
    }

    #[test]
    fn test_tcn_forward() {
        let tcn = TcnLayer::new(2, 4, 3, 1, true);
        let input = DMatrix::from_row_slice(2, 5, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]);

        let output = tcn.forward(&input).unwrap();
        assert_eq!(output.shape(), (4, 5));
    }

    #[test]
    fn test_dense_forward() {
        let dense = DenseLayer::new(4, 2, ActivationFunction::Relu);
        let input = DVector::from_vec(vec![1.0, -2.0, 3.0, -4.0]);

        let output = dense.forward(&input).unwrap();
        assert_eq!(output.len(), 2);
    }

    #[test]
    fn test_activation_functions() {
        let x = DVector::from_vec(vec![-2.0, -1.0, 0.0, 1.0, 2.0]);

        let relu_out = apply_activation(&x, ActivationFunction::Relu);
        assert_eq!(relu_out[0], 0.0); // ReLU of negative should be 0
        assert_eq!(relu_out[3], 1.0); // ReLU of positive should be unchanged

        let tanh_out = apply_activation(&x, ActivationFunction::Tanh);
        assert!(tanh_out[2] == 0.0); // tanh(0) = 0

        let sigmoid_out = apply_activation(&x, ActivationFunction::Sigmoid);
        assert!(sigmoid_out[2] == 0.5); // sigmoid(0) = 0.5
    }
}