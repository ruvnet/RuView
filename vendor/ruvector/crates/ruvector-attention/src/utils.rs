//! Utility functions for attention mechanisms.
//!
//! This module provides common utilities like softmax, masking, and
//! numerical stability helpers used across attention implementations.

use crate::error::{AttentionError, AttentionResult};

/// Stable softmax that returns Vec<f32> directly (no Result)
/// Used by sparse, moe, and graph modules
#[inline]
pub fn stable_softmax(values: &[f32]) -> Vec<f32> {
    if values.is_empty() {
        return vec![];
    }

    // Find maximum for numerical stability
    let max_val = values
        .iter()
        .copied()
        .filter(|x| x.is_finite())
        .fold(f32::NEG_INFINITY, f32::max);

    if !max_val.is_finite() {
        // All values are -inf or invalid, return uniform
        let n = values.len();
        return vec![1.0 / n as f32; n];
    }

    // Compute exp(x - max) and sum
    let mut exp_values: Vec<f32> = values
        .iter()
        .map(|&x| {
            if x.is_finite() {
                (x - max_val).exp()
            } else {
                0.0
            }
        })
        .collect();

    let sum: f32 = exp_values.iter().sum();

    if sum <= 1e-10 || !sum.is_finite() {
        // Fallback to uniform
        let n = values.len();
        return vec![1.0 / n as f32; n];
    }

    // Normalize
    let inv_sum = 1.0 / sum;
    exp_values.iter_mut().for_each(|x| *x *= inv_sum);

    exp_values
}

/// Computes softmax over a slice of values.
///
/// Uses the numerically stable variant: softmax(x) = exp(x - max(x)) / sum(exp(x - max(x)))
///
/// # Arguments
///
/// * `values` - Input values
///
/// # Returns
///
/// Softmax-normalized values
#[inline]
pub fn softmax(values: &[f32]) -> AttentionResult<Vec<f32>> {
    if values.is_empty() {
        return Err(AttentionError::EmptyInput(
            "cannot compute softmax of empty slice".to_string(),
        ));
    }

    // Find maximum for numerical stability
    let max_val = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);

    if !max_val.is_finite() {
        return Err(AttentionError::NumericalInstability(
            "non-finite values in softmax input".to_string(),
        ));
    }

    // Compute exp(x - max) and sum
    let mut exp_values: Vec<f32> = values.iter().map(|&x| (x - max_val).exp()).collect();

    let sum: f32 = exp_values.iter().sum();

    if sum <= 0.0 || !sum.is_finite() {
        return Err(AttentionError::NumericalInstability(
            "invalid sum in softmax computation".to_string(),
        ));
    }

    // Normalize
    let inv_sum = 1.0 / sum;
    exp_values.iter_mut().for_each(|x| *x *= inv_sum);

    Ok(exp_values)
}

/// Computes softmax with masking support.
///
/// Masked positions are set to negative infinity before softmax,
/// resulting in zero attention weights.
///
/// # Arguments
///
/// * `values` - Input values
/// * `mask` - Optional mask (true = attend, false = mask out)
///
/// # Returns
///
/// Masked and softmax-normalized values
#[inline]
pub fn masked_softmax(values: &[f32], mask: Option<&[bool]>) -> AttentionResult<Vec<f32>> {
    if values.is_empty() {
        return Err(AttentionError::EmptyInput(
            "cannot compute softmax of empty slice".to_string(),
        ));
    }

    let masked_values = if let Some(m) = mask {
        if m.len() != values.len() {
            return Err(AttentionError::InvalidMask {
                expected: format!("{}", values.len()),
                actual: format!("{}", m.len()),
            });
        }

        values
            .iter()
            .zip(m.iter())
            .map(|(&v, &keep)| if keep { v } else { f32::NEG_INFINITY })
            .collect::<Vec<_>>()
    } else {
        values.to_vec()
    };

    softmax(&masked_values)
}

/// Applies causal masking to attention scores.
///
/// For position i, only positions 0..=i can be attended to.
///
/// # Arguments
///
/// * `scores` - Attention scores matrix [query_len, key_len]
/// * `query_len` - Number of query positions
/// * `key_len` - Number of key positions
///
/// # Returns
///
/// Causally masked scores
pub fn apply_causal_mask(
    scores: &mut [f32],
    query_len: usize,
    key_len: usize,
) -> AttentionResult<()> {
    if scores.len() != query_len * key_len {
        return Err(AttentionError::InvalidMask {
            expected: format!("{}x{}", query_len, key_len),
            actual: format!("{}", scores.len()),
        });
    }

    for i in 0..query_len {
        for j in (i + 1)..key_len {
            scores[i * key_len + j] = f32::NEG_INFINITY;
        }
    }

    Ok(())
}

/// Computes dot product between two vectors.
///
/// # Arguments
///
/// * `a` - First vector
/// * `b` - Second vector
///
/// # Returns
///
/// Dot product value
#[inline]
pub fn dot_product(a: &[f32], b: &[f32]) -> AttentionResult<f32> {
    if a.len() != b.len() {
        return Err(AttentionError::DimensionMismatch {
            expected: a.len(),
            actual: b.len(),
        });
    }

    Ok(a.iter().zip(b.iter()).map(|(x, y)| x * y).sum())
}

