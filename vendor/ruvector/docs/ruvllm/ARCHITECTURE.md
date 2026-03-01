# RuvLLM Architecture (v2.0.0)

This document describes the system architecture of RuvLLM, a high-performance LLM inference engine optimized for Apple Silicon.

## v2.0.0 New Features

| Feature | Description | Performance Impact |
|---------|-------------|-------------------|
| Multi-threaded GEMM/GEMV | Rayon parallelization | 12.7x speedup on M4 Pro |
| Flash Attention 2 | Auto block sizing | +10% throughput |
| Quantized Inference | INT8/INT4/Q4_K kernels | 4-8x memory reduction |
| Metal GPU Shaders | simdgroup_matrix ops | 3x speedup |
| Memory Pool | Arena allocator | Zero-alloc inference |
| WASM Support | Browser inference | ~2.5x overhead |
| npm Integration | @ruvector/ruvllm | JavaScript/TypeScript API |

## System Overview

```
                              +----------------------------------+
                              |          User Application        |
                              +----------------------------------+
                                              |
                                              v
+-------------------------------------------------------------------------------------+
|                                    RuvLLM Core                                       |
|  +-------------------------------------------------------------------------------+  |
|  |                              Backend Abstraction                               |  |
|  |  +-------------------------+  +-------------------------+                     |  |
|  |  |    Candle Backend       |  |    mistral-rs Backend   |                     |  |
|  |  |  - Model Loading        |  |  - Model Loading        |                     |  |
|  |  |  - Tokenization         |  |  - Tokenization         |                     |  |
|  |  |  - Forward Pass         |  |  - Forward Pass         |                     |  |
|  |  +-------------------------+  +-------------------------+                     |  |
|  +-------------------------------------------------------------------------------+  |
|                                          |                                          |
|  +-------------------------------------------------------------------------------+  |
|  |                              SONA Learning Layer                               |  |
|  |  +---------------------+  +----------------------+  +---------------------+   |  |
|  |  |    Instant Loop     |  |   Background Loop    |  |     Deep Loop       |   |  |
|  |  |    (<1ms latency)   |  |   (~100ms interval)  |  |   (minutes/hours)   |   |  |
|  |  |  - MicroLoRA adapt  |  |  - Pattern merge     |  |  - Full fine-tune   |   |  |
|  |  |  - Per-request      |  |  - EWC++ update      |  |  - Model distill    |   |  |
|  |  +---------------------+  +----------------------+  +---------------------+   |  |
|  +-------------------------------------------------------------------------------+  |
|                                          |                                          |
|  +-------------------------------------------------------------------------------+  |
|  |                              Optimized Kernels                                 |  |
|  |  +------------------+  +------------------+  +------------------+              |  |
|  |  |  Attention       |  |  Normalization   |  |  Embedding       |              |  |
|  |  |  - Flash Attn 2  |  |  - RMSNorm       |  |  - RoPE          |              |  |
|  |  |  - Paged Attn    |  |  - LayerNorm     |  |  - Token Embed   |              |  |
|  |  |  - GQA/MQA       |  |  - Fused Ops     |  |  - Pos Embed     |              |  |
|  |  +------------------+  +------------------+  +------------------+              |  |
|  +-------------------------------------------------------------------------------+  |
|                                          |                                          |
|  +-------------------------------------------------------------------------------+  |
|  |                              Memory Management                                 |  |
|  |  +-------------------------+  +-------------------------------------------+   |  |
|  |  |   Two-Tier KV Cache     |  |           Memory Pool                     |   |  |
|  |  |  +-------------------+  |  |  - Slab allocator                         |   |  |
|  |  |  |  FP16 Tail (hot)  |  |  |  - Arena allocation                       |   |  |
|  |  |  +-------------------+  |  |  - Zero-copy transfers                    |   |  |
|  |  |  |  Q4 Store (cold)  |  |  |                                           |   |  |
|  |  |  +-------------------+  |  +-------------------------------------------+   |  |
|  |  +-------------------------+                                                  |  |
|  +-------------------------------------------------------------------------------+  |
+-------------------------------------------------------------------------------------+
                                          |
                                          v
+-------------------------------------------------------------------------------------+
|                              Hardware Acceleration                                   |
|  +---------------------------+  +---------------------------+                       |
|  |     Metal (Apple GPU)     |  |      CUDA (NVIDIA)        |                       |
|  |  - MLX integration        |  |  - cuBLAS                 |                       |
|  |  - Metal Performance      |  |  - cuDNN                  |                       |
|  |    Shaders                |  |  - TensorRT               |                       |
|  +---------------------------+  +---------------------------+                       |
+-------------------------------------------------------------------------------------+
```

