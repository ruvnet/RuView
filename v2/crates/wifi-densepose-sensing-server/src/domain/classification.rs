//! Presence and motion classification models.
//!
//! Complies with ADR-297 (room-level inference aggregation) and issue #1442
//! (presence is derived strictly from motion level, never from raw unverified flags).

use crate::inference::RoomInference;

/// Minimum time a candidate room-level classification must be observed
/// consistently before the debounced room state flips (seconds).
pub const ROOM_DEBOUNCE_DURATION_SECS: f64 = 1.5;

/// ADR-297: Window a node may be silent before it stops contributing to the fused room aggregate.
pub const NODE_STALE_AFTER_MS: u64 = 10_000;

pub use super::types::ClassificationInfo;

/// Derives the classification triple from raw ESP32 vitals fields.
///
/// `presence` is derived from `motion_level`, never taken from the raw
/// `presence` flag directly, so `motion_level: "present_moving"` can never
/// pair with `presence: false` (issue #1442).
pub fn classify_vitals(motion: bool, presence: bool, presence_score: f32) -> ClassificationInfo {
    let motion_level = if motion {
        "present_moving"
    } else if presence {
        "present_still"
    } else {
        "absent"
    };
    ClassificationInfo {
        motion_level: motion_level.to_string(),
        presence: motion_level != "absent",
        confidence: presence_score as f64,
    }
}

/// Convert smoothed person score to discrete count with hysteresis.
///
/// Uses asymmetric thresholds: higher threshold to *add* a person, lower to
/// *drop* one. This prevents flickering when the score hovers near a boundary
/// (see issues #237, #249, #280, #292).
pub fn score_to_person_count(smoothed_score: f64, prev_count: usize) -> usize {
    match prev_count {
        0 | 1 => {
            if smoothed_score > 0.85 {
                3
            } else if smoothed_score > 0.70 {
                2
            } else {
                1
            }
        }
        2 => {
            if smoothed_score > 0.92 {
                3
            } else if smoothed_score < 0.55 {
                1
            } else {
                2
            }
        }
        _ => {
            if smoothed_score < 0.55 {
                1
            } else if smoothed_score < 0.78 {
                2
            } else {
                3
            }
        }
    }
}

/// Derive the top-level (room) [`ClassificationInfo`] from the fused
/// [`RoomInference`] aggregate (ADR-297, issue #1554).
pub fn classification_from_room(room: &RoomInference) -> ClassificationInfo {
    let motion_level = if room.classification == "unavailable" {
        "absent"
    } else {
        room.classification.as_str()
    };
    ClassificationInfo {
        motion_level: motion_level.to_string(),
        presence: motion_level != "absent",
        confidence: room.confidence,
    }
}

