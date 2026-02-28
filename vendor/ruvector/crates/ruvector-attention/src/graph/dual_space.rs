//! Dual-space attention combining Euclidean and Hyperbolic geometries
//!
//! This module implements attention that operates in both Euclidean and hyperbolic
//! spaces, combining their complementary properties:
//! - Euclidean: Good for flat, local structure
//! - Hyperbolic: Good for hierarchical, tree-like structure

use crate::error::{AttentionError, AttentionResult};
use crate::hyperbolic::project_to_ball;
use crate::traits::Attention;
use crate::utils::stable_softmax;

/// Compute Poincaré distance between two points
fn poincare_dist(u: &[f32], v: &[f32], curvature: f32) -> f32 {
    let c = curvature.abs();
    let sqrt_c = c.sqrt();

    let diff_sq: f32 = u.iter().zip(v.iter()).map(|(a, b)| (a - b).powi(2)).sum();
    let norm_u_sq: f32 = u.iter().map(|x| x * x).sum();
    let norm_v_sq: f32 = v.iter().map(|x| x * x).sum();

    let denom = (1.0 - c * norm_u_sq).max(1e-7) * (1.0 - c * norm_v_sq).max(1e-7);
    let arg = 1.0 + 2.0 * c * diff_sq / denom;

    (1.0 / sqrt_c) * arg.max(1.0).acosh()
}

/// Configuration for dual-space attention
#[derive(Clone, Debug)]
pub struct DualSpaceConfig {
    pub dim: usize,
    pub curvature: f32,
    pub euclidean_weight: f32,
    pub hyperbolic_weight: f32,
    pub learn_weights: bool,
    pub temperature: f32,
}

impl Default for DualSpaceConfig {
    fn default() -> Self {
        Self {
            dim: 256,
            curvature: 1.0,
            euclidean_weight: 0.5,
            hyperbolic_weight: 0.5,
            learn_weights: false,
            temperature: 1.0,
        }
    }
}

impl DualSpaceConfig {
    pub fn builder() -> DualSpaceConfigBuilder {
        DualSpaceConfigBuilder::default()
    }
}

#[derive(Default)]
pub struct DualSpaceConfigBuilder {
    config: DualSpaceConfig,
}

impl DualSpaceConfigBuilder {
    pub fn dim(mut self, d: usize) -> Self {
        self.config.dim = d;
        self
    }

    pub fn curvature(mut self, c: f32) -> Self {
        self.config.curvature = c;
        self
    }

    pub fn euclidean_weight(mut self, w: f32) -> Self {
        self.config.euclidean_weight = w;
        self
    }

    pub fn hyperbolic_weight(mut self, w: f32) -> Self {
        self.config.hyperbolic_weight = w;
        self
    }

    pub fn temperature(mut self, t: f32) -> Self {
        self.config.temperature = t;
        self
    }

    pub fn build(self) -> DualSpaceConfig {
        self.config
    }
}

/// Dual-space attention layer
pub struct DualSpaceAttention {
    config: DualSpaceConfig,
    scale: f32,
    /// Linear projection for Euclidean space
    w_euclidean: Vec<f32>,
    /// Linear projection for hyperbolic space
    w_hyperbolic: Vec<f32>,
    /// Output projection
    w_out: Vec<f32>,
}

impl DualSpaceAttention {
    pub fn new(config: DualSpaceConfig) -> Self {
        let dim = config.dim;
        let scale = 1.0 / (dim as f32).sqrt();

        // Xavier initialization
        let w_scale = (2.0 / (dim + dim) as f32).sqrt();
        let mut seed = 42u64;
        let mut rand = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((seed as f32) / (u64::MAX as f32) - 0.5) * 2.0 * w_scale
        };

        let w_euclidean: Vec<f32> = (0..dim * dim).map(|_| rand()).collect();
        let w_hyperbolic: Vec<f32> = (0..dim * dim).map(|_| rand()).collect();
        let w_out: Vec<f32> = (0..dim * dim).map(|_| rand()).collect();

