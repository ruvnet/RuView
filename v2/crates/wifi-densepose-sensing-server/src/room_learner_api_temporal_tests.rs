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
        .route("/retry", post(retry_dispatch))
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
        "/api/v1/room-learner/recovery/retry",
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

#[tokio::test]
async fn configured_owner_status_serializes_its_capture_clock_tick_before_sampling_time() {
    use wifi_densepose_sensing_server::room_learner::recovery_store::RecoveryStore;
    let (dir,state)=fixture();
    let owner="a".repeat(64);
    let path=dir.path().join(&owner).join("SensingServer");
    let store=RecoveryStore::new(&path,&owner,"synthetic-install",Some(&[7;32])).unwrap();
    let rt=Arc::new(RoomLearningRuntime::new_with_recovery(&path,&owner,"synthetic-install",store).unwrap());
    state.write().await.room_learning=Some(rt.clone());
    let app=temporal_app(state.clone());
    let configured=decoded(app.clone(),"/configure",Some(configure_v2(rt.mutation_guard().unwrap()))).await;
    assert_eq!(configured["success"],true,"{configured}");
    for _ in 0..100 {
        let s=decoded(app.clone(),&format!("/status?schema={STATUS_SCHEMA}"),None).await;
        if s["status"]["capture"]["stage"]=="ready"{break;}
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
    }
    let shared_reader=state.read().await;
    let task=tokio::spawn(async move{decoded(app,&format!("/status?schema={STATUS_SCHEMA}"),None).await});
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    let ran_concurrently=task.is_finished();
    drop(shared_reader);
    let reply=tokio::time::timeout(std::time::Duration::from_secs(5),task).await.unwrap().unwrap();
    assert_eq!(reply["status"]["capture"]["stage"],"ready","{reply}");
    assert_eq!(reply["status"]["capture"]["retained_windows"],0);
    assert_eq!(reply["status"]["capture"]["observed_ms"],0);
    assert!(!ran_concurrently,"clock-mutating owner status must not sample/tick concurrently under a shared read lock");
}

#[tokio::test(flavor="multi_thread", worker_threads=2)]
async fn concurrent_actual_owner_status_requests_keep_unstarted_full_run_ready() {
    use wifi_densepose_sensing_server::room_learner::recovery_store::RecoveryStore;
    let (dir,state)=fixture();let owner="a".repeat(64);let path=dir.path().join(&owner).join("SensingServer");
    let store=RecoveryStore::new(&path,&owner,"synthetic-install",Some(&[7;32])).unwrap();
    let rt=Arc::new(RoomLearningRuntime::new_with_recovery(&path,&owner,"synthetic-install",store).unwrap());
    state.write().await.room_learning=Some(rt.clone());let app=temporal_app(state.clone());
    let configured=decoded(app.clone(),"/configure",Some(configure_v2(rt.mutation_guard().unwrap()))).await;assert_eq!(configured["success"],true,"{configured}");
    let mut ready=false;for _ in 0..100{let s=decoded(app.clone(),&format!("/status?schema={STATUS_SCHEMA}"),None).await;if s["status"]["capture"]["stage"]=="ready"{ready=true;break;}tokio::time::sleep(std::time::Duration::from_millis(2)).await;}assert!(ready);
    let barrier=Arc::new(tokio::sync::Barrier::new(2));let mut tasks=Vec::new();
    for _ in 0..2 {let app=app.clone();let barrier=barrier.clone();tasks.push(tokio::spawn(async move{let mut failed=None;for _ in 0..200{barrier.wait().await;let s=decoded(app.clone(),&format!("/status?schema={STATUS_SCHEMA}"),None).await;if s["status"]["capture"]["stage"]!="ready"&&failed.is_none(){failed=Some(s);}}failed}));}
    let mut errors=Vec::new();for task in tasks{if let Some(error)=tokio::time::timeout(std::time::Duration::from_secs(30),task).await.unwrap().unwrap(){errors.push(error);}}
    if let Some(error)=errors.first(){println!("SYNTHETIC_ACTUAL_PARALLEL_OWNER_STATUS_FAILURE={error}");}
    assert!(errors.is_empty(),"actual concurrent owner GETs failed an unstarted run: {errors:?}");
    let last=decoded(app,&format!("/status?schema={STATUS_SCHEMA}"),None).await;assert_eq!(last["status"]["capture"]["retained_windows"],0);assert_eq!(last["status"]["capture"]["durable_windows"],0);assert_eq!(last["status"]["capture"]["observed_ms"],0);
}

