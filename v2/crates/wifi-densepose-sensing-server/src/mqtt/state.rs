//! State payload encoding + rate limiting (ADR-115 §3.5 / §3.7).
//!
//! This module owns the translation from internal `sensing-server`
//! broadcast messages (`pose_data`, `edge_vitals`, `sensing_update`)
//! into the per-entity MQTT state-topic payloads consumed by Home
//! Assistant. It is gated behind the `mqtt` feature flag at the call
//! site, but the encoders and rate-limiter logic compile without any
//! network deps so they're testable under `--no-default-features`.
//!
//! Per ADR-115 §3.5, state-topic QoS / retain / cadence is:
//!
//! | Topic kind             | QoS | Retain | Cadence                |
//! |------------------------|-----|--------|------------------------|
//! | `sensor/*/state`       |  0  |   no   | rate-limited per §3.7  |
//! | `binary_sensor/*/state`|  1  |  yes   | on change only         |
//! | `event/*/state`        |  1  |   no   | on event               |
//! | `*/availability`       |  1  |  yes   | LWT + 30 s heartbeat   |
//!
//! Per ADR-115 §3.7, default rates are:
//!
//! - presence binary  : on change
//! - person count     : 1.0 Hz
//! - vitals (HR / BR) : 0.2 Hz (every 5 s)
//! - motion level     : 1.0 Hz
//! - fall events      : on event (no rate limit)
//! - RSSI             : 0.1 Hz
//! - pose             : 1.0 Hz when `--mqtt-publish-pose` (off by default)
//! - zones            : on change

use std::collections::HashMap;
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

use super::config::PublishRates;
use super::discovery::{DiscoveryComponent, EntityKind};

/// Encoded outbound MQTT publication. `topic` is fully-qualified
/// (already prefixed with the discovery namespace + node id). `payload`
/// is the UTF-8 string the broker should publish on that topic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateMessage {
    pub topic: String,
    pub payload: String,
    pub qos: u8,
    pub retain: bool,
}

impl StateMessage {
    pub fn new(topic: String, payload: String, component: DiscoveryComponent, is_change_only: bool) -> Self {
        let (qos, retain) = match component {
            DiscoveryComponent::BinarySensor => (1, is_change_only),
            DiscoveryComponent::Event => (1, false),
            DiscoveryComponent::Sensor => (0, false),
        };
        Self { topic, payload, qos, retain }
    }
}

/// Sample-rate-limit decisions, per `(node, entity)`. Tracks the
/// last-emitted instant for each entity *on each node* and gates further
/// emissions accordingly. Time is supplied by the caller so the limiter is
/// testable without a clock.
///
/// ADR-297 (issue #1541): the key is `(NodeId, EntityKind)`, not `EntityKind`
/// alone. With an entity-only key one node consumed the numeric publish slot
/// and every other node was suppressed until the interval expired — while
/// availability still reported them online. Keying by node keeps each node's
/// per-entity budget independent so nodes no longer starve one another.
#[derive(Debug, Default)]
pub struct RateLimiter {
    last: HashMap<(String, EntityKind), Duration>,
}

impl RateLimiter {
    /// Build a fresh limiter with no per-`(node, entity)` history.
    pub fn new() -> Self {
        Self { last: HashMap::new() }
    }

    /// Decide whether a sample for `entity` on node `node_id` is allowed to
    /// publish at `now`, given the configured `rates`. Returns true to publish
    /// (and updates last-emitted state); false to drop. Each node's budget for
    /// an entity is independent of every other node's (ADR-297).
    pub fn allow(
        &mut self,
        node_id: &str,
        entity: EntityKind,
        now: Duration,
        rates: &PublishRates,
    ) -> bool {
        let min_gap = match rate_hz_for(entity, rates) {
            // Zero / negative Hz → emit only on change (caller path).
            // Here we treat it as "always allow" because the caller is
            // already gating with change detection.
            rate if rate <= 0.0 => return true,
            rate => Duration::from_secs_f64(1.0 / rate),
        };
        // Borrow the key without allocating on the hot lookup path; only
        // allocate the owned `String` when inserting a new node/entity slot.
        if let Some(&prev) = self.last.get(&(node_id.to_string(), entity)) {
            if now.saturating_sub(prev) < min_gap {
                return false;
            }
        }
        self.last.insert((node_id.to_string(), entity), now);
        true
    }

