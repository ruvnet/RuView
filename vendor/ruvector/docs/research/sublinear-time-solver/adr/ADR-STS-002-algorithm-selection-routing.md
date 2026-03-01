# ADR-STS-002: Algorithm Selection and Sublinear Routing Strategy

## Status

Accepted

## Date

2026-02-20

## Authors

RuVector Architecture Team

## Deciders

Architecture Review Board

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-02-20 | RuVector Team | Initial proposal |
| 0.2 | 2026-02-20 | RuVector Team | Comprehensive rewrite: crossover analysis, error budget decomposition, SONA/EWC integration, full decision matrix |
| 1.0 | 2026-02-20 | RuVector Team | Accepted: full implementation complete |

---

## Context

RuVector integrates seven sublinear algorithms from the sublinear-time-solver library. Each
algorithm occupies a distinct region of the problem-characteristic space, with non-trivial
crossover boundaries determined by matrix size (n), sparsity (nnz/n^2), condition number
(kappa), query type (single-source, pairwise, batch), target platform (native/WASM/edge),
and available compute budget (wall-time, memory, energy).

### Algorithm Portfolio

| Algorithm | Complexity | Primary Domain |
|-----------|-----------|----------------|
| Neumann Series | O(k * nnz) | Sparse SPD, diagonally dominant |
| Forward Push | O(1/eps) | Single-source PPR, graph exploration |
| Backward Push | O(1/eps) | Reverse relevance to target node |
| Hybrid Random Walk | O(sqrt(n)/eps) | Pairwise relevance, Monte Carlo |
| TRUE | O(log n) amortized | Large-scale Laplacian, JL + sparsification |
| Conjugate Gradient | O(sqrt(kappa) * log(1/eps) * nnz) | Gold-standard SPD solve |
| BMSSP | O(nnz * log n) | Multigrid hierarchical, near-linear |

### RuVector Consumption Points

Each RuVector subsystem requires different algorithms:

```
Prime Radiant (sheaf Laplacian)     -> CG, TRUE, Neumann
ruvector-gnn (message passing)      -> Forward Push, Neumann
ruvector-math (spectral filtering)  -> Neumann, CG, Chebyshev (existing)
ruvector-graph (PageRank/centrality)-> Forward Push, Backward Push
ruvector-attention (PDE diffusion)  -> CG on sparse Laplacian
ruvector-mincut (effective resist.) -> CG, TRUE (shared sparsifier)
ruvector-core (distance approx.)    -> JL projection (TRUE component)
```

### Motivating Constraints

Without a principled routing strategy, each RuVector subsystem would need to independently
select an algorithm, leading to duplicated logic, inconsistent quality-of-service, missed
optimization opportunities, and platform-incompatible choices (e.g., selecting TRUE with
heavy preprocessing on a WASM target with a 4 MB memory budget).

The routing problem is compounded by:

1. **Heterogeneous platforms**: Native (AVX-512, 64 GB RAM), WASM browser (SIMD128, 4 MB),
   Cloudflare edge (SIMD128, 128 MB), Apple Silicon (NEON, 16 GB).
2. **Diverse query types**: RuVector's 10+ crates generate fundamentally different problem
   structures, from dense O(n^2) attention matrices to ultra-sparse HNSW adjacency graphs.
3. **Cascading error budgets**: When multiple sublinear algorithms compose (e.g., JL
   projection -> sparsification -> Neumann iteration -> push aggregation), error accumulates
   and must be managed holistically.
4. **Latency constraints**: Compute lanes range from Lane 0 Reflex (<1 ms) to Lane 3
   Deliberate (unbounded), requiring algorithm choices that respect wall-time budgets.

---

## Decision

Implement a **three-tier routing system** that combines compile-time platform constraints,
runtime heuristic dispatch, and adaptive learning from historical performance.

### Tier 1: Static Rules (Compile-Time)

Feature flags and `cfg` attributes select the set of available algorithms per target platform.
These constraints are absolute and override all runtime decisions.

```toml
# WASM target -- exclude algorithms requiring heavy preprocessing
[target.'cfg(target_arch = "wasm32")'.dependencies]
ruvector-solver = { version = "0.1", default-features = false, features = [
    "neumann", "forward-push", "backward-push", "cg"
] }
# TRUE excluded: preprocessing O(m*log(n)/eps^2) exceeds WASM memory budget
# BMSSP excluded: hierarchy construction + storage O(nnz*log(n)) too large
# Hybrid Random Walk: conditional on getrandom/wasm_js for PRNG

# Native target -- all algorithms available
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
ruvector-solver = { version = "0.1", features = ["full"] }
```

Platform-algorithm availability matrix:

| Algorithm | Native x86_64 | Native ARM64 | WASM Browser (4MB) | WASM Edge (128MB) | NAPI Node |
|-----------|:---:|:---:|:---:|:---:|:---:|
| Neumann Series | Yes | Yes | Yes | Yes | Yes |
| Forward Push | Yes | Yes | Yes | Yes | Yes |
| Backward Push | Yes | Yes | Yes | Yes | Yes |
| Hybrid Random Walk | Yes | Yes | No (1) | Yes | Yes |
| TRUE | Yes | Yes | No (2) | Yes (n<500K) | Yes |
| Conjugate Gradient | Yes | Yes | Yes | Yes | Yes |
| BMSSP | Yes | Yes | No (2) | Yes (n<50K) | Yes |

(1) Requires `getrandom/wasm_js` feature for cryptographic PRNG in browser context.
(2) Preprocessing memory exceeds browser budget for problems of practical size.

Feature flag structure:

```toml
[features]
default = ["solver-full"]
solver-full = ["solver-true", "solver-bmssp", "solver-neumann", "solver-push", "solver-cg"]
solver-true = []           # TRUE: JL + sparsification + adaptive Neumann
solver-bmssp = []          # BMSSP: multigrid hierarchical
solver-neumann = []        # Neumann series
solver-push = []           # Forward/Backward Push + Hybrid Random Walk
solver-cg = []             # Conjugate Gradient
solver-wasm = ["solver-neumann", "solver-push", "solver-cg"]  # WASM-safe subset
```

