# RuvLLM: Candle + mistral-rs + SONA Integration Architecture

**Document Version**: 1.0
**Status**: Proposed
**Date**: 2026-01-18
**Target Hardware**: Apple M4 Pro (ARM64/NEON)

---

## 1. Executive Summary

This document defines the architecture for integrating Candle tensor operations, mistral-rs model inference, and RuvLLM's SONA learning framework into a unified, high-performance LLM serving runtime optimized for Apple Silicon.

### Key Design Goals

| Goal | Target | Rationale |
|------|--------|-----------|
| Inference Latency | <50ms TTFT | Real-time interactive use |
| Memory Efficiency | 4GB for 7B model | M4 Pro unified memory constraint |
| Learning Overhead | <1ms per request | SONA instant loop requirement |
| Throughput | 100+ tokens/sec | Competitive with cloud inference |

---

## 2. Component Diagram

```
+===========================================================================+
|                           RuvLLM Engine (Orchestration Layer)              |
+===========================================================================+
|                                                                            |
|   +-------------------+     +-------------------+     +------------------+ |
|   |   Request Router  |---->|   Model Selector  |---->|  Batch Scheduler | |
|   |   (SONA-guided)   |     |   (FastGRNN)      |     |  (Continuous)    | |
|   +-------------------+     +-------------------+     +------------------+ |
|            |                         |                        |            |
|            v                         v                        v            |
|   +------------------------------------------------------------------------+
|   |                         Backend Abstraction Layer                       |
|   +------------------------------------------------------------------------+
|            |                         |                        |            |
|            v                         v                        v            |
|   +-------------------+     +-------------------+     +------------------+ |
|   |   Candle Backend  |     | mistral-rs Backend|     | Hybrid Backend   | |
|   |   (Tensor Ops)    |     | (Full Inference)  |     | (Mix & Match)    | |
|   +-------------------+     +-------------------+     +------------------+ |
|            |                         |                        |            |
|            +-------------+-----------+------------------------+            |
|                          |                                                 |
|                          v                                                 |
|   +------------------------------------------------------------------------+
|   |                    NEON-Optimized Kernel Layer                         |
|   |                    (ruvector-core/simd_intrinsics)                     |
|   +------------------------------------------------------------------------+
|   |  Attention    |  RoPE/ALiBi  |  RMSNorm    |  Quantization  |  GEMM  |
|   +------------------------------------------------------------------------+
|                          |                                                 |
|                          v                                                 |
|   +------------------------------------------------------------------------+
|   |                    Memory Management Layer                              |
|   +------------------------------------------------------------------------+
|   | +----------------+ +------------------+ +----------------------------+ |
|   | | Arena Allocator| | Unified Mem Pool | | 3-Tier KV Cache            | |
|   | | (Batch Ops)    | | (ADR-006)        | | Hot(FP16)/Warm(Q8)/Cold(Q4)| |
|   | +----------------+ +------------------+ +----------------------------+ |
|   +------------------------------------------------------------------------+
|                          |                                                 |
|                          v                                                 |
|   +------------------------------------------------------------------------+
|   |                    SONA Learning Integration                            |
|   +------------------------------------------------------------------------+
|   | +----------------+ +------------------+ +----------------------------+ |
|   | | MicroLoRA      | | ReasoningBank    | | EWC++ Fisher               | |
|   | | (Rank 1-2)     | | (Pattern Store)  | | (Forgetting Prevention)    | |
|   | +----------------+ +------------------+ +----------------------------+ |
|   +------------------------------------------------------------------------+
|                                                                            |
+============================================================================+
```

---

## 3. Integration Architecture

### 3.1 Backend Selection Strategy

```
+-----------------------------------------------------------------------+
|                    BACKEND SELECTION DECISION TREE                     |
+-----------------------------------------------------------------------+

                        +-------------------+
                        |  Inference Request |
                        +---------+---------+
                                  |
                        +---------v---------+
                        | Check Model Type  |
                        +---------+---------+
                                  |
            +---------------------+---------------------+
            |                     |                     |
    +-------v-------+     +-------v-------+     +-------v-------+
    | Standard LLM  |     | Custom/LoRA   |     | Embedding     |
    | (Mistral/Llama)|     | (Fine-tuned)  |     | Only          |
    +-------+-------+     +-------+-------+     +-------+-------+
            |                     |                     |
    +-------v-------+     +-------v-------+     +-------v-------+
    | mistral-rs    |     | Candle Backend|     | Candle Backend|
    | Backend       |     | + MicroLoRA   |     | (Optimized)   |
    | (Full Model)  |     | Injection     |     |               |
    +---------------+     +---------------+     +---------------+

Backend Selection Criteria:
- mistral-rs: Best for standard models (optimized loading, PagedAttention)
- Candle: Best for custom operations, LoRA injection, embeddings
- Hybrid: Route different layers to different backends
```

### 3.2 Candle Integration Layer

```rust
// crates/ruvllm/src/backends/candle.rs

/// Candle backend configuration
pub struct CandleBackendConfig {
    /// Device type (Metal for M4 Pro)
    pub device: DeviceType,
    /// Default dtype for operations
    pub default_dtype: DType,
    /// Enable Metal Performance Shaders
    pub use_mps: bool,
    /// Memory pool configuration
    pub memory_config: MemoryConfig,
}

/// Candle backend for tensor operations
pub struct CandleBackend {
    config: CandleBackendConfig,
    device: Device,
    /// NEON kernel registry
    neon_kernels: NeonKernelRegistry,
    /// Memory pool
    memory_pool: Arc<UnifiedMemoryPool>,
}

impl CandleBackend {
    /// Create tensors with NEON-optimized operations
    pub fn create_tensor(&self, data: &[f32], shape: &[usize]) -> Result<Tensor> {
        // Use CacheAlignedVec for NEON compatibility
        let aligned = CacheAlignedVec::from_slice(data);
        Tensor::from_slice(aligned.as_slice(), shape, &self.device)
    }

    /// Execute NEON-optimized attention
    pub fn attention(&self, q: &Tensor, k: &Tensor, v: &Tensor, scale: f32) -> Result<Tensor> {
        // Route to NEON kernel if dimensions match optimization thresholds
        if self.should_use_neon(q.dims()) {
            self.neon_kernels.attention(q, k, v, scale)
        } else {
            // Fallback to Candle default
            candle_nn::attention(q, k, v, scale)
        }
    }
}
```

### 3.3 mistral-rs Integration Layer

