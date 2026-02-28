//! Sheaf Attention Layer
//!
//! Implements coherence-based attention where weights are inversely proportional
//! to residual energy:
//!
//! ```text
//! A_ij = exp(-beta * E_ij) / sum_k exp(-beta * E_ik)
//! ```
//!
//! ## Key Properties
//!
//! - High residual (incoherent) -> Low attention (don't propagate inconsistency)
//! - Low residual (coherent) -> High attention (reinforce consistency)
//! - Beta parameter controls temperature (higher = sharper attention)

use crate::error::{AttentionError, AttentionResult};
use crate::sheaf::restriction::RestrictionMap;
use crate::traits::Attention;
use crate::utils::stable_softmax;
use serde::{Deserialize, Serialize};

/// Configuration for sheaf attention
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheafAttentionConfig {
    /// Model dimension
    pub dim: usize,
    /// Number of attention heads
    pub num_heads: usize,
    /// Temperature parameter (higher = sharper attention)
    pub beta: f32,
    /// Sparsity threshold for attention (skip if energy > threshold)
    pub sparsity_threshold: Option<f32>,
    /// Whether to use shared restriction maps across heads
    pub shared_restrictions: bool,
    /// Dropout probability (0.0 = no dropout)
    pub dropout: f32,
}

impl Default for SheafAttentionConfig {
    fn default() -> Self {
        Self {
            dim: 64,
            num_heads: 1,
            beta: 1.0,
            sparsity_threshold: None,
            shared_restrictions: false,
            dropout: 0.0,
        }
    }
}

impl SheafAttentionConfig {
    /// Create config with dimension
    pub fn new(dim: usize) -> Self {
        Self {
            dim,
            ..Default::default()
        }
    }

    /// Builder: set number of heads
    pub fn with_num_heads(mut self, num_heads: usize) -> Self {
        self.num_heads = num_heads;
        self
    }

    /// Builder: set beta temperature
    pub fn with_beta(mut self, beta: f32) -> Self {
        self.beta = beta;
        self
    }

    /// Builder: set sparsity threshold
    pub fn with_sparsity_threshold(mut self, threshold: f32) -> Self {
        self.sparsity_threshold = Some(threshold);
        self
    }

    /// Builder: set shared restrictions
    pub fn with_shared_restrictions(mut self, shared: bool) -> Self {
        self.shared_restrictions = shared;
        self
    }

    /// Builder: set dropout
    pub fn with_dropout(mut self, dropout: f32) -> Self {
        self.dropout = dropout;
        self
    }

    /// Compute head dimension
    pub fn head_dim(&self) -> usize {
        self.dim / self.num_heads
    }

    /// Validate configuration
    pub fn validate(&self) -> AttentionResult<()> {
        if self.dim == 0 {
            return Err(AttentionError::InvalidConfig(
                "dimension must be positive".to_string(),
            ));
        }
        if self.num_heads == 0 {
            return Err(AttentionError::InvalidConfig(
                "num_heads must be positive".to_string(),
            ));
        }
        if self.dim % self.num_heads != 0 {
            return Err(AttentionError::InvalidHeadCount {
                dim: self.dim,
                num_heads: self.num_heads,
            });
        }
        if self.beta <= 0.0 {
            return Err(AttentionError::InvalidConfig(
                "beta must be positive".to_string(),
            ));
        }
        if self.dropout < 0.0 || self.dropout >= 1.0 {
            return Err(AttentionError::InvalidConfig(
                "dropout must be in [0, 1)".to_string(),
            ));
        }
        Ok(())
    }
}

/// Sheaf Attention Layer
///
/// Uses restriction maps instead of learned QKV projections and computes
/// attention weights based on residual energy.
pub struct SheafAttention {
    config: SheafAttentionConfig,
    /// Restriction map for queries
    rho_query: RestrictionMap,
    /// Restriction map for keys
    rho_key: RestrictionMap,
    /// Restriction map for values
    rho_value: RestrictionMap,
}

