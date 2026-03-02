# RuvSense Domain Model

## Domain-Driven Design Specification

### Ubiquitous Language

| Term | Definition |
|------|------------|
| **Sensing Cycle** | One complete TDMA round (all nodes TX once): 50ms at 20 Hz |
| **Link** | A single TX-RX pair; with N nodes there are N×(N-1) directed links |
| **Multi-Band Frame** | Fused CSI from one node hopping across multiple channels in one dwell cycle |
| **Fused Sensing Frame** | Aggregated observation from all nodes at one sensing cycle, ready for inference |
| **Coherence Score** | 0.0-1.0 metric quantifying consistency of current CSI with reference template |
| **Coherence Gate** | Decision rule that accepts, inflates noise, rejects, or triggers recalibration |
| **Pose Track** | A temporally persistent per-person 17-keypoint trajectory with Kalman state |
| **Track Lifecycle** | State machine: Tentative → Active → Lost → Terminated |
| **Re-ID Embedding** | 128-dim AETHER contrastive vector encoding body identity |
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
- `KalmanPredictionService` — Predict step at 20 Hz (decoupled from measurement rate)
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
- TDMA cycle period must be ≤50ms for 20 Hz output
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
