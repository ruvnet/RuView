//! SIMD Correctness Tests
//!
//! This module verifies that SIMD implementations produce identical results
//! to scalar fallback implementations across various input sizes and edge cases.

use ruvector_core::simd_intrinsics::*;

// ============================================================================
// Helper Functions for Scalar Computations (Ground Truth)
// ============================================================================

fn scalar_euclidean(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| {
            let diff = x - y;
            diff * diff
        })
        .sum::<f32>()
        .sqrt()
}

fn scalar_dot_product(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

fn scalar_cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a > f32::EPSILON && norm_b > f32::EPSILON {
        dot / (norm_a * norm_b)
    } else {
        0.0
    }
}

fn scalar_manhattan(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| (x - y).abs()).sum()
}

// ============================================================================
// Euclidean Distance Tests
// ============================================================================

#[test]
fn test_euclidean_simd_vs_scalar_small() {
    let a = vec![1.0, 2.0, 3.0, 4.0];
    let b = vec![5.0, 6.0, 7.0, 8.0];

    let simd_result = euclidean_distance_simd(&a, &b);
    let scalar_result = scalar_euclidean(&a, &b);

    assert!(
        (simd_result - scalar_result).abs() < 1e-5,
        "Euclidean mismatch: SIMD={}, scalar={}",
        simd_result,
        scalar_result
    );
}

#[test]
fn test_euclidean_simd_vs_scalar_exact_simd_width() {
    // Test with exact AVX2 width (8 floats)
    let a: Vec<f32> = (0..8).map(|i| i as f32).collect();
    let b: Vec<f32> = (0..8).map(|i| (i + 1) as f32).collect();

    let simd_result = euclidean_distance_simd(&a, &b);
    let scalar_result = scalar_euclidean(&a, &b);

    assert!(
        (simd_result - scalar_result).abs() < 1e-5,
        "8-element Euclidean mismatch: SIMD={}, scalar={}",
        simd_result,
        scalar_result
    );
}

#[test]
fn test_euclidean_simd_vs_scalar_non_aligned() {
    // Test with non-SIMD-aligned sizes
    for size in [3, 5, 7, 9, 11, 13, 15, 17, 31, 33, 63, 65, 127, 129] {
        let a: Vec<f32> = (0..size).map(|i| (i as f32) * 0.1).collect();
        let b: Vec<f32> = (0..size).map(|i| (i as f32) * 0.2).collect();

        let simd_result = euclidean_distance_simd(&a, &b);
        let scalar_result = scalar_euclidean(&a, &b);

        assert!(
            (simd_result - scalar_result).abs() < 0.01,
            "Size {} Euclidean mismatch: SIMD={}, scalar={}",
            size,
            simd_result,
            scalar_result
        );
    }
}

#[test]
fn test_euclidean_simd_vs_scalar_common_embedding_sizes() {
    // Test common embedding dimensions
    for dim in [128, 256, 384, 512, 768, 1024, 1536, 2048] {
        let a: Vec<f32> = (0..dim).map(|i| ((i % 100) as f32) * 0.01).collect();
        let b: Vec<f32> = (0..dim).map(|i| (((i + 50) % 100) as f32) * 0.01).collect();

        let simd_result = euclidean_distance_simd(&a, &b);
        let scalar_result = scalar_euclidean(&a, &b);

        assert!(
            (simd_result - scalar_result).abs() < 0.1,
            "Dim {} Euclidean mismatch: SIMD={}, scalar={}",
            dim,
            simd_result,
            scalar_result
        );
    }
}

#[test]
fn test_euclidean_simd_identical_vectors() {
    let v = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let result = euclidean_distance_simd(&v, &v);
    assert!(
        result < 1e-6,
        "Distance to self should be ~0, got {}",
        result
    );
}

#[test]
fn test_euclidean_simd_zero_vectors() {
    let zeros = vec![0.0; 16];
    let result = euclidean_distance_simd(&zeros, &zeros);
    assert!(result < 1e-6, "Distance between zeros should be 0");
}

