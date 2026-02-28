//! Scalar (non-SIMD) distance implementations
//!
//! These are fallback implementations that work on all platforms.

/// Euclidean (L2) distance - scalar implementation
#[inline]
pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());

    let sum: f32 = a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| {
            let diff = x - y;
            diff * diff
        })
        .sum();

    sum.sqrt()
}

/// Squared Euclidean distance (avoids sqrt for comparisons)
#[inline]
pub fn euclidean_distance_squared(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());

    a.iter()
        .zip(b.iter())
        .map(|(x, y)| {
            let diff = x - y;
            diff * diff
        })
        .sum()
}

/// Cosine distance - scalar implementation
#[inline]
pub fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());

    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }

    let denominator = (norm_a * norm_b).sqrt();

    if denominator == 0.0 {
        return 1.0; // Max distance if either vector is zero
    }

    1.0 - (dot / denominator)
}

/// Cosine similarity (1 - distance)
#[inline]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    1.0 - cosine_distance(a, b)
}

/// Inner product (dot product) distance - scalar implementation
/// Returns negative for use with ORDER BY ASC
#[inline]
pub fn inner_product_distance(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();

    -dot
}

/// Dot product (positive value)
#[inline]
pub fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());

    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// Manhattan (L1) distance - scalar implementation
#[inline]
pub fn manhattan_distance(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());

    a.iter().zip(b.iter()).map(|(x, y)| (x - y).abs()).sum()
}

/// Hamming distance for f32 vectors (based on sign bit)
#[inline]
pub fn hamming_distance_f32(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());

    let count: u32 = a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| {
            let sign_a = x.to_bits() >> 31;
            let sign_b = y.to_bits() >> 31;
            (sign_a ^ sign_b) as u32
        })
        .sum();

    count as f32
}

/// Hamming distance for binary vectors (u64)
#[inline]
pub fn hamming_distance_binary(a: &[u64], b: &[u64]) -> u32 {
    debug_assert_eq!(a.len(), b.len());

    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x ^ y).count_ones())
        .sum()
}

/// Jaccard distance for sparse binary vectors
#[inline]
pub fn jaccard_distance(a: &[u64], b: &[u64]) -> f32 {
    debug_assert_eq!(a.len(), b.len());

    let mut intersection = 0u32;
    let mut union = 0u32;

    for (x, y) in a.iter().zip(b.iter()) {
        intersection += (x & y).count_ones();
        union += (x | y).count_ones();
    }

    if union == 0 {
        return 0.0;
    }

    1.0 - (intersection as f32 / union as f32)
}

/// Chebyshev (L∞) distance
#[inline]
pub fn chebyshev_distance(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());

    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).abs())
        .fold(0.0f32, f32::max)
}

/// Minkowski distance with parameter p
#[inline]
pub fn minkowski_distance(a: &[f32], b: &[f32], p: f32) -> f32 {
    debug_assert_eq!(a.len(), b.len());

    if p == 1.0 {
        return manhattan_distance(a, b);
    }
    if p == 2.0 {
        return euclidean_distance(a, b);
    }
    if p == f32::INFINITY {
        return chebyshev_distance(a, b);
    }

    let sum: f32 = a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).abs().powf(p))
        .sum();

    sum.powf(1.0 / p)
}

/// Canberra distance
#[inline]
pub fn canberra_distance(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());

    a.iter()
        .zip(b.iter())
        .map(|(x, y)| {
            let num = (x - y).abs();
            let denom = x.abs() + y.abs();
            if denom == 0.0 {
                0.0
            } else {
                num / denom
            }
        })
        .sum()
}

/// Bray-Curtis distance
#[inline]
pub fn bray_curtis_distance(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());

    let mut sum_diff = 0.0f32;
    let mut sum_total = 0.0f32;

    for (x, y) in a.iter().zip(b.iter()) {
        sum_diff += (x - y).abs();
        sum_total += x.abs() + y.abs();
    }

    if sum_total == 0.0 {
        return 0.0;
    }

    sum_diff / sum_total
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_euclidean() {
        let a = vec![0.0, 0.0];
        let b = vec![3.0, 4.0];
        assert!((euclidean_distance(&a, &b) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_euclidean_squared() {
        let a = vec![0.0, 0.0];
        let b = vec![3.0, 4.0];
        assert!((euclidean_distance_squared(&a, &b) - 25.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_same_direction() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![2.0, 0.0, 0.0];
        assert!(cosine_distance(&a, &b).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_opposite() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        assert!((cosine_distance(&a, &b) - 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!((cosine_distance(&a, &b) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_inner_product() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        // dot = 4 + 10 + 18 = 32
        assert!((inner_product_distance(&a, &b) - (-32.0)).abs() < 1e-6);
    }

    #[test]
    fn test_manhattan() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 6.0, 8.0];
        // |3| + |4| + |5| = 12
        assert!((manhattan_distance(&a, &b) - 12.0).abs() < 1e-6);
    }

    #[test]
    fn test_hamming_binary() {
        let a = vec![0b1010_1010u64];
        let b = vec![0b1111_0000u64];
        let dist = hamming_distance_binary(&a, &b);
        assert_eq!(dist, 4);
    }

    #[test]
    fn test_chebyshev() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 10.0, 5.0];
        // max(|3|, |8|, |2|) = 8
        assert!((chebyshev_distance(&a, &b) - 8.0).abs() < 1e-6);
    }

    #[test]
    fn test_minkowski_p1() {
        let a = vec![1.0, 2.0];
        let b = vec![4.0, 6.0];
        // Same as manhattan
        assert!((minkowski_distance(&a, &b, 1.0) - manhattan_distance(&a, &b)).abs() < 1e-6);
    }

    #[test]
    fn test_minkowski_p2() {
        let a = vec![0.0, 0.0];
        let b = vec![3.0, 4.0];
        // Same as euclidean
        assert!((minkowski_distance(&a, &b, 2.0) - euclidean_distance(&a, &b)).abs() < 1e-6);
    }
}