```rust
// crates/ruvllm/src/backends/mistral.rs

/// mistral-rs backend configuration
pub struct MistralBackendConfig {
    /// Model path or HuggingFace ID
    pub model_id: String,
    /// Quantization format
    pub quantization: QuantizationFormat,
    /// Use PagedAttention
    pub paged_attention: bool,
    /// KV cache configuration
    pub kv_cache: KvCacheConfig,
    /// Device mapping (for multi-device)
    pub device_map: DeviceMap,
}

/// mistral-rs backend for model inference
pub struct MistralBackend {
    config: MistralBackendConfig,
    /// mistral-rs model pipeline
    pipeline: Arc<MistralPipeline>,
    /// KV cache manager
    kv_cache: Arc<TwoTierKvCache>,
    /// Paged attention manager
    paged_attention: Arc<PagedAttention>,
}

impl MistralBackend {
    /// Load model with SONA-aware caching
    pub async fn load(config: MistralBackendConfig) -> Result<Self> {
        // Create model loader with custom device configuration
        let loader = MistralLoader::new(&config.model_id)
            .with_dtype(config.quantization.dtype())
            .with_device_map(&config.device_map);

        // Load model
        let pipeline = loader.load().await?;

        // Initialize KV cache with existing RuvLLM implementation
        let kv_cache = TwoTierKvCache::new(config.kv_cache.clone());
        let paged_attention = PagedAttention::new(config.paged_attention_config());

        Ok(Self {
            config,
            pipeline: Arc::new(pipeline),
            kv_cache: Arc::new(kv_cache),
            paged_attention: Arc::new(paged_attention),
        })
    }

    /// Forward pass with KV cache integration
    pub fn forward(
        &self,
        tokens: &[u32],
        sequence_id: &str,
        generation_config: &GenerationConfig,
    ) -> Result<GenerationOutput> {
        // Allocate paged attention for this sequence
        self.paged_attention.allocate_sequence(sequence_id, tokens.len())?;

        // Run inference through mistral-rs pipeline
        let output = self.pipeline.forward(tokens, generation_config)?;

        // Update KV cache
        self.kv_cache.append(
            &output.key_cache,
            &output.value_cache,
        )?;

        Ok(output)
    }
}
```

---

## 4. Data Flow for Inference

```
+===========================================================================+
|                        INFERENCE DATA FLOW                                 |
+===========================================================================+

 User Request                                                     Response
      |                                                               ^
      v                                                               |
+-----+-----+                                                   +-----+-----+
| Tokenize  |                                                   | Decode    |
| (HF)      |                                                   | (HF)      |
+-----+-----+                                                   +-----+-----+
      |                                                               ^
      v                                                               |
+-----+-----+     +----------------+     +----------------+     +-----+-----+
| Embedding |---->| SONA Pattern   |---->| Route Decision |---->| Log       |
| Lookup    |     | Lookup         |     | (Model+Quant)  |     | Witness   |
+-----------+     +----------------+     +----------------+     +-----------+
      |                  |                      |
      |    +-------------+                      |
      |    |                                    |
      v    v                                    v
+-----+----+-----+                        +-----+-----+
| Context Prep   |                        | Select    |
| - Retrieve KV  |                        | Backend   |
| - Load LoRA    |                        | (Candle/  |
| - Apply Policy |                        |  Mistral) |
+-----+----------+                        +-----+-----+
      |                                         |
      +------------------+----------------------+
                         |
                         v
              +----------+----------+
              |    NEON Kernels     |
              |    (Attention,      |
              |     RoPE, Norm)     |
              +----------+----------+
                         |
                         v
              +----------+----------+
              | Transformer Layers  |
              | (Loop N times)      |
              +----------+----------+
                         |
                         v
              +----------+----------+
              | Output Projection   |
              | + Sampling          |
              +----------+----------+
                         |
                         v
              +----------+----------+
              | MicroLoRA Update    |
              | (Instant Loop)      |
              +----------+----------+
                         |
                         v
              +----------+----------+
              | Update KV Cache     |
              | (Tiered Storage)    |
              +----------+----------+
                         |
                         v
                    [Output]
```

### 4.1 Detailed Token Processing Flow

```
Token IDs: [1, 234, 567, ...]
              |
              v
    +-------------------+
    | Embedding Layer   |
    | (NEON dot_product)|
    +-------------------+
              |
              v
    +-------------------+
    | RoPE Position     |
    | Encoding (NEON)   |
    +-------------------+
              |
              v
    For each layer (0..N):
    +-------------------+
    | RMSNorm (NEON)    |
    +-------------------+
              |
              v
    +-------------------+
    | Self-Attention    |
    | - Q/K/V Project   |
    | - Paged Attention |
    | - Output Project  |
    +-------------------+
              |
              v
    +-------------------+
    | Feed Forward      |
    | - Gate Project    |
    | - Up Project      |
    | - Down Project    |
    +-------------------+
              |
              v
    +-------------------+
    | MicroLoRA Inject  |
    | (If active)       |
    +-------------------+
              |
              +-- Next Layer --+
                               |
                               v
    +-------------------+
    | Final RMSNorm     |
    +-------------------+
              |
              v
    +-------------------+
    | LM Head Project   |
    +-------------------+
              |
              v
    [Logits]
```

---

## 5. Memory Layout

### 5.1 Unified Memory Architecture (M4 Pro)

```
+===========================================================================+
|                    UNIFIED MEMORY LAYOUT (16GB M4 Pro)                     |
+===========================================================================+

Address Space:
0x0000_0000_0000 +--------------------------------------------------+
                 |  System Reserved (2GB)                            |
0x0000_8000_0000 +--------------------------------------------------+
                 |  Model Weights (4-8GB depending on quantization)  |
                 |  +--------------------------------------------+   |
                 |  | Embedding Matrix (128MB - 512MB)           |   |
                 |  +--------------------------------------------+   |
                 |  | Transformer Layers (N x ~200MB)            |   |
                 |  | - Attention Weights (Q, K, V, O)           |   |
                 |  | - FFN Weights (Gate, Up, Down)             |   |
                 |  +--------------------------------------------+   |
                 |  | LM Head (128MB - 512MB)                    |   |
                 |  +--------------------------------------------+   |
0x0002_0000_0000 +--------------------------------------------------+
                 |  KV Cache Pool (2-4GB)                            |
                 |  +--------------------------------------------+   |
                 |  | Hot Tier (FP16) - 512MB                    |   |
                 |  | - Last 256 tokens per sequence             |   |
                 |  +--------------------------------------------+   |
                 |  | Warm Tier (Q8) - 1GB                       |   |
                 |  | - Tokens 257-2048                          |   |
                 |  +--------------------------------------------+   |
                 |  | Cold Tier (Q4/KIVI) - 1-2GB                |   |
                 |  | - Tokens 2049+                             |   |
                 |  +--------------------------------------------+   |
0x0003_0000_0000 +--------------------------------------------------+
                 |  LoRA Adapter Pool (256MB - 1GB)                  |
                 |  +--------------------------------------------+   |
                 |  | Active Adapters (FP16, ~10MB each)         |   |
                 |  | MicroLoRA Weights (Rank 1-2, ~1MB)         |   |
                 |  | BaseLoRA Weights (Rank 4-8, ~4MB)          |   |
                 |  +--------------------------------------------+   |
0x0003_4000_0000 +--------------------------------------------------+
                 |  Activation Scratch Space (512MB)                 |
                 |  +--------------------------------------------+   |
                 |  | Per-request activations                    |   |
                 |  | Intermediate computations                  |   |
                 |  +--------------------------------------------+   |
0x0003_6000_0000 +--------------------------------------------------+
                 |  Arena Allocator Pool (256MB)                     |
                 |  +--------------------------------------------+   |
                 |  | Batch Vector Allocator                     |   |
                 |  | Temporary SIMD buffers                     |   |
                 |  +--------------------------------------------+   |
0x0003_7000_0000 +--------------------------------------------------+
                 |  SONA Learning State (128MB)                      |
                 |  +--------------------------------------------+   |
                 |  | ReasoningBank Patterns                     |   |
                 |  | EWC++ Fisher Diagonal                      |   |
                 |  | Trajectory Buffer                          |   |
                 |  +--------------------------------------------+   |
0x0003_7800_0000 +--------------------------------------------------+
                 |  Free / Expansion (Remaining)                     |
0x0004_0000_0000 +--------------------------------------------------+
```

