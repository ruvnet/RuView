# Sublinear-Time Solver: DDD Integration Patterns

**Version**: 1.0
**Date**: 2026-02-20
**Status**: Proposed

---

## 1. Anti-Corruption Layers

Anti-Corruption Layers (ACLs) translate between the Solver Core bounded context and each consuming bounded context, preventing domain model leakage.

### 1.1 Solver-to-Coherence ACL

Translates between Prime Radiant's sheaf graph types and the solver's sparse matrix types.

```rust
/// ACL: Coherence Engine ←→ Solver Core
pub struct CoherenceSolverAdapter {
    solver: Arc<dyn SparseLaplacianSolver>,
    cache: DashMap<u64, SolverResult>, // Keyed on graph version hash
}

impl CoherenceSolverAdapter {
    /// Convert SheafGraph to CsrMatrix for solver input
    pub fn sheaf_to_csr(graph: &SheafGraph) -> CsrMatrix<f32> {
        let n = graph.node_count();
        let mut row_ptrs = Vec::with_capacity(n + 1);
        let mut col_indices = Vec::new();
        let mut values = Vec::new();

        row_ptrs.push(0u32);
        for node_id in 0..n {
            let edges = graph.edges_from(node_id);
            let degree: f32 = edges.iter().map(|e| e.weight).sum();

            // Laplacian: L = D - A
            // Add diagonal (degree)
            col_indices.push(node_id as u32);
            values.push(degree);

            // Add off-diagonal (-weight)
            for edge in &edges {
                col_indices.push(edge.target as u32);
                values.push(-edge.weight);
            }
            row_ptrs.push(col_indices.len() as u32);
        }

        CsrMatrix { values: values.into(), col_indices: col_indices.into(), row_ptrs, rows: n, cols: n }
    }

    /// Convert solver result back to coherence energy
    pub fn solution_to_energy(
        solution: &SolverResult,
        graph: &SheafGraph,
    ) -> CoherenceEnergy {
        // Residual vector r = L*x represents per-edge contradiction
        let residual_norm = solution.convergence.final_residual;

        // Energy = sum of squared edge residuals
        let energy = residual_norm * residual_norm;

        // Per-node energy distribution
        let node_energies: Vec<f64> = solution.solution.iter()
            .map(|&x| (x as f64) * (x as f64))
            .collect();

        CoherenceEnergy {
            global_energy: energy,
            node_energies,
            solver_algorithm: solution.algorithm_used,
            solver_iterations: solution.iterations,
            accuracy_bound: solution.error_bounds.relative_error,
        }
    }

    /// Cached solve: reuse result if graph hasn't changed
    pub async fn solve_coherence(
        &self,
        graph: &SheafGraph,
        signal: &[f32],
    ) -> Result<CoherenceEnergy, SolverError> {
        let graph_hash = graph.content_hash();

        if let Some(cached) = self.cache.get(&graph_hash) {
            return Ok(Self::solution_to_energy(&cached, graph));
        }

        let csr = Self::sheaf_to_csr(graph);
        let system = SparseSystem::new(csr, signal.to_vec());
        let result = self.solver.solve(&system)?;

        self.cache.insert(graph_hash, result.clone());
        Ok(Self::solution_to_energy(&result, graph))
    }
}
```

### 1.2 Solver-to-GNN ACL

Translates between GNN message passing and sparse system solves.

