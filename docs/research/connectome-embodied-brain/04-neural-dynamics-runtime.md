---
Research Document ID: RD-C-04
Date: 2026-04-21
Status: Draft
Authors: RuView Research Team
Related ADRs: proposed ADR-085
---

# RD-C-04: Neural Dynamics Runtime (Layer 2)

## Abstract

Layer 2 of CC-OS is the event-driven leaky integrate-and-fire (LIF)
runtime that consumes the typed `ConnectomeGraph` from Layer 1 and emits
per-neuron spikes and voltage traces. This document specifies the LIF
model, argues for event-driven rather than clock-driven simulation at
connectome scale, defines the data layout, identifies where
`ruvector-solver`'s Neumann series is applicable (quasi-steady-state
firing-rate approximation) and where it is not (event-driven spike
propagation), and specifies the compressed storage protocol built on
`ruvector-temporal-tensor`. We propose a new workspace crate
`wifi-densepose-neuro` and outline its module layout, dependencies,
determinism requirements, and performance targets (1 kHz real-time for
50k neurons on one workstation).

---

## Table of Contents

1. LIF neuron model
2. Event-driven vs clock-driven simulation
3. Data layout
4. Spike propagation
5. Sparse system solves with `ruvector-solver`
6. Voltage and spike-train storage with `ruvector-temporal-tensor`
7. Replay and motif retrieval
8. Scheduling and concurrency
9. Determinism and witness logs
10. Proposed crate: `wifi-densepose-neuro`
11. Performance targets and benchmarks
12. Non-goals
13. References

---

## 1. LIF Neuron Model

The canonical single-compartment LIF (Brunel 2000, *J Comput Neurosci*):

$$
  \tau_m \frac{dV_i}{dt} = -(V_i - V_{\text{rest}}) + R_m I_i(t),
$$

with a reset rule: when $V_i \ge V_\theta$, emit a spike and clamp $V_i$ to
$V_{\text{reset}}$ for $\tau_{\text{ref}}$ ms. Synaptic current
$I_i(t) = \sum_j w_{ij} \cdot \sum_{s \in \text{spikes}(j)} \kappa(t - s - \delta_{ij})$
where $\kappa$ is an exponential or alpha kernel. Default parameters per
neuron class:

| Parameter | Sensory | Interneuron | Motor | Units |
|-----------|---------|-------------|-------|-------|
| $\tau_m$ | 10 | 15 | 20 | ms |
| $V_{\text{rest}}$ | −65 | −65 | −65 | mV |
| $V_\theta$ | −50 | −50 | −55 | mV |
| $V_{\text{reset}}$ | −70 | −70 | −70 | mV |
| $\tau_{\text{ref}}$ | 2 | 2 | 3 | ms |
| $\tau_{\text{syn}}$ (exc.) | 3 | 3 | 3 | ms |
| $\tau_{\text{syn}}$ (inh.) | 8 | 8 | 8 | ms |

These are literature defaults; real runs should override per-cell-type
where published data exists (Kakaria & de Bivort 2017 for central complex).

## 2. Event-Driven vs Clock-Driven

At 50k neurons with a mean firing rate of 5 Hz, the network emits ~250k
spikes per simulated second. A clock-driven step at 1 kHz visits every
neuron every step — 50M updates/s — regardless of activity. Event-driven
visits only firing neurons and their postsynaptic targets — 250k × 400 =
100M updates/s in the worst case but with better cache behaviour because
updates cluster around active populations.

| Dimension | Clock-driven | Event-driven |
|-----------|--------------|--------------|
| Determinism | Easy (step order) | Harder (priority queue tiebreaks) |
| Parallelism | Embarrassingly parallel | Spike-queue contention |
| Low-rate neurons | Wasteful | Efficient |
| Dense transient bursts | Efficient | Queue grows |
| Voltage log cadence | Fixed | Interpolated |

CC-OS chooses **event-driven with 1 kHz voltage sampling**: spikes drive
state changes, but a fixed-rate sampler still records voltage every ms for
the temporal-tensor storage. This matches the existing
`mat/breathing.rs`-style fixed-rate push semantics.

## 3. Data Layout

```text
State {
    v:              Vec<f32>,       // length = n_neurons — membrane potentials
    last_spike_ms:  Vec<f32>,       // length = n_neurons — for refractory check
    csr_adjacency:  CsrAdjacency,   // read-only view of ConnectomeGraph
    spike_queue:    BinaryHeap<ScheduledSpike>,
    rng:            DeterministicRng,
    t_ms:           f32,
}

ScheduledSpike {
    at_ms:          f32,
    pre_id:         u64,
    post_id:        u64,
    weight:         f32,
}
```

