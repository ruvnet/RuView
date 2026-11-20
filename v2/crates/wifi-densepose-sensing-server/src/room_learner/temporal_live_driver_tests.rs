//! SYNTHETIC native sources; actual fitter/store/cold loader/opaque infer path.
use crate::room_learner::temporal_acquisition::AcquisitionPlan;
use super::*;
use crate::room_learner::{
    artifact::ArtifactStore,
    native::{canonicalize_esp32, NativePacketInput},
    observation::*,
    temporal_acquisition::{ExperimentalPlanId, ReceiverCadenceBand},
    temporal_model_input::TemporalTrainingSession,
    training::FitConfig,
};
use std::sync::OnceLock;
use wifi_densepose_hardware::{Adr018Flags, PpduType};
const PERIOD: u64 = 30_303_031;
const FRESH: u64 = 1_000_000_000_000;
fn context() -> CaptureContext {
    CaptureContext {
        owner_scope: "a".repeat(64),
        installation_id: "synthetic-continuous".into(),
        room_digest: "synthetic-room".into(),
        revision: 1,
        sources: (1..=2)
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
fn packet(source: u8, sequence: u32, time: u64, label: RoomLabel, row: usize) -> CanonicalPacket {
    let gain = match label {
        RoomLabel::Empty => 3,
        RoomLabel::AdjacentOnly => 4,
        RoomLabel::OccupiedStatic => 8,
        RoomLabel::OccupiedMoving => 10 + (row % 2) as u8,
        RoomLabel::Unknown => 8,
    };
    let bytes = (0..64)
        .flat_map(|bin| [gain + source + (bin % 2) as u8, 4])
        .collect::<Vec<_>>();
    canonicalize_esp32(NativePacketInput {
        node_id: source,
        n_antennas: 1,
        raw_bins: 64,
        frequency_hz: 2_437_000_000,
        ppdu_type: PpduType::HtLegacy,
        sequence,
        received_at_ns: time,
        receiver_sample_rate_hz: 33.,
        iq_bytes: &bytes,
        flags: Adr018Flags::default(),
        provenance: CaptureProvenance::Synthetic,
    })
    .unwrap()
}
fn fitted() -> TemporalModel {
    fitted_id(ExperimentalPlanId::Native2000msSlots2)
}
fn fitted_id(id: ExperimentalPlanId) -> TemporalModel {
    static SHORT: OnceLock<Vec<u8>> = OnceLock::new();
    static LONG: OnceLock<Vec<u8>> = OnceLock::new();
    let bytes = if id == ExperimentalPlanId::Native2000msSlots2 {
        &SHORT
    } else {
        &LONG
    };
    TemporalModel::import(bytes.get_or_init(|| {
        let mut s = TemporalTrainingSession::new(
            context(),
            id,
            &[
                (1, ReceiverCadenceBand::Hz30To36),
                (2, ReceiverCadenceBand::Hz30To36),
            ],
            "synthetic-continuous-fit".into(),
            FitConfig {
                epochs: 2,
                ..Default::default()
            },
        )
        .unwrap();
        let plan = AcquisitionPlan::build(
            context(),
            id,
            &[
                (1, ReceiverCadenceBand::Hz30To36),
                (2, ReceiverCadenceBand::Hz30To36),
            ],
        )
        .unwrap();
        let mut sequence = 0;
        for index in 0..16 {
            let at = 10_000_000_000 + index * 5_000_000_000;
            let label = plan.schedule()[index as usize / 2].label;
            s.begin_window(at, at + 5_000_000_000).unwrap();
            'window: for row in 0..256 {
                sequence += 1;
                for source in 1..=2 {
                    let now = at + row as u64 * PERIOD + u64::from(source) * 1000;
                    if s.ingest(&packet(source, sequence, now, label, row))
                        .unwrap()
                        == WindowAdmission::Ready
                    {
                        s.finish_window(now).unwrap();
                        break 'window;
                    }
                }
            }
        }
        let (model, report) = s.fit().unwrap();
        assert!(report.backbone_delta_l2 > 0.);
        assert_eq!(report.training_cohorts, 16);
        model.export().unwrap()
    }))
    .unwrap()
}
fn feed(
    d: &mut ContinuousTemporalInput,
    start: u64,
    rows: usize,
    sequence: u32,
) -> Vec<LiveDiagnostic> {
    let mut completed = vec![];
    for row in 0..rows {
        for source in 1..=2 {
            let now = start + row as u64 * PERIOD + u64::from(source) * 1000;
            let status = d.ingest(
                &packet(source, sequence + row as u32, now, RoomLabel::Unknown, row),
                now,
            );
            if status.buffer_state == LiveState::WindowComplete {
                completed.push(status);
            }
        }
    }
    completed
}
fn driver() -> ContinuousTemporalInput {
    ContinuousTemporalInput::new(fitted()).unwrap()
}
#[test]
fn default_inactive_and_short_inputs_never_produce_absence_or_prediction() {
    let mut d = driver();
    let p = packet(1, 100_000, FRESH, RoomLabel::Unknown, 0);
    assert_eq!(d.ingest(&p, FRESH).buffer_state, LiveState::Inactive);
    assert!(d.fences.is_empty());
    d.activate();
    assert!(feed(&mut d, FRESH, 8, 100_000).is_empty());
    let status = d.poll(FRESH + 7 * PERIOD + 2000);
    assert_eq!(status.buffer_state, LiveState::Warming);
    assert!(status.current_prediction.is_none());
    assert!(status.diagnostic_only && !status.physically_validated && !status.adoption_authorized);
}
#[test]
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn actual_fit_store_cold_load_then_continuous_windows_use_the_same_infer_path() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = context();
    let store = ArtifactStore::new(
        &dir.path().join(&ctx.owner_scope).join("SensingServer"),
        &ctx.owner_scope,
        &ctx.installation_id,
    )
    .unwrap();
    let model = fitted();
    let receipt = store.save_temporal_candidate(&model).unwrap();
    let cold = store
        .load_exact_temporal_candidate(&ctx, &receipt.artifact_sha256)
        .unwrap();
    assert_eq!(cold.export().unwrap(), model.export().unwrap());
    let mut d = ContinuousTemporalInput::new(cold).unwrap();
    assert_eq!(d.status.model_sha256, receipt.artifact_sha256);
    d.activate();
    let results = feed(&mut d, FRESH, 201, 100_000);
    assert_eq!(results.len(), 3);
    assert_eq!(d.ordinal, 3);
    assert_eq!(d.fences.len(), 2);
    assert!(d.input.is_none());
    assert!(d.counts.is_empty());
    for (i, status) in results.iter().enumerate() {
        let t = status.last_completed.as_ref().unwrap();
        assert_eq!(t.ordinal, i as u64);
        assert!(t.min_observed_span_ns.parse::<u64>().unwrap() >= 2_000_000_000);
        assert_eq!(t.min_source_rows, 67);
        assert_eq!(t.max_source_rows, 67);
        assert!(status
            .current_prediction
            .as_ref()
            .is_some_and(|p| !p.physically_validated));
    }
    // Independently call the SAME opaque builder and model infer on the exact
    // first source interval, without access to the inner v1 model.
    let mut input = model.live_input("comparison-window".into()).unwrap();
    input
        .begin_window(FRESH + 1000, FRESH + 3_000_000_000)
        .unwrap();
    let mut now = FRESH;
    'ready: for row in 0..67 {
        for source in 1..=2 {
            now = FRESH + row as u64 * PERIOD + u64::from(source) * 1000;
            if input
                .ingest(&packet(
                    source,
                    100_000 + row as u32,
                    now,
                    RoomLabel::Unknown,
                    row,
                ))
                .unwrap()
                == WindowAdmission::Ready
            {
                break 'ready;
            }
        }
    }
    let expected = model.predict(input.finish(now).unwrap(), now).unwrap();
    assert_eq!(
        serde_json::to_value(expected).unwrap(),
        serde_json::to_value(results[0].current_prediction.as_ref().unwrap()).unwrap()
    );
    assert!(
        results[1]
            .last_completed
            .as_ref()
            .unwrap()
            .interval_since_previous_ns
            .as_ref()
            .unwrap()
            .parse::<u64>()
            .unwrap()
            >= 2_000_000_000
    );
    let next = FRESH + 201 * PERIOD + 1000;
    let status = d.ingest(&packet(1, 100_201, next, RoomLabel::Unknown, 201), next);
    assert_eq!(status.buffer_state, LiveState::Warming);
    assert!(status.current_prediction.is_some());
    assert_eq!(
        status.observation.availability,
        ObservationAvailability::Available
    );
    assert_eq!(status.last_completed.as_ref().unwrap().ordinal, 2);
}
#[test]
fn replay_clock_and_sequence_fences_survive_window_rollover_and_reactivation() {
    let mut d = driver();
    d.activate();
    assert_eq!(feed(&mut d, FRESH, 67, 100_000).len(), 1);
    let now = FRESH + 67 * PERIOD + 1000;
    let status = d.ingest(&packet(1, 100_067, now, RoomLabel::Unknown, 67), now);
    assert_eq!(status.buffer_state, LiveState::Warming);
    let old = packet(
        2,
        100_066,
        FRESH + 66 * PERIOD + 2000,
        RoomLabel::Unknown,
        66,
    );
    let status = d.ingest(&old, now);
    assert_eq!(status.buffer_state, LiveState::Withdrawn);
    assert_eq!(status.reason, Some("replayed_or_reversed_packet"));
    assert!(status.current_prediction.is_none());
    d.deactivate();
    d.activate();
    let status = d.ingest(&old, now + 1);
    assert_eq!(status.reason, Some("replayed_or_reversed_packet"));
    assert_eq!(d.fences.len(), 2);
}
#[test]
fn source_gap_and_polling_withdraw_without_renewing_source_freshness_then_fresh_warmup_recovers() {
    let mut d = driver();
    d.activate();
    feed(&mut d, FRESH, 10, 100_000);
    let last = FRESH + 9 * PERIOD + 2000;
    let status = d.poll(last + 100_000_000);
    assert_eq!(status.buffer_state, LiveState::Withdrawn);
    assert_eq!(status.reason, Some("source_gap"));
    assert!(status.current_prediction.is_none());
    let fences = d.fences.len();
    d.poll(last + 200_000_000);
    assert_eq!(d.fences.len(), fences);
    let fresh = last + 300_000_000;
    let results = feed(&mut d, fresh, 67, 100_010);
    assert_eq!(results.len(), 1);
    let at = fresh + 66 * PERIOD + 2000;
    let status = d.poll(at + 100_000_000);
    assert_eq!(status.buffer_state, LiveState::Withdrawn);
    assert!(status.current_prediction.is_none());
    assert_eq!(status.last_completed.as_ref().unwrap().ordinal, 0);
}
#[test]
fn cold_training_fences_invalid_phy_stale_and_reversed_receiver_clock_never_emit_prediction() {
    let mut d = driver();
    d.activate();
    let old = packet(1, 1, FRESH + 1000, RoomLabel::Unknown, 0);
    let status = d.ingest(&old, FRESH + 1000);
    assert_eq!(status.reason, Some("source_or_window_invalid"));
    assert!(d.fences.is_empty());
    for kind in ["grid", "frequency", "source", "stale", "clock"] {
        let mut d = driver();
        d.activate();
        feed(&mut d, FRESH, 8, 100_000);
        let last = FRESH + 7 * PERIOD + 2000;
        let mut p = packet(1, 100_008, FRESH + 8 * PERIOD + 1000, RoomLabel::Unknown, 8);
        let mut now = p.native_frame.timestamp_ns;
        match kind {
            "grid" => p.grid_id = "wrong-grid".into(),
            "frequency" => p.frequency_hz = 5_180_000_000,
            "source" => p.source_id = 99,
            "stale" => now += 6_000_000_000,
            "clock" => now = last - 1,
            _ => unreachable!(),
        }
        let before = d.fences[&1].sequence;
        let status = d.ingest(&p, now);
        assert_eq!(status.buffer_state, LiveState::Withdrawn, "{kind}");
        assert!(status.current_prediction.is_none());
        assert_eq!(
            d.fences[&1].sequence, before,
            "invalid packet advanced fence: {kind}"
        );
    }
}

