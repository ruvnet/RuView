//! SYNTHETIC private C4B base-interop probes; no producer/fit authority issuer.
use super::tests as fixture;
use super::*;
use crate::room_learner::{
    native::{canonicalize_esp32, NativePacketInput},
    observation::CaptureProvenance,
};
use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
use wifi_densepose_hardware::{Adr018Flags, PpduType};
#[test]
fn c4b_full_v3_cannot_adopt_kind1_fresh343_train_policy() {
    use sha2::{Digest, Sha256};
    let directory = tempfile::tempdir().unwrap();
    let rt = fixture::runtime(directory.path(), fixture::START);
    rt.configure_temporal_full_run(
        &rt.mutation_guard().unwrap(),
        fixture::context(),
        "synthetic-c4b-profile-interop".into(),
        fixture::START,
    )
    .unwrap();
    let now = fixture::settle_async_boundary(&rt, fixture::START);
    let store = rt.temporal_store().unwrap();
    let loaded = store.load_temporal(now).unwrap().unwrap();
    let prefix = loaded.prefix.as_ref().unwrap();
    let original = std::str::from_utf8(prefix.bytes()).unwrap();
    let data: serde_json::Value = serde_json::from_str(original).unwrap();
    let old = data["payload"]["stage"]["bridge"]["policy"]
        .as_str()
        .unwrap();
    let fresh="native-receiver-whole-train-period-v2;all-selected-packets;existing-cadence-gap-arrival-skew;confirmed-durable-seals;train-budget343;no-fit-authority";
    assert_ne!(old, fresh);
    let mut changed = original.replacen(old, fresh, 1);
    assert_ne!(changed, original);
    let start = changed.find("\"payload\":").unwrap() + "\"payload\":".len();
    let digest = format!(
        "{:x}",
        Sha256::digest(changed[start..changed.len() - 1].as_bytes())
    );
    changed = changed.replacen(data["payload_sha256"].as_str().unwrap(), &digest, 1);
    let error=crate::room_learner::temporal_model_input::managed::execution::checkpoint::validated_prefix(changed.as_bytes(),prefix.descriptor(),now).err().expect("Full-v3 silently adopted a different Train profile");
    assert_eq!(
        error, "temporal_full_run_train_profile_changed",
        "failure must not be merely a changed canonical JSON checksum"
    );
    assert_eq!(
        store.load_temporal(now).unwrap().unwrap().token,
        loaded.token,
        "raw structural probe must not publish anything"
    );
    assert_eq!(prefix.metadata_kind(), "temporal-prefix-commitment-v2");
    assert_eq!(prefix.transition_limit(), 433);
    rt.cancel_temporal_durability(&rt.mutation_guard().unwrap())
        .unwrap();
}

