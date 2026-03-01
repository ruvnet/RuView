//! Residual-Sparse Attention
//!
//! Generates sparse attention masks based on residual energy.
//! Only computes attention for token pairs with high residuals (incoherent).
//!
//! ## Key Insight
//!
//! Tokens that are already coherent (low residual) don't need expensive attention.
//! By only attending to high-residual pairs, we can achieve significant speedups
//! while maintaining quality.
//!
//! ## Sparsity Pattern
//!
//! Unlike fixed patterns (local, strided), residual-sparse attention adapts to content:
//! - Coherent regions: Few attention connections
//! - Incoherent regions: More attention connections

use crate::error::{AttentionError, AttentionResult};
use crate::sheaf::restriction::RestrictionMap;
use crate::traits::SparseMask;
use serde::{Deserialize, Serialize};

/// Configuration for residual-sparse attention
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SparseResidualConfig {
    /// Residual threshold: only attend if residual > threshold
    pub residual_threshold: f32,
    /// Maximum sparsity ratio (0.0 = full dense, 1.0 = maximally sparse)
    pub max_sparsity: f32,
    /// Minimum connections per query (ensure each query attends to at least k keys)
    pub min_connections: usize,
    /// Whether to always include self-attention (diagonal)
    pub include_self: bool,
    /// Whether to include local window regardless of residual
    pub local_window: Option<usize>,
}

impl Default for SparseResidualConfig {
    fn default() -> Self {
        Self {
            residual_threshold: 0.05,
            max_sparsity: 0.9,
            min_connections: 1,
            include_self: true,
            local_window: Some(8),
        }
    }
}

impl SparseResidualConfig {
    /// Create with residual threshold
    pub fn new(residual_threshold: f32) -> Self {
        Self {
            residual_threshold,
            ..Default::default()
        }
    }

    /// Builder: set residual threshold
    pub fn with_residual_threshold(mut self, threshold: f32) -> Self {
        self.residual_threshold = threshold;
        self
    }

    /// Builder: set max sparsity
    pub fn with_max_sparsity(mut self, sparsity: f32) -> Self {
        self.max_sparsity = sparsity.clamp(0.0, 1.0);
        self
    }

    /// Builder: set minimum connections
    pub fn with_min_connections(mut self, min: usize) -> Self {
        self.min_connections = min;
        self
    }

    /// Builder: set self-attention inclusion
    pub fn with_self_attention(mut self, include: bool) -> Self {
        self.include_self = include;
        self
    }

    /// Builder: set local window
    pub fn with_local_window(mut self, window: Option<usize>) -> Self {
        self.local_window = window;
        self
    }

    /// Validate configuration
    pub fn validate(&self) -> AttentionResult<()> {
        if self.residual_threshold < 0.0 {
            return Err(AttentionError::InvalidConfig(
                "residual_threshold must be non-negative".to_string(),
            ));
        }
        if self.max_sparsity < 0.0 || self.max_sparsity > 1.0 {
            return Err(AttentionError::InvalidConfig(
                "max_sparsity must be in [0, 1]".to_string(),
            ));
        }
        Ok(())
    }
}

/// Sparse mask based on residual energy
#[derive(Debug, Clone)]
pub struct ResidualSparseMask {
    /// Number of queries
    pub n_queries: usize,
    /// Number of keys
    pub n_keys: usize,
    /// Sparse mask indices: (query_idx, key_idx) pairs
    pub connections: Vec<(usize, usize)>,
    /// Optional residual values for each connection
    pub residuals: Option<Vec<f32>>,
    /// Sparsity ratio achieved
    pub sparsity: f32,
}

impl ResidualSparseMask {
    /// Create from connections
    pub fn new(n_queries: usize, n_keys: usize, connections: Vec<(usize, usize)>) -> Self {
        let total_possible = n_queries * n_keys;
        let sparsity = if total_possible > 0 {
            1.0 - (connections.len() as f32 / total_possible as f32)
        } else {
            0.0
        };

        Self {
            n_queries,
            n_keys,
            connections,
            residuals: None,
            sparsity,
        }
    }

    /// Create with residual values
    pub fn with_residuals(
        n_queries: usize,
        n_keys: usize,
        connections: Vec<(usize, usize)>,
        residuals: Vec<f32>,
    ) -> Self {
        let total_possible = n_queries * n_keys;
        let sparsity = if total_possible > 0 {
            1.0 - (connections.len() as f32 / total_possible as f32)
        } else {
            0.0
        };

        Self {
            n_queries,
            n_keys,
            connections,
            residuals: Some(residuals),
            sparsity,
        }
    }

