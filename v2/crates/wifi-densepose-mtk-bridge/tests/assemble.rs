//! Assembler behaviour, exercised through the public API.
//!
//! These live outside `src/assemble.rs` to keep that file under the repository's
//! 500-line limit; they use nothing private.

use wifi_densepose_hardware::mediatek_csi::{ChipsetProfile, CsiFlags, CsiPayload, PpduType};
use wifi_densepose_mtk_bridge::assemble::{
    AssemblerConfig, FrameAssembler, GroupBy, DBM_NOT_REPORTED,
};
use wifi_densepose_mtk_bridge::record::{device_id_from_node, MtkRecord};
use wifi_densepose_mtk_bridge::BridgeError;

fn record(ts: u32, tx: u16, rx: u16, n: usize, pkt_sn: Option<u32>) -> MtkRecord {
    let bw_code = if n == 256 { 2 } else { 0 };
    MtkRecord {
        ts,
        ta: [0x02, 0, 0, 0, 0, 1],
        rssi: Some(-50 - rx as i8),
        snr: Some(25),
        bw_code: Some(bw_code), // 0 = BW20, 2 = BW80
        pri_ch_idx: 0,
        rx_mode: Some(8), // MT_PHY_TYPE_HE_SU
        tx_idx: tx,
        rx_idx: rx,
        chain_info: 0,
        ext_info: 0,
        pkt_sn,
        data_i: (0..n).map(|k| (k as i16).wrapping_mul(3)).collect(),
        data_q: (0..n).map(|k| -(k as i16)).collect(),
        trimmed: false,
    }
}

fn assembler() -> FrameAssembler {
    FrameAssembler::new(AssemblerConfig {
        device_id: 0x1122_3344_5566_7788,
        ..Default::default()
    })
}

#[test]
fn two_by_two_mt7981_group_yields_one_frame_with_right_dimensions() {
    let mut a = assembler();
    assert!(a.push(record(10, 0, 0, 256, None)).unwrap().is_none());
    assert!(a.push(record(10, 0, 1, 256, None)).unwrap().is_none());
    assert!(a.push(record(10, 1, 0, 256, None)).unwrap().is_none());
    let frame = a.push(record(10, 1, 1, 256, None)).unwrap().unwrap();

    assert_eq!((frame.tx_count, frame.rx_count), (2, 2));
    assert_eq!(frame.subcarrier_count, 256);
    assert_eq!(frame.payload.len(), 2 * 2 * 256);
    assert_eq!(frame.bandwidth_mhz, 80);
    assert_eq!(frame.subcarrier_spacing_hz, 312_500.0);
    assert_eq!(frame.device_id, 0x1122_3344_5566_7788);
    assert_eq!(frame.timestamp_us, 10);
    assert_eq!(frame.payload.rssi_dbm(), &[-50, -51]);
    assert_eq!(frame.noise_floor_dbm, -75);
    assert_eq!(frame.chipset, ChipsetProfile::Mt7981Mt7976);
}

#[test]
fn payload_is_chain_major_in_tx_then_rx_order() {
    let mut a = assembler();
    for (tx, rx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
        let mut r = record(7, tx, rx, 64, None);
        // Tag every sample with its chain so ordering is observable.
        let tag = (tx * 2 + rx) as i16 * 1000;
        r.data_i = (0..64).map(|k| tag + k as i16).collect();
        r.data_q = vec![0; 64];
        let _ = a.push(r).unwrap();
    }
    let frame = {
        let mut a2 = assembler();
        let mut out = None;
        for (tx, rx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
            let mut r = record(7, tx, rx, 64, None);
            let tag = (tx * 2 + rx) as i16 * 1000;
            r.data_i = (0..64).map(|k| tag + k as i16).collect();
            r.data_q = vec![0; 64];
            out = a2.push(r).unwrap();
        }
        out.unwrap()
    };
    let values = match &frame.payload {
        CsiPayload::ComplexI16 { values, .. } => values,
        _ => panic!("expected ComplexI16"),
    };
    assert_eq!(values[0][0], 0); // tx0 rx0 sc0
    assert_eq!(values[64][0], 1000); // tx0 rx1 sc0
    assert_eq!(values[128][0], 2000); // tx1 rx0 sc0
    assert_eq!(values[192][0], 3000); // tx1 rx1 sc0
}

