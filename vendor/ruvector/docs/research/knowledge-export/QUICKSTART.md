# RuVector Developer Quickstart

> Distilled from 3,135 commits, 91 crates, and 55 ADRs across 99 days of development.

## What is RuVector?

A Rust-native computation platform for vectors, graphs, and neural networks. Not just a vector database — a full stack from PostgreSQL extension to WASM microkernel.

**91 crates** organized in layers:

```
Applications     ruvector-postgres (230+ SQL), ruvllm (LLM serving), mcp-gate
                      |
Compute          ruvector-graph-transformer, ruvector-gnn, ruvector-solver,
                 ruvector-mincut, ruvector-attention (39 types), ruvector-coherence
                      |
Core             ruvector-core (HNSW + SIMD), ruvector-graph (Cypher),
                 ruvector-math, ruvector-verified (proofs)
                      |
Format           rvf-types, rvf-wire, rvf-runtime, rvf-crypto (ML-DSA-65)
                      |
Bindings         *-wasm (20+), *-node (NAPI-RS), ruvector-cli
```

## First Steps

### Build everything

```bash
# Prerequisites: Rust 1.83+, Node.js 20+
cargo build --workspace
npm run build   # NAPI-RS bindings
npm test
```

### Use the vector database

```rust
use ruvector_core::vector_db::VectorDb;

let db = VectorDb::create("my_vectors.db", 384)?; // 384-dim embeddings
db.insert("doc1", &embedding_vector, &metadata)?;
let results = db.search(&query_vector, 10)?;        // top-10 nearest
```

### Use from PostgreSQL

```sql
CREATE EXTENSION ruvector;

CREATE TABLE items (id serial, embedding vector(384));
CREATE INDEX ON items USING hnsw (embedding vector_cosine_ops);

SELECT * FROM items ORDER BY embedding <=> '[0.1, 0.2, ...]' LIMIT 10;

-- GNN in SQL
SELECT ruvector_gcn_forward(features, adjacency, weights);

-- Flash attention in SQL
SELECT ruvector_flash_attention(q, k, v);
```

### Use from WASM

```js
import { VectorDb } from '@ruvector/wasm';
const db = new VectorDb(384);
db.insert('doc1', embedding);
const results = db.search(query, 10);
```

## Key Crates to Know

| If you need... | Use this crate | Key fact |
|----------------|---------------|----------|
| Vector search | `ruvector-core` | HNSW, SIMD, 2.5K qps on 10K vectors |
| Graph database | `ruvector-graph` | Neo4j-compatible Cypher, petgraph + roaring |
| GNN training | `ruvector-gnn` | Message-passing on HNSW topology |
| Graph transformers | `ruvector-graph-transformer` | 8 verified modules, proof-gated |
| LLM inference | `ruvllm` | Paged attention, Metal/CUDA/CoreML |
| Sparse solvers | `ruvector-solver` | O(log n) PageRank, spectral methods |
| Min-cut | `ruvector-mincut` | First subpolynomial dynamic min-cut |
| PostgreSQL | `ruvector-postgres` | 230+ SQL functions, pgvector replacement |
| Binary format | `rvf-*` | 25 segment types, crash-safe, post-quantum |

## Architecture Patterns

### Feature flags everywhere

```toml
[features]
default = ["simd", "storage", "hnsw", "parallel"]
wasm = []              # Disables storage, SIMD, parallel
full = ["simd", "storage", "async-runtime", "compression", "hnsw"]
```

Every WASM crate mirrors a non-WASM crate. Storage falls back to in-memory.

### Concurrency stack

- `rayon` — data parallelism (map/reduce)
- `crossbeam` — channels and concurrent queues
- `dashmap` — concurrent HashMap (never use `std::sync::Mutex`)
- `parking_lot` — fast locks when you must lock

### Testing strategy

- `proptest` for property-based testing
- `criterion` for benchmarks
- `mockall` for mocking
- London-school TDD (mock-first) for new code

### Publishing order

Leaf crates first, then dependents:
```
ruvector-solver → ruvector-solver-wasm, ruvector-solver-node
```

Always: `cargo publish --dry-run --allow-dirty` before real publish.

## RVF Format (The Unifier)

All RuVector libraries converge on RVF — a single binary format with:

- **25 segment types** (Vec, Index, Overlay, Journal, Manifest, Quant, Meta, Witness, Crypto, Kernel, WASM, ...)
- **Crash-safe** without WAL (append-only + two-fsync protocol)
- **Progressive indexing** (Layer A/B/C — first query in <5ms)
- **Post-quantum crypto** (ML-DSA-65 signatures)
- **5 domain profiles** (.rvf, .rvdna, .rvtext, .rvgraph, .rvvis)
- **Self-booting** (embedded WASM microkernel <8KB)

```rust
use rvf_runtime::RvfStore;

let store = RvfStore::create("knowledge.rvf", options)?;
store.ingest_batch(&embeddings, &ids, Some(&metadata))?;
let results = store.query(&query_vec, 10, &query_options)?;
```

## Critical ADRs to Read

| ADR | Why it matters |
|-----|---------------|
| ADR-001 | Core architecture — the foundation everything builds on |
| ADR-029 | RVF canonical format — the single most important design decision |
| ADR-015 | Coherence-gated transformer — sheaf attention mechanism |
| ADR-046 | Graph transformer architecture — the unified compute model |
| ADR-044 | PostgreSQL v0.3 — 230+ SQL functions |
| ADR-042 | TEE attestation — confidential computing model |

## Common Gotchas

1. **redb locking** — Use the global connection pool; don't open the same DB file twice
2. **NAPI binaries** — `git add -f` needed in CI to commit .node files past .gitignore
3. **WASM size** — Microkernel budget is 8KB; CI asserts `wasm-opt -Oz < 8192`
4. **pgrx** — Requires explicit `--features pg17` in test commands
5. **ruvector-profiler** — Has `publish = false`; intentionally not on crates.io
6. **Rust version** — Main workspace needs 1.83+; rvf crates need 1.87+

## Project Links

- **Repository**: https://github.com/ruvnet/ruvector
- **ADRs**: `docs/adr/` (55+ decisions)
- **Benchmarks**: `cargo bench` in individual crates
- **Knowledge export**: `docs/research/knowledge-export/ruvector-knowledge.rvf.json`
