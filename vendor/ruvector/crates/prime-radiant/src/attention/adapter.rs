//! Adapter to ruvector-attention
//!
//! Wraps attention mechanisms for coherence computation.

use super::{AttentionCoherenceConfig, AttentionError, Result};

/// Adapter wrapping ruvector-attention functionality
#[derive(Debug)]
pub struct AttentionAdapter {
    /// Configuration
    config: AttentionCoherenceConfig,
}

impl AttentionAdapter {
    /// Create a new adapter
    pub fn new(config: AttentionCoherenceConfig) -> Self {
        Self { config }
    }

    /// Compute attention scores for node states
    ///
    /// Returns a vector of attention scores (one per node).
    pub fn compute_scores(&self, node_states: &[&[f32]]) -> Result<Vec<f32>> {
        if node_states.is_empty() {
            return Err(AttentionError::EmptyInput("node_states".to_string()));
        }

        let n = node_states.len();

        // Validate dimensions
        let dim = node_states[0].len();
        for (i, state) in node_states.iter().enumerate() {
            if state.len() != dim {
                return Err(AttentionError::DimensionMismatch {
                    expected: dim,
                    actual: state.len(),
                });
            }
        }

        // Compute pairwise similarities
        let mut similarity_matrix = vec![vec![0.0f32; n]; n];
        for i in 0..n {
            for j in 0..n {
                if i != j {
                    similarity_matrix[i][j] =
                        self.cosine_similarity(node_states[i], node_states[j]);
                }
            }
        }

        // Compute attention scores as normalized sum of similarities
        let mut scores = Vec::with_capacity(n);
        for i in 0..n {
            let sum: f32 = similarity_matrix[i].iter().sum();
            let avg = sum / (n - 1).max(1) as f32;
            // Normalize to [0, 1]
            let normalized = (avg + 1.0) / 2.0; // cosine is in [-1, 1]
            scores.push(normalized.clamp(0.0, 1.0));
        }

        Ok(scores)
    }

    /// Compute attention over query and keys
    pub fn compute_attention(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
    ) -> Result<Vec<f32>> {
        if keys.is_empty() || values.is_empty() {
            return Err(AttentionError::EmptyInput("keys/values".to_string()));
        }

        if keys.len() != values.len() {
            return Err(AttentionError::InvalidConfig(
                "keys and values must have same length".to_string(),
            ));
        }

        let dim = query.len();

        // Compute scaled dot-product attention
        let scale = 1.0 / (dim as f32).sqrt();

        let logits: Vec<f32> = keys
            .iter()
            .map(|k| self.dot_product(query, k) * scale / self.config.temperature)
            .collect();

        let weights = self.stable_softmax(&logits);

        // Weighted sum of values
        self.weighted_sum(&weights, values)
    }

    /// Compute sparse attention (top-k)
    pub fn compute_sparse_attention(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
        k: usize,
    ) -> Result<Vec<f32>> {
        if keys.is_empty() || values.is_empty() {
            return Err(AttentionError::EmptyInput("keys/values".to_string()));
        }

        let k = k.min(keys.len());
        let dim = query.len();
        let scale = 1.0 / (dim as f32).sqrt();

        // Get top-k scores
        let mut scores: Vec<(usize, f32)> = keys
            .iter()
            .enumerate()
            .map(|(i, k)| (i, self.dot_product(query, k) * scale))
            .collect();

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let top_k: Vec<(usize, f32)> = scores.into_iter().take(k).collect();

        // Compute attention over selected
        let logits: Vec<f32> = top_k
            .iter()
            .map(|(_, s)| s / self.config.temperature)
            .collect();

        let weights = self.stable_softmax(&logits);

        let selected_values: Vec<&[f32]> = top_k.iter().map(|(i, _)| values[*i]).collect();

        self.weighted_sum(&weights, &selected_values)
    }

    // === Helper methods ===

