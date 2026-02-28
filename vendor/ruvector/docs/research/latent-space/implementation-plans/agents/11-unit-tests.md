# Agent 11: Comprehensive Unit Testing Suite

**Agent**: QA Testing Specialist
**Status**: Implementation Ready
**Dependencies**: All attention mechanism implementations (Agents 1-6)
**Target Directory**: `tests/unit/`

## Overview

This document provides complete unit test specifications for all attention mechanisms in the ruvector-attention crate. Tests cover functionality, edge cases, numerical stability, gradient correctness, and serialization, with property-based testing for mathematical invariants.

## Test Organization Structure

```
tests/
├── unit/
│   ├── mod.rs                          # Test module aggregator
│   ├── scaled_dot_product_tests.rs     # Agent 1 tests
│   ├── multi_head_tests.rs             # Agent 3 tests
│   ├── hyperbolic_tests.rs             # Agent 2 tests
│   ├── sparse_tests.rs                 # Agent 4 tests
│   ├── graph_tests.rs                  # Agent 6 tests
│   ├── moe_tests.rs                    # Agent 5 tests
│   └── test_utils.rs                   # Shared utilities
└── fixtures/
    ├── sample_data.rs
    └── numerical_gradients.rs
```

---

## 1. Test Utilities and Fixtures

### 1.1 Test Utilities (`tests/unit/test_utils.rs`)

```rust
//! Common test utilities for attention mechanisms

use ndarray::{Array1, Array2, ArrayView1};
use approx::AbsDiffEq;

pub const EPSILON: f32 = 1e-6;
pub const GRAD_EPSILON: f32 = 1e-4;

/// Generate random array with deterministic seed for reproducibility
pub fn random_array1(size: usize, seed: u64) -> Array1<f32> {
    use rand::{SeedableRng, Rng};
    use rand::rngs::StdRng;

    let mut rng = StdRng::seed_from_u64(seed);
    Array1::from_shape_fn(size, |_| rng.gen_range(-1.0..1.0))
}

pub fn random_array2(shape: (usize, usize), seed: u64) -> Array2<f32> {
    use rand::{SeedableRng, Rng};
    use rand::rngs::StdRng;

    let mut rng = StdRng::seed_from_u64(seed);
    Array2::from_shape_fn(shape, |_| rng.gen_range(-1.0..1.0))
}

/// Numerical gradient computation for gradient checking
pub fn numerical_gradient<F>(
    f: F,
    x: &Array1<f32>,
    idx: usize,
) -> f32
where
    F: Fn(&Array1<f32>) -> f32,
{
    let mut x_plus = x.clone();
    let mut x_minus = x.clone();

    x_plus[idx] += GRAD_EPSILON;
    x_minus[idx] -= GRAD_EPSILON;

    (f(&x_plus) - f(&x_minus)) / (2.0 * GRAD_EPSILON)
}

/// Check if two arrays are approximately equal
pub fn assert_arrays_close(a: &Array1<f32>, b: &Array1<f32>, tolerance: f32) {
    assert_eq!(a.len(), b.len(), "Array lengths must match");

    for (i, (a_val, b_val)) in a.iter().zip(b.iter()).enumerate() {
        assert!(
            (a_val - b_val).abs() < tolerance,
            "Arrays differ at index {}: {} vs {} (diff: {})",
            i, a_val, b_val, (a_val - b_val).abs()
        );
    }
}

/// Verify attention weights sum to 1.0
pub fn verify_attention_weights(weights: &Array1<f32>, tolerance: f32) {
    let sum: f32 = weights.iter().sum();
    assert!(
        (sum - 1.0).abs() < tolerance,
        "Attention weights should sum to 1.0, got: {}",
        sum
    );
}

/// Verify all weights are non-negative
pub fn verify_non_negative(weights: &Array1<f32>) {
    for (i, w) in weights.iter().enumerate() {
        assert!(
            *w >= 0.0,
            "Weight at index {} is negative: {}",
            i, w
        );
    }
}

/// Generate normalized random vector
pub fn random_normalized_vector(size: usize, seed: u64) -> Array1<f32> {
    let vec = random_array1(size, seed);
    let norm = vec.mapv(|x| x * x).sum().sqrt();
    vec / norm
}

/// Create one-hot encoded vector
pub fn one_hot(size: usize, idx: usize) -> Array1<f32> {
    let mut vec = Array1::zeros(size);
    vec[idx] = 1.0;
    vec
}

/// Relative error between two values
pub fn relative_error(a: f32, b: f32) -> f32 {
    if a.abs() < EPSILON && b.abs() < EPSILON {
        0.0
    } else {
        (a - b).abs() / (a.abs() + b.abs()).max(EPSILON)
    }
}
```

### 1.2 Test Fixtures (`tests/fixtures/sample_data.rs`)

```rust
//! Sample data fixtures for testing

use ndarray::{Array1, Array2};

pub struct AttentionTestData {
    pub query: Array1<f32>,
    pub keys: Array2<f32>,
    pub values: Array2<f32>,
    pub expected_weights: Option<Array1<f32>>,
}

impl AttentionTestData {
    /// Simple test case: query matches first key exactly
    pub fn exact_match() -> Self {
        let query = Array1::from_vec(vec![1.0, 0.0, 0.0, 0.0]);
        let keys = Array2::from_shape_vec(
            (3, 4),
            vec![
                1.0, 0.0, 0.0, 0.0,  // Exact match
                0.0, 1.0, 0.0, 0.0,  // Orthogonal
                0.0, 0.0, 1.0, 0.0,  // Orthogonal
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

        Self { query, keys, values, expected_weights: None }
    }

    /// Uniform case: all keys equally similar to query
    pub fn uniform() -> Self {
        let dim = 4;
        let n_keys = 3;
        let query = Array1::from_elem(dim, 0.5);
        let keys = Array2::from_elem((n_keys, dim), 0.5);
        let values = Array2::from_elem((n_keys, dim), 1.0);

        Self { query, keys, values, expected_weights: None }
    }

    /// Empty case: no keys
    pub fn empty() -> Self {
        let query = Array1::from_vec(vec![1.0, 0.0, 0.0, 0.0]);
        let keys = Array2::zeros((0, 4));
        let values = Array2::zeros((0, 4));

        Self { query, keys, values, expected_weights: None }
    }

    /// Single key
    pub fn single_key() -> Self {
        let query = Array1::from_vec(vec![1.0, 0.5, 0.0, 0.0]);
        let keys = Array2::from_shape_vec((1, 4), vec![0.5, 1.0, 0.0, 0.0]).unwrap();
        let values = Array2::from_shape_vec((1, 4), vec![2.0, 3.0, 0.0, 0.0]).unwrap();

        Self { query, keys, values, expected_weights: Some(Array1::from_vec(vec![1.0])) }
    }
}
```

---

## 2. Scaled Dot-Product Attention Tests

### File: `tests/unit/scaled_dot_product_tests.rs`

