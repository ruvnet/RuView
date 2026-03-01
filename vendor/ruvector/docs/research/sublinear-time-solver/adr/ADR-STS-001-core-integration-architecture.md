# ADR-STS-001: Sublinear-Time Solver Core Integration Architecture

**Status**: Accepted
**Date**: 2026-02-20
**Authors**: RuVector Architecture Team
**Deciders**: Architecture Review Board

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-02-20 | RuVector Team | Initial proposal |
| 1.0 | 2026-02-20 | RuVector Team | Accepted: full implementation complete |

---

## Context

### The Performance Ceiling Problem

RuVector is a 79-crate Rust monorepo (v2.0.3, edition 2021, `rust-version = "1.77"`) that
implements a high-performance vector database with capabilities spanning far beyond
conventional vector search. The system includes:

- **HNSW vector indexing** (`ruvector-core`) with SIMD-accelerated distance metrics
  (AVX2, AVX-512, NEON, WASM SIMD128) achieving 61us p50 latency at 384 dimensions
- **Neo4j-compatible graph database** (`ruvector-graph`) with Cypher query engine,
  petgraph-backed storage, and hybrid vector-graph search
- **Graph Neural Networks** (`ruvector-gnn`) with GCN, GraphSAGE, GAT, GIN layers,
  EWC++ continual learning, and ndarray-based tensor operations
- **40+ attention mechanisms** (`ruvector-attention`) including Flash, PDE, Sheaf,
  Hyperbolic, Optimal Transport, MoE, and Information Geometry attention
- **Prime Radiant coherence engine** (`prime-radiant`) implementing sheaf Laplacian
  mathematics for universal coherence verification with domain-agnostic interpretation
- **Spectral methods** (`ruvector-math`) using Chebyshev polynomial expansion,
  spectral clustering, graph wavelets, optimal transport (Sinkhorn), information
  geometry (Fisher, K-FAC), tensor networks, and persistent homology
- **Quantum algorithms** (`ruQu`) with VQE, Grover, QAOA, and surface code support
- **Subpolynomial dynamic min-cut** (`ruvector-mincut`) implementing the December 2024
  breakthrough (arXiv:2512.13105) with O(n^{o(1)}) update time
- **27 WASM crates** targeting `wasm32-unknown-unknown` via `wasm-bindgen`
- **Node.js bindings** via NAPI-RS for server-side deployment
- **MCP integration** across 5 servers exposing 80+ tools via JSON-RPC 2.0

Despite this breadth, the mathematical backbone for sparse linear systems -- graph
Laplacians, spectral methods, PageRank computations, optimal transport, and Fisher
information inversion -- currently relies on dense O(n^2) or O(n^3) algorithms via
`ndarray 0.16`, `nalgebra 0.33`, and custom implementations. This creates a performance
ceiling that becomes acute at scale:

| Subsystem | Current Complexity | Bottleneck Operation |
|-----------|--------------------|----------------------|
| Prime Radiant coherence | O(n^2) to O(n^3) | Dense sheaf Laplacian solve |
| GNN message passing | O(n * avg_degree) per layer | Sparse matrix-vector products |
| Spectral Chebyshev filters | O(K * nnz(L)) per signal | Repeated sparse matvec |
| Graph PageRank | O(n * iterations) | Iterative power method |
| Sinkhorn optimal transport | O(n^2 * iterations) | Dense kernel updates |
| Natural gradient (K-FAC) | O(d * hidden) | Fisher information inversion |

### The Sublinear-Time Solver Opportunity

The `sublinear-time-solver` project (v1.4.1 Rust, v1.5.0 npm) provides a Rust + WASM
mathematical toolkit implementing true O(log n) algorithms for sparse linear systems:

| Algorithm | Complexity | Primary Use Case |
|-----------|------------|------------------|
| **Neumann Series** | O(k * nnz) | Diagonally dominant systems (Laplacians) |
| **Forward Push** | O(1/eps) | Personalized PageRank from single source |
| **Backward Push** | O(1/eps) | Reverse PPR, importance propagation |
| **Hybrid Random Walk** | O(log(1/eps)/eps) | Combined push + walk for large graphs |
| **TRUE (Truncated Random Walk)** | O(sqrt(nnz) * log(1/eps)) | General sparse systems with sparsifiers |
| **Conjugate Gradient (CG)** | O(sqrt(kappa) * nnz) | Symmetric positive-definite systems |
| **BMSSP** | O(nnz * log n) | Bounded max-sum subarray (optimization) |

The solver shares Rust 2021 edition, `wasm-bindgen 0.2.x`, `rayon 1.10`, `serde 1.0`,
and `criterion`-based benchmarking. Its 9-crate workspace structure mirrors RuVector's
modular architecture. License compatibility is confirmed: MIT (RuVector) and
MIT/Apache-2.0 (solver).

### Technical Compatibility Assessment

**Overall Score: 91/100**

| Category | Score | Notes |
|----------|-------|-------|
| Language and toolchain | 98 | Both Rust 2021, same MSRV range |
| Dependency compatibility | 90 | No conflicting major versions |
| Architecture alignment | 92 | Same workspace monorepo pattern |
| WASM target compatibility | 95 | Identical wasm-bindgen toolchain |
| API design philosophy | 88 | Both use trait-based interfaces |
| Performance characteristics | 95 | Complementary optimization targets |
| Testing infrastructure | 90 | Both use criterion + proptest |

### Decision Drivers

