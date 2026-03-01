# Axis 3: Biological -- Spiking Graph Transformers

**Document:** 23 of 30
**Series:** Graph Transformers: 2026-2036 and Beyond
**Last Updated:** 2026-02-25
**Status:** Research Prospectus

---

## 1. Problem Statement

The brain processes graph-structured information (connectomes, neural circuits, cortical columns) using mechanisms fundamentally different from backpropagation-trained transformers: discrete spikes, local Hebbian learning rules, dendritic computation, and spike-timing-dependent plasticity. These mechanisms are energy-efficient (the brain uses ~20 watts for ~86 billion neurons) and naturally parallel.

The biological axis asks: can we build graph transformers that compute like brains?

### 1.1 The Efficiency Gap

| System | Nodes | Power | Power/Node | Latency |
|--------|-------|-------|------------|---------|
| Human brain | 86 x 10^9 | 20 W | 0.23 nW | ~100ms |
| GPU graph transformer | 10^6 | 300 W | 300 uW | ~1ms |
| Neuromorphic (Loihi 2) | 10^6 | 1 W | 1 uW | ~10ms |
| Spiking graph transformer (proposed) | 10^8 | 10 W | 0.1 uW | ~50ms |

The brain achieves 6 orders of magnitude better power efficiency per node. Spiking graph transformers aim to close this gap by 3-4 orders of magnitude.

### 1.2 RuVector Baseline

- **`ruvector-mincut-gated-transformer`**: Spiking neurons (`spike.rs`), energy gates (`energy_gate.rs`)
- **`ruvector-nervous-system`**: Hopfield nets (`hopfield/`), HDC (`hdc/`), dendrite compute (`dendrite/`), plasticity (`plasticity/`), competitive learning (`compete/`), routing (`routing/`)
- **`ruvector-attention`**: Neighborhood attention (`graph/`), sparse attention (`sparse/`)

---

## 2. Spiking Graph Attention

### 2.1 From Softmax to Spikes

Standard graph attention:
```
alpha_{uv} = softmax_v(Q_u . K_v^T / sqrt(d))
z_u = sum_v alpha_{uv} * V_v
```

Spiking graph attention:
```
// Accumulate input current from neighbors
I_u(t) = sum_{v in N(u)} w_{uv} * S_v(t) * V_v

// Leaky integrate-and-fire (LIF) dynamics
tau * dU_u/dt = -U_u(t) + I_u(t)

// Spike when membrane potential exceeds threshold
if U_u(t) >= theta_u:
    S_u(t) = 1     // Emit spike
    U_u(t) = U_reset  // Reset potential
else:
    S_u(t) = 0
```

**Key differences from standard attention:**
1. **Temporal coding**: Information is in spike timing, not continuous values
2. **Winner-take-all**: High-attention nodes spike first (rate and temporal coding)
3. **Energy proportional to activity**: Silent nodes consume zero energy
4. **Local computation**: Each node only sees spikes from its graph neighbors

### 2.2 Spike-Based Attention Weights

We propose three mechanisms for spike-based attention:

**Mechanism 1: Rate-Coded Attention**
```
alpha_{uv} = spike_rate(v, window_T) / sum_w spike_rate(w, window_T)
```
Attention weight proportional to how often a neighbor spikes. Reduces to standard attention in the continuous limit.

**Mechanism 2: Temporal-Coded Attention**
```
alpha_{uv} = exp(-|t_spike(u) - t_spike(v)| / tau) / Z
```
Nodes that spike close in time attend to each other. Implements temporal coincidence detection.

**Mechanism 3: Phase-Coded Attention**
```
alpha_{uv} = cos(phi_u(t) - phi_v(t)) / Z
```
Attention based on oscillatory phase coherence. Nodes oscillating in phase form attention groups. Related to gamma oscillations in the brain.

### 2.3 Spiking Graph Attention Network (SGAT)

