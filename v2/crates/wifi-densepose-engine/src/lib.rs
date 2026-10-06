//! # RuView Streaming Engine — integration layer
//!
//! This crate is the **composition root** that wires the ADR-135..146 building
//! blocks into one end-to-end *trust-traceable* pipeline cycle. Each block was
//! built and unit-tested independently; this crate proves they compose and that
//! the **trust throughline** holds end-to-end:
//!
//! > *Why believe the system when it says a person is present?* — every
//! > [`TrustedOutput`] names its **signal evidence** (ADR-137 `EvidenceRef`),
//! > its **model version** (ADR-136), its **calibration version** (ADR-135
//! > baseline id, ADR-136 `calibration_id`), and the **privacy decision**
//! > (ADR-141 mode → class) it was emitted under — and is anchored as a
//! > provenance-bearing node in the ADR-139 WorldGraph.
//!
//! One [`StreamingEngine::process_cycle`] performs, in order:
//! 1. **Fuse + score** the node frames (ADR-137 `fuse_scored`) → `QualityScore`
//!    with per-node weights, evidence, and tolerated contradiction flags.
//! 2. **Stamp calibration provenance** (ADR-135/136): the `CalibrationId` the
//!    calibration stage applied is recorded on the `QualityScore`.
//! 3. **Privacy control plane** (ADR-141): if the fusion recorded a tolerated
//!    contradiction, the active privacy class is **demoted one step** before
//!    emission (monotonic — information only ever removed).
//! 4. **Semantic state** (ADR-139/140): a `SemanticState` node is appended to
//!    the WorldGraph with mandatory provenance and a `DerivedFrom` edge to the
//!    room it was observed in.
//!
//! What is intentionally *not* here: the live 20 Hz I/O loop (sensing-server),
//! UWB hardware (ADR-144), and model training (ADR-146). This is the
//! composition + validation layer those will plug into.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use wifi_densepose_bfld::{PrivacyAction, PrivacyClass, PrivacyMode, PrivacyModeRegistry};
use wifi_densepose_geo::types::GeoRegistration;
use wifi_densepose_signal::ruvsense::fusion_quality::CalibrationId;
use wifi_densepose_signal::ruvsense::multistatic::{MultistaticConfig, MultistaticFuser, PhaseFusion};
use wifi_densepose_signal::ruvsense::{
    ArrayCoordinator, ArrayCoordinatorConfig, ArrayNodeInput, ChangePoint, DirectionalEvidence,
    EvolutionTracker, MultiBandCsiFrame, QualityScore, ReflectorObservation, RfSlam,
};
use wifi_densepose_worldgraph::{
    AnchorKind, EnuPoint, PrivacyRollup, SemanticProvenance, WorldEdge, WorldGraph, WorldGraphError,
    WorldId, WorldNode, ZoneBoundsEnu,
};

pub mod mesh_guard;
pub use mesh_guard::{MeshGuard, MeshPartitionReport};
pub use wifi_densepose_ruvector::viewpoint::coherence::ClockQualityScore;

/// Clock score for a node with no sync evidence (issue #2156). The ADR-138
/// gate rejects it as `ClockInvalid`, so the node adds no directional evidence.
pub const UNKNOWN_CLOCK: ClockQualityScore =
    ClockQualityScore { offset_stdev_us: f32::INFINITY, age_us: u64::MAX, valid: false };

/// Errors from an engine cycle.
#[derive(Debug)]
pub enum EngineError {
    /// Multistatic fusion failed (no frames, timestamp spread, dimension mismatch).
    Fusion(wifi_densepose_signal::ruvsense::multistatic::MultistaticError),
}

impl core::fmt::Display for EngineError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            EngineError::Fusion(e) => write!(f, "fusion error: {e}"),
        }
    }
}
impl std::error::Error for EngineError {}
impl From<wifi_densepose_signal::ruvsense::multistatic::MultistaticError> for EngineError {
    fn from(e: wifi_densepose_signal::ruvsense::multistatic::MultistaticError) -> Self {
        EngineError::Fusion(e)
    }
}

/// Geometry of a sensing node, needed to run the ADR-138 array coordinator.
#[derive(Debug, Clone, Copy)]
struct NodeGeom {
    x: f32,
    y: f32,
    azimuth: f32,
}

/// The auditable result of one engine cycle — the trust chain made concrete.
#[derive(Debug, Clone)]
pub struct TrustedOutput {
    /// The `SemanticState` node id created in the WorldGraph.
    pub semantic_id: WorldId,
    /// The fusion quality record (evidence + contradictions + calibration).
    pub quality: QualityScore,
    /// The privacy class the output was emitted under (after any demotion).
    pub effective_class: PrivacyClass,
    /// Whether a tolerated contradiction forced a privacy demotion this cycle.
    pub demoted: bool,
    /// The mandatory provenance attached to the semantic node.
    pub provenance: SemanticProvenance,
    /// ADR-138 directional evidence, when node geometry is registered for every
    /// contributing node (else `None`).
    pub directional: Option<DirectionalEvidence>,
    /// ADR-142 cross-link change-point detected this cycle, if any (and the
    /// `Event` node it was recorded as in the WorldGraph).
    pub change_point: Option<(ChangePoint, WorldId)>,
    /// BLAKE3 witness over the trust decision (provenance ‖ class ‖ calibration)
    /// — a deterministic, signed-belief fingerprint (ADR-137 §2.7 / ADR-028).
    pub witness: [u8; 32],
    /// Whether the drift→recalibration advisor recommends re-running the
    /// ADR-135 baseline / refitting the per-room adapter (ADR-150 §3.4):
    /// sustained low coherence or an ADR-142 change-point this cycle.
    pub recalibration_recommended: bool,
    /// Dynamic min-cut partition report over the live mesh coupling graph
    /// (None for meshes of fewer than two nodes). `at_risk` counts as a
    /// structural event for the recalibration advisor and names the nodes
    /// (`weak_side`) closest to splitting off — failure/jamming triage.
    pub mesh: Option<MeshPartitionReport>,
    /// Whether phase was combined across nodes this cycle, and why not when it
    /// was not (issue #1752). Amplitude-only sources report
    /// `AmplitudeOnly(PhaseUnavailable)`.
    pub phase_fusion: PhaseFusion,
}

/// Composition root for the RuView streaming engine.
pub struct StreamingEngine {
    fuser: MultistaticFuser,
    coherence_accept: f32,
    privacy: PrivacyModeRegistry,
    world: WorldGraph,
    model_version: u16,
    cycle: u64,
    // ADR-138: array coordinator + per-node geometry (by frame node_id).
    array: ArrayCoordinator,
    node_geom: BTreeMap<u8, NodeGeom>,
    // ADR-138: per-node clock quality from ADR-110 sync state (#2156). Nodes
    // without an entry are scored `UNKNOWN_CLOCK`.
    node_clock: BTreeMap<u8, ClockQualityScore>,
    // ADR-142: per-link evolution tracker (sized lazily to the node count).
    evolution: Option<EvolutionTracker>,
    // ADR-143: persistent reflector discovery (v2 mode).
    slam: RfSlam,
    // ADR-139 live loop: stable track_id -> PersonTrack WorldId.
    person_tracks: BTreeMap<u64, WorldId>,
    // WorldGraph belief retention: max live SemanticState nodes. The live loop
    // appends one belief per cycle (1.7M/day at 20 Hz); durable history is the
    // recorder's job, so old beliefs are evicted deterministically past this cap.
    semantic_retention: usize,
    // Per-room calibration adapter (ADR-150 §3.4: ~11 KB LoRA on a frozen
    // base). Identity is part of the trust chain: when set, the adapter id is
    // appended to the provenance model_version, so swapping adapters changes
    // the witness. None = shared base model.
    adapter: Option<AdapterInfo>,
    // Drift→recalibration advisor (ADR-135 trigger for ADR-150 §3.4 refit).
    recal: RecalibrationAdvisor,
    // Dynamic min-cut mesh partition guard (incremental, change-gated).
    mesh: MeshGuard,
}

/// Identity of an active per-room calibration adapter (ADR-150 §3.4). The id
/// must be content-derived (e.g. a hash prefix of the adapter file) so the
/// provenance/witness chain pins the exact weights that shaped inference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterInfo {
    /// Content-derived adapter identity (e.g. first 16 hex of its SHA-256).
    pub adapter_id: String,
    /// Number of in-room samples the adapter was fitted on (0 if unknown).
    pub trained_samples: u32,
}

