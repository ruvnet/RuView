//! # Multi-Head Attention
//!
//! Implements multi-head attention mechanism with parallel head computation.
//! Each head learns different attention patterns, enabling the model to
//! attend to information from different representation subspaces.

use super::{Attention, ScaledDotAttention};
use rayon::prelude::*;

/// Multi-head attention mechanism
///
/// Splits the input into multiple heads, computes attention independently
/// for each head in parallel, then concatenates results.
///
/// Time complexity: O(h * n²d/h) = O(n²d) where h=num_heads
/// Space complexity: O(n² * h)
#[derive(Debug, Clone)]
pub struct MultiHeadAttention {
    /// Number of attention heads
    num_heads: usize,

    /// Dimension per head (total_dim / num_heads)
    head_dim: usize,

    /// Total dimension (num_heads * head_dim)
    total_dim: usize,

    /// Attention mechanism for each head
    heads: Vec<ScaledDotAttention>,
}

impl MultiHeadAttention {
    /// Create a new multi-head attention mechanism
    ///
    /// # Arguments
    /// * `num_heads` - Number of parallel attention heads
    /// * `total_dim` - Total embedding dimension (must be divisible by num_heads)
    ///
    /// # Panics
    /// Panics if total_dim is not divisible by num_heads
    pub fn new(num_heads: usize, total_dim: usize) -> Self {
        assert!(num_heads > 0, "Number of heads must be positive");
        assert!(total_dim > 0, "Total dimension must be positive");
        assert_eq!(
            total_dim % num_heads,
            0,
            "Total dimension must be divisible by number of heads"
        );

        let head_dim = total_dim / num_heads;

        // Create attention mechanism for each head
        let heads = (0..num_heads)
            .map(|_| ScaledDotAttention::new(head_dim))
            .collect();

        Self {
            num_heads,
            head_dim,
            total_dim,
            heads,
        }
    }

    /// Get number of heads
    pub fn num_heads(&self) -> usize {
        self.num_heads
    }

    /// Get dimension per head
    pub fn head_dim(&self) -> usize {
        self.head_dim
    }

    /// Split input vector into heads
    ///
    /// # Arguments
    /// * `input` - Input vector [total_dim]
    ///
    /// # Returns
    /// Vec of head vectors, each [head_dim]
    fn split_heads(&self, input: &[f32]) -> Vec<Vec<f32>> {
        assert_eq!(
            input.len(),
            self.total_dim,
            "Input dimension mismatch: expected {}, got {}",
            self.total_dim,
            input.len()
        );

        (0..self.num_heads)
            .map(|h| {
                let start = h * self.head_dim;
                let end = start + self.head_dim;
                input[start..end].to_vec()
            })
            .collect()
    }

    /// Concatenate head outputs back into single vector
    ///
    /// # Arguments
    /// * `heads` - Vec of head outputs, each [head_dim]
    ///
    /// # Returns
    /// Concatenated vector [total_dim]
    fn concat_heads(&self, heads: &[Vec<f32>]) -> Vec<f32> {
        assert_eq!(heads.len(), self.num_heads, "Wrong number of heads");

        let mut result = Vec::with_capacity(self.total_dim);
        for head in heads {
            assert_eq!(head.len(), self.head_dim, "Wrong head dimension");
            result.extend_from_slice(head);
        }

        result
    }

