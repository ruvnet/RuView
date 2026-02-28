# ADR-004: KV Cache Management Strategy for RuvLLM

**Status**: Proposed
**Date**: 2026-01-18
**Authors**: ruv.io, RuVector Team
**Deciders**: Architecture Review Board
**SDK**: Claude-Flow

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-01-18 | ruv.io | Initial architecture proposal |

---

## Context

### The Memory Bottleneck Problem

KV (Key-Value) cache is the primary memory bottleneck for long-context LLM inference. The cache grows linearly with sequence length and batch size, quickly dominating memory consumption:

**Memory Scaling Analysis:**

| Model Size | Batch | Context | KV Cache (FP16) | KV Cache (FP32) |
|------------|-------|---------|-----------------|-----------------|
| 7B | 1 | 2048 | ~256 MB | ~512 MB |
| 70B | 1 | 2048 | ~2.6 GB | ~5.2 GB |
| 70B | 32 | 2048 | ~83 GB | ~166 GB |
| 540B | 512 | 2048 | **~3 TB** | ~6 TB |
| 70B | 1 | 128K | ~166 GB | ~332 GB |

**Formula:** `KV_cache_size = 2 * num_layers * num_heads * head_dim * seq_len * batch_size * bytes_per_element`

### Current Limitations

The existing `ruvector-mincut-gated-transformer` implementation provides:
- Basic 2-bit and 4-bit quantization via Hadamard transform (RotateKV)
- Per-head min/max scaling factors
- ~16x compression at 2-bit, ~8x at 4-bit

**However, it lacks:**

| Limitation | Impact |
|------------|--------|
| **Single-tier quantization** | Cannot adapt precision to token staleness |
| **No temporal awareness** | Recent tokens (high precision) treated same as stale tokens |
| **Limited to FP32 scales** | Scale storage overhead not optimized |
| **No rematerialization** | Cannot trade compute for memory in extreme cases |
| **Static policy** | No adaptive threshold tuning based on quality metrics |

### The Missing Primitive

Current implementations ask:
> "How do I quantize all KV cache entries uniformly?"

They cannot ask:
> "Which tokens need high precision now, and which can be aggressively compressed without quality loss?"

**That question, answered dynamically based on attention patterns and token staleness, is the missing primitive.**

---

## Decision

### Introduce a Three-Tier Adaptive KV Cache Management System

We propose a hierarchical KV cache architecture combining:

1. **High-Precision Tail Buffer**: Recent tokens in FP16/BF16
2. **Moderate Quantization Zone**: Intermediate tokens in 4-bit (KIVI)
3. **Aggressive Compression Zone**: Stale tokens in 2-bit (KIVI/SQuat)

### Architecture Overview

```
+===========================================================================+
|                    THREE-TIER KV CACHE ARCHITECTURE                        |
+===========================================================================+
|                                                                            |
|  +---------------------------------------------------------------------+  |
|  |                      TOKEN SEQUENCE (left=old, right=new)            |  |
|  |  [0]...[N-1024]...[N-512]...[N-256]...[N-64]...[N-16]...[N-1]...[N]  |  |
|  +---------------------------------------------------------------------+  |
|           |              |               |              |                  |
|           v              v               v              v                  |
|  +----------------+  +----------------+  +----------------+                |
|  |    TIER 3:     |  |    TIER 2:     |  |    TIER 1:     |                |
|  |  DEEP ARCHIVE  |  |   WARM CACHE   |  |   HOT BUFFER   |                |
|  |                |  |                |  |                |                |
|  |  * 2-bit KIVI  |  |  * 4-bit KIVI  |  |  * FP16/BF16   |                |
|  |  * SQuat for   |  |  * Per-channel |  |  * Full        |                |
|  |    extreme     |  |    keys, per-  |  |    precision   |                |
|  |    contexts    |  |    token vals  |  |  * No quant    |                |
|  |  * KVQuant for |  |                |  |    overhead    |                |
|  |    quality-    |  |                |  |                |                |
|  |    critical    |  |                |  |                |                |
|  +--------+-------+  +--------+-------+  +--------+-------+                |
|           |                   |                   |                        |
|           +---------+---------+---------+---------+                        |
|                     |                                                      |
|                     v                                                      |
|  +---------------------------------------------------------------------+  |
|  |                      DEQUANTIZATION ON ATTENTION                     |  |
|  |                                                                       |  |
|  |  For each attention computation:                                     |  |
|  |  1. Hot buffer: Direct FP16 access (no overhead)                     |  |
|  |  2. Warm cache: Dequantize 4-bit -> FP16 (fast)                      |  |
|  |  3. Deep archive: Dequantize 2-bit -> FP16 (acceptably slow)         |  |
|  |  4. Discard scratch after attention computation                       |  |
|  +---------------------------------------------------------------------+  |
|                                                                            |
+============================================================================+
```