```
Architecture:

Input Layer: Encode features as spike trains
  |
Spiking Attention Layer 1:
  - Each node: LIF neuron
  - Attention: via spike timing (Mechanism 2)
  - Aggregation: spike-weighted sum
  |
Spiking Attention Layer 2:
  - Lateral inhibition for competition
  - Winner-take-all within neighborhoods
  |
...
  |
Readout Layer: Decode spike trains to continuous values
  - Population coding: average over neuron populations
  - Rate decoding: spike count in window
```

**RuVector integration:**

```rust
/// Spiking graph attention layer
pub struct SpikingGraphAttention {
    /// Neuron parameters per node
    neurons: Vec<LIFNeuron>,
    /// Synaptic weights (graph edges)
    synapses: SparseMatrix<SynapticWeight>,
    /// Attention mechanism
    attention_mode: SpikeAttentionMode,
    /// Time step
    dt: f64,
    /// Current simulation time
    t: f64,
}

pub struct LIFNeuron {
    /// Membrane potential
    pub membrane_potential: f32,
    /// Resting potential
    pub v_rest: f32,
    /// Threshold
    pub threshold: f32,
    /// Reset potential
    pub v_reset: f32,
    /// Membrane time constant
    pub tau: f32,
    /// Refractory period counter
    pub refractory: f32,
    /// Last spike time
    pub last_spike: f64,
    /// Spike train history
    pub spike_train: VecDeque<f64>,
}

pub struct SynapticWeight {
    /// Base weight
    pub weight: f32,
    /// Plasticity trace (for STDP)
    pub trace: f32,
    /// Delay (in dt units)
    pub delay: u16,
}

pub enum SpikeAttentionMode {
    /// Attention proportional to spike rate
    RateCoded { window: f64 },
    /// Attention from spike timing coincidence
    TemporalCoded { tau: f64 },
    /// Attention from phase coherence
    PhaseCoded { frequency: f64 },
}

impl SpikingGraphAttention {
    /// Simulate one time step
    pub fn step(
        &mut self,
        graph: &PropertyGraph,
        input_currents: &[f32],
    ) -> Vec<bool> {  // Returns which nodes spiked
        let mut spikes = vec![false; self.neurons.len()];

        for (v, neuron) in self.neurons.iter_mut().enumerate() {
            // Skip if in refractory period
            if neuron.refractory > 0.0 {
                neuron.refractory -= self.dt as f32;
                continue;
            }

            // Accumulate input from spiking neighbors
            let mut input = input_currents[v];
            for (u, synapse) in self.incoming_synapses(v, graph) {
                if self.neurons[u].spiked_at(self.t - synapse.delay as f64 * self.dt) {
                    input += synapse.weight;
                }
            }

            // LIF dynamics
            neuron.membrane_potential +=
                self.dt as f32 * (-neuron.membrane_potential + neuron.v_rest + input)
                / neuron.tau;

            // Spike check
            if neuron.membrane_potential >= neuron.threshold {
                spikes[v] = true;
                neuron.membrane_potential = neuron.v_reset;
                neuron.refractory = 2.0; // 2ms refractory
                neuron.last_spike = self.t;
                neuron.spike_train.push_back(self.t);
            }
        }

        self.t += self.dt;
        spikes
    }
}
```

---

## 3. Hebbian Learning on Graphs

### 3.1 Graph Hebbian Rules

Classical Hebb's rule: "Neurons that fire together, wire together."

**Graph Hebbian attention update:**
```
Delta_w_{uv} = eta * (
    pre_trace(u) * post_trace(v)  // Hebbian term
    - lambda * w_{uv}              // Weight decay
)
```

where pre_trace and post_trace are exponentially filtered spike trains:
```
pre_trace(u, t) = sum_{t_spike < t} exp(-(t - t_spike) / tau_pre)
post_trace(v, t) = sum_{t_spike < t} exp(-(t - t_spike) / tau_post)
```

