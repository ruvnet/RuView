# ADR-139: WorldGraph: Environmental Digital Twin with Typed Petgraph

| Field | Value |
|-------|-------|
| **Status** | Accepted — partial (built + tested building block; integration glue pending — see Implementation Status, commit `521a012d8`) |
| **Date** | 2026-05-28 |
| **Deciders** | ruv |
| **Codebase target** | New module/crate `wifi-densepose-worldgraph` alongside `v2/crates/wifi-densepose-geo` and `v2/crates/homecore`; petgraph bridge pattern from `v2/crates/ruv-neural/ruv-neural-graph/src/petgraph_bridge.rs`; integrates `homecore/src/registry.rs` `area_id` and `wifi-densepose-mat/src/domain/scan_zone.rs` |
| **Relates to** | ADR-044 (Geospatial Satellite Integration), ADR-113 (Multistatic Placement Strategy), ADR-127 (HomeCore State Machine), ADR-030 (Persistent Field Model), ADR-136 (RuView Streaming Engine), ADR-137 (Fusion Quality Scoring), ADR-138 (LinkGroup / ArrayCoordinator), ADR-142 (Evolution Tracker), ADR-144 (UWB Range-Constraint Fusion), ADR-145 (Ablation Eval Harness) |

---

## 1. Context

### 1.1 The Gap

There is no single, queryable model of *the environment a RuView installation senses*. The spatial knowledge that exists in the workspace is fragmented across four crates, each holding one projection of "where things are" with no edges connecting them:

- **`v2/crates/wifi-densepose-geo`** holds the *outdoor / global* frame. `src/types.rs` defines `GeoPoint { lat, lon, alt }` (the ADR-044 WGS84 anchor), `GeoBBox`, `GeoScene`, and `GeoRegistration { origin, heading_deg, scale }`. `src/coord.rs` implements `wgs84_to_enu()` / `enu_to_wgs84()` — the exact transform needed to pin a room into a local East-North-Up frame relative to a `GeoPoint`. But `GeoScene` only models buildings and roads (`OsmFeature::Building`, `OsmFeature::Road`); it has no concept of an interior room, wall, doorway, sensor placement, or a person inside.
- **`v2/crates/homecore/src/registry.rs`** holds the *entity / automation* frame. `EntityEntry` carries `area_id: Option<String>` and `device_id: Option<String>` (mirroring Home Assistant `core.entity_registry` v13 per ADR-127). This is the canonical handle for "which room an entity is in" — but `area_id` is an opaque string with no geometry, no adjacency, and no link to the sensors that observe it.
- **`v2/crates/wifi-densepose-mat/src/domain/scan_zone.rs`** holds the *sensing geometry* frame. `ScanZone` has `ZoneBounds` (Rectangle/Circle/Polygon), `SensorPosition { id, x, y, z, sensor_type }`, and `contains_point()`. This is the only place that knows sensor coordinates relative to a monitored area — but its coordinates are bare `f64` meters with no declared origin, no link to `homecore` `area_id`, and no link to a `GeoPoint`.
- **`v2/crates/ruv-neural/ruv-neural-graph/src/petgraph_bridge.rs`** demonstrates the *graph algorithm* pattern we want: it bridges a domain `BrainGraph` to `petgraph::graph::{Graph, UnGraph}` (`to_petgraph()` / `from_petgraph()`) so that petgraph's traversal/shortest-path algorithms run over a typed domain model. But its nodes are bare `usize` and its edges carry only an `f64` weight plus a `ConnectivityMetric` enum — there is no node *type* and no edge *semantics*. It is the right mechanical pattern, the wrong domain.

Concretely, what is **missing**:

1. **No node typing.** Nothing in the workspace represents `room`, `zone`, `wall`, `doorway`, `sensor`, `rf_link`, `person_track`, `object_anchor`, `event`, or `semantic_state` as first-class graph nodes with a shared identity space.
2. **No typed edges.** There is no `observes` edge (sensor → node), no `located_in` (person → room), no `adjacent_to` (room ↔ room through a doorway), no `supports` / `contradicts` (evidence relations), no `derived_from` (provenance), and no `privacy_limited_by` (sensor capability constrained by a privacy mode).
3. **No provenance / contradiction tracking.** ADR-137's fusion engine produces `EvidenceRef` and `ContradictionFlag` records, but there is nowhere to *attach* them — they cannot point at the world entity they support or contradict.
4. **No privacy-impact rollup.** ADR-141's privacy control plane will define named modes and per-action allow/deny, but no structure answers "given the current mode, which world nodes can sensor X still observe?"
5. **No persistence of topology.** Each of the four crates persists independently (HomeCore to `core.entity_registry`, geo to a tile cache, MAT in memory). There is no single artifact a RuView appliance can load at boot to reconstitute "the rooms, the sensors, who's where, and why we believe it."

This ADR closes the gap with a **WorldGraph**: a typed `petgraph` over a serde-serializable node enum and typed edges, persisted as an RVF bundle, pinned to a `GeoPoint`, keyed by HomeCore `area_id`, and carrying ADR-137 evidence/contradiction provenance plus ADR-141 privacy constraints.

### 1.2 What "WorldGraph" Means Here

The WorldGraph is an **environmental digital twin** of a *single installation*: the static room/zone/wall/doorway/sensor topology plus the dynamic person/object/event/semantic overlay that sensing produces. It is:

- A `petgraph::stable_graph::StableDiGraph<WorldNode, WorldEdge>` (directed; stable indices so node removal does not invalidate other handles).
- The single authority for *spatial identity*: every `area_id` in HomeCore, every `ScanZone` in MAT, and every sensor placement in ADR-113 maps to exactly one WorldGraph node.
- Append-with-provenance, not overwrite: a node update that supersedes a prior belief adds a `derived_from` edge to the old state and (when sources disagree) a `contradicts` edge, so the graph retains *why* it holds its current belief.

It is **not**:

- A real-time per-frame buffer. The streaming engine (ADR-136) owns per-frame data; the WorldGraph is updated at the *event / semantic-state* cadence (sub-Hz to low-Hz), not the 20 Hz CSI cadence.
- A geometry/CAD engine. Walls and doorways are coarse topological elements (an adjacency relation + a 2D segment), not a BIM model.
- A temporal reconfiguration history. v1 models the *current* static topology only; topology reconfiguration history is deferred to ADR-142's evolution tracker (see §2.7).

### 1.3 Frame and Identity Context

A WorldGraph is pinned to one `GeoRegistration { origin: GeoPoint, heading_deg, scale }` (ADR-044, already in `geo/src/types.rs`). All interior coordinates are **local ENU meters** relative to `origin`, exactly the frame produced by `geo::coord::wgs84_to_enu()`. This means:

- A `room`/`zone` node carries its `ScanZone`-style `ZoneBounds` in ENU meters and can be re-projected to WGS84 via `enu_to_wgs84()` for the ADR-044 map overlay.
- A `sensor` node reuses the `SensorPosition { x, y, z }` semantics from `scan_zone.rs`, now anchored to the installation origin.
- A `room`/`zone` node carries `area_id: Option<String>` so a HomeCore `EntityEntry.area_id` resolves to exactly one WorldGraph node (entity linkage per ADR-127).

### 1.4 Pipeline Position

```
                  ADR-044 GeoPoint / GeoRegistration (installation origin)
                                 │  pins local ENU frame
                                 ▼
  ADR-136 streaming frames ─► ADR-137 FusionEngine ─► (EvidenceRef, ContradictionFlag)
                                 │                                │
                                 │ person/object/event             │ provenance
                                 ▼                                ▼
  ADR-113 sensor placement ─► ┌──────────────── WorldGraph ───────────────────┐
  ADR-138 LinkGroup        ─► │ nodes: room/zone/wall/doorway/sensor/rf_link/  │
  homecore area_id         ─► │        person_track/object_anchor/event/       │
  MAT ScanZone bounds      ─► │        semantic_state                          │
                              │ edges: observes/located_in/adjacent_to/        │
  ADR-141 privacy modes ───► │        supports/contradicts/derived_from/       │
                              │        privacy_limited_by                       │
                              └───────────────┬───────────────┬───────────────┘
                                              │ query API     │ RVF write-through
                                              ▼               ▼
                       observability / location / privacy   .rvf bundle (persisted)
                       rollup queries (ADR-140, ADR-144,
                       ADR-145 consume)
```

The WorldGraph sits *downstream* of fusion (it stores fused beliefs, not raw frames) and *upstream* of the semantic/agent layer (ADR-140) and evaluation harness (ADR-145). ADR-144 (UWB range constraints) reads `sensor`/`object_anchor` nodes as the anchor set for range-constraint solving.

---

## 2. Decision

### 2.1 Node and Edge Model: serde Enum, Not Trait Objects

Nodes are a **`#[derive(Serialize, Deserialize)]` enum**, not boxed trait objects. This is the single most consequential decision: a serde enum gives deterministic, schema-versioned, RVF-friendly persistence (every variant serializes to the same wire layout regardless of build), whereas `Box<dyn WorldNodeTrait>` would require `typetag` (an extra dependency, non-deterministic across crate versions) and could not be field-walked by an evaluation harness. The `petgraph_bridge.rs` precedent already stores concrete weights (`usize`, `f64`) rather than trait objects; we extend that to a typed enum.