/// Apply room-level hysteresis to a raw plurality vote.
pub fn debounce_room_classification_state(
    debounced_level: &mut String,
    debounce_candidate: &mut String,
    debounce_since: &mut Option<std::time::Instant>,
    raw: &RoomInference,
    now: std::time::Instant,
) -> ClassificationInfo {
    let candidate = if raw.classification == "unavailable" {
        "absent"
    } else {
        raw.classification.as_str()
    };

    if candidate == debounced_level.as_str() {
        *debounce_candidate = candidate.to_string();
        *debounce_since = None;
    } else if candidate == debounce_candidate.as_str() {
        let since = *debounce_since.get_or_insert(now);
        if now.duration_since(since).as_secs_f64() >= ROOM_DEBOUNCE_DURATION_SECS {
            *debounced_level = candidate.to_string();
            *debounce_since = None;
        }
    } else {
        *debounce_candidate = candidate.to_string();
        *debounce_since = Some(now);
    }

    ClassificationInfo {
        motion_level: debounced_level.clone(),
        presence: debounced_level != "absent",
        confidence: raw.confidence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn room_inf(classification: &str, confidence: f64) -> RoomInference {
        RoomInference {
            classification: classification.to_string(),
            confidence,
            contributing_nodes: 3,
        }
    }

    #[test]
    fn single_cycle_candidate_does_not_flip_immediately() {
        let mut level = "absent".to_string();
        let mut candidate = "absent".to_string();
        let mut since = None;
        let c = debounce_room_classification_state(
            &mut level, &mut candidate, &mut since, &room_inf("present_still", 0.6), Instant::now(),
        );
        assert_eq!(c.motion_level, "absent");
        assert!(!c.presence);
    }

    #[test]
    fn candidate_sustained_past_hold_duration_flips() {
        let mut level = "absent".to_string();
        let mut candidate = "present_still".to_string();
        let mut since = Some(Instant::now() - Duration::from_secs(2));
        let c = debounce_room_classification_state(
            &mut level, &mut candidate, &mut since, &room_inf("present_still", 0.7), Instant::now(),
        );
        assert_eq!(c.motion_level, "present_still");
        assert!(c.presence);
    }

    #[test]
    fn flapping_candidate_resets_the_hold_timer() {
        let mut level = "absent".to_string();
        let mut candidate = "present_still".to_string();
        let mut since = Some(Instant::now() - Duration::from_secs(1));
        let c = debounce_room_classification_state(
            &mut level, &mut candidate, &mut since, &room_inf("active", 0.5), Instant::now(),
        );
        assert_eq!(c.motion_level, "absent");
        assert_eq!(candidate, "active");
        assert!(since.unwrap().elapsed() < Duration::from_millis(100));
    }

    #[test]
    fn already_stable_state_reports_immediately_without_waiting() {
        let mut level = "present_moving".to_string();
        let mut candidate = "present_moving".to_string();
        let mut since = None;
        let c = debounce_room_classification_state(
            &mut level, &mut candidate, &mut since, &room_inf("present_moving", 0.9), Instant::now(),
        );
        assert_eq!(c.motion_level, "present_moving");
        assert!(c.presence);
    }

    #[test]
    fn unavailable_room_debounces_toward_absent() {
        let mut level = "present_still".to_string();
        let mut candidate = "absent".to_string();
        let mut since = Some(Instant::now() - Duration::from_secs(2));
        let c = debounce_room_classification_state(
            &mut level, &mut candidate, &mut since, &RoomInference::unavailable(), Instant::now(),
        );
        assert_eq!(c.motion_level, "absent");
        assert!(!c.presence);
    }

    #[test]
    fn motion_forces_present_moving() {
        let c = classify_vitals(true, false, 0.85);
        assert_eq!(c.motion_level, "present_moving");
        assert!(c.presence);
        assert!((c.confidence - 0.85).abs() < 1e-4);
    }

    #[test]
    fn presence_without_motion_yields_present_still() {
        let c = classify_vitals(false, true, 0.75);
        assert_eq!(c.motion_level, "present_still");
        assert!(c.presence);
    }

    #[test]
    fn no_motion_no_presence_yields_absent() {
        let c = classify_vitals(false, false, 0.0);
        assert_eq!(c.motion_level, "absent");
        assert!(!c.presence);
    }

    #[test]
    fn unavailable_room_maps_to_absent() {
        let room = RoomInference::unavailable();
        let c = classification_from_room(&room);
        assert_eq!(c.motion_level, "absent");
        assert!(!c.presence);
        assert_eq!(c.confidence, 0.0);
    }

    #[test]
    fn motion_implies_presence_issue_1442() {
        let c = classify_vitals(true, false, 0.69);
        assert_eq!(c.motion_level, "present_moving");
        assert!(c.presence);
    }

    #[test]
    fn derives_presence_and_motion_from_room_classification() {
        let room = RoomInference {
            classification: "present_moving".to_string(),
            confidence: 0.82,
            contributing_nodes: 3,
        };
        let c = classification_from_room(&room);
        assert_eq!(c.motion_level, "present_moving");
        assert!(c.presence);
        assert!((c.confidence - 0.82).abs() < 1e-9);
    }

    #[test]
    fn does_not_depend_on_which_node_is_named_last() {
        let a = RoomInference {
            classification: "present_still".to_string(),
            confidence: 0.6,
            contributing_nodes: 2,
        };
        let b = a.clone();
        assert_eq!(classification_from_room(&a), classification_from_room(&b));
    }

    #[test]
    fn person_count_hysteresis() {
        // From 1 person, high score moves to 2 or 3
        assert_eq!(score_to_person_count(0.75, 1), 2);
        assert_eq!(score_to_person_count(0.90, 1), 3);
        assert_eq!(score_to_person_count(0.60, 1), 1);

        // From 2 persons, drop requires < 0.55
        assert_eq!(score_to_person_count(0.60, 2), 2); // holds
        assert_eq!(score_to_person_count(0.50, 2), 1); // drops
    }
}
