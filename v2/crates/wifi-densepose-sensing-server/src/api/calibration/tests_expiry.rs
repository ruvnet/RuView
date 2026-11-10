use std::sync::Arc;
use axum::{extract::State, Json};
use tokio::sync::RwLock;

use wifi_densepose_sensing_server::bootstrap_baseline;
use wifi_densepose_sensing_server::domain::{
    CalibrationGridBinding, CalibrationModelReceipt, CsiGridKey,
};
use wifi_densepose_sensing_server::domain::classification::score_to_person_count;
use wifi_densepose_sensing_server::domain::node::NodeState;
use wifi_densepose_sensing_server::domain::vitals::vitals_for_publication;
use wifi_densepose_sensing_server::field_bridge;
use crate::state::calibrated_vitals_for_publication;
use wifi_densepose_sensing_server::vital_signs::VitalSigns;
use wifi_densepose_signal::ruvsense::field_model::{FieldModel, FieldModelConfig};

use crate::api::calibration::calibration_status;
use crate::state::{AppStateInner, ESP32_OFFLINE_TIMEOUT};

fn finalized_model(calibrated_at_unix_ms: u64) -> FieldModel {
    let mut model = FieldModel::new(FieldModelConfig {
        n_links: 1,
        n_subcarriers: 56,
        n_modes: 3,
        min_calibration_frames: 10,
        min_calibration_duration_s: 0.0,
        baseline_expiry_s: 2.0,
    })
    .unwrap();
    for frame_index in 0..20 {
        let frame = (0..56)
            .map(|subcarrier| {
                10.0 + subcarrier as f64 * 0.01
                    + (frame_index as f64 * 0.1 + subcarrier as f64 * 0.03).sin() * 0.01
            })
            .collect();
        model.feed_calibration(&[frame]).unwrap();
    }
    model
        .finalize_calibration(calibrated_at_unix_ms.saturating_mul(1_000), 7)
        .unwrap();
    model
}

fn state_with_model(bootstrap: bool) -> AppStateInner {
    let model = finalized_model(1_000);
    let baseline = model.modes().unwrap().baseline[0].clone();
    let mut state = AppStateInner::minimal();
    state.frame_history = (0..50).map(|_| baseline.clone()).collect();
    let mut node = NodeState::new();
    node.field_model_history = (0..50).map(|_| baseline.clone()).collect();
    node.field_model_latest_sequence = Some(50);
    node.field_model_latest_seen = Some(std::time::Instant::now());
    state.node_states.insert(5, node);
    state.calibration_source_node_ids.insert(5);
    state.calibration_grid_binding = Some(CalibrationGridBinding {
        source_node_id: 5,
        grid: CsiGridKey {
            n_subcarriers: 64,
            ppdu_type: 0,
        },
        evidence: None,
    });
    state.field_model = Some(model);
    state.bootstrap_baseline_active = bootstrap;
    if bootstrap {
        state.bootstrap_baseline = Some(bootstrap_baseline::BootstrapBaselineMetadata {
            authority: bootstrap_baseline::BOOTSTRAP_BASELINE_AUTHORITY,
            source_node_ids: vec![5],
            source_grid: bootstrap_baseline::BootstrapCsiGrid {
                n_subcarriers: 64,
                ppdu_type: 0,
            },
            source_model_id: "test-model".to_string(),
            created_at_unix_ms: 1_000,
            expires_at_unix_ms: 3_000,
            content_sha256: "00".repeat(32),
        });
    }
    state
}

fn state_with_receipt() -> AppStateInner {
    let mut state = state_with_model(false);
    state.calibration_session_id = Some("cal-session-test".to_string());
    state.calibration_model_id = Some("cal-model-test".to_string());
    state.calibration_binding_digest = Some("ab".repeat(32));
    state.calibration_model_receipt = Some(CalibrationModelReceipt {
        schema: field_bridge::CALIBRATION_MODEL_RECEIPT_SCHEMA,
        boot_epoch: state.calibration_boot_epoch.clone(),
        session_id: "cal-session-test".to_string(),
        model_id: "cal-model-test".to_string(),
        binding_digest: "ab".repeat(32),
        source_node_ids: vec![5],
        frame_count: 1_000,
        variance_explained: 0.9,
        baseline_eigenvalue_count: 1,
        completed_at_unix_ms: 1_000,
    });
    state
}

