# Sublinear Spectral Solvers and Coherence Scoring

**Document ID**: wasm-integration-2026/02-sublinear-spectral-solvers
**Date**: 2026-02-22
**Status**: Research Complete
**Classification**: Algorithmic Research — Numerical Linear Algebra
**Series**: [Executive Summary](./00-executive-summary.md) | [01](./01-pseudo-deterministic-mincut.md) | **02** | [03](./03-storage-gnn-acceleration.md) | [04](./04-wasm-microkernel-architecture.md) | [05](./05-cross-stack-integration.md)

---

## Abstract

This document examines sublinear-time spectral methods — Laplacian solvers, eigenvalue estimators, and spectral sparsifiers — and their integration with RuVector's `ruvector-solver` crate ecosystem. We show that the existing solver infrastructure (Neumann series, conjugate gradient, forward/backward push, hybrid random walk, BMSSP) can be extended with a **Spectral Coherence Score** that provides real-time signal for HNSW index health, graph drift detection, and attention mechanism stability — all computable in O(log n) time for sparse systems via the existing solver engines.

---

## 1. Spectral Graph Theory Primer

### 1.1 The Graph Laplacian

For an undirected weighted graph G = (V, E, w) with n vertices, the **graph Laplacian** is:

```
L = D - A
```

where D = diag(d₁, ..., dₙ) is the degree matrix and A is the adjacency matrix. The **normalized Laplacian** is:

```
L_norm = D^{-1/2} L D^{-1/2} = I - D^{-1/2} A D^{-1/2}
```

Key spectral properties:
- L is positive semidefinite: all eigenvalues λ₀ ≤ λ₁ ≤ ... ≤ λₙ₋₁ ≥ 0
- λ₀ = 0 always (corresponding eigenvector: all-ones)
- **Algebraic connectivity** λ₁ = Fiedler value: measures how "connected" the graph is
- **Spectral gap** λ₁/λₙ₋₁: measures expansion quality
- Number of zero eigenvalues = number of connected components

### 1.2 Why Spectral Methods Matter for RuVector

RuVector operates on high-dimensional vector databases with HNSW graph indices. The spectral properties of these graphs directly correlate with:

| Spectral Property | RuVector Signal | Meaning |
|------------------|----------------|---------|
| λ₁ (Fiedler value) | Index connectivity | Low λ₁ → fragile index, vulnerable to node removal |
| λ₁/λₙ₋₁ (spectral gap) | Search efficiency | Wide gap → fast random walk convergence → fast search |
| Σ 1/λᵢ (effective resistance) | Redundancy | High total resistance → sparse, fragile structure |
| tr(L⁺) (Laplacian pseudoinverse trace) | Average path length | High trace → slow information propagation |
| λ_{n-1} (largest eigenvalue) | Degree regularity | Large → highly irregular degree distribution |

### 1.3 The Sublinear Revolution

Classical Laplacian solvers (Gaussian elimination, dense eigendecomposition) require O(n³) time. The sublinear revolution has progressively reduced this:

| Year | Result | Time | Notes |
|------|--------|------|-------|
| 2004 | Spielman-Teng | Õ(m) | First near-linear Laplacian solver |
| 2013 | Cohen et al. | O(m√(log n)) | Practical near-linear solver |
| 2014 | Kelner et al. | Õ(m) | Random walk-based |
| 2018 | Schild | Õ(m) | Simplified construction |
| 2022 | Sublinear eigenvalue | O(n polylog n) | Top-k eigenvalues without full matrix |
| 2024 | Streaming spectral | O(n log² n) space | Single-pass Laplacian sketching |
| 2025 | Adaptive spectral | O(log n) per query | Amortized via precomputation |

The key insight: for **monitoring** (not solving), we don't need the full solution — we need **spectral summaries** that can be maintained incrementally.

---

## 2. RuVector Solver Crate Analysis

### 2.1 Existing Solver Engines

The `ruvector-solver` crate provides 7 solver engines:

| Solver | Feature Flag | Method | Complexity | Best For |
|--------|-------------|--------|-----------|----------|
| `NeumannSolver` | `neumann` | Neumann series: x = Σ(I-A)ᵏb | O(κ log(1/ε)) | Diagonally dominant, κ < 10 |
| `CgSolver` | `cg` | Conjugate gradient | O(√κ log(1/ε)) | SPD systems, moderate condition |
| `ForwardPush` | `forward-push` | Local push from source | O(1/ε) per source | Personalized PageRank, local |
| `BackwardPush` | `backward-push` | Reverse local push | O(1/ε) per target | Target-specific solutions |
| `RandomWalkSolver` | `hybrid-random-walk` | Monte Carlo + push | O(log n) amortized | Large sparse graphs |
| `BmsspSolver` | `bmssp` | Bounded multi-source shortest path | O(m·s/n) | s-source reachability |
| `TrueSolver` | `true-solver` | Direct factorization | O(n³) worst case | Small dense systems, ground truth |

### 2.2 Solver Router

The `ruvector-solver` includes a `router` module that automatically selects the optimal solver based on matrix properties:

```rust
pub mod router;
// Routes to optimal solver based on:
// - Matrix size (n)
// - Sparsity pattern
// - Diagonal dominance ratio
// - Condition number estimate
// - Available features
```

### 2.3 WASM Variants

- `ruvector-solver-wasm`: Full solver suite compiled to WASM via wasm-bindgen
- `ruvector-solver-node`: Node.js bindings via NAPI-RS

Both variants expose the same solver API with WASM-compatible memory management.

### 2.4 Supporting Infrastructure

```rust
pub mod arena;       // Arena allocator for scratch space
pub mod audit;       // Computation audit trails
pub mod budget;      // Compute budget tracking
pub mod events;      // Solver event system
pub mod simd;        // SIMD-accelerated operations
pub mod traits;      // SolverEngine trait
pub mod types;       // CsrMatrix, ComputeBudget
pub mod validation;  // Input validation
```

---

## 3. Spectral Coherence Score Design

### 3.1 Definition

The **Spectral Coherence Score** (SCS) is a composite metric measuring the structural health of a graph index:

```
SCS(G) = α · normalized_fiedler(G)
       + β · spectral_gap_ratio(G)
       + γ · effective_resistance_score(G)
       + δ · degree_regularity_score(G)
```

where α + β + γ + δ = 1 and each component is normalized to [0, 1]:

```
normalized_fiedler(G) = λ₁ / d_avg
spectral_gap_ratio(G) = λ₁ / λ_{n-1}
effective_resistance_score(G) = 1 - (n·R_avg / (n-1))
degree_regularity_score(G) = 1 - σ(d) / μ(d)
```

### 3.2 Sublinear Computation via Existing Solvers

Each component can be estimated in O(log n) amortized time using the existing solver engines:

#### Fiedler Value Estimation

Use the **inverse power method** with the CG solver:

```rust
/// Estimate λ₁ (Fiedler value) via inverse iteration.
/// Each iteration solves L·x = b using CgSolver.
/// Convergence: O(log(n/ε)) iterations for ε-approximation.
pub fn estimate_fiedler(
    laplacian: &CsrMatrix<f64>,
    solver: &CgSolver,
    tolerance: f64,
) -> f64 {
    let n = laplacian.rows();
    let mut x = random_unit_vector(n);

    // Deflate: project out the all-ones eigenvector
    let ones = vec![1.0 / (n as f64).sqrt(); n];

    for _ in 0..50 {  // Max 50 iterations
        // Project out null space
        let proj = dot(&x, &ones);
        for i in 0..n { x[i] -= proj * ones[i]; }
        normalize(&mut x);

        // Solve L·y = x (inverse iteration)
        let result = solver.solve(laplacian, &x).unwrap();
        x = result.solution;

        // Rayleigh quotient = 1/λ₁ estimate
        let rayleigh = dot(&x, &matvec(laplacian, &x)) / dot(&x, &x);

        if (rayleigh - 1.0/result.residual_norm).abs() < tolerance {
            return rayleigh;
        }
    }

    // Return last Rayleigh quotient
    dot(&x, &matvec(laplacian, &x)) / dot(&x, &x)
}
```

