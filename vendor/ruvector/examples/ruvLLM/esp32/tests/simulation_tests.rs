//! Simulation Tests for ESP32 RuvLLM
//!
//! These tests validate that the implementation will work correctly
//! on ESP32 hardware by simulating memory constraints and operations.

use std::time::Instant;

// Import the crate
use ruvllm_esp32::prelude::*;
use ruvllm_esp32::model::ModelConfig;
use ruvllm_esp32::quantized::{QuantizationType, QuantizedTensor, matmul_int8, binary_xnor_popcount, QuantParams};
use ruvllm_esp32::attention::{MicroAttention, LinearAttention, SlidingWindowAttention};
use ruvllm_esp32::embedding::{EmbeddingTable, RotaryEmbedding, SimpleTokenizer};

/// Validate memory fits within ESP32 constraints
#[test]
fn test_memory_constraints_all_variants() {
    println!("\n=== Memory Constraint Validation ===\n");

    for variant in [
        Esp32Variant::Esp32,
        Esp32Variant::Esp32S2,
        Esp32Variant::Esp32S3,
        Esp32Variant::Esp32C3,
        Esp32Variant::Esp32C6,
    ] {
        let config = ModelConfig::for_variant(variant);

        // Validate config is correct for variant
        assert!(config.validate(variant).is_ok(), "{:?} config validation failed", variant);

        let model = TinyModel::new(config.clone()).unwrap();
        let engine = MicroEngine::new(model).unwrap();

        let usage = engine.memory_usage();
        let available = variant.max_model_ram();

        println!("{:?}:", variant);
        println!("  SRAM: {} KB, Max Model RAM: {} KB", variant.sram_bytes() / 1024, available / 1024);
        println!("  Model: {} KB, Buffers: {} KB, KV: {} KB",
            usage.model_weights / 1024,
            usage.activation_buffers / 1024,
            usage.kv_cache / 1024
        );
        println!("  Total: {} KB, Headroom: {} KB\n",
            usage.total / 1024,
            (available.saturating_sub(usage.total)) / 1024
        );

        assert!(
            usage.total <= available,
            "{:?}: Memory overflow! {} > {} bytes",
            variant, usage.total, available
        );

        // Ensure at least 10KB headroom for stack/runtime
        assert!(
            available - usage.total >= 10 * 1024,
            "{:?}: Insufficient headroom: {} bytes",
            variant, available - usage.total
        );
    }
}

/// Test INT8 matmul correctness
#[test]
fn test_int8_matmul_correctness() {
    // Small matrix for verification
    let weights = [1i8, 2, 3, 4, 5, 6, 7, 8, 9]; // 3x3
    let input = [1i8, 2, 3];
    let mut output = [0i32; 3];

    let params = QuantParams::default();

    matmul_int8(&weights, &params, &input, &params, &mut output, 3, 3);

    // Manual calculation:
    // output[0] = 1*1 + 2*2 + 3*3 = 14
    // output[1] = 4*1 + 5*2 + 6*3 = 32
    // output[2] = 7*1 + 8*2 + 9*3 = 50
    assert_eq!(output[0], 14);
    assert_eq!(output[1], 32);
    assert_eq!(output[2], 50);
}

/// Test binary XNOR popcount
#[test]
fn test_binary_xnor_correctness() {
    let a = [0b11110000u8, 0b10101010];
    let b = [0b11110000u8, 0b10101010];

    // Perfect match: all 16 bits same -> popcount = 16
    // Result = 16 * 2 - 16 = 16
    let result = binary_xnor_popcount(&a, &b);
    assert_eq!(result, 16);

    // Complete mismatch
    let c = [0b00001111u8, 0b01010101];
    let result2 = binary_xnor_popcount(&a, &c);
    // XNOR of 0b11110000 and 0b00001111 = 0b00000000 -> 0 bits
    // XNOR of 0b10101010 and 0b01010101 = 0b00000000 -> 0 bits
    // Result = 0 * 2 - 16 = -16
    assert_eq!(result2, -16);
}

/// Test quantization compression ratios
#[test]
fn test_quantization_compression() {
    let data: Vec<f32> = (0..1024).map(|i| (i as f32 / 512.0) - 1.0).collect();

    let int8: QuantizedTensor<2048> = QuantizedTensor::from_f32(&data, &[1024], QuantizationType::Int8).unwrap();
    let int4: QuantizedTensor<2048> = QuantizedTensor::from_f32(&data, &[1024], QuantizationType::Int4).unwrap();
    let binary: QuantizedTensor<2048> = QuantizedTensor::from_f32(&data, &[1024], QuantizationType::Binary).unwrap();

    println!("\nQuantization compression:");
    println!("  INT8:   {} bytes, {:.1}% savings", int8.compressed_size(), int8.memory_savings() * 100.0);
    println!("  INT4:   {} bytes, {:.1}% savings", int4.compressed_size(), int4.memory_savings() * 100.0);
    println!("  Binary: {} bytes, {:.1}% savings", binary.compressed_size(), binary.memory_savings() * 100.0);

    // Verify compression
    assert_eq!(int8.compressed_size(), 1024);   // 1 byte per value
    assert_eq!(int4.compressed_size(), 512);    // 0.5 bytes per value
    assert_eq!(binary.compressed_size(), 128);  // 0.125 bytes per value
}

