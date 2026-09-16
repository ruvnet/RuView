//! Live trust-path bridge: drive the governed [`StreamingEngine`] from the
//! sensing-server's live `NodeState` map.
//!
//! `multistatic_bridge.rs` already converts `NodeState` → `MultiBandCsiFrame`
//! and runs the *bare* `MultistaticFuser`. That path produces fused amplitudes
//! but skips the trust control plane: privacy demotion on contradiction, the
//! WorldGraph belief with mandatory provenance, and the deterministic witness
//! (ADR-135..146). This bridge routes the same live frames through
//! [`StreamingEngine::process_cycle`], so every governed belief carries
//! evidence + model + calibration + privacy decision and a BLAKE3 witness
//! (narrowing the gap called out in ADR-136 §8 and the beyond-SOTA system
//! review).
//!
//! ## Honest scope of the live-path governance
//!
//! The engine runs *alongside* the bare fusion path that feeds the live
//! `SensingUpdate`; it does not replace it. What the engine's decision **does**
//! gate on the live wire today: when a cycle is emitted at
//! [`PrivacyClass::Restricted`] (base mode or contradiction/mesh-risk
//! demotion), [`EngineBridge::suppress_raw_outputs`] is true and `main.rs`
//! strips the per-node raw amplitude vectors from the published update — the
//! same field mapping `wifi-densepose-bfld`'s privacy gate applies at
//! `Restricted` (drop amplitude/phase proxies). Trust state (latest witness,
//! effective class, recalibration flag, engine-error count) is readable on
//! `GET /api/v1/status`. Gating of the remaining *derived* outputs
//! (person count, classification, signal field) by privacy class is tracked
//! as a follow-up; until then those fields are published ungoverned.
//!
//! Determinism: this module reads server state and forwards explicit
//! timestamps/calibration ids; it introduces no wall-clock reads of its own, so
//! a given `(frames, calibration, now_ms)` always yields the same
//! [`TrustedOutput`] witness.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use wifi_densepose_bfld::{PrivacyClass, PrivacyMode};
use wifi_densepose_signal::ruvsense::fusion_quality::ContradictionFlag;
use wifi_densepose_engine::{AdapterInfo, EngineError, StreamingEngine, TrustedOutput};
use wifi_densepose_geo::types::GeoRegistration;
use wifi_densepose_signal::ruvsense::fusion_quality::CalibrationId;
use wifi_densepose_signal::ruvsense::multistatic::MultistaticConfig;
use wifi_densepose_worldgraph::WorldId;

use super::multistatic_bridge::node_frames_from_states_with_guard;
use super::NodeState;

/// Minimum spacing between engine-error warn logs (errors are still counted
/// every cycle; only the log line is rate-limited — a 20 Hz loop must not
/// emit 20 warns/s).
const ENGINE_ERROR_WARN_INTERVAL: Duration = Duration::from_secs(10);

/// Minimum spacing between demotion warn logs. Demotions are a *normal*
/// outcome of a tolerated contradiction and can fire on every cycle, so the
/// first one is always logged and the rest are rate-limited.
const DEMOTION_WARN_INTERVAL: Duration = Duration::from_secs(30);

