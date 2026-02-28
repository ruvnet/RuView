# ADR-008: mistral-rs Integration for Production-Scale LLM Serving

**Status:** Proposed
**Date:** 2026-01-20
**Decision Makers:** Ruvector Architecture Team
**Technical Area:** LLM Inference Engine / Production Serving

---

## Context and Problem Statement

RuvLLM v2.3 includes a stub `MistralBackend` implementation at `crates/ruvllm/src/backends/mistral_backend.rs` that defines the interface for high-performance LLM inference but lacks actual integration with the mistral-rs crate. The current Candle backend is optimized for single-user and edge deployment scenarios, but production-scale serving requires advanced memory management and multi-tenant capabilities.

### Current State

The existing `MistralBackend` stub provides:
- Configuration structures for PagedAttention, X-LoRA, and ISQ
- `XLoraManager` with adapter loading/routing logic (placeholder)
- `MistralBackendConfig` with builder pattern for Metal/CUDA targets
- Integration hooks for the `LlmBackend` trait

However, the implementation is non-functional:
- No actual mistral-rs crate dependency
- Token generation returns placeholder values
- Model loading does not wire to inference pipeline
- PagedAttention uses RuvLLM's internal implementation, not mistral-rs's optimized version

### Key Challenges

1. **Concurrent User Scaling**: Candle backend is optimized for single-user inference; production servers need 10-100+ concurrent requests
2. **KV Cache Memory Pressure**: Without vLLM-style paging, long-context sessions exhaust GPU memory
3. **Multi-Task Models**: LoRA adapter switching requires per-request overhead; X-LoRA enables per-token routing
4. **Deployment Flexibility**: Models should be quantized at runtime based on available hardware

---

## Decision Drivers

### Performance Requirements
- **Concurrent sessions**: 50-100 simultaneous inference requests
- **Memory efficiency**: 5-10x improvement in KV cache utilization
- **Adapter latency**: <1ms overhead for X-LoRA routing decisions
- **Quantization**: Runtime ISQ without model re-export

### Compatibility Requirements
- **Existing interface**: Must implement `LlmBackend` trait seamlessly
- **Feature isolation**: Optional dependency with feature flags
- **Backend selection**: Runtime choice between Candle and mistral-rs

### Hardware Requirements
- **Apple Silicon**: Metal acceleration via `mistral-rs-metal`
- **NVIDIA GPUs**: CUDA acceleration via `mistral-rs-cuda`
- **CPU fallback**: Pure Rust path for edge/WASM targets

---

## Considered Options

### Option A: Fork and Embed mistral-rs

Vendor mistral-rs source code directly into RuvLLM.

**Pros:**
- Full control over API surface
- No external dependency versioning
- Can customize for RuvLLM's needs

**Cons:**
- Maintenance burden of tracking upstream
- Miss upstream optimizations and fixes
- Duplicated effort

### Option B: Optional Dependency with Feature Flags

Add mistral-rs as an optional dependency behind feature flags, wiring the existing `MistralBackend` interface to actual mistral-rs crate.

**Pros:**
- Leverage upstream development
- Clean separation via features
- Users choose their backend at compile time
- Smaller binary for edge deployments (Candle-only)

**Cons:**
- API surface depends on upstream stability
- Two codepaths to maintain
- Feature matrix complexity

### Option C: Runtime Backend Selection

Use dynamic dispatch to select backend at runtime via configuration.

**Pros:**
- Single binary for all deployments
- Runtime flexibility

**Cons:**
- Binary size includes all backends
- Dynamic dispatch overhead
- Complex testing matrix

---

## Decision Outcome

**Chosen Option: Option B - Optional Dependency with Feature Flags**

Add mistral-rs as an optional dependency with three feature flags, wiring the existing `MistralBackend` stub to the actual mistral-rs implementation.

### Rationale

1. **Separation of concerns**: Edge deployments use Candle (no mistral-rs dependency); server deployments enable mistral-rs features
2. **Upstream leverage**: mistral-rs team maintains PagedAttention, X-LoRA, ISQ implementations
3. **Existing interface**: The `MistralBackend` stub already defines the API; we wire it to real implementation
4. **Incremental adoption**: Users can migrate from Candle to mistral-rs backend per-deployment

