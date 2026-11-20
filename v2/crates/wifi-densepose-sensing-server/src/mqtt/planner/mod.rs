//! Entity → state publication plan (ADR-115 §3.5–§3.7, §3.12; issues
//! #2085, #2093).
//!
//! The planner turns each [`VitalsSnapshot`] into the MQTT messages to
//! publish: per-entity availability, change-gated binary states, rate-limited
//! sensor values, one-shot events, and the semantic primitives (HA-MIND,
//! ADR-115 P4.5). It is pure — no broker, no clock — so the whole mapping is
//! unit-testable; the publisher only sends what it returns.
//!
//! An entity is published only while the server has a real source for it.
//! Otherwise its availability topic carries `offline` and no state is sent,
//! so Home Assistant shows "unavailable" rather than a made-up value.

use std::collections::HashMap;
use std::time::Duration;

use crate::semantic::{PrimitiveConfig, RawSnapshot, SemanticBus, SemanticKind};

use super::config::{MqttConfig, PublishRates};
use super::discovery::{DiscoveryBuilder, DiscoveryComponent, EntityKind};
use super::privacy::{decide, PublishDecision};
use super::state::{RateLimiter, StateEncoder, StateMessage, VitalsSnapshot};

/// Unchanged binary states are re-published at this interval (retained), so
/// a broker that lost its retained copy recovers without a state flip.
pub const BINARY_STATE_HEARTBEAT: Duration = Duration::from_secs(60);

/// An optional input (vitals, RSSI, fall detector, motion energy) not seen
/// for this long no longer counts as a source; its entity goes unavailable.
pub const SOURCE_STALE_AFTER: Duration = Duration::from_secs(30);

/// Semantic FSMs are sampled at 1 Hz per node. Their constants (e.g. the
/// distress EWMA's "~100-sample memory at 1 Hz") assume that rate, while the
/// sensing broadcast runs at the CSI frame rate.
pub const SEMANTIC_TICK: Duration = Duration::from_secs(1);

/// On-change gate with heartbeat for retained binary states (#2093).
#[derive(Debug, Default)]
pub struct ChangeGate {
    last: HashMap<(String, EntityKind), (bool, Duration)>,
}

impl ChangeGate {
    /// True when `value` differs from the last published value for this
    /// `(node, entity)`, or `heartbeat` has elapsed since that publish.
    pub fn allow(
        &mut self,
        node_id: &str,
        entity: EntityKind,
        value: bool,
        now: Duration,
        heartbeat: Duration,
    ) -> bool {
        let key = (node_id.to_string(), entity);
        if let Some(&(prev, at)) = self.last.get(&key) {
            if prev == value && now.saturating_sub(at) < heartbeat {
                return false;
            }
        }
        self.last.insert(key, (value, now));
        true
    }

    pub fn reset(&mut self) {
        self.last.clear();
    }
}

struct NodePlan {
    bus: SemanticBus,
    last_tick: Option<Duration>,
    pending_fall: bool,
    sourced: HashMap<EntityKind, Duration>,
    availability: HashMap<EntityKind, bool>,
}

/// Per-publisher planning state, keyed by node.
pub struct PublishPlanner {
    entities: Vec<EntityKind>,
    rates: PublishRates,
    privacy_mode: bool,
    semantic: PrimitiveConfig,
    nodes: HashMap<String, NodePlan>,
    rate_limiter: RateLimiter,
    gate: ChangeGate,
}

impl PublishPlanner {
    pub fn new(cfg: &MqttConfig) -> Self {
        Self::with_semantic_config(cfg, PrimitiveConfig::default())
    }

    pub fn with_semantic_config(cfg: &MqttConfig, semantic: PrimitiveConfig) -> Self {
        Self {
            entities: DiscoveryBuilder::enabled_entities(cfg.privacy_mode, cfg.publish_pose, &[]),
            rates: cfg.rates,
            privacy_mode: cfg.privacy_mode,
            semantic,
            nodes: HashMap::new(),
            rate_limiter: RateLimiter::new(),
            gate: ChangeGate::default(),
        }
    }

