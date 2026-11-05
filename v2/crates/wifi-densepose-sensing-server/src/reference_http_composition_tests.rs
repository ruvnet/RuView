//! SYNTHETIC actual HTTP -> managed phase -> same-pass RF comparison.
//! In-memory source frames are test fixtures, never physical reference evidence.
use super::*;
use wifi_densepose_sensing_server::room_learner::{
    capture::{PhaseRequest, RawSourceFrame},
    observation::{DataRole, RoomLabel},
    training::FitConfig,
};

async fn owner_json(
    router: Router,
    method: Method,
    path: &str,
    value: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("authorization", "Bearer synthetic-owner")
        .header("content-type", "application/json")
        .body(Body::from(value.to_string()))
        .unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo("127.0.0.1:1".parse::<SocketAddr>().unwrap()));
    let response = router.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 65536)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}
async fn peer_json(
    router: Router,
    token: &str,
    value: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let response = router
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/companion/v1/calibration-references")
                .header("x-ruview-installation-id", "synthetic-install")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(value.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 8192)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}
fn begin_phase(rt: &RoomLearningRuntime, role: DataRole, label: RoomLabel, index: u32, at: u64) {
    rt.begin_phase(
        &rt.mutation_guard().unwrap(),
        PhaseRequest {
            trial_id: format!("r1-http-trial-{index}"),
            session_id: format!("r1-http-session-{index}"),
            role,
            label,
            duration_ms: 12000,
            lead_in_ms: 0,
            conditions_confirmed: true,
        },
        at,
    )
    .unwrap_or_else(|error| panic!("synthetic phase {index} {role:?} at {at}: {error}"));
}
fn feed_quarter(
    rt: &RoomLearningRuntime,
    label: RoomLabel,
    index: u32,
    quarter: u32,
    first: u64,
    last: u64,
) {
    let gain = match label {
        RoomLabel::Empty => 2.,
        RoomLabel::OccupiedStatic => 6.,
        RoomLabel::OccupiedMoving => 7.,
        RoomLabel::AdjacentOnly => 2.3,
        RoomLabel::Unknown => unreachable!(),
    };
    for sample in 0..8u32 {
        for source in 1..=2u8 {
            let values: Vec<f64> = (0..56)
                .map(|bin| {
                    gain + f64::from(source) * 0.3
                        + f64::from(bin) * 0.04
                        + f64::from(sample) * 0.003
                })
                .collect();
            rt.ingest(
                RawSourceFrame {
                    source_id: source,
                    grid_id: "ant1-bins64-ppdu0-usable-drop2-linear-v2",
                    frequency_hz: 2_437_000_000,
                    bandwidth_hz: 20_000_000,
                    sequence: 2000 + index * 100 + quarter * 8 + sample,
                    amplitudes: &values,
                },
                first + (last - first) * u64::from(sample) / 7,
            )
            .unwrap();
        }
    }
}

