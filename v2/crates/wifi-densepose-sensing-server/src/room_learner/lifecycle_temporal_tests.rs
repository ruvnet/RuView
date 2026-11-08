//! SYNTHETIC raw native inputs through the real Runtime, simulated receiver clocks.
//! No real RF, owner credentials, helper process, selected model or fit authority.
use super::*;
use crate::room_learner::{
    native::{canonicalize_esp32, NativePacketInput},
    observation::{CaptureProvenance, SourceSpec},
    recovery_store::RecoveryStore,
};
use wifi_densepose_hardware::{Adr018Flags, PpduType};
const START: u64 = 1_000_000_000_000;
const PERIOD: u64 = 1_000_000_000u64.div_ceil(33);
// Test-only supervision for asynchronous storage/authorization ACKs, including
// final-row waits while ingress continues; not observation or shipping budgets.
const QUIESCENT_ACK_TEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
fn context() -> CaptureContext {
    CaptureContext {
        owner_scope: "a".repeat(64),
        installation_id: "synthetic-runtime-temporal".into(),
        room_digest: "b".repeat(64),
        revision: 1,
        sources: (1..=3)
            .map(|source_id| SourceSpec {
                source_id,
                device_id: format!("esp32-node-{source_id}"),
                grid_id: "ant1-bins64-ppdu0-usable-drop2-linear-v2".into(),
                feature_count: 56,
                signal_kind: ruview_unified::RfModality::WifiCsi,
                center_frequency_hz: 2_437_000_000,
                bandwidth_hz: 20_000_000,
                link_id: format!("native-{source_id}"),
                link_index: 0,
                geometry: None,
            })
            .collect(),
    }
}
fn runtime(root: &Path, now: u64) -> RoomLearningRuntime {
    let c = context();
    let directory = root.join(&c.owner_scope).join("SensingServer");
    let store = RecoveryStore::new(
        &directory,
        &c.owner_scope,
        &c.installation_id,
        Some(&[7; 32]),
    )
    .unwrap();
    RoomLearningRuntime::new_with_recovery_at(
        &directory,
        &c.owner_scope,
        &c.installation_id,
        store,
        now,
    )
    .unwrap()
}
fn configured(root: &Path) -> RoomLearningRuntime {
    let rt = runtime(root, START);
    // Original synchronous C1 reducer remains a reference/golden adapter.
    rt.configure_temporal_capture_with(
        &rt.mutation_guard().unwrap(), context(), "synthetic-runtime-train".into(), START,
        TemporalTrainCapture::prepare_retained,
    )
    .unwrap();
    rt
}
fn packet(source: u8, seq: u32, at: u64) -> CanonicalPacket {
    let iq = vec![8; 128];
    canonicalize_esp32(NativePacketInput {
        node_id: source,
        n_antennas: 1,
        raw_bins: 64,
        frequency_hz: 2_437_000_000,
        ppdu_type: PpduType::HtLegacy,
        sequence: seq,
        received_at_ns: at,
        receiver_sample_rate_hz: 33.,
        iq_bytes: &iq,
        flags: Adr018Flags::default(),
        provenance: CaptureProvenance::LiveReceiverObserved,
    })
    .unwrap()
}
fn snapshot(rt: &RoomLearningRuntime, now: u64) -> Value {
    rt.temporal_capture_snapshot(now).unwrap()["capture"].clone()
}
fn start(rt: &RoomLearningRuntime, at: u64) -> u64 {
    rt.start_temporal_phase(&rt.mutation_guard().unwrap(), at)
        .unwrap();
    at + TRAIN_LEAD_IN_NS
}
fn feed(rt: &RoomLearningRuntime, mut at: u64, to: u64, seq: &mut u32) -> u64 {
    while at <= to {
        *seq += 1;
        for source in 1..=3 {
            rt.ingest_native(&packet(source, *seq, at), at).unwrap();
        }
        at += PERIOD;
    }
    at
}
#[test]
fn private_temporal_setup_uses_real_native_runtime_and_public_configure_cannot_replace_it() {
    let dir = tempfile::tempdir().unwrap();
    let rt = configured(dir.path());
    assert_eq!(snapshot(&rt, START)["stage"], "ready");
    assert_eq!(rt.context().unwrap(), context());
    assert!(rt.selects_grid(1, &context().sources[0].grid_id));
    let before = snapshot(&rt, START);
    let guard = rt.mutation_guard().unwrap();
    assert!(rt
        .configure(&guard, context(), "other".into(), true)
        .unwrap_err()
        .contains("unsupported_command"));
    assert_eq!(snapshot(&rt, START)["rows_sha256"], before["rows_sha256"]);
    assert_eq!(
        rt.status(START)["error_code"],
        "learner_temporal_capture_private_unsupported_command"
    );
    assert!(rt.saved_candidates(&context().room_digest).is_err());
    assert!(rt.prediction(START).is_err());
    assert!(rt.qualify(&rt.mutation_guard().unwrap()).is_err());
    let first = start(&rt, START + 1);
    let mut seq = 0;
    feed(&rt, first, first + 3_000_000_000, &mut seq);
    assert!(
        snapshot(&rt, first + 3_000_000_000)["durable_windows"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(rt
        .temporal_confirmation_challenge(&rt.mutation_guard().unwrap())
        .is_err());
}
#[test]
#[cfg_attr(debug_assertions, ignore = "synchronous native-persistence reference requires Release; CI explicitly executes and counts this exact test")]
fn full_seven_train_phases_require_fresh_confirmation_and_never_issue_fit_authority() {
    let dir = tempfile::tempdir().unwrap();
    let rt = configured(dir.path());
    let mut seq = 0;
    let mut now = START + 1;
    let original = snapshot(&rt, START)["descriptor"].clone();
    for episode in 0..7 {
        let before = snapshot(&rt, now);
        assert_eq!(before["episode_index"], episode);
        let target = before["target_ms"].as_u64().unwrap() * 1_000_000;
        let first = start(&rt, now);
        now = feed(&rt, first, first + target + PERIOD, &mut seq);
        assert_eq!(snapshot(&rt, now)["stage"], "needs_confirmation");
        assert!(rt
            .confirm_temporal_phase(
                &rt.mutation_guard().unwrap(),
                "not-a-current-challenge",
                now
            )
            .is_err());
        assert_eq!(snapshot(&rt, now)["sealed_train_episodes"], episode);
        let expected = rt.mutation_guard().unwrap();
        let challenge = rt.temporal_confirmation_challenge(&expected).unwrap();
        rt.confirm_temporal_phase(&expected, &challenge, now)
            .unwrap();
        assert_eq!(snapshot(&rt, now)["sealed_train_episodes"], episode + 1);
        assert!(rt
            .confirm_temporal_phase(&expected, &challenge, now)
            .is_err());
        now += 1;
    }
    let done = snapshot(&rt, now);
    assert_eq!(done["stage"], "complete");
    assert_eq!(done["durable_windows"], 64);
    assert_eq!(done["descriptor"]["id"], original["id"]);
    assert_eq!(
        done["descriptor"]["expires_at_ms"],
        original["expires_at_ms"]
    );
    assert_eq!(done["fit_admission_available"], false);
    let rt = Arc::new(rt);
    assert!(rt
        .start_training(
            &rt.mutation_guard().unwrap(),
            FitConfig::for_source_count(3)
        )
        .is_err());
}
#[test]
fn a_missing_required_source_between_retained_strata_withdraws_runtime_quality_terminally() {
    let dir = tempfile::tempdir().unwrap();
    let rt = configured(dir.path());
    let mut seq = 0;
    let first = start(&rt, START + 1);
    let mut now = feed(&rt, first, first + 3_000_000_000, &mut seq);
    let prior = snapshot(&rt, now)["durable_windows"].clone();
    let mut failed = false;
    for _ in 0..5 {
        seq += 1;
        for source in 1..=2 {
            if rt.ingest_native(&packet(source, seq, now), now).is_err() {
                failed = true;
            }
        }
        now += PERIOD;
    }
    assert!(failed);
    assert_eq!(snapshot(&rt, now)["stage"], "failed");
    assert_eq!(snapshot(&rt, now)["durable_windows"], prior);
    assert!(rt
        .temporal_confirmation_challenge(&rt.mutation_guard().unwrap())
        .is_err());
    assert!(rt
        .start_temporal_phase(&rt.mutation_guard().unwrap(), now)
        .is_err());
}
#[test]
fn all_unsupported_legacy_commands_are_explicit_and_cannot_change_temporal_rows() {
    let dir = tempfile::tempdir().unwrap();
    let rt = Arc::new(configured(dir.path()));
    let before = snapshot(&rt, START);
    let request = || PhaseRequest {
        trial_id: "legacy-trial".into(),
        session_id: "legacy-session".into(),
        role: DataRole::Train,
        label: crate::room_learner::observation::RoomLabel::Empty,
        duration_ms: 600_000,
        lead_in_ms: 5_000,
        conditions_confirmed: true,
    };
    macro_rules! denied {
        ($call:expr) => {
            assert!($call.unwrap_err().contains("unsupported_command"));
        };
    }
    denied!(rt.begin_phase(&rt.mutation_guard().unwrap(), request(), START));
    denied!(rt.finish_phase(&rt.mutation_guard().unwrap(), START, true));
    denied!(rt.cancel_phase(&rt.mutation_guard().unwrap()));
    denied!(rt.cancel_training(&rt.mutation_guard().unwrap()));
    denied!(rt.start_training(
        &rt.mutation_guard().unwrap(),
        FitConfig::for_source_count(3)
    ));
    denied!(rt.evaluate(&rt.mutation_guard().unwrap(), DataRole::HeldOut));
    denied!(rt.qualify(&rt.mutation_guard().unwrap()));
    denied!(rt.select_model(
        &rt.mutation_guard().unwrap(),
        &"a".repeat(64),
        &"b".repeat(64)
    ));
    denied!(rt.deselect_model(&rt.mutation_guard().unwrap()));
    denied!(rt.rollback_model(&rt.mutation_guard().unwrap()));
    denied!(rt.restore_capture_recovery(
        &rt.mutation_guard().unwrap(),
        "old-id",
        &context(),
        START
    ));
    denied!(rt.discard_capture_recovery(&rt.mutation_guard().unwrap(), "old-id"));
    denied!(rt.saved_recovery_context("old-id", START));
    denied!(rt.enable_current_capture_recovery(
        &rt.mutation_guard().unwrap(),
        "old-id",
        true,
        START
    ));
    assert!(rt
        .paired_progress_snapshot(
            &context().installation_id,
            "public-runtime",
            None,
            &[],
            None,
            START
        )
        .unwrap_err()
        .contains("unsupported_command"));
    assert!(rt
        .selection_snapshot(START, Some(&context().room_digest))
        .integrity_error
        .unwrap()
        .contains("unsupported_command"));
    assert!(matches!(
        rt.selected_binary_evidence(START, Some(&context().room_digest)),
        crate::room_learner::selection::SelectedBinaryEvidence::Unavailable { .. }
    ));
    assert_eq!(snapshot(&rt, START)["rows_sha256"], before["rows_sha256"]);
    assert_eq!(snapshot(&rt, START)["descriptor"], before["descriptor"]);
    assert!(rt.inner.lock().unwrap().model.is_none());
}
#[test]
fn preparation_replay_phy_and_receiver_clock_cannot_rescue_or_relabel_a_capture() {
    for case in ["preparation_replay", "native_phy", "receiver_reverse"] {
        let dir = tempfile::tempdir().unwrap();
        let rt = configured(dir.path());
        let first = start(&rt, START + 1);
        if case == "preparation_replay" {
            rt.ingest_native(
                &packet(1, 100, START + 1_000_000_000),
                START + 1_000_000_000,
            )
            .unwrap();
            assert!(rt.ingest_native(&packet(1, 99, first), first).is_err());
        } else {
            let mut seq = 0;
            let now = feed(&rt, first, first + 3_000_000_000, &mut seq);
            if case == "native_phy" {
                let mut p = packet(1, seq + 1, now);
                p.native_frame.centre_frequency_hz += 1.;
                assert!(rt.ingest_native(&p, now).is_err());
            } else {
                assert!(rt
                    .tick_temporal_capture(&rt.mutation_guard().unwrap(), first)
                    .is_err());
            }
        }
        assert_eq!(
            snapshot(&rt, first + 4_000_000_000)["stage"],
            "failed",
            "{case}"
        );
        assert!(
            rt.restore_temporal_capture(
                &rt.mutation_guard().unwrap(),
                &context(),
                first + 5_000_000_000
            )
            .is_err(),
            "same process cannot rewind floors: {case}"
        );
    }
}
#[test]
fn actual_runtime_two_cold_resumes_keep_original_rows_trial_and_expiry_without_downtime_credit() {
    let dir = tempfile::tempdir().unwrap();
    let (mut rt,mut now) = configured_async(dir.path());
    let original = snapshot(&rt, now)["descriptor"].clone();
    let mut seq = 0;
    let mut trial = None;
    for _ in 0..2 {
        // Cold restore installs the actual asynchronous writer. Await the
        // real start authorization before choosing any observation times.
        let first = async_lead_end(&rt, now);
        rt.tick_temporal_capture(&rt.mutation_guard().unwrap(),first).unwrap();
        now = feed_async_paced(&rt, first, first + 100_000_000_000, &mut seq);
        let phase = rt.mutation_guard().unwrap().phase.unwrap();
        if let Some(prior) = &trial {
            assert_eq!(prior, &phase.trial_id);
        } else {
            trial = Some(phase.trial_id);
        }
        rt.pause_temporal_capture(&rt.mutation_guard().unwrap(), now)
            .unwrap();
        now=settle_async_boundary(&rt,now);
        let paused = snapshot(&rt, now);
        let accepted = paused["durable_observed_ns"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap();
        assert!(accepted < 600_000_000_000);
        let old_guard = rt.mutation_guard().unwrap();
        drop(rt);
        now += 50_000_000_000;
        rt = runtime(dir.path(), now);
        assert!(rt
            .restore_temporal_capture(&old_guard, &context(), now)
            .is_err());
        rt.restore_temporal_capture(&rt.mutation_guard().unwrap(), &context(), now)
            .unwrap();
        let restored = snapshot(&rt, now);
        assert_eq!(restored["stage"], "paused");
        assert_eq!(restored["rows_sha256"], paused["rows_sha256"]);
        assert_eq!(
            restored["durable_observed_ns"],
            paused["durable_observed_ns"]
        );
        assert_eq!(restored["descriptor"]["id"], original["id"]);
        assert_eq!(
            restored["descriptor"]["expires_at_ms"],
            original["expires_at_ms"]
        );
        assert!(rt
            .temporal_confirmation_challenge(&rt.mutation_guard().unwrap())
            .is_err());
        now += 1;
    }
    let accepted = snapshot(&rt, now)["durable_observed_ns"]
        .as_str()
        .unwrap()
        .parse::<u64>()
        .unwrap();
    let first = async_lead_end(&rt, now);
    rt.tick_temporal_capture(&rt.mutation_guard().unwrap(),first).unwrap();
    now = feed_async_paced(
        &rt,
        first,
        first + 600_000_000_000 - accepted + PERIOD,
        &mut seq,
    );
    // Keep the declared receiver stream alive while the final complete
    // reservation awaits its actual ACK. Do not treat submission as confirmed.
    let waiting=std::time::Instant::now();
    while snapshot(&rt,now)["stage"]!="needs_confirmation" {
        now=feed_async_paced(&rt,now,now,&mut seq);
        assert!(waiting.elapsed()<QUIESCENT_ACK_TEST_TIMEOUT,"final row ACK did not settle");
    }
    assert_eq!(snapshot(&rt, now)["stage"], "needs_confirmation");
    let guard = rt.mutation_guard().unwrap();
    let challenge = rt.temporal_confirmation_challenge(&guard).unwrap();
    rt.confirm_temporal_phase(&guard, &challenge, now).unwrap();
    now=settle_async_boundary(&rt,now);
    assert_eq!(snapshot(&rt, now)["sealed_train_episodes"], 1);
    assert_eq!(snapshot(&rt, now)["durable_windows"], 16);
}
#[test]
fn temporal_private_setup_refuses_wrong_owner_stale_generation_and_existing_legacy_work() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path(), START);
    let mut wrong = context();
    wrong.owner_scope = "c".repeat(64);
    assert!(rt
        .configure_temporal_capture(
            &rt.mutation_guard().unwrap(),
            wrong,
            "wrong-owner".into(),
            START
        )
        .is_err());
    let stale = rt.mutation_guard().unwrap();
    rt.configure(&stale, context(), "legacy-saved-work".into(), true)
        .unwrap();
    let before = rt.status(START);
    assert!(rt
        .configure_temporal_capture(&stale, context(), "stale".into(), START)
        .is_err());
    assert!(rt
        .configure_temporal_capture(
            &rt.mutation_guard().unwrap(),
            context(),
            "would-replace".into(),
            START
        )
        .is_err());
    let after = rt.status(START);
    assert_eq!(before["phase_history"], after["phase_history"]);
    assert_eq!(before["capture"], after["capture"]);
    assert_eq!(before["model_selection"], after["model_selection"]);
}

#[test]
fn real_initial_publish_with_lost_ack_stays_blocked_until_exact_runtime_repair() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path(), START);
    assert!(rt
        .configure_temporal_capture_with(
            &rt.mutation_guard().unwrap(),
            context(),
            "synthetic-lost-ack".into(),
            START,
            TemporalTrainCapture::synthetic_prepare_lost_initial_ack
        )
        .is_err());
    let blocked = snapshot(&rt, START);
    assert_eq!(blocked["stage"], "blocked");
    assert_eq!(blocked["durable_windows"], 0);
    assert!(rt
        .start_temporal_phase(&rt.mutation_guard().unwrap(), START + 1)
        .is_err());
    assert!(rt
        .configure(
            &rt.mutation_guard().unwrap(),
            context(),
            "cannot-replace".into(),
            true
        )
        .is_err());
    rt.repair_temporal_capture(&rt.mutation_guard().unwrap(), START + 2)
        .unwrap();
    let repaired = snapshot(&rt, START + 2);
    assert_eq!(repaired["stage"], "paused");
    assert_eq!(repaired["descriptor"]["id"], blocked["descriptor"]["id"]);
    assert_eq!(
        repaired["descriptor"]["expires_at_ms"],
        blocked["descriptor"]["expires_at_ms"]
    );
    let first = start(&rt, START + 3);
    let mut seq = 0;
    feed(&rt, first, first + 3_000_000_000, &mut seq);
    assert_eq!(snapshot(&rt, first + 3_000_000_000)["durable_windows"], 1);
}
#[tokio::test]
async fn private_temporal_setup_cannot_replace_an_actual_selected_legacy_candidate_or_history() {
    let dir = tempfile::tempdir().unwrap();
    let rt = super::super::selection_tests::fitted(dir.path()).await;
    let q = rt.qualify(&rt.mutation_guard().unwrap()).unwrap();
    rt.select_model(
        &rt.mutation_guard().unwrap(),
        &q.receipt.model_sha256,
        &q.receipt_sha256,
    )
    .unwrap();
    let selected_before = serde_json::to_vec(&rt.store.load_selection().unwrap()).unwrap();
    let model_before = rt
        .inner
        .lock()
        .unwrap()
        .model
        .as_ref()
        .unwrap()
        .export()
        .unwrap();
    let history_before =
        serde_json::to_vec(rt.inner.lock().unwrap().legacy_capture().unwrap().history()).unwrap();
    let mut proposed = context();
    proposed.installation_id = rt.installation.clone();
    assert_eq!(
        rt.configure_temporal_capture(
            &rt.mutation_guard().unwrap(),
            proposed,
            "would-replace-control".into(),
            rt.receiver_now_ns()
        )
        .unwrap_err(),
        "temporal_setup_would_replace_existing_work"
    );
    assert_eq!(
        serde_json::to_vec(&rt.store.load_selection().unwrap()).unwrap(),
        selected_before
    );
    assert_eq!(
        rt.inner
            .lock()
            .unwrap()
            .model
            .as_ref()
            .unwrap()
            .export()
            .unwrap(),
        model_before
    );
    assert_eq!(
        serde_json::to_vec(rt.inner.lock().unwrap().legacy_capture().unwrap().history()).unwrap(),
        history_before
    );
}