        Self {
            config,
            scale,
            w_euclidean,
            w_hyperbolic,
            w_out,
        }
    }

    /// Project to Euclidean representation
    fn to_euclidean(&self, x: &[f32]) -> Vec<f32> {
        let dim = self.config.dim;
        (0..dim)
            .map(|i| {
                x.iter()
                    .enumerate()
                    .map(|(j, &xj)| xj * self.w_euclidean[i * dim + j])
                    .sum()
            })
            .collect()
    }

    /// Project to hyperbolic representation (Poincaré ball)
    fn to_hyperbolic(&self, x: &[f32]) -> Vec<f32> {
        let dim = self.config.dim;
        let projected: Vec<f32> = (0..dim)
            .map(|i| {
                x.iter()
                    .enumerate()
                    .map(|(j, &xj)| xj * self.w_hyperbolic[i * dim + j])
                    .sum()
            })
            .collect();

        // Project to ball with curvature
        project_to_ball(&projected, self.config.curvature, 1e-5)
    }

    /// Compute Euclidean similarity (dot product)
    fn euclidean_similarity(&self, q: &[f32], k: &[f32]) -> f32 {
        q.iter().zip(k.iter()).map(|(a, b)| a * b).sum::<f32>() * self.scale
    }

    /// Compute hyperbolic similarity (negative Poincaré distance)
    fn hyperbolic_similarity(&self, q: &[f32], k: &[f32]) -> f32 {
        -poincare_dist(q, k, self.config.curvature)
    }

    /// Output projection
    fn project_output(&self, x: &[f32]) -> Vec<f32> {
        let dim = self.config.dim;
        (0..dim)
            .map(|i| {
                x.iter()
                    .enumerate()
                    .map(|(j, &xj)| xj * self.w_out[i * dim + j])
                    .sum()
            })
            .collect()
    }

    /// Get the contribution weights for analysis
    pub fn get_space_contributions(&self, query: &[f32], keys: &[&[f32]]) -> (Vec<f32>, Vec<f32>) {
        let q_euc = self.to_euclidean(query);
        let q_hyp = self.to_hyperbolic(query);

        let euc_scores: Vec<f32> = keys
            .iter()
            .map(|k| {
                let k_euc = self.to_euclidean(k);
                self.euclidean_similarity(&q_euc, &k_euc)
            })
            .collect();

        let hyp_scores: Vec<f32> = keys
            .iter()
            .map(|k| {
                let k_hyp = self.to_hyperbolic(k);
                self.hyperbolic_similarity(&q_hyp, &k_hyp)
            })
            .collect();

        (euc_scores, hyp_scores)
    }
}

impl Attention for DualSpaceAttention {
    fn compute(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
    ) -> AttentionResult<Vec<f32>> {
        if keys.is_empty() {
            return Err(AttentionError::InvalidConfig("Empty keys".to_string()));
        }
        if query.len() != self.config.dim {
            return Err(AttentionError::DimensionMismatch {
                expected: self.config.dim,
                actual: query.len(),
            });
        }

        let n = keys.len();
        let value_dim = values[0].len();
        let temp = self.config.temperature;

        // Project query to both spaces
        let q_euc = self.to_euclidean(query);
        let q_hyp = self.to_hyperbolic(query);

        // Compute combined scores
        let mut combined_scores = Vec::with_capacity(n);

        for key in keys.iter() {
            let k_euc = self.to_euclidean(key);
            let k_hyp = self.to_hyperbolic(key);

            let euc_score = self.euclidean_similarity(&q_euc, &k_euc);
            let hyp_score = self.hyperbolic_similarity(&q_hyp, &k_hyp);

            // Weighted combination
            let combined = (self.config.euclidean_weight * euc_score
                + self.config.hyperbolic_weight * hyp_score)
                / temp;

            combined_scores.push(combined);
        }

        // Softmax over combined scores
        let weights = stable_softmax(&combined_scores);

        // Weighted sum of values
        let mut output = vec![0.0f32; value_dim];
        for (w, v) in weights.iter().zip(values.iter()) {
            for (o, &vi) in output.iter_mut().zip(v.iter()) {
                *o += w * vi;
            }
        }

        // Output projection
        if value_dim == self.config.dim {
            Ok(self.project_output(&output))
        } else {
            Ok(output)
        }
    }

