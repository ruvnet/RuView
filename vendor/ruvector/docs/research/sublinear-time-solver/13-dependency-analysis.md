# Agent 13: Dependency Graph & Compatibility Analysis

## Sublinear-Time-Solver Integration with RuVector

**Date**: 2026-02-20
**Scope**: Full dependency tree mapping, shared dependency identification, conflict resolution, feature flag compatibility, build system integration, bundle size impact, tree-shaking, and dependency management strategy.

---

## 1. Full Dependency Tree of RuVector

### 1.1 Workspace Overview

RuVector is a large Cargo workspace (`resolver = "2"`) containing **79 crate directories** under `/home/user/ruvector/crates/`, with **72 internal crates** resolved in `Cargo.lock` and **1,127 total packages** (including transitive dependencies). The NPM side has **53 packages** under `/home/user/ruvector/npm/packages/` plus a root-level npm workspace.

**Workspace version**: `2.0.3` (Rust edition 2021, rust-version 1.77)

### 1.2 Cargo Workspace Members (Tier-1 -- Direct Members)

The main workspace has 100 members defined in `/home/user/ruvector/Cargo.toml`. Key crate families:

| Family | Crates | Role |
|--------|--------|------|
| **ruvector-core** | `ruvector-core` | Vector database core, HNSW indexing, SIMD distance metrics |
| **WASM bindings** | `ruvector-wasm`, `ruvector-graph-wasm`, `ruvector-gnn-wasm`, `ruvector-attention-wasm`, `ruvector-mincut-wasm`, `ruvector-delta-wasm`, `ruvector-domain-expansion-wasm`, `ruvector-economy-wasm`, `ruvector-learning-wasm`, `ruvector-exotic-wasm`, `ruvector-attention-unified-wasm`, `ruvector-fpga-transformer-wasm`, `ruvector-sparse-inference-wasm`, `ruvector-temporal-tensor-wasm`, `ruvector-math-wasm`, `ruvector-nervous-system-wasm`, `ruvector-dag-wasm` | Browser/edge WASM targets |
| **Node.js bindings** | `ruvector-node`, `ruvector-graph-node`, `ruvector-gnn-node`, `ruvector-attention-node`, `ruvector-mincut-node`, `ruvector-tiny-dancer-node` | N-API native bindings |
| **Graph** | `ruvector-graph`, `ruvector-graph-wasm`, `ruvector-graph-node` | Distributed hypergraph database |
| **GNN** | `ruvector-gnn`, `ruvector-gnn-wasm`, `ruvector-gnn-node` | Graph Neural Network layer |
| **Attention** | `ruvector-attention`, `ruvector-attention-wasm`, `ruvector-attention-node`, `ruvector-attention-unified-wasm` | Geometric/sparse/topology-gated attention |
| **Min-Cut** | `ruvector-mincut`, `ruvector-mincut-wasm`, `ruvector-mincut-node`, `ruvector-mincut-gated-transformer`, `ruvector-mincut-gated-transformer-wasm` | Subpolynomial dynamic minimum cut |
| **Delta** | `ruvector-delta-core`, `ruvector-delta-wasm`, `ruvector-delta-index`, `ruvector-delta-graph`, `ruvector-delta-consensus` | Behavioral vector change tracking |
| **CLI/Server** | `ruvector-cli`, `ruvector-server`, `ruvector-router-cli`, `ruvector-router-core`, `ruvector-router-ffi`, `ruvector-router-wasm` | REST API, MCP, neural routing |
| **Infrastructure** | `ruvector-cluster`, `ruvector-raft`, `ruvector-replication`, `ruvector-postgres`, `ruvector-snapshot` | Distributed consensus, storage |
| **Math** | `ruvector-math`, `ruvector-math-wasm` | Optimal Transport, Information Geometry, Product Manifolds |
| **Neural** | `ruvector-nervous-system`, `ruvector-nervous-system-wasm` | Bio-inspired spiking networks, BTSP, EWC |
| **SONA** | `sona` (ruvector-sona) | Self-Optimizing Neural Architecture, LoRA, ReasoningBank |
| **Prime Radiant** | `prime-radiant` | Sheaf Laplacian coherence engine |
| **RuVLLM** | `ruvllm`, `ruvllm-cli`, `ruvllm-wasm` | LLM serving runtime |
| **Cognitum Gate** | `cognitum-gate-kernel`, `cognitum-gate-tilezero`, `mcp-gate` | WASM coherence fabric |
| **RuQu** | `ruqu`, `ruqu-core`, `ruqu-algorithms`, `ruqu-wasm`, `ruqu-exotic` | Quantum coherence assessment |
| **Domain Expansion** | `ruvector-domain-expansion`, `ruvector-domain-expansion-wasm` | Cross-domain transfer learning |
| **FPGA Transformer** | `ruvector-fpga-transformer`, `ruvector-fpga-transformer-wasm` | FPGA deterministic inference |
| **Sparse Inference** | `ruvector-sparse-inference`, `ruvector-sparse-inference-wasm` | PowerInfer-style edge inference |
| **Temporal Tensor** | `ruvector-temporal-tensor`, `ruvector-temporal-tensor-wasm` | Temporal tensor compression |
| **Tiny Dancer** | `ruvector-tiny-dancer-core`, `ruvector-tiny-dancer-wasm`, `ruvector-tiny-dancer-node` | Compact runtime |
| **RVF** | 20+ sub-crates in `crates/rvf/` | RuVector Format container system |
| **RVLite** | `rvlite` | Standalone WASM vector database |
| **CRV** | `ruvector-crv` | Signal line protocol integration |
| **DAG** | `ruvector-dag`, `ruvector-dag-wasm` | Directed acyclic graph structures |
| **Utilities** | `ruvector-bench`, `ruvector-metrics`, `ruvector-filter`, `ruvector-collections` | Benchmarking, metrics, filtering |

