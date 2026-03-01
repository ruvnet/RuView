//! Shared utility functions for the RuVector Data Framework
//!
//! This module contains common utilities used across multiple modules,
//! including vector operations and mathematical functions.

/// Compute cosine similarity between two vectors
///
/// Returns a value in [-1, 1] where:
/// - 1 = identical direction
/// - 0 = orthogonal
/// - -1 = opposite direction
///
/// # Arguments
///
/// * `a` - First vector
/// * `b` - Second vector (must be same length as `a`)
///
/// # Returns
///
/// Cosine similarity score, or 0.0 if vectors are empty or different lengths
///
/// # Example
///
/// ```
/// use ruvector_data_framework::utils::cosine_similarity;
///
/// let a = vec![1.0, 0.0, 0.0];
/// let b = vec![1.0, 0.0, 0.0];
/// assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);
///
/// let c = vec![0.0, 1.0, 0.0];
/// assert!(cosine_similarity(&a, &c).abs() < 1e-6);
/// ```
#[inline]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    // Process in chunks for better cache locality
    const CHUNK_SIZE: usize = 8;
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    // Process aligned chunks
    let chunks = a.len() / CHUNK_SIZE;
    for chunk in 0..chunks {
        let base = chunk * CHUNK_SIZE;
        for i in 0..CHUNK_SIZE {
            let ai = a[base + i];
            let bi = b[base + i];
            dot += ai * bi;
            norm_a += ai * ai;
            norm_b += bi * bi;
        }
    }

    // Process remainder
    for i in (chunks * CHUNK_SIZE)..a.len() {
        let ai = a[i];
        let bi = b[i];
        dot += ai * bi;
        norm_a += ai * ai;
        norm_b += bi * bi;
    }

    let denom = (norm_a * norm_b).sqrt();
    if denom > 1e-10 {
        dot / denom
    } else {
        0.0
    }
}

/// Compute Euclidean (L2) distance between two vectors
///
/// # Arguments
///
/// * `a` - First vector
/// * `b` - Second vector (must be same length as `a`)
///
/// # Returns
///
/// Euclidean distance, or 0.0 if vectors are empty or different lengths
#[inline]
pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let sum_sq: f32 = a.iter()
        .zip(b.iter())
        .map(|(ai, bi)| {
            let diff = ai - bi;
            diff * diff
        })
        .sum();

    sum_sq.sqrt()
}

/// Normalize a vector to unit length (L2 normalization)
///
/// # Arguments
///
/// * `v` - Vector to normalize (modified in place)
#[inline]
pub fn normalize_vector(v: &mut [f32]) {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 1e-10 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0, 0.0];
        assert!(cosine_similarity(&a, &b).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) + 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_empty() {
        let a: Vec<f32> = vec![];
        let b: Vec<f32> = vec![];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn test_cosine_similarity_different_lengths() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn test_euclidean_distance() {
        let a = vec![0.0, 0.0];
        let b = vec![3.0, 4.0];
        assert!((euclidean_distance(&a, &b) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_normalize_vector() {
        let mut v = vec![3.0, 4.0];
        normalize_vector(&mut v);
        assert!((v[0] - 0.6).abs() < 1e-6);
        assert!((v[1] - 0.8).abs() < 1e-6);
    }
}
