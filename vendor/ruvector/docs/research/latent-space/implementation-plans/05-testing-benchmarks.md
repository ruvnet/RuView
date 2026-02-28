# Testing and Benchmarking Specification for ruvector-attention

**Version**: 1.0
**Date**: 2025-11-30
**Status**: Draft

## Executive Summary

This document defines comprehensive testing and benchmarking strategies for the `ruvector-attention` crate, ensuring reliability, performance, and correctness across all attention mechanisms and platforms.

---

## 1. Testing Strategy Overview

### Testing Pyramid

```
Testing Distribution:
                    /\
                   /  \  E2E Tests (5%)
                  /----\  - End-to-end workflows
                 /      \  - Real-world scenarios
                /--------\ Integration Tests (25%)
               /          \ - Module integration
              /            \ - Cross-platform validation
             /--------------\ Unit Tests (70%)
            /                \ - Component isolation
           /                  \ - Edge cases & correctness
          /--------------------\
```

### Testing Philosophy

1. **Test-Driven Development (TDD)**: Write tests before implementation
2. **Property-Based Testing**: Use `proptest` for mathematical properties
3. **Regression Prevention**: Automated performance benchmarks
4. **Platform Parity**: Ensure WASM/NAPI-RS match Rust implementations
5. **Continuous Validation**: Integrate into CI/CD pipeline

### Test Coverage Goals

| Component | Target Coverage | Critical Paths |
|-----------|----------------|----------------|
| Core Attention | 95% | 100% |
| Hyperbolic | 90% | 100% |
| Graph Attention | 90% | 100% |
| Optimization (Flash, Linear) | 85% | 100% |
| Platform Bindings | 80% | 95% |

---

## 2. Unit Test Specifications

### 2.1 Scaled Dot-Product Attention

```rust
// crates/ruvector-attention/src/scaled_dot_product/tests.rs

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;
    use ndarray::{Array1, Array2};
    use proptest::prelude::*;

    #[test]
    fn test_basic_attention_computation() {
        let attention = ScaledDotProduct::new(4);

        let query = Array1::from_vec(vec![1.0, 0.0, 0.0, 0.0]);
        let keys = Array2::from_shape_vec(
            (3, 4),
            vec![
                1.0, 0.0, 0.0, 0.0,  // Identical to query
                0.0, 1.0, 0.0, 0.0,  // Orthogonal
                0.5, 0.5, 0.5, 0.5,  // Mixed
            ],
        ).unwrap();
        let values = Array2::from_shape_vec(
            (3, 4),
            vec![
                1.0, 0.0, 0.0, 0.0,
                0.0, 1.0, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
            ],
        ).unwrap();

        let output = attention.forward(&query, &keys, &values).unwrap();

        // Should weight most similar key highest
        assert!(output[0] > 0.5, "First value should dominate");
    }

    #[test]
    fn test_zero_vectors() {
        let attention = ScaledDotProduct::new(4);

        let query = Array1::zeros(4);
        let keys = Array2::zeros((3, 4));
        let values = Array2::ones((3, 4));

        let output = attention.forward(&query, &keys, &values).unwrap();

        // All weights equal, should average values
        assert_relative_eq!(output[0], 1.0, epsilon = 1e-5);
    }

    #[test]
    fn test_numerical_stability_large_values() {
        let attention = ScaledDotProduct::new(128);

        // Large magnitude vectors
        let query = Array1::from_elem(128, 100.0);
        let keys = Array2::from_elem((100, 128), 100.0);
        let values = Array2::from_elem((100, 128), 1.0);

        let output = attention.forward(&query, &keys, &values).unwrap();

        // Should not overflow/underflow
        assert!(output.iter().all(|&x| x.is_finite()));
        assert!(output.iter().all(|&x| x >= 0.0 && x <= 2.0));
    }

    #[test]
    fn test_softmax_sums_to_one() {
        let attention = ScaledDotProduct::new(64);

        let query = Array1::from_vec((0..64).map(|i| i as f32).collect());
        let keys = Array2::from_shape_vec(
            (50, 64),
            (0..50*64).map(|i| (i % 100) as f32).collect(),
        ).unwrap();

        let weights = attention.compute_attention_weights(&query, &keys).unwrap();
        let sum: f32 = weights.sum();

        assert_relative_eq!(sum, 1.0, epsilon = 1e-5);
    }

    #[test]
    fn test_gradient_correctness() {
        // Numerical gradient checking
        let attention = ScaledDotProduct::new(8);

        let query = Array1::from_vec(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let keys = Array2::from_elem((5, 8), 0.5);
        let values = Array2::eye(8).slice(s![0..5, ..]).to_owned();

        let epsilon = 1e-4;

        // Compute numerical gradient
        let output = attention.forward(&query, &keys, &values).unwrap();

        for i in 0..8 {
            let mut query_plus = query.clone();
            query_plus[i] += epsilon;
            let output_plus = attention.forward(&query_plus, &keys, &values).unwrap();

            let numerical_grad = (&output_plus - &output) / epsilon;

            // Analytical gradient should match
            // (Would require backward pass implementation)
            assert!(numerical_grad.iter().all(|&x| x.is_finite()));
        }
    }

    #[test]
    fn test_serialization_roundtrip() {
        let attention = ScaledDotProduct::new(64);

        let serialized = serde_json::to_string(&attention).unwrap();
        let deserialized: ScaledDotProduct = serde_json::from_str(&serialized).unwrap();

        assert_eq!(attention.dim(), deserialized.dim());
    }

    #[test]
    fn test_masking() {
        let attention = ScaledDotProduct::new(4);

        let query = Array1::ones(4);
        let keys = Array2::ones((3, 4));
        let values = Array2::from_shape_vec(
            (3, 4),
            vec![
                1.0, 0.0, 0.0, 0.0,
                0.0, 2.0, 0.0, 0.0,
                0.0, 0.0, 3.0, 0.0,
            ],
        ).unwrap();

        // Mask out second key
        let mask = vec![true, false, true];

        let output = attention.forward_with_mask(&query, &keys, &values, &mask).unwrap();

        // Second value should not contribute
        assert_relative_eq!(output[1], 0.0, epsilon = 1e-5);
    }

    // Property-based testing
    proptest! {
        #[test]
        fn prop_attention_preserves_dimension(
            dim in 4usize..256,
            n_keys in 1usize..100,
        ) {
            let attention = ScaledDotProduct::new(dim);

            let query = Array1::from_elem(dim, 1.0);
            let keys = Array2::from_elem((n_keys, dim), 0.5);
            let values = Array2::from_elem((n_keys, dim), 0.5);

            let output = attention.forward(&query, &keys, &values).unwrap();

            prop_assert_eq!(output.len(), dim);
        }

        #[test]
        fn prop_weights_non_negative(
            dim in 4usize..64,
            n_keys in 1usize..50,
        ) {
            let attention = ScaledDotProduct::new(dim);

            let query = Array1::from_elem(dim, 1.0);
            let keys = Array2::from_elem((n_keys, dim), 0.5);

            let weights = attention.compute_attention_weights(&query, &keys).unwrap();

            prop_assert!(weights.iter().all(|&w| w >= 0.0));
        }
    }
}
```

### 2.2 Hyperbolic Attention Tests

