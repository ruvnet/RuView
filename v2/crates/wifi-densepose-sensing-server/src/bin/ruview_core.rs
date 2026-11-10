//! RuView Core — Physical RF Sensing Micro-Daemon.
//!
//! Provides ultra-lightweight, verified physical sensing:
//! - Human Presence & Motion Energy (Goertzel variance & temporal debouncing)
//! - Respiration Rate (subcarrier spectral analysis in [0.1..0.5] Hz)
//! - Acute Fall Detection (Doppler shockwave & edge vitals alert)
//!
//! Complies strictly with ELMAS.md:
//! - <= 350 lines of code
//! - <= 3 levels of indentation
//! - Zero 3D/Three.js simulation overhead
//! - Low RAM consumption (< 15 MB)

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use clap::Parser;
use serde::Serialize;
use tokio::net::UdpSocket;
use tokio::sync::watch;

use wifi_densepose_sensing_server::domain::smoothing::{PresenceFilter, VitalsFilter};
use wifi_densepose_sensing_server::domain::types::FeatureInfo;
use wifi_densepose_sensing_server::dsp::{
    estimate_breathing_rate_hz, extract_features_from_frame,
};
use wifi_densepose_sensing_server::ingest::{
    generate_simulated_frame, parse_esp32_frame, parse_esp32_vitals, Esp32Frame,
};
use wifi_densepose_sensing_server::vital_signs::VitalSignDetector;

/// Physical Sensing Telemetry Event.
#[derive(Debug, Clone, Serialize)]
pub struct PhysicalTelemetry {
    pub timestamp_ms: u64,
    pub presence: bool,
    pub motion_level: String,
    pub person_score: f64,
    pub breathing_bpm: Option<f64>,
    pub heart_rate_bpm: Option<f64>,
    pub fall_detected: bool,
    pub rssi_dbm: f64,
    pub frames_processed: u64,
    pub source: String,
}

#[derive(Parser, Debug)]
#[command(name = "ruview-core")]
#[command(about = "RuView Physical RF Sensing Engine (Presence + Vitals + Falls)")]
struct CliArgs {
    /// UDP listening port for incoming ESP32 CSI datagrams.
    #[arg(short, long, default_value_t = 5005)]
    port: u16,

    /// Sensing source: 'esp32', 'simulated', or 'auto'.
    #[arg(short, long, default_value = "auto")]
    source: String,

    /// Emit NDJSON telemetry events to stdout.
    #[arg(long, default_value_t = true)]
    json: bool,

    /// Optional HTTP port for lightweight REST status and health check.
    #[arg(long)]
    http_port: Option<u16>,
}

struct EngineContext {
    history: VecDeque<Vec<f64>>,
    presence_filter: PresenceFilter,
    vitals_filter: VitalsFilter,
    vital_detector: VitalSignDetector,
    frames_processed: u64,
    last_fall_time: Option<Instant>,
    latest_rssi: f64,
    source_name: String,
}

impl EngineContext {
    fn new(source_name: String) -> Self {
        Self {
            history: VecDeque::with_capacity(100),
            presence_filter: PresenceFilter::new(),
            vitals_filter: VitalsFilter::new(),
            vital_detector: VitalSignDetector::new(20.0),
            frames_processed: 0,
            last_fall_time: None,
            latest_rssi: -50.0,
            source_name,
        }
    }

    fn process_frame(&mut self, frame: &Esp32Frame) -> PhysicalTelemetry {
        self.frames_processed += 1;
        self.latest_rssi = frame.rssi as f64;

        if self.history.len() >= 100 {
            self.history.pop_front();
        }
        self.history.push_back(frame.amplitudes.clone());

        let (feat, mut raw_cls, raw_motion, _, _) =
            extract_features_from_frame(frame, &self.history, 20.0);

        self.presence_filter.smooth(&mut raw_cls, raw_motion, 20.0);

        let mut raw_vitals = self.vital_detector.process_frame(&frame.amplitudes, &frame.phases);
        let hz = estimate_breathing_rate_hz(&self.history, 20.0);
        if hz > 0.0 {
            raw_vitals.breathing_rate_bpm = Some(hz * 60.0);
        }

        let smoothed_vitals = self.vitals_filter.smooth(&raw_vitals);
        let is_acute_fall = self.evaluate_fall_condition(&feat);

        PhysicalTelemetry {
            timestamp_ms: chrono::Utc::now().timestamp_millis().max(0) as u64,
            presence: raw_cls.presence,
            motion_level: raw_cls.motion_level,
            person_score: (0.4 + self.presence_filter.smoothed_motion * 0.6).clamp(0.0, 1.0),
            breathing_bpm: smoothed_vitals.breathing_rate_bpm,
            heart_rate_bpm: smoothed_vitals.heart_rate_bpm,
            fall_detected: is_acute_fall,
            rssi_dbm: self.latest_rssi,
            frames_processed: self.frames_processed,
            source: self.source_name.clone(),
        }
    }