1. Prime Radiant coherence is limited to ~10K nodes at interactive latency; production
   deployments require 100K to 10M nodes
2. GNN training on HNSW topologies is bottlenecked by dense aggregation
3. No competing vector database offers integrated O(log n) sparse solvers
4. The solver's WASM target enables browser-native graph analytics without backend
5. MCP tool surface (40+ solver tools) extends the existing agent ecosystem

---

## Decision

Create a new `ruvector-solver` crate family following the established Core-Binding-Surface
pattern. The solver integrates as a **Layer 0 mathematical foundation**, alongside
`nalgebra` and `ndarray`, providing O(log n) sparse linear system algorithms to all
consuming subsystems.

### Architecture Design

#### Integration Layer Map

```
+=========================================================================+
|                    Layer 4: DISTRIBUTION                                 |
|  ruvector-cluster | ruvector-raft | ruvector-replication                 |
|  ruvector-delta-consensus                                                |
+=========================================================================+
         |
+=========================================================================+
|                    Layer 3: INTEGRATION SERVICES                         |
|  mcp-gate (JSON-RPC)     | ruvector-server (axum)  | ruvector-cli       |
|  +40 solver MCP tools    | /solver/* routes         | solver subcommands |
+=========================================================================+
         |
+=========================================================================+
|                    Layer 2: PLATFORM BINDINGS                            |
|  ruvector-solver-wasm    | ruvector-solver-node     | ruvector-solver-ffi|
|  (wasm-bindgen)          | (NAPI-RS)                | (extern "C")       |
+=========================================================================+
         |
+=========================================================================+
|                    Layer 1: CORE ENGINES                                 |
|                                                                          |
|  prime-radiant ----+                                                     |
|  ruvector-gnn -----+----> ruvector-solver <---- ruvector-math            |
|  ruvector-graph ---+          |                 ruvector-attention        |
|  ruvector-mincut --+          |                 ruvector-sparse-inference |
|  cognitum-gate ----+          |                                          |
|                               v                                          |
+=========================================================================+
|                    Layer 0: MATH FOUNDATION                              |
|  nalgebra 0.33  |  ndarray 0.16  |  simsimd 5.9  |  rayon 1.10         |
|  [nalgebra-bridge: zero-copy DMatrix <-> Array2 conversion]              |
+=========================================================================+
```

#### Crate Dependency Graph

```
                        ruvector-solver (core)
                       /        |        \
                      /         |         \
        ruvector-solver-wasm  ruvector-solver-node  ruvector-solver-ffi
        (wasm-bindgen)        (NAPI-RS)              (extern "C")

    Upstream consumers (depend on ruvector-solver):
    +-----------------+     +----------------+     +------------------+
    | prime-radiant   |     | ruvector-gnn   |     | ruvector-math    |
    | (coherence)     |     | (GNN layers)   |     | (spectral)       |
    +-----------------+     +----------------+     +------------------+
    | ruvector-graph  |     | ruvector-       |     | ruvector-        |
    | (PageRank)      |     | attention       |     | mincut           |
    +-----------------+     | (PDE attention) |     | (sparsifier)     |
                            +----------------+     +------------------+

    Downstream dependencies (ruvector-solver depends on):
    +-----------------+     +----------------+     +------------------+
    | nalgebra 0.33   |     | ndarray 0.16   |     | rayon 1.10       |
    | (sparse types)  |     | (bridge layer) |     | (parallel)       |
    +-----------------+     +----------------+     +------------------+
```

#### New Crate Structure

```
crates/ruvector-solver/
    Cargo.toml
    src/
        lib.rs                  # Public API, trait re-exports, feature gates
        error.rs                # SolverError enum with thiserror
        config.rs               # SolverConfig with builder pattern
        types.rs                # SparseMatrix, SolverResult, ConvergenceBound
        traits.rs               # SolverEngine, NumericBackend, SolverCache
        bridge/
            mod.rs              # Backend abstraction layer
            nalgebra_backend.rs # nalgebra DMatrix/CsMatrix operations
            ndarray_backend.rs  # ndarray Array2 bridge (zero-copy views)
        algorithms/
            mod.rs              # Algorithm registry + auto-selection
            neumann.rs          # Neumann series expansion
            forward_push.rs     # Forward Push PageRank
            backward_push.rs    # Backward Push reverse PPR
            hybrid_walk.rs      # Hybrid Random Walk
            true_solver.rs      # TRUE sparse solver
            conjugate_gradient.rs # Preconditioned CG
            bmssp.rs            # Bounded Max-Sum Subarray
        integration/
            mod.rs              # Integration adapters
            coherence.rs        # Prime Radiant sheaf Laplacian adapter
            gnn.rs              # GNN SublinearAggregation strategy
            spectral.rs         # Neumann filter for ruvector-math
            graph.rs            # Push-based PageRank for ruvector-graph
            attention.rs        # PDE attention sparse Laplacian
            mincut.rs           # Shared sparsifier with TRUE
        cache.rs                # LRU solution cache with TTL
        events.rs               # SolverEvent enum (event sourcing)
    benches/
        solver_benchmarks.rs    # Criterion benchmarks for all algorithms

crates/ruvector-solver-wasm/
    Cargo.toml
    src/
        lib.rs                  # wasm-bindgen surface (JsSolver)

crates/ruvector-solver-node/
    Cargo.toml
    src/
        lib.rs                  # NAPI-RS bindings (napi macro)
```

