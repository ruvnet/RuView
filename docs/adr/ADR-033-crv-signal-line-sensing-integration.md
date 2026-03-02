# ADR-033: CRV Signal Line Sensing Integration -- Mapping 6-Stage Coordinate Remote Viewing to WiFi-DensePose Pipeline

| Field | Value |
|-------|-------|
| **Status** | Proposed |
| **Date** | 2026-03-01 |
| **Deciders** | ruv |
| **Codename** | **CRV-Sense** -- Coordinate Remote Viewing Signal Line for WiFi Sensing |
| **Relates to** | ADR-016 (RuVector Integration), ADR-017 (RuVector Signal+MAT), ADR-024 (AETHER Embeddings), ADR-029 (RuvSense Multistatic), ADR-030 (Persistent Field Model), ADR-031 (RuView Viewpoint Fusion), ADR-032 (Mesh Security) |

---

## 1. Context

### 1.1 The CRV Signal Line Methodology

Coordinate Remote Viewing (CRV) is a structured 6-stage protocol that progressively refines perception from coarse gestalt impressions (Stage I) through sensory details (Stage II), spatial dimensions (Stage III), noise separation (Stage IV), cross-referencing interrogation (Stage V), to a final composite 3D model (Stage VI). The `ruvector-crv` crate (v0.1.1, published on crates.io) maps these 6 stages to vector database subsystems: Poincare ball embeddings, multi-head attention, GNN graph topology, SNN temporal encoding, differentiable search, and MinCut partitioning.

The WiFi-DensePose sensing pipeline follows a strikingly similar progressive refinement:

1. Raw CSI arrives as an undifferentiated signal -- the system must first classify the gestalt character of the RF environment.
2. Per-subcarrier amplitude/phase/frequency features are extracted -- analogous to sensory impressions.
3. The AP mesh forms a spatial topology with node positions and link geometry -- a dimensional sketch.
4. Coherence gating separates valid signal from noise and interference -- analytically overlaid artifacts must be detected and removed.
5. Pose estimation queries earlier CSI features for cross-referencing -- interrogation of the accumulated evidence.
6. Final multi-person partitioning produces the composite DensePose output -- the 3D model.

This structural isomorphism is not accidental. Both CRV and WiFi sensing solve the same abstract problem: extract structured information from a noisy, high-dimensional signal space through progressive refinement with explicit noise separation.

### 1.2 The ruvector-crv Crate (v0.1.1)

The `ruvector-crv` crate provides the following public API:

| Component | Purpose | Upstream Dependency |
|-----------|---------|-------------------|
| `CrvSessionManager` | Session lifecycle: create, add stage data, convergence analysis | -- |
| `StageIEncoder` | Poincare ball hyperbolic embeddings for gestalt primitives | -- (internal hyperbolic math) |
| `StageIIEncoder` | Multi-head attention for sensory vectors | `ruvector-attention` |
| `StageIIIEncoder` | GNN graph topology encoding | `ruvector-gnn` |
| `StageIVEncoder` | SNN temporal encoding for AOL (Analytical Overlay) detection | -- (internal SNN) |
| `StageVEngine` | Differentiable search and cross-referencing | -- (internal soft attention) |
| `StageVIModeler` | MinCut partitioning for composite model | `ruvector-mincut` |
| `ConvergenceResult` | Cross-session agreement analysis | -- |
| `CrvConfig` | Configuration (384-d default, curvature, AOL threshold, SNN params) | -- |

Key types: `GestaltType` (Manmade/Natural/Movement/Energy/Water/Land), `SensoryModality` (Texture/Color/Temperature/Sound/...), `AOLDetection` (content + anomaly score), `SignalLineProbe` (query + attention weights), `TargetPartition` (MinCut cluster + centroid).

### 1.3 What Already Exists in WiFi-DensePose

The following modules already implement pieces of the pipeline that CRV stages map onto:

| Existing Module | Location | Relevant CRV Stage |
|----------------|----------|-------------------|
| `multiband.rs` | `wifi-densepose-signal/src/ruvsense/` | Stage I (gestalt from multi-band CSI) |
| `phase_align.rs` | `wifi-densepose-signal/src/ruvsense/` | Stage II (phase feature extraction) |
| `multistatic.rs` | `wifi-densepose-signal/src/ruvsense/` | Stage III (AP mesh spatial topology) |
| `coherence_gate.rs` | `wifi-densepose-signal/src/ruvsense/` | Stage IV (signal-vs-noise separation) |
| `field_model.rs` | `wifi-densepose-signal/src/ruvsense/` | Stage V (persistent field for querying) |
| `pose_tracker.rs` | `wifi-densepose-signal/src/ruvsense/` | Stage VI (person tracking output) |
| Viewpoint fusion | `wifi-densepose-ruvector/src/viewpoint/` | Cross-session (multi-viewpoint convergence) |

The `wifi-densepose-ruvector` crate already depends on `ruvector-crv` in its `Cargo.toml`. This ADR defines how to wrap the CRV API with WiFi-DensePose domain types.

### 1.4 The Key Insight: Cross-Session Convergence = Cross-Room Identity

CRV's convergence analysis compares independent sessions targeting the same coordinate to find agreement in their embeddings. In WiFi-DensePose, different AP clusters in different rooms are independent "viewers" of the same person. When a person moves from Room A to Room B, the CRV convergence mechanism can find agreement between the Room A embedding trail and the Room B initial embeddings -- establishing identity continuity without cameras.

---

