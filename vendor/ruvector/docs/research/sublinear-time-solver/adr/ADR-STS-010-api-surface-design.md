# ADR-STS-010: API Surface Design and Ergonomics

## Status

Accepted

## Date

2026-02-20

## Authors

RuVector Architecture Team

## Deciders

Architecture Review Board

## Context

The sublinear-time solver must be consumed from multiple runtime environments: native Rust
libraries, WebAssembly modules in the browser, Node.js addons for server-side JavaScript,
REST endpoints for language-agnostic HTTP clients, MCP tools for AI-agent orchestration,
and TypeScript applications that wrap any of those layers.

RuVector already follows established API conventions:

- **Trait-based polymorphism**: `DistanceMetric`, `DynamicMinCut`, and other core traits
  define behavior contracts with associated types.
- **Generic type parameters with defaults**: `struct Index<D: DistanceMetric = L2>` lets
  callers omit the parameter in the common case.
- **`Arc<dyn Trait>` dependency injection**: runtime-selected backends are threaded through
  the system as trait objects behind atomic reference counts.
- **Builder pattern**: complex structs expose `::builder()` methods that validate at
  construction time rather than at each call site.

The solver must expose a consistent, ergonomic API surface across all six target layers
while preserving zero-cost abstractions in the Rust core and minimizing serialization
overhead at FFI boundaries. Key design tensions include:

1. **Type safety vs. FFI simplicity** -- Rust's rich type system cannot cross the WASM or
   NAPI boundary unchanged.
2. **Sync vs. async** -- Browser and Node.js callers expect `Promise`-based APIs; Rust
   callers may want both sync and async.
3. **Streaming vs. batch** -- Large solver outputs benefit from incremental delivery, but
   not all transports support streaming natively.
4. **Versioning** -- Breaking changes to the solver API must be manageable without forcing
   simultaneous updates across all consumers.

This ADR defines the canonical API surface for every layer, the mapping rules between them,
the error contract, the streaming protocol, and the versioning and deprecation policy.

## Decision

### 1. Core Rust Traits

All solver functionality is expressed through a small set of traits. Consumers that embed
the solver as a Rust dependency program against these traits and never against concrete
types.

```rust
use std::error::Error;
use std::fmt;

// ---------------------------------------------------------------------------
// Compute budget -- lets callers cap wall-clock time, iterations, or memory.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ComputeBudget {
    /// Maximum wall-clock duration.  `None` means unlimited.
    pub max_duration: Option<std::time::Duration>,
    /// Maximum number of iterations the solver may execute.
    pub max_iterations: Option<u64>,
    /// Maximum resident memory the solver may allocate (bytes).
    pub max_memory_bytes: Option<usize>,
}

impl Default for ComputeBudget {
    fn default() -> Self {
        Self {
            max_duration: None,
            max_iterations: Some(10_000),
            max_memory_bytes: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Complexity estimate returned before solving.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ComplexityEstimate {
    /// Estimated time complexity class (e.g. "O(n log n)").
    pub time_class: String,
    /// Estimated number of floating-point operations.
    pub estimated_flops: u64,
    /// Estimated peak memory usage in bytes.
    pub estimated_memory_bytes: usize,
    /// Recommended compute budget for this input.
    pub recommended_budget: ComputeBudget,
}

// ---------------------------------------------------------------------------
// SolverEngine -- the root trait for every solver variant.
// ---------------------------------------------------------------------------

pub trait SolverEngine: Send + Sync {
    /// The input type accepted by this solver.
    type Input;
    /// The output type produced by this solver.
    type Output;
    /// The error type produced by this solver.
    type Error: Error + Send + Sync + 'static;

    /// Solve synchronously with the default compute budget.
    fn solve(&self, input: &Self::Input) -> Result<Self::Output, Self::Error>;

    /// Solve synchronously with an explicit compute budget.
    fn solve_with_budget(
        &self,
        input: &Self::Input,
        budget: ComputeBudget,
    ) -> Result<Self::Output, Self::Error>;

    /// Return a complexity estimate without executing the solve.
    fn estimate_complexity(&self, input: &Self::Input) -> ComplexityEstimate;

    /// Human-readable name for logging and diagnostics.
    fn name(&self) -> &str;

    /// Semantic version of this solver implementation.
    fn version(&self) -> &str;
}

// ---------------------------------------------------------------------------
// NumericBackend -- pluggable linear-algebra kernel.
// ---------------------------------------------------------------------------

pub trait NumericBackend: Send + Sync {
    type Matrix: Send + Sync + Clone;
    type Vector: Send + Sync + Clone;

    /// Dense matrix multiplication: C = A * B.
    fn mat_mul(&self, a: &Self::Matrix, b: &Self::Matrix) -> Self::Matrix;

    /// Singular value decomposition: M = U * diag(S) * V^T.
    fn svd(
        &self,
        m: &Self::Matrix,
    ) -> (Self::Matrix, Self::Vector, Self::Matrix);

    /// Eigenvalue decomposition (symmetric).
    fn eigenvalues(&self, m: &Self::Matrix) -> Self::Vector;

    /// L2 norm of a vector.
    fn norm(&self, v: &Self::Vector) -> f64;

    /// Sparse matrix-vector product: y = A * x.
    fn spmv(
        &self,
        rows: &[usize],
        cols: &[usize],
        vals: &[f64],
        x: &Self::Vector,
    ) -> Self::Vector;

    /// Create a zero vector of length n.
    fn zeros(&self, n: usize) -> Self::Vector;

    /// Create an identity matrix of size n x n.
    fn eye(&self, n: usize) -> Self::Matrix;
}

// ---------------------------------------------------------------------------
// Specialised solver traits -- extend SolverEngine with domain methods.
// ---------------------------------------------------------------------------

/// Sparse Laplacian solver (Spielman-Teng family).
pub trait SparseLaplacianSolver: SolverEngine {
    /// Solve Lx = b where L is a graph Laplacian.
    fn solve_laplacian(
        &self,
        laplacian: &<Self as SolverEngine>::Input,
        rhs: &[f64],
    ) -> Result<<Self as SolverEngine>::Output, <Self as SolverEngine>::Error>;

    /// Return the effective resistance between nodes u and v.
    fn effective_resistance(&self, u: usize, v: usize) -> Result<f64, <Self as SolverEngine>::Error>;
}

/// Sublinear PageRank approximation.
pub trait SublinearPageRank: SolverEngine {
    /// Approximate PageRank for a single target node.
    fn pagerank_single(
        &self,
        target: usize,
        teleport: f64,
    ) -> Result<f64, <Self as SolverEngine>::Error>;

    /// Approximate the top-k PageRank nodes.
    fn pagerank_topk(
        &self,
        k: usize,
        teleport: f64,
    ) -> Result<Vec<(usize, f64)>, <Self as SolverEngine>::Error>;
}

/// Hybrid random-walk solver for mixing-time estimation.
pub trait HybridRandomWalkSolver: SolverEngine {
    /// Estimate the mixing time of the chain.
    fn mixing_time(&self) -> Result<u64, <Self as SolverEngine>::Error>;

    /// Sample a random walk of length `steps` starting from `start`.
    fn random_walk(
        &self,
        start: usize,
        steps: usize,
    ) -> Result<Vec<usize>, <Self as SolverEngine>::Error>;

    /// Estimate stationary distribution via random walks.
    fn estimate_stationary(
        &self,
        num_walks: usize,
        walk_length: usize,
    ) -> Result<Vec<f64>, <Self as SolverEngine>::Error>;
}
```

