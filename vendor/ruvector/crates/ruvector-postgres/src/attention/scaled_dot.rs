//! # Scaled Dot-Product Attention
//!
//! Implements the standard transformer attention mechanism:
//! Attention(Q, K, V) = softmax(QK^T / √d_k) V
//!
//! Uses SIMD-accelerated operations via simsimd for efficient computation.

use super::{softmax_inplace, Attention};
use simsimd::SpatialSimilarity;

/// Scaled dot-product attention mechanism
///
/// This is the core attention operation used in transformers.
/// Time complexity: O(n²d) where n=sequence length, d=dimension
/// Space complexity: O(n²)
#[derive(Debug, Clone)]
pub struct ScaledDotAttention {
    /// Scale factor: 1/√d_k for numerical stability
    scale: f32,

    /// Optional dropout rate (not used in inference)
    #[allow(dead_code)]
    dropout: Option<f32>,

    /// Whether to use SIMD acceleration
    use_simd: bool,
}

impl ScaledDotAttention {
    /// Create a new scaled dot-product attention mechanism
    ///
    /// # Arguments
    /// * `head_dim` - Dimension of each attention head (d_k)
    ///
    /// # Returns
    /// A new ScaledDotAttention instance with scale = 1/√head_dim
    pub fn new(head_dim: usize) -> Self {
        Self {
            scale: 1.0 / (head_dim as f32).sqrt(),
            dropout: None,
            use_simd: true,
        }
    }

    /// Create with custom scale factor
    pub fn with_scale(scale: f32) -> Self {
        Self {
            scale,
            dropout: None,
            use_simd: true,
        }
    }

    /// Disable SIMD acceleration (for testing)
    pub fn without_simd(mut self) -> Self {
        self.use_simd = false;
        self
    }

    /// SIMD-accelerated dot product
    #[inline]
    fn dot_product(&self, a: &[f32], b: &[f32]) -> f32 {
        if self.use_simd && a.len() == b.len() {
            // Try SIMD first - simsimd returns Option<f64>
            if let Some(result) = f32::dot(a, b) {
                return result as f32;
            }
        }

        // Fallback to scalar implementation
        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
    }

    /// Compute raw attention logits (before softmax)
    #[inline]
    pub fn compute_logits(&self, query: &[f32], keys: &[&[f32]]) -> Vec<f32> {
        keys.iter()
            .map(|key| self.dot_product(query, key) * self.scale)
            .collect()
    }
}

impl Default for ScaledDotAttention {
    fn default() -> Self {
        // Default to 64-dimensional heads (common in transformers)
        Self::new(64)
    }
}

impl Attention for ScaledDotAttention {
    /// Compute attention scores: softmax(QK^T / √d_k)
    ///
    /// # Arguments
    /// * `query` - Query vector [d_k]
    /// * `keys` - Slice of key vectors, each [d_k]
    ///
    /// # Returns
    /// Attention scores (probabilities) for each key, sum = 1.0
    fn attention_scores(&self, query: &[f32], keys: &[&[f32]]) -> Vec<f32> {
        if keys.is_empty() {
            return Vec::new();
        }

        // Compute scaled dot products
        let mut scores = self.compute_logits(query, keys);

        // Apply softmax
        softmax_inplace(&mut scores);

        scores
    }

    /// Full forward pass: compute attention and apply to values
    ///
    /// # Arguments
    /// * `query` - Query vector [d_k]
    /// * `keys` - Key vectors [n, d_k]
    /// * `values` - Value vectors [n, d_v]
    ///
    /// # Returns
    /// Attention-weighted combination of values [d_v]
    fn forward(&self, query: &[f32], keys: &[&[f32]], values: &[&[f32]]) -> Vec<f32> {
        assert_eq!(
            keys.len(),
            values.len(),
            "Keys and values must have same length"
        );

        if keys.is_empty() {
            return Vec::new();
        }

        // Compute attention scores
        let scores = self.attention_scores(query, keys);

        // Apply to values
        self.apply_attention(&scores, values)
    }
}

