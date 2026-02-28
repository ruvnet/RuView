//! Sliced Wasserstein Attention
//!
//! Attention using Optimal Transport distances via random 1D projections.
//!
//! ## Algorithm
//!
//! 1. Pre-compute P random projections and cache sorted orders per window
//! 2. For each query:
//!    a. Project query onto all P directions
//!    b. Compare to cached sorted key distributions
//!    c. Convert transport costs to attention logits
//!
//! ## Optimizations
//!
//! - Window-level caching of sorted projections
//! - Two-stage: dot-product prefilter + OT on candidates
//! - Histogram CDF for ultra-fast comparisons
//! - SIMD-friendly kernels throughout

use super::cached_projections::{ProjectionCache, WindowCache};
use crate::error::{AttentionError, AttentionResult};
use crate::traits::Attention;
use serde::{Deserialize, Serialize};

/// Configuration for Sliced Wasserstein Attention
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlicedWassersteinConfig {
    /// Model dimension
    pub dim: usize,
    /// Number of random projections (8-16 typical)
    pub num_projections: usize,
    /// Number of candidates for two-stage filtering (32-64 typical)
    pub num_candidates: usize,
    /// Temperature for softmax
    pub temperature: f32,
    /// Whether to use histogram-based CDF (faster but less precise)
    pub use_histograms: bool,
    /// Number of histogram bins if using histograms
    pub num_bins: usize,
    /// Random seed for reproducibility
    pub seed: u64,
    /// Wasserstein power (1 or 2)
    pub wasserstein_power: f32,
}

impl Default for SlicedWassersteinConfig {
    fn default() -> Self {
        Self {
            dim: 512,
            num_projections: 8,
            num_candidates: 48,
            temperature: 1.0,
            use_histograms: false,
            num_bins: 32,
            seed: 42,
            wasserstein_power: 2.0,
        }
    }
}

impl SlicedWassersteinConfig {
    /// Create config with dimension
    pub fn with_dim(dim: usize) -> Self {
        Self {
            dim,
            ..Default::default()
        }
    }
}

/// Sliced Wasserstein Attention
///
/// Uses OT distance instead of dot product for attention scoring.
/// Robust to local permutations and better for comparing distributions.
#[derive(Debug, Clone)]
pub struct SlicedWassersteinAttention {
    config: SlicedWassersteinConfig,
    projection_cache: ProjectionCache,
}

impl SlicedWassersteinAttention {
    /// Create new Sliced Wasserstein attention
    pub fn new(config: SlicedWassersteinConfig) -> Self {
        let projection_cache =
            ProjectionCache::new(config.dim, config.num_projections, config.seed);

        Self {
            config,
            projection_cache,
        }
    }

    /// Create with dimension only (uses defaults)
    pub fn with_dim(dim: usize) -> Self {
        Self::new(SlicedWassersteinConfig::with_dim(dim))
    }

    /// Build window cache for a set of keys
    pub fn build_window_cache(&self, keys: &[&[f32]]) -> WindowCache {
        let mut cache = WindowCache::build(keys, &self.projection_cache);
        if self.config.use_histograms {
            cache.build_histograms(self.config.num_bins);
        }
        cache
    }

    /// Compute attention using pre-built window cache (fast path)
    pub fn compute_with_cache(
        &self,
        query: &[f32],
        window_cache: &WindowCache,
        values: &[&[f32]],
    ) -> AttentionResult<Vec<f32>> {
        let num_keys = window_cache.num_keys;
        if num_keys == 0 {
            return Err(AttentionError::InvalidConfig("No keys provided".into()));
        }

        // Project query
        let query_projections = self.projection_cache.project(query);

        // Compute OT distances to all keys
        let distances = self.compute_ot_distances(&query_projections, window_cache);

        // Convert to attention weights (negative distance = higher attention)
        let logits: Vec<f32> = distances
            .iter()
            .map(|d| -d / self.config.temperature)
            .collect();

        // Softmax
        let weights = Self::stable_softmax(&logits);

        // Weighted sum of values
        self.weighted_sum(&weights, values)
    }