/// Recommends re-running calibration / adapter refit when the live signal
/// degrades persistently (ADR-135 drift → ADR-150 §3.4 few-shot recalibration).
///
/// Two triggers, both cheap and deterministic:
/// - `low_coherence_streak`: N consecutive cycles whose base coherence fell
///   below the floor (sustained degradation, not a single bad frame);
/// - any ADR-142 change-point this cycle (the environment itself changed).
#[derive(Debug, Clone)]
pub struct RecalibrationAdvisor {
    /// Coherence below this counts toward the streak.
    pub coherence_floor: f32,
    /// Consecutive low-coherence cycles required to recommend recalibration.
    pub streak_threshold: u32,
    streak: u32,
}

impl Default for RecalibrationAdvisor {
    fn default() -> Self {
        Self {
            coherence_floor: 0.5,
            streak_threshold: 60, // ~3 s at 20 Hz of sustained degradation
            streak: 0,
        }
    }
}

impl RecalibrationAdvisor {
    /// Feed one cycle's evidence; returns whether recalibration is recommended.
    fn observe(&mut self, base_coherence: f32, change_point: bool) -> bool {
        if base_coherence < self.coherence_floor {
            self.streak = self.streak.saturating_add(1);
        } else {
            self.streak = 0;
        }
        change_point || self.streak >= self.streak_threshold
    }

    /// Current consecutive low-coherence cycle count.
    #[must_use]
    pub fn streak(&self) -> u32 {
        self.streak
    }
}

impl StreamingEngine {
    /// Build an engine with a starting privacy mode and model version. The
    /// WorldGraph is registered to the installation origin.
    #[must_use]
    pub fn new(mode: PrivacyMode, model_version: u16, registration: GeoRegistration) -> Self {
        Self {
            fuser: MultistaticFuser::with_config(MultistaticConfig::default()),
            coherence_accept: Self::DEFAULT_COHERENCE_ACCEPT,
            privacy: PrivacyModeRegistry::new(mode),
            world: WorldGraph::new(registration),
            model_version,
            cycle: 0,
            array: ArrayCoordinator::new(ArrayCoordinatorConfig::default()),
            node_geom: BTreeMap::new(),
            node_clock: BTreeMap::new(),
            evolution: None,
            slam: RfSlam::with_discovery(
                Self::SLAM_ASSOC_RADIUS_M,
                Self::SLAM_MIN_SIGHTINGS,
                Self::SLAM_MIN_COHERENCE,
            ),
            person_tracks: BTreeMap::new(),
            semantic_retention: Self::DEFAULT_SEMANTIC_RETENTION,
            adapter: None,
            recal: RecalibrationAdvisor::default(),
            mesh: MeshGuard::default(),
        }
    }

    /// Override the multistatic fuser's timestamp guard interval (#1049/#1057).
    /// Without this, `StreamingEngine::new` always builds
    /// `MultistaticFuser::with_config(MultistaticConfig::default())` — a
    /// hardcoded 60 ms hard guard that ignores whatever schedule/override the
    /// caller derived from `WDP_TDM_SLOTS`/`WDP_GUARD_INTERVAL_US`, so
    /// WiFi/ESP-NOW-synced multi-node deployments spuriously fail governed
    /// trust cycles even after widening the guard elsewhere.
    ///
    /// Rebuilds the fuser, so call before any frames are processed. Preserves
    /// any node positions configured via [`Self::set_node_positions`] across
    /// the rebuild (issue #1870) — a caller that configures positions once at
    /// startup and calls this later (e.g. after deriving a TDM schedule) must
    /// not have that configuration silently discarded.
    pub fn set_multistatic_config(&mut self, cfg: MultistaticConfig) {
        let positions = self.fuser.node_positions_by_id().clone();
        self.fuser = MultistaticFuser::with_config(cfg);
        self.fuser.set_node_positions_by_id(positions);
    }

    /// Configure node positions for the governed multistatic fuser, keyed by
    /// node id (issue #1870). Without this, `StreamingEngine`'s fuser — the
    /// one behind `fuse_scored_calibrated`, which feeds provenance and the
    /// witness — always fuses with every node at the origin, regardless of
    /// deployment configuration, because nothing ever called this on it. This
    /// is a separate `MultistaticFuser` instance from the one
    /// `wifi-densepose-sensing-server`'s `AppState` owns (fixed for that
    /// instance in #1860); each fuser instance needs positions set on it
    /// independently. Safe to call before or after [`Self::set_multistatic_config`] —
    /// order does not matter, since that method now preserves whatever
    /// positions are already set across its rebuild.
    pub fn set_node_positions(&mut self, positions: std::collections::HashMap<u8, [f32; 3]>) {
        self.fuser.set_node_positions_by_id(positions);
    }

    /// Node positions the governed fuser currently holds, keyed by node id.
    /// Empty until [`Self::set_node_positions`] is called, in which case every
    /// node fuses at the origin.
    #[must_use]
    pub fn node_positions(&self) -> &std::collections::HashMap<u8, [f32; 3]> {
        self.fuser.node_positions_by_id()
    }

    /// Activate a per-room calibration adapter (ADR-150 §3.4). From the next
    /// cycle on, the adapter id is part of provenance `model_version` — and
    /// therefore of the witness — so the exact weights shaping inference are
    /// pinned in the trust chain. Pass the result of hashing the adapter file.
    pub fn set_room_adapter(&mut self, info: AdapterInfo) {
        self.adapter = Some(info);
    }

    /// Deactivate the adapter (revert to the shared base model).
    pub fn clear_room_adapter(&mut self) {
        self.adapter = None;
    }

    /// The active adapter, if any.
    #[must_use]
    pub fn room_adapter(&self) -> Option<&AdapterInfo> {
        self.adapter.as_ref()
    }

    /// Tune the drift→recalibration advisor (floor + streak threshold).
    pub fn set_recalibration_advisor(&mut self, advisor: RecalibrationAdvisor) {
        self.recal = advisor;
    }

    /// Mutable access to the mesh partition guard (risk threshold, quantum,
    /// min-node count). Operators tune the partition-risk sensitivity here.
    pub fn mesh_guard_mut(&mut self) -> &mut MeshGuard {
        &mut self.mesh
    }

    /// Default cap on live `SemanticState` beliefs in the WorldGraph
    /// (~6 minutes of full-rate history at 20 Hz; older beliefs are evicted —
    /// durable history belongs to the recorder).
    pub const DEFAULT_SEMANTIC_RETENTION: usize = 7_200;

    /// Cross-node coherence at or above which fusion records a positive
    /// `CoherenceGateThreshold` evidence ref (ADR-137). Below it the cycle still
    /// emits, but without that corroborating evidence — so this gate shapes the
    /// trust record, not the privacy class. (== prior inline 0.85.)
    pub const DEFAULT_COHERENCE_ACCEPT: f32 = 0.85;

    /// ADR-143 reflector-discovery parameters used to build the persistent
    /// `RfSlam`: association radius (m) within which two sightings are the same
    /// reflector, the minimum number of sightings before a reflector is
    /// considered stable, and the minimum per-sighting coherence to admit it.
    /// (== prior inline `with_discovery(0.5, 5, 0.6)`.)
    pub const SLAM_ASSOC_RADIUS_M: f64 = 0.5;
    /// Minimum sightings before a discovered reflector is treated as stable.
    pub const SLAM_MIN_SIGHTINGS: u64 = 5;
    /// Minimum per-sighting coherence to admit a reflector sighting.
    pub const SLAM_MIN_COHERENCE: f32 = 0.6;

    /// ADR-143 static-anchor classification thresholds passed to
    /// `RfSlam::static_anchors`: the wall/ceiling stationarity ceiling and the
    /// mobile-reflector floor (anchors more mobile than this are dropped, not
    /// persisted). (== prior inline `static_anchors(0.05, 1.0)`.)
    pub const ANCHOR_WALL_CEILING: f64 = 0.05;
    /// Mobility floor above which a reflector is treated as mobile (skipped).
    pub const ANCHOR_MOBILE_FLOOR: f64 = 1.0;

    /// Override the `SemanticState` retention cap (minimum 1).
    pub fn set_semantic_retention(&mut self, max_states: usize) {
        self.semantic_retention = max_states.max(1);
    }

    /// ADR-139 live loop: create or update a `PersonTrack` node by stable
    /// `track_id`, locate it in `room`, and wire an `Observes` edge from
    /// `sensor` (so the privacy rollup can suppress it under identity-strict
    /// modes). Returns the (stable) WorldGraph id.
    pub fn update_person_track(
        &mut self,
        track_id: u64,
        x: f32,
        y: f32,
        room: WorldId,
        sensor: WorldId,
    ) -> WorldId {
        let existing = self.person_tracks.get(&track_id).copied();
        let node = WorldNode::PersonTrack {
            id: existing.unwrap_or(WorldId::UNASSIGNED),
            track_id,
            last_position: EnuPoint { east_m: f64::from(x), north_m: f64::from(y), up_m: 0.0 },
            reid_embedding_ref: None,
        };
        let id = self.world.upsert_node(node);
        if existing.is_none() {
            self.person_tracks.insert(track_id, id);
            let _ = self.world.add_edge(id, room, WorldEdge::LocatedIn { since_unix_ms: 0 });
            let _ = self.world.add_edge(
                sensor,
                id,
                WorldEdge::Observes { quality: 1.0, last_seen_unix_ms: 0 },
            );
        }
        id
    }

