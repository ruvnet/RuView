//! Fused Mixed-Curvature Attention
//!
//! Single kernel that computes Euclidean, Hyperbolic (tangent), and Spherical
//! similarities in one pass for maximum cache efficiency.
//!
//! logit(q,k) = a * dot(q_E, k_E) + b * dot(q_H_tan, k_H_tan) + c * dot(q_S, k_S)

use super::tangent_space::{TangentSpaceConfig, TangentSpaceMapper};
use crate::error::{AttentionError, AttentionResult};
use crate::traits::Attention;
use serde::{Deserialize, Serialize};

/// Configuration for fused mixed-curvature attention
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusedCurvatureConfig {
    /// Total dimension
    pub dim: usize,
    /// Euclidean component dimension
    pub euclidean_dim: usize,
    /// Hyperbolic component dimension
    pub hyperbolic_dim: usize,
    /// Spherical component dimension
    pub spherical_dim: usize,
    /// Mixing weight for Euclidean component
    pub weight_e: f32,
    /// Mixing weight for Hyperbolic component
    pub weight_h: f32,
    /// Mixing weight for Spherical component
    pub weight_s: f32,
    /// Hyperbolic curvature
    pub hyperbolic_curvature: f32,
    /// Temperature for softmax
    pub temperature: f32,
    /// Number of attention heads
    pub num_heads: usize,
    /// Per-head weight variation (low-rank)
    pub per_head_variation: f32,
}

impl Default for FusedCurvatureConfig {
    fn default() -> Self {
        Self {
            dim: 512,
            euclidean_dim: 256,
            hyperbolic_dim: 192,
            spherical_dim: 64,
            weight_e: 0.5,
            weight_h: 0.35,
            weight_s: 0.15,
            hyperbolic_curvature: -1.0,
            temperature: 1.0,
            num_heads: 8,
            per_head_variation: 0.1,
        }
    }
}

impl FusedCurvatureConfig {
    /// Validate config
    pub fn validate(&self) -> Result<(), String> {
        if self.euclidean_dim + self.hyperbolic_dim + self.spherical_dim != self.dim {
            return Err("Component dimensions must sum to total dim".into());
        }
        Ok(())
    }

    /// Get component ranges
    pub fn component_ranges(
        &self,
    ) -> (
        std::ops::Range<usize>,
        std::ops::Range<usize>,
        std::ops::Range<usize>,
    ) {
        let e_end = self.euclidean_dim;
        let h_end = e_end + self.hyperbolic_dim;
        let s_end = h_end + self.spherical_dim;

        (0..e_end, e_end..h_end, h_end..s_end)
    }
}

/// Window cache for mixed-curvature attention
#[derive(Debug, Clone)]
pub struct MixedCurvatureCache {
    /// Tangent-space mapped hyperbolic components [N × h_dim]
    pub keys_hyperbolic_tangent: Vec<Vec<f32>>,
    /// Normalized spherical components [N × s_dim]
    pub keys_spherical_normalized: Vec<Vec<f32>>,
    /// Number of keys
    pub num_keys: usize,
}

/// Fused mixed-curvature attention
///
/// Computes attention with Euclidean, Hyperbolic, and Spherical
/// similarities in a single fused kernel.
#[derive(Debug, Clone)]
pub struct MixedCurvatureFusedAttention {
    config: FusedCurvatureConfig,
    tangent_mapper: TangentSpaceMapper,
    /// Per-head weight modifiers [num_heads × 3]
    head_weights: Vec<[f32; 3]>,
}

impl MixedCurvatureFusedAttention {
    /// Create new fused attention
    pub fn new(config: FusedCurvatureConfig) -> Self {
        let tangent_config = TangentSpaceConfig {
            hyperbolic_dim: config.hyperbolic_dim,
            curvature: config.hyperbolic_curvature,
            learnable_origin: true,
        };
        let tangent_mapper = TangentSpaceMapper::new(tangent_config);

        // Initialize per-head weights with small variation
        let head_weights: Vec<[f32; 3]> = (0..config.num_heads)
            .map(|h| {
                let var = config.per_head_variation;
                let h_factor = h as f32 / config.num_heads as f32 - 0.5;
                [
                    config.weight_e + h_factor * var,
                    config.weight_h - h_factor * var * 0.5,
                    config.weight_s + h_factor * var * 0.5,
                ]
            })
            .collect();

        Self {
            config,
            tangent_mapper,
            head_weights,
        }
    }

    /// Create with balanced weights
    pub fn with_dim(dim: usize) -> Self {
        let e_dim = dim / 2;
        let h_dim = dim / 4;
        let s_dim = dim - e_dim - h_dim;

        let config = FusedCurvatureConfig {
            dim,
            euclidean_dim: e_dim,
            hyperbolic_dim: h_dim,
            spherical_dim: s_dim,
            ..Default::default()
        };

        Self::new(config)
    }

    /// Build cache for keys (pre-compute expensive operations)
    pub fn build_cache(&self, keys: &[&[f32]]) -> MixedCurvatureCache {
        let (_e_range, h_range, s_range) = self.config.component_ranges();

        // Pre-map hyperbolic components to tangent space
        let keys_hyperbolic_tangent: Vec<Vec<f32>> = keys
            .iter()
            .map(|k| {
                let h_part = &k[h_range.clone()];
                self.tangent_mapper.log_map(h_part)
            })
            .collect();

        // Pre-normalize spherical components
        let keys_spherical_normalized: Vec<Vec<f32>> = keys
            .iter()
            .map(|k| {
                let s_part = &k[s_range.clone()];
                Self::normalize(s_part)
            })
            .collect();

        MixedCurvatureCache {
            keys_hyperbolic_tangent,
            keys_spherical_normalized,
            num_keys: keys.len(),
        }
    }

