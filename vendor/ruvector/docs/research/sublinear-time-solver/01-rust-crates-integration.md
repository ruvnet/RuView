# Rust Crates Integration Analysis: ruvector + sublinear-time-solver

**Agent**: 1 of 15 (Rust Crates Integration Analysis)
**Date**: 2026-02-20
**ruvector version**: 2.0.3
**sublinear-time-solver version**: 0.1.3
**Rust edition**: both use 2021

---

## 1. Complete Inventory of Rust Crates in ruvector

### 1.1 Workspace Configuration

The ruvector workspace (`/home/user/ruvector/Cargo.toml`) uses resolver v2, edition 2021, and rust-version 1.77. The workspace contains 100 member crates organized into the following functional groups.

**Excluded from workspace** (managed independently):
- `crates/micro-hnsw-wasm`
- `crates/ruvector-hyperbolic-hnsw` and its WASM variant
- `crates/rvf` (main RVF crate, though many sub-crates are workspace members)
- Various example crates

### 1.2 Core Database Crates

| Crate | Path | Description | Key Dependencies |
|-------|------|-------------|------------------|
| `ruvector-core` | `crates/ruvector-core` | HNSW indexing, vector storage, distance metrics | ndarray 0.16, redb, memmap2, hnsw_rs, simsimd, serde, rand 0.8 |
| `ruvector-collections` | `crates/ruvector-collections` | Collection management | ruvector-core, dashmap, serde |
| `ruvector-filter` | `crates/ruvector-filter` | Metadata filtering | ruvector-core, ordered-float |
| `ruvector-snapshot` | `crates/ruvector-snapshot` | Database snapshots | (workspace deps) |
| `ruvector-server` | `crates/ruvector-server` | REST API (axum) | ruvector-core, axum, tokio |
| `ruvector-postgres` | `crates/ruvector-postgres` | PostgreSQL extension (pgrx) | pgrx 0.12, simsimd, half, rayon, memmap2 |

### 1.3 Math and Numerics Crates

| Crate | Path | Description | Key Dependencies |
|-------|------|-------------|------------------|
| `ruvector-math` | `crates/ruvector-math` | Optimal transport, info geometry, spectral methods, tropical algebra, tensor networks, homology | **nalgebra 0.33**, rand 0.8, thiserror |
| `ruvector-math-wasm` | `crates/ruvector-math-wasm` | WASM bindings for ruvector-math | ruvector-math, wasm-bindgen |

### 1.4 Graph and Network Crates

| Crate | Path | Description | Key Dependencies |
|-------|------|-------------|------------------|
| `ruvector-graph` | `crates/ruvector-graph` | Neo4j-compatible hypergraph DB | ruvector-core, petgraph, ndarray, roaring |
| `ruvector-graph-node` | `crates/ruvector-graph-node` | Node.js bindings | napi, ruvector-graph |
| `ruvector-graph-wasm` | `crates/ruvector-graph-wasm` | WASM bindings | wasm-bindgen, ruvector-graph |
| `ruvector-mincut` | `crates/ruvector-mincut` | Subpolynomial dynamic min-cut | ruvector-core, petgraph, rayon, roaring |
| `ruvector-mincut-wasm` | `crates/ruvector-mincut-wasm` | WASM bindings for mincut | wasm-bindgen |
| `ruvector-mincut-node` | `crates/ruvector-mincut-node` | Node.js bindings | napi |
| `ruvector-dag` | `crates/ruvector-dag` | DAG for query plan optimization | ruvector-core, ndarray 0.15, rand 0.8, sha2 |

### 1.5 Neural and AI Crates

| Crate | Path | Description | Key Dependencies |
|-------|------|-------------|------------------|
| `ruvector-gnn` | `crates/ruvector-gnn` | GNN layers (GCN, GraphSAGE, GAT, GIN) with EWC | ruvector-core, ndarray, rayon |
| `ruvector-gnn-wasm` | `crates/ruvector-gnn-wasm` | WASM GNN bindings | wasm-bindgen |
| `ruvector-gnn-node` | `crates/ruvector-gnn-node` | Node.js GNN bindings | napi |
| `ruvector-attention` | `crates/ruvector-attention` | 39+ attention mechanisms (geometric, graph, sparse, MoE) | rayon, serde, rand 0.8, optional: ruvector-math |
| `ruvector-attention-wasm` | `crates/ruvector-attention-wasm` | WASM attention bindings | wasm-bindgen |
| `ruvector-attention-node` | `crates/ruvector-attention-node` | Node.js attention bindings | napi |
| `ruvector-attention-unified-wasm` | `crates/ruvector-attention-unified-wasm` | Unified WASM attention | wasm-bindgen |
| `ruvector-sparse-inference` | `crates/ruvector-sparse-inference` | PowerInfer-style sparse inference | ndarray, rayon, memmap2, half, byteorder |
| `ruvector-sparse-inference-wasm` | `crates/ruvector-sparse-inference-wasm` | WASM sparse inference | wasm-bindgen |
| `ruvector-nervous-system` | `crates/ruvector-nervous-system` | Bio-inspired spiking networks, BTSP, EWC | ndarray, rand 0.8, rayon |
| `ruvector-nervous-system-wasm` | `crates/ruvector-nervous-system-wasm` | WASM nervous system | wasm-bindgen |

### 1.6 Transformer and Inference Crates

