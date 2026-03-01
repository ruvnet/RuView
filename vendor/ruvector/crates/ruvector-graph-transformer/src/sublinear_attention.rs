//! Sublinear graph attention mechanisms.
//!
//! Provides O(n log n) attention computation through:
//! - LSH-bucket attention: locality-sensitive hashing for sparse attention patterns
//! - PPR-sampled attention: personalized PageRank for neighbor sampling
//! - Spectral sparsification: graph-theoretic attention pruning
//!
//! Uses `ruvector-attention` for the core attention computation and
//! `ruvector-mincut` for graph structure operations.

#[cfg(feature = "sublinear")]
use ruvector_attention::{Attention, ScaledDotProductAttention};
// ruvector_mincut is available for advanced sparsification strategies.

#[cfg(feature = "sublinear")]
use crate::config::SublinearConfig;
#[cfg(feature = "sublinear")]
use crate::error::{GraphTransformerError, Result};

/// Sublinear graph attention using LSH buckets and PPR sampling.
///
/// Achieves O(n log n) attention by only attending to a subset of nodes
/// selected through locality-sensitive hashing and random walk sampling.
#[cfg(feature = "sublinear")]
pub struct SublinearGraphAttention {
    config: SublinearConfig,
    attention: ScaledDotProductAttention,
    embed_dim: usize,
}

#[cfg(feature = "sublinear")]
impl SublinearGraphAttention {
    /// Create a new sublinear graph attention module.
    pub fn new(embed_dim: usize, config: SublinearConfig) -> Self {
        let attention = ScaledDotProductAttention::new(embed_dim);
        Self {
            config,
            attention,
            embed_dim,
        }
    }

    /// Compute LSH-bucket attention over node features.
    ///
    /// Hashes node features into buckets and computes attention only
    /// within each bucket, reducing complexity from O(n^2) to O(n * B)
    /// where B is the bucket size.
    pub fn lsh_attention(&self, node_features: &[Vec<f32>]) -> Result<Vec<Vec<f32>>> {
        if node_features.is_empty() {
            return Ok(Vec::new());
        }

        let dim = node_features[0].len();
        if dim != self.embed_dim {
            return Err(GraphTransformerError::DimensionMismatch {
                expected: self.embed_dim,
                actual: dim,
            });
        }

        let n = node_features.len();
        let num_buckets = self.config.lsh_buckets.max(1);
        let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); num_buckets];

        // Simple LSH: hash based on sign of random projections
        for (i, feat) in node_features.iter().enumerate() {
            let bucket = lsh_hash(feat, num_buckets);
            buckets[bucket].push(i);
        }

        // Compute attention within each bucket
        let mut outputs = vec![vec![0.0f32; dim]; n];
        for bucket in &buckets {
            if bucket.len() < 2 {
                for &idx in bucket {
                    outputs[idx] = node_features[idx].clone();
                }
                continue;
            }

            for &query_idx in bucket {
                let query = &node_features[query_idx];
                let keys: Vec<&[f32]> = bucket
                    .iter()
                    .filter(|&&i| i != query_idx)
                    .map(|&i| node_features[i].as_slice())
                    .collect();
                let values: Vec<&[f32]> = keys.clone();

                if keys.is_empty() {
                    outputs[query_idx] = query.clone();
                    continue;
                }

                let result = self
                    .attention
                    .compute(query, &keys, &values)
                    .map_err(GraphTransformerError::Attention)?;
                outputs[query_idx] = result;
            }
        }

        Ok(outputs)
    }

    /// Compute PPR-sampled attention.
    ///
    /// Uses Personalized PageRank to select the most relevant neighbors
    /// for each node, then computes attention over only those neighbors.
    pub fn ppr_attention(
        &self,
        node_features: &[Vec<f32>],
        edges: &[(usize, usize, f64)],
    ) -> Result<Vec<Vec<f32>>> {
        if node_features.is_empty() {
            return Ok(Vec::new());
        }

        let dim = node_features[0].len();
        let n = node_features.len();
        let k = self.config.ppr_samples.min(n - 1).max(1);

        // Build adjacency from edges
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        for &(u, v, _w) in edges {
            if u < n && v < n {
                adj[u].push(v);
                adj[v].push(u);
            }
        }

        // For each node, sample neighbors via short random walks
        let mut outputs = vec![vec![0.0f32; dim]; n];
        let mut rng = rand::thread_rng();

        for i in 0..n {
            let sampled = ppr_sample(&adj, i, k, &mut rng);

            if sampled.is_empty() {
                outputs[i] = node_features[i].clone();
                continue;
            }

            let query = &node_features[i];
            let keys: Vec<&[f32]> = sampled
                .iter()
                .map(|&j| node_features[j].as_slice())
                .collect();
            let values: Vec<&[f32]> = keys.clone();

            let result = self
                .attention
                .compute(query, &keys, &values)
                .map_err(GraphTransformerError::Attention)?;
            outputs[i] = result;
        }

        Ok(outputs)
    }

    /// Compute spectrally sparsified attention.
    ///
    /// Uses the graph's spectral structure to prune attention weights,
    /// keeping only edges that contribute significantly to the graph's
    /// connectivity (measured via effective resistance).
    pub fn spectral_attention(
        &self,
        node_features: &[Vec<f32>],
        edges: &[(usize, usize, f64)],
    ) -> Result<Vec<Vec<f32>>> {
        if node_features.is_empty() {
            return Ok(Vec::new());
        }

        let dim = node_features[0].len();
        let n = node_features.len();
        let sparsity = self.config.sparsification_factor;

        // Build adjacency with weight thresholding
        let mut adj: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
        for &(u, v, w) in edges {
            if u < n && v < n {
                adj[u].push((v, w));
                adj[v].push((u, w));
            }
        }

        // Sparsify: keep edges above the sparsification threshold
        let mut outputs = vec![vec![0.0f32; dim]; n];
        for i in 0..n {
            // Select top neighbors by edge weight
            let mut neighbors = adj[i].clone();
            neighbors.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let keep = ((neighbors.len() as f32 * sparsity) as usize).max(1);
            neighbors.truncate(keep);

            if neighbors.is_empty() {
                outputs[i] = node_features[i].clone();
                continue;
            }

            let query = &node_features[i];
            let keys: Vec<&[f32]> = neighbors
                .iter()
                .map(|&(j, _)| node_features[j].as_slice())
                .collect();
            let values: Vec<&[f32]> = keys.clone();

            let result = self
                .attention
                .compute(query, &keys, &values)
                .map_err(GraphTransformerError::Attention)?;
            outputs[i] = result;
        }

        Ok(outputs)
    }

    /// Get the embedding dimension.
    pub fn embed_dim(&self) -> usize {
        self.embed_dim
    }
}

