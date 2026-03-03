//! Configuration management for temporal neural network systems

use crate::error::{Result, TemporalNeuralError};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Main configuration structure containing all system settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Common settings shared between systems
    pub common: CommonConfig,
    /// Model architecture configuration
    pub model: ModelConfig,
    /// Training configuration
    pub training: TrainingConfig,
    /// Inference configuration
    pub inference: InferenceConfig,
    /// System-specific configuration
    pub system: SystemConfig,
}

/// Common configuration shared between System A and B
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommonConfig {
    /// Prediction horizon in milliseconds
    pub horizon_ms: u32,
    /// Input window size in milliseconds
    pub window_ms: u32,
    /// Sample rate in Hz
    pub sample_rate_hz: u32,
    /// Feature names and order
    pub features: Vec<String>,
    /// Whether to use INT8 quantization for inference
    pub quantize: bool,
    /// Random seed for reproducibility
    pub random_seed: Option<u64>,
    /// Enable detailed logging
    pub verbose: bool,
}

/// Model architecture configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Model type: "micro_gru" or "micro_tcn"
    pub model_type: String,
    /// Hidden layer size
    pub hidden_size: u32,
    /// Number of layers
    pub num_layers: u32,
    /// Dropout rate during training
    pub dropout: f64,
    /// Whether to use residual connections
    pub residual: bool,
    /// Activation function: "relu", "tanh", "gelu"
    pub activation: String,
    /// Layer normalization
    pub layer_norm: bool,
}

/// Training configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingConfig {
    /// Optimizer type: "adam", "sgd", "rmsprop"
    pub optimizer: String,
    /// Learning rate
    pub learning_rate: f64,
    /// Batch size
    pub batch_size: u32,
    /// Number of training epochs
    pub epochs: u32,
    /// Early stopping patience
    pub patience: u32,
    /// Validation frequency (epochs)
    pub val_frequency: u32,
    /// Gradient clipping threshold
    pub grad_clip: Option<f64>,
    /// Weight decay / L2 regularization
    pub weight_decay: f64,
    /// Smoothness penalty weight for velocity
    pub smoothness_weight: f64,
    /// Checkpoint saving frequency
    pub checkpoint_frequency: u32,
}

/// Inference configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceConfig {
    /// Target P99.9 latency in milliseconds
    pub target_latency_ms: f64,
    /// Enable SIMD optimizations
    pub enable_simd: bool,
    /// Number of inference threads
    pub num_threads: u32,
    /// Memory pinning for performance
    pub pin_memory: bool,
    /// CPU affinity settings
    pub cpu_affinity: Option<Vec<u32>>,
    /// Batch size for inference (usually 1 for real-time)
    pub batch_size: u32,
}

/// System-specific configuration (A or B)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SystemConfig {
    /// Traditional micro-net (System A)
    Traditional(TraditionalConfig),
    /// Temporal solver net (System B)
    TemporalSolver(TemporalSolverConfig),
}

/// Configuration for traditional system (System A)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraditionalConfig {
    /// Whether traditional system is enabled
    pub enabled: bool,
}

/// Configuration for temporal solver system (System B)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalSolverConfig {
    /// Kalman filter prior configuration
    pub prior: KalmanConfig,
    /// Sublinear solver gate configuration
    pub solver_gate: SolverGateConfig,
    /// Active sample selection configuration
    pub active_selection: ActiveSelectionConfig,
}

/// Kalman filter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KalmanConfig {
    /// Process noise covariance
    pub process_noise: f64,
    /// Measurement noise covariance
    pub measurement_noise: f64,
    /// Initial state uncertainty
    pub initial_uncertainty: f64,
    /// State transition model: "constant_velocity", "constant_acceleration"
    pub transition_model: String,
    /// Update frequency in Hz
    pub update_frequency: f64,
}

/// Sublinear solver gate configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverGateConfig {
    /// Solver algorithm: "neumann", "random_walk", "forward_push"
    pub algorithm: String,
    /// Convergence tolerance
    pub epsilon: f64,
    /// Computational budget
    pub budget: u64,
    /// Maximum certificate error to allow passage
    pub max_cert_error: f64,
    /// Fallback strategy: "hold_last", "kalman_only", "disable_gate"
    pub fallback_strategy: String,
}

/// Active sample selection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveSelectionConfig {
    /// k-NN graph construction parameter
    pub k: u32,
    /// PageRank tolerance for convergence
    pub pagerank_eps: f64,
    /// Number of active samples per epoch
    pub samples_per_epoch: u32,
    /// Error weight for PageRank scoring
    pub error_weight: f64,
    /// Diversity weight to avoid clustering
    pub diversity_weight: f64,
}

