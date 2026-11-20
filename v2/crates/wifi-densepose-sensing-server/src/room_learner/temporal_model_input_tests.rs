//! SYNTHETIC complete native horizons through existing production fitter/infer.
use super::super::native::{canonicalize_esp32, NativePacketInput};
use super::*;
use std::sync::OnceLock;
use wifi_densepose_hardware::{Adr018Flags, PpduType};
fn context(count: u8) -> CaptureContext {
    CaptureContext {
        owner_scope: "a".repeat(64),
        installation_id: "synthetic-temporal".into(),
        room_digest: "synthetic-room".into(),
        revision: 1,
        sources: (1..=count)
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
fn bands(n: u8) -> Vec<(u8, ReceiverCadenceBand)> {
    (1..=n)
        .map(|n| (n, ReceiverCadenceBand::Hz30To36))
        .collect()
}
fn config() -> FitConfig {
    FitConfig {
        epochs: 2,
        ..Default::default()
    }
}
fn packet(source: u8, seq: u32, time: u64, label: RoomLabel, row: usize) -> CanonicalPacket {
    let gain = match label {
        RoomLabel::Empty => 3,
        RoomLabel::AdjacentOnly => 4,
        RoomLabel::OccupiedStatic => 8,
        RoomLabel::OccupiedMoving => 10 + (row % 2) as u8,
        RoomLabel::Unknown => 8,
    };
    let bytes = (0..64)
        .flat_map(|bin| [gain + source + (bin % 2) as u8, 4u8])
        .collect::<Vec<_>>();
    canonicalize_esp32(NativePacketInput {
        node_id: source,
        n_antennas: 1,
        raw_bins: 64,
        frequency_hz: 2_437_000_000,
        ppdu_type: PpduType::HtLegacy,
        sequence: seq,
        received_at_ns: time,
        receiver_sample_rate_hz: 33.,
        iq_bytes: &bytes,
        flags: Adr018Flags::default(),
        provenance: CaptureProvenance::Synthetic,
    })
    .unwrap()
}
fn collect(session: &mut TemporalTrainingSession, target: u64, seq: &mut u32) {
    session
        .begin_window(target, target + 3_000_000_000)
        .unwrap();
    let step = &session.plan.schedule()[session.rows.len() / session.plan.slots_per_trial()];
    let label = step.label;
    let period = 1_000_000_000u64.div_ceil(33);
    for row in 0..256 {
        *seq += 1;
        for source in 1..=2 {
            let now = target + row as u64 * period + u64::from(source) * 1000;
            if session
                .ingest(&packet(source, *seq, now, label, row))
                .unwrap()
                == WindowAdmission::Ready
            {
                session.finish_window(now).unwrap();
                return;
            }
        }
    }
    panic!("no native horizon");
}
fn fitted_bytes() -> Vec<u8> {
    static MODEL: OnceLock<Vec<u8>> = OnceLock::new();
    MODEL
        .get_or_init(|| {
            let mut s = TemporalTrainingSession::new(
                context(2),
                ExperimentalPlanId::Native2000msSlots2,
                &bands(2),
                "synthetic-fit".into(),
                config(),
            )
            .unwrap();
            let mut seq = 0;
            assert!(s.fit().is_err());
            for index in 0..16 {
                collect(&mut s, 10_000_000_000 + index * 3_000_000_000, &mut seq);
            }
            assert_eq!(s.dataset().unwrap().train().len(), 16);
            assert!(s.begin_window(60_000_000_000, 63_000_000_000).is_err());
            let (model, report) = s.fit().unwrap();
            assert_eq!(report.training_cohorts, 16);
            assert!(report.backbone_delta_l2 > 0.);
            assert!(report.parameter_delta_l2 > 0.);
            assert_eq!(report.completed_epochs, config().epochs);
            assert!(report.objective_evaluations > 0);
            assert!(report.initial.total.is_finite());
            assert!(report.final_objective.total.is_finite());
            assert_eq!(model.model.training_manifest().training_cohorts, 16);
            assert_eq!(model.evidence.len(), 16);
            for (index, evidence) in model.evidence.iter().enumerate() {
                assert_eq!(evidence.role, DataRole::Train);
                assert_eq!(evidence.label, s.plan.schedule()[index / 2].label);
                assert_eq!(evidence.target_ns,
                    (10_000_000_000u64 + index as u64 * 3_000_000_000).to_string());
                assert_eq!(evidence.sources.len(), 2);
            }
            assert_eq!(model.model.encoder_config(), s.plan.encoder_config());
            // Frozen fit precedes new HeldOut windows. A later fit cannot consume them.
            for index in 16..32 {
                collect(&mut s, 10_000_000_000 + index * 3_000_000_000, &mut seq);
            }
            let d = s.dataset().unwrap();
            assert_eq!(d.held_out().len(), 16);
            assert!(!d.measurement_clock_available());
            assert!(s.fit().is_err());
            model.export().unwrap()
        })
        .clone()
}
fn model() -> TemporalModel {
    TemporalModel::import(&fitted_bytes()).unwrap()
}
fn live(
    model: &TemporalModel,
    target: u64,
    rows: usize,
) -> Result<(TemporalLiveObservation, u64), String> {
    let mut input = model.live_input("fresh-live".into())?;
    input.begin_window(target, target + 3_000_000_000)?;
    let period = 1_000_000_000u64.div_ceil(33);
    let mut last = target;
    for row in 0..rows {
        for source in 1..=2 {
            last = target + row as u64 * period + u64::from(source) * 1000;
            let state = input.ingest(&packet(
                source,
                100_000 + row as u32,
                last,
                RoomLabel::Unknown,
                row,
            ))?;
            if state == WindowAdmission::Ready {
                return Ok((input.finish(last)?, last));
            }
        }
    }
    Ok((input.finish(last)?, last))
}
#[test]
fn parameter_source_and_frozen_encoder_preflight_run_before_collection_or_fit() {
    for count in [1, 9] {
        assert!(TemporalTrainingSession::new(
            context(count),
            ExperimentalPlanId::Native1000msSlots2,
            &bands(count),
            "bad".into(),
            config()
        )
        .is_err());
    }
    let mut exact = config();
    let count = exact.validate_context(&context(2)).unwrap();
    exact.max_parameters = count - 1;
    assert!(TemporalTrainingSession::new(
        context(2),
        ExperimentalPlanId::Native2000msSlots2,
        &bands(2),
        "actual-boundary".into(),
        exact
    )
    .is_err());
    let mut tiny = config();
    tiny.max_parameters = 1;
    assert!(TemporalTrainingSession::new(
        context(2),
        ExperimentalPlanId::Native2000msSlots2,
        &bands(2),
        "budget".into(),
        tiny
    )
    .is_err());
    let mut changed = config();
    changed.temporal_tokens = 8;
    assert!(TemporalTrainingSession::new(
        context(2),
        ExperimentalPlanId::Native2000msSlots2,
        &bands(2),
        "changed".into(),
        changed
    )
    .is_err());
}
#[test]
fn actual_fitter_cold_roundtrip_and_live_forward_keep_old_room_model_bytes_unchanged() {
    let m = model();
    let bytes = m.export().unwrap();
    assert_eq!(bytes, fitted_bytes());
    let cold = TemporalModel::import(&bytes).unwrap();
    assert_eq!(cold.export().unwrap(), bytes);
    assert_eq!(cold.parameter_count(), m.parameter_count());
    let old = m.model.export().unwrap();
    assert_eq!(RoomModel::import(&old).unwrap().export().unwrap(), old);
    assert!(RoomModel::import(&bytes).is_err());
    assert!(TemporalModel::import(&old).is_err());
    let (input, now) = live(&cold, 1_000_000_000_000, 100).unwrap();
    assert_eq!(input.window.cohort.role, DataRole::Inference);
    assert_eq!(input.window.cohort.label, RoomLabel::Unknown);
    assert_eq!(input.window.cohort.views[0].samples.len(), 67);
    let plain = cold.model.infer(&input.window.cohort);
    let prediction = cold.predict(input, now).unwrap();
    assert_eq!(
        serde_json::to_value(plain).unwrap(),
        serde_json::to_value(prediction).unwrap()
    );
    let envelope: Envelope = serde_json::from_slice(&bytes).unwrap();
    assert!(!serde_json::to_string(&envelope.payload.evidence)
        .unwrap()
        .contains("samples"));
    assert_eq!(
        envelope.payload.training_data_sha256,
        m.model.training_manifest().training_data_sha256
    );
}
#[test]
fn live_eight_or_64_packets_cannot_substitute_for_two_seconds() {
    let m = model();
    for rows in [8, 64] {
        assert!(live(&m, 1_000_000_000_000, rows).is_err());
    }
}
#[test]
fn live_training_replay_foreign_grid_policy_times_and_labelled_roles_are_refused() {
    let m = model();
    let mut input = m.live_input("replayed-live".into()).unwrap();
    input
        .begin_window(1_000_000_000_000, 1_003_000_000_000)
        .unwrap();
    assert!(input
        .ingest(&packet(1, 1, 1_000_000_001_000, RoomLabel::Unknown, 0))
        .is_err());
    for mode in ["policy", "grid", "times", "role", "rows", "stale"] {
        let (mut input, now) = live(&m, 1_000_000_000_000, 100).unwrap();
        match mode {
            "policy" => input.binding.plan_sha256 = "b".repeat(64),
            "grid" => input.window.cohort.views[0].grid_id = "wrong".into(),
            "times" => input.window.row_times_ns[0].1[1] += 100_000_000,
            "role" => {
                input.window.cohort.role = DataRole::Train;
                input.window.cohort.label = RoomLabel::Empty;
            }
            "rows" => {
                input.window.cohort.views[0].samples.truncate(8);
                input.window.cohort.views[0].frame_ids.truncate(8);
                input.window.cohort.views[0].sequence_ids.truncate(8);
                input.window.row_times_ns[0].1.truncate(8);
            }
            _ => {}
        }
        assert!(
            m.predict(
                input,
                if mode == "stale" {
                    now + 6_000_000_000
                } else {
                    now
                }
            )
            .is_err(),
            "{mode}"
        );
    }
}
fn recode(mut e: Envelope) -> Vec<u8> {
    e.payload.input_evidence_sha256 = hash(&e.payload.evidence).unwrap();
    e.payload_sha256 = hash(&e.payload).unwrap();
    serde_json::to_vec(&e).unwrap()
}

#[test]
fn cold_import_rejects_hash_policy_timing_source_and_cross_role_tampering() {
    let bytes = fitted_bytes();
    for mode in [
        "outer_hash",
        "model_hash",
        "plan_hash",
        "plan_horizon",
        "source",
        "rows",
        "gap",
        "frame_overlap",
        "role",
        "provenance",
    ] {
        let mut e: Envelope = serde_json::from_slice(&bytes).unwrap();
        match mode {
            "outer_hash" => e.payload_sha256 = "0".repeat(64),
            "model_hash" => e.payload.model_sha256 = "0".repeat(64),
            "plan_hash" => e.payload.binding.plan_sha256 = "0".repeat(64),
            "plan_horizon" => {
                let p = AcquisitionPlan::build(
                    context(2),
                    ExperimentalPlanId::Native1000msSlots2,
                    &bands(2),
                )
                .unwrap();
                e.payload.binding = Binding::from_plan(&p);
            }
            "source" => e.payload.evidence[0].sources[0].source_spec_sha256 = "f".repeat(64),
            "rows" => {
                let s = &mut e.payload.evidence[0].sources[0];
                s.times_ns.truncate(8);
                s.frame_ids.truncate(8);
                s.sequence_ids.truncate(8);
            }
            "gap" => {
                let s = &mut e.payload.evidence[0].sources[0];
                let t = integer::<u64>(&s.times_ns[1]).unwrap();
                s.times_ns[1] = (t + 500_000_000).to_string();
            }
            "frame_overlap" => {
                e.payload.evidence[1].sources[0].frame_ids[0] =
                    e.payload.evidence[0].sources[0].frame_ids[0].clone()
            }
            "role" => e.payload.evidence[0].role = DataRole::HeldOut,
            "provenance" => {
                e.payload.evidence[0].provenance = CaptureProvenance::LiveReceiverObserved
            }
            _ => unreachable!(),
        }
        // Even recomputing generic envelope checksums cannot make a structurally
        // incompatible plan/role/time/source claim satisfy the real input policy.
        let altered = if mode == "outer_hash" {
            serde_json::to_vec(&e).unwrap()
        } else {
            recode(e)
        };
        assert!(TemporalModel::import(&altered).is_err(), "{mode}");
    }
    assert!(TemporalModel::import(&vec![b' '; MAX_BYTES + 1]).is_err());
}
#[test]
fn caller_cannot_fit_incomplete_or_failed_acquisition_as_a_complete_policy() {
    let mut s = TemporalTrainingSession::new(
        context(2),
        ExperimentalPlanId::Native2000msSlots2,
        &bands(2),
        "failure".into(),
        config(),
    )
    .unwrap();
    let t = 10_000_000_000;
    s.begin_window(t, t + 3_000_000_000).unwrap();
    s.ingest(&packet(1, 1, t + 1000, RoomLabel::Empty, 0))
        .unwrap();
    assert!(s.finish_window(t + 1_000_000_000).is_err());
    assert!(s.fit().is_err());
    assert!(s
        .begin_window(t + 4_000_000_000, t + 7_000_000_000)
        .is_err());
}

#[test]
fn cold_consistent_rate_time_shift_cannot_detach_original_receiver_times_from_frame_ids() {
    let mut e: Envelope = serde_json::from_slice(&fitted_bytes()).unwrap();
    // Keep duration, row count, gaps, average cadence and source skew valid.
    // Recomputed envelope hashes cannot make the unchanged ORIGINAL frame IDs
    // refer to the shifted receiver times or a new original wire counter.
    for source in &mut e.payload.evidence[0].sources {
        for time in &mut source.times_ns {
            *time = (integer::<u64>(time).unwrap() + 1000).to_string();
        }
    }
    let seal = integer::<u64>(&e.payload.evidence[0].sealed_at_ns).unwrap();
    e.payload.evidence[0].sealed_at_ns = (seal + 1000).to_string();
    assert!(TemporalModel::import(&recode(e)).is_err());
}

#[test]
fn post_assembler_evidence_failure_is_terminal_and_preserves_previous_valid_windows() {
    let mut session = TemporalTrainingSession::new(
        context(2),
        ExperimentalPlanId::Native2000msSlots2,
        &bands(2),
        "post-consume".into(),
        config(),
    )
    .unwrap();
    let mut sequence = 0;
    collect(&mut session, 10_000_000_000, &mut sequence);
    let prior = hash(&session.rows).unwrap();
    let target = 20_000_000_000;
    session
        .begin_window(target, target + 3_000_000_000)
        .unwrap();
    let period = 1_000_000_000u64.div_ceil(33);
    let mut now = target;
    'ready: for row in 0..256 {
        sequence += 1;
        for source in 1..=2 {
            now = target + row as u64 * period + u64::from(source) * 1000;
            let mut packet = packet(source, sequence, now, RoomLabel::Empty, row);
            if row == 0 && source == 1 {
                packet.native_frame.frame_id ^= 1;
                assert_ne!(packet.native_frame.frame_id, 0);
            }
            if session.ingest(&packet).unwrap() == WindowAdmission::Ready {
                break 'ready;
            }
        }
    }
    assert_eq!(
        session.finish_window(now).err().as_deref(),
        Some("temporal_native_frame_identity_changed")
    );
    assert!(session.failed);
    assert!(!session.active);
    assert!(session.target_ns.is_none());
    assert_eq!(session.rows.len(), 1);
    assert_eq!(session.evidence.len(), 1);
    assert_eq!(hash(&session.rows).unwrap(), prior);
    assert!(session
        .begin_window(30_000_000_000, 33_000_000_000)
        .is_err());
    assert!(session
        .ingest(&packet(
            1,
            sequence + 1,
            30_000_001_000,
            RoomLabel::Empty,
            0
        ))
        .is_err());
    assert!(session.finish_window(now).is_err());
    assert!(session.fit().is_err());
}

// Frozen SYNTHETIC pre-C1 artifact produced by the actual native-horizon fitter
// on base #2465. Compatibility means preserving its serialized bytes, not
// requiring newly trained floating-point weights to match across platforms.
const PUBLIC_SEED_V2_GOLDEN: &[u8] =
    include_bytes!("../../tests/fixtures/temporal-seed-v2-synthetic-golden.json");

#[test]
fn public_seed_v2_frozen_artifact_import_reexports_exact_bytes() {
    assert_eq!(PUBLIC_SEED_V2_GOLDEN.len(), 113_803);
    assert_eq!(
        bytes_hash(PUBLIC_SEED_V2_GOLDEN),
        "e46e9aff391b0e41a8fe26cc46789ee65d130f109df444726893303960b5735f"
    );
    let frozen = TemporalModel::import(PUBLIC_SEED_V2_GOLDEN).unwrap();
    assert_eq!(frozen.context(), &context(2));
    assert_eq!(frozen.model.training_manifest().training_cohorts, 16);
    assert_eq!(frozen.model.training_manifest().training_provenance,
        vec![CaptureProvenance::Synthetic]);
    assert_eq!(frozen.evidence.len(), 16);
    assert!(frozen.evidence.iter().all(|e| e.role == DataRole::Train));
    assert_eq!(frozen.export().unwrap(), PUBLIC_SEED_V2_GOLDEN);
}

#[test]
fn public_seed_v2_frozen_artifact_rehashed_role_tamper_is_rejected() {
    let mut envelope: Envelope = serde_json::from_slice(PUBLIC_SEED_V2_GOLDEN).unwrap();
    envelope.payload.evidence[0].role = DataRole::HeldOut;
    // Recompute the generic checksums: the real policy must still reject an
    // artifact whose historical Train evidence was relabelled as HeldOut.
    envelope.payload.input_evidence_sha256 = hash(&envelope.payload.evidence).unwrap();
    let altered = recode(envelope);
    assert!(TemporalModel::import(&altered).is_err());
}