/// Owns a [`StreamingEngine`] and the WorldGraph scope (one room + sensor) the
/// live sensing loop publishes beliefs into.
pub struct EngineBridge {
    engine: StreamingEngine,
    /// Hard timestamp guard shared by cohort selection and engine validation.
    guard_interval_us: u64,
    room: WorldId,
    /// Nodes already wired into the WorldGraph as sensors (by `node_id`).
    registered_nodes: HashMap<u8, WorldId>,
    /// Calibration epoch applied to live frames until the ADR-135 baseline
    /// stage supplies a real per-node id. Stable so witnesses are reproducible.
    calibration: CalibrationId,
    // ── Trust state observed from the most recent cycles (review finding 1:
    //    previously write-only fields on AppState; now recorded here and
    //    exposed via the status endpoint + output gating). ──────────────────
    /// BLAKE3 witness of the most recent successful governed cycle.
    last_witness: Option<[u8; 32]>,
    /// Latest drift→recalibration recommendation (ADR-135 → ADR-150 §3.4).
    recalibration_recommended: bool,
    /// Privacy class the most recent cycle was emitted under (post-demotion).
    effective_class: Option<PrivacyClass>,
    /// Whether the most recent cycle was demoted (contradiction / mesh risk).
    demoted: bool,
    /// Total engine cycles that returned an error (previously swallowed by
    /// `if let Some(Ok(..))` at the call sites).
    engine_error_count: u64,
    /// Last time an engine error was actually logged (rate limiter).
    last_error_warn_at: Option<Instant>,
    /// Why the most recent cycle was demoted, as a stable slug naming the
    /// trigger (`timestamp_mismatch`, `calibration_id_mismatch`, ...,
    /// `mesh_at_risk`, joined with `+` when several fired). `None` when the
    /// last cycle was emitted at the base class. ADR-141 review finding 1c
    /// strips the raw amplitude/phase proxies on a demoted cycle, and without
    /// this field a consumer could only observe the stripped output — never
    /// the reason (a 46%-of-cycles `TimestampMismatch` demotion on a real
    /// three-node array looked exactly like a broken CSI pipeline).
    demotion_reason: Option<String>,
    /// Cycles demoted since startup.
    demotion_count: u64,
    /// Last time a demotion was actually logged (rate limiter, kept separate
    /// from the error limiter so a demotion storm cannot hide an engine error).
    last_demotion_warn_at: Option<Instant>,
}

impl EngineBridge {
    /// Build a bridge for one installation. `room_area_id`/`room_name` name the
    /// observation scope; `mode` is the starting privacy mode.
    ///
    /// `multistatic_cfg` is `None` on the pure default (60 ms/20 ms guard);
    /// callers that derive a config from `WDP_TDM_SLOTS`/`WDP_GUARD_INTERVAL_US`
    /// (see `main.rs::multistatic_guard_config_from_env`) should pass it here
    /// too — otherwise the governed trust cycle silently keeps using the
    /// hardcoded default even though the sibling `multistatic_fuser` field on
    /// `AppState` picked up the override (#1049/#1057).
    pub fn new(
        mode: PrivacyMode,
        model_version: u16,
        room_area_id: &str,
        room_name: &str,
        multistatic_cfg: Option<MultistaticConfig>,
    ) -> Self {
        let guard_interval_us = multistatic_cfg
            .as_ref()
            .map_or_else(|| MultistaticConfig::default().guard_interval_us, |cfg| {
                cfg.guard_interval_us
            });
        let mut engine = StreamingEngine::new(mode, model_version, GeoRegistration::default());
        if let Some(cfg) = multistatic_cfg {
            engine.set_multistatic_config(cfg);
        }
        let room = engine.add_room(room_area_id, room_name);
        Self {
            engine,
            guard_interval_us,
            room,
            registered_nodes: HashMap::new(),
            calibration: CalibrationId(0x5256_0001), // "RV\0\x01" — placeholder epoch
            last_witness: None,
            recalibration_recommended: false,
            effective_class: None,
            demoted: false,
            engine_error_count: 0,
            last_error_warn_at: None,
            demotion_reason: None,
            demotion_count: 0,
            last_demotion_warn_at: None,
        }
    }

    /// Override the calibration epoch stamped onto live frames (ADR-135).
    pub fn set_calibration(&mut self, calibration: CalibrationId) {
        self.calibration = calibration;
    }

    /// Override the WorldGraph belief-retention cap (bounds memory on the live
    /// loop; see `WorldGraph::prune_semantic_states`).
    pub fn set_semantic_retention(&mut self, max_states: usize) {
        self.engine.set_semantic_retention(max_states);
    }

    /// Switch the active privacy mode (operator/control-plane action).
    pub fn set_privacy_mode(&mut self, mode: PrivacyMode) {
        self.engine.set_privacy_mode(mode);
    }

    /// Activate a per-room calibration adapter (ADR-150 §3.4). The adapter's
    /// content-derived id becomes part of provenance/witness from the next
    /// cycle — weights can never swap silently on the live path.
    pub fn set_room_adapter(&mut self, info: AdapterInfo) {
        self.engine.set_room_adapter(info);
    }