| Crate | Path | Description | Key Dependencies |
|-------|------|-------------|------------------|
| `ruvector-fpga-transformer` | `crates/ruvector-fpga-transformer` | FPGA transformer backend | thiserror, serde, sha2, ed25519-dalek, rand 0.8 |
| `ruvector-fpga-transformer-wasm` | `crates/ruvector-fpga-transformer-wasm` | WASM FPGA transformer | wasm-bindgen |
| `ruvector-mincut-gated-transformer` | `crates/ruvector-mincut-gated-transformer` | Mincut-gated coherence transformer | thiserror, serde |
| `ruvector-mincut-gated-transformer-wasm` | `crates/ruvector-mincut-gated-transformer-wasm` | WASM variant | wasm-bindgen |
| `ruvllm` | `crates/ruvllm` | LLM serving runtime, paged attention, KV cache | ruvector-core, ndarray, candle-core/nn/transformers, half |
| `ruvllm-cli` | `crates/ruvllm-cli` | CLI for ruvLLM | clap |
| `ruvllm-wasm` | `crates/ruvllm-wasm` | WASM ruvLLM | wasm-bindgen |

### 1.7 Learning and Adaptation Crates

| Crate | Path | Description | Key Dependencies |
|-------|------|-------------|------------------|
| `ruvector-sona` (sona) | `crates/sona` | SONA - self-optimizing neural architecture, EWC++, ReasoningBank | parking_lot, crossbeam, rand 0.8, serde |
| `ruvector-learning-wasm` | `crates/ruvector-learning-wasm` | WASM learning | wasm-bindgen |
| `ruvector-domain-expansion` | `crates/ruvector-domain-expansion` | Cross-domain transfer learning | serde, rand 0.8 |
| `ruvector-domain-expansion-wasm` | `crates/ruvector-domain-expansion-wasm` | WASM domain expansion | wasm-bindgen |

### 1.8 Delta (Incremental) System Crates

| Crate | Path | Description | Key Dependencies |
|-------|------|-------------|------------------|
| `ruvector-delta-core` | `crates/ruvector-delta-core` | Delta types and traits | thiserror, bincode, simsimd, smallvec |
| `ruvector-delta-index` | `crates/ruvector-delta-index` | Delta-aware HNSW | ruvector-delta-core, priority-queue, rand 0.8 |
| `ruvector-delta-graph` | `crates/ruvector-delta-graph` | Delta graph operations | ruvector-delta-core, dashmap |
| `ruvector-delta-consensus` | `crates/ruvector-delta-consensus` | CRDT-based delta consensus | ruvector-delta-core, serde, uuid, chrono |
| `ruvector-delta-wasm` | `crates/ruvector-delta-wasm` | WASM delta system | wasm-bindgen |

### 1.9 Distributed System Crates

| Crate | Path | Description | Key Dependencies |
|-------|------|-------------|------------------|
| `ruvector-cluster` | `crates/ruvector-cluster` | Distributed clustering/sharding | ruvector-core, tokio, dashmap |
| `ruvector-raft` | `crates/ruvector-raft` | Raft consensus | ruvector-core, tokio, dashmap |
| `ruvector-replication` | `crates/ruvector-replication` | Data replication/sync | ruvector-core, tokio, futures |

### 1.10 Coherence Engine Crates

| Crate | Path | Description | Key Dependencies |
|-------|------|-------------|------------------|
| `prime-radiant` | `crates/prime-radiant` | Universal coherence engine (sheaf Laplacian) | ruvector-core, **nalgebra 0.33**, ndarray, blake3, optional: many ruvector crates |
| `cognitum-gate-kernel` | `crates/cognitum-gate-kernel` | 256-tile no_std WASM coherence fabric | libm, optional: ruvector-mincut |
| `cognitum-gate-tilezero` | `crates/cognitum-gate-tilezero` | Tile-zero gate controller | (workspace deps) |

### 1.11 Specialty Crates

| Crate | Path | Description | Key Dependencies |
|-------|------|-------------|------------------|
| `ruvector-temporal-tensor` | `crates/ruvector-temporal-tensor` | Temporal tensor compression, tiered quantization | **zero dependencies** |
| `ruvector-crv` | `crates/ruvector-crv` | CRV protocol integration | ruvector-attention, ruvector-gnn, ruvector-mincut |
| `ruvector-hyperbolic-hnsw` | `crates/ruvector-hyperbolic-hnsw` | Hyperbolic Poincare HNSW | **nalgebra 0.34.1**, ndarray 0.17.1 |
| `ruvector-economy-wasm` | `crates/ruvector-economy-wasm` | WASM economy system | wasm-bindgen |
| `ruvector-exotic-wasm` | `crates/ruvector-exotic-wasm` | WASM exotic features | wasm-bindgen |
| `micro-hnsw-wasm` | `crates/micro-hnsw-wasm` | Tiny WASM HNSW | wasm-bindgen |
| `mcp-gate` | `crates/mcp-gate` | MCP gateway | (workspace deps) |

### 1.12 Quantum Simulation Crates

| Crate | Path | Description | Key Dependencies |
|-------|------|-------------|------------------|
| `ruqu-core` | `crates/ruqu-core` | Quantum circuit simulator | rand 0.8, thiserror |
| `ruqu-algorithms` | `crates/ruqu-algorithms` | VQE, Grover, QAOA, Surface Code | ruqu-core, rand 0.8 |
| `ruqu-wasm` | `crates/ruqu-wasm` | WASM quantum | wasm-bindgen |
| `ruqu-exotic` | `crates/ruqu-exotic` | Exotic quantum features | (workspace deps) |
| `ruQu` | `crates/ruQu` | Quantum umbrella crate | ruqu-core, ruqu-algorithms |

### 1.13 Routing and CLI Crates

