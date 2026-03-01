# Axis 8: Temporal -- Causal & Retrocausal Graph Transformers

**Document:** 28 of 30
**Series:** Graph Transformers: 2026-2036 and Beyond
**Last Updated:** 2026-02-25
**Status:** Research Prospectus

---

## 1. Problem Statement

Graphs change over time. Social networks gain and lose connections. Knowledge graphs accumulate facts. Molecular configurations evolve. Financial transaction graphs grow continuously. Standard graph transformers process static snapshots, losing the temporal dimension entirely.

The temporal axis asks: how do we build graph transformers that reason about time as a first-class concept?

### 1.1 Temporal Graph Categories

| Category | Edge Lifetime | Node Lifetime | Example |
|----------|--------------|--------------|---------|
| Static | Infinite | Infinite | Crystal structures |
| Growing | Infinite | Infinite | Citation networks |
| Evolving | Finite, variable | Infinite | Social networks |
| Streaming | Finite, short | Finite | Financial transactions |
| Episodic | Periodic | Periodic | Daily commute patterns |

### 1.2 RuVector Baseline

- **`ruvector-temporal-tensor`**: Delta compression (`delta.rs`), tiered storage (`tiering.rs`), coherence tracking (`coherence.rs`), segment-based storage (`segment.rs`)
- **`ruvector-gnn`**: Continual learning via EWC (`ewc.rs`), replay buffers (`replay.rs`)
- **`ruvector-attention`**: Existing causal attention research (Doc 11)
- **`ruvector-graph`**: Distributed mode with temporal queries

---

## 2. Causal Graph Transformers

### 2.1 Causal Structure on Graphs

A causal graph transformer respects the arrow of time: node v at time t can only attend to nodes at times t' <= t. This is the temporal analog of the causal mask in autoregressive transformers, but on a graph.

**Causal attention mask:**
```
M_{causal}(u, t_u, v, t_v) =
  1  if t_v <= t_u and (u, v) in E_temporal
  0  otherwise
```

**Subtlety:** In temporal graphs, edges have timestamps too. An edge (u, v, t_e) means u and v interacted at time t_e. The causal constraint is:

```
Node v at time t can attend to node u at time t' only if:
  1. t' < t  (temporal ordering)
  2. There exists a temporal path from u at t' to v at t
     through edges with non-decreasing timestamps
```

### 2.2 Temporal Graph Attention Network (TGAT)

```
TGAT Layer:

Input: Temporal graph G_t, node features X, timestamps T

For each node v at time t:
  1. Gather temporal neighbors:
     N(v, t) = {(u, t_e) : (u, v, t_e) in E, t_e <= t, t - t_e < window}

  2. Compute temporal encoding:
     phi(t - t_e) = [cos(w_1 * (t-t_e)), sin(w_1 * (t-t_e)), ...,
                     cos(w_d * (t-t_e)), sin(w_d * (t-t_e))]
     // Fourier features of time difference

  3. Compute attention with temporal encoding:
     Q = W_Q * [h_v || phi(0)]
     K_u = W_K * [h_u || phi(t - t_e)]
     V_u = W_V * [h_u || phi(t - t_e)]

     alpha_{vu} = softmax_u(Q . K_u^T / sqrt(d))

  4. Aggregate:
     h_v^{new} = sum_{(u,t_e) in N(v,t)} alpha_{vu} * V_u
```

### 2.3 Continuous-Time Attention via Neural ODEs

Instead of discrete time steps, define attention dynamics as a continuous ODE:

```
dh_v/dt = f_theta(h_v(t), {h_u(t) : u in N(v)}, t)

where f_theta is a learned function incorporating attention:

f_theta(h_v, neighbors, t) =
  sum_{u in N(v)} alpha(h_v, h_u, t) * message(h_u, t)
  + self_dynamics(h_v, t)

alpha(h_v, h_u, t) = softmax(Q(h_v, t) . K(h_u, t)^T / sqrt(d))
```

**Solve with ODE solver:**
```
h(t_1) = ODESolve(f_theta, h(t_0), t_0, t_1)
// Adaptive step-size solver (Dormand-Prince, etc.)
```

**Advantage:** Can query the graph state at any continuous time point, not just discrete snapshots.

**RuVector integration:**

