# RuVector Graph WASM

WebAssembly bindings for RuVector graph database with Neo4j-inspired API and Cypher support.

## Features

- **Neo4j-style API**: Familiar node, edge, and relationship operations
- **Hypergraph Support**: N-ary relationships beyond binary edges
- **Cypher Queries**: Basic Cypher query language support
- **Browser & Node.js**: Works in both environments
- **Web Workers**: Background query execution
- **Async Operations**: Streaming results for large datasets
- **Vector Embeddings**: First-class support for semantic relationships

## Installation

```bash
npm install @ruvector/graph-wasm
```

## Quick Start

### Browser (ES Modules)

```javascript
import init, { GraphDB } from '@ruvector/graph-wasm';

await init();

// Create database
const db = new GraphDB('cosine');

// Create nodes
const aliceId = db.createNode(
  ['Person'],
  { name: 'Alice', age: 30 }
);

const bobId = db.createNode(
  ['Person'],
  { name: 'Bob', age: 35 }
);

// Create relationship
const friendshipId = db.createEdge(
  aliceId,
  bobId,
  'KNOWS',
  { since: 2020 }
);

// Query (basic Cypher support)
const results = await db.query('MATCH (n:Person) RETURN n');

// Get statistics
const stats = db.stats();
console.log(`Nodes: ${stats.nodeCount}, Edges: ${stats.edgeCount}`);
```

### Node.js

```javascript
const { GraphDB } = require('@ruvector/graph-wasm/node');

const db = new GraphDB('cosine');
// ... same API as browser
```

## API Reference

### GraphDB

Main class for graph database operations.

#### Constructor

```javascript
new GraphDB(metric?: string)
```

- `metric`: Distance metric for hypergraph embeddings
  - `"cosine"` (default)
  - `"euclidean"`
  - `"dotproduct"`
  - `"manhattan"`

#### Methods

##### Node Operations

```javascript
createNode(labels: string[], properties: object): string
```
Create a node with labels and properties. Returns node ID.

```javascript
getNode(id: string): JsNode | null
```
Retrieve a node by ID.

```javascript
deleteNode(id: string): boolean
```
Delete a node and its associated edges.

##### Edge Operations

```javascript
createEdge(
  from: string,
  to: string,
  type: string,
  properties: object
): string
```
Create a directed edge between two nodes.

```javascript
getEdge(id: string): JsEdge | null
```
Retrieve an edge by ID.

```javascript
deleteEdge(id: string): boolean
```
Delete an edge.

##### Hyperedge Operations

```javascript
createHyperedge(
  nodes: string[],
  description: string,
  embedding?: number[],
  confidence?: number
): string
```
Create an n-ary relationship connecting multiple nodes.

```javascript
getHyperedge(id: string): JsHyperedge | null
```
Retrieve a hyperedge by ID.

##### Query Operations

```javascript
async query(cypher: string): Promise<QueryResult>
```
Execute a Cypher query. Supports basic MATCH and CREATE statements.

```javascript
async importCypher(statements: string[]): Promise<number>
```
Import multiple Cypher CREATE statements.

```javascript
exportCypher(): string
```
Export the entire database as Cypher CREATE statements.

##### Statistics

```javascript
stats(): object
```
Get database statistics:
- `nodeCount`: Total number of nodes
- `edgeCount`: Total number of edges
- `hyperedgeCount`: Total number of hyperedges
- `hypergraphEntities`: Entities in hypergraph index
- `hypergraphEdges`: Hyperedges in index
- `avgEntityDegree`: Average entity degree

### Types

#### JsNode

```typescript
interface JsNode {
  id: string;
  labels: string[];
  properties: object;
  embedding?: number[];

  getProperty(key: string): any;
  hasLabel(label: string): boolean;
}
```

#### JsEdge

```typescript
interface JsEdge {
  id: string;
  from: string;
  to: string;
  type: string;
  properties: object;

  getProperty(key: string): any;
}
```

#### JsHyperedge

```typescript
interface JsHyperedge {
  id: string;
  nodes: string[];
  description: string;
  embedding: number[];
  confidence: number;
  properties: object;
  order: number; // Number of connected nodes
}
```

#### QueryResult

