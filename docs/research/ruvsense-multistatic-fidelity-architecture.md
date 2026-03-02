# RuvSense: Sensing-First RF Mode for High-Fidelity WiFi DensePose

**Date:** 2026-03-02
**Author:** ruv
**Codename:** **RuvSense** — RuVector-Enhanced Sensing for Multistatic Fidelity
**Scope:** Sensing-first RF mode design, multistatic ESP32 mesh, coherence-gated tracking, and complete RuVector integration for achieving sub-centimeter pose jitter, robust multi-person separation, and small-motion sensitivity on existing silicon.

---

## 1. Problem Statement

WiFi-based DensePose estimation suffers from three fidelity bottlenecks that prevent production deployment:

| Fidelity Metric | Current State (Single ESP32) | Target State (RuvSense) |
|-----------------|------------------------------|-------------------------|
| **Pose jitter** | ~15cm RMS at torso keypoints | <3cm RMS torso, <5cm limbs |
| **Multi-person separation** | Fails above 2 people; frequent ID swaps | 4+ people, zero ID swaps over 10 min |
| **Small motion sensitivity** | Detects gross movement only | Breathing at 3m, heartbeat at 1.5m |
| **Update rate** | 10 Hz effective (single AP CSI) | 20 Hz fused (multistatic) |
| **Temporal stability** | Drifts within hours | Stable over days via coherence gating |

**Acceptance test:** Two people in a room, 20 Hz, stable tracks for 10 minutes with no identity swaps and low jitter in the torso keypoints.

The fundamental insight: **you do not need to invent a new WiFi standard. You need a sensing-first RF mode that rides on existing silicon, bands, and regulations.** The improvement comes from better observability — more viewpoints, smarter bandwidth use, and coherent fusion — not from new spectrum.

---

## 2. The Three Fidelity Levers

### 2.1 Bandwidth: Multipath Separability

More bandwidth separates multipath components better, making pose estimation less ambiguous. The channel impulse response (CIR) resolution is:

```
Δτ = 1 / BW
```

| Configuration | Bandwidth | CIR Resolution | Multipath Separability |
|---------------|-----------|----------------|----------------------|
| ESP32-S3 (HT20) | 20 MHz | 50 ns | ~15m path difference |
| ESP32-S3 (HT40) | 40 MHz | 25 ns | ~7.5m |
| WiFi 6 (HE80) | 80 MHz | 12.5 ns | ~3.75m |
| WiFi 7 (EHT160) | 160 MHz | 6.25 ns | ~1.87m |
| WiFi 7 (EHT320) | 320 MHz | 3.125 ns | ~0.94m |

**RuvSense approach:** Use HT40 on ESP32-S3 (supported in ESP-IDF v5.2) to double subcarrier count from 56 to 114. Then apply `ruvector-solver` sparse interpolation (already integrated per ADR-016) to reconstruct virtual subcarriers between measured ones, achieving effective HT80-like resolution from HT40 hardware.

The key algorithmic insight: the body reflection is spatially sparse — only a few multipath components carry pose information. `ruvector-solver`'s `NeumannSolver` exploits this sparsity via compressed sensing reconstruction:

```
||y - Φx||₂ + λ||x||₁ → min
```

Where `y` is the measured 114 subcarriers, `Φ` is the sensing matrix (DFT submatrix), and `x` is the sparse CIR. The L1 penalty promotes sparse solutions, recovering multipath components that fall between measured subcarrier frequencies.

**Expected improvement:** 2-3x multipath separation without hardware changes.

### 2.2 Carrier Frequency: Phase Sensitivity

Shorter wavelength gives more phase sensitivity to tiny motion. The phase shift from a displacement Δd at carrier frequency f is:

```
Δφ = 4π · f · Δd / c
```

| Band | Frequency | Wavelength | Phase/mm | Application |
|------|-----------|------------|----------|-------------|
| 2.4 GHz | 2.412-2.484 GHz | 12.4 cm | 0.10 rad | Gross movement |
| 5 GHz | 5.150-5.825 GHz | 5.8 cm | 0.21 rad | Pose estimation |
| 6 GHz | 5.925-7.125 GHz | 5.1 cm | 0.24 rad | Fine gesture |

**RuvSense approach:** Deploy ESP32 nodes on both 2.4 GHz and 5 GHz bands simultaneously. The dual-band CSI provides:

1. **Coarse-to-fine resolution**: 2.4 GHz for robust detection (better wall penetration, wider coverage), 5 GHz for fine-grained pose (2x phase sensitivity)
2. **Phase ambiguity resolution**: Different wavelengths resolve 2π phase wrapping ambiguities, similar to dual-frequency radar
3. **Frequency diversity**: Body part reflections at different frequencies have different magnitudes — arms that are invisible at λ/4 = 3.1cm (2.4 GHz half-wavelength null) are visible at λ/4 = 1.45cm (5 GHz)

`ruvector-attention`'s `ScaledDotProductAttention` fuses dual-band CSI with learned frequency-dependent weights, automatically emphasizing the band that carries more information for each body region.

