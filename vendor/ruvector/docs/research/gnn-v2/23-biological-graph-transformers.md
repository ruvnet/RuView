# Biological Graph Transformers: Spiking, Hebbian, and Neuromorphic Architectures

## Overview

### The Biological Computation Thesis

Biological neural networks process graph-structured information with an efficiency that remains unmatched by artificial systems. The human brain -- a network of approximately 86 billion neurons connected by 100 trillion synapses -- performs graph-structured reasoning (social inference, spatial navigation, causal reasoning) consuming only 20 watts. A comparable artificial graph transformer processing a social network of similar density would require megawatts.

This disparity is not merely quantitative. Biological networks exploit three computational principles that artificial graph transformers have largely ignored:

1. **Event-driven sparsity.** Cortical neurons fire at 1-10 Hz on average, meaning 99%+ of compute is skipped at any given moment. Only "interesting" graph events trigger processing. Artificial graph transformers compute dense attention over all nodes at every step.

2. **Local learning rules.** Synaptic plasticity (STDP, Hebbian learning, BTSP) requires only information available at the synapse itself -- pre/post-synaptic activity and a neuromodulatory signal. No global backpropagation through the entire graph. This enables truly distributed, scalable learning on graphs.

3. **Temporal coding.** Information is encoded not just in firing rates but in precise spike timing, phase relationships, and oscillatory coupling. This gives biological networks a temporal dimension that artificial attention mechanisms -- which compute static weight matrices -- fundamentally lack.

This research document proposes a 10-year roadmap (2026-2036) for biological graph transformers that systematically incorporate these principles into the RuVector architecture, leveraging existing implementations in `ruvector-mincut-gated-transformer` (spike-driven attention, energy gates, Mamba SSM), `ruvector-nervous-system` (dendritic computation, BTSP, e-prop, Hopfield networks), `ruvector-gnn` (EWC continual learning, replay buffers), and `ruvector-attention` (18+ attention mechanisms).

### Problem Statement

Current graph transformers face five scaling barriers:

| Barrier | Root Cause | Biological Solution |
|---------|-----------|-------------------|
| O(N^2) attention | All-pairs computation | Event-driven sparse firing |
| Catastrophic forgetting | Global weight updates | Local synaptic consolidation (EWC/BTSP) |
| Energy consumption | Dense FP32 multiply-accumulate | Binary spike operations (87x reduction) |
| Static topology | Fixed graph at inference | Activity-dependent rewiring (STDP) |
| No temporal reasoning | Snapshot-based processing | Spike timing and oscillatory coding |

### Expected Impact

- **2028:** 100x energy reduction for graph attention via spiking architectures
- **2030:** Neuromorphic graph chips processing 1B edges at 1mW
- **2032:** Self-organizing graph transformers with no training phase
- **2036:** Bio-digital hybrid processors with living neural tissue for graph reasoning

---

## 1. Spiking Graph Transformers

### 1.1 Event-Driven Attention on Graphs

Standard graph attention (GAT) computes attention for every node pair at every layer. Spiking Graph Transformers (SGT) replace this with event-driven computation: a node only participates in attention when it "fires" -- when its membrane potential exceeds a threshold due to incoming graph signals.

**Architecture:**

```
Graph Input --> Spike Encoder --> Spiking Attention Layers --> Spike Decoder --> Output
                    |                    |
              Rate coding           Coincidence-based
              (value -> spike       attention weights
               frequency)           (no multiplication)
```

RuVector already implements the core of this in `crates/ruvector-mincut-gated-transformer/src/attention/spike_driven.rs`, which provides multiplication-free attention via spike coincidence detection:

```rust
// Existing RuVector implementation (spike_driven.rs)
// Attention via spike timing coincidence -- zero multiplications
pub fn attention(
    &self,
    q_spikes: &[SpikeTrain],
    k_spikes: &[SpikeTrain],
    v_spikes: &[SpikeTrain],
) -> Vec<i32> {
    // For each query position, count spike coincidences with keys
    // coincidence_score += q_polarity * k_polarity (when q_time == k_time)
    // This replaces softmax(QK^T/sqrt(d)) with temporal coincidence
}
```

The extension to graphs requires **topology-aware spike routing**: spikes propagate only along graph edges, not across all node pairs.