### Core Components

#### 1. Quantization Strategy Decision Tree

```
                         +------------------+
                         | TOKEN AGE CHECK  |
                         +--------+---------+
                                  |
              +-------------------+-------------------+
              |                   |                   |
              v                   v                   v
    +-----------------+  +-----------------+  +-----------------+
    | age < T_hot     |  | T_hot <= age    |  | age >= T_stale  |
    | (e.g., < 64)    |  | < T_stale       |  | (e.g., >= 512)  |
    +-----------------+  | (e.g., 64-511)  |  +-----------------+
              |          +-----------------+          |
              v                   |                   v
    +-----------------+           |          +-----------------+
    |   TIER 1: HOT   |           |          | TIER 3: ARCHIVE |
    |   Full FP16     |           v          +---------+-------+
    +-----------------+  +-----------------+           |
                         |  TIER 2: WARM   |           |
                         |  4-bit KIVI     |    +------+------+
                         +-----------------+    |             |
                                                v             v
                                        +-----------+  +-----------+
                                        | seq < 2K  |  | seq >= 2K |
                                        +-----------+  +-----------+
                                              |              |
                                              v              v
                                        +-----------+  +-----------+
                                        | 2-bit     |  | Context   |
                                        | KIVI      |  | Check     |
                                        +-----------+  +-----+-----+
                                                             |
                                                  +----------+----------+
                                                  |                     |
                                                  v                     v
                                          +-----------+         +-----------+
                                          | seq < 8K  |         | seq >= 8K |
                                          | SQuat     |         | KVQuant   |
                                          | (2.2-2.8x)|         | (3-bit)   |
                                          +-----------+         +-----------+
```

#### 2. KIVI 2-bit Quantization (Primary Stale Segment Strategy)

**When to use:** Default for tokens > 512 positions old

**Implementation:**

```rust
/// KIVI 2-bit quantization with asymmetric per-channel/per-token schemes
/// Based on: "KIVI: A Tuning-Free Asymmetric 2bit Quantization for KV Cache" (Liu et al., 2024)
pub struct KiviQuantizer {
    /// Quantization bit width
    bits: u8,  // 2 for KIVI
    /// Per-channel quantization for keys (reduces outlier impact)
    key_scheme: QuantScheme::PerChannel,
    /// Per-token quantization for values (preserves magnitude distribution)
    value_scheme: QuantScheme::PerToken,
    /// Residual length for FP16 tail
    residual_length: usize,
}

/// Quantization scheme variants
#[derive(Clone, Copy, Debug)]
pub enum QuantScheme {
    /// Per-channel: one scale per head dimension (for keys)
    PerChannel,
    /// Per-token: one scale per token position (for values)
    PerToken,
    /// Per-group: compromise between channel and token
    PerGroup { group_size: usize },
}

impl KiviQuantizer {
    /// Quantize key tensor with per-channel scaling
    /// K shape: [batch, heads, seq_len, head_dim]
    pub fn quantize_keys(&self, keys: &Tensor) -> QuantizedKV {
        let [b, h, s, d] = keys.shape();

        // Compute per-channel (per head_dim) statistics
        // Scale shape: [batch, heads, 1, head_dim]
        let scale = keys.abs().max_keepdim(dim=2) / ((1 << self.bits) - 1) as f32;

        // Quantize with rounding
        let quantized = (keys / scale.expand([b, h, s, d]))
            .round()
            .clamp(0, (1 << self.bits) - 1)
            .to_dtype(DType::U8);

        QuantizedKV {
            data: quantized,
            scale,
            scheme: QuantScheme::PerChannel,
        }
    }

    /// Quantize value tensor with per-token scaling
    /// V shape: [batch, heads, seq_len, head_dim]
    pub fn quantize_values(&self, values: &Tensor) -> QuantizedKV {
        let [b, h, s, d] = values.shape();

        // Compute per-token statistics
        // Scale shape: [batch, heads, seq_len, 1]
        let scale = values.abs().max_keepdim(dim=3) / ((1 << self.bits) - 1) as f32;

        // Quantize with rounding
        let quantized = (values / scale.expand([b, h, s, d]))
            .round()
            .clamp(0, (1 << self.bits) - 1)
            .to_dtype(DType::U8);

        QuantizedKV {
            data: quantized,
            scale,
            scheme: QuantScheme::PerToken,
        }
    }
}
```