## 2. Decision

### 2.1 The 6-Stage CRV-to-WiFi Mapping

Create a new `crv` module in the `wifi-densepose-ruvector` crate that wraps `ruvector-crv` with WiFi-DensePose domain types. Each CRV stage maps to a specific point in the sensing pipeline.

```
+-------------------------------------------------------------------+
|                 CRV-Sense Pipeline (6 Stages)                      |
+-------------------------------------------------------------------+
|                                                                     |
|  Raw CSI frames from ESP32 mesh (ADR-029)                          |
|       |                                                             |
|       v                                                             |
|  +----------------------------------------------------------+     |
|  | Stage I: CSI Gestalt Classification                       |     |
|  |   CsiGestaltClassifier                                    |     |
|  |   Input: raw CSI frame (amplitude envelope + phase slope) |     |
|  |   Output: GestaltType (Manmade/Natural/Movement/Energy)   |     |
|  |   Encoder: StageIEncoder (Poincare ball embedding)        |     |
|  |   Module: ruvsense/multiband.rs                           |     |
|  +----------------------------+-----------------------------+     |
|                               |                                     |
|                               v                                     |
|  +----------------------------------------------------------+     |
|  | Stage II: CSI Sensory Feature Extraction                  |     |
|  |   CsiSensoryEncoder                                       |     |
|  |   Input: per-subcarrier CSI                               |     |
|  |   Output: amplitude textures, phase patterns, freq colors |     |
|  |   Encoder: StageIIEncoder (multi-head attention vectors)  |     |
|  |   Module: ruvsense/phase_align.rs                         |     |
|  +----------------------------+-----------------------------+     |
|                               |                                     |
|                               v                                     |
|  +----------------------------------------------------------+     |
|  | Stage III: AP Mesh Spatial Topology                       |     |
|  |   MeshTopologyEncoder                                     |     |
|  |   Input: node positions, link SNR, baseline distances     |     |
|  |   Output: GNN graph embedding of mesh geometry            |     |
|  |   Encoder: StageIIIEncoder (GNN topology)                 |     |
|  |   Module: ruvsense/multistatic.rs                         |     |
|  +----------------------------+-----------------------------+     |
|                               |                                     |
|                               v                                     |
|  +----------------------------------------------------------+     |
|  | Stage IV: Coherence Gating (AOL Detection)                |     |
|  |   CoherenceAolDetector                                    |     |
|  |   Input: phase coherence scores, gate decisions           |     |
|  |   Output: AOL-flagged frames removed, clean signal kept   |     |
|  |   Encoder: StageIVEncoder (SNN temporal encoding)         |     |
|  |   Module: ruvsense/coherence_gate.rs                      |     |
|  +----------------------------+-----------------------------+     |
|                               |                                     |
|                               v                                     |
|  +----------------------------------------------------------+     |
|  | Stage V: Pose Interrogation                               |     |
|  |   PoseInterrogator                                        |     |
|  |   Input: pose hypothesis + accumulated CSI features       |     |
|  |   Output: soft attention over CSI history, top candidates |     |
|  |   Engine: StageVEngine (differentiable search)            |     |
|  |   Module: ruvsense/field_model.rs                         |     |
|  +----------------------------+-----------------------------+     |
|                               |                                     |
|                               v                                     |
|  +----------------------------------------------------------+     |
|  | Stage VI: Multi-Person Partitioning                       |     |
|  |   PersonPartitioner                                       |     |
|  |   Input: all person embedding clusters                    |     |
|  |   Output: MinCut-separated person partitions + centroids  |     |
|  |   Modeler: StageVIModeler (MinCut partitioning)           |     |
|  |   Module: training pipeline (ruvector-mincut)             |     |
|  +----------------------------+-----------------------------+     |
|                               |                                     |
|                               v                                     |
|  +----------------------------------------------------------+     |
|  | Cross-Session: Multi-Room Convergence                     |     |
|  |   MultiViewerConvergence                                  |     |
|  |   Input: per-room embedding trails for candidate persons  |     |
|  |   Output: cross-room identity matches + confidence        |     |
|  |   Engine: CrvSessionManager::find_convergence()           |     |
|  |   Module: ruvsense/cross_room.rs                          |     |
|  +----------------------------------------------------------+     |
+-------------------------------------------------------------------+
```

### 2.2 Stage I: CSI Gestalt Classification

**CRV mapping:** Stage I ideograms classify the target's fundamental character (Manmade/Natural/Movement/Energy). In WiFi sensing, the raw CSI frame's amplitude envelope shape and phase slope direction provide an analogous gestalt classification of the RF environment.

**WiFi domain types:**

```rust
/// CSI-domain gestalt types mapped from CRV GestaltType.
///
/// The CRV taxonomy maps to RF phenomenology:
/// - Manmade: structured multipath (walls, furniture, metallic reflectors)
/// - Natural: diffuse scattering (vegetation, irregular surfaces)
/// - Movement: Doppler-shifted components (human motion, fan, pet)
/// - Energy: high-amplitude transients (microwave, motor, interference)
/// - Water: slow fading envelope (humidity change, condensation)
/// - Land: static baseline (empty room, no perturbation)
pub struct CsiGestaltClassifier {
    encoder: StageIEncoder,
    config: CrvConfig,
}

impl CsiGestaltClassifier {
    /// Classify a raw CSI frame into a gestalt type.
    ///
    /// Extracts three features from the CSI frame:
    /// 1. Amplitude envelope shape (ideogram stroke analog)
    /// 2. Phase slope direction (spontaneous descriptor analog)
    /// 3. Subcarrier correlation structure (classification signal)
    ///
    /// Returns a Poincare ball embedding (384-d by default) encoding
    /// the hierarchical gestalt taxonomy with exponentially less
    /// distortion than Euclidean space.
    pub fn classify(&self, csi_frame: &CsiFrame) -> CrvResult<(GestaltType, Vec<f32>)>;
}
```

