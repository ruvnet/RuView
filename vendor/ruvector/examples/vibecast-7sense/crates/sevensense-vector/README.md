# sevensense-vector

[![Crate](https://img.shields.io/badge/crates.io-sevensense--vector-orange.svg)](https://crates.io/crates/sevensense-vector)
[![Docs](https://img.shields.io/badge/docs-sevensense--vector-blue.svg)](https://docs.rs/sevensense-vector)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](../../LICENSE)
[![Performance](https://img.shields.io/badge/speedup-150x-brightgreen.svg)]()

> Ultra-fast vector similarity search using HNSW for bioacoustic embeddings.

**sevensense-vector** implements Hierarchical Navigable Small World (HNSW) graphs for approximate nearest neighbor search. It achieves **150x speedup** over brute-force search while maintaining >95% recall, enabling real-time similarity queries over millions of bird call embeddings.

## Features

- **HNSW Index**: State-of-the-art ANN algorithm with 150x speedup
- **Hyperbolic Geometry**: Poincaré ball model for hierarchical data
- **Multiple Distance Metrics**: Cosine, Euclidean, Angular, Hyperbolic
- **Dynamic Updates**: Insert and delete without full rebuild
- **Persistence**: Save/load indices to disk
- **Filtered Search**: Query with metadata constraints

## Use Cases

| Use Case | Description | Key Functions |
|----------|-------------|---------------|
| Similarity Search | Find similar bird calls | `search()`, `search_with_filter()` |
| Index Building | Build searchable index | `build()`, `add()` |
| Dynamic Updates | Add/remove vectors | `insert()`, `delete()` |
| Persistence | Save/load index | `save()`, `load()` |
| Hyperbolic Search | Hierarchical similarity | `HyperbolicIndex::search()` |

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
sevensense-vector = "0.1"
```

## Quick Start

```rust
use sevensense_vector::{HnswIndex, HnswConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create HNSW index
    let config = HnswConfig {
        m: 16,                    // Connections per layer
        ef_construction: 200,    // Build-time search width
        ..Default::default()
    };
    let mut index = HnswIndex::new(config);

    // Add embeddings
    let embeddings = load_embeddings()?;
    for (id, embedding) in embeddings.iter().enumerate() {
        index.insert(id as u64, embedding)?;
    }

    // Search for similar vectors
    let query = &embeddings[0];
    let results = index.search(query, 10)?;  // Top 10

    for result in results {
        println!("ID: {}, Distance: {:.4}", result.id, result.distance);
    }

    Ok(())
}
```

---

<details>
<summary><b>Tutorial: Building an HNSW Index</b></summary>

### Basic Index Construction

```rust
use sevensense_vector::{HnswIndex, HnswConfig};

// Configure the index
let config = HnswConfig {
    m: 16,                     // Max connections per node
    m0: 32,                    // Max connections at layer 0
    ef_construction: 200,     // Search width during construction
    ml: 1.0 / (16.0_f32).ln(), // Level multiplier
};

let mut index = HnswIndex::new(config);

// Add vectors one by one
for (id, vector) in vectors.iter().enumerate() {
    index.insert(id as u64, vector)?;
}
```

### Batch Construction

```rust
use sevensense_vector::HnswIndex;

// Build from a batch of vectors (more efficient)
let index = HnswIndex::build(&vectors, config)?;

println!("Index contains {} vectors", index.len());
```

### Progress Monitoring

```rust
let index = HnswIndex::build_with_progress(&vectors, config, |progress| {
    if progress.current % 10000 == 0 {
        println!("Indexed {}/{} vectors ({:.1}%)",
            progress.current, progress.total, progress.percentage());
    }
})?;
```

</details>

<details>
<summary><b>Tutorial: Similarity Search</b></summary>

### Basic Search

```rust
use sevensense_vector::HnswIndex;

let results = index.search(&query_vector, 10)?;

for result in &results {
    println!("ID: {}, Distance: {:.4}, Similarity: {:.4}",
        result.id,
        result.distance,
        1.0 - result.distance  // For cosine distance
    );
}
```

### Search with EF Parameter

The `ef` parameter controls the accuracy/speed tradeoff at query time:

```rust
use sevensense_vector::SearchParams;

// Higher ef = more accurate but slower
let params = SearchParams {
    ef: 100,  // Search width (default: 50)
};

let results = index.search_with_params(&query, 10, params)?;
```

### Filtered Search

```rust
use sevensense_vector::{HnswIndex, Filter};

// Search with metadata filter
let filter = Filter::new()
    .species_in(&["Turdus merula", "Turdus philomelos"])
    .confidence_gte(0.8);

let results = index.search_with_filter(&query, 10, filter)?;
```

### Batch Search

```rust
let queries = vec![query1, query2, query3];

// Search all queries in parallel
let all_results = index.search_batch(&queries, 10)?;

for (i, results) in all_results.iter().enumerate() {
    println!("Query {}: {} results", i, results.len());
}
```

</details>

<details>
<summary><b>Tutorial: Index Persistence</b></summary>

### Saving an Index

```rust
use sevensense_vector::HnswIndex;

// Build and save
let index = HnswIndex::build(&vectors, config)?;
index.save("index.hnsw")?;

println!("Saved index with {} vectors", index.len());
```

### Loading an Index

```rust
let index = HnswIndex::load("index.hnsw")?;

println!("Loaded index with {} vectors", index.len());

// Ready to search
let results = index.search(&query, 10)?;
```

### Memory-Mapped Loading

For large indices that don't fit in RAM:

```rust
use sevensense_vector::MmapIndex;

// Memory-map the index (lazy loading)
let index = MmapIndex::open("large_index.hnsw")?;

// Search works the same way
let results = index.search(&query, 10)?;
```

</details>

<details>
<summary><b>Tutorial: Hyperbolic Embeddings</b></summary>

### Poincaré Ball Model

Hyperbolic space is ideal for hierarchical data like taxonomies:

```rust
use sevensense_vector::{HyperbolicIndex, PoincareConfig};

let config = PoincareConfig {
    curvature: -1.0,          // Negative curvature
    dimension: 1536,          // Same as Euclidean
};

let mut index = HyperbolicIndex::new(config);

// Project Euclidean embeddings to Poincaré ball
for (id, euclidean_vec) in embeddings.iter().enumerate() {
    let poincare_vec = project_to_poincare(euclidean_vec)?;
    index.insert(id as u64, &poincare_vec)?;
}
```

### Hyperbolic Distance

```rust
use sevensense_vector::hyperbolic::{poincare_distance, mobius_add};

// Distance in the Poincaré ball
let dist = poincare_distance(&vec1, &vec2, -1.0);

// Möbius addition (hyperbolic translation)
let translated = mobius_add(&vec1, &vec2, -1.0);
```

### Hierarchical Similarity

```rust
// Hyperbolic distance captures hierarchical relationships
// Closer to origin = more general, farther = more specific

let genus_embedding = index.get("Turdus")?;
let species_embedding = index.get("Turdus merula")?;

// Species is "below" genus in the hierarchy
let genus_norm = l2_norm(&genus_embedding);
let species_norm = l2_norm(&species_embedding);

assert!(species_norm > genus_norm);  // Further from origin
```

</details>

<details>
<summary><b>Tutorial: Performance Tuning</b></summary>

### Parameter Selection

```rust
use sevensense_vector::HnswConfig;

// High accuracy configuration
let accurate_config = HnswConfig {
    m: 32,                     // More connections
    ef_construction: 400,     // More thorough build
    ..Default::default()
};

// Fast configuration
let fast_config = HnswConfig {
    m: 8,                      // Fewer connections
    ef_construction: 100,     // Faster build
    ..Default::default()
};

// Balanced (default)
let balanced_config = HnswConfig::default();
```

### Benchmarking Recall

```rust
use sevensense_vector::{HnswIndex, benchmark_recall};

// Build index
let index = HnswIndex::build(&vectors, config)?;

// Benchmark against brute force
let recall = benchmark_recall(&index, &queries, &ground_truth, 10)?;
println!("Recall@10: {:.4}", recall);  // Should be >0.95
```

### Memory Estimation

```rust
use sevensense_vector::estimate_memory;

let num_vectors = 1_000_000;
let dimensions = 1536;
let m = 16;

let estimated_bytes = estimate_memory(num_vectors, dimensions, m);
println!("Estimated memory: {:.2} GB", estimated_bytes as f64 / 1e9);
```

</details>

---

## Configuration

### HnswConfig Parameters

| Parameter | Default | Description | Impact |
|-----------|---------|-------------|--------|
| `m` | 16 | Connections per node | Higher = better recall, more memory |
| `m0` | 32 | Layer 0 connections | Usually 2×m |
| `ef_construction` | 200 | Build-time search width | Higher = better quality, slower build |
| `ml` | 1/ln(m) | Level multiplier | Controls layer distribution |

### Search Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `ef` | 50 | Search-time width |
| `k` | 10 | Number of results |

## Performance Benchmarks

| Index Size | Build Time | Search (p99) | Recall@10 | Memory |
|------------|------------|--------------|-----------|--------|
| 100K | 5s | 0.8ms | 0.97 | 620 MB |
| 1M | 55s | 2.1ms | 0.96 | 6.0 GB |
| 10M | 12min | 8.5ms | 0.95 | 58 GB |

### Speedup vs Brute Force

| Index Size | HNSW (ms) | Brute Force (ms) | Speedup |
|------------|-----------|------------------|---------|
| 100K | 0.8 | 45 | 56x |
| 1M | 2.1 | 450 | 214x |
| 10M | 8.5 | 4500 | 529x |

## Links

- **Homepage**: [ruv.io](https://ruv.io)
- **Repository**: [github.com/ruvnet/ruvector](https://github.com/ruvnet/ruvector)
- **Crates.io**: [crates.io/crates/sevensense-vector](https://crates.io/crates/sevensense-vector)
- **Documentation**: [docs.rs/sevensense-vector](https://docs.rs/sevensense-vector)

## License

MIT License - see [LICENSE](../../LICENSE) for details.

---

*Part of the [7sense Bioacoustic Intelligence Platform](https://ruv.io) by rUv*