    /// Deactivate the per-room adapter (revert to the shared base model).
    pub fn clear_room_adapter(&mut self) {
        self.engine.clear_room_adapter();
    }

    /// Borrow the engine (queries, WorldGraph snapshot, privacy audit).
    pub fn engine(&self) -> &StreamingEngine {
        &self.engine
    }

    /// Number of sensor nodes wired into the WorldGraph so far.
    pub fn registered_node_count(&self) -> usize {
        self.registered_nodes.len()
    }

    /// Run one governed trust cycle over the current live node states.
    ///
    /// Returns `None` when no active node yields a frame (nothing to fuse —
    /// the engine is not invoked, so no spurious belief is published). On a
    /// real cycle it lazily wires any newly-seen node as a WorldGraph sensor,
    /// then returns the witnessed [`TrustedOutput`] (or a fusion error).
    ///
    /// `now_ms` is supplied by the caller (the sensing loop's clock), keeping
    /// the bridge deterministic and replayable.
    pub fn process_cycle_from_states(
        &mut self,
        node_states: &HashMap<u8, NodeState>,
        now_ms: i64,
    ) -> Option<Result<TrustedOutput, EngineError>> {
        let frames =
            node_frames_from_states_with_guard(node_states, self.guard_interval_us);
        if frames.is_empty() {
            return None;
        }
        // Lazily register each contributing node as a sensor observing the room,
        // so the privacy rollup can suppress it under identity-strict modes.
        for f in &frames {
            self.registered_nodes.entry(f.node_id).or_insert_with(|| {
                self.engine
                    .add_sensor(&format!("node-{}", f.node_id), self.room)
            });
        }
        Some(
            self.engine
                .process_cycle(&frames, self.calibration, self.room, now_ms),
        )
    }

    /// Run one governed cycle **and record the trust state** (review finding
    /// 1): on success the witness / effective class / demotion /
    /// recalibration flag are stored for the status endpoint and output
    /// gating; on error the error counter is incremented and a rate-limited
    /// warning is logged (never silently swallowed). Returns the trusted
    /// output on success, `None` when there was nothing to fuse or the cycle
    /// errored.
    pub fn observe_cycle(
        &mut self,
        node_states: &HashMap<u8, NodeState>,
        now_ms: i64,
    ) -> Option<TrustedOutput> {
        let result = self.process_cycle_from_states(node_states, now_ms)?;
        self.record_cycle_result(result)
    }

    fn record_cycle_result(
        &mut self,
        result: Result<TrustedOutput, EngineError>,
    ) -> Option<TrustedOutput> {
        match result {
            Ok(trust) => {
                self.last_witness = Some(trust.witness);
                self.recalibration_recommended = trust.recalibration_recommended;
                self.effective_class = Some(trust.effective_class);
                self.demoted = trust.demoted;
                if trust.demoted {
                    self.demotion_count += 1;
                    self.demotion_reason = Some(demotion_reason_of(&trust));
                    let now = Instant::now();
                    let warn_due = self.last_demotion_warn_at.map_or(true, |t| {
                        now.duration_since(t) >= DEMOTION_WARN_INTERVAL
                    });
                    if warn_due {
                        self.last_demotion_warn_at = Some(now);
                        tracing::warn!(
                            demotion_reason = self.demotion_reason.as_deref().unwrap_or("unnamed"),
                            total_demotions = self.demotion_count,
                            guard_interval_us = self.guard_interval_us,
                            "governed cycle demoted: ADR-141 strips the per-node raw amplitude/phase \
                             proxies from the live publish for this cycle (see /api/v1/status \
                             trust.demotion_reason). `timestamp_mismatch` means the newest per-node \
                             frame timestamps are further apart than the fusion soft guard \
                             (WDP_SOFT_GUARD_US); on a node array slower than 1/soft_guard frames per \
                             second that fires on a large fraction of cycles"
                        );
                    }
                } else {
                    self.demotion_reason = None;
                }
                Some(trust)
            }
            Err(e) => {
                self.engine_error_count += 1;
                let now = Instant::now();
                let warn_due = self.last_error_warn_at.map_or(true, |t| {
                    now.duration_since(t) >= ENGINE_ERROR_WARN_INTERVAL
                });
                if warn_due {
                    self.last_error_warn_at = Some(now);
                    tracing::warn!(
                        total_engine_errors = self.engine_error_count,
                        "governed trust cycle failed (warn rate-limited to one per {:?}): {e}",
                        ENGINE_ERROR_WARN_INTERVAL
                    );
                }
                None
            }
        }
    }

