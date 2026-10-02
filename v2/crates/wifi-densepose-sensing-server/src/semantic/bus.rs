//! Semantic event bus — dispatches one [`RawSnapshot`] to every
//! primitive in the order they were registered, collects the
//! [`SemanticEvent`]s emitted, and hands them to MQTT + Matter
//! publishers via a shared `tokio::broadcast` (wiring lives in the
//! publisher, see `mqtt::publisher`).
//!
//! Per §3.12.6 — adding a new primitive is one file change. The bus
//! holds a list of trait objects so the call site doesn't grow when we
//! add primitives in P4.5b.

use super::common::{PrimitiveConfig, PrimitiveState, RawSnapshot};
#[cfg(test)]
use super::common::Reason;
use super::{
    bathroom::BathroomOccupied,
    bed_exit::BedExit,
    distress::PossibleDistress,
    elderly_anomaly::ElderlyInactivityAnomaly,
    fall_risk::FallRiskElevated,
    meeting::MeetingInProgress,
    multi_room::MultiRoomTransition,
    no_movement::NoMovement,
    room_active::RoomActive,
    sleeping::SomeoneSleeping,
};

/// Identifier for which primitive produced an event. Used by the
/// publisher to map onto the matching `EntityKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticKind {
    SomeoneSleeping,
    PossibleDistress,
    RoomActive,
    ElderlyAnomaly,
    Meeting,
    BathroomOccupied,
    FallRisk,
    BedExit,
    NoMovement,
    MultiRoom,
}

/// One event published to MQTT / Matter consumers.
#[derive(Debug, Clone, PartialEq)]
pub struct SemanticEvent {
    pub kind: SemanticKind,
    pub state: PrimitiveState,
    pub node_id: String,
    pub timestamp_ms: i64,
}

/// Collection of every primitive FSM. Owned by the publisher task.
pub struct SemanticBus {
    sleeping: SomeoneSleeping,
    distress: PossibleDistress,
    room_active: RoomActive,
    elderly_anomaly: ElderlyInactivityAnomaly,
    meeting: MeetingInProgress,
    bathroom: BathroomOccupied,
    fall_risk: FallRiskElevated,
    bed_exit: BedExit,
    no_movement: NoMovement,
    multi_room: MultiRoomTransition,
    pub config: PrimitiveConfig,
}

impl SemanticBus {
    pub fn new(config: PrimitiveConfig) -> Self {
        Self {
            sleeping: SomeoneSleeping::new(),
            distress: PossibleDistress::new(),
            room_active: RoomActive::new(),
            elderly_anomaly: ElderlyInactivityAnomaly::new(),
            meeting: MeetingInProgress::new(),
            bathroom: BathroomOccupied::new(),
            fall_risk: FallRiskElevated::new(),
            bed_exit: BedExit::new(),
            no_movement: NoMovement::new(),
            multi_room: MultiRoomTransition::new(),
            config,
        }
    }

    /// Current state of a boolean primitive, so the publisher can report it
    /// on a heartbeat and not only on the tick it changed. `None` for the
    /// scalar and event primitives.
    pub fn is_active(&self, kind: SemanticKind) -> Option<bool> {
        Some(match kind {
            SemanticKind::SomeoneSleeping => self.sleeping.active,
            SemanticKind::PossibleDistress => self.distress.active,
            SemanticKind::RoomActive => self.room_active.active,
            SemanticKind::ElderlyAnomaly => self.elderly_anomaly.active,
            SemanticKind::Meeting => self.meeting.active,
            SemanticKind::BathroomOccupied => self.bathroom.active,
            SemanticKind::NoMovement => self.no_movement.active,
            SemanticKind::FallRisk | SemanticKind::BedExit | SemanticKind::MultiRoom => {
                return None
            }
        })
    }

    /// Latest fall-risk score (0–100). Zero until the first post-warmup tick.
    pub fn fall_risk_score(&self) -> f64 {
        self.fall_risk.last_score
    }

