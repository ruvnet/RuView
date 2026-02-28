//! Diffusion Attention
//!
//! Attention as heat diffusion on a key similarity graph.

use super::laplacian::{GraphLaplacian, LaplacianType};
use crate::error::{AttentionError, AttentionResult};
use crate::traits::Attention;
use serde::{Deserialize, Serialize};

/// Diffusion attention configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffusionConfig {
    /// Model dimension
    pub dim: usize,
    /// Total diffusion time
    pub diffusion_time: f32,
    /// Number of diffusion steps
    pub num_steps: usize,
    /// Sigma for Gaussian kernel
    pub sigma: f32,
    /// Use k-NN sparse Laplacian (0 = dense)
    pub knn_k: usize,
    /// Laplacian type
    pub laplacian_type: LaplacianType,
    /// Temperature for final softmax
    pub temperature: f32,
}

impl Default for DiffusionConfig {
    fn default() -> Self {
        Self {
            dim: 512,
            diffusion_time: 1.0,
            num_steps: 5,
            sigma: 1.0,
            knn_k: 0, // Dense
            laplacian_type: LaplacianType::RandomWalk,
            temperature: 1.0,
        }
    }
}

/// Diffusion-based Attention
///
/// Computes attention by diffusing initial logits on a key similarity graph.
/// This provides multi-scale smoothing and noise resistance.
#[derive(Debug, Clone)]
pub struct DiffusionAttention {
    config: DiffusionConfig,
}

impl DiffusionAttention {
    /// Create new diffusion attention
    pub fn new(config: DiffusionConfig) -> Self {
        Self { config }
    }

    /// Create with dimension only
    pub fn with_dim(dim: usize) -> Self {
        Self::new(DiffusionConfig {
            dim,
            ..Default::default()
        })
    }

    /// Compute diffusion attention
    pub fn compute_diffusion(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
    ) -> AttentionResult<Vec<f32>> {
        let n = keys.len();
        if n == 0 {
            return Err(AttentionError::InvalidConfig("No keys".into()));
        }

        // Build Laplacian
        let laplacian = if self.config.knn_k > 0 {
            GraphLaplacian::from_keys_knn(
                keys,
                self.config.knn_k,
                self.config.sigma,
                self.config.laplacian_type,
            )
        } else {
            GraphLaplacian::from_keys(keys, self.config.sigma, self.config.laplacian_type)
        };

        // Initial logits from dot product
        let mut x: Vec<f32> = keys
            .iter()
            .map(|k| Self::dot_product_simd(query, k))
            .collect();

        // Diffusion: x_{t+dt} = x_t - dt * L * x_t
        let dt = self.config.diffusion_time / self.config.num_steps.max(1) as f32;

        for _ in 0..self.config.num_steps {
            let lx = laplacian.apply(&x);
            for i in 0..n {
                x[i] -= dt * lx[i];
            }
        }

        // Apply temperature (Security: prevent division by zero)
        let temp = self.config.temperature.max(1e-6);
        for xi in x.iter_mut() {
            *xi /= temp;
        }

        // Softmax
        let weights = Self::stable_softmax(&x);

        // Weighted sum of values
        self.weighted_sum(&weights, values)
    }

    /// Compute diffusion energy (for monitoring)
    /// E = x^T L x (smoothness measure)
    pub fn diffusion_energy(&self, x: &[f32], laplacian: &GraphLaplacian) -> f32 {
        let lx = laplacian.apply(x);
        Self::dot_product_simd(x, &lx)
    }

    /// Compute multi-scale attention (return attention at different times)
    pub fn compute_multiscale(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        num_scales: usize,
    ) -> Vec<Vec<f32>> {
        let n = keys.len();
        if n == 0 {
            return vec![];
        }

        let laplacian = if self.config.knn_k > 0 {
            GraphLaplacian::from_keys_knn(
                keys,
                self.config.knn_k,
                self.config.sigma,
                self.config.laplacian_type,
            )
        } else {
            GraphLaplacian::from_keys(keys, self.config.sigma, self.config.laplacian_type)
        };

        let mut x: Vec<f32> = keys
            .iter()
            .map(|k| Self::dot_product_simd(query, k))
            .collect();

        let mut scales = Vec::with_capacity(num_scales);
        scales.push(Self::stable_softmax(&x)); // t=0

        let total_steps = self.config.num_steps * num_scales;
        let dt = self.config.diffusion_time / total_steps.max(1) as f32;
        let steps_per_scale = self.config.num_steps;

        for _ in 1..num_scales {
            for _ in 0..steps_per_scale {
                let lx = laplacian.apply(&x);
                for i in 0..n {
                    x[i] -= dt * lx[i];
                }
            }
            scales.push(Self::stable_softmax(&x));
        }

        scales
    }

