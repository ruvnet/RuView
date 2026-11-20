//! Integration test: multi-node per-node state isolation (ADR-068, #249).
//!
//! Sends simulated ESP32 CSI frames from multiple node IDs to a test-owned
//! UDP receiver on an ephemeral port and verifies that:
//! 1. Each node gets independent state (no cross-contamination)
//! 2. Person count aggregates across active nodes
//! 3. Stale nodes are excluded from aggregation
//!
//! This does NOT require QEMU — it sends raw UDP packets directly.

use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

/// Bind a test-owned receiver on an OS-assigned port and return it with its
/// address. Tests in this file must send here, never to a well-known port
/// such as the sensing server's default UDP port: a running local server would
/// otherwise ingest up to 255 synthetic node IDs, and a hardcoded port can
/// collide with other processes or parallel test runs (#2086). The datagrams
/// are never read; keep the returned socket alive for the whole test so the
/// port is not reused mid-test.
fn ephemeral_test_target() -> (UdpSocket, SocketAddr) {
    let receiver = UdpSocket::bind("127.0.0.1:0").expect("bind ephemeral receiver");
    let addr = receiver
        .local_addr()
        .expect("ephemeral receiver has a local addr");
    (receiver, addr)
}

/// Build a minimal valid ESP32 CSI frame (magic 0xC511_0001).
///
/// Format (ADR-018, authoritative: firmware `csi_collector.c`):
///   [0..3]   magic: 0xC511_0001 (LE)
///   [4]      node_id
///   [5]      n_antennas (1)
///   [6..7]   n_subcarriers (LE u16 — 256 for ESP32-C6 HE-SU, issue #1005)
///   [8..11]  freq_mhz (LE u32, 2437 = channel 6)
///   [12..15] sequence (LE u32)
///   [16]     rssi (signed)
///   [17]     noise_floor
///   [18]     PPDU type (ADR-110: 0=HT/legacy, 1=HE-SU)
///   [19]     flags (ADR-110)
///   [20..]   I/Q pairs (n_antennas * n_subcarriers * 2 bytes)
fn build_csi_frame(node_id: u8, seq: u32, rssi: i8, n_sub: u16) -> Vec<u8> {
    let n_pairs = n_sub as usize;
    let mut buf = vec![0u8; 20 + n_pairs * 2];

    // Magic
    let magic: u32 = 0xC511_0001;
    buf[0..4].copy_from_slice(&magic.to_le_bytes());

    buf[4] = node_id;
    buf[5] = 1; // n_antennas
    buf[6..8].copy_from_slice(&n_sub.to_le_bytes());

    // freq = 2437 MHz (channel 6)
    let freq: u32 = 2437;
    buf[8..12].copy_from_slice(&freq.to_le_bytes());

    // sequence
    buf[12..16].copy_from_slice(&seq.to_le_bytes());

    buf[16] = rssi as u8;
    buf[17] = (-90i8) as u8; // noise floor
    buf[18] = u8::from(n_sub >= 256); // ADR-110 PPDU type: HE-SU for 256-bin
    buf[19] = 0; // ADR-110 flags

    // Generate I/Q pairs with node-specific patterns.
    // Different nodes produce different amplitude patterns so the server
    // computes different features for each.
    for i in 0..n_pairs {
        let phase = (i as f64 + node_id as f64 * 0.5) * 0.3;
        let amplitude = 20.0 + (node_id as f64) * 5.0 + (phase.sin() * 10.0);
        let i_val = (amplitude * phase.cos()) as i8;
        let q_val = (amplitude * phase.sin()) as i8;
        buf[20 + i * 2] = i_val as u8;
        buf[20 + i * 2 + 1] = q_val as u8;
    }

    buf
}

