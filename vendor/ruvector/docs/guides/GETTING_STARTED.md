# Getting Started with RuVector

## What is RuVector?

RuVector is a high-performance, Rust-native vector database and file format designed for modern AI applications. It provides:

- **10-100x performance improvements** over Python/TypeScript implementations
- **Sub-millisecond latency** with HNSW indexing and SIMD optimization
- **Multi-platform deployment** (Rust, Node.js, WASM/Browser, CLI)
- **RVF (RuVector Format)** — a self-contained binary format with embedded WASM, kernel, eBPF, and dashboard segments
- **Advanced features** including quantization, filtered search, witness chains, COW branching, and AGI container manifests

## Packages

| Package | Registry | Version | Description |
|---------|----------|---------|-------------|
| `ruvector-core` | crates.io | 2.0.x | Core Rust library (VectorDB, HNSW, quantization) |
| `ruvector` | npm | 0.1.x | Node.js native bindings via NAPI-RS |
| `@ruvector/rvf` | npm | 0.2.x | RVF format library (TypeScript) |
| `@ruvector/rvf-node` | npm | 0.1.x | RVF Node.js native bindings |
| `@ruvector/gnn` | npm | 0.1.x | Graph Neural Network bindings |
| `@ruvector/graph-node` | npm | 2.0.x | Graph database with Cypher queries |
| `ruvector-wasm` / `@ruvector/wasm` | npm | — | Browser WASM build |

## Quick Start

### Installation

#### Rust (ruvector-core)
```toml
# Cargo.toml
[dependencies]
ruvector-core = "2.0"
```

#### Rust (RVF format — separate workspace)
```toml
# Cargo.toml — RVF crates live in examples/rvf or crates/rvf
[dependencies]
rvf-runtime = "0.2"
rvf-crypto = "0.2"
```

#### Node.js
```bash
npm install ruvector
# or for the RVF format:
npm install @ruvector/rvf-node
```

#### CLI
```bash
# Build from source
git clone https://github.com/ruvnet/ruvector.git
cd ruvector
cargo install --path crates/ruvector-cli
```

### Basic Usage — ruvector-core (VectorDB)

#### Rust
```rust
use ruvector_core::{VectorDB, VectorEntry, SearchQuery, DbOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = DbOptions {
        dimensions: 128,
        storage_path: "./vectors.db".to_string(),
        ..Default::default()
    };

    let db = VectorDB::new(options)?;

    // Insert a vector
    let entry = VectorEntry {
        id: None,
        vector: vec![0.1; 128],
        metadata: None,
    };
    let id = db.insert(entry)?;
    println!("Inserted vector: {}", id);

    // Search for similar vectors
    let query = SearchQuery {
        vector: vec![0.1; 128],
        k: 10,
        filter: None,
        ef_search: None,
    };
    let results = db.search(query)?;

    for (i, result) in results.iter().enumerate() {
        println!("{}. ID: {}, Score: {:.4}", i + 1, result.id, result.score);
    }

    Ok(())
}
```

#### Node.js
```javascript
const { VectorDB } = require('ruvector');

async function main() {
    const db = new VectorDB({
        dimensions: 128,
        storagePath: './vectors.db',
        distanceMetric: 'Cosine'
    });

    const id = await db.insert({
        vector: new Float32Array(128).fill(0.1),
        metadata: { text: 'Example document' }
    });
    console.log('Inserted vector:', id);

    const results = await db.search({
        vector: new Float32Array(128).fill(0.1),
        k: 10
    });

    results.forEach((result, i) => {
        console.log(`${i + 1}. ID: ${result.id}, Score: ${result.score.toFixed(4)}`);
    });
}

main().catch(console.error);
```

### Basic Usage — RVF Format (RvfStore)

The RVF format is a newer, self-contained binary format used in the `rvf-runtime` crate. See [`examples/rvf/`](../../examples/rvf/) for working examples.

```rust
use rvf_runtime::{RvfStore, RvfOptions, QueryOptions, MetadataEntry, MetadataValue};
use rvf_runtime::options::DistanceMetric;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opts = RvfOptions {
        dimension: 128,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let mut store = RvfStore::create("data.rvf", opts)?;

    // Ingest vectors with metadata
    let vectors = vec![vec![0.1f32; 128]];
    let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids = vec![0u64];
    let meta = vec![
        MetadataEntry { field_id: 0, value: MetadataValue::String("doc".into()) },
    ];
    store.ingest_batch(&refs, &ids, Some(&meta))?;

    // Query
    let query = vec![0.1f32; 128];
    let results = store.query(&query, 5, &QueryOptions::default())?;
    for r in &results {
        println!("id={}, distance={:.4}", r.id, r.distance);
    }

    Ok(())
}
```