#[test]
fn temporal_storage_barrier_must_not_block_real_runtime_native_ingress() {
    use std::{sync::{Arc, mpsc}, time::Duration};
    let dir = tempfile::tempdir().unwrap();
    let rt = Arc::new(runtime(dir.path(), START));
    let guard = rt.mutation_guard().unwrap();
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let writer_rt = rt.clone();
    let writer = std::thread::spawn(move || {
        writer_rt.configure_temporal_capture_with(&guard, context(),
            "synthetic-c2-stalled-storage".into(), START,
            |schedule, run, fit, now, store| {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                TemporalTrainCapture::prepare_retained(schedule, run, fit, now, store)
            })
    });
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let (input_tx, input_rx) = mpsc::sync_channel(1);
    let input_rt = rt.clone();
    let input = std::thread::spawn(move || {
        input_tx.send(input_rt.ingest_native(&packet(1, 1, START), START)).unwrap();
    });
    let progressed_while_storage_blocked = input_rx.recv_timeout(Duration::from_millis(100));
    release_tx.send(()).unwrap();
    writer.join().unwrap().unwrap();
    input.join().unwrap();
    assert!(progressed_while_storage_blocked.is_ok(),
        "real native ingress waited for a storage barrier held by Runtime.inner");
}

fn feed_async_paced(rt:&RoomLearningRuntime,mut at:u64,to:u64,seq:&mut u32)->u64{
    // Same accelerated receiver cadence as the existing synthetic journey;
    // give actual asynchronous transactions time while pending. No source
    // timestamp backdating or production quality limits are changed.
    while at<=to {
        let pending=snapshot(rt,at)["durability_pending"]==true;
        *seq+=1;
        for source in 1..=3{rt.ingest_native(&packet(source,*seq,at),at).unwrap();}
        at+=PERIOD;
        if pending{std::thread::sleep(std::time::Duration::from_nanos(PERIOD));}
    }
    at
}
fn settle_async_boundary(rt: &RoomLearningRuntime, after: u64) -> u64 {
    let started=std::time::Instant::now();
    loop {
        let at=after+started.elapsed().as_nanos() as u64;
        rt.tick_temporal_capture(&rt.mutation_guard().unwrap(),at).unwrap();
        if snapshot(rt,at)["durability_pending"]==false {
            let admitted={let state=rt.inner.lock().unwrap();let Some(CaptureWorkflow::Temporal(c))=&state.capture else{panic!()};at.max(c.last_now_ns)};
            if started.elapsed()>std::time::Duration::from_secs(1){println!("SYNTHETIC_DURABLE_ACK_WAIT_NS={} ROWS={} SEALED={}",started.elapsed().as_nanos(),snapshot(rt,admitted)["durable_windows"],snapshot(rt,admitted)["sealed_train_episodes"]);}
            return admitted;
        }
        assert!(started.elapsed()<QUIESCENT_ACK_TEST_TIMEOUT,"async boundary did not settle: stage={} rows={} seals={}",snapshot(rt,at)["stage"],snapshot(rt,at)["retained_rows"],snapshot(rt,at)["sealed_train_episodes"]);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}
fn configured_async(root:&Path)->(RoomLearningRuntime,u64){
    let rt=runtime(root,START);
    rt.configure_temporal_capture(&rt.mutation_guard().unwrap(),context(),"synthetic-c2-runtime".into(),START).unwrap();
    let at=settle_async_boundary(&rt,START);
    assert_eq!(snapshot(&rt,at)["stage"],"ready");
    (rt,at)
}

#[test]
fn c2_start_requires_exact_current_prefix_ack_before_any_lead_or_rows(){
    use std::{sync::mpsc,time::Duration};
    let dir=tempfile::tempdir().unwrap();let(rt,at)=configured_async(dir.path());
    let writer={let state=rt.inner.lock().unwrap();let Some(CaptureWorkflow::Temporal(capture))=&state.capture else{panic!()};capture.core.as_ref().unwrap().writer().unwrap()};
    let(entered_tx,entered_rx)=mpsc::sync_channel(1);let(release_tx,release_rx)=mpsc::sync_channel(1);
    writer.hold_next(entered_tx,release_rx);
    rt.start_temporal_phase(&rt.mutation_guard().unwrap(),at).unwrap();
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let mut now=at+1_000_000;
    for source in 1..=3 {rt.ingest_native(&packet(source,1,now),now).unwrap();}
    let pending=snapshot(&rt,now);
    assert_eq!(pending["stage"],"preparing");assert_eq!(pending["retained_rows"],0);
    assert_eq!(pending["durable_windows"],0);assert_eq!(pending["lead_in_remaining_ms"],0);
    assert!(rt.temporal_confirmation_challenge(&rt.mutation_guard().unwrap()).is_err());
    release_tx.send(()).unwrap();
    // Exact recertification chooses a fresh start coordinate only at admission.
    let started=std::time::Instant::now();
    loop {
        now=now.max(at+started.elapsed().as_nanos() as u64);
        rt.tick_temporal_capture(&rt.mutation_guard().unwrap(),now).unwrap();
        if snapshot(&rt,now)["lead_in_remaining_ms"].as_u64().unwrap()>0 {break;}
        assert!(started.elapsed()<Duration::from_secs(5));std::thread::sleep(Duration::from_millis(1));
    }
    let state=rt.inner.lock().unwrap();let Some(CaptureWorkflow::Temporal(c))=&state.capture else{panic!()};
    assert!(c.lead_in_until.unwrap()>now);
    drop(state);
    let durable=rt.temporal_store().unwrap().load_temporal(now).unwrap().unwrap();
    let bytes=durable.prefix.unwrap();let v:Value=serde_json::from_slice(bytes.bytes()).unwrap();
    assert_eq!(v["payload"]["rows"].as_array().unwrap().len(),0);
    assert!(v["payload"]["mapping"].is_null());
    rt.cancel_temporal_durability(&rt.mutation_guard().unwrap()).unwrap();
}

#[test]
fn c2_real_native_ingress_and_source_floors_progress_while_segment_writer_is_blocked(){
    use std::{sync::{Arc,mpsc},time::Duration};
    let dir=tempfile::tempdir().unwrap();let(rt,at)=configured_async(dir.path());let rt=Arc::new(rt);
    rt.start_temporal_phase(&rt.mutation_guard().unwrap(),at).unwrap();
    let started=std::time::Instant::now();let end;
    loop {
        let now=at+started.elapsed().as_nanos() as u64;
        rt.tick_temporal_capture(&rt.mutation_guard().unwrap(),now).unwrap();
        let state=rt.inner.lock().unwrap();let Some(CaptureWorkflow::Temporal(c))=&state.capture else{panic!()};
        if let Some(value)=c.lead_in_until {end=value;break;}
        drop(state);assert!(started.elapsed()<Duration::from_secs(5));std::thread::sleep(Duration::from_millis(1));
    }
    let writer={let state=rt.inner.lock().unwrap();let Some(CaptureWorkflow::Temporal(capture))=&state.capture else{panic!()};capture.core.as_ref().unwrap().writer().unwrap()};let(entered_tx,entered_rx)=mpsc::sync_channel(1);let(release_tx,release_rx)=mpsc::sync_channel(1);
    writer.hold_next(entered_tx,release_rx);
    rt.tick_temporal_capture(&rt.mutation_guard().unwrap(),end).unwrap();
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let (done_tx,done_rx)=mpsc::sync_channel(1);let receiver=rt.clone();
    let input=std::thread::spawn(move||{
        for source in 1..=3 {receiver.ingest_native(&packet(source,2,end),end).unwrap();}
        done_tx.send(()).unwrap();
    });
    let progressed=done_rx.recv_timeout(Duration::from_millis(100));
    let status=if progressed.is_ok(){Some(snapshot(&rt,end))}else{None};
    release_tx.send(()).unwrap();input.join().unwrap();
    assert!(progressed.is_ok(),"native receiver waited for segment storage");
    let status=status.unwrap();assert_eq!(status["stage"],"observing");
    assert_eq!(status["durable_windows"],0);assert_eq!(status["retained_rows"],0);
    assert_eq!(status["durability_pending"],true);
    rt.cancel_temporal_durability(&rt.mutation_guard().unwrap()).unwrap();
}

fn async_lead_end(rt:&RoomLearningRuntime,at:u64)->u64{
    rt.start_temporal_phase(&rt.mutation_guard().unwrap(),at).unwrap();
    let started=std::time::Instant::now();
    loop{
        let now=at+started.elapsed().as_nanos() as u64;
        rt.tick_temporal_capture(&rt.mutation_guard().unwrap(),now).unwrap();
        let state=rt.inner.lock().unwrap();let Some(CaptureWorkflow::Temporal(c))=&state.capture else{panic!()};
        if let Some(end)=c.lead_in_until{return end;}
        drop(state);assert!(started.elapsed()<QUIESCENT_ACK_TEST_TIMEOUT);std::thread::sleep(std::time::Duration::from_millis(1));
    }
}
#[test]
fn c2_missing_source_and_replay_while_writer_is_blocked_never_resurrect_capture(){
    use std::{sync::mpsc,time::Duration};
    for replay in [false,true]{
        let dir=tempfile::tempdir().unwrap();let(rt,at)=configured_async(dir.path());let end=async_lead_end(&rt,at);
        let writer={let state=rt.inner.lock().unwrap();let Some(CaptureWorkflow::Temporal(c))=&state.capture else{panic!()};c.core.as_ref().unwrap().writer().unwrap()};
        let(entered_tx,entered_rx)=mpsc::sync_channel(1);let(release_tx,release_rx)=mpsc::sync_channel(1);writer.hold_next(entered_tx,release_rx);
        rt.tick_temporal_capture(&rt.mutation_guard().unwrap(),end).unwrap();entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        for source in 1..=3{rt.ingest_native(&packet(source,2,end),end).unwrap();}
        let now=end+if replay{PERIOD}else{60_000_000};
        assert!(rt.ingest_native(&packet(1,if replay{2}else{3},now),now).is_err());
        release_tx.send(()).unwrap();
        rt.cancel_temporal_durability(&rt.mutation_guard().unwrap()).unwrap();
        let status=snapshot(&rt,now);assert_eq!(status["stage"],"failed");assert_eq!(status["durable_windows"],0);
        assert!(rt.temporal_confirmation_challenge(&rt.mutation_guard().unwrap()).is_err());
    }
}
#[test]
fn c2_stale_owner_generation_cannot_admit_start_ticket(){
    use std::{sync::mpsc,time::Duration};
    let dir=tempfile::tempdir().unwrap();let(rt,at)=configured_async(dir.path());
    let writer={let state=rt.inner.lock().unwrap();let Some(CaptureWorkflow::Temporal(c))=&state.capture else{panic!()};c.core.as_ref().unwrap().writer().unwrap()};
    let(entered_tx,entered_rx)=mpsc::sync_channel(1);let(release_tx,release_rx)=mpsc::sync_channel(1);writer.hold_next(entered_tx,release_rx);
    rt.start_temporal_phase(&rt.mutation_guard().unwrap(),at).unwrap();entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    rt.inner.lock().unwrap().generation+=1;
    release_tx.send(()).unwrap();
    assert!(rt.tick_temporal_capture(&rt.mutation_guard().unwrap(),at+PERIOD).is_err());
    assert!(rt.temporal_capture_snapshot(at+PERIOD).is_err());
    rt.cancel_temporal_durability(&rt.mutation_guard().unwrap()).unwrap();
}
#[test]
fn c2_cancel_drain_waits_for_real_writer_outside_receiver_lock_before_synthetic_erase(){
    use std::{sync::{Arc,mpsc},time::Duration};
    let dir=tempfile::tempdir().unwrap();let(rt,at)=configured_async(dir.path());let rt=Arc::new(rt);let end=async_lead_end(&rt,at);
    let writer={let state=rt.inner.lock().unwrap();let Some(CaptureWorkflow::Temporal(c))=&state.capture else{panic!()};c.core.as_ref().unwrap().writer().unwrap()};
    let(entered_tx,entered_rx)=mpsc::sync_channel(1);let(release_tx,release_rx)=mpsc::sync_channel(1);writer.hold_next(entered_tx,release_rx);
    rt.tick_temporal_capture(&rt.mutation_guard().unwrap(),end).unwrap();entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let guard=rt.mutation_guard().unwrap();let cancelled=rt.clone();let(done_tx,done_rx)=mpsc::sync_channel(1);
    let cancellation=std::thread::spawn(move||{done_tx.send(cancelled.cancel_temporal_durability(&guard)).unwrap();});
    assert!(done_rx.recv_timeout(Duration::from_millis(50)).is_err(),"drain acknowledged an active writer");
    // Cancellation is waiting on the worker, but never holds Runtime.inner.
    let (input_tx,input_rx)=mpsc::sync_channel(1);let receiver=rt.clone();
    let input=std::thread::spawn(move||{input_tx.send(receiver.ingest_native(&packet(1,2,end+PERIOD),end+PERIOD)).unwrap();});
    let input_returned=input_rx.recv_timeout(Duration::from_millis(100));
    release_tx.send(()).unwrap();cancellation.join().unwrap();input.join().unwrap();
    assert!(input_returned.unwrap().is_err());done_rx.recv_timeout(Duration::from_secs(2)).unwrap().unwrap();
    let profile=dir.path().join(&context().owner_scope).join("SensingServer");
    std::fs::remove_dir_all(&profile).unwrap();std::thread::sleep(Duration::from_millis(20));assert!(!profile.exists());
}
#[test]
fn c2_real_publish_lost_ack_repairs_same_intent_to_paused_without_new_run_or_deadline(){
    let dir=tempfile::tempdir().unwrap();let(rt,at)=configured_async(dir.path());let end=async_lead_end(&rt,at);
    let original=snapshot(&rt,at)["descriptor"].clone();
    let writer={let state=rt.inner.lock().unwrap();let Some(CaptureWorkflow::Temporal(c))=&state.capture else{panic!()};c.core.as_ref().unwrap().writer().unwrap()};writer.lose_next_ack();
    rt.tick_temporal_capture(&rt.mutation_guard().unwrap(),end).unwrap();
    let began=std::time::Instant::now();let mut now=end;let mut seq=0;
    'lost: loop{
        seq+=1;
        for source in 1..=3 {match rt.ingest_native(&packet(source,seq,now),now){Err(e) if e=="synthetic_lost_publish_ack"=>break 'lost,Err(e)=>panic!("unexpected {e}"),Ok(())=>{}}}
        assert!(began.elapsed()<std::time::Duration::from_secs(5));std::thread::sleep(std::time::Duration::from_nanos(PERIOD));now+=PERIOD;
    }
    assert_eq!(snapshot(&rt,now)["stage"],"blocked");assert_eq!(snapshot(&rt,now)["durable_windows"],0);
    let real=rt.temporal_store().unwrap().load_temporal(now).unwrap().unwrap();
    let disk:Value=serde_json::from_slice(real.prefix.unwrap().bytes()).unwrap();assert!(disk["payload"]["mapping"].is_object());
    rt.repair_temporal_capture(&rt.mutation_guard().unwrap(),now).unwrap();
    let now=settle_async_boundary(&rt,now);let recovered=snapshot(&rt,now);
    assert_eq!(recovered["stage"],"paused");assert_eq!(recovered["retained_rows"],0);
    assert_eq!(recovered["descriptor"]["id"],original["id"]);assert_eq!(recovered["descriptor"]["expires_at_ms"],original["expires_at_ms"]);
    assert!(rt.temporal_confirmation_challenge(&rt.mutation_guard().unwrap()).is_err());
    rt.cancel_temporal_durability(&rt.mutation_guard().unwrap()).unwrap();
}

fn settle_observing_with_sources(rt:&RoomLearningRuntime,after:u64,seq:&mut u32)->u64{
    let wait=std::time::Instant::now();let mut now=after;
    loop {
        *seq+=1;
        for source in 1..=3 {rt.ingest_native(&packet(source,*seq,now),now).unwrap();}
        if snapshot(rt,now)["durability_pending"]==false {return now+PERIOD;}
        assert!(wait.elapsed()<std::time::Duration::from_secs(5));
        now+=PERIOD;std::thread::sleep(std::time::Duration::from_nanos(PERIOD));
    }
}
#[test]
fn c2_first_native_window_crash_boundaries_restore_only_real_committed_prefix(){
    use std::{sync::mpsc,time::Duration};
    for published_before_stop in [false,true]{
        let dir=tempfile::tempdir().unwrap();let(rt,at)=configured_async(dir.path());let end=async_lead_end(&rt,at);
        rt.tick_temporal_capture(&rt.mutation_guard().unwrap(),end).unwrap();
        let mut seq=0;
        let observed=settle_observing_with_sources(&rt,end,&mut seq);
        let original=snapshot(&rt,observed)["descriptor"].clone();
        let writer={let state=rt.inner.lock().unwrap();let Some(CaptureWorkflow::Temporal(c))=&state.capture else{panic!()};c.core.as_ref().unwrap().writer().unwrap()};
        let(entered_tx,entered_rx)=mpsc::sync_channel(1);let(release_tx,release_rx)=mpsc::sync_channel(1);
        if published_before_stop{writer.lose_next_ack();}else{writer.hold_next(entered_tx,release_rx);}
        let finish=feed(&rt,observed,end+38_000_000_000,&mut seq);
        let status=snapshot(&rt,finish);assert_eq!(status["retained_rows"],1);assert_eq!(status["durable_windows"],0);
        assert!(rt.temporal_confirmation_challenge(&rt.mutation_guard().unwrap()).is_err());
        if published_before_stop {
            let wait=std::time::Instant::now();
            loop{
                let durable=rt.temporal_store().unwrap().load_temporal(finish).unwrap().unwrap();
                let v:Value=serde_json::from_slice(durable.prefix.unwrap().bytes()).unwrap();
                if v["payload"]["rows"].as_array().unwrap().len()==1{break;}
                assert!(wait.elapsed()<Duration::from_secs(5));std::thread::sleep(Duration::from_millis(1));
            }
        }else{
            entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            // Stop before the worker can publish its immutable first row.
            writer.cancel();release_tx.send(()).unwrap();
        }
        rt.cancel_temporal_durability(&rt.mutation_guard().unwrap()).unwrap();drop(rt);drop(writer);
        let resumed=runtime(dir.path(),finish+1_000_000_000);
        resumed.restore_temporal_capture(&resumed.mutation_guard().unwrap(),&context(),finish+1_000_000_000).unwrap();
        let restored=snapshot(&resumed,finish+1_000_000_000);
        assert_eq!(restored["stage"],"paused");
        assert_eq!(restored["retained_rows"],if published_before_stop{1}else{0});
        assert_eq!(restored["descriptor"]["id"],original["id"]);
        assert_eq!(restored["descriptor"]["expires_at_ms"],original["expires_at_ms"]);
        assert_eq!(restored["lead_in_remaining_ms"],0);
        assert!(resumed.temporal_confirmation_challenge(&resumed.mutation_guard().unwrap()).is_err());
        resumed.cancel_temporal_durability(&resumed.mutation_guard().unwrap()).unwrap();
    }
}

#[test]
fn c2_storage_busy_pauses_at_next_retained_target_with_one_completed_window_and_no_fifo(){
    use std::{sync::mpsc,time::Duration};
    let dir=tempfile::tempdir().unwrap();let(rt,at)=configured_async(dir.path());let rt=std::sync::Arc::new(rt);let reclamation=install_reclaim_probe(&rt);let end=async_lead_end(&rt,at);
    rt.tick_temporal_capture(&rt.mutation_guard().unwrap(),end).unwrap();let mut seq=0;
    let observed=settle_observing_with_sources(&rt,end,&mut seq);
    let writer={let state=rt.inner.lock().unwrap();let Some(CaptureWorkflow::Temporal(c))=&state.capture else{panic!()};c.core.as_ref().unwrap().writer().unwrap()};
    let(entered_tx,entered_rx)=mpsc::sync_channel(1);let(release_tx,release_rx)=mpsc::sync_channel(1);writer.hold_next(entered_tx,release_rx);
    let mut now=observed;let busy;
    'observe:loop{
        seq+=1;
        for source in 1..=3{
            match rt.ingest_native(&packet(source,seq,now),now){
                Ok(())=>{},Err(error)if error=="temporal_storage_busy_pause"=>{busy=now;break 'observe;},Err(error)=>panic!("unexpected {error}"),
            }
        }
        assert!(now<end+120_000_000_000);now+=PERIOD;
    }
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let status=snapshot(&rt,busy);assert_eq!(status["stage"],"blocked");
    assert_eq!(status["retained_rows"],2);assert_eq!(status["durable_windows"],0);
    assert!(rt.temporal_confirmation_challenge(&rt.mutation_guard().unwrap()).is_err());
    release_tx.send(()).unwrap();
    let settled=settle_async_boundary(&rt,busy);
    let status=snapshot(&rt,settled);assert_eq!(status["stage"],"paused");
    assert_eq!(status["retained_rows"],1);assert_eq!(status["durable_windows"],1);
    assert_eq!(status["lead_in_remaining_ms"],0);
    assert!(reclamation.raw_tail.load(Ordering::Acquire)>0,"busy restore did not retire the actual unconfirmed raw tail");
    assert_eq!(reclamation.blocked.load(Ordering::Acquire),0,"busy raw-tail destructor ran under Runtime.inner");
    rt.cancel_temporal_durability(&rt.mutation_guard().unwrap()).unwrap();
}

#[test]
fn c2_three_sources_keep_declared_real_cadence_during_250ms_worker_stall(){
    use std::{sync::mpsc,time::{Duration,Instant}};
    let dir=tempfile::tempdir().unwrap();let rt=runtime(dir.path(),START);let mut c=context();
    let template=c.sources[0].clone();
    c.sources=(1..=3).map(|id|{let mut source=template.clone();source.source_id=id;source.device_id=format!("esp32-node-{id}");source.link_id=format!("native-{id}");source}).collect();
    rt.configure_temporal_capture(&rt.mutation_guard().unwrap(),c,"synthetic-c2-three".into(),START).unwrap();
    let at=settle_async_boundary(&rt,START);let end=async_lead_end(&rt,at);
    let writer={let state=rt.inner.lock().unwrap();let Some(CaptureWorkflow::Temporal(c))=&state.capture else{panic!()};c.core.as_ref().unwrap().writer().unwrap()};
    let(entered_tx,entered_rx)=mpsc::sync_channel(1);let(release_tx,release_rx)=mpsc::sync_channel(1);writer.hold_next(entered_tx,release_rx);
    rt.tick_temporal_capture(&rt.mutation_guard().unwrap(),end).unwrap();entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let clock=Instant::now();let mut max_ingest=Duration::ZERO;let mut last=end;
    let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{
        let mut previous_batch=None;
        for index in 0..9u32 {
            if let Some(previous)=previous_batch {
                let elapsed=Instant::now().duration_since(previous);
                if let Some(wait)=Duration::from_nanos(PERIOD).checked_sub(elapsed){std::thread::sleep(wait);}
            }
            let arrival=Instant::now();previous_batch=Some(arrival);
            // Three sources arrive together; record actual batch arrival once.
            // Processing uses its own current clock. Never catch up missed
            // deadlines by generating an artificially fast next batch.
            let received=end+clock.elapsed().as_nanos() as u64;
            for source in 1..=3 {
                let packet=packet(source,index+1,received);
                let now=end+clock.elapsed().as_nanos() as u64;let started=Instant::now();
                if let Err(error)=rt.ingest_native(&packet,now){panic!("paced index={index} source={source} max_ingest_ns={} error={error}",max_ingest.as_nanos());}
                max_ingest=max_ingest.max(started.elapsed());last=now;
            }
        }
        assert_eq!(snapshot(&rt,last)["stage"],"observing");
        assert_eq!(snapshot(&rt,last)["durable_windows"],0);
    }));
    release_tx.send(()).unwrap();rt.cancel_temporal_durability(&rt.mutation_guard().unwrap()).unwrap();
    if let Err(error)=result{std::panic::resume_unwind(error);}
    println!("SYNTHETIC_3_SOURCE_PACED_WALL_NS={} MAX_INGEST_NS={}",clock.elapsed().as_nanos(),max_ingest.as_nanos());
    assert!(max_ingest<Duration::from_millis(35),"receiver consumed a full source gap while storage stalled");
}