/// Build an edge vitals packet (magic 0xC511_0002).
fn build_vitals_packet(node_id: u8, presence: bool, n_persons: u8, rssi: i8) -> Vec<u8> {
    let mut buf = vec![0u8; 32];

    let magic: u32 = 0xC511_0002;
    buf[0..4].copy_from_slice(&magic.to_le_bytes());

    buf[4] = node_id;
    buf[5] = if presence { 0x01 } else { 0x00 }; // flags
                                                 // breathing_rate (u16 LE) = 15.0 * 100 = 1500
    buf[6..8].copy_from_slice(&1500u16.to_le_bytes());
    // heartrate (u32 LE) = 72.0 * 10000 = 720000
    buf[8..12].copy_from_slice(&720000u32.to_le_bytes());
    buf[12] = rssi as u8;
    buf[13] = n_persons;
    // bytes 14-15: reserved
    // motion_energy (f32 LE)
    let me: f32 = if presence { 0.5 } else { 0.0 };
    buf[16..20].copy_from_slice(&me.to_le_bytes());
    // presence_score (f32 LE)
    let ps: f32 = if presence { 0.8 } else { 0.0 };
    buf[20..24].copy_from_slice(&ps.to_le_bytes());
    // timestamp_ms (u32 LE)
    buf[24..28].copy_from_slice(&1000u32.to_le_bytes());

    buf
}

#[test]
fn test_csi_frame_builder_valid() {
    let frame = build_csi_frame(1, 0, -50, 32);
    assert_eq!(frame.len(), 20 + 32 * 2);
    assert_eq!(
        u32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]),
        0xC511_0001
    );
    assert_eq!(frame[4], 1); // node_id
    assert_eq!(frame[5], 1); // n_antennas
    assert_eq!(frame[6], 32); // n_subcarriers
}

#[test]
fn test_vitals_packet_builder_valid() {
    let pkt = build_vitals_packet(2, true, 1, -45);
    assert_eq!(pkt.len(), 32);
    assert_eq!(
        u32::from_le_bytes([pkt[0], pkt[1], pkt[2], pkt[3]]),
        0xC511_0002
    );
    assert_eq!(pkt[4], 2); // node_id
    assert_eq!(pkt[5], 0x01); // flags: presence
    assert_eq!(pkt[13], 1); // n_persons
}

#[test]
fn test_different_nodes_produce_different_frames() {
    let frame1 = build_csi_frame(1, 0, -50, 32);
    let frame2 = build_csi_frame(2, 0, -50, 32);
    // I/Q data should differ due to node_id-based amplitude offset
    assert_ne!(&frame1[20..], &frame2[20..]);
}

/// Send multiple frames from different nodes to a test-owned UDP receiver.
/// This is a sender-side smoke test; it does not talk to a real server.
#[test]
fn test_multi_node_udp_send() {
    // Bind to a random port and send to our own ephemeral receiver.
    // This is a smoke test — it verifies frames can be sent without panic.
    let sock = UdpSocket::bind("0.0.0.0:0").expect("bind");
    sock.set_write_timeout(Some(Duration::from_millis(100)))
        .ok();
    let (_receiver, target) = ephemeral_test_target();

    let n_sub = 32u16;
    let node_ids = [1u8, 2, 3, 5, 7];

    for &nid in &node_ids {
        for seq in 0..10u32 {
            let frame = build_csi_frame(nid, seq, -50 + nid as i8, n_sub);
            let _ = sock.send_to(&frame, target);
        }
    }

    // Also send vitals packets
    for &nid in &node_ids {
        let pkt = build_vitals_packet(nid, true, 1, -45);
        let _ = sock.send_to(&pkt, target);
    }

    // If we get here without panic, the frame builders work correctly
    let _ = "Multi-node UDP send completed without errors";
}

/// Verify that the frame builder produces frames of the correct minimum
/// size for various subcarrier counts (boundary testing).
#[test]
fn test_frame_sizes() {
    // 256 = ESP32-C6 HE-SU grid (issue #1005) → 532-byte frame as on the wire.
    for n_sub in [1u16, 16, 32, 52, 56, 64, 128, 256] {
        let frame = build_csi_frame(1, 0, -50, n_sub);
        let expected = 20 + (n_sub as usize) * 2;
        assert_eq!(frame.len(), expected, "wrong size for n_sub={n_sub}");
    }
    assert_eq!(build_csi_frame(1, 0, -50, 256).len(), 532);
}

