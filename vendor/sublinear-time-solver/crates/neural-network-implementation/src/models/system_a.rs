//! System A: Traditional micro-neural network implementation
//!
//! This module implements the baseline traditional micro-network for comparison
//! against the temporal solver approach. It uses standard neural architectures
//! without mathematical solver integration.

use crate::{
    config::ModelConfig,
    error::{Result, TemporalNeuralError},
    models::{
        layers::{GruLayer, TcnLayer, DenseLayer, ActivationFunction},
        ModelTrait, ModelParams, ParameterStats,
    },
};
use nalgebra::{DMatrix, DVector};
use serde::{Deserialize, Serialize};

/// System A: Traditional micro-neural network
///
/// This system implements a standard approach using either GRU or TCN
/// for sequence modeling followed by a dense output layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemA {
    /// Model configuration
    config: ModelConfig,
    /// Parameters for the system
    params: SystemAParams,
    /// Current architecture type
    architecture: ArchitectureType,
}

/// Architecture variants for System A
#[derive(Debug, Clone, Serialize, Deserialize)]
enum ArchitectureType {
    /// GRU-based architecture
    Gru {
        /// GRU layers
        layers: Vec<GruLayer>,
        /// Output projection layer
        output_layer: DenseLayer,
    },
    /// TCN-based architecture
    Tcn {
        /// TCN layers
        layers: Vec<TcnLayer>,
        /// Global pooling type
        pooling: PoolingType,
        /// Output projection layer
        output_layer: DenseLayer,
    },
}

/// Pooling types for TCN architecture
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
enum PoolingType {
    /// Take the last timestep
    Last,
    /// Global average pooling
    Average,
    /// Global max pooling
    Max,
    /// Attention-based pooling
    Attention,
}

/// Parameters for System A
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemAParams {
    /// All parameter values flattened
    values: Vec<f64>,
    /// Gradients for backpropagation
    gradients: Vec<f64>,
    /// Parameter structure metadata
    structure: ParameterStructure,
}

/// Metadata about parameter structure for serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ParameterStructure {
    /// Total parameter count
    total_params: usize,
    /// Parameter layout (layer_name -> (start_idx, count))
    layout: Vec<(String, usize, usize)>,
}

impl SystemA {
    /// Create a new System A model
    pub fn new(config: &ModelConfig) -> Result<Self> {
        let architecture = match config.model_type.as_str() {
            "micro_gru" => Self::create_gru_architecture(config)?,
            "micro_tcn" => Self::create_tcn_architecture(config)?,
            _ => {
                return Err(TemporalNeuralError::ConfigurationError {
                    message: format!("Unsupported model type: {}", config.model_type),
                    field: Some("model_type".to_string()),
                });
            }
        };

        let params = SystemAParams::initialize(config, &mut rand::thread_rng());

        Ok(Self {
            config: config.clone(),
            params,
            architecture,
        })
    }

    fn create_gru_architecture(config: &ModelConfig) -> Result<ArchitectureType> {
        let mut layers = Vec::new();

        // First layer takes input features
        let first_layer = GruLayer::new(config.hidden_size, config.hidden_size);
        layers.push(first_layer);

        // Additional layers if specified
        for _ in 1..config.num_layers {
            let layer = GruLayer::new(config.hidden_size, config.hidden_size);
            layers.push(layer);
        }

        // Output layer - predict 2D position (x, y) at horizon
        let activation = match config.activation.as_str() {
            "relu" => ActivationFunction::Relu,
            "tanh" => ActivationFunction::Tanh,
            "gelu" => ActivationFunction::Gelu,
            "linear" => ActivationFunction::Linear,
            _ => ActivationFunction::Tanh, // Default
        };

        let output_layer = DenseLayer::new(config.hidden_size, 2, activation);

        Ok(ArchitectureType::Gru {
            layers,
            output_layer,
        })
    }