### Tier 2: Heuristic Router (Runtime, <1 ms)

A deterministic decision tree selects the optimal algorithm based on problem characteristics.
The router executes in under 1 ms and requires no heap allocation. All thresholds are derived
from the crossover analysis below.

#### Router Input Signature

```rust
pub struct RoutingQuery {
    /// Matrix dimension (n x n) or graph vertex count
    pub n: usize,
    /// Number of non-zero entries (for sparse matrices) or edge count
    pub nnz: usize,
    /// Query type determines which algorithms are applicable
    pub query_type: QueryType,
    /// Target accuracy (epsilon)
    pub eps: f64,
    /// Condition number estimate (0.0 if unknown; see Appendix B)
    pub kappa_estimate: f64,
    /// Available wall-time budget
    pub budget: ComputeBudget,
    /// Whether preprocessing has been amortized (batch mode)
    pub batch_mode: bool,
    /// Number of right-hand sides (for batch Laplacian solves)
    pub num_rhs: usize,
    /// Diagonal dominance ratio: min_i(A_ii / sum_{j!=i} |A_ij|), range [0,inf)
    pub diagonal_dominance: f64,
}

pub enum QueryType {
    /// Solve Ax = b for SPD matrix A
    LinearSolve,
    /// Single-source personalized PageRank from vertex s
    SingleSourcePPR { source: usize },
    /// Reverse relevance: all sources relevant to target t
    ReverseRelevance { target: usize },
    /// Pairwise relevance between specific (s, t)
    PairwiseRelevance { source: usize, target: usize },
    /// Spectral graph filtering: apply h(L)x
    SpectralFilter { filter_type: FilterType },
    /// Eigenvector computation for spectral clustering
    SpectralClustering { num_clusters: usize },
    /// Dimension reduction via JL projection
    DimensionReduction { target_dim: usize },
    /// Multi-scale graph decomposition
    MultiScaleDecomposition,
    /// Batch Laplacian solves (same graph, multiple RHS)
    BatchLaplacian { count: usize },
}

pub enum FilterType {
    /// Rational filter: (I + alpha*L)^{-1}, heat kernel
    Rational { alpha: f64 },
    /// Polynomial filter: Chebyshev expansion
    Polynomial { degree: usize },
    /// General filter requiring inversion
    General,
}
```

#### Decision Tree

The heuristic router implements the following decision tree. At each node, the first
matching rule fires.

```
ROOT
 |
 +-- QueryType::SingleSourcePPR
 |     +-- [ALWAYS] => ForwardPush
 |         Rationale: O(1/eps) independent of n, deterministic, no preprocessing.
 |         No other algorithm is competitive for single-source graph queries.
 |
 +-- QueryType::ReverseRelevance
 |     +-- [ALWAYS] => BackwardPush
 |         Rationale: Dual of ForwardPush, O(1/eps) for column queries.
 |
 +-- QueryType::PairwiseRelevance
 |     +-- n < 1,000 => ForwardPush (compute full PPR, read target entry)
 |     +-- n >= 1,000 => HybridRandomWalk
 |         Rationale: O(sqrt(n)/eps) beats full PPR computation for large n
 |         when only a single pairwise value is needed.
 |
 +-- QueryType::LinearSolve
 |     +-- n < 500 => CG (no preconditioner)
 |     |   Rationale: Below crossover for all sublinear methods. CG converges
 |     |   in O(sqrt(kappa)*log(1/eps)) iterations, each O(nnz). At n=500,
 |     |   even kappa=n^2 yields manageable iteration counts.
 |     |
 |     +-- sparsity_ratio < 0.01 AND kappa_estimate < 5 AND diagonal_dominance > 0.5
 |     |   => NeumannSeries
 |     |   Rationale: Very sparse, well-conditioned, diagonally dominant.
 |     |   Neumann converges geometrically with rate rho < 1-1/kappa.
 |     |   For kappa < 5: rho < 0.8, so k < 14*log(1/eps) iterations.
 |     |
 |     +-- sparsity_ratio < 0.05 AND kappa_estimate < 10,000
 |     |   => CG (diagonal preconditioner)
 |     |   Rationale: Moderate condition number. Diagonal preconditioning
 |     |   reduces effective kappa by ~10x. Iterations: O(sqrt(1000)*log(1/eps)).
 |     |
 |     +-- kappa_estimate >= 25 AND n > 50,000
 |     |   => BMSSP
 |     |   Rationale: Multigrid convergence is independent of kappa.
 |     |   O(nnz*log(1/eps)) per V-cycle. Beats CG's O(sqrt(kappa)*...).
 |     |
 |     +-- [DEFAULT] => CG (diagonal preconditioner)
 |         Rationale: Safe default for all SPD systems. Deterministic,
 |         well-understood convergence. Memory footprint O(nnz + 4n).
 |
 +-- QueryType::BatchLaplacian { count }
 |     +-- count >= 10 AND n >= 100,000 => TRUE
 |     |   Rationale: Amortize preprocessing O(m*log(n)/eps^2) over count solves.
 |     |   Per-solve cost O(log(n)) amortized. Break-even analysis: see Crossover Points.
 |     |
 |     +-- count >= 10 AND kappa_estimate >= 25 => BMSSP
 |     |   Rationale: Reuse multigrid hierarchy across batch. O(nnz*log(1/eps)) per solve.
 |     |
 |     +-- [DEFAULT] => CG
 |
 +-- QueryType::SpectralFilter { filter_type }
 |     +-- Rational { alpha } AND alpha >= 0.01
 |     |   => NeumannSeries on (I + alpha*L)
 |     |   Rationale: Guaranteed convergence (spectral radius of D^{-1}B < 1
 |     |   for alpha > 0). Iterations k = O(1/alpha). For alpha >= 0.01, k <= 100.
 |     |
 |     +-- General => CG
 |     |   Rationale: CG solves Lx = b directly for inversion-based filters.
 |     |
 |     +-- Polynomial { degree } => NoRoute (use existing Chebyshev)
 |         Rationale: Chebyshev recurrence is optimal for arbitrary polynomial
 |         filters. Router returns NoRoute; caller uses ruvector-math Chebyshev.
 |
 +-- QueryType::SpectralClustering { num_clusters }
 |     +-- n < 10,000 => CG (shift-invert for eigenvectors)
 |     +-- n >= 10,000 AND n < 100,000 => BMSSP (multigrid eigensolver)
 |     +-- n >= 100,000 => TRUE (sparsify + JL + adaptive Neumann)
 |
 +-- QueryType::DimensionReduction
 |     +-- [ALWAYS] => TRUE (JL component only)
 |         Rationale: JL projection to k = ceil(24*ln(n)/eps^2) dimensions.
 |
 +-- QueryType::MultiScaleDecomposition
       +-- [ALWAYS] => BMSSP
           Rationale: BMSSP coarsening hierarchy IS the decomposition.
```

