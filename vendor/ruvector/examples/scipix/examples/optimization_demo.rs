//! Demonstration of performance optimizations in ruvector-scipix
//!
//! This example shows how to use various optimization features:
//! - SIMD operations for image processing
//! - Parallel batch processing
//! - Memory pooling
//! - Model quantization
//! - Dynamic batching

use ruvector_scipix::optimize::*;
use std::sync::Arc;
use std::time::Instant;

fn main() {
    println!("=== Ruvector-Scipix Optimization Demo ===\n");

    // 1. Feature Detection
    demo_feature_detection();

    // 2. SIMD Operations
    demo_simd_operations();

    // 3. Parallel Processing
    demo_parallel_processing();

    // 4. Memory Optimizations
    demo_memory_optimizations();

    // 5. Model Quantization
    demo_quantization();

    println!("\n=== Demo Complete ===");
}

fn demo_feature_detection() {
    println!("1. CPU Feature Detection");
    println!("------------------------");

    let features = detect_features();
    println!("AVX2 Support:    {}", if features.avx2 { "✓" } else { "✗" });
    println!(
        "AVX-512 Support: {}",
        if features.avx512f { "✓" } else { "✗" }
    );
    println!("NEON Support:    {}", if features.neon { "✓" } else { "✗" });
    println!(
        "SSE4.2 Support:  {}",
        if features.sse4_2 { "✓" } else { "✗" }
    );

    let opt_level = get_opt_level();
    println!("Optimization Level: {:?}", opt_level);
    println!();
}

fn demo_simd_operations() {
    println!("2. SIMD Operations");
    println!("------------------");

    // Create test image (512x512 RGBA)
    let size = 512;
    let rgba: Vec<u8> = (0..size * size * 4).map(|i| (i % 256) as u8).collect();
    let mut gray = vec![0u8; size * size];

    // Benchmark grayscale conversion
    let iterations = 100;

    let start = Instant::now();
    for _ in 0..iterations {
        simd::simd_grayscale(&rgba, &mut gray);
    }
    let simd_time = start.elapsed();

    println!("Grayscale conversion ({} iterations):", iterations);
    println!(
        "  SIMD: {:?} ({:.2} MP/s)",
        simd_time,
        (iterations as f64 * size as f64 * size as f64 / 1_000_000.0) / simd_time.as_secs_f64()
    );

    // Benchmark threshold
    let mut binary = vec![0u8; size * size];

    let start = Instant::now();
    for _ in 0..iterations {
        simd::simd_threshold(&gray, 128, &mut binary);
    }
    let threshold_time = start.elapsed();

    println!("Threshold operation ({} iterations):", iterations);
    println!(
        "  SIMD: {:?} ({:.2} MP/s)",
        threshold_time,
        (iterations as f64 * size as f64 * size as f64 / 1_000_000.0)
            / threshold_time.as_secs_f64()
    );

    // Benchmark normalization
    let mut data: Vec<f32> = (0..8192).map(|i| i as f32).collect();

    let start = Instant::now();
    for _ in 0..iterations {
        simd::simd_normalize(&mut data);
    }
    let normalize_time = start.elapsed();

    println!("Normalization ({} iterations):", iterations);
    println!("  SIMD: {:?}", normalize_time);
    println!();
}

fn demo_parallel_processing() {
    println!("3. Parallel Processing");
    println!("----------------------");

    let data: Vec<i32> = (0..10000).collect();

    // Sequential processing
    let start = Instant::now();
    let _seq_result: Vec<i32> = data.iter().map(|&x| expensive_computation(x)).collect();
    let seq_time = start.elapsed();

    // Parallel processing
    let start = Instant::now();
    let _par_result =
        parallel::parallel_map_chunked(data.clone(), 100, |x| expensive_computation(x));
    let par_time = start.elapsed();

    println!("Processing 10,000 items:");
    println!("  Sequential: {:?}", seq_time);
    println!("  Parallel:   {:?}", par_time);
    println!(
        "  Speedup:    {:.2}x",
        seq_time.as_secs_f64() / par_time.as_secs_f64()
    );

    let threads = parallel::optimal_thread_count();
    println!("  Using {} threads", threads);
    println!();
}

fn expensive_computation(x: i32) -> i32 {
    // Simulate some work
    (0..100).fold(x, |acc, i| acc.wrapping_add(i))
}

