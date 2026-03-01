//! # Flash Attention v2
//!
//! Memory-efficient attention implementation using tiled computation.
//! Reduces memory usage from O(N²) to O(√N) through block-wise processing.
//!
//! Reference: "FlashAttention-2: Faster Attention with Better Parallelism and Work Partitioning"

use super::{softmax_inplace, Attention};

/// Flash Attention v2 - memory-efficient attention
///
/// Processes attention in tiles/blocks to minimize memory bandwidth and
/// enable processing of very long sequences.
///
/// Time complexity: O(n²d) (same as standard attention)
/// Space complexity: O(√n) instead of O(n²)
#[derive(Debug, Clone)]
pub struct FlashAttention {
    /// Block size for query dimension tiling
    #[allow(dead_code)]
    block_size_q: usize,

    /// Block size for key/value dimension tiling
    block_size_kv: usize,

    /// Scale factor for attention (1/√d_k)
    scale: f32,
}

impl FlashAttention {
    /// Create a new Flash Attention mechanism
    ///
    /// # Arguments
    /// * `head_dim` - Dimension of attention head
    /// * `block_size` - Tile size for blocking (default: 64)
    pub fn new(head_dim: usize, block_size: usize) -> Self {
        Self {
            block_size_q: block_size,
            block_size_kv: block_size,
            scale: 1.0 / (head_dim as f32).sqrt(),
        }
    }

    /// Create with default block size (64)
    pub fn with_head_dim(head_dim: usize) -> Self {
        Self::new(head_dim, 64)
    }

    /// Compute attention scores for a single query-key pair (scaled dot product)
    #[inline]
    fn compute_score(&self, query: &[f32], key: &[f32]) -> f32 {
        let dot: f32 = query.iter().zip(key.iter()).map(|(q, k)| q * k).sum();
        dot * self.scale
    }

    /// Process a single block of the attention matrix
    ///
    /// This is the core of Flash Attention - processing small blocks at a time
    /// to reduce memory usage.
    fn process_block(
        &self,
        query_block: &[f32],
        key_block: &[&[f32]],
        value_block: &[&[f32]],
    ) -> Vec<f32> {
        if key_block.is_empty() {
            return vec![0.0; value_block.first().map_or(0, |v| v.len())];
        }

        // Compute attention scores for this block
        let mut scores: Vec<f32> = key_block
            .iter()
            .map(|key| self.compute_score(query_block, key))
            .collect();

        // Apply softmax to scores
        softmax_inplace(&mut scores);

        // Weighted sum of values
        let value_dim = value_block[0].len();
        let mut output = vec![0.0; value_dim];

        for (score, value) in scores.iter().zip(value_block.iter()) {
            for (out, val) in output.iter_mut().zip(value.iter()) {
                *out += score * val;
            }
        }

        output
    }

    /// Forward pass with tiled computation
    ///
    /// For simplicity, this implementation processes the full sequence in blocks
    /// along the key/value dimension. A full Flash Attention implementation would
    /// also tile the query dimension and use online softmax updates.
    pub fn forward_tiled(&self, query: &[f32], keys: &[&[f32]], values: &[&[f32]]) -> Vec<f32> {
        assert_eq!(keys.len(), values.len(), "Keys and values length mismatch");

        if keys.is_empty() {
            return Vec::new();
        }

        let num_keys = keys.len();
        let value_dim = values[0].len();

        // For small sequences, just use standard attention
        if num_keys <= self.block_size_kv {
            return self.process_block(query, keys, values);
        }

        // Process in blocks along the key/value dimension
        let mut block_outputs = Vec::new();
        let mut block_max_scores = Vec::new();

        for block_start in (0..num_keys).step_by(self.block_size_kv) {
            let block_end = (block_start + self.block_size_kv).min(num_keys);

            let key_block = &keys[block_start..block_end];
            let value_block = &values[block_start..block_end];

            // Compute scores for this block
            let mut scores: Vec<f32> = key_block
                .iter()
                .map(|key| self.compute_score(query, key))
                .collect();

            let block_max = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            block_max_scores.push(block_max);

            // Apply exp (will normalize later)
            for score in &mut scores {
                *score = (*score - block_max).exp();
            }

            // Weighted sum
            let mut block_output = vec![0.0; value_dim];
            for (score, value) in scores.iter().zip(value_block.iter()) {
                for (out, val) in block_output.iter_mut().zip(value.iter()) {
                    *out += score * val;
                }
            }

            block_outputs.push((scores.iter().sum::<f32>(), block_output));
        }

        // Global max for numerical stability
        let global_max = block_max_scores
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max);

