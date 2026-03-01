# ADR-STS-007: Feature Flag Architecture and Progressive Rollout

## Status

**Accepted**

## Metadata

| Field       | Value                                          |
|-------------|------------------------------------------------|
| Version     | 1.0                                            |
| Date        | 2026-02-20                                     |
| Authors     | RuVector Architecture Team                     |
| Deciders    | Architecture Review Board                      |
| Supersedes  | N/A                                            |
| Related     | ADR-STS-001 (Solver Integration), ADR-STS-003 (WASM Strategy) |

---

## Context

The RuVector workspace (v2.0.3, Rust 2021 edition, resolver v2) contains 100+ crates
spanning vector storage, graph databases, GNN layers, attention mechanisms, sparse
inference, and mathematics. Feature flags are already used extensively throughout the
codebase:

- **ruvector-core**: `default = ["simd", "storage", "hnsw", "api-embeddings", "parallel"]`
- **ruvector-graph**: `default = ["full"]` with `full`, `simd`, `storage`, `async-runtime`,
  `compression`, `distributed`, `federation`, `wasm`
- **ruvector-math**: `default = ["std"]` with `simd`, `parallel`, `serde`
- **ruvector-gnn**: `default = ["simd", "mmap"]` with `wasm`, `napi`
- **ruvector-attention**: `default = ["simd"]` with `wasm`, `napi`, `math`, `sheaf`

The sublinear-time-solver (v0.1.3) introduces new algorithmic capabilities --- coherence
verification, spectral graph methods, GNN-accelerated search, and sublinear query
resolution --- that must be integrated without disrupting any of these existing feature
surfaces.

### Constraints

1. **Zero breaking changes** to the public API of any existing crate.
2. **Opt-in per subsystem**: each solver capability must be individually selectable.
3. **Gradual rollout**: phased introduction from experimental to default.
4. **Platform parity**: feature gates must account for native, WASM, and Node.js targets.
5. **CI tractability**: the feature matrix must remain testable without combinatorial
   explosion.
6. **Dependency hygiene**: enabling a solver feature must not pull in nalgebra when only
   ndarray is needed, and vice versa.

---

## Decision

We adopt a **hierarchical feature flag architecture** with four tiers: the solver crate
defines its own backend and acceleration flags, consuming crates expose subsystem-scoped
`sublinear-*` flags, the workspace root provides aggregate flags for convenience, and CI
tests a curated feature matrix rather than all 2^N combinations.

### 1. Solver Crate Feature Definitions

```toml
# crates/ruvector-solver/Cargo.toml

[package]
name = "ruvector-solver"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
description = "Sublinear-time solver: coherence verification, spectral methods, GNN search"

[features]
default = []

# Linear algebra backends (mutually independent, both can be active)
nalgebra-backend = ["dep:nalgebra"]
ndarray-backend  = ["dep:ndarray"]

# Acceleration
parallel = ["dep:rayon"]
simd     = []                          # Auto-detected at build time via cfg
gpu      = ["ruvector-math/parallel"]  # Future: GPU dispatch through ruvector-math

# Platform targets
wasm = [
    "dep:wasm-bindgen",
    "dep:serde_wasm_bindgen",
    "dep:js-sys",
]

# Convenience aggregates
full = ["nalgebra-backend", "ndarray-backend", "parallel"]

[dependencies]
# Core (always present)
ruvector-math = { path = "../ruvector-math", default-features = false }
serde         = { workspace = true }
serde_json    = { workspace = true }
thiserror     = { workspace = true }
tracing       = { workspace = true }
rand          = { workspace = true }
rand_distr    = { workspace = true }

# Optional backends
nalgebra = { version = "0.33", default-features = false, features = ["std"], optional = true }
ndarray  = { workspace = true, features = ["serde"], optional = true }

# Optional acceleration
rayon = { workspace = true, optional = true }

# Optional WASM
wasm-bindgen       = { workspace = true, optional = true }
serde_wasm_bindgen = { version = "0.6", optional = true }
js-sys             = { workspace = true, optional = true }

[dev-dependencies]
criterion = { workspace = true }
proptest  = { workspace = true }
approx    = "0.5"
```

