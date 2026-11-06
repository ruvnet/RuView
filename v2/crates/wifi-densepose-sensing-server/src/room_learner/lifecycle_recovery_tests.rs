//! SYNTHETIC process-loss and disk-failure integration; no real account or clock.
use super::*;
use crate::room_learner::{capture::RawSourceFrame, recovery_store::RecoveryStore};
const BASE: u64 = 1_800_000_000_000_000_000;
fn runtime(root: &Path, now: u64, key: bool) -> Arc<RoomLearningRuntime> {
    let owner = "a".repeat(64);
    let path = root.join(&owner).join("SensingServer");
    let seed = [7u8; 32];
    let store =
        RecoveryStore::new(&path, &owner, "synthetic-install", key.then_some(&seed)).unwrap();
    Arc::new(
        RoomLearningRuntime::new_with_recovery_at(&path, &owner, "synthetic-install", store, now)
            .unwrap(),
    )
}
fn context() -> CaptureContext {
    super::tests::context()
}
fn request(duration: u64) -> PhaseRequest {
    PhaseRequest {
        trial_id: "original-empty".into(),
        session_id: "original-train".into(),
        role: DataRole::Train,
        label: RoomLabel::Empty,
        duration_ms: duration,
        lead_in_ms: 0,
        conditions_confirmed: true,
    }
}
fn window(rt: &RoomLearningRuntime, at: u64, sequence: u32) {
    for row in 0..8 {
        for source in 1..=2 {
            let values = vec![6.0 + f64::from(source) * 0.3; 8];
            rt.ingest(
                RawSourceFrame {
                    source_id: source,
                    grid_id: "ht64-canonical8",
                    frequency_hz: 2_437_000_000,
                    bandwidth_hz: 20_000_000,
                    sequence: sequence + row,
                    amplitudes: &values,
                },
                at + u64::from(row) * 10_000_000 + u64::from(source),
            )
            .unwrap();
        }
    }
}
#[test]
fn completed_six_hundred_second_capture_survives_process_loss_without_auto_confirmation() {
    let temp = tempfile::tempdir().unwrap();
    let rt = runtime(temp.path(), BASE, true);
    let ctx = context();
    rt.configure_for_recovery(
        &rt.mutation_guard().unwrap(),
        ctx.clone(),
        "original-run".into(),
        true,
        true,
        BASE,
    )
    .unwrap();
    rt.begin_phase(&rt.mutation_guard().unwrap(), request(600_000), BASE)
        .unwrap();
    for quarter in 0..4 {
        window(
            &rt,
            BASE + u64::from(quarter) * 150_000_000_000 + 1_000_000,
            quarter * 8 + 1,
        );
    }
    let complete = rt.status(BASE + 600_000_000_000);
    assert_eq!(complete["recovery"]["saved"]["phase"]["complete"], true);
    let id = complete["recovery"]["discard_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let original_created = complete["recovery"]["saved"]["created_at_ms"].clone();
    let original_expiry = complete["recovery"]["saved"]["expires_at_ms"].clone();
    drop(rt); // The only process state holding live/pending buffers is gone.
    let cold = runtime(temp.path(), BASE + 601_000_000_000, true);
    assert!(cold.context().is_none());
    cold.restore_capture_recovery(
        &cold.mutation_guard().unwrap(),
        &id,
        &ctx,
        BASE + 601_000_000_000,
    )
    .unwrap();
    let restored = cold.status(BASE + 601_000_000_000);
    assert_eq!(
        restored["recovery"]["restored_disposition"],
        "completed_needs_confirmation"
    );
    assert_eq!(
        restored["recovery"]["saved"]["created_at_ms"],
        original_created
    );
    assert_eq!(
        restored["recovery"]["saved"]["expires_at_ms"],
        original_expiry
    );
    assert_eq!(restored["capture"]["sealed_cohorts"], 0);
    assert!(restored["capture"]["sources"]
        .as_array()
        .unwrap()
        .iter()
        .all(|s| s["live_buffer_frames"] == 0));
    assert!(cold
        .finish_phase(
            &cold.mutation_guard().unwrap(),
            BASE + 601_000_000_000,
            true
        )
        .is_err());
    window(&cold, BASE + 601_000_000_001, 33);
    cold.finish_phase(
        &cold.mutation_guard().unwrap(),
        BASE + 601_100_000_000,
        true,
    )
    .unwrap();
    assert_eq!(
        cold.status(BASE + 601_100_000_000)["capture"]["sealed_cohorts"],
        4
    );
    assert_eq!(restored["raw_iq_persisted"], false);
    assert_eq!(restored["derived_samples_persisted"], true);
    assert_eq!(restored["physically_validated"], false);
}
#[test]
fn unavailable_key_blocks_opt_in_without_starting_capture_and_ram_remains_explicit() {
    let temp = tempfile::tempdir().unwrap();
    let rt = runtime(temp.path(), BASE, false);
    assert!(rt
        .configure_for_recovery(
            &rt.mutation_guard().unwrap(),
            context(),
            "run".into(),
            true,
            true,
            BASE
        )
        .is_err());
    assert!(rt.context().is_none());
    assert_eq!(rt.status(BASE)["recovery"]["state"], "unavailable");
    rt.configure_for_recovery(
        &rt.mutation_guard().unwrap(),
        context(),
        "ram".into(),
        true,
        false,
        BASE,
    )
    .unwrap();
    assert_eq!(
        rt.status(BASE)["untrained_capture_progress_is_volatile"],
        true
    );
}
#[test]
fn failed_initial_save_is_visible_and_blocks_long_capture_until_explicit_ram_configure() {
    let temp = tempfile::tempdir().unwrap();
    let rt = runtime(temp.path(), BASE, true);
    let dir = temp
        .path()
        .join("a".repeat(64))
        .join("SensingServer/room-learner-recovery");
    // A conflicting non-file destination is an owned fixture only.
    let file = dir.join("capture-recovery-v1.bin");
    std::fs::create_dir(&file).unwrap();
    assert!(rt
        .configure_for_recovery(
            &rt.mutation_guard().unwrap(),
            context(),
            "blocked".into(),
            true,
            true,
            BASE
        )
        .is_err());
    assert!(rt
        .begin_phase(&rt.mutation_guard().unwrap(), request(600_000), BASE)
        .is_err());
    assert_ne!(rt.status(BASE)["recovery"]["state"], "saved");
    assert!(file.is_dir());
}
#[test]
fn discard_fences_later_status_and_ingress_writes_and_does_not_remove_models() {
    let temp = tempfile::tempdir().unwrap();
    let rt = runtime(temp.path(), BASE, true);
    rt.configure_for_recovery(
        &rt.mutation_guard().unwrap(),
        context(),
        "retained".into(),
        true,
        true,
        BASE,
    )
    .unwrap();
    let id = rt.status(BASE)["recovery"]["discard_id"]
        .as_str()
        .unwrap()
        .to_owned();
    rt.discard_capture_recovery(&rt.mutation_guard().unwrap(), &id)
        .unwrap();
    window(&rt, BASE + 100_000_000, 1);
    let status = rt.status(BASE + 200_000_000);
    assert_eq!(status["recovery"]["enabled"], false);
    assert!(status["recovery"]["saved"].is_null());
    let owner = "a".repeat(64);
    let path = temp.path().join(&owner).join("SensingServer");
    assert!(
        RecoveryStore::new(&path, &owner, "synthetic-install", Some(&[7; 32]))
            .unwrap()
            .inspect()
            .unwrap()
            .is_none()
    );
}

#[cfg(unix)]
#[test]
fn failed_completion_checkpoint_retries_after_bounded_backoff_with_original_expiry() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let rt = runtime(temp.path(), BASE, true);
    rt.configure_for_recovery(
        &rt.mutation_guard().unwrap(),
        context(),
        "retry-run".into(),
        true,
        true,
        BASE,
    )
    .unwrap();
    rt.begin_phase(&rt.mutation_guard().unwrap(), request(600_000), BASE)
        .unwrap();
    for quarter in 0..4 {
        window(
            &rt,
            BASE + u64::from(quarter) * 150_000_000_000 + 1_000_000,
            quarter * 8 + 1,
        );
    }
    let expiry = rt.status(BASE + 450_100_000_000)["recovery"]["saved"]["expires_at_ms"].clone();
    let file = temp
        .path()
        .join("a".repeat(64))
        .join("SensingServer/room-learner-recovery/capture-recovery-v1.bin");
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
    let failed = rt.status(BASE + 600_000_000_000);
    assert_eq!(failed["recovery"]["state"], "error");
    assert_persisted_flags(&failed, Value::Null);
    assert_eq!(failed["recovery"]["saved"]["phase"]["complete"], false);
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(
        rt.status(BASE + 601_000_000_000)["recovery"]["state"],
        "error"
    );
    let retried = rt.status(BASE + 605_000_000_000);
    assert_eq!(retried["recovery"]["state"], "saved");
    assert_eq!(retried["recovery"]["saved"]["phase"]["complete"], true);
    assert_eq!(retried["recovery"]["saved"]["expires_at_ms"], expiry);
    rt.finish_phase(&rt.mutation_guard().unwrap(), BASE + 605_000_000_000, true)
        .unwrap();
    assert_eq!(
        rt.status(BASE + 605_000_000_000)["capture"]["sealed_cohorts"],
        4
    );
}
#[test]
fn expired_slot_is_summary_redacted_but_explicitly_discardable() {
    let temp = tempfile::tempdir().unwrap();
    let rt = runtime(temp.path(), BASE, true);
    rt.configure_for_recovery(
        &rt.mutation_guard().unwrap(),
        context(),
        "expiry-run".into(),
        true,
        true,
        BASE,
    )
    .unwrap();
    drop(rt);
    let cold = runtime(temp.path(), BASE + 86_400_000_000_000, true);
    let status = cold.status(BASE + 86_400_000_000_000);
    assert_eq!(status["recovery"]["state"], "expired");
    assert!(status["recovery"]["saved"].is_null());
    let id = status["recovery"]["discard_id"].as_str().unwrap();
    assert!(cold
        .saved_recovery_context(id, BASE + 86_400_000_000_000)
        .is_err());
    cold.discard_capture_recovery(&cold.mutation_guard().unwrap(), id)
        .unwrap();
    assert!(cold.status(BASE + 86_400_000_000_000)["recovery"]["discard_id"].is_null());
}