`BinaryHeap` is `std`'s max-heap; we wrap it with `Reverse` to get a
min-heap keyed on `at_ms`. Ties broken by `(pre_id, post_id)` for
determinism. See §9.

## 4. Spike Propagation

When neuron $i$ fires at time $t$:

```text
for each outgoing synapse (i → j) in csr_adjacency.out_edges(i):
    delay     = synapse.axonal_delay_ms
    arrival   = t + delay
    weight    = synapse.weight × synapse.sign
    push ScheduledSpike { at_ms: arrival, pre_id: i, post_id: j, weight }
```

The spike queue is drained up to the current timestep. Each drained event
adds $\kappa(0) \cdot w$ to the target neuron's voltage (using the
synaptic kernel at its peak, then decaying between events).

## 5. Sparse System Solves with `ruvector-solver`

The Neumann series solver $(I - A)^{-1} b = \sum_k A^k b$ converges when
the spectral radius $\rho(A) < 1$. This rules out direct simulation of
the LIF dynamic matrix — the fly connectome has $\rho \gg 1$ when raw
synapse counts are used — but it is well-suited to two specific
sub-problems:

### 5.1 Quasi-steady-state firing rates

For tonic-stimulus analysis, the firing-rate equation
$r = \phi(W r + I)$ can be linearised around a fixed point and solved via
Neumann series if $W$ is appropriately scaled. This is the same use case
the `mat/triangulation.rs` TDoA solver relies on: matrix entries near
unity so the series converges. The scaling factor must be computed per
run; Kakaria & de Bivort 2017 give the scaling convention.

### 5.2 Backward influence for perturbation

If we want "how does a change at neuron $j$ propagate to neuron $k$
through the linear part?", the Neumann series gives a truncated
$\sum_{k \le K} A^k$ solution that is the right approximation for small
perturbations. This feeds the fragility metric of 03 §7 and the
counterfactual protocol of 08.

### 5.3 `CsrMatrix` construction

```rust
use ruvector_solver::types::CsrMatrix;
use ruvector_solver::neumann::NeumannSolver;

// triplets: (row, col, value) — scale weights so spectral radius < 1
let triplets: Vec<(usize, usize, f32)> = graph.synapses()
    .map(|s| (s.post_id as usize, s.pre_id as usize, (s.weight as f32) * scale_factor))
    .collect();

let a = CsrMatrix::<f32>::from_coo(n_neurons, n_neurons, triplets);
let b = vec![1.0_f32; n_neurons];  // external drive

let solver = NeumannSolver::new(1e-5, 500);
let result = solver.solve(&a, &b)?;
let rates = result.solution;
```

## 6. Voltage and Spike-Train Storage

### 6.1 Voltage (per-neuron, 1 kHz)

Use the `VoltageBuffer` pattern from 02 §7, which delegates to
`TemporalTensorCompressor`. At 50k neurons × 1 kHz × 60 s × 4 B = 12 GB
raw; tiered 8/5–7/3-bit brings this to ~3 GB. Stored as an append-only
log of compressed segments.

### 6.2 Spike trains

Two encodings:

**Dense bit-vector per timestep.** `[n_timesteps / 8, n_neurons]` with 1
bit per spike. For a 5 Hz mean rate this is sparse in time, so the
temporal-tensor's 3-bit cold tier compresses it aggressively.

**Event list.** `Vec<(neuron_id, t_ms)>` — 16 bytes per spike. For 250k
spikes/s over 60 s = 15M events = 240 MB. No temporal-tensor compression
needed; sorting by `(t_ms, neuron_id)` gives fast window queries.

Default: dual write. Bit-vector for replay; event list for analysis
queries.

## 7. Replay and Motif Retrieval

Replay is a deterministic re-execution of the LIF runtime seeded from the
same RNG, producing bit-identical spike trains. This is the foundation of
the witness-bundle (§9) and the counterfactual protocol (08).

Motif retrieval uses the per-neuron embeddings from 02 §6 plus short
spike-train signatures. `ruvector-attention` provides
`ScaledDotProductAttention` which is enough to score motif similarity
between the current episode and a library of previously observed
episodes. See 05-cross-region-attention-fusion.md for the region-level
treatment.

## 8. Scheduling and Concurrency

| Component | Concurrency class | Why |
|-----------|-------------------|-----|
| Spike queue drain | Single-threaded | ordering is deterministic |
| Post-synaptic voltage updates | Rayon-parallel | disjoint targets within a time slot |
| Voltage sampler | Single-threaded | 1 kHz cadence |
| Compressed segment writer | `tokio` task | I/O |
| Analysis queries | `tokio` tasks | read-only graph access |

