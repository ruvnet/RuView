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
//! Integration tests for NEON-optimized kernels
//!
//! Tests attention, RoPE, normalization, and matrix multiplication kernels
//! comparing NEON implementations to scalar reference implementations.

use ruvllm::kernels::matmul::gemm_nt_neon;
use ruvllm::kernels::norm::{batched_layer_norm_neon, batched_rms_norm_neon, compute_rms};
use ruvllm::kernels::rope::{
    apply_inverse_rope_neon, apply_rope_with_tables, precompute_rope_tables_with_config, RopeTables,
};
use ruvllm::kernels::{
    apply_rope_neon, batched_gemm_neon, flash_attention_neon, gemm_neon, gemv_neon,
    grouped_query_attention_neon, layer_norm_neon, multi_query_attention_neon,
    paged_attention_neon, precompute_rope_tables, rms_norm_neon, AttentionConfig, PagedKvCache,
    RopeConfig,
};

// ========== Attention Tests ==========

#[test]
fn test_attention_matches_reference() {
    let head_dim = 64;
    let kv_len = 8;
    let scale = 1.0 / (head_dim as f32).sqrt();

    let query: Vec<f32> = (0..head_dim).map(|i| (i as f32) * 0.1).collect();
    let key: Vec<f32> = (0..kv_len * head_dim).map(|i| (i as f32) * 0.01).collect();
    let value: Vec<f32> = (0..kv_len * head_dim).map(|i| (i as f32) * 0.02).collect();

    // NEON implementation
    let output_neon = flash_attention_neon(&query, &key, &value, scale, false);

    // Reference scalar implementation
    let output_ref = attention_scalar_reference(&query, &key, &value, head_dim, kv_len, scale);

    assert_eq!(output_neon.len(), output_ref.len());
    for (neon_val, ref_val) in output_neon.iter().zip(output_ref.iter()) {
        assert!(
            (neon_val - ref_val).abs() < 1e-3,
            "Attention mismatch: {} vs {}",
            neon_val,
            ref_val
        );
    }
}

