//! Legacy-OFDM CSI, the shape of the first WN586X3 capture.
//!
//! On 2026-09-19 a WL-WN586X3 (MT7981B + MT7976C) reported `rx_mode = 1`
//! (`MT_PHY_TYPE_OFDM`) for a 2.4 GHz, 20 MHz client: one chain, 64
//! subcarriers, the driver's last-chain bit set. The bridge had no MTC1
//! value for that mode and dropped the frame. MediaTek's CSI driver treats
//! legacy OFDM as its own tone-mask group, so the frame has to be carried.
//!
//! The record itself is not in this tree. It contains a client transmitter
//! address and raw channel samples. This test builds a fabricated record
//! with the same structure and a locally administered address.

use std::process::Command;

use wifi_densepose_hardware::mediatek_csi::{CsiFlags, CsiPayload, PpduType};

use wifi_densepose_mtk_bridge::assemble::{AssemblerConfig, FrameAssembler, GroupBy};
use wifi_densepose_mtk_bridge::dump_file::parse_dump;
use wifi_densepose_mtk_bridge::record::{device_id_from_node, Bandwidth};

/// Driver last-chain marker, `chain_info & BIT(15)` (the CSI patch, "last chain").
const LAST_CHAIN: u32 = 1 << 15;

fn legacy_dump() -> String {
    let i: Vec<String> = (0..64i32).map(|k| (k - 32).to_string()).collect();
    let q: Vec<String> = (0..64i32).map(|k| (8 - (k % 17)).to_string()).collect();
    // 02:00:00:00:00:01 is locally administered. Not a device that was on the air.
    format!(
        r#"[[1000000,"020000000001",-40,30,0,0,1,0,0,{LAST_CHAIN},0,[{i}],[{q}]]]"#,
        i = i.join(","),
        q = q.join(","),
    )
}

fn bridge_bin() -> std::path::PathBuf {
    let mut path = std::env::current_exe().expect("test exe path");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.join("wifi-densepose-mtk-bridge")
}

#[test]
fn a_legacy_ofdm_record_decodes_as_bw20_not_as_ht() {
    let records = parse_dump(&legacy_dump()).unwrap();
    assert_eq!(records.len(), 1);
    let r = &records[0];

    assert_eq!(r.ta, [0x02, 0x00, 0x00, 0x00, 0x00, 0x01]);
    assert_eq!(r.rssi, Some(-40));
    assert_eq!(r.snr, Some(30));
    assert_eq!(r.bandwidth().unwrap(), Bandwidth::Bw20);
    assert_eq!(r.rx_mode, Some(1));
    assert_eq!(r.ppdu_type(), PpduType::Legacy);
    assert_eq!(r.tx_idx, 0);
    assert_eq!(r.rx_idx, 0);
    assert_eq!(r.chain_info, LAST_CHAIN);
    assert_eq!(r.subcarrier_count(), 64);
    assert!(!r.trimmed);
    assert_eq!(r.pkt_sn, None);
    assert!(!r.saturated());
    assert!(r.data_i.iter().any(|v| *v != r.data_i[0]));
}

#[test]
fn the_record_assembles_to_one_by_one_by_sixtyfour_and_round_trips() {
    let records = parse_dump(&legacy_dump()).unwrap();
    let mut assembler = FrameAssembler::new(AssemblerConfig {
        device_id: device_id_from_node("wn586x3-1"),
        center_freq_khz: 2_437_000,
        group_by: GroupBy::LastChainMarker {
            fallback_tx: 2,
            fallback_rx: 2,
        },
        ..Default::default()
    });
    let frame = assembler
        .push(records[0].clone())
        .unwrap()
        .expect("bit 15 closes a one-chain run");

    assert_eq!((frame.tx_count, frame.rx_count), (1, 1));
    assert_eq!(frame.subcarrier_count, 64);
    assert_eq!(frame.payload.len(), 64);
    assert_eq!(frame.bandwidth_mhz, 20);
    assert_eq!(frame.subcarrier_spacing_hz, 312_500.0);
    assert_eq!(frame.ppdu_type, PpduType::Legacy);
    assert_eq!(frame.payload.rssi_dbm(), &[-40]);
    assert_eq!(frame.noise_floor_dbm, -70);
    assert!(!frame.flags.contains(CsiFlags::SYNTHETIC));
    assert!(!frame.flags.contains(CsiFlags::CALIBRATED));

    match &frame.payload {
        CsiPayload::ComplexI16 { values, .. } => assert_eq!(values[0], [-32, 8]),
        other => panic!("expected ComplexI16, got {other:?}"),
    }

    let bytes = frame.to_bytes().unwrap();
    let (decoded, n) = wifi_densepose_hardware::mediatek_csi::CsiFrame::from_bytes(&bytes).unwrap();
    assert_eq!(n, bytes.len());
    assert_eq!(decoded, frame);
}

#[test]
fn the_binary_refuses_to_call_a_synthetic_legacy_record_physical() {
    let dir = std::env::temp_dir().join(format!(
        "ruview-mtk-legacy-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("legacy-ofdm-bw20.synthetic.json");
    std::fs::write(&path, legacy_dump()).unwrap();

    let out = Command::new(bridge_bin())
        .args([
            "--replay",
            path.to_str().unwrap(),
            "--synthetic",
            "--center-freq-khz",
            "2437000",
            "--node",
            "wn586x3-1",
            "--replay-hz",
            "0",
            "--dry-run",
        ])
        .output()
        .expect("run wifi-densepose-mtk-bridge");
    let _ = std::fs::remove_dir_all(&dir);

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "bridge refused the record: {stderr}");
    assert!(stderr.contains("PROVENANCE: SYNTHETIC"), "{stderr}");
    assert!(!stderr.contains("PHYSICAL-UNVALIDATED"), "{stderr}");
    assert!(
        stderr.contains("rx_mode=Some(1) -> ppdu=Legacy"),
        "{stderr}"
    );
    assert!(stderr.contains("chain_info=0x8000"), "{stderr}");
    assert!(stderr.contains("bit15/last_chain=true"), "{stderr}");
    assert!(stderr.contains("frames_out=1"), "{stderr}");
}

/// The lab record's `chain_info` was `0x10020`. Bit 15 is clear in that word.
/// The replay path groups on bit 15 and only falls back to `--tx-chains` by
/// `--rx-chains` when it has never seen the marker. A 2x2 fallback therefore
/// holds this record instead of emitting it.
#[test]
fn a_chain_word_without_bit15_does_not_close_a_2x2_fallback() {
    let text = legacy_dump().replace(&LAST_CHAIN.to_string(), "65568");
    let records = parse_dump(&text).unwrap();
    assert_eq!(records[0].chain_info, 0x1_0020);
    assert_eq!(records[0].chain_info & (1 << 15), 0);

    let mut assembler = FrameAssembler::new(AssemblerConfig {
        group_by: GroupBy::LastChainMarker {
            fallback_tx: 2,
            fallback_rx: 2,
        },
        ..Default::default()
    });
    let frame = assembler.push(records[0].clone()).unwrap();
    assert!(frame.is_none());
    assert_eq!(assembler.stats().frames_out, 0);
}