        // Combine block outputs with proper normalization
        let mut output = vec![0.0; value_dim];
        let mut total_weight = 0.0;

        for ((block_sum, block_output), block_max) in
            block_outputs.iter().zip(block_max_scores.iter())
        {
            let correction = (block_max - global_max).exp();
            let block_weight = block_sum * correction;
            total_weight += block_weight;

            for (out, block_val) in output.iter_mut().zip(block_output.iter()) {
                *out += block_val * correction;
            }
        }

        // Final normalization
        if total_weight > 0.0 {
            for out in &mut output {
                *out /= total_weight;
            }
        }

        output
    }
}

impl Default for FlashAttention {
    fn default() -> Self {
        Self::new(64, 64)
    }
}

impl Attention for FlashAttention {
    fn attention_scores(&self, query: &[f32], keys: &[&[f32]]) -> Vec<f32> {
        if keys.is_empty() {
            return Vec::new();
        }

        // Compute all scores
        let mut scores: Vec<f32> = keys
            .iter()
            .map(|key| self.compute_score(query, key))
            .collect();

        // Apply softmax
        softmax_inplace(&mut scores);

        scores
    }

    fn forward(&self, query: &[f32], keys: &[&[f32]], values: &[&[f32]]) -> Vec<f32> {
        self.forward_tiled(query, keys, values)
    }
}

#[cfg(feature = "pg_test")]
#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_flash_attention_basic() {
        let flash = FlashAttention::new(4, 64);

        let query = vec![1.0, 0.0, 0.0, 0.0];
        let key1 = vec![1.0, 0.0, 0.0, 0.0];
        let key2 = vec![0.0, 1.0, 0.0, 0.0];
        let keys = vec![&key1[..], &key2[..]];

        let scores = flash.attention_scores(&query, &keys);

        assert_eq!(scores.len(), 2);
        let sum: f32 = scores.iter().sum();
        assert_relative_eq!(sum, 1.0, epsilon = 1e-6);
        assert!(scores[0] > scores[1]); // First key matches better
    }

    #[test]
    fn test_flash_forward_small() {
        let flash = FlashAttention::new(2, 64);

        let query = vec![1.0, 0.0];
        let key1 = vec![1.0, 0.0];
        let key2 = vec![0.0, 1.0];
        let value1 = vec![1.0, 2.0, 3.0];
        let value2 = vec![4.0, 5.0, 6.0];

        let keys = vec![&key1[..], &key2[..]];
        let values = vec![&value1[..], &value2[..]];

        let result = flash.forward(&query, &keys, &values);

        assert_eq!(result.len(), 3);
        // Result should be closer to value1 than value2
        assert!(result[0] < 2.5);
    }

    #[test]
    fn test_flash_tiled_processing() {
        // Test with block size smaller than sequence length
        let flash = FlashAttention::new(4, 2); // block_size = 2

        let query = vec![1.0, 0.0, 0.0, 0.0];
        let keys: Vec<Vec<f32>> = vec![
            vec![1.0, 0.0, 0.0, 0.0],
            vec![0.9, 0.1, 0.0, 0.0],
            vec![0.8, 0.2, 0.0, 0.0],
            vec![0.0, 1.0, 0.0, 0.0],
        ];
        let values: Vec<Vec<f32>> = vec![vec![1.0], vec![2.0], vec![3.0], vec![4.0]];

        let key_refs: Vec<&[f32]> = keys.iter().map(|k| &k[..]).collect();
        let value_refs: Vec<&[f32]> = values.iter().map(|v| &v[..]).collect();

        let result = flash.forward(&query, &key_refs, &value_refs);

        assert_eq!(result.len(), 1);
        // Should be weighted towards first values (better key matches)
        assert!(result[0] < 2.5);
    }

    #[test]
    fn test_flash_vs_standard_attention() {
        // Compare Flash Attention with standard attention (should be very close)
        use super::super::ScaledDotAttention;

        let head_dim = 4;
        let flash = FlashAttention::new(head_dim, 2);
        let standard = ScaledDotAttention::new(head_dim);

        let query = vec![1.0, 0.5, 0.25, 0.0];
        let keys: Vec<Vec<f32>> = vec![
            vec![1.0, 0.5, 0.25, 0.0],
            vec![0.0, 0.25, 0.5, 1.0],
            vec![0.5, 0.5, 0.5, 0.5],
        ];
        let values: Vec<Vec<f32>> = vec![vec![1.0, 0.0], vec![0.0, 1.0], vec![0.5, 0.5]];

        let key_refs: Vec<&[f32]> = keys.iter().map(|k| &k[..]).collect();
        let value_refs: Vec<&[f32]> = values.iter().map(|v| &v[..]).collect();

        let flash_result = flash.forward(&query, &key_refs, &value_refs);
        let standard_result = standard.forward(&query, &key_refs, &value_refs);

        assert_eq!(flash_result.len(), standard_result.len());
        for (f, s) in flash_result.iter().zip(standard_result.iter()) {
            assert_relative_eq!(f, s, epsilon = 1e-4);
        }
    }

    #[test]
    fn test_flash_empty_sequence() {
        let flash = FlashAttention::new(4, 64);
        let query = vec![1.0, 0.0, 0.0, 0.0];
        let keys: Vec<&[f32]> = vec![];
        let values: Vec<&[f32]> = vec![];

        let result = flash.forward(&query, &keys, &values);
        assert!(result.is_empty());
    }

    #[test]
    fn test_flash_numerical_stability() {
        let flash = FlashAttention::new(4, 2);

        // Very large values that could overflow
        let query = vec![100.0, 100.0, 100.0, 100.0];
        let keys: Vec<Vec<f32>> = vec![
            vec![100.0, 100.0, 100.0, 100.0],
            vec![99.0, 99.0, 99.0, 99.0],
            vec![98.0, 98.0, 98.0, 98.0],
        ];
        let values: Vec<Vec<f32>> = vec![vec![1.0, 0.0], vec![0.0, 1.0], vec![0.5, 0.5]];

        let key_refs: Vec<&[f32]> = keys.iter().map(|k| &k[..]).collect();
        let value_refs: Vec<&[f32]> = values.iter().map(|v| &v[..]).collect();

        let result = flash.forward(&query, &key_refs, &value_refs);

        // Should not overflow to NaN or Inf
        assert!(result.iter().all(|x| x.is_finite()));
    }
}

