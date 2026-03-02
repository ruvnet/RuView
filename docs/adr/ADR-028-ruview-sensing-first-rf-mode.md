# ADR-028: Project RuView -- Sensing-First RF Mode for Multistatic Fidelity Enhancement

| Field | Value |
|-------|-------|
| **Status** | Proposed |
| **Date** | 2026-03-02 |
| **Deciders** | ruv |
| **Codename** | **RuView** -- RuVector Viewpoint-Integrated Enhancement |
| **Relates to** | ADR-012 (ESP32 Mesh), ADR-014 (SOTA Signal), ADR-016 (RuVector Integration), ADR-017 (RuVector Signal+MAT), ADR-021 (Vital Signs), ADR-024 (AETHER Embeddings), ADR-027 (MERIDIAN Cross-Environment) |

---

## 1. Context

### 1.1 The Single-Viewpoint Fidelity Ceiling

Current WiFi DensePose operates with a single transmitter-receiver pair (or single node receiving). This creates three fundamental limitations:

- **Body self-occlusion**: Limbs behind the torso are invisible to a single viewpoint.
- **Depth ambiguity**: Motion along the RF propagation axis (toward/away from receiver) produces minimal phase change.
- **Multi-person confusion**: Two people at similar range but different angles create overlapping CSI signatures.

The ESP32 mesh (ADR-012) partially addresses this via feature-level fusion across 3-6 nodes, but feature-level fusion cannot learn optimal fusion weights -- it uses hand-crafted aggregation (max, mean, coherent sum).

### 1.2 Three Fidelity Levers

1. **Bandwidth**: More bandwidth produces better multipath separability. Currently limited to 20 MHz (ESP32 HT20). Wider channels (80/160 MHz) are available on commodity 802.11ac/ax APs.
2. **Carrier frequency**: Higher frequency produces more phase sensitivity. 2.4 GHz sees macro-motion; 5 GHz sees micro-motion; 60 GHz sees vital signs.
3. **Viewpoints**: More viewpoints from different angles reduces geometric ambiguity. This is the lever RuView pulls.

### 1.3 Why "Sensing-First RF Mode"

RuView is NOT a new WiFi standard. It is a sensing-first protocol that rides on existing silicon, bands, and regulations. The key insight: instead of upgrading the RF hardware, upgrade the observability by coordinating multiple commodity receivers.

### 1.4 What Already Exists

| Component | ADR | Current State |
|-----------|-----|---------------|
| ESP32 mesh with feature-level fusion | ADR-012 | Implemented (firmware + aggregator) |
| SOTA signal processing (Hampel, Fresnel, BVP, spectrogram) | ADR-014 | Implemented |
| RuVector training pipeline (5 crates) | ADR-016 | Complete |
| RuVector signal + MAT integration (7 points) | ADR-017 | Accepted |
| Vital sign detection pipeline | ADR-021 | Partially implemented |
| AETHER contrastive embeddings | ADR-024 | Proposed |
| MERIDIAN cross-environment generalization | ADR-027 | Proposed |

RuView fills the gap: **cross-viewpoint embedding fusion** using learned attention weights.

---

## 2. Decision

Introduce RuView as a cross-viewpoint embedding fusion layer that operates on top of AETHER per-viewpoint embeddings. RuView adds a new bounded context (ViewpointFusion) and extends three existing crates.

### 2.1 Core Architecture

```
+-----------------------------------------------------------------+
|                    RuView Multistatic Pipeline                    |
+-----------------------------------------------------------------+
|                                                                   |
|  +----------+  +----------+  +----------+  +----------+          |
|  | Node 1   |  | Node 2   |  | Node 3   |  | Node N   |          |
|  | ESP32-S3 |  | ESP32-S3 |  | ESP32-S3 |  | ESP32-S3 |          |
|  |          |  |          |  |          |  |          |          |
|  | CSI Rx   |  | CSI Rx   |  | CSI Rx   |  | CSI Rx   |          |
|  +----+-----+  +----+-----+  +----+-----+  +----+-----+          |
|       |              |              |              |               |
|       v              v              v              v               |
|  +--------------------------------------------------------+      |
|  |              Per-Viewpoint Signal Processing             |      |
|  |  Phase sanitize -> Hampel -> BVP -> Subcarrier select   |      |
|  |              (ADR-014, unchanged per viewpoint)          |      |
|  +----------------------------+---------------------------+      |
|                               |                                   |
|                               v                                   |
|  +--------------------------------------------------------+      |
|  |              Per-Viewpoint AETHER Embedding              |      |
|  |  CsiToPoseTransformer -> 128-d contrastive embedding    |      |
|  |              (ADR-024, one per viewpoint)                |      |
|  +----------------------------+---------------------------+      |
|                               |                                   |
|                [emb_1, emb_2, ..., emb_N]                         |
|                               |                                   |
|                               v                                   |
|  +--------------------------------------------------------+      |
|  |        * RuView Cross-Viewpoint Fusion *                |      |
|  |                                                          |      |
|  |  Q = W_q * X,  K = W_k * X,  V = W_v * X              |      |
|  |  A = softmax((QK^T + G_bias) / sqrt(d))                |      |
|  |  fused = A * V                                          |      |
|  |                                                          |      |
|  |  G_bias: geometric bias from viewpoint pair geometry    |      |
|  |  (ruvector-attention: ScaledDotProductAttention)         |      |
|  +----------------------------+---------------------------+      |
|                               |                                   |
|                        fused_embedding                            |
|                               |                                   |
|                               v                                   |
|  +--------------------------------------------------------+      |
|  |              DensePose Regression Head                   |      |
|  |  Keypoint head: [B,17,H,W]                             |      |
|  |  Part/UV head: [B,25,H,W] + [B,48,H,W]                |      |
|  +--------------------------------------------------------+      |
+-----------------------------------------------------------------+
```

