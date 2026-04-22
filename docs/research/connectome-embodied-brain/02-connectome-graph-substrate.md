---
Research Document ID: RD-C-02
Date: 2026-04-21
Status: Draft
Authors: RuView Research Team
Related ADRs: proposed ADR-084
---

# RD-C-02: Connectome Graph Substrate (Layer 1)

## Abstract

Layer 1 of the Coherence-Aware Connectome Operating System (CC-OS) is a
typed, provenance-tagged, read-mostly graph store for a published
connectome (primarily FlyWire, Dorkenwald et al. 2024) with per-neuron
activity embeddings and tiered temporal state compression. This document
specifies the schema for `Neuron`, `Synapse`, and `Region` value objects;
the `ConnectomeGraph` aggregate root; the edge-triplet encoding that feeds
`ruvector-mincut`; the per-neuron embedding tensor that feeds
`ruvector-attention`; and the voltage/spike-train compression protocol built
on `ruvector-temporal-tensor`. All identifiers, storage layouts, and query
patterns are sized for the 139k-neuron / 54M-synapse fly regime on a single
workstation.

---

## Table of Contents

1. Requirements
2. Neuron node schema
3. Synapse edge schema
4. Region / neuropil meta-graph
5. Mapping to `ruvector-mincut` edge triplets
6. Per-neuron activity embeddings
7. Temporal state storage via `ruvector-temporal-tensor`
8. Sizing worked example
9. Query patterns
10. DDD bounded context
11. Integration with existing RuView code
12. Non-goals
13. References

---

## 1. Requirements

| Requirement | Target | Rationale |
|-------------|--------|-----------|
| Neuron count | 10k–150k | FlyWire full brain = 139k |
| Synapse count | 1M–60M | FlyWire reports ~54M |
| Adjacency access | O(degree) per neuron | event-driven LIF propagation |
| Per-neuron state write | O(1) | 1 kHz voltage updates |
| Bulk graph read at startup | < 10 s for 139k nodes / 54M edges | User experience |
| Memory budget | < 8 GB for full FlyWire | Laptop-grade target |
| Persistence | Append-only with SHA-256 | Witness-audit compatibility (ADR-028) |
| Concurrent readers | Yes | LIF + analysis can query in parallel |
| Concurrent writer | Single | Deterministic replay |

## 2. Neuron Node Schema

```text
Neuron {
    id:                 u64,            // FlyWire root_id or internal
    class:              NeuronClass,    // Sensory / Interneuron / Motor / Ascending / Descending
    cell_type_hash:     u64,            // stable hash of FlyWire cell_type
    neurotransmitter:   Neurotransmitter,   // Ach / GABA / Glu / DA / 5HT / NE / Peptide / Unknown
    morphology_hash:    u64,            // hash of skeleton for de-duplication
    region_id:          RegionId,       // neuropil
    position:           Option<(f32, f32, f32)>,  // Cartesian centroid (µm)
    side:               Side,           // Left / Right / Midline
    sensory_label:      Option<SensoryLabel>,   // Bristle / JohnstonsOrgan / Ocelli / etc.
    motor_label:        Option<MotorLabel>,     // ProthoracicLegMN / WingMN / etc.
    provenance:         ProvenanceTag,  // dataset + version + SHA-256 of record
}
```

Enum sketches:

```text
NeuronClass   ::= Sensory | Interneuron | Motor | Ascending | Descending
Neurotransmitter ::= Ach | GABA | Glu | DA | Serotonin | NE | Peptide | Unknown
Side          ::= Left | Right | Midline
```

`id` is `u64` rather than the existing `u32` `NodeId` from
`viewpoint/geometry.rs` because FlyWire root_ids are 64-bit. Section 11
discusses the widening.

## 3. Synapse Edge Schema

```text
Synapse {
    pre_id:             u64,
    post_id:            u64,
    weight:             u16,            // synaptic contact count
    sign:               Sign,           // +1 for excitatory, -1 for inhibitory
    nt:                 Neurotransmitter,  // inferred from presynaptic neuron
    axonal_delay_ms:    f32,            // estimated from neurite length
    plasticity_tag:     PlasticityTag,  // Static | STP | LTP | LTD | Pending
    provenance:         ProvenanceTag,
}
```

Sign is deterministic given the presynaptic neuron's transmitter (Ach/Glu →
+1 by default, GABA → −1, others → lookup table). Axonal delay is
provisional; at fly scale, neurite lengths imply delays in the 0.5–5 ms
range. The canonical formulation for integer-weighted connectomes is
`weight = number of anatomical synapses`; functional correlation weights
live separately in a side-table for analysis (see 03
§3 and 04-neural-dynamics-runtime.md §5).

## 4. Region / Neuropil Meta-Graph

The 80+ FlyWire neuropils (AL, MB, CX, LAL, GNG, VNC, etc.) form a
higher-level `RegionGraph` where each node is a region and each edge
aggregates synaptic weight between regions.

