//! Distance metrics for vector similarity computation.
//!
//! This module provides optimized implementations of common distance metrics:
//! - Cosine distance/similarity
//! - Euclidean (L2) distance
//! - Dot product
//!
//! ## SIMD Optimization
//!
//! When the `simd` feature is enabled, these functions use SIMD intrinsics
//! for improved performance on supported architectures.

#![allow(dead_code)]  // Utility functions for future use

/// Compute the cosine distance between two vectors.
///
/// Cosine distance = 1 - cosine_similarity
///
/// Returns a value in [0, 2]:
/// - 0 = identical direction
/// - 1 = orthogonal
/// - 2 = opposite direction
///
/// # Panics
/// Panics in debug mode if vectors have different lengths.
#[inline]
pub fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    1.0 - cosine_similarity(a, b)
}

/// Compute the cosine similarity between two vectors.
///
/// Returns a value in [-1, 1]:
/// - 1 = identical direction
/// - 0 = orthogonal
/// - -1 = opposite direction
///
/// # Panics
/// Panics in debug mode if vectors have different lengths.
#[inline]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len(), "Vector length mismatch");

    let dot = dot_product(a, b);
    let norm_a = l2_norm(a);
    let norm_b = l2_norm(b);

    if norm_a < 1e-10 || norm_b < 1e-10 {
        return 0.0;
    }

    (dot / (norm_a * norm_b)).clamp(-1.0, 1.0)
}

/// Compute the Euclidean (L2) distance between two vectors.
///
/// Returns the straight-line distance in n-dimensional space.
///
/// # Panics
/// Panics in debug mode if vectors have different lengths.
#[inline]
pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    squared_euclidean_distance(a, b).sqrt()
}

/// Compute the squared Euclidean distance.
///
/// This is faster than `euclidean_distance` when only comparing distances,
/// as it avoids the square root operation.
#[inline]
pub fn squared_euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len(), "Vector length mismatch");

    #[cfg(feature = "simd")]
    {
        simd_squared_euclidean(a, b)
    }

    #[cfg(not(feature = "simd"))]
    {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| {
                let diff = x - y;
                diff * diff
            })
            .sum()
    }
}

/// Compute the dot product of two vectors.
///
/// # Panics
/// Panics in debug mode if vectors have different lengths.
#[inline]
pub fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len(), "Vector length mismatch");

    #[cfg(feature = "simd")]
    {
        simd_dot_product(a, b)
    }

    #[cfg(not(feature = "simd"))]
    {
        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
    }
}

/// Compute the L2 (Euclidean) norm of a vector.
#[inline]
pub fn l2_norm(v: &[f32]) -> f32 {
    dot_product(v, v).sqrt()
}

/// Compute the L1 (Manhattan) norm of a vector.
#[inline]
pub fn l1_norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x.abs()).sum()
}

/// Normalize a vector to unit length (L2 normalization).
///
/// Returns a zero vector if the input has zero or near-zero norm.
#[inline]
pub fn normalize_vector(v: &[f32]) -> Vec<f32> {
    let norm = l2_norm(v);
    if norm < 1e-10 {
        return vec![0.0; v.len()];
    }
    v.iter().map(|x| x / norm).collect()
}

/// Normalize a vector in place.
#[inline]
pub fn normalize_vector_inplace(v: &mut [f32]) {
    let norm = l2_norm(v);
    if norm < 1e-10 {
        v.fill(0.0);
        return;
    }
    for x in v.iter_mut() {
        *x /= norm;
    }
}

/// Compute the Manhattan (L1) distance between two vectors.
#[inline]
pub fn manhattan_distance(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len(), "Vector length mismatch");

    a.iter().zip(b.iter()).map(|(x, y)| (x - y).abs()).sum()
}

/// Compute the Chebyshev (L-infinity) distance between two vectors.
///
/// This is the maximum absolute difference along any dimension.
#[inline]
pub fn chebyshev_distance(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len(), "Vector length mismatch");

    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).abs())
        .fold(0.0f32, f32::max)
}

/// Compute angular distance (based on cosine).
///
/// Returns the angle in radians between two vectors, normalized to [0, 1].
#[inline]
pub fn angular_distance(a: &[f32], b: &[f32]) -> f32 {
    let cos_sim = cosine_similarity(a, b);
    cos_sim.acos() / std::f32::consts::PI
}

/// Add two vectors element-wise.
#[inline]
pub fn vector_add(a: &[f32], b: &[f32]) -> Vec<f32> {
    debug_assert_eq!(a.len(), b.len(), "Vector length mismatch");
    a.iter().zip(b.iter()).map(|(x, y)| x + y).collect()
}

/// Subtract two vectors element-wise (a - b).
#[inline]
pub fn vector_sub(a: &[f32], b: &[f32]) -> Vec<f32> {
    debug_assert_eq!(a.len(), b.len(), "Vector length mismatch");
    a.iter().zip(b.iter()).map(|(x, y)| x - y).collect()
}

/// Scale a vector by a scalar.
#[inline]
pub fn vector_scale(v: &[f32], scalar: f32) -> Vec<f32> {
    v.iter().map(|x| x * scalar).collect()
}