| Crate | Path | Description | Key Dependencies |
|-------|------|-------------|------------------|
| `ruvector-router-core` | `crates/ruvector-router-core` | Neural routing engine | redb, simsimd, ndarray 0.15, rayon |
| `ruvector-router-cli` | `crates/ruvector-router-cli` | Router CLI | clap |
| `ruvector-router-ffi` | `crates/ruvector-router-ffi` | Router FFI | (workspace deps) |
| `ruvector-router-wasm` | `crates/ruvector-router-wasm` | WASM router | wasm-bindgen |
| `ruvector-cli` | `crates/ruvector-cli` | Main CLI | clap |
| `ruvector-attention-cli` | `crates/ruvector-attention-cli` | Attention CLI | clap |

### 1.14 Platform Binding Crates

| Crate | Path | Description | Key Dependencies |
|-------|------|-------------|------------------|
| `ruvector-wasm` | `crates/ruvector-wasm` | Core WASM bindings (kernel pack system) | ruvector-core, wasm-bindgen, sha2, ed25519-dalek |
| `ruvector-node` | `crates/ruvector-node` | Node.js bindings | napi |
| `ruvector-bench` | `crates/ruvector-bench` | Benchmarking harness | criterion |
| `ruvector-metrics` | `crates/ruvector-metrics` | Prometheus metrics | prometheus |
| `rvlite` | `crates/rvlite` | Standalone WASM vector DB (SQL/SPARQL/Cypher) | ruvector-core, wasm-bindgen |
| `ruvector-tiny-dancer-core` | `crates/ruvector-tiny-dancer-core` | Tiny Dancer routing core | (workspace deps) |
| `ruvector-tiny-dancer-wasm` | `crates/ruvector-tiny-dancer-wasm` | WASM Tiny Dancer | wasm-bindgen |
| `ruvector-tiny-dancer-node` | `crates/ruvector-tiny-dancer-node` | Node.js Tiny Dancer | napi |

### 1.15 RVF (RuVector Format) Sub-Crates

| Crate | Path | Description | Key Dependencies |
|-------|------|-------------|------------------|
| `rvf-types` | `crates/rvf/rvf-types` | Core binary format types (no_std) | serde, ed25519-dalek |
| `rvf-wire` | `crates/rvf/rvf-wire` | Wire protocol | rvf-types |
| `rvf-crypto` | `crates/rvf/rvf-crypto` | Cryptographic signing | rvf-types |
| `rvf-quant` | `crates/rvf/rvf-quant` | Temperature-tiered quantization | rvf-types |
| `rvf-manifest` | `crates/rvf/rvf-manifest` | Package manifests | rvf-types |
| `rvf-index` | `crates/rvf/rvf-index` | Index structures | rvf-types |
| `rvf-runtime` | `crates/rvf/rvf-runtime` | Container runtime | rvf-types |
| `rvf-kernel` | `crates/rvf/rvf-kernel` | Microkernel builder | rvf-types |
| `rvf-ebpf` | `crates/rvf/rvf-ebpf` | eBPF integration | rvf-types |
| `rvf-import` | `crates/rvf/rvf-import` | Model import | rvf-types |
| `rvf-launch` | `crates/rvf/rvf-launch` | Launch orchestration | rvf-types |
| `rvf-server` | `crates/rvf/rvf-server` | Server runtime | rvf-types |
| `rvf-cli` | `crates/rvf/rvf-cli` | CLI tool | rvf-types, clap |
| `rvf-solver-wasm` | `crates/rvf/rvf-solver-wasm` | Thompson Sampling solver | rvf-types, rvf-crypto, libm |
| `rvf-wasm` | `crates/rvf/rvf-wasm` | WASM bindings | rvf-types |
| `rvf-node` | `crates/rvf/rvf-node` | Node.js bindings | rvf-types, napi |
| RVF adapters | `crates/rvf/rvf-adapters/*` | agentdb, agentic-flow, claude-flow, ospipe, rvlite, sona | various |

---

## 2. Dependency Overlap with sublinear-time-solver

### 2.1 Direct Dependency Overlap Matrix

The sublinear-time-solver core crate (`sublinear` v0.1.3) uses: **nalgebra 0.32**, serde, rand, fnv, num-traits, num-complex, bit-set.

| Dependency | sublinear-time-solver | ruvector | Version Gap | Compatibility |
|------------|----------------------|----------|-------------|---------------|
| **nalgebra** | 0.32 | 0.33 (ruvector-math, prime-radiant), 0.34.1 (hyperbolic-hnsw) | 0.32 vs 0.33/0.34 | BREAKING - see 2.2 |
| **serde** | 1.x | 1.0 (workspace) | Compatible | Full overlap |
| **rand** | 0.8.x | 0.8.x (workspace) | Compatible | Full overlap |
| **thiserror** | (not listed) | 2.0 (workspace) | N/A | ruvector-only |
| **ndarray** | (not used) | 0.16 (workspace), 0.15 (dag, router-core), 0.17.1 (hyperbolic-hnsw) | N/A | ruvector-only |
| **fnv** | used | not used | N/A | No overlap |
| **num-traits** | 0.2 | 0.2 (via hnsw_rs patch) | Compatible | Indirect overlap |
| **num-complex** | used | not used (only in examples) | N/A | No overlap |
| **bit-set** | used | not used | N/A | No overlap |
| **rayon** | (in sub-crates) | 1.10 (workspace) | Likely compatible | Overlap in parallel crates |

### 2.2 nalgebra Version Analysis

This is the most critical dependency gap.

**sublinear-time-solver** uses `nalgebra 0.32`:
- Matrix/DMatrix/DVector types
- Sparse matrix representations
- Linear algebra operations (eigendecomposition, LU, etc.)

**ruvector** uses:
- `nalgebra 0.33` in `ruvector-math` and `prime-radiant` (optional)
- `nalgebra 0.34.1` in `ruvector-hyperbolic-hnsw` (excluded from workspace)

