//! Property-based tests using proptest
//!
//! These tests verify mathematical properties and invariants that should hold
//! for all inputs within a given domain.

use proptest::prelude::*;
use ruvector_core::distance::*;
use ruvector_core::quantization::*;
use ruvector_core::types::DistanceMetric;

// ============================================================================
// Distance Metric Properties
// ============================================================================

// Strategy to generate valid vectors with bounded values to prevent overflow
// Using range that won't overflow when squared: sqrt(f32::MAX) ≈ 1.84e19
// We use a more conservative range for numerical stability in distance calculations
fn vector_strategy(dim: usize) -> impl Strategy<Value = Vec<f32>> {
    prop::collection::vec(-1000.0f32..1000.0f32, dim)
}

// Strategy for normalized vectors (for cosine similarity)
fn normalized_vector_strategy(dim: usize) -> impl Strategy<Value = Vec<f32>> {
    vector_strategy(dim).prop_map(move |v| {
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            v.iter().map(|x| x / norm).collect()
        } else {
            vec![1.0 / (dim as f32).sqrt(); dim]
        }
    })
}

proptest! {
    // Property: Distance to self is zero
    #[test]
    fn test_euclidean_self_distance_zero(v in vector_strategy(128)) {
        let dist = euclidean_distance(&v, &v);
        prop_assert!(dist < 0.001, "Distance to self should be ~0, got {}", dist);
    }

    // Property: Euclidean distance is symmetric
    #[test]
    fn test_euclidean_symmetry(
        a in vector_strategy(64),
        b in vector_strategy(64)
    ) {
        let dist_ab = euclidean_distance(&a, &b);
        let dist_ba = euclidean_distance(&b, &a);
        prop_assert!((dist_ab - dist_ba).abs() < 0.001, "Distance should be symmetric");
    }

    // Property: Triangle inequality for Euclidean distance
    #[test]
    fn test_euclidean_triangle_inequality(
        a in vector_strategy(32),
        b in vector_strategy(32),
        c in vector_strategy(32)
    ) {
        let dist_ab = euclidean_distance(&a, &b);
        let dist_bc = euclidean_distance(&b, &c);
        let dist_ac = euclidean_distance(&a, &c);

        // d(a,c) <= d(a,b) + d(b,c)
        prop_assert!(
            dist_ac <= dist_ab + dist_bc + 0.01, // Small epsilon for floating point
            "Triangle inequality violated: {} > {} + {}",
            dist_ac, dist_ab, dist_bc
        );
    }

    // Property: Non-negativity of Euclidean distance
    #[test]
    fn test_euclidean_non_negative(
        a in vector_strategy(64),
        b in vector_strategy(64)
    ) {
        let dist = euclidean_distance(&a, &b);
        prop_assert!(dist >= 0.0, "Distance must be non-negative, got {}", dist);
    }

    // Property: Cosine distance symmetry
    #[test]
    fn test_cosine_symmetry(
        a in normalized_vector_strategy(64),
        b in normalized_vector_strategy(64)
    ) {
        let dist_ab = cosine_distance(&a, &b);
        let dist_ba = cosine_distance(&b, &a);
        prop_assert!((dist_ab - dist_ba).abs() < 0.01, "Cosine distance should be symmetric");
    }

    // Property: Cosine distance to self is zero
    #[test]
    fn test_cosine_self_distance(v in normalized_vector_strategy(64)) {
        let dist = cosine_distance(&v, &v);
        prop_assert!(dist < 0.01, "Cosine distance to self should be ~0, got {}", dist);
    }

    // Property: Manhattan distance symmetry
    #[test]
    fn test_manhattan_symmetry(
        a in vector_strategy(64),
        b in vector_strategy(64)
    ) {
        let dist_ab = manhattan_distance(&a, &b);
        let dist_ba = manhattan_distance(&b, &a);
        prop_assert!((dist_ab - dist_ba).abs() < 0.001);
    }

    // Property: Manhattan distance non-negativity
    #[test]
    fn test_manhattan_non_negative(
        a in vector_strategy(64),
        b in vector_strategy(64)
    ) {
        let dist = manhattan_distance(&a, &b);
        prop_assert!(dist >= 0.0, "Manhattan distance must be non-negative");
    }

    // Property: Dot product symmetry
    #[test]
    fn test_dot_product_symmetry(
        a in vector_strategy(64),
        b in vector_strategy(64)
    ) {
        let dist_ab = dot_product_distance(&a, &b);
        let dist_ba = dot_product_distance(&b, &a);
        prop_assert!((dist_ab - dist_ba).abs() < 0.01);
    }
}

