# ruv-neural-embed

Graph embedding generation for brain connectivity states using RuVector format.

## Overview

`ruv-neural-embed` converts brain connectivity graphs into fixed-dimensional
vector representations suitable for downstream classification, clustering, and
temporal analysis. It provides multiple embedding methods and supports export
to the RuVector `.rvf` binary format for interoperability with the broader
RuVector ecosystem.

## Features

- **Spectral embedding** (`spectral_embed`): Laplacian eigenvector-based positional
  encoding from the graph's normalized Laplacian
- **Topology embedding** (`topology_embed`): Hand-crafted topological feature vectors
  derived from graph-theoretic metrics
- **Node2Vec** (`node2vec`): Random-walk co-occurrence embeddings using configurable
  walk length, return parameter (p), and in-out parameter (q)
- **Combined embedding** (`combined`): Weighted concatenation of multiple embedding
  methods into a single vector
- **Temporal embedding** (`temporal`): Sliding-window context-enriched embeddings
  that capture graph dynamics over time
- **Distance metrics** (`distance`): Embedding distance and similarity computations
- **RVF export** (`rvf_export`): Serialization of embeddings and trajectories to the
  RuVector `.rvf` binary format
- **Helper utilities**: `default_metadata` for quick `EmbeddingMetadata` construction

## Usage

```rust
use ruv_neural_embed::{
    NeuralEmbedding, EmbeddingMetadata, EmbeddingTrajectory,
    default_metadata,
};
use ruv_neural_core::brain::Atlas;

// Create an embedding with metadata
let meta = default_metadata("spectral", Atlas::Schaefer100);
let emb = NeuralEmbedding::new(vec![0.1, 0.5, -0.3, 0.8], 1000.0, meta).unwrap();
assert_eq!(emb.dimension, 4);

// Compute similarity between embeddings
let other = NeuralEmbedding::new(
    vec![0.2, 0.4, -0.2, 0.9],
    1001.0,
    default_metadata("spectral", Atlas::Schaefer100),
).unwrap();
let similarity = emb.cosine_similarity(&other).unwrap();
let distance = emb.euclidean_distance(&other).unwrap();

// Build a trajectory from a sequence of embeddings
let trajectory = EmbeddingTrajectory {
    embeddings: vec![emb, other],
    timestamps: vec![1000.0, 1001.0],
};
assert_eq!(trajectory.len(), 2);
```

## API Reference

| Module           | Key Types / Functions                               |
|------------------|-----------------------------------------------------|
| `spectral_embed` | Spectral positional encoding from graph Laplacian   |
| `topology_embed` | Topological feature vector extraction               |
| `node2vec`       | Random-walk based node embeddings                   |
| `combined`       | Weighted multi-method embedding concatenation       |
| `temporal`       | Sliding-window temporal context embeddings          |
| `distance`       | Distance and similarity computations                |
| `rvf_export`     | RVF binary format serialization                     |

## Feature Flags

| Feature | Default | Description                         |
|---------|---------|-------------------------------------|
| `std`   | Yes     | Standard library support            |
| `wasm`  | No      | WASM-compatible implementations     |
| `rvf`   | No      | RuVector RVF format export support  |

## Integration

Depends on `ruv-neural-core` for `NeuralEmbedding`, `BrainGraph`, and
`EmbeddingGenerator` trait. Receives graphs from `ruv-neural-graph` or
`ruv-neural-mincut`. Produced embeddings are stored by `ruv-neural-memory`
and classified by `ruv-neural-decoder`.

## License

MIT OR Apache-2.0