```rust
//! v2/crates/wifi-densepose-worldgraph/src/model.rs

use serde::{Deserialize, Serialize};
use wifi_densepose_geo::types::GeoRegistration; // ADR-044

/// Stable, monotonic identity for a world entity. Distinct from petgraph's
/// NodeIndex (which is a graph-internal handle); WorldId survives RVF
/// round-trips and node removal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorldId(pub u64);

/// Local ENU coordinate in meters relative to the installation origin.
/// Mirrors `scan_zone::SensorPosition` {x,y,z} but in a named frame.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnuPoint {
    pub east_m: f64,
    pub north_m: f64,
    pub up_m: f64,
}

/// A typed world node. Persistence-deterministic serde enum (no trait objects).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorldNode {
    /// A bounded interior space. Linked to HomeCore `area_id` (ADR-127).
    Room {
        id: WorldId,
        /// HomeCore registry area_id; the entity-linkage join key.
        area_id: Option<String>,
        name: String,
        /// ZoneBounds in local ENU meters (reuses MAT ZoneBounds shape).
        bounds_enu: ZoneBoundsEnu,
        floor: i16,
    },
    /// A sub-region of a room targeted for sensing (MAT ScanZone analogue).
    Zone {
        id: WorldId,
        parent_room: WorldId,
        name: String,
        bounds_enu: ZoneBoundsEnu,
    },
    /// A wall segment (coarse topological element, 2D segment in ENU).
    Wall {
        id: WorldId,
        a: EnuPoint,
        b: EnuPoint,
        /// Coarse RF attenuation estimate in dB (drywall ≈ 3, brick ≈ 12).
        rf_attenuation_db: f32,
    },
    /// A passable opening between two rooms.
    Doorway {
        id: WorldId,
        center: EnuPoint,
        width_m: f32,
    },
    /// A physical sensing device placement (ADR-113 placement target).
    Sensor {
        id: WorldId,
        device_id: String,     // matches homecore EntityEntry.device_id
        position: EnuPoint,    // SensorPosition x/y/z analogue
        modality: SensorModality,
    },
    /// A directed RF propagation channel between two sensors (ADR-138 LinkGroup member).
    RfLink {
        id: WorldId,
        tx: WorldId,           // Sensor node
        rx: WorldId,           // Sensor node
        link_group_id: Option<String>, // ADR-138 MLO LinkGroup
        center_freq_mhz: u32,
    },
    /// A tracked person (Kalman track id from ruvsense pose_tracker).
    PersonTrack {
        id: WorldId,
        track_id: u64,
        last_position: EnuPoint,
        reid_embedding_ref: Option<String>, // AETHER re-ID handle
    },
    /// A persistent static reflector / object (ADR-143 RF SLAM anchor; ADR-144 UWB anchor).
    ObjectAnchor {
        id: WorldId,
        position: EnuPoint,
        anchor_kind: AnchorKind,
        confidence: f32,
    },
    /// A discrete detected event (fall, entry, gesture) at a point in time.
    Event {
        id: WorldId,
        event_type: String,
        at_unix_ms: i64,
        located_in: Option<WorldId>, // Room/Zone
    },
    /// A fused semantic belief about the world (the ADR-140 record's graph anchor).
    SemanticState {
        id: WorldId,
        statement: String,        // e.g. "occupant present, seated, room=living_room"
        confidence: f32,
        /// Mandatory provenance per the house rule (see §2.3).
        provenance: SemanticProvenance,
        valid_from_unix_ms: i64,
    },
}

/// MAT ZoneBounds reprojected into the installation ENU frame.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "snake_case")]
pub enum ZoneBoundsEnu {
    Rectangle { min_e: f64, min_n: f64, max_e: f64, max_n: f64 },
    Circle { center_e: f64, center_n: f64, radius_m: f64 },
    Polygon { vertices: Vec<(f64, f64)> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensorModality { WifiCsi, MmWave, Uwb, Presence }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnchorKind { Reflector, Furniture, UwbBeacon }
```