**Breaking changes from 0.32 to 0.33**: The nalgebra 0.33 release included changes to matrix storage traits and some generic bounds. Direct type reuse between 0.32 and 0.33 is NOT possible without one side updating. However, the numerical data within matrices (f32/f64 slices) is fully interchangeable via raw pointer/slice conversion.

**Recommended resolution**: The sublinear-time-solver should update to nalgebra 0.33 to align with the majority of ruvector crates, or ruvector should provide a conversion layer. Both projects use nalgebra's `DMatrix<f64>` and `DVector<f64>` as primary types.

### 2.3 Shared Workspace Dependencies

These workspace-level dependencies in ruvector are also used across sublinear-time-solver sub-crates:

| Dependency | ruvector Workspace Version | Usage Pattern |
|------------|---------------------------|---------------|
| `serde` | 1.0 with `derive` | Serialization throughout |
| `rand` | 0.8 | Random number generation |
| `rayon` | 1.10 | Parallel computation |
| `wasm-bindgen` | 0.2 | WASM bindings |
| `js-sys` | 0.3 | JavaScript interop |
| `thiserror` | 2.0 | Error types |

### 2.4 Notable Non-Overlapping Dependencies

**ruvector uses but sublinear does not**: ndarray, redb, memmap2, hnsw_rs, simsimd, rkyv, bincode, tokio, petgraph, roaring, pgrx, candle, half.

**sublinear uses but ruvector does not**: fnv, num-complex, bit-set (as direct workspace deps).

---

## 3. Type Compatibility Analysis

### 3.1 Matrix Types

#### sublinear-time-solver Matrix Types

```
// Core types (nalgebra-based)
Matrix         - Dense matrix wrapper around nalgebra::DMatrix<f64>
SparseMatrix   - CSR/CSC sparse matrix with nalgebra-compatible indexing
SparseFormat   - Enum: CSR | CSC | COO
OptimizedSparseMatrix - Cache-optimized sparse matrix with block structure
```

#### ruvector Matrix Types

```
// ruvector-math (nalgebra 0.33)
TropicalMatrix       - Max-plus semiring matrix (Vec<f64> storage)
MinPlusMatrix        - Min-plus semiring matrix

// prime-radiant (nalgebra 0.33 optional)
CsrMatrix            - CSR sparse matrix (f32, COO construction)
MatrixStorage enum   - Dense | Sparse(CsrMatrix) | Identity

// ruvector-core
ndarray::Array1/Array2 - Used for neural hash, TDA

// ruvector-gnn, ruvector-sparse-inference, ruvector-nervous-system
ndarray::ArrayN       - Primary tensor representation

// ruvector-fpga-transformer
QuantizedMatrix       - Quantized matrix for hardware inference

// ruvector-mincut
SynapseMatrix         - Specialized neural adjacency matrix
```

#### Compatibility Assessment

| sublinear Type | ruvector Equivalent | Compatibility | Notes |
|---------------|---------------------|---------------|-------|
| `Matrix` (nalgebra DMatrix) | `nalgebra::DMatrix` in ruvector-math, prime-radiant | **High** - same base type, version gap only | Align to nalgebra 0.33 |
| `SparseMatrix` (CSR) | `CsrMatrix` in prime-radiant | **Medium** - same CSR concept, different field types (f64 vs f32) | Need f32/f64 adapter |
| `SparseFormat::CSR` | `MatrixStorage::Sparse` in prime-radiant | **Medium** - conceptually equivalent | Wrap with From impl |
| `OptimizedSparseMatrix` | No direct equivalent | **Low** - must build adapter | Could wrap CsrMatrix |
| Dense matrix data | `ndarray::Array2` in core/gnn/sparse-inference | **Medium** - different abstraction | nalgebra-to-ndarray conversion exists |

### 3.2 Numeric Types

| sublinear Type | ruvector Type | Compatibility |
|---------------|---------------|---------------|
| `f64` (primary) | `f32` (primary in core, CsrMatrix) / `f64` (math, mincut) | **Mixed** - need precision adapters |
| `Complex<f64>` (num-complex) | Not used directly | **Low** - add num-complex dep or convert |
| nalgebra `DVector<f64>` | `ndarray::Array1<f32/f64>` | **Medium** - different types, same semantics |

### 3.3 Error Types

#### sublinear-time-solver Errors
Uses custom error types per solver module.

#### ruvector Error Types

```rust
// ruvector-core: RuvectorError (thiserror)
//   DimensionMismatch { expected, actual }
//   VectorNotFound, InvalidParameter, InvalidInput
//   StorageError, ModelLoadError, IndexError
//   SerializationError, IoError, DatabaseError, Internal

// ruvector-math: MathError (thiserror, Clone + PartialEq)
//   DimensionMismatch { expected, got }
//   EmptyInput, NumericalInstability
//   ConvergenceFailure { iterations, residual }
//   InvalidParameter, NotOnManifold, SingularMatrix
//   CurvatureViolation
```

**Compatibility**: The `MathError::ConvergenceFailure` and `MathError::SingularMatrix` variants map directly to errors that sublinear solvers produce. A `From<SublinearError>` implementation into `MathError` is straightforward since both use thiserror and share the same semantic error categories.

---

## 4. Specific Integration Opportunities for Each ruvector Crate

### 4.1 HIGH-PRIORITY Integrations

#### ruvector-math + sublinear core

**Opportunity**: The richest integration point. ruvector-math provides optimal transport, spectral methods, Chebyshev polynomials, and tensor networks. The sublinear solver provides Neumann series, conjugate gradient, and forward/backward push solvers on sparse matrices.

