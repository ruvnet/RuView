//! SYNTHETIC public API fixture; no manufactured model/qualification receipts.
use super::*;

pub(super) fn update() -> SensingUpdate {
    serde_json::from_value(serde_json::json!({
        "type":"sensing_update","timestamp":chrono::Utc::now().timestamp_millis() as f64/1000.0,
        "source":"esp32","tick":321,"nodes":[],
        "features":{"mean_rssi":-50.0,"variance":1.0,"motion_band_power":2.0,
            "breathing_band_power":1.0,"dominant_freq_hz":0.2,"change_points":0,"spectral_power":3.0},
        "classification":{"motion_level":"present_moving","presence":true,"confidence":0.99},
        "signal_field":{"grid_size":[1,1,1],"values":[1.0]},
        "estimated_persons":3,
        "vital_signs":{"breathing_rate_bpm":14.0,"heart_rate_bpm":62.0,
            "breathing_confidence":0.8,"heartbeat_confidence":0.7,"signal_quality":0.9},
        "pose_keypoints":[[0.1,0.2,0.0,0.9]],"enhanced_motion":{"level":"moving"},
        "enhanced_breathing":{"rate_bpm":14.0},"posture":"standing"
    })).unwrap()
}

fn gain(label: RoomLabel) -> f64 {
    match label {
        RoomLabel::Empty => 2.0,
        RoomLabel::OccupiedStatic => 6.0,
        RoomLabel::OccupiedMoving => 7.0,
        RoomLabel::AdjacentOnly => 2.3,
        RoomLabel::Unknown => unreachable!(),
    }
}
pub(super) fn feed(rt: &RoomLearningRuntime, grid: &str, label: RoomLabel, sequence: u32, at: u64) {
    for sample in 0..WINDOW_PACKETS {
        for source in [15u8, 16] {
            let values: Vec<_> = (0..56)
                .map(|bin| {
                    gain(label)
                        + f64::from(source - 14) * 0.3
                        + bin as f64 * 0.04
                        + sample as f64 * 0.003
                })
                .collect();
            rt.ingest(
                RawSourceFrame {
                    source_id: source,
                    grid_id: grid,
                    frequency_hz: 2_437_000_000,
                    bandwidth_hz: 20_000_000,
                    sequence: sequence + sample as u32,
                    amplitudes: &values,
                },
                at + sample as u64 * 1_000_000 + u64::from(source),
            )
            .unwrap();
        }
    }
}
fn phase(
    rt: &RoomLearningRuntime,
    grid: &str,
    role: DataRole,
    label: RoomLabel,
    index: u32,
    base: u64,
) {
    let at = base + u64::from(index) * 13_000_000_000;
    rt.begin_phase(
        &rt.mutation_guard().unwrap(),
        PhaseRequest {
            trial_id: format!("synthetic-composer-trial-{index}"),
            session_id: format!("synthetic-composer-session-{index}"),
            role,
            label,
            duration_ms: MIN_PHASE_MS,
            lead_in_ms: 0,
            conditions_confirmed: true,
        },
        at,
    )
    .unwrap();
    for quarter in 0..4 {
        feed(
            rt,
            grid,
            label,
            index * 32 + quarter * 8 + 1,
            at + u64::from(quarter) * 3_000_000_000 + 1_000_000,
        );
    }
    rt.finish_phase(
        &rt.mutation_guard().unwrap(),
        at + MIN_PHASE_MS * 1_000_000,
        true,
    )
    .unwrap();
}
pub(super) async fn owner_post(
    state: SharedState,
    path: &str,
    body: serde_json::Value,
) -> serde_json::Value {
    let app = owner_selection_routes(
        wifi_densepose_sensing_server::bearer_auth::AuthState::from_token("synthetic-owner"),
    )
    .with_state(state);
    let mut request = Request::builder()
        .method(Method::POST)
        .uri(path)
        .header("content-type", "application/json")
        .header("authorization", "Bearer synthetic-owner")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    request.extensions_mut().insert(axum::extract::ConnectInfo(
        "127.0.0.1:41000".parse::<SocketAddr>().unwrap(),
    ));
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 65536)
            .await
            .unwrap(),
    )
    .unwrap()
}