### 2.2 TDM Sensing Protocol

- Coordinator (aggregator) broadcasts sync beacon at start of each cycle.
- Each node transmits in assigned time slot; all others receive.
- 6 nodes x 1.4 ms/slot = 8.4 ms cycle -> ~119 Hz aggregate, ~20 Hz per bistatic pair.
- Clock drift handled at feature level (no cross-node phase alignment).

### 2.3 Geometric Bias Matrix

The geometric bias `G_bias` encodes the spatial relationship between viewpoint pairs:

```
G_bias[i,j] = w_angle * cos(theta_ij) + w_dist * exp(-d_ij / d_ref)
```

where:

- `theta_ij` = angle between viewpoint i and viewpoint j (from room center)
- `d_ij` = baseline distance between node i and node j
- `w_angle`, `w_dist` = learnable weights
- `d_ref` = reference distance (room diagonal / 2)

This allows the attention mechanism to learn that widely-separated, orthogonal viewpoints are more complementary than clustered ones.

### 2.4 Coherence-Gated Environment Updates

```rust
/// Only update environment model when phase coherence exceeds threshold.
pub fn coherence_gate(
    phase_diffs: &[f32],  // delta-phi over T recent frames
    threshold: f32,        // typically 0.7
) -> bool {
    // Complex mean of unit phasors
    let (sum_cos, sum_sin) = phase_diffs.iter()
        .fold((0.0f32, 0.0f32), |(c, s), &dp| {
            (c + dp.cos(), s + dp.sin())
        });
    let n = phase_diffs.len() as f32;
    let coherence = ((sum_cos / n).powi(2) + (sum_sin / n).powi(2)).sqrt();
    coherence > threshold
}
```

### 2.5 Two Implementation Paths

| Path | Hardware | Bandwidth | Per-Viewpoint Rate | Target Tier |
|------|----------|-----------|-------------------|-------------|
| **ESP32 Multistatic** | 6x ESP32-S3 ($84) | 20 MHz (HT20) | 20 Hz | Silver |
| **Cognitum + RF** | Cognitum v1 + LimeSDR | 20-160 MHz | 20-100 Hz | Gold |

ESP32 path: commodity, achievable today, targets Silver tier (tracking + pose quality).
Cognitum path: higher fidelity, targets Gold tier (tracking + pose + vitals).

---

## 3. DDD Design

### 3.1 New Bounded Context: ViewpointFusion

**Aggregate Root: `MultistaticArray`**

```rust
pub struct MultistaticArray {
    /// Unique array deployment ID
    id: ArrayId,
    /// Viewpoint geometry (node positions, orientations)
    geometry: ArrayGeometry,
    /// TDM schedule (slot assignments, cycle period)
    schedule: TdmSchedule,
    /// Active viewpoint embeddings (latest per node)
    viewpoints: Vec<ViewpointEmbedding>,
    /// Fused output embedding
    fused: Option<FusedEmbedding>,
    /// Coherence gate state
    coherence_state: CoherenceState,
}
```

**Entity: `ViewpointEmbedding`**

```rust
pub struct ViewpointEmbedding {
    /// Source node ID
    node_id: NodeId,
    /// AETHER embedding vector (128-d)
    embedding: Vec<f32>,
    /// Geometric metadata
    azimuth: f32,      // radians from array center
    elevation: f32,    // radians
    baseline: f32,     // meters from centroid
    /// Capture timestamp
    timestamp: Instant,
    /// Signal quality
    snr_db: f32,
}
```

**Value Object: `GeometricDiversityIndex`**

