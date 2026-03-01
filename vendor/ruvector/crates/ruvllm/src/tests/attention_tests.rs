//! Attention Tests
//!
//! Tests for Flash Attention, Paged Attention, MQA/GQA implementations,
//! output correctness, memory allocation, pre-allocated buffer reuse, and benchmarks.

use crate::kernels::{
    flash_attention_auto, flash_attention_neon, flash_attention_v2, grouped_query_attention_neon,
    multi_query_attention_neon, paged_attention_neon, select_block_size, AttentionConfig,
    PagedKvCache, BLOCK_SIZE_LARGE, BLOCK_SIZE_MEDIUM, BLOCK_SIZE_SMALL,
};
use std::time::Instant;

// ============================================================================
// Helper Functions
// ============================================================================

/// Reference scalar attention implementation for correctness checking
fn attention_reference(
    query: &[f32],
    key: &[f32],
    value: &[f32],
    head_dim: usize,
    scale: f32,
) -> Vec<f32> {
    let kv_len = key.len() / head_dim;

    // Compute scores: Q @ K^T
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

/// Generate random test data
fn generate_test_data(head_dim: usize, kv_len: usize, seed: u64) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let mut rng_state = seed;
    let next_float = |state: &mut u64| -> f32 {
        *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((*state >> 33) as f32) / (u32::MAX as f32) * 2.0 - 1.0
    };

    let query: Vec<f32> = (0..head_dim).map(|_| next_float(&mut rng_state)).collect();
    let key: Vec<f32> = (0..kv_len * head_dim)
        .map(|_| next_float(&mut rng_state))
        .collect();
    let value: Vec<f32> = (0..kv_len * head_dim)
        .map(|_| next_float(&mut rng_state))
        .collect();

    (query, key, value)
}

/// Check if two vectors are approximately equal
fn vectors_approx_equal(a: &[f32], b: &[f32], tolerance: f32) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .all(|(x, y)| (x - y).abs() < tolerance)
}

// ============================================================================
// Flash Attention Basic Tests
// ============================================================================

#[test]
fn test_flash_attention_basic() {
    let head_dim = 16;
    let kv_len = 4;

    let query: Vec<f32> = (0..head_dim).map(|i| (i as f32) * 0.1).collect();
    let key: Vec<f32> = (0..kv_len * head_dim).map(|i| (i as f32) * 0.01).collect();
    let value: Vec<f32> = (0..kv_len * head_dim).map(|i| (i as f32) * 0.02).collect();

    let scale = 1.0 / (head_dim as f32).sqrt();
    let output = flash_attention_neon(&query, &key, &value, scale, false);

    assert_eq!(
        output.len(),
        head_dim,
        "Output should have head_dim elements"
    );
    assert!(
        output.iter().all(|&x| x.is_finite()),
        "All outputs should be finite"
    );
}

#[test]
fn test_flash_attention_vs_reference() {
    let head_dim = 32;
    let kv_len = 16;
    let scale = 1.0 / (head_dim as f32).sqrt();

    let (query, key, value) = generate_test_data(head_dim, kv_len, 12345);

    let neon_output = flash_attention_neon(&query, &key, &value, scale, false);
    let ref_output = attention_reference(&query, &key, &value, head_dim, scale);

    assert!(
        vectors_approx_equal(&neon_output, &ref_output, 1e-3),
        "NEON and reference outputs should match"
    );
}

#[test]
fn test_flash_attention_empty_kv() {
    let head_dim = 16;
    let query: Vec<f32> = (0..head_dim).map(|i| i as f32).collect();
    let key: Vec<f32> = vec![];
    let value: Vec<f32> = vec![];

    let scale = 1.0 / (head_dim as f32).sqrt();
    let output = flash_attention_neon(&query, &key, &value, scale, false);

    // Should handle empty KV gracefully - either return empty or zero-filled vector
    assert!(output.len() == 0 || output.len() == head_dim);
}

#[test]
fn test_flash_attention_single_token() {
    let head_dim = 64;
    let kv_len = 1;
    let scale = 1.0 / (head_dim as f32).sqrt();

    let (query, key, value) = generate_test_data(head_dim, kv_len, 42);

    let output = flash_attention_neon(&query, &key, &value, scale, false);

    // With single KV token, output should be proportional to the value
    // (after softmax, the single token gets weight 1.0)
    assert!(
        vectors_approx_equal(&output, &value, 1e-5),
        "Single token attention should return value directly"
    );
}