    /// Get number of non-zero connections
    pub fn nnz(&self) -> usize {
        self.connections.len()
    }

    /// Convert to dense boolean mask
    pub fn to_dense_mask(&self) -> Vec<bool> {
        let mut mask = vec![false; self.n_queries * self.n_keys];
        for &(i, j) in &self.connections {
            mask[i * self.n_keys + j] = true;
        }
        mask
    }

    /// Convert to SparseMask (for Attention trait compatibility)
    pub fn to_sparse_mask(&self) -> SparseMask {
        let rows: Vec<usize> = self.connections.iter().map(|(i, _)| *i).collect();
        let cols: Vec<usize> = self.connections.iter().map(|(_, j)| *j).collect();

        SparseMask {
            rows,
            cols,
            values: self.residuals.clone(),
        }
    }

    /// Get connections for a specific query
    pub fn query_connections(&self, query_idx: usize) -> Vec<usize> {
        self.connections
            .iter()
            .filter_map(|&(i, j)| if i == query_idx { Some(j) } else { None })
            .collect()
    }

    /// Get connections as CSR format (row pointers and column indices)
    pub fn to_csr(&self) -> (Vec<usize>, Vec<usize>) {
        let mut row_ptr = vec![0; self.n_queries + 1];
        let mut col_idx = Vec::with_capacity(self.connections.len());

        // Count connections per query
        for &(i, _) in &self.connections {
            row_ptr[i + 1] += 1;
        }

        // Cumulative sum
        for i in 1..=self.n_queries {
            row_ptr[i] += row_ptr[i - 1];
        }

        // Fill column indices (assumes connections are sorted by query)
        let mut current_row = vec![0; self.n_queries];
        col_idx.resize(self.connections.len(), 0);

        for &(i, j) in &self.connections {
            let pos = row_ptr[i] + current_row[i];
            col_idx[pos] = j;
            current_row[i] += 1;
        }

        (row_ptr, col_idx)
    }
}

/// Sparse attention layer based on residual energy
pub struct SparseResidualAttention {
    config: SparseResidualConfig,
    /// Restriction map for computing residuals
    restriction_map: RestrictionMap,
}

impl SparseResidualAttention {
    /// Create new sparse residual attention
    pub fn new(config: SparseResidualConfig, restriction_map: RestrictionMap) -> Self {
        Self {
            config,
            restriction_map,
        }
    }

    /// Create with dimension (creates default restriction map)
    pub fn with_dim(config: SparseResidualConfig, dim: usize) -> Self {
        let restriction_map = RestrictionMap::new(dim, dim);
        Self::new(config, restriction_map)
    }

    /// Get configuration
    pub fn config(&self) -> &SparseResidualConfig {
        &self.config
    }

    /// Get restriction map
    pub fn restriction_map(&self) -> &RestrictionMap {
        &self.restriction_map
    }

    /// Compute residual matrix between queries and keys
    ///
    /// R[i,j] = ||rho(q_i) - rho(k_j)||^2
    pub fn compute_residual_matrix(
        &self,
        queries: &[&[f32]],
        keys: &[&[f32]],
    ) -> AttentionResult<Vec<f32>> {
        let n_q = queries.len();
        let n_k = keys.len();

        // Project all queries and keys
        let q_proj: Vec<Vec<f32>> = queries
            .iter()
            .map(|q| self.restriction_map.apply(q))
            .collect::<AttentionResult<_>>()?;

        let k_proj: Vec<Vec<f32>> = keys
            .iter()
            .map(|k| self.restriction_map.apply(k))
            .collect::<AttentionResult<_>>()?;

        // Compute pairwise residuals
        let mut residuals = vec![0.0; n_q * n_k];
        for i in 0..n_q {
            for j in 0..n_k {
                let residual: f32 = q_proj[i]
                    .iter()
                    .zip(k_proj[j].iter())
                    .map(|(&q, &k)| (q - k) * (q - k))
                    .sum();
                residuals[i * n_k + j] = residual;
            }
        }

        Ok(residuals)
    }

