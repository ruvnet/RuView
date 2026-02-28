# ADR-053: Temporal and Causal Graph Transformer Layers

## Status

Accepted

## Date

2026-02-25

## Context

Most real-world graphs evolve over time: social networks rewire daily, financial transaction graphs stream continuously, biological interaction networks change with cellular state. Standard graph transformers treat the graph as a static snapshot, computing attention over a fixed adjacency matrix. This causes stale representations, causal confusion (future events leaking into past representations), and missing dynamics (temporal patterns carry signal that static embeddings cannot capture).

RuVector has extensive infrastructure for temporal and causal graph processing:

- `ruvector-dag/src/attention/causal_cone.rs`: `CausalConeAttention` focusing on ancestors with temporal discount
- `ruvector-dag/src/attention/temporal_btsp.rs`: Behavioral Timescale Synaptic Plasticity attention with eligibility traces
- `ruvector-dag/src/attention/topological.rs`: topological attention respecting DAG structure
- `ruvector-dag/src/dag/traversal.rs`: DAG traversal, topological sort, ancestor/descendant queries
- `ruvector-dag/src/dag/query_dag.rs`: query DAG construction
- `ruvector-temporal-tensor/src/delta.rs`: `DeltaChain` for sparse temporal compression
- `ruvector-temporal-tensor/src/tier_policy.rs`: hot/warm/cold tiered storage policies
- `ruvector-temporal-tensor/src/tiering.rs`: tiered tensor storage implementation
- `ruvector-attention/src/hyperbolic/lorentz_cascade.rs`: `LorentzCascadeAttention` with Busemann scoring (Lorentz metric is spacetime metric)
- `ruvector-graph/`: property graph with temporal metadata, Cypher queries

However, there is no composition layer that enforces causal ordering through the proof system, provides continuous-time ODE dynamics on graphs, or extracts Granger causality from attention weights with structural certificates. The research at `docs/research/gnn-v2/28-temporal-causal-graph-transformers.md` describes the theory but provides no integration path with the proof-gated mutation protocol.

## Decision

We will implement a `temporal` module in `ruvector-graph-transformer` behind the `temporal` feature flag. The module provides causal graph attention with proof-gated temporal ordering, retrocausal safety enforcement, continuous-time neural ODE on graphs, Granger causality extraction, and delta chain integration for temporal compression.

### CausalGraphTransformer

Causal masking with proof-gated temporal ordering:

```rust
/// Causal graph transformer with proof-gated temporal mutations.
///
/// Every temporal mutation must prove that its timestamp is strictly
/// greater than all predecessor timestamps in the causal cone.
/// Bridges to ruvector-dag::attention::causal_cone::CausalConeAttention.
pub struct CausalGraphTransformer {
    /// Causal cone attention from ruvector-dag.
    causal_attention: CausalConeAttention,
    /// Mask strategy: Strict, TimeWindow, or Topological.
    mask_strategy: MaskStrategy,
    /// Temporal discount factor for ancestor weighting.
    discount: f32,
    /// Whether retrocausal (bidirectional) mode is permitted.
    allow_retrocausal: bool,
    /// Proof requirement: causal ordering.
    causal_proof: ProofRequirement,
}

pub enum MaskStrategy {
    /// Strict: only ancestors in the DAG may attend.
    Strict,
    /// TimeWindow: ancestors within a fixed time window.
    TimeWindow { window_size: f64 },
    /// Topological: attention follows topological ordering.
    Topological,
}

impl CausalGraphTransformer {
    /// Causal forward pass.
    ///
    /// For each node v at time t, computes attention only over
    /// nodes u with timestamp t_u <= t. The causal ordering is
    /// verified via proof gate:
    ///
    ///   ProofRequirement::InvariantPreserved {
    ///       invariant_id: CAUSAL_ORDERING_INVARIANT,
    ///   }
    ///
    /// Routes to ProofTier::Reflex for timestamp comparisons (< 10 ns)
    /// since these are scalar comparisons.
    pub fn forward(
        &self,
        features: &[f32],
        timestamps: &[f64],
        graph: &impl GraphRepr,
        env: &mut ProofEnvironment,
    ) -> Result<ProofGate<TemporalOutput>>;

    /// Interventional query: compute P(h_v(t) | do(h_u(t') = x)).
    ///
    /// Severs incoming edges to the intervened node and propagates
    /// the intervention downstream through the causal graph.
    /// Uses ruvector-dag::dag::traversal for descendant computation.
    pub fn intervene(
        &self,
        target_node: NodeId,
        target_time: f64,
        intervention_value: &[f32],
        graph: &impl GraphRepr,
        env: &mut ProofEnvironment,
    ) -> Result<ProofGate<InterventionResult>>;
}
```

### Retrocausal Safety

Bidirectional temporal attention is only permitted in offline/batch mode:

```rust
/// Retrocausal attention with strict safety enforcement.
///
/// Forward (causal) pass: h_v^->(t) uses only events at t' <= t.
/// Backward (retrocausal) pass: h_v^<-(t) uses only events at t' >= t.
/// Smoothed: h_v(t) = gate(h_v^->(t), h_v^<-(t)).
///
/// The retrocausal pass is ONLY invoked when `mode == TemporalMode::Batch`.
/// In online/streaming mode, the proof gate REJECTS any attempt to
/// access future timestamps. This is enforced at the type level:
/// `RetrocausalAttention::forward` requires `&BatchModeToken`, which
/// can only be constructed when the full temporal window is available.
pub struct RetrocausalAttention {
    forward_attention: CausalConeAttention,
    backward_attention: CausalConeAttention,
    gate: LearnedGate,
}

/// Token proving batch mode is active. Cannot be constructed in streaming mode.
pub struct BatchModeToken { _private: () }

impl RetrocausalAttention {
    /// Bidirectional smoothed attention. Requires batch mode proof.
    pub fn forward(
        &self,
        features: &[f32],
        timestamps: &[f64],
        graph: &impl GraphRepr,
        batch_token: &BatchModeToken,
        env: &mut ProofEnvironment,
    ) -> Result<ProofGate<SmoothedOutput>>;
}
```

### ContinuousTimeODE

Neural ODE on graphs with adaptive integration:

```rust
/// Continuous-time graph network via neural ODE.
///
/// dh_v(t)/dt = f_theta(h_v(t), {h_u(t) : u in N(v, t)}, t)
///
/// Uses adaptive Dormand-Prince (RK45) integration with proof-gated
/// error control. The error tolerance proof ensures the local
/// truncation error stays below a configurable bound.
pub struct ContinuousTimeODE {
    /// Hidden dimension.
    dim: usize,
    /// ODE solver tolerance (absolute).
    atol: f64,
    /// ODE solver tolerance (relative).
    rtol: f64,
    /// Maximum integration steps (prevents infinite loops).
    max_steps: usize,
    /// Proof requirement: integration error bound.
    error_proof: ProofRequirement,
}

impl ContinuousTimeODE {
    /// Integrate node embeddings from t_start to t_end.
    ///
    /// The neighborhood N(v, t) changes as edges appear/disappear.
    /// Edge events between t_start and t_end are processed in order.
    /// Proof gate verifies local truncation error at each adaptive step
    /// via ProofTier::Standard (error norm computation).
    pub fn integrate(
        &self,
        features: &mut [f32],
        t_start: f64,
        t_end: f64,
        edge_events: &[TemporalEdgeEvent],
        graph: &impl GraphRepr,
        env: &mut ProofEnvironment,
    ) -> Result<ProofGate<OdeOutput>>;
}

pub struct TemporalEdgeEvent {
    pub source: NodeId,
    pub target: NodeId,
    pub timestamp: f64,
    pub event_type: EdgeEventType,
}

pub enum EdgeEventType {
    Add,
    Remove,
    UpdateWeight(f32),
}
```

### Granger Causality Extraction

Extract causal structure from learned attention weights:

```rust
/// Granger causality extraction from temporal attention weights.
///
/// Computes time-averaged attention weights and thresholds them
/// to produce a Granger-causal DAG. The DAG is stored in
/// ruvector-dag format for efficient traversal and querying.
///
/// A structural certificate attests that the extracted graph is
/// acyclic (a valid DAG) and that edge weights exceed the
/// significance threshold.
pub struct GrangerCausalityExtractor {
    /// Significance threshold for edge inclusion.
    threshold: f64,
    /// Minimum time window for averaging attention weights.
    min_window: usize,
}

impl GrangerCausalityExtractor {
    /// Extract Granger-causal graph from temporal attention history.
    ///
    /// Returns a DAG with edge weights = time-averaged attention.
    /// The proof gate certifies acyclicity via topological sort
    /// from ruvector-dag::dag::traversal (ProofTier::Standard).
    pub fn extract(
        &self,
        attention_history: &[AttentionSnapshot],
        timestamps: &[f64],
        env: &mut ProofEnvironment,
    ) -> Result<ProofGate<GrangerGraph>>;
}
```

### Delta Chain Integration

Temporal compression via `ruvector-temporal-tensor`:

```rust
/// Temporal embedding storage with delta chain compression.
///
/// Bridges to ruvector-temporal-tensor::delta::DeltaChain for
/// storing node embedding histories as base + sparse deltas.
/// Retrieval of h_v(t) for any historical time t is O(chain_length).
///
/// Tiered storage (hot/warm/cold) via ruvector-temporal-tensor::tiering
/// keeps recent embeddings in memory and older ones on disk.
pub struct TemporalEmbeddingStore {
    /// Delta chain per node.
    chains: Vec<DeltaChain>,
    /// Tier policy from ruvector-temporal-tensor.
    tier_policy: TierPolicy,
}

impl TemporalEmbeddingStore {
    /// Store a new embedding snapshot for node v at time t.
    /// Computes delta from previous snapshot and appends to chain.
    pub fn store(&mut self, node: NodeId, time: f64, embedding: &[f32]);

    /// Retrieve embedding at historical time t via delta replay.
    pub fn retrieve(&self, node: NodeId, time: f64) -> Option<Vec<f32>>;

    /// Compact old deltas according to tier policy.
    pub fn compact(&mut self);
}
```

