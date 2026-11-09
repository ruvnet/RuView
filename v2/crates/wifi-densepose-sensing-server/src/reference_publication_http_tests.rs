//! Actual owner-token/room/permit HTTP composers with NO native RF input/model.
//! This is publication authority testing, not the pending C4C64 association test.
use super::*;
use std::time::Duration;
async fn owner_request(
    router: Router,
    method: Method,
    path: &str,
    value: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("authorization", "Bearer boundary-owner")
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
async fn boundary_fixture() -> (
    tempfile::TempDir,
    SharedState,
    Arc<companion_room::SharedRoomStore>,
    Arc<wifi_densepose_sensing_server::companion_references::ReferenceStore>,
    Router,
    Arc<(tokio::sync::Notify, tokio::sync::Notify)>,
) {
    let directory = tempfile::tempdir().unwrap();
    let publication = companion_room::SharedRoomStore::load(
        &directory.path().join("publication"),
        "synthetic-install",
    )
    .unwrap();
    let (status, _) = json_call(
        companion_room::owner_router(publication.clone()),
        Method::PUT,
        "/api/v1/companion/room",
        serde_json::json!({"expected_revision":0,"room":room()}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let runtime = Arc::new(
        RoomLearningRuntime::new(
            &directory.path().join("a".repeat(64)).join("SensingServer"),
            &"a".repeat(64),
            "synthetic-install",
        )
        .unwrap(),
    );
    let mut inner = AppStateInner::minimal();
    inner.room_learning = Some(runtime);
    inner.current_room_store = Some(publication.clone());
    let service = Companion::load_for_account(
        &directory.path().join("pairing"),
        "synthetic-install".into(),
        "f".repeat(64),
        "https://synthetic.local:3443".into(),
        Some("a".repeat(64)),
    )
    .unwrap();
    let _access = pair(service.clone(), "phone").await;
    let references =
        wifi_densepose_sensing_server::companion_references::ReferenceStore::new(service);
    inner.calibration_references = Some(references.clone());
    let state = Arc::new(RwLock::new(inner));
    let barrier = Arc::new((tokio::sync::Notify::new(), tokio::sync::Notify::new()));
    let control = Control {
        state: state.clone(),
        serial: Arc::new(tokio::sync::Mutex::new(())),
        publication_barrier: Some(barrier.clone()),
    };
    let api = companion_reference_api::ReferenceApi {
        state: state.clone(),
        room: publication.clone(),
        references: references.clone(),
    };
    let router = companion_reference_api::owner_router(api)
        .merge(companion_room::owner_router(publication.clone()))
        .layer(axum::middleware::from_fn_with_state(
            control,
            serialize_control,
        ))
        .layer(axum::middleware::from_fn_with_state(
            AuthState::from_token("boundary-owner"),
            require_owner,
        ));
    (directory, state, publication, references, router, barrier)
}
async fn permit(router: Router) -> (StatusCode, serde_json::Value) {
    owner_request(
        router,
        Method::POST,
        "/api/v1/companion/devices/phone/references",
        serde_json::json!({"record_comparison_evidence":true}),
    )
    .await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actual_room_put_blocks_renewal_across_cas_without_holding_global_locks() {
    let (_directory, state, publication, references, router, barrier) = boundary_fixture().await;
    assert_eq!(permit(router.clone()).await.0, StatusCode::OK);
    let runtime = state.read().await.room_learning.clone().unwrap();
    let guard = runtime.mutation_guard().unwrap();
    let mut changed = room();
    changed["widthM"] = serde_json::json!(4);
    let publisher = router.clone();
    let pending = tokio::spawn(async move {
        owner_request(
            publisher,
            Method::PUT,
            "/api/v1/companion/room",
            serde_json::json!({"expected_revision":1,"room":changed}),
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(2), barrier.0.notified())
        .await
        .unwrap();
    let app_unlocked = state.try_write().is_ok();
    let read_room = publication.clone();
    let mut room_check = tokio::task::spawn_blocking(move || read_room.snapshot());
    let room_unlocked = tokio::time::timeout(Duration::from_millis(100), &mut room_check)
        .await
        .is_ok();
    let check_runtime = runtime.clone();
    let mut runtime_check = tokio::task::spawn_blocking(move || check_runtime.mutation_guard());
    let runtime_unlocked = tokio::time::timeout(Duration::from_millis(100), &mut runtime_check)
        .await
        .is_ok();
    let (renew_status, renewed) = permit(router.clone()).await;
    barrier.1.notify_one();
    let result = pending.await.unwrap();
    if !room_unlocked {
        let _ = room_check.await;
    }
    if !runtime_unlocked {
        let _ = runtime_check.await;
    }
    assert!(
        app_unlocked && room_unlocked && runtime_unlocked,
        "no App/Room/Runtime guard held across publication wait"
    );
    assert_eq!(
        renew_status,
        StatusCode::CONFLICT,
        "renewal cannot reopen purpose between drain and CAS"
    );
    assert_eq!(renewed["error_code"], "reference_publication_transition");
    assert_eq!(result.0, StatusCode::OK);
    assert_eq!(publication.snapshot().unwrap().revision, 2);
    assert_eq!(
        references.summary(chrono::Utc::now().timestamp().max(0) as u64)["active"],
        false
    );
    assert_eq!(
        runtime.mutation_guard().unwrap(),
        guard,
        "publication reference fence is not an RF mutation"
    );
    assert_eq!(
        permit(router).await.0,
        StatusCode::OK,
        "explicit new consent is possible only after transition"
    );
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_preflight_preserves_purpose_and_failed_cas_never_restores_it() {
    let (_directory, _state, publication, references, router, barrier) = boundary_fixture().await;
    assert_eq!(permit(router.clone()).await.0, StatusCode::OK);
    let old = owner_request(
        router.clone(),
        Method::PUT,
        "/api/v1/companion/room",
        serde_json::json!({"expected_revision":0,"room":room()}),
    )
    .await;
    assert_eq!(old.0, StatusCode::CONFLICT);
    assert_eq!(
        references.summary(chrono::Utc::now().timestamp().max(0) as u64)["active"],
        true
    );
    let publisher = router.clone();
    let pending = tokio::spawn(async move {
        owner_request(
            publisher,
            Method::PUT,
            "/api/v1/companion/room",
            serde_json::json!({"expected_revision":1,"room":room()}),
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(2), barrier.0.notified())
        .await
        .unwrap();
    // Actual competing in-process CAS, with no callback/proof injection.
    let (foreign, _) = json_call(
        companion_room::owner_router(publication.clone()),
        Method::PUT,
        "/api/v1/companion/room",
        serde_json::json!({"expected_revision":1,"room":room()}),
    )
    .await;
    barrier.1.notify_one();
    let result = pending.await.unwrap();
    assert_eq!(foreign, StatusCode::OK);
    assert_eq!(result.0, StatusCode::CONFLICT);
    assert_eq!(publication.snapshot().unwrap().revision, 2);
    assert_eq!(
        references.summary(chrono::Utc::now().timestamp().max(0) as u64)["active"],
        false
    );
    assert_eq!(
        permit(router).await.0,
        StatusCode::OK,
        "failed CAS releases only its transition fence"
    );
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_room_put_drops_its_transition_without_reviving_old_purpose() {
    let (_directory, _state, publication, references, router, barrier) = boundary_fixture().await;
    assert_eq!(permit(router.clone()).await.0, StatusCode::OK);
    let publisher = router.clone();
    let pending = tokio::spawn(async move {
        owner_request(
            publisher,
            Method::PUT,
            "/api/v1/companion/room",
            serde_json::json!({"expected_revision":1,"room":room()}),
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(2), barrier.0.notified())
        .await
        .unwrap();
    pending.abort();
    assert!(pending.await.is_err());
    assert_eq!(publication.snapshot().unwrap().revision, 1);
    assert_eq!(
        references.summary(chrono::Utc::now().timestamp().max(0) as u64)["active"],
        false
    );
    assert_eq!(permit(router).await.0, StatusCode::OK);
}