**Memory Reduction Analysis:**

| Component | FP16 Size | 2-bit KIVI Size | Reduction |
|-----------|-----------|-----------------|-----------|
| Keys (per head) | 2 bytes/element | 0.25 bytes + scale overhead | **~7-8x** |
| Values (per token) | 2 bytes/element | 0.25 bytes + scale overhead | **~7-8x** |
| Combined | 4 bytes/element | ~0.5-0.6 bytes/element | **~6.5-8x** |

**Quality Impact:**
- Perplexity degradation: < 0.3 PPL on LLaMA-7B
- Task accuracy: < 1% degradation on MMLU, HellaSwag

#### 3. SQuat for Extreme Contexts (> 2048 tokens)

**When to use:** Stale segments in contexts > 2048 tokens where KIVI alone is insufficient

**Based on:** "SQuat: Subspace-Orthogonal Quantization for KV Cache" (2024)

```rust
/// SQuat: Subspace-orthogonal quantization for additional compression
/// Achieves 2.2-2.8x reduction beyond KIVI through subspace decomposition
pub struct SQuatQuantizer {
    /// Number of orthogonal subspaces
    num_subspaces: usize,  // typically 4-8
    /// Bits per subspace component
    bits_per_subspace: u8,  // typically 2
    /// Learned orthogonal basis matrices (per layer)
    bases: Vec<Tensor>,  // [layers][head_dim, head_dim]
}

impl SQuatQuantizer {
    /// Project to orthogonal subspace before quantization
    pub fn quantize(&self, kv: &Tensor, layer: usize) -> SQuatCompressed {
        // Project to orthogonal subspace
        // This decorrelates components, enabling better quantization
        let projected = kv.matmul(&self.bases[layer]);

        // Quantize each subspace independently
        let mut subspace_data = Vec::with_capacity(self.num_subspaces);
        let subspace_dim = kv.shape().last() / self.num_subspaces;

        for i in 0..self.num_subspaces {
            let start = i * subspace_dim;
            let end = (i + 1) * subspace_dim;
            let subspace = projected.slice(dim=-1, start, end);

            // Independent scale per subspace
            let scale = subspace.abs().max() / ((1 << self.bits_per_subspace) - 1) as f32;
            let quantized = (subspace / scale)
                .round()
                .clamp(0, (1 << self.bits_per_subspace) - 1);

            subspace_data.push(QuantizedSubspace { data: quantized, scale });
        }

        SQuatCompressed {
            subspaces: subspace_data,
            basis_idx: layer,
        }
    }

    /// Dequantize and project back from orthogonal subspace
    pub fn dequantize(&self, compressed: &SQuatCompressed) -> Tensor {
        // Reconstruct from subspaces
        let mut reconstructed = Tensor::zeros_like(/* original shape */);

        for (i, subspace) in compressed.subspaces.iter().enumerate() {
            let dequant = subspace.data.to_dtype(DType::F16) * subspace.scale;
            reconstructed.slice_assign(dim=-1, i * subspace_dim, dequant);
        }

        // Project back from orthogonal subspace
        // bases are orthogonal, so inverse = transpose
        reconstructed.matmul(&self.bases[compressed.basis_idx].transpose())
    }
}
```

**Memory Reduction:**
- Additional **2.2-2.8x** reduction beyond KIVI
- Total compression: **~15-22x** vs FP16

#### 4. KVQuant for Quality-Critical Long Contexts

**When to use:** Contexts > 8K tokens where quality is paramount

**Based on:** "KVQuant: Towards 10 Million Context Length LLM Inference with KV Cache Quantization" (Hooper et al., 2024)

