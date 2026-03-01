# RVF Implementation Swarm Guidance

## Objective

Implement, test, optimize, and publish the RVF (RuVector Format) as the canonical binary format across all RuVector libraries. Deliver as Rust crates (crates.io), WASM packages (npm), and Node.js N-API bindings (npm).

## Phase Overview

```
Phase 1: Foundation (rvf-types + rvf-wire)         ──  Week 1-2
Phase 2: Core Runtime (manifest + index + quant)    ──  Week 3-5
Phase 3: Integration (library adapters)             ──  Week 6-8
Phase 4: WASM + Node Bindings                       ──  Week 9-10
Phase 5: Testing + Benchmarks                       ──  Week 11-12
Phase 6: Optimization + Publishing                  ──  Week 13-14
```

---

## Phase 1: Foundation — `rvf-types` + `rvf-wire`

### Agent Assignments

| Agent | Role | Crate | Deliverable |
|-------|------|-------|-------------|
| **coder-1** | Types specialist | `crates/rvf/rvf-types/` | All segment types, enums, headers |
| **coder-2** | Wire format specialist | `crates/rvf/rvf-wire/` | Read/write segment headers + payloads |
| **tester-1** | TDD for types/wire | `crates/rvf/rvf-types/tests/`, `crates/rvf/rvf-wire/tests/` | Round-trip tests, fuzz targets |
| **reviewer-1** | Spec compliance | N/A | Verify code matches wire format spec |

### `rvf-types` (no_std, no alloc dependency)

```toml
[package]
name = "rvf-types"
version = "0.1.0"
edition = "2021"
description = "RuVector Format core types — segment headers, enums, flags"
license = "MIT OR Apache-2.0"
categories = ["data-structures", "no-std"]

[features]
default = []
std = []
serde = ["dep:serde"]
```

**Files to create:**

```
crates/rvf/rvf-types/
  src/
    lib.rs              # Re-exports
    segment.rs          # SegmentHeader (64 bytes), SegmentType enum
    flags.rs            # Flags bitfield (COMPRESSED, ENCRYPTED, SIGNED, etc.)
    manifest.rs         # Level0Root (4096 bytes), ManifestTag enum
    vec_seg.rs          # BlockDirectory, BlockHeader, DataType enum
    index_seg.rs        # IndexHeader, IndexType, AdjacencyLayout
    hot_seg.rs          # HotHeader, HotEntry layout
    quant_seg.rs        # QuantHeader, QuantType enum
    sketch_seg.rs       # SketchHeader layout
    meta_seg.rs         # MetaField, FilterOp enum
    profile.rs          # ProfileId, ProfileMagic constants
    error.rs            # RvfError enum (format, query, write, tile, crypto)
    constants.rs        # Magic numbers, alignment, limits
  Cargo.toml
```

**Key constants (from spec):**

```rust
pub const SEGMENT_MAGIC: u32 = 0x52564653; // "RVFS"
pub const ROOT_MANIFEST_MAGIC: u32 = 0x52564D30; // "RVM0"
pub const SEGMENT_ALIGNMENT: usize = 64;
pub const ROOT_MANIFEST_SIZE: usize = 4096;
pub const MAX_SEGMENT_PAYLOAD: u64 = 4 * 1024 * 1024 * 1024; // 4 GB
```

**SegmentType enum (from spec 01):**

```rust
#[repr(u8)]
pub enum SegmentType {
    Invalid    = 0x00,
    Vec        = 0x01,
    Index      = 0x02,
    Overlay    = 0x03,
    Journal    = 0x04,
    Manifest   = 0x05,
    Quant      = 0x06,
    Meta       = 0x07,
    Hot        = 0x08,
    Sketch     = 0x09,
    Witness    = 0x0A,
    Profile    = 0x0B,
    Crypto     = 0x0C,
    MetaIdx    = 0x0D,
}
```

### `rvf-wire` (no_std + alloc)

```toml
[package]
name = "rvf-wire"
version = "0.1.0"
description = "RuVector Format wire format reader/writer"

[dependencies]
rvf-types = { path = "../rvf-types" }

[features]
default = ["std"]
std = ["rvf-types/std"]
```