/// Test attention mechanisms
#[test]
fn test_attention_mechanisms() {
    // Micro attention
    let attn = MicroAttention::new(64, 4);
    let query = [32i8; 16];
    let key1 = [32i8; 16];
    let key2 = [16i8; 16];
    let keys: [&[i8]; 2] = [&key1, &key2];
    let mut scores = [0i32; 2];

    attn.compute_scores(&query, &keys, &mut scores);

    // First key should have higher score (more similar)
    assert!(scores[0] > scores[1], "scores[0]={} should be > scores[1]={}", scores[0], scores[1]);

    // Softmax should normalize
    attn.softmax_fixed(&mut scores);
    let sum: i32 = scores.iter().sum();
    assert!((sum - 256).abs() < 20, "Softmax sum {} should be ~256", sum);
}

/// Test linear attention
#[test]
fn test_linear_attention() {
    let attn = LinearAttention::new(16);

    let query = [10i8; 16];
    let key = [10i8; 16];
    let value = [5i8; 16];
    let keys: [&[i8]; 1] = [&key];
    let values: [&[i8]; 1] = [&value];

    let mut output = [0i32; 16];
    attn.forward(&query, &keys, &values, &mut output);

    // Output should be non-zero
    assert!(output.iter().any(|&x| x != 0), "Linear attention output should be non-zero");
}

/// Test embedding operations
#[test]
fn test_embedding_operations() {
    let embed: EmbeddingTable<256, 64> = EmbeddingTable::random(256, 64, 42).unwrap();

    let mut output = [0i8; 64];
    embed.lookup(42, &mut output).unwrap();

    // Should have non-zero values
    assert!(output.iter().any(|&x| x != 0));

    // Test accumulation
    let mut accum = [0i32; 64];
    embed.lookup_add(42, &mut accum).unwrap();
    embed.lookup_add(42, &mut accum).unwrap();

    // Should be 2x the single lookup
    for i in 0..64 {
        assert_eq!(accum[i], 2 * output[i] as i32);
    }
}

/// Test rotary embeddings
#[test]
fn test_rotary_embeddings() {
    let mut rope = RotaryEmbedding::new(32, 10000);

    // Test different positions
    for pos in [0, 5, 10, 20] {
        rope.update_cache(pos);

        let mut x = [64i8; 32];
        let original = x;
        rope.apply(&mut x, pos);

        // Values should change (except possibly at position 0)
        if pos > 0 {
            assert!(x != original, "RoPE should modify values at position {}", pos);
        }
    }
}

/// Test tokenizer
#[test]
fn test_tokenizer() {
    let tokenizer = SimpleTokenizer::ascii();

    // Test encoding
    let tokens = tokenizer.encode("Hello World!");
    assert_eq!(tokens.len(), 12);
    assert_eq!(tokens[0], 'H' as u16);

    // Test decoding
    let decoded = tokenizer.decode(&tokens);
    assert_eq!(&decoded[..], b"Hello World!");
}

/// Test full inference pipeline
#[test]
fn test_full_inference_pipeline() {
    let config = ModelConfig::for_variant(Esp32Variant::Esp32);
    let model = TinyModel::new(config).unwrap();
    let mut engine = MicroEngine::new(model).unwrap();

    // Single token forward pass
    let next_token = engine.forward_one(10).unwrap();
    assert!(next_token < 256);

    // Full generation
    engine.reset();
    let prompt = [1u16, 2, 3, 4, 5];
    let gen_config = InferenceConfig {
        max_tokens: 5,
        greedy: true,
        ..Default::default()
    };

    let result = engine.generate(&prompt, &gen_config).unwrap();
    assert!(!result.tokens.is_empty());
    assert!(result.tokens.len() <= 5);

    println!("\nGeneration test:");
    println!("  Prompt: {:?}", prompt);
    println!("  Generated: {:?}", result.tokens.as_slice());
    println!("  Peak memory: {} KB", result.peak_memory_bytes / 1024);
}

/// Test model serialization
#[test]
fn test_model_serialization() {
    let config = ModelConfig::default();
    let model = TinyModel::new(config).unwrap();

    let header = model.to_bytes();
    assert_eq!(&header[0..4], b"RUVM");
    assert!(header.len() >= 32);
}

