//! Exact actual-client wire fixture, zero recorded native windows and NO fit.
//! Crossing the revision-domain guard may reach missing-proof/evaluation only;
//! this is not an accepted C4C reference or a fabricated opaque capability.
use super::*;
use axum::{
    body::Body,
    http::{Method, Request},
};
use tower::ServiceExt;
use wifi_densepose_sensing_server::room_learner::recovery_store::RecoveryStore;
async fn call(router: Router, path: &str, value: Option<serde_json::Value>) -> serde_json::Value {
    let mut request = Request::builder().uri(path);
    let body = if let Some(value) = value {
        request = request
            .method(Method::POST)
            .header("content-type", "application/json");
        Body::from(value.to_string())
    } else {
        Body::empty()
    };
    let response = router.oneshot(request.body(body).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 65536)
            .await
            .unwrap(),
    )
    .unwrap()
}
fn header(node: u8, sequence: u32) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend(0xC5110001u32.to_le_bytes());
    b.extend([node, 1]);
    b.extend(64u16.to_le_bytes());
    b.extend(2437u32.to_le_bytes());
    b.extend(sequence.to_le_bytes());
    b.extend([(-40i8) as u8, (-90i8) as u8, 0, 0x20]);
    for _ in 0..64 {
        b.extend([3, 4]);
    }
    b
}
#[tokio::test]
async fn actual_client_publication_counter_is_not_reference_learner_fingerprint() {
    let client: serde_json::Value = serde_json::from_str(include_str!(
        "testdata/paired_temporal_client_revision.json"
    ))
    .unwrap();
    assert_eq!(
        client["binding_digest"],
        "c020caf87c161f0bf595e1f603aef139045c9362f0d933341537d73225bca30d"
    );
    assert_eq!(client["computed_structural_revision"], 211247076768791u64);
    let directory = tempfile::tempdir().unwrap();
    let owner = "a".repeat(64);
    let installation = client["installation_id"].as_str().unwrap();
    let path = directory.path().join(&owner).join("SensingServer");
    let recovery = RecoveryStore::new(&path, &owner, installation, Some(&[7; 32])).unwrap();
    let runtime = Arc::new(
        RoomLearningRuntime::new_with_recovery(&path, &owner, installation, recovery).unwrap(),
    );
    let publication = crate::companion_room::SharedRoomStore::load(
        &directory.path().join("publication"),
        installation,
    )
    .unwrap();
    let response = crate::companion_room::owner_router(publication.clone())
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri("/api/v1/companion/room")
                .header("content-type", "application/json")
                .body(Body::from(
                    client["publication_request"]["body"].to_string(),
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
    assert_eq!(published["binding_digest"], client["binding_digest"]);
    let mut inner = AppStateInner::minimal();
    inner.room_learning = Some(runtime.clone());
    inner.current_room_store = Some(publication.clone());
    inner.installation_id = Some(installation.into());
    let seen = std::time::Instant::now();
    // Transport readiness only, as in the existing parent paired-client seam;
    // Runtime has no configured capture while these SYNTHETIC headers arrive.
    for row in 0..120u32 {
        for node in [15, 16, 17] {
            let b = header(node, row + 1);
            let f = parse_esp32_frame(&b).unwrap();
            let at = seen - std::time::Duration::from_millis(u64::from(120 - row) * 100);
            inner
                .node_states
                .entry(node)
                .or_insert_with(NodeState::new)
                .observe_raw_grid(CsiGridKey::from_frame(&f), at);
            super::super::observe(&mut inner, &f, &b, at);
        }
    }
    let state = Arc::new(RwLock::new(inner));
    let router = Router::new()
        .route("/configure", post(configure_dispatch))
        .route("/status", get(status_dispatch))
        .with_state(state);
    let mut command = client["configure_request"]["body"].clone();
    command["expected"] = serde_json::to_value(runtime.mutation_guard().unwrap()).unwrap();
    assert_eq!(
        call(router.clone(), "/configure", Some(command)).await["success"],
        true
    );
    let mut ready = false;
    for _ in 0..100 {
        let status = call(
            router.clone(),
            &format!("/status?schema={STATUS_SCHEMA}"),
            None,
        )
        .await;
        if status["status"]["capture"]["stage"] == "ready" {
            assert_eq!(status["status"]["capture"]["retained_windows"], 0);
            ready = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
    }
    assert!(ready);
    let context = runtime.context().unwrap();
    let original_digest = context.digest();
    let guard = runtime.mutation_guard().unwrap();
    assert_eq!(context.revision, 211247076768791);
    assert_eq!(
        context.room_digest,
        client["binding_digest"].as_str().unwrap()
    );
    let mut other_fingerprint = context.clone();
    other_fingerprint.revision = 1;
    assert_ne!(other_fingerprint.digest(), original_digest);
    let scope_error = publication
        .with_reference_publication(|p| {
            runtime.with_reference_scope(
                p.binding_digest.as_deref().unwrap(),
                p.revision,
                &[15, 16, 17],
                runtime.receiver_now_ns(),
                |_, _| Ok(()),
            )
        })
        .unwrap()
        .unwrap_err();
    assert_eq!(scope_error,"reference_training_proof_required","a canonical room pair cannot be rejected merely because its counter is not its learner fingerprint");
    let read_error = match publication
        .with_reference_publication(|p| {
            runtime.temporal_comparison_readback(p.binding_digest.as_deref().unwrap(), p.revision)
        })
        .unwrap()
    {
        Err(e) => e,
        Ok(_) => panic!("no protected evaluation exists; no read capability may be manufactured"),
    };
    assert_eq!(read_error, "reference_current_evaluation_required");
    assert_eq!(runtime.context().unwrap().digest(), original_digest);
    assert_eq!(runtime.mutation_guard().unwrap(), guard);
}