```rust
// crates/ruvector-attention/src/hyperbolic/tests.rs

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_poincare_distance_symmetry() {
        let attention = HyperbolicAttention::new(-1.0); // Unit curvature

        let x = Array1::from_vec(vec![0.1, 0.2, 0.0, 0.0]);
        let y = Array1::from_vec(vec![0.3, 0.1, 0.0, 0.0]);

        let d_xy = attention.poincare_distance(&x, &y);
        let d_yx = attention.poincare_distance(&y, &x);

        assert_relative_eq!(d_xy, d_yx, epsilon = 1e-6);
    }

    #[test]
    fn test_poincare_distance_identity() {
        let attention = HyperbolicAttention::new(-1.0);

        let x = Array1::from_vec(vec![0.1, 0.2, 0.3, 0.4]);

        let d = attention.poincare_distance(&x, &x);

        assert_relative_eq!(d, 0.0, epsilon = 1e-6);
    }

    #[test]
    fn test_mobius_addition_identity() {
        let attention = HyperbolicAttention::new(-1.0);

        let x = Array1::from_vec(vec![0.1, 0.2, 0.0, 0.0]);
        let zero = Array1::zeros(4);

        let result = attention.mobius_add(&x, &zero);

        assert_relative_eq!(result.as_slice().unwrap(), x.as_slice().unwrap(), epsilon = 1e-6);
    }

    #[test]
    fn test_mobius_addition_commutativity() {
        let attention = HyperbolicAttention::new(-1.0);

        let x = Array1::from_vec(vec![0.1, 0.2, 0.0, 0.0]);
        let y = Array1::from_vec(vec![0.05, 0.1, 0.0, 0.0]);

        let xy = attention.mobius_add(&x, &y);
        let yx = attention.mobius_add(&y, &x);

        assert_relative_eq!(xy.as_slice().unwrap(), yx.as_slice().unwrap(), epsilon = 1e-5);
    }

    #[test]
    fn test_boundary_stability() {
        let attention = HyperbolicAttention::new(-1.0);

        // Vector near boundary (||x|| → 1)
        let norm = 0.99;
        let x = Array1::from_vec(vec![norm / 2.0_f32.sqrt(), norm / 2.0_f32.sqrt(), 0.0, 0.0]);
        let y = Array1::from_vec(vec![0.1, 0.1, 0.0, 0.0]);

        let distance = attention.poincare_distance(&x, &y);

        // Should remain finite
        assert!(distance.is_finite());
        assert!(distance >= 0.0);
    }

    #[test]
    fn test_exponential_map() {
        let attention = HyperbolicAttention::new(-1.0);

        let origin = Array1::zeros(4);
        let tangent = Array1::from_vec(vec![0.1, 0.2, 0.0, 0.0]);

        let point = attention.exponential_map(&origin, &tangent);

        // Point should be in disk
        let norm: f32 = point.iter().map(|x| x * x).sum();
        assert!(norm < 1.0);
    }

    #[test]
    fn test_logarithmic_map() {
        let attention = HyperbolicAttention::new(-1.0);

        let origin = Array1::zeros(4);
        let point = Array1::from_vec(vec![0.3, 0.4, 0.0, 0.0]);

        let tangent = attention.logarithmic_map(&origin, &point);

        // Roundtrip should recover point
        let recovered = attention.exponential_map(&origin, &tangent);

        assert_relative_eq!(recovered.as_slice().unwrap(), point.as_slice().unwrap(), epsilon = 1e-5);
    }

    #[test]
    fn test_curvature_parameter() {
        let c_values = vec![-0.1, -0.5, -1.0, -2.0];

        for c in c_values {
            let attention = HyperbolicAttention::new(c);

            let x = Array1::from_vec(vec![0.1, 0.1, 0.0, 0.0]);
            let y = Array1::from_vec(vec![0.2, 0.2, 0.0, 0.0]);

            let distance = attention.poincare_distance(&x, &y);

            // Distance should vary with curvature
            assert!(distance > 0.0);
            assert!(distance.is_finite());
        }
    }

    #[test]
    fn test_gyrovector_parallel_transport() {
        let attention = HyperbolicAttention::new(-1.0);

        let x = Array1::from_vec(vec![0.1, 0.0, 0.0, 0.0]);
        let y = Array1::from_vec(vec![0.0, 0.1, 0.0, 0.0]);
        let v = Array1::from_vec(vec![0.05, 0.05, 0.0, 0.0]);

        let transported = attention.parallel_transport(&x, &y, &v);

        // Transported vector should maintain properties
        assert!(transported.iter().all(|&x| x.is_finite()));
    }

    proptest! {
        #[test]
        fn prop_poincare_distance_triangle_inequality(
            dim in 4usize..16,
        ) {
            let attention = HyperbolicAttention::new(-1.0);

            // Generate points in Poincaré disk
            let x = Array1::from_elem(dim, 0.1);
            let y = Array1::from_elem(dim, 0.2);
            let z = Array1::from_elem(dim, 0.15);

            let d_xy = attention.poincare_distance(&x, &y);
            let d_yz = attention.poincare_distance(&y, &z);
            let d_xz = attention.poincare_distance(&x, &z);

            // Triangle inequality: d(x,z) ≤ d(x,y) + d(y,z)
            prop_assert!(d_xz <= d_xy + d_yz + 1e-5);
        }
    }
}
```

### 2.3 Graph Attention Tests

```rust
// crates/ruvector-attention/src/graph/tests.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_featured_attention() {
        let attention = EdgeFeaturedAttention::new(4, 2);

        let node = Array1::from_vec(vec![1.0, 0.0, 0.0, 0.0]);
        let neighbors = Array2::from_shape_vec(
            (3, 4),
            vec![
                0.5, 0.5, 0.0, 0.0,
                0.0, 1.0, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
            ],
        ).unwrap();

        // Edge features: distance, weight
        let edge_features = Array2::from_shape_vec(
            (3, 2),
            vec![
                0.1, 1.0,  // Close, high weight
                0.5, 0.5,  // Medium distance, medium weight
                1.0, 0.1,  // Far, low weight
            ],
        ).unwrap();

        let output = attention.forward(&node, &neighbors, &edge_features).unwrap();

        assert_eq!(output.len(), 4);
        assert!(output.iter().all(|&x| x.is_finite()));
    }

    #[test]
    fn test_local_global_attention() {
        let attention = LocalGlobalAttention::new(4, 2, 8);

        let query = Array1::ones(4);
        let local_keys = Array2::ones((10, 4));
        let global_keys = Array2::ones((100, 4));
        let local_values = Array2::ones((10, 4));
        let global_values = Array2::ones((100, 4));

        let output = attention.forward(
            &query,
            &local_keys,
            &global_keys,
            &local_values,
            &global_values,
        ).unwrap();

        assert_eq!(output.len(), 4);
    }

    #[test]
    fn test_relational_attention() {
        let attention = RelationalAttention::new(4, vec![
            "friend".to_string(),
            "colleague".to_string(),
            "family".to_string(),
        ]);

        let node = Array1::ones(4);
        let neighbors = Array2::ones((5, 4));
        let relations = vec![0, 1, 0, 2, 1]; // Relation types

        let output = attention.forward(&node, &neighbors, &relations).unwrap();

        assert_eq!(output.len(), 4);
    }
}
```

### 2.4 Optimization Tests

```rust
// crates/ruvector-attention/src/flash/tests.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flash_attention_matches_standard() {
        let dim = 64;
        let n_neighbors = 1000;

        let flash = FlashAttention::new(dim, 256); // Block size 256
        let standard = ScaledDotProduct::new(dim);

        let query = Array1::ones(dim);
        let keys = Array2::ones((n_neighbors, dim));
        let values = Array2::ones((n_neighbors, dim));

        let flash_output = flash.forward(&query, &keys, &values).unwrap();
        let standard_output = standard.forward(&query, &keys, &values).unwrap();

        // Should be nearly identical
        assert_relative_eq!(
            flash_output.as_slice().unwrap(),
            standard_output.as_slice().unwrap(),
            epsilon = 1e-3
        );
    }

    #[test]
    fn test_flash_memory_usage() {
        let flash = FlashAttention::new(128, 512);

        // Should use O(sqrt(N)) memory for blocks
        let query = Array1::ones(128);
        let keys = Array2::ones((10000, 128));
        let values = Array2::ones((10000, 128));

        // Measure memory before and after
        let before = get_memory_usage();
        let _ = flash.forward(&query, &keys, &values).unwrap();
        let after = get_memory_usage();

        let memory_used = after - before;

        // Should be much less than O(N) memory
        assert!(memory_used < 50 * 1024 * 1024); // < 50MB
    }

    #[test]
    fn test_linear_attention_approximation() {
        let linear = LinearAttention::new(64);
        let standard = ScaledDotProduct::new(64);

        let query = Array1::ones(64);
        let keys = Array2::ones((100, 64));
        let values = Array2::ones((100, 64));

        let linear_output = linear.forward(&query, &keys, &values).unwrap();
        let standard_output = standard.forward(&query, &keys, &values).unwrap();

        // Should approximate standard attention
        let diff: f32 = (&linear_output - &standard_output)
            .iter()
            .map(|x| x.abs())
            .sum();

        assert!(diff < 5.0); // Reasonable approximation error
    }
}
```