### 1.3 Workspace-Level Dependency Pinning

Workspace dependencies defined in the root `Cargo.toml` (versions resolved from `Cargo.lock`):

| Category | Dependency | Workspace Spec | Lockfile Version |
|----------|-----------|----------------|-----------------|
| **Storage** | redb | 2.1 | 2.1.x |
| **Storage** | memmap2 | 0.9 | 0.9.x |
| **Indexing** | hnsw_rs | 0.3 (patched) | 0.3.x (local) |
| **SIMD** | simsimd | 5.9 | 5.9.x |
| **Parallelism** | rayon | 1.10 | 1.11.0 |
| **Parallelism** | crossbeam | 0.8 | 0.8.x |
| **Serialization** | rkyv | 0.8 | 0.8.x |
| **Serialization** | bincode | 2.0.0-rc.3 | 2.0.0-rc.3 |
| **Serialization** | serde | 1.0 | 1.0.228 |
| **Serialization** | serde_json | 1.0 | 1.0.145 |
| **Node.js** | napi | 2.16 | 2.16.x |
| **Node.js** | napi-derive | 2.16 | 2.16.x |
| **WASM** | wasm-bindgen | 0.2 | 0.2.106 |
| **WASM** | wasm-bindgen-futures | 0.4 | 0.4.x |
| **WASM** | js-sys | 0.3 | 0.3.x |
| **WASM** | web-sys | 0.3 | 0.3.x |
| **WASM** | getrandom | 0.3 | 0.3.4 |
| **Async** | tokio | 1.41 | 1.48.0 |
| **Async** | futures | 0.3 | 0.3.x |
| **Errors** | thiserror | 2.0 | 1.0.69 + 2.0.17 (both) |
| **Errors** | anyhow | 1.0 | 1.0.x |
| **Tracing** | tracing | 0.1 | 0.1.x |
| **Tracing** | tracing-subscriber | 0.3 | 0.3.x |
| **Math** | ndarray | 0.16 | 0.16.x |
| **Math** | rand | 0.8 | 0.8.5 (also 0.6.5, 0.9.2) |
| **Math** | rand_distr | 0.4 | 0.4.x |
| **Time** | chrono | 0.4 | 0.4.x |
| **UUID** | uuid | 1.11 | 1.19.0 |
| **CLI** | clap | 4.5 | 4.5.53 |
| **CLI** | indicatif | 0.17 | 0.17.x |
| **CLI** | console | 0.15 | 0.15.x |
| **Performance** | dashmap | 6.1 | 6.1.x |
| **Performance** | parking_lot | 0.12 | 0.12.x |
| **Performance** | once_cell | 1.20 | 1.20.x |
| **Testing** | criterion | 0.5 | 0.5.x |
| **Testing** | proptest | 1.5 | 1.5.x |
| **Testing** | mockall | 0.13 | 0.13.x |

### 1.4 Non-Workspace Dependencies (Crate-Specific)

Key dependencies pulled in by individual crates, outside workspace management:

| Crate | Dependency | Version |
|-------|-----------|---------|
| `ruvector-math` | **nalgebra** | 0.33 |
| `prime-radiant` | **nalgebra** | 0.33 |
| `prime-radiant` | **wide** | 0.7 |
| `ruvector-graph` | petgraph | 0.6 |
| `ruvector-graph` | roaring | 0.10 |
| `ruvector-graph` | nom/nom_locate | 7.1/4.2 |
| `ruvector-graph` | tonic/prost | 0.12/0.13 |
| `ruvector-cli` | axum | 0.7 |
| `ruvector-cli` | colored | 2.1 |
| `ruvector-server` | axum | 0.7 |
| `ruvector-server` | tower-http | 0.6 |
| `ruvector-wasm` | serde-wasm-bindgen | 0.6 |
| `ruvector-wasm` | console_error_panic_hook | 0.1 |
| `ruvector-fpga-transformer` | ed25519-dalek | 2.1 |
| `ruvector-fpga-transformer` | sha2 | 0.10 |
| `ruvllm` | candle-core/nn/transformers | 0.8 |
| `ruvllm` | tokenizers | 0.20 |
| `ruvector-delta-core` | smallvec | 1.13 |
| `ruvector-delta-core` | arrayvec | 0.7 |
| `ruqu` | blake3 | 1.5 |
| `ruqu` | ed25519-dalek | 2.1 |
| `ruqu` | petgraph | 0.6 |
| `cognitum-gate-kernel` | libm | 0.2 |

### 1.5 NPM Dependency Tree

Root `/home/user/ruvector/package.json`:
- `@claude-flow/memory` ^3.0.0-alpha.7

NPM workspace `/home/user/ruvector/npm/package.json`:
- devDeps: `@types/node`, `@typescript-eslint/*`, `eslint`, `prettier`, `typescript`

Key NPM packages:

| Package | Dependencies |
|---------|-------------|
| `@ruvector/core` | Platform-specific native binaries, `@napi-rs/cli` |
| `@ruvector/node` | `@ruvector/core`, `@ruvector/gnn` |
| `@ruvector/cli` | `commander`, optional `pg` |
| `ruvector` (unified) | `@modelcontextprotocol/sdk`, `@ruvector/attention`, `@ruvector/core`, `@ruvector/gnn`, `@ruvector/sona`, `chalk`, `commander`, `ora` |
| `@ruvector/rvf-mcp-server` | `@modelcontextprotocol/sdk`, `@ruvector/rvf`, `express`, `zod` |
| `@ruvector/agentic-integration` | `express`, `fastify`, `ioredis`, `pg`, `uuid`, `zod`, `claude-flow`, `axios`, Google Cloud SDKs |