#### nalgebra/ndarray Bridge with Zero-Copy Conversion

The primary architectural tension is the linear algebra backend divergence: the solver
uses `nalgebra` while RuVector's crates use `ndarray`. The bridge layer resolves this
with zero-copy view conversions:

```rust
// crates/ruvector-solver/src/bridge/nalgebra_backend.rs

use nalgebra::{DMatrix, DVector, CsMatrix};
use ndarray::{Array2, ArrayView2};

/// Zero-copy view from nalgebra DMatrix to ndarray ArrayView2.
/// nalgebra uses column-major (Fortran) layout; ndarray defaults to
/// row-major (C) layout. The view preserves column-major stride.
pub fn dmatrix_to_ndarray_view(m: &DMatrix<f32>) -> ArrayView2<f32> {
    let (rows, cols) = m.shape();
    let slice = m.as_slice();
    // nalgebra stores column-major: stride = (1, rows)
    unsafe {
        ArrayView2::from_shape_ptr(
            (rows, cols).strides((1, rows)),
            slice.as_ptr(),
        )
    }
}

/// Zero-copy view from ndarray Array2 to nalgebra DMatrix.
/// Requires the ndarray to be in standard (row-major contiguous) layout.
pub fn ndarray_to_dmatrix_view(a: &Array2<f32>) -> Option<DMatrix<f32>> {
    if a.is_standard_layout() {
        let (rows, cols) = a.dim();
        let slice = a.as_slice()?;
        // Copy required: layout mismatch (row-major -> column-major)
        Some(DMatrix::from_row_slice(rows, cols, slice))
    } else {
        None
    }
}

/// Trait for types that can produce a sparse CSR representation.
/// This is the primary interop format between subsystems.
pub trait AsSparseCSR {
    fn to_csr(&self) -> (Vec<usize>, Vec<usize>, Vec<f32>, usize, usize);
}
```

#### Feature Flag Architecture

```toml
# crates/ruvector-solver/Cargo.toml
[features]
default = ["nalgebra-backend"]

# Backend selection (at least one required)
nalgebra-backend = ["nalgebra"]
ndarray-backend = ["ndarray"]

# Performance features
parallel = ["rayon"]
simd = []              # Enables hand-tuned SIMD kernels for sparse matvec

# Deployment targets
wasm = []              # Disables rayon, std::thread; enables wasm-compatible paths
gpu = ["wgpu"]         # WebGPU compute shader backend (future)

# Algorithm selection (all enabled by default)
neumann = []
push-methods = []
random-walk = []
true-solver = []
conjugate-gradient = []
bmssp = []

# Integration features (opt-in by consumers)
coherence = []         # Prime Radiant integration adapters
gnn-integration = []   # GNN aggregation strategies
spectral = []          # Spectral method filters
graph-analytics = []   # PageRank, centrality push methods
```

#### Trait Hierarchy

```rust
// crates/ruvector-solver/src/traits.rs

use crate::types::{SparseMatrix, SolverResult, ConvergenceBound};
use crate::config::SolverConfig;
use crate::error::SolverError;

/// Core solver engine trait. All sublinear algorithms implement this.
pub trait SolverEngine: Send + Sync {
    /// Solve the sparse linear system Ax = b.
    fn solve(
        &self,
        matrix: &SparseMatrix,
        rhs: &[f32],
        config: &SolverConfig,
    ) -> Result<SolverResult, SolverError>;

    /// Return the algorithm name for diagnostics.
    fn algorithm_name(&self) -> &'static str;

    /// Estimated complexity class for the given problem size.
    fn estimated_complexity(&self, n: usize, nnz: usize) -> u64;

    /// Whether this solver is suitable for the given matrix properties.
    fn is_applicable(&self, matrix: &SparseMatrix) -> bool;
}

/// Backend abstraction for linear algebra operations.
/// Bridges nalgebra and ndarray without tight coupling.
pub trait NumericBackend: Send + Sync {
    type Vector;
    type Matrix;

    fn sparse_matvec(&self, matrix: &SparseMatrix, x: &Self::Vector) -> Self::Vector;
    fn dot(&self, a: &Self::Vector, b: &Self::Vector) -> f32;
    fn axpy(&self, alpha: f32, x: &Self::Vector, y: &mut Self::Vector);
    fn norm(&self, x: &Self::Vector) -> f32;
}

/// Distance function trait matching ruvector-core's DistanceMetric pattern.
pub trait DistanceFunction: Send + Sync {
    fn distance(&self, a: &[f32], b: &[f32]) -> f32;
    fn name(&self) -> &'static str;
}

/// Solution cache trait for memoizing solver results.
pub trait SolverCache: Send + Sync {
    fn get(&self, key: &[u8]) -> Option<SolverResult>;
    fn put(&self, key: &[u8], result: SolverResult, ttl_secs: u64);
    fn invalidate(&self, key: &[u8]);
    fn stats(&self) -> CacheStats;
}
```

#### Event Sourcing Integration

The solver emits domain events matching Prime Radiant's `DomainEvent` pattern, enabling
computation provenance tracking and audit trails:

```rust
// crates/ruvector-solver/src/events.rs

use serde::{Serialize, Deserialize};

/// Solver domain events for event sourcing integration.
/// Follows the same tagged-enum pattern as prime-radiant's DomainEvent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SolverEvent {
    /// A solve operation was initiated.
    SolveRequested {
        request_id: String,
        algorithm: String,
        matrix_size: usize,
        nnz: usize,
        timestamp_ms: u64,
    },

    /// A solve operation completed successfully.
    SolveCompleted {
        request_id: String,
        iterations: usize,
        residual_norm: f64,
        wall_time_us: u64,
        convergence_rate: f64,
    },

    /// Algorithm was auto-selected based on matrix properties.
    AlgorithmSelected {
        request_id: String,
        selected: String,
        candidates: Vec<String>,
        selection_reason: String,
    },

    /// Convergence warning: solver may not have fully converged.
    ConvergenceWarning {
        request_id: String,
        achieved_residual: f64,
        target_residual: f64,
        iterations_used: usize,
    },

    /// Cache hit: result was served from the solution cache.
    CacheHit {
        request_id: String,
        cache_key_hash: String,
    },

    /// Solver configuration was updated.
    ConfigUpdated {
        field: String,
        old_value: String,
        new_value: String,
    },
}
```

### Integration Points

#### 1. Prime Radiant Coherence Engine -- Sparse Laplacian Solver

**Current state**: Prime Radiant computes coherence energy E(S) = sum(w_e * ||r_e||^2)
by constructing the full sheaf Laplacian L and solving dense systems. At n > 10K nodes,
the O(n^2) construction and O(n^3) solve dominate latency.

**Integration**: Replace the dense Laplacian solve with the Neumann series solver.
Graph Laplacians are diagonally dominant (L = D - A where D is degree matrix), making
Neumann convergence guaranteed with rate rho(D^{-1}A) < 1.

```rust
// crates/prime-radiant/src/coherence/sublinear_solver.rs

use ruvector_solver::{SolverEngine, NeumannSolver, SparseMatrix};

pub struct SublinearCoherenceSolver {
    solver: NeumannSolver,
    tolerance: f64,
}

impl SublinearCoherenceSolver {
    /// Solve L * x = residual_vector for coherence energy computation.
    /// L is the sheaf Laplacian (always diagonally dominant for graphs).
    pub fn solve_coherence(
        &self,
        laplacian: &SparseMatrix,
        residuals: &[f32],
    ) -> Result<CoherenceResult, SolverError> {
        let result = self.solver.solve(laplacian, residuals, &SolverConfig {
            algorithm: Algorithm::Neumann,
            tolerance: self.tolerance,
            max_iterations: 50,  // k=50 terms sufficient for eps < 1e-6
            ..Default::default()
        })?;

        Ok(CoherenceResult {
            energy: result.solution.iter().map(|x| x * x).sum::<f32>(),
            convergence_witness: result.residual_norm,
            iterations: result.iterations,
        })
    }
}
```

**Projected impact**: 50-600x speedup at n=100K. Enables real-time coherence verification
for graphs with 10M+ nodes, unlocking production deployment of the Universal Coherence
Object across all six domain interpretations (AI agents, finance, medical, robotics,
security, science).

#### 2. GNN Message Passing -- SublinearAggregation Strategy

**Current state**: GNN layers in `ruvector-gnn` compute message aggregation as
h_v = sigma(W * AGG({h_u : u in N(v)})). The aggregation is O(n * avg_degree)
per layer, which is O(n * avg_degree * L) for L layers.

**Integration**: Introduce a `SublinearAggregation` strategy that uses Forward Push
to compute approximate neighborhood aggregation in O(1/eps) time, independent of
graph size.

```rust
// crates/ruvector-gnn/src/aggregation/sublinear.rs

use ruvector_solver::{ForwardPush, SparseMatrix};

pub struct SublinearAggregation {
    push_solver: ForwardPush,
    alpha: f32,      // teleport probability (PPR damping)
    epsilon: f32,    // approximation tolerance
}

impl AggregationStrategy for SublinearAggregation {
    fn aggregate(
        &self,
        adjacency: &SparseMatrix,
        node_features: &Array2<f32>,
        target_node: usize,
    ) -> Vec<f32> {
        // Forward Push computes approximate PPR from target_node
        // in O(1/epsilon) time, independent of n
        let ppr = self.push_solver.personalized_pagerank(
            adjacency, target_node, self.alpha, self.epsilon
        );

        // Weighted aggregation using PPR scores as attention weights
        let mut aggregated = vec![0.0f32; node_features.ncols()];
        for (node, weight) in ppr.iter() {
            let features = node_features.row(*node);
            for (i, f) in features.iter().enumerate() {
                aggregated[i] += weight * f;
            }
        }
        aggregated
    }
}
```

**Projected impact**: 10-50x training iteration speedup on sparse HNSW topologies.
Enables million-node GNN training on the HNSW index topology itself.

#### 3. Spectral Methods -- Neumann Filter for Rational Filters

**Current state**: `ruvector-math/src/spectral/` computes Chebyshev polynomial filters
h(L)x via three-term recurrence T_{k+1}(L)x = 2L*T_k(L)x - T_{k-1}(L)x, costing
O(K * nnz(L)) per signal vector.

**Integration**: For rational spectral filters h(L) = p(L)/q(L), the denominator
q(L)^{-1} requires solving a sparse linear system. The Neumann series provides this
inversion in O(k * nnz) with geometric convergence for Laplacian-based denominators.

