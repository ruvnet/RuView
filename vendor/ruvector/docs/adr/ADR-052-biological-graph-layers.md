# ADR-052: Biological Graph Transformer Layers

## Status

Accepted

## Date

2026-02-25

## Context

Biological neural networks process graph-structured information at 20 watts while consuming 86 billion neurons and 100 trillion synapses. Artificial graph transformers processing comparable graphs require megawatts. This disparity stems from three computational principles that artificial graph transformers have not adopted: event-driven sparsity (99%+ of compute is skipped when neurons are below threshold), local learning rules (synaptic updates require only pre/post-synaptic activity, no global backpropagation), and temporal coding (precise spike timing carries information beyond firing rates).

RuVector already implements the core biological primitives across several crates:

- `ruvector-mincut-gated-transformer/src/attention/spike_driven.rs`: `SpikeDrivenAttention` with multiplication-free attention via spike coincidence detection
- `ruvector-mincut-gated-transformer/src/spike.rs`: `SpikeScheduler` with rate-based tier selection and novelty gating
- `ruvector-nervous-system/src/dendrite/compartment.rs`: multi-compartment dendritic models
- `ruvector-nervous-system/src/dendrite/coincidence.rs`: dendritic coincidence detection
- `ruvector-nervous-system/src/dendrite/plateau.rs`: plateau potential generation for BTSP
- `ruvector-nervous-system/src/plasticity/btsp.rs`: Behavioral Timescale Synaptic Plasticity
- `ruvector-nervous-system/src/plasticity/eprop.rs`: e-prop eligibility trace learning
- `ruvector-nervous-system/src/plasticity/consolidate.rs`: synaptic consolidation
- `ruvector-nervous-system/src/hopfield/network.rs`: modern Hopfield network as associative memory
- `ruvector-gnn/src/ewc.rs`: `ElasticWeightConsolidation` for continual learning
- `ruvector-gnn/src/replay.rs`: `ReplayBuffer` for experience replay

However, there is no composition layer that integrates these primitives into graph transformer layers with proof-gated stability guarantees. The research at `docs/research/gnn-v2/23-biological-graph-transformers.md` describes the theoretical roadmap but does not map onto existing crate APIs or the proof-gated mutation protocol.

## Decision

We will implement a `biological` module in `ruvector-graph-transformer` behind the `biological` feature flag. The module provides four layer types: `SpikingGraphAttention`, `HebbianLayer`, `DendriticAttention`, and `StdpEdgeUpdater`, each integrated with proof-gated stability bounds.

### SpikingGraphAttention

Composes spike-driven attention with graph topology:

```rust
/// Spiking graph attention with edge-constrained spike propagation.
///
/// Bridges ruvector-mincut-gated-transformer::attention::spike_driven
/// with graph adjacency to route spikes only along edges.
/// Proof gate: membrane potential stability (spectral radius < 1.0).
pub struct SpikingGraphAttention {
    /// Spike-driven attention from ruvector-mincut-gated-transformer.
    spike_attn: SpikeDrivenAttention,
    /// Per-node membrane potentials (LIF model).
    membrane: Vec<f32>,
    /// Per-node refractory counters.
    refractory: Vec<u8>,
    /// Per-edge synaptic delays (in timesteps).
    edge_delays: Vec<u8>,
    /// Membrane decay constant (must be < 1.0 for stability).
    decay: f32,
    /// Spike threshold.
    threshold: f32,
    /// Proof requirement: spectral radius of effective operator < 1.0.
    stability_proof: ProofRequirement,
    /// Inhibition strategy for preventing synchrony collapse.
    inhibition: InhibitionStrategy,
}

/// The effective operator whose spectral radius is bounded.
///
/// The proof does not bound the raw weight matrix. It bounds the
/// effective operator: A_eff = diag(decay) * (W_adj ⊙ W_attn).
/// Power iteration estimates rho(A_eff) with variance; the proof
/// attests to: rho_estimated + safety_margin < 1.0, where
/// safety_margin = 3 * stddev(rho) over `num_iterations` runs.
///
/// ProofClass: Statistical { iterations: num_iterations, tolerance: safety_margin }.
pub struct EffectiveOperator {
    /// Number of power iteration rounds for spectral radius estimation.
    pub num_iterations: usize,
    /// Safety margin above estimated rho (3-sigma conservative).
    pub safety_margin: f32,
    /// Whether to use layerwise bounds (cheaper, tighter for block-diagonal).
    pub layerwise: bool,
}

/// Inhibition strategy for dense graphs where synchrony is a safety risk.
///
/// Inhibitory dynamics are CORE, not optional. Synchrony collapse on
/// dense graphs (degree > 100) is not a feature regression — it is a
/// safety failure. Without inhibition, proof-gated stability (rho < 1.0)
/// can still permit correlated firing that violates the independence
/// assumption in the spectral bound.
pub enum InhibitionStrategy {
    /// Winner-take-all: top-k nodes fire, rest are suppressed.
    /// From ruvector-nervous-system::compete::inhibition::WTA.
    WinnerTakeAll { k: usize },
    /// Lateral inhibition: each firing node suppresses neighbors
    /// with strength proportional to edge weight.
    /// From ruvector-nervous-system::compete::inhibition::Lateral.
    Lateral { strength: f32 },
    /// Balanced excitation/inhibition: maintain E/I ratio within bounds.
    /// Dale's law: each node is either excitatory or inhibitory, not both.
    BalancedEI { ei_ratio: f32, dale_law: bool },
}

impl SpikingGraphAttention {
    /// Process one timestep of spiking graph attention.
    ///
    /// Spikes propagate only along graph edges with per-edge delays.
    /// LIF membrane dynamics: V(t+1) = decay * V(t) + I_syn(t).
    /// Fires when V > threshold, then resets to 0.
    ///
    /// Proof gate verifies spectral radius of the effective operator
    /// A_eff = diag(decay) * (W_adj ⊙ W_attn) is below 1.0 to
    /// prevent runaway excitation. The bound is conservative:
    /// rho_estimated + 3*sigma < 1.0 (see EffectiveOperator).
    /// Routes to ProofTier::Standard(500) with ProofClass::Statistical.
    /// After step: inhibition is applied (core, not optional).
    pub fn step(
        &mut self,
        input_spikes: &[bool],
        graph: &impl GraphRepr,
        env: &mut ProofEnvironment,
    ) -> Result<ProofGate<SpikeOutput>>;

    /// Compute current firing rate per node (exponential moving average).
    pub fn firing_rates(&self) -> &[f32];
}
```

### HebbianLayer with EWC Protection

Local learning rules with catastrophic forgetting prevention:

```rust
/// Hebbian learning layer with Oja/BCM rules.
///
/// Weight updates are purely local: delta_w_ij = eta * f(x_i, y_j, w_ij).
/// Bridges to ruvector-gnn::ewc::ElasticWeightConsolidation to prevent
/// catastrophic forgetting when the graph evolves.
///
/// Proof gate: weight update must not increase Fisher-weighted
/// distance from consolidated parameters beyond a bound.
///
/// Constitutional rule: NO weight update proceeds without consuming
/// a ProofGate<HebbianUpdateResult>. The update method returns a
/// ProofGate, and the caller must unlock it to apply the weights.
/// This is not advisory — it is a type-level enforcement.
pub struct HebbianLayer {
    /// Learning rule variant.
    rule: HebbianRule,
    /// Learning rate.
    eta: f32,
    /// EWC from ruvector-gnn for consolidation.
    ewc: Option<ElasticWeightConsolidation>,
    /// Proof requirement: weight stability bound.
    stability_proof: ProofRequirement,
    /// Norm bound specification for EWC distance metric.
    norm_bound: HebbianNormBound,
}

/// Specifies how the Fisher-weighted norm bound is computed.
///
/// The bound ||w_new - w_consolidated||_F < threshold uses the
/// diagonal Fisher approximation (full Fisher is O(n^2) and
/// infeasible for large graphs). Layerwise bounds are tighter
/// than a single global bound because they exploit block-diagonal
/// structure.
pub struct HebbianNormBound {
    /// Maximum Fisher-weighted distance from consolidated weights.
    pub threshold: f32,
    /// Use diagonal Fisher approximation (always true in practice).
    pub diagonal_fisher: bool,
    /// Compute bounds per-layer rather than globally.
    /// Tighter but slightly more expensive (one norm per layer vs one total).
    pub layerwise: bool,
    /// ProofClass for this bound.
    /// Formal if diagonal Fisher is exact; Statistical if sampled.
    pub proof_class: ProofClass,
}

pub enum HebbianRule {
    /// Oja's rule: delta_w = eta * y * (x - w * y).
    /// Converges to first principal component.
    Oja,
    /// BCM rule: delta_w = eta * y * (y - theta_m) * x.
    /// theta_m is a sliding threshold (metaplasticity).
    BCM { theta_init: f32 },
    /// STDP: delta_w depends on spike timing (pre/post).
    /// Delegates to StdpEdgeUpdater.
    STDP { a_plus: f32, a_minus: f32, tau: f32 },
}

impl HebbianLayer {
    /// Apply one Hebbian weight update step.
    ///
    /// When EWC is active, the update is modified:
    ///   delta_w_ij = eta * hebb(x_i, y_j) - lambda * F_ij * (w_ij - w*_ij)
    /// where F_ij is the Fisher information and w*_ij are consolidated weights.
    ///
    /// Proof gate: verifies ||w_new - w_consolidated||_F < bound
    /// where ||.||_F is the diagonal Fisher-weighted norm, computed
    /// layerwise when `norm_bound.layerwise` is true.
    ///
    /// Constitutional rule: the returned ProofGate<HebbianUpdateResult>
    /// must be unlocked before weights are committed. There is no
    /// code path that writes weights without a satisfied gate.
    ///
    /// Routes to ProofTier::Standard (norm computation, < 1 us).
    pub fn update(
        &mut self,
        pre_activations: &[f32],
        post_activations: &[f32],
        weights: &mut [f32],
        graph: &impl GraphRepr,
        env: &mut ProofEnvironment,
    ) -> Result<ProofGate<HebbianUpdateResult>>;

    /// Consolidate current weights into EWC anchor.
    /// Called at task boundaries during continual learning.
    pub fn consolidate(&mut self, weights: &[f32]);
}
```

### DendriticAttention

Multi-compartment dendritic computation as attention:

```rust
/// Dendritic attention using compartment models.
///
/// Each graph node is modeled as a multi-compartment neuron
/// (from ruvector-nervous-system::dendrite). Different dendritic
/// branches attend to different subsets of graph neighbors,
/// enabling multiplicative gating without explicit gating networks.
///
/// Bridges to:
/// - ruvector_nervous_system::dendrite::compartment::Compartment
/// - ruvector_nervous_system::dendrite::coincidence::CoincidenceDetector
/// - ruvector_nervous_system::dendrite::plateau::PlateauGenerator
pub struct DendriticAttention {
    /// Number of dendritic branches per node.
    num_branches: usize,
    /// Compartment model parameters.
    compartment_config: CompartmentConfig,
    /// Branch-to-neighbor assignment (learned or heuristic).
    branch_assignment: BranchAssignment,
    /// Plateau potential threshold for nonlinear dendritic events.
    plateau_threshold: f32,
}

pub enum BranchAssignment {
    /// Assign neighbors to branches round-robin by degree.
    RoundRobin,
    /// Cluster neighbors by feature similarity, one branch per cluster.
    FeatureClustered { num_clusters: usize },
    /// Learned assignment via attention routing.
    Learned,
}

impl DendriticAttention {
    /// Forward pass: route neighbor messages to dendritic branches,
    /// compute compartment dynamics, trigger plateau potentials.
    ///
    /// The output is the soma (cell body) voltage after dendritic
    /// integration. Plateau potentials provide nonlinear amplification
    /// of coincident inputs on the same branch.
    pub fn forward(
        &self,
        features: &[f32],
        graph: &impl GraphRepr,
        env: &mut ProofEnvironment,
    ) -> Result<ProofGate<DendriticOutput>>;
}
```