#[test]
fn bridge_never_sets_synthetic_or_calibrated() {
    let mut a = assembler();
    let mut frame = None;
    for (tx, rx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
        frame = a.push(record(1, tx, rx, 64, None)).unwrap();
    }
    let frame = frame.unwrap();
    assert!(!frame.flags.contains(CsiFlags::SYNTHETIC));
    assert!(!frame.flags.contains(CsiFlags::CALIBRATED));
    assert!(!frame.flags.contains(CsiFlags::TIME_SYNCHRONIZED));
    assert_eq!(frame.calibration_id, 0);
}

#[test]
fn time_synchronized_is_set_only_when_asserted() {
    let mut a = FrameAssembler::new(AssemblerConfig {
        time_synchronized: true,
        ..Default::default()
    });
    let mut frame = None;
    for (tx, rx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
        frame = a.push(record(1, tx, rx, 64, None)).unwrap();
    }
    assert!(frame.unwrap().flags.contains(CsiFlags::TIME_SYNCHRONIZED));
}

#[test]
fn pkt_sn_gap_sets_dropped_predecessor() {
    let mut a = assembler();
    let push_group = |a: &mut FrameAssembler, ts: u32, sn: u32| {
        let mut out = None;
        for (tx, rx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
            out = a.push(record(ts, tx, rx, 64, Some(sn))).unwrap();
        }
        out.unwrap()
    };
    let f1 = push_group(&mut a, 1, 100);
    assert_eq!(f1.sequence, 100);
    assert!(!f1.flags.contains(CsiFlags::DROPPED_PREDECESSOR));

    let f2 = push_group(&mut a, 2, 101);
    assert!(!f2.flags.contains(CsiFlags::DROPPED_PREDECESSOR));

    let f3 = push_group(&mut a, 3, 105); // four missing
    assert_eq!(f3.sequence, 105);
    assert!(f3.flags.contains(CsiFlags::DROPPED_PREDECESSOR));
    assert_eq!(a.stats().pkt_sn_gaps, 1);
}

#[test]
fn without_pkt_sn_no_gap_is_ever_claimed() {
    let mut a = assembler();
    let mut seqs = Vec::new();
    for ts in 0..3u32 {
        for (tx, rx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
            if let Some(f) = a.push(record(ts, tx, rx, 64, None)).unwrap() {
                assert!(!f.flags.contains(CsiFlags::DROPPED_PREDECESSOR));
                seqs.push(f.sequence);
            }
        }
    }
    assert_eq!(seqs, vec![0, 1, 2]);
    assert_eq!(a.stats().pkt_sn_gaps, 0);
}

#[test]
fn incomplete_group_is_dropped_not_zero_filled() {
    let mut a = assembler();
    assert!(a.push(record(1, 0, 0, 64, None)).unwrap().is_none());
    assert!(a.push(record(1, 0, 1, 64, None)).unwrap().is_none());
    assert!(a.push(record(1, 1, 0, 64, None)).unwrap().is_none());
    // New packet begins before the rectangle closed.
    assert!(a.push(record(2, 0, 0, 64, None)).unwrap().is_none());
    assert_eq!(a.stats().groups_dropped_incomplete, 1);
    assert_eq!(a.stats().frames_out, 0);
}

#[test]
fn ragged_subcarrier_counts_are_rejected() {
    let mut a = assembler();
    let _ = a.push(record(1, 0, 0, 64, None)).unwrap();
    let _ = a.push(record(1, 0, 1, 64, None)).unwrap();
    let _ = a.push(record(1, 1, 0, 64, None)).unwrap();
    let mut odd = record(1, 1, 1, 64, None);
    odd.data_i.truncate(32);
    odd.data_q.truncate(32);
    // The group completes, but the chains disagree on the grid.
    assert!(matches!(a.push(odd), Err(BridgeError::RaggedGroup)));
}

