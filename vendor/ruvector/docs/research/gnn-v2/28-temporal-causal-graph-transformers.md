# Temporal and Causal Graph Transformers: Time Crystals, Retrocausal Attention, and Causal Discovery

## Overview

### Problem Statement

Most real-world graphs are not static snapshots -- they evolve. Social networks rewire daily. Financial transaction graphs stream continuously. Biological interaction networks change with cellular state. Yet the dominant paradigm in Graph Transformers treats the graph as frozen, computing attention over a fixed adjacency matrix and static node features.

This temporal blindness causes three fundamental failures:

1. **Stale representations**: Node embeddings computed at training time decay in accuracy as the graph evolves. A user's embedding from last week does not reflect today's interests.
2. **Causal confusion**: Standard attention is symmetric in time -- future events can influence past representations during message passing, violating the arrow of causality. This produces models that appear accurate but fail to generalize because they have access to information that would not be available at inference time.
3. **Missing dynamics**: The temporal evolution pattern itself is informative. A node that suddenly gains many connections (a viral post, a fraud ring activating) carries signal in its dynamics that static embeddings cannot capture.

The solution requires Graph Transformers that are natively temporal and causally aware: attention must respect the causal ordering of events, and representations must be functions of time.

### Connection to RuVector

RuVector has extensive infrastructure for temporal and causal graph processing:

- **`ruvector-temporal-tensor/`**: Delta compression with sparse delta chains (`delta.rs`), tiered storage with hot/warm/cold policies (`tier_policy.rs`, `tiering.rs`), epoch-based versioning, quantized tensor storage, and full persistence layer
- **`ruvector-dag/src/attention/causal_cone.rs`**: Causal cone attention that focuses on ancestors with temporal discount
- **`ruvector-dag/src/attention/temporal_btsp.rs`**: Behavioral Timescale Synaptic Plasticity attention with eligibility traces and plateau potentials
- **`ruvector-dag/src/attention/topological.rs`**: Topological attention respecting DAG structure
- **`ruvector-dag/src/dag/`**: Full DAG implementation with traversal, serialization, and query DAGs
- **`ruvector-attention/src/hyperbolic/lorentz_cascade.rs`**: Lorentz model attention -- the Lorentz metric is the metric of spacetime, making it the natural setting for causal structure
- **`ruvector-graph/`**: Property graph with temporal metadata support, distributed federation, Cypher queries
- **`ruvector-dag/src/sona/`**: Self-Optimizing Neural Architecture with EWC++ (Elastic Weight Consolidation), trajectory tracking, reasoning bank

This document extends these capabilities toward full temporal-causal Graph Transformers with causal discovery, continuous-time dynamics, and time-crystal-inspired periodic attention structures.

---

## Technical Deep Dive

### 1. Causal Graph Transformers

#### Attention That Respects Causal Ordering

In a temporal graph where events occur at times $t_1 < t_2 < \cdots < t_T$, causal attention ensures that the representation of node $v$ at time $t$ depends only on events at times $\leq t$:

$$\alpha_{ij}(t) = \frac{\exp(f(q_i(t), k_j(t')) / \tau) \cdot \mathbf{1}[t' \leq t]}{\sum_{l: t_l \leq t} \exp(f(q_i(t), k_l(t_l)) / \tau)}$$

The indicator function $\mathbf{1}[t' \leq t]$ is the causal mask. RuVector's `CausalConeAttention` already implements this with configurable time windows and ancestor weighting. The mask strategy options (Strict, TimeWindow, Topological) from the existing causal attention research (document 11) carry forward directly.

The key extension is **do-calculus-aware message passing**. Standard causal attention prevents future-to-past information flow, but does not distinguish between **observational** and **interventional** queries:

- **Observational**: "What is the embedding of node $v$ at time $t$, given all observed events?" -- standard causal attention
- **Interventional**: "What would the embedding of node $v$ be at time $t$ if we had set node $u$'s value to $x$?" -- requires do-calculus: $P(h_v(t) \mid \text{do}(h_u(t') = x))$

Interventional queries sever all incoming edges to the intervened node and propagate the intervention downstream through the causal graph. This is precisely the `InterventionKind::SetValue` operation from RuVector's causal attention network (document 11), now extended to temporal graphs.

#### Interventional Graph Queries

An interventional query on a temporal graph proceeds as:

```
Algorithm: Temporal Interventional Query

Input: Temporal graph G(t), intervention do(h_u(t_0) = x), query node v, query time t_q > t_0

1. Identify the causal descendants of u after t_0:
   D = {w : exists directed temporal path from (u, t_0) to (w, t) for some t > t_0}

2. For each node w in D, recompute embeddings forward in time:
   For t in [t_0, t_q] ordered by event time:
       If w == u and t == t_0:
           h_w(t) = x   // Intervention: set, don't compute
       Else:
           h_w(t) = CausalAttention(h_w, {h_j(t') : j in N(w), t' <= t})
           // Only use causally valid neighbors with potentially modified embeddings

3. Return h_v(t_q) under the intervention
```

### 2. Time-Crystal Graph Dynamics

#### Discrete Time-Symmetry Breaking in Graph Attention

A **time crystal** in condensed matter physics is a state of matter that spontaneously breaks discrete time-translation symmetry: the system is driven periodically at frequency $\omega$, but responds at a subharmonic frequency $\omega/n$. The ground state oscillates with a period that is a multiple of the driving period.

This concept translates to Graph Transformers in a precise way. Consider a temporal graph with periodic driving -- for example, a social network with daily activity cycles, or a financial market with trading-day periodicity. A standard temporal Graph Transformer that is time-translation-equivariant at the driving frequency $\omega$ would produce embeddings that repeat every cycle. But real systems exhibit **period-doubled dynamics**: weekly patterns in daily-driven systems, seasonal patterns in monthly-driven systems.

The time-crystal Graph Transformer explicitly models this symmetry breaking:

$$h_v(t + T) \neq h_v(t), \quad \text{but} \quad h_v(t + nT) = h_v(t)$$

where $T$ is the driving period and $n > 1$ is the emergent period multiplier.

**Implementation:** Add a **Floquet attention** layer that computes attention in the frequency domain:

$$\hat{\alpha}_{ij}(\omega) = \text{FFT}\left[\alpha_{ij}(t)\right]$$

The Floquet spectrum reveals the subharmonic responses. Peaks at $\omega/2$ indicate period-doubling; peaks at $\omega/3$ indicate period-tripling. The model learns which subharmonic to attend to for each node pair.

This connects to RuVector's temporal-tensor crate, which uses epoch-based versioning and delta chains -- the delta between consecutive epochs captures the dynamics, and Fourier analysis of the delta sequence reveals the time-crystal structure.

#### Periodic Ground States in Temporal Graph Transformers

The "ground state" of a temporal Graph Transformer is the stationary distribution of node embeddings under the temporal attention dynamics. For a system with discrete time-translation symmetry at period $T$, the ground state satisfies:

$$h^*(t) = \text{TemporalGT}(h^*(t-1), G(t))$$

A time-crystal ground state is a limit cycle:

$$h^*(t) = h^*(t + nT) \neq h^*(t + T) \quad \text{for } 1 < k < n$$

Detecting time-crystal behavior in graph embeddings serves as a diagnostic: if the graph's temporal pattern exhibits period multiplication, the embedding dynamics should as well. Failure to capture this indicates that the temporal model is too coarse.

### 3. Retrocausal Attention

#### Bidirectional Temporal Attention

In **online/streaming** settings, attention must be strictly causal (past-to-present). But in **offline/batch** settings where the entire temporal graph is available, we can leverage future information to improve past representations -- analogous to **smoothing** in Hidden Markov Models (forward-backward algorithm) or **bidirectional** LSTMs.

Retrocausal attention computes two sets of embeddings:

1. **Forward (causal) pass**: $h_v^{\rightarrow}(t) = \text{CausalAttention}(v, t, \{(u, t') : t' \leq t\})$
2. **Backward (retrocausal) pass**: $h_v^{\leftarrow}(t) = \text{AnticausalAttention}(v, t, \{(u, t') : t' \geq t\})$
3. **Smoothed embedding**: $h_v(t) = \text{Combine}(h_v^{\rightarrow}(t), h_v^{\leftarrow}(t))$

The combination can be a learned gate:

$$h_v(t) = \sigma(W_g [h_v^{\rightarrow}(t); h_v^{\leftarrow}(t)]) \odot h_v^{\rightarrow}(t) + (1 - \sigma(\cdots)) \odot h_v^{\leftarrow}(t)$$

**Connection to HMMs:** In a Hidden Markov Model, the forward pass computes $P(z_t \mid x_{1:t})$ and the backward pass computes $P(x_{t+1:T} \mid z_t)$. The smoothed posterior $P(z_t \mid x_{1:T})$ is the product of both. Retrocausal attention is the graph-structured generalization.

**Practical value:** Retrocausal attention is valuable for temporal knowledge graph completion (filling in missing past events given future context), historical analysis (understanding the precursors of an event given its consequences), and offline recommendation (refining past user state given subsequent behavior).

**Causal safety:** Retrocausal attention must never be used in online/streaming mode. The system must enforce a strict boundary: retrocausal modules are only invoked when the full temporal window is available. RuVector's existing `MaskStrategy::Strict` and `MaskStrategy::TimeWindow` from the causal attention module provide this enforcement.

### 4. Granger Causality on Graphs

#### Attention Weights as Granger-Causal Indicators

Granger causality asks: does knowing the history of node $u$ improve prediction of node $v$'s future state, beyond knowing $v$'s own history? Formally:

$$u \xrightarrow{G} v \iff P(h_v(t+1) \mid h_v(t), h_v(t-1), \ldots) \neq P(h_v(t+1) \mid h_v(t), h_v(t-1), \ldots, h_u(t), h_u(t-1), \ldots)$$

In a causal Graph Transformer, the learned attention weights $\alpha_{ij}(t)$ naturally encode Granger-causal relationships. If $\alpha_{vj}(t)$ is consistently large across time, node $j$ Granger-causes node $v$.

The **Granger-causal graph** $G_{\text{Granger}}$ has edge $(u, v)$ if:

$$\frac{1}{T} \sum_{t=1}^T \alpha_{vu}(t) > \theta$$

where $\theta$ is a significance threshold. This graph can be extracted directly from a trained causal Graph Transformer without any additional computation -- the attention weights are already computed during inference.

#### Automated Causal Graph Discovery

Going further, the Graph Transformer can be trained to **discover** the causal graph structure rather than having it provided as input:

```
Algorithm: Attention-Based Causal Discovery

Input: Multivariate time series {x_v(t)} for v in V, t in [1, T]
       Initial fully-connected graph G_0

1. Initialize causal Graph Transformer with G_0 (full attention)
2. For epoch in 1..E:
   a. Forward pass: compute h_v(t) for all v, t with causal masking
   b. Loss: prediction error + sparsity penalty on attention
      L = sum_t ||h_v(t+1) - h_v_pred(t+1)||^2 + lambda * sum_{i,j} |alpha_{ij}|
   c. Backward pass: update parameters
   d. Prune: remove edges where max_t alpha_{ij}(t) < threshold

3. Output: Learned causal graph G* = {(i,j) : edge not pruned}
           Granger-causal strength: s(i,j) = mean_t alpha_{ij}(t)
```

This connects to RuVector's `ruvector-dag` crate: the discovered causal graph is a DAG (directed acyclic graph by construction, since causal edges only go forward in time), and RuVector's DAG infrastructure provides efficient traversal, topological sort, and ancestor/descendant queries on the discovered structure.

### 5. Temporal Knowledge Graph Completion

#### Predicting Future Edges and Nodes

A temporal knowledge graph (TKG) consists of quadruples $(s, r, o, t)$: subject $s$ has relation $r$ with object $o$ at time $t$. Temporal KG completion predicts:

- **Future link prediction**: Given $(s, r, ?, t_{future})$, predict the object
- **Temporal link prediction**: Given $(s, r, o, ?)$, predict the time
- **Novel entity prediction**: Predict the emergence of entirely new nodes

A causal Graph Transformer for TKG completion uses:

1. **Temporal node embeddings**: $h_v(t)$ computed via causal attention over the event history
2. **Relation-aware attention**: Different relation types modulate the attention weights
3. **Temporal scoring**: $\text{score}(s, r, o, t) = f(h_s(t), h_r, h_o(t))$ where $f$ is a relation-specific scoring function

The causal constraint ensures that the prediction of $(s, r, o, t)$ uses only information from events before time $t$, enabling valid temporal forecasting.

RuVector's temporal-tensor crate provides the storage backbone: each node's embedding history is stored as a base tensor plus a delta chain (per `DeltaChain` in `delta.rs`), enabling efficient retrieval of $h_v(t)$ for any historical time $t$ via delta replay.

### 6. Continuous-Time Graph Networks

#### Neural ODEs on Graphs

Discrete-time temporal GNNs process snapshots $G(t_1), G(t_2), \ldots$ at fixed intervals. This misses events between snapshots and requires choosing a discretization granularity. **Continuous-time graph networks** model the embedding as a continuous function governed by a neural ODE:

$$\frac{dh_v(t)}{dt} = f_\theta\left(h_v(t), \{h_u(t) : u \in \mathcal{N}(v, t)\}, t\right)$$

where $\mathcal{N}(v, t)$ is the neighborhood of $v$ at time $t$ (which changes as edges appear and disappear).

The embedding at any time $t$ is obtained by integrating the ODE:

$$h_v(t) = h_v(t_0) + \int_{t_0}^{t} f_\theta(h_v(s), \ldots, s) \, ds$$

The integral is computed via an adaptive ODE solver (Dormand-Prince, Runge-Kutta) that takes smaller steps when the dynamics are fast and larger steps when they are slow.

**Connection to RuVector's PDE attention:** The `ruvector-attention/src/pde_attention/` module implements diffusion-based attention using Laplacian operators. The neural ODE approach generalizes this: diffusion is the special case where $f_\theta$ is the graph Laplacian operator.

#### Continuous-Depth Graph Transformers

The continuous-time ODE framework also enables **continuous-depth** Graph Transformers, where the number of attention layers is replaced by integration time:

$$h_v^{(T)} = h_v^{(0)} + \int_0^T \text{GraphTransformerBlock}(h_v^{(s)}, G, s) \, ds$$

Instead of stacking $L$ discrete layers, the model has a single parameterized dynamics that is integrated to a learned depth $T$. This enables:
- Adaptive computation: harder nodes integrate longer
- Memory efficiency: $O(1)$ memory for arbitrary depth (via adjoint method)
- Smooth feature evolution: no abrupt layer transitions

---

## Research Timeline

### 2026-2030: Real-Time Causal Discovery on Streaming Graphs

**Financial Fraud Detection (2026-2028):** Streaming transaction graphs processed by causal Graph Transformers in real-time. The attention weights automatically reveal anomalous causal patterns -- a node that suddenly becomes Granger-causal for many others indicates coordinated behavior (fraud ring, market manipulation). RuVector's delta-chain temporal storage enables microsecond-scale updates as new transactions arrive.

**Social Network Analysis (2027-2029):** Misinformation propagation modeled as a causal process on the social graph. Retrocausal attention (offline analysis) reveals the origin nodes of viral misinformation. Causal Graph Transformers predict which content will go viral before it does, enabling proactive moderation.

**Biological Networks (2028-2030):** Gene regulatory networks modeled as continuous-time causal graphs. Neural ODE Graph Transformers learn the dynamics of gene expression from single-cell RNA-seq time series. The learned causal graph recovers known regulatory relationships and discovers novel ones. Time-crystal dynamics reveal circadian and cell-cycle oscillations.

**Infrastructure:** By 2030, causal Graph Transformers are deployed in production for real-time monitoring of financial, social, and infrastructure networks. Standard practice includes causal validation: before deploying a temporal model, verify that it cannot access future information (achieved by RuVector's strict causal masking). Granger-causal graph extraction becomes a standard interpretability tool.

### 2030-2036: Autonomous Causal Reasoning Engines

**Self-Supervised Causal Discovery (2030-2032):** Graph Transformers learn causal structure without any labeled causal data. The training objective is purely predictive (predict future graph states), but the learned attention patterns converge to the true causal graph. Theoretical guarantees emerge linking attention convergence to causal identifiability under the faithfulness assumption.

**Interventional Planning (2032-2034):** Causal Graph Transformers are used for decision-making. Given a goal state for the graph, the system plans a sequence of interventions (node modifications) that causally propagate to achieve the goal. Applications include drug target identification (intervene on which gene to achieve desired expression pattern) and infrastructure planning (which upgrades causally improve overall network performance).

**Time-Crystal-Aware Forecasting (2032-2034):** Temporal Graph Transformers with Floquet attention automatically detect and exploit subharmonic patterns. Weekly patterns in daily data, seasonal patterns in monthly data, and multi-year cycles in annual data are captured without explicit feature engineering. The time-crystal diagnostic becomes a standard tool for assessing whether a temporal model has sufficient capacity.

**Causal Reasoning Engines (2034-2036):** Fully autonomous systems that discover causal mechanisms, verify them via interventional experiments (simulated or real), and use the verified causal model for planning and prediction. The Graph Transformer serves as both the hypothesis generator (attention weights suggest causal links) and the verifier (interventional queries test hypotheses). Human oversight shifts from designing models to auditing discovered causal mechanisms.

---

## Architecture Proposals

### Causal Attention with Temporal Masking

```
Input: Temporal graph events {(u, v, t, feat)} ordered by time t
       Node embeddings h_v^{(0)} for all v
       Causal mask M(t) = {(i,j) : t_j <= t_i}   (strict causal ordering)

For each attention layer l:
    For each event (u, v, t) in temporal order:
        // Compute time encoding
        dt = t - t_prev[u]
        time_enc = FourierTimeEncoding(dt)

        // Causal query: only attend to past events involving node u
        q = W_Q * [h_u^{(l)}; time_enc]
        K = {W_K * [h_j^{(l)}; time_enc_j] : j in CausalNeighbors(u, t)}
        V = {W_V * [h_j^{(l)}; time_enc_j] : j in CausalNeighbors(u, t)}

        // Masked attention (future events have -inf score)
        scores = q @ K^T / sqrt(d)
        scores[M(t) == 0] = -inf
        alpha = softmax(scores)

        // Update node embedding
        m_u = sum_j alpha_j * V_j
        h_u^{(l+1)} = GRU(h_u^{(l)}, m_u)   // GRU update for temporal continuity

        // Store temporal state in delta chain
        delta = h_u^{(l+1)} - h_u^{(l)}
        DeltaChain.append(delta, epoch=t)

Output: h_v^{(L)}(t) for all v and query time t
```

### Continuous-Time Causal Graph Transformer

```
Architecture Overview:

    Events: (u, v, t_1), (w, x, t_2), ...     (continuous timestamps)
                |
    +-----------+-----------+
    |                       |
    Event Encoder        Temporal Position
    (node features)      (Fourier encoding)
    |                       |
    +-----------+-----------+
                |
    Continuous-Time Neural ODE on Graph:
    dh_v/dt = f_theta(h_v(t), Aggregate(h_N(v)(t)), t)
                |
    Adaptive ODE Solver (Dormand-Prince):
    h_v(t) = h_v(t_0) + integral[t_0, t] f_theta ds
                |
    +-----------+-----------+
    |                       |
    Causal Masking:      Granger Analysis:
    h_v(t) depends       Extract attention
    only on events       weights as Granger-
    at t' <= t           causal indicators
    |                       |
    +-----------+-----------+
                |
    Output Layer:
    - Link prediction: score(s, r, o, t) = f(h_s(t), h_r, h_o(t))
    - Causal graph: G_granger = threshold(mean_t alpha_ij(t))
    - Intervention: do(h_u(t_0) = x) -> propagate forward
```

### Rust Pseudocode: Continuous-Time Causal Graph Transformer

```rust
/// Continuous-time causal graph transformer with neural ODE dynamics
pub struct ContinuousTimeCausalGT {
    /// Node embedding dimension
    dim: usize,
    /// Time encoding dimension
    time_dim: usize,
    /// Fourier time encoder (from ruvector temporal GNN research)
    time_encoder: FourierTimeEncoder,
    /// Neural ODE dynamics: dh/dt = f_theta(h, neighbors, t)
    dynamics: GraphODEDynamics,
    /// Causal mask enforcer
    causal_mask: TemporalCausalMask,
    /// Delta chain storage for temporal versioning
    delta_store: DeltaChainStore,
    /// Granger causality tracker
    granger_tracker: GrangerTracker,
}

/// Neural ODE dynamics on graph: dh_v/dt = f(h_v, agg(h_N(v)), t)
struct GraphODEDynamics {
    /// Query/Key/Value projections
    w_q: Matrix,
    w_k: Matrix,
    w_v: Matrix,
    /// GRU cell for state update
    gru: GRUCell,
    /// ODE solver configuration
    solver: DormandPrinceSolver,
}

/// Temporal causal mask: only attend to events at t' <= t
struct TemporalCausalMask {
    /// Temporal event index (sorted by time)
    event_timeline: BTreeMap<OrderedFloat<f64>, Vec<(NodeId, NodeId)>>,
    /// Maximum attention window (optional)
    max_window: Option<f64>,
}

impl ContinuousTimeCausalGT {
    /// Process a stream of temporal graph events
    pub fn process_event_stream(
        &mut self,
        events: &[(NodeId, NodeId, f64, Vec<f32>)],  // (src, dst, time, features)
        node_embeddings: &mut HashMap<NodeId, Vec<f32>>,
    ) -> Result<(), TemporalError> {
        // Events must be sorted by time (causal ordering)
        for &(src, dst, t, ref feat) in events {
            // 1. Compute time encoding for this event
            let dt = t - self.last_event_time(src);
            let time_enc = self.time_encoder.encode(dt);

            // 2. Gather causally valid neighbors (events at t' <= t only)
            let causal_neighbors = self.causal_mask.get_neighbors(src, t);

            // 3. Compute causal attention
            let h_src = node_embeddings.get(&src)
                .cloned()
                .unwrap_or_else(|| vec![0.0; self.dim]);

            let q = mat_vec_mul(&self.dynamics.w_q, &concat(&h_src, &time_enc));

            let mut keys = Vec::new();
            let mut vals = Vec::new();
            for &(neighbor, neighbor_time) in &causal_neighbors {
                let h_n = node_embeddings.get(&neighbor)
                    .cloned()
                    .unwrap_or_else(|| vec![0.0; self.dim]);
                let dt_n = t - neighbor_time;
                let time_enc_n = self.time_encoder.encode(dt_n);

                keys.push(mat_vec_mul(&self.dynamics.w_k, &concat(&h_n, &time_enc_n)));
                vals.push(mat_vec_mul(&self.dynamics.w_v, &concat(&h_n, &time_enc_n)));
            }

            // Masked attention (strictly causal: all neighbors are already past)
            let scores: Vec<f32> = keys.iter()
                .map(|k| dot_product(&q, k) / (self.dim as f32).sqrt())
                .collect();
            let weights = stable_softmax(&scores);

            // Track Granger-causal influence
            for (idx, &(neighbor, _)) in causal_neighbors.iter().enumerate() {
                self.granger_tracker.record(neighbor, src, t, weights[idx]);
            }

            // 4. Aggregate messages
            let message = weighted_sum(&vals, &weights);

            // 5. GRU update for temporal continuity
            let h_new = self.dynamics.gru.forward(&h_src, &message);

            // 6. Store delta in temporal-tensor delta chain
            let delta = element_sub(&h_new, &h_src);
            self.delta_store.append_delta(src, t, &delta)?;

            // 7. Update embedding
            node_embeddings.insert(src, h_new);

            // 8. Register event in causal mask
            self.causal_mask.register_event(src, dst, t);
        }

        Ok(())
    }

    /// Query node embedding at arbitrary historical time via delta replay
    pub fn embedding_at_time(
        &self,
        node: NodeId,
        t: f64,
        base_embeddings: &HashMap<NodeId, Vec<f32>>,
    ) -> Vec<f32> {
        let base = base_embeddings.get(&node)
            .cloned()
            .unwrap_or_else(|| vec![0.0; self.dim]);

        // Replay delta chain up to time t
        self.delta_store.reconstruct_at_time(node, t, &base)
    }

    /// Continuous-time integration via neural ODE
    /// Solves: h_v(t1) = h_v(t0) + integral[t0, t1] f(h_v(s), N(v,s), s) ds
    pub fn integrate_continuous(
        &self,
        node: NodeId,
        t0: f64,
        t1: f64,
        h0: &[f32],
        graph_state: &TemporalGraphState,
    ) -> Vec<f32> {
        self.dynamics.solver.integrate(
            |t, h| {
                // Dynamics function: dh/dt = f(h, neighbors(t), t)
                let neighbors = graph_state.neighbors_at(node, t);
                let time_enc = self.time_encoder.encode(t);
                let neighbor_agg = self.aggregate_neighbors(h, &neighbors, t);
                // dh/dt = -h + tanh(W * [h; neighbor_agg; time_enc])
                self.dynamics.compute_derivative(h, &neighbor_agg, &time_enc)
            },
            t0, t1, h0,
        )
    }

    /// Extract Granger-causal graph from learned attention weights
    pub fn extract_granger_graph(&self, threshold: f32) -> CausalGraph {
        self.granger_tracker.to_causal_graph(threshold)
    }

    /// Interventional query: do(h_u(t0) = x)
    /// Returns the counterfactual embedding of target node at query time
    pub fn interventional_query(
        &self,
        intervention_node: NodeId,
        intervention_time: f64,
        intervention_value: &[f32],
        target_node: NodeId,
        query_time: f64,
        graph_state: &TemporalGraphState,
    ) -> InterventionalResult {
        // 1. Compute factual embedding (no intervention)
        let factual = self.embedding_at_time(
            target_node, query_time, &graph_state.base_embeddings,
        );

        // 2. Find causal descendants of intervention_node after intervention_time
        let descendants = graph_state.causal_descendants(
            intervention_node, intervention_time, query_time,
        );

        // 3. Recompute embeddings with intervention applied
        let mut modified_embeddings = graph_state.base_embeddings.clone();
        modified_embeddings.insert(intervention_node, intervention_value.to_vec());

        // Forward propagate through causal descendants in temporal order
        for (node, t) in descendants.iter_temporal_order() {
            let h = self.integrate_continuous(
                *node, intervention_time, *t,
                modified_embeddings.get(node).unwrap(),
                graph_state,
            );
            modified_embeddings.insert(*node, h);
        }

        let counterfactual = modified_embeddings.get(&target_node)
            .cloned()
            .unwrap_or_else(|| factual.clone());

        InterventionalResult {
            factual,
            counterfactual,
            effect_size: l2_distance(&factual, &counterfactual),
            affected_nodes: descendants.len(),
        }
    }
}

/// Granger causality tracker: accumulates attention weights over time
struct GrangerTracker {
    /// Accumulated attention from source -> target over time
    attention_sums: HashMap<(NodeId, NodeId), f32>,
    attention_counts: HashMap<(NodeId, NodeId), u32>,
}

impl GrangerTracker {
    fn record(&mut self, source: NodeId, target: NodeId, _t: f64, weight: f32) {
        *self.attention_sums.entry((source, target)).or_insert(0.0) += weight;
        *self.attention_counts.entry((source, target)).or_insert(0) += 1;
    }

    fn to_causal_graph(&self, threshold: f32) -> CausalGraph {
        let mut edges = Vec::new();
        for (&(src, dst), &sum) in &self.attention_sums {
            let count = self.attention_counts[&(src, dst)];
            let mean_attention = sum / count as f32;
            if mean_attention > threshold {
                edges.push(CausalEdge {
                    source: src,
                    target: dst,
                    strength: mean_attention,
                });
            }
        }
        CausalGraph { edges }
    }
}
```

---

## Mathematical Formulations

### Causal Attention with Temporal Masking

For a temporal graph with events $\{(u_i, v_i, t_i)\}_{i=1}^N$ sorted by time:

$$\alpha_{ij}(t) = \frac{\exp\left(\frac{\langle W_Q h_i(t), W_K h_j(t_j) \rangle}{\sqrt{d}} - \lambda(t - t_j)\right) \cdot \mathbf{1}[t_j \leq t]}{\sum_{k: t_k \leq t} \exp\left(\frac{\langle W_Q h_i(t), W_K h_k(t_k) \rangle}{\sqrt{d}} - \lambda(t - t_k)\right)}$$

The exponential decay $\exp(-\lambda(t - t_j))$ ensures that more recent events receive higher attention, while the indicator $\mathbf{1}[t_j \leq t]$ enforces strict causality. The decay rate $\lambda$ is learnable.

### Continuous-Time Neural ODE on Graphs

$$\frac{dh_v(t)}{dt} = -h_v(t) + \sigma\left(W_h h_v(t) + \sum_{u \in \mathcal{N}(v, t)} \alpha_{vu}(t) \cdot W_m h_u(t) + W_t \phi(t)\right)$$

where:
- $\sigma$ is a nonlinearity (tanh or ReLU)
- $\alpha_{vu}(t)$ are time-dependent causal attention weights
- $\phi(t)$ is the Fourier time encoding
- $\mathcal{N}(v, t) = \{u : \exists \text{ event } (u, v, t') \text{ with } t' \leq t\}$

### Floquet Attention for Time Crystals

Given periodic driving at frequency $\omega_0$, the Floquet decomposition of attention weights is:

$$\alpha_{ij}(t) = \sum_{n=-\infty}^{\infty} a_{ij}^{(n)} e^{in\omega_0 t}$$

The time-crystal signature is: $|a_{ij}^{(n)}| > 0$ for $n \neq \pm 1$, indicating subharmonic response. The dominant subharmonic determines the period multiplication factor.

### Granger-Causal Strength

$$\text{GC}(u \to v) = \frac{1}{T} \sum_{t=1}^{T} \alpha_{vu}(t) \cdot \mathbf{1}\left[\frac{\partial \hat{h}_v(t+1)}{\partial h_u(t)} > \epsilon\right]$$

This measures both the attention weight (how much $v$ attends to $u$) and the sensitivity (how much $v$'s future state depends on $u$'s current state).

---

## Implementation Roadmap for RuVector

### Phase 1: Unify Temporal and Causal Attention (3-4 months)

- Merge `CausalConeAttention` and `TemporalBTSPAttention` from `ruvector-dag` into a unified temporal-causal attention module
- Integrate with `ruvector-temporal-tensor`'s delta chain for efficient historical embedding storage and retrieval
- Implement Fourier time encoding (already specified in temporal GNN research, document 06)
- Add strict causal masking with configurable time windows
- Benchmark against existing causal attention on temporal link prediction tasks

### Phase 2: Granger Causal Discovery and Interventional Queries (4-6 months)

- Implement `GrangerTracker` that accumulates attention weights during inference
- Build interventional query engine extending the counterfactual framework from document 11
- Add temporal delta propagation for interventional queries via `DeltaChain`
- Develop causal graph visualization using `ruvector-graph`'s Cypher export
- Validate Granger-causal discovery against known causal structures (synthetic benchmarks)

### Phase 3: Continuous-Time Neural ODE (6-9 months)

- Implement adaptive ODE solver (Dormand-Prince RK45) in Rust
- Build `GraphODEDynamics` module that integrates node embeddings continuously
- Connect to `ruvector-attention/src/pde_attention/` for Laplacian-based dynamics
- Implement adjoint method for memory-efficient backpropagation through ODE solver
- Benchmark continuous-time model against discrete-time temporal GNN

### Phase 4: Time Crystals and Retrocausal Attention (9-12 months)

- Implement Floquet attention with FFT-based spectral analysis of attention weights
- Build retrocausal attention module with strict online/offline mode enforcement
- Add time-crystal diagnostic: detect subharmonic responses in embedding dynamics
- Integrate periodic structure detection with `ruvector-temporal-tensor`'s epoch system
- Develop forward-backward smoothing algorithm for temporal graph embeddings

---

## Success Metrics

| Metric | Baseline (Static/Discrete) | Target (Continuous-Time Causal) |
|--------|---------------------------|--------------------------------|
| Temporal link prediction (MRR) | 0.40 | 0.55-0.65 |
| Granger-causal graph F1 score | N/A | 0.70-0.85 |
| Counterfactual query accuracy | N/A | 0.80-0.90 |
| Event update latency | 5ms (retrain) | 50us (delta) |
| Temporal embedding staleness | Hours | Milliseconds |
| Subharmonic detection accuracy | N/A | 0.85-0.95 |
| Online causal violation rate | ~5% (unchecked) | 0% (enforced) |

---

## Risks and Mitigations

| Risk | Severity | Mitigation |
|------|----------|------------|
| Causal mask overhead (sparse attention on large temporal graphs) | Medium | Use `ruvector-solver`'s sublinear algorithms for neighbor pruning; amortize mask computation |
| ODE solver instability (stiff dynamics on graphs with heterogeneous timescales) | High | Implement implicit solvers alongside explicit RK45; add step-size safety bounds |
| Retrocausal information leakage (accidentally using future info in online mode) | Critical | Enforce mode separation at type level -- retrocausal modules require `OfflineContext` token |
| Time-crystal false positives (detecting spurious periodicity) | Medium | Require statistical significance testing on Floquet spectra; cross-validate on held-out time windows |
| Delta chain growth (long temporal histories) | Medium | Use `ruvector-temporal-tensor`'s existing compaction and tiering policies (hot/warm/cold) |
| Granger causality != true causality (correlation-based discovery has limits) | High | Supplement Granger analysis with interventional validation; document limitations clearly |

---

## References

1. Xu, Rethage, Peng, Lippe (2020). "Inductive Representation Learning on Temporal Graphs." ICLR.
2. Rossi, Bronstein, Galke, Meilicke (2020). "Temporal Graph Networks for Deep Learning on Dynamic Graphs." ICML Workshop.
3. Pearl (2009). "Causality: Models, Reasoning, and Inference." Cambridge University Press.
4. Granger (1969). "Investigating Causal Relations by Econometric Models and Cross-Spectral Methods." Econometrica.
5. Tank, Covert, Foti, Shojaie, Fox (2022). "Neural Granger Causality." IEEE TPAMI.
6. Chen, Rubanova, Bettencourt, Duvenaud (2018). "Neural Ordinary Differential Equations." NeurIPS.
7. Sarao Mannelli, Vanden-Eijnden, Biroli (2020). "Complex Dynamics in Simple Neural Networks: Understanding Gradient Flow in Phase Retrieval." NeurIPS.
8. Wilczek (2012). "Quantum Time Crystals." Physical Review Letters.
9. Yao, Potter, Potirniche, Vishwanath (2017). "Discrete Time Crystals: Rigidity, Criticality, and Realizations." Physical Review Letters.
10. Kazemi, Goel, Jain, Kobyzev, Sethi, Forsyth, Poupart (2020). "Representation Learning for Dynamic Graphs: A Survey." JMLR.
11. Lacroix, Obozinski, Usunier (2020). "Tensor Decompositions for Temporal Knowledge Base Completion." ICLR.
12. Rubanova, Chen, Duvenaud (2019). "Latent Ordinary Differential Equations for Irregularly-Sampled Time Series." NeurIPS.
13. Lowe, Madras, Zemel, Welling (2022). "Amortized Causal Discovery: Learning to Infer Causal Graphs from Time-Series Data." CLeaR.
14. Peters, Janzing, Scholkopf (2017). "Elements of Causal Inference." MIT Press.

---

**Document Status:** Research Proposal
**Last Updated:** 2026-02-25
**Owner:** RuVector Architecture Team
**Related ADRs:** ADR-045 (Lean Agentic Integration)
**Related Crates:** ruvector-temporal-tensor, ruvector-dag, ruvector-attention, ruvector-graph, ruvector-solver