#### Router Implementation

The router is a pure function with no allocations:

```rust
pub fn route(query: &RoutingQuery, available: &[Algorithm]) -> RoutingDecision {
    let sparsity_ratio = query.nnz as f64 / (query.n as f64 * query.n as f64);

    let candidate = match &query.query_type {
        QueryType::SingleSourcePPR { .. } => Algorithm::ForwardPush,
        QueryType::ReverseRelevance { .. } => Algorithm::BackwardPush,
        QueryType::PairwiseRelevance { .. } => {
            if query.n < 1_000 { Algorithm::ForwardPush }
            else { Algorithm::HybridRandomWalk }
        }
        QueryType::LinearSolve => route_linear_solve(query, sparsity_ratio),
        QueryType::BatchLaplacian { count } => route_batch(query, *count),
        QueryType::SpectralFilter { filter_type } => route_filter(query, filter_type),
        QueryType::SpectralClustering { .. } => route_clustering(query),
        QueryType::DimensionReduction { .. } => Algorithm::TRUE,
        QueryType::MultiScaleDecomposition => Algorithm::BMSSP,
    };

    if available.contains(&candidate) {
        RoutingDecision::Route(candidate)
    } else {
        RoutingDecision::Fallback(select_fallback(query, available))
    }
}

fn select_fallback(query: &RoutingQuery, available: &[Algorithm]) -> Algorithm {
    // Fallback priority: CG > Neumann > BMSSP > ForwardPush > HybridRW > TRUE
    let priority = [
        Algorithm::ConjugateGradient,
        Algorithm::NeumannSeries,
        Algorithm::BMSSP,
        Algorithm::ForwardPush,
        Algorithm::HybridRandomWalk,
        Algorithm::TRUE,
    ];
    priority.iter()
        .find(|a| available.contains(a))
        .copied()
        .unwrap_or(Algorithm::ConjugateGradient)
}
```

### Tier 3: Adaptive Learning (Runtime, SONA-Powered)

The third tier uses RuVector's SONA (Self-Optimizing Neural Architecture) framework to learn
from historical solve performance and adjust routing weights.

#### Architecture

```
                     RoutingQuery
                          |
                     [Tier 2 Heuristic]
                          |
                   candidate algorithm
                          |
                  [SONA Override Check]
                   /              \
              no override      override (confidence > 0.8)
                  |                  |
           use heuristic      use SONA prediction
                  |                  |
                  v                  v
              Execute Algorithm
                  |
             SolveOutcome {
               algorithm, wall_time, residual,
               iterations, memory_peak
             }
                  |
             [Feedback to SONA]
                  |
            Update routing weights
```

#### SONA Feature Extraction

SONA maintains a routing weight matrix W of shape [num_features x num_algorithms]. The
features are derived from the RoutingQuery:

```
Feature vector f(query):
  f[0]  = log2(n)                           // Scale feature
  f[1]  = log2(nnz + 1)                     // Density feature
  f[2]  = nnz / n^2                         // Sparsity ratio
  f[3]  = log2(kappa_estimate + 1)          // Condition number
  f[4]  = log2(1/eps)                       // Precision requirement
  f[5]  = encode(query_type)                // One-hot (7 categories)
  f[6]  = encode(platform)                  // One-hot (4 categories)
  f[7]  = budget.max_wall_time_ms / 1000.0  // Normalized time budget
  f[8]  = num_rhs                           // Batch size
  f[9]  = diagonal_dominance                // Dominance ratio
```

The SONA model predicts algorithm performance scores:

```
scores = softmax(W^T * f(query))
selected = argmax(scores) if max(scores) > confidence_threshold (0.8)
         = heuristic_choice otherwise
```

Memory overhead of SONA model: 10 features x 7 algorithms = 70 floats = 280 bytes.
EWC Fisher diagonal adds another 280 bytes. Total: < 1 KB.

#### EWC for Catastrophic Forgetting Prevention

When workload distribution shifts (e.g., a cluster transitions from graph queries to
attention-matrix solves), the learned weights must adapt without losing knowledge of the
previous workload. Elastic Weight Consolidation (EWC), already implemented in
`ruvector-gnn/src/ewc.rs`, prevents this:

```
L_total = L_current + (lambda/2) * sum_i( F_i * (W_i - W*_i)^2 )
```

where:
- F_i is the diagonal of the Fisher Information Matrix computed over the previous
  workload's routing decisions
- W*_i are the weights learned for that workload
- lambda controls the consolidation penalty strength (default: 100.0)

The Fisher diagonal is computed efficiently over the last N routing decisions:

```
F_i = (1/N) * sum_{j=1}^{N} (d log p(a_j | q_j; W) / dW_i)^2
```

EWC checkpoints every 1,000 outcomes to capture workload snapshots.

#### Feedback Loop and Reward Function

After each solve, the actual performance is fed back:

```rust
pub struct SolveOutcome {
    pub query: RoutingQuery,
    pub algorithm_used: Algorithm,
    pub wall_time_us: u64,
    pub iterations: usize,
    pub final_residual: f64,
    pub memory_peak_bytes: usize,
    pub converged: bool,
}

impl SONARouter {
    pub fn record_outcome(&mut self, outcome: SolveOutcome) {
        let reward = self.compute_reward(&outcome);
        let features = self.extract_features(&outcome.query);
        self.sona.update(features, outcome.algorithm_used, reward);

        if self.outcome_count % 1000 == 0 {
            self.sona.update_fisher_diagonal();
        }
    }

    fn compute_reward(&self, outcome: &SolveOutcome) -> f64 {
        if !outcome.converged {
            return -1.0;
        }
        let time_score = 1.0 - (outcome.wall_time_us as f64
            / outcome.query.budget.max_wall_time_us() as f64).min(1.0);
        let accuracy_score = (outcome.final_residual.log10()
            / outcome.query.eps.log10()).min(1.0).max(0.0);
        let memory_score = 1.0 - (outcome.memory_peak_bytes as f64
            / outcome.query.budget.max_memory_bytes as f64).min(1.0);
        0.5 * time_score + 0.3 * accuracy_score + 0.2 * memory_score
    }
}
```

Cold start: SONA requires ~10,000 recorded outcomes before overrides become reliable.
During cold start, all decisions fall through to the Tier 2 heuristic.

---

### Algorithm Selection Decision Matrix

The authoritative reference for routing decisions across all relevant dimensions:

| Dimension | Neumann Series | Forward Push | Backward Push | Hybrid RW | TRUE | CG | BMSSP |
|-----------|---------------|-------------|---------------|-----------|------|-----|-------|
| **Input type** | Sparse SPD matrix A | Graph G + source s | Graph G + target t | Graph G + (s,t) | Sparse Laplacian L | Sparse SPD matrix A | Sparse Laplacian L |
| **Output type** | Approx A^{-1}b | PPR vector pi_s | PPR column pi_{*,t} | Scalar pi(s,t) | Approx L^{-1}b | Exact (to tol) x | Approx L^{-1}b |
| **Best n range** | 500 - 1M | 1 - unlimited | 1 - unlimited | 1K - 10M | 100K - unlimited | 100 - 10M | 50K - 10M |
| **Sparsity req.** | nnz/n^2 < 0.1 | Natural graph | Natural graph | Natural graph | Any sparse | nnz/n^2 < 0.5 | Hierarchical structure |
| **Preprocessing** | None, O(1) | None, O(1) | None, O(1) | None, O(1) | O(m*log(n)/eps^2) | O(n) diag precond | O(m*log(n)) coarsen |
| **Per-solve cost** | O(k*nnz) | O(1/eps) | O(1/eps) | O(sqrt(n)/eps) | O(log(n)) amortized | O(sqrt(kappa)*log(1/eps)*nnz) | O(nnz*log(1/eps)) |
| **Deterministic?** | Yes | Yes | Yes | No (Monte Carlo) | No (JL + sampling) | Yes | Partially (AMG coarsening) |
| **Parallelizable?** | SpMV parallel | Limited (push serial) | Limited (push serial) | Walk parallel (good) | High (all phases) | SpMV parallel | Level-parallel |
| **WASM compatible?** | Yes | Yes | Yes | Conditional (PRNG) | No (memory) | Yes | Conditional (n<50K) |
| **Numerical stability** | Requires rho(D^{-1}B)<1 | Kahan summation | Kahan summation | Variance management | 3-component error | Reorthogonalization | Coarsening quality |
| **Convergence** | Geometric: rho^k | Absolute: eps*vol(G) | Absolute: eps*vol(G) | Probabilistic | Relative energy norm | Deterministic A-norm | V-cycle: sigma<1 |
| **Memory footprint** | O(nnz + n) | O(nonzero PPR entries) | O(nonzero PPR entries) | O(n + num_walks) | O(n*log(n)/eps^2) | O(nnz + 4n) | O(nnz*log(n)) |
| **Condition sensitivity** | High (diverges if rho>=1) | None (topology only) | None (topology only) | None (topology only) | Low (sparsification) | High (sqrt(kappa) iters) | Low (multigrid) |
| **Composability** | Nestable | Chainable (multi-hop) | Chainable (multi-hop) | Terminal (point query) | Preprocessing reusable | Preconditioner swappable | Hierarchy reusable |

---

### Crossover Points

The following analysis determines the exact n and kappa values where each algorithm becomes
faster than alternatives. Constant factors are calibrated from RuVector's published benchmark
results (doc 08) on Apple M4 Pro (NEON) and Linux/AVX2.

#### Constant Factor Calibration

From RuVector benchmarks:

```
c_spmv   = 2.5 ns per nonzero  (AVX2 SpMV, from prime-radiant SIMD benchmarks)
c_spmv_n = 3.5 ns per nonzero  (NEON SpMV, extrapolated from distance benchmarks)
c_push   = 15  ns per push op  (graph traversal, from HNSW benchmark overhead)
c_walk   = 50  ns per RW step  (includes PRNG + graph access, from ruvector-core)
c_jl     = 1.5 ns per f32 mul  (from dot product: 12 ns / 8 dims)
c_alloc  = 20  ns per arena alloc (from bench_memory.rs)
```

#### Crossover 1: Neumann Series vs Conjugate Gradient

Both solve Ax = b for sparse SPD A. Wall-time models:

```
T_neumann(n) = k_neumann * nnz * c_spmv
  where k_neumann = ceil( log(1/eps) / log(1/rho) )
  and rho = 1 - 1/kappa  (for regularized Laplacian with shift delta ~ lambda_min)

T_cg(n) = k_cg * nnz * c_spmv
  where k_cg = ceil( sqrt(kappa) * log(2/eps) )
```

Setting T_neumann = T_cg and simplifying (nnz and c_spmv cancel):

```
log(1/eps) / log(1/(1-1/kappa)) = sqrt(kappa) * log(2/eps)
```

For kappa >> 1, using log(1/(1-x)) ~ x for small x:

```
log(1/eps) / (1/kappa) ~ sqrt(kappa) * log(2/eps)
kappa * log(1/eps) ~ sqrt(kappa) * log(2/eps)
sqrt(kappa) ~ log(2/eps) / log(1/eps) ~ 1
kappa ~ 1
```

This means Neumann iteration count grows as O(kappa * log(1/eps)), while CG grows as
O(sqrt(kappa) * log(1/eps)). CG dominates for kappa > ~4.