impl SheafAttention {
    /// Create new sheaf attention layer
    pub fn new(config: SheafAttentionConfig) -> Self {
        let head_dim = config.head_dim();

        let rho_query = RestrictionMap::new(config.dim, head_dim);
        let rho_key = RestrictionMap::new(config.dim, head_dim);
        let rho_value = RestrictionMap::new(config.dim, head_dim);

        Self {
            config,
            rho_query,
            rho_key,
            rho_value,
        }
    }

    /// Create with custom restriction maps
    pub fn with_restriction_maps(
        config: SheafAttentionConfig,
        rho_query: RestrictionMap,
        rho_key: RestrictionMap,
        rho_value: RestrictionMap,
    ) -> Self {
        Self {
            config,
            rho_query,
            rho_key,
            rho_value,
        }
    }

    /// Get configuration
    pub fn config(&self) -> &SheafAttentionConfig {
        &self.config
    }

    /// Get query restriction map
    pub fn rho_query(&self) -> &RestrictionMap {
        &self.rho_query
    }

    /// Get key restriction map
    pub fn rho_key(&self) -> &RestrictionMap {
        &self.rho_key
    }

    /// Get value restriction map
    pub fn rho_value(&self) -> &RestrictionMap {
        &self.rho_value
    }

    /// Get mutable query restriction map (for training)
    pub fn rho_query_mut(&mut self) -> &mut RestrictionMap {
        &mut self.rho_query
    }

    /// Get mutable key restriction map (for training)
    pub fn rho_key_mut(&mut self) -> &mut RestrictionMap {
        &mut self.rho_key
    }

    /// Get mutable value restriction map (for training)
    pub fn rho_value_mut(&mut self) -> &mut RestrictionMap {
        &mut self.rho_value
    }

    /// Compute residual energy between query and key
    ///
    /// E_qk = ||rho_q(q) - rho_k(k)||^2
    pub fn compute_energy(&self, query: &[f32], key: &[f32]) -> AttentionResult<f32> {
        let q_proj = self.rho_query.apply(query)?;
        let k_proj = self.rho_key.apply(key)?;

        let energy: f32 = q_proj
            .iter()
            .zip(k_proj.iter())
            .map(|(&q, &k)| (q - k) * (q - k))
            .sum();

        Ok(energy)
    }

    /// Compute energy matrix for all query-key pairs
    ///
    /// E[i,j] = ||rho_q(q_i) - rho_k(k_j)||^2
    pub fn compute_energy_matrix(
        &self,
        queries: &[&[f32]],
        keys: &[&[f32]],
    ) -> AttentionResult<Vec<f32>> {
        let n_q = queries.len();
        let n_k = keys.len();

        // Project all queries and keys
        let q_proj: Vec<Vec<f32>> = queries
            .iter()
            .map(|q| self.rho_query.apply(q))
            .collect::<AttentionResult<_>>()?;

        let k_proj: Vec<Vec<f32>> = keys
            .iter()
            .map(|k| self.rho_key.apply(k))
            .collect::<AttentionResult<_>>()?;

        // Compute pairwise energies
        let mut energies = vec![0.0; n_q * n_k];
        for i in 0..n_q {
            for j in 0..n_k {
                let energy: f32 = q_proj[i]
                    .iter()
                    .zip(k_proj[j].iter())
                    .map(|(&q, &k)| (q - k) * (q - k))
                    .sum();
                energies[i * n_k + j] = energy;
            }
        }

        Ok(energies)
    }

    /// Convert energy matrix to attention weights
    ///
    /// A_ij = exp(-beta * E_ij) / Z
    pub fn energy_to_attention(&self, energies: &[f32], n_keys: usize) -> Vec<f32> {
        let n_queries = energies.len() / n_keys;
        let mut weights = Vec::with_capacity(energies.len());

        for i in 0..n_queries {
            let row_start = i * n_keys;
            let row = &energies[row_start..row_start + n_keys];

            // Apply sparsity threshold if configured
            let masked_logits: Vec<f32> = if let Some(threshold) = self.config.sparsity_threshold {
                row.iter()
                    .map(|&e| {
                        if e > threshold {
                            f32::NEG_INFINITY // Mask out high-energy pairs
                        } else {
                            -self.config.beta * e
                        }
                    })
                    .collect()
            } else {
                row.iter().map(|&e| -self.config.beta * e).collect()
            };

            let row_weights = stable_softmax(&masked_logits);
            weights.extend(row_weights);
        }

        weights
    }