#[tokio::test]
async fn actual_client_publication_and_learner_revision_domains_pair_without_rewriting_context() {
    use wifi_densepose_sensing_server::room_learner::recovery_store::RecoveryStore;
    let client:serde_json::Value=serde_json::from_str(include_str!("testdata/paired_temporal_client_revision.json")).unwrap();
    let (dir,state)=fixture();let owner="a".repeat(64);let installation=client["installation_id"].as_str().unwrap();
    let path=dir.path().join(&owner).join("SensingServer");let recovery=RecoveryStore::new(&path,&owner,installation,Some(&[7;32])).unwrap();
    let rt=Arc::new(RoomLearningRuntime::new_with_recovery(&path,&owner,installation,recovery).unwrap());
    let publication=crate::companion_room::SharedRoomStore::load(&dir.path().join("publication"),installation).unwrap();
    let publish=|body:serde_json::Value|{let router=crate::companion_room::owner_router(publication.clone());async move{
        let reply=router.oneshot(Request::builder().method(Method::PUT).uri("/api/v1/companion/room").header("content-type","application/json").body(Body::from(body.to_string())).unwrap()).await.unwrap();assert_eq!(reply.status(),StatusCode::OK);let bytes=axum::body::to_bytes(reply.into_body(),65536).await.unwrap();serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()
    }};
    let initial=publish(client["publication_request"]["body"].clone()).await;
    assert_eq!(initial["binding_digest"],client["binding_digest"]);assert_eq!(initial["revision"],1);
    {let mut s=state.write().await;s.room_learning=Some(rt.clone());s.current_room_store=Some(publication.clone());s.installation_id=Some(installation.into());s.calibration_boot_epoch="synthetic-paired-domain-boot".into();}
    let mut command=client["configure_request"]["body"].clone();command["expected"]=serde_json::to_value(rt.mutation_guard().unwrap()).unwrap();
    assert_eq!(command["revision"],211247076768791u64);
    let app=temporal_app(state.clone());let configured=decoded(app.clone(),"/configure",Some(command)).await;assert_eq!(configured["success"],true,"{configured}");
    let mut ready=false;for _ in 0..100{let s=decoded(app.clone(),&format!("/status?schema={STATUS_SCHEMA}"),None).await;if s["status"]["capture"]["stage"]=="ready"{ready=true;break;}tokio::time::sleep(std::time::Duration::from_millis(2)).await;}assert!(ready);
    let context=rt.context().unwrap();assert_eq!(context.revision,211247076768791);assert_eq!(context.room_digest,client["binding_digest"].as_str().unwrap());
    let before_guard=rt.mutation_guard().unwrap();let before=decoded(app.clone(),&format!("/status?schema={STATUS_SCHEMA}"),None).await;
    let progress=Router::new().route("/progress",get(super::super::paired_progress::progress_dispatch)).with_state(state.clone());
    let current=decoded(progress.clone(),"/progress",None).await;
    println!("SYNTHETIC_ACTUAL_CLIENT_PAIRED_REVISION_SEAM={current}");
    assert_eq!(current["context_match"],true,"{current}");assert_eq!(current["room"]["revision"],1);assert_eq!(current["room"]["binding_digest"],client["binding_digest"]);
    assert_eq!(current["sources"]["ready"],3);assert_eq!(current["capture"]["retained_windows"],0);assert_eq!(current["capture"]["durable_windows"],0);
    assert_eq!(rt.mutation_guard().unwrap(),before_guard);assert_eq!(rt.context().unwrap(),context);
    let after=decoded(app.clone(),&format!("/status?schema={STATUS_SCHEMA}"),None).await;
    assert_eq!(before["status"]["capture"]["capture_generation_id"],after["status"]["capture"]["capture_generation_id"]);assert_eq!(before["status"]["capture"]["original_deadline"],after["status"]["capture"]["original_deadline"]);
    let mut old_learner_context=context.clone();old_learner_context.revision=1;
    assert_ne!(old_learner_context.digest(),context.digest(),"exact selected/capture context digest must retain the learner revision binding");
    let wrong_ready=rt.paired_temporal_progress_snapshot(installation,"synthetic-paired-domain-boot",Some(wifi_densepose_sensing_server::room_learner::lifecycle::PublishedProgressRoom{binding_digest:context.room_digest.clone(),revision:1}),&[15,16,17],Some(&"f".repeat(64)),rt.receiver_now_ns());
    assert_eq!(wrong_ready.unwrap_err(),"progress_context_changed");
    {state.write().await.privacy_mode=true;}
    assert_eq!(request_raw(progress.clone(),"/progress",None).await.0,StatusCode::FORBIDDEN);
    {let mut s=state.write().await;s.privacy_mode=false;s.installation_id=Some("foreign-installation".into());}
    assert_eq!(request_raw(progress.clone(),"/progress",None).await.0,StatusCode::SERVICE_UNAVAILABLE);
    {state.write().await.installation_id=Some(installation.into());}
    let source_cache={let mut s=state.write().await;let old=s.room_learning_sources.clone();s.room_learning_sources.retain(|key,_|key.node_id!=17);old};
    let missing_source=decoded(progress.clone(),"/progress",None).await;assert_eq!(missing_source["context_match"],true);assert_eq!(missing_source["sources"]["ready"],2);
    {state.write().await.room_learning_sources=source_cache;}
    let (revoked,rx)=tokio::sync::watch::channel(false);let expiry=chrono::Utc::now().timestamp().max(0)as u64+8;
    let grant=wifi_densepose_sensing_server::companion::Grant{device_id:"synthetic-paired-reader".into(),expires_at:expiry,revoked:rx};
    let guarded=Router::new().route("/progress",get(super::super::paired_progress::progress_dispatch)).with_state(state.clone()).layer(axum::Extension(grant));
    let authorized=decoded(guarded.clone(),"/progress",None).await;assert!(authorized["expires_at_unix_ms"].as_u64().unwrap()<=expiry*1000);
    revoked.send(true).unwrap();assert_eq!(request_raw(guarded,"/progress",None).await.0,StatusCode::UNAUTHORIZED);
    let mut revision=1;
    for change in ["same_geometry_new_publication","dimensions","nodes","access_point"] {
        let mut room=client["canonical_room"].clone();
        match change{"dimensions"=>room["widthM"]=serde_json::json!(2.6),"nodes"=>room["nodes"][0]["xM"]=serde_json::json!(0.5),"access_point"=>room["accessPoint"]=serde_json::json!({"slot":"north_center","xM":1.25,"yM":0.4,"provenance":"operator_entered","confirmedAtUnixMs":1}),_=>{}}
        let changed=publish(serde_json::json!({"expected_revision":revision,"room":room})).await;revision+=1;assert_ne!(changed["binding_digest"],client["binding_digest"]);
        let blocked=decoded(progress.clone(),"/progress",None).await;assert_eq!(blocked["context_match"],false,"{change}:{blocked}");assert_eq!(blocked["blocker"],"context_mismatch");
        for key in ["room","model_kind","profile","capture","candidate","evaluation","qualification","selection","sources"]{assert!(blocked[key].is_null(),"{change}:{key}");}
        assert_eq!(rt.context().unwrap(),context);assert_eq!(rt.mutation_guard().unwrap(),before_guard);
    }
}

