//! Natural Gradient Descent
//!
//! Update parameters using the natural gradient: F^{-1} * grad
//! where F is the Fisher information matrix.

use super::fisher::{FisherConfig, FisherMetric};
use serde::{Deserialize, Serialize};

/// Natural gradient configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NaturalGradientConfig {
    /// Learning rate
    pub lr: f32,
    /// Fisher metric config
    pub fisher: FisherConfig,
    /// Use diagonal approximation (faster but less accurate)
    pub use_diagonal: bool,
}

impl Default for NaturalGradientConfig {
    fn default() -> Self {
        Self {
            lr: 0.1,
            fisher: FisherConfig::default(),
            use_diagonal: false,
        }
    }
}

/// Natural gradient optimizer
#[derive(Debug, Clone)]
pub struct NaturalGradient {
    config: NaturalGradientConfig,
    fisher: FisherMetric,
}

impl NaturalGradient {
    /// Create new natural gradient optimizer
    pub fn new(config: NaturalGradientConfig) -> Self {
        let fisher = FisherMetric::new(config.fisher.clone());
        Self { config, fisher }
    }

    /// Compute natural gradient step for logits
    /// Returns updated logits
    pub fn step_logits(&self, logits: &[f32], grad_logits: &[f32]) -> Vec<f32> {
        let probs = Self::softmax(logits);

        // Compute natural gradient direction
        let nat_grad = if self.config.use_diagonal {
            self.fisher.apply_inverse_approx(&probs, grad_logits)
        } else {
            self.fisher.solve_cg(&probs, grad_logits)
        };

        // Update logits
        let mut new_logits = logits.to_vec();
        for i in 0..new_logits.len() {
            new_logits[i] -= self.config.lr * nat_grad[i];
        }

        new_logits
    }

    /// Compute natural gradient step for general parameters with diagonal Fisher
    /// Fisher diag should be pre-computed from data
    pub fn step_diagonal(&self, params: &[f32], grads: &[f32], fisher_diag: &[f32]) -> Vec<f32> {
        let n = params.len();
        let mut new_params = params.to_vec();
        let eps = self.config.fisher.eps;

        for i in 0..n {
            let f_inv = 1.0 / (fisher_diag[i].abs() + eps);
            new_params[i] -= self.config.lr * grads[i] * f_inv;
        }

        new_params
    }

    /// Compute natural gradient for attention logits
    /// Uses the Fisher metric on the output probability distribution
    pub fn step_attention_logits(&self, logits: &[f32], grad_logits: &[f32]) -> Vec<f32> {
        self.step_logits(logits, grad_logits)
    }

    /// Stable softmax
    fn softmax(logits: &[f32]) -> Vec<f32> {
        if logits.is_empty() {
            return vec![];
        }

        let max_logit = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_logits: Vec<f32> = logits.iter().map(|&l| (l - max_logit).exp()).collect();
        let sum: f32 = exp_logits.iter().sum();

        if sum > 0.0 {
            exp_logits.iter().map(|&e| e / sum).collect()
        } else {
            vec![1.0 / logits.len() as f32; logits.len()]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_natural_gradient_step() {
        let config = NaturalGradientConfig {
            lr: 0.1,
            ..Default::default()
        };
        let ng = NaturalGradient::new(config);

        let logits = vec![1.0, 2.0, 0.5, 0.5];
        let grads = vec![0.1, -0.1, 0.05, -0.05];

        let new_logits = ng.step_logits(&logits, &grads);

        assert_eq!(new_logits.len(), 4);
        // Should be different from original
        assert!(
            (new_logits[0] - logits[0]).abs() > 1e-6 || (new_logits[1] - logits[1]).abs() > 1e-6
        );
    }

    #[test]
    fn test_diagonal_step() {
        let ng = NaturalGradient::new(NaturalGradientConfig::default());

        let params = vec![1.0, 2.0, 3.0];
        let grads = vec![0.1, 0.1, 0.1]; // Equal gradients
        let fisher_diag = vec![1.0, 2.0, 0.5]; // Different Fisher values

        let new_params = ng.step_diagonal(&params, &grads, &fisher_diag);

        assert_eq!(new_params.len(), 3);
        // Larger Fisher = smaller step (with equal gradients)
        let step0 = (new_params[0] - params[0]).abs();
        let step1 = (new_params[1] - params[1]).abs();
        let step2 = (new_params[2] - params[2]).abs();
        // Fisher[1] > Fisher[0] > Fisher[2], so step1 < step0 < step2
        assert!(step1 < step0);
        assert!(step0 < step2);
    }

    #[test]
    fn test_attention_logits_step() {
        let ng = NaturalGradient::new(NaturalGradientConfig::default());

        let logits = vec![0.0; 10];
        let grads = vec![0.1; 10];

        let new_logits = ng.step_attention_logits(&logits, &grads);

        assert_eq!(new_logits.len(), 10);
    }
}
