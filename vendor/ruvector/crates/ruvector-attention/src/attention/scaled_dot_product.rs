//! Scaled dot-product attention implementation.
//!
//! Implements the fundamental attention mechanism: softmax(QK^T / √d)V

use crate::{
    error::{AttentionError, AttentionResult},
    traits::Attention,
};

/// Scaled dot-product attention: softmax(QK^T / √d)V
///
/// This is the fundamental attention mechanism used in transformers.
/// It computes attention scores by taking the dot product of queries
/// and keys, scaling by the square root of the dimension, applying
/// softmax, and using the result to weight values.
pub struct ScaledDotProductAttention {
    dim: usize,
}

impl ScaledDotProductAttention {
    /// Creates a new scaled dot-product attention mechanism.
    ///
    /// # Arguments
    ///
    /// * `dim` - The embedding dimension
    pub fn new(dim: usize) -> Self {
        Self { dim }
    }

    /// Computes attention scores (before softmax).
    fn compute_scores(&self, query: &[f32], keys: &[&[f32]]) -> Vec<f32> {
        let scale = (self.dim as f32).sqrt();
        keys.iter()
            .map(|key| {
                query
                    .iter()
                    .zip(key.iter())
                    .map(|(q, k)| q * k)
                    .sum::<f32>()
                    / scale
            })
            .collect()
    }

    /// Applies softmax to attention scores.
    fn softmax(&self, scores: &[f32]) -> Vec<f32> {
        let max_score = scores.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        let exp_scores: Vec<f32> = scores.iter().map(|s| (s - max_score).exp()).collect();
        let sum: f32 = exp_scores.iter().sum();
        exp_scores.iter().map(|e| e / sum).collect()
    }
}

impl Attention for ScaledDotProductAttention {
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

        if keys.is_empty() || values.is_empty() {
            return Err(AttentionError::EmptyInput("keys or values".to_string()));
        }

        if keys.len() != values.len() {
            return Err(AttentionError::DimensionMismatch {
                expected: keys.len(),
                actual: values.len(),
            });
        }

        // Compute attention scores
        let scores = self.compute_scores(query, keys);

        // Apply softmax
        let weights = self.softmax(&scores);

        // Weight values
        let mut output = vec![0.0; self.dim];
        for (weight, value) in weights.iter().zip(values.iter()) {
            for (out, val) in output.iter_mut().zip(value.iter()) {
                *out += weight * val;
            }
        }

        Ok(output)
    }

    fn compute_with_mask(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
        mask: Option<&[bool]>,
    ) -> AttentionResult<Vec<f32>> {
        if mask.is_none() {
            return self.compute(query, keys, values);
        }

        let mask = mask.unwrap();
        if mask.len() != keys.len() {
            return Err(AttentionError::InvalidMask {
                expected: format!("{}", keys.len()),
                actual: format!("{}", mask.len()),
            });
        }

        // Compute scores
        let mut scores = self.compute_scores(query, keys);

        // Apply mask (set masked positions to very negative value)
        for (score, &m) in scores.iter_mut().zip(mask.iter()) {
            if !m {
                *score = f32::NEG_INFINITY;
            }
        }

        // Apply softmax
        let weights = self.softmax(&scores);

        // Weight values
        let mut output = vec![0.0; self.dim];
        for (weight, value) in weights.iter().zip(values.iter()) {
            for (out, val) in output.iter_mut().zip(value.iter()) {
                *out += weight * val;
            }
        }

        Ok(output)
    }

    fn dim(&self) -> usize {
        self.dim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scaled_dot_product() {
        let attn = ScaledDotProductAttention::new(4);
        let query = vec![1.0_f32, 0.0, 0.0, 0.0];
        let key1 = vec![1.0_f32, 0.0, 0.0, 0.0];
        let key2 = vec![0.0_f32, 1.0, 0.0, 0.0];
        let val1 = vec![1.0_f32, 2.0, 3.0, 4.0];
        let val2 = vec![5.0_f32, 6.0, 7.0, 8.0];
        let keys = vec![key1.as_slice(), key2.as_slice()];
        let values = vec![val1.as_slice(), val2.as_slice()];

        let result = attn.compute(&query, &keys, &values).unwrap();
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn test_with_mask() {
        let attn = ScaledDotProductAttention::new(4);
        let query = vec![1.0_f32; 4];
        let key1 = vec![1.0_f32; 4];
        let key2 = vec![0.5_f32; 4];
        let val1 = vec![1.0_f32; 4];
        let val2 = vec![2.0_f32; 4];
        let keys = vec![key1.as_slice(), key2.as_slice()];
        let values = vec![val1.as_slice(), val2.as_slice()];
        let mask = vec![true, false];

        let result = attn
            .compute_with_mask(&query, &keys, &values, Some(&mask))
            .unwrap();
        assert_eq!(result.len(), 4);
    }
}