    /// Generate sparse mask based on residual thresholding
    ///
    /// Include connections where residual > threshold (incoherent pairs need attention)
    pub fn generate_mask(
        &self,
        queries: &[&[f32]],
        keys: &[&[f32]],
    ) -> AttentionResult<ResidualSparseMask> {
        let n_q = queries.len();
        let n_k = keys.len();

        let residuals = self.compute_residual_matrix(queries, keys)?;

        let mut connections = Vec::new();
        let mut connection_residuals = Vec::new();

        for i in 0..n_q {
            let mut query_connections: Vec<(usize, f32)> = Vec::new();

            for j in 0..n_k {
                let r = residuals[i * n_k + j];

                // Include self-attention
                if self.config.include_self && i == j && i < n_k {
                    query_connections.push((j, r));
                    continue;
                }

                // Include local window
                if let Some(window) = self.config.local_window {
                    let half_window = window / 2;
                    if (i as isize - j as isize).unsigned_abs() <= half_window {
                        query_connections.push((j, r));
                        continue;
                    }
                }

                // Include high-residual pairs (incoherent - need attention)
                if r > self.config.residual_threshold {
                    query_connections.push((j, r));
                }
            }

            // Ensure minimum connections by adding highest-residual pairs if needed
            if query_connections.len() < self.config.min_connections {
                // Sort all pairs by residual (descending) and take top k
                let mut all_pairs: Vec<(usize, f32)> =
                    (0..n_k).map(|j| (j, residuals[i * n_k + j])).collect();
                all_pairs
                    .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

                for (j, r) in all_pairs.into_iter().take(self.config.min_connections) {
                    if !query_connections.iter().any(|(jj, _)| *jj == j) {
                        query_connections.push((j, r));
                    }
                }
            }

            // Enforce max sparsity
            let max_connections = ((1.0 - self.config.max_sparsity) * n_k as f32).ceil() as usize;
            if query_connections.len() > max_connections {
                // Sort by residual (descending) and keep top max_connections
                query_connections
                    .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                query_connections.truncate(max_connections);
            }

            // Add to global connections
            for (j, r) in query_connections {
                connections.push((i, j));
                connection_residuals.push(r);
            }
        }

        // Sort connections by (i, j) for CSR conversion
        let mut paired: Vec<((usize, usize), f32)> =
            connections.into_iter().zip(connection_residuals).collect();
        paired.sort_by_key(|((i, j), _)| (*i, *j));

        let connections: Vec<(usize, usize)> = paired.iter().map(|(c, _)| *c).collect();
        let residuals: Vec<f32> = paired.iter().map(|(_, r)| *r).collect();

        Ok(ResidualSparseMask::with_residuals(
            n_q,
            n_k,
            connections,
            residuals,
        ))
    }

    /// Compute sparse attention output
    ///
    /// Only computes attention for connections in the mask
    pub fn compute_sparse(
        &self,
        queries: &[&[f32]],
        keys: &[&[f32]],
        values: &[&[f32]],
        mask: &ResidualSparseMask,
        beta: f32,
    ) -> AttentionResult<Vec<Vec<f32>>> {
        if keys.len() != values.len() {
            return Err(AttentionError::DimensionMismatch {
                expected: keys.len(),
                actual: values.len(),
            });
        }

        let n_q = queries.len();
        let dim = if values.is_empty() {
            0
        } else {
            values[0].len()
        };

        let mut outputs = vec![vec![0.0; dim]; n_q];

        // Group connections by query
        for i in 0..n_q {
            let query_conns = mask.query_connections(i);
            if query_conns.is_empty() {
                continue;
            }

            // Compute attention weights for this query's connections
            let residuals: Vec<f32> = query_conns
                .iter()
                .map(|&j| self.restriction_map.energy(queries[i], keys[j]))
                .collect::<AttentionResult<_>>()?;

            // Convert to attention weights: exp(-beta * E) / Z
            let logits: Vec<f32> = residuals.iter().map(|&r| -beta * r).collect();
            let max_logit = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

            let exp_logits: Vec<f32> = logits.iter().map(|&l| (l - max_logit).exp()).collect();
            let sum: f32 = exp_logits.iter().sum();

            let weights: Vec<f32> = if sum > 1e-10 {
                exp_logits.iter().map(|&e| e / sum).collect()
            } else {
                vec![1.0 / query_conns.len() as f32; query_conns.len()]
            };

            // Weighted sum of values
            for (weight, &j) in weights.iter().zip(query_conns.iter()) {
                for (out, &val) in outputs[i].iter_mut().zip(values[j].iter()) {
                    *out += weight * val;
                }
            }
        }

        Ok(outputs)
    }

