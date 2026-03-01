# ADR-055: Manifold-Aware Graph Transformer Layers

## Status

Accepted

## Date

2026-02-25

## Context

Nearly all deployed graph transformers operate in flat Euclidean space. This is a geometric mismatch: power-law degree distributions (social networks, citation graphs) exhibit tree-like branching that requires exponentially many Euclidean dimensions to embed without distortion. Hierarchical structures embed naturally in hyperbolic space (exponential volume growth), cyclic substructures embed on spheres (positive curvature), and hybrid graphs require multiple curvature regimes simultaneously. A product manifold decomposition S^n x H^m x R^k captures all three regimes, but existing graph transformers do not operate natively in such spaces.

RuVector has substantial infrastructure for mixed-curvature operations:

- `ruvector-attention/src/hyperbolic/poincare.rs`: Poincare ball operations, `mobius_add`, `mobius_scalar_mult`, `frechet_mean`, geodesic distance with epsilon-buffered projection
- `ruvector-attention/src/hyperbolic/lorentz_cascade.rs`: `LorentzCascadeAttention` with Busemann scoring, Einstein midpoint aggregation, multi-curvature heads at logarithmically-spaced curvatures
- `ruvector-attention/src/hyperbolic/mixed_curvature.rs`: `MixedCurvatureAttention` combining Poincare and Lorentz models
- `ruvector-attention/src/curvature/fused_attention.rs`: `MixedCurvatureFusedAttention` with `FusedCurvatureConfig` for E x H x S product manifold
- `ruvector-attention/src/curvature/tangent_space.rs`: `TangentSpaceMapper` for 10-100x faster tangent-space operations
- `ruvector-attention/src/curvature/component_quantizer.rs`: quantization of mixed-curvature components
- `ruvector-attention/src/transport/sliced_wasserstein.rs`: `SlicedWassersteinAttention` for optimal transport on manifolds
- `ruvector-attention/src/transport/centroid_ot.rs`: `CentroidOTAttention` for centroid-based transport
- `ruvector-attention/src/sheaf/restriction.rs`: `RestrictionMap` for fiber bundle structure (Lie group equivariance)
- `ruvector-attention/src/sheaf/attention.rs`: `SheafAttention` for sheaf-structured attention

However, there is no module that provides curvature compatibility proofs before merging embeddings from different manifold components, geodesic message passing with parallel transport along shortest paths, Riemannian optimization (Riemannian Adam with exponential map), or Lie group equivariance (SE(3)/SO(3)) as a graph attention layer. The research at `docs/research/gnn-v2/27-hyperbolic-mixed-curvature-graph-transformers.md` describes the mathematics but defines no integration path with the proof-gated mutation protocol.

## Decision

We will implement a `manifold` module in `ruvector-graph-transformer` behind the `manifold` feature flag. The module provides `ProductManifoldAttention`, `CurvatureAdaptiveRouter`, `GeodesicMessagePassing`, `RiemannianAdamOptimizer`, and Lie group equivariance via sheaf bundle structure.

### ProductManifoldAttention

S^n x H^m x R^k product manifold attention with curvature compatibility proofs:

```rust
/// Product manifold attention on S^n x H^m x R^k.
///
/// Bridges to ruvector-attention::curvature::fused_attention for the
/// fused kernel. Before merging embeddings from different manifold
/// components, a curvature compatibility proof verifies that the
/// component curvatures are consistent (no NaN/Inf from mismatched
/// curvature parameters).
pub struct ProductManifoldAttention {
    /// Fused curvature config from ruvector-attention.
    fused_config: FusedCurvatureConfig,
    /// Per-component learned curvatures (extends FusedCurvatureConfig
    /// beyond its single hyperbolic_curvature to support per-head curvatures).
    component_curvatures: Vec<f32>,
    /// Tangent space mapper for efficient computation.
    tangent_mapper: TangentSpaceMapper,
    /// Proof requirement: curvature compatibility.
    curvature_proof: ProofRequirement,
}

impl ProductManifoldAttention {
    /// Product manifold attention forward pass.
    ///
    /// Decomposes features into (spherical, hyperbolic, Euclidean)
    /// components, computes attention in each space:
    /// - Spherical: normalized inner product on S^n
    /// - Hyperbolic: Busemann scoring via LorentzCascadeAttention
    /// - Euclidean: standard scaled dot product
    ///
    /// Merges via learned mixing weights: beta_S, beta_H, beta_E.
    ///
    /// Proof gate: before merging, verifies curvature compatibility:
    /// - Hyperbolic curvature c > 0 (no degenerate flat limit)
    /// - Spherical embeddings on unit sphere (||x_S|| = 1 +/- eps)
    /// - Poincare embeddings inside ball (c * ||x_H||^2 < 1 - margin)
    /// Routes to ProofTier::Reflex (scalar/norm checks).
    pub fn forward(
        &self,
        features: &[f32],
        graph: &impl GraphRepr,
        env: &mut ProofEnvironment,
    ) -> Result<ProofGate<ManifoldOutput>>;

    /// Compute optimal curvature for the hyperbolic component.
    ///
    /// kappa* = -4 * delta^2 / diam(G)^2
    /// where delta is Gromov hyperbolicity (tree-likeness).
    /// Uses ruvector-solver for sublinear graph traversal.
    pub fn estimate_optimal_curvature(
        &self,
        graph: &impl GraphRepr,
    ) -> f32;
}
```

### CurvatureAdaptiveRouter

Routes attention to the geometrically appropriate manifold component:

```rust
/// Curvature-adaptive attention routing.
///
/// Analyzes local graph structure around each node to determine
/// which manifold component should receive the most attention weight.
/// Hierarchical neighborhoods (high tree-likeness) route to H^m;
/// clustered neighborhoods (many triangles) route to S^n;
/// flat/uniform neighborhoods route to R^k.
///
/// Bridges to ruvector-attention::curvature::{fused_attention, tangent_space}.
pub struct CurvatureAdaptiveRouter {
    /// Fused attention for computing all components.
    fused_attention: MixedCurvatureFusedAttention,
    /// Tangent space mapper for local curvature estimation.
    tangent_mapper: TangentSpaceMapper,
    /// Learned routing weights per node.
    routing_dim: usize,
}

impl CurvatureAdaptiveRouter {
    /// Route attention based on local graph curvature.
    ///
    /// For each node v, computes local Ollivier-Ricci curvature
    /// (via neighbor overlap heuristic) and routes:
    /// - kappa < -threshold -> hyperbolic component (H^m)
    /// - kappa > +threshold -> spherical component (S^n)
    /// - |kappa| <= threshold -> Euclidean component (R^k)
    ///
    /// The routing decision is soft (sigmoid gating), not hard,
    /// so gradients flow through all components.
    pub fn forward(
        &self,
        features: &[f32],
        graph: &impl GraphRepr,
        env: &mut ProofEnvironment,
    ) -> Result<ProofGate<RoutedOutput>>;
}
```

### GeodesicMessagePassing

Message passing with parallel transport along shortest paths:

```rust
/// Geodesic message passing with Levi-Civita parallel transport.
///
/// Standard message passing aggregates: m_v = sum alpha_{vu} * W * h_u.
/// This assumes all values live in the same vector space (Euclidean).
/// On a manifold, values at different nodes live in different tangent
/// spaces. Aggregation requires parallel transport from T_{h_u}M
/// to T_{h_v}M along the geodesic connecting h_u and h_v.
///
/// For Poincare ball: transport uses gyration (Thomas precession).
/// For hyperboloid: transport uses Lorentz boost.
/// For sphere: transport uses rotation along great circle.
pub struct GeodesicMessagePassing {
    /// Manifold type for transport computation.
    manifold: ManifoldType,
    /// Attention mechanism for computing weights.
    attention: Box<dyn SublinearGraphAttention>,
    /// Proof requirement: transport preserves vector norm.
    transport_proof: ProofRequirement,
}

pub enum ManifoldType {
    /// Poincare ball B^n_c with curvature c.
    PoincareBall { curvature: f32 },
    /// Lorentz hyperboloid H^n_c.
    Lorentz { curvature: f32 },
    /// Unit sphere S^n.
    Sphere,
    /// Product manifold with per-component types.
    Product(Vec<ManifoldType>),
}

impl GeodesicMessagePassing {
    /// Forward pass with parallel transport.
    ///
    /// For each edge (u, v) with attention weight alpha_{vu}:
    /// 1. Compute geodesic from h_u to h_v on the manifold.
    /// 2. Parallel transport W * h_u along geodesic to T_{h_v}M.
    /// 3. Aggregate transported values in T_{h_v}M.
    /// 4. Map back to manifold via exponential map.
    ///
    /// Proof gate: verifies ||transported_v||_g = ||v||_g (transport
    /// preserves the Riemannian norm). Routes to ProofTier::Reflex
    /// for norm comparison.
    pub fn forward(
        &self,
        features: &[f32],
        graph: &impl GraphRepr,
        env: &mut ProofEnvironment,
    ) -> Result<ProofGate<GeodesicOutput>>;

    /// Compute Frechet mean of neighbor embeddings on the manifold.
    ///
    /// Uses iterative Riemannian gradient descent:
    /// mu_{t+1} = Exp_{mu_t}(-eta * sum_i w_i * Log_{mu_t}(x_i))
    /// Converges in O(1/epsilon) steps for non-negative curvature.
    pub fn frechet_mean(
        &self,
        points: &[f32],
        weights: &[f32],
        dim: usize,
    ) -> Vec<f32>;
}
```

### RiemannianAdamOptimizer

Riemannian Adam for training on product manifolds:

```rust
/// Riemannian Adam optimizer for product manifold parameters.
///
/// Extends ruvector-attention::training::optimizer with Riemannian
/// operations: exponential map for parameter updates, parallel
/// transport for momentum, and Riemannian gradient rescaling.
///
/// Uses existing poincare.rs exp_map/log_map and
/// lorentz_cascade.rs tangent operations.
pub struct RiemannianAdamOptimizer {
    /// Learning rate.
    lr: f64,
    /// Beta1 for first moment.
    beta1: f64,
    /// Beta2 for second moment.
    beta2: f64,
    /// Epsilon for numerical stability.
    epsilon: f64,
    /// Manifold type for exp/log map selection.
    manifold: ManifoldType,
    /// First moment estimates (in tangent space).
    m: Vec<f32>,
    /// Second moment estimates (scalar, no transport needed).
    v: Vec<f32>,
    /// Step counter.
    t: u64,
}

impl RiemannianAdamOptimizer {
    /// One optimization step on the product manifold.
    ///
    /// 1. Compute Riemannian gradient: rescale Euclidean grad by
    ///    inverse metric (conformal factor for Poincare).
    /// 2. Update first moment with parallel transport from old
    ///    tangent space to new tangent space.
    /// 3. Update second moment (scalar, no transport).
    /// 4. Bias-corrected update in tangent space.
    /// 5. Exponential map back to manifold.
    ///
    /// Proof gate: verifies updated parameters remain on manifold
    /// (c * ||x||^2 < 1 for Poincare, <x,x>_L = -1/c for Lorentz).
    /// Routes to ProofTier::Reflex (norm check).
    pub fn step(
        &mut self,
        params: &mut [f32],
        grad: &[f32],
        env: &mut ProofEnvironment,
    ) -> Result<ProofGate<OptimizerStep>>;
}
```

### Lie Group Equivariance via Sheaf Bundle

SE(3)/SO(3) equivariance for 3D molecular and protein graphs:

```rust
/// Lie group equivariant attention via sheaf bundle structure.
///
/// Models the graph as a principal G-bundle where G is a Lie group
/// (SE(3) for rigid body, SO(3) for rotation). The fiber at each
/// node is a copy of G, and restriction maps from
/// ruvector-attention::sheaf serve as the connection (parallel
/// transport of G-representations along edges).
///
/// This is the manifold generalization of gauge-equivariant MP
/// (ADR-051): gauge invariance is Lie group equivariance where
/// the gauge group is a Lie group.
pub struct LieGroupEquivariantAttention {
    /// Sheaf attention for bundle structure.
    sheaf_attention: SheafAttention,
    /// Lie group type.
    group: LieGroupType,
    /// Irreducible representation degrees (for SO(3): l = 0, 1, 2, ...).
    irrep_degrees: Vec<usize>,
}

pub enum LieGroupType {
    /// Special orthogonal group SO(3): rotations in 3D.
    SO3,
    /// Special Euclidean group SE(3): rotations + translations in 3D.
    SE3,
    /// Unitary group U(1): phase rotations (electromagnetism gauge).
    U1,
}

impl LieGroupEquivariantAttention {
    /// Equivariant forward pass.
    ///
    /// Decomposes features into irreducible representations (irreps)
    /// of the Lie group. For SO(3), these are spherical harmonics
    /// at each degree l. Attention is computed per-irrep using
    /// Clebsch-Gordan coefficients for tensor products.
    ///
    /// Proof gate: verifies equivariance by checking that a random
    /// group element g applied to input produces g-transformed output.
    /// Routes to ProofTier::Deep (requires forward pass with
    /// transformed input).
    pub fn forward(
        &self,
        features: &[f32],
        positions: &[f32],    // 3D coordinates for SE(3)/SO(3)
        graph: &impl GraphRepr,
        env: &mut ProofEnvironment,
    ) -> Result<ProofGate<EquivariantOutput>>;
}
```

### Proof-Gated Manifold Invariants

| Operation | Proof Requirement | Tier | Latency |
|-----------|------------------|------|---------|
| Poincare ball containment | `c * \|\|x\|\|^2 < 1 - margin` | Reflex | < 10 ns |
| Sphere normalization | `\|\|x_S\|\| = 1 +/- eps` | Reflex | < 10 ns |
| Hyperboloid constraint | `<x,x>_L = -1/c +/- eps` | Reflex | < 10 ns |
| Transport norm preservation | `\|\|Gamma(v)\|\|_g = \|\|v\|\|_g` | Reflex | < 10 ns |
| Curvature positivity | `c > 0` | Reflex | < 10 ns |
| Frechet mean convergence | Residual norm < atol | Standard(200) | < 2 us |
| Equivariance check | Random group test | Deep | < 100 us |
| Optimal curvature estimation | Graph traversal for Gromov delta | Standard(500) | < 10 us |

### Feature Flag

```toml
# In crates/ruvector-graph-transformer/Cargo.toml
[features]
manifold = [
    "ruvector-attention/math",
]
```

The `math` feature on `ruvector-attention` gates the hyperbolic, curvature, sheaf, and transport submodules. For Lie group equivariance, an additional sub-feature is available:

```toml
manifold-lie = ["manifold", "ruvector-attention/sheaf"]
```

## Consequences

### Positive

- Hyperbolic components embed hierarchies with O(log n) dimensions instead of O(n) in Euclidean space, reducing model size by orders of magnitude for tree-like graphs
- Spherical components capture cyclic/cluster structure without wasting capacity on non-existent hierarchy
- Curvature compatibility proofs prevent NaN/Inf from mismatched curvature parameters, a common silent failure mode in mixed-curvature training
- Geodesic message passing with parallel transport is geometrically correct, unlike Euclidean aggregation in curved spaces which introduces systematic bias
- Riemannian Adam enables direct optimization on the product manifold without projection bias
- Lie group equivariance guarantees SE(3)/SO(3) symmetry for molecular and protein graphs

### Negative

- Poincare ball operations near the boundary (||x|| -> 1/sqrt(c)) suffer from numerical instability; epsilon-buffered projection mitigates but introduces small errors
- Frechet mean iteration does not have closed-form convergence rate for negative curvature; may require many iterations for widely-spread point sets
- Riemannian Adam adds ~2x overhead per step compared to Euclidean Adam due to exp/log map computations (mitigated by tangent-space approximation for small step sizes)
- Lie group equivariance via Clebsch-Gordan coefficients is O(l^3) per tensor product at degree l; high-degree irreps are expensive

### Risks

