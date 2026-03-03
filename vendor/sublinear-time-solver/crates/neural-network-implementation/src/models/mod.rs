//! Neural network model implementations for temporal prediction

use crate::{
    config::{Config, ModelConfig},
    error::{Result, TemporalNeuralError},
};
use nalgebra::{DVector, DMatrix};
use serde::{Deserialize, Serialize};

pub mod system_a;
pub mod system_b;
pub mod layers;
pub mod quantization;

pub use system_a::SystemA;
pub use system_b::SystemB;
pub use layers::{GruLayer, TcnLayer, DenseLayer};
pub use quantization::{QuantizedModel, QuantizationScheme};

/// Common trait for all neural network models
pub trait ModelTrait: Send + Sync {
    /// Model parameter type
    type Params: ModelParams;

    /// Create a new model with the given configuration
    fn new(config: &ModelConfig) -> Result<Self> where Self: Sized;

    /// Forward pass through the network
    fn forward(&self, input: &DMatrix<f64>) -> Result<DVector<f64>>;

    /// Get model parameters for training/serialization
    fn parameters(&self) -> &Self::Params;

    /// Get mutable model parameters for training
    fn parameters_mut(&mut self) -> &mut Self::Params;

    /// Load parameters from another model (for transfer learning)
    fn load_parameters(&mut self, params: Self::Params) -> Result<()>;

    /// Get the number of parameters in the model
    fn parameter_count(&self) -> usize;

    /// Get model memory usage in bytes
    fn memory_usage(&self) -> usize;

    /// Get the expected input shape
    fn input_shape(&self) -> (usize, usize);

    /// Get the output dimension
    fn output_dim(&self) -> usize;

    /// Model name for identification
    fn model_name(&self) -> &'static str;

    /// Prepare model for inference (e.g., quantization)
    fn prepare_for_inference(&mut self) -> Result<()> {
        Ok(()) // Default implementation does nothing
    }

    /// Check if model is ready for inference
    fn is_inference_ready(&self) -> bool {
        true // Default implementation always ready
    }

    /// Get model configuration
    fn config(&self) -> &ModelConfig;

    /// Validate input dimensions
    fn validate_input(&self, input: &DMatrix<f64>) -> Result<()> {
        let expected_shape = self.input_shape();
        let actual_shape = (input.nrows(), input.ncols());

        if actual_shape != expected_shape {
            return Err(TemporalNeuralError::ModelError {
                component: self.model_name().to_string(),
                message: format!(
                    "Input shape mismatch: expected {:?}, got {:?}",
                    expected_shape, actual_shape
                ),
                context: vec![
                    ("expected_rows".to_string(), expected_shape.0.to_string()),
                    ("expected_cols".to_string(), expected_shape.1.to_string()),
                    ("actual_rows".to_string(), actual_shape.0.to_string()),
                    ("actual_cols".to_string(), actual_shape.1.to_string()),
                ],
            });
        }

        Ok(())
    }
}

/// Trait for model parameters
pub trait ModelParams: Clone + Send + Sync + Serialize + for<'de> Deserialize<'de> {
    /// Initialize parameters with the given configuration
    fn initialize(config: &ModelConfig, rng: &mut impl rand::Rng) -> Self;

    /// Get the number of parameters
    fn parameter_count(&self) -> usize;

    /// Get parameter memory usage in bytes
    fn memory_usage(&self) -> usize {
        self.parameter_count() * std::mem::size_of::<f64>()
    }

    /// Apply L2 regularization to parameters
    fn apply_l2_regularization(&mut self, weight_decay: f64);

    /// Clip gradients to prevent explosion
    fn clip_gradients(&mut self, max_norm: f64);

    /// Zero out gradients
    fn zero_gradients(&mut self);

    /// Update parameters using gradients
    fn update_parameters(&mut self, learning_rate: f64);

    /// Get parameter statistics for monitoring
    fn parameter_stats(&self) -> ParameterStats;
}

/// Statistics about model parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterStats {
    /// Mean absolute value of parameters
    pub mean_abs_value: f64,
    /// Standard deviation of parameters
    pub std_dev: f64,
    /// Minimum parameter value
    pub min_value: f64,
    /// Maximum parameter value
    pub max_value: f64,
    /// Mean absolute gradient (if available)
    pub mean_abs_gradient: Option<f64>,
    /// Gradient norm (if available)
    pub gradient_norm: Option<f64>,
}

/// Model metadata for serialization and identification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    /// Model name and version
    pub name: String,
    /// Configuration used to create the model
    pub config: ModelConfig,
    /// Training information
    pub training_info: Option<TrainingInfo>,
    /// Performance metrics
    pub performance_metrics: Option<PerformanceMetrics>,
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last modified timestamp
    pub modified_at: chrono::DateTime<chrono::Utc>,
}