### 5.2 KV Cache Memory Layout (Detailed)

```
+===========================================================================+
|                    3-TIER KV CACHE MEMORY LAYOUT                           |
+===========================================================================+

Per-Sequence Layout (4096 context length, 32 KV heads, 128 head dim):

+------------------------+------------------------+------------------------+
|      HOT TIER          |      WARM TIER         |      COLD TIER        |
|      (FP16)            |      (Q8)              |      (Q4/KIVI)        |
+------------------------+------------------------+------------------------+
| Tokens: 3841-4096      | Tokens: 2049-3840      | Tokens: 0-2048        |
| Length: 256 tokens     | Length: 1792 tokens    | Length: 2048 tokens   |
+------------------------+------------------------+------------------------+
| Size per KV head:      | Size per KV head:      | Size per KV head:     |
| 256 * 128 * 2 bytes    | 1792 * 128 * 1 byte    | 2048 * 128 * 0.5 byte |
| = 64KB                 | = 224KB                | = 128KB               |
+------------------------+------------------------+------------------------+
| Total (32 heads):      | Total (32 heads):      | Total (32 heads):     |
| 64KB * 32 * 2 (K+V)    | 224KB * 32 * 2 (K+V)   | 128KB * 32 * 2 (K+V)  |
| = 4MB                  | = 14MB                 | = 8MB                 |
+------------------------+------------------------+------------------------+

Total per sequence: 4MB + 14MB + 8MB = 26MB
With 100 concurrent sequences: 2.6GB

Page Table Structure:
+--------+--------+--------+--------+--------+--------+
| Seq ID | Tier   | Page 0 | Page 1 | Page 2 | ...    |
+--------+--------+--------+--------+--------+--------+
| seq-1  | HOT    | 0x100  | 0x101  | 0x102  | 0x103  |
| seq-1  | WARM   | 0x200  | 0x201  | ...    | ...    |
| seq-1  | COLD   | 0x300  | 0x301  | ...    | ...    |
| seq-2  | HOT    | 0x104  | 0x105  | ...    | ...    |
+--------+--------+--------+--------+--------+--------+
```

---

## 6. NEON Optimization Points

### 6.1 Kernel Registry

```rust
// crates/ruvllm/src/kernels/mod.rs

/// NEON-optimized kernel registry
pub struct NeonKernelRegistry {
    /// Attention kernels
    pub attention: AttentionKernels,
    /// RoPE kernels
    pub rope: RoPEKernels,
    /// Normalization kernels
    pub norm: NormKernels,
    /// Quantization kernels
    pub quant: QuantKernels,
    /// GEMM kernels
    pub gemm: GemmKernels,
}

impl NeonKernelRegistry {
    pub fn new() -> Self {
        Self {
            attention: AttentionKernels::new(),
            rope: RoPEKernels::new(),
            norm: NormKernels::new(),
            quant: QuantKernels::new(),
            gemm: GemmKernels::new(),
        }
    }
}
```

### 6.2 Attention Kernels (NEON)

```rust
// crates/ruvllm/src/kernels/attention.rs

use std::arch::aarch64::*;

/// Flash Attention variant optimized for M4 Pro NEON
pub struct FlashAttentionNeon {
    /// Block size for tiled computation
    block_size: usize,
    /// Softmax scale factor
    scale: f32,
}

impl FlashAttentionNeon {
    /// Compute attention with 4x unrolling (matching simd_intrinsics.rs pattern)
    #[inline(always)]
    pub unsafe fn forward(
        &self,
        query: &[f32],    // [seq_len, num_heads, head_dim]
        key: &[f32],      // [seq_len, num_kv_heads, head_dim]
        value: &[f32],    // [seq_len, num_kv_heads, head_dim]
        output: &mut [f32],
        seq_len: usize,
        num_heads: usize,
        num_kv_heads: usize,
        head_dim: usize,
    ) {
        let gqa_ratio = num_heads / num_kv_heads;
        let scale = self.scale;

        // For each query head
        for h in 0..num_heads {
            let kv_head = h / gqa_ratio;

            // Tiled attention computation
            for q_block_start in (0..seq_len).step_by(self.block_size) {
                let q_block_end = (q_block_start + self.block_size).min(seq_len);

                for k_block_start in (0..seq_len).step_by(self.block_size) {
                    let k_block_end = (k_block_start + self.block_size).min(seq_len);

                    // Compute QK^T for this tile
                    self.compute_attention_tile(
                        query, key, value, output,
                        q_block_start, q_block_end,
                        k_block_start, k_block_end,
                        h, kv_head, head_dim, scale,
                    );
                }
            }
        }
    }

    #[inline(always)]
    unsafe fn compute_attention_tile(
        &self,
        query: &[f32],
        key: &[f32],
        value: &[f32],
        output: &mut [f32],
        q_start: usize, q_end: usize,
        k_start: usize, k_end: usize,
        head: usize, kv_head: usize,
        head_dim: usize, scale: f32,
    ) {
        // Use 4 accumulators for better ILP (matching simd_intrinsics.rs)
        let mut sum0 = vdupq_n_f32(0.0);
        let mut sum1 = vdupq_n_f32(0.0);
        let mut sum2 = vdupq_n_f32(0.0);
        let mut sum3 = vdupq_n_f32(0.0);

        let scale_vec = vdupq_n_f32(scale);

        // Process head_dim in chunks of 16 (4x4 unrolling)
        let chunks = head_dim / 16;

        for q_pos in q_start..q_end {
            let q_offset = (q_pos * head_dim) + (head * head_dim);
            let q_ptr = query.as_ptr().add(q_offset);

            let mut max_score = f32::NEG_INFINITY;
            let mut scores = Vec::with_capacity(k_end - k_start);

            // Compute attention scores
            for k_pos in k_start..k_end {
                let k_offset = (k_pos * head_dim) + (kv_head * head_dim);
                let k_ptr = key.as_ptr().add(k_offset);

                // Reset accumulators
                sum0 = vdupq_n_f32(0.0);
                sum1 = vdupq_n_f32(0.0);
                sum2 = vdupq_n_f32(0.0);
                sum3 = vdupq_n_f32(0.0);

                let mut idx = 0;
                for _ in 0..chunks {
                    // Load Q vectors
                    let q0 = vld1q_f32(q_ptr.add(idx));
                    let q1 = vld1q_f32(q_ptr.add(idx + 4));
                    let q2 = vld1q_f32(q_ptr.add(idx + 8));
                    let q3 = vld1q_f32(q_ptr.add(idx + 12));

                    // Load K vectors
                    let k0 = vld1q_f32(k_ptr.add(idx));
                    let k1 = vld1q_f32(k_ptr.add(idx + 4));
                    let k2 = vld1q_f32(k_ptr.add(idx + 8));
                    let k3 = vld1q_f32(k_ptr.add(idx + 12));

                    // FMA: sum += q * k
                    sum0 = vfmaq_f32(sum0, q0, k0);
                    sum1 = vfmaq_f32(sum1, q1, k1);
                    sum2 = vfmaq_f32(sum2, q2, k2);
                    sum3 = vfmaq_f32(sum3, q3, k3);

                    idx += 16;
                }

                // Tree reduction
                let sum01 = vaddq_f32(sum0, sum1);
                let sum23 = vaddq_f32(sum2, sum3);
                let sum = vaddq_f32(sum01, sum23);

                // Horizontal sum + scale
                let score = vaddvq_f32(vmulq_f32(sum, scale_vec));
                scores.push(score);
                max_score = max_score.max(score);
            }

            // Online softmax + value accumulation
            self.softmax_and_accumulate(
                &scores, max_score, value, output,
                q_pos, k_start, k_end, kv_head, head_dim, head,
            );
        }
    }
}
```