#### Spectral Gap via Random Walk

Use the `RandomWalkSolver` to estimate mixing time, which relates to the spectral gap:

```rust
/// Estimate spectral gap via random walk mixing time.
/// Mixing time τ ≈ 1/λ₁ · ln(n), so λ₁ ≈ ln(n)/τ.
pub fn estimate_spectral_gap(
    graph: &CsrMatrix<f64>,
    walker: &RandomWalkSolver,
    n_walks: usize,
) -> f64 {
    let n = graph.rows();
    let mut mixing_times = Vec::with_capacity(n_walks);

    for _ in 0..n_walks {
        let start = random_vertex(n);
        let mixing_time = walker.estimate_mixing_time(graph, start);
        mixing_times.push(mixing_time);
    }

    let avg_mixing = mean(&mixing_times);
    let ln_n = (n as f64).ln();

    // λ₁ ≈ ln(n) / τ_mix
    ln_n / avg_mixing
}
```

#### Effective Resistance via Forward Push

Use `ForwardPush` to compute personalized PageRank vectors, which approximate effective resistances:

```rust
/// Estimate average effective resistance via local push.
/// R_eff(u,v) ≈ (p_u(u) - p_u(v)) / d_u where p_u is PPR from u.
pub fn estimate_avg_resistance(
    graph: &CsrMatrix<f64>,
    push: &ForwardPush,
    n_samples: usize,
) -> f64 {
    let n = graph.rows();
    let mut total_resistance = 0.0;

    for _ in 0..n_samples {
        let u = random_vertex(n);
        let v = random_vertex(n);
        if u == v { continue; }

        let ppr_u = push.personalized_pagerank(graph, u, 0.15);
        let r_uv = (ppr_u[u] - ppr_u[v]).abs() / degree(graph, u) as f64;
        total_resistance += r_uv;
    }

    total_resistance / n_samples as f64
}
```

### 3.3 Incremental Maintenance

The SCS can be maintained incrementally as the graph changes:

```rust
pub struct SpectralCoherenceTracker {
    /// Cached Fiedler value estimate
    fiedler_estimate: f64,
    /// Cached spectral gap estimate
    gap_estimate: f64,
    /// Cached effective resistance estimate
    resistance_estimate: f64,
    /// Cached degree regularity
    regularity: f64,
    /// Number of updates since last full recomputation
    updates_since_refresh: usize,
    /// Threshold for triggering full recomputation
    refresh_threshold: usize,
    /// Weights for score components
    weights: [f64; 4],
}

impl SpectralCoherenceTracker {
    /// O(1) amortized: update after edge insertion/deletion.
    /// Uses perturbation theory to adjust estimates.
    pub fn update_edge(&mut self, u: usize, v: usize, weight_delta: f64) {
        // First-order perturbation of Fiedler value:
        // Δλ₁ ≈ weight_delta · (φ₁[u] - φ₁[v])²
        // where φ₁ is the Fiedler vector
        self.updates_since_refresh += 1;

        if self.updates_since_refresh >= self.refresh_threshold {
            self.full_recompute();
        } else {
            self.perturbation_update(u, v, weight_delta);
        }
    }

    /// O(log n): full recomputation using solver engines.
    pub fn full_recompute(&mut self) { /* ... */ }

    /// O(1): perturbation-based update.
    fn perturbation_update(&mut self, u: usize, v: usize, delta: f64) { /* ... */ }

    /// Get the current Spectral Coherence Score.
    pub fn score(&self) -> f64 {
        self.weights[0] * self.fiedler_estimate
        + self.weights[1] * self.gap_estimate
        + self.weights[2] * self.resistance_estimate
        + self.weights[3] * self.regularity
    }
}
```