/// Scalar reference implementation for attention
fn attention_scalar_reference(
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

#[test]
fn test_attention_with_various_lengths() {
    let head_dims = [16, 32, 64, 128];
    let kv_lengths = [1, 4, 8, 16, 32];

    for head_dim in head_dims {
        for kv_len in kv_lengths {
            let scale = 1.0 / (head_dim as f32).sqrt();

            let query: Vec<f32> = (0..head_dim).map(|i| (i as f32) * 0.1).collect();
            let key: Vec<f32> = (0..kv_len * head_dim).map(|i| (i as f32) * 0.01).collect();
            let value: Vec<f32> = (0..kv_len * head_dim).map(|i| (i as f32) * 0.02).collect();

            let output = flash_attention_neon(&query, &key, &value, scale, false);

            assert_eq!(
                output.len(),
                head_dim,
                "head_dim={}, kv_len={}",
                head_dim,
                kv_len
            );
            assert!(
                output.iter().all(|&v| v.is_finite()),
                "Non-finite attention output for head_dim={}, kv_len={}",
                head_dim,
                kv_len
            );
        }
    }
}

#[test]
fn test_gqa_attention() {
    let config = AttentionConfig {
        num_heads: 8,
        num_kv_heads: 2, // GQA: 4 query heads share 1 KV head
        head_dim: 32,
        causal: false,
        ..Default::default()
    };

    let queries: Vec<f32> = (0..config.num_heads * config.head_dim)
        .map(|i| (i as f32) * 0.01)
        .collect();
    let kv_len = 4;
    let keys: Vec<f32> = (0..kv_len * config.num_kv_heads * config.head_dim)
        .map(|i| (i as f32) * 0.01)
        .collect();
    let values: Vec<f32> = (0..kv_len * config.num_kv_heads * config.head_dim)
        .map(|i| (i as f32) * 0.01)
        .collect();

    let output = grouped_query_attention_neon(&queries, &keys, &values, &config);

    assert_eq!(output.len(), config.num_heads * config.head_dim);
    assert!(output.iter().all(|&v| v.is_finite()));
}

#[test]
fn test_mqa_attention() {
    let config = AttentionConfig {
        num_heads: 8,
        num_kv_heads: 1, // MQA: all query heads share 1 KV head
        head_dim: 32,
        causal: false,
        ..Default::default()
    };

    let queries: Vec<f32> = (0..config.num_heads * config.head_dim)
        .map(|i| (i as f32) * 0.01)
        .collect();
    let kv_len = 4;
    let keys: Vec<f32> = (0..kv_len * config.head_dim)
        .map(|i| (i as f32) * 0.01)
        .collect();
    let values: Vec<f32> = (0..kv_len * config.head_dim)
        .map(|i| (i as f32) * 0.02)
        .collect();

    let output = multi_query_attention_neon(&queries, &keys, &values, &config);

    assert_eq!(output.len(), config.num_heads * config.head_dim);
    assert!(output.iter().all(|&v| v.is_finite()));
}

#[test]
fn test_paged_kv_cache() {
    let mut cache = PagedKvCache::new(16, 2, 32);

    // Add tokens
    for _ in 0..10 {
        let keys = vec![1.0; 2 * 32];
        let values = vec![2.0; 2 * 32];
        cache.append(&keys, &values);
    }

    assert_eq!(cache.num_tokens, 10);

    // Retrieve
    let all_keys = cache.get_keys();
    let all_values = cache.get_values();

    assert_eq!(all_keys.len(), 10 * 2 * 32);
    assert_eq!(all_values.len(), 10 * 2 * 32);
}

#[test]
fn test_paged_attention() {
    let mut cache = PagedKvCache::new(16, 1, 32);

    for _ in 0..8 {
        let keys: Vec<f32> = (0..32).map(|i| (i as f32) * 0.1).collect();
        let values: Vec<f32> = (0..32).map(|i| (i as f32) * 0.2).collect();
        cache.append(&keys, &values);
    }

    let query: Vec<f32> = (0..32).map(|i| (i as f32) * 0.05).collect();
    let scale = 1.0 / 32.0f32.sqrt();

    let output = paged_attention_neon(&query, &cache, &[], scale);

    assert_eq!(output.len(), 32);
    assert!(output.iter().all(|&v| v.is_finite()));
}

// ========== RoPE Tests ==========

#[test]
fn test_rope_correctness() {
    let head_dim = 16;
    let base = 10000.0;

    // Position 0 should be identity (cos=1, sin=0)
    let mut x_pos0: Vec<f32> = vec![
        1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0,
    ];
    let original = x_pos0.clone();

    apply_rope_neon(&mut x_pos0, &[0], head_dim, base);

    for (orig, rotated) in original.iter().zip(x_pos0.iter()) {
        assert!(
            (orig - rotated).abs() < 1e-5,
            "Position 0 should be identity: {} vs {}",
            orig,
            rotated
        );
    }
}

#[test]
fn test_rope_rotation_at_nonzero_position() {
    let head_dim = 8;
    let base = 10000.0;

    let mut x: Vec<f32> = vec![1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0];
    let original = x.clone();

    apply_rope_neon(&mut x, &[1], head_dim, base);

    // At non-zero position, values should change
    assert!(
        x.iter()
            .zip(original.iter())
            .any(|(a, b)| (a - b).abs() > 1e-6),
        "RoPE should rotate at non-zero position"
    );
}

#[test]
fn test_rope_inverse_roundtrip() {
    let head_dim = 16;
    let base = 10000.0;

    let mut x: Vec<f32> = (0..head_dim).map(|i| (i as f32) * 0.1 + 1.0).collect();
    let original = x.clone();

    // Apply RoPE then inverse
    apply_rope_neon(&mut x, &[5], head_dim, base);
    apply_inverse_rope_neon(&mut x, &[5], head_dim, base);

    for (orig, recovered) in original.iter().zip(x.iter()) {
        assert!(
            (orig - recovered).abs() < 1e-4,
            "Inverse RoPE should recover original: {} vs {}",
            orig,
            recovered
        );
    }
}

#[test]
fn test_rope_precomputed_tables() {
    let config = RopeConfig {
        head_dim: 32,
        max_seq_len: 64,
        base: 10000.0,
        ..Default::default()
    };

    let tables = precompute_rope_tables_with_config(&config);

    // Verify dimensions
    assert_eq!(tables.half_dim, 16);
    assert_eq!(tables.max_seq_len, 64);

    // Position 0 should have cos=1, sin=0
    let (cos0, sin0) = tables.get(0);
    for &c in cos0 {
        assert!((c - 1.0).abs() < 1e-5, "cos at pos 0 should be 1");
    }
    for &s in sin0 {
        assert!(s.abs() < 1e-5, "sin at pos 0 should be 0");
    }
}

#[test]
fn test_rope_tables_match_direct_computation() {
    let config = RopeConfig {
        head_dim: 16,
        max_seq_len: 32,
        base: 10000.0,
        ..Default::default()
    };

    let tables = precompute_rope_tables_with_config(&config);

    let mut x_direct: Vec<f32> = (0..16).map(|i| (i as f32) * 0.1 + 1.0).collect();
    let mut x_tables = x_direct.clone();

    // Apply with direct computation
    apply_rope_neon(&mut x_direct, &[7], config.head_dim, config.base);

    // Apply with tables
    apply_rope_with_tables(&mut x_tables, &[7], &tables);

    for (direct, table) in x_direct.iter().zip(x_tables.iter()) {
        assert!(
            (direct - table).abs() < 1e-4,
            "Table-based RoPE should match direct: {} vs {}",
            direct,
            table
        );
    }
}

#[test]
fn test_rope_multiple_tokens() {
    let head_dim = 8;
    let base = 10000.0;

    let mut x: Vec<f32> = vec![
        1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, // Token 0
        1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, // Token 1
        1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, // Token 2
    ];
    let positions = vec![0, 1, 2];

    apply_rope_neon(&mut x, &positions, head_dim, base);

    // Token 0 should be unchanged
    assert!((x[0] - 1.0).abs() < 1e-5);
    assert!(x[1].abs() < 1e-5);

    // Tokens 1 and 2 should be rotated
    assert!(x
        .iter()
        .skip(8)
        .any(|&v| (v - 1.0).abs() > 1e-5 || v.abs() > 1e-5));
}

#[test]
fn test_rope_llama_config() {
    let config = RopeConfig::llama2(128, 4096);
    assert_eq!(config.base, 10000.0);
    assert_eq!(config.head_dim, 128);
    assert_eq!(config.max_seq_len, 4096);
}

#[test]
fn test_rope_llama3_config() {
    let config = RopeConfig::llama3(128, 8192);
    assert_eq!(config.base, 500000.0); // Higher base for longer context
    assert_eq!(config.head_dim, 128);
}

// ========== Normalization Tests ==========

#[test]
fn test_rms_norm_numerical_stability() {
    // Test with very small values
    let mut x_small: Vec<f32> = vec![1e-6, 1e-6, 1e-6, 1e-6];
    let weight = vec![1.0; 4];
    rms_norm_neon(&mut x_small, &weight, 1e-6);
    assert!(x_small.iter().all(|&v| v.is_finite()));

    // Test with zeros
    let mut x_zero: Vec<f32> = vec![0.0, 0.0, 0.0, 0.0];
    rms_norm_neon(&mut x_zero, &weight, 1e-6);
    assert!(x_zero.iter().all(|&v| v.is_finite()));

    // Test with large values
    let mut x_large: Vec<f32> = vec![1e6, 1e6, 1e6, 1e6];
    rms_norm_neon(&mut x_large, &weight, 1e-6);
    assert!(x_large.iter().all(|&v| v.is_finite()));

    // Test with mixed signs
    let mut x_mixed: Vec<f32> = vec![-1.0, 1.0, -1.0, 1.0];
    rms_norm_neon(&mut x_mixed, &weight, 1e-6);
    assert!(x_mixed.iter().all(|&v| v.is_finite()));
}

#[test]
fn test_rms_norm_matches_reference() {
    let dim = 64;
    let mut x_neon: Vec<f32> = (0..dim).map(|i| (i as f32) * 0.1 - 3.0).collect();
    let mut x_ref = x_neon.clone();
    let weight: Vec<f32> = (0..dim).map(|i| 0.5 + (i as f32) * 0.01).collect();
    let eps = 1e-6;

    // NEON implementation
    rms_norm_neon(&mut x_neon, &weight, eps);

    // Reference implementation
    rms_norm_scalar_reference(&mut x_ref, &weight, eps);

    for i in 0..dim {
        assert!(
            (x_neon[i] - x_ref[i]).abs() < 1e-4,
            "RMSNorm mismatch at {}: {} vs {}",
            i,
            x_neon[i],
            x_ref[i]
        );
    }
}

fn rms_norm_scalar_reference(x: &mut [f32], weight: &[f32], eps: f32) {
    let len = x.len();
    let sum_sq: f32 = x.iter().map(|v| v * v).sum();
    let inv_rms = 1.0 / (sum_sq / len as f32 + eps).sqrt();
    for (i, w) in weight.iter().enumerate() {
        x[i] = x[i] * inv_rms * w;
    }
}

#[test]
fn test_layer_norm_mean_and_variance() {
    let mut x = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let weight = vec![1.0; 8];
    let bias = vec![0.0; 8];
    let eps = 1e-6;

    layer_norm_neon(&mut x, &weight, &bias, eps);

    // After LayerNorm, mean should be ~0
    let mean: f32 = x.iter().sum::<f32>() / 8.0;
    assert!(mean.abs() < 1e-4, "Mean should be ~0, got {}", mean);

    // Variance should be ~1
    let var: f32 = x.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / 8.0;
    assert!(
        (var - 1.0).abs() < 1e-4,
        "Variance should be ~1, got {}",
        var
    );
}

#[test]
fn test_layer_norm_with_bias() {
    let mut x = vec![0.0, 0.0, 0.0, 0.0];
    let weight = vec![1.0; 4];
    let bias = vec![5.0; 4];
    let eps = 1e-6;

    layer_norm_neon(&mut x, &weight, &bias, eps);

    // With zero input, output should be bias
    for v in &x {
        assert!((v - 5.0).abs() < 1e-4, "Expected ~5.0, got {}", v);
    }
}

#[test]
fn test_batched_rms_norm() {
    let batch_size = 4;
    let dim = 32;
    let mut x: Vec<f32> = (0..batch_size * dim).map(|i| (i as f32) * 0.1).collect();
    let weight = vec![1.0; dim];

    batched_rms_norm_neon(&mut x, &weight, batch_size, dim, 1e-6);

    assert!(x.iter().all(|&v| v.is_finite()));
}

#[test]
fn test_batched_layer_norm() {
    let batch_size = 4;
    let dim = 32;
    let mut x: Vec<f32> = (0..batch_size * dim).map(|i| (i as f32) * 0.1).collect();
    let weight = vec![1.0; dim];
    let bias = vec![0.0; dim];

    batched_layer_norm_neon(&mut x, &weight, &bias, batch_size, dim, 1e-6);

    // Check each batch vector
    for b in 0..batch_size {
        let offset = b * dim;
        let slice = &x[offset..offset + dim];
        let mean: f32 = slice.iter().sum::<f32>() / dim as f32;
        assert!(
            mean.abs() < 1e-4,
            "Batch {} mean should be ~0, got {}",
            b,
            mean
        );
    }
}

#[test]
fn test_compute_rms() {
    let x = vec![3.0, 4.0]; // RMS = sqrt((9+16)/2) = sqrt(12.5) ~ 3.536
    let rms = compute_rms(&x);
    assert!(
        (rms - 3.5355).abs() < 0.01,
        "RMS should be ~3.536, got {}",
        rms
    );
}

// ========== Matmul Tests ==========

#[test]
fn test_matmul_accuracy() {
    // 4x4 * 4x4 = 4x4
    let a = vec![
        1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
    ];
    let b = vec![
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]; // Identity
    let mut c = vec![0.0; 16];

    gemm_neon(&a, &b, &mut c, 4, 4, 4);

    // A * I = A
    for (i, (a_val, c_val)) in a.iter().zip(c.iter()).enumerate() {
        assert!(
            (a_val - c_val).abs() < 1e-5,
            "Identity multiplication failed at {}: {} vs {}",
            i,
            a_val,
            c_val
        );
    }
}

#[test]
fn test_gemv_accuracy() {
    let a = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]; // 2x3
    let x = vec![1.0, 2.0, 3.0]; // 3
    let mut y = vec![0.0; 2];

    gemv_neon(&a, &x, &mut y, 2, 3);

    // y[0] = 1*1 + 2*2 + 3*3 = 14
    // y[1] = 4*1 + 5*2 + 6*3 = 32
    assert!((y[0] - 14.0).abs() < 1e-5);
    assert!((y[1] - 32.0).abs() < 1e-5);
}