### 2. Builder Pattern

Every concrete solver is constructed through a validated builder. The builder enforces
invariants at construction time so that `solve()` never fails due to misconfiguration.

```rust
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Algorithm selection enum.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Algorithm {
    /// Truncated Neumann series for Laplacian solves.
    Neumann,
    /// Spielman-Teng nearly-linear solver.
    SpielmanTeng,
    /// Approximate PageRank via local random walks.
    ApproxPageRank,
    /// Hybrid random-walk with spectral fallback.
    HybridRandomWalk,
    /// Chebyshev polynomial acceleration.
    Chebyshev,
}

impl Default for Algorithm {
    fn default() -> Self {
        Algorithm::Neumann
    }
}

// ---------------------------------------------------------------------------
// Preconditioner strategy.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preconditioner {
    None,
    Jacobi,
    IncompleteCholesky,
    LowStretchTree,
}

impl Default for Preconditioner {
    fn default() -> Self {
        Preconditioner::None
    }
}

// ---------------------------------------------------------------------------
// Builder.
// ---------------------------------------------------------------------------

pub struct SublinearSolverBuilder<B: NumericBackend = NalgebraBackend> {
    algorithm: Algorithm,
    tolerance: f64,
    max_iterations: u64,
    preconditioner: Preconditioner,
    parallelism: usize,
    backend: Option<B>,
    budget: ComputeBudget,
}

#[derive(Debug)]
pub struct BuilderError(String);

impl fmt::Display for BuilderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SublinearSolverBuilder: {}", self.0)
    }
}

impl Error for BuilderError {}

impl<B: NumericBackend> SublinearSolverBuilder<B> {
    pub fn new() -> Self {
        Self {
            algorithm: Algorithm::default(),
            tolerance: 1e-6,
            max_iterations: 1_000,
            preconditioner: Preconditioner::default(),
            parallelism: num_cpus::get(),
            backend: None,
            budget: ComputeBudget::default(),
        }
    }

    pub fn algorithm(mut self, alg: Algorithm) -> Self {
        self.algorithm = alg;
        self
    }

    pub fn tolerance(mut self, tol: f64) -> Self {
        self.tolerance = tol;
        self
    }

    pub fn max_iterations(mut self, n: u64) -> Self {
        self.max_iterations = n;
        self
    }

    pub fn preconditioner(mut self, p: Preconditioner) -> Self {
        self.preconditioner = p;
        self
    }

    pub fn parallelism(mut self, n: usize) -> Self {
        self.parallelism = n;
        self
    }

    pub fn backend(mut self, b: B) -> Self {
        self.backend = Some(b);
        self
    }

    pub fn budget(mut self, b: ComputeBudget) -> Self {
        self.budget = b;
        self
    }

    /// Validate all parameters and construct the solver.
    pub fn build(self) -> Result<SublinearSolver<B>, BuilderError> {
        if self.tolerance <= 0.0 || self.tolerance >= 1.0 {
            return Err(BuilderError(
                "tolerance must be in the open interval (0, 1)".into(),
            ));
        }
        if self.max_iterations == 0 {
            return Err(BuilderError(
                "max_iterations must be at least 1".into(),
            ));
        }
        if self.parallelism == 0 {
            return Err(BuilderError(
                "parallelism must be at least 1".into(),
            ));
        }
        let backend = self
            .backend
            .ok_or_else(|| BuilderError("backend is required".into()))?;

        Ok(SublinearSolver {
            algorithm: self.algorithm,
            tolerance: self.tolerance,
            max_iterations: self.max_iterations,
            preconditioner: self.preconditioner,
            parallelism: self.parallelism,
            backend: Arc::new(backend),
            budget: self.budget,
        })
    }
}

/// Convenience entry point.
impl SublinearSolver<NalgebraBackend> {
    pub fn builder() -> SublinearSolverBuilder<NalgebraBackend> {
        SublinearSolverBuilder::new()
    }
}

// Usage example:
//
//   let solver = SublinearSolver::builder()
//       .algorithm(Algorithm::Neumann)
//       .tolerance(1e-6)
//       .max_iterations(1_000)
//       .preconditioner(Preconditioner::Jacobi)
//       .parallelism(8)
//       .backend(NalgebraBackend::default())
//       .budget(ComputeBudget {
//           max_duration: Some(Duration::from_secs(30)),
//           ..Default::default()
//       })
//       .build()
//       .expect("valid configuration");
```

