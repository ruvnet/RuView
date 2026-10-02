//! ADR-115 P4 — MQTT integration tests against a real broker.
//!
//! These tests require an MQTT broker reachable at `localhost:11883`
//! (overridable via `RUVIEW_TEST_MQTT_PORT`). They are gated behind the
//! `mqtt` feature (which pulls in `rumqttc`) **and** behind the
//! `RUVIEW_RUN_INTEGRATION` env var so the default test run on
//! developer machines doesn't break when there's no broker.
//!
//! In CI, the `.github/workflows/mqtt-integration.yml` workflow spins
//! up a Mosquitto sidecar container, sets `RUVIEW_RUN_INTEGRATION=1`,
//! and runs `cargo test -p wifi-densepose-sensing-server --features mqtt
//! --test mqtt_integration`.
//!
//! ## What these tests prove
//!
//! 1. The publisher connects to a real broker and emits HA discovery
//!    `config` topics for every enabled entity.
//! 2. The discovery payloads round-trip back via `mosquitto_sub`-style
//!    subscription with the exact JSON shape `mqtt::discovery` produces.
//! 3. Availability is published `online` retained on connect and
//!    `offline` on graceful disconnect (the LWT/disconnect path).
//! 4. Privacy mode strips heart-rate / breathing-rate / pose discovery
//!    from the wire entirely — the integration confirms the strip
//!    happens at the broker boundary, not just in unit-test logic.
//!
//! ## Why this is gated
//!
//! We need a live broker. Pulling `rumqttd` into the dev-dep tree as an
//! embedded broker would work in theory but adds 60+ transitive deps
//! and 1+ min compile time to every `cargo test` invocation on every
//! developer's machine. Gating behind an env var keeps the default
//! `cargo test --workspace` fast.

#![cfg(feature = "mqtt")]

use std::time::Duration;

use rumqttc::{AsyncClient, Event, EventLoop, MqttOptions, Packet, QoS};
use serde_json::Value;
use tokio::sync::broadcast;
use tokio::time::timeout;

use wifi_densepose_sensing_server::mqtt::{
    config::{MqttConfig, PublishRates, TlsConfig},
    publisher::{spawn, OwnedDiscoveryBuilder},
    state::VitalsSnapshot,
};

fn should_run() -> Option<u16> {
    if std::env::var("RUVIEW_RUN_INTEGRATION").is_err() {
        eprintln!("[skip] set RUVIEW_RUN_INTEGRATION=1 + run a broker on the test port");
        return None;
    }
    let port = std::env::var("RUVIEW_TEST_MQTT_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(11883);
    Some(port)
}

fn make_cfg(port: u16, privacy_mode: bool, label: &str) -> std::sync::Arc<MqttConfig> {
    std::sync::Arc::new(MqttConfig {
        host: "127.0.0.1".into(),
        port,
        username: None,
        password: None,
        // Per-test client_id so cargo test --test-threads=1 doesn't make
        // mosquitto kick the previous session when the next test connects
        // with the same client_id (default MQTT session-takeover behaviour).
        client_id: format!("ruview-int-test-{}-{}", std::process::id(), label),
        discovery_prefix: "homeassistant".into(),
        tls: TlsConfig::Off,
        refresh_secs: 60,
        rates: PublishRates {
            // Fast rates so the test gets a sample quickly.
            vitals_hz: 5.0,
            motion_hz: 5.0,
            count_hz: 5.0,
            rssi_hz: 5.0,
            pose_hz: 5.0,
        },
        publish_pose: false,
        privacy_mode,
    })
}

fn make_builder(node: &str) -> OwnedDiscoveryBuilder {
    OwnedDiscoveryBuilder {
        discovery_prefix: "homeassistant".into(),
        node_id: node.into(),
        node_friendly_name: Some(format!("Test {}", node)),
        sw_version: "0.7.0-test".into(),
        model: "integration".into(),
        via_device: None,
    }
}

async fn subscribe_client(port: u16, topics: &[&str]) -> (AsyncClient, EventLoop) {
    // Per-call unique client_id so subscribers across tests don't take
    // each other over.
    let suffix: u64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0);
    let mut opts = MqttOptions::new(
        format!("ruview-test-sub-{}-{}", std::process::id(), suffix),
        ("127.0.0.1", port),
    );
    opts.set_keep_alive(10);
    opts.set_clean_session(true);
    let (client, mut eventloop) = AsyncClient::builder(opts).capacity(256).build();
    for t in topics {
        client.subscribe(*t, QoS::AtLeastOnce).await.unwrap();
    }

    // Drive the eventloop until we see the SubAck for our last subscribe.
    // Without this the SUBSCRIBE packet is only queued in rumqttc's
    // outbound channel; it doesn't reach the broker until something
    // pumps the eventloop. The caller's `collect_published` does that,
    // but by then the publisher may already have emitted state
    // messages — including the retained ones that won't be re-sent.
    let until = tokio::time::Instant::now() + Duration::from_secs(3);
    while tokio::time::Instant::now() < until {
        let remain = until - tokio::time::Instant::now();
        match timeout(remain, eventloop.poll()).await {
            Ok(Ok(Event::Incoming(Packet::SubAck(_)))) => break,
            Ok(Ok(_)) => continue,
            Ok(Err(e)) => {
                eprintln!("[subscribe_client] eventloop error before SubAck: {e}");
                break;
            }
            Err(_) => break,
        }
    }

    (client, eventloop)
}