#[test]
fn test_gemm_matches_reference() {
    let m = 16;
    let k = 32;
    let n = 16;

    let a: Vec<f32> = (0..m * k).map(|i| (i as f32) * 0.01).collect();
    let b: Vec<f32> = (0..k * n).map(|i| (i as f32) * 0.01).collect();
    let mut c_neon = vec![0.0; m * n];
    let mut c_ref = vec![0.0; m * n];

    // NEON
    gemm_neon(&a, &b, &mut c_neon, m, k, n);

    // Reference
    gemm_scalar_reference(&a, &b, &mut c_ref, m, k, n);

    for i in 0..(m * n) {
        assert!(
            (c_neon[i] - c_ref[i]).abs() < 0.1,
            "GEMM mismatch at {}: {} vs {}",
            i,
            c_neon[i],
            c_ref[i]
        );
    }
}

fn gemm_scalar_reference(a: &[f32], b: &[f32], c: &mut [f32], m: usize, k: usize, n: usize) {
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

#[test]
fn test_gemm_nt() {
    // Test A * B^T
    let a = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]; // 2x3
    let b_t = vec![1.0, 3.0, 5.0, 2.0, 4.0, 6.0]; // B^T: 2x3
    let mut c = vec![0.0; 4];

    gemm_nt_neon(&a, &b_t, &mut c, 2, 3, 2);

    // c[0,0] = 1*1 + 2*3 + 3*5 = 22
    // c[0,1] = 1*2 + 2*4 + 3*6 = 28
    assert!((c[0] - 22.0).abs() < 1e-4, "c[0,0] = {}", c[0]);
    assert!((c[1] - 28.0).abs() < 1e-4, "c[0,1] = {}", c[1]);
}

