//! Offline RF is not loss of authenticated owner metadata authority.
//! Synthetic temporary data only; no device or broker operations.
use super::*;

async fn cancel_via_authenticated_router(
    state: SharedState,
    path: &str,
    guard: MutationGuard,
    token: &str,
) -> (StatusCode, serde_json::Value) {
    let router = Router::new()
        .route("/api/v1/room-learner/phase/cancel", post(cancel_phase))
        .route("/api/v1/room-learner/train/cancel", post(cancel_training))
        .with_state(state)
        .layer(axum::middleware::from_fn_with_state(
            wifi_densepose_sensing_server::bearer_auth::AuthState::from_token("synthetic-owner"),
            wifi_densepose_sensing_server::bearer_auth::require_bearer,
        ));
    let response = router
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(path)
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({"expected":guard})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let code = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 65536)
        .await
        .unwrap();
    (
        code,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

#[tokio::test]
async fn offline_owner_can_cancel_then_deselect_but_cannot_reset_corrupt_pointer() {
    let (dir, state, rt, _binding) = fitted().await;
    let started = begin_phase(
        State(state.clone()),
        Json(BeginRequest {
            expected: rt.mutation_guard().unwrap(),
            phase: PhaseRequest {
                trial_id: "synthetic-offline-cancel".into(),
                session_id: "synthetic-offline-session".into(),
                role: DataRole::HeldOut,
                label: RoomLabel::Empty,
                duration_ms: 12000,
                lead_in_ms: 0,
                conditions_confirmed: true,
            },
        }),
    )
    .await
    .0;
    assert_eq!(started["success"], true, "{started}");
    assert!(rt.capture_active().unwrap());
    // Readiness requires a native arrival within two seconds. Let the actual
    // observed metadata become stale; do not replace its clock or forge flags.
    tokio::time::sleep(std::time::Duration::from_millis(2100)).await;
    {
        let mut inner = state.write().await;
        inner.last_esp32_frame = None;
        assert!(rt.context().unwrap().sources.iter().all(|s| !source_ready(
            &inner,
            s,
            std::time::Instant::now()
        )));
    }
    let grid = rt.context().unwrap().sources[0].grid_id.clone();
    rt.record_native_rejection(16, &grid, "synthetic RF interruption");
    let metadata = status(State(state.clone())).await.0;
    assert_eq!(metadata["enabled"], true);
    assert!(metadata["mutation_guard"].is_object());
    assert!(metadata["model_selection"]["selected"].is_object());
    assert_eq!(
        metadata["model_selection"]["current"]["status"],
        "unavailable"
    );
    let guard = rt.mutation_guard().unwrap();
    let (code, _) = cancel_via_authenticated_router(
        state.clone(),
        "/api/v1/room-learner/phase/cancel",
        guard.clone(),
        "synthetic-peer",
    )
    .await;
    assert_ne!(
        code,
        StatusCode::OK,
        "loss of RF must not broaden actor rights"
    );
    assert!(rt.capture_active().unwrap());
    let (code, cancelled) = cancel_via_authenticated_router(
        state.clone(),
        "/api/v1/room-learner/phase/cancel",
        guard,
        "synthetic-owner",
    )
    .await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(cancelled["success"], true, "{cancelled}");
    assert!(!rt.capture_active().unwrap());
    let known = status(State(state.clone())).await.0;
    let deselected = owner_post(
        state.clone(),
        "/api/v1/room-learner/deselect",
        serde_json::json!({"expected":known["mutation_guard"]}),
    )
    .await;
    assert_eq!(deselected["success"], true, "{deselected}");
    assert!(deselected["selection"]["receipt"]["selected"].is_null());
    assert_eq!(deselected["selection"]["receipt"]["revision"], 2);

    // Unknown disk state is different from absent RF. Do not guess a pointer
    // revision or overwrite the file merely to acknowledge a metadata action.
    let pointer = dir
        .path()
        .join("a".repeat(64))
        .join("SensingServer/room-learner/model-selection-v1.json");
    assert!(pointer.is_file());
    let corrupt = b"{ synthetic corrupt selection pointer";
    std::fs::write(&pointer, corrupt).unwrap();
    let refused = owner_post(
        state.clone(),
        "/api/v1/room-learner/deselect",
        serde_json::json!({"expected":rt.mutation_guard().unwrap()}),
    )
    .await;
    assert_eq!(refused["success"], false, "{refused}");
    assert_eq!(refused["error_code"], "learner_deselection_rejected");
    assert_eq!(std::fs::read(pointer).unwrap(), corrupt);
}

#[tokio::test]
async fn authenticated_training_cancel_does_not_require_live_native_ingress() {
    let (_dir, state) = fixture();
    let configured=json_request(app(state.clone()),"/configure",Method::POST,serde_json::json!({
        "binding_digest":"b".repeat(64),"revision":7001,"source_node_ids":[15,16],"new_training":true})).await;
    assert_eq!(configured["success"], true, "{configured}");
    let rt = state.read().await.room_learning.as_ref().unwrap().clone();
    let grid = rt.context().unwrap().sources[0].grid_id.clone();
    let base = rt.receiver_now_ns() - 100_000_000_000;
    for (i, label) in [
        RoomLabel::Empty,
        RoomLabel::OccupiedStatic,
        RoomLabel::OccupiedMoving,
        RoomLabel::AdjacentOnly,
    ]
    .into_iter()
    .enumerate()
    {
        fixture_support::phase(&rt, &grid, DataRole::Train, label, i as u32, base);
    }
    rt.start_training(
        &rt.mutation_guard().unwrap(),
        FitConfig {
            epochs: 8,
            learning_rate: 0.5,
            hidden_dim: 2,
            embedding_dim: 2,
            temporal_tokens: 2,
            ..FitConfig::default()
        },
    )
    .unwrap();
    state.write().await.room_learning_sources.clear();
    let guard = rt.mutation_guard().unwrap();
    let (code, _) = cancel_via_authenticated_router(
        state.clone(),
        "/api/v1/room-learner/train/cancel",
        guard.clone(),
        "synthetic-peer",
    )
    .await;
    assert_ne!(code, StatusCode::OK);
    let (code, body) = cancel_via_authenticated_router(
        state.clone(),
        "/api/v1/room-learner/train/cancel",
        guard,
        "synthetic-owner",
    )
    .await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(body["success"], true, "{body}");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let snapshot = rt.status(rt.receiver_now_ns());
        if snapshot["state"] == "cancelled" {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "cancelled training did not settle: {snapshot}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
}
