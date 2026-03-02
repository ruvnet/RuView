# ADR-030: RuvSense Persistent Field Model — Longitudinal Drift Detection and Exotic Sensing Tiers

| Field | Value |
|-------|-------|
| **Status** | Proposed |
| **Date** | 2026-03-02 |
| **Deciders** | ruv |
| **Codename** | **RuvSense Field** — Persistent Electromagnetic World Model |
| **Relates to** | ADR-029 (RuvSense Multistatic), ADR-005 (SONA Self-Learning), ADR-024 (AETHER Embeddings), ADR-016 (RuVector Integration), ADR-026 (Survivor Track Lifecycle), ADR-027 (MERIDIAN Generalization) |

---

## 1. Context

### 1.1 Beyond Pose Estimation

ADR-029 establishes RuvSense as a sensing-first multistatic mesh achieving 20 Hz DensePose with <30mm jitter. That treats WiFi as a **momentary pose estimator**. The next leap: treat the electromagnetic field as a **persistent world model** that remembers, predicts, and explains.

The most exotic capabilities come from this shift in abstraction level:
- The room is the model, not the person
- People are structured perturbations to a baseline
- Changes are deltas from a known state, not raw measurements
- Time is a first-class dimension — the system remembers days, not frames

### 1.2 The Seven Capability Tiers

| Tier | Capability | Foundation |
|------|-----------|-----------|
| 1 | **Field Normal Modes** — Room electromagnetic eigenstructure | Baseline calibration + SVD |
| 2 | **Coarse RF Tomography** — 3D occupancy volume from link attenuations | Sparse tomographic inversion |
| 3 | **Intention Lead Signals** — Pre-movement prediction (200-500ms lead) | Temporal embedding trajectory analysis |
| 4 | **Longitudinal Biomechanics Drift** — Personal baseline deviation over days | Welford statistics + HNSW memory |
| 5 | **Cross-Room Continuity** — Identity persistence across spaces without optics | Environment fingerprinting + transition graph |
| 6 | **Invisible Interaction Layer** — Multi-user gesture control through walls/darkness | Per-person CSI perturbation classification |
| 7 | **Adversarial Detection** — Physically impossible signal identification | Multi-link consistency + field model constraints |

### 1.3 Signals, Not Diagnoses

RF sensing detects **biophysical proxies**, not medical conditions:

| Detectable Signal | Not Detectable |
|-------------------|---------------|
| Breathing rate variability | COPD diagnosis |
| Gait asymmetry shift (18% over 14 days) | Parkinson's disease |
| Posture instability increase | Neurological condition |
| Micro-tremor onset | Specific tremor etiology |
| Activity level decline | Depression or pain diagnosis |

The output is: "Your movement symmetry has shifted 18 percent over 14 days." That is actionable without being diagnostic. The evidence chain (stored embeddings, drift statistics, coherence scores) is fully traceable.

### 1.4 Acceptance Tests

**Tier 0 (ADR-029):** Two people, 20 Hz, 10 min stable tracks, zero ID swaps, <30mm torso jitter.

**Tier 1-4 (this ADR):** Seven-day run, no manual tuning. System flags one real environmental change and one real human drift event, produces traceable explanation using stored embeddings plus graph constraints.

**Tier 5-7 (appliance):** Thirty-day local run, no camera. Detects meaningful drift with <5% false alarm rate.

---

## 2. Decision

### 2.1 Implement Field Normal Modes as the Foundation

Add a `field_model` module to `wifi-densepose-signal/src/ruvsense/` that learns the room's electromagnetic baseline during unoccupied periods and decomposes all subsequent observations into environmental drift + body perturbation.

```
wifi-densepose-signal/src/ruvsense/
├── mod.rs                // (existing, extend)
├── field_model.rs        // NEW: Field normal mode computation + perturbation extraction
├── tomography.rs         // NEW: Coarse RF tomography from link attenuations
├── longitudinal.rs       // NEW: Personal baseline + drift detection
├── intention.rs          // NEW: Pre-movement lead signal detector
├── cross_room.rs         // NEW: Cross-room identity continuity
├── gesture.rs            // NEW: Gesture classification from CSI perturbations
├── adversarial.rs        // NEW: Physically impossible signal detection
└── (existing files...)
```

### 2.2 Core Architecture: The Persistent Field Model

