# Performance Optimizations Guide

This document describes the performance optimizations available in ruvector-scipix and how to use them effectively.

## Overview

The optimization module provides multiple strategies to improve performance:

1. **SIMD Operations**: Vectorized image processing (AVX2, AVX-512, NEON)
2. **Parallel Processing**: Multi-threaded execution using Rayon
3. **Memory Optimizations**: Object pooling, memory mapping, zero-copy views
4. **Model Quantization**: INT8 quantization for reduced memory and faster inference
5. **Dynamic Batching**: Intelligent batching for throughput optimization

## Feature Detection

The library automatically detects CPU capabilities at runtime:

```rust
use ruvector_scipix::optimize::{detect_features, get_features};

// Detect CPU features
let features = detect_features();
println!("AVX2: {}", features.avx2);
println!("AVX-512: {}", features.avx512f);
println!("NEON: {}", features.neon);
println!("SSE4.2: {}", features.sse4_2);
```

## SIMD Operations

### Grayscale Conversion

Convert RGBA images to grayscale using SIMD:

```rust
use ruvector_scipix::optimize::simd;

let rgba: Vec<u8> = /* your RGBA data */;
let mut gray = vec![0u8; rgba.len() / 4];

// Automatically uses best SIMD implementation available
simd::simd_grayscale(&rgba, &mut gray);
```

**Performance**: Up to 4x faster than scalar implementation on AVX2 systems.

### Threshold Operation

Fast binary thresholding:

```rust
simd::simd_threshold(&gray, 128, &mut binary);
```

**Performance**: Up to 8x faster on AVX2 systems.

### Normalization

Fast tensor normalization for model inputs:

```rust
let mut tensor_data: Vec<f32> = /* your data */;
simd::simd_normalize(&mut tensor_data);
```

**Performance**: Up to 3x faster on AVX2 systems.

## Parallel Processing

### Parallel Image Preprocessing

Process multiple images in parallel:

```rust
use ruvector_scipix::optimize::parallel;
use image::DynamicImage;

let images: Vec<DynamicImage> = /* your images */;

let processed = parallel::parallel_preprocess(images, |img| {
    // Your preprocessing function
    preprocess_image(img)
});
```

### Pipeline Execution

Create processing pipelines with parallel stages:

```rust
use ruvector_scipix::optimize::parallel::Pipeline3;

let pipeline = Pipeline3::new(
    |img| preprocess(img),
    |img| detect_regions(img),
    |regions| recognize_text(regions),
);

let results = pipeline.execute_batch(images);
```

### Async Parallel Execution

Execute async operations with concurrency limits:

```rust
use ruvector_scipix::optimize::parallel::AsyncParallelExecutor;

let executor = AsyncParallelExecutor::new(4); // Max 4 concurrent

let results = executor.execute(tasks, |task| async move {
    process_async(task).await
}).await;
```

## Memory Optimizations

### Buffer Pooling

Reuse buffers to reduce allocations:

```rust
use ruvector_scipix::optimize::memory::{BufferPool, GlobalPools};

// Use global pools
let pools = GlobalPools::get();
let mut buffer = pools.acquire_large(); // 1MB buffer
buffer.extend_from_slice(&data);
// Buffer automatically returns to pool when dropped

// Or create custom pool
let pool = BufferPool::new(
    || Vec::with_capacity(1024),
    initial_size: 10,
    max_size: 100
);
```

**Benefits**: Reduces allocation overhead, improves cache locality.

### Memory-Mapped Models

Load large models without copying to memory:

```rust
use ruvector_scipix::optimize::memory::MmapModel;

let model = MmapModel::from_file("model.bin")?;
let data = model.as_slice(); // Zero-copy access
```

**Benefits**: Faster loading, lower memory usage, shared across processes.

### Zero-Copy Image Views

Work with image data without copying:

```rust
use ruvector_scipix::optimize::memory::ImageView;

let view = ImageView::new(&data, width, height, channels)?;
let pixel = view.pixel(x, y);

// Create subview without copying
let roi = view.subview(x, y, width, height)?;
```

### Arena Allocation

Fast temporary allocations:

```rust
use ruvector_scipix::optimize::memory::Arena;

let mut arena = Arena::with_capacity(1024 * 1024);

for _ in 0..iterations {
    let buffer = arena.alloc(size, alignment);
    // Use buffer...
    arena.reset(); // Reuse capacity
}
```

## Model Quantization

### Basic Quantization

Quantize f32 weights to INT8:

```rust
use ruvector_scipix::optimize::quantize;

let weights: Vec<f32> = /* your model weights */;
let (quantized, params) = quantize::quantize_weights(&weights);

// Later, dequantize for inference
let restored = quantize::dequantize(&quantized, params);
```

**Benefits**: 4x memory reduction, faster inference on some hardware.

### Quantized Tensors

Work with quantized tensor representations:

```rust
use ruvector_scipix::optimize::quantize::QuantizedTensor;

let tensor = QuantizedTensor::from_f32(&data, vec![batch, channels, height, width]);
println!("Compression ratio: {:.2}x", tensor.compression_ratio());

// Dequantize when needed
let f32_data = tensor.to_f32();
```

### Per-Channel Quantization

Better accuracy for convolutional/linear layers:

```rust
use ruvector_scipix::optimize::quantize::PerChannelQuant;

// For weight tensor [out_channels, in_channels, ...]
let quant = PerChannelQuant::from_f32(&weights, shape);

// Each output channel has its own scale/zero-point
```

### Quality Metrics

Measure quantization quality:

```rust
use ruvector_scipix::optimize::quantize::{quantization_error, sqnr};

let (quantized, params) = quantize::quantize_weights(&original);

let mse = quantization_error(&original, &quantized, params);
let signal_noise_ratio = sqnr(&original, &quantized, params);

println!("MSE: {:.6}, SQNR: {:.2} dB", mse, signal_noise_ratio);
```

## Dynamic Batching

### Basic Batching

Automatically batch requests for better throughput:

```rust
use ruvector_scipix::optimize::batch::{DynamicBatcher, BatchConfig};

let config = BatchConfig {
    max_batch_size: 32,
    max_wait_ms: 50,
    max_queue_size: 1000,
    preferred_batch_size: 16,
};

let batcher = Arc::new(DynamicBatcher::new(config, |items: Vec<Image>| {
    process_batch(items) // Your batch processing logic
}));

// Start processing loop
tokio::spawn({
    let batcher = batcher.clone();
    async move { batcher.run().await }
});

// Add items
let result = batcher.add(image).await?;
```

### Adaptive Batching

Automatically adjust batch size based on latency:

```rust
use ruvector_scipix::optimize::batch::AdaptiveBatcher;
use std::time::Duration;

let batcher = Arc::new(AdaptiveBatcher::new(
    config,
    Duration::from_millis(100), // Target latency
    processor,
));

// Batch size adapts to maintain target latency
```

## Optimization Levels

Control which optimizations are enabled:

```rust
use ruvector_scipix::optimize::{OptLevel, set_opt_level};

// Set optimization level at startup
set_opt_level(OptLevel::Full); // All optimizations

// Available levels:
// - OptLevel::None:     No optimizations
// - OptLevel::Simd:     SIMD only
// - OptLevel::Parallel: SIMD + parallel
// - OptLevel::Full:     All optimizations (default)
```

## Benchmarking

Run benchmarks to compare optimized vs non-optimized implementations:

```bash
# Run all optimization benchmarks
cargo bench --bench optimization_bench

# Run specific benchmark group
cargo bench --bench optimization_bench -- grayscale

# Generate detailed reports
cargo bench --bench optimization_bench -- --verbose
```

### Expected Performance Improvements

Based on benchmarks on modern x86_64 systems with AVX2:

| Operation | Speedup | Notes |
|-----------|---------|-------|
| Grayscale conversion | 3-4x | AVX2 vs scalar |
| Threshold | 6-8x | AVX2 vs scalar |
| Normalization | 2-3x | AVX2 vs scalar |
| Parallel preprocessing (8 cores) | 6-7x | vs sequential |
| Buffer pooling | 2-3x | vs direct allocation |
| Quantization | 4x memory | INT8 vs FP32 |

## Best Practices

1. **Enable optimizations by default**: Use the `optimize` feature in production
2. **Profile first**: Use benchmarks to identify bottlenecks
3. **Use appropriate batch sizes**: Larger batches = better throughput, higher latency
4. **Pool buffers for hot paths**: Reduces allocation overhead significantly
5. **Quantize models**: 4x memory reduction with minimal accuracy loss
6. **Match parallelism to workload**: Use thread count ≤ CPU cores

## Platform-Specific Notes

### x86_64

- **AVX2**: Widely available on modern CPUs (2013+)
- **AVX-512**: Available on newer server CPUs, provides marginal improvements
- Best performance on CPUs with good SIMD execution units

### ARM (AArch64)

- **NEON**: Available on all ARMv8+ CPUs
- Good SIMD performance, especially on Apple Silicon
- Some operations may be faster with scalar code due to different execution units

### WebAssembly

- SIMD support is limited and experimental
- Optimizations gracefully degrade to scalar implementations
- Focus on algorithmic optimizations and caching

## Troubleshooting

### Low SIMD Performance

If SIMD optimizations are not providing expected speedup:

1. Check CPU features: `cargo run -- detect-features`
2. Ensure data is properly aligned (16-byte alignment for SIMD)
3. Profile to ensure SIMD code paths are being used
4. Try different optimization levels

### High Memory Usage

If memory usage is too high:

1. Enable buffer pooling for frequently allocated buffers
2. Use memory-mapped models instead of loading into RAM
3. Enable model quantization
4. Reduce batch sizes

### Thread Contention

If parallel performance is poor:

1. Reduce thread count: `set_thread_count(cores - 1)`
2. Use chunked parallel processing for better load balancing
3. Avoid fine-grained parallelism (prefer coarser chunks)
4. Profile mutex/lock contention

## Integration Example

Complete example using multiple optimizations:

```rust
use ruvector_scipix::optimize::*;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    // Set optimization level
    set_opt_level(OptLevel::Full);

    // Detect features
    let features = detect_features();
    println!("Features: {:?}", features);

    // Create buffer pools
    let pools = memory::GlobalPools::get();

    // Create adaptive batcher
    let batcher = Arc::new(batch::AdaptiveBatcher::new(
        batch::BatchConfig::default(),
        Duration::from_millis(100),
        |images| process_images(images),
    ));

    // Start batcher
    let batcher_clone = batcher.clone();
    tokio::spawn(async move { batcher_clone.run().await });

    // Process images
    let result = batcher.add(image).await?;

    Ok(())
}

fn process_images(images: Vec<Image>) -> Vec<Result<Output, String>> {
    // Use parallel processing
    parallel::parallel_map_chunked(images, 8, |img| {
        // Get pooled buffer
        let mut buffer = memory::GlobalPools::get().acquire_large();

        // Use SIMD operations
        let mut gray = vec![0u8; img.width() * img.height()];
        simd::simd_grayscale(img.as_rgba8(), &mut gray);

        // Process...
        Ok(output)
    })
}
```

## Future Optimizations

Planned improvements:

- GPU acceleration using wgpu
- Custom ONNX runtime integration
- Advanced quantization (INT4, mixed precision)
- Streaming processing for video
- Distributed inference

## References

- [SIMD in Rust](https://doc.rust-lang.org/std/arch/)
- [Rayon Parallel Processing](https://docs.rs/rayon/)
- [Quantization Techniques](https://arxiv.org/abs/2103.13630)
- Benchmark results: See `benches/optimization_bench.rs`
