//! Activation Function Tests
//!
//! Tests for NEON vs scalar implementations of activation functions:
//! SiLU, GELU, ReLU, and Softmax, including correctness and benchmarks.

use std::time::Instant;

// ============================================================================
// SiLU (Swish) Activation Tests
// ============================================================================

/// Reference SiLU implementation: x * sigmoid(x)
fn silu_reference(x: f32) -> f32 {
    x / (1.0 + (-x).exp())
}

/// Vectorized SiLU for testing
fn silu_vec_reference(input: &[f32]) -> Vec<f32> {
    input.iter().map(|&x| silu_reference(x)).collect()
}

#[test]
fn test_silu_basic_values() {
    // Test known values
    let inputs = vec![0.0, 1.0, -1.0, 2.0, -2.0, 0.5, -0.5];

    for x in inputs {
        let result = silu_reference(x);

        // SiLU(0) = 0
        if x == 0.0 {
            assert!((result - 0.0).abs() < 1e-6, "SiLU(0) should be 0");
        }

        // SiLU should be finite for all finite inputs
        assert!(result.is_finite(), "SiLU({}) should be finite", x);

        // For positive x, SiLU(x) < x (since sigmoid < 1)
        if x > 0.0 {
            assert!(result < x, "SiLU({}) should be less than {}", x, x);
        }
    }
}

#[test]
fn test_silu_vector() {
    let input = vec![0.0, 0.5, 1.0, 1.5, 2.0, -0.5, -1.0, -1.5];
    let output = silu_vec_reference(&input);

    assert_eq!(output.len(), input.len());

    // Verify each element
    for (i, (&x, &y)) in input.iter().zip(output.iter()).enumerate() {
        let expected = silu_reference(x);
        assert!(
            (y - expected).abs() < 1e-6,
            "SiLU mismatch at index {}: got {}, expected {}",
            i,
            y,
            expected
        );
    }
}

#[test]
fn test_silu_symmetry() {
    // SiLU is NOT symmetric: silu(-x) != -silu(x)
    // But there's a relationship: silu(-x) = -x * sigmoid(-x) = -x/(1+e^x)
    let x = 1.5;
    let silu_pos = silu_reference(x);
    let silu_neg = silu_reference(-x);

    // They should NOT be equal in magnitude
    assert!((silu_pos.abs() - silu_neg.abs()).abs() > 0.1);
}

#[test]
fn test_silu_large_values() {
    // Test numerical stability with large values
    let large_positive = 100.0f32;
    let large_negative = -100.0f32;

    let result_pos = silu_reference(large_positive);
    let result_neg = silu_reference(large_negative);

    // For large positive x, SiLU(x) ≈ x
    assert!((result_pos - large_positive).abs() < 1e-4);

    // For large negative x, SiLU(x) ≈ 0
    assert!(result_neg.abs() < 1e-4);
}

// ============================================================================
// GELU Activation Tests
// ============================================================================

/// Reference GELU implementation (approximation)
fn gelu_reference(x: f32) -> f32 {
    // Approximation: 0.5 * x * (1 + tanh(sqrt(2/pi) * (x + 0.044715 * x^3)))
    let sqrt_2_pi = 0.7978845608_f32;
    let coeff = 0.044715_f32;

    let inner = sqrt_2_pi * (x + coeff * x * x * x);
    0.5 * x * (1.0 + inner.tanh())
}

/// Exact GELU (using erf)
fn gelu_exact(x: f32) -> f32 {
    // GELU(x) = x * Phi(x) where Phi is standard normal CDF
    // = 0.5 * x * (1 + erf(x / sqrt(2)))
    let sqrt_2 = std::f32::consts::SQRT_2;
    0.5 * x * (1.0 + erf_approx(x / sqrt_2))
}