```
                    Time
                     │
                     ▼
    ┌────────────────────────────────┐
    │     Field Normal Modes (Tier 1) │
    │     Room baseline + SVD modes   │
    │     ruvector-solver             │
    └────────────┬───────────────────┘
                 │ Body perturbation (environmental drift removed)
                 │
         ┌───────┴───────┐
         │               │
         ▼               ▼
    ┌──────────┐   ┌──────────────┐
    │ Pose     │   │ RF Tomography│
    │ (ADR-029)│   │ (Tier 2)     │
    │ 20 Hz    │   │ Occupancy vol│
    └────┬─────┘   └──────────────┘
         │
         ▼
    ┌──────────────────────────────┐
    │  AETHER Embedding (ADR-024)  │
    │  128-dim contrastive vector  │
    └────────────┬─────────────────┘
                 │
         ┌───────┼───────┐
         │       │       │
         ▼       ▼       ▼
    ┌────────┐ ┌─────┐ ┌──────────┐
    │Intention│ │Track│ │Cross-Room│
    │Lead    │ │Re-ID│ │Continuity│
    │(Tier 3)│ │     │ │(Tier 5)  │
    └────────┘ └──┬──┘ └──────────┘
                  │
                  ▼
    ┌──────────────────────────────┐
    │  RuVector Longitudinal Memory │
    │  HNSW + graph + Welford stats│
    │  (Tier 4)                     │
    └──────────────┬───────────────┘
                   │
           ┌───────┴───────┐
           │               │
           ▼               ▼
    ┌──────────────┐ ┌──────────────┐
    │ Drift Reports│ │ Adversarial  │
    │ (Level 1-3)  │ │ Detection    │
    │              │ │ (Tier 7)     │
    └──────────────┘ └──────────────┘
```

### 2.3 Field Normal Modes (Tier 1)

**What it is:** The room's electromagnetic eigenstructure — the stable propagation paths, reflection coefficients, and interference patterns when nobody is present.

**How it works:**
1. During quiet periods (empty room, overnight), collect 10 minutes of CSI across all links
2. Compute per-link baseline (mean CSI vector)
3. Compute environmental variation modes via SVD (temperature, humidity, time-of-day effects)
4. Store top-K modes (K=3-5 typically captures >95% of environmental variance)
5. At runtime: subtract baseline, project out environmental modes, keep body perturbation

```rust
pub struct FieldNormalMode {
    pub baseline: Vec<Vec<Complex<f32>>>,      // [n_links × n_subcarriers]
    pub environmental_modes: Vec<Vec<f32>>,    // [n_modes × n_subcarriers]
    pub mode_energies: Vec<f32>,               // eigenvalues
    pub calibrated_at: u64,
    pub geometry_hash: u64,
}
```

**RuVector integration:**
- `ruvector-solver` → Low-rank SVD for mode extraction
- `ruvector-temporal-tensor` → Compressed baseline history storage
- `ruvector-attn-mincut` → Identify which subcarriers belong to which mode

### 2.4 Longitudinal Drift Detection (Tier 4)

**The defensible pipeline:**

```
RF → AETHER contrastive embedding
   → RuVector longitudinal memory (HNSW + graph)
     → Coherence-gated drift detection (Welford statistics)
       → Risk flag with traceable evidence
```

**Three monitoring levels:**

| Level | Signal Type | Example Output |
|-------|------------|----------------|
| **1: Physiological** | Raw biophysical metrics | "Breathing rate: 18.3 BPM today, 7-day avg: 16.1" |
| **2: Drift** | Personal baseline deviation | "Gait symmetry shifted 18% over 14 days" |
| **3: Risk correlation** | Pattern-matched concern | "Pattern consistent with increased fall risk" |

**Storage model:**

```rust
pub struct PersonalBaseline {
    pub person_id: PersonId,
    pub gait_symmetry: WelfordStats,
    pub stability_index: WelfordStats,
    pub breathing_regularity: WelfordStats,
    pub micro_tremor: WelfordStats,
    pub activity_level: WelfordStats,
    pub embedding_centroid: Vec<f32>,  // [128]
    pub observation_days: u32,
    pub updated_at: u64,
}
```

**RuVector integration:**
- `ruvector-temporal-tensor` → Compressed daily summaries (50-75% memory savings)
- HNSW → Embedding similarity search across longitudinal record
- `ruvector-attention` → Per-metric drift significance weighting
- `ruvector-mincut` → Temporal segmentation (detect changepoints in metric series)

### 2.5 Regulatory Classification