#[test]
fn calibrated_presence_evidence_binds_the_active_model_receipt() {
    let state = state_with_receipt();
    let evidence = state
        .calibrated_presence_evidence(5, 77, 1_500)
        .expect("a fresh explicit calibration must publish calibrated evidence");

    assert_eq!(
        evidence.schema,
        field_bridge::CALIBRATED_PRESENCE_EVIDENCE_SCHEMA
    );
    assert_eq!(evidence.boot_epoch, state.calibration_boot_epoch);
    assert_eq!(evidence.session_id, "cal-session-test");
    assert_eq!(evidence.model_id, "cal-model-test");
    assert_eq!(evidence.binding_digest, "ab".repeat(32));
    assert_eq!(evidence.source_node_ids, vec![5]);
    assert_eq!(evidence.model_completed_at_unix_ms, 1_000);
    assert_eq!(evidence.inference_node_id, 5);
    assert_eq!(evidence.source_tick, 77);
    assert_eq!(evidence.observed_at_unix_ms, 1_500);
    assert!(!evidence.inference_method.is_empty());

    assert_eq!(evidence.person_count, state.person_count_at(1_500));
    assert_eq!(evidence.presence, evidence.person_count > 0);
}

#[test]
fn calibrated_presence_evidence_absent_without_an_explicit_fresh_calibration() {
    let mut no_receipt = state_with_receipt();
    no_receipt.calibration_model_receipt = None;
    assert!(no_receipt.calibrated_presence_evidence(5, 77, 1_500).is_none());

    let mut bootstrap = state_with_receipt();
    bootstrap.bootstrap_baseline_active = true;
    assert!(bootstrap.calibrated_presence_evidence(5, 77, 1_500).is_none());

    let expired = state_with_receipt();
    assert!(expired.calibrated_presence_evidence(5, 77, u64::MAX / 2).is_none());
}

fn qualified_vital_candidates() -> VitalSigns {
    VitalSigns {
        breathing_rate_bpm: Some(15.0),
        heart_rate_bpm: Some(70.0),
        breathing_confidence: 0.9,
        heartbeat_confidence: 0.9,
        signal_quality: 0.9,
    }
}

fn state_with_runtime_window() -> AppStateInner {
    let mut state = state_with_receipt();
    let model = state.field_model.as_mut().unwrap();
    let mut snapshot = model.export_snapshot().unwrap();
    snapshot.modes.baseline_runtime_window_size = Some(50);
    *model = FieldModel::from_snapshot(snapshot, 1_500_000).unwrap();
    state
}

#[test]
fn diagnostics_stay_outside_the_strict_calibration_authority_envelope() {
    let state = state_with_runtime_window();
    let evidence = state.calibrated_presence_evidence(5, 77, 1_500).unwrap();
    let diagnostic = evidence.diagnostics.as_ref().expect("bound source diagnostics");
    assert_eq!(diagnostic.scored_node_id, 5);
    assert_eq!(diagnostic.runtime.window_frames, 50);
    let encoded = serde_json::to_value(&evidence).unwrap();
    assert!(encoded.get("diagnostics").is_none());
    assert_eq!(encoded.as_object().unwrap().len(), 13);
    assert_eq!(encoded["person_count"], 0);
    assert_eq!(serde_json::to_value(diagnostic).unwrap()["schema"],
        field_bridge::DETECTOR_DIAGNOSTICS_SCHEMA);
}

#[test]
fn calibrated_empty_blocks_vitals_even_when_room_heuristic_reports_one() {
    let state = state_with_runtime_window();
    assert_eq!(state.person_count_at(1_500), 0);
    let candidates = qualified_vital_candidates();
    assert!(vitals_for_publication(&candidates, true, 1).is_some());
    assert!(calibrated_vitals_for_publication(&state, &candidates, 1, 1_500).is_none());
}