#[test]
fn a_chain_outside_the_configured_dimensions_is_rejected() {
    let mut a = assembler();
    assert!(matches!(
        a.push(record(1, 0, 2, 64, None)),
        Err(BridgeError::ChainOutOfRange { rx: 2, .. })
    ));
}

#[test]
fn a_single_record_never_emits_a_one_by_one_frame() {
    let mut a = assembler();
    assert!(a.push(record(1, 0, 0, 64, None)).unwrap().is_none());
    assert_eq!(a.stats().frames_out, 0);
}

#[test]
fn saturated_samples_set_the_saturated_flag() {
    let mut a = assembler();
    let mut frame = None;
    for (tx, rx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
        let mut r = record(1, tx, rx, 64, None);
        if (tx, rx) == (1, 1) {
            r.data_i[3] = i16::MAX;
        }
        frame = a.push(r).unwrap();
    }
    assert!(frame.unwrap().flags.contains(CsiFlags::SATURATED));
}

#[test]
fn per_record_mode_emits_one_1x1_frame_per_datagram() {
    let mut a = FrameAssembler::new(AssemblerConfig {
        device_id: 9,
        group_by: GroupBy::PerRecord,
        ..Default::default()
    });
    let mut r0 = record(1, 0, 0, 253, None);
    r0.bw_code = None;
    r0.rx_mode = None;
    r0.rssi = None;
    r0.snr = None;
    r0.trimmed = true;

    let frame = a.push(r0).unwrap().expect("one datagram is one frame");
    assert_eq!((frame.tx_count, frame.rx_count), (1, 1));
    assert_eq!(frame.subcarrier_count, 253);
    assert_eq!(frame.bandwidth_mhz, 80);
    assert_eq!(frame.payload.len(), 253);
    // Nothing the datagram lacked may be reported as a measurement.
    assert_eq!(frame.payload.rssi_dbm(), &[DBM_NOT_REPORTED]);
    assert_eq!(frame.noise_floor_dbm, DBM_NOT_REPORTED);
    assert_eq!(frame.ppdu_type, PpduType::Unknown);
}

/// Regression for the real CSIdump send order.
///
/// CSIdump's loop is antenna-major — every packet of antenna 0, then every packet
/// of antenna 1 — so a rule that pairs consecutive datagrams by chain index both
/// loses most frames and, worse, splices chains from unrelated PPDUs into the few
/// it emits. Measured against the previous chain-window rule, 10 records yielded
/// 1 frame and 7 discarded groups. `PerRecord` emits one honest frame per record.
#[test]
fn antenna_major_order_loses_nothing_and_splices_nothing() {
    let mut a = FrameAssembler::new(AssemblerConfig {
        group_by: GroupBy::PerRecord,
        ..Default::default()
    });
    let mut frames = Vec::new();
    for antenna in 0..2u16 {
        for pkt in 0..5u32 {
            let mut r = record(1_000 + pkt, 0, antenna, 61, None);
            r.bw_code = None;
            r.rx_mode = None;
            r.rssi = None;
            r.snr = None;
            r.trimmed = true;
            // Tag each record so a spliced frame would be detectable.
            r.data_i = vec![(antenna as i16) * 100 + pkt as i16; 61];
            frames.push(a.push(r).unwrap().expect("every record yields a frame"));
        }
    }

    assert_eq!(frames.len(), 10, "no record may be dropped");
    assert_eq!(a.stats().records_in, 10);
    assert_eq!(a.stats().frames_out, 10);
    assert_eq!(
        a.stats().groups_dropped_incomplete,
        0,
        "per-record emission never leaves a partial group"
    );

    // Every frame carries exactly one chain, all of it from one record, so no
    // frame can mix two PPDUs.
    for frame in &frames {
        assert_eq!((frame.tx_count, frame.rx_count), (1, 1));
        let values = match &frame.payload {
            CsiPayload::ComplexI16 { values, .. } => values,
            other => panic!("expected ComplexI16, got {other:?}"),
        };
        let first = values[0][0];
        assert!(
            values.iter().all(|v| v[0] == first),
            "frame {first} mixes samples from more than one record"
        );
    }
}