| Classification | What You Claim | Regulatory Path |
|---------------|---------------|-----------------|
| **Consumer wellness** (recommended first) | Activity metrics, breathing rate, stability score | Self-certification, FCC Part 15 |
| **Clinical decision support** (future) | Fall risk alert, respiratory pattern concern | FDA Class II 510(k) or De Novo |
| **Regulated medical device** (requires clinical partner) | Diagnostic claims for specific conditions | FDA Class II/III + clinical trials |

**Decision: Start as consumer wellness.** Build 12+ months of real-world longitudinal data. The dataset itself becomes the asset for future regulatory submissions.

---

## 3. Appliance Product Categories

### 3.1 Invisible Guardian

Wall-mounted wellness monitor for elderly care and independent living. No camera, no microphone, no reconstructable data. Stores embeddings and structural deltas only.

| Spec | Value |
|------|-------|
| Nodes | 4 ESP32-S3 pucks per room |
| Processing | Central hub (RPi 5 or x86) |
| Power | PoE or USB-C |
| Output | Risk flags, drift alerts, occupancy timeline |
| BOM | $73-91 (ESP32 mesh) + $35-80 (hub) |
| Validation | 30-day autonomous run, <5% false alarm rate |

### 3.2 Spatial Digital Twin Node

Live electromagnetic room model for smart buildings and workplace analytics.

| Spec | Value |
|------|-------|
| Output | Occupancy heatmap, flow vectors, dwell time, anomaly events |
| Integration | MQTT/REST API for BMS and CAFM |
| Retention | 30-day rolling, GDPR-compliant |
| Vertical | Smart buildings, retail, workspace optimization |

### 3.3 RF Interaction Surface

Multi-user gesture interface. No cameras. Works in darkness, smoke, through clothing.

| Spec | Value |
|------|-------|
| Gestures | Wave, point, beckon, push, circle + custom |
| Users | Up to 4 simultaneous |
| Latency | <100ms gesture recognition |
| Vertical | Smart home, hospitality, accessibility |

### 3.4 Pre-Incident Drift Monitor

Longitudinal biomechanics tracker for rehabilitation and occupational health.

| Spec | Value |
|------|-------|
| Baseline | 7-day calibration per person |
| Alert | Metric drift >2sigma for >3 days |
| Evidence | Stored embedding trajectory + statistical report |
| Vertical | Elderly care, rehab, occupational health |

### 3.5 Vertical Recommendation for First Hardware SKU

**Invisible Guardian** — the elderly care wellness monitor. Rationale:
1. Largest addressable market with immediate revenue (aging population, care facility demand)
2. Lowest regulatory bar (consumer wellness, no diagnostic claims)
3. Privacy advantage over cameras is a selling point, not a limitation
4. 30-day autonomous operation validates all tiers (field model, drift detection, coherence gating)
5. $108-171 BOM allows $299-499 retail with healthy margins

---

## 4. RuVector Integration Map (Extended)

All five crates are exercised across the exotic tiers:

| Tier | Crate | API | Role |
|------|-------|-----|------|
| 1 (Field) | `ruvector-solver` | `NeumannSolver` + SVD | Environmental mode decomposition |
| 1 (Field) | `ruvector-temporal-tensor` | `TemporalTensorCompressor` | Baseline history storage |
| 1 (Field) | `ruvector-attn-mincut` | `attn_mincut` | Mode-subcarrier assignment |
| 2 (Tomo) | `ruvector-solver` | `NeumannSolver` (L1) | Sparse tomographic inversion |
| 3 (Intent) | `ruvector-attention` | `ScaledDotProductAttention` | Temporal trajectory weighting |
| 3 (Intent) | `ruvector-temporal-tensor` | `CompressedCsiBuffer` | 2-second embedding history |
| 4 (Drift) | `ruvector-temporal-tensor` | `TemporalTensorCompressor` | Daily summary compression |
| 4 (Drift) | `ruvector-attention` | `ScaledDotProductAttention` | Metric drift significance |
| 4 (Drift) | `ruvector-mincut` | `DynamicMinCut` | Temporal changepoint detection |
| 5 (Cross-Room) | `ruvector-attention` | HNSW | Room and person fingerprint matching |
| 5 (Cross-Room) | `ruvector-mincut` | `MinCutBuilder` | Transition graph partitioning |
| 6 (Gesture) | `ruvector-attention` | `ScaledDotProductAttention` | Gesture template matching |
| 7 (Adversarial) | `ruvector-solver` | `NeumannSolver` | Physical plausibility verification |
| 7 (Adversarial) | `ruvector-attn-mincut` | `attn_mincut` | Multi-link consistency check |

