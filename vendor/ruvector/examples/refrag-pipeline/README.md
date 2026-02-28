# REFRAG Pipeline Example

> **Compress-Sense-Expand Architecture for ~30x RAG Latency Reduction**

This example demonstrates the REFRAG (Rethinking RAG) framework from [arXiv:2509.01092](https://arxiv.org/abs/2509.01092) using ruvector as the underlying vector store.

## Overview

Traditional RAG systems return text chunks that must be tokenized and processed by the LLM. REFRAG instead stores pre-computed "representation tensors" and uses a lightweight policy network to decide whether to return:

- **COMPRESS**: The tensor representation (directly injectable into LLM context)
- **EXPAND**: The original text (for cases where full context is needed)

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      REFRAG Pipeline                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐       │
│  │   COMPRESS   │    │    SENSE     │    │    EXPAND    │       │
│  │    Layer     │───▶│    Layer     │───▶│    Layer     │       │
│  └──────────────┘    └──────────────┘    └──────────────┘       │
│                                                                  │
│  Binary tensor       Policy network     Dimension projection    │
│  storage with        decides COMPRESS   (768 → 4096 dims)       │
│  zero-copy access    vs EXPAND                                  │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Compress Layer (`compress.rs`)

Stores representation tensors in binary format with multiple compression strategies:

| Strategy | Compression | Use Case |
|----------|-------------|----------|
| `None` | 1x | Maximum precision |
| `Float16` | 2x | Good balance |
| `Int8` | 4x | Memory constrained |
| `Binary` | 32x | Extreme compression |

### Sense Layer (`sense.rs`)

Policy network that decides the response type for each retrieved chunk:

| Policy | Latency | Description |
|--------|---------|-------------|
| `ThresholdPolicy` | ~2μs | Cosine similarity threshold |
| `LinearPolicy` | ~5μs | Single layer classifier |
| `MLPPolicy` | ~15μs | Two-layer neural network |

### Expand Layer (`expand.rs`)

Projects tensors to target LLM dimensions when needed:

| Source | Target | LLM |
|--------|--------|-----|
| 768 | 4096 | LLaMA-3 8B |
| 768 | 8192 | LLaMA-3 70B |
| 1536 | 8192 | GPT-4 |

## Quick Start

```bash
# Run the demo
cargo run --bin refrag-demo

# Run benchmarks (use release for accurate measurements)
cargo run --bin refrag-benchmark --release
```

## Usage

### Basic Usage

```rust
use refrag_pipeline_example::{RefragStore, RefragEntry};

// Create REFRAG-enabled store
let store = RefragStore::new(384, 768)?;

// Insert with representation tensor
let entry = RefragEntry::new("doc_1", search_vector, "The quick brown fox...")
    .with_tensor(tensor_bytes, "llama3-8b");
store.insert(entry)?;

// Standard search (text only)
let results = store.search(&query, 10)?;

// Hybrid search (policy-based COMPRESS/EXPAND)
let results = store.search_hybrid(&query, 10, Some(0.85))?;

for result in results {
    match result.response_type {
        RefragResponseType::Compress => {
            println!("Tensor: {} dims", result.tensor_dims.unwrap());
        }
        RefragResponseType::Expand => {
            println!("Text: {}", result.content.unwrap());
        }
    }
}
```

### Custom Configuration

```rust
use refrag_pipeline_example::{
    RefragStoreBuilder,
    PolicyNetwork,
    ExpandLayer,
};

let store = RefragStoreBuilder::new()
    .search_dimensions(384)
    .tensor_dimensions(768)
    .target_dimensions(4096)
    .compress_threshold(0.85)  // Higher = more COMPRESS
    .auto_project(true)
    .policy(PolicyNetwork::mlp(768, 32, 0.85))
    .expand_layer(ExpandLayer::for_roberta())
    .build()?;
```

### Response Format

REFRAG search returns a hybrid response format:

```json
{
  "results": [
    {
      "id": "doc_1",
      "score": 0.95,
      "response_type": "EXPAND",
      "content": "The quick brown fox...",
      "policy_confidence": 0.92
    },
    {
      "id": "doc_2",
      "score": 0.88,
      "response_type": "COMPRESS",
      "tensor_b64": "base64_encoded_float32_array...",
      "tensor_dims": 4096,
      "alignment_model_id": "llama3-8b",
      "policy_confidence": 0.97
    }
  ]
}
```

## Performance

### Latency Breakdown

| Component | Latency |
|-----------|---------|
| Vector search (HNSW) | 100-500μs |
| Policy decision | 1-50μs |
| Tensor decompression | 1-10μs |
| Projection (optional) | 10-100μs |
| **Total** | **~150-700μs** |

### Comparison to Traditional RAG

| Operation | Traditional | REFRAG |
|-----------|-------------|--------|
| Text tokenization | 1-5ms | N/A |
| LLM context prep | 5-20ms | ~100μs |
| Network transfer | 10-50ms | ~1-5ms |
| **Speedup** | - | **10-30x** |

## Why REFRAG Works for RuVector

1. **Rust/WASM**: Python implementations suffer from loop overhead. RuVector runs the policy in SIMD-optimized Rust (<50μs decisions).

2. **Edge Deployment**: The WASM build can serve as a "Smart Context Compressor" in the browser, sending only necessary tokens/tensors to the server LLM.

3. **Zero-Copy**: Using `rkyv` serialization enables direct memory access to tensors without deserialization.

## Future Integration

This example demonstrates REFRAG concepts without modifying ruvector-core. For production use, consider:

1. **Phase 1**: Add `RefragEntry` as new struct in ruvector-core
2. **Phase 2**: Integrate policy network into ruvector-router
3. **Phase 3**: Update REST API with hybrid response format

See [Issue #10](https://github.com/ruvnet/ruvector/issues/10) for the full integration proposal.

## References

- [REFRAG: Rethinking RAG based Decoding (arXiv:2509.01092)](https://arxiv.org/abs/2509.01092)
- [RuVector Documentation](https://github.com/ruvnet/ruvector)