    /// BLAKE3 witness of the most recent successful governed cycle.
    pub fn last_trust_witness(&self) -> Option<[u8; 32]> {
        self.last_witness
    }

    /// Latest drift→recalibration recommendation from the governed engine.
    pub fn recalibration_recommended(&self) -> bool {
        self.recalibration_recommended
    }

    /// Privacy class the most recent cycle was emitted under (post-demotion);
    /// `None` until a governed cycle has run.
    pub fn effective_class(&self) -> Option<PrivacyClass> {
        self.effective_class
    }

    /// Whether the most recent cycle was demoted (contradiction / mesh risk).
    pub fn demoted(&self) -> bool {
        self.demoted
    }

    /// Engine cycles that returned an error since startup.
    pub fn engine_error_count(&self) -> u64 {
        self.engine_error_count
    }

    /// Stable slug naming why the most recent cycle was demoted, or `None`
    /// when it was emitted at the base class. Set by `observe_cycle`.
    pub fn demotion_reason(&self) -> Option<&str> {
        self.demotion_reason.as_deref()
    }

    /// Number of cycles demoted since startup (each one strips the raw
    /// amplitude/phase proxies from the live publish).
    pub fn demotion_count(&self) -> u64 {
        self.demotion_count
    }

    /// ADR-141 output mapping for the live publish path (review finding 1c):
    /// at effective class [`PrivacyClass::Restricted`] the bfld privacy gate
    /// drops the amplitude + phase proxies; the live `SensingUpdate` applies
    /// the same field mapping by suppressing the per-node raw amplitude
    /// vectors when this returns true. Classes below `Restricted` leave the
    /// publish unchanged.
    pub fn suppress_raw_outputs(&self) -> bool {
        self.effective_class
            .is_some_and(|c| c.as_u8() >= PrivacyClass::Restricted.as_u8())
    }
}

/// Stable slug for every trigger that demoted this cycle, so a status
/// consumer can tell a privacy-class policy from a timing wall without
/// parsing a log line.
fn demotion_reason_of(trust: &TrustedOutput) -> String {
    let mut parts: Vec<&'static str> = Vec::new();
    for flag in &trust.quality.contradiction_flags {
        parts.push(contradiction_slug(flag));
    }
    if let Some(directional) = trust.directional.as_ref() {
        for flag in &directional.contradictions {
            parts.push(contradiction_slug(flag));
        }
    }
    if trust.mesh.as_ref().is_some_and(|m| m.at_risk) {
        parts.push("mesh_at_risk");
    }
    parts.dedup();
    if parts.is_empty() {
        // A demotion must come from one of the three inputs above; if one ever
        // arrives unnamed, say so rather than inventing a cause.
        "unnamed".to_string()
    } else {
        parts.join("+")
    }
}