---

## 5. Implementation Priority

| Priority | Tier | Module | Weeks | Dependency |
|----------|------|--------|-------|------------|
| P0 | 1 | `field_model.rs` | 2 | ADR-029 multistatic mesh operational |
| P0 | 4 | `longitudinal.rs` | 2 | Tier 1 baseline + AETHER embeddings |
| P1 | 2 | `tomography.rs` | 1 | Tier 1 perturbation extraction |
| P1 | 3 | `intention.rs` | 2 | Tier 1 + temporal embedding history |
| P2 | 5 | `cross_room.rs` | 2 | Tier 4 person profiles + multi-room deployment |
| P2 | 6 | `gesture.rs` | 1 | Tier 1 perturbation + per-person separation |
| P3 | 7 | `adversarial.rs` | 1 | Tier 1 field model + multi-link consistency |

**Total exotic tier: ~11 weeks after ADR-029 acceptance test passes.**

---

## 6. Consequences

### 6.1 Positive

- **Room becomes self-sensing**: Field normal modes provide a persistent baseline that explains change as structured deltas
- **7-day autonomous operation**: Coherence gating + SONA adaptation + longitudinal memory eliminate manual tuning
- **Privacy by design**: No images, no audio, no reconstructable data — only embeddings and statistical summaries
- **Traceable evidence**: Every drift alert links to stored embeddings, timestamps, and graph constraints
- **Multiple product categories**: Same software stack, different packaging — Guardian, Twin, Interaction, Drift Monitor
- **Regulatory clarity**: Consumer wellness first, clinical decision support later with accumulated dataset
- **Security primitive**: Coherence gating detects adversarial injection, not just quality issues

### 6.2 Negative

- **7-day calibration** required for personal baselines (system is less useful during initial period)
- **Empty-room calibration** needed for field normal modes (may not always be available)
- **Storage growth**: Longitudinal memory grows ~1 KB/person/day (manageable but non-zero)
- **Statistical power**: Drift detection requires 14+ days of data for meaningful z-scores
- **Multi-room**: Cross-room continuity requires hardware in all rooms (cost scales linearly)

### 6.3 Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Field modes drift faster than expected | Medium | False perturbation detections | Reduce mode update interval from 24h to 4h |
| Personal baselines too variable | Medium | High false alarm rate for drift | Widen sigma threshold from 2σ to 3σ; require 5+ days |
| Cross-room matching fails for similar body types | Low | Identity confusion | Require temporal proximity (<60s) plus spatial adjacency |
| Gesture recognition insufficient SNR | Medium | <80% accuracy | Restrict to near-field (<2m) initially |
| Adversarial injection via coordinated WiFi injection | Very Low | Spoofed occupancy | Multi-link consistency check makes single-link spoofing detectable |

---

## 7. Related ADRs

| ADR | Relationship |
|-----|-------------|
| ADR-029 | **Prerequisite**: Multistatic mesh is the sensing substrate for all exotic tiers |
| ADR-005 (SONA) | **Extended**: SONA recalibration triggered by coherence gate → now also by drift events |
| ADR-016 (RuVector) | **Extended**: All 5 crates exercised across 7 exotic tiers |
| ADR-024 (AETHER) | **Critical dependency**: Embeddings are the representation for all longitudinal memory |
| ADR-026 (Tracking) | **Extended**: Track lifecycle now spans days (not minutes) for drift detection |
| ADR-027 (MERIDIAN) | **Used**: Room geometry encoding for field normal mode conditioning |

---

## 8. References

1. IEEE 802.11bf-2024. "WLAN Sensing." IEEE Standards Association.
2. FDA. "General Wellness: Policy for Low Risk Devices." Guidance Document, 2019.
3. EU MDR 2017/745. "Medical Device Regulation." Official Journal of the European Union.
4. Welford, B.P. (1962). "Note on a Method for Calculating Corrected Sums of Squares." Technometrics.
5. Chen, L. et al. (2026). "PerceptAlign: Geometry-Aware WiFi Sensing." arXiv:2601.12252.
6. AM-FM (2026). "A Foundation Model for Ambient Intelligence Through WiFi." arXiv:2602.11200.
7. Geng, J. et al. (2023). "DensePose From WiFi." arXiv:2301.00250.
