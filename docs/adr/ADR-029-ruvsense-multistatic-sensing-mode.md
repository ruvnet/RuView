# ADR-029: Project RuvSense -- Sensing-First RF Mode for Multistatic WiFi DensePose

| Field | Value |
|-------|-------|
| **Status** | Proposed |
| **Date** | 2026-03-02 |
| **Deciders** | ruv |
| **Codename** | **RuvSense** -- RuVector-Enhanced Sensing for Multistatic Fidelity |
| **Relates to** | ADR-012 (ESP32 Mesh), ADR-014 (SOTA Signal Processing), ADR-016 (RuVector Training), ADR-017 (RuVector Signal+MAT), ADR-018 (ESP32 Implementation), ADR-024 (AETHER Embeddings), ADR-026 (Survivor Track Lifecycle), ADR-027 (MERIDIAN Generalization) |

---

## 1. Context

### 1.1 The Fidelity Gap

Current WiFi-DensePose achieves functional pose estimation from a single ESP32 AP, but three fidelity metrics prevent production deployment:

| Metric | Current (Single ESP32) | Required (Production) | Root Cause |
|--------|------------------------|----------------------|------------|
| Torso keypoint jitter | ~15cm RMS | <3cm RMS | Single viewpoint, 20 MHz bandwidth, no temporal smoothing |
| Multi-person separation | Fails >2 people, frequent ID swaps | 4+ people, zero swaps over 10 min | Underdetermined with 1 TX-RX link; no person-specific features |
| Small motion sensitivity | Gross movement only | Breathing at 3m, heartbeat at 1.5m | Insufficient phase sensitivity at 2.4 GHz; noise floor too high |
| Update rate | ~10 Hz effective | 20 Hz | Single-channel serial CSI collection |
| Temporal stability | Drifts within hours | Stable over days | No coherence gating; model absorbs environmental drift |

### 1.2 The Insight: Sensing-First RF Mode on Existing Silicon

You do not need to invent a new WiFi standard. The winning move is a **sensing-first RF mode** that rides on existing silicon (ESP32-S3), existing bands (2.4/5 GHz), and existing regulations (802.11n NDP frames). The fidelity improvement comes from three physical levers:

1. **Bandwidth**: Channel-hopping across 2.4 GHz channels 1/6/11 triples effective bandwidth from 20 MHz to 60 MHz, 3x multipath separation
2. **Carrier frequency**: Dual-band sensing (2.4 + 5 GHz) doubles phase sensitivity to small motion
3. **Viewpoints**: Multistatic ESP32 mesh (4 nodes = 12 TX-RX links) provides 360-degree geometric diversity

### 1.3 Acceptance Test

**Two people in a room, 20 Hz update rate, stable tracks for 10 minutes with no identity swaps and low jitter in the torso keypoints.**

Quantified:
- Torso keypoint jitter < 30mm RMS (hips, shoulders, spine)
- Zero identity swaps over 600 seconds (12,000 frames)
- 20 Hz output rate (50 ms cycle time)
- Breathing SNR > 10dB at 3m (validates small-motion sensitivity)

---

## 2. Decision

### 2.1 Architecture Overview

Implement RuvSense as a new bounded context within `wifi-densepose-signal`, consisting of 6 modules:

```
wifi-densepose-signal/src/ruvsense/
├── mod.rs              // Module exports, RuvSense pipeline orchestrator
├── multiband.rs        // Multi-band CSI frame fusion (§2.2)
├── phase_align.rs      // Cross-channel phase alignment (§2.3)
├── multistatic.rs      // Multi-node viewpoint fusion (§2.4)
├── coherence.rs        // Coherence metric computation (§2.5)
├── coherence_gate.rs   // Gated update policy (§2.6)
└── pose_tracker.rs     // 17-keypoint Kalman tracker with re-ID (§2.7)
```

### 2.2 Channel-Hopping Firmware (ESP32-S3)

Modify the ESP32 firmware (`firmware/esp32-csi-node/main/csi_collector.c`) to cycle through non-overlapping channels at configurable dwell times:

```c
// Channel hop table (populated from NVS at boot)
static uint8_t s_hop_channels[6] = {1, 6, 11, 36, 40, 44};
static uint8_t s_hop_count = 3;   // default: 2.4 GHz only
static uint32_t s_dwell_ms = 50;  // 50ms per channel
```