    fn compute_with_mask(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
        mask: Option<&[bool]>,
    ) -> AttentionResult<Vec<f32>> {
        if let Some(m) = mask {
            let filtered: Vec<(usize, bool)> = m
                .iter()
                .copied()
                .enumerate()
                .filter(|(_, keep)| *keep)
                .collect();
            let filtered_keys: Vec<&[f32]> = filtered.iter().map(|(i, _)| keys[*i]).collect();
            let filtered_values: Vec<&[f32]> = filtered.iter().map(|(i, _)| values[*i]).collect();
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
    fn test_dual_space_basic() {
        let config = DualSpaceConfig::builder()
            .dim(64)
            .curvature(1.0)
            .euclidean_weight(0.5)
            .hyperbolic_weight(0.5)
            .build();

        let attn = DualSpaceAttention::new(config);

        let query = vec![0.1; 64];
        let keys: Vec<Vec<f32>> = (0..10).map(|_| vec![0.1; 64]).collect();
        let values: Vec<Vec<f32>> = (0..10).map(|_| vec![1.0; 64]).collect();

        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

        let result = attn.compute(&query, &keys_refs, &values_refs).unwrap();
        assert_eq!(result.len(), 64);
    }

    #[test]
    fn test_euclidean_dominant() {
        let config = DualSpaceConfig::builder()
            .dim(32)
            .euclidean_weight(1.0)
            .hyperbolic_weight(0.0)
            .build();

        let attn = DualSpaceAttention::new(config);

        let query = vec![0.5; 32];
        let keys: Vec<Vec<f32>> = vec![vec![0.3; 32]; 5];
        let values: Vec<Vec<f32>> = vec![vec![1.0; 32]; 5];

        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

        let result = attn.compute(&query, &keys_refs, &values_refs).unwrap();
        assert_eq!(result.len(), 32);
    }

    #[test]
    fn test_hyperbolic_dominant() {
        let config = DualSpaceConfig::builder()
            .dim(32)
            .curvature(0.5)
            .euclidean_weight(0.0)
            .hyperbolic_weight(1.0)
            .build();

        let attn = DualSpaceAttention::new(config);

        let query = vec![0.1; 32]; // Small values for Poincaré ball
        let keys: Vec<Vec<f32>> = vec![vec![0.1; 32]; 5];
        let values: Vec<Vec<f32>> = vec![vec![1.0; 32]; 5];

        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

        let result = attn.compute(&query, &keys_refs, &values_refs).unwrap();
        assert_eq!(result.len(), 32);
    }

    #[test]
    fn test_space_contributions() {
        let config = DualSpaceConfig::builder()
            .dim(16)
            .euclidean_weight(0.5)
            .hyperbolic_weight(0.5)
            .build();

        let attn = DualSpaceAttention::new(config);

        let query = vec![0.2; 16];
        let keys: Vec<Vec<f32>> = vec![vec![0.2; 16]; 3];
        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();

        let (euc_scores, hyp_scores) = attn.get_space_contributions(&query, &keys_refs);

        assert_eq!(euc_scores.len(), 3);
        assert_eq!(hyp_scores.len(), 3);
    }

    #[test]
    fn test_temperature_scaling() {
        let config_low_temp = DualSpaceConfig::builder().dim(16).temperature(0.5).build();

        let config_high_temp = DualSpaceConfig::builder().dim(16).temperature(2.0).build();

        let attn_low = DualSpaceAttention::new(config_low_temp);
        let attn_high = DualSpaceAttention::new(config_high_temp);

        let query = vec![0.5; 16];
        let keys: Vec<Vec<f32>> = vec![vec![0.8; 16], vec![0.2; 16]];
        let values: Vec<Vec<f32>> = vec![vec![1.0; 16], vec![0.0; 16]];

        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

        let result_low = attn_low.compute(&query, &keys_refs, &values_refs).unwrap();
        let result_high = attn_high.compute(&query, &keys_refs, &values_refs).unwrap();

        // Low temperature should be more peaked (closer to [1,0,0...])
        // High temperature should be more uniform
        // We just verify both compute successfully
        assert_eq!(result_low.len(), 16);
        assert_eq!(result_high.len(), 16);
    }
}
