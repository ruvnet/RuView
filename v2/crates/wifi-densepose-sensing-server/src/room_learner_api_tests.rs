//! @tier durable — SYNTHETIC datagrams through real configure/parser/observe.
//! Receiver-observed fixtures are not physical hardware or accuracy evidence.
use super::*;
use axum::{
    body::Body,
    http::{Method, Request},
};
use tower::ServiceExt;
fn wire(node: u8, seq: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend(0xC5110001u32.to_le_bytes());
    bytes.extend([node, 1]);
    bytes.extend(64u16.to_le_bytes());
    bytes.extend(2437u32.to_le_bytes());
    bytes.extend(seq.to_le_bytes());
    bytes.extend([(-40i8) as u8, (-90i8) as u8, 0, 0x20]);
    for _ in 0..64 {
        bytes.extend([3, 4]);
    }
    bytes
}
fn fixture() -> (tempfile::TempDir, SharedState) {
    let dir = tempfile::tempdir().unwrap();
    let owner = "a".repeat(64);
    let runtime = Arc::new(
        RoomLearningRuntime::new(
            &dir.path().join(&owner).join("SensingServer"),
            &owner,
            "synthetic-install",
        )
        .unwrap(),
    );
    let mut inner = AppStateInner::minimal();
    inner.room_learning = Some(runtime);
    let now = std::time::Instant::now();
    for row in 0..120 {
        for node in [15, 16, 17] {
            let packet = wire(node, row + 1);
            let frame = parse_esp32_frame(&packet).unwrap();
            let at = now - std::time::Duration::from_millis((120 - row) as u64 * 100);
            inner
                .node_states
                .entry(node)
                .or_insert_with(NodeState::new)
                .observe_raw_grid(CsiGridKey::from_frame(&frame), at);
            observe(&mut inner, &frame, &packet, at);
        }
    }
    (dir, Arc::new(RwLock::new(inner)))
}
fn app(state: SharedState) -> Router {
    Router::new()
        .route("/configure", post(configure))
        .route("/status", get(status))
        .route("/phase", post(begin_phase))
        .route("/finish", post(finish_phase))
        .with_state(state)
}
async fn json_request(
    app: Router,
    path: &str,
    method: Method,
    mut body: serde_json::Value,
) -> serde_json::Value {
    if method == Method::POST && body.get("expected").is_none() {
        let current = json_request_status(app.clone()).await;
        if path == "/phase" {
            body = serde_json::json!({"phase":body});
        }
        body["expected"] = if current["mutation_guard"].is_object() {
            current["mutation_guard"].clone()
        } else {
            serde_json::json!({"runtime_epoch":"synthetic-disabled","generation":0,"workflow_revision":0,"context_digest":null,"phase":null})
        };
    }
    let response = app
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(response.into_body(), 65536)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
async fn json_request_status(app: Router) -> serde_json::Value {
    let response = app
        .oneshot(
            Request::builder()
                .uri("/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(response.into_body(), 65536)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
fn config() -> serde_json::Value {
    serde_json::json!({"binding_digest":"b".repeat(64),"revision":1,"source_node_ids":[15,16,17],"new_training":true})
}
#[tokio::test]
async fn configure_and_actual_native_ingress_share_grid_projection_and_device_identity() {
    let (_dir, state) = fixture();
    let router = app(state.clone());
    let configured = json_request(router.clone(), "/configure", Method::POST, config()).await;
    assert_eq!(configured["success"], true, "{configured}");
    let sources = configured["status"]["sources"].as_array().unwrap();
    assert_eq!(sources.len(), 3);
    assert_eq!(sources[0]["device_id"], "esp32-node-15");
    assert_eq!(sources[0]["grid_id"], "ant1-bins64-ppdu0-usable-drop2-v1");
    let begun=json_request(router.clone(),"/phase",Method::POST,serde_json::json!({
        "trial_id":"synthetic-empty","session_id":"synthetic-train-session","role":"Train","label":"Empty",
        "duration_ms":12000,"conditions_confirmed":true})).await;
    assert_eq!(begun["success"], true, "{begun}");
    for row in 0..8 {
        for node in [15, 16, 17] {
            let packet = wire(node, 121 + row);
            let frame = parse_esp32_frame(&packet).unwrap();
            let mut inner = state.write().await;
            observe(&mut inner, &frame, &packet, std::time::Instant::now());
        }
    }
    let read = json_request(
        router.clone(),
        "/status",
        Method::GET,
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(read["required_source_ids"], serde_json::json!([15, 16, 17]));
    assert_eq!(read["trained_source_ids"], serde_json::json!([]));
    assert_eq!(read["scored_source_ids"], serde_json::json!([]));
    for source in read["capture"]["sources"].as_array().unwrap() {
        assert_eq!(source["admitted_packets"], 8, "{source}");
        assert_eq!(source["live_buffer_frames"], 8);
    }
    assert_eq!(read["physically_validated"], false);
    assert!(
        state.read().await.field_model.is_none(),
        "new learner must not silently create/change control calibration"
    );
    let incomplete = json_request(
        router,
        "/finish",
        Method::POST,
        serde_json::json!({"conditions_held":true}),
    )
    .await;
    assert_eq!(
        incomplete["success"], false,
        "wall-clock duration cannot be bypassed by fast synthetic packets"
    );
}
#[tokio::test]
async fn privacy_disabled_source_drop_and_unconfirmed_conditions_are_explicit() {
    let empty = Arc::new(RwLock::new(AppStateInner::minimal()));
    let disabled = json_request(app(empty), "/status", Method::GET, serde_json::Value::Null).await;
    assert_eq!(disabled["error_code"], "learner_disabled");
    let (_dir, state) = fixture();
    state.write().await.privacy_mode = true;
    let hidden = json_request(app(state.clone()), "/configure", Method::POST, config()).await;
    assert_eq!(hidden["error_code"], "learner_privacy_mode");
    assert!(!hidden.to_string().contains("owner_scope"));
    state.write().await.privacy_mode = false;
    state.write().await.node_states.remove(&17);
    let missing = json_request(app(state.clone()), "/configure", Method::POST, config()).await;
    assert_eq!(missing["error_code"], "learner_source_missing");
    assert!(state
        .read()
        .await
        .room_learning
        .as_ref()
        .unwrap()
        .context()
        .is_none());
}
#[test]
fn every_new_mutation_requires_admin_and_default_cli_does_not_enable_capture() {
    use wifi_densepose_sensing_server::bearer_auth::required_scope_for;
    for suffix in [
        "configure",
        "phase/start",
        "phase/finish",
        "phase/cancel",
        "train",
        "train/cancel",
        "evaluate",
    ] {
        assert_eq!(
            required_scope_for(&Method::POST, &format!("/api/v1/room-learner/{suffix}")),
            "sensing:admin"
        );
    }
    assert_eq!(
        required_scope_for(&Method::GET, "/api/v1/room-learner/status"),
        "sensing:read"
    );
    assert!(
        !Args::try_parse_from(["sensing-server"])
            .unwrap()
            .room_learner
    );
}

#[tokio::test]
async fn learner_examples_and_legacy_empty_capture_cannot_contaminate_each_other() {
    let (_dir, state) = fixture();
    let router = app(state.clone());
    assert_eq!(
        json_request(router.clone(), "/configure", Method::POST, config()).await["success"],
        true
    );
    {
        let mut s = state.write().await;
        s.field_model = Some(FieldModel::new(field_bridge::single_link_config()).unwrap());
        s.calibration_session_id = Some("cal-session-synthetic".into());
    }
    let request = serde_json::json!({"trial_id":"occupied","session_id":"train","role":"Train",
        "label":"OccupiedStatic","duration_ms":12000,"lead_in_ms":5000,"conditions_confirmed":true});
    let blocked = json_request(router.clone(), "/phase", Method::POST, request.clone()).await;
    assert_eq!(blocked["error_code"], "legacy_empty_capture_active");
    {
        let mut s = state.write().await;
        s.field_model = None;
        s.calibration_session_id = None;
    }
    assert_eq!(
        json_request(router, "/phase", Method::POST, request).await["success"],
        true
    );
    let Json(blocked) = calibration_start(
        State(state.clone()),
        Query(CalibrationStartQuery {
            source_node_id: None,
        }),
        None,
        None,
        Json(CalibrationBindingRequest {
            binding_digest: "b".repeat(64),
            source_node_ids: vec![15, 16, 17],
        }),
    )
    .await;
    assert_eq!(blocked["error_code"], "learner_capture_active");
    assert!(state.read().await.field_model.is_none());
}

#[tokio::test]
async fn stale_http_start_is_rejected_after_another_client_changes_room() {
    let (_dir, state) = fixture();
    let router = app(state);
    let a = json_request(router.clone(), "/configure", Method::POST, config()).await;
    assert_eq!(a["success"], true);
    let guard_a = a["status"]["mutation_guard"].clone();
    let mut desired_b = config();
    desired_b["binding_digest"] = serde_json::json!("c".repeat(64));
    desired_b["expected"] = guard_a.clone();
    let b = json_request(router.clone(), "/configure", Method::POST, desired_b).await;
    assert_eq!(b["success"], true, "{b}");
    let delayed = serde_json::json!({"expected":guard_a,"phase":{
        "trial_id":"delayed-room-a","session_id":"room-a","role":"Train","label":"Empty",
        "duration_ms":12000,"conditions_confirmed":true}});
    let rejected = json_request(router.clone(), "/phase", Method::POST, delayed).await;
    assert_eq!(
        rejected["error_code"], "learner_state_changed",
        "{rejected}"
    );
    let current = json_request_status(router).await;
    assert_eq!(current["mutation_guard"], b["status"]["mutation_guard"]);
    assert!(current["capture"]["phase"].is_null());
}
