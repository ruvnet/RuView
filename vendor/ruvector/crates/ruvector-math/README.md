# ruvector-math

Advanced Mathematics for Next-Generation Vector Search

[![Crates.io](https://img.shields.io/crates/v/ruvector-math.svg)](https://crates.io/crates/ruvector-math)
[![Documentation](https://docs.rs/ruvector-math/badge.svg)](https://docs.rs/ruvector-math)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## What is ruvector-math?

**ruvector-math** brings advanced mathematical tools to vector search and AI systems. Think of it as a Swiss Army knife for working with high-dimensional data, embeddings, and neural networks.

### The Core Idea: Mincut as the Governance Signal

All modules in this library connect through a single unifying concept: **mincut** (minimum cut). Mincut measures how "connected" a graph is - specifically, how much you'd need to cut to separate it into parts.

In AI systems, mincut tells us:
- **Low mincut (near 0)**: The system is stable - use fast, simple processing
- **High mincut**: The system is changing - be cautious, use more careful methods
- **Very high mincut**: Major shifts detected - pause and re-evaluate

This "governance dial" lets AI systems automatically adjust their behavior based on the structure of the data they're processing.

### Five Theoretical CS Modules

1. **Tropical Algebra** - Piecewise linear math for neural networks
   - Uses max/min instead of multiply/add
   - Reveals the "skeleton" of how neural networks make decisions
   - *Example*: Find the shortest path in a graph, or count linear regions in a ReLU network

2. **Tensor Networks** - Compress high-dimensional data dramatically
   - Break big tensors into chains of small ones
   - *Example*: Store a 1000x1000x1000 tensor using only ~1% of the memory

3. **Spectral Methods** - Work with graphs without expensive matrix operations
   - Use Chebyshev polynomials to approximate filters
   - *Example*: Smooth a signal on a social network graph, or cluster nodes

4. **Persistent Homology (TDA)** - Find shapes in data that persist across scales
   - Track holes, loops, and voids as you zoom in/out
   - *Example*: Detect when data is drifting by watching for topological changes

5. **Polynomial Optimization** - Prove mathematical facts about polynomials
   - Check if a function is always non-negative
   - *Example*: Verify that a neural network's output is bounded

### How They Work Together

```
                    ┌─────────────────────────────────────┐
                    │     MINCUT (Stoer-Wagner)          │
                    │    "Is the system stable?"          │
                    └──────────────┬──────────────────────┘
                                   │
          ┌────────────────────────┼────────────────────────┐
          │                        │                        │
          ▼                        ▼                        ▼
   λ ≈ 0 (Stable)          λ moderate              λ high (Drift)
   ┌──────────────┐      ┌──────────────┐       ┌──────────────┐
   │ Fast Path    │      │ Cautious     │       │ Freeze       │
   │ SSM backbone │      │ Governed ATT │       │ Re-evaluate  │
   │ Tropical     │      │ Spectral     │       │ TDA detect   │
   │ analysis     │      │ filtering    │       │ boundaries   │
   └──────────────┘      └──────────────┘       └──────────────┘
```

## Overview

`ruvector-math` provides production-grade implementations of advanced mathematical algorithms that differentiate RuVector from traditional vector databases:

| Algorithm | Purpose | Speedup | Use Case |
|-----------|---------|---------|----------|
| **Sliced Wasserstein** | Distribution comparison | ~1000x vs exact OT | Cross-lingual search, image retrieval |
| **Sinkhorn Algorithm** | Entropic optimal transport | ~100x vs LP | Document similarity, time series |
| **Gromov-Wasserstein** | Cross-space structure matching | N/A (unique) | Multi-modal alignment |
| **Fisher Information** | Parameter space geometry | 3-5x convergence | Index optimization |
| **Natural Gradient** | Curvature-aware optimization | 3-5x fewer iterations | Embedding training |
| **K-FAC** | Scalable natural gradient | O(n) vs O(n²) | Neural network training |
| **Product Manifolds** | Mixed-curvature spaces | 20x memory reduction | Taxonomy + cyclical data |
| **Spherical Geometry** | Operations on S^n | Native | Cyclical patterns |

## Features

- **Pure Rust**: No BLAS/LAPACK dependencies for full WASM compatibility
- **SIMD-Ready**: Hot paths optimized for auto-vectorization
- **Numerically Stable**: Log-domain arithmetic, clamping, and stable softmax
- **Modular**: Each component usable independently
- **WebAssembly**: Full browser support via `ruvector-math-wasm`

## Installation

```toml
[dependencies]
ruvector-math = "0.1"
```

For WASM:
```toml
[dependencies]
ruvector-math-wasm = "0.1"
```

## Quick Start

### Optimal Transport

```rust
use ruvector_math::optimal_transport::{SlicedWasserstein, SinkhornSolver, OptimalTransport};

// Sliced Wasserstein: Fast distribution comparison
let sw = SlicedWasserstein::new(100) // 100 random projections
    .with_power(2.0)                 // W2 distance
    .with_seed(42);                  // Reproducible

let source = vec![vec![0.0, 0.0], vec![1.0, 0.0], vec![0.0, 1.0]];
let target = vec![vec![2.0, 0.0], vec![3.0, 0.0], vec![2.0, 1.0]];

let distance = sw.distance(&source, &target);
println!("Sliced Wasserstein distance: {:.4}", distance);

// Sinkhorn: Get optimal transport plan
let sinkhorn = SinkhornSolver::new(0.1, 100); // regularization, max_iters
let result = sinkhorn.solve(&cost_matrix, &weights_a, &weights_b)?;

println!("Transport cost: {:.4}", result.cost);
println!("Converged in {} iterations", result.iterations);
```

### Information Geometry

```rust
use ruvector_math::information_geometry::{FisherInformation, NaturalGradient};

// Compute Fisher Information Matrix from gradient samples
let fisher = FisherInformation::new().with_damping(1e-4);
let fim = fisher.empirical_fim(&gradient_samples)?;

// Natural gradient for faster optimization
let mut optimizer = NaturalGradient::new(0.01)
    .with_diagonal(true)  // Use diagonal approximation
    .with_damping(1e-4);

let update = optimizer.step(&gradient, Some(&gradient_samples))?;
```

### Product Manifolds

```rust
use ruvector_math::product_manifold::{ProductManifold, ProductManifoldConfig};

// Create E^64 × H^16 × S^8 product manifold
let manifold = ProductManifold::new(64, 16, 8);

// Project point onto manifold
let point = manifold.project(&raw_point)?;

// Compute geodesic distance
let dist = manifold.distance(&point_a, &point_b)?;

// Fréchet mean (centroid on manifold)
let mean = manifold.frechet_mean(&points, None)?;

// K-nearest neighbors
let neighbors = manifold.knn(&query, &database, 10)?;
```

### Spherical Geometry

```rust
use ruvector_math::spherical::SphericalSpace;

// Create S^{127} (128-dimensional unit sphere)
let sphere = SphericalSpace::new(128);

// Project to sphere
let unit_vec = sphere.project(&raw_vector)?;

// Geodesic distance (great-circle)
let dist = sphere.distance(&x, &y)?;

// Interpolate along geodesic
let midpoint = sphere.geodesic(&x, &y, 0.5)?;

// Parallel transport tangent vector
let transported = sphere.parallel_transport(&x, &y, &v)?;
```

## Algorithm Details

### Optimal Transport

#### Sliced Wasserstein Distance

The Sliced Wasserstein distance approximates the Wasserstein distance by averaging 1D Wasserstein distances along random projections:

```
SW_p(μ, ν) = (∫_{S^{d-1}} W_p(Proj_θ μ, Proj_θ ν)^p dθ)^{1/p}
```

**Complexity**: O(L × n log n) where L = projections, n = points

**When to use**:
- Comparing embedding distributions across languages
- Image region similarity
- Time series pattern matching

#### Sinkhorn Algorithm

Solves entropic-regularized optimal transport:

```
min_{γ ∈ Π(a,b)} ⟨γ, C⟩ - ε H(γ)
```

Uses log-domain stabilization to prevent numerical overflow.

**Complexity**: O(n² × iterations), typically ~100 iterations

**When to use**:
- Document similarity with word distributions
- Soft matching between sets
- Computing transport plans (not just distances)

#### Gromov-Wasserstein

Compares metric spaces without shared embedding:

```
GW(X, Y) = min_{γ} Σ |d_X(i,k) - d_Y(j,l)|² γ_ij γ_kl
```

**When to use**:
- Cross-modal retrieval (text ↔ image)
- Graph matching
- Shape comparison

### Information Geometry

#### Fisher Information Matrix

Captures curvature of the log-likelihood surface:

```
F(θ) = E[∇log p(x|θ) ∇log p(x|θ)^T]
```

#### Natural Gradient

Updates parameters along geodesics in probability space:

```
θ_{t+1} = θ_t - η F(θ)^{-1} ∇L(θ)
```

**Benefits**:
- Invariant to parameterization
- 3-5x faster convergence than Adam
- Better generalization

#### K-FAC

Kronecker-factored approximation for scalable natural gradient:

```
F_W ≈ E[gg^T] ⊗ E[aa^T]
```

Reduces storage from O(n²) to O(n) and inversion from O(n³) to O(n^{3/2}).

### Product Manifolds

Combines three geometric spaces:

| Space | Curvature | Best For |
|-------|-----------|----------|
| Euclidean E^n | 0 | General embeddings |
| Hyperbolic H^n | < 0 | Hierarchies, trees |
| Spherical S^n | > 0 | Cyclical patterns |

**Distance in product space**:
```
d(x, y)² = w_e·d_E(x_e, y_e)² + w_h·d_H(x_h, y_h)² + w_s·d_S(x_s, y_s)²
```

## WASM Usage

```typescript
import {
  WasmSlicedWasserstein,
  WasmProductManifold
} from 'ruvector-math-wasm';

// Sliced Wasserstein in browser
const sw = new WasmSlicedWasserstein(100);
const distance = sw.distance(sourceFlat, targetFlat, dim);

// Product manifold operations
const manifold = new WasmProductManifold(64, 16, 8);
const projected = manifold.project(rawPoint);
const dist = manifold.distance(pointA, pointB);
```

## Benchmarks

Run benchmarks:

```bash
cargo bench -p ruvector-math
```

### Sample Results (M1 MacBook Pro)

| Operation | n=1000, dim=128 | Throughput |
|-----------|-----------------|------------|
| Sliced Wasserstein (100 proj) | 2.1 ms | 476 ops/s |
| Sliced Wasserstein (500 proj) | 8.5 ms | 117 ops/s |
| Sinkhorn (ε=0.1) | 15.2 ms | 65 ops/s |
| Product Manifold distance | 0.8 μs | 1.25M ops/s |
| Spherical geodesic | 0.3 μs | 3.3M ops/s |
| Diagonal FIM (100 samples) | 0.5 ms | 2K ops/s |

## Theory References

### Optimal Transport
- Peyré & Cuturi (2019): [Computational Optimal Transport](https://arxiv.org/abs/1803.00567)
- Bonneel et al. (2015): Sliced and Radon Wasserstein Barycenters

### Information Geometry
- Amari & Nagaoka (2000): Methods of Information Geometry
- Martens & Grosse (2015): Optimizing Neural Networks with K-FAC

### Mixed-Curvature Spaces
- Gu et al. (2019): Learning Mixed-Curvature Representations
- Nickel & Kiela (2018): Learning Continuous Hierarchies in the Lorentz Model

## API Reference

### Optimal Transport

```rust
// Sliced Wasserstein
SlicedWasserstein::new(num_projections: usize) -> Self
  .with_power(p: f64) -> Self           // W_p distance
  .with_seed(seed: u64) -> Self         // Reproducibility
  .distance(&source, &target) -> f64
  .weighted_distance(&source, &source_w, &target, &target_w) -> f64

// Sinkhorn
SinkhornSolver::new(regularization: f64, max_iterations: usize) -> Self
  .with_threshold(threshold: f64) -> Self
  .solve(&cost_matrix, &a, &b) -> Result<TransportPlan>
  .distance(&source, &target) -> Result<f64>
  .barycenter(&distributions, weights, support_size, dim) -> Result<Vec<Vec<f64>>>

// Gromov-Wasserstein
GromovWasserstein::new(regularization: f64) -> Self
  .with_max_iterations(max_iter: usize) -> Self
  .solve(&source, &target) -> Result<GromovWassersteinResult>
  .distance(&source, &target) -> Result<f64>
```

### Information Geometry

```rust
// Fisher Information
FisherInformation::new() -> Self
  .with_damping(damping: f64) -> Self
  .empirical_fim(&gradients) -> Result<Vec<Vec<f64>>>
  .diagonal_fim(&gradients) -> Result<Vec<f64>>
  .natural_gradient(&fim, &gradient) -> Result<Vec<f64>>

// Natural Gradient
NaturalGradient::new(learning_rate: f64) -> Self
  .with_diagonal(use_diagonal: bool) -> Self
  .with_damping(damping: f64) -> Self
  .step(&gradient, gradient_samples) -> Result<Vec<f64>>
  .optimize_step(&mut params, &gradient, samples) -> Result<f64>

// K-FAC
KFACApproximation::new(&layer_dims) -> Self
  .update_layer(idx, &activations, &gradients) -> Result<()>
  .natural_gradient_layer(idx, &weight_grad) -> Result<Vec<Vec<f64>>>
```

### Product Manifolds

```rust
ProductManifold::new(euclidean_dim, hyperbolic_dim, spherical_dim) -> Self
  .project(&point) -> Result<Vec<f64>>
  .distance(&x, &y) -> Result<f64>
  .exp_map(&x, &v) -> Result<Vec<f64>>
  .log_map(&x, &y) -> Result<Vec<f64>>
  .geodesic(&x, &y, t) -> Result<Vec<f64>>
  .frechet_mean(&points, weights) -> Result<Vec<f64>>
  .knn(&query, &points, k) -> Result<Vec<(usize, f64)>>
  .pairwise_distances(&points) -> Result<Vec<Vec<f64>>>

SphericalSpace::new(ambient_dim: usize) -> Self
  .project(&point) -> Result<Vec<f64>>
  .distance(&x, &y) -> Result<f64>
  .exp_map(&x, &v) -> Result<Vec<f64>>
  .log_map(&x, &y) -> Result<Vec<f64>>
  .geodesic(&x, &y, t) -> Result<Vec<f64>>
  .parallel_transport(&x, &y, &v) -> Result<Vec<f64>>
  .frechet_mean(&points, weights) -> Result<Vec<f64>>
```

## License

MIT License - see [LICENSE](LICENSE) for details.

## Contributing

Contributions welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.