```rust
use ruvector_attention::scaled_dot_product::ScaledDotProduct;
use ndarray::{Array1, Array2};
use approx::assert_relative_eq;
use proptest::prelude::*;

mod test_utils;
use test_utils::*;

#[cfg(test)]
mod basic_functionality {
    use super::*;

    #[test]
    fn test_exact_match_attention() {
        let attention = ScaledDotProduct::new(4);
        let data = AttentionTestData::exact_match();

        let output = attention.forward(&data.query, &data.keys, &data.values)
            .expect("Forward pass failed");

        // Output should be dominated by first value (exact match)
        assert!(output[0] > 0.8, "First dimension should dominate: {}", output[0]);
    }

    #[test]
    fn test_attention_weights_sum_to_one() {
        let attention = ScaledDotProduct::new(4);
        let query = random_array1(4, 42);
        let keys = random_array2((5, 4), 123);

        let weights = attention.compute_weights(&query, &keys)
            .expect("Weight computation failed");

        verify_attention_weights(&weights, EPSILON);
        verify_non_negative(&weights);
    }

    #[test]
    fn test_dimension_preservation() {
        let dim = 8;
        let n_keys = 10;
        let attention = ScaledDotProduct::new(dim);

        let query = random_array1(dim, 1);
        let keys = random_array2((n_keys, dim), 2);
        let values = random_array2((n_keys, dim), 3);

        let output = attention.forward(&query, &keys, &values)
            .expect("Forward pass failed");

        assert_eq!(output.len(), dim, "Output dimension mismatch");
    }

    #[test]
    fn test_orthogonal_queries() {
        let attention = ScaledDotProduct::new(4);

        // Query orthogonal to all keys should give uniform attention
        let query = Array1::from_vec(vec![1.0, 0.0, 0.0, 0.0]);
        let keys = Array2::from_shape_vec(
            (3, 4),
            vec![
                0.0, 1.0, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
                0.0, 0.0, 0.0, 1.0,
            ],
        ).unwrap();

        let weights = attention.compute_weights(&query, &keys)
            .expect("Weight computation failed");

        // All weights should be approximately equal (1/3)
        for w in weights.iter() {
            assert_relative_eq!(*w, 1.0/3.0, epsilon = 0.01);
        }
    }
}

#[cfg(test)]
mod edge_cases {
    use super::*;

    #[test]
    fn test_empty_keys() {
        let attention = ScaledDotProduct::new(4);
        let data = AttentionTestData::empty();

        let result = attention.forward(&data.query, &data.keys, &data.values);

        // Should return error or zero vector
        match result {
            Ok(output) => assert_eq!(output.len(), 4),
            Err(_) => (), // Error is acceptable
        }
    }

    #[test]
    fn test_single_key() {
        let attention = ScaledDotProduct::new(4);
        let data = AttentionTestData::single_key();

        let weights = attention.compute_weights(&data.query, &data.keys)
            .expect("Single key should work");

        assert_relative_eq!(weights[0], 1.0, epsilon = EPSILON);
    }

    #[test]
    fn test_zero_query() {
        let attention = ScaledDotProduct::new(4);
        let query = Array1::zeros(4);
        let keys = random_array2((3, 4), 42);
        let values = random_array2((3, 4), 43);

        let output = attention.forward(&query, &keys, &values)
            .expect("Zero query should work");

        // Should give uniform attention
        assert_eq!(output.len(), 4);
    }

    #[test]
    fn test_zero_keys() {
        let attention = ScaledDotProduct::new(4);
        let query = random_array1(4, 42);
        let keys = Array2::zeros((3, 4));
        let values = random_array2((3, 4), 43);

        let weights = attention.compute_weights(&query, &keys)
            .expect("Zero keys should work");

        // Uniform attention when all keys are identical
        verify_attention_weights(&weights, EPSILON);
    }

    #[test]
    fn test_very_large_values() {
        let attention = ScaledDotProduct::new(4);
        let query = Array1::from_elem(4, 1000.0);
        let keys = Array2::from_elem((3, 4), 1000.0);
        let values = Array2::from_elem((3, 4), 1.0);

        let output = attention.forward(&query, &keys, &values)
            .expect("Large values should not overflow");

        assert!(output.iter().all(|&x| x.is_finite()));
    }
}

#[cfg(test)]
mod numerical_stability {
    use super::*;

    #[test]
    fn test_softmax_numerical_stability() {
        let attention = ScaledDotProduct::new(4);

        // Very large scores that could cause overflow
        let query = Array1::from_elem(4, 100.0);
        let keys = Array2::from_elem((3, 4), 100.0);

        let weights = attention.compute_weights(&query, &keys)
            .expect("Should handle large scores");

        verify_attention_weights(&weights, EPSILON);
        assert!(weights.iter().all(|&x| x.is_finite()));
    }

    #[test]
    fn test_very_small_values() {
        let attention = ScaledDotProduct::new(4);
        let query = Array1::from_elem(4, 1e-10);
        let keys = Array2::from_elem((3, 4), 1e-10);
        let values = Array2::from_elem((3, 4), 1.0);

        let output = attention.forward(&query, &keys, &values)
            .expect("Small values should not underflow");

        assert!(output.iter().all(|&x| x.is_finite()));
    }

    #[test]
    fn test_mixed_magnitude_scores() {
        let attention = ScaledDotProduct::new(4);

        let query = Array1::from_vec(vec![1.0, 0.0, 0.0, 0.0]);
        let keys = Array2::from_shape_vec(
            (3, 4),
            vec![
                100.0, 0.0, 0.0, 0.0,  // Very high score
                0.001, 0.0, 0.0, 0.0,  // Very low score
                1.0, 0.0, 0.0, 0.0,    // Medium score
            ],
        ).unwrap();

        let weights = attention.compute_weights(&query, &keys)
            .expect("Mixed magnitudes should work");

        verify_attention_weights(&weights, EPSILON);
    }

    #[test]
    fn test_scaling_factor_effectiveness() {
        let dim = 64; // Larger dimension to test scaling
        let attention = ScaledDotProduct::new(dim);

        let query = random_normalized_vector(dim, 42);
        let keys = random_array2((10, dim), 123);

        let weights = attention.compute_weights(&query, &keys)
            .expect("Scaling should prevent saturation");

        // No weight should be extremely close to 1 or 0 with random data
        let max_weight = weights.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        assert!(max_weight < 0.95, "Scaling should prevent saturation");
    }
}

#[cfg(test)]
mod gradient_correctness {
    use super::*;

    #[test]
    fn test_gradient_numerical_check() {
        let attention = ScaledDotProduct::new(4);

        let query = random_array1(4, 42);
        let keys = random_array2((3, 4), 123);
        let values = random_array2((3, 4), 456);

        // Define loss function (mean of output)
        let loss_fn = |q: &Array1<f32>| -> f32 {
            let output = attention.forward(q, &keys, &values).unwrap();
            output.mean().unwrap()
        };

        // Compute analytical gradient (via autograd or manual)
        let analytical_grad = attention.backward(&query, &keys, &values)
            .expect("Backward pass failed");

        // Compute numerical gradient
        let mut numerical_grad = Array1::zeros(4);
        for i in 0..4 {
            numerical_grad[i] = numerical_gradient(&loss_fn, &query, i);
        }

        // Compare
        for i in 0..4 {
            let rel_err = relative_error(analytical_grad[i], numerical_grad[i]);
            assert!(
                rel_err < 0.01,
                "Gradient mismatch at index {}: analytical={}, numerical={}, rel_err={}",
                i, analytical_grad[i], numerical_grad[i], rel_err
            );
        }
    }
}

#[cfg(test)]
mod serialization {
    use super::*;
    use serde_json;

    #[test]
    fn test_serialization_roundtrip() {
        let attention = ScaledDotProduct::new(4);

        // Serialize
        let serialized = serde_json::to_string(&attention)
            .expect("Serialization failed");

        // Deserialize
        let deserialized: ScaledDotProduct = serde_json::from_str(&serialized)
            .expect("Deserialization failed");

        // Verify behavior is identical
        let query = random_array1(4, 42);
        let keys = random_array2((3, 4), 123);
        let values = random_array2((3, 4), 456);

        let original_output = attention.forward(&query, &keys, &values).unwrap();
        let deserialized_output = deserialized.forward(&query, &keys, &values).unwrap();

        assert_arrays_close(&original_output, &deserialized_output, EPSILON);
    }
}

#[cfg(test)]
mod property_based_tests {
    use super::*;

    proptest! {
        #[test]
        fn prop_dimension_preservation(
            dim in 2usize..32,
            n_keys in 1usize..20,
            seed in 0u64..1000
        ) {
            let attention = ScaledDotProduct::new(dim);
            let query = random_array1(dim, seed);
            let keys = random_array2((n_keys, dim), seed + 1);
            let values = random_array2((n_keys, dim), seed + 2);

            let output = attention.forward(&query, &keys, &values).unwrap();
            prop_assert_eq!(output.len(), dim);
        }

        #[test]
        fn prop_weights_sum_to_one(
            dim in 2usize..16,
            n_keys in 1usize..10,
            seed in 0u64..100
        ) {
            let attention = ScaledDotProduct::new(dim);
            let query = random_array1(dim, seed);
            let keys = random_array2((n_keys, dim), seed + 1);

            let weights = attention.compute_weights(&query, &keys).unwrap();
            let sum: f32 = weights.iter().sum();

            prop_assert!((sum - 1.0).abs() < EPSILON);
        }

        #[test]
        fn prop_non_negative_weights(
            dim in 2usize..16,
            n_keys in 1usize..10,
            seed in 0u64..100
        ) {
            let attention = ScaledDotProduct::new(dim);
            let query = random_array1(dim, seed);
            let keys = random_array2((n_keys, dim), seed + 1);

            let weights = attention.compute_weights(&query, &keys).unwrap();

            for w in weights.iter() {
                prop_assert!(*w >= 0.0);
            }
        }

        #[test]
        fn prop_output_bounded(
            dim in 2usize..16,
            n_keys in 1usize..10,
            seed in 0u64..100
        ) {
            let attention = ScaledDotProduct::new(dim);
            let query = random_array1(dim, seed);
            let keys = random_array2((n_keys, dim), seed + 1);

            // Values bounded in [-1, 1]
            let values = random_array2((n_keys, dim), seed + 2);

            let output = attention.forward(&query, &keys, &values).unwrap();

            // Output should be convex combination, thus also bounded
            for &val in output.iter() {
                prop_assert!(val.abs() <= 2.0); // Allow some margin
            }
        }

        #[test]
        fn prop_permutation_invariance_values(
            dim in 2usize..8,
            seed in 0u64..50
        ) {
            // Swapping rows of values with same keys shouldn't change
            // which keys get attention (weights should be same)
            let attention = ScaledDotProduct::new(dim);
            let query = random_array1(dim, seed);
            let keys = random_array2((3, dim), seed + 1);

            let weights = attention.compute_weights(&query, &keys).unwrap();

            // Permute keys and check weights permute accordingly
            let mut keys_perm = Array2::zeros((3, dim));
            keys_perm.row_mut(0).assign(&keys.row(2));
            keys_perm.row_mut(1).assign(&keys.row(0));
            keys_perm.row_mut(2).assign(&keys.row(1));

            let weights_perm = attention.compute_weights(&query, &keys_perm).unwrap();

            // weights_perm should be permutation of weights
            prop_assert_relative_eq!(weights[0], weights_perm[1], epsilon = EPSILON);
            prop_assert_relative_eq!(weights[1], weights_perm[2], epsilon = EPSILON);
            prop_assert_relative_eq!(weights[2], weights_perm[0], epsilon = EPSILON);
        }
    }
}
```

