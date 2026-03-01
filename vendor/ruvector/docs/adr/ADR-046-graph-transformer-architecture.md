# ADR-046: Graph Transformer Unified Architecture

## Status

Accepted

## Date

2026-02-25

## Context

RuVector has accumulated eight specialized crates that together provide the building blocks for a full graph transformer stack: `ruvector-verified` for formal proofs, `ruvector-gnn` for graph neural network layers, `ruvector-attention` for 18+ attention mechanisms, `ruvector-mincut-gated-transformer` for energy-gated inference, `ruvector-solver` for sublinear sparse algorithms, `ruvector-coherence` for quality measurement, `ruvector-graph` for property graphs with Cypher, and `ruvector-mincut` for graph partitioning.

These crates were developed independently, each with their own error types, configuration patterns, and public APIs. Users who want to build proof-gated graph transformers must manually wire them together, handle error conversion between six different `thiserror` enums, coordinate feature flags across eight `Cargo.toml` files, and discover API composition patterns through trial and error.

We need a single `ruvector-graph-transformer` crate that composes these building blocks into a unified graph transformer with proof-gated mutation as the central control substrate, without duplicating any existing code.

## Decision

We will create `ruvector-graph-transformer` as a composition crate at `crates/ruvector-graph-transformer/` that delegates to existing crates and provides a unified entry point, error type, and configuration surface. The crate will not reimplement any algorithm -- it wraps, delegates, and orchestrates.

### Module Structure

```
crates/ruvector-graph-transformer/
  src/
    lib.rs                    # GraphTransformer unified entry point, re-exports
    error.rs                  # Unified GraphTransformerError composing sub-crate errors
    config.rs                 # Unified configuration with builder pattern
    proof_gated/
      mod.rs                  # ProofGate<T>, ProofScope, MutationLedger
      gate.rs                 # GateController bridging to ruvector-verified::gated
      attestation.rs          # Attestation chain composition via ProofAttestation
      epoch.rs                # Epoch boundaries for proof algebra upgrades
    sublinear_attention/
      mod.rs                  # SublinearGraphAttention trait and registry
      lsh.rs                  # LSH-attention on spectral coordinates
      ppr.rs                  # PPR-sampled attention via ruvector-solver
      spectral_sparsify.rs    # Spectral sparsification for edge reduction
    physics/
      mod.rs                  # PhysicsLayer: energy gates, diffusion, PDE attention
      energy.rs               # Bridges to ruvector-mincut-gated-transformer::EnergyGate
      diffusion.rs            # Bridges to ruvector-attention::DiffusionAttention
    biological/
      mod.rs                  # BiologicalLayer: spiking attention, EWC
      spiking.rs              # Bridges to ruvector-mincut-gated-transformer::spike
      ewc.rs                  # Bridges to ruvector-gnn::ElasticWeightConsolidation
    self_organizing/
      mod.rs                  # Mincut-driven topology adaptation
      partitioner.rs          # Bridges to ruvector-mincut
      coarsening.rs           # Hierarchical graph coarsening with learned pooling
    verified_training/
      mod.rs                  # VerifiedTrainer, TrainingCertificate
      pipeline.rs             # Proof-carrying training loop
      invariants.rs           # Per-step invariant specifications
    manifold/
      mod.rs                  # Manifold-aware operations
      hyperbolic.rs           # Bridges to ruvector-attention::HyperbolicAttention
      mixed_curvature.rs      # Bridges to ruvector-attention::MixedCurvatureFusedAttention
    temporal/
      mod.rs                  # Time-varying graph support
      snapshot.rs             # Temporal graph snapshots with proof chains
      evolving.rs             # Evolving attention over graph time series
```

### Feature Flags

Each module is gated behind an opt-in feature flag so users pay only for what they use:

```toml
[features]
default = ["proof-gated"]

# Core (always available when enabled)
proof-gated = ["ruvector-verified/gated-proofs", "ruvector-verified/fast-arena"]

# Attention mechanisms
sublinear-attention = ["ruvector-solver/forward-push", "ruvector-solver/hybrid-random-walk", "ruvector-attention"]
physics = ["ruvector-mincut-gated-transformer/energy_gate", "ruvector-attention/pde_attention"]
biological = ["ruvector-mincut-gated-transformer/spike_attention", "ruvector-gnn"]
manifold = ["ruvector-attention/math"]

# Graph structure
self-organizing = ["ruvector-mincut/canonical", "ruvector-graph"]
temporal = ["ruvector-graph/temporal"]

# Training
verified-training = ["ruvector-gnn", "ruvector-verified/all-proofs", "ruvector-coherence/spectral"]

# Convenience
full = ["proof-gated", "sublinear-attention", "physics", "biological",
        "manifold", "self-organizing", "temporal", "verified-training"]
```

### Unified Entry Point

The `GraphTransformer` struct is the primary public API. It is generic over the graph representation and parameterized by a `GraphTransformerConfig`:

```rust
pub struct GraphTransformer<G: GraphRepr = DefaultPropertyGraph> {
    config: GraphTransformerConfig,
    proof_env: ProofEnvironment,        // from ruvector-verified
    arena: FastTermArena,               // from ruvector-verified::fast_arena
    attention_registry: AttentionRegistry,
    gate_controller: Option<GateController>,
    graph: G,
}

impl<G: GraphRepr> GraphTransformer<G> {
    pub fn new(config: GraphTransformerConfig, graph: G) -> Result<Self>;
    pub fn forward(&mut self, input: &GraphBatch) -> Result<ProofGated<GraphOutput>>;
    pub fn mutate(&mut self, op: GraphMutation) -> Result<ProofGated<MutationResult>>;
    pub fn attention_scores(&self) -> &AttentionScores;
    pub fn coherence(&self) -> CoherenceSnapshot;
    pub fn proof_chain(&self) -> &[ProofAttestation];
}
```