    /// ADR-139 §2.4 / ADR-141: materialise `PrivacyLimitedBy` edges for the
    /// active privacy mode. Under an identity-suppressing mode, `person_track`
    /// observations are denied; the rollup names what was suppressed.
    pub fn apply_active_privacy_mode(&mut self) -> PrivacyRollup {
        let mode = self.privacy.active_mode();
        let suppress_identity = self.privacy.is_action_enforced(PrivacyAction::SuppressIdentity);
        self.world.apply_privacy_mode(
            &format!("{mode:?}"),
            "SuppressIdentity",
            move |_sensor_kind, node_kind| !(suppress_identity && node_kind == "person_track"),
        )
    }

    /// Persist the WorldGraph as deterministic JSON (the RVF payload). Contains
    /// only graph nodes/edges — **never** raw RF frames.
    ///
    /// # Errors
    /// [`WorldGraphError`] on serialisation failure.
    pub fn snapshot_json(&self) -> Result<Vec<u8>, WorldGraphError> {
        self.world.to_json()
    }

    /// Register a contributing node's geometry (ADR-138). When every frame's
    /// `node_id` in a cycle has a registered geometry, the cycle runs the array
    /// coordinator and folds its contradictions into the privacy decision.
    pub fn register_node_geometry(&mut self, node_id: u8, x: f32, y: f32, azimuth: f32) {
        self.node_geom.insert(node_id, NodeGeom { x, y, azimuth });
    }

    /// Set a node's clock-sync quality for the ADR-138 gate (#2156). Callers
    /// derive it from the node's ADR-110 sync packets; a node never set here
    /// is scored [`UNKNOWN_CLOCK`] and rejected by the gate.
    pub fn set_node_clock(&mut self, node_id: u8, clock: ClockQualityScore) {
        self.node_clock.insert(node_id, clock);
    }

    /// Ingest CIR-derived reflector sightings (ADR-143) and persist any newly
    /// stable static anchors into the WorldGraph as `ObjectAnchor` nodes.
    /// Returns the WorldGraph ids written this call.
    pub fn ingest_reflectors(&mut self, observations: &[ReflectorObservation]) -> Vec<WorldId> {
        for obs in observations {
            self.slam.observe(obs);
        }
        let mut written = Vec::new();
        for (pos, class) in
            self.slam.static_anchors(Self::ANCHOR_WALL_CEILING, Self::ANCHOR_MOBILE_FLOOR)
        {
            let kind = match class {
                wifi_densepose_signal::ruvsense::ReflectorClass::Wall => AnchorKind::Reflector,
                wifi_densepose_signal::ruvsense::ReflectorClass::Furniture => AnchorKind::Furniture,
                wifi_densepose_signal::ruvsense::ReflectorClass::Mobile => continue,
            };
            let id = self.world.upsert_node(WorldNode::ObjectAnchor {
                id: WorldId::UNASSIGNED,
                position: EnuPoint { east_m: pos[0], north_m: pos[1], up_m: pos[2] },
                anchor_kind: kind,
                confidence: 0.9,
            });
            written.push(id);
        }
        written
    }

    /// Register a room and return its WorldGraph id (the observation scope).
    pub fn add_room(&mut self, area_id: &str, name: &str) -> WorldId {
        self.world.upsert_node(WorldNode::Room {
            id: WorldId::UNASSIGNED,
            area_id: Some(area_id.to_string()),
            name: name.to_string(),
            bounds_enu: ZoneBoundsEnu::Rectangle { min_e: 0.0, min_n: 0.0, max_e: 5.0, max_n: 4.0 },
            floor: 0,
        })
    }

    /// Register a sensor node and an `observes` edge to a room.
    pub fn add_sensor(&mut self, device_id: &str, room: WorldId) -> WorldId {
        let id = self.world.upsert_node(WorldNode::Sensor {
            id: WorldId::UNASSIGNED,
            device_id: device_id.to_string(),
            position: EnuPoint { east_m: 0.0, north_m: 0.0, up_m: 0.0 },
            modality: wifi_densepose_worldgraph::SensorModality::WifiCsi,
        });
        let _ = self.world.add_edge(
            id,
            room,
            WorldEdge::Observes { quality: 1.0, last_seen_unix_ms: 0 },
        );
        id
    }

    /// Switch the active privacy mode (records a hash-chained attestation).
    pub fn set_privacy_mode(&mut self, mode: PrivacyMode) {
        self.privacy.set_mode(mode);
    }

    /// Borrow the WorldGraph (for queries / persistence).
    #[must_use]
    pub fn world(&self) -> &WorldGraph {
        &self.world
    }

    /// Borrow the privacy registry (for attestation audit).
    #[must_use]
    pub fn privacy(&self) -> &PrivacyModeRegistry {
        &self.privacy
    }

    /// Cycles processed so far.
    #[must_use]
    pub fn cycle_count(&self) -> u64 {
        self.cycle
    }

    /// Run one full trust-traceable cycle (see crate docs for the steps).
    ///
    /// `calibration` is the [`CalibrationId`] the calibration stage applied to
    /// these frames (ADR-135 `BaselineCalibration::calibration_id()`); `room` is
    /// the observation scope (an existing WorldGraph Room id).
    ///
    /// # Errors
    /// [`EngineError::Fusion`] if multistatic fusion rejects the input.
    pub fn process_cycle(
        &mut self,
        node_frames: &[MultiBandCsiFrame],
        calibration: CalibrationId,
        room: WorldId,
        now_ms: i64,
    ) -> Result<TrustedOutput, EngineError> {
        // Uniform-calibration convenience: every node shares one epoch.
        let cals = vec![Some(calibration); node_frames.len()];
        self.process_cycle_calibrated(node_frames, &cals, room, now_ms)
    }

    /// Like [`Self::process_cycle`] but with a **per-node** calibration epoch
    /// (ADR-137 §2.3). If the nodes' calibrations disagree, fusion raises a
    /// `CalibrationIdMismatch`, the score's `calibration_id` is `None`, and the
    /// privacy class is demoted — proving the calibration → trust → privacy path.
    ///
    /// # Errors
    /// [`EngineError::Fusion`] if multistatic fusion rejects the input.
    pub fn process_cycle_calibrated(
        &mut self,
        node_frames: &[MultiBandCsiFrame],
        calibrations: &[Option<CalibrationId>],
        room: WorldId,
        now_ms: i64,
    ) -> Result<TrustedOutput, EngineError> {
        // 1. Array coordination (ADR-138) — only when geometry is known for
        //    every contributing node. Its contradictions feed the privacy gate.
        let directional = self.coordinate_array(node_frames);
        let array_contradiction =
            directional.as_ref().is_some_and(|d| !d.contradictions.is_empty());

        // 2. Fuse + score with per-node calibration (ADR-137 §2.3).
        let (fused, quality) =
            self.fuser.fuse_scored_calibrated(node_frames, calibrations, self.coherence_accept)?;

        // 4. Evolution change-point (ADR-142) over per-node mean amplitude.
        let change_point = self.track_evolution(node_frames, now_ms, room);

        // 5. Mesh partition guard (ADR-032): dynamic min-cut over the coupling
        //    graph. Coupling between nodes i and j is the product of their
        //    fusion attention weights scaled by the node count, so a node the
        //    fuser down-weights is exactly a node weakly coupled in the graph.
        //    (Change-gated incremental updates: steady state touches 0 edges.)
        let node_ids: Vec<u8> = node_frames.iter().map(|f| f.node_id).collect();
        let weights = &quality.per_node_weights;
        let n = weights.len() as f64;
        let mesh = self.mesh.update(&node_ids, |i, j| {
            let wi = weights.get(i).copied().unwrap_or(0.0) as f64;
            let wj = weights.get(j).copied().unwrap_or(0.0) as f64;
            wi * wj * n
        });
        let mesh_at_risk = mesh.as_ref().is_some_and(|m| m.at_risk);

        // 6. Privacy control plane (ADR-141): demote on a fusion-level OR an
        //    array-level contradiction OR a mesh close to partitioning. The
        //    last is a security/reliability signal (ADR-032): a fragmenting
        //    array makes the fused belief less trustworthy, so we emit at a
        //    more restricted class. Monotonic — information is only ever
        //    removed — and the demotion is part of the witness.
        let base_class = self.privacy.active_class();
        let demoted = quality.forces_privacy_demotion() || array_contradiction || mesh_at_risk;
        let effective_class = if demoted { demote_one(base_class) } else { base_class };

        // 7. Semantic state with mandatory provenance (ADR-139/140). The
        //    calibration version comes from the *agreed* epoch (None on mismatch).
        //    When a per-room adapter is active (ADR-150 §3.4) its content-derived
        //    id is part of model_version — and therefore of the witness — so the
        //    exact weights shaping inference are pinned in the trust chain.
        let calibration_version = match quality.calibration_id {
            Some(c) => format!("cal:{:016x}", c.0),
            None => "cal:none".to_string(),
        };
        let model_version = match &self.adapter {
            Some(a) => format!("rfenc-v{}+adapter:{}", self.model_version, a.adapter_id),
            None => format!("rfenc-v{}", self.model_version),
        };
        let provenance = SemanticProvenance {
            evidence: quality.evidence_refs.iter().map(|e| format!("{e:?}")).collect(),
            model_version,
            calibration_version,
            privacy_decision: format!("{:?}/{:?}", self.privacy.active_mode(), effective_class),
        };
        let statement = format!(
            "occupancy coherence={:.2} nodes={} demoted={}",
            quality.base_coherence, fused.active_nodes, demoted
        );
        let semantic_id = self.world.add_semantic_state(
            statement,
            quality.penalized_coherence(),
            now_ms,
            provenance.clone(),
            &[room],
        );
        // Retention: bound the live belief set (one node is appended per cycle;
        // without this the graph grows ~1.7M nodes/day at 20 Hz). Deterministic
        // eviction; the just-added belief is always newest and survives.
        self.world.prune_semantic_states(self.semantic_retention);

        // 8. Deterministic witness over the trust decision (ADR-137 §2.7).
        //    `effective_class` already reflects any mesh-risk demotion, so a
        //    fragmenting array shifts the witness — partition risk is auditable.
        let witness = witness_of(&provenance, effective_class);

        // 9. Drift→recalibration advisor (ADR-135 → ADR-150 §3.4): sustained
        //    low coherence, an environment change-point, or a mesh close to
        //    partitioning recommends refit.
        let recalibration_recommended = self
            .recal
            .observe(quality.base_coherence, change_point.is_some() || mesh_at_risk);

        self.cycle += 1;
        Ok(TrustedOutput {
            semantic_id,
            quality,
            effective_class,
            demoted,
            provenance,
            directional,
            change_point,
            witness,
            recalibration_recommended,
            mesh,
            phase_fusion: fused.phase_fusion,
        })
    }