## Component Architecture

### 1. Backend Abstraction Layer

The backend abstraction provides a unified interface for different ML frameworks.

```
+---------------------------+
|     LlmBackend Trait      |
|  - load_model()           |
|  - generate()             |
|  - forward()              |
|  - get_tokenizer()        |
+---------------------------+
           ^
           |
    +------+------+
    |             |
+-------+   +-----------+
|Candle |   |mistral-rs |
+-------+   +-----------+
```

**Candle Backend Features:**
- HuggingFace model hub integration
- Native Rust tensor operations
- Metal/CUDA acceleration
- Safetensors loading

### 2. SONA Learning Layer

Self-Optimizing Neural Architecture with three learning loops:

```
+-------------------+     +-------------------+
| Inference Request |---->| Instant Loop      |
| + feedback        |     | - MicroLoRA adapt |
+-------------------+     | - <1ms latency    |
                          +--------+----------+
                                   |
                                   v (async, 100ms)
                          +--------+----------+
                          | Background Loop   |
                          | - Pattern merge   |
                          | - Adapter compose |
                          | - EWC++ update    |
                          +--------+----------+
                                   |
                                   v (triggered)
                          +--------+----------+
                          | Deep Loop         |
                          | - Full fine-tune  |
                          | - Model distill   |
                          | - Pattern bank    |
                          +-------------------+
```

**Loop Characteristics:**

| Loop | Latency | Trigger | Purpose |
|------|---------|---------|---------|
| Instant | <1ms | Per-request | Real-time adaptation |
| Background | ~100ms | Interval/threshold | Pattern consolidation |
| Deep | Minutes | Accumulated quality | Full optimization |

### 3. Optimized Kernel Layer

NEON SIMD-optimized kernels for ARM64:

```
+-----------------------------------------------+
|              Attention Kernels                 |
+-----------------------------------------------+
|                                               |
|  +------------------+  +------------------+   |
|  | Flash Attention  |  | Paged Attention  |   |
|  |  - Tiled QKV     |  |  - Block tables  |   |
|  |  - Online softmax|  |  - Non-contiguous|   |
|  |  - O(N) memory   |  |  - KV cache aware|   |
|  +------------------+  +------------------+   |
|                                               |
|  +------------------+  +------------------+   |
|  | Multi-Query (MQA)|  | Grouped-Query    |   |
|  |  - 1 KV head     |  |  - KV groups     |   |
|  |  - Shared KV     |  |  - 4-8x savings  |   |
|  +------------------+  +------------------+   |
+-----------------------------------------------+

+-----------------------------------------------+
|            Normalization Kernels               |
+-----------------------------------------------+
|  +------------------+  +------------------+   |
|  |    RMSNorm       |  |    LayerNorm     |   |
|  |  - NEON SIMD     |  |  - NEON SIMD     |   |
|  |  - Fused ops     |  |  - Fused ops     |   |
|  +------------------+  +------------------+   |
+-----------------------------------------------+

+-----------------------------------------------+
|             Embedding Kernels                  |
+-----------------------------------------------+
|  +------------------+  +------------------+   |
|  | Rotary Position  |  | Token Embedding  |   |
|  |  (RoPE)          |  |  - Lookup table  |   |
|  |  - Precomputed   |  |  - Batch gather  |   |
|  +------------------+  +------------------+   |
+-----------------------------------------------+
```

### 4. Memory Management

Two-tier KV cache for optimal memory/quality tradeoff:

```
+----------------------------------------------------+
|                  Two-Tier KV Cache                  |
+----------------------------------------------------+
|                                                    |
|   Position: 0            tail_length        max    |
|   +------------------+------------------+          |
|   |                  |                  |          |
|   |  Quantized Store |  High-Precision  |          |
|   |     (Cold)       |    Tail (Hot)    |          |
|   |                  |                  |          |
|   |  - Q4/Q8 format  |  - FP16 format   |          |
|   |  - Older tokens  |  - Recent tokens |          |
|   |  - 4x smaller    |  - Full quality  |          |
|   |                  |                  |          |
|   +------------------+------------------+          |
|                                                    |
|   Migration: Hot -> Cold (when tail_length exceeded)|
|   Eviction:  Cold first, then Hot                  |
+----------------------------------------------------+
```

**Cache Operations:**

1. **Append**: Add new KV pairs to tail
2. **Migrate**: Move old tokens from tail to quantized store
3. **Evict**: Remove oldest tokens when max exceeded
4. **Attend**: Dequantize cold + use hot for attention

## Data Flow

### Inference Pipeline