```rust
/// Proposed: Spiking Graph Attention with edge-constrained propagation
pub struct SpikingGraphAttention {
    /// Spike-driven attention (existing)
    spike_attn: SpikeDrivenAttention,
    /// Graph adjacency for spike routing
    adjacency: CompressedSparseRow,
    /// Per-edge synaptic delays (in timesteps)
    edge_delays: Vec<u8>,
    /// Per-node membrane potentials (LIF model)
    membrane: Vec<f32>,
    /// Refractory state per node
    refractory: Vec<u8>,
}

impl SpikingGraphAttention {
    /// Process one timestep of spiking graph attention
    pub fn step(&mut self, input_spikes: &[bool]) -> Vec<bool> {
        let mut output_spikes = vec![false; self.membrane.len()];

        for node in 0..self.membrane.len() {
            if self.refractory[node] > 0 {
                self.refractory[node] -= 1;
                continue;
            }

            // Accumulate spikes from graph neighbors only
            let mut incoming_current: f32 = 0.0;
            for &(neighbor, weight_idx) in self.adjacency.neighbors(node) {
                let delay = self.edge_delays[weight_idx] as usize;
                if self.was_spike_at(neighbor, delay) {
                    // Spike contribution weighted by learned edge attention
                    incoming_current += self.edge_attention_weight(node, neighbor);
                }
            }

            // LIF membrane dynamics
            self.membrane[node] = self.membrane[node] * 0.9 + incoming_current;

            if self.membrane[node] > SPIKE_THRESHOLD {
                output_spikes[node] = true;
                self.membrane[node] = 0.0; // reset
                self.refractory[node] = REFRACTORY_PERIOD;
            }
        }

        output_spikes
    }
}
```

### 1.2 Spike-Timing-Dependent Plasticity (STDP) for Edge Weight Updates

STDP provides a local, unsupervised learning rule for graph edge weights: if a presynaptic spike arrives just before a postsynaptic spike, strengthen the connection (causal). If after, weaken it (anti-causal).

**STDP Window Function:**

```
delta_w(dt) = A_+ * exp(-dt / tau_+)   if dt > 0  (pre before post: LTP)
            = -A_- * exp(dt / tau_-)    if dt < 0  (post before pre: LTD)
```

Applied to graphs, this means edge weights self-organize based on the temporal structure of spike propagation through the graph. Edges that consistently carry predictive information (pre-fires-before-post) are strengthened. Redundant or noisy edges are pruned.

```rust
/// STDP-based edge weight update for graph attention
pub struct StdpEdgeUpdater {
    /// Potentiation amplitude
    a_plus: f32,
    /// Depression amplitude
    a_minus: f32,
    /// Potentiation time constant (ms)
    tau_plus: f32,
    /// Depression time constant (ms)
    tau_minus: f32,
    /// Last spike time per node
    last_spike: Vec<f64>,
}

impl StdpEdgeUpdater {
    /// Update edge weight based on pre/post spike timing
    pub fn update_edge(&self, pre_node: usize, post_node: usize,
                       current_time: f64) -> f32 {
        let dt = self.last_spike[post_node] - self.last_spike[pre_node];

        if dt > 0.0 {
            // Pre fired before post -> potentiate (causal)
            self.a_plus * (-dt / self.tau_plus).exp()
        } else {
            // Post fired before pre -> depress (anti-causal)
            -self.a_minus * (dt / self.tau_minus).exp()
        }
    }
}
```

### 1.3 Temporal Coding in Graph Messages

Beyond rate coding (spike frequency encodes value), biological neurons use **temporal codes** where precise spike timing carries information. For graph transformers, this enables a richer message-passing scheme:

- **Phase coding:** Node embeddings encoded as phase offsets within oscillatory cycles. Two nodes with similar embeddings fire at similar phases, enabling interference-based similarity detection.
- **Burst coding:** The number of spikes in a burst encodes attention weight magnitude. Single spikes indicate weak attention; bursts of 3-5 spikes indicate strong attention.
- **Population coding:** Multiple neurons per graph node, each tuned to different features. The population spike pattern encodes the full node embedding.

The existing `SpikeScheduler` in `crates/ruvector-mincut-gated-transformer/src/spike.rs` already implements rate-based tier selection and novelty gating, which can be extended to temporal coding.