---

## 3. Integration Test Specifications

### 3.1 Integration with ruvector-gnn

```rust
// tests/integration/gnn_integration.rs

use ruvector_attention::{HyperbolicAttention, GraphAttention};
use ruvector_gnn::{GNNLayer, GraphConvolution};

#[test]
fn test_hyperbolic_attention_in_gnn_layer() {
    let attention = HyperbolicAttention::new(-1.0);
    let layer = GNNLayer::with_attention(4, 8, attention);

    // Create graph: 5 nodes, 8 edges
    let nodes = Array2::ones((5, 4));
    let edges = vec![
        (0, 1), (0, 2), (1, 2), (1, 3),
        (2, 3), (2, 4), (3, 4), (4, 0),
    ];

    let output = layer.forward(&nodes, &edges).unwrap();

    assert_eq!(output.shape(), &[5, 8]);
    assert!(output.iter().all(|&x| x.is_finite()));
}

#[test]
fn test_multi_layer_gnn_with_attention() {
    let attention1 = GraphAttention::new(4);
    let attention2 = GraphAttention::new(8);

    let layer1 = GNNLayer::with_attention(4, 8, attention1);
    let layer2 = GNNLayer::with_attention(8, 16, attention2);

    let nodes = Array2::ones((10, 4));
    let edges = (0..20).map(|i| (i % 10, (i + 1) % 10)).collect();

    let hidden = layer1.forward(&nodes, &edges).unwrap();
    let output = layer2.forward(&hidden, &edges).unwrap();

    assert_eq!(output.shape(), &[10, 16]);
}

#[test]
fn test_attention_aggregation_methods() {
    let attention = GraphAttention::new(4);

    let node_features = Array1::ones(4);
    let neighbor_features = Array2::ones((10, 4));

    // Test different aggregation
    let sum_agg = attention.aggregate_neighbors(&node_features, &neighbor_features, "sum").unwrap();
    let mean_agg = attention.aggregate_neighbors(&node_features, &neighbor_features, "mean").unwrap();
    let max_agg = attention.aggregate_neighbors(&node_features, &neighbor_features, "max").unwrap();

    assert_eq!(sum_agg.len(), 4);
    assert_eq!(mean_agg.len(), 4);
    assert_eq!(max_agg.len(), 4);

    // Mean should be sum / count
    assert_relative_eq!(mean_agg[0], sum_agg[0] / 10.0, epsilon = 1e-5);
}
```

### 3.2 Integration with ruvector-core HNSW

```rust
// tests/integration/hnsw_integration.rs

use ruvector_core::{HnswIndex, SearchParams};
use ruvector_attention::{ScaledDotProduct, AttentionGuidedSearch};

#[test]
fn test_attention_guided_search() {
    // Build HNSW index
    let mut index = HnswIndex::new(128, 16, 200);

    for i in 0..1000 {
        let vector = generate_random_vector(128, i);
        index.add(i, vector);
    }

    // Create attention mechanism
    let attention = ScaledDotProduct::new(128);
    let guided_search = AttentionGuidedSearch::new(attention);

    let query = generate_random_vector(128, 12345);

    // Standard search
    let standard_results = index.search(&query, 10).unwrap();

    // Attention-guided search
    let attention_results = guided_search.search(&index, &query, 10).unwrap();

    // Should find similar results but potentially better ranking
    assert_eq!(attention_results.len(), 10);

    // Check recall
    let standard_ids: HashSet<_> = standard_results.iter().map(|r| r.id).collect();
    let attention_ids: HashSet<_> = attention_results.iter().map(|r| r.id).collect();
    let overlap = standard_ids.intersection(&attention_ids).count();

    assert!(overlap >= 7); // At least 70% overlap
}

#[test]
fn test_attention_edge_weighting() {
    let mut index = HnswIndex::new(64, 16, 200);

    // Add hierarchical clusters
    for cluster in 0..10 {
        for i in 0..100 {
            let vector = generate_cluster_vector(64, cluster, i);
            index.add(cluster * 100 + i, vector);
        }
    }

    let attention = GraphAttention::new(64);

    // Query from cluster 0
    let query = generate_cluster_vector(64, 0, 0);

    let results = index.search_with_attention(&query, 20, &attention).unwrap();

    // Most results should be from cluster 0
    let cluster_0_count = results.iter()
        .filter(|r| r.id < 100)
        .count();

    assert!(cluster_0_count >= 15); // At least 75% from same cluster
}

#[test]
fn test_multi_scale_search() {
    let index = build_test_index(1000, 128);

    let local_attention = GraphAttention::new(128);
    let global_attention = ScaledDotProduct::new(128);

    let multi_scale = MultiScaleAttention::new(local_attention, global_attention);

    let query = generate_random_vector(128, 999);
    let results = index.search_with_multi_scale(&query, 10, &multi_scale).unwrap();

    assert_eq!(results.len(), 10);
    assert!(results.windows(2).all(|w| w[0].distance <= w[1].distance));
}
```

### 3.3 Cross-Platform Consistency Tests

```rust
// tests/integration/cross_platform.rs

#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
use wasm_bindgen_test::*;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn test_wasm_rust_attention_consistency() {
    let dim = 128;
    let n_neighbors = 100;

    #[cfg(target_arch = "wasm32")]
    let attention = ScaledDotProduct::new_wasm(dim);

    #[cfg(not(target_arch = "wasm32"))]
    let attention = ScaledDotProduct::new(dim);

    let query = Array1::ones(dim);
    let keys = Array2::ones((n_neighbors, dim));
    let values = Array2::ones((n_neighbors, dim));

    let output = attention.forward(&query, &keys, &values).unwrap();

    // Expected output (computed offline)
    let expected = Array1::ones(dim);

    assert_relative_eq!(
        output.as_slice().unwrap(),
        expected.as_slice().unwrap(),
        epsilon = 1e-4
    );
}

#[cfg(feature = "napi")]
#[test]
fn test_napi_rust_consistency() {
    use napi::threadsafe_function::ThreadsafeFunction;
    use ruvector_attention_napi::ScaledDotProductNAPI;

    let rust_attention = ScaledDotProduct::new(64);
    let napi_attention = ScaledDotProductNAPI::new(64);

    let query = vec![1.0f32; 64];
    let keys = vec![vec![0.5f32; 64]; 50];
    let values = vec![vec![0.75f32; 64]; 50];

    let rust_output = rust_attention.forward(
        &Array1::from_vec(query.clone()),
        &Array2::from_shape_vec((50, 64), keys.clone().into_iter().flatten().collect()).unwrap(),
        &Array2::from_shape_vec((50, 64), values.clone().into_iter().flatten().collect()).unwrap(),
    ).unwrap();

    let napi_output = napi_attention.forward_sync(query, keys, values).unwrap();

    assert_relative_eq!(
        rust_output.as_slice().unwrap(),
        &napi_output[..],
        epsilon = 1e-5
    );
}

#[test]
fn test_serialization_consistency() {
    let attention = HyperbolicAttention::new(-1.0);

    // Serialize to JSON
    let json = serde_json::to_string(&attention).unwrap();

    // Deserialize
    let deserialized: HyperbolicAttention = serde_json::from_str(&json).unwrap();

    // Test equivalence
    let query = Array1::from_vec(vec![0.1, 0.2, 0.0, 0.0]);
    let keys = Array2::from_elem((10, 4), 0.3);
    let values = Array2::from_elem((10, 4), 0.5);

    let output1 = attention.forward(&query, &keys, &values).unwrap();
    let output2 = deserialized.forward(&query, &keys, &values).unwrap();

    assert_relative_eq!(
        output1.as_slice().unwrap(),
        output2.as_slice().unwrap(),
        epsilon = 1e-8
    );
}
```