**Concrete integrations**:
1. **Spectral methods**: ruvector-math's `ChebyshevExpansion` and `SpectralFilter` require matrix-vector products on graph Laplacians. The sublinear `NeumannSolver` and `OptimizedConjugateGradientSolver` can solve `Lx = b` systems arising from spectral graph filtering in sublinear time.
2. **Optimal transport**: `SinkhornSolver` in ruvector-math iterates matrix-vector products. The sublinear `ForwardPushSolver` can accelerate the entropic regularized OT by providing approximate solutions as warm starts.
3. **Tensor network contraction**: `TensorTrain` decomposition in ruvector-math requires solving least-squares problems. `OptimizedConjugateGradientSolver` can solve these.

#### ruvector-sparse-inference + sublinear core

**Opportunity**: Direct architectural alignment. The sparse inference engine uses P*Q matrix factorization for neuron prediction. The sublinear solver's sparse matrix types and solvers map directly to the hot/cold neuron selection problem.

**Concrete integrations**:
1. **Sparse FFN acceleration**: Use `SublinearNeumannSolver` for approximate activation prediction instead of dense matmul.
2. **Low-rank prediction**: Use `HybridSolver` which combines forward push (local exploration) with backward push (global approximation) for the prediction matrix factorization.
3. **SIMD-compatible sparse ops**: The sublinear `OptimizedSparseMatrix` with block structure aligns with the SIMD-accelerated paths in ruvector-sparse-inference.

#### prime-radiant + sublinear core

**Opportunity**: The coherence engine's sheaf Laplacian computations are fundamentally linear algebra on sparse matrices. The sublinear solver was designed for exactly these kinds of operations.

**Concrete integrations**:
1. **Sheaf Laplacian solve**: The `CsrMatrix` in prime-radiant's restriction module stores the sheaf structure. The sublinear `NeumannSolver` can solve `(I - P)x = b` where P is the random walk matrix of the sheaf Laplacian, achieving sublinear-time coherence checks.
2. **Spectral analysis**: prime-radiant's spectral coherence module (currently uses nalgebra `SymmetricEigen`) can use sublinear's solvers for approximate eigenvalue computation on large sheafs.
3. **Incremental updates**: The `ForwardPushSolver` supports local updates, enabling incremental coherence recomputation when a single tile changes.

#### ruvector-mincut + sublinear core

**Opportunity**: Min-cut algorithms require solving max-flow problems, which can be formulated as linear systems. The sublinear solver's push-based algorithms have natural connections to push-relabel max-flow.

**Concrete integrations**:
1. **Spectral min-cut**: Use `SublinearNeumannSolver` to approximate the Fiedler vector (second eigenvector of Laplacian) for spectral graph partitioning in sublinear time.
2. **Expander decomposition**: The expander decomposition subroutine in ruvector-mincut can use sublinear random-walk solvers to test expansion properties.

### 4.2 MEDIUM-PRIORITY Integrations

#### ruvector-gnn + neural-network-implementation

**Opportunity**: The GNN crate and the sublinear neural-network-implementation share the goal of neural network computation on graph-structured data.

**Concrete integrations**:
1. **Message passing acceleration**: GNN message passing is sparse matrix-vector multiplication. Use sublinear's `SparseMatrix` operations for the aggregation step.
2. **EWC Fisher Information**: The EWC module in ruvector-gnn computes Fisher information matrices. The sublinear conjugate gradient solver can compute the diagonal Fisher approximation more efficiently.
3. **Training loop**: Integrate sublinear's backpropagation with ruvector-gnn's `Optimizer` (Adam) and `LearningRateScheduler`.

#### ruvector-attention + sublinear core

**Opportunity**: Attention mechanisms involve matrix-vector products (Q*K^T*V) that can be approximated with sublinear methods for large sequence lengths.

**Concrete integrations**:
1. **Sparse attention**: The `sparse` module in ruvector-attention can use sublinear's sparse matrix types for the attention pattern.
2. **Fisher information attention**: The `info_geometry/fisher.rs` module already has a `solve_cg` (conjugate gradient) method. Replace with sublinear's `OptimizedConjugateGradientSolver` for better convergence.
3. **Graph attention**: The `graph` attention module computes attention on graph structures; the sublinear push-based solvers can compute personalized PageRank attention weights.

#### ruvector-nervous-system + psycho-symbolic-reasoner

**Opportunity**: The bio-inspired spiking network in ruvector-nervous-system shares conceptual overlap with the symbolic reasoning in psycho-symbolic-reasoner.

**Concrete integrations**:
1. **HDC binding operations**: Hyperdimensional computing in the nervous system uses vector binding/bundling that can be expressed as sparse matrix ops.
2. **Synaptic weight matrices**: The `SynapseMatrix` in ruvector-mincut's SNN module can be backed by sublinear's `OptimizedSparseMatrix`.

#### ruvector-graph + sublinear core

**Opportunity**: The graph database needs efficient linear algebra for graph algorithms (PageRank, centrality, community detection).

**Concrete integrations**:
1. **PageRank**: `ForwardPushSolver` is essentially the ForwardPush algorithm for approximate personalized PageRank, directly applicable.
2. **Community detection**: Use `NeumannSolver` for spectral community detection on the stored graph.
3. **Path queries**: Tropical matrix multiplication in ruvector-math + sublinear matrix solvers for all-pairs shortest paths.

### 4.3 LOWER-PRIORITY Integrations

#### ruvector-temporal-tensor + temporal-compare / temporal-lead-solver

**Opportunity**: Both deal with temporal data. The ruvector temporal tensor crate handles compression/quantization of time-series tensor data; the sublinear temporal crates solve time-dependent problems.

