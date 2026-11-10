use std::sync::Arc;
use axum::{extract::{Query, State}, Json};
use tokio::sync::RwLock;

use wifi_densepose_sensing_server::domain::CsiGridKey;
use wifi_densepose_sensing_server::domain::node::NodeState;
use wifi_densepose_sensing_server::field_bridge;
use wifi_densepose_signal::ruvsense::field_model::FieldModel;

use crate::api::calibration::{
    calibration_source_accepts, calibration_source_has_fresh_csi, calibration_start,
    calibration_status, CalibrationBindingRequest, CalibrationIdentityRequest,
    CalibrationStartQuery,
};
use crate::state::{AppStateInner, ESP32_OFFLINE_TIMEOUT};

fn test_grid() -> CsiGridKey {
    CsiGridKey {
        n_subcarriers: 64,
        ppdu_type: 0,
    }
}

fn test_binding_request(source_node_ids: Vec<u8>) -> CalibrationBindingRequest {
    CalibrationBindingRequest {
        binding_digest: "a".repeat(64),
        source_node_ids,
    }
}

fn bind_test_identity(state: &mut AppStateInner) -> CalibrationIdentityRequest {
    state.calibration_session_id = Some("cal-session-11111111111111111111111111111111".into());
    state.calibration_binding_digest = Some("a".repeat(64));
    CalibrationIdentityRequest {
        boot_epoch: state.calibration_boot_epoch.clone(),
        session_id: state.calibration_session_id.clone(),
        binding_digest: state.calibration_binding_digest.clone(),
        source_node_ids: Some(state.calibration_source_node_ids.iter().copied().collect()),
    }
}

#[test]
fn single_link_calibration_binds_to_first_source() {
    let mut sources = std::collections::BTreeSet::new();
    assert!(calibration_source_accepts(&sources, 5));
    sources.insert(5);
    assert!(calibration_source_accepts(&sources, 5));
    assert!(!calibration_source_accepts(&sources, 3));
}

#[test]
fn explicit_single_link_source_rejects_other_live_nodes() {
    let sources = std::collections::BTreeSet::from([5]);
    assert!(calibration_source_accepts(&sources, 5));
    assert!(!calibration_source_accepts(&sources, 3));
    assert!(!calibration_source_accepts(&sources, 7));
}

#[test]
fn edge_vitals_liveness_cannot_authorize_raw_csi_calibration() {
    let now = std::time::Instant::now();
    let mut node = NodeState::new();
    node.last_frame_time = Some(now);
    assert!(!calibration_source_has_fresh_csi(&node, now));

    node.observe_accepted_csi_frame(9, false, now);
    assert!(calibration_source_has_fresh_csi(&node, now));
    assert!(!calibration_source_has_fresh_csi(
        &node,
        now + ESP32_OFFLINE_TIMEOUT
    ));
}

#[test]
fn calibration_grid_selection_prefers_stable_rate_qualified_stream() {
    let start = std::time::Instant::now();
    let mut node = NodeState::new();
    let stable = CsiGridKey {
        n_subcarriers: 64,
        ppdu_type: 0,
    };
    let sparse = CsiGridKey {
        n_subcarriers: 192,
        ppdu_type: 0,
    };
    for index in 0..=48 {
        node.observe_raw_grid(
            stable,
            start + std::time::Duration::from_millis(index * 250),
        );
    }
    for index in 0..=6 {
        node.observe_raw_grid(
            sparse,
            start + std::time::Duration::from_secs(index * 2),
        );
    }
    let (selected, evidence) = node
        .select_calibration_grid(start + std::time::Duration::from_secs(12))
        .expect("stable grid is eligible");
    assert_eq!(selected, stable);
    assert!(evidence.rate_hz >= 3.9);
    assert!(evidence.max_gap_s < 5.0);
}