---

## 2. Hebbian Learning on Graphs

### 2.1 Local Learning Rules for Graph Attention

The core Hebbian principle -- "cells that fire together wire together" -- provides a radical alternative to backpropagation for training graph attention weights. In a Hebbian graph transformer:

1. **No global loss function.** Each edge learns independently based on co-activation of its endpoint nodes.
2. **No gradient computation.** Weight updates are purely local: `delta_w_ij = eta * x_i * x_j` (basic Hebb rule) or variants with normalization.
3. **No training/inference distinction.** The network continuously adapts to new graph inputs.

**Oja's Rule for Normalized Hebbian Graph Attention:**

```
delta_w_ij = eta * y_j * (x_i - w_ij * y_j)
```

Where `x_i` is the pre-synaptic (source node) activation and `y_j` is the post-synaptic (target node) activation. The subtraction term prevents unbounded weight growth.

```rust
/// Hebbian graph attention with no backpropagation
pub struct HebbianGraphAttention {
    /// Edge attention weights [num_edges]
    edge_weights: Vec<f32>,
    /// Learning rate
    eta: f32,
    /// Normalization: Oja, BCM, or raw Hebb
    rule: HebbianRule,
}

pub enum HebbianRule {
    /// Basic: dw = eta * x_pre * x_post
    RawHebb,
    /// Oja's rule: dw = eta * x_post * (x_pre - w * x_post)
    Oja,
    /// BCM: dw = eta * x_post * (x_post - theta) * x_pre
    BCM { theta: f32 },
}

impl HebbianGraphAttention {
    /// Single-pass Hebbian update -- no backprop needed
    pub fn update(&mut self, node_activations: &[f32], edges: &[(usize, usize)]) {
        for (edge_idx, &(src, dst)) in edges.iter().enumerate() {
            let x_pre = node_activations[src];
            let x_post = node_activations[dst];
            let w = self.edge_weights[edge_idx];

            let delta_w = match self.rule {
                HebbianRule::RawHebb => self.eta * x_pre * x_post,
                HebbianRule::Oja => {
                    self.eta * x_post * (x_pre - w * x_post)
                }
                HebbianRule::BCM { theta } => {
                    self.eta * x_post * (x_post - theta) * x_pre
                }
            };

            self.edge_weights[edge_idx] += delta_w;
        }
    }
}
```

### 2.2 Connection to RuVector Continual Learning

The existing EWC implementation in `crates/ruvector-gnn/src/ewc.rs` already captures the importance of weights via Fisher information. Hebbian learning naturally complements EWC:

- **Hebbian forward pass:** Learns new graph patterns via local co-activation
- **EWC regularization:** Prevents forgetting previously learned patterns by penalizing changes to important weights
- **Replay buffer:** `crates/ruvector-gnn/src/replay.rs` provides experience replay for rehearsing old graph patterns

This forms a biologically plausible continual learning loop that requires zero backpropagation through the graph.

---

## 3. Neuromorphic Graph Processing

### 3.1 Mapping Graph Transformers to Neuromorphic Hardware

Intel Loihi 2 and IBM TrueNorth implement spiking neural networks in silicon with 100-1000x energy efficiency over GPUs. Mapping graph transformers to these chips requires:

| Component | GPU Implementation | Neuromorphic Mapping |
|-----------|-------------------|---------------------|
| Node embeddings | FP32 vectors | Spike trains (temporal coding) |
| Attention weights | Softmax(QK^T) | Synaptic weights + STDP |
| Message passing | Matrix multiply | Spike propagation along edges |
| Aggregation | Sum/mean pooling | Population spike counting |
| Non-linearity | ReLU/GELU | Membrane threshold (LIF neuron) |

**Energy analysis for 1M-node graph:**

| Operation | GPU (A100) | Loihi 2 | Savings |
|-----------|-----------|---------|---------|
| Single attention layer | 2.1 J | 0.003 J | 700x |
| Full 6-layer GNN | 12.6 J | 0.02 J | 630x |
| Training step (one batch) | 38 J | 0.1 J | 380x |
| Continuous inference (1 hour) | 540 kJ | 0.72 kJ | 750x |

### 3.2 Loihi 2 Graph Transformer Architecture