### 2. Consuming Crate Feature Gates

Each crate that integrates solver capabilities exposes granular `sublinear-*` flags
that map onto solver features. This keeps the dependency graph explicit and auditable.

#### 2.1 ruvector-core

```toml
# Additions to crates/ruvector-core/Cargo.toml [features]

# Sublinear solver integration (opt-in)
sublinear = ["dep:ruvector-solver"]

# Coherence verification for HNSW index quality
sublinear-coherence = [
    "sublinear",
    "ruvector-solver/nalgebra-backend",
]
```

The `sublinear-coherence` flag enables runtime coherence checks on HNSW graph edges.
It requires the nalgebra backend because the coherence verifier uses sheaf-theoretic
linear algebra that maps naturally to nalgebra's matrix abstractions.

#### 2.2 ruvector-graph

```toml
# Additions to crates/ruvector-graph/Cargo.toml [features]

# Sublinear spectral partitioning and Laplacian solvers
sublinear = ["dep:ruvector-solver"]

sublinear-graph = [
    "sublinear",
    "ruvector-solver/ndarray-backend",
]

# Spectral methods for graph partitioning
sublinear-spectral = [
    "sublinear-graph",
    "ruvector-solver/parallel",
]
```

Graph crates use the ndarray backend because ruvector-graph already depends on ndarray
for adjacency matrices and spectral embeddings. Pulling in nalgebra here would add an
unnecessary second linear algebra library.

#### 2.3 ruvector-gnn

```toml
# Additions to crates/ruvector-gnn/Cargo.toml [features]

# GNN-accelerated sublinear search
sublinear = ["dep:ruvector-solver"]

sublinear-gnn = [
    "sublinear",
    "ruvector-solver/ndarray-backend",
]
```

#### 2.4 ruvector-attention

```toml
# Additions to crates/ruvector-attention/Cargo.toml [features]

# Sublinear attention routing
sublinear = ["dep:ruvector-solver"]

sublinear-attention = [
    "sublinear",
    "ruvector-solver/nalgebra-backend",
    "math",
]
```

#### 2.5 ruvector-collections

```toml
# Additions to crates/ruvector-collections/Cargo.toml [features]

# Sublinear collection-level query dispatch
sublinear = ["ruvector-core/sublinear"]
```

Collections delegates to ruvector-core and does not directly depend on the solver crate.

### 3. Workspace-Level Aggregate Flags

```toml
# Additions to workspace Cargo.toml [workspace.dependencies]

ruvector-solver = { path = "crates/ruvector-solver", default-features = false }
```

No workspace-level default features are set for the solver. Each consumer pulls exactly
the features it needs.

### 4. Conditional Compilation Patterns

All solver-gated code uses consistent `cfg` attribute patterns to ensure the compiler
eliminates dead code paths when features are disabled.

#### 4.1 Module-Level Gating

```rust
// In crates/ruvector-core/src/lib.rs

#[cfg(feature = "sublinear")]
pub mod sublinear;

#[cfg(feature = "sublinear-coherence")]
pub mod coherence;
```

#### 4.2 Trait Implementation Gating

```rust
// In crates/ruvector-core/src/index/hnsw.rs

#[cfg(feature = "sublinear-coherence")]
impl HnswIndex {
    /// Verify edge coherence across the HNSW graph using sheaf Laplacian.
    ///
    /// Returns the coherence score in [0, 1] where 1.0 means perfectly coherent.
    /// Only available when the `sublinear-coherence` feature is enabled.
    pub fn verify_coherence(&self, config: &CoherenceConfig) -> Result<f64, SolverError> {
        use ruvector_solver::coherence::SheafCoherenceVerifier;

        let verifier = SheafCoherenceVerifier::new(config.clone());
        verifier.verify(&self.graph)
    }
}
```

#### 4.3 Function-Level Gating with Fallback