    fn evaluate_fall_condition(&mut self, feat: &FeatureInfo) -> bool {
        let is_recent_fall = self.last_fall_time.is_some_and(|t| t.elapsed() < Duration::from_secs(3));
        if is_recent_fall {
            return true;
        }

        let is_shockwave = feat.motion_band_power > 45.0 && feat.change_points >= 4;
        if is_shockwave {
            self.last_fall_time = Some(Instant::now());
            return true;
        }

        false
    }
}

async fn run_http_server(port: u16, rx: watch::Receiver<Option<PhysicalTelemetry>>) {
    use axum::{routing::get, Json, Router};

    let app = Router::new()
        .route("/health", get(|| async { Json(serde_json::json!({"status": "ok"})) }))
        .route(
            "/status",
            get(move || {
                let current = rx.borrow().clone();
                async move {
                    match current {
                        Some(t) => Json(serde_json::to_value(t).unwrap()),
                        None => Json(serde_json::json!({"status": "warming_up"})),
                    }
                }
            }),
        );

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    eprintln!("[RuView Core] REST Telemetry listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let _ = axum::serve(listener, app).await;
}

async fn run_udp_receiver(
    socket: UdpSocket,
    tx: watch::Sender<Option<PhysicalTelemetry>>,
    emit_json: bool,
) {
    let mut ctx = EngineContext::new("esp32".to_string());
    let mut buf = [0u8; 4096];

    while let Ok((len, _)) = socket.recv_from(&mut buf).await {
        if len == 0 {
            continue;
        }

        if let Some(edge) = parse_esp32_vitals(&buf[..len]) {
            if edge.fall_detected {
                ctx.last_fall_time = Some(Instant::now());
            }
            continue;
        }

        let Some(frame) = parse_esp32_frame(&buf[..len]) else {
            continue;
        };

        let telemetry = ctx.process_frame(&frame);
        if emit_json {
            println!("{}", serde_json::to_string(&telemetry).unwrap());
        }
        let _ = tx.send(Some(telemetry));
    }
}

async fn run_simulator_loop(tx: watch::Sender<Option<PhysicalTelemetry>>, emit_json: bool) {
    let mut ctx = EngineContext::new("simulated".to_string());
    let mut tick = 0u64;
    let mut interval = tokio::time::interval(Duration::from_millis(50));

    loop {
        interval.tick().await;
        tick += 1;
        let frame = generate_simulated_frame(tick);
        let telemetry = ctx.process_frame(&frame);

        if emit_json {
            println!("{}", serde_json::to_string(&telemetry).unwrap());
        }
        let _ = tx.send(Some(telemetry));
    }
}

#[tokio::main]
async fn main() {
    let args = CliArgs::parse();
    let (tx, rx) = watch::channel(None);

    if let Some(http_port) = args.http_port {
        let http_rx = rx.clone();
        tokio::spawn(async move {
            run_http_server(http_port, http_rx).await;
        });
    }

    let bind_addr = format!("0.0.0.0:{}", args.port);
    let socket_res = UdpSocket::bind(&bind_addr).await;

    let use_simulation = match (args.source.as_str(), socket_res) {
        ("simulated", _) => true,
        (_, Err(e)) => {
            eprintln!("[RuView Core] WARN: Could not bind {bind_addr}: {e}. Falling back to simulation.");
            true
        }
        ("esp32", Ok(sock)) => {
            eprintln!("[RuView Core] Listening for ESP32 CSI on {bind_addr}...");
            run_udp_receiver(sock, tx, args.json).await;
            return;
        }
        ("auto", Ok(sock)) => {
            eprintln!("[RuView Core] Starting UDP receiver on {bind_addr} (auto-detection)...");
            let sock_tx = tx.clone();
            tokio::spawn(async move {
                run_udp_receiver(sock, sock_tx, args.json).await;
            });
            false
        }
        _ => true,
    };

    if use_simulation {
        eprintln!("[RuView Core] Running high-fidelity RF physical simulator (20 Hz)...");
        run_simulator_loop(tx, args.json).await;
    }
}