#[test]
fn c2_eight_source_full_schedule_preserves_existing_numeric_preflight_limit(){
    let dir=tempfile::tempdir().unwrap();let rt=runtime(dir.path(),START);let mut c=context();let template=c.sources[0].clone();
    c.sources=(1..=8).map(|id|{let mut source=template.clone();source.source_id=id;source.device_id=format!("esp32-node-{id}");source.link_id=format!("native-{id}");source}).collect();
    assert_eq!(rt.configure_temporal_capture(&rt.mutation_guard().unwrap(),c,"synthetic-c2-budget".into(),START).unwrap_err(),"acquisition_numeric_capacity");
    assert!(rt.context().is_none());
    assert!(rt.temporal_capture_snapshot(START).is_err());
}
#[test]
fn c2_reverse_clock_and_expiry_while_start_ack_pending_never_authorize_observation(){
    use std::{sync::mpsc,time::Duration};
    for expires in [false,true]{
        let dir=tempfile::tempdir().unwrap();let(rt,at)=configured_async(dir.path());
        let writer={let state=rt.inner.lock().unwrap();let Some(CaptureWorkflow::Temporal(c))=&state.capture else{panic!()};c.core.as_ref().unwrap().writer().unwrap()};
        let(entered_tx,entered_rx)=mpsc::sync_channel(1);let(release_tx,release_rx)=mpsc::sync_channel(1);writer.hold_next(entered_tx,release_rx);
        rt.start_temporal_phase(&rt.mutation_guard().unwrap(),at).unwrap();entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let bad_time=if expires{snapshot(&rt,at)["descriptor"]["expires_at_ms"].as_u64().unwrap()*1_000_000}else{at-1};
        assert!(rt.tick_temporal_capture(&rt.mutation_guard().unwrap(),bad_time).is_err());
        release_tx.send(()).unwrap();rt.cancel_temporal_durability(&rt.mutation_guard().unwrap()).unwrap();
        let status=snapshot(&rt,bad_time);assert_eq!(status["stage"],"failed");assert_eq!(status["retained_rows"],0);assert_eq!(status["lead_in_remaining_ms"],0);
    }
}

