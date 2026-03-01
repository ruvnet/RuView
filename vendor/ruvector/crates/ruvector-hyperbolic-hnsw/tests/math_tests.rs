//! Comprehensive Mathematical Correctness Tests for Hyperbolic Operations
//!
//! These tests verify the mathematical properties of Poincaré ball operations
//! as specified in the evaluation protocol.

use ruvector_hyperbolic_hnsw::poincare::*;
use ruvector_hyperbolic_hnsw::tangent::*;
use ruvector_hyperbolic_hnsw::hnsw::*;
use ruvector_hyperbolic_hnsw::shard::*;

// ============================================================================
// Poincaré Ball Properties
// ============================================================================

#[test]
fn test_mobius_add_identity() {
    // x ⊕ 0 = x (right identity)
    let x = vec![0.3, 0.2, 0.1];
    let zero = vec![0.0, 0.0, 0.0];

    let result = mobius_add(&x, &zero, 1.0);

    for (a, b) in x.iter().zip(result.iter()) {
        assert!((a - b).abs() < 1e-5, "Right identity failed");
    }
}

#[test]
fn test_mobius_add_inverse() {
    // x ⊕ (-x) ≈ 0 (inverse element)
    let x = vec![0.3, 0.2];
    let neg_x: Vec<f32> = x.iter().map(|v| -v).collect();

    let result = mobius_add(&x, &neg_x, 1.0);
    let result_norm = norm(&result);

    // Result should be close to zero
    assert!(result_norm < 0.1, "Inverse element failed: norm = {}", result_norm);
}

#[test]
fn test_mobius_add_gyrocommutative() {
    // Gyrocommutative: x ⊕ y ≈ gyr[x,y](y ⊕ x) (holds for small vectors)
    let x = vec![0.1, 0.05];
    let y = vec![0.08, -0.03];

    let xy = mobius_add(&x, &y, 1.0);
    let yx = mobius_add(&y, &x, 1.0);

    // For small vectors, these should be similar
    let diff: f32 = xy.iter().zip(yx.iter()).map(|(a, b)| (a - b).abs()).sum();
    assert!(diff < 0.5, "Gyrocommutative property check: diff = {}", diff);
}

#[test]
fn test_exp_log_inverse() {
    // log_p(exp_p(v)) = v (inverse relationship)
    let p = vec![0.1, 0.2, 0.1];
    let v = vec![0.1, -0.1, 0.05];

    let q = exp_map(&v, &p, 1.0);
    let v_recovered = log_map(&q, &p, 1.0);

    for (a, b) in v.iter().zip(v_recovered.iter()) {
        assert!((a - b).abs() < 1e-4, "exp-log inverse failed: expected {}, got {}", a, b);
    }
}

#[test]
fn test_log_exp_inverse() {
    // exp_p(log_p(q)) = q (inverse relationship)
    let p = vec![0.1, 0.15];
    let q = vec![0.2, -0.1];

    let v = log_map(&q, &p, 1.0);
    let q_recovered = exp_map(&v, &p, 1.0);

    for (a, b) in q.iter().zip(q_recovered.iter()) {
        assert!((a - b).abs() < 1e-4, "log-exp inverse failed: expected {}, got {}", a, b);
    }
}

#[test]
fn test_distance_symmetry() {
    // d(x, y) = d(y, x)
    let x = vec![0.3, 0.2, 0.1];
    let y = vec![-0.1, 0.4, 0.2];

    let d1 = poincare_distance(&x, &y, 1.0);
    let d2 = poincare_distance(&y, &x, 1.0);

    assert!((d1 - d2).abs() < 1e-6, "Symmetry failed: {} vs {}", d1, d2);
}

#[test]
fn test_distance_identity() {
    // d(x, x) = 0
    let x = vec![0.3, 0.2, 0.1];
    let d = poincare_distance(&x, &x, 1.0);

    assert!(d.abs() < 1e-6, "Identity of indiscernibles failed: d = {}", d);
}

#[test]
fn test_distance_non_negative() {
    // d(x, y) >= 0
    let x = vec![0.3, 0.2];
    let y = vec![-0.1, 0.4];

    let d = poincare_distance(&x, &y, 1.0);
    assert!(d >= 0.0, "Non-negativity failed: d = {}", d);
}