```rust
/// ACL: GNN ←→ Solver Core
pub struct GnnSolverAdapter {
    solver: Arc<dyn SolverEngine>,
}

impl GnnSolverAdapter {
    /// Sublinear message aggregation using sparse solver
    /// Replaces: O(n × avg_degree) per layer
    /// With: O(nnz × log(1/ε)) per layer
    pub fn sublinear_aggregate(
        &self,
        adjacency: &CsrMatrix<f32>,
        features: &[Vec<f32>],
        epsilon: f64,
    ) -> Result<Vec<Vec<f32>>, SolverError> {
        let n = adjacency.rows;
        let feature_dim = features[0].len();
        let mut aggregated = vec![vec![0.0f32; feature_dim]; n];

        // Solve A·X_col = F_col for each feature dimension
        // Using batch solver amortization
        for d in 0..feature_dim {
            let rhs: Vec<f32> = features.iter().map(|f| f[d]).collect();
            let system = SparseSystem::new(adjacency.clone(), rhs);
            let result = self.solver.solve_with_budget(
                &system,
                ComputeBudget::for_lane(ComputeLane::Heavy),
            )?;

            for i in 0..n {
                aggregated[i][d] = result.solution[i];
            }
        }

        Ok(aggregated)
    }
}

/// GNN aggregation strategy using solver
pub struct SublinearAggregation {
    adapter: GnnSolverAdapter,
    epsilon: f64,
}

impl AggregationStrategy for SublinearAggregation {
    fn aggregate(
        &self,
        adjacency: &CsrMatrix<f32>,
        features: &[Vec<f32>],
    ) -> Vec<Vec<f32>> {
        self.adapter.sublinear_aggregate(adjacency, features, self.epsilon)
            .unwrap_or_else(|_| {
                // Fallback to mean aggregation
                MeanAggregation.aggregate(adjacency, features)
            })
    }
}
```

### 1.3 Solver-to-Graph ACL

Translates between ruvector-graph's property graph model and solver's sparse adjacency.

```rust
/// ACL: Graph Analytics ←→ Solver Core
pub struct GraphSolverAdapter {
    push_solver: Arc<dyn SublinearPageRank>,
}

impl GraphSolverAdapter {
    /// Convert PropertyGraph to SparseAdjacency for solver
    pub fn property_graph_to_adjacency(graph: &PropertyGraph) -> SparseAdjacency {
        let n = graph.node_count();
        let edges: Vec<(usize, usize, f32)> = graph.edges()
            .map(|e| (e.source, e.target, e.weight.unwrap_or(1.0)))
            .collect();

        SparseAdjacency {
            adj: CsrMatrix::from_edges(&edges, n),
            directed: graph.is_directed(),
            weighted: graph.is_weighted(),
        }
    }

    /// Solver-accelerated PageRank using Forward Push
    /// Replaces: O(n × m × iterations) power iteration
    /// With: O(1/ε) Forward Push
    pub fn fast_pagerank(
        &self,
        graph: &PropertyGraph,
        source: usize,
        alpha: f64,
        epsilon: f64,
    ) -> Result<Vec<(usize, f64)>, SolverError> {
        let adj = Self::property_graph_to_adjacency(graph);
        let problem = GraphProblem {
            id: ProblemId::new(),
            graph: adj,
            query: GraphQuery::SingleSource { source },
            parameters: PushParameters { alpha, epsilon, max_iterations: 1_000_000 },
        };

        let result = self.push_solver.solve(&problem)?;

        // Convert solver output to ranked node list
        let mut ranked: Vec<(usize, f64)> = result.solution.iter()
            .enumerate()
            .map(|(i, &score)| (i, score as f64))
            .filter(|(_, score)| *score > epsilon)
            .collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        Ok(ranked)
    }
}
```

### 1.4 Platform ACL (WASM / NAPI / REST / MCP)

Serialization boundary between domain types and platform representations.