/// Training information for model provenance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingInfo {
    /// Number of training epochs completed
    pub epochs_trained: u32,
    /// Training dataset size
    pub training_samples: usize,
    /// Validation dataset size
    pub validation_samples: usize,
    /// Final training loss
    pub final_train_loss: f64,
    /// Final validation loss
    pub final_val_loss: f64,
    /// Training time in seconds
    pub training_time_sec: f64,
    /// Optimizer used
    pub optimizer: String,
    /// Learning rate schedule
    pub learning_rate_schedule: Vec<(u32, f64)>,
}

/// Performance metrics for model evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Mean squared error on test set
    pub mse: f64,
    /// Mean absolute error on test set
    pub mae: f64,
    /// P90 absolute error
    pub p90_error: f64,
    /// P99 absolute error
    pub p99_error: f64,
    /// Average inference latency in milliseconds
    pub avg_latency_ms: f64,
    /// P50 inference latency in milliseconds
    pub p50_latency_ms: f64,
    /// P99.9 inference latency in milliseconds
    pub p99_9_latency_ms: f64,
    /// Model memory usage in bytes
    pub memory_usage_bytes: usize,
    /// Throughput in predictions per second
    pub throughput_pred_per_sec: f64,
}

/// Factory function to create models from configuration
pub fn create_model(config: &Config) -> Result<Box<dyn ModelTrait<Params = Box<dyn ModelParams>>>> {
    match config.system {
        crate::config::SystemConfig::Traditional(_) => {
            let system_a = SystemA::new(&config.model)?;
            Ok(Box::new(system_a) as Box<dyn ModelTrait<Params = Box<dyn ModelParams>>>)
        }
        crate::config::SystemConfig::TemporalSolver(_) => {
            let system_b = SystemB::new(&config.model)?;
            Ok(Box::new(system_b) as Box<dyn ModelTrait<Params = Box<dyn ModelParams>>>)
        }
    }
}

/// Load a model from file
pub fn load_model(path: &std::path::Path) -> Result<(Box<dyn ModelTrait<Params = Box<dyn ModelParams>>>, ModelMetadata)> {
    let content = std::fs::read_to_string(path)?;
    let data: serde_json::Value = serde_json::from_str(&content)?;

    let metadata: ModelMetadata = serde_json::from_value(data["metadata"].clone())?;

    let model = match metadata.name.as_str() {
        "SystemA" => {
            let system_a = SystemA::new(&metadata.config)?;
            Box::new(system_a) as Box<dyn ModelTrait<Params = Box<dyn ModelParams>>>
        }
        "SystemB" => {
            let system_b = SystemB::new(&metadata.config)?;
            Box::new(system_b) as Box<dyn ModelTrait<Params = Box<dyn ModelParams>>>
        }
        _ => {
            return Err(TemporalNeuralError::ModelError {
                component: "model_loader".to_string(),
                message: format!("Unknown model type: {}", metadata.name),
                context: vec![],
            });
        }
    };

    Ok((model, metadata))
}

/// Save a model to file
pub fn save_model(
    model: &dyn ModelTrait<Params = Box<dyn ModelParams>>,
    metadata: &ModelMetadata,
    path: &std::path::Path,
) -> Result<()> {
    let data = serde_json::json!({
        "metadata": metadata,
        "parameters": model.parameters(),
    });

    let content = serde_json::to_string_pretty(&data)?;
    std::fs::write(path, content)?;

    Ok(())
}

/// Model comparison utilities
pub mod comparison {
    use super::*;

    /// Compare two models and return similarity metrics
    pub fn compare_models(
        model1: &dyn ModelTrait<Params = Box<dyn ModelParams>>,
        model2: &dyn ModelTrait<Params = Box<dyn ModelParams>>,
    ) -> ModelComparison {
        let params1 = model1.parameters();
        let params2 = model2.parameters();

        ModelComparison {
            parameter_count_diff: (model1.parameter_count() as i64 - model2.parameter_count() as i64).abs() as usize,
            memory_usage_diff: (model1.memory_usage() as i64 - model2.memory_usage() as i64).abs() as usize,
            architecture_match: model1.model_name() == model2.model_name(),
            config_match: model1.config() == model2.config(),
        }
    }

    /// Result of model comparison
    #[derive(Debug, Clone)]
    pub struct ModelComparison {
        /// Difference in parameter count
        pub parameter_count_diff: usize,
        /// Difference in memory usage
        pub memory_usage_diff: usize,
        /// Whether architectures match
        pub architecture_match: bool,
        /// Whether configurations match
        pub config_match: bool,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn test_model_creation() {
        let config = Config::default();
        let model = create_model(&config);
        assert!(model.is_ok());
    }

    #[test]
    fn test_model_validation() {
        let config = Config::default();
        let model = create_model(&config).unwrap();

        let input = DMatrix::zeros(10, 4); // Wrong size
        assert!(model.validate_input(&input).is_err());

        let correct_input = DMatrix::zeros(256, 4); // Correct size
        assert!(model.validate_input(&correct_input).is_ok());
    }

    #[test]
    fn test_parameter_stats() {
        // This would be implemented once we have concrete parameter types
        // For now, just test that the trait compiles
    }
}