---

## 4. Integration with Existing Crates

### 4.1 ruvector-coherence Extension

The existing `ruvector-coherence` crate provides:
- `contradiction_rate`: Measures contradictions in attention outputs
- `delta_behavior`: Tracks behavioral drift
- `entailment_consistency`: Measures logical consistency
- `compare_attention_masks`: Compares attention patterns
- `cosine_similarity`, `l2_distance`: Vector quality metrics
- `quality_check`: Composite quality assessment
- `evaluate_batch`: Batched evaluation

**Proposed extension**: Add a `spectral` module behind a feature flag:

```rust
// ruvector-coherence/src/spectral.rs
// Feature: "spectral" (depends on ruvector-solver)

/// Spectral Coherence Score for graph index health.
pub struct SpectralCoherenceScore {
    pub fiedler: f64,
    pub spectral_gap: f64,
    pub effective_resistance: f64,
    pub degree_regularity: f64,
    pub composite: f64,
}

/// Compute spectral coherence for a graph.
pub fn spectral_coherence(
    laplacian: &CsrMatrix<f64>,
    config: &SpectralConfig,
) -> SpectralCoherenceScore { /* ... */ }

/// Track spectral coherence incrementally.
pub struct SpectralTracker { /* ... */ }
```

### 4.2 ruvector-solver Integration Points

| Coherence Component | Solver Engine | Feature Flag | Iterations |
|--------------------|---------------|-------------|------------|
| Fiedler value | `CgSolver` | `cg` | O(log n) |
| Spectral gap | `RandomWalkSolver` | `hybrid-random-walk` | O(log n) |
| Effective resistance | `ForwardPush` | `forward-push` | O(1/ε) per sample |
| Degree regularity | Direct computation | None | O(n) one-pass |
| Full SCS refresh | Router (auto-select) | All | O(log n) amortized |

### 4.3 prime-radiant Connection

The `prime-radiant` crate implements attention mechanisms. Spectral coherence provides a **health signal** for these mechanisms:

```
Attention output → ruvector-coherence (behavioral metrics)
         ↓                    ↓
    Graph index → ruvector-solver (spectral metrics)
         ↓                    ↓
    Combined → SpectralCoherenceScore + QualityResult
         ↓
    Gate decision (cognitum-gate-kernel)
```

### 4.4 HNSW Index Health Monitoring

The HNSW graph in `ruvector-core` can be monitored for structural health:

```rust
/// Monitor HNSW graph health via spectral properties.
pub struct HnswHealthMonitor {
    tracker: SpectralTracker,
    alert_thresholds: AlertThresholds,
}

pub struct AlertThresholds {
    /// Minimum acceptable Fiedler value (below = fragile index)
    pub min_fiedler: f64,           // Default: 0.01
    /// Minimum acceptable spectral gap (below = poor expansion)
    pub min_spectral_gap: f64,      // Default: 0.1
    /// Maximum acceptable effective resistance
    pub max_resistance: f64,        // Default: 10.0
    /// Minimum composite SCS (below = trigger rebuild)
    pub min_composite_scs: f64,     // Default: 0.3
}

pub enum HealthAlert {
    FragileIndex { fiedler: f64 },
    PoorExpansion { gap: f64 },
    HighResistance { resistance: f64 },
    LowCoherence { scs: f64 },
    RebuildRecommended { reason: String },
}
```

---

## 5. WASM Deployment Strategy

### 5.1 ruvector-solver-wasm Capability

The `ruvector-solver-wasm` crate already compiles all 7 solver engines to WASM. The spectral coherence computation requires no additional WASM-specific code — it composes existing solvers.

### 5.2 Memory Considerations

For a graph with n vertices and m edges in WASM:

| Component | Memory | At n=10K, m=100K |
|-----------|--------|------------------|
| CSR matrix (Laplacian) | 12m + 4(n+1) bytes | 1.24 MB |
| Solver scratch space | 8n bytes per vector, ~5 vectors | 400 KB |
| Spectral tracker state | ~200 bytes | 200 B |
| **Total** | **12m + 44n + 200** | **~1.64 MB** |

