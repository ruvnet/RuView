//! SIMD-optimized distance metrics
//! Uses SimSIMD when available (native), falls back to pure Rust for WASM

use crate::error::{Result, RuvectorError};
use crate::types::DistanceMetric;

/// Calculate distance between two vectors using the specified metric
#[inline]
pub fn distance(a: &[f32], b: &[f32], metric: DistanceMetric) -> Result<f32> {
    if a.len() != b.len() {
        return Err(RuvectorError::DimensionMismatch {
            expected: a.len(),
            actual: b.len(),
        });
    }

    match metric {
        DistanceMetric::Euclidean => Ok(euclidean_distance(a, b)),
        DistanceMetric::Cosine => Ok(cosine_distance(a, b)),
        DistanceMetric::DotProduct => Ok(dot_product_distance(a, b)),
        DistanceMetric::Manhattan => Ok(manhattan_distance(a, b)),
    }
}

/// Euclidean (L2) distance
#[inline]
pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    #[cfg(all(feature = "simd", not(target_arch = "wasm32")))]
    {
        (simsimd::SpatialSimilarity::sqeuclidean(a, b)
            .expect("SimSIMD euclidean failed")
            .sqrt()) as f32
    }
    #[cfg(any(not(feature = "simd"), target_arch = "wasm32"))]
    {
        // Pure Rust fallback for WASM
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y) * (x - y))
            .sum::<f32>()
            .sqrt()
    }
}

/// Cosine distance (1 - cosine_similarity)
#[inline]
pub fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    #[cfg(all(feature = "simd", not(target_arch = "wasm32")))]
    {
        simsimd::SpatialSimilarity::cosine(a, b).expect("SimSIMD cosine failed") as f32
    }
    #[cfg(any(not(feature = "simd"), target_arch = "wasm32"))]
    {
        // Pure Rust fallback for WASM
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a > 1e-8 && norm_b > 1e-8 {
            1.0 - (dot / (norm_a * norm_b))
        } else {
            1.0
        }
    }
}

/// Dot product distance (negative for maximization)
#[inline]
pub fn dot_product_distance(a: &[f32], b: &[f32]) -> f32 {
    #[cfg(all(feature = "simd", not(target_arch = "wasm32")))]
    {
        let dot = simsimd::SpatialSimilarity::dot(a, b).expect("SimSIMD dot product failed");
        (-dot) as f32
    }
    #[cfg(any(not(feature = "simd"), target_arch = "wasm32"))]
    {
        // Pure Rust fallback for WASM
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        -dot
    }
}

/// Manhattan (L1) distance
#[inline]
pub fn manhattan_distance(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| (x - y).abs()).sum()
}

/// Batch distance calculation optimized with Rayon (native) or sequential (WASM)
pub fn batch_distances(
    query: &[f32],
    vectors: &[Vec<f32>],
    metric: DistanceMetric,
) -> Result<Vec<f32>> {
    #[cfg(all(feature = "parallel", not(target_arch = "wasm32")))]
    {
        use rayon::prelude::*;
        vectors
            .par_iter()
            .map(|v| distance(query, v, metric))
            .collect()
    }
    #[cfg(any(not(feature = "parallel"), target_arch = "wasm32"))]
    {
        // Sequential fallback for WASM
        vectors.iter().map(|v| distance(query, v, metric)).collect()
    }
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
    fn test_cosine_distance() {
        // Test with identical vectors (should have distance ~0)
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        let dist = cosine_distance(&a, &b);
        assert!(
            dist < 0.01,
            "Identical vectors should have ~0 distance, got {}",
            dist
        );

        // Test with opposite vectors (should have high distance)
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        let dist = cosine_distance(&a, &b);
        assert!(
            dist > 1.5,
            "Opposite vectors should have high distance, got {}",
            dist
        );
    }

    #[test]
    fn test_dot_product_distance() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let dist = dot_product_distance(&a, &b);
        assert!((dist + 32.0).abs() < 0.01); // -(4 + 10 + 18) = -32
    }

    #[test]
    fn test_manhattan_distance() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let dist = manhattan_distance(&a, &b);
        assert!((dist - 9.0).abs() < 0.01); // |1-4| + |2-5| + |3-6| = 9
    }

    #[test]
    fn test_dimension_mismatch() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        let result = distance(&a, &b, DistanceMetric::Euclidean);
        assert!(result.is_err());
    }
}