```rust
/// WASM ACL
#[wasm_bindgen]
pub struct JsSolverConfig {
    inner: SolverConfig,
}

#[wasm_bindgen]
impl JsSolverConfig {
    #[wasm_bindgen(constructor)]
    pub fn new(js_config: JsValue) -> Result<JsSolverConfig, JsValue> {
        let config: SolverConfig = serde_wasm_bindgen::from_value(js_config)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(JsSolverConfig { inner: config })
    }
}

/// REST ACL
pub async fn solve_handler(
    State(state): State<AppState>,
    Json(request): Json<SolverRequest>,
) -> Result<Json<SolverResponse>, AppError> {
    // Translate REST types to domain types
    let system = SparseSystem::from_request(&request)?;
    let budget = ComputeBudget::from_request(&request);

    // Execute domain logic
    let result = state.orchestrator.solve(system).await?;

    // Translate domain result to REST response
    Ok(Json(SolverResponse::from_result(&result)))
}

/// MCP ACL
pub fn solver_tool_schema() -> McpTool {
    McpTool {
        name: "solve_sublinear".to_string(),
        description: "Solve sparse linear system using sublinear algorithms".to_string(),
        input_schema: json!({
            "type": "object",
            "required": ["matrix_rows", "matrix_cols", "values", "col_indices", "row_ptrs", "rhs"],
            "properties": {
                "matrix_rows": { "type": "integer", "minimum": 1 },
                "matrix_cols": { "type": "integer", "minimum": 1 },
                "values": { "type": "array", "items": { "type": "number" } },
                "col_indices": { "type": "array", "items": { "type": "integer" } },
                "row_ptrs": { "type": "array", "items": { "type": "integer" } },
                "rhs": { "type": "array", "items": { "type": "number" } },
                "tolerance": { "type": "number", "default": 1e-6 },
                "max_iterations": { "type": "integer", "default": 1000 },
                "algorithm": { "type": "string", "enum": ["auto", "neumann", "cg", "true"] },
            }
        }),
    }
}
```

---

## 2. Shared Kernel

Types shared between Solver Core and other bounded contexts.

### 2.1 Sparse Matrix Types

Shared between Solver Core and Min-Cut Context:

```rust
// crates/ruvector-solver/src/shared/sparse.rs
// Also used by ruvector-mincut

pub use crate::domain::values::CsrMatrix;
pub use crate::domain::values::SparsityProfile;

/// Conversion between CsrMatrix and CscMatrix (Compressed Sparse Column)
impl<T: Copy> CsrMatrix<T> {
    pub fn to_csc(&self) -> CscMatrix<T> { ... }
    pub fn transpose(&self) -> CsrMatrix<T> { ... }
}
```

### 2.2 Error Types

Shared across all solver-related contexts:

```rust
// crates/ruvector-solver/src/shared/errors.rs

#[derive(Debug, thiserror::Error)]
pub enum SolverError {
    #[error("solver did not converge: {iterations} iterations, best residual {best_residual}")]
    NonConvergence { iterations: usize, best_residual: f64, budget: ComputeBudget },

    #[error("numerical instability in {source}: {detail}")]
    NumericalInstability { source: &'static str, detail: String },

    #[error("compute budget exhausted: {progress:.1}% complete")]
    BudgetExhausted { budget: ComputeBudget, progress: f64 },

    #[error("invalid input: {0}")]
    InvalidInput(#[from] ValidationError),

    #[error("precision loss: expected ε={expected_eps}, achieved ε={achieved_eps}")]
    PrecisionLoss { expected_eps: f64, achieved_eps: f64 },

    #[error("all algorithms failed")]
    AllAlgorithmsFailed,

    #[error("backend error: {0}")]
    BackendError(#[from] Box<dyn std::error::Error + Send + Sync>),
}
```

### 2.3 Compute Budget

Shared between Solver and Coherence Gate's compute ladder:

```rust
// Used by both ruvector-solver and cognitum-gate-tilezero
pub use crate::domain::entities::ComputeBudget;
pub use crate::domain::entities::ComputeLane;
```

---

## 3. Published Language