### StdpEdgeUpdater

STDP-driven graph rewiring with proof-gated stability:

```rust
/// STDP edge update with two proof-gated tiers:
///
/// 1. **Weight updates** (Standard tier): Causal spike timing
///    potentiates edges; anti-causal timing depresses edges.
///    Stability certificate proves rho(A_eff) < 1.0.
///
/// 2. **Topology changes** (Deep tier): When edge weight drops
///    below `prune_threshold`, the edge is removed. When a node
///    pair has sustained high co-firing rate, a new edge is added.
///    Topology changes require Deep tier proof because they alter
///    the graph Laplacian and can invalidate partition boundaries.
///
/// Both operations return ProofGate. Topology changes are strictly
/// more expensive and are batched per epoch, not per timestep.
pub struct StdpEdgeUpdater {
    a_plus: f32,
    a_minus: f32,
    tau_plus: f32,
    tau_minus: f32,
    /// Last spike time per node (for timing computation).
    last_spike: Vec<f64>,
    /// Weight bounds [min, max] to prevent degenerate solutions.
    weight_bounds: (f32, f32),
    /// Threshold below which edges are pruned (topology change).
    prune_threshold: f32,
    /// Co-firing threshold above which new edges are created.
    growth_threshold: f32,
    /// Maximum edges that can be added per epoch (budget).
    max_new_edges_per_epoch: usize,
}

impl StdpEdgeUpdater {
    /// Update edge weights based on recent spike history.
    /// Weight-only: does not change graph topology.
    ///
    /// Routes to ProofTier::Standard(500).
    /// Returns ProofGate<StdpWeightResult> with stability certificate.
    pub fn update_weights(
        &mut self,
        graph: &impl GraphRepr,
        env: &mut ProofEnvironment,
    ) -> Result<ProofGate<StdpWeightResult>>;

    /// Rewire graph topology based on accumulated STDP statistics.
    /// Prunes weak edges, grows edges between co-firing pairs.
    ///
    /// Routes to ProofTier::Deep because topology changes affect:
    /// - Min-cut partition boundaries (ProofScope invalidation)
    /// - Graph Laplacian eigenvalues (spectral sparsification)
    /// - Attestation chain (ScopeTransitionAttestation required)
    ///
    /// Returns ProofGate<StdpTopologyResult> with:
    /// - edges_pruned, edges_added counts
    /// - new spectral radius bound
    /// - ScopeTransitionAttestation if partitions changed
    pub fn rewire_topology(
        &mut self,
        graph: &mut impl GraphRepr,
        env: &mut ProofEnvironment,
    ) -> Result<ProofGate<StdpTopologyResult>>;
}
```

### Proof-Gated Plasticity Protocol

All weight update mechanisms (Hebbian, STDP, dendritic plateau) are gated through the proof system:

| Update Type | Proof Requirement | Tier | Latency | ProofClass |
|-------------|------------------|------|---------|------------|
| Oja/BCM weight step | Fisher-weighted norm bound (diagonal, layerwise) | Standard(200) | < 1 us | Formal (diagonal exact) or Statistical (sampled Fisher) |
| STDP weight update | rho(A_eff) + 3σ < 1.0 | Standard(500) | < 5 us | Statistical { iterations, safety_margin } |
| STDP topology rewire | Laplacian + partition integrity | Deep | < 100 us | Formal (exact edge count) + Statistical (spectral bound) |
| Plateau potential | Membrane stability bound | Reflex | < 10 ns | Formal |
| EWC consolidation | Fisher diagonal computation | Deep | < 100 us | Formal |
| Inhibition enforcement | E/I ratio within bounds | Reflex | < 10 ns | Formal |

### Feature Flag

```toml
# In crates/ruvector-graph-transformer/Cargo.toml
[features]
biological = [
    "ruvector-mincut-gated-transformer/spike_attention",
    "ruvector-gnn",
]
```

The `ruvector-nervous-system` dependency is optional and gated behind a sub-feature `biological-dendritic`:

```toml
biological-dendritic = ["biological", "ruvector-nervous-system"]
```

## Consequences

### Positive

