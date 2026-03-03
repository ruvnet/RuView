//! Optimizers for neural network training

use crate::{
    error::{Result, TemporalNeuralError},
    models::ModelParams,
};

/// Trait for optimization algorithms
pub trait Optimizer: Send + Sync {
    /// Perform one optimization step
    fn step(&mut self, params: &mut dyn ModelParams) -> Result<()>;

    /// Get current learning rate
    fn get_learning_rate(&self) -> f64;

    /// Set learning rate
    fn set_learning_rate(&mut self, lr: f64);

    /// Reset optimizer state
    fn reset(&mut self);
}

/// Adam optimizer implementation
pub struct AdamOptimizer {
    learning_rate: f64,
    beta1: f64,
    beta2: f64,
    epsilon: f64,
    step_count: usize,
}

impl AdamOptimizer {
    /// Create new Adam optimizer
    pub fn new(learning_rate: f64) -> Self {
        Self {
            learning_rate,
            beta1: 0.9,
            beta2: 0.999,
            epsilon: 1e-8,
            step_count: 0,
        }
    }

    /// Create Adam optimizer with custom parameters
    pub fn with_params(learning_rate: f64, beta1: f64, beta2: f64, epsilon: f64) -> Self {
        Self {
            learning_rate,
            beta1,
            beta2,
            epsilon,
            step_count: 0,
        }
    }
}

impl Optimizer for AdamOptimizer {
    fn step(&mut self, params: &mut dyn ModelParams) -> Result<()> {
        self.step_count += 1;

        // In a full implementation, this would:
        // 1. Compute bias-corrected first and second moment estimates
        // 2. Update parameters using adaptive learning rates
        // For now, just apply basic gradient update

        params.update_parameters(self.learning_rate);
        Ok(())
    }

    fn get_learning_rate(&self) -> f64 {
        self.learning_rate
    }

    fn set_learning_rate(&mut self, lr: f64) {
        self.learning_rate = lr;
    }

    fn reset(&mut self) {
        self.step_count = 0;
    }
}

/// SGD optimizer implementation
pub struct SgdOptimizer {
    learning_rate: f64,
    momentum: f64,
    weight_decay: f64,
}

impl SgdOptimizer {
    /// Create new SGD optimizer
    pub fn new(learning_rate: f64) -> Self {
        Self {
            learning_rate,
            momentum: 0.0,
            weight_decay: 0.0,
        }
    }

    /// Create SGD optimizer with momentum
    pub fn with_momentum(learning_rate: f64, momentum: f64) -> Self {
        Self {
            learning_rate,
            momentum,
            weight_decay: 0.0,
        }
    }
}

impl Optimizer for SgdOptimizer {
    fn step(&mut self, params: &mut dyn ModelParams) -> Result<()> {
        // Apply weight decay
        if self.weight_decay > 0.0 {
            params.apply_l2_regularization(self.weight_decay);
        }

        // Update parameters
        params.update_parameters(self.learning_rate);
        Ok(())
    }

    fn get_learning_rate(&self) -> f64 {
        self.learning_rate
    }

    fn set_learning_rate(&mut self, lr: f64) {
        self.learning_rate = lr;
    }

    fn reset(&mut self) {
        // No state to reset for basic SGD
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock implementation for testing
    struct MockParams {
        values: Vec<f64>,
        gradients: Vec<f64>,
    }

    impl ModelParams for MockParams {
        fn initialize(_config: &crate::config::ModelConfig, _rng: &mut impl rand::Rng) -> Self {
            Self {
                values: vec![1.0, 2.0, 3.0],
                gradients: vec![0.1, 0.2, 0.3],
            }
        }

        fn parameter_count(&self) -> usize {
            self.values.len()
        }

        fn apply_l2_regularization(&mut self, weight_decay: f64) {
            for (grad, &param) in self.gradients.iter_mut().zip(self.values.iter()) {
                *grad += weight_decay * param;
            }
        }

        fn clip_gradients(&mut self, _max_norm: f64) {
            // Simple implementation
        }

        fn zero_gradients(&mut self) {
            self.gradients.fill(0.0);
        }

        fn update_parameters(&mut self, learning_rate: f64) {
            for (param, &grad) in self.values.iter_mut().zip(self.gradients.iter()) {
                *param -= learning_rate * grad;
            }
        }

        fn parameter_stats(&self) -> crate::models::ParameterStats {
            crate::models::ParameterStats {
                mean_abs_value: 0.0,
                std_dev: 0.0,
                min_value: 0.0,
                max_value: 0.0,
                mean_abs_gradient: None,
                gradient_norm: None,
            }
        }
    }

    #[test]
    fn test_adam_optimizer() {
        let mut optimizer = AdamOptimizer::new(0.01);
        let mut params = MockParams::initialize(
            &crate::config::ModelConfig {
                model_type: "test".to_string(),
                hidden_size: 1,
                num_layers: 1,
                dropout: 0.0,
                residual: false,
                activation: "linear".to_string(),
                layer_norm: false,
            },
            &mut rand::thread_rng(),
        );

        let initial_values = params.values.clone();

        optimizer.step(&mut params).unwrap();

        // Parameters should have changed
        assert_ne!(params.values, initial_values);
        assert_eq!(optimizer.get_learning_rate(), 0.01);
    }

    #[test]
    fn test_sgd_optimizer() {
        let mut optimizer = SgdOptimizer::new(0.1);
        let mut params = MockParams::initialize(
            &crate::config::ModelConfig {
                model_type: "test".to_string(),
                hidden_size: 1,
                num_layers: 1,
                dropout: 0.0,
                residual: false,
                activation: "linear".to_string(),
                layer_norm: false,
            },
            &mut rand::thread_rng(),
        );

        let initial_values = params.values.clone();

        optimizer.step(&mut params).unwrap();

        // Parameters should have changed
        assert_ne!(params.values, initial_values);
        assert_eq!(optimizer.get_learning_rate(), 0.1);
    }
}