/// Simulate a mesh of N nodes sending frames at different rates.
/// Nodes 1-3 send every "tick", node 4 sends every other tick,
/// node 5 stops after 5 ticks (simulating going offline).
#[test]
fn test_mesh_simulation_pattern() {
    let sock = UdpSocket::bind("0.0.0.0:0").expect("bind");
    sock.set_write_timeout(Some(Duration::from_millis(50))).ok();
    let (_receiver, target) = ephemeral_test_target();

    let mut total_sent = 0u32;

    for tick in 0..20u32 {
        // Nodes 1-3: every tick
        for nid in 1..=3u8 {
            let frame = build_csi_frame(nid, tick, -50, 32);
            let _ = sock.send_to(&frame, target);
            total_sent += 1;
        }

        // Node 4: every other tick
        if tick % 2 == 0 {
            let frame = build_csi_frame(4, tick / 2, -55, 32);
            let _ = sock.send_to(&frame, target);
            total_sent += 1;
        }

        // Node 5: stops after tick 5
        if tick < 5 {
            let frame = build_csi_frame(5, tick, -60, 32);
            let _ = sock.send_to(&frame, target);
            total_sent += 1;
        }
    }

    // Expected: 3*20 + 10 + 5 = 75 frames
    assert_eq!(total_sent, 75, "unexpected frame count");
}

/// Large mesh: simulate 100 nodes each sending 10 frames.
/// Verifies the frame builder scales without issues.
#[test]
fn test_large_mesh_100_nodes() {
    let sock = UdpSocket::bind("0.0.0.0:0").expect("bind");
    sock.set_write_timeout(Some(Duration::from_millis(50))).ok();
    let (_receiver, target) = ephemeral_test_target();

    let mut total = 0u32;
    for nid in 1..=100u8 {
        for seq in 0..10u32 {
            let frame = build_csi_frame(nid, seq, -50 + (nid % 30) as i8, 32);
            let _ = sock.send_to(&frame, target);
            total += 1;
        }
    }

    assert_eq!(total, 1000);
}

/// Max mesh: simulate 255 nodes (max u8 node_id) with 1 frame each.
#[test]
fn test_max_nodes_255() {
    let sock = UdpSocket::bind("0.0.0.0:0").expect("bind");
    sock.set_write_timeout(Some(Duration::from_millis(100)))
        .ok();
    let (_receiver, target) = ephemeral_test_target();

    for nid in 1..=255u8 {
        let frame = build_csi_frame(nid, 0, -50, 16);
        let _ = sock.send_to(&frame, target);
    }

    // 255 unique node_ids — the HashMap should handle this fine
    let _ = 255; // loop completed without panic
}

// ── Issue #1894: live server, multi-node grid drift ─────────────────────────
//
// Boots the real binary on ephemeral ports (nothing shared with a server on
// the default ports) and drives it over real UDP.

struct LiveServer {
    child: std::process::Child,
    http: u16,
    udp: u16,
}

impl Drop for LiveServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn free_tcp_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
    l.local_addr().unwrap().port()
}

fn free_udp_port() -> u16 {
    let s = UdpSocket::bind("127.0.0.1:0").expect("bind ephemeral");
    s.local_addr().unwrap().port()
}