```rust
pub struct GeometricDiversityIndex {
    /// GDI = (1/N) sum min_{j!=i} |theta_i - theta_j|
    value: f32,
    /// Effective independent viewpoints (after correlation discount)
    n_effective: f32,
    /// Worst viewpoint pair (most redundant)
    worst_pair: (NodeId, NodeId),
}
```

**Domain Events:**

```rust
pub enum ViewpointFusionEvent {
    ViewpointCaptured { node_id: NodeId, timestamp: Instant, snr_db: f32 },
    TdmCycleCompleted { cycle_id: u64, viewpoints_received: usize },
    FusionCompleted { fused_embedding: Vec<f32>, gdi: f32 },
    CoherenceGateTriggered { coherence: f32, accepted: bool },
    GeometryUpdated { new_gdi: f32, n_effective: f32 },
}
```

### 3.2 Extended Bounded Contexts

**Signal (wifi-densepose-signal):**
- New service: `CrossViewpointSubcarrierSelection`
  - Consensus sensitive subcarrier set across all viewpoints via ruvector-mincut.
  - Input: per-viewpoint sensitivity scores. Output: globally-sensitive + locally-sensitive partition.

**Hardware (wifi-densepose-hardware):**
- New protocol: `TdmSensingProtocol`
  - Coordinator logic: beacon generation, slot scheduling, clock drift compensation.
  - Event: `TdmSlotCompleted { node_id, slot_index, capture_quality }`

**Training (wifi-densepose-train):**
- New module: `ruview_metrics.rs`
  - Three-metric acceptance test: PCK/OKS (joint error), MOTA (multi-person separation), vital sign accuracy.
  - Tiered pass/fail: Bronze/Silver/Gold.

---

## 4. Implementation Plan (File-Level)

### 4.1 Phase 1: ViewpointFusion Core (New Files)

| File | Purpose | RuVector Crate |
|------|---------|---------------|
| `crates/wifi-densepose-ruvector/src/viewpoint/mod.rs` | Module root, re-exports | -- |
| `crates/wifi-densepose-ruvector/src/viewpoint/attention.rs` | Cross-viewpoint scaled dot-product attention with geometric bias | ruvector-attention |
| `crates/wifi-densepose-ruvector/src/viewpoint/geometry.rs` | GeometricDiversityIndex, Cramer-Rao bound estimation | ruvector-solver |
| `crates/wifi-densepose-ruvector/src/viewpoint/coherence.rs` | Coherence gating for environment stability | -- (pure math) |
| `crates/wifi-densepose-ruvector/src/viewpoint/fusion.rs` | MultistaticArray aggregate, orchestrates fusion pipeline | ruvector-attention + ruvector-attn-mincut |

### 4.2 Phase 2: Signal Processing Extension

| File | Purpose | RuVector Crate |
|------|---------|---------------|
| `crates/wifi-densepose-signal/src/cross_viewpoint.rs` | Cross-viewpoint subcarrier consensus via min-cut | ruvector-mincut |

### 4.3 Phase 3: Hardware Protocol Extension

| File | Purpose | RuVector Crate |
|------|---------|---------------|
| `crates/wifi-densepose-hardware/src/esp32/tdm.rs` | TDM sensing protocol coordinator | -- (protocol logic) |

### 4.4 Phase 4: Training and Metrics

| File | Purpose | RuVector Crate |
|------|---------|---------------|
| `crates/wifi-densepose-train/src/ruview_metrics.rs` | Three-metric acceptance test (PCK/OKS, MOTA, vital sign accuracy) | ruvector-mincut (person matching) |

---

## 5. Three-Metric Acceptance Test

### 5.1 Metric 1: Joint Error (PCK / OKS)

| Criterion | Threshold |
|-----------|-----------|
| PCK@0.2 (all 17 keypoints) | >= 0.70 |
| PCK@0.2 (torso: shoulders + hips) | >= 0.80 |
| Mean OKS | >= 0.50 |
| Torso jitter RMS (10s window) | < 3 cm |
| Per-keypoint max error (95th percentile) | < 15 cm |

### 5.2 Metric 2: Multi-Person Separation

| Criterion | Threshold |
|-----------|-----------|
| Subjects | 2 |
| Capture rate | 20 Hz |
| Track duration | 10 minutes |
| Identity swaps (MOTA ID-switch) | 0 |
| Track fragmentation ratio | < 0.05 |
| False track creation | 0/min |

### 5.3 Metric 3: Vital Sign Sensitivity

| Criterion | Threshold |
|-----------|-----------|
| Breathing detection (6-30 BPM) | +/- 2 BPM |
| Breathing band SNR (0.1-0.5 Hz) | >= 6 dB |
| Heartbeat detection (40-120 BPM) | +/- 5 BPM (aspirational) |
| Heartbeat band SNR (0.8-2.0 Hz) | >= 3 dB (aspirational) |
| Micro-motion resolution | 1 mm at 3m |