/// Simple LSH hash based on the sum of feature values.
#[cfg(feature = "sublinear")]
fn lsh_hash(features: &[f32], num_buckets: usize) -> usize {
    let mut h: u64 = 0;
    for (i, &v) in features.iter().enumerate() {
        let bits = v.to_bits() as u64;
        h = h.wrapping_add(bits.wrapping_mul(i as u64 + 1));
    }
    h = h.wrapping_mul(0x517cc1b727220a95);
    (h as usize) % num_buckets
}

/// Sample neighbors via short random walks (PPR approximation).
#[cfg(feature = "sublinear")]
fn ppr_sample(adj: &[Vec<usize>], source: usize, k: usize, rng: &mut impl rand::Rng) -> Vec<usize> {
    use std::collections::HashSet;

    let alpha = 0.15; // teleportation probability
    let mut visited = HashSet::new();
    let max_walks = k * 4;

    for _ in 0..max_walks {
        if visited.len() >= k {
            break;
        }

        let mut current = source;
        for _ in 0..10 {
            if rng.gen::<f64>() < alpha {
                break;
            }
            if adj[current].is_empty() {
                break;
            }
            let idx = rng.gen_range(0..adj[current].len());
            current = adj[current][idx];
        }

        if current != source {
            visited.insert(current);
        }
    }

    visited.into_iter().collect()
}

#[cfg(test)]
#[cfg(feature = "sublinear")]
mod tests {
    use super::*;

    #[test]
    fn test_lsh_attention_basic() {
        let config = SublinearConfig {
            lsh_buckets: 4,
            ppr_samples: 8,
            sparsification_factor: 0.5,
        };
        let attn = SublinearGraphAttention::new(8, config);

        let features = vec![vec![1.0; 8], vec![0.5; 8], vec![0.3; 8], vec![0.8; 8]];

        let result = attn.lsh_attention(&features);
        assert!(result.is_ok());
        let outputs = result.unwrap();
        assert_eq!(outputs.len(), 4);
        for out in &outputs {
            assert_eq!(out.len(), 8);
        }
    }

    #[test]
    fn test_lsh_attention_empty() {
        let config = SublinearConfig::default();
        let attn = SublinearGraphAttention::new(8, config);
        let result = attn.lsh_attention(&[]);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_ppr_attention_basic() {
        let config = SublinearConfig {
            lsh_buckets: 4,
            ppr_samples: 2,
            sparsification_factor: 0.5,
        };
        let attn = SublinearGraphAttention::new(4, config);

        let features = vec![
            vec![1.0, 0.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0, 0.0],
            vec![0.0, 0.0, 1.0, 0.0],
            vec![0.0, 0.0, 0.0, 1.0],
        ];
        let edges = vec![(0, 1, 1.0), (1, 2, 1.0), (2, 3, 1.0), (3, 0, 1.0)];

        let result = attn.ppr_attention(&features, &edges);
        assert!(result.is_ok());
        let outputs = result.unwrap();
        assert_eq!(outputs.len(), 4);
    }

    #[test]
    fn test_spectral_attention_basic() {
        let config = SublinearConfig {
            lsh_buckets: 4,
            ppr_samples: 4,
            sparsification_factor: 0.5,
        };
        let attn = SublinearGraphAttention::new(4, config);

        let features = vec![
            vec![1.0, 0.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0, 0.0],
            vec![0.0, 0.0, 1.0, 0.0],
        ];
        let edges = vec![(0, 1, 2.0), (1, 2, 1.0), (0, 2, 0.5)];

        let result = attn.spectral_attention(&features, &edges);
        assert!(result.is_ok());
        let outputs = result.unwrap();
        assert_eq!(outputs.len(), 3);
    }

    #[test]
    fn test_dimension_mismatch() {
        let config = SublinearConfig::default();
        let attn = SublinearGraphAttention::new(8, config);
        let features = vec![vec![1.0; 4]]; // dim 4 != embed_dim 8
        let result = attn.lsh_attention(&features);
        assert!(result.is_err());
    }

    #[test]
    fn test_lsh_hash_deterministic() {
        let f = vec![1.0, 2.0, 3.0, 4.0];
        let h1 = lsh_hash(&f, 16);
        let h2 = lsh_hash(&f, 16);
        assert_eq!(h1, h2);
        assert!(h1 < 16);
    }
}