### 1.6 Excluded/Separate Workspaces

The following are **excluded** from the main workspace and have their own `Cargo.lock`:
- `crates/micro-hnsw-wasm`
- `crates/ruvector-hyperbolic-hnsw` and `ruvector-hyperbolic-hnsw-wasm`
- `crates/rvf/` (has its own workspace, rust-version 1.87)
- `examples/ruvLLM/esp32` and `esp32-flash`
- `examples/edge-net`, `examples/data`, `examples/delta-behavior`

### 1.7 Patch Registry

The workspace applies one crate patch:
```toml
[patch.crates-io]
hnsw_rs = { path = "./patches/hnsw_rs" }
```
This patches `hnsw_rs` to use `rand 0.8` instead of `rand 0.9` for WASM compatibility, resolving the `getrandom` 0.2 vs 0.3 conflict.

---

## 2. Shared Dependencies with Sublinear-Time-Solver

### 2.1 Rust Dependency Overlap Matrix

| Sublinear-Time-Solver Dep | Version Required | RuVector Version | Status | Location in RuVector |
|---------------------------|-----------------|-----------------|--------|---------------------|
| **nalgebra** | 0.32 | 0.32.6 + 0.33.2 | PARTIAL MATCH | `ruvector-math` (0.33), transitive (0.32.6 in lockfile) |
| **serde** | (any 1.x) | 1.0.228 | COMPATIBLE | Workspace dep, ubiquitous |
| **thiserror** | (any) | 1.0.69 + 2.0.17 | COMPATIBLE | Workspace dep (2.0), some crates pin 1.0 |
| **log** | (any 0.4) | 0.4.29 | COMPATIBLE | Transitive, present in lockfile |
| **rand** | (any 0.8) | 0.8.5 | COMPATIBLE | Workspace dep, used everywhere |
| **fnv** | (any 1.x) | 1.0.7 | COMPATIBLE | Transitive, present in lockfile |
| **num-traits** | (any 0.2) | 0.2.19 | COMPATIBLE | Transitive via nalgebra/ndarray |
| **num-complex** | (any) | 0.2.4 + 0.4.6 | COMPATIBLE | Transitive, both versions present |
| **bit-set** | (any) | 0.5.3 + 0.8.0 | COMPATIBLE | Transitive, both versions present |
| **lazy_static** | (any 1.x) | 1.5.0 | COMPATIBLE | Transitive, present in lockfile |

### 2.2 WASM Dependency Overlap Matrix

| Sublinear-Time-Solver Dep | Version Required | RuVector Version | Status | Location in RuVector |
|---------------------------|-----------------|-----------------|--------|---------------------|
| **wasm-bindgen** | 0.2 | 0.2.106 | COMPATIBLE | Workspace dep |
| **web-sys** | 0.3 | 0.3.x | COMPATIBLE | Workspace dep |
| **js-sys** | 0.3 | 0.3.x | COMPATIBLE | Workspace dep |
| **serde-wasm-bindgen** | (any 0.6) | 0.6.5 | COMPATIBLE | Used in ruvector-wasm, rvlite |
| **console_error_panic_hook** | 0.1 | 0.1.7 | COMPATIBLE | Used in ruvector-wasm, rvlite |
| **getrandom** | (WASM) | 0.2.16 + 0.3.4 | SEE SECTION 3 | Both versions present, managed carefully |

### 2.3 CLI Dependency Overlap Matrix

| Sublinear-Time-Solver Dep | Version Required | RuVector Version | Status | Location in RuVector |
|---------------------------|-----------------|-----------------|--------|---------------------|
| **clap** | (any 4.x) | 4.5.53 | COMPATIBLE | Workspace dep |
| **tokio** | (any 1.x) | 1.48.0 | COMPATIBLE | Workspace dep |
| **axum** | (any 0.7) | 0.7.9 | COMPATIBLE | ruvector-cli, ruvector-server |
| **serde_json** | (any 1.x) | 1.0.145 | COMPATIBLE | Workspace dep |
| **uuid** | (any 1.x) | 1.19.0 | COMPATIBLE | Workspace dep |
| **colored** | (any 2.x) | 2.2.0 | COMPATIBLE | ruvector-cli |

### 2.4 Server Dependency Overlap Matrix (NPM)

| Sublinear-Time-Solver Dep | Version Required | RuVector Version | Status | Location in RuVector |
|---------------------------|-----------------|-----------------|--------|---------------------|
| **express** | (any 4.x) | ^4.18.0 | COMPATIBLE | rvf-mcp-server, agentic-integration |
| **cors** | -- | -- | NOT PRESENT | RuVector uses `tower-http` cors on Rust side |
| **helmet** | -- | -- | NOT PRESENT | Not used in any npm package |
| **compression** | -- | -- | NOT PRESENT | RuVector uses `tower-http` compression on Rust side |

### 2.5 Performance Dependency Overlap Matrix

| Sublinear-Time-Solver Dep | Version Required | RuVector Version | Status | Location in RuVector |
|---------------------------|-----------------|-----------------|--------|---------------------|
| **wide** (SIMD) | (any 0.7) | 0.7.33 | COMPATIBLE | prime-radiant (optional) |
| **rayon** | (any 1.x) | 1.11.0 | COMPATIBLE | Workspace dep, broadly used |

### 2.6 NPM Dependency Overlap Matrix

| Sublinear-Time-Solver Dep | Version Required | RuVector Version | Status | Location in RuVector |
|---------------------------|-----------------|-----------------|--------|---------------------|
| **@modelcontextprotocol/sdk** | (any 1.x) | ^1.0.0 | COMPATIBLE | ruvector unified pkg, rvf-mcp-server |
| **@ruvnet/strange-loop** | -- | -- | NOT PRESENT | New dependency |
| **strange-loops** | -- | -- | NOT PRESENT | New dependency |