### 6.3 RoPE Kernels (NEON)

```rust
// crates/ruvllm/src/kernels/rope.rs

use std::arch::aarch64::*;

/// Rotary Position Embedding optimized for NEON
pub struct RoPENeon {
    /// Precomputed cos table
    cos_cache: Vec<f32>,
    /// Precomputed sin table
    sin_cache: Vec<f32>,
    /// Maximum sequence length
    max_seq_len: usize,
    /// Head dimension
    head_dim: usize,
}

impl RoPENeon {
    pub fn new(max_seq_len: usize, head_dim: usize, base: f32) -> Self {
        let half_dim = head_dim / 2;
        let mut cos_cache = vec![0.0; max_seq_len * half_dim];
        let mut sin_cache = vec![0.0; max_seq_len * half_dim];

        // Precompute frequencies
        for pos in 0..max_seq_len {
            for i in 0..half_dim {
                let freq = 1.0 / base.powf((2 * i) as f32 / head_dim as f32);
                let angle = pos as f32 * freq;
                cos_cache[pos * half_dim + i] = angle.cos();
                sin_cache[pos * half_dim + i] = angle.sin();
            }
        }

        Self { cos_cache, sin_cache, max_seq_len, head_dim }
    }

    /// Apply RoPE to query/key tensors in-place
    #[inline(always)]
    pub unsafe fn apply(
        &self,
        tensor: &mut [f32],
        positions: &[usize],
        num_heads: usize,
    ) {
        let half_dim = self.head_dim / 2;
        let chunks = half_dim / 4;

        for (seq_idx, &pos) in positions.iter().enumerate() {
            let cos_ptr = self.cos_cache.as_ptr().add(pos * half_dim);
            let sin_ptr = self.sin_cache.as_ptr().add(pos * half_dim);

            for head in 0..num_heads {
                let base_offset = (seq_idx * num_heads + head) * self.head_dim;
                let tensor_ptr = tensor.as_mut_ptr().add(base_offset);

                let mut idx = 0;
                for _ in 0..chunks {
                    // Load first half (x0)
                    let x0 = vld1q_f32(tensor_ptr.add(idx));
                    // Load second half (x1)
                    let x1 = vld1q_f32(tensor_ptr.add(idx + half_dim));

                    // Load cos/sin
                    let cos = vld1q_f32(cos_ptr.add(idx));
                    let sin = vld1q_f32(sin_ptr.add(idx));

                    // Apply rotation: [x0*cos - x1*sin, x0*sin + x1*cos]
                    let neg_sin = vnegq_f32(sin);
                    let new_x0 = vfmaq_f32(vmulq_f32(x0, cos), x1, neg_sin);
                    let new_x1 = vfmaq_f32(vmulq_f32(x0, sin), x1, cos);

                    // Store results
                    vst1q_f32(tensor_ptr.add(idx), new_x0);
                    vst1q_f32(tensor_ptr.add(idx + half_dim), new_x1);

                    idx += 4;
                }
            }
        }
    }
}
```

### 6.4 RMSNorm Kernel (NEON)

```rust
// crates/ruvllm/src/kernels/norm.rs

use std::arch::aarch64::*;

/// RMSNorm optimized for NEON
pub struct RMSNormNeon {
    /// Weight vector (gamma)
    weight: Vec<f32>,
    /// Epsilon for numerical stability
    eps: f32,
}

impl RMSNormNeon {
    /// Apply RMSNorm in-place
    #[inline(always)]
    pub unsafe fn forward(&self, x: &mut [f32], hidden_size: usize) {
        let num_tokens = x.len() / hidden_size;

        for token_idx in 0..num_tokens {
            let offset = token_idx * hidden_size;
            let x_ptr = x.as_mut_ptr().add(offset);
            let w_ptr = self.weight.as_ptr();

            // Compute variance (mean of squares)
            let mut var0 = vdupq_n_f32(0.0);
            let mut var1 = vdupq_n_f32(0.0);
            let mut var2 = vdupq_n_f32(0.0);
            let mut var3 = vdupq_n_f32(0.0);

            let chunks = hidden_size / 16;
            let mut idx = 0;

            for _ in 0..chunks {
                let v0 = vld1q_f32(x_ptr.add(idx));
                let v1 = vld1q_f32(x_ptr.add(idx + 4));
                let v2 = vld1q_f32(x_ptr.add(idx + 8));
                let v3 = vld1q_f32(x_ptr.add(idx + 12));

                var0 = vfmaq_f32(var0, v0, v0);
                var1 = vfmaq_f32(var1, v1, v1);
                var2 = vfmaq_f32(var2, v2, v2);
                var3 = vfmaq_f32(var3, v3, v3);

                idx += 16;
            }

            // Tree reduction
            let var01 = vaddq_f32(var0, var1);
            let var23 = vaddq_f32(var2, var3);
            let var = vaddq_f32(var01, var23);
            let variance = vaddvq_f32(var) / hidden_size as f32;

            // Compute scale: 1/sqrt(variance + eps)
            let scale = 1.0 / (variance + self.eps).sqrt();
            let scale_vec = vdupq_n_f32(scale);

            // Apply normalization and weight
            idx = 0;
            for _ in 0..chunks {
                let v0 = vld1q_f32(x_ptr.add(idx));
                let v1 = vld1q_f32(x_ptr.add(idx + 4));
                let v2 = vld1q_f32(x_ptr.add(idx + 8));
                let v3 = vld1q_f32(x_ptr.add(idx + 12));

                let w0 = vld1q_f32(w_ptr.add(idx));
                let w1 = vld1q_f32(w_ptr.add(idx + 4));
                let w2 = vld1q_f32(w_ptr.add(idx + 8));
                let w3 = vld1q_f32(w_ptr.add(idx + 12));

                let out0 = vmulq_f32(vmulq_f32(v0, scale_vec), w0);
                let out1 = vmulq_f32(vmulq_f32(v1, scale_vec), w1);
                let out2 = vmulq_f32(vmulq_f32(v2, scale_vec), w2);
                let out3 = vmulq_f32(vmulq_f32(v3, scale_vec), w3);

                vst1q_f32(x_ptr.add(idx), out0);
                vst1q_f32(x_ptr.add(idx + 4), out1);
                vst1q_f32(x_ptr.add(idx + 8), out2);
                vst1q_f32(x_ptr.add(idx + 12), out3);

                idx += 16;
            }
        }
    }
}
```