#[tokio::test]
async fn owner_consent_paired_http_admission_and_owner_evaluation_compose_without_teacher_authority(
) {
    let (dir, state, publication) = fixture().await;
    let rt = state.read().await.room_learning.clone().unwrap();
    let published = publication.snapshot().unwrap();
    let binding = published.binding_digest.clone().unwrap();
    let owner_scope = "a".repeat(64);
    let context = CaptureContext {
        owner_scope: owner_scope.clone(),
        installation_id: "synthetic-install".into(),
        room_digest: binding.clone(),
        revision: 77,
        sources: (1..=2)
            .map(|id| SourceSpec {
                source_id: id,
                device_id: format!("esp32-node-{id}"),
                grid_id: "ant1-bins64-ppdu0-usable-drop2-linear-v2".into(),
                feature_count: 56,
                signal_kind: ruview_unified::RfModality::WifiCsi,
                center_frequency_hz: 2_437_000_000,
                bandwidth_hz: 20_000_000,
                link_id: format!("synthetic-link-{id}"),
                link_index: 0,
                geometry: None,
            })
            .collect(),
    };
    rt.configure(
        &rt.mutation_guard().unwrap(),
        context,
        "r1-http-fitted-run".into(),
        true,
    )
    .unwrap();
    let train_start = rt.receiver_now_ns().saturating_sub(100_000_000_000);
    for (index, label) in [
        RoomLabel::Empty,
        RoomLabel::OccupiedStatic,
        RoomLabel::OccupiedMoving,
        RoomLabel::AdjacentOnly,
    ]
    .into_iter()
    .enumerate()
    {
        let at = train_start + index as u64 * 13_000_000_000;
        begin_phase(&rt, DataRole::Train, label, index as u32, at);
        for quarter in 0..4u32 {
            let first = at + u64::from(quarter) * 3_000_000_000 + 1_000_000;
            feed_quarter(&rt, label, index as u32, quarter, first, first + 70_000_000);
        }
        rt.finish_phase(&rt.mutation_guard().unwrap(), at + 12_000_000_000, true)
            .unwrap();
    }
    rt.start_training(
        &rt.mutation_guard().unwrap(),
        FitConfig {
            epochs: 2,
            ..FitConfig::default()
        },
    )
    .unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let status = rt.status(rt.receiver_now_ns());
            if status["state"] == "ready" {
                break;
            }
            assert_ne!(status["state"], "failed", "{status}");
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .expect("bounded synthetic fit did not finish");

    let service = Companion::load_for_account(
        &dir.path().join("r1-http-pair"),
        "synthetic-install".into(),
        "f".repeat(64),
        "https://synthetic.local:3443".into(),
        Some(owner_scope),
    )
    .unwrap();
    let token = pair(service.clone(), "phone").await;
    let references =
        wifi_densepose_sensing_server::companion_references::ReferenceStore::new(service.clone());
    state.write().await.calibration_references = Some(references.clone());
    let api = companion_reference_api::ReferenceApi {
        state: state.clone(),
        room: publication.clone(),
        references: references.clone(),
    };
    let peer = companion_reference_api::peer_router(api.clone()).layer(
        axum::middleware::from_fn_with_state(service.clone(), require_companion),
    );
    let owner = companion_reference_api::owner_router(api)
        .merge(
            Router::new()
                .route(
                    "/api/v1/room-learner/evaluate",
                    post(room_learner_api::evaluate),
                )
                .with_state(state.clone()),
        )
        .layer(axum::middleware::from_fn_with_state(
            AuthState::from_token("synthetic-owner"),
            require_owner,
        ));
    assert!(
        service
            .authorize("synthetic-install", &token, true)
            .is_err(),
        "references must not grant calibration control"
    );
    let (_, permit) = owner_json(
        owner.clone(),
        Method::POST,
        "/api/v1/companion/devices/phone/references",
        serde_json::json!({"record_comparison_evidence":true}),
    )
    .await;
    assert_eq!(permit["recording_consented"], true);
    let at = rt.receiver_now_ns();
    begin_phase(&rt, DataRole::HeldOut, RoomLabel::Empty, 4, at);
    let lineage = serde_json::json!({"capture_session_id":"r1-http-capture","coordinate_frame_id":"r1-http-frame","clock_model_id":"arkit-monotonic+session-wall-offset-v1","room_binding_digest":binding,"room_revision":published.revision});
    let begin = serde_json::json!({"type":"begin","schema":"ruview.companion.calibration-reference.v1","lineage":lineage});
    // A streamed body cannot pin app state or preserve permission/context
    // sampled before body completion. No user network or sensor process is used.
    for change in ["privacy", "store"] {
        let _ = owner_json(
            owner.clone(),
            Method::POST,
            "/api/v1/companion/devices/phone/references",
            serde_json::json!({"record_comparison_evidence":true}),
        )
        .await;
        let (polled_tx, polled_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let bytes = serde_json::to_vec(&begin).unwrap();
        let stream = futures_util::stream::once(async move {
            let _ = polled_tx.send(());
            release_rx
                .await
                .map_err(|_| std::io::Error::other("synthetic body cancelled"))?;
            Ok::<Vec<u8>, std::io::Error>(bytes)
        });
        let request = Request::builder()
            .method(Method::POST)
            .uri("/companion/v1/calibration-references")
            .header("x-ruview-installation-id", "synthetic-install")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from_stream(stream))
            .unwrap();
        let router = peer.clone();
        let pending = tokio::spawn(async move { router.oneshot(request).await.unwrap() });
        tokio::time::timeout(Duration::from_secs(2), polled_rx)
            .await
            .unwrap()
            .unwrap();
        let guard = rt.mutation_guard().unwrap();
        {
            let mut current = tokio::time::timeout(Duration::from_secs(2), state.write())
                .await
                .expect("body held the app lock");
            if change == "privacy" {
                current.privacy_mode = true;
            } else {
                current.calibration_references = Some(
                    wifi_densepose_sensing_server::companion_references::ReferenceStore::new(
                        service.clone(),
                    ),
                );
            }
        }
        release_tx.send(()).unwrap();
        let response = pending.await.unwrap();
        assert_eq!(
            response.status(),
            if change == "privacy" {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::CONFLICT
            }
        );
        assert_eq!(rt.mutation_guard().unwrap(), guard);
        assert_eq!(
            references.summary(chrono::Utc::now().timestamp().max(0) as u64)["retained_ram_rows"],
            0
        );
        if change == "privacy" {
            assert!(references
                .snapshot(chrono::Utc::now().timestamp().max(0) as u64)
                .is_err());
        }
        let mut current = state.write().await;
        current.privacy_mode = false;
        current.calibration_references = Some(references.clone());
    }
    let _ = owner_json(
        owner.clone(),
        Method::POST,
        "/api/v1/companion/devices/phone/references",
        serde_json::json!({"record_comparison_evidence":true}),
    )
    .await;
    let guard = rt.mutation_guard().unwrap();
    let (status, session) = peer_json(peer.clone(), &token, begin.clone()).await;
    assert_eq!(status, StatusCode::OK, "{session}");
    let first = rt.receiver_now_ns();
    let (status, observation) = peer_json(peer.clone(), &token, serde_json::json!({
        "type":"observation","schema":"ruview.companion.calibration-reference.v1","lineage":lineage,
        "reference_session_id":session["reference_session_id"],"sequence":1,"monotonic_timestamp_seconds":"42.125",
        "captured_at_unix_ms":chrono::Utc::now().timestamp_millis(),"tracking_state":"normal","detector_completed":true,"visible_body_count":0
    })).await;
    let last = rt.receiver_now_ns().max(first + 16);
    assert_eq!(status, StatusCode::OK, "{observation}");
    assert_eq!(
        rt.mutation_guard().unwrap(),
        guard,
        "HTTP references advanced RF authority"
    );
    feed_quarter(&rt, RoomLabel::Empty, 4, 0, first, last);
    for quarter in 1..4u32 {
        let first = at + u64::from(quarter) * 3_000_000_000 + 1_000_000;
        feed_quarter(&rt, RoomLabel::Empty, 4, quarter, first, first + 70_000_000);
    }
    rt.finish_phase(&rt.mutation_guard().unwrap(), at + 12_000_000_000, true)
        .unwrap();
    let plain = rt
        .evaluate(&rt.mutation_guard().unwrap(), DataRole::HeldOut)
        .unwrap();
    let (_, compared) = owner_json(owner.clone(), Method::POST, "/api/v1/room-learner/evaluate", serde_json::json!({"expected":rt.mutation_guard().unwrap(),"role":"HeldOut","record_comparison_evidence":true,"record_reference_comparison":true})).await;
    assert_eq!(compared["success"], true, "{compared}");
    assert_eq!(
        compared["evaluation"]["receipt_sha256"],
        plain["receipt_sha256"]
    );
    assert_eq!(
        compared["evaluation"]["reference_comparison"]["cohort_count"],
        4
    );
    assert_eq!(
        compared["evaluation"]["reference_comparison"]["associated_reference_rows"],
        1
    );
    assert_eq!(
        compared["evaluation"]["reference_comparison"]["unknown_cohorts"],
        4
    );
    assert_eq!(
        compared["evaluation"]["reference_comparison"]["adoption_authorized"],
        false
    );
    assert_eq!(
        rt.status(rt.receiver_now_ns())["model_selection"]["current"]["status"],
        "none"
    );
    // The sidecar is nested in the exact owned profile already recursively erased
    // by native account erasure; no device-wide or external evidence directory.
    let profile = dir.path().join("a".repeat(64)).join("SensingServer");
    let sidecar = profile
        .join("room-learner")
        .join(compared["evaluation"]["context_digest"].as_str().unwrap())
        .join("evaluations")
        .join(compared["evaluation"]["model_sha256"].as_str().unwrap())
        .join("held-out/reference-comparisons-v1")
        .join(format!(
            "{}.json",
            compared["evaluation"]["reference_comparison"]["sidecar_sha256"]
                .as_str()
                .unwrap()
        ));
    assert!(sidecar.is_file());
    assert!(sidecar
        .canonicalize()
        .unwrap()
        .starts_with(profile.canonicalize().unwrap()));
    let summary = references.summary(chrono::Utc::now().timestamp().max(0) as u64);
    for field in [
        "rows",
        "source_time",
        "captured_ms",
        "visible_body_count",
        "device_id",
    ] {
        assert!(summary.get(field).is_none());
    }
    let (_, _) = owner_json(
        owner,
        Method::DELETE,
        "/api/v1/companion/devices/phone/references",
        serde_json::json!({}),
    )
    .await;
    let (status, _) = peer_json(peer, &token, begin).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        references.summary(chrono::Utc::now().timestamp().max(0) as u64)["retained_ram_rows"],
        0
    );
}