- Learned curvatures may collapse to zero (degenerate flat limit), losing the benefit of curved geometry. Mitigation: curvature lower bound enforced via proof gate (c > c_min = 0.01)
- Mixed-curvature training is known to be sensitive to learning rate; too-large steps may leave the manifold. Mitigation: Riemannian Adam with manifold constraint proofs at every step
- Component quantization (from `ruvector-attention::curvature::component_quantizer`) interacts poorly with curvature -- quantization errors in hyperbolic space are amplified by the metric near the boundary. Mitigation: use higher quantization precision for hyperbolic components

## Implementation

1. Create `crates/ruvector-graph-transformer/src/manifold/mod.rs` re-exporting all types
2. Implement `ProductManifoldAttention` in `manifold/product.rs`, bridging to `ruvector-attention::curvature::fused_attention` and `ruvector-attention::hyperbolic::lorentz_cascade`
3. Implement `CurvatureAdaptiveRouter` in `manifold/router.rs`, bridging to `ruvector-attention::curvature::tangent_space`
4. Implement `GeodesicMessagePassing` in `manifold/geodesic.rs`, using `ruvector-attention::hyperbolic::poincare` for exp/log/transport
5. Implement `RiemannianAdamOptimizer` in `manifold/optimizer.rs`, extending `ruvector-attention::training::optimizer`
6. Implement `LieGroupEquivariantAttention` in `manifold/lie_group.rs`, bridging to `ruvector-attention::sheaf::{SheafAttention, RestrictionMap}`
7. Add benchmark: `benches/manifold_bench.rs` measuring mixed-curvature attention throughput on a 50K-node hierarchical graph
8. Integration test: product manifold attention on a synthetic graph with known curvature, verify embedding distortion is lower than Euclidean baseline
9. Verify build: `cargo test --features manifold -p ruvector-graph-transformer`

## References

- ADR-046: Graph Transformer Unified Architecture (module structure, `manifold` feature flag, `mixed_curvature.rs` bridge)
- ADR-047: Proof-Gated Mutation Protocol (`ProofGate<T>`, manifold containment invariants)
- ADR-049: Verified Training Pipeline (Riemannian optimization verification during training)
- ADR-051: Physics-Informed Graph Layers (gauge equivariance via sheaf, related to Lie group equivariance)
- Research: `docs/research/gnn-v2/27-hyperbolic-mixed-curvature-graph-transformers.md`
- `crates/ruvector-attention/src/hyperbolic/poincare.rs`: `mobius_add`, `mobius_scalar_mult`, `frechet_mean`, `exp_map`, `log_map`
- `crates/ruvector-attention/src/hyperbolic/lorentz_cascade.rs`: `LorentzCascadeAttention`, Busemann scoring, Einstein midpoint
- `crates/ruvector-attention/src/hyperbolic/mixed_curvature.rs`: `MixedCurvatureAttention`
- `crates/ruvector-attention/src/curvature/fused_attention.rs`: `MixedCurvatureFusedAttention`, `FusedCurvatureConfig`
- `crates/ruvector-attention/src/curvature/tangent_space.rs`: `TangentSpaceMapper`
- `crates/ruvector-attention/src/curvature/component_quantizer.rs`: mixed-curvature quantization
- `crates/ruvector-attention/src/sheaf/restriction.rs`: `RestrictionMap`
- `crates/ruvector-attention/src/sheaf/attention.rs`: `SheafAttention`
- `crates/ruvector-attention/src/transport/sliced_wasserstein.rs`: `SlicedWassersteinAttention`
- `crates/ruvector-attention/src/training/optimizer.rs`: base optimizer
- `crates/ruvector-verified/src/gated.rs`: `ProofTier`, `route_proof`
- Nickel & Kiela, "Poincare Embeddings for Learning Hierarchical Representations" (NeurIPS, 2017)
- Gu et al., "Learning Mixed-Curvature Representations in Product Spaces" (ICLR, 2019)
- Chami et al., "Hyperbolic Graph Convolutional Neural Networks" (NeurIPS, 2019)
- Becigneul & Ganea, "Riemannian Adaptive Optimization Methods" (ICLR, 2019)
- Fuchs et al., "SE(3)-Transformers: 3D Roto-Translation Equivariant Attention Networks" (NeurIPS, 2020)