### 5.4 Tiered Pass/Fail

| Tier | Requirements | Deployment Gate |
|------|-------------|-----------------|
| Bronze | Metric 2 | Prototype demo |
| Silver | Metrics 1 + 2 | Production candidate |
| Gold | All three | Full deployment |

---

## 6. Consequences

### 6.1 Positive

- **Fundamental geometric improvement**: Viewpoint diversity reduces body self-occlusion and depth ambiguity -- these are physics, not model, limitations.
- **Uses existing silicon**: ESP32-S3, commodity WiFi, no custom RF hardware required for Silver tier.
- **Learned fusion weights**: Embedding-level fusion (Tier 3) outperforms hand-crafted feature-level fusion (Tier 2).
- **Composes with existing ADRs**: AETHER (per-viewpoint), MERIDIAN (cross-environment), and RuView (cross-viewpoint) are orthogonal -- they compose freely.
- **IEEE 802.11bf aligned**: TDM protocol maps to 802.11bf sensing sessions, enabling future migration to standard-compliant APs.
- **Commodity price point**: $84 for 6-node Silver-tier deployment.

### 6.2 Negative

- **TDM rate reduction**: N viewpoints leads to per-viewpoint rate divided by N. With 6 nodes at 120 Hz aggregate, each viewpoint sees 20 Hz.
- **More complex aggregator**: Embedding fusion + geometric bias learning adds ~25K parameters on top of per-viewpoint AETHER model.
- **Placement planning required**: Geometric Diversity Index optimization requires intentional node placement (not random scatter).
- **Clock drift limits TDM precision**: ESP32 crystal drift (20-50 ppm) limits slot precision to ~1 ms, which is sufficient for feature-level fusion but not signal-level coherent combining.
- **Training data**: Cross-viewpoint training requires multi-receiver CSI captures, which are not available in existing public datasets (MM-Fi, Wi-Pose).

### 6.3 Interaction with Other ADRs

| ADR | Interaction |
|-----|------------|
| ADR-012 (ESP32 Mesh) | RuView extends the aggregator from feature-level to embedding-level fusion; TDM protocol replaces simple UDP collection |
| ADR-014 (SOTA Signal) | Per-viewpoint signal processing is unchanged; cross-viewpoint subcarrier consensus is new |
| ADR-016/017 (RuVector) | All 5 ruvector crates get new cross-viewpoint operations (see Section 4) |
| ADR-021 (Vital Signs) | Multi-viewpoint SNR improvement directly benefits vital sign extraction (Gold tier target) |
| ADR-024 (AETHER) | Per-viewpoint AETHER embeddings are the input to RuView fusion; AETHER is required |
| ADR-027 (MERIDIAN) | Cross-environment (MERIDIAN) and cross-viewpoint (RuView) are orthogonal; MERIDIAN handles room transfer, RuView handles within-room geometry |

---

## 7. References

1. IEEE 802.11bf (2024). "WLAN Sensing." IEEE Standards Association.
2. Kotaru, M. et al. (2015). "SpotFi: Decimeter Level Localization Using WiFi." SIGCOMM 2015.
3. Zeng, Y. et al. (2019). "FarSense: Pushing the Range Limit of WiFi-based Respiration Sensing with CSI Ratio of Two Antennas." MobiCom 2019.
4. Zheng, Y. et al. (2019). "Zero-Effort Cross-Domain Gesture Recognition with Wi-Fi." (Widar 3.0) MobiSys 2019.
5. Yan, K. et al. (2024). "Person-in-WiFi 3D: End-to-End Multi-Person 3D Pose Estimation with Wi-Fi." CVPR 2024.
6. Zhou, Y. et al. (2024). "AdaPose: Towards Cross-Site Device-Free Human Pose Estimation with Commodity WiFi." IEEE IoT Journal. arXiv:2309.16964.
7. Zhou, R. et al. (2025). "DGSense: A Domain Generalization Framework for Wireless Sensing." arXiv:2502.08155.
8. Chen, X. & Yang, J. (2025). "X-Fi: A Modality-Invariant Foundation Model for Multimodal Human Sensing." ICLR 2025. arXiv:2410.10167.
9. AM-FM (2026). "AM-FM: A Foundation Model for Ambient Intelligence Through WiFi." arXiv:2602.11200.
10. Chen, L. et al. (2026). "PerceptAlign: Breaking Coordinate Overfitting." arXiv:2601.12252.
11. Li, J. & Stoica, P. (2007). "MIMO Radar with Colocated Antennas." IEEE Signal Processing Magazine, 24(5):106-114.
12. ADR-012 through ADR-027 (internal).