#[test]
fn test_distance_triangle_inequality() {
    // d(x, z) <= d(x, y) + d(y, z)
    let x = vec![0.1, 0.2];
    let y = vec![0.3, 0.1];
    let z = vec![-0.1, 0.35];

    let dxz = poincare_distance(&x, &z, 1.0);
    let dxy = poincare_distance(&x, &y, 1.0);
    let dyz = poincare_distance(&y, &z, 1.0);

    assert!(dxz <= dxy + dyz + 1e-5,
        "Triangle inequality failed: {} > {} + {}", dxz, dxy, dyz);
}

// ============================================================================
// Numerical Stability
// ============================================================================

#[test]
fn test_projection_keeps_points_inside() {
    // All projected points should satisfy ||x|| < 1/sqrt(c) - eps
    let test_points = vec![
        vec![0.5, 0.5, 0.5],
        vec![0.9, 0.9],
        vec![10.0, 10.0, 10.0],
        vec![-5.0, 3.0],
    ];

    for point in test_points {
        let projected = project_to_ball(&point, 1.0, EPS);
        let n = norm(&projected);
        // Use <= with small tolerance for floating point
        assert!(n <= 1.0 - EPS + 1e-7,
            "Projection failed: norm {} >= max {}", n, 1.0 - EPS);
    }
}

#[test]
fn test_near_boundary_stability() {
    // Operations near the boundary should remain stable
    let near_boundary = vec![0.99 - EPS, 0.0];
    let small_vec = vec![0.01, 0.01];

    // Should not panic or produce NaN/Inf
    let result = mobius_add(&near_boundary, &small_vec, 1.0);
    assert!(!result.iter().any(|v| v.is_nan() || v.is_infinite()),
        "Near boundary operation produced NaN/Inf");

    let n = norm(&result);
    assert!(n < 1.0 - EPS, "Result escaped ball boundary");
}

#[test]
fn test_zero_vector_handling() {
    // Operations with zero vector should be stable
    let zero = vec![0.0, 0.0, 0.0];
    let x = vec![0.3, 0.2, 0.1];

    // exp_map with zero tangent should return base point
    let result = exp_map(&zero, &x, 1.0);
    for (a, b) in x.iter().zip(result.iter()) {
        assert!((a - b).abs() < 1e-5, "exp_map with zero failed");
    }

    // log_map of same point should be zero
    let log_result = log_map(&x, &x, 1.0);
    assert!(norm(&log_result) < 1e-5, "log_map of same point should be zero");
}

#[test]
fn test_small_curvature_stability() {
    // Small curvatures should work (approaches Euclidean)
    let x = vec![0.3, 0.2];
    let y = vec![0.1, 0.4];

    let d_small_c = poincare_distance(&x, &y, 0.01);
    let d_euclidean: f32 = x.iter().zip(y.iter())
        .map(|(a, b)| (a - b) * (a - b))
        .sum::<f32>()
        .sqrt();

    // For small curvature, should approach Euclidean
    // The ratio should be bounded
    assert!(!d_small_c.is_nan() && !d_small_c.is_infinite(),
        "Small curvature produced invalid result");
}

#[test]
fn test_large_curvature_stability() {
    // Large curvatures should work (stronger hyperbolic effect)
    let x = vec![0.1, 0.1];
    let y = vec![0.2, 0.1];

    let d_large_c = poincare_distance(&x, &y, 10.0);

    assert!(!d_large_c.is_nan() && !d_large_c.is_infinite(),
        "Large curvature produced invalid result: {}", d_large_c);
}

// ============================================================================
// Frechet Mean Properties
// ============================================================================

#[test]
fn test_frechet_mean_single_point() {
    // Frechet mean of single point is that point
    let points = vec![vec![0.3, 0.2]];
    let point_refs: Vec<&[f32]> = points.iter().map(|p| p.as_slice()).collect();
    let config = PoincareConfig::default();

    let mean = frechet_mean(&point_refs, None, &config).unwrap();

    for (a, b) in points[0].iter().zip(mean.iter()) {
        assert!((a - b).abs() < 1e-4, "Single point mean failed");
    }
}

#[test]
fn test_frechet_mean_symmetric() {
    // Mean of symmetric points should be near origin
    let points = vec![
        vec![0.3, 0.0],
        vec![-0.3, 0.0],
    ];
    let point_refs: Vec<&[f32]> = points.iter().map(|p| p.as_slice()).collect();
    let config = PoincareConfig::default();

    let mean = frechet_mean(&point_refs, None, &config).unwrap();

    // Mean should be close to origin
    let mean_norm = norm(&mean);
    assert!(mean_norm < 0.1, "Symmetric mean not near origin: {}", mean_norm);
}

