//! Local-Global attention for efficient long-range dependencies
//!
//! Complexity: O(n * (w + g)) where w = window size, g = global tokens

use crate::error::{AttentionError, AttentionResult};
use crate::traits::Attention;
use crate::utils::stable_softmax;

/// Local-Global attention mechanism
///
/// Combines local windowed attention with global tokens for O(n*(w+g)) complexity.
pub struct LocalGlobalAttention {
    dim: usize,
    local_window: usize,
    num_global_tokens: usize,
    scale: f32,
}

impl LocalGlobalAttention {
    /// Create new local-global attention
    pub fn new(dim: usize, local_window: usize, num_global_tokens: usize) -> Self {
        Self {
            dim,
            local_window,
            num_global_tokens,
            scale: 1.0 / (dim as f32).sqrt(),
        }
    }

    /// Compute attention scores for local window
    fn compute_local_scores(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        position: usize,
    ) -> Vec<(usize, f32)> {
        let n = keys.len();
        let half_window = self.local_window / 2;
        let start = position.saturating_sub(half_window);
        let end = (position + half_window + 1).min(n);

        (start..end)
            .map(|j| {
                let score: f32 = query
                    .iter()
                    .zip(keys[j].iter())
                    .map(|(q, k)| q * k)
                    .sum::<f32>()
                    * self.scale;
                (j, score)
            })
            .collect()
    }

    /// Compute attention scores for global tokens
    fn compute_global_scores(&self, query: &[f32], keys: &[&[f32]]) -> Vec<(usize, f32)> {
        let num_global = self.num_global_tokens.min(keys.len());

        (0..num_global)
            .map(|j| {
                let score: f32 = query
                    .iter()
                    .zip(keys[j].iter())
                    .map(|(q, k)| q * k)
                    .sum::<f32>()
                    * self.scale;
                (j, score)
            })
            .collect()
    }
}

impl Attention for LocalGlobalAttention {
    fn compute(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
    ) -> AttentionResult<Vec<f32>> {
        if keys.is_empty() {
            return Err(AttentionError::InvalidConfig("Empty keys".to_string()));
        }
        if keys.len() != values.len() {
            return Err(AttentionError::DimensionMismatch {
                expected: keys.len(),
                actual: values.len(),
            });
        }
        if query.len() != self.dim {
            return Err(AttentionError::DimensionMismatch {
                expected: self.dim,
                actual: query.len(),
            });
        }

        // For simplicity, compute at position 0 (middle of sequence would be typical)
        let position = keys.len() / 2;

        // Collect all attended positions and scores
        let mut attended: Vec<(usize, f32)> = Vec::new();

        // Add global scores
        attended.extend(self.compute_global_scores(query, keys));

        // Add local scores
        for (idx, score) in self.compute_local_scores(query, keys, position) {
            if !attended.iter().any(|(i, _)| *i == idx) {
                attended.push((idx, score));
            }
        }

        if attended.is_empty() {
            return Err(AttentionError::ComputationError(
                "No attended positions".to_string(),
            ));
        }

        // Softmax over attended positions
        let scores: Vec<f32> = attended.iter().map(|(_, s)| *s).collect();
        let weights = stable_softmax(&scores);

        // Weighted sum of values
        let mut output = vec![0.0f32; self.dim];
        for ((idx, _), weight) in attended.iter().zip(weights.iter()) {
            for (o, v) in output.iter_mut().zip(values[*idx].iter()) {
                *o += weight * v;
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
        if let Some(m) = mask {
            let filtered: Vec<(usize, bool)> = m
                .iter()
                .copied()
                .enumerate()
                .filter(|(_, keep)| *keep)
                .collect();
            let filtered_keys: Vec<&[f32]> = filtered.iter().map(|(i, _)| keys[*i]).collect();
            let filtered_values: Vec<&[f32]> = filtered.iter().map(|(i, _)| values[*i]).collect();
            self.compute(query, &filtered_keys, &filtered_values)
        } else {
            self.compute(query, keys, values)
        }
    }

    fn dim(&self) -> usize {
        self.dim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_global_attention() {
        let attention = LocalGlobalAttention::new(64, 8, 2);

        let query = vec![0.5; 64];
        let keys: Vec<Vec<f32>> = (0..100).map(|_| vec![0.3; 64]).collect();
        let values: Vec<Vec<f32>> = (0..100).map(|i| vec![i as f32; 64]).collect();

        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

        let result = attention.compute(&query, &keys_refs, &values_refs).unwrap();
        assert_eq!(result.len(), 64);
    }

    #[test]
    fn test_small_sequence() {
        let attention = LocalGlobalAttention::new(32, 4, 1);

        let query = vec![1.0; 32];
        let keys = vec![vec![0.5; 32]; 5];
        let values = vec![vec![1.0; 32]; 5];

        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

        let result = attention.compute(&query, &keys_refs, &values_refs).unwrap();
        assert_eq!(result.len(), 32);
    }
}