**Integration point:** `ruvsense/multiband.rs` already processes multi-band CSI. The `CsiGestaltClassifier` wraps this with Poincare ball embedding via `StageIEncoder`, producing a hyperbolic embedding that captures the gestalt hierarchy.

### 2.3 Stage II: CSI Sensory Feature Extraction

**CRV mapping:** Stage II collects sensory impressions (texture, color, temperature). In WiFi sensing, the per-subcarrier CSI features are the sensory modalities:

| CRV Sensory Modality | WiFi CSI Analog |
|----------------------|-----------------|
| Texture | Amplitude variance pattern across subcarriers (smooth vs rough surface reflection) |
| Color | Frequency-domain spectral shape (which subcarriers carry the most energy) |
| Temperature | Phase drift rate (thermal expansion changes path length) |
| Luminosity | Overall signal power level (SNR) |
| Dimension | Delay spread (multipath extent maps to room size) |

**WiFi domain types:**

```rust
pub struct CsiSensoryEncoder {
    encoder: StageIIEncoder,
}

impl CsiSensoryEncoder {
    /// Extract sensory features from per-subcarrier CSI data.
    ///
    /// Maps CSI signal characteristics to CRV sensory modalities:
    /// - Amplitude variance -> Texture
    /// - Spectral shape -> Color
    /// - Phase drift rate -> Temperature
    /// - Signal power -> Luminosity
    /// - Delay spread -> Dimension
    ///
    /// Uses multi-head attention (ruvector-attention) to produce
    /// a unified sensory embedding that captures cross-modality
    /// correlations.
    pub fn encode(&self, csi_subcarriers: &SubcarrierData) -> CrvResult<Vec<f32>>;
}
```

**Integration point:** `ruvsense/phase_align.rs` already computes per-subcarrier phase features. The `CsiSensoryEncoder` maps these to `StageIIData` sensory impressions and produces attention-weighted embeddings via `StageIIEncoder`.

### 2.4 Stage III: AP Mesh Spatial Topology

**CRV mapping:** Stage III sketches the spatial layout with geometric primitives and relationships. In WiFi sensing, the AP mesh nodes and their inter-node links form the spatial sketch:

| CRV Sketch Element | WiFi Mesh Analog |
|-------------------|-----------------|
| `SketchElement` | AP node (position, antenna orientation) |
| `GeometricKind::Point` | Single AP location |
| `GeometricKind::Line` | Bistatic link between two APs |
| `SpatialRelationship` | Link quality, baseline distance, angular separation |

**WiFi domain types:**

```rust
pub struct MeshTopologyEncoder {
    encoder: StageIIIEncoder,
}

impl MeshTopologyEncoder {
    /// Encode the AP mesh as a GNN graph topology.
    ///
    /// Each AP node becomes a SketchElement with its position and
    /// antenna count. Each bistatic link becomes a SpatialRelationship
    /// with strength proportional to link SNR.
    ///
    /// Uses ruvector-gnn to produce a graph embedding that captures
    /// the mesh's geometric diversity index (GDI) and effective
    /// viewpoint count.
    pub fn encode(&self, mesh: &MultistaticArray) -> CrvResult<Vec<f32>>;
}
```

**Integration point:** `ruvsense/multistatic.rs` manages the AP mesh topology. The `MeshTopologyEncoder` translates `MultistaticArray` geometry into `StageIIIData` sketch elements and relationships, producing a GNN-encoded topology embedding via `StageIIIEncoder`.

### 2.5 Stage IV: Coherence Gating as AOL Detection

**CRV mapping:** Stage IV detects Analytical Overlay (AOL) -- moments when the analytical mind contaminates the raw signal with pre-existing assumptions. In WiFi sensing, the coherence gate (ADR-030/032) serves the same function: it detects when environmental interference, multipath changes, or hardware artifacts contaminate the CSI signal, and flags those frames for exclusion.

| CRV AOL Concept | WiFi Coherence Analog |
|-----------------|---------------------|
| AOL event | Low-coherence frame (interference, multipath shift, hardware glitch) |
| AOL anomaly score | Coherence metric (0.0 = fully incoherent, 1.0 = fully coherent) |
| AOL break (flagged, set aside) | `GateDecision::Reject` or `GateDecision::PredictOnly` |
| Clean signal line | `GateDecision::Accept` with noise multiplier |
| Forced accept after timeout | `GateDecision::ForcedAccept` (ADR-032) with inflated noise |

**WiFi domain types:**

```rust
pub struct CoherenceAolDetector {
    encoder: StageIVEncoder,
}

impl CoherenceAolDetector {
    /// Map coherence gate decisions to CRV AOL detection.
    ///
    /// The SNN temporal encoding models the spike pattern of
    /// coherence violations over time:
    /// - Burst of low-coherence frames -> high AOL anomaly score
    /// - Sustained coherence -> low anomaly score (clean signal)
    /// - Single transient -> moderate score (check and continue)
    ///
    /// Returns an embedding that encodes the temporal pattern of
    /// signal quality, enabling downstream stages to weight their
    /// attention based on signal cleanliness.
    pub fn detect(
        &self,
        coherence_history: &[GateDecision],
        timestamps: &[u64],
    ) -> CrvResult<(Vec<AOLDetection>, Vec<f32>)>;
}
```