pub(super) async fn fitted() -> (
    tempfile::TempDir,
    SharedState,
    Arc<RoomLearningRuntime>,
    String,
) {
    let (dir, state) = fixture();
    let room =
        companion_room::SharedRoomStore::load(&dir.path().join("companion"), "synthetic-install")
            .unwrap();
    let room_json = serde_json::json!({"schema":"ruview.companion.room.v1","id":"synthetic-room","name":"Test room",
        "widthM":2.5,"lengthM":2.5,"heightM":2.8,"nodes":[
        {"nodeId":15,"slot":"north_west","xM":0,"yM":0,"role":"receiver","headingDeg":90,"orientationConfirmed":true,"confirmedAtUnixMs":1},
        {"nodeId":16,"slot":"north_east","xM":2.5,"yM":0,"role":"receiver","headingDeg":180,"orientationConfirmed":true,"confirmedAtUnixMs":1}],"accessPoint":{"slot":"south_center","xM":1.25,"yM":2.5,"provenance":"operator_entered","confirmedAtUnixMs":1}});
    let response = companion_room::owner_router(room.clone())
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/v1/companion/room")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(
                        &serde_json::json!({"expected_revision":0,"room":room_json}),
                    )
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let published: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 65536)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(published["revision"], 1);
    let binding = room.current_binding_digest().unwrap().unwrap();
    state.write().await.current_room_store = Some(room);
    let configured = json_request(
        app(state.clone()),
        "/configure",
        Method::POST,
        serde_json::json!({
        "binding_digest":binding,"revision":7001,"source_node_ids":[15,16],"new_training":true}),
    )
    .await;
    assert_eq!(configured["success"], true, "{configured}");
    // Learner config generation and companion publication revision are separate
    // domains. Only the full shared binding digest joins their authority.
    assert_eq!(configured["status"]["revision"], 7001);

    let rt = state.read().await.room_learning.as_ref().unwrap().clone();
    let grid = rt.context().unwrap().sources[0].grid_id.clone();
    let base = rt.receiver_now_ns() - 200_000_000_000;
    let labels = [
        RoomLabel::Empty,
        RoomLabel::OccupiedStatic,
        RoomLabel::OccupiedMoving,
        RoomLabel::AdjacentOnly,
    ];
    for (i, label) in labels.into_iter().enumerate() {
        phase(&rt, &grid, DataRole::Train, label, i as u32, base)
    }
    // Small real optimizer fixture; production defaults remain unchanged.
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
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(90);
    loop {
        let status = rt.status(rt.receiver_now_ns());
        if status["state"] == "ready" {
            break;
        }
        assert!(status["state"] != "failed", "{status}");
        assert!(
            std::time::Instant::now() < deadline,
            "bounded synthetic fit timed out"
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    for (i, label) in labels.into_iter().enumerate() {
        phase(&rt, &grid, DataRole::HeldOut, label, i as u32 + 4, base)
    }
    rt.evaluate(&rt.mutation_guard().unwrap(), DataRole::HeldOut)
        .unwrap();
    // Refresh the actual parser/readiness cache. Then admit labelled-value
    // synthetic live frames with fresh sequence IDs through the public runtime.
    let now = std::time::Instant::now();
    {
        let mut inner = state.write().await;
        for row in 0..120 {
            for node in [15u8, 16] {
                let packet = wire(node, 500 + row);
                let frame = parse_esp32_frame(&packet).unwrap();
                let at = now - std::time::Duration::from_millis((120 - row) as u64 * 100);
                observe(&mut inner, &frame, &packet, at);
            }
        }
        for node in [15u8, 16] {
            inner.node_states.get_mut(&node).unwrap().last_frame_time =
                Some(std::time::Instant::now());
        }
        inner.source = "esp32".into();
        inner.last_esp32_frame = Some(std::time::Instant::now());
    }
    feed(
        &rt,
        &grid,
        RoomLabel::OccupiedStatic,
        1000,
        rt.receiver_now_ns(),
    );
    tokio::time::sleep(std::time::Duration::from_millis(12)).await;
    let q = owner_post(
        state.clone(),
        "/api/v1/room-learner/qualify",
        serde_json::json!({"expected":rt.mutation_guard().unwrap()}),
    )
    .await;
    assert_eq!(q["success"], true, "{q}");
    let selected=owner_post(state.clone(),"/api/v1/room-learner/select",serde_json::json!({"expected":rt.mutation_guard().unwrap(),
        "model_sha256":q["qualification"]["receipt"]["model_sha256"],"qualification_sha256":q["qualification"]["receipt_sha256"]})).await;
    assert_eq!(selected["success"], true, "{selected}");
    (dir, state, rt, binding)
}
