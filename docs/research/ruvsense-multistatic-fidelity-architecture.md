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

---

## Part II: The Persistent Field Model

*The most exotic capabilities come from treating RF as a persistent world model, not a momentary pose estimate.*

---

## 16. Beyond Pose: RF as Spatial Intelligence

Sections 1-15 treat WiFi as a pose estimator. That is the floor. The ceiling is treating the electromagnetic field as a **persistent, self-updating model of the physical world** — a model that remembers, predicts, and explains.

The shift: instead of asking "where are the keypoints right now?", ask "how has this room changed since yesterday, and what does that change mean?"

This requires three architectural upgrades:
1. **Field normal modes**: Model the room itself, not just the people in it
2. **Longitudinal memory**: Store structured embeddings over days/weeks via RuVector
3. **Coherence as reasoning**: Use coherence gating not just for quality control, but as a semantic signal — when coherence breaks, something meaningful happened

---

## 17. The Seven Exotic Capability Tiers

### Tier 1: Field Normal Modes

The room becomes the thing you model. You learn the stable electromagnetic baseline — the set of propagation paths, reflection coefficients, and interference patterns that exist when nobody is present. This is the **field normal mode**: the eigenstructure of the empty room's channel transfer function.

People and objects become **structured perturbations** to this baseline. A person entering the room does not create a new signal — they perturb existing modes. The perturbation has structure: it is spatially localized (the person is somewhere), spectrally colored (different body parts affect different subcarriers), and temporally smooth (people move continuously).

```rust
/// Field Normal Mode — the room's electromagnetic eigenstructure
pub struct FieldNormalMode {
    /// Baseline CSI per link (measured during empty-room calibration)
    pub baseline: Vec<Vec<Complex<f32>>>,  // [n_links × n_subcarriers]
    /// Principal components of baseline variation (temperature, humidity)
    pub environmental_modes: Vec<Vec<f32>>,  // [n_modes × n_subcarriers]
    /// Eigenvalues: how much variance each mode explains
    pub mode_energies: Vec<f32>,
    /// Timestamp of last baseline update
    pub calibrated_at: u64,
    /// Room geometry hash (invalidate if nodes move)
    pub geometry_hash: u64,
}

impl FieldNormalMode {
    /// Compute perturbation: subtract baseline, project out environmental modes.
    /// What remains is body-caused change.
    pub fn extract_perturbation(
        &self,
        current_csi: &[Vec<Complex<f32>>],
    ) -> Vec<Vec<f32>> {
        current_csi.iter().zip(self.baseline.iter()).map(|(curr, base)| {
            let delta: Vec<f32> = curr.iter().zip(base.iter())
                .map(|(c, b)| (c - b).norm())
                .collect();

            // Project out environmental modes (slow drift)
            let mut residual = delta.clone();
            for mode in &self.environmental_modes {
                let projection: f32 = residual.iter().zip(mode.iter())
                    .map(|(r, m)| r * m).sum();
                for (r, m) in residual.iter_mut().zip(mode.iter()) {
                    *r -= projection * m;
                }
            }
            residual  // Pure body perturbation
        }).collect()
    }
}
```

**Why this matters:** The field normal mode enables a building that **senses itself**. Changes are explained as deltas from baseline. A new chair is a permanent mode shift. A person walking is a transient perturbation. A door opening changes specific path coefficients. The system does not need to be told what changed — it can decompose the change into structural categories.

**RuVector integration:** `ruvector-solver` fits the environmental mode matrix via low-rank SVD. `ruvector-temporal-tensor` stores the baseline history with adaptive quantization.

### Tier 2: Coarse RF Tomography

With multiple viewpoints, you can infer a low-resolution 3D occupancy volume, not just skeleton keypoints.

```
          Node A
          ╱    ╲
        ╱        ╲        Link A→B passes through voxel (2,3)
      ╱            ╲      Link A→C passes through voxels (2,3), (3,4)
    ╱    ┌─────┐     ╲    Link B→D passes through voxel (3,3)
  ╱      │ occ │       ╲
Node B   │upa- │   Node C    From 12 link attenuations,
  ╲      │ ncy │       ╱    solve for voxel occupancy
    ╲    └─────┘     ╱      using ruvector-solver (L1)
      ╲            ╱
        ╲        ╱
          ╲    ╱
          Node D
```

This is not a camera. It is a **probabilistic density field** that tells you where mass is, not what it looks like. It stays useful in darkness, smoke, occlusion, and clutter.