```
Input Tokens
     |
     v
+--------------------+
|   Token Embedding  |
|   + RoPE Position  |
+--------------------+
     |
     v (for each layer)
+--------------------+
|   Attention Layer  |
|   +---------------+|
|   | Q,K,V Project ||
|   +---------------+|
|          |         |
|   +---------------+|
|   | KV Cache      ||
|   | Update        ||
|   +---------------+|
|          |         |
|   +---------------+|
|   | Flash/Paged   ||
|   | Attention     ||
|   +---------------+|
|          |         |
|   +---------------+|
|   | Output Proj   ||
|   +---------------+|
+--------------------+
     |
     v
+--------------------+
|   FFN Layer        |
|   - Gate Proj      |
|   - Up Proj        |
|   - Down Proj      |
|   - Activation     |
+--------------------+
     |
     v
+--------------------+
|   RMSNorm          |
+--------------------+
     |
     v
+--------------------+
|   LM Head          |
|   (final layer)    |
+--------------------+
     |
     v
Logits -> Sampling -> Token
```

### Learning Pipeline

```
Request + Response + Feedback
              |
              v
+---------------------------+
|      Instant Loop         |
|  - Compute embeddings     |
|  - Apply MicroLoRA        |
|  - Queue for background   |
+---------------------------+
              |
              v (async)
+---------------------------+
|    Background Loop        |
|  - Batch samples          |
|  - Update EWC++ Fisher    |
|  - Merge adapters         |
|  - Store in ReasoningBank |
+---------------------------+
              |
              v (threshold triggered)
+---------------------------+
|       Deep Loop           |
|  - Full training pipeline |
|  - Pattern distillation   |
|  - Catastrophic forget    |
|    prevention (EWC++)     |
+---------------------------+
```

## Module Structure

```
ruvllm/
├── src/
│   ├── lib.rs              # Crate root, re-exports
│   ├── error.rs            # Error types
│   ├── types.rs            # Common types (Precision, etc.)
│   │
│   ├── backends/           # ML framework backends
│   │   ├── mod.rs          # Backend trait
│   │   ├── candle_backend.rs
│   │   └── config.rs
│   │
│   ├── kernels/            # Optimized kernels
│   │   ├── mod.rs          # Kernel exports
│   │   ├── attention.rs    # Attention variants
│   │   ├── matmul.rs       # Matrix multiplication
│   │   ├── norm.rs         # Normalization ops
│   │   └── rope.rs         # Rotary embeddings
│   │
│   ├── lora/               # LoRA adapters
│   │   ├── mod.rs          # LoRA exports
│   │   ├── micro_lora.rs   # Real-time MicroLoRA
│   │   └── training.rs     # Training pipeline
│   │
│   ├── optimization/       # SONA integration
│   │   ├── mod.rs
│   │   └── sona_llm.rs     # Learning loops
│   │
│   ├── kv_cache.rs         # Two-tier KV cache
│   ├── sona.rs             # SONA core integration
│   ├── policy_store.rs     # Learned policies
│   └── witness_log.rs      # Inference logging
│
└── benches/                # Benchmarks
    ├── attention_bench.rs
    ├── lora_bench.rs
    └── e2e_bench.rs
```

## Performance Characteristics

### Memory Layout

| Component | Memory Pattern | Optimization |
|-----------|---------------|--------------|
| KV Cache Tail | Sequential | NEON vectorized |
| KV Cache Store | Quantized blocks | Batch dequant |
| Model Weights | Memory-mapped | Zero-copy |
| Intermediate | Stack allocated | Arena alloc |

### Throughput Targets (M4 Pro)

| Operation | Target | Achieved |
|-----------|--------|----------|
| Flash Attention | 2.5x vs naive | ~2.3x |
| Paged Attention | 1.8x vs contiguous | ~1.7x |
| GQA vs MHA | 4x less KV memory | 4x |
| MicroLoRA adapt | <1ms | ~0.5ms |

## Integration Points

### With RuVector Core

```rust
// Memory backend integration
use ruvector_core::storage::Storage;

// SONA learning integration
use ruvector_sona::{SonaEngine, ReasoningBank};
```

### With External Systems

- **HuggingFace Hub**: Model downloads
- **OpenAI API**: Compatible inference endpoint
- **Prometheus**: Metrics export
- **gRPC**: High-performance RPC

## Future Architecture

Planned enhancements:

1. **Speculative Decoding**: Draft model integration
2. **Tensor Parallelism**: Multi-GPU support
3. **Continuous Batching**: Dynamic batch scheduling
4. **PagedAttention v2**: vLLM-style memory management