---

## 7. MicroLoRA Integration

### 7.1 MicroLoRA Architecture

```
+===========================================================================+
|                    MICROLORA REAL-TIME ADAPTATION                          |
+===========================================================================+

                        +-------------------+
                        |  Input Activation |
                        |  x: [batch, dim]  |
                        +---------+---------+
                                  |
        +-------------------------+-------------------------+
        |                         |                         |
        v                         v                         v
+-------+-------+         +-------+-------+         +-------+-------+
| Base Weight   |         | MicroLoRA A   |         | MicroLoRA B   |
| W: [out, in]  |         | A: [rank, in] |         | B: [out, rank]|
| (Frozen)      |         | (Rank 1-2)    |         | (Rank 1-2)    |
+-------+-------+         +-------+-------+         +-------+-------+
        |                         |                         |
        v                         +----------+--------------+
   +----+----+                               |
   | W @ x   |                               v
   +---------+                    +----------+----------+
        |                         | scale * B @ (A @ x) |
        |                         +----------+----------+
        +-------------+------------------------+
                      |
                      v
              +-------+-------+
              | y = Wx + sBAx |
              +---------------+
```

### 7.2 MicroLoRA Implementation

```rust
// crates/ruvllm/src/lora/micro_lora.rs

/// MicroLoRA for per-request real-time adaptation
pub struct MicroLoRA {
    /// Config
    config: MicroLoRAConfig,
    /// A matrices per layer: [num_layers, rank, hidden_dim]
    a_matrices: Vec<Vec<f32>>,
    /// B matrices per layer: [num_layers, hidden_dim, rank]
    b_matrices: Vec<Vec<f32>>,
    /// Scale factor
    scale: f32,
    /// Gradient accumulators for instant learning
    grad_a: Vec<Vec<f32>>,
    grad_b: Vec<Vec<f32>>,
}

/// MicroLoRA configuration
pub struct MicroLoRAConfig {
    /// LoRA rank (typically 1-2 for instant learning)
    pub rank: usize,
    /// Hidden dimension
    pub hidden_dim: usize,
    /// Number of layers
    pub num_layers: usize,
    /// Learning rate for instant updates
    pub learning_rate: f32,
    /// Scale factor (alpha / rank)
    pub scale: f32,
    /// Apply to which modules
    pub target_modules: TargetModules,
}

#[derive(Clone, Copy)]
pub enum TargetModules {
    /// Query and Value projections only
    QV,
    /// All attention projections
    QKVO,
    /// All linear layers
    All,
}

impl MicroLoRA {
    pub fn new(config: MicroLoRAConfig) -> Self {
        let num_layers = config.num_layers;
        let rank = config.rank;
        let hidden_dim = config.hidden_dim;

        // Initialize with small random values (Xavier)
        let mut rng = rand::thread_rng();
        let std_a = (2.0 / (hidden_dim + rank) as f32).sqrt();
        let std_b = 0.0; // B initialized to zero

        let a_matrices: Vec<Vec<f32>> = (0..num_layers)
            .map(|_| {
                (0..rank * hidden_dim)
                    .map(|_| rng.gen::<f32>() * std_a)
                    .collect()
            })
            .collect();

        let b_matrices: Vec<Vec<f32>> = (0..num_layers)
            .map(|_| vec![std_b; hidden_dim * rank])
            .collect();

        let grad_a = vec![vec![0.0; rank * hidden_dim]; num_layers];
        let grad_b = vec![vec![0.0; hidden_dim * rank]; num_layers];

        Self {
            scale: config.scale,
            config,
            a_matrices,
            b_matrices,
            grad_a,
            grad_b,
        }
    }

    /// Forward pass: adds LoRA contribution to base output
    #[inline(always)]
    pub fn forward(
        &self,
        x: &[f32],           // Input: [batch_size, hidden_dim]
        base_output: &mut [f32], // Base output to modify in-place
        layer_idx: usize,
        batch_size: usize,
    ) {
        let rank = self.config.rank;
        let hidden_dim = self.config.hidden_dim;

        let a = &self.a_matrices[layer_idx];
        let b = &self.b_matrices[layer_idx];

        // Compute A @ x -> [batch_size, rank]
        let mut ax = vec![0.0; batch_size * rank];
        for batch in 0..batch_size {
            for r in 0..rank {
                let mut sum = 0.0;
                for d in 0..hidden_dim {
                    sum += a[r * hidden_dim + d] * x[batch * hidden_dim + d];
                }
                ax[batch * rank + r] = sum;
            }
        }

        // Compute B @ (A @ x) and add to base_output
        for batch in 0..batch_size {
            for d in 0..hidden_dim {
                let mut sum = 0.0;
                for r in 0..rank {
                    sum += b[d * rank + r] * ax[batch * rank + r];
                }
                base_output[batch * hidden_dim + d] += self.scale * sum;
            }
        }
    }

    /// Instant update from trajectory (SONA instant loop)
    pub fn instant_update(
        &mut self,
        input: &[f32],
        grad_output: &[f32],
        layer_idx: usize,
        quality_score: f32,
    ) {
        let rank = self.config.rank;
        let hidden_dim = self.config.hidden_dim;
        let lr = self.config.learning_rate * quality_score; // Scale by quality

        // Compute gradients
        // grad_B = grad_output @ (A @ input)^T
        // grad_A = B^T @ grad_output @ input^T

        // Simplified single-sample update
        let a = &self.a_matrices[layer_idx];
        let b = &mut self.b_matrices[layer_idx];

        // A @ input -> [rank]
        let mut ax = vec![0.0; rank];
        for r in 0..rank {
            let mut sum = 0.0;
            for d in 0..hidden_dim {
                sum += a[r * hidden_dim + d] * input[d];
            }
            ax[r] = sum;
        }

        // Update B: grad_B[d, r] = grad_output[d] * ax[r]
        for d in 0..hidden_dim {
            for r in 0..rank {
                let grad = grad_output[d] * ax[r];
                b[d * rank + r] -= lr * grad;
            }
        }

        // Update A: grad_A[r, d] = sum_d'(B[d', r] * grad_output[d']) * input[d]
        let a = &mut self.a_matrices[layer_idx];
        for r in 0..rank {
            let mut b_grad_sum = 0.0;
            for d in 0..hidden_dim {
                b_grad_sum += self.b_matrices[layer_idx][d * rank + r] * grad_output[d];
            }
            for d in 0..hidden_dim {
                let grad = b_grad_sum * input[d];
                a[r * hidden_dim + d] -= lr * grad;
            }
        }
    }
}
```