**Concrete integrations**:
1. Use temporal-compare for comparing compressed temporal tensor segments.
2. Use temporal-lead-solver for time-dependent linear system solving on decompressed tensor data.

#### ruvector-hyperbolic-hnsw + sublinear core

**Opportunity**: Hyperbolic embeddings use Poincare distance computations that involve matrix exponentials and logarithms solvable via Neumann series.

#### ruqu-core + sublinear core

**Opportunity**: Quantum circuit simulation involves unitary matrix operations. The sublinear solver's matrix types could represent quantum gates, though the primary value would be in hybrid quantum-classical optimization (VQE uses classical linear algebra).

#### ruvector-domain-expansion + strange-loop

**Opportunity**: The strange-loop crate handles recursive computation patterns; domain expansion handles transfer learning. Recursive self-improvement loops could use strange-loop's computation model.

#### WASM Integration: ruvector WASM crates + temporal-neural-solver-wasm / wasm-solver

**Opportunity**: Both ruvector and sublinear have WASM compilation targets. WASM crates from both projects could be composed in a browser environment.

**Concrete integrations**:
1. Use `wasm-solver` as a backend for `ruvector-math-wasm`'s linear algebra operations.
2. Compose `temporal-neural-solver-wasm` with `ruvector-attention-unified-wasm` for temporal attention in WASM.

#### RVF sub-crates + sublinear

**Opportunity**: The `rvf-solver-wasm` already implements a Thompson Sampling solver. The sublinear solver could provide an alternative solver backend for the RVF runtime.

#### rustc-hyperopt + ruvector-sona

**Opportunity**: Both handle hyperparameter optimization. SONA's two-tier LoRA and adaptive thresholds could use rustc-hyperopt's optimization algorithms for tuning.

---

## 5. Code-Level Integration Patterns

### 5.1 Trait Implementations for Matrix Interoperability

```rust
// Bridge trait: Convert between nalgebra (sublinear) and ndarray (ruvector-core)
pub trait MatrixBridge {
    fn to_ndarray(&self) -> ndarray::Array2<f64>;
    fn from_ndarray(arr: &ndarray::Array2<f64>) -> Self;
    fn to_nalgebra(&self) -> nalgebra::DMatrix<f64>;
    fn from_nalgebra(mat: &nalgebra::DMatrix<f64>) -> Self;
}

// Example: nalgebra DMatrix <-> ndarray Array2
impl MatrixBridge for nalgebra::DMatrix<f64> {
    fn to_ndarray(&self) -> ndarray::Array2<f64> {
        let (rows, cols) = self.shape();
        ndarray::Array2::from_shape_fn((rows, cols), |(i, j)| self[(i, j)])
    }

    fn from_ndarray(arr: &ndarray::Array2<f64>) -> Self {
        let (rows, cols) = arr.dim();
        nalgebra::DMatrix::from_fn(rows, cols, |i, j| arr[[i, j]])
    }

    fn to_nalgebra(&self) -> nalgebra::DMatrix<f64> {
        self.clone()
    }

    fn from_nalgebra(mat: &nalgebra::DMatrix<f64>) -> Self {
        mat.clone()
    }
}
```

### 5.2 Sparse Matrix Conversion (prime-radiant CsrMatrix <-> sublinear SparseMatrix)

```rust
// Conversion between prime-radiant CsrMatrix (f32) and sublinear SparseMatrix (f64)
pub trait SparseConvert {
    fn to_sublinear_csr(&self) -> sublinear::SparseMatrix;
    fn from_sublinear_csr(sparse: &sublinear::SparseMatrix) -> Self;
}

impl SparseConvert for prime_radiant::substrate::CsrMatrix {
    fn to_sublinear_csr(&self) -> sublinear::SparseMatrix {
        // Convert COO triplets with f32->f64 promotion
        let entries: Vec<(usize, usize, f64)> = (0..self.rows)
            .flat_map(|i| {
                let start = self.row_ptr[i];
                let end = self.row_ptr[i + 1];
                (start..end).map(move |idx| {
                    (i, self.col_indices[idx], self.values[idx] as f64)
                })
            })
            .collect();
        sublinear::SparseMatrix::from_coo(
            self.rows, self.cols, entries,
            sublinear::SparseFormat::CSR,
        )
    }

    fn from_sublinear_csr(sparse: &sublinear::SparseMatrix) -> Self {
        // Extract CSR components, demote f64->f32
        let (row_ptr, col_indices, values) = sparse.to_csr_components();
        Self {
            row_ptr,
            col_indices,
            values: values.iter().map(|&v| v as f32).collect(),
            rows: sparse.rows(),
            cols: sparse.cols(),
        }
    }
}
```

### 5.3 Error Type Bridging

```rust
use ruvector_math::MathError;

impl From<sublinear::SolverError> for MathError {
    fn from(err: sublinear::SolverError) -> Self {
        match err {
            sublinear::SolverError::ConvergenceFailure { iterations, residual } => {
                MathError::ConvergenceFailure { iterations, residual }
            }
            sublinear::SolverError::SingularMatrix(msg) => {
                MathError::SingularMatrix { context: msg }
            }
            sublinear::SolverError::DimensionMismatch { expected, got } => {
                MathError::DimensionMismatch { expected, got }
            }
            sublinear::SolverError::InvalidParameter(msg) => {
                MathError::InvalidParameter {
                    name: "solver".into(),
                    reason: msg,
                }
            }
            _ => MathError::NumericalInstability {
                message: err.to_string(),
            },
        }
    }
}

// Also bridge to ruvector-core errors
impl From<sublinear::SolverError> for ruvector_core::RuvectorError {
    fn from(err: sublinear::SolverError) -> Self {
        RuvectorError::Internal(format!("Sublinear solver error: {}", err))
    }
}
```

