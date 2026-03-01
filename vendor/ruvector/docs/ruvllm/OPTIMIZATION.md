# RuvLLM Optimization Guide (v2.0.0)

This guide covers performance optimization strategies for RuvLLM, including SONA learning loops, batch sizing, KV cache management, and hardware-specific tuning.

## v2.0.0 Performance Highlights

| Feature | Improvement | Notes |
|---------|-------------|-------|
| Multi-threaded GEMM | 12.7x speedup | Rayon on M4 Pro 10-core |
| Flash Attention 2 | +10% throughput | Auto block sizing |
| Quantized Inference | 4-8x memory | INT8/INT4/Q4_K |
| Metal GPU | 3x speedup | simdgroup_matrix |
| Memory Pool | Zero-alloc | Arena allocator |

## Performance Overview

### Key Metrics

| Metric | Target (M4 Pro) | Achieved (v2.0.0) | Description |
|--------|-----------------|-------------------|-------------|
| Prefill | >2000 tok/s | 3500 tok/s | Processing input tokens |
| Decode | >80 tok/s | 120 tok/s | Generating output tokens |
| TTFT | <50ms | 35ms | Time to first token |
| Memory | <8GB for 7B | 3.4GB (Q4K) | Peak memory usage |
| MicroLoRA | <1ms | 8.56us | Per-request adaptation |

### Architecture Impact

```
┌─────────────────────────────────────────────────────────┐
│                    Optimization Layers                   │
├─────────────────────────────────────────────────────────┤
│  SONA Learning      │ Real-time adaptation, routing     │
├─────────────────────────────────────────────────────────┤
│  Attention          │ Flash, Paged, GQA - 2-4x speedup │
├─────────────────────────────────────────────────────────┤
│  KV Cache           │ Two-tier, quantized - 4x memory  │
├─────────────────────────────────────────────────────────┤
│  Quantization       │ Q4K, Q8 - 4-8x smaller           │
├─────────────────────────────────────────────────────────┤
│  SIMD/GPU           │ NEON, Metal - hardware accel     │
└─────────────────────────────────────────────────────────┘
```

## SONA Learning Optimization

### Instant Loop Tuning

The instant loop runs per-request with <1ms target latency.

```rust
let config = SonaLlmConfig {
    // Learning rate for instant updates
    // Higher = faster adaptation, more variance
    // Lower = slower adaptation, more stable
    instant_lr: 0.01,

    // Quality threshold - skip low-quality samples
    training: TrainingConfig {
        quality_threshold: 0.5,  // 0.0-1.0
        ..Default::default()
    },
    ..Default::default()
};
```

**Tuning Guidelines:**

| Use Case | instant_lr | quality_threshold |
|----------|------------|-------------------|
| High variance tasks | 0.005 | 0.7 |
| Stable domains | 0.02 | 0.3 |
| User personalization | 0.01 | 0.5 |

### Background Loop Tuning

Consolidates patterns without blocking inference.

```rust
let config = SonaLlmConfig {
    // How often to run (milliseconds)
    background_interval_ms: 100,

    // Minimum samples before consolidation
    background_min_samples: 10,

    // Maximum pending (triggers forced consolidation)
    max_pending_samples: 1000,

    // Consolidation strategy
    consolidation_strategy: ConsolidationStrategy::EwcMerge,
    ..Default::default()
};
```

**Tuning Guidelines:**

| Priority | interval_ms | min_samples | Strategy |
|----------|-------------|-------------|----------|
| Latency | 200 | 20 | Average |
| Quality | 50 | 5 | EwcMerge |
| Memory | 100 | 50 | BestOnly |

### Deep Loop Optimization

Triggered periodically for full optimization.

```rust
let config = SonaLlmConfig {
    // Accumulated quality threshold to trigger
    deep_trigger_threshold: 100.0,
    ..Default::default()
};

// Manual trigger for scheduled optimization
if sona.should_trigger_deep() || is_scheduled_time() {
    let samples = collect_high_quality_samples();
    let result = sona.deep_optimize(&samples);

    // Log improvement
    println!("Deep optimization: quality delta = {:.3}", result.quality_delta);
}
```

## Batch Size Optimization

### Dynamic Batching

```rust
// Optimal batch sizes vary by operation
struct BatchConfig {
    prefill_batch: usize,   // Process multiple prompts together
    decode_batch: usize,    // Parallel token generation
    lora_batch: usize,      // LoRA adaptation batch
}

impl BatchConfig {
    fn for_memory(available_gb: f32) -> Self {
        match available_gb {
            x if x < 8.0 => Self {
                prefill_batch: 1,
                decode_batch: 4,
                lora_batch: 16,
            },
            x if x < 16.0 => Self {
                prefill_batch: 2,
                decode_batch: 8,
                lora_batch: 32,
            },
            _ => Self {
                prefill_batch: 4,
                decode_batch: 16,
                lora_batch: 64,
            },
        }
    }
}
```

