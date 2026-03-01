<p align="center">
  <strong>RVF</strong> &mdash; RuVector Format
</p>

<p align="center">
  <em>One file. Store vectors. Ship models. Boot services. Prove everything.</em>
</p>

<p align="center">
  <a href="#quick-start">🚀 Quick Start</a> &bull;
  <a href="#what-rvf-contains">📦 What It Contains</a> &bull;
  <a href="#sealed-cognitive-engines">🧠 Cognitive Engines</a> &bull;
  <a href="#architecture">🏗️ Architecture</a> &bull;
  <a href="#performance">⚡ Performance</a> &bull;
  <a href="#comparison">📊 Comparison</a>
</p>

<p align="center">
  <img alt="Tests" src="https://img.shields.io/badge/tests-1156_passing-brightgreen?style=flat-square" />
  <img alt="Examples" src="https://img.shields.io/badge/examples-46_runnable-brightgreen?style=flat-square" />
  <img alt="Crates" src="https://img.shields.io/badge/crates-22-blue?style=flat-square" />
  <img alt="Lines" src="https://img.shields.io/badge/rust-64k_lines-orange?style=flat-square" />
  <img alt="License" src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue?style=flat-square" />
  <img alt="MSRV" src="https://img.shields.io/badge/MSRV-1.87-purple?style=flat-square" />
  <img alt="no_std" src="https://img.shields.io/badge/no__std-compatible-green?style=flat-square" />
  <a href="https://crates.io/crates/rvf-runtime"><img alt="crates.io" src="https://img.shields.io/crates/v/rvf-runtime?style=flat-square&label=crates.io" /></a>
  <a href="https://www.npmjs.com/package/@ruvector/rvf"><img alt="npm" src="https://img.shields.io/npm/v/@ruvector/rvf?style=flat-square&label=npm" /></a>
</p>

---
dsp
## 🧠 What is RVF? A Cognitive Container

**RVF (RuVector Format)** is a universal binary substrate that merges database, model, graph engine, kernel, and attestation into a single deployable file. 

A `.rvf` file can store vector embeddings, carry LoRA adapter deltas, embed GNN graph state, include a bootable Linux microkernel, run queries in a 5.5 KB WASM runtime, and prove every operation through a cryptographic witness chain &mdash; all in one file that runs anywhere from a browser to bare metal.

This is not a database format. It is an **executable knowledge unit**.

#### 🖥️ Compute & Execution

| Capability | How | Segment |
|------------|-----|---------|
| 🖥️ **Self-boot as a microservice** | The file contains a real Linux kernel. Drop it on a VM and it boots as a running service in under 125 ms. No install, no dependencies. | `KERNEL_SEG` (0x0E) |
| ⚡ **Hardware-speed lookups via eBPF** | Hot vectors are served directly in the Linux kernel data path, bypassing userspace entirely. Three real C programs handle distance, filtering, and routing. | `EBPF_SEG` (0x0F) |
| 🌐 **Runs in any browser** | A 5.5 KB WebAssembly runtime lets the same file serve queries in a browser tab with zero backend. | `WASM_SEG` |

#### 🧠 AI & Data Storage

| Capability | How | Segment |
|------------|-----|---------|
| 🧠 **Ship models, graphs, and quantum state** | One file carries LoRA fine-tune weights, graph neural network state, and quantum circuit snapshots alongside vectors. No separate model registry needed. | `OVERLAY` / `GRAPH` / `SKETCH` |
| 🌿 **Git-like branching** | Create a child file that shares all parent data. Only changed vectors are copied. A 1M-vector parent with 100 edits produces a ~2.5 MB child instead of a 512 MB copy. | `COW_MAP` / `MEMBERSHIP` (0x20-0x23) |
| 📊 **Instant queries while loading** | Start answering queries at 70% accuracy immediately. Accuracy improves to 95%+ as the full index loads in the background. No waiting. | `INDEX_SEG` |
| 🔍 **Search with filters** | Combine vector similarity with metadata conditions like "genre = sci-fi AND year > 2020" in a single query. | `META_IDX_SEG` (0x0D) |
| 💥 **Never corrupts on crash** | Power loss mid-write? The file is always readable. Append-only design means incomplete writes are simply ignored on recovery. No write-ahead log needed. | Format rule |

#### 🔐 Security & Trust

RVF treats security as a structural property of the format, not an afterthought. Every segment can be individually signed, every operation is hash-chained into a tamper-evident ledger, and every derived file carries a cryptographic link to its parent. The result: you can hand someone a `.rvf` file and they can independently verify what data is inside, who produced it, what operations were performed, and whether anything was altered — without trusting the sender.

| Capability | How | Segment |
|------------|-----|---------|
| 🔗 **Tamper-evident audit trail** | Every insert, query, and deletion is recorded in a SHAKE-256 hash-linked chain. Change one byte anywhere and the entire chain fails verification. | `WITNESS_SEG` (0x0A) |
| 🔐 **Kernel locked to its data** | A 128-byte `KernelBinding` footer ties each signed kernel to its manifest hash. Prevents segment-swap attacks — the kernel only boots if the data it was built for is present and unmodified. | `KERNEL_SEG` + `CRYPTO_SEG` |
| 🛡️ **Quantum-safe signatures** | Segments can be signed with ML-DSA-65 (FIPS 204) and SLH-DSA-128s alongside Ed25519. Dual-signing means files stay trustworthy even after quantum computers break classical crypto. | `CRYPTO_SEG` (0x0C) |
| 🧬 **Track where data came from** | Every file records its parent, grandparent, and full derivation history with cryptographic hashes — DNA-style lineage. Verify that a child was legitimately derived from its parent without accessing the parent file. | `MANIFEST_SEG` |
| 🏛️ **TEE attestation** | Record hardware attestation quotes from Intel SGX, AMD SEV-SNP, Intel TDX, and ARM CCA. Proves vector operations ran inside a verified secure enclave. | `CRYPTO_SEG` |
| 🛡️ **Adversarial hardening** | Input validation, rate limiting, and resource exhaustion guards. Declarative `SecurityPolicy` configuration prevents denial-of-service and malformed-input attacks. | Runtime |

#### 📦 Ecosystem & Tooling

| Capability | How | Segment |
|------------|-----|---------|
| 🤖 **Plug into AI agents** | An MCP server lets Claude Code, Cursor, and other AI tools create, query, and manage vector stores directly. | npm package |
| 📦 **Use from any language** | Published as 14 Rust crates, 6 adapters, 4 npm packages, a CLI tool, and an HTTP server. Works from Rust, Node.js, browsers, and the command line. | 14 crates + 6 adapters + 4 npm |
| ♻️ **Always backward-compatible** | Old tools skip new segment types they don't understand. A file with COW branching still works in a reader that only knows basic vectors. | Format rule |

```
         📦 Anatomy of a .rvf Cognitive Container (24 segment types)
       ┌─────────────────────────────────────────────────────────────┐
       │                       .rvf file                             │
       ├──────────────────────────┬──────────────────────────────────┤
       │  📋 Core Data            │  🧠 AI & Models                 │
       │  MANIFEST  (4 KB root)   │  OVERLAY   (LoRA deltas)         │
       │  VEC_SEG   (embeddings)  │  GRAPH     (GNN state)           │
       │  INDEX_SEG (HNSW graph)  │  SKETCH    (quantum / VQE)       │
       │  QUANT     (codebooks)   │  META      (key-value)           │
       │  HOT       (promoted)    │  PROFILE   (domain config)       │
       │  META_IDX  (filter idx)  │  JOURNAL   (mutations)           │
       ├──────────────────────────┼──────────────────────────────────┤
       │  🌿 COW Branching        │  🔐 Security & Trust            │
       │  COW_MAP   (ownership)   │  WITNESS   (audit chain)         │
       │  REFCOUNT  (ref counts)  │  CRYPTO    (signatures)          │
       │  MEMBERSHIP (visibility) │  KERNEL    (Linux + binding)     │
       │  DELTA     (sparse patch)│  EBPF      (XDP / TC / socket)   │
       │                          │  WASM      (5.5 KB runtime)      │
       ├──────────────────────────┴──────────────────────────────────┤
       │                                                             │
       │   Store it ─── single-file vector DB, no external deps      │
       │   Ship it  ─── wire-format streaming, one file = one unit   │
       │   Run it   ─── boots Linux, runs in browser, eBPF in kernel │
       │   Trust it ─── witness chain + attestation + PQ signatures  │
       │   Branch it ── COW at cluster granularity, <3 ms            │
       │   Track it ─── DNA-style lineage from parent to child       │
       │                                                             │
       │         ┌──────────┐              ┌──────────┐              │
       │         │ 🖥️ Boots │             │ 🌐 Runs  │              │
       │         │ as Linux │              │ in any   │              │
       │         │ microVM  │              │ browser  │              │
       │         │ <125 ms  │              │ 5.5 KB   │              │
       │         └──────────┘              └──────────┘              │
       └─────────────────────────────────────────────────────────────┘
```

The same `.rvf` file runs on servers, browsers (WASM), edge devices, TEE enclaves, Firecracker microVMs, and in the Linux kernel data path (eBPF) &mdash; no conversion, no re-indexing, no external dependencies.

---

## 📦 Published Packages

### Rust Crates (crates.io)