**Files to create:**

```
crates/rvf/rvf-wire/
  src/
    lib.rs
    reader.rs           # SegmentReader: parse header, validate magic/hash
    writer.rs           # SegmentWriter: build header, compute hash, align
    varint.rs           # LEB128 encode/decode
    delta.rs            # Delta encoding with restart points
    crc32c.rs           # CRC32C (software + hardware detect)
    xxh3.rs             # XXH3-128 hash (or re-export from xxhash-rust)
    tail_scan.rs        # find_latest_manifest() backward scan
    manifest_reader.rs  # Level 0 root manifest parser
    manifest_writer.rs  # Level 0 + Level 1 manifest builder
    vec_seg_codec.rs    # VEC_SEG columnar encode/decode
    hot_seg_codec.rs    # HOT_SEG interleaved encode/decode
    index_seg_codec.rs  # INDEX_SEG adjacency encode/decode
  Cargo.toml
```

### Phase 1 Acceptance Criteria

- [ ] `rvf-types` compiles with `#![no_std]`
- [ ] `rvf-wire` round-trips: create segment -> serialize -> deserialize -> compare
- [ ] Tail scan finds manifest in valid file
- [ ] CRC32C matches reference implementation
- [ ] Varint codec matches LEB128 spec
- [ ] `cargo test` passes for both crates
- [ ] `cargo clippy` clean, `cargo fmt` clean

---

## Phase 2: Core Runtime — manifest + index + quant

### Agent Assignments

| Agent | Role | Crate | Deliverable |
|-------|------|-------|-------------|
| **coder-3** | Manifest system | `crates/rvf/rvf-manifest/` | Two-level manifest, progressive boot |
| **coder-4** | Progressive indexing | `crates/rvf/rvf-index/` | Layer A/B/C HNSW with progressive load |
| **coder-5** | Quantization | `crates/rvf/rvf-quant/` | Temperature-tiered quant (fp16/i8/PQ/binary) |
| **coder-6** | Full runtime | `crates/rvf/rvf-runtime/` | RvfStore API, compaction, append-only |
| **tester-2** | Integration tests | `crates/rvf/tests/` | Progressive load, crash safety, recall |

### `rvf-manifest`

**Key functionality:**
- Parse Level 0 root manifest (4096 bytes) -> extract hotset pointers
- Parse Level 1 TLV records -> build segment directory
- Write new manifest on mutation (two-fsync protocol)
- Manifest chain for rollback (OVERLAY_CHAIN record)

### `rvf-index`

**Key functionality:**
- Layer A: Entry points + top-layer adjacency (from INDEX_SEG with HOT flag)
- Layer B: Partial adjacency for hot region (built incrementally)
- Layer C: Full HNSW adjacency (built lazily in background)
- Varint delta-encoded neighbor lists with restart points
- Prefetch hints for cache-friendly traversal

**Integration with existing ruvector-core HNSW:**
- Wrap `hnsw_rs` graph as the in-memory structure
- Serialize HNSW to INDEX_SEG format
- Deserialize INDEX_SEG into `hnsw_rs` layers

### `rvf-quant`

**Key functionality:**
- Scalar quantization: fp32 -> int8 (4x compression)
- Product quantization: M subspaces, K centroids (8-16x compression)
- Binary quantization: sign bit (32x compression)
- QUANT_SEG read/write for codebooks
- Temperature tier assignment from SKETCH_SEG access counters

### `rvf-runtime`

**Key functionality:**
- `RvfStore::create()` / `RvfStore::open()` / `RvfStore::open_readonly()`
- Append-only write path (VEC_SEG + MANIFEST_SEG)
- Progressive load sequence (Level 0 -> hotset -> Level 1 -> on-demand)
- Background compaction (IO-budget-aware, priority-ordered)
- Count-Min Sketch maintenance for temperature decisions
- Promotion/demotion lifecycle

### Phase 2 Acceptance Criteria