### Proof-Gated Temporal Mutations

| Operation | Proof Requirement | Tier | Latency |
|-----------|------------------|------|---------|
| Timestamp ordering (causal mask) | `t_new > t_predecessor` | Reflex | < 10 ns |
| Retrocausal mode check | Batch mode token valid | Reflex | < 10 ns |
| ODE error bound | Local truncation error < atol | Standard(100) | < 1 us |
| Granger DAG acyclicity | Topological sort succeeds | Standard(500) | < 5 us |
| Interventional propagation | Causal cone completeness | Deep | < 50 us |

### Feature Flag

```toml
# In crates/ruvector-graph-transformer/Cargo.toml
[features]
temporal = [
    "ruvector-dag/attention",
    "ruvector-temporal-tensor",
    "ruvector-graph/temporal",
]
```

## Consequences

### Positive

- Causal ordering is enforced by the proof system, preventing future information leakage that corrupts online predictions
- Retrocausal safety is enforced at the type level (`BatchModeToken`), making it impossible to accidentally use bidirectional attention in streaming mode
- Continuous-time ODE handles irregular event streams without discretization artifacts
- Granger causality extraction produces auditable causal graphs with structural certificates
- Delta chain compression reduces temporal embedding storage by 10-100x compared to full snapshots

### Negative

- Causal masking reduces effective attention receptive field compared to full (non-causal) attention
- Neural ODE integration with adaptive stepping has variable compute cost per forward pass
- Granger causality extraction requires accumulating attention history, adding O(T * n^2 / sparsity) memory
- Delta chain retrieval for deep historical queries is O(chain_length), not O(1)

### Risks

- In streaming mode with high event rates (>10K events/sec), causal cone computation may become a bottleneck. Mitigation: maintain incremental ancestor sets using `ruvector-dag::dag::traversal` with cached topological order
- ODE solver may fail to converge for stiff graph dynamics. Mitigation: fall back to implicit Euler with Newton iteration when adaptive RK45 exceeds max_steps
- Retrocausal attention smoothing may overfit to the specific temporal window available in batch mode. Mitigation: temporal cross-validation with held-out future windows

## Implementation

1. Create `crates/ruvector-graph-transformer/src/temporal/mod.rs` re-exporting all types
2. Implement `CausalGraphTransformer` in `temporal/causal.rs`, bridging to `ruvector-dag::attention::causal_cone`
3. Implement `RetrocausalAttention` in `temporal/retrocausal.rs` with `BatchModeToken` type safety
4. Implement `ContinuousTimeODE` in `temporal/ode.rs` with adaptive Dormand-Prince integration
5. Implement `GrangerCausalityExtractor` in `temporal/granger.rs` using `ruvector-dag::dag::traversal`
6. Implement `TemporalEmbeddingStore` in `temporal/store.rs`, bridging to `ruvector-temporal-tensor::delta::DeltaChain`
7. Add benchmark: `benches/temporal_bench.rs` measuring causal attention throughput on a 100K-event stream over 10K nodes
8. Integration test: streaming causal attention for 1,000 events + Granger extraction, verify DAG acyclicity certificate
9. Verify build: `cargo test --features temporal -p ruvector-graph-transformer`

## References

- ADR-046: Graph Transformer Unified Architecture (module structure, `temporal` feature flag)
- ADR-047: Proof-Gated Mutation Protocol (`ProofGate<T>`, timestamp ordering invariants)
- ADR-049: Verified Training Pipeline (temporal invariant checking during training)
- Research: `docs/research/gnn-v2/28-temporal-causal-graph-transformers.md`
- `crates/ruvector-dag/src/attention/causal_cone.rs`: `CausalConeAttention`, `MaskStrategy`
- `crates/ruvector-dag/src/attention/temporal_btsp.rs`: BTSP attention with eligibility traces
- `crates/ruvector-dag/src/attention/topological.rs`: topological attention
- `crates/ruvector-dag/src/dag/traversal.rs`: topological sort, ancestor/descendant queries
- `crates/ruvector-dag/src/dag/query_dag.rs`: query DAG construction
- `crates/ruvector-temporal-tensor/src/delta.rs`: `DeltaChain` for sparse delta compression
- `crates/ruvector-temporal-tensor/src/tier_policy.rs`: `TierPolicy` for hot/warm/cold storage
- `crates/ruvector-temporal-tensor/src/tiering.rs`: tiered storage implementation
- `crates/ruvector-attention/src/hyperbolic/lorentz_cascade.rs`: `LorentzCascadeAttention`
- `crates/ruvector-verified/src/gated.rs`: `ProofTier`, `route_proof`
- Granger, "Investigating Causal Relations by Econometric Models and Cross-spectral Methods" (Econometrica, 1969)
- Chen et al., "Neural Ordinary Differential Equations" (NeurIPS, 2018)
- Pearl, "Causality: Models, Reasoning, and Inference" (Cambridge, 2009)