#[tokio::test]
async fn calibration_start_requires_evidence_then_binds_the_qualified_grid() {
    let mut inner = AppStateInner::minimal();
    inner.node_states.insert(5, NodeState::new());
    let state = Arc::new(RwLock::new(inner));

    let Json(unavailable) = calibration_start(
        State(state.clone()),
        Query(CalibrationStartQuery {
            source_node_id: Some(5),
        }),
        Json(test_binding_request(vec![5])),
    )
    .await;
    assert_eq!(unavailable["success"], false);
    assert_eq!(unavailable["error_code"], "calibration_grid_unavailable");
    {
        let state = state.read().await;
        assert!(state.field_model.is_none());
        assert!(state.calibration_model_id.is_none());
        assert!(state.calibration_source_node_ids.is_empty());
        assert!(state.calibration_grid_binding.is_none());
    }

    let now = std::time::Instant::now();
    let start = now - std::time::Duration::from_secs(10);
    {
        let mut state = state.write().await;
        let node = state.node_states.get_mut(&5).expect("node");
        for index in 0..=40 {
            node.observe_raw_grid(
                test_grid(),
                start + std::time::Duration::from_millis(index * 250),
            );
        }
    }
    let Json(started) = calibration_start(
        State(state.clone()),
        Query(CalibrationStartQuery {
            source_node_id: Some(5),
        }),
        Json(test_binding_request(vec![5])),
    )
    .await;
    assert_eq!(started["success"], true);
    assert_eq!(started["status"], "uncalibrated");
    assert_eq!(started["frame_count"], 0);
    assert_eq!(started["binding_digest"], "a".repeat(64));
    assert!(started["boot_epoch"].as_str().is_some_and(|value| value.starts_with("cal-boot-") && value.len() == 41));
    assert!(started["session_id"].as_str().is_some_and(|value| value.starts_with("cal-session-") && value.len() == 44));
    assert!(started["model_id"].as_str().is_some_and(|value| value.starts_with("cal-model-") && value.len() == 42));
    assert_eq!(started["source_node_ids"], serde_json::json!([5]));
    assert_eq!(started["source_grid"]["n_subcarriers"], 64);
    assert_eq!(started["source_grid"]["ppdu_type"], 0);
    let state = state.read().await;
    assert!(state.field_model.is_some());
    assert!(state.calibration_model_id.is_some());
    assert_eq!(state.calibration_source_node_ids, [5].into_iter().collect());
    assert_eq!(
        state.calibration_grid_binding.map(|binding| binding.grid),
        Some(test_grid())
    );
}

#[tokio::test]
async fn calibration_status_emits_bound_identity_and_exact_contributions() {
    let mut inner = AppStateInner::minimal();
    inner.field_model = Some(FieldModel::new(field_bridge::single_link_config()).expect("field model"));
    inner.calibration_model_id = Some("cal-model-22222222222222222222222222222222".into());
    inner.calibration_source_node_ids.extend([5, 7]);
    inner.calibration_observed_source_node_ids.insert(5);
    let request = bind_test_identity(&mut inner);
    let expected_boot_epoch = request.boot_epoch.clone();
    let state = Arc::new(RwLock::new(inner));

    let Json(status) = calibration_status(State(state)).await;
    assert_eq!(status["active"], true);
    assert_eq!(status["binding_mode"], "bound");
    assert_eq!(status["legacy"], false);
    assert_eq!(status["boot_epoch"], expected_boot_epoch);
    assert_eq!(status["session_id"], request.session_id.unwrap());
    assert_eq!(status["binding_digest"], "a".repeat(64));
    assert_eq!(status["source_node_ids"], serde_json::json!([5, 7]));
    assert_eq!(status["observed_source_node_ids"], serde_json::json!([5]));
    assert_eq!(status["missing_source_node_ids"], serde_json::json!([7]));
    assert!(status["model_receipt"].is_null());
}

#[tokio::test]
async fn calibration_status_emits_idle_process_identity() {
    let inner = AppStateInner::minimal();
    let boot_epoch = inner.calibration_boot_epoch.clone();
    let state = Arc::new(RwLock::new(inner));

    let Json(status) = calibration_status(State(state)).await;
    assert_eq!(status["active"], false);
    assert_eq!(status["binding_mode"], "none");
    assert_eq!(status["legacy"], false);
    assert_eq!(status["boot_epoch"], boot_epoch);
    assert!(status["session_id"].is_null());
    assert!(status["binding_digest"].is_null());
    assert_eq!(status["source_node_ids"], serde_json::json!([]));
    assert_eq!(status["observed_source_node_ids"], serde_json::json!([]));
    assert_eq!(status["missing_source_node_ids"], serde_json::json!([]));
    assert!(status["model_receipt"].is_null());
}