    /// Compute attention with cache (fast path)
    pub fn compute_with_cache(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
        cache: &MixedCurvatureCache,
        head_idx: usize,
    ) -> AttentionResult<Vec<f32>> {
        let num_keys = cache.num_keys;
        if num_keys == 0 {
            return Err(AttentionError::InvalidConfig("No keys".into()));
        }

        let (e_range, h_range, s_range) = self.config.component_ranges();
        let weights = &self.head_weights[head_idx % self.head_weights.len()];

        // Extract query components
        let q_e = &query[e_range.clone()];
        let q_h = &query[h_range.clone()];
        let q_s = &query[s_range.clone()];

        // Map query hyperbolic to tangent space
        let q_h_tangent = self.tangent_mapper.log_map(q_h);

        // Normalize query spherical
        let q_s_normalized = Self::normalize(q_s);

        // Compute fused logits
        let logits: Vec<f32> = (0..num_keys)
            .map(|i| {
                let k = keys[i];

                // Euclidean similarity (dot product)
                let sim_e = Self::dot_product_simd(&q_e, &k[e_range.clone()]);

                // Hyperbolic similarity (tangent space dot product)
                let sim_h = Self::dot_product_simd(&q_h_tangent, &cache.keys_hyperbolic_tangent[i]);

                // Spherical similarity (normalized dot product)
                let sim_s =
                    Self::dot_product_simd(&q_s_normalized, &cache.keys_spherical_normalized[i]);

                // Fused logit
                (weights[0] * sim_e + weights[1] * sim_h + weights[2] * sim_s)
                    / self.config.temperature
            })
            .collect();

        // Softmax
        let attention_weights = Self::stable_softmax(&logits);

        // Weighted sum
        self.weighted_sum(&attention_weights, values)
    }

    /// Fused similarity computation (single pass through all components)
    /// This is the hot path - maximize SIMD utilization
    #[inline]
    pub fn fused_similarity(
        &self,
        query: &[f32],
        key: &[f32],
        key_h_tangent: &[f32],
        key_s_normalized: &[f32],
        query_h_tangent: &[f32],
        query_s_normalized: &[f32],
        weights: &[f32; 3],
    ) -> f32 {
        let (e_range, _, _) = self.config.component_ranges();

        // Euclidean: direct dot product on original vectors
        let sim_e = Self::dot_product_simd(&query[e_range.clone()], &key[e_range.clone()]);

        // Hyperbolic: dot product in tangent space
        let sim_h = Self::dot_product_simd(query_h_tangent, key_h_tangent);

        // Spherical: dot product of normalized vectors
        let sim_s = Self::dot_product_simd(query_s_normalized, key_s_normalized);

        weights[0] * sim_e + weights[1] * sim_h + weights[2] * sim_s
    }

    /// Normalize vector to unit length
    #[inline]
    fn normalize(v: &[f32]) -> Vec<f32> {
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-8 {
            v.iter().map(|x| x / norm).collect()
        } else {
            v.to_vec()
        }
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

        exp_logits.iter().map(|&e| e / sum).collect()
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

impl Attention for MixedCurvatureFusedAttention {
    fn compute(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
    ) -> AttentionResult<Vec<f32>> {
        let cache = self.build_cache(keys);
        self.compute_with_cache(query, keys, values, &cache, 0)
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

    fn num_heads(&self) -> usize {
        self.config.num_heads
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fused_attention_config() {
        let config = FusedCurvatureConfig {
            dim: 64,
            euclidean_dim: 32,
            hyperbolic_dim: 24,
            spherical_dim: 8,
            ..Default::default()
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_fused_attention() {
        let config = FusedCurvatureConfig {
            dim: 64,
            euclidean_dim: 32,
            hyperbolic_dim: 24,
            spherical_dim: 8,
            ..Default::default()
        };
        let attention = MixedCurvatureFusedAttention::new(config);

        let query = vec![0.5f32; 64];
        let keys: Vec<Vec<f32>> = (0..20).map(|i| vec![0.1 + i as f32 * 0.02; 64]).collect();
        let values: Vec<Vec<f32>> = (0..20).map(|i| vec![i as f32; 64]).collect();

        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

        let output = attention.compute(&query, &keys_refs, &values_refs).unwrap();
        assert_eq!(output.len(), 64);
    }

    #[test]
    fn test_cache_reuse() {
        let attention = MixedCurvatureFusedAttention::with_dim(32);

        let keys: Vec<Vec<f32>> = (0..10).map(|i| vec![0.1 * i as f32; 32]).collect();
        let values: Vec<Vec<f32>> = (0..10).map(|i| vec![i as f32; 32]).collect();

        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

        let cache = attention.build_cache(&keys_refs);

        // Multiple queries with same cache
        for h in 0..4 {
            let query = vec![0.5f32; 32];
            let output = attention
                .compute_with_cache(&query, &keys_refs, &values_refs, &cache, h)
                .unwrap();
            assert_eq!(output.len(), 32);
        }
    }
}