```rust
// In crates/ruvector-graph/src/query/planner.rs

/// Select the optimal query execution strategy.
///
/// When `sublinear-spectral` is enabled, the planner considers spectral
/// partitioning for large graph traversals. Otherwise, it falls back to
/// the existing cost-based optimizer.
pub fn select_strategy(&self, query: &GraphQuery) -> ExecutionStrategy {
    #[cfg(feature = "sublinear-spectral")]
    {
        if self.should_use_spectral(query) {
            return self.plan_spectral(query);
        }
    }

    // Default path: cost-based optimizer (always available)
    self.plan_cost_based(query)
}
```

#### 4.4 Compile-Time Backend Selection

```rust
// In crates/ruvector-solver/src/backend.rs

/// Marker type for the active linear algebra backend.
///
/// The solver supports nalgebra and ndarray simultaneously. Consumers
/// select which backend(s) to activate via feature flags. When both
/// are active, the solver can dispatch to whichever backend is more
/// efficient for a given operation.

#[cfg(feature = "nalgebra-backend")]
pub mod nalgebra_ops {
    use nalgebra::{DMatrix, DVector};

    pub fn solve_laplacian(laplacian: &DMatrix<f64>, rhs: &DVector<f64>) -> DVector<f64> {
        // Cholesky decomposition for positive semi-definite Laplacians
        let chol = laplacian.clone().cholesky()
            .expect("Laplacian must be positive semi-definite");
        chol.solve(rhs)
    }
}

#[cfg(feature = "ndarray-backend")]
pub mod ndarray_ops {
    use ndarray::{Array1, Array2};

    pub fn spectral_embedding(adjacency: &Array2<f64>, dim: usize) -> Array2<f64> {
        // Eigendecomposition of the normalized Laplacian
        // ... implementation details
        todo!("spectral embedding via ndarray")
    }
}
```

### 5. Runtime Algorithm Selection

Beyond compile-time feature gates, the solver provides a runtime dispatch layer
that selects between dense and sublinear code paths based on data characteristics.

```rust
// In crates/ruvector-solver/src/dispatch.rs

/// Configuration for runtime algorithm selection.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SolverDispatchConfig {
    /// Sparsity threshold above which the sublinear path is preferred.
    /// Default: 0.95 (95% sparse). Range: [0.0, 1.0].
    pub sparsity_threshold: f64,

    /// Minimum number of elements before sublinear algorithms are considered.
    /// Below this threshold, dense algorithms are always faster due to setup costs.
    /// Default: 10_000.
    pub min_elements_for_sublinear: usize,

    /// Maximum fraction of elements the sublinear path may touch.
    /// If the solver would need to examine more than this fraction,
    /// it falls back to the dense path.
    /// Default: 0.1 (10%).
    pub max_touch_fraction: f64,

    /// Force a specific path regardless of data characteristics.
    /// None means auto-detection (recommended).
    pub force_path: Option<SolverPath>,
}

impl Default for SolverDispatchConfig {
    fn default() -> Self {
        Self {
            sparsity_threshold: 0.95,
            min_elements_for_sublinear: 10_000,
            max_touch_fraction: 0.1,
            force_path: None,
        }
    }
}

/// Which execution path to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SolverPath {
    /// Traditional dense algorithms.
    Dense,
    /// Sublinear-time algorithms (only touches a fraction of the data).
    Sublinear,
}

/// Determine the optimal execution path for the given data.
pub fn select_path(
    total_elements: usize,
    nonzero_elements: usize,
    config: &SolverDispatchConfig,
) -> SolverPath {
    if let Some(forced) = config.force_path {
        return forced;
    }

    if total_elements < config.min_elements_for_sublinear {
        return SolverPath::Dense;
    }

    let sparsity = 1.0 - (nonzero_elements as f64 / total_elements as f64);
    if sparsity >= config.sparsity_threshold {
        SolverPath::Sublinear
    } else {
        SolverPath::Dense
    }
}
```

### 6. WASM Feature Interaction Matrix

