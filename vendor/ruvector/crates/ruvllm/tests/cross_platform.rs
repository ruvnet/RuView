#![allow(
    clippy::all,
    unused_imports,
    unused_variables,
    dead_code,
    unused_mut,
    unused_assignments,
    non_camel_case_types,
    clippy::approx_constant,
    unexpected_cfgs,
    unused_must_use,
    unused_parens
)]
//! Cross-platform tests for scalar fallback implementations
//!
//! These tests verify that the scalar fallback implementations produce
//! correct results and work on all platforms (including non-NEON and WASM).

use ruvllm::kernels::{flash_attention_neon, gemm_neon, gemv_neon, layer_norm_neon, rms_norm_neon};

// ========== Scalar Reference Implementations ==========

/// Scalar reference GEMV implementation
fn gemv_scalar(a: &[f32], x: &[f32], y: &mut [f32], m: usize, n: usize) {
    for row in 0..m {
        let mut sum = 0.0f32;
        for col in 0..n {
            sum += a[row * n + col] * x[col];
        }
        y[row] = sum;
    }
}

/// Scalar reference GEMM implementation
fn gemm_scalar(a: &[f32], b: &[f32], c: &mut [f32], m: usize, k: usize, n: usize) {
    c.fill(0.0);
    for i in 0..m {
        for j in 0..n {
            let mut sum = 0.0f32;
            for kk in 0..k {
                sum += a[i * k + kk] * b[kk * n + j];
            }
            c[i * n + j] = sum;
        }
    }
}

/// Scalar reference attention implementation
fn attention_scalar(
    query: &[f32],
    key: &[f32],
    value: &[f32],
    head_dim: usize,
    kv_len: usize,
    scale: f32,
) -> Vec<f32> {
    // Compute attention scores
    let mut scores = Vec::with_capacity(kv_len);
    for t in 0..kv_len {
        let k_offset = t * head_dim;
        let score: f32 = query
            .iter()
            .zip(&key[k_offset..k_offset + head_dim])
            .map(|(q, k)| q * k * scale)
            .sum();
        scores.push(score);
    }

    // Softmax
    let max_score = scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let exp_scores: Vec<f32> = scores.iter().map(|s| (s - max_score).exp()).collect();
    let sum_exp: f32 = exp_scores.iter().sum();
    let attn_weights: Vec<f32> = exp_scores.iter().map(|e| e / sum_exp).collect();

    // Weighted sum of values
    let mut output = vec![0.0; head_dim];
    for (t, weight) in attn_weights.iter().enumerate() {
        let v_offset = t * head_dim;
        for (i, v) in value[v_offset..v_offset + head_dim].iter().enumerate() {
            output[i] += weight * v;
        }
    }

    output
}

/// Scalar reference RMSNorm implementation
fn rms_norm_scalar(x: &mut [f32], weight: &[f32], eps: f32) {
    let len = x.len();
    let sum_sq: f32 = x.iter().map(|v| v * v).sum();
    let inv_rms = 1.0 / (sum_sq / len as f32 + eps).sqrt();
    for (i, w) in weight.iter().enumerate() {
        x[i] = x[i] * inv_rms * w;
    }
}

/// Scalar reference LayerNorm implementation
fn layer_norm_scalar(x: &mut [f32], weight: &[f32], bias: &[f32], eps: f32) {
    let len = x.len();
    let mean: f32 = x.iter().sum::<f32>() / len as f32;
    let var: f32 = x.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / len as f32;
    let inv_std = 1.0 / (var + eps).sqrt();

    for i in 0..len {
        x[i] = (x[i] - mean) * inv_std * weight[i] + bias[i];
    }
}

// ========== Cross-Platform Tests ==========