    fn create_tcn_architecture(config: &ModelConfig) -> Result<ArchitectureType> {
        let mut layers = Vec::new();
        let mut channels = config.hidden_size as usize;

        // Create dilated TCN layers
        for layer_idx in 0..config.num_layers as usize {
            let dilation = 2_usize.pow(layer_idx as u32);
            let layer = TcnLayer::new(
                channels,
                config.hidden_size as usize,
                3, // kernel size
                dilation,
                config.residual,
            );
            layers.push(layer);
            channels = config.hidden_size as usize;
        }

        // Global pooling strategy
        let pooling = PoolingType::Last; // Simple approach - take last timestep

        // Output layer
        let activation = match config.activation.as_str() {
            "relu" => ActivationFunction::Relu,
            "tanh" => ActivationFunction::Tanh,
            "gelu" => ActivationFunction::Gelu,
            "linear" => ActivationFunction::Linear,
            _ => ActivationFunction::Tanh,
        };

        let output_layer = DenseLayer::new(config.hidden_size as usize, 2, activation);

        Ok(ArchitectureType::Tcn {
            layers,
            pooling,
            output_layer,
        })
    }

    /// Apply pooling to TCN output
    fn apply_pooling(output: &DMatrix<f64>, pooling: PoolingType) -> DVector<f64> {
        match pooling {
            PoolingType::Last => {
                // Take the last timestep
                output.column(output.ncols() - 1).into()
            }
            PoolingType::Average => {
                // Global average pooling
                let mut result = DVector::zeros(output.nrows());
                for i in 0..output.nrows() {
                    result[i] = output.row(i).mean();
                }
                result
            }
            PoolingType::Max => {
                // Global max pooling
                let mut result = DVector::zeros(output.nrows());
                for i in 0..output.nrows() {
                    result[i] = output.row(i).max();
                }
                result
            }
            PoolingType::Attention => {
                // Simple attention: uniform weights for now
                // In a full implementation, this would be learned
                let seq_len = output.ncols();
                let weights = DVector::from_element(seq_len, 1.0 / seq_len as f64);
                output * weights
            }
        }
    }
}

impl ModelTrait for SystemA {
    type Params = SystemAParams;

    fn new(config: &ModelConfig) -> Result<Self> {
        Self::new(config)
    }

    fn forward(&self, input: &DMatrix<f64>) -> Result<DVector<f64>> {
        self.validate_input(input)?;

        match &self.architecture {
            ArchitectureType::Gru { layers, output_layer } => {
                // Process through GRU layers
                let mut current_input = input.clone();

                // For GRU, we need to process the sequence through each layer
                // Input shape: (features, sequence_length)
                // We need to process timestep by timestep

                let seq_len = input.ncols();
                let mut hidden_states = Vec::new();

                // Create temporary GRU layers for forward pass (clone to avoid mutation issues)
                let mut temp_layers: Vec<GruLayer> = layers.iter().cloned().collect();

                // Process sequence through all GRU layers
                for t in 0..seq_len {
                    let timestep_input = current_input.column(t).into();

                    // Process through each layer for this timestep
                    let mut layer_input = timestep_input;
                    for layer in temp_layers.iter_mut() {
                        layer_input = layer.forward(&layer_input)?;
                    }

                    hidden_states.push(layer_input);
                }

                // Take the final hidden state
                let final_hidden = hidden_states.last().unwrap();

                // Project to output
                output_layer.forward(final_hidden)
            }
            ArchitectureType::Tcn { layers, pooling, output_layer } => {
                // Process through TCN layers
                let mut current_output = input.clone();

                for layer in layers {
                    current_output = layer.forward(&current_output)?;
                }

                // Apply pooling to get fixed-size representation
                let pooled = Self::apply_pooling(&current_output, *pooling);

                // Project to output
                output_layer.forward(&pooled)
            }
        }
    }

    fn parameters(&self) -> &Self::Params {
        &self.params
    }