    /// ADR-138: build per-node array inputs and coordinate, iff every frame's
    /// `node_id` has a registered geometry. Returns `None` otherwise.
    fn coordinate_array(&self, node_frames: &[MultiBandCsiFrame]) -> Option<DirectionalEvidence> {
        if node_frames.is_empty() {
            return None;
        }
        let mut inputs = Vec::with_capacity(node_frames.len());
        for f in node_frames {
            let g = self.node_geom.get(&f.node_id)?; // bail if any node lacks geometry
            inputs.push(ArrayNodeInput {
                node_id: u32::from(f.node_id),
                position: (g.x, g.y),
                azimuth: g.azimuth,
                coherence: f.coherence,
                clock: self.node_clock.get(&f.node_id).copied().unwrap_or(UNKNOWN_CLOCK),
                amplitude: f.channel_frames.first().map(|cf| cf.amplitude.clone()),
            });
        }
        Some(self.array.coordinate(&inputs))
    }

    /// ADR-142: fold per-node mean amplitude into the evolution tracker and,
    /// on a cross-link change-point, record an `Event` node in the WorldGraph.
    fn track_evolution(
        &mut self,
        node_frames: &[MultiBandCsiFrame],
        now_ms: i64,
        room: WorldId,
    ) -> Option<(ChangePoint, WorldId)> {
        let values: Vec<f64> = node_frames
            .iter()
            .filter_map(|f| f.channel_frames.first())
            .map(|cf| {
                if cf.amplitude.is_empty() {
                    0.0
                } else {
                    cf.amplitude.iter().map(|&a| f64::from(a)).sum::<f64>() / cf.amplitude.len() as f64
                }
            })
            .collect();
        if values.is_empty() {
            return None;
        }
        let n = values.len();
        let tracker = self
            .evolution
            .get_or_insert_with(|| EvolutionTracker::new(n, 2.0, (n / 2).max(2)));
        // Node count must be stable for the tracker to remain meaningful.
        if tracker.n_links() != n {
            return None;
        }
        let cp = tracker.observe_window(&values)?;
        let event = self.world.upsert_node(WorldNode::Event {
            id: WorldId::UNASSIGNED,
            event_type: "baseline_topology_change".to_string(),
            at_unix_ms: now_ms,
            located_in: Some(room),
        });
        let _ = self.world.add_edge(event, room, WorldEdge::LocatedIn { since_unix_ms: now_ms });
        Some((cp, event))
    }
}

/// Domain-separation tag for the witness hash. Bumping this string
/// intentionally invalidates every previously-recorded witness (a schema break).
const WITNESS_DOMAIN: &[u8] = b"ruview.engine.witness.v1";

/// Length-prefix a variable-length field into the witness hash so adjacent
/// fields can never be confused for one another. The 8-byte little-endian
/// length makes the field framing unambiguous regardless of the bytes inside
/// it (a field can contain the separator, the domain tag, anything).
fn witness_field(h: &mut blake3::Hasher, bytes: &[u8]) {
    h.update(&(bytes.len() as u64).to_le_bytes());
    h.update(bytes);
}

/// Deterministic BLAKE3 witness over a trust decision: the provenance tuple
/// (evidence ‖ model ‖ calibration ‖ privacy decision) plus the effective
/// privacy-class byte. Stable across runs for identical decisions — the
/// "signed operational belief" fingerprint (ADR-137 §2.7 / ADR-028).
///
/// # Witness integrity (review finding: domain separation)
/// Every privacy-relevant field is **length-prefixed** before hashing, and the
/// (variable-length) evidence list is preceded by an explicit count. Without
/// this framing the fields were concatenated boundary-to-boundary, so a string
/// straddling a field boundary (e.g. an adapter id absorbing the leading bytes
/// of the calibration epoch, or a model_version absorbing a trailing evidence
/// ref) collided with a *different* trust decision — silently un-distinguishing
/// two distinct privacy-relevant inputs and defeating the tamper/drift audit.
/// `model_version` is operator-influenceable (per-room adapter id, ADR-150
/// §3.4), so the ambiguity was reachable, not merely theoretical.
fn witness_of(p: &SemanticProvenance, class: PrivacyClass) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(WITNESS_DOMAIN);
    // Explicit evidence count, then each ref length-prefixed: the number of
    // evidence refs is itself privacy-relevant and must be unambiguous.
    h.update(&(p.evidence.len() as u64).to_le_bytes());
    for e in &p.evidence {
        witness_field(&mut h, e.as_bytes());
    }
    witness_field(&mut h, p.model_version.as_bytes());
    witness_field(&mut h, p.calibration_version.as_bytes());
    witness_field(&mut h, p.privacy_decision.as_bytes());
    h.update(&[class.as_u8()]);
    *h.finalize().as_bytes()
}