    /// Efficient sparse matmul: output = sparse_weights @ values
    ///
    /// Uses CSR format for efficiency
    pub fn sparse_matmul(
        &self,
        row_ptr: &[usize],
        col_idx: &[usize],
        weights: &[f32],
        values: &[&[f32]],
    ) -> Vec<Vec<f32>> {
        let n_queries = row_ptr.len() - 1;
        let dim = if values.is_empty() {
            0
        } else {
            values[0].len()
        };

        let mut outputs = vec![vec![0.0; dim]; n_queries];

        for i in 0..n_queries {
            let start = row_ptr[i];
            let end = row_ptr[i + 1];

            for k in start..end {
                let j = col_idx[k];
                let w = weights[k];

                for (out, &val) in outputs[i].iter_mut().zip(values[j].iter()) {
                    *out += w * val;
                }
            }
        }

        outputs
    }
}

/// Statistics about sparsity pattern
#[derive(Debug, Clone)]
pub struct SparsityStatistics {
    /// Total number of queries
    pub n_queries: usize,
    /// Total number of keys
    pub n_keys: usize,
    /// Number of non-zero connections
    pub nnz: usize,
    /// Sparsity ratio (0 = dense, 1 = maximally sparse)
    pub sparsity: f32,
    /// Average connections per query
    pub avg_connections: f32,
    /// Min connections for any query
    pub min_connections: usize,
    /// Max connections for any query
    pub max_connections: usize,
}

impl SparsityStatistics {
    /// Compute statistics from mask
    pub fn from_mask(mask: &ResidualSparseMask) -> Self {
        let n_q = mask.n_queries;
        let n_k = mask.n_keys;
        let nnz = mask.nnz();

        // Count connections per query
        let mut per_query = vec![0usize; n_q];
        for &(i, _) in &mask.connections {
            per_query[i] += 1;
        }

        let min_conn = per_query.iter().cloned().min().unwrap_or(0);
        let max_conn = per_query.iter().cloned().max().unwrap_or(0);
        let avg_conn = if n_q > 0 {
            nnz as f32 / n_q as f32
        } else {
            0.0
        };

        Self {
            n_queries: n_q,
            n_keys: n_k,
            nnz,
            sparsity: mask.sparsity,
            avg_connections: avg_conn,
            min_connections: min_conn,
            max_connections: max_conn,
        }
    }