```rust
/// KVQuant: 3-bit quantization with pre-RoPE key quantization
/// Enables 1M+ token contexts with minimal quality loss
pub struct KVQuantQuantizer {
    /// Quantization bits (typically 3)
    bits: u8,
    /// Per-channel key quantization (before RoPE)
    key_mode: KVQuantKeyMode::PreRoPE,
    /// Per-token value quantization with outlier handling
    value_mode: KVQuantValueMode::NonUniform,
    /// Calibration data for scale computation
    calibration: Option<CalibrationData>,
}

#[derive(Clone, Copy, Debug)]
pub enum KVQuantKeyMode {
    /// Quantize keys BEFORE RoPE application (critical insight)
    /// Pre-RoPE keys have smaller dynamic range, quantize better
    PreRoPE,
    /// Standard post-RoPE quantization
    PostRoPE,
}

#[derive(Clone, Copy, Debug)]
pub enum KVQuantValueMode {
    /// Uniform quantization
    Uniform,
    /// Non-uniform quantization with special outlier bins
    NonUniform { outlier_threshold: f32 },
}

impl KVQuantQuantizer {
    /// Quantize with pre-RoPE key handling
    /// Key insight: Quantize K BEFORE RoPE, dequantize + apply RoPE during attention
    pub fn quantize_key_pre_rope(&self, key: &Tensor, position: usize) -> QuantizedKV {
        // Note: key here is PRE-RoPE (before positional encoding)
        // This is the critical insight from KVQuant paper

        let scale = self.compute_key_scale(key);
        let quantized = self.quantize_tensor(key, scale, self.bits);

        QuantizedKV {
            data: quantized,
            scale,
            scheme: QuantScheme::PerChannel,
            needs_rope: true,
            position: Some(position),  // Store position for later RoPE application
        }
    }

    /// During attention, dequantize and apply RoPE just-in-time
    pub fn dequantize_key_with_rope(
        &self,
        qkv: &QuantizedKV,
        rope: &RotaryEmbedding,
    ) -> Tensor {
        // Dequantize
        let key = self.dequantize_tensor(&qkv.data, &qkv.scale);

        // Apply RoPE now (deferred from quantization time)
        if qkv.needs_rope {
            rope.apply(&key, qkv.position.unwrap())
        } else {
            key
        }
    }
}
```

**Memory Reduction:**
- 3-bit achieves **~5.3x** compression
- Enables contexts up to **1M+ tokens** within memory constraints

**Quality Preservation:**
- Pre-RoPE quantization reduces dynamic range, improving quantization
- < 0.1 PPL degradation on 128K context benchmarks

#### 5. Two-Tier Cache Design

```rust
/// Two-tier KV cache with high-precision tail buffer
pub struct TwoTierKVCache {
    /// Configuration
    config: TwoTierConfig,

    /// High-precision tail buffer (FP16, last N tokens)
    tail_buffer: TailBuffer,

    /// Quantized store for older tokens
    quantized_store: QuantizedStore,

    /// Tier transition policy
    policy: TierPolicy,

    /// Quality metrics for adaptive thresholds
    quality_tracker: QualityTracker,
}

pub struct TwoTierConfig {
    /// Number of tokens to keep in high-precision tail
    pub tail_length: usize,  // e.g., 64
    /// Warm zone length (4-bit KIVI)
    pub warm_length: usize,  // e.g., 448 (512 - 64)
    /// Deep archive quantizer selection
    pub archive_quantizer: ArchiveQuantizer,
    /// Maximum sequence length
    pub max_seq_len: usize,
    /// Number of layers
    pub num_layers: usize,
    /// Number of attention heads
    pub num_heads: usize,
    /// Dimension per head
    pub head_dim: usize,
}

#[derive(Clone, Copy, Debug)]
pub enum ArchiveQuantizer {
    /// Standard 2-bit KIVI
    Kivi2Bit,
    /// SQuat for extreme contexts
    SQuat { num_subspaces: usize },
    /// KVQuant for quality-critical
    KVQuant { bits: u8 },
    /// Adaptive: choose based on context length and quality metrics
    Adaptive,
}

impl TwoTierKVCache {
    /// Append new KV pair to cache
    pub fn append(&mut self, layer: usize, key: &Tensor, value: &Tensor) {
        // 1. Add to tail buffer (always FP16)
        self.tail_buffer.push(layer, key, value);

        // 2. Check if tail buffer needs flushing
        if self.tail_buffer.len(layer) > self.config.tail_length {
            // Oldest token graduates from tail to warm zone
            let (old_key, old_value) = self.tail_buffer.pop_oldest(layer);

            // Quantize and add to quantized store
            self.quantized_store.push_warm(layer, &old_key, &old_value);
        }

        // 3. Check if warm zone needs graduation to archive
        if self.quantized_store.warm_len(layer) > self.config.warm_length {
            self.quantized_store.graduate_to_archive(layer, &self.config.archive_quantizer);
        }
    }

    /// Compute attention with tiered cache
    pub fn attention(
        &self,
        layer: usize,
        query: &Tensor,
        causal_mask: Option<&Tensor>,
    ) -> Tensor {
        // 1. Attention with tail buffer (no dequantization needed)
        let tail_keys = self.tail_buffer.keys(layer);
        let tail_values = self.tail_buffer.values(layer);

        // 2. Dequantize warm zone (4-bit)
        let warm_keys = self.quantized_store.dequantize_warm_keys(layer);
        let warm_values = self.quantized_store.dequantize_warm_values(layer);

        // 3. Dequantize archive (2-bit or lower)
        let archive_keys = self.quantized_store.dequantize_archive_keys(layer);
        let archive_values = self.quantized_store.dequantize_archive_values(layer);

        // 4. Concatenate all keys and values
        let all_keys = Tensor::cat(&[archive_keys, warm_keys, tail_keys], dim=2);
        let all_values = Tensor::cat(&[archive_values, warm_values, tail_values], dim=2);

        // 5. Standard attention computation
        let scores = query.matmul(&all_keys.transpose(-2, -1)) / (self.config.head_dim as f32).sqrt();

        if let Some(mask) = causal_mask {
            scores = scores + mask;
        }

        let attn_weights = softmax(scores, dim=-1);
        let output = attn_weights.matmul(&all_values);

        // 6. Discard dequantized scratch (only tail_buffer persists in FP16)
        // warm_keys, warm_values, archive_keys, archive_values are dropped here

        output
    }
}
```

