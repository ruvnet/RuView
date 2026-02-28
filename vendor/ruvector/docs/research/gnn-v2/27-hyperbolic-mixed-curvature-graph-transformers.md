# Hyperbolic and Mixed-Curvature Graph Transformers: Product Manifold Attention

## Overview

### Problem Statement

Graph Transformers have become the dominant architecture for learning on relational data, yet nearly all deployed systems operate in flat Euclidean space. This is a geometric mismatch: most real-world graphs are not flat.

**Why Euclidean space fails for real-world graphs:**

1. **Power-law degree distributions** (social networks, citation graphs, the web) exhibit tree-like branching that requires exponentially many dimensions in Euclidean space to embed without distortion. A binary tree of depth $d$ has $2^d$ leaves, but fitting them equidistantly in $\mathbb{R}^n$ requires $n \geq 2^d - 1$ dimensions.
2. **Hierarchical structures** (taxonomies, organizational charts, ontologies) naturally live in hyperbolic space, where the volume of a ball grows exponentially with radius -- matching the exponential growth of tree levels.
3. **Cyclic substructures** (molecular rings, periodic lattices, social cliques) have positive curvature and embed naturally on spheres $S^n$.
4. **Hybrid graphs** (knowledge graphs combining hierarchies with lateral associations) require multiple curvature regimes simultaneously.

The consequence: flat-space Graph Transformers waste capacity representing geometric structure that is free in the correct curved space, leading to higher distortion, larger models, and slower convergence.

### Proposed Solution

Develop **Product Manifold Graph Transformers** that operate natively on mixed-curvature spaces. The core decomposition is:

$$\mathcal{M} = S^{n_1} \times H^{n_2} \times \mathbb{R}^{n_3}$$

where $S^{n_1}$ captures cyclic/clustered structure, $H^{n_2}$ captures hierarchical structure, and $\mathbb{R}^{n_3}$ captures flat semantic similarity. Every component of the attention mechanism -- queries, keys, values, aggregation, and optimization -- operates in its geometrically appropriate space.

### Connection to RuVector

RuVector already has substantial infrastructure for this research direction:

- **`ruvector-attention/src/hyperbolic/`**: Poincare ball operations (`poincare.rs`), Lorentz cascade attention with Busemann scoring and Einstein midpoint (`lorentz_cascade.rs`), mixed-curvature attention (`mixed_curvature.rs`)
- **`ruvector-attention/src/curvature/`**: Fused E x H x S attention (`fused_attention.rs`), tangent space mapping (`tangent_space.rs`), component quantizer (`component_quantizer.rs`)
- **`ruvector-attention/src/transport/`**: Sliced Wasserstein and centroid optimal transport attention
- **`ruvector-attention/src/topology/`**: Topology-gated attention with coherence metrics
- **`ruvector-graph/`**: Full property graph with Cypher queries, distributed federation, hybrid vector-graph search
- **`ruvector-solver/`**: Sublinear graph solvers (forward/backward push, CG, random walk, BMSSP)

This document extends RuVector's existing mixed-curvature capabilities toward full product manifold Graph Transformers with learned curvature fields.

---

## Technical Deep Dive

### 1. Hyperbolic Graph Transformers

#### Poincare Ball Attention

In the Poincare ball model $\mathbb{B}^n_c = \{x \in \mathbb{R}^n : c\|x\|^2 < 1\}$, the standard dot-product attention $\text{softmax}(QK^T / \sqrt{d})$ is replaced with geodesic attention:

$$\alpha_{ij} = \frac{\exp(-d_{\mathbb{B}}(q_i, k_j) / \tau)}{\sum_l \exp(-d_{\mathbb{B}}(q_i, k_l) / \tau)}$$

where $d_{\mathbb{B}}(x, y) = \frac{1}{\sqrt{c}} \text{arcosh}\left(1 + \frac{2c\|x - y\|^2}{(1 - c\|x\|^2)(1 - c\|y\|^2)}\right)$.