- Event-driven spiking attention skips 99%+ of node computations, enabling significant energy reduction for sparse graph workloads (the exact factor is hardware-dependent: 87x is measured on neuromorphic hardware with native spike support; on von Neumann architectures the reduction is lower due to memory access patterns)
- Local Hebbian learning eliminates global backpropagation dependency, enabling truly distributed graph learning
- EWC integration prevents catastrophic forgetting during continual graph learning
- Dendritic attention provides multiplicative gating without explicit gating parameters
- Proof-gated stability (spectral radius < 1.0) prevents runaway excitation cascades
- STDP self-organizes edge weights based on temporal structure, pruning redundant connections

### Negative

- Spiking models require choosing a simulation timestep, adding a hyperparameter not present in standard graph transformers
- Hebbian rules converge to principal components, which may not align with downstream task objectives; requires hybrid training (Hebbian pre-training + fine-tuning)
- DendriticAttention introduces per-node compartment state, increasing memory by `num_branches * compartment_dim` per node
- Spectral radius estimation via power iteration has variance; the `EffectiveOperator` uses a conservative 3-sigma bound (rho_est + 3σ < 1.0) with configurable iteration count. If variance is too high (σ > 0.05), the proof gate rejects and forces a re-estimation with more iterations

### Risks

- Spiking graph attention on dense graphs (degree > 100) may produce pathological synchronization (all nodes fire simultaneously). Mitigation: `InhibitionStrategy` is CORE, not optional — synchrony collapse is a safety failure. The `BalancedEI` variant enforces Dale's law and maintains E/I ratio within proven bounds. Refractory periods provide the first line of defense; inhibition provides the structural guarantee
- BCM metaplasticity threshold drift can cause learning shutdown if the graph distribution shifts. Mitigation: periodic threshold reset via EWC anchor points
- Neuromorphic hardware mapping (Loihi 2 core allocation mentioned in the research doc) is out of scope for this ADR; it requires hardware-specific compilation not available in the Rust toolchain today

### Design Decisions

**Q: Are inhibitory dynamics core or an optional module?**

Core. Synchrony collapse on dense graphs is a safety failure, not a feature regression. Without inhibition, the spectral radius bound can be satisfied (rho < 1.0) while correlated firing still violates the independence assumption in the bound. `InhibitionStrategy` is a required field on `SpikingGraphAttention`, not an optional module behind a feature flag. The `BalancedEI` variant is the recommended default for graphs with mean degree > 50.

**Q: Does STDP rewiring change topology or weights only?**

Both, at different proof tiers. Weight updates are Standard tier (frequent, cheap, per-timestep). Topology changes (edge pruning and growth) are Deep tier (expensive, batched per epoch). This separation exists because topology changes invalidate min-cut partitions and require `ScopeTransitionAttestation`, while weight changes within a fixed topology preserve partition boundaries. The `StdpEdgeUpdater` exposes `update_weights()` and `rewire_topology()` as separate methods with different proof gates.

### Missing Layer: BTSP and e-prop

This ADR does not yet define a `BtspLayer` or `EpropLayer` as first-class graph transformer components. The primitives exist in `ruvector-nervous-system::plasticity::{btsp, eprop}` and should be composed into graph transformer layers in a follow-up ADR. The key integration question is how eligibility traces (e-prop) interact with the proof-gated mutation protocol — each trace update is a stateful mutation that should carry a lightweight Reflex-tier proof.

### Acceptance Tests

1. `test_synchrony_invariant`: Create a fully connected 200-node spiking graph. Run 1000 timesteps without inhibition — verify synchrony collapse (>90% simultaneous firing). Enable `BalancedEI` inhibition — verify firing rate stays below 20% per timestep. The proof gate must reject any step where E/I ratio exceeds bounds.

2. `test_hebbian_constitutional_rule`: Attempt to apply Hebbian weight update without unlocking the ProofGate. Verify compile-time enforcement (the weight buffer is only accessible via `ProofGate::unlock()`). At runtime, verify that a HebbianLayer with `norm_bound.threshold = 0.001` rejects a large learning rate step.