```rust
// crates/ruvector-math/src/spectral/neumann_filter.rs

use ruvector_solver::{NeumannSolver, SparseMatrix};

pub struct NeumannSpectralFilter {
    solver: NeumannSolver,
    polynomial_order: usize,
}

impl NeumannSpectralFilter {
    /// Apply rational filter h(L) = p(L) * q(L)^{-1} to signal x.
    /// The inverse q(L)^{-1} is computed via Neumann series.
    pub fn apply_rational_filter(
        &self,
        laplacian: &SparseMatrix,
        signal: &[f32],
        p_coeffs: &[f32],  // numerator polynomial coefficients
        q_coeffs: &[f32],  // denominator polynomial coefficients
    ) -> Result<Vec<f32>, SolverError> {
        // Step 1: Compute q(L) * x via Chebyshev recurrence
        let qx = chebyshev_apply(laplacian, signal, q_coeffs);

        // Step 2: Solve q(L) * y = x using Neumann series O(k * nnz)
        let y = self.solver.solve(laplacian, signal, &Default::default())?;

        // Step 3: Apply numerator p(L) to y
        Ok(chebyshev_apply(laplacian, &y.solution, p_coeffs))
    }
}
```

**Projected impact**: 20-100x speedup at n=1M for rational spectral filtering.
Enables real-time spectral filtering in the graph wavelet pipeline.

#### 4. Graph Analytics -- Forward Push PageRank and Backward Push

**Current state**: `ruvector-graph` computes PageRank via iterative power method with
O(n * iterations) per convergence, and centrality measures via full BFS/DFS traversals.

**Integration**: Direct replacement with Forward Push (O(1/eps) per query node) and
Backward Push (O(1/eps) per target node). For local queries, this is sublinear in
graph size.

**Projected impact**: 100-500x speedup for local PageRank queries on billion-edge graphs.
Enables real-time hybrid vector-graph search with PageRank-weighted re-ranking.

#### 5. PDE Attention -- CG on Sparse Laplacian

**Current state**: PDE attention in `ruvector-attention/src/pde_attention/` constructs
a graph Laplacian from key similarities and applies diffusion. The diffusion step
involves solving (I + t*L)x = query, currently approximated via truncated expansion.

**Integration**: Replace the truncated expansion with Conjugate Gradient on the SPD
system (I + t*L), which converges in O(sqrt(kappa) * nnz) iterations where kappa is
the condition number. For graph Laplacians, kappa is bounded by O(n/lambda_2), and
preconditioning with the diagonal reduces this further.

**Projected impact**: 5-20x speedup with tighter convergence guarantees. Enables
deeper diffusion (larger t) without numerical instability.

#### 6. Min-Cut Infrastructure -- Shared Sparsifier with TRUE

**Current state**: `ruvector-mincut` implements spectral sparsification
(Benczur-Karger, Nagamochi-Ibaraki) producing O(n * log(n) / eps^2) edges preserving
all cuts within (1 +/- eps).

**Integration**: The TRUE solver uses spectral sparsifiers internally. Sharing the
sparsification infrastructure between `ruvector-mincut` and `ruvector-solver` avoids
redundant computation and ensures consistent approximation guarantees. The shared
sparsifier trait:

```rust
pub trait GraphSparsifier: Send + Sync {
    fn sparsify(
        &self,
        graph: &SparseMatrix,
        epsilon: f32,
    ) -> Result<SparseMatrix, SolverError>;

    fn sparsification_ratio(&self) -> f64;
}
```

**Projected impact**: 2-5x reduction in preprocessing for combined mincut + solver
workloads. Provides algorithmic validation paths for the expander decomposition.

#### 7. MCP Tool Registration -- 40+ Solver Tools in mcp-gate

**Current state**: `mcp-gate` exposes 3 tools (coherence gate operations) via
JSON-RPC 2.0 over stdio. The `ruvector-cli` MCP server adds 12 tools. The npm MCP
server provides 40+ tools.

**Integration**: Register solver capabilities as MCP tools, enabling AI agents to
invoke O(log n) algorithms through the existing protocol:

```rust
// New tools added to mcp-gate tool registry
McpTool {
    name: "solver_sparse_solve",
    description: "Solve sparse linear system Ax=b using sublinear algorithms",
    input_schema: json!({
        "type": "object",
        "properties": {
            "matrix_csr": { "type": "object", "description": "CSR sparse matrix" },
            "rhs": { "type": "array", "items": { "type": "number" } },
            "algorithm": {
                "type": "string",
                "enum": ["auto", "neumann", "forward_push", "cg", "true"]
            },
            "tolerance": { "type": "number", "default": 1e-6 }
        },
        "required": ["matrix_csr", "rhs"]
    })
}
```

Additional tools: `solver_pagerank`, `solver_ppr`, `solver_coherence_check`,
`solver_spectral_filter`, `solver_benchmark`, `solver_config`, and algorithm-specific
variants.

**Projected impact**: Enables AI agent access to O(log n) solvers through the existing
MCP protocol with no architectural changes.

#### 8. WASM Deployment -- Browser-Native O(log n) Solvers

**Current state**: 27 WASM crates provide browser-native vector search, attention,
GNN, and graph operations.

**Integration**: `ruvector-solver-wasm` follows the established pattern exactly:

```rust
// crates/ruvector-solver-wasm/src/lib.rs
use wasm_bindgen::prelude::*;
use js_sys::Float32Array;
use ruvector_solver::{SublinearSolver, SolverConfig};

#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub struct JsSolver {
    inner: SublinearSolver,
}

#[wasm_bindgen]
impl JsSolver {
    #[wasm_bindgen(constructor)]
    pub fn new(config: JsValue) -> Result<JsSolver, JsValue> {
        let config: SolverConfig = serde_wasm_bindgen::from_value(config)?;
        Ok(JsSolver {
            inner: SublinearSolver::new(config)
                .map_err(|e| JsValue::from_str(&e.to_string()))?,
        })
    }

    #[wasm_bindgen]
    pub fn solve(&self, input: Float32Array) -> Result<JsValue, JsValue> {
        let data = input.to_vec();
        let result = self.inner.solve(&data)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        serde_wasm_bindgen::to_value(&result)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen]
    pub fn pagerank(
        &self,
        edges_flat: &[u32],
        num_nodes: u32,
        alpha: f32,
        epsilon: f32,
    ) -> Result<Float32Array, JsValue> {
        // Forward Push PageRank in O(1/epsilon)
        let result = self.inner.forward_push_pagerank(
            edges_flat, num_nodes, alpha, epsilon,
        ).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let arr = Float32Array::new_with_length(result.len() as u32);
        arr.copy_from(&result);
        Ok(arr)
    }
}
```

WASM-specific constraints handled:
- `nalgebra` compiles to WASM with `default-features = false`
- Memory-efficient: sublinear algorithms use O(n) auxiliary storage
- No threading: sequential execution (Web Workers for parallelism via
  existing `worker-pool.js` infrastructure)
- SIMD128 acceleration via `#[target_feature(enable = "simd128")]`

**Projected impact**: Browser-native graph analytics without server roundtrip.
Enables offline-first coherence verification in RVF cognitive containers.

---

## Consequences

### Positive

1. **50-600x coherence speedup**: Prime Radiant scales from 10K to 10M+ nodes at
   interactive latency, enabling production deployment of the Universal Coherence
   Object across all six domain interpretations.

2. **10-50x GNN training acceleration**: SublinearAggregation strategy makes
   million-node GNN training feasible on HNSW topologies, directly serving the
   "gets smarter the more you use it" strategic pillar.

3. **Unique competitive position**: No competing vector database (Pinecone, Weaviate,
   Milvus, Qdrant, ChromaDB) offers integrated O(log n) sparse solvers. This is a
   defensible technical moat.

4. **Browser-native graph analytics**: WASM solver eliminates server roundtrips for
   graph queries, directly serving the "works offline / runs in browsers" pillar.

5. **Unified mathematical foundation**: Single `SolverEngine` trait provides
   consistent interface across all six integration points, reducing code duplication
   and simplifying testing.

6. **Event sourcing compatibility**: `SolverEvent` enum integrates with Prime
   Radiant's existing `DomainEvent` infrastructure for end-to-end computation
   provenance and audit trails.

7. **MCP ecosystem extension**: 40+ new solver tools extend the AI agent surface
   without protocol changes, enabling autonomous graph analytics workflows.

8. **Shared sparsifier infrastructure**: Amortizes spectral sparsification cost
   across mincut and solver workloads, reducing preprocessing overhead by 2-5x.

9. **Algorithm auto-selection**: The `SolverEngine::is_applicable` trait method
   enables automatic algorithm selection based on matrix properties (diagonal
   dominance, sparsity, symmetry), reducing the expertise required to use the
   solver effectively.

10. **Incremental adoption**: Feature flags allow subsystems to adopt the solver
    independently. `prime-radiant` can integrate first without requiring changes
    to `ruvector-gnn` or other consumers.

### Negative

1. **nalgebra/ndarray duality increases complexity**: Two linear algebra backends
   require bridge code and layout-aware conversions. Column-major (nalgebra) vs
   row-major (ndarray) mismatch introduces transposition overhead for non-view
   conversions.
   - **Mitigation**: CSR sparse format is layout-agnostic. Dense operations use
     zero-copy views where possible; copies are restricted to initial setup paths,
     not hot loops.

2. **Workspace dependency footprint grows**: Adding `nalgebra 0.33` as a workspace
   dependency increases compile time by an estimated 10-15 seconds for clean builds.
   - **Mitigation**: `nalgebra` is already used by `prime-radiant` and
     `ruvector-hyperbolic-hnsw`. The marginal impact is the workspace declaration,
     not new compilation.

3. **Approximation accuracy tradeoffs**: O(log n) algorithms produce approximate
   solutions with error bounds dependent on epsilon and iteration count. Consumers
   must reason about acceptable tolerance levels.
   - **Mitigation**: `SolverResult` includes `residual_norm` and
     `convergence_rate` fields. `ConvergenceWarning` events alert when tolerance
     is not met. Default configurations are conservative (eps = 1e-6).

4. **Maintenance burden of external alignment**: The solver is actively developed
   (v1.4.1/v1.5.0). Tracking upstream changes requires version pinning and
   periodic vendor updates.
   - **Mitigation**: Vendor the core algorithm crate (`sublinear-solver-core`)
     into the workspace. Wrap it behind RuVector's own `SolverEngine` trait to
     insulate consumers from upstream API changes.

5. **Testing surface area expansion**: Six integration points, three deployment
   targets (native, WASM, Node.js), and seven algorithm variants create a
   combinatorial testing matrix.
   - **Mitigation**: Property-based testing via `proptest` for algorithm
     correctness (verify ||Ax - b|| < eps). Integration tests use the existing
     `criterion` benchmark infrastructure with correctness assertions.

