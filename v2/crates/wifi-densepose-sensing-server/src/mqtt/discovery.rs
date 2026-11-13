//! HA MQTT auto-discovery payload generators.
//!
//! Per ADR-115 §3.1 — §3.4 each RuView node becomes one HA `device` and
//! each capability (presence, person count, heart rate, breathing rate,
//! motion, fall, RSSI, zone occupancy, pose) becomes one entity on that
//! device. This module owns the JSON-serializable structures HA expects
//! on the `homeassistant/<component>/<object_id>/<entity>/config` topic.
//!
//! The structures are `Serialize`-only; we never need to parse them
//! back. Field names match Home Assistant's published MQTT-discovery
//! schema (https://www.home-assistant.io/integrations/mqtt/#mqtt-discovery)
//! pinned to the version the project tests against (v2025.5 as of this
//! ADR; bump in `docs/integrations/home-assistant.md` when the test
//! matrix moves).

use serde::Serialize;

use super::{MANUFACTURER, ORIGIN_NAME, SUPPORT_URL};
use crate::privacy_fields::FieldClass;

/// HA component kinds we publish today. Strings match the HA URL slug.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryComponent {
    BinarySensor,
    Sensor,
    Event,
}

impl DiscoveryComponent {
    pub fn as_str(self) -> &'static str {
        match self {
            DiscoveryComponent::BinarySensor => "binary_sensor",
            DiscoveryComponent::Sensor => "sensor",
            DiscoveryComponent::Event => "event",
        }
    }
}

/// Top-level HA discovery payload. Serialised to JSON and published
/// retained, QoS 1 on `<prefix>/<component>/<object_id>/<entity>/config`.
///
/// We only model the fields ADR-115 §3.3 examples touch. HA's schema has
/// many more optional fields; we add them on a per-entity-need basis to
/// keep payloads small (some retained brokers cap message size).
#[derive(Debug, Clone, Serialize)]
pub struct DiscoveryConfig {
    pub name: String,
    pub unique_id: String,
    pub object_id: String,
    pub state_topic: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availability_topic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_available: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_not_available: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_on: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_off: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit_of_measurement: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_template: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_attributes_topic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_types: Option<Vec<String>>,
    pub qos: u8,
    pub device: DeviceMeta,
    pub origin: OriginMeta,
}

/// HA `device` block. Multiple entities pointing at the same
/// `identifiers` are grouped into one device card in the HA UI.
#[derive(Debug, Clone, Serialize)]
pub struct DeviceMeta {
    pub identifiers: Vec<String>,
    pub name: String,
    pub manufacturer: String,
    pub model: String,
    pub sw_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub via_device: Option<String>,
}

/// HA `origin` block. Tells HA users which software emitted the entities.
#[derive(Debug, Clone, Serialize)]
pub struct OriginMeta {
    pub name: String,
    pub sw_version: String,
    pub support_url: String,
}

/// Per-entity availability payload. Used as MQTT LWT so the broker
/// publishes `offline` automatically if our connection drops.
#[derive(Debug, Clone)]
pub struct AvailabilityPayload {
    pub topic: String,
    pub online: &'static str,
    pub offline: &'static str,
}

impl AvailabilityPayload {
    pub fn for_entity(prefix: &str, component: DiscoveryComponent, node_id: &str, entity: &str) -> Self {
        Self {
            topic: format!(
                "{prefix}/{}/wifi_densepose_{node_id}/{entity}/availability",
                component.as_str()
            ),
            online: "online",
            offline: "offline",
        }
    }
}

/// All entity kinds RuView publishes via MQTT. Used by [`DiscoveryBuilder`]
/// to generate matching `config` and `state` topic strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntityKind {
    Presence,
    PersonCount,
    BreathingRate,
    HeartRate,
    MotionLevel,
    MotionEnergy,
    FallDetected,
    PresenceScore,
    Rssi,
    ZoneOccupancy,
    PoseKeypoints,
    // Semantic primitives (ADR-115 §3.12).
    SomeoneSleeping,
    PossibleDistress,
    RoomActive,
    ElderlyInactivityAnomaly,
    MeetingInProgress,
    BathroomOccupied,
    FallRiskElevated,
    BedExit,
    NoMovement,
    MultiRoomTransition,
}