### 2.7 Summary: 22 of 26 Dependencies are Shared or Compatible

- **Fully shared (same version range)**: 18 dependencies -- serde, thiserror, log, rand, fnv, num-traits, lazy_static, wasm-bindgen, web-sys, js-sys, serde-wasm-bindgen, console_error_panic_hook, clap, tokio, axum, serde_json, uuid, rayon
- **Compatible with minor version management**: 4 -- nalgebra (0.32 vs 0.33), colored, wide, express
- **Needs new integration**: 4 -- `cors`, `helmet`, `compression` (npm), `@ruvnet/strange-loop`, `strange-loops`
- **Requires careful handling**: 1 -- `getrandom` (dual 0.2/0.3)

---

## 3. Version Conflicts and Resolution Strategies

### 3.1 CRITICAL: nalgebra 0.32 vs 0.33

**Conflict**: Sublinear-time-solver requires `nalgebra 0.32`. RuVector's `ruvector-math` and `prime-radiant` use `nalgebra 0.33`.

**Current state**: The lockfile already contains *both* `nalgebra 0.32.6` and `nalgebra 0.33.2`. This means some transitive dependency (likely from the `hnsw_rs` patch or `ndarray`) already pulls in 0.32.

**Resolution strategy**:
1. **Dual-version coexistence (RECOMMENDED)**: Cargo natively supports multiple semver-incompatible versions. The sublinear-time-solver crate can depend on `nalgebra = "0.32"` while the rest of the workspace uses 0.33. Cargo will compile both and link them separately. No source changes needed.
2. **Upgrade sublinear-time-solver to nalgebra 0.33**: If the solver's nalgebra usage is limited (matrix operations, type aliases), this is a low-risk upgrade. The 0.32->0.33 API is largely backward-compatible. This eliminates duplicate compilation.
3. **Thin adapter layer**: Create a `sublinear-solver-types` crate that re-exports nalgebra types, allowing a single version.

**Recommendation**: Start with option 1 (dual coexistence) for immediate integration, then migrate to option 2 as a follow-up.

### 3.2 IMPORTANT: getrandom 0.2 vs 0.3

**Conflict**: RuVector workspace pins `getrandom 0.3` with `wasm_js` feature. However, many crates (sona, rvlite, fpga-transformer) explicitly use `getrandom 0.2` with the `js` feature. Sublinear-time-solver uses `getrandom` via WASM feature flags.

**Current state**: The lockfile has both `getrandom 0.2.16` and `getrandom 0.3.4`. The workspace already manages this dual-version scenario via the patched `hnsw_rs` and explicit `getrandom02` aliases in `ruvector-wasm`.

**Resolution strategy**: No action needed. The existing dual-version approach works. The sublinear-time-solver should use whichever getrandom version it needs, and Cargo will resolve correctly. For WASM targets, ensure `features = ["js"]` (0.2) or `features = ["wasm_js"]` (0.3) is set.

### 3.3 MODERATE: thiserror 1.0 vs 2.0

**Conflict**: The workspace declares `thiserror = "2.0"`, but several crates (`ruvector-attention`, `ruvector-crv`, `rvlite`, `mcp-gate`, `cognitum-gate-kernel` via transitive) still use `thiserror = "1.0"`. The lockfile contains both 1.0.69 and 2.0.17.

**Resolution strategy**: Cargo handles this automatically since 1.x and 2.x are semver-incompatible. The sublinear-time-solver can use either version. If it uses `thiserror 1.x`, it will coexist with the workspace's 2.0. No action needed.

### 3.4 MODERATE: rand Version Fragmentation

**Conflict**: The lockfile contains `rand 0.6.5`, `rand 0.8.5`, and `rand 0.9.2`. The workspace standardizes on `rand 0.8`. The sublinear-time-solver also uses `rand 0.8`.

**Resolution strategy**: No conflict. The solver will unify with the workspace's `rand 0.8.5`. The 0.6 and 0.9 versions are pulled by specific transitive dependencies and will not interfere.

### 3.5 LOW: num-complex Dual Versions

**Current state**: `num-complex 0.2.4` and `0.4.6` both present. These are transitive and do not affect the solver integration.

### 3.6 LOW: Express Server Middleware

**Non-conflict**: Sublinear-time-solver's server needs `cors`, `helmet`, and `compression` npm packages. RuVector handles these on the Rust side via `tower-http` (CORS, compression) in `ruvector-server` and `ruvector-cli`. The npm packages simply need to be added to the solver's `package.json`. No version conflict.

---

## 4. Feature Flag Compatibility Matrix

### 4.1 RuVector Feature Flag Architecture

RuVector uses a layered feature flag system to support multiple build targets:

| Target | Feature Pattern | Key Flags |
|--------|----------------|-----------|
| **Native (full)** | `default` includes storage, SIMD, parallel, HNSW | `simd`, `storage`, `hnsw`, `parallel`, `api-embeddings` |
| **WASM (browser)** | `default-features = false` + `memory-only` | `memory-only`, `wasm`, no storage/SIMD |
| **Node.js (N-API)** | Full native with N-API bindings | `napi`, full features |
| **no_std** | `cognitum-gate-kernel` supports `no_std` | `std` (optional) |

### 4.2 Sublinear-Time-Solver Feature Compatibility

The solver must support three build targets. Here is the feature flag mapping:

| Solver Target | Solver Deps | RuVector Compatible Features | Notes |
|---------------|------------|------------------------------|-------|
| **Rust library** | nalgebra, serde, thiserror, log, rand, fnv, num-traits, num-complex, bit-set, lazy_static | `ruvector-core/default`, `ruvector-math/default`, `ruvector-mincut/default` | Full native build, SIMD ok |
| **WASM** | wasm-bindgen, web-sys, js-sys, serde-wasm-bindgen, console_error_panic_hook, getrandom | `ruvector-core/memory-only`, `ruvector-wasm` features | No storage, no SIMD, no parallel |
| **CLI** | clap, tokio, axum, serde_json, uuid, colored | `ruvector-cli` feature set | Full async runtime |
| **Server** | express, cors, helmet, compression | `@ruvector/rvf-mcp-server` pattern | NPM side only |
| **Performance** | wide (SIMD), rayon | `prime-radiant/simd`, workspace `rayon` | Conditional on target |

### 4.3 Recommended Feature Flags for Sublinear-Time-Solver

```toml
[features]
default = ["std"]

# Core features
std = []
simd = ["wide"]              # Matches prime-radiant/simd
parallel = ["rayon"]          # Matches ruvector workspace rayon
serde = ["dep:serde"]         # Matches workspace serde

# WASM target
wasm = [
    "wasm-bindgen",
    "web-sys",
    "js-sys",
    "serde-wasm-bindgen",
    "console_error_panic_hook",
    "getrandom/wasm_js",       # Use 0.3 style if possible
]

# CLI target
cli = ["clap", "tokio", "axum", "colored"]

# RuVector integration
ruvector = ["dep:ruvector-core", "dep:ruvector-mincut"]
ruvector-math = ["dep:ruvector-math"]
ruvector-full = ["ruvector", "ruvector-math", "dep:ruvector-graph"]
```

### 4.4 Feature Compatibility Conflicts

| Feature Combination | Issue | Resolution |
|--------------------|-------|------------|
| `wasm` + `simd` | `wide` crate may not compile for `wasm32-unknown-unknown` without SIMD proposal | Gate behind `cfg(target_feature = "simd128")` or use separate `wasm-simd` feature |
| `wasm` + `parallel` | `rayon` does not work in WASM | Make `parallel` mutually exclusive with `wasm` |
| `wasm` + `cli` | CLI features require full OS, not browser | Make `cli` mutually exclusive with `wasm` |
| `ruvector` + `wasm` | Must use `ruvector-core` with `default-features = false, features = ["memory-only"]` | Conditional dependency in Cargo.toml |

---

## 5. Build System Integration

### 5.1 Cargo Workspace Integration

**Strategy**: Add the sublinear-time-solver as a new member of the RuVector Cargo workspace.

```toml
# In /home/user/ruvector/Cargo.toml [workspace] members:
members = [
    # ... existing members ...
    "crates/sublinear-time-solver",
]
```

**Workspace dependency additions** (in `[workspace.dependencies]`):

```toml
# New dependencies for sublinear-time-solver
# nalgebra 0.32 -- NOT added to workspace (let solver pin its own version)
# The following are already in workspace:
# serde, thiserror, rand, wasm-bindgen, web-sys, js-sys, clap, tokio, axum,
# serde_json, uuid, rayon, getrandom, console_error_panic_hook

# New workspace additions needed:
fnv = "1.0"
num-traits = "0.2"
num-complex = "0.4"
bit-set = "0.8"
lazy_static = "1.5"
log = "0.4"
wide = "0.7"                    # Already used by prime-radiant, promote to workspace
colored = "2.2"                 # Already used by ruvector-cli, promote to workspace
serde-wasm-bindgen = "0.6"      # Already used by ruvector-wasm, promote to workspace
console_error_panic_hook = "0.1" # Already used, promote to workspace
```

**Crate Cargo.toml** for the solver:

```toml
[package]
name = "sublinear-time-solver"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
# Math (solver pins nalgebra 0.32 separately)
nalgebra = { version = "0.32", default-features = false, features = ["std"] }
num-traits = { workspace = true }
num-complex = { workspace = true }

# Core
serde = { workspace = true }
thiserror = { workspace = true }
log = { workspace = true }
rand = { workspace = true }
fnv = { workspace = true }
bit-set = { workspace = true }
lazy_static = { workspace = true }

# WASM (optional)
wasm-bindgen = { workspace = true, optional = true }
web-sys = { workspace = true, optional = true }
js-sys = { workspace = true, optional = true }
serde-wasm-bindgen = { workspace = true, optional = true }
console_error_panic_hook = { workspace = true, optional = true }
getrandom = { workspace = true, optional = true }

# CLI (optional)
clap = { workspace = true, optional = true }
tokio = { workspace = true, optional = true }
axum = { workspace = true, optional = true }
serde_json = { workspace = true, optional = true }
uuid = { workspace = true, optional = true }
colored = { workspace = true, optional = true }

# Performance (optional)
wide = { workspace = true, optional = true }
rayon = { workspace = true, optional = true }

# RuVector integration (optional)
ruvector-core = { path = "../ruvector-core", default-features = false, optional = true }
ruvector-mincut = { path = "../ruvector-mincut", default-features = false, optional = true }
ruvector-math = { path = "../ruvector-math", default-features = false, optional = true }
```

### 5.2 NPM Workspace Integration

**Strategy**: Add solver's JavaScript server package to the NPM workspace.

In `/home/user/ruvector/npm/package.json`:
```json
{
  "workspaces": [
    "packages/*"
  ]
}
```

The solver's npm package would live at `/home/user/ruvector/npm/packages/sublinear-solver/` and would be included automatically by the `packages/*` glob.

**Solver's package.json**:
```json
{
  "name": "@ruvector/sublinear-solver",
  "version": "0.1.0",
  "dependencies": {
    "@modelcontextprotocol/sdk": "^1.0.0",
    "express": "^4.18.0",
    "cors": "^2.8.5",
    "helmet": "^7.0.0",
    "compression": "^1.7.4"
  }
}
```

