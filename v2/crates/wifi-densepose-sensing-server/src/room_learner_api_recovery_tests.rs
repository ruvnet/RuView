//! SYNTHETIC actual HTTP handlers; no listeners, user data or hardware.
use super::*;
use wifi_densepose_sensing_server::room_learner::recovery_store::RecoveryStore;
async fn recovery_fixture() -> (tempfile::TempDir, SharedState) {
    let (dir, state) = fixture();
    replace_runtime(dir.path(), &state).await;
    (dir, state)
}
async fn replace_runtime(root: &std::path::Path, state: &SharedState) {
    let owner = "a".repeat(64);
    let path = root.join(&owner).join("SensingServer");
    let store = RecoveryStore::new(&path, &owner, "synthetic-install", Some(&[7; 32])).unwrap();
    state.write().await.room_learning = Some(Arc::new(
        RoomLearningRuntime::new_with_recovery(&path, &owner, "synthetic-install", store).unwrap(),
    ));
}
fn recovery_app(state: SharedState) -> Router {
    Router::new()
        .route("/configure", post(configure))
        .route("/status", get(status))
        .route("/phase", post(begin_phase))
        .route("/finish", post(finish_phase))
        .route("/recovery/restore", post(restore_capture))
        .route("/recovery/discard", post(discard_capture))
        .with_state(state)
}
fn recovery_config() -> serde_json::Value {
    let mut c = config();
    c["recovery_consent"] = true.into();
    c
}
#[tokio::test]
async fn http_recovery_is_explicit_guarded_exact_and_metadata_only() {
    let (dir, state) = recovery_fixture().await;
    let router = recovery_app(state.clone());
    let configured = json_request(
        router.clone(),
        "/configure",
        Method::POST,
        recovery_config(),
    )
    .await;
    assert_eq!(configured["success"], true, "{configured}");
    assert_eq!(configured["status"]["derived_samples_persisted"], false);
    let id = configured["status"]["recovery"]["discard_id"].clone();
    let oldguard = configured["status"]["mutation_guard"].clone();
    assert_eq!(
        json_request(
            router.clone(),
            "/configure",
            Method::POST,
            recovery_config()
        )
        .await["error_code"],
        "learner_recovery_pending"
    );
    replace_runtime(dir.path(), &state).await;
    let cold = json_request_status(router.clone()).await;
    assert!(cold["context_digest"].is_null());
    assert_eq!(cold["recovery"]["state"], "restore_required");
    let mut restore = serde_json::json!({"recovery_id":id,"binding_digest":"b".repeat(64),"revision":1,"source_node_ids":[15,16,17]});
    restore["expected"] = oldguard;
    assert_eq!(
        json_request(
            router.clone(),
            "/recovery/restore",
            Method::POST,
            restore.clone()
        )
        .await["error_code"],
        "learner_state_changed"
    );
    restore["expected"] = cold["mutation_guard"].clone();
    restore["revision"] = 2.into();
    assert_eq!(
        json_request(
            router.clone(),
            "/recovery/restore",
            Method::POST,
            restore.clone()
        )
        .await["error_code"],
        "learner_recovery_context_mismatch"
    );
    restore["revision"] = 1.into();
    let recovered = json_request(router.clone(), "/recovery/restore", Method::POST, restore).await;
    assert_eq!(recovered["success"], true, "{recovered}");
    assert_eq!(recovered["status"]["recovery"]["automatic_resume"], false);
    assert!(recovered["status"]["capture"]["sources"]
        .as_array()
        .unwrap()
        .iter()
        .all(|s| s["live_buffer_frames"] == 0));
    let encoded = serde_json::to_string(&recovered).unwrap();
    for forbidden in [
        "amplitudes",
        "plaintext",
        "source_ordinals",
        "cohort_id",
        "signing_seed",
    ] {
        assert!(!encoded.contains(forbidden), "{forbidden}");
    }
    let guard = recovered["status"]["mutation_guard"].clone();
    assert_eq!(
        json_request(
            router.clone(),
            "/recovery/discard",
            Method::POST,
            serde_json::json!({"expected":guard,"recovery_id":"wrong"})
        )
        .await["success"],
        false
    );
    let discarded = json_request(
        router.clone(),
        "/recovery/discard",
        Method::POST,
        serde_json::json!({"recovery_id":id}),
    )
    .await;
    assert_eq!(discarded["success"], true, "{discarded}");
    assert!(discarded["status"]["recovery"]["saved"].is_null());
}
#[tokio::test]
async fn http_recovery_rejects_unready_native_context_and_privacy_without_deleting_slot() {
    let (_dir, state) = recovery_fixture().await;
    let router = recovery_app(state.clone());
    let configured = json_request(
        router.clone(),
        "/configure",
        Method::POST,
        recovery_config(),
    )
    .await;
    let id = configured["status"]["recovery"]["discard_id"].clone();
    state.write().await.room_learning_sources.clear();
    let restore = serde_json::json!({"recovery_id":id,"binding_digest":"b".repeat(64),"revision":1,"source_node_ids":[15,16,17]});
    assert_eq!(
        json_request(router.clone(), "/recovery/restore", Method::POST, restore).await
            ["error_code"],
        "learner_recovery_source_unready"
    );
    state.write().await.privacy_mode = true;
    assert_eq!(
        json_request(
            router.clone(),
            "/recovery/discard",
            Method::POST,
            serde_json::json!({"recovery_id":id})
        )
        .await["error_code"],
        "learner_privacy_mode"
    );
    state.write().await.privacy_mode = false;
    assert_eq!(
        json_request_status(router).await["recovery"]["discard_id"],
        id
    );
}
