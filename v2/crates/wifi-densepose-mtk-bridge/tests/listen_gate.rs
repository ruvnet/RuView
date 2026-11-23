//! The live UDP path's source allowlist, exercised through the real binary.
//!
//! `--listen` is the only route to a `mediatek:physical-unvalidated` label, so
//! it must not accept datagrams from sources the operator did not name.

use std::io::{BufRead, BufReader, Read};
use std::net::UdpSocket;
use std::path::PathBuf;
use std::process::{Child, ChildStderr, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

fn bridge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_wifi-densepose-mtk-bridge"))
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

/// Start the bridge on an OS-assigned loopback port and wait for it to report
/// the bound address. Picking a free port first and passing it in races any
/// other socket that takes the port before the bridge binds it.
fn spawn_listening(args: &[&str]) -> (Child, BufReader<ChildStderr>, String, String) {
    let mut child = Command::new(bridge_bin())
        .args(["--listen", "127.0.0.1:0"])
        .args(args)
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn wifi-densepose-mtk-bridge");
    let mut stderr = BufReader::new(child.stderr.take().unwrap());
    let (send, receive) = mpsc::sync_channel(1);
    let reader = std::thread::spawn(move || {
        let mut seen = String::new();
        loop {
            let mut line = String::new();
            match stderr.read_line(&mut line) {
                Ok(0) => {
                    let _ = send.send(Err(format!("bridge exited before listening: {seen}")));
                    return;
                }
                Err(error) => {
                    let _ = send.send(Err(format!("read bridge stderr: {error}")));
                    return;
                }
                Ok(_) => {}
            }
            seen.push_str(&line);
            if let Some(rest) = line.strip_prefix("listening for MtkCSIdump datagrams on ") {
                let addr = rest.split(',').next().unwrap().to_string();
                let _ = send.send(Ok((stderr, addr, seen)));
                return;
            }
        }
    });
    match receive.recv_timeout(Duration::from_secs(10)) {
        Ok(Ok((stderr, addr, seen))) => {
            reader.join().expect("stderr reader panicked");
            (child, stderr, addr, seen)
        }
        error => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            panic!("bridge did not become ready within ten seconds: {error:?}");
        }
    }
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

    // Allow only a network the loopback sender is not in.
    let (mut child, mut child_stderr, listen_addr, mut stderr) = spawn_listening(&[
        "--listen-allow",
        "10.99.0.0/16",
        "--sink",
        &sink_addr,
        "--node",
        "listen-gate",
    ]);

    let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
    for antenna in 0..4u32 {
        sender
            .send_to(&datagram(antenna % 2), &listen_addr)
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
    child_stderr.read_to_string(&mut stderr).unwrap();
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

    // Same setup, but loopback is named this time.
    let (mut child, _stderr, listen_addr, _) = spawn_listening(&[
        "--listen-allow",
        "127.0.0.1",
        "--sink",
        &sink_addr,
        "--node",
        "listen-gate",
        "--max-frames",
        "1",
    ]);

    let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
    for antenna in 0..2u32 {
        sender.send_to(&datagram(antenna), &listen_addr).unwrap();
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