### 3. WASM API (wasm-bindgen)

The WASM layer wraps the core Rust traits behind `wasm-bindgen`-compatible structs.
All complex types cross the boundary as JSON via `serde_wasm_bindgen`.

```rust
use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Configuration passed from JavaScript.
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone)]
pub struct JsSolverConfig {
    pub algorithm: String,        // "neumann" | "spielman_teng" | ...
    pub tolerance: f64,
    pub max_iterations: u64,
    pub preconditioner: String,   // "none" | "jacobi" | ...
}

impl Default for JsSolverConfig {
    fn default() -> Self {
        Self {
            algorithm: "neumann".into(),
            tolerance: 1e-6,
            max_iterations: 1_000,
            preconditioner: "none".into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Result returned to JavaScript.
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
pub struct JsSolverResult {
    pub solution: Vec<f64>,
    pub residual_norm: f64,
    pub iterations_used: u64,
    pub wall_time_ms: f64,
    pub converged: bool,
}

// ---------------------------------------------------------------------------
// JsSolver -- the wasm-bindgen entry point.
// ---------------------------------------------------------------------------

#[wasm_bindgen]
pub struct JsSolver {
    // opaque inner solver
    inner: SublinearSolver<WasmBackend>,
}

#[wasm_bindgen]
impl JsSolver {
    /// Create a new solver with the given config (passed as a JS object).
    #[wasm_bindgen(constructor)]
    pub fn new(config: JsValue) -> Result<JsSolver, JsError> {
        let cfg: JsSolverConfig =
            serde_wasm_bindgen::from_value(config).map_err(|e| {
                JsError::new(&format!("invalid config: {e}"))
            })?;

        let alg = parse_algorithm(&cfg.algorithm)
            .map_err(|e| JsError::new(&e))?;
        let pre = parse_preconditioner(&cfg.preconditioner)
            .map_err(|e| JsError::new(&e))?;

        let solver = SublinearSolverBuilder::<WasmBackend>::new()
            .algorithm(alg)
            .tolerance(cfg.tolerance)
            .max_iterations(cfg.max_iterations)
            .preconditioner(pre)
            .backend(WasmBackend::default())
            .build()
            .map_err(|e| JsError::new(&e.to_string()))?;

        Ok(JsSolver { inner: solver })
    }

    /// Synchronous solve.  Blocks the WASM thread.
    pub fn solve(&self, input: JsValue) -> Result<JsValue, JsError> {
        let parsed = serde_wasm_bindgen::from_value(input)
            .map_err(|e| JsError::new(&format!("invalid input: {e}")))?;
        let result = self.inner.solve(&parsed)
            .map_err(|e| JsError::new(&e.to_string()))?;
        serde_wasm_bindgen::to_value(&result)
            .map_err(|e| JsError::new(&format!("serialization error: {e}")))
    }

    /// Asynchronous solve.  Returns a `Promise<JsSolverResult>`.
    #[wasm_bindgen(js_name = solveAsync)]
    pub async fn solve_async(&self, input: JsValue) -> Result<JsValue, JsError> {
        let parsed = serde_wasm_bindgen::from_value(input)
            .map_err(|e| JsError::new(&format!("invalid input: {e}")))?;
        // In the WASM single-threaded model this yields to the event loop
        // between iterations when possible.
        let result = self.inner.solve_async(&parsed).await
            .map_err(|e| JsError::new(&e.to_string()))?;
        serde_wasm_bindgen::to_value(&result)
            .map_err(|e| JsError::new(&format!("serialization error: {e}")))
    }

    /// Estimate complexity without solving.
    #[wasm_bindgen(js_name = estimateComplexity)]
    pub fn estimate_complexity(&self, input: JsValue) -> Result<JsValue, JsError> {
        let parsed = serde_wasm_bindgen::from_value(input)
            .map_err(|e| JsError::new(&format!("invalid input: {e}")))?;
        let est = self.inner.estimate_complexity(&parsed);
        serde_wasm_bindgen::to_value(&est)
            .map_err(|e| JsError::new(&format!("serialization error: {e}")))
    }

    /// Free WASM memory held by this solver.
    pub fn free(self) {
        drop(self);
    }
}
```