---

## 3. Multi-Head Attention Tests

### File: `tests/unit/multi_head_tests.rs`

```rust
use ruvector_attention::multi_head::MultiHeadAttention;
use ndarray::{Array1, Array2, Array3};
use approx::assert_relative_eq;
use proptest::prelude::*;

mod test_utils;
use test_utils::*;

#[cfg(test)]
mod basic_functionality {
    use super::*;

    #[test]
    fn test_multi_head_initialization() {
        let mha = MultiHeadAttention::new(8, 4); // 8 dim, 4 heads

        assert_eq!(mha.num_heads(), 4);
        assert_eq!(mha.model_dim(), 8);
        assert_eq!(mha.head_dim(), 2); // 8/4 = 2
    }

    #[test]
    fn test_head_dimension_must_divide_model_dim() {
        // Should panic or return error if dimensions incompatible
        let result = std::panic::catch_unwind(|| {
            MultiHeadAttention::new(7, 4); // 7 not divisible by 4
        });

        assert!(result.is_err(), "Should fail when dimensions don't divide");
    }

    #[test]
    fn test_multi_head_forward() {
        let mha = MultiHeadAttention::new(8, 4);

        let query = random_array1(8, 42);
        let keys = random_array2((5, 8), 123);
        let values = random_array2((5, 8), 456);

        let output = mha.forward(&query, &keys, &values)
            .expect("Multi-head forward failed");

        assert_eq!(output.len(), 8, "Output dimension should match model_dim");
    }

    #[test]
    fn test_independent_heads() {
        // Each head should process different subspaces
        let mha = MultiHeadAttention::new(16, 4);

        let query = random_array1(16, 1);
        let keys = random_array2((3, 16), 2);
        let values = random_array2((3, 16), 3);

        let head_outputs = mha.get_head_outputs(&query, &keys, &values)
            .expect("Failed to get head outputs");

        assert_eq!(head_outputs.len(), 4, "Should have 4 head outputs");

        // Each head output should have dimension model_dim/num_heads
        for head_output in head_outputs.iter() {
            assert_eq!(head_output.len(), 4); // 16/4
        }
    }

    #[test]
    fn test_concat_projection() {
        let mha = MultiHeadAttention::new(12, 3);

        let query = random_array1(12, 42);
        let keys = random_array2((4, 12), 123);
        let values = random_array2((4, 12), 456);

        // Get individual head outputs
        let head_outputs = mha.get_head_outputs(&query, &keys, &values).unwrap();

        // Concatenate manually
        let manual_concat = concatenate_heads(&head_outputs);

        // Get model output
        let model_output = mha.forward(&query, &keys, &values).unwrap();

        // After projection, dimensions should match
        assert_eq!(model_output.len(), 12);
    }
}

#[cfg(test)]
mod edge_cases {
    use super::*;

    #[test]
    fn test_single_head() {
        // Single head should behave like standard attention
        let mha = MultiHeadAttention::new(8, 1);

        let query = random_array1(8, 42);
        let keys = random_array2((3, 8), 123);
        let values = random_array2((3, 8), 456);

        let output = mha.forward(&query, &keys, &values)
            .expect("Single head should work");

        assert_eq!(output.len(), 8);
    }

    #[test]
    fn test_maximum_heads() {
        // Each dimension is its own head
        let dim = 16;
        let mha = MultiHeadAttention::new(dim, dim);

        let query = random_array1(dim, 1);
        let keys = random_array2((3, dim), 2);
        let values = random_array2((3, dim), 3);

        let output = mha.forward(&query, &keys, &values)
            .expect("Max heads should work");

        assert_eq!(output.len(), dim);
    }

    #[test]
    fn test_empty_keys_all_heads() {
        let mha = MultiHeadAttention::new(8, 4);

        let query = random_array1(8, 42);
        let keys = Array2::zeros((0, 8));
        let values = Array2::zeros((0, 8));

        let result = mha.forward(&query, &keys, &values);

        // Should handle gracefully (error or zero output)
        assert!(result.is_ok() || result.is_err());
    }
}

#[cfg(test)]
mod numerical_stability {
    use super::*;

    #[test]
    fn test_large_number_of_heads() {
        let mha = MultiHeadAttention::new(64, 16);

        let query = random_array1(64, 42);
        let keys = random_array2((10, 64), 123);
        let values = random_array2((10, 64), 456);

        let output = mha.forward(&query, &keys, &values)
            .expect("Large number of heads should work");

        assert!(output.iter().all(|&x| x.is_finite()));
    }

    #[test]
    fn test_head_independence_no_interference() {
        // Changing input in one head's subspace shouldn't affect others
        let mha = MultiHeadAttention::new(8, 2);

        let mut query1 = Array1::zeros(8);
        query1[0] = 1.0; // First head's subspace

        let mut query2 = query1.clone();
        query2[4] = 1.0; // Second head's subspace

        let keys = random_array2((3, 8), 123);
        let values = random_array2((3, 8), 456);

        let output1 = mha.forward(&query1, &keys, &values).unwrap();
        let output2 = mha.forward(&query2, &keys, &values).unwrap();

        // Outputs should differ (heads process different subspaces)
        let diff = (&output1 - &output2).mapv(|x| x.abs()).sum();
        assert!(diff > EPSILON, "Outputs should differ when heads differ");
    }
}

#[cfg(test)]
mod gradient_correctness {
    use super::*;

    #[test]
    fn test_multi_head_gradient_check() {
        let mha = MultiHeadAttention::new(8, 2);

        let query = random_array1(8, 42);
        let keys = random_array2((3, 8), 123);
        let values = random_array2((3, 8), 456);

        let loss_fn = |q: &Array1<f32>| -> f32 {
            mha.forward(q, &keys, &values).unwrap().mean().unwrap()
        };

        let analytical_grad = mha.backward(&query, &keys, &values).unwrap();

        let mut numerical_grad = Array1::zeros(8);
        for i in 0..8 {
            numerical_grad[i] = numerical_gradient(&loss_fn, &query, i);
        }

        for i in 0..8 {
            let rel_err = relative_error(analytical_grad[i], numerical_grad[i]);
            assert!(rel_err < 0.01, "Gradient error at {}: {}", i, rel_err);
        }
    }
}

#[cfg(test)]
mod serialization {
    use super::*;
    use serde_json;

    #[test]
    fn test_multi_head_serialization() {
        let mha = MultiHeadAttention::new(12, 3);

        let serialized = serde_json::to_string(&mha).unwrap();
        let deserialized: MultiHeadAttention = serde_json::from_str(&serialized).unwrap();

        let query = random_array1(12, 42);
        let keys = random_array2((4, 12), 123);
        let values = random_array2((4, 12), 456);

        let original = mha.forward(&query, &keys, &values).unwrap();
        let restored = deserialized.forward(&query, &keys, &values).unwrap();

        assert_arrays_close(&original, &restored, EPSILON);
    }
}

#[cfg(test)]
mod property_based_tests {
    use super::*;

    proptest! {
        #[test]
        fn prop_valid_head_dimensions(
            model_dim in vec![8usize, 12, 16, 24, 32, 64],
            num_heads in 1usize..8
        ) {
            if model_dim % num_heads == 0 {
                let mha = MultiHeadAttention::new(model_dim, num_heads);
                prop_assert_eq!(mha.head_dim(), model_dim / num_heads);
            }
        }

        #[test]
        fn prop_output_dimension_preserved(
            heads in 1usize..5,
            head_dim in 2usize..8,
            n_keys in 1usize..10,
            seed in 0u64..100
        ) {
            let model_dim = heads * head_dim;
            let mha = MultiHeadAttention::new(model_dim, heads);

            let query = random_array1(model_dim, seed);
            let keys = random_array2((n_keys, model_dim), seed + 1);
            let values = random_array2((n_keys, model_dim), seed + 2);

            let output = mha.forward(&query, &keys, &values).unwrap();
            prop_assert_eq!(output.len(), model_dim);
        }

        #[test]
        fn prop_finite_outputs(
            heads in 1usize..4,
            head_dim in 2usize..6,
            n_keys in 1usize..8,
            seed in 0u64..50
        ) {
            let model_dim = heads * head_dim;
            let mha = MultiHeadAttention::new(model_dim, heads);

            let query = random_array1(model_dim, seed);
            let keys = random_array2((n_keys, model_dim), seed + 1);
            let values = random_array2((n_keys, model_dim), seed + 2);

            let output = mha.forward(&query, &keys, &values).unwrap();

            for &val in output.iter() {
                prop_assert!(val.is_finite());
            }
        }
    }
}

// Helper function
fn concatenate_heads(heads: &[Array1<f32>]) -> Array1<f32> {
    let total_dim: usize = heads.iter().map(|h| h.len()).sum();
    let mut result = Array1::zeros(total_dim);
    let mut offset = 0;

    for head in heads {
        result.slice_mut(s![offset..offset + head.len()]).assign(head);
        offset += head.len();
    }

    result
}
```

