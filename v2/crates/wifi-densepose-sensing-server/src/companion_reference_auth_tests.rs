//! SYNTHETIC R1 owner/paired HTTP fixtures. No paired-progress feature dependency.
use super::*;
use axum::{
    body::Body,
    http::{Method, Request},
};
use tower::ServiceExt;
use wifi_densepose_sensing_server::room_learner::{
    lifecycle::RoomLearningRuntime,
    observation::{CaptureContext, SourceSpec},
};
async fn json_call(
    router: Router,
    method: Method,
    path: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let r = router
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = r.status();
    let b = axum::body::to_bytes(r.into_body(), 65536).await.unwrap();
    (
        status,
        serde_json::from_slice(&b).unwrap_or(serde_json::Value::Null),
    )
}
async fn pair(service: Arc<Companion>, device: &str) -> String {
    let owner = companion::owner_router(service.clone());
    let public = companion::public_router(service);
    let (s, invite) = json_call(
        owner.clone(),
        Method::POST,
        "/api/v1/companion/invites",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let proof = companion::secret();
    let (s,enrolled)=json_call(public.clone(),Method::POST,"/companion/v1/enroll",serde_json::json!({"installation_id":"synthetic-install","invite_id":invite["invite_id"],"invite_secret":invite["invite_secret"],"device_id":device,"device_name":"Synthetic phone","pairing_secret":proof})).await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = json_call(
        owner,
        Method::POST,
        &format!(
            "/api/v1/companion/requests/{}/approve",
            enrolled["request_id"].as_str().unwrap()
        ),
        serde_json::json!({"comparison_code":enrolled["comparison_code"]}),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s,session)=json_call(public,Method::POST,"/companion/v1/session",serde_json::json!({"installation_id":"synthetic-install","device_id":device,"pairing_secret":proof})).await;
    assert_eq!(s, StatusCode::OK);
    session["access_token"].as_str().unwrap().into()
}
fn room() -> serde_json::Value {
    serde_json::json!({"schema":"ruview.companion.room.v1","id":"synthetic-room","name":"Synthetic room","widthM":3,"lengthM":3,"heightM":2.8,"nodes":[{"nodeId":1,"slot":"north_west","xM":0,"yM":0,"role":"base_station","headingDeg":90,"orientationConfirmed":true,"confirmedAtUnixMs":1},{"nodeId":2,"slot":"south_east","xM":3,"yM":3,"role":"receiver","headingDeg":270,"orientationConfirmed":true,"confirmedAtUnixMs":1}],"accessPoint":null})
}
async fn fixture() -> (
    tempfile::TempDir,
    SharedState,
    Arc<companion_room::SharedRoomStore>,
) {
    let dir = tempfile::tempdir().unwrap();
    let owner = "a".repeat(64);
    let rt = Arc::new(
        RoomLearningRuntime::new(
            &dir.path().join(&owner).join("SensingServer"),
            &owner,
            "synthetic-install",
        )
        .unwrap(),
    );
    let publication =
        companion_room::SharedRoomStore::load(&dir.path().join("publication"), "synthetic-install")
            .unwrap();
    let (status, envelope) = json_call(
        companion_room::owner_router(publication.clone()),
        Method::PUT,
        "/api/v1/companion/room",
        serde_json::json!({"expected_revision":0,"room":room()}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let context = CaptureContext {
        owner_scope: owner,
        installation_id: "synthetic-install".into(),
        room_digest: envelope["binding_digest"].as_str().unwrap().into(),
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
        "synthetic-actual-http-run".into(),
        true,
    )
    .unwrap();
    let mut inner = AppStateInner::minimal();
    inner.room_learning = Some(rt);
    inner.current_room_store = Some(publication.clone());
    inner.installation_id = Some("synthetic-install".into());
    inner.calibration_boot_epoch = "cal-boot-synthetic".into();
    let now = std::time::Instant::now();
    for row in 0..120u32 {
        for node in 1..=2u8 {
            let mut wire = Vec::new();
            wire.extend(0xC5110001u32.to_le_bytes());
            wire.extend([node, 1]);
            wire.extend(64u16.to_le_bytes());
            wire.extend(2437u32.to_le_bytes());
            wire.extend((row + 1).to_le_bytes());
            wire.extend([(-40i8) as u8, (-90i8) as u8, 0, 0x20]);
            for _ in 0..64 {
                wire.extend([3, 4]);
            }
            let frame = parse_esp32_frame(&wire).unwrap();
            room_learner_api::observe(
                &mut inner,
                &frame,
                &wire,
                now - std::time::Duration::from_millis(u64::from(120 - row) * 100),
            );
        }
    }
    (dir, Arc::new(RwLock::new(inner)), publication)
}

async fn fetch(
    router: Router,
    method: Method,
    installation: &str,
    token: &str,
    path: &str,
) -> (StatusCode, serde_json::Value) {
    let response = router
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("x-ruview-installation-id", installation)
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let b = axum::body::to_bytes(response.into_body(), 65536)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&b).unwrap_or(serde_json::Value::Null),
    )
}

#[tokio::test]
async fn reference_routes_require_exact_owner_and_paired_read_does_not_grant_reference_purpose() {
    const PEER: &str = "/companion/v1/calibration-references";
    let (dir, state, room) = fixture().await;
    let service = Companion::load(
        &dir.path().join("reference-pair"),
        "synthetic-install".into(),
        "f".repeat(64),
        "https://synthetic.local:3443".into(),
    )
    .unwrap();
    let token = pair(service.clone(), "phone").await;
    let refs =
        wifi_densepose_sensing_server::companion_references::ReferenceStore::new(service.clone());
    let api = companion_reference_api::ReferenceApi {
        state: state.clone(),
        room: room.clone(),
        references: refs.clone(),
    };
    state.write().await.calibration_references = Some(refs.clone());
    let owner = companion_reference_api::owner_router(api.clone()).layer(
        axum::middleware::from_fn_with_state(AuthState::from_token("owner"), require_owner),
    );
    for (peer, credential, status) in [
        ("127.0.0.1:1", "owner", 200),
        ("127.0.0.1:1", token.as_str(), 403),
        ("10.0.0.1:1", "owner", 403),
    ] {
        let mut req = Request::builder()
            .method("POST")
            .uri("/api/v1/companion/devices/phone/references")
            .header("authorization", format!("Bearer {credential}"))
            .header("content-type", "application/json")
            .body(Body::from("{}"))
            .unwrap();
        req.extensions_mut()
            .insert(ConnectInfo(peer.parse::<SocketAddr>().unwrap()));
        let response = owner.clone().oneshot(req).await.unwrap();
        assert_eq!(response.status().as_u16(), status);
        if status == 200 {
            let b = axum::body::to_bytes(response.into_body(), 8192)
                .await
                .unwrap();
            let v: serde_json::Value = serde_json::from_slice(&b).unwrap();
            assert_eq!(v["recording_consented"], false);
        }
    }
    assert!(refs
        .snapshot(chrono::Utc::now().timestamp().max(0) as u64)
        .is_err());
    assert!(
        service
            .authorize("synthetic-install", &token, true)
            .is_err(),
        "reference permission must not hand off calibration control"
    );
    let peer = companion_reference_api::peer_router(api.clone()).layer(
        axum::middleware::from_fn_with_state(service.clone(), require_companion),
    );
    assert_eq!(
        fetch(
            peer.clone(),
            Method::POST,
            "synthetic-install",
            "invalid",
            PEER
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        fetch(peer.clone(), Method::POST, "foreign-install", &token, PEER)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    for path in [
        "/api/v1/companion/references",
        "/api/v1/companion/devices/phone/references",
    ] {
        assert_eq!(
            fetch(peer.clone(), Method::GET, "synthetic-install", &token, path)
                .await
                .0,
            StatusCode::FORBIDDEN
        );
    }
    let huge = Request::builder()
        .method("POST")
        .uri(PEER)
        .header("x-ruview-installation-id", "synthetic-install")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(vec![b'x'; 8193]))
        .unwrap();
    assert_eq!(
        peer.clone().oneshot(huge).await.unwrap().status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    state.write().await.privacy_mode = true;
    let req = Request::builder()
        .method("POST")
        .uri(PEER)
        .header("x-ruview-installation-id", "synthetic-install")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from("{}"))
        .unwrap();
    assert_eq!(
        peer.clone().oneshot(req).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    state.write().await.privacy_mode = false;
    state.write().await.calibration_references =
        Some(wifi_densepose_sensing_server::companion_references::ReferenceStore::new(service));
    let req = Request::builder()
        .method("POST")
        .uri(PEER)
        .header("x-ruview-installation-id", "synthetic-install")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from("{}"))
        .unwrap();
    let response = peer.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = axum::body::to_bytes(response.into_body(), 8192)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap()["error_code"],
        "reference_service_changed"
    );
    assert_eq!(
        policy(
            &Method::GET,
            "/api/v1/companion/references/temporal-comparison"
        ),
        None,
        "owner private comparison must never become paired progress"
    );
}

// Actual current-C4C positive composition is registered separately after its
// genuine producer-fixture export/publicRestore boundary is established.

#[path="reference_publication_http_tests.rs"]
mod publication_http;