RuVector's `poincare.rs` already implements this with numerical stability via epsilon-buffered projection. The key insight from Lorentz cascade attention (`lorentz_cascade.rs`) is that the **Lorentz model avoids boundary instability entirely**: points live on the hyperboloid $\{x : \langle x, x \rangle_L = -1/c, x_0 > 0\}$ rather than inside a ball, and attention scores reduce to Busemann functions (single dot products).

#### Lorentz Model Message Passing

In the Lorentz model, message passing between graph nodes proceeds as:

1. **Embed** each node $v$ onto the hyperboloid: $h_v \in H^n_c$
2. **Attend** using Busemann scoring: $B_\xi(x) = \ln(-\langle x, \xi \rangle_L)$, where $\xi$ is a light-like focal direction defining the hierarchy
3. **Aggregate** via Einstein midpoint (closed-form, unlike iterative Frechet mean): $\bar{h} = \text{proj}_H\left(\sum_i w_i \gamma_i h_i / \|\sum_i w_i \gamma_i h_i\|_L\right)$ where $\gamma_i$ is the Lorentz factor

RuVector's `LorentzCascadeAttention` implements this with multi-curvature heads operating at logarithmically-spaced curvatures, capturing hierarchy at multiple scales simultaneously.

#### Gyrovector Aggregation

Standard weighted averaging in Euclidean space ($\bar{v} = \sum_i w_i v_i$) does not preserve the Poincare ball constraint. Instead, aggregation must use Mobius operations:

$$\text{AGGREGATE}(\{(w_i, v_i)\}) = \bigoplus_{i=1}^n (w_i \otimes_c v_i)$$

where $\oplus_c$ is Mobius addition and $\otimes_c$ is Mobius scalar multiplication. RuVector's `poincare.rs` provides `mobius_add` and `mobius_scalar_mult` with full numerical stability.

The practical limitation is that Mobius aggregation is sequential -- each addition depends on the previous result. The Frechet mean (`frechet_mean` in RuVector) offers a parallel alternative via Riemannian gradient descent in the tangent space.

### 2. Mixed-Curvature Product Manifolds

#### $S^n \times H^m \times \mathbb{R}^k$ Decomposition

A product manifold $\mathcal{M} = \mathcal{M}_1 \times \mathcal{M}_2 \times \cdots \times \mathcal{M}_p$ has the metric:

$$d_{\mathcal{M}}(x, y)^2 = \sum_{i=1}^p \beta_i \cdot d_{\mathcal{M}_i}(x^{(i)}, y^{(i)})^2$$

where $\beta_i$ are learnable mixing weights and each $\mathcal{M}_i$ is either spherical ($\kappa_i > 0$), hyperbolic ($\kappa_i < 0$), or Euclidean ($\kappa_i = 0$).

RuVector's `FusedCurvatureConfig` already defines this decomposition:

```rust
pub struct FusedCurvatureConfig {
    pub euclidean_dim: usize,     // R^k component
    pub hyperbolic_dim: usize,    // H^m component
    pub spherical_dim: usize,     // S^n component
    pub weight_e: f32,            // beta_E
    pub weight_h: f32,            // beta_H
    pub weight_s: f32,            // beta_S
    pub hyperbolic_curvature: f32,
}
```

The fused attention kernel computes all three similarities in a single vectorized pass:

$$\text{logit}(q, k) = \beta_E \langle q_E, k_E \rangle + \beta_H \langle q_{H}^{\text{tan}}, k_{H}^{\text{tan}} \rangle + \beta_S \langle q_S, k_S \rangle_S$$

