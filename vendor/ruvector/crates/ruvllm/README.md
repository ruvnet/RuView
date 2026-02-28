# RuvLLM

[![Crates.io](https://img.shields.io/crates/v/ruvllm.svg)](https://crates.io/crates/ruvllm)
[![docs.rs](https://docs.rs/ruvllm/badge.svg)](https://docs.rs/ruvllm)
[![npm](https://img.shields.io/npm/v/@ruvector/ruvllm.svg)](https://www.npmjs.com/package/@ruvector/ruvllm)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

**The local LLM inference engine that learns from every request -- Metal, CUDA, WebGPU, no cloud APIs.**

```bash
cargo add ruvllm
```

RuvLLM loads GGUF models and runs them on your hardware with full acceleration -- Apple Silicon, NVIDIA GPUs, WebAssembly, whatever you have. Unlike other local inference tools, it gets smarter over time: SONA (Self-Optimizing Neural Architecture) watches how you use it and adapts automatically, so responses improve without manual tuning. It's part of [RuVector](https://github.com/ruvnet/ruvector), the self-learning vector database with graph intelligence.

| | RuvLLM | OpenAI API | llama.cpp | Ollama | vLLM |
|---|---|---|---|---|---|
| **Cost** | Free after hardware | Per-token billing | Free | Free | Free |
| **Privacy** | Data stays on your machine | Sent to third party | Local | Local | Local |
| **Self-learning** | SONA adapts automatically | Static | Static | Static | Static |
| **Per-request tuning** | MicroLoRA in <1 ms | Not available | Not available | Not available | Not available |
| **Hardware support** | Metal, CUDA, ANE, WebGPU, CPU | N/A | Metal, CUDA, CPU | Metal, CUDA, CPU | CUDA only |
| **WASM / Browser** | Yes (5.5 KB runtime) | Via network call | Not available | Not available | Not available |
| **Vector DB integration** | Built-in (RuVector) | Separate service | Not available | Not available | Not available |
| **Speculative decoding** | Yes | N/A | Yes | No | Yes |
| **Continuous batching** | Yes | N/A | No | No | Yes |
| **Production serving** | mistral-rs backend | N/A | Server mode | Server mode | Native |

## Key Features

| Feature | What It Does | Why It Matters |
|---------|-------------|----------------|
| **SONA three-tier learning** | Adapts to your queries at three speeds: instant (<1 ms), background (~100 ms), deep (minutes) | Responses improve automatically without manual retraining |
| **Metal + CUDA + ANE** | Hardware-accelerated inference across Apple Silicon, NVIDIA GPUs, and Apple Neural Engine | Get the most out of whatever hardware you have |
| **Flash Attention 2** | Memory-efficient attention with O(N) complexity and online softmax | Longer contexts with less memory |
| **GGUF memory mapping** | Memory-mapped model loading with quantization (Q4K, Q8, FP16) | Load large models fast, use 4-8x less RAM |
| **Speculative decoding** | Draft model generates candidates, target model verifies in parallel | 2-3x faster text generation |
| **Continuous batching** | Dynamic batch scheduling for concurrent requests | 2-3x throughput improvement for serving |
| **MicroLoRA** | Per-request fine-tuning with rank 1-2 adapters | Personalize responses in <1 ms without full retraining |
| **HuggingFace Hub** | Download and upload models directly | One-line model access, easy sharing |
| **mistral-rs backend** | PagedAttention, X-LoRA, ISQ for production serving | Scale to 50+ concurrent users |
| **Task-specific adapters** | 5 pre-trained LoRA adapters (coder, researcher, security, architect, reviewer) | Instant specialization with hot-swap |

> Part of the [RuVector](https://github.com/ruvnet/ruvector) ecosystem -- the self-learning vector database with graph intelligence, local AI, and PostgreSQL built in.

## Quick Start

```rust
use ruvllm::prelude::*;

let mut backend = CandleBackend::with_device(DeviceType::Metal)?;
backend.load_gguf("models/qwen2.5-7b-q4_k.gguf", ModelConfig::default())?;

let response = backend.generate("Explain quantum computing in simple terms.",
    GenerateParams {
        max_tokens: 256,
        temperature: 0.7,
        top_p: 0.9,
        ..Default::default()
    }
)?;

println!("{}", response);
```

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
# Recommended for Apple Silicon Mac
ruvllm = { version = "2.0", features = ["inference-metal", "coreml", "parallel"] }

# For NVIDIA GPUs
ruvllm = { version = "2.0", features = ["inference-cuda", "parallel"] }

# Minimal (CPU only)
ruvllm = { version = "2.0" }
```

Or install the npm package:

```bash
npm install @ruvector/ruvllm
```

## What's New in v2.3

| Feature | Description | Benefit |
|---------|-------------|---------|
| **RuvLTRA-Medium 3B** | Purpose-built 3B model for Claude Flow | 42 layers, 256K context, speculative decode |
| **HuggingFace Hub** | Full Hub integration (download/upload) | Easy model sharing and distribution |
| **Task-Specific LoRA** | 5 pre-trained adapters for agent types | Optimized for coder/researcher/security/architect/reviewer |
| **Adapter Merging** | TIES, DARE, SLERP, Task Arithmetic | Combine adapters for multi-task models |
| **Hot-Swap Adapters** | Zero-downtime adapter switching | Runtime task specialization |
| **Claude Dataset** | 2,700+ Claude-style training examples | Optimized for Claude Flow integration |
| **HNSW Routing** | 150x faster semantic pattern matching | <25µs pattern retrieval |
| **Evaluation Harness** | Real model evaluation with SWE-Bench | 5 ablation modes, quality metrics |
| **HNSW Auto-Dimension** | Automatic embedding dimension detection | No manual config needed |
| **mistral-rs Backend** | Production-scale serving with PagedAttention | 5-10x concurrent users, X-LoRA, ISQ |

### Previous v2.0-2.2 Features

| Feature | Description | Benefit |
|---------|-------------|---------|
| **Apple Neural Engine** | Core ML backend with ANE routing | 38 TOPS, 3-4x power efficiency |
| **Hybrid GPU+ANE Pipeline** | Intelligent operation routing | Best of both accelerators |
| **Multi-threaded GEMM** | Rayon parallelization | 4-12x speedup on M4 Pro |
| **Flash Attention 2** | Auto block sizing, online softmax | O(N) memory, +10% throughput |
| **Quantized Inference** | INT8/INT4/Q4_K/Q8_K kernels | 4-8x memory reduction |
| **Metal GPU Shaders** | simdgroup_matrix operations | 3x speedup on Apple Silicon |
| **GGUF Support** | Memory-mapped model loading | Fast loading, reduced RAM |
| **Continuous Batching** | Dynamic batch scheduling | 2-3x throughput improvement |
| **Speculative Decoding** | Draft model acceleration | 2-3x faster generation |
| **Gemma-2 & Phi-3** | New model architectures | Extended model support |

## Backends

| Backend | Best For | Acceleration |
|---------|----------|-------------|
| **Candle** | Single user, edge, WASM | Metal, CUDA, CPU |
| **Core ML** | Apple Silicon efficiency | Apple Neural Engine (38 TOPS) |
| **Hybrid Pipeline** | Maximum throughput on Mac | GPU for attention, ANE for MLP |
| **mistral-rs** | Production serving (10-100 users) | PagedAttention, X-LoRA, ISQ |

### Feature Flags

| Feature | Description |
|---------|-------------|
| `candle` | Enable Candle backend (HuggingFace) |
| `metal` | Apple Silicon GPU acceleration via Candle |
| `metal-compute` | Native Metal compute shaders (M4 Pro optimized) |
| `cuda` | NVIDIA GPU acceleration |
| `coreml` | Apple Neural Engine via Core ML |
| `hybrid-ane` | GPU+ANE hybrid pipeline (recommended for Mac) |
| `inference-metal` | Full Metal inference stack |
| `inference-metal-native` | Metal + native shaders (best M4 Pro perf) |
| `inference-cuda` | Full CUDA inference stack |
| `parallel` | Multi-threaded GEMM/GEMV with Rayon |
| `accelerate` | Apple Accelerate BLAS (~2x GEMV speedup) |
| `gguf-mmap` | Memory-mapped GGUF loading |
| `async-runtime` | Tokio async support |
| `wasm` | WebAssembly support |
| `mistral-rs` | mistral-rs backend (PagedAttention, X-LoRA, ISQ) |
| `mistral-rs-metal` | mistral-rs with Apple Silicon acceleration |
| `mistral-rs-cuda` | mistral-rs with NVIDIA CUDA acceleration |

## Architecture

```
+----------------------------------+
|         Application              |
+----------------------------------+
               |
+----------------------------------+
|        RuvLLM Backend            |
|  +----------------------------+  |
|  |   Hybrid Pipeline Router   |  |
|  |  ┌─────────┐ ┌──────────┐  |  |
|  |  │  Metal  │ │   ANE    │  |  |
|  |  │   GPU   │ │ Core ML  │  |  |
|  |  └────┬────┘ └────┬─────┘  |  |
|  |       │    ↕      │        |  |
|  |  Attention    MLP/FFN      |  |
|  |  RoPE         Activations  |  |
|  |  Softmax      LayerNorm    |  |
|  +----------------------------+  |
|               |                  |
|  +----------------------------+  |
|  |     SONA Learning          |  |
|  |  - Instant (<1ms)          |  |
|  |  - Background (~100ms)     |  |
|  |  - Deep (minutes)          |  |
|  +----------------------------+  |
|               |                  |
|  +----------------------------+  |
|  |     NEON/SIMD Kernels      |  |
|  |  - Flash Attention 2       |  |
|  |  - Paged KV Cache          |  |
|  |  - Quantized MatMul        |  |
|  +----------------------------+  |
+----------------------------------+
```

## Supported Models

| Model Family | Sizes | Quantization | Backend |
|--------------|-------|--------------|---------|
| **RuvLTRA-Small** | 0.5B | Q4K, Q5K, Q8, FP16 | Candle/Metal/ANE |
| **RuvLTRA-Medium** | 3B | Q4K, Q5K, Q8, FP16 | Candle/Metal |
| Qwen 2.5 | 0.5B-72B | Q4K, Q8, FP16 | Candle/Metal |
| Llama 3.x | 8B-70B | Q4K, Q8, FP16 | Candle/Metal |
| Mistral | 7B-22B | Q4K, Q8, FP16 | Candle/Metal |
| Phi-3 | 3.8B-14B | Q4K, Q8, FP16 | Candle/Metal |
| Gemma-2 | 2B-27B | Q4K, Q8, FP16 | Candle/Metal |

### RuvLTRA Models (Claude Flow Optimized)

| Model | Parameters | Hidden | Layers | Context | Features |
|-------|------------|--------|--------|---------|----------|
| RuvLTRA-Small | 494M | 896 | 24 | 32K | GQA 7:1, SONA hooks |
| RuvLTRA-Medium | 3.0B | 2560 | 42 | 256K | Flash Attention 2, Speculative Decode |

## Performance (M4 Pro 14-core)

### Inference Benchmarks

| Model | Quant | Prefill (tok/s) | Decode (tok/s) | Memory |
|-------|-------|-----------------|----------------|--------|
| Qwen2.5-7B | Q4K | 2,800 | 95 | 4.2 GB |
| Qwen2.5-7B | Q8 | 2,100 | 72 | 7.8 GB |
| Llama3-8B | Q4K | 2,600 | 88 | 4.8 GB |
| Mistral-7B | Q4K | 2,500 | 85 | 4.1 GB |
| Phi-3-3.8B | Q4K | 3,500 | 135 | 2.3 GB |
| Gemma2-9B | Q4K | 2,200 | 75 | 5.2 GB |

### ANE vs GPU Performance (M4 Pro)

| Dimension | ANE | GPU | Winner |
|-----------|-----|-----|--------|
| < 512 | +30-50% | - | ANE |
| 512-1024 | +10-30% | - | ANE |
| 1024-1536 | ~Similar | ~Similar | Either |
| 1536-2048 | - | +10-20% | GPU |
| > 2048 | - | +30-50% | GPU |

### Kernel Benchmarks

| Kernel | Single-thread | Multi-thread (10-core) |
|--------|---------------|------------------------|
| GEMM 4096x4096 | 1.2 GFLOPS | 12.7 GFLOPS |
| GEMV 4096x4096 | 0.8 GFLOPS | 6.4 GFLOPS |
| Flash Attention (seq=2048) | 850μs | 320μs |
| RMS Norm (4096) | 2.1μs | 0.8μs |
| RoPE (4096, 128) | 4.3μs | 1.6μs |

## Apple Neural Engine (ANE) Integration

RuvLLM v2.0 includes full ANE support via Core ML:

```rust
use ruvllm::backends::coreml::{CoreMLBackend, AneStrategy};

// Create ANE-optimized backend
let backend = CoreMLBackend::new(AneStrategy::PreferAneForMlp)?;

// Or use hybrid pipeline for best performance
use ruvllm::backends::HybridPipeline;

let pipeline = HybridPipeline::new(HybridConfig {
    ane_strategy: AneStrategy::Adaptive,
    gpu_for_attention: true,  // Attention on GPU
    ane_for_mlp: true,        // MLP/FFN on ANE
    ..Default::default()
})?;
```

### ANE Routing Recommendations

| Operation | Recommended | Reason |
|-----------|-------------|--------|
| Attention | GPU | Better for variable sequence lengths |
| Flash Attention | GPU | GPU memory bandwidth advantage |
| MLP/FFN | ANE | Optimal for fixed-size matmuls |
| GELU/SiLU | ANE | Dedicated activation units |
| LayerNorm/RMSNorm | ANE | Good for small dimensions |
| Embedding | GPU | Sparse operations |

## MicroLoRA Real-Time Adaptation

RuvLLM supports per-request fine-tuning using MicroLoRA:

```rust
use ruvllm::lora::{MicroLoRA, MicroLoraConfig, AdaptFeedback};

// Create MicroLoRA adapter
let config = MicroLoraConfig::for_hidden_dim(4096);
let lora = MicroLoRA::new(config);

// Adapt on user feedback
let feedback = AdaptFeedback::from_quality(0.9);
lora.adapt(&input_embedding, feedback)?;

// Apply learned updates
lora.apply_updates(0.01); // learning rate

// Get adaptation stats
let stats = lora.stats();
println!("Samples: {}, Avg quality: {:.2}", stats.samples, stats.avg_quality);
```

## SONA Three-Tier Learning

Continuous improvement with three learning loops:

```rust
use ruvllm::optimization::{SonaLlm, SonaLlmConfig, ConsolidationStrategy};

let config = SonaLlmConfig {
    instant_lr: 0.01,
    background_interval_ms: 100,
    deep_trigger_threshold: 100.0,
    consolidation_strategy: ConsolidationStrategy::EwcMerge,
    ..Default::default()
};

let sona = SonaLlm::new(config);

// 1. Instant Loop (<1ms): Per-request MicroLoRA
let result = sona.instant_adapt("user query", "model response", 0.85);
println!("Instant adapt: {}μs", result.latency_us);

// 2. Background Loop (~100ms): Pattern consolidation
if let result = sona.maybe_background() {
    if result.applied {
        println!("Consolidated {} samples", result.samples_used);
    }
}

// 3. Deep Loop (minutes): Full optimization
if sona.should_trigger_deep() {
    let result = sona.deep_optimize(OptimizationTrigger::QualityThreshold(100.0));
    println!("Deep optimization: {:.1}s", result.latency_us as f64 / 1_000_000.0);
}

// Check learning stats
let stats = sona.stats();
println!("Total samples: {}", stats.total_samples);
println!("Accumulated quality: {:.2}", stats.accumulated_quality);
```

## Two-Tier KV Cache

Memory-efficient caching with automatic tiering:

```rust
use ruvllm::kv_cache::{TwoTierKvCache, KvCacheConfig};

let config = KvCacheConfig {
    tail_length: 256,              // Recent tokens in FP16
    tail_precision: Precision::FP16,
    store_precision: Precision::Q4,  // Older tokens in Q4
    max_tokens: 8192,
    num_layers: 32,
    num_kv_heads: 8,
    head_dim: 128,
};

let cache = TwoTierKvCache::new(config);
cache.append(&keys, &values)?;

// Automatic migration from tail to quantized store
let stats = cache.stats();
println!("Tail: {} tokens, Store: {} tokens", stats.tail_tokens, stats.store_tokens);
println!("Compression ratio: {:.2}x", stats.compression_ratio);
println!("Memory saved: {:.1} MB", stats.memory_saved_mb);
```

## Continuous Batching

High-throughput serving with dynamic batching:

```rust
use ruvllm::serving::{ContinuousBatchScheduler, SchedulerConfig, InferenceRequest};

let scheduler = ContinuousBatchScheduler::new(SchedulerConfig {
    max_batch_size: 32,
    max_batch_tokens: 4096,
    max_waiting_time_ms: 50,
    preemption_mode: PreemptionMode::Recompute,
    ..Default::default()
});

// Add requests
scheduler.add_request(InferenceRequest::new(tokens, params))?;

// Process batch
while let Some(batch) = scheduler.get_next_batch() {
    let outputs = backend.forward_batch(&batch)?;
    scheduler.process_outputs(outputs)?;
}

// Get throughput stats
let stats = scheduler.stats();
println!("Throughput: {:.1} tok/s", stats.tokens_per_second);
println!("Batch utilization: {:.1}%", stats.avg_batch_utilization * 100.0);
```

## Speculative Decoding

Accelerate generation with draft models:

```rust
use ruvllm::speculative::{SpeculativeDecoder, SpeculativeConfig};

let config = SpeculativeConfig {
    draft_tokens: 4,           // Tokens to draft per step
    acceptance_threshold: 0.8, // Min probability for acceptance
    ..Default::default()
};

let decoder = SpeculativeDecoder::new(
    target_model,
    draft_model,
    config,
)?;

// Generate with speculation
let output = decoder.generate(prompt, GenerateParams {
    max_tokens: 256,
    ..Default::default()
})?;

println!("Acceptance rate: {:.1}%", output.stats.acceptance_rate * 100.0);
println!("Speedup: {:.2}x", output.stats.speedup);
```

## GGUF Model Loading

Efficient loading with memory mapping:

```rust
use ruvllm::gguf::{GgufLoader, GgufConfig};

let loader = GgufLoader::new(GgufConfig {
    mmap_enabled: true,       // Memory-map for fast loading
    validate_checksum: true,  // Verify file integrity
    ..Default::default()
});

// Load model metadata
let metadata = loader.read_metadata("model.gguf")?;
println!("Model: {}", metadata.name);
println!("Parameters: {}B", metadata.parameters / 1_000_000_000);
println!("Quantization: {:?}", metadata.quantization);

// Load into backend
let tensors = loader.load_tensors("model.gguf")?;
backend.load_tensors(tensors)?;
```

## mistral-rs Backend (Production Serving)

RuvLLM v2.3 includes integration with [mistral-rs](https://github.com/EricLBuehler/mistral.rs) for production-scale LLM serving with advanced memory management.

> **Note**: The mistral-rs crate is not yet published to crates.io. The integration is designed and ready—enable it when mistral-rs becomes available.

### Key Features

| Feature | Description | Benefit |
|---------|-------------|---------|
| **PagedAttention** | vLLM-style KV cache management | 5-10x concurrent users, 85-95% memory utilization |
| **X-LoRA** | Per-token adapter routing | <1ms routing overhead, multi-task inference |
| **ISQ** | In-Situ Quantization (AWQ, GPTQ, RTN) | Runtime quantization without re-export |

### Usage Example

```rust
use ruvllm::backends::mistral::{
    MistralBackend, MistralBackendConfig,
    PagedAttentionConfig, XLoraConfig, IsqConfig
};

// Configure mistral-rs backend for production serving
let config = MistralBackendConfig::builder()
    // PagedAttention: Enable 50+ concurrent users
    .paged_attention(PagedAttentionConfig {
        block_size: 16,
        max_blocks: 4096,
        gpu_memory_fraction: 0.9,
        enable_prefix_caching: true,
    })
    // X-LoRA: Per-token adapter routing
    .xlora(XLoraConfig {
        adapters: vec![
            "adapters/coder".into(),
            "adapters/researcher".into(),
        ],
        top_k: 2,
        temperature: 0.3,
    })
    // ISQ: Runtime quantization
    .isq(IsqConfig {
        bits: 4,
        method: IsqMethod::AWQ,
        calibration_samples: 128,
    })
    .build();

let mut backend = MistralBackend::new(config)?;
backend.load_model("mistralai/Mistral-7B-Instruct-v0.2", ModelConfig::default())?;

// Generate with PagedAttention + X-LoRA
let response = backend.generate("Write secure authentication code", GenerateParams {
    max_tokens: 512,
    temperature: 0.7,
    ..Default::default()
})?;
```

### When to Use mistral-rs vs Candle

| Scenario | Recommended Backend | Reason |
|----------|---------------------|--------|
| Single user / Edge | Candle | Simpler, smaller binary |
| 10-100 concurrent users | mistral-rs | PagedAttention memory efficiency |
| Multi-task models | mistral-rs | X-LoRA per-token routing |
| Runtime quantization | mistral-rs | ISQ without model re-export |
| WASM / Browser | Candle | mistral-rs doesn't support WASM |

### Feature Flags

```toml
# Enable mistral-rs (when available on crates.io)
ruvllm = { version = "2.3", features = ["mistral-rs"] }

# With Metal acceleration (Apple Silicon)
ruvllm = { version = "2.3", features = ["mistral-rs-metal"] }

# With CUDA acceleration (NVIDIA)
ruvllm = { version = "2.3", features = ["mistral-rs-cuda"] }
```

See [ADR-008: mistral-rs Integration](../../docs/adr/ADR-008-mistral-rs-integration.md) for detailed architecture decisions.

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `RUVLLM_CACHE_DIR` | Model cache directory | `~/.cache/ruvllm` |
| `RUVLLM_LOG_LEVEL` | Logging level | `info` |
| `RUVLLM_METAL_DEVICE` | Metal device index | `0` |
| `RUVLLM_ANE_ENABLED` | Enable ANE routing | `true` |
| `RUVLLM_SONA_ENABLED` | Enable SONA learning | `true` |

### Model Configuration

```rust
let config = ModelConfig {
    max_context: 8192,
    use_flash_attention: true,
    quantization: Quantization::Q4K,
    kv_cache_config: KvCacheConfig::default(),
    rope_scaling: Some(RopeScaling::Linear { factor: 2.0 }),
    sliding_window: Some(4096),
    ..Default::default()
};
```

## Benchmarks

Run benchmarks with:

```bash
# Attention benchmarks
cargo bench --bench attention_bench --features inference-metal

# ANE benchmarks (Mac only)
cargo bench --bench ane_bench --features coreml

# LoRA benchmarks
cargo bench --bench lora_bench

# End-to-end inference
cargo bench --bench e2e_bench --features inference-metal

# Metal shader benchmarks
cargo bench --bench metal_bench --features metal-compute

# Serving benchmarks
cargo bench --bench serving_bench --features inference-metal
```

## HuggingFace Hub Integration (v2.3)

Download and upload models to HuggingFace Hub:

```rust
use ruvllm::hub::{ModelDownloader, ModelUploader, RuvLtraRegistry, DownloadConfig};

// Download from Hub
let downloader = ModelDownloader::new(DownloadConfig::default());
let model_path = downloader.download(
    "ruvector/ruvltra-small-q4km",
    Some("./models"),
)?;

// Or use the registry for RuvLTRA models
let registry = RuvLtraRegistry::new();
let model = registry.get("ruvltra-medium", "Q4_K_M")?;

// Upload to Hub (requires HF_TOKEN)
let uploader = ModelUploader::new("hf_your_token");
let url = uploader.upload(
    "./my-model.gguf",
    "username/my-ruvltra-model",
    Some(metadata),
)?;
println!("Uploaded to: {}", url);
```

## Task-Specific LoRA Adapters (v2.3)

Pre-trained adapters optimized for Claude Flow agent types:

```rust
use ruvllm::lora::{RuvLtraAdapters, AdapterTrainer, AdapterMerger, HotSwapManager};

// Create adapter for specific task
let adapters = RuvLtraAdapters::new();
let coder = adapters.create_lora("coder", 768)?;       // Rank 16, code generation
let security = adapters.create_lora("security", 768)?; // Rank 16, vulnerability detection

// Available adapters:
// - coder:     Rank 16, Alpha 32.0, targets attention (Q,K,V,O)
// - researcher: Rank 8, Alpha 16.0, targets Q,K,V
// - security:  Rank 16, Alpha 32.0, targets attention + MLP
// - architect: Rank 12, Alpha 24.0, targets Q,V + Gate,Up
// - reviewer:  Rank 8, Alpha 16.0, targets Q,V

// Merge adapters for multi-task models
let merger = AdapterMerger::new(MergeConfig::weighted(weights));
let multi_task = merger.merge(&[coder, security], &output_config, 768)?;

// Hot-swap adapters at runtime
let mut manager = HotSwapManager::new();
manager.set_active(coder);
manager.prepare_standby(security);
manager.swap()?; // Zero-downtime switch
```

### Adapter Merging Strategies

| Strategy | Description | Use Case |
|----------|-------------|----------|
| **Average** | Equal-weight averaging | Simple multi-task |
| **WeightedSum** | User-defined weights | Task importance weighting |
| **SLERP** | Spherical interpolation | Smooth transitions |
| **TIES** | Trim, Elect, Merge | Robust multi-adapter |
| **DARE** | Drop And REscale | Sparse merging |
| **TaskArithmetic** | Add/subtract vectors | Task composition |

## Evaluation Harness (v2.3)

RuvLLM includes a comprehensive evaluation harness for benchmarking model quality:

```rust
use ruvllm::evaluation::{RealEvaluationHarness, EvalConfig, AblationMode};

// Create harness with GGUF model
let harness = RealEvaluationHarness::with_gguf(
    "./models/tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf",
    EvalConfig::default(),
)?;

// Run single evaluation
let result = harness.evaluate(
    "Fix the null pointer exception in this code",
    "def process(data):\n    return data.split()",
    AblationMode::Full,
)?;

println!("Success: {}, Quality: {:.2}", result.success, result.quality_score);

// Run full ablation study (5 modes)
let report = harness.run_ablation_study(&tasks)?;
for (mode, metrics) in &report.mode_metrics {
    println!("{:?}: {:.1}% success, {:.2} quality",
        mode, metrics.success_rate * 100.0, metrics.avg_quality);
}
```

### Ablation Modes

| Mode | Description | Use Case |
|------|-------------|----------|
| **Baseline** | No enhancements | Control baseline |
| **RetrievalOnly** | HNSW pattern retrieval | Measure retrieval impact |
| **AdaptersOnly** | LoRA adapters | Measure adaptation impact |
| **RetrievalPlusAdapters** | HNSW + LoRA | Combined without SONA |
| **Full** | All systems (SONA + HNSW + LoRA) | Production mode |

### SWE-Bench Task Loader

```rust
use ruvllm::evaluation::swe_bench::SweBenchLoader;

// Load SWE-Bench tasks
let loader = SweBenchLoader::new();
let tasks = loader.load_subset("lite", 50)?; // 50 tasks from lite subset

for task in &tasks {
    println!("Instance: {}", task.instance_id);
    println!("Problem: {}", task.problem_statement);
}
```

### CLI Evaluation

```bash
# Run evaluation with default settings
cargo run --example run_eval --features async-runtime -- \
    --model ./models/tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf

# Run SWE-Bench subset
cargo run --example run_eval --features async-runtime -- \
    --model ./models/model.gguf \
    --swe-bench-path ./data/swe-bench \
    --subset lite \
    --max-tasks 100

# Output report
cargo run --example run_eval --features async-runtime -- \
    --model ./models/model.gguf \
    --output ./reports/eval-report.json
```

### HNSW Auto-Dimension Detection

The evaluation harness automatically detects model embedding dimensions:

```rust
// HNSW router automatically uses model's hidden_size
// TinyLlama 1.1B → 2048 dimensions
// Qwen2 0.5B → 896 dimensions
// RuvLTRA-Small → 896 dimensions
// RuvLTRA-Medium → 2560 dimensions

let harness = RealEvaluationHarness::with_config(
    EvalConfig::default(),
    RealInferenceConfig {
        enable_hnsw: true,
        hnsw_config: None, // Auto-detect from model
        ..Default::default()
    },
)?;
```

## Examples

See the `/examples` directory for:

- `download_test_model.rs` - Download and validate models
- `benchmark_model.rs` - Full inference benchmarking
- `run_eval.rs` - Run evaluation harness with SWE-Bench
- Basic inference
- Streaming generation
- MicroLoRA adaptation
- Multi-turn chat
- Speculative decoding
- Continuous batching
- ANE hybrid inference

## Error Handling

```rust
use ruvllm::error::{Result, RuvLLMError};

match backend.generate(prompt, params) {
    Ok(response) => println!("{}", response),
    Err(RuvLLMError::Model(e)) => eprintln!("Model error: {}", e),
    Err(RuvLLMError::OutOfMemory(e)) => eprintln!("OOM: {}", e),
    Err(RuvLLMError::Generation(e)) => eprintln!("Generation failed: {}", e),
    Err(RuvLLMError::Ane(e)) => eprintln!("ANE error: {}", e),
    Err(RuvLLMError::Gguf(e)) => eprintln!("GGUF loading error: {}", e),
    Err(e) => eprintln!("Error: {}", e),
}
```

## npm Package

RuvLLM is also available as an npm package with native bindings:

```bash
npm install @ruvector/ruvllm
```

```typescript
import { RuvLLM } from '@ruvector/ruvllm';

const llm = new RuvLLM();
const response = llm.query('Explain quantum computing');
console.log(response.text);
```

See [@ruvector/ruvllm on npm](https://www.npmjs.com/package/@ruvector/ruvllm) for full documentation.

## License

Apache-2.0 / MIT dual license.

## Contributing

Contributions welcome! Please see [CONTRIBUTING.md](../../CONTRIBUTING.md) for guidelines.

## Links

- [GitHub Repository](https://github.com/ruvnet/ruvector)
- [API Documentation](https://docs.rs/ruvllm)
- [npm Package](https://www.npmjs.com/package/@ruvector/ruvllm)
- [Issue Tracker](https://github.com/ruvnet/ruvector/issues)

---

Part of [RuVector](https://github.com/ruvnet/ruvector) -- the self-learning vector database.