#[test]
fn test_euclidean_simd_negative_values() {
    let a = vec![-1.0, -2.0, -3.0, -4.0, -5.0, -6.0, -7.0, -8.0];
    let b = vec![-5.0, -6.0, -7.0, -8.0, -9.0, -10.0, -11.0, -12.0];

    let simd_result = euclidean_distance_simd(&a, &b);
    let scalar_result = scalar_euclidean(&a, &b);

    assert!(
        (simd_result - scalar_result).abs() < 1e-5,
        "Negative values Euclidean mismatch: SIMD={}, scalar={}",
        simd_result,
        scalar_result
    );
}

#[test]
fn test_euclidean_simd_mixed_signs() {
    let a = vec![-1.0, 2.0, -3.0, 4.0, -5.0, 6.0, -7.0, 8.0];
    let b = vec![1.0, -2.0, 3.0, -4.0, 5.0, -6.0, 7.0, -8.0];

    let simd_result = euclidean_distance_simd(&a, &b);
    let scalar_result = scalar_euclidean(&a, &b);

    assert!(
        (simd_result - scalar_result).abs() < 1e-4,
        "Mixed signs Euclidean mismatch: SIMD={}, scalar={}",
        simd_result,
        scalar_result
    );
}

// ============================================================================
// Dot Product Tests
// ============================================================================

#[test]
fn test_dot_product_simd_vs_scalar_small() {
    let a = vec![1.0, 2.0, 3.0, 4.0];
    let b = vec![5.0, 6.0, 7.0, 8.0];

    let simd_result = dot_product_simd(&a, &b);
    let scalar_result = scalar_dot_product(&a, &b);

    assert!(
        (simd_result - scalar_result).abs() < 1e-4,
        "Dot product mismatch: SIMD={}, scalar={}",
        simd_result,
        scalar_result
    );
}

#[test]
fn test_dot_product_simd_vs_scalar_exact_simd_width() {
    let a: Vec<f32> = (1..=8).map(|i| i as f32).collect();
    let b: Vec<f32> = (1..=8).map(|i| i as f32).collect();

    let simd_result = dot_product_simd(&a, &b);
    let scalar_result = scalar_dot_product(&a, &b);

    assert!(
        (simd_result - scalar_result).abs() < 1e-4,
        "8-element dot product mismatch: SIMD={}, scalar={}",
        simd_result,
        scalar_result
    );
}

#[test]
fn test_dot_product_simd_vs_scalar_non_aligned() {
    for size in [3, 5, 7, 9, 11, 13, 15, 17, 31, 33, 63, 65, 127, 129] {
        let a: Vec<f32> = (0..size).map(|i| (i as f32) * 0.1).collect();
        let b: Vec<f32> = (0..size).map(|i| (i as f32) * 0.2).collect();

        let simd_result = dot_product_simd(&a, &b);
        let scalar_result = scalar_dot_product(&a, &b);

        assert!(
            (simd_result - scalar_result).abs() < 0.1,
            "Size {} dot product mismatch: SIMD={}, scalar={}",
            size,
            simd_result,
            scalar_result
        );
    }
}

#[test]
fn test_dot_product_simd_common_embedding_sizes() {
    for dim in [128, 256, 384, 512, 768, 1024, 1536, 2048] {
        let a: Vec<f32> = (0..dim).map(|i| ((i % 10) as f32) * 0.1).collect();
        let b: Vec<f32> = (0..dim).map(|i| (((i + 5) % 10) as f32) * 0.1).collect();

        let simd_result = dot_product_simd(&a, &b);
        let scalar_result = scalar_dot_product(&a, &b);

        assert!(
            (simd_result - scalar_result).abs() < 0.5,
            "Dim {} dot product mismatch: SIMD={}, scalar={}",
            dim,
            simd_result,
            scalar_result
        );
    }
}

#[test]
fn test_dot_product_simd_orthogonal_vectors() {
    let a = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    let b = vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];

    let result = dot_product_simd(&a, &b);
    assert!(result.abs() < 1e-6, "Orthogonal dot product should be 0");
}

// ============================================================================
// Cosine Similarity Tests
// ============================================================================

#[test]
fn test_cosine_simd_vs_scalar_small() {
    let a = vec![1.0, 2.0, 3.0, 4.0];
    let b = vec![5.0, 6.0, 7.0, 8.0];

    let simd_result = cosine_similarity_simd(&a, &b);
    let scalar_result = scalar_cosine_similarity(&a, &b);

    assert!(
        (simd_result - scalar_result).abs() < 1e-4,
        "Cosine mismatch: SIMD={}, scalar={}",
        simd_result,
        scalar_result
    );
}