Concrete comparison at eps = 1e-6:

| kappa | k_neumann | k_cg | Winner |
|-------|----------|------|--------|
| 2 | 20 | 20 | Tie |
| 4 | 55 | 28 | CG (2.0x) |
| 10 | 138 | 44 | CG (3.1x) |
| 100 | 1,382 | 138 | CG (10x) |
| 1,000 | 13,816 | 437 | CG (31.6x) |

**Router threshold**: Use Neumann only when kappa_estimate < 5 AND diagonal_dominance > 0.5.
For graph Laplacians, expander graphs have kappa ~ O(1), making Neumann competitive.

#### Crossover 2: CG vs BMSSP

```
T_cg = sqrt(kappa) * log(1/eps) * nnz * c_spmv
T_bmssp = C_mg * nnz * log(1/eps) * c_spmv + T_coarsen
  where C_mg ~ 5 (multigrid cycle overhead: 2 smoothing sweeps + restriction + prolongation)
  and T_coarsen = nnz * log(n) * c_spmv (one-time hierarchy construction)
```

Ignoring preprocessing (batch mode or amortized):

```
sqrt(kappa) > C_mg = 5
kappa > 25
```

Including preprocessing over B solves:

```
sqrt(kappa) * nnz * c > C_mg * nnz * c + (nnz * log(n) * c) / B
sqrt(kappa) > 5 + log(n) / (B * log(1/eps))
```

For n = 100K, eps = 1e-6, B = 10:

```
sqrt(kappa) > 5 + 17 / (10 * 14) = 5.12
kappa > 26.2
```

**Router threshold**: Use BMSSP when kappa > 25 (single solve, n > 50K) or
kappa > 25 + log(n) / B (batch mode).

#### Crossover 3: CG vs TRUE (Batch Mode)

TRUE has heavy preprocessing but very fast per-solve amortized cost:

```
T_true_prep = m * log(n) / eps^2 * c_spmv    (sparsification + JL)
T_true_solve = log^2(n) * n / eps^2 * c_jl   (per-solve, JL-dominated)
T_cg_solve = sqrt(kappa) * log(1/eps) * nnz * c_spmv  (per-solve)
```

TRUE wins over B solves when:

```
T_true_prep + B * T_true_solve < B * T_cg_solve
T_true_prep / B < T_cg_solve - T_true_solve
```

For n = 100K, nnz = 10n = 10^6, eps = 0.01, kappa = 1000, c_spmv = 2.5 ns, c_jl = 1.5 ns:

```
T_true_prep  = 10^6 * 17 / 0.0001 * 2.5e-9 = 425 ms
T_true_solve = 289 * 10^5 / 0.0001 * 1.5e-9 = 43 ms
T_cg_solve   = 31.6 * 14 * 10^6 * 2.5e-9 = 1.1 ms
```

TRUE per-solve (43 ms) is 39x slower than CG (1.1 ms) at this configuration. TRUE only
becomes viable when the preprocessing is amortized over a very large batch:

```
425 ms / B + 43 ms < 1.1 ms  =>  impossible (43 ms > 1.1 ms)
```

TRUE per-solve dominates CG at this scale. TRUE becomes practical only for n >> 10^6
or when eps is relaxed to ~0.1 (reducing JL target dimension dramatically):

At eps = 0.1: T_true_solve = 289 * 10^5 / 0.01 * 1.5e-9 = 0.43 ms.
Now: 425 ms / B + 0.43 ms < 1.1 ms => B > 425 / 0.67 = 634.

**Router threshold**: Use TRUE when n >= 100K AND batch_mode AND num_rhs >= max(10, n/1000)
AND eps >= 0.05.

#### Crossover 4: Forward Push vs Hybrid Random Walk (Pairwise)

For pairwise PPR(s,t):

```
T_push = (1/eps) * c_push              (computes full PPR vector, reads entry t)
T_hybrid = (sqrt(n)/eps) * c_walk      (directly estimates pairwise value)
```

However, Forward Push for pairwise is wasteful because it computes the entire PPR vector
but only needs one entry. The relevant comparison accounts for the push's "wasted work":

```
T_push_effective = (1/eps) * c_push    (total, regardless of target)
T_hybrid = (sqrt(n)/eps) * c_walk
```

Crossover at T_push = T_hybrid:

```
c_push / eps = sqrt(n) * c_walk / eps
sqrt(n) = c_push / c_walk = 15 / 50 = 0.3
n = 0.09
```

This suggests push is always cheaper in raw operations. But for large graphs (n > 10^5),
the push generates O(1/eps) nonzero PPR entries that must be stored in memory, while
Hybrid RW uses O(sqrt(n)) memory. The practical crossover considers memory pressure and
cache behavior:

- For n < 1,000: Push is faster and memory is not a concern.
- For n >= 1,000: Hybrid RW has better cache locality (walks are sequential in the
  adjacency list) and provides probabilistic confidence bounds.
- For n >= 10^6: Push may exceed L3 cache with its O(1/eps) nonzero entries when eps < 10^{-4}.

**Router threshold**: Use Forward Push for pairwise when n < 1,000. Use Hybrid RW otherwise.

#### Crossover Summary Table

| Algorithm A | Algorithm B | Crossover Condition | Winner Below | Winner Above |
|------------|------------|-------------------|-------------|-------------|
| Neumann | CG | kappa ~ 4 | Neumann | CG |
| CG | BMSSP | kappa ~ 25 (n > 50K) | CG | BMSSP |
| CG | TRUE (batch) | n*B ~ 10^7, eps >= 0.05 | CG | TRUE |
| Forward Push | Hybrid RW | n ~ 1K (pairwise query) | Push | Hybrid RW |
| CG | BMSSP (batch) | kappa ~ 25 + log(n)/B | CG | BMSSP |
| Neumann | BMSSP | kappa ~ 5 (n > 50K) | Neumann | BMSSP |
| Chebyshev | Neumann | alpha ~ 0.01 (rational) | Chebyshev | Neumann |

---

### Error Budget Decomposition

When multiple sublinear algorithms compose in a RuVector pipeline, the total approximation
error is bounded by the sum of individual component errors.

