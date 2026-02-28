# Sublinear-Time Solver: DDD Strategic Design

**Version**: 1.0
**Date**: 2026-02-20
**Status**: Proposed

---

## 1. Domain Vision Statement

The **Sublinear Solver Domain** provides O(log n) to O(√n) mathematical computation capabilities that transform RuVector's polynomial-time bottlenecks into sublinear-time operations. By replacing dense O(n²-n³) linear algebra with sparse-aware solvers, we enable real-time performance at 100K+ node scales across the coherence engine, GNN, spectral methods, and graph analytics — delivering 10-600x speedups while maintaining configurable accuracy guarantees.

> **Core insight**: The same mathematical object (sparse linear system) appears in coherence computation, GNN message passing, spectral filtering, PageRank, and optimal transport. One solver serves them all.

---

## 2. Bounded Contexts

### 2.1 Solver Core Context

**Responsibility**: Pure mathematical algorithm implementations — Neumann series, Forward/Backward Push, Hybrid Random Walk, TRUE, Conjugate Gradient, BMSSP.

**Ubiquitous Language**:
- *Sparse system*: Ax = b where A has nnz << n² nonzeros
- *Convergence*: Residual norm ||Ax - b|| < ε
- *Neumann iteration*: x = Σ(I-A)^k · b
- *Push operation*: Redistribute probability mass along graph edges
- *Sparsification*: Reduce edge count while preserving spectral properties
- *Condition number*: κ(A) = λ_max / λ_min (drives CG convergence rate)
- *Diagonal dominance*: |a_ii| ≥ Σ|a_ij| for all rows

**Crate**: `ruvector-solver`

**Key Types**:
```rust
// Core domain model
pub struct CsrMatrix<T> { values, col_indices, row_ptrs, rows, cols }
pub struct SolverResult { solution, convergence_info, audit_entry }
pub struct ComputeBudget { max_wall_time, max_iterations, max_memory_bytes, lane }
pub enum Algorithm { Neumann, ForwardPush, BackwardPush, HybridRandomWalk, TRUE, CG, BMSSP }
```

### 2.2 Algorithm Routing Context

**Responsibility**: Selecting the optimal algorithm for each problem based on matrix properties, platform constraints, and learned performance history.

**Ubiquitous Language**:
- *Routing decision*: Map (problem profile) → Algorithm
- *Sparsity threshold*: Density below which sublinear methods outperform dense
- *Crossover point*: Problem size n where algorithm A becomes faster than B
- *Adaptive weight*: SONA-learned routing confidence per algorithm
- *Compute lane*: Reflex (<1ms) / Retrieval (~10ms) / Heavy (~100ms) / Deliberate (unbounded)

**Crate**: `ruvector-solver` (routing module)

### 2.3 Solver Platform Context

**Responsibility**: Platform-specific bindings that translate between domain types and platform-specific representations.

**Ubiquitous Language**:
- *JsSolver*: WASM-bindgen wrapper exposing solver to JavaScript
- *NapiSolver*: NAPI-RS wrapper for Node.js
- *Solver endpoint*: REST route for HTTP-based solving
- *Solver tool*: MCP JSON-RPC tool for AI agent access

**Crates**: `ruvector-solver-wasm`, `ruvector-solver-node`

### 2.4 Consuming Contexts (Existing RuVector Domains)

#### Coherence Context (prime-radiant)
- Consumes: SparseLaplacianSolver trait
- Translates: SheafGraph → CsrMatrix → CoherenceEnergy
- Integration: ACL adapter converts sheaf types to solver types

#### Learning Context (ruvector-gnn, sona)
- Consumes: SolverEngine for sublinear message aggregation
- Translates: Adjacency + Features → Sparse system → Aggregated features
- Integration: SublinearAggregation strategy alongside Mean/Max/Sum

#### Graph Analytics Context (ruvector-graph)
- Consumes: ForwardPush, BackwardPush for PageRank/centrality
- Translates: PropertyGraph → SparseAdjacency → PPR scores
- Integration: Published Language (shared sparse matrix format)

#### Spectral Context (ruvector-math)
- Consumes: Neumann, CG for spectral filtering
- Translates: Filter polynomial → Sparse system → Filtered signal
- Integration: NeumannFilter replaces ChevyshevFilter for rational approximation

#### Attention Context (ruvector-attention)
- Consumes: CG for PDE-based attention diffusion
- Translates: Attention matrix → Sparse Laplacian → Diffused attention
- Integration: PDEAttention mechanism using solver backend

#### Min-Cut Context (ruvector-mincut)
- Consumes: TRUE (shared sparsifier infrastructure)
- Translates: Graph → Sparsified graph → Effective resistances
- Integration: Partnership — co-evolving sparsification code

---