**Integration point:** `ruvsense/coherence_gate.rs` already produces `GateDecision` values. The `CoherenceAolDetector` translates the coherence gate's temporal stream into `StageIVData` with `AOLDetection` events, and the SNN temporal encoding via `StageIVEncoder` produces an embedding of signal quality over time.

### 2.6 Stage V: Pose Interrogation via Differentiable Search

**CRV mapping:** Stage V is the interrogation phase -- probing earlier stage data with specific queries to extract targeted information. In WiFi sensing, this maps to querying the accumulated CSI feature history with a pose hypothesis to find supporting or contradicting evidence.

**WiFi domain types:**

```rust
pub struct PoseInterrogator {
    engine: StageVEngine,
}

impl PoseInterrogator {
    /// Cross-reference a pose hypothesis against CSI history.
    ///
    /// Uses differentiable search (soft attention with temperature
    /// scaling) to find which historical CSI frames best support
    /// or contradict the current pose estimate.
    ///
    /// Returns:
    /// - Attention weights over the CSI history buffer
    /// - Top-k supporting frames (highest attention)
    /// - Cross-references linking pose keypoints to specific
    ///   CSI subcarrier features from earlier stages
    pub fn interrogate(
        &self,
        pose_embedding: &[f32],
        csi_history: &[CrvSessionEntry],
    ) -> CrvResult<(StageVData, Vec<f32>)>;
}
```

**Integration point:** `ruvsense/field_model.rs` maintains the persistent electromagnetic field model (ADR-030). The `PoseInterrogator` wraps this with CRV Stage V semantics -- the field model's history becomes the corpus that `StageVEngine` searches over, and the pose hypothesis becomes the probe query.

### 2.7 Stage VI: Multi-Person Partitioning via MinCut

**CRV mapping:** Stage VI produces the composite 3D model by clustering accumulated data into distinct target partitions via MinCut. In WiFi sensing, this maps to multi-person separation -- partitioning the accumulated CSI embeddings into person-specific clusters.

**WiFi domain types:**

```rust
pub struct PersonPartitioner {
    modeler: StageVIModeler,
}

impl PersonPartitioner {
    /// Partition accumulated embeddings into distinct persons.
    ///
    /// Uses MinCut (ruvector-mincut) to find natural cluster
    /// boundaries in the embedding space. Each partition corresponds
    /// to one person, with:
    /// - A centroid embedding (person signature)
    /// - Member frame indices (which CSI frames belong to this person)
    /// - Separation strength (how distinct this person is from others)
    ///
    /// The MinCut value between partitions serves as a confidence
    /// metric for person separation quality.
    pub fn partition(
        &self,
        person_embeddings: &[CrvSessionEntry],
    ) -> CrvResult<(StageVIData, Vec<f32>)>;
}
```

**Integration point:** The training pipeline in `wifi-densepose-train` already uses `ruvector-mincut` for `DynamicPersonMatcher` (ADR-016). The `PersonPartitioner` wraps this with CRV Stage VI semantics, framing person separation as composite model construction.

### 2.8 Cross-Session Convergence: Multi-Room Identity Matching

**CRV mapping:** CRV convergence analysis compares embeddings from independent sessions targeting the same coordinate to find agreement. In WiFi-DensePose, independent AP clusters in different rooms are independent "viewers" of the same person.

**WiFi domain types:**

```rust
pub struct MultiViewerConvergence {
    session_manager: CrvSessionManager,
}

impl MultiViewerConvergence {
    /// Match person identities across rooms via CRV convergence.
    ///
    /// Each room's AP cluster is modeled as an independent CRV session.
    /// When a person moves from Room A to Room B:
    /// 1. Room A session contains the person's embedding trail (Stages I-VI)
    /// 2. Room B session begins accumulating new embeddings
    /// 3. Convergence analysis finds agreement between Room A's final
    ///    embeddings and Room B's initial embeddings
    /// 4. Agreement score above threshold establishes identity continuity
    ///
    /// Returns ConvergenceResult with:
    /// - Session pairs (room pairs) that converged
    /// - Per-pair similarity scores
    /// - Convergent stages (which CRV stages showed strongest agreement)
    /// - Consensus embedding (merged identity signature)
    pub fn match_across_rooms(
        &self,
        room_sessions: &[(RoomId, SessionId)],
        threshold: f32,
    ) -> CrvResult<ConvergenceResult>;
}
```

**Integration point:** `ruvsense/cross_room.rs` already handles cross-room identity continuity (ADR-030). The `MultiViewerConvergence` wraps the existing `CrossRoomTracker` with CRV convergence semantics, using `CrvSessionManager::find_convergence()` to compute embedding agreement.

### 2.9 WifiCrvSession: Unified Pipeline Wrapper

The top-level wrapper ties all six stages into a single pipeline:

```rust
/// A WiFi-DensePose sensing session modeled as a CRV session.
///
/// Wraps CrvSessionManager with CSI-specific convenience methods.
/// Each call to process_frame() advances through all six CRV stages
/// and appends stage embeddings to the session.
pub struct WifiCrvSession {
    session_manager: CrvSessionManager,
    gestalt: CsiGestaltClassifier,
    sensory: CsiSensoryEncoder,
    topology: MeshTopologyEncoder,
    coherence: CoherenceAolDetector,
    interrogator: PoseInterrogator,
    partitioner: PersonPartitioner,
    convergence: MultiViewerConvergence,
}

impl WifiCrvSession {
    /// Create a new WiFi CRV session with the given configuration.
    pub fn new(config: WifiCrvConfig) -> Self;

    /// Process a single CSI frame through all six CRV stages.
    ///
    /// Returns the per-stage embeddings and the final person partitions.
    pub fn process_frame(
        &mut self,
        frame: &CsiFrame,
        mesh: &MultistaticArray,
        coherence_state: &GateDecision,
        pose_hypothesis: Option<&[f32]>,
    ) -> CrvResult<WifiCrvOutput>;

    /// Find convergence across room sessions for identity matching.
    pub fn find_convergence(
        &self,
        room_sessions: &[(RoomId, SessionId)],
        threshold: f32,
    ) -> CrvResult<ConvergenceResult>;
}
```

---

## 3. Implementation Plan (File-Level)

### 3.1 Phase 1: CRV Module Core (New Files)

| File | Purpose | Upstream Dependency |
|------|---------|-------------------|
| `crates/wifi-densepose-ruvector/src/crv/mod.rs` | Module root, re-exports all CRV-Sense types | -- |
| `crates/wifi-densepose-ruvector/src/crv/config.rs` | `WifiCrvConfig` extending `CrvConfig` with WiFi-specific defaults (128-d instead of 384-d to match AETHER) | `ruvector-crv` |
| `crates/wifi-densepose-ruvector/src/crv/session.rs` | `WifiCrvSession` wrapping `CrvSessionManager` | `ruvector-crv` |
| `crates/wifi-densepose-ruvector/src/crv/output.rs` | `WifiCrvOutput` struct with per-stage embeddings and diagnostics | -- |

### 3.2 Phase 2: Stage Encoders (New Files)

| File | Purpose | Upstream Dependency |
|------|---------|-------------------|
| `crates/wifi-densepose-ruvector/src/crv/gestalt.rs` | `CsiGestaltClassifier` -- Stage I Poincare ball embedding | `ruvector-crv::StageIEncoder` |
| `crates/wifi-densepose-ruvector/src/crv/sensory.rs` | `CsiSensoryEncoder` -- Stage II multi-head attention | `ruvector-crv::StageIIEncoder`, `ruvector-attention` |
| `crates/wifi-densepose-ruvector/src/crv/topology.rs` | `MeshTopologyEncoder` -- Stage III GNN topology | `ruvector-crv::StageIIIEncoder`, `ruvector-gnn` |
| `crates/wifi-densepose-ruvector/src/crv/coherence.rs` | `CoherenceAolDetector` -- Stage IV SNN temporal encoding | `ruvector-crv::StageIVEncoder` |
| `crates/wifi-densepose-ruvector/src/crv/interrogation.rs` | `PoseInterrogator` -- Stage V differentiable search | `ruvector-crv::StageVEngine` |
| `crates/wifi-densepose-ruvector/src/crv/partition.rs` | `PersonPartitioner` -- Stage VI MinCut partitioning | `ruvector-crv::StageVIModeler`, `ruvector-mincut` |

### 3.3 Phase 3: Cross-Session Convergence

| File | Purpose | Upstream Dependency |
|------|---------|-------------------|
| `crates/wifi-densepose-ruvector/src/crv/convergence.rs` | `MultiViewerConvergence` -- cross-room identity matching | `ruvector-crv::CrvSessionManager` |

### 3.4 Phase 4: Integration with Existing Modules (Edits to Existing Files)

| File | Change | Notes |
|------|--------|-------|
| `crates/wifi-densepose-ruvector/src/lib.rs` | Add `pub mod crv;` | Expose new module |
| `crates/wifi-densepose-ruvector/Cargo.toml` | No change needed | `ruvector-crv` dependency already present |
| `crates/wifi-densepose-signal/src/ruvsense/multiband.rs` | Add trait impl for `CrvGestaltSource` | Allow gestalt classifier to consume multiband output |
| `crates/wifi-densepose-signal/src/ruvsense/phase_align.rs` | Add trait impl for `CrvSensorySource` | Allow sensory encoder to consume phase features |
| `crates/wifi-densepose-signal/src/ruvsense/coherence_gate.rs` | Add method to export `GateDecision` history as `Vec<AOLDetection>` | Bridge coherence gate to CRV Stage IV |
| `crates/wifi-densepose-signal/src/ruvsense/cross_room.rs` | Add `CrvConvergenceAdapter` trait impl | Bridge cross-room tracker to CRV convergence |

---

## 4. DDD Design

### 4.1 New Bounded Context: CrvSensing

**Aggregate Root: `WifiCrvSession`**

```rust
pub struct WifiCrvSession {
    /// Underlying CRV session manager
    session_manager: CrvSessionManager,
    /// Per-stage encoders
    stages: CrvStageEncoders,
    /// Session configuration
    config: WifiCrvConfig,
    /// Running statistics for convergence quality
    convergence_stats: ConvergenceStats,
}
```

**Value Objects:**

