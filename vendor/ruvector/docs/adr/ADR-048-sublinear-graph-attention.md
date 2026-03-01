# ADR-048: Sublinear Graph Attention

## Status

Accepted

## Date

2026-02-25

## Context

Standard graph attention (GAT, Graph Transformer) computes pairwise attention over all nodes, yielding O(n^2) time and memory complexity. For RuVector's target use cases -- billion-node knowledge graphs, large-scale molecular graphs, and real-time recommendation systems -- quadratic scaling is prohibitive.

The RuVector workspace already contains the algorithmic building blocks for sublinear attention:

- `ruvector-solver` provides O(sqrt(n)) Personalized PageRank (PPR) via forward-push (`crates/ruvector-solver/src/forward_push.rs`) and hybrid random walks (`crates/ruvector-solver/src/random_walk.rs`)
- `ruvector-attention` provides `FlashAttention`, `LinearAttention`, and `LocalGlobalAttention` in `crates/ruvector-attention/src/sparse/`
- `ruvector-mincut` provides graph partitioning with the `canonical` feature for pseudo-deterministic min-cut
- `ruvector-gnn` provides memory-mapped tensor storage (`crates/ruvector-gnn/src/mmap.rs`) and cold-tier hyperbatch training for out-of-core processing
- `ruvector-coherence` provides spectral coherence scoring (`spectral` feature) for measuring attention quality

However, there is no unified mechanism for composing these into a graph attention layer with provable sublinear complexity, and no integration with the proof-gated mutation protocol (ADR-047) to certify complexity bounds before execution.

## Decision

We will implement a `sublinear_attention` module in `ruvector-graph-transformer` that provides three complementary sublinear graph attention mechanisms, a proof-gated complexity certification layer, and an integration path with memory-mapped processing for billion-node graphs.

### Mechanism 1: LSH-Attention on Spectral Coordinates

**Complexity**: O(n^{3/2}) time, O(n) memory

Locality-Sensitive Hashing (LSH) groups nodes by their spectral coordinates (Laplacian eigenvectors), then computes attention only within hash buckets. This exploits the fact that spectrally similar nodes tend to be structurally close.

```rust
pub struct LshSpectralAttention {
    /// Number of hash tables (more = higher recall, higher cost).
    num_tables: usize,
    /// Number of hash bits per table.
    hash_bits: usize,
    /// Spectral dimension (number of Laplacian eigenvectors).
    spectral_dim: usize,
    /// Proof requirement: complexity bound must be certified.
    complexity_proof: ProofRequirement,
}

impl LshSpectralAttention {
    /// Compute spectral coordinates via ruvector-coherence::spectral::estimate_fiedler
    /// and ruvector-solver's Neumann series for eigenvalue estimation.
    pub fn compute_spectral_coords(
        &self,
        graph: &impl GraphRepr,
        env: &mut ProofEnvironment,
    ) -> Result<ProofGate<SpectralCoords>>;

    /// Attention forward pass: hash nodes, compute intra-bucket attention.
    pub fn forward(
        &mut self,
        coords: &SpectralCoords,
        features: &NodeFeatures,
        env: &mut ProofEnvironment,
    ) -> Result<ProofGate<AttentionOutput>>;
}
```

The spectral coordinates are computed once per epoch using `ruvector-coherence::spectral::estimate_fiedler` for the Fiedler vector and `ruvector-solver::neumann::NeumannSolver` for fast eigenvalue approximation. LSH tables are rebuilt only when the graph topology changes (detected via min-cut value drift).

### Mechanism 2: PPR-Sampled Attention

**Complexity**: O(n log n) time, O(n log n / eps) memory

Personalized PageRank defines a node-specific importance distribution. For each query node, we sample the top-k PPR neighbors and compute attention only over those:

```rust
pub struct PprSampledAttention {
    /// PPR teleport probability (alpha). Standard: 0.15.
    alpha: f64,
    /// Number of PPR neighbors to attend to per query node.
    top_k: usize,
    /// Residual threshold for forward-push termination.
    epsilon: f64,
    /// Solver to use for PPR computation.
    solver: PprSolver,
}

pub enum PprSolver {
    /// Forward push from ruvector-solver. O(1/eps) per source.
    ForwardPush,
    /// Hybrid random walk from ruvector-solver. O(sqrt(n) / eps) total.
    HybridRandomWalk,
    /// Combined: forward push for hot nodes, random walk for cold.
    Adaptive { hot_threshold: f64 },
}

impl PprSampledAttention {
    /// Compute PPR-sampled attention for a batch of query nodes.
    ///
    /// Delegates to ruvector_solver::forward_push::ForwardPushSolver
    /// or ruvector_solver::random_walk (depending on PprSolver variant).
    pub fn forward(
        &mut self,
        query_nodes: &[NodeId],
        graph: &impl GraphRepr,
        features: &NodeFeatures,
        env: &mut ProofEnvironment,
    ) -> Result<ProofGate<AttentionOutput>>;
}
```