#[test]
#[cfg_attr(debug_assertions, ignore = "native-persistence journey requires Release; CI explicitly executes and counts this test")]
fn c2_all_seven_train_phases_use_actual_async_runtime_and_durable_seals_without_fit(){
    assert!(!cfg!(debug_assertions), "execute this native-persistence journey in the explicit Release batch");
    // Synthetic receiver time accelerates only when no write is pending. While
    // a writer is pending we pace packets, preserving the declared cadence and
    // allowing explicit busy/pause+fresh-start rather than inventing credit.
    let dir=tempfile::tempdir().unwrap();let(rt,mut now)=configured_async(dir.path());let mut seq=0;
    let original=snapshot(&rt,now)["descriptor"].clone();let mut fresh_starts=0usize;
    for phase in 0..7u64 {
        loop {
            let state=snapshot(&rt,now);
            match state["stage"].as_str().unwrap() {
                "ready"|"paused"=>{
                    now=async_lead_end(&rt,now);fresh_starts+=1;
                    rt.tick_temporal_capture(&rt.mutation_guard().unwrap(),now).unwrap();
                }
                "needs_confirmation"=>break,
                "failed"=>panic!("phase{phase} failed: {state}"),
                _=>{}
            }
            let pending=snapshot(&rt,now)["durability_pending"]==true;
            seq+=1;
            for source in 1..=3 {
                match rt.ingest_native(&packet(source,seq,now),now){
                    Ok(())=>{},Err(error) if error=="temporal_storage_busy_pause"=>break,
                    Err(error)=>panic!("phase{phase} input failed: {error}"),
                }
            }
            now+=PERIOD;
            if pending {std::thread::sleep(std::time::Duration::from_nanos(PERIOD));}
            assert!(fresh_starts<120,"bounded interruption budget exhausted");
        }
        let guard=rt.mutation_guard().unwrap();let challenge=rt.temporal_confirmation_challenge(&guard).unwrap();
        rt.confirm_temporal_phase(&guard,&challenge,now).unwrap();
        // Submission is not durable phase confirmation.
        assert_eq!(snapshot(&rt,now)["sealed_train_episodes"],phase);
        assert_eq!(snapshot(&rt,now)["durability_pending"],true);
        now=settle_async_boundary(&rt,now);
        assert_eq!(snapshot(&rt,now)["sealed_train_episodes"],phase+1);
        println!("SYNTHETIC_ASYNC_PHASE_SEALED={} ROWS={} FRESH_STARTS={fresh_starts}",phase+1,snapshot(&rt,now)["durable_windows"]);
    }
    let complete=snapshot(&rt,now);
    assert_eq!(complete["stage"],"complete");assert_eq!(complete["retained_rows"],64);assert_eq!(complete["durable_windows"],64);
    assert_eq!(complete["fit_admission_available"],false);
    assert_eq!(complete["descriptor"]["id"],original["id"]);assert_eq!(complete["descriptor"]["expires_at_ms"],original["expires_at_ms"]);
    let inner=rt.inner.lock().unwrap();assert!(inner.model.is_none());assert!(!inner.selection.has_history());drop(inner);
    println!("SYNTHETIC_ASYNC_TRAIN64_SEALS7_FRESH_STARTS={fresh_starts}");
    rt.cancel_temporal_durability(&rt.mutation_guard().unwrap()).unwrap();
}

