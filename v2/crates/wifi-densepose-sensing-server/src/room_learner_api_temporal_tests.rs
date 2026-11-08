//! Actual Axum extraction/dispatch controls; synthetic state only.
use super::super::temporal_routes::*;
use super::*;
use wifi_densepose_sensing_server::room_learner::temporal_wire::*;
fn temporal_app(state: SharedState) -> Router {
    Router::new()
        .route("/status", get(status_dispatch))
        .route("/selection", get(selection_dispatch))
        .route("/configure", post(configure_dispatch))
        .route("/phase/start", post(begin_dispatch))
        .route("/phase/finish", post(finish_dispatch))
        .route("/train", post(train_dispatch))
        .route("/evaluate", post(evaluate_dispatch))
        .route("/restore", post(restore_dispatch))
        .route("/discard", post(discard_dispatch))
        .route("/phase/cancel", post(pause_dispatch))
        .route("/train/cancel", post(cancel_operation_dispatch))
        .with_state(state)
}
async fn request_raw(
    app: Router,
    path: &str,
    body: Option<serde_json::Value>,
) -> (StatusCode, Vec<u8>) {
    let mut request = Request::builder().uri(path);
    let body = if let Some(body) = body {
        request = request
            .method(Method::POST)
            .header("content-type", "application/json");
        Body::from(serde_json::to_vec(&body).unwrap())
    } else {
        Body::empty()
    };
    let response = app.oneshot(request.body(body).unwrap()).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 65536)
        .await
        .unwrap();
    (status, bytes.to_vec())
}
async fn decoded(app: Router, path: &str, body: Option<serde_json::Value>) -> serde_json::Value {
    let (status, bytes) = request_raw(app, path, body).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    serde_json::from_slice(&bytes).unwrap()
}
fn configure_v2(expected: MutationGuard) -> serde_json::Value {
    serde_json::json!({"schema":CONFIGURE_REQUEST_SCHEMA,"model_kind":"ruview.room-learner.managed-temporal-input.v3",
        "expected":expected,"binding_digest":"b".repeat(64),"revision":1,"source_node_ids":[15,16,17],
        "new_training":true,"recovery_consent":true})
}
#[tokio::test]
async fn negotiated_idle_capability_preserves_default_status_and_selection() {
    let (_dir, state) = fixture();
    let app = temporal_app(state);
    let legacy = decoded(app.clone(), "/status", None).await;
    assert!(legacy.get("mutation_guard").is_some());
    assert!(legacy.get("available_model_kinds").is_none());
    let v2 = decoded(
        app.clone(),
        &format!("/status?schema={STATUS_SCHEMA}"),
        None,
    )
    .await;
    assert_eq!(v2["success"], true);
    assert_eq!(v2["status"]["schema"], STATUS_SCHEMA);
    assert_eq!(v2["status"]["workflow_kind"], "idle");
    assert_eq!(
        v2["status"]["model_selection"]["schema"],
        SELECTION_STATUS_SCHEMA
    );
    assert!(v2["status"]["model_selection"].get("success").is_none());
    assert_eq!(
        v2["status"]["available_model_kinds"],
        serde_json::json!([]),
        "Runtime without encrypted recovery must not advertise Temporal activation"
    );
    let selection = decoded(app, "/selection", None).await;
    assert_eq!(selection["schema"], "ruview.room-learner.selection.v1");
    assert_eq!(
        selection["progress_schema"],
        "ruview.room-learner.progress.v1"
    );
}
#[tokio::test]
async fn unknown_status_schema_fails_without_starting_or_reconfiguring() {
    let (_dir, state) = fixture();
    let rt = state.read().await.room_learning.clone().unwrap();
    let before = rt.mutation_guard().unwrap();
    let result = decoded(
        temporal_app(state),
        "/status?schema=ruview.room-learner.status.v999",
        None,
    )
    .await;
    assert_eq!(result["success"], false);
    assert_eq!(result["error_code"], "learner_schema_unsupported");
    assert_eq!(
        serde_json::to_value(rt.mutation_guard().unwrap()).unwrap(),
        serde_json::to_value(before).unwrap()
    );
    assert!(rt.context().is_none());
}
#[tokio::test]
async fn temporal_configure_unknown_kind_or_injected_authority_cannot_fall_back_to_legacy() {
    let (_dir, state) = fixture();
    let rt = state.read().await.room_learning.clone().unwrap();
    let app = temporal_app(state);
    let valid = configure_v2(rt.mutation_guard().unwrap());
    for (key, value) in [
        ("model_kind", serde_json::json!("legacy")),
        ("report", serde_json::json!({"passed":true})),
        ("verified", serde_json::json!(true)),
        ("fit_config", serde_json::json!({"epochs":1})),
    ] {
        let mut body = valid.clone();
        body[key] = value;
        let (status, _) = request_raw(app.clone(), "/configure", Some(body)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{key}");
        assert!(rt.context().is_none());
    }
    let mut unknown = valid;
    unknown["schema"] = serde_json::json!("configure.v999");
    let result = decoded(app, "/configure", Some(unknown)).await;
    assert_eq!(result["error_code"], "learner_schema_unsupported");
    assert!(rt.context().is_none());
}
#[tokio::test]
async fn temporal_configure_requires_exact_profile_and_current_explicit_consent() {
    let (_dir, state) = fixture();
    let rt = state.read().await.room_learning.clone().unwrap();
    let app = temporal_app(state);
    for (key, value, code) in [
        (
            "recovery_consent",
            serde_json::json!(false),
            "learner_recovery_consent_required",
        ),
        (
            "new_training",
            serde_json::json!(false),
            "learner_recovery_consent_required",
        ),
        (
            "source_node_ids",
            serde_json::json!([15, 16]),
            "learner_context_invalid",
        ),
        (
            "source_node_ids",
            serde_json::json!([17, 16, 15]),
            "learner_context_invalid",
        ),
    ] {
        let mut body = configure_v2(rt.mutation_guard().unwrap());
        body[key] = value;
        assert_eq!(
            decoded(app.clone(), "/configure", Some(body)).await["error_code"],
            code
        );
        assert!(rt.context().is_none());
    }
}
#[tokio::test]
async fn temporal_commands_reject_role_rows_scores_and_unknown_schema_without_legacy_fallback() {
    let (_dir, state) = fixture();
    let rt = state.read().await.room_learning.clone().unwrap();
    let app = temporal_app(state);
    let start = serde_json::json!({"schema":PHASE_START_REQUEST_SCHEMA,"expected":rt.mutation_guard().unwrap(),
        "capture_generation_id":"c".repeat(64),"schedule_sha256":"d".repeat(64),"episode_index":0,"conditions_confirmed":true});
    for (key, value) in [
        ("role", serde_json::json!("HeldOut")),
        ("label", serde_json::json!("Empty")),
        ("rows", serde_json::json!([])),
        ("verified", serde_json::json!(true)),
    ] {
        let mut body = start.clone();
        body[key] = value;
        assert_eq!(
            request_raw(app.clone(), "/phase/start", Some(body)).await.0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    let mut bad = start;
    bad["schema"] = serde_json::json!("start.v999");
    assert_eq!(
        decoded(app.clone(), "/phase/start", Some(bad)).await["error_code"],
        "learner_schema_unsupported"
    );
    let train = serde_json::json!({"schema":TRAIN_REQUEST_SCHEMA,"expected":rt.mutation_guard().unwrap(),"capture_generation_id":"c".repeat(64)});
    for key in ["config", "rows", "report", "score"] {
        let mut bad = train.clone();
        bad[key] = serde_json::json!({});
        assert_eq!(
            request_raw(app.clone(), "/train", Some(bad)).await.0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    assert!(rt.context().is_none());
}
#[tokio::test]
async fn negotiated_status_still_obeys_privacy_and_disabled_runtime() {
    let (_dir, state) = fixture();
    state.write().await.privacy_mode = true;
    let path = format!("/status?schema={STATUS_SCHEMA}");
    let result = decoded(temporal_app(state.clone()), &path, None).await;
    assert_eq!(result["error_code"], "learner_privacy_mode");
    assert!(result.get("status").is_none());
    let mut s = state.write().await;
    s.privacy_mode = false;
    s.room_learning = None;
    drop(s);
    assert_eq!(
        decoded(temporal_app(state), &path, None).await["error_code"],
        "learner_disabled"
    );
}

#[tokio::test]
async fn temporal_selection_is_read_scoped_and_unknown_schema_does_not_use_v1() {
    let (_dir, state) = fixture();
    let app = temporal_app(state);
    let result = decoded(
        app.clone(),
        &format!("/selection?schema={SELECTION_SCHEMA}"),
        None,
    )
    .await;
    assert_eq!(result["schema"], SELECTION_SCHEMA);
    let text = serde_json::to_string(&result).unwrap();
    for forbidden in [
        "owner_scope",
        "mutation_guard",
        "runtime_epoch",
        "encoder_weights",
        "raw_frames",
        "source_watermarks",
    ] {
        assert!(
            !text.contains(forbidden),
            "read projection leaked {forbidden}"
        );
    }
    let bad = decoded(app, "/selection?schema=selection.v999", None).await;
    assert_eq!(bad["error_code"], "learner_schema_unsupported");
    assert!(bad.get("model_selection").is_none());
}

#[tokio::test]
async fn temporal_selection_mutations_keep_the_existing_local_owner_gate() {
    let (_dir, state) = fixture();
    let app = owner_selection_routes(
        wifi_densepose_sensing_server::bearer_auth::AuthState::from_token("synthetic-owner"),
    )
    .with_state(state);
    for path in [
        "/api/v1/room-learner/qualify",
        "/api/v1/room-learner/select",
    ] {
        for (peer, token) in [
            ("127.0.0.1:41000", "synthetic-peer-read"),
            ("192.0.2.1:41000", "synthetic-owner"),
        ] {
            let mut request = Request::builder()
                .uri(path)
                .method(Method::POST)
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap();
            request.extensions_mut().insert(axum::extract::ConnectInfo(
                peer.parse::<SocketAddr>().unwrap(),
            ));
            assert_eq!(
                app.clone().oneshot(request).await.unwrap().status(),
                StatusCode::FORBIDDEN
            );
        }
    }
}

#[tokio::test]
async fn discard_and_pause_do_not_accept_ignored_context_or_new_phase_authority() {
    let (_dir, state) = fixture();
    let rt = state.read().await.room_learning.clone().unwrap();
    let app = temporal_app(state);
    let discard = serde_json::json!({"schema":RECOVERY_DISCARD_REQUEST_SCHEMA,"model_kind":"ruview.room-learner.managed-temporal-input.v3","expected":rt.mutation_guard().unwrap(),"capture_generation_id":"c".repeat(64)});
    for key in [
        "binding_digest",
        "revision",
        "source_node_ids",
        "consent",
        "rows",
    ] {
        let mut bad = discard.clone();
        bad[key] = serde_json::json!(null);
        assert_eq!(
            request_raw(app.clone(), "/discard", Some(bad)).await.0,
            StatusCode::UNPROCESSABLE_ENTITY,
            "discard:{key}"
        );
    }
    for (path, schema) in [
        ("/phase/cancel", PHASE_CANCEL_REQUEST_SCHEMA),
        ("/train/cancel", TRAIN_CANCEL_REQUEST_SCHEMA),
    ] {
        let action = serde_json::json!({"schema":schema,"expected":rt.mutation_guard().unwrap(),"capture_generation_id":"c".repeat(64)});
        for key in ["phase", "role", "new_training", "consent"] {
            let mut bad = action.clone();
            bad[key] = serde_json::json!(true);
            assert_eq!(
                request_raw(app.clone(), path, Some(bad)).await.0,
                StatusCode::UNPROCESSABLE_ENTITY,
                "{path}:{key}"
            );
        }
    }
    assert!(rt.context().is_none());
}

#[tokio::test]
async fn actual_temporal_configure_dispatches_queryless_phone_reads_without_owner_status() {
    use wifi_densepose_sensing_server::room_learner::recovery_store::RecoveryStore;
    let (dir, state) = fixture();
    let owner = "a".repeat(64);
    let path = dir.path().join(&owner).join("SensingServer");
    let store = RecoveryStore::new(&path, &owner, "synthetic-install", Some(&[7; 32])).unwrap();
    let rt = Arc::new(
        RoomLearningRuntime::new_with_recovery(&path, &owner, "synthetic-install", store).unwrap(),
    );
    state.write().await.room_learning = Some(rt.clone());
    let app = temporal_app(state.clone());
    let capable = decoded(
        app.clone(),
        &format!("/status?schema={STATUS_SCHEMA}"),
        None,
    )
    .await;
    assert_eq!(
        capable["status"]["available_model_kinds"],
        serde_json::json!(["ruview.room-learner.managed-temporal-input.v3"])
    );
    let configured = decoded(
        app.clone(),
        "/configure",
        Some(configure_v2(rt.mutation_guard().unwrap())),
    )
    .await;
    assert_eq!(configured["success"], true, "{configured}");
    for _ in 0..200 {
        let _ = decoded(
            app.clone(),
            &format!("/status?schema={STATUS_SCHEMA}"),
            None,
        )
        .await;
        if rt.temporal_v2_state_present() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(
        rt.temporal_v2_state_present(),
        "actual full run must be admitted"
    );
    let selected = decoded(app.clone(), "/selection", None).await;
    assert_eq!(selected["schema"], SELECTION_SCHEMA);
    assert_eq!(selected["progress_schema"], PAIRED_PROGRESS_SCHEMA);
    assert!(selected.get("status").is_none());
    assert!(!selected.to_string().contains("mutation_guard"));
    let progress = Router::new()
        .route(
            "/progress",
            get(super::super::paired_progress::progress_dispatch),
        )
        .with_state(state);
    let (status, body) = request_raw(progress, "/progress", None).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["schema"], PAIRED_PROGRESS_SCHEMA);
    assert_eq!(body["error_code"], "publication_unavailable");
    assert!(body.get("mutation_guard").is_none());
}

#[tokio::test]
async fn queryless_idle_progress_remains_v1_and_unknown_query_cannot_activate() {
    let (_dir, state) = fixture();
    let rt = state.read().await.room_learning.clone().unwrap();
    let app = Router::new()
        .route(
            "/progress",
            get(super::super::paired_progress::progress_dispatch),
        )
        .with_state(state);
    let (status, body) = request_raw(app.clone(), "/progress", None).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap()["schema"],
        "ruview.room-learner.progress.v1"
    );
    let (status, body) = request_raw(app, "/progress?schema=progress.v999", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap()["error_code"],
        "learner_schema_unsupported"
    );
    assert!(rt.context().is_none());
}
