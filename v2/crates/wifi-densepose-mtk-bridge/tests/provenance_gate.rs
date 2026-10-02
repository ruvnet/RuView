//! The provenance gate: which inputs may produce which label.
//!
//! `mediatek:physical-unvalidated` must be reachable only from a real device, so
//! a replay has to be either declared synthetic or attested to hardware. These
//! tests pin both directions, including that a live-shaped datagram still
//! produces a physical frame.

use std::net::UdpSocket;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

use wifi_densepose_hardware::mediatek_csi::{CsiFlags, CsiFrame};

use wifi_densepose_mtk_bridge::assemble::{AssemblerConfig, FrameAssembler, GroupBy};
use wifi_densepose_mtk_bridge::capture::{CaptureHeader, CaptureWriter};
use wifi_densepose_mtk_bridge::udp_in::{decode_datagram, DatagramHeader};

fn bridge_bin() -> PathBuf {
    let mut path = std::env::current_exe().expect("test exe path");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.join("wifi-densepose-mtk-bridge")
}

fn tmp(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("provenance");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

/// The fabricated fixture's records, written out under a name that declares
/// nothing — no `.synthetic.` infix and no sidecar. This stands in for a file an
/// operator claims came off hardware.
fn undeclared_dump(name: &str) -> PathBuf {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("FABRICATED-mt76-vendor-dump-mt7981-bw80.synthetic.json");
    let dst = tmp(name);
    std::fs::copy(src, &dst).unwrap();
    // CARGO_TARGET_TMPDIR persists between runs, so clear any sidecar a previous
    // run left behind before asserting this input declares nothing.
    let _ = std::fs::remove_file(super_sidecar(&dst));
    assert!(!super_sidecar(&dst).exists());
    dst
}

fn super_sidecar(input: &Path) -> PathBuf {
    let mut name = input.file_name().unwrap().to_os_string();
    name.push(".provenance.json");
    input.with_file_name(name)
}

fn spawn(replay: &Path, sink: &str, extra: &[&str]) -> Output {
    let mut cmd = Command::new(bridge_bin());
    cmd.args([
        "--replay",
        replay.to_str().unwrap(),
        "--sink",
        sink,
        "--node",
        "gate-test",
        "--replay-hz",
        "0",
        "--max-frames",
        "2",
    ]);
    cmd.args(extra);
    cmd.output().expect("run wifi-densepose-mtk-bridge")
}

fn frames_from(replay: &Path, extra: &[&str]) -> (Vec<CsiFrame>, String) {
    let server = UdpSocket::bind("127.0.0.1:0").unwrap();
    server
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let addr = server.local_addr().unwrap().to_string();
    let out = spawn(replay, &addr, extra);
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(out.status.success(), "bridge failed: {stderr}");

    let mut frames = Vec::new();
    let mut buf = vec![0u8; 65_535];
    for _ in 0..2 {
        let Ok((n, _)) = server.recv_from(&mut buf) else {
            break;
        };
        frames.push(CsiFrame::from_bytes(&buf[..n]).unwrap().0);
    }
    (frames, stderr)
}

#[test]
fn an_undeclared_file_will_not_replay_without_a_provenance_decision() {
    let path = undeclared_dump("undeclared-none.json");
    let out = spawn(&path, "127.0.0.1:1", &[]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("needs a provenance decision"), "{stderr}");
}

#[test]
fn an_attested_file_stays_physical_unvalidated() {
    let path = undeclared_dump("undeclared-attested.json");
    let (frames, stderr) = frames_from(&path, &["--captured-on", "WN586X3/OpenWrt-24.10.8"]);
    assert_eq!(frames.len(), 2);
    for frame in &frames {
        assert!(!frame.flags.contains(CsiFlags::SYNTHETIC));
        assert!(!frame.flags.contains(CsiFlags::CALIBRATED));
    }
    assert!(stderr.contains("PHYSICAL-UNVALIDATED"), "{stderr}");
    assert!(stderr.contains("WN586X3/OpenWrt-24.10.8"), "{stderr}");
}

#[test]
fn a_malformed_attestation_is_refused() {
    let path = undeclared_dump("undeclared-badattest.json");
    let out = spawn(&path, "127.0.0.1:1", &["--captured-on", "WN586X3"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("<model>/<firmware>"), "{stderr}");
}

#[test]
fn a_sidecar_alone_forces_synthetic_even_without_the_filename_infix() {
    let path = undeclared_dump("undeclared-with-sidecar.json");
    std::fs::write(
        super_sidecar(&path),
        r#"{"synthetic": true, "generator": "unit test"}"#,
    )
    .unwrap();
    let out = spawn(&path, "127.0.0.1:1", &["--captured-on", "WN586X3/24.10.8"]);
    assert!(!out.status.success(), "sidecar must beat the attestation");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("declares itself synthetic"), "{stderr}");
    assert!(stderr.contains("unit test"), "generator should be shown");
}

#[test]
fn a_capture_recorded_under_synthetic_cannot_be_replayed_as_physical() {
    // Record the marker into the header the way `--record --synthetic` does.
    let path = tmp("recorded-synthetic.mtkcap");
    {
        let file = std::fs::File::create(&path).unwrap();
        let mut writer = CaptureWriter::new(file, CaptureHeader::new("udp", "n", true));
        let (_, record) = decode_datagram(&hand_built_datagram(0)).unwrap();
        writer.write_record(&record).unwrap();
        let (_, record) = decode_datagram(&hand_built_datagram(1)).unwrap();
        writer.write_record(&record).unwrap();
        writer.flush().unwrap();
    }
    let out = spawn(&path, "127.0.0.1:1", &["--captured-on", "WN586X3/24.10.8"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("declares itself synthetic"), "{stderr}");
    assert!(stderr.contains("capture header"), "{stderr}");
}

/// A datagram in the MtkCSIdump layout, built byte by byte
/// (`motion_detector.h:18-28`). BW80 trimmed to 253 bins.
fn hand_built_datagram(antenna: u32) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&1_726_000_000_123u64.to_le_bytes());
    buf.extend_from_slice(&antenna.to_le_bytes());
    buf.extend_from_slice(&1u32.to_le_bytes());
    buf.extend_from_slice(&253u32.to_le_bytes());
    for n in 0..253i32 {
        buf.extend_from_slice(&(n as f64).to_le_bytes());
        buf.extend_from_slice(&(-n as f64).to_le_bytes());
    }
    buf
}

/// The live radio shape still yields a physical frame: nothing in the gate makes
/// the UDP path synthetic by default.
#[test]
fn a_hand_built_datagram_still_yields_a_physical_unvalidated_frame() {
    let mut assembler = FrameAssembler::new(AssemblerConfig {
        device_id: 0xabcd,
        group_by: GroupBy::PerRecord,
        ..Default::default()
    });
    let (header, r0) = decode_datagram(&hand_built_datagram(0)).unwrap();
    assert_eq!(
        header,
        DatagramHeader {
            timestamp_ms: 1_726_000_000_123,
            antenna_idx: 0,
            packet_count: 1,
            total_samples: 253,
        }
    );
    // One datagram is one chain of one packet: one 1x1 frame, immediately.
    let frame = assembler
        .push(r0)
        .unwrap()
        .expect("a datagram yields a frame");

    assert!(!frame.flags.contains(CsiFlags::SYNTHETIC));
    assert!(!frame.flags.contains(CsiFlags::CALIBRATED));
    assert_eq!((frame.tx_count, frame.rx_count), (1, 1));
    assert_eq!(frame.subcarrier_count, 253);
    assert_eq!(frame.bandwidth_mhz, 80);
}