- [ ] Progressive boot: parse Level 0 in < 1ms, first query in < 50ms (1M vectors)
- [ ] Recall@10 >= 0.70 with Layer A only
- [ ] Recall@10 >= 0.95 with all layers loaded
- [ ] Crash safety: kill -9 during write -> recover to last valid manifest
- [ ] Compaction reduces dead space while respecting IO budget
- [ ] Scalar quantization reconstruction error < 0.5%

---

## Phase 3: Integration — Library Adapters

### Agent Assignments

| Agent | Role | Target Library | Deliverable |
|-------|------|---------------|-------------|
| **coder-7** | claude-flow adapter | claude-flow memory | RVF-backed memory store |
| **coder-8** | agentdb adapter | agentdb | RVF as persistence backend |
| **coder-9** | agentic-flow adapter | agentic-flow | RVF streaming for inter-agent exchange |
| **coder-10** | rvlite adapter | rvlite | RVF Core Profile minimal store |

### claude-flow Memory -> RVF

```
Current: JSON flat files + in-memory HNSW
Target:  RVF file per memory namespace

Mapping:
  memory store  -> RvfStore with RVText profile
  memory search -> rvf_runtime.query()
  memory persist -> RVF append (VEC_SEG + META_SEG + MANIFEST_SEG)
  audit trail   -> WITNESS_SEG with hash chain
  session state -> META_SEG with TTL metadata
```

### agentdb -> RVF

```
Current: Custom HNSW + serde persistence
Target:  RVF file per database instance

Mapping:
  agentdb.insert()  -> rvf_runtime.ingest_batch()
  agentdb.search()  -> rvf_runtime.query()
  agentdb.persist() -> already persistent (append-only)
  HNSW graph        -> INDEX_SEG (Layer A/B/C)
  Metadata          -> META_SEG + METAIDX_SEG
```

### agentic-flow -> RVF

```
Current: Shared memory blobs between agents
Target:  RVF TCP streaming protocol

Mapping:
  agent memory share -> RVF SUBSCRIBE + UPDATE_NOTIFY
  swarm state        -> META_SEG in shared RVF file
  learning patterns  -> SKETCH_SEG for access tracking
  consensus state    -> WITNESS_SEG with signatures
```

### Phase 3 Acceptance Criteria

- [ ] claude-flow `memory store` and `memory search` work against RVF backend
- [ ] agentdb existing test suite passes with RVF storage (swap in, not rewrite)
- [ ] agentic-flow agents can share vectors through RVF streaming protocol
- [ ] Legacy format import tools for each library

---

## Phase 4: WASM + Node.js Bindings

### Agent Assignments

| Agent | Role | Target | Deliverable |
|-------|------|--------|-------------|
| **coder-11** | WASM microkernel | `crates/rvf/rvf-wasm/` | 14-export WASM module (<8 KB) |
| **coder-12** | WASM full runtime | `npm/packages/rvf-wasm/` | wasm-pack build, browser-compatible |
| **coder-13** | Node.js N-API | `crates/rvf/rvf-node/` | napi-rs bindings, platform packages |
| **coder-14** | TypeScript SDK | `npm/packages/rvf/` | TypeScript wrapper, types, docs |

### WASM Microkernel (`rvf-wasm` crate, `wasm32-unknown-unknown`)

```rust
// 14 exports matching spec (microkernel/wasm-runtime.md)
#[no_mangle] pub extern "C" fn rvf_init(config_ptr: i32) -> i32;
#[no_mangle] pub extern "C" fn rvf_load_query(query_ptr: i32, dim: i32) -> i32;
#[no_mangle] pub extern "C" fn rvf_load_block(block_ptr: i32, count: i32, dtype: i32) -> i32;
#[no_mangle] pub extern "C" fn rvf_distances(metric: i32, result_ptr: i32) -> i32;
#[no_mangle] pub extern "C" fn rvf_topk_merge(dist_ptr: i32, id_ptr: i32, count: i32, k: i32) -> i32;
#[no_mangle] pub extern "C" fn rvf_topk_read(out_ptr: i32) -> i32;
// ... remaining 8 exports
```

**Build command:**
```bash
cargo build --target wasm32-unknown-unknown --release -p rvf-wasm
wasm-opt -Oz -o rvf-microkernel.wasm target/wasm32-unknown-unknown/release/rvf_wasm.wasm
```