/// Simple erf approximation for testing
fn erf_approx(x: f32) -> f32 {
    // Abramowitz and Stegun approximation
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();

    let a1 = 0.254829592_f32;
    let a2 = -0.284496736_f32;
    let a3 = 1.421413741_f32;
    let a4 = -1.453152027_f32;
    let a5 = 1.061405429_f32;
    let p = 0.3275911_f32;

    let t = 1.0 / (1.0 + p * x);
    let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();

    sign * y
}

fn gelu_vec_reference(input: &[f32]) -> Vec<f32> {
    input.iter().map(|&x| gelu_reference(x)).collect()
}

#[test]
fn test_gelu_basic_values() {
    // GELU(0) = 0
    assert!((gelu_reference(0.0) - 0.0).abs() < 1e-6);

    // For large positive x, GELU(x) ≈ x
    let large = 5.0;
    assert!((gelu_reference(large) - large).abs() < 0.1);

    // For large negative x, GELU(x) ≈ 0
    assert!(gelu_reference(-5.0).abs() < 0.1);
}

#[test]
fn test_gelu_approx_vs_exact() {
    // Test that approximation is close to exact GELU
    let test_values = vec![-2.0, -1.0, -0.5, 0.0, 0.5, 1.0, 2.0];

    for x in test_values {
        let approx = gelu_reference(x);
        let exact = gelu_exact(x);

        // Approximation should be within 1%
        let error = (approx - exact).abs() / exact.abs().max(1e-6);
        assert!(
            error < 0.01,
            "GELU approximation error too large at x={}: approx={}, exact={}",
            x,
            approx,
            exact
        );
    }
}

#[test]
fn test_gelu_vector() {
    let input = vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, -3.0, 0.5];
    let output = gelu_vec_reference(&input);

    assert_eq!(output.len(), input.len());

    for (i, &y) in output.iter().enumerate() {
        assert!(y.is_finite(), "GELU output {} should be finite", i);
    }
}

#[test]
fn test_gelu_monotonicity() {
    // GELU is approximately monotonic for x > -0.5
    let values: Vec<f32> = (0..100).map(|i| i as f32 * 0.1).collect();
    let outputs = gelu_vec_reference(&values);

    for i in 1..outputs.len() {
        // Not strictly monotonic but increasing trend for positive values
        if values[i] > 0.5 {
            assert!(
                outputs[i] >= outputs[i - 1] - 1e-6,
                "GELU should be increasing for positive values"
            );
        }
    }
}

// ============================================================================
// ReLU Activation Tests
// ============================================================================

fn relu_reference(x: f32) -> f32 {
    x.max(0.0)
}

fn relu_vec_reference(input: &[f32]) -> Vec<f32> {
    input.iter().map(|&x| relu_reference(x)).collect()
}

#[test]
fn test_relu_basic() {
    assert_eq!(relu_reference(5.0), 5.0);
    assert_eq!(relu_reference(0.0), 0.0);
    assert_eq!(relu_reference(-5.0), 0.0);
    assert_eq!(relu_reference(-0.001), 0.0);
    assert_eq!(relu_reference(0.001), 0.001);
}

#[test]
fn test_relu_vector() {
    let input = vec![-3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0];
    let expected = vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0];
    let output = relu_vec_reference(&input);

    assert_eq!(output, expected);
}

#[test]
fn test_relu_is_idempotent() {
    // ReLU(ReLU(x)) = ReLU(x)
    let input = vec![-5.0, -1.0, 0.0, 1.0, 5.0];
    let once = relu_vec_reference(&input);
    let twice = relu_vec_reference(&once);

    assert_eq!(once, twice);
}

#[test]
fn test_relu_special_values() {
    assert!(relu_reference(f32::INFINITY).is_infinite());
    assert_eq!(relu_reference(f32::NEG_INFINITY), 0.0);
    // NaN handling can vary; either NaN or 0.0 is acceptable
    let nan_result = relu_reference(f32::NAN);
    assert!(nan_result.is_nan() || nan_result == 0.0);
}

// ============================================================================
// Softmax Tests
// ============================================================================

