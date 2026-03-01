# RvLite

Lightweight vector database with SQL, SPARQL, and Cypher - runs everywhere (Node.js, Browser, Edge).

Built on WebAssembly for maximum performance and portability. Only ~850KB!

## Features

- **Vector Search** - Semantic similarity with cosine, euclidean, or dot product distance
- **SQL** - Query vectors with standard SQL plus distance operations
- **Cypher** - Property graph queries (Neo4j-compatible syntax)
- **SPARQL** - RDF triple store with W3C SPARQL queries
- **Persistence** - Save/load to file (Node.js) or IndexedDB (browser)
- **Tiny** - ~850KB WASM bundle, no external dependencies

## Installation

```bash
npm install rvlite
```

## CLI Usage

```bash
# Initialize a new database
npx rvlite init

# Insert a vector
npx rvlite insert "[0.1, 0.2, 0.3, ...]" --metadata '{"text": "Hello"}'

# Search for similar vectors
npx rvlite search "[0.1, 0.2, 0.3, ...]" --top 5

# SQL queries
npx rvlite sql "SELECT * FROM vectors LIMIT 10"

# Cypher queries
npx rvlite cypher "CREATE (p:Person {name: 'Alice'})"
npx rvlite cypher "MATCH (p:Person) RETURN p"

# SPARQL queries
npx rvlite triple "http://example.org/alice" "http://xmlns.com/foaf/0.1/name" "Alice"
npx rvlite sparql "SELECT ?s ?p ?o WHERE { ?s ?p ?o }"

# Interactive REPL
npx rvlite repl

# Database stats
npx rvlite stats

# Export/Import
npx rvlite export backup.json
npx rvlite import backup.json
```

### REPL Commands

```
.sql      - Switch to SQL mode
.cypher   - Switch to Cypher mode
.sparql   - Switch to SPARQL mode
.stats    - Show database statistics
.save     - Save database
.exit     - Exit REPL
```

## SDK Usage

### Basic Vector Operations

```typescript
import { RvLite, createRvLite } from 'rvlite';

// Create instance
const db = await createRvLite({ dimensions: 384 });

// Insert vectors
const id = await db.insert([0.1, 0.2, 0.3, ...], { text: "Hello world" });

// Insert with custom ID
await db.insertWithId("my-doc-1", [0.1, 0.2, ...], { source: "article" });

// Search similar
const results = await db.search([0.1, 0.2, ...], 5);
// [{ id: "...", score: 0.95, metadata: {...} }, ...]

// Get by ID
const item = await db.get("my-doc-1");

// Delete
await db.delete("my-doc-1");
```

### SQL Queries

```typescript
// Create table and insert
await db.sql("CREATE TABLE documents (id TEXT, title TEXT, embedding VECTOR)");
await db.sql("INSERT INTO documents VALUES ('doc1', 'Hello', '[0.1, 0.2, ...]')");

// Query with vector distance
const results = await db.sql(`
  SELECT id, title, distance(embedding, '[0.1, 0.2, ...]') as dist
  FROM documents
  WHERE dist < 0.5
  ORDER BY dist
`);
```

### Cypher Graph Queries

```typescript
// Create nodes
await db.cypher("CREATE (alice:Person {name: 'Alice', age: 30})");
await db.cypher("CREATE (bob:Person {name: 'Bob', age: 25})");

// Create relationships
await db.cypher(`
  MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'})
  CREATE (a)-[:KNOWS {since: 2020}]->(b)
`);

// Query
const friends = await db.cypher(`
  MATCH (p:Person)-[:KNOWS]->(friend:Person)
  WHERE p.name = 'Alice'
  RETURN friend.name
`);
```

### SPARQL RDF Queries

```typescript
// Add triples
await db.addTriple(
  "http://example.org/alice",
  "http://xmlns.com/foaf/0.1/name",
  "Alice"
);

await db.addTriple(
  "http://example.org/alice",
  "http://xmlns.com/foaf/0.1/knows",
  "http://example.org/bob"
);

// Query
const results = await db.sparql(`
  PREFIX foaf: <http://xmlns.com/foaf/0.1/>
  SELECT ?name WHERE {
    <http://example.org/alice> foaf:name ?name
  }
`);
```

### Persistence

```typescript
// Node.js - Export to file
const state = await db.exportJson();
fs.writeFileSync('db.json', JSON.stringify(state));

// Node.js - Import from file
const data = JSON.parse(fs.readFileSync('db.json'));
await db.importJson(data);

// Browser - Save to IndexedDB
await db.save();

// Browser - Load from IndexedDB
const db = await RvLite.load({ dimensions: 384 });

// Browser - Clear storage
await RvLite.clearStorage();
```

### Semantic Memory for AI

