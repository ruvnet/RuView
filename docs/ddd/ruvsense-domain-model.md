# RuvSense Domain Model

RuvSense is the multistatic WiFi sensing subsystem of RuView. It turns raw radio signals from multiple ESP32 sensors into tracked human poses, vital signs, and spatial awareness — all without cameras.

This document defines the system using [Domain-Driven Design](https://martinfowler.com/bliki/DomainDrivenDesign.html) (DDD): bounded contexts that own their data and rules, aggregate roots that enforce invariants, value objects that carry meaning, and domain events that connect everything. The goal is to make the system's structure match the physics it models — so that anyone reading the code (or an AI agent modifying it) understands *why* each piece exists, not just *what* it does.

**Bounded Contexts:**

| # | Context | Responsibility | Key ADRs | Code |
|---|---------|----------------|----------|------|
| 1 | [Multistatic Sensing](#1-multistatic-sensing-context) | Collect and fuse CSI from multiple nodes and channels | [ADR-029](../adr/ADR-029-ruvsense-multistatic-sensing-mode.md) | `signal/src/ruvsense/{multiband,phase_align,multistatic}.rs` |
| 2 | [Coherence](#2-coherence-context) | Monitor signal quality, gate bad data | [ADR-029](../adr/ADR-029-ruvsense-multistatic-sensing-mode.md) | `signal/src/ruvsense/{coherence,coherence_gate}.rs` |
| 3 | [Pose Tracking](#3-pose-tracking-context) | Track people as persistent skeletons with re-ID | [ADR-024](../adr/ADR-024-contrastive-csi-embedding-model.md), [ADR-037](../adr/ADR-037-multi-person-pose-detection.md) | `signal/src/ruvsense/pose_tracker.rs` |
| 4 | [Field Model](#4-field-model-context) | Learn room baselines, extract body perturbations | [ADR-030](../adr/ADR-030-ruvsense-persistent-field-model.md) | `signal/src/ruvsense/{field_model,tomography}.rs` |
| 5 | [Longitudinal Monitoring](#5-longitudinal-monitoring-context) | Track health trends over days/weeks | [ADR-030](../adr/ADR-030-ruvsense-persistent-field-model.md) | `signal/src/ruvsense/longitudinal.rs` |
| 6 | [Spatial Identity](#6-spatial-identity-context) | Cross-room tracking via environment fingerprints | [ADR-030](../adr/ADR-030-ruvsense-persistent-field-model.md) | `signal/src/ruvsense/cross_room.rs` |
| 7 | [Edge Intelligence](#7-edge-intelligence-context) | On-device sensing (no server needed) | [ADR-039](../adr/ADR-039-esp32-edge-intelligence.md), [ADR-040](../adr/ADR-040-wasm-programmable-sensing.md) | `firmware/esp32-csi-node/main/edge_processing.c` |

All code paths shown are relative to `rust-port/wifi-densepose-rs/crates/wifi-densepose-` unless otherwise noted.

---

## Domain-Driven Design Specification

### Ubiquitous Language

| Term | Definition |
|------|------------|
| **Sensing Cycle** | One complete TDMA round (all nodes TX once): ~35ms at 28.5 Hz (measured) |
| **Link** | A single TX-RX pair; with N nodes there are N×(N-1) directed links |
| **Multi-Band Frame** | Fused CSI from one node hopping across multiple channels in one dwell cycle |
| **Fused Sensing Frame** | Aggregated observation from all nodes at one sensing cycle, ready for inference |
| **Coherence Score** | 0.0-1.0 metric quantifying consistency of current CSI with reference template |
| **Coherence Gate** | Decision rule that accepts, inflates noise, rejects, or triggers recalibration |
| **Pose Track** | A temporally persistent per-person 17-keypoint trajectory with Kalman state |
| **Track Lifecycle** | State machine: Tentative → Active → Lost → Terminated |
| **Re-ID Embedding** | 128-dim AETHER contrastive vector encoding body identity |
| **Edge Tier** | Processing level on the ESP32: 0 = raw passthrough, 1 = signal cleanup, 2 = vitals, 3 = WASM modules |
| **WASM Module** | A small program compiled to WebAssembly that runs on the ESP32 for custom on-device sensing |
| **Node** | An ESP32-S3 device acting as both TX and RX in the multistatic mesh |
| **Aggregator** | Central device (ESP32/RPi/x86) that collects CSI from all nodes and runs fusion |
| **Sensing Schedule** | TDMA slot assignment: which node transmits when |
| **Channel Hop** | Switching the ESP32 radio to a different WiFi channel for multi-band sensing |
| **Person Cluster** | A subset of links whose CSI variations are correlated (attributed to one person) |

---

## Bounded Contexts

### 1. Multistatic Sensing Context

**Responsibility:** Collect, normalize, and fuse CSI from multiple ESP32 nodes across multiple channels into a single coherent sensing frame per cycle.

```
┌──────────────────────────────────────────────────────────┐
│              Multistatic Sensing Context                    │
├──────────────────────────────────────────────────────────┤
│                                                            │
│  ┌───────────────┐    ┌───────────────┐                   │
│  │  Link Buffer  │    │  Multi-Band   │                   │
│  │  Collector    │    │  Fuser        │                   │
│  │  (per-link    │    │  (per-node    │                   │
│  │   ring buf)   │    │   channel     │                   │
│  └───────┬───────┘    │   fusion)     │                   │
│          │            └───────┬───────┘                   │
│          │                    │                            │
│          └────────┬───────────┘                           │
│                   ▼                                        │
│          ┌────────────────┐                               │
│          │  Phase Aligner │                               │
│          │  (cross-chan   │                               │
│          │   correction)  │                               │
│          └────────┬───────┘                               │
│                   ▼                                        │
│          ┌────────────────┐                               │
│          │  Multistatic   │                               │
│          │  Fuser         │──▶ FusedSensingFrame          │
│          │  (cross-node   │                               │
│          │   attention)   │                               │
│          └────────────────┘                               │
│                                                            │
└──────────────────────────────────────────────────────────┘
```

**Aggregates:**
- `FusedSensingFrame` (Aggregate Root)

**Value Objects:**
- `MultiBandCsiFrame`
- `LinkGeometry` (tx_pos, rx_pos, distance, angle)
- `SensingSchedule`
- `ChannelHopConfig`

**Domain Services:**
- `PhaseAlignmentService` — Corrects LO-induced phase rotation between channels
- `MultiBandFusionService` — Merges per-channel CSI into wideband virtual frame
- `MultistaticFusionService` — Attention-based fusion of N nodes into one frame

**RuVector Integration:**
- `ruvector-solver` → Phase alignment (NeumannSolver)
- `ruvector-attention` → Cross-channel feature weighting
- `ruvector-attn-mincut` → Cross-node spectrogram attention gating
- `ruvector-temporal-tensor` → Per-link compressed ring buffers

---

### 2. Coherence Context

**Responsibility:** Monitor temporal consistency of CSI observations and gate downstream updates to reject drift, transient interference, and environmental changes.

```
┌──────────────────────────────────────────────────────────┐
│                  Coherence Context                          │
├──────────────────────────────────────────────────────────┤
│                                                            │
│  ┌───────────────┐    ┌───────────────┐                   │
│  │  Reference    │    │  Coherence    │                   │
│  │  Template     │    │  Calculator   │                   │
│  │  (EMA of      │    │  (z-score per │                   │
│  │   static CSI) │    │   subcarrier) │                   │
│  └───────┬───────┘    └───────┬───────┘                   │
│          │                    │                            │
│          └────────┬───────────┘                           │
│                   ▼                                        │
│          ┌────────────────┐                               │
│          │  Static/Dynamic│                               │
│          │  Decomposer    │                               │
│          │  (separate env │                               │
│          │   vs. body)    │                               │
│          └────────┬───────┘                               │
│                   ▼                                        │
│          ┌────────────────┐                               │
│          │  Gate Policy   │──▶ GateDecision               │
│          │  (Accept /     │    (Accept / PredictOnly /    │
│          │   Reject /     │     Reject / Recalibrate)    │
│          │   Recalibrate) │                               │
│          └────────────────┘                               │
│                                                            │
└──────────────────────────────────────────────────────────┘
```

**Aggregates:**
- `CoherenceState` (Aggregate Root) — Maintains reference template and gate state

**Value Objects:**
- `CoherenceScore` (0.0-1.0)
- `GateDecision` (Accept / PredictOnly / Reject / Recalibrate)
- `ReferenceTemplate` (EMA of static-period CSI)
- `DriftProfile` (Stable / Linear / StepChange)

**Domain Services:**
- `CoherenceCalculatorService` — Computes per-subcarrier z-score coherence
- `StaticDynamicDecomposerService` — Separates environmental drift from body motion
- `GatePolicyService` — Applies threshold-based gating rules

**RuVector Integration:**
- `ruvector-solver` → Coherence matrix decomposition (static vs. dynamic)
- `ruvector-attn-mincut` → Gate which subcarriers contribute to template update

---

### 3. Pose Tracking Context

**Responsibility:** Track multiple people as persistent 17-keypoint skeletons across time, with Kalman-smoothed trajectories, lifecycle management, and identity preservation via re-ID.

```
┌──────────────────────────────────────────────────────────┐
│                 Pose Tracking Context                       │
├──────────────────────────────────────────────────────────┤
│                                                            │
│  ┌───────────────┐    ┌───────────────┐                   │
│  │  Person       │    │  Detection    │                   │
│  │  Separator    │    │  -to-Track    │                   │
│  │  (min-cut on  │    │  Assigner     │                   │
│  │   link corr)  │    │  (Hungarian+  │                   │
│  └───────┬───────┘    │   embedding)  │                   │
│          │            └───────┬───────┘                   │
│          │                    │                            │
│          └────────┬───────────┘                           │
│                   ▼                                        │
│          ┌────────────────┐                               │
│          │  Kalman Filter │                               │
│          │  (17-keypoint  │                               │
│          │   6D state ×17)│                               │
│          └────────┬───────┘                               │
│                   ▼                                        │
│          ┌────────────────┐                               │
│          │  Lifecycle     │                               │
│          │  Manager       │──▶ TrackedPose                │
│          │  (Tentative →  │                               │
│          │   Active →     │                               │
│          │   Lost)        │                               │
│          └────────┬───────┘                               │
│                   │                                        │
│          ┌────────▼───────┐                               │
│          │  Embedding     │                               │
│          │  Identifier    │                               │
│          │  (AETHER re-ID)│                               │
│          └────────────────┘                               │
│                                                            │
└──────────────────────────────────────────────────────────┘
```

**Aggregates:**
- `PoseTrack` (Aggregate Root)

**Entities:**
- `KeypointState` — Per-keypoint Kalman state (x,y,z,vx,vy,vz) with covariance

**Value Objects:**
- `TrackedPose` — Immutable snapshot: 17 keypoints + confidence + track_id + lifecycle
- `PersonCluster` — Subset of links attributed to one person
- `AssignmentCost` — Combined Mahalanobis + embedding distance
- `TrackLifecycleState` (Tentative / Active / Lost / Terminated)

**Domain Services:**
- `PersonSeparationService` — Min-cut partitioning of cross-link correlation graph
- `TrackAssignmentService` — Bipartite matching of detections to existing tracks
- `KalmanPredictionService` — Predict step at 28 Hz (decoupled from measurement rate)
- `KalmanUpdateService` — Gated measurement update (subject to coherence gate)
- `EmbeddingIdentifierService` — AETHER cosine similarity for re-ID

**RuVector Integration:**
- `ruvector-mincut` → Person separation (DynamicMinCut on correlation graph)
- `ruvector-mincut` → Detection-to-track assignment (DynamicPersonMatcher)
- `ruvector-attention` → Embedding similarity via ScaledDotProductAttention

---

## Core Domain Entities

### FusedSensingFrame (Value Object)

```rust
pub struct FusedSensingFrame {
    /// Timestamp of this sensing cycle
    pub timestamp_us: u64,
    /// Fused multi-band spectrogram from all nodes
    /// Shape: [n_velocity_bins x n_time_frames]
    pub fused_bvp: Vec<f32>,
    pub n_velocity_bins: usize,
    pub n_time_frames: usize,
    /// Per-node multi-band frames (preserved for geometry)
    pub node_frames: Vec<MultiBandCsiFrame>,
    /// Node positions (from deployment config)
    pub node_positions: Vec<[f32; 3]>,
    /// Number of active nodes contributing
    pub active_nodes: usize,
    /// Cross-node coherence (higher = more agreement)
    pub cross_node_coherence: f32,
}
```

### PoseTrack (Aggregate Root)

```rust
pub struct PoseTrack {
    /// Unique track identifier
    pub id: TrackId,
    /// Per-keypoint Kalman state
    pub keypoints: [KeypointState; 17],
    /// Track lifecycle state
    pub lifecycle: TrackLifecycleState,
    /// Running-average AETHER embedding for re-ID
    pub embedding: Vec<f32>,  // [128]
    /// Frames since creation
    pub age: u64,
    /// Frames since last successful measurement update
    pub time_since_update: u64,
    /// Creation timestamp
    pub created_at: u64,
    /// Last update timestamp
    pub updated_at: u64,
}
```

### KeypointState (Entity)

```rust
pub struct KeypointState {
    /// State vector [x, y, z, vx, vy, vz]
    pub state: [f32; 6],
    /// 6x6 covariance matrix (upper triangle, row-major)
    pub covariance: [f32; 21],
    /// Confidence (0.0-1.0) from DensePose model
    pub confidence: f32,
}
```

### CoherenceState (Aggregate Root)

```rust
pub struct CoherenceState {
    /// Per-subcarrier reference amplitude (EMA)
    pub reference: Vec<f32>,
    /// Per-subcarrier variance over recent window
    pub variance: Vec<f32>,
    /// EMA decay rate for reference update
    pub decay: f32,
    /// Current coherence score
    pub score: f32,
    /// Frames since last accepted update
    pub stale_count: u64,
    /// Current drift profile classification
    pub drift_profile: DriftProfile,
}
```

---

## Domain Events

### Sensing Events

```rust
pub enum SensingEvent {
    /// New fused sensing frame available
    FrameFused {
        timestamp_us: u64,
        active_nodes: usize,
        cross_node_coherence: f32,
    },

    /// Node joined or left the mesh
    MeshTopologyChanged {
        node_id: u8,
        change: TopologyChange,  // Joined / Left / Degraded
        active_nodes: usize,
    },

    /// Channel hop completed on a node
    ChannelHopCompleted {
        node_id: u8,
        from_channel: u8,
        to_channel: u8,
        gap_us: u32,
    },
}
```

### Coherence Events

```rust
pub enum CoherenceEvent {
    /// Coherence dropped below accept threshold
    CoherenceLost {
        score: f32,
        threshold: f32,
        timestamp_us: u64,
    },

    /// Coherence recovered above accept threshold
    CoherenceRestored {
        score: f32,
        stale_duration_ms: u64,
        timestamp_us: u64,
    },

    /// Recalibration triggered (>10s low coherence)
    RecalibrationTriggered {
        stale_duration_ms: u64,
        timestamp_us: u64,
    },

    /// Recalibration completed via SONA TTT
    RecalibrationCompleted {
        adaptation_loss: f32,
        timestamp_us: u64,
    },

    /// Environmental drift detected
    DriftDetected {
        drift_type: DriftProfile,
        magnitude: f32,
        timestamp_us: u64,
    },
}
```

### Tracking Events

```rust
pub enum TrackingEvent {
    /// New person detected (track born)
    PersonDetected {
        track_id: TrackId,
        position: [f32; 3],  // centroid
        confidence: f32,
        timestamp_us: u64,
    },

    /// Person pose updated
    PoseUpdated {
        track_id: TrackId,
        keypoints: [[f32; 4]; 17],  // [x, y, z, conf] per keypoint
        jitter_mm: f32,  // RMS jitter at torso
        timestamp_us: u64,
    },

    /// Person lost (signal dropout)
    PersonLost {
        track_id: TrackId,
        last_position: [f32; 3],
        last_embedding: Vec<f32>,
        timestamp_us: u64,
    },

    /// Person re-identified after loss
    PersonReidentified {
        track_id: TrackId,
        previous_track_id: TrackId,
        similarity: f32,
        gap_duration_ms: u64,
        timestamp_us: u64,
    },

    /// Track terminated (exceeded max lost duration)
    TrackTerminated {
        track_id: TrackId,
        reason: TerminationReason,
        total_duration_ms: u64,
        timestamp_us: u64,
    },
}

pub enum TerminationReason {
    /// Exceeded max_lost_frames without re-acquisition
    SignalTimeout,
    /// Confidence below minimum for too long
    LowConfidence,
    /// Determined to be false positive
    FalsePositive,
    /// System shutdown
    SystemShutdown,
}
```

---

## Context Map

```
┌──────────────────────────────────────────────────────────────────┐
│                      RuvSense System                               │
├──────────────────────────────────────────────────────────────────┤
│                                                                    │
│  ┌──────────────────┐   FusedFrame   ┌──────────────────┐        │
│  │   Multistatic    │──────────────▶│   Pose Tracking   │        │
│  │   Sensing        │               │   Context          │        │
│  │   Context        │               │                    │        │
│  └────────┬─────────┘               └────────┬───────────┘        │
│           │                                   │                    │
│           │ Publishes                         │ Publishes          │
│           │ SensingEvent                      │ TrackingEvent      │
│           ▼                                   ▼                    │
│  ┌────────────────────────────────────────────────────┐           │
│  │              Event Bus (Domain Events)              │           │
│  └────────────────────┬───────────────────────────────┘           │
│                       │                                            │
│           ┌───────────▼───────────┐                               │
│           │   Coherence Context   │                               │
│           │   (subscribes to      │                               │
│           │    SensingEvent;      │                               │
│           │    publishes          │                               │
│           │    CoherenceEvent;    │                               │
│           │    gates Tracking     │                               │
│           │    updates)           │                               │
│           └───────────────────────┘                               │
│                                                                    │
├──────────────────────────────────────────────────────────────────┤
│                    UPSTREAM (Conformist)                           │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐            │
│  │wifi-densepose│  │wifi-densepose│  │wifi-densepose│            │
│  │  -hardware   │  │    -nn       │  │   -signal    │            │
│  │  (CsiFrame   │  │  (DensePose  │  │  (SOTA algs  │            │
│  │   parser)    │  │   model)     │  │   per link)  │            │
│  └──────────────┘  └──────────────┘  └──────────────┘            │
│                                                                    │
├──────────────────────────────────────────────────────────────────┤
│                    SIBLING (Partnership)                           │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐            │
│  │  AETHER      │  │  MERIDIAN    │  │  MAT         │            │
│  │  (ADR-024)   │  │  (ADR-027)   │  │  (ADR-001)   │            │
│  │  embeddings  │  │  geometry    │  │  triage      │            │
│  │  for re-ID   │  │  encoding    │  │  lifecycle   │            │
│  └──────────────┘  └──────────────┘  └──────────────┘            │
└──────────────────────────────────────────────────────────────────┘
```

**Relationship Types:**
- Multistatic Sensing → Pose Tracking: **Customer/Supplier** (Sensing produces FusedFrames; Tracking consumes)
- Coherence → Multistatic Sensing: **Subscriber** (monitors frame quality)
- Coherence → Pose Tracking: **Gate/Interceptor** (controls measurement updates)
- RuvSense → Upstream crates: **Conformist** (adapts to their types)
- RuvSense → AETHER/MERIDIAN/MAT: **Partnership** (shared embedding/geometry/tracking abstractions)

---

## Anti-Corruption Layer

### Hardware Adapter (Multistatic Sensing → wifi-densepose-hardware)

```rust
/// Adapts raw ESP32 CsiFrame to RuvSense MultiBandCsiFrame
pub struct MultiBandAdapter {
    /// Group frames by (node_id, channel) within time window
    window_ms: u32,
    /// Hardware normalizer (from MERIDIAN, ADR-027)
    normalizer: HardwareNormalizer,
}

impl MultiBandAdapter {
    /// Collect raw CsiFrames from one TDMA cycle and produce
    /// one MultiBandCsiFrame per node.
    pub fn adapt_cycle(
        &self,
        raw_frames: &[CsiFrame],
    ) -> Vec<MultiBandCsiFrame>;
}
```

### Model Adapter (Pose Tracking → wifi-densepose-nn)

```rust
/// Adapts DensePose model output to tracking-compatible detections
pub struct PoseDetectionAdapter;

impl PoseDetectionAdapter {
    /// Convert model output (heatmaps + offsets) to detected poses
    /// with keypoint positions and AETHER embeddings.
    pub fn adapt(
        &self,
        model_output: &ModelOutput,
        fused_frame: &FusedSensingFrame,
    ) -> Vec<PoseDetection>;
}

pub struct PoseDetection {
    pub keypoints: [[f32; 4]; 17],  // [x, y, z, confidence]
    pub embedding: Vec<f32>,         // [128] AETHER embedding
    pub person_cluster: PersonCluster,
}
```

### MAT Adapter (Pose Tracking → wifi-densepose-mat)

```rust
/// Adapts RuvSense TrackedPose to MAT Survivor entity
/// for disaster response scenarios.
pub struct SurvivorAdapter;

impl SurvivorAdapter {
    /// Convert a RuvSense TrackedPose to a MAT Survivor
    /// with vital signs extracted from small-motion analysis.
    pub fn to_survivor(
        &self,
        track: &PoseTrack,
        vital_signs: Option<&VitalSignsReading>,
    ) -> Survivor;
}
```

---

## Repository Interfaces

```rust
/// Persists and retrieves pose tracks
pub trait PoseTrackRepository {
    fn save(&self, track: &PoseTrack);
    fn find_by_id(&self, id: &TrackId) -> Option<PoseTrack>;
    fn find_active(&self) -> Vec<PoseTrack>;
    fn find_lost(&self) -> Vec<PoseTrack>;
    fn remove(&self, id: &TrackId);
}

/// Persists coherence state for long-term analysis
pub trait CoherenceRepository {
    fn save_snapshot(&self, state: &CoherenceState, timestamp_us: u64);
    fn load_latest(&self) -> Option<CoherenceState>;
    fn load_history(&self, duration_ms: u64) -> Vec<(u64, f32)>;
}

/// Persists mesh topology and node health
pub trait MeshRepository {
    fn save_node(&self, node_id: u8, position: [f32; 3], health: NodeHealth);
    fn load_topology(&self) -> Vec<(u8, [f32; 3], NodeHealth)>;
    fn save_schedule(&self, schedule: &SensingSchedule);
    fn load_schedule(&self) -> Option<SensingSchedule>;
}
```

---

## Invariants

### Multistatic Sensing
- At least 2 nodes must be active for multistatic fusion (fallback to single-node mode otherwise)
- Channel hop sequence must contain at least 1 non-overlapping channel
- TDMA cycle period must be ≤50ms for 28 Hz output
- Guard interval must be ≥2× clock drift budget (≥1ms for 50ms cycle)

### Coherence
- Reference template must be recalculated every 10 minutes during quiet periods
- Gate threshold must be calibrated per-environment (initial defaults: accept=0.85, drift=0.5)
- Stale count must not exceed max_stale (200 frames = 10s) without triggering recalibration
- Static/dynamic decomposition must preserve energy: ||S|| + ||D|| ≈ ||C||

### Pose Tracking
- Exactly one Kalman predict step per output frame (20 Hz, regardless of measurement availability)
- Birth gate: track not promoted to Active until 2 consecutive measurement updates
- Loss threshold: track marked Lost after 5 consecutive missed measurements
- Re-ID window: Lost tracks eligible for re-identification for 5 seconds
- Embedding EMA decay: 0.95 (slow adaptation preserves identity across environmental changes)
- Joint assignment cost must use both position (60%) and embedding (40%) terms

---

## Part II: Persistent Field Model Bounded Contexts (ADR-030)

### Ubiquitous Language (Extended)

| Term | Definition |
|------|------------|
| **Field Normal Mode** | The room's electromagnetic eigenstructure — stable propagation baseline when unoccupied |
| **Body Perturbation** | Structured change to field caused by a person, after environmental drift is removed |
| **Environmental Mode** | Principal component of baseline variation due to temperature, humidity, time-of-day |
| **Personal Baseline** | Per-person rolling statistical profile of biophysical proxies over days/weeks |
| **Drift Event** | Statistically significant deviation from personal baseline (>2sigma for >3 days) |
| **Drift Report** | Traceable evidence package: z-score, direction, window, supporting embeddings |
| **Risk Signal** | Actionable observation about biophysical change — not a diagnosis |
| **Intention Lead Signal** | Pre-movement dynamics (lean, weight shift) detected 200-500ms before visible motion |
| **Occupancy Volume** | Low-resolution 3D probabilistic density field from RF tomography |
| **Room Fingerprint** | HNSW-indexed embedding characterizing a room's electromagnetic identity |
| **Transition Event** | Person exiting one room and entering another, matched by embedding similarity |

---

### 4. Field Model Context

**Responsibility:** Learn and maintain the room's electromagnetic baseline. Decompose all CSI observations into environmental drift, body perturbation, and anomalies. Provide the foundation for all downstream exotic capabilities.

```
┌──────────────────────────────────────────────────────────┐
│                  Field Model Context                       │
├──────────────────────────────────────────────────────────┤
│                                                            │
│  ┌───────────────┐    ┌───────────────┐                   │
│  │  Calibration  │    │  Mode         │                   │
│  │  Collector    │    │  Extractor    │                   │
│  │  (empty-room  │    │  (SVD on      │                   │
│  │   CSI frames) │    │   baseline)   │                   │
│  └───────┬───────┘    └───────┬───────┘                   │
│          │                    │                            │
│          └────────┬───────────┘                           │
│                   ▼                                        │
│          ┌────────────────┐                               │
│          │  Perturbation  │                               │
│          │  Extractor     │                               │
│          │  (subtract     │                               │
│          │   baseline +   │──▶ BodyPerturbation           │
│          │   project out  │                               │
│          │   env modes)   │                               │
│          └────────┬───────┘                               │
│                   │                                        │
│          ┌────────▼───────┐                               │
│          │  RF Tomographer│                               │
│          │  (sparse 3D    │──▶ OccupancyVolume            │
│          │   inversion)   │                               │
│          └────────────────┘                               │
│                                                            │
└──────────────────────────────────────────────────────────┘
```

**Aggregates:**
- `FieldNormalMode` (Aggregate Root)

**Value Objects:**
- `BodyPerturbation` — Per-link CSI residual after baseline + environmental mode removal
- `EnvironmentalMode` — One principal component of baseline variation
- `OccupancyVolume` — 3D voxel grid of estimated mass density
- `CalibrationStatus` — Fresh / Stale / Expired (based on time since last empty-room)

**Domain Services:**
- `CalibrationService` — Detects empty-room windows, collects calibration data
- `ModeExtractionService` — SVD computation for environmental modes
- `PerturbationService` — Baseline subtraction + mode projection
- `TomographyService` — Sparse L1 inversion for occupancy volume

**RuVector Integration:**
- `ruvector-solver` → SVD for mode extraction; L1 for tomographic inversion
- `ruvector-temporal-tensor` → Baseline history compression
- `ruvector-attn-mincut` → Mode-subcarrier assignment partitioning

---

### 5. Longitudinal Monitoring Context

**Responsibility:** Maintain per-person biophysical baselines over days/weeks. Detect meaningful drift. Produce traceable evidence reports. Enforce the signals-not-diagnoses boundary.

```
┌──────────────────────────────────────────────────────────┐
│             Longitudinal Monitoring Context                 │
├──────────────────────────────────────────────────────────┤
│                                                            │
│  ┌───────────────┐    ┌───────────────┐                   │
│  │  Metric       │    │  Baseline     │                   │
│  │  Extractor    │    │  Updater      │                   │
│  │  (pose → gait,│    │  (Welford     │                   │
│  │   stability,  │    │   online      │                   │
│  │   breathing)  │    │   statistics) │                   │
│  └───────┬───────┘    └───────┬───────┘                   │
│          │                    │                            │
│          └────────┬───────────┘                           │
│                   ▼                                        │
│          ┌────────────────┐                               │
│          │  Drift Detector│                               │
│          │  (z-score vs   │                               │
│          │   personal     │                               │
│          │   baseline)    │                               │
│          └────────┬───────┘                               │
│                   ▼                                        │
│          ┌────────────────┐                               │
│          │  Evidence      │                               │
│          │  Assembler     │──▶ DriftReport                │
│          │  (embeddings + │                               │
│          │   timestamps + │                               │
│          │   graph links) │                               │
│          └────────────────┘                               │
│                                                            │
└──────────────────────────────────────────────────────────┘
```

**Aggregates:**
- `PersonalBaseline` (Aggregate Root)

**Entities:**
- `DailyMetricSummary` — One day's worth of compressed metric statistics per person

**Value Objects:**
- `DriftReport` — Evidence package with z-score, direction, window, embeddings
- `DriftMetric` — GaitSymmetry / StabilityIndex / BreathingRegularity / MicroTremor / ActivityLevel
- `DriftDirection` — Increasing / Decreasing
- `MonitoringLevel` — Physiological (Level 1) / Drift (Level 2) / RiskCorrelation (Level 3)
- `WelfordStats` — Online mean/variance accumulator (count, mean, M2)

**Domain Services:**
- `MetricExtractionService` — Extract biomechanical proxies from pose tracks
- `BaselineUpdateService` — Update Welford statistics with daily observations
- `DriftDetectionService` — Compute z-scores, identify significant deviations
- `EvidenceAssemblyService` — Package supporting embeddings and graph constraints

**RuVector Integration:**
- `ruvector-temporal-tensor` → Compressed daily summary storage
- `ruvector-attention` → Weight metric significance in drift score
- `ruvector-mincut` → Temporal changepoint detection in metric series
- HNSW → Similarity search across longitudinal embedding record

**Invariants:**
- Baseline requires 7+ observation days before drift detection activates
- Drift alert requires >2sigma deviation sustained for >3 consecutive days
- Evidence chain must include start/end embeddings bracketing the drift window
- System never outputs diagnostic language — only metric values and deviations
- Personal baseline decay: Welford stats use full history (no windowing) for stability

---

### 6. Spatial Identity Context

**Responsibility:** Maintain cross-room identity continuity via environment fingerprinting and transition graphs. Track who is where across spaces without storing images.

```
┌──────────────────────────────────────────────────────────┐
│               Spatial Identity Context                     │
├──────────────────────────────────────────────────────────┤
│                                                            │
│  ┌───────────────┐    ┌───────────────┐                   │
│  │  Room         │    │  Transition   │                   │
│  │  Fingerprint  │    │  Detector     │                   │
│  │  Index (HNSW) │    │  (exit/entry  │                   │
│  └───────┬───────┘    │   events)     │                   │
│          │            └───────┬───────┘                   │
│          │                    │                            │
│          └────────┬───────────┘                           │
│                   ▼                                        │
│          ┌────────────────┐                               │
│          │  Cross-Room    │                               │
│          │  Matcher       │                               │
│          │  (exit embed ↔ │──▶ TransitionEvent            │
│          │   entry embed) │                               │
│          └────────┬───────┘                               │
│                   │                                        │
│          ┌────────▼───────┐                               │
│          │  Transition    │                               │
│          │  Graph         │                               │
│          │  (rooms,       │                               │
│          │   persons,     │                               │
│          │   timestamps)  │                               │
│          └────────────────┘                               │
│                                                            │
└──────────────────────────────────────────────────────────┘
```

**Aggregates:**
- `SpatialIdentityGraph` (Aggregate Root)

**Entities:**
- `RoomProfile` — HNSW-indexed electromagnetic fingerprint of a room
- `PersonSpatialRecord` — Which rooms a person has visited, in order

**Value Objects:**
- `TransitionEvent` — Person, from_room, to_room, timestamps, embedding similarity
- `RoomFingerprint` — 128-dim AETHER embedding of the room's CSI profile
- `SpatialContinuity` — Confidence score for cross-room identity chain

**Domain Services:**
- `RoomFingerprintService` — Compute and index room electromagnetic profiles
- `TransitionDetectionService` — Detect exits (track lost near boundary) and entries (new track)
- `CrossRoomMatchingService` — HNSW similarity between exit and entry embeddings
- `TransitionGraphService` — Build and query the room-person-time graph

**RuVector Integration:**
- HNSW → Room and person fingerprint similarity search
- `ruvector-mincut` → Transition graph partitioning for occupancy analysis

**Invariants:**
- Cross-room match requires >0.80 cosine similarity AND <60s temporal gap
- Room fingerprint must be recalculated if mesh topology changes
- Transition graph edges are immutable once created (append-only audit trail)
- No image data stored — only 128-dim embeddings and structural events

---

### Domain Events (Extended)

#### Field Model Events

```rust
pub enum FieldModelEvent {
    /// Baseline calibration completed (empty room detected and measured)
    BaselineCalibrated {
        room_id: RoomId,
        n_modes: usize,
        variance_explained: f32,  // fraction of total variance captured
        timestamp_us: u64,
    },

    /// Environmental drift detected (baseline shift without body cause)
    EnvironmentalDriftDetected {
        room_id: RoomId,
        magnitude: f32,
        drift_type: DriftProfile,
        timestamp_us: u64,
    },

    /// Anomalous perturbation detected (does not match body or environment)
    AnomalousPerturbation {
        room_id: RoomId,
        anomaly_score: f32,
        affected_links: Vec<usize>,
        timestamp_us: u64,
    },

    /// Occupancy volume updated
    OccupancyUpdated {
        room_id: RoomId,
        occupied_voxels: usize,
        total_voxels: usize,
        timestamp_us: u64,
    },
}
```

#### Longitudinal Monitoring Events

```rust
pub enum LongitudinalEvent {
    /// Personal baseline established (7-day calibration complete)
    BaselineEstablished {
        person_id: PersonId,
        observation_days: u32,
        metrics_tracked: Vec<DriftMetric>,
        timestamp_us: u64,
    },

    /// Drift detected — biophysical metric significantly changed
    DriftDetected {
        person_id: PersonId,
        report: DriftReport,
        timestamp_us: u64,
    },

    /// Drift resolved — metric returned to baseline range
    DriftResolved {
        person_id: PersonId,
        metric: DriftMetric,
        resolution_days: u32,
        timestamp_us: u64,
    },

    /// Daily summary computed for a person
    DailySummaryComputed {
        person_id: PersonId,
        date: u64,  // day timestamp
        metrics: Vec<(DriftMetric, f32)>,  // metric, today's value
        timestamp_us: u64,
    },
}
```

#### Spatial Identity Events

```rust
pub enum SpatialEvent {
    /// New room fingerprinted
    RoomFingerprinted {
        room_id: RoomId,
        fingerprint_dims: usize,
        timestamp_us: u64,
    },

    /// Person transitioned between rooms
    PersonTransitioned {
        person_id: PersonId,
        from_room: RoomId,
        to_room: RoomId,
        similarity: f32,
        gap_duration_ms: u64,
        timestamp_us: u64,
    },

    /// Cross-room match failed (new person in destination room)
    CrossRoomMatchFailed {
        entry_room: RoomId,
        entry_embedding: Vec<f32>,
        candidates_checked: usize,
        best_similarity: f32,
        timestamp_us: u64,
    },
}
```

---

### Extended Context Map

```
┌──────────────────────────────────────────────────────────────────────┐
│                    RuvSense Full System (ADR-029 + ADR-030)            │
├──────────────────────────────────────────────────────────────────────┤
│                                                                        │
│  ┌───────────────┐  FusedFrame  ┌──────────────┐                     │
│  │  Multistatic  │────────────▶│ Pose Tracking │                     │
│  │  Sensing      │             │ Context       │                     │
│  └───────┬───────┘             └───────┬───────┘                     │
│          │                             │                              │
│          │                             │ TrackedPose                  │
│          │                             │                              │
│          ▼                     ┌───────▼───────┐                     │
│  ┌───────────────┐             │  Longitudinal │                     │
│  │  Coherence    │             │  Monitoring   │                     │
│  │  Context      │             │  Context      │                     │
│  └───────┬───────┘             └───────┬───────┘                     │
│          │ Gates                       │ DriftReport                  │
│          │                             │                              │
│          ▼                             ▼                              │
│  ┌───────────────┐             ┌───────────────┐                     │
│  │  Field Model  │             │  Spatial      │                     │
│  │  Context      │             │  Identity     │                     │
│  │  (baseline,   │             │  Context      │                     │
│  │   modes,      │             │  (cross-room, │                     │
│  │   tomography) │             │   transitions)│                     │
│  └───────────────┘             └───────────────┘                     │
│                                                                        │
│  ──────────────── Event Bus ──────────────────                       │
│  SensingEvent | CoherenceEvent | TrackingEvent |                     │
│  FieldModelEvent | LongitudinalEvent | SpatialEvent                  │
│                                                                        │
├──────────────────────────────────────────────────────────────────────┤
│  UPSTREAM:  wifi-densepose-{hardware, nn, signal}                     │
│  SIBLINGS:  AETHER (embeddings) | MERIDIAN (geometry) | MAT (triage) │
└──────────────────────────────────────────────────────────────────────┘
```

**New Relationship Types:**
- Multistatic Sensing → Field Model: **Partnership** (sensing provides raw CSI; field model provides perturbation extraction)
- Pose Tracking → Longitudinal Monitoring: **Customer/Supplier** (tracking provides daily pose metrics; monitoring builds baselines)
- Pose Tracking → Spatial Identity: **Customer/Supplier** (tracking provides track exit/entry events; spatial maintains transition graph)
- Coherence → Field Model: **Subscriber** (coherence events inform baseline recalibration)
- Longitudinal Monitoring → Spatial Identity: **Partnership** (person profiles shared for cross-room matching)

---

### Extended Repository Interfaces

```rust
/// Persists field normal modes and calibration history
pub trait FieldModelRepository {
    fn save_baseline(&self, room_id: RoomId, mode: &FieldNormalMode);
    fn load_baseline(&self, room_id: RoomId) -> Option<FieldNormalMode>;
    fn list_rooms(&self) -> Vec<RoomId>;
    fn save_occupancy_snapshot(&self, room_id: RoomId, volume: &OccupancyVolume, timestamp_us: u64);
}

/// Persists personal baselines and drift history
pub trait LongitudinalRepository {
    fn save_baseline(&self, baseline: &PersonalBaseline);
    fn load_baseline(&self, person_id: &PersonId) -> Option<PersonalBaseline>;
    fn save_daily_summary(&self, person_id: &PersonId, summary: &DailyMetricSummary);
    fn load_summaries(&self, person_id: &PersonId, days: u32) -> Vec<DailyMetricSummary>;
    fn save_drift_report(&self, report: &DriftReport);
    fn load_drift_history(&self, person_id: &PersonId) -> Vec<DriftReport>;
}

/// Persists room fingerprints and transition graphs
pub trait SpatialIdentityRepository {
    fn save_room_fingerprint(&self, room_id: RoomId, fingerprint: &RoomFingerprint);
    fn load_room_fingerprint(&self, room_id: RoomId) -> Option<RoomFingerprint>;
    fn save_transition(&self, transition: &TransitionEvent);
    fn load_transitions(&self, person_id: &PersonId, window_ms: u64) -> Vec<TransitionEvent>;
    fn load_room_occupancy(&self, room_id: RoomId) -> Vec<PersonId>;
}
```

---

### Extended Invariants

#### Field Model
- Baseline calibration requires ≥10 minutes of empty-room CSI (≥12,000 frames at 28 Hz)
- Environmental modes capped at K=5 (more modes overfit to noise)
- Tomographic inversion only valid with ≥8 links (4 nodes minimum)
- Baseline expires after 24 hours if not refreshed during quiet period
- Perturbation energy must be non-negative (enforced by magnitude computation)

#### Longitudinal Monitoring
- Personal baseline requires ≥7 observation days before drift detection activates
- Drift alert requires >2sigma deviation sustained for ≥3 consecutive days
- Evidence chain must include embedding pairs bracketing the drift window
- Output must never use diagnostic language — only metric values and statistical deviations
- Daily summaries stored for ≥90 days (rolling retention policy)
- Welford statistics use full history (no windowing) for maximum stability

#### Spatial Identity
- Cross-room match requires >0.80 cosine similarity AND <60s temporal gap
- Room fingerprint recalculated when mesh topology changes (node added/removed/moved)
- Transition graph is append-only (immutable audit trail)
- No image data stored — only 128-dim embeddings and structural events
- Maximum 100 rooms indexed per deployment (HNSW scaling constraint)

---

## Part III: Edge Intelligence Bounded Context (ADR-039, ADR-040, ADR-041)

### 7. Edge Intelligence Context

**Responsibility:** Run signal processing and sensing algorithms directly on the ESP32-S3, without requiring a server. The node detects presence, measures breathing and heart rate, alerts on falls, and runs custom WASM modules — all locally with instant response.

This is the only bounded context that runs on the microcontroller rather than the aggregator. It operates independently: the server is optional for visualization, but the ESP32 handles real-time sensing on its own.

```
┌──────────────────────────────────────────────────────────┐
│              Edge Intelligence Context                     │
│              (runs on ESP32-S3, Core 1)                    │
├──────────────────────────────────────────────────────────┤
│                                                            │
│  ┌───────────────┐    ┌───────────────┐                   │
│  │  Phase        │    │  Welford      │                   │
│  │  Extractor    │    │  Variance     │                   │
│  │  (I/Q → φ,   │    │  Tracker      │                   │
│  │   unwrap)     │    │  (per-subk)   │                   │
│  └───────┬───────┘    └───────┬───────┘                   │
│          │                    │                            │
│          └────────┬───────────┘                           │
│                   ▼                                        │
│          ┌────────────────┐                               │
│          │  Top-K Select  │                               │
│          │  + Bandpass     │                               │
│          │  (breathing:    │                               │
│          │   0.1-0.5 Hz,   │                               │
│          │   HR: 0.8-2 Hz) │                               │
│          └────────┬───────┘                               │
│                   ▼                                        │
│     ┌─────────────┼─────────────┐                        │
│     ▼             ▼             ▼                        │
│  ┌────────┐  ┌──────────┐  ┌──────────┐                 │
│  │Presence│  │ Vitals   │  │  Fall    │                  │
│  │Detector│  │ (BPM via │  │ Detector │                  │
│  │(motion │  │  zero-   │  │ (phase   │                  │
│  │ energy)│  │  crossing)│  │  accel)  │                  │
│  └────┬───┘  └────┬─────┘  └────┬─────┘                 │
│       └───────────┼──────────────┘                       │
│                   ▼                                        │
│          ┌────────────────┐                               │
│          │  Vitals Packet │──▶ UDP 32-byte (0xC5110002)   │
│          │  Assembler     │    at 1 Hz to aggregator      │
│          └────────┬───────┘                               │
│                   │                                        │
│          ┌────────▼───────┐                               │
│          │  WASM3 Runtime │                               │
│          │  (Tier 3: hot- │──▶ Custom module outputs      │
│          │   loadable     │                               │
│          │   modules)     │                               │
│          └────────────────┘                               │
│                                                            │
└──────────────────────────────────────────────────────────┘
```

**Aggregates:**
- `EdgeProcessingState` (Aggregate Root) — Holds all per-subcarrier state, filter history, and detection flags

**Value Objects:**
- `VitalsPacket` — 32-byte UDP packet: presence, motion, breathing BPM, heart rate BPM, confidence, fall flag, occupancy
- `EdgeTier` — Off (0) / BasicSignal (1) / FullVitals (2) / WasmExtended (3)
- `PresenceState` — Empty / Present / Moving
- `BandpassOutput` — Filtered signal in breathing or heart rate band
- `FallAlert` — Phase acceleration exceeding configurable threshold

**Entities:**
- `WasmModule` — A loaded WASM binary with its own memory arena (160 KB), frame budget (10 ms), and timer interval

**Domain Services:**
- `PhaseExtractionService` — Converts raw I/Q to unwrapped phase per subcarrier
- `VarianceTrackingService` — Welford running stats for subcarrier selection
- `TopKSelectionService` — Picks highest-variance subcarriers for downstream analysis
- `BandpassFilterService` — Biquad IIR filters for breathing (0.1-0.5 Hz) and heart rate (0.8-2.0 Hz)
- `PresenceDetectionService` — Adaptive threshold calibration (3-sigma over 1200-frame window)
- `VitalSignService` — Zero-crossing BPM estimation from filtered phase signals
- `FallDetectionService` — Phase acceleration exceeding threshold triggers alert
- `WasmRuntimeService` — WASM3 interpreter: load, execute, and sandbox custom modules

**NVS Configuration (runtime, no reflash needed):**

| Key | Type | Default | Purpose |
|-----|------|---------|---------|
| `edge_tier` | u8 | 0 | Processing tier (0/1/2/3) |
| `pres_thresh` | u16 | 0 | Presence threshold (0 = auto-calibrate) |
| `fall_thresh` | u16 | 2000 | Fall detection threshold (rad/s^2 x 1000) |
| `vital_win` | u16 | 256 | Phase history window (frames) |
| `vital_int` | u16 | 1000 | Vitals packet interval (ms) |
| `subk_count` | u8 | 8 | Top-K subcarrier count |
| `wasm_max` | u8 | 4 | Max concurrent WASM modules |
| `wasm_verify` | u8 | 0 | Require Ed25519 signature for uploads |

**Implementation files:**
- `firmware/esp32-csi-node/main/edge_processing.c` — DSP pipeline (~750 lines)
- `firmware/esp32-csi-node/main/edge_processing.h` — Types and API
- `firmware/esp32-csi-node/main/nvs_config.c` — NVS key reader (20 keys)
- `firmware/esp32-csi-node/provision.py` — CLI provisioning tool

**Invariants:**
- Edge processing runs on Core 1; WiFi and CSI callbacks run on Core 0 (no contention)
- CSI data flows from Core 0 to Core 1 via a lock-free SPSC ring buffer
- UDP sends are rate-limited to 50 Hz to prevent lwIP buffer exhaustion (Issue #127)
- ENOMEM backoff suppresses sends for 100 ms if lwIP runs out of packet buffers
- WASM modules are sandboxed: 160 KB arena, 10 ms frame budget, no direct hardware access
- Tier changes via NVS take effect on next reboot — no hot-reconfiguration of the DSP pipeline
- Fall detection threshold should be tuned per deployment (default 2000 causes false positives in static environments)

**Domain Events:**
```rust
pub enum EdgeEvent {
    /// Presence state changed
    PresenceChanged {
        node_id: u8,
        state: PresenceState,  // Empty / Present / Moving
        motion_energy: f32,
        timestamp_ms: u32,
    },

    /// Fall detected on-device
    FallDetected {
        node_id: u8,
        acceleration: f32,  // rad/s^2
        timestamp_ms: u32,
    },

    /// Vitals packet emitted
    VitalsEmitted {
        node_id: u8,
        breathing_bpm: f32,
        heart_rate_bpm: f32,
        confidence: f32,
        timestamp_ms: u32,
    },

    /// WASM module loaded or failed
    WasmModuleLoaded {
        slot: u8,
        module_name: String,
        success: bool,
        timestamp_ms: u32,
    },
}
```

**Relationship to other contexts:**
- Edge Intelligence → Multistatic Sensing: **Alternative** (edge runs on-device; multistatic runs on aggregator — same physics, different compute location)
- Edge Intelligence → Pose Tracking: **Upstream** (edge provides presence/vitals; aggregator can skip detection if edge already confirmed occupancy)
- Edge Intelligence → Coherence: **Simplified** (edge uses simple variance thresholds instead of full coherence gating)