```text
Region {
    id:             RegionId,       // u16 is sufficient
    name:           String,         // FlyWire neuropil label
    neuron_count:   u32,
    centroid:       Option<(f32, f32, f32)>,
    functional_tag: RegionTag,      // Visual | Olfactory | Motor | Memory | etc.
}

RegionEdge {
    from:           RegionId,
    to:             RegionId,
    total_weight:   u32,            // sum of synapse counts
    n_synapses:     u32,
    mean_delay_ms:  f32,
}
```

This meta-graph is the target input for 05-cross-region-attention-fusion.md:
per-region activity embeddings feed `ScaledDotProductAttention` with
geometric biases derived from `RegionEdge` connectomic distance. This is
the neural analog of `viewpoint/geometry.rs`
`GeometricDiversityIndex`.

## 5. Mapping to `ruvector-mincut` Edge Triplets

`MinCutBuilder` expects `Vec<(u64, u64, f64)>`. The encoder:

```rust
fn edges_for_mincut(graph: &ConnectomeGraph,
                    source_nodes: &[u64],
                    sink_nodes:   &[u64]) -> Vec<(u64, u64, f64)> {
    let n = graph.neuron_count() as u64;
    let source = n;
    let sink   = n + 1;
    let mut edges = Vec::with_capacity(graph.synapse_count() + source_nodes.len() + sink_nodes.len());

    for &s in source_nodes { edges.push((source, s, 1e18)); }
    for &t in sink_nodes   { edges.push((t, sink, 1e18)); }

    for syn in graph.synapses() {
        let w = f64::from(syn.weight);
        edges.push((syn.pre_id, syn.post_id, w));
    }
    edges
}
```

This mirrors `signal/subcarrier.rs` verbatim up to domain-specific naming.
The virtual source and sink nodes sit at `n` and `n+1`. Inhibitory synapse
handling is discussed in 03 §3.

## 6. Per-Neuron Activity Embeddings

Each neuron carries a fixed-dimensional embedding used by
`ScaledDotProductAttention` as a key or value. Recommended dimension
matches the existing `AETHER` embedding size (128-d).

```text
NeuronEmbedding {
    id:         u64,
    dim:        u16,            // typically 128
    weights:    Vec<f32>,       // length = dim
    source:     EmbeddingSource,  // Raw | TrainedAETHER | Lappalainen2024
    timestamp:  u64,            // wallclock of computation
}
```

The embedding table is a dense `[n_neurons, dim]` matrix. At 139k × 128 ×
4 bytes = 71 MB, it fits in cache layers comfortably. Recomputed
embeddings are versioned; older versions can be kept in warm storage via
the temporal-tensor tiers described in §7.

## 7. Temporal State Storage via `ruvector-temporal-tensor`

Running LIF dynamics produces two streams:

1. **Membrane voltage** — `f32` per neuron per timestep.
2. **Spike train** — sparse `u64` per spike or dense bit-vector per
   timestep.

Both are perfect fits for `TemporalTensorCompressor`:

```rust
use ruvector_temporal_tensor::{TemporalTensorCompressor, TierPolicy};

pub struct VoltageBuffer {
    compressor:     TemporalTensorCompressor,
    segments:       Vec<Vec<u8>>,
    frame_count:    u32,
    n_neurons:      usize,
}

impl VoltageBuffer {
    pub fn new(n_neurons: usize, tensor_id: u32) -> Self {
        Self {
            compressor: TemporalTensorCompressor::new(
                TierPolicy::default(),
                n_neurons as u32,
                tensor_id,
            ),
            segments:    Vec::new(),
            frame_count: 0,
            n_neurons,
        }
    }

    pub fn push_frame(&mut self, voltages: &[f32]) {
        let ts = self.frame_count;
        self.compressor.set_access(ts, ts);
        let mut seg = Vec::new();
        self.compressor.push_frame(voltages, ts, &mut seg);
        if !seg.is_empty() { self.segments.push(seg); }
        self.frame_count += 1;
    }
}
```

This is the **same pattern** as `mat/breathing.rs`
`CompressedBreathingBuffer`, simply retargeted from 56 subcarrier
amplitudes to `n_neurons` voltages. Tier policies:

| Tier | Bits | Typical window | Compression vs f32 |
|------|------|----------------|--------------------|
| Hot | 8 | last 10 ms | 4× |
| Warm | 5–7 | last 1–10 s | 5–6× |
| Cold | 3 | > 10 s | ~10× |

## 8. Sizing Worked Example

Scenario: 50k neurons, 1 kHz voltage, 60 s of simulation, one full run.

| Quantity | Raw | Tiered (est.) | Notes |
|----------|-----|---------------|-------|
| Voltages: 50k × 1kHz × 60s × f32 | 12 GB | ~3.0 GB | 3-bit cold tier for the first 50 s, warm for next 9 s, hot for last 1 s |
| Spikes (bit-vector): 50k × 60k × 1b | 375 MB | ~40–60 MB | bit-packed, then tier-compressed |
| Spikes (event list at 5 Hz avg): 15M events × 16 B | 240 MB | — | alternative; no tier compression needed |
| Graph edges (2M × 24 B) | 48 MB | 48 MB | CSR, rarely changes |
| Neuron metadata (50k × ~200 B) | 10 MB | 10 MB | small |
| Neuron embeddings (50k × 128 × f32) | 26 MB | — | dense |