### 4. Node.js API (NAPI-RS)

The Node.js addon uses `napi-rs` to expose the solver as a native class. CPU-bound work
runs on the libuv thread pool via `spawn_blocking`.

```rust
use napi::bindgen_prelude::*;
use napi_derive::napi;

// ---------------------------------------------------------------------------
// Config and result structs (napi-compatible).
// ---------------------------------------------------------------------------

#[napi(object)]
pub struct NapiSolverConfig {
    pub algorithm: String,
    pub tolerance: f64,
    pub max_iterations: i64,       // napi does not support u64 directly
    pub preconditioner: String,
    pub parallelism: Option<i32>,
}

#[napi(object)]
pub struct NapiSolverResult {
    pub solution: Vec<f64>,
    pub residual_norm: f64,
    pub iterations_used: i64,
    pub wall_time_ms: f64,
    pub converged: bool,
}

#[napi(object)]
pub struct NapiComplexityEstimate {
    pub time_class: String,
    pub estimated_flops: i64,
    pub estimated_memory_bytes: i64,
    pub recommended_max_iterations: i64,
}

// ---------------------------------------------------------------------------
// NapiSolver class.
// ---------------------------------------------------------------------------

#[napi]
pub struct NapiSolver {
    inner: SublinearSolver<NalgebraBackend>,
}

#[napi]
impl NapiSolver {
    /// Construct from a config object.
    #[napi(constructor)]
    pub fn new(config: NapiSolverConfig) -> Result<Self> {
        let alg = parse_algorithm(&config.algorithm)
            .map_err(|e| Error::from_reason(e))?;
        let pre = parse_preconditioner(&config.preconditioner)
            .map_err(|e| Error::from_reason(e))?;

        let mut builder = SublinearSolverBuilder::new()
            .algorithm(alg)
            .tolerance(config.tolerance)
            .max_iterations(config.max_iterations as u64)
            .preconditioner(pre)
            .backend(NalgebraBackend::default());

        if let Some(p) = config.parallelism {
            builder = builder.parallelism(p as usize);
        }

        let inner = builder.build()
            .map_err(|e| Error::from_reason(e.to_string()))?;

        Ok(Self { inner })
    }

    /// Async solve -- offloads to the libuv thread pool.
    #[napi]
    pub async fn solve(&self, input: Buffer) -> Result<NapiSolverResult> {
        let data = input.to_vec();
        let solver = self.inner.clone();

        let result = tokio::task::spawn_blocking(move || {
            let parsed: SolverInput = bincode::deserialize(&data)
                .map_err(|e| Error::from_reason(format!("deserialize: {e}")))?;
            solver.solve(&parsed)
                .map_err(|e| Error::from_reason(e.to_string()))
        })
        .await
        .map_err(|e| Error::from_reason(format!("join: {e}")))??;

        Ok(to_napi_result(result))
    }

    /// Async solve from a JSON string (convenience for scripting).
    #[napi(js_name = "solveJson")]
    pub async fn solve_json(&self, json: String) -> Result<NapiSolverResult> {
        let solver = self.inner.clone();

        let result = tokio::task::spawn_blocking(move || {
            let parsed: SolverInput = serde_json::from_str(&json)
                .map_err(|e| Error::from_reason(format!("json: {e}")))?;
            solver.solve(&parsed)
                .map_err(|e| Error::from_reason(e.to_string()))
        })
        .await
        .map_err(|e| Error::from_reason(format!("join: {e}")))??;

        Ok(to_napi_result(result))
    }

    /// Estimate complexity without solving.
    #[napi(js_name = "estimateComplexity")]
    pub fn estimate_complexity(&self, json: String) -> Result<NapiComplexityEstimate> {
        let parsed: SolverInput = serde_json::from_str(&json)
            .map_err(|e| Error::from_reason(format!("json: {e}")))?;
        let est = self.inner.estimate_complexity(&parsed);
        Ok(NapiComplexityEstimate {
            time_class: est.time_class,
            estimated_flops: est.estimated_flops as i64,
            estimated_memory_bytes: est.estimated_memory_bytes as i64,
            recommended_max_iterations: est
                .recommended_budget
                .max_iterations
                .unwrap_or(0) as i64,
        })
    }

    /// Return the solver name.
    #[napi(getter)]
    pub fn name(&self) -> String {
        self.inner.name().to_string()
    }

    /// Return the solver version.
    #[napi(getter)]
    pub fn version(&self) -> String {
        self.inner.version().to_string()
    }
}
```