#[test]
fn repeated_poll_clock_reversal_cannot_lower_the_existing_receiver_floor() {
    let mut d = driver();
    d.activate();
    feed(&mut d, FRESH, 8, 100_000);
    let floor = FRESH + 7 * PERIOD + 2000;
    for now in [floor - 1, floor - 100_000_000, floor - 2] {
        let s = d.poll(now);
        assert_eq!(s.reason, Some("receiver_clock_reversed"));
        assert_eq!(d.last_now, Some(floor));
        assert!(s.current_prediction.is_none());
    }
    d.deactivate();
    d.activate();
    assert_eq!(d.poll(floor - 1).reason, Some("receiver_clock_reversed"));
    assert_eq!(d.last_now, Some(floor));
    let now = floor + PERIOD;
    let s = d.ingest(&packet(1, 100_008, now, RoomLabel::Unknown, 8), now);
    assert_eq!(s.buffer_state, LiveState::Warming);
    assert!(s.current_prediction.is_none());
}
#[test]
fn source_dropout_across_a_completed_window_boundary_withdraws_the_busy_source() {
    let mut d = driver();
    d.activate();
    assert_eq!(feed(&mut d, FRESH, 67, 100_000).len(), 1);
    let next = FRESH + 67 * PERIOD + 1000;
    assert_eq!(
        d.ingest(&packet(1, 100_067, next, RoomLabel::Unknown, 67), next)
            .buffer_state,
        LiveState::Warming
    );
    let later = FRESH + 68 * PERIOD + 1000;
    let s = d.ingest(&packet(1, 100_068, later, RoomLabel::Unknown, 68), later);
    assert_eq!(s.buffer_state, LiveState::Withdrawn);
    assert_eq!(s.reason, Some("source_gap"));
    assert!(s.current_prediction.is_none());
    assert_eq!(s.last_completed.as_ref().unwrap().ordinal, 0);
    assert_eq!(d.fences[&1].sequence, 100_067);
    assert_eq!(d.fences[&2].sequence, 100_066);
}
#[test]
fn fresh_timestamps_do_not_authorize_a_counter_reset_after_rollover() {
    let mut d = driver();
    d.activate();
    assert_eq!(feed(&mut d, FRESH, 67, 100_000).len(), 1);
    let now = FRESH + 67 * PERIOD + 1000;
    let reset = packet(1, 100_000, now, RoomLabel::Unknown, 67);
    let s = d.ingest(&reset, now);
    assert_eq!(s.reason, Some("replayed_or_reversed_packet"));
    assert_eq!(s.buffer_state, LiveState::Withdrawn);
    assert!(s.current_prediction.is_none());
    assert_eq!(d.fences[&1].sequence, 100_066);
}
#[test]
fn invalid_waveform_values_or_shape_withdraw_for_each_bound_source_without_advancing_fences() {
    for source in 1..=2 {
        for kind in ["nan", "shape"] {
            let mut d = driver();
            d.activate();
            feed(&mut d, FRESH, 8, 100_000);
            let now = FRESH + 8 * PERIOD + u64::from(source) * 1000;
            let mut bad = packet(source, 100_008, now, RoomLabel::Unknown, 8);
            if kind == "nan" {
                bad.amplitudes[0] = f64::NAN;
            } else {
                bad.amplitudes.truncate(8);
            }
            let before = d.fences[&source].sequence;
            let s = d.ingest(&bad, now);
            assert_eq!(s.reason, Some("source_or_window_invalid"));
            assert!(s.current_prediction.is_none());
            assert_eq!(d.fences[&source].sequence, before);
        }
    }
}