    /// Compute sheaf attention output
    ///
    /// 1. Project queries and keys through restriction maps
    /// 2. Compute residual energy matrix
    /// 3. Convert to attention weights: exp(-beta * E) / Z
    /// 4. Weight values and sum
    pub fn forward(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
    ) -> AttentionResult<(Vec<f32>, Vec<f32>)> {
        if keys.len() != values.len() {
            return Err(AttentionError::DimensionMismatch {
                expected: keys.len(),
                actual: values.len(),
            });
        }

        if keys.is_empty() {
            return Err(AttentionError::EmptyInput(
                "keys cannot be empty".to_string(),
            ));
        }

        let n_keys = keys.len();

        // Compute energies for this query against all keys
        let mut energies = Vec::with_capacity(n_keys);
        for key in keys {
            energies.push(self.compute_energy(query, key)?);
        }

        // Convert to attention weights
        let logits: Vec<f32> = if let Some(threshold) = self.config.sparsity_threshold {
            energies
                .iter()
                .map(|&e| {
                    if e > threshold {
                        f32::NEG_INFINITY
                    } else {
                        -self.config.beta * e
                    }
                })
                .collect()
        } else {
            energies.iter().map(|&e| -self.config.beta * e).collect()
        };

        let attention_weights = stable_softmax(&logits);

        // Project values and compute weighted sum
        let v_proj: Vec<Vec<f32>> = values
            .iter()
            .map(|v| self.rho_value.apply(v))
            .collect::<AttentionResult<_>>()?;

        let head_dim = self.config.head_dim();
        let mut output = vec![0.0; head_dim];

        for (weight, v) in attention_weights.iter().zip(v_proj.iter()) {
            for (out, &val) in output.iter_mut().zip(v.iter()) {
                *out += weight * val;
            }
        }

        Ok((output, attention_weights))
    }

    /// Compute total energy for a token (sum over all keys)
    ///
    /// E_i = sum_j E_ij
    pub fn token_energy(&self, query: &[f32], keys: &[&[f32]]) -> AttentionResult<f32> {
        let mut total_energy = 0.0;
        for key in keys {
            total_energy += self.compute_energy(query, key)?;
        }
        Ok(total_energy)
    }

    /// Compute average energy for a token
    ///
    /// E_avg = (1/N) * sum_j E_ij
    pub fn average_token_energy(&self, query: &[f32], keys: &[&[f32]]) -> AttentionResult<f32> {
        if keys.is_empty() {
            return Ok(0.0);
        }
        Ok(self.token_energy(query, keys)? / keys.len() as f32)
    }
}

impl Attention for SheafAttention {
    fn compute(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
    ) -> AttentionResult<Vec<f32>> {
        let (output, _weights) = self.forward(query, keys, values)?;
        Ok(output)
    }