### 2.3 Viewpoints: Geometric Diversity

DensePose accuracy improves fundamentally with multiple viewpoints. A single TX-RX pair observes the body projection onto a single bistatic plane. Multiple pairs observe different projections, resolving depth ambiguity and self-occlusion.

**The geometry argument:**

A single link measures the body's effect on one ellipsoidal Fresnel zone (defined by TX and RX positions). The zone's intersection with the body produces a 1D integral of body conductivity along the ellipsoid. N links with different geometries provide N such integrals. With sufficient angular diversity, these can be inverted to recover the 3D body conductivity distribution — which is exactly what DensePose estimates.

**Required diversity:** For 17-keypoint pose estimation, theoretical minimum is ~6 independent viewpoints (each resolving 2-3 DOF). Practical minimum with noise: 8-12 links with >30° angular separation.

**RuvSense multistatic mesh:**

```
Room Layout (top view, 4m x 5m):

  TX₁ ──────────────── RX₃
  │ \                  / │
  │   \              /   │
  │     \          /     │
  │       \      /       │
  │     Person₁ ·       │
  │       /      \       │
  │     /          \     │
  │   /              \   │
  │ /                  \ │
  RX₁ ──────────────── TX₂

  4 ESP32 nodes → 4 TX + 4 RX = 12 links
  Angular coverage: 360° (full surround)
  Geometric dilution of precision: <2.0
```

Each ESP32-S3 acts as both transmitter and receiver in time-division mode. With 4 nodes, we get C(4,2) × 2 = 12 unique TX-RX links (each direction is a separate observation). With careful scheduling, all 12 links can be measured within a 50ms cycle (20 Hz update).

**TDMA schedule for 4-node mesh:**

| Slot (ms) | TX | RX₁ | RX₂ | RX₃ | Duration |
|-----------|-----|-----|-----|-----|----------|
| 0-4 | Node A | B | C | D | 4ms |
| 5-9 | Node B | A | C | D | 4ms |
| 10-14 | Node C | A | B | D | 4ms |
| 15-19 | Node D | A | B | C | 4ms |
| 20-49 | — | Processing + fusion | | | 30ms |

**Total cycle: 50ms = 20 Hz update rate.**

---

## 3. Sensing-First RF Mode Design

### 3.1 What "Sensing-First" Means

Traditional WiFi treats sensing as a side-effect of communication. CSI is extracted from standard data/management frames designed for connectivity. This is suboptimal because:

1. **Frame timing is unpredictable**: Data traffic is bursty; CSI sample rate varies
2. **Preamble is short**: Limited subcarrier training symbols
3. **No sensing coordination**: Multiple APs interfere with each other's sensing

A sensing-first RF mode inverts the priority: **the primary purpose of the RF emission is sensing; communication rides on top.**

### 3.2 Design on Existing Silicon (ESP32-S3)

The ESP32-S3 WiFi PHY supports:
- 802.11n HT20/HT40 (2.4 GHz + 5 GHz on ESP32-C6)
- Null Data Packet (NDP) transmission (no payload, just preamble)
- CSI callback in ESP-IDF v5.2
- GPIO-triggered packet transmission

**RuvSense sensing frame:**

```
Standard 802.11n NDP frame:
┌──────────────┬──────────────┬──────────────┐
│   L-STF      │    L-LTF     │   HT-SIG     │
│  (8μs)       │   (8μs)      │   (8μs)      │
└──────────────┴──────────────┴──────────────┘
                ▲
                │
    CSI extracted from L-LTF + HT-LTF
    56 subcarriers (HT20) or 114 (HT40)
```

NDP frames are already used by 802.11bf for sensing. They contain only preamble (training symbols) and no data payload, making them:
- **Short**: ~24μs total air time
- **Deterministic**: Same structure every time (no variable-length payload)
- **Efficient**: Maximum CSI quality per unit airtime

**ESP32-S3 NDP injection:** ESP-IDF's `esp_wifi_80211_tx()` raw frame API allows injecting custom NDP frames at precise GPIO-triggered intervals. This is the same API used by ESP-CSI tools.

### 3.3 Sensing Schedule Protocol (SSP)

RuvSense defines a lightweight time-division protocol for coordinating multistatic sensing:

```rust
/// Sensing Schedule Protocol — coordinates multistatic ESP32 mesh
pub struct SensingSchedule {
    /// Nodes in the mesh, ordered by slot assignment
    nodes: Vec<NodeId>,
    /// Duration of each TX slot in microseconds
    slot_duration_us: u32,    // default: 4000 (4ms)
    /// Guard interval between slots in microseconds
    guard_interval_us: u32,   // default: 1000 (1ms)
    /// Processing window after all TX slots
    processing_window_us: u32, // default: 30000 (30ms)
    /// Total cycle period = n_nodes * (slot + guard) + processing
    cycle_period_us: u32,
}
```