impl EntityKind {
    pub fn topic_slug(self) -> &'static str {
        match self {
            EntityKind::Presence => "presence",
            EntityKind::PersonCount => "person_count",
            EntityKind::BreathingRate => "breathing_rate",
            EntityKind::HeartRate => "heart_rate",
            EntityKind::MotionLevel => "motion_level",
            EntityKind::MotionEnergy => "motion_energy",
            EntityKind::FallDetected => "fall",
            EntityKind::PresenceScore => "presence_score",
            EntityKind::Rssi => "rssi",
            EntityKind::ZoneOccupancy => "zone_occupancy",
            EntityKind::PoseKeypoints => "pose",
            EntityKind::SomeoneSleeping => "someone_sleeping",
            EntityKind::PossibleDistress => "possible_distress",
            EntityKind::RoomActive => "room_active",
            EntityKind::ElderlyInactivityAnomaly => "elderly_inactivity_anomaly",
            EntityKind::MeetingInProgress => "meeting_in_progress",
            EntityKind::BathroomOccupied => "bathroom_occupied",
            EntityKind::FallRiskElevated => "fall_risk_elevated",
            EntityKind::BedExit => "bed_exit",
            EntityKind::NoMovement => "no_movement",
            EntityKind::MultiRoomTransition => "multi_room_transition",
        }
    }

    pub fn component(self) -> DiscoveryComponent {
        match self {
            // Boolean states → binary_sensor.
            EntityKind::Presence
            | EntityKind::ZoneOccupancy
            | EntityKind::SomeoneSleeping
            | EntityKind::PossibleDistress
            | EntityKind::RoomActive
            | EntityKind::ElderlyInactivityAnomaly
            | EntityKind::MeetingInProgress
            | EntityKind::BathroomOccupied
            | EntityKind::NoMovement => DiscoveryComponent::BinarySensor,
            // One-shot triggers → event.
            EntityKind::FallDetected
            | EntityKind::BedExit
            | EntityKind::MultiRoomTransition => DiscoveryComponent::Event,
            // Numeric measurements → sensor.
            EntityKind::PersonCount
            | EntityKind::BreathingRate
            | EntityKind::HeartRate
            | EntityKind::MotionLevel
            | EntityKind::MotionEnergy
            | EntityKind::PresenceScore
            | EntityKind::Rssi
            | EntityKind::PoseKeypoints
            | EntityKind::FallRiskElevated => DiscoveryComponent::Sensor,
        }
    }

    /// Privacy class of this entity (#2165). The match has no wildcard, so
    /// a new entity doesn't compile until it is classified. Semantic states
    /// whose inputs include vital signs (sleeping reads breathing rate,
    /// possible distress reads heart rate) are biometric too.
    pub fn privacy_class(self) -> FieldClass {
        match self {
            EntityKind::BreathingRate
            | EntityKind::HeartRate
            | EntityKind::PoseKeypoints
            | EntityKind::SomeoneSleeping
            | EntityKind::PossibleDistress => FieldClass::Biometric,
            EntityKind::Presence
            | EntityKind::PersonCount
            | EntityKind::MotionLevel
            | EntityKind::MotionEnergy
            | EntityKind::FallDetected
            | EntityKind::PresenceScore
            | EntityKind::Rssi
            | EntityKind::ZoneOccupancy
            | EntityKind::RoomActive
            | EntityKind::ElderlyInactivityAnomaly
            | EntityKind::MeetingInProgress
            | EntityKind::BathroomOccupied
            | EntityKind::FallRiskElevated
            | EntityKind::BedExit
            | EntityKind::NoMovement
            | EntityKind::MultiRoomTransition => FieldClass::Public,
        }
    }

    /// True iff `--privacy-mode` must suppress this entity (ADR-115 §3.10).
    pub fn is_biometric(self) -> bool {
        self.privacy_class() != FieldClass::Public
    }

    /// False for entities the sensing broadcast has no source for. They are
    /// not announced, so Home Assistant never shows an entity that can only
    /// stay "unknown" (issue #2085):
    ///
    /// - zone occupancy, bathroom occupied, bed exit, room transition: the
    ///   broadcast carries no zones, and nothing loads a zone-tag map;
    /// - meeting in progress: needs a 1–20 % motion level, but per-node motion
    ///   is a three-level classification (absent / still / moving);
    /// - pose: the broadcast's `pose_keypoints` is never populated, and the
    ///   `persons[].keypoints` skeletons are synthesized, not measured.
    pub fn has_server_source(self) -> bool {
        !matches!(
            self,
            EntityKind::ZoneOccupancy
                | EntityKind::BathroomOccupied
                | EntityKind::BedExit
                | EntityKind::MultiRoomTransition
                | EntityKind::MeetingInProgress
                | EntityKind::PoseKeypoints
        )
    }

    /// Human-readable HA entity name shown in the UI.
    pub fn display_name(self) -> &'static str {
        match self {
            EntityKind::Presence => "Presence",
            EntityKind::PersonCount => "Person count",
            EntityKind::BreathingRate => "Breathing rate",
            EntityKind::HeartRate => "Heart rate",
            EntityKind::MotionLevel => "Motion level",
            EntityKind::MotionEnergy => "Motion energy",
            EntityKind::FallDetected => "Fall detected",
            EntityKind::PresenceScore => "Presence score",
            EntityKind::Rssi => "Signal strength",
            EntityKind::ZoneOccupancy => "Zone occupancy",
            EntityKind::PoseKeypoints => "Pose",
            EntityKind::SomeoneSleeping => "Someone sleeping",
            EntityKind::PossibleDistress => "Possible distress",
            EntityKind::RoomActive => "Room active",
            EntityKind::ElderlyInactivityAnomaly => "Elderly inactivity anomaly",
            EntityKind::MeetingInProgress => "Meeting in progress",
            EntityKind::BathroomOccupied => "Bathroom occupied",
            EntityKind::FallRiskElevated => "Fall risk elevated",
            EntityKind::BedExit => "Bed exit",
            EntityKind::NoMovement => "No movement",
            EntityKind::MultiRoomTransition => "Room transition",
        }
    }
}