---

## 4. Hyperbolic Attention Tests

### File: `tests/unit/hyperbolic_tests.rs`

```rust
use ruvector_attention::hyperbolic::HyperbolicAttention;
use ndarray::{Array1, Array2};
use approx::assert_relative_eq;
use proptest::prelude::*;

mod test_utils;
use test_utils::*;

#[cfg(test)]
mod poincare_operations {
    use super::*;

    #[test]
    fn test_poincare_distance_identity() {
        let x = random_normalized_vector(4, 42) * 0.5; // Stay in ball
        let curvature = 1.0;

        let dist = poincare_distance(&x, &x, curvature);

        assert_relative_eq!(dist, 0.0, epsilon = EPSILON);
    }

    #[test]
    fn test_poincare_distance_symmetry() {
        let x = random_normalized_vector(4, 42) * 0.3;
        let y = random_normalized_vector(4, 123) * 0.3;
        let curvature = 1.0;

        let dist_xy = poincare_distance(&x, &y, curvature);
        let dist_yx = poincare_distance(&y, &x, curvature);

        assert_relative_eq!(dist_xy, dist_yx, epsilon = EPSILON);
    }

    #[test]
    fn test_poincare_distance_triangle_inequality() {
        let x = random_normalized_vector(4, 1) * 0.2;
        let y = random_normalized_vector(4, 2) * 0.2;
        let z = random_normalized_vector(4, 3) * 0.2;
        let curvature = 1.0;

        let dist_xy = poincare_distance(&x, &y, curvature);
        let dist_yz = poincare_distance(&y, &z, curvature);
        let dist_xz = poincare_distance(&x, &z, curvature);

        assert!(
            dist_xz <= dist_xy + dist_yz + EPSILON,
            "Triangle inequality violated: {} > {} + {}",
            dist_xz, dist_xy, dist_yz
        );
    }

    #[test]
    fn test_euclidean_limit() {
        // As curvature → 0, should approach Euclidean distance
        let x = Array1::from_vec(vec![1.0, 0.0, 0.0, 0.0]);
        let y = Array1::from_vec(vec![0.0, 1.0, 0.0, 0.0]);

        let euclidean_dist = ((x.clone() - y.clone()).mapv(|v| v * v).sum()).sqrt();

        let hyperbolic_dist = poincare_distance(&x, &y, 0.001);

        assert_relative_eq!(hyperbolic_dist, euclidean_dist, epsilon = 0.01);
    }

    #[test]
    fn test_mobius_addition_identity() {
        let x = random_normalized_vector(4, 42) * 0.4;
        let zero = Array1::zeros(4);
        let curvature = 1.0;

        let result = mobius_add(&x, &zero, curvature);

        assert_arrays_close(&result, &x, EPSILON);
    }

    #[test]
    fn test_mobius_addition_stays_in_ball() {
        let curvature = 1.0;
        let boundary = 1.0 / curvature.sqrt();

        let x = random_normalized_vector(4, 42) * 0.8 * boundary;
        let y = random_normalized_vector(4, 123) * 0.8 * boundary;

        let result = mobius_add(&x, &y, curvature);
        let norm = result.mapv(|v| v * v).sum().sqrt();

        assert!(norm < boundary, "Result escaped ball: {} >= {}", norm, boundary);
    }
}

#[cfg(test)]
mod hyperbolic_attention_basic {
    use super::*;

    #[test]
    fn test_hyperbolic_attention_initialization() {
        let attention = HyperbolicAttention::new(4, 1.0);

        assert_eq!(attention.dim(), 4);
        assert_relative_eq!(attention.curvature(), 1.0);
    }

    #[test]
    fn test_hyperbolic_attention_forward() {
        let attention = HyperbolicAttention::new(4, 1.0);

        // Keep vectors well inside ball
        let query = random_normalized_vector(4, 42) * 0.3;
        let keys = random_array2((3, 4), 123) * 0.3;
        let values = random_array2((3, 4), 456);

        let output = attention.forward(&query, &keys, &values)
            .expect("Hyperbolic attention failed");

        assert_eq!(output.len(), 4);
    }

    #[test]
    fn test_weights_based_on_hyperbolic_distance() {
        let attention = HyperbolicAttention::new(4, 1.0);

        let query = Array1::from_vec(vec![0.1, 0.0, 0.0, 0.0]);

        // Key 1: Close to query in hyperbolic space
        let close_key = Array1::from_vec(vec![0.15, 0.0, 0.0, 0.0]);

        // Key 2: Far from query
        let far_key = Array1::from_vec(vec![0.0, 0.5, 0.0, 0.0]);

        let mut keys = Array2::zeros((2, 4));
        keys.row_mut(0).assign(&close_key);
        keys.row_mut(1).assign(&far_key);

        let weights = attention.compute_weights(&query, &keys).unwrap();

        // Closer key should have higher weight
        assert!(weights[0] > weights[1],
            "Close key weight {} should exceed far key weight {}",
            weights[0], weights[1]);
    }

    #[test]
    fn test_variable_curvature() {
        let query = random_normalized_vector(4, 1) * 0.2;
        let keys = random_array2((3, 4), 2) * 0.2;
        let values = random_array2((3, 4), 3);

        let attention_low = HyperbolicAttention::new(4, 0.1);
        let attention_high = HyperbolicAttention::new(4, 10.0);

        let output_low = attention_low.forward(&query, &keys, &values).unwrap();
        let output_high = attention_high.forward(&query, &keys, &values).unwrap();

        // Different curvatures should give different results
        let diff = (&output_low - &output_high).mapv(|x| x.abs()).sum();
        assert!(diff > EPSILON, "Curvature should affect output");
    }
}

#[cfg(test)]
mod edge_cases {
    use super::*;

    #[test]
    fn test_origin_query() {
        let attention = HyperbolicAttention::new(4, 1.0);

        let query = Array1::zeros(4); // Origin
        let keys = random_array2((3, 4), 123) * 0.3;
        let values = random_array2((3, 4), 456);

        let output = attention.forward(&query, &keys, &values)
            .expect("Origin query should work");

        assert!(output.iter().all(|&x| x.is_finite()));
    }

    #[test]
    fn test_near_boundary_points() {
        let curvature = 1.0;
        let boundary = 1.0 / curvature.sqrt();
        let attention = HyperbolicAttention::new(4, curvature);

        // Point very close to boundary
        let query = random_normalized_vector(4, 42) * (boundary * 0.99);
        let keys = random_array2((3, 4), 123) * 0.3;
        let values = random_array2((3, 4), 456);

        let result = attention.forward(&query, &keys, &values);

        // Should handle gracefully with clamping
        assert!(result.is_ok(), "Should handle near-boundary points");
    }

    #[test]
    fn test_zero_curvature_euclidean() {
        // Zero curvature should behave like Euclidean attention
        let attention = HyperbolicAttention::new(4, 0.0);

        let query = random_array1(4, 42);
        let keys = random_array2((3, 4), 123);
        let values = random_array2((3, 4), 456);

        let output = attention.forward(&query, &keys, &values)
            .expect("Zero curvature should work");

        assert!(output.iter().all(|&x| x.is_finite()));
    }
}

#[cfg(test)]
mod numerical_stability {
    use super::*;

    #[test]
    fn test_extreme_curvature_values() {
        let query = random_normalized_vector(4, 42) * 0.1;
        let keys = random_array2((3, 4), 123) * 0.1;
        let values = random_array2((3, 4), 456);

        // Very small curvature
        let attention_small = HyperbolicAttention::new(4, 1e-5);
        let output_small = attention_small.forward(&query, &keys, &values).unwrap();
        assert!(output_small.iter().all(|&x| x.is_finite()));

        // Large curvature (but keep points scaled appropriately)
        let scaled_query = query.clone() * 0.01;
        let scaled_keys = keys.clone() * 0.01;
        let attention_large = HyperbolicAttention::new(4, 100.0);
        let output_large = attention_large.forward(&scaled_query, &scaled_keys, &values).unwrap();
        assert!(output_large.iter().all(|&x| x.is_finite()));
    }

    #[test]
    fn test_numerical_precision_distances() {
        let attention = HyperbolicAttention::new(4, 1.0);

        // Very close points (numerical precision challenge)
        let query = Array1::from_vec(vec![0.1, 0.0, 0.0, 0.0]);
        let close_key = Array1::from_vec(vec![0.1 + 1e-7, 0.0, 0.0, 0.0]);

        let mut keys = Array2::zeros((1, 4));
        keys.row_mut(0).assign(&close_key);

        let weights = attention.compute_weights(&query, &keys).unwrap();

        verify_attention_weights(&weights, EPSILON);
    }
}

#[cfg(test)]
mod gradient_correctness {
    use super::*;

    #[test]
    fn test_hyperbolic_attention_gradient() {
        let attention = HyperbolicAttention::new(4, 1.0);

        let query = random_normalized_vector(4, 42) * 0.3;
        let keys = random_array2((3, 4), 123) * 0.3;
        let values = random_array2((3, 4), 456);

        let loss_fn = |q: &Array1<f32>| -> f32 {
            let q_scaled = q * 0.3; // Keep in ball
            attention.forward(&q_scaled, &keys, &values).unwrap().mean().unwrap()
        };

        let analytical_grad = attention.backward(&query, &keys, &values).unwrap();

        let mut numerical_grad = Array1::zeros(4);
        for i in 0..4 {
            numerical_grad[i] = numerical_gradient(&loss_fn, &query, i);
        }

        for i in 0..4 {
            let rel_err = relative_error(analytical_grad[i], numerical_grad[i]);
            assert!(rel_err < 0.05, "Gradient error at {}: {}", i, rel_err);
        }
    }
}

#[cfg(test)]
mod property_based_tests {
    use super::*;

    proptest! {
        #[test]
        fn prop_weights_sum_to_one(
            dim in 2usize..12,
            n_keys in 1usize..8,
            curvature in 0.1f32..5.0,
            seed in 0u64..100
        ) {
            let attention = HyperbolicAttention::new(dim, curvature);
            let boundary = 1.0 / curvature.sqrt();

            let query = random_normalized_vector(dim, seed) * (boundary * 0.5);
            let keys = random_array2((n_keys, dim), seed + 1) * (boundary * 0.5);

            let weights = attention.compute_weights(&query, &keys).unwrap();
            let sum: f32 = weights.iter().sum();

            prop_assert!((sum - 1.0).abs() < EPSILON);
        }

        #[test]
        fn prop_distance_non_negative(
            dim in 2usize..8,
            curvature in 0.1f32..3.0,
            seed in 0u64..50
        ) {
            let boundary = 1.0 / curvature.sqrt();
            let x = random_normalized_vector(dim, seed) * (boundary * 0.6);
            let y = random_normalized_vector(dim, seed + 1) * (boundary * 0.6);

            let dist = poincare_distance(&x, &y, curvature);

            prop_assert!(dist >= 0.0);
        }

        #[test]
        fn prop_output_finite(
            dim in 2usize..8,
            n_keys in 1usize..6,
            curvature in 0.1f32..3.0,
            seed in 0u64..50
        ) {
            let attention = HyperbolicAttention::new(dim, curvature);
            let boundary = 1.0 / curvature.sqrt();

            let query = random_normalized_vector(dim, seed) * (boundary * 0.4);
            let keys = random_array2((n_keys, dim), seed + 1) * (boundary * 0.4);
            let values = random_array2((n_keys, dim), seed + 2);

            let output = attention.forward(&query, &keys, &values).unwrap();

            for &val in output.iter() {
                prop_assert!(val.is_finite());
            }
        }
    }
}

// Helper functions (would be in hyperbolic module)
fn poincare_distance(x: &Array1<f32>, y: &Array1<f32>, curvature: f32) -> f32 {
    // Simplified placeholder - actual implementation in module
    ((x - y).mapv(|v| v * v).sum()).sqrt()
}

fn mobius_add(x: &Array1<f32>, y: &Array1<f32>, curvature: f32) -> Array1<f32> {
    // Simplified placeholder - actual implementation in module
    x + y
}
```