### Batch Size Impact

| Batch Size | Throughput | Latency | Memory |
|------------|------------|---------|--------|
| 1 | Low | Lowest | Lowest |
| 4 | Medium | Low | Medium |
| 8 | High | Medium | High |
| 16+ | Highest | Higher | Highest |

**Rule of thumb:** Increase batch size until memory pressure or latency constraints are hit.

## KV Cache Optimization

### Two-Tier Configuration

```rust
let config = KvCacheConfig {
    // Tokens in high-precision tail
    // More = better attention quality for recent context
    // Less = less memory usage
    tail_length: 256,

    // Tail precision (FP16 recommended)
    tail_precision: Precision::FP16,

    // Store precision (Q4 for 4x compression)
    store_precision: Precision::Q4,

    // Maximum context length
    max_tokens: 4096,

    // KV heads (depends on model architecture)
    num_kv_heads: 8,
    head_dim: 128,

    // Batch size for migration (affects latency spikes)
    migration_batch: 64,
};
```

### Memory Calculation

```
KV Cache Memory = num_layers * 2 * max_tokens * num_kv_heads * head_dim * bytes_per_element

Example (Qwen2.5-7B with 4096 context):
- Layers: 32
- KV heads: 8
- Head dim: 128
- FP16 tail (256 tokens): 32 * 2 * 256 * 8 * 128 * 2 = 33.5 MB
- Q4 store (3840 tokens): 32 * 2 * 3840 * 8 * 128 * 0.5 = 125.8 MB
- Total: ~160 MB (vs ~672 MB for full FP16)
```

### Cache Strategies by Use Case

| Use Case | tail_length | store_precision | max_tokens |
|----------|-------------|-----------------|------------|
| Chat (short) | 128 | Q8 | 2048 |
| Chat (long) | 256 | Q4 | 8192 |
| Document QA | 512 | Q4 | 16384 |
| Code completion | 128 | Q8 | 4096 |

## Attention Optimization

### Grouped-Query Attention (GQA)

```rust
let config = AttentionConfig {
    num_heads: 32,      // Query heads
    num_kv_heads: 8,    // KV heads (4:1 ratio)
    head_dim: 128,
    causal: true,
    ..Default::default()
};

// GQA ratio determines memory savings
// 4:1 = ~4x KV cache reduction
// 8:1 = ~8x KV cache reduction
assert_eq!(config.gqa_ratio(), 4);
```

### Flash Attention Optimization

```rust
// Flash Attention is memory-efficient but has setup overhead
// Best for: longer sequences (>256 tokens)

// For short sequences, standard attention may be faster
let use_flash = sequence_length > 256;

if use_flash {
    let output = flash_attention_neon(&query, &key, &value, scale, causal);
} else {
    let output = standard_attention(&query, &key, &value, scale, causal);
}
```

### Paged Attention for Inference

```rust
// Paged attention enables non-contiguous KV cache
// Best for: long-running inference with variable context

let mut cache = PagedKvCache::new(
    16,     // block_size: tokens per block
    8,      // num_kv_heads
    128,    // head_dim
);

// Append incrementally
for token in tokens {
    let (k, v) = compute_kv(token)?;
    cache.append(&k, &v);
}

// Efficient attention over paged cache
let output = paged_attention_neon(&query, &cache, &block_tables, scale);
```

## Quantization Optimization

### Model Quantization

| Precision | Memory | Quality | Speed |
|-----------|--------|---------|-------|
| FP32 | 4x | Best | Slowest |
| FP16 | 2x | Excellent | Fast |
| Q8 | 1x | Very Good | Faster |
| Q4K | 0.5x | Good | Fastest |
| Q4 | 0.5x | Acceptable | Fastest |

**Recommendations:**

```rust
// High quality (16GB+ RAM)
let config = ModelConfig {
    quantization: Precision::Q8,
    ..Default::default()
};

// Balanced (8-16GB RAM)
let config = ModelConfig {
    quantization: Precision::Q4K,  // K-quant preserves quality
    ..Default::default()
};

// Memory constrained (<8GB RAM)
let config = ModelConfig {
    quantization: Precision::Q4,
    ..Default::default()
};
```

### KV Cache Quantization

```rust
// Hybrid quantization: recent tokens in high precision
let config = KvCacheConfig {
    tail_length: 256,           // Recent: FP16
    tail_precision: Precision::FP16,
    store_precision: Precision::Q4,  // Older: Q4
    ..Default::default()
};

// Quality impact by position
// Position 0-256 (tail): Full quality
// Position 256+: ~95% quality with Q4
```

## Hardware-Specific Optimization

### Apple Silicon (M1/M2/M3/M4)