**Size budget:** Must be < 8 KB after wasm-opt.

### WASM Full Runtime (wasm-pack, browser)

```bash
cd crates/rvf/rvf-runtime
wasm-pack build --target web --features wasm
```

**npm package:** `@ruvector/rvf-wasm`

```typescript
// npm/packages/rvf-wasm/index.ts
import init, { RvfStore } from './pkg/rvf_runtime.js';

await init();
const store = RvfStore.fromBytes(rvfFileBytes);
const results = store.query(queryVector, 10);
```

### Node.js N-API Bindings (napi-rs)

```bash
cd crates/rvf/rvf-node
npm run build  # napi build --platform --release
```

**Platform packages:**

| Package | Target |
|---------|--------|
| `@ruvector/rvf-node` | Main package with postinstall platform select |
| `@ruvector/rvf-node-linux-x64-gnu` | Linux x86_64 glibc |
| `@ruvector/rvf-node-linux-arm64-gnu` | Linux aarch64 glibc |
| `@ruvector/rvf-node-darwin-arm64` | macOS Apple Silicon |
| `@ruvector/rvf-node-darwin-x64` | macOS Intel |
| `@ruvector/rvf-node-win32-x64-msvc` | Windows x64 |

### TypeScript SDK

```typescript
// npm/packages/rvf/src/index.ts
export class RvfDatabase {
  static async open(path: string): Promise<RvfDatabase>;
  static async create(path: string, options?: RvfOptions): Promise<RvfDatabase>;

  async insert(id: string, vector: Float32Array, metadata?: Record<string, unknown>): Promise<void>;
  async insertBatch(entries: RvfEntry[]): Promise<RvfIngestResult>;
  async query(vector: Float32Array, k: number, options?: RvfQueryOptions): Promise<RvfResult[]>;
  async delete(ids: string[]): Promise<RvfDeleteResult>;

  // Progressive loading
  async openProgressive(source: string | URL): Promise<RvfProgressiveReader>;
}

export interface RvfOptions {
  profile?: 'generic' | 'rvdna' | 'rvtext' | 'rvgraph' | 'rvvision';
  dimensions: number;
  metric?: 'l2' | 'cosine' | 'dotproduct' | 'hamming';
  compression?: 'none' | 'lz4' | 'zstd';
  signing?: { algorithm: 'ed25519' | 'ml-dsa-65'; key: Uint8Array };
}
```

### Phase 4 Acceptance Criteria

- [ ] WASM microkernel < 8 KB after wasm-opt
- [ ] WASM full runtime works in Chrome, Firefox, Node.js
- [ ] N-API bindings pass same test suite as Rust crate
- [ ] TypeScript types match Rust API surface
- [ ] All platform binaries build in CI

---

## Phase 5: Testing + Benchmarks

### Agent Assignments

| Agent | Role | Scope |
|-------|------|-------|
| **tester-3** | Acceptance tests | 10M vector cold start, recall, crash safety |
| **tester-4** | Benchmark harness | criterion benches, perf targets from spec |
| **tester-5** | Fuzz testing | cargo-fuzz for wire format parsing |
| **tester-6** | WASM tests | Browser + Cognitum tile simulation |

### Test Matrix

| Test Category | Description | Target |
|--------------|-------------|--------|
| **Round-trip** | Write + read all segment types | `rvf-wire` |
| **Progressive boot** | Cold start, measure recall at each phase | `rvf-runtime` |
| **Crash safety** | kill -9 during ingest/manifest/compaction | `rvf-runtime` |
| **Bit flip detection** | Random corruption -> hash/CRC catch | `rvf-wire` |
| **Recall benchmarks** | recall@10 at Layer A, B, C | `rvf-index` |
| **Latency benchmarks** | p50/p95/p99 query latency | `rvf-runtime` |
| **Throughput benchmarks** | QPS and ingest rate | `rvf-runtime` |
| **WASM performance** | Distance compute, top-K in WASM | `rvf-wasm` |
| **Interop** | agentdb/claude-flow/agentic-flow integration | adapters |
| **Profile compatibility** | Generic reader opens RVDNA/RVText files | `rvf-runtime` |