### 3.2 Spike-Timing-Dependent Plasticity (STDP) on Graphs

STDP adjusts edge weights based on the relative timing of pre- and post-synaptic spikes:

```
Delta_w_{uv} =
  A_+ * exp(-(t_post - t_pre) / tau_+)  if t_post > t_pre  (LTP)
  -A_- * exp(-(t_pre - t_post) / tau_-)  if t_pre > t_post  (LTD)
```

- LTP (Long-Term Potentiation): Pre before post -> strengthen connection
- LTD (Long-Term Depression): Post before pre -> weaken connection

**Graph STDP attention:**
```
For each edge (u, v) in E:
  For each pair of spikes (t_u, t_v):
    dt = t_v - t_u
    if dt > 0:  // u spiked before v
      w_{uv} += A_+ * exp(-dt / tau_+)   // Strengthen u->v
    else:
      w_{uv} -= A_- * exp(dt / tau_-)     // Weaken u->v
```

**Interpretation as attention learning:** STDP automatically learns attention weights that encode causal influence in the graph. If node u's activity reliably precedes node v's, the u->v attention weight increases.

### 3.3 Homeostatic Plasticity for Attention Stability

Pure STDP can lead to runaway excitation or silencing. Homeostatic mechanisms maintain stable attention distributions:

**Intrinsic plasticity (threshold adaptation):**
```
theta_v += eta_theta * (spike_rate(v) - target_rate)
```
Nodes that spike too often raise their threshold; rarely-spiking nodes lower it.

**Synaptic scaling:**
```
w_{uv} *= (target_rate / actual_rate(v))^{1/3}
```
All incoming weights scale to maintain target activity.

**BCM rule (Bienenstock-Cooper-Munro):**
```
Delta_w_{uv} = eta * post_activity * (post_activity - theta_BCM) * pre_activity
```
The sliding threshold theta_BCM prevents both runaway excitation and complete depression.

---

## 4. Dendritic Graph Computation

### 4.1 Beyond Flat Embeddings

Standard GNNs treat each node as a single computational unit with a flat embedding vector. Real neurons have elaborate dendritic trees with nonlinear computation in individual branches.

**Dendritic graph node:**
```
Each node v has a dendritic tree D_v with:
- B branches, each receiving input from a subset of neighbors
- Nonlinear dendritic activation per branch
- Somatic integration combining branch outputs

Node embedding:
  h_v = soma(
    branch_1(inputs from neighbors N_1(v)),
    branch_2(inputs from neighbors N_2(v)),
    ...
    branch_B(inputs from neighbors N_B(v))
  )
```

**Advantage:** A single dendritic node can compute functions (like XOR) that require multiple layers of flat neurons. This makes dendritic graph transformers deeper in computational power despite being shallower in layer count.

### 4.2 Dendritic Attention Mechanism

```
For node v with B dendritic branches:

1. PARTITION neighbors into branches:
   N_1(v), N_2(v), ..., N_B(v) = partition(N(v))
   (partition can be learned or based on graph structure)

2. BRANCH computation:
   For each branch b:
     z_b = sigma(W_b * aggregate(h_u for u in N_b(v)))
     // Nonlinear dendritic activation per branch

3. BRANCH attention:
   alpha_b = softmax(W_attn * z_b)
   // Attention across branches (which branch is most relevant)

4. SOMATIC integration:
   h_v = soma(sum_b alpha_b * z_b)
   // Final node embedding
```

**Complexity:** O(|N(v)| * d + B * d) per node. The B-fold increase in parameters is compensated by the ability to use fewer layers.

**RuVector integration:** The `ruvector-nervous-system/src/dendrite/` module already implements dendritic computation. Extending it to graph attention requires:
1. Neighbor-to-branch assignment (can use graph clustering from `ruvector-mincut`)
2. Branch-level attention computation
3. Integration with the main attention trait system in `ruvector-attention`

