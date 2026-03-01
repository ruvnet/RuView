# Axis 7: Hyperbolic & Mixed-Curvature Graph Transformers

**Document:** 27 of 30
**Series:** Graph Transformers: 2026-2036 and Beyond
**Last Updated:** 2026-02-25
**Status:** Research Prospectus

---

## 1. Problem Statement

Euclidean space is the wrong geometry for most real-world graphs. Hierarchical data (taxonomies, organizational charts, phylogenetic trees) embeds naturally into hyperbolic space, where the volume of a ball grows exponentially with radius -- matching the exponential branching of trees. Cyclical data (molecular rings, social cycles) embeds into spherical space. Most real graphs contain a mixture of hierarchical, cyclical, and flat substructures.

The mixed-curvature axis asks: how do we build graph transformers that operate in the right geometry for each part of the graph?

### 1.1 Why Geometry Matters

**Distortion theorem (Bourgain, 1985).** Any metric space with n points can be embedded in Euclidean space with O(log n) distortion. For trees, hyperbolic space achieves O(1) distortion. The gap is exponential.

**Practical impact:**

| Graph Structure | Euclidean (d=128) | Hyperbolic (d=128) | Improvement |
|----------------|-------------------|-------------------|-------------|
| Tree (branching=3, depth=10) | 40% recall@10 | 95% recall@10 | 2.4x |
| Social network (power-law) | 70% | 92% | 1.3x |
| Molecular graph (cycles) | 85% | 75% | Worse |
| Mixed (wiki hyperlinks) | 75% | 80% | 1.07x |

Hyperbolic helps hierarchies but hurts cycles. We need both.

### 1.2 RuVector Baseline

- **`ruvector-hyperbolic-hnsw`**: Poincare ball model (`poincare.rs`), hyperbolic HNSW search (`hnsw.rs`), tangent space operations (`tangent.rs`), sharding (`shard.rs`)
- **`ruvector-attention`**: Hyperbolic attention (`hyperbolic/`), curvature attention (`curvature/`)
- **`ruvector-attention`**: Info-geometry (`info_geometry/`), transport attention (`transport/`)

---

## 2. Hyperbolic Graph Attention

### 2.1 The Poincare Ball Model

The Poincare ball B_c^d = {x in R^d : c * ||x||^2 < 1} with curvature -1/c. Key operations:

**Mobius addition:**
```
x (+)_c y = ((1 + 2c<x,y> + c||y||^2) * x + (1 - c||x||^2) * y)
             / (1 + 2c<x,y> + c^2 * ||x||^2 * ||y||^2)
```

**Hyperbolic distance:**
```
d_c(x, y) = (2/sqrt(c)) * arctanh(sqrt(c) * ||(-x) (+)_c y||)
```

**Exponential map (tangent -> ball):**
```
exp_x^c(v) = x (+)_c (tanh(sqrt(c) * lambda_x * ||v|| / 2) * v / (sqrt(c) * ||v||))
where lambda_x = 2 / (1 - c * ||x||^2)  (conformal factor)
```

**Logarithmic map (ball -> tangent):**
```
log_x^c(y) = (2 / (sqrt(c) * lambda_x)) * arctanh(sqrt(c) * ||(-x) (+)_c y||)
             * ((-x) (+)_c y) / ||(-x) (+)_c y||
```

### 2.2 Hyperbolic Multi-Head Attention

Standard multi-head attention operates in Euclidean space. Hyperbolic MHA works in the Poincare ball:

```
HyperbolicMHA(Q, K, V):

For each head h:
  1. Project to tangent space at origin:
     Q_h = log_0(Q) * W_Q^h
     K_h = log_0(K) * W_K^h
     V_h = log_0(V) * W_V^h

  2. Compute attention in tangent space (Euclidean):
     alpha_h = softmax(Q_h * K_h^T / sqrt(d_h))

  3. Aggregate values in tangent space:
     Z_h = alpha_h * V_h

  4. Map back to hyperbolic space:
     O_h = exp_0(Z_h)

Concatenate and project:
  O = exp_0(concat(log_0(O_1), ..., log_0(O_H)) * W_O)
```

**Advantage:** Attention weights computed from hyperbolic distances naturally give more weight to semantically close nodes in the tree hierarchy.

### 2.3 Fully Hyperbolic Attention (No Tangent Space)

The tangent space approach "flattens" the hyperbolic geometry. Fully hyperbolic attention operates entirely in the ball:

```
FullyHyperbolicAttention(q, K, V):

  For each key k_j:
    // Hyperbolic attention score
    score_j = -beta * d_c(q, k_j)^2 + <q, k_j>_L
    // where <.,.>_L is the Lorentzian inner product

  alpha = softmax(scores)

  // Hyperbolic weighted midpoint (Einstein midpoint)
  z = EinsteinMidpoint(V, alpha, c)
    = exp_0(sum_j alpha_j * gamma_j * log_0(v_j) / sum_j alpha_j * gamma_j)
    // where gamma_j = 1 / sqrt(1 - c * ||v_j||^2) is the Lorentz factor
```

**Complexity:** Same as Euclidean attention O(n^2 * d), but with ~3x constant factor due to hyperbolic arithmetic.

---

## 3. Product Manifold Transformers

### 3.1 Product Spaces

Real graphs have mixed curvature. We use product manifolds:

```
M = H_{c1}^{d1} x S_{c2}^{d2} x R^{d3}

where:
  H_c^d = Hyperbolic space (curvature -1/c)  -- for hierarchies
  S_c^d = Spherical space (curvature 1/c)    -- for cycles
  R^d   = Euclidean space (curvature 0)      -- for flat structures

Total dimension: d = d1 + d2 + d3
```

**Distance in product space:**
```
d_M(x, y) = sqrt(w_H * d_H(x_H, y_H)^2 + w_S * d_S(x_S, y_S)^2 + w_R * d_R(x_R, y_R)^2)
```
where w_H, w_S, w_R are learned weights.

### 3.2 Product Manifold Attention

```
ProductAttention(Q, K, V):

  // Split embeddings into manifold components
  Q_H, Q_S, Q_R = split(Q, [d1, d2, d3])
  K_H, K_S, K_R = split(K, [d1, d2, d3])
  V_H, V_S, V_R = split(V, [d1, d2, d3])

  // Attention scores from each manifold
  score_H = -d_H(Q_H, K_H)^2        // Hyperbolic distance
  score_S = <Q_S, K_S>_S              // Spherical inner product
  score_R = Q_R . K_R^T / sqrt(d3)   // Euclidean dot product

  // Combined attention
  alpha = softmax(w_H * score_H + w_S * score_S + w_R * score_R)

  // Aggregate per manifold
  Z_H = HyperbolicMidpoint(V_H, alpha)
  Z_S = SphericalMidpoint(V_S, alpha)
  Z_R = EuclideanWeightedSum(V_R, alpha)

  return concat(Z_H, Z_S, Z_R)
```

### 3.3 Learned Dimension Allocation

**Key question:** How many dimensions to allocate to each manifold component?

**Differentiable allocation:**
```
Input: Total dimension budget d, curvature signal from data

1. Compute curvature estimates per subgraph:
   kappa_i = estimated_sectional_curvature(subgraph_i)

2. Classify:
   if kappa_i < -threshold: allocate to H (hyperbolic)
   if kappa_i > +threshold: allocate to S (spherical)
   else: allocate to R (Euclidean)

3. Dimension allocation:
   d_H = d * fraction_hyperbolic
   d_S = d * fraction_spherical
   d_R = d * fraction_euclidean
```

**Continuous relaxation:** Use Gumbel-Softmax to make dimension allocation differentiable and trainable end-to-end.

---

## 4. Lorentzian Graph Neural Networks

### 4.1 The Hyperboloid Model

The hyperboloid (Lorentz) model represents hyperbolic space as:

```
L_c^d = {x in R^{d+1} : <x, x>_L = -1/c}

Lorentzian inner product:
  <x, y>_L = -x_0 * y_0 + x_1 * y_1 + ... + x_d * y_d
```

**Advantages over Poincare ball:**
- Numerically stable (no division by small numbers near boundary)
- Natural connection to special relativity
- Efficient parallel transport

### 4.2 Lorentzian Attention

```
LorentzianAttention(Q, K, V):

  For each query q_i, key k_j:
    // Lorentzian inner product as attention score
    score_{ij} = -<q_i, k_j>_L - 1/c

    // This is related to hyperbolic distance:
    // d_L(x,y) = (1/sqrt(c)) * arccosh(-c * <x, y>_L)

  alpha = softmax(scores / sqrt(d))

  // Lorentzian centroid (Frechet mean on hyperboloid)
  z_i = LorentzianCentroid(V, alpha[i])
```

**Lorentzian centroid computation:**
```
LorentzianCentroid(points, weights):
  1. Weighted sum in ambient space:
     s = sum_j w_j * v_j

  2. Project back to hyperboloid:
     z = s / sqrt(|<s, s>_L| * c)
     // Ensures <z, z>_L = -1/c
```

