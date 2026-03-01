# @ruvector/graph-node

Native Node.js bindings for RuVector Graph Database with hypergraph support, Cypher queries, and persistence. **10x faster than WASM**.

## Features

- **Native Performance**: Direct NAPI-RS bindings - no WASM overhead
- **Hypergraph Support**: Multi-node relationships with vector embeddings
- **Cypher Queries**: Neo4j-compatible query language
- **Persistence**: ACID-compliant storage with redb backend
- **Vector Similarity Search**: Fast k-NN search on embeddings
- **Graph Traversal**: k-hop neighbor discovery
- **Transactions**: Full ACID support with begin/commit/rollback
- **Batch Operations**: High-throughput bulk inserts (131K+ ops/sec)
- **Zero-Copy**: Efficient Float32Array handling
- **TypeScript**: Full type definitions included

## Installation

```bash
npm install @ruvector/graph-node
```

## Quick Start

```javascript
const { GraphDatabase } = require('@ruvector/graph-node');

// Create an in-memory database
const db = new GraphDatabase({
  distanceMetric: 'Cosine',
  dimensions: 384
});

// Or create a persistent database
const persistentDb = new GraphDatabase({
  distanceMetric: 'Cosine',
  dimensions: 384,
  storagePath: './my-graph.db'
});

// Or open an existing database
const existingDb = GraphDatabase.open('./my-graph.db');

// Create nodes
await db.createNode({
  id: 'alice',
  embedding: new Float32Array([1.0, 0.0, 0.0, /* ... */]),
  labels: ['Person', 'Employee'],
  properties: { name: 'Alice', age: '30' }
});

// Create edges
await db.createEdge({
  from: 'alice',
  to: 'bob',
  description: 'KNOWS',
  embedding: new Float32Array([0.5, 0.5, 0.0, /* ... */]),
  confidence: 0.95
});

// Create hyperedges (multi-node relationships)
await db.createHyperedge({
  nodes: ['alice', 'bob', 'charlie'],
  description: 'COLLABORATED_ON_PROJECT',
  embedding: new Float32Array([0.33, 0.33, 0.33, /* ... */]),
  confidence: 0.85
});

// Query with Cypher
const results = await db.query('MATCH (n:Person) RETURN n');

// Vector similarity search
const similar = await db.searchHyperedges({
  embedding: new Float32Array([0.3, 0.3, 0.3, /* ... */]),
  k: 10
});

// Get statistics
const stats = await db.stats();
console.log(\`Nodes: \${stats.totalNodes}, Edges: \${stats.totalEdges}\`);
```

## Benchmarks

| Operation | Throughput | Latency |
|-----------|------------|---------|
| Node Creation | 9.17K ops/sec | 109ms |
| Batch Node Creation | 131.10K ops/sec | 7.63ms |
| Edge Creation | 9.30K ops/sec | 107ms |
| Vector Search (k=10) | 2.35K ops/sec | 42ms |
| k-hop Traversal | 10.28K ops/sec | 9.73ms |

## Platform Support

| Platform | Architecture | Status |
|----------|--------------|--------|
| Linux | x64 (glibc) | Supported |
| Linux | arm64 (glibc) | Supported |
| macOS | x64 | Supported |
| macOS | arm64 (M1/M2) | Supported |
| Windows | x64 | Supported |

## License

MIT