    /// Compute with two-stage filtering (prefilter + OT on candidates)
    pub fn compute_two_stage(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
        window_cache: &WindowCache,
    ) -> AttentionResult<Vec<f32>> {
        let num_keys = keys.len();
        if num_keys == 0 {
            return Err(AttentionError::InvalidConfig("No keys provided".into()));
        }

        let num_candidates = self.config.num_candidates.min(num_keys);

        // Stage 1: Cheap dot-product prefilter to get top-C candidates
        let mut dot_scores: Vec<(usize, f32)> = keys
            .iter()
            .enumerate()
            .map(|(i, k)| (i, Self::dot_product_simd(query, k)))
            .collect();
        dot_scores
            .sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let candidate_indices: Vec<usize> = dot_scores
            .iter()
            .take(num_candidates)
            .map(|(i, _)| *i)
            .collect();

        // Stage 2: OT distance only on candidates
        let query_projections = self.projection_cache.project(query);

        let candidate_distances: Vec<(usize, f32)> = candidate_indices
            .iter()
            .map(|&idx| {
                let key_projs = &window_cache.key_projections[idx];
                let ot_dist = self.compute_1d_ot_distance(&query_projections, key_projs);
                (idx, ot_dist)
            })
            .collect();

        // Convert to attention weights
        let logits: Vec<f32> = candidate_distances
            .iter()
            .map(|(_, d)| -d / self.config.temperature)
            .collect();

        let weights = Self::stable_softmax(&logits);

        // Weighted sum using only candidate values
        let candidate_values: Vec<&[f32]> = candidate_indices.iter().map(|&i| values[i]).collect();

        self.weighted_sum(&weights, &candidate_values)
    }

    /// Compute 1D OT distances using sorted projections
    fn compute_ot_distances(&self, query_projs: &[f32], cache: &WindowCache) -> Vec<f32> {
        let num_keys = cache.num_keys;
        let mut distances = vec![0.0f32; num_keys];

        // For each key, sum OT distances across all projections
        for key_idx in 0..num_keys {
            let key_projs = &cache.key_projections[key_idx];
            distances[key_idx] = self.compute_1d_ot_distance(query_projs, key_projs);
        }

        distances
    }

    /// Compute OT distance between two projected points
    /// Simple case: |q - k|^p averaged across projections
    #[inline]
    fn compute_1d_ot_distance(&self, query_projs: &[f32], key_projs: &[f32]) -> f32 {
        let p = self.config.wasserstein_power;
        let num_proj = query_projs.len();

        if (p - 2.0).abs() < 0.01 {
            // W2: squared differences (SIMD-friendly)
            let sum: f32 = query_projs
                .iter()
                .zip(key_projs.iter())
                .map(|(&q, &k)| {
                    let d = q - k;
                    d * d
                })
                .sum();
            (sum / num_proj as f32).sqrt()
        } else if (p - 1.0).abs() < 0.01 {
            // W1: absolute differences
            let sum: f32 = query_projs
                .iter()
                .zip(key_projs.iter())
                .map(|(&q, &k)| (q - k).abs())
                .sum();
            sum / num_proj as f32
        } else {
            // General case
            let sum: f32 = query_projs
                .iter()
                .zip(key_projs.iter())
                .map(|(&q, &k)| (q - k).abs().powf(p))
                .sum();
            (sum / num_proj as f32).powf(1.0 / p)
        }
    }