```
Loihi 2 Neuromorphic Cores (128 per chip)
┌─────────────────────────────────────────────┐
│  Core 0-15:   Graph Partition A             │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐    │
│  │ Node 0  │──│ Node 1  │──│ Node 2  │    │
│  │ (LIF)   │  │ (LIF)   │  │ (LIF)   │    │
│  └────┬────┘  └────┬────┘  └────┬────┘    │
│       │ STDP       │ STDP       │ STDP     │
│  ┌────▼────┐  ┌────▼────┐  ┌────▼────┐    │
│  │ Attn 0  │  │ Attn 1  │  │ Attn 2  │    │
│  │ (Spike) │  │ (Spike) │  │ (Spike) │    │
│  └─────────┘  └─────────┘  └─────────┘    │
│                                             │
│  Core 16-31:  Graph Partition B             │
│  (same structure, inter-partition spikes    │
│   via on-chip mesh interconnect)            │
│                                             │
│  Core 120-127: Global Readout              │
│  (population decoding, output spikes)       │
└─────────────────────────────────────────────┘
```

The `SpikeScheduler` from `ruvector-mincut-gated-transformer/src/spike.rs` directly maps to Loihi's event-driven scheduling: the `SpikeScheduleDecision` (should_run, suggested_tier, use_sparse_mask) maps to Loihi's core-level power gating.

### 3.3 Projected Neuromorphic Graph Processor Milestones

| Year | Qubits/Neurons | Edges | Power | Application |
|------|---------------|-------|-------|-------------|
| 2026 | 1M neurons | 10M edges | 50mW | IoT sensor graphs |
| 2028 | 10M neurons | 100M edges | 100mW | Social network subgraphs |
| 2030 | 100M neurons | 1B edges | 1mW* | Full social network attention |
| 2032 | 1B neurons | 10B edges | 5mW | Protein interaction networks |
| 2036 | 10B neurons | 100B edges | 10mW | Whole-brain connectome |

*1mW achieved through aggressive event-driven sparsity (>99.9% neurons idle at any timestep)

---

## 4. Dendritic Computation as Graph Attention

### 4.1 Multi-Compartment Neuron Models as Graph Nodes

Biological neurons are not point units. A single pyramidal neuron has thousands of dendritic compartments, each performing nonlinear computation. RuVector's `ruvector-nervous-system` crate already implements this in `src/dendrite/compartment.rs`:

```rust
// Existing: Compartment with membrane and calcium dynamics
pub struct Compartment {
    membrane: f32,      // Membrane potential (0.0-1.0)
    calcium: f32,       // Calcium concentration (0.0-1.0)
    tau_membrane: f32,  // ~20ms fast dynamics
    tau_calcium: f32,   // ~100ms slow dynamics
}
```

In a dendritic graph transformer, each graph node is a multi-compartment neuron. Different input edges synapse onto different dendritic branches. This enables:

- **Nonlinear input gating:** A dendritic branch only activates when multiple correlated inputs arrive together (coincidence detection via `src/dendrite/coincidence.rs`)
- **Hierarchical attention:** Proximal dendrites compute local attention; apical dendrites integrate global context
- **Dendritic plateau potentials:** Enable one-shot learning of new graph patterns (via BTSP in `src/plasticity/btsp.rs`)

```rust
/// Dendritic Graph Node: each node is a multi-compartment neuron
pub struct DendriticGraphNode {
    /// Basal dendrites: receive input from graph neighbors
    basal_branches: Vec<DendriticBranch>,
    /// Apical dendrite: receives top-down context
    apical: DendriticBranch,
    /// Soma: integrates all branches, fires output spike
    soma: Compartment,
    /// BTSP for one-shot learning of new edges
    plasticity: BTSPLayer,
}

pub struct DendriticBranch {
    /// Compartments along this branch
    compartments: Vec<Compartment>,
    /// Synapses from specific graph neighbors
    synapses: Vec<(usize, f32)>, // (neighbor_id, weight)
    /// Nonlinear dendritic spike threshold
    plateau_threshold: f32,
}

impl DendriticGraphNode {
    /// Process graph inputs through dendritic tree
    pub fn process(&mut self, neighbor_activations: &[(usize, f32)]) -> f32 {
        // Route each neighbor's activation to appropriate branch
        for &(neighbor, activation) in neighbor_activations {
            let branch = self.route_to_branch(neighbor);
            branch.receive_input(activation);
        }

        // Each branch computes nonlinear dendritic integration
        let mut branch_outputs = Vec::new();
        for branch in &mut self.basal_branches {
            let output = branch.compute_plateau(); // nonlinear!
            branch_outputs.push(output);
        }

        // Soma integrates branch outputs
        let soma_input: f32 = branch_outputs.iter().sum();
        self.soma.step(soma_input, 1.0);
        self.soma.membrane()
    }
}
```