async fn cadence_priming_fixture(cold_restore:bool)->(tempfile::TempDir,SharedState,Arc<RoomLearningRuntime>,Router,serde_json::Value) {
    use wifi_densepose_sensing_server::room_learner::recovery_store::RecoveryStore;
    let (dir,state)=fixture();let owner="a".repeat(64);let path=dir.path().join(&owner).join("SensingServer");
    let store=RecoveryStore::new(&path,&owner,"synthetic-install",Some(&[7;32])).unwrap();
    let rt=Arc::new(RoomLearningRuntime::new_with_recovery(&path,&owner,"synthetic-install",store).unwrap());
    state.write().await.room_learning=Some(rt.clone());let app=temporal_app(state.clone());
    let configured=decoded(app.clone(),"/configure",Some(configure_v2(rt.mutation_guard().unwrap()))).await;assert_eq!(configured["success"],true,"{configured}");
    let mut before=serde_json::Value::Null;for _ in 0..100{let s=decoded(app.clone(),&format!("/status?schema={STATUS_SCHEMA}"),None).await;if s["status"]["capture"]["stage"]=="ready"{before=s;break;}tokio::time::sleep(std::time::Duration::from_millis(2)).await;}assert!(before.is_object());
    if !cold_restore{return(dir,state,rt,app,before);}
    let context=rt.context().unwrap();let capture_id=before["status"]["capture"]["capture_generation_id"].as_str().unwrap().to_string();
    state.write().await.room_learning=None;drop(rt);
    let store=RecoveryStore::new(&path,&owner,"synthetic-install",Some(&[7;32])).unwrap();
    let restored=Arc::new(RoomLearningRuntime::new_with_recovery(&path,&owner,"synthetic-install",store).unwrap());
    restored.temporal_restore_v2(&restored.mutation_guard().unwrap(),&context,&capture_id,restored.receiver_now_ns()).unwrap();
    state.write().await.room_learning=Some(restored.clone());
    let after=decoded(app.clone(),&format!("/status?schema={STATUS_SCHEMA}"),None).await;assert_eq!(after["status"]["capture"]["stage"],"paused","{after}");
    assert_eq!(after["status"]["capture"]["capture_generation_id"],before["status"]["capture"]["capture_generation_id"]);assert_eq!(after["status"]["capture"]["original_deadline"],before["status"]["capture"]["original_deadline"]);
    (dir,state,restored,app,after)
}
async fn actual_header_observe(state:&SharedState,sequence:u32) -> f64 {
    let packet=wire(15,sequence);let frame=parse_esp32_frame(&packet).unwrap();let mut s=state.write().await;
    observe(&mut s,&frame,&packet,std::time::Instant::now());
    s.room_learning_sources.iter().find(|(k,_)|k.node_id==15).unwrap().1.receiver_rate_hz
}
async fn assert_nonobserving_cadence_does_not_fail(cold:bool) {
    let (_dir,state,rt,app,before)=cadence_priming_fixture(cold).await;
    let rate=actual_header_observe(&state,121).await;assert!(rate>0.&&rate<30.,"fixture must expose a real moving-arrival estimate below recording band: {rate}");
    let after=decoded(app,&format!("/status?schema={STATUS_SCHEMA}"),None).await;
    println!("SYNTHETIC_ACTUAL_INGRESS_PRIMING_CADENCE cold={cold} measured_receiver_hz={rate} capture={}",after["status"]["capture"]);
    assert_eq!(after["status"]["capture"]["stage"],if cold{"paused"}else{"ready"},"{after}");
    for key in ["capture_generation_id","original_deadline","schedule_sha256","retained_windows","durable_windows","sealed_train_episodes","sealed_held_out_episodes","observed_ms","durable_commitment_sha256","continuation_policy_sha256"]{assert_eq!(after["status"]["capture"][key],before["status"]["capture"][key],"{key}");}
    assert_eq!(rt.context().unwrap().revision,1);
}
#[tokio::test]
async fn actual_ingress_ready_zero_window_rate_estimate_does_not_record_or_fail(){assert_nonobserving_cadence_does_not_fail(false).await;}
#[tokio::test]
async fn actual_ingress_cold_paused_zero_window_rate_estimate_does_not_record_or_fail(){assert_nonobserving_cadence_does_not_fail(true).await;}

