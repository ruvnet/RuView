//! Multi-head attention implementation.
//!
//! Implements parallel attention heads for diverse representation learning.

use crate::{
    error::{AttentionError, AttentionResult},
    traits::Attention,
};

use super::scaled_dot_product::ScaledDotProductAttention;

/// Multi-head attention mechanism.
///
/// Splits the input into multiple heads, applies attention in parallel,
/// and concatenates the results. This allows the model to attend to
/// different representation subspaces simultaneously.
pub struct MultiHeadAttention {
    dim: usize,
    num_heads: usize,
    head_dim: usize,
}

impl MultiHeadAttention {
    /// Creates a new multi-head attention mechanism.
    ///
    /// # Arguments
    ///
    /// * `dim` - The embedding dimension
    /// * `num_heads` - Number of attention heads
    ///
    /// # Panics
    ///
    /// Panics if `dim` is not divisible by `num_heads`.
    pub fn new(dim: usize, num_heads: usize) -> Self {
        assert!(
            dim % num_heads == 0,
            "Dimension {} must be divisible by number of heads {}",
            dim,
            num_heads
        );

        Self {
            dim,
            num_heads,
            head_dim: dim / num_heads,
        }
    }

    /// Splits input into multiple heads.
    fn split_heads(&self, input: &[f32]) -> Vec<Vec<f32>> {
        (0..self.num_heads)
            .map(|h| {
                let start = h * self.head_dim;
                let end = start + self.head_dim;
                input[start..end].to_vec()
            })
            .collect()
    }

    /// Concatenates outputs from multiple heads.
    fn concat_heads(&self, heads: Vec<Vec<f32>>) -> Vec<f32> {
        heads.into_iter().flatten().collect()
    }
}

impl Attention for MultiHeadAttention {
    fn compute(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
    ) -> AttentionResult<Vec<f32>> {
        if query.len() != self.dim {
            return Err(AttentionError::DimensionMismatch {
                expected: self.dim,
                actual: query.len(),
            });
        }

        // Split query into heads
        let query_heads = self.split_heads(query);

        // Split keys and values
        let key_heads: Vec<Vec<Vec<f32>>> = keys.iter().map(|k| self.split_heads(k)).collect();

        let value_heads: Vec<Vec<Vec<f32>>> = values.iter().map(|v| self.split_heads(v)).collect();

        // Compute attention for each head
        let mut head_outputs = Vec::new();
        for h in 0..self.num_heads {
            let head_attn = ScaledDotProductAttention::new(self.head_dim);

            let head_keys: Vec<&[f32]> = key_heads.iter().map(|kh| kh[h].as_slice()).collect();

            let head_values: Vec<&[f32]> = value_heads.iter().map(|vh| vh[h].as_slice()).collect();

            let head_out = head_attn.compute(&query_heads[h], &head_keys, &head_values)?;
            head_outputs.push(head_out);
        }

        // Concatenate head outputs
        Ok(self.concat_heads(head_outputs))
    }

    fn compute_with_mask(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
        _mask: Option<&[bool]>,
    ) -> AttentionResult<Vec<f32>> {
        // For simplicity, delegate to compute (mask handling can be added per-head)
        self.compute(query, keys, values)
    }

    fn dim(&self) -> usize {
        self.dim
    }

    fn num_heads(&self) -> usize {
        self.num_heads
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_head() {
        let attn = MultiHeadAttention::new(8, 2);
        let query = vec![1.0_f32; 8];
        let key1 = vec![0.5_f32; 8];
        let key2 = vec![0.3_f32; 8];
        let val1 = vec![1.0_f32; 8];
        let val2 = vec![2.0_f32; 8];
        let keys = vec![key1.as_slice(), key2.as_slice()];
        let values = vec![val1.as_slice(), val2.as_slice()];

        let result = attn.compute(&query, &keys, &values).unwrap();
        assert_eq!(result.len(), 8);
    }

    #[test]
    #[should_panic(expected = "divisible")]
    fn test_invalid_heads() {
        MultiHeadAttention::new(10, 3);
    }
}
