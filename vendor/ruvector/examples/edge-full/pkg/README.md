# @ruvector/edge-full

[![npm](https://img.shields.io/npm/v/@ruvector/edge-full.svg)](https://www.npmjs.com/package/@ruvector/edge-full)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![WASM](https://img.shields.io/badge/wasm-8.4MB-purple.svg)]()

## The Complete Edge AI Toolkit

**Run AI agent swarms, graph databases, neural networks, workflow engines, and ONNX inference - all in the browser, all for free.**

@ruvector/edge-full is the batteries-included version of the RuVector edge computing platform. It bundles six powerful WASM modules into a single package, giving you everything you need to build sophisticated distributed AI systems that run entirely on user devices.

### What's Inside

| Module | Size | What It Does |
|--------|------|--------------|
| **Edge Core** | 364KB | Cryptographic identity (Ed25519), AES-256-GCM encryption, HNSW vector search, Raft consensus, spiking neural networks, post-quantum signatures |
| **Graph DB** | 288KB | Neo4j-style graph database with Cypher query language, relationship modeling, traversal algorithms |
| **RVLite** | 260KB | Multi-query vector database supporting SQL, SPARQL, and Cypher - semantic search with familiar syntax |
| **SONA** | 238KB | Self-Optimizing Neural Architecture - LoRA fine-tuning, EWC++, ReasoningBank for adaptive learning |
| **DAG** | 132KB | Directed acyclic graph engine for workflow orchestration, dependency resolution, topological execution |
| **ONNX** | 7.1MB | Full ONNX inference engine with 6 pre-trained HuggingFace embedding models, parallel worker support |

**Total: 1.28MB core + 7.1MB optional ONNX = 8.4MB complete**

### Why Use This?

- **Zero Infrastructure Cost**: Everything runs in the browser. No servers, no API keys, no monthly bills.
- **Complete Feature Set**: Vector search + graph database + neural networks + workflow engine + embeddings. All in one package.
- **True P2P**: Agents communicate directly via WebRTC, GUN.js, libp2p, or Nostr. No central server required.
- **Self-Learning**: SONA provides continuous improvement through LoRA fine-tuning and experience replay.
- **Production-Ready**: Post-quantum cryptography, Byzantine fault tolerance, encrypted communication.

## Quick Start

```bash
npm install @ruvector/edge-full
```

### Initialize All Modules

```javascript
import { initAll } from '@ruvector/edge-full';

// Initialize all core modules (excludes ONNX for faster startup)
const { edge, graph, rvlite, sona, dag } = await initAll();

// Create agent identity
const identity = new edge.WasmIdentity.generate();
console.log(`Agent: ${identity.agent_id()}`);

// Build a knowledge graph
const graphStore = new graph.WasmGraphStore();
graphStore.run_cypher("CREATE (a:Agent {id: 'agent-1', type: 'researcher'})");

// Vector search with SQL
const db = new rvlite.Database();
db.execute("CREATE TABLE memories (id TEXT, embedding BLOB)");

// Self-learning neural routing
const sonaEngine = new sona.SonaEngine();
sonaEngine.route_request({ task: "analyze code", context: "rust" });

// Workflow orchestration
const workflow = new dag.Dag();
workflow.add_node("fetch");
workflow.add_node("process");
workflow.add_edge("fetch", "process");
```

### Selective Module Loading

```javascript
import { initModules } from '@ruvector/edge-full';

// Only load what you need
const { edge, graph } = await initModules(['edge', 'graph']);

// Or import modules directly
import init from '@ruvector/edge-full/edge';
import graphInit, { WasmGraphStore } from '@ruvector/edge-full/graph';

await init();
await graphInit();
```

### Add ONNX Embeddings

```javascript
import onnxInit, { WasmEmbedder } from '@ruvector/edge-full/onnx';

await onnxInit();

const embedder = new WasmEmbedder();
await embedder.load_model('all-MiniLM-L6-v2'); // 384-dimensional embeddings

const embedding = await embedder.embed("The quick brown fox");
console.log(`Dimensions: ${embedding.length}`); // 384
```

## Module Deep Dive

### Edge Core - Cryptographic Foundation

```javascript
import init, {
  WasmIdentity,      // Ed25519 key pairs
  WasmCrypto,        // AES-256-GCM encryption
  WasmHnswIndex,     // 150x faster vector search
  WasmRaftNode,      // Distributed consensus
  WasmHybridKeyPair, // Post-quantum signatures
  WasmSpikingNetwork // Bio-inspired neural nets
} from '@ruvector/edge-full/edge';

await init();

// Create cryptographic identity
const identity = WasmIdentity.generate();
const signature = identity.sign(new TextEncoder().encode("hello"));
const verified = identity.verify(new TextEncoder().encode("hello"), signature);

// Encrypted communication
const crypto = new WasmCrypto();
const encrypted = crypto.encrypt(data, key, nonce);

// HNSW vector index (150x faster than brute force)
const index = new WasmHnswIndex(384, 16, 200); // dimensions, M, ef_construction
index.add(0, embedding1);
index.add(1, embedding2);
const neighbors = index.search(queryVector, 10); // top 10 results

// Raft consensus for distributed state
const node = new WasmRaftNode('node-1', ['node-1', 'node-2', 'node-3']);
node.start_election();
```

### Graph DB - Neo4j in the Browser

```javascript
import graphInit, { WasmGraphStore } from '@ruvector/edge-full/graph';

await graphInit();

const store = new WasmGraphStore();

// Create nodes and relationships
store.run_cypher(`
  CREATE (alice:Person {name: 'Alice', role: 'researcher'})
  CREATE (bob:Person {name: 'Bob', role: 'developer'})
  CREATE (alice)-[:COLLABORATES_WITH]->(bob)
`);

// Query the graph
const results = store.run_cypher(`
  MATCH (p:Person)-[:COLLABORATES_WITH]->(colleague)
  RETURN p.name, colleague.name
`);

// Complex traversals
const paths = store.run_cypher(`
  MATCH path = (start:Person)-[:KNOWS*1..3]->(end:Person)
  WHERE start.name = 'Alice'
  RETURN path
`);
```

### RVLite - SQL + SPARQL + Cypher Vector DB

```javascript
import rvliteInit, { Database } from '@ruvector/edge-full/rvlite';

await rvliteInit();

const db = new Database();

// SQL for familiar operations
db.execute(`
  CREATE TABLE documents (
    id TEXT PRIMARY KEY,
    content TEXT,
    embedding BLOB
  )
`);

db.execute(`INSERT INTO documents VALUES (?, ?, ?)`,
  ['doc-1', 'Machine learning basics', embedding1]);

// Semantic search
const similar = db.execute(`
  SELECT * FROM documents
  ORDER BY vector_distance(embedding, ?)
  LIMIT 5
`, [queryEmbedding]);

// SPARQL for knowledge graphs
const sparqlResults = db.sparql(`
  PREFIX ex: <http://example.org/>
  SELECT ?name ?type
  WHERE {
    ?entity ex:type ?type .
    ?entity ex:name ?name .
    FILTER (?type = "Agent")
  }
`);
```

### SONA - Self-Learning Neural Router

```javascript
import sonaInit, { SonaEngine, ReasoningBank } from '@ruvector/edge-full/sona';

await sonaInit();

const engine = new SonaEngine();
const reasoningBank = new ReasoningBank();

// Route tasks to best agent
const decision = await engine.route_request({
  task: "review pull request",
  context: { language: "rust", complexity: "high" },
  available_agents: [
    { id: "agent-1", capabilities: ["rust", "code-review"] },
    { id: "agent-2", capabilities: ["testing", "qa"] }
  ]
});

console.log(`Routed to: ${decision.selected_agent}`);
console.log(`Confidence: ${decision.confidence}`);

// Learn from outcomes
reasoningBank.record_trajectory({
  state: "review pull request",
  action: decision.selected_agent,
  reward: 0.95, // positive outcome
  timestamp: Date.now()
});

// Apply LoRA fine-tuning
await engine.apply_lora_update({
  positive_examples: ["rust expert handled rust code well"],
  negative_examples: []
});
```

### DAG - Workflow Orchestration

```javascript
import dagInit, { Dag } from '@ruvector/edge-full/dag';

await dagInit();

const workflow = new Dag();

// Define workflow steps
workflow.add_node("fetch_data");
workflow.add_node("validate");
workflow.add_node("transform");
workflow.add_node("store");

// Define dependencies
workflow.add_edge("fetch_data", "validate");
workflow.add_edge("validate", "transform");
workflow.add_edge("transform", "store");

// Get execution order
const order = workflow.topological_sort();
console.log(order); // ["fetch_data", "validate", "transform", "store"]

// Check for cycles
if (workflow.has_cycle()) {
  console.error("Invalid workflow!");
}

// Get dependencies for a node
const deps = workflow.get_dependencies("transform");
console.log(deps); // ["validate"]
```

### ONNX - HuggingFace Embeddings

```javascript
import onnxInit, { WasmEmbedder } from '@ruvector/edge-full/onnx';

await onnxInit();

const embedder = new WasmEmbedder();

// Available models:
// - all-MiniLM-L6-v2    (384D, fastest)
// - all-MiniLM-L12-v2   (384D, better quality)
// - bge-small-en-v1.5   (384D, SOTA)
// - bge-base-en-v1.5    (768D, highest quality)
// - e5-small-v2         (384D, search/retrieval)
// - gte-small           (384D, multilingual)

await embedder.load_model('bge-small-en-v1.5');

// Single embedding
const embedding = await embedder.embed("What is machine learning?");

// Batch processing (3.8x faster with parallel workers)
const embeddings = await embedder.embed_batch([
  "First document about AI",
  "Second document about ML",
  "Third document about neural networks"
]);

// Compute similarity
function cosineSimilarity(a, b) {
  let dot = 0, normA = 0, normB = 0;
  for (let i = 0; i < a.length; i++) {
    dot += a[i] * b[i];
    normA += a[i] * a[i];
    normB += b[i] * b[i];
  }
  return dot / (Math.sqrt(normA) * Math.sqrt(normB));
}

const similarity = cosineSimilarity(embeddings[0], embeddings[1]);
```

## Interactive Generator

The package includes an interactive HTML generator that creates ready-to-use code for any combination of:

- **6 Network Topologies**: Mesh, Star, Hierarchical, Ring, Gossip, Sharded
- **4 P2P Transports**: GUN.js, WebRTC, libp2p, Nostr
- **6 Use Cases**: AI Assistants, Data Pipeline, Gaming, IoT, Marketplace, Research
- **6 WASM Modules**: Edge, Graph, RVLite, SONA, DAG, ONNX
- **8 Core Features**: Identity, Encryption, HNSW, Semantic Match, Raft, Post-Quantum, Spiking NN, Compression
- **7 Exotic Patterns**: MCP Tools, Byzantine Fault, Quantum Resistant, Neural Consensus, Swarm Intelligence, Self-Healing, Emergent Behavior

Open `generator.html` in your browser to start generating code.

## Bundle Size Comparison

| Configuration | Size | Use Case |
|--------------|------|----------|
| Edge only | 364KB | Minimal crypto + vectors |
| Edge + Graph | 652KB | Agent relationships |
| Edge + RVLite | 624KB | SQL-style queries |
| Edge + SONA | 602KB | Self-learning routing |
| All Core | 1.28MB | Full capabilities |
| With ONNX | 8.4MB | ML embeddings |

## Free Infrastructure

All components use free public infrastructure:

| Service | Free Providers |
|---------|----------------|
| P2P Relay | GUN.js (gun-manhattan, gun-us-west) |
| STUN | Google, Twilio, Cloudflare |
| Signaling | PeerJS Cloud (free tier) |
| Nostr Relays | nostr.wine, relay.damus.io, nos.lol |

## Performance

| Module | Operation | Performance |
|--------|-----------|-------------|
| Edge | Ed25519 sign/verify | 50,000 ops/sec |
| Edge | AES-256-GCM | 1 GB/sec |
| Edge | HNSW search | 150x faster than brute force |
| Graph | Cypher query | <1ms for simple queries |
| RVLite | Vector search | Sub-millisecond |
| SONA | Route decision | <5ms |
| ONNX | Single embed | ~20ms (MiniLM-L6) |
| ONNX | Batch embed | 3.8x speedup with workers |

## Examples

See the `/examples` directory for:

- Multi-agent chat with shared memory
- Distributed RAG pipeline
- Real-time multiplayer coordination
- IoT sensor swarm
- Knowledge graph construction
- Workflow automation

## License

MIT License - Use freely for any purpose.

## Links

- [npm](https://www.npmjs.com/package/@ruvector/edge-full)
- [GitHub](https://github.com/ruvnet/ruvector)
- [@ruvector/edge](https://www.npmjs.com/package/@ruvector/edge) (lightweight version)
