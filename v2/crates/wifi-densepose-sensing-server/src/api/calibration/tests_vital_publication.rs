use wifi_densepose_sensing_server::domain::vitals::{
    edge_vitals_message_for_publication, vitals_for_publication,
};
use wifi_densepose_sensing_server::ingest::Esp32VitalsPacket;
use wifi_densepose_sensing_server::vital_signs::VitalSigns;

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