    /// Run all primitives on one snapshot. Returns only events that
    /// emit (Idle states are filtered).
    pub fn tick(&mut self, snap: &RawSnapshot) -> Vec<SemanticEvent> {
        let pairs: [(SemanticKind, PrimitiveState); 10] = [
            (SemanticKind::SomeoneSleeping,   self.sleeping.tick(snap, &self.config)),
            (SemanticKind::PossibleDistress,  self.distress.tick(snap, &self.config)),
            (SemanticKind::RoomActive,        self.room_active.tick(snap, &self.config)),
            (SemanticKind::ElderlyAnomaly,    self.elderly_anomaly.tick(snap, &self.config)),
            (SemanticKind::Meeting,           self.meeting.tick(snap, &self.config)),
            (SemanticKind::BathroomOccupied,  self.bathroom.tick(snap, &self.config)),
            (SemanticKind::FallRisk,          self.fall_risk.tick(snap, &self.config)),
            (SemanticKind::BedExit,           self.bed_exit.tick(snap, &self.config)),
            (SemanticKind::NoMovement,        self.no_movement.tick(snap, &self.config)),
            (SemanticKind::MultiRoom,         self.multi_room.tick(snap, &self.config)),
        ];
        pairs
            .into_iter()
            .filter_map(|(kind, state)| match state {
                PrimitiveState::Idle => None,
                _ => Some(SemanticEvent {
                    kind,
                    state,
                    node_id: snap.node_id.clone(),
                    timestamp_ms: snap.timestamp_ms,
                }),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn cfg() -> PrimitiveConfig {
        PrimitiveConfig::default()
    }

    #[test]
    fn bus_returns_empty_during_warmup() {
        let mut bus = SemanticBus::new(cfg());
        let snap = RawSnapshot {
            since_start: Duration::from_secs(30),
            presence: true,
            motion: 0.5,
            ..Default::default()
        };
        assert!(bus.tick(&snap).is_empty());
    }

    #[test]
    fn bus_emits_room_active_on_sustained_motion() {
        let mut bus = SemanticBus::new(cfg());
        let snap = RawSnapshot {
            node_id: "test".into(),
            since_start: Duration::from_secs(120),
            timestamp_ms: 1_000,
            presence: true,
            motion: 0.4,
            ..Default::default()
        };
        let events = bus.tick(&snap);
        assert!(events.iter().any(|e| e.kind == SemanticKind::RoomActive));
    }

    #[test]
    fn bus_emits_bathroom_when_zone_active() {
        let mut bus = SemanticBus::new(cfg());
        let snap = RawSnapshot {
            node_id: "test".into(),
            since_start: Duration::from_secs(120),
            timestamp_ms: 1_000,
            presence: true,
            active_zones: vec!["bathroom".into()],
            ..Default::default()
        };
        let events = bus.tick(&snap);
        assert!(events.iter().any(|e| e.kind == SemanticKind::BathroomOccupied));
    }

    #[test]
    fn bus_supports_multiple_simultaneous_primitives() {
        let mut bus = SemanticBus::new(cfg());
        let snap = RawSnapshot {
            node_id: "test".into(),
            since_start: Duration::from_secs(120),
            timestamp_ms: 1_000,
            presence: true,
            motion: 0.4,
            active_zones: vec!["bathroom".into()],
            ..Default::default()
        };
        let events = bus.tick(&snap);
        // Both RoomActive AND BathroomOccupied should fire.
        let kinds: Vec<_> = events.iter().map(|e| e.kind).collect();
        assert!(kinds.contains(&SemanticKind::RoomActive));
        assert!(kinds.contains(&SemanticKind::BathroomOccupied));
    }

    #[test]
    fn semantic_event_carries_node_id_and_ts() {
        let mut bus = SemanticBus::new(cfg());
        let snap = RawSnapshot {
            node_id: "aabb".into(),
            since_start: Duration::from_secs(120),
            timestamp_ms: 1779_512_400_000,
            presence: true,
            active_zones: vec!["bathroom".into()],
            ..Default::default()
        };
        let events = bus.tick(&snap);
        let bath = events.into_iter().find(|e| e.kind == SemanticKind::BathroomOccupied).unwrap();
        assert_eq!(bath.node_id, "aabb");
        assert_eq!(bath.timestamp_ms, 1779_512_400_000);
    }

    #[test]
    fn semantic_event_includes_explanation_reason() {
        // Verify that primitives populate the explanation field —
        // critical for HA users debugging automations.
        let mut bus = SemanticBus::new(cfg());
        let snap = RawSnapshot {
            node_id: "test".into(),
            since_start: Duration::from_secs(120),
            timestamp_ms: 1_000,
            presence: true,
            motion: 0.4,
            ..Default::default()
        };
        let events = bus.tick(&snap);
        let ra = events.into_iter().find(|e| e.kind == SemanticKind::RoomActive).unwrap();
        if let PrimitiveState::Boolean { reason, .. } = ra.state {
            assert!(!reason.tags.is_empty(), "reason tags must explain why primitive fired");
        } else {
            panic!("expected Boolean state");
        }
    }

    #[test]
    fn is_active_reflects_boolean_primitive_state() {
        let mut bus = SemanticBus::new(cfg());
        assert_eq!(bus.is_active(SemanticKind::RoomActive), Some(false));
        assert_eq!(bus.is_active(SemanticKind::FallRisk), None);
        let snap = RawSnapshot {
            node_id: "test".into(),
            since_start: Duration::from_secs(120),
            presence: true,
            motion: 0.4,
            ..Default::default()
        };
        bus.tick(&snap);
        assert_eq!(bus.is_active(SemanticKind::RoomActive), Some(true));
    }

    #[test]
    fn _unused_reason_helper_remains_constructible() {
        // Touch Reason::empty to keep clippy happy when the bus uses
        // it indirectly via primitives.
        let _ = Reason::empty();
    }

    // ─── Property-based invariants ─────────────────────────────────
    //
    // The example-based tests above hit the obvious FSM transitions.
    // These proptest cases throw random snapshot sequences at the bus
    // and assert no primitive panics, every emitted state carries a
    // reason payload, and the bus never returns Idle events (Idle is
    // explicitly filtered).

    use proptest::prelude::*;

    fn arb_snapshot() -> impl Strategy<Value = RawSnapshot> {
        // proptest only impls Strategy for tuples up to length 12, so
        // we split into two nested tuples and merge in the prop_map.
        let core = (
            0u64..86400,                             // since_start secs
            0i64..(1u64 << 40) as i64,               // timestamp_ms
            any::<bool>(),                           // presence
            any::<bool>(),                           // fall_detected
            -0.5f64..2.0,                            // motion (incl. out-of-range)
            -1000.0f64..10000.0,                     // motion_energy
            proptest::option::of(0.0f64..200.0),     // breathing_rate_bpm
        );
        let extra = (
            proptest::option::of(0.0f64..250.0),     // heart_rate_bpm
            0u32..10,                                // n_persons
            proptest::option::of(-120.0f64..0.0),    // rssi_dbm
            0.0f64..1.0,                             // vital_confidence
            0u32..86400,                             // local_seconds_since_midnight
            prop::collection::vec("[a-z]{3,8}", 0..4), // active_zones
        );
        (core, extra).prop_map(
            |((secs, ts, presence, fall, motion, energy, br),
              (hr, n, rssi, conf, tod, zones))| {
                RawSnapshot {
                    node_id: "fuzz".into(),
                    since_start: std::time::Duration::from_secs(secs),
                    timestamp_ms: ts,
                    presence,
                    fall_detected: fall,
                    motion,
                    motion_energy: energy,
                    breathing_rate_bpm: br,
                    heart_rate_bpm: hr,
                    n_persons: n,
                    rssi_dbm: rssi,
                    vital_confidence: conf,
                    active_zones: zones,
                    bed_zones: vec!["bedroom".into()],
                    local_seconds_since_midnight: tod,
                }
            },
        )
    }

    proptest! {
        /// The bus never panics on any single snapshot, even with
        /// pathological inputs (motion>1.0, NaN-prone HRs, empty
        /// zones, etc).
        #[test]
        fn bus_tick_never_panics_on_arbitrary_snapshot(snap in arb_snapshot()) {
            let mut bus = SemanticBus::new(PrimitiveConfig::default());
            let _events = bus.tick(&snap);
        }

        /// Every emitted SemanticEvent carries a populated `node_id`
        /// and the same `timestamp_ms` as the input snapshot. The bus
        /// MUST NOT manufacture events with empty node IDs.
        #[test]
        fn bus_events_carry_node_id_and_ts(snap in arb_snapshot()) {
            let mut bus = SemanticBus::new(PrimitiveConfig::default());
            for ev in bus.tick(&snap) {
                prop_assert!(!ev.node_id.is_empty(), "empty node_id in event {:?}", ev);
                prop_assert_eq!(ev.timestamp_ms, snap.timestamp_ms);
            }
        }

        /// No primitive emits a SemanticState::Boolean without
        /// populating its `reason` field — the explainability contract
        /// is enforced at the wire boundary.
        #[test]
        fn boolean_states_always_have_reason_tags(snap in arb_snapshot()) {
            let mut bus = SemanticBus::new(PrimitiveConfig::default());
            for ev in bus.tick(&snap) {
                match &ev.state {
                    PrimitiveState::Boolean { reason, changed, .. } => {
                        if *changed {
                            prop_assert!(
                                !reason.tags.is_empty(),
                                "changed Boolean must have reason tags: {:?}", ev,
                            );
                        }
                    }
                    _ => {}
                }
            }
        }

        /// A randomly-sequenced run of snapshots never makes the bus
        /// produce more events than primitives it owns (currently 10).
        /// This is the upper-bound invariant — each primitive emits at
        /// most one event per tick.
        #[test]
        fn per_tick_event_count_bounded_by_primitive_count(snap in arb_snapshot()) {
            let mut bus = SemanticBus::new(PrimitiveConfig::default());
            let events = bus.tick(&snap);
            prop_assert!(events.len() <= 10, "too many events: {}", events.len());
        }

        /// Replaying the same snapshot N times to a fresh bus produces
        /// monotonic / consistent state (no jitter). This catches FSMs
        /// that accidentally use uninitialised internal state.
        #[test]
        fn replay_same_snapshot_is_deterministic_per_fresh_bus(
            snap in arb_snapshot(),
            replays in 1usize..5,
        ) {
            let mut last: Option<Vec<SemanticKind>> = None;
            for _ in 0..replays {
                let mut bus = SemanticBus::new(PrimitiveConfig::default());
                let kinds: Vec<_> = bus.tick(&snap).into_iter().map(|e| e.kind).collect();
                if let Some(prev) = &last {
                    prop_assert_eq!(prev, &kinds, "non-deterministic tick from fresh bus");
                }
                last = Some(kinds);
            }
        }
    }
}