    fn dot_product(&self, a: &[f32], b: &[f32]) -> f32 {
        let len = a.len().min(b.len());
        let mut sum = 0.0f32;

        // Unrolled for performance
        let chunks = len / 4;
        let remainder = len % 4;

        for i in 0..chunks {
            let base = i * 4;
            sum += a[base] * b[base];
            sum += a[base + 1] * b[base + 1];
            sum += a[base + 2] * b[base + 2];
            sum += a[base + 3] * b[base + 3];
        }

        let base = chunks * 4;
        for i in 0..remainder {
            sum += a[base + i] * b[base + i];
        }

        sum
    }

    fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        let dot = self.dot_product(a, b);
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a < 1e-10 || norm_b < 1e-10 {
            return 0.0;
        }

        (dot / (norm_a * norm_b)).clamp(-1.0, 1.0)
    }

    fn stable_softmax(&self, logits: &[f32]) -> Vec<f32> {
        if logits.is_empty() {
            return vec![];
        }

        let max_logit = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_logits: Vec<f32> = logits.iter().map(|&l| (l - max_logit).exp()).collect();
        let sum: f32 = exp_logits.iter().sum();

        if sum > 0.0 {
            exp_logits.iter().map(|&e| e / sum).collect()
        } else {
            // Fallback to uniform
            vec![1.0 / logits.len() as f32; logits.len()]
        }
    }

    fn weighted_sum(&self, weights: &[f32], values: &[&[f32]]) -> Result<Vec<f32>> {
        if weights.is_empty() || values.is_empty() {
            return Err(AttentionError::EmptyInput("weights/values".to_string()));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_scores() {
        let config = AttentionCoherenceConfig::default();
        let adapter = AttentionAdapter::new(config);

        let states: Vec<Vec<f32>> = (0..5).map(|i| vec![0.1 * (i + 1) as f32; 16]).collect();
        let state_refs: Vec<&[f32]> = states.iter().map(|s| s.as_slice()).collect();

        let scores = adapter.compute_scores(&state_refs).unwrap();

        assert_eq!(scores.len(), 5);
        for score in &scores {
            assert!(*score >= 0.0 && *score <= 1.0);
        }
    }

    #[test]
    fn test_compute_attention() {
        let config = AttentionCoherenceConfig::default();
        let adapter = AttentionAdapter::new(config);

        let query = vec![0.5f32; 16];
        let keys: Vec<Vec<f32>> = (0..10).map(|i| vec![0.1 * (i + 1) as f32; 16]).collect();
        let values: Vec<Vec<f32>> = (0..10).map(|i| vec![i as f32; 16]).collect();

        let key_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let value_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

        let output = adapter
            .compute_attention(&query, &key_refs, &value_refs)
            .unwrap();

        assert_eq!(output.len(), 16);
    }

    #[test]
    fn test_sparse_attention() {
        let config = AttentionCoherenceConfig::default();
        let adapter = AttentionAdapter::new(config);

        let query = vec![0.5f32; 16];
        let keys: Vec<Vec<f32>> = (0..20).map(|i| vec![0.1 * (i + 1) as f32; 16]).collect();
        let values: Vec<Vec<f32>> = (0..20).map(|i| vec![i as f32; 16]).collect();

        let key_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let value_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

        let output = adapter
            .compute_sparse_attention(&query, &key_refs, &value_refs, 5)
            .unwrap();

        assert_eq!(output.len(), 16);
    }

    #[test]
    fn test_cosine_similarity() {
        let config = AttentionCoherenceConfig::default();
        let adapter = AttentionAdapter::new(config);

        let a = vec![1.0, 0.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0, 0.0];
        let c = vec![-1.0, 0.0, 0.0, 0.0];

        assert!((adapter.cosine_similarity(&a, &b) - 1.0).abs() < 0.01);
        assert!((adapter.cosine_similarity(&a, &c) + 1.0).abs() < 0.01);
    }
}