### 5.3 Build Pipeline Integration

Existing build scripts in `/home/user/ruvector/package.json`:

```json
"build": "cargo build --release",
"build:wasm": "cd crates/ruvector-wasm && npm run build",
"test": "cargo test --workspace"
```

The solver integrates into these without modification because:
1. `cargo build --release` will build all workspace members including the solver
2. `cargo test --workspace` will test the solver
3. WASM build needs a new script: `"build:solver-wasm": "cd crates/sublinear-time-solver && wasm-pack build --target web"`

### 5.4 RVF Sub-Workspace Consideration

The `crates/rvf/` directory has its own workspace (rust-version 1.87). If the solver needs RVF integration, it should be added to the **main** workspace (not the RVF sub-workspace), and use path dependencies like `rvf-types` and `rvf-wire` as `ruvector-domain-expansion` does.

---

## 6. Bundle Size Impact Analysis

### 6.1 Rust Binary Size Impact

Based on the solver's dependency profile, estimated incremental impact on compiled binary size:

| Dependency | Estimated Size (release) | Already in RuVector? | Incremental Cost |
|-----------|-------------------------|---------------------|-----------------|
| nalgebra 0.32 | ~1.5 MB | 0.33 exists (separate) | +1.5 MB (dual version) |
| serde | ~300 KB | Yes | 0 KB |
| thiserror | ~50 KB | Yes | 0 KB |
| log | ~30 KB | Yes | 0 KB |
| rand | ~200 KB | Yes | 0 KB |
| fnv | ~10 KB | Yes | 0 KB |
| num-traits | ~100 KB | Yes | 0 KB |
| num-complex | ~80 KB | Partial | ~40 KB |
| bit-set | ~20 KB | Yes | 0 KB |
| lazy_static | ~10 KB | Yes | 0 KB |
| wide (SIMD) | ~200 KB | Yes (prime-radiant) | 0 KB |
| rayon | ~500 KB | Yes | 0 KB |
| **Solver logic** | ~500 KB-2 MB | No | +500 KB - 2 MB |
| **TOTAL incremental** | | | **~2-3.5 MB** |

With LTO (`lto = "fat"`) and `codegen-units = 1` (already configured in workspace), dead code elimination will reduce this significantly.

### 6.2 WASM Bundle Size Impact

WASM builds are more size-sensitive. Estimated `.wasm` file size impact:

| Component | Size (opt-level "z", wasm-opt) | Notes |
|-----------|-------------------------------|-------|
| Current ruvector-wasm | ~300-500 KB | Memory-only mode |
| nalgebra 0.32 (WASM) | ~200-400 KB | Depends on feature usage |
| Solver core logic | ~100-300 KB | Algorithmic code compresses well |
| wasm-bindgen glue | ~20 KB | Already present, shared |
| serde-wasm-bindgen | ~30 KB | Already present, shared |
| **Total solver WASM** | ~350-750 KB | Standalone |
| **Incremental if bundled** | ~300-700 KB | Shared deps deduplicated |

**Mitigation strategies**:
1. Use `opt-level = "z"` (already configured in ruvector-wasm)
2. Use `panic = "abort"` (already configured)
3. Enable `wasm-opt` post-processing
4. Use `#[wasm_bindgen]` only on the public API surface
5. Consider splitting solver-wasm into its own `.wasm` module (lazy loading)

### 6.3 NPM Package Size Impact

| Package | Current Size | With Solver | Notes |
|---------|-------------|-------------|-------|
| `express` | ~200 KB | 0 KB (reused) | Already in rvf-mcp-server |
| `cors` | ~15 KB | +15 KB | New |
| `helmet` | ~30 KB | +30 KB | New |
| `compression` | ~20 KB | +20 KB | New |
| `@modelcontextprotocol/sdk` | ~100 KB | 0 KB (reused) | Already in ruvector pkg |
| Solver JS glue | ~50-100 KB | +50-100 KB | TypeScript wrapper |
| **NPM incremental** | | **~115-165 KB** | |

---

## 7. Tree-Shaking and Dead Code Elimination

### 7.1 Rust/Cargo Dead Code Elimination

Cargo with `lto = "fat"` and `codegen-units = 1` (both configured in workspace release profile) enables aggressive dead code elimination:

**What gets eliminated**:
- Unused nalgebra matrix sizes and operations (nalgebra uses generics heavily)
- Unused serde derive implementations for types not serialized
- Unused feature-gated code paths
- Unused rayon parallel iterators

**What does NOT get eliminated**:
- Generic monomorphizations that are instantiated
- `#[no_mangle]` FFI exports
- `wasm_bindgen` exports
- Panic formatting strings (mitigated by `panic = "abort"` in WASM)

**Effectiveness estimate**: 40-60% reduction from naive compilation. The solver's nalgebra usage will likely instantiate only a few matrix sizes (e.g., `DMatrix<f64>`), so most of nalgebra's type machinery gets eliminated.

### 7.2 Feature Flag Tree-Shaking

The feature flag design from Section 4.3 enables compile-time elimination:

| Build Target | Compiled Deps | Eliminated |
|-------------|--------------|------------|
| Rust library only | nalgebra, serde, thiserror, log, rand, fnv, num-traits, num-complex, bit-set, lazy_static | wasm-bindgen, web-sys, js-sys, clap, tokio, axum, colored, express, etc. |
| WASM only | Core + wasm-bindgen, web-sys, js-sys, serde-wasm-bindgen, console_error_panic_hook | rayon, wide, clap, tokio, axum, storage deps |
| CLI only | Core + clap, tokio, axum, serde_json, uuid, colored | wasm deps |
| Server (NPM) only | Core WASM + express, cors, helmet, compression | Native-only deps |

