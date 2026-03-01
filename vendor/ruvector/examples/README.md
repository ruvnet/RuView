# RuVector Examples

Comprehensive examples demonstrating RuVector's capabilities across multiple platforms and use cases.

## Directory Structure

```
examples/
├── rust/                 # Rust SDK examples
├── nodejs/               # Node.js SDK examples
├── graph/                # Graph database features
├── wasm-react/           # React + WebAssembly integration
├── wasm-vanilla/         # Vanilla JS + WebAssembly
├── agentic-jujutsu/      # AI agent version control
├── exo-ai-2025/          # Advanced cognitive substrate
├── refrag-pipeline/      # Document processing pipeline
└── docs/                 # Additional documentation
```

## Quick Start by Platform

### Rust

```bash
cd rust
cargo run --example basic_usage
cargo run --example advanced_features
cargo run --example agenticdb_demo
```

### Node.js

```bash
cd nodejs
npm install
node basic_usage.js
node semantic_search.js
```

### WebAssembly (React)

```bash
cd wasm-react
npm install
npm run dev
```

### WebAssembly (Vanilla)

```bash
cd wasm-vanilla
# Open index.html in browser
```

## Example Categories

| Category | Directory | Description |
|----------|-----------|-------------|
| **Core API** | `rust/basic_usage.rs` | Vector DB fundamentals |
| **Batch Ops** | `rust/batch_operations.rs` | High-throughput ingestion |
| **RAG Pipeline** | `rust/rag_pipeline.rs` | Retrieval-Augmented Generation |
| **Advanced** | `rust/advanced_features.rs` | Hypergraphs, neural hashing |
| **AgenticDB** | `rust/agenticdb_demo.rs` | AI agent memory system |
| **GNN** | `rust/gnn_example.rs` | Graph Neural Networks |
| **Graph** | `graph/` | Cypher queries, clustering |
| **Node.js** | `nodejs/` | JavaScript integration |
| **WASM React** | `wasm-react/` | Modern React apps |
| **WASM Vanilla** | `wasm-vanilla/` | Browser without framework |
| **Agentic Jujutsu** | `agentic-jujutsu/` | Multi-agent version control |
| **EXO-AI 2025** | `exo-ai-2025/` | Cognitive substrate research |
| **Refrag** | `refrag-pipeline/` | Document fragmentation |

## Feature Highlights

### Vector Database Core
- High-performance similarity search
- Multiple distance metrics (Cosine, Euclidean, Dot Product)
- Metadata filtering
- Batch operations

### Advanced Features
- **Hypergraph Index**: Multi-entity relationships
- **Temporal Hypergraph**: Time-aware relationships
- **Causal Memory**: Cause-effect chains
- **Learned Index**: ML-optimized indexing
- **Neural Hash**: Locality-sensitive hashing
- **Topological Analysis**: Persistent homology

### AgenticDB
- Reflexion episodes (self-critique)
- Skill library (consolidated patterns)
- Causal memory (hypergraph relationships)
- Learning sessions (RL training data)
- Vector embeddings (core storage)

### EXO-AI Cognitive Substrate
- **exo-core**: IIT consciousness, thermodynamics
- **exo-temporal**: Causal memory coordination
- **exo-hypergraph**: Topological structures
- **exo-manifold**: Continuous deformation
- **exo-exotic**: 10 cutting-edge experiments
- **exo-wasm**: Browser deployment
- **exo-federation**: Distributed consensus
- **exo-node**: Native bindings
- **exo-backend-classical**: Classical compute

## Running Benchmarks

```bash
# Rust benchmarks
cargo bench --example advanced_features

# Refrag pipeline benchmarks
cd refrag-pipeline
cargo bench

# EXO-AI benchmarks
cd exo-ai-2025
cargo bench
```

## Related Documentation

- [Graph CLI Usage](docs/graph-cli-usage.md)
- [Graph WASM Usage](docs/graph_wasm_usage.html)
- [Agentic Jujutsu](agentic-jujutsu/README.md)
- [Refrag Pipeline](refrag-pipeline/README.md)
- [EXO-AI 2025](exo-ai-2025/README.md)

## License

MIT OR Apache-2.0