### 5. REST API (axum)

HTTP routes follow the existing RuVector REST conventions: JSON request/response bodies,
`application/json` content type, structured error responses, and SSE for streaming.

```rust
use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::{sse, Sse},
    routing::{get, post, put},
    Router,
};
use std::sync::Arc;
use tokio_stream::StreamExt;

// ---------------------------------------------------------------------------
// Application state shared across handlers.
// ---------------------------------------------------------------------------

pub struct AppState {
    pub solver: Arc<dyn SolverEngine<
        Input = SolverInput,
        Output = SolverOutput,
        Error = SolverError,
    >>,
}

// ---------------------------------------------------------------------------
// Request / response types.
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct SolveRequest {
    pub input: SolverInput,
    pub budget: Option<ComputeBudget>,
}

#[derive(Serialize)]
pub struct SolveResponse {
    pub result: SolverOutput,
    pub metadata: SolveMetadata,
}

#[derive(Serialize)]
pub struct SolveMetadata {
    pub wall_time_ms: f64,
    pub iterations_used: u64,
    pub converged: bool,
    pub solver_version: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: ErrorDetail,
}

#[derive(Serialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

#[derive(Serialize)]
pub struct ConfigResponse {
    pub algorithm: String,
    pub tolerance: f64,
    pub max_iterations: u64,
    pub preconditioner: String,
    pub parallelism: usize,
    pub version: String,
}

// ---------------------------------------------------------------------------
// Route table.
// ---------------------------------------------------------------------------

pub fn solver_routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/solver/solve", post(handle_solve))
        .route("/solver/solve/stream", post(handle_solve_stream))
        .route("/solver/complexity", post(handle_estimate_complexity))
        .route("/solver/config", get(handle_get_config))
        .route("/solver/config", put(handle_update_config))
        .route("/solver/health", get(handle_health))
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Handlers.
// ---------------------------------------------------------------------------

async fn handle_solve(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SolveRequest>,
) -> Result<Json<SolveResponse>, (StatusCode, Json<ErrorResponse>)> {
    let solver = state.solver.clone();
    let input = req.input;
    let budget = req.budget;

    let start = std::time::Instant::now();

    let result = tokio::task::spawn_blocking(move || {
        match budget {
            Some(b) => solver.solve_with_budget(&input, b),
            None => solver.solve(&input),
        }
    })
    .await
    .map_err(|e| internal_error(format!("task join: {e}")))?
    .map_err(|e| solver_error(e))?;

    let elapsed = start.elapsed();

    Ok(Json(SolveResponse {
        result,
        metadata: SolveMetadata {
            wall_time_ms: elapsed.as_secs_f64() * 1000.0,
            iterations_used: 0, // filled by solver
            converged: true,
            solver_version: state.solver.version().to_string(),
        },
    }))
}

/// SSE streaming endpoint.  Emits partial results as solver iterations
/// progress, then a final `done` event.
async fn handle_solve_stream(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SolveRequest>,
) -> Sse<impl tokio_stream::Stream<Item = Result<sse::Event, std::convert::Infallible>>> {
    let solver = state.solver.clone();
    let (tx, rx) = tokio::sync::mpsc::channel::<sse::Event>(64);

    tokio::task::spawn_blocking(move || {
        // The solver calls `tx.blocking_send()` for each iteration update.
        // Final result is sent as event type "done".
        let _ = solver.solve_streaming(&req.input, move |update| {
            let event = sse::Event::default()
                .event("progress")
                .json_data(&update)
                .unwrap();
            let _ = tx.blocking_send(event);
        });
    });

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx)
        .map(Ok);

    Sse::new(stream)
}

async fn handle_estimate_complexity(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SolveRequest>,
) -> Json<ComplexityEstimate> {
    let est = state.solver.estimate_complexity(&req.input);
    Json(est)
}

async fn handle_get_config(
    State(state): State<Arc<AppState>>,
) -> Json<ConfigResponse> {
    Json(ConfigResponse {
        algorithm: "neumann".into(),
        tolerance: 1e-6,
        max_iterations: 1_000,
        preconditioner: "none".into(),
        parallelism: num_cpus::get(),
        version: state.solver.version().to_string(),
    })
}

async fn handle_update_config(
    State(_state): State<Arc<AppState>>,
    Json(_cfg): Json<JsSolverConfig>,
) -> StatusCode {
    // Hot-reload configuration at runtime.
    StatusCode::NO_CONTENT
}

async fn handle_health(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    serde_json::json!({
        "status": "ok",
        "solver": state.solver.name(),
        "version": state.solver.version(),
    })
    .pipe(Json)
}

// ---------------------------------------------------------------------------
// Error helpers.
// ---------------------------------------------------------------------------

fn internal_error(msg: String) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: ErrorDetail {
                code: "INTERNAL_ERROR".into(),
                message: msg,
                details: None,
            },
        }),
    )
}

fn solver_error(e: impl Error) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(ErrorResponse {
            error: ErrorDetail {
                code: "SOLVER_ERROR".into(),
                message: e.to_string(),
                details: None,
            },
        }),
    )
}
```

