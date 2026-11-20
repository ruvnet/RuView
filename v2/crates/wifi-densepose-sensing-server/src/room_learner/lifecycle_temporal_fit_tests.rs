//! SYNTHETIC packets/labels through the actual native/seal producer and fitter.
//! No test permit issuer, physical RF, teacher or qualification evidence.
use super::super::tests as capture_fixture;
use super::*;
use crate::room_learner::{
    native::{canonicalize_esp32, NativePacketInput},
    observation::CaptureProvenance,
};
use std::sync::mpsc;
use std::time::Duration;
use wifi_densepose_hardware::{Adr018Flags, PpduType};

fn configured(root: &Path) -> (Arc<RoomLearningRuntime>, u64) {
    let runtime = Arc::new(capture_fixture::runtime(root, capture_fixture::START));
    let store = runtime.recovery_store.clone().unwrap();
    runtime
        .configure_temporal_capture_with(
            &runtime.mutation_guard().unwrap(),
            capture_fixture::context(),
            "synthetic-c3-native-fit".into(),
            capture_fixture::START,
            move |schedule, run, mut config, now, _| {
                // Bound when this capture is CREATED, never overridden by fit input.
                config.epochs = 2;
                TemporalTrainCapture::prepare_async(schedule, run, config, now, store)
            },
        )
        .unwrap();
    let now = capture_fixture::settle_async_boundary(&runtime, capture_fixture::START);
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
fn record_train(root: &Path) -> (Arc<RoomLearningRuntime>, u64) {
    assert!(!cfg!(debug_assertions), "complete native-persistence fixture requires Release; classify the caller");
    let (runtime, mut now) = configured(root);
    let schedule = ManagedScheduleContract::build(
        capture_fixture::context(),
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
            let progress = capture_fixture::snapshot(&runtime, now);
            match progress["stage"].as_str().unwrap() {
                "ready" | "paused" => {
                    now = capture_fixture::async_lead_end(&runtime, now);
                    starts += 1;
                    runtime
                        .tick_temporal_capture(&runtime.mutation_guard().unwrap(), now)
                        .unwrap();
                }
                "needs_confirmation" => break,
                "failed" => panic!("synthetic C3 phase{} failed: {progress}", episode.index),
                _ => {}
            }
            let pending = capture_fixture::snapshot(&runtime, now)["durability_pending"] == true;
            sequence += 1;
            for source in 1..=3 {
                match runtime.ingest_native(&packet(source, sequence, now, episode.condition), now)
                {
                    Ok(()) => {}
                    Err(error) if error == "temporal_storage_busy_pause" => break,
                    Err(error) => panic!("synthetic C3 ingest: {error}"),
                }
            }
            now += capture_fixture::PERIOD;
            if pending {
                std::thread::sleep(Duration::from_nanos(capture_fixture::PERIOD));
            }
            assert!(starts < 120, "synthetic C3 interruption bound exhausted");
        }
        let guard = runtime.mutation_guard().unwrap();
        let challenge = runtime.temporal_confirmation_challenge(&guard).unwrap();
        runtime
            .confirm_temporal_phase(&guard, &challenge, now)
            .unwrap();
        assert_eq!(
            capture_fixture::snapshot(&runtime, now)["durability_pending"],
            true
        );
        now = capture_fixture::settle_async_boundary(&runtime, now);
    }
    let complete = capture_fixture::snapshot(&runtime, now);
    assert_eq!(complete["stage"], "complete");
    assert_eq!(
        complete["sealed_train_episodes"],
        schedule.counts().train_episodes
    );
    assert_eq!(complete["durable_windows"], schedule.counts().train_windows);
    println!(
        "SYNTHETIC_C3_REAL_TRAIN_CAPTURE: rows={} seals={} starts={starts}",
        schedule.counts().train_windows,
        schedule.counts().train_episodes
    );
    (runtime, now)
}
// This process-local cache is created by the REAL seven-phase native/seal
// producer once, then strictly cold-restored for independent fault cases. It
// contains only disposable SYNTHETIC private test files, never user samples.
fn complete_train(root: &Path) -> (Arc<RoomLearningRuntime>, u64) {
    static RECORDED: std::sync::OnceLock<(tempfile::TempDir, u64)> = std::sync::OnceLock::new();
    let (recorded, at) = RECORDED.get_or_init(|| {
        let directory = tempfile::tempdir().unwrap();
        let (runtime, at) = record_train(directory.path());
        drop(runtime);
        (directory, at)
    });
    fn copy_private(from: &Path, to: &Path) {
        use std::os::unix::fs::PermissionsExt;
        assert!(!std::fs::symlink_metadata(from)
            .unwrap()
            .file_type()
            .is_symlink());
        if from.is_dir() {
            std::fs::create_dir_all(to).unwrap();
            std::fs::set_permissions(to, std::fs::Permissions::from_mode(0o700)).unwrap();
            for entry in std::fs::read_dir(from).unwrap() {
                let entry = entry.unwrap();
                copy_private(&entry.path(), &to.join(entry.file_name()));
            }
        } else {
            assert!(from.is_file());
            std::fs::copy(from, to).unwrap();
            std::fs::set_permissions(to, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
    }
    copy_private(recorded.path(), root);
    let now = at + 1_000_000_000;
    let runtime = Arc::new(capture_fixture::runtime(root, now));
    runtime
        .restore_temporal_capture(
            &runtime.mutation_guard().unwrap(),
            &capture_fixture::context(),
            now,
        )
        .unwrap();
    assert_eq!(
        capture_fixture::snapshot(&runtime, now)["stage"],
        "complete"
    );
    (runtime, now)
}
struct ReleaseGate(Option<mpsc::SyncSender<()>>);
impl ReleaseGate {
    fn release(&mut self) {
        if let Some(send) = self.0.take() {
            let _ = send.try_send(());
        }
    }
}
impl Drop for ReleaseGate {
    fn drop(&mut self) {
        self.release();
    }
}
fn hold(runtime: &RoomLearningRuntime, stage: &'static str) -> (mpsc::Receiver<()>, ReleaseGate) {
    hold_outcome(runtime, stage, false)
}
fn hold_outcome(
    runtime: &RoomLearningRuntime,
    stage: &'static str,
    unwind: bool,
) -> (mpsc::Receiver<()>, ReleaseGate) {
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let release = Mutex::new(release_rx);
    *runtime.temporal_fit_hook.lock().unwrap() = Some(Arc::new(move |at| {
        if at == stage {
            entered_tx.send(()).unwrap();
            release
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(30))
                .expect("synthetic fit gate not released");
            assert!(!unwind, "synthetic last-completion-owner unwind");
        }
    }));
    (entered_rx, ReleaseGate(Some(release_tx)))
}
async fn wait_fit(runtime: &RoomLearningRuntime) -> JobPhase {
    let start = Instant::now();
    loop {
        let phase = runtime.inner.lock().unwrap().job.phase;
        if !matches!(phase, JobPhase::Running | JobPhase::Cancelling) {
            return phase;
        }
        assert!(
            start.elapsed() < Duration::from_secs(120),
            "bounded synthetic fit did not finish"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn c3_incomplete_real_capture_cannot_issue_fit_or_weaken_public_legacy_guard() {
    let directory = tempfile::tempdir().unwrap();
    let (runtime, now) = configured(directory.path());
    assert!(runtime
        .start_temporal_fit(&runtime.mutation_guard().unwrap(), now)
        .unwrap_err()
        .contains("confirmed_durable_train"));
    assert!(runtime
        .start_training(&runtime.mutation_guard().unwrap(), FitConfig::default())
        .unwrap_err()
        .contains("unsupported_command"));
    assert_eq!(runtime.inner.lock().unwrap().job.phase, JobPhase::Idle);
    assert!(runtime.temporal_diagnostic_fit().unwrap().is_none());
    runtime
        .cancel_temporal_durability(&runtime.mutation_guard().unwrap())
        .unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[cfg_attr(debug_assertions, ignore = "native-persistence journey requires Release; CI explicitly executes and counts this test")]
async fn c3_actual_native_train_fit_and_same_store_cold_report_keep_whole_model_diagnostic() {
    let directory = tempfile::tempdir().unwrap();
    let (runtime, now) = complete_train(directory.path());
    let before = capture_fixture::snapshot(&runtime, now);
    let (entered, mut release) = hold(&runtime, "before_validate");
    runtime
        .start_temporal_fit(&runtime.mutation_guard().unwrap(), now)
        .unwrap();
    entered.recv_timeout(Duration::from_secs(10)).unwrap();
    // A denied unsupported command consumes a CAS revision, but does not
    // replace the recorded fit operation or strand it in Running.
    assert!(runtime
        .start_training(&runtime.mutation_guard().unwrap(), FitConfig::default())
        .unwrap_err()
        .contains("unsupported_command"));
    assert_eq!(runtime.inner.lock().unwrap().job.phase, JobPhase::Running);
    release.release();
    assert_eq!(
        wait_fit(&runtime).await,
        JobPhase::Ready,
        "{:?}",
        runtime.inner.lock().unwrap().job.error
    );
    let (model, receipt, diagnostics) = runtime.temporal_diagnostic_fit().unwrap().unwrap();
    assert_eq!(
        receipt.artifact_kind,
        "ruview.room-learner.managed-temporal-input.v3"
    );
    assert!(receipt.diagnostic_only);
    assert!(!receipt.qualification_granted);
    assert!(!receipt.physically_validated);
    let report = runtime.inner.lock().unwrap().job.report.clone().unwrap();
    assert_eq!(report.training_cohorts, 64);
    assert_eq!(report.completed_epochs, 2);
    assert!(report.backbone_delta_l2 > 0.);
    let summary = diagnostics.summary.unwrap();
    assert_eq!(summary.metrics.training_cohorts, report.training_cohorts);
    assert_eq!(
        summary.metrics.objective_evaluations,
        report.objective_evaluations
    );
    assert_eq!(
        summary.metrics.parameter_delta_l2,
        report.parameter_delta_l2
    );
    let cold = runtime
        .store
        .load_exact_temporal_candidate(&capture_fixture::context(), &receipt.artifact_sha256)
        .unwrap();
    assert_eq!(cold.export().unwrap(), model.export().unwrap());
    let cold_report = runtime.store.temporal_fit_diagnostics(&cold);
    assert_eq!(
        serde_json::to_value(cold_report.summary).unwrap(),
        serde_json::to_value(Some(summary)).unwrap()
    );
    let state = runtime.inner.lock().unwrap();
    assert!(state.model.is_none());
    assert!(state.receipt.is_none());
    assert!(!state.selection.has_history());
    assert!(state.qualification.is_none());
    drop(state);
    assert_eq!(
        capture_fixture::snapshot(&runtime, now)["rows_sha256"],
        before["rows_sha256"]
    );
    assert!(runtime.qualify(&runtime.mutation_guard().unwrap()).is_err());
    runtime
        .cancel_temporal_durability(&runtime.mutation_guard().unwrap())
        .unwrap();
    assert!(
        runtime.temporal_diagnostic_fit().is_err(),
        "cancelled owner retained a current diagnostic content read"
    );
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[cfg_attr(debug_assertions, ignore = "native-persistence journey requires Release; CI explicitly executes and counts this test")]
async fn c3_cancel_before_validation_drains_same_job_without_candidate_or_legacy_changes() {
    let directory = tempfile::tempdir().unwrap();
    let (runtime, now) = complete_train(directory.path());
    let (entered, mut release) = hold(&runtime, "before_validate");
    runtime
        .start_temporal_fit(&runtime.mutation_guard().unwrap(), now)
        .unwrap();
    entered.recv_timeout(Duration::from_secs(10)).unwrap();
    let guard = runtime.mutation_guard().unwrap();
    let worker = runtime.clone();
    let (done_tx, done_rx) = mpsc::sync_channel(1);
    let cancel = std::thread::spawn(move || {
        done_tx.send(worker.cancel_temporal_fit(&guard)).unwrap();
    });
    assert!(
        matches!(
            done_rx.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ),
        "cancel falsely acknowledged an undrained worker"
    );
    // Cancellation waits outside the actual ingress lock.
    assert!(runtime.inner.try_lock().is_ok());
    release.release();
    assert_eq!(
        done_rx
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .unwrap(),
        JobPhase::Cancelled
    );
    cancel.join().unwrap();
    assert!(runtime.temporal_diagnostic_fit().unwrap().is_none());
    assert!(runtime
        .store
        .load_managed_temporal_candidate(&capture_fixture::context())
        .unwrap()
        .is_none());
    assert_eq!(
        capture_fixture::snapshot(&runtime, now)["stage"],
        "complete"
    );
    runtime
        .cancel_temporal_durability(&runtime.mutation_guard().unwrap())
        .unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[cfg_attr(debug_assertions, ignore = "native-persistence journey requires Release; CI explicitly executes and counts this test")]
async fn c3_exact_store_recertification_rejects_tampered_head_before_any_fit_save() {
    let directory = tempfile::tempdir().unwrap();
    let (runtime, now) = complete_train(directory.path());
    let (entered, mut release) = hold(&runtime, "before_validate");
    runtime
        .start_temporal_fit(&runtime.mutation_guard().unwrap(), now)
        .unwrap();
    entered.recv_timeout(Duration::from_secs(10)).unwrap();
    let head = directory
        .path()
        .join(capture_fixture::context().owner_scope)
        .join(
            "SensingServer/room-learner-recovery/temporal-prefix-commitment-v1/temporal-head.bin",
        );
    let original = std::fs::read(&head).unwrap();
    std::fs::write(&head, b"synthetic-tampered-head").unwrap();
    release.release();
    assert_eq!(wait_fit(&runtime).await, JobPhase::Failed);
    assert!(runtime.temporal_diagnostic_fit().unwrap().is_none());
    assert!(runtime
        .store
        .load_managed_temporal_candidate(&capture_fixture::context())
        .unwrap()
        .is_none());
    std::fs::write(&head, &original).unwrap();
    runtime
        .cancel_temporal_durability(&runtime.mutation_guard().unwrap())
        .unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[cfg_attr(debug_assertions, ignore = "native-persistence journey requires Release; CI explicitly executes and counts this test")]
async fn c3_real_model_save_is_drained_before_synthetic_owner_erase_ack_and_never_recreated() {
    let directory = tempfile::tempdir().unwrap();
    let (runtime, now) = complete_train(directory.path());
    let (entered, mut release) = hold(&runtime, "model_saved");
    runtime
        .start_temporal_fit(&runtime.mutation_guard().unwrap(), now)
        .unwrap();
    entered.recv_timeout(Duration::from_secs(90)).unwrap();
    // The hook is INSIDE the shared real store body: an actual whole model is
    // already saved, but optional report persistence has not executed yet.
    assert!(runtime
        .store
        .load_managed_temporal_candidate(&capture_fixture::context())
        .unwrap()
        .is_some());
    // Exercise the actual Runtime tick while genuine artifact persistence is
    // held, not just an unrelated synthetic mutex.
    runtime
        .tick_temporal_capture(&runtime.mutation_guard().unwrap(), now + 1_000_000)
        .unwrap();
    let worker = runtime.clone();
    let guard = runtime.mutation_guard().unwrap();
    let (done_tx, done_rx) = mpsc::sync_channel(1);
    let cancel = std::thread::spawn(move || {
        done_tx
            .send(worker.cancel_temporal_durability(&guard))
            .unwrap();
    });
    assert!(
        matches!(
            done_rx.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ),
        "owner erase ACK escaped a real in-progress store body"
    );
    assert!(runtime.inner.try_lock().is_ok());
    release.release();
    done_rx
        .recv_timeout(Duration::from_secs(15))
        .unwrap()
        .unwrap();
    cancel.join().unwrap();
    assert_eq!(runtime.inner.lock().unwrap().job.phase, JobPhase::Cancelled);
    assert!(runtime.temporal_diagnostic_fit().unwrap().is_none());
    // Disposable, explicitly synthetic profile only. Never ask a store reader
    // to recreate the erased owner namespace during this post-ACK oracle.
    let owner = directory
        .path()
        .join(capture_fixture::context().owner_scope);
    std::fs::remove_dir_all(&owner).unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        !owner.exists(),
        "old fit recreated an erased profile after drain ACK"
    );
    drop(runtime);
    assert!(!owner.exists());
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[cfg_attr(debug_assertions, ignore = "native-persistence journey requires Release; CI explicitly executes and counts this test")]
async fn c3_last_external_owner_drop_drains_real_store_before_return_and_synthetic_erase() {
    let directory = tempfile::tempdir().unwrap();
    let (runtime, now) = complete_train(directory.path());
    let (entered, mut release) = hold(&runtime, "model_saved");
    runtime
        .start_temporal_fit(&runtime.mutation_guard().unwrap(), now)
        .unwrap();
    entered.recv_timeout(Duration::from_secs(90)).unwrap();
    assert_eq!(
        Arc::strong_count(&runtime),
        1,
        "worker retained the owner Runtime during storage"
    );
    let weak = Arc::downgrade(&runtime);
    let (done_tx, done_rx) = mpsc::sync_channel(1);
    let dropper = std::thread::spawn(move || {
        drop(runtime);
        done_tx.send(()).unwrap();
    });
    assert!(
        matches!(
            done_rx.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ),
        "last owner Drop failed to drain actual storage"
    );
    assert!(
        weak.upgrade().is_none(),
        "fit worker kept Runtime::Drop from starting"
    );
    release.release();
    done_rx.recv_timeout(Duration::from_secs(15)).unwrap();
    dropper.join().unwrap();
    let owner = directory
        .path()
        .join(capture_fixture::context().owner_scope);
    std::fs::remove_dir_all(&owner).unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!owner.exists());
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[cfg_attr(debug_assertions, ignore = "native-persistence journey requires Release; CI explicitly executes and counts this test")]
async fn c3_last_completion_owner_releases_after_signal_on_normal_and_unwind_paths() {
    for unwind in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let (runtime, now) = complete_train(directory.path());
        let (entered, mut release) = hold_outcome(&runtime, "completion_owner_upgraded", unwind);
        runtime
            .start_temporal_fit(&runtime.mutation_guard().unwrap(), now)
            .unwrap();
        entered.recv_timeout(Duration::from_secs(90)).unwrap();
        let lifetime = {
            let state = runtime.inner.lock().unwrap();
            let Some(CaptureWorkflow::Temporal(capture)) = state.capture.as_ref() else {
                panic!()
            };
            capture.fit_job.as_ref().unwrap().lifetime.clone()
        };
        assert_eq!(
            Arc::strong_count(&runtime),
            2,
            "only post-I/O completion may transiently own Runtime"
        );
        let weak = Arc::downgrade(&runtime);
        drop(runtime);
        release.release();
        let started = Instant::now();
        while !lifetime.done() {
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "last completion Arc self-drained before signaling"
            );
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        while weak.strong_count() != 0 {
            assert!(started.elapsed() < Duration::from_secs(10));
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        let owner = directory
            .path()
            .join(capture_fixture::context().owner_scope);
        std::fs::remove_dir_all(&owner).unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!owner.exists());
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[cfg_attr(debug_assertions, ignore = "native-persistence journey requires Release; CI explicitly executes and counts this test")]
async fn c3_original_expiry_during_fitting_withdraws_publication_without_renewal() {
    let directory = tempfile::tempdir().unwrap();
    let (runtime, now) = complete_train(directory.path());
    let original = capture_fixture::snapshot(&runtime, now)["descriptor"].clone();
    let (entered, mut release) = hold(&runtime, "before_save");
    runtime
        .start_temporal_fit(&runtime.mutation_guard().unwrap(), now)
        .unwrap();
    entered.recv_timeout(Duration::from_secs(90)).unwrap();
    let expiry = original["expires_at_ms"].as_u64().unwrap() * 1_000_000;
    assert!(runtime
        .tick_temporal_capture(&runtime.mutation_guard().unwrap(), expiry)
        .is_err());
    release.release();
    assert_eq!(wait_fit(&runtime).await, JobPhase::Cancelled);
    assert!(runtime.temporal_diagnostic_fit().unwrap().is_none());
    assert!(runtime
        .store
        .load_managed_temporal_candidate(&capture_fixture::context())
        .unwrap()
        .is_none());
    assert_eq!(
        capture_fixture::snapshot(&runtime, expiry)["descriptor"],
        original
    );
    runtime
        .cancel_temporal_durability(&runtime.mutation_guard().unwrap())
        .unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[cfg_attr(debug_assertions, ignore = "native-persistence journey requires Release; CI explicitly executes and counts this test")]
async fn c3_stale_generation_after_real_save_cannot_adopt_or_strand_job() {
    for change in ["generation", "room", "revision", "owner"] {
        let directory = tempfile::tempdir().unwrap();
        let (runtime, now) = complete_train(directory.path());
        let (entered, mut release) = hold(&runtime, "before_admission");
        runtime
            .start_temporal_fit(&runtime.mutation_guard().unwrap(), now)
            .unwrap();
        entered.recv_timeout(Duration::from_secs(90)).unwrap();
        {
            let mut state = runtime.inner.lock().unwrap();
            if change == "generation" {
                state.generation += 1;
            } else {
                let Some(CaptureWorkflow::Temporal(capture)) = state.capture.as_mut() else {
                    panic!()
                };
                match change {
                    "room" => capture.context.room_digest = "changed-room".into(),
                    "revision" => capture.context.revision += 1,
                    "owner" => capture.context.owner_scope = "c".repeat(64),
                    _ => unreachable!(),
                }
            }
        }
        release.release();
        assert_eq!(wait_fit(&runtime).await, JobPhase::Failed, "{change}");
        if change == "generation" {
            assert!(runtime.temporal_diagnostic_fit().is_err());
        } else {
            assert!(runtime.temporal_diagnostic_fit().unwrap().is_none());
        }
        assert!(runtime.inner.lock().unwrap().model.is_none());
        assert!(
            runtime
                .store
                .load_managed_temporal_candidate(&capture_fixture::context())
                .unwrap()
                .is_some(),
            "completed immutable history should not be deleted to fake cancellation"
        );
        runtime
            .cancel_temporal_durability(&runtime.mutation_guard().unwrap())
            .unwrap();
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[cfg_attr(debug_assertions, ignore = "native-persistence journey requires Release; CI explicitly executes and counts this test")]
async fn c3_old_completion_cannot_fail_a_replacement_job_with_different_cancel_identity() {
    let directory = tempfile::tempdir().unwrap();
    let (runtime, now) = complete_train(directory.path());
    let (entered, mut release) = hold(&runtime, "before_admission");
    runtime
        .start_temporal_fit(&runtime.mutation_guard().unwrap(), now)
        .unwrap();
    entered.recv_timeout(Duration::from_secs(90)).unwrap();
    let replacement = Arc::new(AtomicBool::new(false));
    let old_lifetime = {
        let mut state = runtime.inner.lock().unwrap();
        let Some(CaptureWorkflow::Temporal(capture)) = state.capture.as_ref() else {
            panic!()
        };
        let lifetime = capture.fit_job.as_ref().unwrap().lifetime.clone();
        state.job = Job {
            phase: JobPhase::Running,
            cancel: replacement.clone(),
            report: None,
            error: None,
        };
        lifetime
    };
    release.release();
    let started = Instant::now();
    while !old_lifetime.done() {
        assert!(started.elapsed() < Duration::from_secs(10));
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    {
        let mut state = runtime.inner.lock().unwrap();
        assert_eq!(state.job.phase, JobPhase::Running);
        assert!(Arc::ptr_eq(&state.job.cancel, &replacement));
        assert!(state.job.error.is_none());
        state.job.phase = JobPhase::Idle;
    }
    assert!(runtime.temporal_diagnostic_fit().unwrap().is_none());
    runtime
        .cancel_temporal_durability(&runtime.mutation_guard().unwrap())
        .unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[cfg_attr(debug_assertions, ignore = "native-persistence journey requires Release; CI explicitly executes and counts this test")]
async fn c3_optional_actual_summary_failure_keeps_durable_diagnostic_model() {
    let directory = tempfile::tempdir().unwrap();
    let (runtime, now) = complete_train(directory.path());
    let weak = Arc::downgrade(&runtime);
    let root = directory.path().to_path_buf();
    *runtime.temporal_fit_hook.lock().unwrap() = Some(Arc::new(move |stage| {
        if stage == "model_saved" {
            let runtime = weak.upgrade().unwrap();
            let model = runtime
                .store
                .load_managed_temporal_candidate(&capture_fixture::context())
                .unwrap()
                .unwrap();
            let binding = model.fit_diagnostic_binding().unwrap();
            let path = root
                .join(capture_fixture::context().owner_scope)
                .join("SensingServer/room-learner/temporal-models")
                .join(binding.context_digest)
                .join(format!("{}.fit-summary-v1.json", binding.artifact_sha256));
            std::fs::create_dir(path).unwrap();
        }
    }));
    runtime
        .start_temporal_fit(&runtime.mutation_guard().unwrap(), now)
        .unwrap();
    assert_eq!(wait_fit(&runtime).await, JobPhase::Ready);
    let (model, receipt, diagnostics) = runtime.temporal_diagnostic_fit().unwrap().unwrap();
    assert_eq!(
        serde_json::to_value(&diagnostics).unwrap()["state"],
        "not_recorded"
    );
    assert!(diagnostics.summary.is_none());
    assert_eq!(
        runtime
            .store
            .load_exact_temporal_candidate(&capture_fixture::context(), &receipt.artifact_sha256)
            .unwrap()
            .export()
            .unwrap(),
        model.export().unwrap()
    );
    runtime
        .cancel_temporal_durability(&runtime.mutation_guard().unwrap())
        .unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[cfg_attr(debug_assertions, ignore = "native-persistence journey requires Release; CI explicitly executes and counts this test")]
async fn c3_worker_unwind_keeps_saved_history_but_never_adopts_current_candidate() {
    let directory = tempfile::tempdir().unwrap();
    let (runtime, now) = complete_train(directory.path());
    *runtime.temporal_fit_hook.lock().unwrap() = Some(Arc::new(|stage| {
        if stage == "before_admission" {
            panic!("synthetic C3 worker failure")
        }
    }));
    runtime
        .start_temporal_fit(&runtime.mutation_guard().unwrap(), now)
        .unwrap();
    assert_eq!(wait_fit(&runtime).await, JobPhase::Failed);
    assert!(runtime.temporal_diagnostic_fit().unwrap().is_none());
    assert!(runtime
        .store
        .load_managed_temporal_candidate(&capture_fixture::context())
        .unwrap()
        .is_some());
    runtime
        .cancel_temporal_durability(&runtime.mutation_guard().unwrap())
        .unwrap();
}