| Crate | Version | Description |
|-------|---------|-------------|
| [`rvf-types`](https://crates.io/crates/rvf-types) | 0.2.0 | Segment types, 24 headers, quality, security, AGI container types (`no_std`) |
| [`rvf-wire`](https://crates.io/crates/rvf-wire) | 0.1.0 | Wire format read/write (`no_std`) |
| [`rvf-manifest`](https://crates.io/crates/rvf-manifest) | 0.1.0 | Two-level manifest, FileIdentity, COW pointers |
| [`rvf-quant`](https://crates.io/crates/rvf-quant) | 0.1.0 | Scalar, product, and binary quantization |
| [`rvf-index`](https://crates.io/crates/rvf-index) | 0.1.0 | HNSW progressive indexing (Layer A/B/C) |
| [`rvf-crypto`](https://crates.io/crates/rvf-crypto) | 0.2.0 | SHAKE-256, Ed25519, witness chains, seed crypto |
| [`rvf-runtime`](https://crates.io/crates/rvf-runtime) | 0.2.0 | Full store API, COW engine, AGI containers, QR seeds, safety net |
| [`rvf-kernel`](https://crates.io/crates/rvf-kernel) | 0.1.0 | Linux kernel builder, initramfs, Docker pipeline |
| [`rvf-ebpf`](https://crates.io/crates/rvf-ebpf) | 0.1.0 | BPF C compiler (XDP, socket filter, TC) |
| [`rvf-launch`](https://crates.io/crates/rvf-launch) | 0.1.0 | QEMU microvm launcher, KVM/TCG, QMP |
| [`rvf-server`](https://crates.io/crates/rvf-server) | 0.1.0 | HTTP REST + TCP streaming server |
| [`rvf-import`](https://crates.io/crates/rvf-import) | 0.1.0 | JSON, CSV, NumPy importers |
| [`rvf-cli`](https://crates.io/crates/rvf-cli) | 0.1.0 | Unified CLI with 17 subcommands |
| [`rvf-solver-wasm`](https://crates.io/crates/rvf-solver-wasm) | 0.1.0 | Thompson Sampling temporal solver (WASM, `no_std`) |

### npm Packages (npmjs.org)

| Package | Version | Description |
|---------|---------|-------------|
| [`@ruvector/rvf`](https://www.npmjs.com/package/@ruvector/rvf) | 0.1.0 | Unified TypeScript SDK |
| [`@ruvector/rvf-node`](https://www.npmjs.com/package/@ruvector/rvf-node) | 0.1.0 | Node.js N-API native bindings |
| [`@ruvector/rvf-wasm`](https://www.npmjs.com/package/@ruvector/rvf-wasm) | 0.1.0 | WASM browser package |
| [`@ruvector/rvf-mcp-server`](https://www.npmjs.com/package/@ruvector/rvf-mcp-server) | 0.1.0 | MCP server for AI agents |

### Platform Support

| Platform | Status | Notes |
|----------|--------|-------|
| **Linux** (x86_64, aarch64) | Full | KVM acceleration, eBPF, SIMD (AVX2/NEON) |
| **macOS** (x86_64, Apple Silicon) | Full | TCG fallback for QEMU, NEON SIMD on ARM |
| **Windows** (x86_64) | Core | Store, query, index, crypto work. QEMU launcher requires WSL or Windows QEMU. |
| **WASM** (browser, edge) | Full | 5.5 KB microkernel, ~46 KB control plane |
| **no_std** (embedded) | Types only | `rvf-types` and `rvf-wire` are `no_std` compatible |

---

## 🚀 Quick Start

### Install

```bash
# Rust crate (library)
cargo add rvf-runtime

# CLI tool
cargo install rvf-cli
# or build from source:
cd crates/rvf && cargo build -p rvf-cli --release

# Node.js / npm
npm install @ruvector/rvf-node

# WASM (browser / edge)
rustup target add wasm32-unknown-unknown
cargo build -p rvf-wasm --target wasm32-unknown-unknown --release
# → target/wasm32-unknown-unknown/release/rvf_wasm.wasm (~46 KB)

# MCP Server (for Claude Code, Cursor, etc.)
npx @ruvector/rvf-mcp-server --transport stdio
```

### Rust Crate

```toml
# Cargo.toml
[dependencies]
rvf-runtime = "0.2"          # full store API
rvf-types   = "0.2"          # types only (no_std)
rvf-wire    = "0.1"          # wire format (no_std)
rvf-crypto  = "0.2"          # signatures + witness chains
rvf-import  = "0.1"          # JSON/CSV/NumPy importers
```

```rust
use rvf_runtime::{RvfStore, options::{RvfOptions, QueryOptions, DistanceMetric}};

let mut store = RvfStore::create("vectors.rvf", RvfOptions {
    dimension: 384,
    metric: DistanceMetric::Cosine,
    ..Default::default()
})?;

// Insert
store.ingest_batch(&[&embedding], &[1], None)?;

// Query
let results = store.query(&query, 10, &QueryOptions::default())?;

// Derive a child with lineage tracking
let child = store.derive("child.rvf", DerivationType::Filter, None)?;

// Embed a kernel — file now boots as a microservice
store.embed_kernel(0x00, 0x01, 0, &kernel_image, 8080, None)?;

store.close()?;
```

### Node.js / npm

```bash
npm install @ruvector/rvf-node
```

```javascript
const { RvfDatabase } = require('@ruvector/rvf-node');

// Create, insert, query
const db = RvfDatabase.create('vectors.rvf', { dimension: 384 });
db.ingestBatch(new Float32Array(384), [1]);
const results = db.query(new Float32Array(384), 10);

// Lineage & inspection
console.log(db.fileId());       // unique file UUID
console.log(db.dimension());    // 384
console.log(db.segments());     // [{ type, id, size }]

db.close();
```

### WASM (Browser / Edge)

```html
<script type="module">
  import init, { WasmRvfStore } from './rvf_wasm.js';
  await init();

  const store = WasmRvfStore.create(384);
  store.ingest(1, new Float32Array(384));
  const results = store.query(new Float32Array(384), 10);
  console.log(results); // [{ id, distance }]
</script>
```

The WASM binary is **~46 KB** (control plane with in-memory store) or **~5.5 KB** (tile microkernel for Cognitum). No backend required.

### CLI

```bash
# Full lifecycle from the command line
rvf create vectors.rvf --dimension 384
rvf ingest vectors.rvf --input data.json --format json
rvf query  vectors.rvf --vector "0.1,0.2,..." --k 10
rvf status vectors.rvf
rvf inspect vectors.rvf          # show all segments
rvf compact vectors.rvf          # reclaim deleted space
rvf derive parent.rvf child.rvf --type filter
rvf serve  vectors.rvf --port 8080

# Machine-readable output
rvf status vectors.rvf --json
```

### Lightweight (rvlite)

```rust
use rvf_adapter_rvlite::{RvliteCollection, RvliteConfig};

let mut col = RvliteCollection::create(RvliteConfig::new("vectors.rvf", 128))?;
col.add(1, &[0.1; 128])?;
let matches = col.search(&[0.15; 128], 5);
```

### Generate Sample Files

```bash
cd examples/rvf
cargo run --example generate_all
ls output/   # 46 .rvf files ready to inspect
rvf status output/sealed_engine.rvf
rvf inspect output/linux_microkernel.rvf
```

---

## 📋 What RVF Contains

An RVF file is a sequence of typed segments. Each segment is self-describing, 64-byte aligned, and independently integrity-checked. The format supports 24 segment types that together constitute a complete cognitive runtime:

```
.rvf file (Sealed Cognitive Engine)
  |
  +-- MANIFEST_SEG .... 4 KB root manifest, segment directory, instant boot
  +-- VEC_SEG ......... Vector embeddings (fp16/fp32/int8/int4/binary)
  +-- INDEX_SEG ....... HNSW progressive index (Layer A/B/C)
  +-- OVERLAY_SEG ..... LoRA adapter deltas, incremental updates
  +-- GRAPH_SEG ....... GNN adjacency, edge weights, graph state
  +-- QUANT_SEG ....... Quantization codebooks (scalar/PQ/binary)
  +-- SKETCH_SEG ...... Access sketches, VQE snapshots, quantum state
  +-- META_SEG ........ Key-value metadata, observation-state
  +-- WITNESS_SEG ..... Tamper-evident audit trails, attestation records
  +-- CRYPTO_SEG ...... ML-DSA-65 / Ed25519 signatures, sealed keys
  +-- WASM_SEG ........ 5.5 KB query microkernel (Tier 1: browser/edge)
  +-- EBPF_SEG ........ eBPF fast-path program (Tier 2: kernel acceleration)
  +-- KERNEL_SEG ...... Compressed unikernel (Tier 3: self-booting service)
  +-- PROFILE_SEG ..... Domain profile (RVDNA/RVText/RVGraph/RVVision)
  +-- HOT_SEG ......... Temperature-promoted hot data
  +-- META_IDX_SEG .... Metadata inverted indexes for filtered search
  +-- COW_MAP_SEG ..... Cluster ownership map for COW branching (0x20)
  +-- REFCOUNT_SEG .... Cluster reference counts, rebuildable (0x21)
  +-- MEMBERSHIP_SEG .. Vector visibility filter for branches (0x22)
  +-- DELTA_SEG ....... Sparse delta patches / LoRA overlays (0x23)
  +-- TRANSFER_PRIOR .. Transfer learning priors (0x30)
  +-- POLICY_KERNEL ... Thompson Sampling policy state (0x31)
  +-- COST_CURVE ...... Cost/reward curves for solver (0x32)
```

---

## 🧠 Sealed Cognitive Engines

When an RVF file combines vectors, models, compute, and trust segments, it becomes a **deployable intelligence capsule**:

### Example: Domain Intelligence Unit

```
ClinicalOncologyEngine.rvdna           (one file, ~50 MB)
  Contains:
  -- Medical corpus embeddings          VEC_SEG      384-dim, 2M vectors
  -- MicroLoRA oncology fine-tune       OVERLAY_SEG  adapter deltas
  -- Biological pathway GNN             GRAPH_SEG    pathway modeling
  -- Molecular similarity state         SKETCH_SEG   quantum-enhanced
  -- Linux microkernel service          KERNEL_SEG   boots on Firecracker
  -- Browser query runtime              WASM_SEG     5.5 KB, no backend
  -- eBPF drug lookup accelerator       EBPF_SEG     sub-microsecond
  -- Attested execution proof           WITNESS_SEG  tamper-evident chain
  -- Post-quantum signature             CRYPTO_SEG   ML-DSA-65
```

This is not a database. It is a **sealed, auditable, self-booting domain expert**. Copy it to a Firecracker VM and it boots a Linux service. Open it in a browser and WASM serves queries locally. Ship it air-gapped and it produces identical results under audit.

---

## 🔌 RuVector Ecosystem Integration

RVF is the canonical binary format across 87+ Rust crates in the RuVector ecosystem:

| Domain | Crates | RVF Segment |
|--------|--------|-------------|
| **LLM Inference** | `ruvllm`, `ruvllm-cli`, `ruvllm-wasm` | VEC_SEG (KV cache), OVERLAY_SEG (LoRA) |
| **Attention** | `ruvector-attention`, coherence-gated transformer | VEC_SEG, INDEX_SEG |
| **GNN** | `ruvector-gnn`, `ruvector-graph`, graph-node/wasm | GRAPH_SEG |
| **Quantum** | `ruQu`, `ruqu-core`, `ruqu-algorithms`, `ruqu-exotic` | SKETCH_SEG (VQE, syndrome tables) |
| **Min-Cut Coherence** | `ruvector-mincut`, mincut-gated-transformer | GRAPH_SEG, INDEX_SEG |
| **Delta Tracking** | `ruvector-delta-core`, delta-graph, delta-index | OVERLAY_SEG, JOURNAL_SEG |
| **Neural Routing** | `ruvector-tiny-dancer-core` (FastGRNN) | VEC_SEG, META_SEG |
| **Sparse Inference** | `ruvector-sparse-inference` | VEC_SEG, QUANT_SEG |
| **Temporal Tensors** | `ruvector-temporal-tensor` | VEC_SEG, META_SEG |
| **Cognitum Silicon** | `cognitum-gate-kernel`, `cognitum-gate-tilezero` | WASM_SEG (64 KB tiles) |
| **SONA Learning** | `sona` (self-optimizing neural arch) | VEC_SEG, WITNESS_SEG |
| **Agent Memory** | claude-flow, agentdb, agentic-flow, ospipe | All segments via adapters |

The same `.rvf` file format runs on cloud servers, Firecracker microVMs, TEE enclaves, edge devices, Cognitum tiles, and in the browser.

---

## ✨ Features

### Storage & Indexing

| Feature | Description |
|---------|-------------|
| **Append-only segments** | Crash-safe without WAL. Every write is atomic with per-segment integrity checksums. |
| **Progressive indexing** | Three-tier HNSW (Layer A/B/C). First query at 70% recall before full index loads. |
| **Temperature-tiered quantization** | Hot vectors stay fp16, warm use product quantization, cold use binary &mdash; automatically. |
| **Metadata filtering** | Filtered k-NN with boolean expressions (AND/OR/NOT/IN/RANGE). |
| **4 KB instant boot** | Root manifest fits in one page read. Cold boot < 5 ms. |
| **24 segment types** | VEC, INDEX, MANIFEST, QUANT, WITNESS, CRYPTO, KERNEL, EBPF, WASM, COW_MAP, MEMBERSHIP, DELTA, TRANSFER_PRIOR, POLICY_KERNEL, COST_CURVE, and 9 more. |

### COW Branching (RVCOW)

| Feature | Description |
|---------|-------------|
| **COW branching** | Git-like copy-on-write at cluster granularity. Derive child stores that share parent data; only changed clusters are copied. |
| **Membership filters** | Shared HNSW index across branches with bitmap visibility control. Include/exclude modes. |
| **Snapshot freeze** | Immutable snapshot at any generation. Metadata-only operation, no data copy. |
| **Delta segments** | Sparse patches for LoRA overlays. Hot-path guard upgrades to full slab. |
| **Rebuildable refcounts** | No WAL. Refcounts derived from COW map chain during compaction. |

### Ecosystem & Tooling

| Feature | Description |
|---------|-------------|
| **Domain profiles** | `.rvdna`, `.rvtext`, `.rvgraph`, `.rvvis` extensions map to optimized profiles. |
| **Unified CLI** | 17 subcommands: create, ingest, query, delete, status, inspect, compact, derive, serve, launch, embed-kernel, embed-ebpf, filter, freeze, verify-witness, verify-attestation, rebuild-refcounts. |
| **6 library adapters** | Drop-in integration for claude-flow, agentdb, ospipe, agentic-flow, rvlite, sona. |
| **MCP server** | Model Context Protocol integration for Claude Code, Cursor, and AI agents. |
| **Node.js bindings** | N-API bindings with lineage, kernel/eBPF, and inspection support. |

---

## 🏗️ Architecture

```
  +-----------------------------------------------------------------+
  |                    Cognitive Layer                                |
  |  ruvllm (LLM)  | ruvector-gnn (GNN) | ruQu (Quantum)           |
  |  ruvector-attention | sona (SONA) | ruvector-mincut             |
  +---+------------------+-----------------+-----------+------------+
      |                  |                 |           |
  +---v------------------v-----------------v-----------v------------+
  |                    Agent & Application Layer                     |
  |  claude-flow | agentdb | agentic-flow | ospipe | rvlite         |
  +---+------------------+-----------------+-----------+------------+
      |                  |                 |           |
  +---v------------------v-----------------v-----------v------------+
  |                    RVF SDK Layer                                  |
  |  rvf-runtime | rvf-index | rvf-quant | rvf-crypto | rvf-wire    |
  |  rvf-manifest | rvf-types | rvf-import | rvf-adapters            |
  +---+--------+---------+----------+-----------+------------------+
      |        |         |          |           |
  +---v---+ +--v----+ +--v-----+ +-v--------+ +v-----------+ +v------+
  |server | | node  | | wasm   | | kernel   | | ebpf       | | cli   |
  |HTTP   | | N-API | | ~46 KB | |bzImage+  | |clang BPF   | |17 cmds|
  |REST+  | |       | |        | |initramfs | |XDP/TC/sock | |       |
  |TCP    | |       | |        | +----------+ +------------+ +-------+
  +-------+ +-------+ +--------+ +-v--------+
                                  | launch   |
                                  |QEMU+QMP  |
                                  +----------+
```

### Segment Model

An `.rvf` file is a sequence of 64-byte-aligned segments. Each segment has a self-describing header:

```
+--------+------+-------+--------+-----------+-------+----------+
| Magic  | Ver  | Type  | Flags  | SegmentID | Size  | Hash     |
| 4B     | 1B   | 1B    | 2B     | 8B        | 8B    | 16B ...  |
+--------+------+-------+--------+-----------+-------+----------+
| Payload (variable length, 64-byte aligned)                     |
+----------------------------------------------------------------+
```

### Crate Map

| Crate | Lines | Purpose |
|-------|------:|---------|
| `rvf-types` | 7,000+ | 24 segment types, AGI container, quality, security, WASM bootstrap, QR seed (`no_std`) |
| `rvf-wire` | 2,011 | Wire format read/write (`no_std`) |
| `rvf-manifest` | 1,700+ | Two-level manifest with 4 KB root, FileIdentity codec, COW pointers, double-root scheme |
| `rvf-index` | 2,691 | HNSW progressive indexing (Layer A/B/C) |
| `rvf-quant` | 1,443 | Scalar, product, and binary quantization |
| `rvf-crypto` | 1,725 | SHAKE-256, Ed25519, witness chains, attestation, seed crypto |
| `rvf-runtime` | 8,000+ | Full store API, COW engine, AGI containers, QR seeds, safety net, adversarial defense |
| `rvf-kernel` | 2,400+ | Real Linux kernel builder, cpio/newc initramfs, Docker build, SHA3-256 verification |
| `rvf-launch` | 1,200+ | QEMU microvm launcher, KVM/TCG detection, QMP shutdown protocol |
| `rvf-ebpf` | 1,100+ | Real BPF C compiler (XDP, socket filter, TC), vmlinux.h generation |
| `rvf-wasm` | 1,700+ | WASM control plane: in-memory store, query, segment inspection, witness chain verification (~46 KB) |
| `rvf-solver-wasm` | 1,500+ | Thompson Sampling temporal solver, PolicyKernel, three-loop architecture (`no_std`) |
| `rvf-node` | 852 | Node.js N-API bindings with lineage, kernel/eBPF, and inspection |
| `rvf-cli` | 1,800+ | Unified CLI with 17 subcommands (create, ingest, query, delete, status, inspect, compact, derive, serve, launch, embed-kernel, embed-ebpf, filter, freeze, verify-witness, verify-attestation, rebuild-refcounts) |
| `rvf-server` | 1,165 | HTTP REST + TCP streaming server |
| `rvf-import` | 980 | JSON, CSV, NumPy (.npy) importers |
| **Adapters** | **6,493** | **6 library integrations (see below)** |

---

## ⚡ Performance

| Metric | Target | Achieved |
|--------|--------|----------|
| Cold boot (4 KB manifest read) | < 5 ms | **1.6 us** |
| First query recall@10 (Layer A only) | >= 0.70 | >= 0.70 |
| Full quality recall@10 (Layer C) | >= 0.95 | >= 0.95 |
| WASM binary (tile microkernel) | < 8 KB | **~5.5 KB** |
| WASM binary (control plane) | < 50 KB | **~46 KB** |
| Segment header size | 64 bytes | 64 bytes |
| Minimum file overhead | < 1 KB | < 256 bytes |
| COW branch creation (10K vecs) | < 10 ms | **2.6 ms** (child = 162 bytes) |
| COW branch creation (100K vecs) | < 50 ms | **6.8 ms** (child = 162 bytes) |
| COW read (local cluster, pread) | < 5 us | **1,348 ns/vector** |
| COW read (inherited from parent) | < 5 us | **1,442 ns/vector** |
| Write coalescing (32 vecs, 1 cluster) | 1 COW event | **654 us**, 1 event |
| CowMap lookup | < 100 ns | **28 ns** |
| Membership filter contains() | < 100 ns | **23-33 ns** |
| Snapshot freeze | < 100 ns | **30-52 ns** |

### Progressive Loading

RVF doesn't make you wait for the full index:

| Stage | Data Loaded | Recall@10 | Latency |
|-------|-------------|-----------|---------|
| **Layer A** | Entry points + centroids | >= 0.70 | < 5 ms |
| **Layer B** | Hot region adjacency | >= 0.85 | ~10 ms |
| **Layer C** | Full HNSW graph | >= 0.95 | ~50 ms |

---

## 📊 Comparison

| Feature | RVF | Annoy | FAISS | Qdrant | Milvus |
|---------|-----|-------|-------|--------|--------|
| Single-file format | Yes | Yes | No | No | No |
| Crash-safe (no WAL) | Yes | No | No | Needs WAL | Needs WAL |
| Progressive loading | Yes (3 layers) | No | No | No | No |
| COW branching | Yes (cluster-level) | No | No | No | No |
| Membership filters | Yes (shared HNSW) | No | No | No | No |
| Snapshot freeze | Yes (zero-copy) | No | No | No | No |
| WASM support | Yes (5.5 KB) | No | No | No | No |
| Self-booting kernel | Yes (real Linux) | No | No | No | No |
| eBPF acceleration | Yes (XDP/TC/socket) | No | No | No | No |
| `no_std` compatible | Yes | No | No | No | No |
| Post-quantum sigs | Yes (ML-DSA-65) | No | No | No | No |
| TEE attestation | Yes | No | No | No | No |
| Metadata filtering | Yes | No | Yes | Yes | Yes |
| Temperature tiering | Automatic | No | Manual | No | No |
| Quantization | 3-tier auto | No | Yes (manual) | Yes | Yes |
| Lineage provenance | Yes (DNA-style) | No | No | No | No |
| Domain profiles | 5 profiles | No | No | No | No |
| Append-only | Yes | Build-once | Build-once | Log-based | Log-based |

### vs Docker / OCI Containers

| | RVF Cognitive Container | Docker / OCI |
|---|---|---|
| **File format** | Single `.rvf` file | Layered tarball images |
| **Boot target** | QEMU microVM (microvm machine) | Container runtime (runc, containerd) |
| **Vector data** | Native segment, HNSW-indexed | External volume mount |
| **Branching** | Vector-native COW at cluster granularity | Layer-based COW (filesystem) |
| **eBPF** | Embedded in file, verified | Separate deployment |
| **Attestation** | Witness chain + KernelBinding | External signing (cosign, notary) |
| **Size (hello world)** | ~17 KB (with initramfs + vectors) | ~5 MB (Alpine) |

### vs Traditional Vector Databases

| | RVF | Pinecone / Milvus / Qdrant |
|---|---|---|
| **Deployment** | Single file, zero dependencies | Server process + storage |
| **Branching** | Native COW, 2.6 ms for 10K vectors | Copy entire collection |
| **Multi-tenant** | Membership filter on shared index | Separate collections |
| **Edge deploy** | `scp file.rvf host:` + boot | Install + configure + import |
| **Provenance** | Cryptographic witness chain | External audit logs |
| **Compute** | Embedded kernel + eBPF | N/A |

### vs Git LFS / DVC

| | RVF COW | Git LFS / DVC |
|---|---|---|
| **Granularity** | Vector cluster (256 KB) | Whole file |
| **Index sharing** | Shared HNSW + membership filter | No index awareness |
| **Query during branch** | Yes, sub-microsecond | No query capability |
| **Delta encoding** | Sparse row patches (LoRA) | Binary diff |

### vs SQLite / DuckDB

| | RVF | SQLite | DuckDB |
|---|---|---|---|
| **Vector-native** | Yes (HNSW, quantization, COW) | No (extension needed) | No (extension needed) |
| **Self-booting** | Yes (KERNEL_SEG) | No | No |
| **eBPF acceleration** | Yes (XDP, TC, socket) | No | No |
| **Cryptographic audit** | Yes (witness chains) | No | No |
| **Progressive loading** | 3-tier HNSW (70% &rarr; 95% recall) | N/A | N/A |
| **WASM support** | 5.5 KB microkernel | Yes (via wasm) | No |
| **Single file** | Yes | Yes | Yes |

---

## 🧬 Lineage Provenance

RVF supports DNA-style derivation chains for tracking how files were produced from one another. Each `.rvf` file carries a 68-byte `FileIdentity` recording its unique ID, its parent's ID, and a cryptographic hash of the parent's manifest. This enables tamper-evident provenance verification from any file back to its root ancestor.

```
  parent.rvf          child.rvf          grandchild.rvf
  (depth=0)           (depth=1)          (depth=2)
  file_id: AAA        file_id: BBB       file_id: CCC
  parent_id: 000      parent_id: AAA     parent_id: BBB
  parent_hash: 000    parent_hash: H(A)  parent_hash: H(B)
       |                   |                   |
       +-------derive------+-------derive------+
```

### Domain Profiles & Extension Aliasing

Domain-specific extensions are automatically mapped to optimized profiles. The authoritative profile lives in the `Level0Root.profile_id` field; the file extension is a convenience hint:

| Extension | Domain Profile | Optimized For |
|-----------|---------------|---------------|
| `.rvf` | Generic | General-purpose vectors |
| `.rvdna` | RVDNA | Genomic sequence embeddings |
| `.rvtext` | RVText | Language model embeddings |
| `.rvgraph` | RVGraph | Graph/network node embeddings |
| `.rvvis` | RVVision | Image/vision model embeddings |

### Deriving a Child Store

```rust
use rvf_runtime::{RvfStore, options::{RvfOptions, DistanceMetric}};
use rvf_types::DerivationType;
use std::path::Path;

let options = RvfOptions {
    dimension: 384,
    metric: DistanceMetric::Cosine,
    ..Default::default()
};
let parent = RvfStore::create(Path::new("parent.rvf"), options)?;

// Derive a filtered child -- inherits dimensions and options
let child = parent.derive(
    Path::new("child.rvf"),
    DerivationType::Filter,
    None,
)?;
assert_eq!(child.lineage_depth(), 1);
assert_eq!(child.parent_id(), parent.file_id());
```

---

## 🖥️ Self-Booting RVF (Cognitive Container)

RVF supports an optional three-tier execution model that allows a single `.rvf` file to carry executable compute alongside its vector data. A file can serve queries from a browser (Tier 1 WASM), accelerate hot-path lookups in the Linux kernel (Tier 2 eBPF), or boot as a standalone microservice inside a Firecracker microVM or TEE enclave (Tier 3 unikernel) -- all from the same file.

| Tier | Segment | Size | Environment | Boot Time | Use Case |
|------|---------|------|-------------|-----------|----------|
| **1: WASM** | WASM_SEG (existing) | 5.5 KB | Browser, edge, IoT | <1 ms | Portable queries everywhere |
| **2: eBPF** | EBPF_SEG (`0x0F`) | 10-50 KB | Linux kernel (XDP, TC) | <20 ms | Sub-microsecond hot cache hits |
| **3: Unikernel** | KERNEL_SEG (`0x0E`) | 200 KB - 2 MB | Firecracker, TEE, bare metal | <125 ms | Zero-dependency self-booting service |

Readers that do not recognize KERNEL_SEG or EBPF_SEG skip them per the RVF forward-compatibility rule. The computational capability is purely additive.

### Embedding a Kernel

```rust
use rvf_runtime::RvfStore;
use rvf_types::kernel::{KernelArch, KernelType};
use std::path::Path;

let mut store = RvfStore::open(Path::new("vectors.rvf"))?;

// Embed a compressed unikernel image
store.embed_kernel(
    KernelArch::X86_64 as u8,       // arch
    KernelType::Hermit as u8,        // kernel type
    0x0018,                          // flags: HAS_QUERY_API | HAS_NETWORKING
    &compressed_kernel_image,        // kernel binary
    8080,                            // API port
    Some("console=ttyS0 quiet"),     // cmdline (optional)
)?;

// Later, extract it
if let Some((header, image_data)) = store.extract_kernel()? {
    println!("Kernel: {:?} ({} bytes)", header.kernel_arch(), image_data.len());
}
```

### Embedding an eBPF Program

```rust
use rvf_types::ebpf::{EbpfProgramType, EbpfAttachType};

// Embed an eBPF XDP program for fast-path vector lookup
store.embed_ebpf(
    EbpfProgramType::XdpDistance as u8,   // program type
    EbpfAttachType::XdpIngress as u8,     // attach point
    384,                                   // max vector dimension
    &ebpf_bytecode,                        // BPF ELF object
    Some(&btf_section),                    // BTF data (optional)
)?;

if let Some((header, program_data)) = store.extract_ebpf()? {
    println!("eBPF: {:?} ({} bytes)", header.program_type, program_data.len());
}
```

### Security Model

- **7-step fail-closed verification**: hash, signature, TEE measurement, all must pass before kernel boot
- **Authority boundary**: guest kernel owns auth/audit/witness; host eBPF is acceleration-only
- **Signing**: Ed25519 for development, ML-DSA-65 (FIPS 204) for production
- **TEE priority**: SEV-SNP first, SGX second, ARM CCA third
- **Size limits**: kernel images capped at 128 MiB, eBPF programs at 16 MiB

For the full specification including wire formats, attestation binding, and implementation phases, see [ADR-030: RVF Cognitive Container](docs/adr/ADR-030-rvf-computational-container.md).

### End-to-End: Claude Code Appliance

The `claude_code_appliance` example builds a complete self-booting AI development environment as a single `.rvf` file. It uses real infrastructure — a Docker-built Linux kernel, Ed25519 SSH keys, a BPF C socket filter, and a cryptographic witness chain.

**Prerequisites:** Docker (for kernel build), Rust 1.87+

```bash
# Build and run the example
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

**Boot sequence** (once launched on Firecracker/QEMU):

```
1. Firecracker loads KERNEL_SEG → Linux boots (<125 ms)
2. SSH server starts on port 2222
3. curl -fsSL https://claude.ai/install.sh | bash
4. RVF query server starts on port 8080
5. Claude Code ready for use
```

**Connect and use:**

```bash
# Boot the file (requires QEMU or Firecracker)
rvf launch claude_code_appliance.rvf

# SSH in
ssh -p 2222 deploy@localhost

# Query the package database
curl -s localhost:8080/query -d '{"vector":[0.1,...], "k":5}'

# Or use the CLI
rvf query claude_code_appliance.rvf --vector "0.1,0.2,..." --k 5
```

**Verified output from the example run:**

```
=== Claude Code Appliance Summary ===
  File size:       5,260,093 bytes (5.1 MB)
  Segments:        8
  Packages:        20 (203.1 MB manifest)
  KERNEL_SEG:      MicroLinux x86_64 (5,243,904 bytes)
  EBPF_SEG:        SocketFilter (3,805 bytes)
  SSH users:       3 (Ed25519 signed, all verified)
  Witness chain:   6 entries (tamper-evident, all verified)
  Lineage:         base + v1 snapshot (parent hash matches)
```

Final file: **5.1 MB single `.rvf`** — boots Linux, serves queries, runs Claude Code.

One file. Boots Linux. Runs SSH. Serves vectors. Installs Claude Code. Proves every step.

### Launching with QEMU

```bash
# CLI launcher (auto-detects KVM or falls back to TCG)
rvf launch vectors.rvf

# Manual QEMU (if you want control)
rvf launch vectors.rvf --memory 512M --cpus 2 --port-forward 2222:22,8080:8080

# Extract kernel for external use
rvf inspect vectors.rvf --segment kernel --output kernel.bin
qemu-system-x86_64 -M microvm -kernel kernel.bin -append "console=ttyS0" -nographic
```

### Building Your Own Bootable RVF

Step-by-step to create a self-booting `.rvf` from scratch:

```bash
# 1. Create a vector store
rvf create myservice.rvf --dimension 384

# 2. Ingest your data
rvf ingest myservice.rvf --input embeddings.json --format json

# 3. Build and embed a Linux kernel (uses Docker)
rvf embed-kernel myservice.rvf --arch x86_64

# 4. Optionally embed an eBPF filter
rvf embed-ebpf myservice.rvf --program filter.c

# 5. Verify the result
rvf inspect myservice.rvf
# MANIFEST_SEG, VEC_SEG, INDEX_SEG, KERNEL_SEG, EBPF_SEG, WITNESS_SEG

# 6. Boot it
rvf launch myservice.rvf
```

---

## 🔗 Library Adapters

RVF provides drop-in adapters for 6 libraries in the RuVector ecosystem:

| Adapter | Purpose | Key Feature |
|---------|---------|-------------|
| `rvf-adapter-claude-flow` | AI agent memory | WITNESS_SEG audit trails |
| `rvf-adapter-agentdb` | Agent vector database | Progressive HNSW indexing |
| `rvf-adapter-ospipe` | Observation-State pipeline | META_SEG for state vectors |
| `rvf-adapter-agentic-flow` | Swarm coordination | Inter-agent memory sharing |
| `rvf-adapter-rvlite` | Lightweight embedded store | Minimal API, edge-friendly |
| `rvf-adapter-sona` | Neural architecture | Experience replay + trajectories |

---

## 🤖 AGI Cognitive Container (ADR-036)

An AGI container packages a complete AI agent runtime into a single sealed `.rvf` file. Where the [Self-Booting RVF](#%EF%B8%8F-self-booting-rvf-cognitive-container) section covers the compute tiers (WASM/eBPF/Kernel), the AGI container adds the intelligence layer on top: model identity, orchestration config, tool registries, evaluation harnesses, authority controls, and coherence gates.

```
AGI Cognitive Container (.rvf)
├── Identity ────── container UUID, build UUID, model ID hash
├── Orchestrator ── Claude Code / Claude Flow config (JSON)
├── Tools ──────── MCP tool adapter registry
├── Agent Prompts ─ role definitions per agent type
├── Eval Harness ── task suite + grading rules
├── Skills ──────── promoted skill library
├── Policy ──────── governance rules + authority config
├── Coherence ───── min score, contradiction rate, rollback ratio
├── Resources ───── time/token/cost budgets with clamping
├── Replay ──────── automation script for deterministic re-execution
├── Kernel Config ─ boot parameters, network, SSH
├── Domain Profile ─ coding / research / ops specialization
└── Signature ───── HMAC-SHA256 or Ed25519 tamper seal
```

### Execution Modes

| Mode | Purpose | Requires |
|------|---------|----------|
| **Replay** | Deterministic re-execution from witness logs | Witness chain |
| **Verify** | Validate container integrity and run eval harness | Kernel + world model, or WASM + vectors |
| **Live** | Full autonomous operation with tool use | Kernel + world model |

### Authority Levels

Authority is hierarchical — each level permits everything below it:

| Level | Allows |
|-------|--------|
| `ReadOnly` | Read vectors, run queries |
| `WriteMemory` | + Write to vector store, update index |
| `ExecuteTools` | + Invoke MCP tools, run commands |
| `WriteExternal` | + Network access, file I/O, push to git |

Default authority per mode: Replay → ReadOnly, Verify → ExecuteTools, Live → WriteMemory.

### Resource Budgets

Every container carries hard limits that are clamped to safety maximums:

| Resource | Max | Default |
|----------|-----|---------|
| Time | 3,600 sec | 300 sec |
| Tokens | 1,000,000 | 100,000 |
| Cost | $10.00 | $1.00 |
| Tool calls | 500 | 100 |
| External writes | 50 | 10 |

### Coherence Gates

Coherence thresholds halt execution when the agent's world model drifts:

- `min_coherence_score` (0.0–1.0) — minimum quality gate
- `max_contradiction_rate` (0.0–1.0) — tolerable contradiction frequency
- `max_rollback_ratio` (0.0–1.0) — ratio of rolled-back decisions

### Building a Container

```rust
use rvf_runtime::agi_container::AgiContainerBuilder;
use rvf_types::agi_container::*;

let (payload, header) = AgiContainerBuilder::new(container_id, build_id)
    .with_model_id("claude-opus-4-6")
    .with_orchestrator(b"{\"max_turns\":100}")
    .with_tool_registry(b"[{\"name\":\"search\",\"type\":\"rvf_query\"}]")
    .with_eval_tasks(b"[{\"id\":1,\"spec\":\"fix bug\"}]")
    .with_eval_graders(b"[{\"type\":\"test_pass\"}]")
    .with_authority_config(b"{\"level\":\"WriteMemory\"}")
    .with_coherence_config(b"{\"min_cut\":0.7,\"rollback\":true}")
    .with_project_instructions(b"# CLAUDE.md\nFix bugs, run tests.")
    .with_segments(ContainerSegments {
        kernel_present: true, manifest_present: true,
        world_model_present: true, ..Default::default()
    })
    .build_and_sign(signing_key)?;

// Parse and validate
let manifest = ParsedAgiManifest::parse(&payload)?;
assert_eq!(manifest.model_id_str(), Some("claude-opus-4-6"));
assert!(manifest.is_autonomous_capable());
assert!(header.is_signed());
```

See [ADR-036](../../docs/adr/ADR-036-agi-cognitive-container.md) for the full specification.

## 📱 QR Cognitive Seed (ADR-034)

A QR Cognitive Seed (RVQS) encodes a portable intelligence capsule into a scannable QR code. It carries bootstrap hosts, layer hashes, and cryptographic signatures in a compact binary format.

```rust
use rvf_runtime::seed_crypto;

let hash = seed_crypto::seed_content_hash(data);       // 8-byte SHAKE-256
let sig  = seed_crypto::sign_seed(key, payload);        // 32-byte HMAC
let ok   = seed_crypto::verify_seed(key, payload, &sig);
```

Types: `SeedHeader`, `HostEntry`, `LayerEntry` (rvf-types), plus `qr_encode` for QR matrix generation (rvf-runtime).

## 🔒 Quality & Safety Net

The quality system tracks retrieval fidelity across progressive index layers and enforces graceful degradation when budgets are exceeded.

- `RetrievalQuality` — Full / Partial / Degraded / Failed
- `ResponseQuality` — per-query quality metadata with evidence
- `SafetyNetBudget` — time, token, and cost budgets with automatic clamping
- `DegradationReport` — structured fallback path and reason tracking

## 🛡️ Security Modules

| Module | Crate | Purpose |
|--------|-------|---------|
| `SecurityPolicy` / `HardeningFields` | rvf-types | Declarative per-file security configuration |
| `adversarial` | rvf-runtime | Input validation, dimension/size checks at write boundary |
| `dos` | rvf-runtime | Rate limiting, resource exhaustion guards |
| `KernelBinding` | rvf-types | Binds signed kernels to specific manifest hashes |
| `verify_witness_chain` | rvf-crypto | SHAKE-256 chain integrity verification |

## 🧬 WASM Self-Bootstrapping (0x10)

WASM_SEG enables an RVF file to carry its own WASM interpreter, creating a three-layer bootstrap stack:

```
Raw bytes → WASM interpreter → microkernel → vector data
```

Types: `WasmRole` (Interpreter/Microkernel/Solver), `WasmTarget` (Browser/Node/Edge/Embedded), `WasmHeader` (rvf-types/wasm_bootstrap).

The `rvf-solver-wasm` crate implements a Thompson Sampling temporal solver as a `no_std` WASM module with `dlmalloc`, producing segment types `TRANSFER_PRIOR` (0x30), `POLICY_KERNEL` (0x31), and `COST_CURVE` (0x32).

---

<details>
<summary><strong>46 Runnable Examples</strong></summary>

Every example uses real RVF APIs end-to-end &mdash; no mocks, no stubs. Run any example with:

```bash
cd examples/rvf
cargo run --example <name>
```

#### Core Fundamentals (6)

| # | Example | What It Demonstrates |
|---|---------|---------------------|
| 1 | `basic_store` | Create, insert 100 vectors, k-NN query, close, reopen, verify persistence |
| 2 | `progressive_index` | Build three-layer HNSW, measure recall@10 progression (0.70 &rarr; 0.95) |
| 3 | `quantization` | Scalar, product, and binary quantization with temperature tiering |
| 4 | `wire_format` | Raw 64-byte segment I/O, CRC32c hash validation, manifest tail-scan |
| 5 | `crypto_signing` | Ed25519 segment signing, SHAKE-256 witness chains, tamper detection |
| 6 | `filtered_search` | Metadata-filtered queries: Eq, Ne, Gt, Range, In, And, Or |

#### Agentic AI (6)

| # | Example | What It Demonstrates |
|---|---------|---------------------|
| 7 | `agent_memory` | Persistent agent memory across sessions with witness audit trail |
| 8 | `swarm_knowledge` | Multi-agent shared knowledge base, cross-agent semantic search |
| 9 | `reasoning_trace` | Chain-of-thought lineage: parent &rarr; child &rarr; grandchild derivation |
| 10 | `tool_cache` | Tool call result caching with TTL expiry, delete_by_filter, compaction |
| 11 | `agent_handoff` | Transfer agent state between instances via derive + clone |
| 12 | `experience_replay` | Reinforcement learning replay buffer with priority sampling |

#### Production Patterns (5)

| # | Example | What It Demonstrates |
|---|---------|---------------------|
| 13 | `semantic_search` | Document search engine with 4 filter workflows |
| 14 | `recommendation` | Collaborative filtering with genre and quality filters |
| 15 | `rag_pipeline` | 5-step RAG: chunk, embed, retrieve, rerank, assemble context |
| 16 | `embedding_cache` | Zipf access patterns, 3-tier quantization, memory savings |
| 17 | `dedup_detector` | Near-duplicate detection, clustering, compaction |

#### Industry Verticals (4)

| # | Example | What It Demonstrates |
|---|---------|---------------------|
| 18 | `genomic_pipeline` | DNA k-mer search with `.rvdna` profile and lineage tracking |
| 19 | `financial_signals` | Market signals with Ed25519 signing and TEE attestation |
| 20 | `medical_imaging` | Radiology embedding search with `.rvvis` profile |
| 21 | `legal_discovery` | Legal document similarity with `.rvtext` profile |

#### Cognitive Containers (5)

| # | Example | What It Demonstrates |
|---|---------|---------------------|
| 22 | `self_booting` | Embed/extract unikernel (KERNEL_SEG), header verification |
| 23 | `ebpf_accelerator` | Embed/extract eBPF (EBPF_SEG), XDP program co-existence |
| 24 | `hyperbolic_taxonomy` | Hierarchy-aware Poincar&eacute; embeddings, depth-filtered search |
| 25 | `multimodal_fusion` | Cross-modal text + image search with modality filtering |
| 26 | `sealed_engine` | Capstone: vectors + kernel + eBPF + witness + lineage in one file |

#### Runtime Targets (5)

| # | Example | What It Demonstrates |
|---|---------|---------------------|
| 27 | `browser_wasm` | WASM-compatible API surface, raw wire segments, size budget |
| 28 | `edge_iot` | Constrained IoT device with binary quantization |
| 29 | `serverless_function` | Cold-start optimization, manifest tail-scan, progressive loading |
| 30 | `ruvllm_inference` | LLM KV cache + LoRA adapters + policy store via RVF |
| 31 | `postgres_bridge` | PostgreSQL export/import with lineage and witness audit |

#### Network & Security (4)

| # | Example | What It Demonstrates |
|---|---------|---------------------|
| 32 | `network_sync` | Peer-to-peer vector store synchronization |
| 33 | `tee_attestation` | TEE platform attestation, sealed keys, computation proof |
| 34 | `access_control` | Role-based vector access control with audit trails |
| 35 | `zero_knowledge` | Zero-knowledge proofs for privacy-preserving vector ops |

#### Systems & Integration (5)

| # | Example | What It Demonstrates |
|---|---------|---------------------|
| 36 | `ruvbot` | Autonomous agent with RVF memory, planning, and tool use |
| 37 | `posix_fileops` | POSIX raw I/O, atomic rename, advisory locking, segment access |
| 38 | `linux_microkernel` | 20-package Linux distro with SSH keys and kernel embed |
| 39 | `mcp_in_rvf` | MCP server runtime + eBPF filter embedded in RVF |
| 40 | `network_interfaces` | 6-chassis / 60-interface network telemetry with anomaly detection |

#### COW Branching & Generation (3)

| # | Example | What It Demonstrates |
|---|---------|---------------------|
| 41 | [`cow_branching`](../../examples/rvf/examples/cow_branching.rs) | COW derive, cluster-level copy, write coalescing, parent inheritance |
| 42 | [`membership_filter`](../../examples/rvf/examples/membership_filter.rs) | Include/exclude bitmap filters for shared HNSW traversal |
| 43 | [`snapshot_freeze`](../../examples/rvf/examples/snapshot_freeze.rs) | Generation snapshots, immutable freeze, generation tracking |

#### Appliance & Generation (3)

| # | Example | What It Demonstrates |
|---|---------|---------------------|
| 44 | [`claude_code_appliance`](../../examples/rvf/examples/claude_code_appliance.rs) | Bootable AI dev environment: real kernel + eBPF + vectors + witness + crypto |
| 45 | [`live_boot_proof`](../../examples/rvf/examples/live_boot_proof.rs) | Docker-boot an `.rvf`, SSH in, verify segments are live and operational |
| 46 | [`generate_all`](../../examples/rvf/examples/generate_all.rs) | Batch generation of all example `.rvf` files |

See the [examples README](../../examples/rvf/README.md) for tutorials, usage patterns, and detailed walkthroughs.

</details>

<details>
<summary><strong>Importing Data</strong></summary>

### From NumPy (.npy)

```rust
use rvf_import::numpy::{parse_npy_file, NpyConfig};
use std::path::Path;

let records = parse_npy_file(
    Path::new("embeddings.npy"),
    &NpyConfig { start_id: 0 },
)?;
// records: Vec<VectorRecord> with id, vector, metadata
```

### From CSV

```rust
use rvf_import::csv_import::{parse_csv_file, CsvConfig};
use std::path::Path;

let config = CsvConfig {
    id_column: Some("id".into()),
    dimension: 128,
    ..Default::default()
};
let records = parse_csv_file(Path::new("vectors.csv"), &config)?;
```

### From JSON

```rust
use rvf_import::json::{parse_json_file, JsonConfig};
use std::path::Path;

let config = JsonConfig {
    id_field: "id".into(),
    vector_field: "embedding".into(),
    ..Default::default()
};
let records = parse_json_file(Path::new("vectors.json"), &config)?;
```

### CLI Import Tool

```bash
# Using rvf-import binary directly
cargo run --bin rvf-import -- \
    --input data.npy \
    --output vectors.rvf \
    --format npy \
    --dimension 384

# Or via the unified rvf CLI
rvf create vectors.rvf --dimension 384
rvf ingest vectors.rvf --input data.json --format json
```

</details>

<details>
<summary><strong>HTTP Server API</strong></summary>

### Starting the Server

```bash
cargo run --bin rvf-server -- --path vectors.rvf --port 8080
```

### REST Endpoints

**Ingest vectors:**
```bash
curl -X POST http://localhost:8080/ingest \
  -H "Content-Type: application/json" \
  -d '{
    "vectors": [[0.1, 0.2, 0.3, 0.4], [0.5, 0.6, 0.7, 0.8]],
    "ids": [1, 2]
  }'
```

**Query nearest neighbors:**
```bash
curl -X POST http://localhost:8080/query \
  -H "Content-Type: application/json" \
  -d '{
    "vector": [0.1, 0.2, 0.3, 0.4],
    "k": 10
  }'
```

**Delete vectors:**
```bash
curl -X POST http://localhost:8080/delete \
  -H "Content-Type: application/json" \
  -d '{"ids": [1, 2]}'
```

**Get status:**
```bash
curl http://localhost:8080/status
```

**Compact (reclaim space):**
```bash
curl -X POST http://localhost:8080/compact
```

</details>

<details>
<summary><strong>MCP Server (Model Context Protocol)</strong></summary>

### Overview

The `@ruvector/rvf-mcp-server` package exposes RVF stores to AI agents via the Model Context Protocol. Supports stdio and SSE transports.

### Starting the MCP Server

```bash
# stdio transport (for Claude Code, Cursor, etc.)
npx @ruvector/rvf-mcp-server --transport stdio

# SSE transport (for web clients)
npx @ruvector/rvf-mcp-server --transport sse --port 3100
```

### Claude Code Integration

Add to your Claude Code MCP config:

```json
{
  "mcpServers": {
    "rvf": {
      "command": "npx",
      "args": ["@ruvector/rvf-mcp-server", "--transport", "stdio"]
    }
  }
}
```

### Available MCP Tools

| Tool | Description |
|------|-------------|
| `rvf_create_store` | Create a new RVF vector store |
| `rvf_open_store` | Open an existing store (read-write or read-only) |
| `rvf_close_store` | Close a store and release the writer lock |
| `rvf_ingest` | Insert vectors with optional metadata |
| `rvf_query` | k-NN similarity search with metadata filters |
| `rvf_delete` | Delete vectors by ID |
| `rvf_delete_filter` | Delete vectors matching a metadata filter |
| `rvf_compact` | Compact store to reclaim dead space |
| `rvf_status` | Get store status (dimensions, vector count, etc.) |
| `rvf_list_stores` | List all open stores |

### MCP Resources

| URI | Description |
|-----|-------------|
| `rvf://stores` | JSON listing of all open stores and their status |

### MCP Prompts

| Prompt | Description |
|--------|-------------|
| `rvf-search` | Natural language similarity search |
| `rvf-ingest` | Data ingestion with auto-embedding |

</details>

<details>
<summary><strong>Confidential Core Attestation</strong></summary>

### Overview

RVF can record hardware TEE (Trusted Execution Environment) attestation quotes alongside vector data. This proves that vector operations occurred inside a verified secure enclave.

### Supported Platforms

| Platform | Enum Value | Quote Format |
|----------|-----------|--------------|
| Intel SGX | `TeePlatform::Sgx` (0) | DCAP quote |
| AMD SEV-SNP | `TeePlatform::SevSnp` (1) | VCEK attestation report |
| Intel TDX | `TeePlatform::Tdx` (2) | TD quote |
| ARM CCA | `TeePlatform::ArmCca` (3) | CCA token |
| Software (testing) | `TeePlatform::SoftwareTee` (0xFE) | Synthetic |

### Attestation Types

| Type | Witness Code | Purpose |
|------|-------------|---------|
| Platform Attestation | `0x05` | TEE identity and measurement verification |
| Key Binding | `0x06` | Encryption keys sealed to TEE measurement |
| Computation Proof | `0x07` | Proof that operations ran inside the enclave |
| Data Provenance | `0x08` | Chain of custody: model to TEE to RVF |

### Recording an Attestation

```rust
use rvf_crypto::attestation::*;
use rvf_types::attestation::*;

// Build attestation header
let mut header = AttestationHeader::new(
    TeePlatform::SoftwareTee as u8,
    AttestationWitnessType::PlatformAttestation as u8,
);
header.measurement = shake256_256(b"my-enclave-code");
header.nonce = [0x42; 16];
header.quote_length = 64;
header.timestamp_ns = 1_700_000_000_000_000_000;

// Encode the full record
let report_data = b"model=all-MiniLM-L6-v2";
let quote = vec![0xAA; 64]; // platform-specific quote bytes
let record = encode_attestation_record(&header, report_data, &quote);

// Create a witness chain entry binding this attestation
let entry = attestation_witness_entry(
    &record,
    header.timestamp_ns,
    AttestationWitnessType::PlatformAttestation,
);
// entry.action_hash == SHAKE-256-256(record)
```

### Key Binding to TEE

```rust
use rvf_crypto::attestation::*;
use rvf_types::attestation::*;

let key = TeeBoundKeyRecord {
    key_type: KEY_TYPE_TEE_BOUND,
    algorithm: 0, // Ed25519
    sealed_key_length: 32,
    key_id: shake256_128(b"my-public-key"),
    measurement: shake256_256(b"my-enclave"),
    platform: TeePlatform::Sgx as u8,
    reserved: [0; 3],
    valid_from: 0,
    valid_until: 0, // no expiry
    sealed_key: vec![0xBB; 32],
};

// Verify the key is accessible in the current environment
verify_key_binding(
    &key,
    TeePlatform::Sgx,
    &shake256_256(b"my-enclave"),
    current_time_ns,
)?; // Ok(()) if platform + measurement match
```

### Attested Segment Flag

Any segment produced inside a TEE can set the `ATTESTED` flag for fast scanning:

```rust
use rvf_types::SegmentFlags;

let flags = SegmentFlags::empty()
    .with(SegmentFlags::SIGNED)
    .with(SegmentFlags::ATTESTED);
// bit 2 (SIGNED) + bit 10 (ATTESTED) = 0x0404
```

</details>

<details>
<summary><strong>Progressive Indexing</strong></summary>

### How It Works

Traditional vector databases make you wait for the full index before you can query. RVF uses a three-layer progressive model:

**Layer A (Coarse Routing)**
- Contains entry points and partition centroids
- Loads in microseconds from the manifest
- Provides approximate results immediately (recall >= 0.70)

**Layer B (Hot Region)**
- Contains adjacency lists for frequently-accessed vectors
- Loaded based on temperature heuristics
- Improves recall to >= 0.85

**Layer C (Full Graph)**
- Complete HNSW adjacency for all vectors
- Full recall >= 0.95
- Loaded in the background while queries are already being served

### Using Progressive Indexing

```rust
use rvf_index::progressive::ProgressiveIndex;
use rvf_index::layers::IndexLayer;

let mut adapter = RvfIndexAdapter::new(IndexAdapterConfig::default());
adapter.build(vectors, ids);

// Start with Layer A only (fastest)
adapter.load_progressive(&[IndexLayer::A]);
let fast_results = adapter.search(&query, 10);

// Add layers as they load
adapter.load_progressive(&[IndexLayer::A, IndexLayer::B, IndexLayer::C]);
let precise_results = adapter.search(&query, 10);
```

</details>

<details>
<summary><strong>Quantization Tiers</strong></summary>

### Temperature-Based Quantization

RVF automatically assigns vectors to quantization tiers based on access frequency:

| Tier | Temperature | Method | Memory | Recall |
|------|------------|--------|--------|--------|
| **Hot** | Frequently accessed | fp16 / scalar | 2x per dim | ~0.999 |
| **Warm** | Moderate access | Product quantization | 8-16x compression | ~0.95 |
| **Cold** | Rarely accessed | Binary quantization | 32x compression | ~0.80 |

### How It Works

1. A Count-Min Sketch tracks access frequency per vector
2. Vectors are assigned to tiers based on configurable thresholds
3. Hot vectors stay at full precision for fast, accurate retrieval
4. Cold vectors are heavily compressed but still searchable
5. Tier assignment is stored in SKETCH_SEG and updated periodically

### Using Quantization

```rust
use rvf_quant::scalar::ScalarQuantizer;
use rvf_quant::product::ProductQuantizer;
use rvf_quant::binary::{encode_binary, hamming_distance};
use rvf_quant::traits::Quantizer;

// Scalar quantization (Hot tier)
let sq = ScalarQuantizer::train(&vectors);
let encoded = sq.encode(&vector);
let decoded = sq.decode(&encoded);

// Product quantization (Warm tier)
let pq = ProductQuantizer::train(&vectors, 8); // 8 subquantizers
let code = pq.encode(&vector);

// Binary quantization (Cold tier)
let bits = encode_binary(&vector);
let dist = hamming_distance(&bits_a, &bits_b);
```

</details>

<details>
<summary><strong>Wire Format Specification</strong></summary>

### Segment Header (64 bytes, `repr(C)`)

| Offset | Size | Field | Description |
|--------|------|-------|-------------|
| 0x00 | 4 | `magic` | `0x52564653` ("RVFS") |
| 0x04 | 1 | `version` | Format version (currently 1) |
| 0x05 | 1 | `seg_type` | Segment type (see enum below) |
| 0x06 | 2 | `flags` | Bitfield (COMPRESSED, ENCRYPTED, SIGNED, SEALED, ATTESTED, ...) |
| 0x08 | 8 | `segment_id` | Monotonically increasing ID |
| 0x10 | 8 | `payload_length` | Byte length of payload |
| 0x18 | 8 | `timestamp_ns` | Nanosecond UNIX timestamp |
| 0x20 | 1 | `checksum_algo` | 0=CRC32C, 1=XXH3-128, 2=SHAKE-256 |
| 0x21 | 1 | `compression` | 0=none, 1=LZ4, 2=ZSTD |
| 0x22 | 2 | `reserved_0` | Must be zero |
| 0x24 | 4 | `reserved_1` | Must be zero |
| 0x28 | 16 | `content_hash` | First 128 bits of payload hash |
| 0x38 | 4 | `uncompressed_len` | Original size before compression |
| 0x3C | 4 | `alignment_pad` | Padding to 64-byte boundary |

### Segment Types

| Code | Name | Description |
|------|------|-------------|
| `0x01` | VEC | Raw vector embeddings |
| `0x02` | INDEX | HNSW adjacency and routing |
| `0x03` | OVERLAY | Graph overlay deltas |
| `0x04` | JOURNAL | Metadata mutations, deletions |
| `0x05` | MANIFEST | Segment directory, epoch state |
| `0x06` | QUANT | Quantization dictionaries |
| `0x07` | META | Key-value metadata |
| `0x08` | HOT | Temperature-promoted data |
| `0x09` | SKETCH | Access counter sketches |
| `0x0A` | WITNESS | Audit trails, attestation proofs |
| `0x0B` | PROFILE | Domain profile declarations |
| `0x0C` | CRYPTO | Key material, signature chains |
| `0x0D` | META_IDX | Metadata inverted indexes |
| `0x0E` | KERNEL | Compressed unikernel image (self-booting) |
| `0x0F` | EBPF | eBPF program for kernel-level acceleration |
| `0x10` | WASM | WASM microkernel / self-bootstrapping bytecode |
| `0x20` | COW_MAP | Cluster ownership map (local vs parent) |
| `0x21` | REFCOUNT | Cluster reference counts (rebuildable) |
| `0x22` | MEMBERSHIP | Vector visibility filter for branches |
| `0x23` | DELTA | Sparse delta patches (LoRA overlays) |
| `0x30` | TRANSFER_PRIOR | Transfer learning prior distributions |
| `0x31` | POLICY_KERNEL | Thompson Sampling policy kernels |
| `0x32` | COST_CURVE | Cost/reward curves for solver |

### Segment Flags

| Bit | Name | Description |
|-----|------|-------------|
| 0 | COMPRESSED | Payload is compressed |
| 1 | ENCRYPTED | Payload is encrypted |
| 2 | SIGNED | Signature footer follows payload |
| 3 | SEALED | Immutable (compaction output) |
| 4 | PARTIAL | Streaming/partial write |
| 5 | TOMBSTONE | Logical deletion |
| 6 | HOT | Temperature-promoted |
| 7 | OVERLAY | Contains delta data |
| 8 | SNAPSHOT | Full snapshot |
| 9 | CHECKPOINT | Safe rollback point |
| 10 | ATTESTED | Produced inside attested TEE |
| 11 | HAS_LINEAGE | File carries FileIdentity lineage data |

### Crash Safety

RVF uses a two-fsync protocol:
1. Write data segment + payload, then `fsync`
2. Write MANIFEST_SEG with updated state, then `fsync`

If the process crashes between fsyncs, the incomplete segment is ignored on recovery (no valid manifest references it). No write-ahead log is needed.

### Signature Footer

When `SIGNED` flag is set, a signature footer follows the payload:

| Offset | Size | Field |
|--------|------|-------|
| 0x00 | 2 | `sig_algo` (0=Ed25519, 1=ML-DSA-65, 2=SLH-DSA-128s) |
| 0x02 | 2 | `sig_length` |
| 0x04 | var | `signature` (64 to 7,856 bytes) |
| var | 4 | `footer_length` (for backward scan) |

</details>

<details>
<summary><strong>Witness Chains & Audit Trails</strong></summary>

### How Witness Chains Work

A witness chain is a tamper-evident linked list of events, stored in WITNESS_SEG payloads. Each entry is 73 bytes:

| Field | Size | Description |
|-------|------|-------------|
| `prev_hash` | 32 | SHAKE-256 of previous entry (zero for genesis) |
| `action_hash` | 32 | SHAKE-256 of the action being witnessed |
| `timestamp_ns` | 8 | Nanosecond timestamp |
| `witness_type` | 1 | Event type discriminator |

Changing any byte in any entry causes all subsequent `prev_hash` values to fail verification. This provides tamper-evidence without a blockchain.

### Witness Types

| Code | Name | Usage |
|------|------|-------|
| `0x01` | PROVENANCE | Data origin tracking |
| `0x02` | COMPUTATION | Operation recording |
| `0x03` | SEARCH | Query audit logging |
| `0x04` | DELETION | Deletion audit logging |
| `0x05` | PLATFORM_ATTESTATION | TEE attestation quote |
| `0x06` | KEY_BINDING | Key sealed to TEE |
| `0x07` | COMPUTATION_PROOF | Verified enclave computation |
| `0x08` | DATA_PROVENANCE | Model-to-TEE-to-RVF chain |
| `0x09` | DERIVATION | File lineage derivation event |
| `0x0A` | LINEAGE_MERGE | Multi-parent lineage merge |
| `0x0B` | LINEAGE_SNAPSHOT | Lineage snapshot checkpoint |
| `0x0C` | LINEAGE_TRANSFORM | Lineage transform operation |
| `0x0D` | LINEAGE_VERIFY | Lineage verification event |
| `0x0E` | CLUSTER_COW | COW cluster copy event |
| `0x0F` | CLUSTER_DELTA | Delta patch applied to cluster |

### Creating a Witness Chain

```rust
use rvf_crypto::{create_witness_chain, verify_witness_chain, WitnessEntry};
use rvf_crypto::shake256_256;

let entries = vec![
    WitnessEntry {
        prev_hash: [0; 32],
        action_hash: shake256_256(b"inserted 1000 vectors"),
        timestamp_ns: 1_700_000_000_000_000_000,
        witness_type: 0x01,
    },
    WitnessEntry {
        prev_hash: [0; 32], // set by create_witness_chain
        action_hash: shake256_256(b"queried top-10"),
        timestamp_ns: 1_700_000_001_000_000_000,
        witness_type: 0x03,
    },
];

let chain_bytes = create_witness_chain(&entries);
let verified = verify_witness_chain(&chain_bytes)?;
assert_eq!(verified.len(), 2);
```

</details>

<details>
<summary><strong>Building from Source</strong></summary>

### Prerequisites

- Rust 1.87+ (`rustup update stable`)
- For WASM: `rustup target add wasm32-unknown-unknown`
- For Node.js bindings: Node.js 18+ and `npm`

### Build All Crates

```bash
cd crates/rvf
cargo build --workspace
```

### Run All Tests

```bash
cargo test --workspace
```

### Run Clippy

```bash
cargo clippy --all-targets --workspace --exclude rvf-wasm
```

### Build WASM Microkernel

```bash
cargo build --target wasm32-unknown-unknown -p rvf-wasm --release
ls target/wasm32-unknown-unknown/release/rvf_wasm.wasm
```

### Build CLI

```bash
cargo build -p rvf-cli
./target/debug/rvf --help
```

### Build Node.js Bindings

```bash
cd rvf-node
npm install
npm run build
```

### Run Benchmarks

```bash
cargo bench --bench rvf_benchmarks
```

</details>

<details>
<summary><strong>Domain Profiles</strong></summary>

### What Are Profiles?

Domain profiles optimize RVF behavior for specific data types:

| Profile | Code | Optimized For |
|---------|------|---------------|
| Generic | `0x00` | General-purpose vectors |
| RVDNA | `0x01` | Genomic sequence embeddings |
| RVText | `0x02` | Language model embeddings (default for agentdb) |
| RVGraph | `0x03` | Graph/network node embeddings |
| RVVision | `0x04` | Image/vision model embeddings |

### Hardware Profiles

| Profile | Level | Description |
|---------|-------|-------------|
| Generic | 0 | Minimal features, fits anywhere |
| Core | 1 | Moderate resources, good defaults |
| Hot | 2 | Memory-rich, high-performance |
| Full | 3 | All features enabled |

</details>

<details>
<summary><strong>File Format Reference</strong></summary>

### File Extension

- `.rvf` &mdash; Standard RuVector Format file
- `.rvf.cold.N` &mdash; Cold shard N (multi-file mode)
- `.rvf.idx.N` &mdash; Index shard N (multi-file mode)

### MIME Type

`application/x-ruvector-format`

### Magic Number

`0x52564653` (ASCII: "RVFS")

### Byte Order

All multi-byte integers are little-endian.

### Alignment

All segments are 64-byte aligned (cache-line friendly).

### Root Manifest

The root manifest (Level 0) occupies the last 4,096 bytes of the most recent MANIFEST_SEG. This enables instant location via `seek(EOF - scan)` and provides:

- Segment directory (offsets to all segments)
- Hotset pointers (entry points, top layer, centroids, quant dicts)
- Epoch counter
- Vector count and dimension
- Profile identifiers

</details>

---

## 🌿 RVCOW: Vector-Native Copy-on-Write Branching

RVF supports copy-on-write branching at cluster granularity (ADR-031). Instead of copying an entire file to create a variant, a derived file stores only the clusters that changed. This enables Git-like branching for vector databases.

### COW Branching

A COW child inherits all vector data from its parent by reference. Writes only allocate local clusters as needed (one slab copy per modified cluster). A 1M-vector parent (~512 MB) with 100 modified vectors produces a child of ~10 clusters (~2.5 MB).

```rust
use rvf_runtime::RvfStore;

// Create parent with vectors
let parent = RvfStore::create(Path::new("parent.rvf"), options)?;
// ... ingest vectors ...

// Derive a COW child — inherits all data, stores only changes
let child = parent.branch(Path::new("child.rvf"))?;

// COW statistics
if let Some(stats) = child.cow_stats() {
    println!("Clusters: {} total, {} local", stats.cluster_count, stats.local_cluster_count);
}
```

### Membership Filters

Branches share the parent's HNSW index. A membership filter (dense bitmap) controls which vectors are visible per branch. Excluded nodes still serve as routing waypoints during graph traversal but are never returned in results.

- **Include mode** (default): vector visible iff `filter.contains(id)`. Empty filter = empty view (fail-safe).
- **Exclude mode**: vector visible iff `!filter.contains(id)`. Empty filter = full view.

```rust
use rvf_runtime::membership::MembershipFilter;

let mut filter = MembershipFilter::new_include(1_000_000);
filter.add(42);        // vector 42 is now visible
filter.contains(42);   // true
filter.contains(100);  // false
```

### Snapshot Freeze

Freeze creates an immutable snapshot of the current generation. Further writes require deriving a new branch. Freeze is a metadata-only operation (no data copy).

```rust
let mut branch = parent.branch(Path::new("snapshot.rvf"))?;
branch.freeze()?;

// Writes now fail:
assert!(branch.ingest_batch(&[&vec], &[1], None).is_err());

// Continue on a new branch:
let next = parent.branch(Path::new("next.rvf"))?;
```

### Kernel Binding (128 bytes)

The `KernelBinding` footer (128 bytes, padded) cryptographically ties a KERNEL_SEG to its manifest. This prevents segment-swap attacks where a signed kernel from one file is embedded into a different file.

```rust
use rvf_types::kernel_binding::KernelBinding;

let binding = KernelBinding {
    manifest_root_hash: manifest_hash,   // SHAKE-256-256 of Level0Root
    policy_hash: policy_hash,            // SHAKE-256-256 of security policy
    binding_version: 1,
    ..Default::default()
};

store.embed_kernel_with_binding(arch, ktype, flags, &image, port, cmdline, &binding)?;
```

### New Segment Types

| Code | Name | Size | Purpose |
|------|------|------|---------|
| `0x20` | COW_MAP | 64B header | Cluster ownership map (local vs parent) |
| `0x21` | REFCOUNT | 32B header | Cluster reference counts (rebuildable) |
| `0x22` | MEMBERSHIP | 96B header | Vector visibility filter for branches |
| `0x23` | DELTA | 64B header | Sparse delta patches between clusters |

### New CLI Commands

```bash
rvf launch <file>                        # Boot RVF in QEMU microVM
rvf embed-kernel <file> [--arch x86_64]  # Embed kernel image
rvf embed-ebpf <file> --program <src.c>  # Compile and embed eBPF
rvf filter <file> --include <id-list>    # Create membership filter
rvf freeze <file>                        # Snapshot-freeze current state
rvf verify-witness <file>                # Verify witness chain
rvf verify-attestation <file>            # Verify KernelBinding + attestation
rvf rebuild-refcounts <file>             # Recompute refcounts from COW map
```

For the full specification, see [ADR-031: RVCOW Branching and Real Cognitive Containers](docs/adr/ADR-031-rvcow-branching-and-real-cognitive-containers.md).

---

## 🔬 Proof of Operations

Verified end-to-end workflows that demonstrate real capabilities:

### CLI: Full Lifecycle

```bash
# Create a store, ingest 100 vectors, query, derive a child
rvf create demo.rvf --dimension 128
rvf ingest demo.rvf --input data.json --format json
rvf query demo.rvf --vector "0.1,0.2,0.3,..." --k 5
rvf derive demo.rvf child.rvf --type filter
rvf inspect demo.rvf
# MANIFEST_SEG (4 KB), VEC_SEG (51 KB), INDEX_SEG (12 KB)
```

### Self-Booting: Vectors + Kernel in One File

```bash
cargo run --example self_booting
# Output:
#   Ingested 50 vectors (128 dims)
#   Pre-kernel query: top-5 results OK (nearest ID=25)
#   Kernel: 4,640 bytes embedded (x86_64, Hermit)
#   Extracted kernel: arch=X86_64, api_port=8080
#   Witness chain: 5 entries, all verified ✓
#   File size: 31 KB — data + kernel + witness in one file
```

### Linux Microkernel: Bootable OS Image

```bash
cargo run --example linux_microkernel
# Output:
#   20 packages installed as vector embeddings
#   Kernel: Linux x86_64 (4,640 bytes)
#   SSH: Ed25519 keys signed and verified ✓
#   Witness chain: 22 entries, all verified ✓
#   Package search: "build tool" → found gcc, make, cmake
#   File size: 14 KB — bootable system image
```

### Claude Code Appliance: Sealed AI Dev Environment

```bash
cargo run --example claude_code_appliance
# Output:
#   20 dev packages (rust, node, python, docker, ...)
#   Kernel: Linux x86_64 with SSH on port 2222
#   eBPF: XDP distance program embedded
#   Witness chain: 6 entries, all verified ✓
#   Ed25519 signed, tamper-evident
#   File size: 17 KB — sealed cognitive container
```

### Test Suite: 1,156 Passing

```bash
cargo test --workspace
# agi_e2e .................. 12 passed
# adr033_integration ....... 34 passed
# qr_seed_e2e .............. 11 passed
# witness_e2e .............. 10 passed
# attestation .............. 6 passed
# crypto ................... 10 passed
# computational_container .. 8 passed
# cow_branching ............ 8 passed
# cross_platform ........... 6 passed
# lineage .................. 4 passed
# smoke .................... 4 passed
# + unit tests across all crates
# Total: 1,156 tests passed
```

### Generate All 46 Example Files

```bash
cd examples/rvf && cargo run --example generate_all
ls output/  # 45 .rvf files (~11 MB total)
rvf inspect output/sealed_engine.rvf
rvf inspect output/linux_microkernel.rvf
```

## 🤝 Contributing

```bash
git clone https://github.com/ruvnet/ruvector
cd ruvector/crates/rvf
cargo test --workspace
```

All contributions must pass `cargo clippy --all-targets` with zero warnings and maintain the existing test count (currently 1,156+).

### Architecture Decision Records

| ADR | Title |
|-----|-------|
| [ADR-030](docs/adr/ADR-030-rvf-computational-container.md) | RVF Cognitive Container (Kernel, eBPF, WASM tiers) |
| [ADR-031](docs/adr/ADR-031-rvcow-branching-and-real-cognitive-containers.md) | RVCOW Branching & Real Cognitive Containers |
| [ADR-033](../../docs/adr/ADR-033-progressive-indexing-hardening.md) | Progressive Indexing Hardening |
| [ADR-034](../../docs/adr/ADR-034-qr-cognitive-seed.md) | QR Cognitive Seed (RVQS) |
| [ADR-035](../../docs/adr/ADR-035-capability-report.md) | Capability Report |
| [ADR-036](../../docs/adr/ADR-036-agi-cognitive-container.md) | AGI Cognitive Container |
| [ADR-037](../../docs/adr/ADR-037-publishable-rvf-acceptance-test.md) | Publishable RVF Acceptance Tests |
| [ADR-038](../../docs/adr/ADR-038-npx-ruvector-rvlite-witness-integration.md) | npx ruvector rvlite Witness Integration |
| [ADR-039](../../docs/adr/ADR-039-rvf-solver-wasm-agi-integration.md) | RVF Solver WASM AGI Integration |

## 📄 License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.

---

<p align="center">
  <sub>Built with Rust. Not a database &mdash; a portable cognitive runtime.</sub>
</p>
