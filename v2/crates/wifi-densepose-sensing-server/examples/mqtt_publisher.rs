//! ADR-115 P6 — minimal runnable example wiring the MQTT publisher
//! against a broadcast channel of `VitalsSnapshot`s.
//!
//! Run with:
//!     cargo run --release -p wifi-densepose-sensing-server \
//!         --features mqtt --example mqtt_publisher -- \
//!         --mqtt --mqtt-host 127.0.0.1
//!
//! Then in another terminal:
//!     mosquitto_sub -h 127.0.0.1 -t 'homeassistant/#' -v
//!
//! You should see one HA discovery `config` topic per entity per node
//! land within a second of startup, followed by `state` topics ticking
//! at the configured rates.
//!
//! This example is the production-wiring blueprint for `main.rs`:
//! every line below is what the binary's startup path should do when
//! `args.mqtt` is true. Keeping it in `examples/` lets us validate the
//! wiring end-to-end without touching the 6000-line main.rs (which is
//! the active edit surface of the parallel ADR-110 agent — see
//! [[feedback-multi-agent-worktree]]).

// The full example body needs the `mqtt` feature (rumqttc, publisher::spawn,
// etc.). When the feature is off we provide a stub `main` so the example
// still compiles cleanly during a default `cargo build --workspace` —
// otherwise CI fails with E0601 (`main function not found`) on every PR
// that touches the workspace, even ones unrelated to ADR-115.
#[cfg(not(feature = "mqtt"))]
fn main() {
    eprintln!(
        "This example requires --features mqtt. Re-run with: \n  \
         cargo run -p wifi-densepose-sensing-server --features mqtt \
         --example mqtt_publisher -- --mqtt"
    );
    std::process::exit(2);
}

#[cfg(feature = "mqtt")]
use std::sync::Arc;
#[cfg(feature = "mqtt")]
use std::time::Duration;

#[cfg(feature = "mqtt")]
use clap::Parser;
#[cfg(feature = "mqtt")]
use tokio::sync::broadcast;
#[cfg(feature = "mqtt")]
use tracing::info;
#[cfg(feature = "mqtt")]
use wifi_densepose_sensing_server::cli::MqttArgs;
#[cfg(feature = "mqtt")]
use wifi_densepose_sensing_server::mqtt::{
    config::MqttConfig,
    publisher::{spawn, OwnedDiscoveryBuilder},
    security::audit,
    state::VitalsSnapshot,
};

#[cfg(feature = "mqtt")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let (args, privacy_mode) = {
        use clap::Parser;
        #[derive(Parser)]
        struct W {
            #[command(flatten)]
            m: MqttArgs,
            /// Strip biometrics (HR/BR/pose) from MQTT
            #[arg(long, env = "RUVIEW_PRIVACY_MODE")]
            privacy_mode: bool,
        }
        let w = W::parse();
        (w.m, w.privacy_mode)
    };

    if !args.mqtt {
        eprintln!("This example requires --mqtt. Aborting.");
        std::process::exit(2);
    }

    // 1. Build MqttConfig from CLI + run the security audit before any
    //    network I/O. A failed audit short-circuits with a clear error.
    let mut cfg = MqttConfig::from_args(&args);
    cfg.privacy_mode = privacy_mode;
    let cfg = Arc::new(cfg);
    match audit(&cfg) {
        Ok(()) => {}
        Err(e) if !e.is_fatal() => {
            tracing::warn!(error = %e, "non-fatal MQTT audit advisory");
        }
        Err(e) => {
            eprintln!("MQTT audit failed: {e}");
            std::process::exit(1);
        }
    }

    // 2. The DiscoveryBuilder owns the per-node identity. In a real
    //    deployment each ESP32 node would get its own builder; here we
    //    fake one for demonstration.
    let builder = OwnedDiscoveryBuilder {
        discovery_prefix: cfg.discovery_prefix.clone(),
        node_id: "example_node".into(),
        node_friendly_name: Some("Example RuView Node".into()),
        sw_version: env!("CARGO_PKG_VERSION").into(),
        model: "ESP32-S3 CSI node (example)".into(),
        via_device: None,
    };

    // 3. Broadcast channel — `sensing-server` already creates one of
    //    these in main.rs (the one the WebSocket handler subscribes to).
    //    We mirror it here.
    let (tx, rx) = broadcast::channel::<VitalsSnapshot>(256);

    // 4. Spawn the publisher. It returns a JoinHandle the caller can
    //    await on shutdown.
    let publisher = spawn(cfg.clone(), builder, rx);
    info!("publisher spawned, sending demo snapshots every 500ms");

    // 5. Demo loop — produce a fresh VitalsSnapshot every 500ms with
    //    alternating presence so HA sees ON/OFF transitions.
    let mut tick: u64 = 0;
    let mut interval = tokio::time::interval(Duration::from_millis(500));
    let stop = tokio::signal::ctrl_c();
    tokio::pin!(stop);
    loop {
        tokio::select! {
            _ = interval.tick() => {
                tick += 1;
                let snap = VitalsSnapshot {
                    node_id: "example_node".into(),
                    timestamp_ms: chrono::Utc::now().timestamp_millis(),
                    presence: tick % 20 < 10,
                    fall_detected: Some(tick % 60 == 30),
                    motion: 0.10 + ((tick as f64).sin().abs() * 0.30),
                    motion_energy: Some(1000.0 + (tick as f64).cos() * 200.0),
                    presence_score: 0.85,
                    breathing_rate_bpm: Some(13.0 + ((tick as f64) * 0.05).sin()),
                    heartrate_bpm: Some(68.0 + ((tick as f64) * 0.03).sin() * 5.0),
                    n_persons: if tick % 20 < 10 { 1 } else { 0 },
                    rssi_dbm: Some(-50.0 + ((tick as f64) * 0.1).sin() * 5.0),
                    vital_confidence: 0.85,
                };
                let _ = tx.send(snap);
            }
            _ = &mut stop => {
                info!("ctrl-c received, shutting down");
                break;
            }
        }
    }

    drop(tx); // close broadcast → publisher publishes `offline` + disconnects.
    let _ = tokio::time::timeout(Duration::from_secs(2), publisher).await;
    Ok(())
}