/// Builds HA discovery payloads for a specific RuView node.
pub struct DiscoveryBuilder<'a> {
    pub discovery_prefix: &'a str,
    pub node_id: &'a str,
    pub node_friendly_name: Option<&'a str>,
    pub sw_version: &'a str,
    pub model: &'a str,
    pub via_device: Option<&'a str>,
}

impl<'a> DiscoveryBuilder<'a> {
    fn unique_id(&self, entity: EntityKind) -> String {
        format!("wifi_densepose_{}_{}", self.node_id, entity.topic_slug())
    }

    fn state_topic(&self, entity: EntityKind) -> String {
        format!(
            "{}/{}/wifi_densepose_{}/{}/state",
            self.discovery_prefix,
            entity.component().as_str(),
            self.node_id,
            entity.topic_slug(),
        )
    }

    pub fn config_topic(&self, entity: EntityKind) -> String {
        format!(
            "{}/{}/wifi_densepose_{}/{}/config",
            self.discovery_prefix,
            entity.component().as_str(),
            self.node_id,
            entity.topic_slug(),
        )
    }

    pub fn availability_topic(&self, entity: EntityKind) -> String {
        format!(
            "{}/{}/wifi_densepose_{}/{}/availability",
            self.discovery_prefix,
            entity.component().as_str(),
            self.node_id,
            entity.topic_slug(),
        )
    }

