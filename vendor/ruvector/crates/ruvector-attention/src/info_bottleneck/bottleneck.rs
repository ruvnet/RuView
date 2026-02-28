//! Information Bottleneck Layer
//!
//! Apply information bottleneck principle to attention.

use super::kl_divergence::{DiagonalGaussian, KLDivergence};
use serde::{Deserialize, Serialize};

/// Information Bottleneck configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IBConfig {
    /// Bottleneck dimension
    pub bottleneck_dim: usize,
    /// Beta parameter (tradeoff between compression and reconstruction)
    pub beta: f32,
    /// Minimum variance (for numerical stability)
    pub min_var: f32,
    /// Whether to use reparameterization trick
    pub reparameterize: bool,
}

impl Default for IBConfig {
    fn default() -> Self {
        Self {
            bottleneck_dim: 64,
            beta: 1e-3,
            min_var: 1e-4,
            reparameterize: true,
        }
    }
}

/// Information Bottleneck for Attention
///
/// Compresses attention representations through a variational bottleneck.
/// Loss = Reconstruction + beta * KL(q(z|x) || p(z))
#[derive(Debug, Clone)]
pub struct InformationBottleneck {
    config: IBConfig,
}

impl InformationBottleneck {
    /// Create new information bottleneck
    pub fn new(config: IBConfig) -> Self {
        Self { config }
    }

    /// Compute IB KL term for attention values
    /// Assumes values encode (mean, log_var) in first 2*bottleneck_dim dims
    pub fn compute_kl_loss(&self, mean: &[f32], log_var: &[f32]) -> f32 {
        let kl = KLDivergence::gaussian_to_unit_arrays(mean, log_var);
        self.config.beta * kl
    }

    /// Compute IB KL term from DiagonalGaussian
    pub fn compute_kl_loss_gaussian(&self, gaussian: &DiagonalGaussian) -> f32 {
        let kl = KLDivergence::gaussian_to_unit(gaussian);
        self.config.beta * kl
    }

    /// Sample from bottleneck distribution (for forward pass)
    pub fn sample(&self, mean: &[f32], log_var: &[f32], epsilon: &[f32]) -> Vec<f32> {
        let n = mean.len().min(log_var.len()).min(epsilon.len());
        let mut z = vec![0.0f32; n];

        for i in 0..n {
            let lv = log_var[i].max(self.config.min_var.ln());
            // Security: clamp to prevent exp() overflow
            let std = (0.5 * lv.clamp(-20.0, 20.0)).exp();
            z[i] = mean[i] + std * epsilon[i];
        }

        z
    }

    /// Compute gradient of KL term w.r.t. mean and log_var
    /// d KL / d mu = mu
    /// d KL / d log_var = 0.5 * (exp(log_var) - 1)
    pub fn kl_gradients(&self, mean: &[f32], log_var: &[f32]) -> (Vec<f32>, Vec<f32>) {
        let n = mean.len().min(log_var.len()); // Security: bounds check

        let mut d_mean = vec![0.0f32; n];
        let mut d_log_var = vec![0.0f32; n];

        for i in 0..n {
            d_mean[i] = self.config.beta * mean[i];
            // Security: clamp log_var to prevent exp() overflow
            let lv_clamped = log_var[i].clamp(-20.0, 20.0);
            d_log_var[i] = self.config.beta * 0.5 * (lv_clamped.exp() - 1.0);
        }

        (d_mean, d_log_var)
    }

    /// Apply bottleneck to attention weights
    /// Returns: (compressed_weights, kl_loss)
    pub fn compress_attention_weights(&self, weights: &[f32], temperature: f32) -> (Vec<f32>, f32) {
        let n = weights.len();

        // Compute entropy-based compression
        let entropy = self.compute_entropy(weights);

        // Target is uniform distribution (maximum entropy)
        let uniform_entropy = (n as f32).ln();

        // KL from attention to uniform is the "information" we're encoding
        let kl = (uniform_entropy - entropy).max(0.0);

        // Apply temperature scaling
        let mut compressed = weights.to_vec();
        for w in compressed.iter_mut() {
            *w = (*w).powf(1.0 / temperature.max(0.1));
        }

        // Renormalize
        let sum: f32 = compressed.iter().sum();
        if sum > 0.0 {
            for w in compressed.iter_mut() {
                *w /= sum;
            }
        }

        (compressed, self.config.beta * kl)
    }

    /// Compute entropy of attention distribution
    fn compute_entropy(&self, weights: &[f32]) -> f32 {
        let eps = 1e-10;
        let mut entropy = 0.0f32;

        for &w in weights {
            if w > eps {
                entropy -= w * w.ln();
            }
        }

        entropy.max(0.0)
    }

    /// Rate-distortion tradeoff
    /// Higher beta = more compression, lower rate
    pub fn set_beta(&mut self, beta: f32) {
        self.config.beta = beta.max(0.0);
    }

    /// Get current beta
    pub fn beta(&self) -> f32 {
        self.config.beta
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ib_kl_loss() {
        let ib = InformationBottleneck::new(IBConfig::default());

        // Unit Gaussian = 0 KL
        let mean = vec![0.0; 16];
        let log_var = vec![0.0; 16];

        let loss = ib.compute_kl_loss(&mean, &log_var);
        assert!(loss.abs() < 1e-5);
    }

    #[test]
    fn test_ib_sample() {
        let ib = InformationBottleneck::new(IBConfig::default());

        let mean = vec![1.0, 2.0];
        let log_var = vec![0.0, 0.0];
        let epsilon = vec![0.0, 0.0];

        let z = ib.sample(&mean, &log_var, &epsilon);

        assert!((z[0] - 1.0).abs() < 1e-5);
        assert!((z[1] - 2.0).abs() < 1e-5);
    }

    #[test]
    fn test_kl_gradients() {
        let ib = InformationBottleneck::new(IBConfig {
            beta: 1.0,
            ..Default::default()
        });

        let mean = vec![1.0, 0.0];
        let log_var = vec![0.0, 0.0];

        let (d_mean, d_log_var) = ib.kl_gradients(&mean, &log_var);

        assert!((d_mean[0] - 1.0).abs() < 1e-5);
        assert!((d_mean[1] - 0.0).abs() < 1e-5);
        assert!((d_log_var[0] - 0.0).abs() < 1e-5);
    }

    #[test]
    fn test_compress_weights() {
        let ib = InformationBottleneck::new(IBConfig::default());

        let weights = vec![0.7, 0.2, 0.1];
        let (compressed, kl) = ib.compress_attention_weights(&weights, 1.0);

        assert_eq!(compressed.len(), 3);
        assert!(kl >= 0.0);

        // Should still sum to 1
        let sum: f32 = compressed.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5);
    }
}