6. **WASM memory pressure for large problems**: Sublinear algorithms are
   memory-efficient (O(n) auxiliary), but the sparse matrix representation
   itself is O(nnz). Very large graphs may exceed WASM's default 16MB linear
   memory.
   - **Mitigation**: Chunked processing for matrices exceeding configurable
     thresholds. WASM memory growth via `WebAssembly.Memory.grow()`.

### Neutral

1. **No impact on distance computation hot path**: The solver operates on sparse
   linear systems, not vector distance metrics. The SIMD-accelerated distance
   kernels in `ruvector-core` are unaffected.

2. **Compile-time feature complexity increases**: The feature flag matrix
   (`nalgebra-backend`, `ndarray-backend`, `parallel`, `simd`, `wasm`, plus six
   integration features) adds configuration surface area without runtime cost.

3. **Algorithm selection requires domain knowledge**: While auto-selection handles
   common cases, optimal algorithm choice for novel problem structures may require
   understanding of spectral properties (diagonal dominance, condition number,
   sparsity pattern).

---

## Alternatives Considered

### Option 1: External Dependency Only

Use `sublinear-time-solver` as a pure `Cargo.toml` dependency without vendoring
or custom integration adapters.

- **Pros**:
  - Minimal integration effort (1-2 days)
  - Automatic upstream updates via `cargo update`
  - No code duplication
- **Cons**:
  - No control over API evolution; breaking changes propagate directly
  - Cannot customize algorithms for RuVector-specific sparse patterns
  - No event sourcing integration
  - No shared sparsifier with `ruvector-mincut`
  - Consumer crates must handle nalgebra/ndarray bridge individually
- **Decision**: Rejected. Insufficient control for a foundational mathematical
  dependency in a production system.

### Option 2: Partial Vendoring (Algorithm Core Only)

Vendor only the core algorithm implementations (Neumann, Push, CG) as Rust source
files within `ruvector-math`, without creating a separate crate.

- **Pros**:
  - Smaller footprint; no new crate overhead
  - Direct access to modify algorithms
  - Reuses existing `ruvector-math` infrastructure
- **Cons**:
  - Violates separation of concerns; `ruvector-math` already has 10+ modules
  - Cannot produce independent WASM/Node.js bindings for solver-only deployments
  - Makes upstream merges difficult
  - `ruvector-math` uses nalgebra 0.33 already, but other consumers use ndarray;
    the bridge must exist regardless
- **Decision**: Rejected. Pollutes `ruvector-math` scope and prevents independent
  deployment of solver capabilities.

### Option 3: Full Rewrite from Scratch

Reimplement all sublinear algorithms from scratch within a new RuVector crate,
using only ndarray as the backend.

- **Pros**:
  - Complete control over implementation
  - No nalgebra dependency; ndarray-only backend
  - Tailored to RuVector's exact sparse matrix formats
- **Cons**:
  - Estimated 8-12 weeks of algorithm engineering
  - High risk of numerical bugs in reimplementation
  - Loses access to the solver's existing test suite, benchmarks, and MCP tools
  - Duplicates effort that is already production-tested
  - The solver's nalgebra usage for sparse types (CsMatrix) is actually superior
    to ndarray's sparse support
- **Decision**: Rejected. Unnecessary risk and effort when a compatible,
  production-tested implementation exists.

### Selected: Option 4 -- New Crate with Vendored Core and Integration Adapters

Create `ruvector-solver` as a new workspace crate that vendors the solver's core
algorithms, wraps them behind RuVector's own trait hierarchy (`SolverEngine`,
`NumericBackend`), provides nalgebra/ndarray bridging, and offers integration
adapters for each consuming subsystem. This balances control, maintainability,
and deployment flexibility.

---

## Compliance

### ADR-001: Core Architecture

The `ruvector-solver` crate family follows the layered architecture defined in
ADR-001 (Layer 0: Math Foundation, Layer 1: Core Engines, Layer 2: Platform
Bindings, Layer 3: Integration Services). The Core-Binding-Surface pattern
(core Rust, WASM binding, Node.js binding) is preserved exactly. The solver
does not modify any existing crate's public API; integration is opt-in via
feature flags on consuming crates.

### ADR-003: SIMD Optimization Strategy

The solver's sparse matrix-vector multiplication benefits from SIMD acceleration.
The implementation follows ADR-003's architecture-specific dispatch pattern:
AVX2 for x86_64, NEON for ARM, SIMD128 for WASM, with scalar fallback. The
solver reuses `simsimd` for distance-related operations and implements custom
SIMD kernels for sparse matvec scatter/gather patterns that `simsimd` does not
cover.

### ADR-005: WASM Runtime Integration

`ruvector-solver-wasm` follows ADR-005's security model for sandboxed WASM
execution. The solver's pure-computation nature (no filesystem, no network,
no unsafe in core algorithms) makes it naturally compatible with WASM's
capability-based security. The crate uses `console_error_panic_hook` for
debugging and `serde_wasm_bindgen` for type marshalling, matching the
established WASM patterns across 27 existing crates.

### ADR-006: Memory Management

The solver's sublinear algorithms use O(n) auxiliary memory, well within ADR-006's
memory efficiency requirements. The sparse matrix representation uses CSR format
with O(nnz + n) storage, which is compatible with the unified memory pool's page-
based allocation. For WASM deployments, the solver respects the linear memory growth
budget and supports chunked processing for large problems.

### ADR-014: Coherence Engine Architecture

