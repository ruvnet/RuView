//! FlashAttention demonstration
//!
//! Shows how to use FlashAttention-style tiled attention for CPU inference.

use ruvector_mincut_gated_transformer::flash_attention::{
    flash_attention_forward, flash_attention_forward_i8, flash_mha, FlashAttentionConfig,
};

fn main() {
    println!("=== FlashAttention CPU Demo ===\n");

    // Configuration for 64-dim attention head
    let config = FlashAttentionConfig::for_head_dim(64);
    println!("Configuration:");
    println!("  Block size (Q): {}", config.block_size_q);
    println!("  Block size (KV): {}", config.block_size_kv);
    println!("  Head dimension: {}", config.head_dim);
    println!("  Causal masking: {}", config.causal);
    println!("  Softmax scale: {:.4}\n", config.softmax_scale);

    // Example 1: Single-head attention
    {
        println!("Example 1: Single-head attention (128 tokens, 64 dims)");

        let seq_len = 128;
        let head_dim = 64;

        // Create random-like input (deterministic for demo)
        let q: Vec<f32> = (0..seq_len * head_dim)
            .map(|i| ((i % 100) as f32) * 0.01)
            .collect();
        let k: Vec<f32> = (0..seq_len * head_dim)
            .map(|i| ((i % 100) as f32) * 0.01)
            .collect();
        let v: Vec<f32> = (0..seq_len * head_dim)
            .map(|i| ((i % 100) as f32) * 0.01)
            .collect();

        let mut output = vec![0.0f32; seq_len * head_dim];

        flash_attention_forward(&config, &q, &k, &v, seq_len, seq_len, &mut output);

        println!("  ✓ Computed attention output: {} elements", output.len());
        println!("  ✓ First 5 output values: {:?}\n", &output[0..5]);
    }

    // Example 2: Multi-head attention
    {
        println!("Example 2: Multi-head attention (8 heads, 64 tokens, 64 dims)");

        let num_heads = 8;
        let seq_len = 64;
        let head_dim = 64;

        let total_size = num_heads * seq_len * head_dim;
        let q: Vec<f32> = (0..total_size).map(|i| ((i % 100) as f32) * 0.01).collect();
        let k: Vec<f32> = (0..total_size).map(|i| ((i % 100) as f32) * 0.01).collect();
        let v: Vec<f32> = (0..total_size).map(|i| ((i % 100) as f32) * 0.01).collect();

        let mut output = vec![0.0f32; total_size];

        flash_mha(
            &config,
            &q,
            &k,
            &v,
            num_heads,
            seq_len,
            seq_len,
            &mut output,
        );

        println!(
            "  ✓ Computed multi-head attention: {} elements",
            output.len()
        );
        println!("  ✓ Output per head: {} elements", seq_len * head_dim);
        println!("  ✓ First 5 output values: {:?}\n", &output[0..5]);
    }

    // Example 3: INT8 quantized attention
    {
        println!("Example 3: INT8 quantized attention (64 tokens, 64 dims)");

        let seq_len = 64;
        let head_dim = 64;

        // Create FP32 data and quantize to INT8
        let q_f32: Vec<f32> = (0..seq_len * head_dim)
            .map(|i| ((i % 100) as f32) * 0.01)
            .collect();
        let k_f32: Vec<f32> = (0..seq_len * head_dim)
            .map(|i| ((i % 100) as f32) * 0.01)
            .collect();
        let v_f32: Vec<f32> = (0..seq_len * head_dim)
            .map(|i| ((i % 100) as f32) * 0.01)
            .collect();

        // Quantization scales
        let q_scale = 0.01f32;
        let k_scale = 0.01f32;
        let v_scale = 0.01f32;

        // Quantize to INT8
        let q_i8: Vec<i8> = q_f32
            .iter()
            .map(|&x| (x / q_scale).round().clamp(-128.0, 127.0) as i8)
            .collect();
        let k_i8: Vec<i8> = k_f32
            .iter()
            .map(|&x| (x / k_scale).round().clamp(-128.0, 127.0) as i8)
            .collect();
        let v_i8: Vec<i8> = v_f32
            .iter()
            .map(|&x| (x / v_scale).round().clamp(-128.0, 127.0) as i8)
            .collect();

        let mut output = vec![0.0f32; seq_len * head_dim];

        flash_attention_forward_i8(
            &config,
            &q_i8,
            &k_i8,
            &v_i8,
            q_scale,
            k_scale,
            v_scale,
            seq_len,
            seq_len,
            &mut output,
        );

        println!("  ✓ Computed INT8 quantized attention");
        println!("  ✓ Memory savings: 4× (INT8 vs FP32)");
        println!("  ✓ First 5 output values: {:?}\n", &output[0..5]);
    }

    // Example 4: Configuration for long sequences
    {
        println!("Example 4: Optimized config for long sequences (512 tokens)");

        let long_config = FlashAttentionConfig::for_long_sequence(64);
        println!(
            "  Block size (Q): {} (smaller for cache reuse)",
            long_config.block_size_q
        );
        println!(
            "  Block size (KV): {} (larger for efficiency)",
            long_config.block_size_kv
        );

        let seq_len = 512;
        let head_dim = 64;

        let q: Vec<f32> = (0..seq_len * head_dim)
            .map(|i| ((i % 100) as f32) * 0.01)
            .collect();
        let k: Vec<f32> = (0..seq_len * head_dim)
            .map(|i| ((i % 100) as f32) * 0.01)
            .collect();
        let v: Vec<f32> = (0..seq_len * head_dim)
            .map(|i| ((i % 100) as f32) * 0.01)
            .collect();

        let mut output = vec![0.0f32; seq_len * head_dim];

        flash_attention_forward(&long_config, &q, &k, &v, seq_len, seq_len, &mut output);

        println!("  ✓ Computed attention for {} tokens", seq_len);
        println!("  ✓ Memory efficient: O(n) instead of O(n²)");
        println!("  ✓ Cache efficient: Tiled for L1/L2 cache\n");
    }

    println!("=== All examples completed successfully! ===");
}