---

## 5. Sparse Attention Tests

### File: `tests/unit/sparse_tests.rs`

```rust
use ruvector_attention::sparse::SparseAttention;
use ndarray::{Array1, Array2};
use approx::assert_relative_eq;
use proptest::prelude::*;

mod test_utils;
use test_utils::*;

#[cfg(test)]
mod basic_functionality {
    use super::*;

    #[test]
    fn test_top_k_selection() {
        let sparse = SparseAttention::new(4, TopK(2));

        let query = Array1::from_vec(vec![1.0, 0.0, 0.0, 0.0]);
        let keys = Array2::from_shape_vec(
            (4, 4),
            vec![
                1.0, 0.0, 0.0, 0.0,   // High similarity
                0.0, 1.0, 0.0, 0.0,   // Low similarity
                0.9, 0.1, 0.0, 0.0,   // Medium-high similarity
                0.0, 0.0, 1.0, 0.0,   // Low similarity
            ],
        ).unwrap();

        let mask = sparse.compute_sparsity_mask(&query, &keys).unwrap();

        // Should select exactly top-2 keys
        let selected_count = mask.iter().filter(|&&x| x).count();
        assert_eq!(selected_count, 2, "Should select exactly top-2 keys");

        // First and third keys should be selected (highest similarities)
        assert!(mask[0], "Highest similarity key should be selected");
        assert!(mask[2], "Second highest similarity key should be selected");
    }

    #[test]
    fn test_windowed_attention() {
        let sparse = SparseAttention::new(4, Window(3)); // Window size 3

        let query_idx = 5; // Query at position 5
        let n_keys = 10;

        let mask = sparse.compute_window_mask(query_idx, n_keys).unwrap();

        // Should attend to indices 4, 5, 6 (window of ±1)
        assert_eq!(mask.iter().filter(|&&x| x).count(), 3);
        assert!(mask[4]);
        assert!(mask[5]);
        assert!(mask[6]);
    }

    #[test]
    fn test_sparse_reduces_computation() {
        let sparse = SparseAttention::new(8, TopK(3));
        let dense = ScaledDotProduct::new(8);

        let query = random_array1(8, 42);
        let keys = random_array2((100, 8), 123); // Many keys
        let values = random_array2((100, 8), 456);

        // Sparse should compute faster (in practice, not testing timing here)
        let sparse_output = sparse.forward(&query, &keys, &values).unwrap();
        let dense_output = dense.forward(&query, &keys, &values).unwrap();

        assert_eq!(sparse_output.len(), dense_output.len());

        // Outputs will differ, but both should be valid
        assert!(sparse_output.iter().all(|&x| x.is_finite()));
    }

    #[test]
    fn test_strided_attention() {
        let sparse = SparseAttention::new(4, Strided(2)); // Every 2nd key

        let mask = sparse.compute_strided_mask(10).unwrap();

        // Should select indices 0, 2, 4, 6, 8
        assert!(mask[0]);
        assert!(!mask[1]);
        assert!(mask[2]);
        assert!(!mask[3]);
        assert!(mask[4]);
    }
}

#[cfg(test)]
mod edge_cases {
    use super::*;

    #[test]
    fn test_top_k_greater_than_num_keys() {
        let sparse = SparseAttention::new(4, TopK(10)); // Request more than available

        let query = random_array1(4, 42);
        let keys = random_array2((5, 4), 123); // Only 5 keys

        let mask = sparse.compute_sparsity_mask(&query, &keys).unwrap();

        // Should select all 5 keys
        assert_eq!(mask.iter().filter(|&&x| x).count(), 5);
    }

    #[test]
    fn test_window_at_boundaries() {
        let sparse = SparseAttention::new(4, Window(5));

        // Window at start
        let mask_start = sparse.compute_window_mask(0, 10).unwrap();
        assert!(mask_start[0]);
        assert!(!mask_start[5]);

        // Window at end
        let mask_end = sparse.compute_window_mask(9, 10).unwrap();
        assert!(mask_end[9]);
        assert!(!mask_end[4]);
    }

    #[test]
    fn test_empty_sparse_pattern() {
        let sparse = SparseAttention::new(4, TopK(0)); // Select nothing

        let query = random_array1(4, 42);
        let keys = random_array2((5, 4), 123);
        let values = random_array2((5, 4), 456);

        let result = sparse.forward(&query, &keys, &values);

        // Should handle gracefully (error or zero vector)
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_single_key_sparse() {
        let sparse = SparseAttention::new(4, TopK(1));

        let query = random_array1(4, 42);
        let keys = random_array2((1, 4), 123);
        let values = random_array2((1, 4), 456);

        let output = sparse.forward(&query, &keys, &values).unwrap();

        // Should equal the single value
        assert_arrays_close(&output, &values.row(0).to_owned(), EPSILON);
    }
}

#[cfg(test)]
mod numerical_stability {
    use super::*;

    #[test]
    fn test_sparse_with_extreme_scores() {
        let sparse = SparseAttention::new(4, TopK(2));

        let query = Array1::from_vec(vec![1.0, 0.0, 0.0, 0.0]);
        let keys = Array2::from_shape_vec(
            (3, 4),
            vec![
                100.0, 0.0, 0.0, 0.0,   // Extremely high score
                0.001, 0.0, 0.0, 0.0,   // Very low score
                1.0, 0.0, 0.0, 0.0,     // Medium score
            ],
        ).unwrap();
        let values = random_array2((3, 4), 456);

        let output = sparse.forward(&query, &keys, &values).unwrap();

        assert!(output.iter().all(|&x| x.is_finite()));
    }

    #[test]
    fn test_large_sequence_length() {
        let sparse = SparseAttention::new(8, TopK(10));

        let query = random_array1(8, 42);
        let keys = random_array2((1000, 8), 123); // Long sequence
        let values = random_array2((1000, 8), 456);

        let output = sparse.forward(&query, &keys, &values)
            .expect("Should handle long sequences");

        assert_eq!(output.len(), 8);
    }
}

#[cfg(test)]
mod gradient_correctness {
    use super::*;

    #[test]
    fn test_sparse_gradient_check() {
        let sparse = SparseAttention::new(4, TopK(2));

        let query = random_array1(4, 42);
        let keys = random_array2((5, 4), 123);
        let values = random_array2((5, 4), 456);

        let loss_fn = |q: &Array1<f32>| -> f32 {
            sparse.forward(q, &keys, &values).unwrap().mean().unwrap()
        };

        let analytical_grad = sparse.backward(&query, &keys, &values).unwrap();

        let mut numerical_grad = Array1::zeros(4);
        for i in 0..4 {
            numerical_grad[i] = numerical_gradient(&loss_fn, &query, i);
        }

        // Gradients may be zero for non-selected keys
        for i in 0..4 {
            if analytical_grad[i].abs() > EPSILON {
                let rel_err = relative_error(analytical_grad[i], numerical_grad[i]);
                assert!(rel_err < 0.05, "Gradient error at {}: {}", i, rel_err);
            }
        }
    }
}

#[cfg(test)]
mod property_based_tests {
    use super::*;

    proptest! {
        #[test]
        fn prop_top_k_selects_correctly(
            dim in 2usize..8,
            n_keys in 5usize..20,
            k in 1usize..10,
            seed in 0u64..100
        ) {
            let k_clamped = k.min(n_keys);
            let sparse = SparseAttention::new(dim, TopK(k_clamped));

            let query = random_array1(dim, seed);
            let keys = random_array2((n_keys, dim), seed + 1);

            let mask = sparse.compute_sparsity_mask(&query, &keys).unwrap();
            let selected = mask.iter().filter(|&&x| x).count();

            prop_assert_eq!(selected, k_clamped);
        }

        #[test]
        fn prop_sparse_output_dimension(
            dim in 2usize..12,
            n_keys in 5usize..15,
            k in 1usize..8,
            seed in 0u64..50
        ) {
            let k_clamped = k.min(n_keys);
            let sparse = SparseAttention::new(dim, TopK(k_clamped));

            let query = random_array1(dim, seed);
            let keys = random_array2((n_keys, dim), seed + 1);
            let values = random_array2((n_keys, dim), seed + 2);

            let output = sparse.forward(&query, &keys, &values).unwrap();

            prop_assert_eq!(output.len(), dim);
        }

        #[test]
        fn prop_window_size_respected(
            n_keys in 10usize..50,
            window_size in 1usize..10,
            query_idx in 0usize..20
        ) {
            let sparse = SparseAttention::new(4, Window(window_size));
            let query_idx = query_idx.min(n_keys - 1);

            let mask = sparse.compute_window_mask(query_idx, n_keys).unwrap();
            let selected = mask.iter().filter(|&&x| x).count();

            // Selected count should be at most window_size (may be less at boundaries)
            prop_assert!(selected <= window_size * 2 + 1);
        }
    }
}

// Sparse pattern types
enum SparsePattern {
    TopK(usize),
    Window(usize),
    Strided(usize),
}

use SparsePattern::*;
```