    /// SIMD-friendly dot product
    #[inline(always)]
    fn dot_product_simd(a: &[f32], b: &[f32]) -> f32 {
        let len = a.len().min(b.len());
        let chunks = len / 4;
        let remainder = len % 4;

        let mut sum0 = 0.0f32;
        let mut sum1 = 0.0f32;
        let mut sum2 = 0.0f32;
        let mut sum3 = 0.0f32;

        for i in 0..chunks {
            let base = i * 4;
            sum0 += a[base] * b[base];
            sum1 += a[base + 1] * b[base + 1];
            sum2 += a[base + 2] * b[base + 2];
            sum3 += a[base + 3] * b[base + 3];
        }

        let base = chunks * 4;
        for i in 0..remainder {
            sum0 += a[base + i] * b[base + i];
        }

        sum0 + sum1 + sum2 + sum3
    }

    /// Stable softmax
    fn stable_softmax(logits: &[f32]) -> Vec<f32> {
        if logits.is_empty() {
            return vec![];
        }

        let max_logit = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_logits: Vec<f32> = logits.iter().map(|&l| (l - max_logit).exp()).collect();
        let sum: f32 = exp_logits.iter().sum();

        // Security: prevent division by zero if all exp values underflow
        if sum > 0.0 {
            exp_logits.iter().map(|&e| e / sum).collect()
        } else {
            // Fallback to uniform distribution
            vec![1.0 / logits.len() as f32; logits.len()]
        }
    }

    /// Weighted sum
    fn weighted_sum(&self, weights: &[f32], values: &[&[f32]]) -> AttentionResult<Vec<f32>> {
        if weights.is_empty() || values.is_empty() {
            return Err(AttentionError::InvalidConfig("Empty inputs".into()));
        }

        let dim = values[0].len();
        let mut output = vec![0.0f32; dim];

        for (weight, value) in weights.iter().zip(values.iter()) {
            for (o, &v) in output.iter_mut().zip(value.iter()) {
                *o += weight * v;
            }
        }

        Ok(output)
    }
}

impl Attention for DiffusionAttention {
    fn compute(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
    ) -> AttentionResult<Vec<f32>> {
        self.compute_diffusion(query, keys, values)
    }

    fn compute_with_mask(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
        mask: Option<&[bool]>,
    ) -> AttentionResult<Vec<f32>> {
        if let Some(m) = mask {
            let filtered: Vec<(&[f32], &[f32])> = keys
                .iter()
                .zip(values.iter())
                .enumerate()
                .filter(|(i, _)| m.get(*i).copied().unwrap_or(true))
                .map(|(_, (k, v))| (*k, *v))
                .collect();

            let filtered_keys: Vec<&[f32]> = filtered.iter().map(|(k, _)| *k).collect();
            let filtered_values: Vec<&[f32]> = filtered.iter().map(|(_, v)| *v).collect();

            self.compute(query, &filtered_keys, &filtered_values)
        } else {
            self.compute(query, keys, values)
        }
    }

    fn dim(&self) -> usize {
        self.config.dim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diffusion_attention() {
        let attention = DiffusionAttention::with_dim(16);

        let query = vec![1.0f32; 16];
        let keys: Vec<Vec<f32>> = (0..8).map(|i| vec![i as f32 * 0.1; 16]).collect();
        let values: Vec<Vec<f32>> = (0..8).map(|i| vec![i as f32; 16]).collect();

        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

        let output = attention.compute(&query, &keys_refs, &values_refs).unwrap();
        assert_eq!(output.len(), 16);
    }

    #[test]
    fn test_multiscale() {
        let config = DiffusionConfig {
            dim: 8,
            num_steps: 2,
            ..Default::default()
        };
        let attention = DiffusionAttention::new(config);

        let query = vec![1.0f32; 8];
        let keys: Vec<Vec<f32>> = (0..5).map(|i| vec![i as f32 * 0.1; 8]).collect();

        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();

        let scales = attention.compute_multiscale(&query, &keys_refs, 3);

        assert_eq!(scales.len(), 3);
        for scale in scales {
            assert_eq!(scale.len(), 5);
            // Each scale should sum to 1
            let sum: f32 = scale.iter().sum();
            assert!((sum - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn test_knn_diffusion() {
        let config = DiffusionConfig {
            dim: 8,
            knn_k: 3,
            ..Default::default()
        };
        let attention = DiffusionAttention::new(config);

        let query = vec![1.0f32; 8];
        let keys: Vec<Vec<f32>> = (0..10).map(|i| vec![i as f32 * 0.1; 8]).collect();
        let values: Vec<Vec<f32>> = (0..10).map(|i| vec![i as f32; 8]).collect();

        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

        let output = attention.compute(&query, &keys_refs, &values_refs).unwrap();
        assert_eq!(output.len(), 8);
    }
}