### 4.3 Causal Structure in Lorentzian Graphs

In Minkowski space, the Lorentzian metric defines a causal structure: event A can influence event B only if A is in B's past light cone.

**Causal attention:** Only allow attention from past to future:
```
alpha_{ij} = softmax(score_{ij}) * causal_mask_{ij}

causal_mask_{ij} = 1  if <x_i - x_j, x_i - x_j>_L <= 0 and x_j^0 < x_i^0
                   0  otherwise

// Interpretation: j can attend to i only if i is in j's causal past
```

This naturally enforces causality in temporal graph transformers.

### 4.4 Lorentz Boosts as Attention Transformations

In special relativity, Lorentz boosts map between reference frames. In Lorentzian GNNs, we use boosts as learned transformations:

```
Boost(x, v):
  // Boost embedding x by velocity v
  gamma = 1 / sqrt(1 - ||v||^2)
  x_0' = gamma * (x_0 - v . x_{1:d})
  x_{1:d}' = x_{1:d} + (gamma - 1) * (v . x_{1:d}) / ||v||^2 * v - gamma * v * x_0
  return (x_0', x_{1:d}')
```

**Boost-equivariant attention:** Attention weights are invariant under Lorentz boosts:
```
alpha(Boost(x, v), Boost(y, v)) = alpha(x, y)
// Same attention regardless of reference frame
```

---

## 5. Curvature-Adaptive Routing

### 5.1 The Problem

Different parts of a graph have different optimal curvatures. A single global curvature is suboptimal. We need per-node or per-subgraph curvature.

### 5.2 Sectional Curvature Estimation

For a small triangle (u, v, w) in the graph, estimate sectional curvature using the Toponogov comparison:

```
Given triangle with side lengths a = d(u,v), b = d(v,w), c = d(u,w):

Euclidean comparison angle:
  cos(alpha_0) = (a^2 + b^2 - c^2) / (2ab)

Actual angle (from embeddings):
  cos(alpha) = <h_u - h_v, h_w - h_v> / (||h_u - h_v|| * ||h_w - h_v||)

Curvature estimate:
  kappa ~ 3 * (alpha - alpha_0) / (a * b * sin(alpha_0))

  kappa < 0: locally hyperbolic (tree-like)
  kappa > 0: locally spherical (cycle-like)
  kappa = 0: locally Euclidean (flat)
```

### 5.3 Adaptive Curvature Attention

```
CurvatureAdaptiveAttention(Q, K, V, G):

  For each node v:
    // Estimate local curvature
    kappa_v = estimate_curvature(v, G)

    // Select attention mechanism based on curvature
    if kappa_v < -threshold:
      attn_v = HyperbolicAttention(Q[v], K[N(v)], V[N(v)], c=-1/kappa_v)
    elif kappa_v > threshold:
      attn_v = SphericalAttention(Q[v], K[N(v)], V[N(v)], c=1/kappa_v)
    else:
      attn_v = EuclideanAttention(Q[v], K[N(v)], V[N(v)])

  // Smooth blending at curvature transitions
  For boundary nodes (where curvature changes sign):
    attn_v = lerp(attn_neg, attn_pos, sigmoid(kappa_v / sigma))
```

**RuVector integration:**

```rust
/// Curvature-adaptive graph attention
pub trait CurvatureAdaptiveAttention {
    /// Estimate local curvature at each node
    fn estimate_curvature(
        &self,
        graph: &PropertyGraph,
        features: &Tensor,
        node: NodeId,
    ) -> f64;

    /// Compute attention with locally-adapted geometry
    fn attend(
        &self,
        graph: &PropertyGraph,
        features: &Tensor,
        curvatures: &[f64],
    ) -> Result<Tensor, CurvatureError>;

    /// Get curvature distribution statistics
    fn curvature_stats(&self) -> CurvatureDistribution;
}

pub struct CurvatureDistribution {
    pub mean: f64,
    pub std: f64,
    pub min: f64,
    pub max: f64,
    pub fraction_hyperbolic: f64,
    pub fraction_spherical: f64,
    pub fraction_euclidean: f64,
    pub per_node: Vec<f64>,
}
```

---

## 6. Riemannian Optimization on Graphs

### 6.1 Riemannian Gradient Descent

Standard gradient descent does not preserve manifold constraints. Riemannian GD operates on the manifold directly:

```
Riemannian SGD update:

1. Compute Euclidean gradient: g = dL/dtheta
2. Project to tangent space: g_R = proj_{T_theta M}(g)
3. Retract to manifold: theta' = Retract_theta(-lr * g_R)

For Poincare ball:
  proj(g) = g / (lambda_theta)^2         // Rescale by conformal factor
  Retract(v) = exp_theta(-lr * v)         // Exponential map

For Hyperboloid:
  proj(g) = g + <g, theta>_L * theta      // Lorentzian projection
  Retract(v) = cosh(||v||_L) * theta + sinh(||v||_L) * v / ||v||_L
```

### 6.2 Mixed-Curvature Optimization

For product manifold M = H x S x R:
```
1. Split gradient: g = (g_H, g_S, g_R)
2. Project each component:
   g_H' = proj_{T_H}(g_H)   // Hyperbolic projection
   g_S' = proj_{T_S}(g_S)   // Spherical projection
   g_R' = g_R                 // Euclidean (no projection needed)
3. Retract each component:
   theta_H' = exp_H(-lr_H * g_H')
   theta_S' = exp_S(-lr_S * g_S')
   theta_R' = theta_R - lr_R * g_R'
```

**Per-manifold learning rates:** Different curvatures need different learning rates. Hyperbolic components typically need smaller learning rates to avoid exploding gradients near the boundary.

---

## 7. Projections

### 7.1 By 2030

**Likely:**
- Product manifold transformers with learned dimension allocation standard for heterogeneous graphs
- Curvature-adaptive attention for knowledge graphs (hierarchical + cyclical)
- Riemannian optimization integrated into standard training frameworks

**Possible:**
- Lorentzian graph neural networks for spacetime-structured data
- Per-node curvature adaptation (not just per-subgraph)
- Curvature-based architecture search (select geometry by task)

**Speculative:**
- General Riemannian manifold attention (beyond constant-curvature spaces)
- Learned metric tensors that define custom geometry per graph

### 7.2 By 2033

**Likely:**
- Mixed-curvature graph transformers as default for graph ML
- Hardware-accelerated hyperbolic operations

**Possible:**
- Finsler manifold attention (asymmetric distances for directed graphs)
- Sub-Riemannian attention (constrained movement in embedding space)
- Connection to physics: graph attention in curved spacetime

### 7.3 By 2036+

**Possible:**
- Emergent geometry: graph transformers that discover the right manifold
- Geometric deep learning unification: all attention as parallel transport on bundles
- Quantum hyperbolic attention on quantum hardware

**Speculative:**
- Graph transformers operating in exotic manifolds (Calabi-Yau, spin manifolds)
- Attention as geodesic flow on the manifold of distributions

---

## 8. RuVector Implementation Roadmap

### Phase 1: Product Manifolds (2026-2027)
- Extend `ruvector-hyperbolic-hnsw` with spherical and product space support
- Implement product manifold attention in `ruvector-attention/src/hyperbolic/`
- Learned dimension allocation with Gumbel-Softmax
- Benchmark on mixed-curvature datasets

### Phase 2: Lorentzian & Curvature-Adaptive (2027-2028)
- Implement Lorentzian (hyperboloid) model alongside Poincare ball
- Curvature estimation module
- Curvature-adaptive attention routing
- Riemannian optimizer for mixed-curvature training
- Integration with `ruvector-attention/src/curvature/` existing infrastructure

### Phase 3: Advanced Geometry (2028-2030)
- Finsler manifold attention for directed graphs
- General Riemannian attention with learned metric tensors
- Causal Lorentzian attention for temporal graphs
- Integration with physics-informed axis (Doc 22)

---

## References

1. Chami et al., "Hyperbolic Graph Convolutional Neural Networks," NeurIPS 2019
2. Bachmann et al., "Constant Curvature Graph Convolutional Networks," ICML 2020
3. Gu et al., "Learning Mixed-Curvature Representations in Product Spaces," ICLR 2019
4. Law et al., "Lorentzian Distance Learning for Hyperbolic Representations," ICML 2019
5. Nickel & Kiela, "Poincare Embeddings for Learning Hierarchical Representations," NeurIPS 2017
6. Bonnabel, "Stochastic Gradient Descent on Riemannian Manifolds," IEEE TAC 2013
7. RuVector `ruvector-hyperbolic-hnsw` documentation (internal)

---

**End of Document 27**

**Next:** [Doc 28 - Temporal: Causal & Retrocausal Attention](28-temporal-causal-retrocausal.md)