/// Performance simulation test
#[test]
fn test_performance_simulation() {
    println!("\n=== Performance Simulation ===\n");

    // ESP32 runs at 240MHz
    const ESP32_CLOCK_MHZ: f64 = 240.0;
    // Estimated cycles per INT8 MAC operation
    const CYCLES_PER_MAC: f64 = 4.0;

    let config = ModelConfig::for_variant(Esp32Variant::Esp32);

    // Count operations per forward pass
    let embed_dim = config.embed_dim;
    let hidden_dim = config.hidden_dim;
    let num_layers = config.num_layers;
    let num_heads = config.num_heads;

    // Per layer:
    // - QKV projection: 3 * embed_dim * embed_dim MACs
    // - Attention: seq_len * head_dim * num_heads MACs (simplified)
    // - FFN: 3 * embed_dim * hidden_dim MACs
    let qkv_macs = 3 * embed_dim * embed_dim;
    let attn_macs = 32 * (embed_dim / num_heads) * num_heads; // Assuming seq_len=32
    let ffn_macs = 3 * embed_dim * hidden_dim;
    let layer_macs = qkv_macs + attn_macs + ffn_macs;
    let total_macs = layer_macs * num_layers;

    // Estimate time
    let cycles = total_macs as f64 * CYCLES_PER_MAC;
    let estimated_us = cycles / ESP32_CLOCK_MHZ;
    let estimated_tokens_per_sec = 1_000_000.0 / estimated_us;

    println!("Model configuration:");
    println!("  Embed dim: {}", embed_dim);
    println!("  Hidden dim: {}", hidden_dim);
    println!("  Layers: {}", num_layers);
    println!("  Heads: {}", num_heads);
    println!();
    println!("Operations per forward pass:");
    println!("  QKV projections: {} MACs", qkv_macs * num_layers);
    println!("  Attention: {} MACs", attn_macs * num_layers);
    println!("  FFN: {} MACs", ffn_macs * num_layers);
    println!("  Total: {} MACs ({:.2}M)", total_macs, total_macs as f64 / 1_000_000.0);
    println!();
    println!("Estimated ESP32 performance:");
    println!("  Cycles: {:.0}", cycles);
    println!("  Time per token: {:.1} us ({:.2} ms)", estimated_us, estimated_us / 1000.0);
    println!("  Tokens per second: {:.1}", estimated_tokens_per_sec);

    // Actual benchmark on host
    let model = TinyModel::new(config).unwrap();
    let mut engine = MicroEngine::new(model).unwrap();

    let start = Instant::now();
    for _ in 0..100 {
        engine.reset();
        let _ = engine.forward_one(42).unwrap();
    }
    let elapsed = start.elapsed();
    let host_us_per_token = elapsed.as_micros() as f64 / 100.0;

    println!();
    println!("Host (x86) performance:");
    println!("  Time per token: {:.1} us", host_us_per_token);
    println!("  ESP32/Host ratio: {:.1}x slower", estimated_us / host_us_per_token);

    // Validate reasonable performance
    assert!(estimated_tokens_per_sec > 10.0, "Should achieve >10 tokens/sec on ESP32");
    assert!(estimated_us < 100_000.0, "Should be <100ms per token");
}

/// Test edge cases
#[test]
fn test_edge_cases() {
    let config = ModelConfig::for_variant(Esp32Variant::Esp32);
    let model = TinyModel::new(config.clone()).unwrap();
    let mut engine = MicroEngine::new(model).unwrap();

    // Empty prompt
    let result = engine.generate(&[], &InferenceConfig::default());
    assert!(result.is_ok());

    // Single token prompt
    engine.reset();
    let result = engine.generate(&[1], &InferenceConfig::default());
    assert!(result.is_ok());

    // Max sequence length
    engine.reset();
    let long_prompt: Vec<u16> = (0..config.max_seq_len as u16).collect();
    let result = engine.generate(&long_prompt, &InferenceConfig { max_tokens: 1, ..Default::default() });
    // Should handle gracefully (may error or truncate)
}

/// Test determinism
#[test]
fn test_determinism() {
    // Use smallest variant to avoid stack overflow in tests
    let config = ModelConfig::for_variant(Esp32Variant::Esp32S2);

    // Same seed should produce same model - use Box for heap allocation
    let model1 = Box::new(TinyModel::new(config.clone()).unwrap());
    let model2 = Box::new(TinyModel::new(config.clone()).unwrap());

    // Same input should produce same output
    let mut engine1 = Box::new(MicroEngine::new(*model1).unwrap());
    let mut engine2 = Box::new(MicroEngine::new(*model2).unwrap());

    let gen_config = InferenceConfig {
        max_tokens: 3,
        greedy: true,
        seed: 42,
        ..Default::default()
    };

    let result1 = engine1.generate(&[1, 2, 3], &gen_config).unwrap();
    let result2 = engine2.generate(&[1, 2, 3], &gen_config).unwrap();

    assert_eq!(result1.tokens.as_slice(), result2.tokens.as_slice());
}