    /// Compute attention for all heads in parallel
    ///
    /// # Arguments
    /// * `query` - Query vector [total_dim]
    /// * `keys` - Key vectors, each [total_dim]
    /// * `values` - Value vectors, each [total_dim]
    ///
    /// # Returns
    /// Multi-head attention output [total_dim]
    pub fn forward(&self, query: &[f32], keys: &[&[f32]], values: &[&[f32]]) -> Vec<f32> {
        assert_eq!(keys.len(), values.len(), "Keys and values length mismatch");

        if keys.is_empty() {
            return vec![0.0; self.total_dim];
        }

        // Split query into heads
        let q_heads = self.split_heads(query);

        // Split keys into heads
        let k_heads: Vec<Vec<Vec<f32>>> = keys.iter().map(|key| self.split_heads(key)).collect();

        // Split values into heads
        let v_heads: Vec<Vec<Vec<f32>>> =
            values.iter().map(|value| self.split_heads(value)).collect();

        // Process each head in parallel
        let head_outputs: Vec<Vec<f32>> = (0..self.num_heads)
            .into_par_iter()
            .map(|h| {
                // Extract keys and values for this head
                let head_keys: Vec<&[f32]> = k_heads.iter().map(|k| &k[h][..]).collect();
                let head_values: Vec<&[f32]> = v_heads.iter().map(|v| &v[h][..]).collect();

                // Compute attention for this head
                self.heads[h].forward(&q_heads[h], &head_keys, &head_values)
            })
            .collect();

        // Concatenate head outputs
        self.concat_heads(&head_outputs)
    }

    /// Compute attention scores for all heads (without applying to values)
    ///
    /// # Returns
    /// Vec of score vectors, one per head
    pub fn attention_scores_all_heads(&self, query: &[f32], keys: &[&[f32]]) -> Vec<Vec<f32>> {
        let q_heads = self.split_heads(query);

        let k_heads: Vec<Vec<Vec<f32>>> = keys.iter().map(|key| self.split_heads(key)).collect();

        (0..self.num_heads)
            .into_par_iter()
            .map(|h| {
                let head_keys: Vec<&[f32]> = k_heads.iter().map(|k| &k[h][..]).collect();
                self.heads[h].attention_scores(&q_heads[h], &head_keys)
            })
            .collect()
    }
}

impl Attention for MultiHeadAttention {
    /// Compute averaged attention scores across all heads
    fn attention_scores(&self, query: &[f32], keys: &[&[f32]]) -> Vec<f32> {
        let all_scores = self.attention_scores_all_heads(query, keys);

        if all_scores.is_empty() || all_scores[0].is_empty() {
            return Vec::new();
        }

        // Average scores across heads
        let num_keys = all_scores[0].len();
        let mut avg_scores = vec![0.0; num_keys];

        for head_scores in &all_scores {
            for (avg, score) in avg_scores.iter_mut().zip(head_scores.iter()) {
                *avg += score;
            }
        }

        let num_heads_f32 = self.num_heads as f32;
        for score in &mut avg_scores {
            *score /= num_heads_f32;
        }

        avg_scores
    }

    fn forward(&self, query: &[f32], keys: &[&[f32]], values: &[&[f32]]) -> Vec<f32> {
        self.forward(query, keys, values)
    }
}