#[derive(Default)]
struct ReclaimCounts {
    unlocked:std::sync::atomic::AtomicUsize,
    blocked:std::sync::atomic::AtomicUsize,
    final_prefix_owner:std::sync::atomic::AtomicUsize,
    raw_tail:std::sync::atomic::AtomicUsize,
}
fn install_reclaim_probe(rt:&std::sync::Arc<RoomLearningRuntime>)->std::sync::Arc<ReclaimCounts>{
    let counts=std::sync::Arc::new(ReclaimCounts::default());let observed=counts.clone();let weak=std::sync::Arc::downgrade(rt);
    *rt.reclamation_probe.lock().unwrap()=Some(std::sync::Arc::new(move|stats|{
        if !stats.has_ack{return;}
        // Never panic from a destructor, especially while testing unwinding.
        let unlocked=weak.upgrade().is_some_and(|rt|match rt.inner.try_lock(){Ok(_)=>true,Err(std::sync::TryLockError::Poisoned(_))=>true,Err(std::sync::TryLockError::WouldBlock)=>false});
        if unlocked{observed.unlocked.fetch_add(1,Ordering::AcqRel);}else{observed.blocked.fetch_add(1,Ordering::AcqRel);}
        if stats.old_prefix_bytes>0 && stats.old_prefix_last_owner{observed.final_prefix_owner.fetch_add(1,Ordering::AcqRel);}
        if stats.raw_tail_rows>0{observed.raw_tail.fetch_add(1,Ordering::AcqRel);}
    }));counts
}
#[test]
fn c2_actual_ack_retirement_unlocks_before_normal_rejection_and_unwind_destructors(){
    use std::{sync::{Arc,mpsc},time::{Duration,Instant}};
    for via_tick in [false,true]{for outcome in ["normal","rejected","unwind"]{
        let dir=tempfile::tempdir().unwrap();let(rt,at)=configured_async(dir.path());let rt=Arc::new(rt);let counts=install_reclaim_probe(&rt);
        let writer={let state=rt.inner.lock().unwrap();let Some(CaptureWorkflow::Temporal(c))=&state.capture else{panic!()};c.core.as_ref().unwrap().writer().unwrap()};
        let(entered_tx,entered_rx)=mpsc::sync_channel(1);let(release_tx,release_rx)=mpsc::sync_channel(1);writer.hold_next(entered_tx,release_rx);
        if outcome=="rejected"{writer.corrupt_next_ack_id();}
        rt.start_temporal_phase(&rt.mutation_guard().unwrap(),at).unwrap();entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();release_tx.send(()).unwrap();
        let wait=Instant::now();while !writer.ack_is_queued(){assert!(wait.elapsed()<Duration::from_secs(5));std::thread::sleep(Duration::from_millis(1));}
        if outcome=="unwind"{rt.panic_after_reclamation_ack.store(true,Ordering::Release);}
        let now=rt.receiver_now_ns();
        let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{
            if via_tick{rt.tick_temporal_capture(&rt.mutation_guard().unwrap(),now)}else{rt.ingest_native(&packet(1,1,now),now)}
        }));
        match outcome{"normal"=>assert!(result.unwrap().is_ok()),"rejected"=>assert!(result.unwrap().is_err()),_=>assert!(result.is_err())}
        assert!(counts.unlocked.load(Ordering::Acquire)+counts.blocked.load(Ordering::Acquire)>0,"destructor was never observed");
        assert_eq!(counts.blocked.load(Ordering::Acquire),0,"actual payload destructor ran while Runtime.inner was held");
        assert!(counts.unlocked.load(Ordering::Acquire)>0);
        if outcome!="rejected"{assert!(counts.final_prefix_owner.load(Ordering::Acquire)>0,"did not retire last old-prefix Arc owner");}
        writer.drain().unwrap();
    }}
}

