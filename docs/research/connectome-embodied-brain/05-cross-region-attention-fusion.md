---
Research Document ID: RD-C-05
Date: 2026-04-21
Status: Draft
Authors: RuView Research Team
Related ADRs: proposed ADR-085
---

# RD-C-05: Cross-Region Attention Fusion (Layer 2 / Layer 4 bridge)

## Abstract

The fly brain contains ~80 anatomically defined neuropils (AL, MB, CX,
LAL, GNG, VNC, etc.), each computing partial representations of the
environment and internal state. Fusing their activity into a single
decision-relevant embedding is the neural analog of the multistatic
WiFi-sensing fusion already implemented in
`rust-port/.../viewpoint/fusion.rs`. This document specifies
`NeuralFusionArray`, a DDD aggregate root that mirrors the existing
`MultistaticArray` — cross-viewpoint scaled dot-product attention with a
geometric bias term, gated by a rolling coherence window and described
by a geometric diversity index — but re-targets every component from RF
viewpoints to connectomic regions. The resulting fused embedding feeds
downstream CRV behavioral-episode encoding (07) and min-cut
behavior-conditioned circuit extraction (03 §6).

---

## Table of Contents

1. Motivation — regions as viewpoints
2. Direct analogy to `MultistaticArray`
3. Region activity embedding
4. Geometric bias in connectomic space
5. Coherence gate on population state
6. Functional Diversity Index (connectomic GDI analog)
7. `NeuralFusionArray` aggregate
8. Worked example — mushroom-body attention during foraging
9. Integration with LIF runtime
10. Non-goals
11. References

---

## 1. Motivation — Regions as Viewpoints

The multistatic WiFi sensing pipeline treats each access point as a
viewpoint on the propagation environment: the scene exists, and each AP
samples it from a geometrically distinct vantage. The fusion aggregate
combines these into a single scene embedding weighted by how much each
viewpoint adds (angle diversity, baseline distance, SNR).

A connectome has the same structure at the region level. The organism's
state exists; each neuropil samples a partial view (visual, olfactory,
mechanosensory, contextual, motor-plan-in-progress, etc.); those views
must be fused into a coherent behavioral signal. The mathematical
machinery — scaled dot-product attention with a learned query over
key–value region embeddings — is identical.

## 2. Direct Analogy to `MultistaticArray`

| `viewpoint/fusion.rs` concept | Neural analog |
|-------------------------------|---------------|
| `ViewpointEmbedding` | `RegionEmbedding` |
| `NodeId` (u32) | `RegionId` (u16) |
| `azimuth`, `elevation` | angular position in region meta-graph (optional) |
| `baseline` distance | connectomic path-length to "reference" region |
| `snr_db` | mean firing rate / recent signal-to-background |
| `GeometricBias` | `ConnectomicBias` — path-length + transmitter-sign modifier |
| `CrossViewpointAttention` | `CrossRegionAttention` (same SDPA kernel) |
| `CoherenceGate` | unchanged; gate on population phase coherence |
| `GeometricDiversityIndex` | `FunctionalDiversityIndex` |
| `MultistaticArray` aggregate root | `NeuralFusionArray` aggregate root |

The reuse is structural, not literal: we do not import
`MultistaticArray` for neural use; we copy its shape into a sibling
aggregate in the proposed `wifi-densepose-neuro` crate.

## 3. Region Activity Embedding

```text
RegionEmbedding {
    region_id:      RegionId,
    embedding:      Vec<f32>,   // length = d_model (typically 128)
    mean_rate_hz:   f32,
    phase_vector:   [f32; 2],   // unit-norm (cos θ, sin θ)
    baseline_mu:    f32,        // connectomic distance to anchor region
    snr_proxy:      f32,        // recent mean rate / background
    t_ms:           f32,
}
```

Construction: at a given simulation time $t$, for each region,
aggregate the last 50 ms of spikes and rates over every neuron in that
region into a fixed-length vector. Two simple constructors:

1. **Rate histogram.** Bin spike counts by neuron class (sensory /
   interneuron / motor), pad to `d_model`.
2. **Mean-pooled per-neuron embedding.** Average `NeuronEmbedding`
   weights (02 §6) over neurons active in the window.

Mean-pooled is more expressive; rate histogram is cheaper and serves as
a default.

## 4. Geometric Bias in Connectomic Space

`viewpoint/fusion.rs` `GeometricBias` encodes angular separation and
baseline distance between viewpoint pairs. The connectomic analog:

$$
  G^{\text{conn}}_{ij} = -w_{\text{dist}} \cdot \frac{d_{ij}^{\text{path}}}{d_{\text{ref}}}
                       - w_{\text{sign}} \cdot \mathbb{1}[\mathrm{sign}(ij) < 0],
$$

where $d_{ij}^{\text{path}}$ is the shortest weighted path length between
regions $i$ and $j$ in the `RegionGraph`, $d_{\text{ref}}$ is the median
pairwise path length, and the sign term penalises predominantly-inhibitory
region-region connections so that attention prefers cooperative pairs.
Added to the query-key logits inside the softmax; same shape as the RF
case.

## 5. Coherence Gate on Population State

The neural coherence signal (Accept / PredictOnly / Reject / Recalibrate)
uses the same hysteresis gate as `viewpoint/coherence.rs`
`CoherenceGate`, re-seeded with a neural coherence definition:

$$
  C(t) = \frac{1}{N_{\text{reg}}} \sum_r \Bigl\lVert \frac{1}{|r|} \sum_{i \in r} e^{i \phi_i(t)} \Bigr\rVert,
$$

a mean across regions of each region's population-vector phasor
magnitude. Accept when $C(t)$ is above threshold with hysteresis;
PredictOnly in the hysteresis band; Reject when below; Recalibrate on
detected drift.

Thresholds (starting values):

| State | $C(t)$ range |
|-------|--------------|
| Accept | ≥ 0.6 |
| PredictOnly | 0.5–0.6 |
| Reject | < 0.5 |
| Recalibrate | $\|\Delta C\| > 0.2$ within 100 ms |

## 6. Functional Diversity Index

The `GeometricDiversityIndex` in `viewpoint/geometry.rs` measures how
well-spread viewpoints are. For regions, we use a functional-spread
analog:

$$
  \mathrm{FDI} = \frac{1}{N_{\text{reg}}} \sum_r \min_{r' \ne r} \, 1 - \cos\bigl(e_r, e_{r'}\bigr),
$$

the mean cosine-distance from each region's embedding to its nearest
neighbour. High FDI means regions produce distinct representations
(cooperative fusion is informative); low FDI means they are redundant
(fusion adds little).

Cramer-Rao-analog uncertainty bounds on behavioral decoding follow from
constructing the Fisher information matrix over region embeddings and
inverting via `ruvector-solver`'s `NeumannSolver` (same pattern as
`viewpoint/geometry.rs` §4). This mirrors the RF Cramer-Rao bound
almost verbatim.

## 7. `NeuralFusionArray` Aggregate

```text
NeuralFusionArray {
    id:                 ArrayId,                 // u64
    config:             FusionConfig,            // same shape as viewpoint::FusionConfig
    coherence_gate:     CoherenceGate,           // reuse impl
    fdi:                FunctionalDiversityIndex,// computed on fuse()
}

FusionConfig {
    embed_dim:              usize,      // 128
    coherence_threshold:    f32,        // 0.6
    coherence_hysteresis:   f32,        // 0.05
    coherence_window:       usize,      // 50 frames @ 1 kHz = 50 ms
    w_dist:                 f32,        // geometric-bias distance weight
    w_sign:                 f32,        // inhibitory penalty weight
    d_ref:                  f32,        // reference path length (connectomic units)
    min_rate_hz:            f32,        // per-region minimum mean rate to contribute
}

FusedEmbedding {
    embedding:      Vec<f32>,
    fdi:            f32,
    coherence:      f32,
    n_regions:      usize,
    n_effective:    f32,
}
```