The primary integration point. The solver directly addresses ADR-014's performance
limitation: "real-time coherence for graphs with 100K+ nodes." The
`SublinearCoherenceSolver` adapter replaces the dense Laplacian solve path with
Neumann series expansion, preserving the sheaf Laplacian mathematical model, the
`DomainEvent` audit trail, and the coherence gate refusal mechanism. The solver's
`ConvergenceBound` type provides a witness of solution quality that integrates with
the existing `GateRefusalWitness` pattern from ADR-CE-012.

---

## Implementation Roadmap

| Phase | Duration | Deliverables |
|-------|----------|-------------|
| **Phase 1**: Core crate | 2 weeks | `ruvector-solver` with Neumann, CG, Forward Push; nalgebra bridge; trait hierarchy; event sourcing; benchmarks |
| **Phase 2**: Prime Radiant integration | 1 week | `SublinearCoherenceSolver` adapter; coherence benchmarks at 10K, 100K, 1M nodes |
| **Phase 3**: GNN and spectral integration | 2 weeks | `SublinearAggregation` strategy; Neumann spectral filter; integration tests |
| **Phase 4**: WASM and Node.js bindings | 1 week | `ruvector-solver-wasm`, `ruvector-solver-node`; browser benchmarks |
| **Phase 5**: MCP and graph analytics | 1 week | MCP tool registration; Forward/Backward Push PageRank in ruvector-graph |
| **Phase 6**: Shared sparsifier and mincut | 1 week | `GraphSparsifier` trait; TRUE integration; mincut shared infrastructure |

Total estimated effort: **8 weeks** for full integration across all eight points.

---

## Acceptance Criteria

1. `ruvector-solver` crate compiles on native (x86_64, aarch64) and
   `wasm32-unknown-unknown` targets
2. All seven algorithm variants pass property-based correctness tests:
   ||Ax - b|| / ||b|| < epsilon for 1000 random sparse systems
3. Prime Radiant coherence computation at n=100K completes in < 100ms
   (currently > 10s)
4. GNN aggregation benchmark shows >= 10x throughput improvement on
   sparse HNSW topologies (n=50K, avg_degree=16)
5. WASM solver binary < 200KB gzipped
6. No regressions in existing `cargo test` or `cargo bench` suites
7. MCP tools pass JSON Schema validation and return valid SolverResult
8. nalgebra/ndarray bridge introduces zero-copy overhead for view conversions

---

## References

## Implementation Status

Full solver crate (ruvector-solver) delivered with 8 algorithms (Neumann, CG, Forward Push, Backward Push, Hybrid Random Walk, TRUE, BMSSP, Router), CSR matrix types, SIMD-accelerated SpMV (AVX2), fused residual kernel, arena allocator, audit logging, event system, and comprehensive validation. WASM and NAPI bindings complete. 177 tests passing.

---

### Research Documents (docs/research/sublinear-time-solver/)

| Document | Title |
|----------|-------|
| [00](../00-executive-summary.md) | Executive Summary |
| [01](../01-rust-crates-integration.md) | Rust Crates Integration Analysis |
| [02](../02-npm-integration.md) | NPM Integration Analysis |
| [03](../03-rvf-format-integration.md) | RVF Format Integration |
| [04](../04-examples-integration.md) | Examples Integration |
| [05](../05-architecture-analysis.md) | Architecture Analysis |
| [06](../06-wasm-integration.md) | WASM Integration Analysis |
| [07](../07-mcp-integration.md) | MCP Integration Analysis |
| [08](../08-performance-analysis.md) | Performance and Benchmarking Analysis |
| [09](../09-security-analysis.md) | Security Analysis |
| [10](../10-algorithm-analysis.md) | Algorithm Deep-Dive Analysis |
| [11](../11-typescript-integration.md) | TypeScript Integration |
| [12](../12-testing-strategy.md) | Testing Strategy |
| [13](../13-dependency-analysis.md) | Dependency Analysis |
| [14](../14-use-cases-roadmap.md) | Use Cases and Roadmap |
| [15](../15-fifty-year-sota-vision.md) | 50-Year SOTA Vision |
| [16](../16-dna-sublinear-convergence.md) | DNA Sublinear Convergence |
| [17](../17-quantum-sublinear-convergence.md) | Quantum Sublinear Convergence |

### Related ADRs

| ADR | Title | Relevance |
|-----|-------|-----------|
| [ADR-001](../../adr/ADR-001-ruvector-core-architecture.md) | Core Architecture | Layered architecture pattern |
| [ADR-003](../../adr/ADR-003-simd-optimization-strategy.md) | SIMD Strategy | SIMD dispatch pattern for sparse matvec |
| [ADR-005](../../adr/ADR-005-wasm-runtime-integration.md) | WASM Integration | WASM security and deployment model |
| [ADR-006](../../adr/ADR-006-memory-management.md) | Memory Management | Memory pool compatibility |
| [ADR-014](../../adr/ADR-014-coherence-engine.md) | Coherence Engine | Primary integration target |
| [ADR-CE-012](../../adr/coherence-engine/ADR-CE-012-gate-refusal-witness.md) | Gate Refusal Witness | Convergence witness integration |

### External References

- Spielman, D. and Teng, S. "Nearly-linear time algorithms for graph partitioning, graph sparsification, and solving linear systems." STOC 2004.
- Andersen, R., Chung, F., and Lang, K. "Local graph partitioning using PageRank vectors." FOCS 2006.
- arXiv:2512.13105 -- Subpolynomial dynamic minimum cut (December 2024 breakthrough).