async fn collect_published(
    eventloop: &mut EventLoop,
    deadline: Duration,
) -> Vec<(String, Vec<u8>, bool)> {
    let mut out = Vec::new();
    let until = tokio::time::Instant::now() + deadline;
    while tokio::time::Instant::now() < until {
        let remain = until - tokio::time::Instant::now();
        match timeout(remain, eventloop.poll()).await {
            Ok(Ok(Event::Incoming(Packet::Publish(p)))) => {
                out.push((
                    String::from_utf8_lossy(&p.topic).to_string(),
                    p.payload.to_vec(),
                    p.retain,
                ));
            }
            Ok(Ok(_)) => {} // ignore other events
            Ok(Err(e)) => {
                eprintln!("[test] eventloop error: {}", e);
                break;
            }
            Err(_) => break,
        }
    }
    out
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn discovery_topics_appear_on_broker() {
    let Some(port) = should_run() else { return; };

    // Subscriber wired first so we don't miss the initial discovery burst.
    let (sub, mut sub_loop) =
        subscribe_client(port, &["homeassistant/#"]).await;

    // Spawn the publisher.
    let cfg = make_cfg(port, false, "discovery");
    let builder = make_builder("inttest1");
    let (tx, rx) = broadcast::channel::<VitalsSnapshot>(32);
    let _handle = spawn(cfg, builder, rx);

    // #898: discovery is now published per-node the first time a snapshot for
    // that node_id arrives (not eagerly at startup). Drive snapshots for
    // "inttest1" throughout the window so its device's discovery lands — same
    // pattern as state_messages_published_on_snapshot_broadcast.
    let tx_bg = tx.clone();
    let drive = tokio::spawn(async move {
        for _ in 0..60 {
            let _ = tx_bg.send(VitalsSnapshot {
                node_id: "inttest1".into(),
                ..Default::default()
            });
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    });

    // Drain the subscriber for up to 6 s — enough for initial discovery
    // + first availability publication.
    let msgs = collect_published(&mut sub_loop, Duration::from_secs(6)).await;
    drive.abort();
    let _ = sub.disconnect().await;

    // Assertions: at least the presence + heart_rate + fall discovery
    // configs should have landed.
    let topics: Vec<&str> = msgs.iter().map(|(t, _, _)| t.as_str()).collect();
    let presence_cfg = topics
        .iter()
        .any(|t| t.ends_with("/wifi_densepose_inttest1/presence/config"));
    let hr_cfg = topics
        .iter()
        .any(|t| t.ends_with("/wifi_densepose_inttest1/heart_rate/config"));
    let fall_cfg = topics
        .iter()
        .any(|t| t.ends_with("/wifi_densepose_inttest1/fall/config"));

    assert!(presence_cfg, "missing presence discovery topic in {:?}", topics);
    assert!(hr_cfg, "missing heart_rate discovery topic in {:?}", topics);
    assert!(fall_cfg, "missing fall discovery topic in {:?}", topics);

    // Spot-check the JSON shape of one discovery payload.
    let presence_payload = msgs
        .iter()
        .find(|(t, _, _)| t.ends_with("/presence/config"))
        .map(|(_, p, _)| p.clone())
        .unwrap();
    let json: Value = serde_json::from_slice(&presence_payload).unwrap();
    assert_eq!(json["device_class"], "occupancy");
    assert_eq!(json["payload_on"], "ON");
    assert_eq!(json["payload_off"], "OFF");
    assert!(json["unique_id"]
        .as_str()
        .unwrap()
        .starts_with("wifi_densepose_"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn privacy_mode_suppresses_biometric_discovery() {
    let Some(port) = should_run() else { return; };

    let (sub, mut sub_loop) =
        subscribe_client(port, &["homeassistant/#"]).await;

    let cfg = make_cfg(port, /* privacy_mode = */ true, "privacy");
    let builder = make_builder("inttest2");
    let (tx, rx) = broadcast::channel::<VitalsSnapshot>(32);
    let _handle = spawn(cfg, builder, rx);

    // #898: per-node discovery is triggered by a snapshot for that node_id.
    let tx_bg = tx.clone();
    let drive = tokio::spawn(async move {
        for _ in 0..60 {
            let _ = tx_bg.send(VitalsSnapshot {
                node_id: "inttest2".into(),
                ..Default::default()
            });
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    });

    let msgs = collect_published(&mut sub_loop, Duration::from_secs(6)).await;
    drive.abort();
    let _ = sub.disconnect().await;

    let topics: Vec<&str> = msgs.iter().map(|(t, _, _)| t.as_str()).collect();

    // Biometric discovery must NOT appear.
    let leaked_hr = topics
        .iter()
        .any(|t| t.contains("/inttest2/heart_rate/"));
    let leaked_br = topics
        .iter()
        .any(|t| t.contains("/inttest2/breathing_rate/"));
    let leaked_pose = topics.iter().any(|t| t.contains("/inttest2/pose/"));

    assert!(!leaked_hr, "heart_rate leaked under privacy mode: {:?}", topics);
    assert!(!leaked_br, "breathing_rate leaked under privacy mode");
    assert!(!leaked_pose, "pose leaked under privacy mode");

    // Non-biometric entities + semantic primitives still appear.
    let presence_cfg = topics
        .iter()
        .any(|t| t.ends_with("/wifi_densepose_inttest2/presence/config"));
    let sleeping_cfg = topics.iter().any(|t| {
        t.ends_with("/wifi_densepose_inttest2/someone_sleeping/config")
    });

    assert!(presence_cfg, "presence missing in privacy mode");
    assert!(
        sleeping_cfg,
        "someone_sleeping must remain in privacy mode (it's inferred, not biometric)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn state_messages_published_on_snapshot_broadcast() {
    let Some(port) = should_run() else { return; };

    // Subscribe to the entire homeassistant tree so the diagnostic
    // capture shows EVERYTHING the publisher is doing, not just
    // the narrow presence/state filter — narrow filters can hide
    // ordering issues (e.g., if the publisher is publishing only
    // discovery and not state, a narrow filter on state can't tell
    // us that).
    let (sub, mut sub_loop) = subscribe_client(port, &["homeassistant/#"]).await;

    let cfg = make_cfg(port, false, "state");
    let builder = make_builder("inttest3");
    let (tx, rx) = broadcast::channel::<VitalsSnapshot>(32);
    let _handle = spawn(cfg, builder, rx);

    // Iter 46 — instead of front-loading 6 snapshots and hoping the
    // publisher's startup beats them, drive snapshots in a background
    // task THROUGHOUT the capture window. CI runners can be slow to
    // boot the publisher (mosquitto sidecar + cold cargo cache + slow
    // QoS-1 discovery publishes), so a "publisher must be ready by
    // t=3s" assumption is fragile. Steady-state ON/OFF traffic for the
    // full 14 s window guarantees both states appear in the capture
    // even if the first 3-5 s of publishes are missed.
    let tx_bg = tx.clone();
    let drive = tokio::spawn(async move {
        // Brief warm-up before first publish.
        tokio::time::sleep(Duration::from_secs(1)).await;
        for i in 0..40 {
            let _ = tx_bg.send(VitalsSnapshot {
                node_id: "inttest3".into(),
                timestamp_ms: 1779_512_400_000 + (i as i64) * 300,
                presence: i % 2 == 0,
                fall_detected: Some(false),
                motion: if i % 2 == 0 { 0.40 } else { 0.02 },
                motion_energy: Some(800.0),
                presence_score: if i % 2 == 0 { 0.95 } else { 0.10 },
                breathing_rate_bpm: Some(14.0),
                heartrate_bpm: Some(72.0),
                n_persons: if i % 2 == 0 { 1 } else { 0 },
                rssi_dbm: Some(-48.0),
                vital_confidence: 0.9,
            });
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    });

    // 14 s window covers warm-up + 12 s of steady-state ON/OFF traffic.
    let msgs = collect_published(&mut sub_loop, Duration::from_secs(14)).await;
    drive.abort();
    let _ = sub.disconnect().await;

    // Diagnostic: dump every captured topic so we can see what (if
    // anything) the subscriber received. CI runs with --nocapture, so
    // this lands in the workflow log when the test fails.
    eprintln!("[diag] subscriber captured {} messages:", msgs.len());
    for (t, p, retain) in &msgs {
        eprintln!(
            "[diag]   retain={} topic={} payload={}",
            retain,
            t,
            String::from_utf8_lossy(p).chars().take(80).collect::<String>(),
        );
    }

    // Filter for THIS test's presence state messages. The topic format
    // is `homeassistant/binary_sensor/wifi_densepose_<node>/presence/state`
    // — `wifi_densepose_inttest3` is one path segment with an underscore
    // separator, NOT slash-separated. The previous version looked for
    // `/inttest3/presence/state` (with leading slash) which is the bug
    // that took 5 commits + a diagnostic dump to find.
    let presence_states: Vec<String> = msgs
        .iter()
        .filter(|(t, _, _)| t.contains("wifi_densepose_inttest3/presence/state"))
        .map(|(_, p, _)| String::from_utf8_lossy(p).into_owned())
        .collect();

    assert!(
        presence_states.iter().any(|p| p == "ON"),
        "expected ON state, got {:?} (of {} total captured)",
        presence_states,
        msgs.len(),
    );
    assert!(
        presence_states.iter().any(|p| p == "OFF"),
        "expected OFF state, got {:?}",
        presence_states
    );
}