WASM targets cannot use certain features (mmap, threads via rayon, SIMD on older
runtimes). The following matrix defines valid feature combinations per platform.

```
Legend:  Y = supported    N = not supported    P = partial (polyfill)

Feature                    | native-x86_64 | native-aarch64 | wasm32-unknown | wasm32-wasi
---------------------------+---------------+----------------+----------------+------------
sublinear                  | Y             | Y              | Y              | Y
sublinear-coherence        | Y             | Y              | Y              | Y
sublinear-graph            | Y             | Y              | Y              | Y
sublinear-gnn              | Y             | Y              | Y              | Y
sublinear-spectral         | Y             | Y              | N (no rayon)   | N
sublinear-attention        | Y             | Y              | Y              | Y
nalgebra-backend           | Y             | Y              | Y              | Y
ndarray-backend            | Y             | Y              | Y              | Y
parallel (rayon)           | Y             | Y              | N              | N
simd                       | Y             | Y              | P (128-bit)    | P
gpu                        | Y             | P              | N              | N
solver + storage           | Y             | Y              | N              | Y (fs)
solver + hnsw              | Y             | Y              | N              | N
```

#### WASM Guard Pattern

```rust
// In crates/ruvector-solver/src/lib.rs

// Prevent invalid feature combinations at compile time.
#[cfg(all(feature = "parallel", target_arch = "wasm32"))]
compile_error!(
    "The `parallel` feature (rayon) is not supported on wasm32 targets. \
     Remove it or use `--no-default-features` when building for WASM."
);

#[cfg(all(feature = "gpu", target_arch = "wasm32"))]
compile_error!(
    "The `gpu` feature is not supported on wasm32 targets."
);
```

### 7. Feature Flag Documentation Pattern

Every feature flag must include a doc comment in the crate-level documentation.

```rust
// In crates/ruvector-solver/src/lib.rs

//! # Feature Flags
//!
//! | Flag               | Default | Description                                      |
//! |--------------------|---------|--------------------------------------------------|
//! | `nalgebra-backend` | off     | Enable nalgebra for sheaf/coherence operations    |
//! | `ndarray-backend`  | off     | Enable ndarray for spectral/graph operations      |
//! | `parallel`         | off     | Enable rayon for multi-threaded solver execution   |
//! | `simd`             | off     | Enable SIMD intrinsics (auto-detected at build)   |
//! | `gpu`              | off     | Enable GPU dispatch through ruvector-math          |
//! | `wasm`             | off     | Enable WASM bindings via wasm-bindgen              |
//! | `full`             | off     | Enable nalgebra + ndarray + parallel               |
```

---

## Progressive Rollout Plan

### Phase 1: Foundation (Weeks 1-3)

**Goal**: Introduce the solver crate with zero consumer integration.

| Task                                              | Acceptance Criteria                          |
|---------------------------------------------------|----------------------------------------------|
| Create `crates/ruvector-solver` with empty public API | Crate compiles, no downstream changes      |
| Define all feature flags in Cargo.toml            | `cargo check --all-features` passes          |
| Add solver to workspace members list              | `cargo build -p ruvector-solver` succeeds    |
| Write compile-time WASM guards                    | WASM build fails gracefully on invalid combos|
| Add `ruvector-solver` to workspace dependencies   | Resolver v2 is satisfied                     |
| Set up CI job for `ruvector-solver` feature matrix | All matrix entries pass                     |

**Feature flags available**: `nalgebra-backend`, `ndarray-backend`, `parallel`, `simd`,
`wasm`, `full`.

**Consumer flags available**: None (solver is not yet a dependency of any consumer).

**Risk**: Minimal. No consumer code changes.

### Phase 2: Core Integration (Weeks 4-7)

**Goal**: Enable coherence verification in ruvector-core and GNN acceleration in
ruvector-gnn behind opt-in feature flags.

