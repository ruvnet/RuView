# Architecture Analysis: Sublinear-Time Solver Integration with ruvector

**Agent**: 5 -- Architecture & System Design
**Date**: 2026-02-20
**Status**: Complete
**Scope**: Full-stack architectural mapping, compatibility analysis, and integration strategy

---

## Table of Contents

1. [ruvector's Current Architecture Patterns](#1-ruvectors-current-architecture-patterns)
2. [Architectural Compatibility with Sublinear-Time Solver](#2-architectural-compatibility-with-sublinear-time-solver)
3. [Layered Integration Strategy (Rust -> WASM -> JS -> API)](#3-layered-integration-strategy)
4. [Module Boundary Recommendations](#4-module-boundary-recommendations)
5. [Dependency Injection Points](#5-dependency-injection-points)
6. [Event-Driven Integration Patterns](#6-event-driven-integration-patterns)
7. [Performance Architecture Considerations](#7-performance-architecture-considerations)

---

## 1. ruvector's Current Architecture Patterns

### 1.1 Macro-Architecture: Rust Workspace Monorepo

ruvector is organized as a Cargo workspace monorepo with approximately 75+ crates under
`/crates`. The workspace configuration in `Cargo.toml` lists roughly 100 workspace members
spanning core database functionality, mathematical engines, neural systems, governance layers,
and multiple deployment targets.

**Topology**: The codebase follows a layered architecture with a clear separation between
computational cores and their platform bindings:

```
Layer 0: Mathematical Foundations
  ruvector-math, ruvector-mincut, ruqu-core, ruqu-algorithms

Layer 1: Core Engines
  ruvector-core, ruvector-graph, ruvector-dag, ruvector-sparse-inference,
  prime-radiant, sona, cognitum-gate-kernel, cognitum-gate-tilezero

Layer 2: Platform Bindings
  *-wasm crates (wasm-bindgen), *-node crates (NAPI-RS), *-ffi crates

Layer 3: Integration Services
  ruvector-server (axum REST), mcp-gate (MCP/JSON-RPC), ruvector-cli (clap)

Layer 4: Distribution & Orchestration
  ruvector-cluster, ruvector-raft, ruvector-replication, ruvector-delta-consensus
```

### 1.2 The Core-Binding-Surface Pattern

Every major subsystem in ruvector follows a consistent three-part decomposition:

| Component | Purpose | Example |
|-----------|---------|---------|
| **Core** (pure Rust) | Algorithms, data structures, business logic | `ruvector-core`, `ruvector-graph`, `ruvector-math` |
| **WASM binding** | Browser/edge deployment via `wasm-bindgen` | `ruvector-wasm`, `ruvector-graph-wasm`, `ruvector-math-wasm` |
| **Node binding** | Server-side deployment via NAPI-RS | `ruvector-node`, `ruvector-graph-node`, `ruvector-gnn-node` |

This pattern is the primary architectural convention in ruvector. It appears in at least
15 subsystems: core, graph, GNN, attention, mincut, DAG, sparse-inference, math,
domain-expansion, economy, exotic, learning, nervous-system, tiny-dancer, and the
prime-radiant advanced WASM.

Key characteristics observed in the codebase:

- **Pure Rust cores** use `no_std`-compatible patterns where possible, avoiding I/O and
  platform-specific code.
- **WASM crates** wrap core types in `#[wasm_bindgen]`-annotated structs with `JsValue`
  serialization via `serde_wasm_bindgen`. They handle browser-specific concerns like
  IndexedDB persistence, Web Worker pool management, and Float32Array interop.
- **Node crates** use `#[napi]` macros with `tokio::task::spawn_blocking` for async I/O,
  leveraging zero-copy `Float32Array` buffers through NAPI-RS.

### 1.3 Dependency Management Strategy

The workspace `Cargo.toml` centralizes all shared dependencies. Critical shared dependencies
relevant to the sublinear-time solver integration:

- **Linear algebra**: `ndarray 0.16` (ruvector-math uses this extensively)
- **Numerics**: `rand 0.8`, `rand_distr 0.4`
- **WASM**: `wasm-bindgen 0.2`, `js-sys 0.3`, `web-sys 0.3`
- **Node.js**: `napi 2.16`, `napi-derive 2.16`
- **Async**: `tokio 1.41` (multi-thread runtime), `futures 0.3`
- **SIMD**: `simsimd 5.9` (distance calculations)
- **Serialization**: `serde 1.0`, `rkyv 0.8`, `bincode 2.0.0-rc.3`
- **Concurrency**: `rayon 1.10`, `crossbeam 0.8`, `dashmap 6.1`, `parking_lot 0.12`

Notable **absence**: `nalgebra` is not currently a workspace dependency. The sublinear-time
solver uses `nalgebra` as its linear algebra backend. This is a significant compatibility
consideration (analyzed in Section 2).

### 1.4 Feature Flag Architecture

ruvector makes extensive use of Cargo feature flags for conditional compilation:

- `storage` / `storage-memory`: Toggle between REDB-backed and in-memory storage
- `parallel`: Enables lock-free structures and rayon parallelism (disabled on `wasm32`)
- `collections`: Multi-collection support (requires file I/O, so conditionally excluded in WASM)
- `kernel-pack`: ADR-005 compliant secure WASM kernel execution
- `full`: Enables async-dependent modules (healing, qudag, sona) in the DAG crate
- `api-embeddings` / `real-embeddings`: External embedding model support

### 1.5 Event Sourcing and Domain Events

The `prime-radiant` crate implements a comprehensive event sourcing pattern through its
`events.rs` module. Domain events are defined as a tagged enum (`DomainEvent`) covering:

- Substrate events (NodeCreated, NodeUpdated, NodeRemoved, EdgeCreated, EdgeRemoved)
- Coherence computation events (energy calculations, residual updates)
- Governance events (policy changes, witness records)

Events are serialized with `serde` using `#[serde(tag = "type")]` for deterministic replay
and tamper detection via content hashes. This aligns well with the sublinear-time solver's
potential need for computation provenance tracking.

### 1.6 MCP Integration Pattern

The `mcp-gate` crate provides a Model Context Protocol server using JSON-RPC 2.0 over stdio.
Tools are defined declaratively with JSON Schema input specifications. The architecture uses
`Arc<RwLock<TileZero>>` for shared state with the coherence gate engine. This existing MCP
infrastructure provides a natural extension point for exposing solver capabilities to AI agents.

### 1.7 Server Architecture

`ruvector-server` uses `axum` with tower middleware layers (compression, CORS, tracing).
Routes are modular (health, collections, points). The server shares application state via
`AppState` and uses the standard Rust web service pattern with `Router` composition.

---

## 2. Architectural Compatibility with Sublinear-Time Solver

### 2.1 Structural Alignment Matrix

| Solver Component | ruvector Equivalent | Compatibility | Notes |
|-----------------|--------------------|----|-------|
| Rust core library (`sublinear_solver`) | `ruvector-core`, `ruvector-math` | **HIGH** | Both are pure Rust crates with algorithm-focused design |
| WASM layer (`wasm-bindgen`) | `ruvector-wasm`, `*-wasm` crates | **HIGH** | Identical binding technology, identical patterns |
| JS bridge (`solver.js`, etc.) | `npm/core/src/index.ts` | **HIGH** | Both provide platform-detection loaders and typed APIs |
| Express server | `ruvector-server` (axum) | **MEDIUM** | Different frameworks (Express vs axum) but compatible at API level |
| MCP integration (40+ tools) | `mcp-gate` (3 tools) | **HIGH** | Same protocol, ruvector has established patterns |
| CLI (NPX) | `ruvector-cli` (clap) | **MEDIUM** | Different CLI paradigms; ruvector uses native Rust CLI |
| TypeScript types | `npm/core/src/index.ts` | **HIGH** | ruvector already publishes TypeScript definitions |
| 9 workspace crates | ~75+ workspace crates | **HIGH** | Same Cargo workspace model |

### 2.2 Linear Algebra Backend Divergence

**This is the single most significant architectural tension.**

- **Sublinear-time solver**: Uses `nalgebra` for matrix operations, linear algebra, and
  numerical computation.
- **ruvector**: Uses `ndarray 0.16` in `ruvector-math` and raw `Vec<f32>` with SIMD intrinsics
  in `ruvector-core`.

**Resolution strategy**: Introduce `nalgebra` as a workspace dependency and create an
adapter layer. The two libraries can coexist. The adapter should provide zero-cost conversions
between `nalgebra::DMatrix<f32>` and `ndarray::Array2<f32>` views using shared memory backing.
Specifically:

```rust
// Proposed adapter in crates/ruvector-math/src/nalgebra_bridge.rs
use nalgebra::DMatrix;
use ndarray::Array2;

/// Zero-copy view conversion from nalgebra DMatrix to ndarray Array2
pub fn dmatrix_to_ndarray_view(m: &DMatrix<f32>) -> ndarray::ArrayView2<f32> {
    let (rows, cols) = m.shape();
    let slice = m.as_slice();
    ndarray::ArrayView2::from_shape((rows, cols), slice)
        .expect("nalgebra DMatrix is always contiguous column-major")
}
```

Note: `nalgebra` uses column-major storage while `ndarray` defaults to row-major. The adapter
must handle layout transposition or use `.reversed_axes()` for correct interpretation.

### 2.3 Server Framework Compatibility

The sublinear-time solver uses Express.js with session management and streaming. ruvector
uses axum (Rust). These are not in conflict because they serve different layers:

- **Solver Express server**: JS-level API for browser and Node clients, session management,
  streaming results.
- **ruvector axum server**: Rust-level REST API for database operations.

The integration should layer the solver's Express functionality as a separate API surface,
or preferably, expose solver endpoints through axum with the same streaming semantics using
axum's SSE (Server-Sent Events) or WebSocket support.

### 2.4 WASM Compilation Target Compatibility

Both projects target `wasm32-unknown-unknown` via `wasm-bindgen`. ruvector already manages
the WASM-specific constraints:

- No `std::fs`, `std::net` in WASM builds
- `parking_lot::Mutex` instead of `std::sync::Mutex` (which does not panic on web)
- `getrandom` with `wasm_js` feature for random number generation
- Console error panic hooks for debugging

The sublinear-time solver's WASM layer should be able to reuse these patterns directly. The
existing `ruvector-wasm` crate demonstrates the complete pattern including IndexedDB persistence,
Web Worker pools, Float32Array interop, and SIMD detection.

---

## 3. Layered Integration Strategy

### 3.1 Layer Architecture Overview

```
+===========================================================================+
|                        APPLICATION CONSUMERS                               |
|  MCP Agents | REST Clients | Browser Apps | CLI Users | Edge Devices       |
+===========================================================================+
         |              |             |            |            |
+===========================================================================+
|                        API SURFACE (Layer 4)                               |
|  mcp-gate          | ruvector-server | solver-server | ruvector-cli        |
|  (JSON-RPC/stdio)  | (axum REST)     | (axum SSE)    | (clap binary)      |
+===========================================================================+
         |              |             |            |
+===========================================================================+
|                        JS/TS BRIDGE (Layer 3)                              |
|  npm/core/index.ts | solver-bridge.ts | solver-worker.ts                  |
|  Platform detection, typed wrappers, async coordination                    |
+===========================================================================+
         |              |             |
+===========================================================================+
|                        WASM SURFACE (Layer 2)                              |
|  ruvector-wasm     | ruvector-solver-wasm | ruvector-math-wasm            |
|  wasm-bindgen, Float32Array, Web Workers, IndexedDB                       |
+===========================================================================+
         |              |
+===========================================================================+
|                        RUST CORE (Layer 1)                                 |
|  ruvector-core     | ruvector-solver | ruvector-math | ruvector-dag       |
|  Pure algorithms, nalgebra/ndarray, SIMD, rayon                           |
+===========================================================================+
         |
+===========================================================================+
|                        MATH FOUNDATION (Layer 0)                           |
|  nalgebra | ndarray | simsimd | ndarray-linalg (optional)                 |
+===========================================================================+
```

### 3.2 Layer 0 -> Layer 1: Rust Core Integration

**New crate**: `crates/ruvector-solver` (or `crates/sublinear-solver` if preserving the
upstream name is preferred).

Structure:

```
crates/ruvector-solver/
  Cargo.toml
  src/
    lib.rs           # Public API: traits, types, re-exports
    algorithms/
      mod.rs         # Algorithm registry
      bmssp.rs       # Bounded Max-Sum Subarray Problem solver
      fast.rs        # Fast solver variants
      sublinear.rs   # Core sublinear-time algorithms
    backend/
      mod.rs         # Backend abstraction
      nalgebra.rs    # nalgebra-backed implementation
      ndarray.rs     # ndarray bridge for ruvector interop
    config.rs        # Solver configuration
    error.rs         # Error types
    types.rs         # Core domain types (matrices, results, bounds)
```

Integration points with existing ruvector crates:

- **`ruvector-math`**: The solver's mathematical operations (optimal transport, spectral
  methods, tropical algebra) overlap with `ruvector-math`. Common abstractions should be
  extracted into shared traits.
- **`ruvector-dag`**: Sublinear graph algorithms can be applied to DAG bottleneck analysis.
  The `DagMinCutEngine` already uses subpolynomial O(n^0.12) bottleneck detection; solver
  algorithms could provide alternative or improved implementations.
- **`ruvector-sparse-inference`**: Sparse matrix operations and activation-locality patterns
  in the inference engine are natural consumers of sublinear-time solvers.

### 3.3 Layer 1 -> Layer 2: WASM Compilation

**New crate**: `crates/ruvector-solver-wasm`

This follows the established ruvector pattern exactly:

```rust
// crates/ruvector-solver-wasm/src/lib.rs
use wasm_bindgen::prelude::*;
use ruvector_solver::{SublinearSolver, SolverConfig, SolverResult};

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
        let solver = SublinearSolver::new(config)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(JsSolver { inner: solver })
    }

    #[wasm_bindgen]
    pub fn solve(&self, input: Float32Array) -> Result<JsValue, JsValue> {
        let data = input.to_vec();
        let result = self.inner.solve(&data)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        serde_wasm_bindgen::to_value(&result)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}
```

Critical WASM considerations:

1. **nalgebra WASM compatibility**: `nalgebra` compiles to WASM without issues. Ensure
   `default-features = false` if the `std` feature pulls in incompatible dependencies.
2. **Memory limits**: WASM linear memory is limited (default 256 pages = 16MB). Sublinear
   algorithms are inherently memory-efficient, which is an advantage. However, large matrix
   operations may need chunked processing.
3. **No threads by default**: WASM does not support `std::thread`. Use the existing
   `worker-pool.js` and `worker.js` patterns from `ruvector-wasm` for parallelism.

### 3.4 Layer 2 -> Layer 3: JavaScript Bridge

**New package**: `npm/solver/` (or extension of `npm/core/`)

```typescript
// npm/solver/src/index.ts
import { SublinearSolver as WasmSolver } from '../pkg/ruvector_solver_wasm';

export interface SolverConfig {
  algorithm: 'bmssp' | 'fast' | 'sublinear';
  tolerance?: number;
  maxIterations?: number;
  dimensions?: number;
}

export interface SolverResult {
  solution: Float32Array;
  iterations: number;
  converged: boolean;
  residualNorm: number;
  wallTimeMs: number;
}

export class SublinearSolver {
  private inner: WasmSolver;

  constructor(config: SolverConfig) {
    this.inner = new WasmSolver(config);
  }

  solve(input: Float32Array): SolverResult {
    return this.inner.solve(input);
  }

  async solveAsync(input: Float32Array): Promise<SolverResult> {
    // Offload to Web Worker for non-blocking execution
    return workerPool.dispatch('solve', { input, config: this.config });
  }
}
```

### 3.5 Layer 3 -> Layer 4: API Surface

For the axum-based server integration, add a new route module:

```rust
// crates/ruvector-server/src/routes/solver.rs
use axum::{extract::State, Json, response::sse::Event};
use ruvector_solver::{SublinearSolver, SolverConfig};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/solver/solve", post(solve))
        .route("/solver/solve/stream", post(solve_stream))
        .route("/solver/config", get(get_config).put(update_config))
}
```

For the MCP integration, add new tools to `mcp-gate`:

```rust
McpTool {
    name: "solve_sublinear".to_string(),
    description: "Execute a sublinear-time solver on the provided input data".to_string(),
    input_schema: serde_json::json!({
        "type": "object",
        "properties": {
            "algorithm": { "type": "string", "enum": ["bmssp", "fast", "sublinear"] },
            "input": { "type": "array", "items": { "type": "number" } },
            "tolerance": { "type": "number", "default": 1e-6 }
        },
        "required": ["algorithm", "input"]
    }),
}
```

---

## 4. Module Boundary Recommendations

### 4.1 Boundary Principles

The following boundaries should be enforced through Cargo crate visibility and trait-based
abstraction:

```
                    PUBLIC API BOUNDARY
                    ===================
                           |
            +--------------+--------------+
            |                             |
    Solver Core Trait              ruvector Core Trait
    (SolverEngine)                 (VectorDB, SearchEngine)
            |                             |
            +------+------+       +-------+------+
            |      |      |       |       |      |
         BMSSP   Fast  Sublin   HNSW   Graph   DAG
```

### 4.2 Recommended Trait Boundaries

**Solver engine trait** (new, in `ruvector-solver`):

```rust
pub trait SolverEngine: Send + Sync {
    type Input;
    type Output;
    type Error: std::error::Error;

    fn solve(&self, input: &Self::Input) -> Result<Self::Output, Self::Error>;
    fn solve_with_budget(
        &self,
        input: &Self::Input,
        budget: ComputeBudget,
    ) -> Result<Self::Output, Self::Error>;
    fn estimate_complexity(&self, input: &Self::Input) -> ComplexityEstimate;
}
```

**Numeric backend trait** (new, in `ruvector-math` or `ruvector-solver`):

```rust
pub trait NumericBackend: Send + Sync {
    type Matrix;
    type Vector;

    fn mat_mul(&self, a: &Self::Matrix, b: &Self::Matrix) -> Self::Matrix;
    fn svd(&self, m: &Self::Matrix) -> (Self::Matrix, Self::Vector, Self::Matrix);
    fn eigenvalues(&self, m: &Self::Matrix) -> Self::Vector;
    fn norm(&self, v: &Self::Vector) -> f64;
}
```

This trait allows the solver to abstract over `nalgebra` and `ndarray` backends, and also
enables future GPU-accelerated backends (the `prime-radiant` crate already has a GPU module
with buffer management and kernel dispatch).

### 4.3 Crate Dependency Graph (Proposed)

```
ruvector-solver-wasm -----> ruvector-solver -----> ruvector-math
        |                         |                     |
        |                         |                     +---> nalgebra (new dep)
        |                         |                     +---> ndarray  (existing)
        |                         |
        |                         +---> ruvector-core (optional, for VectorDB integration)
        |
        +---> wasm-bindgen, serde_wasm_bindgen (existing workspace deps)

ruvector-solver-node -----> ruvector-solver
        |
        +---> napi, napi-derive (existing workspace deps)

mcp-gate -----> ruvector-solver (optional feature)
ruvector-server -----> ruvector-solver (optional feature)
ruvector-dag -----> ruvector-solver (optional feature for bottleneck algorithms)
```

### 4.4 Feature Flag Recommendations

```toml
[features]
default = []
nalgebra-backend = ["nalgebra"]
ndarray-backend = ["ndarray"]
wasm = ["wasm-bindgen", "serde_wasm_bindgen", "js-sys"]
parallel = ["rayon"]
simd = []  # Auto-detected via cfg(target_feature)
gpu = ["ruvector-math/gpu"]
full = ["nalgebra-backend", "ndarray-backend", "parallel"]
```

---

## 5. Dependency Injection Points

### 5.1 Core DI Architecture

ruvector uses a combination of generic type parameters and `Arc<dyn Trait>` for dependency
injection. The following injection points are relevant for the sublinear-time solver:

#### 5.1.1 Numeric Backend Injection

The solver's core algorithm implementations should accept a generic numeric backend:

```rust
pub struct SublinearSolver<B: NumericBackend = NalgebraBackend> {
    backend: B,
    config: SolverConfig,
}

impl<B: NumericBackend> SublinearSolver<B> {
    pub fn with_backend(backend: B, config: SolverConfig) -> Self {
        Self { backend, config }
    }
}
```

This allows ruvector consumers who already have `ndarray` matrices to use the solver
without conversion overhead.

#### 5.1.2 Distance Function Injection

ruvector-core's `DistanceMetric` enum defines four distance functions (Euclidean, Cosine,
DotProduct, Manhattan). The solver may need additional distance metrics or custom distance
functions. Injection point:

```rust
pub trait DistanceFunction: Send + Sync {
    fn distance(&self, a: &[f32], b: &[f32]) -> f32;
    fn name(&self) -> &str;
}

// Adapt ruvector's existing DistanceMetric
impl DistanceFunction for DistanceMetric {
    fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        match self {
            DistanceMetric::Euclidean => simsimd_euclidean(a, b),
            DistanceMetric::Cosine => simsimd_cosine(a, b),
            // ...
        }
    }
}
```

#### 5.1.3 Storage Backend Injection

ruvector-core already has conditional compilation for storage backends (`storage` vs
`storage_memory`). The solver should use a similar pattern for result caching:

```rust
pub trait SolverCache: Send + Sync {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>>;
    fn put(&self, key: &[u8], value: &[u8]);
    fn invalidate(&self, key: &[u8]);
}
```

Implementations could include:
- `InMemoryCache` (default, using `DashMap`)
- `VectorDBCache` (using ruvector-core's VectorDB for nearest-neighbor result caching)
- `WasmCache` (using IndexedDB, following the `ruvector-wasm/src/indexeddb.js` pattern)

#### 5.1.4 Compute Budget Injection

Following `prime-radiant`'s compute ladder pattern (Lane 0 Reflex through Lane 3 Human),
the solver should accept compute budgets:

```rust
pub struct ComputeBudget {
    pub max_wall_time: Duration,
    pub max_iterations: usize,
    pub max_memory_bytes: usize,
    pub lane: ComputeLane,
}

pub enum ComputeLane {
    Reflex,     // < 1ms, local only
    Retrieval,  // ~ 10ms, can fetch cached results
    Heavy,      // ~ 100ms, full solver execution
    Deliberate, // unbounded, with streaming progress
}
```

### 5.2 WASM-Specific Injection Points

In the WASM layer, dependency injection occurs through JavaScript configuration objects:

```typescript
interface SolverOptions {
  // Backend selection
  backend?: 'wasm-simd' | 'wasm-baseline' | 'js-fallback';

  // Worker pool configuration
  workerCount?: number;
  workerUrl?: string;

  // Memory management
  maxMemoryMB?: number;
  useSharedArrayBuffer?: boolean;

  // Progress callback (for streaming)
  onProgress?: (progress: SolverProgress) => void;
}
```

### 5.3 Server-Level Injection

At the API layer, the solver should be injected into the axum `AppState`:

```rust
pub struct AppState {
    // Existing
    pub vector_db: Arc<RwLock<CoreVectorDB>>,
    pub collection_manager: Arc<RwLock<CoreCollectionManager>>,

    // New: solver engine injection
    pub solver: Arc<dyn SolverEngine<Input = SolverInput, Output = SolverOutput, Error = SolverError>>,
}
```

---

## 6. Event-Driven Integration Patterns

### 6.1 Alignment with Prime-Radiant Event Sourcing

The `prime-radiant` crate's `DomainEvent` enum provides a proven event-sourcing pattern.
The solver should emit analogous events for computation provenance:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SolverEvent {
    /// A solve request was received
    SolveRequested {
        request_id: String,
        algorithm: String,
        input_dimensions: (usize, usize),
        timestamp: Timestamp,
    },

    /// An iteration completed
    IterationCompleted {
        request_id: String,
        iteration: usize,
        residual_norm: f64,
        wall_time_us: u64,
        timestamp: Timestamp,
    },

    /// The solver converged to a solution
    SolveConverged {
        request_id: String,
        total_iterations: usize,
        final_residual: f64,
        total_wall_time_us: u64,
        timestamp: Timestamp,
    },

    /// The solver exceeded its compute budget
    BudgetExhausted {
        request_id: String,
        budget: ComputeBudget,
        best_residual: f64,
        timestamp: Timestamp,
    },

    /// A complexity estimate was computed
    ComplexityEstimated {
        request_id: String,
        estimated_flops: u64,
        estimated_memory_bytes: u64,
        recommended_lane: ComputeLane,
        timestamp: Timestamp,
    },
}
```

### 6.2 Event Bus Integration

The solver events should be published to the same event infrastructure that prime-radiant
uses. The recommended pattern is a channel-based event bus:

```rust
pub struct SolverWithEvents<S: SolverEngine> {
    solver: S,
    event_tx: tokio::sync::broadcast::Sender<SolverEvent>,
}

impl<S: SolverEngine> SolverWithEvents<S> {
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<SolverEvent> {
        self.event_tx.subscribe()
    }
}
```

This enables:
- **Coherence gate integration**: Prime-radiant can subscribe to solver events and include
  solver stability in its coherence energy calculations.
- **Streaming API responses**: The axum server can convert the event stream to SSE.
- **MCP progress notifications**: The MCP server can emit JSON-RPC notifications for
  long-running solve operations.
- **Telemetry and monitoring**: The `ruvector-metrics` crate can subscribe and export
  Prometheus metrics for solver operations.

### 6.3 Coherence Gate as Solver Governor

A powerful integration pattern connects the solver to prime-radiant's coherence gate:

```
Solve Request --> Complexity Estimate --> Gate Decision --> Execute or Escalate
                                              |
                                     Prime-Radiant evaluates:
                                     - Energy budget available?
                                     - System coherence stable?
                                     - Resource contention low?
```

The `cognitum-gate-tilezero` crate's `permit_action` tool can govern solver execution:

```rust
// Before executing a solver, request permission from the gate
let action = ActionContext {
    action_id: format!("solve-{}", request_id),
    action_type: "heavy_compute".into(),
    target: ActionTarget {
        device: "solver-engine".into(),
        path: format!("/solver/{}", algorithm),
    },
    metadata: ActionMetadata {
        estimated_cost: complexity.estimated_flops as f64,
        estimated_duration_ms: complexity.estimated_wall_time_ms,
    },
};

match gate.permit_action(action).await {
    GateDecision::Permit(token) => solver.solve_with_token(input, token),
    GateDecision::Defer(info) => escalate_to_queue(input, info),
    GateDecision::Deny(reason) => Err(SolverError::Denied(reason)),
}
```

### 6.4 DAG Integration Events

The `ruvector-dag` crate's query plan optimizer can emit events when bottleneck analysis
identifies nodes that would benefit from sublinear-time solving:

```rust
// In ruvector-dag when a bottleneck is detected
SolverEvent::BottleneckSolverRequested {
    dag_id: dag.id(),
    bottleneck_nodes: bottlenecks.iter().map(|b| b.node_id).collect(),
    estimated_speedup: bottlenecks.iter().map(|b| b.speedup_potential).sum(),
    timestamp: now(),
}
```

---

## 7. Performance Architecture Considerations

### 7.1 Memory Architecture

#### Current ruvector Memory Model

ruvector-core uses several memory optimization strategies:

- **Arena allocator** (`arena.rs`): Cache-aligned vector allocation with `CACHE_LINE_SIZE`
  awareness and batch allocation via `BatchVectorAllocator`.
- **SoA storage** (`cache_optimized.rs`): Structure-of-Arrays layout for cache-friendly
  sequential access to vector components.
- **Memory pools** (`memory.rs`): Basic allocation tracking with optional limits.
- **Paged memory** (ADR-006): 2MB page-granular allocation with LRU eviction and
  Hot/Warm/Cold residency tiers.

#### Solver Memory Requirements

Sublinear-time algorithms are inherently memory-efficient (often O(n^alpha) for alpha < 1),
but the nalgebra backend may allocate large intermediate matrices. Recommendations:

1. **Use ruvector's arena allocator** for solver-internal scratch space. Wrap nalgebra
   allocations in arena-backed storage:

   ```rust
   pub struct SolverArena {
       inner: Arena,
       scratch_matrices: Vec<DMatrix<f32>>,
   }
   ```

2. **Integrate with ADR-006 paged memory** for large problem instances. The solver should
   respect the memory pool's limit and request pages through the established interface rather
   than allocating directly.

3. **WASM memory budget**: In WASM, limit solver memory to a configurable fraction of the
   linear memory. The default WASM memory of 16MB is tight; ensure the solver can operate
   within 4-8MB for typical problem sizes, using the `ComputeBudget.max_memory_bytes` field.

### 7.2 SIMD Optimization Strategy

ruvector uses `simsimd 5.9` for distance calculations, achieving approximately 16M ops/sec
for 512-dimensional vectors. The solver should leverage SIMD at two levels:

1. **Auto-vectorization**: Write inner loops in a SIMD-friendly style (sequential access,
   no branches, aligned data). Rust's LLVM backend will auto-vectorize these for both native
   and WASM targets.

2. **Explicit SIMD**: For hot paths, use `std::arch` intrinsics with runtime detection:

   ```rust
   #[cfg(target_arch = "x86_64")]
   use std::arch::x86_64::*;

   #[cfg(target_arch = "wasm32")]
   use std::arch::wasm32::*;
   ```

   The existing `ruvector-core/src/simd_intrinsics.rs` provides patterns for this.

3. **WASM SIMD128**: The `ruvector-wasm` crate already detects SIMD support via
   `detect_simd()`. Ensure the solver WASM crate is compiled with `-C target-feature=+simd128`
   for WASM SIMD support, with a non-SIMD fallback.

### 7.3 Concurrency Architecture

#### Native (Server) Concurrency

ruvector uses a rich concurrency toolkit:

- **Rayon** for data-parallel operations (conditional on `feature = "parallel"`)
- **Crossbeam** for lock-free data structures
- **DashMap** for concurrent hash maps
- **Parking_lot** for efficient mutexes and RwLocks
- **Tokio** for async I/O and task scheduling
- **Lock-free structures** (`lockfree.rs`): `AtomicVectorPool`, `LockFreeWorkQueue`,
  `LockFreeBatchProcessor`

The solver should integrate with this concurrency model:

```rust
impl SublinearSolver {
    pub fn solve_parallel(&self, input: &[f32]) -> Result<SolverResult> {
        #[cfg(feature = "parallel")]
        {
            input.par_chunks(self.config.chunk_size)
                .map(|chunk| self.solve_chunk(chunk))
                .reduce_with(|a, b| self.merge_results(a?, b?))
                .unwrap_or(Err(SolverError::EmptyInput))
        }
        #[cfg(not(feature = "parallel"))]
        {
            self.solve_sequential(input)
        }
    }
}
```

#### WASM Concurrency

WASM does not support native threads. The solver must use Web Workers for parallelism:

- Follow the `ruvector-wasm/src/worker-pool.js` pattern
- Use `SharedArrayBuffer` for zero-copy data sharing between workers (requires
  `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp`)
- Fall back to `postMessage` with transferable `ArrayBuffer` when SAB is unavailable

### 7.4 Latency Targets by Deployment Context

| Context | Target Latency | Memory Budget | Strategy |
|---------|---------------|---------------|----------|
| **WASM (browser)** | < 50ms for 10K elements | 4-8 MB | SIMD128, single-threaded, streaming |
| **WASM (edge/Cloudflare)** | < 10ms for 10K elements | 128 MB | SIMD128, limited workers |
| **Node.js (NAPI)** | < 5ms for 10K elements | 512 MB | Native SIMD, Rayon parallel |
| **Server (axum)** | < 2ms for 10K elements | 2 GB | Full SIMD, Rayon, memory-mapped |
| **MCP (agent)** | Budget-dependent | Configurable | Gate-governed, compute ladder |

### 7.5 Benchmarking Integration

ruvector uses `criterion 0.5` for benchmarking with HTML reports. The solver should integrate
into the existing benchmark infrastructure:

```rust
// benches/solver_benchmarks.rs
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use ruvector_solver::{SublinearSolver, SolverConfig};

fn bench_sublinear_solve(c: &mut Criterion) {
    let mut group = c.benchmark_group("sublinear_solver");

    for size in [100, 1_000, 10_000, 100_000] {
        group.bench_with_input(
            BenchmarkId::new("bmssp", size),
            &size,
            |b, &size| {
                let solver = SublinearSolver::new(SolverConfig::default());
                let input: Vec<f32> = (0..size).map(|i| i as f32).collect();
                b.iter(|| solver.solve(&input));
            },
        );
    }
    group.finish();
}
```

The benchmark results should be stored in the existing `bench_results/` directory in JSON
format, matching the schema used by `comparison_benchmark.json` and `latency_benchmark.json`.

### 7.6 Profile-Guided Optimization

The workspace `Cargo.toml` already configures aggressive release optimizations:

```toml
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
strip = true
```

These settings are critical for solver performance. Additional considerations:

- **PGO (Profile-Guided Optimization)**: For the NAPI binary, consider adding a PGO training
  step using representative solver workloads.
- **WASM opt**: Run `wasm-opt -O3` on the solver WASM output (the existing build scripts
  in `ruvector-wasm` likely already do this).
- **Link-time optimization across crates**: The `lto = "fat"` setting enables cross-crate
  LTO, which is essential for inlining nalgebra operations into solver hot paths.

### 7.7 Zero-Copy Data Path

The critical performance path for the solver is the data pipeline from API input to solver
core and back. Minimize copies:

```
API (axum): body bytes  --deserialize-->  SolverInput
                                               |
                            +---------borrow-----------+
                            |                          |
                      nalgebra::DMatrixSlice      result buffer
                            |                          |
                            +------solve-------->------+
                                                       |
                                          --serialize-->  API response bytes
```

For the WASM path:

```
JS Float32Array --view (no copy)--> wasm linear memory --solve--> wasm linear memory
                                                                       |
                                                         --view (no copy)--> JS Float32Array
```

The key is to use `Float32Array::view()` in wasm-bindgen rather than `Float32Array::copy_from()`
wherever the solver does not need to retain ownership of the input data.

---

## Summary of Key Recommendations

1. **Create `crates/ruvector-solver`** as a new pure-Rust workspace member, following the
   established core-binding-surface pattern.

2. **Add `nalgebra` as a workspace dependency** and create a bridge module in `ruvector-math`
   for zero-cost conversions between nalgebra and ndarray representations.

3. **Follow the existing three-crate pattern** exactly: `ruvector-solver` (core),
   `ruvector-solver-wasm` (browser), `ruvector-solver-node` (server).

4. **Integrate with prime-radiant's event sourcing** by emitting `SolverEvent`s through
   a broadcast channel, enabling coherence gate governance and streaming API responses.

5. **Use the coherence gate as a solver governor** to prevent runaway computation and
   integrate with the compute ladder (Lane 0-3).

6. **Inject the solver into `AppState`** for axum server integration, and add new MCP
   tools to `mcp-gate` for AI agent access.

7. **Respect ruvector's memory architecture** by integrating with the arena allocator,
   SoA storage patterns, and ADR-006 paged memory management.

8. **Target WASM SIMD128** for browser performance, with graceful fallback to scalar code
   detected at runtime via the existing `detect_simd()` mechanism.

9. **Use Rayon with feature gating** for native parallelism, and Web Workers for WASM
   parallelism, following the patterns already established in `ruvector-wasm`.

10. **Integrate benchmarks into the existing `criterion` infrastructure** and store results
    in the `bench_results/` directory for regression tracking.