#[test]
fn test_cosine_simd_vs_scalar_non_aligned() {
    for size in [3, 5, 7, 9, 11, 13, 15, 17, 31, 33, 63, 65] {
        let a: Vec<f32> = (1..=size).map(|i| (i as f32) * 0.1).collect();
        let b: Vec<f32> = (1..=size).map(|i| (i as f32) * 0.2).collect();

        let simd_result = cosine_similarity_simd(&a, &b);
        let scalar_result = scalar_cosine_similarity(&a, &b);

        assert!(
            (simd_result - scalar_result).abs() < 0.01,
            "Size {} cosine mismatch: SIMD={}, scalar={}",
            size,
            simd_result,
            scalar_result
        );
    }
}

#[test]
fn test_cosine_simd_identical_vectors() {
    let v = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let result = cosine_similarity_simd(&v, &v);
    assert!(
        (result - 1.0).abs() < 1e-5,
        "Identical vectors should have similarity 1.0, got {}",
        result
    );
}

#[test]
fn test_cosine_simd_opposite_vectors() {
    let a = vec![1.0, 2.0, 3.0, 4.0];
    let b = vec![-1.0, -2.0, -3.0, -4.0];

    let result = cosine_similarity_simd(&a, &b);
    assert!(
        (result + 1.0).abs() < 1e-5,
        "Opposite vectors should have similarity -1.0, got {}",
        result
    );
}

#[test]
fn test_cosine_simd_orthogonal_vectors() {
    let a = vec![1.0, 0.0, 0.0, 0.0];
    let b = vec![0.0, 1.0, 0.0, 0.0];

    let result = cosine_similarity_simd(&a, &b);
    assert!(
        result.abs() < 1e-5,
        "Orthogonal vectors should have similarity 0, got {}",
        result
    );
}

// ============================================================================
// Manhattan Distance Tests
// ============================================================================

#[test]
fn test_manhattan_simd_vs_scalar_small() {
    let a = vec![1.0, 2.0, 3.0, 4.0];
    let b = vec![5.0, 6.0, 7.0, 8.0];

    let simd_result = manhattan_distance_simd(&a, &b);
    let scalar_result = scalar_manhattan(&a, &b);

    assert!(
        (simd_result - scalar_result).abs() < 1e-4,
        "Manhattan mismatch: SIMD={}, scalar={}",
        simd_result,
        scalar_result
    );
}

#[test]
fn test_manhattan_simd_vs_scalar_non_aligned() {
    for size in [3, 5, 7, 9, 11, 13, 15, 17, 31, 33, 63, 65] {
        let a: Vec<f32> = (0..size).map(|i| (i as f32) * 0.1).collect();
        let b: Vec<f32> = (0..size).map(|i| (i as f32) * 0.2).collect();

        let simd_result = manhattan_distance_simd(&a, &b);
        let scalar_result = scalar_manhattan(&a, &b);

        assert!(
            (simd_result - scalar_result).abs() < 0.01,
            "Size {} Manhattan mismatch: SIMD={}, scalar={}",
            size,
            simd_result,
            scalar_result
        );
    }
}

#[test]
fn test_manhattan_simd_identical_vectors() {
    let v = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let result = manhattan_distance_simd(&v, &v);
    assert!(
        result < 1e-6,
        "Manhattan to self should be 0, got {}",
        result
    );
}

// ============================================================================
// Numerical Stability Tests
// ============================================================================

#[test]
fn test_simd_large_values() {
    // Test with large but finite values
    let large_val = 1e10;
    let a: Vec<f32> = (0..16).map(|i| large_val + (i as f32)).collect();
    let b: Vec<f32> = (0..16).map(|i| large_val + (i as f32) + 1.0).collect();

    let simd_result = euclidean_distance_simd(&a, &b);
    let scalar_result = scalar_euclidean(&a, &b);

    assert!(
        simd_result.is_finite() && scalar_result.is_finite(),
        "Results should be finite for large values"
    );
    assert!(
        (simd_result - scalar_result).abs() < 0.1,
        "Large values mismatch: SIMD={}, scalar={}",
        simd_result,
        scalar_result
    );
}

