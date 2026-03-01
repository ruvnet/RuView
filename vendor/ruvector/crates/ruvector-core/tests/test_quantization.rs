//! Quantization Accuracy Tests
//!
//! This module provides comprehensive tests for quantization techniques,
//! verifying accuracy, compression ratios, and distance calculations.

use ruvector_core::quantization::*;

// ============================================================================
// Scalar Quantization Tests
// ============================================================================

mod scalar_quantization_tests {
    use super::*;

    #[test]
    fn test_scalar_quantization_basic() {
        let vector = vec![0.0, 0.5, 1.0, 1.5, 2.0];
        let quantized = ScalarQuantized::quantize(&vector);

        assert_eq!(quantized.data.len(), 5);
        assert!(quantized.scale > 0.0, "Scale should be positive");
    }

    #[test]
    fn test_scalar_quantization_min_max() {
        let vector = vec![-10.0, -5.0, 0.0, 5.0, 10.0];
        let quantized = ScalarQuantized::quantize(&vector);

        // Min should be -10.0
        assert!((quantized.min - (-10.0)).abs() < 0.001);

        // Scale should map range 20 to 255
        let expected_scale = 20.0 / 255.0;
        assert!(
            (quantized.scale - expected_scale).abs() < 0.001,
            "Scale mismatch: expected {}, got {}",
            expected_scale,
            quantized.scale
        );
    }

    #[test]
    fn test_scalar_quantization_reconstruction_accuracy() {
        let test_vectors = vec![
            vec![1.0, 2.0, 3.0, 4.0, 5.0],
            vec![0.0, 0.25, 0.5, 0.75, 1.0],
            vec![-100.0, 0.0, 100.0],
            vec![0.001, 0.002, 0.003, 0.004, 0.005],
        ];

        for vector in test_vectors {
            let quantized = ScalarQuantized::quantize(&vector);
            let reconstructed = quantized.reconstruct();

            assert_eq!(vector.len(), reconstructed.len());

            // Calculate max error based on range
            let min = vector.iter().copied().fold(f32::INFINITY, f32::min);
            let max = vector.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            let max_allowed_error = (max - min) / 128.0; // Allow 2 quantization steps error

            for (orig, recon) in vector.iter().zip(reconstructed.iter()) {
                let error = (orig - recon).abs();
                assert!(
                    error <= max_allowed_error,
                    "Reconstruction error {} exceeds max {} for value {}",
                    error,
                    max_allowed_error,
                    orig
                );
            }
        }
    }

    #[test]
    fn test_scalar_quantization_constant_values() {
        let constant = vec![5.0, 5.0, 5.0, 5.0, 5.0];
        let quantized = ScalarQuantized::quantize(&constant);
        let reconstructed = quantized.reconstruct();

        for (orig, recon) in constant.iter().zip(reconstructed.iter()) {
            assert!(
                (orig - recon).abs() < 0.1,
                "Constant value reconstruction failed"
            );
        }
    }

    #[test]
    fn test_scalar_quantization_distance_self() {
        let vector = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let quantized = ScalarQuantized::quantize(&vector);

        let distance = quantized.distance(&quantized);
        assert!(
            distance < 0.001,
            "Distance to self should be ~0, got {}",
            distance
        );
    }

    #[test]
    fn test_scalar_quantization_distance_symmetry() {
        let v1 = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let v2 = vec![5.0, 4.0, 3.0, 2.0, 1.0];

        let q1 = ScalarQuantized::quantize(&v1);
        let q2 = ScalarQuantized::quantize(&v2);

        let dist_ab = q1.distance(&q2);
        let dist_ba = q2.distance(&q1);

        assert!(
            (dist_ab - dist_ba).abs() < 0.1,
            "Distance not symmetric: {} vs {}",
            dist_ab,
            dist_ba
        );
    }