---

## Technical Specifications

### Feature Flags

```toml
# Cargo.toml additions
[features]
default = ["candle-backend"]

# Base mistral-rs integration
mistral-rs = ["dep:mistralrs", "dep:mistralrs-core"]

# Apple Silicon Metal acceleration
mistral-rs-metal = ["mistral-rs", "mistralrs/metal"]

# NVIDIA CUDA acceleration
mistral-rs-cuda = ["mistral-rs", "mistralrs/cuda"]

[dependencies]
# Optional mistral-rs integration
mistralrs = { version = "0.3", optional = true }
mistralrs-core = { version = "0.3", optional = true }
```

### Feature Matrix

| Feature | Candle | mistral-rs | mistral-rs-metal | mistral-rs-cuda |
|---------|--------|------------|------------------|-----------------|
| Single-user inference | Yes | Yes | Yes | Yes |
| PagedAttention | No | Yes | Yes | Yes |
| X-LoRA | No | Yes | Yes | Yes |
| ISQ | No | Yes | Yes | Yes |
| Metal acceleration | Yes | No | Yes | No |
| CUDA acceleration | Partial | No | No | Yes |
| WASM support | Yes | No | No | No |
| Binary size | ~15MB | ~45MB | ~50MB | ~60MB |

### Architecture

```
+-----------------------------------------------------------------------+
|                    MISTRAL-RS INTEGRATION ARCHITECTURE                 |
+-----------------------------------------------------------------------+
|                                                                        |
|   +-------------------+     +-------------------+     +--------------+ |
|   | MistralBackend    |     | mistralrs::Model  |     | Hardware     | |
|   | (RuvLLM adapter)  |     | (inference core)  |     | Accelerator  | |
|   |                   |     |                   |     |              | |
|   | - Config mapping  |---->| - PagedAttention  |---->| - Metal      | |
|   | - Trait impl      |     | - X-LoRA routing  |     | - CUDA       | |
|   | - Error handling  |     | - ISQ runtime     |     | - CPU        | |
|   +--------+----------+     +---------+---------+     +------+-------+ |
|            |                          |                      |         |
|            v                          v                      v         |
|   +--------+----------+     +---------+---------+     +------+-------+ |
|   | LlmBackend trait  |     | KV Cache Pool     |     | Tensor Ops   | |
|   | (RuvLLM unified)  |     | (PagedAttention)  |     | (kernels)    | |
|   +-------------------+     +-------------------+     +--------------+ |
|                                                                        |
+-----------------------------------------------------------------------+
```

### Key Features to Enable

#### 1. PagedAttention (vLLM-style KV Cache Management)

PagedAttention partitions the KV cache into fixed-size blocks (pages) that can be allocated non-contiguously, enabling:
- **5-10x concurrent users**: Memory shared across requests via copy-on-write pages
- **Dynamic allocation**: Pages allocated as sequences grow, freed when complete
- **Prefix caching**: Common prefixes (system prompts) share pages across requests

```rust
/// PagedAttention configuration for mistral-rs
#[cfg(feature = "mistral-rs")]
pub struct PagedAttentionConfig {
    /// Block size in tokens (typical: 16)
    pub block_size: usize,
    /// Maximum blocks in page table
    pub max_blocks: usize,
    /// GPU memory fraction for KV cache (0.0-1.0)
    pub gpu_memory_fraction: f32,
    /// Enable prefix caching for repeated prompts
    pub enable_prefix_caching: bool,
}

impl Default for PagedAttentionConfig {
    fn default() -> Self {
        Self {
            block_size: 16,
            max_blocks: 4096,
            gpu_memory_fraction: 0.9,
            enable_prefix_caching: true,
        }
    }
}
```

**Performance Impact:**
| Metric | Without PagedAttention | With PagedAttention |
|--------|------------------------|---------------------|
| Concurrent users | 1-2 | 10-50 |
| Memory utilization | 40-60% | 85-95% |
| Memory fragmentation | High | Near-zero |

