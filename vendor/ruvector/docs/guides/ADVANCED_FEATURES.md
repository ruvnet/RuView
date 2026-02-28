# Advanced Features Guide

This guide covers advanced features of Ruvector including hybrid search, filtered search, MMR, quantization techniques, and performance optimization.

## Table of Contents

1. [Hybrid Search (Vector + Keyword)](#hybrid-search)
2. [Filtered Search](#filtered-search)
3. [MMR (Maximal Marginal Relevance)](#mmr-maximal-marginal-relevance)
4. [Product Quantization](#product-quantization)
5. [Conformal Prediction](#conformal-prediction)
6. [Performance Optimization](#performance-optimization)
7. [Collection Management](#collection-management)
8. [Additional VectorDB Operations](#additional-vectordb-operations)
9. [Server REST API](#server-rest-api)
10. [Advanced Filter Expressions](#advanced-filter-expressions)
11. [Graph Database](#graph-database)
12. [Metrics & Health Monitoring](#metrics--health-monitoring)
13. [RVF Format Capabilities](#rvf-format-capabilities)
14. [Additional Crates](#additional-crates)

## Hybrid Search

Combine vector similarity with keyword-based BM25 scoring for best of both worlds.

### Rust

```rust
use ruvector_core::{HybridSearch, HybridConfig};

fn hybrid_search_example(db: &VectorDB) -> Result<(), Box<dyn std::error::Error>> {
    let config = HybridConfig {
        vector_weight: 0.7,  // 70% vector similarity
        bm25_weight: 0.3,    // 30% keyword relevance
        k1: 1.5,             // BM25 parameter
        b: 0.75,             // BM25 parameter
    };

    let hybrid = HybridSearch::new(db, config)?;

    // Search with both vector and keywords
    let results = hybrid.search(
        &query_vector,
        &["machine", "learning", "embeddings"],
        10
    )?;

    for result in results {
        println!(
            "ID: {}, Vector Score: {:.4}, BM25 Score: {:.4}, Combined: {:.4}",
            result.id, result.vector_score, result.bm25_score, result.combined_score
        );
    }

    Ok(())
}
```

### Node.js

```javascript
const { HybridSearch } = require('ruvector');

const hybrid = new HybridSearch(db, {
    vectorWeight: 0.7,
    bm25Weight: 0.3,
    k1: 1.5,
    b: 0.75
});

const results = await hybrid.search(
    queryVector,
    ['machine', 'learning', 'embeddings'],
    10
);

results.forEach(result => {
    console.log(`ID: ${result.id}`);
    console.log(`  Vector: ${result.vectorScore.toFixed(4)}`);
    console.log(`  BM25: ${result.bm25Score.toFixed(4)}`);
    console.log(`  Combined: ${result.combinedScore.toFixed(4)}`);
});
```

### Use Cases

- **Document search**: Combine semantic similarity with keyword matching
- **E-commerce**: Vector similarity for visual features + text search for descriptions
- **Q&A systems**: Semantic understanding + exact term matching

## Filtered Search

Apply metadata filters before or after vector search.

### Pre-filtering

Apply filters before graph traversal (efficient for selective filters).

```rust
use ruvector_core::{FilteredSearch, FilterExpression, FilterStrategy};
use serde_json::json;

fn pre_filtering_example(db: &VectorDB) -> Result<(), Box<dyn std::error::Error>> {
    let filter = FilterExpression::And(vec![
        FilterExpression::Eq("category".to_string(), json!("tech")),
        FilterExpression::Gte("timestamp".to_string(), json!(1640000000)),
    ]);

    let filtered = FilteredSearch::new(db, FilterStrategy::PreFilter);

    let results = filtered.search(&query_vector, 10, Some(filter))?;

    Ok(())
}
```

### Post-filtering

Traverse full graph, then apply filters (better for loose constraints).

```rust
let filtered = FilteredSearch::new(db, FilterStrategy::PostFilter);

let filter = FilterExpression::In(
    "tags".to_string(),
    vec![json!("ml"), json!("ai")]
);

let results = filtered.search(&query_vector, 10, Some(filter))?;
```

### Filter Expressions

```rust
// Equality
FilterExpression::Eq("status".into(), json!("active"))

// Comparison
FilterExpression::Gt("score".into(), json!(0.8))
FilterExpression::Gte("timestamp".into(), json!(start_time))
FilterExpression::Lt("price".into(), json!(100))
FilterExpression::Lte("rating".into(), json!(5))

// Set operations
FilterExpression::In("category".into(), vec![json!("a"), json!("b")])
FilterExpression::Nin("id".into(), vec![json!("exclude1"), json!("exclude2")])

// Logical operators
FilterExpression::And(vec![expr1, expr2])
FilterExpression::Or(vec![expr1, expr2])
FilterExpression::Not(Box::new(expr))
```

### Node.js

```javascript
const { FilteredSearch } = require('ruvector');

const filtered = new FilteredSearch(db, 'preFilter');

const results = await filtered.search(queryVector, 10, {
    and: [
        { field: 'category', op: 'eq', value: 'tech' },
        { field: 'timestamp', op: 'gte', value: 1640000000 }
    ]
});
```

## MMR (Maximal Marginal Relevance)

Diversify search results to reduce redundancy.

### Rust

```rust
use ruvector_core::{MMRSearch, MMRConfig};

fn mmr_example(db: &VectorDB) -> Result<(), Box<dyn std::error::Error>> {
    let config = MMRConfig {
        lambda: 0.5,        // Balance relevance (1.0) vs diversity (0.0)
        diversity_weight: 0.3,
    };

    let mmr = MMRSearch::new(db, config)?;

    // Get diverse results
    let results = mmr.search(&query_vector, 20)?;

    println!("Diverse results (λ = 0.5):");
    for (i, result) in results.iter().enumerate() {
        println!("{}. ID: {}, Relevance: {:.4}", i + 1, result.id, result.score);
    }

    Ok(())
}
```

### Lambda Parameter

- **λ = 1.0**: Pure relevance (no diversity)
- **λ = 0.5**: Balanced (recommended)
- **λ = 0.0**: Pure diversity (may sacrifice relevance)

### Node.js

```javascript
const { MMRSearch } = require('ruvector');

const mmr = new MMRSearch(db, {
    lambda: 0.5,
    diversityWeight: 0.3
});

const results = await mmr.search(queryVector, 20);
```

### Use Cases

- **Recommendation systems**: Avoid showing too many similar items
- **Document retrieval**: Diverse perspectives on a topic
- **Search results**: Reduce redundancy in top results

## Product Quantization

Achieve 8-16x memory compression with 90-95% recall.

### Rust

```rust
use ruvector_core::{EnhancedPQ, PQConfig};

fn product_quantization_example() -> Result<(), Box<dyn std::error::Error>> {
    let mut options = DbOptions::default();
    options.dimensions = 128;
    options.quantization = Some(QuantizationConfig::Product {
        subspaces: 16,  // Split into 16 subvectors of 8D each
        k: 256,         // 256 centroids per subspace
    });

    let db = VectorDB::new(options)?;

    // Insert vectors (automatically quantized)
    db.insert_batch(vectors)?;

    // Search uses quantized vectors
    let results = db.search(&query)?;

    Ok(())
}
```

### Configuration

| Subspaces | Dimensions per subspace | Compression | Recall |
|-----------|------------------------|-------------|--------|
| 8 | 16 | 8x | 92-95% |
| 16 | 8 | 16x | 90-94% |
| 32 | 4 | 32x | 85-90% |

### Node.js

```javascript
const db = new VectorDB({
    dimensions: 128,
    quantization: {
        type: 'product',
        subspaces: 16,
        k: 256
    }
});
```

### Performance Impact

```
Without PQ: 1M vectors × 128 dims × 4 bytes = 512 MB
With PQ (16 subspaces): 1M vectors × 16 bytes = 16 MB (32x compression)
+ Codebooks: 16 × 256 × 8 × 4 bytes = 128 KB
Total: ~16.1 MB
```

## Conformal Prediction

Get confidence intervals for predictions.

### Rust

```rust
use ruvector_core::{ConformalPredictor, ConformalConfig, PredictionSet};

fn conformal_prediction_example() -> Result<(), Box<dyn std::error::Error>> {
    let config = ConformalConfig {
        alpha: 0.1,              // 90% confidence
        calibration_size: 1000,  // Calibration set size
    };

    let mut predictor = ConformalPredictor::new(config);

    // Calibrate with known similarities
    let calibration_data: Vec<(Vec<f32>, Vec<f32>, f64)> = get_calibration_data();
    predictor.calibrate(&calibration_data)?;

    // Predict with confidence
    let prediction: PredictionSet = predictor.predict(&query_vector, &db)?;

    println!("Prediction set size: {}", prediction.candidates.len());
    println!("Confidence level: {:.1}%", (1.0 - config.alpha) * 100.0);

    for candidate in prediction.candidates {
        println!(
            "  ID: {}, Distance: {:.4}, Confidence: [{:.4}, {:.4}]",
            candidate.id,
            candidate.distance,
            candidate.confidence_lower,
            candidate.confidence_upper
        );
    }

    Ok(())
}
```

### Node.js

```javascript
const { ConformalPredictor } = require('ruvector');

const predictor = new ConformalPredictor({
    alpha: 0.1,           // 90% confidence
    calibrationSize: 1000
});

// Calibrate
await predictor.calibrate(calibrationData);

// Predict with confidence
const prediction = await predictor.predict(queryVector, db);

console.log(`Prediction set size: ${prediction.candidates.length}`);
prediction.candidates.forEach(c => {
    console.log(`ID: ${c.id}, Distance: ${c.distance.toFixed(4)}`);
    console.log(`  Confidence: [${c.confidenceLower.toFixed(4)}, ${c.confidenceUpper.toFixed(4)}]`);
});
```

### Use Cases

- **Adaptive top-k**: Dynamically adjust number of results based on confidence
- **Query routing**: Route uncertain queries to expensive rerankers
- **Trust scores**: Provide confidence metrics to users

## Performance Optimization

### 1. SIMD Optimization

```bash
# Enable all SIMD instructions for your CPU
RUSTFLAGS="-C target-cpu=native" cargo build --release

# Specific features
RUSTFLAGS="-C target-feature=+avx2,+fma" cargo build --release

# Verify SIMD is enabled
cargo build --release -vv | grep target-cpu
```

### 2. Memory-Mapped Vectors

```rust
let mut options = DbOptions::default();
// options.mmap_vectors = true;  // Enable memory mapping (if supported by storage backend)

let db = VectorDB::new(options)?;
```

Benefits:
- Instant loading (no deserialization)
- Datasets larger than RAM
- OS-managed caching

### 3. Batch Operations

```rust
// ❌ Slow: Individual inserts
for entry in entries {
    db.insert(entry)?;  // Many individual operations
}

// ✅ Fast: Batch insert
db.insert_batch(entries)?;  // Single optimized operation
```

Performance: **10-100x faster** for large batches.

### 4. Parallel Search

```rust
use rayon::prelude::*;

let queries: Vec<Vec<f32>> = get_query_vectors();

let results: Vec<Vec<SearchResult>> = queries
    .par_iter()
    .map(|query| {
        db.search(&SearchQuery {
            vector: query.clone(),
            k: 10,
            filter: None,
            include_vectors: false,
        }).unwrap()
    })
    .collect();
```

### 5. HNSW Parameter Tuning

```rust
// For speed (lower recall)
options.hnsw_config.as_mut().unwrap().ef_search = 50;

// For accuracy (slower)
options.hnsw_config.as_mut().unwrap().ef_search = 500;

// Balanced (recommended)
options.hnsw_config.as_mut().unwrap().ef_search = 100;
```

### 6. Quantization

```rust
// 4x compression, 97-99% recall
options.quantization = Some(QuantizationConfig::Scalar);

// 16x compression, 90-95% recall
options.quantization = Some(QuantizationConfig::Product {
    subspaces: 16,
    k: 256,
});
```

### 7. Distance Metric Selection

```rust
// For normalized embeddings (faster)
options.distance_metric = DistanceMetric::DotProduct;

// For unnormalized embeddings
options.distance_metric = DistanceMetric::Cosine;  // Auto-normalizes

// For general similarity
options.distance_metric = DistanceMetric::Euclidean;
```

### Performance Comparison

| Configuration | Memory | Latency | Recall |
|---------------|--------|---------|--------|
| Full precision, ef=50 | 100% | 0.5ms | 85% |
| Full precision, ef=100 | 100% | 1.0ms | 95% |
| Full precision, ef=500 | 100% | 5.0ms | 99% |
| Scalar quant, ef=100 | 25% | 0.8ms | 94% |
| Product quant, ef=100 | 6% | 1.2ms | 92% |

## Complete Advanced Example

```rust
use ruvector_core::*;

fn advanced_demo() -> Result<(), Box<dyn std::error::Error>> {
    // Create high-performance database
    let mut options = DbOptions::default();
    options.dimensions = 384;
    options.storage_path = "./advanced_db.db".to_string();
    options.hnsw_config = Some(HnswConfig {
        m: 64,
        ef_construction: 400,
        ef_search: 200,
        max_elements: 10_000_000,
    });
    options.distance_metric = DistanceMetric::Cosine;
    options.quantization = Some(QuantizationConfig::Product {
        subspaces: 16,
        k: 256,
    });

    let db = VectorDB::new(options)?;

    // Hybrid search with filtering
    let hybrid_config = HybridConfig {
        vector_weight: 0.7,
        bm25_weight: 0.3,
        k1: 1.5,
        b: 0.75,
    };
    let hybrid = HybridSearch::new(&db, hybrid_config)?;

    let filter = FilterExpression::And(vec![
        FilterExpression::Eq("category".into(), json!("research")),
        FilterExpression::Gte("year".into(), json!(2020)),
    ]);

    // Search with all features
    let results = hybrid.search_filtered(
        &query_vector,
        &["neural", "networks"],
        20,
        Some(filter)
    )?;

    // Apply MMR for diversity
    let mmr_config = MMRConfig {
        lambda: 0.6,
        diversity_weight: 0.4,
    };
    let diverse_results = MMRSearch::rerank(&results, mmr_config)?;

    // Conformal prediction for confidence
    let mut predictor = ConformalPredictor::new(ConformalConfig {
        alpha: 0.1,
        calibration_size: 1000,
    });
    predictor.calibrate(&calibration_data)?;
    let prediction = predictor.predict_batch(&diverse_results)?;

    // Display results with confidence
    for (i, result) in prediction.candidates.iter().enumerate() {
        println!("{}. ID: {} (confidence: {:.1}%)",
            i + 1,
            result.id,
            result.mean_confidence * 100.0
        );
    }

    Ok(())
}
```

## Collection Management

Organize vectors into named collections with alias support.

### Rust

```rust
use ruvector_collections::CollectionManager;

let manager = CollectionManager::new("./data")?;

// Create collections
manager.create_collection("products", CollectionConfig {
    dimensions: 384,
    distance_metric: "cosine".into(),
    ..Default::default()
})?;

// List, delete, stats
let names = manager.list_collections()?;
let stats = manager.collection_stats("products")?;
println!("Vectors: {}, Size: {} bytes", stats.vectors_count, stats.disk_size_bytes);

// Aliases — point multiple names at one collection
manager.create_alias("shop", "products")?;
manager.list_aliases()?;  // [("shop", "products")]
manager.delete_alias("shop")?;
```

### Node.js

```javascript
const { CollectionManager } = require('ruvector');

const mgr = new CollectionManager('./data');

mgr.createCollection('products', { dimensions: 384, distanceMetric: 'Cosine' });
const collections = mgr.listCollections();
const stats = mgr.getStats('products');
console.log(`Vectors: ${stats.vectorsCount}, RAM: ${stats.ramSizeBytes}`);

mgr.createAlias('shop', 'products');
mgr.listAliases();
mgr.deleteAlias('shop');
```

## Additional VectorDB Operations

Beyond `insert`, `search`, and `insert_batch`, the Node.js bindings expose:

```javascript
const { VectorDB } = require('ruvector');
const db = new VectorDB({ dimensions: 128, storagePath: './db' });

// Retrieve a single vector
const entry = db.get('vec_0001');

// Delete a vector
db.delete('vec_0001');

// Count vectors
const count = db.len();
const empty = db.isEmpty();
```

## Server REST API

`ruvector-server` exposes an Axum-based HTTP API for remote access.

### Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/collections` | Create a collection |
| `GET` | `/collections` | List all collections |
| `GET` | `/collections/:name` | Get collection info |
| `DELETE` | `/collections/:name` | Delete a collection |
| `PUT` | `/collections/:name/points` | Upsert vectors |
| `POST` | `/collections/:name/points/search` | Search (with optional `score_threshold`) |
| `GET` | `/collections/:name/points/:id` | Retrieve a point by ID |
| `GET` | `/health` | Health check |
| `GET` | `/ready` | Readiness probe |

### Example — cURL

```bash
# Create a collection
curl -X POST http://localhost:8080/collections \
  -H 'Content-Type: application/json' \
  -d '{"name": "docs", "dimensions": 384, "distanceMetric": "cosine"}'

# Upsert vectors
curl -X PUT http://localhost:8080/collections/docs/points \
  -H 'Content-Type: application/json' \
  -d '{"points": [{"id": "doc_1", "vector": [0.1, ...], "metadata": {"title": "Hello"}}]}'

# Search
curl -X POST http://localhost:8080/collections/docs/points/search \
  -H 'Content-Type: application/json' \
  -d '{"vector": [0.1, ...], "k": 10, "scoreThreshold": 0.8}'
```

## Advanced Filter Expressions

`ruvector-filter` supports rich filter expressions beyond simple equality.

### Available Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `eq` | Equality | `FilterExpression::eq("status", "active")` |
| `ne` | Not equal | `FilterExpression::ne("status", "deleted")` |
| `gt`, `gte` | Greater than (or equal) | `FilterExpression::gte("score", 0.8)` |
| `lt`, `lte` | Less than (or equal) | `FilterExpression::lt("price", 100)` |
| `in_values` | Set membership | `FilterExpression::in_values("tag", vec!["a","b"])` |
| `match_text` | Text search | `FilterExpression::match_text("content", "rust")` |
| `geo_radius` | Geospatial radius | `FilterExpression::geo_radius("location", 37.7, -122.4, 5000.0)` |
| `and`, `or` | Boolean combinators | `FilterExpression::and(vec![f1, f2])` |
| `not` | Negation | `FilterExpression::not(expr)` |

### Payload Indexing

Create field indices for fast filtered search:

```rust
use ruvector_filter::{PayloadIndexManager, IndexType};

let mut index_mgr = PayloadIndexManager::new();
index_mgr.create_index("category", IndexType::Keyword)?;
index_mgr.create_index("price", IndexType::Float)?;
index_mgr.create_index("location", IndexType::Geo)?;
index_mgr.create_index("description", IndexType::Text)?;

// Index a payload
index_mgr.index_payload("doc_1", &json!({
    "category": "electronics",
    "price": 299.99,
    "location": {"lat": 37.7749, "lon": -122.4194},
    "description": "High-performance vector database"
}))?;
```

### Node.js

```javascript
const results = await db.search({
    vector: queryVec,
    k: 10,
    filter: {
        and: [
            { field: 'category', op: 'eq', value: 'electronics' },
            { field: 'price', op: 'lte', value: 500 }
        ]
    }
});
```

## Graph Database

`ruvector-graph` provides a full property graph database with Cypher query support.

### CLI

```bash
# Create a graph database
ruvector graph create --db ./graph.db --dimensions 128

# Run Cypher queries
ruvector graph query --db ./graph.db \
  --query "CREATE (n:Person {name: 'Alice', age: 30})"

ruvector graph query --db ./graph.db \
  --query "MATCH (n:Person) WHERE n.age > 25 RETURN n.name, n.age"

# Interactive shell
ruvector graph shell --db ./graph.db

# Start graph server
ruvector graph serve --db ./graph.db --port 8081
```

### Rust API

```rust
use ruvector_graph::{GraphDB, NodeBuilder, EdgeBuilder};

let db = GraphDB::new("./graph.db")?;

// Create nodes
let alice = NodeBuilder::new("Person")
    .property("name", "Alice")
    .property("age", 30)
    .build();
let bob = NodeBuilder::new("Person")
    .property("name", "Bob")
    .build();

let alice_id = db.insert_node(alice)?;
let bob_id = db.insert_node(bob)?;

// Create edges
let edge = EdgeBuilder::new("KNOWS", alice_id, bob_id)
    .property("since", 2024)
    .build();
db.insert_edge(edge)?;

// Cypher queries
let results = db.execute_cypher("MATCH (a:Person)-[:KNOWS]->(b) RETURN a.name, b.name")?;
```

### Hybrid Vector + Graph

```rust
use ruvector_graph::HybridIndex;

// Combine vector similarity with graph traversal
let hybrid = HybridIndex::new(&graph_db, &vector_db)?;
let results = hybrid.semantic_search(query_vector, 10)?;
```

## Metrics & Health Monitoring

`ruvector-metrics` provides Prometheus-compatible metrics.

### Exposed Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `ruvector_search_requests_total` | Counter | Searches by collection/status |
| `ruvector_search_latency_seconds` | Histogram | Search latency |
| `ruvector_insert_requests_total` | Counter | Inserts by collection |
| `ruvector_insert_latency_seconds` | Histogram | Insert latency |
| `ruvector_vectors_total` | Gauge | Total vectors per collection |
| `ruvector_collections_total` | Gauge | Number of collections |
| `ruvector_memory_usage_bytes` | Gauge | Memory utilization |
| `ruvector_uptime_seconds` | Gauge | Server uptime |

### Node.js

```javascript
const metrics = db.getMetrics();   // Prometheus text format
const health = db.getHealth();     // { status, uptime, ... }
```

## RVF Format Capabilities

The RVF binary format (via `rvf-runtime`) provides capabilities beyond basic vector storage. See [`examples/rvf/`](../../examples/rvf/) for complete working examples.

### Key Capabilities

| Feature | API | Description |
|---------|-----|-------------|
| Quality envelopes | `query_with_envelope()` | Response quality, HNSW vs safety-net stats, budget tracking |
| Audited queries | `query_audited()` | Auto-appends witness entry per search for compliance |
| Membership filters | `MembershipFilter` | Bitmap-based tenant isolation (include/exclude modes) |
| DoS hardening | `BudgetTokenBucket`, `NegativeCache`, `ProofOfWork` | Three-layer defense |
| Adversarial detection | `is_degenerate_distribution()`, `centroid_distance_cv()` | Detects uniform attack vectors |
| WASM embedding | `embed_wasm()` / `extract_wasm()` | Self-bootstrapping query engine |
| Kernel embedding | `embed_kernel()` / `extract_kernel()` | Linux image with cmdline |
| eBPF embedding | `embed_ebpf()` / `extract_ebpf()` | Socket filter programs |
| Dashboard embedding | `embed_dashboard()` / `extract_dashboard()` | HTML/JS bundles |
| Delete + compact | `delete()` + `compact()` | Soft-delete with space reclamation |
| Lineage derivation | `derive()` | Parent-child snapshots with depth tracking |
| COW branching | `freeze()` + `branch()` | Copy-on-write staging environments |
| AGI containers | `AgiContainerBuilder` | Self-describing agent manifests |
| Witness chains | `create_witness_chain()` | Cryptographic audit trails (SHAKE256) |
| Segment directory | `segment_dir()` | Enumerate all segments in an RVF file |

## Additional Crates

RuVector includes 80+ crates. Key specialized crates include:

| Crate | npm Package | Description |
|-------|------------|-------------|
| `ruvector-gnn` | `@ruvector/gnn` | GNN training with EWC forgetting mitigation, LoRA, curriculum learning |
| `ruvector-attention` | `@ruvector/attention` | 50+ attention mechanisms (flash, sparse, hyperbolic, sheaf, MoE) |
| `ruvector-mincut` | `@ruvector/mincut` | Subpolynomial-time dynamic graph partitioning |
| `ruvector-solver` | `@ruvector/solver` | Sparse linear solvers (Neumann, CG, forward/backward push) |
| `ruvector-graph` | `@ruvector/graph-node` | Property graph DB with Cypher, hybrid vector+graph |
| `ruvector-graph-transformer` | `@ruvector/graph-transformer` | Transformer-based graph encoding |
| `ruvllm` | — | Full LLM serving with paged attention, speculative decoding, LoRA |
| `sona` | — | Self-Optimizing Neural Architecture (continual learning) |
| `rvlite` | — | Lightweight vector DB for edge devices |
| `ruvector-verified` | — | Cryptographic proof system for vector correctness |
| `ruvector-postgres` | — | PostgreSQL extension for vector operations |

See the [API reference](../api/) and individual crate READMEs for detailed documentation.

## Best Practices

1. **Start simple**: Begin with default settings, optimize later
2. **Measure first**: Profile before optimizing
3. **Batch operations**: Always use batch methods for bulk operations
4. **Choose quantization wisely**: Scalar for general use, product for extreme scale
5. **Tune HNSW gradually**: Increase parameters only if needed
6. **Use appropriate metrics**: Cosine for normalized, Euclidean otherwise
7. **Enable SIMD**: Always compile with target-cpu=native
8. **Use collections**: Organize vectors by domain for better management
9. **Monitor with metrics**: Enable Prometheus metrics in production
10. **Use RVF for self-contained files**: When you need portability with embedded segments

## Next Steps

- [AgenticDB Quickstart](AGENTICDB_QUICKSTART.md) - AI agent memory features
- [Performance Tuning](../optimization/PERFORMANCE_TUNING_GUIDE.md) - Detailed optimization
- [API Reference](../api/) - Complete API documentation
- [Cypher Reference](../api/CYPHER_REFERENCE.md) - Graph query language
- [RVF Examples](../../examples/rvf/) - Working RVF format examples
- [Examples](../../examples/) - All working code examples