#[test]
fn calibrated_single_occupant_preserves_quality_and_room_gates() {
    let mut state = state_with_runtime_window();
    let baseline = state.field_model.as_ref().unwrap().modes().unwrap().baseline[0].clone();
    for offset in [0.1, 0.2, 0.3, 0.5, 0.7, 1.0] {
        state.node_states.get_mut(&5).unwrap().field_model_history =
            (0..50).map(|_| baseline.iter().map(|value| value + offset).collect()).collect();
        if state.person_count_at(1_500) == 1 { break; }
    }
    assert_eq!(state.person_count_at(1_500), 1);
    let candidates = qualified_vital_candidates();
    assert!(calibrated_vitals_for_publication(&state, &candidates, 1, 1_500).is_some());
    for count in [0, 2, 3] {
        assert!(calibrated_vitals_for_publication(&state, &candidates, count, 1_500).is_none());
    }
    let weak = VitalSigns { signal_quality: 0.1, ..candidates.clone() };
    assert!(calibrated_vitals_for_publication(&state, &weak, 1, 1_500).is_none());
    state.bootstrap_baseline_active = true;
    assert!(calibrated_vitals_for_publication(&state, &candidates, 1, 1_500).is_none());
    state.bootstrap_baseline_active = false;
    assert!(calibrated_vitals_for_publication(&state, &candidates, 1, u64::MAX / 2).is_none());
    state.calibration_model_receipt = None;
    assert!(calibrated_vitals_for_publication(&state, &candidates, 1, 1_500).is_none());
}

#[test]
fn calibrated_vitals_require_a_complete_fresh_unfaulted_source_window() {
    let mut state = state_with_runtime_window();
    let candidates = qualified_vital_candidates();
    let baseline = state.field_model.as_ref().unwrap().modes().unwrap().baseline[0].clone();
    state.node_states.get_mut(&5).unwrap().field_model_history =
        (0..50).map(|_| baseline.iter().map(|value| value + 0.3).collect()).collect();
    assert_eq!(state.person_count_at(1_500), 1);
    assert!(calibrated_vitals_for_publication(&state, &candidates, 1, 1_500).is_some());
    state.calibration_sequence_fault_node_ids.insert(5);
    assert!(calibrated_vitals_for_publication(&state, &candidates, 1, 1_500).is_none());
    state.calibration_sequence_fault_node_ids.clear();
    let node = state.node_states.get_mut(&5).unwrap();
    node.field_model_latest_seen = Some(std::time::Instant::now() - ESP32_OFFLINE_TIMEOUT);
    assert!(calibrated_vitals_for_publication(&state, &candidates, 1, 1_500).is_none());
    let node = state.node_states.get_mut(&5).unwrap();
    node.field_model_latest_seen = Some(std::time::Instant::now());
    node.field_model_history.pop_front();
    assert!(calibrated_vitals_for_publication(&state, &candidates, 1, 1_500).is_none());
}

#[test]
fn bootstrap_field_model_remains_negative_only_for_person_count() {
    let runtime = state_with_model(false);
    assert_eq!(runtime.person_count_at(1_500), 0);

    let bootstrap = state_with_model(true);
    assert_eq!(
        bootstrap.person_count_at(1_500),
        score_to_person_count(0.0, 0),
        "a bootstrap prior must not derive a positive or negative count through the runtime model path"
    );
}

#[tokio::test]
async fn long_running_expiry_is_not_reported_active_for_runtime_or_bootstrap() {
    let runtime = Arc::new(RwLock::new(state_with_model(false)));
    let Json(runtime_status) = calibration_status(State(runtime)).await;
    assert_eq!(runtime_status["active"], false);
    assert_eq!(runtime_status["status"], "expired");
    assert_eq!(runtime_status["binding_mode"], "none");

    let bootstrap = Arc::new(RwLock::new(state_with_model(true)));
    let Json(bootstrap_status) = calibration_status(State(bootstrap)).await;
    assert_eq!(bootstrap_status["active"], false);
    assert_eq!(bootstrap_status["status"], "expired");
    assert_eq!(bootstrap_status["binding_mode"], "none");
    assert_eq!(bootstrap_status["bootstrap_baseline"]["stored"], true);
    assert_eq!(bootstrap_status["bootstrap_baseline"]["active"], false);
    assert_eq!(
        bootstrap_status["bootstrap_baseline"]["calibrated_evidence_authorized"],
        false
    );
    assert_eq!(
        bootstrap_status["bootstrap_baseline"]["numeric_vitals_authorized"],
        false
    );
}