    #[test]
    fn test_scalar_quantization_distance_triangle_inequality() {
        let v1 = vec![1.0, 0.0, 0.0, 0.0];
        let v2 = vec![0.0, 1.0, 0.0, 0.0];
        let v3 = vec![0.0, 0.0, 1.0, 0.0];

        let q1 = ScalarQuantized::quantize(&v1);
        let q2 = ScalarQuantized::quantize(&v2);
        let q3 = ScalarQuantized::quantize(&v3);

        let d12 = q1.distance(&q2);
        let d23 = q2.distance(&q3);
        let d13 = q1.distance(&q3);

        // Triangle inequality: d(1,3) <= d(1,2) + d(2,3)
        // Allow some slack for quantization errors
        assert!(
            d13 <= d12 + d23 + 0.5,
            "Triangle inequality violated: {} > {} + {}",
            d13,
            d12,
            d23
        );
    }

    #[test]
    fn test_scalar_quantization_common_embedding_sizes() {
        for dim in [128, 256, 384, 512, 768, 1024, 1536, 2048] {
            let vector: Vec<f32> = (0..dim).map(|i| (i as f32) * 0.01).collect();
            let quantized = ScalarQuantized::quantize(&vector);
            let reconstructed = quantized.reconstruct();

            assert_eq!(quantized.data.len(), dim);
            assert_eq!(reconstructed.len(), dim);

            // Verify compression ratio (4x for f32 -> u8)
            let original_size = dim * std::mem::size_of::<f32>();
            let quantized_size = quantized.data.len() + std::mem::size_of::<f32>() * 2; // data + min + scale
            assert!(
                quantized_size < original_size,
                "No compression achieved for dim {}",
                dim
            );
        }
    }

    #[test]
    fn test_scalar_quantization_extreme_values() {
        // Test with large values
        let large = vec![1e10, 2e10, 3e10];
        let quantized = ScalarQuantized::quantize(&large);
        let reconstructed = quantized.reconstruct();

        for (orig, recon) in large.iter().zip(reconstructed.iter()) {
            let relative_error = (orig - recon).abs() / orig.abs();
            assert!(
                relative_error < 0.02,
                "Large value reconstruction error too high: {}",
                relative_error
            );
        }

        // Test with small values
        let small = vec![1e-5, 2e-5, 3e-5, 4e-5, 5e-5];
        let quantized = ScalarQuantized::quantize(&small);
        let reconstructed = quantized.reconstruct();

        for (orig, recon) in small.iter().zip(reconstructed.iter()) {
            let error = (orig - recon).abs();
            let range = 4e-5;
            assert!(
                error < range / 100.0,
                "Small value reconstruction error too high: {}",
                error
            );
        }
    }

    #[test]
    fn test_scalar_quantization_negative_values() {
        let negative = vec![-5.0, -4.0, -3.0, -2.0, -1.0];
        let quantized = ScalarQuantized::quantize(&negative);
        let reconstructed = quantized.reconstruct();

        for (orig, recon) in negative.iter().zip(reconstructed.iter()) {
            assert!(
                (orig - recon).abs() < 0.1,
                "Negative value reconstruction failed: {} vs {}",
                orig,
                recon
            );
        }
    }
}

// ============================================================================
// Binary Quantization Tests
// ============================================================================

mod binary_quantization_tests {
    use super::*;

    #[test]
    fn test_binary_quantization_basic() {
        let vector = vec![1.0, -1.0, 0.5, -0.5, 0.1];
        let quantized = BinaryQuantized::quantize(&vector);

        assert_eq!(quantized.dimensions, 5);
        assert_eq!(quantized.bits.len(), 1); // 5 bits fit in 1 byte
    }

    #[test]
    fn test_binary_quantization_packing() {
        // Test byte packing
        for dim in 1..=32 {
            let vector: Vec<f32> = (0..dim)
                .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
                .collect();
            let quantized = BinaryQuantized::quantize(&vector);

            let expected_bytes = (dim + 7) / 8;
            assert_eq!(
                quantized.bits.len(),
                expected_bytes,
                "Wrong byte count for dim {}",
                dim
            );
            assert_eq!(quantized.dimensions, dim);
        }
    }

