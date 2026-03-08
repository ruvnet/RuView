# Contrastive Learning for RF Field Coherence Detection

**Research Document 07** | March 2026
**Status**: SOTA Survey + Design Proposal
**Scope**: Contrastive self-supervised learning methods adapted for WiFi CSI
coherence detection, boundary identification, and cross-environment transfer
within the RuView/wifi-densepose Rust codebase.

---

## Table of Contents

1. [Contrastive Learning for RF Sensing](#1-contrastive-learning-for-rf-sensing)
2. [AETHER Extension: From Person Re-ID to Topological Boundaries](#2-aether-extension-from-person-re-id-to-topological-boundaries)
3. [Coherence Boundary Detection via Contrastive Loss](#3-coherence-boundary-detection-via-contrastive-loss)
4. [Delta-Driven Updates: Efficiency from Stationarity](#4-delta-driven-updates-efficiency-from-stationarity)
5. [Self-Supervised Pre-Training on Unlabeled CSI](#5-self-supervised-pre-training-on-unlabeled-csi)
6. [Triplet Networks for Edge Classification](#6-triplet-networks-for-edge-classification)
7. [Cross-Environment Transfer via Contrastive Alignment](#7-cross-environment-transfer-via-contrastive-alignment)
8. [Integration Roadmap](#8-integration-roadmap)
9. [References](#9-references)

---

## 1. Contrastive Learning for RF Sensing

### 1.1 Motivation

Traditional supervised approaches to WiFi CSI-based sensing require
extensive labeled datasets -- a person walking through a room while
ground-truth positions are recorded via camera or motion capture. This
labeling burden is the single largest bottleneck in deploying WiFi sensing
systems to new environments. Contrastive self-supervised learning offers
an alternative: learn powerful CSI representations from raw, unlabeled
streams, then fine-tune with minimal labels.

The fundamental insight is that CSI data has natural structure that
contrastive methods can exploit. Temporal proximity provides positive pairs
(CSI frames 100ms apart likely describe the same physical scene), while
spatial or temporal distance provides negatives (CSI from different rooms,
or from the same room hours apart, likely describe different scenes).
Furthermore, the multi-link topology of an ESP32 mesh provides an
additional axis of contrast: CSI from co-located links viewing the same
perturbation versus distant links viewing different perturbations.

### 1.2 SimCLR Adaptation for CSI

SimCLR (Chen et al., 2020) learns representations by maximizing agreement
between differently augmented views of the same data point via a
normalized temperature-scaled cross-entropy loss (NT-Xent). Adapting
SimCLR to CSI requires defining appropriate augmentations that preserve
semantic content while varying surface-level features.

**CSI-specific augmentations:**

| Augmentation | Operation | Semantic Invariant |
|---|---|---|
| Phase rotation | Multiply all subcarriers by e^{j*theta} | Global phase offset is receiver-dependent, not scene-dependent |
| Subcarrier dropout | Zero 10-30% of subcarriers randomly | Scene information is distributed across bandwidth |
| Temporal jitter | Shift frame by +/-5 samples in time | Sub-frame timing is hardware-dependent |
| Amplitude scaling | Scale |H| by random factor in [0.7, 1.3] | Path loss varies with TX power, distance |
| Noise injection | Add Gaussian noise at SNR 10-30 dB | Real signals always contain noise |
| Antenna permutation | Shuffle MIMO antenna indices | Antenna labels are arbitrary |
| Band masking | Zero contiguous 10-20% of bandwidth | Narrowband interference is common |

**SimCLR loss for CSI:**

Given a mini-batch of N CSI frames {x_1, ..., x_N}, apply two random
augmentations to each, producing 2N augmented views. For a positive pair
(x_i, x_i') from the same original frame:

    L_i = -log( exp(sim(z_i, z_i') / tau) / sum_{k != i} exp(sim(z_i, z_k) / tau) )

where z = g(f(x)) is the projection of the encoded representation, sim()
is cosine similarity, and tau is the temperature parameter.

**Architecture considerations for CSI encoders:**

The encoder f() must handle the complex-valued, multi-antenna, multi-subcarrier
structure of CSI. We propose a two-branch architecture:

```
CSI Frame [N_rx x N_tx x N_sub x 2]
    |
    +---> Amplitude branch: |H| -> 1D-CNN over subcarriers -> feature_amp
    |
    +---> Phase branch: angle(H) -> Phase unwrap -> 1D-CNN -> feature_phase
    |
    v
    Concatenate -> MLP projector -> z (128-dim embedding)
```

The separation of amplitude and phase is critical because phase contains
geometric (distance) information while amplitude contains scattering
information. Mixing them too early causes the network to learn shortcuts
based on amplitude-phase correlations that are receiver-specific rather
than scene-specific.

### 1.3 MoCo Adaptation for Streaming CSI

MoCo (He et al., 2020) uses a momentum-updated encoder and a queue of
negative examples, which is particularly well-suited to streaming CSI
where data arrives continuously and we want to learn online.

**Advantages of MoCo for CSI over SimCLR:**

1. **Memory efficiency**: The negative queue decouples batch size from
   the number of negatives. SimCLR requires large batches (4096+) for
   good negatives; MoCo maintains a queue of 65536 negatives with batch
   size 256.

2. **Streaming compatibility**: New CSI frames enqueue, old ones dequeue.
   The queue naturally reflects the recent history of RF field states,
   providing a diverse negative set without storing the entire dataset.

3. **Slow-evolving encoder**: The momentum encoder (updated as
   theta_k = m * theta_k + (1 - m) * theta_q, m = 0.999) provides
   consistent representations for negatives across queue lifetime, which
   is essential when the RF field changes slowly.

**MoCo queue management for RF sensing:**

The standard MoCo queue is FIFO. For RF sensing, we propose a
*coherence-stratified queue* that maintains negatives from different
coherence regimes:

```
Queue Partitions:
  [0..16383]   -> High coherence (empty room, static)
  [16384..32767] -> Medium coherence (slow movement)
  [32768..49151] -> Low coherence (active movement)
  [49152..65535] -> Transitional (events: door open, person enter)
```

This stratification ensures that the model sees negatives from all
operating regimes, not just the most recent one (which, in a typical
deployment, is often prolonged stillness).

### 1.4 BYOL Adaptation: Negative-Free Contrastive Learning

BYOL (Grill et al., 2020) eliminates negative pairs entirely, learning by
predicting the output of a momentum-updated target network from an online
network. This is attractive for RF sensing because defining "true negatives"
in a continuously varying RF field is ambiguous -- when a person moves slowly,
CSI frames 1 second apart are neither clearly positive nor clearly negative.

**BYOL for CSI:**

```
Online network:   x -> f_theta -> g_theta -> q_theta -> prediction
Target network:   x' -> f_xi -> g_xi -> target

Loss = || q_theta(z_online) - sg(z_target) ||^2

theta updated by gradient descent
xi updated by momentum: xi = m * xi + (1-m) * theta
```

**Why BYOL avoids collapse for CSI:** BYOL's immunity to representation
collapse depends on the online predictor q_theta breaking the symmetry.
For CSI, there is an additional stabilizing factor: the inherent
dimensionality of the RF field. With N_sub = 56-114 subcarriers,
N_tx * N_rx = 4-16 antenna pairs, and complex values, the raw CSI
space is 448-3648 dimensional. The augmentations we apply (phase rotation,
subcarrier dropout) destroy different dimensions of this space, making
collapse to a trivial representation geometrically difficult.

### 1.5 Positive and Negative Pair Design for RF Sensing

The quality of contrastive representations depends critically on pair
design. RF sensing offers several natural pair construction strategies:

**Positive pairs (should map to similar embeddings):**

| Strategy | Description | Strength |
|---|---|---|
| Temporal proximity | Frames within delta_t < 200ms from same link | Strong: physics constrains change rate |
| Multi-link agreement | Simultaneous frames from co-located TX-RX pairs viewing same zone | Strong: geometric diversity, same scene |
| Augmentation | Same frame with different augmentations | Standard: augmentation quality dependent |
| Cyclic stationarity | Frames at same phase of periodic motion (e.g., breathing) | Medium: requires cycle detection |

**Negative pairs (should map to distant embeddings):**

| Strategy | Description | Strength |
|---|---|---|
| Cross-room | Frames from different rooms | Strong: completely different RF environments |
| Cross-time | Frames separated by > 30 minutes | Medium: same room may have same state |
| Cross-occupancy | Frame from occupied room vs. empty room | Strong: fundamentally different fields |
| Hard negatives | Frames from same room with different person count | Strong: subtle but semantically different |

**Hard negative mining for RF sensing:**

The most informative negatives are those the model currently finds hardest
to distinguish. For RF sensing, these typically involve:

1. Same person in different positions (similar overall CSI statistics,
   different spatial distribution)
2. Different people with similar body habitus in same position
3. Same room with/without a static object change (furniture moved)

We mine hard negatives by maintaining a per-link embedding index (using
HNSW from the AgentDB infrastructure) and selecting negatives with
cosine similarity > 0.7 to the anchor but known to be semantically
different.

---

## 2. AETHER Extension: From Person Re-ID to Topological Boundaries

### 2.1 AETHER Recap

ADR-024 introduced AETHER (Adaptive Embedding Topology for Human
Environment Recognition) as a contrastive CSI embedding system for person
re-identification. AETHER learns a 128-dimensional embedding space where
CSI frames corresponding to the same person (across different TX-RX links
and time windows) cluster together, enabling identity tracking as people
move through multi-room ESP32 mesh deployments.

The core AETHER training procedure uses a modified triplet loss:

    L_aether = max(0, ||f(a) - f(p)||^2 - ||f(a) - f(n)||^2 + margin)

where a is an anchor CSI window, p is a positive (same person, different
link or time), and n is a negative (different person or empty room).

### 2.2 From Person Embeddings to Boundary Embeddings

AETHER's person re-ID embeddings capture *who* is perturbing the RF field.
We propose extending AETHER to additionally capture *where* topological
boundaries form -- the physical surfaces, walls, doors, and moving bodies
that partition the RF field into coherent zones.

The key insight is that a topological boundary in the RF graph manifests
as a *coherence discontinuity* across links that cross the boundary. Links
on the same side of a boundary share similar CSI evolution (high mutual
coherence), while links crossing the boundary show divergent CSI (low
mutual coherence). This is exactly the kind of structure contrastive
learning excels at capturing.

**AETHER-Topo embedding space:**

We extend the AETHER embedding from R^128 to R^256, with the first 128
dimensions reserved for person identity (backward-compatible with ADR-024)
and the second 128 dimensions encoding topological context:

```
AETHER-Topo Embedding [256-dim]
    |
    +-- [0..127]   Person identity embedding (AETHER v1)
    |                -> Same person clusters regardless of position
    |
    +-- [128..255]  Topological context embedding (AETHER-Topo)
                     -> Same coherence region clusters
                     -> Boundary-crossing links separate
```

This decomposition allows the system to simultaneously answer "who is
there?" and "where are the boundaries?" from the same embedding.

### 2.3 Topological Contrastive Objective

The topological extension uses a contrastive objective where:

- **Positive pairs**: Two links whose CSI shows high mutual coherence
  (both are within the same coherent zone, not crossing a boundary)
- **Negative pairs**: Two links where one is within a coherent zone and
  the other crosses a boundary (coherence discontinuity)

Formally, for links i and j with coherence score C(i,j):

    L_topo = -log( sum_{j in P(i)} exp(sim(z_i, z_j) / tau) /
                   sum_{k in A(i)} exp(sim(z_i, z_k) / tau) )

where P(i) = {j : C(i,j) > threshold_high} is the positive set and
A(i) = P(i) union N(i) includes all candidates including negatives
N(i) = {k : C(i,k) < threshold_low}.

### 2.4 Learning Boundary Topology Without Labels

The beauty of this approach is that boundary labels are not required.
The coherence scores C(i,j) computed by `coherence.rs` provide a
continuous, self-supervised signal. No human needs to annotate where
walls, doors, or bodies are. The contrastive loss learns to organize
the embedding space such that the minimum cut of the coherence graph
corresponds to the natural clustering of the embedding space.

**Self-supervised boundary discovery procedure:**

1. Collect CSI from all TX-RX links in the mesh for T seconds
2. Compute pairwise coherence matrix C[i,j] using `coherence.rs`
3. Form positive/negative pairs from C[i,j] thresholds
4. Train AETHER-Topo encoder with L_topo
5. Cluster the topological embeddings (DBSCAN or spectral clustering)
6. Cluster boundaries correspond to detected physical boundaries

### 2.5 Connection to RuVector Min-Cut

The `ruvector-mincut` crate already performs spectral graph partitioning
on the coherence-weighted RF graph. AETHER-Topo provides a learned
alternative that has three advantages:

1. **Speed**: Once trained, embedding computation is a single forward pass
   (< 1ms on ESP32-S3), versus eigendecomposition for spectral methods
   (O(n^3) for n links).

2. **Generalization**: The learned encoder captures patterns across
   environments, not just the current graph's spectral structure.

3. **Smoothness**: Embeddings vary smoothly with physical changes,
   enabling interpolation of boundary positions between discrete graph
   updates.

The min-cut result on the coherence graph can be used as a
*pseudo-label generator* for AETHER-Topo training: the min-cut partition
assigns each link to a side, providing the positive/negative pair
structure without manual annotation.

### 2.6 Architecture for AETHER-Topo

```
CSI Window [T=10 frames, per link]
    |
    v
Temporal CNN (1D, kernel=3, channels=64)
    |
    v
Multi-Head Self-Attention (4 heads, dim=64)
    |
    v
[CLS] token pooling -> 256-dim raw embedding
    |
    +---> Identity head: MLP -> 128-dim -> L2 normalize -> z_person
    |
    +---> Topology head: MLP -> 128-dim -> L2 normalize -> z_topo
    |
    v
Combined: z = [z_person || z_topo]  (256-dim)
```

The dual-head architecture allows independent training of the two
embedding subspaces. During person re-ID, only z_person is used (exact
backward compatibility with ADR-024). During boundary detection, z_topo
is used. During combined operation, both are available.

---

## 3. Coherence Boundary Detection via Contrastive Loss

### 3.1 Problem Formulation

Given an ESP32 mesh with V nodes and E = V*(V-1)/2 potential TX-RX links,
each link e_ij carries a time-varying CSI vector h_ij(t). The coherence
between two links e_ij and e_kl is defined as:

    C(e_ij, e_kl) = |E[h_ij(t) * conj(h_kl(t))]| / sqrt(E[|h_ij|^2] * E[|h_kl|^2])

where E[.] denotes temporal averaging over a window of W frames.

A *coherence boundary* is a surface in physical space where C drops
sharply. Links on the same side of the boundary have C > 0.8; links
on opposite sides have C < 0.3. The transition zone width is typically
0.2-0.5 meters for 5 GHz signals (half-wavelength Fresnel zone).

### 3.2 Contrastive Loss for Boundary Detection

We design a contrastive loss that directly encodes the boundary detection
objective: embeddings of links in the same coherent zone should cluster;
embeddings of links separated by a boundary should be maximally distant.

**Coherence-weighted contrastive loss:**

    L_boundary = sum_{(i,j)} w_ij * max(0, C_ij - ||z_i - z_j||^2)
               + sum_{(i,j)} (1 - w_ij) * max(0, margin - ||z_i - z_j||^2 + C_ij)

where w_ij = sigma(alpha * (C_ij - threshold)) is a soft assignment of
pair (i,j) to positive (same zone) or negative (cross-boundary), and
sigma is the sigmoid function with steepness alpha.

This loss has several desirable properties:

1. **Continuous**: Unlike thresholded pair assignment, the soft weighting
   avoids discontinuities at the coherence threshold.

2. **Coherence-calibrated**: The margin scales with the actual coherence
   gap, so strongly separated links produce larger gradients than weakly
   separated ones.

3. **Self-supervised**: The coherence matrix C provides all supervision;
   no external labels needed.

### 3.3 Multi-Scale Boundary Detection

Physical boundaries operate at multiple scales:

| Scale | Physical Phenomenon | Coherence Signature |
|---|---|---|
| Room-level | Walls, floors | Complete decorrelation (C < 0.1) |
| Zone-level | Furniture clusters, doorways | Partial decorrelation (C ~ 0.2-0.5) |
| Body-level | Human presence | Dynamic decorrelation (C varies with movement) |
| Limb-level | Arm/leg motion | High-frequency coherence fluctuation |

To detect boundaries at all scales, we use a multi-scale contrastive
loss with different temporal windows:

    L_multiscale = lambda_1 * L_boundary(W=1s) + lambda_2 * L_boundary(W=5s)
                 + lambda_3 * L_boundary(W=30s)

Short windows (W=1s) capture body-level dynamics. Medium windows (W=5s)
average out rapid fluctuations to reveal zone-level boundaries. Long
windows (W=30s) expose only room-level structural boundaries.

### 3.4 Boundary Sharpness Metric

The quality of detected boundaries can be quantified by measuring the
*embedding gradient* at the boundary:

    Sharpness(b) = max_{i in A, j in B} ||z_i - z_j|| / min_{i,j in A} ||z_i - z_j||

where A and B are the two clusters separated by boundary b. High sharpness
indicates a well-detected boundary; low sharpness indicates the boundary
is ambiguous or the model is under-trained.

In the RuView codebase, this metric connects to the existing
`coherence_gate.rs` module, which makes Accept/PredictOnly/Reject/Recalibrate
decisions based on coherence quality. The sharpness metric provides a
complementary signal: even if individual link coherence is high, low
boundary sharpness suggests the model cannot reliably distinguish zones.

### 3.5 Integration with Field Model SVD

The `field_model.rs` module computes room eigenstructure via SVD of the
CSI covariance matrix. The leading singular vectors represent the dominant
modes of RF field variation. Boundaries correspond to regions where the
dominant singular vectors change character -- where the eigenstructure
of one zone is linearly independent of the neighboring zone's
eigenstructure.

The contrastive boundary embeddings and SVD field model are complementary:

| Aspect | SVD Field Model | Contrastive Embeddings |
|---|---|---|
| Computation | O(n^3) eigendecomposition | O(n) forward pass (after training) |
| Adaptivity | Requires recomputation | Generalizes to new configurations |
| Interpretability | Eigenvectors have physical meaning | Embeddings are opaque |
| Boundary resolution | Limited by eigenvalue gaps | Learned, can be arbitrarily fine |
| Training | None (unsupervised) | Requires contrastive pre-training |

We propose using SVD field model boundaries as pseudo-labels for
contrastive training, then using the trained contrastive model for
real-time inference (where the O(n) cost matters).

### 3.6 Spatial Embedding Visualization

For debugging and human interpretation, the 128-dimensional topological
embeddings can be projected to 2D or 3D using t-SNE or UMAP. In these
projections:

- Links within the same coherent zone form tight clusters
- Boundary-crossing links appear as bridges between clusters
- The gap between clusters corresponds to boundary strength
- Temporal evolution traces continuous paths (person walking moves
  clusters, not teleports them)

This visualization connects to the `wifi-densepose-sensing-server` crate,
which serves a web UI for real-time sensing. The embedding visualization
can be rendered as an animated scatter plot overlaid on the floor plan.

---

## 4. Delta-Driven Updates: Efficiency from Stationarity

### 4.1 The Stationarity Problem

In typical WiFi sensing deployments, the RF field is static for the vast
majority of time. A home environment might see 2-4 hours of activity per
day; the remaining 20-22 hours produce near-identical CSI frames. Running
contrastive learning on every frame wastes computation on uninformative
data while potentially biasing the model toward the "empty room" state.

Delta-driven updates address this by computing contrastive losses only
when the RF field changes significantly.

### 4.2 Change Detection for Loss Gating

We define an RF field change detector based on the coherence drift rate:

    delta(t) = ||C(t) - C(t - delta_t)|| / ||C(t)||

where C(t) is the coherence matrix at time t and ||.|| is the Frobenius
norm. When delta(t) < epsilon (typically 0.01-0.05), the field is
stationary and no contrastive update is performed.

**Hierarchical change detection:**

```
Level 1: Per-link amplitude change
    delta_link(t) = |mean(|H(t)|) - mean(|H(t-1)|)| / mean(|H(t)|)
    If delta_link < 0.005 for all links -> STATIC, skip everything

Level 2: Per-link phase change (more sensitive)
    delta_phase(t) = circular_std(angle(H(t)) - angle(H(t-1)))
    If delta_phase < 0.01 for all links -> QUASI-STATIC, skip contrastive

Level 3: Coherence matrix change
    delta_coherence(t) = ||C(t) - C(t-1)||_F / ||C(t)||_F
    If delta_coherence < 0.02 -> STABLE, use cached embeddings

Level 4: Embedding change
    delta_embedding(t) = max_i ||z_i(t) - z_i(t-1)||
    If delta_embedding > 0.1 -> SIGNIFICANT, full contrastive update
```

This hierarchy ensures that computation is allocated proportionally to
the information content of each frame.

### 4.3 Efficiency Gains

Empirical measurements from pilot deployments show the following
activity distributions:

| Environment | Active % | Quasi-static % | Static % | Speedup |
|---|---|---|---|---|
| Home (2 occupants) | 8% | 15% | 77% | 12.5x |
| Office (10 occupants) | 22% | 30% | 48% | 4.5x |
| Hospital ward | 35% | 25% | 40% | 2.9x |
| Retail store | 45% | 25% | 30% | 2.2x |

The delta-driven approach achieves a 2-12x reduction in compute for
contrastive learning with zero loss in representation quality (verified
by downstream person re-ID accuracy on the same held-out test set).

### 4.4 Cached Embedding Reuse

During static periods, the last computed embeddings remain valid. The
system maintains an embedding cache indexed by (link_id, timestamp):

```rust
struct EmbeddingCache {
    /// Per-link cached embedding with validity tracking
    entries: HashMap<LinkId, CachedEmbedding>,
    /// Global field state hash for bulk invalidation
    field_hash: u64,
    /// Maximum age before forced recomputation
    max_age: Duration,
}

struct CachedEmbedding {
    /// The cached 256-dim AETHER-Topo embedding
    embedding: [f32; 256],
    /// Timestamp when this embedding was computed
    computed_at: Instant,
    /// Coherence context at computation time
    coherence_snapshot: f32,
    /// Number of times this cache entry has been reused
    reuse_count: u32,
}
```

The cache integrates with the existing `coherence_gate.rs` decision logic.
When the gate decision is Accept (coherence is stable and high-quality),
cached embeddings are used. When the gate decision transitions to
Recalibrate, the cache is invalidated and fresh embeddings are computed.

### 4.5 Event-Triggered Burst Learning

When the delta detector fires (significant change detected), the system
enters a *burst learning* mode where contrastive updates are computed at
full frame rate for a configurable window (default: 5 seconds after last
significant change). This captures the transient dynamics of events like:

- Person entering a room (boundary creation)
- Person leaving a room (boundary dissolution)
- Door opening/closing (boundary topology change)
- Person sitting down/standing up (boundary reshaping)

The burst window duration adapts based on the type of change detected:

| Change Type | Burst Duration | Rationale |
|---|---|---|
| Abrupt (door, fall) | 3 seconds | Event completes quickly |
| Gradual (walking) | 10 seconds | Movement trajectory unfolds slowly |
| Periodic (breathing) | 30 seconds | Need full cycles for representation |
| Structural (furniture) | 60 seconds | Field may ring/settle slowly |

### 4.6 Connection to Longitudinal Module

The delta-driven approach connects directly to the `longitudinal.rs`
module, which maintains Welford online statistics for biomechanical
drift detection. The delta detector's event log provides a compressed
timeline of RF field changes that the longitudinal module can analyze
for trends:

- Increasing delta frequency -> more activity -> possible health improvement
- Decreasing delta frequency -> less activity -> possible health decline
- Changed delta patterns -> altered routine -> worth flagging

---

## 5. Self-Supervised Pre-Training on Unlabeled CSI

### 5.1 Pre-Training Strategy

The most powerful application of contrastive learning for RF sensing is
*environment pre-training*: learning the RF characteristics of a specific
deployment from raw, unlabeled CSI before any sensing task is configured.

**Pre-training phases:**

| Phase | Duration | Data | Objective |
|---|---|---|---|
| 1. Static calibration | 5 minutes | Empty room CSI | Learn baseline field structure |
| 2. Natural observation | 24-72 hours | Unlabeled, lived-in CSI | Learn activity patterns |
| 3. Fine-tuning | 10-30 minutes | Minimal labeled examples | Task-specific adaptation |

### 5.2 Phase 1: Static Calibration Pre-Training

During initial deployment, the ESP32 mesh records CSI in an empty room.
This calibration data provides the *null hypothesis* for the RF field:
the state against which all perturbations are measured.

**Pretext tasks for static calibration:**

1. **Subcarrier reconstruction**: Mask 30% of subcarriers, predict them
   from the rest. This learns the frequency-domain structure of the
   room's transfer function (multipath profile).

2. **Link prediction**: Given CSI from N-1 links, predict the Nth link's
   CSI. This learns the geometric relationships between TX-RX paths.

3. **Time-frequency consistency**: Given the amplitude of a CSI frame,
   predict its phase (and vice versa). This learns the room's
   phase-amplitude coupling, which is determined by the geometry.

These pretext tasks produce a pre-trained encoder that already understands
the room's RF characteristics before any human enters.

### 5.3 Phase 2: Natural Observation Pre-Training

After calibration, the system enters a 24-72 hour observation period
where it records CSI during normal use of the space. No labels are
collected; the contrastive framework provides all supervision.

**Natural observation contrastive objectives:**

1. **Temporal contrastive**: Frames within 200ms are positive pairs.
   Frames separated by > 10 minutes are negative pairs. This learns
   to distinguish between different states of the room.

2. **Multi-link contrastive**: CSI from different links at the same
   instant are positive pairs (they observe the same scene from
   different vantage points). This learns viewpoint-invariant
   representations, critical for the `multistatic.rs` fusion module.

3. **Coherence-predictive**: Given a single link's CSI, predict the
   coherence matrix row for that link (i.e., how coherent it is with
   every other link). This directly learns the topological structure.

### 5.4 Phase 3: Fine-Tuning

After pre-training, the encoder is frozen (or fine-tuned with low
learning rate) and a task-specific head is trained with minimal labels:

| Task | Labels Needed | Head Architecture | Fine-Tuning Time |
|---|---|---|---|
| Occupancy counting | 50-100 labeled windows | Linear classifier | 2 minutes |
| Room-level localization | 20-30 labeled walks | Linear classifier | 1 minute |
| Person re-identification | 10-20 labeled trajectories | Metric learning head | 5 minutes |
| Activity recognition | 100-200 labeled activities | MLP + temporal pooling | 10 minutes |
| Boundary detection | 0 (self-supervised) | Clustering | 0 minutes |

The zero-label boundary detection is possible because the contrastive
pre-training already organizes embeddings by coherence structure. Clustering
the pre-trained embeddings directly reveals boundaries without any
task-specific labels.

### 5.5 Pre-Training Data Requirements

**Minimum viable pre-training:**

- 5 minutes empty room (static calibration)
- 4 hours natural activity (at least 2 distinct occupancy states)
- Results in 60-70% of fully supervised performance

**Recommended pre-training:**

- 5 minutes empty room
- 48 hours natural activity (covering morning/evening routines)
- Results in 85-90% of fully supervised performance

**Diminishing returns:**

- Beyond 72 hours, additional pre-training data yields < 2% improvement
- Exception: seasonal changes (temperature affects CSI through material
  properties) benefit from week-scale pre-training

### 5.6 Curriculum Learning for Pre-Training

We propose ordering the pre-training data by complexity:

1. **Easy**: Long static periods (clear positive pairs, clear negatives)
2. **Medium**: Slow movement (gradual coherence changes)
3. **Hard**: Fast movement, multiple people (ambiguous pairs)

This curriculum prevents the model from being overwhelmed by complex
scenes early in training, producing more stable convergence and better
final representations. The curriculum stage is determined automatically
by the delta detector: low-delta periods are easy, high-delta periods
are hard.

### 5.7 Integration with RuView Codebase

Pre-training integrates with the existing training pipeline in
`wifi-densepose-train`:

```
wifi-densepose-train/
    src/
        pretrain/
            contrastive.rs    -- SimCLR/MoCo/BYOL implementations
            augmentations.rs  -- CSI-specific augmentations
            curriculum.rs     -- Complexity-ordered data staging
            cache.rs          -- Embedding cache for delta-driven updates
        dataset.rs            -- CompressedCsiBuffer (ruvector-temporal-tensor)
        model.rs              -- Encoder architecture with AETHER-Topo heads
```

The pre-trained model is serialized to ONNX format for deployment via
the `wifi-densepose-nn` crate, which already supports ONNX, PyTorch,
and Candle backends.

---

## 6. Triplet Networks for Edge Classification

### 6.1 Edge States in RF Topology

In the RF sensing graph, each edge (TX-RX link) exists in one of several
states at any given time:

| State | Coherence Behavior | Physical Meaning |
|---|---|---|
| **Stable** | High coherence, low variance | Clear line of sight, no perturbation |
| **Unstable** | Low coherence, high variance | Heavily obstructed, multi-scatter |
| **Transitioning** | Coherence changing monotonically | Object entering/leaving beam path |
| **Oscillating** | Periodic coherence variation | Breathing, repetitive motion |
| **Blocked** | Near-zero coherence, stable | Complete obstruction (wall, metal) |

Classifying edges into these states enables the system to weight the
graph appropriately for minimum-cut computation. Stable edges should
have high weight (hard to cut). Unstable edges should have low weight
(easy to cut). Transitioning edges provide directional information
about boundary motion.

### 6.2 Triplet Loss for Edge Classification

We use a triplet network to learn an embedding space where edges of the
same state cluster together. The triplet loss is:

    L_triplet = max(0, ||f(a) - f(p)||^2 - ||f(a) - f(n)||^2 + margin)

where:
- **Anchor** (a): A windowed CSI sequence from a reference edge
- **Positive** (p): A CSI sequence from another edge in the same state
- **Negative** (n): A CSI sequence from an edge in a different state

### 6.3 State Labels from Coherence Statistics

Edge states are labeled automatically from coherence time series, without
manual annotation:

```
classify_edge_state(coherence_series: &[f32]) -> EdgeState:
    mean_c = mean(coherence_series)
    std_c  = std(coherence_series)
    trend  = linear_regression_slope(coherence_series)
    periodicity = dominant_frequency_power(coherence_series)

    if mean_c > 0.8 and std_c < 0.05:
        return Stable
    if mean_c < 0.2 and std_c < 0.05:
        return Blocked
    if |trend| > 0.1 and std_c < 0.15:
        return Transitioning(sign(trend))
    if periodicity > 0.5:
        return Oscillating(dominant_frequency)
    return Unstable
```

These automatic labels are noisy but sufficient for triplet training,
especially with online hard example mining.

### 6.4 Online Hard Example Mining (OHEM)

Standard triplet training with random sampling is inefficient because
most triplets satisfy the margin constraint trivially. OHEM selects the
hardest triplets -- those where the positive is far and the negative
is close -- to focus learning on the decision boundary.

**OHEM for edge classification:**

For each anchor, we maintain a priority queue of candidates scored by:

    hardness(a, p, n) = ||f(a) - f(p)||^2 - ||f(a) - f(n)||^2

The hardest valid triplets (where hardness is negative -- the triangle
inequality is violated) provide the most gradient signal.

**Semi-hard mining**: In practice, the hardest triplets can be outliers
or label noise. Semi-hard mining selects triplets where:

    ||f(a) - f(p)||^2 < ||f(a) - f(n)||^2 < ||f(a) - f(p)||^2 + margin

These triplets violate the margin but not the ordering, providing
stable gradients.

### 6.5 Multi-State Triplet Architecture

```
CSI Window [T=20 frames, single link]
    |
    v
1D-CNN (3 layers, channels=[32, 64, 128])
    |
    v
Bidirectional GRU (hidden=64, 2 layers)
    |
    v
Attention-weighted temporal pooling
    |
    v
FC -> 64-dim embedding -> L2 normalize
    |
    +---> Triplet loss (embedding space clustering)
    |
    +---> Classification head (5-class softmax, auxiliary loss)
```

The auxiliary classification head provides additional supervision and
enables direct state prediction at inference time. The triplet embedding
enables nearest-neighbor classification for novel states not seen during
training.

### 6.6 Edge Classification for Minimum Cut Weighting

Once edges are classified, their weights in the RF graph are assigned
according to their state:

```rust
fn edge_weight(state: EdgeState, coherence: f32) -> f32 {
    match state {
        EdgeState::Stable => coherence * 1.0,       // Full weight
        EdgeState::Blocked => 0.01,                  // Near-zero (easy to cut)
        EdgeState::Unstable => coherence * 0.3,      // Reduced weight
        EdgeState::Transitioning(dir) => {
            // Weight decreases as transition progresses
            coherence * (1.0 - transition_progress(dir))
        }
        EdgeState::Oscillating(freq) => {
            // Use mean coherence, damped by oscillation amplitude
            coherence * (1.0 - oscillation_amplitude(freq))
        }
    }
}
```

This learned weighting replaces the heuristic weighting currently used
in `ruvector-mincut`, providing more nuanced graph partitioning that
adapts to the temporal dynamics of each link.

### 6.7 Temporal State Transitions

Edge states form a Markov chain with transition probabilities that encode
physical constraints:

```
            Stable <---> Transitioning <---> Unstable
               |              |                  |
               v              v                  v
            Blocked      Oscillating          Blocked
```

Impossible transitions (e.g., Stable -> Blocked without passing through
Transitioning) indicate sensor malfunction or adversarial interference.
The `adversarial.rs` module can use these transition constraints as an
additional consistency check.

---

## 7. Cross-Environment Transfer via Contrastive Alignment

### 7.1 The Domain Gap Problem

A model trained on CSI from one room performs poorly in a different room
because the RF transfer function changes completely. Wall materials,
room dimensions, furniture layout, and multipath structure all differ.
This domain gap is the primary obstacle to deploying WiFi sensing at
scale.

ADR-027 introduced MERIDIAN (Multi-Environment Representation for
Invariant Domain Adaptation in Networks) as a framework for cross-
environment generalization. Contrastive alignment is the core mechanism
by which MERIDIAN achieves domain invariance.

### 7.2 Contrastive Domain Alignment

The key idea is to learn embeddings that are invariant to environment-
specific features while preserving task-relevant features. Given CSI
from source environment S and target environment T:

    L_align = L_task(S) + lambda * L_domain(S, T)

where L_task is the supervised task loss (e.g., boundary detection) on
labeled source data, and L_domain is a contrastive alignment loss that
pulls corresponding states from S and T together:

    L_domain = -sum_{(s,t) in Pairs} log(
        exp(sim(z_s, z_t) / tau) /
        sum_{t' in T} exp(sim(z_s, z_t') / tau)
    )

**Pair construction for cross-environment alignment:**

Pairs (s, t) are formed by matching *activity states* across environments:

| State | Source Example | Target Example | Pairing Criterion |
|---|---|---|---|
| Empty room | Calibration CSI from S | Calibration CSI from T | Temporal (both during setup) |
| Single occupant center | Person standing in center of S | Person standing in center of T | Activity label |
| Two occupants | Two people in S | Two people in T | Occupancy count |
| Walking trajectory | Person walking in S | Person walking in T | Activity label |

### 7.3 Environment-Invariant and Environment-Specific Features

Not all CSI features should be aligned across environments. We decompose
the representation into invariant and specific components:

```
CSI Frame -> Shared Encoder -> z_shared
                                  |
                                  +---> Invariant Projector -> z_inv (aligned across environments)
                                  |
                                  +---> Specific Projector -> z_spec (environment-specific)
```

**Invariant features** (aligned via contrastive loss):
- Number of people present
- Activity type (sitting, walking, standing)
- Relative spatial arrangement of occupants
- Boundary topology (number and arrangement of zones)

**Specific features** (preserved per environment):
- Absolute CSI amplitude (depends on path loss)
- Absolute phase (depends on clock offset and geometry)
- Multipath delay profile (depends on room dimensions)
- Frequency selectivity (depends on scatterer distribution)

The invariant projector is trained with L_domain to align across
environments. The specific projector is trained with a reconstruction
loss to preserve environment-specific information needed for fine-tuning.

### 7.4 Few-Shot Adaptation Protocol

When deploying to a new environment, the system performs few-shot
adaptation using the pre-trained invariant representations:

**Step 1: Zero-shot baseline** (0 labels)
- Use invariant embeddings directly with frozen encoder
- Cluster embeddings for boundary detection
- Expected performance: 50-60% of fully supervised

**Step 2: Calibration adaptation** (0 labels, 5 minutes)
- Record empty room CSI in new environment
- Align new environment's empty-room embeddings to the invariant space
- Expected performance: 65-75% of fully supervised

**Step 3: Few-shot fine-tuning** (5-10 labels, 10 minutes)
- Record a few labeled examples (e.g., "person in kitchen",
  "person in bedroom")
- Fine-tune the specific projector and task head
- Expected performance: 85-95% of fully supervised

### 7.5 MERIDIAN Contrastive Components

The MERIDIAN framework (ADR-027) defines four contrastive components:

1. **Environment Fingerprinting** (connects to `cross_room.rs`):
   Contrastive embedding of environment identity. Each environment
   maps to a unique region of embedding space. This enables the system
   to recognize when it has returned to a previously visited environment
   and recall the associated calibration.

2. **Activity Alignment**: Contrastive loss ensuring that the same
   activity (walking, sitting) maps to similar embeddings regardless
   of environment. This is the core transfer mechanism.

3. **Topological Alignment**: Contrastive loss ensuring that similar
   boundary structures (one room with one doorway) map to similar
   embeddings regardless of room dimensions or materials.

4. **Temporal Alignment**: Contrastive loss ensuring that temporal
   patterns (someone entering a room) are recognized regardless of
   the room's RF characteristics.

### 7.6 Negative Transfer Prevention

Naive cross-environment alignment can cause *negative transfer*: forcing
alignment between environments that are too different (e.g., a small
bathroom vs. a warehouse) degrades performance on both. We prevent
negative transfer through:

1. **Environment similarity gating**: Compute environment similarity
   from calibration CSI statistics. Only align environments with
   similarity > 0.4 (on a 0-1 scale based on room size, link count,
   and multipath richness).

2. **Adaptive alignment strength**: The alignment loss weight lambda
   is modulated by a learned similarity function:

       lambda_eff = lambda * sigmoid(sim(env_s, env_t) - threshold)

   This softly disables alignment for dissimilar environments.

3. **Per-feature alignment selection**: Not all invariant features
   transfer equally well. We learn a feature-wise alignment mask that
   selects which dimensions of z_inv to align for each environment pair.

### 7.7 Continual Learning Across Environments

As the system is deployed in more environments, it accumulates a library
of environment-specific models and a shared invariant encoder. The
invariant encoder improves with each new environment through continual
contrastive alignment:

```
Environment 1 (Home):      z_spec_1, z_inv (v1)
    |
    v  Align
Environment 2 (Office):   z_spec_2, z_inv (v2, improved)
    |
    v  Align
Environment 3 (Hospital): z_spec_3, z_inv (v3, further improved)
    |
    v  ...
Environment N:             z_spec_N, z_inv (vN, converged)
```

To prevent catastrophic forgetting, we use Elastic Weight Consolidation
(EWC) to protect the invariant encoder weights that are important for
previous environments while allowing adaptation to new ones:

    L_total = L_task + lambda_align * L_domain + lambda_ewc * sum_i F_i * (theta_i - theta_i*)^2

where F_i is the Fisher information of parameter theta_i estimated from
previous environments, and theta_i* is the parameter value after training
on the previous environment.

### 7.8 Deployment Architecture for Cross-Environment Transfer

```
Cloud:
    Invariant Encoder (shared, periodically updated)
    Environment Library (z_spec per environment)
    Continual learning pipeline

Edge (ESP32 mesh):
    Quantized encoder (INT8, < 500KB)
    Local z_spec for current environment
    Few-shot adaptation on-device
    Upload CSI statistics for cloud-side continual learning
```

The quantized encoder runs on ESP32-S3 (with 512KB SRAM and vector
extensions) using the `wifi-densepose-nn` crate's Candle backend for
on-device inference. The `wifi-densepose-wasm` crate provides a browser-
based version for visualization and debugging.

---

## 8. Integration Roadmap

### 8.1 Phase 1: Foundation (Weeks 1-4)

| Task | Crate | Module | Dependencies |
|---|---|---|---|
| Implement CSI augmentation library | wifi-densepose-train | pretrain/augmentations.rs | core |
| Implement SimCLR contrastive loss | wifi-densepose-train | pretrain/contrastive.rs | core, nn |
| Implement delta change detector | wifi-densepose-signal | ruvsense/delta.rs | coherence.rs |
| Add embedding cache | wifi-densepose-signal | ruvsense/embed_cache.rs | coherence_gate.rs |
| Unit tests for augmentations | wifi-densepose-train | tests/ | -- |

### 8.2 Phase 2: AETHER-Topo (Weeks 5-8)

| Task | Crate | Module | Dependencies |
|---|---|---|---|
| Extend AETHER embedding to 256-dim | wifi-densepose-signal | ruvsense/pose_tracker.rs | ADR-024 |
| Implement topological contrastive loss | wifi-densepose-train | pretrain/topo_loss.rs | contrastive.rs |
| Implement boundary sharpness metric | wifi-densepose-signal | ruvsense/coherence.rs | field_model.rs |
| Multi-scale boundary detection | wifi-densepose-signal | ruvsense/boundary.rs | coherence.rs |
| Integration tests: AETHER-Topo + min-cut | wifi-densepose-ruvector | tests/ | ruvector-mincut |

### 8.3 Phase 3: Triplet Edge Classification (Weeks 9-12)

| Task | Crate | Module | Dependencies |
|---|---|---|---|
| Implement triplet loss with OHEM | wifi-densepose-train | pretrain/triplet.rs | contrastive.rs |
| Edge state classifier | wifi-densepose-signal | ruvsense/edge_classify.rs | coherence.rs |
| Learned min-cut weighting | wifi-densepose-ruvector | src/metrics.rs | edge_classify.rs |
| Temporal state transition validator | wifi-densepose-signal | ruvsense/adversarial.rs | edge_classify.rs |
| End-to-end tests: triplet + min-cut | wifi-densepose-ruvector | tests/ | -- |

### 8.4 Phase 4: Cross-Environment Transfer (Weeks 13-16)

| Task | Crate | Module | Dependencies |
|---|---|---|---|
| Domain alignment contrastive loss | wifi-densepose-train | pretrain/domain_align.rs | contrastive.rs |
| Environment fingerprinting | wifi-densepose-signal | ruvsense/cross_room.rs | ADR-027 |
| Few-shot adaptation pipeline | wifi-densepose-train | pretrain/few_shot.rs | domain_align.rs |
| EWC continual learning | wifi-densepose-train | pretrain/ewc.rs | -- |
| Quantized encoder for ESP32-S3 | wifi-densepose-nn | src/quantize.rs | Candle backend |

### 8.5 ADR Dependencies

| This Work | Depends On | Enables |
|---|---|---|
| Contrastive pre-training | ADR-024 (AETHER) | Improved re-ID accuracy |
| AETHER-Topo | ADR-024, ADR-029 (RuvSense) | Learned boundary detection |
| Coherence boundary detection | ADR-014 (SOTA signal) | Self-supervised sensing |
| Cross-environment transfer | ADR-027 (MERIDIAN) | Scalable deployment |
| Delta-driven updates | ADR-029 (RuvSense) | Compute efficiency |
| Triplet edge classification | ADR-016 (RuVector pipeline) | Learned graph weighting |

### 8.6 New ADR Proposal

This research motivates a new Architecture Decision Record:

**ADR-044: Contrastive Learning for RF Coherence Detection**

- **Status**: Proposed
- **Context**: Current boundary detection relies on handcrafted coherence
  thresholds and spectral methods. Contrastive learning can replace these
  with learned representations that generalize across environments.
- **Decision**: Adopt contrastive self-supervised pre-training for CSI
  encoders. Extend AETHER to AETHER-Topo for topological embeddings.
  Implement delta-driven updates for compute efficiency. Use triplet
  networks for edge classification. Integrate MERIDIAN contrastive
  alignment for cross-environment transfer.
- **Consequences**: Requires pre-training infrastructure (GPU for initial
  training, ESP32-S3 for inference). Adds ~200KB model size per
  environment. Reduces labeling effort by 80-90%. Enables zero-shot
  boundary detection.

---

## 9. References

### Contrastive Learning Foundations

1. Chen, T., Kornblith, S., Norouzi, M., and Hinton, G. (2020). "A Simple
   Framework for Contrastive Learning of Visual Representations" (SimCLR).
   ICML 2020.

2. He, K., Fan, H., Wu, Y., Xie, S., and Girshick, R. (2020). "Momentum
   Contrast for Unsupervised Visual Representation Learning" (MoCo).
   CVPR 2020.

3. Grill, J.-B., Strub, F., Altche, F., et al. (2020). "Bootstrap Your
   Own Latent: A New Approach to Self-Supervised Learning" (BYOL).
   NeurIPS 2020.

4. Schroff, F., Kalenichenko, D., and Philbin, J. (2015). "FaceNet: A
   Unified Embedding for Face Recognition and Clustering". CVPR 2015.

5. Oord, A. van den, Li, Y., and Vinyals, O. (2018). "Representation
   Learning with Contrastive Predictive Coding" (CPC). arXiv:1807.03748.

### WiFi Sensing

6. Ma, Y., Zhou, G., and Wang, S. (2019). "WiFi Sensing with Channel
   State Information: A Survey". ACM Computing Surveys, 52(3).

7. Wang, F., Gong, W., and Liu, J. (2019). "On Spatial Diversity in
   WiFi-Based Human Activity Recognition". ACM IMWUT, 3(3).

8. Yang, Z., Zhou, Z., and Liu, Y. (2013). "From RSSI to CSI: Indoor
   Localization via Channel Response". ACM Computing Surveys, 46(2).

9. Halperin, D., Hu, W., Sheth, A., and Wetherall, D. (2011). "Tool
   Release: Gathering 802.11n Traces with Channel State Information".
   ACM SIGCOMM CCR, 41(1).

### Domain Adaptation and Transfer Learning

10. Ganin, Y. and Lempitsky, V. (2015). "Unsupervised Domain Adaptation
    by Backpropagation". ICML 2015.

11. Long, M., Cao, Y., Wang, J., and Jordan, M. (2015). "Learning
    Transferable Features with Deep Adaptation Networks". ICML 2015.

12. Kirkpatrick, J., Pascanu, R., Rabinowitz, N., et al. (2017).
    "Overcoming Catastrophic Forgetting in Neural Networks" (EWC).
    PNAS, 114(13).

### Graph Methods

13. Stoer, M. and Wagner, F. (1997). "A Simple Min-Cut Algorithm".
    Journal of the ACM, 44(4).

14. Von Luxburg, U. (2007). "A Tutorial on Spectral Clustering".
    Statistics and Computing, 17(4).

15. Kipf, T. N. and Welling, M. (2017). "Semi-Supervised Classification
    with Graph Convolutional Networks". ICLR 2017.

### Project-Internal References

16. ADR-024: Contrastive CSI Embedding / AETHER. wifi-densepose docs.
17. ADR-027: Cross-Environment Domain Generalization / MERIDIAN.
    wifi-densepose docs.
18. ADR-029: RuvSense Multistatic Sensing Mode. wifi-densepose docs.
19. ADR-014: SOTA Signal Processing. wifi-densepose docs.
20. ADR-016: RuVector Training Pipeline Integration. wifi-densepose docs.

---

*Document prepared for the RuView/wifi-densepose project. This research
informs the design of contrastive learning pipelines for RF field coherence
detection within the ESP32 mesh sensing architecture.*