# RuVector Graph Examples

Graph database features including Cypher queries, distributed clustering, and hybrid search.

## Examples

| File | Description |
|------|-------------|
| `basic_graph.rs` | Graph creation and traversal |
| `cypher_queries.rs` | Cypher query language examples |
| `distributed_cluster.rs` | Multi-node graph clustering |
| `hybrid_search.rs` | Combined vector + graph search |

## Quick Start

```bash
cargo run --example basic_graph
cargo run --example cypher_queries
```

## Basic Graph Operations

```rust
use ruvector_graph::{Graph, Node, Edge};

let mut graph = Graph::new();

// Add nodes with embeddings
let n1 = graph.add_node(Node {
    id: "user:1".to_string(),
    embedding: vec![0.1; 128],
    properties: json!({"name": "Alice"}),
});

let n2 = graph.add_node(Node {
    id: "user:2".to_string(),
    embedding: vec![0.2; 128],
    properties: json!({"name": "Bob"}),
});

// Create relationship
graph.add_edge(Edge {
    from: n1,
    to: n2,
    relation: "KNOWS".to_string(),
    weight: 0.95,
});
```

## Cypher Queries

```rust
// Find connected nodes
let query = "MATCH (a:User)-[:KNOWS]->(b:User) RETURN b";
let results = graph.cypher(query)?;

// Pattern matching with vector similarity
let query = "
    MATCH (u:User)
    WHERE vector_similarity(u.embedding, $query_vec) > 0.8
    RETURN u
";
let results = graph.cypher_with_params(query, params)?;
```

## Distributed Clustering

```rust
use ruvector_graph::{DistributedGraph, ClusterConfig};

let config = ClusterConfig {
    nodes: vec!["node1:9000", "node2:9000"],
    replication_factor: 2,
    partitioning: Partitioning::Hash,
};

let cluster = DistributedGraph::connect(config)?;

// Data is automatically partitioned
cluster.add_node(node)?;

// Queries are distributed
let results = cluster.query("MATCH (n) RETURN n LIMIT 10")?;
```

## Hybrid Search

Combine vector similarity with graph traversal:

```rust
use ruvector_graph::HybridSearch;

let search = HybridSearch::new(graph, vector_index);

// Step 1: Find similar nodes by embedding
// Step 2: Expand via graph relationships
// Step 3: Re-rank by combined score
let results = search.query(HybridQuery {
    embedding: query_vec,
    relation_filter: vec!["KNOWS", "WORKS_WITH"],
    depth: 2,
    top_k: 10,
    vector_weight: 0.6,
    graph_weight: 0.4,
})?;
```

## Graph Algorithms

```rust
// PageRank
let scores = graph.pagerank(0.85, 100)?;

// Community detection (Louvain)
let communities = graph.detect_communities()?;

// Shortest path
let path = graph.shortest_path(from, to)?;

// Connected components
let components = graph.connected_components()?;
```

## Use Cases

| Use Case | Query Pattern |
|----------|---------------|
| Social Networks | `(user)-[:FOLLOWS]->(user)` |
| Knowledge Graphs | `(entity)-[:RELATED_TO]->(entity)` |
| Recommendations | Vector similarity + collaborative filtering |
| Fraud Detection | Subgraph pattern matching |
| Supply Chain | Path analysis and bottleneck detection |

## Performance

- **Index Types**: B-tree, hash, vector (HNSW)
- **Caching**: LRU cache for hot subgraphs
- **Partitioning**: Hash, range, or custom
- **Replication**: Configurable factor

## Related

- [Graph CLI Usage](../docs/graph-cli-usage.md)
- [Graph WASM Usage](../docs/graph_wasm_usage.html)