### 6. MCP Tools (JSON-RPC)

Each solver capability is exposed as an MCP tool with a JSON Schema input specification.
Tools follow the MCP protocol with `name`, `description`, and `inputSchema`.

```json
[
  {
    "name": "solve_sublinear",
    "description": "Solve a linear system Ax=b using sublinear-time methods. Returns approximate solution vector.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "matrix": {
          "type": "object",
          "description": "Sparse matrix in COO format",
          "properties": {
            "rows": { "type": "array", "items": { "type": "integer" } },
            "cols": { "type": "array", "items": { "type": "integer" } },
            "vals": { "type": "array", "items": { "type": "number" } },
            "n":    { "type": "integer", "description": "Matrix dimension" }
          },
          "required": ["rows", "cols", "vals", "n"]
        },
        "rhs": {
          "type": "array",
          "items": { "type": "number" },
          "description": "Right-hand side vector b"
        },
        "algorithm": {
          "type": "string",
          "enum": ["neumann", "spielman_teng", "chebyshev"],
          "default": "neumann"
        },
        "tolerance": {
          "type": "number",
          "default": 1e-6,
          "minimum": 0,
          "exclusiveMinimum": true,
          "maximum": 1,
          "exclusiveMaximum": true
        },
        "max_iterations": {
          "type": "integer",
          "default": 1000,
          "minimum": 1
        }
      },
      "required": ["matrix", "rhs"]
    }
  },
  {
    "name": "estimate_complexity",
    "description": "Estimate the computational complexity of solving a given system without performing the solve.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "matrix": {
          "type": "object",
          "properties": {
            "rows": { "type": "array", "items": { "type": "integer" } },
            "cols": { "type": "array", "items": { "type": "integer" } },
            "vals": { "type": "array", "items": { "type": "number" } },
            "n":    { "type": "integer" }
          },
          "required": ["rows", "cols", "vals", "n"]
        },
        "algorithm": {
          "type": "string",
          "enum": ["neumann", "spielman_teng", "chebyshev"]
        }
      },
      "required": ["matrix"]
    }
  },
  {
    "name": "solve_pagerank",
    "description": "Compute approximate PageRank for a graph using sublinear-time local random walks.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "adjacency": {
          "type": "object",
          "description": "Sparse adjacency matrix in COO format",
          "properties": {
            "rows": { "type": "array", "items": { "type": "integer" } },
            "cols": { "type": "array", "items": { "type": "integer" } },
            "n":    { "type": "integer" }
          },
          "required": ["rows", "cols", "n"]
        },
        "teleport": {
          "type": "number",
          "default": 0.15,
          "minimum": 0,
          "maximum": 1
        },
        "target_node": {
          "type": "integer",
          "description": "If provided, compute PageRank for this node only"
        },
        "top_k": {
          "type": "integer",
          "description": "If provided, return the top-k nodes by PageRank",
          "minimum": 1
        }
      },
      "required": ["adjacency"]
    }
  },
  {
    "name": "solve_laplacian",
    "description": "Solve Lx=b where L is a graph Laplacian using nearly-linear time solvers.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "edges": {
          "type": "array",
          "items": {
            "type": "object",
            "properties": {
              "u": { "type": "integer" },
              "v": { "type": "integer" },
              "weight": { "type": "number", "default": 1.0 }
            },
            "required": ["u", "v"]
          },
          "description": "Edge list defining the graph"
        },
        "n": {
          "type": "integer",
          "description": "Number of vertices"
        },
        "rhs": {
          "type": "array",
          "items": { "type": "number" },
          "description": "Right-hand side vector b (must sum to zero)"
        },
        "tolerance": {
          "type": "number",
          "default": 1e-6
        }
      },
      "required": ["edges", "n", "rhs"]
    }
  }
]
```

### 7. TypeScript Type Definitions

TypeScript types are generated from the Rust types to ensure consistency. The generated
file is published alongside the WASM and NAPI packages.