#### 2. X-LoRA (eXpert-mixed LoRA)

X-LoRA enables per-token adapter routing for multi-task models:
- **Dynamic mixing**: Router network selects adapters per token
- **Learned routing**: MLP router trained on adapter selection
- **Top-k activation**: Only k adapters compute per token (efficiency)

```rust
/// X-LoRA configuration for multi-adapter inference
#[cfg(feature = "mistral-rs")]
pub struct XLoraConfig {
    /// Adapter names/paths to load
    pub adapters: Vec<String>,
    /// Top-k adapters to activate per token
    pub top_k: usize,
    /// Router temperature for softmax
    pub temperature: f32,
    /// Mixing mode
    pub mixing_mode: XLoraMixingMode,
}

#[derive(Debug, Clone, Copy)]
pub enum XLoraMixingMode {
    /// Sum weighted adapter outputs
    Additive,
    /// Concatenate and project
    Concatenate,
    /// Gated mixture with learned gates
    Gated,
}
```

**Use Cases:**
- Code + chat model: Route code tokens to code adapter, natural language to chat adapter
- Multi-language: Route based on detected language
- Domain-specific: Finance, medical, legal adapters activated by context

#### 3. ISQ (In-Situ Quantization)

ISQ enables runtime quantization without pre-exported quantized models:
- **Runtime flexibility**: Same model weights, different quantization per deployment
- **Memory adaptation**: Quantize to fit available hardware
- **Quality preservation**: Activation-aware methods (AWQ, GPTQ) maintain accuracy

```rust
/// ISQ configuration for runtime quantization
#[cfg(feature = "mistral-rs")]
pub struct IsqConfig {
    /// Quantization bits (2, 4, 8)
    pub bits: u8,
    /// Quantization method
    pub method: IsqMethod,
    /// Calibration dataset size
    pub calibration_samples: usize,
}

#[derive(Debug, Clone, Copy)]
pub enum IsqMethod {
    /// Activation-aware Weight Quantization
    AWQ,
    /// GPTQ with optimal brain quantization
    GPTQ,
    /// Round-to-nearest (fastest, lower quality)
    RTN,
    /// SmoothQuant (activation smoothing)
    SmoothQuant,
}
```

**Performance Impact:**
| Method | Bits | Memory Reduction | Quality Loss |
|--------|------|------------------|--------------|
| AWQ | 4 | 4x | <1% |
| GPTQ | 4 | 4x | <1% |
| RTN | 4 | 4x | 2-3% |
| AWQ | 2 | 8x | 3-5% |

### Implementation Roadmap

#### Phase 1: Core Integration (Week 1-2)

1. Add mistral-rs dependencies with feature flags
2. Implement config mapping: `MistralBackendConfig` -> `mistralrs::Config`
3. Wire `load_model` to mistral-rs model loading
4. Wire `generate` and `generate_stream` to mistral-rs inference

```rust
#[cfg(feature = "mistral-rs")]
impl LlmBackend for MistralBackend {
    fn load_model(&mut self, model_id: &str, config: ModelConfig) -> Result<()> {
        use mistralrs::{ModelKind, MistralRs, MistralRsBuilder};

        let builder = MistralRsBuilder::new(model_id)
            .with_paged_attention(self.config.paged_attention.as_ref().map(|pa| {
                mistralrs::PagedAttentionConfig {
                    block_size: pa.block_size,
                    ..Default::default()
                }
            }));

        self.inner = Some(builder.build()?);
        Ok(())
    }

    fn generate(&self, prompt: &str, params: GenerateParams) -> Result<String> {
        let inner = self.inner.as_ref()
            .ok_or_else(|| Error::msg("Model not loaded"))?;

        let request = mistralrs::Request::new(prompt)
            .with_max_tokens(params.max_tokens)
            .with_temperature(params.temperature);

        let response = inner.send_request(request)?;
        Ok(response.text)
    }
}
```

#### Phase 2: Advanced Features (Week 3-4)

1. Enable PagedAttention with configurable parameters
2. Add X-LoRA adapter loading and routing
3. Implement ISQ with calibration pipeline