#[tokio::test]
async fn trained_checkpoint_cold_restores_exact_candidate_without_live_buffers_or_auto_fit() {
    let temp = tempfile::tempdir().unwrap();
    let now = super::tests::START;
    let rt = runtime(temp.path(), now, true);
    let ctx = context();
    rt.configure_for_recovery(
        &rt.mutation_guard().unwrap(),
        ctx.clone(),
        "fit-run".into(),
        true,
        true,
        now,
    )
    .unwrap();
    super::tests::train_phases(&rt);
    rt.start_training(
        &rt.mutation_guard().unwrap(),
        FitConfig {
            epochs: 4,
            ..FitConfig::default()
        },
    )
    .unwrap();
    let ready = super::tests::finished(&rt).await;
    assert_eq!(ready["state"], "ready", "{ready}");
    let id = ready["recovery"]["discard_id"].as_str().unwrap().to_owned();
    let expected =
        super::recovery_control::model_sha(rt.inner.lock().unwrap().model.as_ref().unwrap())
            .unwrap();
    // Move today's candidate pointer after the recovery was saved. Recovery
    // must still load the immutable candidate bound by its original hash.
    let dataset = rt
        .inner
        .lock()
        .unwrap()
        .legacy_capture()
        .unwrap()
        .validated_dataset()
        .unwrap();
    let (replacement, _) = RoomModel::fit_with_cancel(
        &dataset,
        FitConfig {
            epochs: 1,
            ..FitConfig::default()
        },
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_ne!(
        super::recovery_control::model_sha(&replacement).unwrap(),
        expected
    );
    rt.store.save_candidate(&replacement).unwrap();
    drop(rt);
    let cold = runtime(temp.path(), now + 61_000_000_000, true);
    cold.restore_capture_recovery(
        &cold.mutation_guard().unwrap(),
        &id,
        &ctx,
        now + 61_000_000_000,
    )
    .unwrap();
    let status = cold.status(now + 61_000_000_000);
    assert_eq!(status["state"], "ready");
    assert_eq!(status["training_frozen"], true);
    assert_eq!(status["capture"]["sealed_cohorts"], 16);
    assert_eq!(
        super::recovery_control::model_sha(cold.inner.lock().unwrap().model.as_ref().unwrap())
            .unwrap(),
        expected
    );
    assert!(cold.prediction(now + 61_000_000_000).is_err());
    assert!(cold
        .begin_phase(
            &cold.mutation_guard().unwrap(),
            request(12_000),
            now + 61_000_000_000
        )
        .is_err());
    assert_eq!(status["physically_validated"], false);
}
#[test]
fn interrupted_fit_checkpoint_restores_failed_frozen_dataset_without_job() {
    let temp = tempfile::tempdir().unwrap();
    let now = super::tests::START;
    let rt = runtime(temp.path(), now, true);
    let ctx = context();
    rt.configure_for_recovery(
        &rt.mutation_guard().unwrap(),
        ctx.clone(),
        "interrupted-fit".into(),
        true,
        true,
        now,
    )
    .unwrap();
    super::tests::train_phases(&rt);
    // Exact durable boundary immediately before handing work to the fitter.
    {
        let mut state = rt.inner.lock().unwrap();
        state.frozen_training = true;
        state.job.phase = JobPhase::Running;
        rt.checkpoint_recovery(&mut state, now + 60_000_000_000, true)
            .unwrap();
    }
    let id = rt.status(now + 60_000_000_000)["recovery"]["discard_id"]
        .as_str()
        .unwrap()
        .to_owned();
    drop(rt);
    let cold = runtime(temp.path(), now + 61_000_000_000, true);
    cold.restore_capture_recovery(
        &cold.mutation_guard().unwrap(),
        &id,
        &ctx,
        now + 61_000_000_000,
    )
    .unwrap();
    let status = cold.status(now + 61_000_000_000);
    assert_eq!(status["state"], "failed");
    assert_eq!(status["training_frozen"], true);
    assert_eq!(status["capture"]["sealed_cohorts"], 16);
    assert!(cold.inner.lock().unwrap().model.is_none());
}

#[test]
fn partial_phase_restores_restart_required_and_ram_run_preserves_saved_slot() {
    let temp = tempfile::tempdir().unwrap();
    let rt = runtime(temp.path(), BASE, true);
    let ctx = context();
    rt.configure_for_recovery(
        &rt.mutation_guard().unwrap(),
        ctx.clone(),
        "partial-run".into(),
        true,
        true,
        BASE,
    )
    .unwrap();
    rt.begin_phase(&rt.mutation_guard().unwrap(), request(600_000), BASE)
        .unwrap();
    window(&rt, BASE + 1_000_000, 1);
    let id = rt.status(BASE + 100_000_000)["recovery"]["discard_id"]
        .as_str()
        .unwrap()
        .to_owned();
    drop(rt);
    let cold = runtime(temp.path(), BASE + 1_000_000_000, true);
    cold.configure_for_recovery(
        &cold.mutation_guard().unwrap(),
        ctx.clone(),
        "explicit-ram".into(),
        true,
        false,
        BASE + 1_000_000_000,
    )
    .unwrap();
    assert_eq!(
        cold.status(BASE + 1_000_000_000)["recovery"]["discard_id"],
        id
    );
    cold.restore_capture_recovery(
        &cold.mutation_guard().unwrap(),
        &id,
        &ctx,
        BASE + 1_000_000_000,
    )
    .unwrap();
    let s = cold.status(BASE + 1_000_000_000);
    assert_eq!(
        s["recovery"]["restored_disposition"],
        "interrupted_needs_restart"
    );
    assert_eq!(s["capture"]["sealed_cohorts"], 0);
    assert!(s["capture"]["phase"].is_null());
    assert_eq!(s["recovery"]["saved"]["phase"], Value::Null);
}
#[test]
fn failed_erase_is_error_and_disables_future_writer_without_claiming_data_removed() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let rt = runtime(temp.path(), BASE, true);
    rt.configure_for_recovery(
        &rt.mutation_guard().unwrap(),
        context(),
        "erase-fail".into(),
        true,
        true,
        BASE,
    )
    .unwrap();
    let id = rt.status(BASE)["recovery"]["discard_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let file = temp
        .path()
        .join("a".repeat(64))
        .join("SensingServer/room-learner-recovery/capture-recovery-v1.bin");
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(rt
        .discard_capture_recovery(&rt.mutation_guard().unwrap(), &id)
        .is_err());
    let s = rt.status(BASE + 10_000_000_000);
    assert_eq!(s["recovery"]["enabled"], false);
    assert_eq!(s["recovery"]["state"], "error");
    assert_eq!(s["recovery"]["discard_id"], id);
    assert!(file.exists());
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    rt.discard_capture_recovery(&rt.mutation_guard().unwrap(), &id)
        .unwrap();
    assert!(!file.exists());
}

fn assert_persisted_flags(status: &Value, expected: Value) {
    assert_eq!(status["derived_samples_persisted"], expected, "{status}");
    assert_eq!(status["raw_observations_persisted"], expected);
    assert_eq!(status["recovery"]["derived_samples_persisted"], expected);
    assert_eq!(status["raw_iq_persisted"], false);
    assert_eq!(status["recovery"]["raw_iq_persisted"], false);
}
fn assert_unverified_retained_contents(case: &str) {
    let temp = tempfile::tempdir().unwrap();
    let rt = runtime(temp.path(), BASE, true);
    rt.configure_for_recovery(
        &rt.mutation_guard().unwrap(),
        context(),
        "retained-flags".into(),
        true,
        true,
        BASE,
    )
    .unwrap();
    rt.begin_phase(&rt.mutation_guard().unwrap(), request(600_000), BASE)
        .unwrap();
    window(&rt, BASE + 1_000_000, 1);
    assert_persisted_flags(&rt.status(BASE + 100_000_000), json!(true));
    drop(rt);
    let owner = "a".repeat(64);
    let path = temp.path().join(&owner).join("SensingServer");
    let file = path.join("room-learner-recovery/capture-recovery-v1.bin");
    if case == "corrupt" {
        let mut bytes = std::fs::read(&file).unwrap();
        *bytes.last_mut().unwrap() ^= 1;
        std::fs::write(&file, bytes).unwrap();
    }
    let retained = std::fs::read(&file).unwrap();
    let now = if case == "expired" {
        BASE + 86_400_000_000_000
    } else {
        BASE + 1_000_000_000
    };
    let seed = if case == "wrong_key" {
        [8; 32]
    } else {
        [7; 32]
    };
    let store = RecoveryStore::new(
        &path,
        &owner,
        "synthetic-install",
        (case != "missing_key").then_some(&seed),
    )
    .unwrap();
    let cold =
        RoomLearningRuntime::new_with_recovery_at(&path, &owner, "synthetic-install", store, now)
            .unwrap();
    let status = cold.status(now);
    assert_persisted_flags(&status, Value::Null);
    assert!(status["recovery"]["error"].is_string());
    assert!(status["recovery"]["saved"].is_null());
    assert_eq!(
        std::fs::read(&file).unwrap(),
        retained,
        "{case} altered retained encrypted bytes"
    );
}
#[test]
fn persisted_flags_expired_contents_are_unknown_and_retained() {
    assert_unverified_retained_contents("expired");
}
#[test]
fn persisted_flags_corrupt_contents_are_unknown_and_retained() {
    assert_unverified_retained_contents("corrupt");
}
#[test]
fn persisted_flags_wrong_key_contents_are_unknown_and_retained() {
    assert_unverified_retained_contents("wrong_key");
}
#[test]
fn persisted_flags_missing_key_contents_are_unknown_and_retained() {
    assert_unverified_retained_contents("missing_key");
}
#[test]
fn persisted_flags_confirmed_empty_discard_and_default_ram_are_false() {
    let temp = tempfile::tempdir().unwrap();
    let ram = super::tests::runtime(temp.path());
    assert_persisted_flags(&ram.status(BASE), json!(false));
    drop(ram);
    let rt = runtime(temp.path(), BASE, true);
    assert_persisted_flags(&rt.status(BASE), json!(false));
    rt.configure_for_recovery(
        &rt.mutation_guard().unwrap(),
        context(),
        "verified-empty".into(),
        true,
        true,
        BASE,
    )
    .unwrap();
    let empty = rt.status(BASE);
    assert_persisted_flags(&empty, json!(false));
    let id = empty["recovery"]["discard_id"].as_str().unwrap();
    rt.begin_phase(&rt.mutation_guard().unwrap(), request(600_000), BASE)
        .unwrap();
    window(&rt, BASE + 1_000_000, 1);
    assert_persisted_flags(&rt.status(BASE + 100_000_000), json!(true));
    rt.discard_capture_recovery(&rt.mutation_guard().unwrap(), id)
        .unwrap();
    assert_persisted_flags(&rt.status(BASE), json!(false));
}

fn recovery_file(root: &Path) -> std::path::PathBuf {
    root.join("a".repeat(64)).join("SensingServer/room-learner-recovery/capture-recovery-v1.bin")
}
fn expiry_request(role: DataRole, label: RoomLabel, index: u32) -> PhaseRequest {
    PhaseRequest {
        trial_id: format!("expiry-trial-{index}"), session_id: format!("expiry-session-{index}"),
        role, label, duration_ms: 12_000, lead_in_ms: 0, conditions_confirmed: true,
    }
}
fn expiry_phase(rt: &RoomLearningRuntime, at: u64, role: DataRole, label: RoomLabel, index: u32) {
    rt.begin_phase(&rt.mutation_guard().unwrap(), expiry_request(role, label, index), at).unwrap();
    let gain = match label { RoomLabel::Empty => 2.0, RoomLabel::OccupiedStatic => 6.0,
        RoomLabel::OccupiedMoving => 7.0, RoomLabel::AdjacentOnly => 2.3, RoomLabel::Unknown => panic!("label required") };
    for quarter in 0..4u32 {
        for sample in 0..8u32 {
            for source in 1..=2u8 {
                let values: Vec<_> = (0..8).map(|bin| gain + f64::from(source)*0.3 + f64::from(bin)*0.04 + f64::from(sample)*0.003).collect();
                rt.ingest(RawSourceFrame {source_id: source, grid_id: "ht64-canonical8",
                    frequency_hz: 2_437_000_000, bandwidth_hz: 20_000_000,
                    sequence: index*32 + quarter*8 + sample + 1, amplitudes: &values},
                    at + u64::from(quarter)*3_000_000_000 + 1_000_000 + u64::from(sample)*10_000_000 + u64::from(source)).unwrap();
            }
        }
    }
    rt.finish_phase(&rt.mutation_guard().unwrap(), at + 12_000_000_000, true).unwrap();
}
fn original_descriptor(root: &Path) -> crate::room_learner::recovery_store::RecoveryDescriptor {
    let owner = "a".repeat(64);
    RecoveryStore::new(&root.join(&owner).join("SensingServer"), &owner, "synthetic-install", Some(&[7;32]))
        .unwrap().inspect().unwrap().unwrap()
}

#[test]
fn loaded_recovery_expires_to_ram_at_exact_deadline_without_erasing_or_rebasing() {
    let temp = tempfile::tempdir().unwrap();
    let rt = runtime(temp.path(), BASE, true);
    rt.configure_for_recovery(&rt.mutation_guard().unwrap(), context(), "expiry-loaded".into(), true, true, BASE).unwrap();
    for index in 0..7 { expiry_phase(&rt, BASE + u64::from(index)*13_000_000_000, DataRole::Train, RoomLabel::Empty, index); }
    let descriptor = original_descriptor(temp.path());
    drop(rt);
    let recovered = runtime(temp.path(), BASE + 100_000_000_000, true);
    recovered.restore_capture_recovery(&recovered.mutation_guard().unwrap(), &descriptor.id, &context(), BASE + 100_000_000_000).unwrap();
    let descriptor = original_descriptor(temp.path());
    let expires = descriptor.expires_at_ms*1_000_000;
    let file = recovery_file(temp.path());
    let bytes = std::fs::read(&file).unwrap();
    let before = recovered.status(expires - 1);
    assert_eq!(before["recovery"]["enabled"], true);
    assert_eq!(before["capture"]["sealed_cohorts"], 28);
    let (rows, history) = {
        let state = recovered.inner.lock().unwrap();let capture = state.legacy_capture().unwrap();
        (serde_json::to_vec(capture.cohorts()).unwrap(), serde_json::to_vec(capture.history()).unwrap())
    };
    let at = recovered.status(expires);
    assert_eq!(at["recovery"]["state"], "expired");assert_eq!(at["recovery"]["enabled"], false);
    assert!(at["recovery"]["saved"].is_null());assert_eq!(at["capture"]["sealed_cohorts"], 28);
    assert_eq!(at["capture"]["sources"], before["capture"]["sources"]);
    assert!(at["recovery"]["error"].as_str().unwrap().contains("memory only"));
    assert!(recovered.inner.lock().unwrap().recovery.error.is_none(), "informational expiry must not become a persistence error");
    assert_eq!(original_descriptor(temp.path()), descriptor);assert_eq!(std::fs::read(&file).unwrap(), bytes);
    let values = vec![6.0;8];
    assert!(recovered.ingest(RawSourceFrame{source_id:1,grid_id:"ht64-canonical8",frequency_hz:2_437_000_000,
        bandwidth_hz:20_000_000,sequence:224,amplitudes:&values},expires+1).unwrap_err().contains("replayed"));
    assert!(recovered.begin_phase(&recovered.mutation_guard().unwrap(), expiry_request(DataRole::Validation,RoomLabel::Empty,0), expires+2).is_err(), "frozen phase roles must survive expiry");
    expiry_phase(&recovered,expires+1_000_000,DataRole::Train,RoomLabel::Empty,7);
    let after = recovered.status(expires+13_000_000_000);
    assert_eq!(after["capture"]["sealed_cohorts"],32);assert!(after["capture"]["phase"].is_null());
    assert_eq!(after["capture"]["sources"][0]["last_accepted_sequence"],256);
    {
        let state=recovered.inner.lock().unwrap();let capture=state.legacy_capture().unwrap();
        assert_eq!(serde_json::to_vec(&capture.cohorts()[..28]).unwrap(),rows);
        assert_eq!(serde_json::to_vec(&capture.history()[..7]).unwrap(),history);
        assert_eq!(capture.cohorts()[28].views[0].sequence_ids[0],225);
    }
    assert_eq!(original_descriptor(temp.path()),descriptor);assert_eq!(std::fs::read(&file).unwrap(),bytes);
    drop(recovered);
    let cold=runtime(temp.path(),expires+14_000_000_000,true);
    assert!(cold.context().is_none());
    assert_eq!(cold.restore_capture_recovery(&cold.mutation_guard().unwrap(),&descriptor.id,&context(),expires+14_000_000_000).unwrap_err(),"recovery_expired");
    assert_eq!(std::fs::read(&file).unwrap(),bytes);
}

#[test]
fn active_phase_can_cross_expiry_and_seal_without_a_false_failed_save() {
    let temp=tempfile::tempdir().unwrap();let rt=runtime(temp.path(),BASE,true);
    rt.configure_for_recovery(&rt.mutation_guard().unwrap(),context(),"crossing-phase".into(),true,true,BASE).unwrap();
    let descriptor=original_descriptor(temp.path());let expires=descriptor.expires_at_ms*1_000_000;
    let start=expires-6_000_000_000;
    rt.begin_phase(&rt.mutation_guard().unwrap(),request(12_000),start).unwrap();
    window(&rt,start+1_000_000,1);window(&rt,start+3_001_000_000,9);
    let bytes=std::fs::read(recovery_file(temp.path())).unwrap();let final_saved=original_descriptor(temp.path());
    window(&rt,expires+1_000_000,17);window(&rt,expires+3_001_000_000,25);
    assert_eq!(rt.status(expires+4_000_000_000)["capture"]["sealed_cohorts"],0,"expiry must not auto-confirm a phase");
    rt.finish_phase(&rt.mutation_guard().unwrap(),expires+6_000_000_000,true).unwrap();
    let status=rt.status(expires+7_000_000_000);
    assert_eq!(status["capture"]["sealed_cohorts"],4);assert!(status["capture"]["phase"].is_null());
    assert_eq!(status["recovery"]["enabled"],false);assert_eq!(original_descriptor(temp.path()),final_saved);
    assert_eq!(std::fs::read(recovery_file(temp.path())).unwrap(),bytes);
}

#[test]
fn expiry_does_not_turn_io_tamper_missing_or_replacement_failures_into_ram_fallback() {
    use std::os::unix::fs::PermissionsExt;
    for case in ["latched_io", "tamper", "missing", "replacement"] {
        let temp=tempfile::tempdir().unwrap();let rt=runtime(temp.path(),BASE,true);
        rt.configure_for_recovery(&rt.mutation_guard().unwrap(),context(),format!("expiry-{case}"),true,true,BASE).unwrap();
        let original=original_descriptor(temp.path());let expires=original.expires_at_ms*1_000_000;
        let file=recovery_file(temp.path());
        match case {
            "latched_io"=>{
                std::fs::set_permissions(&file,std::fs::Permissions::from_mode(0o644)).unwrap();
                {let mut state=rt.inner.lock().unwrap();assert!(rt.checkpoint_recovery(&mut state,BASE+1_000_000,true).is_err());}
                std::fs::set_permissions(&file,std::fs::Permissions::from_mode(0o600)).unwrap();
            },
            "tamper"=>{let mut bytes=std::fs::read(&file).unwrap();let last=bytes.len()-1;bytes[last]^=1;std::fs::write(&file,bytes).unwrap();},
            "missing"=>std::fs::remove_file(&file).unwrap(),
            "replacement"=>{
                let owner="a".repeat(64);let store=RecoveryStore::new(&temp.path().join(&owner).join("SensingServer"),&owner,"synthetic-install",Some(&[7;32])).unwrap();
                store.discard(&original.id).unwrap();let mut other=original.clone();other.id="different-authenticated-copy".into();store.save(&other,b"SYNTHETIC REPLACEMENT",BASE/1_000_000).unwrap();
            },_=>unreachable!()
        }
        let bytes=std::fs::read(&file).ok();
        assert!(rt.begin_phase(&rt.mutation_guard().unwrap(),request(12_000),expires).is_err(),"{case}");
        let status=rt.status(expires+1);
        assert!(status["capture"]["phase"].is_null(),"{case}: failed preflight must not start a phase");
        assert_eq!(status["recovery"]["enabled"],true,"{case}: generic error cannot disable recovery silently");
        assert!(!status["recovery"]["error"].as_str().unwrap().contains("memory only"));
        assert_eq!(std::fs::read(&file).ok(),bytes,"{case}: expiry must not rewrite/delete a failed copy");
    }
}

#[tokio::test]
async fn recovered_28_cohorts_can_finish_heldout_after_expiry_under_the_same_qualification_policy() {
    let temp=tempfile::tempdir().unwrap();let rt=runtime(temp.path(),BASE,true);
    rt.configure_for_recovery(&rt.mutation_guard().unwrap(),context(),"expiry-qualified".into(),true,true,BASE).unwrap();
    for (i,label) in [RoomLabel::Empty,RoomLabel::OccupiedStatic,RoomLabel::OccupiedMoving,RoomLabel::AdjacentOnly].into_iter().enumerate(){
        expiry_phase(&rt,BASE+i as u64*13_000_000_000,DataRole::Train,label,i as u32);
    }
    rt.start_training(&rt.mutation_guard().unwrap(),FitConfig{epochs:8,learning_rate:0.5,hidden_dim:2,embedding_dim:2,temporal_tokens:2,..FitConfig::default()}).unwrap();
    let wait_started=std::time::Instant::now();
    loop {
        let ready=rt.status(BASE+51_000_000_000);
        if ready["state"]=="ready" {break;}
        assert!(ready["state"]=="running" && wait_started.elapsed()<std::time::Duration::from_secs(65),"{ready}");
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    for (i,label) in [RoomLabel::OccupiedStatic,RoomLabel::OccupiedMoving,RoomLabel::AdjacentOnly].into_iter().enumerate(){
        expiry_phase(&rt,BASE+(i+4) as u64*13_000_000_000,DataRole::HeldOut,label,(i+4) as u32);
    }
    let descriptor=original_descriptor(temp.path());drop(rt);
    let recovered=runtime(temp.path(),BASE+100_000_000_000,true);
    recovered.restore_capture_recovery(&recovered.mutation_guard().unwrap(),&descriptor.id,&context(),BASE+100_000_000_000).unwrap();
    let expires=descriptor.expires_at_ms*1_000_000;let bytes=std::fs::read(recovery_file(temp.path())).unwrap();
    assert_eq!(recovered.status(expires)["capture"]["sealed_cohorts"],28);
    let before={let state=recovered.inner.lock().unwrap();serde_json::to_vec(state.legacy_capture().unwrap().cohorts()).unwrap()};
    assert!(recovered.qualify(&recovered.mutation_guard().unwrap()).is_err(),"expiry does not waive missing HeldOut Empty");
    assert!(recovered.status(expires+1)["model_selection"]["selected"].is_null());
    expiry_phase(&recovered,expires+1_000_000,DataRole::HeldOut,RoomLabel::Empty,7);
    let q=recovered.qualify(&recovered.mutation_guard().unwrap()).unwrap();
    assert_eq!(q.receipt.policy,crate::room_learner::qualification::QualificationPolicy::default());
    assert!(q.receipt.criteria.iter().all(|c|c.train_cohorts==4&&c.held_out_cohorts==4));
    assert!(!q.receipt.physically_validated&&!q.receipt.independence_verified);
    let status=recovered.status(expires+13_000_000_000);
    assert!(status["model_selection"]["selected"].is_null(),"qualification must not auto-select");
    assert_eq!(status["control_detector_changed"],false);assert_eq!(status["recovery"]["enabled"],false);
    {let state=recovered.inner.lock().unwrap();assert_eq!(serde_json::to_vec(&state.legacy_capture().unwrap().cohorts()[..28]).unwrap(),before);}
    assert_eq!(std::fs::read(recovery_file(temp.path())).unwrap(),bytes);
    assert_eq!(original_descriptor(temp.path()).expires_at_ms,descriptor.expires_at_ms);
}

#[test]
fn known_expiry_is_sticky_for_out_of_order_status_and_saved_context_reads() {
    let temp=tempfile::tempdir().unwrap();let rt=runtime(temp.path(),BASE,true);
    rt.configure_for_recovery(&rt.mutation_guard().unwrap(),context(),"sticky-status".into(),true,true,BASE).unwrap();
    expiry_phase(&rt,BASE,DataRole::Train,RoomLabel::Empty,0);
    let descriptor=original_descriptor(temp.path());let expires=descriptor.expires_at_ms*1_000_000;
    assert_eq!(rt.status(expires)["recovery"]["state"],"expired");
    let older=rt.status(expires-1_000_000);
    let read=rt.saved_recovery_context(&descriptor.id,expires-1_000_000);
    assert_eq!(older["recovery"]["state"],"expired","an older queued observation must not resurrect recovery availability");
    assert!(older["recovery"]["saved"].is_null());
    assert_eq!(read.unwrap_err(),"recovery_expired");
    assert_eq!(older["recovery"]["enabled"],false);
}

#[test]
fn known_expiry_cannot_be_reenabled_by_a_queued_restore_with_an_earlier_timestamp() {
    for observe_by_status in [true,false] {
        let temp=tempfile::tempdir().unwrap();let rt=runtime(temp.path(),BASE,true);
        rt.configure_for_recovery(&rt.mutation_guard().unwrap(),context(),"sticky-restore".into(),true,true,BASE).unwrap();
        expiry_phase(&rt,BASE,DataRole::Train,RoomLabel::Empty,0);
        let descriptor=original_descriptor(temp.path());let expires=descriptor.expires_at_ms*1_000_000;
        let bytes=std::fs::read(recovery_file(temp.path())).unwrap();
        let rows={let state=rt.inner.lock().unwrap();serde_json::to_vec(state.legacy_capture().unwrap().cohorts()).unwrap()};
        if observe_by_status {
            rt.status(expires);
        } else {
            let mut invalid=expiry_request(DataRole::Train,RoomLabel::Empty,1);invalid.conditions_confirmed=false;
            assert!(rt.begin_phase(&rt.mutation_guard().unwrap(),invalid,expires).is_err());
        }
        let result=rt.restore_capture_recovery(&rt.mutation_guard().unwrap(),&descriptor.id,&context(),expires-1_000_000);
        assert_eq!(result.unwrap_err(),"recovery_expired","queued restore: observe_by_status={observe_by_status}");
        let status=rt.status(expires-1_000_000);
        assert_eq!(status["recovery"]["state"],"expired");assert_eq!(status["recovery"]["enabled"],false);
        assert!(status["recovery"]["saved"].is_null());assert_eq!(status["capture"]["sealed_cohorts"],4);
        assert_eq!(original_descriptor(temp.path()),descriptor);assert_eq!(std::fs::read(recovery_file(temp.path())).unwrap(),bytes);
        let state=rt.inner.lock().unwrap();assert_eq!(serde_json::to_vec(state.legacy_capture().unwrap().cohorts()).unwrap(),rows);
    }
}

#[test]
fn cold_inspection_of_expired_copy_fences_earlier_queued_loads_without_creating_ram_state() {
    let temp=tempfile::tempdir().unwrap();let rt=runtime(temp.path(),BASE,true);
    rt.configure_for_recovery(&rt.mutation_guard().unwrap(),context(),"sticky-cold".into(),true,true,BASE).unwrap();
    expiry_phase(&rt,BASE,DataRole::Train,RoomLabel::Empty,0);
    let descriptor=original_descriptor(temp.path());let expires=descriptor.expires_at_ms*1_000_000;
    let bytes=std::fs::read(recovery_file(temp.path())).unwrap();drop(rt);
    let cold=runtime(temp.path(),expires,true); // Constructor inspection knows expiry before any status request.
    assert_eq!(cold.saved_recovery_context(&descriptor.id,expires-1_000_000).unwrap_err(),"recovery_expired");
    assert_eq!(cold.restore_capture_recovery(&cold.mutation_guard().unwrap(),&descriptor.id,&context(),expires-1_000_000).unwrap_err(),"recovery_expired");
    let status=cold.status(expires-1_000_000);
    assert_eq!(status["recovery"]["state"],"expired");assert_eq!(status["recovery"]["enabled"],false);
    assert!(status["recovery"]["saved"].is_null());assert!(status["capture"].is_null());assert!(!status["candidate_loaded"].as_bool().unwrap());
    assert_eq!(original_descriptor(temp.path()),descriptor);assert_eq!(std::fs::read(recovery_file(temp.path())).unwrap(),bytes);
}

#[path = "lifecycle_recovery_enable_tests.rs"]
mod enable_tests;
