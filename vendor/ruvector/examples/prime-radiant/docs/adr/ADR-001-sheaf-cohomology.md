# ADR-001: Sheaf Cohomology for AI Coherence

**Status**: Accepted
**Date**: 2024-12-15
**Authors**: RuVector Team
**Supersedes**: None

---

## Context

Large Language Models and AI agents frequently produce outputs that are locally plausible but globally inconsistent. Traditional approaches to detecting such "hallucinations" rely on:

1. **Confidence scores**: Unreliable due to overconfidence on out-of-distribution inputs
2. **Retrieval augmentation**: Helps but doesn't verify consistency across retrieved facts
3. **Chain-of-thought verification**: Manual and prone to same failures as original reasoning
4. **Ensemble methods**: Expensive and still vulnerable to correlated errors

We need a mathematical framework that can:

- Detect **local-to-global consistency** failures systematically
- Provide **quantitative measures** of coherence
- Support **incremental updates** as new information arrives
- Work across **multiple domains** with the same underlying math

### Why Sheaf Theory?

Sheaf theory was developed in algebraic geometry and topology precisely to handle local-to-global problems. A sheaf assigns data to open sets in a way that:

1. **Locality**: Information at a point is determined by nearby information
2. **Gluing**: Locally consistent data can be assembled into global data
3. **Restriction**: Global data determines local data uniquely

These properties exactly match our coherence requirements:

- AI claims are local (about specific facts)
- Coherent knowledge should glue together globally
- Contradictions appear when local data fails to extend globally

---

## Decision

We implement **cellular sheaf cohomology** on graphs as the mathematical foundation for Prime-Radiant's coherence engine.

### Mathematical Foundation

#### Definition: Sheaf on a Graph

A **cellular sheaf** F on a graph G = (V, E) assigns:

1. To each vertex v, a vector space F(v) (the **stalk** at v)
2. To each edge e = (u,v), a vector space F(e)
3. For each vertex v incident to edge e, a linear map (the **restriction map**):
   ```
   rho_{v,e}: F(v) -> F(e)
   ```

#### Definition: Residual

For an edge e = (u,v) with vertex states x_u in F(u) and x_v in F(v), the **residual** is:

```
r_e = rho_{u,e}(x_u) - rho_{v,e}(x_v)
```

The residual measures local inconsistency: if states agree through their restriction maps, r_e = 0.

#### Definition: Sheaf Laplacian

The **sheaf Laplacian** L is the block matrix:

```
L = D^T W D
```

where:
- D is the coboundary map (encodes graph topology and restriction maps)
- W is a diagonal weight matrix for edges

The quadratic form x^T L x = sum_e w_e ||r_e||^2 computes total coherence energy.

#### Definition: Cohomology Groups

The **first cohomology group** H^1(G, F) measures obstruction to finding a global section:

```
H^1(G, F) = ker(delta_1) / im(delta_0)
```

where delta_i are coboundary maps. If H^1 is non-trivial, the sheaf admits no global section (global inconsistency exists).

### Implementation Architecture

```rust
/// A sheaf on a graph with fixed-dimensional stalks
pub struct SheafGraph {
    /// Node stalks: state vectors at each vertex
    nodes: HashMap<NodeId, StateVector>,

    /// Edge stalks and restriction maps
    edges: HashMap<EdgeId, SheafEdge>,

    /// Cached Laplacian blocks for incremental updates
    laplacian_cache: LaplacianCache,
}

/// A restriction map implemented as a matrix
pub struct RestrictionMap {
    /// The linear map as a matrix (output_dim x input_dim)
    matrix: Array2<f32>,

    /// Input dimension (node stalk dimension)
    input_dim: usize,

    /// Output dimension (edge stalk dimension)
    output_dim: usize,
}

impl RestrictionMap {
    /// Apply the restriction map: rho(x)
    pub fn apply(&self, x: &[f32]) -> Vec<f32> {
        self.matrix.dot(&ArrayView1::from(x)).to_vec()
    }

    /// Identity restriction (node stalk = edge stalk)
    pub fn identity(dim: usize) -> Self {
        Self {
            matrix: Array2::eye(dim),
            input_dim: dim,
            output_dim: dim,
        }
    }

    /// Projection restriction (edge stalk is subset of node stalk)
    pub fn projection(input_dim: usize, output_dim: usize) -> Self {
        let mut matrix = Array2::zeros((output_dim, input_dim));
        for i in 0..output_dim.min(input_dim) {
            matrix[[i, i]] = 1.0;
        }
        Self { matrix, input_dim, output_dim }
    }
}
```

### Cohomology Computation

```rust
/// Compute the first cohomology dimension
pub fn cohomology_dimension(&self) -> usize {
    // Build coboundary matrix D
    let d = self.build_coboundary_matrix();

    // Compute rank using SVD
    let svd = d.svd(true, true).unwrap();
    let rank = svd.singular_values
        .iter()
        .filter(|&s| *s > 1e-10)
        .count();

    // dim H^1 = dim(edge stalks) - rank(D)
    let edge_dim: usize = self.edges.values()
        .map(|e| e.stalk_dim)
        .sum();

    edge_dim.saturating_sub(rank)
}

/// Check if sheaf admits a global section
pub fn has_global_section(&self) -> bool {
    self.cohomology_dimension() == 0
}
```

### Energy Computation

The total coherence energy is:

```rust
/// Compute total coherence energy: E = sum_e w_e ||r_e||^2
pub fn coherence_energy(&self) -> f32 {
    self.edges.values()
        .map(|edge| {
            let source = &self.nodes[&edge.source];
            let target = &self.nodes[&edge.target];

            // Apply restriction maps
            let rho_s = edge.source_restriction.apply(&source.state);
            let rho_t = edge.target_restriction.apply(&target.state);

            // Compute residual
            let residual: Vec<f32> = rho_s.iter()
                .zip(rho_t.iter())
                .map(|(a, b)| a - b)
                .collect();

            // Weighted squared norm
            let norm_sq: f32 = residual.iter().map(|r| r * r).sum();
            edge.weight * norm_sq
        })
        .sum()
}
```

### Incremental Updates

For efficiency, we maintain a **residual cache** and update incrementally:

```rust
/// Update a single node and recompute affected energies
pub fn update_node(&mut self, node_id: NodeId, new_state: Vec<f32>) {
    // Store old state for delta computation
    let old_state = self.nodes.insert(node_id, new_state.clone());

    // Only recompute residuals for edges incident to this node
    for edge_id in self.edges_incident_to(node_id) {
        self.recompute_residual(edge_id);
    }

    // Update fingerprint
    self.update_fingerprint(node_id, &old_state, &new_state);
}
```

---

## Consequences

### Positive

1. **Mathematically Grounded**: Sheaf cohomology provides rigorous foundations for coherence
2. **Domain Agnostic**: Same math applies to facts, financial signals, medical data, etc.
3. **Local-to-Global Detection**: Naturally captures the essence of hallucination (local OK, global wrong)
4. **Incremental Computation**: Residual caching enables real-time updates
5. **Spectral Analysis**: Sheaf Laplacian eigenvalues provide drift detection
6. **Quantitative Measure**: Energy gives a continuous coherence score, not just binary

### Negative

1. **Computational Cost**: Full cohomology computation is O(n^3) for n nodes
2. **Restriction Map Design**: Choosing appropriate rho requires domain knowledge
3. **Curse of Dimensionality**: High-dimensional stalks increase memory and compute
4. **Learning Complexity**: Non-trivial to learn restriction maps from data

### Mitigations

1. **Incremental Updates**: Avoid full recomputation for small changes
2. **Learned rho**: GNN-based restriction map learning (see `learned-rho` feature)
3. **Dimensional Reduction**: Use projection restriction maps to reduce edge stalk dimension
4. **Subpolynomial MinCut**: Use for approximation when full computation is infeasible

---

## Mathematical Properties

### Theorem: Energy Minimization

If the sheaf Laplacian L has full column rank, the minimum energy configuration is unique:

```
x* = argmin_x ||Dx||^2_W = L^+ b
```

where L^+ is the pseudoinverse and b encodes boundary conditions.

### Theorem: Cheeger Inequality

The spectral gap (second smallest eigenvalue) of L relates to graph cuts:

```
lambda_2 / 2 <= h(G) <= sqrt(2 * lambda_2)
```

where h(G) is the Cheeger constant. This enables **cut prediction** from spectral analysis.

### Theorem: Hodge Decomposition

The space of edge states decomposes:

```
C^1(G, F) = im(delta_0) + ker(delta_1) + H^1(G, F)
```

This separates gradient flows (consistent), harmonic forms (neutral), and cohomology (obstructions).

---

## Related Decisions

- [ADR-004: Spectral Invariants](ADR-004-spectral-invariants.md) - Uses sheaf Laplacian eigenvalues
- [ADR-002: Category Theory](ADR-002-category-topos.md) - Sheaves are presheaves satisfying gluing
- [ADR-003: Homotopy Type Theory](ADR-003-homotopy-type-theory.md) - Higher sheaves and stacks

---

## References

1. Hansen, J., & Ghrist, R. (2019). "Toward a spectral theory of cellular sheaves." Journal of Applied and Computational Topology.

2. Curry, J. (2014). "Sheaves, Cosheaves and Applications." PhD thesis, University of Pennsylvania.

3. Robinson, M. (2014). "Topological Signal Processing." Springer.

4. Bodnar, C., et al. (2022). "Neural Sheaf Diffusion: A Topological Perspective on Heterophily and Oversmoothing in GNNs." NeurIPS.

5. Ghrist, R. (2014). "Elementary Applied Topology." Createspace.

---

## Appendix: Worked Example

Consider a knowledge graph with three facts:

- F1: "Paris is the capital of France" (state: [1, 0, 0, 1])
- F2: "France is in Europe" (state: [0, 1, 1, 0])
- F3: "Paris is not in Europe" (state: [1, 0, 0, -1]) -- HALLUCINATION

Edges with identity restriction maps:
- E1: F1 -> F2 (France connection)
- E2: F1 -> F3 (Paris connection)
- E3: F2 -> F3 (Europe connection)

Residuals:
- r_{E1} = [1,0,0,1] - [0,1,1,0] = [1,-1,-1,1], ||r||^2 = 4
- r_{E2} = [1,0,0,1] - [1,0,0,-1] = [0,0,0,2], ||r||^2 = 4
- r_{E3} = [0,1,1,0] - [1,0,0,-1] = [-1,1,1,1], ||r||^2 = 4

Total energy = 4 + 4 + 4 = 12 (HIGH -- indicates hallucination)

If F3 were corrected to "Paris is in Europe" (state: [1,0,1,1]):
- r_{E3} = [0,1,1,0] - [1,0,1,1] = [-1,1,0,-1], ||r||^2 = 3

Energy decreases, indicating better coherence.
