//! The live UDP path's source allowlist, exercised through the real binary.
//!
//! `--listen` is the only route to a `mediatek:physical-unvalidated` label, so
//! it must not accept datagrams from sources the operator did not name.

use std::io::Read;
use std::net::UdpSocket;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

fn bridge_bin() -> PathBuf {
    let mut path = std::env::current_exe().expect("test exe path");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.join("wifi-densepose-mtk-bridge")
}

/// One MtkCSIdump datagram: 20-byte packed header then `{f64 i, f64 q}` pairs.
fn datagram(antenna: u32) -> Vec<u8> {
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

fn free_port() -> u16 {
    UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[test]
fn listen_without_an_allowlist_refuses_to_start() {
    let out = Command::new(bridge_bin())
        .args([
            "--listen",
            "127.0.0.1:0",
            "--sink",
            "127.0.0.1:1",
            "--node",
            "listen-gate",
        ])
        .output()
        .expect("run wifi-densepose-mtk-bridge");

    assert!(!out.status.success(), "bridge must refuse to listen");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("--listen-allow"), "{stderr}");
    assert!(
        stderr.contains("physical-unvalidated"),
        "the refusal should say why: {stderr}"
    );
}

#[test]
fn a_datagram_from_outside_the_allowlist_is_dropped_and_never_assembled() {
    // Stand in for the sensing server; nothing may arrive here.
    let sink = UdpSocket::bind("127.0.0.1:0").unwrap();
    sink.set_read_timeout(Some(Duration::from_millis(700)))
        .unwrap();
    let sink_addr = sink.local_addr().unwrap().to_string();
    let listen_port = free_port();

    // Allow only a network the loopback sender is not in.
    let mut child = Command::new(bridge_bin())
        .args([
            "--listen",
            &format!("127.0.0.1:{listen_port}"),
            "--listen-allow",
            "10.99.0.0/16",
            "--sink",
            &sink_addr,
            "--node",
            "listen-gate",
        ])
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn wifi-densepose-mtk-bridge");

    std::thread::sleep(Duration::from_millis(500));
    let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
    for antenna in 0..4u32 {
        sender
            .send_to(&datagram(antenna % 2), format!("127.0.0.1:{listen_port}"))
            .unwrap();
    }
    std::thread::sleep(Duration::from_millis(500));

    // Two full chain windows would have been assembled had they been admitted.
    let mut buf = vec![0u8; 65_535];
    assert!(
        sink.recv_from(&mut buf).is_err(),
        "a rejected source must never reach the assembler"
    );

    child.kill().expect("kill bridge");
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    let _ = child.wait();

    assert!(stderr.contains("rejected_source"), "{stderr}");
    assert!(stderr.contains("127.0.0.1"), "{stderr}");
    assert!(
        !stderr.contains("PHYSICAL-UNVALIDATED:"),
        "the first-frame banner must not fire for a rejected source: {stderr}"
    );
}

#[test]
fn a_datagram_from_inside_the_allowlist_is_admitted() {
    let sink = UdpSocket::bind("127.0.0.1:0").unwrap();
    sink.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let sink_addr = sink.local_addr().unwrap().to_string();
    let listen_port = free_port();

    // Same setup, but loopback is named this time.
    let mut child = Command::new(bridge_bin())
        .args([
            "--listen",
            &format!("127.0.0.1:{listen_port}"),
            "--listen-allow",
            "127.0.0.1",
            "--sink",
            &sink_addr,
            "--node",
            "listen-gate",
            "--max-frames",
            "1",
        ])
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn wifi-densepose-mtk-bridge");

    std::thread::sleep(Duration::from_millis(500));
    let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
    for antenna in 0..2u32 {
        sender
            .send_to(&datagram(antenna), format!("127.0.0.1:{listen_port}"))
            .unwrap();
    }

    let mut buf = vec![0u8; 65_535];
    let (n, _) = sink.recv_from(&mut buf).expect("frame should arrive");
    let (frame, _) =
        wifi_densepose_hardware::mediatek_csi::CsiFrame::from_bytes(&buf[..n]).unwrap();
    // One datagram is one chain of one packet, so one 1x1 frame.
    assert_eq!((frame.tx_count, frame.rx_count), (1, 1));
    assert_eq!(frame.subcarrier_count, 253);

    let _ = child.kill();
    let _ = child.wait();
}