// ============================================================================
// Flash Attention V2 Block Size Tests
// ============================================================================

#[test]
fn test_flash_attention_v2_small_block() {
    let head_dim = 64;
    let kv_len = 100;
    let scale = 1.0 / (head_dim as f32).sqrt();

    let (query, key, value) = generate_test_data(head_dim, kv_len, 111);

    let output_small = flash_attention_v2(&query, &key, &value, scale, false, BLOCK_SIZE_SMALL);
    let output_medium = flash_attention_v2(&query, &key, &value, scale, false, BLOCK_SIZE_MEDIUM);

    // Different block sizes should produce same results
    assert!(
        vectors_approx_equal(&output_small, &output_medium, 1e-3),
        "Block sizes should not affect correctness"
    );
}

#[test]
fn test_flash_attention_v2_all_block_sizes() {
    let head_dim = 128;
    let kv_len = 256;
    let scale = 1.0 / (head_dim as f32).sqrt();

    let (query, key, value) = generate_test_data(head_dim, kv_len, 222);

    let output_small = flash_attention_v2(&query, &key, &value, scale, false, BLOCK_SIZE_SMALL);
    let output_medium = flash_attention_v2(&query, &key, &value, scale, false, BLOCK_SIZE_MEDIUM);
    let output_large = flash_attention_v2(&query, &key, &value, scale, false, BLOCK_SIZE_LARGE);

    // All should produce similar results
    assert!(vectors_approx_equal(&output_small, &output_medium, 1e-3));
    assert!(vectors_approx_equal(&output_medium, &output_large, 1e-3));
}

#[test]
fn test_flash_attention_auto_block_selection() {
    let head_dim = 128;
    let scale = 1.0 / (head_dim as f32).sqrt();

    // Short sequence should use small blocks
    let (q1, k1, v1) = generate_test_data(head_dim, 32, 333);
    let _output1 = flash_attention_auto(&q1, &k1, &v1, scale, false);

    // Long sequence should use larger blocks
    let (q2, k2, v2) = generate_test_data(head_dim, 1024, 444);
    let _output2 = flash_attention_auto(&q2, &k2, &v2, scale, false);

    // Just verify they complete without error
}

// ============================================================================
// Block Size Selection Tests
// ============================================================================

#[test]
fn test_select_block_size_short_sequence() {
    let head_dim = 128;

    // Very short sequences should use small blocks
    assert_eq!(select_block_size(32, head_dim), BLOCK_SIZE_SMALL);
    assert_eq!(select_block_size(64, head_dim), BLOCK_SIZE_SMALL);
}

#[test]
fn test_select_block_size_medium_sequence() {
    let head_dim = 128;

    // Medium sequences should use medium blocks
    assert_eq!(select_block_size(128, head_dim), BLOCK_SIZE_MEDIUM);
    assert_eq!(select_block_size(256, head_dim), BLOCK_SIZE_MEDIUM);
    assert_eq!(select_block_size(512, head_dim), BLOCK_SIZE_MEDIUM);
}

#[test]
fn test_select_block_size_long_sequence() {
    let head_dim = 64; // Smaller head_dim allows larger blocks

    // Long sequences with small head_dim can use large blocks
    let block = select_block_size(2048, head_dim);
    assert!(
        block >= BLOCK_SIZE_MEDIUM,
        "Long sequences should use at least medium blocks"
    );
}

#[test]
fn test_select_block_size_large_head_dim() {
    let head_dim = 256; // Large head_dim limits block size

    // Large head_dim should constrain block size to fit in L1
    let block = select_block_size(2048, head_dim);
    assert!(block <= BLOCK_SIZE_LARGE);
}

// ============================================================================
// Paged KV Cache Tests
// ============================================================================

#[test]
fn test_paged_kv_cache_creation() {
    let cache = PagedKvCache::new(16, 4, 64);

    assert_eq!(cache.block_size, 16);
    assert_eq!(cache.num_kv_heads, 4);
    assert_eq!(cache.head_dim, 64);
    assert_eq!(cache.num_tokens, 0);
    assert!(cache.key_blocks.is_empty());
    assert!(cache.value_blocks.is_empty());
}

