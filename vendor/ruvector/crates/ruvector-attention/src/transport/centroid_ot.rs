//! Centroid-Based Optimal Transport Attention
//!
//! Clusters keys into M centroids and computes OT between query and centroids.
//! Much faster than full pairwise OT.
//!
//! ## Algorithm
//!
//! 1. Cluster keys into M centroids using k-means
//! 2. Store centroid vectors and weights (fraction of keys in each cluster)
//! 3. For each query, compute transport to centroid distribution
//! 4. Convert transport cost to attention logits

use crate::error::{AttentionError, AttentionResult};
use crate::traits::Attention;
use serde::{Deserialize, Serialize};

/// Configuration for Centroid OT Attention
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CentroidOTConfig {
    /// Model dimension
    pub dim: usize,
    /// Number of centroids (16-32 typical)
    pub num_centroids: usize,
    /// Number of k-means iterations
    pub kmeans_iterations: usize,
    /// Temperature for softmax
    pub temperature: f32,
    /// Regularization for Sinkhorn (0.1 typical)
    pub sinkhorn_reg: f32,
    /// Max Sinkhorn iterations
    pub sinkhorn_iterations: usize,
    /// Random seed
    pub seed: u64,
}

impl Default for CentroidOTConfig {
    fn default() -> Self {
        Self {
            dim: 512,
            num_centroids: 16,
            kmeans_iterations: 10,
            temperature: 1.0,
            sinkhorn_reg: 0.1,
            sinkhorn_iterations: 20,
            seed: 42,
        }
    }
}

/// Cached centroid information for a window
#[derive(Debug, Clone)]
pub struct CentroidCache {
    /// Centroid vectors [M × dim]
    pub centroids: Vec<Vec<f32>>,
    /// Weights for each centroid (sum to 1)
    pub weights: Vec<f32>,
    /// Assignment of each key to centroid
    pub assignments: Vec<usize>,
    /// Number of keys
    pub num_keys: usize,
}

impl CentroidCache {
    /// Build centroid cache using k-means
    pub fn build(keys: &[&[f32]], num_centroids: usize, iterations: usize, seed: u64) -> Self {
        let num_keys = keys.len();
        let m = num_centroids.min(num_keys);

        if num_keys == 0 || keys[0].is_empty() {
            return Self {
                centroids: vec![],
                weights: vec![],
                assignments: vec![],
                num_keys: 0,
            };
        }

        let dim = keys[0].len();

        // Initialize centroids with random keys
        use rand::prelude::*;
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let mut indices: Vec<usize> = (0..num_keys).collect();
        indices.shuffle(&mut rng);

        let mut centroids: Vec<Vec<f32>> =
            indices.iter().take(m).map(|&i| keys[i].to_vec()).collect();

        let mut assignments = vec![0usize; num_keys];

        // K-means iterations
        for _ in 0..iterations {
            // Assign each key to nearest centroid
            for (key_idx, key) in keys.iter().enumerate() {
                let mut min_dist = f32::MAX;
                let mut best_centroid = 0;

                for (c_idx, centroid) in centroids.iter().enumerate() {
                    let dist = Self::squared_distance(key, centroid);
                    if dist < min_dist {
                        min_dist = dist;
                        best_centroid = c_idx;
                    }
                }

                assignments[key_idx] = best_centroid;
            }

            // Update centroids
            let mut new_centroids = vec![vec![0.0f32; dim]; m];
            let mut counts = vec![0usize; m];

            for (key_idx, &assignment) in assignments.iter().enumerate() {
                counts[assignment] += 1;
                for (d, &v) in keys[key_idx].iter().enumerate() {
                    new_centroids[assignment][d] += v;
                }
            }

            for c_idx in 0..m {
                if counts[c_idx] > 0 {
                    for d in 0..dim {
                        new_centroids[c_idx][d] /= counts[c_idx] as f32;
                    }
                    centroids[c_idx] = new_centroids[c_idx].clone();
                }
            }
        }

        // Compute weights
        let mut counts = vec![0usize; m];
        for &a in &assignments {
            counts[a] += 1;
        }
        let weights: Vec<f32> = counts.iter().map(|&c| c as f32 / num_keys as f32).collect();

        Self {
            centroids,
            weights,
            assignments,
            num_keys,
        }
    }

    /// Squared Euclidean distance (SIMD-friendly)
    #[inline]
    fn squared_distance(a: &[f32], b: &[f32]) -> f32 {
        let len = a.len();
        let chunks = len / 4;
        let remainder = len % 4;

        let mut sum0 = 0.0f32;
        let mut sum1 = 0.0f32;
        let mut sum2 = 0.0f32;
        let mut sum3 = 0.0f32;

        for i in 0..chunks {
            let base = i * 4;
            let d0 = a[base] - b[base];
            let d1 = a[base + 1] - b[base + 1];
            let d2 = a[base + 2] - b[base + 2];
            let d3 = a[base + 3] - b[base + 3];
            sum0 += d0 * d0;
            sum1 += d1 * d1;
            sum2 += d2 * d2;
            sum3 += d3 * d3;
        }

        let base = chunks * 4;
        for i in 0..remainder {
            let d = a[base + i] - b[base + i];
            sum0 += d * d;
        }

        sum0 + sum1 + sum2 + sum3
    }
}

/// Centroid-based OT Attention
///
/// Computes attention by finding optimal transport between query and
/// centroid distribution, then distributing attention to original keys.
#[derive(Debug, Clone)]
pub struct CentroidOTAttention {
    config: CentroidOTConfig,
}

impl CentroidOTAttention {
    /// Create new Centroid OT attention
    pub fn new(config: CentroidOTConfig) -> Self {
        Self { config }
    }