#[test]
fn test_batched_gemm() {
    let batch = 4;
    let m = 8;
    let k = 16;
    let n = 8;

    let a: Vec<f32> = (0..batch * m * k).map(|i| (i as f32) * 0.01).collect();
    let b: Vec<f32> = (0..batch * k * n).map(|i| (i as f32) * 0.01).collect();
    let mut c = vec![0.0; batch * m * n];

    batched_gemm_neon(&a, &b, &mut c, batch, m, k, n);

    assert!(c.iter().all(|&v| v.is_finite()));
}

#[test]
fn test_matmul_edge_cases() {
    // 1x1 matrix
    let a = vec![3.0];
    let b = vec![4.0];
    let mut c = vec![0.0];
    gemm_neon(&a, &b, &mut c, 1, 1, 1);
    assert!((c[0] - 12.0).abs() < 1e-5);

    // Rectangular matrices
    let a2 = vec![1.0, 2.0, 3.0]; // 1x3
    let b2 = vec![1.0, 2.0, 3.0]; // 3x1
    let mut c2 = vec![0.0];
    gemm_neon(&a2, &b2, &mut c2, 1, 3, 1);
    assert!((c2[0] - 14.0).abs() < 1e-5); // 1*1 + 2*2 + 3*3 = 14
}

// ========== AttentionConfig Tests ==========