    #[test]
    fn test_binary_quantization_sign_preservation() {
        let test_vectors = vec![
            vec![1.0, -1.0, 2.0, -2.0],
            vec![0.001, -0.001, 100.0, -100.0],
            vec![f32::MAX / 2.0, f32::MIN / 2.0],
        ];

        for vector in test_vectors {
            let quantized = BinaryQuantized::quantize(&vector);
            let reconstructed = quantized.reconstruct();

            for (orig, recon) in vector.iter().zip(reconstructed.iter()) {
                if *orig > 0.0 {
                    assert_eq!(*recon, 1.0, "Positive value should reconstruct to 1.0");
                } else if *orig < 0.0 {
                    assert_eq!(*recon, -1.0, "Negative value should reconstruct to -1.0");
                }
            }
        }
    }

    #[test]
    fn test_binary_quantization_zero_handling() {
        let vector = vec![0.0, 0.0, 0.0, 0.0];
        let quantized = BinaryQuantized::quantize(&vector);
        let reconstructed = quantized.reconstruct();

        // Zero maps to negative bit (0), which reconstructs to -1.0
        for val in reconstructed {
            assert_eq!(val, -1.0);
        }
    }

    #[test]
    fn test_binary_quantization_hamming_distance() {
        // Test specific Hamming distance cases
        let cases = vec![
            // (v1, v2, expected_distance)
            (vec![1.0, 1.0, 1.0, 1.0], vec![1.0, 1.0, 1.0, 1.0], 0.0), // identical
            (vec![1.0, 1.0, 1.0, 1.0], vec![-1.0, -1.0, -1.0, -1.0], 4.0), // opposite
            (vec![1.0, 1.0, -1.0, -1.0], vec![1.0, -1.0, -1.0, 1.0], 2.0), // 2 bits differ
            (vec![1.0, -1.0, 1.0, -1.0], vec![-1.0, 1.0, -1.0, 1.0], 4.0), // all differ
        ];

        for (v1, v2, expected) in cases {
            let q1 = BinaryQuantized::quantize(&v1);
            let q2 = BinaryQuantized::quantize(&v2);

            let distance = q1.distance(&q2);
            assert!(
                (distance - expected).abs() < 0.001,
                "Hamming distance mismatch: expected {}, got {}",
                expected,
                distance
            );
        }
    }

    #[test]
    fn test_binary_quantization_distance_symmetry() {
        let v1 = vec![1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0];
        let v2 = vec![-1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0];

        let q1 = BinaryQuantized::quantize(&v1);
        let q2 = BinaryQuantized::quantize(&v2);

        let d12 = q1.distance(&q2);
        let d21 = q2.distance(&q1);

        assert_eq!(d12, d21, "Binary distance should be symmetric");
    }

    #[test]
    fn test_binary_quantization_distance_bounds() {
        for dim in [8, 16, 32, 64, 128, 256] {
            let v1: Vec<f32> = (0..dim)
                .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
                .collect();
            let v2: Vec<f32> = (0..dim)
                .map(|i| if i % 3 == 0 { 1.0 } else { -1.0 })
                .collect();

            let q1 = BinaryQuantized::quantize(&v1);
            let q2 = BinaryQuantized::quantize(&v2);

            let distance = q1.distance(&q2);

            // Distance should be in [0, dim]
            assert!(
                distance >= 0.0 && distance <= dim as f32,
                "Distance {} out of bounds [0, {}]",
                distance,
                dim
            );
        }
    }

    #[test]
    fn test_binary_quantization_compression_ratio() {
        for dim in [128, 256, 512, 1024] {
            let vector: Vec<f32> = (0..dim)
                .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
                .collect();
            let quantized = BinaryQuantized::quantize(&vector);

            // f32 to 1 bit = theoretical 32x compression for data only
            // Actual ratio depends on overhead but should be significant
            let original_data_size = dim * std::mem::size_of::<f32>();
            let quantized_data_size = quantized.bits.len();

            let data_compression_ratio = original_data_size as f32 / quantized_data_size as f32;
            assert!(
                data_compression_ratio >= 31.0,
                "Data compression ratio {} less than expected ~32x for dim {}",
                data_compression_ratio,
                dim
            );

            // Verify bits.len() is correct: ceil(dim / 8)
            assert_eq!(quantized.bits.len(), (dim + 7) / 8);
        }
    }