    fn device(&self) -> DeviceMeta {
        let display = self
            .node_friendly_name
            .map(|n| n.to_string())
            .unwrap_or_else(|| format!("RuView node {}", self.node_id));
        DeviceMeta {
            identifiers: vec![format!("wifi_densepose_{}", self.node_id)],
            name: display,
            manufacturer: MANUFACTURER.to_string(),
            model: self.model.to_string(),
            sw_version: self.sw_version.to_string(),
            via_device: self.via_device.map(|s| s.to_string()),
        }
    }

    fn origin(&self) -> OriginMeta {
        OriginMeta {
            name: ORIGIN_NAME.to_string(),
            sw_version: env!("CARGO_PKG_VERSION").to_string(),
            support_url: SUPPORT_URL.to_string(),
        }
    }

    /// Build a discovery config payload for one entity on this node.
    pub fn build(&self, entity: EntityKind) -> DiscoveryConfig {
        let component = entity.component();
        let mut cfg = DiscoveryConfig {
            name: entity.display_name().to_string(),
            unique_id: self.unique_id(entity),
            object_id: self.unique_id(entity),
            state_topic: self.state_topic(entity),
            availability_topic: Some(self.availability_topic(entity)),
            payload_available: Some("online".into()),
            payload_not_available: Some("offline".into()),
            payload_on: None,
            payload_off: None,
            device_class: None,
            state_class: None,
            unit_of_measurement: None,
            icon: None,
            value_template: None,
            json_attributes_topic: None,
            event_types: None,
            qos: match component {
                DiscoveryComponent::BinarySensor | DiscoveryComponent::Event => 1,
                DiscoveryComponent::Sensor => 0,
            },
            device: self.device(),
            origin: self.origin(),
        };

        match entity {
            EntityKind::Presence
            | EntityKind::ZoneOccupancy
            | EntityKind::SomeoneSleeping
            | EntityKind::RoomActive
            | EntityKind::MeetingInProgress
            | EntityKind::BathroomOccupied => {
                cfg.payload_on = Some("ON".into());
                cfg.payload_off = Some("OFF".into());
                cfg.device_class = Some("occupancy".into());
                cfg.icon = Some(match entity {
                    EntityKind::SomeoneSleeping => "mdi:sleep",
                    EntityKind::MeetingInProgress => "mdi:account-group",
                    EntityKind::BathroomOccupied => "mdi:shower",
                    EntityKind::RoomActive => "mdi:home-account",
                    EntityKind::ZoneOccupancy => "mdi:map-marker",
                    _ => "mdi:motion-sensor",
                }.into());
            }
            EntityKind::PossibleDistress
            | EntityKind::ElderlyInactivityAnomaly
            | EntityKind::NoMovement => {
                cfg.payload_on = Some("ON".into());
                cfg.payload_off = Some("OFF".into());
                cfg.device_class = Some("problem".into());
                cfg.icon = Some("mdi:alert-octagon".into());
            }
            EntityKind::FallDetected => {
                cfg.event_types = Some(vec!["fall_detected".into()]);
                cfg.icon = Some("mdi:human-fall".into());
            }
            EntityKind::BedExit => {
                cfg.event_types = Some(vec!["bed_exit".into()]);
                cfg.icon = Some("mdi:bed-empty".into());
            }
            EntityKind::MultiRoomTransition => {
                cfg.event_types = Some(vec!["transition".into()]);
                cfg.icon = Some("mdi:transit-transfer".into());
            }
            EntityKind::PersonCount => {
                cfg.state_class = Some("measurement".into());
                cfg.unit_of_measurement = Some("persons".into());
                cfg.icon = Some("mdi:account-group".into());
                cfg.value_template = Some("{{ value_json.n_persons }}".into());
            }
            EntityKind::BreathingRate => {
                cfg.state_class = Some("measurement".into());
                cfg.unit_of_measurement = Some("bpm".into());
                cfg.icon = Some("mdi:lungs".into());
                cfg.value_template = Some("{{ value_json.bpm }}".into());
                cfg.json_attributes_topic = Some(cfg.state_topic.clone());
            }
            EntityKind::HeartRate => {
                cfg.state_class = Some("measurement".into());
                cfg.unit_of_measurement = Some("bpm".into());
                cfg.icon = Some("mdi:heart-pulse".into());
                cfg.value_template = Some("{{ value_json.bpm }}".into());
                cfg.json_attributes_topic = Some(cfg.state_topic.clone());
            }
            EntityKind::MotionLevel => {
                cfg.state_class = Some("measurement".into());
                cfg.unit_of_measurement = Some("%".into());
                cfg.icon = Some("mdi:run".into());
                cfg.value_template = Some("{{ value_json.level_pct }}".into());
            }
            EntityKind::MotionEnergy => {
                cfg.state_class = Some("measurement".into());
                cfg.icon = Some("mdi:waveform".into());
                cfg.value_template = Some("{{ value_json.energy }}".into());
            }
            EntityKind::PresenceScore => {
                cfg.state_class = Some("measurement".into());
                cfg.unit_of_measurement = Some("%".into());
                cfg.icon = Some("mdi:gauge".into());
                cfg.value_template = Some("{{ value_json.score_pct }}".into());
            }
            EntityKind::Rssi => {
                cfg.state_class = Some("measurement".into());
                cfg.device_class = Some("signal_strength".into());
                cfg.unit_of_measurement = Some("dBm".into());
                cfg.icon = Some("mdi:wifi".into());
                cfg.value_template = Some("{{ value_json.dbm }}".into());
            }
            EntityKind::PoseKeypoints => {
                cfg.icon = Some("mdi:human".into());
                cfg.json_attributes_topic = Some(cfg.state_topic.clone());
                cfg.value_template = Some("{{ value_json.n_keypoints }}".into());
            }
            EntityKind::FallRiskElevated => {
                cfg.state_class = Some("measurement".into());
                cfg.unit_of_measurement = Some("score".into());
                cfg.icon = Some("mdi:human-fall".into());
                cfg.value_template = Some("{{ value_json.score }}".into());
            }
        }

        cfg
    }