## 3. Context Map

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    SUBLINEAR SOLVER UNIVERSE                            │
│                                                                         │
│  ┌──────────────────┐     ┌──────────────────┐                         │
│  │  ALGORITHM        │     │  SOLVER CORE      │                        │
│  │  ROUTING          │────▶│                    │                        │
│  │                   │ CS  │  Neumann, CG,      │                        │
│  │  Tier1/2/3 select │     │  Push, TRUE, BMSSP │                        │
│  └──────────────────┘     └────────┬───────────┘                        │
│                                     │                                    │
│                          ┌──────────┴──────────┐                        │
│                          │  SOLVER PLATFORM     │                        │
│                          │                      │                        │
│                          │  WASM│NAPI│REST│MCP  │                        │
│                          └──────────┬───────────┘                        │
│                                     │ ACL                                │
└─────────────────────────────────────┼───────────────────────────────────┘
                                      │
                     ┌────────────────┼────────────────────┐
                     │                │                     │
              ┌──────▼──────┐  ┌──────▼──────┐  ┌──────────▼─────┐
              │ COHERENCE    │  │ LEARNING     │  │ GRAPH           │
              │ (prime-rad.) │  │ (gnn, sona)  │  │ ANALYTICS       │
              │              │  │              │  │                  │
              │ Conformist   │  │ OHS          │  │ Published Lang.  │
              └──────────────┘  └──────────────┘  └──────────────────┘
                     │                │                     │
              ┌──────▼──────┐  ┌──────▼──────┐  ┌──────────▼─────┐
              │ SPECTRAL     │  │ ATTENTION    │  │ MIN-CUT          │
              │ (math)       │  │              │  │ (mincut)         │
              │              │  │              │  │                  │
              │ Shared Kernel│  │ OHS          │  │ Partnership      │
              └──────────────┘  └──────────────┘  └──────────────────┘
```

### Relationship Types

| From | To | Pattern | Description |
|------|-----|---------|-------------|
| Routing → Core | **Customer-Supplier** | Routing decides, Core executes |
| Platform → Core | **Anti-Corruption Layer** | Serialization boundary |
| Core → Coherence | **Conformist** | Solver adapts to coherence's trait interfaces |
| Core → GNN | **Open Host Service** | Solver exposes SolverEngine trait |
| Core → Graph | **Published Language** | Shared CsrMatrix format |
| Core → Spectral | **Shared Kernel** | Common matrix types, error types |
| Core → Min-Cut | **Partnership** | Co-evolving sparsification code |
| Core → Attention | **Open Host Service** | Solver exposes CG backend |

---

## 4. Strategic Classification

| Context | Type | Priority | Competitive Advantage |
|---------|------|----------|----------------------|
| **Solver Core** | Core Domain | P0 | Unique O(log n) solving — no competitor offers this |
| **Algorithm Routing** | Core Domain | P0 | Intelligent auto-selection differentiates from manual tuning |
| **Solver Platform** | Supporting | P1 | Multi-platform deployment (WASM/NAPI/REST/MCP) |
| **Integration Adapters** | Supporting | P1 | Seamless adoption by existing subsystems |
| **Coherence Integration** | Core | P0 | Primary use case: 50-600x coherence speedup |
| **GNN Integration** | Core | P1 | 10-50x message passing speedup |
| **Graph Integration** | Supporting | P1 | O(1/ε) PageRank, new capability |
| **Spectral Integration** | Supporting | P2 | 20-100x spectral filtering |

---

## 5. Subdomains

### 5.1 Core Subdomains (Build In-House)

- **Sparse Linear Algebra**: Neumann, CG, BMSSP implementations optimized for RuVector's workloads
- **Graph Proximity**: Forward/Backward Push, Hybrid Random Walk for PPR computation
- **Dimensionality Reduction**: JL projection and spectral sparsification (TRUE pipeline)

### 5.2 Supporting Subdomains (Build Lean)

- **Numerical Stability**: Regularization, Kahan summation, reorthogonalization, mass invariant monitoring
- **Compute Budget Management**: Resource allocation, deadline enforcement, memory tracking
- **Platform Adaptation**: WASM/NAPI/REST serialization, type conversion, Worker pools

### 5.3 Generic Subdomains (Buy/Reuse)

- **Configuration Management**: Reuse `serde` + feature flags (existing pattern)
- **Logging and Metrics**: Reuse `tracing` ecosystem (existing pattern)
- **Error Handling**: Follow existing `thiserror` pattern
- **Benchmarking**: Reuse Criterion.rs infrastructure

---

## 6. Ubiquitous Language Glossary

### Solver Core Terms

| Term | Definition |
|------|-----------|
| **CsrMatrix** | Compressed Sparse Row format: three arrays (values, col_indices, row_ptrs) representing a sparse matrix |
| **SpMV** | Sparse Matrix-Vector multiply: y = A·x where A is CSR |
| **Neumann Series** | x = Σ_{k=0}^{K} (I-A)^k · b — converges when ρ(I-A) < 1 |
| **Forward Push** | Redistribute positive residual mass to neighbors in graph |
| **PPR** | Personalized PageRank: random-walk-based node relevance |
| **TRUE** | Toolbox for Research on Universal Estimation: JL + sparsify + Neumann |
| **CG** | Conjugate Gradient: iterative Krylov solver for SPD systems |
| **BMSSP** | Bounded Min-Cut Sparse Solver Paradigm: multigrid V-cycle solver |
| **Spectral Radius** | ρ(A) = max eigenvalue magnitude; ρ(I-A) < 1 required for Neumann |
| **Condition Number** | κ(A) = λ_max/λ_min; CG converges in O(√κ) iterations |
| **Diagonal Dominance** | |a_ii| ≥ Σ_{j≠i} |a_ij|; ensures Neumann convergence |
| **Sparsifier** | Reweighted subgraph preserving spectral properties within (1±ε) |
| **JL Projection** | Johnson-Lindenstrauss random projection reducing dimensionality |

### Integration Terms

| Term | Definition |
|------|-----------|
| **Compute Lane** | Execution tier: Reflex (<1ms), Retrieval (~10ms), Heavy (~100ms), Deliberate (unbounded) |
| **Solver Event** | Domain event emitted during/after solve (SolveRequested, IterationCompleted, etc.) |
| **Witness Entry** | SHAKE-256 hash chain entry in audit trail |
| **PermitToken** | Authorization token from MCP coherence gate |
| **Coherence Energy** | Scalar measure of system contradiction from sheaf Laplacian residuals |
| **Fallback Chain** | Ordered algorithm cascade: sublinear → CG → dense |
| **Error Budget** | ε_total decomposed across pipeline stages |

### Platform Terms

| Term | Definition |
|------|-----------|
| **Core-Binding-Surface** | Three-crate pattern: pure Rust core → WASM/NAPI binding → npm surface |
| **JsSolver** | wasm-bindgen struct exposing solver to browser JavaScript |
| **NapiSolver** | NAPI-RS struct exposing solver to Node.js |
| **Worker Pool** | Web Worker collection for browser parallelism |
| **SharedArrayBuffer** | Browser shared memory for zero-copy inter-worker data |

---

## 7. Domain Events (Cross-Context)

| Event | Producer | Consumers | Payload |
|-------|----------|-----------|---------|
| `SolveRequested` | Solver Core | Metrics, Audit | request_id, algorithm, dimensions |
| `SolveConverged` | Solver Core | Coherence, Metrics, Streaming API | request_id, iterations, residual |
| `AlgorithmFallback` | Solver Core | Routing (SONA), Metrics | from_algorithm, to_algorithm, reason |
| `SparsityDetected` | Sparsity Analyzer | Routing | density, recommended_path |
| `BudgetExhausted` | Budget Enforcer | Coherence Gate, Metrics | budget, best_residual |
| `CoherenceUpdated` | Coherence Adapter | Prime Radiant | energy_before, energy_after, solver_used |
| `RoutingDecision` | Algorithm Router | SONA Learning | features, selected_algorithm, latency |

### Event Flow

```
                SolverOrchestrator
                       │
               emits SolverEvent
                       │
              ┌────────┴────────┐
              │ broadcast::Sender│
              └────────┬────────┘
                       │
        ┌──────┬───────┼───────┬──────────┐
        ▼      ▼       ▼       ▼          ▼
  Coherence  Metrics  Stream  Audit   SONA
  Engine     Collector API    Trail   Learning