#[test]
fn two_device_ids_do_not_collide_in_emitted_frames() {
    let build = |device_id: u64| {
        let mut a = FrameAssembler::new(AssemblerConfig {
            device_id,
            ..Default::default()
        });
        let mut out = None;
        for (tx, rx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
            out = a.push(record(1, tx, rx, 64, None)).unwrap();
        }
        out.unwrap()
    };
    let a = build(device_id_from_node("wn586x3-a"));
    let b = build(device_id_from_node("wn586x3-b"));
    assert_ne!(a.device_id, b.device_id);
    assert_ne!(a.to_bytes().unwrap(), b.to_bytes().unwrap());
}

// ── GroupBy::LastChainMarker ────────────────────────────────────────────────
//
// Cases drawn from the two live WN586X3 dump loops. Unit 1's client was entirely
// HT and produced clean 2x2 runs; unit 2's client mixed HT with legacy
// single-stream (1x2) traffic and shared the radio with a second client, which is
// what the fixed 2x2 rectangle could not handle.

// Fabricated, locally administered transmitter addresses. The real clients'
// addresses are not reproduced here; only distinctness matters to these tests.
/// Stands in for unit 1's client.
const TA_UNIT1: [u8; 6] = [0x02, 0x00, 0x00, 0x00, 0x00, 0x01];
/// Stands in for unit 2's primary client.
const TA_UNIT2: [u8; 6] = [0x02, 0x00, 0x00, 0x00, 0x00, 0x02];
/// Stands in for the second client sharing unit 2's radio.
const TA_UNIT2_OTHER: [u8; 6] = [0x02, 0x00, 0x00, 0x00, 0x00, 0x03];

const LAST_CHAIN: u32 = 1 << 15;

fn chain_record(ta: [u8; 6], ts: u32, tx: u16, rx: u16, rx_mode: u8, last: bool) -> MtkRecord {
    let mut r = record(ts, tx, rx, 64, None);
    r.ta = ta;
    r.rx_mode = Some(rx_mode);
    r.bw_code = Some(0); // BW20, as both units reported
    r.chain_info = if last { LAST_CHAIN } else { 0 };
    r
}

fn last_chain_assembler() -> FrameAssembler {
    FrameAssembler::new(AssemblerConfig {
        group_by: GroupBy::LastChainMarker {
            fallback_tx: 2,
            fallback_rx: 2,
        },
        ..Default::default()
    })
}

/// Unit 1: every run is HT 2x2, terminated on the fourth chain.
#[test]
fn an_ht_run_closes_as_two_by_two_on_the_last_chain_marker() {
    let mut a = last_chain_assembler();
    let chains = [(0, 0), (0, 1), (1, 0), (1, 1)];
    let mut out = None;
    for (n, (tx, rx)) in chains.iter().enumerate() {
        let last = n == chains.len() - 1;
        let frame = a
            .push(chain_record(TA_UNIT1, 500, *tx, *rx, 2, last))
            .unwrap();
        if last {
            out = frame;
        } else {
            assert!(frame.is_none(), "run must not close before the marker");
        }
    }
    let frame = out.expect("the marker closes the run");
    assert_eq!((frame.tx_count, frame.rx_count), (2, 2));
    assert_eq!(frame.payload.len(), 4 * 64);
    assert_eq!(a.stats().groups_dropped_incomplete, 0);
    assert_eq!(a.stats().runs_dropped_ragged, 0);
}