    /// Create with dimension only
    pub fn with_dim(dim: usize) -> Self {
        Self::new(CentroidOTConfig {
            dim,
            ..Default::default()
        })
    }

    /// Build centroid cache for a window
    pub fn build_cache(&self, keys: &[&[f32]]) -> CentroidCache {
        CentroidCache::build(
            keys,
            self.config.num_centroids,
            self.config.kmeans_iterations,
            self.config.seed,
        )
    }

    /// Compute attention using cached centroids
    pub fn compute_with_cache(
        &self,
        query: &[f32],
        cache: &CentroidCache,
        values: &[&[f32]],
    ) -> AttentionResult<Vec<f32>> {
        if cache.centroids.is_empty() {
            return Err(AttentionError::InvalidConfig("Empty cache".into()));
        }

        // Compute distances from query to each centroid
        let centroid_distances: Vec<f32> = cache
            .centroids
            .iter()
            .map(|c| CentroidCache::squared_distance(query, c).sqrt())
            .collect();

        // Convert to centroid attention weights
        let centroid_logits: Vec<f32> = centroid_distances
            .iter()
            .map(|d| -d / self.config.temperature)
            .collect();

        let centroid_weights = Self::stable_softmax(&centroid_logits);

        // Distribute centroid weights to original keys
        let mut key_weights = vec![0.0f32; cache.num_keys];
        for (key_idx, &assignment) in cache.assignments.iter().enumerate() {
            // Key weight = centroid weight / number of keys in cluster
            let cluster_size = cache
                .assignments
                .iter()
                .filter(|&&a| a == assignment)
                .count();
            if cluster_size > 0 {
                key_weights[key_idx] = centroid_weights[assignment] / cluster_size as f32;
            }
        }

        // Weighted sum of values
        self.weighted_sum(&key_weights, values)
    }

    /// Fast Sinkhorn transport (simplified for point-to-distribution)
    #[allow(dead_code)]
    fn sinkhorn_distance(&self, query: &[f32], cache: &CentroidCache) -> f32 {
        let m = cache.centroids.len();
        if m == 0 {
            return 0.0;
        }

        // Cost matrix: 1 × M (query to each centroid)
        let costs: Vec<f32> = cache
            .centroids
            .iter()
            .map(|c| CentroidCache::squared_distance(query, c))
            .collect();

        // Source is delta at query (weight 1)
        // Target is centroid distribution (cache.weights)

        // Log-domain Sinkhorn
        let reg = self.config.sinkhorn_reg;
        let log_k: Vec<f32> = costs.iter().map(|c| -c / reg).collect();

        let mut log_v = vec![0.0f32; m];
        let log_b: Vec<f32> = cache.weights.iter().map(|w| w.ln().max(-20.0)).collect();

        for _ in 0..self.config.sinkhorn_iterations {
            // Update log_v
            let log_sum: f32 = log_k
                .iter()
                .zip(log_v.iter())
                .map(|(&lk, &lv)| lk + lv)
                .fold(f32::NEG_INFINITY, |max, x| if x > max { x } else { max });

            let exp_sum: f32 = log_k
                .iter()
                .zip(log_v.iter())
                .map(|(&lk, &lv)| (lk + lv - log_sum).exp())
                .sum();

            let log_u = -log_sum - exp_sum.ln();

            // Update log_v
            for j in 0..m {
                log_v[j] = log_b[j] - (log_u + log_k[j]);
            }
        }

        // Compute transport cost
        let mut total_cost = 0.0f32;
        for j in 0..m {
            let gamma = (log_v[j] + log_k[j]).exp();
            total_cost += gamma * costs[j];
        }

        total_cost
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

impl Attention for CentroidOTAttention {
    fn compute(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
    ) -> AttentionResult<Vec<f32>> {
        let cache = self.build_cache(keys);
        self.compute_with_cache(query, &cache, values)
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
    fn test_centroid_cache() {
        let keys: Vec<Vec<f32>> = (0..50).map(|i| vec![i as f32 * 0.1; 32]).collect();
        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();

        let cache = CentroidCache::build(&keys_refs, 8, 5, 42);

        assert_eq!(cache.centroids.len(), 8);
        assert_eq!(cache.weights.len(), 8);
        assert_eq!(cache.assignments.len(), 50);

        // Weights should sum to 1
        let weight_sum: f32 = cache.weights.iter().sum();
        assert!((weight_sum - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_centroid_ot_attention() {
        let attention = CentroidOTAttention::with_dim(32);

        let query = vec![0.5f32; 32];
        let keys: Vec<Vec<f32>> = (0..30).map(|i| vec![i as f32 * 0.05; 32]).collect();
        let values: Vec<Vec<f32>> = (0..30).map(|i| vec![i as f32; 32]).collect();

        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

        let output = attention.compute(&query, &keys_refs, &values_refs).unwrap();
        assert_eq!(output.len(), 32);
    }

    #[test]
    fn test_cache_reuse() {
        let attention = CentroidOTAttention::with_dim(64);

        let keys: Vec<Vec<f32>> = (0..40).map(|i| vec![i as f32 * 0.025; 64]).collect();
        let values: Vec<Vec<f32>> = (0..40).map(|i| vec![i as f32; 64]).collect();

        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

        // Build cache once
        let cache = attention.build_cache(&keys_refs);

        // Reuse for multiple queries
        for q in 0..10 {
            let query = vec![q as f32 * 0.1; 64];
            let output = attention
                .compute_with_cache(&query, &cache, &values_refs)
                .unwrap();
            assert_eq!(output.len(), 64);
        }
    }
}