#[test]
fn continuous_rollover_exceeds_finite_training_schedule_without_retaining_dataset_rows() {
    let mut d = driver();
    let bound = d.model.input_policy().unwrap().resources().cohorts;
    assert_eq!(bound, 32);
    d.activate();
    let results = feed(&mut d, FRESH, (bound + 2) * 67, 100_000);
    assert_eq!(results.len(), bound + 2);
    assert_eq!(d.ordinal, (bound + 2) as u64);
    assert_eq!(d.fences.len(), 2);
    assert_eq!(d.continuity.len(), 2);
    assert!(d.counts.is_empty());
    assert!(d.input.is_none());
    assert_eq!(
        d.status.last_completed.as_ref().unwrap().ordinal,
        (bound + 1) as u64
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(d.model.export().unwrap())),
        d.status.model_sha256
    );
}

// The production portion of the frozen072 driver, verbatim, is test control
// only. It calls the SAME opaque model/infer; no parallel trainer is introduced.
use crate::room_learner::{model, native, observation, temporal_acquisition, temporal_model_input};
#[allow(dead_code)]
#[path = "temporal_live_driver_prior_control_tests.rs"]
mod prior_control;

#[test]
fn healthy_two_second_stream_retains_original_observation_during_collection_and_improves_duty_cycle(
) {
    let model = fitted();
    let mut d = ContinuousTemporalInput::new(model.clone()).unwrap();
    let mut old = prior_control::ContinuousTemporalInput::new(model).unwrap();
    d.activate();
    old.activate();
    let mut previous = None;
    let mut elapsed_new = 0u64;
    let mut elapsed_old = 0u64;
    let mut first_complete = None;
    let mut digest = None;
    let mut deadline = None;
    for row in 0..201 {
        for source in 1..=2 {
            let now = FRESH + row as u64 * PERIOD + u64::from(source) * 1000;
            if let Some((time, new_available, old_available)) = previous {
                if new_available {
                    elapsed_new += now - time;
                }
                if old_available {
                    elapsed_old += now - time;
                }
            }
            let p = packet(source, 100_000 + row as u32, now, RoomLabel::Unknown, row);
            let new = d.ingest(&p, now);
            let control = old.ingest(&p, now);
            if row < 66 {
                assert!(new.current_prediction.is_none());
            }
            if row == 66 && source == 2 {
                first_complete = Some(now);
                digest = new.observation.digest.clone();
                deadline = new.observation.expires_ns.clone();
            }
            if row == 67 {
                assert_eq!(new.buffer_state, LiveState::Warming);
                assert_eq!(
                    new.observation.availability,
                    ObservationAvailability::Available
                );
                assert_eq!(new.observation.digest, digest);
                assert_eq!(new.observation.expires_ns, deadline);
                assert!(new.current_prediction.is_some());
            }
            previous = Some((
                now,
                new.current_prediction.is_some(),
                control.current_prediction.is_some(),
            ));
        }
    }
    let end = FRESH + 200 * PERIOD + 2000;
    assert_eq!(elapsed_new, end - first_complete.unwrap());
    assert!(elapsed_old * 20 < elapsed_new);
    assert_eq!(d.ordinal, 3);
}
#[test]
fn first_row_deadline_is_fixed_exact_and_expiry_preserves_a_healthy_long_window() {
    let model = fitted_id(ExperimentalPlanId::Native3900msMax4000Slots2);
    let mut d = ContinuousTemporalInput::new(model).unwrap();
    d.activate();
    let completed = feed(&mut d, FRESH, 130, 100_000);
    assert_eq!(completed.len(), 1);
    let initial = completed[0].observation.clone();
    let expires = initial.expires_ns.as_ref().unwrap().parse::<u64>().unwrap();
    let first = initial
        .first_receiver_ns
        .as_ref()
        .unwrap()
        .parse::<u64>()
        .unwrap();
    assert_eq!(expires, first + MAX_AGE_NS);
    // Keep all sources healthy, collecting the next3.9s window, through the old
    // result's5s FIRST-row deadline; its LAST-row+5s bound would be much later.
    for row in 130..165 {
        for source in 1..=2 {
            let now = FRESH + row as u64 * PERIOD + u64::from(source) * 1000;
            let s = d.ingest(
                &packet(source, 100_000 + row as u32, now, RoomLabel::Unknown, row),
                now,
            );
            assert_eq!(s.observation.digest, initial.digest);
            assert_eq!(s.observation.expires_ns, initial.expires_ns);
        }
    }
    assert!(d.input.is_some());
    let count = d.counts.values().map(|c| c.rows).sum::<usize>();
    let at = d.poll(expires);
    assert_eq!(
        at.observation.availability,
        ObservationAvailability::Available
    );
    assert_eq!(at.observation.age_ns.as_deref(), Some("5000000000"));
    let expired = d.poll(expires + 1);
    assert_eq!(expired.buffer_state, LiveState::Warming);
    assert_eq!(
        expired.observation.availability,
        ObservationAvailability::Unavailable
    );
    assert_eq!(
        expired.observation.reason,
        Some("original_first_row_expired")
    );
    assert!(expired.current_prediction.is_none());
    assert!(d.input.is_some());
    assert_eq!(d.counts.values().map(|c| c.rows).sum::<usize>(), count);
    assert_eq!(d.ordinal, 1);
    for _ in 0..3 {
        let same = d.poll(expires + 1);
        assert_eq!(same.observation.digest, initial.digest);
        assert_eq!(same.observation.expires_ns, initial.expires_ns);
        assert!(same.current_prediction.is_none());
    }
    let next = FRESH + 165 * PERIOD + 1000;
    let after_packet = d.ingest(&packet(1, 100_165, next, RoomLabel::Unknown, 165), next);
    assert_eq!(after_packet.observation.digest, initial.digest);
    assert_eq!(after_packet.observation.expires_ns, initial.expires_ns);
    assert!(after_packet.current_prediction.is_none());
    assert!(
        expires
            < initial
                .completed_ns
                .as_ref()
                .unwrap()
                .parse::<u64>()
                .unwrap()
                + MAX_AGE_NS,
        "causal first-vs-last boundary missing"
    );
}
#[test]
fn gap_invalidity_and_deactivation_cannot_resurrect_a_retained_observation() {
    for mode in ["gap", "invalid", "deactivate"] {
        let mut d = driver();
        d.activate();
        let complete = feed(&mut d, FRESH, 67, 100_000);
        assert!(complete[0].current_prediction.is_some());
        let now = FRESH + 67 * PERIOD + 1000;
        let s = match mode {
            "gap" => d.poll(now + 100_000_000),
            "invalid" => {
                let mut bad = packet(1, 100_067, now, RoomLabel::Unknown, 67);
                bad.amplitudes[0] = f64::NAN;
                d.ingest(&bad, now)
            }
            _ => {
                d.deactivate();
                d.poll(now)
            }
        };
        assert!(s.current_prediction.is_none());
        assert!(s.observation.digest.is_none());
        assert!(d.observation.is_none());
        d.activate();
        let reactivated = d.poll(now + 100_000_001);
        assert!(reactivated.current_prediction.is_none());
        assert!(reactivated.observation.digest.is_none());
    }
}