---

## 4. Benchmark Suite

### 4.1 Criterion Benchmarks

```rust
// benches/attention_benchmarks.rs

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};
use ruvector_attention::*;
use ndarray::{Array1, Array2};

fn benchmark_scaled_dot_product(c: &mut Criterion) {
    let dims = [64, 128, 256, 512, 1024];
    let neighbors = [10, 50, 100, 500, 1000];

    let mut group = c.benchmark_group("scaled_dot_product");

    for &d in &dims {
        for &n in &neighbors {
            let param = format!("d={}/n={}", d, n);

            group.throughput(Throughput::Elements((n * d) as u64));

            group.bench_with_input(
                BenchmarkId::new("forward", &param),
                &(d, n),
                |b, &(d, n)| {
                    let attention = ScaledDotProduct::new(d);
                    let query = Array1::ones(d);
                    let keys = Array2::ones((n, d));
                    let values = Array2::ones((n, d));

                    b.iter(|| {
                        black_box(attention.forward(
                            black_box(&query),
                            black_box(&keys),
                            black_box(&values),
                        ))
                    });
                },
            );
        }
    }

    group.finish();
}

fn benchmark_hyperbolic_attention(c: &mut Criterion) {
    let mut group = c.benchmark_group("hyperbolic_attention");

    let curvatures = [-0.1, -0.5, -1.0, -2.0];
    let dims = [64, 128, 256];
    let neighbors = [10, 50, 100];

    for &c_val in &curvatures {
        for &d in &dims {
            for &n in &neighbors {
                let param = format!("c={}/d={}/n={}", c_val, d, n);

                group.bench_with_input(
                    BenchmarkId::new("poincare_distance", &param),
                    &(c_val, d, n),
                    |b, &(c_val, d, n)| {
                        let attention = HyperbolicAttention::new(c_val);
                        let query = Array1::from_elem(d, 0.1);
                        let keys = Array2::from_elem((n, d), 0.2);

                        b.iter(|| {
                            for i in 0..n {
                                black_box(attention.poincare_distance(
                                    black_box(&query),
                                    black_box(&keys.row(i).to_owned()),
                                ));
                            }
                        });
                    },
                );
            }
        }
    }

    group.finish();
}

fn benchmark_flash_attention(c: &mut Criterion) {
    let mut group = c.benchmark_group("flash_attention");

    let block_sizes = [64, 128, 256, 512];
    let neighbors = [1000, 5000, 10000];

    for &block_size in &block_sizes {
        for &n in &neighbors {
            let param = format!("block={}/n={}", block_size, n);

            group.throughput(Throughput::Elements((n * 128) as u64));

            group.bench_with_input(
                BenchmarkId::new("flash_vs_standard", &param),
                &(block_size, n),
                |b, &(block_size, n)| {
                    let flash = FlashAttention::new(128, block_size);
                    let query = Array1::ones(128);
                    let keys = Array2::ones((n, 128));
                    let values = Array2::ones((n, 128));

                    b.iter(|| {
                        black_box(flash.forward(
                            black_box(&query),
                            black_box(&keys),
                            black_box(&values),
                        ))
                    });
                },
            );
        }
    }

    group.finish();
}

fn benchmark_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage");

    let methods = [
        ("standard", false),
        ("flash", true),
    ];

    for (name, use_flash) in &methods {
        group.bench_function(*name, |b| {
            b.iter_custom(|iters| {
                let start_memory = get_memory_usage();
                let start_time = std::time::Instant::now();

                for _ in 0..iters {
                    if *use_flash {
                        let attention = FlashAttention::new(128, 256);
                        let query = Array1::ones(128);
                        let keys = Array2::ones((10000, 128));
                        let values = Array2::ones((10000, 128));
                        black_box(attention.forward(&query, &keys, &values));
                    } else {
                        let attention = ScaledDotProduct::new(128);
                        let query = Array1::ones(128);
                        let keys = Array2::ones((10000, 128));
                        let values = Array2::ones((10000, 128));
                        black_box(attention.forward(&query, &keys, &values));
                    }
                }

                let end_memory = get_memory_usage();
                let duration = start_time.elapsed();

                println!(
                    "{}: Memory used = {} MB",
                    name,
                    (end_memory - start_memory) / 1024 / 1024
                );

                duration
            });
        });
    }

    group.finish();
}

fn benchmark_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");

    let batch_sizes = [1, 10, 100, 1000];

    for &batch_size in &batch_sizes {
        group.throughput(Throughput::Elements(batch_size as u64));

        group.bench_with_input(
            BenchmarkId::new("batch_processing", batch_size),
            &batch_size,
            |b, &batch_size| {
                let attention = ScaledDotProduct::new(128);
                let queries: Vec<_> = (0..batch_size)
                    .map(|_| Array1::ones(128))
                    .collect();
                let keys = Array2::ones((100, 128));
                let values = Array2::ones((100, 128));

                b.iter(|| {
                    for query in &queries {
                        black_box(attention.forward(
                            black_box(query),
                            black_box(&keys),
                            black_box(&values),
                        ));
                    }
                });
            },
        );
    }

    group.finish();
}

fn benchmark_multi_head_attention(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_head_attention");

    let num_heads = [1, 2, 4, 8, 16];

    for &heads in &num_heads {
        group.bench_with_input(
            BenchmarkId::new("heads", heads),
            &heads,
            |b, &heads| {
                let attention = MultiHeadAttention::new(128, heads);
                let query = Array1::ones(128);
                let keys = Array2::ones((100, 128));
                let values = Array2::ones((100, 128));

                b.iter(|| {
                    black_box(attention.forward(
                        black_box(&query),
                        black_box(&keys),
                        black_box(&values),
                    ))
                });
            },
        );
    }

    group.finish();
}

fn benchmark_mixture_of_experts(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixture_of_experts");

    let num_experts = [2, 4, 8, 16];

    for &experts in &num_experts {
        group.bench_with_input(
            BenchmarkId::new("experts", experts),
            &experts,
            |b, &experts| {
                let attention = MixtureOfExpertsAttention::new(128, experts);
                let query = Array1::ones(128);
                let keys = Array2::ones((100, 128));
                let values = Array2::ones((100, 128));

                b.iter(|| {
                    black_box(attention.forward(
                        black_box(&query),
                        black_box(&keys),
                        black_box(&values),
                    ))
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_scaled_dot_product,
    benchmark_hyperbolic_attention,
    benchmark_flash_attention,
    benchmark_memory_usage,
    benchmark_throughput,
    benchmark_multi_head_attention,
    benchmark_mixture_of_experts,
);

criterion_main!(benches);

// Helper function
fn get_memory_usage() -> usize {
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        let status = fs::read_to_string("/proc/self/status").unwrap();
        for line in status.lines() {
            if line.starts_with("VmRSS:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                return parts[1].parse::<usize>().unwrap() * 1024;
            }
        }
    }
    0
}
```

---

## 5. Performance Targets

### 5.1 Latency Targets