fn contradiction_slug(flag: &ContradictionFlag) -> &'static str {
    match flag {
        ContradictionFlag::TimestampMismatch { .. } => "timestamp_mismatch",
        ContradictionFlag::CalibrationIdMismatch { .. } => "calibration_id_mismatch",
        ContradictionFlag::PhaseAlignmentFailed { .. } => "phase_alignment_failed",
        ContradictionFlag::DriftProfileConflict { .. } => "drift_profile_conflict",
        ContradictionFlag::CoherenceDrop { .. } => "coherence_drop",
        ContradictionFlag::GeometryInsufficient { .. } => "geometry_insufficient",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::time::Instant;
    use wifi_densepose_bfld::PrivacyClass;

    fn node_state_with_history(amp: f64, n_sub: usize) -> NodeState {
        let mut ns = NodeState::new();
        let frame: Vec<f64> = (0..n_sub).map(|i| amp + 0.1 * i as f64).collect();
        ns.frame_history = VecDeque::from(vec![frame]);
        ns.last_frame_time = Some(Instant::now());
        ns
    }

    fn two_node_states() -> HashMap<u8, NodeState> {
        let mut m = HashMap::new();
        m.insert(0u8, node_state_with_history(1.0, 56));
        m.insert(1u8, node_state_with_history(1.05, 56));
        m
    }

    #[test]
    fn empty_states_produce_no_belief() {
        let mut bridge = EngineBridge::new(PrivacyMode::PrivateHome, 1, "living_room", "Living Room", None);
        let out = bridge.process_cycle_from_states(&HashMap::new(), 1_000);
        assert!(out.is_none());
        // No belief published, no sensor wired.
        assert_eq!(bridge.registered_node_count(), 0);
    }

    #[test]
    fn live_cycle_produces_witnessed_belief_with_provenance() {
        let mut bridge = EngineBridge::new(PrivacyMode::PrivateHome, 1, "living_room", "Living Room", None);
        let states = two_node_states();
        let out = bridge
            .process_cycle_from_states(&states, 10_000)
            .expect("frames present")
            .expect("fusion succeeds");

        // Full provenance: evidence + model + calibration + privacy decision.
        assert!(!out.provenance.evidence.is_empty());
        assert_eq!(out.provenance.model_version, "rfenc-v1");
        assert!(out.provenance.calibration_version.starts_with("cal:"));
        assert!(out.provenance.privacy_decision.starts_with("PrivateHome/"));
        // A witness was produced and the belief is in the WorldGraph.
        assert_ne!(out.witness, [0u8; 32]);
        assert!(bridge.engine().world().node(out.semantic_id).is_some());
        // Both nodes are now wired as sensors.
        assert_eq!(bridge.registered_node_count(), 2);
    }

    #[test]
    fn live_path_is_deterministic() {
        let states = two_node_states_fixed();
        let run = || {
            let mut b = EngineBridge::new(PrivacyMode::PrivateHome, 1, "r", "R", None);
            b.process_cycle_from_states(&states, 5_000).unwrap().unwrap()
        };
        let a = run();
        let b = run();
        assert_eq!(a.witness, b.witness);
        assert_eq!(a.provenance.calibration_version, b.provenance.calibration_version);
        assert_eq!(a.effective_class, b.effective_class);
    }

    // Deterministic node states (no wall-clock in amplitude/history).
    fn two_node_states_fixed() -> HashMap<u8, NodeState> {
        let mut m = HashMap::new();
        for (id, amp) in [(0u8, 1.0_f64), (1u8, 1.05)] {
            let mut ns = NodeState::new();
            ns.frame_history = VecDeque::from(vec![(0..56)
                .map(|i| amp + 0.1 * i as f64)
                .collect::<Vec<f64>>()]);
            ns.last_frame_time = Some(Instant::now());
            m.insert(id, ns);
        }
        m
    }

    #[test]
    fn nodes_registered_once_across_cycles() {
        let mut bridge = EngineBridge::new(PrivacyMode::PrivateHome, 1, "r", "R", None);
        let states = two_node_states();
        bridge.process_cycle_from_states(&states, 1_000);
        bridge.process_cycle_from_states(&states, 2_000);
        bridge.process_cycle_from_states(&states, 3_000);
        // Still exactly two sensors — idempotent registration.
        assert_eq!(bridge.registered_node_count(), 2);
    }

    #[test]
    fn retention_bounds_world_graph_growth() {
        let mut bridge = EngineBridge::new(PrivacyMode::PrivateHome, 1, "r", "R", None);
        bridge.set_semantic_retention(5);
        let states = two_node_states();
        for i in 0..20i64 {
            bridge.process_cycle_from_states(&states, 1_000 + i * 50);
        }
        // room + 2 sensors + at most 5 retained beliefs.
        assert!(bridge.engine().world().node_count() <= 3 + 5);
    }

    #[test]
    fn adapter_identity_flows_into_live_witness() {
        let states = two_node_states_fixed();
        let mut bridge = EngineBridge::new(PrivacyMode::PrivateHome, 1, "r", "R", None);
        let base = bridge
            .process_cycle_from_states(&states, 1_000)
            .unwrap()
            .unwrap();
        bridge.set_room_adapter(AdapterInfo {
            adapter_id: "deadbeefcafef00d".into(),
            trained_samples: 120,
        });
        let adapted = bridge
            .process_cycle_from_states(&states, 2_000)
            .unwrap()
            .unwrap();
        assert!(adapted
            .provenance
            .model_version
            .ends_with("+adapter:deadbeefcafef00d"));
        assert_ne!(adapted.witness, base.witness);
        // Clearing reverts to the base model identity.
        bridge.clear_room_adapter();
        let back = bridge
            .process_cycle_from_states(&states, 3_000)
            .unwrap()
            .unwrap();
        assert_eq!(back.provenance.model_version, "rfenc-v1");
    }

    /// Wiring (review finding 1): a live frame in → trust state recorded on
    /// the bridge (witness, effective class, recalibration flag), readable by
    /// the status endpoint, with a zero error count on the happy path.
    #[test]
    fn observe_cycle_records_trust_state() {
        let mut bridge = EngineBridge::new(PrivacyMode::PrivateHome, 1, "r", "R", None);
        assert!(bridge.last_trust_witness().is_none());
        assert_eq!(bridge.effective_class(), None);

        let out = bridge
            .observe_cycle(&two_node_states(), 1_000)
            .expect("two fresh nodes → governed cycle runs");

        assert_eq!(bridge.last_trust_witness(), Some(out.witness));
        assert_eq!(bridge.effective_class(), Some(out.effective_class));
        assert_eq!(
            bridge.recalibration_recommended(),
            out.recalibration_recommended
        );
        assert_eq!(bridge.demoted(), out.demoted);
        assert_eq!(bridge.engine_error_count(), 0);
        // PrivateHome clean cycle → Anonymous → raw outputs NOT suppressed.
        assert_eq!(bridge.effective_class(), Some(PrivacyClass::Anonymous));
        assert!(!bridge.suppress_raw_outputs());
    }

    /// Error-result accounting is independent from live cohort selection, so
    /// a future engine error remains auditable without deliberately forwarding
    /// temporally incoherent frames through the production bridge.
    #[test]
    fn observe_cycle_counts_engine_errors() {
        let mut bridge = EngineBridge::new(PrivacyMode::PrivateHome, 1, "r", "R", None);
        let error = || {
            EngineError::Fusion(
                wifi_densepose_signal::ruvsense::multistatic::MultistaticError::TimestampMismatch {
                    spread_us: 500_000,
                    guard_us: 60_000,
                },
            )
        };

        assert!(bridge.record_cycle_result(Err(error())).is_none());
        assert_eq!(bridge.engine_error_count(), 1);
        assert!(
            bridge.last_trust_witness().is_none(),
            "no witness from a failed cycle"
        );

        assert!(bridge.record_cycle_result(Err(error())).is_none());
        assert_eq!(bridge.engine_error_count(), 2);

        // A later good cycle records trust state; the audit count is kept.
        let out = bridge.observe_cycle(&two_node_states(), 3_000);
        assert!(out.is_some());
        assert!(bridge.last_trust_witness().is_some());
        assert_eq!(bridge.engine_error_count(), 2);

        // And a subsequent failure keeps the last good witness readable.
        assert!(bridge.record_cycle_result(Err(error())).is_none());
        assert_eq!(bridge.engine_error_count(), 3);
        assert!(bridge.last_trust_witness().is_some());
    }

    #[test]
    fn governed_cycles_prune_slow_mixed_width_nodes_without_errors() {
        let mut bridge = EngineBridge::new(PrivacyMode::PrivateHome, 1, "r", "R", None);

        for tick in 0..50_i64 {
            let now = Instant::now();
            let mut states = HashMap::new();
            for (node_id, age_ms, n_sub) in [
                (1, 0, 64),
                (3, 10, 256),
                (4, 50, 64),
                (7, 1_000, 256),
            ] {
                let mut node = node_state_with_history(1.0 + node_id as f64 * 0.01, n_sub);
                node.last_frame_time = Some(now - Duration::from_millis(age_ms));
                states.insert(node_id, node);
            }

            assert!(
                bridge.observe_cycle(&states, 1_000 + tick * 50).is_some(),
                "governed cycle {tick}"
            );
        }

        assert_eq!(bridge.engine_error_count(), 0);
        assert!(bridge.last_trust_witness().is_some());
    }

    /// ADR-141 mapping (review finding 1c): a cycle emitted at class
    /// Restricted flips `suppress_raw_outputs`, which `main.rs` uses to strip
    /// per-node raw amplitude vectors from the live publish — the same field
    /// mapping bfld's privacy gate applies at `Restricted`.
    #[test]
    fn restricted_class_suppresses_raw_outputs() {
        let mut bridge = EngineBridge::new(PrivacyMode::PrivateHome, 1, "r", "R", None);
        bridge.set_privacy_mode(PrivacyMode::StrictNoIdentity); // base = Restricted
        bridge
            .observe_cycle(&two_node_states(), 1_000)
            .expect("cycle runs");
        assert_eq!(bridge.effective_class(), Some(PrivacyClass::Restricted));
        assert!(bridge.suppress_raw_outputs());
    }

    /// A demotion must name its trigger. The measured failure this guards
    /// against: on a three-node ESP32 array the 20 ms default soft guard sat
    /// below the inter-node frame-arrival spread, so 45.9% of cycles were
    /// demoted by `TimestampMismatch` and lost their raw amplitude vectors —
    /// with nothing on any surface saying why.
    #[test]
    fn timestamp_mismatch_demotion_is_named() {
        let mut bridge = EngineBridge::new(PrivacyMode::PrivateHome, 1, "r", "R", None);
        let now = Instant::now();
        let mut states = two_node_states();
        // 25 ms spread: above the 20 ms soft guard, well inside the 60 ms hard one.
        states.get_mut(&0).expect("node 0").last_frame_time = Some(now - Duration::from_millis(25));
        states.get_mut(&1).expect("node 1").last_frame_time = Some(now);

        let out = bridge.observe_cycle(&states, 1_000).expect("cycle runs");
        assert!(out.demoted, "a 25 ms spread must exceed the 20 ms soft guard");
        assert!(
            bridge.demotion_reason().is_some_and(|r| r.contains("timestamp_mismatch")),
            "demotion must name the trigger, got {:?}",
            bridge.demotion_reason()
        );
        assert_eq!(bridge.demotion_count(), 1);
        assert!(bridge.suppress_raw_outputs());
    }

    /// A clean cycle must clear the reason again, so a consumer polling
    /// `/api/v1/status` never reads a stale cause.
    #[test]
    fn a_clean_cycle_clears_the_demotion_reason() {
        let mut bridge = EngineBridge::new(PrivacyMode::PrivateHome, 1, "r", "R", None);
        let now = Instant::now();
        let mut states = two_node_states();
        states.get_mut(&0).expect("node 0").last_frame_time = Some(now - Duration::from_millis(25));
        states.get_mut(&1).expect("node 1").last_frame_time = Some(now);
        bridge.observe_cycle(&states, 1_000).expect("cycle runs");
        assert!(bridge.demotion_reason().is_some());

        // Both nodes now arrive together, inside the soft guard.
        let now = Instant::now();
        let mut aligned = two_node_states();
        for id in [0u8, 1u8] {
            aligned.get_mut(&id).expect("node").last_frame_time = Some(now);
        }
        let out = bridge.observe_cycle(&aligned, 2_000).expect("cycle runs");
        assert!(!out.demoted, "aligned arrivals must not demote");
        assert_eq!(bridge.demotion_reason(), None);
        assert_eq!(bridge.demotion_count(), 1, "the counter is cumulative");
    }

    #[test]
    fn identity_strict_mode_is_carried_into_provenance() {
        let mut bridge = EngineBridge::new(PrivacyMode::PrivateHome, 1, "r", "R", None);
        bridge.set_privacy_mode(PrivacyMode::StrictNoIdentity);
        let out = bridge
            .process_cycle_from_states(&two_node_states(), 7_000)
            .unwrap()
            .unwrap();
        assert!(out.provenance.privacy_decision.starts_with("StrictNoIdentity/"));
        // Effective class is a valid privacy class (sanity).
        let _ = matches!(
            out.effective_class,
            PrivacyClass::Raw | PrivacyClass::Derived | PrivacyClass::Anonymous | PrivacyClass::Restricted
        );
    }
}
