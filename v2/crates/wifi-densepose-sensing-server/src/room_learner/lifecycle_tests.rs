//! @tier durable — SYNTHETIC packets through the actual managed public path.
use super::*;
use crate::room_learner::{
    capture::{MIN_PHASE_MS, WINDOW_PACKETS},
    observation::{RoomLabel, SourceSpec},
};
use ruview_unified::RfModality;
use std::time::{Duration, Instant};
pub(super) const START: u64 = 1_000_000_000_000;
pub(super) fn context() -> CaptureContext {
    CaptureContext {
        owner_scope: "a".repeat(64),
        installation_id: "synthetic-install".into(),
        room_digest: "synthetic-room".into(),
        revision: 1,
        sources: (1..=2)
            .map(|id| SourceSpec {
                source_id: id,
                device_id: format!("synthetic-esp32-{id}"),
                grid_id: "ht64-canonical8".into(),
                feature_count: 8,
                signal_kind: RfModality::WifiCsi,
                link_id: format!("synthetic-link-{id}"),
                link_index: 0,
                center_frequency_hz: 2_437_000_000,
                bandwidth_hz: 20_000_000,
                geometry: None,
            })
            .collect(),
    }
}
pub(super) fn runtime(root: &Path) -> Arc<RoomLearningRuntime> {
    Arc::new(
        RoomLearningRuntime::new(
            &root.join("a".repeat(64)).join("SensingServer"),
            &"a".repeat(64),
            "synthetic-install",
        )
        .unwrap(),
    )
}
pub(super) fn phase(rt: &RoomLearningRuntime, role: DataRole, label: RoomLabel, index: u32) -> u64 {
    let at = START + u64::from(index) * 13_000_000_000;
    rt.begin_phase(
        &rt.mutation_guard().unwrap(),
        PhaseRequest {
            trial_id: format!("synthetic-trial-{index}"),
            session_id: format!("synthetic-session-{index}"),
            role,
            label,
            duration_ms: MIN_PHASE_MS,
            lead_in_ms: 0,
            conditions_confirmed: true,
        },
        at,
    )
    .unwrap();
    let gain = match label {
        RoomLabel::Empty => 2.0,
        RoomLabel::OccupiedStatic => 6.0,
        RoomLabel::OccupiedMoving => 7.0,
        RoomLabel::AdjacentOnly => 2.3,
        RoomLabel::Unknown => panic!("not labelled"),
    };
    for quarter in 0..4 {
        for sample in 0..WINDOW_PACKETS {
            for source in 1..=2 {
                let t = at
                    + quarter * 3_000_000_000
                    + 1_000_000
                    + sample as u64 * 10_000_000
                    + u64::from(source);
                let values: Vec<_> = (0..8)
                    .map(|bin| {
                        gain + f64::from(source) * 0.3 + bin as f64 * 0.04 + sample as f64 * 0.003
                    })
                    .collect();
                rt.ingest(
                    RawSourceFrame {
                        source_id: source,
                        grid_id: "ht64-canonical8",
                        frequency_hz: 2_437_000_000,
                        bandwidth_hz: 20_000_000,
                        sequence: index * 32 + quarter as u32 * 8 + sample as u32 + 1,
                        amplitudes: &values,
                    },
                    t,
                )
                .unwrap();
            }
        }
    }
    rt.finish_phase(
        &rt.mutation_guard().unwrap(),
        at + MIN_PHASE_MS * 1_000_000,
        true,
    )
    .unwrap();
    at + MIN_PHASE_MS * 1_000_000
}
pub(super) fn train_phases(rt: &RoomLearningRuntime) {
    for (index, label) in [
        RoomLabel::Empty,
        RoomLabel::OccupiedStatic,
        RoomLabel::OccupiedMoving,
        RoomLabel::AdjacentOnly,
    ]
    .into_iter()
    .enumerate()
    {
        phase(rt, DataRole::Train, label, index as u32);
    }
}
pub(super) async fn finished(rt: &RoomLearningRuntime) -> Value {
    let start = Instant::now();
    loop {
        let s = rt.status(START + 60_000_000_000);
        if !matches!(s["state"].as_str(), Some("running" | "cancelling")) {
            return s;
        }
        assert!(
            start.elapsed() < Duration::from_secs(65),
            "bounded synthetic training did not finish: {s}"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}
#[tokio::test]
async fn real_capture_train_private_store_cold_load_and_unlabelled_inference_compose() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let ctx = context();
    rt.configure(
        &rt.mutation_guard().unwrap(),
        ctx.clone(),
        "synthetic-run".into(),
        true,
    )
    .unwrap();
    train_phases(&rt);
    rt.start_training(
        &rt.mutation_guard().unwrap(),
        FitConfig {
            epochs: 4, // Composition needs real updates, not accuracy convergence.
            ..FitConfig::default()
        },
    )
    .unwrap();
    let trained = finished(&rt).await;
    assert_eq!(trained["state"], "ready", "{trained}");
    assert!(
        trained["training_report"]["encoder_delta_l2"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert_eq!(trained["physically_validated"], false);
    assert_eq!(trained["control_detector_changed"], false);
    assert!(rt
        .begin_phase(
            &rt.mutation_guard().unwrap(),
            PhaseRequest {
                trial_id: "late-train".into(),
                session_id: "late-session".into(),
                role: DataRole::Train,
                label: RoomLabel::Empty,
                duration_ms: MIN_PHASE_MS,
                lead_in_ms: 0,
                conditions_confirmed: true
            },
            START + 60_000_000_000
        )
        .is_err());
    phase(&rt, DataRole::HeldOut, RoomLabel::Empty, 4);
    phase(&rt, DataRole::HeldOut, RoomLabel::OccupiedStatic, 5);
    let report = rt
        .evaluate(&rt.mutation_guard().unwrap(), DataRole::HeldOut)
        .unwrap();
    assert_eq!(report["adoption_authorized"], false);
    assert!(rt
        .evaluate(&rt.mutation_guard().unwrap(), DataRole::Train)
        .is_err());
    let cold = runtime(dir.path());
    cold.configure(
        &cold.mutation_guard().unwrap(),
        ctx,
        "synthetic-cold".into(),
        false,
    )
    .unwrap();
    assert_eq!(cold.status(START)["candidate_loaded"], true);
    for row in 0..8 {
        for source in 1..=2 {
            let values = vec![6.0 + f64::from(source) * 0.3; 8];
            cold.ingest(
                RawSourceFrame {
                    source_id: source,
                    grid_id: "ht64-canonical8",
                    frequency_hz: 2_437_000_000,
                    bandwidth_hz: 20_000_000,
                    sequence: 100 + row,
                    amplitudes: &values,
                },
                START + 100_000_000_000 + u64::from(row) * 10_000_000 + u64::from(source),
            )
            .unwrap();
        }
    }
    let (_, prediction) = cold.prediction(START + 100_080_000_000).unwrap();
    assert!(prediction.occupied_probability.is_some());
    assert!(!prediction.physically_validated);
    super::evaluation_tests::assert_no_raw_observation_arrays(&cold.status(START));
}
#[tokio::test]
async fn cancellation_and_context_replacement_cannot_publish_a_late_model() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let ctx = context();
    rt.configure(
        &rt.mutation_guard().unwrap(),
        ctx.clone(),
        "synthetic-cancel".into(),
        true,
    )
    .unwrap();
    train_phases(&rt);
    rt.start_training(
        &rt.mutation_guard().unwrap(),
        FitConfig {
            epochs: 128,
            ..FitConfig::default()
        },
    )
    .unwrap();
    assert!(rt
        .configure(
            &rt.mutation_guard().unwrap(),
            ctx.clone(),
            "other-run".into(),
            true
        )
        .is_err());
    assert_eq!(
        rt.cancel_training(&rt.mutation_guard().unwrap(),).unwrap(),
        JobPhase::Cancelling
    );
    assert_eq!(finished(&rt).await["state"], "cancelled");
    let cold = runtime(dir.path());
    assert!(cold
        .configure(
            &cold.mutation_guard().unwrap(),
            ctx,
            "no-late-model".into(),
            false
        )
        .is_err());
    assert_eq!(cold.status(START)["candidate_loaded"], false);
}
#[tokio::test]
async fn wrong_owner_and_incomplete_training_leave_records_and_model_store_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let mut ctx = context();
    ctx.owner_scope = "b".repeat(64);
    assert!(rt
        .configure(&rt.mutation_guard().unwrap(), ctx, "wrong".into(), true)
        .is_err());
    rt.configure(
        &rt.mutation_guard().unwrap(),
        context(),
        "incomplete".into(),
        true,
    )
    .unwrap();
    phase(&rt, DataRole::Train, RoomLabel::Empty, 0);
    assert!(rt
        .start_training(&rt.mutation_guard().unwrap(), FitConfig::default())
        .is_err());
    assert_eq!(rt.status(START)["state"], "idle");
    assert_eq!(rt.status(START)["capture"]["sealed_cohorts"], 4);
    assert!(rt.prediction(START).is_err());
}

#[tokio::test]
async fn operator_refusal_discards_only_unsealed_examples_and_bad_config_does_not_freeze_data() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    rt.configure(
        &rt.mutation_guard().unwrap(),
        context(),
        "synthetic-refusal".into(),
        true,
    )
    .unwrap();
    phase(&rt, DataRole::Train, RoomLabel::Empty, 0);
    rt.begin_phase(
        &rt.mutation_guard().unwrap(),
        PhaseRequest {
            trial_id: "refused".into(),
            session_id: "refused-session".into(),
            role: DataRole::Train,
            label: RoomLabel::OccupiedStatic,
            duration_ms: MIN_PHASE_MS,
            lead_in_ms: 0,
            conditions_confirmed: true,
        },
        START + 13_000_000_000,
    )
    .unwrap();
    assert!(rt
        .finish_phase(&rt.mutation_guard().unwrap(), START + 25_000_000_000, false)
        .is_err());
    assert_eq!(
        rt.status(START + 25_000_000_000)["capture"]["sealed_cohorts"],
        4
    );
    assert_eq!(rt.status(START + 25_000_000_000)["training_frozen"], false);
    assert!(rt
        .start_training(
            &rt.mutation_guard().unwrap(),
            FitConfig {
                epochs: 0,
                ..FitConfig::default()
            }
        )
        .is_err());
    assert_eq!(rt.status(START)["training_frozen"], false);
}
#[tokio::test]
async fn cancelled_training_can_retry_frozen_data_without_recapture_or_reopening_train() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    rt.configure(
        &rt.mutation_guard().unwrap(),
        context(),
        "synthetic-retry".into(),
        true,
    )
    .unwrap();
    train_phases(&rt);
    rt.start_training(
        &rt.mutation_guard().unwrap(),
        FitConfig {
            epochs: 128,
            ..FitConfig::default()
        },
    )
    .unwrap();
    rt.cancel_training(&rt.mutation_guard().unwrap()).unwrap();
    assert_eq!(finished(&rt).await["state"], "cancelled");
    assert_eq!(rt.status(START)["capture"]["sealed_cohorts"], 16);
    rt.start_training(
        &rt.mutation_guard().unwrap(),
        FitConfig {
            epochs: 4,
            ..FitConfig::default()
        },
    )
    .unwrap();
    assert_eq!(finished(&rt).await["state"], "ready");
    assert_eq!(rt.status(START)["capture"]["sealed_cohorts"], 16);
}
#[test]
fn typed_synthetic_native_input_cannot_be_relabelled_live_by_runtime() {
    use crate::room_learner::{
        native::{canonicalize_esp32, NativePacketInput},
        observation::CaptureProvenance,
    };
    use wifi_densepose_hardware::{Adr018Flags, PpduType};
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let bytes: Vec<_> = (0..64).flat_map(|_| [3u8, 4]).collect();
    for provenance in [
        CaptureProvenance::Synthetic,
        CaptureProvenance::CapturedReplay,
    ] {
        let packet = canonicalize_esp32(NativePacketInput {
            node_id: 1,
            n_antennas: 1,
            raw_bins: 64,
            frequency_hz: 2_437_000_000,
            ppdu_type: PpduType::HtLegacy,
            sequence: 1,
            received_at_ns: START,
            receiver_sample_rate_hz: 10.0,
            iq_bytes: &bytes,
            flags: Adr018Flags::default(),
            provenance,
        })
        .unwrap();
        assert!(rt.ingest_native(&packet, START).is_err());
    }
}