### 7.3 WASM Tree-Shaking

For WASM builds, additional tree-shaking happens at two levels:

1. **wasm-pack/wasm-opt level**: Removes unreachable WASM functions. Typically 10-30% size reduction.
2. **JavaScript bundler level** (webpack/rollup/vite): Tree-shakes the JS glue code. The solver should export via ESM (`"type": "module"`) to enable this.

**Recommendations**:
- Use `#[wasm_bindgen(skip)]` on internal types
- Avoid `wasm_bindgen` on large enum variants that are never exposed
- Use `serde-wasm-bindgen` instead of `JsValue::from_serde` (already the pattern in ruvector-wasm)
- Configure `wasm-opt = ["-Oz"]` for production builds

### 7.4 NPM Bundle Tree-Shaking

The solver's NPM package should use:
- ESM exports with `"type": "module"` and `"exports"` field
- Side-effect-free annotation: `"sideEffects": false`
- Separate entry points for server vs. WASM usage

```json
{
  "exports": {
    ".": "./dist/index.js",
    "./wasm": "./dist/wasm/index.js",
    "./server": "./dist/server/index.js"
  },
  "sideEffects": false
}
```

---

## 8. Recommended Dependency Management Strategy

### 8.1 Integration Architecture

```
/home/user/ruvector/
  Cargo.toml                          # Main workspace - add solver as member
  crates/
    sublinear-time-solver/            # NEW: Solver Rust crate
      Cargo.toml
      src/
        lib.rs                        # Core solver library
        wasm.rs                       # WASM bindings (feature-gated)
    sublinear-time-solver-wasm/       # NEW: Separate WASM crate (optional)
      Cargo.toml
  npm/
    packages/
      sublinear-solver/              # NEW: NPM package for server/JS
        package.json
        src/
          index.ts
          server.ts
```

### 8.2 Dependency Governance Rules

1. **All shared dependencies must use workspace versions**: The solver should use `{ workspace = true }` for every dependency that is already in the workspace `[workspace.dependencies]` section. This prevents version drift and reduces duplicate compilation.

2. **nalgebra is the exception**: Pin `nalgebra = "0.32"` directly in the solver's `Cargo.toml` since the workspace uses 0.33. Plan migration to 0.33 within 1-2 release cycles.

3. **Promote commonly-used deps to workspace level**: Move `wide`, `colored`, `serde-wasm-bindgen`, `console_error_panic_hook`, `fnv`, `log`, `lazy_static` into `[workspace.dependencies]` since they are now used by 2+ crates.

4. **Feature flags must be additive**: Never use `default-features = true` for cross-crate dependencies within the workspace. Each consumer specifies exactly the features it needs.

5. **WASM builds must be tested separately**: Add CI jobs for `cargo build --target wasm32-unknown-unknown -p sublinear-time-solver --features wasm --no-default-features`.

### 8.3 Version Pinning Strategy

| Layer | Strategy | Tool |
|-------|----------|------|
| Workspace deps | Semver ranges in `[workspace.dependencies]`, exact in `Cargo.lock` | Cargo |
| Solver-specific deps | Pin to minor version in `Cargo.toml` | Cargo |
| NPM deps | Caret ranges (`^`) in `package.json`, exact in `package-lock.json` | npm |
| WASM | Track wasm-bindgen version across all crates | Manual + CI check |

### 8.4 Migration Path

**Phase 1 -- Immediate Integration** (zero conflicts):
1. Add solver crate to workspace members
2. Use workspace dependencies for all shared deps
3. Pin `nalgebra = "0.32"` locally
4. Test with `cargo check -p sublinear-time-solver`

**Phase 2 -- Optimize** (1-2 weeks):
1. Promote shared deps to workspace level
2. Add WASM build target with appropriate feature flags
3. Create NPM package with server dependencies
4. Run size benchmarks

**Phase 3 -- Unify** (1-2 months):
1. Migrate solver to nalgebra 0.33 to eliminate dual compilation
2. Standardize on thiserror 2.0 across all crates
3. Evaluate consolidating `log` vs `tracing` (ruvector uses tracing, solver uses log -- consider `tracing` compatibility layer)
4. Add `@ruvnet/strange-loop` and `strange-loops` to npm workspace

### 8.5 CI/CD Integration

Add to existing CI pipeline:

```yaml
# Solver-specific checks
- name: Check solver (native)
  run: cargo check -p sublinear-time-solver

- name: Check solver (WASM)
  run: cargo check -p sublinear-time-solver --target wasm32-unknown-unknown --no-default-features --features wasm

- name: Check solver (CLI)
  run: cargo check -p sublinear-time-solver --features cli

- name: Test solver
  run: cargo test -p sublinear-time-solver

- name: Size check (WASM)
  run: |
    wasm-pack build crates/sublinear-time-solver --target web --features wasm
    ls -la crates/sublinear-time-solver/pkg/*.wasm
```

### 8.6 Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| nalgebra dual-version compile time | High | Low | LTO caching, sccache |
| getrandom WASM breakage | Low | High | Existing dual-version pattern proven |
| Bundle size regression | Medium | Medium | CI size checks, separate WASM module |
| Feature flag combinatorial explosion | Medium | Low | Document valid combinations, test matrix |
| npm dependency conflicts with express middleware | Low | Low | Isolated package, workspace hoisting |
| `log` vs `tracing` ecosystem split | Medium | Low | Use `tracing-log` bridge crate |

### 8.7 Dependency Audit Notes

- **Security**: `ed25519-dalek 2.1` (used by ruqu, fpga-transformer) had past advisory RUSTSEC-2022-0093, but version 2.x resolves it. The solver does not use this crate directly.
- **Licensing**: All shared dependencies are MIT or Apache-2.0, compatible with RuVector's MIT license. nalgebra is Apache-2.0, which is compatible.
- **Maintenance**: All 26 solver dependencies are actively maintained with recent releases within 2025-2026.