impl Config {
    /// Load configuration from YAML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref()).map_err(|e| {
            TemporalNeuralError::IoError {
                message: format!("Failed to read config file: {}", e),
                path: Some(path.as_ref().to_string_lossy().to_string()),
                source: Some(e),
            }
        })?;

        let config: Self = serde_yaml::from_str(&content).map_err(|e| {
            TemporalNeuralError::ConfigurationError {
                message: format!("Failed to parse config: {}", e),
                field: None,
            }
        })?;

        config.validate()?;
        Ok(config)
    }

    /// Save configuration to YAML file
    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = serde_yaml::to_string(self).map_err(|e| {
            TemporalNeuralError::SerializationError {
                message: format!("Failed to serialize config: {}", e),
                format: Some("yaml".to_string()),
            }
        })?;

        std::fs::write(path.as_ref(), content).map_err(|e| {
            TemporalNeuralError::IoError {
                message: format!("Failed to write config file: {}", e),
                path: Some(path.as_ref().to_string_lossy().to_string()),
                source: Some(e),
            }
        })?;

        Ok(())
    }

    /// Validate configuration parameters
    pub fn validate(&self) -> Result<()> {
        // Validate common config
        if self.common.horizon_ms == 0 {
            return Err(TemporalNeuralError::config_field_error(
                "horizon_ms", "Must be greater than 0"
            ));
        }

        if self.common.window_ms == 0 {
            return Err(TemporalNeuralError::config_field_error(
                "window_ms", "Must be greater than 0"
            ));
        }

        if self.common.sample_rate_hz == 0 {
            return Err(TemporalNeuralError::config_field_error(
                "sample_rate_hz", "Must be greater than 0"
            ));
        }

        if self.common.features.is_empty() {
            return Err(TemporalNeuralError::config_field_error(
                "features", "Must specify at least one feature"
            ));
        }

        // Validate model config
        if self.model.hidden_size == 0 {
            return Err(TemporalNeuralError::config_field_error(
                "hidden_size", "Must be greater than 0"
            ));
        }

        if self.model.num_layers == 0 {
            return Err(TemporalNeuralError::config_field_error(
                "num_layers", "Must be greater than 0"
            ));
        }

        if !(0.0..=1.0).contains(&self.model.dropout) {
            return Err(TemporalNeuralError::config_field_error(
                "dropout", "Must be between 0.0 and 1.0"
            ));
        }

        // Validate training config
        if self.training.learning_rate <= 0.0 {
            return Err(TemporalNeuralError::config_field_error(
                "learning_rate", "Must be positive"
            ));
        }

        if self.training.batch_size == 0 {
            return Err(TemporalNeuralError::config_field_error(
                "batch_size", "Must be greater than 0"
            ));
        }

        if self.training.epochs == 0 {
            return Err(TemporalNeuralError::config_field_error(
                "epochs", "Must be greater than 0"
            ));
        }

        // Validate inference config
        if self.inference.target_latency_ms <= 0.0 {
            return Err(TemporalNeuralError::config_field_error(
                "target_latency_ms", "Must be positive"
            ));
        }

        if self.inference.num_threads == 0 {
            return Err(TemporalNeuralError::config_field_error(
                "num_threads", "Must be greater than 0"
            ));
        }

        // Validate system-specific config
        match &self.system {
            SystemConfig::Traditional(_) => {
                // No additional validation needed
            }
            SystemConfig::TemporalSolver(config) => {
                self.validate_temporal_solver_config(config)?;
            }
        }

        Ok(())
    }

    fn validate_temporal_solver_config(&self, config: &TemporalSolverConfig) -> Result<()> {
        // Validate Kalman config
        if config.prior.process_noise <= 0.0 {
            return Err(TemporalNeuralError::config_field_error(
                "process_noise", "Must be positive"
            ));
        }

        if config.prior.measurement_noise <= 0.0 {
            return Err(TemporalNeuralError::config_field_error(
                "measurement_noise", "Must be positive"
            ));
        }

        if config.prior.update_frequency <= 0.0 {
            return Err(TemporalNeuralError::config_field_error(
                "update_frequency", "Must be positive"
            ));
        }

        // Validate solver gate config
        if config.solver_gate.epsilon <= 0.0 {
            return Err(TemporalNeuralError::config_field_error(
                "epsilon", "Must be positive"
            ));
        }

        if config.solver_gate.budget == 0 {
            return Err(TemporalNeuralError::config_field_error(
                "budget", "Must be greater than 0"
            ));
        }

        if config.solver_gate.max_cert_error < 0.0 {
            return Err(TemporalNeuralError::config_field_error(
                "max_cert_error", "Must be non-negative"
            ));
        }

        // Validate active selection config
        if config.active_selection.k == 0 {
            return Err(TemporalNeuralError::config_field_error(
                "k", "Must be greater than 0"
            ));
        }

        if config.active_selection.pagerank_eps <= 0.0 {
            return Err(TemporalNeuralError::config_field_error(
                "pagerank_eps", "Must be positive"
            ));
        }

        Ok(())
    }

    /// Get the expected input window size in samples
    pub fn window_samples(&self) -> usize {
        ((self.common.window_ms as f64 / 1000.0) * self.common.sample_rate_hz as f64) as usize
    }

    /// Get the prediction horizon in samples
    pub fn horizon_samples(&self) -> usize {
        ((self.common.horizon_ms as f64 / 1000.0) * self.common.sample_rate_hz as f64) as usize
    }

    /// Get the feature count
    pub fn feature_count(&self) -> usize {
        self.common.features.len()
    }

    /// Get input shape for the neural network
    pub fn input_shape(&self) -> (usize, usize) {
        (self.window_samples(), self.feature_count())
    }

    /// Check if this is a temporal solver system
    pub fn is_temporal_solver(&self) -> bool {
        matches!(self.system, SystemConfig::TemporalSolver(_))
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            common: CommonConfig {
                horizon_ms: 500,
                window_ms: 128,
                sample_rate_hz: 2000,
                features: vec!["x".to_string(), "y".to_string(), "vx".to_string(), "vy".to_string()],
                quantize: true,
                random_seed: Some(42),
                verbose: false,
            },
            model: ModelConfig {
                model_type: "micro_gru".to_string(),
                hidden_size: 32,
                num_layers: 1,
                dropout: 0.1,
                residual: true,
                activation: "tanh".to_string(),
                layer_norm: false,
            },
            training: TrainingConfig {
                optimizer: "adam".to_string(),
                learning_rate: 1e-3,
                batch_size: 256,
                epochs: 15,
                patience: 5,
                val_frequency: 1,
                grad_clip: Some(1.0),
                weight_decay: 1e-4,
                smoothness_weight: 0.1,
                checkpoint_frequency: 5,
            },
            inference: InferenceConfig {
                target_latency_ms: 0.9,
                enable_simd: true,
                num_threads: 1,
                pin_memory: true,
                cpu_affinity: None,
                batch_size: 1,
            },
            system: SystemConfig::Traditional(TraditionalConfig { enabled: true }),
        }
    }
}

