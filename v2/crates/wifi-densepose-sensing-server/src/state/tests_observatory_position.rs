use wifi_densepose_sensing_server::domain::classification::ClassificationInfo;
use wifi_densepose_sensing_server::domain::pose_attachment::attach_field_positions;
use crate::state::legacy_pose::assess_legacy_image_pose;
use wifi_densepose_sensing_server::domain::types::{FeatureInfo, SensingUpdate, SignalField};
use wifi_densepose_sensing_server::pose::derive_pose_from_sensing;
use crate::state::AppStateInner;

fn field_with_peak(ix: usize, iz: usize) -> SignalField {
    let nx = 20usize;
    let nz = 20usize;
    let mut values = vec![0.05f64; nx * nz];
    values[iz * nx + ix] = 1.0;
    SignalField {
        grid_size: [nx, 1, nz],
        values,
    }
}

fn empty_field() -> SignalField {
    SignalField {
        grid_size: [20, 1, 20],
        values: vec![0.05f64; 20 * 20],
    }
}

fn base_update(
    signal_field: SignalField,
    presence: bool,
    motion_band_power: f64,
) -> SensingUpdate {
    SensingUpdate {
        msg_type: "sensing_update".to_string(),
        timestamp: 1.0,
        source: "test".to_string(),
        tick: 1,
        nodes: vec![],
        features: FeatureInfo {
            mean_rssi: -60.0,
            variance: 48.6,
            motion_band_power,
            breathing_band_power: 0.0,
            dominant_freq_hz: 1.0,
            change_points: 0,
            spectral_power: 0.0,
        },
        classification: ClassificationInfo {
            motion_level: if presence {
                "present_moving".to_string()
            } else {
                "absent".to_string()
            },
            presence,
            confidence: 0.8,
        },
        signal_field,
        vital_signs: None,
        calibrated_presence_evidence: None,
        detector_diagnostics: None,
        enhanced_motion: None,
        enhanced_breathing: None,
        posture: None,
        signal_quality_score: None,
        quality_verdict: None,
        bssid_count: None,
        pose_keypoints: None,
        model_status: None,
        persons: None,
        estimated_persons: Some(1),
        node_features: None,
        room_inference: None,
        classifier: None,
        activity: None,
    }
}

#[test]
fn sensing_update_emits_persons_with_field_derived_position() {
    let peak_ix = 15;
    let peak_iz = 4;
    let mut update = base_update(field_with_peak(peak_ix, peak_iz), true, 63.3);

    update.persons = Some(derive_pose_from_sensing(&update));
    attach_field_positions(&mut update);

    let persons = update.persons.as_ref().expect("persons should be Some");
    assert!(!persons.is_empty(), "a present person must be emitted");

    let p0 = &persons[0];
    assert!((p0.position[0] - 3.0).abs() < 1e-6, "x={}", p0.position[0]);
    assert!((p0.position[1] - 0.0).abs() < 1e-9);
    assert!(
        (p0.position[2] - (-3.0)).abs() < 1e-6,
        "z={}",
        p0.position[2]
    );

    assert!(
        (p0.motion_score - 63.3).abs() < 1e-6,
        "motion_score={}",
        p0.motion_score
    );

    let v = serde_json::to_value(&update).unwrap();
    let arr = v["persons"].as_array().expect("persons must be a JSON array");
    assert_eq!(arr.len(), persons.len());
    let pj = &arr[0];
    assert!(
        pj.get("position").is_some(),
        "person.position missing from WS frame"
    );
    assert!(
        pj.get("motion_score").is_some(),
        "person.motion_score missing from WS frame"
    );
    assert!((pj["position"][0].as_f64().unwrap() - 3.0).abs() < 1e-6);
    assert!((pj["position"][2].as_f64().unwrap() - (-3.0)).abs() < 1e-6);
    assert!((pj["motion_score"].as_f64().unwrap() - 63.3).abs() < 1e-6);
}

#[test]
fn legacy_live_pose_is_audited_as_uncalibrated_image_data() {
    let mut update = base_update(field_with_peak(10, 10), true, 20.0);
    update.persons = Some(derive_pose_from_sensing(&update));
    let mut state = AppStateInner::minimal();

    assess_legacy_image_pose(&mut state, &update);

    let (raw, result) = state
        .pose_physics
        .latest()
        .expect("legacy image pose should reach canonical audit");
    assert_eq!(
        raw.dimensionality,
        wifi_densepose_core::PoseDimensionality::Image2d
    );
    assert!(!raw.uncertainty_calibrated);
    assert!(!raw.source.authenticated);
    assert_eq!(
        result.disposition,
        wifi_densepose_core::RefinementDisposition::Audited2d
    );
    assert!(!result.selected);
    assert!(result.refined_joints_m.is_none());
}

#[test]
fn pose_is_real_when_posture_present_and_absent_otherwise() {
    let mut no_posture = base_update(field_with_peak(10, 10), true, 40.0);
    no_posture.persons = Some(derive_pose_from_sensing(&no_posture));
    attach_field_positions(&mut no_posture);
    let p = &no_posture.persons.as_ref().unwrap()[0];
    assert!(
        p.pose.is_none(),
        "pose must stay None when no real posture exists"
    );
    let v = serde_json::to_value(&no_posture).unwrap();
    assert!(v["persons"][0].get("pose").is_none());

    let mut with_posture = base_update(field_with_peak(10, 10), true, 40.0);
    with_posture.posture = Some("lying".to_string());
    with_posture.persons = Some(derive_pose_from_sensing(&with_posture));
    attach_field_positions(&mut with_posture);
    let p2 = &with_posture.persons.as_ref().unwrap()[0];
    assert_eq!(p2.pose.as_deref(), Some("lying"));
    let v2 = serde_json::to_value(&with_posture).unwrap();
    assert_eq!(v2["persons"][0]["pose"], "lying");
}

#[test]
fn empty_room_yields_no_phantom_person() {
    let mut update = base_update(empty_field(), false, 2.0);
    update.persons = Some(derive_pose_from_sensing(&update));
    attach_field_positions(&mut update);

    let persons = update.persons.as_ref().unwrap();
    assert!(
        persons.is_empty(),
        "no-presence frame must not emit a phantom person, got {} persons",
        persons.len()
    );

    let v = serde_json::to_value(&update).unwrap();
    assert_eq!(v["persons"].as_array().unwrap().len(), 0);
}

#[test]
fn present_but_below_threshold_field_keeps_position_at_origin_not_fabricated() {
    let mut update = base_update(empty_field(), true, 55.0);
    update.persons = Some(derive_pose_from_sensing(&update));
    attach_field_positions(&mut update);

    let p = &update.persons.as_ref().unwrap()[0];
    assert_eq!(
        p.position,
        [0.0, 0.0, 0.0],
        "no peak → default origin, not fabricated coords"
    );
    assert!((p.motion_score - 55.0).abs() < 1e-6, "motion_score stays real");
}