/// Scales a vector by a scalar value.
///
/// # Arguments
///
/// * `vector` - Input vector (modified in place)
/// * `scale` - Scale factor
#[inline]
pub fn scale_vector(vector: &mut [f32], scale: f32) {
    vector.iter_mut().for_each(|x| *x *= scale);
}

/// Adds two vectors element-wise.
///
/// # Arguments
///
/// * `a` - First vector
/// * `b` - Second vector
///
/// # Returns
///
/// Sum vector
#[inline]
pub fn add_vectors(a: &[f32], b: &[f32]) -> AttentionResult<Vec<f32>> {
    if a.len() != b.len() {
        return Err(AttentionError::DimensionMismatch {
            expected: a.len(),
            actual: b.len(),
        });
    }

    Ok(a.iter().zip(b.iter()).map(|(x, y)| x + y).collect())
}

/// Computes L2 norm of a vector.
///
/// # Arguments
///
/// * `vector` - Input vector
///
/// # Returns
///
/// L2 norm value
#[inline]
pub fn l2_norm(vector: &[f32]) -> f32 {
    vector.iter().map(|x| x * x).sum::<f32>().sqrt()
}

/// Normalizes a vector to unit length.
///
/// # Arguments
///
/// * `vector` - Input vector (modified in place)
///
/// # Returns
///
/// Original norm before normalization
pub fn normalize_vector(vector: &mut [f32]) -> AttentionResult<f32> {
    let norm = l2_norm(vector);

    if norm <= 0.0 || !norm.is_finite() {
        return Err(AttentionError::NumericalInstability(
            "cannot normalize zero or non-finite vector".to_string(),
        ));
    }

    let inv_norm = 1.0 / norm;
    vector.iter_mut().for_each(|x| *x *= inv_norm);

    Ok(norm)
}

/// Applies dropout to a vector during training.
///
/// # Arguments
///
/// * `vector` - Input vector (modified in place)
/// * `dropout_prob` - Dropout probability (0.0 to 1.0)
/// * `training` - Whether in training mode
/// * `rng` - Random number generator
pub fn apply_dropout(
    vector: &mut [f32],
    dropout_prob: f32,
    training: bool,
    rng: &mut impl rand::Rng,
) {
    if !training || dropout_prob == 0.0 {
        return;
    }

    let scale = 1.0 / (1.0 - dropout_prob);
    for x in vector.iter_mut() {
        if rng.gen::<f32>() < dropout_prob {
            *x = 0.0;
        } else {
            *x *= scale;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_softmax() {
        let values = vec![1.0, 2.0, 3.0];
        let result = softmax(&values).unwrap();

        // Sum should be approximately 1.0
        let sum: f32 = result.iter().sum();
        assert_relative_eq!(sum, 1.0, epsilon = 1e-6);

        // Values should be in ascending order
        assert!(result[0] < result[1]);
        assert!(result[1] < result[2]);
    }

    #[test]
    fn test_softmax_numerical_stability() {
        let values = vec![1000.0, 1001.0, 1002.0];
        let result = softmax(&values).unwrap();

        let sum: f32 = result.iter().sum();
        assert_relative_eq!(sum, 1.0, epsilon = 1e-6);
    }

    #[test]
    fn test_masked_softmax() {
        let values = vec![1.0, 2.0, 3.0, 4.0];
        let mask = vec![true, true, false, false];
        let result = masked_softmax(&values, Some(&mask)).unwrap();

        // Masked positions should be zero
        assert_relative_eq!(result[2], 0.0, epsilon = 1e-6);
        assert_relative_eq!(result[3], 0.0, epsilon = 1e-6);

        // Unmasked positions should sum to 1
        let sum: f32 = result[0] + result[1];
        assert_relative_eq!(sum, 1.0, epsilon = 1e-6);
    }

    #[test]
    fn test_dot_product() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let result = dot_product(&a, &b).unwrap();

        assert_relative_eq!(result, 32.0, epsilon = 1e-6);
    }

    #[test]
    fn test_scale_vector() {
        let mut vector = vec![1.0, 2.0, 3.0];
        scale_vector(&mut vector, 2.0);

        assert_relative_eq!(vector[0], 2.0);
        assert_relative_eq!(vector[1], 4.0);
        assert_relative_eq!(vector[2], 6.0);
    }

    #[test]
    fn test_normalize_vector() {
        let mut vector = vec![3.0, 4.0];
        let norm = normalize_vector(&mut vector).unwrap();

        assert_relative_eq!(norm, 5.0, epsilon = 1e-6);
        assert_relative_eq!(l2_norm(&vector), 1.0, epsilon = 1e-6);
    }

    #[test]
    fn test_causal_mask() {
        let mut scores = vec![0.0; 9]; // 3x3 matrix
        apply_causal_mask(&mut scores, 3, 3).unwrap();

        // Check upper triangle is masked
        assert_eq!(scores[1], f32::NEG_INFINITY); // (0, 1)
        assert_eq!(scores[2], f32::NEG_INFINITY); // (0, 2)
        assert_eq!(scores[5], f32::NEG_INFINITY); // (1, 2)

        // Check diagonal and lower triangle are not masked
        assert_eq!(scores[0], 0.0); // (0, 0)
        assert_eq!(scores[4], 0.0); // (1, 1)
        assert_eq!(scores[8], 0.0); // (2, 2)
    }
}