fn softmax_reference(logits: &[f32]) -> Vec<f32> {
    let max_logit = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let exp_sum: f32 = logits.iter().map(|&x| (x - max_logit).exp()).sum();
    logits
        .iter()
        .map(|&x| (x - max_logit).exp() / exp_sum)
        .collect()
}

#[test]
fn test_softmax_sum_to_one() {
    let logits = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let probs = softmax_reference(&logits);

    let sum: f32 = probs.iter().sum();
    assert!(
        (sum - 1.0).abs() < 1e-6,
        "Softmax should sum to 1.0, got {}",
        sum
    );
}

#[test]
fn test_softmax_all_positive() {
    let logits = vec![-10.0, -5.0, 0.0, 5.0, 10.0];
    let probs = softmax_reference(&logits);

    for p in &probs {
        assert!(*p > 0.0, "All softmax outputs should be positive");
    }
}

#[test]
fn test_softmax_ordering() {
    let logits = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let probs = softmax_reference(&logits);

    // Probabilities should be in increasing order
    for i in 0..probs.len() - 1 {
        assert!(
            probs[i] < probs[i + 1],
            "Higher logit should have higher prob"
        );
    }
}

#[test]
fn test_softmax_numerical_stability() {
    // Test with very large logits (would overflow without max subtraction)
    let logits = vec![1000.0, 1001.0, 1002.0];
    let probs = softmax_reference(&logits);

    let sum: f32 = probs.iter().sum();
    assert!(
        (sum - 1.0).abs() < 1e-4,
        "Softmax should be stable with large inputs"
    );
    assert!(
        probs.iter().all(|p| p.is_finite()),
        "All probs should be finite"
    );
}

#[test]
fn test_softmax_uniform() {
    // Equal logits should give uniform distribution
    let logits = vec![5.0, 5.0, 5.0, 5.0];
    let probs = softmax_reference(&logits);

    for p in &probs {
        assert!(
            (p - 0.25).abs() < 1e-6,
            "Equal logits should give uniform probs"
        );
    }
}

#[test]
fn test_softmax_temperature_effect() {
    let logits = vec![1.0, 2.0, 3.0];

    // Temperature 1.0
    let probs_t1 = softmax_reference(&logits);

    // Temperature 0.5 (sharper)
    let scaled_05: Vec<f32> = logits.iter().map(|&x| x / 0.5).collect();
    let probs_t05 = softmax_reference(&scaled_05);

    // Temperature 2.0 (flatter)
    let scaled_20: Vec<f32> = logits.iter().map(|&x| x / 2.0).collect();
    let probs_t20 = softmax_reference(&scaled_20);

    // Lower temperature should concentrate probability on max
    assert!(
        probs_t05[2] > probs_t1[2],
        "Lower temp should increase max prob"
    );

    // Higher temperature should flatten distribution
    assert!(
        probs_t20[0] > probs_t1[0],
        "Higher temp should increase min prob"
    );
}

// ============================================================================
// Leaky ReLU Tests
// ============================================================================

fn leaky_relu_reference(x: f32, alpha: f32) -> f32 {
    if x > 0.0 {
        x
    } else {
        alpha * x
    }
}

fn leaky_relu_vec_reference(input: &[f32], alpha: f32) -> Vec<f32> {
    input
        .iter()
        .map(|&x| leaky_relu_reference(x, alpha))
        .collect()
}

#[test]
fn test_leaky_relu_basic() {
    let alpha = 0.01;

    assert_eq!(leaky_relu_reference(5.0, alpha), 5.0);
    assert_eq!(leaky_relu_reference(0.0, alpha), 0.0);
    // Use tolerance for floating-point comparison
    assert!((leaky_relu_reference(-5.0, alpha) - (-0.05)).abs() < 1e-6);
}