    #[test]
    fn test_binary_quantization_common_embedding_sizes() {
        for dim in [128, 256, 384, 512, 768, 1024, 1536, 2048] {
            let vector: Vec<f32> = (0..dim).map(|i| (i as f32 - dim as f32 / 2.0)).collect();
            let quantized = BinaryQuantized::quantize(&vector);
            let reconstructed = quantized.reconstruct();

            assert_eq!(reconstructed.len(), dim);

            // Check all values are +1 or -1
            for val in &reconstructed {
                assert!(*val == 1.0 || *val == -1.0);
            }
        }
    }
}

// ============================================================================
// Product Quantization Tests
// ============================================================================

mod product_quantization_tests {
    use super::*;

    #[test]
    fn test_product_quantization_training() {
        let vectors: Vec<Vec<f32>> = (0..100)
            .map(|i| (0..32).map(|j| (i * 32 + j) as f32 * 0.01).collect())
            .collect();

        let num_subspaces = 4;
        let codebook_size = 16;

        let pq = ProductQuantized::train(&vectors, num_subspaces, codebook_size, 10).unwrap();

        assert_eq!(pq.codebooks.len(), num_subspaces);
        for codebook in &pq.codebooks {
            assert_eq!(codebook.len(), codebook_size);
        }
    }

    #[test]
    fn test_product_quantization_encode() {
        let vectors: Vec<Vec<f32>> = (0..100)
            .map(|i| (0..32).map(|j| (i * 32 + j) as f32 * 0.01).collect())
            .collect();

        let num_subspaces = 4;
        let codebook_size = 16;

        let pq = ProductQuantized::train(&vectors, num_subspaces, codebook_size, 10).unwrap();

        let test_vector: Vec<f32> = (0..32).map(|i| i as f32 * 0.02).collect();
        let codes = pq.encode(&test_vector);

        assert_eq!(codes.len(), num_subspaces);
        for code in &codes {
            assert!(*code < codebook_size as u8);
        }
    }

    #[test]
    fn test_product_quantization_empty_input_error() {
        let result = ProductQuantized::train(&[], 4, 16, 10);
        assert!(result.is_err());
    }

    #[test]
    fn test_product_quantization_codebook_size_limit() {
        let vectors: Vec<Vec<f32>> = (0..10)
            .map(|i| (0..16).map(|j| (i * 16 + j) as f32).collect())
            .collect();

        // Codebook size > 256 should error
        let result = ProductQuantized::train(&vectors, 4, 300, 10);
        assert!(result.is_err());
    }

    #[test]
    fn test_product_quantization_various_subspaces() {
        let dim = 64;
        let vectors: Vec<Vec<f32>> = (0..200)
            .map(|i| (0..dim).map(|j| (i * dim + j) as f32 * 0.001).collect())
            .collect();

        for num_subspaces in [1, 2, 4, 8, 16] {
            let pq = ProductQuantized::train(&vectors, num_subspaces, 16, 5).unwrap();

            assert_eq!(pq.codebooks.len(), num_subspaces);

            let subspace_dim = dim / num_subspaces;
            for codebook in &pq.codebooks {
                for centroid in codebook {
                    assert_eq!(centroid.len(), subspace_dim);
                }
            }
        }
    }
}

// ============================================================================
// Comparative Tests
// ============================================================================

mod comparative_tests {
    use super::*;

    #[test]
    fn test_scalar_vs_binary_reconstruction() {
        let vector = vec![1.0, -2.0, 3.0, -4.0, 5.0, -6.0, 7.0, -8.0];

        let scalar = ScalarQuantized::quantize(&vector);
        let binary = BinaryQuantized::quantize(&vector);

        let scalar_recon = scalar.reconstruct();
        let binary_recon = binary.reconstruct();

        // Scalar should have better accuracy
        let scalar_error: f32 = vector
            .iter()
            .zip(scalar_recon.iter())
            .map(|(o, r)| (o - r).abs())
            .sum::<f32>()
            / vector.len() as f32;

        // Binary only preserves sign
        for (orig, recon) in vector.iter().zip(binary_recon.iter()) {
            assert_eq!(orig.signum(), recon.signum());
        }

        // Scalar error should be small
        assert!(
            scalar_error < 0.5,
            "Scalar reconstruction error {} too high",
            scalar_error
        );
    }