### 3.1 Solver Protocol (JSON Schema)

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://ruvector.io/schemas/solver/v1",
  "title": "RuVector Sublinear Solver Protocol v1",
  "definitions": {
    "SolverRequest": {
      "type": "object",
      "required": ["system"],
      "properties": {
        "system": { "$ref": "#/definitions/SparseSystem" },
        "config": { "$ref": "#/definitions/SolverConfig" },
        "budget": { "$ref": "#/definitions/ComputeBudget" }
      }
    },
    "SparseSystem": {
      "type": "object",
      "required": ["rows", "cols", "values", "col_indices", "row_ptrs", "rhs"],
      "properties": {
        "rows": { "type": "integer", "minimum": 1, "maximum": 10000000 },
        "cols": { "type": "integer", "minimum": 1, "maximum": 10000000 },
        "values": { "type": "array", "items": { "type": "number" } },
        "col_indices": { "type": "array", "items": { "type": "integer", "minimum": 0 } },
        "row_ptrs": { "type": "array", "items": { "type": "integer", "minimum": 0 } },
        "rhs": { "type": "array", "items": { "type": "number" } }
      }
    },
    "SolverResult": {
      "type": "object",
      "properties": {
        "solution": { "type": "array", "items": { "type": "number" } },
        "algorithm_used": { "type": "string" },
        "iterations": { "type": "integer" },
        "residual_norm": { "type": "number" },
        "wall_time_us": { "type": "integer" },
        "converged": { "type": "boolean" },
        "error_bounds": {
          "type": "object",
          "properties": {
            "absolute_error": { "type": "number" },
            "relative_error": { "type": "number" }
          }
        }
      }
    }
  }
}
```

---

## 4. Event-Driven Integration

### 4.1 Event Flow Architecture

```
    SolverOrchestrator
           │
    emits SolverEvent
           │
    ┌──────┴──────┐
    │ broadcast    │
    │ ::Sender     │
    └──────┬──────┘
           │
     ┌─────┼─────┬──────────┬──────────┐
     ▼     ▼     ▼          ▼          ▼
 Coherence Metrics  Stream   Audit   SONA
 Engine   Collector  API     Trail   Learning
     │       │       │        │        │
     ▼       ▼       ▼        ▼        ▼
 Update  Prometheus Server-  Witness  Update
 energy  counters   Sent     chain   routing
                    Events   entry   weights
```

### 4.2 Coherence Gate as Solver Governor

```
Solve Request
     │
     ▼
┌────────────────┐
│ Complexity Est.│  "How expensive will this be?"
└───────┬────────┘
        │
        ▼
┌────────────────┐
│ Gate Decision  │  Permit / Defer / Deny
└───┬────┬───┬───┘
    │    │   │
 Permit Defer Deny
    │    │   │
    ▼    ▼   ▼
Execute Wait Reject
 solver  for  with
         human witness
         approval
```

### 4.3 SONA Feedback Loop

```
[Solve Request] → [Route] → [Execute] → [Record Result]
                     ▲                         │
                     │                         ▼
               [Update Routing]     [SONA micro-LoRA update]
               [Weights]                       │
                     ▲                         │
                     └─── EWC-protected ──────┘
                          weight update
```

---

## 5. Dependency Injection

### 5.1 Generic Type Parameters

```rust
/// Solver generic over numeric backend
pub struct SublinearSolver<B: NumericBackend = NalgebraBackend> {
    backend: B,
    config: SolverConfig,
}

impl<B: NumericBackend> SolverEngine for SublinearSolver<B> {
    type Input = SparseSystem;
    type Output = SolverResult;
    type Error = SolverError;

    fn solve(&self, input: &Self::Input) -> Result<Self::Output, Self::Error> {
        // Implementation using self.backend for matrix operations
        todo!()
    }
}
```

### 5.2 Runtime DI via Arc<dyn Trait>

```rust
/// Application state with DI
pub struct AppState {
    pub solver: Arc<dyn SolverEngine<Input = SparseSystem, Output = SolverResult, Error = SolverError>>,
    pub router: Arc<AlgorithmRouter>,
    pub session_repo: Arc<dyn SolverSessionRepository>,
    pub event_bus: broadcast::Sender<SolverEvent>,
}
```

---

## 6. Integration with Existing Patterns

### 6.1 Core-Binding-Surface Compliance

```
ruvector-solver           → Core (pure Rust algorithms)
ruvector-solver-wasm      → Binding (wasm-bindgen)
ruvector-solver-node      → Binding (NAPI-RS)
@ruvector/solver (npm)    → Surface (TypeScript API)
```

### 6.2 Event Sourcing Alignment

SolverEvent matches Prime Radiant's DomainEvent contract:
- `#[serde(tag = "type")]` — Discriminated union in JSON
- Deterministic replay via event log
- Content-addressable via SHAKE-256 hash
- Tamper-detectable in witness chain

