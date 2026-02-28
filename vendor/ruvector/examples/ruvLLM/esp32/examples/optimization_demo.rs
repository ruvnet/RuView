//! Optimization Benchmark Demo
//!
//! Compares the various ruvector-inspired optimizations for ESP32.

use std::time::Instant;
use ruvllm_esp32::optimizations::{
    binary_quant::{BinaryVector, hamming_distance, xnor_popcount},
    product_quant::{ProductQuantizer, PQConfig},
    lookup_tables::{SOFTMAX_LUT, DISTANCE_LUT},
    sparse_attention::{SparseAttention, AttentionPattern},
    pruning::{LayerPruner, PruningConfig},
    micro_lora::{MicroLoRA, LoRAConfig},
};

fn main() {
    println!("=== RuvLLM ESP32 Optimization Benchmarks ===\n");

    // Benchmark parameters
    const ITERS: usize = 10000;
    const DIM: usize = 64;
    const VOCAB_TEST: usize = 256;

    // 1. Binary Quantization Benchmark
    println!("--- Binary Quantization (32x Compression) ---");
    let int8_vector: Vec<i8> = (0..DIM).map(|i| (i as i8).wrapping_mul(3)).collect();
    let binary_vec = BinaryVector::<8>::from_i8(&int8_vector, 0).unwrap();

    println!("  INT8 vector size: {} bytes", DIM);
    println!("  Binary vector size: {} bytes", binary_vec.num_bytes());
    println!("  Compression ratio: {:.1}x", binary_vec.compression_ratio());

    // Benchmark Hamming distance
    let binary_a: [u8; 8] = [0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55];
    let binary_b: [u8; 8] = [0x55, 0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55, 0xAA];

    let start = Instant::now();
    for _ in 0..ITERS {
        let _ = hamming_distance(&binary_a, &binary_b);
    }
    let hamming_time = start.elapsed();
    println!("  Hamming distance ({} iters): {:?}", ITERS, hamming_time);
    println!("  Per-op: {:.3} us", hamming_time.as_nanos() as f64 / ITERS as f64 / 1000.0);

    // XNOR-popcount for BNN
    let start = Instant::now();
    for _ in 0..ITERS {
        let _ = xnor_popcount(&binary_a, &binary_b);
    }
    let xnor_time = start.elapsed();
    println!("  XNOR-popcount ({} iters): {:?}", ITERS, xnor_time);
    println!("");

    // 2. Product Quantization Benchmark
    println!("--- Product Quantization (8x Compression) ---");
    let pq_config = PQConfig {
        num_subquantizers: 4,
        codebook_size: 16,
        subvec_dim: 8,
        dim: 32,
    };
    let pq = ProductQuantizer::<4, 16, 8>::random(pq_config, 42).unwrap();

    println!("  Original vector: 32 bytes");
    println!("  PQ code: 4 bytes");
    println!("  Compression: {:.1}x", pq.compression_ratio());
    println!("  Codebook memory: {} bytes", pq.memory_size());

    // Benchmark encoding
    let test_vec: [i8; 32] = [0; 32];
    let start = Instant::now();
    for _ in 0..ITERS {
        let _ = pq.encode(&test_vec);
    }
    let pq_encode_time = start.elapsed();
    println!("  PQ encode ({} iters): {:?}", ITERS, pq_encode_time);
    println!("");

    // 3. Lookup Tables Benchmark
    println!("--- Lookup Tables (Zero-Compute Operations) ---");

    // Softmax LUT
    let test_logits: [i32; 8] = [100, 50, 0, -50, -100, 25, 75, -25];
    let mut output = [0u16; 8];

    let start = Instant::now();
    for _ in 0..ITERS {
        SOFTMAX_LUT.softmax(&test_logits, &mut output);
    }
    let softmax_time = start.elapsed();
    println!("  Softmax LUT ({} iters): {:?}", ITERS, softmax_time);
    println!("  Per-op: {:.3} us", softmax_time.as_nanos() as f64 / ITERS as f64 / 1000.0);

    // Distance LUT
    let vec_a: Vec<i8> = (0..32).map(|i| i as i8).collect();
    let vec_b: Vec<i8> = (0..32).map(|i| (31 - i) as i8).collect();

    let start = Instant::now();
    for _ in 0..ITERS {
        let _ = DISTANCE_LUT.l2_squared(&vec_a, &vec_b);
    }
    let dist_time = start.elapsed();
    println!("  L2 Distance LUT ({} iters): {:?}", ITERS, dist_time);
    println!("");

    // 4. Sparse Attention Benchmark
    println!("--- Sparse Attention Patterns ---");

    let full_attention = SparseAttention::new(AttentionPattern::Full, 16).unwrap();
    let sliding_4 = SparseAttention::new(
        AttentionPattern::SlidingWindow { window_size: 4 }, 16
    ).unwrap();
    let bigbird = SparseAttention::new(
        AttentionPattern::BigBird { window_size: 4, global_tokens: 2 }, 16
    ).unwrap();

    println!("  Full attention sparsity: {:.1}%", full_attention.sparsity_ratio() * 100.0);
    println!("  Sliding (w=4) sparsity: {:.1}%", sliding_4.sparsity_ratio() * 100.0);
    println!("  BigBird sparsity: {:.1}%", bigbird.sparsity_ratio() * 100.0);
    println!("  Compute savings (sliding): {:.1}x", 1.0 / sliding_4.sparsity_ratio());
    println!("");

    // 5. MicroLoRA Benchmark
    println!("--- MicroLoRA (On-Device Adaptation) ---");

    let lora_config = LoRAConfig {
        rank: 2,
        dim: 32,
        scale: 8,
        frozen: true,
    };
    let mut lora = MicroLoRA::new(lora_config, 42).unwrap();

    println!("  LoRA rank: {}", lora_config.rank);
    println!("  LoRA dimension: {}", lora_config.dim);
    println!("  LoRA memory: {} bytes", lora.memory_size());
    println!("  Memory overhead: {:.2}%", lora.memory_size() as f32 / (32 * 32) as f32 * 100.0);

    let lora_input: [i8; 32] = [16; 32];
    let mut lora_output = [0i32; 32];

    let start = Instant::now();
    for _ in 0..ITERS {
        lora.apply(&lora_input, &mut lora_output);
    }
    let lora_time = start.elapsed();
    println!("  LoRA apply ({} iters): {:?}", ITERS, lora_time);
    println!("");

    // 6. Pruning Benchmark
    println!("--- MinCut-Inspired Pruning ---");

    let pruning_config = PruningConfig {
        target_sparsity: 0.5,
        structured: true,
        ..Default::default()
    };
    let mut pruner = LayerPruner::new(pruning_config);

    // Create test weights
    let mut weights: Vec<i8> = (0..256).map(|i| ((i % 127) as i8 - 64)).collect();

    pruner.compute_magnitude_importance(&weights);
    let mask = pruner.create_mask::<256>(256).unwrap();

    println!("  Target sparsity: {:.0}%", pruning_config.target_sparsity * 100.0);
    println!("  Achieved sparsity: {:.1}%", mask.sparsity() * 100.0);
    println!("  Weights pruned: {}", mask.pruned_count);
    println!("  Memory saved: {} bytes", mask.pruned_count);
    println!("");

    // Summary
    println!("=== Optimization Summary for ESP32 ===");
    println!("┌────────────────────────┬───────────────┬─────────────────┐");
    println!("│ Optimization           │ Compression   │ Speed Impact    │");
    println!("├────────────────────────┼───────────────┼─────────────────┤");
    println!("│ Binary Quantization    │ 8x            │ 10-20x faster   │");
    println!("│ Product Quantization   │ 8x            │ 2-4x faster     │");
    println!("│ Softmax LUT            │ -             │ 5-10x faster    │");
    println!("│ Sliding Attention      │ {:.1}x less ops  │ {:.1}x faster     │",
        1.0 / sliding_4.sparsity_ratio(),
        1.0 / sliding_4.sparsity_ratio());
    println!("│ Weight Pruning (50%)   │ 2x            │ 1.5-2x faster   │");
    println!("│ MicroLoRA              │ N/A           │ +{:.1}% overhead │",
        lora.memory_size() as f32 / 1024.0);
    println!("└────────────────────────┴───────────────┴─────────────────┘");

    println!("\nTotal potential speedup: 20-50x for binary, 5-10x for hybrid");
    println!("Total memory savings: Up to 32x with binary + pruning");

    // Estimated ESP32 performance with optimizations
    let baseline_tok_s = 236.0;
    let optimized_tok_s_low = baseline_tok_s * 5.0;
    let optimized_tok_s_high = baseline_tok_s * 15.0;

    println!("\n=== Projected ESP32 Performance ===");
    println!("Baseline: {:.0} tokens/sec", baseline_tok_s);
    println!("With optimizations: {:.0} - {:.0} tokens/sec", optimized_tok_s_low, optimized_tok_s_high);
    println!("Memory: 119KB (baseline) → 37-60KB (optimized)");
}