/// Unit 2: a legacy single-stream PPDU is 1x2 and must close on its own terms.
/// Under the old fixed 2x2 rectangle this group could never complete.
#[test]
fn a_legacy_single_stream_run_closes_as_one_by_two() {
    let mut a = last_chain_assembler();
    assert!(a
        .push(chain_record(TA_UNIT2, 700, 0, 0, 1, false))
        .unwrap()
        .is_none());
    let frame = a
        .push(chain_record(TA_UNIT2, 700, 0, 1, 1, true))
        .unwrap()
        .expect("two chains and a marker is a complete 1x2 run");

    assert_eq!((frame.tx_count, frame.rx_count), (1, 2));
    assert_eq!(frame.payload.len(), 2 * 64);
    assert_eq!(frame.ppdu_type, PpduType::Legacy);
    assert_eq!(a.stats().groups_dropped_incomplete, 0);
    assert_eq!(a.stats().runs_dropped_ragged, 0);
}

/// The unit 2 case end to end: HT and legacy PPDUs from one client, back to back.
#[test]
fn mixed_ht_and_legacy_traffic_both_emit_with_their_own_dimensions() {
    let mut a = last_chain_assembler();
    let mut shapes = Vec::new();

    for round in 0..3u32 {
        // HT 2x2
        for (n, (tx, rx)) in [(0, 0), (0, 1), (1, 0), (1, 1)].iter().enumerate() {
            let f = a
                .push(chain_record(TA_UNIT2, 1_000 + round, *tx, *rx, 2, n == 3))
                .unwrap();
            if let Some(f) = f {
                shapes.push((f.tx_count, f.rx_count, f.ppdu_type));
            }
        }
        // Legacy 1x2
        for (n, (tx, rx)) in [(0, 0), (0, 1)].iter().enumerate() {
            let f = a
                .push(chain_record(TA_UNIT2, 2_000 + round, *tx, *rx, 1, n == 1))
                .unwrap();
            if let Some(f) = f {
                shapes.push((f.tx_count, f.rx_count, f.ppdu_type));
            }
        }
    }

    assert_eq!(
        shapes,
        vec![
            (2, 2, PpduType::Ht),
            (1, 2, PpduType::Legacy),
            (2, 2, PpduType::Ht),
            (1, 2, PpduType::Legacy),
            (2, 2, PpduType::Ht),
            (1, 2, PpduType::Legacy),
        ]
    );
    assert_eq!(a.stats().frames_out, 6);
    assert_eq!(a.stats().groups_dropped_incomplete, 0);
    assert_eq!(a.stats().runs_dropped_ragged, 0);
}

/// Two clients on one radio interleave their chains. Per-address buckets must
/// keep them apart, or a frame would mix two transmitters.
#[test]
fn two_clients_interleaving_do_not_contaminate_each_other() {
    let mut a = last_chain_assembler();
    // Interleave chain by chain between the two transmitters unit 2 really saw.
    let a_chains = [(0u16, 0u16), (0, 1), (1, 0), (1, 1)];
    let b_chains = [(0u16, 0u16), (0, 1)];
    let mut frames = Vec::new();

    for n in 0..4 {
        if let Some(f) = a
            .push(chain_record(
                TA_UNIT2,
                10,
                a_chains[n].0,
                a_chains[n].1,
                2,
                n == 3,
            ))
            .unwrap()
        {
            frames.push(f);
        }
        if n < b_chains.len() {
            if let Some(f) = a
                .push(chain_record(
                    TA_UNIT2_OTHER,
                    11,
                    b_chains[n].0,
                    b_chains[n].1,
                    1,
                    n == b_chains.len() - 1,
                ))
                .unwrap()
            {
                frames.push(f);
            }
        }
    }

    assert_eq!(frames.len(), 2, "one frame per client");
    let legacy = frames
        .iter()
        .find(|f| f.ppdu_type == PpduType::Legacy)
        .expect("the second client's legacy run");
    let ht = frames
        .iter()
        .find(|f| f.ppdu_type == PpduType::Ht)
        .expect("the first client's HT run");
    assert_eq!((ht.tx_count, ht.rx_count), (2, 2));
    assert_eq!((legacy.tx_count, legacy.rx_count), (1, 2));
    assert_eq!(a.stats().runs_dropped_ragged, 0);
    assert_eq!(a.stats().groups_dropped_incomplete, 0);
}