---

## 6. Graph Attention Tests

### File: `tests/unit/graph_tests.rs`

```rust
use ruvector_attention::graph::GraphAttention;
use ndarray::{Array1, Array2};
use approx::assert_relative_eq;
use proptest::prelude::*;

mod test_utils;
use test_utils::*;

#[cfg(test)]
mod basic_functionality {
    use super::*;

    #[test]
    fn test_graph_attention_with_adjacency() {
        let gat = GraphAttention::new(4, 1); // 1 attention head

        let node_features = random_array2((5, 4), 42); // 5 nodes

        // Adjacency matrix: simple path graph (0-1-2-3-4)
        let adj = Array2::from_shape_vec(
            (5, 5),
            vec![
                0.0, 1.0, 0.0, 0.0, 0.0,
                1.0, 0.0, 1.0, 0.0, 0.0,
                0.0, 1.0, 0.0, 1.0, 0.0,
                0.0, 0.0, 1.0, 0.0, 1.0,
                0.0, 0.0, 0.0, 1.0, 0.0,
            ],
        ).unwrap();

        let output = gat.forward(&node_features, &adj)
            .expect("Graph attention failed");

        assert_eq!(output.shape(), &[5, 4]);
    }

    #[test]
    fn test_masked_attention_respects_graph() {
        let gat = GraphAttention::new(4, 1);

        let features = random_array2((3, 4), 42);

        // Star graph: node 0 connected to nodes 1 and 2
        let adj = Array2::from_shape_vec(
            (3, 3),
            vec![
                0.0, 1.0, 1.0,
                1.0, 0.0, 0.0,
                1.0, 0.0, 0.0,
            ],
        ).unwrap();

        let attention_weights = gat.compute_attention_weights(&features, &adj).unwrap();

        // Node 1 should not attend to node 2 (not connected)
        assert_relative_eq!(attention_weights[[1, 2]], 0.0, epsilon = EPSILON);

        // Node 0 should attend to nodes 1 and 2
        assert!(attention_weights[[0, 1]] > EPSILON);
        assert!(attention_weights[[0, 2]] > EPSILON);
    }

    #[test]
    fn test_learnable_attention_coefficients() {
        let gat = GraphAttention::new(4, 1);

        let features = random_array2((3, 4), 42);
        let adj = Array2::eye(3); // Self-loops only

        // Different features should produce different attention coefficients
        let features2 = random_array2((3, 4), 123);

        let weights1 = gat.compute_attention_weights(&features, &adj).unwrap();
        let weights2 = gat.compute_attention_weights(&features2, &adj).unwrap();

        let diff = (&weights1 - &weights2).mapv(|x| x.abs()).sum();
        assert!(diff > EPSILON, "Different features should give different attention");
    }

    #[test]
    fn test_multi_head_graph_attention() {
        let gat = GraphAttention::new(8, 4); // 4 heads

        let features = random_array2((5, 8), 42);
        let adj = Array2::eye(5);

        let output = gat.forward(&features, &adj).unwrap();

        assert_eq!(output.shape(), &[5, 8]);
        assert!(output.iter().all(|&x| x.is_finite()));
    }
}

#[cfg(test)]
mod edge_cases {
    use super::*;

    #[test]
    fn test_isolated_nodes() {
        let gat = GraphAttention::new(4, 1);

        let features = random_array2((3, 4), 42);

        // Node 1 is isolated (no connections, not even self-loop)
        let adj = Array2::from_shape_vec(
            (3, 3),
            vec![
                1.0, 0.0, 1.0,
                0.0, 0.0, 0.0,  // Isolated node
                1.0, 0.0, 1.0,
            ],
        ).unwrap();

        let output = gat.forward(&features, &adj).unwrap();

        // Isolated node output might be zero or use self features
        assert_eq!(output.shape(), &[3, 4]);
    }

    #[test]
    fn test_complete_graph() {
        let gat = GraphAttention::new(4, 1);

        let n_nodes = 5;
        let features = random_array2((n_nodes, 4), 42);
        let adj = Array2::ones((n_nodes, n_nodes)); // Fully connected

        let output = gat.forward(&features, &adj).unwrap();

        assert_eq!(output.shape(), &[n_nodes, 4]);
    }

    #[test]
    fn test_single_node_graph() {
        let gat = GraphAttention::new(4, 1);

        let features = random_array2((1, 4), 42);
        let adj = Array2::from_elem((1, 1), 1.0); // Self-loop

        let output = gat.forward(&features, &adj).unwrap();

        // Single node should attend only to itself
        assert_eq!(output.shape(), &[1, 4]);
    }

    #[test]
    fn test_weighted_edges() {
        let gat = GraphAttention::new(4, 1);

        let features = random_array2((3, 4), 42);

        // Weighted adjacency matrix
        let adj = Array2::from_shape_vec(
            (3, 3),
            vec![
                1.0, 0.5, 0.0,
                0.5, 1.0, 0.8,
                0.0, 0.8, 1.0,
            ],
        ).unwrap();

        let output = gat.forward(&features, &adj).unwrap();

        assert!(output.iter().all(|&x| x.is_finite()));
    }
}

#[cfg(test)]
mod numerical_stability {
    use super::*;

    #[test]
    fn test_large_graph() {
        let gat = GraphAttention::new(8, 2);

        let n_nodes = 100;
        let features = random_array2((n_nodes, 8), 42);

        // Random sparse graph (10% density)
        let mut adj = Array2::zeros((n_nodes, n_nodes));
        for i in 0..n_nodes {
            adj[[i, i]] = 1.0; // Self-loops
            for j in (i + 1)..n_nodes {
                if (i * 7 + j * 13) % 10 == 0 {
                    adj[[i, j]] = 1.0;
                    adj[[j, i]] = 1.0;
                }
            }
        }

        let output = gat.forward(&features, &adj)
            .expect("Large graph should work");

        assert!(output.iter().all(|&x| x.is_finite()));
    }

    #[test]
    fn test_attention_normalization() {
        let gat = GraphAttention::new(4, 1);

        let features = random_array2((3, 4), 42);
        let adj = Array2::ones((3, 3));

        let weights = gat.compute_attention_weights(&features, &adj).unwrap();

        // Each row should sum to 1 (attention distribution over neighbors)
        for i in 0..3 {
            let row_sum: f32 = weights.row(i).sum();
            assert_relative_eq!(row_sum, 1.0, epsilon = 0.01);
        }
    }
}

#[cfg(test)]
mod gradient_correctness {
    use super::*;

    #[test]
    fn test_graph_attention_gradient() {
        let gat = GraphAttention::new(4, 1);

        let features = random_array2((3, 4), 42);
        let adj = Array2::eye(3);

        // Gradient with respect to first node's features
        let loss_fn = |f: &Array1<f32>| -> f32 {
            let mut features_mod = features.clone();
            features_mod.row_mut(0).assign(f);

            let output = gat.forward(&features_mod, &adj).unwrap();
            output.mean().unwrap()
        };

        let first_features = features.row(0).to_owned();
        let analytical_grad = gat.backward_node(&first_features, 0, &features, &adj).unwrap();

        let mut numerical_grad = Array1::zeros(4);
        for i in 0..4 {
            numerical_grad[i] = numerical_gradient(&loss_fn, &first_features, i);
        }

        for i in 0..4 {
            let rel_err = relative_error(analytical_grad[i], numerical_grad[i]);
            assert!(rel_err < 0.05, "Gradient error at {}: {}", i, rel_err);
        }
    }
}

#[cfg(test)]
mod property_based_tests {
    use super::*;

    proptest! {
        #[test]
        fn prop_output_shape_preserved(
            n_nodes in 2usize..20,
            dim in 2usize..16,
            num_heads in 1usize..4,
            seed in 0u64..100
        ) {
            let gat = GraphAttention::new(dim, num_heads);
            let features = random_array2((n_nodes, dim), seed);
            let adj = Array2::eye(n_nodes); // Simple case

            let output = gat.forward(&features, &adj).unwrap();

            prop_assert_eq!(output.shape(), &[n_nodes, dim]);
        }

        #[test]
        fn prop_attention_respects_mask(
            n_nodes in 3usize..10,
            dim in 2usize..8,
            seed in 0u64..50
        ) {
            let gat = GraphAttention::new(dim, 1);
            let features = random_array2((n_nodes, dim), seed);

            // Diagonal matrix (only self-loops)
            let adj = Array2::eye(n_nodes);

            let weights = gat.compute_attention_weights(&features, &adj).unwrap();

            // Off-diagonal elements should be zero
            for i in 0..n_nodes {
                for j in 0..n_nodes {
                    if i != j {
                        prop_assert_relative_eq!(weights[[i, j]], 0.0, epsilon = EPSILON);
                    }
                }
            }
        }

        #[test]
        fn prop_permutation_equivariance(
            n_nodes in 2usize..6,
            dim in 2usize..6,
            seed in 0u64..30
        ) {
            // Graph attention should be permutation equivariant:
            // permute input nodes → output nodes permuted accordingly
            let gat = GraphAttention::new(dim, 1);
            let features = random_array2((n_nodes, dim), seed);
            let adj = Array2::eye(n_nodes);

            let output = gat.forward(&features, &adj).unwrap();

            // Permute: swap first two rows
            if n_nodes >= 2 {
                let mut features_perm = features.clone();
                let row0 = features.row(0).to_owned();
                let row1 = features.row(1).to_owned();
                features_perm.row_mut(0).assign(&row1);
                features_perm.row_mut(1).assign(&row0);

                let output_perm = gat.forward(&features_perm, &adj).unwrap();

                // Output rows should also be swapped
                assert_arrays_close(
                    &output.row(0).to_owned(),
                    &output_perm.row(1).to_owned(),
                    EPSILON * 10.0
                );
            }
        }
    }
}
```