#### Error Accumulation Model

For additive error components (independent approximations):

```
eps_total <= eps_quantization + eps_jl + eps_sparsify + eps_solver + eps_push
```

For multiplicative error components (chained distance-preserving transformations):

```
(1 + eps_total) <= (1 + eps_jl) * (1 + eps_sparsify) * (1 + eps_solver)
```

For small eps (< 0.1), the multiplicative model approximates to:

```
eps_total ~ eps_jl + eps_sparsify + eps_solver + O(eps^2)
```

We use the additive model with conservative bounds throughout.

#### Default Budget Allocation (eps_total = 0.1)

| Component | Symbol | Budget | Fraction | Rationale |
|-----------|--------|--------|----------|-----------|
| Quantization | eps_q | 0.030 | 30% | Scalar u8: error per dim ~ range/255. For normalized [-1,1]: 2/255 ~ 0.0078/dim. Over d dims with sqrt cancellation: 0.0078*sqrt(d). At d=128: 0.088 (within budget). At d=384: 0.153 (exceeds; use f32 inputs to solver). |
| JL Projection | eps_jl | 0.020 | 20% | Target dim k = ceil(24*ln(n)/eps_jl^2). For n=1M, eps_jl=0.02: k=840K (impractical). JL is practical only when eps_jl >= 0.1 (k=3360 for n=1M). Reallocate when JL absent. |
| Sparsification | eps_s | 0.020 | 20% | Benczur-Karger: O(n*log(n)/eps_s^2) edges. For n=100K, eps_s=0.02: 4.25e9 edges (too many). Practical: eps_s >= 0.05 yields 6.8e8 edges. Adjust per problem. |
| Solver Residual | eps_r | 0.020 | 20% | CG: \|\|r\|\|/\|\|b\|\| < eps_r. Neumann: rho^k < eps_r. BMSSP: sigma^k < eps_r. Cheapest to improve (logarithmic in 1/eps_r). |
| Push Approx | eps_p | 0.010 | 10% | Forward/Backward Push: \|\|pi - pi_approx\|\|_1 < eps_p*vol(G). For search: PPR rank errors at eps_p=0.01 are negligible for top-k retrieval. |

#### Adaptive Budget Reallocation

Not all components are active in every pipeline. The router reallocates unused budget
proportionally:

```rust
pub fn allocate_error_budget(
    eps_total: f64,
    active_components: &[ErrorComponent],
) -> HashMap<ErrorComponent, f64> {
    let base_weights: HashMap<ErrorComponent, f64> = [
        (ErrorComponent::Quantization,   0.30),
        (ErrorComponent::JLProjection,   0.20),
        (ErrorComponent::Sparsification, 0.20),
        (ErrorComponent::SolverResidual, 0.20),
        (ErrorComponent::PushApprox,     0.10),
    ].into_iter().collect();

    let active_weight_sum: f64 = active_components.iter()
        .filter_map(|c| base_weights.get(c))
        .sum();

    active_components.iter()
        .filter_map(|c| {
            base_weights.get(c).map(|w| (*c, eps_total * w / active_weight_sum))
        })
        .collect()
}
```

Example allocations for common pipelines:

**Pipeline: CG-only linear solve** (active: SolverResidual)

```
eps_solver = eps_total = 0.1
```

**Pipeline: Forward Push for hybrid search** (active: Quantization, PushApprox)

```
eps_quantization = 0.1 * 0.30 / 0.40 = 0.075
eps_push         = 0.1 * 0.10 / 0.40 = 0.025
```

**Pipeline: TRUE for batch spectral clustering** (active: JL, Sparsification, Solver)

```
eps_jl       = 0.1 * 0.20 / 0.60 = 0.0333
eps_sparsify = 0.1 * 0.20 / 0.60 = 0.0333
eps_solver   = 0.1 * 0.20 / 0.60 = 0.0333
```

**Pipeline: Full stack** (Quantized input -> JL -> Sparsify -> Neumann -> Push)

```
eps_q = 0.030, eps_jl = 0.020, eps_s = 0.020, eps_r = 0.020, eps_p = 0.010
Total = 0.100
```

#### Precision Requirements by RuVector Use Case

| Use Case | Required eps | Recommended Algorithm | Justification |
|----------|-------------|----------------------|---------------|
| k-NN vector search | 0.1 | Forward Push + quantized dist | Top-k robust to 10% distance error |
| Spectral clustering | 0.05 | CG + diagonal preconditioner | Eigenvector sign determines partition |
| GNN attention weights | 0.01 | CG or Neumann | Softmax amplifies small errors |
| Optimal transport plan | 0.001 | CG (high precision) | Marginal constraints are strict |
| Min-cut value | 0.01 | Sparsification + exact | Cut value used for structural decisions |
| Natural gradient (FIM) | 0.1 | Diagonal approx or CG | FIM is inherently ill-conditioned |

#### Post-Solve Error Verification

```rust
pub struct ErrorAudit {
    pub component: ErrorComponent,
    pub budget: f64,
    pub actual: f64,
    pub within_budget: bool,
}

pub fn audit_error_budget(
    budgets: &HashMap<ErrorComponent, f64>,
    actuals: &HashMap<ErrorComponent, f64>,
) -> Vec<ErrorAudit> {
    budgets.iter().map(|(component, budget)| {
        let actual = actuals.get(component).copied().unwrap_or(0.0);
        ErrorAudit {
            component: *component,
            budget: *budget,
            actual,
            within_budget: actual <= budget * 1.1, // 10% slack
        }
    }).collect()
}
```

If any component exceeds its budget by more than 10%, the router logs a warning and
the SONA feedback loop penalizes the algorithm selection for that problem profile.

---

## Consequences

### Positive

1. **Automatic optimization**: Consumers (ruvector-math, ruvector-graph, ruvector-attention,
   ruvector-mincut, ruvector-gnn) call a single `route()` function instead of manually
   selecting algorithms. This eliminates duplicated selection logic across 10+ crates.

2. **Platform safety**: Compile-time Tier 1 rules make it impossible to select
   memory-exceeding algorithms on WASM targets. Prevents runtime OOM in browsers.