#[test]
fn repeated_available_reads_age_the_same_observation_without_changing_identity_or_deadline() {
    let mut d = driver();
    d.activate();
    let completed = feed(&mut d, FRESH, 67, 100_000);
    let initial = completed[0].observation.clone();
    let now = FRESH + 66 * PERIOD + 2000;
    for offset in [1, 10, 1000, 1_000_000] {
        let status = d.poll(now + offset);
        assert_eq!(
            status.observation.availability,
            ObservationAvailability::Available
        );
        assert_eq!(status.observation.digest, initial.digest);
        assert_eq!(
            status.observation.first_receiver_ns,
            initial.first_receiver_ns
        );
        assert_eq!(status.observation.completed_ns, initial.completed_ns);
        assert_eq!(status.observation.expires_ns, initial.expires_ns);
        assert_eq!(
            status
                .observation
                .age_ns
                .as_ref()
                .unwrap()
                .parse::<u64>()
                .unwrap(),
            now + offset
                - initial
                    .first_receiver_ns
                    .as_ref()
                    .unwrap()
                    .parse::<u64>()
                    .unwrap()
        );
        assert_eq!(d.ordinal, 1);
        assert!(!status.physically_validated && !status.adoption_authorized);
    }
    // A model replacement is a new inactive owner-scoped driver; it cannot borrow
    // a cached result merely because the cold bytes/model policy are identical.
    let replacement = ContinuousTemporalInput::new(fitted()).unwrap();
    assert!(replacement.observation.is_none());
    assert!(replacement.status.current_prediction.is_none());
    assert_eq!(replacement.status.buffer_state, LiveState::Inactive);
}