**Synchronization:** All ESP32 nodes synchronize via a GPIO pulse from the aggregator node at the start of each cycle. The aggregator also collects CSI from all nodes via UDP and performs fusion. Clock drift between 20ms cycles is <1μs (ESP32 crystal accuracy ±10ppm × 50ms = 0.5μs), well within the guard interval.

### 3.4 IEEE 802.11bf Alignment

IEEE 802.11bf (WLAN Sensing, published 2024) defines:
- **Sensing Initiator / Responder** roles (maps to RuvSense TX/RX slots)
- **Sensing Measurement Setup / Reporting** frames (RuvSense uses NDP + custom reporting)
- **Trigger-Based Sensing** for coordinated measurements

RuvSense's SSP is forward-compatible with 802.11bf. When commercial APs support 802.11bf, the ESP32 mesh can interoperate by translating SSP slots into 802.11bf Sensing Trigger frames.

---

## 4. RuVector Integration Map

### 4.1 System Architecture

```
ESP32 Mesh (4+ nodes)
    │
    │ UDP CSI frames (binary, ADR-018 format)
    │ Per-link: 56-114 subcarriers × I/Q
    │
    ▼
┌─────────────────────────────────────────────────────┐
│           RuvSense Aggregator (Rust)                  │
│                                                       │
│  ┌──────────────────────────────────────┐             │
│  │  Multistatic CSI Collector           │             │
│  │  (per-link ring buffers)             │             │
│  │  ruvector-temporal-tensor            │             │
│  └──────────────┬───────────────────────┘             │
│                 │                                      │
│  ┌──────────────▼───────────────────────┐             │
│  │  Bandwidth Enhancement               │             │
│  │  (sparse CIR reconstruction)         │             │
│  │  ruvector-solver (NeumannSolver)     │             │
│  └──────────────┬───────────────────────┘             │
│                 │                                      │
│  ┌──────────────▼───────────────────────┐             │
│  │  Viewpoint Fusion                    │             │
│  │  (multi-link attention aggregation)  │             │
│  │  ruvector-attention + ruvector-attn  │             │
│  │  -mincut                             │             │
│  └──────────────┬───────────────────────┘             │
│                 │                                      │
│  ┌──────────────▼───────────────────────┐             │
│  │  Subcarrier Selection                │             │
│  │  (dynamic partition per link)        │             │
│  │  ruvector-mincut (DynamicMinCut)     │             │
│  └──────────────┬───────────────────────┘             │
│                 │                                      │
│  ┌──────────────▼───────────────────────┐             │
│  │  Coherence Gate                      │             │
│  │  (reject drift, enforce stability)   │             │
│  │  ruvector-attn-mincut                │             │
│  └──────────────┬───────────────────────┘             │
│                 │                                      │
│  ┌──────────────▼───────────────────────┐             │
│  │  Pose Estimation                     │             │
│  │  (CsiToPoseTransformer + MERIDIAN)   │             │
│  │  ruvector-attention (spatial attn)   │             │
│  └──────────────┬───────────────────────┘             │
│                 │                                      │
│  ┌──────────────▼───────────────────────┐             │
│  │  Track Management                    │             │
│  │  (Kalman + re-ID, ADR-026)           │             │
│  │  ruvector-mincut (assignment)        │             │
│  └──────────────────────────────────────┘             │
│                                                       │
└─────────────────────────────────────────────────────┘
```

### 4.2 RuVector Crate Mapping

| Pipeline Stage | Crate | API | Purpose |
|----------------|-------|-----|---------|
| CSI buffering | `ruvector-temporal-tensor` | `TemporalTensorCompressor` | 50-75% memory reduction for multi-link ring buffers |
| CIR reconstruction | `ruvector-solver` | `NeumannSolver::solve()` | Sparse L1-regularized CIR from HT40 subcarriers |
| Multi-link fusion | `ruvector-attention` | `ScaledDotProductAttention` | Learned per-link weighting for viewpoint fusion |
| Attention gating | `ruvector-attn-mincut` | `attn_mincut()` | Suppress temporally incoherent links (gating) |
| Subcarrier selection | `ruvector-mincut` | `DynamicMinCut` | Per-link dynamic sensitive/insensitive partition |
| Coherence gate | `ruvector-attn-mincut` | `attn_mincut()` | Cross-temporal coherence verification |
| Person separation | `ruvector-mincut` | `MinCutBuilder` | Multi-person CSI component separation |
| Track assignment | `ruvector-mincut` | `DynamicMinCut` | Observation-to-track bipartite matching |

---

## 5. Multistatic Fusion: From N Links to One Pose

### 5.1 The Fusion Problem

With N=12 TX-RX links, each producing 114 subcarriers at 20 Hz, the raw data rate is:

```
12 links × 114 subcarriers × 2 (I/Q) × 4 bytes × 20 Hz = 219 KB/s
```

This must be fused into a single coherent DensePose estimate. The challenge: each link sees the body from a different geometry, so the CSI features are not directly comparable.

