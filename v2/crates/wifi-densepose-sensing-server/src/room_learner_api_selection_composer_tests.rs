//! SYNTHETIC public-runtime training and actual publication/API composition.
//! These tests grant no hardware accuracy or real-room qualification evidence.
use super::*;
use wifi_densepose_sensing_server::room_learner::{
    capture::{RawSourceFrame, MIN_PHASE_MS, WINDOW_PACKETS},
    observation::RoomLabel,
};

#[path = "room_learner_api_selection_fixture.rs"]
mod fixture_support;
use fixture_support::{feed, fitted, owner_post, update};

#[tokio::test]
async fn actual_selected_composer_withdraws_without_fallback_and_never_grants_auxiliary_authority()
{
    let (_dir, state, rt, binding) = fitted().await;
    let mut out = update();
    {
        let inner = state.read().await;
        project_live_selection(&inner, &mut out);
    }
    let proof = out.selected_binary_evidence.as_ref().unwrap();
    assert_eq!(proof["status"], "ready", "{proof}");
    assert_eq!(proof["decision"], "occupied");
    assert_eq!(proof["presence"], true);
    assert_eq!(
        out.classification_source.as_deref(),
        Some("legacy_control_comparison")
    );
    assert!(
        out.estimated_persons.is_none() && out.persons.is_none() && out.pose_keypoints.is_none()
    );
    assert!(
        out.vital_signs.is_none()
            && out.vital_evidence.is_none()
            && out.enhanced_breathing.is_none()
            && out.posture.is_none()
    );
    assert!(derive_pose_from_sensing(&out).is_empty());
    // Exercise the actual RuField emitter with a valid governed cycle: the
    // legacy positive control emits, but the selected binary update cannot be
    // relabelled as node motion/location/count evidence.
    {
        let mut inner = state.write().await;
        let mut engine_nodes = HashMap::new();
        for id in [15u8, 16] {
            let mut node = NodeState::new();
            node.frame_history = std::collections::VecDeque::from(vec![(0..56)
                .map(|bin| 1.0 + f64::from(id) * 0.01 + bin as f64 * 0.1)
                .collect()]);
            node.last_frame_time = Some(std::time::Instant::now());
            engine_nodes.insert(id, node);
        }
        assert!(inner
            .engine_bridge
            .observe_cycle(&engine_nodes, 1000)
            .is_some());
        emit_rufield_event(&inner, &update(), 15);
        let emitted = inner.field_surface.read().await.recent().len();
        assert!(
            emitted > 0,
            "valid legacy positive control must reach RuField"
        );
        emit_rufield_event(&inner, &out, 15);
        assert_eq!(inner.field_surface.read().await.recent().len(), emitted);
    }
    let observed = proof["observed_at_unix_ms"].clone();
    {
        let mut inner = state.write().await;
        inner.latest_update = Some(out.clone());
    }
    let cached = current_sensing_snapshot(&*state.read().await).unwrap();
    assert_eq!(
        cached.selected_binary_evidence.as_ref().unwrap()["observed_at_unix_ms"],
        observed
    );
    // A fresh Empty model result overrides no legacy fields by implication:
    // the authoritative binary proof is false while the explicitly labelled
    // comparison still contains its previous positive heuristic.
    let grid = rt.context().unwrap().sources[0].grid_id.clone();
    feed(&rt, &grid, RoomLabel::Empty, 2000, rt.receiver_now_ns());
    tokio::time::sleep(std::time::Duration::from_millis(12)).await;
    let mut empty = update();
    project_live_selection(&*state.read().await, &mut empty);
    let evidence = empty.selected_binary_evidence.as_ref().unwrap();
    assert_eq!(evidence["status"], "ready", "{evidence}");
    assert_eq!(evidence["decision"], "empty");
    assert_eq!(evidence["presence"], false);
    assert!(
        empty.classification.presence,
        "legacy comparator stays source-labelled"
    );
    let queued_original = serde_json::to_string(&out).unwrap();
    let retained = refresh_queued_update(&*state.read().await, &queued_original).unwrap();
    let retained: serde_json::Value = serde_json::from_str(&retained).unwrap();
    assert_eq!(
        retained["selected_binary_evidence"]["observed_at_unix_ms"],
        observed
    );
    assert_eq!(
        retained["selected_binary_evidence"]["decision"], "occupied",
        "rolling cohort advancement must not relabel an earlier still-fresh measured decision"
    );
    let mut expired: serde_json::Value = serde_json::from_str(&queued_original).unwrap();
    expired["selected_binary_evidence"]["observed_at_unix_ms"] =
        serde_json::json!(rt.receiver_now_ns() / 1_000_000 - 6000);
    let expired = refresh_queued_update(&*state.read().await, &expired.to_string()).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&expired).unwrap()["selected_binary_evidence"]
            ["status"],
        "unavailable"
    );
    let boot = state.read().await.calibration_boot_epoch.clone();
    state.write().await.calibration_boot_epoch = "synthetic-other-boot".into();
    let withdrawn_boot = refresh_queued_update(&*state.read().await, &queued_original).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&withdrawn_boot).unwrap()
            ["selected_binary_evidence"]["status"],
        "unavailable"
    );
    state.write().await.calibration_boot_epoch = boot;
    state.write().await.latest_update = Some(empty);

    // Legacy preferred-grid bookkeeping is not the learner input authority.
    // Its stale/missing node cache must not erase a valid native learner cohort.
    let removed = state.write().await.node_states.remove(&16).unwrap();
    let last_seen = state
        .write()
        .await
        .node_states
        .get_mut(&15)
        .unwrap()
        .last_frame_time
        .take();
    let cached = current_sensing_snapshot(&*state.read().await).unwrap();
    assert_eq!(cached.selected_binary_evidence.unwrap()["status"], "ready");
    state.write().await.node_states.insert(16, removed);
    state
        .write()
        .await
        .node_states
        .get_mut(&15)
        .unwrap()
        .last_frame_time = last_seen;
    let wrong_room =
        companion_room::SharedRoomStore::load(&_dir.path().join("other-room"), "synthetic-install")
            .unwrap();
    let original = state
        .write()
        .await
        .current_room_store
        .replace(wrong_room)
        .unwrap();
    let mut withdrawn = update();
    project_live_selection(&*state.read().await, &mut withdrawn);
    assert_eq!(
        withdrawn.selected_binary_evidence.as_ref().unwrap()["status"],
        "unavailable"
    );
    state.write().await.current_room_store = Some(original);
    assert_eq!(
        rt.selection_snapshot(rt.receiver_now_ns(), Some(&binding))
            .selected
            .unwrap()
            .room_digest,
        binding
    );
    // Every read-only composer withholds auxiliary authority while selected.
    let vitals = vital_signs_endpoint(State(state.clone())).await.0;
    assert!(vitals["vital_signs"]["heart_rate_bpm"].is_null());
    assert_eq!(vitals["authority"], "abstained");
    let pose = pose_current(
        State(state.clone()),
        Query(PoseCurrentQuery {
            view: pose_physics::PoseView::Raw,
        }),
    )
    .await
    .into_response();
    assert_eq!(pose.status(), StatusCode::CONFLICT);

    assert!(edge_vitals_endpoint(State(state.clone())).await.0["edge_vitals"].is_null());
    assert_eq!(
        pose_zones_summary(State(state.clone())).await.0["authority"],
        "withheld"
    );
    let read = selection(State(state.clone())).await.0;
    let serialized = serde_json::to_string(&read).unwrap();
    for forbidden in [
        "owner_scope",
        "mutation_guard",
        "encoder_weights",
        "training_samples",
        "head_weights",
        "geometry",
    ] {
        assert!(
            !serialized.contains(forbidden),
            "paired read leaked {forbidden}"
        );
    }
    let old_json =
        serde_json::to_string(state.read().await.latest_update.as_ref().unwrap()).unwrap();
    {
        let inner = state.read().await;
        assert!(
            refresh_queued_update(&inner, r#"{"type":"edge_vitals","heart_rate_bpm":62}"#)
                .is_none()
        );
        assert!(refresh_queued_update(
            &inner,
            r#"{"type":"edge_fused_vitals","heartrate_bpm":62}"#
        )
        .is_none());
    }
    state.write().await.privacy_mode = true;
    let privacy = refresh_queued_update(&*state.read().await, &old_json).unwrap();
    let privacy: serde_json::Value = serde_json::from_str(&privacy).unwrap();
    let private_proof = &privacy["selected_binary_evidence"];
    assert_eq!(private_proof["status"], "unavailable");
    assert!(private_proof["identity"].is_null() && private_proof["boot_epoch"].is_null());
    assert!(private_proof.get("presence").is_none() && private_proof.get("decision").is_none());
    assert_eq!(selection(State(state.clone())).await.0["success"], false);
    state.write().await.privacy_mode = false;

    // A selected native source failure clears the actual cohort. Refilling only
    // one of the required inputs cannot borrow the old last-good other source.
    rt.record_native_rejection(16, &grid, "synthetic selected source interruption");
    let at = rt.receiver_now_ns();
    for sample in 0..WINDOW_PACKETS {
        let values: Vec<_> = (0..56)
            .map(|bin| 2.3 + bin as f64 * 0.04 + sample as f64 * 0.003)
            .collect();
        rt.ingest(
            RawSourceFrame {
                source_id: 15,
                grid_id: &grid,
                frequency_hz: 2_437_000_000,
                bandwidth_hz: 20_000_000,
                sequence: 4000 + sample as u32,
                amplitudes: &values,
            },
            at + sample as u64 * 1_000_000 + 15,
        )
        .unwrap();
    }
    tokio::time::sleep(std::time::Duration::from_millis(12)).await;
    let mut missing = update();
    project_live_selection(&*state.read().await, &mut missing);
    let missing = missing.selected_binary_evidence.unwrap();
    assert_eq!(missing["status"], "unavailable");
    assert!(missing.get("presence").is_none());
    assert!(
        rt.selection_snapshot(rt.receiver_now_ns(), Some(&binding))
            .selected
            .is_some(),
        "withdrawal preserves selection history"
    );

    // Explicit owner deselection restores legacy control; it is never automatic.
    let response = owner_post(
        state.clone(),
        "/api/v1/room-learner/deselect",
        serde_json::json!({"expected":rt.mutation_guard().unwrap()}),
    )
    .await;
    assert_eq!(response["success"], true, "{response}");
    let mut legacy = update();
    project_live_selection(&*state.read().await, &mut legacy);
    assert!(legacy.selected_binary_evidence.is_none());
    assert_eq!(legacy.estimated_persons, Some(3));
    let queued = refresh_queued_update(&*state.read().await, &old_json).unwrap();
    let queued: serde_json::Value = serde_json::from_str(&queued).unwrap();
    assert!(queued.get("selected_binary_evidence").is_none());
    assert!(queued.get("classification_source").is_none());
}