The `Adaptive` solver variant uses a heuristic: nodes with degree > `hot_threshold * avg_degree` use forward push (cheaper for high-degree nodes), while low-degree nodes use hybrid random walks.

### Mechanism 3: Spectral Sparsification

**Complexity**: O(n log n / eps^2) edges retained, O(n log n / eps^2) time

Spectral sparsification reduces the number of edges while preserving the graph Laplacian's spectral properties within a (1 + eps) factor. This is applied as a preprocessing step before any attention mechanism:

```rust
pub struct SpectralSparsifier {
    /// Approximation factor. Smaller eps = more edges retained.
    epsilon: f64,
    /// Effective resistance estimation samples.
    resistance_samples: usize,
}

impl SpectralSparsifier {
    /// Sparsify the graph, retaining O(n log n / eps^2) edges.
    ///
    /// Uses ruvector_coherence::spectral::estimate_effective_resistance_sampled
    /// to compute edge importance, then samples edges proportional to
    /// their effective resistance.
    pub fn sparsify(
        &self,
        graph: &impl GraphRepr,
        env: &mut ProofEnvironment,
    ) -> Result<ProofGate<SparsifiedGraph>>;
}
```

### Memory-Mapped Processing for Billion-Node Graphs

For graphs exceeding RAM, the sublinear attention layer integrates with `ruvector-gnn`'s memory-mapped infrastructure:

```rust
pub struct MmapSublinearAttention<A: SublinearGraphAttention> {
    /// The underlying attention mechanism.
    inner: A,
    /// Memory-mapped node features via ruvector_gnn::MmapManager.
    mmap_manager: MmapManager,
    /// Batch size for out-of-core processing.
    batch_size: usize,
}

impl<A: SublinearGraphAttention> MmapSublinearAttention<A> {
    /// Process in batches, memory-mapping node features on demand.
    /// Uses ruvector-gnn's cold-tier hyperbatch scheduling.
    pub fn forward_batched(
        &mut self,
        graph: &impl GraphRepr,
        env: &mut ProofEnvironment,
    ) -> Result<ProofGate<AttentionOutput>>;
}
```

This uses `ruvector_gnn::mmap::MmapManager` (gated behind `mmap` feature) for zero-copy access to node features stored on disk, and `ruvector_gnn::cold_tier` (gated behind `cold-tier` feature) for scheduling hyperbatches that fit in available RAM.

### Hierarchical Coarsening with Learned Pooling

For multi-scale attention, the module provides hierarchical coarsening that uses `ruvector-mincut` to partition the graph, then computes attention at each coarsening level:

```rust
pub struct HierarchicalAttention {
    /// Number of coarsening levels.
    levels: usize,
    /// Coarsening ratio per level (fraction of nodes to keep).
    ratio: f64,
    /// Min-cut feature flag: uses canonical min-cut for deterministic partitioning.
    use_canonical_mincut: bool,
    /// Pooling: how to aggregate node features within a partition.
    pooling: PoolingStrategy,
}

pub enum PoolingStrategy {
    /// Mean of node features within partition.
    Mean,
    /// Attention-weighted sum (learnable).
    AttentionPooling { dim: usize },
    /// Top-k scoring (learnable, like SAGPool).
    TopK { ratio: f64 },
}
```

### Proof-Gated Complexity Certification

Before executing any sublinear attention operation, the complexity bound is certified via the proof gate. This prevents accidental quadratic execution:

```rust
/// Certify that the attention mechanism will run within the stated
/// complexity bound for the given graph size.
///
/// Returns a ProofGate<ComplexityBound> that must be unlocked before
/// the attention forward pass can proceed.
pub fn certify_complexity(
    mechanism: &dyn SublinearGraphAttention,
    graph_stats: &GraphStats,
    env: &mut ProofEnvironment,
) -> Result<ProofGate<ComplexityBound>>;

pub struct ComplexityBound {
    /// Upper bound on operations: O(f(n, m, params)).
    pub ops_upper_bound: u64,
    /// Upper bound on memory bytes.
    pub memory_upper_bound: u64,
    /// The complexity class (for display/logging).
    pub complexity_class: String,
}
```

The certification computes the concrete upper bound given the graph's node count `n`, edge count `m`, and mechanism-specific parameters (eps, top_k, num_tables), then proves via `ProofTier::Reflex` that the bound is within the configured budget.

### SublinearGraphAttention Trait

All mechanisms implement a common trait:

```rust
pub trait SublinearGraphAttention {
    /// Theoretical complexity class as a string (e.g., "O(n^{3/2})").
    fn complexity_class(&self) -> &str;

    /// Concrete operation count upper bound for a graph with n nodes, m edges.
    fn ops_upper_bound(&self, n: usize, m: usize) -> u64;

    /// Concrete memory upper bound in bytes.
    fn memory_upper_bound(&self, n: usize, m: usize) -> u64;

    /// Forward pass.
    fn forward(
        &mut self,
        graph: &dyn GraphRepr,
        features: &NodeFeatures,
        env: &mut ProofEnvironment,
    ) -> Result<ProofGate<AttentionOutput>>;
}
```

### Attention Registry Integration

The `AttentionRegistry` in `GraphTransformer` (ADR-046) can hold any `SublinearGraphAttention` implementor. Users can register custom sublinear mechanisms:

```rust
let mut gt = GraphTransformer::new(config, graph)?;
gt.register_attention("ppr-k64", PprSampledAttention::new(0.15, 64, 1e-6, PprSolver::Adaptive { hot_threshold: 2.0 }));
gt.register_attention("lsh-spectral", LshSpectralAttention::new(8, 12, 32));
```

## Consequences

### Positive

- Billion-node graphs become tractable: O(n log n) PPR attention scales to 10^9 nodes
- Proof-gated complexity bounds prevent runtime blowup -- the system refuses to execute if the bound exceeds budget
- Three complementary mechanisms cover different graph structures (dense clusters via LSH, sparse power-law via PPR, general via sparsification)
- Memory-mapped integration avoids OOM for large graphs
- Hierarchical coarsening enables multi-scale representation learning

### Negative

- LSH spectral coordinates require an upfront eigenvalue computation (amortized over epochs)
- PPR forward-push has high variance for disconnected or near-disconnected components
- Spectral sparsification quality degrades for non-expander graphs
- Three mechanisms increase the decision surface for users choosing an approach

### Risks

- PPR alpha parameter is sensitive: too high (> 0.3) makes attention too local, too low (< 0.05) loses locality. Mitigated by the `Adaptive` solver which auto-tunes based on graph diameter
- Memory-mapped processing introduces I/O latency. On NVMe SSDs, random 4KB reads are ~10 us; on HDDs, ~10 ms. The cold-tier scheduler mitigates this by prefetching based on PPR locality
- Spectral sparsification discards edges that may be important for attention. Mitigated by post-sparsification coherence check via `ruvector-coherence::spectral::SpectralCoherenceScore`

## Implementation

1. Define `SublinearGraphAttention` trait in `crates/ruvector-graph-transformer/src/sublinear_attention/mod.rs`
2. Implement `PprSampledAttention` bridging to `ruvector-solver::forward_push` and `ruvector-solver::random_walk`
3. Implement `LshSpectralAttention` using `ruvector-coherence::spectral` for eigenvector estimation
4. Implement `SpectralSparsifier` using `ruvector-coherence::spectral::estimate_effective_resistance_sampled`
5. Implement `HierarchicalAttention` bridging to `ruvector-mincut` canonical partitioning
6. Implement `MmapSublinearAttention<A>` bridging to `ruvector-gnn::mmap::MmapManager`
7. Implement `certify_complexity` using `ruvector-verified::gated::route_proof`
8. Benchmarks: PPR-64 on ogbn-papers100M (111M nodes), LSH on ogbn-products (2.4M nodes)

## References

- ADR-046: Graph Transformer Unified Architecture (module structure, `AttentionRegistry`)
- ADR-047: Proof-Gated Mutation Protocol (`ProofGate<T>`, `ProofRequirement`)
- `crates/ruvector-solver/src/forward_push.rs`: `ForwardPushSolver` for PPR
- `crates/ruvector-solver/src/random_walk.rs`: hybrid random walk PPR
- `crates/ruvector-solver/src/neumann.rs`: `NeumannSolver` for eigenvalue estimation
- `crates/ruvector-solver/src/traits.rs`: `SolverEngine` trait
- `crates/ruvector-attention/src/sparse/`: `FlashAttention`, `LinearAttention`, `LocalGlobalAttention`
- `crates/ruvector-coherence/src/spectral.rs`: `estimate_fiedler`, `estimate_effective_resistance_sampled`, `SpectralCoherenceScore`
- `crates/ruvector-gnn/src/mmap.rs`: `MmapManager`, `MmapGradientAccumulator`
- `crates/ruvector-gnn/src/cold_tier.rs`: hyperbatch scheduling for out-of-core training
- `crates/ruvector-mincut/Cargo.toml`: `canonical` feature for pseudo-deterministic min-cut
- Klicpera et al., "Predict then Propagate" (ICLR 2019) -- PPR-based GNN
- Spielman & Srivastava, "Graph Sparsification by Effective Resistances" (STOC 2008)