#[test]
fn test_cross_platform_gemv() {
    let test_cases = [
        (4, 4),
        (8, 16),
        (16, 32),
        (32, 64),
        (64, 128),
        (100, 50),
        (7, 13), // Non-aligned
    ];

    for (m, n) in test_cases {
        let a: Vec<f32> = (0..m * n)
            .map(|i| ((i % 100) as f32 - 50.0) / 50.0)
            .collect();
        let x: Vec<f32> = (0..n).map(|i| ((i % 20) as f32 - 10.0) / 10.0).collect();

        let mut y_neon = vec![0.0; m];
        let mut y_scalar = vec![0.0; m];

        gemv_neon(&a, &x, &mut y_neon, m, n);
        gemv_scalar(&a, &x, &mut y_scalar, m, n);

        for i in 0..m {
            let abs_error = (y_neon[i] - y_scalar[i]).abs();
            let rel_error = abs_error / y_scalar[i].abs().max(1e-6);
            assert!(
                rel_error < 0.001 || abs_error < 1e-5,
                "Cross-platform GEMV mismatch at ({},{}) index {}: {} vs {} (rel: {:.6})",
                m,
                n,
                i,
                y_neon[i],
                y_scalar[i],
                rel_error
            );
        }
    }
}

#[test]
fn test_cross_platform_gemm() {
    let test_cases = [
        (4, 4, 4),
        (8, 16, 8),
        (16, 32, 16),
        (32, 64, 32),
        (7, 11, 13), // Non-aligned
    ];

    for (m, k, n) in test_cases {
        let a: Vec<f32> = (0..m * k)
            .map(|i| ((i % 100) as f32 - 50.0) / 100.0)
            .collect();
        let b: Vec<f32> = (0..k * n)
            .map(|i| ((i % 50) as f32 - 25.0) / 50.0)
            .collect();

        let mut c_neon = vec![0.0; m * n];
        let mut c_scalar = vec![0.0; m * n];

        gemm_neon(&a, &b, &mut c_neon, m, k, n);
        gemm_scalar(&a, &b, &mut c_scalar, m, k, n);

        for i in 0..(m * n) {
            let abs_error = (c_neon[i] - c_scalar[i]).abs();
            let rel_error = abs_error / c_scalar[i].abs().max(1e-6);
            assert!(
                rel_error < 0.01 || abs_error < 0.001,
                "Cross-platform GEMM mismatch at ({},{},{}) index {}: {} vs {} (rel: {:.6})",
                m,
                k,
                n,
                i,
                c_neon[i],
                c_scalar[i],
                rel_error
            );
        }
    }
}

#[test]
fn test_cross_platform_attention() {
    let test_cases = [(16, 4), (32, 8), (64, 16), (128, 32)];

    for (head_dim, kv_len) in test_cases {
        let scale = 1.0 / (head_dim as f32).sqrt();

        let query: Vec<f32> = (0..head_dim)
            .map(|i| ((i % 7) as f32 - 3.0) / 10.0)
            .collect();
        let key: Vec<f32> = (0..kv_len * head_dim)
            .map(|i| ((i % 11) as f32 - 5.0) / 20.0)
            .collect();
        let value: Vec<f32> = (0..kv_len * head_dim)
            .map(|i| ((i % 13) as f32 - 6.0) / 15.0)
            .collect();

        let output_neon = flash_attention_neon(&query, &key, &value, scale, false);
        let output_scalar = attention_scalar(&query, &key, &value, head_dim, kv_len, scale);

        assert_eq!(output_neon.len(), output_scalar.len());

        for i in 0..head_dim {
            let abs_error = (output_neon[i] - output_scalar[i]).abs();
            let rel_error = abs_error / output_scalar[i].abs().max(1e-6);
            assert!(
                rel_error < 0.01 || abs_error < 1e-4,
                "Cross-platform attention mismatch at head_dim={}, kv_len={}, index {}: {} vs {} (rel: {:.6})",
                head_dim, kv_len, i, output_neon[i], output_scalar[i], rel_error
            );
        }
    }
}