### Benchmark Commands

```bash
# Rust benchmarks
cd crates/rvf/rvf-runtime && cargo bench

# WASM benchmarks
cd npm/packages/rvf-wasm && npm run bench

# Node.js benchmarks
cd npm/packages/rvf-node && npm run bench

# Full acceptance test (10M vectors)
cd crates/rvf && cargo test --release --test acceptance -- --ignored
```

### Phase 5 Acceptance Criteria

- [ ] All performance targets from `benchmarks/acceptance-tests.md` met
- [ ] Zero data loss in crash safety tests (100 iterations)
- [ ] 100% bit-flip detection rate
- [ ] WASM microkernel passes Cognitum tile simulation
- [ ] No memory safety issues found by fuzz testing (1M iterations)

---

## Phase 6: Optimization + Publishing

### Agent Assignments

| Agent | Role | Scope |
|-------|------|-------|
| **optimizer-1** | SIMD tuning | AVX-512/NEON distance kernels, alignment |
| **optimizer-2** | Compression tuning | LZ4/ZSTD level selection, block size |
| **publisher-1** | crates.io publishing | Version management, dependency graph |
| **publisher-2** | npm publishing | Platform packages, wasm-pack output |

### SIMD Optimization Targets

| Operation | AVX-512 Target | NEON Target | WASM v128 Target |
|-----------|---------------|-------------|-----------------|
| L2 distance (384-dim fp16) | ~12 cycles | ~48 cycles | ~96 cycles |
| Dot product (384-dim fp16) | ~12 cycles | ~48 cycles | ~96 cycles |
| Hamming (384-bit) | 1 cycle (VPOPCNTDQ) | ~6 cycles (CNT) | ~24 cycles |
| PQ ADC (48 subspaces) | ~48 cycles (gather) | ~96 cycles (TBL) | ~192 cycles |

### Publishing Dependency Order

Crates must be published in dependency order:

```
1. rvf-types        (no deps)
2. rvf-wire          (depends on rvf-types)
3. rvf-quant         (depends on rvf-types)
4. rvf-manifest      (depends on rvf-types, rvf-wire)
5. rvf-index         (depends on rvf-types, rvf-wire, rvf-quant)
6. rvf-crypto        (depends on rvf-types, rvf-wire)
7. rvf-runtime       (depends on all above)
8. rvf-wasm          (depends on rvf-types, rvf-wire, rvf-quant)
9. rvf-node          (depends on rvf-runtime)
10. rvf-server       (depends on rvf-runtime)
```

### crates.io Publishing

```bash
# Publish in dependency order
for crate in rvf-types rvf-wire rvf-quant rvf-manifest rvf-index rvf-crypto rvf-runtime rvf-wasm rvf-node rvf-server; do
  cd crates/rvf/$crate
  cargo publish
  sleep 30  # Wait for crates.io index update
  cd -
done
```

### npm Publishing

```bash
# WASM package
cd npm/packages/rvf-wasm
npm publish --access public

# Node.js platform binaries
for platform in linux-x64-gnu linux-arm64-gnu darwin-arm64 darwin-x64 win32-x64-msvc; do
  cd npm/packages/rvf-node-$platform
  npm publish --access public
  cd -
done

# Main Node.js package
cd npm/packages/rvf-node
npm publish --access public

# TypeScript SDK
cd npm/packages/rvf
npm publish --access public
```

### Phase 6 Acceptance Criteria

- [ ] SIMD distance kernels meet cycle targets on each platform
- [ ] All crates published to crates.io with correct dependency graph
- [ ] All npm packages published with correct platform detection
- [ ] `npx rvf --version` works
- [ ] `npm install @ruvector/rvf` works on all supported platforms
- [ ] GitHub release with changelog

---

## Swarm Topology

