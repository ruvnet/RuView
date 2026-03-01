//! Configuration types for learned restriction maps.

use serde::{Deserialize, Serialize};

/// Configuration for a learned restriction map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestrictionMapConfig {
    /// Input dimension (source node state dimension).
    pub input_dim: usize,
    /// Output dimension (shared space dimension).
    pub output_dim: usize,
    /// Hidden dimension for the neural network.
    pub hidden_dim: usize,
    /// Number of hidden layers.
    pub num_layers: usize,
    /// Activation function.
    pub activation: Activation,
    /// Optimizer configuration.
    pub optimizer: OptimizerConfig,
    /// Learning rate scheduler configuration.
    pub scheduler: SchedulerConfig,
    /// EWC lambda for weight consolidation.
    pub ewc_lambda: f32,
    /// Replay buffer capacity.
    pub replay_capacity: usize,
    /// Batch size for training.
    pub batch_size: usize,
    /// Dropout rate (0 = no dropout).
    pub dropout: f32,
    /// L2 regularization weight.
    pub weight_decay: f32,
}

impl Default for RestrictionMapConfig {
    fn default() -> Self {
        Self {
            input_dim: 128,
            output_dim: 64,
            hidden_dim: 256,
            num_layers: 2,
            activation: Activation::ReLU,
            optimizer: OptimizerConfig::default(),
            scheduler: SchedulerConfig::default(),
            ewc_lambda: 0.4,
            replay_capacity: 10000,
            batch_size: 32,
            dropout: 0.1,
            weight_decay: 1e-5,
        }
    }
}

impl RestrictionMapConfig {
    /// Create a small configuration for testing.
    pub fn small() -> Self {
        Self {
            input_dim: 32,
            output_dim: 16,
            hidden_dim: 64,
            num_layers: 1,
            activation: Activation::ReLU,
            optimizer: OptimizerConfig::sgd(0.01),
            scheduler: SchedulerConfig::none(),
            ewc_lambda: 0.2,
            replay_capacity: 1000,
            batch_size: 8,
            dropout: 0.0,
            weight_decay: 0.0,
        }
    }

    /// Create a large configuration for production.
    pub fn large() -> Self {
        Self {
            input_dim: 512,
            output_dim: 256,
            hidden_dim: 1024,
            num_layers: 4,
            activation: Activation::GELU,
            optimizer: OptimizerConfig::adamw(1e-4),
            scheduler: SchedulerConfig::cosine_annealing(1000, 1e-6),
            ewc_lambda: 0.5,
            replay_capacity: 100000,
            batch_size: 64,
            dropout: 0.2,
            weight_decay: 1e-4,
        }
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), String> {
        if self.input_dim == 0 {
            return Err("input_dim must be > 0".into());
        }
        if self.output_dim == 0 {
            return Err("output_dim must be > 0".into());
        }
        if self.hidden_dim == 0 {
            return Err("hidden_dim must be > 0".into());
        }
        if self.batch_size == 0 {
            return Err("batch_size must be > 0".into());
        }
        if self.dropout < 0.0 || self.dropout >= 1.0 {
            return Err("dropout must be in [0, 1)".into());
        }
        Ok(())
    }
}

/// Activation function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Activation {
    /// Rectified Linear Unit.
    ReLU,
    /// Leaky ReLU.
    LeakyReLU,
    /// GELU (Gaussian Error Linear Unit).
    GELU,
    /// Tanh.
    Tanh,
    /// Sigmoid.
    Sigmoid,
    /// No activation (identity).
    None,
}

impl Activation {
    /// Apply the activation function.
    pub fn apply(&self, x: f32) -> f32 {
        match self {
            Self::ReLU => x.max(0.0),
            Self::LeakyReLU => {
                if x > 0.0 {
                    x
                } else {
                    0.01 * x
                }
            }
            Self::GELU => {
                // Approximation: 0.5 * x * (1 + tanh(sqrt(2/pi) * (x + 0.044715 * x^3)))
                0.5 * x * (1.0 + ((0.7978845608 * (x + 0.044715 * x.powi(3))).tanh()))
            }
            Self::Tanh => x.tanh(),
            Self::Sigmoid => 1.0 / (1.0 + (-x).exp()),
            Self::None => x,
        }
    }

    /// Apply the derivative of the activation function.
    pub fn derivative(&self, x: f32) -> f32 {
        match self {
            Self::ReLU => {
                if x > 0.0 {
                    1.0
                } else {
                    0.0
                }
            }
            Self::LeakyReLU => {
                if x > 0.0 {
                    1.0
                } else {
                    0.01
                }
            }
            Self::GELU => {
                // Approximation of GELU derivative
                let t = (0.7978845608 * (x + 0.044715 * x.powi(3))).tanh();
                0.5 * (1.0 + t)
                    + 0.5 * x * (1.0 - t * t) * 0.7978845608 * (1.0 + 3.0 * 0.044715 * x * x)
            }
            Self::Tanh => 1.0 - x.tanh().powi(2),
            Self::Sigmoid => {
                let s = 1.0 / (1.0 + (-x).exp());
                s * (1.0 - s)
            }
            Self::None => 1.0,
        }
    }
}