/// Explicit opt-in producer fixture for the actual TypeScript wire consumer.
/// All input, training and labels are SYNTHETIC; no device/user state is read.
#[tokio::test]
async fn export_actual_selected_contract_cases() {
    async fn capture_case(
        name: &str,
        state: SharedState,
        rt: &RoomLearningRuntime,
    ) -> serde_json::Value {
        let mut sensing = update();
        project_live_selection(&*state.read().await, &mut sensing);
        let response = Router::new()
            .route("/api/v1/room-learner/selection", get(selection))
            .with_state(state.clone())
            .oneshot(
                Request::builder()
                    .uri("/api/v1/room-learner/selection")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 65536)
            .await
            .unwrap();
        let paired_selection: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let owner_status = status(State(state)).await.0;
        serde_json::json!({
            "owner_status": owner_status,
            "name": name,
            "captured_at_unix_ms": rt.receiver_now_ns() / 1_000_000,
            "sensing_update": sensing,
            "paired_selection": paired_selection,
        })
    }
    let (_dir, state, rt, _binding) = fitted().await;
    let grid = rt.context().unwrap().sources[0].grid_id.clone();
    let occupied = capture_case("ready_occupied", state.clone(), &rt).await;
    assert_eq!(
        occupied["sensing_update"]["selected_binary_evidence"]["decision"],
        "occupied"
    );
    feed(&rt, &grid, RoomLabel::Empty, 2000, rt.receiver_now_ns());
    tokio::time::sleep(std::time::Duration::from_millis(12)).await;
    let empty = capture_case("ready_empty", state.clone(), &rt).await;
    assert_eq!(
        empty["sensing_update"]["selected_binary_evidence"]["decision"],
        "empty"
    );
    rt.record_native_rejection(16, &grid, "synthetic missing selected source");
    let unavailable = capture_case("unavailable_missing_selected_source", state.clone(), &rt).await;
    assert_eq!(
        unavailable["sensing_update"]["selected_binary_evidence"]["status"],
        "unavailable"
    );
    state.write().await.privacy_mode = true;
    let privacy = capture_case("privacy_withheld", state.clone(), &rt).await;
    assert_eq!(privacy["paired_selection"]["success"], false);
    assert!(privacy["sensing_update"]["selected_binary_evidence"]["identity"].is_null());
    let producer_sha = std::env::var("RUVIEW_TEST_PRODUCER_SHA").ok();
    let fixture = serde_json::json!({
        "schema": "ruview.test.selected-contract-cases.v1",
        "evidence": "SYNTHETIC_SOFTWARE_COMPOSITION_NOT_PHYSICAL_QUALIFICATION",
        "producer_commit": producer_sha,
        "producer_path": "real public runtime capture/train/evaluate/qualify/select; actual GET selection handler and final SensingUpdate projector",
        "clock_contract": "Use each captured_at_unix_ms as an injected parser clock; stored fixtures are not fresh live observations.",
        "authentication_scope": "owner and paired-read policy tested separately; this fixture exercises the real bounded GET response producer",
        "cases": [occupied, empty, unavailable, privacy],
    });
    if let Ok(output) = std::env::var("RUVIEW_TEST_SELECTED_CONTRACT_OUTPUT") {
        use std::io::Write;
        #[cfg(unix)]
        use std::os::unix::fs::OpenOptionsExt;
        let sha = producer_sha
            .as_deref()
            .expect("explicit fixture export requires producer commit");
        assert!(sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit()));
        let path = std::path::PathBuf::from(output);
        assert!(path.is_absolute());
        let parent = path.parent().unwrap().canonicalize().unwrap();
        let system_tmp = std::env::temp_dir().canonicalize().unwrap();
        assert!(
            parent.starts_with(&system_tmp)
                || parent == std::path::Path::new("/private/tmp")
                || parent == std::path::Path::new("/tmp"),
            "test export is confined to temporary storage"
        );
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options
            .open(&path)
            .expect("never overwrite an existing fixture");
        serde_json::to_writer_pretty(&mut file, &fixture).unwrap();
        file.write_all(b"\n").unwrap();
        file.sync_all().unwrap();
        println!("RUVIEW_SELECTED_CONTRACT_FILE={}", path.display());
    }
}