    /// Entities announced via discovery. Same list drives state + availability.
    pub fn entities(&self) -> &[EntityKind] {
        &self.entities
    }

    /// Forget publish history after a reconnect so every state and
    /// availability is re-sent promptly.
    pub fn reset(&mut self) {
        self.rate_limiter.reset();
        self.gate.reset();
        for np in self.nodes.values_mut() {
            np.availability.clear();
        }
    }

    /// Messages for one snapshot. `now` is time since the publisher started
    /// (it is also the semantic warmup clock).
    pub fn on_snapshot(
        &mut self,
        b: &DiscoveryBuilder<'_>,
        snap: &VitalsSnapshot,
        now: Duration,
    ) -> Vec<StateMessage> {
        let node = snap.node_id.as_str();
        let np = self
            .nodes
            .entry(snap.node_id.clone())
            .or_insert_with(|| NodePlan {
                bus: SemanticBus::new(self.semantic.clone()),
                last_tick: None,
                pending_fall: false,
                sourced: HashMap::new(),
                availability: HashMap::new(),
            });

        for (entity, present) in [
            (EntityKind::MotionEnergy, snap.motion_energy.is_some()),
            (EntityKind::Rssi, snap.rssi_dbm.is_some()),
            (EntityKind::BreathingRate, snap.breathing_rate_bpm.is_some()),
            (EntityKind::HeartRate, snap.heartrate_bpm.is_some()),
            (EntityKind::FallDetected, snap.fall_detected.is_some()),
        ] {
            if present {
                np.sourced.insert(entity, now);
            }
        }

        np.pending_fall |= snap.fall_detected == Some(true);
        if np
            .last_tick
            .is_none_or(|t| now.saturating_sub(t) >= SEMANTIC_TICK)
        {
            let raw = raw_snapshot(snap, now, np.pending_fall);
            np.bus.tick(&raw);
            np.last_tick = Some(now);
            np.pending_fall = false;
        }

        let mut out = self.availability(b, node, true, now, false);
        let np = &self.nodes[node];
        let enc = StateEncoder { builder: b };
        for &e in &self.entities {
            if decide(e, self.privacy_mode) == PublishDecision::Suppress
                || !entity_available(np, e, now, &self.semantic)
            {
                continue;
            }
            match e.component() {
                DiscoveryComponent::BinarySensor => {
                    let on = match e {
                        EntityKind::Presence => Some(snap.presence),
                        _ => semantic_kind(e).and_then(|k| np.bus.is_active(k)),
                    };
                    if let Some(on) = on {
                        if self.gate.allow(node, e, on, now, BINARY_STATE_HEARTBEAT) {
                            out.extend(enc.boolean(e, on));
                        }
                    }
                }
                DiscoveryComponent::Event => {
                    if e == EntityKind::FallDetected && snap.fall_detected == Some(true) {
                        out.extend(enc.event(e, "fall_detected", snap.timestamp_ms, None));
                    }
                }
                DiscoveryComponent::Sensor => {
                    let msg = match e {
                        EntityKind::FallRiskElevated => {
                            enc.score(e, np.bus.fall_risk_score(), snap.timestamp_ms)
                        }
                        _ => enc.numeric(e, snap),
                    };
                    if let Some(m) = msg {
                        if self.rate_limiter.allow(node, e, now, &self.rates) {
                            out.push(m);
                        }
                    }
                }
            }
        }
        out
    }

    /// Availability messages for one node. With `force`, every entity is
    /// re-published (heartbeat); otherwise only transitions are. A node
    /// that is not `node_fresh` reports every entity `offline`.
    pub fn availability(
        &mut self,
        b: &DiscoveryBuilder<'_>,
        node_id: &str,
        node_fresh: bool,
        now: Duration,
        force: bool,
    ) -> Vec<StateMessage> {
        let Some(np) = self.nodes.get_mut(node_id) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for &e in &self.entities {
            let want = node_fresh && entity_available(np, e, now, &self.semantic);
            if force || np.availability.get(&e) != Some(&want) {
                np.availability.insert(e, want);
                out.push(availability_message(b, e, want));
            }
        }
        out
    }