#### Phase 3: Hardware Acceleration (Week 5-6)

1. Test and validate Metal acceleration
2. Test and validate CUDA acceleration
3. Benchmark against Candle backend

---

## Consequences

### Positive Consequences

1. **Production-scale serving**: PagedAttention enables 5-10x more concurrent users
2. **Multi-task efficiency**: X-LoRA eliminates adapter switching overhead
3. **Deployment flexibility**: ISQ allows runtime quantization decisions
4. **Upstream maintenance**: mistral-rs team maintains core inference optimizations
5. **Feature parity**: Access to latest mistral-rs features (Flash Attention 2, speculative decoding)

### Negative Consequences

1. **Dependency complexity**: Additional crate dependencies increase build complexity
2. **API surface coupling**: Changes in mistral-rs may require RuvLLM updates
3. **Feature matrix**: Two backend codepaths require testing both paths
4. **WASM incompatibility**: mistral-rs does not support WASM targets

### Neutral Consequences

1. **Two backend options**: Candle remains optimal for edge/WASM; mistral-rs for server
2. **Compile-time selection**: Users choose backend via feature flags
3. **Binary size tradeoff**: Server builds are larger; edge builds unchanged

### Risk Mitigation

| Risk | Mitigation |
|------|------------|
| mistral-rs API instability | Pin to specific version; abstract via MistralBackend interface |
| Feature flag complexity | Comprehensive CI matrix testing all feature combinations |
| Performance regression | Benchmark suite comparing Candle vs mistral-rs |
| Metal/CUDA compatibility | Platform-specific CI runners for hardware validation |

---

## Alternatives Considered

### llama.cpp via rust-llama

- **Rejected**: Different model format (GGUF), weaker Rust integration
- **Consideration**: Could add as third backend for GGUF model support

### candle-transformers PagedAttention

- **Rejected**: Candle's PagedAttention is experimental and less mature
- **Consideration**: Monitor upstream development

### vLLM Python Backend

- **Rejected**: Python FFI adds latency; deployment complexity
- **Consideration**: vLLM's algorithm informs our understanding

---

## Related Decisions

- **ADR-001**: Ruvector Core Architecture (HNSW, Graph Store)
- **ADR-002**: RuvLLM Integration with Ruvector
- **ADR-003**: SIMD Optimization Strategy
- **ADR-004**: KV Cache Management
- **ADR-006**: Memory Management
- **ADR-007**: Security Review & Technical Debt

---

## Compliance and Standards

### API Compatibility
- `MistralBackend` implements `LlmBackend` trait
- All existing RuvLLM consumers work unchanged
- Feature flags are additive (no breaking changes)

### Testing Requirements
- Unit tests for config mapping
- Integration tests with sample models
- Benchmark suite comparing backends
- CI matrix for feature flag combinations

### Documentation Requirements
- Feature flag documentation in README
- Backend selection guide
- Performance comparison benchmarks

---

## References

1. mistral-rs Repository: https://github.com/EricLBuehler/mistral.rs
2. vLLM PagedAttention Paper: "Efficient Memory Management for Large Language Model Serving with PagedAttention"
3. X-LoRA Paper: "X-LoRA: Mixture of Low-Rank Adapter Experts"
4. ISQ/AWQ Paper: "AWQ: Activation-aware Weight Quantization for LLM Compression"
5. Existing MistralBackend stub: `crates/ruvllm/src/backends/mistral_backend.rs`

---

## Implementation Status

| Component | Status | Notes |
|-----------|--------|-------|
| Feature flags | Pending | Add to Cargo.toml |
| Config mapping | Pending | MistralBackendConfig -> mistralrs::Config |
| Model loading | Pending | Wire to mistral-rs loader |
| Generation | Pending | Wire to mistral-rs inference |
| PagedAttention | Pending | Enable via config |
| X-LoRA | Pending | Wire existing XLoraManager |
| ISQ | Pending | Implement calibration pipeline |
| Metal acceleration | Pending | Test on Apple Silicon |
| CUDA acceleration | Pending | Test on NVIDIA GPUs |

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-20 | Ruvector Architecture Team | Initial proposal |