    /// Reset all per-entity history. Used after a reconnect so the first
    /// post-reconnect sample is emitted promptly.
    pub fn reset(&mut self) {
        self.last.clear();
    }
}

/// Look up the configured Hz for an entity. Numerical entities use the
/// `rates` struct; non-rate-limited entities (events / change-only)
/// return 0.0 to short-circuit limiting.
fn rate_hz_for(entity: EntityKind, rates: &PublishRates) -> f64 {
    match entity {
        // Change-only / event entities — caller drives them.
        EntityKind::Presence
        | EntityKind::ZoneOccupancy
        | EntityKind::FallDetected
        | EntityKind::BedExit
        | EntityKind::MultiRoomTransition
        | EntityKind::SomeoneSleeping
        | EntityKind::PossibleDistress
        | EntityKind::RoomActive
        | EntityKind::ElderlyInactivityAnomaly
        | EntityKind::MeetingInProgress
        | EntityKind::BathroomOccupied
        | EntityKind::NoMovement => 0.0,
        // Rate-limited measurements.
        EntityKind::PersonCount => rates.count_hz,
        EntityKind::BreathingRate | EntityKind::HeartRate => rates.vitals_hz,
        EntityKind::MotionLevel | EntityKind::MotionEnergy => rates.motion_hz,
        EntityKind::PresenceScore => rates.motion_hz,
        EntityKind::Rssi => rates.rssi_hz,
        EntityKind::PoseKeypoints => rates.pose_hz,
        EntityKind::FallRiskElevated => rates.motion_hz,
    }
}

// ─── Per-entity state payload encoders ───────────────────────────────────

/// Inputs the encoder accepts. The caller (publisher loop) projects the
/// internal server broadcast into this struct so the encoder never
/// touches the original `serde_json::Value`s directly. Avoids leaking
/// the server's internal schema into ADR-115's wire format.
#[derive(Debug, Clone, Default)]
pub struct VitalsSnapshot {
    pub node_id: String,
    pub timestamp_ms: i64,
    pub presence: bool,
    /// `Some(true)` once per detected fall, `Some(false)` while the node's
    /// fall detector reports, `None` when the node has no fall source.
    pub fall_detected: Option<bool>,
    pub motion: f64,             // 0.0–1.0
    /// `None` when the frame carries no motion band power.
    pub motion_energy: Option<f64>,
    pub presence_score: f64,     // 0.0–1.0
    pub breathing_rate_bpm: Option<f64>,
    pub heartrate_bpm: Option<f64>,
    pub n_persons: u32,
    pub rssi_dbm: Option<f64>,
    pub vital_confidence: f64,   // 0.0–1.0
}

#[derive(Serialize, Debug)]
struct NumberWithConfidence {
    bpm: f64,
    confidence: f64,
    ts: String,
}

#[derive(Serialize, Debug)]
struct MotionStatePayload {
    level_pct: f64,
    ts: String,
}

#[derive(Serialize, Debug)]
struct EnergyStatePayload {
    energy: f64,
    ts: String,
}

#[derive(Serialize, Debug)]
struct CountStatePayload {
    n_persons: u32,
    ts: String,
}

#[derive(Serialize, Debug)]
struct PresenceScorePayload {
    score_pct: f64,
    ts: String,
}

#[derive(Serialize, Debug)]
struct RssiPayload {
    dbm: f64,
    ts: String,
}

#[derive(Serialize, Debug)]
struct ScorePayload {
    score: f64,
    ts: String,
}

#[derive(Serialize, Debug)]
struct FallEventPayload {
    event_type: &'static str,
    ts: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    confidence: Option<f64>,
}

