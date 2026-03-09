# Temporal Graph Evolution Tracking and RuVector Integration for RF Topological Sensing

**Research Document 08** | March 2026
**Status**: SOTA Survey + Design Proposal
**Scope**: Temporal dynamic graph models applied to WiFi CSI-based RF sensing,
with concrete integration points into the RuView/wifi-densepose Rust codebase.

---

## Table of Contents

1. [Introduction and Motivation](#1-introduction-and-motivation)
2. [Temporal Graph Models: SOTA Survey](#2-temporal-graph-models-sota-survey)
3. [RuVector as Graph Memory](#3-ruvector-as-graph-memory)
4. [Graph Evolution Patterns in RF Sensing](#4-graph-evolution-patterns-in-rf-sensing)
5. [Minimum Cut Trajectory Tracking](#5-minimum-cut-trajectory-tracking)
6. [Event Detection from Graph Dynamics](#6-event-detection-from-graph-dynamics)
7. [Compressed Temporal Storage](#7-compressed-temporal-storage)
8. [Cross-Room Transition Graphs](#8-cross-room-transition-graphs)
9. [Longitudinal Drift Detection on Graph Topology](#9-longitudinal-drift-detection-on-graph-topology)
10. [Proposed Data Structures](#10-proposed-data-structures)
11. [Integration Roadmap](#11-integration-roadmap)
12. [References](#12-references)

---

## 1. Introduction and Motivation

WiFi-based sensing produces a rich, continuously evolving graph structure.
Each ESP32 node is a vertex; each TX-RX link is an edge carrying time-varying
Channel State Information (CSI). People, furniture, doors, and environmental
conditions perturb this graph in characteristic patterns. Tracking *how* the
graph changes over time -- not just the current snapshot -- unlocks several
capabilities that static analysis cannot provide:

- **Trajectory reconstruction** from the movement of minimum-cut boundaries.
- **Event classification** (entry, exit, gesture, fall) from graph dynamics.
- **Longitudinal health monitoring** by tracking topological drift over weeks.
- **Cross-room identity continuity** through temporal transition graphs.
- **Anomaly detection** when graph evolution violates learned patterns.

This document surveys state-of-the-art temporal graph models, then designs
concrete data structures and algorithms for integrating temporal graph
evolution tracking into the RuView codebase via RuVector's graph engine.

### 1.1 Scope Boundaries

This research covers the RF sensing graph specifically -- the graph whose
vertices are ESP32 nodes and whose edges are CSI links. It does not address
the DensePose skeleton graph (which is a separate, downstream structure).
The two graphs interact at the fusion boundary where `MultistaticArray`
(in `ruvector/src/viewpoint/fusion.rs`) produces fused embeddings from
the RF graph and the pose tracker (in `signal/src/ruvsense/pose_tracker.rs`)
consumes them.

### 1.2 Relationship to Existing Modules

| Module | Current Role | Temporal Extension |
|--------|-------------|-------------------|
| `coherence.rs` | Per-link coherence scoring | Coherence time series per edge |
| `field_model.rs` | SVD eigenstructure (static) | Eigenmode drift trajectories |
| `multistatic.rs` | Single-cycle fusion | Cross-cycle graph state memory |
| `cross_room.rs` | Transition event log | Temporal transition graph |
| `longitudinal.rs` | Welford stats per person | Welford stats per graph metric |
| `coherence_gate.rs` | Accept/Reject decisions | Gate decision history analysis |
| `viewpoint/fusion.rs` | Aggregate root for fusion | Temporal GDI tracking |
| `viewpoint/geometry.rs` | GDI + Cramer-Rao bounds | Time-varying geometry quality |
| `intention.rs` | Embedding acceleration | Graph-level acceleration detection |

---

## 2. Temporal Graph Models: SOTA Survey

### 2.1 Taxonomy of Temporal Graph Representations

Temporal graphs fall into two broad families:

**Discrete-Time Dynamic Graphs (DTDGs)**: The graph is represented as a
sequence of snapshots G_1, G_2, ..., G_T at fixed time intervals.

```
DTDG State Diagram:

  [Snapshot t-2] --delta--> [Snapshot t-1] --delta--> [Snapshot t]
       |                         |                         |
       v                         v                         v
   {V, E, W}_{t-2}          {V, E, W}_{t-1}          {V, E, W}_t

  Where each snapshot contains:
    V = vertex set (ESP32 nodes, typically stable)
    E = edge set (active links, may vary with node failures)
    W = edge weights (CSI amplitude/phase/coherence)
```

**Continuous-Time Dynamic Graphs (CTDGs)**: Events (edge additions,
deletions, weight changes) are recorded as a timestamped event stream.

```
CTDG Event Stream:

  t=0.000  EdgeUpdate(A->B, coherence=0.95)
  t=0.050  EdgeUpdate(A->C, coherence=0.91)
  t=0.050  EdgeUpdate(B->C, coherence=0.88)
  t=0.100  EdgeUpdate(A->B, coherence=0.72)  <-- person crosses link
  t=0.100  EdgeUpdate(B->D, coherence=0.93)
  t=0.150  EdgeUpdate(A->B, coherence=0.45)  <-- strong perturbation
  ...
```

For RuView's 20 Hz TDMA cycle, the DTDG snapshot model aligns naturally
with the `MultistaticFuser` output cadence. However, within a single TDMA
cycle the individual node frames arrive asynchronously (per
`MultistaticConfig::guard_interval_us`), making a hybrid approach optimal:
DTDG at the cycle level, CTDG for intra-cycle event recording.

### 2.2 Key Frameworks

#### 2.2.1 Temporal Graph Networks (TGN)

Rossi et al. (2020) introduced TGN as a unified framework combining:

- **Memory module**: Per-node memory vectors updated after each interaction.
- **Message function**: Computes messages from temporal events.
- **Message aggregator**: Combines messages for nodes with multiple events.
- **Embedding module**: Generates node embeddings from memory + graph.

TGN's per-node memory maps directly to the per-link `CoherenceState` in
`coherence.rs`. The EMA reference template is effectively a memory vector
that encodes the link's recent history. The `DriftProfile` enum
(Stable/Linear/StepChange) serves as a coarse embedding.

**Relevance to RuView**: TGN's memory update mechanism can be adapted for
our per-edge CSI state. Rather than learning memory updates via
backpropagation, we use physics-informed updates (Welford statistics,
EMA reference tracking) that are deterministic and auditable.

#### 2.2.2 JODIE (Joint Dynamic User-Item Embeddings)

Kumar et al. (2019) model interactions between two types of nodes using
coupled RNN-based projections. Each interaction updates both nodes'
embeddings and projects them forward in time.

**Relevance to RuView**: The TX-RX duality in our multistatic mesh is
analogous to JODIE's user-item pairs. When person P crosses link L(A,B),
we can update both the "transmitter A state" and "receiver B state"
simultaneously, projecting both forward to the next expected observation.

#### 2.2.3 CT-DGNN (Continuous-Time Dynamic Graph Neural Network)

Chen et al. (2021) use temporal point processes to model irregularly-sampled
graph events. Edge events are modeled as a Hawkes process with learned
triggering kernels.

**Relevance to RuView**: The coherence gate decision stream
(Accept/PredictOnly/Reject/Recalibrate from `coherence_gate.rs`) is
naturally a point process. Gate transitions from Accept to Reject cluster
in time during person movement, exhibiting the self-exciting behavior that
Hawkes processes capture.

#### 2.2.4 DyRep (Learning Representations over Dynamic Graphs)

Trivedi et al. (2019) model two processes jointly: association (structural
changes) and communication (information flow). The temporal attention
mechanism weighs recent events more heavily.

**Relevance to RuView**: The `CrossViewpointAttention` module in
`viewpoint/attention.rs` already implements geometric bias via
`GeometricBias::new(w_angle, w_dist, d_ref)`. DyRep suggests adding
temporal bias: more recent viewpoint observations should receive higher
attention weight.

### 2.3 Comparison Matrix

| Framework | Time Model | Memory | Scalability | RuView Fit |
|-----------|-----------|--------|-------------|-----------|
| TGN | Continuous | Per-node | O(N) update | High -- maps to CoherenceState |
| JODIE | Continuous | Per-pair | O(E) update | Medium -- TX-RX pairs |
| CT-DGNN | Continuous | Global | O(N^2) attention | Low -- too expensive at 20 Hz |
| DyRep | Continuous | Per-node | O(N*K) | Medium -- temporal attention useful |
| GraphSAGE-T | Discrete | Aggregated | O(N*K*L) | High -- snapshot aggregation |

### 2.4 Recommended Hybrid Approach

For RuView, we propose a **snapshot-anchored event-driven** model:

1. **Anchor snapshots** at each TDMA cycle (20 Hz) capturing the full graph
   state (all link coherences, amplitudes, phases).
2. **Between anchors**, record edge-level events (coherence drops, gate
   decisions, perturbation detections) as a CTDG event stream.
3. **Memory** is maintained per-edge using the existing `CoherenceState`
   and `WelfordStats`, extended with temporal query capabilities.
4. **Attention** uses the existing `CrossViewpointAttention` with an
   additional temporal decay term.

This avoids the computational overhead of full neural temporal graph models
while preserving the event-level granularity needed for gesture detection
and intention lead signals.

---

## 3. RuVector as Graph Memory

### 3.1 Current RuVector Graph Capabilities

RuVector provides five crates relevant to graph memory:

| Crate | Graph Primitive | Temporal Capability |
|-------|----------------|-------------------|
| `ruvector-mincut` | Dynamic min-cut partitioning | Per-frame partition snapshots |
| `ruvector-attn-mincut` | Attention-gated spectrogram | Spectral evolution tracking |
| `ruvector-temporal-tensor` | CompressedCsiBuffer | Ring-buffer CSI history |
| `ruvector-solver` | Sparse matrix (CsrMatrix) | System state solving |
| `ruvector-attention` | Spatial attention weights | Attention weight trajectories |

### 3.2 Vertex and Edge Versioning

To support temporal queries ("what was the coherence of link A-B at time
T?"), we need versioned graph state. The design follows an append-only
event sourcing pattern consistent with the project's DDD architecture.

```
Vertex/Edge Version Model:

  VertexState {
    node_id: NodeId,
    version: u64,          // Monotonic version counter
    timestamp_us: u64,     // Wall clock at version creation
    embedding: Vec<f32>,   // AETHER embedding (128-d)
    coherence_score: f32,  // Aggregate coherence
    calibration_status: CalibrationStatus,
  }

  EdgeState {
    link_id: (NodeId, NodeId),
    version: u64,
    timestamp_us: u64,
    coherence: f32,         // From CoherenceState::score()
    drift_profile: DriftProfile,
    gate_decision: GateDecision,
    amplitude_hash: u64,    // Compact representation of full CSI
    perturbation_energy: f64,
  }
```

### 3.3 Temporal Query Interface

```rust
/// Temporal graph query interface for RF sensing graph.
pub trait TemporalGraphQuery {
    /// Get the graph state at a specific timestamp.
    fn snapshot_at(&self, timestamp_us: u64) -> Option<GraphSnapshot>;

    /// Get edge state history within a time range.
    fn edge_history(
        &self,
        link: (NodeId, NodeId),
        start_us: u64,
        end_us: u64,
    ) -> Vec<EdgeState>;

    /// Get all edges that changed between two timestamps.
    fn diff(&self, t1_us: u64, t2_us: u64) -> GraphDelta;

    /// Find the first timestamp where a predicate holds.
    fn find_first<P: Fn(&GraphSnapshot) -> bool>(
        &self,
        start_us: u64,
        predicate: P,
    ) -> Option<u64>;

    /// Aggregate a metric over a time window.
    fn aggregate_window(
        &self,
        link: (NodeId, NodeId),
        metric: EdgeMetric,
        window_us: u64,
    ) -> WelfordStats;
}
```

### 3.4 Integration with Existing Memory Stores

The `EmbeddingHistory` in `longitudinal.rs` already implements a brute-force
nearest-neighbor store. We extend this pattern to graph state:

```
Memory Architecture:

  +------------------+     +-------------------+     +------------------+
  | CoherenceState   |     | TemporalGraphStore|     | EmbeddingHistory |
  | (per-edge, live) |---->| (versioned, disk) |<----| (per-person)     |
  +------------------+     +-------------------+     +------------------+
          |                        |                         |
          v                        v                         v
  [20 Hz live feed]        [Queryable history]       [HNSW-indexed]
                                   |
                           +-------+-------+
                           |               |
                     [Snapshot Index]  [Event Stream]
                     (binary search)   (append-only)
```

### 3.5 Storage Budget

For a 6-node mesh with 15 bidirectional links at 20 Hz:

| Component | Per-Frame | Per-Second | Per-Hour | Per-Day |
|-----------|-----------|-----------|----------|---------|
| Edge coherence (15 x f32) | 60 B | 1.2 KB | 4.3 MB | 103 MB |
| Edge amplitude hash (15 x u64) | 120 B | 2.4 KB | 8.6 MB | 207 MB |
| Gate decisions (15 x u8) | 15 B | 300 B | 1.1 MB | 26 MB |
| Full snapshot (anchor) | ~2 KB | 40 KB | 144 MB | 3.4 GB |
| Delta (inter-anchor) | ~200 B | 4 KB | 14 MB | 340 MB |

With delta compression (Section 7), the per-day cost drops to approximately
100 MB for full temporal history, well within ESP32 aggregator SD card limits.

---

## 4. Graph Evolution Patterns in RF Sensing

### 4.1 Pattern Taxonomy

RF field graphs exhibit characteristic evolution patterns during different
physical events. We classify these as **temporal motifs** -- recurring
subgraph evolution signatures.

```
Temporal Motif State Machine:

                     +----------+
                     |  Static  |
                     | (Stable) |
                     +----+-----+
                          |
          +---------------+---------------+
          |               |               |
          v               v               v
    +-----------+   +-----------+   +-----------+
    | Single    |   | Multi     |   | Global    |
    | Link Drop |   | Link Drop |   | Shift     |
    +-----------+   +-----------+   +-----------+
    Person crosses  Person in       Environmental
    one link        open area       change (door,
          |               |         HVAC, etc.)
          v               v               v
    +-----------+   +-----------+   +-----------+
    | Sweep     |   | Cluster   |   | Offset    |
    | Pattern   |   | Migration |   | Plateau   |
    +-----------+   +-----------+   +-----------+
    Sequential      Correlated      All links shift
    link drops      group moves     to new baseline
          |               |               |
          +-------+-------+               |
                  |                        |
                  v                        v
           +-----------+           +-----------+
           | Recovery  |           | New        |
           | to Static |           | Baseline   |
           +-----------+           +-----------+
```

### 4.2 Person Walking Across a Room

When a person walks from position P1 to P2, the RF graph evolves in a
characteristic sweep pattern:

1. **Pre-movement phase** (200-500 ms): Subtle coherence shifts detected by
   the `IntentionDetector` in `intention.rs`. The embedding acceleration
   exceeds the threshold while velocity remains low.

2. **Leading edge**: Links nearest to the person's current position show
   coherence drops first. The `CoherenceState` transitions from `Stable`
   to `StepChange` drift profile.

3. **Body zone**: Links directly traversing the person show minimum coherence
   and maximum perturbation energy (from `FieldModel::extract_perturbation`).

4. **Trailing recovery**: Links the person has passed recover coherence,
   transitioning back to `Stable` drift profile.

The temporal signature is a **traveling wave of coherence depression** that
sweeps across the graph in the direction of movement.

```
Coherence Evolution During Walk (6-link example):

Time -->  0s    0.5s   1.0s   1.5s   2.0s   2.5s
Link A-B: 0.95  0.95   0.92   0.45   0.88   0.94
Link A-C: 0.93  0.91   0.50   0.82   0.93   0.95
Link B-C: 0.94  0.55   0.78   0.92   0.94   0.93
Link B-D: 0.92  0.48   0.72   0.91   0.93   0.94
Link C-D: 0.95  0.93   0.88   0.52   0.85   0.93
Link A-D: 0.94  0.92   0.78   0.60   0.55   0.90

                  ^ sweep starts   ^ sweep peak   ^ recovery
```

### 4.3 Door Opening/Closing

A door event produces a **global step change** in the graph:

1. Links traversing the door aperture show sudden, large coherence drops.
2. Links not traversing the door show smaller, delayed coherence shifts
   (due to changed multipath structure).
3. The new coherence pattern stabilizes at a **different baseline** from
   the pre-door state.

The `FieldModel` eigenstructure changes because the room's electromagnetic
boundary conditions have changed. The environmental modes shift, requiring
recalibration (detected by `CalibrationStatus::Stale` or `Expired`).

### 4.4 Environmental Shift (HVAC, Temperature)

Slow environmental changes produce a **linear drift** pattern:

1. All links show gradual, correlated coherence changes over minutes/hours.
2. The `DriftProfile::Linear` classification activates.
3. The `FieldNormalMode` environmental projection magnitude increases.
4. Per-link Welford statistics track the drift rate.

This pattern is distinct from person-caused changes because:
- It affects all links simultaneously (not a traveling wave).
- The drift rate is slow (sub-Hz) compared to body motion (0.5-5 Hz).
- The eigenmode projection captures most of the change (high `variance_explained`).

### 4.5 Temporal Motif Detection Algorithm

```rust
/// Temporal motif classifier for RF graph evolution.
pub struct TemporalMotifClassifier {
    /// Per-link coherence history (ring buffer, 10 seconds at 20 Hz).
    link_histories: Vec<RingBuffer<f32>>,  // [n_links][200]
    /// Cross-correlation matrix of link coherence changes.
    cross_correlation: Vec<Vec<f32>>,       // [n_links][n_links]
    /// Detected motif patterns.
    active_motifs: Vec<ActiveMotif>,
}

/// A detected temporal motif in the graph evolution.
pub struct ActiveMotif {
    /// Motif type.
    pub motif_type: MotifType,
    /// Links involved in this motif.
    pub affected_links: Vec<(NodeId, NodeId)>,
    /// Start timestamp.
    pub start_us: u64,
    /// Current phase of the motif.
    pub phase: MotifPhase,
    /// Confidence (0.0-1.0).
    pub confidence: f32,
    /// Estimated velocity (for sweep motifs, m/s).
    pub estimated_velocity: Option<f32>,
}

pub enum MotifType {
    /// Sequential coherence drops along a path.
    Sweep { direction: [f32; 2] },
    /// Correlated drops in a spatial cluster.
    ClusterDrop,
    /// All links shift simultaneously.
    GlobalShift,
    /// Single isolated link perturbation.
    Isolated,
}

pub enum MotifPhase {
    Leading,
    Peak,
    Trailing,
    Recovery,
}
```

---

## 5. Minimum Cut Trajectory Tracking

### 5.1 Background: Min-Cut in RF Graphs

The `ruvector-mincut` crate provides `DynamicMinCut` for partitioning the
CSI correlation graph into person clusters (`PersonCluster` in
`multistatic.rs`). At each TDMA cycle, the min-cut boundary separates
regions of the graph associated with different people.

### 5.2 Cut Boundary as a Spatial Contour

The min-cut boundary in the RF graph corresponds to a physical contour in
the room. Each cut edge (link) has a known geometry (from node positions),
so the cut boundary can be projected into 2D room coordinates.

```
Min-Cut Boundary Projection:

  Graph Space:                Room Space:

  A ----[cut]---- B           A(0,0) ........... B(5,0)
  |               |           .    +---------+   .
  |   Person 1    |           .    | Person  |   .
  |               |           .    | Region  |   .
  C ----[cut]---- D           .    +---------+   .
                              C(0,5) ........... D(5,5)

  Cut edges: A-B, C-D        Cut contour: horizontal line at y~2.5
```

### 5.3 Kalman Filtering of Graph Partitions

To track smooth person trajectories from noisy min-cut outputs, we apply
Kalman filtering to the cut boundary parameters:

```rust
/// Kalman-filtered min-cut boundary tracker.
pub struct CutBoundaryTracker {
    /// State: [centroid_x, centroid_y, velocity_x, velocity_y, area].
    state: [f64; 5],
    /// 5x5 covariance matrix (upper triangle, 15 elements).
    covariance: [f64; 15],
    /// Process noise (acceleration variance).
    process_noise: f64,
    /// Measurement noise (cut boundary estimation variance).
    measurement_noise: f64,
    /// Track ID linking to pose_tracker TrackId.
    track_id: u64,
    /// History of filtered centroids for trajectory extraction.
    trajectory: VecDeque<(u64, [f64; 2])>,  // (timestamp_us, [x, y])
}

impl CutBoundaryTracker {
    /// Predict step: advance state by dt seconds.
    pub fn predict(&mut self, dt: f64) {
        // Constant velocity model
        self.state[0] += self.state[2] * dt;  // x += vx * dt
        self.state[1] += self.state[3] * dt;  // y += vy * dt
        // Covariance prediction: P = F*P*F' + Q
        // (simplified: add process noise to velocity components)
    }

    /// Update step: incorporate new min-cut boundary measurement.
    pub fn update(&mut self, measurement: &CutBoundaryMeasurement) {
        // Kalman gain, state update, covariance update
        // Measurement model: observe centroid_x, centroid_y, area
    }

    /// Extract the smoothed trajectory over the last N seconds.
    pub fn trajectory(&self, duration_us: u64) -> &[(u64, [f64; 2])] {
        // Return from self.trajectory deque
        &[]  // placeholder
    }
}

/// Measurement from a single min-cut partition.
pub struct CutBoundaryMeasurement {
    /// Centroid of the partition in room coordinates.
    pub centroid: [f64; 2],
    /// Estimated area of the partition (square metres).
    pub area: f64,
    /// Number of cut edges (higher = more confident boundary).
    pub n_cut_edges: usize,
    /// Mean coherence of cut edges (lower = stronger signal).
    pub mean_cut_coherence: f32,
}
```

### 5.4 Smooth Interpolation of Cut Boundaries

Between TDMA cycles (50 ms intervals), the cut boundary position can be
interpolated using the Kalman velocity estimate:

```
Interpolation Timeline:

  Cycle N          Cycle N+1        Cycle N+2
  |                |                |
  v                v                v
  [Measurement]    [Measurement]    [Measurement]
  |    ^    ^    ^ |    ^    ^    ^ |
  |    |    |    | |    |    |    | |
  | Interpolated positions at 5ms intervals |
  | using Kalman velocity prediction        |
```

This gives the sensing-server UI (in `wifi-densepose-sensing-server`) a
smooth 200 Hz rendering of person positions even though the underlying
measurements arrive at 20 Hz.

### 5.5 Multi-Person Cut Tracking

For K persons, the min-cut produces K partitions. Each partition is tracked
by a separate `CutBoundaryTracker`. The assignment of partitions to trackers
across frames uses the Hungarian algorithm (already available via
`ruvector-mincut::DynamicPersonMatcher`).

```
Multi-Person State Diagram:

  [Partition Detection]
        |
        v
  [Assignment] <-- Hungarian algorithm (DynamicPersonMatcher)
        |
    +---+---+---+
    |       |       |
    v       v       v
  [Track 1] [Track 2] [Track 3]
  Kalman    Kalman    Kalman
  filter    filter    filter
    |       |       |
    v       v       v
  [Smoothed Trajectories]
```

---

## 6. Event Detection from Graph Dynamics

### 6.1 Change-Point Detection on Graph Time Series

Discrete events (person entry, exit, gesture, fall) manifest as change
points in the graph evolution. We detect these using three complementary
methods:

#### 6.1.1 CUSUM (Cumulative Sum) on Coherence

```rust
/// CUSUM change-point detector for per-link coherence.
pub struct CusumDetector {
    /// Target mean (expected coherence under null hypothesis).
    target: f64,
    /// Allowable slack before triggering.
    slack: f64,
    /// Detection threshold.
    threshold: f64,
    /// Cumulative sum (positive direction).
    s_pos: f64,
    /// Cumulative sum (negative direction).
    s_neg: f64,
    /// Frame count since last reset.
    frame_count: u64,
}

impl CusumDetector {
    pub fn update(&mut self, value: f64) -> Option<ChangePoint> {
        self.frame_count += 1;
        let deviation = value - self.target;

        self.s_pos = (self.s_pos + deviation - self.slack).max(0.0);
        self.s_neg = (self.s_neg - deviation - self.slack).max(0.0);

        if self.s_pos > self.threshold {
            let cp = ChangePoint {
                frame: self.frame_count,
                direction: ChangeDirection::Increasing,
                magnitude: self.s_pos,
            };
            self.s_pos = 0.0;
            return Some(cp);
        }
        if self.s_neg > self.threshold {
            let cp = ChangePoint {
                frame: self.frame_count,
                direction: ChangeDirection::Decreasing,
                magnitude: self.s_neg,
            };
            self.s_neg = 0.0;
            return Some(cp);
        }
        None
    }
}
```

#### 6.1.2 Graph Spectral Analysis

Changes in the graph's Laplacian eigenvalues indicate topological shifts:

- **Fiedler value** (second-smallest eigenvalue of the Laplacian) drops when
  the graph becomes easier to partition (person creating a bottleneck).
- **Spectral gap** changes indicate connectivity shifts.
- **Eigenvalue tracking** over time reveals smooth vs. sudden transitions.

The existing `FieldModel` SVD in `field_model.rs` computes eigenvalues of
the CSI covariance. Extending this to the graph Laplacian requires building
the Laplacian from the `CoherenceState` of all links:

```rust
/// Build the coherence-weighted Laplacian of the RF sensing graph.
pub fn build_coherence_laplacian(
    links: &[(NodeId, NodeId)],
    coherences: &[f32],
    n_nodes: usize,
) -> Vec<Vec<f64>> {
    let mut laplacian = vec![vec![0.0f64; n_nodes]; n_nodes];

    for (link, &coh) in links.iter().zip(coherences.iter()) {
        let i = link.0 as usize;
        let j = link.1 as usize;
        let w = coh as f64;

        laplacian[i][j] -= w;
        laplacian[j][i] -= w;
        laplacian[i][i] += w;
        laplacian[j][j] += w;
    }

    laplacian
}
```

#### 6.1.3 Temporal Motif Matching

Using the motif patterns from Section 4.5, event detection becomes a
pattern-matching problem. Each event type has a characteristic temporal
motif signature:

| Event | Motif Type | Duration | Distinguishing Feature |
|-------|-----------|----------|----------------------|
| Person entry | Sweep (inward) | 1-3 s | Links near door drop first |
| Person exit | Sweep (outward) | 1-3 s | Links near door drop last |
| Gesture | Isolated oscillation | 0.5-2 s | Single-link high-frequency perturbation |
| Fall | Sudden cluster drop | 0.2-0.5 s | Multiple links drop simultaneously, fast |
| Door open | Global step change | 0.1-0.5 s | All links shift, new baseline forms |
| HVAC cycle | Global linear drift | 10-60 s | Slow, correlated, recoverable |

### 6.2 Event Detection Pipeline

```
Event Detection State Machine:

  [Raw CSI Frames at 20 Hz]
        |
        v
  [Per-Link Coherence Update]  --> coherence.rs
        |
        v
  [Gate Decision]              --> coherence_gate.rs
        |
  +-----+-----+
  |           |
  v           v
  [CUSUM     [Spectral
  Detector]   Analysis]
  |           |
  +-----+-----+
        |
        v
  [Temporal Motif Matching]
        |
        v
  [Event Classification]
        |
        +---> EntryEvent   --> cross_room.rs
        +---> ExitEvent    --> cross_room.rs
        +---> GestureEvent --> gesture.rs
        +---> FallEvent    --> pose_tracker.rs (emergency)
        +---> DoorEvent    --> field_model.rs (recalibrate)
        +---> DriftEvent   --> longitudinal.rs
```

### 6.3 Integration with Existing Event Types

The `CrossRoomTracker` in `cross_room.rs` already defines `ExitEvent`,
`EntryEvent`, and `TransitionEvent`. The temporal graph event detector
feeds these types:

```rust
/// Bridge between temporal graph events and cross-room tracker.
pub fn graph_event_to_cross_room(
    event: &DetectedEvent,
    tracker: &mut CrossRoomTracker,
    embedding: &[f32],
) -> Result<(), CrossRoomError> {
    match event.event_type {
        EventType::PersonExit { room_id, track_id } => {
            tracker.record_exit(ExitEvent {
                embedding: embedding.to_vec(),
                room_id,
                track_id,
                timestamp_us: event.timestamp_us,
                matched: false,
            })
        }
        EventType::PersonEntry { room_id, track_id } => {
            let entry = EntryEvent {
                embedding: embedding.to_vec(),
                room_id,
                track_id,
                timestamp_us: event.timestamp_us,
            };
            let _match_result = tracker.match_entry(&entry)?;
            Ok(())
        }
        _ => Ok(()),  // Other events don't affect cross-room tracking
    }
}
```

---

## 7. Compressed Temporal Storage

### 7.1 The CompressedCsiBuffer Concept

The `ruvector-temporal-tensor` crate provides `CompressedCsiBuffer` for
efficient ring-buffer storage of CSI data. We extend this concept to
store graph evolution history with minimal memory overhead.

### 7.2 Delta Compression of Graph Snapshots

Since the RF graph changes incrementally (most edges remain similar between
consecutive frames), delta encoding provides significant compression:

```rust
/// Delta-compressed temporal graph store.
pub struct DeltaGraphStore {
    /// Anchor snapshots at regular intervals (every 1 second = 20 frames).
    anchors: Vec<AnchorSnapshot>,
    /// Delta frames between anchors.
    deltas: Vec<Vec<EdgeDelta>>,
    /// Anchor interval in frames.
    anchor_interval: usize,
    /// Maximum history depth (anchors).
    max_anchors: usize,
    /// Current frame within the anchor interval.
    frame_in_interval: usize,
}

/// Full graph state at an anchor point.
pub struct AnchorSnapshot {
    pub timestamp_us: u64,
    pub frame_id: u64,
    /// Per-edge coherence values (quantized to u8: 0-255 maps to 0.0-1.0).
    pub coherences: Vec<u8>,
    /// Per-edge gate decisions (packed: 2 bits each).
    pub gate_decisions: Vec<u8>,
    /// Per-edge perturbation energy (quantized to u16).
    pub perturbation_energies: Vec<u16>,
    /// Graph-level Fiedler value.
    pub fiedler_value: f32,
    /// Graph-level total perturbation.
    pub total_perturbation: f32,
}

/// Change to a single edge between consecutive frames.
pub struct EdgeDelta {
    /// Edge index (into the link array).
    pub edge_idx: u8,
    /// Coherence change (quantized: i8, where 1 unit = 1/255).
    pub coherence_delta: i8,
    /// Whether the gate decision changed.
    pub gate_changed: bool,
    /// New gate decision (only present if gate_changed).
    pub new_gate: Option<u8>,
}
```

### 7.3 Compression Ratios

For a 15-link mesh:

| Representation | Per-Frame Size | 1-Hour Size | Compression Ratio |
|---------------|---------------|-------------|------------------|
| Full snapshot | 135 B | 9.7 MB | 1.0x (baseline) |
| Delta (typical 3 edges change) | 12 B | 864 KB | 11.2x |
| Delta (quiet, 0 edges change) | 2 B | 144 KB | 67.3x |
| Delta (active, 8 edges change) | 34 B | 2.4 MB | 4.0x |

With 1-second anchor intervals (every 20 frames), the anchor overhead adds
135 B * 3600 = 486 KB/hour, bringing the total to approximately 1.3 MB/hour
for typical occupancy, or 31 MB/day.

### 7.4 Temporal Index Structure

To support efficient temporal queries, we maintain a two-level index:

```
Index Structure:

  Level 0 (Anchor Index):  Binary search over anchor timestamps.
  Level 1 (Delta Index):   Sequential scan within anchor interval.

  Query: "coherence of link A-B at time T"
  1. Binary search anchors for latest anchor before T  --> O(log A)
  2. Reconstruct state at anchor                       --> O(1)
  3. Apply deltas from anchor to T                     --> O(F) where F <= 20
  Total: O(log A + F), F bounded by anchor_interval
```

For 24 hours of data with 1-second anchors, A = 86,400 anchors.
Binary search costs log2(86400) ~ 17 comparisons. Delta replay costs
at most 20 frame applications. Total: ~37 operations per point query.

### 7.5 Ring-Buffer Lifecycle

```
Ring-Buffer Rotation:

  +---+---+---+---+---+---+---+---+
  | A | d | d | d | A | d | d | d | ...
  +---+---+---+---+---+---+---+---+
    ^                               ^
    oldest                          newest

  When buffer is full:
  1. Evict oldest anchor + its deltas
  2. (Optionally) downsample to hourly archive before eviction
  3. Write new anchor at tail

  Archive downsampling:
  - Keep 1 anchor per minute (instead of per second)
  - Discard inter-anchor deltas
  - Retain only aggregate statistics (mean, min, max coherence)
```

---

## 8. Cross-Room Transition Graphs

### 8.1 Current Implementation

The `CrossRoomTracker` in `cross_room.rs` maintains:
- **Room fingerprints**: 128-dim AETHER embeddings of each room's static profile.
- **Pending exits**: Unmatched exit events with person embeddings.
- **Transition log**: Append-only record of cross-room transitions.

The transition log is already a temporal graph: rooms are vertices,
transitions are directed temporal edges with timestamps and similarity scores.

### 8.2 Extending to Full Temporal Transition Graphs

```rust
/// Temporal transition graph extending CrossRoomTracker.
pub struct TemporalTransitionGraph {
    /// Room-to-room adjacency with temporal statistics.
    adjacency: Vec<Vec<TransitionEdgeStats>>,
    /// Per-room temporal occupancy profile.
    room_profiles: Vec<RoomTemporalProfile>,
    /// Global transition patterns (time-of-day effects).
    circadian_patterns: Vec<CircadianPattern>,
}

/// Aggregated statistics for transitions between two rooms.
pub struct TransitionEdgeStats {
    pub from_room: u64,
    pub to_room: u64,
    /// Total transition count.
    pub count: u64,
    /// Welford statistics on transition gap times.
    pub gap_stats: WelfordStats,
    /// Welford statistics on similarity scores.
    pub similarity_stats: WelfordStats,
    /// Time-of-day histogram (24 bins, 1 hour each).
    pub hourly_histogram: [u32; 24],
    /// Most recent transition timestamp.
    pub last_transition_us: u64,
}

/// Per-room temporal occupancy model.
pub struct RoomTemporalProfile {
    pub room_id: u64,
    /// Welford statistics on occupancy duration.
    pub duration_stats: WelfordStats,
    /// Average occupancy by hour of day.
    pub hourly_occupancy: [f32; 24],
    /// Total person-seconds observed.
    pub total_person_seconds: f64,
    /// Fingerprint drift (cosine similarity of current vs. initial).
    pub fingerprint_drift: f32,
}
```

### 8.3 Transition Prediction

With sufficient history, the temporal transition graph enables prediction
of likely next transitions:

```rust
/// Predict the most likely next room for a person.
pub fn predict_next_room(
    graph: &TemporalTransitionGraph,
    current_room: u64,
    current_hour: u8,
    person_history: &[TransitionEvent],
) -> Vec<(u64, f64)> {
    // Combine three signals:
    // 1. Global transition frequency (base rate)
    // 2. Time-of-day pattern (circadian bias)
    // 3. Person-specific history (Markov chain)

    let mut predictions = Vec::new();

    for edge_stats in &graph.adjacency[current_room as usize] {
        let base_rate = edge_stats.count as f64;
        let circadian_weight = edge_stats.hourly_histogram[current_hour as usize] as f64
            / (edge_stats.count as f64).max(1.0);
        let personal_weight = person_specific_weight(
            person_history,
            current_room,
            edge_stats.to_room,
        );

        let score = base_rate * circadian_weight * personal_weight;
        predictions.push((edge_stats.to_room, score));
    }

    predictions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    predictions
}
```

### 8.4 Environment Fingerprint Evolution

Room fingerprints drift over time as furniture moves, seasonal temperature
changes affect multipath, and building modifications alter RF propagation.
The temporal transition graph tracks this drift:

```
Fingerprint Drift Timeline:

  Day 1        Day 30       Day 60       Day 90
  |            |            |            |
  v            v            v            v
  [F_0]  cos=0.99  [F_30] cos=0.95 [F_60] cos=0.88 [F_90]
                                          |
                                          v
                                    Drift threshold exceeded
                                    --> Re-fingerprint room
```

When `fingerprint_drift` drops below `CrossRoomConfig::min_similarity`,
the room's fingerprint should be recomputed to maintain cross-room
matching accuracy.

---

## 9. Longitudinal Drift Detection on Graph Topology

### 9.1 Current Implementation

The `PersonalBaseline` in `longitudinal.rs` tracks five biophysical metrics
per person using `WelfordStats`:
- Gait symmetry
- Stability index
- Breathing regularity
- Micro-tremor amplitude
- Activity level

Drift is detected when a metric exceeds 2-sigma for 3+ consecutive days,
escalating to `MonitoringLevel::RiskCorrelation` after 7+ days.

### 9.2 Extending to Graph Topology Metrics

The same Welford-based drift detection can monitor graph-level properties:

```rust
/// Graph-level longitudinal health metrics.
pub enum GraphHealthMetric {
    /// Mean coherence across all links.
    MeanCoherence,
    /// Minimum coherence (weakest link).
    MinCoherence,
    /// Standard deviation of coherence across links.
    CoherenceSpread,
    /// Fiedler value (graph connectivity).
    FiedlerValue,
    /// Total perturbation energy.
    TotalPerturbation,
    /// Fraction of links in Accept gate state.
    AcceptFraction,
    /// Geometric Diversity Index.
    Gdi,
    /// Mean Cramer-Rao bound (localisation accuracy).
    MeanCrb,
}

/// Per-graph longitudinal baseline extending PersonalBaseline pattern.
pub struct GraphBaseline {
    /// Per-metric Welford accumulators.
    pub metrics: Vec<(GraphHealthMetric, WelfordStats)>,
    /// Observation count (TDMA cycles).
    pub observation_count: u64,
    /// Consecutive drift counters (one per metric).
    pub drift_counters: Vec<u32>,
    /// Minimum observations before drift detection activates.
    pub min_observations: u64,
    /// Z-score threshold for drift.
    pub z_threshold: f64,
    /// Consecutive-frame threshold for drift alert.
    pub sustained_threshold: u32,
}

impl GraphBaseline {
    /// Update with a new graph-level observation.
    pub fn update(&mut self, observation: &GraphObservation) -> Vec<GraphDriftReport> {
        self.observation_count += 1;
        let mut reports = Vec::new();

        for (i, (metric, stats)) in self.metrics.iter_mut().enumerate() {
            let value = observation.value_for(metric);
            stats.update(value);

            if self.observation_count < self.min_observations {
                continue;
            }

            let z = stats.z_score(value);
            if z.abs() > self.z_threshold {
                self.drift_counters[i] += 1;
            } else {
                self.drift_counters[i] = 0;
            }

            if self.drift_counters[i] >= self.sustained_threshold {
                reports.push(GraphDriftReport {
                    metric: *metric,
                    z_score: z,
                    current_value: value,
                    baseline_mean: stats.mean,
                    baseline_std: stats.std_dev(),
                    sustained_frames: self.drift_counters[i],
                });
            }
        }

        reports
    }
}
```

### 9.3 Graph Health Monitoring

```
Graph Health State Machine:

  [Healthy]
    | coherence stable, Fiedler stable, GDI stable
    |
    +-- Mean coherence drops > 2-sigma for 5 min
    |       |
    |       v
    |   [Degraded]
    |     | Investigate: node failure? environmental shift?
    |     |
    |     +-- Node offline detected
    |     |       |
    |     |       v
    |     |   [Node Failure]
    |     |     | GDI drops, CRB increases
    |     |     | Alert: reduced sensing accuracy
    |     |     |
    |     |     +-- Node recovers --> [Healthy]
    |     |     +-- Sustained --> [Reconfigure]
    |     |
    |     +-- Environmental shift detected
    |     |       |
    |     |       v
    |     |   [Recalibrating]
    |     |     | FieldModel::reset_calibration()
    |     |     | Collect new baseline frames
    |     |     | FieldModel::finalize_calibration()
    |     |     +-- Success --> [Healthy]
    |     |     +-- Fails --> [Degraded]
    |     |
    |     +-- Recovers spontaneously --> [Healthy]
    |
    +-- Fiedler value drops sharply (< 3-sigma)
            |
            v
        [Partitioned]
          | Graph connectivity compromised
          | Fall-back to per-partition sensing
          +-- Connectivity restored --> [Healthy]
```

### 9.4 Biomechanics-Inspired Graph Health

Drawing from the `DriftMetric` enum in `longitudinal.rs`, we define
analogous graph health metrics with biomechanical parallels:

| Biomechanics Metric | Graph Analogue | Interpretation |
|---------------------|---------------|---------------|
| Gait Symmetry | Link coherence symmetry | Even sensing quality across all links |
| Stability Index | Fiedler value stability | Consistent graph connectivity |
| Breathing Regularity | Coherence periodicity | Regular environmental cycles (HVAC) |
| Micro-Tremor | High-freq coherence jitter | Electronic noise floor health |
| Activity Level | Total perturbation rate | Sensing volume utilisation |

---

## 10. Proposed Data Structures

### 10.1 Core Temporal Graph Type

```rust
/// The RF sensing temporal graph.
///
/// Central data structure for temporal graph evolution tracking.
/// Integrates with existing modules via the integration points
/// listed in Section 1.2.
pub struct RfTemporalGraph {
    // -- Topology (stable) --
    /// Node identifiers.
    nodes: Vec<NodeId>,
    /// Link definitions (directed: tx -> rx).
    links: Vec<(NodeId, NodeId)>,
    /// Node positions in room coordinates.
    positions: Vec<[f32; 3]>,

    // -- Live state (updated at 20 Hz) --
    /// Per-link coherence state (from coherence.rs).
    coherence_states: Vec<CoherenceState>,
    /// Per-link gate policy (from coherence_gate.rs).
    gate_policies: Vec<GatePolicy>,
    /// Field model for eigenstructure tracking.
    field_model: FieldModel,

    // -- Temporal storage --
    /// Delta-compressed graph history.
    history: DeltaGraphStore,
    /// Graph-level Welford baseline.
    graph_baseline: GraphBaseline,

    // -- Analysis --
    /// Per-link CUSUM detectors for change-point detection.
    cusum_detectors: Vec<CusumDetector>,
    /// Temporal motif classifier.
    motif_classifier: TemporalMotifClassifier,
    /// Cut boundary trackers (one per tracked person).
    cut_trackers: Vec<CutBoundaryTracker>,

    // -- Configuration --
    config: TemporalGraphConfig,
}

pub struct TemporalGraphConfig {
    /// TDMA cycle rate (Hz).
    pub cycle_rate_hz: f64,
    /// Anchor interval for delta compression (frames).
    pub anchor_interval: usize,
    /// Maximum history depth (seconds).
    pub max_history_s: f64,
    /// CUSUM slack parameter.
    pub cusum_slack: f64,
    /// CUSUM detection threshold.
    pub cusum_threshold: f64,
    /// Graph health z-score threshold.
    pub health_z_threshold: f64,
}

impl Default for TemporalGraphConfig {
    fn default() -> Self {
        Self {
            cycle_rate_hz: 20.0,
            anchor_interval: 20,  // 1 second
            max_history_s: 3600.0,  // 1 hour live
            cusum_slack: 0.05,
            cusum_threshold: 2.0,
            health_z_threshold: 2.0,
        }
    }
}
```

### 10.2 Frame Processing Pipeline

```rust
impl RfTemporalGraph {
    /// Process one TDMA cycle's worth of data.
    ///
    /// This is the main entry point called at 20 Hz.
    pub fn process_cycle(
        &mut self,
        fused_frame: &FusedSensingFrame,
        timestamp_us: u64,
    ) -> CycleResult {
        let mut result = CycleResult::default();

        // 1. Update per-link coherence states
        for (i, link) in self.links.iter().enumerate() {
            if let Some(amplitude) = extract_link_amplitude(fused_frame, link) {
                if let Ok(score) = self.coherence_states[i].update(&amplitude) {
                    // 2. Evaluate gate decision
                    let stale = self.coherence_states[i].stale_count();
                    let decision = self.gate_policies[i].evaluate(score, stale);

                    // 3. Run CUSUM change-point detection
                    if let Some(cp) = self.cusum_detectors[i].update(score as f64) {
                        result.change_points.push((*link, cp));
                    }
                }
            }
        }

        // 4. Extract perturbation from field model
        if let Ok(perturbation) = self.field_model.extract_perturbation(
            &build_observations(fused_frame),
        ) {
            result.total_perturbation = perturbation.total_energy;
        }

        // 5. Store snapshot/delta in temporal history
        self.history.record_frame(
            timestamp_us,
            &self.coherence_states,
            &self.gate_policies,
        );

        // 6. Run temporal motif classification
        result.motifs = self.motif_classifier.classify(
            &self.coherence_states,
            timestamp_us,
        );

        // 7. Update graph baseline for longitudinal monitoring
        let observation = GraphObservation::from_states(
            &self.coherence_states,
            &self.gate_policies,
            result.total_perturbation,
        );
        result.drift_reports = self.graph_baseline.update(&observation);

        // 8. Update cut boundary trackers
        // (Requires min-cut output from ruvector-mincut, omitted for clarity)

        result
    }
}

#[derive(Default)]
pub struct CycleResult {
    pub change_points: Vec<((NodeId, NodeId), ChangePoint)>,
    pub motifs: Vec<ActiveMotif>,
    pub drift_reports: Vec<GraphDriftReport>,
    pub total_perturbation: f64,
}
```

### 10.3 Type Summary

| Type | Module Location | Responsibility |
|------|----------------|---------------|
| `RfTemporalGraph` | `signal/src/ruvsense/temporal_graph.rs` (new) | Aggregate root |
| `DeltaGraphStore` | `signal/src/ruvsense/temporal_graph.rs` (new) | Compressed history |
| `CusumDetector` | `signal/src/ruvsense/temporal_graph.rs` (new) | Change-point detection |
| `TemporalMotifClassifier` | `signal/src/ruvsense/temporal_graph.rs` (new) | Pattern recognition |
| `CutBoundaryTracker` | `signal/src/ruvsense/temporal_graph.rs` (new) | Kalman-filtered cuts |
| `GraphBaseline` | `signal/src/ruvsense/temporal_graph.rs` (new) | Longitudinal health |
| `TemporalTransitionGraph` | `signal/src/ruvsense/cross_room.rs` (extend) | Room transitions |
| `CoherenceState` | `signal/src/ruvsense/coherence.rs` (existing) | Per-link live state |
| `GatePolicy` | `signal/src/ruvsense/coherence_gate.rs` (existing) | Per-link gate |
| `FieldModel` | `signal/src/ruvsense/field_model.rs` (existing) | Eigenstructure |
| `WelfordStats` | `signal/src/ruvsense/field_model.rs` (existing) | Online statistics |
| `PersonalBaseline` | `signal/src/ruvsense/longitudinal.rs` (existing) | Per-person drift |
| `CrossRoomTracker` | `signal/src/ruvsense/cross_room.rs` (existing) | Identity continuity |
| `MultistaticArray` | `ruvector/src/viewpoint/fusion.rs` (existing) | Viewpoint fusion |
| `GeometricDiversityIndex` | `ruvector/src/viewpoint/geometry.rs` (existing) | Array quality |
| `CramerRaoBound` | `ruvector/src/viewpoint/geometry.rs` (existing) | Localisation bound |

---

## 11. Integration Roadmap

### 11.1 Phase 1: Temporal Storage Foundation (2-3 weeks)

**Goal**: Implement `DeltaGraphStore` and basic temporal queries.

**Files to create**:
- `signal/src/ruvsense/temporal_graph.rs` -- Core temporal graph types
- `signal/src/ruvsense/temporal_store.rs` -- Delta compression engine

**Files to modify**:
- `signal/src/ruvsense/mod.rs` -- Register new modules
- `signal/src/ruvsense/coherence.rs` -- Add `snapshot()` method to `CoherenceState`

**Dependencies**: None (builds on existing `WelfordStats`, `CoherenceState`).

**Validation**:
- Unit tests for delta encode/decode roundtrip.
- Property tests: reconstruct any timestamp from anchors + deltas.
- Memory budget tests: verify < 100 MB/day for 6-node mesh.

### 11.2 Phase 2: Change-Point Detection (1-2 weeks)

**Goal**: Implement CUSUM detectors and event classification.

**Files to create**:
- `signal/src/ruvsense/change_point.rs` -- CUSUM and spectral detectors

**Files to modify**:
- `signal/src/ruvsense/cross_room.rs` -- Accept events from detector

**Dependencies**: Phase 1 (temporal store for history access).

**Validation**:
- Replay recorded CSI sessions, compare detected events to ground truth.
- False positive rate < 1 per hour for empty room.
- Detection latency < 500 ms for person entry/exit.

### 11.3 Phase 3: Min-Cut Trajectory Tracking (2-3 weeks)

**Goal**: Implement `CutBoundaryTracker` with Kalman filtering.

**Files to create**:
- `signal/src/ruvsense/cut_trajectory.rs` -- Kalman-filtered cut tracking

**Files to modify**:
- `signal/src/ruvsense/multistatic.rs` -- Feed `PersonCluster` to tracker

**Dependencies**: Phase 1, `ruvector-mincut` integration.

**Validation**:
- Trajectory smoothness: velocity discontinuity < 0.5 m/s between frames.
- Interpolation accuracy: compare 200 Hz interpolated vs. 20 Hz measured.

### 11.4 Phase 4: Longitudinal Graph Health (1-2 weeks)

**Goal**: Implement `GraphBaseline` with drift detection.

**Files to modify**:
- `signal/src/ruvsense/longitudinal.rs` -- Extract `WelfordStats` pattern
  into shared trait, implement for graph metrics.

**Dependencies**: Phase 1, Phase 2.

**Validation**:
- Inject simulated node failures, verify detection within 5 minutes.
- Inject simulated environmental drift, verify detection within 10 minutes.
- No false drift alerts during 24-hour stable operation.

### 11.5 Phase 5: Temporal Transition Graph (1 week)

**Goal**: Extend `CrossRoomTracker` with `TemporalTransitionGraph`.

**Files to modify**:
- `signal/src/ruvsense/cross_room.rs` -- Add temporal statistics to
  transition log, implement transition prediction.

**Dependencies**: Phase 2 (event detection feeds transitions).

**Validation**:
- Transition prediction accuracy > 70% for top-1 room after 7 days.
- Circadian patterns detected within 3 days of continuous operation.

### 11.6 Proposed ADR

This work warrants a new Architecture Decision Record:

**ADR-044: Temporal Graph Evolution Tracking**
- Status: Proposed
- Context: Static graph analysis misses temporal patterns critical for
  event detection, trajectory tracking, and longitudinal monitoring.
- Decision: Implement `RfTemporalGraph` as described in Section 10.
- Consequences: Adds ~100 MB/day storage, ~2 ms per-frame processing
  overhead, enables 5 new sensing capabilities.

---

## 12. References

### 12.1 Temporal Graph Networks

1. Rossi, E., Chamberlain, B., Frasca, F., Eynard, D., Monti, F., &
   Bronstein, M. (2020). "Temporal Graph Networks for Deep Learning on
   Dynamic Graphs." ICML Workshop on GRL+.

2. Kumar, S., Zhang, X., & Leskovec, J. (2019). "Predicting Dynamic
   Embedding Trajectory in Temporal Interaction Networks." KDD.

3. Chen, J., Zheng, S., Song, H., & Zhu, J. (2021). "Continuous-Time
   Dynamic Graph Learning via Neural Interaction Processes." CIKM.

4. Trivedi, R., Farajtabar, M., Bisber, P., & Zha, H. (2019). "DyRep:
   Learning Representations over Dynamic Graphs." ICLR.

5. Xu, D., Ruan, C., Korpeoglu, E., Kumar, S., & Achan, K. (2020).
   "Inductive Representation Learning on Temporal Graphs." ICLR.

### 12.2 Graph Signal Processing

6. Shuman, D., Narang, S., Frossard, P., Ortega, A., & Vandergheynst, P.
   (2013). "The Emerging Field of Signal Processing on Graphs." IEEE
   Signal Processing Magazine.

7. Sandryhaila, A. & Moura, J. M. F. (2014). "Big Data Analysis with
   Signal Processing on Graphs." IEEE Signal Processing Magazine.

### 12.3 Change-Point Detection

8. Page, E. S. (1954). "Continuous Inspection Schemes." Biometrika.

9. Aminikhanghahi, S. & Cook, D. J. (2017). "A Survey of Methods for
   Time Series Change Point Detection." Knowledge and Information Systems.

### 12.4 RF Tomography and WiFi Sensing

10. Wilson, J. & Patwari, N. (2010). "Radio Tomographic Imaging with
    Wireless Networks." IEEE Transactions on Mobile Computing.

11. Wang, H., Zhang, D., Wang, Y., Ma, J., Wang, Y., & Li, S. (2017).
    "RT-Fall: A Real-Time and Contactless Fall Detection System with
    Commodity WiFi Devices." IEEE Transactions on Mobile Computing.

12. Ma, Y., Zhou, G., & Wang, S. (2019). "WiFi Sensing with Channel State
    Information: A Survey." ACM Computing Surveys.

### 12.5 Internal Architecture References

13. ADR-029: RuvSense Multistatic Sensing Mode
14. ADR-030: RuvSense Persistent Field Model
15. ADR-031: RuView Sensing-First RF Mode
16. ADR-024: Contrastive CSI Embedding / AETHER
17. ADR-027: Cross-Environment Domain Generalization / MERIDIAN

### 12.6 Kalman Filtering

18. Welch, G. & Bishop, G. (2006). "An Introduction to the Kalman Filter."
    UNC-Chapel Hill, TR 95-041.

19. Rauch, H. E., Tung, F., & Striebel, C. T. (1965). "Maximum Likelihood
    Estimates of Linear Dynamic Systems." AIAA Journal.

### 12.7 Graph Spectral Analysis

20. Chung, F. R. K. (1997). "Spectral Graph Theory." CBMS Regional
    Conference Series in Mathematics, AMS.

21. Fiedler, M. (1973). "Algebraic Connectivity of Graphs." Czechoslovak
    Mathematical Journal.