```rust
/// Coarse RF tomography — 3D occupancy from link attenuations
pub struct RfTomographer {
    /// 3D voxel grid dimensions
    pub grid_dims: [usize; 3],  // e.g., [8, 10, 4] for 4m × 5m × 2m at 0.5m resolution
    /// Voxel size in metres
    pub voxel_size: f32,  // 0.5m
    /// Projection matrix: which voxels does each link pass through
    /// Shape: [n_links × n_voxels], sparse
    pub projection: Vec<Vec<(usize, f32)>>,  // (voxel_idx, path_weight)
    /// Regularization strength (sparsity prior)
    pub lambda: f32,  // default: 0.01
}

impl RfTomographer {
    /// Reconstruct occupancy volume from link perturbation magnitudes.
    /// Uses ruvector-solver for L1-regularized least squares.
    pub fn reconstruct(
        &self,
        link_perturbations: &[f32],  // [n_links], magnitude of body perturbation
    ) -> Vec<f32> {
        // Sparse tomographic inversion: find occupancy x such that
        // ||Ax - b||₂ + λ||x||₁ → min
        // where A is projection matrix, b is link perturbations
        let n_voxels = self.grid_dims.iter().product();
        let solver = NeumannSolver::new(n_voxels, self.lambda);
        solver.solve_sparse(&self.projection, link_perturbations)
    }
}
```

**Resolution:** With 4 nodes (12 links) and 0.5m voxels, the tomographic grid is 8×10×4 = 320 voxels. 12 measurements for 320 unknowns is severely underdetermined, but L1 regularization exploits sparsity — typically only 5-15 voxels are occupied by a person. At 8+ nodes (56 links), resolution improves to ~0.25m.

### Tier 3: Intention Lead Signals

Subtle pre-movement dynamics appear **before visible motion**. Lean, weight shift, arm tension, center-of-mass displacement. These are not noise — they are the body's preparatory phase for action.

With contrastive embeddings plus temporal memory, you can **predict action onset** early enough to drive safety and robotics applications.

```rust
/// Intention lead signal detector.
/// Monitors the embedding trajectory for pre-movement patterns.
pub struct IntentionDetector {
    /// Temporal window of AETHER embeddings (last 2 seconds at 20 Hz = 40 frames)
    pub embedding_history: VecDeque<Vec<f32>>,  // [40 × 128]
    /// Trained classifiers for pre-movement signatures
    pub lean_classifier: MicroClassifier,
    pub weight_shift_classifier: MicroClassifier,
    pub reach_intent_classifier: MicroClassifier,
    /// Lead time budget: how far ahead we predict (ms)
    pub max_lead_ms: u32,  // default: 500ms
}

impl IntentionDetector {
    /// Detect pre-movement intention from embedding trajectory.
    /// Returns predicted action and time-to-onset.
    pub fn detect(&self) -> Option<IntentionSignal> {
        let trajectory = self.compute_trajectory_features();

        // Pre-movement signatures:
        // 1. Embedding velocity increases before visible motion
        // 2. Embedding curvature changes (trajectory bends toward action cluster)
        // 3. Subcarrier variance pattern matches stored pre-action templates

        let lean = self.lean_classifier.score(&trajectory);
        let shift = self.weight_shift_classifier.score(&trajectory);
        let reach = self.reach_intent_classifier.score(&trajectory);

        let best = [
            (lean, IntentionType::Lean),
            (shift, IntentionType::WeightShift),
            (reach, IntentionType::Reach),
        ].iter().max_by(|a, b| a.0.partial_cmp(&b.0).unwrap())?;

        if best.0 > 0.7 {
            Some(IntentionSignal {
                intent_type: best.1,
                confidence: best.0,
                estimated_lead_ms: self.estimate_lead_time(&trajectory),
            })
        } else {
            None
        }
    }
}
```

**How much lead time is realistic?** Research on anticipatory postural adjustments shows 200-500ms of preparatory muscle activation before voluntary movement. At 20 Hz with 2-second embedding history, we observe 4-10 frames of pre-movement dynamics. Contrastive pre-training teaches the encoder to separate pre-movement from noise.

### Tier 4: Longitudinal Biomechanics Drift

Not diagnosis. **Drift.** You build a personal baseline for gait symmetry, stability, breathing regularity, and micro-tremor, then detect meaningful deviation over days.

RuVector is the memory and the audit trail.