At 100 Hz raw CSI rate with 50 ms dwell across 3 channels, each channel yields ~33 frames/second. The existing ADR-018 binary frame format already carries `channel_freq_mhz` at offset 8, so no wire format change is needed.

> **Note (Issue #127 fix):** In promiscuous mode, CSI callbacks fire 100-500+ times/sec — far exceeding the channel dwell rate. The firmware now rate-limits UDP sends to 50 Hz and applies a 100 ms ENOMEM backoff if lwIP buffers are exhausted. This is essential for stable channel hopping under load.

**NDP frame injection:** `esp_wifi_80211_tx()` injects deterministic Null Data Packet frames (preamble-only, no payload, ~24 us airtime) at GPIO-triggered intervals. This is sensing-first: the primary RF emission purpose is CSI measurement, not data communication.

### 2.3 Multi-Band Frame Fusion

Aggregate per-channel CSI frames into a wideband virtual snapshot:

```rust
/// Fused multi-band CSI from one node at one time slot.
pub struct MultiBandCsiFrame {
    pub node_id: u8,
    pub timestamp_us: u64,
    /// One canonical-56 row per channel, ordered by center frequency.
    pub channel_frames: Vec<CanonicalCsiFrame>,
    /// Center frequencies (MHz) for each channel row.
    pub frequencies_mhz: Vec<u32>,
    /// Cross-channel coherence score (0.0-1.0).
    pub coherence: f32,
}
```

Cross-channel phase alignment uses `ruvector-solver::NeumannSolver` to solve for the channel-dependent phase rotation introduced by the ESP32 local oscillator during channel hops. The system:

```
[Φ₁, Φ₆, Φ₁₁] = [Φ_body + δ₁, Φ_body + δ₆, Φ_body + δ₁₁]
```

NeumannSolver fits the `δ` offsets from the static subcarrier components (which should have zero body-caused phase shift), then removes them.

### 2.4 Multistatic Viewpoint Fusion

With N ESP32 nodes, collect N `MultiBandCsiFrame` per time slot and fuse with geometric diversity:

**TDMA Sensing Schedule (4 nodes):**

| Slot | TX | RX₁ | RX₂ | RX₃ | Duration |
|------|-----|-----|-----|-----|----------|
| 0 | Node A | B | C | D | 4 ms |
| 1 | Node B | A | C | D | 4 ms |
| 2 | Node C | A | B | D | 4 ms |
| 3 | Node D | A | B | C | 4 ms |
| 4 | -- | Processing + fusion | | | 30 ms |
| **Total** | | | | | **50 ms = 20 Hz** |

Synchronization: GPIO pulse from aggregator node at cycle start. Clock drift at ±10ppm over 50 ms is ~0.5 us, well within the 1 ms guard interval.

**Cross-node fusion** uses `ruvector-attn-mincut::attn_mincut` where time-frequency cells from different nodes attend to each other. Cells showing correlated motion energy across nodes (body reflection) are amplified; cells with single-node energy (local multipath artifact) are suppressed.

**Multi-person separation** via `ruvector-mincut::DynamicMinCut`:

1. Build cross-link temporal correlation graph (nodes = TX-RX links, edges = correlation coefficient)
2. `DynamicMinCut` partitions into K clusters (one per detected person)
3. Attention fusion (§5.3 of research doc) runs independently per cluster

### 2.5 Coherence Metric

Per-link coherence quantifies consistency with recent history:

```rust
pub fn coherence_score(
    current: &[f32],
    reference: &[f32],
    variance: &[f32],
) -> f32 {
    current.iter().zip(reference.iter()).zip(variance.iter())
        .map(|((&c, &r), &v)| {
            let z = (c - r).abs() / v.sqrt().max(1e-6);
            let weight = 1.0 / (v + 1e-6);
            ((-0.5 * z * z).exp(), weight)
        })
        .fold((0.0, 0.0), |(sc, sw), (c, w)| (sc + c * w, sw + w))
        .pipe(|(sc, sw)| sc / sw)
}
```

The static/dynamic decomposition uses `ruvector-solver` to separate environmental drift (slow, global) from body motion (fast, subcarrier-specific).

### 2.6 Coherence-Gated Update Policy

```rust
pub enum GateDecision {
    /// Coherence > 0.85: Full Kalman measurement update
    Accept(Pose),
    /// 0.5 < coherence < 0.85: Kalman predict only (3x inflated noise)
    PredictOnly,
    /// Coherence < 0.5: Reject measurement entirely
    Reject,
    /// >10s continuous low coherence: Trigger SONA recalibration (ADR-005)
    Recalibrate,
}
```

When `Recalibrate` fires:
1. Freeze output at last known good pose
2. Collect 200 frames (10s) of unlabeled CSI
3. Run AETHER contrastive TTT (ADR-024) to adapt encoder
4. Update SONA LoRA weights (ADR-005), <1ms per update
5. Resume sensing with adapted model

### 2.7 Pose Tracker (17-Keypoint Kalman with Re-ID)

Lift the Kalman + lifecycle + re-ID infrastructure from `wifi-densepose-mat/src/tracking/` (ADR-026) into the RuvSense bounded context, extended for 17-keypoint skeletons:

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| State dimension | 6 per keypoint (x,y,z,vx,vy,vz) | Constant-velocity model |
| Process noise σ_a | 0.3 m/s² | Normal walking acceleration |
| Measurement noise σ_obs | 0.08 m | Target <8cm RMS at torso |
| Mahalanobis gate | χ²(3) = 9.0 | 3σ ellipsoid (same as ADR-026) |
| Birth hits | 2 frames (100ms at 20Hz) | Reject single-frame noise |
| Loss misses | 5 frames (250ms) | Brief occlusion tolerance |
| Re-ID feature | AETHER 128-dim embedding | Body-shape discriminative (ADR-024) |
| Re-ID window | 5 seconds | Sufficient for crossing recovery |

**Track assignment** uses `ruvector-mincut`'s `DynamicPersonMatcher` (already integrated in `metrics.rs`, ADR-016) with joint position + embedding cost:

```
cost(track_i, det_j) = 0.6 * mahalanobis(track_i, det_j.position)
                      + 0.4 * (1 - cosine_sim(track_i.embedding, det_j.embedding))
```

---

## 3. GOAP Integration Plan (Goal-Oriented Action Planning)

### 3.1 Action Dependency Graph

```
Phase 1: Foundation
  Action 1: Channel-Hopping Firmware ──────────────────────┐
      │                                                      │
      v                                                      │
  Action 2: Multi-Band Frame Fusion ──→ Action 6: Coherence │
      │                                  Metric              │
      v                                    │                 │
  Action 3: Multistatic Mesh              v                 │
      │                              Action 7: Coherence    │
      v                                  Gate               │
Phase 2: Tracking                         │                 │
  Action 4: Pose Tracker ←────────────────┘                 │
      │                                                      │
      v                                                      │
  Action 5: End-to-End Pipeline @ 20 Hz ←────────────────────┘
      │
      v
Phase 4: Hardening
  Action 8: AETHER Track Re-ID
      │
      v
  Action 9: ADR-029 Documentation (this document)
```

### 3.2 Cost and RuVector Mapping

| # | Action | Cost | Preconditions | RuVector Crates | Effects |
|---|--------|------|---------------|-----------------|---------|
| 1 | Channel-hopping firmware | 4/10 | ESP32 firmware exists | None (pure C) | `bandwidth_extended = true` |
| 2 | Multi-band frame fusion | 5/10 | Action 1 | `solver`, `attention` | `fused_multi_band_frame = true` |
| 3 | Multistatic mesh aggregation | 5/10 | Action 2 | `mincut`, `attn-mincut` | `multistatic_mesh = true` |
| 4 | Pose tracker | 4/10 | Action 3, 7 | `mincut` | `pose_tracker = true` |
| 5 | End-to-end pipeline | 6/10 | Actions 2-4 | `temporal-tensor`, `attention` | `20hz_update = true` |
| 6 | Coherence metric | 3/10 | Action 2 | `solver` | `coherence_metric = true` |
| 7 | Coherence gate | 3/10 | Action 6 | `attn-mincut` | `coherence_gating = true` |
| 8 | AETHER re-ID | 4/10 | Actions 4, 7 | `attention` | `identity_stable = true` |
| 9 | ADR documentation | 2/10 | All above | None | Decision documented |

**Total cost: 36 units. Minimum viable path to acceptance test: Actions 1-5 + 6-7 = 30 units.**

### 3.3 Latency Budget (50ms cycle)

| Stage | Budget | Method |
|-------|--------|--------|
| UDP receive + parse | <1 ms | ADR-018 binary, 148 bytes, zero-alloc |
| Multi-band fusion | ~2 ms | NeumannSolver on 2×2 phase alignment |
| Multistatic fusion | ~3 ms | attn_mincut on 3-6 nodes × 64 velocity bins |
| Model inference | ~30-40 ms | CsiToPoseTransformer (lightweight, no ResNet) |
| Kalman update | <1 ms | 17 independent 6D filters, stack-allocated |
| **Total** | **~37-47 ms** | **Fits in 50 ms** |

---

## 4. Hardware Bill of Materials

| Component | Qty | Unit Cost | Purpose |
|-----------|-----|-----------|---------|
| ESP32-S3-DevKitC-1 | 4 | $10 | TX/RX sensing nodes |
| ESP32-S3-DevKitC-1 | 1 | $10 | Aggregator (or x86/RPi host) |
| External 5dBi antenna | 4-8 | $3 | Improved gain, directional coverage |
| USB-C hub (4 port) | 1 | $15 | Power distribution |
| Wall mount brackets | 4 | $2 | Ceiling/wall installation |
| **Total** | | **$73-91** | Complete 4-node mesh |

---

## 5. RuVector v2.0.4 Integration Map

All five published crates are exercised:

| Crate | Actions | Integration Point | Algorithmic Advantage |
|-------|---------|-------------------|----------------------|
| `ruvector-solver` | 2, 6 | Phase alignment; coherence matrix decomposition | O(√n) Neumann convergence |
| `ruvector-attention` | 2, 5, 8 | Cross-channel weighting; ring buffer; embedding similarity | Sublinear attention for small d |
| `ruvector-mincut` | 3, 4 | Viewpoint diversity partitioning; track assignment | O(n^1.5 log n) dynamic updates |
| `ruvector-attn-mincut` | 3, 7 | Cross-node spectrogram fusion; coherence gating | Attention + mincut in one pass |
| `ruvector-temporal-tensor` | 5 | Compressed sensing window ring buffer | 50-75% memory reduction |

---

## 6. IEEE 802.11bf Alignment

RuvSense's TDMA sensing schedule is forward-compatible with IEEE 802.11bf (WLAN Sensing, published 2024):

| RuvSense Concept | 802.11bf Equivalent |
|-----------------|---------------------|
| TX slot | Sensing Initiator |
| RX slot | Sensing Responder |
| TDMA cycle | Sensing Measurement Instance |
| NDP frame | Sensing NDP |
| Aggregator | Sensing Session Owner |

When commercial APs support 802.11bf, the ESP32 mesh can interoperate by translating SSP slots into 802.11bf Sensing Trigger frames.

---

## 7. Dependency Changes

### Firmware (C)

New files:
- `firmware/esp32-csi-node/main/sensing_schedule.h`
- `firmware/esp32-csi-node/main/sensing_schedule.c`

Modified files:
- `firmware/esp32-csi-node/main/csi_collector.c` (add channel hopping, link tagging)
- `firmware/esp32-csi-node/main/main.c` (add GPIO sync, TDMA timer)

### Rust

New module: `crates/wifi-densepose-signal/src/ruvsense/` (6 files, ~1500 lines estimated)

Modified files:
- `crates/wifi-densepose-signal/src/lib.rs` (export `ruvsense` module)
- `crates/wifi-densepose-signal/Cargo.toml` (no new deps; all ruvector crates already present per ADR-017)
- `crates/wifi-densepose-sensing-server/src/main.rs` (wire RuvSense pipeline into WebSocket output)

No new workspace dependencies. All ruvector crates are already in the workspace `Cargo.toml`.

---

## 8. Implementation Priority

| Priority | Actions | Weeks | Milestone |
|----------|---------|-------|-----------|
| P0 | 1 (firmware) | 2 | Channel-hopping ESP32 prototype |
| P0 | 2 (multi-band) | 2 | Wideband virtual frames |
| P1 | 3 (multistatic) | 2 | Multi-node fusion |
| P1 | 4 (tracker) | 1 | 17-keypoint Kalman |
| P1 | 6, 7 (coherence) | 1 | Gated updates |
| P2 | 5 (end-to-end) | 2 | 20 Hz pipeline |
| P2 | 8 (AETHER re-ID) | 1 | Identity hardening |
| P3 | 9 (docs) | 0.5 | This ADR finalized |
| **Total** | | **~10 weeks** | **Acceptance test** |

---

## 9. Consequences

### 9.1 Positive

- **3x bandwidth improvement** without hardware changes (channel hopping on existing ESP32)
- **12 independent viewpoints** from 4 commodity $10 nodes (C(4,2) × 2 links)
- **20 Hz update rate** with Kalman-smoothed output for sub-30mm torso jitter
- **Days-long stability** via coherence gating + SONA recalibration
- **All five ruvector crates exercised** — consistent algorithmic foundation
- **$73-91 total BOM** — accessible for research and production
- **802.11bf forward-compatible** — investment protected as commercial sensing arrives
- **Cognitum upgrade path** — same software stack, swap ESP32 for higher-bandwidth front end

### 9.2 Negative

- **4-node deployment** requires physical installation and calibration of node positions
- **TDMA scheduling** reduces per-node CSI rate (each node only transmits 1/4 of the time)
- **Channel hopping** introduces ~1-5ms gaps during `esp_wifi_set_channel()` transitions
- **5 GHz CSI on ESP32-S3** may not be available (ESP32-C6 supports it natively)
- **Coherence gate** may reject valid measurements during fast body motion (mitigation: gate only on static-subcarrier coherence)

### 9.3 Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| ESP32 channel hop causes CSI gaps | Medium | Reduced effective rate | Measure gap duration; increase dwell if >5ms |
| CSI callback rate exhausts lwIP pbufs | **Resolved** | Guru meditation crash | 50 Hz rate limiter + 100 ms ENOMEM backoff (Issue #127, PR #132) |
| 5 GHz CSI unavailable on S3 | High | Lose frequency diversity | Fallback: 3-channel 2.4 GHz still provides 3x BW; ESP32-C6 for dual-band |
| Model inference >40ms | Medium | Miss 20 Hz target | Run model at 10 Hz; Kalman predict at 20 Hz interpolates |
| Two-person separation fails at 3 nodes | Low | Identity swaps | AETHER re-ID recovers; increase to 4-6 nodes |
| Coherence gate false-triggers | Low | Missed updates | Gate on environmental coherence only, not body-motion subcarriers |

---

## 10. Related ADRs

| ADR | Relationship |
|-----|-------------|
| ADR-012 | **Extended**: RuvSense adds TDMA multistatic to single-AP mesh |
| ADR-014 | **Used**: All 6 SOTA algorithms applied per-link |
| ADR-016 | **Extended**: New ruvector integration points for multi-link fusion |
| ADR-017 | **Extended**: Coherence gating adds temporal stability layer |
| ADR-018 | **Modified**: Firmware gains channel hopping, TDMA schedule, HT40 |
| ADR-022 | **Complementary**: RuvSense is the ESP32 equivalent of Windows multi-BSSID |
| ADR-024 | **Used**: AETHER embeddings for person re-identification |
| ADR-026 | **Reused**: Kalman + lifecycle infrastructure lifted to RuvSense |
| ADR-027 | **Used**: GeometryEncoder, HardwareNormalizer, FiLM conditioning |

---

## 11. References

1. IEEE 802.11bf-2024. "WLAN Sensing." IEEE Standards Association.
2. Geng, J., Huang, D., De la Torre, F. (2023). "DensePose From WiFi." arXiv:2301.00250.
3. Yan, K. et al. (2024). "Person-in-WiFi 3D." CVPR 2024, pp. 969-978.
4. Chen, L. et al. (2026). "PerceptAlign: Geometry-Aware WiFi Sensing." arXiv:2601.12252.
5. Kotaru, M. et al. (2015). "SpotFi: Decimeter Level Localization Using WiFi." SIGCOMM.
6. Zheng, Y. et al. (2019). "Zero-Effort Cross-Domain Gesture Recognition with Wi-Fi." MobiSys.
7. Zeng, Y. et al. (2019). "FarSense: Pushing the Range Limit of WiFi-based Respiration Sensing." MobiCom.
8. AM-FM (2026). "A Foundation Model for Ambient Intelligence Through WiFi." arXiv:2602.11200.
9. Espressif ESP-CSI. https://github.com/espressif/esp-csi