    fn compute_with_mask(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
        mask: Option<&[bool]>,
    ) -> AttentionResult<Vec<f32>> {
        if keys.len() != values.len() {
            return Err(AttentionError::DimensionMismatch {
                expected: keys.len(),
                actual: values.len(),
            });
        }

        if keys.is_empty() {
            return Err(AttentionError::EmptyInput(
                "keys cannot be empty".to_string(),
            ));
        }

        let n_keys = keys.len();

        // Compute energies
        let mut energies = Vec::with_capacity(n_keys);
        for key in keys {
            energies.push(self.compute_energy(query, key)?);
        }

        // Apply mask and convert to logits
        let logits: Vec<f32> = if let Some(m) = mask {
            if m.len() != n_keys {
                return Err(AttentionError::InvalidMask {
                    expected: n_keys.to_string(),
                    actual: m.len().to_string(),
                });
            }

            energies
                .iter()
                .zip(m.iter())
                .map(|(&e, &keep)| {
                    if !keep {
                        f32::NEG_INFINITY
                    } else if let Some(threshold) = self.config.sparsity_threshold {
                        if e > threshold {
                            f32::NEG_INFINITY
                        } else {
                            -self.config.beta * e
                        }
                    } else {
                        -self.config.beta * e
                    }
                })
                .collect()
        } else if let Some(threshold) = self.config.sparsity_threshold {
            energies
                .iter()
                .map(|&e| {
                    if e > threshold {
                        f32::NEG_INFINITY
                    } else {
                        -self.config.beta * e
                    }
                })
                .collect()
        } else {
            energies.iter().map(|&e| -self.config.beta * e).collect()
        };

        let attention_weights = stable_softmax(&logits);

        // Project values and compute weighted sum
        let v_proj: Vec<Vec<f32>> = values
            .iter()
            .map(|v| self.rho_value.apply(v))
            .collect::<AttentionResult<_>>()?;

        let head_dim = self.config.head_dim();
        let mut output = vec![0.0; head_dim];

        for (weight, v) in attention_weights.iter().zip(v_proj.iter()) {
            for (out, &val) in output.iter_mut().zip(v.iter()) {
                *out += weight * val;
            }
        }

        Ok(output)
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
    fn test_config_default() {
        let config = SheafAttentionConfig::default();
        assert_eq!(config.dim, 64);
        assert_eq!(config.num_heads, 1);
        assert_eq!(config.beta, 1.0);
        assert!(config.sparsity_threshold.is_none());
    }

    #[test]
    fn test_config_builder() {
        let config = SheafAttentionConfig::new(128)
            .with_num_heads(4)
            .with_beta(2.0)
            .with_sparsity_threshold(0.5)
            .with_dropout(0.1);

        assert_eq!(config.dim, 128);
        assert_eq!(config.num_heads, 4);
        assert_eq!(config.head_dim(), 32);
        assert_eq!(config.beta, 2.0);
        assert_eq!(config.sparsity_threshold, Some(0.5));
        assert_eq!(config.dropout, 0.1);
    }

    #[test]
    fn test_config_validation() {
        assert!(SheafAttentionConfig::new(64).validate().is_ok());

        assert!(SheafAttentionConfig::new(64)
            .with_num_heads(3)
            .validate()
            .is_err()); // 64 not divisible by 3

        assert!(SheafAttentionConfig::new(64)
            .with_beta(-1.0)
            .validate()
            .is_err());
    }

    #[test]
    fn test_sheaf_attention_creation() {
        let config = SheafAttentionConfig::new(64).with_num_heads(4);
        let attention = SheafAttention::new(config);

        assert_eq!(attention.dim(), 64);
        assert_eq!(attention.num_heads(), 4);
    }

    #[test]
    fn test_compute_energy() {
        let config = SheafAttentionConfig::new(8);
        let attention = SheafAttention::new(config);

        let q = vec![1.0; 8];
        let k = vec![1.0; 8];

        let energy = attention.compute_energy(&q, &k).unwrap();
        assert!(energy >= 0.0); // Energy is non-negative
    }

    #[test]
    fn test_energy_zero_for_identical() {
        // With identity-like restriction maps, identical vectors should have low energy
        let config = SheafAttentionConfig::new(4);
        let rho = RestrictionMap::identity(4);
        let attention =
            SheafAttention::with_restriction_maps(config, rho.clone(), rho.clone(), rho);

        let v = vec![1.0, 2.0, 3.0, 4.0];
        let energy = attention.compute_energy(&v, &v).unwrap();
        assert!(energy.abs() < 1e-6);
    }

    #[test]
    fn test_forward() {
        let config = SheafAttentionConfig::new(8);
        let attention = SheafAttention::new(config);

        let query = vec![1.0; 8];
        let k1 = vec![1.0; 8];
        let k2 = vec![0.5; 8];
        let v1 = vec![1.0; 8];
        let v2 = vec![2.0; 8];

        let keys: Vec<&[f32]> = vec![&k1, &k2];
        let values: Vec<&[f32]> = vec![&v1, &v2];

        let (output, weights) = attention.forward(&query, &keys, &values).unwrap();

        // Output should be head_dim
        assert_eq!(output.len(), 8);

        // Weights should sum to 1
        let weight_sum: f32 = weights.iter().sum();
        assert!((weight_sum - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_attention_trait() {
        let config = SheafAttentionConfig::new(8);
        let attention = SheafAttention::new(config);

        let query = vec![1.0; 8];
        let k1 = vec![1.0; 8];
        let k2 = vec![0.5; 8];
        let v1 = vec![1.0; 8];
        let v2 = vec![2.0; 8];

        let keys: Vec<&[f32]> = vec![&k1, &k2];
        let values: Vec<&[f32]> = vec![&v1, &v2];

        let output = attention.compute(&query, &keys, &values).unwrap();
        assert_eq!(output.len(), 8);
    }

    #[test]
    fn test_attention_with_mask() {
        let config = SheafAttentionConfig::new(8);
        let attention = SheafAttention::new(config);

        let query = vec![1.0; 8];
        let k1 = vec![1.0; 8];
        let k2 = vec![0.5; 8];
        let v1 = vec![1.0; 8];
        let v2 = vec![2.0; 8];

        let keys: Vec<&[f32]> = vec![&k1, &k2];
        let values: Vec<&[f32]> = vec![&v1, &v2];
        let mask = vec![true, false]; // Only attend to first key

        let output = attention
            .compute_with_mask(&query, &keys, &values, Some(&mask))
            .unwrap();
        assert_eq!(output.len(), 8);
    }

    #[test]
    fn test_sparsity_threshold() {
        let config = SheafAttentionConfig::new(8).with_sparsity_threshold(0.1);
        let attention = SheafAttention::new(config);

        let query = vec![1.0; 8];
        let k1 = vec![1.0; 8];
        let k2 = vec![100.0; 8]; // Very different - high energy
        let v1 = vec![1.0; 8];
        let v2 = vec![2.0; 8];

        let keys: Vec<&[f32]> = vec![&k1, &k2];
        let values: Vec<&[f32]> = vec![&v1, &v2];

        let (_output, weights) = attention.forward(&query, &keys, &values).unwrap();

        // Second key should have near-zero weight due to high energy
        // (depends on initialization, but the masked one should be lower)
        assert!(weights[0] > weights[1]);
    }

    #[test]
    fn test_token_energy() {
        let config = SheafAttentionConfig::new(8);
        let attention = SheafAttention::new(config);

        let query = vec![1.0; 8];
        let k1 = vec![1.0; 8];
        let k2 = vec![0.5; 8];

        let keys: Vec<&[f32]> = vec![&k1, &k2];

        let total_energy = attention.token_energy(&query, &keys).unwrap();
        let avg_energy = attention.average_token_energy(&query, &keys).unwrap();

        assert!(total_energy >= 0.0);
        assert!((avg_energy - total_energy / 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_beta_effect() {
        // Higher beta = sharper attention (more peaked distribution)
        let config_low = SheafAttentionConfig::new(8).with_beta(0.1);
        let config_high = SheafAttentionConfig::new(8).with_beta(10.0);

        // Use same restriction maps
        let rho = RestrictionMap::new(8, 8);
        let attention_low = SheafAttention::with_restriction_maps(
            config_low,
            rho.clone(),
            rho.clone(),
            rho.clone(),
        );
        let attention_high =
            SheafAttention::with_restriction_maps(config_high, rho.clone(), rho.clone(), rho);

        let query = vec![1.0; 8];
        let k1 = vec![1.0; 8];
        let k2 = vec![0.5; 8];
        let v1 = vec![1.0; 8];
        let v2 = vec![2.0; 8];

        let keys: Vec<&[f32]> = vec![&k1, &k2];
        let values: Vec<&[f32]> = vec![&v1, &v2];

        let (_out_low, weights_low) = attention_low.forward(&query, &keys, &values).unwrap();
        let (_out_high, weights_high) = attention_high.forward(&query, &keys, &values).unwrap();

        // High beta should have more peaked distribution
        let max_low = weights_low.iter().cloned().fold(0.0f32, f32::max);
        let max_high = weights_high.iter().cloned().fold(0.0f32, f32::max);

        assert!(max_high >= max_low);
    }
}