3. **Quantified crossover points**: The crossover analysis provides concrete thresholds
   (kappa < 4 for Neumann, kappa > 25 for BMSSP, n >= 100K + batch for TRUE) validated
   against benchmark-calibrated constant factors.

4. **Error budget composability**: Adaptive error allocation ensures multi-algorithm
   pipelines maintain end-to-end accuracy guarantees without manual per-component tuning.

5. **Continuous improvement**: SONA adaptive learning improves routing over time. EWC
   prevents catastrophic forgetting during workload shifts.

6. **Latency predictability**: Tier 2 heuristic executes in <1 ms with zero heap allocation,
   making routing overhead negligible relative to solve times (10 us - 10 ms).

7. **Batch optimization**: TRUE amortization analysis provides clear break-even criteria
   (num_rhs >= max(10, n/1000), eps >= 0.05) for when expensive preprocessing is justified.

### Negative

1. **Implementation complexity**: Three routing tiers add code. SONA adaptive tier requires
   training data collection, EWC checkpoint management, and model validation.

2. **Threshold brittleness**: Tier 2 thresholds (kappa < 4, kappa > 25, n >= 100K) are
   calibrated on AVX2 at c_spmv = 2.5 ns. NEON and WASM have different constants, requiring
   per-platform threshold tuning or dynamic calibration.

3. **Condition number estimation cost**: Several decisions depend on kappa_estimate. If the
   caller does not provide it, the router must use a default (risking suboptimal selection)
   or estimate it at O(40 * nnz) cost (~100 us for nnz = 10^6). See Appendix B.

4. **Error budget conservatism**: The additive error model is conservative; in practice
   errors may partially cancel. This means allocated budgets are slightly tighter than
   necessary, leading to marginally more computation.

5. **SONA cold start**: ~10,000 outcomes needed before adaptive overrides are reliable.
   During cold start, the system operates as a two-tier (static + heuristic) router.

6. **Testing surface**: 7 algorithms x 8 query types x 5 platforms = 280 configurations.
   Exhaustive testing is infeasible; sampling-based CI validation is required.

### Neutral

1. **Fallback degradation**: When the preferred algorithm is excluded by Tier 1, the router
   selects the best available alternative. This degrades gracefully but may produce
   unexpected performance characteristics for platform-constrained targets.

2. **Chebyshev path preserved**: The router returns `NoRoute` for polynomial spectral
   filters, preserving the existing ruvector-math Chebyshev infrastructure unchanged.

3. **SONA memory**: < 1 KB total (280 bytes weights + 280 bytes Fisher diagonal). Negligible.

---

## Options Considered

### Option 1: Single-Tier Static Dispatch (Rejected)

Map each RuVector subsystem to a fixed algorithm at compile time:
- ruvector-graph -> Forward Push
- ruvector-attention -> CG
- ruvector-math/spectral -> Neumann
- ruvector-mincut -> BMSSP

**Pros**:
- Zero runtime overhead.
- Simple implementation, easy to test.

**Cons**:
- No adaptation to problem characteristics. A 100-node graph gets the same algorithm as
  a 10M-node graph.
- No error budget management across composed components.
- Each subsystem locked to one algorithm regardless of query type.

**Rejected**: The problem-characteristic space is too varied (n from 100 to 10M,
kappa from 1 to 10^6). A single algorithm cannot be optimal across this range.

### Option 2: Per-Call Manual Selection (Rejected)

Expose all seven algorithms directly; each caller selects explicitly:

```rust
solver.solve_with(Algorithm::ConjugateGradient, input, eps)
```

**Pros**:
- Maximum flexibility. No routing overhead.

**Cons**:
- Duplicates selection logic across every call site (10+ crates).
- Requires every caller to understand crossover analysis and numerical tradeoffs.
- No centralized error budget management.

**Rejected**: Violates DRY. Algorithm selection expertise should be centralized.

### Option 3: Two-Tier Without Adaptive Learning (Accepted as Phase 1)

Implement only Tier 1 (static rules) and Tier 2 (heuristic router), deferring SONA.

**Pros**:
- Simpler implementation. Fully deterministic. No cold-start problem.
- Easier to debug and validate.

**Cons**:
- Cannot adapt to hardware-specific performance characteristics.
- Cannot improve as workload patterns emerge.
- Heuristic thresholds may become stale as hardware evolves.

**Accepted as Phase 1**: The two-tier system is the initial implementation. SONA Tier 3
is added in Phase 2 after the heuristic router is validated in production. This ADR
documents all three tiers as the target architecture.

---

## Compliance

- **ADR-STS-001**: Routing integrates within the SolverEngine trait hierarchy
- **ADR-STS-007**: Feature flags control per-platform algorithm availability
- **ADR-STS-008**: Fallback chain (sublinear -> CG -> dense) triggered by routing failures
- **ADR-STS-009**: Parallel dispatch of solver operations via Rayon (feature-gated)
- **ADR-STS-010**: Router exposed through the SolverEngine API surface

---

## Related Decisions

- **ADR-STS-001**: Core Integration Architecture (trait hierarchy, crate structure)
- **ADR-002**: Modular DDD Architecture (bounded context separation)
- **ADR-004**: MCP Transport Optimization (solver routing exposed via MCP tools)
- **ADR-006**: Unified Memory Service (SONA model + cache stored in memory service)
- **ADR-008**: Neural Learning Integration (SONA framework for Tier 3)
- **ADR-009**: Hybrid Memory Backend (HNSW search for similar routing queries in SONA)
- **ADR-026**: 3-Tier Model Routing (solver tiers mirror agent model tiers)

---

## References

1. `/home/user/ruvector/docs/research/sublinear-time-solver/10-algorithm-analysis.md` -- Full mathematical analysis of all seven algorithms, convergence guarantees, error bounds, and RuVector use-case mappings.

2. `/home/user/ruvector/docs/research/sublinear-time-solver/08-performance-analysis.md` -- Benchmark infrastructure, SIMD acceleration, memory efficiency, crossover projections.

3. `/home/user/ruvector/docs/research/sublinear-time-solver/05-architecture-analysis.md` -- Layered integration strategy, module boundaries, event-driven patterns.