/// Encoder bundle that knows how to render each entity's state payload
/// from a [`VitalsSnapshot`]. Operates on an existing [`DiscoveryBuilder`]
/// so topics are guaranteed to match what was advertised at discovery
/// time.
pub struct StateEncoder<'a> {
    pub builder: &'a super::discovery::DiscoveryBuilder<'a>,
}

impl<'a> StateEncoder<'a> {
    /// Build the binary state ("ON"/"OFF") topic + payload for the given
    /// boolean entity.
    pub fn boolean(&self, entity: EntityKind, on: bool) -> Option<StateMessage> {
        if !matches!(entity.component(), DiscoveryComponent::BinarySensor) {
            return None;
        }
        let topic = format!(
            "{}/{}/wifi_densepose_{}/{}/state",
            self.builder.discovery_prefix,
            entity.component().as_str(),
            self.builder.node_id,
            entity.topic_slug(),
        );
        let payload = if on { "ON" } else { "OFF" }.to_string();
        Some(StateMessage::new(topic, payload, entity.component(), true))
    }

    /// Numeric/measurement state encoder.
    pub fn numeric(&self, entity: EntityKind, snap: &VitalsSnapshot) -> Option<StateMessage> {
        if !matches!(entity.component(), DiscoveryComponent::Sensor) {
            return None;
        }
        let ts = iso_ts(snap.timestamp_ms);
        let payload_value: Value = match entity {
            EntityKind::PersonCount => serde_json::to_value(CountStatePayload {
                n_persons: snap.n_persons,
                ts: ts.clone(),
            }).ok()?,
            EntityKind::BreathingRate => {
                let bpm = snap.breathing_rate_bpm?;
                serde_json::to_value(NumberWithConfidence {
                    bpm,
                    confidence: snap.vital_confidence,
                    ts: ts.clone(),
                }).ok()?
            }
            EntityKind::HeartRate => {
                let bpm = snap.heartrate_bpm?;
                serde_json::to_value(NumberWithConfidence {
                    bpm,
                    confidence: snap.vital_confidence,
                    ts: ts.clone(),
                }).ok()?
            }
            EntityKind::MotionLevel => serde_json::to_value(MotionStatePayload {
                level_pct: (snap.motion.clamp(0.0, 1.0)) * 100.0,
                ts: ts.clone(),
            }).ok()?,
            EntityKind::MotionEnergy => serde_json::to_value(EnergyStatePayload {
                energy: snap.motion_energy?,
                ts: ts.clone(),
            }).ok()?,
            EntityKind::PresenceScore => serde_json::to_value(PresenceScorePayload {
                score_pct: snap.presence_score.clamp(0.0, 1.0) * 100.0,
                ts: ts.clone(),
            }).ok()?,
            EntityKind::Rssi => {
                let dbm = snap.rssi_dbm?;
                serde_json::to_value(RssiPayload { dbm, ts: ts.clone() }).ok()?
            }
            _ => return None,
        };
        let topic = format!(
            "{}/{}/wifi_densepose_{}/{}/state",
            self.builder.discovery_prefix,
            entity.component().as_str(),
            self.builder.node_id,
            entity.topic_slug(),
        );
        let payload = serde_json::to_string(&payload_value).ok()?;
        Some(StateMessage::new(topic, payload, DiscoveryComponent::Sensor, false))
    }

    /// 0–100 score sensor (fall risk), matching the `{{ value_json.score }}`
    /// discovery template.
    pub fn score(&self, entity: EntityKind, value: f64, ts_ms: i64) -> Option<StateMessage> {
        if !matches!(entity.component(), DiscoveryComponent::Sensor) || !value.is_finite() {
            return None;
        }
        let payload = serde_json::to_string(&ScorePayload {
            score: value.clamp(0.0, 100.0),
            ts: iso_ts(ts_ms),
        })
        .ok()?;
        let topic = format!(
            "{}/{}/wifi_densepose_{}/{}/state",
            self.builder.discovery_prefix,
            entity.component().as_str(),
            self.builder.node_id,
            entity.topic_slug(),
        );
        Some(StateMessage::new(
            topic,
            payload,
            DiscoveryComponent::Sensor,
            false,
        ))
    }

