# ADR-068: Per-Node State Pipeline for Multi-Node Sensing

| Field      | Value                               |
|------------|-------------------------------------|
| Status     | Accepted                            |
| Date       | 2026-03-27                          |
| Authors    | rUv, claude-flow                    |
| Drivers    | #249, #237, #276, #282              |
| Supersedes | —                                   |

## Context

The sensing server (`wifi-densepose-sensing-server`) was originally designed for
single-node operation. When multiple ESP32 nodes send CSI frames simultaneously,
all data is mixed into a single shared pipeline:

- **One** `frame_history` VecDeque for all nodes
- **One** `smoothed_person_score` / `smoothed_motion` / vital sign buffers
- **One** baseline and debounce state

This means the classification, person count, and vital signs reported to the UI
are an uncontrolled aggregate of all nodes' data. The result: the detection
window shows identical output regardless of how many nodes are deployed, where
people stand, or how many people are in the room (#249 — 24 comments, the most
reported issue).

### Root Cause Verified

Investigation of `AppStateInner` (main.rs lines 279-367) confirmed:

| Shared field              | Impact                                     |
|---------------------------|--------------------------------------------|
| `frame_history`           | Temporal analysis mixes all nodes' CSI data |
| `smoothed_person_score`   | Person count aggregates all nodes           |
| `smoothed_motion`         | Motion classification undifferentiated      |
| `smoothed_hr` / `br`     | Vital signs are global, not per-node        |
| `baseline_motion`         | Adaptive baseline learned from mixed data   |
| `debounce_counter`        | All nodes share debounce state              |

## Decision

Introduce **per-node state tracking** via a `HashMap<u8, NodeState>` in
`AppStateInner`. Each ESP32 node (identified by its `node_id` byte) gets an
independent sensing pipeline with its own temporal history, smoothing buffers,
baseline, and classification state.

### Architecture

```
                     ┌─────────────────────────────────────────┐
   UDP frames        │           AppStateInner                  │
   ───────────►      │                                         │
   node_id=1    ──►  │  node_states: HashMap<u8, NodeState>    │
   node_id=2    ──►  │    ├── 1: NodeState { frame_history,    │
   node_id=3    ──►  │    │      smoothed_motion, vitals, ... }│
                     │    ├── 2: NodeState { ... }              │
                     │    └── 3: NodeState { ... }              │
                     │                                         │
                     │  ┌── Per-Node Pipeline ──┐               │
                     │  │ extract_features()     │               │
                     │  │ smooth_and_classify()  │               │
                     │  │ smooth_vitals()        │               │
                     │  │ score_to_person_count()│               │
                     │  └────────────────────────┘               │
                     │                                         │
                     │  ┌── Multi-Node Fusion ──┐               │
                     │  │ Aggregate person count │               │
                     │  │ Per-node classification│               │
                     │  │ All-nodes WebSocket msg│               │
                     │  └────────────────────────┘               │
                     │                                         │
                     │  ──► WebSocket broadcast (sensing_update) │
                     └─────────────────────────────────────────┘
```

### NodeState Struct

```rust
struct NodeState {
    frame_history: VecDeque<Vec<f64>>,
    smoothed_person_score: f64,
    prev_person_count: usize,
    smoothed_motion: f64,
    current_motion_level: String,
    debounce_counter: u32,
    debounce_candidate: String,
    baseline_motion: f64,
    baseline_frames: u64,
    smoothed_hr: f64,
    smoothed_br: f64,
    smoothed_hr_conf: f64,
    smoothed_br_conf: f64,
    hr_buffer: VecDeque<f64>,
    br_buffer: VecDeque<f64>,
    rssi_history: VecDeque<f64>,
    vital_detector: VitalSignDetector,
    latest_vitals: VitalSigns,
    last_frame_time: Option<std::time::Instant>,
    edge_vitals: Option<Esp32VitalsPacket>,
}
```

### Multi-Node Aggregation

- **Person count**: Sum of per-node `prev_person_count` for active nodes
  (seen within last 10 seconds).
- **Classification**: Per-node classification included in `SensingUpdate.nodes`.
- **Vital signs**: Per-node vital signs; UI can render per-node or aggregate.
- **Signal field**: Generated from the most-recently-updated node's features.
- **Stale nodes**: Nodes with no frame for >10 seconds are excluded from
  aggregation and marked offline (consistent with PR #300).

### Backward Compatibility

- The simulated data path (`simulated_data_task`) continues using global state.
- Single-node deployments behave identically (HashMap has one entry).
- The WebSocket message format (`sensing_update`) remains the same but the
  `nodes` array now contains all active nodes, and `estimated_persons` reflects
  the cross-node aggregate.
- The edge vitals path (#323 fix) also uses per-node state.

## Scaling Characteristics

| Nodes | Per-Node Memory | Total Overhead | Notes |
|-------|----------------|----------------|-------|
| 1     | ~50 KB         | ~50 KB         | Identical to current |
| 3     | ~50 KB         | ~150 KB        | Typical home setup |
| 10    | ~50 KB         | ~500 KB        | Small office |
| 50    | ~50 KB         | ~2.5 MB        | Building floor |
| 100   | ~50 KB         | ~5 MB          | Large deployment |
| 256   | ~50 KB         | ~12.8 MB       | Max (u8 node_id) |

Memory is dominated by `frame_history` (100 frames x ~500 bytes each = ~50 KB
per node). This scales linearly and fits comfortably in server memory even at
256 nodes.

## QEMU Validation

The existing QEMU swarm infrastructure (ADR-062, `scripts/qemu_swarm.py`)
supports multi-node simulation with configurable topologies:

- `star`: Central coordinator + sensor nodes
- `mesh`: Fully connected peer network
- `line`: Sequential chain
- `ring`: Circular topology

Each QEMU instance runs with a unique `node_id` via NVS provisioning. The
swarm health validator (`scripts/swarm_health.py`) checks per-node UART output.

Validation plan:
1. QEMU swarm with 3-5 nodes in mesh topology
2. Verify server produces distinct per-node classifications
3. Verify aggregate person count reflects multi-node contributions
4. Verify stale-node eviction after timeout

## Consequences

### Positive
- Each node's CSI data is processed independently — no cross-contamination
- Person count scales with the number of deployed nodes
- Vital signs are per-node, enabling room-level health monitoring
- Foundation for spatial localization (per-node positions + triangulation)
- Scales to 256 nodes with <13 MB memory overhead

### Negative
- Slightly more memory per node (~50 KB each)
- `smooth_and_classify_node` function duplicates some logic from global version
- Per-node `VitalSignDetector` instances add CPU cost proportional to node count

### Risks
- Node ID collisions (mitigated by NVS persistence since v0.5.0)
- HashMap growth without cleanup (mitigated by stale-node eviction)

## References

- Issue #249: Detection window same regardless (24 comments)
- Issue #237: Same display for 0/1/2 people (12 comments)
- Issue #276: Only one can be detected (8 comments)
- Issue #282: Detection fail (5 comments)
- PR #295: Hysteresis smoothing (partial mitigation)
- PR #300: ESP32 offline detection after 5s
- ADR-062: QEMU Swarm Configurator