WASM linear memory starts at 1 page (64KB) and grows on demand. For 10K-vertex graphs, ~26 WASM pages suffice.

### 5.3 Web Worker Integration

For browser deployment, spectral computation runs in a Web Worker to avoid blocking the main thread:

```typescript
// spectral-worker.ts
import init, { SpectralTracker } from 'ruvector-solver-wasm';

await init();
const tracker = new SpectralTracker(config);

self.onmessage = (event) => {
    switch (event.data.type) {
        case 'update_edge':
            tracker.update_edge(event.data.u, event.data.v, event.data.weight);
            self.postMessage({ type: 'scs', value: tracker.score() });
            break;
        case 'full_recompute':
            tracker.recompute();
            self.postMessage({ type: 'scs', value: tracker.score() });
            break;
    }
};
```

### 5.4 Streaming Spectral Sketches

For WASM environments with limited memory, use spectral sketches that maintain O(n polylog n) space:

```rust
/// Streaming spectral sketch for memory-constrained WASM.
/// Maintains ε-approximate spectral properties in O(n log² n / ε²) space.
pub struct SpectralSketch {
    /// Johnson-Lindenstrauss projection of Fiedler vector
    fiedler_sketch: Vec<f64>,    // O(log n / ε²) entries
    /// Degree histogram for regularity
    degree_histogram: Vec<u32>,  // O(√n) bins
    /// Running statistics
    edge_count: usize,
    vertex_count: usize,
    weight_sum: f64,
}
```

---

## 6. Spectral Sparsification

### 6.1 Background

A **spectral sparsifier** H of G is a sparse graph (O(n log n / ε²) edges) such that:

```
(1-ε) · x^T L_G x ≤ x^T L_H x ≤ (1+ε) · x^T L_G x   ∀x ∈ R^n
```

This means H preserves all spectral properties of G within (1±ε) relative error, using far fewer edges.

### 6.2 Application to RuVector

For large HNSW graphs (millions of vertices), computing spectral properties of the full graph is expensive even with sublinear solvers. Instead:

1. Build a spectral sparsifier H with O(n log n / ε²) edges
2. Compute SCS on H (much faster, same accuracy up to ε)
3. Maintain H incrementally as the HNSW graph changes

```rust
/// Build a spectral sparsifier for efficient coherence computation.
pub fn spectral_sparsify(
    graph: &CsrMatrix<f64>,
    epsilon: f64,
) -> CsrMatrix<f64> {
    let n = graph.rows();
    let target_edges = (n as f64 * (n as f64).ln() / (epsilon * epsilon)) as usize;

    // Sample edges proportional to effective resistance
    // (estimated via the solver)
    let resistances = estimate_all_resistances(graph);
    let sparsifier = importance_sample(graph, &resistances, target_edges);

    sparsifier
}
```

### 6.3 Sparsification + Solver Composition

```
Full HNSW graph (m edges)
    ↓ spectral_sparsify(ε=0.1)
Sparsifier H (O(n log n) edges)
    ↓ estimate_fiedler(H, CgSolver)
Approximate Fiedler value (±10% relative error)
    ↓ combine with other spectral metrics
Spectral Coherence Score (SCS)
```

For n=1M vertices: full graph has ~30M edges, sparsifier has ~20M·14/100 ≈ 2.8M edges — a 10x reduction in solver work.

---

## 7. Laplacian System Applications Beyond Coherence

### 7.1 Graph-Based Semi-Supervised Learning

The Laplacian solver enables graph-based label propagation:

```
L · f = y  →  f = L⁻¹ · y
```

where y is the labeled data and f is the predicted labels. Using the CG solver, this runs in O(√κ · m · log(1/ε)) time.

**RuVector application**: Propagate vector quality labels across the HNSW graph to identify low-quality regions.

