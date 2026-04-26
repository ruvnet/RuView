//! Stage 2: Attention-based BSSID weighting.
//!
//! Uses scaled dot-product attention to learn which BSSIDs respond
//! most to body movement. High-variance BSSIDs on body-affected
//! paths get higher attention weights.
//!
//! When the `pipeline` feature is enabled, this uses
//! `ruvector_attention::ScaledDotProductAttention` for the core
//! attention computation. Otherwise, it falls back to a pure-Rust
//! softmax implementation.

/// Weights BSSIDs by body-sensitivity using attention mechanism.
pub struct AttentionWeighter {
    dim: usize,
}

impl AttentionWeighter {
    /// Create a new attention weighter.
    ///
    /// - `dim`: dimensionality of the attention space (typically 1 for scalar RSSI).
    #[must_use]
    pub fn new(dim: usize) -> Self {
        Self { dim }
    }

    /// Compute attention-weighted output from BSSID residuals.
    ///
    /// - `query`: the aggregated variance profile (1 x dim).
    /// - `keys`: per-BSSID residual vectors (`n_bssids` x dim).
    /// - `values`: per-BSSID amplitude vectors (`n_bssids` x dim).
    ///
    /// Returns the weighted amplitude vector and per-BSSID weights.
    #[must_use]
    pub fn weight(
        &self,
        query: &[f32],
        keys: &[Vec<f32>],
        values: &[Vec<f32>],
    ) -> (Vec<f32>, Vec<f32>) {
        if keys.is_empty() || values.is_empty() {
            return (vec![0.0; self.dim], vec![]);
        }

        // Compute per-BSSID attention scores (softmax of q·k / sqrt(d))
        let scores = self.compute_scores(query, keys);

        // Weighted sum of values
        let mut weighted = vec![0.0f32; self.dim];
        for (i, score) in scores.iter().enumerate() {
            if let Some(val) = values.get(i) {
                for (d, v) in weighted.iter_mut().zip(val.iter()) {
                    *d += score * v;
                }
            }
        }

        (weighted, scores)
    }

    /// Compute raw attention scores (softmax of q*k / sqrt(d)).
    #[allow(clippy::cast_precision_loss)]
    fn compute_scores(&self, query: &[f32], keys: &[Vec<f32>]) -> Vec<f32> {
        let scale = (self.dim as f32).sqrt();
        let mut scores: Vec<f32> = keys
            .iter()
            .map(|key| {
                let dot: f32 = query.iter().zip(key.iter()).map(|(q, k)| q * k).sum();
                dot / scale
            })
            .collect();

        // Softmax
        let max_score = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let sum_exp: f32 = scores.iter().map(|&s| (s - max_score).exp()).sum();
        for s in &mut scores {
            *s = (*s - max_score).exp() / sum_exp;
        }
        scores
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_returns_zero() {
        let weighter = AttentionWeighter::new(1);
        let (output, scores) = weighter.weight(&[0.0], &[], &[]);
        assert_eq!(output, vec![0.0]);
        assert!(scores.is_empty());
    }

    #[test]
    fn single_bssid_gets_full_weight() {
        let weighter = AttentionWeighter::new(1);
        let query = vec![1.0];
        let keys = vec![vec![1.0]];
        let values = vec![vec![5.0]];
        let (output, scores) = weighter.weight(&query, &keys, &values);
        assert!((scores[0] - 1.0).abs() < 1e-5, "single BSSID should have weight 1.0");
        assert!((output[0] - 5.0).abs() < 1e-3, "output should equal the single value");
    }

    #[test]
    fn higher_residual_gets_more_weight() {
        let weighter = AttentionWeighter::new(1);
        let query = vec![1.0];
        // BSSID 0 has low residual, BSSID 1 has high residual
        let keys = vec![vec![0.1], vec![10.0]];
        let values = vec![vec![1.0], vec![1.0]];
        let (_output, scores) = weighter.weight(&query, &keys, &values);
        assert!(
            scores[1] > scores[0],
            "high-residual BSSID should get higher weight: {scores:?}"
        );
    }

    #[test]
    fn scores_sum_to_one() {
        let weighter = AttentionWeighter::new(1);
        let query = vec![1.0];
        let keys = vec![vec![0.5], vec![1.0], vec![2.0]];
        let values = vec![vec![1.0], vec![2.0], vec![3.0]];
        let (_output, scores) = weighter.weight(&query, &keys, &values);
        let sum: f32 = scores.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5, "scores should sum to 1.0, got {sum}");
    }
}