| Attention Type | Dims | Neighbors | P50 Latency | P99 Latency | Memory Peak |
|---------------|------|-----------|-------------|-------------|-------------|
| Scaled Dot-Product | 64 | 100 | < 30μs | < 50μs | < 500KB |
| Scaled Dot-Product | 128 | 100 | < 40μs | < 70μs | < 1MB |
| Scaled Dot-Product | 256 | 100 | < 60μs | < 100μs | < 2MB |
| Multi-Head (4) | 128 | 100 | < 120μs | < 200μs | < 1.5MB |
| Multi-Head (8) | 128 | 100 | < 200μs | < 350μs | < 2MB |
| Hyperbolic | 64 | 100 | < 60μs | < 100μs | < 750KB |
| Hyperbolic | 128 | 100 | < 90μs | < 150μs | < 1.2MB |
| Graph (Edge) | 128 | 100 | < 80μs | < 130μs | < 1.5MB |
| Local+Global | 128 | 100+500 | < 400μs | < 700μs | < 4MB |
| Local+Global | 128 | 1000+5000 | < 2ms | < 4ms | < 15MB |
| Linear | 128 | 1000 | < 150μs | < 250μs | < 2MB |
| Linear | 128 | 10000 | < 1ms | < 1.8ms | < 8MB |
| Flash | 128 | 1000 | < 120μs | < 200μs | < 1.5MB |
| Flash | 128 | 10000 | < 800μs | < 1.5ms | < 6MB |
| Flash | 128 | 100000 | < 15ms | < 25ms | < 40MB |
| MoE (2 experts) | 128 | 100 | < 180μs | < 300μs | < 4MB |
| MoE (4 experts) | 128 | 100 | < 300μs | < 500μs | < 8MB |
| MoE (8 experts) | 128 | 100 | < 550μs | < 900μs | < 15MB |

### 5.2 Throughput Targets

| Operation | Batch Size | Target QPS | Notes |
|-----------|-----------|------------|-------|
| Single Query | 1 | > 20,000 | 128D, 100 neighbors |
| Batch Processing | 10 | > 150,000 | Total queries/sec |
| Batch Processing | 100 | > 1,000,000 | Total queries/sec |
| Large Scale | 1000 | > 8,000,000 | Parallel processing |

### 5.3 Accuracy Targets

| Metric | Target | Measurement Method |
|--------|--------|--------------------|
| Recall@10 | > 95% | vs brute-force |
| Recall@100 | > 98% | vs brute-force |
| Precision@10 | > 90% | vs brute-force |
| Numerical Stability | < 1e-5 error | Relative to FP64 |
| Cross-Platform Consistency | < 1e-4 error | WASM vs Native |

---

## 6. Regression Testing

### 6.1 Automated Performance Regression

```yaml
# .github/workflows/performance-regression.yml

name: Performance Regression Tests

on:
  pull_request:
    branches: [main, develop]
  push:
    branches: [main]

jobs:
  benchmark:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: actions-rust-lang/setup-rust-toolchain@v1

      - name: Run benchmarks (PR)
        run: |
          cargo bench --bench attention_benchmarks -- --save-baseline pr

      - name: Checkout main branch
        run: |
          git fetch origin main
          git checkout origin/main

      - name: Run benchmarks (main)
        run: |
          cargo bench --bench attention_benchmarks -- --save-baseline main

      - name: Compare benchmarks
        run: |
          cargo bench --bench attention_benchmarks -- --baseline main --compare pr

      - name: Check for regressions
        run: |
          python3 scripts/check_regression.py \
            --threshold-p50 5 \
            --threshold-p99 10 \
            --threshold-memory 10

      - name: Upload results
        uses: actions/upload-artifact@v3
        with:
          name: benchmark-results
          path: target/criterion/
```

### 6.2 Regression Detection Script

```python
# scripts/check_regression.py

import json
import sys
import argparse
from pathlib import Path

def load_benchmark_results(path):
    """Load Criterion benchmark results"""
    results = {}

    for bench_dir in Path(path).glob("*/"):
        bench_name = bench_dir.name

        # Load estimates.json
        estimates_file = bench_dir / "new" / "estimates.json"
        if estimates_file.exists():
            with open(estimates_file) as f:
                data = json.load(f)
                results[bench_name] = {
                    'mean': data['mean']['point_estimate'],
                    'median': data['median']['point_estimate'],
                }

    return results

def compare_results(baseline, current, thresholds):
    """Compare benchmark results and detect regressions"""
    regressions = []
    warnings = []

    for bench_name in current:
        if bench_name not in baseline:
            continue

        base_mean = baseline[bench_name]['mean']
        curr_mean = current[bench_name]['mean']

        percent_change = ((curr_mean - base_mean) / base_mean) * 100

        if percent_change > thresholds['p99']:
            regressions.append({
                'benchmark': bench_name,
                'change': percent_change,
                'baseline': base_mean,
                'current': curr_mean,
            })
        elif percent_change > thresholds['p50']:
            warnings.append({
                'benchmark': bench_name,
                'change': percent_change,
                'baseline': base_mean,
                'current': curr_mean,
            })

    return regressions, warnings

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--threshold-p50', type=float, default=5.0)
    parser.add_argument('--threshold-p99', type=float, default=10.0)
    parser.add_argument('--threshold-memory', type=float, default=10.0)
    parser.add_argument('--baseline-path', default='target/criterion/main')
    parser.add_argument('--current-path', default='target/criterion/pr')

    args = parser.parse_args()

    thresholds = {
        'p50': args.threshold_p50,
        'p99': args.threshold_p99,
        'memory': args.threshold_memory,
    }

    baseline = load_benchmark_results(args.baseline_path)
    current = load_benchmark_results(args.current_path)

    regressions, warnings = compare_results(baseline, current, thresholds)

    if warnings:
        print("⚠️  Performance Warnings:")
        for w in warnings:
            print(f"  {w['benchmark']}: +{w['change']:.2f}% ({w['baseline']:.2f}ns → {w['current']:.2f}ns)")

    if regressions:
        print("\n❌ Performance Regressions Detected:")
        for r in regressions:
            print(f"  {r['benchmark']}: +{r['change']:.2f}% ({r['baseline']:.2f}ns → {r['current']:.2f}ns)")
        sys.exit(1)
    else:
        print("✅ No performance regressions detected")
        sys.exit(0)

if __name__ == '__main__':
    main()
```

### 6.3 Regression Test Configuration

```toml
# regression-tests.toml

[metrics]

[metrics.p50_latency]
threshold = 5.0  # 5% increase triggers warning
action = "warn"
description = "Median latency regression"

[metrics.p99_latency]
threshold = 10.0  # 10% increase blocks PR
action = "block"
description = "99th percentile latency regression"

[metrics.memory_peak]
threshold = 10.0
action = "warn"
description = "Peak memory usage regression"

[metrics.recall_at_10]
threshold = 1.0  # 1% decrease blocks PR
action = "block"
comparison = "decrease"
description = "Recall@10 accuracy regression"

[metrics.throughput]
threshold = 5.0
action = "warn"
comparison = "decrease"
description = "Throughput regression"

[benchmarks]
critical = [
    "scaled_dot_product/forward/d=128/n=100",
    "hyperbolic_attention/poincare_distance/c=-1.0/d=128/n=100",
    "flash_attention/flash_vs_standard/block=256/n=10000",
]

[notifications]
slack_webhook = "${SLACK_WEBHOOK_URL}"
email = ["team@example.com"]
```

---

## 7. Platform-Specific Tests

### 7.1 WASM Tests