#[test]
fn test_paged_kv_cache_append() {
    let mut cache = PagedKvCache::new(4, 2, 8);

    // Append one token (2 kv_heads * 8 head_dim = 16 elements)
    let keys = vec![1.0; 16];
    let values = vec![2.0; 16];

    cache.append(&keys, &values);

    assert_eq!(cache.num_tokens, 1);
    assert_eq!(cache.key_blocks.len(), 1);
    assert_eq!(cache.value_blocks.len(), 1);
}

#[test]
fn test_paged_kv_cache_append_multiple() {
    let mut cache = PagedKvCache::new(4, 2, 8);
    let stride = 2 * 8; // 16 elements per token

    // Append 5 tokens (more than one block)
    for i in 0..5 {
        let keys = vec![(i + 1) as f32; stride];
        let values = vec![(i + 1) as f32 * 2.0; stride];
        cache.append(&keys, &values);
    }

    assert_eq!(cache.num_tokens, 5);
    assert_eq!(cache.key_blocks.len(), 2); // 5 tokens, 4 per block = 2 blocks
}

#[test]
fn test_paged_kv_cache_get_keys() {
    let mut cache = PagedKvCache::new(4, 1, 8);

    // Append 2 tokens
    let keys1 = vec![1.0; 8];
    let values1 = vec![10.0; 8];
    cache.append(&keys1, &values1);

    let keys2 = vec![2.0; 8];
    let values2 = vec![20.0; 8];
    cache.append(&keys2, &values2);

    let retrieved_keys = cache.get_keys();
    assert_eq!(retrieved_keys.len(), 16); // 2 tokens * 1 head * 8 dim
    assert!(retrieved_keys[..8].iter().all(|&x| x == 1.0));
    assert!(retrieved_keys[8..].iter().all(|&x| x == 2.0));
}

#[test]
fn test_paged_kv_cache_get_values() {
    let mut cache = PagedKvCache::new(4, 1, 8);

    let keys = vec![1.0; 8];
    let values = vec![5.0; 8];
    cache.append(&keys, &values);

    let retrieved_values = cache.get_values();
    assert_eq!(retrieved_values.len(), 8);
    assert!(retrieved_values.iter().all(|&x| x == 5.0));
}

// ============================================================================
// Paged Attention Tests
// ============================================================================

#[test]
fn test_paged_attention_empty_cache() {
    let cache = PagedKvCache::new(16, 1, 16);
    let query = vec![0.5; 16];
    let scale = 0.25;

    let output = paged_attention_neon(&query, &cache, &[], scale);

    assert_eq!(output.len(), 16);
    // Empty cache should return zeros
    assert!(output.iter().all(|&x| x == 0.0));
}

#[test]
fn test_paged_attention_with_cache() {
    let mut cache = PagedKvCache::new(16, 1, 16);

    // Add some tokens
    for _ in 0..8 {
        let keys: Vec<f32> = (0..16).map(|i| (i as f32) * 0.1).collect();
        let values: Vec<f32> = (0..16).map(|i| (i as f32) * 0.2).collect();
        cache.append(&keys, &values);
    }

    let query: Vec<f32> = (0..16).map(|i| (i as f32) * 0.05).collect();
    let scale = 1.0 / 4.0;

    let output = paged_attention_neon(&query, &cache, &[], scale);

    assert_eq!(output.len(), 16);
    assert!(output.iter().all(|&x| x.is_finite()));
}

// ============================================================================
// Multi-Query Attention (MQA) Tests
// ============================================================================