    fn parameters_mut(&mut self) -> &mut Self::Params {
        &mut self.params
    }

    fn load_parameters(&mut self, params: Self::Params) -> Result<()> {
        if params.values.len() != self.params.values.len() {
            return Err(TemporalNeuralError::ModelError {
                component: "SystemA".to_string(),
                message: format!(
                    "Parameter count mismatch: expected {}, got {}",
                    self.params.values.len(),
                    params.values.len()
                ),
                context: vec![],
            });
        }

        self.params = params;
        Ok(())
    }

    fn parameter_count(&self) -> usize {
        self.params.parameter_count()
    }

    fn memory_usage(&self) -> usize {
        self.params.memory_usage()
    }

    fn input_shape(&self) -> (usize, usize) {
        // Returns (sequence_length, features)
        // This will be set based on configuration in a real implementation
        (256, 4) // Default for 128ms window at 2kHz with 4 features
    }

    fn output_dim(&self) -> usize {
        2 // Predicting 2D position (x, y)
    }

    fn model_name(&self) -> &'static str {
        "SystemA"
    }

    fn config(&self) -> &ModelConfig {
        &self.config
    }

    fn prepare_for_inference(&mut self) -> Result<()> {
        // For System A, we might apply quantization here
        // For now, just mark as ready
        Ok(())
    }
}

impl ModelParams for SystemAParams {
    fn initialize(config: &ModelConfig, rng: &mut impl rand::Rng) -> Self {
        // Calculate total parameter count based on architecture
        let total_params = Self::calculate_param_count(config);

        // Initialize with small random values
        let values: Vec<f64> = (0..total_params)
            .map(|_| (rng.sample(rand::distributions::Standard) as f64 - 0.5) * 0.02)
            .collect();

        let gradients = vec![0.0; total_params];

        let structure = Self::build_structure(config);

        Self {
            values,
            gradients,
            structure,
        }
    }

    fn parameter_count(&self) -> usize {
        self.values.len()
    }

    fn apply_l2_regularization(&mut self, weight_decay: f64) {
        for (param, grad) in self.values.iter().zip(self.gradients.iter_mut()) {
            *grad += weight_decay * param;
        }
    }

    fn clip_gradients(&mut self, max_norm: f64) {
        let grad_norm: f64 = self.gradients.iter().map(|g| g * g).sum::<f64>().sqrt();

        if grad_norm > max_norm {
            let scale = max_norm / grad_norm;
            for grad in self.gradients.iter_mut() {
                *grad *= scale;
            }
        }
    }

    fn zero_gradients(&mut self) {
        self.gradients.fill(0.0);
    }

    fn update_parameters(&mut self, learning_rate: f64) {
        for (param, grad) in self.values.iter_mut().zip(self.gradients.iter()) {
            *param -= learning_rate * grad;
        }
    }

    fn parameter_stats(&self) -> ParameterStats {
        let n = self.values.len() as f64;
        let mean = self.values.iter().sum::<f64>() / n;
        let variance = self.values.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / n;

        let mean_abs_value = self.values.iter().map(|x| x.abs()).sum::<f64>() / n;
        let min_value = self.values.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_value = self.values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));

        let mean_abs_gradient = if !self.gradients.is_empty() {
            Some(self.gradients.iter().map(|g| g.abs()).sum::<f64>() / self.gradients.len() as f64)
        } else {
            None
        };

        let gradient_norm = if !self.gradients.is_empty() {
            Some(self.gradients.iter().map(|g| g * g).sum::<f64>().sqrt())
        } else {
            None
        };

        ParameterStats {
            mean_abs_value,
            std_dev: variance.sqrt(),
            min_value,
            max_value,
            mean_abs_gradient,
            gradient_norm,
        }
    }
}

