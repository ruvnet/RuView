<p align="center">
  <strong>RVF Examples</strong> &mdash; Learn by Running
</p>

<p align="center">
  <em>Hands-on examples for the unified agentic AI format &mdash; store it, send it, run it</em>
</p>

<p align="center">
  <a href="#quick-start">Quick Start</a> &bull;
  <a href="#examples-at-a-glance">Examples</a> &bull;
  <a href="#features-covered">Features</a> &bull;
  <a href="#performance">Performance</a> &bull;
  <a href="#comparison">Comparison</a>
</p>

<p align="center">
  <img alt="Examples" src="https://img.shields.io/badge/examples-40_runnable-brightgreen?style=flat-square" />
  <img alt="Rust" src="https://img.shields.io/badge/rust-1.87%2B-orange?style=flat-square" />
  <img alt="License" src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue?style=flat-square" />
  <img alt="Tests" src="https://img.shields.io/badge/tests-453_passing-brightgreen?style=flat-square" />
  <img alt="no_std" src="https://img.shields.io/badge/no__std-compatible-green?style=flat-square" />
  <img alt="Crates" src="https://img.shields.io/badge/crates-13-blue?style=flat-square" />
</p>

---

## What is RVF?

**RVF (RuVector Format)** is the unified agentic AI file format. One `.rvf` file does three jobs:

1. **Store** &mdash; vectors, indexes, metadata, and cryptographic proofs live in one file. No database server required.
2. **Transfer** &mdash; the same file streams over a network. Query, insert, and delete operations work over the wire with zero conversion.
3. **Run** &mdash; pack model weights, graph neural networks, WASM code, or even a bootable OS kernel into the file. Now it's not just data &mdash; it's a self-contained intelligence unit you can deploy anywhere.

### Why does this matter?

Today, an AI agent's state is scattered: embeddings in one database, model weights in another, graph structure in a third, config in a fourth. Nothing talks to anything else. Moving between tools means re-indexing from scratch. There's no standard way to prove any of it was computed securely &mdash; and no way to hand an agent its complete knowledge as a single portable artifact.

RVF solves this. It gives agentic AI a **universal substrate** &mdash; one file that works everywhere:

| What it does | Where it runs | What you get |
|-------------|--------------|-------------|
| Stores vectors | Server (HNSW index) | Sub-millisecond search over millions of vectors |
| Stores vectors | Browser (5.5 KB WASM) | Same file, no backend needed |
| Stores vectors | Edge / IoT / mobile | Lightweight API, tiny footprint |
| Transfers data | Over the network | Batched query/ingest/delete via TCP |
| Runs code | Inside a TEE | Cryptographic proof of secure computation |
| Runs code | Bare metal / VM | File boots itself as a microservice |
| Runs code | Linux kernel (eBPF) | Sub-microsecond hot-path acceleration |
| Runs intelligence | Anywhere | Model + data + graph + trust chain in one file |

### Key properties

- **Crash-safe** &mdash; no write-ahead log needed; if power dies mid-write, the file stays consistent
- **Self-describing** &mdash; the schema is in the file; no external catalog required
- **Progressive loading** &mdash; start answering queries before the full index is loaded
- **Domain profiles** &mdash; `.rvdna` for genomics, `.rvtext` for language, `.rvgraph` for networks, `.rvvis` for vision &mdash; same format underneath
- **Lineage tracking** &mdash; every derived file records its parent's hash, like DNA inheritance
- **Tamper-evident** &mdash; witness chains and post-quantum signatures prove nothing was altered

These examples walk you through every major feature, from the simplest "insert and query" to wire format inspection, witness chains, and sealed cognitive engines.

### What you can build with RVF

| Use case | What goes in the file | Result |
|----------|----------------------|--------|
| **Semantic search** | Vectors + HNSW index | Single-file vector database, no server needed |
| **Agent memory** | Vectors + metadata + witness chain | Portable, auditable AI agent knowledge base |
| **Sealed LoRA distribution** | Base embeddings + OVERLAY_SEG adapter deltas | Ship fine-tuned models as one versioned file |
| **Portable graph intelligence** | Node embeddings + GRAPH_SEG adjacency | GNN state that transfers between systems |
| **Self-booting AI service** | Vectors + index + KERNEL_SEG unikernel | File boots as a microservice on bare metal or Firecracker |
| **Kernel-accelerated cache** | Hot vectors + EBPF_SEG XDP program | Sub-microsecond lookups in the Linux kernel data path |
| **Confidential AI** | Any of the above + TEE attestation | Cryptographic proof everything ran inside a secure enclave |
| **Genomic analysis** | DNA k-mer embeddings + variant tensors | `.rvdna` file with lineage tracking across analysis pipeline |
| **Firmware-style AI versioning** | Full cognitive state + lineage chain | Parent &rarr; child derivation with hash verification, like DNA |

---

## Quick Start

```bash
# Clone the repo
git clone https://github.com/ruvnet/ruvector
cd ruvector/examples/rvf

# Run your first example
cargo run --example basic_store
```

That's it. You'll see a store created, 100 vectors inserted, nearest neighbors found, and persistence verified &mdash; all in under a second.

### Using the CLI

You can also work with RVF stores from the command line without writing any Rust:

```bash
# Build the CLI
cd crates/rvf && cargo build -p rvf-cli

# Create a store, ingest data, and query
rvf create vectors.rvf --dimension 384
rvf ingest vectors.rvf --input data.json --format json
rvf query vectors.rvf --vector "0.1,0.2,..." --k 10
rvf status vectors.rvf
rvf inspect vectors.rvf
rvf compact vectors.rvf

# Derive a child store with lineage tracking
rvf derive parent.rvf child.rvf --type filter

# All commands support --json for machine-readable output
rvf status vectors.rvf --json
```

<details>
<summary><strong>Run All 40 Examples</strong></summary>

**Core (6):**
```bash
cargo run --example basic_store          # Store lifecycle + k-NN
cargo run --example progressive_index    # Three-layer HNSW recall
cargo run --example quantization         # Scalar / product / binary
cargo run --example wire_format          # Raw segment I/O
cargo run --example crypto_signing       # Ed25519 + witness chains
cargo run --example filtered_search      # Metadata-filtered queries
```

**Agentic AI (6):**
```bash
cargo run --example agent_memory         # Persistent agent memory + witness audit
cargo run --example swarm_knowledge      # Multi-agent shared knowledge base
cargo run --example reasoning_trace      # Chain-of-thought with lineage derivation
cargo run --example tool_cache           # Tool call result cache with TTL
cargo run --example agent_handoff        # Transfer agent state between instances
cargo run --example experience_replay    # RL experience replay buffer
```

**Practical Production (5):**
```bash
cargo run --example semantic_search      # Document search with metadata filters
cargo run --example recommendation       # Item recommendations (collaborative filtering)
cargo run --example rag_pipeline         # Retrieval-augmented generation pipeline
cargo run --example embedding_cache      # LRU cache with temperature tiering
cargo run --example dedup_detector       # Near-duplicate detection + compaction
```

**Vertical Domains (4):**
```bash
cargo run --example genomic_pipeline     # DNA k-mer search (.rvdna profile)
cargo run --example financial_signals    # Market signals with TEE attestation
cargo run --example medical_imaging      # Radiology search (.rvvis profile)
cargo run --example legal_discovery      # Legal doc similarity (.rvtext profile)
```

**Exotic Capabilities (5):**
```bash
cargo run --example self_booting         # RVF with embedded unikernel
cargo run --example ebpf_accelerator     # eBPF hot-path acceleration
cargo run --example hyperbolic_taxonomy  # Hierarchy-aware search
cargo run --example multimodal_fusion    # Cross-modal text + image search
cargo run --example sealed_engine        # Full cognitive engine (capstone)
```

**Runtime Targets (4) + Postgres (1):**
```bash
cargo run --example browser_wasm         # Browser-side WASM vector search
cargo run --example edge_iot             # IoT device with binary quantization
cargo run --example serverless_function  # Cold-start optimized for Lambda
cargo run --example ruvllm_inference     # LLM KV cache + LoRA via RVF
cargo run --example postgres_bridge      # PostgreSQL ↔ RVF export/import
```

**Network & Security (4):**
```bash
cargo run --example network_sync         # Peer-to-peer vector store sync
cargo run --example tee_attestation      # TEE attestation + sealed keys
cargo run --example access_control       # Role-based vector access control
cargo run --example zero_knowledge       # Zero-knowledge proof integration
```