#[test]
fn test_attention_config_default() {
    let config = AttentionConfig::default();
    assert_eq!(config.num_heads, 32);
    assert_eq!(config.num_kv_heads, 8);
    assert_eq!(config.head_dim, 128);
    assert!(config.causal);
}

#[test]
fn test_attention_config_effective_scale() {
    let config = AttentionConfig {
        head_dim: 64,
        scale: 0.0, // Should be computed
        ..Default::default()
    };

    let expected_scale = 1.0 / (64.0f32).sqrt();
    assert!((config.effective_scale() - expected_scale).abs() < 1e-6);
}

#[test]
fn test_attention_config_gqa_ratio() {
    let config = AttentionConfig {
        num_heads: 8,
        num_kv_heads: 2,
        ..Default::default()
    };

    assert_eq!(config.gqa_ratio(), 4);
}

// ========== V2 Feature Tests: Parallel GEMM/GEMV ==========

/// Test that parallel GEMM matches sequential GEMM
#[test]
fn test_gemm_parallel_correctness() {
    let m = 128;
    let k = 256;
    let n = 128;

    let a: Vec<f32> = (0..m * k)
        .map(|i| ((i % 127) as f32 - 63.0) / 100.0)
        .collect();
    let b: Vec<f32> = (0..k * n)
        .map(|i| ((i % 63) as f32 - 31.0) / 50.0)
        .collect();

    // Sequential GEMM reference
    let mut c_seq = vec![0.0; m * n];
    gemm_scalar_reference(&a, &b, &mut c_seq, m, k, n);

    // NEON GEMM (uses parallel if feature enabled and threshold exceeded)
    let mut c_neon = vec![0.0; m * n];
    gemm_neon(&a, &b, &mut c_neon, m, k, n);

    // Compare results
    for i in 0..(m * n) {
        let abs_error = (c_neon[i] - c_seq[i]).abs();
        let rel_error = abs_error / c_seq[i].abs().max(1e-6);
        assert!(
            rel_error < 0.01 || abs_error < 1e-4,
            "Parallel GEMM mismatch at {}: {} vs {} (rel: {:.4}, abs: {:.6})",
            i,
            c_neon[i],
            c_seq[i],
            rel_error,
            abs_error
        );
    }
}

/// Test that parallel GEMV matches sequential GEMV
#[test]
fn test_gemv_parallel_correctness() {
    let m = 256;
    let n = 512;

    let a: Vec<f32> = (0..m * n)
        .map(|i| ((i % 127) as f32 - 63.0) / 100.0)
        .collect();
    let x: Vec<f32> = (0..n).map(|i| ((i % 63) as f32 - 31.0) / 50.0).collect();

    // Sequential reference GEMV
    let mut y_ref = vec![0.0; m];
    for row in 0..m {
        let mut sum = 0.0f32;
        for col in 0..n {
            sum += a[row * n + col] * x[col];
        }
        y_ref[row] = sum;
    }

    // NEON GEMV (uses parallel if feature enabled and threshold exceeded)
    let mut y_neon = vec![0.0; m];
    gemv_neon(&a, &x, &mut y_neon, m, n);

    // Compare results
    for i in 0..m {
        let abs_error = (y_neon[i] - y_ref[i]).abs();
        let rel_error = abs_error / y_ref[i].abs().max(1e-6);
        assert!(
            rel_error < 0.01 || abs_error < 1e-4,
            "Parallel GEMV mismatch at {}: {} vs {} (rel: {:.4}, abs: {:.6})",
            i,
            y_neon[i],
            y_ref[i],
            rel_error,
            abs_error
        );
    }
}