| Task                                              | Acceptance Criteria                          |
|---------------------------------------------------|----------------------------------------------|
| Add `sublinear` flag to ruvector-core             | Flag compiles with no behavioral change      |
| Add `sublinear-coherence` flag to ruvector-core   | Coherence verifier runs on HNSW graphs       |
| Add `sublinear-gnn` flag to ruvector-gnn          | GNN training uses sublinear message passing  |
| Write integration tests for coherence             | Tests pass with and without the flag         |
| Write integration tests for GNN acceleration      | Tests pass with and without the flag         |
| Benchmark coherence overhead                      | Less than 5% latency increase on default path|
| Update ruvector-core README with new flags        | Documentation is current                     |

**Feature flags available**: Phase 1 flags + `sublinear`, `sublinear-coherence`,
`sublinear-gnn`.

**Rollback plan**: Remove the `sublinear*` feature flags from consumer Cargo.toml and
delete the gated modules. No API changes to revert because all new code is behind
feature gates.

### Phase 3: Extended Integration (Weeks 8-11)

**Goal**: Bring sublinear spectral methods to ruvector-graph and sublinear attention
routing to ruvector-attention.

| Task                                              | Acceptance Criteria                          |
|---------------------------------------------------|----------------------------------------------|
| Add `sublinear-graph` flag to ruvector-graph      | Spectral partitioning available behind flag  |
| Add `sublinear-spectral` flag to ruvector-graph   | Parallel spectral solver works               |
| Add `sublinear-attention` flag to ruvector-attention | Attention routing uses solver dispatch    |
| Add `sublinear` flag to ruvector-collections      | Collection query dispatch delegates properly |
| WASM builds for all new flags                     | `cargo build --target wasm32-unknown-unknown`|
| Performance benchmarks for spectral partitioning  | At least 2x speedup on graphs with >100k nodes|
| Cross-crate integration tests                     | Multi-crate feature combos work end-to-end   |

**Feature flags available**: Phase 2 flags + `sublinear-graph`, `sublinear-spectral`,
`sublinear-attention`.

### Phase 4: Default Promotion (Weeks 12-16)

**Goal**: After validation, promote selected sublinear features to default feature sets.

| Task                                              | Acceptance Criteria                          |
|---------------------------------------------------|----------------------------------------------|
| Collect benchmark data from all phases            | Data covers all target platforms              |
| Run `cargo semver-checks` on all modified crates  | Zero breaking changes detected               |
| Promote `sublinear-coherence` to ruvector-core default | Default build includes coherence checks |
| Promote `sublinear-gnn` to ruvector-gnn default   | Default GNN build uses solver acceleration   |
| Update ruvector workspace version to 2.1.0        | Minor version bump signals new capabilities  |
| Publish updated crates to crates.io               | All crates pass `cargo publish --dry-run`    |

**Promotion criteria** (all must be met):

1. Zero regressions in existing benchmark suite.
2. Less than 2% compile-time increase for `cargo build` with default features.
3. Less than 50 KB binary size increase for default builds.
4. All platform CI targets pass.
5. At least 4 weeks of Phase 3 stability with no feature-related bug reports.

**Feature changes at promotion**:

```toml
# BEFORE (Phase 3)
# crates/ruvector-core/Cargo.toml
[features]
default = ["simd", "storage", "hnsw", "api-embeddings", "parallel"]

# AFTER (Phase 4)
# crates/ruvector-core/Cargo.toml
[features]
default = ["simd", "storage", "hnsw", "api-embeddings", "parallel", "sublinear-coherence"]
```

---

## CI Configuration for Feature Matrix Testing

### Strategy: Tiered Matrix

Testing all 2^N feature combinations is infeasible. Instead, we test a curated set of
meaningful profiles that cover: (a) each feature in isolation, (b) common real-world
combinations, and (c) platform-specific builds.

