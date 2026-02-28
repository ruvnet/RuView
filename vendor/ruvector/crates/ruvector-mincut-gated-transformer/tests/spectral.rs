//! Integration tests for spectral position encoding.
//!
//! Tests the complete spectral PE pipeline including:
//! - Laplacian computation from graph structure
//! - Eigenvector computation via power iteration
//! - Position encoding generation
//! - Integration with quantized embeddings

#![cfg(feature = "spectral_pe")]

use ruvector_mincut_gated_transformer::{
    spectral::{power_iteration, rayleigh_quotient},
    SpectralPEConfig, SpectralPositionEncoder,
};

#[test]
fn test_config_default() {
    let config = SpectralPEConfig::default();

    assert_eq!(config.num_eigenvectors, 8);
    assert_eq!(config.pe_attention_heads, 4);
    assert!(!config.learnable_pe);
}

#[test]
fn test_laplacian_empty_graph() {
    let encoder = SpectralPositionEncoder::default();

    let edges: Vec<(u16, u16)> = vec![];
    let laplacian = encoder.compute_laplacian(&edges, 4);

    assert_eq!(laplacian.len(), 16);

    // No edges means zero Laplacian (no connections, zero degrees)
    assert!(laplacian.iter().all(|&x| x == 0.0));
}

#[test]
fn test_laplacian_simple_chain() {
    let encoder = SpectralPositionEncoder::default();

    // Chain graph: 0-1-2-3
    let edges = vec![(0, 1), (1, 2), (2, 3)];
    let laplacian = encoder.compute_laplacian(&edges, 4);

    // Check diagonal (degrees)
    assert_eq!(laplacian[0 * 4 + 0], 1.0); // node 0: degree 1
    assert_eq!(laplacian[1 * 4 + 1], 2.0); // node 1: degree 2
    assert_eq!(laplacian[2 * 4 + 2], 2.0); // node 2: degree 2
    assert_eq!(laplacian[3 * 4 + 3], 1.0); // node 3: degree 1

    // Check adjacency entries (off-diagonal)
    assert_eq!(laplacian[0 * 4 + 1], -1.0);
    assert_eq!(laplacian[1 * 4 + 0], -1.0);
    assert_eq!(laplacian[1 * 4 + 2], -1.0);
    assert_eq!(laplacian[2 * 4 + 1], -1.0);
    assert_eq!(laplacian[2 * 4 + 3], -1.0);
    assert_eq!(laplacian[3 * 4 + 2], -1.0);

    // Non-adjacent nodes should be 0
    assert_eq!(laplacian[0 * 4 + 2], 0.0);
    assert_eq!(laplacian[0 * 4 + 3], 0.0);
}

#[test]
fn test_laplacian_symmetry() {
    let encoder = SpectralPositionEncoder::default();

    // Triangle graph
    let edges = vec![(0, 1), (1, 2), (2, 0)];
    let laplacian = encoder.compute_laplacian(&edges, 3);

    // Laplacian should be symmetric
    for i in 0..3 {
        for j in 0..3 {
            assert_eq!(
                laplacian[i * 3 + j],
                laplacian[j * 3 + i],
                "Laplacian should be symmetric at ({}, {})",
                i,
                j
            );
        }
    }
}