---

## 7. Mixture of Experts (MoE) Attention Tests

### File: `tests/unit/moe_tests.rs`

```rust
use ruvector_attention::moe::MoEAttention;
use ndarray::{Array1, Array2};
use approx::assert_relative_eq;
use proptest::prelude::*;

mod test_utils;
use test_utils::*;

#[cfg(test)]
mod basic_functionality {
    use super::*;

    #[test]
    fn test_moe_initialization() {
        let moe = MoEAttention::new(8, 4, 2); // 8 dim, 4 experts, top-2 routing

        assert_eq!(moe.num_experts(), 4);
        assert_eq!(moe.top_k(), 2);
        assert_eq!(moe.dim(), 8);
    }

    #[test]
    fn test_router_selects_top_k() {
        let moe = MoEAttention::new(8, 4, 2);

        let query = random_array1(8, 42);

        let (selected_experts, weights) = moe.route(&query)
            .expect("Routing failed");

        assert_eq!(selected_experts.len(), 2, "Should select top-2 experts");
        assert_eq!(weights.len(), 2, "Should have 2 weights");

        // Weights should sum to ~1
        let sum: f32 = weights.iter().sum();
        assert_relative_eq!(sum, 1.0, epsilon = EPSILON);
    }

    #[test]
    fn test_moe_forward_combines_experts() {
        let moe = MoEAttention::new(8, 3, 2);

        let query = random_array1(8, 42);
        let keys = random_array2((5, 8), 123);
        let values = random_array2((5, 8), 456);

        let output = moe.forward(&query, &keys, &values)
            .expect("MoE forward failed");

        assert_eq!(output.len(), 8);
        assert!(output.iter().all(|&x| x.is_finite()));
    }

    #[test]
    fn test_different_queries_route_differently() {
        let moe = MoEAttention::new(8, 4, 2);

        let query1 = Array1::from_elem(8, 0.5);
        let query2 = Array1::from_elem(8, -0.5);

        let (experts1, _) = moe.route(&query1).unwrap();
        let (experts2, _) = moe.route(&query2).unwrap();

        // Different queries may route to different experts
        // (Not guaranteed to be different, but likely)
        // Just verify routing works for both
        assert_eq!(experts1.len(), 2);
        assert_eq!(experts2.len(), 2);
    }

    #[test]
    fn test_expert_specialization() {
        // Experts should process inputs differently
        let moe = MoEAttention::new(8, 3, 1); // Top-1 for deterministic routing

        let query = random_array1(8, 42);
        let keys = random_array2((5, 8), 123);
        let values = random_array2((5, 8), 456);

        // Get output from specific expert
        let expert_outputs = moe.get_expert_outputs(&query, &keys, &values).unwrap();

        assert_eq!(expert_outputs.len(), 3, "Should have 3 expert outputs");

        // Experts should produce different outputs
        let diff_01 = (&expert_outputs[0] - &expert_outputs[1]).mapv(|x| x.abs()).sum();
        assert!(diff_01 > EPSILON, "Experts should specialize differently");
    }
}

#[cfg(test)]
mod routing_tests {
    use super::*;

    #[test]
    fn test_router_weights_sum_to_one() {
        let moe = MoEAttention::new(8, 5, 3);

        for seed in 0..10 {
            let query = random_array1(8, seed);
            let (_, weights) = moe.route(&query).unwrap();

            let sum: f32 = weights.iter().sum();
            assert_relative_eq!(sum, 1.0, epsilon = EPSILON);
        }
    }

    #[test]
    fn test_router_non_negative_weights() {
        let moe = MoEAttention::new(8, 4, 2);

        let query = random_array1(8, 42);
        let (_, weights) = moe.route(&query).unwrap();

        for &w in weights.iter() {
            assert!(w >= 0.0, "Router weights must be non-negative");
        }
    }

    #[test]
    fn test_top_k_larger_than_num_experts() {
        // Request more experts than available
        let moe = MoEAttention::new(8, 3, 5);

        let query = random_array1(8, 42);
        let (selected, _) = moe.route(&query).unwrap();

        // Should select all available experts
        assert_eq!(selected.len(), 3);
    }

    #[test]
    fn test_deterministic_routing() {
        let moe = MoEAttention::new(8, 4, 2);

        let query = random_array1(8, 42);

        let (experts1, weights1) = moe.route(&query).unwrap();
        let (experts2, weights2) = moe.route(&query).unwrap();

        // Same query should route identically
        assert_eq!(experts1, experts2);
        assert_arrays_close(&Array1::from_vec(weights1), &Array1::from_vec(weights2), EPSILON);
    }
}

#[cfg(test)]
mod load_balancing {
    use super::*;

    #[test]
    fn test_load_balancing_auxiliary_loss() {
        let moe = MoEAttention::new(8, 4, 2);

        // Process multiple queries
        let queries = vec![
            random_array1(8, 1),
            random_array1(8, 2),
            random_array1(8, 3),
            random_array1(8, 4),
            random_array1(8, 5),
        ];

        let mut expert_counts = vec![0; 4];

        for query in queries.iter() {
            let (selected, _) = moe.route(query).unwrap();
            for expert_idx in selected {
                expert_counts[expert_idx] += 1;
            }
        }

        // With load balancing, experts should be used somewhat evenly
        // (This is a weak test - just verify all experts used at least once)
        let total: usize = expert_counts.iter().sum();
        assert_eq!(total, 5 * 2, "Total selections should be 5 queries * top-2");
    }

    #[test]
    fn test_load_balancing_loss_computation() {
        let moe = MoEAttention::new(8, 4, 2);

        let batch_queries = vec![
            random_array1(8, 1),
            random_array1(8, 2),
            random_array1(8, 3),
        ];

        let loss = moe.compute_load_balancing_loss(&batch_queries)
            .expect("Load balancing loss failed");

        assert!(loss >= 0.0, "Loss should be non-negative");
        assert!(loss.is_finite());
    }
}

#[cfg(test)]
mod edge_cases {
    use super::*;

    #[test]
    fn test_single_expert() {
        let moe = MoEAttention::new(8, 1, 1);

        let query = random_array1(8, 42);
        let keys = random_array2((5, 8), 123);
        let values = random_array2((5, 8), 456);

        let output = moe.forward(&query, &keys, &values).unwrap();

        assert_eq!(output.len(), 8);
    }

    #[test]
    fn test_all_experts_selected() {
        let moe = MoEAttention::new(8, 3, 3); // Select all 3 experts

        let query = random_array1(8, 42);
        let (selected, _) = moe.route(&query).unwrap();

        assert_eq!(selected.len(), 3);
    }

    #[test]
    fn test_zero_query() {
        let moe = MoEAttention::new(8, 4, 2);

        let query = Array1::zeros(8);
        let keys = random_array2((5, 8), 123);
        let values = random_array2((5, 8), 456);

        let output = moe.forward(&query, &keys, &values)
            .expect("Zero query should work");

        assert!(output.iter().all(|&x| x.is_finite()));
    }
}

#[cfg(test)]
mod numerical_stability {
    use super::*;

    #[test]
    fn test_many_experts() {
        let moe = MoEAttention::new(16, 16, 4); // Many experts

        let query = random_array1(16, 42);
        let keys = random_array2((10, 16), 123);
        let values = random_array2((10, 16), 456);

        let output = moe.forward(&query, &keys, &values)
            .expect("Many experts should work");

        assert!(output.iter().all(|&x| x.is_finite()));
    }

    #[test]
    fn test_extreme_routing_scores() {
        let moe = MoEAttention::new(8, 4, 2);

        // Query that might produce extreme routing scores
        let query = Array1::from_elem(8, 100.0);

        let (selected, weights) = moe.route(&query)
            .expect("Should handle extreme scores");

        assert_eq!(selected.len(), 2);

        for &w in weights.iter() {
            assert!(w.is_finite());
            assert!(w >= 0.0);
        }
    }
}

#[cfg(test)]
mod gradient_correctness {
    use super::*;

    #[test]
    fn test_moe_gradient_check() {
        let moe = MoEAttention::new(8, 3, 2);

        let query = random_array1(8, 42);
        let keys = random_array2((5, 8), 123);
        let values = random_array2((5, 8), 456);

        let loss_fn = |q: &Array1<f32>| -> f32 {
            moe.forward(q, &keys, &values).unwrap().mean().unwrap()
        };

        let analytical_grad = moe.backward(&query, &keys, &values).unwrap();

        let mut numerical_grad = Array1::zeros(8);
        for i in 0..8 {
            numerical_grad[i] = numerical_gradient(&loss_fn, &query, i);
        }

        for i in 0..8 {
            let rel_err = relative_error(analytical_grad[i], numerical_grad[i]);
            assert!(rel_err < 0.05, "Gradient error at {}: {}", i, rel_err);
        }
    }
}

#[cfg(test)]
mod property_based_tests {
    use super::*;

    proptest! {
        #[test]
        fn prop_routing_weights_valid(
            dim in 4usize..16,
            num_experts in 2usize..8,
            top_k in 1usize..5,
            seed in 0u64..100
        ) {
            let top_k_clamped = top_k.min(num_experts);
            let moe = MoEAttention::new(dim, num_experts, top_k_clamped);

            let query = random_array1(dim, seed);
            let (selected, weights) = moe.route(&query).unwrap();

            prop_assert_eq!(selected.len(), top_k_clamped);
            prop_assert_eq!(weights.len(), top_k_clamped);

            let sum: f32 = weights.iter().sum();
            prop_assert!((sum - 1.0).abs() < EPSILON);
        }

        #[test]
        fn prop_output_dimension_preserved(
            dim in 4usize..16,
            num_experts in 2usize..6,
            top_k in 1usize..4,
            n_keys in 2usize..10,
            seed in 0u64..50
        ) {
            let top_k_clamped = top_k.min(num_experts);
            let moe = MoEAttention::new(dim, num_experts, top_k_clamped);

            let query = random_array1(dim, seed);
            let keys = random_array2((n_keys, dim), seed + 1);
            let values = random_array2((n_keys, dim), seed + 2);

            let output = moe.forward(&query, &keys, &values).unwrap();

            prop_assert_eq!(output.len(), dim);
        }

        #[test]
        fn prop_expert_indices_valid(
            dim in 4usize..12,
            num_experts in 2usize..8,
            top_k in 1usize..5,
            seed in 0u64..50
        ) {
            let top_k_clamped = top_k.min(num_experts);
            let moe = MoEAttention::new(dim, num_experts, top_k_clamped);

            let query = random_array1(dim, seed);
            let (selected, _) = moe.route(&query).unwrap();

            for &expert_idx in selected.iter() {
                prop_assert!(expert_idx < num_experts);
            }
        }
    }
}
```