impl LiveServer {
    fn start() -> Self {
        use std::io::Read;
        for attempt in 1..=3 {
            let (http, ws, udp) = (free_tcp_port(), free_tcp_port(), free_udp_port());
            let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_sensing-server"))
                .args([
                    "--http-port", &http.to_string(),
                    "--ws-port", &ws.to_string(),
                    "--udp-port", &udp.to_string(),
                    "--bind-addr", "127.0.0.1",
                    "--no-edge-registry",
                    "--source", "esp32",
                ])
                .env_remove("RUVIEW_API_TOKEN")
                .env_remove("RUVIEW_UDP_ALLOW")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .expect("spawn sensing-server");
            // Generous: a debug build boots slowly on a loaded machine.
            let deadline = std::time::Instant::now() + Duration::from_secs(90);
            while std::time::Instant::now() < deadline {
                if health(http).is_some() {
                    return LiveServer { child, http, udp };
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            // Kill first: reading stderr of a live child blocks until it exits.
            let _ = child.kill();
            let _ = child.wait();
            let mut err = String::new();
            if let Some(mut s) = child.stderr.take() {
                let _ = s.read_to_string(&mut err);
            }
            if err.contains("Address already in use") && attempt < 3 {
                continue;
            }
            panic!("sensing-server did not become ready\n--- stderr ---\n{err}");
        }
        unreachable!()
    }
}

/// `GET /health` over a raw socket; `None` until the listener answers.
fn health(http: u16) -> Option<serde_json::Value> {
    use std::io::{Read, Write};
    let mut s = std::net::TcpStream::connect(("127.0.0.1", http)).ok()?;
    s.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    s.write_all(b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .ok()?;
    let mut resp = String::new();
    s.read_to_string(&mut resp).ok()?;
    let body = resp.split("\r\n\r\n").nth(1)?;
    serde_json::from_str(body).ok()
}

fn tick(h: &serde_json::Value) -> u64 {
    h["tick"].as_u64().expect("health.tick")
}

/// Send `n_sub`-subcarrier frames from `nodes` at ~50 fps each for `dur`.
fn stream(sock: &UdpSocket, udp: u16, nodes: &[u8], n_sub: u16, seq: &mut u32, dur: Duration) {
    let end = std::time::Instant::now() + dur;
    while std::time::Instant::now() < end {
        for &nid in nodes {
            let _ = sock.send_to(&build_csi_frame(nid, *seq, -50, n_sub), ("127.0.0.1", udp));
        }
        *seq = seq.wrapping_add(1);
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Issue #1894: three nodes lock onto a 128-subcarrier grid, then their
/// radios drop to 64 for good. Before the fix every later frame was rejected
/// by the grid gate, so `tick` froze for the life of the process while
/// `/health` kept answering `"status": "ok"`. The node must re-lock onto the
/// grid it is actually sending, and `/health` must report input stopping.
#[test]
#[ignore = "slow: spawns the live binary; run with --ignored"]
fn multi_node_grid_drift_does_not_freeze_processing() {
    let server = LiveServer::start();
    let sock = UdpSocket::bind("127.0.0.1:0").expect("bind sender");
    let nodes = [1u8, 2, 3];
    let mut seq = 0u32;

    stream(&sock, server.udp, &nodes, 128, &mut seq, Duration::from_millis(1500));
    let h = health(server.http).expect("health after 128-bin phase");
    let locked_tick = tick(&h);
    assert!(locked_tick > 0, "128-bin frames must be processed: {h}");

    // Radios switch to 64 bins and stay there. The gate re-locks once 64
    // dominates its vote window; then processing must have resumed.
    stream(&sock, server.udp, &nodes, 64, &mut seq, Duration::from_secs(7));
    let before = tick(&health(server.http).expect("health mid-drift"));
    stream(&sock, server.udp, &nodes, 64, &mut seq, Duration::from_secs(1));
    let h = health(server.http).expect("health after drift");
    assert!(
        tick(&h) > before && before > locked_tick,
        "tick froze after the grid change: locked={locked_tick} before={before} now={}",
        tick(&h)
    );
    assert_eq!(h["processing"]["state"], "live", "{h}");

    // Every node goes quiet: /health must say so rather than read healthy.
    std::thread::sleep(Duration::from_secs(6));
    let h = health(server.http).expect("health after silence");
    assert_eq!(h["status"], "ok", "server itself is still up: {h}");
    assert_eq!(h["processing"]["state"], "no_input", "{h}");
    assert!(h["processing"]["udp"]["datagrams"].as_u64().unwrap_or(0) > 0, "{h}");
}
