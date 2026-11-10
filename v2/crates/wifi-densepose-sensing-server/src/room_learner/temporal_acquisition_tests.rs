//! SYNTHETIC original native IQ through the existing native adapter and SourceView
//! validators. No physical timing/accuracy or integrated v2 runtime claim.
use super::super::native::{canonicalize_esp32, NativePacketInput};
use super::*;
use wifi_densepose_hardware::{Adr018Flags, PpduType};

fn context(count: u8) -> CaptureContext {
    CaptureContext {
        owner_scope: "a".repeat(64),
        installation_id: "synthetic-install".into(),
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
#[test]
fn c5_claimed_feature_grid_plan_keeps_exact_frequency_and_rejects_unversioned_shapes() {
    let mut ctx=context(3);
    for source in &mut ctx.sources {
        source.grid_id=super::super::native::C5_LLTF8_V1_GRID.into();
        source.center_frequency_hz=5_560_000_000;
    }
    let cadences=[(1,ReceiverCadenceBand::Hz30To36),(2,ReceiverCadenceBand::Hz30To36),(3,ReceiverCadenceBand::Hz30To36)];
    let p=AcquisitionPlan::build(ctx.clone(),ExperimentalPlanId::Native1000msSlots2,&cadences).unwrap();
    assert_eq!(p.context,ctx);
    for mhz in [2437u64,5185,5955] {
        let mut bad=ctx.clone();bad.sources[0].center_frequency_hz=mhz*1_000_000;
        assert!(AcquisitionPlan::build(bad,ExperimentalPlanId::Native1000msSlots2,&cadences).is_err());
    }
    let mut bad=ctx;bad.sources[0].grid_id="ant1-bins53-ppdu0-usable-drop2-linear-v2".into();
    assert!(AcquisitionPlan::build(bad,ExperimentalPlanId::Native1000msSlots2,&cadences).is_err());
}
fn plan(
    count: u8,
    id: ExperimentalPlanId,
    band: ReceiverCadenceBand,
) -> Result<AcquisitionPlan, String> {
    AcquisitionPlan::build(
        context(count),
        id,
        &(1..=count).map(|n| (n, band)).collect::<Vec<_>>(),
    )
}
fn packet(source: u8, sequence: u32, at: u64, hz: f64) -> CanonicalPacket {
    let bytes = (0..64).flat_map(|_| [3u8, 4]).collect::<Vec<_>>();
    canonicalize_esp32(NativePacketInput {
        node_id: source,
        n_antennas: 1,
        raw_bins: 64,
        frequency_hz: 2_437_000_000,
        ppdu_type: PpduType::HtLegacy,
        sequence,
        received_at_ns: at,
        receiver_sample_rate_hz: hz,
        iq_bytes: &bytes,
        flags: Adr018Flags::default(),
        provenance: CaptureProvenance::Synthetic,
    })
    .unwrap()
}
fn collect(
    assembler: &mut NativeWindowAssembler,
    target: u64,
    hz: u64,
    sequence: &mut u32,
) -> TemporalWindow {
    let period = 1_000_000_000u64.div_ceil(hz);
    let end = target + 6_000_000_000;
    assembler.begin_window(target, end).unwrap();
    for index in 0..MAX_ROWS {
        *sequence += 1;
        for source in 1..=assembler.policy.context.sources.len() as u8 {
            let last = target + index as u64 * period + u64::from(source) * 1000;
            let result = assembler
                .ingest(&packet(source, *sequence, last, hz as f64))
                .unwrap();
            if result == WindowAdmission::Ready {
                return assembler.finish_window(last).unwrap();
            }
        }
    }
    panic!("window did not reach its actual horizon");
}
#[test]
fn immutable_plan_hash_and_actual_max_capacity_are_context_cadence_and_variant_bound() {
    use ExperimentalPlanId::*;
    use ReceiverCadenceBand::*;
    let a = plan(3, Native2000msSlots8, Hz30To36).unwrap();
    let b = plan(3, Native2000msSlots8, Hz30To36).unwrap();
    assert_eq!(a.sha256(), b.sha256());
    assert_eq!(a.resources().cohorts, 128);
    assert_eq!(a.source_policies()[0].max_rows, 77);
    assert_eq!(a.resources().max_numeric_values, 128 * 3 * 56 * 77);
    assert!(a.resources().serialized_upper_bound_bytes <= SERIALIZED_LIMIT);
    assert_eq!(
        a.schedule()
            .iter()
            .filter(|s| s.role == DataRole::Train)
            .count(),
        8
    );
    assert_eq!(
        a.schedule()
            .iter()
            .filter(|s| s.role == DataRole::HeldOut)
            .count(),
        8
    );
    for label in [
        RoomLabel::Empty,
        RoomLabel::OccupiedStatic,
        RoomLabel::OccupiedMoving,
        RoomLabel::AdjacentOnly,
    ] {
        for role in [DataRole::Train, DataRole::HeldOut] {
            assert_eq!(
                a.schedule()
                    .iter()
                    .filter(|s| s.role == role && s.label == label)
                    .count(),
                2
            );
        }
    }
    assert!(plan(2, Native2000msSlots8, Hz45To55).is_ok());
    assert_eq!(
        plan(3, Native2000msSlots8, Hz45To55).unwrap_err(),
        "acquisition_numeric_capacity"
    );
    assert!(plan(3, Native2000msSlots4, Hz45To55).is_ok());
    assert!(plan(8, Native2000msSlots4, Hz30To36).is_err());
    let smaller = plan(8, Native2000msSlots2, Hz30To36).unwrap();
    assert_eq!(smaller.resources().cohorts, 32);
    assert_ne!(
        a.sha256(),
        plan(2, Native2000msSlots8, Hz30To36).unwrap().sha256()
    );
    assert_ne!(
        a.sha256(),
        plan(3, Native2000msSlots4, Hz30To36).unwrap().sha256()
    );
    let mut ctx = context(2);
    ctx.sources[1].source_id = 1;
    assert!(
        AcquisitionPlan::build(ctx, Native1000msSlots2, &[(1, Hz30To36), (1, Hz30To36)]).is_err()
    );
    assert!(AcquisitionPlan::build(
        context(2),
        Native1000msSlots2,
        &[(2, Hz30To36), (1, Hz30To36)]
    )
    .is_err());
    let mut ctx = context(2);
    ctx.sources[0].feature_count = 64;
    assert!(
        AcquisitionPlan::build(ctx, Native1000msSlots2, &[(1, Hz30To36), (2, Hz30To36)]).is_err()
    );
}
#[test]
fn actual_two_seconds_at_33_and_50_hz_preserve_all_original_rows_phy_and_frame_identity() {
    for (hz, band, rows) in [
        (33, ReceiverCadenceBand::Hz30To36, 67),
        (50, ReceiverCadenceBand::Hz45To55, 101),
    ] {
        let p = plan(2, ExperimentalPlanId::Native2000msSlots2, band).unwrap();
        let hash = p.sha256().to_string();
        let mut a = NativeWindowAssembler::new(p, "synthetic-run".into()).unwrap();
        let window = collect(&mut a, 10_000_000_000, hz, &mut 0);
        assert_eq!(window.policy_sha256, hash);
        assert_eq!(window.cohort.role, DataRole::Train);
        assert_eq!(window.cohort.provenance, CaptureProvenance::Synthetic);
        assert!(window.cohort.teacher.is_none());
        for (view, (source, times)) in window.cohort.views.iter().zip(&window.row_times_ns) {
            assert_eq!(view.source_id, *source);
            assert_eq!(view.samples.len(), rows);
            assert_eq!(
                view.last_time_ns - view.first_time_ns,
                times.last().unwrap() - times[0]
            );
            assert!(view.last_time_ns - view.first_time_ns >= 2_000_000_000);
            assert_eq!(view.center_frequency_hz, 2_437_000_000);
            assert_eq!(view.bandwidth_hz, 20_000_000);
            assert_eq!(view.clock.kind, ClockKind::ReceiverOnly);
            assert_eq!(view.clock.uncertainty_ns, u64::MAX);
            for (index, ((sequence, frame), row)) in view
                .sequence_ids
                .iter()
                .zip(&view.frame_ids)
                .zip(&view.samples)
                .enumerate()
            {
                let original = packet(*source, *sequence as u32, times[index], hz as f64);
                assert_eq!(*frame, original.native_frame.frame_id);
                assert_eq!(
                    row,
                    &original
                        .amplitudes
                        .iter()
                        .map(|n| *n as f32)
                        .collect::<Vec<_>>()
                );
                assert!(row.iter().all(|v| *v == 5.0));
            }
        }
    }
}
#[test]
fn fixed_64_rows_cannot_claim_two_seconds_and_legacy_eight_rows_are_not_upgraded() {
    for count in [8, 64] {
        for hz in [33, 50] {
            let band = if hz == 33 {
                ReceiverCadenceBand::Hz30To36
            } else {
                ReceiverCadenceBand::Hz45To55
            };
            let p = plan(2, ExperimentalPlanId::Native2000msSlots2, band).unwrap();
            let mut a = NativeWindowAssembler::new(p, "short-control".into()).unwrap();
            let target = 10_000_000_000;
            a.begin_window(target, target + 3_000_000_000).unwrap();
            let period = 1_000_000_000u64.div_ceil(hz);
            for index in 0..count {
                for source in 1..=2 {
                    assert_eq!(
                        a.ingest(&packet(
                            source,
                            index + 1,
                            target + u64::from(index) * period + u64::from(source) * 1000,
                            hz as f64
                        ))
                        .unwrap(),
                        WindowAdmission::Collecting
                    );
                }
            }
            assert_eq!(
                a.finish_window(target + u64::from(count) * period)
                    .err()
                    .as_deref(),
                Some("acquisition_horizon_incomplete")
            );
        }
    }
}
#[test]
fn gaps_replay_wrong_source_grid_phy_age_and_skew_fail_without_downsampling() {
    for failure in [
        "gap",
        "sequence",
        "frame",
        "source",
        "grid",
        "phy",
        "nonfinite",
        "cadence",
        "fast_actual",
        "provenance",
    ] {
        let p = plan(
            2,
            ExperimentalPlanId::Native1000msSlots2,
            ReceiverCadenceBand::Hz30To36,
        )
        .unwrap();
        let mut a = NativeWindowAssembler::new(p, "negative".into()).unwrap();
        let t = 10_000_000_000;
        a.begin_window(t, t + 2_000_000_000).unwrap();
        let first = packet(1, 1, t + 1000, 33.);
        let original = first.native_frame.frame_id;
        a.ingest(&first).unwrap();
        let mut bad = packet(1, 2, t + 30_304_000, 33.);
        match failure {
            "gap" => bad = packet(1, 2, t + 100_000_000, 33.),
            "sequence" => bad.sequence = 1,
            "frame" => bad.native_frame.frame_id = original,
            "source" => bad.source_id = 99,
            "grid" => bad.grid_id = "fake".into(),
            "phy" => bad.frequency_hz = 5_180_000_000,
            "nonfinite" => bad.amplitudes[0] = f64::NAN,
            "cadence" => bad.receiver_sample_rate_hz = 50.,
            "fast_actual" => bad = packet(1, 2, t + 20_000_000, 33.),
            "provenance" => bad.provenance = CaptureProvenance::LiveReceiverObserved,
            _ => unreachable!(),
        }
        assert!(a.ingest(&bad).is_err(), "{failure}");
        assert_eq!(
            a.active.as_ref().unwrap().rows[&1].len(),
            1,
            "{failure} replaced a row"
        );
        assert!(a
            .begin_window(t + 3_000_000_000, t + 5_000_000_000)
            .is_err());
    }
    let mut a = NativeWindowAssembler::new(
        plan(
            2,
            ExperimentalPlanId::Native3900msMax4000Slots2,
            ReceiverCadenceBand::Hz30To36,
        )
        .unwrap(),
        "age".into(),
    )
    .unwrap();
    let w = collect(&mut a, 10_000_000_000, 33, &mut 0);
    assert!(w
        .cohort
        .views
        .iter()
        .all(|v| v.last_time_ns - v.first_time_ns >= 3_900_000_000
            && v.last_time_ns - v.first_time_ns <= 4_000_000_000));
    let mut a = NativeWindowAssembler::new(
        plan(
            2,
            ExperimentalPlanId::Native3900msMax4000Slots2,
            ReceiverCadenceBand::Hz30To36,
        )
        .unwrap(),
        "stale".into(),
    )
    .unwrap();
    let t = 10_000_000_000;
    a.begin_window(t, t + 7_000_000_000).unwrap();
    let period = 1_000_000_000u64.div_ceil(33);
    let mut last = t;
    'ready: for index in 0..MAX_ROWS {
        for source in 1..=2 {
            last = t + index as u64 * period + u64::from(source) * 1000;
            if a.ingest(&packet(source, index as u32 + 1, last, 33.))
                .unwrap()
                == WindowAdmission::Ready
            {
                break 'ready;
            }
        }
    }
    assert!(last - t <= 4_000_000_000);
    assert_eq!(
        a.finish_window(t + 5_000_000_001 + 1000).err().as_deref(),
        Some("acquisition_first_packet_age")
    );
}
#[test]
fn schedule_roles_and_original_sequence_fences_survive_multiple_windows() {
    let ctx = context(2);
    let p = plan(
        2,
        ExperimentalPlanId::Native1000msSlots2,
        ReceiverCadenceBand::Hz30To36,
    )
    .unwrap();
    let mut a = NativeWindowAssembler::new(p, "role-run".into()).unwrap();
    let mut rows = vec![];
    let mut sequence = 0;
    for index in 0..32 {
        let w = collect(
            &mut a,
            10_000_000_000 + index * 3_000_000_000,
            33,
            &mut sequence,
        );
        assert_eq!(
            w.cohort.role,
            if index < 16 {
                DataRole::Train
            } else {
                DataRole::HeldOut
            }
        );
        rows.push(w.cohort);
    }
    let valid = ValidatedDataset::new(ctx.clone(), rows.clone()).unwrap();
    assert_eq!(valid.train().len(), 16);
    assert_eq!(valid.held_out().len(), 16);
    assert!(!valid.measurement_clock_available());
    rows[16].views[0].frame_ids[0] = rows[0].views[0].frame_ids[0];
    assert!(ValidatedDataset::new(ctx, rows).is_err());
    assert!(a.begin_window(200_000_000_000, 202_000_000_000).is_err());
    let p = plan(
        2,
        ExperimentalPlanId::Native1000msSlots2,
        ReceiverCadenceBand::Hz30To36,
    )
    .unwrap();
    let mut a = NativeWindowAssembler::new(p, "replay".into()).unwrap();
    collect(&mut a, 10_000_000_000, 33, &mut 0);
    a.begin_window(20_000_000_000, 22_000_000_000).unwrap();
    assert_eq!(
        a.ingest(&packet(1, 1, 20_000_001_000, 33.))
            .err()
            .as_deref(),
        Some("acquisition_replayed_or_reversed")
    );
}
#[test]
fn resource_upper_bound_includes_max_rows_and_actual_json_not_just_float_memory() {
    let p = plan(
        3,
        ExperimentalPlanId::Native2000msSlots8,
        ReceiverCadenceBand::Hz30To36,
    )
    .unwrap();
    let resources = p.resources();
    assert!(
        resources.serialized_upper_bound_bytes
            > resources.max_numeric_values * std::mem::size_of::<f32>()
    );
    let mut a = NativeWindowAssembler::new(p.clone(), "serialization".into()).unwrap();
    let mut w = collect(&mut a, 10_000_000_000, 33, &mut 0);
    for view in &mut w.cohort.views {
        let max = p
            .source_policies()
            .iter()
            .find(|s| s.source_id == view.source_id)
            .unwrap()
            .max_rows;
        view.samples = vec![vec![1.2345678e-30f32; 56]; max];
        view.frame_ids = vec![u128::MAX; max];
        view.sequence_ids = vec![u64::MAX; max];
    }
    let one = serde_json::to_vec(&w.cohort).unwrap().len();
    assert!(
        one * resources.cohorts + SERIALIZED_METADATA_RESERVE
            < resources.serialized_upper_bound_bytes
    );
}

#[test]
fn individually_long_enough_sources_still_need_bounded_pairwise_receiver_skew() {
    let p = plan(
        2,
        ExperimentalPlanId::Native1000msSlots2,
        ReceiverCadenceBand::Hz30To36,
    )
    .unwrap();
    let mut a = NativeWindowAssembler::new(p, "skew".into()).unwrap();
    let t = 10_000_000_000;
    a.begin_window(t, t + 2_000_000_000).unwrap();
    let period = 1_000_000_000u64.div_ceil(33);
    let mut now = t;
    'ready: for index in 0..MAX_ROWS {
        for source in 1..=2 {
            now = t + index as u64 * period + if source == 1 { 1000 } else { 80_001_000 };
            if a.ingest(&packet(source, index as u32 + 1, now, 33.))
                .unwrap()
                == WindowAdmission::Ready
            {
                break 'ready;
            }
        }
    }
    assert_eq!(
        a.finish_window(now).err().as_deref(),
        Some("acquisition_source_skew")
    );
}
#[test]
fn capacity_failure_is_terminal_and_does_not_evict_or_shorten_the_horizon() {
    let p = plan(
        2,
        ExperimentalPlanId::Native2000msSlots2,
        ReceiverCadenceBand::Hz30To36,
    )
    .unwrap();
    let capacity = p.source_policies()[0].max_rows;
    let mut a = NativeWindowAssembler::new(p, "capacity".into()).unwrap();
    let t = 10_000_000_000;
    a.begin_window(t, t + 3_000_000_000).unwrap();
    a.ingest(&packet(1, 1, t + 1000, 33.)).unwrap();
    // Boundary fault injection of a full buffer: the public admission operation
    // must fail rather than pop old rows, regardless of any future caller bug.
    let original = a.active.as_ref().unwrap().rows[&1][0].frame_id;
    let buffer = a.active.as_mut().unwrap().rows.get_mut(&1).unwrap();
    while buffer.len() < capacity {
        buffer.push(PacketRow {
            received_ns: t + 1000,
            frame_id: original,
            sequence: 1,
            values: vec![5.; 56],
        });
    }
    assert_eq!(
        a.ingest(&packet(1, 2, t + 30_304_000, 33.))
            .err()
            .as_deref(),
        Some("acquisition_row_capacity")
    );
    assert_eq!(a.active.as_ref().unwrap().rows[&1].len(), capacity);
    assert_eq!(a.active.as_ref().unwrap().rows[&1][0].frame_id, original);
    assert!(a.finish_window(t + 2_100_000_000).is_err());
}