```rust
/// Personal biomechanics baseline — stores longitudinal embedding statistics
pub struct PersonalBaseline {
    pub person_id: PersonId,
    /// Per-metric rolling statistics (Welford online algorithm)
    pub gait_symmetry: WelfordStats,      // left-right step ratio
    pub stability_index: WelfordStats,    // center-of-mass sway area
    pub breathing_regularity: WelfordStats, // coefficient of variation of breath interval
    pub micro_tremor: WelfordStats,       // high-frequency (4-12 Hz) limb oscillation power
    pub activity_level: WelfordStats,     // average movement energy per hour
    /// Embedding centroid (EMA, 128-dim)
    pub embedding_centroid: Vec<f32>,
    /// Days of data accumulated
    pub observation_days: u32,
    /// Last update timestamp
    pub updated_at: u64,
}

/// Drift detection result
pub struct DriftReport {
    pub person_id: PersonId,
    pub metric: DriftMetric,
    /// How many standard deviations from personal baseline
    pub z_score: f32,
    /// Direction of change
    pub direction: DriftDirection,  // Increasing / Decreasing
    /// Duration over which drift occurred
    pub window_days: u32,
    /// Confidence that this is a real change (not noise)
    pub confidence: f32,
    /// Supporting evidence: stored embeddings bracketing the change
    pub evidence_embeddings: Vec<(u64, Vec<f32>)>,
}

pub enum DriftMetric {
    GaitSymmetry,
    StabilityIndex,
    BreathingRegularity,
    MicroTremor,
    ActivityLevel,
}
```

**What can be detected (signals, not diagnoses):**

| Signal | Physiological Proxy | Detectable Via |
|--------|---------------------|---------------|
| Gait symmetry shift | Asymmetric loading, injury compensation | Left-right step timing ratio from pose tracks |
| Stability decrease | Balance degradation | CoM sway area increase (static standing) |
| Breathing irregularity | Respiratory pattern change | Coefficient of variation in breath interval |
| Micro-tremor onset | Involuntary oscillation | 4-12 Hz power in limb keypoint FFT |
| Activity decline | Reduced mobility | Hourly movement energy integral |

**The output:** "Your movement symmetry has shifted 18 percent over 14 days." That is actionable without being diagnostic.

**RuVector integration:** `ruvector-temporal-tensor` stores compressed daily summaries. HNSW indexes embeddings for fast similarity search across the longitudinal record. `ruvector-attention` weights which metrics contribute to the overall drift score.

### Tier 5: Cross-Room Continuity Without Optics

Environment fingerprints plus track graphs let you carry identity continuity across spaces. You can know who moved where without storing images.

```rust
/// Cross-room identity continuity via environment fingerprinting
pub struct CrossRoomTracker {
    /// Per-room AETHER environment fingerprints (HNSW indexed)
    pub room_index: HnswIndex,
    /// Per-person embedding profiles (HNSW indexed)
    pub person_index: HnswIndex,
    /// Transition graph: room_a → room_b with timestamps
    pub transitions: Vec<RoomTransition>,
    /// Active tracks per room
    pub active_tracks: HashMap<RoomId, Vec<TrackId>>,
}

pub struct RoomTransition {
    pub person_id: PersonId,
    pub from_room: RoomId,
    pub to_room: RoomId,
    pub exit_time: u64,
    pub entry_time: u64,
    /// Embedding at exit (for matching at entry)
    pub exit_embedding: Vec<f32>,
}

impl CrossRoomTracker {
    /// When a person appears in a new room, match against recent exits.
    pub fn match_entry(
        &self,
        room_id: RoomId,
        entry_embedding: &[f32],
        entry_time: u64,
    ) -> Option<PersonId> {
        // Search recent exits (within 60 seconds) from adjacent rooms
        let candidates: Vec<_> = self.transitions.iter()
            .filter(|t| entry_time - t.exit_time < 60_000_000) // 60s window
            .collect();

        // HNSW similarity match
        let best = candidates.iter()
            .map(|t| {
                let sim = cosine_similarity(&t.exit_embedding, entry_embedding);
                (t, sim)
            })
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        match best {
            Some((transition, sim)) if sim > 0.80 => Some(transition.person_id),
            _ => None,
        }
    }
}
```

**Privacy advantage:** No images are stored. The system stores 128-dimensional embeddings (not reconstructable to appearance) and structural transition events. Identity is established by **behavioral consistency**, not visual recognition.

### Tier 6: Invisible Interaction Layer

A room becomes an interface. Multi-user gesture control that works through clothing, in darkness, with line-of-sight blocked.