#[tokio::test]
async fn actual_ingress_low_cadence_lead_in_has_zero_credit_then_observing_rejects() {
    let (_dir,state,rt,app,before)=cadence_priming_fixture(false).await;
    let capture=&before["status"]["capture"];
    let start=decoded(app.clone(),"/phase/start",Some(serde_json::json!({"schema":PHASE_START_REQUEST_SCHEMA,"expected":rt.mutation_guard().unwrap(),"capture_generation_id":capture["capture_generation_id"],"schedule_sha256":capture["schedule_sha256"],"episode_index":0,"conditions_confirmed":true}))).await;
    assert_eq!(start["success"],true,"{start}");
    let wait=std::time::Instant::now();let mut saw_lead=false;let mut failed=false;
    for sequence in 121..221 {
        {let mut s=state.write().await;for node in [15,16,17]{let packet=wire(node,sequence);let frame=parse_esp32_frame(&packet).unwrap();observe(&mut s,&frame,&packet,std::time::Instant::now());}}
        let current=decoded(app.clone(),&format!("/status?schema={STATUS_SCHEMA}"),None).await;
        assert_eq!(current["status"]["capture"]["retained_windows"],0);assert_eq!(current["status"]["capture"]["durable_windows"],0);
        if current["status"]["capture"]["stage"]=="failed" {
            assert!(saw_lead,"must first prove non-observing lead-in accepted without credit");
            assert_eq!(current["status"]["capture"]["failure"],"acquisition_cadence_changed","{current}");failed=true;break;
        }
        if current["status"]["capture"]["lead_in_remaining_ms"].as_u64().unwrap()>0 {
            saw_lead=true;assert_eq!(current["status"]["capture"]["observed_ms"],0);
        }
        assert!(wait.elapsed()<std::time::Duration::from_secs(10));
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(failed,"observing must still reject the unchanged low moving-arrival rate");
}
fn low_rate_native_packet(rt:&RoomLearningRuntime, sequence:u32)->native::CanonicalPacket {
    let bytes=wire(15,sequence);let frame=parse_esp32_frame(&bytes).unwrap();
    native::canonicalize_esp32(native::NativePacketInput{node_id:15,n_antennas:1,raw_bins:64,frequency_hz:2_437_000_000,ppdu_type:frame.ppdu_type,sequence,received_at_ns:rt.receiver_now_ns(),receiver_sample_rate_hz:10.,iq_bytes:&bytes[20..],flags:frame.adr018_flags,provenance:wifi_densepose_sensing_server::room_learner::observation::CaptureProvenance::LiveReceiverObserved}).unwrap()
}
#[tokio::test]
async fn fullrun_priming_still_rejects_replay_and_changed_phy_identity() {
    {let (_dir,state,_rt,app,_)=cadence_priming_fixture(false).await;actual_header_observe(&state,121).await;actual_header_observe(&state,121).await;
     let s=decoded(app,&format!("/status?schema={STATUS_SCHEMA}"),None).await;assert_eq!(s["status"]["capture"]["failure"],"temporal_priming_source_or_fence");assert_eq!(s["status"]["capture"]["retained_windows"],0);}
    {let (_dir,_state,rt,app,_)=cadence_priming_fixture(false).await;let mut p=low_rate_native_packet(&rt,121);p.frequency_hz=2_462_000_000;
     assert_eq!(rt.ingest_native(&p,rt.receiver_now_ns()),Err("acquisition_native_source_changed".into()));
     let s=decoded(app,&format!("/status?schema={STATUS_SCHEMA}"),None).await;assert_eq!(s["status"]["capture"]["retained_windows"],0);}
}
#[tokio::test]
async fn fullrun_priming_still_rejects_zero_nan_and_nonfinite_rate_metadata() {
    for rate in [0.,-1.,f64::NAN,f64::INFINITY] {
        let (_dir,_state,rt,app,_)=cadence_priming_fixture(false).await;
        let mut p=low_rate_native_packet(&rt,121);p.receiver_sample_rate_hz=rate;
        assert_eq!(rt.ingest_native(&p,rt.receiver_now_ns()),Err("acquisition_cadence_changed".into()));
        let s=decoded(app,&format!("/status?schema={STATUS_SCHEMA}"),None).await;assert_eq!(s["status"]["capture"]["retained_windows"],0);assert_eq!(s["status"]["capture"]["observed_ms"],0);
    }
}

#[tokio::test]
async fn retry_owner_router_denies_anonymous_and_paired_grants_before_mutating_runtime(){
    let (_dir,state)=fixture();let rt=state.read().await.room_learning.as_ref().unwrap().clone();
    let auth=wifi_densepose_sensing_server::bearer_auth::AuthState::from_token("synthetic-owner");
    let app=owner_selection_routes(auth).with_state(state);
    let guard=rt.mutation_guard().unwrap();
    let body=serde_json::json!({"schema":RECOVERY_RETRY_REQUEST_SCHEMA,"model_kind":"ruview.room-learner.managed-temporal-input.v3","expected":guard,"capture_generation_id":"c".repeat(64),"retry_confirmed":true});
    for (token,paired) in [(None,false),(Some("synthetic-peer-read"),false),(Some("synthetic-owner"),true)] {
        let mut builder=Request::builder().method(Method::POST).uri("/api/v1/room-learner/recovery/retry").header("content-type","application/json");
        if let Some(token)=token{builder=builder.header("authorization",format!("Bearer {token}"));}
        let mut request=builder.body(Body::from(serde_json::to_vec(&body).unwrap())).unwrap();
        request.extensions_mut().insert(axum::extract::ConnectInfo("127.0.0.1:41000".parse::<SocketAddr>().unwrap()));
        if paired {let(_,rx)=tokio::sync::watch::channel(false);request.extensions_mut().insert(wifi_densepose_sensing_server::companion::Grant{device_id:"synthetic-paired-readonly".into(),expires_at:chrono::Utc::now().timestamp().max(0)as u64+30,revoked:rx});}
        assert_eq!(app.clone().oneshot(request).await.unwrap().status(),StatusCode::FORBIDDEN);
        assert_eq!(rt.mutation_guard().unwrap(),guard);
    }
}

#[tokio::test]
async fn retry_command_refuses_imported_rows_and_wrong_cas_at_the_actual_dispatch(){
    let (_dir,state)=fixture();let rt=state.read().await.room_learning.as_ref().unwrap().clone();let app=temporal_app(state);
    let guard=rt.mutation_guard().unwrap();
    let body=serde_json::json!({"schema":RECOVERY_RETRY_REQUEST_SCHEMA,"model_kind":"ruview.room-learner.managed-temporal-input.v3","expected":guard,"capture_generation_id":"c".repeat(64),"retry_confirmed":true});
    for field in ["rows","recording_start","new_training","deadline","committed"]{
        let mut invalid=body.clone();invalid[field]=serde_json::json!(true);
        assert_eq!(request_raw(app.clone(),"/retry",Some(invalid)).await.0,StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(rt.mutation_guard().unwrap(),guard);
    }
    let mut wrong=body;wrong["expected"]["runtime_epoch"]=serde_json::json!("wrong-runtime");
    let response=decoded(app,"/retry",Some(wrong)).await;
    assert_eq!(response["success"],false);assert_eq!(rt.mutation_guard().unwrap(),guard);
}

#[tokio::test]
async fn actual_owner_retry_http_returns_paused_same_capture_without_starting_recording(){
    use wifi_densepose_sensing_server::room_learner::{observation::SourceSpec,recovery_store::RecoveryStore};
    let(dir,state)=fixture();let owner="a".repeat(64);let path=dir.path().join(&owner).join("SensingServer");
    let store=RecoveryStore::new(&path,&owner,"synthetic-install",Some(&[7;32])).unwrap();
    let rt=Arc::new(RoomLearningRuntime::new_with_recovery(&path,&owner,"synthetic-install",store).unwrap());
    let profile="native-2s-three-source-56bin-operator-eight-step-v1";
    rt.temporal_select_profile_guarded_v2(Some(profile),&rt.mutation_guard().unwrap()).unwrap();
    let sources=(15..=17u8).map(|id|SourceSpec{source_id:id,device_id:format!("esp32-node-{id}"),grid_id:"ant1-bins64-ppdu0-usable-drop2-linear-v2".into(),feature_count:56,signal_kind:ruview_unified::RfModality::WifiCsi,center_frequency_hz:2_437_000_000,bandwidth_hz:20_000_000,link_id:format!("synthetic-{id}"),link_index:0,geometry:None}).collect();
    let context=rt.bind_context("b".repeat(64),1,sources).unwrap();
    rt.temporal_configure_v2(&rt.mutation_guard().unwrap(),context,"synthetic-http-retry".into(),true,rt.receiver_now_ns()).unwrap();
    let started=std::time::Instant::now();
    loop{let status=rt.temporal_status_v2(rt.receiver_now_ns()).unwrap();if status["capture"]["stage"]=="ready"{break;}assert!(started.elapsed()<std::time::Duration::from_secs(10));tokio::time::sleep(std::time::Duration::from_millis(1)).await;}
    rt.record_native_rejection(15,"ant1-bins64-ppdu0-usable-drop2-linear-v2","SYNTHETIC stopped capture for actual HTTP Retry");
    let before=rt.temporal_status_v2(rt.receiver_now_ns()).unwrap();
    assert_eq!(before["capture"]["stage"],"failed");
    state.write().await.room_learning=Some(rt.clone());
    let router=owner_selection_routes(wifi_densepose_sensing_server::bearer_auth::AuthState::from_token("synthetic-owner")).with_state(state);
    let body=serde_json::json!({"schema":RECOVERY_RETRY_REQUEST_SCHEMA,"model_kind":"ruview.room-learner.managed-temporal-input.v3","expected":rt.mutation_guard().unwrap(),"capture_generation_id":before["capture"]["capture_generation_id"],"retry_confirmed":true});
    let mut request=Request::builder().method(Method::POST).uri("/api/v1/room-learner/recovery/retry").header("content-type","application/json").header("authorization","Bearer synthetic-owner").body(Body::from(serde_json::to_vec(&body).unwrap())).unwrap();
    request.extensions_mut().insert(axum::extract::ConnectInfo("127.0.0.1:41000".parse::<SocketAddr>().unwrap()));
    let response=router.oneshot(request).await.unwrap();assert_eq!(response.status(),StatusCode::OK);
    let bytes=axum::body::to_bytes(response.into_body(),65536).await.unwrap();let reply:serde_json::Value=serde_json::from_slice(&bytes).unwrap();
    assert_eq!(reply["success"],true,"{reply}");let after=&reply["status"]["capture"];
    assert_eq!(after["stage"],"paused");assert_eq!(after["durability_pending"],false);assert!(after["failure"].is_null());
    assert_eq!(after["capture_generation_id"],before["capture"]["capture_generation_id"]);assert_eq!(after["original_deadline"],before["capture"]["original_deadline"]);
    assert_eq!(after["retained_windows"],0);assert_eq!(after["durable_windows"],0);assert_eq!(after["observed_ms"],0);
}

/// SYNTHETIC real recovery file-lock contention through the production status
/// composer. The handshake runs inside the actual blocking reader; there is no
/// timing sleep or production Runtime hook standing in for contention.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn temporal_status_recovery_wait_releases_ingress_and_rechecks_authority() {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    use wifi_densepose_sensing_server::room_learner::recovery_store::RecoveryStore;
    for change in ["none", "privacy", "runtime", "profile", "epoch"] {
        let (dir, state) = fixture();
        let owner = "a".repeat(64);
        let path = dir.path().join(&owner).join("SensingServer");
        let recovery = RecoveryStore::new(&path, &owner, "synthetic-install", Some(&[7; 32])).unwrap();
        let rt = Arc::new(RoomLearningRuntime::new_with_recovery(&path, &owner, "synthetic-install", recovery).unwrap());
        state.write().await.room_learning = Some(rt.clone());
        let recovery_path = path.join("room-learner-recovery");
        let prefix_path = recovery_path.join("temporal-prefix-commitment-v1");
        std::fs::create_dir(&prefix_path).unwrap();
        std::fs::set_permissions(&prefix_path, std::fs::Permissions::from_mode(0o700)).unwrap();
        let held = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false)
            .mode(0o600).open(recovery_path.join("temporal-prefix.lock")).unwrap();
        held.lock().unwrap();
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let reader_state = state.clone();
        let reader_runtime = rt.clone();
        let task = tokio::spawn(async move {
            temporal_status_with_reader(&reader_state, &reader_runtime, move |reader, now| {
                entered_tx.send(()).unwrap();
                reader.temporal_status_snapshot_v2(now)
            }).await.0
        });
        tokio::time::timeout(std::time::Duration::from_secs(2), entered_rx).await.unwrap().unwrap();
        // This is exactly the shared write acquisition the live UDP path needs.
        // Before the repair it could not complete until held was unlocked.
        let mut ingress = tokio::time::timeout(std::time::Duration::from_secs(2), state.write()).await
            .expect("recovery file I/O must not hold the RF ingress lock");
        assert!(!task.is_finished(), "the real recovery reader must still be blocked");
        match change {
            "none" => {},
            "privacy" => ingress.privacy_mode = true,
            "runtime" => ingress.room_learning = None,
            "profile" => { rt.temporal_select_profile_guarded_v2(Some("native-2s-three-source-56bin-operator-eight-step-v1"), &rt.mutation_guard().unwrap()).unwrap(); },
            "epoch" => ingress.calibration_boot_epoch.push_str("-changed"),
            _ => unreachable!(),
        }
        drop(ingress);
        held.unlock().unwrap();
        let result = tokio::time::timeout(std::time::Duration::from_secs(2), task).await.unwrap().unwrap();
        if change == "none" {
            assert_eq!(result["success"], true, "{result}");
            assert_eq!(result["status"]["mutation_guard"], serde_json::to_value(rt.mutation_guard().unwrap()).unwrap());
        } else {
            assert_eq!(result["success"], false, "{change}: {result}");
            assert!(result.get("status").is_none(), "{change}: stale snapshot must not escape");
            assert_eq!(result["error_code"], if change == "privacy" {"learner_privacy_mode"} else {"learner_state_changed"});
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_status_retains_one_reader_permit_until_actual_work_finishes() {
    let (_dir, state) = fixture();
    let rt = state.read().await.room_learning.clone().unwrap();
    let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let blocked_state = state.clone();
    let blocked_rt = rt.clone();
    let blocked = tokio::spawn(async move {
        temporal_status_with_reader(&blocked_state, &blocked_rt, move |reader, now| {
            entered_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            reader.temporal_status_snapshot_v2(now)
        }).await
    });
    tokio::time::timeout(std::time::Duration::from_secs(2), entered_rx).await.unwrap().unwrap();
    blocked.abort();
    assert!(blocked.await.unwrap_err().is_cancelled());
    // Poll a real waiter once: the aborted HTTP task must not have released
    // the permit still owned by its actual blocking reader.
    let mut direct_wait = Box::pin(rt.acquire_temporal_status_reader());
    std::future::poll_fn(|cx| {
        assert!(std::future::Future::poll(direct_wait.as_mut(), cx).is_pending());
        std::task::Poll::Ready(())
    }).await;
    drop(direct_wait);
    let invoked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut waiters = Vec::new();
    for _ in 0..3 {
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let waiter_state = state.clone();
        let waiter_rt = rt.clone();
        let calls = invoked.clone();
        waiters.push(tokio::spawn(async move {
            started_tx.send(()).unwrap();
            temporal_status_with_reader(&waiter_state, &waiter_rt, move |reader, now| {
                calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                reader.temporal_status_snapshot_v2(now)
            }).await
        }));
        tokio::time::timeout(std::time::Duration::from_secs(2), started_rx).await.unwrap().unwrap();
    }
    assert_eq!(invoked.load(std::sync::atomic::Ordering::SeqCst), 0,
        "concurrent async waiters must not spawn more blocking readers");
    for waiter in waiters {
        assert!(!waiter.is_finished());
        waiter.abort();
        assert!(waiter.await.unwrap_err().is_cancelled());
    }
    // Cancellation did not retain the RF ingress lock either.
    drop(tokio::time::timeout(std::time::Duration::from_secs(2), state.write()).await.unwrap());
    release_tx.send(()).unwrap();
    let permit = tokio::time::timeout(std::time::Duration::from_secs(2), rt.acquire_temporal_status_reader()).await
        .expect("permit must return only after real blocking work exits").unwrap();
    drop(permit);
    assert_eq!(invoked.load(std::sync::atomic::Ordering::SeqCst), 0,
        "explicitly canceled async waiters must never run later");
    let response = temporal_status_with_reader(&state, &rt, |reader, now| reader.temporal_status_snapshot_v2(now)).await.0;
    assert_eq!(response["success"], true, "{response}");
}