Total active memory budget for one 60 s run at 50k neurons: **~4 GB**.
Full FlyWire 139k at 60 s: **~12 GB raw, ~3–4 GB tiered**, within reach of
a 32 GB workstation.

## 9. Query Patterns

Common queries and their cost class:

| Query | Cost class | Implementation hint |
|-------|------------|---------------------|
| k-hop downstream of neuron set | O(mean_degree^k) | BFS with visited set |
| All motor MNs active in window | O(spikes_in_window) | scan tier-appropriate segments |
| Subcircuit induced by weight ≥ w | O(|E|) | filter+induced-subgraph builder |
| Min-cut between S and T | see 03 §10 | `MinCutBuilder::exact()` |
| Similar-activity neurons | O(n · dim) | embedding cosine search |
| Region × region flux | O(|region_edges|) | meta-graph aggregation |

For recurring queries (e.g. "activity of descending motor neurons during
last 200 ms"), the runtime should maintain live indices; otherwise the
query cost becomes a log-compaction problem.

## 10. DDD Bounded Context

`ConnectomeGraph` is the aggregate root; `Neuron`, `Synapse`, `Region` are
value objects; `NeuronEmbedding` and `VoltageBuffer` are separately-owned
entities referenced by id.

Domain events:

| Event | Trigger |
|-------|---------|
| `ConnectomeLoaded { source, sha256, n_neurons, n_synapses }` | startup |
| `SynapseAblated { pre_id, post_id, reason }` | perturbation API |
| `SynapseRestored { pre_id, post_id }` | perturbation rollback |
| `EmbeddingRecomputed { neuron_id, source }` | manual or batch |
| `SpikeObserved { neuron_id, t_ms }` | LIF runtime |
| `RegionFluxSnapshot { region, total_rate, t_ms }` | periodic |

Invariants enforced by the aggregate:

- Every synapse references two existing neurons.
- No duplicate synapses (same pre, post, provenance).
- Neurotransmitter is consistent with presynaptic neuron (unless
  explicitly overridden and flagged in provenance).
- Ablations are append-only events; graph state is materialised by replay.

## 11. Integration with Existing RuView Code

### 11.1 `NodeId` widening

The existing `NodeId = u32` in `viewpoint/geometry.rs` is sufficient for
16–32 sensor nodes. The connectome uses `u64`. We recommend introducing a
new alias `NeuronId = u64` in the proposed `wifi-densepose-connectome`
crate without disturbing the existing `NodeId`.

### 11.2 Reuse of `ApNode` sketch pattern

`crv/mod.rs` `ApNode { id, position, coverage_radius }` is a viable
template for `RegionSketchElement` when CRV Stage III is applied to
behavioral episodes (see 07-coherence-crv-behavioral-episodes.md).

### 11.3 Temporal-tensor reuse

`mat/breathing.rs` `CompressedBreathingBuffer` and
`mat/heartbeat.rs` `CompressedHeartbeatSpectrogram` demonstrate the
per-frame push + lazy decode pattern. `VoltageBuffer` is implemented by
retargeting the same pattern.

### 11.4 Witness bundle inclusion

Every connectome load emits a `ConnectomeLoaded` event whose SHA-256
covers the source file contents. This fits the ADR-028 witness-bundle
convention without modification.

## 12. Non-Goals

- **Dendritic compartments** — single-compartment point neurons only.
- **Multi-scale biophysics** — no ion channels, no NEURON/GENESIS
  compatibility.
- **Connectome generation** — CC-OS ingests published connectomes; it does
  not segment EM volumes or proofread reconstructions.
- **Runtime graph mutation in the spike loop** — ablations are explicit
  perturbation events, not continuous plasticity. STP/LTP are deferred
  to v2.
- **Mammalian connectomes** — out of scope for v1 on compute and ethics
  grounds (see 11-risks-positioning-roadmap.md).

## 13. References

1. Dorkenwald, S., et al. (2024). *Neuronal wiring diagram of an adult
   brain.* Nature. (FlyWire dataset.)
2. Eckstein, N., et al. (2024). *Neurotransmitter classification from
   electron microscopy images.* Cell.
3. Scheffer, L. K., et al. (2020). *Connectome of the adult Drosophila
   central brain.* eLife.
4. ADR-028 — ESP32 capability audit + witness verification (RuView repo).
5. ADR-017 — RuVector signal + MAT integration (RuView repo).
6. Winding, M., et al. (2023). *The connectome of an insect brain.*
   Science.
7. RuVector v2.0.4 crate documentation — `ruvector-mincut`,
   `ruvector-attention`, `ruvector-temporal-tensor`.

---

**Next document**: 03-mincut-circuit-analysis.md (complete) and
04-neural-dynamics-runtime.md — the Rust event-driven LIF runtime that
consumes this substrate.