#[test]
fn four_second_native_ceiling_is_independent_of_the_existing_five_second_age_guard() {
    let p = plan(
        2,
        ExperimentalPlanId::Native3900msMax4000Slots2,
        ReceiverCadenceBand::Hz30To36,
    )
    .unwrap();
    assert_eq!(p.max_observed_span_ms(), 4000);
    let mut a = NativeWindowAssembler::new(p, "four-second-limit".into()).unwrap();
    let t = 10_000_000_000;
    a.begin_window(t, t + 6_000_000_000).unwrap();
    let period = 1_000_000_000u64.div_ceil(33);
    'ready: for index in 0..MAX_ROWS {
        for source in 1..=2 {
            if a.ingest(&packet(
                source,
                index as u32 + 1,
                t + index as u64 * period + u64::from(source) * 1000,
                33.,
            ))
            .unwrap()
                == WindowAdmission::Ready
            {
                break 'ready;
            }
        }
    }
    // Corrupted/overlong buffer negative control: the inherited SourceView
    // validator permits up to60s, so v2's own <=4s guard must independently win.
    let rows = a.active.as_mut().unwrap().rows.get_mut(&1).unwrap();
    let first = rows[0].received_ns;
    rows.last_mut().unwrap().received_ns = first + 4_000_000_001;
    assert_eq!(
        a.finish_window(t + 4_100_000_000).err().as_deref(),
        Some("acquisition_observed_span")
    );
}