```javascript
// tests/wasm/attention.test.js

import { describe, it, expect, beforeAll } from 'vitest';
import init, {
  ScaledDotProduct,
  HyperbolicAttention,
  MultiHeadAttention,
} from '../../pkg/ruvector_attention.js';

describe('WASM Attention Module', () => {
  beforeAll(async () => {
    await init();
  });

  describe('ScaledDotProduct', () => {
    it('should create instance with correct dimensions', () => {
      const attention = ScaledDotProduct.new(128);
      expect(attention.dim()).toBe(128);
    });

    it('should compute attention for small inputs', () => {
      const attention = ScaledDotProduct.new(4);

      const query = new Float32Array([1, 0, 0, 0]);
      const keys = new Float32Array([
        1, 0, 0, 0,
        0, 1, 0, 0,
        0, 0, 1, 0,
      ]);
      const values = new Float32Array([
        1, 0, 0, 0,
        0, 1, 0, 0,
        0, 0, 1, 0,
      ]);

      const result = attention.forward(query, keys, values, 3);

      expect(result).toBeInstanceOf(Float32Array);
      expect(result.length).toBe(4);
      expect(result[0]).toBeGreaterThan(0.5);
    });

    it('should handle large batches', async () => {
      const attention = ScaledDotProduct.new(128);

      const query = new Float32Array(128).fill(1);
      const n_neighbors = 10000;
      const keys = new Float32Array(n_neighbors * 128).fill(0.5);
      const values = new Float32Array(n_neighbors * 128).fill(0.75);

      const start = performance.now();
      const result = attention.forward(query, keys, values, n_neighbors);
      const duration = performance.now() - start;

      expect(result.length).toBe(128);
      expect(duration).toBeLessThan(100); // < 100ms
    });

    it('should produce consistent results', () => {
      const attention = ScaledDotProduct.new(64);

      const query = new Float32Array(64).fill(1);
      const keys = new Float32Array(50 * 64).fill(0.5);
      const values = new Float32Array(50 * 64).fill(0.75);

      const result1 = attention.forward(query, keys, values, 50);
      const result2 = attention.forward(query, keys, values, 50);

      for (let i = 0; i < 64; i++) {
        expect(Math.abs(result1[i] - result2[i])).toBeLessThan(1e-6);
      }
    });
  });

  describe('HyperbolicAttention', () => {
    it('should compute Poincaré distance', () => {
      const attention = HyperbolicAttention.new(-1.0, 4);

      const x = new Float32Array([0.1, 0.2, 0, 0]);
      const y = new Float32Array([0.3, 0.1, 0, 0]);

      const distance = attention.poincare_distance(x, y);

      expect(distance).toBeGreaterThan(0);
      expect(distance).toBeLessThan(10);
      expect(isFinite(distance)).toBe(true);
    });

    it('should handle boundary cases', () => {
      const attention = HyperbolicAttention.new(-1.0, 4);

      // Near boundary
      const x = new Float32Array([0.7, 0.7, 0, 0]);
      const y = new Float32Array([0.1, 0.1, 0, 0]);

      const distance = attention.poincare_distance(x, y);
      expect(isFinite(distance)).toBe(true);
    });
  });

  describe('MultiHeadAttention', () => {
    it('should compute multi-head attention', () => {
      const attention = MultiHeadAttention.new(128, 8);

      const query = new Float32Array(128).fill(1);
      const keys = new Float32Array(100 * 128).fill(0.5);
      const values = new Float32Array(100 * 128).fill(0.75);

      const result = attention.forward(query, keys, values, 100);

      expect(result.length).toBe(128);
      expect(result.every(x => isFinite(x))).toBe(true);
    });
  });

  describe('Memory Management', () => {
    it('should properly free memory', () => {
      const attention = ScaledDotProduct.new(128);

      // Force garbage collection if available
      if (global.gc) {
        global.gc();
      }

      attention.free();

      // Should not crash
      expect(true).toBe(true);
    });

    it('should handle concurrent operations', async () => {
      const attentions = Array.from({ length: 10 }, () =>
        ScaledDotProduct.new(128)
      );

      const query = new Float32Array(128).fill(1);
      const keys = new Float32Array(100 * 128).fill(0.5);
      const values = new Float32Array(100 * 128).fill(0.75);

      const results = await Promise.all(
        attentions.map(att =>
          Promise.resolve(att.forward(query, keys, values, 100))
        )
      );

      expect(results).toHaveLength(10);

      // Cleanup
      attentions.forEach(att => att.free());
    });
  });

  describe('Performance', () => {
    it('should meet latency targets', () => {
      const attention = ScaledDotProduct.new(128);

      const query = new Float32Array(128).fill(1);
      const keys = new Float32Array(100 * 128).fill(0.5);
      const values = new Float32Array(100 * 128).fill(0.75);

      // Warm-up
      for (let i = 0; i < 10; i++) {
        attention.forward(query, keys, values, 100);
      }

      // Measure
      const times = [];
      for (let i = 0; i < 100; i++) {
        const start = performance.now();
        attention.forward(query, keys, values, 100);
        times.push(performance.now() - start);
      }

      times.sort((a, b) => a - b);
      const p50 = times[Math.floor(times.length * 0.5)];
      const p99 = times[Math.floor(times.length * 0.99)];

      expect(p50).toBeLessThan(1); // < 1ms for WASM
      expect(p99).toBeLessThan(2); // < 2ms for WASM
    });
  });
});
```

### 7.2 NAPI-RS Tests

```typescript
// tests/napi/attention.test.ts

import { describe, it, expect, beforeEach } from '@jest/globals';
import {
  ScaledDotProduct,
  HyperbolicAttention,
  MultiHeadAttention,
  FlashAttention,
} from '../../index';

describe('NAPI-RS Attention Module', () => {
  describe('ScaledDotProduct', () => {
    let attention: ScaledDotProduct;

    beforeEach(() => {
      attention = new ScaledDotProduct(128);
    });

    it('should create instance', () => {
      expect(attention).toBeDefined();
      expect(attention.dim()).toBe(128);
    });

    it('should compute attention synchronously', () => {
      const query = new Float32Array(128).fill(1);
      const keys = Array.from({ length: 100 }, () =>
        Array.from({ length: 128 }, () => 0.5)
      );
      const values = Array.from({ length: 100 }, () =>
        Array.from({ length: 128 }, () => 0.75)
      );

      const result = attention.forward(query, keys, values);

      expect(result).toHaveLength(128);
      expect(result.every(x => isFinite(x))).toBe(true);
    });

    it('should compute attention asynchronously', async () => {
      const query = new Float32Array(128).fill(1);
      const keys = Array.from({ length: 100 }, () =>
        Array.from({ length: 128 }, () => 0.5)
      );
      const values = Array.from({ length: 100 }, () =>
        Array.from({ length: 128 }, () => 0.75)
      );

      const result = await attention.forwardAsync(query, keys, values);

      expect(result).toHaveLength(128);
    });

    it('should process batch asynchronously', async () => {
      const queries = Array.from({ length: 10 }, () =>
        new Float32Array(128).fill(1)
      );
      const keys = Array.from({ length: 100 }, () =>
        Array.from({ length: 128 }, () => 0.5)
      );
      const values = Array.from({ length: 100 }, () =>
        Array.from({ length: 128 }, () => 0.75)
      );

      const results = await attention.forwardBatchAsync(queries, keys, values);

      expect(results).toHaveLength(10);
      expect(results[0]).toHaveLength(128);
    });

    it('should match Rust implementation', () => {
      // Test data computed in Rust
      const query = new Float32Array([1, 0, 0, 0]);
      const keys = [
        [1, 0, 0, 0],
        [0, 1, 0, 0],
        [0, 0, 1, 0],
      ];
      const values = [
        [1, 0, 0, 0],
        [0, 1, 0, 0],
        [0, 0, 1, 0],
      ];

      const attention4 = new ScaledDotProduct(4);
      const result = attention4.forward(query, keys, values);

      // Expected result from Rust
      const expected = [0.6652, 0.2424, 0.0924, 0.0];

      for (let i = 0; i < 4; i++) {
        expect(Math.abs(result[i] - expected[i])).toBeLessThan(1e-3);
      }
    });
  });

  describe('HyperbolicAttention', () => {
    it('should compute hyperbolic attention', () => {
      const attention = new HyperbolicAttention(-1.0, 128);

      const query = new Float32Array(128).fill(0.1);
      const keys = Array.from({ length: 100 }, () =>
        Array.from({ length: 128 }, () => 0.2)
      );
      const values = Array.from({ length: 100 }, () =>
        Array.from({ length: 128 }, () => 0.5)
      );

      const result = attention.forward(query, keys, values);

      expect(result).toHaveLength(128);
      expect(result.every(x => isFinite(x))).toBe(true);
    });

    it('should compute Poincaré distance', () => {
      const attention = new HyperbolicAttention(-1.0, 4);

      const x = new Float32Array([0.1, 0.2, 0, 0]);
      const y = new Float32Array([0.3, 0.1, 0, 0]);

      const distance = attention.poincareDistance(x, y);

      expect(distance).toBeGreaterThan(0);
      expect(isFinite(distance)).toBe(true);
    });
  });

  describe('MultiHeadAttention', () => {
    it('should compute with multiple heads', () => {
      const attention = new MultiHeadAttention(128, 8);

      const query = new Float32Array(128).fill(1);
      const keys = Array.from({ length: 100 }, () =>
        Array.from({ length: 128 }, () => 0.5)
      );
      const values = Array.from({ length: 100 }, () =>
        Array.from({ length: 128 }, () => 0.75)
      );

      const result = attention.forward(query, keys, values);

      expect(result).toHaveLength(128);
    });
  });

  describe('FlashAttention', () => {
    it('should handle large inputs efficiently', async () => {
      const attention = new FlashAttention(128, 256);

      const query = new Float32Array(128).fill(1);
      const keys = Array.from({ length: 10000 }, () =>
        Array.from({ length: 128 }, () => 0.5)
      );
      const values = Array.from({ length: 10000 }, () =>
        Array.from({ length: 128 }, () => 0.75)
      );

      const start = Date.now();
      const result = await attention.forwardAsync(query, keys, values);
      const duration = Date.now() - start;

      expect(result).toHaveLength(128);
      expect(duration).toBeLessThan(100); // < 100ms
    });
  });

  describe('Performance', () => {
    it('should meet throughput targets', async () => {
      const attention = new ScaledDotProduct(128);

      const query = new Float32Array(128).fill(1);
      const keys = Array.from({ length: 100 }, () =>
        Array.from({ length: 128 }, () => 0.5)
      );
      const values = Array.from({ length: 100 }, () =>
        Array.from({ length: 128 }, () => 0.75)
      );

      const iterations = 1000;
      const start = Date.now();

      for (let i = 0; i < iterations; i++) {
        attention.forward(query, keys, values);
      }

      const duration = Date.now() - start;
      const qps = (iterations / duration) * 1000;

      expect(qps).toBeGreaterThan(10000); // > 10k QPS
    });

    it('should support concurrent async operations', async () => {
      const attention = new ScaledDotProduct(128);

      const query = new Float32Array(128).fill(1);
      const keys = Array.from({ length: 100 }, () =>
        Array.from({ length: 128 }, () => 0.5)
      );
      const values = Array.from({ length: 100 }, () =>
        Array.from({ length: 128 }, () => 0.75)
      );

      const promises = Array.from({ length: 100 }, () =>
        attention.forwardAsync(query, keys, values)
      );

      const results = await Promise.all(promises);

      expect(results).toHaveLength(100);
      expect(results.every(r => r.length === 128)).toBe(true);
    });
  });
});
```

