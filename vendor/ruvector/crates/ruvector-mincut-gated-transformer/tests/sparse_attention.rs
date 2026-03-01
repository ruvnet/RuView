//! Comprehensive tests for mincut-aware sparse attention.

#![cfg(feature = "sparse_attention")]

use ruvector_mincut_gated_transformer::{
    GatePacket, LambdaDensitySchedule, MincutSparseAttention, SparseMask, SparsityConfig,
};

#[test]
fn test_sparse_mask_creation() {
    let empty = SparseMask::empty();
    assert_eq!(empty.num_positions(), 0);
    assert_eq!(empty.density, 0.0);
    assert_eq!(empty.sparsity(), 1.0);

    let full = SparseMask::full(8);
    assert_eq!(full.num_positions(), 36); // 8*9/2 = 36 causal positions
    assert_eq!(full.density, 1.0);
    assert_eq!(full.sparsity(), 0.0);
}

#[test]
fn test_sparse_mask_can_attend() {
    let mut mask = SparseMask::empty();
    mask.positions.push((2, 0));
    mask.positions.push((2, 1));
    mask.positions.push((2, 2));

    assert!(mask.can_attend(2, 0));
    assert!(mask.can_attend(2, 1));
    assert!(mask.can_attend(2, 2));
    assert!(!mask.can_attend(2, 3));
    assert!(!mask.can_attend(1, 0));
}

#[test]
fn test_density_calculation_adaptive() {
    let config = SparsityConfig {
        lambda_based_density: Some(LambdaDensitySchedule::Adaptive),
        ..Default::default()
    };
    let sparse_attn = MincutSparseAttention::new(config);

    // High lambda, low boundaries = dense
    let gate_stable = GatePacket {
        lambda: 200,
        boundary_edges: 5,
        boundary_concentration_q15: 4096,
        partition_count: 2,
        ..Default::default()
    };
    let density_stable = sparse_attn.calculate_density(&gate_stable);

    // Low lambda, high boundaries = sparse
    let gate_unstable = GatePacket {
        lambda: 50,
        boundary_edges: 40,
        boundary_concentration_q15: 24576,
        partition_count: 8,
        ..Default::default()
    };
    let density_unstable = sparse_attn.calculate_density(&gate_unstable);

    assert!(density_stable > density_unstable);
}

#[test]
fn test_mask_building_with_partitions() {
    let config = SparsityConfig::default();
    let sparse_attn = MincutSparseAttention::new(config);

    let gate = GatePacket {
        lambda: 100,
        partition_count: 3,
        boundary_edges: 5,
        ..Default::default()
    };

    let mask = sparse_attn.build_mask(&gate, 32);

    // Should have some positions
    assert!(mask.num_positions() > 0);

    // Should be sparse (density < 1.0)
    assert!(mask.density < 1.0);

    // Should have 3 partitions
    assert_eq!(mask.partition_boundaries.len(), 3);

    // Positions should be causal
    for &(q, k) in &mask.positions {
        assert!(k <= q, "Non-causal position: ({}, {})", q, k);
    }
}

#[test]
fn test_flops_estimation() {
    let config = SparsityConfig::default();
    let sparse_attn = MincutSparseAttention::new(config);

    let gate = GatePacket {
        lambda: 100,
        partition_count: 4,
        boundary_edges: 10,
        ..Default::default()
    };

    let mask = sparse_attn.build_mask(&gate, 64);
    let ratio = sparse_attn.estimated_flops_ratio(&mask, 64);

    // Should have speedup
    assert!(ratio < 1.0);
    assert!(ratio > 0.0);
}