    /// Compute OT distance to the window distribution using sorted values
    /// This compares query to the empirical CDF of keys
    #[allow(dead_code)]
    fn compute_distributional_ot(&self, query_projs: &[f32], cache: &WindowCache) -> f32 {
        let num_proj = query_projs.len();
        let mut total_dist = 0.0f32;

        for p in 0..num_proj {
            let sorted = cache.get_sorted(p);
            let q_val = query_projs[p];

            // Find where query falls in the sorted distribution
            // and compute distance to nearest quantile
            let n = sorted.len() as f32;
            let mut min_dist = f32::MAX;

            for (i, &k_val) in sorted.iter().enumerate() {
                let quantile_dist = ((i as f32 + 0.5) / n - 0.5).abs();
                let value_dist = (q_val - k_val).abs();
                min_dist = min_dist.min(value_dist + 0.1 * quantile_dist);
            }

            total_dist += min_dist;
        }

        total_dist / num_proj as f32
    }

    /// Stable softmax implementation
    #[inline]
    fn stable_softmax(logits: &[f32]) -> Vec<f32> {
        if logits.is_empty() {
            return vec![];
        }

        let max_logit = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_logits: Vec<f32> = logits.iter().map(|&l| (l - max_logit).exp()).collect();
        let sum: f32 = exp_logits.iter().sum();

        exp_logits.iter().map(|&e| e / sum).collect()
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

    /// Weighted sum of values
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

impl Attention for SlicedWassersteinAttention {
    fn compute(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
    ) -> AttentionResult<Vec<f32>> {
        // Build cache and compute
        let cache = self.build_window_cache(keys);

        if self.config.num_candidates < keys.len() {
            self.compute_two_stage(query, keys, values, &cache)
        } else {
            self.compute_with_cache(query, &cache, values)
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
            // Filter by mask
            let filtered: Vec<(usize, &[f32], &[f32])> = keys
                .iter()
                .zip(values.iter())
                .enumerate()
                .filter(|(i, _)| m.get(*i).copied().unwrap_or(true))
                .map(|(i, (k, v))| (i, *k, *v))
                .collect();

            let filtered_keys: Vec<&[f32]> = filtered.iter().map(|(_, k, _)| *k).collect();
            let filtered_values: Vec<&[f32]> = filtered.iter().map(|(_, _, v)| *v).collect();

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
    fn test_sliced_wasserstein_attention() {
        let attention = SlicedWassersteinAttention::with_dim(32);

        let query = vec![1.0f32; 32];
        let keys: Vec<Vec<f32>> = (0..10).map(|i| vec![0.5 + i as f32 * 0.1; 32]).collect();
        let values: Vec<Vec<f32>> = (0..10).map(|i| vec![i as f32; 32]).collect();

        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

        let output = attention.compute(&query, &keys_refs, &values_refs).unwrap();
        assert_eq!(output.len(), 32);
    }

    #[test]
    fn test_window_cache_reuse() {
        let attention = SlicedWassersteinAttention::with_dim(64);

        let keys: Vec<Vec<f32>> = (0..20).map(|i| vec![i as f32 * 0.05; 64]).collect();
        let values: Vec<Vec<f32>> = (0..20).map(|i| vec![i as f32; 64]).collect();

        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

        // Build cache once
        let cache = attention.build_window_cache(&keys_refs);

        // Reuse for multiple queries
        for _ in 0..5 {
            let query = vec![0.5f32; 64];
            let output = attention
                .compute_with_cache(&query, &cache, &values_refs)
                .unwrap();
            assert_eq!(output.len(), 64);
        }
    }

    #[test]
    fn test_two_stage_filtering() {
        let config = SlicedWassersteinConfig {
            dim: 32,
            num_candidates: 8,
            ..Default::default()
        };
        let attention = SlicedWassersteinAttention::new(config);

        let query = vec![1.0f32; 32];
        let keys: Vec<Vec<f32>> = (0..50).map(|i| vec![0.5 + i as f32 * 0.02; 32]).collect();
        let values: Vec<Vec<f32>> = (0..50).map(|i| vec![i as f32; 32]).collect();

        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

        let output = attention.compute(&query, &keys_refs, &values_refs).unwrap();
        assert_eq!(output.len(), 32);
    }
}
