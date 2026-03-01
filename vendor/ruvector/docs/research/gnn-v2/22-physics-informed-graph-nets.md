# Axis 2: Physics-Informed Graph Neural Networks

**Document:** 22 of 30
**Series:** Graph Transformers: 2026-2036 and Beyond
**Last Updated:** 2026-02-25
**Status:** Research Prospectus

---

## 1. Problem Statement

Standard graph transformers learn arbitrary functions over graphs without respecting the physical laws that govern many real-world graph systems. Molecular dynamics, fluid networks, electrical circuits, crystal structures, and spacetime discretizations all carry symmetries and conservation laws that, if baked into the architecture, yield better generalization, data efficiency, and physical plausibility.

The physics-informed axis asks: how do we build graph transformers that are *incapable* of violating physical laws?

### 1.1 The Five Pillars of Physics-Informed Design

1. **Conservation laws**: Energy, momentum, charge, and other quantities must be conserved by message passing
2. **Symmetry equivariance**: Rotations, translations, reflections, gauge transformations must commute with attention
3. **Variational structure**: The network's dynamics should derive from an action principle (Lagrangian or Hamiltonian)
4. **Symplecticity**: Time evolution must preserve phase space volume (Liouville's theorem)
5. **Locality**: Physical interactions are local (or decay with distance); the architecture should respect this

### 1.2 RuVector Baseline

- **`ruvector-attention`**: PDE attention (`pde_attention/`), curvature attention (`curvature/`), transport attention (`transport/`), topology attention (`topology/`)
- **`ruvector-mincut-gated-transformer`**: Energy gates (`energy_gate.rs`), spectral methods (`spectral.rs`)
- **`ruvector-attention`**: Info-geometry (`info_geometry/`), sheaf attention (`sheaf/`)
- **`ruvector-math`**: Mathematical utility functions

---

## 2. Hamiltonian Graph Neural Networks

### 2.1 Formulation

A Hamiltonian GNN treats each node v as a particle with position q_v and momentum p_v in a phase space P = R^{2d}. The graph defines interactions. The system evolves according to Hamilton's equations:

```
dq_v/dt = dH/dp_v
dp_v/dt = -dH/dq_v
```

where the Hamiltonian H is a learned function of the entire graph state:

```
H(q, p, G) = sum_v T(p_v) + sum_v U_self(q_v) + sum_{(u,v) in E} V_pair(q_u, q_v)
```

- T(p) = kinetic energy (typically ||p||^2 / 2m)
- U_self(q) = self-potential (learned per-node)
- V_pair(q_u, q_v) = pairwise interaction potential (learned, respects edge structure)

**Key property:** Energy H is exactly conserved by construction. No learned parameter can cause energy drift.

### 2.2 Hamiltonian Attention

We propose Hamiltonian Attention, where attention weights derive from energy gradients:

```
alpha_{uv} = softmax_v(-(dV_pair/dq_u)(q_u, q_v) . (dV_pair/dq_v)(q_u, q_v) / sqrt(d))
```

**Interpretation:** Nodes attend most strongly to neighbors with which they have the steepest energy gradient -- i.e., the strongest physical interaction.

**Advantage over standard attention:** The attention pattern automatically respects physical structure. Nodes in equilibrium (flat energy landscape) have diffuse attention. Nodes near phase transitions (steep gradients) have sharp, focused attention.

### 2.3 Symplectic Integration

Standard Euler or RK4 integrators do not preserve the symplectic structure. Over long trajectories, this causes energy drift. We use symplectic integrators:

**Stormer-Verlet (leapfrog):**

```
p_{t+1/2} = p_t - (dt/2) * dH/dq(q_t)
q_{t+1} = q_t + dt * dH/dp(p_{t+1/2})
p_{t+1} = p_{t+1/2} - (dt/2) * dH/dq(q_{t+1})
```

**Graph Symplectic Integrator:**

```rust
pub trait SymplecticGraphIntegrator {
    /// One step of symplectic integration on a graph
    fn step(
        &self,
        graph: &PropertyGraph,
        positions: &mut Tensor,    // q: n x d
        momenta: &mut Tensor,      // p: n x d
        hamiltonian: &dyn GraphHamiltonian,
        dt: f64,
    ) -> Result<StepResult, PhysicsError>;

    /// Energy at current state (should be conserved)
    fn energy(
        &self,
        graph: &PropertyGraph,
        positions: &Tensor,
        momenta: &Tensor,
        hamiltonian: &dyn GraphHamiltonian,
    ) -> f64;
}

pub trait GraphHamiltonian {
    /// Kinetic energy T(p)
    fn kinetic_energy(&self, momenta: &Tensor) -> f64;

    /// Self-potential U(q_v) for node v
    fn self_potential(&self, node: NodeId, position: &[f32]) -> f64;

    /// Pairwise potential V(q_u, q_v) for edge (u,v)
    fn pair_potential(
        &self,
        src: NodeId,
        dst: NodeId,
        pos_src: &[f32],
        pos_dst: &[f32],
    ) -> f64;

    /// Gradient of H w.r.t. positions (force)
    fn force(&self, graph: &PropertyGraph, positions: &Tensor) -> Tensor;

    /// Gradient of H w.r.t. momenta (velocity)
    fn velocity(&self, momenta: &Tensor) -> Tensor;
}
```

### 2.4 Complexity Analysis

| Operation | Complexity | Notes |
|-----------|-----------|-------|
| Hamiltonian evaluation | O(n*d + |E|*d) | Per-node + per-edge potentials |
| Force computation | O(n*d + |E|*d) | Autodiff through Hamiltonian |
| Symplectic step | O(n*d + |E|*d) | Two half-steps + one full step |
| Hamiltonian attention | O(|E|*d) | Sparse: only along edges |
| Full trajectory (T steps) | O(T * (n + |E|) * d) | Linear in time and graph size |

---

## 3. Lagrangian Message Passing

### 3.1 From Hamiltonian to Lagrangian

The Lagrangian formulation uses generalized coordinates q and velocities q_dot instead of positions and momenta. The Lagrangian L = T - V, and equations of motion follow from the Euler-Lagrange equations:

```
d/dt (dL/dq_dot_v) = dL/dq_v + sum_{u: (u,v) in E} F_{constraint}(u, v)
```

**Advantage over Hamiltonian:** The Lagrangian formulation naturally handles constraints (e.g., rigid bonds, conservation laws) through Lagrange multipliers.

### 3.2 Lagrangian Message Passing Protocol

```
1. COMPUTE LAGRANGIAN:
   L = sum_v T(q_dot_v) - sum_v U(q_v) - sum_{(u,v)} V(q_u, q_v)

2. COMPUTE MESSAGES (from Euler-Lagrange):
   m_{v->u} = dV/dq_u(q_u, q_v)    // "force message"

3. AGGREGATE:
   F_v = sum_{u: (v,u) in E} m_{u->v}  // Total force on v

4. UPDATE:
   a_v = (F_v + dU/dq_v) / m_v         // Acceleration
   q_dot_v += a_v * dt                   // Update velocity
   q_v += q_dot_v * dt                   // Update position
```

### 3.3 Constrained Lagrangian GNN

For systems with constraints (e.g., molecular bonds of fixed length), we add constraint forces via Lagrange multipliers:

```
Input: Graph G, coordinates q, velocities q_dot, constraints C
Output: Constrained update

1. Unconstrained step:
   q_hat = q + q_dot * dt + a * dt^2 / 2

2. Constraint projection (SHAKE algorithm adapted to graphs):
   for each constraint c_k(q) = 0:
     lambda_k = (c_k(q_hat)) / (dc_k/dq . dc_k/dq * dt^2)
     q_hat -= lambda_k * dc_k/dq * dt^2

3. Corrected velocity:
   q_dot_new = (q_hat - q) / dt
```

---

## 4. Gauge-Equivariant Graph Transformers

### 4.1 What is Gauge Symmetry?

A gauge symmetry is a local symmetry transformation that varies from node to node. In physics, electromagnetic fields have U(1) gauge symmetry. In graph ML, a gauge transformation is a node-wise rotation of the feature space.

**Definition.** A graph transformer is gauge-equivariant if for any collection of node-wise transformations {g_v in G}_v:

```
f(g_v . X_v, A) = g_v . f(X_v, A)
```

where G is a symmetry group and . is the group action.

### 4.2 Gauge-Equivariant Attention

Standard attention: `alpha_{uv} = softmax(Q_u . K_v^T / sqrt(d))`

This is NOT gauge-equivariant because Q_u and K_v live in different tangent spaces (at nodes u and v). Rotating Q_u without rotating K_v changes the attention weight.

**Gauge-equivariant attention:**

```
alpha_{uv} = softmax(Q_u . Gamma_{u->v} . K_v^T / sqrt(d))
```

where Gamma_{u->v} is a learned parallel transport operator that maps from the tangent space at v to the tangent space at u. This is a *connection* in the language of differential geometry.

**The connection Gamma must satisfy:**
1. Gamma_{u->v} in G (group-valued)
2. Gamma_{u->v} = Gamma_{v->u}^{-1} (inverse consistency)
3. For paths u -> v -> w: Gamma_{u->w} approx= Gamma_{u->v} . Gamma_{v->w} (parallel transport)

### 4.3 Curvature from Holonomy

The deviation from exact parallel transport around a loop (holonomy) defines curvature:

```
F_{uvw} = Gamma_{u->v} . Gamma_{v->w} . Gamma_{w->u} - I
```

This is the discrete analog of the field strength tensor in physics. Non-zero F means the graph has "gauge curvature" -- the feature space is non-trivially curved.

**Curvature-aware attention:** Weight attention by curvature magnitude:

```
alpha_{uv} = softmax(Q_u . Gamma_{u->v} . K_v^T / sqrt(d) + beta * ||F_{uvw}||)
```

Nodes in high-curvature regions get extra attention, similar to how gravitational lensing focuses light near massive objects.

**RuVector integration:**

```rust
/// Gauge-equivariant attention mechanism
pub trait GaugeEquivariantAttention {
    type Group: LieGroup;

    /// Compute parallel transport along edge
    fn parallel_transport(
        &self,
        src: NodeId,
        dst: NodeId,
        features_src: &[f32],
        features_dst: &[f32],
    ) -> <Self::Group as LieGroup>::Element;

    /// Compute gauge-equivariant attention weights
    fn attention(
        &self,
        query: NodeId,
        keys: &[NodeId],
        graph: &PropertyGraph,
    ) -> Vec<f32>;

    /// Compute holonomy (curvature) around a cycle
    fn holonomy(
        &self,
        cycle: &[NodeId],
    ) -> <Self::Group as LieGroup>::Element;

    /// Compute field strength tensor for a triangle
    fn field_strength(
        &self,
        u: NodeId,
        v: NodeId,
        w: NodeId,
    ) -> Tensor;
}

pub trait LieGroup: Sized {
    type Element;
    type Algebra;

    fn identity() -> Self::Element;
    fn inverse(g: &Self::Element) -> Self::Element;
    fn compose(g: &Self::Element, h: &Self::Element) -> Self::Element;
    fn exp(xi: &Self::Algebra) -> Self::Element;
    fn log(g: &Self::Element) -> Self::Algebra;
}
```

---

## 5. Noether Attention: Discovering Conservation Laws

### 5.1 Noether's Theorem on Graphs

Noether's theorem: every continuous symmetry of the action implies a conserved quantity.

**Graph version:** If the graph transformer's learned Hamiltonian H is invariant under a continuous transformation phi_epsilon:

```
H(phi_epsilon(q), phi_epsilon(p)) = H(q, p) for all epsilon
```

then the quantity:

```
Q = sum_v dp_v/d(epsilon) . q_v
```

is conserved during the transformer's dynamics.

### 5.2 Noether Attention Layer

We propose a Noether Attention layer that:
1. Learns symmetries of the Hamiltonian via equivariance testing
2. Derives conserved quantities from discovered symmetries
3. Uses conserved quantities as attention bias terms

```
Algorithm: Noether Attention

1. DISCOVER SYMMETRIES:
   For candidate symmetry generators {xi_k}:
     Test: ||H(exp(epsilon * xi_k) . state) - H(state)|| < threshold
     If passes: xi_k is an approximate symmetry

2. COMPUTE CONSERVED QUANTITIES:
   For each symmetry xi_k:
     Q_k = sum_v (dL/dq_dot_v) . (xi_k . q_v)

3. ATTENTION WITH CONSERVATION BIAS:
   alpha_{uv} = softmax(
     standard_attention(u, v) +
     gamma * sum_k |dQ_k/dq_u . dQ_k/dq_v| / ||dQ_k||^2
   )
```

**Interpretation:** Nodes that contribute to the same conserved quantity attend to each other more strongly. This automatically discovers physically meaningful communities (e.g., parts of a molecule that share the same vibrational mode).

---

## 6. Symplectic Graph Transformers

### 6.1 Symplectic Attention Layers

A symplectic map preserves the symplectic form omega = sum_i dq_i ^ dp_i. We construct attention layers that are symplectic by design.

**Symplectic attention block:**

```
q_{l+1} = q_l + dt * dH_1/dp(p_l)
p_{l+1} = p_l - dt * dH_2/dq(q_{l+1})
```

where H_1 and H_2 are learned attention-based Hamiltonians:

```
H_1(q, p) = sum_v ||p_v||^2 / 2 + sum_{(u,v)} alpha_1(q_u, q_v) * V_1(p_u, p_v)
H_2(q, p) = sum_v U(q_v) + sum_{(u,v)} alpha_2(q_u, q_v) * V_2(q_u, q_v)
```

**Key property:** Each layer is exactly symplectic (not approximately). This means:
- Volume in phase space is exactly preserved
- Long-time energy conservation is guaranteed
- KAM theory applies: quasi-periodic orbits are stable

### 6.2 Symplectic Graph Transformer Architecture

```
Input: Graph G, initial (q_0, p_0)

Layer 1: Symplectic Attention Block (H_1, H_2)
  |
Layer 2: Symplectic Attention Block (H_3, H_4)
  |
  ...
  |
Layer L: Symplectic Attention Block (H_{2L-1}, H_{2L})
  |
Output: (q_L, p_L) -- guaranteed symplectic map from input
```

**Complexity:** Same as standard graph transformer per layer: O((n + |E|) * d). The symplectic structure adds no overhead -- it constrains the architecture, not the computation.

---

## 7. Projections

### 7.1 By 2030

**Likely:**
- Hamiltonian GNNs standard for molecular dynamics simulation
- Gauge-equivariant attention for crystal property prediction
- Symplectic graph transformers for long-horizon trajectory prediction
- Conservation-law enforcement reduces training data by 10x for physics problems

**Possible:**
- Lagrangian message passing for constrained multi-body systems
- Noether attention automatically discovering unknown conservation laws
- Physics-informed graph transformers for climate modeling

**Speculative:**
- General covariance (diffeomorphism invariance) in graph attention
- Graph transformers that discover new physics from data

### 7.2 By 2033

**Likely:**
- Physics-informed graph transformers as standard tool in computational physics
- Gauge-equivariant architectures for particle physics (lattice QCD on graphs)

**Possible:**
- Graph transformers that respect general relativity (curved spacetime graphs)
- Topological field theory on graphs (topological invariant computation)

### 7.3 By 2036+

**Possible:**
- Graph transformers that simulate quantum field theory
- Emergent spacetime from graph attention dynamics (graph transformers discovering gravity)

**Speculative:**
- Graph transformers as a computational substrate for fundamental physics simulation
- New physical theories discovered by physics-informed graph attention

---

## 8. RuVector Integration Roadmap

### Phase 1: Hamiltonian Foundation (2026-2027)
- New module: `ruvector-attention/src/physics/hamiltonian.rs`
- Extend energy gates in `ruvector-mincut-gated-transformer` to enforce conservation
- Implement Stormer-Verlet integrator for graph dynamics
- Benchmark on molecular dynamics datasets (MD17, QM9)

### Phase 2: Gauge & Symmetry (2027-2028)
- Extend `ruvector-attention/src/curvature/` with parallel transport operators
- Implement gauge-equivariant attention using sheaf attention infrastructure
- Add Noether attention layer
- Integration with `ruvector-verified` for conservation law certificates

### Phase 3: Full Physics Stack (2028-2030)
- Symplectic graph transformer architecture
- Lagrangian message passing with constraint handling
- General covariance for Riemannian manifold graphs
- Production deployment for computational physics applications

---

## References

1. Greydanus et al., "Hamiltonian Neural Networks," NeurIPS 2019
2. Cranmer et al., "Lagrangian Neural Networks," ICML Workshop 2020
3. Brandstetter et al., "Geometric and Physical Quantities improve E(3) Equivariant Message Passing," ICLR 2022
4. Batzner et al., "E(3)-equivariant graph neural networks for data-efficient and accurate interatomic potentials," Nature Communications 2022
5. Cohen et al., "Gauge Equivariant Convolutional Networks and the Icosahedral CNN," ICML 2019
6. Chen et al., "Symplectic Recurrent Neural Networks," ICLR 2020
7. de Haan et al., "Gauge Equivariant Mesh CNNs," ICLR 2021

---

**End of Document 22**

**Next:** [Doc 23 - Biological: Spiking Graph Transformers](23-biological-spiking-graph-transformers.md)
