# ruv-neural-memory

Persistent neural state memory with vector search and longitudinal tracking.

## Overview

`ruv-neural-memory` provides in-memory and persistent storage for neural
embeddings, supporting brute-force and HNSW-based approximate nearest neighbor
search. It includes session-based memory management for organizing recordings
by subject and session, longitudinal drift detection for tracking embedding
distribution changes over time, and RVF/bincode persistence for durable storage.

## Features

- **Embedding store** (`store`): `NeuralMemoryStore` for inserting, querying,
  and managing collections of `NeuralEmbedding` values with brute-force
  nearest neighbor search
- **HNSW index** (`hnsw`): `HnswIndex` for approximate nearest neighbor search
  with configurable M (max connections), ef_construction, and ef_search parameters;
  provides 150x-12,500x speedup over brute-force for large collections
- **Session management** (`session`): `SessionMemory` and `SessionMetadata` for
  organizing embeddings by recording session, subject ID, and timestamp ranges
- **Longitudinal tracking** (`longitudinal`): `LongitudinalTracker` for detecting
  embedding distribution drift over time with `TrendDirection` classification
  (stable, increasing, decreasing)
- **Persistence** (`persistence`): `save_store` / `load_store` for bincode
  serialization, `save_rvf` / `load_rvf` for RuVector format I/O

## Usage

```rust
use ruv_neural_memory::{
    NeuralMemoryStore, HnswIndex, SessionMemory, SessionMetadata,
    LongitudinalTracker, save_store, load_store,
};
use ruv_neural_core::{NeuralEmbedding, EmbeddingMetadata, Atlas};

// Create a memory store and insert embeddings
let mut store = NeuralMemoryStore::new();
let meta = EmbeddingMetadata {
    subject_id: Some("sub-01".into()),
    session_id: Some("ses-01".into()),
    cognitive_state: None,
    source_atlas: Atlas::Schaefer100,
    embedding_method: "spectral".into(),
};
let emb = NeuralEmbedding::new(vec![0.1, 0.5, -0.3], 0.0, meta).unwrap();
store.insert(emb);

// Query nearest neighbors (brute-force)
let query = vec![0.1, 0.4, -0.2];
let neighbors = store.query_nearest(&query, 5);

// Build HNSW index for fast approximate search
let mut hnsw = HnswIndex::new(16, 200);
// ... insert vectors, then search

// Session-based memory management
let session = SessionMemory::new(SessionMetadata {
    subject_id: "sub-01".into(),
    session_id: "ses-01".into(),
    ..Default::default()
});

// Persistence
save_store(&store, "memory.bin").unwrap();
let loaded = load_store("memory.bin").unwrap();
```

## API Reference

| Module          | Key Types / Functions                                       |
|-----------------|-------------------------------------------------------------|
| `store`         | `NeuralMemoryStore`                                         |
| `hnsw`          | `HnswIndex`                                                 |
| `session`       | `SessionMemory`, `SessionMetadata`                          |
| `longitudinal`  | `LongitudinalTracker`, `TrendDirection`                     |
| `persistence`   | `save_store`, `load_store`, `save_rvf`, `load_rvf`          |

## Feature Flags

| Feature | Default | Description                  |
|---------|---------|------------------------------|
| `std`   | Yes     | Standard library support     |
| `wasm`  | No      | WASM-compatible storage      |

## Integration

Depends on `ruv-neural-core` for `NeuralEmbedding` types. Receives embeddings
from `ruv-neural-embed`. Stored embeddings are queried by `ruv-neural-decoder`
for KNN-based cognitive state classification. Uses `bincode` for efficient
binary serialization.

## License

MIT OR Apache-2.0