    #[test]
    fn test_quantization_preserves_relative_ordering() {
        // Test that vectors closest in original space are also closest in quantized space
        let v1 = vec![1.0, 0.0, 0.0, 0.0];
        let v2 = vec![0.9, 0.1, 0.0, 0.0]; // close to v1
        let v3 = vec![0.0, 0.0, 0.0, 1.0]; // far from v1

        // For scalar quantization
        let q1_s = ScalarQuantized::quantize(&v1);
        let q2_s = ScalarQuantized::quantize(&v2);
        let q3_s = ScalarQuantized::quantize(&v3);

        let d12_s = q1_s.distance(&q2_s);
        let d13_s = q1_s.distance(&q3_s);

        // v2 should be closer to v1 than v3
        assert!(
            d12_s < d13_s,
            "Scalar: v2 should be closer to v1 than v3: {} vs {}",
            d12_s,
            d13_s
        );

        // For binary quantization
        let q1_b = BinaryQuantized::quantize(&v1);
        let q2_b = BinaryQuantized::quantize(&v2);
        let q3_b = BinaryQuantized::quantize(&v3);

        let d12_b = q1_b.distance(&q2_b);
        let d13_b = q1_b.distance(&q3_b);

        // Same relative ordering should hold
        assert!(
            d12_b <= d13_b,
            "Binary: v2 should be at most as far as v3: {} vs {}",
            d12_b,
            d13_b
        );
    }

    #[test]
    fn test_compression_ratios() {
        let dim = 512;
        let vector: Vec<f32> = (0..dim).map(|i| i as f32 * 0.01).collect();

        // Original size
        let original_size = dim * std::mem::size_of::<f32>(); // 2048 bytes

        // Scalar quantization: u8 per element + 2 floats for min/scale
        let scalar = ScalarQuantized::quantize(&vector);
        let scalar_size = scalar.data.len() + 2 * std::mem::size_of::<f32>(); // ~520 bytes
        let scalar_ratio = original_size as f32 / scalar_size as f32;

        // Binary quantization: 1 bit per element + usize for dimensions
        let binary = BinaryQuantized::quantize(&vector);
        let binary_size = binary.bits.len() + std::mem::size_of::<usize>(); // ~72 bytes
        let binary_ratio = original_size as f32 / binary_size as f32;

        println!("Original: {} bytes", original_size);
        println!(
            "Scalar: {} bytes ({:.1}x compression)",
            scalar_size, scalar_ratio
        );
        println!(
            "Binary: {} bytes ({:.1}x compression)",
            binary_size, binary_ratio
        );

        // Verify expected ratios
        assert!(scalar_ratio > 3.5, "Scalar should achieve ~4x compression");
        assert!(
            binary_ratio > 25.0,
            "Binary should achieve ~32x compression"
        );
    }
}

// ============================================================================
// Edge Cases and Error Handling
// ============================================================================

mod edge_cases {
    use super::*;

    #[test]
    fn test_single_element_vector() {
        let vector = vec![42.0];

        let scalar = ScalarQuantized::quantize(&vector);
        let binary = BinaryQuantized::quantize(&vector);

        assert_eq!(scalar.data.len(), 1);
        assert_eq!(binary.bits.len(), 1);
        assert_eq!(binary.dimensions, 1);
    }

    #[test]
    fn test_large_vector() {
        let dim = 8192;
        let vector: Vec<f32> = (0..dim).map(|i| (i as f32).sin()).collect();

        let scalar = ScalarQuantized::quantize(&vector);
        let binary = BinaryQuantized::quantize(&vector);

        assert_eq!(scalar.data.len(), dim);
        assert_eq!(binary.dimensions, dim);
    }

    #[test]
    fn test_all_positive() {
        let vector = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let binary = BinaryQuantized::quantize(&vector);
        let reconstructed = binary.reconstruct();

        // All values should reconstruct to 1.0
        for val in reconstructed {
            assert_eq!(val, 1.0);
        }
    }