    /// One-shot event encoder. Used for fall, bed exit, multi-room
    /// transition.
    pub fn event(&self, entity: EntityKind, event_type: &'static str, ts_ms: i64, confidence: Option<f64>) -> Option<StateMessage> {
        if !matches!(entity.component(), DiscoveryComponent::Event) {
            return None;
        }
        let payload_json = FallEventPayload { event_type, ts: iso_ts(ts_ms), confidence };
        let payload = serde_json::to_string(&payload_json).ok()?;
        let topic = format!(
            "{}/{}/wifi_densepose_{}/{}/state",
            self.builder.discovery_prefix,
            entity.component().as_str(),
            self.builder.node_id,
            entity.topic_slug(),
        );
        Some(StateMessage::new(topic, payload, DiscoveryComponent::Event, false))
    }
}

fn iso_ts(ms: i64) -> String {
    // Avoid pulling chrono into a hot path: format manually as ISO-8601
    // UTC. chrono is already in the crate's deps, but we keep this
    // encoder allocation-light for benchmark numbers.
    let secs = ms / 1000;
    let nanos = ((ms % 1000) * 1_000_000) as u32;
    let dt = chrono::DateTime::<chrono::Utc>::from_timestamp(secs, nanos)
        .unwrap_or_else(|| chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).unwrap());
    dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mqtt::discovery::DiscoveryBuilder;

    fn builder() -> DiscoveryBuilder<'static> {
        DiscoveryBuilder {
            discovery_prefix: "homeassistant",
            node_id: "aabbccddeeff",
            node_friendly_name: Some("Bedroom"),
            sw_version: "v0.7.0",
            model: "ESP32-S3 CSI node",
            via_device: None,
        }
    }

    fn rates() -> PublishRates {
        PublishRates {
            vitals_hz: 0.2,
            motion_hz: 1.0,
            count_hz: 1.0,
            rssi_hz: 0.1,
            pose_hz: 1.0,
        }
    }

    fn snap() -> VitalsSnapshot {
        VitalsSnapshot {
            node_id: "aabbccddeeff".into(),
            timestamp_ms: 1779_512_400_000,
            presence: true,
            fall_detected: Some(false),
            motion: 0.35,
            motion_energy: Some(1234.5),
            presence_score: 0.91,
            breathing_rate_bpm: Some(14.2),
            heartrate_bpm: Some(68.2),
            n_persons: 1,
            rssi_dbm: Some(-52.0),
            vital_confidence: 0.87,
        }
    }

    // ─── Rate limiter ────────────────────────────────────────────────

    const NODE: &str = "node-a";

    #[test]
    fn rate_limiter_first_sample_always_passes() {
        let mut rl = RateLimiter::new();
        assert!(rl.allow(NODE, EntityKind::HeartRate, Duration::ZERO, &rates()));
    }

    #[test]
    fn rate_limiter_drops_within_gap() {
        let mut rl = RateLimiter::new();
        let r = rates();
        // 0.2 Hz → 5 s gap.
        assert!(rl.allow(NODE, EntityKind::HeartRate, Duration::from_secs(0), &r));
        assert!(!rl.allow(NODE, EntityKind::HeartRate, Duration::from_secs(1), &r));
        assert!(!rl.allow(NODE, EntityKind::HeartRate, Duration::from_secs(4), &r));
    }

    #[test]
    fn rate_limiter_allows_after_gap() {
        let mut rl = RateLimiter::new();
        let r = rates();
        assert!(rl.allow(NODE, EntityKind::HeartRate, Duration::from_secs(0), &r));
        // 5 s gap met → allow.
        assert!(rl.allow(NODE, EntityKind::HeartRate, Duration::from_secs(5), &r));
    }

    #[test]
    fn rate_limiter_per_entity_independent() {
        let mut rl = RateLimiter::new();
        let r = rates();
        assert!(rl.allow(NODE, EntityKind::HeartRate, Duration::from_secs(0), &r));
        // Different entity, same instant → independent budget.
        assert!(rl.allow(NODE, EntityKind::MotionLevel, Duration::from_secs(0), &r));
    }

    #[test]
    fn rate_limiter_change_only_entities_always_allow() {
        let mut rl = RateLimiter::new();
        let r = rates();
        // Presence is change-only → rate=0 → unlimited; caller does change detection.
        for s in 0..3 {
            assert!(rl.allow(NODE, EntityKind::Presence, Duration::from_secs(s), &r));
        }
    }

    #[test]
    fn rate_limiter_reset_re_enables_immediate_publish() {
        let mut rl = RateLimiter::new();
        let r = rates();
        assert!(rl.allow(NODE, EntityKind::HeartRate, Duration::from_secs(0), &r));
        assert!(!rl.allow(NODE, EntityKind::HeartRate, Duration::from_secs(1), &r));
        rl.reset();
        // Post-reset: first sample passes.
        assert!(rl.allow(NODE, EntityKind::HeartRate, Duration::from_secs(1), &r));
    }

    #[test]
    fn rate_limiter_nodes_do_not_starve_each_other() {
        // ADR-297 (issue #1541): with an entity-only key, node-a consuming the
        // slot suppressed node-b. Keyed by (node, entity), both publish.
        let mut rl = RateLimiter::new();
        let r = rates();
        assert!(rl.allow("node-a", EntityKind::PersonCount, Duration::from_secs(0), &r));
        assert!(
            rl.allow("node-b", EntityKind::PersonCount, Duration::from_secs(0), &r),
            "second node must not be starved by the first"
        );
        // Each node still rate-limits itself.
        assert!(!rl.allow("node-a", EntityKind::PersonCount, Duration::from_millis(100), &r));
        assert!(!rl.allow("node-b", EntityKind::PersonCount, Duration::from_millis(100), &r));
    }

    // ─── Boolean / binary_sensor encoder ─────────────────────────────

    #[test]
    fn boolean_encoder_emits_on_off_payload() {
        let b = builder();
        let enc = StateEncoder { builder: &b };
        let on = enc.boolean(EntityKind::Presence, true).unwrap();
        assert_eq!(on.payload, "ON");
        assert_eq!(on.qos, 1);
        assert!(on.retain, "binary_sensor state must be retained per §3.5");
        let off = enc.boolean(EntityKind::Presence, false).unwrap();
        assert_eq!(off.payload, "OFF");
    }

    #[test]
    fn boolean_encoder_rejects_non_binary_entities() {
        let b = builder();
        let enc = StateEncoder { builder: &b };
        assert!(enc.boolean(EntityKind::HeartRate, true).is_none());
        assert!(enc.boolean(EntityKind::FallDetected, true).is_none());
    }

    #[test]
    fn boolean_topic_matches_discovery_state_topic() {
        let b = builder();
        let enc = StateEncoder { builder: &b };
        let msg = enc.boolean(EntityKind::Presence, true).unwrap();
        assert_eq!(
            msg.topic,
            "homeassistant/binary_sensor/wifi_densepose_aabbccddeeff/presence/state"
        );
    }

    // ─── Numeric / sensor encoder ────────────────────────────────────

    #[test]
    fn numeric_encoder_emits_bpm_payload_for_heart_rate() {
        let b = builder();
        let enc = StateEncoder { builder: &b };
        let s = snap();
        let msg = enc.numeric(EntityKind::HeartRate, &s).unwrap();
        let json: serde_json::Value = serde_json::from_str(&msg.payload).unwrap();
        assert_eq!(json["bpm"], 68.2);
        assert_eq!(json["confidence"], 0.87);
        assert_eq!(msg.qos, 0, "sensor state is QoS 0 per §3.5");
        assert!(!msg.retain);
    }

    #[test]
    fn numeric_encoder_emits_motion_percent_payload() {
        let b = builder();
        let enc = StateEncoder { builder: &b };
        let s = snap();
        let msg = enc.numeric(EntityKind::MotionLevel, &s).unwrap();
        let json: serde_json::Value = serde_json::from_str(&msg.payload).unwrap();
        // 0.35 → 35.0%
        assert_eq!(json["level_pct"], 35.0);
    }

    #[test]
    fn numeric_encoder_returns_none_when_optional_field_missing() {
        let b = builder();
        let enc = StateEncoder { builder: &b };
        let mut s = snap();
        s.heartrate_bpm = None;
        assert!(enc.numeric(EntityKind::HeartRate, &s).is_none());
    }

    #[test]
    fn numeric_encoder_has_no_motion_energy_without_a_source() {
        let b = builder();
        let enc = StateEncoder { builder: &b };
        let mut s = snap();
        s.motion_energy = None;
        assert!(
            enc.numeric(EntityKind::MotionEnergy, &s).is_none(),
            "never a default 0"
        );
    }

    #[test]
    fn score_encoder_matches_discovery_template() {
        let b = builder();
        let enc = StateEncoder { builder: &b };
        let msg = enc.score(EntityKind::FallRiskElevated, 142.0, 0).unwrap();
        let json: serde_json::Value = serde_json::from_str(&msg.payload).unwrap();
        assert_eq!(json["score"], 100.0, "clamped to 0..100");
        assert!(msg.topic.ends_with("/fall_risk_elevated/state"));
        assert!(enc
            .score(EntityKind::FallRiskElevated, f64::NAN, 0)
            .is_none());
        assert!(enc.score(EntityKind::Presence, 1.0, 0).is_none());
    }

    #[test]
    fn numeric_encoder_clamps_out_of_range_motion() {
        let b = builder();
        let enc = StateEncoder { builder: &b };
        let mut s = snap();
        s.motion = 1.7; // pathological — clamp to 1.0 then ×100.
        let msg = enc.numeric(EntityKind::MotionLevel, &s).unwrap();
        let json: serde_json::Value = serde_json::from_str(&msg.payload).unwrap();
        assert_eq!(json["level_pct"], 100.0);
    }

    #[test]
    fn numeric_encoder_rejects_non_sensor_entities() {
        let b = builder();
        let enc = StateEncoder { builder: &b };
        let s = snap();
        assert!(enc.numeric(EntityKind::Presence, &s).is_none());
        assert!(enc.numeric(EntityKind::FallDetected, &s).is_none());
    }

    // ─── Event encoder ───────────────────────────────────────────────

    #[test]
    fn event_encoder_emits_fall_payload() {
        let b = builder();
        let enc = StateEncoder { builder: &b };
        let msg = enc
            .event(EntityKind::FallDetected, "fall_detected", 1779_512_400_000, Some(0.87))
            .unwrap();
        let json: serde_json::Value = serde_json::from_str(&msg.payload).unwrap();
        assert_eq!(json["event_type"], "fall_detected");
        assert_eq!(json["confidence"], 0.87);
        assert_eq!(msg.qos, 1);
        assert!(!msg.retain, "events must never be retained — HA would replay old falls");
    }

    #[test]
    fn event_encoder_omits_confidence_when_absent() {
        let b = builder();
        let enc = StateEncoder { builder: &b };
        let msg = enc
            .event(EntityKind::BedExit, "bed_exit", 1779_512_400_000, None)
            .unwrap();
        assert!(!msg.payload.contains("confidence"));
    }

    #[test]
    fn event_encoder_rejects_non_event_entities() {
        let b = builder();
        let enc = StateEncoder { builder: &b };
        assert!(enc.event(EntityKind::Presence, "x", 0, None).is_none());
        assert!(enc.event(EntityKind::HeartRate, "x", 0, None).is_none());
    }

    #[test]
    fn iso_ts_is_rfc3339_utc_with_millis() {
        let ts = iso_ts(1779_512_400_000);
        assert!(ts.ends_with("Z"));
        assert!(ts.contains("T"));
        // .000 suffix from `SecondsFormat::Millis`.
        assert!(ts.contains("."), "want millisecond fraction in: {}", ts);
    }
}
