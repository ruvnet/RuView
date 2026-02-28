# Ruvector Graph

[![Crates.io](https://img.shields.io/crates/v/ruvector-graph.svg)](https://crates.io/crates/ruvector-graph)
[![Documentation](https://docs.rs/ruvector-graph/badge.svg)](https://docs.rs/ruvector-graph)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.77%2B-orange.svg)](https://www.rust-lang.org)

**A graph database with Cypher queries, hyperedges, and vector search -- all in one crate.**

```toml
[dependencies]
ruvector-graph = "0.1.1"
```

Most graph databases make you choose: you can have relationships *or* vector search, a query language *or* raw traversals, pairwise edges *or* nothing. `ruvector-graph` gives you all of them together. Write familiar Cypher queries like Neo4j, attach vector embeddings to any node for semantic search, and model complex group relationships with hyperedges that connect three or more nodes at once. It runs on servers, in browsers via WASM, and across clusters with built-in RAFT consensus. Part of the [RuVector](https://github.com/ruvnet/ruvector) ecosystem.

| | ruvector-graph | Neo4j / Typical Graph DB | Vector DB + Custom Glue |
|---|---|---|---|
| **Query language** | Full Cypher parser built-in | Cypher (Neo4j) or proprietary | No graph queries |
| **Hyperedges** | Native -- one edge connects N nodes | Pairwise only -- workarounds needed | Not applicable |
| **Vector search** | HNSW on every node, semantic similarity | Separate plugin or not available | Vectors only, no graph structure |
| **SIMD acceleration** | SimSIMD hardware-optimized ops | JVM-based | Varies |
| **Browser / WASM** | `default-features = false, features = ["wasm"]` | Server only | Server only |
| **Distributed** | Built-in RAFT consensus + federation | Enterprise tier (paid) | Varies |
| **Cost** | Free, open source (MIT) | Community or paid license | Varies |

## Key Features

| Feature | What It Does | Why It Matters |
|---------|-------------|----------------|
| **Cypher Engine** | Parse and execute Cypher queries -- `MATCH (a)-[:KNOWS]->(b)` | Use a query language you already know instead of raw traversal code |
| **Hypergraph Model** | Edges connect any number of nodes, not just pairs | Model meetings, co-authorships, reactions -- any group relationship -- natively |
| **Vector Embeddings** | Attach embeddings to nodes, run HNSW similarity search | Combine "who is connected to whom" with "what is semantically similar" |
| **Property Graph** | Rich JSON properties on every node and edge | Store real data on your graph elements, not just IDs |
| **Label Indexes** | Roaring bitmap indexes for fast label lookups | Filter millions of nodes by label in microseconds |
| **SIMD Optimized** | Hardware-accelerated distance calculations via SimSIMD | Faster vector operations without changing your code |
| **Distributed Mode** | RAFT consensus for multi-node deployments | Scale out without bolting on a separate coordination layer |
| **Federation** | Cross-cluster graph queries | Query across data centers as if they were one graph |
| **Compression** | ZSTD and LZ4 for storage | Smaller on disk without sacrificing read speed |
| **WASM Compatible** | Run in browsers with WebAssembly | Same graph engine on server and client |

## Installation

```toml
[dependencies]
ruvector-graph = "0.1.1"
```

### Feature Flags

```toml
[dependencies]
# Full feature set
ruvector-graph = { version = "0.1.1", features = ["full"] }

# Minimal WASM-compatible build
ruvector-graph = { version = "0.1.1", default-features = false, features = ["wasm"] }

# Distributed deployment
ruvector-graph = { version = "0.1.1", features = ["distributed"] }
```

Available features:
- `full` (default): Complete feature set with all optimizations
- `simd`: SIMD-optimized operations
- `storage`: Persistent storage with redb
- `async-runtime`: Tokio async support
- `compression`: ZSTD/LZ4 compression
- `distributed`: RAFT consensus support
- `federation`: Cross-cluster federation
- `wasm`: WebAssembly-compatible minimal build
- `metrics`: Prometheus monitoring

## Quick Start

### Create a Graph

```rust
use ruvector_graph::{Graph, Node, Edge, GraphConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a new graph
    let config = GraphConfig::default();
    let graph = Graph::new(config)?;

    // Create nodes
    let alice = graph.create_node(Node {
        labels: vec!["Person".to_string()],
        properties: serde_json::json!({
            "name": "Alice",
            "age": 30
        }),
        ..Default::default()
    })?;

    let bob = graph.create_node(Node {
        labels: vec!["Person".to_string()],
        properties: serde_json::json!({
            "name": "Bob",
            "age": 25
        }),
        ..Default::default()
    })?;

    // Create relationship
    graph.create_edge(Edge {
        label: "KNOWS".to_string(),
        source: alice.id,
        target: bob.id,
        properties: serde_json::json!({
            "since": 2020
        }),
        ..Default::default()
    })?;

    Ok(())
}
```

### Cypher Queries

```rust
use ruvector_graph::{Graph, CypherExecutor};

// Execute Cypher query
let executor = CypherExecutor::new(&graph);
let results = executor.execute("
    MATCH (p:Person)-[:KNOWS]->(friend:Person)
    WHERE p.name = 'Alice'
    RETURN friend.name AS name, friend.age AS age
")?;

for row in results {
    println!("Friend: {} (age {})", row["name"], row["age"]);
}
```

### Vector-Enhanced Graph

```rust
use ruvector_graph::{Graph, VectorConfig};

// Enable vector embeddings on nodes
let config = GraphConfig {
    vector_config: Some(VectorConfig {
        dimensions: 384,
        distance_metric: DistanceMetric::Cosine,
        ..Default::default()
    }),
    ..Default::default()
};

let graph = Graph::new(config)?;

// Create node with embedding
let node = graph.create_node(Node {
    labels: vec!["Document".to_string()],
    properties: serde_json::json!({"title": "Introduction to Graphs"}),
    embedding: Some(vec![0.1, 0.2, 0.3, /* ... 384 dims */]),
    ..Default::default()
})?;

// Semantic similarity search
let similar = graph.search_similar_nodes(
    vec![0.1, 0.2, 0.3, /* query vector */],
    10,  // top-k
    Some(vec!["Document".to_string()]),  // filter by labels
)?;
```

### Hyperedges

```rust
use ruvector_graph::{Graph, Hyperedge};

// Create a hyperedge connecting multiple nodes
let meeting = graph.create_hyperedge(Hyperedge {
    label: "PARTICIPATED_IN".to_string(),
    nodes: vec![alice.id, bob.id, charlie.id],
    properties: serde_json::json!({
        "event": "Team Meeting",
        "date": "2024-01-15"
    }),
    ..Default::default()
})?;
```

## API Overview

### Core Types

```rust
// Node in the graph
pub struct Node {
    pub id: NodeId,
    pub labels: Vec<String>,
    pub properties: serde_json::Value,
    pub embedding: Option<Vec<f32>>,
}

// Edge connecting two nodes
pub struct Edge {
    pub id: EdgeId,
    pub label: String,
    pub source: NodeId,
    pub target: NodeId,
    pub properties: serde_json::Value,
}

// Hyperedge connecting multiple nodes
pub struct Hyperedge {
    pub id: HyperedgeId,
    pub label: String,
    pub nodes: Vec<NodeId>,
    pub properties: serde_json::Value,
}
```

### Graph Operations

```rust
impl Graph {
    // Node operations
    pub fn create_node(&self, node: Node) -> Result<Node>;
    pub fn get_node(&self, id: &NodeId) -> Result<Option<Node>>;
    pub fn update_node(&self, node: Node) -> Result<Node>;
    pub fn delete_node(&self, id: &NodeId) -> Result<bool>;

    // Edge operations
    pub fn create_edge(&self, edge: Edge) -> Result<Edge>;
    pub fn get_edge(&self, id: &EdgeId) -> Result<Option<Edge>>;
    pub fn delete_edge(&self, id: &EdgeId) -> Result<bool>;

    // Traversal
    pub fn neighbors(&self, id: &NodeId, direction: Direction) -> Result<Vec<Node>>;
    pub fn traverse(&self, start: &NodeId, config: TraversalConfig) -> Result<Vec<Path>>;

    // Vector search
    pub fn search_similar_nodes(&self, query: Vec<f32>, k: usize, labels: Option<Vec<String>>) -> Result<Vec<Node>>;
}
```

## Performance

### Benchmarks (1M Nodes, 10M Edges)

```
Operation               Latency (p50)    Throughput
-----------------------------------------------------
Node lookup             ~0.1ms           100K ops/s
Edge traversal          ~0.5ms           50K ops/s
1-hop neighbors         ~1ms             20K ops/s
Cypher simple query     ~5ms             5K ops/s
Vector similarity       ~2ms             10K ops/s
```

## Related Crates

- **[ruvector-core](../ruvector-core/)** - Core vector database engine
- **[ruvector-graph-node](../ruvector-graph-node/)** - Node.js bindings
- **[ruvector-graph-wasm](../ruvector-graph-wasm/)** - WebAssembly bindings
- **[ruvector-raft](../ruvector-raft/)** - RAFT consensus for distributed mode
- **[ruvector-cluster](../ruvector-cluster/)** - Clustering and sharding

## Documentation

- **[RuVector README](../../README.md)** - Complete project overview
- **[API Documentation](https://docs.rs/ruvector-graph)** - Full API reference
- **[GitHub Repository](https://github.com/ruvnet/ruvector)** - Source code

## License

**MIT License** - see [LICENSE](../../LICENSE) for details.

---

<div align="center">

**Part of [RuVector](https://github.com/ruvnet/ruvector) - Built by [rUv](https://ruv.io)**

[![Star on GitHub](https://img.shields.io/github/stars/ruvnet/ruvector?style=social)](https://github.com/ruvnet/ruvector)

[Documentation](https://docs.rs/ruvector-graph) | [Crates.io](https://crates.io/crates/ruvector-graph) | [GitHub](https://github.com/ruvnet/ruvector)

</div>