    #[test]
    fn test_all_negative() {
        let vector = vec![-1.0, -2.0, -3.0, -4.0, -5.0, -6.0, -7.0, -8.0];
        let binary = BinaryQuantized::quantize(&vector);
        let reconstructed = binary.reconstruct();

        // All values should reconstruct to -1.0
        for val in reconstructed {
            assert_eq!(val, -1.0);
        }
    }

    #[test]
    fn test_alternating_pattern() {
        let vector: Vec<f32> = (0..100)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();

        let binary = BinaryQuantized::quantize(&vector);
        let reconstructed = binary.reconstruct();

        for (i, val) in reconstructed.iter().enumerate() {
            let expected = if i % 2 == 0 { 1.0 } else { -1.0 };
            assert_eq!(*val, expected);
        }
    }

    #[test]
    fn test_quantization_deterministic() {
        let vector = vec![1.0, 2.0, 3.0, 4.0, 5.0];

        // Quantize multiple times - should get same result
        let q1 = ScalarQuantized::quantize(&vector);
        let q2 = ScalarQuantized::quantize(&vector);

        assert_eq!(q1.data, q2.data);
        assert_eq!(q1.min, q2.min);
        assert_eq!(q1.scale, q2.scale);
    }
}

// ============================================================================
// Performance Characteristic Tests
// ============================================================================

mod performance_tests {
    use super::*;

    #[test]
    fn test_scalar_quantization_speed() {
        let vector: Vec<f32> = (0..1024).map(|i| i as f32 * 0.001).collect();

        let start = std::time::Instant::now();

        for _ in 0..10000 {
            let _ = ScalarQuantized::quantize(&vector);
        }

        let duration = start.elapsed();
        let ops_per_sec = 10000.0 / duration.as_secs_f64();

        println!(
            "Scalar quantization: {:.0} ops/sec for 1024-dim vectors",
            ops_per_sec
        );

        // Should be fast
        assert!(
            duration.as_millis() < 5000,
            "Scalar quantization too slow: {:?}",
            duration
        );
    }

    #[test]
    fn test_binary_quantization_speed() {
        let vector: Vec<f32> = (0..1024).map(|i| i as f32 * 0.001).collect();

        let start = std::time::Instant::now();

        for _ in 0..10000 {
            let _ = BinaryQuantized::quantize(&vector);
        }

        let duration = start.elapsed();
        let ops_per_sec = 10000.0 / duration.as_secs_f64();

        println!(
            "Binary quantization: {:.0} ops/sec for 1024-dim vectors",
            ops_per_sec
        );

        // Should be fast
        assert!(
            duration.as_millis() < 5000,
            "Binary quantization too slow: {:?}",
            duration
        );
    }

    #[test]
    fn test_distance_calculation_speed() {
        let v1: Vec<f32> = (0..512).map(|i| i as f32 * 0.01).collect();
        let v2: Vec<f32> = (0..512).map(|i| (i as f32 * 0.01) + 0.5).collect();

        let q1_s = ScalarQuantized::quantize(&v1);
        let q2_s = ScalarQuantized::quantize(&v2);

        let q1_b = BinaryQuantized::quantize(&v1);
        let q2_b = BinaryQuantized::quantize(&v2);

        // Scalar distance
        let start = std::time::Instant::now();
        for _ in 0..100000 {
            let _ = q1_s.distance(&q2_s);
        }
        let scalar_duration = start.elapsed();

        // Binary distance (Hamming)
        let start = std::time::Instant::now();
        for _ in 0..100000 {
            let _ = q1_b.distance(&q2_b);
        }
        let binary_duration = start.elapsed();

        println!("Scalar distance: {:?} for 100k ops", scalar_duration);
        println!("Binary distance: {:?} for 100k ops", binary_duration);

        // Binary should be faster (just XOR and popcount)
        // But both should be fast
        assert!(scalar_duration.as_millis() < 1000);
        assert!(binary_duration.as_millis() < 1000);
    }
}
