# RuVector Rust Examples

Core Rust SDK examples demonstrating RuVector's vector database capabilities.

## Examples

| File | Description |
|------|-------------|
| `basic_usage.rs` | Getting started with vector DB operations |
| `batch_operations.rs` | High-throughput batch ingestion |
| `rag_pipeline.rs` | Retrieval-Augmented Generation pipeline |
| `advanced_features.rs` | Hypergraphs, neural hashing, topology |
| `agenticdb_demo.rs` | AI agent memory with 5 tables |
| `gnn_example.rs` | Graph Neural Network layer usage |

## Quick Start

```bash
# Run basic example
cargo run --example basic_usage

# Run with release optimizations
cargo run --release --example advanced_features
```

## Basic Usage

```rust
use ruvector_core::{VectorDB, VectorEntry, DbOptions, Result};

fn main() -> Result<()> {
    // Create database
    let mut options = DbOptions::default();
    options.dimensions = 128;
    let db = VectorDB::new(options)?;

    // Insert vector
    let entry = VectorEntry {
        id: Some("doc_001".to_string()),
        vector: vec![0.1; 128],
        metadata: None,
    };
    db.insert(entry)?;

    // Search
    let results = db.search(&vec![0.1; 128], 10)?;
    Ok(())
}
```

## Advanced Features

### Hypergraph Index
Multi-entity relationships with weighted edges.

```rust
use ruvector_core::advanced::*;

let mut index = HypergraphIndex::new(DistanceMetric::Cosine);
index.add_entity(1, vec![0.9, 0.1, 0.0]);
index.add_entity(2, vec![0.8, 0.2, 0.0]);

let edge = Hyperedge::new(
    vec![1, 2],
    "Co-cited papers".to_string(),
    vec![0.7, 0.2, 0.1],
    0.95,
);
index.add_hyperedge(edge)?;
```

### Temporal Hypergraph
Time-aware relationships for event tracking.

```rust
let mut temporal = TemporalHypergraph::new(DistanceMetric::Cosine);
temporal.add_entity_at_time(1, vec![0.5; 3], 1000);
temporal.add_entity_at_time(1, vec![0.6; 3], 2000); // Entity evolves
```

### Causal Memory
Cause-effect relationship chains.

```rust
let mut causal = CausalMemory::new(DistanceMetric::Cosine);
let id1 = causal.add_pattern(vec![0.9, 0.1], "initial event")?;
let id2 = causal.add_pattern_with_cause(
    vec![0.8, 0.2],
    "consequence",
    id1, // Caused by id1
    0.9  // High confidence
)?;
```

### Learned Index
ML-optimized index structure.

```rust
let mut learned = LearnedIndex::new(DistanceMetric::Cosine);
learned.set_model_type(ModelType::LinearRegression);
for (i, vec) in vectors.iter().enumerate() {
    learned.insert(i, vec.clone())?;
}
learned.train()?; // Train the model
```

### Neural Hash
Locality-sensitive hashing.

```rust
let neural_hash = NeuralHash::new(128, 64, 8)?;
let hash = neural_hash.hash(&vector)?;
let candidates = neural_hash.query_approximate(&query, 10)?;
```

## AgenticDB Tables

| Table | Purpose |
|-------|---------|
| `reflexion_episodes` | Self-critique memories |
| `skill_library` | Consolidated patterns |
| `causal_memory` | Hypergraph relationships |
| `learning_sessions` | RL training data |
| `vector_db` | Core embeddings |

```rust
use ruvector_core::AgenticDB;

let db = AgenticDB::new(options)?;

// Store reflexion episode
db.store_episode(
    "Task description".to_string(),
    vec!["Action 1".to_string()],
    vec!["Error observed".to_string()],
    "What I learned".to_string(),
)?;

// Query similar past experiences
let episodes = db.query_similar_episodes(&embedding, 5)?;
```

## GNN Layer

```rust
use ruvector_gnn::RuvectorLayer;

let gnn = RuvectorLayer::new(128, 256, 4, 0.1);
let node = vec![0.5; 128];
let neighbors = vec![vec![0.3; 128], vec![0.7; 128]];
let weights = vec![0.8, 0.6];

let updated = gnn.forward(&node, &neighbors, &weights);
```

## Performance Tips

1. **Batch Operations**: Use `insert_batch` for bulk inserts
2. **Dimension**: Match embedding dimensions exactly
3. **Index Type**: Choose based on query patterns
4. **Distance Metric**: Cosine for normalized, Euclidean for raw

## Dependencies

```toml
[dependencies]
ruvector-core = "0.1"
ruvector-gnn = "0.1"
```
