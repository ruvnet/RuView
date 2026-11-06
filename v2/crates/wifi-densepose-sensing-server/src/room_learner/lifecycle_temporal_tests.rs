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
    rt.configure_temporal_capture(
        &rt.mutation_guard().unwrap(),
        context(),
        "synthetic-runtime-train".into(),
        START,
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
    let mut rt = configured(dir.path());
    let original = snapshot(&rt, START)["descriptor"].clone();
    let mut seq = 0;
    let mut now = START + 1;
    let mut trial = None;
    for _ in 0..2 {
        let first = start(&rt, now);
        now = feed(&rt, first, first + 100_000_000_000, &mut seq);
        let phase = rt.mutation_guard().unwrap().phase.unwrap();
        if let Some(prior) = &trial {
            assert_eq!(prior, &phase.trial_id);
        } else {
            trial = Some(phase.trial_id);
        }
        rt.pause_temporal_capture(&rt.mutation_guard().unwrap(), now)
            .unwrap();
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
    let first = start(&rt, now);
    now = feed(
        &rt,
        first,
        first + 600_000_000_000 - accepted + PERIOD,
        &mut seq,
    );
    assert_eq!(snapshot(&rt, now)["stage"], "needs_confirmation");
    let guard = rt.mutation_guard().unwrap();
    let challenge = rt.temporal_confirmation_challenge(&guard).unwrap();
    rt.confirm_temporal_phase(&guard, &challenge, now).unwrap();
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