**Autonomous Agent (1):**
```bash
cargo run --example ruvbot               # Autonomous RVF-powered agent bot
```

**POSIX & Systems (3):**
```bash
cargo run --example posix_fileops        # POSIX file operations with RVF
cargo run --example linux_microkernel    # Linux microkernel distribution
cargo run --example mcp_in_rvf           # MCP server embedded in RVF
```

**Network Operations (1):**
```bash
cargo run --example network_interfaces   # Network OS telemetry (60 interfaces)
```

</details>

### Prerequisites

- **Rust 1.87+** &mdash; install via [rustup](https://rustup.rs/)
- No other dependencies needed &mdash; everything builds from source
- All examples use deterministic pseudo-random data, so results are reproducible across runs

---

<details>
<summary><strong>Examples at a Glance (40 examples)</strong></summary>

### Core

| # | Example | Difficulty | What You'll Learn |
|---|---------|-----------|-------------------|
| 1 | basic_store | Beginner | Create, insert, query, persist, reopen |
| 2 | progressive_index | Intermediate | Three-layer HNSW, recall measurement |
| 3 | quantization | Intermediate | Scalar/product/binary quantization, tiering |
| 4 | wire_format | Advanced | Raw segment I/O, hash validation, tail-scan |
| 5 | crypto_signing | Advanced | Ed25519 signing, witness chains, tamper detection |
| 6 | filtered_search | Intermediate | Metadata filters: Eq, Range, AND/OR/IN |

### Agentic AI

| # | Example | Difficulty | What You'll Learn |
|---|---------|-----------|-------------------|
| 7 | agent_memory | Intermediate | Persistent agent memory, session recall, witness audit |
| 8 | swarm_knowledge | Intermediate | Multi-agent shared knowledge, cross-agent search |
| 9 | reasoning_trace | Advanced | Chain-of-thought lineage (parent &rarr; child &rarr; grandchild) |
| 10 | tool_cache | Intermediate | Tool call caching, TTL, delete_by_filter, compaction |
| 11 | agent_handoff | Advanced | Transfer agent state, derive clone, lineage verification |
| 12 | experience_replay | Intermediate | RL replay buffer, priority sampling, tiering |

### Practical Production

| # | Example | Difficulty | What You'll Learn |
|---|---------|-----------|-------------------|
| 13 | semantic_search | Beginner | Document search engine, 4 filter workflows |
| 14 | recommendation | Intermediate | Collaborative filtering, genre/quality filters |
| 15 | rag_pipeline | Advanced | 5-step RAG: chunk, embed, retrieve, rerank, assemble |
| 16 | embedding_cache | Advanced | Zipf access patterns, 3-tier quantization, memory savings |
| 17 | dedup_detector | Intermediate | Near-duplicate detection, clustering, compaction |

### Vertical Domains

| # | Example | Difficulty | What You'll Learn |
|---|---------|-----------|-------------------|
| 18 | genomic_pipeline | Advanced | DNA k-mer search, `.rvdna` profile, lineage |
| 19 | financial_signals | Advanced | Market signals, Ed25519 signing, attestation |
| 20 | medical_imaging | Intermediate | Radiology search, `.rvvis` profile, audit trail |
| 21 | legal_discovery | Intermediate | Legal similarity, `.rvtext` profile, discovery audit |

### Exotic Capabilities

| # | Example | Difficulty | What You'll Learn |
|---|---------|-----------|-------------------|
| 22 | self_booting | Advanced | Embed/extract unikernel, kernel header verification |
| 23 | ebpf_accelerator | Advanced | Embed/extract eBPF, XDP program, co-existence |
| 24 | hyperbolic_taxonomy | Intermediate | Hierarchy-aware embeddings, depth-filtered search |
| 25 | multimodal_fusion | Intermediate | Cross-modal text+image search, modality filtering |
| 26 | sealed_engine | Advanced | Capstone: vectors + kernel + eBPF + witness + lineage |

### Runtime Targets + Postgres

| # | Example | Difficulty | What You'll Learn |
|---|---------|-----------|-------------------|
| 27 | browser_wasm | Intermediate | WASM-compatible API, raw wire segments, size targets |
| 28 | edge_iot | Beginner | Constrained device, binary quantization, memory budget |
| 29 | serverless_function | Intermediate | Cold start, manifest tail-scan, progressive loading |
| 30 | ruvllm_inference | Advanced | KV cache + LoRA adapters + policy store via RVF |
| 31 | postgres_bridge | Intermediate | PG export/import, offline query, lineage, witness audit |

### Network & Security

| # | Example | Difficulty | What You'll Learn |
|---|---------|-----------|-------------------|
| 32 | network_sync | Advanced | Peer-to-peer sync, vector exchange, conflict resolution |
| 33 | tee_attestation | Advanced | TEE platform attestation, sealed keys, computation proof |
| 34 | access_control | Intermediate | Role-based access, permission checks, audit trails |
| 35 | zero_knowledge | Advanced | ZK proofs for vector operations, privacy-preserving search |

### Autonomous Agent

| # | Example | Difficulty | What You'll Learn |
|---|---------|-----------|-------------------|
| 36 | ruvbot | Advanced | Autonomous agent with RVF memory, planning, tool use |

### POSIX & Systems

| # | Example | Difficulty | What You'll Learn |
|---|---------|-----------|-------------------|
| 37 | posix_fileops | Intermediate | Raw I/O, atomic rename, locking, segment random access |
| 38 | linux_microkernel | Advanced | Package management, SSH keys, kernel embed, lineage updates |
| 39 | mcp_in_rvf | Advanced | MCP server runtime embedded in RVF, eBPF filter, tools |

### Network Operations

| # | Example | Difficulty | What You'll Learn |
|---|---------|-----------|-------------------|
| 40 | network_interfaces | Intermediate | Multi-chassis telemetry, anomaly detection, filtered queries |

</details>

---

<details>
<summary><strong>Features Covered</strong></summary>

### Storage &mdash; vectors in, answers out

| Feature | Example | Description |
|---------|---------|-------------|
| k-NN Search | basic_store | Find nearest neighbors by L2 or cosine distance |
| Persistence | basic_store | Close a store, reopen it, verify results match |
| Metadata Filters | filtered_search | Eq, Ne, Gt, Lt, Range, In, And, Or expressions |
| Combined Filters | filtered_search | Multi-condition queries (category + score range) |

### Indexing &mdash; speed vs. accuracy trade-offs

| Feature | Example | Description |
|---------|---------|-------------|
| Progressive Indexing | progressive_index | Three-tier HNSW: Layer A (fast), B (better), C (best) |
| Recall Measurement | progressive_index | Compare approximate results against brute-force ground truth |

### Compression &mdash; fit more vectors in less memory

| Feature | Example | Description |
|---------|---------|-------------|
| Scalar Quantization | quantization | fp32 &rarr; u8 (4x compression, Hot tier) |
| Product Quantization | quantization | fp32 &rarr; PQ codes (8-32x compression, Warm tier) |
| Binary Quantization | quantization | fp32 &rarr; 1-bit (32x compression, Cold tier) |
| Temperature Tiering | quantization | Count-Min Sketch access tracking + automatic tier assignment |

### Wire format &mdash; what the bytes look like on disk and over the network

| Feature | Example | Description |
|---------|---------|-------------|
| Segment I/O | wire_format | Write/read 64-byte-aligned segments with type/flags/hash |
| Hash Validation | wire_format | CRC32c / XXH3 integrity checks on every segment |
| Tail-Scan | wire_format | Find latest manifest by scanning backward from EOF |

### Trust &mdash; signatures, audit trails, and tamper detection

| Feature | Example | Description |
|---------|---------|-------------|
| Ed25519 Signing | crypto_signing | Sign segments, verify signatures, detect tampering |
| Witness Chains | crypto_signing | SHAKE-256 linked audit trails (73-byte entries) |
| Tamper Detection | crypto_signing | Any byte flip breaks chain verification |

### Agentic AI &mdash; lineage, domains, and self-booting intelligence

| Feature | Example | Description |
|---------|---------|-------------|
| DNA-Style Lineage | (API) | Every derived file records its parent's hash and derivation type |
| Domain Profiles | (API) | `.rvdna`, `.rvtext`, `.rvgraph`, `.rvvis` &mdash; same format, domain-specific hints |
| Computational Container | `claude_code_appliance` | Embed a WASM microkernel, eBPF program, or bootable unikernel |
| Self-Booting Appliance | `claude_code_appliance` | 5.1 MB `.rvf` &mdash; boots Linux, serves queries, runs Claude Code |
| Import (JSON/CSV/NumPy) | (API) | Load embeddings from `.json`, `.csv`, or `.npy` files via `rvf-import` or `rvf ingest` CLI |
| Unified CLI | `rvf` | 9 subcommands: create, ingest, query, delete, status, inspect, compact, derive, serve |
| Compaction | (API) | Garbage-collect tombstoned vectors and reclaim disk space |
| Batch Delete | (API) | Delete vectors by ID with tombstone markers |

### Self-Booting RVF &mdash; Claude Code Appliance

The `claude_code_appliance` example builds a complete self-booting AI development environment as a single `.rvf` file. It uses real infrastructure &mdash; a Docker-built Linux kernel, Ed25519 SSH keys, a BPF C socket filter, and a cryptographic witness chain.

```bash
cd examples/rvf
cargo run --example claude_code_appliance
```

**What it produces** (5.1 MB file):

```
claude_code_appliance.rvf
  ├── KERNEL_SEG    Linux 6.8.12 bzImage (5.2 MB, x86_64)
  ├── EBPF_SEG      Socket filter — allows ports 2222, 8080 only
  ├── VEC_SEG       20 package embeddings (128-dim)
  ├── INDEX_SEG     HNSW graph for package search
  ├── WITNESS_SEG   6-entry tamper-evident audit trail
  ├── CRYPTO_SEG    3 Ed25519 SSH user keys (root, deploy, claude)
  ├── MANIFEST_SEG  4 KB root with segment directory
  └── Snapshot      v1 derived image with lineage tracking
```

**Boot and connect:**

```bash
rvf launch claude_code_appliance.rvf        # Boot on QEMU/Firecracker
ssh -p 2222 deploy@localhost                 # SSH in
curl -s localhost:8080/query -d '{"vector":[0.1,...], "k":5}'
```

Final file: **5.1 MB single `.rvf`** &mdash; boots Linux, serves queries, runs Claude Code.

</details>

<details>
<summary><strong>What RVF Contains</strong></summary>

An RVF file is built from **segments** &mdash; self-describing blocks that can be combined freely. Here are all 16 types, grouped by purpose:

```
 Data              Indexing           Compression        Runtime
+-----------+     +-----------+     +-----------+     +-----------+
| VEC  0x01 |     | INDEX 0x02|     | QUANT 0x06|     | WASM      |
| (vectors) |     | (HNSW)    |     | (SQ/PQ/BQ)|     | (5.5 KB)  |
+-----------+     +-----------+     +-----------+     +-----------+
| META 0x07 |     | META_IDX  |     | HOT  0x08 |     | KERNEL    |
| (key-val) |     | 0x0D      |     | (promoted) |     | 0x0E      |
+-----------+     +-----------+     +-----------+     +-----------+
| JOURNAL   |     | OVERLAY   |     | SKETCH    |     | EBPF      |
| 0x04      |     | 0x03      |     | 0x09      |     | 0x0F      |
+-----------+     +-----------+     +-----------+     +-----------+

 Trust             State              Domain
+-----------+     +-----------+     +-----------+
| WITNESS   |     | MANIFEST  |     | PROFILE   |
| 0x0A      |     | 0x05      |     | 0x0B      |
+-----------+     +-----------+     +-----------+
| CRYPTO    |
| 0x0C      |
+-----------+
```

Any segment you don't need is simply absent. A basic vector store uses VEC + INDEX + MANIFEST. A sealed cognitive engine might use all 16.

### RuVector Ecosystem Integration

RVF is the universal substrate for the entire RuVector ecosystem. Here's how the 75+ Rust crates map onto RVF segments:

| Domain | Crates | RVF Segments Used |
|--------|--------|-------------------|
| **LLM inference** | `ruvllm`, `ruvllm-cli` | VEC (KV cache), OVERLAY (LoRA), WITNESS (audit) |
| **Self-optimizing learning** | `sona` | OVERLAY (micro-LoRA), META (EWC++ weights) |
| **Graph neural networks** | `ruvector-gnn`, `ruvector-graph` | INDEX (HNSW topology), META (edge weights) |
| **Quantum computing** | `ruQu`, `ruqu-core`, `ruqu-algorithms` | SKETCH (VQE snapshots), META (syndrome tables) |
| **Attention mechanisms** | `ruvector-attention`, `ruvector-mincut-gated-transformer` | VEC (attention matrices), QUANT (INT4/FP16) |
| **Coherence systems** | `cognitum-gate-kernel`, `prime-radiant` | WITNESS (tile witnesses), WASM (64 KB tiles) |
| **Neuromorphic** | `ruvector-nervous-system`, `micro-hnsw-wasm` | VEC (spike trains), INDEX (spiking HNSW) |
| **Agent memory** | `agentdb`, `claude-flow`, `agentic-flow` | VEC + INDEX + WITNESS (full agent state) |
| **Edge / browser** | `rvlite`, `rvf-wasm` | VEC + INDEX via 5.5 KB WASM microkernel |
| **Hyperbolic geometry** | `ruvector-hyperbolic-hnsw`, `ruvector-math` | INDEX (Poincar&eacute; ball HNSW) |
| **Routing / inference** | `ruvector-tiny-dancer-core`, `ruvector-sparse-inference` | VEC (feature vectors), META (routing policies) |
| **Observation pipeline** | `ospipe` | META (state vectors), WITNESS (provenance) |

</details>

<details>
<summary><strong>Performance & Comparison</strong></summary>

RVF is designed for speed at every layer:

| Metric | Value | Example |
|--------|-------|---------|
| Cold boot (4 KB manifest) | **< 5 ms** | wire_format |
| First query (Layer A only) | **recall >= 0.70** | progressive_index |
| Full recall (Layer C) | **>= 0.95** | progressive_index |
| WASM binary size | **~5.5 KB** | &mdash; |
| Segment header | **64 bytes** | wire_format |
| Witness chain entry | **73 bytes** | crypto_signing |
| Scalar quantization | **4x compression** | quantization |
| Product quantization | **8-32x compression** | quantization |
| Binary quantization | **32x compression** | quantization |

### Progressive Loading

Instead of waiting for the full index, RVF serves queries immediately:

```
Layer A ─────> Layer B ─────> Layer C
(microsecs)    (~10 ms)       (~50 ms)
recall ~0.70   recall ~0.85   recall ~0.95
```

The `progressive_index` example measures this recall progression with brute-force ground truth.

### Comparison

#### vs. vector databases

| Feature | RVF | Annoy | FAISS | Qdrant | Milvus |
|---------|-----|-------|-------|--------|--------|
| Single-file format | Yes | Yes | No | No | No |
| Crash-safe (no WAL) | Yes | No | No | WAL | WAL |
| Progressive loading | 3 layers | No | No | No | No |
| WASM support | 5.5 KB | No | No | No | No |
| `no_std` compatible | Yes | No | No | No | No |
| Post-quantum sigs | ML-DSA-65 | No | No | No | No |
| TEE attestation | Yes | No | No | No | No |
| Metadata filtering | Yes | No | Yes | Yes | Yes |
| Auto quantization | 3-tier | No | Manual | Yes | Yes |
| Append-only | Yes | Build-once | Build-once | Log | Log |
| Witness chains | Yes | No | No | No | No |
| Lineage provenance | Yes (DNA-style) | No | No | No | No |
| Computational container | Yes (WASM/eBPF/unikernel) | No | No | No | No |
| Domain profiles | 5 profiles | No | No | No | No |
| Language bindings | Rust, Node, WASM | C++, Python | C++, Python | Rust, Python | Go, Python |

#### vs. model registries, graph DBs, and container formats

RVF replaces multiple tools because it carries data, model, graph, runtime, and trust chain together:

| Capability | RVF | GGUF | ONNX | SafeTensors | Neo4j | Docker/OCI |
|-----------|-----|------|------|-------------|-------|------------|
| Vector storage + search | Yes | No | No | No | No | No |
| Model weight deltas (LoRA) | OVERLAY_SEG | Full weights | Full graph | Weights only | No | No |
| Graph neural state | GRAPH_SEG | No | No | No | Yes | No |
| Cryptographic audit trail | WITNESS_SEG | No | No | No | No | No |
| Self-booting runtime | KERNEL_SEG | No | No | No | No | Yes |
| Kernel-level acceleration | EBPF_SEG | No | No | No | No | No |
| File lineage / versioning | DNA-style | No | No | No | No | Image layers |
| TEE attestation | Built-in | No | No | No | No | No |
| Single portable file | Yes | Yes | Yes | Yes | No | Image tarball |
| Runs in browser | 5.5 KB WASM | No | ONNX.js | No | No | No |

</details>

<details>
<summary><strong>Usage Patterns (8 patterns)</strong></summary>

### Pattern 1: Simple Vector Store

The most common use case. Create a store, add embeddings, query nearest neighbors.

```rust
use rvf_runtime::{RvfStore, RvfOptions, QueryOptions};
use rvf_runtime::options::DistanceMetric;

let options = RvfOptions {
    dimension: 384,
    metric: DistanceMetric::L2,
    ..Default::default()
};
let mut store = RvfStore::create("vectors.rvf", options)?;

// Insert embeddings
store.ingest_batch(&[&embedding], &[1], None)?;

// Query top-10 nearest neighbors
let results = store.query(&query, 10, &QueryOptions::default())?;
for r in &results {
    println!("id={}, distance={:.4}", r.id, r.distance);
}
```

See: [`basic_store.rs`](examples/basic_store.rs)

### Pattern 2: Filtered Search

Attach metadata to vectors, then filter during queries.

```rust
use rvf_runtime::{FilterExpr, MetadataEntry, MetadataValue};
use rvf_runtime::filter::FilterValue;

// Add metadata during ingestion
let metadata = vec![
    MetadataEntry { field_id: 0, value: MetadataValue::String("science".into()) },
    MetadataEntry { field_id: 1, value: MetadataValue::U64(95) },
];
store.ingest_batch(&[&vec], &[42], Some(&metadata))?;

// Query with filter: category == "science" AND score > 80
let filter = FilterExpr::And(vec![
    FilterExpr::Eq(0, FilterValue::String("science".into())),
    FilterExpr::Gt(1, FilterValue::U64(80)),
]);
let opts = QueryOptions { filter: Some(filter), ..Default::default() };
let results = store.query(&query, 10, &opts)?;
```

See: [`filtered_search.rs`](examples/filtered_search.rs)

### Pattern 3: Progressive Recall

Start serving queries instantly, improve quality as more data loads.

```rust
use rvf_index::{build_full_index, build_layer_a, build_layer_c, ProgressiveIndex};

// Build HNSW graph
let graph = build_full_index(&store, n, &config, &rng, &l2_distance);

// Layer A: instant but approximate
let layer_a = build_layer_a(&graph, &centroids, &assignments, n as u64);
let idx = ProgressiveIndex { layer_a: Some(layer_a), layer_b: None, layer_c: None };
let fast_results = idx.search(&query, 10, 200, &store); // recall ~0.70

// Layer C: full precision
let layer_c = build_layer_c(&graph);
let idx_full = ProgressiveIndex { layer_a: Some(layer_a), layer_b: None, layer_c: Some(layer_c) };
let precise_results = idx_full.search(&query, 10, 200, &store); // recall ~0.95
```

See: [`progressive_index.rs`](examples/progressive_index.rs)

### Pattern 4: Cryptographic Integrity

Sign segments and build tamper-evident audit trails.

```rust
use rvf_crypto::{sign_segment, verify_segment, create_witness_chain, WitnessEntry, shake256_256};
use ed25519_dalek::SigningKey;

// Sign a segment
let footer = sign_segment(&header, &payload, &signing_key);

// Verify signature
assert!(verify_segment(&header, &payload, &footer, &verifying_key));

// Build an audit trail
let entries = vec![WitnessEntry {
    prev_hash: [0; 32],
    action_hash: shake256_256(b"inserted 1000 vectors"),
    timestamp_ns: 1_700_000_000_000_000_000,
    witness_type: 0x01, // PROVENANCE
}];
let chain = create_witness_chain(&entries);
```

See: [`crypto_signing.rs`](examples/crypto_signing.rs)

### Pattern 5: Import from JSON / CSV / NumPy

Load embeddings from common formats without writing a parser.

```rust
use rvf_import::{import_json, import_csv, import_npy};

// From a JSON array of vectors
import_json("embeddings.json", &mut store)?;

// From a CSV file (one vector per row)
import_csv("embeddings.csv", &mut store)?;

// From a NumPy .npy file
import_npy("embeddings.npy", &mut store)?;
```

### Pattern 6: Delete and Compact

Remove vectors by ID, then reclaim disk space.

```rust
// Delete specific vectors (marks as tombstones)
store.delete_batch(&[42, 99, 1001])?;

// Compact: rewrite the file without tombstoned data
store.compact()?;
```

### Pattern 7: File Lineage (Parent &rarr; Child Derivation)

Create derived files that track their ancestry.

```rust
use rvf_types::DerivationType;

// Create a parent store
let parent = RvfStore::create("parent.rvf", options)?;

// Derive a filtered child — records parent's hash automatically
let child = parent.derive("child.rvf", DerivationType::Filter, None)?;
assert_eq!(child.lineage_depth(), 1);
assert_eq!(child.parent_id(), parent.file_id());

// Derive a grandchild
let grandchild = child.derive("grandchild.rvdna", DerivationType::Quantize, None)?;
assert_eq!(grandchild.lineage_depth(), 2);
```

### Pattern 8: Embed a Computational Container

Pack a bootable kernel or eBPF program into the file.

```rust
use rvf_types::kernel::{KernelArch, KernelType};
use rvf_types::ebpf::{EbpfProgramType, EbpfAttachType};

// Embed a unikernel — file can now boot as a standalone service
store.embed_kernel(KernelArch::X86_64, KernelType::HermitOs, &kernel_image, 8080)?;

// Embed an eBPF program — enables kernel-level acceleration
store.embed_ebpf(EbpfProgramType::Xdp, EbpfAttachType::XdpIngress, 384, &bytecode, &btf)?;

// Extract later
let (hdr, img) = store.extract_kernel()?.unwrap();
let (hdr, prog) = store.extract_ebpf()?.unwrap();
```

</details>

<details>
<summary><strong>Tutorial: Your First RVF Store (Step by Step)</strong></summary>

### Step 1: Set Up

Create a new Rust project and add the dependency:

```bash
cargo new my_vectors
cd my_vectors
```

Add to `Cargo.toml`:

```toml
[dependencies]
rvf-runtime = { path = "../crates/rvf/rvf-runtime" }
tempfile = "3"
```

### Step 2: Create a Store

```rust
use rvf_runtime::{RvfStore, RvfOptions, QueryOptions};
use rvf_runtime::options::DistanceMetric;
use tempfile::TempDir;

fn main() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("my.rvf");

    let opts = RvfOptions {
        dimension: 128,
        metric: DistanceMetric::L2,
        ..Default::default()
    };
    let mut store = RvfStore::create(&path, opts).unwrap();
```

### Step 3: Insert Vectors

Vectors are inserted in batches. Each vector needs a unique `u64` ID.

```rust
    let vec_a = vec![0.1f32; 128];
    let vec_b = vec![0.2f32; 128];
    let vecs: Vec<&[f32]> = vec![&vec_a, &vec_b];
    let ids = vec![1u64, 2];

    let result = store.ingest_batch(&vecs, &ids, None).unwrap();
    println!("Accepted: {}, Rejected: {}", result.accepted, result.rejected);
```

### Step 4: Query

```rust
    let query = vec![0.15f32; 128];
    let results = store.query(&query, 5, &QueryOptions::default()).unwrap();

    for r in &results {
        println!("  id={}, dist={:.6}", r.id, r.distance);
    }
```

### Step 5: Verify Persistence

```rust
    store.close().unwrap();

    let reopened = RvfStore::open(&path).unwrap();
    let results2 = reopened.query(&query, 5, &QueryOptions::default()).unwrap();
    assert_eq!(results.len(), results2.len());
    println!("Persistence verified!");
}
```

### Expected Output

```
Accepted: 2, Rejected: 0
  id=1, dist=0.064000
  id=2, dist=0.032000
Persistence verified!
```

</details>

<details>
<summary><strong>Tutorial: Understanding Quantization Tiers</strong></summary>

### The Problem

A million 384-dim vectors at full precision (fp32) takes **1.5 GB** of RAM. Not all vectors are accessed equally &mdash; most are rarely touched. Why keep them all at full precision?

### The Solution: Temperature Tiering

RVF assigns vectors to three compression levels based on how often they're accessed:

| Tier | Access Pattern | Compression | Memory per Vector (384d) |
|------|---------------|------------|--------------------------|
| **Hot** | Frequently queried | Scalar (fp32 -> u8) | 384 bytes (4x smaller) |
| **Warm** | Occasionally queried | Product quantization | 48 bytes (32x smaller) |
| **Cold** | Rarely accessed | Binary (1-bit) | 48 bytes (32x smaller) |
| Raw | No compression | fp32 | 1,536 bytes |

### How It Works

**1. Track access patterns** using a Count-Min Sketch (a probabilistic counter):

```rust
let mut sketch = CountMinSketch::default_sketch();

// Every time a vector is accessed, increment its counter
sketch.increment(vector_id);

// Check how often a vector has been accessed
let count = sketch.estimate(vector_id);
```

**2. Assign tiers** based on configurable thresholds:

```rust
let tier = assign_tier(count);
// Hot:  count >= 100
// Warm: count >= 10
// Cold: count < 10
```

**3. Encode at the appropriate level:**

```rust
// Hot: Scalar (fast, low error)
let sq = ScalarQuantizer::train(&vectors);
let encoded = sq.encode_vec(&vector);  // 384 bytes

// Warm: Product (balanced)
let pq = ProductQuantizer::train(&vectors, 48, 64, 20);
let encoded = pq.encode_vec(&vector);  // 48 bytes

// Cold: Binary (smallest, approximate)
let bits = encode_binary(&vector);     // 48 bytes
```

### Run the Example

```bash
cargo run --example quantization
```

You'll see a comparison table showing compression ratio, reconstruction error (MSE), and bytes per vector for each tier.

</details>

<details>
<summary><strong>Tutorial: Building Witness Chains for Audit Trails</strong></summary>

### What Is a Witness Chain?

A witness chain is a tamper-evident log of events. Each entry links to the previous one through a cryptographic hash. If any entry is modified, all subsequent hash links break &mdash; making tampering detectable without a blockchain.

### Chain Structure

```
  Entry 0 (genesis)         Entry 1                  Entry 2
+-------------------+   +-------------------+   +-------------------+
| prev_hash: 0x00.. |   | prev_hash: H(E0)  |   | prev_hash: H(E1)  |
| action:   H(data) |   | action:   H(data) |   | action:   H(data) |
| timestamp: T0     |   | timestamp: T1     |   | timestamp: T2     |
| type: PROVENANCE  |   | type: COMPUTATION |   | type: SEARCH      |
+-------------------+   +-------------------+   +-------------------+
        73 bytes                73 bytes                73 bytes
```

- **prev_hash**: SHAKE-256 hash of the previous entry (zeroed for genesis)
- **action_hash**: SHAKE-256 hash of whatever action is being recorded
- **timestamp_ns**: Nanosecond UNIX timestamp
- **witness_type**: What kind of event (see table below)

### Witness Types

| Code | Name | When to Use |
|------|------|------------|
| `0x01` | PROVENANCE | Data origin tracking (e.g., "loaded from model X") |
| `0x02` | COMPUTATION | Operation recording (e.g., "built HNSW index") |
| `0x03` | SEARCH | Query audit (e.g., "searched for query Q, got results R") |
| `0x04` | DELETION | Deletion audit (e.g., "deleted vectors 1-100") |
| `0x05` | PLATFORM_ATTESTATION | TEE attestation (e.g., "enclave measured as M") |
| `0x06` | KEY_BINDING | Sealed key (e.g., "key K bound to enclave M") |
| `0x07` | COMPUTATION_PROOF | Verified computation (e.g., "search ran inside enclave") |
| `0x08` | DATA_PROVENANCE | Full chain (e.g., "model -> TEE -> RVF file") |
| `0x09` | DERIVATION | File lineage derivation event |
| `0x0A` | LINEAGE_MERGE | Multi-parent lineage merge |
| `0x0B` | LINEAGE_SNAPSHOT | Lineage snapshot checkpoint |
| `0x0C` | LINEAGE_TRANSFORM | Lineage transform operation |
| `0x0D` | LINEAGE_VERIFY | Lineage verification event |

### Creating and Verifying

```rust
use rvf_crypto::{create_witness_chain, verify_witness_chain, WitnessEntry, shake256_256};

// Record three events
let entries = vec![
    WitnessEntry {
        prev_hash: [0; 32], // genesis
        action_hash: shake256_256(b"loaded embeddings from model-v2"),
        timestamp_ns: 1_700_000_000_000_000_000,
        witness_type: 0x01,
    },
    WitnessEntry {
        prev_hash: [0; 32], // filled by create_witness_chain
        action_hash: shake256_256(b"built HNSW index (M=16, ef=200)"),
        timestamp_ns: 1_700_000_001_000_000_000,
        witness_type: 0x02,
    },
    WitnessEntry {
        prev_hash: [0; 32],
        action_hash: shake256_256(b"query: top-10 for user request #42"),
        timestamp_ns: 1_700_000_002_000_000_000,
        witness_type: 0x03,
    },
];

let chain_bytes = create_witness_chain(&entries);
let verified = verify_witness_chain(&chain_bytes).unwrap();
assert_eq!(verified.len(), 3);
```

### Tamper Detection

Flip any byte in the chain and verification fails:

```rust
let mut tampered = chain_bytes.clone();
tampered[100] ^= 0xFF; // flip one byte

assert!(verify_witness_chain(&tampered).is_err()); // detected!
```

### Run the Example

```bash
cargo run --example crypto_signing
```

The example creates a 5-entry chain, verifies it, then demonstrates tamper and truncation detection.

</details>

<details>
<summary><strong>Tutorial: Wire Format Deep Dive</strong></summary>

### Segment Header (64 bytes)

Every piece of data in an RVF file is wrapped in a self-describing segment. The header is always exactly 64 bytes:

```
Offset  Size  Field             Description
------  ----  -----             -----------
0x00    4     magic             0x52564653 ("RVFS")
0x04    1     version           Format version (currently 1)
0x05    1     seg_type          Segment type (VEC, INDEX, MANIFEST, ...)
0x06    2     flags             Bitfield (COMPRESSED, SIGNED, ATTESTED, ...)
0x08    8     segment_id        Monotonically increasing ID
0x10    8     payload_length    Byte length of payload
0x18    8     timestamp_ns      Nanosecond UNIX timestamp
0x20    1     checksum_algo     0=CRC32C, 1=XXH3-128, 2=SHAKE-256
0x21    1     compression       0=none, 1=LZ4, 2=ZSTD
0x22    2     reserved_0        Must be zero
0x24    4     reserved_1        Must be zero
0x28    16    content_hash      First 128 bits of payload hash
0x38    4     uncompressed_len  Original size before compression
0x3C    4     alignment_pad     Padding to 64-byte boundary
```

### The 16 Segment Types

| Code | Name | Purpose |
|------|------|---------|
| `0x01` | VEC | Raw vector embeddings |
| `0x02` | INDEX | HNSW adjacency and routing tables |
| `0x03` | OVERLAY | Graph overlay deltas |
| `0x04` | JOURNAL | Metadata mutations, deletions |
| `0x05` | MANIFEST | Segment directory, epoch state |
| `0x06` | QUANT | Quantization dictionaries (scalar/PQ/binary) |
| `0x07` | META | Key-value metadata |
| `0x08` | HOT | Temperature-promoted data |
| `0x09` | SKETCH | Access counter sketches (Count-Min) |
| `0x0A` | WITNESS | Audit trails, attestation proofs |
| `0x0B` | PROFILE | Domain profile declarations |
| `0x0C` | CRYPTO | Key material, signature chains |
| `0x0D` | META_IDX | Metadata inverted indexes |
| `0x0E` | KERNEL | Compressed unikernel image (self-booting) |
| `0x0F` | EBPF | eBPF program for kernel-level acceleration |

### Segment Flags

| Bit | Name | Description |
|-----|------|-------------|
| 0 | COMPRESSED | Payload is compressed (LZ4 or ZSTD) |
| 1 | ENCRYPTED | Payload is encrypted |
| 2 | SIGNED | Signature footer follows payload |
| 3 | SEALED | Immutable (compaction output) |
| 4 | PARTIAL | Streaming / partial write |
| 5 | TOMBSTONE | Logical deletion marker |
| 6 | HOT | Temperature-promoted |
| 7 | OVERLAY | Contains delta data |
| 8 | SNAPSHOT | Full snapshot |
| 9 | CHECKPOINT | Safe rollback point |
| 10 | ATTESTED | Produced inside attested TEE |
| 11 | HAS_LINEAGE | File carries FileIdentity lineage data |

### Crash Safety: Two-fsync Protocol

RVF doesn't need a write-ahead log. Instead:

1. Write data segment + payload, then `fsync`
2. Write MANIFEST_SEG with updated state, then `fsync`

If the process crashes between fsyncs, the incomplete segment has no manifest reference &mdash; it's ignored on recovery. Simple, safe, fast.

### Tail-Scan

To find the current state, scan backward from the end of the file for the latest MANIFEST_SEG. The root manifest fits in 4 KB, so cold boot takes < 5 ms.

### Run the Example

```bash
cargo run --example wire_format
```

You'll see three segments written, read back, hash-validated, corruption detected, and a tail-scan for the manifest.

</details>

<details>
<summary><strong>Tutorial: Metadata Filtering Patterns</strong></summary>

### Available Filter Expressions

| Expression | Syntax | Description |
|-----------|--------|-------------|
| `Eq` | `FilterExpr::Eq(field_id, value)` | Exact match |
| `Ne` | `FilterExpr::Ne(field_id, value)` | Not equal |
| `Gt` | `FilterExpr::Gt(field_id, value)` | Greater than |
| `Lt` | `FilterExpr::Lt(field_id, value)` | Less than |
| `Range` | `FilterExpr::Range(field_id, low, high)` | Value in [low, high) |
| `In` | `FilterExpr::In(field_id, values)` | Value is one of |
| `And` | `FilterExpr::And(vec![...])` | All conditions must match |
| `Or` | `FilterExpr::Or(vec![...])` | Any condition matches |

### Metadata Types

| Type | Rust | Use Case |
|------|------|----------|
| `String` | `MetadataValue::String("cat".into())` | Categories, labels, tags |
| `U64` | `MetadataValue::U64(95)` | Scores, counts, timestamps |
| `Bytes` | `MetadataValue::Bytes(vec![...])` | Binary data, hashes |

### Common Patterns

**Category filter:**
```rust
FilterExpr::Eq(0, FilterValue::String("science".into()))
```

**Score range:**
```rust
FilterExpr::Range(1, FilterValue::U64(30), FilterValue::U64(90))
```

**Multi-category:**
```rust
FilterExpr::In(0, vec![
    FilterValue::String("science".into()),
    FilterValue::String("tech".into()),
])
```

**Combined (AND):**
```rust
FilterExpr::And(vec![
    FilterExpr::Eq(0, FilterValue::String("science".into())),
    FilterExpr::Gt(1, FilterValue::U64(80)),
])
```

### Run the Example

```bash
cargo run --example filtered_search
```

The example creates 500 vectors with category and score metadata, then runs 7 different filter queries showing selectivity and verification.

</details>

<details>
<summary><strong>Tutorial: Progressive Index Recall Measurement</strong></summary>

### What Is Recall?

**Recall@K** measures how many of the true K nearest neighbors your approximate algorithm actually returns. A recall of 0.95 means 95% of results are correct.

```
recall@K = |approximate_results ∩ exact_results| / K
```

### How Progressive Indexing Achieves This

RVF builds an HNSW (Hierarchical Navigable Small World) graph, then splits it into three loadable layers:

**Layer A: Coarse Routing**
- Entry points (topmost HNSW nodes)
- Partition centroids for guided search
- Loads in microseconds
- Recall: ~0.40-0.70

**Layer B: Hot Region**
- Adjacency lists for the most frequently accessed vectors
- Covers the "working set" of your data
- Recall: ~0.70-0.85

**Layer C: Full Graph**
- Complete HNSW adjacency for all vectors
- Loaded in background while queries are already being served
- Recall: >= 0.95

### Measuring Recall in the Example

The `progressive_index` example:
1. Generates 5,000 vectors (128 dims)
2. Builds the full HNSW graph (M=16, ef_construction=200)
3. Splits into Layer A, B, C
4. Runs 50 queries at each stage
5. Computes recall@10 against brute-force ground truth

```bash
cargo run --example progressive_index
```

Expected output:

```
=== Recall Progression Summary ===
        Layers  Recall@10
  A only         0.xxx
  A + B          0.xxx
  A + B + C      0.9xx
```

### Tuning ef_search

The `ef_search` parameter controls how many candidates HNSW explores during search. Higher values improve recall at the cost of latency:

| ef_search | Recall@10 | Relative Speed |
|-----------|-----------|---------------|
| 10 | ~0.75 | Fastest |
| 50 | ~0.90 | Balanced |
| 200 | ~0.97 | Most accurate |

</details>

<details>
<summary><strong>Technical Reference: Signature Footer Format</strong></summary>

When the `SIGNED` flag is set on a segment, a signature footer follows the payload:

| Offset | Size | Field |
|--------|------|-------|
| 0x00 | 2 | `sig_algo` (0=Ed25519, 1=ML-DSA-65, 2=SLH-DSA-128s) |
| 0x02 | 2 | `sig_length` |
| 0x04 | var | `signature` (64 to 7,856 bytes) |
| var | 4 | `footer_length` (for backward scan) |

### Supported Algorithms

| Algorithm | Signature Size | Security Level | Standard |
|-----------|---------------|---------------|----------|
| Ed25519 | 64 bytes | 128-bit classical | RFC 8032 |
| ML-DSA-65 | 3,309 bytes | NIST Level 3 (post-quantum) | FIPS 204 |
| SLH-DSA-128s | 7,856 bytes | NIST Level 1 (post-quantum, stateless) | FIPS 205 |

### Signing Flow

1. Serialize the segment header (64 bytes) and payload into a signing buffer
2. Compute SHAKE-256 hash of the buffer
3. Sign the hash with the chosen algorithm
4. Append the signature footer after the payload (before padding)
5. Set the `SIGNED` flag in the header

### Verification Flow

1. Read segment header and payload
2. Recompute SHAKE-256 hash of header + payload
3. Read signature footer (scan backward from segment end using `footer_length`)
4. Verify signature against the public key

</details>

<details>
<summary><strong>Technical Reference: Confidential Core Attestation</strong></summary>

### Overview

RVF can record hardware TEE (Trusted Execution Environment) attestation quotes alongside vector data. This provides cryptographic proof that:

- The platform is genuine (e.g., real Intel SGX hardware)
- The code running inside the enclave matches a known measurement
- Encryption keys are sealed to the enclave identity
- Vector operations were computed inside the secure environment

### Supported TEE Platforms

| Platform | Enum Value | Quote Format |
|----------|-----------|--------------|
| Intel SGX | `TeePlatform::Sgx` (0) | DCAP attestation quote |
| AMD SEV-SNP | `TeePlatform::SevSnp` (1) | VCEK attestation report |
| Intel TDX | `TeePlatform::Tdx` (2) | TD quote |
| ARM CCA | `TeePlatform::ArmCca` (3) | CCA token |
| Software (testing) | `TeePlatform::SoftwareTee` (0xFE) | Synthetic (no hardware) |

### Attestation Header (112 bytes, `repr(C)`)

```
Offset  Size  Field
------  ----  -----
0x00    1     platform           TeePlatform enum value
0x01    1     attestation_type   AttestationWitnessType enum value
0x02    4     quote_length       Length of the platform-specific quote
0x06    2     reserved
0x08    32    measurement        SHAKE-256 hash of enclave code
0x28    32    signer_id          SHAKE-256 hash of signing identity
0x48    8     timestamp_ns       Nanosecond UNIX timestamp
0x50    16    nonce              Anti-replay nonce
0x60    2     svn                Security Version Number
0x62    1     sig_algo           Signature algorithm for the quote
0x63    1     flags              Attestation flags
0x64    4     report_data_len    Length of additional report data
0x68    8     reserved
```

### Attestation Types

| Type | Witness Code | Purpose |
|------|-------------|---------|
| Platform Attestation | `0x05` | TEE identity + measurement verification |
| Key Binding | `0x06` | Keys sealed to enclave measurement |
| Computation Proof | `0x07` | Proof that operations ran inside enclave |
| Data Provenance | `0x08` | Full chain: model -> TEE -> RVF file |

### ATTESTED Segment Flag

Any segment produced inside a TEE should set bit 10 (`ATTESTED`) in the segment header flags. This enables fast scanning to identify attested segments without parsing payloads.

### QuoteVerifier Trait

The verification interface is pluggable:

```rust
pub trait QuoteVerifier {
    fn platform(&self) -> TeePlatform;
    fn verify_quote(
        &self,
        quote: &[u8],
        report_data: &[u8],
        expected_measurement: &[u8; 32],
    ) -> Result<(), String>;
}
```

Implement this trait for your TEE platform to enable hardware-backed verification. The `SoftwareTee` variant allows testing without real hardware.

</details>

<details>
<summary><strong>Technical Reference: Computational Container (Self-Booting RVF)</strong></summary>

### Three-Tier Execution Model

RVF files can optionally carry executable compute alongside vector data:

| Tier | Segment | Size | Environment | Boot Time | Use Case |
|------|---------|------|-------------|-----------|----------|
| **1: WASM** | WASM_SEG (existing) | 5.5 KB | Browser, edge, IoT | <1 ms | Portable queries everywhere |
| **2: eBPF** | EBPF_SEG (`0x0F`) | 10-50 KB | Linux kernel (XDP, TC) | <20 ms | Sub-microsecond hot cache hits |
| **3: Unikernel** | KERNEL_SEG (`0x0E`) | 200 KB - 2 MB | Firecracker, TEE, bare metal | <125 ms | Zero-dependency self-booting service |

### KernelHeader (128 bytes)

| Field | Size | Description |
|-------|------|-------------|
| `kernel_magic` | 4 | `0x52564B4E` ("RVKN") |
| `header_version` | 2 | Currently 1 |
| `kernel_arch` | 1 | x86_64 (0), AArch64 (1), RISC-V (2), WASM (3) |
| `kernel_type` | 1 | HermitOS (0), Unikraft (1), Custom (2), TestStub (0xFE) |
| `image_size` | 4 | Uncompressed kernel size |
| `compressed_size` | 4 | Compressed (ZSTD) size |
| `image_hash` | 32 | SHAKE-256-256 of uncompressed image |
| `api_port` | 2 | HTTP API port (network byte order) |
| `api_transport` | 1 | HTTP (0), gRPC (1), virtio-vsock (2) |
| `kernel_flags` | 8 | Feature flags (read-only, metrics, TEE, etc.) |
| `cmdline_len` | 2 | Length of kernel command line |

### EbpfHeader (64 bytes)

| Field | Size | Description |
|-------|------|-------------|
| `ebpf_magic` | 4 | `0x52564250` ("RVBP") |
| `program_type` | 1 | XDP (0), TC (1), Tracepoint (2), Socket (3) |
| `attach_type` | 1 | XdpIngress (0), TcIngress (1), etc. |
| `max_dimension` | 4 | Maximum vector dimension (eBPF verifier loop bound) |
| `bytecode_size` | 4 | Size of BPF ELF object |
| `btf_size` | 4 | Size of BTF section |
| `map_count` | 4 | Number of BPF maps |

### Embedding and Extracting

```rust
use rvf_runtime::RvfStore;
use rvf_types::kernel::{KernelArch, KernelType};
use rvf_types::ebpf::{EbpfProgramType, EbpfAttachType};

let mut store = RvfStore::open("vectors.rvf")?;

// Embed a kernel
store.embed_kernel(KernelArch::X86_64, KernelType::HermitOs, &image, 8080)?;

// Embed an eBPF program
store.embed_ebpf(EbpfProgramType::Xdp, EbpfAttachType::XdpIngress, 384, &bytecode, &btf)?;

// Extract later
let (kernel_hdr, kernel_img) = store.extract_kernel()?.unwrap();
let (ebpf_hdr, ebpf_prog) = store.extract_ebpf()?.unwrap();
```

### Forward Compatibility

Files with KERNEL_SEG or EBPF_SEG work with older readers -- unknown segment types are skipped per the RVF forward-compatibility rule. The computational capability is purely additive.

See [ADR-030](../../docs/adr/ADR-030-rvf-computational-container.md) for the full specification.

</details>

<details>
<summary><strong>Technical Reference: DNA-Style Lineage Provenance</strong></summary>

### How Lineage Works

Every RVF file carries a 68-byte `FileIdentity` in its root manifest:

| Field | Size | Description |
|-------|------|-------------|
| `file_id` | 16 | Unique UUID for this file |
| `parent_id` | 16 | UUID of the parent file (all zeros for root) |
| `parent_hash` | 32 | SHAKE-256-256 of parent's manifest |
| `lineage_depth` | 4 | Generation count (0 for root) |

### Derivation Chain

```
Parent.rvf ──derive()──> Child.rvf ──derive()──> Grandchild.rvdna
  file_id: A               file_id: B               file_id: C
  parent_id: [0;16]         parent_id: A              parent_id: B
  parent_hash: [0;32]       parent_hash: hash(A)      parent_hash: hash(B)
  depth: 0                  depth: 1                  depth: 2
```

### Derivation Types

| Code | Type | Description |
|------|------|-------------|
| 0 | Clone | Exact copy |
| 1 | Filter | Subset of parent's vectors |
| 2 | Merge | Multi-parent merge |
| 3 | Quantize | Re-quantized version |
| 4 | Reindex | Re-indexed with different parameters |
| 5 | Transform | Transformed embeddings |
| 6 | Snapshot | Point-in-time snapshot |
| 0xFF | UserDefined | Application-specific derivation |

### Using the API

```rust
use rvf_runtime::RvfStore;
use rvf_types::DerivationType;

let parent = RvfStore::create("parent.rvf", options)?;

// Derive a filtered child
let child = parent.derive("child.rvf", DerivationType::Filter, None)?;
assert_eq!(child.lineage_depth(), 1);
assert_eq!(child.parent_id(), parent.file_id());
```

### Domain Extensions

| Extension | Domain Profile | Optimized For |
|-----------|---------------|---------------|
| `.rvf` | Generic | General-purpose vectors |
| `.rvdna` | RVDNA | Genomic sequence embeddings |
| `.rvtext` | RVText | Language model embeddings |
| `.rvgraph` | RVGraph | Graph/network node embeddings |
| `.rvvis` | RVVision | Image/vision model embeddings |

See [ADR-029](../../docs/adr/ADR-029-rvf-canonical-format.md) for the full format specification.

</details>

<details>
<summary><strong>Technical Reference: Crate Architecture</strong></summary>

### Crate Map

```
                    +-----------------------------------------+
                    |         Cognitive Layer                   |
                    |  ruvllm | gnn | ruQu | attention | sona  |
                    |  mincut | prime-radiant | nervous-system |
                    +---+-------------+---------------+-------+
                        |             |               |
                    +-----------------------------------------+
                    |           Application Layer              |
                    |  claude-flow | agentdb | agentic-flow    |
                    |  ospipe | rvlite | sona | your-app      |
                    +---+-------------+---------------+-------+
                        |             |               |
                    +---v-------------v---------------v-------+
                    |           RVF SDK Layer                   |
                    |  rvf-runtime | rvf-index | rvf-quant      |
                    |  rvf-manifest | rvf-crypto | rvf-wire     |
                    +---+-------------+---------------+-------+
                        |             |               |
               +--------v------+ +---v--------+ +----v-------+ +----v------+
               |  rvf-server   | |  rvf-node  | |  rvf-wasm  | |  rvf-cli  |
               |  HTTP + TCP   | |  N-API     | |  ~46 KB    | |  clap     |
               +---------------+ +------------+ +------------+ +-----------+
```

### Crate Details

| Crate | Lines | no_std | Purpose |
|-------|------:|:------:|---------|
| `rvf-types` | 3,184 | Yes | Segment types, kernel/eBPF headers, lineage, enums |
| `rvf-wire` | 2,011 | Yes | Wire format read/write, hash validation |
| `rvf-manifest` | 1,580 | No | Two-level manifest with 4 KB root, FileIdentity codec |
| `rvf-index` | 2,691 | No | HNSW progressive indexing (Layer A/B/C) |
| `rvf-quant` | 1,443 | No | Scalar, product, and binary quantization |
| `rvf-crypto` | 1,725 | Partial | SHAKE-256, Ed25519, witness chains, attestation, lineage |
| `rvf-runtime` | 3,607 | No | Full store API, compaction, lineage, kernel/eBPF embed |
| `rvf-import` | 980 | No | JSON, CSV, NumPy (.npy) importers |
| `rvf-wasm` | 1,616 | Yes | WASM control plane: in-memory store, query, segment inspection |
| `rvf-node` | 852 | No | Node.js N-API bindings with lineage, kernel/eBPF, inspection |
| `rvf-cli` | 665 | No | Unified CLI: create, ingest, query, delete, status, inspect, compact, derive, serve |
| `rvf-server` | 1,165 | No | HTTP REST + TCP streaming server |

### Library Adapters

| Adapter | Purpose | Key Feature |
|---------|---------|-------------|
| `rvf-adapter-claude-flow` | AI agent memory | WITNESS_SEG audit trails |
| `rvf-adapter-agentdb` | Agent vector database | Progressive HNSW indexing |
| `rvf-adapter-ospipe` | Observation-State pipeline | META_SEG for state vectors |
| `rvf-adapter-agentic-flow` | Swarm coordination | Inter-agent memory sharing |
| `rvf-adapter-rvlite` | Lightweight embedded store | Minimal API, edge-friendly |
| `rvf-adapter-sona` | Neural architecture | Experience replay + trajectories |

</details>

<details>
<summary><strong>Technical Reference: File Format Specification</strong></summary>

### File Extension

| Extension | Usage |
|-----------|-------|
| `.rvf` | Standard RuVector Format file |
| `.rvf.cold.N` | Cold shard N (multi-file mode) |
| `.rvf.idx.N` | Index shard N (multi-file mode) |

### MIME Type

`application/x-ruvector-format`

### Magic Number

`0x52564653` (ASCII: "RVFS")

### Byte Order

All multi-byte integers are **little-endian**.

### Alignment

All segments are **64-byte aligned** (cache-line friendly). Payloads are padded to the next 64-byte boundary.

### Root Manifest

The root manifest (Level 0) occupies the last 4,096 bytes of the most recent MANIFEST_SEG. This enables instant location via backward scan:

```rust
let (offset, header) = find_latest_manifest(&file_data)?;
```

The root manifest provides:
- Segment directory (offsets to all segments)
- Hotset pointers (entry points, top layer, centroids, quant dicts)
- Epoch counter
- Vector count and dimension
- Profile identifiers

### Domain Profiles

| Profile | Code | Optimized For |
|---------|------|---------------|
| Generic | `0x00` | General-purpose vectors |
| RVDNA | `0x01` | Genomic sequence embeddings |
| RVText | `0x02` | Language model embeddings |
| RVGraph | `0x03` | Graph/network node embeddings |
| RVVision | `0x04` | Image/vision model embeddings |

</details>

<details>
<summary><strong>Building from Source</strong></summary>

### Prerequisites

- **Rust 1.87+** via [rustup](https://rustup.rs/) (`rustup update stable`)
- For WASM: `rustup target add wasm32-unknown-unknown`
- For Node.js bindings: Node.js 18+ and `npm`

### Build Examples

```bash
cd examples/rvf
cargo build
```

### Build All RVF Crates

```bash
cd crates/rvf
cargo build --workspace
```

### Run All Tests

```bash
cd crates/rvf
cargo test --workspace
```

### Run Clippy

```bash
cd crates/rvf
cargo clippy --all-targets --workspace --exclude rvf-wasm
```

### Build WASM Microkernel

```bash
cd crates/rvf
cargo build --target wasm32-unknown-unknown -p rvf-wasm --release
ls target/wasm32-unknown-unknown/release/rvf_wasm.wasm
```

### Build Node.js Bindings

```bash
cd crates/rvf/rvf-node
npm install && npm run build
```

### Run Benchmarks

```bash
cd crates/rvf
cargo bench --bench rvf_benchmarks
```

</details>

---

<details>
<summary><strong>Project Structure</strong></summary>

```
examples/rvf/
  Cargo.toml                  # Standalone workspace
  src/lib.rs                  # Shared utilities
  examples/
    # Core (6)
    basic_store.rs            # Store lifecycle, insert, query, persistence
    progressive_index.rs      # Three-layer HNSW, recall measurement
    quantization.rs           # Scalar, product, binary quantization + tiering
    wire_format.rs            # Raw segment I/O, hash validation, tail-scan
    crypto_signing.rs         # Ed25519 signing, witness chains, tamper detection
    filtered_search.rs        # Metadata-filtered vector search
    # Agentic AI (6)
    agent_memory.rs           # Persistent agent memory + witness audit
    swarm_knowledge.rs        # Multi-agent shared knowledge base
    reasoning_trace.rs        # Chain-of-thought with lineage derivation
    tool_cache.rs             # Tool call result cache with TTL + compaction
    agent_handoff.rs          # Transfer agent state between instances
    experience_replay.rs      # RL experience replay buffer
    # Practical Production (5)
    semantic_search.rs        # Document search engine (4 filter workflows)
    recommendation.rs         # Item recommendations (collaborative filtering)
    rag_pipeline.rs           # Retrieval-augmented generation pipeline
    embedding_cache.rs        # LRU cache with temperature tiering
    dedup_detector.rs         # Near-duplicate detection + compaction
    # Vertical Domains (4)
    genomic_pipeline.rs       # DNA k-mer search (.rvdna profile)
    financial_signals.rs      # Market signals with attestation
    medical_imaging.rs        # Radiology embedding search (.rvvis)
    legal_discovery.rs        # Legal document similarity (.rvtext)
    # Exotic Capabilities (5)
    self_booting.rs           # RVF with embedded unikernel
    ebpf_accelerator.rs       # eBPF hot-path acceleration
    hyperbolic_taxonomy.rs    # Hierarchy-aware search
    multimodal_fusion.rs      # Cross-modal text + image search
    sealed_engine.rs          # Full cognitive engine (capstone)
    # Runtime Targets + Postgres (5)
    browser_wasm.rs           # Browser-side WASM vector search
    edge_iot.rs               # IoT device with binary quantization
    serverless_function.rs    # Cold-start optimized for Lambda
    ruvllm_inference.rs       # LLM KV cache + LoRA via RVF
    postgres_bridge.rs        # PostgreSQL ↔ RVF export/import
    # Network & Security (4)
    network_sync.rs           # Peer-to-peer vector store sync
    tee_attestation.rs        # TEE attestation + sealed keys
    access_control.rs         # Role-based vector access control
    zero_knowledge.rs         # Zero-knowledge proof integration
    # Autonomous Agent (1)
    ruvbot.rs                 # Autonomous RVF-powered agent bot
    # POSIX & Systems (3)
    posix_fileops.rs          # POSIX file operations with RVF
    linux_microkernel.rs      # Linux microkernel distribution
    mcp_in_rvf.rs             # MCP server embedded in RVF
    # Network Operations (1)
    network_interfaces.rs     # Network OS telemetry (60 interfaces)
```

</details>

## Learn More

| Resource | Description |
|----------|-------------|
| [RVF Format Specification](../../crates/rvf/README.md) | Full format documentation, architecture, and API reference |
| [ADR-029](../../docs/adr/ADR-029-rvf-canonical-format.md) | Architecture decision record for the canonical format |
| [ADR-030](../../docs/adr/ADR-030-rvf-computational-container.md) | Computational container (KERNEL_SEG, EBPF_SEG) specification |
| [ADR-031](../../docs/adr/ADR-031-rvf-example-repository.md) | Example repository design (this collection of 40 examples) |
| [Benchmarks](../../crates/rvf/benches/) | Performance benchmarks (HNSW build, quantization, wire I/O) |
| [Integration Tests](../../crates/rvf/tests/rvf-integration/) | E2E test suite (progressive recall, quantization, wire interop) |

## Contributing

```bash
git clone https://github.com/ruvnet/ruvector
cd ruvector/examples/rvf
cargo build && cargo run --example basic_store
```

All contributions must pass `cargo clippy` with zero warnings and maintain the existing test count (currently 543+).

## License

Dual-licensed under [MIT](../../LICENSE-MIT) or [Apache-2.0](../../LICENSE-APACHE) at your option.

---

<p align="center">
  <sub>Built with Rust. One file &mdash; store it, send it, run it.</sub>
</p>
