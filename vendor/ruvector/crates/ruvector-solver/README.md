# ruvector-solver

[![Crates.io](https://img.shields.io/crates/v/ruvector-solver.svg)](https://crates.io/crates/ruvector-solver)
[![docs.rs](https://docs.rs/ruvector-solver/badge.svg)](https://docs.rs/ruvector-solver)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Tests](https://img.shields.io/badge/tests-177_passing-brightgreen.svg)]()

**Sublinear-time sparse solvers -- O(log n) PageRank, spectral methods, and linear systems in Rust and WASM.**

Most numerical libraries use dense solvers that slow down dramatically as data grows. ruvector-solver provides seven specialized sparse algorithms that run in O(log n) to O(sqrt(n)) time, automatically picks the best one for your problem, and works in native Rust or in the browser via WebAssembly. It powers the graph analytics and AI coherence layers inside [RuVector](https://github.com/ruvnet/ruvector).

| | Dense Solvers (e.g. nalgebra) | ruvector-solver |
|---|---|---|
| **Speed at scale** | O(n^3) -- slows fast | O(nnz * log n) to O(log n) -- stays fast |
| **Memory** | Stores full n*n matrix | Only stores non-zero entries (CSR) |
| **SIMD acceleration** | Partial | AVX2 8-wide + fused kernels |
| **Algorithm selection** | Manual | Automatic router picks the best of 7 |
| **PageRank** | Not available | 3 sublinear algorithms built in |
| **Browser / WASM** | No | Full wasm-bindgen bindings |

## Quick Start

```toml
[dependencies]
ruvector-solver = "0.1"
```

```rust
use ruvector_solver::types::CsrMatrix;
use ruvector_solver::neumann::NeumannSolver;

// Build a sparse 3x3 system and solve it
let a = CsrMatrix::<f32>::from_coo(3, 3, vec![
    (0, 0, 2.0_f32), (0, 1, -0.5),
    (1, 0, -0.5),    (1, 1, 2.0),  (1, 2, -0.5),
    (2, 1, -0.5),    (2, 2, 2.0),
]);
let b = vec![1.0_f32, 0.0, 1.0];

let solver = NeumannSolver::new(1e-6, 500);
let result = solver.solve(&a, &b).unwrap();

println!("solution: {:?}", result.solution);
println!("iterations: {}", result.iterations);
println!("residual:   {:.2e}", result.residual_norm);
```

Or let the router pick the best algorithm automatically:

```rust
use ruvector_solver::router::{SolverRouter, QueryType};
use ruvector_solver::types::{CsrMatrix, ComputeBudget};

let router = SolverRouter::new();
let (algo, result) = router.solve(&matrix, &rhs, &ComputeBudget::default(), QueryType::LinearSystem).unwrap();
println!("Router selected: {:?}", algo);
```

## Key Features

| Feature | What It Does | Why It Matters |
|---------|-------------|----------------|
| **7 specialized algorithms** | Neumann, CG, Forward Push, Backward Push, Random Walk, TRUE, BMSSP | Each tuned for a specific problem class |
| **Automatic routing** | SolverRouter analyzes matrix structure and picks the optimal algorithm | No need to be a numerical methods expert |
| **Fallback chain** | If the selected algorithm fails, tries CG, then dense | Robust convergence in production |
| **AVX2 SIMD SpMV** | 8-wide vectorized sparse matrix-vector multiply | Maximizes throughput on x86_64 |
| **Fused residual + norm** | Computes residual and norm in one pass instead of three | 3x less memory traffic per iteration |
| **Arena allocator** | Bump allocation for scratch buffers, O(1) reset | Zero heap allocation inside solve loops |
| **WASM support** | Full wasm-bindgen bindings | Run solvers in the browser |
| **ComputeBudget** | Set max time, iterations, and tolerance | Predictable resource usage |

## Algorithms

| Algorithm | Module | Complexity | Applicable to |
|-----------|--------|------------|---------------|
| Jacobi-preconditioned Neumann series | `neumann` | O(nnz * log(1/eps)) | Diagonally dominant Ax = b |
| Conjugate Gradient (Hestenes-Stiefel) | `cg` | O(nnz * sqrt(kappa)) | Symmetric positive-definite Ax = b |
| Forward Push (Andersen-Chung-Lang) | `forward_push` | O(1/epsilon) | Single-source Personalized PageRank |
| Backward Push | `backward_push` | O(1/epsilon) | Reverse relevance / target-centric PPR |
| Hybrid Random Walk | `random_walk` | O(sqrt(n)/epsilon) | Large-graph PPR with push initialisation |
| TRUE (JL + sparsification + Neumann) | `true_solver` | O(nnz * log n) | Batch linear systems with shared A |
| BMSSP Multigrid (V-cycle + Jacobi) | `bmssp` | O(n log n) | Ill-conditioned / graph Laplacian systems |

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `neumann` | Yes | Jacobi-preconditioned Neumann series solver |
| `cg` | Yes | Conjugate Gradient (Hestenes-Stiefel) solver |
| `forward-push` | Yes | Forward push for single-source PPR |
| `backward-push` | No | Backward push for reverse relevance computation |
| `hybrid-random-walk` | No | Hybrid random walk with push initialisation (enables `getrandom`) |
| `true-solver` | No | TRUE batch solver (implies `neumann`) |
| `bmssp` | No | BMSSP multigrid solver (V-cycle with Jacobi smoothing) |
| `all-algorithms` | No | Enables every algorithm above |
| `simd` | No | AVX2 SIMD-accelerated SpMV (x86_64 only) |
| `wasm` | No | WebAssembly target support |
| `parallel` | No | Multi-threaded SpMV and solver loops (enables `rayon`, `crossbeam`) |
| `full` | No | All algorithms + `parallel` + `nalgebra-backend` |

Enable all algorithms:

```toml
[dependencies]
ruvector-solver = { version = "0.1", features = ["all-algorithms"] }
```

## Performance Optimisations

### Bounds-check-free SpMV (`spmv_unchecked`)

The inner SpMV loop is the single hottest path in every iterative solver. The
`spmv_unchecked` method on `CsrMatrix<f32>` and `CsrMatrix<f64>` uses raw
pointers to eliminate per-element bounds checks, relying on a one-time CSR
structural validation (`validation::validate_csr_matrix`) performed before
entering the solve loop.

### Fused residual + norm computation (`fused_residual_norm_sq`)

Standard implementations compute the residual `r = b - Ax` and its squared norm
`||r||^2` in separate passes (SpMV, vector subtraction, dot product -- three
full memory traversals). `fused_residual_norm_sq` computes both in a single
pass, reducing memory traffic by roughly 3x per iteration.

### AVX2 8-wide SIMD SpMV

When the `simd` feature is enabled on x86_64, `spmv_simd` dispatches to an
AVX2 kernel that processes 8 `f32` values per instruction using `_mm256`
intrinsics with a horizontal sum reduction at the end of each row. Falls back
to a portable scalar loop on other architectures.

### 4-wide unrolled Jacobi update

The Neumann iteration's update step `x[j] += d_inv[j] * r[j]` is manually
unrolled 4-wide for instruction-level parallelism, with a scalar remainder loop
for dimensions not divisible by 4.

### Arena allocator

`SolverArena` provides bump allocation for per-solve scratch buffers. All
temporary vectors are allocated from a single contiguous backing buffer and
reclaimed in O(1) via `arena.reset()`, eliminating heap allocation overhead
inside the iteration loop.

## Architecture

```text
                          +-------------------+
                          |   SolverRouter    |
                          | (algorithm select)|
                          +--------+----------+
                                   |
            +----------+-----------+-----------+----------+
            |          |           |           |          |
       +----v---+ +----v---+ +----v------+ +--v----+ +---v----+
       |Neumann | |   CG   | |ForwardPush| | TRUE  | | BMSSP  |
       |Solver  | | Solver | |  Solver   | |Solver | |Solver  |
       +----+---+ +----+---+ +-----+-----+ +--+----+ +---+----+
            |          |            |          |          |
            +-----+----+-----+-----+-----+----+-----+---+
                  |          |           |           |
             +----v---+ +---v----+ +----v----+ +----v-----+
             |types.rs| |simd.rs | |arena.rs | |budget.rs |
             |CsrMatrix| |AVX2   | |Bump     | |ComputeBudget|
             +--------+ |SpMV   | |Alloc    | |enforcement|
                         +-------+ +--------+ +----------+
                  |          |           |
             +----v---+ +---v------+ +--v---------+
             |traits.rs| |validate.rs| |error.rs    |
             |SolverEngine| |CSR check| |SolverError |
             +--------+ +---------+ +-----------+
```

The `SolverRouter` analyses the matrix `SparsityProfile` and `QueryType`
to select the optimal algorithm. When the selected algorithm fails,
`SolverOrchestrator::solve_with_fallback` tries a deterministic fallback
chain: **selected -> CG -> Dense**.

## API Overview

### Core types (`types.rs`)

| Type | Description |
|------|-------------|
| `CsrMatrix<T>` | Compressed Sparse Row matrix with `spmv`, `spmv_unchecked`, `from_coo`, `transpose` |
| `SolverResult` | Solution vector, iteration count, residual norm, wall time, convergence history |
| `ComputeBudget` | Maximum time, max iterations, target tolerance |
| `Algorithm` | Enum of all solver algorithms (Neumann, CG, ForwardPush, ...) |
| `SparsityProfile` | Matrix structural analysis (density, diagonal dominance, spectral radius estimate) |
| `QueryType` | What the caller wants to solve (LinearSystem, PageRankSingle, Batch, ...) |
| `ComplexityEstimate` | Predicted flops, iterations, memory, and complexity class |

### Traits (`traits.rs`)

| Trait | Description |
|-------|-------------|
| `SolverEngine` | Core trait: `solve(matrix, rhs, budget) -> SolverResult` |
| `SparseLaplacianSolver` | Extension for graph Laplacian systems and effective resistance |
| `SublinearPageRank` | Extension for sublinear PPR: `ppr(matrix, source, alpha, epsilon)` |

### Error hierarchy (`error.rs`)

| Error | Cause |
|-------|-------|
| `SolverError::NonConvergence` | Iteration budget exhausted without reaching tolerance |
| `SolverError::NumericalInstability` | NaN/Inf or residual growth > 2x detected |
| `SolverError::SpectralRadiusExceeded` | Spectral radius >= 1.0 (Neumann series would diverge) |
| `SolverError::BudgetExhausted` | Wall-clock time limit exceeded |
| `SolverError::InvalidInput` | Dimension mismatch, non-finite values, index out of bounds |
| `SolverError::BackendError` | Backend-specific failure (nalgebra, BLAS) |

### Infrastructure modules

| Module | Description |
|--------|-------------|
| `router.rs` | `SolverRouter` for automatic algorithm selection; `SolverOrchestrator` with fallback |
| `simd.rs` | AVX2-accelerated SpMV with runtime feature detection |
| `validation.rs` | CSR structural validation (index bounds, monotonic row_ptr, NaN/Inf) |
| `arena.rs` | `SolverArena` bump allocator for zero per-iteration heap allocation |
| `budget.rs` | `ComputeBudget` enforcement during solve |
| `audit.rs` | Audit logging for solver invocations |
| `events.rs` | Event system for solver lifecycle hooks |

## Testing

The crate includes **177 tests** (138 unit tests + 39 integration/doctests):

```bash
# Run all tests
cargo test -p ruvector-solver

# Run tests with all algorithms enabled
cargo test -p ruvector-solver --features all-algorithms

# Run a specific test module
cargo test -p ruvector-solver -- neumann::tests
```

### Benchmarks

Five Criterion benchmark groups are provided:

```bash
# Run all benchmarks
cargo bench -p ruvector-solver

# Run a specific benchmark
cargo bench -p ruvector-solver --bench solver_neumann
```

| Benchmark | Description |
|-----------|-------------|
| `solver_baseline` | Baseline SpMV and vector operations |
| `solver_neumann` | Neumann solver convergence on tridiagonal systems |
| `solver_cg` | Conjugate Gradient on SPD matrices |
| `solver_push` | Forward/backward push on graph adjacency matrices |
| `solver_e2e` | End-to-end solve through the router with algorithm selection |

<details>
<summary><strong>Tutorial: Solving a Sparse Linear System</strong></summary>

### Step 1: Build a CSR matrix

```rust
use ruvector_solver::types::CsrMatrix;

// 4x4 tridiagonal system (diagonally dominant)
let a = CsrMatrix::<f32>::from_coo(4, 4, vec![
    (0, 0, 3.0), (0, 1, -1.0),
    (1, 0, -1.0), (1, 1, 3.0), (1, 2, -1.0),
    (2, 1, -1.0), (2, 2, 3.0), (2, 3, -1.0),
    (3, 2, -1.0), (3, 3, 3.0),
]);
let b = vec![2.0f32, 1.0, 1.0, 2.0];
```

### Step 2: Choose a solver

```rust
use ruvector_solver::neumann::NeumannSolver;

let solver = NeumannSolver::new(1e-6, 500);
let result = solver.solve(&a, &b).unwrap();

println!("Solution:   {:?}", result.solution);
println!("Iterations: {}", result.iterations);
println!("Residual:   {:.2e}", result.residual_norm);
```

### Step 3: Use the automatic router

```rust
use ruvector_solver::router::{SolverRouter, QueryType};
use ruvector_solver::types::{CsrMatrix, ComputeBudget};

let a64 = CsrMatrix::<f64>::from_coo(4, 4, vec![/* ... */]);
let b64 = vec![2.0, 1.0, 1.0, 2.0];
let budget = ComputeBudget::default();

let router = SolverRouter::new();
let (algo, result) = router.solve(&a64, &b64, &budget, QueryType::LinearSystem).unwrap();
println!("Router selected: {:?}", algo);
```

### Step 4: Validate input

```rust
use ruvector_solver::validation::validate_csr_matrix;

let errors = validate_csr_matrix(&a);
assert!(errors.is_empty(), "CSR validation failed: {:?}", errors);
```

### Step 5: Benchmark

```bash
cargo bench -p ruvector-solver --bench solver_neumann
cargo bench -p ruvector-solver --bench solver_e2e
```

</details>

<details>
<summary><strong>Tutorial: PageRank with Forward Push</strong></summary>

```rust
use ruvector_solver::forward_push::ForwardPushSolver;
use ruvector_solver::types::CsrMatrix;

// Build adjacency matrix for a small graph
let adj = CsrMatrix::<f32>::from_coo(4, 4, vec![
    (0, 1, 1.0), (1, 0, 1.0),
    (1, 2, 1.0), (2, 1, 1.0),
    (2, 3, 1.0), (3, 2, 1.0),
    (0, 3, 1.0), (3, 0, 1.0),
]);

let solver = ForwardPushSolver::new(0.85, 1e-6);  // alpha=0.85
let ppr = solver.ppr(&adj, 0);  // PPR from node 0

println!("PPR scores: {:?}", ppr);
```

</details>

## Related Crates

| Crate | Role |
|-------|------|
| [`ruvector-attn-mincut`](../ruvector-attn-mincut/README.md) | Min-cut gating using graph solvers |
| [`ruvector-coherence`](../ruvector-coherence/README.md) | Coherence metrics for attention comparison |
| [`ruvector-profiler`](../ruvector-profiler/README.md) | Benchmarking memory, power, latency |

## Minimum Supported Rust Version

Rust **1.77** or later.

## License

Licensed under the [MIT License](../../LICENSE).

---

Part of [RuVector](https://github.com/ruvnet/ruvector) -- the self-learning vector database.
