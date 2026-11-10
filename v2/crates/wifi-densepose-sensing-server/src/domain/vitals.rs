//! Vital signs publication and legacy edge vitals message formatting.

use crate::domain::classification::classify_vitals;
use crate::ingest::Esp32VitalsPacket;
use crate::vital_signs::VitalSigns;

/// Numeric RF vitals are public sensing evidence only after an explicit fresh
/// room calibration has resolved exactly one occupant. Bootstrap priors are
/// intentionally lower authority and can never authorize rates.
pub const VITAL_PUBLICATION_MIN_CONFIDENCE: f64 = 0.55;
pub const VITAL_PUBLICATION_MIN_SIGNAL_QUALITY: f64 = 0.40;

/// Gate raw vital signs candidates for public emission based on calibration and SNR.
pub fn vitals_for_publication(
    candidates: &VitalSigns,
    explicit_calibration_fresh: bool,
    person_count: usize,
) -> Option<VitalSigns> {
    if !explicit_calibration_fresh || person_count != 1 {
        return None;
    }
    let signal_quality = candidates.signal_quality.clamp(0.0, 1.0);
    if signal_quality < VITAL_PUBLICATION_MIN_SIGNAL_QUALITY {
        return None;
    }
    let breathing_rate_bpm = (candidates.breathing_confidence
        >= VITAL_PUBLICATION_MIN_CONFIDENCE)
        .then_some(candidates.breathing_rate_bpm)
        .flatten();
    let heart_rate_bpm =
        (candidates.heartbeat_confidence >= VITAL_PUBLICATION_MIN_CONFIDENCE)
            .then_some(candidates.heart_rate_bpm)
            .flatten();
    if breathing_rate_bpm.is_none() && heart_rate_bpm.is_none() {
        return None;
    }
    Some(VitalSigns {
        breathing_rate_bpm,
        heart_rate_bpm,
        breathing_confidence: candidates.breathing_confidence.clamp(0.0, 1.0),
        heartbeat_confidence: candidates.heartbeat_confidence.clamp(0.0, 1.0),
        signal_quality,
    })
}

/// Format the legacy edge vitals JSON object for a specific node.
pub fn edge_vitals_message_for_publication(
    raw: &Esp32VitalsPacket,
    published_vitals: Option<&VitalSigns>,
    bootstrap_empty: bool,
    explicit_calibration_fresh: bool,
    person_count: usize,
) -> serde_json::Value {
    let numeric_vitals_authorized = published_vitals.is_some();
    let abstention_reason = if numeric_vitals_authorized {
        None
    } else if bootstrap_empty {
        Some("bootstrap_empty_background")
    } else if !explicit_calibration_fresh {
        Some("explicit_calibration_required")
    } else if person_count != 1 {
        Some("exactly_one_occupant_required")
    } else {
        Some("vital_quality_gate_failed")
    };

    let local_presence = classify_vitals(raw.motion, raw.presence, raw.presence_score).presence;
    let effective_presence = local_presence && !bootstrap_empty && person_count > 0;
    let effective_motion = raw.motion && effective_presence;
    let effective_person_count = if effective_presence {
        (raw.n_persons as usize).max(1)
    } else {
        0
    };

    serde_json::json!({
        "type": "edge_vitals",
        "node_id": raw.node_id,
        "presence": effective_presence,
        "fall_detected": raw.fall_detected && !bootstrap_empty,
        "motion": effective_motion,
        "breathing_rate_bpm": published_vitals
            .and_then(|vitals| vitals.breathing_rate_bpm),
        "heartrate_bpm": published_vitals
            .and_then(|vitals| vitals.heart_rate_bpm),
        "n_persons": effective_person_count,
        "person_count_valid": raw.person_count_valid && !bootstrap_empty,
        "motion_energy": if bootstrap_empty { 0.0 } else { raw.motion_energy },
        "presence_score": if bootstrap_empty { 0.0 } else { raw.presence_score },
        "rssi": raw.rssi,
        "calibrated_evidence_authorized": explicit_calibration_fresh,
        "numeric_vitals_authorized": numeric_vitals_authorized,
        "abstention_reason": abstention_reason,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strong_candidates() -> VitalSigns {
        VitalSigns {
            breathing_rate_bpm: Some(15.0),
            heart_rate_bpm: Some(72.0),
            breathing_confidence: 0.9,
            heartbeat_confidence: 0.9,
            signal_quality: 0.9,
        }
    }

    #[test]
    fn uncalibrated_empty_and_multi_person_rooms_publish_no_vitals() {
        let candidates = strong_candidates();
        assert!(vitals_for_publication(&candidates, false, 1).is_none());
        assert!(vitals_for_publication(&candidates, true, 0).is_none());
        assert!(vitals_for_publication(&candidates, true, 2).is_none());
    }

    #[test]
    fn one_explicitly_calibrated_occupant_may_publish_qualified_vitals() {
        let published = vitals_for_publication(&strong_candidates(), true, 1).unwrap();
        assert_eq!(published.breathing_rate_bpm, Some(15.0));
        assert_eq!(published.heart_rate_bpm, Some(72.0));
    }

    #[test]
    fn weak_vital_candidates_fail_closed() {
        let mut candidates = strong_candidates();
        candidates.signal_quality = 0.2;
        assert!(vitals_for_publication(&candidates, true, 1).is_none());
    }

    fn raw_edge_vitals() -> Esp32VitalsPacket {
        Esp32VitalsPacket {
            node_id: 5,
            presence: true,
            fall_detected: true,
            motion: true,
            breathing_rate_bpm: 15.0,
            heartrate_bpm: 72.0,
            rssi: -42,
            n_persons: 1,
            person_count_valid: true,
            motion_energy: 0.8,
            presence_score: 0.9,
            timestamp_ms: 42,
        }
    }

    #[test]
    fn bootstrap_empty_background_suppresses_legacy_edge_vitals() {
        let message = edge_vitals_message_for_publication(
            &raw_edge_vitals(),
            None,
            true,
            false,
            0,
        );
        assert_eq!(message["presence"], false);
        assert_eq!(message["motion"], false);
        assert_eq!(message["fall_detected"], false);
        assert_eq!(message["n_persons"], 0);
        assert!(message["breathing_rate_bpm"].is_null());
        assert!(message["heartrate_bpm"].is_null());
        assert_eq!(message["numeric_vitals_authorized"], false);
        assert_eq!(message["abstention_reason"], "bootstrap_empty_background");
    }

    #[test]
    fn explicit_single_occupant_may_publish_legacy_edge_vitals() {
        let published = vitals_for_publication(&strong_candidates(), true, 1).unwrap();
        let message = edge_vitals_message_for_publication(
            &raw_edge_vitals(),
            Some(&published),
            false,
            true,
            1,
        );
        assert_eq!(message["presence"], true);
        assert_eq!(message["fall_detected"], true);
        assert_eq!(message["n_persons"], 1);
        assert_eq!(message["breathing_rate_bpm"], 15.0);
        assert_eq!(message["heartrate_bpm"], 72.0);
        assert_eq!(message["numeric_vitals_authorized"], true);
        assert!(message["abstention_reason"].is_null());
    }
}