fn configured(root: &Path) -> (Arc<RoomLearningRuntime>, u64) {
    let runtime = Arc::new(fixture::runtime(root, fixture::START));
    let store = runtime.recovery_store.clone().unwrap();
    runtime
        .configure_temporal_capture_with_custody(
            &runtime.mutation_guard().unwrap(),
            fixture::context(),
            "synthetic-c4b-native-fit".into(),
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
                    now = fixture::async_lead_end(&runtime, now);
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
        "SYNTHETIC_C4B_REAL_TRAIN_CAPTURE: rows={} seals={} starts={starts}",
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

fn phase_target(rt: &RoomLearningRuntime) -> Option<(u64, u64)> {
    let state = rt.inner.lock().unwrap();
    let Some(CaptureWorkflow::Temporal(c)) = &state.capture else {
        panic!()
    };
    c.core.as_ref().unwrap().c4b_test_target()
}
fn receiver_floor(rt: &RoomLearningRuntime) -> u32 {
    let state = rt.inner.lock().unwrap();
    let Some(CaptureWorkflow::Temporal(c)) = &state.capture else {
        panic!()
    };
    c.core
        .as_ref()
        .unwrap()
        .test_full_run_floors()
        .0
        .iter()
        .map(|(_, q, _)| u32::try_from(*q).unwrap())
        .max()
        .unwrap()
}
fn prefix_value(rt: &RoomLearningRuntime, now: u64) -> serde_json::Value {
    let loaded = rt
        .temporal_store()
        .unwrap()
        .load_temporal(now)
        .unwrap()
        .unwrap();
    serde_json::from_slice(loaded.prefix.unwrap().bytes()).unwrap()
}
fn same_train_and_frozen(before: &serde_json::Value, after: &serde_json::Value) {
    for field in ["rows", "evidence", "identities"] {
        assert_eq!(
            &before["payload"]["accepted"][field].as_array().unwrap()[..64],
            &after["payload"]["accepted"][field].as_array().unwrap()[..64],
            "immutable Train {field}"
        );
    }
    assert_eq!(
        before["payload"]["stage"]["fit"],
        after["payload"]["stage"]["fit"]
    );
    assert_eq!(
        before["payload"]["stage"]["train_seals"],
        after["payload"]["stage"]["train_seals"]
    );
    assert_original_lifetime(
        &after["payload"]["common"]["descriptor"],
        &before["payload"]["common"]["descriptor"],
    );
}
async fn produce_complete_uncached(directory: &Path) -> u64 {
    let (mut rt, mut now) = record_train(directory);
    now = fit_ready(&rt, now).await;
    let (model, receipt, _) = rt.temporal_diagnostic_fit().unwrap().unwrap();
    let model_bytes = model.export().unwrap();
    let report_before = serde_json::to_value(rt.store.temporal_fit_diagnostics(&model)).unwrap();
    now = prepare_frozen(&rt, now).await;
    now = fixture::settle_async_boundary(&rt, now);
    assert_eq!(fixture::snapshot(&rt, now)["stage"], "frozen");
    let frozen = prefix_value(&rt, now);
    let original = frozen["payload"]["common"]["descriptor"].clone();
    let schedule = ManagedScheduleContract::build(
        fixture::context(),
        LeadInPolicy::Train5HeldOut15EngineeringV1,
    )
    .unwrap();
    let mut sequence = receiver_floor(&rt);
    let mut partial_cold = false;
    let mut terminal_cold = false;
    let mut starts = 0;
    for episode in schedule
        .episodes()
        .iter()
        .filter(|e| e.role == DataRole::HeldOut)
    {
        loop {
            let challenge = rt.temporal_confirmation_challenge(&rt.mutation_guard().unwrap());
            if challenge.is_ok() {
                if episode.index == 14 && !terminal_cold {
                    let before = prefix_value(&rt, now);
                    let rows = fixture::snapshot(&rt, now)["durable_windows"].clone();
                    drop(rt);
                    now += 1_000_000_000;
                    rt = Arc::new(fixture::runtime(directory, now));
                    rt.restore_temporal_full_run(
                        &rt.mutation_guard().unwrap(),
                        &fixture::context(),
                        now,
                    )
                    .unwrap();
                    assert_eq!(fixture::snapshot(&rt, now)["durable_windows"], rows);
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
                    now = fixture::async_lead_end(&rt, now);
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
                rt.restore_temporal_full_run(
                    &rt.mutation_guard().unwrap(),
                    &fixture::context(),
                    now,
                )
                .unwrap();
                assert_eq!(fixture::snapshot(&rt, now)["durable_windows"], durable);
                assert!(phase_target(&rt).is_none());
                assert_eq!(
                    before["payload"]["accepted"],
                    prefix_value(&rt, now)["payload"]["accepted"]
                );
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
        rt.confirm_temporal_phase(&guard, &challenge, now).unwrap();
        now = fixture::settle_async_boundary(&rt, now);
        assert_eq!(
            fixture::snapshot(&rt, now)["sealed_held_out_episodes"],
            episode.index - 6
        );
        let sealed=prefix_value(&rt,now);
        same_train_and_frozen(&frozen, &sealed);
        println!("SYNTHETIC_C4B_HELDOUT_SEAL episode={} durable={} sealed={} starts={starts}",episode.index,fixture::snapshot(&rt,now)["durable_windows"],fixture::snapshot(&rt,now)["sealed_held_out_episodes"]);
        if episode.index==13{
            let loaded=rt.temporal_store().unwrap().load_temporal(now).unwrap().unwrap();let prefix=loaded.prefix.unwrap();
            let oracle=crate::room_learner::temporal_model_input::managed::execution::checkpoint::c4b_reference_batch_and_stale_seal_probe(prefix.bytes(),prefix.descriptor(),now).unwrap();
            println!("SYNTHETIC_C4B_FRESH120_REFERENCE_STALE_SEAL={oracle}");
        }
    }
    assert!(
        partial_cold && terminal_cold,
        "two actual cold interruptions must execute"
    );
    let status = fixture::snapshot(&rt, now);
    assert_eq!(status["stage"], "full_complete");
    assert_eq!(status["durable_windows"], 128);
    assert_eq!(status["sealed_train_episodes"], 7);
    assert_eq!(status["sealed_held_out_episodes"], 8);
    let complete = prefix_value(&rt, now);
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
    let final_prefix=loaded.prefix.unwrap();
    let bytes = final_prefix.bytes().len();
    let oracle=crate::room_learner::temporal_model_input::managed::execution::checkpoint::c4b_reference_batch_and_stale_seal_probe(final_prefix.bytes(),final_prefix.descriptor(),now).unwrap();
    println!("SYNTHETIC_C4B_FRESH128_REFERENCE_STALE_SEAL={oracle}");
    let raw_values=complete["payload"]["accepted"]["rows"].as_array().unwrap().iter().map(|row|row["views"].as_array().unwrap().iter().map(|view|view["samples"].as_array().unwrap().iter().map(|sample|sample.as_array().unwrap().len()).sum::<usize>()).sum::<usize>()).sum::<usize>();
    assert!(raw_values<=2_000_000);
    fn slot_sizes(root:&Path,out:&mut Vec<u64>){for entry in std::fs::read_dir(root).unwrap(){let path=entry.unwrap().path();if path.is_dir(){slot_sizes(&path,out)}else if path.file_name().unwrap().to_string_lossy().starts_with("slot-")&&path.extension().is_some_and(|e|e=="bin"){out.push(std::fs::metadata(path).unwrap().len());}}}
    let mut outer_slot_sizes=vec![];slot_sizes(directory,&mut outer_slot_sizes);outer_slot_sizes.sort_unstable();
    println!("SYNTHETIC_C4B_RAW_NUMERIC_VALUES={raw_values} AEAD_SLOT_FILE_BYTES={outer_slot_sizes:?}");
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
    let cold = fixture::runtime(directory, now);
    cold.restore_temporal_full_run(&cold.mutation_guard().unwrap(), &fixture::context(), now)
        .unwrap();
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
            .load_exact_temporal_candidate(&fixture::context(), &receipt.artifact_sha256)
            .unwrap()
            .export()
            .unwrap(),
        model_bytes
    );
    cold.cancel_temporal_durability(&cold.mutation_guard().unwrap())
        .unwrap();
    now
}

/// Isolated actual native/fence resource reproduction. This does NOT issue a
/// fitted/conditions/write permission or claim the full fifteen-phase path.
#[test]
fn c4b_native_partial_rollback_reproduces_retained_only_frame_fence_assumption() {
    use crate::room_learner::temporal_acquisition::{
        AcquisitionPlan, ExperimentalPlanId, NativeWindowAssembler, ReceiverCadenceBand,
        WindowAdmission,
    };
    let budget = actual_full2_replay_budget();
    let context = fixture::context();
    let plan = AcquisitionPlan::build(
        context,
        ExperimentalPlanId::Native2000msSlots8,
        &(1..=3)
            .map(|id| (id, ReceiverCadenceBand::Hz30To36))
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let maximum = plan
        .source_policies()
        .iter()
        .map(|source| source.max_rows)
        .sum::<usize>()
        * 128;
    let mut assembler =
        NativeWindowAssembler::new(plan, "synthetic-b-fence-isolation".into()).unwrap();
    let mut sequence = 0u32;
    let mut now = fixture::START;
    let mut retained = 0usize;
    let mut discarded = 0usize;
    let mut reproduced = false;
    'attempts: for _ in 0..128 {
        // One bounded partially consumed reservation, not a fabricated ID set.
        assembler.begin_window(now, now + 6_000_000_000).unwrap();
        for index in 0..24 {
            sequence += 1;
            let at = now + index * fixture::PERIOD;
            for source in 1..=3 {
                assert_eq!(
                    assembler
                        .ingest(&packet(source, sequence, at, RoomLabel::Empty))
                        .unwrap(),
                    WindowAdmission::Collecting
                );
            }
        }
        discarded += 1;
        let checkpoint = assembler
            .fence_checkpoint()
            .unwrap()
            .for_prefix(retained)
            .unwrap();
        let state = serde_json::to_value(&checkpoint).unwrap();
        if let Err(error) = assembler.restore_prefix_fences(checkpoint.clone(), retained) {
            let next = state["next"].as_u64().unwrap() as usize;
            let frames = state["frames"].as_array().unwrap().len();
            let watermarks = state["watermarks"].as_array().unwrap().len();
            println!("SYNTHETIC_FENCE_BOUND phase=partial next={next} expected={retained} next_gt128={} frames={frames} max={maximum} frames_gtmax={} watermarks={watermarks} sources=3 retained={retained} discarded={discarded}",next>128,frames>maximum);
            assert_eq!(error, "temporal_checkpoint_fence_bounds");
            assert_eq!(next, retained);
            assert!(next <= 128);
            assert!(frames > maximum);
            assert!(watermarks <= 3);
            assembler
                .restore_full_run_prefix_fences(checkpoint, retained, &budget)
                .unwrap();
            assert_eq!(
                assembler.c4b_test_fence_counts().1,
                frames,
                "typed budget must preserve EVERY actual ID"
            );
            reproduced = true;
            break 'attempts;
        }
        now += 6_000_000_000;
        assembler.begin_window(now, now + 6_000_000_000).unwrap();
        let mut ready = false;
        'packets: for index in 0..77 {
            sequence += 1;
            let at = now + index * fixture::PERIOD;
            for source in 1..=3 {
                if assembler
                    .ingest(&packet(source, sequence, at, RoomLabel::Empty))
                    .unwrap()
                    == WindowAdmission::Ready
                {
                    assembler.finish_window(at).unwrap();
                    ready = true;
                    break 'packets;
                }
            }
        }
        assert!(ready);
        retained += 1;
        let checkpoint = assembler.fence_checkpoint().unwrap();
        let state = serde_json::to_value(&checkpoint).unwrap();
        if let Err(error) = assembler.restore_prefix_fences(checkpoint.clone(), retained) {
            let next = state["next"].as_u64().unwrap() as usize;
            let frames = state["frames"].as_array().unwrap().len();
            let watermarks = state["watermarks"].as_array().unwrap().len();
            println!("SYNTHETIC_FENCE_BOUND phase=retained next={next} expected={retained} next_gt128={} frames={frames} max={maximum} frames_gtmax={} watermarks={watermarks} sources=3 retained={retained} discarded={discarded}",next>128,frames>maximum);
            assert_eq!(error, "temporal_checkpoint_fence_bounds");
            assert_eq!(next, retained);
            assert!(next <= 128);
            assert!(frames > maximum);
            assert!(watermarks <= 3);
            assembler
                .restore_full_run_prefix_fences(checkpoint, retained, &budget)
                .unwrap();
            assert_eq!(
                assembler.c4b_test_fence_counts().1,
                frames,
                "typed budget must preserve EVERY actual ID"
            );
            reproduced = true;
            break 'attempts;
        }
        now += 6_000_000_000;
    }
    assert!(reproduced,"bounded actual native/discard fixture did not reproduce the suspected retained-only resource assumption");
    assert!(retained <= 128 && discarded <= 128);
}

#[test]
fn c4b_old_kind1_status_shape_has_no_full2_role_fields() {
    for fresh in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let rt = fixture::runtime(directory.path(), fixture::START);
        if fresh {
            rt.configure_fresh_train_343_capture(
                &rt.mutation_guard().unwrap(),
                fixture::context(),
                "synthetic-old-status343".into(),
                fixture::START,
            )
            .unwrap();
        } else {
            rt.configure_temporal_capture(
                &rt.mutation_guard().unwrap(),
                fixture::context(),
                "synthetic-old-status329".into(),
                fixture::START,
            )
            .unwrap();
        }
        let now = fixture::settle_async_boundary(&rt, fixture::START);
        let value = fixture::snapshot(&rt, now);
        assert!(value.get("sealed_held_out_episodes").is_none());
        assert_eq!(value["role"], "Train");
        assert_eq!(
            value["schema"],
            "ruview.room-learner.private-temporal-capture.v1"
        );
        assert_eq!(
            value["label"],
            serde_json::to_value(RoomLabel::Empty).unwrap()
        );
        rt.cancel_temporal_durability(&rt.mutation_guard().unwrap())
            .unwrap();
    }
}

fn actual_full2_replay_budget(
) -> crate::room_learner::temporal_model_input::managed::execution::full_run::ReplayFenceBudget {
    let directory = tempfile::tempdir().unwrap();
    let rt = fixture::runtime(directory.path(), fixture::START);
    rt.configure_temporal_full_run(
        &rt.mutation_guard().unwrap(),
        fixture::context(),
        "synthetic-full2-resource-budget".into(),
        fixture::START,
    )
    .unwrap();
    let now = fixture::settle_async_boundary(&rt, fixture::START);
    let budget = {
        let state = rt.inner.lock().unwrap();
        let Some(CaptureWorkflow::Temporal(c)) = &state.capture else {
            panic!()
        };
        c.core.as_ref().unwrap().c4b_test_replay_budget().unwrap()
    };
    rt.cancel_temporal_durability(&rt.mutation_guard().unwrap())
        .unwrap();
    assert_eq!(budget.max_frames(), 59_136);
    let _ = now;
    budget
}
#[test]
fn c4b_full2_fence_budget_exact_boundary_and_other_guards_remain_strict() {
    use crate::room_learner::temporal_acquisition::{
        AcquisitionPlan, AssemblerCheckpoint, ExperimentalPlanId, NativeWindowAssembler,
        ReceiverCadenceBand,
    };
    let budget = actual_full2_replay_budget();
    let make = |context| {
        NativeWindowAssembler::new(
            AcquisitionPlan::build(
                context,
                ExperimentalPlanId::Native2000msSlots8,
                &(1..=3)
                    .map(|id| (id, ReceiverCadenceBand::Hz30To36))
                    .collect::<Vec<_>>(),
            )
            .unwrap(),
            "synthetic-fence-boundary".into(),
        )
        .unwrap()
    };
    let mut assembler = make(fixture::context());
    let per_source = 59_136 / 3;
    let mut frames = Vec::new();
    let mut watermarks = Vec::new();
    for source in 1..=3 {
        for sequence in 1..=per_source {
            let at = fixture::START + sequence as u64 * fixture::PERIOD;
            let p = packet(source, sequence as u32, at, RoomLabel::Empty);
            frames.push(serde_json::json!([
                source,
                p.native_frame.frame_id.to_string()
            ]));
        }
        watermarks.push(serde_json::json!([
            source,
            per_source,
            fixture::START + per_source as u64 * fixture::PERIOD
        ]));
    }
    let value = serde_json::json!({"next":128,"last_end_ns":fixture::START,"frames":frames,"watermarks":watermarks});
    let checkpoint: AssemblerCheckpoint = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(
        assembler
            .restore_prefix_fences(checkpoint.clone(), 128)
            .unwrap_err(),
        "temporal_checkpoint_fence_bounds"
    );
    assembler
        .restore_full_run_prefix_fences(checkpoint, 128, &budget)
        .unwrap();
    assert_eq!(assembler.c4b_test_fence_counts().1, 59_136);
    assert!(
        assembler
            .begin_window(
                fixture::START + 1_000_000_000_000,
                fixture::START + 1_006_000_000_000
            )
            .is_err(),
        "no129thwindow may createa newtail"
    );
    assert_eq!(assembler.c4b_test_fence_counts().1, 59_136);
    let mut beyond = value.clone();
    let sequence = per_source + 1;
    let at = fixture::START + sequence as u64 * fixture::PERIOD;
    let p = packet(1, sequence as u32, at, RoomLabel::Empty);
    beyond["frames"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!([1, p.native_frame.frame_id.to_string()]));
    beyond["watermarks"][0] = serde_json::json!([1, sequence, at]);
    assert_eq!(
        make(fixture::context())
            .restore_full_run_prefix_fences(serde_json::from_value(beyond).unwrap(), 128, &budget)
            .unwrap_err(),
        "temporal_checkpoint_fence_bounds"
    );
    let mut wrong_next = value.clone();
    wrong_next["next"] = serde_json::json!(129);
    assert_eq!(
        make(fixture::context())
            .restore_full_run_prefix_fences(
                serde_json::from_value(wrong_next).unwrap(),
                129,
                &budget
            )
            .unwrap_err(),
        "temporal_checkpoint_fence_bounds"
    );
    let mut extra_source = value.clone();
    extra_source["watermarks"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!([4, 1, fixture::START]));
    assert_eq!(
        make(fixture::context())
            .restore_full_run_prefix_fences(
                serde_json::from_value(extra_source).unwrap(),
                128,
                &budget
            )
            .unwrap_err(),
        "temporal_checkpoint_fence_bounds"
    );
    let mut context = fixture::context();
    context.room_digest = "different-room".into();
    assert_eq!(
        make(context)
            .restore_full_run_prefix_fences(serde_json::from_value(value).unwrap(), 128, &budget)
            .unwrap_err(),
        "temporal_full2_fence_policy_changed"
    );
}
#[test]
fn c4b_rejected_native_packet_adds_no_fence_and_terminal_state_cannot_accumulate() {
    use crate::room_learner::temporal_acquisition::{
        AcquisitionPlan, ExperimentalPlanId, NativeWindowAssembler, ReceiverCadenceBand,
    };
    let plan = AcquisitionPlan::build(
        fixture::context(),
        ExperimentalPlanId::Native2000msSlots8,
        &(1..=3)
            .map(|id| (id, ReceiverCadenceBand::Hz30To36))
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let mut assembler =
        NativeWindowAssembler::new(plan, "synthetic-rejection-fence".into()).unwrap();
    assembler
        .begin_window(fixture::START, fixture::START + 6_000_000_000)
        .unwrap();
    let p = packet(1, 1, fixture::START, RoomLabel::Empty);
    assembler.ingest(&p).unwrap();
    let before = assembler.c4b_test_fence_counts();
    assert_eq!(
        assembler.ingest(&p).unwrap_err(),
        "acquisition_replayed_or_reversed"
    );
    let after = assembler.c4b_test_fence_counts();
    assert_eq!(after.1, before.1);
    assert_eq!(after.2, before.2);
    assert!(after.3);
    for sequence in 2..10 {
        assert_eq!(
            assembler
                .ingest(&packet(
                    1,
                    sequence,
                    fixture::START + u64::from(sequence) * fixture::PERIOD,
                    RoomLabel::Empty
                ))
                .unwrap_err(),
            "acquisition_window_state"
        );
        assert_eq!(assembler.c4b_test_fence_counts().1, before.1);
    }
}

fn runtime_custody(
    rt: &RoomLearningRuntime,
) -> (usize, usize, bool, (usize, usize, usize, bool, bool)) {
    let state = rt.inner.lock().unwrap();
    let Some(CaptureWorkflow::Temporal(c)) = &state.capture else {
        panic!()
    };
    c.core.as_ref().unwrap().c4b_test_custody()
}
#[test]
fn c4b_full2_abort_contract_is_denied_before_any_parent_ticket_operation() {
    let directory = tempfile::tempdir().unwrap();
    let rt = fixture::runtime(directory.path(), fixture::START);
    rt.configure_temporal_full_run(
        &rt.mutation_guard().unwrap(),
        fixture::context(),
        "synthetic-full2-abort-denial".into(),
        fixture::START,
    )
    .unwrap();
    let now = fixture::settle_async_boundary(&rt, fixture::START);
    let before = rt
        .temporal_store()
        .unwrap()
        .load_temporal(now)
        .unwrap()
        .unwrap()
        .token;
    let result = {
        let state = rt.inner.lock().unwrap();
        let Some(CaptureWorkflow::Temporal(c)) = &state.capture else {
            panic!()
        };
        c.core.as_ref().unwrap().c4b_test_abort_contract()
    };
    assert_eq!(
        result.unwrap_err(),
        "temporal_full2_abort_to_parent_unsupported"
    );
    assert_eq!(
        rt.temporal_store()
            .unwrap()
            .load_temporal(now)
            .unwrap()
            .unwrap()
            .token,
        before
    );
    rt.cancel_temporal_durability(&rt.mutation_guard().unwrap())
        .unwrap();
}
#[test]
fn c4b_full2_one_inflight_one_tail_no_third_and_same_repair_adopts_queued_window() {
    let directory = tempfile::tempdir().unwrap();
    let rt = Arc::new(fixture::runtime(directory.path(), fixture::START));
    rt.configure_temporal_full_run(
        &rt.mutation_guard().unwrap(),
        fixture::context(),
        "synthetic-full2-one-tail".into(),
        fixture::START,
    )
    .unwrap();
    let now = fixture::settle_async_boundary(&rt, fixture::START);
    let mut now = fixture::async_lead_end(&rt, now);
    rt.tick_temporal_capture(&rt.mutation_guard().unwrap(), now)
        .unwrap();
    let mut sequence = 0;
    now = fixture::settle_observing_with_sources(&rt, now, &mut sequence, Duration::from_secs(5));
    assert_eq!(runtime_custody(&rt).0, 0, "initial segment acknowledgement must precede the first retained window");
    let writer = {
        let state = rt.inner.lock().unwrap();
        let Some(CaptureWorkflow::Temporal(c)) = &state.capture else {
            panic!()
        };
        c.core.as_ref().unwrap().writer().unwrap()
    };
    let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(1);
    let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
    let mut gate = ReleaseGate(Some(release_tx));
    writer.lose_next_ack();
    writer.hold_next(entered_tx, release_rx);
    let mut blocked = false;
    for _ in 0..6000 {
        sequence += 1;
        for source in 1..=3 {
            match rt.ingest_native(&packet(source, sequence, now, RoomLabel::Empty), now) {
                Ok(()) => {}
                Err(e) if e == "temporal_storage_busy_pause" => {
                    blocked = true;
                    break;
                }
                Err(e) => panic!("one-tail ingress {e}"),
            }
        }
        now += fixture::PERIOD;
        if blocked {
            break;
        }
    }
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(blocked);
    let custody = runtime_custody(&rt);
    assert_eq!(custody.0, 2, "one queued plusone nextconsumed");
    assert_eq!(custody.1, 1);
    assert!(!custody.2);
    assert_eq!(custody.3 .0, 2);
    let frames = custody.3 .1;
    for _ in 0..10 {
        sequence += 1;
        for source in 1..=3 {
            rt.ingest_native(&packet(source, sequence, now, RoomLabel::Empty), now)
                .unwrap();
        }
        now += fixture::PERIOD;
    }
    assert_eq!(
        runtime_custody(&rt).3 .1,
        frames,
        "blockedprimingmustnotcreateathirdwindoworframeIDs"
    );
    gate.release();
    let wait = Instant::now();
    let anchor = now;
    loop {
        now = anchor + wait.elapsed().as_nanos() as u64;
        match rt.tick_temporal_capture(&rt.mutation_guard().unwrap(), now) {
            Err(e) if e == "synthetic_lost_publish_ack" => break,
            Err(e) => panic!("one-tail waiting {e}"),
            Ok(()) => {}
        }
        assert!(wait.elapsed() < Duration::from_secs(30));
        std::thread::sleep(Duration::from_millis(1));
    }
    let stored = rt
        .temporal_store()
        .unwrap()
        .load_temporal(now)
        .unwrap()
        .unwrap();
    assert_eq!(stored.prefix.unwrap().window_count(), 1);
    rt.repair_temporal_capture(&rt.mutation_guard().unwrap(), now)
        .unwrap();
    now = fixture::settle_async_boundary(&rt, now);
    assert_eq!(fixture::snapshot(&rt, now)["durable_windows"], 1);
    let repaired = runtime_custody(&rt);
    assert_eq!(repaired.0, 1);
    assert_eq!(repaired.3 .0, 1);
    assert_eq!(
        repaired.3 .1, frames,
        "SAMEadoptionmustpreservethetailreplayIDswithoutretainingtailraw"
    );
    rt.cancel_temporal_durability(&rt.mutation_guard().unwrap())
        .unwrap();
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


// Cache saved synthetic bytes ONLY. Every user of this fixture makes a new
// isolated copy and obtains actual current AEAD/model/owner recertification.
// No Runtime, token, producer/write capability or authority is cached.
fn complete_saved_copy(root:&Path)->u64{
    assert!(!cfg!(debug_assertions), "actual native full-run fixture requires Release profile; classify every cache consumer explicitly");
    static COMPLETE: std::sync::OnceLock<(tempfile::TempDir,u64)>=std::sync::OnceLock::new();
    let (saved,at)=COMPLETE.get_or_init(||std::thread::spawn(||{
        let directory=tempfile::tempdir().unwrap();
        let runtime=tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
        let now=runtime.block_on(produce_complete_uncached(directory.path()));
        (directory,now)
    }).join().unwrap());
    copy_private(saved.path(),root);at+1_000_000_000
}
#[test]
#[cfg_attr(debug_assertions, ignore = "requires Release: actual native full-run fixture and current-store fault matrix")]
fn c4b_actual_native_train_frozen_eight_heldout_two_cold_interruptions_complete(){
    let d=tempfile::tempdir().unwrap();let now=complete_saved_copy(d.path());
    let rt=fixture::runtime(d.path(),now);
    rt.restore_temporal_full_run(&rt.mutation_guard().unwrap(),&fixture::context(),now).unwrap();
    assert_eq!(fixture::snapshot(&rt,now)["stage"],"full_complete");
    assert_eq!(fixture::snapshot(&rt,now)["durable_windows"],128);
    rt.cancel_temporal_durability(&rt.mutation_guard().unwrap()).unwrap();
}
#[test]
#[cfg_attr(debug_assertions, ignore = "requires Release: actual native full-run fixture and current-store fault matrix")]
fn c4b_actual_complete_rehashed_role_phase_binding_floor_and_deadline_negatives(){
    let d=tempfile::tempdir().unwrap();let now=complete_saved_copy(d.path());let rt=fixture::runtime(d.path(),now);
    let store=rt.temporal_store().unwrap();let loaded=store.load_temporal(now).unwrap().unwrap();let prefix=loaded.prefix.unwrap();
    let report=crate::room_learner::temporal_model_input::managed::execution::checkpoint::c4b_complete_fault_matrix(prefix.bytes(),prefix.descriptor(),now).unwrap();
    println!("SYNTHETIC_C4B_COMPLETE_STRUCTURAL_FAULTS={report}");
    assert!(store.prepare_temporal(Some(&loaded.token),&prefix,now).err().unwrap().contains("producer_write_grant_required"));
    assert_eq!(store.load_temporal(now).unwrap().unwrap().token,loaded.token);
    assert!(rt.inner.lock().unwrap().capture.is_none());
    assert!(rt.inner.lock().unwrap().qualification.is_none());assert!(!rt.inner.lock().unwrap().selection.has_history());
}
#[test]
#[cfg_attr(debug_assertions, ignore = "requires Release: actual native full-run fixture and current-store fault matrix")]
fn c4b_actual_complete_owner_cancel_and_retirement_deny_reactivation(){
    let d=tempfile::tempdir().unwrap();let now=complete_saved_copy(d.path());let rt=fixture::runtime(d.path(),now);
    rt.restore_temporal_full_run(&rt.mutation_guard().unwrap(),&fixture::context(),now).unwrap();
    let store=rt.recovery_store.clone().unwrap();let token=store.load_temporal(now).unwrap().unwrap().token;
    rt.cancel_temporal_durability(&rt.mutation_guard().unwrap()).unwrap();
    assert!(rt.start_temporal_phase(&rt.mutation_guard().unwrap(),now).is_err());
    assert!(rt.current_verified_frozen_model().ok().flatten().is_none());
    assert_eq!(store.load_temporal(now).unwrap().unwrap().token,token);drop(rt);
    let later=now+1_000_000_000;let intent=store.prepare_retire_temporal(&token,later,false).unwrap();
    let retired=store.publish_temporal(&intent,None,later).unwrap();assert_ne!(retired,token);
    assert!(store.load_temporal(later).unwrap().unwrap().prefix.is_none());
    let cold=fixture::runtime(d.path(),later);
    assert!(cold.restore_temporal_full_run(&cold.mutation_guard().unwrap(),&fixture::context(),later).is_err());
    assert!(cold.current_verified_frozen_model().ok().flatten().is_none());
    assert!(cold.inner.lock().unwrap().qualification.is_none());assert!(!cold.inner.lock().unwrap().selection.has_history());
}
fn find_exact_model(root:&Path,filename:&str)->Option<std::path::PathBuf>{
    for entry in std::fs::read_dir(root).unwrap(){let p=entry.unwrap().path();if p.is_dir(){if let Some(v)=find_exact_model(&p,filename){return Some(v)}}else if p.file_name().unwrap()==filename{return Some(p)}}None
}
#[test]
#[cfg_attr(debug_assertions, ignore = "requires Release: actual native full-run fixture and current-store fault matrix")]
fn c4b_actual_complete_cold_restore_requires_model_owner_context_and_original_deadline(){
    for case in ["missing_model","corrupt_model","wrong_owner","changed_context","expired","clock_regression"]{
        let d=tempfile::tempdir().unwrap();let mut now=complete_saved_copy(d.path());let rt=fixture::runtime(d.path(),now);
        let store=rt.temporal_store().unwrap();let loaded=store.load_temporal(now).unwrap().unwrap();let prefix=loaded.prefix.unwrap();let original=prefix.descriptor().clone();
        let value:serde_json::Value=serde_json::from_slice(prefix.bytes()).unwrap();let sha=value["payload"]["stage"]["fit"]["body"]["artifact_sha256"].as_str().unwrap();
        if case=="missing_model"{std::fs::rename(find_exact_model(d.path(),&format!("{sha}.json")).unwrap(),d.path().join("SYNTHETIC-missing-model.json")).unwrap();}
        if case=="corrupt_model"{std::fs::write(find_exact_model(d.path(),&format!("{sha}.json")).unwrap(),b"SYNTHETIC corrupt whole model").unwrap();}
        let mut context=fixture::context();if case=="wrong_owner"{context.owner_scope="c".repeat(64);}if case=="changed_context"{context.room_digest="d".repeat(64);}
        if case=="expired"{now=original.expires_at_ms*1_000_000;}if case=="clock_regression"{now=value["payload"]["common"]["last_now_ns"].as_u64().unwrap()-1;}
        let error=rt.restore_temporal_full_run(&rt.mutation_guard().unwrap(),&context,now).err().expect(case);
        println!("SYNTHETIC_C4B_COLD_DENIAL case={case} error={error}");
        assert!(rt.current_verified_frozen_model().ok().flatten().is_none());assert!(rt.inner.lock().unwrap().capture.as_ref().is_none_or(|c|matches!(c,CaptureWorkflow::Temporal(t) if t.core.is_none())));assert!(rt.inner.lock().unwrap().model.is_none());assert!(rt.inner.lock().unwrap().qualification.is_none());assert!(!rt.inner.lock().unwrap().selection.has_history());
        if case!="expired"&&case!="clock_regression"{assert_eq!(store.load_temporal(now).unwrap().unwrap().token,loaded.token);}
    }
}