### 6.3 Compute Ladder Integration

Solver maps to cognitum-gate-tilezero compute lanes:

| Lane | Solver Use Case | Budget |
|------|----------------|--------|
| Reflex | Cached result lookup | <1ms, 1MB |
| Retrieval | Small solve (n<1K) or Push query | ~10ms, 16MB |
| Heavy | Full CG/Neumann/BMSSP solve | ~100ms, 256MB |
| Deliberate | TRUE with preprocessing, streaming | Unbounded |

---

## 7. Migration Patterns

### 7.1 Strangler Fig for Coherence Engine

Gradual replacement of dense Laplacian computation:

```rust
impl CoherenceComputer {
    pub fn compute_energy(&self, graph: &SheafGraph) -> CoherenceEnergy {
        let density = graph.edge_density();

        #[cfg(feature = "sublinear-coherence")]
        if density < 0.05 {
            // New: Sublinear path for sparse graphs
            if let Ok(energy) = self.solver_adapter.solve_coherence(graph, &signal) {
                return energy;
            }
            // Fallthrough to dense on solver failure
        }

        // Existing: Dense path (unchanged)
        self.dense_laplacian_energy(graph)
    }
}
```

Phase 1: Feature flag (opt-in, default off)
Phase 2: Default on for sparse graphs (density < 5%)
Phase 3: Default on for all graphs after benchmark validation
Phase 4: Remove dense path (breaking change in major version)

### 7.2 Branch by Abstraction for GNN

```rust
pub enum AggregationStrategy {
    Mean,
    Max,
    Sum,
    Attention,
    #[cfg(feature = "sublinear-gnn")]
    Sublinear { epsilon: f64 },
}

impl GnnLayer {
    pub fn aggregate(&self, adj: &CsrMatrix<f32>, features: &[Vec<f32>]) -> Vec<Vec<f32>> {
        match self.strategy {
            AggregationStrategy::Mean => mean_aggregate(adj, features),
            AggregationStrategy::Max => max_aggregate(adj, features),
            AggregationStrategy::Sum => sum_aggregate(adj, features),
            AggregationStrategy::Attention => attention_aggregate(adj, features),
            #[cfg(feature = "sublinear-gnn")]
            AggregationStrategy::Sublinear { epsilon } => {
                SublinearAggregation::new(epsilon).aggregate(adj, features)
            }
        }
    }
}
```

---

## 8. Cross-Cutting Concerns

### 8.1 Observability

```rust
use tracing::{instrument, info, warn};

impl SolverOrchestrator {
    #[instrument(skip(self, system), fields(n = system.matrix.rows, nnz = system.matrix.nnz()))]
    pub async fn solve(&self, system: SparseSystem) -> Result<SolverResult, SolverError> {
        let algorithm = self.router.select(&system.profile());
        info!(algorithm = ?algorithm, "routing decision");

        let start = Instant::now();
        let result = self.execute(algorithm, &system).await;
        let elapsed = start.elapsed();

        match &result {
            Ok(r) => info!(iterations = r.iterations, residual = r.residual_norm, elapsed_us = elapsed.as_micros() as u64, "solve completed"),
            Err(e) => warn!(error = %e, "solve failed"),
        }

        result
    }
}
```

### 8.2 Caching

```rust
pub struct SolverCache {
    results: DashMap<u64, (SolverResult, Instant)>,
    ttl: Duration,
    max_entries: usize,
}

impl SolverCache {
    pub fn get_or_compute(
        &self,
        key: u64,
        compute: impl FnOnce() -> Result<SolverResult, SolverError>,
    ) -> Result<SolverResult, SolverError> {
        if let Some(entry) = self.results.get(&key) {
            if entry.1.elapsed() < self.ttl {
                return Ok(entry.0.clone());
            }
        }

        let result = compute()?;
        self.results.insert(key, (result.clone(), Instant::now()));

        // Evict if over capacity
        if self.results.len() > self.max_entries {
            self.evict_oldest();
        }

        Ok(result)
    }
}
```
