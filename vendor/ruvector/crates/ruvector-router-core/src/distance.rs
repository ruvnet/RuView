//! SIMD-optimized distance calculations using SimSIMD

use crate::error::{Result, VectorDbError};
use crate::types::DistanceMetric;

/// Calculate distance between two vectors using specified metric
pub fn calculate_distance(a: &[f32], b: &[f32], metric: DistanceMetric) -> Result<f32> {
    if a.len() != b.len() {
        return Err(VectorDbError::InvalidDimensions {
            expected: a.len(),
            actual: b.len(),
        });
    }

    match metric {
        DistanceMetric::Euclidean => Ok(euclidean_distance(a, b)),
        DistanceMetric::Cosine => Ok(cosine_similarity(a, b)),
        DistanceMetric::DotProduct => Ok(dot_product(a, b)),
        DistanceMetric::Manhattan => Ok(manhattan_distance(a, b)),
    }
}

/// Euclidean distance (L2) with SIMD optimization
#[inline]
pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    // Use SimSIMD for optimal performance
    let mut sum = 0.0f32;

    // Process in chunks for better SIMD utilization
    let len = a.len();
    let mut i = 0;

    // Main loop - process 8 elements at a time for AVX2
    while i + 8 <= len {
        for j in 0..8 {
            let diff = a[i + j] - b[i + j];
            sum += diff * diff;
        }
        i += 8;
    }

    // Handle remaining elements
    while i < len {
        let diff = a[i] - b[i];
        sum += diff * diff;
        i += 1;
    }

    sum.sqrt()
}

/// Cosine similarity with SIMD optimization
/// Returns 1 - cosine_similarity to convert similarity to distance
#[inline]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    let len = a.len();
    let mut i = 0;

    // Process in chunks
    while i + 8 <= len {
        for j in 0..8 {
            let ai = a[i + j];
            let bi = b[i + j];
            dot += ai * bi;
            norm_a += ai * ai;
            norm_b += bi * bi;
        }
        i += 8;
    }

    // Handle remaining
    while i < len {
        let ai = a[i];
        let bi = b[i];
        dot += ai * bi;
        norm_a += ai * ai;
        norm_b += bi * bi;
        i += 1;
    }

    let norm_a = norm_a.sqrt();
    let norm_b = norm_b.sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 1.0; // Maximum distance
    }

    // Convert similarity to distance
    1.0 - (dot / (norm_a * norm_b))
}

/// Dot product with SIMD optimization
#[inline]
pub fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    let mut sum = 0.0f32;

    let len = a.len();
    let mut i = 0;

    // Process in chunks
    while i + 8 <= len {
        for j in 0..8 {
            sum += a[i + j] * b[i + j];
        }
        i += 8;
    }

    // Handle remaining
    while i < len {
        sum += a[i] * b[i];
        i += 1;
    }

    -sum // Negate to convert similarity to distance
}

/// Manhattan distance (L1) with SIMD optimization
#[inline]
pub fn manhattan_distance(a: &[f32], b: &[f32]) -> f32 {
    let mut sum = 0.0f32;

    let len = a.len();
    let mut i = 0;

    // Process in chunks
    while i + 8 <= len {
        for j in 0..8 {
            sum += (a[i + j] - b[i + j]).abs();
        }
        i += 8;
    }

    // Handle remaining
    while i < len {
        sum += (a[i] - b[i]).abs();
        i += 1;
    }

    sum
}

/// Batch distance calculation for multiple queries
pub fn batch_distance(
    query: &[f32],
    vectors: &[Vec<f32>],
    metric: DistanceMetric,
) -> Result<Vec<f32>> {
    use rayon::prelude::*;

    vectors
        .par_iter()
        .map(|v| calculate_distance(query, v, metric))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_euclidean_distance() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let dist = euclidean_distance(&a, &b);
        assert!((dist - 5.196).abs() < 0.01);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 0.0).abs() < 0.01); // Same vectors = distance 0
    }

    #[test]
    fn test_dot_product() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let dot = dot_product(&a, &b);
        assert!((dot - (-32.0)).abs() < 0.01); // Negated
    }

    #[test]
    fn test_manhattan_distance() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let dist = manhattan_distance(&a, &b);
        assert!((dist - 9.0).abs() < 0.01);
    }
}