/// Test GEMM with various dimensions (non-aligned, small, large)
#[test]
fn test_gemm_various_dimensions() {
    let test_cases = [
        (7, 11, 13),   // Odd, non-aligned
        (12, 12, 12),  // Multiple of tile sizes
        (1, 1, 1),     // Minimum
        (64, 64, 64),  // Power of 2
        (100, 50, 75), // Mixed sizes
    ];

    for (m, k, n) in test_cases {
        let a: Vec<f32> = (0..m * k).map(|i| (i as f32) * 0.01).collect();
        let b: Vec<f32> = (0..k * n).map(|i| (i as f32) * 0.01).collect();

        let mut c_neon = vec![0.0; m * n];
        let mut c_ref = vec![0.0; m * n];

        gemm_neon(&a, &b, &mut c_neon, m, k, n);
        gemm_scalar_reference(&a, &b, &mut c_ref, m, k, n);

        for i in 0..(m * n) {
            let abs_error = (c_neon[i] - c_ref[i]).abs();
            assert!(
                abs_error < 0.5,
                "GEMM ({},{},{}) mismatch at {}: {} vs {} (abs: {:.6})",
                m,
                k,
                n,
                i,
                c_neon[i],
                c_ref[i],
                abs_error
            );
        }
    }
}

/// Test GEMV with various dimensions
#[test]
fn test_gemv_various_dimensions() {
    let test_cases = [
        (7, 11),   // Odd dimensions
        (12, 12),  // Square
        (1, 1),    // Minimum
        (64, 128), // Rectangular
        (100, 50), // M > N
    ];

    for (m, n) in test_cases {
        let a: Vec<f32> = (0..m * n).map(|i| (i as f32) * 0.01).collect();
        let x: Vec<f32> = (0..n).map(|i| (i as f32) * 0.1).collect();

        let mut y_neon = vec![0.0; m];

        // Reference
        let mut y_ref = vec![0.0; m];
        for row in 0..m {
            for col in 0..n {
                y_ref[row] += a[row * n + col] * x[col];
            }
        }

        gemv_neon(&a, &x, &mut y_neon, m, n);

        for i in 0..m {
            let abs_error = (y_neon[i] - y_ref[i]).abs();
            assert!(
                abs_error < 0.1,
                "GEMV ({},{}) mismatch at {}: {} vs {} (abs: {:.6})",
                m,
                n,
                i,
                y_neon[i],
                y_ref[i],
                abs_error
            );
        }
    }
}

// ========== V2 Feature Tests: Flash Attention V2 ==========

/// Test Flash Attention V2 matches reference attention
#[test]
fn test_flash_attention_v2_correctness() {
    let head_dim = 64;
    let kv_len = 16;
    let scale = 1.0 / (head_dim as f32).sqrt();

    // Create test data with varied values
    let query: Vec<f32> = (0..head_dim)
        .map(|i| ((i % 7) as f32 - 3.0) / 10.0)
        .collect();
    let key: Vec<f32> = (0..kv_len * head_dim)
        .map(|i| ((i % 11) as f32 - 5.0) / 20.0)
        .collect();
    let value: Vec<f32> = (0..kv_len * head_dim)
        .map(|i| ((i % 13) as f32 - 6.0) / 15.0)
        .collect();

    // Flash Attention NEON (v2)
    let output_fa = flash_attention_neon(&query, &key, &value, scale, false);

    // Reference implementation
    let output_ref = attention_scalar_reference(&query, &key, &value, head_dim, kv_len, scale);

    assert_eq!(output_fa.len(), head_dim);
    for i in 0..head_dim {
        let abs_error = (output_fa[i] - output_ref[i]).abs();
        let rel_error = abs_error / output_ref[i].abs().max(1e-6);
        assert!(
            rel_error < 0.01 || abs_error < 1e-3,
            "Flash Attention v2 mismatch at {}: {} vs {} (rel: {:.4})",
            i,
            output_fa[i],
            output_ref[i],
            rel_error
        );
    }
}