### 4.2 Dendritic Attention vs. Standard Attention

| Property | Standard Attention | Dendritic Attention |
|----------|-------------------|-------------------|
| Computation | Linear dot-product | Nonlinear dendritic spikes |
| Learning | Backpropagation | BTSP (one-shot, local) |
| Input routing | All inputs to same function | Different branches per input cluster |
| Memory | Stateless (per-step) | Stateful (calcium traces, ~100ms) |
| Energy | O(N^2 d) multiplies | O(branches * compartments) additions |
| Temporal | Instantaneous | History-dependent (membrane dynamics) |

---

## 5. Connectomics-Inspired Architectures

### 5.1 Small-World Graph Transformers

The brain exhibits small-world topology: high local clustering with short global path lengths. This is not an accident -- it optimizes the tradeoff between wiring cost (local connections are cheap) and communication efficiency (short paths enable fast information flow).

**Small-World Graph Transformer Design:**

- **Local attention:** Dense attention within topological neighborhoods (clusters)
- **Global shortcuts:** Sparse random long-range connections (rewiring probability p)
- **Watts-Strogatz topology:** Start with regular lattice, rewire edges with probability p

The existing `ruvector-attention` sparse attention module (`src/sparse/local_global.rs`) already supports this pattern with local and global attention heads.

### 5.2 Scale-Free Attention Networks

Biological networks (protein interactions, neural connectivity) follow power-law degree distributions: a few hub nodes have many connections while most nodes have few. Scale-free graph transformers:

- **Hub nodes get more attention heads:** High-degree nodes use multi-head attention; leaf nodes use single-head
- **Preferential attachment for edge learning:** New edges are more likely to form to high-degree nodes
- **Degree-aware compute allocation:** Matches the existing `SpikeScheduler` tier system (high-rate nodes get more compute)

### 5.3 Criticality-Tuned GNNs

The brain operates near a critical point between order and chaos, maximizing information processing capacity. A criticality-tuned graph transformer:

- **Branching ratio = 1:** On average, each spike causes exactly one downstream spike
- **Power-law avalanche distributions:** Activity cascades follow P(s) proportional to s^(-3/2)
- **Maximum dynamic range:** Responds to inputs spanning many orders of magnitude
- **Self-organized criticality:** The `EnergyGate` in `ruvector-mincut-gated-transformer/src/energy_gate.rs` already implements energy-based decision boundaries that can be tuned to maintain criticality

```rust
/// Criticality controller for graph transformer
pub struct CriticalityTuner {
    /// Target branching ratio (1.0 = critical)
    target_branching: f32,
    /// Moving average of actual branching ratio
    measured_branching: f32,
    /// Adaptation rate
    adaptation_rate: f32,
}

impl CriticalityTuner {
    /// Adjust global inhibition to maintain criticality
    pub fn adjust(&mut self, spike_counts: &[usize]) -> f32 {
        let total_input_spikes: usize = spike_counts.iter().sum();
        let total_output_spikes: usize = /* count from next timestep */;

        let branching = total_output_spikes as f32 / total_input_spikes.max(1) as f32;
        self.measured_branching = 0.99 * self.measured_branching + 0.01 * branching;

        // Return inhibition adjustment
        (self.measured_branching - self.target_branching) * self.adaptation_rate
    }
}
```

---

## 6. Architecture Proposals

### 6.1 Near-Term (2026-2028): Spiking Graph Attention Network (SGAT)

**Architecture:** Replace standard GAT layers with spike-driven attention using existing RuVector components.