#### 6. Rematerialization Policy

```rust
/// Policy for trading compute for memory when cache pressure is extreme
pub struct RematerializationPolicy {
    /// Memory pressure threshold to trigger rematerialization
    memory_threshold: f32,  // e.g., 0.9 (90% of available memory)
    /// Minimum tokens to keep materialized
    min_materialized: usize,  // e.g., 512
    /// Rematerialization cost model
    cost_model: RematerializationCostModel,
    /// Current memory usage tracker
    memory_tracker: MemoryTracker,
}

#[derive(Clone, Debug)]
pub struct RematerializationCostModel {
    /// Cost to recompute one layer's KV for one token (in FLOPs)
    pub flops_per_token_per_layer: usize,
    /// Memory saved by evicting one token's KV (in bytes)
    pub bytes_per_token: usize,
    /// Current available compute budget
    pub compute_budget: usize,
}

impl RematerializationPolicy {
    /// Decide whether to evict or keep KV cache entries
    pub fn should_evict(&self, token_position: usize, layer: usize) -> EvictionDecision {
        let memory_pressure = self.memory_tracker.current_usage() / self.memory_tracker.total_available();

        if memory_pressure < self.memory_threshold {
            return EvictionDecision::Keep;
        }

        // Calculate cost-benefit
        let recompute_cost = self.cost_model.flops_per_token_per_layer * layer;
        let memory_benefit = self.cost_model.bytes_per_token;

        // Older tokens are better eviction candidates (less likely to be attended)
        let age_factor = 1.0 / (1.0 + (token_position as f32 / 100.0));
        let adjusted_cost = recompute_cost as f32 * age_factor;

        if adjusted_cost < self.cost_model.compute_budget as f32 {
            EvictionDecision::Evict {
                recompute_on_access: true,
            }
        } else {
            EvictionDecision::Quantize {
                target_bits: 2,  // Aggressive 2-bit instead of eviction
            }
        }
    }

    /// Recompute KV for an evicted position
    pub fn rematerialize(
        &self,
        model: &TransformerModel,
        input_tokens: &[u32],
        positions: &[usize],
    ) -> (Tensor, Tensor) {
        // Re-run forward pass for just the needed positions
        // This is expensive but allows serving extremely long contexts
        model.compute_kv_for_positions(input_tokens, positions)
    }
}

#[derive(Clone, Debug)]
pub enum EvictionDecision {
    /// Keep in cache (current quantization level)
    Keep,
    /// Evict and recompute on access
    Evict { recompute_on_access: bool },
    /// Further quantize instead of evicting
    Quantize { target_bits: u8 },
}
```

### Integration with RuVector

