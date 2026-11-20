//! End-to-end: run the bridge binary on a fixture, receive the MTC1 frames on a
//! UDP socket standing in for the sensing server's `:5005` ingest, and assert
//! the provenance label the server would compute.
//!
//! # Fixture provenance
//!
//! `fixtures/FABRICATED-*.synthetic.json` is **not a hardware capture**. It is a
//! hand-generated file in MediaTek's `mt76-vendor` dump format, used to exercise
//! the decoder and the wire path with no radio present. Its `.synthetic.` infix
//! and its `.provenance.json` sidecar both declare that, and the bridge refuses
//! to replay it as anything else. Per the repository rule that hardware
//! validation requires evidence from real silicon, nothing here is evidence that
//! MT7981 CSI works — only that the bridge transforms records of that shape
//! correctly. No real silicon has been through this path.

use std::net::UdpSocket;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::Duration;

use wifi_densepose_hardware::mediatek_csi::{CsiFlags, CsiFrame};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("FABRICATED-mt76-vendor-dump-mt7981-bw80.synthetic.json")
}

fn bridge_bin() -> PathBuf {
    let mut path = std::env::current_exe().expect("test exe path");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.join("wifi-densepose-mtk-bridge")
}

/// Mirrors `MediatekCsiSnapshot::from_frame` in the sensing server: provenance
/// comes from the frame's own flags, SYNTHETIC first.
fn server_source_label(frame: &CsiFrame) -> &'static str {
    if frame.flags.contains(CsiFlags::SYNTHETIC) {
        "mediatek:simulated"
    } else if frame.flags.contains(CsiFlags::CALIBRATED) {
        "mediatek"
    } else {
        "mediatek:physical-unvalidated"
    }
}

fn spawn(node: &str, sink: &str, max_frames: u64, extra: &[&str]) -> Output {
    let mut cmd = Command::new(bridge_bin());
    cmd.args([
        "--replay",
        fixture().to_str().unwrap(),
        "--sink",
        sink,
        "--node",
        node,
        "--replay-hz",
        "0",
        "--max-frames",
        &max_frames.to_string(),
    ]);
    cmd.args(extra);
    cmd.output().expect("run wifi-densepose-mtk-bridge")
}

fn run_into_socket(node: &str, max_frames: u64, extra: &[&str]) -> Vec<CsiFrame> {
    let server = UdpSocket::bind("127.0.0.1:0").expect("bind fake sensing server");
    server
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let addr = server.local_addr().unwrap().to_string();

    let out = spawn(node, &addr, max_frames, extra);
    assert!(
        out.status.success(),
        "bridge failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let mut frames = Vec::new();
    let mut buf = vec![0u8; 65_535];
    for _ in 0..max_frames {
        let (n, _) = match server.recv_from(&mut buf) {
            Ok(v) => v,
            Err(_) => break,
        };
        let (frame, consumed) = CsiFrame::from_bytes(&buf[..n]).expect("decode MTC1");
        assert_eq!(consumed, n, "frame must fill the datagram");
        frames.push(frame);
    }
    frames
}

/// The central guarantee: a fabricated file cannot reach the wire unlabelled.
#[test]
fn replaying_the_fabricated_fixture_without_synthetic_fails_fast() {
    let out = spawn("wn586x3-livingroom", "127.0.0.1:1", 1, &[]);
    assert!(!out.status.success(), "bridge must refuse to start");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("declares itself synthetic"),
        "unexpected stderr: {stderr}"
    );
    assert!(stderr.contains("--synthetic"), "{stderr}");
}

/// An attestation must not launder a declared-synthetic input into physical.
#[test]
fn an_attestation_cannot_launder_the_fabricated_fixture() {
    let out = spawn(
        "wn586x3-livingroom",
        "127.0.0.1:1",
        1,
        &["--captured-on", "WN586X3/OpenWrt-24.10.8"],
    );
    assert!(!out.status.success(), "bridge must refuse to start");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("cannot override"), "{stderr}");
}

#[test]
fn replay_with_synthetic_reaches_the_sensing_server_as_simulated() {
    let frames = run_into_socket("wn586x3-livingroom", 8, &["--synthetic"]);
    assert_eq!(frames.len(), 8, "fixture holds 8 packets of 2x2 chains");

    for (n, frame) in frames.iter().enumerate() {
        assert_eq!(
            server_source_label(frame),
            "mediatek:simulated",
            "frame {n}"
        );
        assert!(frame.flags.contains(CsiFlags::SYNTHETIC));
        assert!(!frame.flags.contains(CsiFlags::CALIBRATED));
        assert!(!frame.flags.contains(CsiFlags::TIME_SYNCHRONIZED));
        assert_eq!((frame.tx_count, frame.rx_count), (2, 2));
        assert_eq!(frame.subcarrier_count, 256);
        assert_eq!(frame.payload.len(), 2 * 2 * 256);
        assert_eq!(frame.bandwidth_mhz, 80);
        assert_eq!(frame.subcarrier_spacing_hz, 312_500.0);
        assert_eq!(frame.payload.rssi_dbm(), &[-54, -55]);
        assert_eq!(frame.calibration_id, 0);
    }

    // Timestamps advance monotonically at the fixture's 20 ms packet period.
    let ts: Vec<u64> = frames.iter().map(|f| f.timestamp_us).collect();
    assert!(ts.windows(2).all(|w| w[1] > w[0]), "{ts:?}");
    assert_eq!(ts[1] - ts[0], 20_000);

    // No pkt_sn in a stock dump, so sequence is the bridge's local counter and
    // no frame may claim a dropped predecessor.
    let seqs: Vec<u32> = frames.iter().map(|f| f.sequence).collect();
    assert_eq!(seqs, (0..8).collect::<Vec<u32>>());
    assert!(frames
        .iter()
        .all(|f| !f.flags.contains(CsiFlags::DROPPED_PREDECESSOR)));
}

#[test]
fn two_nodes_replaying_the_same_capture_do_not_collide() {
    let a = run_into_socket("wn586x3-livingroom", 2, &["--synthetic"]);
    let b = run_into_socket("wn586x3-kitchen", 2, &["--synthetic"]);
    assert_ne!(a[0].device_id, b[0].device_id);
    // Everything except identity is identical, so a collision would be silent.
    assert_eq!(a[0].payload, b[0].payload);
    assert_eq!(a[0].timestamp_us, b[0].timestamp_us);
    assert_ne!(a[0].to_bytes().unwrap(), b[0].to_bytes().unwrap());
}