    /// Estimated speedup from sparsity
    pub fn estimated_speedup(&self) -> f32 {
        if self.sparsity < 1.0 {
            1.0 / (1.0 - self.sparsity)
        } else {
            f32::INFINITY
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = SparseResidualConfig::default();
        assert!(config.residual_threshold > 0.0);
        assert!(config.max_sparsity > 0.0);
        assert!(config.include_self);
    }

    #[test]
    fn test_config_builder() {
        let config = SparseResidualConfig::new(0.1)
            .with_max_sparsity(0.8)
            .with_min_connections(2)
            .with_self_attention(false)
            .with_local_window(None);

        assert_eq!(config.residual_threshold, 0.1);
        assert_eq!(config.max_sparsity, 0.8);
        assert_eq!(config.min_connections, 2);
        assert!(!config.include_self);
        assert!(config.local_window.is_none());
    }

    #[test]
    fn test_sparse_mask_creation() {
        let connections = vec![(0, 0), (0, 1), (1, 1), (1, 2)];
        let mask = ResidualSparseMask::new(2, 3, connections);

        assert_eq!(mask.n_queries, 2);
        assert_eq!(mask.n_keys, 3);
        assert_eq!(mask.nnz(), 4);
        assert!((mask.sparsity - (1.0 - 4.0 / 6.0)).abs() < 1e-6);
    }

    #[test]
    fn test_to_dense_mask() {
        let connections = vec![(0, 0), (0, 2), (1, 1)];
        let mask = ResidualSparseMask::new(2, 3, connections);

        let dense = mask.to_dense_mask();
        assert_eq!(dense.len(), 6);
        assert!(dense[0]); // (0, 0)
        assert!(!dense[1]); // (0, 1)
        assert!(dense[2]); // (0, 2)
        assert!(!dense[3]); // (1, 0)
        assert!(dense[4]); // (1, 1)
        assert!(!dense[5]); // (1, 2)
    }

    #[test]
    fn test_query_connections() {
        let connections = vec![(0, 0), (0, 2), (1, 1), (1, 2)];
        let mask = ResidualSparseMask::new(2, 3, connections);

        assert_eq!(mask.query_connections(0), vec![0, 2]);
        assert_eq!(mask.query_connections(1), vec![1, 2]);
    }

    #[test]
    fn test_to_csr() {
        let connections = vec![(0, 0), (0, 2), (1, 1), (1, 2)];
        let mask = ResidualSparseMask::new(2, 3, connections);

        let (row_ptr, col_idx) = mask.to_csr();

        assert_eq!(row_ptr, vec![0, 2, 4]);
        assert_eq!(col_idx, vec![0, 2, 1, 2]);
    }

    #[test]
    fn test_generate_mask() {
        let config = SparseResidualConfig::default()
            .with_local_window(None)
            .with_self_attention(false)
            .with_min_connections(0);

        let rmap = RestrictionMap::identity(4);
        let sparse = SparseResidualAttention::new(config, rmap);

        // Create queries and keys with varying similarity
        let q1 = vec![1.0, 0.0, 0.0, 0.0];
        let q2 = vec![0.0, 1.0, 0.0, 0.0];
        let k1 = vec![1.0, 0.0, 0.0, 0.0]; // Similar to q1
        let k2 = vec![0.0, 0.0, 1.0, 0.0]; // Different from both

        let queries: Vec<&[f32]> = vec![&q1, &q2];
        let keys: Vec<&[f32]> = vec![&k1, &k2];

        let mask = sparse.generate_mask(&queries, &keys).unwrap();

        // Should have connections for high-residual pairs
        assert!(mask.nnz() > 0);
    }

    #[test]
    fn test_compute_sparse() {
        let config = SparseResidualConfig::default();
        let rmap = RestrictionMap::identity(4);
        let sparse = SparseResidualAttention::new(config, rmap);

        let q1 = vec![1.0, 0.0, 0.0, 0.0];
        let k1 = vec![1.0, 0.0, 0.0, 0.0];
        let k2 = vec![0.0, 1.0, 0.0, 0.0];
        let v1 = vec![1.0, 2.0, 3.0, 4.0];
        let v2 = vec![5.0, 6.0, 7.0, 8.0];

        let queries: Vec<&[f32]> = vec![&q1];
        let keys: Vec<&[f32]> = vec![&k1, &k2];
        let values: Vec<&[f32]> = vec![&v1, &v2];

        let mask = sparse.generate_mask(&queries, &keys).unwrap();
        let output = sparse
            .compute_sparse(&queries, &keys, &values, &mask, 1.0)
            .unwrap();

        assert_eq!(output.len(), 1);
        assert_eq!(output[0].len(), 4);
    }

    #[test]
    fn test_sparsity_statistics() {
        let connections = vec![(0, 0), (0, 1), (1, 0), (1, 1), (1, 2)];
        let mask = ResidualSparseMask::new(2, 3, connections);

        let stats = SparsityStatistics::from_mask(&mask);

        assert_eq!(stats.n_queries, 2);
        assert_eq!(stats.n_keys, 3);
        assert_eq!(stats.nnz, 5);
        assert_eq!(stats.min_connections, 2);
        assert_eq!(stats.max_connections, 3);
        assert!((stats.avg_connections - 2.5).abs() < 1e-6);
    }

    #[test]
    fn test_sparse_matmul() {
        let config = SparseResidualConfig::default();
        let rmap = RestrictionMap::identity(2);
        let sparse = SparseResidualAttention::new(config, rmap);

        // 2x3 sparse matrix with weights
        let row_ptr = vec![0, 2, 3];
        let col_idx = vec![0, 1, 2];
        let weights = vec![0.5, 0.5, 1.0];

        let v1 = vec![1.0, 2.0];
        let v2 = vec![3.0, 4.0];
        let v3 = vec![5.0, 6.0];
        let values: Vec<&[f32]> = vec![&v1, &v2, &v3];

        let output = sparse.sparse_matmul(&row_ptr, &col_idx, &weights, &values);

        assert_eq!(output.len(), 2);
        // Row 0: 0.5 * [1,2] + 0.5 * [3,4] = [2, 3]
        assert!((output[0][0] - 2.0).abs() < 1e-6);
        assert!((output[0][1] - 3.0).abs() < 1e-6);
        // Row 1: 1.0 * [5,6] = [5, 6]
        assert!((output[1][0] - 5.0).abs() < 1e-6);
        assert!((output[1][1] - 6.0).abs() < 1e-6);
    }
}
