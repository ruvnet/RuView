use std::sync::Arc;
use axum::{extract::State, Json};
use tokio::sync::RwLock;

use wifi_densepose_sensing_server::domain::{CalibrationGridBinding, CsiGridKey};
use wifi_densepose_sensing_server::domain::node::NodeState;
use wifi_densepose_sensing_server::field_bridge;
use wifi_densepose_signal::ruvsense::field_model::FieldModel;

use crate::api::calibration::{
    calibration_status, calibration_stop, validate_startup_calibration_request,
    CalibrationIdentityRequest,
};
use crate::state::{
    calibration_sequence_order, AppStateInner, CalibrationSequenceOrder,
};

fn test_grid() -> CsiGridKey {
    CsiGridKey {
        n_subcarriers: 64,
        ppdu_type: 0,
    }
}

fn bind_test_calibration(state: &mut AppStateInner) {
    state.calibration_source_node_ids.insert(5);
    state.calibration_grid_binding = Some(CalibrationGridBinding {
        source_node_id: 5,
        grid: test_grid(),
        evidence: None,
    });
    state.node_states.entry(5).or_insert_with(NodeState::new);
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

fn feed_test_frame(
    state: &mut AppStateInner,
    node_id: u8,
    sequence: u32,
    amplitudes: &[f64],
) -> bool {
    state.maybe_feed_calibration_frame(
        node_id,
        test_grid(),
        sequence,
        amplitudes,
        std::time::Instant::now(),
    )
}

#[test]
fn calibration_session_counts_each_unique_bound_csi_sequence_once() {
    let mut state = AppStateInner::minimal();
    state.field_model = Some(
        FieldModel::new(field_bridge::single_link_config()).expect("field model"),
    );
    bind_test_calibration(&mut state);

    let first = vec![0.25; 56];
    assert!(feed_test_frame(&mut state, 5, 41, &first));
    assert!(!feed_test_frame(&mut state, 5, 41, &first));

    let second = vec![0.5; 56];
    assert!(feed_test_frame(&mut state, 5, 42, &second));
    assert!(!feed_test_frame(&mut state, 3, 43, &second));

    let model = state.field_model.as_ref().expect("field model remains active");
    assert_eq!(model.calibration_frame_count(), 2);
    assert_eq!(state.calibration_last_sequences.get(&5), Some(&42));
    assert!(!state.calibration_last_sequences.contains_key(&3));
}

#[test]
fn calibration_bounded_reorder_is_dropped_without_feeding_model() {
    let mut state = AppStateInner::minimal();
    state.field_model = Some(
        FieldModel::new(field_bridge::single_link_config()).expect("field model"),
    );
    bind_test_calibration(&mut state);
    let frame = vec![0.25; 56];

    assert!(feed_test_frame(&mut state, 5, 4_119_566, &frame));
    assert!(!feed_test_frame(&mut state, 5, 4_119_555, &frame));
    assert!(!feed_test_frame(&mut state, 5, 4_119_556, &frame));
    assert!(!feed_test_frame(&mut state, 5, 4_119_557, &frame));
    assert!(feed_test_frame(&mut state, 5, 4_119_567, &frame));
    assert!(state.calibration_sequence_fault_node_ids.is_empty());
    assert_eq!(state.calibration_reordered_packets.get(&5), Some(&3));
    assert_eq!(state.calibration_max_reorder_depth.get(&5), Some(&11));
    assert_eq!(
        state
            .field_model
            .as_ref()
            .expect("field model")
            .calibration_frame_count(),
        2
    );
}

#[test]
fn calibration_sequence_discontinuity_latches_session_failure() {
    let mut state = AppStateInner::minimal();
    state.field_model = Some(
        FieldModel::new(field_bridge::single_link_config()).expect("field model"),
    );
    bind_test_calibration(&mut state);
    let frame = vec![0.25; 56];

    assert!(feed_test_frame(&mut state, 5, 100, &frame));
    assert!(!feed_test_frame(&mut state, 5, 83, &frame));
    assert!(!feed_test_frame(&mut state, 5, 101, &frame));
    assert!(state.calibration_sequence_fault_node_ids.contains(&5));
    assert_eq!(
        state
            .field_model
            .as_ref()
            .expect("field model")
            .calibration_frame_count(),
        1
    );

    state.clear_calibration_sequence_state();
    assert!(state.calibration_sequence_fault_node_ids.is_empty());
    assert!(state.calibration_last_sequences.is_empty());
    assert!(state.calibration_reordered_packets.is_empty());
    assert!(state.calibration_max_reorder_depth.is_empty());
}

#[test]
fn calibration_sequence_wraps_and_failed_empty_input_can_retry() {
    let mut state = AppStateInner::minimal();
    state.field_model = Some(
        FieldModel::new(field_bridge::single_link_config()).expect("field model"),
    );
    bind_test_calibration(&mut state);
    let frame = vec![0.25; 56];

    assert!(!feed_test_frame(&mut state, 5, u32::MAX, &[]));
    assert!(state.calibration_last_sequences.is_empty());
    assert!(feed_test_frame(&mut state, 5, u32::MAX, &frame));
    assert!(feed_test_frame(&mut state, 5, 0, &frame));
    assert!(!feed_test_frame(&mut state, 5, 0, &frame));
    assert!(feed_test_frame(&mut state, 5, 1, &frame));
    assert_eq!(
        state
            .field_model
            .as_ref()
            .expect("field model")
            .calibration_frame_count(),
        3
    );
}

#[test]
fn calibration_sequence_order_drops_bounded_late_and_rejects_discontinuity() {
    assert_eq!(
        calibration_sequence_order(None, 7),
        CalibrationSequenceOrder::First
    );
    assert_eq!(
        calibration_sequence_order(Some(7), 8),
        CalibrationSequenceOrder::Forward
    );
    assert_eq!(
        calibration_sequence_order(Some(8), 8),
        CalibrationSequenceOrder::Duplicate
    );
    assert_eq!(
        calibration_sequence_order(Some(8), 7),
        CalibrationSequenceOrder::Reordered(1)
    );
    assert_eq!(
        calibration_sequence_order(Some(32), 16),
        CalibrationSequenceOrder::Reordered(16)
    );
    assert_eq!(
        calibration_sequence_order(Some(32), 15),
        CalibrationSequenceOrder::Discontinuity
    );
    assert_eq!(
        calibration_sequence_order(Some(0), 1_u32 << 31),
        CalibrationSequenceOrder::Discontinuity
    );
    assert_eq!(
        calibration_sequence_order(Some(u32::MAX), 0),
        CalibrationSequenceOrder::Forward
    );
}

#[tokio::test]
async fn calibration_status_and_stop_fail_closed_after_sequence_regression() {
    let mut inner = AppStateInner::minimal();
    inner.field_model = Some(
        FieldModel::new(field_bridge::single_link_config()).expect("field model"),
    );
    inner.calibration_model_id = Some("cal-model-test".to_string());
    inner.calibration_source_node_ids.insert(5);
    inner.calibration_last_sequences.insert(5, 41);
    inner.calibration_reordered_packets.insert(5, 3);
    inner.calibration_max_reorder_depth.insert(5, 3);
    inner.calibration_sequence_fault_node_ids.insert(5);
    let request = bind_test_identity(&mut inner);
    let state = Arc::new(RwLock::new(inner));

    let Json(status) = calibration_status(State(state.clone())).await;
    assert_eq!(status["last_sequence_by_node"]["5"], 41);
    assert_eq!(status["reorder_window_sequences"], 16);
    assert_eq!(status["reordered_packets_by_node"]["5"], 3);
    assert_eq!(status["max_reorder_depth_by_node"]["5"], 3);
    assert_eq!(status["sequence_fault_node_ids"], serde_json::json!([5]));

    let Json(stopped) = calibration_stop(State(state), Json(request)).await;
    assert_eq!(stopped["success"], false);
    assert_eq!(
        stopped["error_code"],
        "calibration_sequence_discontinuity"
    );
    assert_eq!(stopped["model_id"], "cal-model-test");
    assert_eq!(stopped["source_node_ids"], serde_json::json!([5]));
    assert_eq!(stopped["reordered_packets_by_node"]["5"], 3);
    assert_eq!(stopped["max_reorder_depth_by_node"]["5"], 3);
}

#[tokio::test]
async fn calibration_stop_rejects_missing_or_stale_grid_identity() {
    let mut inner = AppStateInner::minimal();
    inner.field_model = Some(
        FieldModel::new(field_bridge::single_link_config()).expect("field model"),
    );
    inner.calibration_model_id = Some("cal-model-test".to_string());
    inner.calibration_source_node_ids.insert(5);
    inner.calibration_observed_source_node_ids.insert(5);
    let request = bind_test_identity(&mut inner);
    let state = Arc::new(RwLock::new(inner));

    let Json(missing) = calibration_stop(State(state.clone()), Json(request.clone())).await;
    assert_eq!(missing["success"], false);
    assert_eq!(missing["error_code"], "calibration_grid_identity_missing");

    {
        let mut state = state.write().await;
        state.calibration_grid_binding = Some(CalibrationGridBinding {
            source_node_id: 5,
            grid: test_grid(),
            evidence: None,
        });
        state.node_states.insert(5, NodeState::new());
    }
    let Json(stale) = calibration_stop(State(state), Json(request)).await;
    assert_eq!(stale["success"], false);
    assert_eq!(stale["error_code"], "calibration_grid_stale");
}

#[test]
fn post_finalize_holdout_excludes_training_history_and_sequence_faults() {
    let mut state = AppStateInner::minimal();
    bind_test_calibration(&mut state);
    let binding = state.calibration_grid_binding.expect("binding");
    let now = std::time::Instant::now();
    {
        let node = state.node_states.get_mut(&5).expect("node");
        for sequence in 0..50 {
            node.push_field_model_frame(sequence, &[0.25; 64], now);
        }
    }
    assert!(state.field_model_holdout_ready(binding, 50, now));

    state.begin_field_model_holdout(binding);
    assert!(state
        .node_states
        .get(&5)
        .expect("node")
        .field_model_history
        .is_empty());
    assert!(!state.field_model_holdout_ready(binding, 50, now));

    {
        let node = state.node_states.get_mut(&5).expect("node");
        for sequence in 50..100 {
            node.push_field_model_frame(sequence, &[0.5; 64], now);
        }
    }
    assert!(state.field_model_holdout_ready(binding, 50, now));
    state.calibration_sequence_fault_node_ids.insert(5);
    assert!(!state.field_model_holdout_ready(binding, 50, now));
}

#[test]
fn startup_calibration_flag_fails_closed_before_state_creation() {
    assert!(validate_startup_calibration_request(false).is_ok());
    let error = validate_startup_calibration_request(true).expect_err("must fail closed");
    assert!(error.contains("--calibrate"));
}

