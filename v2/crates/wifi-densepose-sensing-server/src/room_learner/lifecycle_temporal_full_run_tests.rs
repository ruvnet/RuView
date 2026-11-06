//! SYNTHETIC private fresh-v3 genesis/Train-path checks; no physical evidence.
use super::tests as fixture;
use super::*;
use crate::room_learner::{
    native::{canonicalize_esp32, NativePacketInput},
    observation::CaptureProvenance,
};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use wifi_densepose_hardware::{Adr018Flags, PpduType};
#[test]
fn c4a_fresh_genesis_is_preparing_until_actual_ack_in_same_store_and_raw_read_cannot_write() {
    let directory = tempfile::tempdir().unwrap();
    let rt = fixture::runtime(directory.path(), fixture::START);
    rt.configure_temporal_full_run(
        &rt.mutation_guard().unwrap(),
        fixture::context(),
        "synthetic-c4a-fresh".into(),
        fixture::START,
    )
    .unwrap();
    assert_eq!(fixture::snapshot(&rt, fixture::START)["stage"], "preparing");
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(
        fixture::snapshot(&rt, fixture::START)["stage"],
        "preparing",
        "queued storage result is not receiver ACK admission"
    );
    let now = fixture::settle_async_boundary(&rt, fixture::START);
    assert_eq!(fixture::snapshot(&rt, now)["stage"], "ready");
    let store = rt.temporal_store().unwrap();
    let loaded = store.load_temporal(now).unwrap().unwrap();
    let prefix = loaded.prefix.unwrap();
    assert_eq!(prefix.metadata_kind(), "temporal-prefix-commitment-v2");
    assert_eq!(prefix.transition_limit(), 433);
    assert_eq!(prefix.window_count(), 0);
    let data: serde_json::Value = serde_json::from_slice(prefix.bytes()).unwrap();
    assert_eq!(
        data["schema"],
        "ruview.room-learner.temporal-runtime-prefix.v3"
    );
    assert_eq!(data["payload"]["policy"]["body"]["max_total_windows"], 128);
    assert_eq!(data["payload"]["stage"]["kind"], "train");
    assert!(store
        .prepare_temporal(Some(&loaded.token), &prefix, now)
        .err()
        .unwrap()
        .contains("producer_write_grant_required"));
    assert_eq!(
        store.load_temporal(now).unwrap().unwrap().token,
        loaded.token
    );
    let base = directory
        .path()
        .join(fixture::context().owner_scope)
        .join("SensingServer/room-learner-recovery");
    assert!(base
        .join("temporal-prefix-commitment-v1/temporal-head.bin")
        .is_file());
    assert!(!base.join("temporal-prefix-commitment-v2").exists());
    if let Some(destination) = std::env::var_os("RUVIEW_C4A_EXPORT_PROFILE") {
        use std::os::unix::fs::PermissionsExt;
        let destination = std::path::PathBuf::from(destination);
        assert!(destination.starts_with("/tmp/ruview-c4a-kind2-current-fixture-20261106"));
        assert!(!destination.exists());
        fn copy(from: &std::path::Path, to: &std::path::Path) {
            use std::os::unix::fs::PermissionsExt;
            assert!(!std::fs::symlink_metadata(from)
                .unwrap()
                .file_type()
                .is_symlink());
            if from.is_dir() {
                std::fs::create_dir_all(to).unwrap();
                std::fs::set_permissions(to, std::fs::Permissions::from_mode(0o700)).unwrap();
                for e in std::fs::read_dir(from).unwrap() {
                    let e = e.unwrap();
                    copy(&e.path(), &to.join(e.file_name()));
                }
            } else {
                std::fs::copy(from, to).unwrap();
                std::fs::set_permissions(to, std::fs::Permissions::from_mode(0o600)).unwrap();
            }
        }
        copy(directory.path(), &destination);
        let marker = destination.join("synthetic-c4a-fixture.json");
        std::fs::write(&marker,serde_json::to_vec(&serde_json::json!({"schema":"ruview.test-only.synthetic-c4a-kind2-fixture.v1","receiver_now_ns":now,"real_user_data":false})).unwrap()).unwrap();
        std::fs::set_permissions(marker, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    rt.cancel_temporal_durability(&rt.mutation_guard().unwrap())
        .unwrap();
}
#[test]
fn c4a_new_full_run_reuses_actual_native_train_writer_without_changing_model_input_identity() {
    let directory = tempfile::tempdir().unwrap();
    let rt = fixture::runtime(directory.path(), fixture::START);
    rt.configure_temporal_full_run(
        &rt.mutation_guard().unwrap(),
        fixture::context(),
        "synthetic-c4a-window".into(),
        fixture::START,
    )
    .unwrap();
    let mut now = fixture::settle_async_boundary(&rt, fixture::START);
    let original = fixture::snapshot(&rt, now)["descriptor"].clone();
    now = fixture::async_lead_end(&rt, now);
    let mut sequence = 0;
    let end = now + 3_000_000_000;
    while now <= end {
        sequence += 1;
        for source in 1..=3 {
            rt.ingest_native(&fixture::packet(source, sequence, now), now)
                .unwrap();
        }
        now += fixture::PERIOD;
        if fixture::snapshot(&rt, now)["durability_pending"] == true {
            std::thread::sleep(Duration::from_nanos(fixture::PERIOD));
        } else {
            std::thread::sleep(Duration::from_micros(500));
        }
    }
    let pending = fixture::snapshot(&rt, now)["durability_pending"] == true;
    if pending {
        now = fixture::settle_async_boundary(&rt, now);
    }
    assert!(
        fixture::snapshot(&rt, now)["durable_windows"]
            .as_u64()
            .unwrap()
            >= 1
    );
    let loaded = rt
        .temporal_store()
        .unwrap()
        .load_temporal(now)
        .unwrap()
        .unwrap();
    let prefix = loaded.prefix.unwrap();
    assert_eq!(prefix.metadata_kind(), "temporal-prefix-commitment-v2");
    assert_eq!(prefix.transition_limit(), 433);
    assert_eq!(
        fixture::snapshot(&rt, now)["descriptor"]["id"],
        original["id"]
    );
    assert_eq!(
        fixture::snapshot(&rt, now)["descriptor"]["expires_at_ms"],
        original["expires_at_ms"]
    );
    let data: serde_json::Value = serde_json::from_slice(prefix.bytes()).unwrap();
    let policy = &data["payload"]["policy"]["body"];
    assert_ne!(
        policy["input_policy_sha256"], data["payload"]["policy"]["sha256"],
        "continuation identity must not replace model input identity"
    );
    rt.cancel_temporal_durability(&rt.mutation_guard().unwrap())
        .unwrap();
}
#[test]
fn c4a_full_run_rejects_unsupported_source_profile_before_genesis_write() {
    let directory = tempfile::tempdir().unwrap();
    let rt = fixture::runtime(directory.path(), fixture::START);
    let mut context = fixture::context();
    context.sources.pop();
    assert!(rt
        .configure_temporal_full_run(
            &rt.mutation_guard().unwrap(),
            context,
            "synthetic-c4a-unsupported".into(),
            fixture::START
        )
        .is_err());
    let path = directory.path().join(fixture::context().owner_scope).join(
        "SensingServer/room-learner-recovery/temporal-prefix-commitment-v1/temporal-head.bin",
    );
    assert!(!path.exists());
}

fn configured(root: &Path) -> (Arc<RoomLearningRuntime>, u64) {
    let runtime = Arc::new(fixture::runtime(root, fixture::START));
    let store = runtime.recovery_store.clone().unwrap();
    runtime
        .configure_temporal_capture_with_custody(
            &runtime.mutation_guard().unwrap(),
            fixture::context(),
            "synthetic-c4a-native-fit".into(),
            fixture::START,
            move |schedule, run, mut config, now, _, witness| {
                // Bound when this capture is CREATED, never overridden by fit input.
                config.epochs = 2;
                TemporalTrainCapture::prepare_full_run_async(
                    schedule, run, config, now, store, witness,
                )
            },
        )
        .unwrap();
    let now = fixture::settle_async_boundary(&runtime, fixture::START);
    (runtime, now)
}
fn packet(source: u8, sequence: u32, at: u64, label: RoomLabel) -> CanonicalPacket {
    let gain = match label {
        RoomLabel::Empty => 3,
        RoomLabel::AdjacentOnly => 4,
        RoomLabel::OccupiedStatic => 8,
        RoomLabel::OccupiedMoving => 10 + (sequence % 2) as u8,
        _ => unreachable!(),
    };
    let iq = (0..64)
        .flat_map(|bin| [gain + source + (bin % 2) as u8, 4])
        .collect::<Vec<_>>();
    canonicalize_esp32(NativePacketInput {
        node_id: source,
        n_antennas: 1,
        raw_bins: 64,
        frequency_hz: 2_437_000_000,
        ppdu_type: PpduType::HtLegacy,
        sequence,
        received_at_ns: at,
        receiver_sample_rate_hz: 33.,
        iq_bytes: &iq,
        flags: Adr018Flags::default(),
        provenance: CaptureProvenance::LiveReceiverObserved,
    })
    .unwrap()
}
fn record_train_uncached(root: &Path) -> (Arc<RoomLearningRuntime>, u64) {
    let (runtime, mut now) = configured(root);
    let schedule = ManagedScheduleContract::build(
        fixture::context(),
        LeadInPolicy::Train5HeldOut15EngineeringV1,
    )
    .unwrap();
    let mut sequence = 0;
    let mut starts = 0;
    for episode in schedule
        .episodes()
        .iter()
        .filter(|e| e.role == DataRole::Train)
    {
        loop {
            let progress = fixture::snapshot(&runtime, now);
            match progress["stage"].as_str().unwrap() {
                "ready" | "paused" => {
                    now = fixture::async_lead_end(&runtime, now);
                    starts += 1;
                    runtime
                        .tick_temporal_capture(&runtime.mutation_guard().unwrap(), now)
                        .unwrap();
                }
                "needs_confirmation" => break,
                "failed" => panic!("synthetic C4A phase{} failed: {progress}", episode.index),
                _ => {}
            }
            let pending = fixture::snapshot(&runtime, now)["durability_pending"] == true;
            sequence += 1;
            for source in 1..=3 {
                match runtime.ingest_native(&packet(source, sequence, now, episode.condition), now)
                {
                    Ok(()) => {}
                    Err(error) if error == "temporal_storage_busy_pause" => break,
                    Err(error) => panic!("synthetic C4A ingest: {error}"),
                }
            }
            now += fixture::PERIOD;
            if pending {
                std::thread::sleep(Duration::from_nanos(fixture::PERIOD));
            }
            assert!(starts < 120, "synthetic C4A interruption bound exhausted");
        }
        let guard = runtime.mutation_guard().unwrap();
        let challenge = runtime.temporal_confirmation_challenge(&guard).unwrap();
        runtime
            .confirm_temporal_phase(&guard, &challenge, now)
            .unwrap();
        assert_eq!(fixture::snapshot(&runtime, now)["durability_pending"], true);
        now = fixture::settle_async_boundary(&runtime, now);
    }
    let complete = fixture::snapshot(&runtime, now);
    assert_eq!(complete["stage"], "complete");
    assert_eq!(
        complete["sealed_train_episodes"],
        schedule.counts().train_episodes
    );
    assert_eq!(complete["durable_windows"], schedule.counts().train_windows);
    println!(
        "SYNTHETIC_C4A_REAL_TRAIN_CAPTURE: rows={} seals={} starts={starts}",
        schedule.counts().train_windows,
        schedule.counts().train_episodes
    );
    (runtime, now)
}

fn copy_private(from: &Path, to: &Path) {
    use std::os::unix::fs::PermissionsExt;
    assert!(!std::fs::symlink_metadata(from)
        .unwrap()
        .file_type()
        .is_symlink());
    if from.is_dir() {
        std::fs::create_dir_all(to).unwrap();
        std::fs::set_permissions(to, std::fs::Permissions::from_mode(0o700)).unwrap();
        for e in std::fs::read_dir(from).unwrap() {
            let e = e.unwrap();
            copy_private(&e.path(), &to.join(e.file_name()));
        }
    } else {
        std::fs::copy(from, to).unwrap();
        std::fs::set_permissions(to, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
}
// Cache only the actual seven-phase native producer output. Each independent
// fault case gets its own files and strictly current recertification/owner.
fn record_train(root: &Path) -> (Arc<RoomLearningRuntime>, u64) {
    static TRAIN: std::sync::OnceLock<(tempfile::TempDir, u64)> = std::sync::OnceLock::new();
    let (saved, at) = TRAIN.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        let (rt, at) = record_train_uncached(d.path());
        drop(rt);
        (d, at)
    });
    copy_private(saved.path(), root);
    let now = at + 1_000_000_000;
    let rt = Arc::new(fixture::runtime(root, now));
    rt.restore_temporal_full_run(&rt.mutation_guard().unwrap(), &fixture::context(), now)
        .unwrap();
    assert_eq!(fixture::snapshot(&rt, now)["stage"], "complete");
    (rt, now)
}
fn assert_original_lifetime(value: &serde_json::Value, original: &serde_json::Value) {
    for field in [
        "schema",
        "id",
        "created_at_ms",
        "expires_at_ms",
        "owner_scope",
        "installation_id",
        "context_digest",
    ] {
        assert_eq!(value[field], original[field], "{field}");
    }
}
struct ReleaseGate(Option<std::sync::mpsc::SyncSender<()>>);
impl ReleaseGate {
    fn release(&mut self) {
        if let Some(tx) = self.0.take() {
            let _ = tx.send(());
        }
    }
}
impl Drop for ReleaseGate {
    fn drop(&mut self) {
        self.release();
    }
}
fn floors(rt: &RoomLearningRuntime) -> (Vec<(u8, u64, u64)>, Option<u64>) {
    let s = rt.inner.lock().unwrap();
    let Some(CaptureWorkflow::Temporal(c)) = &s.capture else {
        panic!()
    };
    c.core.as_ref().unwrap().test_full_run_floors()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c4a_actual_seven_train_fit_frozen_ack_and_exact_cold_model() {
    let directory = tempfile::tempdir().unwrap();
    let (runtime, now) = record_train(directory.path());
    let original = fixture::snapshot(&runtime, now)["descriptor"].clone();
    let train_rows_sha = fixture::snapshot(&runtime, now)["rows_sha256"].clone();
    let loaded = runtime
        .temporal_store()
        .unwrap()
        .load_temporal(now)
        .unwrap()
        .unwrap();
    assert_eq!(
        loaded.token.data().revision,
        78,
        "actual genesis +7 segments +64 windows +7 seals"
    );
    let (compute_tx, compute_rx) = std::sync::mpsc::sync_channel(1);
    let (compute_release_tx, compute_release_rx) = std::sync::mpsc::sync_channel(1);
    let mut compute_gate = ReleaseGate(Some(compute_release_tx));
    let compute_release_rx = std::sync::Mutex::new(compute_release_rx);
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
    let (done_tx, done_rx) = std::sync::mpsc::sync_channel(1);
    let mut done_gate = ReleaseGate(Some(done_tx));
    let done_rx = std::sync::Mutex::new(done_rx);
    *runtime.temporal_fit_hook.lock().unwrap() = Some(Arc::new(move |stage| {
        if stage == "before_validate" {
            compute_tx.send(()).unwrap();
            compute_release_rx
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(30))
                .unwrap();
        }
        if stage == "accepted_ready_published" {
            ready_tx.send(()).unwrap();
            done_rx
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(30))
                .unwrap();
        }
    }));
    let fit_started = Instant::now();
    runtime
        .start_temporal_fit(&runtime.mutation_guard().unwrap(), now)
        .unwrap();
    compute_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(runtime.inner.lock().unwrap().job.phase, JobPhase::Running);
    let during_fit = now + fit_started.elapsed().as_nanos() as u64;
    for source in 1..=3 {
        runtime
            .ingest_native(
                &packet(source, 500_000, during_fit, RoomLabel::Empty),
                during_fit,
            )
            .unwrap();
    }
    assert_eq!(
        fixture::snapshot(&runtime, during_fit)["rows_sha256"],
        train_rows_sha
    );
    compute_gate.release();
    loop {
        let phase = runtime.inner.lock().unwrap().job.phase;
        if phase == JobPhase::Ready {
            break;
        }
        assert_eq!(
            phase,
            JobPhase::Running,
            "{:?}",
            runtime.inner.lock().unwrap().job.error
        );
        assert!(fit_started.elapsed() < Duration::from_secs(120));
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    ready_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let (model, receipt, _) = runtime.temporal_diagnostic_fit().unwrap().unwrap();
    assert_eq!(
        runtime
            .cancel_temporal_fit(&runtime.mutation_guard().unwrap())
            .unwrap(),
        JobPhase::Ready,
        "late compute cancellation cannot revoke actual Accepted Ready"
    );
    assert_eq!(
        runtime
            .inner
            .lock()
            .unwrap()
            .job
            .report
            .as_ref()
            .unwrap()
            .completed_epochs,
        2
    );
    assert!(runtime.current_verified_frozen_model().unwrap().is_none());
    let before_notify = now + fit_started.elapsed().as_nanos() as u64;
    assert_eq!(
        runtime
            .prepare_frozen_transition(&runtime.mutation_guard().unwrap(), before_notify)
            .unwrap_err(),
        "temporal_actual_current_fit_required"
    );
    done_gate.release();
    // This real native ingress updates the live receiver floor without adding
    // Train rows or modifying the exact accepted Train commitment.
    let prime_at = now + fit_started.elapsed().as_nanos() as u64;
    for source in 1..=3 {
        runtime
            .ingest_native(
                &packet(source, 1_000_000, prime_at, RoomLabel::Empty),
                prime_at,
            )
            .unwrap();
    }
    let prepare_floors = floors(&runtime).0;
    let orphan = tempfile::tempdir().unwrap();
    copy_private(directory.path(), orphan.path());
    let writer = {
        let state = runtime.inner.lock().unwrap();
        let Some(CaptureWorkflow::Temporal(c)) = &state.capture else {
            panic!()
        };
        c.core.as_ref().unwrap().writer().unwrap()
    };
    let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(1);
    let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
    let mut writer_gate = ReleaseGate(Some(release_tx));
    writer.hold_next_after_prepare(entered_tx, release_rx);
    // Completion publication and drain signal are separate. Only the latter
    // permits transfer of the actual non-serde accepted-fit capability.
    let mut freeze_now;
    loop {
        freeze_now = now + u64::try_from(fit_started.elapsed().as_nanos()).unwrap();
        match runtime.prepare_frozen_transition(&runtime.mutation_guard().unwrap(), freeze_now) {
            Ok(()) => break,
            Err(e) if e == "temporal_actual_current_fit_required" => {
                tokio::time::sleep(Duration::from_millis(1)).await
            }
            Err(e) => panic!("actual Frozen preparation failed: {e}"),
        }
    }
    entered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(
        runtime
            .start_training(&runtime.mutation_guard().unwrap(), FitConfig::default())
            .unwrap_err()
            .contains("unsupported_command"),
        "a rejected command must not replace the accepted Frozen operation"
    );
    assert_eq!(
        fixture::snapshot(&runtime, freeze_now)["stage"],
        "preparing"
    );
    assert!(runtime.current_verified_frozen_model().unwrap().is_none());
    let advanced = freeze_now + fixture::PERIOD;
    for source in 1..=3 {
        runtime
            .ingest_native(
                &packet(source, 2_000_000, advanced, RoomLabel::Empty),
                advanced,
            )
            .unwrap();
    }
    let live_floors = floors(&runtime).0;
    assert!(live_floors
        .iter()
        .all(|(_, seq, t)| *seq == 2_000_000 && *t == advanced));
    writer_gate.release();
    let frozen_now = fixture::settle_async_boundary(&runtime, advanced);
    assert_eq!(fixture::snapshot(&runtime, frozen_now)["stage"], "frozen");
    assert_eq!(
        runtime
            .current_verified_frozen_model()
            .unwrap()
            .unwrap()
            .export()
            .unwrap(),
        model.export().unwrap()
    );
    let loaded = runtime
        .temporal_store()
        .unwrap()
        .load_temporal(frozen_now)
        .unwrap()
        .unwrap();
    assert_eq!(
        loaded.token.data().revision,
        79,
        "one actual durable Frozen transition"
    );
    let data: serde_json::Value = serde_json::from_slice(loaded.prefix.unwrap().bytes()).unwrap();
    assert_eq!(data["payload"]["stage"]["kind"], "frozen");
    assert_eq!(
        data["payload"]["stage"]["fit"]["body"]["artifact_sha256"],
        receipt.artifact_sha256
    );
    assert_original_lifetime(
        &fixture::snapshot(&runtime, frozen_now)["descriptor"],
        &original,
    );
    assert_eq!(
        data["payload"]["stage"]["fit"]["body"]["freeze_prepare_receiver_ns"],
        freeze_now
    );
    assert_eq!(
        fixture::snapshot(&runtime, frozen_now)["descriptor"]["saved_at_ms"],
        freeze_now / 1_000_000
    );
    assert_eq!(
        data["payload"]["stage"]["fit"]["body"]["source_floors"],
        serde_json::to_value(&prepare_floors).unwrap()
    );
    assert_eq!(floors(&runtime), (live_floors, Some(frozen_now)));
    let loaded = runtime
        .temporal_store()
        .unwrap()
        .load_temporal(frozen_now)
        .unwrap()
        .unwrap();
    assert!(runtime
        .temporal_store()
        .unwrap()
        .prepare_temporal(
            Some(&loaded.token),
            loaded.prefix.as_ref().unwrap(),
            frozen_now
        )
        .err()
        .unwrap()
        .contains("producer_write_grant_required"));

    assert!(runtime.inner.lock().unwrap().model.is_none());
    assert!(runtime.inner.lock().unwrap().qualification.is_none());
    let post_ack = frozen_now + fixture::PERIOD;
    for source in 1..=3 {
        runtime
            .ingest_native(
                &packet(source, 3_000_000, post_ack, RoomLabel::Empty),
                post_ack,
            )
            .unwrap();
    }
    assert!(floors(&runtime)
        .0
        .iter()
        .all(|(_, seq, t)| *seq == 3_000_000 && *t == post_ack));
    assert_eq!(floors(&runtime).1, Some(frozen_now));
    assert_eq!(
        fixture::snapshot(&runtime, post_ack)["rows_sha256"],
        train_rows_sha
    );
    assert_eq!(fixture::snapshot(&runtime, post_ack)["retained_rows"], 64);
    assert_eq!(
        fixture::snapshot(&runtime, post_ack)["sealed_train_episodes"],
        7
    );
    assert_eq!(
        runtime
            .current_verified_frozen_model()
            .unwrap()
            .unwrap()
            .export()
            .unwrap(),
        model.export().unwrap()
    );
    assert_eq!(
        runtime
            .temporal_store()
            .unwrap()
            .load_temporal(post_ack)
            .unwrap()
            .unwrap()
            .token
            .data()
            .revision,
        79
    );
    drop(runtime);
    let cold_now = post_ack + 1_000_000_000;
    let cold = fixture::runtime(directory.path(), cold_now);
    cold.restore_temporal_full_run(
        &cold.mutation_guard().unwrap(),
        &fixture::context(),
        cold_now,
    )
    .unwrap();
    assert_eq!(fixture::snapshot(&cold, cold_now)["stage"], "frozen");
    assert_eq!(
        floors(&cold),
        (prepare_floors, Some(cold_now)),
        "cold recertification retains only fsynced floors and a fresh local floor"
    );
    assert_eq!(
        cold.current_verified_frozen_model()
            .unwrap()
            .unwrap()
            .export()
            .unwrap(),
        model.export().unwrap()
    );
    assert_original_lifetime(&fixture::snapshot(&cold, cold_now)["descriptor"], &original);
    assert!(
        cold.start_temporal_phase(&cold.mutation_guard().unwrap(), cold_now)
            .is_err(),
        "private Frozen must not activate HeldOut"
    );
    cold.cancel_temporal_durability(&cold.mutation_guard().unwrap())
        .unwrap();
    let orphan_rt = fixture::runtime(orphan.path(), cold_now);
    orphan_rt
        .restore_temporal_full_run(
            &orphan_rt.mutation_guard().unwrap(),
            &fixture::context(),
            cold_now,
        )
        .unwrap();
    assert_eq!(fixture::snapshot(&orphan_rt, cold_now)["stage"], "complete");
    assert!(
        orphan_rt.current_verified_frozen_model().unwrap().is_none(),
        "saved model before Frozen ACK is only a diagnostic orphan"
    );
    assert!(orphan_rt
        .store
        .load_managed_temporal_candidate(&fixture::context())
        .unwrap()
        .is_some());
    orphan_rt
        .cancel_temporal_durability(&orphan_rt.mutation_guard().unwrap())
        .unwrap();
}

async fn fit_ready(rt: &Arc<RoomLearningRuntime>, now: u64) -> u64 {
    let start = Instant::now();
    rt.start_temporal_fit(&rt.mutation_guard().unwrap(), now)
        .unwrap();
    loop {
        let phase = rt.inner.lock().unwrap().job.phase;
        if phase == JobPhase::Ready {
            break;
        }
        assert_eq!(
            phase,
            JobPhase::Running,
            "{:?}",
            rt.inner.lock().unwrap().job.error
        );
        assert!(start.elapsed() < Duration::from_secs(120));
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    now + start.elapsed().as_nanos() as u64
}
async fn prepare_frozen(rt: &Arc<RoomLearningRuntime>, at: u64) -> u64 {
    let start = Instant::now();
    loop {
        let now = at + start.elapsed().as_nanos() as u64;
        match rt.prepare_frozen_transition(&rt.mutation_guard().unwrap(), now) {
            Ok(()) => return now,
            Err(e) if e == "temporal_actual_current_fit_required" => {
                assert!(start.elapsed() < Duration::from_secs(10));
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            Err(e) => panic!("Frozen prepare {e}"),
        }
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c4a_actual_frozen_cold_missing_corrupt_wrong_kind_bytes_and_expiry_fail_closed() {
    let directory = tempfile::tempdir().unwrap();
    let (rt, now) = record_train(directory.path());
    let at = fit_ready(&rt, now).await;
    let (_, receipt, _) = rt.temporal_diagnostic_fit().unwrap().unwrap();
    let at = prepare_frozen(&rt, at).await;
    let at = fixture::settle_async_boundary(&rt, at);
    assert_eq!(fixture::snapshot(&rt, at)["stage"], "frozen");
    let descriptor = fixture::snapshot(&rt, at)["descriptor"].clone();
    drop(rt);
    for corruption in ["missing", "corrupt", "wrong_kind_bytes", "head", "expired"] {
        let copy = tempfile::tempdir().unwrap();
        copy_private(directory.path(), copy.path());
        let path = copy
            .path()
            .join(fixture::context().owner_scope)
            .join("SensingServer/room-learner/temporal-models")
            .join(fixture::context().digest())
            .join(format!("{}.json", receipt.artifact_sha256));
        match corruption{
            "missing"=>std::fs::remove_file(&path).unwrap(),
            "corrupt"=>std::fs::write(&path,b"corrupt synthetic exact model").unwrap(),
            "wrong_kind_bytes"=>std::fs::write(&path,b"{\"schema\":\"ruview.room-learner.temporal-input.v2\"}").unwrap(),
            "head"=>std::fs::write(copy.path().join(fixture::context().owner_scope).join("SensingServer/room-learner-recovery/temporal-prefix-commitment-v1/temporal-head.bin"),b"corrupt synthetic current head").unwrap(),
            _=>{},
        }
        let now = if corruption == "expired" {
            descriptor["expires_at_ms"].as_u64().unwrap() * 1_000_000
        } else {
            at + 1_000_000_000
        };
        let cold = fixture::runtime(copy.path(), now);
        assert!(
            cold.restore_temporal_full_run(
                &cold.mutation_guard().unwrap(),
                &fixture::context(),
                now
            )
            .is_err(),
            "{corruption}"
        );
        assert!(
            cold.current_verified_frozen_model().is_err(),
            "{corruption}"
        );
        assert!(cold.inner.lock().unwrap().model.is_none());
        assert!(!cold.inner.lock().unwrap().selection.has_history());
        // Failed recovery deliberately leaves a blocked placeholder with no
        // core/writer; dropping it must not invent a cancellation target.
        assert!(cold
            .configure_temporal_capture(
                &cold.mutation_guard().unwrap(),
                fixture::context(),
                "forbidden-fallback".into(),
                now
            )
            .is_err());
        drop(cold);
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c4a_owner_cancel_drains_actual_prepared_frozen_writer_before_synthetic_erase() {
    let directory = tempfile::tempdir().unwrap();
    let (rt, now) = record_train(directory.path());
    let at = fit_ready(&rt, now).await;
    let writer = {
        let s = rt.inner.lock().unwrap();
        let Some(CaptureWorkflow::Temporal(c)) = &s.capture else {
            panic!()
        };
        c.core.as_ref().unwrap().writer().unwrap()
    };
    let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(1);
    let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
    let mut gate = ReleaseGate(Some(release_tx));
    writer.hold_next_after_prepare(entered_tx, release_rx);
    let at = prepare_frozen(&rt, at).await;
    entered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(rt.current_verified_frozen_model().unwrap().is_none());
    let (worker_tx, worker_rx) = std::sync::mpsc::sync_channel(1);
    let worker = rt.clone();
    let guard = rt.mutation_guard().unwrap();
    let cancel = std::thread::spawn(move || {
        worker_tx
            .send(worker.cancel_temporal_durability(&guard))
            .unwrap()
    });
    assert!(
        worker_rx.recv_timeout(Duration::from_millis(50)).is_err(),
        "owner cancellation acknowledged an active actual prepared Frozen write"
    );
    assert!(rt.inner.try_lock().is_ok());
    gate.release();
    worker_rx
        .recv_timeout(Duration::from_secs(10))
        .unwrap()
        .unwrap();
    cancel.join().unwrap();
    assert!(rt
        .current_verified_frozen_model()
        .map_or(true, |m| m.is_none()));
    let owner = directory.path().join(fixture::context().owner_scope);
    std::fs::remove_dir_all(&owner).unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!owner.exists());
    drop(rt);
    drop(writer);
    assert!(!owner.exists());
    println!("SYNTHETIC_C4A_OWNER_DRAIN_PREPARED_FROZEN_AT={at}");
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c4a_actual_frozen_lost_ack_same_intent_repair_installs_only_after_exact_ack() {
    let directory = tempfile::tempdir().unwrap();
    let (rt, now) = record_train(directory.path());
    let at = fit_ready(&rt, now).await;
    let writer = {
        let s = rt.inner.lock().unwrap();
        let Some(CaptureWorkflow::Temporal(c)) = &s.capture else {
            panic!()
        };
        c.core.as_ref().unwrap().writer().unwrap()
    };
    writer.lose_next_ack();
    let at = prepare_frozen(&rt, at).await;
    let started = Instant::now();
    let mut now;
    loop {
        now = at + started.elapsed().as_nanos() as u64;
        let _ = rt.tick_temporal_capture(&rt.mutation_guard().unwrap(), now);
        if fixture::snapshot(&rt, now)["stage"] == "blocked" {
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(30));
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(rt.current_verified_frozen_model().unwrap().is_none());
    let token = rt
        .temporal_store()
        .unwrap()
        .load_temporal(now)
        .unwrap()
        .unwrap()
        .token;
    assert_eq!(token.data().revision, 79);
    rt.repair_temporal_capture(&rt.mutation_guard().unwrap(), now)
        .unwrap();
    now = fixture::settle_async_boundary(&rt, now);
    assert_eq!(fixture::snapshot(&rt, now)["stage"], "frozen");
    assert!(rt.current_verified_frozen_model().unwrap().is_some());
    assert_eq!(
        rt.temporal_store()
            .unwrap()
            .load_temporal(now)
            .unwrap()
            .unwrap()
            .token,
        token,
        "SAME-intent repair must not allocate another transition"
    );
    rt.cancel_temporal_durability(&rt.mutation_guard().unwrap())
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c4a_running_compute_cancel_cannot_produce_accepted_fit_or_frozen() {
    let directory = tempfile::tempdir().unwrap();
    let (rt, now) = record_train(directory.path());
    let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(1);
    let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
    let mut gate = ReleaseGate(Some(release_tx));
    let release = std::sync::Mutex::new(release_rx);
    *rt.temporal_fit_hook.lock().unwrap() = Some(Arc::new(move |stage| {
        if stage == "before_validate" {
            entered_tx.send(()).unwrap();
            release
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(30))
                .unwrap();
        }
    }));
    rt.start_temporal_fit(&rt.mutation_guard().unwrap(), now)
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let (done_tx, done_rx) = std::sync::mpsc::sync_channel(1);
    let worker = rt.clone();
    let guard = rt.mutation_guard().unwrap();
    let cancel =
        std::thread::spawn(move || done_tx.send(worker.cancel_temporal_fit(&guard)).unwrap());
    assert!(done_rx.recv_timeout(Duration::from_millis(50)).is_err());
    assert!(rt.inner.try_lock().is_ok());
    gate.release();
    assert_eq!(
        done_rx
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .unwrap(),
        JobPhase::Cancelled
    );
    cancel.join().unwrap();
    assert!(rt.temporal_diagnostic_fit().unwrap().is_none());
    assert!(rt
        .prepare_frozen_transition(&rt.mutation_guard().unwrap(), now + 1_000_000_000)
        .is_err());
    assert!(rt.current_verified_frozen_model().unwrap().is_none());
    rt.cancel_temporal_durability(&rt.mutation_guard().unwrap())
        .unwrap();
}