    /// All entity kinds this builder will publish, given a `privacy_mode`
    /// flag and a `publish_pose` flag. Used by the publisher to drive the
    /// discovery-emission loop.
    pub fn enabled_entities(privacy_mode: bool, publish_pose: bool, semantic_disabled: &[String]) -> Vec<EntityKind> {
        let all = [
            EntityKind::Presence,
            EntityKind::PersonCount,
            EntityKind::BreathingRate,
            EntityKind::HeartRate,
            EntityKind::MotionLevel,
            EntityKind::MotionEnergy,
            EntityKind::FallDetected,
            EntityKind::PresenceScore,
            EntityKind::Rssi,
            EntityKind::ZoneOccupancy,
            EntityKind::PoseKeypoints,
            EntityKind::SomeoneSleeping,
            EntityKind::PossibleDistress,
            EntityKind::RoomActive,
            EntityKind::ElderlyInactivityAnomaly,
            EntityKind::MeetingInProgress,
            EntityKind::BathroomOccupied,
            EntityKind::FallRiskElevated,
            EntityKind::BedExit,
            EntityKind::NoMovement,
            EntityKind::MultiRoomTransition,
        ];

        all.into_iter()
            .filter(|e| {
                if !e.has_server_source() {
                    return false;
                }
                if privacy_mode && e.is_biometric() {
                    return false;
                }
                if *e == EntityKind::PoseKeypoints && !publish_pose {
                    return false;
                }
                if let Some(slug) = semantic_slug_for(*e) {
                    if semantic_disabled.iter().any(|d| d == slug) {
                        return false;
                    }
                }
                true
            })
            .collect()
    }
}