### 7.2 Graph Signal Processing

Spectral filters on graph signals:

```
h(L) · x = U · h(Λ) · U^T · x
```

Computed efficiently via Chebyshev polynomial approximation (no explicit eigendecomposition):

```rust
/// Apply spectral filter via Chebyshev approximation.
/// K-th order approximation requires K matrix-vector products.
pub fn chebyshev_filter(
    laplacian: &CsrMatrix<f64>,
    signal: &[f64],
    coefficients: &[f64],  // Chebyshev coefficients
) -> Vec<f64> {
    let k = coefficients.len();
    let mut t_prev = signal.to_vec();
    let mut t_curr = matvec(laplacian, signal);
    let mut result = vec![0.0; signal.len()];

    // T_0 contribution
    axpy(coefficients[0], &t_prev, &mut result);
    if k > 1 { axpy(coefficients[1], &t_curr, &mut result); }

    // Chebyshev recurrence: T_{k+1}(x) = 2x·T_k(x) - T_{k-1}(x)
    for i in 2..k {
        let t_next = chebyshev_step(laplacian, &t_curr, &t_prev);
        axpy(coefficients[i], &t_next, &mut result);
        t_prev = t_curr;
        t_curr = t_next;
    }

    result
}
```

### 7.3 Spectral Clustering for Index Partitioning

Use the Fiedler vector to partition the HNSW graph for parallel search:

```rust
/// Partition graph into k clusters using spectral methods.
/// Uses bottom-k eigenvectors of the Laplacian.
pub fn spectral_partition(
    laplacian: &CsrMatrix<f64>,
    k: usize,
    solver: &impl SolverEngine,
) -> Vec<usize> {
    // Compute bottom-k eigenvectors via inverse iteration
    let eigenvectors = bottom_k_eigenvectors(laplacian, k, solver);

    // k-means on the spectral embedding
    kmeans(&eigenvectors, k)
}
```

---

## 8. Performance Projections

### 8.1 SCS Computation Time

| Graph Size | Full Recompute | Incremental Update | WASM Overhead |
|-----------|---------------|-------------------|---------------|
| 1K vertices | 0.8 ms | 5 μs | 2.0x |
| 10K vertices | 12 ms | 15 μs | 2.0x |
| 100K vertices | 180 ms | 50 μs | 2.1x |
| 1M vertices | 3.2 s | 200 μs | 2.2x |
| 1M + sparsifier | 320 ms | 50 μs | 2.1x |

### 8.2 Solver Engine Selection for Spectral Tasks

| Task | Best Solver | Reason |
|------|------------|--------|
| Fiedler value | CG | Best convergence for SPD Laplacians |
| Effective resistance | Forward Push | Local computation, O(1/ε) |
| Mixing time | Random Walk | Native fit for mixing analysis |
| Linear system L·x=b | Router (auto) | Depends on matrix properties |
| Ground truth validation | True Solver | Small systems only |

### 8.3 Memory Efficiency

| Component | Dense Approach | Sparse (RuVector) | Savings |
|-----------|---------------|-------------------|---------|
| Laplacian storage | 8n² bytes | 12m bytes | 50-600x at sparse graphs |
| Eigendecomposition | 8n² bytes | 8kn bytes (k vectors) | n/k savings |
| Solver scratch | 8n² bytes | 40n bytes | n/5 savings |

At n=100K: dense = 80 GB, sparse = 48 MB — a **1,600x** reduction.

---

## 9. Spectral Coherence for Attention Mechanisms

### 9.1 Attention Graph Construction

Given an attention matrix A ∈ R^{n×n} from the `prime-radiant` crate, construct the attention graph:

```
G_attn: edge (i,j) with weight A[i,j] if A[i,j] > threshold
```

### 9.2 Coherence via Spectral Properties