### Error Handling

A single `GraphTransformerError` enum composes errors from all sub-crates using `#[from]` conversions via `thiserror`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum GraphTransformerError {
    #[error(transparent)]
    Verification(#[from] ruvector_verified::VerificationError),
    #[error(transparent)]
    Gnn(#[from] ruvector_gnn::GnnError),
    #[error(transparent)]
    Attention(#[from] ruvector_attention::AttentionError),
    #[error(transparent)]
    Graph(#[from] ruvector_graph::GraphError),
    #[error(transparent)]
    Solver(#[from] ruvector_solver::error::SolverError),
    #[error("proof gate rejected mutation: {reason}")]
    ProofGateRejected { reason: String, tier: ProofTier },
    #[error("coherence below threshold: {score} < {threshold}")]
    CoherenceBelowThreshold { score: f64, threshold: f64 },
    #[error("epoch boundary: proof algebra upgrade required")]
    EpochBoundary { current_epoch: u64, required_epoch: u64 },
}
```

### No-std Compatibility

Core types in `proof_gated/` (`ProofGate<T>`, `ProofScope`, `MutationLedger`) are `no_std` compatible via conditional compilation. They use `core::` primitives and avoid heap allocation on the critical path. The `alloc` feature gates `Vec`-based attestation chains for `no_std` environments with an allocator.

### Dependency Graph

```
ruvector-graph-transformer
  |-- ruvector-verified      (proof gates, attestations, FastTermArena)
  |-- ruvector-gnn           (GNN layers, EWC, training, mmap)
  |-- ruvector-attention     (18+ attention mechanisms)
  |-- ruvector-mincut-gated-transformer  (energy gates, spiking, Mamba SSM)
  |-- ruvector-solver        (sublinear sparse algorithms)
  |-- ruvector-coherence     (coherence measurement, spectral scoring)
  |-- ruvector-graph         (property graph, Cypher queries)
  |-- ruvector-mincut        (partitioning, canonical min-cut)
```

All dependencies use path-relative references (`path = "../ruvector-verified"`) and workspace version (`version = "2.0.4"`) except `ruvector-verified` (version `"0.1.1"`) and `ruvector-mincut-gated-transformer` (version `"0.1.0"`), which have independent versioning.

## Consequences

### Positive

- Users get a single dependency (`ruvector-graph-transformer`) instead of coordinating eight crates
- Feature flags keep compile times low for users who only need a subset
- Unified error type eliminates manual `map_err` boilerplate at call sites
- `GraphTransformer` struct provides discoverability -- IDE autocomplete shows all available operations
- No code duplication -- every algorithm lives in exactly one crate
- The composition pattern means sub-crate improvements automatically flow through

### Negative

- Adding a new attention mechanism to `ruvector-attention` requires updating `AttentionRegistry` in this crate
- The unified error enum grows as sub-crates add error variants
- Feature flag combinatorics create a large CI test matrix (mitigated by testing `default` and `full` profiles)
- `GraphTransformer` struct may become a god-object if module boundaries are not enforced during review

### Risks

- Circular dependency: `ruvector-graph-transformer` depends on `ruvector-graph`, which must not depend back. Enforced by `cargo publish --dry-run` in CI
- Version skew: if `ruvector-verified` ships a breaking change at 0.2.0, the composition crate must update its bridge code. Mitigated by workspace-level `[patch]` during development
- Feature flag conflicts: enabling `biological` and `physics` simultaneously must not cause duplicate symbol errors from `ruvector-mincut-gated-transformer`. Verified by the `full` feature CI test

## Implementation

1. Create `crates/ruvector-graph-transformer/` with the module structure above
2. Add to `[workspace.members]` in root `Cargo.toml`
3. Implement `proof_gated/` first (it is the dependency of every other module)
4. Implement each module as a thin bridge layer with integration tests
5. Add `crates/ruvector-graph-transformer-wasm/` and `crates/ruvector-graph-transformer-node/` (see ADR-050)
6. CI: test `--features default`, `--features full`, and each individual feature in isolation

## References

- ADR-045: Lean-Agentic Integration (establishes `ruvector-verified` and `ProofEnvironment`)
- ADR-015: Coherence-Gated Transformer (sheaf attention design)
- ADR-047: Proof-Gated Mutation Protocol (details the `ProofGate<T>` type)
- ADR-048: Sublinear Graph Attention (attention complexity analysis)
- ADR-049: Verified Training Pipeline (proof-carrying training)
- ADR-050: Graph Transformer WASM and Node.js Bindings
- `crates/ruvector-verified/src/gated.rs`: `ProofTier`, `route_proof`, `verify_tiered`
- `crates/ruvector-attention/src/lib.rs`: 18+ attention mechanism re-exports
- `crates/ruvector-solver/src/lib.rs`: `SolverEngine` trait, sublinear algorithms
- `crates/ruvector-mincut-gated-transformer/src/energy_gate.rs`: `EnergyGate`, `EnergyGateConfig`