4. Andersen, R., Chung, F., Lang, K. (2006). "Local Graph Partitioning using PageRank Vectors." FOCS 2006.

5. Lofgren, P., Banerjee, S., Goel, A., Seshadhri, C. (2014). "FAST-PPR: Scaling Personalized PageRank Estimation for Large Graphs." KDD 2014.

6. Spielman, D., Teng, S.-H. (2014). "Nearly Linear Time Algorithms for Preconditioning and Solving Symmetric, Diagonally Dominant Linear Systems." SIAM J. Matrix Anal. Appl.

7. Koutis, I., Miller, G.L., Peng, R. (2011). "A Nearly-m*log(n) Time Solver for SDD Linear Systems." FOCS 2011.

8. Hestenes, M.R., Stiefel, E. (1952). "Methods of Conjugate Gradients for Solving Linear Systems." J. Res. Nat. Bur. Standards.

9. Johnson, W.B., Lindenstrauss, J. (1984). "Extensions of Lipschitz mappings into a Hilbert space." Contemporary Mathematics.

10. Kirkpatrick, J., et al. (2017). "Overcoming catastrophic forgetting in neural networks." PNAS.

---

## Implementation Status

Algorithm router implemented with crossover analysis: Neumann for diag-dominant (fastest for well-conditioned), CG as gold-standard SPD fallback, Forward/Backward Push for PageRank, TRUE for large-scale Laplacian, BMSSP for multigrid. Router uses matrix characterization (size, density, diagonal dominance, symmetry) for automatic algorithm selection.

---

## Appendix A: Router Configuration Schema

```toml
[solver.router]
# Tier 1: Platform (auto-detected, overridable)
platform = "auto"  # "native", "wasm-browser", "wasm-edge", "auto"

# Tier 2: Heuristic thresholds
[solver.router.thresholds]
neumann_max_kappa = 4.0
neumann_min_diagonal_dominance = 0.5
bmssp_min_kappa = 25.0
bmssp_min_n = 50_000
true_min_n = 100_000
true_min_batch_size = 10
true_min_eps = 0.05
hybrid_rw_min_n = 1_000
cg_default_preconditioner = "diagonal"
spectral_cluster_bmssp_min_n = 10_000
spectral_cluster_true_min_n = 100_000
neumann_filter_min_alpha = 0.01
small_n_threshold = 500

# Tier 3: SONA adaptive learning
[solver.router.sona]
enabled = false           # Enable after Phase 1 validation
confidence_threshold = 0.8
ewc_lambda = 100.0
ewc_checkpoint_interval = 1000
learning_rate = 0.001
feature_dim = 10
cold_start_outcomes = 10_000
exploration_rate_initial = 0.1
exploration_decay = 1000.0
```

## Appendix B: Condition Number Estimation

When kappa_estimate is not provided, the router estimates it using power iteration:

```
Algorithm: Estimate kappa(A) for SPD matrix A

1. lambda_max via 20 power iterations:
   v = random unit vector
   for i in 1..20:
     v = A * v / ||A * v||
   lambda_max_est = v^T * A * v

2. lambda_min via shifted inverse iteration:
   Use trace-based estimate:
   lambda_min_est = trace(A)/n - sqrt( trace(A^2)/n - (trace(A)/n)^2 )
   (Requires one SpMV for trace(A^2) = sum of squared row norms)

3. kappa_est = lambda_max_est / max(lambda_min_est, 1e-15)

Cost: ~22 SpMV = O(22 * nnz).
At nnz = 10^6, c_spmv = 2.5 ns: ~55 us (acceptable overhead).
```

The router caches kappa estimates per matrix fingerprint (hash of dimensions + first 64
nonzero values) to avoid recomputation on repeated calls with the same matrix.

## Appendix C: Platform Detection

```rust
pub fn detect_platform() -> Platform {
    #[cfg(target_arch = "wasm32")]
    {
        let pages = core::arch::wasm32::memory_size(0);
        let bytes = pages * 65536;
        if bytes < 32 * 1024 * 1024 {
            Platform::WasmBrowser
        } else {
            Platform::WasmEdge
        }
    }

    #[cfg(all(not(target_arch = "wasm32"), target_arch = "x86_64"))]
    {
        if is_x86_feature_detected!("avx512f") {
            Platform::NativeAVX512
        } else if is_x86_feature_detected!("avx2") {
            Platform::NativeAVX2
        } else {
            Platform::NativeScalar
        }
    }

    #[cfg(all(not(target_arch = "wasm32"), target_arch = "aarch64"))]
    {
        Platform::NativeNEON
    }
}
```

## Appendix D: Notation Reference

| Symbol | Meaning |
|--------|---------|
| n | Matrix dimension or graph vertex count |
| m | Edge count (m = nnz/2 for symmetric graphs) |
| nnz | Number of nonzero entries in sparse matrix |
| d | Vector dimensionality |
| kappa | Condition number: lambda_max / lambda_min |
| eps | Target approximation accuracy |
| rho | Spectral radius of iteration matrix D^{-1}B |
| k | Number of iterations, clusters, or neighbors (context-dependent) |
| alpha | PPR teleportation probability (typically 0.15) or filter parameter |
| vol(G) | Volume of graph: sum of all vertex degrees = 2m |
| L | Graph Laplacian: L = D - A |
| D | Degree diagonal matrix |
| A | Adjacency matrix |
| sigma | Multigrid V-cycle convergence factor (< 1 for convergence) |
| delta | Failure probability for probabilistic algorithms |
| C_mg | Multigrid cycle overhead constant (~5 for typical AMG) |
| c_spmv | Nanoseconds per nonzero for sparse matrix-vector multiply |
| c_push | Nanoseconds per push operation in Forward/Backward Push |
| c_walk | Nanoseconds per random walk step (includes PRNG) |
| c_jl | Nanoseconds per f32 multiply in JL projection |
| F_i | Fisher Information Matrix diagonal entry (EWC) |
| W | SONA routing weight matrix, shape [features x algorithms] |
| B | Batch size (number of right-hand sides for batch solves) |