```rust
/// Continuous-time graph attention
pub trait ContinuousTimeAttention {
    /// Compute node representations at arbitrary time t
    fn query_at_time(
        &self,
        graph: &TemporalGraph,
        node: NodeId,
        time: f64,
    ) -> Result<Tensor, TemporalError>;

    /// Compute attention weights at time t
    fn attention_at_time(
        &self,
        graph: &TemporalGraph,
        query_node: NodeId,
        query_time: f64,
    ) -> Result<Vec<(NodeId, f64, f32)>, TemporalError>;
    // Returns: [(neighbor_id, event_time, attention_weight)]

    /// Evolve all node states from t0 to t1
    fn evolve(
        &mut self,
        graph: &TemporalGraph,
        t0: f64,
        t1: f64,
        step_size: f64,
    ) -> Result<(), TemporalError>;

    /// Get temporal attention trajectory for a node
    fn attention_trajectory(
        &self,
        node: NodeId,
        t_start: f64,
        t_end: f64,
        num_points: usize,
    ) -> Result<Vec<(f64, Vec<f32>)>, TemporalError>;
}
```

---

## 3. Time-Crystal Dynamics in Graph Attention

### 3.1 What are Time Crystals?

In physics, a time crystal is a state of matter whose ground state exhibits periodic motion -- it breaks time-translation symmetry spontaneously. In graph transformers, a time crystal is an attention pattern that oscillates periodically without external driving.

### 3.2 Time-Crystal Attention

**Definition.** A graph attention pattern alpha(t) is a time crystal if:
1. alpha(t + T) = alpha(t) for some period T (periodic)
2. The periodicity is spontaneous (not imposed by input periodicity)
3. The system is in a stable state (ground state or metastable)

**Construction:**

```
Time-crystal graph attention dynamics:

dh_v/dt = -dE/dh_v + noise

Energy functional:
  E = sum_{(u,v)} J_{uv} * ||h_u(t) - h_v(t-tau)||^2
      + sum_v U(h_v)
      - lambda * sum_v ||dh_v/dt||^2

The third term (negative kinetic energy penalty) drives oscillation.
When lambda exceeds a critical value lambda_c, the ground state
spontaneously oscillates with period T ~ 2 * tau.
```

**Graph attention from time-crystal dynamics:**
```
alpha_{uv}(t) = exp(-J_{uv} * ||h_u(t) - h_v(t-tau)||^2)
                / sum_w exp(-J_{uw} * ||h_u(t) - h_w(t-tau)||^2)
```

**Interpretation:** The attention weights oscillate periodically. Different phases of the oscillation capture different aspects of the graph structure. This is analogous to how the brain uses oscillatory dynamics (theta, gamma rhythms) to multiplex different types of information.

### 3.3 Applications of Time-Crystal Attention

1. **Periodic pattern detection**: Financial cycles, seasonal trends, biological rhythms
2. **Multi-phase reasoning**: Different attention patterns activated at different phases
3. **Memory through oscillation**: Information persists in the oscillation pattern, not in static weights
4. **Temporal multiplexing**: Multiple attention patterns time-share the same graph

---

## 4. Retrocausal Attention

### 4.1 The Concept

Retrocausal attention allows information to flow "backward in time" -- future events influence past representations. This is not time travel; it is bidirectional processing with information-theoretic constraints to prevent paradoxes.