### 7.3 LoRA Adapter Manager

```rust
// crates/ruvllm/src/lora/adapter.rs

/// LoRA adapter management with hot-swapping
pub struct LoRAAdapterManager {
    /// Active MicroLoRA (per-request)
    micro_lora: Arc<RwLock<MicroLoRA>>,
    /// Base LoRA adapters (shared across requests)
    base_adapters: DashMap<String, Arc<BaseLoRAAdapter>>,
    /// Adapter residency manager
    residency: AdapterResidencyManager,
    /// Memory pool for adapter weights
    memory_pool: Arc<UnifiedMemoryPool>,
}

/// Base LoRA adapter (rank 4-8, trained in background loop)
pub struct BaseLoRAAdapter {
    pub id: String,
    pub rank: usize,
    pub a_matrices: Vec<Vec<f32>>,
    pub b_matrices: Vec<Vec<f32>>,
    pub scale: f32,
    pub precision: Precision,
    pub last_access: AtomicU64,
    pub access_count: AtomicU64,
}

impl LoRAAdapterManager {
    /// Load adapter from storage with tier management
    pub async fn load_adapter(&self, adapter_id: &str) -> Result<Arc<BaseLoRAAdapter>> {
        // Check if already loaded
        if let Some(adapter) = self.base_adapters.get(adapter_id) {
            adapter.access_count.fetch_add(1, Ordering::Relaxed);
            adapter.last_access.store(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                Ordering::Relaxed,
            );
            return Ok(adapter.clone());
        }

        // Load from appropriate tier
        let adapter = self.residency.load(adapter_id).await?;
        let adapter = Arc::new(adapter);
        self.base_adapters.insert(adapter_id.to_string(), adapter.clone());

        Ok(adapter)
    }

    /// Merge MicroLoRA into Base LoRA (background loop)
    pub fn merge_micro_to_base(&self, base_adapter_id: &str, quality_threshold: f32) {
        let micro = self.micro_lora.read();

        if let Some(mut base) = self.base_adapters.get_mut(base_adapter_id) {
            // Only merge if recent trajectories exceed quality threshold
            // This is handled by SONA's trajectory filtering
            for layer_idx in 0..micro.config.num_layers {
                for (i, (micro_a, base_a)) in micro.a_matrices[layer_idx]
                    .iter()
                    .zip(base.a_matrices[layer_idx].iter_mut())
                    .enumerate()
                {
                    // Exponential moving average merge
                    *base_a = 0.99 * *base_a + 0.01 * micro_a;
                }
                for (i, (micro_b, base_b)) in micro.b_matrices[layer_idx]
                    .iter()
                    .zip(base.b_matrices[layer_idx].iter_mut())
                    .enumerate()
                {
                    *base_b = 0.99 * *base_b + 0.01 * micro_b;
                }
            }
        }
    }
}
```

---

## 8. SONA-LLM Integration

### 8.1 SONA LLM Configuration

```rust
// crates/ruvllm/src/optimization/sona_llm.rs

/// SONA integration specifically for LLM operations
pub struct SonaLLM {
    /// Core SONA integration
    sona: Arc<SonaIntegration>,
    /// MicroLoRA manager
    micro_lora: Arc<RwLock<MicroLoRA>>,
    /// KV cache policy learning
    kv_policy_learner: KvPolicyLearner,
    /// Router learning
    router_learner: RouterLearner,
}

impl SonaLLM {
    /// Record LLM trajectory for learning
    pub fn record_llm_trajectory(
        &self,
        request_id: &str,
        session_id: &str,
        input_tokens: &[u32],
        output_tokens: &[u32],
        quality_score: f32,
        latency_ms: f32,
        model_used: ModelSize,
        kv_cache_stats: &KvCacheStats,
    ) -> Result<()> {
        // Compute embeddings
        let query_embedding = self.compute_embedding(input_tokens)?;
        let response_embedding = self.compute_embedding(output_tokens)?;

        // Create trajectory
        let trajectory = Trajectory {
            request_id: request_id.to_string(),
            session_id: session_id.to_string(),
            query_embedding,
            response_embedding,
            quality_score,
            routing_features: vec![
                latency_ms / 1000.0, // Normalize
                kv_cache_stats.compression_ratio,
                kv_cache_stats.total_tokens as f32 / 4096.0,
                model_used.index() as f32 / 4.0,
            ],
            model_index: model_used.index(),
            timestamp: chrono::Utc::now(),
        };

        // Record in SONA
        self.sona.record_trajectory(trajectory)?;

        // Update MicroLoRA if quality is good
        if quality_score >= 0.7 {
            self.update_micro_lora(&query_embedding, quality_score)?;
        }

        // Update KV cache policy
        self.kv_policy_learner.update(kv_cache_stats, quality_score);

        Ok(())
    }

    /// Get routing recommendation for new request
    pub fn get_llm_routing(&self, input_embedding: &[f32]) -> LLMRoutingDecision {
        // Get base SONA recommendation
        let base_rec = self.sona.get_routing_recommendation(input_embedding);

        // Get router learner recommendation
        let router_rec = self.router_learner.recommend(input_embedding);

        // Get KV cache policy recommendation
        let kv_rec = self.kv_policy_learner.recommend(input_embedding);

        LLMRoutingDecision {
            model: base_rec.suggested_model,
            confidence: (base_rec.confidence + router_rec.confidence) / 2.0,
            kv_quantization: kv_rec.quantization,
            kv_tail_length: kv_rec.tail_length,
            use_micro_lora: base_rec.average_quality > 0.6,
        }
    }
}

/// LLM-specific routing decision
pub struct LLMRoutingDecision {
    /// Model size to use (0=tiny, 1=small, 2=medium, 3=large)
    pub model: usize,
    /// Confidence in decision
    pub confidence: f32,
    /// KV cache quantization level
    pub kv_quantization: Precision,
    /// KV cache tail length (high-precision)
    pub kv_tail_length: usize,
    /// Whether to apply MicroLoRA
    pub use_micro_lora: bool,
}
```

