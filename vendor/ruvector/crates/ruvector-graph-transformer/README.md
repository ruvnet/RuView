# ruvector-graph-transformer

[![Crates.io](https://img.shields.io/crates/v/ruvector-graph-transformer.svg)](https://crates.io/crates/ruvector-graph-transformer)
[![docs.rs](https://docs.rs/ruvector-graph-transformer/badge.svg)](https://docs.rs/ruvector-graph-transformer)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Tests](https://img.shields.io/badge/tests-186_passing-brightgreen.svg)]()

**A graph neural network where every operation is mathematically proven correct before it runs.**

```toml
[dependencies]
ruvector-graph-transformer = "2.0"
```

Most graph neural networks let you modify data freely -- add nodes, change weights, update edges -- with no safety guarantees. If a bug corrupts your graph, you find out later (or never). This crate takes a different approach: every mutation to graph state requires a formal proof that the operation is valid. No proof, no access. Think of it like a lock on every piece of data that can only be opened with the right mathematical key.

On top of that safety layer, 8 specialized modules bring cutting-edge graph intelligence: attention that scales to millions of nodes without checking every pair, physics simulations that conserve energy by construction, neurons that only fire when they should, training that automatically rolls back bad gradient steps, and geometry that works in curved spaces instead of assuming everything is flat.

The result is a graph transformer you can trust: if it produces an answer, that answer was computed correctly. Part of the [RuVector](https://github.com/ruvnet/ruvector) ecosystem.

| | Standard GNN | ruvector-graph-transformer |
|---|---|---|
| **Mutation safety** | Unchecked -- bugs corrupt silently | Proof-gated: no mutation without formal witness |
| **Attention complexity** | O(n^2) -- slows down on large graphs | O(n log n) sublinear via LSH/PPR/spectral |
| **Training guarantees** | Hope for the best | Verified: certificates, delta-apply rollback, fail-closed |
| **Geometry** | Euclidean only -- assumes flat space | Product manifolds S^n x H^m x R^k |
| **Causality** | No enforcement | Temporal masking + Granger causality extraction |
| **Incentive alignment** | Not considered | Nash equilibrium + Shapley attribution |
| **Platforms** | Python only | Rust + WASM + Node.js (NAPI-RS) |

## Key Features

| Feature | What It Does | Why It Matters |
|---------|-------------|----------------|
| **Proof-Gated Mutation** | Every write to graph state requires a formal proof | Bugs cannot silently corrupt your data -- invalid operations are rejected before they happen |
| **Sublinear Attention** | LSH-bucketed, PPR-sampled, and spectral sparsification | Scales to millions of nodes where O(n^2) attention would be unusable |
| **Verified Training** | Delta-apply rollback with BLAKE3-hashed certificates | Bad gradient steps are automatically rolled back; every training step is auditable |
| **Physics-Informed** | Hamiltonian dynamics, gauge equivariant message passing | Simulations conserve energy by construction -- no drift over long runs |
| **Biological** | Spiking attention, Hebbian/STDP learning, dendritic branching | Model brain-like dynamics where neurons only fire when they should |
| **Self-Organizing** | Morphogenetic fields, developmental programs, graph coarsening | Graphs that grow, adapt, and restructure themselves |
| **Manifold Geometry** | Product manifolds S^n x H^m x R^k, Riemannian Adam | Work in curved spaces where Euclidean assumptions break down |
| **Temporal-Causal** | Causal masking, continuous-time ODE, Granger causality | Enforce cause-before-effect and extract causal relationships from time series |
| **Economic** | Nash equilibrium attention, Shapley attribution | Align incentives in multi-agent graphs and attribute value fairly |

## Modules

8 feature-gated modules, each backed by an Architecture Decision Record:

| Module | Feature Flag | ADR | What It Does |
|--------|-------------|-----|--------------|
| **Proof-Gated Mutation** | always on | [ADR-047](../../docs/adr/ADR-047-proof-gated-mutation-protocol.md) | `ProofGate<T>`, `MutationLedger`, `ProofScope`, `EpochBoundary` |
| **Sublinear Attention** | `sublinear` | [ADR-048](../../docs/adr/ADR-048-sublinear-graph-attention.md) | LSH-bucket, PPR-sampled, spectral sparsification |
| **Physics-Informed** | `physics` | [ADR-051](../../docs/adr/ADR-051-physics-informed-graph-layers.md) | Hamiltonian dynamics, gauge equivariant MP, Lagrangian attention, conservative PDE |
| **Biological** | `biological` | [ADR-052](../../docs/adr/ADR-052-biological-graph-layers.md) | Spiking attention, Hebbian/STDP learning, dendritic branching, inhibition strategies |
| **Self-Organizing** | `self-organizing` | -- | Morphogenetic fields, developmental programs, graph coarsening |
| **Verified Training** | `verified-training` | [ADR-049](../../docs/adr/ADR-049-verified-training-pipeline.md) | Training certificates, delta-apply rollback, LossStabilityBound, EnergyGate |
| **Manifold** | `manifold` | [ADR-055](../../docs/adr/ADR-055-manifold-graph-layers.md) | Product manifolds, Riemannian Adam, geodesic MP, Lie group equivariance |
| **Temporal-Causal** | `temporal` | [ADR-053](../../docs/adr/ADR-053-temporal-causal-graph-layers.md) | Causal masking, retrocausal attention, continuous-time ODE, Granger causality |
| **Economic** | `economic` | [ADR-054](../../docs/adr/ADR-054-economic-graph-layers.md) | Nash equilibrium attention, Shapley attribution, incentive-aligned MPNN |

## Quick Start

```toml
[dependencies]
ruvector-graph-transformer = "2.0"

# Or with all modules:
ruvector-graph-transformer = { version = "2.0", features = ["full"] }
```

### Proof-Gated Mutation

Every mutation to graph state passes through a proof gate:

```rust
use ruvector_graph_transformer::{ProofGate, GraphTransformer, GraphTransformerConfig};
use ruvector_verified::ProofEnvironment;

// Create a proof environment and graph transformer
let mut env = ProofEnvironment::new();
let gt = GraphTransformer::with_defaults();

// Gate a value behind a proof
let gate: ProofGate<Vec<f32>> = gt.create_gate(vec![1.0; 128]);

// Mutation requires proof -- no proof, no access
let proof_id = ruvector_verified::prove_dim_eq(&mut env, 128, 128).unwrap();
let mutated = gate.mutate_with_proof(&env, proof_id, |v| {
    v.iter_mut().for_each(|x| *x *= 2.0);
}).unwrap();
```

### Sublinear Attention

```rust
use ruvector_graph_transformer::sublinear_attention::SublinearGraphAttention;
use ruvector_graph_transformer::config::SublinearConfig;

let config = SublinearConfig {
    lsh_buckets: 16,
    ppr_samples: 8,
    sparsification_factor: 0.5,
};
let attn = SublinearGraphAttention::new(128, config);

// O(n log n) instead of O(n^2)
let features = vec![vec![0.5f32; 128]; 1000];
let outputs = attn.lsh_attention(&features).unwrap();
```

### Verified Training

```rust
use ruvector_graph_transformer::verified_training::{VerifiedTrainer, TrainingInvariant};
use ruvector_graph_transformer::config::VerifiedTrainingConfig;

let config = VerifiedTrainingConfig {
    fail_closed: true,  // reject step if any invariant fails
    ..Default::default()
};
let mut trainer = VerifiedTrainer::new(
    config,
    vec![
        TrainingInvariant::LossStabilityBound { window: 10, max_deviation: 0.1 },
        TrainingInvariant::WeightNormBound { max_norm: 100.0 },
    ],
);

// Delta-apply: gradients go to scratch buffer, commit only if invariants pass
let result = trainer.step(&weights, &gradients, lr).unwrap();
assert!(result.certificate.is_some()); // BLAKE3-hashed training certificate
```

### Physics-Informed Layers

```rust
use ruvector_graph_transformer::physics::HamiltonianGraphNet;
use ruvector_graph_transformer::config::PhysicsConfig;

let config = PhysicsConfig::default();
let mut hgn = HamiltonianGraphNet::new(config);

// Symplectic leapfrog preserves energy
let (new_q, new_p) = hgn.step(&positions, &momenta, &edges, dt);
assert!(hgn.energy_conserved(1e-6)); // formal conservation proof
```

### Manifold Operations

```rust
use ruvector_graph_transformer::manifold::{ProductManifoldAttention, ManifoldType};
use ruvector_graph_transformer::config::ManifoldConfig;

let config = ManifoldConfig {
    spherical_dim: 64,
    hyperbolic_dim: 32,
    euclidean_dim: 32,
    curvature: -1.0,
};
let attn = ProductManifoldAttention::new(config);

// Attention in S^64 x H^32 x R^32
let outputs = attn.forward(&features, &edges).unwrap();
```

## Feature Flags

```toml
[features]
default = ["sublinear", "verified-training"]
full = ["sublinear", "physics", "biological", "self-organizing",
        "verified-training", "manifold", "temporal", "economic"]
```

| Flag | Default | Adds |
|------|---------|------|
| `sublinear` | yes | LSH, PPR, spectral attention |
| `verified-training` | yes | Training certificates, delta-apply rollback |
| `physics` | no | Hamiltonian, gauge, Lagrangian, PDE layers |
| `biological` | no | Spiking, Hebbian, STDP, dendritic layers |
| `self-organizing` | no | Morphogenetic fields, developmental programs |
| `manifold` | no | Product manifolds, Riemannian Adam, Lie groups |
| `temporal` | no | Causal masking, Granger causality, ODE |
| `economic` | no | Nash equilibrium, Shapley, incentive-aligned MPNN |

## Architecture

```
ruvector-graph-transformer
├── proof_gated.rs          <- ProofGate<T>, MutationLedger, attestation chains
├── sublinear_attention.rs  <- O(n log n) attention via LSH/PPR/spectral
├── physics.rs              <- Energy-conserving Hamiltonian/Lagrangian dynamics
├── biological.rs           <- Spiking networks, Hebbian plasticity, STDP
├── self_organizing.rs      <- Morphogenetic fields, reaction-diffusion growth
├── verified_training.rs    <- Certified training with delta-apply rollback
├── manifold.rs             <- Product manifold S^n x H^m x R^k geometry
├── temporal.rs             <- Causal masking, Granger causality, ODE integration
├── economic.rs             <- Nash equilibrium, Shapley values, mechanism design
├── config.rs               <- Per-module configuration with sensible defaults
├── error.rs                <- Unified error composing 4 sub-crate errors
└── lib.rs                  <- Unified entry point with feature-gated re-exports
```

### Dependencies

```
ruvector-graph-transformer
├── ruvector-verified    <- formal proofs, attestations, gated routing
├── ruvector-gnn         <- base GNN message passing
├── ruvector-attention   <- scaled dot-product attention
├── ruvector-mincut      <- graph structure operations
├── ruvector-solver      <- sparse linear systems
└── ruvector-coherence   <- coherence measurement
```

## Bindings

| Platform | Package | Install |
|----------|---------|---------|
| **WASM** | [`ruvector-graph-transformer-wasm`](../ruvector-graph-transformer-wasm) | `wasm-pack build` |
| **Node.js** | [`ruvector-graph-transformer-node`](../ruvector-graph-transformer-node) | `npm install @ruvector/graph-transformer` |

## Tests

```bash
# Default features (sublinear + verified-training)
cargo test -p ruvector-graph-transformer

# All modules
cargo test -p ruvector-graph-transformer --features full

# Individual module
cargo test -p ruvector-graph-transformer --features physics
```

**163 unit tests + 23 integration tests = 186 total**, all passing.

## ADR Documentation

| ADR | Title |
|-----|-------|
| [ADR-046](../../docs/adr/ADR-046-graph-transformer-architecture.md) | Unified Graph Transformer Architecture |
| [ADR-047](../../docs/adr/ADR-047-proof-gated-mutation-protocol.md) | Proof-Gated Mutation Protocol |
| [ADR-048](../../docs/adr/ADR-048-sublinear-graph-attention.md) | Sublinear Graph Attention |
| [ADR-049](../../docs/adr/ADR-049-verified-training-pipeline.md) | Verified Training Pipeline |
| [ADR-050](../../docs/adr/ADR-050-graph-transformer-bindings.md) | WASM + Node.js Bindings |
| [ADR-051](../../docs/adr/ADR-051-physics-informed-graph-layers.md) | Physics-Informed Graph Layers |
| [ADR-052](../../docs/adr/ADR-052-biological-graph-layers.md) | Biological Graph Layers |
| [ADR-053](../../docs/adr/ADR-053-temporal-causal-graph-layers.md) | Temporal-Causal Graph Layers |
| [ADR-054](../../docs/adr/ADR-054-economic-graph-layers.md) | Economic Graph Layers |
| [ADR-055](../../docs/adr/ADR-055-manifold-graph-layers.md) | Manifold Graph Layers |

## License

**MIT License** - see [LICENSE](../../LICENSE) for details.

---

<div align="center">

**Part of [RuVector](https://github.com/ruvnet/ruvector) - Built by [rUv](https://ruv.io)**

[![Star on GitHub](https://img.shields.io/github/stars/ruvnet/ruvector?style=social)](https://github.com/ruvnet/ruvector)

[Documentation](https://docs.rs/ruvector-graph-transformer) | [Crates.io](https://crates.io/crates/ruvector-graph-transformer) | [GitHub](https://github.com/ruvnet/ruvector)

</div>