    /// `offline` for every entity of a node (shutdown path).
    pub fn offline(&self, b: &DiscoveryBuilder<'_>) -> Vec<StateMessage> {
        self.entities
            .iter()
            .map(|&e| availability_message(b, e, false))
            .collect()
    }
}

fn availability_message(b: &DiscoveryBuilder<'_>, e: EntityKind, online: bool) -> StateMessage {
    StateMessage {
        topic: b.availability_topic(e),
        payload: if online { "online" } else { "offline" }.to_string(),
        qos: 1,
        retain: true,
    }
}

/// Whether the server currently has a real source for `e` on this node.
fn entity_available(np: &NodePlan, e: EntityKind, now: Duration, cfg: &PrimitiveConfig) -> bool {
    let fresh = |k: EntityKind| {
        np.sourced
            .get(&k)
            .is_some_and(|t| now.saturating_sub(*t) < SOURCE_STALE_AFTER)
    };
    let warm = now >= cfg.warmup;
    match e {
        EntityKind::Presence
        | EntityKind::PersonCount
        | EntityKind::MotionLevel
        | EntityKind::PresenceScore => true,
        EntityKind::MotionEnergy
        | EntityKind::Rssi
        | EntityKind::BreathingRate
        | EntityKind::HeartRate
        | EntityKind::FallDetected => fresh(e),
        EntityKind::RoomActive | EntityKind::NoMovement | EntityKind::ElderlyInactivityAnomaly => {
            warm
        }
        EntityKind::SomeoneSleeping => warm && fresh(EntityKind::BreathingRate),
        EntityKind::PossibleDistress => warm && fresh(EntityKind::HeartRate),
        EntityKind::FallRiskElevated => warm && fresh(EntityKind::FallDetected),
        // No source on the sensing broadcast (see EntityKind::has_server_source).
        EntityKind::ZoneOccupancy
        | EntityKind::PoseKeypoints
        | EntityKind::MeetingInProgress
        | EntityKind::BathroomOccupied
        | EntityKind::BedExit
        | EntityKind::MultiRoomTransition => false,
    }
}

fn semantic_kind(e: EntityKind) -> Option<SemanticKind> {
    Some(match e {
        EntityKind::SomeoneSleeping => SemanticKind::SomeoneSleeping,
        EntityKind::PossibleDistress => SemanticKind::PossibleDistress,
        EntityKind::RoomActive => SemanticKind::RoomActive,
        EntityKind::ElderlyInactivityAnomaly => SemanticKind::ElderlyAnomaly,
        EntityKind::MeetingInProgress => SemanticKind::Meeting,
        EntityKind::BathroomOccupied => SemanticKind::BathroomOccupied,
        EntityKind::NoMovement => SemanticKind::NoMovement,
        _ => return None,
    })
}

fn raw_snapshot(snap: &VitalsSnapshot, now: Duration, fall: bool) -> RawSnapshot {
    use chrono::Timelike;
    RawSnapshot {
        node_id: snap.node_id.clone(),
        since_start: now,
        timestamp_ms: snap.timestamp_ms,
        presence: snap.presence,
        fall_detected: fall,
        motion: snap.motion,
        motion_energy: snap.motion_energy.unwrap_or(0.0),
        breathing_rate_bpm: snap.breathing_rate_bpm,
        heart_rate_bpm: snap.heartrate_bpm,
        n_persons: snap.n_persons,
        rssi_dbm: snap.rssi_dbm,
        vital_confidence: snap.vital_confidence,
        // No zone source on the sensing broadcast; the zone-based primitives
        // are not announced (EntityKind::has_server_source).
        active_zones: Vec::new(),
        bed_zones: Vec::new(),
        local_seconds_since_midnight: chrono::Local::now().num_seconds_from_midnight(),
    }
}

#[cfg(test)]
mod tests;