// ============================================================================
// Quantization Round-Trip Properties
// ============================================================================

proptest! {
    // Property: Scalar quantization round-trip preserves approximate values
    #[test]
    fn test_scalar_quantization_roundtrip(
        v in prop::collection::vec(0.0f32..100.0f32, 64)
    ) {
        let quantized = ScalarQuantized::quantize(&v);
        let reconstructed = quantized.reconstruct();

        prop_assert_eq!(v.len(), reconstructed.len());

        // Check reconstruction error is bounded
        for (orig, recon) in v.iter().zip(reconstructed.iter()) {
            let error = (orig - recon).abs();
            let relative_error = if *orig != 0.0 {
                error / orig.abs()
            } else {
                error
            };
            prop_assert!(
                relative_error < 0.5 || error < 1.0,
                "Reconstruction error too large: {} vs {}",
                orig, recon
            );
        }
    }

    // Property: Binary quantization preserves signs
    #[test]
    fn test_binary_quantization_sign_preservation(
        v in prop::collection::vec(-10.0f32..10.0f32, 64)
            .prop_filter("No zeros", |v| v.iter().all(|x| *x != 0.0))
    ) {
        let quantized = BinaryQuantized::quantize(&v);
        let reconstructed = quantized.reconstruct();

        for (orig, recon) in v.iter().zip(reconstructed.iter()) {
            prop_assert_eq!(
                orig.signum(),
                *recon,
                "Sign not preserved for {} -> {}",
                orig, recon
            );
        }
    }

    // Property: Binary quantization distance to self is zero
    #[test]
    fn test_binary_quantization_self_distance(
        v in prop::collection::vec(-10.0f32..10.0f32, 64)
    ) {
        let quantized = BinaryQuantized::quantize(&v);
        let dist = quantized.distance(&quantized);
        prop_assert_eq!(dist, 0.0, "Distance to self should be 0");
    }

    // Property: Binary quantization distance is symmetric
    #[test]
    fn test_binary_quantization_symmetry(
        a in prop::collection::vec(-10.0f32..10.0f32, 64),
        b in prop::collection::vec(-10.0f32..10.0f32, 64)
    ) {
        let qa = BinaryQuantized::quantize(&a);
        let qb = BinaryQuantized::quantize(&b);

        let dist_ab = qa.distance(&qb);
        let dist_ba = qb.distance(&qa);

        prop_assert_eq!(dist_ab, dist_ba, "Distance should be symmetric");
    }

    // Property: Binary quantization distance is bounded
    #[test]
    fn test_binary_quantization_distance_bounded(
        a in prop::collection::vec(-10.0f32..10.0f32, 64),
        b in prop::collection::vec(-10.0f32..10.0f32, 64)
    ) {
        let qa = BinaryQuantized::quantize(&a);
        let qb = BinaryQuantized::quantize(&b);

        let dist = qa.distance(&qb);

        // Hamming distance for 64 bits should be in [0, 64]
        prop_assert!(dist >= 0.0 && dist <= 64.0, "Distance {} out of bounds", dist);
    }

    // Property: Scalar quantization distance is non-negative
    #[test]
    fn test_scalar_quantization_distance_non_negative(
        a in prop::collection::vec(0.0f32..100.0f32, 64),
        b in prop::collection::vec(0.0f32..100.0f32, 64)
    ) {
        let qa = ScalarQuantized::quantize(&a);
        let qb = ScalarQuantized::quantize(&b);

        let dist = qa.distance(&qb);
        prop_assert!(dist >= 0.0, "Distance must be non-negative");
    }
}