### 8.2 Real-Time Optimization Loop

```rust
// crates/ruvllm/src/optimization/realtime.rs

/// Real-time optimization during inference
pub struct RealtimeOptimizer {
    /// SONA LLM integration
    sona_llm: Arc<SonaLLM>,
    /// Performance monitor
    perf_monitor: PerformanceMonitor,
    /// Optimization triggers
    triggers: OptimizationTriggers,
}

#[derive(Clone)]
pub struct OptimizationTriggers {
    /// Trigger MicroLoRA update after N requests
    pub micro_lora_update_interval: usize,
    /// Trigger KV cache rebalance at memory threshold
    pub kv_rebalance_threshold: f32,
    /// Trigger router update after N trajectories
    pub router_update_interval: usize,
}

impl RealtimeOptimizer {
    /// Called before each forward pass
    pub fn pre_forward(&self, request: &InferenceRequest) -> ForwardConfig {
        // Get SONA routing decision
        let routing = self.sona_llm.get_llm_routing(&request.input_embedding);

        // Check if real-time adjustments needed
        let perf = self.perf_monitor.current_metrics();

        ForwardConfig {
            model_index: routing.model,
            use_micro_lora: routing.use_micro_lora,
            kv_config: KvConfig {
                quantization: if perf.memory_pressure > 0.9 {
                    Precision::Q4 // Aggressive compression under pressure
                } else {
                    routing.kv_quantization
                },
                tail_length: routing.kv_tail_length,
            },
            batch_optimization: perf.throughput < 50.0, // tokens/sec
        }
    }

    /// Called after each forward pass
    pub fn post_forward(&self, result: &InferenceResult) {
        // Record trajectory
        self.sona_llm.record_llm_trajectory(
            &result.request_id,
            &result.session_id,
            &result.input_tokens,
            &result.output_tokens,
            result.quality_score,
            result.latency_ms,
            result.model_used,
            &result.kv_stats,
        ).ok();

        // Update performance monitor
        self.perf_monitor.record(result);

        // Check optimization triggers
        if self.should_trigger_micro_lora_update() {
            self.trigger_micro_lora_merge();
        }

        if self.should_trigger_kv_rebalance() {
            self.trigger_kv_rebalance();
        }
    }
}
```

---

## 9. API Design

### 9.1 Public API

```rust
// crates/ruvllm/src/engine.rs (to be added)

/// Main inference engine combining all components
pub struct LLMInferenceEngine {
    /// Configuration
    config: LLMInferenceConfig,
    /// Backend (Candle, mistral-rs, or Hybrid)
    backend: Box<dyn InferenceBackend>,
    /// SONA LLM integration
    sona_llm: Arc<SonaLLM>,
    /// Real-time optimizer
    optimizer: Arc<RealtimeOptimizer>,
    /// KV cache manager
    kv_cache: Arc<TwoTierKvCache>,
    /// Paged attention manager
    paged_attention: Arc<PagedAttention>,
    /// LoRA adapter manager
    lora_manager: Arc<LoRAAdapterManager>,
    /// Session manager
    session_manager: SessionManager,
}

/// Engine configuration
pub struct LLMInferenceConfig {
    /// Backend type
    pub backend: BackendType,
    /// Model configuration
    pub model: ModelConfig,
    /// Memory configuration
    pub memory: MemoryConfig,
    /// SONA configuration
    pub sona: SonaConfig,
    /// KV cache configuration
    pub kv_cache: KvCacheConfig,
    /// LoRA configuration
    pub lora: LoRAConfig,
}

#[derive(Clone)]
pub enum BackendType {
    Candle(CandleBackendConfig),
    MistralRs(MistralBackendConfig),
    Hybrid {
        candle: CandleBackendConfig,
        mistral: MistralBackendConfig,
        routing: HybridRoutingConfig,
    },
}

impl LLMInferenceEngine {
    /// Create a new inference engine
    pub async fn new(config: LLMInferenceConfig) -> Result<Self> {
        let backend: Box<dyn InferenceBackend> = match &config.backend {
            BackendType::Candle(cfg) => Box::new(CandleBackend::new(cfg.clone())?),
            BackendType::MistralRs(cfg) => Box::new(MistralBackend::load(cfg.clone()).await?),
            BackendType::Hybrid { candle, mistral, routing } => {
                Box::new(HybridBackend::new(candle.clone(), mistral.clone(), routing.clone()).await?)
            }
        };

        // Initialize components
        let sona_llm = Arc::new(SonaLLM::new(config.sona.clone())?);
        let optimizer = Arc::new(RealtimeOptimizer::new(sona_llm.clone()));
        let kv_cache = Arc::new(TwoTierKvCache::new(config.kv_cache.clone()));
        let paged_attention = Arc::new(PagedAttention::new(config.kv_cache.into()));
        let lora_manager = Arc::new(LoRAAdapterManager::new(config.lora.clone()));
        let session_manager = SessionManager::new(config.session.clone());

        Ok(Self {
            config,
            backend,
            sona_llm,
            optimizer,
            kv_cache,
            paged_attention,
            lora_manager,
            session_manager,
        })
    }

    /// Run inference
    pub async fn generate(
        &self,
        request: GenerationRequest,
    ) -> Result<GenerationResponse> {
        // Get or create session
        let session = self.session_manager
            .get_or_create(&request.session_id)?;

        // Pre-forward optimization
        let forward_config = self.optimizer.pre_forward(&request.into());

        // Load LoRA adapter if specified
        if let Some(adapter_id) = &request.adapter_id {
            self.lora_manager.load_adapter(adapter_id).await?;
        }

        // Run generation
        let start = std::time::Instant::now();
        let output = self.backend.generate(&request, &forward_config, &session).await?;
        let latency_ms = start.elapsed().as_secs_f32() * 1000.0;

        // Post-forward optimization
        let result = InferenceResult {
            request_id: request.request_id.clone(),
            session_id: session.id.clone(),
            input_tokens: request.input_ids.clone(),
            output_tokens: output.token_ids.clone(),
            quality_score: output.quality_estimate,
            latency_ms,
            model_used: forward_config.model_index.into(),
            kv_stats: self.kv_cache.stats(),
        };
        self.optimizer.post_forward(&result);

        Ok(GenerationResponse {
            request_id: request.request_id,
            generated_text: output.text,
            token_ids: output.token_ids,
            latency_ms,
            tokens_per_second: output.token_ids.len() as f32 / (latency_ms / 1000.0),
        })
    }
}

/// Generation request
pub struct GenerationRequest {
    pub request_id: String,
    pub session_id: Option<String>,
    pub prompt: String,
    pub input_ids: Vec<u32>,
    pub max_new_tokens: usize,
    pub temperature: f32,
    pub top_p: f32,
    pub adapter_id: Option<String>,
}

/// Generation response
pub struct GenerationResponse {
    pub request_id: String,
    pub generated_text: String,
    pub token_ids: Vec<u32>,
    pub latency_ms: f32,
    pub tokens_per_second: f32,
}
```