---

## Appendix A: Complete Lockfile Version Snapshot (Solver-Relevant)

| Crate | Lockfile Version | Solver Requires | Compatible |
|-------|-----------------|-----------------|------------|
| nalgebra | 0.32.6, 0.33.2 | 0.32 | Yes (0.32.6) |
| serde | 1.0.228 | 1.x | Yes |
| thiserror | 1.0.69, 2.0.17 | any | Yes |
| log | 0.4.29 | 0.4 | Yes |
| rand | 0.8.5 | 0.8 | Yes |
| fnv | 1.0.7 | 1.x | Yes |
| num-traits | 0.2.19 | 0.2 | Yes |
| num-complex | 0.2.4, 0.4.6 | any | Yes |
| bit-set | 0.5.3, 0.8.0 | any | Yes |
| lazy_static | 1.5.0 | 1.x | Yes |
| wasm-bindgen | 0.2.106 | 0.2 | Yes |
| web-sys | 0.3.x | 0.3 | Yes |
| js-sys | 0.3.x | 0.3 | Yes |
| serde-wasm-bindgen | 0.6.5 | 0.6 | Yes |
| console_error_panic_hook | 0.1.7 | 0.1 | Yes |
| getrandom | 0.2.16, 0.3.4 | WASM | Yes |
| clap | 4.5.53 | 4.x | Yes |
| tokio | 1.48.0 | 1.x | Yes |
| axum | 0.7.9 | 0.7 | Yes |
| serde_json | 1.0.145 | 1.x | Yes |
| uuid | 1.19.0 | 1.x | Yes |
| colored | 2.2.0 | 2.x | Yes |
| wide | 0.7.33 | 0.7 | Yes |
| rayon | 1.11.0 | 1.x | Yes |

## Appendix B: Dependency Graph Visualization (Text)

```
sublinear-time-solver
├── nalgebra 0.32 ─────────────────────┐
│   ├── num-traits 0.2 ◄──────────────┤ (shared with ndarray, ruvector-math)
│   ├── num-complex 0.4 ◄─────────────┤ (shared)
│   └── simba ◄────────────────────────┤
├── serde 1.0 ◄────────────────────────┤ (workspace, ubiquitous)
├── thiserror ◄────────────────────────┤ (workspace)
├── log 0.4 ◄──────────────────────────┤ (transitive, bridge to tracing)
├── rand 0.8 ◄─────────────────────────┤ (workspace)
├── fnv 1.0 ◄──────────────────────────┤ (transitive)
├── bit-set ◄──────────────────────────┤ (transitive)
├── lazy_static 1.5 ◄─────────────────┤ (transitive)
├── [wasm feature]
│   ├── wasm-bindgen 0.2 ◄─────────────┤ (workspace)
│   ├── web-sys 0.3 ◄─────────────────┤ (workspace)
│   ├── js-sys 0.3 ◄──────────────────┤ (workspace)
│   ├── serde-wasm-bindgen 0.6 ◄───────┤ (ruvector-wasm, rvlite)
│   ├── console_error_panic_hook 0.1 ◄─┤ (ruvector-wasm, rvlite)
│   └── getrandom (wasm) ◄────────────┤ (managed dual-version)
├── [cli feature]
│   ├── clap 4.5 ◄─────────────────────┤ (workspace)
│   ├── tokio 1.x ◄───────────────────┤ (workspace)
│   ├── axum 0.7 ◄────────────────────┤ (ruvector-cli, ruvector-server)
│   ├── serde_json 1.0 ◄──────────────┤ (workspace)
│   ├── uuid 1.x ◄────────────────────┤ (workspace)
│   └── colored 2.x ◄─────────────────┤ (ruvector-cli)
├── [server feature - NPM]
│   ├── express 4.x ◄─────────────────┤ (rvf-mcp-server, agentic-integration)
│   ├── cors ◄─────────────────────────┤ (NEW)
│   ├── helmet ◄───────────────────────┤ (NEW)
│   └── compression ◄─────────────────┤ (NEW)
├── [performance feature]
│   ├── wide 0.7 ◄─────────────────────┤ (prime-radiant)
│   └── rayon 1.x ◄───────────────────┤ (workspace)
└── [ruvector feature]
    ├── ruvector-core ◄────────────────┤ (workspace path dep)
    ├── ruvector-mincut ◄──────────────┤ (workspace path dep)
    └── ruvector-math ◄───────────────┤ (workspace path dep)

Legend: ◄──┤ = shared with existing RuVector dependency
```

## Appendix C: NPM Dependency Overlap Summary

```
@ruvector/sublinear-solver (proposed)
├── @modelcontextprotocol/sdk ^1.0.0 ──── SHARED (ruvector, rvf-mcp-server)
├── express ^4.18.0 ───────────────────── SHARED (rvf-mcp-server, agentic-integration)
├── cors ^2.8.5 ───────────────────────── NEW
├── helmet ^7.0.0 ─────────────────────── NEW
├── compression ^1.7.4 ────────────────── NEW
├── @ruvnet/strange-loop ──────────────── NEW
└── strange-loops ─────────────────────── NEW
```

---

**Conclusion**: The sublinear-time-solver has exceptional dependency compatibility with RuVector. Of 26 total dependencies, 22 are already present in the workspace with compatible versions. The only material conflict is `nalgebra` 0.32 vs 0.33, which Cargo resolves natively through dual-version compilation. Only 4 NPM packages (`cors`, `helmet`, `compression`, `@ruvnet/strange-loop`/`strange-loops`) are genuinely new. The integration can proceed with high confidence and minimal friction.