3. `test_stdp_topology_tier_separation`: Run STDP on a 500-node graph for 100 timesteps. Verify all weight updates route to Standard tier. Trigger topology rewire (edge pruning). Verify it routes to Deep tier and produces `ScopeTransitionAttestation`. Verify total attestation chain length matches expected (100 Standard + 1 Deep).

4. `test_spectral_radius_conservative_bound`: Construct a weight matrix with known spectral radius 0.95. Run `EffectiveOperator` estimation with 20 iterations. Verify the estimated bound + 3σ < 1.0. Reduce `safety_margin` to 0.001 — verify the proof gate rejects (too tight).

## Implementation

1. Create `crates/ruvector-graph-transformer/src/biological/mod.rs` re-exporting all types including `EffectiveOperator`, `InhibitionStrategy`, `HebbianNormBound`
2. Implement `SpikingGraphAttention` in `biological/spiking.rs`, bridging to `ruvector-mincut-gated-transformer::attention::spike_driven`, with mandatory `InhibitionStrategy` and `EffectiveOperator`
3. Implement `HebbianLayer` in `biological/hebbian.rs`, bridging to `ruvector-gnn::ewc::ElasticWeightConsolidation`, with `HebbianNormBound` (diagonal Fisher, layerwise)
4. Implement `StdpEdgeUpdater` in `biological/stdp.rs` with two-tier proof gates: `update_weights()` at Standard, `rewire_topology()` at Deep
5. Implement `DendriticAttention` in `biological/dendritic.rs`, bridging to `ruvector-nervous-system::dendrite::{compartment, coincidence, plateau}`
6. Add benchmark: `benches/biological_bench.rs` measuring spike throughput on a 10,000-node graph over 1,000 timesteps, with and without inhibition
7. Integration test: spiking graph attention + STDP update loop for 100 steps, verify stability attestation chain including tier distribution
8. Run acceptance tests 1-4 defined above
9. Verify build: `cargo test --features biological -p ruvector-graph-transformer`

## References

- ADR-046: Graph Transformer Unified Architecture (module structure, feature flags)
- ADR-047: Proof-Gated Mutation Protocol (`ProofGate<T>`, `ProofRequirement`, spectral radius invariants)
- ADR-049: Verified Training Pipeline (per-step invariant verification, `LipschitzBound`)
- Research: `docs/research/gnn-v2/23-biological-graph-transformers.md`
- `crates/ruvector-mincut-gated-transformer/src/attention/spike_driven.rs`: `SpikeDrivenAttention`
- `crates/ruvector-mincut-gated-transformer/src/spike.rs`: `SpikeScheduler`, novelty gating
- `crates/ruvector-nervous-system/src/dendrite/compartment.rs`: `Compartment` model
- `crates/ruvector-nervous-system/src/dendrite/coincidence.rs`: `CoincidenceDetector`
- `crates/ruvector-nervous-system/src/dendrite/plateau.rs`: `PlateauGenerator`
- `crates/ruvector-nervous-system/src/plasticity/btsp.rs`: BTSP with eligibility traces
- `crates/ruvector-nervous-system/src/plasticity/eprop.rs`: e-prop learning
- `crates/ruvector-nervous-system/src/plasticity/consolidate.rs`: synaptic consolidation
- `crates/ruvector-nervous-system/src/compete/inhibition.rs`: lateral inhibition
- `crates/ruvector-gnn/src/ewc.rs`: `ElasticWeightConsolidation`
- `crates/ruvector-gnn/src/replay.rs`: `ReplayBuffer`, `ReplayEntry`
- `crates/ruvector-verified/src/gated.rs`: `ProofTier`, `route_proof`, `ProofClass`
- `crates/ruvector-nervous-system/src/compete/inhibition.rs`: `WTA`, `Lateral`, `BalancedEI`
- Bellec et al., "A solution to the learning dilemma for recurrent networks of spiking neurons" (Nature Comms, 2020) -- e-prop
- Bittner et al., "Behavioral time scale synaptic plasticity" (Neuron, 2017)
- Oja, "Simplified neuron model as a principal component analyzer" (J Math Bio, 1982)