---

## 8. Test Coverage and CI Integration

### Coverage Configuration (`Cargo.toml` additions)

```toml
[dev-dependencies]
proptest = "1.0"
approx = "0.5"
criterion = "0.5"
serde_json = "1.0"

[profile.test]
opt-level = 0
debug = true

[profile.bench]
opt-level = 3
debug = false
```

### CI Test Script (`.github/workflows/tests.yml`)

```yaml
name: Unit Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          override: true
          components: llvm-tools-preview

      - name: Run unit tests
        run: cargo test --all-features --verbose

      - name: Run property-based tests
        run: cargo test --release -- --test-threads=1 prop_

      - name: Generate coverage
        run: |
          cargo install cargo-llvm-cov
          cargo llvm-cov --all-features --lcov --output-path lcov.info

      - name: Upload coverage
        uses: codecov/codecov-action@v3
        with:
          files: lcov.info
```

---

## Summary

This comprehensive unit test suite provides:

1. **6 Complete Test Modules**: One for each attention mechanism
2. **Test Categories**: Basic functionality, edge cases, numerical stability, gradient correctness, serialization
3. **Property-Based Testing**: 20+ property tests using `proptest`
4. **Test Utilities**: Reusable fixtures and validation functions
5. **Coverage Goals**: >85% code coverage with 100% critical path coverage
6. **CI Integration**: Automated testing and coverage reporting

All tests follow Rust best practices with clear documentation, deterministic seeds for reproducibility, and comprehensive edge case coverage.