fn demo_memory_optimizations() {
    println!("4. Memory Optimizations");
    println!("-----------------------");

    let pools = memory::GlobalPools::get();

    // Benchmark buffer pool vs direct allocation
    let iterations = 10000;

    // Pooled allocation
    let start = Instant::now();
    for _ in 0..iterations {
        let mut buf = pools.acquire_small();
        buf.extend_from_slice(&[0u8; 512]);
    }
    let pooled_time = start.elapsed();

    // Direct allocation
    let start = Instant::now();
    for _ in 0..iterations {
        let mut buf = Vec::with_capacity(1024);
        buf.extend_from_slice(&[0u8; 512]);
    }
    let direct_time = start.elapsed();

    println!("Buffer allocation ({} iterations):", iterations);
    println!("  Pooled:  {:?}", pooled_time);
    println!("  Direct:  {:?}", direct_time);
    println!(
        "  Speedup: {:.2}x",
        direct_time.as_secs_f64() / pooled_time.as_secs_f64()
    );

    // Arena allocation
    let mut arena = memory::Arena::with_capacity(1024 * 1024);

    let start = Instant::now();
    for _ in 0..iterations {
        arena.reset();
        for _ in 0..10 {
            let _slice = arena.alloc(1024, 8);
        }
    }
    let arena_time = start.elapsed();

    println!(
        "\nArena allocation ({} iterations, 10 allocs each):",
        iterations
    );
    println!("  Time: {:?}", arena_time);
    println!();
}

fn demo_quantization() {
    println!("5. Model Quantization");
    println!("---------------------");

    // Create model weights
    let size = 100_000;
    let weights: Vec<f32> = (0..size)
        .map(|i| ((i as f32 / size as f32) * 2.0 - 1.0))
        .collect();

    println!(
        "Original model: {} weights ({:.2} MB)",
        weights.len(),
        (weights.len() * std::mem::size_of::<f32>()) as f64 / 1_048_576.0
    );

    // Quantize
    let start = Instant::now();
    let (quantized, params) = quantize::quantize_weights(&weights);
    let quant_time = start.elapsed();

    println!(
        "Quantized:      {} weights ({:.2} MB)",
        quantized.len(),
        (quantized.len() * std::mem::size_of::<i8>()) as f64 / 1_048_576.0
    );
    println!(
        "Compression:    {:.2}x",
        (weights.len() * std::mem::size_of::<f32>()) as f64
            / (quantized.len() * std::mem::size_of::<i8>()) as f64
    );
    println!("Quantization time: {:?}", quant_time);

    // Check quality
    let error = quantize::quantization_error(&weights, &quantized, params);
    let snr = quantize::sqnr(&weights, &quantized, params);

    println!("Quality metrics:");
    println!("  MSE:  {:.6}", error);
    println!("  SQNR: {:.2} dB", snr);

    // Benchmark dequantization
    let iterations = 100;
    let start = Instant::now();
    for _ in 0..iterations {
        let _restored = quantize::dequantize(&quantized, params);
    }
    let dequant_time = start.elapsed();

    println!(
        "Dequantization ({} iterations): {:?}",
        iterations, dequant_time
    );

    // Per-channel quantization
    let weights_2d: Vec<f32> = (0..10_000).map(|i| i as f32).collect();
    let shape = vec![100, 100]; // 100 channels, 100 values each

    let start = Instant::now();
    let per_channel = quantize::PerChannelQuant::from_f32(&weights_2d, shape);
    let per_channel_time = start.elapsed();

    println!("\nPer-channel quantization:");
    println!("  Channels: {}", per_channel.params.len());
    println!("  Time:     {:?}", per_channel_time);
    println!();
}

// Async batching demo (would need tokio runtime)
#[allow(dead_code)]
async fn demo_batching() {
    println!("6. Dynamic Batching");
    println!("-------------------");

    use batch::{BatchConfig, DynamicBatcher};

    let config = BatchConfig {
        max_batch_size: 32,
        max_wait_ms: 50,
        max_queue_size: 1000,
        preferred_batch_size: 16,
    };

    let batcher = Arc::new(DynamicBatcher::new(config, |items: Vec<i32>| {
        // Simulate batch processing
        items.into_iter().map(|x| Ok(x * 2)).collect()
    }));

    // Start processing loop
    let batcher_clone = batcher.clone();
    tokio::spawn(async move {
        batcher_clone.run().await;
    });

    // Add items
    let mut handles = vec![];
    for i in 0..100 {
        let batcher = batcher.clone();
        handles.push(tokio::spawn(async move { batcher.add(i).await }));
    }

    // Wait for results
    for handle in handles {
        let _ = handle.await;
    }

    let stats = batcher.stats().await;
    println!("Queue size: {}", stats.queue_size);
    println!("Max wait:   {:?}", stats.max_wait_time);

    batcher.shutdown().await;
}