#[test]
fn test_cross_platform_rms_norm() {
    let test_cases = [8, 16, 32, 64, 128];

    for dim in test_cases {
        let mut x_neon: Vec<f32> = (0..dim)
            .map(|i| (i as f32 - dim as f32 / 2.0) / 10.0)
            .collect();
        let mut x_scalar = x_neon.clone();
        let weight: Vec<f32> = (0..dim).map(|i| 0.5 + (i as f32) * 0.01).collect();
        let eps = 1e-6;

        rms_norm_neon(&mut x_neon, &weight, eps);
        rms_norm_scalar(&mut x_scalar, &weight, eps);

        for i in 0..dim {
            let abs_error = (x_neon[i] - x_scalar[i]).abs();
            assert!(
                abs_error < 1e-4,
                "Cross-platform RMSNorm mismatch at dim={}, index {}: {} vs {} (abs: {:.6})",
                dim,
                i,
                x_neon[i],
                x_scalar[i],
                abs_error
            );
        }
    }
}

#[test]
fn test_cross_platform_layer_norm() {
    let test_cases = [8, 16, 32, 64, 128];

    for dim in test_cases {
        let mut x_neon: Vec<f32> = (0..dim).map(|i| (i as f32) * 0.1 - 5.0).collect();
        let mut x_scalar = x_neon.clone();
        let weight: Vec<f32> = vec![1.0; dim];
        let bias: Vec<f32> = vec![0.0; dim];
        let eps = 1e-6;

        layer_norm_neon(&mut x_neon, &weight, &bias, eps);
        layer_norm_scalar(&mut x_scalar, &weight, &bias, eps);

        for i in 0..dim {
            let abs_error = (x_neon[i] - x_scalar[i]).abs();
            assert!(
                abs_error < 1e-4,
                "Cross-platform LayerNorm mismatch at dim={}, index {}: {} vs {} (abs: {:.6})",
                dim,
                i,
                x_neon[i],
                x_scalar[i],
                abs_error
            );
        }
    }
}

// ========== Edge Case Tests ==========

#[test]
fn test_scalar_fallback_edge_cases() {
    // Zero vectors
    let a_zero = vec![0.0f32; 16];
    let x_zero = vec![0.0f32; 4];
    let mut y = vec![0.0f32; 4];

    gemv_neon(&a_zero, &x_zero, &mut y, 4, 4);
    assert!(
        y.iter().all(|&v| v == 0.0),
        "Zero input should give zero output"
    );

    // Single element
    let a_single = vec![3.0f32];
    let x_single = vec![4.0f32];
    let mut y_single = vec![0.0f32];

    gemv_neon(&a_single, &x_single, &mut y_single, 1, 1);
    assert!((y_single[0] - 12.0).abs() < 1e-5, "1x1 GEMV failed");

    // Negative values
    let a_neg: Vec<f32> = (0..16)
        .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
        .collect();
    let x_neg: Vec<f32> = (0..4)
        .map(|i| if i % 2 == 0 { -1.0 } else { 1.0 })
        .collect();
    let mut y_neg = vec![0.0f32; 4];

    gemv_neon(&a_neg, &x_neg, &mut y_neg, 4, 4);
    assert!(
        y_neg.iter().all(|&v| v.is_finite()),
        "Negative values should produce finite output"
    );
}

#[test]
fn test_scalar_fallback_numerical_stability() {
    // Very small values
    let a_small: Vec<f32> = vec![1e-20; 64];
    let x_small: Vec<f32> = vec![1e-20; 8];
    let mut y_small = vec![0.0f32; 8];

    gemv_neon(&a_small, &x_small, &mut y_small, 8, 8);
    assert!(
        y_small.iter().all(|&v| v.is_finite()),
        "Very small values should produce finite output"
    );

    // Large values (but not overflow)
    let a_large: Vec<f32> = vec![1e10; 64];
    let x_large: Vec<f32> = vec![1e-10; 8]; // Scale x to avoid overflow
    let mut y_large = vec![0.0f32; 8];

    gemv_neon(&a_large, &x_large, &mut y_large, 8, 8);
    assert!(
        y_large.iter().all(|&v| v.is_finite()),
        "Large values with small x should produce finite output"
    );

    // Mixed magnitudes
    let a_mixed: Vec<f32> = (0..64)
        .map(|i| if i % 2 == 0 { 1e5 } else { 1e-5 })
        .collect();
    let x_mixed: Vec<f32> = vec![1.0; 8];
    let mut y_mixed = vec![0.0f32; 8];

    gemv_neon(&a_mixed, &x_mixed, &mut y_mixed, 8, 8);
    assert!(
        y_mixed.iter().all(|&v| v.is_finite()),
        "Mixed magnitude values should produce finite output"
    );
}