```typescript
interface QueryResult {
  nodes: JsNode[];
  edges: JsEdge[];
  hyperedges: JsHyperedge[];
  data: object[];
  count: number;
  isEmpty(): boolean;
}
```

## Advanced Features

### Async Query Execution

For large result sets, use async query execution with streaming:

```javascript
import { AsyncQueryExecutor } from '@ruvector/graph-wasm';

const executor = new AsyncQueryExecutor(100); // Batch size
const results = await executor.executeStreaming(
  'MATCH (n:Person) RETURN n'
);
```

### Web Worker Support

Execute queries in the background:

```javascript
const executor = new AsyncQueryExecutor();
const promise = executor.executeInWorker(
  'MATCH (n) RETURN count(n)'
);
```

### Batch Operations

Optimize multiple operations:

```javascript
import { BatchOperations } from '@ruvector/graph-wasm';

const batch = new BatchOperations(1000); // Max batch size
await batch.executeBatch([
  'CREATE (n:Person {name: "Alice"})',
  'CREATE (n:Person {name: "Bob"})',
  // ... more statements
]);
```

### Transactions

Atomic operation execution:

```javascript
import { AsyncTransaction } from '@ruvector/graph-wasm';

const tx = new AsyncTransaction();
tx.addOperation('CREATE (n:Person {name: "Alice"})');
tx.addOperation('CREATE (n:Person {name: "Bob"})');

try {
  await tx.commit();
} catch (error) {
  tx.rollback();
}
```

## Cypher Support

Currently supports basic Cypher operations:

### CREATE

```cypher
CREATE (n:Person {name: "Alice", age: 30})
CREATE (n:Person)-[:KNOWS]->(m:Person)
```

### MATCH

```cypher
MATCH (n:Person) RETURN n
MATCH (n:Person)-[r:KNOWS]->(m) RETURN n, r, m
```

**Note**: Full Cypher support is planned for future releases.

## Hypergraph Examples

### Creating Multi-Entity Relationships

```javascript
// Create nodes
const doc1 = db.createNode(['Document'], {
  title: 'AI Research',
  embedding: [0.1, 0.2, 0.3, ...] // 384-dim vector
});

const doc2 = db.createNode(['Document'], {
  title: 'ML Tutorial'
});

const author = db.createNode(['Person'], {
  name: 'Dr. Smith'
});

// Create hyperedge connecting all three
const hyperedgeId = db.createHyperedge(
  [doc1, doc2, author],
  'Documents authored by researcher on related topics',
  null, // Auto-generate embedding from node embeddings
  0.95  // High confidence
);

const hyperedge = db.getHyperedge(hyperedgeId);
console.log(`Hyperedge connects ${hyperedge.order} nodes`);
```

## Performance

- **Zero-copy transfers**: Uses WASM memory for efficient data transfer
- **SIMD acceleration**: When available in WASM environment
- **Lazy evaluation**: Streaming results for large queries
- **Optimized indices**: Fast lookups by label, type, and properties

## Browser Compatibility

- Chrome 90+
- Firefox 88+
- Safari 15.4+
- Edge 90+

## Building from Source

```bash
# Install wasm-pack
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

# Build for web
npm run build

# Build for all targets
npm run build:all

# Run tests
npm test
```

## Examples

See the [examples directory](../../../examples/) for more usage examples:
- Basic graph operations
- Hypergraph relationships
- Temporal queries
- Vector similarity search

## Roadmap

- [ ] Full Cypher query parser
- [ ] IndexedDB persistence
- [ ] Graph algorithms (PageRank, community detection)
- [ ] Schema validation
- [ ] Transaction log
- [ ] Multi-graph support
- [ ] GraphQL integration

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](../../../CONTRIBUTING.md).

## License

MIT - See [LICENSE](../../../LICENSE) for details.

## Support

- GitHub Issues: https://github.com/ruvnet/ruvector/issues
- Documentation: https://github.com/ruvnet/ruvector/wiki

## Related Projects

- [ruvector-core](../ruvector-core) - Core vector database
- [ruvector-wasm](../ruvector-wasm) - Vector database WASM bindings
- [ruvector-node](../ruvector-node) - Node.js native bindings
