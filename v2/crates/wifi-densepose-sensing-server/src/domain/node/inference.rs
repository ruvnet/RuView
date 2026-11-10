//! Node-level inference derivation, classification updates, and RSSI validation.
//!
//! Complies with ELMAS.md: <= 350 lines, guard clauses, pure domain logic.

use std::time::Instant;
use crate::inference::NodeInference;
use crate::ingest::Esp32VitalsPacket;

use crate::domain::classification::{classify_vitals, score_to_person_count};
use super::state::NodeState;

/// Upper plausibility ceiling (dBm) for a real 2.4/5 GHz WiFi RSSI reading.
pub const MAX_PLAUSIBLE_RSSI_DBM: i8 = -10;

/// Returns `true` if `rssi_dbm` is a physically plausible WiFi RSSI reading.
pub fn is_plausible_rssi(rssi_dbm: i8) -> bool {
    rssi_dbm <= MAX_PLAUSIBLE_RSSI_DBM
}

/// Edge-only nodes must update the same node-local state used by the room vote.
pub fn update_edge_node_classification(n: &mut NodeState, vitals: &Esp32VitalsPacket) {
    let classification = classify_vitals(vitals.motion, vitals.presence, vitals.presence_score);
    n.current_motion_level = classification.motion_level;
    n.debounce_candidate = n.current_motion_level.clone();
    n.debounce_counter = 0;
    // Presence confidence is not the raw-CSI count score. Mixing those EMAs
    // turns a high-confidence single-person edge report into multiple people
    // when the next raw CSI packet arrives.
    let presence_score = if vitals.presence_score.is_finite() {
        (vitals.presence_score as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    n.latest_classification_confidence = Some(if classification.presence {
        presence_score
    } else if vitals.presence_score.is_finite() {
        1.0 - presence_score
    } else {
        0.0
    });
    n.prev_person_count = if classification.presence {
        (vitals.n_persons as usize).max(1)
    } else {
        0
    };
}

/// Raw-CSI nodes gate their count on the node's own published level.
pub fn update_node_person_count(n: &mut NodeState) {
    n.prev_person_count = if n.current_motion_level != "absent" {
        score_to_person_count(n.smoothed_person_score, n.prev_person_count)
    } else {
        0
    };
}

/// Build a node's *own* [`NodeInference`] from its smoothed per-node state.
pub fn node_inference_for(n: &NodeState, now: Instant) -> NodeInference {
    let age_ms = n
        .last_frame_time
        .map(|t| now.duration_since(t).as_millis() as u64);
    let present = !matches!(n.current_motion_level.as_str(), "absent");
    let score = n.smoothed_person_score.clamp(0.0, 1.0);
    let confidence = n.latest_classification_confidence
        .unwrap_or(if present { score } else { 1.0 - score });
    NodeInference::new(n.current_motion_level.clone(), confidence, age_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn realistic_readings_are_plausible() {
        assert!(is_plausible_rssi(-42));
        assert!(is_plausible_rssi(-53));
        assert!(is_plausible_rssi(-90));
    }

    #[test]
    fn near_zero_sentinel_values_are_rejected() {
        assert!(!is_plausible_rssi(-1));
        assert!(!is_plausible_rssi(-2));
        assert!(!is_plausible_rssi(0));
    }

    #[test]
    fn boundary_is_inclusive() {
        assert!(is_plausible_rssi(MAX_PLAUSIBLE_RSSI_DBM));
        assert!(!is_plausible_rssi(MAX_PLAUSIBLE_RSSI_DBM + 1));
    }
}