/// A missing terminator must abandon the run, never splice it onto the next PPDU.
#[test]
fn a_repeated_chain_abandons_the_run_instead_of_splicing() {
    let mut a = last_chain_assembler();
    // Two chains, no marker, then the same chain again: the first run lost its
    // terminator.
    assert!(a
        .push(chain_record(TA_UNIT2, 1, 0, 0, 2, false))
        .unwrap()
        .is_none());
    assert!(a
        .push(chain_record(TA_UNIT2, 1, 0, 1, 2, false))
        .unwrap()
        .is_none());
    assert!(a
        .push(chain_record(TA_UNIT2, 2, 0, 0, 2, false))
        .unwrap()
        .is_none());
    assert_eq!(a.stats().runs_dropped_ragged, 1);
    assert_eq!(a.stats().frames_out, 0);

    // The fresh run still completes normally.
    let frame = a
        .push(chain_record(TA_UNIT2, 2, 0, 1, 2, true))
        .unwrap()
        .expect("the new run closes on its marker");
    assert_eq!((frame.tx_count, frame.rx_count), (1, 2));
}

/// A run closed on a marker but missing a chain is not a rectangle, so it is
/// dropped rather than emitted with a hole.
#[test]
fn a_marked_run_that_is_not_a_rectangle_is_dropped() {
    let mut a = last_chain_assembler();
    for (n, (tx, rx)) in [(0u16, 0u16), (0, 1), (1, 1)].iter().enumerate() {
        let f = a
            .push(chain_record(TA_UNIT1, 9, *tx, *rx, 2, n == 2))
            .unwrap();
        assert!(f.is_none(), "a 3-of-4 run must not emit");
    }
    assert_eq!(a.stats().groups_dropped_incomplete, 1);
    assert_eq!(a.stats().frames_out, 0);
}

/// An input whose chain_info never sets the marker still works, via the
/// configured fallback rectangle.
#[test]
fn an_input_with_no_marker_falls_back_to_the_configured_rectangle() {
    let mut a = last_chain_assembler();
    let mut out = None;
    for (tx, rx) in [(0u16, 0u16), (0, 1), (1, 0), (1, 1)] {
        out = a.push(chain_record(TA_UNIT1, 4, tx, rx, 2, false)).unwrap();
    }
    let frame = out.expect("the fallback rectangle closes the run");
    assert_eq!((frame.tx_count, frame.rx_count), (2, 2));
    assert_eq!(a.stats().runs_dropped_ragged, 0);
}

/// Once a real marker has been seen the fallback must not cut runs short, or a
/// legacy 1x2 run would be merged with the start of the next PPDU.
#[test]
fn a_seen_marker_disables_the_fallback_rectangle() {
    let mut a = last_chain_assembler();
    // One marked legacy run teaches the assembler that markers exist.
    let _ = a.push(chain_record(TA_UNIT2, 1, 0, 0, 1, false)).unwrap();
    let first = a.push(chain_record(TA_UNIT2, 1, 0, 1, 1, true)).unwrap();
    assert!(first.is_some());

    // Now four unmarked chains must NOT close as a rectangle at 4 records.
    for (tx, rx) in [(0u16, 0u16), (0, 1), (1, 0), (1, 1)] {
        assert!(
            a.push(chain_record(TA_UNIT2, 2, tx, rx, 2, false))
                .unwrap()
                .is_none(),
            "the fallback must be off once markers are known to work"
        );
    }
    assert_eq!(a.stats().frames_out, 1);
}