impl SystemAParams {
    fn calculate_param_count(config: &ModelConfig) -> usize {
        match config.model_type.as_str() {
            "micro_gru" => {
                // GRU parameters: 3 * (input_size * hidden + hidden * hidden + hidden) per layer
                // Plus output layer: hidden * output + output
                let hidden = config.hidden_size as usize;
                let input_size = hidden; // Simplified assumption
                let gru_params_per_layer = 3 * (input_size * hidden + hidden * hidden + hidden);
                let gru_params = gru_params_per_layer * config.num_layers as usize;
                let output_params = hidden * 2 + 2; // 2D output
                gru_params + output_params
            }
            "micro_tcn" => {
                // TCN parameters: kernel_size * input_channels * output_channels + output_channels per layer
                // Plus output layer
                let channels = config.hidden_size as usize;
                let kernel_size = 3;
                let tcn_params_per_layer = kernel_size * channels * channels + channels;
                let tcn_params = tcn_params_per_layer * config.num_layers as usize;
                let output_params = channels * 2 + 2; // 2D output
                tcn_params + output_params
            }
            _ => 1000, // Default fallback
        }
    }

    fn build_structure(config: &ModelConfig) -> ParameterStructure {
        let mut layout = Vec::new();
        let mut offset = 0;

        match config.model_type.as_str() {
            "micro_gru" => {
                for i in 0..config.num_layers {
                    let layer_params = Self::gru_layer_param_count(config.hidden_size as usize);
                    layout.push((format!("gru_layer_{}", i), offset, layer_params));
                    offset += layer_params;
                }
            }
            "micro_tcn" => {
                for i in 0..config.num_layers {
                    let layer_params = Self::tcn_layer_param_count(config.hidden_size as usize);
                    layout.push((format!("tcn_layer_{}", i), offset, layer_params));
                    offset += layer_params;
                }
            }
            _ => {}
        }

        // Output layer
        let output_params = config.hidden_size as usize * 2 + 2;
        layout.push(("output_layer".to_string(), offset, output_params));

        ParameterStructure {
            total_params: Self::calculate_param_count(config),
            layout,
        }
    }

    fn gru_layer_param_count(hidden_size: usize) -> usize {
        3 * (hidden_size * hidden_size + hidden_size * hidden_size + hidden_size)
    }

    fn tcn_layer_param_count(channels: usize) -> usize {
        3 * channels * channels + channels // kernel_size=3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_a_creation() {
        let mut config = ModelConfig {
            model_type: "micro_gru".to_string(),
            hidden_size: 16,
            num_layers: 2,
            dropout: 0.1,
            residual: true,
            activation: "tanh".to_string(),
            layer_norm: false,
        };

        let system = SystemA::new(&config).unwrap();
        assert_eq!(system.model_name(), "SystemA");
        assert_eq!(system.output_dim(), 2);

        // Test TCN variant
        config.model_type = "micro_tcn".to_string();
        let system_tcn = SystemA::new(&config).unwrap();
        assert_eq!(system_tcn.model_name(), "SystemA");
    }

    #[test]
    fn test_forward_pass() {
        let config = ModelConfig {
            model_type: "micro_gru".to_string(),
            hidden_size: 8,
            num_layers: 1,
            dropout: 0.0,
            residual: false,
            activation: "tanh".to_string(),
            layer_norm: false,
        };

        let system = SystemA::new(&config).unwrap();
        let input = DMatrix::from_element(4, 10, 1.0); // 4 features, 10 timesteps

        let output = system.forward(&input).unwrap();
        assert_eq!(output.len(), 2);
    }

    #[test]
    fn test_parameter_operations() {
        let config = ModelConfig {
            model_type: "micro_gru".to_string(),
            hidden_size: 4,
            num_layers: 1,
            dropout: 0.0,
            residual: false,
            activation: "tanh".to_string(),
            layer_norm: false,
        };

        let system = SystemA::new(&config).unwrap();
        let param_count = system.parameter_count();
        assert!(param_count > 0);

        let stats = system.parameters().parameter_stats();
        assert!(stats.mean_abs_value >= 0.0);
    }
}