```rust
/// Output of a single frame through the 6-stage pipeline.
pub struct WifiCrvOutput {
    /// Per-stage embeddings (6 vectors, one per CRV stage).
    pub stage_embeddings: [Vec<f32>; 6],
    /// Gestalt classification for this frame.
    pub gestalt: GestaltType,
    /// AOL detections (frames flagged as noise-contaminated).
    pub aol_events: Vec<AOLDetection>,
    /// Person partitions from Stage VI.
    pub partitions: Vec<TargetPartition>,
    /// Processing latency per stage in microseconds.
    pub stage_latencies_us: [u64; 6],
}

/// WiFi-specific CRV configuration extending CrvConfig.
pub struct WifiCrvConfig {
    /// Base CRV config (dimensions, curvature, thresholds).
    pub crv: CrvConfig,
    /// AETHER embedding dimension (default: 128, overrides CrvConfig.dimensions).
    pub aether_dim: usize,
    /// Coherence threshold for AOL detection (maps to aol_threshold).
    pub coherence_threshold: f32,
    /// Maximum CSI history frames for Stage V interrogation.
    pub max_history_frames: usize,
    /// Cross-room convergence threshold (default: 0.75).
    pub convergence_threshold: f32,
}
```

**Domain Events:**

```rust
pub enum CrvSensingEvent {
    /// Stage I completed: gestalt classified
    GestaltClassified { gestalt: GestaltType, confidence: f32 },
    /// Stage IV: AOL detected (noise contamination)
    AolDetected { anomaly_score: f32, flagged: bool },
    /// Stage VI: Persons partitioned
    PersonsPartitioned { count: usize, min_separation: f32 },
    /// Cross-session: Identity matched across rooms
    IdentityConverged { room_pair: (RoomId, RoomId), score: f32 },
    /// Full pipeline completed for one frame
    FrameProcessed { latency_us: u64, stages_completed: u8 },
}
```

### 4.2 Integration with Existing Bounded Contexts

**Signal (wifi-densepose-signal):** New traits `CrvGestaltSource` and `CrvSensorySource` allow the CRV module to consume signal processing outputs without tight coupling. The signal crate does not depend on the CRV crate -- the dependency flows one direction only.

**Training (wifi-densepose-train):** The `PersonPartitioner` (Stage VI) produces the same MinCut partitions as the existing `DynamicPersonMatcher`. A shared trait `PersonSeparator` allows both to be used interchangeably.

**Hardware (wifi-densepose-hardware):** No changes. The CRV module consumes CSI frames after they have been received and parsed by the hardware layer.

---

## 5. RuVector Integration Map

All seven `ruvector` crates exercised by the CRV-Sense integration:

| CRV Stage | ruvector Crate | API Used | WiFi-DensePose Role |
|-----------|---------------|----------|-------------------|
| I (Gestalt) | -- (internal Poincare math) | `StageIEncoder::encode()` | Hyperbolic embedding of CSI gestalt taxonomy |
| II (Sensory) | `ruvector-attention` | `StageIIEncoder::encode()` | Multi-head attention over subcarrier features |
| III (Dimensional) | `ruvector-gnn` | `StageIIIEncoder::encode()` | GNN encoding of AP mesh topology |
| IV (AOL) | -- (internal SNN) | `StageIVEncoder::encode()` | SNN temporal encoding of coherence violations |
| V (Interrogation) | -- (internal soft attention) | `StageVEngine::search()` | Differentiable search over field model history |
| VI (Composite) | `ruvector-mincut` | `StageVIModeler::partition()` | MinCut person separation |
| Convergence | -- (cosine similarity) | `CrvSessionManager::find_convergence()` | Cross-room identity matching |

Additionally, the CRV module benefits from existing ruvector integrations already in the workspace:

| Existing Integration | ADR | CRV Stage Benefit |
|---------------------|-----|-------------------|
| `ruvector-attn-mincut` in `spectrogram.rs` | ADR-016 | Stage II (subcarrier attention for sensory features) |
| `ruvector-temporal-tensor` in `dataset.rs` | ADR-016 | Stage IV (compressed coherence history buffer) |
| `ruvector-solver` in `subcarrier.rs` | ADR-016 | Stage III (sparse interpolation for mesh topology) |
| `ruvector-attention` in `model.rs` | ADR-016 | Stage V (spatial attention for pose interrogation) |
| `ruvector-mincut` in `metrics.rs` | ADR-016 | Stage VI (person matching baseline) |

---

## 6. Acceptance Criteria

### 6.1 Stage I: CSI Gestalt Classification

| ID | Criterion | Test Method |
|----|-----------|-------------|
| S1-1 | `CsiGestaltClassifier::classify()` returns a valid `GestaltType` for any well-formed CSI frame | Unit test: feed 100 synthetic CSI frames, verify all return one of 6 gestalt types |
| S1-2 | Poincare ball embedding has correct dimensionality (matching `WifiCrvConfig.aether_dim`) | Unit test: verify `embedding.len() == config.aether_dim` |
| S1-3 | Embedding norm is strictly less than 1.0 (Poincare ball constraint) | Unit test: verify L2 norm < 1.0 for all outputs |
| S1-4 | Movement gestalt is classified for CSI frames with Doppler signature | Unit test: synthetic Doppler-shifted CSI -> `GestaltType::Movement` |
| S1-5 | Energy gestalt is classified for CSI frames with transient interference | Unit test: synthetic interference burst -> `GestaltType::Energy` |

### 6.2 Stage II: CSI Sensory Features

| ID | Criterion | Test Method |
|----|-----------|-------------|
| S2-1 | `CsiSensoryEncoder::encode()` produces embedding of correct dimensionality | Unit test: verify output length |
| S2-2 | Amplitude variance maps to Texture modality in `StageIIData.impressions` | Unit test: verify Texture entry present for non-flat amplitude |
| S2-3 | Phase drift rate maps to Temperature modality | Unit test: inject linear phase drift, verify Temperature entry |
| S2-4 | Multi-head attention weights sum to 1.0 per head | Unit test: verify softmax normalization |