### 5.4 Generic Bounds Pattern for Solver Integration

```rust
/// Trait for any linear system solver, unifying ruvector and sublinear approaches
pub trait LinearSolver: Send + Sync {
    type Matrix;
    type Vector;
    type Error: std::error::Error;

    /// Solve Ax = b
    fn solve(&self, a: &Self::Matrix, b: &Self::Vector) -> Result<Self::Vector, Self::Error>;

    /// Solve (I - alpha*A)x = b (Neumann-style)
    fn solve_neumann(
        &self,
        a: &Self::Matrix,
        b: &Self::Vector,
        alpha: f64,
        tol: f64,
    ) -> Result<Self::Vector, Self::Error>;
}

// Implement for sublinear solvers
impl LinearSolver for sublinear::NeumannSolver {
    type Matrix = sublinear::SparseMatrix;
    type Vector = Vec<f64>;
    type Error = sublinear::SolverError;

    fn solve(&self, a: &Self::Matrix, b: &Self::Vector) -> Result<Self::Vector, Self::Error> {
        self.solve(a, b)
    }

    fn solve_neumann(
        &self, a: &Self::Matrix, b: &Self::Vector, alpha: f64, tol: f64,
    ) -> Result<Self::Vector, Self::Error> {
        self.solve_with_params(a, b, alpha, tol)
    }
}

// Implement for ruvector-math's existing solvers
impl LinearSolver for ruvector_math::spectral::ChebyshevSolver {
    type Matrix = nalgebra::DMatrix<f64>;
    type Vector = nalgebra::DVector<f64>;
    type Error = MathError;
    // ... implementations
}
```

### 5.5 Feature Flag Integration Pattern

```rust
// In ruvector-math/src/spectral/chebyshev.rs
pub struct ChebyshevExpansion {
    // ...
}

impl ChebyshevExpansion {
    /// Apply Chebyshev filter using sparse matrix-vector products
    ///
    /// When the `sublinear-solver` feature is enabled, uses optimized
    /// sublinear-time sparse matrix operations for O(k * nnz^{1-epsilon})
    /// complexity instead of O(k * nnz).
    pub fn filter(&self, laplacian: &ScaledLaplacian, signal: &[f64]) -> Vec<f64> {
        #[cfg(feature = "sublinear-solver")]
        {
            use sublinear::{SparseMatrix, ForwardPushSolver};
            let sparse = SparseMatrix::from_laplacian(laplacian);
            let solver = ForwardPushSolver::new(sparse);
            solver.apply_polynomial(&self.coefficients, signal)
        }

        #[cfg(not(feature = "sublinear-solver"))]
        {
            self.filter_dense(laplacian, signal)
        }
    }
}
```

### 5.6 WASM Composition Pattern

```rust
// In ruvector-math-wasm or a new bridge crate
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct SublinearBridge {
    solver: sublinear::HybridSolver,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl SublinearBridge {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            solver: sublinear::HybridSolver::default(),
        }
    }

    /// Solve sparse system from JavaScript, returning Float64Array
    pub fn solve_sparse(
        &self,
        row_ptr: &[u32],
        col_indices: &[u32],
        values: &[f64],
        rhs: &[f64],
        rows: usize,
        cols: usize,
    ) -> Vec<f64> {
        let matrix = sublinear::SparseMatrix::from_csr_raw(
            row_ptr, col_indices, values, rows, cols
        );
        self.solver.solve(&matrix, rhs).unwrap_or_default()
    }
}
```

---

## 6. Recommended Cargo.toml Changes for Integration

### 6.1 Root Workspace Cargo.toml Additions

```toml
# Add to /home/user/ruvector/Cargo.toml [workspace.dependencies]

# Sublinear-time solver integration
sublinear = { version = "0.1.3", optional = true, default-features = false }
bit-parallel-search = { version = "0.1", optional = true }
# Note: sublinear uses nalgebra 0.32; either:
#   (a) Pin sublinear to use nalgebra 0.33 via patch, or
#   (b) Wait for sublinear to update to 0.33
# Option (a):
# [patch.crates-io]
# sublinear = { git = "https://github.com/ruvnet/sublinear-time-solver", branch = "nalgebra-0.33" }
```

### 6.2 ruvector-math/Cargo.toml

```toml
# Add to [dependencies]
sublinear = { workspace = true, optional = true }

# Add to [features]
sublinear-solver = ["dep:sublinear"]
# Include in a full feature
full = ["std", "simd", "parallel", "serde", "sublinear-solver"]
```

### 6.3 ruvector-sparse-inference/Cargo.toml

```toml
# Add to [dependencies]
sublinear = { workspace = true, optional = true }

# Add to [features]
sublinear-backend = ["dep:sublinear"]
```

### 6.4 prime-radiant/Cargo.toml

```toml
# Add to [dependencies]
sublinear = { workspace = true, optional = true }

# Add to [features]
sublinear-solver = ["dep:sublinear"]
# Add to full feature list
full = [
    # ... existing features ...
    "sublinear-solver",
]
```

### 6.5 ruvector-mincut/Cargo.toml

```toml
# Add to [dependencies]
sublinear = { workspace = true, optional = true }

# Add to [features]
spectral-solver = ["dep:sublinear"]
full = ["exact", "approximate", "integration", "monitoring", "simd", "agentic", "jtree", "tiered", "spectral-solver"]
```

### 6.6 ruvector-attention/Cargo.toml

```toml
# Add to [dependencies]
sublinear = { version = "0.1.3", optional = true }

# Add to [features]
sublinear-attention = ["dep:sublinear"]
```

### 6.7 ruvector-gnn/Cargo.toml