```

---

## 8. Strategic Patterns

### 8.1 Event Sourcing (Aligned with Prime Radiant)

SolverEvent follows the same tagged-enum pattern as Prime Radiant's DomainEvent:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SolverEvent {
    SolveRequested { ... },
    IterationCompleted { ... },
    SolveConverged { ... },
    AlgorithmFallback { ... },
    BudgetExhausted { ... },
}
```

Enables deterministic replay, tamper detection via content hashes, and forensic analysis.

### 8.2 CQRS for Solver

- **Command side**: `solve(input)` — mutates state, produces events
- **Query side**: `estimate_complexity(input)` — pure function, no side effects
- Separate read/write models enable caching of complexity estimates

### 8.3 Saga for Multi-Phase Solves

TRUE algorithm requires three sequential phases:
1. JL Projection (reduces dimensionality)
2. Spectral Sparsification (reduces edges)
3. Neumann Solve (actual computation)

Each phase is compensatable: if phase 3 fails, phases 1-2 results are cached for retry with different solver.

```
[JL Projection] ──success──▶ [Sparsification] ──success──▶ [Neumann Solve]
       │                           │                              │
    failure                     failure                        failure
       │                           │                              │
       ▼                           ▼                              ▼
  [Log & Abort]            [Retry with coarser ε]         [Fallback to CG]
```

---

## 9. Evolution Strategy

| Phase | Timeline | Scope | Key Milestone |
|-------|----------|-------|---------------|
| Phase 1 | Weeks 1-2 | Foundation crate + CG + Neumann | First `cargo test` passing |
| Phase 2 | Weeks 3-5 | Push algorithms + routing + coherence integration | Coherence 10x speedup |
| Phase 3 | Weeks 6-8 | TRUE + BMSSP + WASM + NAPI | Full platform coverage |
| Phase 4 | Weeks 9-10 | SONA learning + benchmarks + security hardening | Production readiness |