### 6.3 Stage III: AP Mesh Topology

| ID | Criterion | Test Method |
|----|-----------|-------------|
| S3-1 | `MeshTopologyEncoder::encode()` produces one `SketchElement` per AP node | Unit test: 4-node mesh produces 4 sketch elements |
| S3-2 | `SpatialRelationship` count equals number of bistatic links | Unit test: 4 nodes -> 6 links (fully connected) or configured subset |
| S3-3 | Relationship strength is proportional to link SNR | Unit test: verify monotonic relationship between SNR and strength |
| S3-4 | GNN embedding changes when node positions change | Unit test: perturb one node position, verify embedding changes |

### 6.4 Stage IV: Coherence AOL Detection

| ID | Criterion | Test Method |
|----|-----------|-------------|
| S4-1 | `CoherenceAolDetector::detect()` flags low-coherence frames as AOL events | Unit test: inject 10 `GateDecision::Reject` frames, verify 10 `AOLDetection` entries |
| S4-2 | Anomaly score correlates with coherence violation burst length | Unit test: burst of 5 violations scores higher than isolated violation |
| S4-3 | `GateDecision::Accept` frames produce no AOL detections | Unit test: all-accept history produces empty AOL list |
| S4-4 | SNN temporal encoding respects refractory period | Unit test: two violations within `refractory_period_ms` produce single spike |
| S4-5 | `GateDecision::ForcedAccept` (ADR-032) maps to AOL with moderate score | Unit test: forced accept frames flagged but not at max anomaly score |

### 6.5 Stage V: Pose Interrogation

| ID | Criterion | Test Method |
|----|-----------|-------------|
| S5-1 | `PoseInterrogator::interrogate()` returns attention weights over CSI history | Unit test: history of 50 frames produces 50 attention weights summing to 1.0 |
| S5-2 | Top-k candidates are the highest-attention frames | Unit test: verify `top_candidates` indices correspond to highest `attention_weights` |
| S5-3 | Cross-references link correct stage numbers | Unit test: verify `from_stage` and `to_stage` are in [1..6] |
| S5-4 | Empty history returns empty probe results | Unit test: empty `csi_history` produces zero candidates |

### 6.6 Stage VI: Person Partitioning

| ID | Criterion | Test Method |
|----|-----------|-------------|
| S6-1 | `PersonPartitioner::partition()` separates two well-separated embedding clusters into two partitions | Unit test: two Gaussian clusters with distance > 5 sigma -> two partitions |
| S6-2 | Each partition has a centroid embedding of correct dimensionality | Unit test: verify centroid length matches config |
| S6-3 | `separation_strength` (MinCut value) is positive for distinct persons | Unit test: verify separation_strength > 0.0 |
| S6-4 | Single-person scenario produces exactly one partition | Unit test: single cluster -> one partition |
| S6-5 | Partition `member_entries` indices are non-overlapping and exhaustive | Unit test: union of all member entries covers all input frames |

### 6.7 Cross-Session Convergence

| ID | Criterion | Test Method |
|----|-----------|-------------|
| C-1 | `MultiViewerConvergence::match_across_rooms()` returns positive score for same person in two rooms | Unit test: inject same embedding trail into two room sessions, verify score > threshold |
| C-2 | Different persons in different rooms produce score below threshold | Unit test: inject distinct embedding trails, verify score < threshold |
| C-3 | `convergent_stages` identifies the stage with highest cross-room agreement | Unit test: make Stage I embeddings identical, others random, verify Stage I in convergent_stages |
| C-4 | `consensus_embedding` has correct dimensionality when convergence succeeds | Unit test: verify consensus embedding length on successful match |
| C-5 | Threshold parameter is respected (no matches below threshold) | Unit test: set threshold to 0.99, verify only near-identical sessions match |

### 6.8 End-to-End Pipeline

| ID | Criterion | Test Method |
|----|-----------|-------------|
| E-1 | `WifiCrvSession::process_frame()` returns `WifiCrvOutput` with all 6 stage embeddings populated | Integration test: process 10 synthetic frames, verify 6 non-empty embeddings per frame |
| E-2 | Total pipeline latency < 5 ms per frame on x86 host | Benchmark: process 1000 frames, verify p95 latency < 5 ms |
| E-3 | Pipeline handles missing pose hypothesis gracefully (Stage V skipped or uses default) | Unit test: pass `None` for pose_hypothesis, verify no panic and output is valid |
| E-4 | Pipeline handles empty mesh (single AP) without panic | Unit test: single-node mesh produces valid output with degenerate Stage III |
| E-5 | Session state accumulates across frames (Stage V history grows) | Unit test: process 50 frames, verify Stage V candidate count increases |

---

## 7. Consequences

### 7.1 Positive