#### CLI
```bash
# Create a database
ruvector create --path ./vectors.db --dimensions 128

# Insert vectors from a JSON file
ruvector insert --db ./vectors.db --input vectors.json --format json

# Search for similar vectors
ruvector search --db ./vectors.db --query "[0.1, 0.2, ...]" --top-k 10

# Show database info
ruvector info --db ./vectors.db

# Graph operations
ruvector graph create --db ./graph.db --dimensions 128
ruvector graph query --db ./graph.db --query "MATCH (n) RETURN n LIMIT 10"
```

## Two API Surfaces

RuVector has two main API surfaces:

| | **ruvector-core (VectorDB)** | **rvf-runtime (RvfStore)** |
|---|---|---|
| **Use case** | General-purpose vector DB | Self-contained binary format |
| **Storage** | Directory-based | Single `.rvf` file |
| **IDs** | String-based | u64-based |
| **Metadata** | JSON HashMap | Typed fields (String, U64) |
| **Extras** | Collections, metrics, health | Witness chains, WASM/kernel/eBPF embedding, COW branching, AGI containers |
| **Node.js** | `ruvector` npm package | `@ruvector/rvf-node` npm package |

## Core Concepts

### 1. Vector Database

A vector database stores high-dimensional vectors (embeddings) and enables fast similarity search. Common use cases:
- **Semantic search**: Find similar documents, images, or audio
- **Recommendation systems**: Find similar products or content
- **RAG (Retrieval Augmented Generation)**: Retrieve relevant context for LLMs
- **Agent memory**: Store and retrieve experiences for AI agents

### 2. Distance Metrics

RuVector supports multiple distance metrics:
- **Euclidean (L2)**: Standard distance in Euclidean space
- **Cosine**: Measures angle between vectors (normalized dot product)
- **Dot Product**: Inner product (useful for pre-normalized vectors)
- **Manhattan (L1)**: Sum of absolute differences (ruvector-core only)

### 3. HNSW Indexing

Hierarchical Navigable Small World (HNSW) provides:
- **O(log n) search complexity**
- **95%+ recall** with proper tuning
- **Sub-millisecond latency** for millions of vectors

Key parameters:
- `m`: Connections per node (16-64, default 32)
- `ef_construction`: Build quality (100-400, default 200)
- `ef_search`: Search quality (50-500, default 100)

### 4. Quantization

Reduce memory usage with quantization (ruvector-core):
- **Scalar (int8)**: 4x compression, 97-99% recall
- **Product**: 8-16x compression, 90-95% recall
- **Binary**: 32x compression, 80-90% recall (filtering)

### 5. RVF Format Features

The RVF binary format supports:
- **Witness chains**: Cryptographic audit trails (SHAKE256)
- **Segment embedding**: WASM, kernel, eBPF, and dashboard segments in one file
- **COW branching**: Copy-on-write branches for staging environments
- **Lineage tracking**: Parent-child derivation with depth tracking
- **Membership filters**: Bitmap-based tenant isolation
- **DoS hardening**: Token buckets, negative caches, proof-of-work
- **AGI containers**: Self-describing agent manifests

## Next Steps

- [Installation Guide](INSTALLATION.md) - Detailed installation instructions
- [Basic Tutorial](BASIC_TUTORIAL.md) - Step-by-step tutorial with ruvector-core
- [Advanced Features](ADVANCED_FEATURES.md) - Hybrid search, quantization, filtering
- [RVF Examples](../../examples/rvf/) - Working RVF format examples (openfang, security_hardened, etc.)
- [API Reference](../api/) - Complete API documentation
- [Examples](../../examples/) - All working code examples

## Performance Tips

1. **Choose the right distance metric**: Cosine for normalized embeddings, Euclidean otherwise
2. **Tune HNSW parameters**: Higher `m` and `ef_construction` for better recall
3. **Enable quantization**: Reduces memory 4-32x with minimal accuracy loss
4. **Batch operations**: Use `insert_batch()` / `ingest_batch()` for better throughput
5. **Build with SIMD**: `RUSTFLAGS="-C target-cpu=native" cargo build --release`

## Common Issues

### Out of Memory
- Enable quantization to reduce memory usage
- Reduce `max_elements` or increase available RAM

### Slow Search
- Lower `ef_search` for faster (but less accurate) search
- Enable quantization for cache-friendly operations
- Check if SIMD is enabled (`RUSTFLAGS="-C target-cpu=native"`)

### Low Recall
- Increase `ef_construction` during index building
- Increase `ef_search` during queries
- Use full-precision vectors instead of quantization

## Community & Support

- **GitHub**: [https://github.com/ruvnet/ruvector](https://github.com/ruvnet/ruvector)
- **Issues**: [https://github.com/ruvnet/ruvector/issues](https://github.com/ruvnet/ruvector/issues)

## License

RuVector is licensed under the MIT License. See [LICENSE](../../LICENSE) for details.
