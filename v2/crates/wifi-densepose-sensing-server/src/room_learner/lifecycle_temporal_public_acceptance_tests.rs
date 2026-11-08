//! SYNTHETIC production-default-profile software acceptance. No physical RF
//! or clock precision/generalization/cadence acceptance follows from this case.
use super::super::{held_out_tests as established, tests as fixture};
use super::*;
use established::{
    assert_original_lifetime, copy_private, packet, phase_target, prefix_value, receiver_floor,
    same_train_and_frozen,
};
use std::{
    path::Path,
    time::{Duration, Instant},
};
fn capture(rt: &RoomLearningRuntime, now: u64) -> TemporalCaptureWire {
    let state = rt.inner.lock().unwrap();
    let Some(CaptureWorkflow::Temporal(c)) = &state.capture else {
        panic!("actualTemporalcapturemissing")
    };
    c.core
        .as_ref()
        .unwrap()
        .public_capture(&rt.runtime_epoch, c.lead_in_until, now)
}
fn public_lead_end(rt: &Arc<RoomLearningRuntime>, at: u64) -> u64 {
    let c = capture(rt, at);
    rt.temporal_begin_phase_v2(
        TemporalPhaseStartRequestV2 {
            schema: PHASE_START_REQUEST_SCHEMA.into(),
            expected: rt.mutation_guard().unwrap(),
            capture_generation_id: c.capture_generation_id,
            schedule_sha256: c.schedule_sha256,
            episode_index: c.current_episode.unwrap(),
            conditions_confirmed: true,
        },
        at,
    )
    .unwrap();
    let start = Instant::now();
    loop {
        let now = at + start.elapsed().as_nanos() as u64;
        rt.tick_temporal_capture(&rt.mutation_guard().unwrap(), now)
            .unwrap();
        let state = rt.inner.lock().unwrap();
        let Some(CaptureWorkflow::Temporal(c)) = &state.capture else {
            panic!()
        };
        if let Some(end) = c.lead_in_until {
            return end;
        }
        drop(state);
        assert!(start.elapsed() < Duration::from_secs(30));
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn public_finish(rt: &Arc<RoomLearningRuntime>, guard: MutationGuard, challenge: String, now: u64) {
    let c = capture(rt, now);
    rt.temporal_finish_phase_v2(
        TemporalPhaseFinishRequestV2 {
            schema: PHASE_FINISH_REQUEST_SCHEMA.into(),
            expected: guard,
            capture_generation_id: c.capture_generation_id,
            schedule_sha256: c.schedule_sha256,
            episode_index: c.current_episode.unwrap(),
            confirmation_challenge: challenge,
            conditions_confirmed: true,
        },
        now,
    )
    .unwrap();
}
async fn metadata_ready(rt: &RoomLearningRuntime) {
    let start = Instant::now();
    loop {
        let state = rt.inner.lock().unwrap();
        let Some(CaptureWorkflow::Temporal(c)) = &state.capture else {
            panic!()
        };
        if state.job.phase == JobPhase::Ready
            && c.training_proof.is_some()
            && c.evidence_job.as_ref().is_some_and(|j| j.done())
        {
            return;
        }
        assert!(
            !matches!(state.job.phase, JobPhase::Failed | JobPhase::Cancelled),
            "actualmetadatajobfailed {:?}",
            state.job.error
        );
        drop(state);
        assert!(
            start.elapsed() < Duration::from_secs(180),
            "actualmetadatajobwatchdog"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}
async fn restore_public(rt: &Arc<RoomLearningRuntime>, now: u64) {
    let status = rt.temporal_status_v2(now).unwrap();
    let id = status["recovery"]["capture_generation_id"]
        .as_str()
        .unwrap();
    rt.temporal_restore_v2(&rt.mutation_guard().unwrap(), &fixture::context(), id, now)
        .unwrap();
    if rt.current_verified_frozen_model().unwrap().is_some() {
        metadata_ready(rt).await;
    }
}
fn record_train_default48(root: &Path) -> (Arc<RoomLearningRuntime>, u64) {
    let runtime = Arc::new(fixture::runtime(root, fixture::START));
    runtime
        .temporal_configure_v2(
            &runtime.mutation_guard().unwrap(),
            fixture::context(),
            "synthetic-c4c-production-default48".into(),
            true,
            fixture::START,
        )
        .unwrap();
    let mut now = fixture::settle_async_boundary(&runtime, fixture::START);
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
            // A durable terminal pause can be confirmable without resuming.
            if runtime
                .temporal_confirmation_challenge(&runtime.mutation_guard().unwrap())
                .is_ok()
            {
                break;
            }
            let progress = fixture::snapshot(&runtime, now);
            match progress["stage"].as_str().unwrap() {
                "ready" | "paused" => {
                    now = public_lead_end(&runtime, now);
                    starts += 1;
                    runtime
                        .tick_temporal_capture(&runtime.mutation_guard().unwrap(), now)
                        .unwrap();
                }
                "needs_confirmation" => break,
                "failed" => panic!("synthetic C4B phase{} failed: {progress}", episode.index),
                _ => {}
            }
            let pending = fixture::snapshot(&runtime, now)["durability_pending"] == true;
            sequence += 1;
            for source in 1..=3 {
                match runtime.ingest_native(&packet(source, sequence, now, episode.condition), now)
                {
                    Ok(()) => {}
                    Err(error) if error == "temporal_storage_busy_pause" => break,
                    Err(error) => panic!("synthetic C4B ingest: {error}"),
                }
            }
            now += fixture::PERIOD;
            if pending {
                std::thread::sleep(Duration::from_nanos(fixture::PERIOD));
            }
            assert!(starts < 120, "synthetic C4B interruption bound exhausted");
        }
        let guard = runtime.mutation_guard().unwrap();
        let challenge = runtime.temporal_confirmation_challenge(&guard).unwrap();
        public_finish(&runtime, guard, challenge, now);
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
        "SYNTHETIC_C4B_REAL_TRAIN_CAPTURE: rows={} seals={} starts={starts}",
        schedule.counts().train_windows,
        schedule.counts().train_episodes
    );
    (runtime, now)
}

async fn produce_default48_complete(directory: &Path) -> u64 {
    let (rt, mut now) = record_train_default48(directory);
    // Fresh actual cold Runtime uses its authenticated producer floor as this
    // explicitly synthetic clock-domain anchor; no live clock clamp/rescue.
    drop(rt);
    now += 1_000_000_000;
    let mut rt = Arc::new(fixture::runtime(directory, now));
    restore_public(&rt, now).await;
    let generation = capture(&rt, rt.receiver_now_ns()).capture_generation_id;
    rt.temporal_train_v2(
        TemporalTrainRequestV2 {
            schema: TRAIN_REQUEST_SCHEMA.into(),
            expected: rt.mutation_guard().unwrap(),
            capture_generation_id: generation,
        },
        rt.receiver_now_ns(),
    )
    .unwrap();
    metadata_ready(&rt).await;
    now = rt.receiver_now_ns();
    let fit = rt.temporal_diagnostic_fit().unwrap().unwrap().2;
    assert_eq!(
        serde_json::to_value(&fit.summary.as_ref().unwrap().metrics.configured).unwrap(),
        serde_json::to_value(FitConfig::for_source_count(3)).unwrap()
    );
    assert_eq!(
        fit.summary.as_ref().unwrap().metrics.completed_epochs,
        48,
        "production default profile was not executed"
    );
    println!(
        "SYNTHETIC_C4C_ACTUAL_DEFAULT48_FIT={}",
        serde_json::to_string(&fit.summary.as_ref().unwrap().metrics).unwrap()
    );
    now = fixture::settle_async_boundary(&rt, now);
    assert_eq!(fixture::snapshot(&rt, now)["stage"], "frozen");
    drive_remaining_held_out(directory, rt, now).await
}
async fn drive_remaining_held_out(
    directory: &Path,
    mut rt: Arc<RoomLearningRuntime>,
    mut now: u64,
) -> u64 {
    let model = rt.current_verified_frozen_model().unwrap().unwrap();
    let model_bytes = model.export().unwrap();
    let model_sha = {
        use sha2::Digest;
        format!("{:x}", sha2::Sha256::digest(&model_bytes))
    };
    let existing = rt.store.temporal_fit_diagnostics(&model);
    assert_eq!(
        existing.summary.as_ref().unwrap().metrics.completed_epochs,
        48
    );
    assert_eq!(
        serde_json::to_value(&existing.summary.as_ref().unwrap().metrics.configured).unwrap(),
        serde_json::to_value(FitConfig::for_source_count(3)).unwrap()
    );
    let report_before = serde_json::to_value(rt.store.temporal_fit_diagnostics(&model)).unwrap();
    now = fixture::settle_async_boundary(&rt, now);
    let initial = capture(&rt, now);
    assert!(matches!(initial.stage.as_str(), "frozen" | "paused"));
    assert!((64..128).contains(&initial.durable_windows));
    let frozen = prefix_value(&rt, now);
    assert_eq!(
        frozen["payload"]["stage"]["fit"]["body"]["configured_fit_sha256"],
        crate::room_learner::qualification::hash(&FitConfig::for_source_count(3)).unwrap()
    );
    let original = frozen["payload"]["common"]["descriptor"].clone();
    let schedule = ManagedScheduleContract::build(
        fixture::context(),
        LeadInPolicy::Train5HeldOut15EngineeringV1,
    )
    .unwrap();
    let mut sequence = receiver_floor(&rt);
    let mut partial_cold = initial.durable_windows > 64;
    let mut terminal_cold = false;
    let mut starts = 0;
    let original_proof = {
        let state = rt.inner.lock().unwrap();
        let Some(CaptureWorkflow::Temporal(c)) = &state.capture else {
            panic!()
        };
        c.training_proof.as_ref().unwrap().sha256.clone()
    };
    let mut observing_refused = false;
    let mut pending_refused = false;
    let mut train_overlap_refused = false;
    if initial.durable_windows > 64 {
        let before_rows = initial.durable_windows;
        let forged = rt.temporal_begin_phase_v2(
            TemporalPhaseStartRequestV2 {
                schema: PHASE_START_REQUEST_SCHEMA.into(),
                expected: rt.mutation_guard().unwrap(),
                capture_generation_id: initial.capture_generation_id.clone(),
                schedule_sha256: initial.schedule_sha256.clone(),
                episode_index: 0,
                conditions_confirmed: true,
            },
            now,
        );
        assert_eq!(forged.unwrap_err(), "learner_phase_identity_changed");
        assert_eq!(capture(&rt, now).durable_windows, before_rows);
        train_overlap_refused = true;
        println!("SYNTHETIC_C4C_COLD_CONTINUATION_BASELINE rows={} episode={} train_seals=7 fit_epochs=48 same_model={} proof={}",initial.durable_windows,initial.current_episode.unwrap(),model_sha,original_proof);
    }

    for episode in schedule.episodes().iter().filter(|e| {
        e.role == DataRole::HeldOut && e.index >= initial.current_episode.unwrap_or(15) as usize
    }) {
        loop {
            let challenge = rt.temporal_confirmation_challenge(&rt.mutation_guard().unwrap());
            if challenge.is_ok() {
                if episode.index == 14 && !terminal_cold {
                    let before = prefix_value(&rt, now);
                    let rows = fixture::snapshot(&rt, now)["durable_windows"].clone();
                    drop(rt);
                    now += 1_000_000_000;
                    rt = Arc::new(fixture::runtime(directory, now));
                    restore_public(&rt, now).await;
                    now = rt.receiver_now_ns();
                    assert_eq!(fixture::snapshot(&rt, now)["durable_windows"], rows);
                    same_accepted_prefix(&frozen, &prefix_value(&rt, now));
                    same_train_and_frozen(&frozen, &prefix_value(&rt, now));
                    assert_eq!(
                        before["payload"]["accepted"],
                        prefix_value(&rt, now)["payload"]["accepted"]
                    );
                    terminal_cold = true;
                    continue;
                }
                break;
            }
            let progress = fixture::snapshot(&rt, now);
            assert_eq!(
                progress["label"],
                serde_json::to_value(episode.condition).unwrap()
            );
            assert_eq!(progress["role"], "HeldOut");
            assert_eq!(progress["target_ms"], 30_000);
            match progress["stage"].as_str().unwrap() {
                "frozen" | "ready" | "paused" => {
                    now = public_lead_end(&rt, now);
                    starts += 1;
                    if episode.index == 7 && starts == 1 {
                        now += 50_000_000;
                    }
                    rt.tick_temporal_capture(&rt.mutation_guard().unwrap(), now)
                        .unwrap();
                }
                "failed" => panic!("HeldOut failed: {progress}"),
                _ => {}
            }
            let pending = fixture::snapshot(&rt, now)["durability_pending"] == true;
            if !observing_refused
                && !pending
                && fixture::snapshot(&rt, now)["stage"] == "observing"
                && phase_target(&rt).is_some()
            {
                let state = rt.inner.lock().unwrap();
                let Some(CaptureWorkflow::Temporal(c)) = &state.capture else {
                    panic!()
                };
                assert!(
                    c.core
                        .as_ref()
                        .unwrap()
                        .prepare_evaluation_source(now, false)
                        .is_err(),
                    "observingcannotmintstableTrainprojection"
                );
                observing_refused = true;
            }
            sequence += 1;
            for source in 1..=3 {
                match rt.ingest_native(&packet(source, sequence, now, episode.condition), now) {
                    Ok(()) => {}
                    Err(e) if e == "temporal_storage_busy_pause" => break,
                    Err(e) => panic!("HeldOut ingress: {e}"),
                }
            }
            now += fixture::PERIOD;
            if pending {
                std::thread::sleep(Duration::from_nanos(fixture::PERIOD));
            }
            let progress = fixture::snapshot(&rt, now);
            if !partial_cold
                && episode.index == 7
                && progress["durable_windows"].as_u64().unwrap() >= 67
                && progress["durability_pending"] == false
                && phase_target(&rt)
                    .is_some_and(|(start, end)| now >= start + 3 * fixture::PERIOD && now < end)
            {
                let durable = progress["durable_windows"].clone();
                let before = prefix_value(&rt, now);
                drop(rt);
                now += 1_000_000_000;
                rt = Arc::new(fixture::runtime(directory, now));
                restore_public(&rt, now).await;
                now = rt.receiver_now_ns();
                {
                    let state = rt.inner.lock().unwrap();
                    let Some(CaptureWorkflow::Temporal(c)) = &state.capture else {
                        panic!()
                    };
                    assert_eq!(
                        c.training_proof.as_ref().unwrap().sha256,
                        original_proof,
                        "partialHeldOutRestoremustreconcileSAMETrainproof"
                    );
                }
                let current = capture(&rt, now);
                let before_rejected_train = fixture::snapshot(&rt, now)["durable_windows"].clone();
                let forged = rt.temporal_begin_phase_v2(
                    TemporalPhaseStartRequestV2 {
                        schema: PHASE_START_REQUEST_SCHEMA.into(),
                        expected: rt.mutation_guard().unwrap(),
                        capture_generation_id: current.capture_generation_id,
                        schedule_sha256: current.schedule_sha256,
                        episode_index: 0,
                        conditions_confirmed: true,
                    },
                    now,
                );
                assert_eq!(forged.unwrap_err(), "learner_phase_identity_changed");
                assert_eq!(
                    fixture::snapshot(&rt, now)["durable_windows"],
                    before_rejected_train
                );
                train_overlap_refused = true;
                assert_eq!(fixture::snapshot(&rt, now)["durable_windows"], durable);
                assert!(phase_target(&rt).is_none());
                assert_eq!(
                    before["payload"]["accepted"],
                    prefix_value(&rt, now)["payload"]["accepted"]
                );
                same_accepted_prefix(&frozen, &prefix_value(&rt, now));
                same_train_and_frozen(&frozen, &prefix_value(&rt, now));
                partial_cold = true;
            }
            assert!(
                starts < 300,
                "bounded synthetic HeldOut interruption budget exhausted"
            );
        }
        let guard = rt.mutation_guard().unwrap();
        let challenge = rt.temporal_confirmation_challenge(&guard).unwrap();
        public_finish(&rt, guard, challenge, now);
        {
            let state = rt.inner.lock().unwrap();
            let Some(CaptureWorkflow::Temporal(c)) = &state.capture else {
                panic!()
            };
            assert!(
                c.core
                    .as_ref()
                    .unwrap()
                    .public_capture(&rt.runtime_epoch, c.lead_in_until, now)
                    .durability_pending
            );
            assert!(
                c.core
                    .as_ref()
                    .unwrap()
                    .prepare_evaluation_source(now, false)
                    .is_err(),
                "pendingdurabilitycannotmintstableTrainprojection"
            );
            pending_refused = true;
        }
        now = fixture::settle_async_boundary(&rt, now);
        assert_eq!(
            fixture::snapshot(&rt, now)["sealed_held_out_episodes"],
            episode.index - 6
        );
        let sealed = prefix_value(&rt, now);
        same_accepted_prefix(&frozen, &sealed);
        same_train_and_frozen(&frozen, &sealed);
        println!(
            "SYNTHETIC_C4B_HELDOUT_SEAL episode={} durable={} sealed={} starts={starts}",
            episode.index,
            fixture::snapshot(&rt, now)["durable_windows"],
            fixture::snapshot(&rt, now)["sealed_held_out_episodes"]
        );
    }
    assert!(
        partial_cold && terminal_cold,
        "two actual cold interruptions must execute"
    );
    assert!(
        observing_refused && pending_refused && train_overlap_refused,
        "actualprotectedsource/phaseadmissioncontrolsnotexecuted"
    );
    println!("SYNTHETIC_C4C_ACTUAL_SOURCE_REFUSALS observing=true pending=true forged_train_episode=true same_stable_train_proof=true");

    let status = fixture::snapshot(&rt, now);
    assert_eq!(status["stage"], "full_complete");
    assert_eq!(status["durable_windows"], 128);
    assert_eq!(status["sealed_train_episodes"], 7);
    assert_eq!(status["sealed_held_out_episodes"], 8);
    let complete = prefix_value(&rt, now);
    same_accepted_prefix(&frozen, &complete);
    same_train_and_frozen(&frozen, &complete);
    assert_eq!(complete["payload"]["stage"]["kind"], "complete");
    assert_eq!(
        rt.current_verified_frozen_model()
            .unwrap()
            .unwrap()
            .export()
            .unwrap(),
        model_bytes
    );
    assert_eq!(
        serde_json::to_value(rt.store.temporal_fit_diagnostics(&model)).unwrap(),
        report_before
    );
    assert!(rt.inner.lock().unwrap().model.is_none());
    assert!(rt.inner.lock().unwrap().qualification.is_none());
    assert!(!rt.inner.lock().unwrap().selection.has_history());
    let loaded = rt
        .temporal_store()
        .unwrap()
        .load_temporal(now)
        .unwrap()
        .unwrap();
    let final_prefix = loaded.prefix.unwrap();
    let bytes = final_prefix.bytes().len();
    let raw_values = complete["payload"]["accepted"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            row["views"]
                .as_array()
                .unwrap()
                .iter()
                .map(|view| {
                    view["samples"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|sample| sample.as_array().unwrap().len())
                        .sum::<usize>()
                })
                .sum::<usize>()
        })
        .sum::<usize>();
    assert!(raw_values <= 2_000_000);
    fn slot_sizes(root: &Path, out: &mut Vec<u64>) {
        for entry in std::fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                slot_sizes(&path, out)
            } else if path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("slot-")
                && path.extension().is_some_and(|e| e == "bin")
            {
                out.push(std::fs::metadata(path).unwrap().len());
            }
        }
    }
    let mut outer_slot_sizes = vec![];
    slot_sizes(directory, &mut outer_slot_sizes);
    outer_slot_sizes.sort_unstable();
    println!(
        "SYNTHETIC_C4B_RAW_NUMERIC_VALUES={raw_values} AEAD_SLOT_FILE_BYTES={outer_slot_sizes:?}"
    );
    assert!(bytes <= crate::room_learner::recovery_store::MAX_RECOVERY_BYTES);
    println!(
        "SYNTHETIC_C4B_FULL128_ACTUAL_JSON_BYTES={bytes} STARTS={starts} GLOBAL_GAPS={}",
        complete["payload"]["common"]["ledger"]["gaps"]
            .as_array()
            .unwrap()
            .len()
    );
    drop(rt);
    now += 1_000_000_000;
    let cold = Arc::new(fixture::runtime(directory, now));
    restore_public(&cold, now).await;
    now = cold.receiver_now_ns();
    assert_eq!(fixture::snapshot(&cold, now)["stage"], "full_complete");
    assert_eq!(
        cold.current_verified_frozen_model()
            .unwrap()
            .unwrap()
            .export()
            .unwrap(),
        model_bytes
    );
    assert_original_lifetime(&fixture::snapshot(&cold, now)["descriptor"], &original);
    assert!(cold
        .start_temporal_phase(&cold.mutation_guard().unwrap(), now)
        .is_err());
    assert_eq!(
        cold.store
            .load_exact_temporal_candidate(&fixture::context(), &model_sha)
            .unwrap()
            .export()
            .unwrap(),
        model_bytes
    );
    cold.cancel_temporal_durability(&cold.mutation_guard().unwrap())
        .unwrap();
    now
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[cfg_attr(
    debug_assertions,
    ignore = "actual native default-profile workflow runs in mandatory Temporal Release batch"
)]
async fn c4c_default48_public_operator_workflow_native128_and_cold_selection() {
    let directory = tempfile::Builder::new()
        .prefix("SYNTHETIC-c4c-default48-")
        .tempdir()
        .unwrap()
        .keep();
    std::fs::write(directory.join("SYNTHETIC-OWNED-DEFAULT48-RUN.json"),br#"{"real_user_data":false,"physical_accuracy_claim":false,"profile":"production-default48","retention":"owned_test_evidence"}"#).unwrap();
    let configured = FitConfig::for_source_count(3);
    assert_eq!(configured.epochs, 48);
    println!(
        "SYNTHETIC_C4C_ACTUAL_CONFIG={}",
        serde_json::to_string(&configured).unwrap()
    );
    let now = produce_default48_complete(&directory).await + 1_000_000_000;
    finish_public_checks(&directory, now).await;
}
async fn finish_public_checks(directory: &Path, now: u64) {
    let rt = Arc::new(fixture::runtime(directory, now));
    restore_public(&rt, now).await;
    let status = rt.temporal_status_v2(rt.receiver_now_ns()).unwrap();
    let c: TemporalCaptureWire = serde_json::from_value(status["capture"].clone()).unwrap();
    let complete: TemporalCompleteRefsWire =
        serde_json::from_value(status["complete"].clone()).unwrap();
    assert!(status["evaluation"].is_null());
    rt.temporal_evaluate_v2(
        TemporalEvaluateRequestV2 {
            schema: EVALUATE_REQUEST_SCHEMA.into(),
            expected: rt.mutation_guard().unwrap(),
            capture_generation_id: c.capture_generation_id.clone(),
            refs: complete,
        },
        rt.receiver_now_ns(),
    )
    .unwrap();
    metadata_ready(&rt).await;
    let status = rt.temporal_status_v2(rt.receiver_now_ns()).unwrap();
    let evaluation: TemporalEvaluationWire =
        serde_json::from_value(status["evaluation"].clone()).unwrap();
    assert_eq!(evaluation.report.samples, 64);
    assert_eq!(evaluation.report.declared_trial_count, 8);
    assert!(evaluation
        .report
        .per_condition
        .iter()
        .all(|c| c.samples == 16 && c.declared_trial_count == 2));
    rt.temporal_qualify_v2(
        TemporalQualifyRequestV2 {
            schema: QUALIFY_REQUEST_SCHEMA.into(),
            expected: rt.mutation_guard().unwrap(),
            capture_generation_id: c.capture_generation_id.clone(),
            refs: evaluation.refs.clone(),
        },
        rt.receiver_now_ns(),
    )
    .unwrap();
    metadata_ready(&rt).await;
    let status = rt.temporal_status_v2(rt.receiver_now_ns()).unwrap();
    println!(
        "SYNTHETIC_C4C_DEFAULT48_ACTUAL_EVALUATION_AND_ASSESSMENT={}",
        serde_json::to_string(
            &json!({"evaluation":status["evaluation"],"assessment":status["assessment"]})
        )
        .unwrap()
    );
    let q = status["assessment"]["qualification_sha256"].as_str();
    if q.is_none() {
        assert_eq!(status["assessment"]["outcome"], "not_qualified");
        assert!(!rt.inner.lock().unwrap().selection.has_history());
        let before = rt.store.load_selection().unwrap();
        let denied = rt.temporal_select_v2(
            TemporalSelectRequestV2 {
                schema: SELECT_REQUEST_SCHEMA.into(),
                expected: rt.mutation_guard().unwrap(),
                capture_generation_id: c.capture_generation_id.clone(),
                refs: TemporalQualificationRefsWire {
                    evaluation: evaluation.refs.clone(),
                    qualification_sha256: "0".repeat(64),
                },
            },
            rt.receiver_now_ns(),
        );
        assert_eq!(denied.unwrap_err(), "temporal_candidate_not_qualified");
        assert_eq!(
            rt.store.load_selection().unwrap(),
            before,
            "refusedSelectmustnotchangeprotectedpointerorhistory"
        );
        assert!(!rt.inner.lock().unwrap().selection.has_history());
        panic!("production-default48 actual fixture refused qualification; positiveSelect remainsOPEN; preservedprofile={}",directory.display());
    }
    let safety_preselect = tempfile::Builder::new()
        .prefix("SYNTHETIC-c4c-safety-preselect-")
        .tempdir()
        .unwrap()
        .keep();
    copy_private(directory, &safety_preselect);
    let selection_request = TemporalSelectRequestV2 {
        schema: SELECT_REQUEST_SCHEMA.into(),
        expected: rt.mutation_guard().unwrap(),
        capture_generation_id: c.capture_generation_id,
        refs: TemporalQualificationRefsWire {
            evaluation: evaluation.refs,
            qualification_sha256: q.unwrap().into(),
        },
    };
    let selected = rt
        .temporal_select_v2(selection_request.clone(), rt.receiver_now_ns())
        .unwrap();
    assert_eq!(selected.as_object().unwrap().len(), 3);
    assert_eq!(
        selected["model_selection"]["schema"],
        SELECTION_STATUS_SCHEMA
    );
    let identity: TemporalSelectedIdentityWire =
        serde_json::from_value(selected["model_selection"]["selected"].clone()).unwrap();
    assert_eq!(identity.model_kind, WholeTemporalKind::ManagedTemporalV3);
    assert_ne!(
        selected["model_selection"]["current"]["status"], "ready",
        "historicalHeldOutisnotliveinput"
    );
    assert!(
        rt.deselect_model(&rt.mutation_guard().unwrap()).is_err(),
        "cold/wholekindcannotuselegacywithdrawal"
    );
    println!(
        "SYNTHETIC_C4C_DEFAULT48_SELECTED={}",
        serde_json::to_string(&identity).unwrap()
    );
    exercise_live_and_cold_withdrawal(&rt, directory, &identity, &selection_request).await;
    drop(rt);
    safety::run(&safety_preselect, directory).await;
}

#[derive(Debug, Default)]
struct LiveTiming {
    packets: u64,
    min_gap_ns: Option<u64>,
    max_gap_ns: u64,
    below_min: u64,
    above_max: u64,
    policy_min_ns: u64,
    policy_max_ns: u64,
}
struct LiveFeed {
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
    error: Arc<Mutex<Option<String>>>,
    timing: Arc<Mutex<LiveTiming>>,
}
impl Drop for LiveFeed {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            handle.join().unwrap();
        }
    }
}
fn live_feed(rt: &Arc<RoomLearningRuntime>) -> LiveFeed {
    let selected = rt.inner.lock().unwrap().selection.temporal.clone().unwrap();
    let mut sequence = selected
        .driver
        .lock()
        .unwrap()
        .c4c_replay_floor_probe()
        .iter()
        .map(|f| f.1)
        .max()
        .unwrap();
    let policy = selected.stored.model().window_input_policy().unwrap();
    let min_gap = policy
        .source_policies()
        .iter()
        .map(|p| p.min_packet_interval_ns)
        .max()
        .unwrap();
    let max_gap = policy
        .source_policies()
        .iter()
        .map(|p| p.max_gap_ns)
        .min()
        .unwrap();
    let previous = selected
        .driver
        .lock()
        .unwrap()
        .c4c_replay_floor_probe()
        .iter()
        .map(|f| f.2)
        .max()
        .unwrap();
    let timing = Arc::new(Mutex::new(LiveTiming {
        policy_min_ns: min_gap,
        policy_max_ns: max_gap,
        ..LiveTiming::default()
    }));
    let stats = timing.clone();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let error = Arc::new(Mutex::new(None));
    let failure = error.clone();
    let weak = Arc::downgrade(rt);
    let handle = std::thread::spawn(move || {
        let origin = Instant::now();
        let mut ordinal = 0u32;
        let mut previous = previous;
        let mut next_minimum = origin + Duration::from_nanos(min_gap);
        while !flag.load(Ordering::Acquire) {
            // Schedule real emissions, never rewrite packet time. A missed
            // nominal deadline cannot produce a catch-up burst below the
            // unchanged source policy's minimum packet interval.
            let target = (origin + Duration::from_nanos(u64::from(ordinal) * fixture::PERIOD))
                .max(next_minimum);
            // This finite SOFTWARE stimulus owns one host thread. Sleeping
            // timers may be coalesced beyond a30Hz receiver contract; wait
            // against the actual monotonic deadline without forging time.
            while Instant::now() < target && !flag.load(Ordering::Acquire) {
                std::hint::spin_loop();
            }
            if flag.load(Ordering::Acquire) {
                break;
            }
            let Some(runtime) = weak.upgrade() else { break };
            sequence = sequence.checked_add(1).unwrap();
            let now = runtime.receiver_now_ns();
            next_minimum = Instant::now() + Duration::from_nanos(min_gap);
            {
                let gap = now - previous;
                let mut st = stats.lock().unwrap();
                st.packets += 1;
                st.min_gap_ns = Some(st.min_gap_ns.map_or(gap, |v| v.min(gap)));
                st.max_gap_ns = st.max_gap_ns.max(gap);
                st.below_min += u64::from(gap < min_gap);
                st.above_max += u64::from(gap > max_gap);
            }
            previous = now;
            for source in 1..=3 {
                if !runtime.selects_grid(
                    source,
                    &fixture::context()
                        .sources
                        .iter()
                        .find(|s| s.source_id == source)
                        .unwrap()
                        .grid_id,
                ) {
                    *failure.lock().unwrap() = Some("actualcoldselectedgridnotadmitted".into());
                    return;
                }
                if let Err(e) =
                    runtime.ingest_native(&packet(source, sequence, now, RoomLabel::Empty), now)
                {
                    *failure.lock().unwrap() = Some(e);
                    return;
                }
            }
            drop(runtime);
            ordinal = ordinal.checked_add(1).unwrap();
        }
    });
    LiveFeed {
        stop,
        handle: Some(handle),
        error,
        timing,
    }
}
async fn current_ready(rt: &RoomLearningRuntime, feed: &LiveFeed) -> Value {
    let start = Instant::now();
    loop {
        let error = feed.error.lock().unwrap().clone();
        assert!(error.is_none(), "actualsyntheticliveinputfailed {error:?}");
        let now = rt.receiver_now_ns();
        let frame = rt
            .temporal_selected_binary_v2(
                now,
                Some(&fixture::context().room_digest),
                Some("synthetic-c4c-public-boot"),
            )
            .unwrap();
        if frame["status"] == "ready" {
            println!(
                "SYNTHETIC_C4C_ACTUAL_LIVE_EMISSION_TIMING={:?}",
                feed.timing.lock().unwrap()
            );
            return frame;
        }
        if start.elapsed() >= Duration::from_secs(12) {
            let selected = rt.inner.lock().unwrap().selection.temporal.clone().unwrap();
            let error = selected
                .driver
                .lock()
                .unwrap()
                .c4c_last_input_failure()
                .map(str::to_owned);
            panic!("actualsyntheticselectedinputdidnotbecomeready input_error={error:?} timing={:?} {frame}",feed.timing.lock().unwrap());
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
async fn exercise_live_and_cold_withdrawal(
    rt: &Arc<RoomLearningRuntime>,
    directory: &Path,
    identity: &TemporalSelectedIdentityWire,
    selection_request: &TemporalSelectRequestV2,
) {
    let feed = live_feed(rt);
    let frame = current_ready(rt, &feed).await;
    assert_eq!(frame["identity"], serde_json::to_value(identity).unwrap());
    assert_eq!(frame["schema"], SELECTED_BINARY_SCHEMA);
    let first = frame["window_start_unix_ms"].as_u64().unwrap();
    let end = frame["window_end_unix_ms"].as_u64().unwrap();
    let expires = frame["valid_until_unix_ms"].as_u64().unwrap();
    assert!((2000..=2150).contains(&(end - first)));
    assert_eq!(frame["observed_at_unix_ms"], end);
    assert_eq!(expires, first + 5000);
    let before_arc = rt.inner.lock().unwrap().selection.temporal.clone().unwrap();
    let before_floors = before_arc.driver.lock().unwrap().c4c_replay_floor_probe();
    let before_pointer = rt
        .store
        .load_temporal_selection()
        .unwrap()
        .unwrap()
        .envelope()
        .clone();
    let retried = rt
        .temporal_select_v2(
            TemporalSelectRequestV2 {
                expected: rt.mutation_guard().unwrap(),
                ..selection_request.clone()
            },
            rt.receiver_now_ns(),
        )
        .unwrap();
    assert_eq!(
        retried["model_selection"]["selected"],
        serde_json::to_value(identity).unwrap()
    );
    let after_arc = rt.inner.lock().unwrap().selection.temporal.clone().unwrap();
    assert!(
        Arc::ptr_eq(&before_arc, &after_arc),
        "lostACKsamerateSelectresetdriver"
    );
    assert_eq!(
        *rt.store
            .load_temporal_selection()
            .unwrap()
            .unwrap()
            .envelope(),
        before_pointer
    );
    let after_floors = after_arc.driver.lock().unwrap().c4c_replay_floor_probe();
    for before in before_floors {
        let after = after_floors.iter().find(|f| f.0 == before.0).unwrap();
        assert!(after.1 >= before.1 && after.2 >= before.2);
    }
    assert_eq!(
        rt.temporal_selected_binary_v2(
            rt.receiver_now_ns(),
            Some(&fixture::context().room_digest),
            Some("synthetic-c4c-public-boot")
        )
        .unwrap()["status"],
        "ready",
        "idempotentSelectcaused2swarmup"
    );
    let boot = frame["boot_epoch"].clone();
    let receiver = frame["receiver_epoch"].clone();
    for _ in 0..3 {
        let proof = rt
            .temporal_selection_v2(
                rt.receiver_now_ns(),
                Some(&fixture::context().room_digest),
                Some("synthetic-c4c-public-boot"),
            )
            .unwrap();
        assert_eq!(proof["model_selection"]["current"]["status"], "ready");
        assert_eq!(
            proof["model_selection"]["current"]["identity"],
            serde_json::to_value(identity).unwrap()
        );
        assert_eq!(proof["model_selection"]["current"]["boot_epoch"], boot);
        assert_eq!(
            proof["model_selection"]["current"]["receiver_epoch"],
            receiver
        );
    }
    let wrong = rt
        .paired_temporal_progress_snapshot(
            &fixture::context().installation_id,
            "synthetic-c4c-public-boot",
            Some(crate::room_learner::lifecycle::PublishedProgressRoom {
                binding_digest: "c".repeat(64),
                revision: fixture::context().revision,
            }),
            &[1, 2, 3],
            Some(&fixture::context().digest()),
            rt.receiver_now_ns(),
        )
        .unwrap();
    let wrong = serde_json::to_value(wrong).unwrap();
    assert_eq!(wrong["context_match"], false);
    for key in [
        "room",
        "capture",
        "candidate",
        "profile",
        "evaluation",
        "qualification",
        "selection",
        "sources",
        "model_kind",
    ] {
        assert!(wrong[key].is_null(), "pairedmismatchleaked{key}");
    }
    println!(
        "SYNTHETIC_C4C_ACTUAL_SELECTED_LIVE={}",
        serde_json::to_string(&json!({"current":frame,"paired_mismatch":wrong})).unwrap()
    );
    drop(feed);
    let copy = tempfile::Builder::new()
        .prefix("SYNTHETIC-c4c-selected-expired-")
        .tempdir()
        .unwrap()
        .keep();
    copy_private(directory, &copy);
    // Already-selected cold operation intentionally has no capture. The
    // protected admitted chain, not an expired raw reread, supplies its original
    // retention deadline for this test's separate cold-copy clock domain.
    let deadline = rt
        .inner
        .lock()
        .unwrap()
        .selection
        .temporal
        .as_ref()
        .unwrap()
        .stored
        .held()
        .body
        .identity
        .original_deadline
        .expires_at_unix_ms;
    fn deny_raw(root: &Path) {
        use std::os::unix::fs::PermissionsExt;
        for e in std::fs::read_dir(root).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                deny_raw(&p)
            } else if p
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("slot-")
            {
                std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o000)).unwrap();
            }
        }
    }
    deny_raw(&copy);
    let cold = Arc::new(fixture::runtime(&copy, deadline * 1_000_000 + 1));
    assert!(cold.inner.lock().unwrap().capture.is_none());
    assert!(cold.temporal_v2_state_present());
    let selection = cold
        .temporal_selection_v2(
            cold.receiver_now_ns(),
            Some(&fixture::context().room_digest),
            Some("synthetic-cold-boot"),
        )
        .unwrap();
    assert_eq!(
        selection["model_selection"]["selected"],
        serde_json::to_value(identity).unwrap()
    );
    assert_ne!(selection["model_selection"]["current"]["status"], "ready");
    assert_eq!(cold.temporal_progress_context(), Some(fixture::context()));
    let cold_arc = cold
        .inner
        .lock()
        .unwrap()
        .selection
        .temporal
        .clone()
        .unwrap();
    let cold_retry = cold
        .temporal_select_v2(
            TemporalSelectRequestV2 {
                expected: cold.mutation_guard().unwrap(),
                ..selection_request.clone()
            },
            cold.receiver_now_ns(),
        )
        .unwrap();
    assert_eq!(
        cold_retry["model_selection"]["selected"],
        serde_json::to_value(identity).unwrap()
    );
    assert!(Arc::ptr_eq(
        &cold_arc,
        cold.inner
            .lock()
            .unwrap()
            .selection
            .temporal
            .as_ref()
            .unwrap()
    ));

    // Actual fresh native input must reach the admitted model even when cold
    // selected state has NO capture and the original raw slots are unreadable.
    let cold_feed = live_feed(&cold);
    let cold_frame = current_ready(&cold, &cold_feed).await;
    assert_eq!(
        cold_frame["identity"],
        serde_json::to_value(identity).unwrap()
    );
    drop(cold_feed);
    tokio::time::sleep(Duration::from_millis(250)).await;
    assert_ne!(
        cold.temporal_selected_binary_v2(
            cold.receiver_now_ns(),
            Some(&fixture::context().room_digest),
            Some("synthetic-cold-boot")
        )
        .unwrap()["status"],
        "ready"
    );

    assert!(cold
        .deselect_model(&cold.mutation_guard().unwrap())
        .is_err());
    let off = cold
        .temporal_deselect_v2(
            TemporalDeselectRequestV2 {
                schema: DESELECT_REQUEST_SCHEMA.into(),
                model_kind: WholeTemporalKind::ManagedTemporalV3,
                expected: cold.mutation_guard().unwrap(),
                selection_sha256: identity.selection_sha256.clone(),
                selection_revision: identity.selection_revision,
            },
            cold.receiver_now_ns(),
        )
        .unwrap();
    assert_eq!(off["model_selection"]["schema"], SELECTION_STATUS_SCHEMA);
    assert_eq!(
        off["model_selection"]["revision"],
        identity.selection_revision + 1
    );
    assert!(off["model_selection"]["selected"].is_null());
    assert_eq!(off["model_selection"]["current"]["status"], "none");
    drop(cold);
    let reread = fixture::runtime(&copy, deadline * 1_000_000 + 1_000_000_000);
    assert!(!reread.temporal_v2_state_present());
    let pointer = reread.store.load_selection().unwrap().unwrap();
    assert!(pointer.receipt.selected.is_none());
    assert!(pointer.receipt.previous.is_none());
    assert_eq!(pointer.receipt.revision, identity.selection_revision + 1);
    assert_eq!(
        rt.store
            .load_temporal_selection()
            .unwrap()
            .unwrap()
            .identity(),
        *identity,
        "otherownedprofilewithdrawalmustnotchangeoriginalmodelselection"
    );
}

fn same_accepted_prefix(before: &Value, after: &Value) {
    let count = before["payload"]["accepted"]["rows"]
        .as_array()
        .unwrap()
        .len();
    for field in ["rows", "evidence", "identities"] {
        let a = &before["payload"]["accepted"][field].as_array().unwrap()[..count];
        let b = &after["payload"]["accepted"][field].as_array().unwrap()[..count];
        assert_eq!(
            crate::room_learner::qualification::hash(&a).unwrap(),
            crate::room_learner::qualification::hash(&b).unwrap(),
            "originalacceptedprefixchanged{field}"
        );
    }
}

#[cfg(all(test, unix))]
#[path = "lifecycle_temporal_public_safety_tests.rs"]
mod safety;