```yaml
# .github/workflows/solver-features.yml

name: Solver Feature Matrix
on:
  push:
    paths:
      - 'crates/ruvector-solver/**'
      - 'crates/ruvector-core/**'
      - 'crates/ruvector-graph/**'
      - 'crates/ruvector-gnn/**'
      - 'crates/ruvector-attention/**'
  pull_request:
    paths:
      - 'crates/ruvector-solver/**'

jobs:
  feature-matrix:
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        include:
          # Tier 1: Individual features on Linux
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            features: "nalgebra-backend"
            name: "nalgebra-only"
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            features: "ndarray-backend"
            name: "ndarray-only"
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            features: "parallel"
            name: "parallel-only"
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            features: "simd"
            name: "simd-only"

          # Tier 2: Common combinations
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            features: "nalgebra-backend,parallel"
            name: "coherence-profile"
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            features: "ndarray-backend,parallel"
            name: "spectral-profile"
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            features: "full"
            name: "full-profile"
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            features: ""
            name: "no-features"

          # Tier 3: Platform-specific
          - os: ubuntu-latest
            target: wasm32-unknown-unknown
            features: "wasm,nalgebra-backend"
            name: "wasm-nalgebra"
          - os: ubuntu-latest
            target: wasm32-unknown-unknown
            features: "wasm,ndarray-backend"
            name: "wasm-ndarray"
          - os: ubuntu-latest
            target: wasm32-unknown-unknown
            features: "wasm"
            name: "wasm-minimal"
          - os: macos-latest
            target: aarch64-apple-darwin
            features: "full"
            name: "aarch64-full"

    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - name: Check ${{ matrix.name }}
        run: |
          cargo check -p ruvector-solver \
            --target ${{ matrix.target }} \
            --no-default-features \
            --features "${{ matrix.features }}"
      - name: Test ${{ matrix.name }}
        if: matrix.target != 'wasm32-unknown-unknown'
        run: |
          cargo test -p ruvector-solver \
            --no-default-features \
            --features "${{ matrix.features }}"

  # Consumer crate integration matrix
  consumer-integration:
    runs-on: ubuntu-latest
    strategy:
      fail-fast: false
      matrix:
        include:
          - crate: ruvector-core
            features: "sublinear-coherence"
          - crate: ruvector-graph
            features: "sublinear-spectral"
          - crate: ruvector-gnn
            features: "sublinear-gnn"
          - crate: ruvector-attention
            features: "sublinear-attention"
          - crate: ruvector-collections
            features: "sublinear"
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Test ${{ matrix.crate }} + ${{ matrix.features }}
        run: |
          cargo test -p ${{ matrix.crate }} \
            --features "${{ matrix.features }}"

  # Semver compliance check
  semver-check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Install cargo-semver-checks
        run: cargo install cargo-semver-checks
      - name: Check semver compliance
        run: |
          for crate in ruvector-core ruvector-graph ruvector-gnn ruvector-attention; do
            cargo semver-checks check-release -p "$crate"
          done
```

### Local Developer Workflow

```bash
# Verify a single feature
cargo check -p ruvector-solver --no-default-features --features nalgebra-backend

# Verify WASM compatibility
cargo check -p ruvector-solver --target wasm32-unknown-unknown --no-default-features --features wasm

# Run the full matrix locally (requires cargo-hack)
cargo install cargo-hack
cargo hack check -p ruvector-solver --feature-powerset --depth 2

# Verify no semver breakage
cargo install cargo-semver-checks
cargo semver-checks check-release -p ruvector-core
```

---

## Migration Guide for Existing Users

### Users Who Do Not Want Sublinear Features

No action required. All sublinear features default to `off`. Existing builds, APIs,
and binary sizes are unchanged.

```toml
# This continues to work exactly as before:
[dependencies]
ruvector-core = "2.1"
```

### Users Who Want Coherence Verification

```toml
# Cargo.toml
[dependencies]
ruvector-core = { version = "2.1", features = ["sublinear-coherence"] }
```

```rust
// main.rs
use ruvector_core::index::HnswIndex;
use ruvector_core::coherence::CoherenceConfig;

fn main() -> anyhow::Result<()> {
    let index = HnswIndex::new(/* ... */)?;
    // ... insert vectors ...

    let config = CoherenceConfig::default();
    let score = index.verify_coherence(&config)?;
    println!("HNSW coherence score: {score:.4}");
    Ok(())
}
```