**Standard causal attention:** h(t) depends on h(t') for t' <= t only.

**Retrocausal attention:** h(t) depends on h(t') for *all* t', with constraints:

```
h_v^{forward}(t) = f(h_u(t') : t' <= t, u in N(v))   // Causal
h_v^{backward}(t) = g(h_u(t') : t' >= t, u in N(v))  // Retrocausal
h_v^{combined}(t) = Merge(h_v^{forward}(t), h_v^{backward}(t))
```

### 4.2 Information-Theoretic Constraints

To prevent "cheating" (using future ground truth to predict the past), we impose:

**Constraint 1: Information bottleneck.**
```
I(h^{backward}(t) ; Y(t')) <= C  for t' > t
// Mutual information between backward representation and future labels is bounded
```

**Constraint 2: No label leakage.**
```
h^{backward}(t) must be computable from unlabeled future observations only
// Future features OK, future labels not OK
```

**Constraint 3: Temporal consistency.**
```
The combined representation must be consistent:
P(Y(t) | h^{combined}(t)) >= P(Y(t) | h^{forward}(t))
// Retrocausal information can only help, never hurt
```

### 4.3 Retrocausal Graph Attention Architecture

```
Retrocausal Graph Transformer:

Forward pass (left to right in time):
  For t = 1 to T:
    h^{fwd}(t) = CausalAttention(h^{fwd}(t-1), neighbors_past)

Backward pass (right to left in time):
  For t = T down to 1:
    h^{bwd}(t) = CausalAttention(h^{bwd}(t+1), neighbors_future)

Merge:
  For t = 1 to T:
    h^{combined}(t) = Gate(h^{fwd}(t), IB(h^{bwd}(t), C))
    // IB = information bottleneck, limiting backward info to C bits

    Gate(f, b) = sigma(W_g * [f || b]) * f + (1 - sigma(W_g * [f || b])) * b
```

### 4.4 Retrocausal Applications

| Application | Forward Signal | Backward Signal | Benefit |
|-------------|---------------|----------------|---------|
| Anomaly detection | Past normal behavior | Future anomaly effects | Earlier detection |
| Link prediction | Past connectivity | Future graph evolution | Better prediction |
| Event forecasting | Historical events | Future event echoes | Improved accuracy |
| Debugging | Past code changes | Future bug reports | Faster diagnosis |

---

## 5. Temporal Graph Condensation

### 5.1 The Problem

Temporal graphs accumulate history. A social network with 10 years of data has orders of magnitude more temporal edges than a single snapshot. Storing and processing all historical data is prohibitive.

### 5.2 Temporal Condensation Algorithm

```
TemporalCondensation(G_temporal, budget_T, budget_N):

  Input: Full temporal graph with T timestamps, N nodes
  Output: Condensed temporal graph with budget_T timestamps, budget_N nodes

  1. TEMPORAL COMPRESSION:
     // Select most informative timestamps
     timestamps_selected = SelectTimestamps(G_temporal, budget_T)
     // Criteria: maximum change in graph structure, attention entropy peaks

  2. NODE CONDENSATION (per selected timestamp):
     For each t in timestamps_selected:
       G_condensed(t) = GraphCondensation(G(t), budget_N)
       // Uses existing graph condensation (Doc 07)

  3. TEMPORAL EDGE SYNTHESIS:
     For consecutive selected timestamps t_i, t_{i+1}:
       // Synthesize temporal edges that capture the dynamics
       E_temporal(t_i, t_{i+1}) = SynthesizeDynamics(
         G_condensed(t_i), G_condensed(t_{i+1}))

  4. ATTENTION DISTILLATION:
     // Train condensed temporal graph to match original attention patterns
     L = sum_t ||Attention(G_condensed(t)) - Attention(G_original(t))||^2
```

**Compression ratios:**

| Temporal span | Original | Condensed | Ratio |
|--------------|----------|-----------|-------|
| 1 year, hourly | 8,760 snapshots | 52 (weekly) | 168x |
| 10 years, daily | 3,650 snapshots | 120 (monthly) | 30x |
| Real-time stream | Unbounded | Fixed window | - |

### 5.3 Integration with ruvector-temporal-tensor

The `ruvector-temporal-tensor` crate already implements delta compression and tiered storage, providing a natural foundation:

```rust
/// Temporal graph condensation
pub trait TemporalCondensation {
    /// Condense temporal graph history
    fn condense(
        &self,
        temporal_graph: &TemporalGraph,
        timestamp_budget: usize,
        node_budget: usize,
    ) -> Result<CondensedTemporalGraph, CondenseError>;

    /// Select most informative timestamps
    fn select_timestamps(
        &self,
        temporal_graph: &TemporalGraph,
        budget: usize,
    ) -> Vec<f64>;

    /// Get condensation quality metrics
    fn quality(&self) -> CondensationQuality;
}

pub struct CondensationQuality {
    pub attention_fidelity: f64,      // How well condensed attention matches original
    pub structural_fidelity: f64,     // Graph structure preservation
    pub temporal_fidelity: f64,       // Temporal dynamics preservation
    pub compression_ratio: f64,       // Size reduction factor
}
```

---

## 6. Temporal Attention Complexity

### 6.1 Complexity Hierarchy

| Method | Time per query | Space | Temporal range |
|--------|---------------|-------|---------------|
| Full temporal attention | O(T * n^2 * d) | O(T * n^2) | Full history |
| Windowed temporal | O(W * n^2 * d) | O(W * n^2) | Last W steps |
| Temporal condensation | O(T_c * n_c^2 * d) | O(T_c * n_c^2) | Full (approx) |
| Neural ODE (continuous) | O(steps * n * avg_deg * d) | O(n * d) | Continuous |
| Time-crystal | O(n * avg_deg * d) | O(n * d) | Periodic |
| Retrocausal | O(2 * T * n * avg_deg * d) | O(2 * n * d) | Full bidirectional |

### 6.2 Information-Theoretic Bounds

**Theorem (Temporal Attention Information Bound).** For a temporal graph with T time steps and entropy rate h (bits per time step), any attention mechanism that maintains epsilon-accurate temporal representations must store at least:

```
S >= T * h / epsilon bits
```

**Corollary.** For stationary temporal graphs (constant entropy rate), condensation can achieve constant storage by approximating with O(1/epsilon) representative timestamps.

**Corollary.** For non-stationary temporal graphs with time-varying entropy rate h(t), storage must grow as integral of h(t) dt.

---

## 7. Projections

### 7.1 By 2030

**Likely:**
- Continuous-time graph attention (Neural ODE) standard for temporal graph learning
- Temporal condensation reducing storage by 10-100x for historical graphs
- Causal graph transformers enforcing temporal consistency by default

**Possible:**
- Time-crystal attention for periodic pattern detection
- Retrocausal attention with information bottleneck for improved temporal prediction
- Real-time streaming graph transformers processing 10^6 events/second

**Speculative:**
- Temporal attention with provable optimal historical compression
- Self-tuning temporal resolution (automatic window size selection)

### 7.2 By 2033

**Likely:**
- Temporal graph transformers as standard database query operators
- Retrocausal attention routinely used in forecasting applications

**Possible:**
- Time-crystal dynamics for multi-phase graph reasoning
- Temporal graph transformers with formally verified causal consistency
- Cross-temporal attention: attention between different time scales simultaneously

### 7.3 By 2036+

**Possible:**
- Temporal graph transformers operating at quantum time scales (femtoseconds for molecular dynamics)
- Retrocausal attention with cosmological applications (analyzing spacetime event graphs)

**Speculative:**
- Time-crystal graph computers: computation via controlled oscillatory dynamics
- Temporal graph transformers that predict their own future states (self-fulfilling forecasts)

---

## 8. RuVector Implementation Roadmap

### Phase 1: Causal Foundation (2026-2027)
- Implement causal temporal attention mask in `ruvector-attention`
- Extend `ruvector-temporal-tensor` with temporal graph attention queries
- Neural ODE integration for continuous-time graph dynamics
- Benchmark on temporal graph benchmarks (JODIE, DyRep, TGN)

### Phase 2: Advanced Temporal (2027-2028)
- Time-crystal attention dynamics
- Retrocausal attention with information bottleneck
- Temporal condensation integrated with `ruvector-temporal-tensor` tiering
- Integration with causal attention (Doc 11) and streaming (Doc 21)

### Phase 3: Production Temporal (2028-2030)
- Real-time streaming temporal attention
- Verified causal consistency (`ruvector-verified`)
- Cross-temporal multi-scale attention
- Production deployment for financial, social, and IoT temporal graphs

---

## References

1. Xu et al., "Inductive Representation Learning on Temporal Graphs," ICLR 2020
2. Rossi et al., "Temporal Graph Networks for Deep Learning on Dynamic Graphs," ICML Workshop 2020
3. Chen et al., "Neural Ordinary Differential Equations," NeurIPS 2018
4. Wilczek, "Quantum Time Crystals," PRL 2012
5. Sacha & Zakrzewski, "Time Crystals: A Review," Reports on Progress in Physics 2018
6. Price, "Time's Arrow and Archimedes' Point," Oxford University Press 1996
7. RuVector `ruvector-temporal-tensor` documentation (internal)

---

**End of Document 28**

**Next:** [Doc 29 - Economic: Game-Theoretic Attention](29-economic-game-theoretic-attention.md)