#[test]
fn delayed_commands_cannot_cross_room_phase_or_helper_restart() {
    let dir = tempfile::tempdir().unwrap();
    let rt = runtime(dir.path());
    let initial = rt.mutation_guard().unwrap();
    rt.configure(&initial, context(), "run-a".into(), true)
        .unwrap();
    let room_a = rt.mutation_guard().unwrap();
    let mut b = context();
    b.room_digest = "other-room".into();
    rt.configure(&room_a, b.clone(), "run-b".into(), true)
        .unwrap();
    let request = PhaseRequest {
        trial_id: "delayed-a".into(),
        session_id: "a".into(),
        role: DataRole::Train,
        label: RoomLabel::Empty,
        duration_ms: MIN_PHASE_MS,
        lead_in_ms: 0,
        conditions_confirmed: true,
    };
    assert_eq!(
        rt.begin_phase(&room_a, request.clone(), START).unwrap_err(),
        "learner_state_changed"
    );
    assert_eq!(
        rt.configure(&room_a, context(), "late-config".into(), true)
            .unwrap_err(),
        "learner_state_changed"
    );
    let idle_b = rt.mutation_guard().unwrap();
    rt.begin_phase(&idle_b, request.clone(), START).unwrap();
    let phase_one = rt.mutation_guard().unwrap();
    rt.cancel_phase(&phase_one).unwrap();
    let mut next = request;
    next.trial_id = "fresh-b".into();
    rt.begin_phase(&rt.mutation_guard().unwrap(), next, START)
        .unwrap();
    assert_eq!(
        rt.finish_phase(&phase_one, START + 13_000_000_000, true)
            .unwrap_err(),
        "learner_state_changed"
    );
    assert_eq!(
        rt.cancel_phase(&phase_one).unwrap_err(),
        "learner_state_changed"
    );
    assert_eq!(
        rt.cancel_training(&phase_one).unwrap_err(),
        "learner_state_changed"
    );
    assert_eq!(
        rt.start_training(&phase_one, FitConfig::default())
            .unwrap_err(),
        "learner_state_changed"
    );
    assert_eq!(
        rt.evaluate(&phase_one, DataRole::HeldOut).unwrap_err(),
        "learner_state_changed"
    );
    assert_eq!(
        rt.mutation_guard().unwrap().phase.unwrap().trial_id,
        "fresh-b"
    );
    let cold = runtime(dir.path());
    assert_ne!(
        initial.runtime_epoch,
        cold.mutation_guard().unwrap().runtime_epoch
    );
    assert_eq!(
        cold.configure(&initial, b, "stale-after-restart".into(), true)
            .unwrap_err(),
        "learner_state_changed"
    );
    assert!(cold.context().is_none());
}
#[test]
fn advertised_defaults_fit_all_native_source_counts_before_capture() {
    for count in 2..=8 {
        let dir = tempfile::tempdir().unwrap();
        let rt = runtime(dir.path());
        let mut ctx = context();
        let template = ctx.sources[0].clone();
        ctx.sources = (1..=count)
            .map(|id| {
                let mut spec = template.clone();
                spec.source_id = id as u8;
                spec.device_id = format!("synthetic-device-{id}");
                spec.link_id = format!("synthetic-link-{id}");
                spec.feature_count = 56;
                spec
            })
            .collect();
        rt.configure(
            &rt.mutation_guard().unwrap(),
            ctx.clone(),
            "budget".into(),
            true,
        )
        .unwrap();
        let defaults: FitConfig =
            serde_json::from_value(rt.status(START)["training_defaults"].clone()).unwrap();
        let actual = defaults.validate_context(&ctx).unwrap();
        assert!(actual <= 512, "{count} sources require {actual} parameters");
        if count >= 7 {
            assert!(FitConfig::default().validate_context(&ctx).is_err());
        }
    }
}

#[test]
fn native_rejection_diagnostics_are_context_bound_bounded_and_not_admission() {
    let dir=tempfile::tempdir().unwrap();let rt=runtime(dir.path());
    rt.configure(&rt.mutation_guard().unwrap(),context(),"native-diagnostic".into(),true).unwrap();
    let observed=rt.mutation_guard().unwrap();
    rt.record_native_rejection(17,"ht64-canonical8","unselected");
    rt.record_native_rejection(1,"other-grid","unselected");
    assert!(rt.status(START)["input_error"].is_null());
    rt.record_native_rejection(1,"ht64-canonical8",&"x".repeat(1000));
    let status=rt.status(START);
    assert!(status["input_error"].as_str().unwrap().starts_with("Node 1 native input rejected:"));
    assert!(status["input_error"].as_str().unwrap().len()<=256);
    assert_eq!(rt.mutation_guard().unwrap(),observed);
    assert_eq!(status["capture"]["sources"][0]["admitted_packets"],0);
    assert_eq!(status["physically_validated"],false);
}