/// Demote a privacy class by one step (more restrictive), clamped at `Restricted`.
/// Monotonic: information is only ever removed (ADR-120/141).
fn demote_one(c: PrivacyClass) -> PrivacyClass {
    let next = (c.as_u8() + 1).min(PrivacyClass::Restricted.as_u8());
    PrivacyClass::try_from(next).unwrap_or(PrivacyClass::Restricted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wifi_densepose_signal::hardware_norm::{CanonicalCsiFrame, HardwareType};
    use wifi_densepose_signal::ruvsense::PhaseReference;

    fn node_frame(node_id: u8, ts_us: u64, n_sub: usize) -> MultiBandCsiFrame {
        MultiBandCsiFrame {
            node_id,
            timestamp_us: ts_us,
            channel_frames: vec![CanonicalCsiFrame {
                amplitude: (0..n_sub).map(|i| 1.0 + 0.1 * i as f32).collect(),
                phase: (0..n_sub).map(|i| i as f32 * 0.05).collect(),
                hardware_type: HardwareType::Esp32S3,
            }],
            frequencies_mhz: vec![2412],
            coherence: 0.9,
            phase_reference: PhaseReference::NodeLocal,
        }
    }

    fn engine() -> (StreamingEngine, WorldId) {
        let mut e = StreamingEngine::new(PrivacyMode::PrivateHome, 1, GeoRegistration::default());
        let room = e.add_room("living_room", "Living Room");
        e.add_sensor("esp32-com9", room);
        (e, room)
    }

    /// Issue #1752: independent node oscillators never yield a coherent cycle;
    /// the trusted output says phase was not combined and why.
    #[test]
    fn independent_nodes_report_amplitude_only_phase_fusion() {
        use wifi_densepose_signal::ruvsense::NonCoherentReason;
        let (mut e, room) = engine();
        let frames = [node_frame(0, 1000, 56), node_frame(1, 1001, 56)];
        let out = e.process_cycle(&frames, CalibrationId(1), room, 10_000).unwrap();
        assert_eq!(
            out.phase_fusion,
            PhaseFusion::AmplitudeOnly(NonCoherentReason::IndependentClocks)
        );
    }

    /// End-to-end trust invariant: a clean cycle produces a SemanticState whose
    /// provenance names evidence + model + calibration + privacy decision, and
    /// the calibration id flows from input → QualityScore → provenance.
    #[test]
    fn cycle_carries_full_provenance() {
        let (mut e, room) = engine();
        let cal = CalibrationId(0xABCD_1234);
        let frames = [node_frame(0, 1000, 56), node_frame(1, 1001, 56)];
        let out = e.process_cycle(&frames, cal, room, 10_000).unwrap();

        // Calibration flows all the way through.
        assert_eq!(out.quality.calibration_id, Some(cal));
        assert_eq!(out.provenance.calibration_version, "cal:00000000abcd1234");
        // Model + privacy provenance present.
        assert_eq!(out.provenance.model_version, "rfenc-v1");
        assert!(out.provenance.privacy_decision.starts_with("PrivateHome/"));
        // Evidence refs recorded.
        assert!(!out.provenance.evidence.is_empty());
        // Clean cycle (tight timestamps) → no demotion, stays Anonymous (PrivateHome).
        assert!(!out.demoted);
        assert_eq!(out.effective_class, PrivacyClass::Anonymous);

        // The SemanticState is in the graph with a DerivedFrom edge to the room.
        assert!(e.world().node(out.semantic_id).is_some());
        assert!(e
            .world()
            .neighbors(out.semantic_id)
            .iter()
            .any(|(to, edge)| *to == room && matches!(edge, WorldEdge::DerivedFrom { .. })));
    }

    /// A tolerated contradiction (loose timestamp spread, within the hard guard)
    /// demotes the privacy class one step — proving ADR-137 → ADR-141 wiring.
    #[test]
    fn contradiction_demotes_privacy() {
        let (mut e, room) = engine();
        let cal = CalibrationId(7);
        // 25 ms spread: within the 60 ms hard guard but above the 20 ms soft
        // guard (#1031 raised both to accommodate the real TDM slot offset).
        let frames = [node_frame(0, 1_000, 56), node_frame(1, 26_000, 56)];
        let out = e.process_cycle(&frames, cal, room, 20_000).unwrap();

        assert!(out.demoted, "loose alignment must demote");
        // PrivateHome base = Anonymous(2) → demoted to Restricted(3).
        assert_eq!(out.effective_class, PrivacyClass::Restricted);
        assert!(out.provenance.privacy_decision.contains("Restricted"));
        // Penalized coherence is below the base coherence.
        assert!(out.quality.penalized_coherence() <= out.quality.base_coherence);
    }

    /// Determinism: identical input twice → identical provenance + class
    /// (the ADR-136 witness-replay spirit, end-to-end through the engine).
    #[test]
    fn cycle_is_deterministic() {
        let cal = CalibrationId(42);
        let frames = [node_frame(0, 1000, 56), node_frame(1, 1001, 56)];

        let (mut e1, r1) = engine();
        let o1 = e1.process_cycle(&frames, cal, r1, 5_000).unwrap();
        let (mut e2, r2) = engine();
        let o2 = e2.process_cycle(&frames, cal, r2, 5_000).unwrap();

        assert_eq!(o1.provenance.calibration_version, o2.provenance.calibration_version);
        assert_eq!(o1.provenance.evidence, o2.provenance.evidence);
        assert_eq!(o1.effective_class, o2.effective_class);
        assert_eq!(o1.quality.per_node_weights, o2.quality.per_node_weights);
    }

    /// ADR-150 §3.4 adapter provenance: activating a per-room adapter changes
    /// the provenance model_version AND the witness — the exact weights shaping
    /// inference are pinned in the trust chain, so an adapter can never swap
    /// silently. Clearing it restores the base identity (and base witness).
    #[test]
    fn adapter_identity_is_witnessed() {
        let cal = CalibrationId(9);
        let frames = [node_frame(0, 1000, 56), node_frame(1, 1001, 56)];

        let (mut e, room) = engine();
        let base = e.process_cycle(&frames, cal, room, 1_000).unwrap();
        assert_eq!(base.provenance.model_version, "rfenc-v1");

        e.set_room_adapter(AdapterInfo {
            adapter_id: "a1b2c3d4e5f60718".into(),
            trained_samples: 150,
        });
        let adapted = e.process_cycle(&frames, cal, room, 2_000).unwrap();
        assert_eq!(
            adapted.provenance.model_version,
            "rfenc-v1+adapter:a1b2c3d4e5f60718"
        );
        assert_ne!(adapted.witness, base.witness, "adapter must shift the witness");

        // A different adapter id yields a different witness again.
        e.set_room_adapter(AdapterInfo {
            adapter_id: "ffffffffffffffff".into(),
            trained_samples: 150,
        });
        let other = e.process_cycle(&frames, cal, room, 3_000).unwrap();
        assert_ne!(other.witness, adapted.witness);

        // Clearing restores the base identity and the base witness.
        e.clear_room_adapter();
        let back = e.process_cycle(&frames, cal, room, 4_000).unwrap();
        assert_eq!(back.provenance.model_version, "rfenc-v1");
        assert_eq!(back.witness, base.witness);
    }

    /// Drift→recalibration advisor logic: a sustained low-coherence streak
    /// recommends refit; a single healthy cycle resets the streak; a
    /// change-point recommends immediately regardless of streak.
    #[test]
    fn recalibration_advisor_streak_and_change_point() {
        let mut adv = RecalibrationAdvisor {
            coherence_floor: 0.5,
            streak_threshold: 3,
            ..Default::default()
        };
        // Healthy cycles never recommend and keep the streak at zero.
        for _ in 0..5 {
            assert!(!adv.observe(0.9, false));
        }
        assert_eq!(adv.streak(), 0);
        // Two low cycles: not yet.
        assert!(!adv.observe(0.2, false));
        assert!(!adv.observe(0.2, false));
        // Third consecutive low cycle: fire.
        assert!(adv.observe(0.2, false));
        // Recovery resets the streak.
        assert!(!adv.observe(0.9, false));
        assert_eq!(adv.streak(), 0);
        // A change-point recommends immediately, even at full coherence.
        assert!(adv.observe(0.9, true));
    }

    /// Engine-level: clean coherent cycles never recommend recalibration (the
    /// advisor is wired into process_cycle and stays quiet on healthy input).
    #[test]
    fn healthy_cycles_do_not_recommend_recalibration() {
        let (mut e, room) = engine();
        e.set_recalibration_advisor(RecalibrationAdvisor {
            coherence_floor: 0.5,
            streak_threshold: 3,
            ..Default::default()
        });
        let cal = CalibrationId(2);
        for i in 0..5u64 {
            let frames = [
                node_frame(0, 1_000 + i * 50_000, 56),
                node_frame(1, 1_001 + i * 50_000, 56),
            ];
            let out = e.process_cycle(&frames, cal, room, i as i64).unwrap();
            assert!(!out.recalibration_recommended);
        }
    }

    /// Maximum total coupling mass of an n-node mesh whose attention weights
    /// sum to 1 (coupling = wᵢ·wⱼ·n): Σ_{i<j} wᵢwⱼ·n = n(1−Σwᵢ²)/2 ≤ (n−1)/2.
    /// Any cut is a subset of the edges, so every achievable cut value is
    /// bounded by this mass — a risk threshold at or above it is *guaranteed*
    /// to be crossed (deterministic fixture, review finding 4).
    fn max_coupling_mass(n_nodes: usize) -> f64 {
        (n_nodes as f64 - 1.0) / 2.0
    }

    /// Mesh guard wiring: a balanced 2-node cycle reports a mesh (cut exists)
    /// but never flags risk (min_nodes=3); a 3-node mesh whose cut value
    /// *deterministically* falls at or below the configured risk threshold
    /// (threshold = the provable upper bound on any achievable cut) is flagged
    /// at_risk, and the structural event feeds the recalibration advisor
    /// immediately — no conditional assertions (review finding 4).
    #[test]
    fn mesh_partition_risk_feeds_recalibration() {
        let (mut e, room) = engine();
        let cal = CalibrationId(3);

        // Balanced 2-node mesh: report present, no risk.
        let out = e
            .process_cycle(&[node_frame(0, 1000, 56), node_frame(1, 1001, 56)], cal, room, 1)
            .unwrap();
        let mesh = out.mesh.expect("2-node mesh reports");
        assert!(!mesh.at_risk);
        assert!(!out.recalibration_recommended);

        // 3-node mesh with the operator risk threshold set to the provable
        // cut upper bound: the crossing is deterministic regardless of the
        // fuser's exact weighting.
        e.mesh_guard_mut().risk_threshold = max_coupling_mass(3);
        let frames = [
            node_frame(0, 10_000_000, 56),
            node_frame(1, 10_000_001, 56),
            node_frame(2, 10_000_002, 56),
        ];
        let out3 = e.process_cycle(&frames, cal, room, 2).unwrap();
        let m3 = out3.mesh.expect("3-node mesh reports");
        assert!(m3.at_risk, "cut ≤ threshold must flag partition risk");
        assert!(
            out3.recalibration_recommended,
            "mesh risk is a structural event — the advisor must fire immediately, no streak"
        );
        assert!(m3.cut_value.is_finite() && m3.cut_value >= 0.0);
    }

    /// Mesh partition risk demotes the privacy class and shifts the witness —
    /// a fragmenting array makes the fused belief less trustworthy, so it is
    /// emitted at a more restricted class, and that demotion is auditable.
    /// Both cycles use the *same 3-node topology and frames*; the engines
    /// differ only in the forced mesh risk, so the witness delta is
    /// attributable to the risk demotion alone (review finding 4).
    #[test]
    fn mesh_risk_demotes_privacy_and_shifts_witness() {
        let cal = CalibrationId(8);
        let frames3 = [
            node_frame(0, 1000, 56),
            node_frame(1, 1001, 56),
            node_frame(2, 1002, 56),
        ];

        // Baseline: same topology, default risk threshold — clean cycle, not
        // demoted (PrivateHome → Anonymous), mesh healthy.
        let (mut e1, r1) = engine();
        let base = e1.process_cycle(&frames3, cal, r1, 5_000).unwrap();
        assert!(!base.mesh.as_ref().unwrap().at_risk);
        assert!(!base.demoted);
        assert_eq!(base.effective_class, PrivacyClass::Anonymous);

        // Forced risk: identical frames/topology, threshold at the provable
        // cut upper bound so the crossing is deterministic.
        let (mut e2, r2) = engine();
        e2.mesh_guard_mut().risk_threshold = max_coupling_mass(3);
        let risky = e2.process_cycle(&frames3, cal, r2, 5_000).unwrap();
        assert!(risky.mesh.as_ref().unwrap().at_risk);
        assert!(risky.demoted, "mesh risk must demote");
        // PrivateHome base Anonymous(2) → demoted to Restricted(3).
        assert_eq!(risky.effective_class, PrivacyClass::Restricted);
        assert!(risky.provenance.privacy_decision.contains("Restricted"));
        assert_ne!(
            risky.witness, base.witness,
            "same topology, risk-only delta must shift the witness"
        );
    }

    /// WorldGraph belief retention: the live loop appends one SemanticState per
    /// cycle; past the cap the oldest beliefs are evicted so graph memory is
    /// bounded, while structural nodes and the newest belief always survive.
    #[test]
    fn semantic_state_growth_is_bounded() {
        let (mut e, room) = engine();
        e.set_semantic_retention(5);
        let cal = CalibrationId(1);
        let mut last_id = None;
        let baseline_nodes = 2; // room + sensor
        for i in 0..20u64 {
            let frames = [
                node_frame(0, 1000 + i * 50_000, 56),
                node_frame(1, 1001 + i * 50_000, 56),
            ];
            let out = e.process_cycle(&frames, cal, room, 5_000 + i as i64).unwrap();
            last_id = Some(out.semantic_id);
            assert!(e.world().node_count() <= baseline_nodes + 5);
        }
        // 20 cycles ran, only 5 beliefs remain, newest is still present.
        assert_eq!(e.world().node_count(), baseline_nodes + 5);
        assert!(e.world().node(last_id.unwrap()).is_some());
        // Structural nodes survive eviction.
        assert!(e.world().node(room).is_some());
    }

    fn node_frame_scaled(node_id: u8, ts_us: u64, n_sub: usize, scale: f32) -> MultiBandCsiFrame {
        MultiBandCsiFrame {
            node_id,
            timestamp_us: ts_us,
            channel_frames: vec![CanonicalCsiFrame {
                amplitude: (0..n_sub).map(|i| scale * (1.0 + 0.1 * i as f32)).collect(),
                phase: (0..n_sub).map(|i| i as f32 * 0.05).collect(),
                hardware_type: HardwareType::Esp32S3,
            }],
            frequencies_mhz: vec![2412],
            coherence: 0.9,
            phase_reference: PhaseReference::NodeLocal,
        }
    }

    /// A well-synchronised node's clock score (ADR-110 measured stdev, fresh).
    fn synced_clock() -> ClockQualityScore {
        ClockQualityScore { offset_stdev_us: 104.0, age_us: 1_000, valid: true }
    }

    /// ADR-138 composed: with node geometry registered, the cycle produces
    /// directional evidence (admitted nodes + weights).
    #[test]
    fn array_coordinator_runs_when_geometry_registered() {
        use std::f32::consts::PI;
        let (mut e, room) = engine();
        e.register_node_geometry(0, 1.0, 0.0, 0.0);
        e.register_node_geometry(1, -1.0, 0.0, PI); // opposite → good diversity
        e.set_node_clock(0, synced_clock());
        e.set_node_clock(1, synced_clock());
        let out = e
            .process_cycle(&[node_frame(0, 1000, 56), node_frame(1, 1001, 56)], CalibrationId(1), room, 1)
            .unwrap();
        let d = out.directional.expect("geometry registered → directional evidence");
        assert_eq!(d.n_admitted, 2);
        assert!((d.weights.iter().map(|(_, w)| *w).sum::<f32>() - 1.0).abs() < 1e-3);
        // Well-separated, coherent nodes → no array contradiction → no demotion.
        assert!(!out.demoted);
    }

    /// ADR-138 composed: poor geometry (near-colinear nodes) raises a
    /// GeometryInsufficient contradiction that demotes privacy.
    #[test]
    fn array_geometry_insufficient_demotes() {
        let (mut e, room) = engine();
        e.register_node_geometry(0, 1.0, 0.0, 0.0);
        e.register_node_geometry(1, 1.0, 0.01, 0.01); // nearly colinear → low GDI
        e.set_node_clock(0, synced_clock());
        e.set_node_clock(1, synced_clock());
        let out = e
            .process_cycle(&[node_frame(0, 1000, 56), node_frame(1, 1001, 56)], CalibrationId(1), room, 1)
            .unwrap();
        let d = out.directional.unwrap();
        assert!(!d.contradictions.is_empty(), "insufficient geometry flagged");
        assert!(out.demoted && out.effective_class == PrivacyClass::Restricted);
    }

    /// Issue #2156: a node with no sync evidence is not assumed to be on a
    /// shared clock. The gate rejects it as `ClockInvalid` instead of admitting
    /// it on a hard-coded score.
    #[test]
    fn nodes_without_clock_evidence_are_rejected_by_the_gate() {
        use std::f32::consts::PI;
        use wifi_densepose_ruvector::viewpoint::coherence::{ClockGateDecision, ClockRejectReason};
        let (mut e, room) = engine();
        e.register_node_geometry(0, 1.0, 0.0, 0.0);
        e.register_node_geometry(1, -1.0, 0.0, PI);
        e.set_node_clock(0, synced_clock());
        let out = e
            .process_cycle(&[node_frame(0, 1000, 56), node_frame(1, 1001, 56)], CalibrationId(1), room, 1)
            .unwrap();
        let d = out.directional.expect("geometry registered → directional evidence");
        assert_eq!(d.n_admitted, 1, "only the node with sync evidence is admitted");
        let node1 = d.gate_decisions.iter().find(|(id, _)| *id == 1).unwrap().1;
        assert_eq!(node1, ClockGateDecision::Reject { reason: ClockRejectReason::ClockInvalid });

        // An explicitly invalid clock is rejected the same way.
        e.set_node_clock(0, UNKNOWN_CLOCK);
        let out = e
            .process_cycle(&[node_frame(0, 2000, 56), node_frame(1, 2001, 56)], CalibrationId(1), room, 2)
            .unwrap();
        assert_eq!(out.directional.unwrap().n_admitted, 0);
    }

    /// ADR-142 composed: a sustained baseline then a simultaneous amplitude
    /// shift on both links yields a change-point + an Event node in the graph.
    #[test]
    fn evolution_change_point_recorded_as_event() {
        let (mut e, room) = engine();
        let cal = CalibrationId(1);
        // Jittered baseline so each link has non-zero std (constant std=0 is undefined).
        for i in 0..30u64 {
            let s = if i % 2 == 0 { 0.99 } else { 1.01 };
            let out = e
                .process_cycle(&[node_frame_scaled(0, 1000, 56, s), node_frame_scaled(1, 1001, 56, s)], cal, room, i as i64)
                .unwrap();
            assert!(out.change_point.is_none(), "baseline must not trip a change-point");
        }
        // Large simultaneous excursion on both links → change-point.
        let out = e
            .process_cycle(&[node_frame_scaled(0, 1000, 56, 1.6), node_frame_scaled(1, 1001, 56, 1.6)], cal, room, 99)
            .unwrap();
        let (_, event_id) = out.change_point.expect("simultaneous shift → change-point");
        assert!(matches!(
            e.world().node(event_id),
            Some(WorldNode::Event { event_type, .. }) if event_type == "baseline_topology_change"
        ));
    }

    /// ADR-143 composed: ingesting stable reflector sightings writes an
    /// ObjectAnchor node into the WorldGraph.
    #[test]
    fn reflector_ingestion_writes_object_anchors() {
        use wifi_densepose_signal::ruvsense::ReflectorObservation;
        let (mut e, _room) = engine();
        let day_ns = 86_400_000_000_000u64;
        // 8 tight, coherent sightings spanning ~a day → a stable Wall anchor.
        let obs: Vec<ReflectorObservation> = (0..8u64)
            .map(|i| {
                let j = if i % 2 == 0 { 0.005 } else { -0.005 };
                ReflectorObservation { position: [3.0 + j, 1.0, 0.0], delay_ns: 12.0, coherence: 0.9, at_ns: i * (day_ns / 8) }
            })
            .collect();
        let written = e.ingest_reflectors(&obs);
        assert!(!written.is_empty(), "stable reflector → ObjectAnchor written");
        assert!(matches!(
            e.world().node(written[0]),
            Some(WorldNode::ObjectAnchor { .. })
        ));
    }

    /// ADR-137 acceptance (the trust-root path):
    /// `two calibrated frames -> calibration mismatch -> QualityScore
    ///  contradiction -> Restricted -> calibration_id None -> witness stable`.
    #[test]
    fn calibration_mismatch_demotes_and_witness_stable() {
        let run = || {
            let (mut e, room) = engine();
            // PrivateHome base = Anonymous; mismatch must demote to Restricted.
            e.process_cycle_calibrated(
                &[node_frame(0, 1000, 56), node_frame(1, 1001, 56)],
                &[Some(CalibrationId(1)), Some(CalibrationId(2))], // DISAGREE
                room,
                1,
            )
            .unwrap()
        };
        let out = run();
        // QualityScore raised the contradiction; no single calibration epoch.
        assert!(out.quality.forces_privacy_demotion());
        assert_eq!(out.quality.calibration_id, None);
        assert_eq!(out.provenance.calibration_version, "cal:none");
        // BFLD class demoted to Restricted (identity surface removed downstream).
        assert!(out.demoted);
        assert_eq!(out.effective_class, PrivacyClass::Restricted);
        // Witness is deterministic across identical runs.
        assert_eq!(out.witness, run().witness);
        assert_ne!(out.witness, [0u8; 32]);
    }

    /// Agreeing calibrations set the epoch and do NOT demote (the happy path
    /// counterpart, proving the mismatch test isn't trivially always-demoting).
    #[test]
    fn matching_calibration_sets_epoch_no_demotion() {
        let (mut e, room) = engine();
        let cal = CalibrationId(0xABCD);
        let out = e
            .process_cycle_calibrated(
                &[node_frame(0, 1000, 56), node_frame(1, 1001, 56)],
                &[Some(cal), Some(cal)],
                room,
                1,
            )
            .unwrap();
        assert_eq!(out.quality.calibration_id, Some(cal));
        assert!(!out.demoted);
        assert_eq!(out.effective_class, PrivacyClass::Anonymous);
    }

    /// ADR-139 live-loop acceptance (the architecture-proving path):
    /// `live_frame -> fusion_event -> worldgraph_update -> privacy_rollup ->
    ///  persist -> reload -> same_contents`, with NO raw RF frame persisted.
    #[test]
    fn live_frame_to_reload_same_contents() {
        let mut e =
            StreamingEngine::new(PrivacyMode::StrictNoIdentity, 1, GeoRegistration::default());
        let room = e.add_room("living_room", "Living Room");
        let sensor = e.add_sensor("esp32-com9", room);

        // live_frame -> fusion_event -> worldgraph_update (SemanticState).
        let out = e
            .process_cycle(&[node_frame(0, 1000, 56), node_frame(1, 1001, 56)], CalibrationId(9), room, 100)
            .unwrap();
        // person track feeding.
        let pt = e.update_person_track(7, 2.0, 2.0, room, sensor);

        // privacy_rollup: StrictNoIdentity suppresses the person_track.
        let rollup = e.apply_active_privacy_mode();
        assert!(rollup.suppressed_nodes.contains(&pt), "person track suppressed");
        assert!(rollup.denied_pairs.iter().any(|(_s, n)| *n == pt));

        // persist.
        let bytes = e.snapshot_json().unwrap();
        // No raw RF frame persisted — the snapshot is graph nodes/edges only.
        let json = String::from_utf8(bytes.clone()).unwrap();
        assert!(!json.contains("\"amplitude\"") && !json.contains("\"data\""), "no raw RF in snapshot");

        // reload.
        let reloaded = WorldGraph::from_json(&bytes).unwrap();

        // same_contents: node count, area resolution, the SemanticState + track,
        // and an identical room-contents query before vs after reload.
        assert_eq!(reloaded.node_count(), e.world().node_count());
        assert_eq!(reloaded.room_for_area("living_room"), e.world().room_for_area("living_room"));
        assert!(reloaded.node(out.semantic_id).is_some());
        assert!(reloaded.node(pt).is_some());
        let mut before = e.world().contents_of(room);
        before.sort_by_key(|w| w.0);
        let mut after = reloaded.contents_of(room);
        after.sort_by_key(|w| w.0);
        assert_eq!(before, after, "same room-contents query after reload");
        // Deterministic persistence: re-serialising the reload is byte-identical.
        assert_eq!(reloaded.to_json().unwrap(), bytes);
    }

    /// The privacy mode switch is recorded in a verifiable attestation chain
    /// (ADR-141), and a stricter mode raises the emitted class.
    #[test]
    fn privacy_mode_switch_is_attested_and_effective() {
        let (mut e, room) = engine();
        e.set_privacy_mode(PrivacyMode::StrictNoIdentity);
        assert!(e.privacy().verify_chain());
        let out = e
            .process_cycle(&[node_frame(0, 1000, 56), node_frame(1, 1001, 56)], CalibrationId(1), room, 1)
            .unwrap();
        // StrictNoIdentity base = Restricted, even with no contradiction.
        assert_eq!(out.effective_class, PrivacyClass::Restricted);
    }

    /// De-magic pin (review finding): the named engine constants must keep
    /// their prior inline values exactly, so the de-magic is a pure rename with
    /// no behavior change.
    #[test]
    fn engine_constants_match_prior_values() {
        assert_eq!(StreamingEngine::DEFAULT_COHERENCE_ACCEPT, 0.85);
        assert_eq!(StreamingEngine::SLAM_ASSOC_RADIUS_M, 0.5);
        assert_eq!(StreamingEngine::SLAM_MIN_SIGHTINGS, 5);
        assert_eq!(StreamingEngine::SLAM_MIN_COHERENCE, 0.6);
        assert_eq!(StreamingEngine::ANCHOR_WALL_CEILING, 0.05);
        assert_eq!(StreamingEngine::ANCHOR_MOBILE_FLOOR, 1.0);
    }

    /// Privacy monotonicity (the crux): across EVERY base mode, a forced
    /// contradiction may only ever make the emitted class *more* restrictive
    /// (higher byte) and never less. Demotion is single-step and clamps at
    /// Restricted; a clean cycle emits exactly the base class. This is the
    /// information-only-removed invariant of ADR-141/120 stated as a property
    /// over the whole mode set.
    #[test]
    fn forced_contradiction_never_relaxes_class() {
        let cal_mismatch = [Some(CalibrationId(1)), Some(CalibrationId(2))]; // disagree → contradiction
        let cal_match = [Some(CalibrationId(5)), Some(CalibrationId(5))];
        let frames = [node_frame(0, 1000, 56), node_frame(1, 1001, 56)];
        for mode in [
            PrivacyMode::RawResearch,
            PrivacyMode::PrivateHome,
            PrivacyMode::EnterpriseAnonymous,
            PrivacyMode::CareWithConsent,
            PrivacyMode::StrictNoIdentity,
        ] {
            let base_class = mode.target_class();

            // Clean cycle: emits exactly the base class (no relaxation upward).
            let mut clean = StreamingEngine::new(mode, 1, GeoRegistration::default());
            let room_c = clean.add_room("r", "R");
            let oc = clean
                .process_cycle_calibrated(&frames, &cal_match, room_c, 1)
                .unwrap();
            assert_eq!(oc.effective_class, base_class, "clean cycle == base class");
            assert!(!oc.demoted);

            // Forced contradiction: class byte only ever increases (more
            // restrictive), never decreases below the base.
            let mut dirty = StreamingEngine::new(mode, 1, GeoRegistration::default());
            let room_d = dirty.add_room("r", "R");
            let od = dirty
                .process_cycle_calibrated(&frames, &cal_mismatch, room_d, 1)
                .unwrap();
            assert!(od.demoted, "calibration mismatch must demote in {mode:?}");
            assert!(
                od.effective_class.as_u8() >= base_class.as_u8(),
                "demotion must never relax: {mode:?} base={:?} got={:?}",
                base_class,
                od.effective_class
            );
            // And it must be strictly more restrictive unless already clamped
            // at the most-restrictive class.
            if base_class != PrivacyClass::Restricted {
                assert!(
                    od.effective_class.as_u8() > base_class.as_u8(),
                    "unclamped demotion must increase restriction in {mode:?}"
                );
            } else {
                assert_eq!(od.effective_class, PrivacyClass::Restricted);
            }
        }
    }

    /// Fail-closed boundary: an empty cycle (zero frames) must NOT emit a
    /// trusted output at all — fusion rejects it and the engine surfaces a
    /// hard error. There is no degenerate output that could carry a stale or
    /// over-permissive class.
    #[test]
    fn empty_cycle_fails_closed() {
        let (mut e, room) = engine();
        let err = e.process_cycle(&[], CalibrationId(1), room, 1);
        assert!(matches!(err, Err(EngineError::Fusion(_))), "empty cycle must error, got {err:?}");
        // No SemanticState was appended (room + sensor only).
        assert_eq!(e.world().node_count(), 2);
        assert_eq!(e.cycle_count(), 0, "a failed cycle must not advance the counter");
    }

    /// Single-node boundary characterization: a one-node cycle fuses (no
    /// multistatic cross-check is possible), reports no mesh (n<2), and emits a
    /// well-formed witness at the base class. Documents that single-node sensing
    /// is a valid, non-demoting mode — not a silent bypass.
    #[test]
    fn single_node_cycle_is_well_formed() {
        let (mut e, room) = engine();
        let out = e
            .process_cycle(&[node_frame(0, 1000, 56)], CalibrationId(1), room, 1)
            .unwrap();
        assert!(out.mesh.is_none(), "one node has no mesh cut");
        assert!(out.directional.is_none(), "no geometry registered");
        assert_eq!(out.effective_class, PrivacyClass::Anonymous); // PrivateHome base
        assert_ne!(out.witness, [0u8; 32], "witness still emitted");
    }

    /// Witness domain-separation (review finding): the witness must change
    /// whenever ANY privacy-relevant field changes. The model_version,
    /// calibration_version, and privacy_decision fields are concatenated into
    /// the hash; without an unambiguous delimiter between them, a string that
    /// straddles the model/calibration boundary collides with a different
    /// (model, calibration) tuple.
    ///
    /// `model_version` is operator-influenceable through the per-room adapter id
    /// (ADR-150 §3.4), and `calibration_version` is `cal:<hex>` — so the two
    /// provenances below are *both reachable* and represent genuinely different
    /// trust decisions (different model identity, different calibration epoch),
    /// yet the field-boundary ambiguity makes them hash-collide. A colliding
    /// witness silently un-distinguishes two distinct privacy-relevant inputs,
    /// defeating the tamper/drift audit guarantee.
    #[test]
    fn witness_distinguishes_model_calibration_boundary() {
        let class = PrivacyClass::Anonymous;
        // A: model "rfenc-v1+adapter:X", calibration epoch "cal:00ab".
        let a = SemanticProvenance {
            evidence: vec!["ev".into()],
            model_version: "rfenc-v1+adapter:X".into(),
            calibration_version: "cal:00ab".into(),
            privacy_decision: "PrivateHome/Anonymous".into(),
        };
        // B: adapter id absorbs the leading "cal:00a" of A's calibration; B's
        // own calibration is the remaining "b". A.model‖A.cal == B.model‖B.cal,
        // so the unseparated concatenation hashes identically — yet these are
        // distinct (model identity, calibration epoch) tuples.
        let b = SemanticProvenance {
            evidence: vec!["ev".into()],
            model_version: "rfenc-v1+adapter:Xcal:00a".into(),
            calibration_version: "b".into(),
            privacy_decision: "PrivateHome/Anonymous".into(),
        };
        assert_ne!(a.model_version, b.model_version);
        assert_ne!(a.calibration_version, b.calibration_version);
        // Sanity: the two collide under naive concatenation.
        assert_eq!(
            format!("{}{}", a.model_version, a.calibration_version),
            format!("{}{}", b.model_version, b.calibration_version),
        );
        assert_ne!(
            witness_of(&a, class),
            witness_of(&b, class),
            "distinct (model, calibration) tuples must not share a witness"
        );
    }

    /// Witness domain-separation across the evidence/model boundary: a witness
    /// must distinguish an extra evidence ref from a model_version that absorbs
    /// the same bytes. The evidence loop terminates each ref with one separator;
    /// the model field must itself be unambiguously delimited from the (variable
    /// number of) evidence refs that precede it.
    #[test]
    fn witness_distinguishes_evidence_model_boundary() {
        let class = PrivacyClass::Anonymous;
        let a = SemanticProvenance {
            evidence: vec!["e1".into(), "e2".into()],
            model_version: "m".into(),
            calibration_version: "cal:1".into(),
            privacy_decision: "PrivateHome/Anonymous".into(),
        };
        let b = SemanticProvenance {
            evidence: vec!["e1".into()],
            // absorbs "e2" + its 0x1f separator into the model field.
            model_version: "e2\u{1f}m".into(),
            calibration_version: "cal:1".into(),
            privacy_decision: "PrivateHome/Anonymous".into(),
        };
        assert_ne!(
            witness_of(&a, class),
            witness_of(&b, class),
            "an extra evidence ref must not collide with a model_version that absorbs it"
        );
    }

    // ---- Issue #1870: StreamingEngine's own fuser never received positions ----

    /// The governed path (`fuse_scored_calibrated`, feeding provenance and the
    /// witness) must carry each node's configured position -- not the origin
    /// for every node, which is what happened before `set_node_positions`
    /// existed on `StreamingEngine` and nothing ever configured this fuser
    /// instance (distinct from the one `wifi-densepose-sensing-server`'s
    /// `AppState` owns, fixed separately in #1860).
    #[test]
    fn governed_fuse_carries_each_nodes_configured_position() {
        let (mut e, _room) = engine();
        let mut positions = std::collections::HashMap::new();
        positions.insert(0u8, [1.0, 2.0, 3.0]);
        positions.insert(1u8, [4.0, 5.0, 6.0]);
        e.set_node_positions(positions.clone());

        let frames = [node_frame(0, 1000, 56), node_frame(1, 1001, 56)];
        let (fused, _score) = e
            .fuser
            .fuse_scored_calibrated(&frames, &[None, None], StreamingEngine::DEFAULT_COHERENCE_ACCEPT)
            .expect("cohort fuses");

        assert_eq!(fused.node_positions.len(), frames.len());
        for (frame, position) in fused.node_frames.iter().zip(&fused.node_positions) {
            let expected = positions
                .get(&frame.node_id)
                .copied()
                .expect("every configured node has a position in this test");
            assert_eq!(
                *position, expected,
                "node {} received the wrong position on the governed path",
                frame.node_id
            );
        }
    }

    /// `set_multistatic_config` used to rebuild the fuser from scratch,
    /// silently discarding any positions configured before it. A caller that
    /// sets positions once at startup and derives/applies a TDM-schedule
    /// config later (a real, documented ordering per #1049/#1057) must not
    /// lose them.
    #[test]
    fn positions_survive_a_later_set_multistatic_config() {
        let (mut e, _room) = engine();
        let mut positions = std::collections::HashMap::new();
        positions.insert(0u8, [7.0, 8.0, 9.0]);
        e.set_node_positions(positions.clone());

        e.set_multistatic_config(MultistaticConfig {
            guard_interval_us: 30_000,
            ..MultistaticConfig::default()
        });

        assert_eq!(
            e.fuser.node_position(0),
            Some([7.0, 8.0, 9.0]),
            "set_multistatic_config must not discard previously-configured positions"
        );
    }
}