The inner loop is deliberately single-threaded to preserve ordering. At
250k spikes/s each triggering ~400 updates, a tight Rayon parallel fan-out
per time slot delivers the needed throughput without breaking ordering
(updates within a single time slot commute because they modify different
neurons).

## 9. Determinism and Witness Logs

Determinism sources:

- Fixed seed RNG (`rand_chacha::ChaCha20Rng::seed_from_u64(seed)`).
- Ordered event tiebreaks: `(t_ms, pre_id, post_id)`.
- Monotonic timestep integer; no wall-clock dependence.

Witness log schema (reuses ADR-028 pattern):

```text
run_manifest.json {
    connectome_sha256:  "...",
    config_sha256:      "...",
    seed:               42,
    n_neurons:          50000,
    n_synapses:         2000000,
    duration_ms:        60000,
    voltage_buffer_id:  "vbuf-0",
    spike_log_id:       "splog-0",
    output_sha256:      "...",  // hash of concatenated compressed segments
}
```

Re-running with the same seed + graph + config reproduces the same
`output_sha256`. Divergence indicates non-determinism bugs.

## 10. Proposed Crate: `wifi-densepose-neuro`

Location: `rust-port/wifi-densepose-rs/crates/wifi-densepose-neuro/`

```text
src/
├── lib.rs            re-exports
├── lif.rs            LIF neuron kernel, parameters, refractory
├── scheduler.rs      spike queue, time stepping
├── propagate.rs      fan-out, weight application, Rayon parallel slot
├── solver.rs         ruvector-solver adapter (rate-code, perturbation)
├── storage.rs        VoltageBuffer, SpikeLog wrappers
├── replay.rs         deterministic re-execution harness
├── witness.rs        ADR-028 witness bundle writer
└── config.rs         serde-backed run configuration
```

Dependencies (workspace):

```toml
wifi-densepose-core         = { path = "../wifi-densepose-core" }
wifi-densepose-ruvector     = { path = "../wifi-densepose-ruvector" }
ruvector-solver             = { workspace = true }
ruvector-attention          = { workspace = true }
ruvector-temporal-tensor    = { workspace = true }
rand_chacha                 = "0.3"
rayon                       = "1.8"
thiserror                   = { workspace = true }
serde                       = { workspace = true }
```

Publishing order: insert between `wifi-densepose-ruvector` and
`wifi-densepose-train` in the order established in `CLAUDE.md`.

## 11. Performance Targets

| Scale | Spike throughput | Voltage write | Wall-clock per 1 s sim |
|-------|------------------|---------------|------------------------|
| 10k neurons | 50k spikes/s | 40 MB/s raw | 0.3 s (3.3× realtime) |
| 50k neurons | 250k spikes/s | 200 MB/s raw | 1.0 s (1.0× realtime) |
| 139k neurons (full fly) | 700k spikes/s | 560 MB/s raw | 3–5 s (0.2–0.3× realtime) |

Benchmarks to track (Criterion, matching existing repo convention):

- `bench/lif_kernel_throughput` — raw spikes/s for a synthetic Poisson
  network at fixed density.
- `bench/csr_adjacency_fanout` — post-synaptic update cost per spike.
- `bench/voltage_buffer_compression` — TemporalTensorCompressor push
  latency.
- `bench/replay_reproducibility` — SHA-256 equality after 10 s replay.

## 12. Non-Goals

- **Hodgkin–Huxley detail** — single-compartment LIF only.
- **Multi-compartment dendrites** — out of scope.
- **NEURON / Brian2 interchange** — not attempted; CC-OS emits its own
  format.
- **Learning rules inside the spike loop** — STP/LTP/LTD scaffolded in
  the edge schema (02 §3) but not executed online in v1.
- **GPU path** — v1 is CPU-only; GPU is a v2 target.

## 13. References

1. Brunel, N. (2000). *Dynamics of sparsely connected networks of
   excitatory and inhibitory spiking neurons.* J. Comput. Neurosci.
2. Izhikevich, E. M. (2003). *Simple model of spiking neurons.* IEEE TNN.
3. Kakaria, K. S., de Bivort, B. L. (2017). *Ring attractor dynamics
   emerge from a spiking model of the entire protocerebral bridge.*
   Front. Behav. Neurosci.
4. Lappalainen, J. K., et al. (2024). *Connectome-constrained networks
   predict neural activity across the fly visual system.* Nature.
5. ADR-028 — ESP32 capability audit + witness verification.
6. RuVector v2.0.4 crate docs — `ruvector-solver`,
   `ruvector-temporal-tensor`, `ruvector-attention`.
7. Dorkenwald, S., et al. (2024). *Neuronal wiring diagram of an adult
   brain.* Nature.

---

**Next**: 05-cross-region-attention-fusion.md — lifting the
`MultistaticArray` pattern to brain regions.