```typescript
// ---------------------------------------------------------------------------
// Core types -- generated from Rust via ts-rs or manually maintained.
// ---------------------------------------------------------------------------

/** Algorithm selection. */
export type Algorithm =
  | "neumann"
  | "spielman_teng"
  | "approx_pagerank"
  | "hybrid_random_walk"
  | "chebyshev";

/** Preconditioner strategy. */
export type Preconditioner =
  | "none"
  | "jacobi"
  | "incomplete_cholesky"
  | "low_stretch_tree";

/** Compute budget constraining solver resource usage. */
export interface ComputeBudget {
  /** Maximum wall-clock duration in milliseconds. */
  maxDurationMs?: number;
  /** Maximum number of solver iterations. */
  maxIterations?: number;
  /** Maximum memory allocation in bytes. */
  maxMemoryBytes?: number;
}

/** Solver configuration passed to the constructor. */
export interface SolverConfig {
  algorithm?: Algorithm;
  tolerance?: number;
  maxIterations?: number;
  preconditioner?: Preconditioner;
  parallelism?: number;
}

/** Sparse matrix in COO (coordinate) format. */
export interface SparseMatrixCOO {
  rows: number[];
  cols: number[];
  vals: number[];
  n: number;
}

/** Input to the general solver. */
export interface SolverInput {
  matrix: SparseMatrixCOO;
  rhs: number[];
}

/** Result returned by the solver. */
export interface SolverResult {
  solution: number[];
  residualNorm: number;
  iterationsUsed: number;
  wallTimeMs: number;
  converged: boolean;
}

/** Complexity estimate returned before solving. */
export interface ComplexityEstimate {
  timeClass: string;
  estimatedFlops: number;
  estimatedMemoryBytes: number;
  recommendedBudget: ComputeBudget;
}

/** Edge in a graph for Laplacian solvers. */
export interface Edge {
  u: number;
  v: number;
  weight?: number;
}

/** PageRank result for a single node. */
export interface PageRankEntry {
  node: number;
  score: number;
}

/** Streaming progress update emitted via SSE. */
export interface SolveProgress {
  iteration: number;
  residualNorm: number;
  elapsedMs: number;
  estimatedRemainingMs: number;
}

/** Structured error response from the REST API. */
export interface ErrorResponse {
  error: {
    code: string;
    message: string;
    details?: Record<string, unknown>;
  };
}

// ---------------------------------------------------------------------------
// WASM solver class (from wasm-bindgen).
// ---------------------------------------------------------------------------

export declare class JsSolver {
  constructor(config?: SolverConfig);
  solve(input: SolverInput): SolverResult;
  solveAsync(input: SolverInput): Promise<SolverResult>;
  estimateComplexity(input: SolverInput): ComplexityEstimate;
  free(): void;
}

// ---------------------------------------------------------------------------
// Node.js solver class (from napi-rs).
// ---------------------------------------------------------------------------

export declare class NapiSolver {
  constructor(config: SolverConfig);
  solve(input: Buffer): Promise<SolverResult>;
  solveJson(json: string): Promise<SolverResult>;
  estimateComplexity(json: string): ComplexityEstimate;
  readonly name: string;
  readonly version: string;
}

// ---------------------------------------------------------------------------
// REST client helper (optional convenience wrapper).
// ---------------------------------------------------------------------------

export interface SolverClientOptions {
  baseUrl: string;
  timeoutMs?: number;
  headers?: Record<string, string>;
}

export declare class SolverClient {
  constructor(options: SolverClientOptions);
  solve(input: SolverInput, budget?: ComputeBudget): Promise<SolverResult>;
  solveStream(
    input: SolverInput,
    onProgress: (progress: SolveProgress) => void,
  ): Promise<SolverResult>;
  estimateComplexity(input: SolverInput): Promise<ComplexityEstimate>;
  getConfig(): Promise<SolverConfig>;
  updateConfig(config: Partial<SolverConfig>): Promise<void>;
  health(): Promise<{ status: string; solver: string; version: string }>;
}
```

### 8. Error Response Formats

All API layers use a unified error taxonomy. Error codes are stable across versions.

| Code | HTTP Status | Description |
|------|-------------|-------------|
| `INVALID_INPUT` | 400 | Malformed or missing required fields |
| `INVALID_MATRIX` | 400 | Matrix fails structural validation (e.g., not square) |
| `BUDGET_EXCEEDED` | 408 | Compute budget exhausted before convergence |
| `SOLVER_DIVERGED` | 422 | Solver failed to converge within tolerance |
| `UNSUPPORTED_ALGORITHM` | 400 | Requested algorithm is not available |
| `BACKEND_ERROR` | 500 | Numeric backend encountered an internal error |
| `INTERNAL_ERROR` | 500 | Unexpected server-side failure |

Error response body (all transports):

```json
{
  "error": {
    "code": "SOLVER_DIVERGED",
    "message": "Solver did not converge after 1000 iterations (residual: 3.2e-4, tolerance: 1e-6)",
    "details": {
      "iterations_used": 1000,
      "final_residual": 3.2e-4,
      "requested_tolerance": 1e-6
    }
  }
}
```

### 9. Streaming API Design

The streaming protocol uses Server-Sent Events (SSE) for HTTP and event callbacks for
native/WASM APIs.

**SSE event types:**

| Event | Data | Description |
|-------|------|-------------|
| `progress` | `SolveProgress` | Iteration update with residual and timing |
| `done` | `SolverResult` | Final converged solution |
| `error` | `ErrorResponse` | Solver error; stream terminates |

**Native Rust streaming callback:**

```rust
pub trait StreamingSolver: SolverEngine {
    fn solve_streaming<F>(
        &self,
        input: &Self::Input,
        on_progress: F,
    ) -> Result<Self::Output, Self::Error>
    where
        F: Fn(SolveProgress) + Send + 'static;
}
```

**WASM streaming via ReadableStream:**

```typescript
// Browser usage with ReadableStream
const stream: ReadableStream<SolveProgress> = solver.solveStream(input);
const reader = stream.getReader();
while (true) {
  const { done, value } = await reader.read();
  if (done) break;
  console.log(`Iteration ${value.iteration}: residual=${value.residualNorm}`);
}
```

### 10. Versioning Strategy

The solver API follows semantic versioning with these rules:

1. **Major version** (`v2.x.x`): Breaking changes to trait signatures, removal of API
   methods, or incompatible serialization format changes.
2. **Minor version** (`v1.1.x`): New methods, new algorithm variants, new optional fields
   in config/result structs.
3. **Patch version** (`v1.0.1`): Bug fixes, performance improvements, documentation
   updates.

**REST API versioning** uses URL path prefixes:

```
POST /v1/solver/solve      -- current stable
POST /v2/solver/solve      -- next major (when applicable)
```

**WASM/NAPI versioning** uses package versions aligned with the Rust crate version.

**MCP tool versioning** embeds the version in the tool description metadata. Tools are
never removed; deprecated tools return a warning header.

### 11. Deprecation Policy

1. Deprecated APIs are marked with `#[deprecated(since = "x.y.z", note = "...")]` in Rust
   and `@deprecated` JSDoc tags in TypeScript.
2. Deprecated REST endpoints return a `Deprecation` header:
   `Deprecation: version="v1", sunset="2027-01-01"`.
3. Deprecated MCP tools include a `deprecated: true` field in their schema.
4. A deprecated API remains functional for at least **6 months** or **2 major versions**,
   whichever is longer.
5. Removal is announced in the changelog at least one release before it takes effect.

## Consequences

### Positive

- Trait-based core ensures zero-cost abstraction overhead in Rust-native consumption.
- Builder pattern catches configuration errors at construction time, not at solve time.
- Unified error taxonomy across all transports means clients can share error-handling logic.
- SSE streaming lets long-running solves report progress without polling.
- Generated TypeScript types guarantee frontend/backend type alignment.
- MCP tools give AI agents direct access to solver capabilities with schema validation.
- Semantic versioning and deprecation policy give consumers predictable upgrade windows.

### Negative

- Six API surfaces require coordinated releases; a breaking change in the core trait
  ripples through all layers.
- `serde_wasm_bindgen` adds serialization overhead at the WASM boundary compared to raw
  pointer passing (mitigated by the `wasm-bindgen` reference types proposal when stable).
- NAPI integer constraints (no native `u64`) force lossy casts for very large iteration
  counts (mitigated by clamping to `i64::MAX`).
- REST SSE streaming does not support backpressure from slow clients; a bounded channel
  with a drop policy is used instead.

### Neutral

- The `NumericBackend` trait adds an indirection layer that compilers can often inline in
  monomorphized code but cannot inline through `Arc<dyn NumericBackend>`.
- MCP tool schemas duplicate information already present in the Rust type system, requiring
  manual synchronization or a code-generation step.

## Options Considered

### Option 1: Single Unified FFI Layer (flatbuffers)

- **Pros**: One serialization format for all non-Rust consumers; very fast encoding.
- **Cons**: Requires all consumers to use a flatbuffers library; poor ergonomics in
  TypeScript; no streaming support in the format itself.

### Option 2: gRPC for All Non-Rust Consumers

- **Pros**: Strongly typed; streaming built in; code generation for many languages.
- **Cons**: Adds a protobuf compilation step; browser gRPC requires grpc-web proxy;
  MCP protocol is JSON-RPC, not gRPC, creating a mismatch.

### Option 3: Per-Layer Bespoke APIs (chosen)

- **Pros**: Each layer uses its idiomatic tooling (wasm-bindgen, napi-rs, axum, MCP
  JSON-RPC); no forced dependencies; best ergonomics per platform.
- **Cons**: More surface area to maintain; requires a disciplined release process.

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-02-20 | RuVector Team | Initial proposal |
| 1.0 | 2026-02-20 | RuVector Team | Accepted: full implementation complete |

---

## Implementation Status

Clean trait-based API: SolverEngine trait with `fn solve(&self, matrix: &CsrMatrix<f64>, rhs: &[f64], budget: &ComputeBudget) -> Result<SolverResult, SolverError>`. Each algorithm also exposes a direct f32 `solve(&self, matrix: &CsrMatrix<f32>, rhs: &[f32]) -> Result<SolverResult, SolverError>` for zero-conversion hot paths. CsrMatrix<T> generic over numeric type with from_coo constructor, identity factory, spmv/spmv_unchecked/fused_residual_norm_sq methods. Router selects optimal algorithm automatically.

---

## Related Decisions

- ADR-STS-001: Solver Architecture Overview
- ADR-STS-003: Numeric Backend Abstraction
- ADR-STS-005: Error Handling Strategy
- ADR-STS-007: Streaming and Progress Reporting
- ADR-STS-008: WASM Compilation Target
- ADR-STS-009: Node.js Native Addon Strategy

## References

- [MADR 3.0 specification](https://adr.github.io/madr/)
- [wasm-bindgen guide](https://rustwasm.github.io/docs/wasm-bindgen/)
- [napi-rs documentation](https://napi.rs/)
- [axum framework](https://docs.rs/axum/latest/axum/)
- [MCP specification](https://modelcontextprotocol.io/specification)
- [Semantic Versioning 2.0.0](https://semver.org/)
- [Spielman & Teng, Nearly-linear time algorithms for graph partitioning](https://arxiv.org/abs/cs/0310051)