### 5.2 Geometry-Aware Link Embedding

Each link's CSI is embedded with its geometric context before fusion:

```rust
/// Embed a single link's CSI with its geometric context.
/// tx_pos, rx_pos: 3D positions of transmitter and receiver (metres).
/// csi: raw CSI vector [n_subcarriers × 2] (I/Q interleaved).
pub fn embed_link(
    tx_pos: &[f32; 3],
    rx_pos: &[f32; 3],
    csi: &[f32],
    geometry_encoder: &GeometryEncoder,  // from MERIDIAN (ADR-027)
) -> Vec<f32> {
    // 1. Encode link geometry
    let geom_embed = geometry_encoder.encode_link(tx_pos, rx_pos); // [64]

    // 2. Normalize CSI (hardware-invariant, from MERIDIAN)
    let csi_norm = hardware_normalizer.normalize(csi); // [56]

    // 3. Concatenate: [56 CSI + 64 geometry = 120]
    // FiLM conditioning: gamma * csi + beta
    let gamma = film_scale.forward(&geom_embed); // [56]
    let beta = film_shift.forward(&geom_embed);  // [56]

    csi_norm.iter().zip(gamma.iter().zip(beta.iter()))
        .map(|(&c, (&g, &b))| g * c + b)
        .collect()
}
```

### 5.3 Attention-Based Multi-Link Aggregation

After embedding, N links are aggregated via cross-attention where the query is a learned "body pose" token and keys/values are the N link embeddings:

```rust
use ruvector_attention::ScaledDotProductAttention;

/// Fuse N link embeddings into a single body representation.
/// link_embeddings: Vec of N vectors, each [d_link=56].
/// Returns fused representation [d_link=56].
pub fn fuse_links(
    link_embeddings: &[Vec<f32>],
    pose_query: &[f32],  // learned query, [d_link=56]
) -> Vec<f32> {
    let d = link_embeddings[0].len();
    let attn = ScaledDotProductAttention::new(d);

    let keys: Vec<&[f32]> = link_embeddings.iter().map(|e| e.as_slice()).collect();
    let values: Vec<&[f32]> = link_embeddings.iter().map(|e| e.as_slice()).collect();

    attn.compute(pose_query, &keys, &values)
        .unwrap_or_else(|_| vec![0.0; d])
}
```

The attention mechanism automatically:
- **Up-weights links** with clear line-of-sight to the body (strong CSI variation)
- **Down-weights links** that are occluded or in multipath nulls (noisy/flat CSI)
- **Adapts per-person**: Different links are informative for different people in the room

### 5.4 Multi-Person Separation via Min-Cut

When N people are present, the N-link CSI contains superimposed contributions from all bodies. Separation requires:

1. **Temporal clustering**: Build a cross-link correlation graph where links observing the same person's motion are connected (high temporal cross-correlation)
2. **Min-cut partitioning**: `DynamicMinCut` separates the correlation graph into K components, one per person
3. **Per-person fusion**: Apply the attention fusion (§5.3) independently within each component

```rust
use ruvector_mincut::{DynamicMinCut, MinCutBuilder};

/// Separate multi-person CSI contributions across links.
/// cross_corr: NxN matrix of cross-link temporal correlation.
/// Returns clusters: Vec of Vec<usize> (link indices per person).
pub fn separate_persons(
    cross_corr: &[Vec<f32>],
    n_links: usize,
    n_expected_persons: usize,
) -> Vec<Vec<usize>> {
    let mut edges = Vec::new();
    for i in 0..n_links {
        for j in (i + 1)..n_links {
            let weight = cross_corr[i][j].max(0.0) as f64;
            if weight > 0.1 {
                edges.push((i as u64, j as u64, weight));
            }
        }
    }

    // Recursive bisection to get n_expected_persons clusters
    let mc = MinCutBuilder::new().exact().with_edges(edges).build();
    recursive_partition(mc, n_expected_persons)
}
```

**Why min-cut works for person separation:** Two links observing the same person have highly correlated CSI fluctuations (the person moves, both links change). Links observing different people have low correlation (independent motion). The minimum cut naturally falls between person clusters.

---

## 6. Coherence-Gated Updates

### 6.1 The Drift Problem

WiFi sensing systems drift over hours/days due to:
- **Environmental changes**: Temperature affects propagation speed; humidity affects absorption
- **AP state changes**: Power cycling, firmware updates, channel switching
- **Gradual furniture/object movement**: Room geometry slowly changes
- **Antenna pattern variation**: Temperature-dependent gain patterns

### 6.2 Coherence Metric

RuvSense defines a real-time coherence metric that quantifies how consistent the current CSI observation is with the recent history:

```rust
/// Compute coherence score between current observation and reference.
/// Returns 0.0 (completely incoherent) to 1.0 (perfectly coherent).
pub fn coherence_score(
    current: &[f32],      // current CSI frame [n_subcarriers]
    reference: &[f32],    // exponential moving average of recent frames
    variance: &[f32],     // per-subcarrier variance over recent window
) -> f32 {
    let n = current.len();
    let mut coherence = 0.0;
    let mut weight_sum = 0.0;

    for i in 0..n {
        let deviation = (current[i] - reference[i]).abs();
        let sigma = variance[i].sqrt().max(1e-6);
        let z_score = deviation / sigma;

        // Coherent if within 3-sigma of expected distribution
        let c = (-0.5 * z_score * z_score).exp();
        let w = 1.0 / (variance[i] + 1e-6); // weight by inverse variance
        coherence += c * w;
        weight_sum += w;
    }

    coherence / weight_sum
}
```

### 6.3 Gated Update Rule

Pose estimation updates are gated by coherence:

```rust
/// Gate a pose update based on coherence score.
pub struct CoherenceGate {
    /// Minimum coherence to accept an update (default: 0.6)
    accept_threshold: f32,
    /// Below this, flag as potential drift event (default: 0.3)
    drift_threshold: f32,
    /// EMA decay for reference update (default: 0.95)
    reference_decay: f32,
    /// Frames since last accepted update
    stale_count: u64,
    /// Maximum stale frames before forced recalibration (default: 200 = 10s at 20Hz)
    max_stale: u64,
}

impl CoherenceGate {
    pub fn update(&mut self, coherence: f32, pose: &Pose) -> GateDecision {
        if coherence >= self.accept_threshold {
            self.stale_count = 0;
            GateDecision::Accept(pose.clone())
        } else if coherence >= self.drift_threshold {
            self.stale_count += 1;
            // Use Kalman prediction only (no measurement update)
            GateDecision::PredictOnly
        } else {
            self.stale_count += 1;
            if self.stale_count > self.max_stale {
                GateDecision::Recalibrate
            } else {
                GateDecision::Reject
            }
        }
    }
}

pub enum GateDecision {
    /// Coherent: apply full pose update
    Accept(Pose),
    /// Marginal: use Kalman prediction, skip measurement
    PredictOnly,
    /// Incoherent: reject entirely, hold last known pose
    Reject,
    /// Prolonged incoherence: trigger SONA recalibration
    Recalibrate,
}
```

### 6.4 Long-Term Stability via SONA Adaptation

When the coherence gate triggers `Recalibrate` (>10s of continuous incoherence), the SONA self-learning system (ADR-005) activates:

1. **Freeze pose output** at last known good state
2. **Collect 200 frames** (10s) of unlabeled CSI
3. **Run contrastive TTT** (AETHER, ADR-024) to adapt the CSI encoder to the new environment state
4. **Update LoRA weights** via SONA (<1ms per update)
5. **Resume sensing** with adapted model

This ensures the system remains stable over days even as the environment slowly changes.

---

## 7. ESP32 Multistatic Mesh Implementation

### 7.1 Hardware Bill of Materials

| Component | Quantity | Unit Cost | Purpose |
|-----------|----------|-----------|---------|
| ESP32-S3-DevKitC-1 | 4 | $10 | TX/RX node |
| ESP32-S3-DevKitC-1 | 1 | $10 | Aggregator (or use x86 host) |
| External 5dBi antenna | 4-8 | $3 | Improved gain/coverage |
| USB-C hub (4 port) | 1 | $15 | Power distribution |
| Mounting brackets | 4 | $2 | Wall/ceiling mount |
| **Total** | | **$73-$91** | Complete 4-node mesh |

### 7.2 Firmware Modifications

The existing ESP32 firmware (ADR-018, 606 lines C) requires these additions:

```c
// sensing_schedule.h — TDMA slot management
typedef struct {
    uint8_t node_id;        // 0-3 for 4-node mesh
    uint8_t n_nodes;        // total nodes in mesh
    uint32_t slot_us;       // TX slot duration (4000μs)
    uint32_t guard_us;      // guard interval (1000μs)
    uint32_t cycle_us;      // total cycle (50000μs for 20Hz)
    gpio_num_t sync_pin;    // GPIO for sync pulse from aggregator
} sensing_schedule_t;

// In main CSI callback:
void csi_callback(void *ctx, wifi_csi_info_t *info) {
    sensing_schedule_t *sched = (sensing_schedule_t *)ctx;

    // Tag frame with link ID (which TX-RX pair)
    esp32_frame_t frame;
    frame.link_id = compute_link_id(sched->node_id, info->src_mac);
    frame.slot_index = current_slot(sched);
    frame.timestamp_us = esp_timer_get_time();

    // Binary serialize (ADR-018 format + link metadata)
    serialize_and_send(&frame, info->buf, info->len);
}
```

**Key additions:**
1. **GPIO sync input**: Listen for sync pulse to align TDMA slots
2. **Slot-aware TX**: Only transmit NDP during assigned slot
3. **Link tagging**: Each CSI frame includes source link ID
4. **HT40 mode**: Configure for 40 MHz bandwidth (114 subcarriers)

### 7.3 Aggregator Architecture