The key insight: the same multistatic CSI pipeline that estimates pose can detect **gestural micro-patterns** when the pose is held relatively still. A hand wave, a pointing gesture, a beckoning motion — all produce characteristic CSI perturbation signatures that are person-localized (thanks to the multi-person separator) and geometry-invariant (thanks to MERIDIAN conditioning).

```rust
/// Gesture recognition from multistatic CSI
pub struct GestureRecognizer {
    /// Per-gesture template embeddings (trained contrastively)
    pub gesture_templates: HashMap<GestureType, Vec<f32>>,  // [128-dim each]
    /// Temporal window for gesture detection
    pub window_frames: usize,  // 20 frames = 1 second at 20 Hz
    /// Minimum confidence for gesture trigger
    pub trigger_threshold: f32,  // default: 0.8
}

pub enum GestureType {
    Wave,
    Point,
    Beckon,
    PushAway,
    CircularMotion,
    StandUp,
    SitDown,
    Custom(String),
}
```

**Multi-user:** Because person separation (§5.4) already isolates each person's CSI contribution, gesture detection runs independently per person. Two people can gesture simultaneously without interference.

### Tier 7: Adversarial and Spoofing Detection

You can detect when the signal looks **physically impossible** given the room model. Coherence gating becomes a **security primitive**, not just a quality check.

```rust
/// Adversarial signal detector — identifies physically impossible CSI
pub struct AdversarialDetector {
    /// Room field normal modes (baseline)
    pub field_model: FieldNormalMode,
    /// Physical constraints: maximum possible CSI change per frame
    pub max_delta_per_frame: f32,  // based on max human velocity
    /// Subcarrier correlation structure (from room geometry)
    pub expected_correlation: Vec<Vec<f32>>,  // [n_sub × n_sub]
}

impl AdversarialDetector {
    /// Check if a CSI frame is physically plausible.
    pub fn check(&self, frame: &[Complex<f32>], prev_frame: &[Complex<f32>]) -> SecurityVerdict {
        // 1. Rate-of-change check: no human can cause faster CSI change
        let delta = frame_delta_magnitude(frame, prev_frame);
        if delta > self.max_delta_per_frame {
            return SecurityVerdict::RateViolation(delta);
        }

        // 2. Correlation structure check: body perturbations have specific
        //    cross-subcarrier correlation patterns (from Fresnel zone geometry).
        //    Injected signals typically lack this structure.
        let observed_corr = compute_correlation(frame);
        let structure_score = correlation_similarity(&observed_corr, &self.expected_correlation);
        if structure_score < 0.5 {
            return SecurityVerdict::StructureViolation(structure_score);
        }

        // 3. Multi-link consistency: a real body affects multiple links
        //    consistently with its position. A spoofed signal on one link
        //    will be inconsistent with other links.
        // (Handled at the aggregator level, not per-frame)

        SecurityVerdict::Plausible
    }
}

pub enum SecurityVerdict {
    Plausible,
    RateViolation(f32),
    StructureViolation(f32),
    MultiLinkInconsistency(Vec<usize>),  // which links disagree
}
```

**Why multistatic helps security:** To spoof a single-link system, an attacker injects a signal into one receiver. To spoof a multistatic mesh, the attacker must simultaneously inject consistent signals into all receivers — signals that are geometrically consistent with a fake body position. This is physically difficult because each receiver sees a different projection.

---

## 18. Signals, Not Diagnoses

### 18.1 The Regulatory Boundary

RF sensing can capture **biophysical proxies**:
- Breathing rate variability
- Gait asymmetry
- Posture instability
- Micro-tremor
- Activity level drift
- Sleep movement patterns

**Diagnosis** requires:
1. Clinical gold standard validation
2. Controlled datasets with IRB approval
3. Regulatory approval (FDA Class II or III)
4. Extremely low false positive and false negative rates

Without that, you are in **"risk signal detection"**, not medical diagnosis.

### 18.2 The Three Levels

| Level | What It Is | What It Says | Regulatory Load |
|-------|-----------|-------------|-----------------|
| **Level 1: Physiological Monitoring** | Respiratory rate trends, movement stability index, fall likelihood score | "Your breathing rate averaged 18.3 BPM today" | Consumer wellness (low) |
| **Level 2: Drift Detection** | Change from personal baseline, early anomaly detection | "Your gait symmetry shifted 18% over 14 days" | Consumer wellness (low) |
| **Level 3: Condition Risk Correlation** | Pattern consistent with respiratory distress, motor instability | "Pattern consistent with increased fall risk" | Clinical decision support (medium-high) |