```toml
# Add to [dependencies]
sublinear = { workspace = true, optional = true }

# Add to [features]
sublinear-message-passing = ["dep:sublinear"]
```

### 6.8 ruvector-graph/Cargo.toml

```toml
# Add to [dependencies]
sublinear = { workspace = true, optional = true }

# Add to [features]
sublinear-graph-algo = ["dep:sublinear"]
full = ["simd", "storage", "async-runtime", "compression", "hnsw_rs", "ruvector-core/hnsw", "sublinear-graph-algo"]
```

### 6.9 New Bridge Crate (recommended)

Create `/home/user/ruvector/crates/ruvector-sublinear-bridge/Cargo.toml`:

```toml
[package]
name = "ruvector-sublinear-bridge"
version.workspace = true
edition.workspace = true
description = "Bridge crate connecting ruvector ecosystem with sublinear-time-solver"

[dependencies]
# Sublinear solver
sublinear = "0.1.3"

# ruvector math types
ruvector-math = { path = "../ruvector-math", optional = true }
ruvector-core = { path = "../ruvector-core", default-features = false, optional = true }

# Shared deps
nalgebra = { version = "0.33", default-features = false, features = ["std"] }
ndarray = { workspace = true, optional = true }
serde = { workspace = true }
thiserror = { workspace = true }

[features]
default = ["math-bridge"]
math-bridge = ["dep:ruvector-math"]
core-bridge = ["dep:ruvector-core", "dep:ndarray"]
full = ["math-bridge", "core-bridge"]
```

This bridge crate would contain:
- `MatrixBridge` trait and implementations
- `SparseConvert` trait and implementations
- `From<SublinearError>` for ruvector error types
- `LinearSolver` trait unifying both ecosystems
- Conversion utilities for f32/f64 precision transitions

---

## 7. nalgebra Version Reconciliation Strategy

The nalgebra version spread across the ecosystem is:

| Version | Used By | Count |
|---------|---------|-------|
| 0.32 | sublinear-time-solver | 1 |
| 0.33 | ruvector-math, prime-radiant | 2 |
| 0.34.1 | ruvector-hyperbolic-hnsw | 1 |

**Recommended approach (phased)**:

1. **Phase 1** (immediate): Create the bridge crate with both nalgebra 0.32 (re-exported from sublinear) and 0.33 conversions via raw slice access. This allows coexistence.

2. **Phase 2** (short-term): Submit a PR to sublinear-time-solver updating nalgebra from 0.32 to 0.33. The API changes between these versions are manageable.

3. **Phase 3** (medium-term): Align ruvector-hyperbolic-hnsw from 0.34.1 down to 0.33, or align everything up to 0.34.

4. **Alternative**: Use a Cargo `[patch]` section in the ruvector workspace root to pin sublinear's nalgebra to 0.33:

```toml
[patch.crates-io]
# When using sublinear as git dep, patch its nalgebra version
# nalgebra = { version = "0.33", ... }
```

---

## 8. Summary of Integration Priority

| Priority | Integration | Effort | Value | Key Blocker |
|----------|-------------|--------|-------|-------------|
| P0 | ruvector-math + sublinear core | Medium | Very High | nalgebra 0.32 vs 0.33 |
| P0 | prime-radiant + sublinear core (sheaf Laplacian) | Medium | Very High | nalgebra version + f32/f64 |
| P1 | ruvector-sparse-inference + sublinear core | Low | High | None (ndarray-based, need bridge) |
| P1 | ruvector-mincut + sublinear core (spectral) | Medium | High | nalgebra version |
| P2 | ruvector-gnn + neural-network-implementation | Medium | Medium | API surface mapping |
| P2 | ruvector-attention + sublinear core | Low | Medium | None |
| P2 | ruvector-graph + sublinear core (PageRank) | Low | Medium | None |
| P3 | WASM crate composition | Low | Medium | None (shared wasm-bindgen) |
| P3 | ruvector-nervous-system + psycho-symbolic-reasoner | High | Low | Conceptual gap |
| P3 | ruvector-temporal-tensor + temporal crates | Low | Low | Thin overlap |
| P3 | RVF solver + sublinear solver | Low | Low | Different problem domains |
| P3 | ruqu + sublinear (quantum-classical hybrid) | High | Low | Very different domains |

**Estimated total integration effort**: 4-6 weeks for P0+P1 items, assuming nalgebra version alignment is resolved first.

---

## 9. Appendix: Complete Workspace Dependency Map

### Workspace-Level Dependencies (`[workspace.dependencies]`)

```
Core:        redb 2.1, memmap2 0.9, hnsw_rs 0.3, simsimd 5.9, rayon 1.10, crossbeam 0.8
Serialization: rkyv 0.8, bincode 2.0.0-rc.3, serde 1.0, serde_json 1.0
Node.js:     napi 2.16, napi-derive 2.16
WASM:        wasm-bindgen 0.2, wasm-bindgen-futures 0.4, js-sys 0.3, web-sys 0.3, getrandom 0.3
Async:       tokio 1.41, futures 0.3
Errors:      thiserror 2.0, anyhow 1.0, tracing 0.1
Math:        ndarray 0.16, rand 0.8, rand_distr 0.4
Time/UUID:   chrono 0.4, uuid 1.11
CLI:         clap 4.5, indicatif 0.17, console 0.15
Testing:     criterion 0.5, proptest 1.5, mockall 0.13
Performance: dashmap 6.1, parking_lot 0.12, once_cell 1.20
```

### Patched Dependencies

```toml
[patch.crates-io]
hnsw_rs = { path = "./patches/hnsw_rs" }  # Pins to rand 0.8 for WASM compat
```