The aggregator runs on the 5th ESP32 (or an x86/RPi host) and:

1. Receives UDP CSI frames from all 4 nodes
2. Demultiplexes by link ID into per-link ring buffers
3. Runs the RuvSense fusion pipeline (§4.1)
4. Outputs fused pose estimates at 20 Hz

```rust
/// RuvSense aggregator — collects and fuses multistatic CSI
pub struct RuvSenseAggregator {
    /// Per-link compressed ring buffers
    link_buffers: Vec<CompressedLinkBuffer>,  // ruvector-temporal-tensor
    /// Link geometry (TX/RX positions for each link)
    link_geometry: Vec<LinkGeometry>,
    /// Coherence gate per link
    link_gates: Vec<CoherenceGate>,
    /// Multi-person separator
    person_separator: PersonSeparator,  // ruvector-mincut
    /// Per-person pose estimator
    pose_estimators: Vec<PoseEstimator>,  // MERIDIAN + AETHER
    /// Per-person Kalman tracker
    trackers: Vec<SurvivorTracker>,  // ADR-026
    /// Sensing schedule
    schedule: SensingSchedule,
}

impl RuvSenseAggregator {
    /// Process one complete TDMA cycle (all links measured)
    pub fn process_cycle(&mut self) -> Vec<TrackedPose> {
        // 1. Reconstruct enhanced CIR per link (ruvector-solver)
        let cirs: Vec<_> = self.link_buffers.iter()
            .map(|buf| reconstruct_cir(buf.latest_frame()))
            .collect();

        // 2. Coherence gate each link
        let coherent_links: Vec<_> = cirs.iter().enumerate()
            .filter(|(i, cir)| self.link_gates[*i].is_coherent(cir))
            .collect();

        // 3. Separate persons via cross-link correlation (ruvector-mincut)
        let person_clusters = self.person_separator.separate(&coherent_links);

        // 4. Per-person: fuse links, estimate pose, update track
        person_clusters.iter().map(|cluster| {
            let fused_csi = fuse_links_for_cluster(cluster, &self.link_geometry);
            let pose = self.pose_estimators[cluster.person_id].estimate(&fused_csi);
            self.trackers[cluster.person_id].update(pose)
        }).collect()
    }
}
```

---

## 8. Cognitum v1 Integration Path

For environments requiring higher fidelity than ESP32 can provide:

### 8.1 Cognitum as Baseband + Embedding Engine

Pair Cognitum v1 hardware with the RuvSense software stack:

1. **RF front end**: Cognitum's wider-bandwidth ADC captures more subcarriers
2. **Baseband processing**: Cognitum handles FFT and initial CSI extraction
3. **Embedding**: Run AETHER contrastive embedding (ADR-024) on extracted CSI
4. **Vector memory**: Feed embeddings into RuVector HNSW for fingerprint matching
5. **Coherence gating**: Apply RuvSense coherence gate to Cognitum's output

### 8.2 Advantage Over Pure ESP32

| Metric | ESP32 Mesh (RuvSense) | Cognitum + RuvSense |
|--------|----------------------|---------------------|
| Subcarriers | 114 (HT40) | 256+ (wideband front end) |
| Sampling rate | 100 Hz per link | 1000+ Hz |
| Phase noise | Consumer-grade | Research-grade |
| Cost per node | $10 | $200-500 (estimated) |
| Deployment | DIY mesh | Integrated unit |

The same RuvSense software stack runs on both — the only difference is the CSI input quality.

---

## 9. AETHER Embedding + RuVector Memory Integration

### 9.1 Contrastive CSI Embeddings for Stable Tracking

AETHER (ADR-024) produces 128-dimensional embeddings from CSI that encode:
- **Person identity**: Different people produce different embedding clusters
- **Pose state**: Similar poses cluster together regardless of environment
- **Temporal continuity**: Sequential frames trace smooth paths in embedding space

RuvSense uses these embeddings for **persistent person identification**:

```rust
/// Use AETHER embeddings for cross-session person identification.
/// When a person leaves and returns, their embedding matches stored profile.
pub struct EmbeddingIdentifier {
    /// HNSW index of known person embeddings
    person_index: HnswIndex,  // ruvector HNSW
    /// Similarity threshold for positive identification
    match_threshold: f32,  // default: 0.85 cosine similarity
    /// Exponential moving average of each person's embedding
    person_profiles: HashMap<PersonId, Vec<f32>>,
}

impl EmbeddingIdentifier {
    /// Identify a person from their current AETHER embedding.
    pub fn identify(&self, embedding: &[f32]) -> IdentifyResult {
        match self.person_index.search(embedding, 1) {
            Some((person_id, similarity)) if similarity >= self.match_threshold => {
                IdentifyResult::Known(person_id, similarity)
            }
            _ => IdentifyResult::NewPerson,
        }
    }

    /// Update a person's profile with new embedding (EMA).
    pub fn update_profile(&mut self, person_id: PersonId, embedding: &[f32]) {
        let profile = self.person_profiles.entry(person_id)
            .or_insert_with(|| embedding.to_vec());
        for (p, &e) in profile.iter_mut().zip(embedding.iter()) {
            *p = 0.95 * *p + 0.05 * e;
        }
        self.person_index.update(person_id, profile);
    }
}
```