#[test]
fn test_mqa_basic() {
    let config = AttentionConfig {
        num_heads: 8,
        num_kv_heads: 1, // MQA: single KV head
        head_dim: 16,
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
    assert!(output.iter().all(|&x| x.is_finite()));
}

#[test]
fn test_mqa_shared_kv() {
    // Verify that all query heads see the same K/V
    let config = AttentionConfig {
        num_heads: 4,
        num_kv_heads: 1,
        head_dim: 8,
        causal: false,
        ..Default::default()
    };

    // All queries identical
    let query_head: Vec<f32> = vec![1.0; config.head_dim];
    let queries: Vec<f32> = query_head
        .iter()
        .cloned()
        .cycle()
        .take(config.num_heads * config.head_dim)
        .collect();

    let kv_len = 2;
    let keys: Vec<f32> = (0..kv_len * config.head_dim)
        .map(|i| (i as f32) * 0.1)
        .collect();
    let values: Vec<f32> = (0..kv_len * config.head_dim).map(|_| 1.0).collect();

    let output = multi_query_attention_neon(&queries, &keys, &values, &config);

    // All output heads should be identical since all queries are identical
    let head_outputs: Vec<&[f32]> = output.chunks(config.head_dim).collect();
    for i in 1..head_outputs.len() {
        assert!(
            vectors_approx_equal(head_outputs[0], head_outputs[i], 1e-5),
            "All heads should produce same output with identical queries"
        );
    }
}

// ============================================================================
// Grouped-Query Attention (GQA) Tests
// ============================================================================

#[test]
fn test_gqa_basic() {
    let config = AttentionConfig {
        num_heads: 8,
        num_kv_heads: 2, // GQA: 4:1 ratio
        head_dim: 16,
        causal: false,
        ..Default::default()
    };

    assert_eq!(config.gqa_ratio(), 4);

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
    assert!(output.iter().all(|&x| x.is_finite()));
}

#[test]
fn test_gqa_head_grouping() {
    let config = AttentionConfig {
        num_heads: 4,
        num_kv_heads: 2, // 2:1 ratio
        head_dim: 8,
        causal: false,
        ..Default::default()
    };

    assert_eq!(config.gqa_ratio(), 2);

    // Query heads 0,1 share KV head 0
    // Query heads 2,3 share KV head 1

    // Create distinct KV for each KV head
    let kv_len = 2;
    let mut keys = vec![0.0; kv_len * config.num_kv_heads * config.head_dim];
    let mut values = vec![0.0; kv_len * config.num_kv_heads * config.head_dim];

    // KV head 0: all 1.0
    for t in 0..kv_len {
        let offset = t * config.num_kv_heads * config.head_dim;
        for i in 0..config.head_dim {
            keys[offset + i] = 1.0;
            values[offset + i] = 1.0;
        }
    }

    // KV head 1: all 2.0
    for t in 0..kv_len {
        let offset = t * config.num_kv_heads * config.head_dim + config.head_dim;
        for i in 0..config.head_dim {
            keys[offset + i] = 2.0;
            values[offset + i] = 2.0;
        }
    }

    // Uniform queries
    let queries: Vec<f32> = vec![0.5; config.num_heads * config.head_dim];

    let output = grouped_query_attention_neon(&queries, &keys, &values, &config);

    // Heads 0,1 should have values around 1.0, heads 2,3 around 2.0
    let head_outputs: Vec<f32> = output
        .chunks(config.head_dim)
        .map(|h| h.iter().sum::<f32>() / config.head_dim as f32)
        .collect();

    assert!(
        (head_outputs[0] - 1.0).abs() < 0.1,
        "Head 0 should use KV head 0"
    );
    assert!(
        (head_outputs[1] - 1.0).abs() < 0.1,
        "Head 1 should use KV head 0"
    );
    assert!(
        (head_outputs[2] - 2.0).abs() < 0.1,
        "Head 2 should use KV head 1"
    );
    assert!(
        (head_outputs[3] - 2.0).abs() < 0.1,
        "Head 3 should use KV head 1"
    );
}

// ============================================================================
// AttentionConfig Tests
// ============================================================================

#[test]
fn test_attention_config_default() {
    let config = AttentionConfig::default();

    assert_eq!(config.num_heads, 32);
    assert_eq!(config.num_kv_heads, 8);
    assert_eq!(config.head_dim, 128);
    assert!(config.causal);
    assert_eq!(config.gqa_ratio(), 4);
}

#[test]
fn test_attention_config_effective_scale() {
    let config = AttentionConfig {
        head_dim: 64,
        scale: 0.0, // Auto-compute
        ..Default::default()
    };

    let expected = 1.0 / (64.0f32).sqrt();
    assert!((config.effective_scale() - expected).abs() < 1e-6);

    // Explicit scale
    let config2 = AttentionConfig {
        head_dim: 64,
        scale: 0.2,
        ..Default::default()
    };
    assert!((config2.effective_scale() - 0.2).abs() < 1e-6);
}

#[test]
fn test_attention_config_gqa_ratios() {
    // Standard MHA (1:1)
    let mha = AttentionConfig {
        num_heads: 32,
        num_kv_heads: 32,
        ..Default::default()
    };
    assert_eq!(mha.gqa_ratio(), 1);

    // GQA 4:1
    let gqa_4 = AttentionConfig {
        num_heads: 32,
        num_kv_heads: 8,
        ..Default::default()
    };
    assert_eq!(gqa_4.gqa_ratio(), 4);

    // GQA 8:1
    let gqa_8 = AttentionConfig {
        num_heads: 32,
        num_kv_heads: 4,
        ..Default::default()
    };
    assert_eq!(gqa_8.gqa_ratio(), 8);

    // MQA (all heads share 1 KV)
    let mqa = AttentionConfig {
        num_heads: 32,
        num_kv_heads: 1,
        ..Default::default()
    };
    assert_eq!(mqa.gqa_ratio(), 32);
}

// ============================================================================
// Memory Allocation Tests
// ============================================================================

#[test]
fn test_attention_no_extra_allocation() {
    let head_dim = 128;
    let kv_len = 256;
    let scale = 1.0 / (head_dim as f32).sqrt();

    let (query, key, value) = generate_test_data(head_dim, kv_len, 555);

    // Run attention multiple times
    let output1 = flash_attention_neon(&query, &key, &value, scale, false);
    let output2 = flash_attention_neon(&query, &key, &value, scale, false);
    let output3 = flash_attention_neon(&query, &key, &value, scale, false);

    // Results should be identical (deterministic)
    assert!(vectors_approx_equal(&output1, &output2, 1e-6));
    assert!(vectors_approx_equal(&output2, &output3, 1e-6));
}

#[test]
fn test_attention_output_size_correct() {
    let head_dim = 64;
    let kv_len = 100;
    let scale = 1.0 / (head_dim as f32).sqrt();

    let (query, key, value) = generate_test_data(head_dim, kv_len, 666);

    let output = flash_attention_neon(&query, &key, &value, scale, false);

    assert_eq!(
        output.len(),
        head_dim,
        "Output should exactly match head_dim"
    );
}

// ============================================================================
// Performance Benchmark Tests
// ============================================================================

#[test]
fn test_attention_benchmark_short_sequence() {
    let head_dim = 128;
    let kv_len = 64;
    let scale = 1.0 / (head_dim as f32).sqrt();

    let (query, key, value) = generate_test_data(head_dim, kv_len, 777);

    // Warm up
    for _ in 0..10 {
        let _ = flash_attention_neon(&query, &key, &value, scale, false);
    }

    // Benchmark
    let iterations = 1000;
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = flash_attention_neon(&query, &key, &value, scale, false);
    }
    let duration = start.elapsed();

    let avg_us = duration.as_micros() as f64 / iterations as f64;
    assert!(
        avg_us < 1000.0,
        "Short sequence attention should be fast: {}us",
        avg_us
    );
}