/// Optimizer configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizerConfig {
    /// Optimizer type.
    pub optimizer_type: OptimizerType,
    /// Learning rate.
    pub learning_rate: f32,
    /// Gradient clipping (0 = no clipping).
    pub gradient_clip: f32,
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        Self::adam(1e-3)
    }
}

impl OptimizerConfig {
    /// Create SGD optimizer configuration.
    pub fn sgd(learning_rate: f32) -> Self {
        Self {
            optimizer_type: OptimizerType::SGD { momentum: 0.0 },
            learning_rate,
            gradient_clip: 1.0,
        }
    }

    /// Create SGD with momentum.
    pub fn sgd_momentum(learning_rate: f32, momentum: f32) -> Self {
        Self {
            optimizer_type: OptimizerType::SGD { momentum },
            learning_rate,
            gradient_clip: 1.0,
        }
    }

    /// Create Adam optimizer configuration.
    pub fn adam(learning_rate: f32) -> Self {
        Self {
            optimizer_type: OptimizerType::Adam {
                beta1: 0.9,
                beta2: 0.999,
                epsilon: 1e-8,
            },
            learning_rate,
            gradient_clip: 1.0,
        }
    }

    /// Create AdamW optimizer configuration.
    pub fn adamw(learning_rate: f32) -> Self {
        Self {
            optimizer_type: OptimizerType::AdamW {
                beta1: 0.9,
                beta2: 0.999,
                epsilon: 1e-8,
            },
            learning_rate,
            gradient_clip: 1.0,
        }
    }
}

/// Optimizer type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OptimizerType {
    /// Stochastic Gradient Descent.
    SGD {
        /// Momentum factor.
        momentum: f32,
    },
    /// Adam optimizer.
    Adam {
        /// First moment decay.
        beta1: f32,
        /// Second moment decay.
        beta2: f32,
        /// Numerical stability epsilon.
        epsilon: f32,
    },
    /// AdamW optimizer (decoupled weight decay).
    AdamW {
        /// First moment decay.
        beta1: f32,
        /// Second moment decay.
        beta2: f32,
        /// Numerical stability epsilon.
        epsilon: f32,
    },
}

/// Learning rate scheduler configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerConfig {
    /// Scheduler type.
    pub scheduler_type: SchedulerType,
    /// Initial learning rate.
    pub initial_lr: f32,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self::cosine_annealing(1000, 1e-6)
    }
}

impl SchedulerConfig {
    /// No scheduler (constant learning rate).
    pub fn none() -> Self {
        Self {
            scheduler_type: SchedulerType::None,
            initial_lr: 1e-3,
        }
    }

    /// Step decay scheduler.
    pub fn step(step_size: usize, gamma: f32) -> Self {
        Self {
            scheduler_type: SchedulerType::Step { step_size, gamma },
            initial_lr: 1e-3,
        }
    }

    /// Cosine annealing scheduler.
    pub fn cosine_annealing(t_max: usize, eta_min: f32) -> Self {
        Self {
            scheduler_type: SchedulerType::CosineAnnealing { t_max, eta_min },
            initial_lr: 1e-3,
        }
    }

    /// Get learning rate at a given step.
    pub fn get_lr(&self, step: usize) -> f32 {
        match &self.scheduler_type {
            SchedulerType::None => self.initial_lr,
            SchedulerType::Step { step_size, gamma } => {
                let decays = step / step_size;
                self.initial_lr * gamma.powi(decays as i32)
            }
            SchedulerType::CosineAnnealing { t_max, eta_min } => {
                let t = (step % t_max) as f32;
                let t_max = *t_max as f32;
                *eta_min
                    + (self.initial_lr - eta_min) * (1.0 + (std::f32::consts::PI * t / t_max).cos())
                        / 2.0
            }
        }
    }
}

/// Scheduler type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SchedulerType {
    /// No scheduling.
    None,
    /// Step decay.
    Step {
        /// Steps between decays.
        step_size: usize,
        /// Decay factor.
        gamma: f32,
    },
    /// Cosine annealing.
    CosineAnnealing {
        /// Period of cosine.
        t_max: usize,
        /// Minimum learning rate.
        eta_min: f32,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let config = RestrictionMapConfig::default();
        assert!(config.validate().is_ok());

        let invalid = RestrictionMapConfig {
            input_dim: 0,
            ..Default::default()
        };
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_activation_functions() {
        assert_eq!(Activation::ReLU.apply(-1.0), 0.0);
        assert_eq!(Activation::ReLU.apply(1.0), 1.0);

        assert!((Activation::Sigmoid.apply(0.0) - 0.5).abs() < 0.01);
        assert!((Activation::Tanh.apply(0.0)).abs() < 0.01);
    }

    #[test]
    fn test_scheduler() {
        let scheduler = SchedulerConfig::cosine_annealing(100, 1e-6);
        let lr0 = scheduler.get_lr(0);
        let lr50 = scheduler.get_lr(50);
        let lr100 = scheduler.get_lr(100);

        assert!(lr50 < lr0);
        assert!((lr0 - lr100).abs() < 0.001); // Should cycle back
    }
}