/// For an entity kind, return the `--no-semantic <PRIMITIVE>` slug it
/// would be disabled by, or `None` if it's not a semantic primitive.
fn semantic_slug_for(e: EntityKind) -> Option<&'static str> {
    Some(match e {
        EntityKind::SomeoneSleeping => "sleeping",
        EntityKind::PossibleDistress => "distress",
        EntityKind::RoomActive => "room_active",
        EntityKind::ElderlyInactivityAnomaly => "elderly_anomaly",
        EntityKind::MeetingInProgress => "meeting",
        EntityKind::BathroomOccupied => "bathroom",
        EntityKind::FallRiskElevated => "fall_risk",
        EntityKind::BedExit => "bed_exit",
        EntityKind::NoMovement => "no_movement",
        EntityKind::MultiRoomTransition => "multi_room",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn builder() -> DiscoveryBuilder<'static> {
        DiscoveryBuilder {
            discovery_prefix: "homeassistant",
            node_id: "aabbccddeeff",
            node_friendly_name: Some("Bedroom"),
            sw_version: "v0.7.0",
            model: "ESP32-S3 CSI node",
            via_device: Some("cognitum_seed_1"),
        }
    }

    #[test]
    fn presence_discovery_payload_shape() {
        let b = builder();
        let cfg = b.build(EntityKind::Presence);
        let j: Value = serde_json::to_value(&cfg).unwrap();
        assert_eq!(j["name"], "Presence");
        assert_eq!(j["unique_id"], "wifi_densepose_aabbccddeeff_presence");
        assert_eq!(j["device_class"], "occupancy");
        assert_eq!(j["payload_on"], "ON");
        assert_eq!(j["payload_off"], "OFF");
        assert_eq!(j["qos"], 1);
        assert_eq!(
            j["state_topic"],
            "homeassistant/binary_sensor/wifi_densepose_aabbccddeeff/presence/state"
        );
        assert_eq!(j["device"]["identifiers"][0], "wifi_densepose_aabbccddeeff");
        assert_eq!(j["device"]["name"], "Bedroom");
        assert_eq!(j["device"]["via_device"], "cognitum_seed_1");
        assert_eq!(j["origin"]["name"], "wifi-densepose-sensing-server");
    }

    #[test]
    fn heart_rate_discovery_payload_shape() {
        let b = builder();
        let cfg = b.build(EntityKind::HeartRate);
        let j: Value = serde_json::to_value(&cfg).unwrap();
        assert_eq!(j["unit_of_measurement"], "bpm");
        assert_eq!(j["state_class"], "measurement");
        assert_eq!(j["value_template"], "{{ value_json.bpm }}");
        assert_eq!(j["qos"], 0);
        assert!(j["json_attributes_topic"].as_str().unwrap().ends_with("/state"));
    }

    #[test]
    fn fall_event_payload_uses_event_component_and_types() {
        let b = builder();
        let cfg = b.build(EntityKind::FallDetected);
        let j: Value = serde_json::to_value(&cfg).unwrap();
        assert!(j["state_topic"].as_str().unwrap().contains("/event/"));
        assert_eq!(j["event_types"][0], "fall_detected");
        assert_eq!(j["qos"], 1);
    }

    #[test]
    fn semantic_primitive_uses_problem_class_for_distress() {
        let b = builder();
        let cfg = b.build(EntityKind::PossibleDistress);
        let j: Value = serde_json::to_value(&cfg).unwrap();
        assert_eq!(j["device_class"], "problem");
        assert_eq!(j["payload_on"], "ON");
        assert_eq!(j["payload_off"], "OFF");
    }

    #[test]
    fn enabled_entities_default_includes_every_sourced_entity() {
        let entities = DiscoveryBuilder::enabled_entities(false, false, &[]);
        assert!(!entities.contains(&EntityKind::PoseKeypoints));
        assert!(entities.contains(&EntityKind::Presence));
        assert!(entities.contains(&EntityKind::HeartRate));
        assert!(entities.contains(&EntityKind::SomeoneSleeping));
        assert_eq!(entities.len(), 15);
    }

    #[test]
    fn unsourced_entities_are_never_announced() {
        // #2085: announcing these left them "unknown" in HA forever.
        let entities = DiscoveryBuilder::enabled_entities(false, true, &[]);
        for e in [
            EntityKind::ZoneOccupancy,
            EntityKind::BathroomOccupied,
            EntityKind::BedExit,
            EntityKind::MultiRoomTransition,
            EntityKind::MeetingInProgress,
            EntityKind::PoseKeypoints,
        ] {
            assert!(!e.has_server_source());
            assert!(!entities.contains(&e), "{e:?} announced without a source");
        }
    }

    #[test]
    fn privacy_mode_strips_biometrics() {
        let entities = DiscoveryBuilder::enabled_entities(true, true, &[]);
        for e in &entities {
            assert!(!e.is_biometric(), "biometric {:?} leaked with privacy_mode", e);
        }
        // States derived from vital signs are biometric (#2165).
        assert!(!entities.contains(&EntityKind::SomeoneSleeping));
        assert!(!entities.contains(&EntityKind::PossibleDistress));
        // Primitives built only on presence, motion, zones and falls remain.
        assert!(entities.contains(&EntityKind::RoomActive));
        assert!(entities.contains(&EntityKind::NoMovement));
        // Without privacy mode both are announced.
        let all = DiscoveryBuilder::enabled_entities(false, true, &[]);
        assert!(all.contains(&EntityKind::SomeoneSleeping));
        assert!(all.contains(&EntityKind::PossibleDistress));
    }

    #[test]
    fn no_semantic_disables_specific_primitive() {
        let disabled = vec!["distress".to_string(), "sleeping".to_string()];
        let entities = DiscoveryBuilder::enabled_entities(false, false, &disabled);
        assert!(!entities.contains(&EntityKind::PossibleDistress));
        assert!(!entities.contains(&EntityKind::SomeoneSleeping));
        // Raw signals untouched.
        assert!(entities.contains(&EntityKind::Presence));
    }

    #[test]
    fn topic_components_match_entity_kind() {
        // binary_sensor for booleans.
        assert_eq!(EntityKind::Presence.component(), DiscoveryComponent::BinarySensor);
        assert_eq!(EntityKind::SomeoneSleeping.component(), DiscoveryComponent::BinarySensor);
        // event for one-shots.
        assert_eq!(EntityKind::FallDetected.component(), DiscoveryComponent::Event);
        assert_eq!(EntityKind::BedExit.component(), DiscoveryComponent::Event);
        // sensor for measurements.
        assert_eq!(EntityKind::HeartRate.component(), DiscoveryComponent::Sensor);
        assert_eq!(EntityKind::Rssi.component(), DiscoveryComponent::Sensor);
    }

    #[test]
    fn discovery_config_serialises_without_null_fields() {
        let b = builder();
        let cfg = b.build(EntityKind::Presence);
        let j = serde_json::to_string(&cfg).unwrap();
        // skip_serializing_if = "Option::is_none" must hide unused fields
        // so retained payloads stay compact on small brokers.
        assert!(!j.contains("\"event_types\":null"));
        assert!(!j.contains("\"unit_of_measurement\":null"));
        assert!(!j.contains("\"value_template\":null"));
    }

    #[test]
    fn availability_topic_matches_state_topic_path() {
        let b = builder();
        let state = format!(
            "homeassistant/binary_sensor/wifi_densepose_aabbccddeeff/presence/state"
        );
        let avail = b.availability_topic(EntityKind::Presence);
        // Must differ only in suffix.
        assert_eq!(
            state.trim_end_matches("/state"),
            avail.trim_end_matches("/availability"),
        );
    }

    #[test]
    fn unique_id_uses_namespaced_node_prefix() {
        let b = builder();
        let cfg = b.build(EntityKind::Rssi);
        assert!(cfg.unique_id.starts_with("wifi_densepose_"));
        // ADR-115 §7 — namespace prevents collision with other HA devices.
        assert!(cfg.unique_id.contains(b.node_id));
    }
}