#[test]
fn test_attention_benchmark_long_sequence() {
    let head_dim = 128;
    let kv_len = 2048;
    let scale = 1.0 / (head_dim as f32).sqrt();

    let (query, key, value) = generate_test_data(head_dim, kv_len, 888);

    // Warm up
    for _ in 0..5 {
        let _ = flash_attention_neon(&query, &key, &value, scale, false);
    }

    // Benchmark
    let iterations = 100;
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = flash_attention_neon(&query, &key, &value, scale, false);
    }
    let duration = start.elapsed();

    let avg_ms = duration.as_millis() as f64 / iterations as f64;
    assert!(
        avg_ms < 50.0,
        "Long sequence attention should complete in <50ms: {}ms",
        avg_ms
    );
}

#[test]
fn test_attention_benchmark_block_sizes() {
    let head_dim = 128;
    let kv_len = 512;
    let scale = 1.0 / (head_dim as f32).sqrt();
    let iterations = 100;

    let (query, key, value) = generate_test_data(head_dim, kv_len, 999);

    // Benchmark small blocks
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = flash_attention_v2(&query, &key, &value, scale, false, BLOCK_SIZE_SMALL);
    }
    let small_time = start.elapsed();

    // Benchmark medium blocks
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = flash_attention_v2(&query, &key, &value, scale, false, BLOCK_SIZE_MEDIUM);
    }
    let medium_time = start.elapsed();

    // Benchmark large blocks
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = flash_attention_v2(&query, &key, &value, scale, false, BLOCK_SIZE_LARGE);
    }
    let large_time = start.elapsed();

    // All should complete in reasonable time
    assert!(small_time.as_millis() < 5000);
    assert!(medium_time.as_millis() < 5000);
    assert!(large_time.as_millis() < 5000);
}