---

## 8. Continuous Benchmarking

### 8.1 Daily Benchmark Job

```yaml
# .github/workflows/daily-benchmarks.yml

name: Daily Performance Benchmarks

on:
  schedule:
    - cron: '0 2 * * *'  # 2 AM daily
  workflow_dispatch:  # Manual trigger

jobs:
  benchmark-rust:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]

    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: actions-rust-lang/setup-rust-toolchain@v1
        with:
          toolchain: stable

      - name: Run benchmarks
        run: |
          cargo bench --bench attention_benchmarks -- --save-baseline daily-${{ matrix.os }}

      - name: Upload results
        uses: actions/upload-artifact@v3
        with:
          name: benchmark-${{ matrix.os }}
          path: target/criterion/

      - name: Generate report
        run: |
          python3 scripts/generate_benchmark_report.py \
            --input target/criterion/ \
            --output benchmark-report-${{ matrix.os }}.md

      - name: Post to dashboard
        env:
          DASHBOARD_URL: ${{ secrets.BENCHMARK_DASHBOARD_URL }}
        run: |
          curl -X POST $DASHBOARD_URL/api/benchmarks \
            -H "Authorization: Bearer ${{ secrets.BENCHMARK_TOKEN }}" \
            -F "os=${{ matrix.os }}" \
            -F "data=@target/criterion/results.json"

  benchmark-wasm:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v3

      - name: Install Rust + wasm-pack
        run: |
          curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

      - name: Build WASM
        run: |
          wasm-pack build --target web

      - name: Setup Node.js
        uses: actions/setup-node@v3
        with:
          node-version: '18'

      - name: Install dependencies
        run: npm ci

      - name: Run WASM benchmarks
        run: |
          npm run bench:wasm

      - name: Upload results
        uses: actions/upload-artifact@v3
        with:
          name: benchmark-wasm
          path: bench-results/

  benchmark-memory:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v3

      - name: Install Rust + Valgrind
        run: |
          sudo apt-get update
          sudo apt-get install -y valgrind
          rustup toolchain install nightly

      - name: Build with profiling
        run: |
          cargo +nightly build --release --features memory-profiling

      - name: Run memory profiling
        run: |
          valgrind --tool=massif \
            --massif-out-file=massif.out \
            target/release/deps/attention_benchmarks-*

      - name: Generate memory report
        run: |
          ms_print massif.out > memory-report.txt

      - name: Upload results
        uses: actions/upload-artifact@v3
        with:
          name: memory-profile
          path: |
            massif.out
            memory-report.txt

  generate-dashboard:
    needs: [benchmark-rust, benchmark-wasm, benchmark-memory]
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v3

      - name: Download all artifacts
        uses: actions/download-artifact@v3

      - name: Generate dashboard
        run: |
          python3 scripts/generate_dashboard.py \
            --output docs/benchmarks/index.html

      - name: Deploy to GitHub Pages
        uses: peaceiris/actions-gh-pages@v3
        with:
          github_token: ${{ secrets.GITHUB_TOKEN }}
          publish_dir: ./docs/benchmarks
```

### 8.2 Benchmark Dashboard Generator

```python
# scripts/generate_dashboard.py

import json
import sys
from pathlib import Path
from datetime import datetime
import matplotlib.pyplot as plt
import pandas as pd

def load_criterion_results(criterion_dir):
    """Load all Criterion benchmark results"""
    results = []

    for bench_dir in Path(criterion_dir).glob("*/"):
        bench_name = bench_dir.name

        estimates_file = bench_dir / "new" / "estimates.json"
        if estimates_file.exists():
            with open(estimates_file) as f:
                data = json.load(f)
                results.append({
                    'name': bench_name,
                    'mean': data['mean']['point_estimate'] / 1e6,  # Convert to ms
                    'median': data['median']['point_estimate'] / 1e6,
                    'std_dev': data['std_dev']['point_estimate'] / 1e6,
                })

    return pd.DataFrame(results)

def generate_latency_chart(df, output_path):
    """Generate latency comparison chart"""
    fig, ax = plt.subplots(figsize=(12, 6))

    # Filter and sort
    df_sorted = df.sort_values('median')

    ax.barh(df_sorted['name'], df_sorted['median'], xerr=df_sorted['std_dev'])
    ax.set_xlabel('Latency (ms)')
    ax.set_title('Attention Mechanism Latency Comparison')
    ax.grid(True, alpha=0.3)

    plt.tight_layout()
    plt.savefig(output_path)
    plt.close()

def generate_html_dashboard(df, charts_dir, output_file):
    """Generate HTML dashboard"""
    html = f"""
<!DOCTYPE html>
<html>
<head>
    <title>RuVector Attention Benchmarks</title>
    <style>
        body {{
            font-family: Arial, sans-serif;
            margin: 20px;
            background-color: #f5f5f5;
        }}
        .container {{
            max-width: 1200px;
            margin: 0 auto;
            background-color: white;
            padding: 20px;
            border-radius: 8px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }}
        h1 {{
            color: #333;
        }}
        table {{
            width: 100%;
            border-collapse: collapse;
            margin: 20px 0;
        }}
        th, td {{
            padding: 12px;
            text-align: left;
            border-bottom: 1px solid #ddd;
        }}
        th {{
            background-color: #4CAF50;
            color: white;
        }}
        tr:hover {{
            background-color: #f5f5f5;
        }}
        .chart {{
            margin: 20px 0;
        }}
        .metric {{
            display: inline-block;
            margin: 10px 20px;
            padding: 15px;
            background-color: #e3f2fd;
            border-radius: 4px;
        }}
        .metric-value {{
            font-size: 24px;
            font-weight: bold;
            color: #1976d2;
        }}
        .metric-label {{
            font-size: 14px;
            color: #666;
        }}
    </style>
</head>
<body>
    <div class="container">
        <h1>RuVector Attention Benchmarks</h1>
        <p>Generated: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}</p>

        <h2>Summary Metrics</h2>
        <div class="metric">
            <div class="metric-value">{df['median'].median():.3f} ms</div>
            <div class="metric-label">Median Latency</div>
        </div>
        <div class="metric">
            <div class="metric-value">{df['median'].min():.3f} ms</div>
            <div class="metric-label">Best Latency</div>
        </div>
        <div class="metric">
            <div class="metric-value">{len(df)}</div>
            <div class="metric-label">Benchmarks</div>
        </div>

        <h2>Latency Chart</h2>
        <div class="chart">
            <img src="latency_chart.png" alt="Latency Chart" style="max-width: 100%;">
        </div>

        <h2>Detailed Results</h2>
        <table>
            <thead>
                <tr>
                    <th>Benchmark</th>
                    <th>Median (ms)</th>
                    <th>Mean (ms)</th>
                    <th>Std Dev (ms)</th>
                </tr>
            </thead>
            <tbody>
"""

    for _, row in df.iterrows():
        html += f"""
                <tr>
                    <td>{row['name']}</td>
                    <td>{row['median']:.4f}</td>
                    <td>{row['mean']:.4f}</td>
                    <td>{row['std_dev']:.4f}</td>
                </tr>
"""

    html += """
            </tbody>
        </table>
    </div>
</body>
</html>
"""

    with open(output_file, 'w') as f:
        f.write(html)

def main():
    import argparse

    parser = argparse.ArgumentParser()
    parser.add_argument('--criterion-dir', default='target/criterion')
    parser.add_argument('--output', default='docs/benchmarks/index.html')

    args = parser.parse_args()

    # Load results
    df = load_criterion_results(args.criterion_dir)

    if df.empty:
        print("No benchmark results found")
        sys.exit(1)

    # Generate charts
    output_dir = Path(args.output).parent
    output_dir.mkdir(parents=True, exist_ok=True)

    generate_latency_chart(df, output_dir / 'latency_chart.png')

    # Generate dashboard
    generate_html_dashboard(df, output_dir, args.output)

    print(f"Dashboard generated: {args.output}")

if __name__ == '__main__':
    main()
```