#[test]
#[ignore = "opt-in host/profile throughput acceptance; normal C2 tests retain growing-prefix custody, busy-pause and full-seven correctness"]
fn c2_real_growing_prefix_three_source_paced_seventeenth_row_throughput_acceptance(){
    use std::time::{Duration,Instant};
    let dir=tempfile::tempdir().unwrap();let(rt,mut now)=configured_async(dir.path());let mut seq=0;
    // Prelude is synthetic receiver-clock collection, not a performance claim.
    // Populate the SAME real Runtime/assembler/store with all16 Empty windows.
    loop {
        match snapshot(&rt,now)["stage"].as_str().unwrap(){
            "ready"|"paused"=>{now=async_lead_end(&rt,now);rt.tick_temporal_capture(&rt.mutation_guard().unwrap(),now).unwrap();},
            "needs_confirmation"=>break,
            "failed"=>panic!("growing-prefix prelude failed"),_=>{}
        }
        let pending=snapshot(&rt,now)["durability_pending"]==true;seq+=1;
        for source in 1..=3{match rt.ingest_native(&packet(source,seq,now),now){Ok(())=>{},Err(e)if e=="temporal_storage_busy_pause"=>break,Err(e)=>panic!("prelude: {e}")}}
        now+=PERIOD;if pending{std::thread::sleep(Duration::from_nanos(PERIOD));}
    }
    let guard=rt.mutation_guard().unwrap();let challenge=rt.temporal_confirmation_challenge(&guard).unwrap();
    rt.confirm_temporal_phase(&guard,&challenge,now).unwrap();now=settle_async_boundary(&rt,now);
    assert_eq!(snapshot(&rt,now)["durable_windows"],16);
    let old_prefix=rt.temporal_store().unwrap().load_temporal(now).unwrap().unwrap().prefix.unwrap().bytes().len();
    let base=async_lead_end(&rt,now);rt.tick_temporal_capture(&rt.mutation_guard().unwrap(),base).unwrap();
    // Measure honest batch arrival and current processing clocks at real33Hz;
    // no accelerated ticks, held-writer hook, replay queue, or deadline catch-up.
    let clock=Instant::now();let mut previous_batch=None;let mut packet_wall=Vec::<u64>::new();
    let mut row_ready_at=None;let mut acked_at=None;let mut measured_now=base;
    let mut batch_arrivals=Vec::<u64>::new();let mut sleep_samples=Vec::<(u64,u64)>::new();
    let mut source_arrivals:[Vec<u64>;3]=std::array::from_fn(|_|Vec::new());
    let mut snapshot_costs=Vec::<u64>::new();let mut failed_attempt:Option<(u8,u32,String)>=None;
    let result:Result<(),String>=(||{
        loop {
            if clock.elapsed()>Duration::from_secs(20){return Err("real growing-prefix ACK did not arrive within measurement window".into());}
            if let Some(previous)=previous_batch{if let Some(wait)=Duration::from_nanos(PERIOD).checked_sub(Instant::now().duration_since(previous)){let slept=Instant::now();std::thread::sleep(wait);sleep_samples.push((wait.as_nanos()as u64,slept.elapsed().as_nanos()as u64));}}
            previous_batch=Some(Instant::now());let received=base+clock.elapsed().as_nanos()as u64;seq+=1;
            batch_arrivals.push(received-base);
            for source in 1..=3{
                let before=Instant::now();let p=packet(source,seq,received);measured_now=base+clock.elapsed().as_nanos()as u64;
                let outcome=rt.ingest_native(&p,measured_now);
                let packet_elapsed=before.elapsed().as_nanos()as u64;
                source_arrivals[usize::from(source)-1].push(p.native_frame.timestamp_ns-base);
                packet_wall.push(packet_elapsed);
                if let Err(error)=outcome{failed_attempt=Some((source,seq,error.clone()));return Err(error);}
            }
            let snapshot_started=Instant::now();let status=snapshot(&rt,measured_now);snapshot_costs.push(snapshot_started.elapsed().as_nanos()as u64);
            if status["retained_rows"].as_u64().unwrap()>=17 && row_ready_at.is_none(){row_ready_at=Some(clock.elapsed());}
            if status["durable_windows"].as_u64().unwrap()>=17 && acked_at.is_none(){acked_at=Some(clock.elapsed());}
            if let Some(acked)=acked_at{if clock.elapsed()-acked>=Duration::from_millis(250){break;}}
        }
        if packet_wall.is_empty(){return Err("no measured packets".into());}
        packet_wall.sort_unstable();let max=*packet_wall.last().unwrap();let p99=packet_wall[(packet_wall.len()-1)*99/100];
        if max>5_000_000 || p99>2_000_000{return Err(format!("packet wall budget exceeded: max_ns={max} p99_ns={p99}"));}
        Ok(())
    })();
    rt.cancel_temporal_durability(&rt.mutation_guard().unwrap()).unwrap();
    let new_prefix=rt.temporal_store().unwrap().load_temporal(measured_now).unwrap().unwrap().prefix.unwrap().bytes().len();
    packet_wall.sort_unstable();let max=packet_wall.last().copied().unwrap_or(0);let p99=packet_wall.get(packet_wall.len().saturating_sub(1)*99/100).copied().unwrap_or(0);
    println!("SYNTHETIC_REAL_3_SOURCE_GROWING_PREFIX status={} old_bytes={} new_bytes={} packets={} max_packet_wall_ns={} p99_packet_wall_ns={} row_ready_at_ns={:?} acked_at_ns={:?} row_ready_to_ack_ns={:?} debug_assertions={} error={:?}",if result.is_ok(){"PASS"}else{"UNQUALIFIED"},old_prefix,new_prefix,packet_wall.len(),max,p99,row_ready_at.map(|d|d.as_nanos()),acked_at.map(|d|d.as_nanos()),row_ready_at.zip(acked_at).map(|(q,a)|a.saturating_sub(q).as_nanos()),cfg!(debug_assertions),result.as_ref().err());
    let summaries=source_arrivals.iter().enumerate().map(|(index,times)|{
        let span=times.last().copied().unwrap_or(0).saturating_sub(times.first().copied().unwrap_or(0));
        let intervals=times.len().saturating_sub(1) as u128;
        let gaps=times.windows(2).map(|w|w[1]-w[0]).collect::<Vec<_>>();
        json!({"source":index+1,"attempts":times.len(),"first_relative_ns":times.first(),"last_relative_ns":times.last(),"span_ns":span,
            "actual_rate_millihz":if span>0{Some(intervals*1_000_000_000_000u128/u128::from(span))}else{None},
            "min_gap_ns":gaps.iter().min(),"max_gap_ns":gaps.iter().max()})
    }).collect::<Vec<_>>();
    let contract=ManagedScheduleContract::build(context(),LeadInPolicy::Train5HeldOut15EngineeringV1).unwrap();let policy=contract.native_window_policy();let slot=&contract.episodes()[1].slots[0];
    println!("SYNTHETIC_CADENCE_DIAGNOSTIC {}",json!({"batch_count":batch_arrivals.len(),"batch_first_relative_ns":batch_arrivals.first(),"batch_last_relative_ns":batch_arrivals.last(),
        "sources":summaries,"failed_attempt":failed_attempt,"sleep_count":sleep_samples.len(),"requested_sleep_min_ns":sleep_samples.iter().map(|x|x.0).min(),"requested_sleep_max_ns":sleep_samples.iter().map(|x|x.0).max(),
        "actual_sleep_min_ns":sleep_samples.iter().map(|x|x.1).min(),"actual_sleep_max_ns":sleep_samples.iter().map(|x|x.1).max(),
        "sleep_overshoot_max_ns":sleep_samples.iter().map(|x|x.1.saturating_sub(x.0)).max(),"snapshot_max_ns":snapshot_costs.iter().max(),
        "slot_target_ms":slot.target_offset_ms,"slot_deadline_ms":slot.deadline_offset_ms,"native_target_span_ms":policy.target_span_ms(),
        "source_min_rows":policy.source_policies()[0].min_rows,"source_max_gap_ns":policy.source_policies()[0].max_gap_ns,
        "rate_band_millihz":policy.source_policies()[0].receiver_cadence.rate_bounds_millihz()}));
    assert!(result.is_ok(),"growing-prefix performance is UNQUALIFIED; no public activation: {:?}",result.err());
}

#[path = "c2_precision_pacer_test_support.rs"]
mod c2_precision_pacer_test_support;

#[test]
#[ignore = "opt-in host/profile throughput acceptance; normal C2 tests retain growing-prefix custody, busy-pause and full-seven correctness"]
fn c2_real_growing_prefix_three_source_absolute_pacer_with_actual_input_precondition(){
    use std::time::{Duration,Instant};
    const SPIN_GUARD_NS:u64=3_000_000;
    let precondition=c2_precision_pacer_test_support::input_study(100,SPIN_GUARD_NS);
    println!("SYNTHETIC_INPUT_PRECONDITION {:?}",precondition);
    assert!(precondition.qualified,"INPUT_UNQUALIFIED before backend exercise; no retry or public activation");
    let dir=tempfile::tempdir().unwrap();let(rt,mut now)=configured_async(dir.path());let mut seq=0;
    // Prelude is synthetic receiver-clock collection, not a performance claim.
    // Populate the SAME real Runtime/assembler/store with all16 Empty windows.
    loop {
        match snapshot(&rt,now)["stage"].as_str().unwrap(){
            "ready"|"paused"=>{now=async_lead_end(&rt,now);rt.tick_temporal_capture(&rt.mutation_guard().unwrap(),now).unwrap();},
            "needs_confirmation"=>break,
            "failed"=>panic!("growing-prefix prelude failed"),_=>{}
        }
        let pending=snapshot(&rt,now)["durability_pending"]==true;seq+=1;
        for source in 1..=3{match rt.ingest_native(&packet(source,seq,now),now){Ok(())=>{},Err(e)if e=="temporal_storage_busy_pause"=>break,Err(e)=>panic!("prelude: {e}")}}
        now+=PERIOD;if pending{std::thread::sleep(Duration::from_nanos(PERIOD));}
    }
    let guard=rt.mutation_guard().unwrap();let challenge=rt.temporal_confirmation_challenge(&guard).unwrap();
    rt.confirm_temporal_phase(&guard,&challenge,now).unwrap();now=settle_async_boundary(&rt,now);
    assert_eq!(snapshot(&rt,now)["durable_windows"],16);
    let old_prefix=rt.temporal_store().unwrap().load_temporal(now).unwrap().unwrap().prefix.unwrap().bytes().len();
    let base=async_lead_end(&rt,now);rt.tick_temporal_capture(&rt.mutation_guard().unwrap(),base).unwrap();
    // Measure honest batch arrival and current processing clocks at real33Hz;
    // no accelerated ticks, held-writer hook, replay queue, or deadline catch-up.
    let clock=Instant::now();let mut absolute_index=0usize;let mut packet_wall=Vec::<u64>::new();
    let mut row_ready_at=None;let mut acked_at=None;let mut measured_now=base;
    let mut batch_arrivals=Vec::<u64>::new();
    let mut source_arrivals:[Vec<u64>;3]=std::array::from_fn(|_|Vec::new());
    let mut snapshot_costs=Vec::<u64>::new();let mut failed_attempt:Option<(u8,u32,String)>=None;
    let result:Result<(),String>=(||{
        loop {
            if clock.elapsed()>Duration::from_secs(20){return Err("real growing-prefix ACK did not arrive within measurement window".into());}
            let actual_relative=c2_precision_pacer_test_support::next_actual_receipt(clock,absolute_index,SPIN_GUARD_NS)?;
            absolute_index+=1;
            // ACTUAL wakeup, never nominal deadline. One batch only; an entire
            // missed period returns UNQUALIFIED instead of catch-up emission.
            let received=base+actual_relative;seq+=1;
            batch_arrivals.push(received-base);
            for source in 1..=3{
                let before=Instant::now();let p=packet(source,seq,received);measured_now=base+clock.elapsed().as_nanos()as u64;
                let outcome=rt.ingest_native(&p,measured_now);
                let packet_elapsed=before.elapsed().as_nanos()as u64;
                source_arrivals[usize::from(source)-1].push(p.native_frame.timestamp_ns-base);
                packet_wall.push(packet_elapsed);
                if let Err(error)=outcome{failed_attempt=Some((source,seq,error.clone()));return Err(error);}
            }
            let snapshot_started=Instant::now();let status=snapshot(&rt,measured_now);snapshot_costs.push(snapshot_started.elapsed().as_nanos()as u64);
            if status["retained_rows"].as_u64().unwrap()>=17 && row_ready_at.is_none(){row_ready_at=Some(clock.elapsed());}
            if status["durable_windows"].as_u64().unwrap()>=17 && acked_at.is_none(){acked_at=Some(clock.elapsed());}
            if let Some(acked)=acked_at{if clock.elapsed()-acked>=Duration::from_millis(250){break;}}
        }
        if !source_arrivals.iter().all(|times|c2_precision_pacer_test_support::actual_stream_qualifies(times)){return Err("INPUT_UNQUALIFIED during real backend measurement; planned deadlines are not evidence".into());}
        if packet_wall.is_empty(){return Err("no measured packets".into());}
        packet_wall.sort_unstable();let max=*packet_wall.last().unwrap();let p99=packet_wall[(packet_wall.len()-1)*99/100];
        if max>5_000_000 || p99>2_000_000{return Err(format!("packet wall budget exceeded: max_ns={max} p99_ns={p99}"));}
        Ok(())
    })();
    rt.cancel_temporal_durability(&rt.mutation_guard().unwrap()).unwrap();
    let new_prefix=rt.temporal_store().unwrap().load_temporal(measured_now).unwrap().unwrap().prefix.unwrap().bytes().len();
    packet_wall.sort_unstable();let max=packet_wall.last().copied().unwrap_or(0);let p99=packet_wall.get(packet_wall.len().saturating_sub(1)*99/100).copied().unwrap_or(0);
    println!("SYNTHETIC_REAL_3_SOURCE_GROWING_PREFIX status={} old_bytes={} new_bytes={} packets={} max_packet_wall_ns={} p99_packet_wall_ns={} row_ready_at_ns={:?} acked_at_ns={:?} row_ready_to_ack_ns={:?} debug_assertions={} error={:?}",if result.is_ok(){"PASS"}else{"UNQUALIFIED"},old_prefix,new_prefix,packet_wall.len(),max,p99,row_ready_at.map(|d|d.as_nanos()),acked_at.map(|d|d.as_nanos()),row_ready_at.zip(acked_at).map(|(q,a)|a.saturating_sub(q).as_nanos()),cfg!(debug_assertions),result.as_ref().err());
    let summaries=source_arrivals.iter().enumerate().map(|(index,times)|{
        let span=times.last().copied().unwrap_or(0).saturating_sub(times.first().copied().unwrap_or(0));
        let intervals=times.len().saturating_sub(1) as u128;
        let gaps=times.windows(2).map(|w|w[1]-w[0]).collect::<Vec<_>>();
        json!({"source":index+1,"attempts":times.len(),"first_relative_ns":times.first(),"last_relative_ns":times.last(),"span_ns":span,
            "actual_rate_millihz":if span>0{Some(intervals*1_000_000_000_000u128/u128::from(span))}else{None},
            "min_gap_ns":gaps.iter().min(),"max_gap_ns":gaps.iter().max()})
    }).collect::<Vec<_>>();
    let contract=ManagedScheduleContract::build(context(),LeadInPolicy::Train5HeldOut15EngineeringV1).unwrap();let policy=contract.native_window_policy();let slot=&contract.episodes()[1].slots[0];
    println!("SYNTHETIC_CADENCE_DIAGNOSTIC {}",json!({"batch_count":batch_arrivals.len(),"batch_first_relative_ns":batch_arrivals.first(),"batch_last_relative_ns":batch_arrivals.last(),
        "sources":summaries,"failed_attempt":failed_attempt,"pacer_strategy":"absolute_actual_deadline_no_whole_period_catchup","spin_guard_ns":SPIN_GUARD_NS,"snapshot_max_ns":snapshot_costs.iter().max(),
        "slot_target_ms":slot.target_offset_ms,"slot_deadline_ms":slot.deadline_offset_ms,"native_target_span_ms":policy.target_span_ms(),
        "source_min_rows":policy.source_policies()[0].min_rows,"source_max_gap_ns":policy.source_policies()[0].max_gap_ns,
        "rate_band_millihz":policy.source_policies()[0].receiver_cadence.rate_bounds_millihz()}));
    assert!(result.is_ok(),"growing-prefix performance is UNQUALIFIED; no public activation: {:?}",result.err());
}