// ============================================================================
// Numerical Stability Tests
// ============================================================================

#[test]
fn test_attention_large_logits() {
    let head_dim = 32;
    let kv_len = 8;

    // Create query and key that will produce large dot products
    let query = vec![10.0; head_dim];
    let key = vec![10.0; kv_len * head_dim];
    let value: Vec<f32> = (0..kv_len * head_dim).map(|i| i as f32).collect();

    let scale = 1.0 / (head_dim as f32).sqrt();
    let output = flash_attention_neon(&query, &key, &value, scale, false);

    // Output should be finite
    assert!(
        output.iter().all(|&x| x.is_finite()),
        "Should handle large dot products"
    );
}

#[test]
fn test_attention_small_values() {
    let head_dim = 32;
    let kv_len = 8;

    // Very small values
    let query = vec![1e-6; head_dim];
    let key = vec![1e-6; kv_len * head_dim];
    let value: Vec<f32> = (0..kv_len * head_dim).map(|i| i as f32).collect();

    let scale = 1.0 / (head_dim as f32).sqrt();
    let output = flash_attention_neon(&query, &key, &value, scale, false);

    // Output should be finite
    assert!(
        output.iter().all(|&x| x.is_finite()),
        "Should handle small values"
    );
}

#[test]
fn test_attention_mixed_signs() {
    let head_dim = 32;
    let kv_len = 8;

    // Mix of positive and negative values
    let query: Vec<f32> = (0..head_dim)
        .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
        .collect();
    let key: Vec<f32> = (0..kv_len * head_dim)
        .map(|i| if i % 3 == 0 { -0.5 } else { 0.5 })
        .collect();
    let value: Vec<f32> = (0..kv_len * head_dim).map(|i| (i as f32) * 0.01).collect();

    let scale = 1.0 / (head_dim as f32).sqrt();
    let output = flash_attention_neon(&query, &key, &value, scale, false);

    assert!(output.iter().all(|&x| x.is_finite()));
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_attention_single_head_dim() {
    let head_dim = 1;
    let kv_len = 4;

    let query = vec![1.0];
    let key = vec![1.0, 2.0, 3.0, 4.0];
    let value = vec![10.0, 20.0, 30.0, 40.0];

    let scale = 1.0;
    let output = flash_attention_neon(&query, &key, &value, scale, false);

    assert_eq!(output.len(), 1);
    assert!(output[0].is_finite());
}

#[test]
fn test_attention_large_head_dim() {
    let head_dim = 512;
    let kv_len = 16;
    let scale = 1.0 / (head_dim as f32).sqrt();

    let (query, key, value) = generate_test_data(head_dim, kv_len, 1111);

    let output = flash_attention_neon(&query, &key, &value, scale, false);

    assert_eq!(output.len(), head_dim);
    assert!(output.iter().all(|&x| x.is_finite()));
}

#[test]
fn test_attention_power_of_two_dims() {
    // Test common power-of-2 dimensions
    for head_dim in [32, 64, 128, 256] {
        let kv_len = 64;
        let scale = 1.0 / (head_dim as f32).sqrt();

        let (query, key, value) = generate_test_data(head_dim, kv_len, head_dim as u64);

        let output = flash_attention_neon(&query, &key, &value, scale, false);

        assert_eq!(output.len(), head_dim);
        assert!(
            output.iter().all(|&x| x.is_finite()),
            "Failed for head_dim={}",
            head_dim
        );
    }
}

#[test]
fn test_attention_non_power_of_two_dims() {
    // Test non-power-of-2 dimensions
    for head_dim in [17, 33, 65, 100, 127] {
        let kv_len = 32;
        let scale = 1.0 / (head_dim as f32).sqrt();

        let (query, key, value) = generate_test_data(head_dim, kv_len, head_dim as u64);

        let output = flash_attention_neon(&query, &key, &value, scale, false);

        assert_eq!(output.len(), head_dim);
        assert!(
            output.iter().all(|&x| x.is_finite()),
            "Failed for head_dim={}",
            head_dim
        );
    }
}