### Users Who Want GNN-Accelerated Search

```toml
# Cargo.toml
[dependencies]
ruvector-gnn = { version = "2.1", features = ["sublinear-gnn"] }
```

```rust
use ruvector_gnn::SublinearGnnSearch;

let searcher = SublinearGnnSearch::builder()
    .sparsity_threshold(0.90)
    .min_elements(5_000)
    .build()?;

let results = searcher.search(&graph, &query_vector, k)?;
```

### Users Who Want Spectral Graph Partitioning

```toml
# Cargo.toml
[dependencies]
ruvector-graph = { version = "2.1", features = ["sublinear-spectral"] }
```

```rust
use ruvector_graph::spectral::SpectralPartitioner;

let partitioner = SpectralPartitioner::new(num_partitions);
let partition_map = partitioner.partition(&graph)?;
```

### Users Who Want Everything

```toml
# Cargo.toml
[dependencies]
ruvector-core      = { version = "2.1", features = ["sublinear-coherence"] }
ruvector-graph     = { version = "2.1", features = ["sublinear-spectral"] }
ruvector-gnn       = { version = "2.1", features = ["sublinear-gnn"] }
ruvector-attention = { version = "2.1", features = ["sublinear-attention"] }
```

### WASM Users

```toml
# Cargo.toml
[dependencies]
ruvector-core = { version = "2.1", default-features = false, features = [
    "memory-only",
    "sublinear-coherence",
] }
```

Note: `sublinear-spectral` is not available on WASM because it depends on rayon.
Use `sublinear-graph` (without parallel spectral) instead.

---

## Consequences

### Positive

- **Zero disruption**: all existing users, builds, and CI pipelines continue to work
  unchanged because every new capability is behind an opt-in feature flag.
- **Granular adoption**: teams can enable exactly the solver capabilities they need
  without pulling in unused backends or dependencies.
- **Dependency isolation**: nalgebra users do not pay for ndarray, and vice versa.
  The feature flag hierarchy enforces this separation at the Cargo resolver level.
- **Platform safety**: compile-time guards prevent invalid feature combinations on
  WASM, eliminating a class of runtime surprises.
- **Auditable dependency graph**: `cargo tree --features sublinear-coherence` shows
  exactly what each flag brings in, making security review straightforward.
- **Reversible**: any phase can be rolled back by removing feature flags from consumer
  crates, with zero API changes to revert.
- **CI efficiency**: the tiered matrix tests meaningful combinations rather than an
  exponential powerset, keeping CI times tractable.

### Negative

- **Cognitive overhead**: developers must understand the feature flag hierarchy to
  choose the right flags. The naming convention (`sublinear-*`) and documentation
  mitigate this but do not eliminate it.
- **Combinatorial testing gap**: we cannot test every possible combination. Edge-case
  interactions between features (e.g., `sublinear-coherence` + `distributed` + `wasm`)
  may surface late.
- **Conditional compilation complexity**: `#[cfg(feature = "...")]` blocks add
  indirection to the codebase. Code navigation tools may not resolve cfg-gated items
  correctly.
- **Feature flag drift**: if a consuming crate adds a solver feature but the solver
  crate reorganizes its flag names, the consumer will fail to compile. Cargo's resolver
  catches this at build time, but the error message may be unclear.
- **Binary size**: each additional feature flag adds code behind conditional compilation,
  potentially increasing binary size for users who enable many features.

### Neutral

- The solver crate is a new workspace member, increasing the total crate count by one.
- Workspace dependency resolution time increases marginally due to one additional crate.
- Feature flags become the primary coordination mechanism between solver and consumer
  crates, replacing what would otherwise be runtime configuration.

---

## Options Considered

### Option 1: Monolithic Feature Flag (Rejected)

A single `sublinear` flag on each consumer crate that enables all solver capabilities.