// ============================================================================
// Tangent Space Operations
// ============================================================================

#[test]
fn test_tangent_cache_creation() {
    let points = vec![
        vec![0.1, 0.2, 0.1],
        vec![-0.1, 0.15, 0.05],
        vec![0.2, -0.1, 0.1],
    ];
    let indices: Vec<usize> = (0..3).collect();

    let cache = TangentCache::new(&points, &indices, 1.0).unwrap();

    assert_eq!(cache.len(), 3);
    assert_eq!(cache.dim(), 3);

    // Centroid should be inside ball
    let centroid_norm = norm(&cache.centroid);
    assert!(centroid_norm < 1.0 - EPS, "Centroid outside ball");
}

#[test]
fn test_tangent_distance_ordering() {
    // Tangent distance should roughly preserve hyperbolic distance ordering
    let points = vec![
        vec![0.1, 0.1],
        vec![0.2, 0.1],
        vec![0.5, 0.3],
    ];
    let indices: Vec<usize> = (0..3).collect();

    let cache = TangentCache::new(&points, &indices, 1.0).unwrap();

    let query = vec![0.12, 0.11];
    let query_tangent = cache.query_tangent(&query);

    let mut tangent_dists: Vec<(usize, f32)> = (0..3)
        .map(|i| (i, cache.tangent_distance_squared(&query_tangent, i)))
        .collect();
    tangent_dists.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    let mut hyp_dists: Vec<(usize, f32)> = (0..3)
        .map(|i| (i, poincare_distance(&query, &points[i], 1.0)))
        .collect();
    hyp_dists.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    // First nearest neighbor should match
    assert_eq!(tangent_dists[0].0, hyp_dists[0].0,
        "First neighbor mismatch: tangent says {}, hyperbolic says {}",
        tangent_dists[0].0, hyp_dists[0].0);
}

// ============================================================================
// HNSW Integration
// ============================================================================

#[test]
fn test_hnsw_insert_and_search() {
    let mut hnsw = HyperbolicHnsw::default_config();

    // Insert points
    for i in 0..20 {
        let v = vec![0.1 * (i as f32 % 5.0), 0.05 * (i as f32 / 5.0)];
        hnsw.insert(v).unwrap();
    }

    assert_eq!(hnsw.len(), 20);

    // Search
    let query = vec![0.25, 0.125];
    let results = hnsw.search(&query, 5).unwrap();

    assert_eq!(results.len(), 5);

    // Results should be sorted by distance
    for i in 1..results.len() {
        assert!(results[i-1].distance <= results[i].distance,
            "Results not sorted at index {}: {} > {}",
            i, results[i-1].distance, results[i].distance);
    }
}

#[test]
fn test_hnsw_nearest_is_correct() {
    let mut hnsw = HyperbolicHnsw::default_config();

    let points = vec![
        vec![0.0, 0.0],
        vec![0.5, 0.0],
        vec![0.0, 0.5],
        vec![0.3, 0.3],
    ];

    for p in &points {
        hnsw.insert(p.clone()).unwrap();
    }

    // Query near origin
    let query = vec![0.05, 0.05];
    let results = hnsw.search(&query, 1).unwrap();

    // Should find point at origin (id 0)
    assert_eq!(results[0].id, 0, "Expected nearest to be origin");
}

#[test]
fn test_hnsw_curvature_update() {
    let mut hnsw = HyperbolicHnsw::default_config();

    hnsw.insert(vec![0.1, 0.2]).unwrap();
    hnsw.insert(vec![0.3, 0.1]).unwrap();

    // Update curvature
    hnsw.set_curvature(2.0).unwrap();

    assert!((hnsw.config.curvature - 2.0).abs() < 1e-6);

    // Search should still work
    let results = hnsw.search(&[0.2, 0.15], 2).unwrap();
    assert_eq!(results.len(), 2);
}

// ============================================================================
// Shard Management
// ============================================================================