#[test]
fn approved_variants_preflight_every_supported_source_count_without_silent_fallback() {
    use ExperimentalPlanId::*;
    for count in 2..=16 {
        for id in [
            Native1000msSlots8,
            Native1000msSlots4,
            Native1000msSlots2,
            Native2000msSlots8,
            Native2000msSlots4,
            Native2000msSlots2,
            Native3900msMax4000Slots8,
            Native3900msMax4000Slots4,
            Native3900msMax4000Slots2,
        ] {
            for band in [ReceiverCadenceBand::Hz30To36, ReceiverCadenceBand::Hz45To55] {
                if let Ok(plan) = plan(count, id, band) {
                    assert_eq!(plan.data.id, id);
                    assert!(plan.resources().cohorts <= MAX_COHORTS);
                    assert!(plan.resources().max_numeric_values <= MAX_TOTAL_VALUES);
                    assert!(plan.resources().serialized_upper_bound_bytes <= SERIALIZED_LIMIT);
                    assert!(plan
                        .source_policies()
                        .iter()
                        .all(|s| s.max_rows <= MAX_ROWS));
                    assert_eq!(plan.encoder_config().source_count, usize::from(count));
                    assert_eq!(plan.encoder_config().temporal_tokens, 4);
                }
            }
        }
    }
}
#[test]
fn mixed_original_ht_and_he_phy_grids_remain_distinct_in_the_same_temporal_window() {
    let mut ctx = context(2);
    ctx.sources[1].grid_id = "ant1-bins256-ppdu1-usable-drop2-linear-v2".into();
    let p = AcquisitionPlan::build(
        ctx,
        ExperimentalPlanId::Native1000msSlots2,
        &[
            (1, ReceiverCadenceBand::Hz30To36),
            (2, ReceiverCadenceBand::Hz30To36),
        ],
    )
    .unwrap();
    let mut a = NativeWindowAssembler::new(p, "mixed-native".into()).unwrap();
    let t = 10_000_000_000;
    a.begin_window(t, t + 2_000_000_000).unwrap();
    let period = 1_000_000_000u64.div_ceil(33);
    for index in 0..MAX_ROWS {
        for source in 1..=2 {
            let bins = if source == 1 { 64 } else { 256 };
            let bytes = (0..bins).flat_map(|_| [3u8, 4]).collect::<Vec<_>>();
            let packet = canonicalize_esp32(NativePacketInput {
                node_id: source,
                n_antennas: 1,
                raw_bins: bins,
                frequency_hz: 2_437_000_000,
                ppdu_type: if source == 1 {
                    PpduType::HtLegacy
                } else {
                    PpduType::HeSu
                },
                sequence: index as u32 + 1,
                received_at_ns: t + index as u64 * period + u64::from(source) * 1000,
                receiver_sample_rate_hz: 33.,
                iq_bytes: &bytes,
                flags: Adr018Flags::default(),
                provenance: CaptureProvenance::Synthetic,
            })
            .unwrap();
            let now = packet.native_frame.timestamp_ns;
            if a.ingest(&packet).unwrap() == WindowAdmission::Ready {
                let w = a.finish_window(now).unwrap();
                assert!(w.cohort.views[0].grid_id.contains("bins64-ppdu0"));
                assert!(w.cohort.views[1].grid_id.contains("bins256-ppdu1"));
                assert!(w
                    .cohort
                    .views
                    .iter()
                    .all(|v| v.samples.len() == 34 && v.samples.iter().all(|r| r.len() == 56)));
                return;
            }
        }
    }
    panic!("mixed source window incomplete");
}

#[test]
fn a_failed_finish_cannot_be_retried_with_an_earlier_clock_or_more_packets() {
    let p = plan(
        2,
        ExperimentalPlanId::Native1000msSlots2,
        ReceiverCadenceBand::Hz30To36,
    )
    .unwrap();
    let mut a = NativeWindowAssembler::new(p, "failed-finish".into()).unwrap();
    let t = 10_000_000_000;
    a.begin_window(t, t + 7_000_000_000).unwrap();
    let period = 1_000_000_000u64.div_ceil(33);
    let mut last = t;
    'ready: for index in 0..MAX_ROWS {
        for source in 1..=2 {
            last = t + index as u64 * period + u64::from(source) * 1000;
            if a.ingest(&packet(source, index as u32 + 1, last, 33.))
                .unwrap()
                == WindowAdmission::Ready
            {
                break 'ready;
            }
        }
    }
    assert_eq!(
        a.finish_window(t + 6_000_000_000).err().as_deref(),
        Some("acquisition_first_packet_age")
    );
    assert_eq!(
        a.finish_window(last).err().as_deref(),
        Some("acquisition_window_state")
    );
    assert!(a.ingest(&packet(1, 200, last + period, 33.)).is_err());
}