**What you never say:**
- "You have Parkinson's."
- "You have heart failure."
- "You have Alzheimer's."

### 18.3 The Defensible Pipeline

```
RF (CSI)
  → AETHER contrastive embedding
    → RuVector longitudinal memory
      → Coherence-gated drift detection
        → Risk flag with traceable evidence
```

That gives you: *"Your movement symmetry has shifted 18 percent over 14 days."*

That is actionable without being diagnostic. The evidence chain (stored embeddings, drift statistics, coherence scores) is fully traceable and auditable via RuVector's graph memory.

### 18.4 Path to Regulated Claims

If you ever want to make diagnostic claims:

| Requirement | Status | Effort |
|-------------|--------|--------|
| IRB-approved clinical studies | Not started | 6-12 months |
| Clinically labeled datasets | Not started | Requires clinical partner |
| Statistical power analysis | Feasible once data exists | 1-2 months |
| FDA 510(k) or De Novo pathway | Not started | 12-24 months + legal |
| CE marking (EU MDR) | Not started | 12-18 months |

The opportunity is massive, but the regulatory surface explodes the moment you use the word "diagnosis."

### 18.5 The Decision: Device Classification

| Class | Example | Regulatory Path | Time to Market |
|-------|---------|----------------|----------------|
| **Consumer wellness** | Breathing rate tracker, activity monitor | Self-certification, FCC Part 15 only | 3-6 months |
| **Clinical decision support** | Fall risk alert, respiratory distress pattern | FDA Class II 510(k) or De Novo | 12-24 months |
| **Regulated medical device** | Diagnostic tool for specific conditions | FDA Class II/III, clinical trials | 24-48 months |

**Recommendation:** Start as consumer wellness device with Level 1-2 signals. Build longitudinal dataset. Pursue FDA pathway only after 12+ months of real-world data proves statistical power.

---

## 19. Appliance Product Categories

Treating RF spatial intelligence as a persistent field model enables appliances that were not possible before because they required cameras, wearables, or invasive sensors.

### 19.1 Invisible Guardian

**Wall-mounted unit that models gait, fall dynamics, and breathing baselines without optics.**

| Attribute | Specification |
|-----------|--------------|
| Form factor | Wall puck, 80mm diameter |
| Nodes | 4 ESP32-S3 pucks per room |
| Processing | Central hub (RPi 5 or x86) |
| Power | PoE or USB-C |
| Storage | Embeddings + deltas only, no images |
| Privacy | No camera, no microphone, no reconstructable data |
| Output | Risk flags, drift alerts, occupancy timeline |
| Vertical | Elderly care, independent living, home health |

**Acceptance test:** Runs locally for 30 days, no camera, detects meaningful environmental or behavioral drift with less than 5% false alarms.

### 19.2 Spatial Digital Twin Node

**Small appliance that builds a live electromagnetic twin of a room.**

Tracks occupancy flow, environmental changes, and structural anomalies. Facilities teams get a time-indexed behavioral map of space usage without video storage risk.

| Attribute | Specification |
|-----------|--------------|
| Output | Occupancy heatmap, flow vectors, dwell time, anomaly events |
| Data retention | 30-day rolling summary, GDPR-compliant |
| Integration | MQTT/REST API for BMS and CAFM systems |
| Vertical | Smart buildings, workplace analytics, retail |

### 19.3 Collective Behavior Engine

**Real-time crowd density, clustering, agitation patterns, and flow bottlenecks.**

| Attribute | Specification |
|-----------|--------------|
| Scale | 10-100 people per zone |
| Metrics | Density, flow velocity, dwell clusters, evacuation rate |
| Latency | <1s for crowd-level metrics |
| Vertical | Fire safety, event management, transit, retail |

### 19.4 RF Interaction Surface

**Turn any room into a gesture interface. No cameras. Multi-user. Works in darkness or smoke.**

Lighting, media, robotics respond to posture and intent.

| Attribute | Specification |
|-----------|--------------|
| Gestures | Wave, point, beckon, push, circle + custom |
| Multi-user | Up to 4 simultaneous users |
| Latency | <100ms gesture recognition |
| Vertical | Smart home, hospitality, accessibility, gaming |

### 19.5 Pre-Incident Drift Monitor

**Detect subtle changes in movement patterns that precede falls or medical instability.**

Not diagnosis. Early warning via longitudinal embedding drift.