/// Compute the centroid (average) of multiple vectors.
pub fn centroid(vectors: &[&[f32]]) -> Option<Vec<f32>> {
    if vectors.is_empty() {
        return None;
    }

    let dim = vectors[0].len();
    let n = vectors.len() as f32;

    let mut result = vec![0.0; dim];
    for v in vectors {
        debug_assert_eq!(v.len(), dim, "Vector dimension mismatch");
        for (i, &x) in v.iter().enumerate() {
            result[i] += x;
        }
    }

    for x in result.iter_mut() {
        *x /= n;
    }

    Some(result)
}

/// Check if a vector is normalized (unit length).
#[inline]
pub fn is_normalized(v: &[f32], tolerance: f32) -> bool {
    let norm = l2_norm(v);
    (norm - 1.0).abs() < tolerance
}

// SIMD implementations

#[cfg(feature = "simd")]
fn simd_dot_product(a: &[f32], b: &[f32]) -> f32 {
    // Fall back to scalar for now - can be enhanced with platform-specific SIMD
    // when needed (e.g., using std::arch for AVX/AVX2/AVX-512)
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

#[cfg(feature = "simd")]
fn simd_squared_euclidean(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| {
            let diff = x - y;
            diff * diff
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_cosine_similarity_identical() {
        let v = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&v, &v);
        assert_relative_eq!(sim, 1.0, epsilon = 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![-1.0, 0.0, 0.0];
        let sim = cosine_similarity(&v1, &v2);
        assert_relative_eq!(sim, -1.0, epsilon = 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&v1, &v2);
        assert_relative_eq!(sim, 0.0, epsilon = 1e-6);
    }

    #[test]
    fn test_cosine_distance() {
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![0.0, 1.0, 0.0];
        let dist = cosine_distance(&v1, &v2);
        assert_relative_eq!(dist, 1.0, epsilon = 1e-6);
    }

    #[test]
    fn test_euclidean_distance() {
        let v1 = vec![0.0, 0.0, 0.0];
        let v2 = vec![3.0, 4.0, 0.0];
        let dist = euclidean_distance(&v1, &v2);
        assert_relative_eq!(dist, 5.0, epsilon = 1e-6);
    }

    #[test]
    fn test_normalize_vector() {
        let v = vec![3.0, 4.0];
        let normalized = normalize_vector(&v);
        assert_relative_eq!(l2_norm(&normalized), 1.0, epsilon = 1e-6);
        assert_relative_eq!(normalized[0], 0.6, epsilon = 1e-6);
        assert_relative_eq!(normalized[1], 0.8, epsilon = 1e-6);
    }

    #[test]
    fn test_normalize_zero_vector() {
        let v = vec![0.0, 0.0, 0.0];
        let normalized = normalize_vector(&v);
        assert!(normalized.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn test_dot_product() {
        let v1 = vec![1.0, 2.0, 3.0];
        let v2 = vec![4.0, 5.0, 6.0];
        let dot = dot_product(&v1, &v2);
        assert_relative_eq!(dot, 32.0, epsilon = 1e-6); // 1*4 + 2*5 + 3*6 = 32
    }

    #[test]
    fn test_manhattan_distance() {
        let v1 = vec![0.0, 0.0];
        let v2 = vec![3.0, 4.0];
        let dist = manhattan_distance(&v1, &v2);
        assert_relative_eq!(dist, 7.0, epsilon = 1e-6);
    }

    #[test]
    fn test_chebyshev_distance() {
        let v1 = vec![0.0, 0.0];
        let v2 = vec![3.0, 4.0];
        let dist = chebyshev_distance(&v1, &v2);
        assert_relative_eq!(dist, 4.0, epsilon = 1e-6);
    }

    #[test]
    fn test_centroid() {
        let v1 = vec![0.0, 0.0];
        let v2 = vec![2.0, 2.0];
        let v3 = vec![4.0, 4.0];

        let c = centroid(&[&v1, &v2, &v3]).unwrap();
        assert_relative_eq!(c[0], 2.0, epsilon = 1e-6);
        assert_relative_eq!(c[1], 2.0, epsilon = 1e-6);
    }

    #[test]
    fn test_is_normalized() {
        let v = normalize_vector(&[3.0, 4.0]);
        assert!(is_normalized(&v, 1e-6));

        let v2 = vec![1.0, 2.0, 3.0];
        assert!(!is_normalized(&v2, 1e-6));
    }

    #[test]
    fn test_vector_operations() {
        let v1 = vec![1.0, 2.0];
        let v2 = vec![3.0, 4.0];

        let sum = vector_add(&v1, &v2);
        assert_eq!(sum, vec![4.0, 6.0]);

        let diff = vector_sub(&v1, &v2);
        assert_eq!(diff, vec![-2.0, -2.0]);

        let scaled = vector_scale(&v1, 2.0);
        assert_eq!(scaled, vec![2.0, 4.0]);
    }

    #[test]
    fn test_angular_distance() {
        let v1 = vec![1.0, 0.0];
        let v2 = vec![0.0, 1.0];
        let dist = angular_distance(&v1, &v2);
        // 90 degrees = pi/2, normalized = 0.5
        assert_relative_eq!(dist, 0.5, epsilon = 1e-6);
    }
}