#[test]
fn c2_quiescent_start_waiter_handles_slow_recertification_without_observation_credit(){
    use std::{sync::mpsc,time::{Duration,Instant}};
    let dir=tempfile::tempdir().unwrap();let(rt,at)=configured_async(dir.path());
    let writer={let state=rt.inner.lock().unwrap();let Some(CaptureWorkflow::Temporal(c))=&state.capture else{panic!()};c.core.as_ref().unwrap().writer().unwrap()};
    let(entered_tx,entered_rx)=mpsc::sync_channel(1);let(release_tx,release_rx)=mpsc::sync_channel(1);writer.hold_next(entered_tx,release_rx);
    let release=std::thread::spawn(move||{entered_rx.recv().unwrap();std::thread::sleep(Duration::from_secs(6));release_tx.send(()).unwrap();});
    let started=Instant::now();let lead_end=async_lead_end(&rt,at);release.join().unwrap();
    assert!(started.elapsed()>=Duration::from_secs(6));
    let admitted=lead_end-TRAIN_LEAD_IN_NS;let status=snapshot(&rt,admitted);
    assert_eq!(status["retained_rows"],0);assert_eq!(status["durable_windows"],0);assert_eq!(status["observed_ns"],"0");
    assert_eq!(status["lead_in_remaining_ms"],5000,"lead-in must start only at actual authorization ACK");
    rt.cancel_temporal_durability(&rt.mutation_guard().unwrap()).unwrap();
}

// READ-ONLY production behavior probe: no budget/policy/clock/threshold changes.
// Private token Debug is parsed ONLY in this synthetic test to read the actual
// authenticated header revision without adding a production metadata accessor.
fn budget_repro_store_observation(rt:&RoomLearningRuntime,now:u64)->(usize,usize,usize,usize,String){
    let loaded=rt.temporal_store().unwrap().load_temporal(now).unwrap().unwrap();
    let token= format!("{:?}",loaded.token);
    let revision=token.split("revision: ").nth(1).unwrap().split(',').next().unwrap().parse::<usize>().unwrap();
    let prefix=loaded.prefix.unwrap();assert_eq!(prefix.transition_limit(),329);
    let value:Value=serde_json::from_slice(prefix.bytes()).unwrap();
    let rows=value["payload"]["rows"].as_array().unwrap().len();
    let gaps=value["payload"]["ledger"]["gaps"].as_array().unwrap().len();
    let bridge=&value["payload"]["bridge"];
    let parts=bridge["parts"].as_array().unwrap().len()+bridge["seals"].as_array().unwrap().iter().map(|s|s["parts"].as_array().unwrap().len()).sum::<usize>();
    (revision,rows,gaps,parts,format!("{:x}",<sha2::Sha256 as sha2::Digest>::digest(prefix.bytes())))
}
#[test]
#[cfg_attr(debug_assertions, ignore = "native-persistence journey requires Release; CI explicitly executes and counts this test")]
fn legacy_329_actual_runtime_saturation_retires_and_explicit_fresh_train_chains_343(){
    assert!(!cfg!(debug_assertions), "execute this native-persistence journey in the explicit Release batch");
    let dir=tempfile::tempdir().unwrap();let(rt,mut now)=configured_async(dir.path());let original=snapshot(&rt,now)["descriptor"].clone();
    assert_eq!(budget_repro_store_observation(&rt,now).0,0);
    //127 zero-observation pauses followed by127 resumes: initialsegment0 is
    //paused, segments1..126 paused, finalsegment127 will actually collectEmpty.
    for cycle in 0..127usize{
        now=async_lead_end(&rt,now);rt.tick_temporal_capture(&rt.mutation_guard().unwrap(),now).unwrap();
        rt.pause_temporal_capture(&rt.mutation_guard().unwrap(),now).unwrap();
        now=settle_async_boundary(&rt,now);
        assert_eq!(snapshot(&rt,now)["stage"],"paused");assert_eq!(snapshot(&rt,now)["durable_windows"],0);
        if cycle==0 || cycle==126{
            let actual=budget_repro_store_observation(&rt,now);
            assert_eq!(actual.0,2*(cycle+1));assert_eq!(actual.2,cycle);
            println!("SYNTHETIC_BUDGET_ZERO_PREFIX cycle={} authenticated_revision={} gaps={} parts={}",cycle+1,actual.0,actual.2,actual.3);
        }
    }
    let mut seq=0;let mut before_failure=None;
    for phase in 0..7u64{
        if phase==6{
            //Newlast-episodeinitialsegment has no newgap; pause it once so the
            //following realresume consumes the128thGLOBALgap. No laterstart.
            now=async_lead_end(&rt,now);rt.tick_temporal_capture(&rt.mutation_guard().unwrap(),now).unwrap();
            rt.pause_temporal_capture(&rt.mutation_guard().unwrap(),now).unwrap();now=settle_async_boundary(&rt,now);
        }
        loop{
            let status=snapshot(&rt,now);
            match status["stage"].as_str().unwrap(){
                "ready"|"paused"=>{now=async_lead_end(&rt,now);rt.tick_temporal_capture(&rt.mutation_guard().unwrap(),now).unwrap();},
                "needs_confirmation"=>break,
                "failed"=>panic!("STATIC_TRACE_REFUTED earlierphase failure phase={phase} status={status}"),_=>{}
            }
            if status["durable_windows"]==59 && before_failure.is_none(){
                let observed=budget_repro_store_observation(&rt,now);
                assert_eq!((observed.0,observed.1,observed.2,observed.3),(328,59,128,135));
                println!("SYNTHETIC_BUDGET_LAST_HEAD revision={} windows={} gaps={} parts={}",observed.0,observed.1,observed.2,observed.3);
                before_failure=Some(observed);
            }
            let pending=snapshot(&rt,now)["durability_pending"]==true;seq+=1;
            for source in 1..=3{
                match rt.ingest_native(&packet(source,seq,now),now){
                    Ok(())=>{},Err(error)if error=="temporal_storage_busy_pause"=>break,
                    Err(error)if error=="temporal_transition_budget"=>{
                        assert_eq!(phase,6,"wrong phase hit budget");let before=before_failure.as_ref().expect("no observed last valid head");
                        let after=budget_repro_store_observation(&rt,now);assert_eq!(&after,before,"failed operation changed authenticated durable prefix");
                        let state=snapshot(&rt,now);assert_eq!(state["durable_windows"],59);assert!(state["retained_rows"].as_u64().unwrap()>=60);
                        assert_eq!(state["stage"],"blocked");assert_eq!(state["descriptor"]["id"],original["id"]);assert_eq!(state["descriptor"]["expires_at_ms"],original["expires_at_ms"]);
                        println!("SYNTHETIC_ACTUAL_TRANSITION_BUDGET_CONFIRMED phase_index=6 failed_window_in_episode=4 proposed_revision=329 accepted_states=329 last_authenticated_revision={} last_durable_windows={} global_gaps={} quality_parts={} current_stage={} last_prefix_sha256_unchanged=true",after.0,after.1,after.2,after.3,state["stage"]);
                        rt.cancel_temporal_durability(&rt.mutation_guard().unwrap()).unwrap();
                        let store=rt.temporal_store().unwrap();
                        let token=store.load_temporal(now).unwrap().unwrap().token;
                        let original_prefix=store.load_temporal(now).unwrap().unwrap().prefix.unwrap();
                        // Same Runtime generated authenticated revision328. No
                        // synthetic metadata/counter injection creates the head.
                        assert!(store.fresh_train_genesis_predecessor(now).is_err());
                        // Independent current-time branch of the SAME real
                        // authenticated saturated prefix. Writer already drained;
                        // copying only this synthetic TempDir creates no live data.
                        let live=tempfile::tempdir().unwrap();let live_root=live.path().join("fixture");
                        copy_synthetic_budget_fixture(dir.path(),&live_root);
                        {let live_rt=runtime(&live_root,now);let live_store=live_rt.temporal_store().unwrap();
                            let current=live_store.load_temporal(now).unwrap().unwrap();
                            let live_intent=live_store.prepare_retire_temporal(&current.token,now,false).unwrap();
                            let terminal=live_store.publish_temporal(&live_intent,None,now).unwrap();
                            assert!(format!("{terminal:?}").contains("revision: 329,"));
                            assert!(live_store.load_temporal(now).unwrap().unwrap().prefix.is_none());
                            assert_eq!(live_store.repair_same_temporal(&live_intent,None,now).unwrap(),terminal);
                        }
                        let expired=original_prefix.descriptor().expires_at_ms*1_000_000;
                        assert_eq!(store.load_temporal(expired).err().as_deref(),Some("recovery_expired"));
                        let inspection=store.inspect_temporal_retirement(expired).unwrap().unwrap();
                        assert!(inspection.expired());
                        let intent=store.prepare_inspected_retirement(&inspection,expired,true).unwrap();
                        let retired=store.publish_temporal(&intent,None,expired).unwrap();
                        assert!(format!("{retired:?}").contains("generation: 0, revision: 329,"));
                        // A lost ACK can only repair the exact same terminal,
                        // never consume revision330 or restore positive rows.
                        assert_eq!(store.repair_same_temporal(&intent,None,expired).unwrap(),retired);
                        let same=store.prepare_retire_temporal(&retired,expired,false).unwrap();assert_eq!(store.publish_temporal(&same,None,expired).unwrap(),retired);
                        assert!(store.load_temporal(expired).unwrap().unwrap().prefix.is_none());
                        assert_eq!(store.fresh_train_genesis_predecessor(expired).unwrap(),Some(retired.clone()));
                        assert!(store.prepare_retire_temporal(&token,expired,false).is_err());
                        drop(rt);
                        let fresh_at=expired+1_000_000;
                        let fresh=runtime(dir.path(),fresh_at);
                        fresh.configure_fresh_train_343_capture(&fresh.mutation_guard().unwrap(),context(),"synthetic-fresh-train-343".into(),fresh_at).unwrap();
                        let fresh_now=settle_async_boundary(&fresh,fresh_at);
                        let loaded=fresh.temporal_store().unwrap().load_temporal(fresh_now).unwrap().unwrap();
                        assert_eq!(loaded.prefix.as_ref().unwrap().transition_limit(),343);
                        assert!(format!("{:?}",loaded.token).contains("generation: 1, revision: 0,"));
                        assert_ne!(loaded.prefix.as_ref().unwrap().descriptor().id,original_prefix.descriptor().id);
                        assert_eq!(snapshot(&fresh,fresh_now)["durable_windows"],0);
                        assert_eq!(snapshot(&fresh,fresh_now)["stage"],"ready");
                        fresh.cancel_temporal_durability(&fresh.mutation_guard().unwrap()).unwrap();
                        println!("SYNTHETIC_LEGACY_SATURATED_RETIRE_AND_FRESH_343_PASS expired_samples_unavailable=true terminal_revision=329 same_intent_repair=true fresh_rows=0");return;
                    },
                    Err(error)=>panic!("STATIC_TRACE_REFUTED earlierguard phase={phase} error={error}"),
                }
            }
            now+=PERIOD;if pending{std::thread::sleep(std::time::Duration::from_nanos(PERIOD));}
        }
        let guard=rt.mutation_guard().unwrap();let challenge=rt.temporal_confirmation_challenge(&guard).unwrap();rt.confirm_temporal_phase(&guard,&challenge,now).unwrap();now=settle_async_boundary(&rt,now);
        let actual=budget_repro_store_observation(&rt,now);println!("SYNTHETIC_BUDGET_PHASE_SEAL phase={} authenticated_revision={} windows={} gaps={} parts={}",phase+1,actual.0,actual.1,actual.2,actual.3);
    }
    panic!("STATIC_TRACE_REFUTED complete path did not hit predicted old329 budget");
}