#[test]
fn test_simd_small_values() {
    // Test with small values
    let small_val = 1e-10;
    let a: Vec<f32> = (0..16).map(|i| small_val * (i as f32 + 1.0)).collect();
    let b: Vec<f32> = (0..16).map(|i| small_val * (i as f32 + 2.0)).collect();

    let simd_result = euclidean_distance_simd(&a, &b);
    let scalar_result = scalar_euclidean(&a, &b);

    assert!(
        simd_result.is_finite() && scalar_result.is_finite(),
        "Results should be finite for small values"
    );
}

#[test]
fn test_simd_denormalized_values() {
    // Test with denormalized floats
    let a = vec![f32::MIN_POSITIVE; 8];
    let b = vec![f32::MIN_POSITIVE * 2.0; 8];

    let simd_result = euclidean_distance_simd(&a, &b);
    let scalar_result = scalar_euclidean(&a, &b);

    assert!(
        simd_result.is_finite() && scalar_result.is_finite(),
        "Results should be finite for denormalized values"
    );
}

// ============================================================================
// Legacy Alias Tests
// ============================================================================

#[test]
fn test_legacy_avx2_aliases_match_simd() {
    let a = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let b = vec![9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0];

    // Legacy AVX2 functions should produce same results as SIMD functions
    assert_eq!(
        euclidean_distance_avx2(&a, &b),
        euclidean_distance_simd(&a, &b)
    );
    assert_eq!(dot_product_avx2(&a, &b), dot_product_simd(&a, &b));
    assert_eq!(
        cosine_similarity_avx2(&a, &b),
        cosine_similarity_simd(&a, &b)
    );
}

// ============================================================================
// Batch Operation Tests
// ============================================================================

#[test]
fn test_simd_batch_consistency() {
    let query: Vec<f32> = (0..64).map(|i| (i as f32) * 0.1).collect();
    let vectors: Vec<Vec<f32>> = (0..100)
        .map(|j| (0..64).map(|i| ((i + j) as f32) * 0.1).collect())
        .collect();

    // Compute distances using SIMD
    let simd_distances: Vec<f32> = vectors
        .iter()
        .map(|v| euclidean_distance_simd(&query, v))
        .collect();

    // Compute distances using scalar
    let scalar_distances: Vec<f32> = vectors
        .iter()
        .map(|v| scalar_euclidean(&query, v))
        .collect();

    // Compare
    for (i, (simd, scalar)) in simd_distances
        .iter()
        .zip(scalar_distances.iter())
        .enumerate()
    {
        assert!(
            (simd - scalar).abs() < 0.01,
            "Vector {} mismatch: SIMD={}, scalar={}",
            i,
            simd,
            scalar
        );
    }
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_simd_single_element() {
    let a = vec![1.0];
    let b = vec![2.0];

    let euclidean = euclidean_distance_simd(&a, &b);
    let dot = dot_product_simd(&a, &b);
    let manhattan = manhattan_distance_simd(&a, &b);

    assert!((euclidean - 1.0).abs() < 1e-6);
    assert!((dot - 2.0).abs() < 1e-6);
    assert!((manhattan - 1.0).abs() < 1e-6);
}

#[test]
fn test_simd_two_elements() {
    let a = vec![1.0, 0.0];
    let b = vec![0.0, 1.0];

    let euclidean = euclidean_distance_simd(&a, &b);
    let expected = (2.0_f32).sqrt(); // sqrt(1 + 1)

    assert!(
        (euclidean - expected).abs() < 1e-5,
        "Two element test: got {}, expected {}",
        euclidean,
        expected
    );
}

// ============================================================================
// Stress Tests for SIMD
// ============================================================================

#[test]
fn test_simd_many_operations() {
    let a: Vec<f32> = (0..512).map(|i| (i as f32) * 0.001).collect();
    let b: Vec<f32> = (0..512).map(|i| ((i + 256) as f32) * 0.001).collect();

    // Perform many operations to stress test
    for _ in 0..1000 {
        let _ = euclidean_distance_simd(&a, &b);
        let _ = dot_product_simd(&a, &b);
        let _ = cosine_similarity_simd(&a, &b);
        let _ = manhattan_distance_simd(&a, &b);
    }

    // Final verification
    let result = euclidean_distance_simd(&a, &b);
    assert!(
        result.is_finite(),
        "Result should be finite after stress test"
    );
}