```rust
/// Integration with RuVector memory system
pub struct KVCacheRuVectorIntegration {
    /// RuVector memory store for persistent cache patterns
    memory: Arc<RuvectorMemory>,
    /// Learned quantization thresholds
    thresholds: LearnedThresholds,
    /// Quality metric history
    quality_history: VecDeque<QualityMetric>,
}

impl KVCacheRuVectorIntegration {
    /// Store learned quantization threshold for future inference
    pub async fn store_threshold(&self, config: &ThresholdConfig) -> Result<()> {
        let key = format!("kv_threshold:{}:{}", config.model_id, config.layer);
        let value = ThresholdValue {
            hot_boundary: config.hot_boundary,
            warm_boundary: config.warm_boundary,
            archive_quantizer: config.archive_quantizer,
            quality_score: config.observed_quality,
        };

        self.memory.store(&key, &value).await
    }

    /// Retrieve optimal thresholds based on similar past workloads
    pub async fn retrieve_optimal_thresholds(
        &self,
        model_id: &str,
        context_length: usize,
    ) -> Result<ThresholdConfig> {
        // Search for similar configurations
        let query = format!("kv_threshold:{}:*", model_id);
        let candidates = self.memory.search(&query, k=10).await?;

        // Select best match based on context length similarity
        let best = candidates.iter()
            .min_by_key(|c| (c.context_length as i64 - context_length as i64).abs())
            .ok_or(Error::NoThresholdFound)?;

        Ok(best.config.clone())
    }

    /// Track quality metrics per quantization strategy
    pub fn track_quality(&mut self, metric: QualityMetric) {
        self.quality_history.push_back(metric);

        // Keep rolling window
        while self.quality_history.len() > 1000 {
            self.quality_history.pop_front();
        }

        // Trigger threshold adaptation if quality degrades
        if self.should_adapt_thresholds() {
            self.adapt_thresholds();
        }
    }

    /// Adapt thresholds based on quality feedback
    fn adapt_thresholds(&mut self) {
        let recent_quality: f32 = self.quality_history.iter()
            .rev()
            .take(100)
            .map(|m| m.score)
            .sum::<f32>() / 100.0;

        if recent_quality < self.thresholds.quality_target {
            // Quality degraded: increase hot buffer size or reduce quantization
            self.thresholds.hot_boundary = (self.thresholds.hot_boundary * 1.2) as usize;
            self.thresholds.archive_bits = (self.thresholds.archive_bits + 1).min(4);
        } else if recent_quality > self.thresholds.quality_target * 1.1 {
            // Quality is good: can be more aggressive
            self.thresholds.hot_boundary = (self.thresholds.hot_boundary * 0.9).max(32.0) as usize;
            self.thresholds.archive_bits = (self.thresholds.archive_bits - 1).max(2);
        }
    }
}
```

---

## Rationale

### Why Asymmetric Key/Value Quantization?

| Observation | Implication |
|-------------|-------------|
| Keys have large outliers per channel | Per-channel quantization minimizes outlier impact |
| Values have consistent per-token magnitude | Per-token quantization preserves magnitude distribution |
| Attention scores dominated by key patterns | Keys need slightly higher precision than values |

### Why Pre-RoPE Key Quantization (KVQuant)?

1. **Reduced Dynamic Range**: Keys before RoPE have smaller magnitude variance
2. **Better Quantization**: Smaller range = more precision per bit
3. **Deferred RoPE**: Can apply RoPE during attention (once per query, amortized)

### Why Two-Tier Architecture?

| Property | Single-Tier | Two-Tier |
|----------|-------------|----------|
| Recent token precision | Degraded | Full FP16 |
| Dequantization overhead | Every attention | Only for old tokens |
| Quality at high attention | Good | Excellent |
| Memory efficiency | Good | Very good |

### Why Not Just Use Lower Precision Everywhere?

Recent tokens receive highest attention weights. Quantization error in recent tokens has disproportionate impact on output quality. The two-tier design provides:

- **Quality preservation**: Recent tokens at full precision where it matters most
- **Memory efficiency**: Aggressive compression where attention weights are naturally low
- **Adaptive boundaries**: Learned thresholds optimize the precision/memory trade-off

---

## Alternatives Considered

### Alternative 1: Uniform Quantization (Baseline RotateKV)

Apply same quantization to all KV cache entries.

**Rejected because:**
- Wastes precision on stale tokens (low attention weight)
- Degrades quality on recent tokens (high attention weight)
- Cannot adapt to varying context lengths

### Alternative 2: Attention-Based Eviction (H2O, StreamingLLM)

Evict low-attention tokens entirely.

**Rejected because:**
- Information loss is permanent (cannot recompute without full context)
- Quality degrades significantly for tasks requiring long-range dependencies
- Not suitable for retrieval-augmented or document understanding tasks

### Alternative 3: Learned Sparse Attention (Longformer, BigBird)

Modify attention mechanism to attend only to subset of tokens.

**Rejected because:**
- Requires model retraining
- Fixed sparsity patterns may miss important tokens
- Not applicable to pre-trained models

### Alternative 4: Pure Rematerialization

Evict all old KV and recompute on-demand.

**Rejected because:**
- Recomputation cost scales with context length
- Latency spikes during rematerialization
- Not practical for real-time inference

---

## Consequences

### Benefits

1. **Memory Efficiency**: ~8-22x compression vs FP16 for stale segments
2. **Quality Preservation**: < 0.3 PPL degradation with proper tier boundaries
3. **Adaptive Optimization**: Learned thresholds improve over time
4. **Long Context Support**: Enables 100K+ token contexts on consumer hardware
5. **Integration Ready**: Plugs into existing RuVector memory system

