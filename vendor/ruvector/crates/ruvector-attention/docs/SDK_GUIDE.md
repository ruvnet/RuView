# ruvector-attention SDK Guide

## Overview

The ruvector-attention SDK provides high-level, ergonomic APIs for building attention mechanisms. It includes three main components:

1. **Builder API** - Fluent interface for configuring attention
2. **Pipeline API** - Composable operations with normalization and residuals
3. **Presets** - Ready-to-use configurations for common models

## Quick Start

### Basic Usage

```rust
use ruvector_attention::sdk::*;

// Create a simple multi-head attention
let attention = multi_head(768, 12)
    .dropout(0.1)
    .causal(true)
    .build()?;

// Use it
let query = vec![0.5; 768];
let keys = vec![&query[..]; 10];
let values = vec![&query[..]; 10];

let output = attention.compute(&query, &keys, &values)?;
```

### Using Presets

```rust
use ruvector_attention::sdk::presets::*;

// BERT-style attention
let bert = AttentionPreset::Bert.builder(768).build()?;

// GPT-style causal attention
let gpt = AttentionPreset::Gpt.builder(768).build()?;

// Flash attention for long sequences
let flash = AttentionPreset::FlashOptimized.builder(1024).build()?;

// Automatic selection based on sequence length
let auto = for_sequences(512, 8192).build()?;
```

### Building Pipelines

```rust
use ruvector_attention::sdk::*;

// Create a transformer block
let attention = multi_head(768, 12).build()?;

let pipeline = AttentionPipeline::new()
    .add_attention(attention)
    .add_dropout(0.1)
    .add_residual()
    .add_norm(NormType::LayerNorm);

// Run the pipeline
let output = pipeline.run(&query, &keys, &values)?;
```

## Builder API

### Available Attention Types

#### 1. Scaled Dot-Product Attention

The fundamental attention mechanism: `softmax(QK^T / √d)V`

```rust
let attention = scaled_dot(512).build()?;
```

#### 2. Multi-Head Attention

Parallel attention heads for diverse representation learning:

```rust
let attention = multi_head(768, 12)
    .dropout(0.1)
    .build()?;
```

#### 3. Flash Attention

Memory-efficient O(n) attention using tiled computation:

```rust
let attention = flash(1024, 128)  // dim, block_size
    .causal(true)
    .build()?;
```

#### 4. Linear Attention

O(n) complexity using kernel feature maps:

```rust
let attention = linear(512, 256)  // dim, num_features
    .build()?;
```

#### 5. Local-Global Attention

Sliding window + global tokens (Longformer-style):

```rust
let attention = local_global(512, 256)  // dim, window_size
    .build()?;
```

#### 6. Hyperbolic Attention

Attention in hyperbolic space for hierarchical data:

```rust
let attention = hyperbolic(512, -1.0)  // dim, curvature
    .build()?;
```

#### 7. Mixture-of-Experts Attention

Learned routing to specialized experts:

```rust
let attention = moe(512, 8, 2)  // dim, num_experts, top_k
    .expert_capacity(1.25)
    .jitter_noise(0.01)
    .build()?;
```

### Builder Options

All builders support these common options:

```rust
let attention = AttentionBuilder::new(512)
    .multi_head(8)           // Number of heads
    .dropout(0.1)            // Dropout probability
    .causal(true)            // Causal masking
    .expert_capacity(1.25)   // MoE capacity factor
    .jitter_noise(0.01)      // MoE routing noise
    .build()?;
```

## Pipeline API

### Creating Pipelines

```rust
let pipeline = AttentionPipeline::new()
    .add_attention(attention)
    .add_norm(NormType::LayerNorm)
    .add_dropout(0.1)
    .add_residual()
    .add_custom(|x| {
        // Custom transformation
        x.iter().map(|v| v.max(0.0)).collect()
    });
```

### Normalization Types

```rust
// Layer Normalization (standard)
.add_norm(NormType::LayerNorm)

// RMS Normalization (simpler)
.add_norm(NormType::RMSNorm)

// Batch Normalization
.add_norm(NormType::BatchNorm)
```

### Pre-built Transformers

```rust
// Standard post-norm transformer block
let block = transformer_block(attention, 0.1);

// Pre-norm transformer block (more stable)
let block = prenorm_transformer_block(attention, 0.1);
```

## Presets

### Model Presets

```rust
// BERT (bidirectional, 12 heads, 0.1 dropout)
AttentionPreset::Bert.builder(768)

// GPT (causal, 12 heads, 0.1 dropout)
AttentionPreset::Gpt.builder(768)

// Longformer (512 window, local-global)
AttentionPreset::Longformer.builder(512)

// Performer (linear attention, O(n))
AttentionPreset::Performer.builder(512)

// Flash (memory-efficient, 128 block)
AttentionPreset::FlashOptimized.builder(1024)

// Switch Transformer (8 experts, top-2)
AttentionPreset::SwitchTransformer.builder(512)

// Hyperbolic (hierarchical data)
AttentionPreset::HyperbolicTree.builder(512)

// T5 (encoder-decoder)
AttentionPreset::T5.builder(768)

// Vision Transformer
AttentionPreset::ViT.builder(768)

// Sparse Transformer
AttentionPreset::SparseTransformer.builder(512)
```