```typescript
import { RvLite, SemanticMemory, createRvLite } from 'rvlite';

// Create with embedding provider
const db = await createRvLite({ dimensions: 1536 });
const memory = new SemanticMemory(db);

// Store memories with embeddings
await memory.store("conv-1", "User asked about weather", embedding1);
await memory.store("conv-2", "Discussed travel plans", embedding2);

// Add relationships
await memory.addRelation("conv-1", "LEADS_TO", "conv-2");

// Query by similarity
const similar = await memory.query("What was the weather question?", queryEmbedding);

// Find related through graph
const related = await memory.findRelated("conv-1", 2);
```

## RVF Storage Backend

RvLite can use [RVF (RuVector Format)](https://github.com/ruvnet/ruvector/tree/main/crates/rvf) as a persistent storage backend. When the optional `@ruvector/rvf-wasm` package is installed, rvlite gains file-backed persistence using the `.rvf` cognitive container format.

### Install

```bash
npm install rvlite @ruvector/rvf-wasm
```

### Usage

```typescript
import { createRvLite } from 'rvlite';

// rvlite auto-detects @ruvector/rvf-wasm when installed
const db = await createRvLite({ dimensions: 384 });

// All operations persist to RVF format
await db.insert([0.1, 0.2, ...], { text: "Hello world" });
const results = await db.search([0.1, 0.2, ...], 5);
```

### Platform Support

The RVF backend works everywhere rvlite runs:

| Platform | RVF Backend | Notes |
|----------|-------------|-------|
| Node.js (Linux, macOS, Windows) | Native or WASM | Auto-detected |
| Browser (Chrome, Firefox, Safari) | WASM | IndexedDB + RVF |
| Deno | WASM | Via `npm:` specifier |
| Cloudflare Workers / Edge | WASM | Stateless queries |

### Rust Feature Flag

If building from source, enable the `rvf-backend` feature in `crates/rvlite`:

```toml
[dependencies]
rvlite = { version = "0.1", features = ["rvf-backend"] }
```

This enables epoch-based reconciliation between RVF and metadata stores:
- Monotonic epoch counter shared between RVF and metadata
- On startup, compares epochs and rebuilds the lagging side
- RVF file is source of truth; metadata (IndexedDB) is rebuildable cache

### Download Example .rvf Files

```bash
# Download pre-built examples to test with
curl -LO https://raw.githubusercontent.com/ruvnet/ruvector/main/examples/rvf/output/basic_store.rvf
curl -LO https://raw.githubusercontent.com/ruvnet/ruvector/main/examples/rvf/output/semantic_search.rvf
curl -LO https://raw.githubusercontent.com/ruvnet/ruvector/main/examples/rvf/output/agent_memory.rvf

# 45 examples available at:
# https://github.com/ruvnet/ruvector/tree/main/examples/rvf/output
```

---

## Integration with claude-flow

RvLite can enhance claude-flow's memory system with semantic search:

```typescript
import { RvLite, SemanticMemory } from 'rvlite';

// In your claude-flow hooks
const db = await createRvLite({ dimensions: 1536 });

// Pre-task hook: Find relevant context
async function preTask(task) {
  const embedding = await getEmbedding(task.description);
  const context = await db.search(embedding, 5);
  return { ...task, context };
}

// Post-task hook: Store results
async function postTask(task, result) {
  const embedding = await getEmbedding(result.summary);
  await db.insert(embedding, {
    task: task.id,
    result: result.summary,
    timestamp: Date.now()
  });
}
```

## API Reference

### RvLite Class

| Method | Description |
|--------|-------------|
| `insert(vector, metadata?)` | Insert vector, returns ID |
| `insertWithId(id, vector, metadata?)` | Insert with custom ID |
| `search(query, k)` | Find k nearest vectors |
| `get(id)` | Get vector by ID |
| `delete(id)` | Delete vector by ID |
| `len()` | Count of vectors |
| `sql(query)` | Execute SQL query |
| `cypher(query)` | Execute Cypher query |
| `cypherStats()` | Get graph statistics |
| `sparql(query)` | Execute SPARQL query |
| `addTriple(s, p, o, graph?)` | Add RDF triple |
| `tripleCount()` | Count of triples |
| `exportJson()` | Export state to JSON |
| `importJson(data)` | Import state from JSON |
| `save()` | Save to IndexedDB (browser) |
| `RvLite.load(config)` | Load from IndexedDB |
| `RvLite.clearStorage()` | Clear IndexedDB |

### CLI Commands

| Command | Description |
|---------|-------------|
| `rvlite init` | Initialize database |
| `rvlite insert <vector>` | Insert vector |
| `rvlite search <vector>` | Search similar |
| `rvlite sql <query>` | Execute SQL |
| `rvlite cypher <query>` | Execute Cypher |
| `rvlite sparql <query>` | Execute SPARQL |
| `rvlite triple <s> <p> <o>` | Add triple |
| `rvlite stats` | Show statistics |
| `rvlite export <file>` | Export to JSON |
| `rvlite import <file>` | Import from JSON |
| `rvlite repl` | Start interactive mode |

## License

MIT OR Apache-2.0
