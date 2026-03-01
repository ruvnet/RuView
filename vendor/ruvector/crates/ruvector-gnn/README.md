# Ruvector GNN

[![Crates.io](https://img.shields.io/crates/v/ruvector-gnn.svg)](https://crates.io/crates/ruvector-gnn)
[![Documentation](https://docs.rs/ruvector-gnn/badge.svg)](https://docs.rs/ruvector-gnn)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.77%2B-orange.svg)](https://www.rust-lang.org)

**A Graph Neural Network layer that makes HNSW vector search get smarter over time.**

Most vector indexes return the same results every time you search. `ruvector-gnn` adds a GNN layer on top of HNSW that learns from your query patterns -- so search results actually improve with use. It runs message passing directly on the HNSW graph structure with SIMD acceleration, keeping latency low even on large indexes. Part of the [RuVector](https://github.com/ruvnet/ruvector) ecosystem.

| | ruvector-gnn | Standard HNSW Search |
|---|---|---|
| **Search quality** | GNN re-ranks neighbors using learned attention weights -- results improve over time | Static ranking -- same results every time |
| **Graph awareness** | Operates directly on HNSW topology; understands graph structure | Treats index as a flat lookup table |
| **Attention mechanisms** | Multi-head GAT weighs which neighbors matter for each query | No attention -- all neighbors weighted equally |
| **Inductive learning** | GraphSAGE generalizes to unseen nodes without retraining | Cannot learn from new data |
| **Hardware acceleration** | SIMD-optimized aggregation; memory-mapped weights for large models | Basic distance calculations only |
| **Deployment** | Native Rust, Node.js (NAPI-RS), and WASM from the same crate | Typically single-platform |

## Installation

Add `ruvector-gnn` to your `Cargo.toml`:

```toml
[dependencies]
ruvector-gnn = "0.1.1"
```

### Feature Flags

```toml
[dependencies]
# Default with SIMD and memory mapping
ruvector-gnn = { version = "0.1.1", features = ["simd", "mmap"] }

# WASM-compatible build
ruvector-gnn = { version = "0.1.1", default-features = false, features = ["wasm"] }

# Node.js bindings
ruvector-gnn = { version = "0.1.1", features = ["napi"] }
```

Available features:
- `simd` (default): SIMD-optimized operations
- `mmap` (default): Memory-mapped weight storage
- `wasm`: WebAssembly-compatible build
- `napi`: Node.js bindings via NAPI-RS

## Key Features

| Feature | What It Does | Why It Matters |
|---------|-------------|----------------|
| **GCN Layers** | Graph Convolutional Network forward pass over HNSW neighbors | Learns structural patterns in your data without manual feature engineering |
| **GAT Layers** | Multi-head Graph Attention with interpretable weights | Automatically discovers which neighbors are most relevant per query |
| **GraphSAGE** | Inductive learning with neighbor sampling | Handles new, unseen nodes without retraining the full model |
| **SIMD Aggregation** | Hardware-accelerated message passing | Keeps GNN overhead under 15 ms for 100K-node graphs |
| **Memory Mapping** | Large model weights loaded via mmap | Run models bigger than RAM; only pages what's needed |
| **INT8/FP16 Quantization** | Compressed weight storage | 2-4x smaller models with minimal accuracy loss |
| **Custom Aggregators** | Mean, max, and LSTM aggregation modes | Tune the aggregation strategy to your data distribution |
| **Skip Connections** | Residual connections for deep GNN stacks | Train deeper networks without vanishing gradients |
| **Batch Processing** | Parallel message passing with Rayon | Saturates all cores during training and inference |
| **Layer Normalization** | Normalize activations between layers | Stable training dynamics across different graph sizes |

## Quick Start

### Basic GCN Layer

```rust
use ruvector_gnn::{GCNLayer, GNNConfig, MessagePassing};
use ndarray::Array2;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure GCN layer
    let config = GNNConfig {
        input_dim: 128,
        output_dim: 64,
        hidden_dim: 128,
        num_heads: 4,        // For GAT
        dropout: 0.1,
        activation: Activation::ReLU,
    };

    // Create GCN layer
    let gcn = GCNLayer::new(config)?;

    // Node features (num_nodes x input_dim)
    let features = Array2::zeros((1000, 128));

    // Adjacency list (HNSW neighbors)
    let adjacency: Vec<Vec<usize>> = /* from HNSW index */;

    // Forward pass
    let output = gcn.forward(&features, &adjacency)?;

    println!("Output shape: {:?}", output.shape());
    Ok(())
}
```

### Graph Attention Network

```rust
use ruvector_gnn::{GATLayer, AttentionConfig};

// Configure multi-head attention
let config = AttentionConfig {
    input_dim: 128,
    output_dim: 64,
    num_heads: 8,
    concat_heads: true,
    dropout: 0.1,
    leaky_relu_slope: 0.2,
};

let gat = GATLayer::new(config)?;

// Forward with attention
let (output, attention_weights) = gat.forward_with_attention(&features, &adjacency)?;

// Attention weights for interpretability
for (node_id, weights) in attention_weights.iter().enumerate() {
    println!("Node {}: attention weights = {:?}", node_id, weights);
}
```

### GraphSAGE with Custom Aggregator

```rust
use ruvector_gnn::{GraphSAGE, SAGEConfig, Aggregator};

let config = SAGEConfig {
    input_dim: 128,
    output_dim: 64,
    num_layers: 2,
    aggregator: Aggregator::Mean,
    sample_sizes: vec![10, 5],  // Neighbor sampling per layer
    normalize: true,
};

let sage = GraphSAGE::new(config)?;

// Mini-batch training with neighbor sampling
let embeddings = sage.forward_minibatch(
    &features,
    &adjacency,
    &batch_nodes,  // Target nodes
)?;
```

### Integration with Ruvector Core

```rust
use ruvector_core::VectorDB;
use ruvector_gnn::{HNSWMessagePassing, GNNEmbedder};

// Load vector database
let db = VectorDB::open("vectors.db")?;

// Create GNN that operates on HNSW structure
let gnn = GNNEmbedder::new(GNNConfig {
    input_dim: db.dimensions(),
    output_dim: 64,
    num_layers: 2,
    ..Default::default()
})?;

// Get HNSW neighbors for message passing
let hnsw_graph = db.get_hnsw_graph()?;

// Compute GNN embeddings
let gnn_embeddings = gnn.encode(&db.get_all_vectors()?, &hnsw_graph)?;

// Enhanced search using GNN embeddings
let results = db.search_with_gnn(&query_vector, &gnn, 10)?;
```

## API Overview

### Core Types

```rust
// GNN layer configuration
pub struct GNNConfig {
    pub input_dim: usize,
    pub output_dim: usize,
    pub hidden_dim: usize,
    pub num_heads: usize,
    pub dropout: f32,
    pub activation: Activation,
}

// Message passing interface
pub trait MessagePassing {
    fn aggregate(&self, features: &Array2<f32>, neighbors: &[Vec<usize>]) -> Array2<f32>;
    fn update(&self, aggregated: &Array2<f32>, self_features: &Array2<f32>) -> Array2<f32>;
    fn forward(&self, features: &Array2<f32>, adjacency: &[Vec<usize>]) -> Result<Array2<f32>>;
}

// Layer types
pub struct GCNLayer { /* ... */ }
pub struct GATLayer { /* ... */ }
pub struct GraphSAGE { /* ... */ }
```

### Layer Operations

```rust
impl GCNLayer {
    pub fn new(config: GNNConfig) -> Result<Self>;
    pub fn forward(&self, x: &Array2<f32>, adj: &[Vec<usize>]) -> Result<Array2<f32>>;
    pub fn save_weights(&self, path: &str) -> Result<()>;
    pub fn load_weights(&mut self, path: &str) -> Result<()>;
}

impl GATLayer {
    pub fn new(config: AttentionConfig) -> Result<Self>;
    pub fn forward(&self, x: &Array2<f32>, adj: &[Vec<usize>]) -> Result<Array2<f32>>;
    pub fn forward_with_attention(&self, x: &Array2<f32>, adj: &[Vec<usize>])
        -> Result<(Array2<f32>, Vec<Vec<f32>>)>;
}
```

## Performance

### Benchmarks (100K Nodes, Avg Degree 16)

```
Operation               Latency (p50)    GFLOPS
-----------------------------------------------------
GCN forward (1 layer)   ~15ms            12.5
GAT forward (8 heads)   ~45ms            8.2
GraphSAGE (2 layers)    ~25ms            10.1
Message aggregation     ~5ms             25.0
```

### Memory Usage

```
Model Size              Peak Memory
---------------------------------------
128 -> 64 (1 layer)     ~50MB
128 -> 64 (4 layers)    ~150MB
With mmap weights       ~10MB (+ disk)
```

## Related Crates

- **[ruvector-core](../ruvector-core/)** - Core vector database engine
- **[ruvector-gnn-node](../ruvector-gnn-node/)** - Node.js bindings
- **[ruvector-gnn-wasm](../ruvector-gnn-wasm/)** - WebAssembly bindings
- **[ruvector-graph](../ruvector-graph/)** - Graph database engine

## Documentation

- **[Main README](../../README.md)** - Complete project overview
- **[API Documentation](https://docs.rs/ruvector-gnn)** - Full API reference
- **[GitHub Repository](https://github.com/ruvnet/ruvector)** - Source code

## License

**MIT License** - see [LICENSE](../../LICENSE) for details.

---

<div align="center">

**Part of [RuVector](https://github.com/ruvnet/ruvector) - Built by [rUv](https://ruv.io)**

[![Star on GitHub](https://img.shields.io/github/stars/ruvnet/ruvector?style=social)](https://github.com/ruvnet/ruvector)

[Documentation](https://docs.rs/ruvector-gnn) | [Crates.io](https://crates.io/crates/ruvector-gnn) | [GitHub](https://github.com/ruvnet/ruvector)

</div>