#[test]
fn test_scalar_fallback_determinism() {
    let m = 32;
    let n = 64;

    let a: Vec<f32> = (0..m * n).map(|i| ((i as f32) * 0.1).sin()).collect();
    let x: Vec<f32> = (0..n).map(|i| ((i as f32) * 0.2).cos()).collect();

    // Run multiple times and verify same result
    let mut results = Vec::new();
    for _ in 0..5 {
        let mut y = vec![0.0f32; m];
        gemv_neon(&a, &x, &mut y, m, n);
        results.push(y);
    }

    for i in 1..results.len() {
        for j in 0..m {
            assert_eq!(
                results[0][j], results[i][j],
                "GEMV should be deterministic: run 0 vs run {} differ at index {}",
                i, j
            );
        }
    }
}

// ========== WASM Compatibility Tests ==========

#[test]
fn test_wasm_compatible_operations() {
    // These operations should work on WASM (no NEON)
    // Test with dimensions that don't require SIMD

    // Small GEMV
    let a = vec![1.0, 2.0, 3.0, 4.0];
    let x = vec![1.0, 1.0];
    let mut y = vec![0.0; 2];
    gemv_neon(&a, &x, &mut y, 2, 2);
    assert!((y[0] - 3.0).abs() < 1e-5); // 1*1 + 2*1 = 3
    assert!((y[1] - 7.0).abs() < 1e-5); // 3*1 + 4*1 = 7

    // Small GEMM
    let a_gemm = vec![1.0, 2.0, 3.0, 4.0];
    let b_gemm = vec![1.0, 0.0, 0.0, 1.0]; // Identity
    let mut c_gemm = vec![0.0; 4];
    gemm_neon(&a_gemm, &b_gemm, &mut c_gemm, 2, 2, 2);
    // A * I = A
    for i in 0..4 {
        assert!(
            (c_gemm[i] - a_gemm[i]).abs() < 1e-5,
            "GEMM with identity failed"
        );
    }

    // Small attention
    let query = vec![0.1, 0.2, 0.3, 0.4];
    let key = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8];
    let value = vec![1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0];
    let scale = 0.5;
    let output = flash_attention_neon(&query, &key, &value, scale, false);
    assert_eq!(output.len(), 4);
    assert!(output.iter().all(|&v| v.is_finite()));
}

#[test]
fn test_scalar_path_verification() {
    // Test that scalar fallback path produces correct results
    // for small inputs that might not trigger SIMD optimizations

    // Verify GEMV with small non-aligned dimensions
    let a = vec![1.0, 2.0, 3.0];
    let x = vec![1.0, 2.0, 3.0];
    let mut y = vec![0.0; 1];
    gemv_neon(&a, &x, &mut y, 1, 3);
    let expected = 1.0 + 4.0 + 9.0; // 1*1 + 2*2 + 3*3 = 14
    assert!(
        (y[0] - expected).abs() < 1e-5,
        "Scalar GEMV expected {}, got {}",
        expected,
        y[0]
    );

    // Verify GEMM with 1x1
    let a1 = vec![5.0f32];
    let b1 = vec![3.0f32];
    let mut c1 = vec![0.0f32];
    gemm_neon(&a1, &b1, &mut c1, 1, 1, 1);
    assert!(
        (c1[0] - 15.0).abs() < 1e-5,
        "1x1 GEMM expected 15, got {}",
        c1[0]
    );

    // Verify normalization with small vector
    let mut x_norm = vec![3.0, 4.0];
    let weight = vec![1.0, 1.0];
    rms_norm_neon(&mut x_norm, &weight, 1e-6);
    // RMS = sqrt((9+16)/2) = sqrt(12.5) = 3.536
    // Normalized: [3/3.536, 4/3.536] = [0.848, 1.131]
    assert!(x_norm.iter().all(|&v| v.is_finite()));
}
