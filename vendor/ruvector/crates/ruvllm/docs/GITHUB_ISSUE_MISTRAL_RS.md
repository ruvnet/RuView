# feat(ruvllm): Full mistral-rs backend integration with PagedAttention, X-LoRA, and ISQ

## Summary

Wire the existing `MistralBackend` stub to the actual [mistral-rs](https://github.com/EricLBuehler/mistral.rs) crate for production-scale LLM serving with advanced memory management and adapter routing.

## Motivation

The current Candle backend is optimized for single-user and edge deployment scenarios, achieving approximately 100 tokens/second. While sufficient for development and small-scale use, production deployments require significantly higher throughput and concurrency.

**mistral-rs enables:**
- **500-1000 tok/s throughput** via continuous batching and PagedAttention
- **50+ concurrent users** with efficient KV cache management
- **Memory efficiency** through paged memory allocation and prefix caching
- **Dynamic adapter routing** via X-LoRA for multi-task inference
- **Runtime quantization** via ISQ for deployment flexibility

### Performance Comparison

| Metric | Candle Backend | mistral-rs Backend |
|--------|----------------|-------------------|
| Throughput | ~100 tok/s | 500-1000 tok/s |
| Concurrent Users | 1-5 | 50+ |
| Memory Efficiency | Static KV | Paged + Prefix Cache |
| Adapter Support | Static LoRA | Dynamic X-LoRA |
| Quantization | Pre-quantized only | Runtime ISQ |

## Features to Implement

### 1. PagedAttention (Priority: High)

PagedAttention revolutionizes KV cache management by treating attention as virtual memory, enabling efficient memory sharing across sequences.

- [ ] Add `mistralrs` dependency to `Cargo.toml` with feature flags
- [ ] Wire PagedAttention to `MistralBackend::generate()`
- [ ] Implement sequence allocation/deallocation callbacks
- [ ] Add prefix caching support for prompt reuse
- [ ] Configure block size and max sequences
- [ ] Benchmark: target 5-10x concurrent capacity improvement

**Key Implementation Points:**
```rust
// Block configuration
let paged_config = PagedAttentionConfig {
    block_size: 16,           // Tokens per block
    max_num_blocks: 1024,     // Total blocks available
    sliding_window: None,     // Optional sliding window
    prefix_caching: true,     // Enable prefix cache
};
```

### 2. X-LoRA Dynamic Routing (Priority: Medium)

X-LoRA enables per-token routing to different LoRA adapters, allowing a single model to handle multiple tasks efficiently.

- [ ] Wire `XLoraManager` to mistral-rs X-LoRA implementation
- [ ] Implement per-token adapter routing logic
- [ ] Support learned routing networks (classifier)
- [ ] Add adapter hot-loading for runtime updates
- [ ] Implement adapter weight caching
- [ ] Benchmark: multi-task quality metrics vs single adapters

**Key Implementation Points:**
```rust
// X-LoRA configuration
let xlora_config = XLoraConfig {
    adapters: vec![
        ("code", "path/to/code-lora"),
        ("chat", "path/to/chat-lora"),
        ("reasoning", "path/to/reasoning-lora"),
    ],
    routing_method: RoutingMethod::Learned,
    top_k_adapters: 2,        // Use top-2 adapters per token
    scaling_factor: 1.0,
};
```

### 3. ISQ Runtime Quantization (Priority: Medium)

In-Situ Quantization allows loading full-precision models and quantizing at runtime, providing deployment flexibility.

- [ ] Wire `IsqConfig` to mistral-rs ISQ implementation
- [ ] Support quantization methods: AWQ, GPTQ, RTN, SmoothQuant
- [ ] Implement calibration workflow with sample data
- [ ] Add memory estimation before/after quantization
- [ ] Support mixed-precision quantization per layer
- [ ] Benchmark: quality vs compression tradeoffs

**Supported Quantization Methods:**
| Method | Bits | Quality | Speed | Use Case |
|--------|------|---------|-------|----------|
| AWQ | 4-bit | High | Fast | Production |
| GPTQ | 4-bit | High | Medium | Accuracy-critical |
| RTN | 8-bit | Very High | Very Fast | Quality-first |
| SmoothQuant | 8-bit | Very High | Fast | Balanced |

## Technical Details

### Cargo.toml Changes

```toml
[dependencies]
# Core mistral-rs integration
mistralrs = { version = "0.4", optional = true }
mistralrs-core = { version = "0.4", optional = true }

# Required for tokenization with mistral-rs
tokenizers = { version = "0.20", optional = true }

[features]
default = ["candle"]

# Base mistral-rs support (CPU)
mistral-rs = ["mistralrs", "mistralrs-core", "tokenizers"]

# Metal acceleration (macOS)
mistral-rs-metal = ["mistral-rs", "mistralrs/metal"]

# CUDA acceleration (NVIDIA)
mistral-rs-cuda = ["mistral-rs", "mistralrs/cuda"]

# Full feature set
full = ["candle", "mistral-rs"]
```

### Files to Modify

| File | Changes |
|------|---------|
| `crates/ruvllm/Cargo.toml` | Add mistral-rs dependencies and feature flags |
| `crates/ruvllm/src/backends/mistral_backend.rs` | Replace stub with real implementation |
| `crates/ruvllm/src/backends/mod.rs` | Update conditional exports |
| `crates/ruvllm/src/paged_attention.rs` | Wire to mistral-rs PagedAttention |
| `crates/ruvllm/src/xlora_manager.rs` | Wire to mistral-rs X-LoRA |
| `crates/ruvllm/src/isq.rs` | Wire to mistral-rs ISQ |
| `crates/ruvllm/src/lib.rs` | Add re-exports and feature gates |
| `crates/ruvllm/README.md` | Document usage and examples |

### API Design

```rust
use ruvllm::{MistralBackend, MistralConfig, PagedAttentionConfig};

// Create backend with PagedAttention
let config = MistralConfig {
    model_id: "mistralai/Mistral-7B-Instruct-v0.2".to_string(),
    paged_attention: Some(PagedAttentionConfig {
        block_size: 16,
        max_num_blocks: 1024,
        prefix_caching: true,
    }),
    xlora: None,
    isq: None,
};

let backend = MistralBackend::new(config).await?;

// Generate with automatic KV cache management
let output = backend.generate(&request).await?;
```

### Feature Flag Matrix

| Build Command | CPU | Metal | CUDA | PagedAttn | X-LoRA | ISQ |
|---------------|-----|-------|------|-----------|--------|-----|
| `--features mistral-rs` | Yes | No | No | Yes | Yes | Yes |
| `--features mistral-rs-metal` | Yes | Yes | No | Yes | Yes | Yes |
| `--features mistral-rs-cuda` | Yes | No | Yes | Yes | Yes | Yes |

## Acceptance Criteria

### Build Verification
- [ ] `cargo build --features mistral-rs` compiles on Linux
- [ ] `cargo build --features mistral-rs-metal` compiles on macOS
- [ ] `cargo build --features mistral-rs-cuda` compiles with CUDA toolkit
- [ ] All clippy warnings resolved
- [ ] No breaking changes to existing Candle backend

### Functionality
- [ ] Model loading works with HuggingFace model IDs
- [ ] Model loading works with local paths
- [ ] Generation produces correct, coherent output
- [ ] Streaming generation works correctly
- [ ] Stop sequences are respected

### PagedAttention
- [ ] KV cache is managed in blocks
- [ ] Sequence allocation succeeds up to max capacity
- [ ] Sequence deallocation frees blocks correctly
- [ ] Prefix caching improves repeated prompt performance
- [ ] Memory usage stays within configured limits

### X-LoRA
- [ ] Multiple adapters can be loaded
- [ ] Per-token routing selects appropriate adapters
- [ ] Adapter hot-loading works without restart
- [ ] Quality matches or exceeds single-adapter baseline

### ISQ
- [ ] Models quantize at runtime without pre-quantized weights
- [ ] All supported methods produce valid output
- [ ] Memory reduction matches expected compression ratio
- [ ] Quality degradation within acceptable bounds (<5% on benchmarks)

### Performance Benchmarks
- [ ] Throughput: >500 tok/s on Mistral-7B (single user)
- [ ] Concurrency: >50 concurrent generations without OOM
- [ ] Latency: <50ms time-to-first-token
- [ ] Memory: PagedAttention reduces peak usage by >30%

## Testing Plan

### Unit Tests
```rust
#[cfg(feature = "mistral-rs")]
mod mistral_tests {
    #[tokio::test]
    async fn test_model_loading() { ... }

    #[tokio::test]
    async fn test_generation() { ... }

    #[tokio::test]
    async fn test_paged_attention_allocation() { ... }

    #[tokio::test]
    async fn test_xlora_routing() { ... }

    #[tokio::test]
    async fn test_isq_quantization() { ... }
}
```

### Integration Tests
- Model download and cache management
- End-to-end generation pipeline
- Concurrent request handling
- Memory pressure scenarios

### Benchmarks
```bash
# Run throughput benchmark
cargo bench --features mistral-rs-metal -- throughput

# Run concurrency benchmark
cargo bench --features mistral-rs-metal -- concurrency

# Run memory benchmark
cargo bench --features mistral-rs-metal -- memory
```

## Implementation Notes

### Thread Safety
mistral-rs uses async Rust throughout. Ensure all shared state is properly synchronized:
- Use `Arc<RwLock<>>` for shared configuration
- Use channels for sequence lifecycle events
- Avoid blocking in async contexts

### Error Handling
Map mistral-rs errors to ruvllm error types:
```rust
impl From<mistralrs::Error> for RuvllmError {
    fn from(e: mistralrs::Error) -> Self {
        match e {
            mistralrs::Error::ModelLoad(_) => RuvllmError::ModelLoad(...),
            mistralrs::Error::Generation(_) => RuvllmError::Generation(...),
            // ...
        }
    }
}
```

### Backward Compatibility
- Keep Candle backend as default
- Use feature flags for mistral-rs
- Maintain consistent API across backends
- Document migration path

## Related Issues

- Depends on: Initial MistralBackend stub implementation
- Blocks: Production deployment readiness
- Related: Candle backend optimizations

## References

- [mistral-rs GitHub](https://github.com/EricLBuehler/mistral.rs)
- [PagedAttention Paper](https://arxiv.org/abs/2309.06180)
- [X-LoRA Paper](https://arxiv.org/abs/2402.07148)
- [AWQ Paper](https://arxiv.org/abs/2306.00978)
- [vLLM PagedAttention](https://blog.vllm.ai/2023/06/20/vllm.html)

---

**Labels:** `enhancement`, `ruvllm`, `backend`, `performance`, `P1`

**Milestone:** v0.2.0

**Assignees:** TBD