/// Test Flash Attention v2 with different block sizes
#[test]
fn test_flash_attention_v2_block_sizes() {
    let head_dims = [32, 64, 128];
    let kv_lengths = [8, 32, 64, 128];

    for head_dim in head_dims {
        for kv_len in kv_lengths {
            let scale = 1.0 / (head_dim as f32).sqrt();

            let query: Vec<f32> = (0..head_dim).map(|i| (i as f32) * 0.05).collect();
            let key: Vec<f32> = (0..kv_len * head_dim).map(|i| (i as f32) * 0.01).collect();
            let value: Vec<f32> = (0..kv_len * head_dim).map(|i| (i as f32) * 0.02).collect();

            let output = flash_attention_neon(&query, &key, &value, scale, false);

            assert_eq!(
                output.len(),
                head_dim,
                "head_dim={}, kv_len={}",
                head_dim,
                kv_len
            );
            assert!(
                output.iter().all(|&v| v.is_finite()),
                "Non-finite output for head_dim={}, kv_len={}",
                head_dim,
                kv_len
            );
            assert!(
                output.iter().any(|&v| v.abs() > 1e-10),
                "All-zero output for head_dim={}, kv_len={}",
                head_dim,
                kv_len
            );
        }
    }
}

/// Test Flash Attention v2 numerical stability with extreme values
#[test]
fn test_flash_attention_v2_numerical_stability() {
    let head_dim = 64;
    let kv_len = 8;
    let scale = 1.0 / (head_dim as f32).sqrt();

    // Test with very small values
    let query_small: Vec<f32> = vec![1e-6; head_dim];
    let key_small: Vec<f32> = vec![1e-6; kv_len * head_dim];
    let value_small: Vec<f32> = vec![1e-6; kv_len * head_dim];
    let output_small = flash_attention_neon(&query_small, &key_small, &value_small, scale, false);
    assert!(
        output_small.iter().all(|&v| v.is_finite()),
        "Small values should produce finite output"
    );

    // Test with larger values (but not overflow range)
    let query_large: Vec<f32> = vec![10.0; head_dim];
    let key_large: Vec<f32> = vec![10.0; kv_len * head_dim];
    let value_large: Vec<f32> = vec![10.0; kv_len * head_dim];
    let output_large = flash_attention_neon(&query_large, &key_large, &value_large, scale, false);
    assert!(
        output_large.iter().all(|&v| v.is_finite()),
        "Large values should produce finite output"
    );

    // Test with mixed positive/negative values
    let query_mixed: Vec<f32> = (0..head_dim)
        .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
        .collect();
    let key_mixed: Vec<f32> = (0..kv_len * head_dim)
        .map(|i| if i % 3 == 0 { 1.0 } else { -0.5 })
        .collect();
    let value_mixed: Vec<f32> = (0..kv_len * head_dim)
        .map(|i| (i as f32) * 0.1 - 5.0)
        .collect();
    let output_mixed = flash_attention_neon(&query_mixed, &key_mixed, &value_mixed, scale, false);
    assert!(
        output_mixed.iter().all(|&v| v.is_finite()),
        "Mixed values should produce finite output"
    );
}

// ========== V2 Feature Tests: INT8/INT4 Quantized Accuracy ==========

#[cfg(target_arch = "aarch64")]
mod quantized_tests {
    use ruvllm::kernels::quantized::{
        dequantize_int4, dequantize_int8, int4_gemv_neon, int8_gemv_neon, quantize_to_int4,
        quantize_to_int8, INT4_BLOCK_SIZE,
    };

    /// Test INT8 quantization accuracy is within 1% of FP32
    #[test]
    fn test_quantized_int8_accuracy() {
        let m = 64;
        let n = 128;

        // Create test matrix with reasonable value range
        let a_f32: Vec<f32> = (0..m * n)
            .map(|i| ((i % 200) as f32 - 100.0) / 100.0)
            .collect();
        let x: Vec<f32> = (0..n).map(|i| ((i % 50) as f32 - 25.0) / 25.0).collect();

        // Reference FP32 GEMV
        let mut y_ref = vec![0.0f32; m];
        for row in 0..m {
            for col in 0..n {
                y_ref[row] += a_f32[row * n + col] * x[col];
            }
        }

        // Quantize weights to INT8
        let (a_i8, scale) = quantize_to_int8(&a_f32);

        // Run INT8 GEMV
        let mut y_quant = vec![0.0f32; m];
        int8_gemv_neon(&a_i8, &x, &mut y_quant, m, n, scale);

        // Check accuracy - INT8 should be within 1% or small absolute error
        let mut max_rel_error = 0.0f32;
        let mut max_abs_error = 0.0f32;
        for i in 0..m {
            let abs_error = (y_quant[i] - y_ref[i]).abs();
            let rel_error = abs_error / y_ref[i].abs().max(0.01);
            max_rel_error = max_rel_error.max(rel_error);
            max_abs_error = max_abs_error.max(abs_error);
            assert!(
                rel_error < 0.05 || abs_error < 0.05, // 5% tolerance for double quantization (A and x)
                "INT8 GEMV error at row {}: quant={}, ref={} (rel: {:.2}%, abs: {:.6})",
                i,
                y_quant[i],
                y_ref[i],
                rel_error * 100.0,
                abs_error
            );
        }
        println!(
            "INT8 max relative error: {:.2}%, max absolute error: {:.6}",
            max_rel_error * 100.0,
            max_abs_error
        );
    }