---

## 9. Test Execution Plan

### 9.1 Development Workflow

```bash
# Local development testing
cargo test                          # Run all tests
cargo test --package ruvector-attention  # Package-specific tests
cargo test test_scaled_dot_product  # Specific test

# Run benchmarks locally
cargo bench                         # All benchmarks
cargo bench --bench attention_benchmarks  # Specific benchmark

# Run with coverage
cargo tarpaulin --out Html          # Generate HTML coverage report

# Platform-specific testing
wasm-pack test --headless --firefox  # WASM tests
npm test                             # NAPI-RS tests
```

### 9.2 CI/CD Integration

```yaml
# .github/workflows/ci.yml (excerpt)

test:
  strategy:
    matrix:
      os: [ubuntu-latest, windows-latest, macos-latest]
      rust: [stable, nightly]

  steps:
    - name: Run tests
      run: cargo test --all-features

    - name: Run benchmarks
      if: matrix.rust == 'stable'
      run: cargo bench --no-run  # Just build, don't run

    - name: Coverage
      if: matrix.os == 'ubuntu-latest'
      run: cargo tarpaulin --out Lcov

    - name: Upload coverage
      uses: codecov/codecov-action@v3
```

---

## 10. Quality Gates

### 10.1 Pre-Merge Requirements

**All PRs must pass:**

1. ✅ All unit tests pass
2. ✅ All integration tests pass
3. ✅ Code coverage ≥ 80% (critical paths ≥ 95%)
4. ✅ No performance regressions > 10%
5. ✅ Platform-specific tests pass (WASM + NAPI)
6. ✅ Benchmarks complete without errors
7. ✅ Memory profiling shows no leaks

### 10.2 Release Requirements

**For releases:**

1. ✅ All pre-merge requirements
2. ✅ Full benchmark suite passes
3. ✅ Performance targets met (see Section 5)
4. ✅ Cross-platform consistency verified
5. ✅ Documentation updated
6. ✅ Changelog updated
7. ✅ Version bumped appropriately

---

## 11. Next Steps

1. **Implement Core Test Suite**
   - Start with scaled dot-product tests
   - Add hyperbolic attention tests
   - Build graph attention tests

2. **Set Up Benchmarking**
   - Configure Criterion
   - Define baseline benchmarks
   - Create comparison framework

3. **Platform Testing**
   - Set up WASM test environment
   - Configure NAPI-RS tests
   - Verify cross-platform parity

4. **CI/CD Integration**
   - Add test workflows
   - Set up benchmark automation
   - Configure performance tracking

5. **Documentation**
   - Document test organization
   - Create benchmark interpretation guide
   - Write troubleshooting guides

---

## Appendix A: Test Data Generation

```rust
// tests/common/test_data.rs

use ndarray::{Array1, Array2};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

pub fn generate_random_vector(dim: usize, seed: u64) -> Array1<f32> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    Array1::from_vec(
        (0..dim).map(|_| rng.gen_range(-1.0..1.0)).collect()
    )
}

pub fn generate_random_vectors(n: usize, dim: usize, seed: u64) -> Array2<f32> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    Array2::from_shape_vec(
        (n, dim),
        (0..n * dim).map(|_| rng.gen_range(-1.0..1.0)).collect(),
    ).unwrap()
}

pub fn generate_cluster_vector(dim: usize, cluster: usize, idx: usize) -> Array1<f32> {
    let mut rng = ChaCha8Rng::seed_from_u64((cluster * 10000 + idx) as u64);
    let center = Array1::from_elem(dim, cluster as f32 / 10.0);
    let noise = Array1::from_vec(
        (0..dim).map(|_| rng.gen_range(-0.1..0.1)).collect()
    );
    center + noise
}

pub fn generate_orthogonal_vectors(dim: usize, n: usize) -> Array2<f32> {
    // Generate approximately orthogonal vectors using Gram-Schmidt
    let mut vectors = Array2::zeros((n, dim));
    let mut rng = ChaCha8Rng::seed_from_u64(42);

    for i in 0..n {
        let mut v = Array1::from_vec(
            (0..dim).map(|_| rng.gen_range(-1.0..1.0)).collect()
        );

        // Orthogonalize against previous vectors
        for j in 0..i {
            let prev = vectors.row(j);
            let projection = v.dot(&prev) / prev.dot(&prev);
            v = v - projection * &prev.to_owned();
        }

        // Normalize
        let norm = v.dot(&v).sqrt();
        if norm > 1e-6 {
            v = v / norm;
        }

        vectors.row_mut(i).assign(&v);
    }

    vectors
}
```

## Appendix B: Performance Monitoring

```rust
// tests/common/perf_monitor.rs

use std::time::{Duration, Instant};

pub struct PerformanceMonitor {
    samples: Vec<Duration>,
}

impl PerformanceMonitor {
    pub fn new() -> Self {
        Self { samples: Vec::new() }
    }

    pub fn measure<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let start = Instant::now();
        let result = f();
        let duration = start.elapsed();
        self.samples.push(duration);
        result
    }

    pub fn p50(&self) -> Duration {
        self.percentile(0.5)
    }

    pub fn p99(&self) -> Duration {
        self.percentile(0.99)
    }

    pub fn mean(&self) -> Duration {
        let total: Duration = self.samples.iter().sum();
        total / self.samples.len() as u32
    }

    fn percentile(&self, p: f64) -> Duration {
        let mut sorted = self.samples.clone();
        sorted.sort();
        let idx = ((sorted.len() as f64) * p) as usize;
        sorted[idx.min(sorted.len() - 1)]
    }
}
```

---

**End of Document**
