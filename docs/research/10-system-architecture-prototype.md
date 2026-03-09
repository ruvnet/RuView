# Research Document 10: RF Topological Sensing — System Architecture and Prototype

**Date**: 2026-03-08
**Status**: Draft
**Author**: Research Agent
**Scope**: End-to-end architecture for RF topological sensing using ESP32 mesh networks

---

## Table of Contents

1. [End-to-End Architecture](#1-end-to-end-architecture)
2. [Existing Crate Integration](#2-existing-crate-integration)
3. [New Module Design](#3-new-module-design)
4. [Real-Time Pipeline](#4-real-time-pipeline)
5. [Prototype Phases](#5-prototype-phases)
6. [Benchmark](#6-benchmark)
7. [ADR-044 Draft](#7-adr-044-draft)
8. [Rust Trait Definitions](#8-rust-trait-definitions)

---

## 1. End-to-End Architecture

### 1.1 Core Concept

RF topological sensing treats a mesh of ESP32 nodes as a "radio nervous system."
Every transmitter-receiver pair defines a graph edge. The Channel State Information
(CSI) measured on each edge encodes how the radio environment between those two
nodes has been perturbed — by walls, furniture, and most importantly, by human
bodies. When a person stands between two nodes, the CSI coherence on that link
drops. The collection of all such drops defines a cut in the graph that traces the
physical boundary of the person.

The system does not estimate pose directly. Instead it answers a more fundamental
question: *where are the boundaries between occupied and unoccupied space?* Pose
estimation, activity recognition, and room segmentation are all downstream
consumers of this boundary information.

### 1.2 Data Flow Summary

```
ESP32 Node A ──CSI──> Edge (A,B) ──weight──> Graph G ──mincut──> Boundaries ──render──> UI
ESP32 Node B ──CSI──> Edge (B,C) ──weight──>    |          |          |
ESP32 Node C ──CSI──> Edge (A,C) ──weight──>    |          |          |
     ...              ...                       v          v          v
ESP32 Node N          Edge (i,j)           RfGraph    CutBoundary  WebSocket
```

### 1.3 Pipeline Diagram

```
+============================================================================+
|                    RF TOPOLOGICAL SENSING PIPELINE                          |
+============================================================================+

  STAGE 1: CSI EXTRACTION                          STAGE 2: EDGE WEIGHT
  ~~~~~~~~~~~~~~~~~~~~~~~~                         ~~~~~~~~~~~~~~~~~~~~
  +-------------+    +-------------+               +-----------------+
  | ESP32 Node  |    | ESP32 Node  |               |  Edge Weight    |
  |    (TX)     |--->|    (RX)     |--[ raw CSI ]->|  Computation    |
  |  ch_hop()   |    |  extract()  |               |                 |
  +-------------+    +-------------+               | - phase_align() |
        |                  |                       | - coherence()   |
        | TDM slot         | 52-subcarrier         | - amplitude()   |
        | assignment       | CSI frame             | - temporal_avg  |
        v                  v                       +---------+-------+
  +-------------+    +-------------+                         |
  | TDM         |    | CSI Frame   |                 weight: f64
  | Scheduler   |    | Buffer      |                 [0.0 .. 1.0]
  | (hardware)  |    | (ring buf)  |                         |
  +-------------+    +-------------+                         v

  STAGE 3: GRAPH CONSTRUCTION                      STAGE 4: DYNAMIC MINCUT
  ~~~~~~~~~~~~~~~~~~~~~~~~~~~                      ~~~~~~~~~~~~~~~~~~~~~~
  +-----------------+                              +------------------+
  |    RfGraph      |                              |  Mincut Solver   |
  |                 |<----[ edge weights ]---------|                  |
  | - add_edge()    |                              | - stoer_wagner() |
  | - update_wt()   |                              |   or             |
  | - prune_stale() |                              | - karger()       |
  | - adjacency mat |----[ graph snapshot ]------->| - push_relabel() |
  |                 |                              |                  |
  +-----------------+                              +--------+---------+
                                                            |
                                                    CutBoundary {
                                                      cut_edges,
                                                      cut_value,
                                                      partitions
                                                    }
                                                            |
                                                            v

  STAGE 5: BOUNDARY VISUALIZATION
  ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
  +------------------+       +-------------------+       +----------------+
  | Boundary         |       | Sensing Server    |       | Browser UI     |
  | Interpolation    |------>| (Axum WebSocket)  |------>| (Canvas/WebGL) |
  |                  |       |                   |       |                |
  | - contour_from() |       | - ws_broadcast()  |       | - draw_room()  |
  | - smooth()       |       | - /api/topology   |       | - draw_cuts()  |
  | - to_polygon()   |       | - /api/stream     |       | - animate()    |
  +------------------+       +-------------------+       +----------------+
```

### 1.4 Data Structures at Each Stage

```
Stage 1 Output:  CsiFrame { tx_id, rx_id, subcarriers: [Complex<f32>; 52], timestamp_us }
Stage 2 Output:  EdgeWeight { tx_id, rx_id, weight: f64, confidence: f64, updated_at }
Stage 3 Output:  RfGraph { nodes: Vec<NodeId>, edges: HashMap<(NodeId,NodeId), EdgeWeight> }
Stage 4 Output:  CutBoundary { cut_edges: Vec<(NodeId,NodeId)>, partitions: (Vec<NodeId>, Vec<NodeId>) }
Stage 5 Output:  BoundaryPolygon { vertices: Vec<(f64,f64)>, confidence: f64 }
```

### 1.5 Communication Protocol

Nodes communicate using TDM (Time Division Multiplexing) as defined in
ADR-028. Each node is assigned a transmit slot. During its slot, a node
transmits on a known subcarrier pattern. All other nodes simultaneously
receive and extract CSI. This yields N*(N-1)/2 unique edges for N nodes.

```
Time -->
  Slot 0    Slot 1    Slot 2    Slot 3    Slot 0    Slot 1  ...
  [Node A]  [Node B]  [Node C]  [Node D]  [Node A]  [Node B]
  TX        TX        TX        TX        TX        TX
  B,C,D RX  A,C,D RX  A,B,D RX  A,B,C RX  B,C,D RX  A,C,D RX

  One full cycle = N slots = one complete graph snapshot
  At 1ms slots, 4-node cycle = 4ms, 16-node cycle = 16ms
```

---

## 2. Existing Crate Integration

### 2.1 Integration Map

```
+---------------------------+     +-----------------------------+
| wifi-densepose-hardware   |     | wifi-densepose-signal       |
| (ESP32 TDM, CSI extract)  |     | (ruvsense modules)          |
+------------+--------------+     +-------------+---------------+
             |                                  |
             | CsiFrame                         | coherence, phase
             v                                  v
+------------------------------------------------------------------+
|                   rf_topology (NEW MODULE)                        |
|  RfGraph, EdgeWeight, CutBoundary, TopologyEvent                 |
+------------------------------------------------------------------+
             |                                  |
             | graph memory                     | boundary data
             v                                  v
+-----------------------------+     +-----------------------------+
| wifi-densepose-ruvector     |     | wifi-densepose-sensing-     |
| (graph memory, attention)   |     |   server (UI, WebSocket)    |
+-----------------------------+     +-----------------------------+
```

### 2.2 wifi-densepose-signal / ruvsense

The signal crate contains the RuvSense modules that provide the mathematical
foundation for edge weight computation.

**coherence.rs** — Z-score coherence scoring with DriftProfile. This module
already computes a coherence metric between CSI frames. For RF topology, we
use coherence as the primary edge weight: high coherence means the link is
unobstructed, low coherence means something (a person) is in the path.

```
Usage in rf_topology:
  - coherence::ZScoreCoherence::score(baseline_csi, current_csi) -> f64
  - coherence::DriftProfile tracks long-term drift per edge
  - coherence_gate::CoherenceGate decides if a measurement is reliable
```

**phase_align.rs** — Iterative LO phase offset estimation using circular mean.
ESP32 local oscillators drift, which corrupts phase measurements. Phase
alignment is a prerequisite for meaningful coherence computation.

```
Usage in rf_topology:
  - phase_align::align_frames(tx_csi, rx_csi) -> AlignedCsiPair
  - Must be called BEFORE coherence scoring
  - Runs per-edge, per-frame
```

**multiband.rs** — Multi-band CSI frame fusion. When nodes operate on multiple
WiFi channels (via channel hopping), this module fuses the measurements into
a single coherent view.

```
Usage in rf_topology:
  - multiband::fuse_channels(ch1_csi, ch5_csi, ch11_csi) -> FusedCsiFrame
  - Increases spatial resolution of edge weights
  - Optional: single-channel operation is sufficient for prototype
```

**multistatic.rs** — Attention-weighted fusion with geometric diversity. This
module already performs multi-link fusion, which is conceptually close to what
rf_topology needs. The key difference is that multistatic.rs fuses for pose
estimation, while rf_topology fuses for boundary detection.

```
Usage in rf_topology:
  - multistatic::GeometricDiversity provides link quality weighting
  - Reuse attention weights for graph edge confidence scoring
```

**adversarial.rs** — Physically impossible signal detection. This module
detects when CSI measurements violate physical constraints (e.g., signal
strength increases when a person is blocking the path). Essential for
filtering bad edges in the graph.

```
Usage in rf_topology:
  - adversarial::PhysicsChecker::validate(edge_measurement) -> Result<(), Violation>
  - Edges that fail validation are marked low-confidence
```

### 2.3 wifi-densepose-ruvector

The ruvector crate provides graph-based data structures and attention mechanisms
that can be repurposed for RF topology.

**viewpoint/attention.rs** — CrossViewpointAttention with GeometricBias and
softmax. The attention mechanism computes importance weights across multiple
viewpoints. In RF topology, each TX-RX pair is a "viewpoint" and the attention
mechanism can prioritize the most informative edges.

```
Usage in rf_topology:
  - CrossViewpointAttention can weight edges by geometric diversity
  - GeometricBias accounts for node placement geometry
  - Softmax normalization produces valid probability distribution over edges
```

**viewpoint/geometry.rs** — GeometricDiversityIndex and Cramer-Rao bounds.
This module quantifies how much geometric information a set of links provides.
RF topology uses this to determine if the current node placement can resolve
a boundary at a given location.

```
Usage in rf_topology:
  - GeometricDiversityIndex tells us if we have enough angular coverage
  - Cramer-Rao bound gives theoretical position error lower bound
  - Fisher Information matrix guides optimal node placement
```

**viewpoint/coherence.rs** — Phase phasor coherence with hysteresis gate.
Already provides a gating mechanism for coherence measurements. RF topology
reuses this to prevent boundary flicker from noisy measurements.

```
Usage in rf_topology:
  - Hysteresis gate prevents rapid edge weight oscillation
  - Smooths boundary detection over time
```

**viewpoint/fusion.rs** — MultistaticArray aggregate root with domain events.
This is a DDD aggregate root that manages a collection of multistatic links.
RF topology can extend this pattern for graph-level aggregate management.

```
Usage in rf_topology:
  - MultistaticArray pattern informs RfGraph aggregate design
  - Domain events (LinkAdded, LinkDropped) map to TopologyEvent
```

### 2.4 wifi-densepose-hardware

The hardware crate manages ESP32 devices and the TDM protocol.

**esp32/tdm.rs** — Time Division Multiplexing scheduler. Assigns transmit
slots to nodes, ensures collision-free CSI extraction.

```
Usage in rf_topology:
  - TdmScheduler provides the frame timing that drives the pipeline
  - Each TDM cycle produces one complete graph snapshot
  - Cycle period = N_nodes * slot_duration
```

**esp32/channel_hop.rs** — Channel hopping firmware control. Allows nodes to
measure CSI on multiple WiFi channels for improved spatial resolution.

```
Usage in rf_topology:
  - Channel diversity increases edge weight accuracy
  - Feeds into multiband.rs fusion
```

**esp32/csi_extract.rs** — Raw CSI extraction from ESP32 hardware registers.
Produces CsiFrame structs that are the input to the entire pipeline.

```
Usage in rf_topology:
  - CsiFrame is the fundamental input type
  - 52 subcarriers per frame on 20MHz channels
  - Timestamp synchronization via NTP or TDM slot timing
```

### 2.5 wifi-densepose-sensing-server

The sensing server provides the web UI and WebSocket streaming.

```
Usage in rf_topology:
  - WebSocket endpoint broadcasts CutBoundary updates to browser
  - REST endpoint /api/topology returns current graph state
  - Static file serving for visualization JavaScript
  - Axum router integrates new topology endpoints
```

### 2.6 Integration Summary Table

| Existing Module              | What It Provides              | How rf_topology Uses It       |
|------------------------------|-------------------------------|-------------------------------|
| signal/ruvsense/coherence    | Z-score coherence scoring     | Primary edge weight metric    |
| signal/ruvsense/phase_align  | LO phase offset correction    | Pre-processing for coherence  |
| signal/ruvsense/multiband    | Multi-channel fusion          | Improved edge resolution      |
| signal/ruvsense/multistatic  | Geometric diversity weighting | Edge confidence scoring       |
| signal/ruvsense/adversarial  | Physics violation detection   | Bad edge filtering            |
| signal/ruvsense/coherence_gate | Hysteresis gating           | Boundary flicker prevention   |
| ruvector/viewpoint/attention | Cross-viewpoint attention     | Edge importance weighting     |
| ruvector/viewpoint/geometry  | Geometric diversity index     | Resolution analysis           |
| ruvector/viewpoint/fusion    | DDD aggregate root pattern    | RfGraph aggregate design      |
| hardware/esp32/tdm           | TDM slot scheduling           | Frame timing, cycle control   |
| hardware/esp32/csi_extract   | Raw CSI extraction            | Pipeline input                |
| sensing-server               | Axum WebSocket + REST         | Visualization delivery        |

---

## 3. New Module Design

### 3.1 Module Location

```
rust-port/wifi-densepose-rs/crates/wifi-densepose-signal/src/ruvsense/
  rf_topology.rs          <-- New module (primary)
  rf_topology/
    graph.rs              <-- RfGraph aggregate root
    edge_weight.rs        <-- EdgeWeight computation
    mincut.rs             <-- Dynamic mincut solver
    boundary.rs           <-- CutBoundary -> spatial polygon
    events.rs             <-- TopologyEvent domain events
    mod.rs                <-- Module re-exports
```

Alternatively, rf_topology could be a standalone crate:

```
rust-port/wifi-densepose-rs/crates/wifi-densepose-topology/
  src/
    lib.rs
    graph.rs
    edge_weight.rs
    mincut.rs
    boundary.rs
    events.rs
  Cargo.toml
```

The standalone crate approach is preferred because RF topology has distinct
bounded-context semantics and its own aggregate root (RfGraph). It depends on
wifi-densepose-signal for coherence computation and wifi-densepose-core for
shared types.

### 3.2 Key Types

#### RfGraph — Aggregate Root

RfGraph is the central aggregate root. It owns the complete graph state: nodes,
edges, weights, and metadata. All mutations go through RfGraph methods, which
emit TopologyEvents for downstream consumers.

```
RfGraph {
  id: GraphId,
  nodes: HashMap<NodeId, NodeInfo>,
  edges: HashMap<EdgeId, EdgeState>,
  adjacency: AdjacencyMatrix,
  epoch: u64,                          // incremented on each full TDM cycle
  last_updated: Instant,
  config: TopologyConfig,
}
```

Invariants enforced by RfGraph:
- No self-loops (tx_id != rx_id)
- Edge weights are in [0.0, 1.0]
- Stale edges (no update in N cycles) are pruned
- Graph is always connected (disconnected subgraphs trigger alert)

#### EdgeWeight — Value Object

```
EdgeWeight {
  tx_id: NodeId,
  rx_id: NodeId,
  weight: f64,                         // 0.0 = fully obstructed, 1.0 = clear
  raw_coherence: f64,                  // pre-normalization coherence
  confidence: f64,                     // measurement quality [0.0, 1.0]
  sample_count: u32,                   // number of CSI frames averaged
  baseline_deviation: f64,             // how far from calibrated baseline
  updated_at: Instant,
}
```

EdgeWeight is a value object: immutable after creation. Each TDM cycle produces
a new EdgeWeight for each edge, which replaces the previous one in RfGraph.

#### CutBoundary — Value Object

```
CutBoundary {
  cut_edges: Vec<EdgeId>,              // edges that cross the boundary
  cut_value: f64,                      // total weight of cut edges
  partition_a: Vec<NodeId>,            // nodes on one side
  partition_b: Vec<NodeId>,            // nodes on the other side
  spatial_boundary: Option<Polygon>,   // interpolated physical boundary
  confidence: f64,                     // based on edge confidences
  detected_at: Instant,
}
```

CutBoundary represents the output of the mincut solver. Multiple CutBoundaries
can exist simultaneously when multiple people are detected.

#### TopologyEvent — Domain Event

```
TopologyEvent {
  id: EventId,
  timestamp: Instant,
  kind: TopologyEventKind,
}

enum TopologyEventKind {
  NodeJoined { node_id: NodeId, position: (f64, f64) },
  NodeLeft { node_id: NodeId, reason: LeaveReason },
  EdgeWeightChanged { edge_id: EdgeId, old: f64, new: f64 },
  BoundaryDetected { boundary: CutBoundary },
  BoundaryMoved { boundary_id: BoundaryId, displacement: (f64, f64) },
  BoundaryLost { boundary_id: BoundaryId },
  GraphPartitioned { components: Vec<Vec<NodeId>> },
  CalibrationRequired { reason: String },
}
```

Events are published to an event bus. The sensing server subscribes and
forwards relevant events to the browser UI via WebSocket.

### 3.3 DDD Aggregate Root Design

```
+-------------------------------------------------------------------+
|                     RfGraph (Aggregate Root)                       |
|                                                                   |
|  +------------------+    +-----------------+    +---------------+ |
|  | NodeRegistry     |    | EdgeRegistry    |    | CutSolver     | |
|  |                  |    |                 |    |               | |
|  | - register()     |    | - update_wt()   |    | - solve()     | |
|  | - deregister()   |    | - prune_stale() |    | - track()     | |
|  | - get_position() |    | - get_weight()  |    | - boundaries  | |
|  +------------------+    +-----------------+    +---------------+ |
|                                                                   |
|  Command Interface:                                               |
|    fn ingest_csi_frame(&mut self, frame: CsiFrame) -> Vec<Event>  |
|    fn tick(&mut self) -> Vec<Event>                                |
|    fn calibrate(&mut self, baseline: &Baseline) -> Vec<Event>     |
|    fn add_node(&mut self, node: NodeInfo) -> Vec<Event>           |
|    fn remove_node(&mut self, node_id: NodeId) -> Vec<Event>       |
|                                                                   |
|  Query Interface:                                                 |
|    fn current_boundaries(&self) -> &[CutBoundary]                 |
|    fn edge_weight(&self, a: NodeId, b: NodeId) -> Option<f64>     |
|    fn graph_snapshot(&self) -> GraphSnapshot                      |
|    fn node_count(&self) -> usize                                  |
|    fn is_connected(&self) -> bool                                 |
+-------------------------------------------------------------------+
                          |
                          | emits
                          v
                  Vec<TopologyEvent>
                          |
                          v
               +---------------------+
               |  Event Bus          |
               |  (tokio broadcast)  |
               +---------------------+
                    |           |
                    v           v
            Sensing Server   Pose Tracker
            (WebSocket)      (ruvsense)
```

### 3.4 Module Responsibilities

| File             | Responsibility                        | LOC Estimate |
|------------------|---------------------------------------|--------------|
| graph.rs         | RfGraph aggregate, node/edge registry | ~200         |
| edge_weight.rs   | Weight computation from CSI coherence | ~120         |
| mincut.rs        | Stoer-Wagner and incremental mincut   | ~180         |
| boundary.rs      | Cut-to-polygon interpolation          | ~150         |
| events.rs        | TopologyEvent types and bus           | ~80          |
| mod.rs           | Public API re-exports                 | ~30          |
| **Total**        |                                       | **~760**     |

All files stay under the 500-line limit by splitting graph.rs if needed.

---

## 4. Real-Time Pipeline

### 4.1 Latency Budget

The system must produce updated boundary estimates within 100ms of a CSI
frame arrival. This enables responsive real-time visualization and is
sufficient for human-speed movement tracking.

```
+============================================================================+
|                         LATENCY BUDGET: 100ms TOTAL                        |
+============================================================================+

  Stage                 Budget    Actual Target    Notes
  ~~~~~~~~~~~~~~~~~~~~  ~~~~~~~~  ~~~~~~~~~~~~~~   ~~~~~~~~~~~~~~~~~~~~~~~~~
  1. CSI Extraction      5 ms       3-5 ms         ESP32 hardware, fixed
  2. Phase Alignment     3 ms       1-2 ms         Per-edge, parallelizable
  3. Edge Weight Comp   10 ms       5-8 ms         Coherence + normalization
  4. Graph Update         2 ms       0.5-1 ms       HashMap insert/update
  5. Mincut Solver        5 ms       2-5 ms         Stoer-Wagner on N<64
  6. Boundary Interp      5 ms       2-3 ms         Polygon from cut edges
  7. Serialization        2 ms       0.5-1 ms       serde_json or bincode
  8. WebSocket TX         3 ms       1-2 ms         Local network
  9. Browser Render      20 ms       10-16 ms       Canvas 2D at 60fps
  ~~~~~~~~~~~~~~~~~~~~  ~~~~~~~~  ~~~~~~~~~~~~~~
  TOTAL                 55 ms       26-43 ms       ~50ms headroom

  Margin for safety:    45 ms                      Absorbs GC, jitter, WiFi
```

### 4.2 Stage Details

#### Stage 1: CSI Extraction (5ms budget)

The ESP32 extracts CSI from each received packet. This happens in firmware
and is bounded by the WiFi hardware. The output is a 52-element complex
vector plus metadata (RSSI, noise floor, timestamp).

```
Input:  WiFi packet on air
Output: CsiFrame { subcarriers: [Complex<f32>; 52], rssi: i8, ... }
Cost:   Fixed by hardware. ~3ms on ESP32-S3, ~5ms on ESP32.
```

#### Stage 2: Phase Alignment (3ms budget)

Phase alignment corrects for local oscillator drift between TX and RX nodes.
Uses the circular mean algorithm from ruvsense/phase_align.rs. This runs
once per edge per frame.

```
Input:  CsiFrame pair (TX reference, RX measurement)
Output: AlignedCsiPair with corrected phase
Cost:   ~50us per edge. For 16 nodes (120 edges): 6ms sequential, <1ms parallel
Note:   Embarrassingly parallel across edges. Use rayon par_iter.
```

#### Stage 3: Edge Weight Computation (10ms budget)

Compute coherence between current CSI and baseline CSI. Apply temporal
averaging (exponential moving average over last K frames). Normalize to
[0.0, 1.0] range. Apply adversarial physics check.

```
Input:  AlignedCsiPair + baseline reference
Output: EdgeWeight { weight, confidence, ... }
Cost:   ~80us per edge. For 120 edges: 9.6ms sequential, <2ms parallel
Pipeline:
  1. coherence::ZScoreCoherence::score()      ~30us
  2. temporal_average()                         ~10us
  3. adversarial::PhysicsChecker::validate()   ~20us
  4. normalize_and_gate()                       ~20us
```

#### Stage 4: Graph Update (2ms budget)

Insert new edge weights into RfGraph. Prune stale edges. Check connectivity.
This is a simple HashMap operation.

```
Input:  Vec<EdgeWeight> from current TDM cycle
Output: Updated RfGraph, list of changed edges
Cost:   O(E) where E = number of edges. <1ms for E < 500.
```

#### Stage 5: Mincut Solver (5ms budget)

Run Stoer-Wagner minimum cut on the weighted graph. For small graphs (N < 64),
Stoer-Wagner runs in O(V * E + V^2 * log V) which is well within budget.

```
Input:  RfGraph adjacency matrix with weights
Output: CutBoundary (minimum cut edges + partitions)
Cost:   4-node:  ~0.1ms
        16-node: ~2ms
        64-node: ~15ms (exceeds budget -- use incremental solver)
```

For graphs larger than ~40 nodes, use incremental mincut: only recompute
the cut in the neighborhood of changed edges. This keeps the cost under
5ms regardless of total graph size.

#### Stage 6: Boundary Interpolation (5ms budget)

Convert the cut edges into a spatial polygon by interpolating between the
known positions of the nodes on either side of the cut.

```
Input:  CutBoundary + node positions
Output: BoundaryPolygon { vertices: Vec<(f64, f64)> }
Cost:   Convex hull + smoothing. <3ms for typical boundaries.
```

#### Stage 7-9: Serialization, Transport, Render (25ms budget)

Serialize boundary polygon to JSON, send over WebSocket, render in browser.

```
Serialization:  serde_json::to_string(&boundary) -- <1ms
WebSocket TX:   axum tungstenite broadcast -- <2ms local
Browser render: Canvas 2D path drawing -- 10-16ms at 60fps
```

### 4.3 Timing Diagram

```
Time (ms)  0    5    10   15   20   25   30   35   40   45   50
           |    |    |    |    |    |    |    |    |    |    |
           [CSI ]
                [Phs][    Edge Weight    ]
                                         [GU][Cut ]
                                                   [Bnd][Ser][WS]
                                                                  [Render....]
           |<-- ESP32 firmware --|<------ Rust pipeline -------->|<-- Browser ->|
           |    5ms              |           ~25ms               |    ~16ms     |
           |<---------------------- Total: ~46ms ------------------------------>|
```

### 4.4 Parallelism Strategy

```
+-- rayon thread pool (4 threads on server, 1 on ESP32) --+
|                                                          |
|  Edge 0: [phase_align] -> [coherence] -> [weight]       |
|  Edge 1: [phase_align] -> [coherence] -> [weight]       |
|  Edge 2: [phase_align] -> [coherence] -> [weight]       |
|  ...                                                     |
|  Edge N: [phase_align] -> [coherence] -> [weight]       |
|                                                          |
+-- barrier: all edges complete --------+                  |
                                        |                  |
                        [graph_update] (single thread)     |
                        [mincut_solve] (single thread)     |
                        [boundary_interp] (single thread)  |
                        [serialize + broadcast]            |
+----------------------------------------------------------+
```

Edge weight computation is embarrassingly parallel and dominates the pipeline
cost. Using rayon reduces this from O(E * cost_per_edge) to
O(E * cost_per_edge / num_threads).

---

## 5. Prototype Phases

### 5.1 Phase 1: 4-Node Proof of Concept

**Goal**: Detect a single person entering a square region bounded by 4 ESP32 nodes.

```
  Node A ─────────── Node B
    |   \           /   |
    |     \       /     |
    |       \   /       |
    |        [X]        |     X = person standing here
    |       /   \       |
    |     /       \     |
    |   /           \   |
  Node D ─────────── Node C

  Edges: A-B, A-C, A-D, B-C, B-D, C-D  (6 total)
  Room size: 3m x 3m
```

**Setup**:
- 4x ESP32-S3 DevKitC boards
- Nodes at corners of a 3m x 3m room
- Single WiFi channel (channel 6, 2.437 GHz)
- TDM with 1ms slots = 4ms cycle = 250 Hz update rate

**Success Criteria**:
- Detect person presence within 500ms of entering the room
- Correctly identify which quadrant the person is in
- No false positives when room is empty (over 10-minute test)
- Mincut correctly separates the person from at least one node

**Deliverables**:
- Working TDM firmware on 4 ESP32 boards
- Rust pipeline processing CSI in real-time
- Web UI showing graph with highlighted cut edges
- Calibration procedure documented

**Timeline**: 4 weeks

```
Week 1: TDM firmware bring-up, CSI extraction verified
Week 2: Edge weight pipeline, baseline calibration
Week 3: Mincut integration, boundary detection logic
Week 4: Web UI, end-to-end test, benchmark
```

### 5.2 Phase 2: 16-Node Room Scale

**Goal**: Track the spatial boundaries of 1-3 people moving through a room.

```
  A ── B ── C ── D
  |  \ | /\ | /\ |
  E ── F ── G ── H
  |  / | \/ | \/ |
  I ── J ── K ── L
  |  \ | /\ | /\ |
  M ── N ── O ── P

  16 nodes, 4x4 grid, 1.5m spacing
  Edges: up to 120 (each node connects to all others within range)
  Room size: 4.5m x 4.5m
```

**New Capabilities**:
- Multi-person detection via multi-way mincut (k-cut)
- Boundary tracking across frames (temporal association)
- Adaptive baseline recalibration (furniture changes)
- Channel hopping for improved resolution

**Success Criteria**:
- Track 1-3 people simultaneously
- Boundary position error < 50cm (compared to ground truth)
- Update rate >= 30 Hz (33ms per cycle)
- Handle person entry/exit without false boundaries
- Recover from node failure (1 of 16 goes offline)

**Deliverables**:
- Scalable TDM scheduler for 16 nodes
- Multi-cut solver with temporal tracking
- Boundary tracking with ID assignment
- Performance dashboard showing latency breakdown
- Comparison against camera ground truth

**Timeline**: 8 weeks

```
Week 1-2: Scale TDM to 16 nodes, test reliability
Week 3-4: Multi-cut solver, k-way partitioning
Week 5-6: Temporal tracking, boundary ID persistence
Week 7:   Channel hopping, multi-band fusion
Week 8:   Benchmark suite, ground truth comparison
```

### 5.3 Phase 3: Multi-Room Mesh

**Goal**: Extend to multi-room deployment with hierarchical graph structure.

```
  +------------------+     +------------------+
  |  Room A (16 nodes)|     |  Room B (16 nodes)|
  |                  |     |                  |
  |  Local RfGraph   |     |  Local RfGraph   |
  |                  |     |                  |
  +--------+---------+     +--------+---------+
           |                         |
           | gateway edges           | gateway edges
           |                         |
  +--------+-------------------------+--------+
  |              Hallway (8 nodes)             |
  |           Corridor RfGraph                 |
  +--------+-------------------------+--------+
           |                         |
  +--------+---------+     +--------+---------+
  |  Room C (16 nodes)|     |  Room D (16 nodes)|
  |                  |     |                  |
  +------------------+     +------------------+

  Total: 72 nodes across 5 zones
  Hierarchical mincut: local cuts + cross-zone cuts
```

**New Capabilities**:
- Hierarchical graph: room-level graphs with inter-room gateway edges
- Cross-room person tracking (handoff between local graphs)
- Distributed processing: each room runs its own mincut, global coordinator
  merges boundaries
- Environment fingerprinting (reuse ruvsense/cross_room.rs)
- Fault tolerance: room operates independently if gateway fails

**Success Criteria**:
- Track people across room transitions
- Latency < 100ms even with 72 nodes (via hierarchical decomposition)
- Handle node failures gracefully (degrade, don't crash)
- Boundary accuracy < 50cm within rooms, < 1m across transitions

**Timeline**: 16 weeks

### 5.4 Phase Summary

```
Phase   Nodes   Edges   People   Accuracy   Update Rate   Duration
~~~~~~  ~~~~~~  ~~~~~~  ~~~~~~~  ~~~~~~~~~  ~~~~~~~~~~~   ~~~~~~~~
  1       4       6       1       Quadrant    250 Hz       4 weeks
  2      16     120      1-3      < 50cm       30 Hz       8 weeks
  3      72     ~500     5-10     < 50cm       30 Hz      16 weeks
```

---

## 6. Benchmark

### 6.1 Primary Benchmark: Person Moving Through Room

**Scenario**: A single person walks a known path through the 16-node room
(Phase 2 setup). Ground truth is captured by an overhead camera with
ArUco markers on the person's shoulders.

```
  A ── B ── C ── D
  |    |    |    |
  E ── F ── G ── H
  |    |    |    |         Person path: start at (+), walk to (*),
  I ── J ── K ── L                      then to (#), then exit
  |    |    |    |
  M ── N ── O ── P

  Path:  (+) near F
          |
          v
         (*) near K
          |
          v
         (#) near O
          |
          v
         exit past P
```

### 6.2 Setup

**Hardware**:
- 16x ESP32-S3 DevKitC, mounted at 1.2m height on stands
- Grid spacing: 1.5m
- Room dimensions: 4.5m x 4.5m, cleared of furniture for baseline
- 1x overhead USB camera, 30fps, for ground truth
- 4x ArUco markers on person (shoulders, hips)

**Software**:
- TDM cycle: 16ms (16 nodes x 1ms slots)
- Update rate: 62.5 Hz
- Mincut solver: Stoer-Wagner
- Edge weight: exponential moving average, alpha = 0.3
- Baseline: 60 seconds of empty room calibration

**Environment**:
- Standard office room, concrete walls
- WiFi channel 6 (2.437 GHz), no other AP on same channel
- Temperature: 20-25C (stable)
- Test duration: 5 minutes per run, 10 runs total

### 6.3 Metrics

| Metric                        | Definition                                              | Target      |
|-------------------------------|---------------------------------------------------------|-------------|
| **Boundary Position Error**   | Distance from detected boundary centroid to GT position | < 50cm      |
| **Detection Latency**         | Time from person entering room to first boundary detect | < 500ms     |
| **Tracking Continuity**       | % of frames where boundary is detected while person present | > 95%  |
| **False Positive Rate**       | Boundaries detected per minute when room is empty       | < 0.1/min   |
| **Pipeline Latency (P95)**    | 95th percentile CSI-to-boundary time                    | < 100ms     |
| **Pipeline Latency (P50)**    | Median CSI-to-boundary time                             | < 50ms      |
| **Update Throughput**         | Boundary updates delivered to UI per second              | > 30/s      |
| **Node Failure Recovery**     | Time to stable operation after 1 node goes offline      | < 5s        |

### 6.4 Success Criteria

The benchmark PASSES if ALL of the following hold over 10 runs:

1. Mean boundary position error < 50cm
2. 95th percentile boundary position error < 75cm
3. Detection latency < 500ms in 9/10 runs
4. Tracking continuity > 95% in 9/10 runs
5. Zero false positives in empty room (10-minute test)
6. Pipeline latency P95 < 100ms in all runs
7. No crashes or hangs during any run

### 6.5 Data Collection

```
Output files per run:
  benchmark_run_{N}/
    csi_raw/              # Raw CSI frames, timestamped
    edge_weights/         # Computed weights per edge per frame
    boundaries/           # Detected boundaries with timestamps
    ground_truth/         # Camera-derived positions with timestamps
    latency_log.csv       # Per-frame pipeline timing breakdown
    summary.json          # Aggregate metrics for this run
```

### 6.6 Analysis

Post-benchmark analysis computes:

1. **Error distribution**: Histogram of boundary position errors
2. **Error vs. position**: Heat map of error across the room (corner vs. center)
3. **Latency breakdown**: Stacked bar chart of pipeline stages
4. **Temporal stability**: Boundary position over time vs. ground truth
5. **Edge weight visualization**: Animation of edge weights during walk

Expected failure modes:
- Higher error near room edges (fewer surrounding nodes)
- Brief detection gaps during fast movement
- Increased error when person is exactly between two nodes (ambiguous cut)

---

## 7. ADR-044 Draft

### ADR-044: RF Topological Sensing

**Status**: Proposed

**Date**: 2026-03-08

#### Context

The wifi-densepose system currently estimates human pose by processing CSI
data through neural network models (wifi-densepose-nn). This approach requires
training data, GPU inference, and per-environment calibration of the neural
model. The RuvSense multistatic sensing mode (ADR-029) improved robustness
through multi-link fusion but still treats each link independently before
fusion.

A fundamentally different approach is possible: treat the entire ESP32 mesh
as a graph where TX-RX pairs are edges and CSI coherence determines edge
weights. A minimum cut of this graph reveals physical boundaries — the
locations where radio propagation is disrupted by human bodies. This is
"RF topological sensing."

Key motivations:
- **No training data required**: The mincut is a pure graph algorithm, not a
  learned model. It works out of the box after baseline calibration.
- **Physics-grounded**: The approach directly exploits the physical fact that
  human bodies attenuate and scatter radio waves.
- **Graceful degradation**: If nodes fail, the graph simply has fewer edges.
  The mincut still works, with reduced resolution.
- **Complementary to neural approach**: Topological boundaries can provide
  spatial priors to the neural pose estimator, improving accuracy.

#### Decision

We will implement RF topological sensing as a new module in the workspace.
The module will:

1. Define an RfGraph aggregate root that maintains a weighted graph of all
   TX-RX links in the mesh.

2. Compute edge weights from CSI coherence using existing ruvsense modules
   (coherence.rs, phase_align.rs).

3. Run dynamic minimum cut to detect physical boundaries in real time.

4. Expose boundaries via the sensing server WebSocket for visualization.

5. Publish TopologyEvents that downstream modules (pose_tracker, intention)
   can consume for spatial priors.

The implementation will proceed in three phases:
- Phase 1: 4-node proof of concept (detect person presence)
- Phase 2: 16-node room scale (track boundaries with < 50cm error)
- Phase 3: Multi-room mesh with hierarchical graph decomposition

#### Consequences

**Positive**:
- Enables WiFi sensing without neural network inference or training data
- Provides spatial boundary information that is complementary to pose estimation
- Reuses existing ruvsense modules for coherence and phase alignment
- Follows DDD patterns established in ruvector/viewpoint/fusion.rs
- Gracefully degrades under node failure
- Sub-100ms latency enables real-time applications

**Negative**:
- Requires minimum 4 ESP32 nodes (higher hardware cost than single-link)
- Mincut provides boundaries, not poses — pose still requires neural inference
  or additional geometric reasoning
- Stoer-Wagner complexity O(V*E + V^2 log V) limits scalability beyond ~40 nodes
  without incremental solver
- Additional firmware complexity for TDM synchronization across many nodes
- New testing infrastructure needed for graph algorithms

**Neutral**:
- Does not replace existing neural pose estimation; supplements it
- Phase 1 can validate the approach before committing to full implementation
- May inform future ADRs on distributed sensing architecture

#### References

- ADR-029: RuvSense multistatic sensing mode
- ADR-028: ESP32 capability audit
- ADR-014: SOTA signal processing
- Research Doc 10: This document

---

## 8. Rust Trait Definitions

### 8.1 Core Traits

```rust
/// Unique identifier for a node in the RF mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u16);

/// Unique identifier for an edge (ordered pair of nodes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EdgeId {
    pub tx: NodeId,
    pub rx: NodeId,
}

impl EdgeId {
    /// Create a canonical edge ID where tx < rx to avoid duplicates.
    pub fn canonical(a: NodeId, b: NodeId) -> Self {
        if a.0 <= b.0 {
            Self { tx: a, rx: b }
        } else {
            Self { tx: b, rx: a }
        }
    }
}

/// Physical position of a node in 2D space (meters).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Position2D {
    pub x: f64,
    pub y: f64,
}

/// Information about a node in the mesh.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub id: NodeId,
    pub position: Position2D,
    pub mac_address: [u8; 6],
    pub tdm_slot: u8,
    pub joined_at: u64, // unix timestamp ms
}
```

### 8.2 Edge Weight Trait

```rust
/// Trait for computing edge weights from CSI measurements.
pub trait EdgeWeightComputer: Send + Sync {
    /// Compute the weight for an edge given current and baseline CSI.
    fn compute(
        &self,
        current: &CsiFrame,
        baseline: &CsiFrame,
        config: &EdgeWeightConfig,
    ) -> Result<EdgeWeight, TopologyError>;

    /// Update the temporal average for an edge.
    fn update_average(
        &self,
        previous: &EdgeWeight,
        new_sample: &EdgeWeight,
        alpha: f64,
    ) -> EdgeWeight;
}

/// Configuration for edge weight computation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeWeightConfig {
    /// Exponential moving average smoothing factor.
    pub ema_alpha: f64,
    /// Minimum confidence to accept a measurement.
    pub min_confidence: f64,
    /// Number of subcarriers to use (0 = all).
    pub subcarrier_count: usize,
    /// Enable adversarial physics check.
    pub physics_check: bool,
}

impl Default for EdgeWeightConfig {
    fn default() -> Self {
        Self {
            ema_alpha: 0.3,
            min_confidence: 0.5,
            subcarrier_count: 0,
            physics_check: true,
        }
    }
}
```

### 8.3 Graph Trait

```rust
/// Trait for the RF topology graph.
pub trait TopologyGraph: Send + Sync {
    /// Add a node to the graph.
    fn add_node(&mut self, node: NodeInfo) -> Result<Vec<TopologyEvent>, TopologyError>;

    /// Remove a node and all its edges.
    fn remove_node(&mut self, id: NodeId) -> Result<Vec<TopologyEvent>, TopologyError>;

    /// Update the weight of an edge. Creates the edge if it doesn't exist.
    fn update_edge(
        &mut self,
        edge: EdgeId,
        weight: EdgeWeight,
    ) -> Result<Vec<TopologyEvent>, TopologyError>;

    /// Remove edges that haven't been updated in `max_age` duration.
    fn prune_stale(&mut self, max_age: std::time::Duration) -> Vec<TopologyEvent>;

    /// Get the current weight of an edge.
    fn edge_weight(&self, edge: EdgeId) -> Option<&EdgeWeight>;

    /// Get all edges as (EdgeId, weight) pairs.
    fn edges(&self) -> Vec<(EdgeId, f64)>;

    /// Get the number of nodes.
    fn node_count(&self) -> usize;

    /// Get the number of edges.
    fn edge_count(&self) -> usize;

    /// Check if the graph is connected.
    fn is_connected(&self) -> bool;

    /// Get a snapshot of the adjacency matrix for mincut computation.
    fn adjacency_matrix(&self) -> AdjacencyMatrix;
}
```

### 8.4 Mincut Solver Trait

```rust
/// Result of a minimum cut computation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinCutResult {
    /// Edges that form the minimum cut.
    pub cut_edges: Vec<EdgeId>,
    /// Total weight of the cut.
    pub cut_value: f64,
    /// Nodes in partition A.
    pub partition_a: Vec<NodeId>,
    /// Nodes in partition B.
    pub partition_b: Vec<NodeId>,
}

/// Trait for minimum cut solvers.
pub trait MinCutSolver: Send + Sync {
    /// Compute the global minimum cut of the graph.
    fn min_cut(&self, graph: &AdjacencyMatrix) -> Result<MinCutResult, TopologyError>;

    /// Compute a k-way minimum cut (for multi-person detection).
    fn k_cut(
        &self,
        graph: &AdjacencyMatrix,
        k: usize,
    ) -> Result<Vec<MinCutResult>, TopologyError>;

    /// Incrementally update the cut after edge weight changes.
    /// Returns None if the cut topology hasn't changed.
    fn incremental_update(
        &self,
        previous_cut: &MinCutResult,
        changed_edges: &[(EdgeId, f64, f64)], // (edge, old_weight, new_weight)
        graph: &AdjacencyMatrix,
    ) -> Result<Option<MinCutResult>, TopologyError>;
}

/// Stoer-Wagner implementation of MinCutSolver.
pub struct StoerWagnerSolver {
    /// Cache the last contraction order for incremental updates.
    last_contraction: Option<Vec<(NodeId, NodeId)>>,
}

impl MinCutSolver for StoerWagnerSolver {
    fn min_cut(&self, graph: &AdjacencyMatrix) -> Result<MinCutResult, TopologyError> {
        // Stoer-Wagner algorithm:
        // 1. Start with arbitrary node
        // 2. Repeatedly add "most tightly connected" node
        // 3. Last two nodes define a cut candidate
        // 4. Merge last two nodes, repeat
        // 5. Return minimum cut found across all phases
        todo!("Implement Stoer-Wagner")
    }

    fn k_cut(
        &self,
        graph: &AdjacencyMatrix,
        k: usize,
    ) -> Result<Vec<MinCutResult>, TopologyError> {
        // Recursive approach:
        // 1. Find global mincut -> 2 partitions
        // 2. Recursively find mincut in larger partition
        // 3. Repeat until k partitions
        todo!("Implement recursive k-cut")
    }

    fn incremental_update(
        &self,
        previous_cut: &MinCutResult,
        changed_edges: &[(EdgeId, f64, f64)],
        graph: &AdjacencyMatrix,
    ) -> Result<Option<MinCutResult>, TopologyError> {
        // Heuristic: if no changed edge crosses the previous cut,
        // and no weight changed by more than threshold, keep previous cut.
        let cut_edge_set: std::collections::HashSet<_> =
            previous_cut.cut_edges.iter().collect();

        let significant_change = changed_edges.iter().any(|(edge, old, new)| {
            let delta = (new - old).abs();
            cut_edge_set.contains(edge) && delta > 0.1
        });

        if !significant_change {
            return Ok(None); // Cut unchanged
        }

        // Recompute full mincut
        self.min_cut(graph).map(Some)
    }
}
```

### 8.5 Boundary Interpolation Trait

```rust
/// A polygon representing a physical boundary in 2D space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundaryPolygon {
    /// Vertices of the boundary polygon (meters, room coordinates).
    pub vertices: Vec<Position2D>,
    /// Confidence of this boundary (0.0 to 1.0).
    pub confidence: f64,
    /// Unique ID for tracking across frames.
    pub boundary_id: u64,
    /// Timestamp of detection.
    pub detected_at_ms: u64,
}

/// Trait for converting graph cuts into spatial boundaries.
pub trait BoundaryInterpolator: Send + Sync {
    /// Convert a minimum cut result into a spatial boundary polygon.
    fn interpolate(
        &self,
        cut: &MinCutResult,
        node_positions: &std::collections::HashMap<NodeId, Position2D>,
    ) -> Result<BoundaryPolygon, TopologyError>;

    /// Smooth a boundary using previous frame's boundary (temporal filtering).
    fn smooth(
        &self,
        current: &BoundaryPolygon,
        previous: &BoundaryPolygon,
        alpha: f64,
    ) -> BoundaryPolygon;
}

/// Midpoint interpolation: boundary passes through midpoints of cut edges.
pub struct MidpointInterpolator;

impl BoundaryInterpolator for MidpointInterpolator {
    fn interpolate(
        &self,
        cut: &MinCutResult,
        node_positions: &std::collections::HashMap<NodeId, Position2D>,
    ) -> Result<BoundaryPolygon, TopologyError> {
        let mut midpoints: Vec<Position2D> = Vec::new();

        for edge in &cut.cut_edges {
            let pos_a = node_positions
                .get(&edge.tx)
                .ok_or(TopologyError::NodeNotFound(edge.tx))?;
            let pos_b = node_positions
                .get(&edge.rx)
                .ok_or(TopologyError::NodeNotFound(edge.rx))?;

            midpoints.push(Position2D {
                x: (pos_a.x + pos_b.x) / 2.0,
                y: (pos_a.y + pos_b.y) / 2.0,
            });
        }

        // Order midpoints to form a non-self-intersecting polygon
        // using angular sort around centroid
        let cx: f64 = midpoints.iter().map(|p| p.x).sum::<f64>() / midpoints.len() as f64;
        let cy: f64 = midpoints.iter().map(|p| p.y).sum::<f64>() / midpoints.len() as f64;

        midpoints.sort_by(|a, b| {
            let angle_a = (a.y - cy).atan2(a.x - cx);
            let angle_b = (b.y - cy).atan2(b.x - cx);
            angle_a.partial_cmp(&angle_b).unwrap()
        });

        Ok(BoundaryPolygon {
            vertices: midpoints,
            confidence: 1.0 - cut.cut_value, // lower cut value = more confident
            boundary_id: 0, // assigned by tracker
            detected_at_ms: 0, // set by caller
        })
    }

    fn smooth(
        &self,
        current: &BoundaryPolygon,
        previous: &BoundaryPolygon,
        alpha: f64,
    ) -> BoundaryPolygon {
        // Simple vertex-wise EMA when vertex counts match
        if current.vertices.len() != previous.vertices.len() {
            return current.clone();
        }

        let smoothed: Vec<Position2D> = current
            .vertices
            .iter()
            .zip(previous.vertices.iter())
            .map(|(c, p)| Position2D {
                x: alpha * c.x + (1.0 - alpha) * p.x,
                y: alpha * c.y + (1.0 - alpha) * p.y,
            })
            .collect();

        BoundaryPolygon {
            vertices: smoothed,
            confidence: alpha * current.confidence + (1.0 - alpha) * previous.confidence,
            boundary_id: current.boundary_id,
            detected_at_ms: current.detected_at_ms,
        }
    }
}
```

### 8.6 Pipeline Orchestrator

```rust
/// The main pipeline that ties all stages together.
pub struct TopologyPipeline {
    graph: Box<dyn TopologyGraph>,
    weight_computer: Box<dyn EdgeWeightComputer>,
    mincut_solver: Box<dyn MinCutSolver>,
    boundary_interpolator: Box<dyn BoundaryInterpolator>,
    event_tx: tokio::sync::broadcast::Sender<TopologyEvent>,
    config: PipelineConfig,
    baselines: std::collections::HashMap<EdgeId, CsiFrame>,
    last_cut: Option<MinCutResult>,
    last_boundary: Option<BoundaryPolygon>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    /// Maximum age before an edge is pruned.
    pub stale_edge_timeout_ms: u64,
    /// Edge weight computation config.
    pub edge_weight: EdgeWeightConfig,
    /// Minimum cut value change to trigger boundary update.
    pub cut_change_threshold: f64,
    /// Temporal smoothing factor for boundary polygon.
    pub boundary_smoothing_alpha: f64,
    /// Maximum number of simultaneous boundaries to track.
    pub max_boundaries: usize,
}

impl TopologyPipeline {
    /// Process a batch of CSI frames from one TDM cycle.
    ///
    /// This is the main entry point, called once per TDM cycle.
    /// Returns all topology events generated during processing.
    pub async fn process_cycle(
        &mut self,
        frames: Vec<CsiFrame>,
    ) -> Result<Vec<TopologyEvent>, TopologyError> {
        let mut all_events = Vec::new();

        // Stage 2-3: Compute edge weights and update graph (parallel)
        let weights: Vec<(EdgeId, EdgeWeight)> = frames
            .par_iter()
            .filter_map(|frame| {
                let edge = EdgeId::canonical(
                    NodeId(frame.tx_id),
                    NodeId(frame.rx_id),
                );
                let baseline = self.baselines.get(&edge)?;
                let weight = self.weight_computer
                    .compute(frame, baseline, &self.config.edge_weight)
                    .ok()?;
                Some((edge, weight))
            })
            .collect();

        // Stage 3: Update graph
        let mut changed_edges = Vec::new();
        for (edge_id, weight) in &weights {
            let old_weight = self.graph
                .edge_weight(*edge_id)
                .map(|w| w.weight)
                .unwrap_or(1.0);
            let events = self.graph.update_edge(*edge_id, weight.clone())?;
            changed_edges.push((*edge_id, old_weight, weight.weight));
            all_events.extend(events);
        }

        // Prune stale edges
        let stale_timeout =
            std::time::Duration::from_millis(self.config.stale_edge_timeout_ms);
        let prune_events = self.graph.prune_stale(stale_timeout);
        all_events.extend(prune_events);

        // Stage 4: Mincut
        let adjacency = self.graph.adjacency_matrix();
        let cut_result = if let Some(ref prev_cut) = self.last_cut {
            self.mincut_solver
                .incremental_update(prev_cut, &changed_edges, &adjacency)?
                .unwrap_or_else(|| prev_cut.clone())
        } else {
            self.mincut_solver.min_cut(&adjacency)?
        };
        self.last_cut = Some(cut_result.clone());

        // Stage 5: Boundary interpolation
        let node_positions = self.node_position_map();
        let mut boundary = self
            .boundary_interpolator
            .interpolate(&cut_result, &node_positions)?;

        // Temporal smoothing
        if let Some(ref prev_boundary) = self.last_boundary {
            boundary = self.boundary_interpolator.smooth(
                &boundary,
                prev_boundary,
                self.config.boundary_smoothing_alpha,
            );
        }
        self.last_boundary = Some(boundary.clone());

        // Emit boundary event
        all_events.push(TopologyEvent {
            id: EventId::new(),
            timestamp: std::time::Instant::now(),
            kind: TopologyEventKind::BoundaryDetected {
                boundary: CutBoundary {
                    cut_edges: cut_result.cut_edges,
                    cut_value: cut_result.cut_value,
                    partition_a: cut_result.partition_a,
                    partition_b: cut_result.partition_b,
                    spatial_boundary: Some(boundary),
                    confidence: cut_result.cut_value,
                    detected_at: std::time::Instant::now(),
                },
            },
        });

        // Broadcast events
        for event in &all_events {
            let _ = self.event_tx.send(event.clone());
        }

        Ok(all_events)
    }

    fn node_position_map(&self) -> std::collections::HashMap<NodeId, Position2D> {
        // Build from graph's node registry
        todo!("Extract node positions from graph")
    }
}
```

### 8.7 Error Types

```rust
/// Errors that can occur in the topology pipeline.
#[derive(Debug, thiserror::Error)]
pub enum TopologyError {
    #[error("Node not found: {0:?}")]
    NodeNotFound(NodeId),

    #[error("Edge not found: {0:?} -> {1:?}")]
    EdgeNotFound(NodeId, NodeId),

    #[error("Graph is disconnected: {0} components")]
    GraphDisconnected(usize),

    #[error("Insufficient nodes for mincut: need >= 2, have {0}")]
    InsufficientNodes(usize),

    #[error("Baseline not available for edge {0:?}")]
    NoBaseline(EdgeId),

    #[error("CSI frame invalid: {0}")]
    InvalidCsiFrame(String),

    #[error("Mincut solver failed: {0}")]
    SolverError(String),

    #[error("Calibration required: {0}")]
    CalibrationRequired(String),
}
```

### 8.8 Adjacency Matrix

```rust
/// Dense adjacency matrix for mincut computation.
///
/// Uses a flat Vec<f64> for cache-friendly access. Indexed as
/// matrix[row * dimension + col].
#[derive(Debug, Clone)]
pub struct AdjacencyMatrix {
    /// Node IDs in index order.
    pub nodes: Vec<NodeId>,
    /// Flat weight matrix (dimension x dimension).
    pub weights: Vec<f64>,
    /// Matrix dimension (= nodes.len()).
    pub dimension: usize,
}

impl AdjacencyMatrix {
    pub fn new(nodes: Vec<NodeId>) -> Self {
        let dim = nodes.len();
        Self {
            nodes,
            weights: vec![0.0; dim * dim],
            dimension: dim,
        }
    }

    pub fn get(&self, row: usize, col: usize) -> f64 {
        self.weights[row * self.dimension + col]
    }

    pub fn set(&mut self, row: usize, col: usize, value: f64) {
        self.weights[row * self.dimension + col] = value;
        self.weights[col * self.dimension + row] = value; // symmetric
    }

    /// Find the index of a node, or None if not present.
    pub fn node_index(&self, id: NodeId) -> Option<usize> {
        self.nodes.iter().position(|n| *n == id)
    }
}
```

---

## Appendix A: Glossary

| Term                  | Definition                                                        |
|-----------------------|-------------------------------------------------------------------|
| CSI                   | Channel State Information -- per-subcarrier complex amplitude     |
| TDM                   | Time Division Multiplexing -- collision-free TX scheduling        |
| Mincut                | Minimum cut -- partition of graph that minimizes total edge weight |
| Stoer-Wagner          | Deterministic O(VE + V^2 log V) mincut algorithm                 |
| Edge weight           | Coherence metric on a TX-RX link; low = obstructed               |
| Boundary              | Spatial region where mincut edges intersect physical space        |
| Aggregate root        | DDD pattern -- single entry point for a consistency boundary      |
| EMA                   | Exponential Moving Average -- temporal smoothing filter           |

## Appendix B: Related ADRs

| ADR   | Title                                  | Relevance                          |
|-------|----------------------------------------|------------------------------------|
| 014   | SOTA signal processing                 | Coherence and phase algorithms     |
| 028   | ESP32 capability audit                 | Hardware constraints and TDM       |
| 029   | RuvSense multistatic sensing           | Multi-link fusion architecture     |
| 030   | RuvSense persistent field model        | Baseline calibration approach      |
| 031   | RuView sensing-first RF mode           | UI integration pattern             |
| 044   | RF Topological Sensing (this doc)      | Architecture decision              |

## Appendix C: Open Questions

1. **Stoer-Wagner vs. Push-Relabel**: Which mincut algorithm is better for
   incremental updates? Push-relabel may allow warm-starting from previous
   flow solution.

2. **Multi-person disambiguation**: When k-cut finds multiple boundaries, how
   do we associate boundaries across frames? Nearest-neighbor in spatial
   coordinates? Hungarian algorithm on boundary centroids?

3. **3D extension**: The current design is 2D (nodes at fixed height). Can we
   extend to 3D by placing nodes at multiple heights? How does this affect
   mincut interpretation?

4. **Furniture vs. people**: Both attenuate CSI. Baseline calibration handles
   static furniture, but what about moved chairs? Adaptive baseline with slow
   drift tracking (ruvsense/longitudinal.rs) may help.

5. **Optimal node placement**: Given a room geometry, where should N nodes be
   placed to maximize boundary resolution? This is related to sensor placement
   optimization and Fisher Information from ruvector/viewpoint/geometry.rs.

6. **Latency at scale**: The 100ms budget assumes local processing. If graph
   data must traverse a network (multi-room, Phase 3), how do we maintain
   latency? Hierarchical decomposition with local mincut per room is the
   current proposal.

---

*End of Research Document 10*