// ============================================================================
// Vector Operation Properties
// ============================================================================

proptest! {
    // Property: Scaling preserves direction for cosine similarity
    #[test]
    fn test_cosine_scale_invariance(
        v in normalized_vector_strategy(32),
        scale in 0.1f32..10.0f32
    ) {
        let scaled: Vec<f32> = v.iter().map(|x| x * scale).collect();
        let dist_original = cosine_distance(&v, &v);
        let dist_scaled = cosine_distance(&v, &scaled);

        // Cosine distance should be approximately the same (scale invariant)
        prop_assert!(
            (dist_original - dist_scaled).abs() < 0.1,
            "Cosine distance should be scale invariant: {} vs {}",
            dist_original, dist_scaled
        );
    }

    // Property: Adding the same vector to both preserves distance
    #[test]
    fn test_euclidean_translation_invariance(
        a in vector_strategy(32),
        b in vector_strategy(32),
        offset in vector_strategy(32)
    ) {
        let a_offset: Vec<f32> = a.iter().zip(&offset).map(|(x, o)| x + o).collect();
        let b_offset: Vec<f32> = b.iter().zip(&offset).map(|(x, o)| x + o).collect();

        let dist_original = euclidean_distance(&a, &b);
        let dist_offset = euclidean_distance(&a_offset, &b_offset);

        prop_assert!(
            (dist_original - dist_offset).abs() < 0.01,
            "Euclidean distance should be translation invariant"
        );
    }
}

// ============================================================================
// Batch Operations Properties
// ============================================================================

proptest! {
    // Property: Batch distance calculation consistency
    #[test]
    fn test_batch_distances_consistency(
        query in vector_strategy(32),
        vectors in prop::collection::vec(vector_strategy(32), 10..20)
    ) {
        // Calculate distances in batch
        let batch_dists = batch_distances(&query, &vectors, DistanceMetric::Euclidean).unwrap();

        // Calculate distances individually
        let individual_dists: Vec<f32> = vectors.iter()
            .map(|v| euclidean_distance(&query, v))
            .collect();

        prop_assert_eq!(batch_dists.len(), individual_dists.len());

        for (batch, individual) in batch_dists.iter().zip(individual_dists.iter()) {
            prop_assert!(
                (batch - individual).abs() < 0.01,
                "Batch and individual distances should match: {} vs {}",
                batch, individual
            );
        }
    }
}

// ============================================================================
// Dimension Handling Properties
// ============================================================================

proptest! {
    // Property: Distance calculation fails on dimension mismatch
    #[test]
    fn test_dimension_mismatch_error(
        dim1 in 1usize..100,
        dim2 in 1usize..100
    ) {
        prop_assume!(dim1 != dim2); // Only test when dimensions differ

        let a = vec![1.0f32; dim1];
        let b = vec![1.0f32; dim2];

        let result = distance(&a, &b, DistanceMetric::Euclidean);
        prop_assert!(result.is_err(), "Should error on dimension mismatch");
    }

    // Property: Distance calculation succeeds on matching dimensions
    #[test]
    fn test_dimension_match_success(
        dim in 1usize..200,
        a in prop::collection::vec(any::<f32>().prop_filter("Must be finite", |x| x.is_finite()), 1..200),
        b in prop::collection::vec(any::<f32>().prop_filter("Must be finite", |x| x.is_finite()), 1..200)
    ) {
        // Ensure same dimensions
        let a_resized = vec![1.0f32; dim];
        let b_resized = vec![1.0f32; dim];

        let result = distance(&a_resized, &b_resized, DistanceMetric::Euclidean);
        prop_assert!(result.is_ok(), "Should succeed on matching dimensions");
    }
}