impl Default for TemporalSolverConfig {
    fn default() -> Self {
        Self {
            prior: KalmanConfig {
                process_noise: 0.01,
                measurement_noise: 0.1,
                initial_uncertainty: 1.0,
                transition_model: "constant_velocity".to_string(),
                update_frequency: 2000.0,
            },
            solver_gate: SolverGateConfig {
                algorithm: "neumann".to_string(),
                epsilon: 0.02,
                budget: 200000,
                max_cert_error: 0.02,
                fallback_strategy: "kalman_only".to_string(),
            },
            active_selection: ActiveSelectionConfig {
                k: 15,
                pagerank_eps: 0.03,
                samples_per_epoch: 1000,
                error_weight: 0.8,
                diversity_weight: 0.2,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_config_validation() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let yaml = serde_yaml::to_string(&config).unwrap();
        let deserialized: Config = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(config.common.horizon_ms, deserialized.common.horizon_ms);
    }

    #[test]
    fn test_file_operations() {
        let config = Config::default();
        let temp_file = NamedTempFile::new().unwrap();

        // Test save
        config.to_file(temp_file.path()).unwrap();

        // Test load
        let loaded_config = Config::from_file(temp_file.path()).unwrap();
        assert_eq!(config.common.horizon_ms, loaded_config.common.horizon_ms);
    }

    #[test]
    fn test_validation_errors() {
        let mut config = Config::default();

        // Test invalid horizon
        config.common.horizon_ms = 0;
        assert!(config.validate().is_err());

        config.common.horizon_ms = 500; // Reset

        // Test invalid learning rate
        config.training.learning_rate = -1.0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_helper_methods() {
        let config = Config::default();

        assert_eq!(config.window_samples(), 256); // 128ms at 2kHz
        assert_eq!(config.horizon_samples(), 1000); // 500ms at 2kHz
        assert_eq!(config.feature_count(), 4);
        assert_eq!(config.input_shape(), (256, 4));
        assert!(!config.is_temporal_solver());
    }
}