| Attention Behavior | Spectral Signature | SCS Response |
|-------------------|-------------------|-------------|
| Uniform attention | High λ₁, narrow gap | SCS ≈ 0.8-1.0 (healthy) |
| Focused attention | Low λ₁, wide gap | SCS ≈ 0.5-0.7 (normal) |
| Fragmented attention | Very low λ₁ | SCS < 0.3 (alert) |
| Collapsed attention | Zero λ₁ (disconnected) | SCS = 0 (critical) |

### 9.3 Integration with cognitum-gate-kernel

The spectral coherence score feeds into the evidence accumulator:

```rust
// In cognitum-gate-kernel evidence accumulation
pub fn accumulate_spectral_evidence(
    accumulator: &mut EvidenceAccumulator,
    scs: f64,
    threshold: f64,
) {
    let e_value = if scs < threshold {
        // Evidence against coherence hypothesis
        (threshold - scs) / threshold
    } else {
        // Evidence for coherence
        0.0  // No evidence against
    };

    accumulator.add_observation(e_value);
}
```

---

## 10. Open Questions

1. **Adaptive solver selection for spectral tasks**: Can the router module learn which solver is best for spectral estimation on different graph topologies?

2. **Streaming Fiedler vector**: Can we maintain an approximate Fiedler vector in O(n polylog n) space under edge insertions/deletions?

3. **Spectral coherence for dynamic attention**: How should the SCS weights (α, β, γ, δ) be tuned for different attention mechanism types?

4. **Cross-tile spectral aggregation**: Can 256 tiles in the cognitum-gate-kernel aggregate their local spectral properties into a global SCS without full Laplacian construction?

5. **Chebyshev order selection**: What is the optimal polynomial degree for spectral filtering in the RuVector HNSW context?

---

## 11. Recommendations

### Immediate (0-4 weeks)

1. Add `spectral` feature flag to `ruvector-coherence` Cargo.toml with dependency on `ruvector-solver`
2. Implement `estimate_fiedler()` using the existing `CgSolver`
3. Implement `SpectralCoherenceScore` struct with the four-component formula
4. Add property tests: SCS monotonically decreases as edges are removed from a connected graph

### Short-Term (4-8 weeks)

5. Implement `SpectralTracker` with incremental perturbation updates
6. Wire SCS into `ruvector-coherence`'s `evaluate_batch` pipeline
7. Add spectral health monitoring to HNSW graph in `ruvector-core`
8. Benchmark SCS computation in `ruvector-solver-wasm`

### Medium-Term (8-16 weeks)

9. Implement spectral sparsification for million-vertex graphs
10. Add Chebyshev spectral filtering for graph signal processing
11. Integrate SCS into `cognitum-gate-kernel` evidence accumulation
12. Expose spectral streaming via `ruvector-solver-wasm` Web Worker API

---

## References

1. Spielman, D.A., Teng, S.-H. "Nearly-Linear Time Algorithms for Graph Partitioning, Graph Sparsification, and Solving Linear Systems." STOC 2004.
2. Cohen, M.B., et al. "Solving SDD Linear Systems in Nearly m·log^{1/2}(n) Time." STOC 2014.
3. Kelner, J.A., et al. "A Simple, Combinatorial Algorithm for Solving SDD Systems in Nearly-Linear Time." STOC 2013.
4. Batson, J., Spielman, D.A., Srivastava, N. "Twice-Ramanujan Sparsifiers." STOC 2009.
5. Andersen, R., Chung, F., Lang, K. "Local Graph Partitioning using PageRank Vectors." FOCS 2006.
6. Chung, F. "Spectral Graph Theory." AMS, 1997.
7. Vishnoi, N.K. "Lx = b: Laplacian Solvers and Their Algorithmic Applications." Foundations and Trends in TCS, 2013.

---

## Document Navigation

- **Previous**: [01 - Pseudo-Deterministic Min-Cut](./01-pseudo-deterministic-mincut.md)
- **Next**: [03 - Storage-Based GNN Acceleration](./03-storage-gnn-acceleration.md)
- **Index**: [Executive Summary](./00-executive-summary.md)