---

## 5. Neuromorphic Hardware Deployment

### 5.1 Target Platforms (2026-2030)

| Platform | Neurons | Synapses | Power | Architecture |
|----------|---------|----------|-------|-------------|
| Intel Loihi 2 | 1M per chip | 120M | 1W | Digital LIF, programmable |
| IBM NorthPole | 256M ops/cycle | - | 12W | Digital inference |
| SynSense Speck | 320K | 65M | 0.7mW | Dynamic vision |
| BrainChip Akida | 1.2M | 10B | 1W | Event-driven |
| SpiNNaker 2 | 10M per board | 10B | 10W | ARM cores + digital neurons |

### 5.2 Graph Transformer to Neuromorphic Compilation

```
Compilation pipeline:

Source: SpikingGraphAttention (RuVector Rust)
  |
  v
Step 1: Graph Partitioning
  - Partition graph to fit chip neuron limits
  - Use ruvector-mincut for optimal partitioning
  - Map partitions to neuromorphic cores
  |
  v
Step 2: Neuron Mapping
  - Map each graph node to a hardware neuron cluster
  - Map attention weights to synaptic connections
  - Configure LIF parameters (threshold, tau, etc.)
  |
  v
Step 3: Synapse Routing
  - Map graph edges to hardware synaptic routes
  - Handle multi-hop routing for non-local edges
  - Optimize for communication bandwidth
  |
  v
Step 4: STDP Configuration
  - Program learning rules into on-chip plasticity engines
  - Set STDP time constants and learning rates
  |
  v
Target: Neuromorphic binary (Loihi SLIF, SpiNNaker PyNN, etc.)
```

**RuVector compilation target:**

```rust
/// Trait for neuromorphic compilation targets
pub trait NeuromorphicTarget {
    type Config;
    type Binary;

    /// Maximum neurons per core
    fn neurons_per_core(&self) -> usize;

    /// Maximum synapses per neuron
    fn synapses_per_neuron(&self) -> usize;

    /// Supported neuron models
    fn supported_models(&self) -> Vec<NeuronModel>;

    /// Compile spiking graph attention to target
    fn compile(
        &self,
        sgat: &SpikingGraphAttention,
        graph: &PropertyGraph,
        config: &Self::Config,
    ) -> Result<Self::Binary, CompileError>;

    /// Estimated power consumption
    fn estimate_power(
        &self,
        binary: &Self::Binary,
        spike_rate: f64,
    ) -> PowerEstimate;
}

pub struct PowerEstimate {
    pub static_power_mw: f64,
    pub dynamic_power_mw: f64,
    pub total_power_mw: f64,
    pub energy_per_spike_nj: f64,
    pub energy_per_inference_uj: f64,
}
```

---

## 6. Oscillatory Graph Attention

### 6.1 Gamma Oscillations and Binding

The brain uses oscillatory synchronization (gamma: 30-100 Hz) to bind features. Neurons representing the same object oscillate in phase; different objects oscillate out of phase.

**Oscillatory graph attention:**
```
Each node v has phase phi_v(t) and frequency omega_v:

dphi_v/dt = omega_v + sum_{u in N(v)} K_{uv} * sin(phi_u - phi_v)
```

This is a Kuramoto model on the graph. Coupled nodes synchronize; uncoupled nodes desynchronize.

**Attention from synchronization:**
```
alpha_{uv}(t) = (1 + cos(phi_u(t) - phi_v(t))) / 2
```

Synchronized nodes have attention weight 1; anti-phase nodes have weight 0.

### 6.2 Multi-Frequency Attention

Different attention heads operate at different frequencies:

```
Head h at frequency omega_h:
  phi_v^h(t) oscillates at omega_h + perturbations from neighbors
  alpha_{uv}^h(t) = (1 + cos(phi_u^h - phi_v^h)) / 2

Cross-frequency coupling:
  phi_v^{slow}(t) modulates amplitude of phi_v^{fast}(t)
  // Implements hierarchical binding:
  // slow oscillation groups communities
  // fast oscillation groups nodes within communities
```

**RuVector connection:** This connects to `ruvector-coherence`'s spectral coherence tracking. The oscillatory phases define a coherence metric on the graph.

---

## 7. Projections

### 7.1 By 2030

**Likely:**
- Spiking graph transformers achieving 100x energy efficiency over GPU versions on small graphs
- STDP-trained graph attention competitive with backprop on benchmark tasks
- Neuromorphic deployment of graph transformers on Loihi 3 / SpiNNaker 2+

**Possible:**
- Dendritic graph attention reducing required depth by 3-5x
- Oscillatory attention for temporal graph problems (event detection, anomaly detection)
- Hebbian graph learning for continual graph learning (no catastrophic forgetting)

**Speculative:**
- Brain-scale (10^10 neuron) spiking graph transformers on neuromorphic clusters
- Online unsupervised STDP learning matching supervised performance

### 7.2 By 2033

**Likely:**
- Neuromorphic graph transformer chips (custom silicon for spiking graph attention)
- Dendritic computation standard in graph attention toolkits
- 1000x energy efficiency over 2026 GPU baselines

**Possible:**
- Self-organizing spiking graph transformers that grow new neurons/connections
- Cross-frequency attention for multi-scale graph reasoning
- Neuromorphic edge AI: graph transformers in IoT sensors

### 7.3 By 2036+

**Possible:**
- Neuromorphic graph transformers matching brain efficiency (~1 nW/node)
- Spiking graph transformers with emergent cognitive-like capabilities
- Biological-digital hybrid systems (graph transformers interfacing with neural tissue)

**Speculative:**
- True neuromorphic graph intelligence: self-learning, self-organizing, self-repairing
- Graph transformers that implement cortical column dynamics

---

## 8. RuVector Implementation Roadmap

### Phase 1: Spiking Foundation (2026-2027)
- Extend `ruvector-mincut-gated-transformer/src/spike.rs` with full LIF graph dynamics
- Implement STDP learning rules in `ruvector-nervous-system/src/plasticity/`
- Add spike-based attention to `ruvector-attention` trait system
- Benchmark on neuromorphic graph datasets

### Phase 2: Dendritic & Oscillatory (2027-2028)
- Extend `ruvector-nervous-system/src/dendrite/` for graph attention
- Implement Kuramoto oscillatory attention
- Add dendritic branching strategies using `ruvector-mincut` partitioning
- Integration with `ruvector-coherence` for coherence tracking

### Phase 3: Neuromorphic Deployment (2028-2030)
- Neuromorphic compilation pipeline (Loihi, SpiNNaker targets)
- Power-optimized spiking graph attention
- Edge deployment for IoT graph processing
- WASM-based spiking graph simulation via existing WASM crates

---

## References

1. Zhu et al., "Spiking Graph Neural Networks," IEEE TNNLS 2023
2. Hazan et al., "BindsNET: A Machine Learning-Oriented Spiking Neural Networks Library in Python," Frontiers in Neuroinformatics 2018
3. Tavanaei et al., "Deep Learning in Spiking Neural Networks," Neural Networks 2019
4. London & Hausser, "Dendritic Computation," Annual Review of Neuroscience 2005
5. Poirazi & Papoutsi, "Illuminating dendritic function with computational models," Nature Reviews Neuroscience 2020
6. Breakspear, "Dynamic Models of Large-Scale Brain Activity," Nature Neuroscience 2017
7. Davies et al., "Loihi 2: A Neuromorphic Processor with Programmable Synapses and Neuron Models," IEEE Micro 2021

---

**End of Document 23**

**Next:** [Doc 24 - Quantum Graph Attention](24-quantum-graph-attention.md)
