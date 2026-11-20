//! SYNTHETIC header-grid fixtures through actual legacy calibration handlers.
use super::*;
use std::time::{Duration, Instant};
fn grid(bins: u16, ppdu: u8) -> CsiGridKey {
    CsiGridKey {
        n_subcarriers: bins,
        ppdu_type: ppdu,
    }
}
fn ready_node(grids: &[CsiGridKey], now: Instant) -> NodeState {
    let mut node = NodeState::new();
    for index in 0..=48 {
        for grid in grids {
            node.observe_raw_grid(
                *grid,
                now - Duration::from_secs(12) + Duration::from_millis(index * 250),
            );
        }
    }
    node
}
fn request(ids: Vec<u8>) -> CalibrationBindingRequest {
    CalibrationBindingRequest {
        binding_digest: "a".repeat(64),
        source_node_ids: ids,
    }
}
#[tokio::test]
async fn mixed_he256_ht64_is_rejected_before_existing_baseline_or_receipt_mutation() {
    let now = Instant::now();
    let mut inner = AppStateInner::minimal();
    inner
        .node_states
        .insert(3, ready_node(&[grid(256, 1)], now));
    inner.node_states.insert(5, ready_node(&[grid(64, 0)], now));
    inner
        .node_states
        .insert(99, ready_node(&[grid(192, 3)], now));
    inner.field_model = Some(FieldModel::new(field_bridge::single_link_config()).unwrap());
    inner.bootstrap_baseline_active = true;
    inner.calibration_model_id = Some("old-model-must-stay".into());
    inner.calibration_session_id = Some("old-session-must-stay".into());
    inner.calibration_binding_digest = Some("b".repeat(64));
    inner.calibration_source_node_ids.extend([3, 5]);
    inner.calibration_observed_source_node_ids.insert(3);
    inner.calibration_model_receipt = Some(CalibrationModelReceipt {
        schema: "ruview.room-calibration.model-receipt.v1".into(),
        boot_epoch: inner.calibration_boot_epoch.clone(),
        session_id: "old-session-must-stay".into(),
        model_id: "old-model-must-stay".into(),
        binding_digest: "b".repeat(64),
        source_node_ids: vec![3, 5],
        frame_count: 12345,
        variance_explained: 0.95,
        baseline_eigenvalue_count: 2,
        completed_at_unix_ms: 1793800000000,
    });
    let before = serde_json::to_value(inner.calibration_model_receipt.clone()).unwrap();
    let state = Arc::new(RwLock::new(inner));
    let Json(result) = calibration_start(
        State(state.clone()),
        Query(CalibrationStartQuery {
            source_node_id: Some(3),
        }),
        None,
        None,
        Json(request(vec![3, 5])),
    )
    .await;
    assert_eq!(
        result["success"], false,
        "heterogeneous raw grids must fail before the ten-minute run: {result}"
    );
    assert_eq!(result["error_code"], "calibration_bound_grid_unavailable");
    let preserved = state.read().await;
    assert!(preserved.field_model.is_some());
    assert!(preserved.bootstrap_baseline_active);
    assert_eq!(
        preserved.calibration_model_id.as_deref(),
        Some("old-model-must-stay")
    );
    assert_eq!(
        preserved.calibration_session_id.as_deref(),
        Some("old-session-must-stay")
    );
    assert_eq!(
        preserved.calibration_binding_digest.as_deref(),
        Some("b".repeat(64).as_str())
    );
    assert_eq!(
        preserved.calibration_observed_source_node_ids,
        [3].into_iter().collect()
    );
    assert_eq!(
        serde_json::to_value(&preserved.calibration_model_receipt).unwrap(),
        before
    );
}
async fn start_nodes(
    nodes: Vec<(u8, NodeState)>,
    ids: Vec<u8>,
    author: u8,
) -> (SharedState, serde_json::Value) {
    let mut inner = AppStateInner::minimal();
    inner.node_states.extend(nodes);
    let state = Arc::new(RwLock::new(inner));
    let Json(result) = calibration_start(
        State(state.clone()),
        Query(CalibrationStartQuery {
            source_node_id: Some(author),
        }),
        None,
        None,
        Json(request(ids)),
    )
    .await;
    (state, result)
}
#[tokio::test]
async fn same_bin_count_different_ppdu_is_not_a_compatible_legacy_grid() {
    let now = Instant::now();
    let (state, result) = start_nodes(
        vec![
            (3, ready_node(&[grid(256, 1)], now)),
            (5, ready_node(&[grid(256, 2)], now)),
        ],
        vec![3, 5],
        3,
    )
    .await;
    assert_eq!(result["success"], false);
    assert_eq!(
        result["source_grid_readiness"]["nodes"][1]["reason"],
        "nonmatching_qualified_grid"
    );
    assert_eq!(
        result["source_grid_readiness"]["nodes"][1]["eligible_grids"][0],
        serde_json::json!({"n_subcarriers":256,"ppdu_type":2})
    );
    assert!(state.read().await.field_model.is_none());
}
#[tokio::test]
async fn every_bound_node_can_supply_the_exact_grid_and_unbound_nodes_do_not_leak() {
    let now = Instant::now();
    let selected = grid(64, 0);
    let (state, result) = start_nodes(
        vec![
            (3, ready_node(&[selected], now)),
            (5, ready_node(&[selected], now)),
            (99, ready_node(&[grid(256, 3)], now)),
        ],
        vec![3, 5],
        3,
    )
    .await;
    assert_eq!(result["success"], true);
    assert_eq!(
        result["source_grid_readiness"]["all_bound_nodes_ready"],
        true
    );
    let Json(status) = calibration_status(State(state)).await;
    assert_eq!(
        status["source_grid_readiness"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n["node_id"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        vec![3, 5]
    );
    assert!(!status["source_grid_readiness"].to_string().contains("99"));
}
#[tokio::test]
async fn nonpreferred_but_eligible_shared_grid_passes_without_blending_model_authors() {
    let now = Instant::now();
    let selected = grid(64, 0);
    let dense = grid(256, 1);
    let c6 = ready_node(&[selected, dense], now);
    assert_eq!(c6.select_calibration_grid(now).unwrap().0, dense);
    let (state, result) = start_nodes(
        vec![(3, c6), (5, ready_node(&[selected], now))],
        vec![3, 5],
        5,
    )
    .await;
    assert_eq!(result["success"], true, "{result}");
    assert_eq!(
        result["source_grid"],
        serde_json::json!({"n_subcarriers":64,"ppdu_type":0})
    );
    assert_eq!(
        result["source_grid_readiness"]["nodes"][0]["eligible_grids"][0],
        result["source_grid"]
    );
    let mut inner = state.write().await;
    let values = vec![2.0; 64];
    let before = inner
        .field_model
        .as_ref()
        .unwrap()
        .calibration_frame_count();
    assert!(inner.maybe_feed_calibration_frame(3, selected, 1, &values, Instant::now()));
    assert_eq!(
        inner
            .field_model
            .as_ref()
            .unwrap()
            .calibration_frame_count(),
        before
    );
    assert!(inner.calibration_observed_source_node_ids.contains(&3));
    assert!(inner.maybe_feed_calibration_frame(5, selected, 1, &values, Instant::now()));
    assert_eq!(
        inner
            .field_model
            .as_ref()
            .unwrap()
            .calibration_frame_count(),
        before + 1
    );
}
#[tokio::test]
async fn missing_edge_only_warming_and_stale_nodes_are_not_claimed_incompatible_hardware() {
    let now = Instant::now();
    let selected = grid(64, 0);
    let cases = [
        "node_missing",
        "raw_grid_missing",
        "grid_evidence_warming",
        "grid_evidence_stale",
        "grid_evidence_unqualified",
    ];
    for reason in cases {
        let mut nodes = vec![(3, ready_node(&[selected], now))];
        if reason != "node_missing" {
            let mut other = if reason == "grid_evidence_stale" {
                ready_node(&[selected], now - Duration::from_secs(6))
            } else {
                NodeState::new()
            };
            other.last_frame_time = Some(now); // Edge/other liveness is not raw grid proof.
            if reason == "grid_evidence_warming" {
                other.observe_raw_grid(selected, now);
            }
            if reason == "grid_evidence_unqualified" {
                for i in 0..=12 {
                    other.observe_raw_grid(
                        selected,
                        now - Duration::from_secs(12) + Duration::from_secs(i),
                    );
                }
            }
            nodes.push((5, other));
        }
        let (_, result) = start_nodes(nodes, vec![3, 5], 3).await;
        assert_eq!(result["success"], false, "{reason}: {result}");
        assert_eq!(
            result["source_grid_readiness"]["nodes"][1]["reason"], reason,
            "{result}"
        );
        assert_eq!(result["source_grid_readiness"]["nodes"][1]["ready"], false);
    }
}
#[tokio::test]
async fn truncated_diagnostics_keep_selected_and_all_reported_eligible_tuples() {
    let now = Instant::now();
    let selected = grid(64, 0);
    let mut other = ready_node(
        &(0..9).map(|n| grid(80 + n * 16, 0)).collect::<Vec<_>>(),
        now,
    );
    other.observe_raw_grid(selected, now);
    let (_, result) = start_nodes(
        vec![(3, ready_node(&[selected], now)), (5, other)],
        vec![3, 5],
        3,
    )
    .await;
    let node = &result["source_grid_readiness"]["nodes"][1];
    assert_eq!(node["reason"], "grid_evidence_warming");
    assert_eq!(node["observed_grids_truncated"], true);
    assert_eq!(node["eligible_grids_truncated"], true);
    let observed = node["observed_grids"].as_array().unwrap();
    let eligible = node["eligible_grids"].as_array().unwrap();
    assert!(observed.len() <= 8 && eligible.len() <= 8);
    assert_eq!(observed[0], result["source_grid"]);
    assert!(eligible.iter().all(|g| observed.contains(g)));
}
#[tokio::test]
async fn current_status_reports_grid_drift_without_rewriting_historical_contributions() {
    let now = Instant::now();
    let selected = grid(64, 0);
    let (state, result) = start_nodes(
        vec![
            (3, ready_node(&[selected], now)),
            (5, ready_node(&[selected], now)),
        ],
        vec![3, 5],
        3,
    )
    .await;
    assert_eq!(result["success"], true);
    {
        let mut inner = state.write().await;
        inner.calibration_observed_source_node_ids.extend([3, 5]);
        inner
            .node_states
            .insert(5, ready_node(&[grid(256, 1)], Instant::now()));
    }
    let Json(status) = calibration_status(State(state.clone())).await;
    assert_eq!(status["missing_source_node_ids"], serde_json::json!([]));
    assert_eq!(
        status["source_grid_readiness"]["nodes"][1]["reason"],
        "nonmatching_qualified_grid"
    );
    assert_eq!(
        status["source_grid_readiness"]["all_bound_nodes_ready"],
        false
    );
    assert_eq!(
        state.read().await.calibration_observed_source_node_ids,
        [3, 5].into_iter().collect()
    );
}
#[tokio::test]
async fn idle_status_does_not_expose_leftover_or_unbound_grid_diagnostics() {
    let mut inner = AppStateInner::minimal();
    let now = Instant::now();
    inner
        .node_states
        .insert(99, ready_node(&[grid(256, 1)], now));
    inner.calibration_grid_binding = Some(CalibrationGridBinding {
        source_node_id: 99,
        grid: grid(256, 1),
        evidence: None,
    });
    let Json(status) = calibration_status(State(Arc::new(RwLock::new(inner)))).await;
    assert!(status["source_grid_readiness"].is_null());
    assert_eq!(status["source_node_ids"], serde_json::json!([]));
}
fn current_identity(state: &AppStateInner) -> CalibrationIdentityRequest {
    CalibrationIdentityRequest {
        boot_epoch: state.calibration_boot_epoch.clone(),
        session_id: state.calibration_session_id.clone(),
        binding_digest: state.calibration_binding_digest.clone(),
        source_node_ids: Some(state.calibration_source_node_ids.iter().copied().collect()),
    }
}
#[tokio::test]
async fn post_start_known_grid_drift_blocks_only_new_finalization_and_preserves_capture() {
    let now = Instant::now();
    let selected = grid(64, 0);
    let different = grid(256, 1);
    let (state, started) = start_nodes(
        vec![
            (3, ready_node(&[selected], now)),
            (5, ready_node(&[selected], now)),
        ],
        vec![3, 5],
        3,
    )
    .await;
    assert_eq!(started["success"], true);
    let identity;
    {
        let mut inner = state.write().await;
        // Short SYNTHETIC model fixture, not a shortened production capture.
        let mut config = field_bridge::single_link_config();
        config.min_calibration_frames = 4;
        config.min_calibration_duration_s = 0.0;
        inner.field_model = Some(FieldModel::new(config).unwrap());
        for seq in 1..=4 {
            let values: Vec<_> = (0..64)
                .map(|bin| {
                    2.0 + f64::from(seq) * 0.04 + (bin as f64 * 0.3 + f64::from(seq)).sin() * 0.01
                })
                .collect();
            assert!(inner.maybe_feed_calibration_frame(3, selected, seq, &values, Instant::now()));
        }
        assert!(inner.maybe_feed_calibration_frame(5, selected, 1, &[2.0; 64], Instant::now()));
        inner.node_states.get_mut(&5).unwrap().raw_grid_observations =
            ready_node(&[different], Instant::now()).raw_grid_observations;
        identity = current_identity(&inner);
    }
    let Json(blocked) = calibration_stop(State(state.clone()), Json(identity)).await;
    assert_eq!(
        blocked["error_code"], "calibration_bound_grid_unavailable",
        "{blocked}"
    );
    {
        let inner = state.read().await;
        assert_eq!(
            inner.field_model.as_ref().unwrap().status(),
            CalibrationStatus::Collecting
        );
        assert_eq!(
            inner
                .field_model
                .as_ref()
                .unwrap()
                .calibration_frame_count(),
            4
        );
        assert!(inner.calibration_model_receipt.is_none());
        assert_eq!(
            inner.calibration_observed_source_node_ids,
            [3, 5].into_iter().collect()
        );
        assert_eq!(inner.node_states[&3].field_model_history.len(), 4);
        assert_eq!(inner.node_states[&5].field_model_history.len(), 1);
    }
    // A qualified common minority grid is sufficient at finalization too.
    let identity = {
        let mut inner = state.write().await;
        inner.node_states.get_mut(&5).unwrap().raw_grid_observations =
            ready_node(&[selected, different], Instant::now()).raw_grid_observations;
        current_identity(&inner)
    };
    let Json(finished) = calibration_stop(State(state.clone()), Json(identity)).await;
    assert_eq!(finished["success"], true, "{finished}");
    assert_eq!(
        state.read().await.field_model.as_ref().unwrap().status(),
        CalibrationStatus::Fresh
    );
    // Completed receipt/metadata calls do not acquire a new all-node readiness gate.
    let identity = {
        let mut inner = state.write().await;
        inner.node_states.get_mut(&5).unwrap().raw_grid_observations =
            ready_node(&[different], Instant::now()).raw_grid_observations;
        current_identity(&inner)
    };
    let Json(historical) = calibration_stop(State(state.clone()), Json(identity)).await;
    assert_ne!(
        historical["error_code"],
        "calibration_bound_grid_unavailable"
    );
    assert!(state.read().await.calibration_model_receipt.is_some());
}
#[tokio::test]
async fn warming_matching_grid_is_not_a_new_stop_incompatibility_gate() {
    let now = Instant::now();
    let selected = grid(64, 0);
    let (state, _) = start_nodes(
        vec![
            (3, ready_node(&[selected], now)),
            (5, ready_node(&[selected], now)),
        ],
        vec![3, 5],
        3,
    )
    .await;
    let identity = {
        let mut inner = state.write().await;
        assert!(inner.maybe_feed_calibration_frame(3, selected, 1, &[2.0; 64], Instant::now()));
        assert!(inner.maybe_feed_calibration_frame(5, selected, 1, &[2.0; 64], Instant::now()));
        let other = inner.node_states.get_mut(&5).unwrap();
        other.raw_grid_observations.clear();
        other.observe_raw_grid(selected, Instant::now());
        current_identity(&inner)
    };
    let Json(stopped) = calibration_stop(State(state), Json(identity)).await;
    assert_ne!(stopped["error_code"], "calibration_bound_grid_unavailable");
    assert!(stopped["error"].as_str().unwrap().contains("not complete"));
}
#[tokio::test]
async fn export_actual_legacy_grid_readiness_contract_cases() {
    let now = Instant::now();
    let selected = grid(64, 0);
    let dense = grid(256, 1);
    let (_, mixed) = start_nodes(
        vec![
            (3, ready_node(&[dense], now)),
            (5, ready_node(&[selected], now)),
        ],
        vec![3, 5],
        3,
    )
    .await;
    let (healthy_state, started) = start_nodes(
        vec![
            (3, ready_node(&[dense, selected], now)),
            (5, ready_node(&[selected], now)),
        ],
        vec![3, 5],
        5,
    )
    .await;
    assert_eq!(started["success"], true);
    let Json(healthy_status) = calibration_status(State(healthy_state.clone())).await;
    {
        let mut inner = healthy_state.write().await;
        inner.node_states.get_mut(&3).unwrap().raw_grid_observations =
            ready_node(&[dense], now).raw_grid_observations;
    }
    let Json(drift_status) = calibration_status(State(healthy_state)).await;
    for status in [&healthy_status, &drift_status] {
        assert_eq!(
            status["source_grid_readiness"]["author_source_node_id"],
            status["grid_binding"]["source_node_id"]
        );
        assert_eq!(
            status["source_grid_readiness"]["selected_grid"],
            status["grid_binding"]["grid"]
        );
    }

    let mut warming = NodeState::new();
    warming.observe_raw_grid(selected, now);
    let (_, warming_refusal) = start_nodes(
        vec![(3, ready_node(&[selected], now)), (5, warming)],
        vec![3, 5],
        3,
    )
    .await;
    let (_, stale_refusal) = start_nodes(
        vec![
            (3, ready_node(&[selected], now)),
            (5, ready_node(&[selected], now - Duration::from_secs(6))),
        ],
        vec![3, 5],
        3,
    )
    .await;
    let (_, missing_refusal) =
        start_nodes(vec![(3, ready_node(&[selected], now))], vec![3, 5], 3).await;
    let Json(idle_status) =
        calibration_status(State(Arc::new(RwLock::new(AppStateInner::minimal())))).await;
    let mut many = ready_node(
        &(0..9).map(|n| grid(80 + n * 16, 0)).collect::<Vec<_>>(),
        now,
    );
    many.observe_raw_grid(selected, now);
    let (_, bounded_refusal) = start_nodes(
        vec![(3, ready_node(&[selected], now)), (5, many)],
        vec![3, 5],
        3,
    )
    .await;
    let cases = serde_json::json!({"fixture_kind":"synthetic_legacy_grid_handlers","requested_source_node_ids":[3,5],"mixed_start_refusal":mixed,"healthy_start":started,"healthy_status":healthy_status,"drift_status":drift_status,"warming_start_refusal":warming_refusal,"stale_start_refusal":stale_refusal,"missing_start_refusal":missing_refusal,"idle_status":idle_status,"bounded_start_refusal":bounded_refusal});
    if let Ok(path) = std::env::var("RUVIEW_GRID_READINESS_FIXTURE_OUT") {
        std::fs::write(path, serde_json::to_vec_pretty(&cases).unwrap()).unwrap();
    }
}