```rust
// Metal backend for GPU acceleration
let backend = CandleBackend::with_device(DeviceType::Metal)?;

// Optimize for unified memory
let config = ModelConfig {
    // Unified memory = larger KV cache possible
    kv_cache_config: KvCacheConfig {
        max_tokens: 8192,  // Can be larger on M-series
        ..Default::default()
    },
    ..Default::default()
};
```

**M4 Pro Specific:**
- Use `metal` feature for GPU acceleration
- NEON SIMD enabled by default
- Leverage unified memory for larger context

### NVIDIA GPUs

```rust
// CUDA backend
let backend = CandleBackend::with_device(DeviceType::Cuda(0))?;

// Optimize for separate VRAM
let config = ModelConfig {
    kv_cache_config: KvCacheConfig {
        // Conservative: VRAM is limited
        max_tokens: 4096,
        ..Default::default()
    },
    ..Default::default()
};
```

### CPU Fallback

```rust
// CPU with SIMD optimization
let backend = CandleBackend::with_device(DeviceType::Cpu)?;

// Reduce memory pressure
let config = ModelConfig {
    quantization: Precision::Q4,
    kv_cache_config: KvCacheConfig {
        tail_length: 128,
        max_tokens: 2048,
        ..Default::default()
    },
    ..Default::default()
};
```

## Real-Time Optimization

### Adaptive Optimization

```rust
use ruvllm::optimization::{RealTimeOptimizer, OptimizerConfig};

let optimizer = RealTimeOptimizer::new(OptimizerConfig {
    target_latency_ms: 100.0,
    min_throughput: 50.0,  // tokens/sec
    memory_threshold: 0.9,  // 90% of available
});

// Optimizer adjusts parameters in real-time
loop {
    let metrics = backend.get_metrics();
    let adjustments = optimizer.recommend(&metrics);

    if adjustments.reduce_batch_size {
        config.batch_size -= 1;
    }
    if adjustments.increase_quantization {
        config.kv_cache_config.store_precision = Precision::Q4;
    }
}
```

### Latency Monitoring

```rust
// Track latency components
struct LatencyBreakdown {
    tokenization_us: u64,
    prefill_us: u64,
    decode_us: u64,
    sampling_us: u64,
    lora_us: u64,
}

impl LatencyBreakdown {
    fn total_ms(&self) -> f64 {
        (self.tokenization_us + self.prefill_us +
         self.decode_us + self.sampling_us + self.lora_us) as f64 / 1000.0
    }

    fn bottleneck(&self) -> &str {
        let max = [
            (self.tokenization_us, "tokenization"),
            (self.prefill_us, "prefill"),
            (self.decode_us, "decode"),
            (self.sampling_us, "sampling"),
            (self.lora_us, "lora"),
        ].into_iter().max_by_key(|(v, _)| *v).unwrap();
        max.1
    }
}
```

## Benchmarking

### Running Benchmarks

```bash
# All benchmarks
cargo bench

# Specific benchmarks
cargo bench --bench attention_bench
cargo bench --bench lora_bench
cargo bench --bench e2e_bench

# With specific features
cargo bench --features metal
cargo bench --features cuda
```

### Custom Benchmarks

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ruvllm::kernels::attention::flash_attention_neon;

fn bench_attention(c: &mut Criterion) {
    let query = vec![0.1f32; 128];
    let key = vec![0.1f32; 512 * 128];
    let value = vec![0.1f32; 512 * 128];
    let scale = 1.0 / 128.0_f32.sqrt();

    c.bench_function("flash_attention_512", |b| {
        b.iter(|| {
            flash_attention_neon(
                black_box(&query),
                black_box(&key),
                black_box(&value),
                scale,
                true,
            )
        })
    });
}

criterion_group!(benches, bench_attention);
criterion_main!(benches);
```

## Optimization Checklist

### Before Deployment

- [ ] Choose appropriate quantization (Q4K for most cases)
- [ ] Configure KV cache for expected context length
- [ ] Enable GQA if model supports it
- [ ] Set appropriate batch sizes for memory
- [ ] Configure SONA learning rates
- [ ] Test with representative workloads

### Monitoring

- [ ] Track prefill and decode throughput
- [ ] Monitor memory usage over time
- [ ] Log KV cache hit rates
- [ ] Track SONA learning metrics
- [ ] Alert on latency spikes

### Troubleshooting

| Symptom | Likely Cause | Solution |
|---------|--------------|----------|
| High latency | Batch too large | Reduce batch size |
| OOM errors | KV cache too large | Reduce max_tokens or use Q4 |
| Quality degradation | Over-quantization | Use Q8 instead of Q4 |
| Slow adaptation | Learning rate too low | Increase instant_lr |
| Forgetting | EWC lambda too low | Increase ewc_lambda |