#[cfg(feature = "pg_test")]
#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_multi_head_basic() {
        let mha = MultiHeadAttention::new(4, 8);

        assert_eq!(mha.num_heads(), 4);
        assert_eq!(mha.head_dim(), 2);
    }

    #[test]
    fn test_split_concat_heads() {
        let mha = MultiHeadAttention::new(4, 8);
        let input = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];

        let split = mha.split_heads(&input);
        assert_eq!(split.len(), 4);
        assert_eq!(split[0], vec![1.0, 2.0]);
        assert_eq!(split[1], vec![3.0, 4.0]);
        assert_eq!(split[2], vec![5.0, 6.0]);
        assert_eq!(split[3], vec![7.0, 8.0]);

        let concat = mha.concat_heads(&split);
        assert_eq!(concat, input);
    }

    #[test]
    fn test_multi_head_forward() {
        let mha = MultiHeadAttention::new(2, 4);

        let query = vec![1.0, 0.0, 0.0, 1.0];
        let key1 = vec![1.0, 0.0, 0.0, 1.0];
        let key2 = vec![0.0, 1.0, 1.0, 0.0];
        let value1 = vec![1.0, 1.0, 1.0, 1.0];
        let value2 = vec![2.0, 2.0, 2.0, 2.0];

        let keys = vec![&key1[..], &key2[..]];
        let values = vec![&value1[..], &value2[..]];

        let result = mha.forward(&query, &keys, &values);

        assert_eq!(result.len(), 4);
        // Result should be weighted combination of values
        assert!(result.iter().all(|&x| x >= 1.0 && x <= 2.0));
    }

    #[test]
    fn test_multi_head_attention_scores() {
        let mha = MultiHeadAttention::new(2, 4);

        let query = vec![1.0, 0.0, 0.0, 1.0];
        let key1 = vec![1.0, 0.0, 0.0, 1.0];
        let key2 = vec![0.0, 1.0, 1.0, 0.0];
        let keys = vec![&key1[..], &key2[..]];

        let scores = mha.attention_scores(&query, &keys);

        assert_eq!(scores.len(), 2);
        // Scores should sum to 1 (averaged across heads)
        let sum: f32 = scores.iter().sum();
        assert_relative_eq!(sum, 1.0, epsilon = 1e-5);
    }

    #[test]
    fn test_multi_head_all_scores() {
        let mha = MultiHeadAttention::new(2, 4);

        let query = vec![1.0, 0.0, 0.0, 1.0];
        let key = vec![1.0, 0.0, 0.0, 1.0];
        let keys = vec![&key[..]];

        let all_scores = mha.attention_scores_all_heads(&query, &keys);

        assert_eq!(all_scores.len(), 2); // One per head
        assert_eq!(all_scores[0].len(), 1); // One key
        assert_eq!(all_scores[1].len(), 1);
    }

    #[test]
    #[should_panic(expected = "Total dimension must be divisible by number of heads")]
    fn test_invalid_dimensions() {
        MultiHeadAttention::new(3, 8); // 8 is not divisible by 3
    }

    #[test]
    fn test_parallel_computation() {
        // Test with larger dimensions to ensure parallelism works
        let mha = MultiHeadAttention::new(8, 64);

        let query: Vec<f32> = (0..64).map(|i| i as f32 / 64.0).collect();
        let key1: Vec<f32> = (0..64).map(|i| (i + 1) as f32 / 64.0).collect();
        let key2: Vec<f32> = (0..64).map(|i| (63 - i) as f32 / 64.0).collect();
        let value1 = vec![1.0; 64];
        let value2 = vec![2.0; 64];

        let keys = vec![&key1[..], &key2[..]];
        let values = vec![&value1[..], &value2[..]];

        let result = mha.forward(&query, &keys, &values);

        assert_eq!(result.len(), 64);
        assert!(result.iter().all(|x| x.is_finite()));
    }
}

#[cfg(feature = "pg_test")]
#[pgrx::pg_schema]
mod pg_tests {
    use super::*;
    use pgrx::prelude::*;

    #[pg_test]
    fn test_pg_multi_head_attention() {
        let mha = MultiHeadAttention::new(4, 8);

        let query = vec![1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0];
        let key = vec![1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0];
        let value = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];

        let keys = vec![&key[..]];
        let values = vec![&value[..]];

        let result = mha.forward(&query, &keys, &values);

        assert_eq!(result.len(), 8);
        // Single matching key should return the value
        for (r, v) in result.iter().zip(value.iter()) {
            assert!((r - v).abs() < 0.01);
        }
    }

    #[pg_test]
    fn test_pg_multi_head_multiple_keys() {
        let mha = MultiHeadAttention::new(2, 4);

        let query = vec![1.0, 0.0, 0.0, 1.0];
        let key1 = vec![1.0, 0.0, 0.0, 1.0];
        let key2 = vec![0.0, 1.0, 1.0, 0.0];
        let value1 = vec![10.0, 10.0, 10.0, 10.0];
        let value2 = vec![20.0, 20.0, 20.0, 20.0];

        let keys = vec![&key1[..], &key2[..]];
        let values = vec![&value1[..], &value2[..]];

        let result = mha.forward(&query, &keys, &values);

        assert_eq!(result.len(), 4);
        // Should be weighted average of values
        assert!(result[0] >= 10.0 && result[0] <= 20.0);
    }
}