| Component | Implementation | Energy Savings |
|-----------|---------------|---------------|
| Spike encoding | `SpikeDrivenAttention::encode_spikes()` | 0x (encoding cost) |
| Attention | `SpikeDrivenAttention::attention()` | 87x (no multiplies) |
| Scheduling | `SpikeScheduler::evaluate()` | 10x (skip idle nodes) |
| Energy gate | `EnergyGate::decide()` | 5x (skip stable regions) |
| EWC consolidation | `ElasticWeightConsolidation::penalty()` | 1x (regularization) |

**Estimated total energy reduction:** 50-100x over standard GAT.

**Latency analysis:**
- Per-node attention: 0.1us (spike coincidence) vs. 10us (softmax attention)
- Per-layer: O(|E|) spike propagations vs. O(|V|^2) attention computations
- For a 1M-node graph with 10M edges: ~10ms (spiking) vs. ~1000s (dense attention)

### 6.2 Medium-Term (2028-2032): Dendritic Graph Transformer (DGT)

**Architecture:** Multi-compartment dendritic nodes with BTSP learning.

```
Input Graph
    |
    v
┌───────────────────────────────────┐
│  Dendritic Graph Transformer      │
│                                   │
│  Layer 1: Dendritic Encoding     │
│  - Each node = multi-compartment  │
│  - Synapses routed to branches   │
│  - BTSP for one-shot learning    │
│                                   │
│  Layer 2: Hebbian Attention      │
│  - No backprop needed            │
│  - Oja's rule for attention      │
│  - EWC for continual learning    │
│                                   │
│  Layer 3: Criticality Readout    │
│  - Branching ratio = 1.0         │
│  - Power-law avalanches          │
│  - Maximum information capacity  │
└───────────────────────────────────┘
    |
    v
Output Embeddings
```

### 6.3 Long-Term (2032-2036): Bio-Digital Hybrid Graph Processor

The most speculative proposal: interface living neural organoids with silicon graph accelerators.

**Concept:**
- **Biological component:** Neural organoid (~1M neurons) cultured on a multi-electrode array (MEA). The organoid self-organizes into a graph with biological small-world topology.
- **Silicon component:** Neuromorphic chip (Loihi-class) handles graph storage, spike routing, and I/O.
- **Interface:** MEA reads/writes spikes bidirectionally. Graph queries become spike patterns injected into the organoid; responses are decoded from organoid output spikes.

**Advantages:**
- Biological neurons naturally implement STDP, dendritic computation, and criticality
- Extreme energy efficiency (~10nW per neuron vs. ~10uW for silicon LIF)
- Self-repair: biological networks compensate for cell death
- Continuous learning: no explicit training phase

**Challenges:**
- Reliability: biological variability, cell death, organoid longevity
- Latency: biological spike propagation ~1-10ms vs. ~1ns for silicon
- Reproducibility: each organoid develops differently
- Ethics: regulatory and ethical frameworks for "computing with living tissue"

---

## 7. Connection to RuVector Crates

### 7.1 Direct Integration Points

| RuVector Crate | Component | Biological Extension |
|---------------|-----------|---------------------|
| `ruvector-mincut-gated-transformer` | `spike.rs` | STDP edge learning, temporal coding |
| `ruvector-mincut-gated-transformer` | `spike_driven.rs` | Graph-constrained spike propagation |
| `ruvector-mincut-gated-transformer` | `energy_gate.rs` | Criticality tuning, energy landscape navigation |
| `ruvector-mincut-gated-transformer` | `mamba.rs` | SSM as continuous-time membrane dynamics |
| `ruvector-nervous-system` | `dendrite/` | Multi-compartment graph nodes |
| `ruvector-nervous-system` | `plasticity/btsp.rs` | One-shot graph pattern learning |
| `ruvector-nervous-system` | `plasticity/eprop.rs` | Online learning without BPTT |
| `ruvector-nervous-system` | `compete/kwta.rs` | Sparse activation (k-winners-take-all) |
| `ruvector-nervous-system` | `hopfield/` | Associative memory for graph patterns |
| `ruvector-gnn` | `ewc.rs` | Fisher-information weight consolidation |
| `ruvector-gnn` | `replay.rs` | Experience replay for continual graph learning |
| `ruvector-attention` | `sparse/` | Local-global attention patterns |
| `ruvector-attention` | `topology/` | Topology-aware attention coherence |

### 7.2 Proposed New Modules