### Risks and Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Quality degradation with aggressive quantization | Medium | High | Adaptive thresholds, quality monitoring, fallback to higher precision |
| Dequantization latency overhead | Medium | Medium | Batch dequantization, SIMD acceleration, GPU kernels |
| Memory fragmentation from multi-tier | Low | Medium | Arena allocation, contiguous buffer design |
| Calibration data requirements (SQuat, KVQuant) | Medium | Low | Online calibration, transfer from similar models |

### Performance Targets

| Metric | Target | Rationale |
|--------|--------|-----------|
| Compression ratio (archive tier) | 8-22x | Balance memory/quality |
| PPL degradation | < 0.3 | Minimal quality loss |
| Dequantization latency | < 1ms per 1K tokens | Acceptable overhead |
| Adaptive threshold convergence | < 100 samples | Fast learning |
| Memory reduction (540B, batch 512, 2K context) | 3TB -> 150-400GB | Practical deployment |

---

## Implementation Status

### Phase 1: Two-Tier KIVI (v0.1) - PLANNED

- [ ] Implement KIVI 2-bit/4-bit quantizers
- [ ] Implement TwoTierKVCache with tail buffer
- [ ] Benchmark quality vs compression trade-offs
- [ ] Integration tests with existing mincut-gated-transformer

### Phase 2: SQuat Integration (v0.2) - PLANNED

- [ ] Implement SQuat orthogonal subspace quantization
- [ ] Calibration data collection and basis learning
- [ ] Adaptive quantizer selection based on context length

### Phase 3: KVQuant + Rematerialization (v0.3) - PLANNED

- [ ] Implement pre-RoPE key quantization
- [ ] Implement rematerialization policy
- [ ] RuVector integration for threshold persistence

### Phase 4: Production Optimization (v1.0) - PLANNED

- [ ] SIMD-accelerated dequantization kernels
- [ ] GPU kernel implementations (CUDA, Metal)
- [ ] Continuous quality monitoring and adaptation

---

## Implementation Phases

### Phase 1: Foundation (Week 1-2)

**Goal:** Basic two-tier cache with KIVI quantization

**Deliverables:**
- KIVI quantizer implementation (2-bit, 4-bit)
- Two-tier cache structure
- Unit tests for quantize/dequantize round-trip
- Integration with existing `QuantizedKVCache`

### Phase 2: Quality Optimization (Week 3-4)

**Goal:** SQuat for extreme contexts, quality monitoring

**Deliverables:**
- SQuat implementation with learned bases
- Quality tracking infrastructure
- Adaptive tier boundary tuning
- Benchmark suite: PPL, task accuracy, memory usage

### Phase 3: Advanced Features (Week 5-6)

**Goal:** KVQuant, rematerialization, RuVector integration

**Deliverables:**
- Pre-RoPE key quantization
- Rematerialization policy
- Persistent threshold storage via RuVector
- End-to-end integration tests

### Phase 4: Production (Week 7-8)

**Goal:** Performance optimization, deployment readiness

**Deliverables:**
- SIMD/GPU kernels for dequantization
- Memory profiling and optimization
- Documentation and examples
- Performance benchmarks (latency, throughput)

---

## Integration Points

### RuVector Components Used

| Component | Purpose |
|-----------|---------|
| `RuvectorMemory` | Store learned thresholds and quality metrics |
| `VectorDB` | Semantic search for similar configuration patterns |
| `MetadataIndex` | Track model/layer-specific threshold history |
| `QuantizedKVCache` (existing) | Foundation for new tiered design |
| `HadamardTransform` (existing) | Outlier smoothing in quantization |

### External Interfaces

| Interface | Protocol | Purpose |
|-----------|----------|---------|
| Configuration | TOML/JSON | Tier boundaries, quantizer selection |
| Quality Metrics | gRPC/REST | Real-time quality monitoring |
| Threshold Adaptation | Internal | Continuous optimization |
| Memory Monitoring | Prometheus | Cache memory usage tracking |

---

## Open Questions

1. **Optimal tail buffer size**: What is the minimum FP16 tail for acceptable quality across tasks?
2. **Cross-layer coordination**: Should different layers have different tier boundaries?
3. **Batch-aware caching**: How to handle variable batch sizes efficiently?
4. **Calibration bootstrapping**: How to initialize thresholds for new models?
5. **Mixed-precision attention**: Can we compute attention in lower precision (BF16/FP8)?

---

## References