// Fresh-profile complete upper-bound journey: original 128 gap / 135 part
// schedule plus a legitimate deferred pause before EACH of the seven seals.
// All counters below come from authenticated store readback, not a mock limit.
#[test]
#[cfg_attr(debug_assertions, ignore = "native-persistence journey requires Release; CI explicitly executes and counts this test")]
fn fresh_343_actual_runtime_all_128_gaps_seven_terminal_pauses_and_retirement_fit_budget(){
    assert!(!cfg!(debug_assertions), "execute this native-persistence journey in the explicit Release batch");
    let dir=tempfile::tempdir().unwrap();let rt=runtime(dir.path(),START);
    rt.configure_fresh_train_343_capture(&rt.mutation_guard().unwrap(),context(),"synthetic-full-343".into(),START).unwrap();
    let mut now=settle_async_boundary(&rt,START);let original=snapshot(&rt,now)["descriptor"].clone();
    for _ in 0..127{
        now=async_lead_end(&rt,now);rt.tick_temporal_capture(&rt.mutation_guard().unwrap(),now).unwrap();
        rt.pause_temporal_capture(&rt.mutation_guard().unwrap(),now).unwrap();now=settle_async_boundary(&rt,now);
        assert_eq!(snapshot(&rt,now)["durable_windows"],0);
    }
    let mut seq=0;let mut terminal_pauses=0;
    for phase in 0..7u64{
        if phase==6{
            now=async_lead_end(&rt,now);rt.tick_temporal_capture(&rt.mutation_guard().unwrap(),now).unwrap();
            rt.pause_temporal_capture(&rt.mutation_guard().unwrap(),now).unwrap();now=settle_async_boundary(&rt,now);
        }
        let target=16+phase*8;let mut terminal_pause=false;let supervision=std::time::Instant::now();
        loop{
            assert!(supervision.elapsed()<std::time::Duration::from_secs(60),"fresh343 phase did not settle: {}",snapshot(&rt,now));
            let status=snapshot(&rt,now);
            match status["stage"].as_str().unwrap(){
                "ready"|"paused" if !terminal_pause=>{now=async_lead_end(&rt,now);rt.tick_temporal_capture(&rt.mutation_guard().unwrap(),now).unwrap();},
                "needs_confirmation"=>break,"failed"|"blocked"=>panic!("fresh343 phase={phase} {status}"),_=>{}
            }
            let pending=snapshot(&rt,now)["durability_pending"]==true;seq+=1;
            for source in 1..=3{
                match rt.ingest_native(&packet(source,seq,now),now){Ok(())=>{},Err(e)if e=="temporal_storage_busy_pause"=>break,Err(e)=>panic!("fresh343 phase={phase} ingress={e}")}
            }
            let after=snapshot(&rt,now);
            if !terminal_pause && after["retained_rows"]==target && after["durability_pending"]==true{
                // Exact real pending final WINDOW. pause does not consume its
                // ACK; no new raw data, target slide, or fake duration is used.
                rt.pause_temporal_capture(&rt.mutation_guard().unwrap(),now).unwrap();
                terminal_pause=true;terminal_pauses+=1;
                now=settle_async_boundary(&rt,now);
                // Paused presentation precedes NeedsConfirmation; the real
                // challenge below is the authority for complete-phase credit.
                assert_eq!(snapshot(&rt,now)["durable_windows"],target);break;
            }else{now+=PERIOD;if pending{std::thread::sleep(std::time::Duration::from_nanos(PERIOD));}}
        }
        assert!(terminal_pause,"did not exercise terminal deferred pause");
        let guard=rt.mutation_guard().unwrap();let challenge=rt.temporal_confirmation_challenge(&guard).unwrap();
        rt.confirm_temporal_phase(&guard,&challenge,now).unwrap();now=settle_async_boundary(&rt,now);
        println!("SYNTHETIC_FRESH343_SEAL phase={} rows={} terminal_pauses={}",phase+1,target,terminal_pauses);
    }
    assert_eq!(snapshot(&rt,now)["sealed_train_episodes"],7);assert_eq!(snapshot(&rt,now)["durable_windows"],64);
    rt.cancel_temporal_durability(&rt.mutation_guard().unwrap()).unwrap();
    let store=rt.temporal_store().unwrap();let loaded=store.load_temporal(now).unwrap().unwrap();let prefix=loaded.prefix.unwrap();
    assert_eq!(prefix.transition_limit(),343);assert_eq!(prefix.window_count(),64);
    let value:Value=serde_json::from_slice(prefix.bytes()).unwrap();
    assert_eq!(value["payload"]["ledger"]["gaps"].as_array().unwrap().len(),128);
    let bridge=&value["payload"]["bridge"];
    let parts=bridge["parts"].as_array().unwrap().len()+bridge["seals"].as_array().unwrap().iter().map(|seal|seal["parts"].as_array().unwrap().len()).sum::<usize>();
    assert_eq!(parts,135);
    assert_eq!(prefix.descriptor().id,original["id"].as_str().unwrap());
    assert_eq!(prefix.descriptor().expires_at_ms,original["expires_at_ms"].as_u64().unwrap());
    let before=format!("{:?}",loaded.token);assert!(before.contains("revision: 341,"),"{before}");
    let intent=store.prepare_retire_temporal(&loaded.token,now,false).unwrap();let terminal=store.publish_temporal(&intent,None,now).unwrap();
    assert!(format!("{terminal:?}").contains("revision: 342,"));assert!(store.load_temporal(now).unwrap().unwrap().prefix.is_none());
    assert_eq!(store.repair_same_temporal(&intent,None,now).unwrap(),terminal);
    println!("SYNTHETIC_FRESH343_COMPLETE authenticated_states=343 last_revision=342 rows=64 gaps=128 quality_parts=135 terminal_pauses=7 retired=true");
}

#[test]
fn fresh343_actual_genesis_fixture_for_frozen_old_runtime_consumer(){
    let dir=tempfile::tempdir().unwrap();let rt=runtime(dir.path(),START);
    rt.configure_fresh_train_343_capture(&rt.mutation_guard().unwrap(),context(),"synthetic-old-reader-fixture".into(),START).unwrap();
    let now=settle_async_boundary(&rt,START);rt.cancel_temporal_durability(&rt.mutation_guard().unwrap()).unwrap();
    let loaded=rt.temporal_store().unwrap().load_temporal(now).unwrap().unwrap();assert_eq!(loaded.prefix.unwrap().transition_limit(),343);
    if let Some(output)=std::env::var_os("RUVIEW_2473_SYNTHETIC_FIXTURE_OUTPUT"){
        fn copy_private(from:&Path,to:&Path){
            std::fs::create_dir(to).unwrap();
            #[cfg(unix)]{use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(to,std::fs::Permissions::from_mode(0o700)).unwrap();}
            for e in std::fs::read_dir(from).unwrap(){let e=e.unwrap();let m=std::fs::symlink_metadata(e.path()).unwrap();assert!(!m.file_type().is_symlink());
                if m.is_dir(){copy_private(&e.path(),&to.join(e.file_name()));}else{assert!(m.is_file());std::fs::copy(e.path(),to.join(e.file_name())).unwrap();
                    #[cfg(unix)]{use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(to.join(e.file_name()),std::fs::Permissions::from_mode(0o600)).unwrap();}
                }
            }
        }
        let output=std::path::PathBuf::from(output);std::fs::create_dir_all(&output).unwrap();copy_private(dir.path(),&output.join("fresh343"));
        std::fs::write(output.join("fresh343.json"),serde_json::to_vec(&json!({"now_ns":now,"owner_scope":context().owner_scope,"installation_id":context().installation_id})).unwrap()).unwrap();
    }
}

// Synthetic temp fixture only: no symbolic links, public state or user profile.
fn copy_synthetic_budget_fixture(from:&Path,to:&Path){
    std::fs::create_dir(to).unwrap();
    #[cfg(unix)]{use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(to,std::fs::Permissions::from_mode(0o700)).unwrap();}
    for entry in std::fs::read_dir(from).unwrap(){let entry=entry.unwrap();let m=std::fs::symlink_metadata(entry.path()).unwrap();assert!(!m.file_type().is_symlink());
        if m.is_dir(){copy_synthetic_budget_fixture(&entry.path(),&to.join(entry.file_name()));}else{assert!(m.is_file());std::fs::copy(entry.path(),to.join(entry.file_name())).unwrap();
            #[cfg(unix)]{use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(to.join(entry.file_name()),std::fs::Permissions::from_mode(0o600)).unwrap();}
        }
    }
}