### 9.2 Vector Graph Memory for Environment Learning

RuVector's graph capabilities enable the system to build a persistent model of the environment:

```
Environment Memory Graph:

    [Room A] ──has_layout──→ [Layout: 4AP, 4x5m]
        │                         │
        has_profile               has_geometry
        │                         │
        ▼                         ▼
    [CSI Profile A]           [AP₁: 0,0,2.5]
    (HNSW embedding)          [AP₂: 4,0,2.5]
        │                     [AP₃: 4,5,2.5]
        matched_person         [AP₄: 0,5,2.5]
        │
        ▼
    [Person₁ Profile]
    (AETHER embedding avg)
```

When the system enters a known room, it:
1. Matches the current CSI profile against stored room embeddings (HNSW)
2. Loads the room's geometry for MERIDIAN conditioning
3. Loads known person profiles for faster re-identification
4. Applies stored SONA LoRA weights for the environment

---

## 10. Fidelity Metric Definitions

### 10.1 Pose Jitter

```
Jitter_k = RMS(p_k[t] - p_k_smooth[t])
```

Where `p_k[t]` is keypoint k's position at time t, and `p_k_smooth[t]` is a 1-second Gaussian-filtered version. Measured in millimetres.

**Target:** Jitter < 30mm at torso keypoints (hips, shoulders, spine), < 50mm at limbs.

### 10.2 Multi-Person Separation

```
ID_switch_rate = n_identity_swaps / (n_persons × duration_seconds)
```

**Target:** 0 identity swaps over 10 minutes for 2 people. < 1 swap per 10 minutes for 4 people.

### 10.3 Small Motion Sensitivity

Measured as SNR of the breathing signal (0.15-0.5 Hz band) relative to noise floor:

```
SNR_breathing = 10 * log10(P_signal / P_noise) dB
```

**Target:** SNR > 10dB at 3m range for breathing, > 6dB at 1.5m for heartbeat.

### 10.4 Temporal Stability

```
Stability = max_t(|p_k[t] - p_k[t-Δ]|) for stationary subject
```

Measured over 10-minute windows with subject standing still.

**Target:** < 20mm drift over 10 minutes (static subject).

---

## 11. SOTA References and Grounding

### 11.1 Seminal Works

| Paper | Venue | Year | Key Contribution |
|-------|-------|------|-----------------|
| DensePose From WiFi (Geng et al.) | arXiv:2301.00250 | 2023 | CSI → UV body surface map |
| Person-in-WiFi 3D (Yan et al.) | CVPR 2024 | 2024 | Multi-person 3D pose from WiFi |
| PerceptAlign (Chen et al.) | arXiv:2601.12252 | 2026 | Geometry-conditioned cross-layout |
| AM-FM Foundation Model | arXiv:2602.11200 | 2026 | 9.2M CSI samples, 20 device types |
| X-Fi (Chen & Yang) | ICLR 2025 | 2025 | Modality-invariant foundation model |

### 11.2 Multistatic WiFi Sensing

| Paper | Venue | Year | Key Finding |
|-------|-------|------|-------------|
| SpotFi (Kotaru et al.) | SIGCOMM 2015 | 2015 | AoA estimation from CSI, sub-meter accuracy |
| Widar 3.0 (Zheng et al.) | MobiSys 2019 | 2019 | Domain-independent gesture via BVP |
| FarSense (Zeng et al.) | MobiCom 2019 | 2019 | CSI ratio for non-conjugate noise elimination |
| WiGesture (Abdelnasser et al.) | Pervasive 2016 | 2016 | Multi-AP gesture recognition, 96% accuracy |

### 11.3 Coherence and Stability

| Paper | Venue | Year | Key Finding |
|-------|-------|------|-------------|
| AdaPose (Zhou et al.) | IEEE IoT Journal 2024 | 2024 | Cross-site domain adaptation |
| DGSense (Zhou et al.) | arXiv:2502.08155 | 2025 | Virtual data generation for domain-invariant features |
| CAPC | IEEE OJCOMS 2024 | 2024 | Context-Aware Predictive Coding, 24.7% improvement |

### 11.4 Standards

| Standard | Status | Relevance |
|----------|--------|-----------|
| IEEE 802.11bf | Published 2024 | WLAN Sensing — defines sensing frames, roles, measurements |
| IEEE 802.11be (WiFi 7) | Finalized 2025 | 320 MHz channels, 3,984 subcarriers |
| IEEE 802.11bn (WiFi 8) | Draft | Sub-7 GHz + 45/60 GHz, native sensing |

### 11.5 ESP32 CSI Research