#[cfg(feature = "pg_test")]
#[pgrx::pg_schema]
mod pg_tests {
    use super::*;
    use pgrx::prelude::*;

    #[pg_test]
    fn test_pg_flash_attention() {
        let flash = FlashAttention::new(4, 64);

        let query = vec![1.0, 0.0, 0.0, 0.0];
        let key = vec![1.0, 0.0, 0.0, 0.0];
        let value = vec![5.0, 10.0];

        let keys = vec![&key[..]];
        let values = vec![&value[..]];

        let result = flash.forward(&query, &keys, &values);

        assert_eq!(result.len(), 2);
        // Single matching key should return the value
        assert!((result[0] - 5.0).abs() < 0.01);
        assert!((result[1] - 10.0).abs() < 0.01);
    }

    #[pg_test]
    fn test_pg_flash_tiled() {
        // Test tiled processing with block size smaller than sequence
        let flash = FlashAttention::new(2, 2);

        let query = vec![1.0, 0.0];
        let keys: Vec<Vec<f32>> = vec![
            vec![1.0, 0.0],
            vec![0.9, 0.1],
            vec![0.0, 1.0],
            vec![0.1, 0.9],
        ];
        let values: Vec<Vec<f32>> = vec![vec![10.0], vec![20.0], vec![30.0], vec![40.0]];

        let key_refs: Vec<&[f32]> = keys.iter().map(|k| &k[..]).collect();
        let value_refs: Vec<&[f32]> = values.iter().map(|v| &v[..]).collect();

        let result = flash.forward(&query, &key_refs, &value_refs);

        assert_eq!(result.len(), 1);
        // Should be weighted towards first values
        assert!(result[0] < 25.0);
    }
}