---

## 10. Cargo.toml Dependencies

```toml
# crates/ruvllm/Cargo.toml (additions to existing)

[package]
name = "ruvllm-integration"
version.workspace = true
edition.workspace = true
# ... existing fields ...

[dependencies]
# Existing dependencies
ruvector-core = { path = "../ruvector-core", default-features = false, features = ["storage"] }
ruvector-sona = { path = "../sona", default-features = false, features = ["serde-support"] }

# Candle - Tensor operations
candle-core = { version = "0.8", features = ["metal"] }
candle-nn = { version = "0.8" }
candle-transformers = { version = "0.8" }

# mistral-rs - Model inference (optional, for hybrid mode)
mistralrs = { version = "0.6", optional = true, features = ["metal", "flash-attn"] }
mistralrs-core = { version = "0.6", optional = true }

# Tokenizers
tokenizers = { version = "0.20", features = ["http"] }
hf-hub = { version = "0.3" }

# Async runtime
tokio = { workspace = true, features = ["rt-multi-thread", "sync", "macros"] }
futures = "0.3"

# Serialization
serde = { workspace = true }
serde_json = { workspace = true }

# Error handling
thiserror = { workspace = true }
anyhow = { workspace = true }
tracing = { workspace = true }

# Performance
dashmap = { workspace = true }
parking_lot = { workspace = true }
once_cell = { workspace = true }

# Time and UUID
chrono = { workspace = true, features = ["serde"] }
uuid = { workspace = true, features = ["v4", "serde"] }

# Math
ndarray = { workspace = true }
rand = { workspace = true }
half = { version = "2.4", features = ["std"] }  # For f16 support

# Memory mapping (for model loading)
memmap2 = "0.9"
bytemuck = { version = "1.18", features = ["derive"] }

[dev-dependencies]
criterion = { workspace = true, features = ["html_reports"] }
tempfile = "3.13"
tracing-subscriber = { workspace = true }
approx = "0.5"

[features]
default = ["async-runtime", "candle-backend"]
async-runtime = ["tokio"]
candle-backend = []
mistral-backend = ["mistralrs", "mistralrs-core"]
hybrid-backend = ["candle-backend", "mistral-backend"]
metal = ["candle-core/metal"]
wasm = []

[[bench]]
name = "attention_benchmarks"
harness = false

[[bench]]
name = "lora_benchmarks"
harness = false
```

---

## 11. Module Structure (Final)

```
crates/ruvllm/src/
+-- lib.rs                    # (modify) Add new module exports
+-- engine.rs                 # NEW: Main LLM inference engine
|
+-- backends/
|   +-- mod.rs                # NEW: Backend trait and selection
|   +-- candle.rs             # NEW: Candle tensor backend
|   +-- mistral.rs            # NEW: mistral-rs model backend
|   +-- hybrid.rs             # NEW: Hybrid routing backend
|
+-- lora/
|   +-- mod.rs                # NEW: LoRA module exports
|   +-- micro_lora.rs         # NEW: MicroLoRA implementation
|   +-- base_lora.rs          # NEW: Base LoRA adapters
|   +-- adapter.rs            # NEW: Adapter manager
|   +-- residency.rs          # NEW: Tier management
|
+-- kernels/
|   +-- mod.rs                # NEW: Kernel registry
|   +-- attention.rs          # NEW: Flash/Paged attention NEON
|   +-- rope.rs               # NEW: RoPE NEON implementation
|   +-- norm.rs               # NEW: RMSNorm/LayerNorm NEON
|   +-- quantize.rs           # NEW: Quantization kernels
|   +-- gemm.rs               # NEW: GEMM kernels (optional)
|
+-- optimization/
|   +-- mod.rs                # NEW: Optimization exports
|   +-- sona_llm.rs           # NEW: SONA LLM integration
|   +-- realtime.rs           # NEW: Real-time optimization
|   +-- policy.rs             # NEW: KV/Router policy learning
|
+-- adapter_manager.rs        # (existing) Modify for new LoRA
+-- error.rs                  # (existing)
+-- kv_cache.rs               # (existing) Enhance with 3-tier
+-- paged_attention.rs        # (existing)
+-- policy_store.rs           # (existing)
+-- session.rs                # (existing)
+-- session_index.rs          # (existing)
+-- sona.rs                   # (existing)
+-- types.rs                  # (existing) Add new types
+-- witness_log.rs            # (existing)
```

---

## 12. Performance Targets

| Operation | Target | Hardware Optimization |
|-----------|--------|----------------------|
| Attention (256 seq) | <2ms | NEON 4x unrolling, Flash tiling |
| RoPE | <0.1ms | Precomputed tables, NEON vectorization |
| RMSNorm | <0.05ms | NEON tree reduction |
| MicroLoRA forward | <0.5ms | Rank 1-2, NEON matmul |
| MicroLoRA update | <1ms | Sparse gradient, instant loop |
| KV append (hot tier) | <0.1ms | Zero-copy append |
| KV migration (hot->warm) | <1ms | Batch quantization |
| Model load (7B Q4) | <30s | mmap, lazy loading |
| TTFT | <50ms | Paged attention, continuous batching |
| Throughput | 100+ tok/s | Batch optimization, prefetching |

---

## 13. Risk Analysis

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Metal compatibility issues | Medium | High | Fallback to CPU NEON |
| Memory pressure at scale | Medium | High | Aggressive KV quantization, eviction |
| mistral-rs API changes | Low | Medium | Version pinning, abstraction layer |
| MicroLoRA quality degradation | Medium | Medium | EWC++, quality thresholds |
| Backend switching overhead | Low | Low | Warm-start caching |

---

## 14. References

1. [Candle Documentation](https://huggingface.co/docs/candle)
2. [mistral-rs GitHub](https://github.com/EricLBuehler/mistral.rs)
3. [Flash Attention Paper](https://arxiv.org/abs/2205.14135)
4. [S-LoRA Paper](https://arxiv.org/abs/2311.03285)
5. [KIVI: 2-bit KV Cache Quantization](https://arxiv.org/abs/2402.02750)
6. ADR-002: RuvLLM Integration with Ruvector
7. ADR-006: Unified Memory Pool and Paging Strategy

---

**Document Status**: Ready for Implementation Review