```
crates/ruvector-mincut-gated-transformer/src/
    stdp.rs                    -- STDP edge weight updates
    temporal_coding.rs         -- Phase/burst/population coding
    criticality.rs             -- Self-organized criticality tuner

crates/ruvector-nervous-system/src/
    graph_neuron.rs            -- Multi-compartment graph node
    spiking_graph_attn.rs      -- Graph-aware spiking attention

crates/ruvector-gnn/src/
    hebbian.rs                 -- Hebbian learning rules (Oja, BCM)
    neuromorphic_backend.rs    -- Loihi/TrueNorth compilation target
```

---

## 8. Research Timeline

### Phase 1: Spike-Driven Graph Attention (2026-2027)
- Extend `SpikeDrivenAttention` to graph-constrained propagation
- Implement STDP edge learning
- Benchmark: energy savings on OGB datasets
- Target: 50x energy reduction, matched accuracy

### Phase 2: Dendritic + Hebbian Graphs (2027-2029)
- Multi-compartment graph nodes using `dendrite/` module
- Hebbian attention training (no backprop)
- BTSP for one-shot graph pattern learning
- Target: Zero-backprop graph transformer with competitive accuracy

### Phase 3: Neuromorphic Deployment (2029-2031)
- Compile graph transformer to Loihi 2 instruction set
- Benchmark on neuromorphic hardware
- Target: 1B edges at 1mW sustained power

### Phase 4: Connectomics-Inspired Scaling (2031-2033)
- Small-world and scale-free graph transformer topologies
- Self-organized criticality for maximum information capacity
- Target: Self-organizing graph transformers (no architecture search)

### Phase 5: Bio-Digital Hybrids (2033-2036)
- Neural organoid interface prototypes
- Hybrid silicon-biological graph processing
- Target: Proof-of-concept bio-digital graph reasoning

---

## 9. Open Questions

1. **Spike coding efficiency.** How many timesteps of spiking simulation are needed to match one forward pass of a standard graph transformer? Current estimates: 8-32 timesteps (from `SpikeDrivenConfig::temporal_coding_steps`), but this may need to be larger for complex graphs.

2. **Hebbian graph attention convergence.** Does Oja's rule on graph attention weights converge to the same solution as backpropagation-trained GAT? Preliminary analysis suggests it converges to the principal component of the attention pattern, which may differ from the optimal supervised solution.

3. **Criticality vs. performance.** Operating at criticality maximizes information capacity but may not optimize for specific downstream tasks. How to balance criticality (generality) with task-specific tuning?

4. **Neuromorphic graph partitioning.** How to partition a large graph across neuromorphic cores while minimizing inter-core spike communication? This is a graph partitioning problem -- potentially solvable by RuVector's own min-cut algorithms.

5. **Bio-digital latency gap.** Biological neurons operate on millisecond timescales; silicon on nanosecond timescales. How to bridge this 10^6 gap in a hybrid system without one component bottlenecking the other?

---

## References

- Yao, M., et al. (2023). Spike-driven Transformer. NeurIPS 2023.
- Yao, M., et al. (2024). Spike-driven Transformer V2. ICLR 2024.
- Bellec, G., et al. (2020). A solution to the learning dilemma for recurrent networks of spiking neurons. Nature Communications.
- Bittner, K., et al. (2017). Behavioral time scale synaptic plasticity underlies CA1 place fields. Science.
- Bi, G. & Poo, M. (1998). Synaptic modifications in cultured hippocampal neurons. Journal of Neuroscience.
- Davies, M., et al. (2018). Loihi: A neuromorphic manycore processor. IEEE Micro.
- Watts, D. & Strogatz, S. (1998). Collective dynamics of 'small-world' networks. Nature.
- Beggs, J. & Plenz, D. (2003). Neuronal avalanches in neocortical circuits. Journal of Neuroscience.
- Gladstone, R., et al. (2025). Energy-Based Transformers.
- Oja, E. (1982). Simplified neuron model as a principal component analyzer. Journal of Mathematical Biology.

---

**Document Status:** Research Proposal
**Target Integration:** RuVector GNN v2 Phase 4-5
**Estimated Effort:** 18-24 months (phased over 10 years)
**Risk Level:** High (Phase 1-3), Very High (Phase 4-5)
**Dependencies:** ruvector-mincut-gated-transformer, ruvector-nervous-system, ruvector-gnn, ruvector-attention