### Smart Selection

The SDK provides intelligent preset selection:

```rust
// Automatic based on sequence length
let attention = for_sequences(512, max_len).build()?;
// ≤512: BERT
// ≤4096: Longformer
// >4096: Performer

// Graph attention
let attention = for_graphs(256, hierarchical).build()?;
// hierarchical=true: Hyperbolic
// hierarchical=false: Multi-head

// Large-scale processing
let attention = for_large_scale(1024).build()?;
// Uses Flash attention

// Vision tasks
let attention = for_vision(768, patch_size).build()?;
// Uses ViT configuration

// Autoregressive generation
let attention = for_generation(768, context_len).build()?;
// ≤2048: GPT
// >2048: Flash with causal

// MoE with custom routing
let attention = for_moe(512, num_experts, top_k).build()?;
```

### From Model Names

```rust
// By model name (case-insensitive)
let bert = from_model_name("bert", 768)?;
let gpt = from_model_name("gpt2", 768)?;
let longformer = from_model_name("longformer", 512)?;
let t5 = from_model_name("t5", 768)?;
let vit = from_model_name("vit", 768)?;
```

## Advanced Examples

### Custom Transformer Layer

```rust
use ruvector_attention::sdk::*;

fn create_transformer_layer(dim: usize, num_heads: usize) -> AttentionResult<AttentionPipeline> {
    let attention = multi_head(dim, num_heads)
        .dropout(0.1)
        .build()?;

    Ok(AttentionPipeline::new()
        .add_norm(NormType::LayerNorm)  // Pre-norm
        .add_attention(attention)
        .add_dropout(0.1)
        .add_residual()
        .add_norm(NormType::LayerNorm)) // Post-norm
}
```

### Efficient Long-Sequence Processing

```rust
use ruvector_attention::sdk::*;

fn create_long_context_attention(dim: usize, max_len: usize) -> AttentionResult<Box<dyn Attention>> {
    if max_len <= 2048 {
        // Standard attention for short sequences
        multi_head(dim, 12).build()
    } else if max_len <= 16384 {
        // Local-global for medium sequences
        local_global(dim, 512).build()
    } else {
        // Linear attention for very long sequences
        linear(dim, dim / 4).build()
    }
}
```

### Hierarchical Graph Attention

```rust
use ruvector_attention::sdk::*;

fn create_graph_attention(dim: usize, is_tree: bool) -> AttentionResult<Box<dyn Attention>> {
    if is_tree {
        // Use hyperbolic space for tree-like structures
        hyperbolic(dim, -1.0).build()
    } else {
        // Standard attention for general graphs
        multi_head(dim, 8).build()
    }
}
```

### Sparse + Dense Hybrid

```rust
use ruvector_attention::sdk::*;

fn create_hybrid_pipeline(dim: usize) -> AttentionResult<AttentionPipeline> {
    // Local attention
    let local = flash(dim, 128).build()?;

    // Global attention (can be added in sequence)
    let global = multi_head(dim, 8).build()?;

    Ok(AttentionPipeline::new()
        .add_attention(local)
        .add_norm(NormType::LayerNorm)
        .add_residual())
}
```

### MoE for Specialized Tasks

```rust
use ruvector_attention::sdk::*;

fn create_moe_attention(dim: usize) -> AttentionResult<Box<dyn Attention>> {
    moe(dim, 16, 2)  // 16 experts, route to top-2
        .expert_capacity(1.5)  // Higher capacity for load balancing
        .jitter_noise(0.1)     // Exploration during training
        .build()
}
```

## Performance Tips

1. **Choose the right attention type:**
   - Short sequences (<512): Standard multi-head
   - Medium sequences (512-4096): Local-global or Flash
   - Long sequences (>4096): Linear or Performer
   - Hierarchical data: Hyperbolic
   - Specialized patterns: MoE

2. **Use Flash attention for:**
   - Long sequences
   - Memory-constrained environments
   - Training with limited GPU memory

3. **Use Linear attention for:**
   - Very long sequences (>16k tokens)
   - Inference-only scenarios
   - Real-time applications

4. **Use MoE for:**
   - Multi-task learning
   - Specialized domain processing
   - Scaling model capacity

5. **Pipeline optimization:**
   - Pre-norm is more stable for deep models
   - RMSNorm is faster than LayerNorm
   - Dropout during training only

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attention_pipeline() {
        let attention = multi_head(512, 8).build().unwrap();
        let pipeline = AttentionPipeline::new()
            .add_attention(attention)
            .add_norm(NormType::LayerNorm);

        let query = vec![0.5; 512];
        let keys = vec![&query[..]; 10];
        let values = vec![&query[..]; 10];

        let output = pipeline.run(&query, &keys, &values).unwrap();
        assert_eq!(output.len(), 512);
    }
}
```

## Next Steps

- See `examples/` directory for complete working examples
- Check the API documentation for detailed parameter descriptions
- Review benchmarks in `benches/` for performance comparisons