Fuse pipeline (matches `viewpoint/fusion.rs`):

1. Filter `RegionEmbedding`s by `min_rate_hz` (SNR analog).
2. Compute FDI from the surviving set.
3. Compute ConnectomicBias matrix.
4. Run `ScaledDotProductAttention::new(d_model).compute(&query, &keys, &values)` with bias-adjusted query.
5. Check `CoherenceGate`; if Reject or PredictOnly, emit the fused vector with a `coherence` tag so downstream can discount.
6. Emit `FusedEmbedding`.

## 8. Worked Example — Mushroom-Body Attention During Foraging

During a foraging bout the mushroom body (MB) attends to olfactory
glomeruli that encode the relevant odor and the central complex (CX)
attends to current heading. Expected pattern:

| Region | Pre-foraging attention weight | During foraging |
|--------|-------------------------------|-----------------|
| AL (olfactory glomeruli) | 0.1 | 0.35 |
| MB (Kenyon cells) | 0.1 | 0.40 |
| CX (EB/FB/PB) | 0.2 | 0.10 |
| LAL (steering motor) | 0.1 | 0.05 |
| VLP (visual LP) | 0.2 | 0.05 |
| GNG (grooming premotor) | 0.1 | 0.01 |
| VNC (motor) | 0.2 | 0.04 |

The precise numbers depend on fit quality; the qualitative shift — AL
and MB gain weight at the expense of GNG/VLP — is a falsifiable
prediction. Cross-behavior attention maps are published in Lappalainen
2024 for vision; equivalent for olfaction is less mature and a natural
CC-OS output.

## 9. Integration with LIF Runtime

`NeuralFusionArray` is re-evaluated every `fusion_period_ms` (default
50 ms) during a simulation run:

```text
every fusion_period_ms:
    for each region r:
        build RegionEmbedding from last 50 ms of spikes
    fused = NeuralFusionArray.fuse(region_embeddings)
    if fused.coherence ≥ threshold:
        hand fused to BehaviorPipeline (07)
    else:
        record as "uncertain" in witness log
```

This is the handoff between Layer 2 (LIF runtime) and Layer 4 (analysis
and behavioral-episode encoding). It is architecturally symmetric with
the RF pipeline, where CSI frames feed the existing fusion before
behavioral claims are made.

## 10. Non-Goals

- **Biologically plausible global workspace theory.** `NeuralFusionArray`
  is a computational fusion aggregate, not a model of conscious
  broadcasting.
- **Trainable attention weights.** The attention is parameter-free SDPA
  over connectome-derived embeddings; no gradient-based tuning in v1.
- **Real-time long-range delays.** Bias term approximates path-length
  effect; not a substitute for explicit delay modelling.

## 11. References

1. Vaswani, A., et al. (2017). *Attention is all you need.* NeurIPS.
2. Lappalainen, J. K., et al. (2024). *Connectome-constrained networks
   predict neural activity across the fly visual system.* Nature.
3. Aso, Y., Sitaraman, D., Ichinose, T., et al. (2014). *Mushroom body
   output neurons encode valence and guide memory-based action
   selection.* eLife. (mushroom-body attention precedent.)
4. Turner-Evans, D. B., Jayaraman, V. (2016). *The insect central
   complex.* Curr Biol.
5. RuView `viewpoint/fusion.rs` (crate `wifi-densepose-ruvector`).
6. RuVector v2.0.4 crate docs — `ruvector-attention`,
   `ruvector-solver`.

---

**Next**: 06-embodied-simulator-closed-loop.md — closing the sensorimotor
loop through a virtual body.