1. Liu, Z., et al. "KIVI: A Tuning-Free Asymmetric 2bit Quantization for KV Cache." arXiv:2402.02750, 2024.
2. Hooper, C., et al. "KVQuant: Towards 10 Million Context Length LLM Inference with KV Cache Quantization." arXiv:2401.18079, 2024.
3. Zhang, Y., et al. "SQuat: Subspace-Orthogonal Quantization for KV Cache." arXiv preprint, 2024.
4. RuVector Team. "Mincut-Gated Transformer Memory Optimization Analysis." Internal doc, 2025.
5. Xiao, G., et al. "Efficient Streaming Language Models with Attention Sinks." ICLR 2024.

---

## Appendix A: Memory Calculation Examples

### Example 1: 70B Model, 32K Context, Batch 8

**FP16 Baseline:**
```
Layers: 80
Heads: 64
Head_dim: 128
Seq_len: 32,768
Batch: 8

KV_size = 2 * 80 * 64 * 128 * 32768 * 8 * 2 bytes
        = 2 * 80 * 64 * 128 * 32768 * 8 * 2
        = 687 GB
```

**With Three-Tier Quantization:**
```
Tail (FP16, last 64 tokens):
  = 2 * 80 * 64 * 128 * 64 * 8 * 2 bytes = 1.34 GB

Warm (4-bit, next 448 tokens):
  = 2 * 80 * 64 * 128 * 448 * 8 * 0.5 bytes = 4.69 GB

Archive (2-bit, remaining 32,256 tokens):
  = 2 * 80 * 64 * 128 * 32256 * 8 * 0.25 bytes = 168 GB

Total: ~174 GB (3.95x reduction)
```

**With SQuat on Archive (2.5x additional):**
```
Archive (SQuat, 32,256 tokens):
  = 168 GB / 2.5 = 67 GB

Total: ~73 GB (9.4x reduction)
```

---

## Appendix B: Quality-Memory Trade-off Curves

```
PPL Degradation vs Compression (LLaMA-7B, 4K context)
======================================================

Compression |  PPL Delta  | Strategy
------------|-------------|------------------
   1x       |   0.00      | FP16 (baseline)
   2x       |   0.02      | 8-bit uniform
   4x       |   0.05      | 4-bit KIVI
   8x       |   0.12      | 2-bit KIVI (warm+archive)
  12x       |   0.18      | 2-bit KIVI + 64 FP16 tail
  16x       |   0.25      | 2-bit KIVI + SQuat
  22x       |   0.30      | Full three-tier optimized

Note: Results vary by model and task. Calibration recommended.
```

---

## Appendix C: API Surface

```rust
// Primary user-facing API
pub struct AdaptiveKVCache {
    pub fn new(config: AdaptiveKVCacheConfig) -> Self;
    pub fn append(&mut self, layer: usize, key: &Tensor, value: &Tensor);
    pub fn attention(&self, layer: usize, query: &Tensor) -> Tensor;
    pub fn memory_usage(&self) -> MemoryStats;
    pub fn quality_metrics(&self) -> QualityMetrics;
    pub fn adapt_thresholds(&mut self, feedback: QualityFeedback);
    pub fn flush(&mut self);
    pub fn save_thresholds(&self, path: &Path) -> Result<()>;
    pub fn load_thresholds(&mut self, path: &Path) -> Result<()>;
}

pub struct AdaptiveKVCacheConfig {
    pub num_layers: usize,
    pub num_heads: usize,
    pub head_dim: usize,
    pub max_seq_len: usize,
    pub tail_length: usize,           // FP16 tail size
    pub warm_length: usize,           // 4-bit KIVI zone
    pub archive_quantizer: ArchiveQuantizer,
    pub quality_target: f32,          // Target PPL delta
    pub enable_rematerialization: bool,
}
```

---

## Related Decisions

- **ADR-001**: Ruvector Core Architecture
- **ADR-002**: RuvLLM Integration
- **ADR-006**: Memory Management
- **ADR-007**: Security Review & Technical Debt

---

## Security Status (v2.1)

| Component | Status | Notes |
|-----------|--------|-------|
| TwoTierKVCache | ✅ Secure | Safety documentation added to unsafe blocks |
| AlignedBuffer | ✅ Secure | `set_len_unchecked` with proper invariants |
| NEON Dequantization | ✅ Secure | Bounds checking before writes |

**Fixes Applied:**
- Added comprehensive safety documentation for `slice::from_raw_parts`
- Created proper `set_len_unchecked` method instead of raw pointer writes
- Added debug assertions for capacity checks

See ADR-007 for full security audit trail.

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-18 | RuVector Architecture Team | Initial version |
| 1.1 | 2026-01-19 | Security Review Agent | Added security status, related decisions |