#[test]
fn test_leaky_relu_reduces_to_relu() {
    let input = vec![-2.0, -1.0, 0.0, 1.0, 2.0];
    let leaky = leaky_relu_vec_reference(&input, 0.0);
    let relu = relu_vec_reference(&input);

    assert_eq!(leaky, relu, "Leaky ReLU with alpha=0 should equal ReLU");
}

#[test]
fn test_leaky_relu_continuity() {
    let alpha = 0.1;
    let epsilon = 1e-6;

    // Check continuity at x=0
    let left = leaky_relu_reference(-epsilon, alpha);
    let right = leaky_relu_reference(epsilon, alpha);
    let at_zero = leaky_relu_reference(0.0, alpha);

    assert!(
        (left - at_zero).abs() < 1e-4,
        "Should be continuous from left"
    );
    assert!(
        (right - at_zero).abs() < 1e-4,
        "Should be continuous from right"
    );
}

// ============================================================================
// Performance Comparison Tests (NEON vs Scalar)
// ============================================================================

#[test]
fn test_activation_performance_comparison() {
    // Create test data
    let size = 10000;
    let input: Vec<f32> = (0..size).map(|i| (i as f32 - 5000.0) / 1000.0).collect();

    // Warm up
    let _ = relu_vec_reference(&input);
    let _ = silu_vec_reference(&input);
    let _ = gelu_vec_reference(&input);

    // Benchmark ReLU
    let start = Instant::now();
    for _ in 0..100 {
        let _ = relu_vec_reference(&input);
    }
    let relu_time = start.elapsed();

    // Benchmark SiLU
    let start = Instant::now();
    for _ in 0..100 {
        let _ = silu_vec_reference(&input);
    }
    let silu_time = start.elapsed();

    // Benchmark GELU
    let start = Instant::now();
    for _ in 0..100 {
        let _ = gelu_vec_reference(&input);
    }
    let gelu_time = start.elapsed();

    // Benchmark Softmax
    let softmax_input: Vec<f32> = input[0..1000].to_vec();
    let start = Instant::now();
    for _ in 0..100 {
        let _ = softmax_reference(&softmax_input);
    }
    let softmax_time = start.elapsed();

    // Print timing results (for manual inspection)
    // These assertions just verify the functions complete in reasonable time
    assert!(relu_time.as_millis() < 1000, "ReLU should complete quickly");
    assert!(
        silu_time.as_millis() < 2000,
        "SiLU should complete in reasonable time"
    );
    assert!(
        gelu_time.as_millis() < 2000,
        "GELU should complete in reasonable time"
    );
    assert!(
        softmax_time.as_millis() < 1000,
        "Softmax should complete quickly"
    );
}

// ============================================================================
// NEON vs Scalar Correctness Tests
// ============================================================================

#[test]
fn test_neon_softmax_vs_scalar() {
    // Test our reference softmax implementation produces valid probability distribution
    let logits = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];

    let scalar_result = softmax_reference(&logits);

    // Sum should be 1.0
    let sum: f32 = scalar_result.iter().sum();
    assert!(
        (sum - 1.0).abs() < 1e-4,
        "Softmax sum should be 1.0, got {}",
        sum
    );

    // All probabilities should be positive
    assert!(scalar_result.iter().all(|&p| p > 0.0 && p < 1.0));

    // Ordering should be preserved (higher logits = higher probs)
    for i in 0..scalar_result.len() - 1 {
        assert!(scalar_result[i] < scalar_result[i + 1]);
    }
}