- **Structured pipeline formalization**: The 6-stage CRV mapping provides a principled progressive refinement structure for the WiFi sensing pipeline, making the data flow explicit and each stage independently testable.
- **Cross-room identity without cameras**: CRV convergence analysis provides a mathematically grounded mechanism for matching person identities across AP clusters in different rooms, using only RF embeddings.
- **Noise separation as first-class concept**: Mapping coherence gating to CRV Stage IV (AOL detection) elevates noise separation from an implementation detail to a core architectural stage with its own embedding and temporal model.
- **Hyperbolic embeddings for gestalt hierarchy**: The Poincare ball embedding for Stage I captures the hierarchical RF environment taxonomy (Manmade > structural multipath, Natural > diffuse scattering, etc.) with exponentially less distortion than Euclidean space.
- **Reuse of ruvector ecosystem**: All seven ruvector crates are exercised through a single unified abstraction, maximizing the return on the existing ruvector integration (ADR-016).
- **No new external dependencies**: `ruvector-crv` is already a workspace dependency in `wifi-densepose-ruvector/Cargo.toml`. This ADR adds only new Rust source files.

### 7.2 Negative

- **Abstraction overhead**: The CRV stage mapping adds a layer of indirection over the existing signal processing pipeline. Each stage wrapper must translate between WiFi domain types and CRV types, adding code that could be a maintenance burden if the mapping proves ill-fitted.
- **Dimensional mismatch**: `ruvector-crv` defaults to 384 dimensions; AETHER embeddings (ADR-024) use 128 dimensions. The `WifiCrvConfig` overrides this, but encoder behavior at non-default dimensionality must be validated.
- **SNN overhead**: The Stage IV SNN temporal encoder adds per-frame computation for spike train simulation. On embedded targets (ESP32), this may exceed the 50 ms frame budget. Initial deployment is host-side only (aggregator, not firmware).
- **Convergence false positives**: Cross-room identity matching via embedding similarity may produce false matches for persons with similar body types and movement patterns in similar room geometries. Temporal proximity constraints (from ADR-030) are required to bound the false positive rate.
- **Testing complexity**: Six stages with independent encoders and a cross-session convergence layer require a comprehensive test matrix. The acceptance criteria in Section 6 define 30+ individual test cases.

### 7.3 Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Poincare ball embedding unstable at boundary (norm approaching 1.0) | Medium | NaN propagation through pipeline | Clamp norm to 0.95 in `CsiGestaltClassifier`; add norm assertion in test suite |
| GNN encoder too slow for real-time mesh topology updates | Low | Stage III becomes bottleneck | Cache topology embedding; only recompute on node geometry change (rare) |
| SNN refractory period too short for 20 Hz coherence gate | Medium | False AOL detections at frame boundaries | Tune `refractory_period_ms` to match frame interval (50 ms) in `WifiCrvConfig` defaults |
| Cross-room convergence threshold too permissive | Medium | False identity matches across rooms | Default threshold 0.75 is conservative; ADR-030 temporal proximity constraint (<60s) adds second guard |
| MinCut partitioning produces too many or too few person clusters | Medium | Person count mismatch | Use expected person count hint (from occupancy detector) as MinCut constraint |
| CRV abstraction becomes tech debt if mapping proves poor fit | Low | Code removed in future ADR | All CRV code in isolated `crv` module; can be removed without affecting existing pipeline |

---

## 8. Related ADRs

| ADR | Relationship |
|-----|-------------|
| ADR-016 (RuVector Integration) | **Extended**: All 5 original ruvector crates plus `ruvector-crv` and `ruvector-gnn` now exercised through CRV pipeline |
| ADR-017 (RuVector Signal+MAT) | **Extended**: Signal processing outputs from ADR-017 feed into CRV Stages I-II |
| ADR-024 (AETHER Embeddings) | **Consumed**: Per-viewpoint AETHER 128-d embeddings are the representation fed into CRV stages |
| ADR-029 (RuvSense Multistatic) | **Extended**: Multistatic mesh topology encoded as CRV Stage III; TDM frames are the input to Stage I |
| ADR-030 (Persistent Field Model) | **Extended**: Field model history serves as the Stage V interrogation corpus; cross-room tracker bridges to CRV convergence |
| ADR-031 (RuView Viewpoint Fusion) | **Complementary**: RuView fuses viewpoints within a room; CRV convergence matches identities across rooms |
| ADR-032 (Mesh Security) | **Consumed**: Authenticated beacons and frame integrity (ADR-032) ensure CRV Stage IV AOL detection reflects genuine signal quality, not spoofed frames |

---

## 9. References

1. Swann, I. (1996). "Remote Viewing: The Real Story." Self-published manuscript. (Original CRV protocol documentation.)
2. Smith, P. H. (2005). "Reading the Enemy's Mind: Inside Star Gate, America's Psychic Espionage Program." Tom Doherty Associates.
3. Nickel, M. & Kiela, D. (2017). "Poincare Embeddings for Learning Hierarchical Representations." NeurIPS 2017.
4. Kipf, T. N. & Welling, M. (2017). "Semi-Supervised Classification with Graph Convolutional Networks." ICLR 2017.
5. Maass, W. (1997). "Networks of Spiking Neurons: The Third Generation of Neural Network Models." Neural Networks, 10(9):1659-1671.
6. Stoer, M. & Wagner, F. (1997). "A Simple Min-Cut Algorithm." Journal of the ACM, 44(4):585-591.
7. `ruvector-crv` v0.1.1. https://crates.io/crates/ruvector-crv
8. `ruvector-attention` v2.0. https://crates.io/crates/ruvector-attention
9. `ruvector-gnn` v2.0.1. https://crates.io/crates/ruvector-gnn
10. `ruvector-mincut` v2.0.1. https://crates.io/crates/ruvector-mincut
11. Geng, J. et al. (2023). "DensePose From WiFi." arXiv:2301.00250.
12. ADR-016 through ADR-032 (internal).