| Paper | Venue | Year | Key Finding |
|-------|-------|------|-------------|
| Gaiba & Bedogni | IEEE CCNC 2024 | 2024 | ESP32 human ID: 88.9-94.5% accuracy |
| Through-wall HAR | Springer 2023 | 2023 | ESP32 CSI: 18.5m range, 5 rooms |
| On-device DenseNet | MDPI Sensors 2025 | 2025 | ESP32-S3: 92.43% accuracy, 232ms |
| EMD augmentation | 2025 | 2025 | ESP32 CSI: 59.91% → 97.55% with augmentation |

---

## 12. Decision Questions

### Q1: Which fidelity metric matters most?

**Answer:** For the RuvSense acceptance test, **joint error + temporal stability** are primary. Multi-person separation is the secondary gate. Vital sign sensitivity is a bonus that validates small-motion detection but is not blocking.

Priority ordering:
1. Torso keypoint jitter < 30mm (directly validates DensePose quality)
2. Zero ID swaps over 10 min (validates tracking + re-ID pipeline)
3. 20 Hz update rate (validates multistatic fusion throughput)
4. Breathing SNR > 10dB at 3m (validates fine-motion sensitivity)

### Q2: Dedicated RF front end or commodity WiFi only?

**Answer:** **Start commodity-only (ESP32 mesh), with a clear upgrade path to dedicated RF.**

The ESP32 mesh is sufficient for the acceptance test based on existing research:
- ESP32 CSI human ID at 88.9-94.5% (single node)
- Through-wall HAR at 18.5m range
- On-device inference at 232ms

Multistatic mesh with 4 nodes should exceed these single-node results by providing 12 independent observations. If the acceptance test fails on ESP32, upgrade to Cognitum (§8) without changing the software stack.

---

## 13. Implementation Roadmap

### Phase 1: Multistatic Firmware (2 weeks)
- Modify ESP32 firmware for TDMA sensing schedule
- Add GPIO sync, link tagging, HT40 mode
- Test 4-node mesh with wired sync

### Phase 2: Aggregator Core (2 weeks)
- Implement `RuvSenseAggregator` in Rust
- Per-link ring buffers with `ruvector-temporal-tensor`
- UDP CSI collector with link demux

### Phase 3: Bandwidth Enhancement (1 week)
- Sparse CIR reconstruction via `ruvector-solver`
- Validate multipath separation improvement on recorded data

### Phase 4: Viewpoint Fusion (2 weeks)
- Geometry-aware link embedding (reuse MERIDIAN GeometryEncoder)
- Attention-based multi-link aggregation via `ruvector-attention`
- Cross-link correlation for person separation via `ruvector-mincut`

### Phase 5: Coherence Gating (1 week)
- Per-link coherence metric
- Gated update rule with SONA recalibration trigger
- Long-term stability test (24-hour continuous run)

### Phase 6: Integration + Acceptance Test (2 weeks)
- Wire into AETHER embedding + MERIDIAN domain adaptation
- Connect to ADR-026 tracking (Kalman + re-ID)
- Run acceptance test: 2 people, 20 Hz, 10 minutes, zero swaps

**Total: ~10 weeks from start to acceptance test.**

---

## 14. Relationship to Existing ADRs

| ADR | Relationship |
|-----|-------------|
| ADR-012 (ESP32 CSI Sensor Mesh) | **Extended**: RuvSense adds multistatic TDMA to single-AP CSI mesh |
| ADR-014 (SOTA Signal Processing) | **Used**: All signal processing algorithms applied per-link |
| ADR-016 (RuVector Integration) | **Extended**: New integration points for multi-link fusion |
| ADR-017 (RuVector Signal+MAT) | **Extended**: Coherence gating adds temporal stability layer |
| ADR-018 (ESP32 Dev Implementation) | **Modified**: Firmware gains TDMA schedule + HT40 |
| ADR-022 (Windows Enhanced Fidelity) | **Complementary**: RuvSense is the ESP32 equivalent |
| ADR-024 (AETHER Embeddings) | **Used**: Person identification via embedding similarity |
| ADR-026 (Survivor Track Lifecycle) | **Used**: Kalman tracking + re-ID for stable tracks |
| ADR-027 (MERIDIAN Generalization) | **Used**: GeometryEncoder, HardwareNormalizer, FiLM conditioning |

---

## 15. Conclusion

RuvSense achieves high-fidelity WiFi DensePose by exploiting three physical levers — bandwidth, frequency, and viewpoints — through a multistatic ESP32 mesh that implements a sensing-first RF mode on existing commodity silicon. The complete RuVector integration provides the algorithmic foundation for sparse CIR reconstruction (solver), multi-link attention fusion (attention), person separation (mincut), temporal compression (temporal-tensor), and coherence gating (attn-mincut).

The architecture is incrementally deployable: start with 2 nodes for basic improvement, scale to 4+ for full multistatic sensing. The same software stack runs on ESP32 mesh or Cognitum hardware, with only the CSI input interface changing.

**The winning move is not inventing new WiFi. It is making existing WiFi see better.**