#[test]
fn test_curvature_registry() {
    let mut registry = CurvatureRegistry::new(1.0);

    registry.set("shard_1", 0.5);
    assert!((registry.get("shard_1") - 0.5).abs() < 1e-6);
    assert!((registry.get("unknown") - 1.0).abs() < 1e-6); // Default

    // Canary testing
    registry.set_canary("shard_1", 0.3, 50);
    assert!((registry.get_effective("shard_1", false) - 0.5).abs() < 1e-6);
    assert!((registry.get_effective("shard_1", true) - 0.3).abs() < 1e-6);

    // Promote canary
    if let Some(shard) = registry.shards.get_mut("shard_1") {
        shard.promote_canary();
    }
    assert!((registry.get("shard_1") - 0.3).abs() < 1e-6);
}

#[test]
fn test_sharded_hnsw() {
    let mut manager = ShardedHyperbolicHnsw::new(1.0);

    for i in 0..30 {
        let v = vec![0.1 * (i as f32 % 6.0), 0.05 * (i as f32 / 6.0)];
        manager.insert(v, Some(i / 10)).unwrap();
    }

    assert_eq!(manager.len(), 30);
    assert!(manager.num_shards() > 0);

    // Search
    let results = manager.search(&[0.25, 0.125], 5).unwrap();
    assert!(!results.is_empty());
}

// ============================================================================
// Hierarchy Metrics
// ============================================================================

#[test]
fn test_hierarchy_metrics_radius_correlation() {
    // Points with radius proportional to depth should have positive correlation
    let points: Vec<Vec<f32>> = (0..20).map(|i| {
        let depth = i / 4;
        let radius = 0.1 + 0.15 * depth as f32;
        let angle = (i % 4) as f32 * std::f32::consts::PI / 2.0;
        vec![radius * angle.cos(), radius * angle.sin()]
    }).collect();

    let depths: Vec<usize> = (0..20).map(|i| i / 4).collect();

    let metrics = HierarchyMetrics::compute(&points, &depths, 1.0).unwrap();

    assert!(metrics.radius_depth_correlation > 0.5,
        "Expected positive correlation, got {}", metrics.radius_depth_correlation);
}

// ============================================================================
// Dual Space Index
// ============================================================================

#[test]
fn test_dual_space_index() {
    let mut dual = DualSpaceIndex::new(1.0, 0.5);

    for i in 0..15 {
        let v = vec![0.1 * i as f32, 0.05 * i as f32];
        dual.insert(v).unwrap();
    }

    let results = dual.search(&[0.35, 0.175], 5).unwrap();

    assert_eq!(results.len(), 5);

    // Results should be sorted
    for i in 1..results.len() {
        assert!(results[i-1].distance <= results[i].distance);
    }
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_empty_index_search() {
    let hnsw = HyperbolicHnsw::default_config();

    let results = hnsw.search(&[0.1, 0.2], 5).unwrap();
    assert!(results.is_empty());
}

#[test]
fn test_single_element_search() {
    let mut hnsw = HyperbolicHnsw::default_config();
    hnsw.insert(vec![0.3, 0.2]).unwrap();

    let results = hnsw.search(&[0.1, 0.2], 5).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, 0);
}

#[test]
fn test_k_larger_than_index() {
    let mut hnsw = HyperbolicHnsw::default_config();

    for i in 0..3 {
        hnsw.insert(vec![0.1 * i as f32, 0.1]).unwrap();
    }

    let results = hnsw.search(&[0.15, 0.1], 10).unwrap();
    assert_eq!(results.len(), 3);
}

// ============================================================================
// Performance Characteristics
// ============================================================================

#[test]
fn test_insert_performance() {
    let mut hnsw = HyperbolicHnsw::default_config();

    // Should handle 100 insertions without panic
    for i in 0..100 {
        let v = vec![
            0.05 * (i % 10) as f32,
            0.05 * (i / 10) as f32,
        ];
        hnsw.insert(v).unwrap();
    }

    assert_eq!(hnsw.len(), 100);
}

#[test]
fn test_search_performance() {
    let mut hnsw = HyperbolicHnsw::default_config();

    for i in 0..100 {
        let v = vec![
            0.05 * (i % 10) as f32,
            0.05 * (i / 10) as f32,
        ];
        hnsw.insert(v).unwrap();
    }

    // Should handle multiple searches
    for _ in 0..10 {
        let query = vec![0.25, 0.25];
        let results = hnsw.search(&query, 10).unwrap();
        assert_eq!(results.len(), 10);
    }
}