| Attribute | Specification |
|-----------|--------------|
| Metrics | Gait symmetry, stability index, breathing regularity, micro-tremor |
| Baseline | 7-day calibration period per person |
| Alert | When any metric drifts >2σ from personal baseline for >3 days |
| Evidence | Stored embedding trajectory + statistical report |
| Vertical | Elderly care, rehabilitation, occupational health |

### 19.6 Cognitum Nervous System Appliance

For the premium lane: always-on, local, coherence-gated, storing structured memory in RuVector.

This appliance was never possible before because we did not have:
- Small edge embedding models (AETHER on ESP32-S3 or Cognitum)
- Persistent vector graph memory (RuVector with HNSW)
- Cheap multistatic RF (ESP32 mesh at $73-91)

---

## 20. Extended Acceptance Tests

### 20.1 Pose Fidelity (Tier 0 — ADR-029)

Two people in a room, 20 Hz, stable tracks for 10 minutes with no identity swaps and low jitter in the torso keypoints.

### 20.2 Longitudinal Stability (Tier 1-4)

**Seven-day run, no manual tuning.** The system:
1. Flags one real environmental change (furniture moved, door state changed)
2. Flags one real human drift event (gait asymmetry shift, breathing pattern change)
3. Produces a traceable explanation using stored embeddings plus graph constraints
4. Zero false alarms on days with no real change

### 20.3 Appliance Validation (Tier 5-7)

**Thirty-day local run, no camera.** The system:
1. Detects meaningful environmental or behavioral drift
2. Less than 5% false alarm rate
3. Provides traceable evidence chain for every alert
4. Operates autonomously — no manual calibration after initial setup

---

## 21. Decision Questions (Exotic Tier)

### Q3: Which exotic tier first?

**Recommendation: Field normal modes (Tier 1).**

Rationale:
- It is the foundation for everything else. Without a room baseline, you cannot detect drift (Tier 4), cross-room transitions (Tier 5), or adversarial signals (Tier 7).
- It requires no new hardware — just a calibration phase during empty-room periods.
- It immediately improves pose quality by separating environmental from body-caused CSI variation.
- It uses `ruvector-solver` (SVD) and `ruvector-temporal-tensor` (baseline storage), both already integrated.

Second priority: **Longitudinal biomechanics drift (Tier 4)**, because it unlocks the Invisible Guardian and Pre-Incident Drift Monitor appliance categories.

Third priority: **Cross-room continuity (Tier 5)**, because it unlocks the Spatial Digital Twin Node.

### Q4: Commodity ESP32 mesh only, or premium Cognitum lane too?

**Recommendation: ESP32 mesh as the primary development and validation platform. Design the software abstraction layer so Cognitum can slot in as a premium SKU without code changes.**

The ESP32 mesh ($73-91) proves the algorithms. The Cognitum lane ($500-1000) proves the fidelity ceiling. Both share the same RuvSense aggregator, AETHER embeddings, and RuVector memory. The only difference is the CSI input quality.

### Q5: Consumer wellness, clinical decision support, or regulated medical device?

**Recommendation: Consumer wellness device first.** Build the longitudinal dataset. Pursue clinical decision support after 12 months of real-world data proves statistical power. Do not attempt regulated medical device claims without a clinical partner and IRB approval.

---

## 22. Conclusion (Extended)

RuvSense is not a pose estimator. It is a **spatial intelligence platform** built on the insight that WiFi RF is a persistent, self-updating model of the physical world.

The architecture decomposes into three layers:

| Layer | Capability | Timeframe |
|-------|-----------|-----------|
| **Pose** (§1-15) | Multistatic DensePose at 20 Hz, <30mm jitter, zero ID swaps | 10 weeks |
| **Field** (§16-17) | Room modeling, drift detection, intention signals, tomography | +8 weeks |
| **Appliance** (§19) | Product categories: Guardian, Digital Twin, Interaction Surface | +12 weeks |

Each layer builds on the one below. The complete stack — from ESP32 NDP injection to 30-day autonomous drift monitoring — uses no cameras, stores no images, and runs on $73-91 of commodity hardware.

RuVector provides the algorithmic spine: solving, attention, graph partitioning, temporal compression, and coherence gating. AETHER provides the embedding space. MERIDIAN provides domain generalization. The result is a system that remembers rooms, recognizes people, detects drift, and explains change — all through WiFi.

**You can detect signals, not diagnoses. That distinction matters legally, ethically, and technically. But the signals are rich enough to build products that were never possible before.**