Edges carry **typed metadata per edge kind** — the metadata for `observes` (a sensor's field-of-regard weight) is structurally different from `contradicts` (a disagreement magnitude) or `privacy_limited_by` (the limiting mode + action). Like `petgraph_bridge.rs`'s `BrainEdge`, this is a single enum stored as the petgraph edge weight:

```rust
/// Typed edge between two WorldNodes. Stored as the petgraph edge weight.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "rel", rename_all = "snake_case")]
pub enum WorldEdge {
    /// sensor/rf_link -> any observable node. Weight is field-of-regard quality.
    Observes { quality: f32, last_seen_unix_ms: i64 },
    /// person_track/object_anchor/event -> room/zone containment.
    LocatedIn { since_unix_ms: i64 },
    /// room <-> room through a doorway (undirected pair stored as two edges).
    AdjacentTo { via_doorway: WorldId },
    /// sensor/rf_link -> sensor/rf_link: physical/clock support (ADR-138).
    Supports { strength: f32 },
    /// evidence/state -> evidence/state: sources disagree (ADR-137).
    Contradicts { magnitude: f32, flag: ContradictionFlagRef },
    /// semantic_state -> prior state/evidence: provenance chain (ADR-137).
    DerivedFrom { evidence: EvidenceRefHandle },
    /// sensor -> any node: observation constrained by a privacy mode (ADR-141).
    PrivacyLimitedBy { mode: String, action: String, allowed: bool },
}
```

`EvidenceRefHandle`, `ContradictionFlagRef`, and `SemanticProvenance` are defined in ADR-137 / ADR-140 and re-exported here; this ADR depends on them but does not own them (see §2.3). Where those crates are not yet present, the handles degrade to opaque `String` content-addresses so the WorldGraph compiles and persists independently.

### 2.2 Graph Container and Bridge

Following `petgraph_bridge.rs`, the WorldGraph wraps petgraph and exposes a domain API. We use `StableDiGraph` (not `Graph`) because nodes are removed at runtime (a person leaves, a track dies) and stable indices keep `WorldId → NodeIndex` resolution valid.

```rust
//! v2/crates/wifi-densepose-worldgraph/src/graph.rs

use petgraph::stable_graph::{StableDiGraph, NodeIndex};
use std::collections::HashMap;
use crate::model::{WorldNode, WorldEdge, WorldId};

pub struct WorldGraph {
    inner: StableDiGraph<WorldNode, WorldEdge>,
    /// Stable WorldId -> petgraph handle. Survives removals.
    index: HashMap<WorldId, NodeIndex>,
    /// Installation origin; all ENU coords are relative to this (ADR-044).
    registration: wifi_densepose_geo::types::GeoRegistration,
    next_id: u64,
    schema_version: u16,
}

impl WorldGraph {
    pub fn new(registration: wifi_densepose_geo::types::GeoRegistration) -> Self;

    /// Insert a node, returning its stable WorldId. Allocates the id if the
    /// node's embedded id is WorldId(0) (sentinel = "assign me one").
    pub fn upsert_node(&mut self, node: WorldNode) -> WorldId;

    /// Add a typed edge. Errors if either endpoint is unknown.
    pub fn add_edge(&mut self, from: WorldId, to: WorldId, edge: WorldEdge)
        -> Result<(), WorldGraphError>;

    /// Resolve a HomeCore area_id to its Room node (entity linkage, ADR-127).
    pub fn room_for_area(&self, area_id: &str) -> Option<WorldId>;

    pub fn node(&self, id: WorldId) -> Option<&WorldNode>;
    pub fn neighbors(&self, id: WorldId) -> impl Iterator<Item = (WorldId, &WorldEdge)>;
}
```

A `bridge.rs` module mirrors `petgraph_bridge.rs`'s `to_petgraph` / `from_petgraph` so external algorithm code can borrow a plain `&StableDiGraph` for petgraph's `dijkstra`, `connected_components`, etc., without leaking the domain wrapper.

### 2.3 Provenance: derived_from and contradicts from ADR-137

The house rule is honored structurally: **every `SemanticState` node carries a `SemanticProvenance`** and is reachable along `DerivedFrom` edges back to the evidence that produced it. The provenance tuple binds the four required traces:

```rust
//! Mandatory provenance for every SemanticState (house rule).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SemanticProvenance {
    /// Signal evidence: ADR-137 EvidenceRef content-address(es).
    pub evidence: Vec<EvidenceRefHandle>,
    /// Model version that produced this belief.
    pub model_version: String,
    /// Calibration version (ADR-135 baseline id) in effect.
    pub calibration_version: String,
    /// Privacy decision (ADR-141 mode + action) under which it was derived.
    pub privacy_decision: PrivacyDecisionRef,
}
```

When the fusion engine (ADR-137) emits a new `SemanticState`:

1. `upsert_node()` inserts the new `SemanticState` node.
2. For each `EvidenceRef` in its provenance, the engine adds a `DerivedFrom` edge from the new state to the corresponding `Event` / prior `SemanticState` / `Observes` source.
3. If ADR-137 attached a `ContradictionFlag` (the new belief disagrees with a still-live prior belief), the engine adds a `Contradicts` edge between the two `SemanticState` nodes carrying the flag's magnitude. The prior node is **not deleted** — it is retained so a query can surface the disagreement; a downstream resolver (ADR-140) decides which belief wins.

This makes node updates *append-with-provenance*: the graph never loses the chain of reasoning, which is exactly what ADR-145's ablation harness needs to attribute a wrong belief to a specific sensor/model/calibration.

### 2.4 Privacy: privacy_limited_by edges from ADR-141

For each `(sensor, observable-node)` pair, the WorldGraph materializes a `PrivacyLimitedBy` edge derived from the ADR-141 privacy mode/action registry. The edge records the limiting `mode`, the `action` evaluated, and whether observation is `allowed` under the current mode. This is computed by a reducer that runs whenever the active privacy mode changes:

```rust
/// Recompute privacy_limited_by edges for the active mode (ADR-141).
/// For every Observes edge (sensor -> node), evaluate the mode's policy for
/// that sensor's modality + the node kind, and write/update a matching
/// PrivacyLimitedBy edge.
pub fn apply_privacy_mode(
    &mut self,
    mode: &PrivacyMode,                 // from ADR-141 control plane
) -> PrivacyRollup;

/// Result of a privacy-impact rollup query (§2.5).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrivacyRollup {
    pub mode: String,
    /// Nodes that become unobservable under this mode.
    pub suppressed_nodes: Vec<WorldId>,
    /// (sensor, node) pairs newly denied.
    pub denied_pairs: Vec<(WorldId, WorldId)>,
    pub allowed_pairs: usize,
}
```

Because `PrivacyLimitedBy` is a first-class edge, "what can sensor X still see under mode Y?" is a one-hop neighbor filter — no separate policy index is needed, and the privacy posture is *visible in the persisted graph* (an auditor can read the `.rvf` and see what was suppressed).

### 2.5 Query API Surface (v1 Scope)

The v1 query API is intentionally narrow — three families, all expressible as petgraph traversals over the typed edges:

```rust
//! v2/crates/wifi-densepose-worldgraph/src/query.rs

impl WorldGraph {
    /// OBSERVABILITY CHAIN: sensor -> all nodes it currently observes.
    /// Follows Observes edges (one hop) filtered by current PrivacyLimitedBy.
    pub fn observed_by(&self, sensor: WorldId) -> Vec<ObservedNode>;

    /// LOCATION QUERY: contents of room X.
    /// Reverse LocatedIn traversal: all PersonTrack/ObjectAnchor/Event/Zone
    /// nodes located_in this room (transitively through child Zones).
    pub fn contents_of(&self, room: WorldId) -> RoomContents;

    /// PRIVACY-IMPACT ROLLUP: for a candidate mode, what is suppressed.
    /// Pure (does not mutate); ADR-145 uses it to score privacy leakage.
    pub fn privacy_impact(&self, mode: &PrivacyMode) -> PrivacyRollup;

    /// ADR-144 anchor accessor: sensors + object anchors with known ENU pos.
    pub fn anchors(&self) -> Vec<(WorldId, EnuPoint)>;
}
```

**Scope boundary for v1:** the graph models the **current static topology** of a single installation. Temporal reconfiguration history (rooms repartitioned, sensors relocated over weeks) is **deferred to ADR-142** (Evolution Tracker / temporal VoxelMap). The WorldGraph emits a `TopologyChanged` domain event when static structure changes; ADR-142 subscribes and aggregates the history. This keeps the WorldGraph a clean *current-state* projection and avoids baking a time-series store into the graph itself.

### 2.6 Persistence: RVF Bundle with Async Write-Through

The graph persists as an **RVF bundle**, reusing the segment-based format already implemented in `v2/crates/wifi-densepose-sensing-server/src/rvf_container.rs` (64-byte aligned segments, `SEG_META` for JSON metadata, `SEG_MANIFEST` for the directory, CRC32 content hashes). No new file format is introduced.

- **Layout:** one `SEG_META` segment holds the serde-JSON of `{ registration, schema_version, nodes: Vec<WorldNode>, edges: Vec<(WorldId, WorldId, WorldEdge)> }`. A `SEG_MANIFEST` segment carries node/edge counts and the schema version. A `SEG_WITNESS` segment carries the SHA-256 of the node+edge payload for the ADR-028 proof chain.
- **Async write-through:** mutations (`upsert_node`, `add_edge`, `apply_privacy_mode`) are applied to the in-memory graph synchronously and enqueued to a bounded `tokio::sync::mpsc` channel drained by a single writer task that coalesces bursts and rewrites the `.rvf` (write-temp-then-rename). The hot path never blocks on disk. This mirrors the `homecore/src/registry.rs` "in-memory now, persistence to a backing store later" staging — except the backing store (RVF) is specified up front.
- **Pinning:** the bundle stores its `GeoRegistration` so a reloaded graph re-establishes the same local ENU frame. `enu_to_wgs84()` (ADR-044) regenerates lat/lon for any node on demand for the map overlay.

```rust
//! v2/crates/wifi-densepose-worldgraph/src/persist.rs

pub struct WorldGraphStore {
    path: std::path::PathBuf,
    tx: tokio::sync::mpsc::Sender<WriteOp>,
}

impl WorldGraphStore {
    /// Open or create an RVF-backed store; spawns the write-through task.
    pub async fn open(path: impl Into<std::path::PathBuf>) -> Result<(Self, WorldGraph), WorldGraphError>;

    /// Enqueue a snapshot write (non-blocking, coalesced by the writer task).
    pub fn enqueue_snapshot(&self, graph: &WorldGraph) -> Result<(), WorldGraphError>;

    /// Force-flush and await durability (used at shutdown / before witness).
    pub async fn flush(&self) -> Result<(), WorldGraphError>;
}
```

### 2.7 Error Type and Domain Events

```rust
#[derive(Debug, thiserror::Error)]
pub enum WorldGraphError {
    #[error("unknown node: {0:?}")]
    UnknownNode(WorldId),
    #[error("edge endpoint type mismatch: {0}")]
    EdgeTypeMismatch(String),
    #[error("schema version {found} unsupported (expected {expected})")]
    SchemaMismatch { found: u16, expected: u16 },
    #[error("RVF (de)serialisation error: {0}")]
    Rvf(String),
    #[error("privacy mode references unknown action: {0}")]
    UnknownPrivacyAction(String),
}

/// Event-sourced change notifications (per project DDD rule).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum WorldGraphEvent {
    NodeUpserted(WorldId),
    NodeRemoved(WorldId),
    EdgeAdded { from: WorldId, to: WorldId },
    TopologyChanged,             // consumed by ADR-142
    PrivacyModeApplied(String),  // emitted by apply_privacy_mode
    ContradictionRecorded { a: WorldId, b: WorldId, magnitude: f32 },
}
```

### 2.8 Interface Boundaries

| Boundary | This crate provides | This crate consumes |
|----------|---------------------|---------------------|
| ADR-044 `wifi-densepose-geo` | — | `GeoRegistration`, `GeoPoint`, `wgs84_to_enu`/`enu_to_wgs84` |
| ADR-127 `homecore/registry.rs` | `room_for_area(area_id)` | `EntityEntry.area_id`, `EntityEntry.device_id` (join keys) |
| MAT `scan_zone.rs` | `ZoneBoundsEnu`, `Sensor` node | `ZoneBounds`, `SensorPosition` shapes (reprojected to ENU) |
| ADR-137 fusion | `DerivedFrom`/`Contradicts` edges, `SemanticState` nodes | `EvidenceRef`, `ContradictionFlag` |
| ADR-141 privacy | `apply_privacy_mode`, `privacy_impact` | `PrivacyMode`, action registry |
| ADR-138 LinkGroup | `RfLink.link_group_id` field | LinkGroup ids |
| ADR-142 evolution | `WorldGraphEvent::TopologyChanged` stream | — |
| ADR-144 UWB | `anchors()` accessor | — |
| ADR-145 ablation | `privacy_impact()`, provenance chains | — |

The crate must compile **standalone**: where ADR-137/141 types are not yet present, their handles are `String` content-addresses (feature-gated `full-fusion` swaps them for the real types). This keeps `wifi-densepose-worldgraph` a no-internal-dep leaf on `wifi-densepose-geo` only, matching the publishing-order discipline in CLAUDE.md.

---

## 3. Consequences

### 3.1 Positive

- **One spatial identity space.** `area_id` (HomeCore), `ScanZone` (MAT), and sensor placement (ADR-113) finally resolve to one node set. `room_for_area()` is the single join.
- **Provenance is structural, not bolted on.** Every belief traces to signal evidence + model version + calibration version + privacy decision via `SemanticProvenance` and `DerivedFrom` edges — the house rule is enforced by the type system, not by convention.
- **Privacy posture is auditable.** `PrivacyLimitedBy` edges live in the persisted `.rvf`, so an auditor can read what each mode suppressed without re-running the system.
- **Deterministic persistence.** The serde-enum-over-RVF choice produces byte-stable snapshots suitable for the ADR-028 witness proof chain (SHA-256 of the node/edge payload).
- **Reuses proven mechanics.** The petgraph bridge pattern (`ruv-neural-graph`) and the RVF container (`sensing-server`) are existing, tested code — no new graph engine or file format.
- **Unblocks four downstream ADRs.** ADR-140 (semantic records anchor to `SemanticState` nodes), ADR-142 (consumes `TopologyChanged`), ADR-144 (consumes `anchors()`), ADR-145 (scores over `privacy_impact()` + provenance).

### 3.2 Negative

- **New crate to maintain.** `wifi-densepose-worldgraph` adds a 16th workspace crate and an entry to the publishing order (leaf on `wifi-densepose-geo`).
- **Cross-crate handle coupling.** The full-fidelity provenance/privacy edges depend on ADR-137/141 types. Until those land, the `String`-handle fallback means provenance is content-addressed but not yet richly typed — a temporary loss of compile-time guarantees.
- **Snapshot-rewrite cost.** Async write-through rewrites the whole `.rvf` on flush rather than appending a delta. For a single-installation graph (hundreds of nodes, low-Hz mutation) this is sub-millisecond, but it does not scale to thousands of installations in one file (out of scope — one bundle per installation).
- **No history in v1.** Querying "where was the sofa last month" requires ADR-142; the WorldGraph alone answers only "now."

### 3.3 Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Stale `petgraph` `NodeIndex` after node removal | Medium | Dangling edge / panic | Use `StableDiGraph` (indices survive removal) and the `WorldId → NodeIndex` map; never expose raw `NodeIndex` across the API boundary |
| Schema drift breaks old `.rvf` bundles | Medium | Reload failure | `schema_version` in `SEG_MANIFEST`; `WorldGraphError::SchemaMismatch` with an explicit migration path; refuse-and-warn rather than mis-parse |
| Contradiction edges accumulate without resolution | Medium | Graph bloat, ambiguous beliefs | A retention policy prunes `Contradicts` edges whose losing `SemanticState` has `valid_from` older than a TTL once ADR-140's resolver has chosen a winner |
| Privacy edge recompute lags a fast mode switch | Low | Brief window of stale `allowed` flags | `apply_privacy_mode` runs synchronously on the mutation path before any new `Observes` edge is honored; rollup returned to caller for confirmation |
| ENU origin re-pinned after partial population | Low | Coordinate frame mismatch | Origin is immutable after `WorldGraph::new`; re-pinning requires a new bundle + ADR-142 migration event |

---

## 4. Alternatives Considered

### 4.1 Trait-Object Nodes (`Box<dyn WorldNode>`)

Rejected. `typetag`-style polymorphic serde is non-deterministic across crate/serde versions, cannot be field-walked by ADR-145's harness, and breaks the byte-stable witness proof. The serde enum gives closed-world exhaustiveness (the compiler forces every query to handle every node kind) and deterministic bytes. The `petgraph_bridge.rs` precedent already stores concrete weights, not trait objects.

### 4.2 Extend `GeoScene` with Interior Features

Rejected. `geo::types::GeoScene` is a WGS84 outdoor scene (buildings/roads from OSM). Bolting rooms/sensors/people onto it would (a) conflate the global frame with the local ENU frame, (b) force the geo crate to depend on fusion/privacy types it has no business knowing, and (c) provide no edges. We *reuse* `GeoRegistration` and the ENU transforms from geo, but the WorldGraph is a separate concern.

### 4.3 Reuse `homecore` Area Registry Directly

Rejected as the home. `EntityEntry.area_id` is an opaque string with no geometry and no adjacency; HomeCore's job is HA-compatible entity bookkeeping, not spatial reasoning. The WorldGraph *links to* `area_id` (so automations and sensing share identity) but owns geometry, sensors, and the typed-edge topology HomeCore deliberately does not model.

### 4.4 A Relational/SQLite Store with Join Tables

Rejected for v1. Edges-as-rows + recursive CTEs can express the same queries, but (a) the workspace already standardizes on RVF for portable, witness-hashable artifacts, (b) petgraph gives shortest-path/connectivity algorithms for free (observability chains, adjacency reachability) that would be hand-rolled SQL, and (c) an embedded SQLite file is not byte-stable for the proof chain. RVF + petgraph matches existing patterns; a SQL backend remains a future option behind `WorldGraphStore` if scale demands it.

### 4.5 Temporal Graph from Day One

Rejected for v1. A bitemporal graph (valid-time + transaction-time on every node/edge) is the correct long-term model, but it doubles the schema complexity and the persistence size before any consumer needs history. v1 ships current-state-only and emits `TopologyChanged`; ADR-142 builds the temporal aggregation on top. This keeps the first deliverable small and the query API simple.

---

## 5. Testing / Acceptance

### 5.1 Unit Tests (CI, no hardware)

**T1 — Node/edge round-trip determinism.** Build a graph with one of every `WorldNode` variant and one of every `WorldEdge` variant. Serialize to RVF bytes, deserialize, assert structural equality and assert the SHA-256 of the node/edge payload is byte-stable across two independent serializations (deterministic-persistence acceptance).

**T2 — `room_for_area` entity linkage.** Insert a `Room { area_id: Some("living_room") }`; assert `room_for_area("living_room")` returns its `WorldId` and `room_for_area("garage")` returns `None`. Mirrors the HomeCore `registry.rs` register-and-read test.

**T3 — ENU pinning round-trip.** Pin a graph to `GeoRegistration { origin: lat/lon }`; place a `Sensor` at a known `EnuPoint`; reproject to WGS84 via `enu_to_wgs84` and back via `wgs84_to_enu`; assert agreement within 1e-6 m (validates the ADR-044 frame reuse).

**T4 — Observability chain.** Sensor S observes nodes A,B,C (three `Observes` edges); assert `observed_by(S)` returns exactly {A,B,C}.

**T5 — Location query (transitive).** Room R contains Zone Z; PersonTrack P `located_in` Z. Assert `contents_of(R)` includes P (transitive through the child zone) and Object/Event nodes located directly in R.

**T6 — Provenance chain (house rule).** Insert a `SemanticState` with `SemanticProvenance { evidence, model_version, calibration_version, privacy_decision }` and `DerivedFrom` edges to two `Event` sources. Assert every `SemanticState` in the graph has non-empty `evidence`, a `model_version`, a `calibration_version`, and a `privacy_decision` (acceptance: the four-fold trace is present on every belief node).

**T7 — Contradiction retention.** Insert belief B1, then a contradicting belief B2 (ADR-137 `ContradictionFlag`). Assert a `Contradicts` edge exists, B1 is **not** removed, and a `WorldGraphEvent::ContradictionRecorded` was emitted.

**T8 — Privacy-impact rollup.** With sensor S observing person P, apply a `PrivacyMode` that denies person observation for S's modality. Assert `privacy_impact(mode).suppressed_nodes` contains P, a `PrivacyLimitedBy { allowed: false }` edge is written, and `observed_by(S)` no longer returns P.

**T9 — Schema-mismatch refusal.** Hand-craft an RVF `SEG_MANIFEST` with `schema_version = 999`; assert `open()` returns `WorldGraphError::SchemaMismatch` (refuse, do not mis-parse).

**T10 — Stable index after removal.** Insert 5 nodes, remove the middle one, add a 6th; assert all surviving `WorldId → WorldNode` lookups still resolve and no edge dangles (validates `StableDiGraph` choice).

### 5.2 Async Persistence Test

**T11 — Write-through coalescing.** Open a `WorldGraphStore`, enqueue 1,000 rapid snapshots, `flush()`, reopen the bundle, assert the final state matches the last snapshot and that the writer task coalesced (write count < enqueue count). Hot-path `enqueue_snapshot` must not block (assert it returns within a tight bound while the disk write is in flight).

### 5.3 Witness / Proof (ADR-028 chain)

Add rows to `docs/WITNESS-LOG-028.md`:

| Row | Capability | Evidence | Hash |
|-----|-----------|----------|------|
| W-39 | WorldGraph RVF round-trip determinism | `cargo test worldgraph::tests::roundtrip_determinism` | SHA-256 of node/edge payload |
| W-40 | Provenance four-fold trace present on every SemanticState | `cargo test worldgraph::tests::provenance_complete` | SHA-256 of test binary |
| W-41 | Privacy rollup suppresses denied nodes | `cargo test worldgraph::tests::privacy_rollup` | SHA-256 of rollup output |

`source-hashes.txt` in the witness bundle gains `SHA-256(worldgraph/model.rs)` and `SHA-256(worldgraph/graph.rs)`.

### 5.4 Acceptance Criteria (Definition of Done)

1. `wifi-densepose-worldgraph` compiles standalone (`cargo check -p wifi-densepose-worldgraph --no-default-features`) depending only on `wifi-densepose-geo` + `petgraph` + `serde`.
2. T1–T11 pass in `cargo test --workspace --no-default-features`; total workspace test count rises and stays at 0 failures.
3. Every `SemanticState` node carries the four-fold provenance trace (signal evidence + model version + calibration version + privacy decision) — enforced by T6 and by the non-`Option` `SemanticProvenance` field.
4. A persisted `.rvf` bundle reloads to a structurally identical graph and re-establishes the same ENU origin.
5. The three query families (observability chain, location, privacy rollup) each have a passing test and a documented signature in `query.rs`.
6. v1 explicitly does **not** store reconfiguration history; a `TopologyChanged` event is emitted for ADR-142 to consume (verified by a unit test asserting the event fires on a wall/room change).

---

## 6. Related ADRs

| ADR | Relationship |
|-----|-------------|
| ADR-044 (Geospatial Satellite Integration) | **Substrate**: reuses `GeoRegistration`, `GeoPoint`, and `wgs84_to_enu`/`enu_to_wgs84` to pin the local ENU frame |
| ADR-113 (Multistatic Placement Strategy) | **Source**: sensor placements become `Sensor` nodes; placement geometry feeds `position` |
| ADR-127 (HomeCore State Machine) | **Linkage**: `EntityEntry.area_id`/`device_id` join to `Room`/`Sensor` nodes via `room_for_area()` |
| ADR-030 (Persistent Field Model) | **Adjacent**: the field model is a per-link signal model; WorldGraph is the spatial/semantic model that field-model events annotate |
| ADR-136 (RuView Streaming Engine) | **Upstream**: frames flow through the streaming engine before fusion populates the WorldGraph |
| ADR-137 (Fusion Quality Scoring) | **Source of provenance**: `EvidenceRef`/`ContradictionFlag` populate `DerivedFrom`/`Contradicts` edges |
| ADR-138 (LinkGroup / ArrayCoordinator) | **Source**: `RfLink.link_group_id` references MLO LinkGroups; `Supports` edges encode clock/physical support |
| ADR-142 (Evolution Tracker) | **Consumer**: subscribes to `TopologyChanged`; owns the deferred temporal history |
| ADR-144 (UWB Range-Constraint Fusion) | **Consumer**: reads `anchors()` (sensors + object anchors) as the range-constraint anchor set |
| ADR-145 (Ablation Eval Harness) | **Consumer**: scores privacy leakage via `privacy_impact()` and attributes errors via provenance chains |

---

## 7. References

### Production Code

- `v2/crates/ruv-neural/ruv-neural-graph/src/petgraph_bridge.rs` — petgraph bridge pattern (`to_petgraph`/`from_petgraph`, typed domain edges) this crate follows
- `v2/crates/wifi-densepose-geo/src/types.rs` — `GeoPoint`, `GeoBBox`, `GeoRegistration`, `GeoScene` (ADR-044 anchor types reused)
- `v2/crates/wifi-densepose-geo/src/coord.rs` — `wgs84_to_enu`/`enu_to_wgs84` (local ENU frame transforms)
- `v2/crates/homecore/src/registry.rs` — `EntityEntry { area_id, device_id }`, in-memory-then-persist staging mirrored by `WorldGraphStore`
- `v2/crates/wifi-densepose-mat/src/domain/scan_zone.rs` — `ZoneBounds`, `SensorPosition`, `contains_point()` shapes reprojected into `ZoneBoundsEnu` / `Sensor`
- `v2/crates/wifi-densepose-sensing-server/src/rvf_container.rs` — RVF segment format (64-byte headers, `SEG_META`/`SEG_MANIFEST`/`SEG_WITNESS`, CRC32) reused for persistence
- `v2/crates/wifi-densepose-geo/src/temporal.rs` — precedent for change tracking that ADR-142 generalizes

### External

- petgraph crate — `StableDiGraph`, `dijkstra`, `connected_components` traversal algorithms used by the query API
- Mardia, K.V. & Jupp, P.E. (2000). *Directional Statistics*. Wiley — circular geometry for ENU/heading consistency (shared with ADR-135 calibration phase model)


---

## Implementation Status & Integration (2026-05-29)
*Part of the ADR-136 streaming-engine series -- skeleton/scaffolding, trust-first, mostly not yet on the live 20 Hz path. See ADR-136 (Implementation Status) for the series framing.*

**Built -- tested building block** (commit `521a012d8`, issue #843): the new `wifi-densepose-worldgraph` crate -- typed petgraph nodes/edges, provenance (`DerivedFrom`) and disagreement (`Contradicts`) edges, the privacy rollup, and deterministic JSON persistence. 7 tests.

**Integration glue -- not yet on the live path:** feeding live fusion outputs and person tracks into nodes; the full `.rvf` bundle container (today it persists as JSON); and the live ADR-141 privacy-mode reducer.

**Trust contribution:** the auditable map -- evidence and contradiction are first-class edges, and the privacy posture is *visible in the persisted graph* (an auditor can read what was suppressed).


## 2026-11-04 amendment: managed configured topology transport

Status: implemented source slice; native delivery and physical geometry acceptance remain pending.

The app-managed sensing server exposes authenticated `GET /api/v1/worldgraph/snapshot`
as protocol 2 when its authoritative companion room store is available. Protocol 1
and genuine registered ENU WorldGraph snapshots remain a separate unchanged contract.
Protocol 2 is `ruview.worldgraph.managed-topology.v1`, with `managed_room_local`
coordinates and `x_right_y_down_metres` axes (room map origin at top left). Its only
geometry provenance is `operator_entered`; geographic registration and sensor
heights are explicitly null. Room dimensions may include the operator-entered
room height, but this never supplies sensor height or AR alignment.

The projection takes one immutable published room envelope, preserving installation,
revision and binding digest. Canonical room/sensor and located-in vocabulary is
reused without injecting the engine's default room, person/semantic/vital records,
raw CSI or phone geometry. The configured AP remains part of the original room
binding but is not invented as a CSI receiver or RF link. This slice is not an ENU
registration, a spatial teacher, localization, occupancy model or model-selection
receipt. It requires no additional persistent graph store.

An ephemeral random `wg-` epoch and safe-integer cursor belong to the loaded account/
installation store. Equal canonical topology uses the same sequence; readiness,
binding or privacy changes advance it. Reads never publish geometry, renew a
baseline, train or select a model. Global server privacy mode conservatively returns
`privacy_denied` with no geometry. No published room yields `not_configured`;
missing managed support remains unsupported, not zero people. The same protected
owner and paired read paths apply, with `Cache-Control: private, no-store`.

The managed client shares capability state between Zones, Settings and MAT, and
never converts this planar topology to a fabricated geographic or phone AR frame.
Cloud Cognitum Spaces/Zone projection remains separate under ADR-325.