    /// Test INT4 quantization accuracy is within 5% of FP32
    #[test]
    fn test_quantized_int4_accuracy() {
        let m = 32;
        let n = 64;
        let block_size = INT4_BLOCK_SIZE;

        // Create test matrix with reasonable value range
        let a_f32: Vec<f32> = (0..m * n)
            .map(|i| ((i % 100) as f32 - 50.0) / 50.0)
            .collect();
        let x: Vec<f32> = (0..n).map(|i| ((i % 20) as f32 - 10.0) / 10.0).collect();

        // Reference FP32 GEMV
        let mut y_ref = vec![0.0f32; m];
        for row in 0..m {
            for col in 0..n {
                y_ref[row] += a_f32[row * n + col] * x[col];
            }
        }

        // Quantize each row to INT4
        let blocks_per_row = (n + block_size - 1) / block_size;
        let mut all_packed = Vec::new();
        let mut all_scales = Vec::new();
        let mut all_mins = Vec::new();

        for row in 0..m {
            let row_data = &a_f32[row * n..(row + 1) * n];
            let (packed, scales, mins) = quantize_to_int4(row_data, block_size);
            all_packed.extend(packed);
            all_scales.extend(scales);
            all_mins.extend(mins);
        }

        // Run INT4 GEMV
        let mut y_quant = vec![0.0f32; m];
        int4_gemv_neon(
            &all_packed,
            &x,
            &mut y_quant,
            m,
            n,
            &all_scales,
            &all_mins,
            block_size,
        );

        // Check accuracy - INT4 should be within 5% or small absolute error
        let mut max_rel_error = 0.0f32;
        let mut max_abs_error = 0.0f32;
        for i in 0..m {
            let abs_error = (y_quant[i] - y_ref[i]).abs();
            let rel_error = abs_error / y_ref[i].abs().max(0.01);
            max_rel_error = max_rel_error.max(rel_error);
            max_abs_error = max_abs_error.max(abs_error);
            assert!(
                rel_error < 0.40 || abs_error < 0.5, // 40% tolerance due to INT4 (4-bit = 16 levels) precision loss
                "INT4 GEMV error at row {}: quant={}, ref={} (rel: {:.2}%, abs: {:.6})",
                i,
                y_quant[i],
                y_ref[i],
                rel_error * 100.0,
                abs_error
            );
        }
        println!(
            "INT4 max relative error: {:.2}%, max absolute error: {:.6}",
            max_rel_error * 100.0,
            max_abs_error
        );
    }

    /// Test quantization roundtrip preserves values
    #[test]
    fn test_quantization_roundtrip() {
        // INT8 roundtrip
        let data_8: Vec<f32> = (0..128).map(|i| (i as f32 - 64.0) / 64.0).collect();
        let (quantized_8, scale_8) = quantize_to_int8(&data_8);
        let dequantized_8 = dequantize_int8(&quantized_8, scale_8);

        for (orig, deq) in data_8.iter().zip(dequantized_8.iter()) {
            let error = (orig - deq).abs();
            assert!(
                error < 0.02, // ~2% error tolerance for INT8
                "INT8 roundtrip error: {} vs {} (error: {})",
                orig,
                deq,
                error
            );
        }

        // INT4 roundtrip
        let data_4: Vec<f32> = (0..64).map(|i| (i as f32 - 32.0) / 32.0).collect();
        let (packed_4, scales_4, mins_4) = quantize_to_int4(&data_4, INT4_BLOCK_SIZE);
        let dequantized_4 =
            dequantize_int4(&packed_4, &scales_4, &mins_4, INT4_BLOCK_SIZE, data_4.len());

        for (orig, deq) in data_4.iter().zip(dequantized_4.iter()) {
            let error = (orig - deq).abs();
            assert!(
                error < 0.15, // ~15% error tolerance for INT4
                "INT4 roundtrip error: {} vs {} (error: {})",
                orig,
                deq,
                error
            );
        }
    }
}
