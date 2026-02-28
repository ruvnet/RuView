//! Softmax-weighted retrieval mechanism
//!
//! This module implements the attention-based retrieval mechanism
//! that is mathematically equivalent to transformer attention.

/// Compute dot product between two vectors
#[inline]
fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Compute softmax with temperature scaling
///
/// Implements: softmax(x * β) = exp(x_i * β) / Σ exp(x_j * β)
///
/// # Arguments
///
/// * `values` - Input values
/// * `beta` - Temperature parameter (inverse temperature)
///
/// # Returns
///
/// Softmax probabilities that sum to 1.0
///
/// # Examples
///
/// ```rust
/// use ruvector_nervous_system::hopfield::softmax;
///
/// let values = vec![1.0, 2.0, 3.0];
/// let probs = softmax(&values, 1.0);
///
/// // Probabilities sum to 1.0
/// let sum: f32 = probs.iter().sum();
/// assert!((sum - 1.0).abs() < 1e-6);
/// ```
pub fn softmax(values: &[f32], beta: f32) -> Vec<f32> {
    if values.is_empty() {
        return Vec::new();
    }

    // Find max for numerical stability
    let max_val = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);

    // Compute exp(x * β - max * β) for stability
    let exp_values: Vec<f32> = values
        .iter()
        .map(|&x| ((x - max_val) * beta).exp())
        .collect();

    let sum: f32 = exp_values.iter().sum();

    // Normalize (guard against division by zero from underflow)
    if sum <= f32::EPSILON {
        // Uniform distribution fallback
        let n = exp_values.len() as f32;
        return vec![1.0 / n; exp_values.len()];
    }
    exp_values.iter().map(|&x| x / sum).collect()
}

/// Compute attention weights and similarities for retrieval
///
/// Implements the transformer-style attention mechanism:
/// 1. Compute similarities: s_i = pattern_i · query
/// 2. Apply softmax: α = softmax(β * s)
///
/// # Arguments
///
/// * `patterns` - Stored patterns (N × d matrix)
/// * `query` - Query vector (d-dimensional)
/// * `beta` - Inverse temperature parameter
///
/// # Returns
///
/// Tuple of (attention_weights, similarities)
///
/// # Examples
///
/// ```rust
/// use ruvector_nervous_system::hopfield::compute_attention;
///
/// let patterns = vec![
///     vec![1.0, 0.0, 0.0],
///     vec![0.0, 1.0, 0.0],
/// ];
/// let query = vec![1.0, 0.0, 0.0];
/// let (attention, similarities) = compute_attention(&patterns, &query, 1.0);
///
/// // First pattern should have highest attention
/// assert!(attention[0] > attention[1]);
/// ```
pub fn compute_attention(patterns: &[Vec<f32>], query: &[f32], beta: f32) -> (Vec<f32>, Vec<f32>) {
    // Compute similarities: s_i = patterns[i] · query
    let similarities: Vec<f32> = patterns
        .iter()
        .map(|pattern| dot_product(pattern, query))
        .collect();

    // Apply softmax with temperature
    let attention = softmax(&similarities, beta);

    (attention, similarities)
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_dot_product() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];

        let result = dot_product(&a, &b);
        assert_relative_eq!(result, 32.0, epsilon = 1e-6);
    }

    #[test]
    fn test_dot_product_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];

        let result = dot_product(&a, &b);
        assert_relative_eq!(result, 0.0, epsilon = 1e-6);
    }

    #[test]
    fn test_softmax_uniform() {
        let values = vec![1.0, 1.0, 1.0];
        let probs = softmax(&values, 1.0);

        // All probabilities should be equal
        for &p in &probs {
            assert_relative_eq!(p, 1.0 / 3.0, epsilon = 1e-6);
        }
    }

    #[test]
    fn test_softmax_sums_to_one() {
        let values = vec![0.5, 1.0, 1.5, 2.0];
        let probs = softmax(&values, 1.0);

        let sum: f32 = probs.iter().sum();
        assert_relative_eq!(sum, 1.0, epsilon = 1e-6);
    }

    #[test]
    fn test_softmax_temperature_effect() {
        let values = vec![1.0, 2.0];

        // Low temperature (β = 0.5) - more uniform
        let probs_low = softmax(&values, 0.5);

        // High temperature (β = 5.0) - sharper
        let probs_high = softmax(&values, 5.0);

        // High temp should give more weight to larger value
        assert!(probs_high[1] > probs_low[1]);
    }

    #[test]
    fn test_softmax_empty() {
        let values: Vec<f32> = Vec::new();
        let probs = softmax(&values, 1.0);

        assert!(probs.is_empty());
    }

    #[test]
    fn test_softmax_numerical_stability() {
        // Large values that could cause overflow
        let values = vec![1000.0, 1001.0, 1002.0];
        let probs = softmax(&values, 1.0);

        // Should still sum to 1.0
        let sum: f32 = probs.iter().sum();
        assert_relative_eq!(sum, 1.0, epsilon = 1e-5);
    }

    #[test]
    fn test_compute_attention_orthogonal_patterns() {
        let patterns = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
        ];
        let query = vec![1.0, 0.0, 0.0];

        let (attention, similarities) = compute_attention(&patterns, &query, 1.0);

        // First pattern matches query
        assert_relative_eq!(similarities[0], 1.0, epsilon = 1e-6);
        assert_relative_eq!(similarities[1], 0.0, epsilon = 1e-6);
        assert_relative_eq!(similarities[2], 0.0, epsilon = 1e-6);

        // First pattern should have highest attention
        assert!(attention[0] > attention[1]);
        assert!(attention[0] > attention[2]);
    }

    #[test]
    fn test_compute_attention_identical_patterns() {
        let patterns = vec![vec![1.0, 1.0, 1.0], vec![1.0, 1.0, 1.0]];
        let query = vec![1.0, 1.0, 1.0];

        let (attention, similarities) = compute_attention(&patterns, &query, 1.0);

        // Identical patterns and query
        assert_relative_eq!(similarities[0], 3.0, epsilon = 1e-6);
        assert_relative_eq!(similarities[1], 3.0, epsilon = 1e-6);

        // Equal attention weights
        assert_relative_eq!(attention[0], 0.5, epsilon = 1e-6);
        assert_relative_eq!(attention[1], 0.5, epsilon = 1e-6);
    }

    #[test]
    fn test_compute_attention_beta_effect() {
        let patterns = vec![vec![1.0, 0.0], vec![0.5, 0.5]];
        let query = vec![1.0, 0.0];

        // Low beta - more diffuse attention
        let (attn_low, _) = compute_attention(&patterns, &query, 0.5);

        // High beta - sharper attention
        let (attn_high, _) = compute_attention(&patterns, &query, 5.0);

        // High beta should concentrate more weight on best match
        assert!(attn_high[0] > attn_low[0]);
        assert!(attn_high[1] < attn_low[1]);
    }
}
