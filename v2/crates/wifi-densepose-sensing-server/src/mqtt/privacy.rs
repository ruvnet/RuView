//! Privacy-mode filter for outbound MQTT (and Matter) state messages.
//!
//! Implements the ADR-106 primitive-isolation contract at the integration
//! boundary, gated by [`crate::cli::Args::privacy_mode`]. When the flag is
//! set, biometric channels (HR, BR, raw pose keypoints) are stripped
//! from every outbound message *and* their entities are never discovered
//! by Home Assistant — `discovery.rs::DiscoveryBuilder::enabled_entities`
//! returns the filtered set.
//!
//! Which entities are biometric is decided by
//! [`EntityKind::privacy_class`], an exhaustive match: a new entity has no
//! default class. Semantic primitives whose inputs include vital signs
//! (someone-sleeping reads breathing rate, possible-distress reads heart
//! rate) are biometric and suppressed too (#2165); the inferred state still
//! says something about the occupant's vitals. Primitives built only on
//! presence, motion, zones and falls stay published.

use super::discovery::EntityKind;

/// Decision for one outbound publication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishDecision {
    /// Send as-is.
    Publish,
    /// Drop silently (entity is suppressed by privacy mode).
    Suppress,
}

/// Decide whether an entity may be published given a privacy-mode flag.
///
/// Discovery and state share the same filter so an HA controller can't
/// learn from the absence of state that the entity might exist with
/// different filters in place — if it's stripped, it's stripped at every
/// layer.
pub fn decide(entity: EntityKind, privacy_mode: bool) -> PublishDecision {
    if privacy_mode && entity.is_biometric() {
        PublishDecision::Suppress
    } else {
        PublishDecision::Publish
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn privacy_off_publishes_everything() {
        for e in [
            EntityKind::Presence,
            EntityKind::HeartRate,
            EntityKind::BreathingRate,
            EntityKind::PoseKeypoints,
            EntityKind::SomeoneSleeping,
            EntityKind::PossibleDistress,
            EntityKind::FallDetected,
        ] {
            assert_eq!(decide(e, false), PublishDecision::Publish);
        }
    }

    #[test]
    fn privacy_on_suppresses_biometrics_only() {
        // HR / BR / pose keypoints → suppressed.
        assert_eq!(decide(EntityKind::HeartRate, true), PublishDecision::Suppress);
        assert_eq!(decide(EntityKind::BreathingRate, true), PublishDecision::Suppress);
        assert_eq!(decide(EntityKind::PoseKeypoints, true), PublishDecision::Suppress);
    }

    #[test]
    fn privacy_on_suppresses_vitals_derived_states() {
        // #2165: sleeping is inferred from breathing rate, distress from
        // heart rate, so they are biometric.
        assert_eq!(decide(EntityKind::SomeoneSleeping, true), PublishDecision::Suppress);
        assert_eq!(decide(EntityKind::PossibleDistress, true), PublishDecision::Suppress);
    }

    #[test]
    fn privacy_on_keeps_non_biometric_signals() {
        for e in [
            EntityKind::Presence,
            EntityKind::PersonCount,
            EntityKind::MotionLevel,
            EntityKind::Rssi,
            EntityKind::ZoneOccupancy,
            EntityKind::FallDetected,
            EntityKind::PresenceScore,
        ] {
            assert_eq!(decide(e, true), PublishDecision::Publish, "{:?} should not be suppressed", e);
        }
    }

    #[test]
    fn privacy_on_keeps_semantic_primitives_not_built_on_vitals() {
        // Per ADR-115 §3.12.3 — semantic primitives whose inputs are
        // presence, motion, zones and falls remain available in privacy mode.
        for e in [
            EntityKind::RoomActive,
            EntityKind::ElderlyInactivityAnomaly,
            EntityKind::MeetingInProgress,
            EntityKind::BathroomOccupied,
            EntityKind::FallRiskElevated,
            EntityKind::BedExit,
            EntityKind::NoMovement,
            EntityKind::MultiRoomTransition,
        ] {
            assert_eq!(decide(e, true), PublishDecision::Publish, "{:?} should not be suppressed", e);
        }
    }
}