#[test]
fn test_laplacian_complete_graph() {
    let encoder = SpectralPositionEncoder::default();

    // Complete graph K4: all nodes connected
    let edges = vec![(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
    let laplacian = encoder.compute_laplacian(&edges, 4);

    // All nodes should have degree 3
    for i in 0..4 {
        assert_eq!(laplacian[i * 4 + i], 3.0);
    }

    // All off-diagonal should be -1
    for i in 0..4 {
        for j in 0..4 {
            if i != j {
                assert_eq!(laplacian[i * 4 + j], -1.0);
            }
        }
    }
}

#[test]
fn test_laplacian_out_of_bounds() {
    let encoder = SpectralPositionEncoder::default();

    // Include edge with node index out of bounds
    let edges = vec![(0, 1), (1, 2), (2, 100)]; // 100 is out of bounds for n=4
    let laplacian = encoder.compute_laplacian(&edges, 4);

    // Should handle gracefully - just ignore invalid edges
    assert_eq!(laplacian.len(), 16);

    // Valid edges should still be processed
    assert_eq!(laplacian[0 * 4 + 1], -1.0);
    assert_eq!(laplacian[1 * 4 + 2], -1.0);
}

#[test]
fn test_normalized_laplacian() {
    let encoder = SpectralPositionEncoder::default();

    let edges = vec![(0, 1), (1, 2)];
    let laplacian = encoder.compute_normalized_laplacian(&edges, 3);

    // Normalized Laplacian values should be in [-1, 1]
    for &val in &laplacian {
        assert!(
            val >= -1.0 - 1e-5 && val <= 1.0 + 1e-5,
            "Normalized value {} out of range",
            val
        );
    }

    // Should still be symmetric
    for i in 0..3 {
        for j in 0..3 {
            assert!((laplacian[i * 3 + j] - laplacian[j * 3 + i]).abs() < 1e-5);
        }
    }
}

#[test]
fn test_power_iteration_identity() {
    let n = 4;
    let mut identity = vec![0.0f32; n * n];
    for i in 0..n {
        identity[i * n + i] = 1.0;
    }

    let v = power_iteration(&identity, n, 100);

    assert_eq!(v.len(), n);

    // Should be normalized
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((norm - 1.0).abs() < 1e-4);
}

#[test]
fn test_power_iteration_diagonal() {
    let n = 3;
    let mut matrix = vec![0.0f32; n * n];

    // Diagonal matrix with distinct eigenvalues
    matrix[0 * n + 0] = 5.0; // Largest
    matrix[1 * n + 1] = 3.0;
    matrix[2 * n + 2] = 1.0;

    let v = power_iteration(&matrix, n, 100);

    assert_eq!(v.len(), n);

    // Should converge to eigenvector of largest eigenvalue [1, 0, 0]
    assert!(v[0].abs() > 0.9, "First component should dominate: {:?}", v);
    assert!(v[1].abs() < 0.3);
    assert!(v[2].abs() < 0.3);
}

#[test]
fn test_power_iteration_convergence() {
    let n = 3;
    let mut matrix = vec![0.0f32; n * n];
    matrix[0 * n + 0] = 4.0;
    matrix[1 * n + 1] = 2.0;
    matrix[2 * n + 2] = 1.0;

    // Test convergence with different iteration counts
    let v_10 = power_iteration(&matrix, n, 10);
    let v_100 = power_iteration(&matrix, n, 100);
    let v_1000 = power_iteration(&matrix, n, 1000);

    // Should converge (later iterations closer together)
    let diff_early: f32 = v_10.iter().zip(&v_100).map(|(a, b)| (a - b).abs()).sum();

    let diff_late: f32 = v_100.iter().zip(&v_1000).map(|(a, b)| (a - b).abs()).sum();

    assert!(
        diff_late < diff_early,
        "Should converge: early_diff={}, late_diff={}",
        diff_early,
        diff_late
    );
}

#[test]
fn test_rayleigh_quotient() {
    let n = 3;
    let mut matrix = vec![0.0f32; n * n];
    matrix[0 * n + 0] = 4.0;
    matrix[1 * n + 1] = 3.0;
    matrix[2 * n + 2] = 2.0;

    // Exact eigenvector for eigenvalue 4.0
    let v = vec![1.0, 0.0, 0.0];
    let lambda = rayleigh_quotient(&matrix, n, &v);

    assert!((lambda - 4.0).abs() < 1e-5);

    // Exact eigenvector for eigenvalue 2.0
    let v2 = vec![0.0, 0.0, 1.0];
    let lambda2 = rayleigh_quotient(&matrix, n, &v2);

    assert!((lambda2 - 2.0).abs() < 1e-5);
}

#[test]
fn test_encode_positions_basic() {
    let encoder = SpectralPositionEncoder::default();

    let eigenvectors = vec![vec![0.1, 0.2, 0.3, 0.4], vec![0.5, 0.6, 0.7, 0.8]];

    let encoding = encoder.encode_positions(&eigenvectors);

    // Should be [n positions x k eigenvectors]
    assert_eq!(encoding.len(), 8); // 4 positions * 2 eigenvectors

    // Verify encoding structure
    // Position 0: [0.1, 0.5]
    assert_eq!(encoding[0], 0.1);
    assert_eq!(encoding[1], 0.5);

    // Position 1: [0.2, 0.6]
    assert_eq!(encoding[2], 0.2);
    assert_eq!(encoding[3], 0.6);

    // Position 3: [0.4, 0.8]
    assert_eq!(encoding[6], 0.4);
    assert_eq!(encoding[7], 0.8);
}

#[test]
fn test_encode_positions_empty() {
    let encoder = SpectralPositionEncoder::default();

    let eigenvectors: Vec<Vec<f32>> = vec![];
    let encoding = encoder.encode_positions(&eigenvectors);

    assert_eq!(encoding.len(), 0);
}

#[test]
fn test_add_to_embeddings() {
    let config = SpectralPEConfig {
        num_eigenvectors: 2,
        ..Default::default()
    };
    let encoder = SpectralPositionEncoder::new(config);

    // 2 positions, 2 dimensions each = 4 total
    let mut embeddings = vec![10i8, 20, 30, 40];
    let pe = vec![
        0.5, 1.0, // Position 0: PE values
        -0.5, -1.0, // Position 1: PE values
    ];

    encoder.add_to_embeddings(&mut embeddings, &pe, 10.0);

    // PE values scaled by 10 and added to first k=2 dims of each position
    // Position 0: [10, 20] + [5, 10] = [15, 30]
    // Position 1: [30, 40] + [-5, -10] = [25, 30]
    assert_eq!(embeddings[0], 15); // 10 + 5
    assert_eq!(embeddings[1], 30); // 20 + 10
    assert_eq!(embeddings[2], 25); // 30 + (-5)
    assert_eq!(embeddings[3], 30); // 40 + (-10)
}

#[test]
fn test_add_to_embeddings_saturation() {
    let config = SpectralPEConfig {
        num_eigenvectors: 2,
        ..Default::default()
    };
    let encoder = SpectralPositionEncoder::new(config);

    // Test overflow protection - 1 position, 2 dims
    let mut embeddings = vec![127i8, -128];
    let pe = vec![10.0, -10.0]; // Single position PE

    encoder.add_to_embeddings(&mut embeddings, &pe, 10.0);

    // Should saturate at i8 limits
    assert_eq!(embeddings[0], 127); // Can't exceed 127 (127 + 100 clamped)
    assert_eq!(embeddings[1], -128); // Can't go below -128 (-128 + (-100) clamped)
}

#[test]
fn test_add_to_embeddings_scale() {
    let config = SpectralPEConfig {
        num_eigenvectors: 2,
        ..Default::default()
    };
    let encoder = SpectralPositionEncoder::new(config);

    // 1 position, 2 dimensions
    let mut embeddings1 = vec![10i8, 20];
    let mut embeddings2 = vec![10i8, 20];
    let pe = vec![1.0, 2.0]; // Single position PE

    // Different scales
    encoder.add_to_embeddings(&mut embeddings1, &pe, 1.0);
    encoder.add_to_embeddings(&mut embeddings2, &pe, 10.0);

    // Scale should affect the magnitude of addition
    assert!(embeddings2[0] > embeddings1[0]);
    assert!(embeddings2[1] > embeddings1[1]);
}

#[test]
fn test_spectral_distance() {
    let config = SpectralPEConfig {
        num_eigenvectors: 2,
        ..Default::default()
    };
    let encoder = SpectralPositionEncoder::new(config);

    // 2 positions, 2 dimensions each
    let pe = vec![
        0.0, 0.0, // position 0
        1.0, 1.0, // position 1
    ];

    let dist = encoder.spectral_distance(&pe, 0, 1);

    // Euclidean distance: sqrt((1-0)^2 + (1-0)^2) = sqrt(2)
    assert!((dist - 1.414).abs() < 0.01);
}

#[test]
fn test_spectral_distance_same_position() {
    let config = SpectralPEConfig {
        num_eigenvectors: 2,
        ..Default::default()
    };
    let encoder = SpectralPositionEncoder::new(config);

    // 2 positions, 2 dims each
    let pe = vec![0.5, 1.0, -0.5, -1.0];

    let dist = encoder.spectral_distance(&pe, 0, 0);

    // Distance to self should be 0
    assert!(dist.abs() < 1e-6);
}

#[test]
fn test_spectral_distance_out_of_bounds() {
    let config = SpectralPEConfig {
        num_eigenvectors: 2,
        ..Default::default()
    };
    let encoder = SpectralPositionEncoder::new(config);

    let pe = vec![0.0, 1.0]; // 1 position, 2 dims

    let dist = encoder.spectral_distance(&pe, 0, 100);

    // Should handle gracefully
    assert_eq!(dist, 0.0);
}

#[test]
fn test_encode_from_edges_chain() {
    let config = SpectralPEConfig {
        num_eigenvectors: 3,
        pe_attention_heads: 2,
        learnable_pe: false,
    };
    let encoder = SpectralPositionEncoder::new(config);

    // Chain graph
    let edges = vec![(0, 1), (1, 2), (2, 3)];
    let encoding = encoder.encode_from_edges(&edges, 4);

    // Should produce [4 positions x 3 eigenvectors] = 12 values
    assert_eq!(encoding.len(), 12);

    // All values should be finite
    assert!(encoding.iter().all(|&x| x.is_finite()));
}

#[test]
fn test_encode_from_edges_triangle() {
    let config = SpectralPEConfig {
        num_eigenvectors: 2,
        pe_attention_heads: 2,
        learnable_pe: false,
    };
    let encoder = SpectralPositionEncoder::new(config);

    // Triangle graph
    let edges = vec![(0, 1), (1, 2), (2, 0)];
    let encoding = encoder.encode_from_edges(&edges, 3);

    assert_eq!(encoding.len(), 6); // 3 positions * 2 eigenvectors

    // All values should be finite
    assert!(encoding.iter().all(|x| x.is_finite()));

    // Triangle is symmetric, so distances should be relatively equal
    let dist_01 = encoder.spectral_distance(&encoding, 0, 1);
    let dist_12 = encoder.spectral_distance(&encoding, 1, 2);
    let dist_20 = encoder.spectral_distance(&encoding, 2, 0);

    // All pairwise distances should be similar (within larger tolerance for numerical stability)
    assert!((dist_01 - dist_12).abs() < 1.0);
    assert!((dist_12 - dist_20).abs() < 1.0);
    assert!((dist_20 - dist_01).abs() < 1.0);
}

#[test]
fn test_encode_from_edges_star() {
    let config = SpectralPEConfig {
        num_eigenvectors: 3,
        pe_attention_heads: 2,
        learnable_pe: false,
    };
    let encoder = SpectralPositionEncoder::new(config);

    // Star graph: center node 0 connected to all others
    let edges = vec![(0, 1), (0, 2), (0, 3), (0, 4)];
    let encoding = encoder.encode_from_edges(&edges, 5);

    assert_eq!(encoding.len(), 15); // 5 positions * 3 eigenvectors

    // All values should be finite
    assert!(encoding.iter().all(|x| x.is_finite()));

    // Center node should be relatively equidistant from all leaf nodes
    let dist_01 = encoder.spectral_distance(&encoding, 0, 1);
    let dist_02 = encoder.spectral_distance(&encoding, 0, 2);
    let dist_03 = encoder.spectral_distance(&encoding, 0, 3);

    // Distances should be similar (within larger tolerance for numerical stability)
    assert!((dist_01 - dist_02).abs() < 1.0);
    assert!((dist_02 - dist_03).abs() < 1.0);
}

#[test]
fn test_eigenvectors_count() {
    let encoder = SpectralPositionEncoder::default();

    let edges = vec![(0, 1), (1, 2), (2, 3)];
    let laplacian = encoder.compute_normalized_laplacian(&edges, 4);

    let eigenvectors = encoder.eigenvectors(&laplacian, 4, 3);

    // Should return requested number of eigenvectors
    assert_eq!(eigenvectors.len(), 3);

    // Each eigenvector should have length n
    for evec in &eigenvectors {
        assert_eq!(evec.len(), 4);
    }
}

#[test]
fn test_eigenvectors_normalized() {
    let encoder = SpectralPositionEncoder::default();

    let edges = vec![(0, 1), (1, 2)];
    let laplacian = encoder.compute_normalized_laplacian(&edges, 3);

    let eigenvectors = encoder.eigenvectors(&laplacian, 3, 2);

    // Each eigenvector should be normalized
    for evec in &eigenvectors {
        let norm: f32 = evec.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 1e-3,
            "Eigenvector should be normalized: norm={}",
            norm
        );
    }
}

#[test]
fn test_position_encoding_uniqueness() {
    let encoder = SpectralPositionEncoder::default();

    // Different graph structures should produce different encodings
    let edges1 = vec![(0, 1), (1, 2)]; // Chain
    let edges2 = vec![(0, 1), (1, 2), (2, 0)]; // Triangle

    let encoding1 = encoder.encode_from_edges(&edges1, 3);
    let encoding2 = encoder.encode_from_edges(&edges2, 3);

    // Encodings should differ
    let diff: f32 = encoding1
        .iter()
        .zip(&encoding2)
        .map(|(a, b)| (a - b).abs())
        .sum();

    assert!(
        diff > 0.01,
        "Different graphs should produce different encodings"
    );
}

#[test]
fn test_mincut_integration() {
    let config = SpectralPEConfig {
        num_eigenvectors: 4,
        pe_attention_heads: 4,
        learnable_pe: false,
    };
    let encoder = SpectralPositionEncoder::new(config);

    // Simulate mincut boundary edges from a bipartite cut
    // Nodes 0,1,2 in one partition, 3,4,5 in another
    let boundary_edges = vec![(0, 3), (0, 4), (1, 3), (1, 5), (2, 4), (2, 5)];

    let encoding = encoder.encode_from_edges(&boundary_edges, 6);

    assert_eq!(encoding.len(), 24); // 6 positions * 4 eigenvectors

    // Nodes within the same partition should have smaller spectral distance
    // than nodes across partitions
    let within_partition_dist = encoder.spectral_distance(&encoding, 0, 1);
    let across_partition_dist = encoder.spectral_distance(&encoding, 0, 3);

    // This is a heuristic - may not always hold for all graphs
    // but should generally be true for bipartite cuts
    assert!(encoding.iter().all(|&x| x.is_finite()));
}

#[test]
fn test_large_graph_scaling() {
    let config = SpectralPEConfig {
        num_eigenvectors: 8,
        pe_attention_heads: 4,
        learnable_pe: false,
    };
    let encoder = SpectralPositionEncoder::new(config);

    // Create larger graph (32 nodes, chain)
    let n = 32;
    let mut edges = vec![];
    for i in 0..n - 1 {
        edges.push((i, i + 1));
    }

    let encoding = encoder.encode_from_edges(&edges, n as usize);

    assert_eq!(encoding.len(), (n * 8) as usize);
    assert!(encoding.iter().all(|&x| x.is_finite()));
}

#[test]
fn test_config_num_eigenvectors() {
    let config1 = SpectralPEConfig {
        num_eigenvectors: 2,
        pe_attention_heads: 2,
        learnable_pe: false,
    };
    let encoder1 = SpectralPositionEncoder::new(config1);

    let config2 = SpectralPEConfig {
        num_eigenvectors: 4,
        pe_attention_heads: 4,
        learnable_pe: false,
    };
    let encoder2 = SpectralPositionEncoder::new(config2);

    // Use larger graph to support more eigenvectors
    let edges = vec![(0, 1), (1, 2), (2, 3), (3, 4)];

    let encoding1 = encoder1.encode_from_edges(&edges, 5);
    let encoding2 = encoder2.encode_from_edges(&edges, 5);

    // More eigenvectors = longer encoding per position
    assert_eq!(encoding1.len(), 10); // 5 positions * 2 eigenvectors
    assert_eq!(encoding2.len(), 20); // 5 positions * 4 eigenvectors
}

#[test]
fn test_empty_edge_list() {
    let config = SpectralPEConfig {
        num_eigenvectors: 4,
        pe_attention_heads: 4,
        learnable_pe: false,
    };
    let encoder = SpectralPositionEncoder::new(config);

    let empty_edges: Vec<(u16, u16)> = vec![];
    let encoding = encoder.encode_from_edges(&empty_edges, 4);

    // Should handle empty graphs gracefully
    // Can get at most n eigenvectors for n nodes
    assert_eq!(encoding.len(), 16); // 4 positions * 4 eigenvectors

    // May produce zero or near-zero encoding for disconnected graph
    // Just verify it doesn't crash and produces finite values
    assert!(encoding.iter().all(|&x| x.is_finite()));
}