#[cfg(feature = "pg_test")]
#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_scaled_dot_basic() {
        let attention = ScaledDotAttention::new(4);

        let query = vec![1.0, 0.0, 0.0, 0.0];
        let key1 = vec![1.0, 0.0, 0.0, 0.0];
        let key2 = vec![0.0, 1.0, 0.0, 0.0];
        let keys = vec![&key1[..], &key2[..]];

        let scores = attention.attention_scores(&query, &keys);

        // Should sum to 1
        let sum: f32 = scores.iter().sum();
        assert_relative_eq!(sum, 1.0, epsilon = 1e-6);

        // First key matches query better
        assert!(scores[0] > scores[1]);
    }

    #[test]
    fn test_scaled_dot_forward() {
        let attention = ScaledDotAttention::new(2);

        let query = vec![1.0, 0.0];
        let key1 = vec![1.0, 0.0];
        let key2 = vec![0.0, 1.0];
        let value1 = vec![1.0, 2.0, 3.0];
        let value2 = vec![4.0, 5.0, 6.0];

        let keys = vec![&key1[..], &key2[..]];
        let values = vec![&value1[..], &value2[..]];

        let result = attention.forward(&query, &keys, &values);

        // Result should be closer to value1 than value2
        assert_eq!(result.len(), 3);
        assert!(result[0] < 2.5); // Closer to 1.0 than 4.0
    }

    #[test]
    fn test_simd_vs_scalar() {
        let dim = 128;
        let query: Vec<f32> = (0..dim).map(|i| i as f32 / dim as f32).collect();
        let key: Vec<f32> = (0..dim).map(|i| (dim - i) as f32 / dim as f32).collect();

        let simd_attn = ScaledDotAttention::new(dim);
        let scalar_attn = ScaledDotAttention::new(dim).without_simd();

        let keys = vec![&key[..]];

        let simd_score = simd_attn.attention_scores(&query, &keys);
        let scalar_score = scalar_attn.attention_scores(&query, &keys);

        // Results should be identical (or very close)
        assert_relative_eq!(simd_score[0], scalar_score[0], epsilon = 1e-5);
    }

    #[test]
    fn test_scale_factor_effect() {
        let query = vec![1.0, 1.0, 1.0, 1.0];
        let key1 = vec![1.0, 1.0, 1.0, 1.0];
        let key2 = vec![0.5, 0.5, 0.5, 0.5];
        let keys = vec![&key1[..], &key2[..]];

        // Large scale makes distribution more uniform
        let large_scale = ScaledDotAttention::with_scale(0.1);
        let large_scores = large_scale.attention_scores(&query, &keys);

        // Small scale makes distribution more peaked
        let small_scale = ScaledDotAttention::with_scale(2.0);
        let small_scores = small_scale.attention_scores(&query, &keys);

        // Small scale should have more extreme probabilities
        assert!(small_scores[0] > large_scores[0]);
    }

    #[test]
    fn test_empty_keys() {
        let attention = ScaledDotAttention::new(4);
        let query = vec![1.0, 0.0, 0.0, 0.0];
        let keys: Vec<&[f32]> = vec![];

        let scores = attention.attention_scores(&query, &keys);
        assert!(scores.is_empty());
    }

    #[test]
    fn test_single_key() {
        let attention = ScaledDotAttention::new(4);
        let query = vec![1.0, 0.0, 0.0, 0.0];
        let key = vec![0.5, 0.5, 0.0, 0.0];
        let keys = vec![&key[..]];

        let scores = attention.attention_scores(&query, &keys);

        // Single key should get all attention
        assert_eq!(scores.len(), 1);
        assert_relative_eq!(scores[0], 1.0, epsilon = 1e-6);
    }

    #[test]
    fn test_numerical_stability() {
        let attention = ScaledDotAttention::new(4);

        // Very large values
        let query = vec![1000.0, 1000.0, 1000.0, 1000.0];
        let key1 = vec![1000.0, 1000.0, 1000.0, 1000.0];
        let key2 = vec![999.0, 999.0, 999.0, 999.0];
        let keys = vec![&key1[..], &key2[..]];

        let scores = attention.attention_scores(&query, &keys);

        // Should not overflow to NaN or Inf
        assert!(scores.iter().all(|x| x.is_finite()));

        // Should still sum to 1
        let sum: f32 = scores.iter().sum();
        assert_relative_eq!(sum, 1.0, epsilon = 1e-5);
    }
}

#[cfg(feature = "pg_test")]
#[pgrx::pg_schema]
mod pg_tests {
    use super::*;
    use pgrx::prelude::*;

    #[pg_test]
    fn test_pg_scaled_dot_attention() {
        let attention = ScaledDotAttention::new(4);

        let query = vec![1.0, 0.0, 0.0, 0.0];
        let key1 = vec![1.0, 0.0, 0.0, 0.0];
        let key2 = vec![0.0, 1.0, 0.0, 0.0];
        let keys = vec![&key1[..], &key2[..]];

        let scores = attention.attention_scores(&query, &keys);

        assert_eq!(scores.len(), 2);
        assert!(scores[0] > 0.5); // First key matches better
    }

    #[pg_test]
    fn test_pg_attention_forward() {
        let attention = ScaledDotAttention::new(2);

        let query = vec![1.0, 0.0];
        let key = vec![1.0, 0.0];
        let value = vec![5.0, 10.0];

        let keys = vec![&key[..]];
        let values = vec![&value[..]];

        let result = attention.forward(&query, &keys, &values);

        // Should return the value (single key gets all attention)
        assert_eq!(result.len(), 2);
        assert!((result[0] - 5.0).abs() < 0.001);
        assert!((result[1] - 10.0).abs() < 0.001);
    }
}