```
                    ┌──────────────┐
                    │  Queen       │
                    │  Coordinator │
                    └──────┬───────┘
                           │
            ┌──────────────┼──────────────┐
            │              │              │
     ┌──────▼──────┐ ┌────▼─────┐ ┌──────▼──────┐
     │ Foundation  │ │  Runtime │ │ Integration │
     │ Squad       │ │  Squad   │ │ Squad       │
     │ (coder 1-2) │ │ (coder  │ │ (coder 7-10)│
     │ (tester-1)  │ │  3-6)   │ │             │
     │ (reviewer-1)│ │ (test-2)│ │             │
     └─────────────┘ └─────────┘ └─────────────┘
            │              │              │
            │    ┌─────────┼──────────┐   │
            │    │         │          │   │
            │  ┌─▼───────┐│┌─────────▼┐  │
            │  │ WASM +  │││ Testing  │  │
            │  │ Node    │││ Squad    │  │
            │  │ Squad   │││(tester   │  │
            │  │(coder   │││ 3-6)    │  │
            │  │ 11-14)  │││          │  │
            │  └─────────┘│└──────────┘  │
            │             │              │
            └─────────────┼──────────────┘
                    ┌─────▼──────┐
                    │ Optimize + │
                    │ Publish    │
                    │ Squad      │
                    └────────────┘
```

### Swarm Init Command

```bash
npx @claude-flow/cli@latest swarm init \
  --topology hierarchical \
  --max-agents 8 \
  --strategy specialized
```

### Agent Spawn Commands (via Claude Code Task tool)

All agents should be spawned as `run_in_background: true` Task calls in a single message. Each agent receives:

1. The relevant RVF spec files to read (from `docs/research/rvf/`)
2. The ADR-029 for context
3. The specific phase deliverables from this guidance
4. The acceptance criteria as exit conditions

---

## Critical Path

```
rvf-types ──> rvf-wire ──> rvf-manifest ──> rvf-runtime ──> adapters ──> publish
                 │                              │
                 └──> rvf-quant ────────────────┘
                 │                              │
                 └──> rvf-index ────────────────┘
                                                │
                 rvf-wasm (parallel) ───────────┘
                 rvf-node (parallel) ───────────┘
```

**Blocking dependencies:**
- Everything depends on `rvf-types`
- `rvf-wire` unlocks all other crates
- `rvf-runtime` blocks integration adapters
- `rvf-wasm` and `rvf-node` can proceed in parallel once `rvf-wire` exists

---

## File Layout Summary

```
crates/rvf/
  rvf-types/        # Segment types, headers, enums (no_std)
  rvf-wire/         # Wire format read/write (no_std + alloc)
  rvf-index/        # Progressive HNSW indexing
  rvf-manifest/     # Two-level manifest system
  rvf-quant/        # Temperature-tiered quantization
  rvf-crypto/       # ML-DSA-65, SHAKE-256
  rvf-runtime/      # Full runtime (RvfStore API)
  rvf-wasm/         # WASM microkernel (<8 KB)
  rvf-node/         # Node.js N-API bindings
  rvf-server/       # TCP/HTTP streaming server
  tests/            # Integration + acceptance tests
  benches/          # Criterion benchmarks

npm/packages/
  rvf/              # TypeScript SDK (@ruvector/rvf)
  rvf-wasm/         # Browser WASM (@ruvector/rvf-wasm)
  rvf-node/         # Node.js native (@ruvector/rvf-node)
  rvf-node-linux-x64-gnu/
  rvf-node-linux-arm64-gnu/
  rvf-node-darwin-arm64/
  rvf-node-darwin-x64/
  rvf-node-win32-x64-msvc/
```

---

## Success Metrics

| Metric | Target | Measured By |
|--------|--------|-------------|
| Cold boot time | < 5 ms | Phase 5 acceptance test |
| First query recall@10 | >= 0.70 | Phase 5 recall benchmark |
| Full recall@10 | >= 0.95 | Phase 5 recall benchmark |
| Query latency p50 | < 0.3 ms (10M vectors) | Phase 5 latency benchmark |
| WASM microkernel size | < 8 KB | Phase 4 build output |
| Crash safety | 0 data loss in 100 kill tests | Phase 5 crash test |
| Crates published | 10 crates on crates.io | Phase 6 publish |
| NPM packages published | 8+ packages on npm | Phase 6 publish |
| Library integration | 4 libraries using RVF | Phase 3 adapter tests |