- **Pros**: Simple to understand, one flag per crate, minimal documentation needed.
- **Cons**: All-or-nothing adoption. Users who only need coherence must also pull in
  ndarray for spectral methods and rayon for parallel solvers. This violates the
  dependency hygiene constraint and increases binary size unnecessarily.
- **Verdict**: Rejected because it forces unnecessary dependencies on consumers.

### Option 2: Runtime-Only Selection (Rejected)

No feature flags. The solver crate is always compiled with all backends. Algorithm
selection happens purely at runtime.

- **Pros**: No conditional compilation, simpler build system, no feature matrix in CI.
- **Cons**: Every consumer always pays the compile-time and binary-size cost of all
  backends. WASM targets would fail to compile because rayon and mmap are always
  included. This violates the platform parity constraint.
- **Verdict**: Rejected because it is incompatible with WASM and wastes resources.

### Option 3: Separate Crates Per Algorithm (Rejected)

Instead of feature flags, create `ruvector-solver-coherence`,
`ruvector-solver-spectral`, `ruvector-solver-gnn` as separate crates.

- **Pros**: Maximum isolation, each crate has its own version and changelog. Consumers
  depend only on the crate they need.
- **Cons**: High maintenance overhead (4+ additional Cargo.toml files, CI jobs, crate
  publications). Shared types between solver algorithms require a `ruvector-solver-types`
  crate, adding another layer. The workspace already has 100+ crates; adding 4-5 more
  for one integration is disproportionate.
- **Verdict**: Rejected due to maintenance burden and workspace bloat.

### Option 4: Hierarchical Feature Flags (Accepted)

The approach described in this ADR. One solver crate with backend flags, consumer crates
with `sublinear-*` flags, workspace-level aggregates for convenience.

- **Pros**: Balances granularity with simplicity. One new crate, N feature flags.
  Cargo's feature unification handles transitive activation. CI matrix is tractable.
- **Cons**: Requires careful documentation and naming conventions. Some cognitive
  overhead for new contributors.
- **Verdict**: Accepted as the best balance of isolation, usability, and maintenance cost.

---

## Related Decisions

- **ADR-STS-001**: Solver Integration Architecture -- defines the overall integration
  strategy that this ADR implements via feature flags.
- **ADR-STS-003**: WASM Strategy -- defines platform constraints that this ADR enforces
  via compile-time guards.
- **ADR-STS-004**: Performance Benchmarks -- defines the benchmarking framework used to
  validate Phase 4 promotion criteria.

---

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-02-20 | RuVector Team | Initial proposal |
| 1.0 | 2026-02-20 | RuVector Team | Accepted: full implementation complete |

---

## Implementation Status

Feature flag system fully operational: `neumann`, `cg`, `forward-push`, `backward-push`, `hybrid-random-walk`, `true-solver`, `bmssp` as individual flags. `all-algorithms` meta-flag enables all. `simd` for AVX2 acceleration. `wasm` for WebAssembly target. `parallel` for rayon/crossbeam concurrency. Default features: neumann, cg, forward-push. Conditional compilation throughout with `#[cfg(feature = ...)]`.

---

## References

- [Cargo Features Reference](https://doc.rust-lang.org/cargo/reference/features.html)
- [cargo-semver-checks](https://github.com/obi1kenobi/cargo-semver-checks)
- [cargo-hack](https://github.com/taiki-e/cargo-hack) -- for feature powerset testing
- [MADR 3.0 Template](https://adr.github.io/madr/)
- [ruvector-core Cargo.toml](/home/user/ruvector/crates/ruvector-core/Cargo.toml)
- [ruvector-graph Cargo.toml](/home/user/ruvector/crates/ruvector-graph/Cargo.toml)
- [ruvector-math Cargo.toml](/home/user/ruvector/crates/ruvector-math/Cargo.toml)
- [ruvector-gnn Cargo.toml](/home/user/ruvector/crates/ruvector-gnn/Cargo.toml)
- [ruvector-attention Cargo.toml](/home/user/ruvector/crates/ruvector-attention/Cargo.toml)
- [Workspace Cargo.toml](/home/user/ruvector/Cargo.toml)