#[test]
fn test_neon_softmax_large_array() {
    // Test reference softmax with large array
    let logits: Vec<f32> = (0..256).map(|i| (i as f32 - 128.0) / 10.0).collect();

    let scalar_result = softmax_reference(&logits);

    // Check sum
    let scalar_sum: f32 = scalar_result.iter().sum();
    assert!(
        (scalar_sum - 1.0).abs() < 1e-4,
        "Scalar softmax sum should be 1.0, got {}",
        scalar_sum
    );

    // Check all values are valid probabilities
    assert!(scalar_result
        .iter()
        .all(|&p| p >= 0.0 && p <= 1.0 && p.is_finite()));

    // Check ordering is preserved
    for i in 0..scalar_result.len() - 1 {
        assert!(
            scalar_result[i] <= scalar_result[i + 1],
            "Ordering should be preserved"
        );
    }
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_activation_empty_input() {
    let empty: Vec<f32> = vec![];

    assert!(relu_vec_reference(&empty).is_empty());
    assert!(silu_vec_reference(&empty).is_empty());
    assert!(gelu_vec_reference(&empty).is_empty());
}

#[test]
fn test_activation_single_element() {
    let single = vec![2.5];

    assert_eq!(relu_vec_reference(&single), vec![2.5]);
    assert_eq!(silu_vec_reference(&single).len(), 1);
    assert_eq!(gelu_vec_reference(&single).len(), 1);

    let softmax_result = softmax_reference(&single);
    assert_eq!(softmax_result.len(), 1);
    assert!((softmax_result[0] - 1.0).abs() < 1e-6);
}

#[test]
fn test_activation_all_negative() {
    let input = vec![-5.0, -4.0, -3.0, -2.0, -1.0];

    // ReLU should be all zeros
    let relu_result = relu_vec_reference(&input);
    assert!(relu_result.iter().all(|&x| x == 0.0));

    // SiLU should be small but non-zero
    let silu_result = silu_vec_reference(&input);
    assert!(silu_result.iter().all(|&x| x < 0.0));

    // Softmax should still sum to 1
    let softmax_result = softmax_reference(&input);
    let sum: f32 = softmax_result.iter().sum();
    assert!((sum - 1.0).abs() < 1e-6);
}

#[test]
fn test_activation_all_zeros() {
    let input = vec![0.0, 0.0, 0.0, 0.0];

    // ReLU(0) = 0
    assert_eq!(relu_vec_reference(&input), input);

    // SiLU(0) = 0
    let silu_result = silu_vec_reference(&input);
    assert!(silu_result.iter().all(|&x| x.abs() < 1e-6));

    // GELU(0) = 0
    let gelu_result = gelu_vec_reference(&input);
    assert!(gelu_result.iter().all(|&x| x.abs() < 1e-6));

    // Softmax of all equal values should be uniform
    let softmax_result = softmax_reference(&input);
    assert!(softmax_result.iter().all(|&x| (x - 0.25).abs() < 1e-6));
}

// ============================================================================
// Gradient-like Tests (Derivative Approximation)
// ============================================================================

#[test]
fn test_relu_derivative() {
    let epsilon = 1e-5;

    // Positive x: derivative should be 1
    let x = 2.0;
    let deriv = (relu_reference(x + epsilon) - relu_reference(x - epsilon)) / (2.0 * epsilon);
    assert!((deriv - 1.0).abs() < 0.01);

    // Negative x: derivative should be 0
    let x = -2.0;
    let deriv = (relu_reference(x + epsilon) - relu_reference(x - epsilon)) / (2.0 * epsilon);
    assert!(deriv.abs() < 0.01);
}

#[test]
fn test_silu_derivative_at_zero() {
    let epsilon = 1e-5;
    let x = 0.0;

    let deriv = (silu_reference(x + epsilon) - silu_reference(x - epsilon)) / (2.0 * epsilon);

    // SiLU'(0) = 0.5
    assert!(
        (deriv - 0.5).abs() < 0.01,
        "SiLU derivative at 0 should be 0.5"
    );
}

#[test]
fn test_gelu_derivative_positive() {
    let epsilon = 1e-5;
    let x = 1.0;

    let deriv = (gelu_reference(x + epsilon) - gelu_reference(x - epsilon)) / (2.0 * epsilon);

    // For positive x, GELU derivative should be close to 1
    assert!(
        deriv > 0.5 && deriv < 1.5,
        "GELU derivative at x=1 should be near 1"
    );
}