where the hyperbolic component uses tangent-space dot products (10-100x faster than geodesic distance per RuVector's `TangentSpaceMapper`) and the spherical component uses normalized inner products on the unit sphere.

#### Curvature-Per-Component

Rather than a single global curvature, each dimension group can have its own learned curvature. For a product of $p$ components:

$$\mathcal{M} = \mathcal{M}_1^{\kappa_1} \times \mathcal{M}_2^{\kappa_2} \times \cdots \times \mathcal{M}_p^{\kappa_p}$$

This is the key extension beyond RuVector's current `MixedCurvatureConfig` (which uses a single curvature for the hyperbolic component). The research direction is to make $\kappa_i$ **learnable per-component**, enabling the model to discover which curvature best fits each subspace of the embedding.

#### Optimal Curvature Learning

Given a graph $G = (V, E)$ with known structure, the optimal curvature for a hyperbolic component can be estimated as:

$$\kappa^* = -\frac{4\delta^2}{(\text{diam}(G))^2}$$

where $\delta$ is the Gromov hyperbolicity (measuring how tree-like the graph is) and $\text{diam}(G)$ is the graph diameter. RuVector's solver crate provides the graph traversal primitives needed to compute both quantities sublinearly.

For learnable curvatures during training, the gradient flows through the exponential map:

$$\frac{\partial \mathcal{L}}{\partial \kappa} = \frac{\partial \mathcal{L}}{\partial d_\kappa} \cdot \frac{\partial d_\kappa}{\partial \kappa}$$

The curvature gradient for the Poincare distance is:

$$\frac{\partial d_c}{\partial c} = -\frac{1}{2c^{3/2}} \text{arcosh}(\alpha) + \frac{1}{\sqrt{c}} \frac{1}{\sqrt{\alpha^2 - 1}} \frac{\partial \alpha}{\partial c}$$

where $\alpha = 1 + 2c\|x - y\|^2 / ((1 - c\|x\|^2)(1 - c\|y\|^2))$.

### 3. Curvature-Adaptive Routing

#### Attention Weights as Parallel Transport

In a curved space, moving a vector from one tangent space to another requires **parallel transport** along the geodesic connecting them. Standard attention aggregation implicitly assumes all values live in the same space, which is only true in flat space.

For a message from node $j$ to node $i$, the value $v_j$ must be parallel-transported from $T_{h_j}\mathcal{M}$ to $T_{h_i}\mathcal{M}$:

$$\tilde{v}_j = \Gamma_{h_j \to h_i}(v_j)$$

In the Poincare ball, parallel transport along the geodesic from $x$ to $y$ is:

$$\Gamma_{x \to y}(v) = \frac{\lambda_x}{\lambda_y} \cdot \text{gyr}[y, -x](v)$$

where $\lambda_x = 2/(1 - c\|x\|^2)$ is the conformal factor and $\text{gyr}$ is the gyration operator (Thomas precession). This connects to RuVector's transport module (`ruvector-attention/src/transport/`), which uses optimal transport for attention -- the Wasserstein distance provides a natural way to compute transport plans between distributions on manifolds.

#### Levi-Civita Connection for Message Passing

The Levi-Civita connection $\nabla$ provides the unique torsion-free, metric-compatible way to differentiate vector fields on a manifold. For graph message passing on a Riemannian manifold $(\mathcal{M}, g)$:

$$m_{i \leftarrow j} = \alpha_{ij} \cdot \Gamma_{j \to i}^{\nabla}(W_v h_j)$$

where $\Gamma_{j \to i}^{\nabla}$ is parallel transport along the Levi-Civita connection. The Christoffel symbols $\Gamma^k_{ij}$ encode the connection in coordinates:

$$\Gamma^k_{ij} = \frac{1}{2} g^{kl}\left(\frac{\partial g_{jl}}{\partial x^i} + \frac{\partial g_{il}}{\partial x^j} - \frac{\partial g_{ij}}{\partial x^l}\right)$$

For the Poincare ball with conformal factor $\lambda_x = 2/(1 - c\|x\|^2)$, the Christoffel symbols simplify considerably, enabling efficient implementation.

### 4. Riemannian Optimization for Graph Transformers

#### Riemannian Adam

Standard Adam cannot be applied directly on manifolds because the update rule $\theta_{t+1} = \theta_t - \eta \cdot \hat{m}_t / (\sqrt{\hat{v}_t} + \epsilon)$ does not preserve manifold constraints. Riemannian Adam replaces Euclidean operations with their Riemannian counterparts:

```
Algorithm: Riemannian Adam on Product Manifold M

Input: Learning rate eta, decay rates beta_1, beta_2, parameters theta in M
Initialize: m_0 = 0, v_0 = 0 (in tangent space at theta_0)

For t = 1, 2, ...:
    g_t = Riemannian_gradient(L, theta_{t-1})   // Project Euclidean grad to tangent space
    m_t = beta_1 * PT(m_{t-1}) + (1 - beta_1) * g_t   // Parallel transport first moment
    v_t = beta_2 * v_{t-1} + (1 - beta_2) * g_t^2     // Second moment (scalar, no transport)
    m_hat = m_t / (1 - beta_1^t)
    v_hat = v_t / (1 - beta_2^t)
    update = -eta * m_hat / (sqrt(v_hat) + epsilon)
    theta_t = Exp_{theta_{t-1}}(update)   // Exponential map back to manifold
```

The key operations are:
- **Riemannian gradient**: $\text{grad}_\mathcal{M} f = \frac{1}{\lambda_x^2} \nabla_E f$ (rescale Euclidean gradient by inverse metric)
- **Exponential map**: $\text{Exp}_x(v)$ moves from $x$ in direction $v$ along the geodesic
- **Parallel transport**: $\text{PT}_{x \to y}(m)$ moves the momentum from the old tangent space to the new one

RuVector's `ruvector-attention/src/training/optimizer.rs` provides the foundation; extending it to Riemannian Adam requires adding `exp_map` and `log_map` calls (already available in `poincare.rs` and `lorentz_cascade.rs::tangent`).

#### Projection-Free Training on Manifolds

An alternative to Riemannian optimization is **projection-free training**, where parameters are optimized in the ambient Euclidean space and projected back to the manifold after each step:

$$\theta_{t+1} = \text{proj}_\mathcal{M}(\theta_t - \eta \nabla_E \mathcal{L})$$

For the Poincare ball, this is simply `project_to_ball`. For the hyperboloid, `project_hyperboloid`. For the sphere, normalize to unit length. The advantage is compatibility with existing optimizers (Adam, SGD); the disadvantage is that projection introduces bias proportional to the step size.

RuVector's tangent space approach (`TangentSpaceMapper`) offers a practical middle ground: map to tangent space at the origin, perform standard operations, then map back. This is exact for small perturbations and provides 10-100x speedup over full geodesic operations.

### 5. Lie Group Equivariant Graph Attention

#### SE(3) and SO(3) Equivariance

For molecular graphs and physical simulations, attention must respect the symmetries of 3D space. An **SE(3)-equivariant** Graph Transformer satisfies:

$$f(Rx + t, Rh) = Rf(x, h)$$

for all rotations $R \in SO(3)$ and translations $t \in \mathbb{R}^3$. This means the model's output transforms consistently with rigid body motions.

The key construction is **equivariant attention** using invariant features:

$$\alpha_{ij} = \phi\left(\|x_i - x_j\|, \langle h_i, h_j \rangle, h_i^T(x_i - x_j)\right)$$

The attention weights depend only on invariants (distances, inner products, projections), ensuring equivariance of the full attention layer. Value messages are constructed using equivariant basis functions:

$$m_{ij} = \alpha_{ij} \left(w_0 h_j + w_1 (x_i - x_j) + w_2 (x_i - x_j) \times h_j\right)$$

where the cross product ensures the message transforms correctly under rotations.

#### General Lie Group Equivariance

Beyond SE(3), graphs with symmetry group $G$ require $G$-equivariant attention. The general framework uses **fiber bundles**: each node carries a feature that transforms under a representation $\rho$ of $G$, and message passing uses intertwining operators.

For a Lie group $G$ acting on the graph, equivariant attention decomposes into irreducible representations:

$$\alpha_{ij} = \sum_l \alpha_{ij}^{(l)} \cdot \rho^{(l)}(g_{ij})$$

where $g_{ij} \in G$ is the relative group element between nodes $i$ and $j$, and $\rho^{(l)}$ is the $l$-th irreducible representation.

This connects to RuVector's sheaf attention module (`ruvector-attention/src/sheaf/`), where restriction maps between stalks play a role analogous to parallel transport between fibers in the Lie group setting.

---

## Research Timeline

### 2026-2030: Mixed-Curvature GNNs Become Standard

**Knowledge Graphs (2026-2028):** Knowledge graphs like Wikidata and Freebase combine deep hierarchies (is-a relations), lateral associations (related-to), and cyclic patterns (mutual relations). Product manifold embeddings $H^{64} \times S^{32} \times \mathbb{R}^{128}$ achieve 15-25% better link prediction than flat embeddings at half the dimensionality. RuVector's existing `FusedCurvatureConfig` provides the production-ready kernel.

**Molecular Design (2027-2029):** Drug discovery graphs have hierarchical scaffolds, cyclic ring systems, and flat functional group features. SE(3)-equivariant product manifold transformers replace flat-space message passing networks, achieving state-of-the-art on molecular property prediction benchmarks.

**Social Networks (2028-2030):** Community detection in social networks benefits from hyperbolic embeddings (communities are hierarchical), spherical embeddings (cliques are cyclic), and Euclidean embeddings (content similarity). Mixed-curvature Graph Transformers become the standard architecture for large-scale social graph analysis.

### 2030-2036: Continuous Manifold Graph Transformers

**Learned Curvature Fields (2030-2032):** Instead of a fixed product manifold, the curvature becomes a learned function of position: $\kappa(x): \mathcal{M} \to \mathbb{R}$. The manifold itself adapts to the local structure of the graph. Regions with tree-like structure automatically develop negative curvature; regions with cliques develop positive curvature; transition zones have near-zero curvature. This requires solving geodesic equations numerically on the learned manifold.

**Arbitrary Riemannian Manifolds (2032-2034):** Graph Transformers operate on manifolds defined by their learned metric tensor $g_{ij}(x)$ rather than restricted to constant-curvature spaces. The exponential map, parallel transport, and geodesic attention are computed via neural ODE solvers. RuVector's PDE attention module (`ruvector-attention/src/pde_attention/`) provides the diffusion-based foundation.

**Manifold-Valued Graph Neural Fields (2034-2036):** The discrete graph is replaced by a continuous neural field on a manifold: $f: \mathcal{M} \to \mathcal{N}$, where both the domain manifold $\mathcal{M}$ and the codomain manifold $\mathcal{N}$ are learned. Attention becomes a kernel on the product manifold $\mathcal{M} \times \mathcal{N}$. This unifies graph transformers with neural radiance fields, geometric deep learning, and topological data analysis.

---

## Architecture Proposals

### Product Manifold Attention Layer

```
Input: node embeddings x_i = (x_i^E, x_i^H, x_i^S) in R^k x H^m x S^n

For each component space M_j in {R^k, H^m, S^n}:
    Q_j = W_Q^j * x^j                     // Linear projection (in tangent space for H, S)
    K_j = W_K^j * x^j
    V_j = W_V^j * x^j

    alpha_ij^j = softmax(-d_{M_j}(Q_j_i, K_j_l) / tau_j)   // Geodesic attention
    out_j_i = AGGREGATE_{M_j}({alpha_ij^j, V_j_l})           // Manifold-aware aggregation

// Fused attention (single kernel, as in RuVector's fused_attention.rs):
alpha_ij = softmax(beta_E * <Q_E_i, K_E_j> + beta_H * <Q_H_i, K_H_j>_tan + beta_S * <Q_S_i, K_S_j>_S)

// Aggregation per component:
out_E_i = sum_j alpha_ij * V_E_j                              // Euclidean: weighted average
out_H_i = einstein_midpoint({alpha_ij, V_H_j}, c)             // Hyperbolic: Einstein midpoint
out_S_i = normalize(sum_j alpha_ij * V_S_j)                   // Spherical: weighted + project

Output: (out_E_i, out_H_i, out_S_i)
```

### Rust Pseudocode: Product Manifold Attention

```rust
/// Product manifold attention layer operating on S^n x H^m x R^k
pub struct ProductManifoldAttention {
    /// Per-component configurations with learned curvatures
    components: Vec<ManifoldComponent>,
    /// Fused attention kernel for single-pass computation
    fused_kernel: FusedCurvatureKernel,
    /// Tangent space mapper for fast hyperbolic operations
    tangent_mapper: TangentSpaceMapper,
    /// Riemannian optimizer state
    optimizer: RiemannianAdamState,
}

#[derive(Clone)]
pub enum ManifoldComponent {
    Euclidean { dim: usize },
    Hyperbolic { dim: usize, curvature: f32 },   // curvature < 0
    Spherical { dim: usize, curvature: f32 },     // curvature > 0
}

impl ProductManifoldAttention {
    /// Compute product manifold attention with geodesic scoring
    pub fn forward(
        &self,
        queries: &[Vec<f32>],    // [N, D_total]
        keys: &[Vec<f32>],       // [M, D_total]
        values: &[Vec<f32>],     // [M, D_total]
        graph_adj: &CsrMatrix,   // Sparse adjacency (attention mask)
    ) -> Vec<Vec<f32>> {
        let n = queries.len();
        let mut outputs = Vec::with_capacity(n);

        for i in 0..n {
            let q = &queries[i];
            let neighbors = graph_adj.neighbors(i);

            // Split query into component spaces
            let (q_e, q_h, q_s) = self.split_components(q);

            // Compute fused attention scores in single pass
            let mut logits = Vec::with_capacity(neighbors.len());
            for &j in &neighbors {
                let k = &keys[j];
                let (k_e, k_h, k_s) = self.split_components(k);

                // Euclidean: dot product
                let score_e = dot_product(q_e, k_e);

                // Hyperbolic: tangent-space dot product (fast path)
                let q_h_tan = self.tangent_mapper.log_map(q_h);
                let k_h_tan = self.tangent_mapper.log_map(k_h);
                let score_h = dot_product(&q_h_tan, &k_h_tan);

                // Spherical: cosine similarity on unit sphere
                let score_s = cosine_similarity(q_s, k_s);

                // Fused logit with learned mixing weights
                let logit = self.fused_kernel.weight_e * score_e
                    + self.fused_kernel.weight_h * score_h
                    + self.fused_kernel.weight_s * score_s;

                logits.push(logit);
            }

            // Softmax over neighbor logits
            let weights = softmax_with_temperature(&logits, self.fused_kernel.temperature);

            // Per-component aggregation
            let mut out_e = vec![0.0; self.euclidean_dim()];
            let mut out_h_weighted = Vec::new(); // for Einstein midpoint
            let mut out_s = vec![0.0; self.spherical_dim()];

            for (idx, &j) in neighbors.iter().enumerate() {
                let v = &values[j];
                let (v_e, v_h, v_s) = self.split_components(v);
                let w = weights[idx];

                // Euclidean: simple weighted sum
                for (d, &val) in v_e.iter().enumerate() {
                    out_e[d] += w * val;
                }

                // Hyperbolic: collect for Einstein midpoint
                out_h_weighted.push((w, v_h.to_vec()));

                // Spherical: weighted sum then project
                for (d, &val) in v_s.iter().enumerate() {
                    out_s[d] += w * val;
                }
            }

            // Hyperbolic aggregation via Einstein midpoint (closed-form)
            let hyp_curvature = self.hyperbolic_curvature();
            let hyp_points: Vec<&[f32]> = out_h_weighted.iter()
                .map(|(_, v)| v.as_slice()).collect();
            let hyp_weights: Vec<f32> = out_h_weighted.iter()
                .map(|(w, _)| *w).collect();
            let out_h = einstein_midpoint(&hyp_points, &hyp_weights, hyp_curvature);

            // Spherical: project weighted sum back to unit sphere
            let out_s = l2_normalize(&out_s);

            // Concatenate component outputs
            let output = concat_components(&out_e, &out_h, &out_s);
            outputs.push(output);
        }

        outputs
    }

    /// Riemannian gradient step: compute gradients in tangent space,
    /// then retract back to manifold via exponential map
    pub fn riemannian_step(&mut self, loss: f32, learning_rate: f32) {
        for component in &mut self.components {
            match component {
                ManifoldComponent::Euclidean { .. } => {
                    // Standard Euclidean Adam step
                }
                ManifoldComponent::Hyperbolic { curvature, .. } => {
                    // 1. Project Euclidean gradient to tangent space
                    // 2. Riemannian Adam update in tangent space
                    // 3. Exponential map back to Poincare ball / hyperboloid
                    let c = curvature.abs();
                    // grad_riemannian = (1/(lambda_x^2)) * grad_euclidean
                    // theta_new = exp_map(theta_old, -lr * grad_riemannian)
                }
                ManifoldComponent::Spherical { .. } => {
                    // 1. Project gradient to tangent plane of sphere
                    // 2. Update in tangent space
                    // 3. Normalize back to unit sphere
                }
            }
        }

        // Optionally update curvatures via gradient descent
        // d(loss)/d(kappa) flows through geodesic distance
    }
}
```

### Curvature-Adaptive Graph Transformer Block

```
                    Input: x in M = S^n x H^m x R^k
                               |
                    +----------+-----------+
                    |                      |
            Product Manifold          Curvature
            Self-Attention            Estimator
            (geodesic QKV)         (kappa = f(x))
                    |                      |
                    +----------+-----------+
                               |
                    Parallel Transport Aggregation
                    (Levi-Civita connection)
                               |
                    Tangent Space Feed-Forward
                    (operate in T_x M, map back via exp)
                               |
                    Riemannian LayerNorm
                    (normalize on manifold)
                               |
                    Output: x' in M
```

---

## Mathematical Formulations

### Geodesic Attention

For two points $x, y$ on a Riemannian manifold $(\mathcal{M}, g)$:

$$\text{GeodesicAttention}(Q, K, V) = \text{Agg}_{\mathcal{M}}\left(\text{softmax}\left(-\frac{d_g(Q, K)}{\tau}\right) \cdot V\right)$$

where $d_g$ is the geodesic distance induced by metric $g$, and $\text{Agg}_{\mathcal{M}}$ is the manifold-appropriate aggregation.

### Exponential Map Aggregation

Given weighted values $\{(w_i, v_i)\}$ in the tangent space $T_x\mathcal{M}$:

$$\text{Agg}(x, \{w_i, v_i\}) = \text{Exp}_x\left(\sum_i w_i \cdot \text{Log}_x(v_i)\right)$$

This is equivalent to one step of Riemannian gradient descent toward the weighted Frechet mean.

### Product Manifold Distance

For $x = (x^{(1)}, \ldots, x^{(p)})$ and $y = (y^{(1)}, \ldots, y^{(p)})$ in $\mathcal{M} = \prod_i \mathcal{M}_i^{\kappa_i}$:

$$d_{\mathcal{M}}(x, y)^2 = \sum_{i=1}^p \beta_i \cdot d_{\mathcal{M}_i}^{\kappa_i}(x^{(i)}, y^{(i)})^2$$

where each $d_{\mathcal{M}_i}^{\kappa_i}$ is the sectional-curvature-$\kappa_i$ geodesic distance.

### Curvature Gradient

For learned curvature $c$ in the Poincare model, the gradient of the distance with respect to curvature is:

$$\frac{\partial d_c(x,y)}{\partial c} = \frac{1}{2\sqrt{c(\alpha^2 - 1)}} \left(\frac{\partial \alpha}{\partial c} - \frac{\alpha \cdot \text{arcosh}(\alpha)}{c}\right)$$

where $\alpha = 1 + 2c\|x-y\|^2 / ((1-c\|x\|^2)(1-c\|y\|^2))$.

---

## Implementation Roadmap for RuVector

### Phase 1: Extend Fused Curvature Attention (3-4 months)

- Add learned per-component curvature to `FusedCurvatureConfig`
- Implement curvature gradient computation in `ruvector-attention/src/curvature/`
- Extend `TangentSpaceMapper` to handle variable curvatures per batch element
- Add spherical aggregation (normalize after weighted sum) alongside Einstein midpoint
- Benchmark against fixed-curvature baseline

### Phase 2: Parallel Transport and Riemannian Optimization (4-6 months)

- Implement parallel transport for Poincare ball and Lorentz model
- Build `RiemannianAdam` optimizer extending `ruvector-attention/src/training/optimizer.rs`
- Add Levi-Civita connection-based message passing to `ruvector-graph`
- Integrate with `ruvector-solver` for sublinear geodesic computation on large graphs

### Phase 3: Lie Group Equivariance (6-9 months)

- Add SE(3)-equivariant attention for molecular graphs
- Implement fiber bundle framework connecting to `ruvector-attention/src/sheaf/`
- Extend `ruvector-graph` property graph to carry manifold-valued node features
- Develop equivariant sparse attention using `ruvector-dag/src/mincut/` for graph sparsification

### Phase 4: Continuous Curvature Fields (12-18 months)

- Implement neural curvature field $\kappa(x)$ using small MLP
- Develop numerical geodesic solver for non-constant curvature (connect to PDE attention module)
- Build differentiable metric tensor learning
- Integrate with `ruvector-temporal-tensor` for time-varying curvature fields

---

## Success Metrics

| Metric | Baseline (Euclidean) | Target (Product Manifold) |
|--------|---------------------|--------------------------|
| Knowledge graph link prediction (MRR) | 0.45 | 0.55-0.60 |
| Hierarchy reconstruction accuracy | 65% | 85-95% |
| Embedding dimension for same quality | 256 | 128 |
| Attention computation (fused kernel) | 1.0x | 1.2x (overhead acceptable) |
| Training convergence (epochs) | 100 | 60-70 |
| Molecular property prediction (MAE) | 1.0x | 0.80-0.85x |

---

## References

1. Bachmann, Becigneul, Ganea (2020). "Constant Curvature Graph Convolutional Networks." ICML.
2. Chami, Ying, Re, Leskovec (2019). "Hyperbolic Graph Convolutional Neural Networks." NeurIPS.
3. Gu, Sala, Gunel, Re (2019). "Learning Mixed-Curvature Representations in Product Spaces." ICLR.
4. Nickel, Kiela (2017). "Poincare Embeddings for Learning Hierarchical Representations." NeurIPS.
5. Sala, De Sa, Gu, Re (2018). "Representation Tradeoffs for Hyperbolic Embeddings." ICML.
6. Ungar (2008). "Analytic Hyperbolic Geometry and Albert Einstein's Special Theory of Relativity."
7. Ganea, Becigneul, Hofmann (2018). "Hyperbolic Neural Networks." NeurIPS.
8. Fuchs, Worrall, Fischer, Welling (2020). "SE(3)-Transformers: 3D Roto-Translation Equivariant Attention Networks." NeurIPS.
9. Brandstetter, Hesselink, van der Pol, Bekkers, Welling (2022). "Geometric and Physical Quantities Improve E(3) Equivariant Message Passing." ICLR.
10. Skopek, Ganea, Becigneul (2020). "Mixed-curvature Variational Autoencoders." ICLR.
11. Lou, Nickel, Zantedeschi (2020). "Differentiating through the Frechet Mean." ICML.
12. Xiong, Zhu, Hsieh, Ma, Liu (2022). "Pseudo-Riemannian Graph Convolutional Networks." NeurIPS.

---

**Document Status:** Research Proposal
**Last Updated:** 2026-02-25
**Owner:** RuVector Architecture Team
**Related ADRs:** ADR-045 (Lean Agentic Integration)
**Related Crates:** ruvector-attention, ruvector-graph, ruvector-solver, ruvector-dag
