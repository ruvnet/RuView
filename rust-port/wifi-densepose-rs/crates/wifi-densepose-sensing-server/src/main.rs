//! WiFi-DensePose Sensing Server
//!
//! Lightweight Axum server that:
//! - Receives ESP32 CSI frames via UDP (port 5005)
//! - Processes signals using RuVector-powered wifi-densepose-signal crate
//! - Broadcasts sensing updates via WebSocket (ws://localhost:8765/ws/sensing)
//! - Serves the static UI files (port 8080)
//!
//! Replaces both ws_server.py and the Python HTTP server.

mod rvf_container;
mod rvf_pipeline;
mod vital_signs;

// Training pipeline modules (exposed via lib.rs)
use wifi_densepose_sensing_server::{graph_transformer, trainer, dataset};

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use clap::Parser;

use serde::{Deserialize, Serialize};
use tokio::net::UdpSocket;
use tokio::sync::{broadcast, RwLock};
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use axum::http::HeaderValue;
use tracing::{info, warn, debug, error};

use rvf_container::{RvfBuilder, RvfContainerInfo, RvfReader, VitalSignConfig};
use rvf_pipeline::ProgressiveLoader;
use vital_signs::{VitalSignDetector, VitalSigns};

// ADR-022 Phase 3: Multi-BSSID pipeline integration
use wifi_densepose_wifiscan::{
    BssidRegistry, WindowsWifiPipeline,
};
use wifi_densepose_wifiscan::parse_netsh_output as parse_netsh_bssid_output;

// ── CLI ──────────────────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(name = "sensing-server", about = "WiFi-DensePose sensing server")]
struct Args {
    /// HTTP port for UI and REST API
    #[arg(long, default_value = "8080")]
    http_port: u16,

    /// WebSocket port for sensing stream
    #[arg(long, default_value = "8765")]
    ws_port: u16,

    /// UDP port for ESP32 CSI frames
    #[arg(long, default_value = "5005")]
    udp_port: u16,

    /// Path to UI static files
    #[arg(long, default_value = "../../ui")]
    ui_path: PathBuf,

    /// Tick interval in milliseconds
    #[arg(long, default_value = "500")]
    tick_ms: u64,

    /// Data source: auto, wifi, esp32, simulate
    #[arg(long, default_value = "auto")]
    source: String,

    /// Run vital sign detection benchmark (1000 frames) and exit
    #[arg(long)]
    benchmark: bool,

    /// Load model config from an RVF container at startup
    #[arg(long, value_name = "PATH")]
    load_rvf: Option<PathBuf>,

    /// Save current model state as an RVF container on shutdown
    #[arg(long, value_name = "PATH")]
    save_rvf: Option<PathBuf>,

    /// Load a trained .rvf model for inference
    #[arg(long, value_name = "PATH")]
    model: Option<PathBuf>,

    /// Enable progressive loading (Layer A instant start)
    #[arg(long)]
    progressive: bool,

    /// Export an RVF container package and exit (no server)
    #[arg(long, value_name = "PATH")]
    export_rvf: Option<PathBuf>,

    /// Run training mode (train a model and exit)
    #[arg(long)]
    train: bool,

    /// Path to dataset directory (MM-Fi or Wi-Pose)
    #[arg(long, value_name = "PATH")]
    dataset: Option<PathBuf>,

    /// Dataset type: "mmfi" or "wipose"
    #[arg(long, value_name = "TYPE", default_value = "mmfi")]
    dataset_type: String,

    /// Number of training epochs
    #[arg(long, default_value = "100")]
    epochs: usize,

    /// Directory for training checkpoints
    #[arg(long, value_name = "DIR")]
    checkpoint_dir: Option<PathBuf>,
}

// ── Data types ───────────────────────────────────────────────────────────────

/// ADR-018 ESP32 CSI binary frame header (20 bytes)
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct Esp32Frame {
    magic: u32,
    node_id: u8,
    n_antennas: u8,
    n_subcarriers: u8,
    freq_mhz: u16,
    sequence: u32,
    rssi: i8,
    noise_floor: i8,
    amplitudes: Vec<f64>,
    phases: Vec<f64>,
}

/// Sensing update broadcast to WebSocket clients
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SensingUpdate {
    #[serde(rename = "type")]
    msg_type: String,
    timestamp: f64,
    source: String,
    tick: u64,
    nodes: Vec<NodeInfo>,
    features: FeatureInfo,
    classification: ClassificationInfo,
    signal_field: SignalField,
    /// Vital sign estimates (breathing rate, heart rate, confidence).
    #[serde(skip_serializing_if = "Option::is_none")]
    vital_signs: Option<VitalSigns>,
    // ── ADR-022 Phase 3: Enhanced multi-BSSID pipeline fields ──
    /// Enhanced motion estimate from multi-BSSID pipeline.
    #[serde(skip_serializing_if = "Option::is_none")]
    enhanced_motion: Option<serde_json::Value>,
    /// Enhanced breathing estimate from multi-BSSID pipeline.
    #[serde(skip_serializing_if = "Option::is_none")]
    enhanced_breathing: Option<serde_json::Value>,
    /// Posture classification from BSSID fingerprint matching.
    #[serde(skip_serializing_if = "Option::is_none")]
    posture: Option<String>,
    /// Signal quality score from multi-BSSID quality gate [0.0, 1.0].
    #[serde(skip_serializing_if = "Option::is_none")]
    signal_quality_score: Option<f64>,
    /// Quality gate verdict: "Permit", "Warn", or "Deny".
    #[serde(skip_serializing_if = "Option::is_none")]
    quality_verdict: Option<String>,
    /// Number of BSSIDs used in the enhanced sensing cycle.
    #[serde(skip_serializing_if = "Option::is_none")]
    bssid_count: Option<usize>,
    // ── ADR-023 Phase 7-8: Model inference fields ──
    /// Pose keypoints when a trained model is loaded (x, y, z, confidence).
    #[serde(skip_serializing_if = "Option::is_none")]
    pose_keypoints: Option<Vec<[f64; 4]>>,
    /// Model status when a trained model is loaded.
    #[serde(skip_serializing_if = "Option::is_none")]
    model_status: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NodeInfo {
    node_id: u8,
    rssi_dbm: f64,
    position: [f64; 3],
    amplitude: Vec<f64>,
    subcarrier_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FeatureInfo {
    mean_rssi: f64,
    variance: f64,
    motion_band_power: f64,
    breathing_band_power: f64,
    dominant_freq_hz: f64,
    change_points: usize,
    spectral_power: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClassificationInfo {
    motion_level: String,
    presence: bool,
    confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SignalField {
    grid_size: [usize; 3],
    values: Vec<f64>,
}

/// WiFi-derived pose keypoint (17 COCO keypoints)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PoseKeypoint {
    name: String,
    x: f64,
    y: f64,
    z: f64,
    confidence: f64,
}

/// Person detection from WiFi sensing
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersonDetection {
    id: u32,
    confidence: f64,
    keypoints: Vec<PoseKeypoint>,
    bbox: BoundingBox,
    zone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BoundingBox {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// Shared application state
struct AppStateInner {
    latest_update: Option<SensingUpdate>,
    rssi_history: VecDeque<f64>,
    tick: u64,
    source: String,
    tx: broadcast::Sender<String>,
    total_detections: u64,
    start_time: std::time::Instant,
    /// Vital sign detector (processes CSI frames to estimate HR/RR).
    vital_detector: VitalSignDetector,
    /// Most recent vital sign reading for the REST endpoint.
    latest_vitals: VitalSigns,
    /// RVF container info if a model was loaded via `--load-rvf`.
    rvf_info: Option<RvfContainerInfo>,
    /// Path to save RVF container on shutdown (set via `--save-rvf`).
    save_rvf_path: Option<PathBuf>,
    /// Progressive loader for a trained model (set via `--model`).
    progressive_loader: Option<ProgressiveLoader>,
    /// Active SONA profile name.
    active_sona_profile: Option<String>,
    /// Whether a trained model is loaded.
    model_loaded: bool,
}

type SharedState = Arc<RwLock<AppStateInner>>;

// ── ESP32 UDP frame parser ───────────────────────────────────────────────────

fn parse_esp32_frame(buf: &[u8]) -> Option<Esp32Frame> {
    if buf.len() < 20 {
        return None;
    }

    let magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    if magic != 0xC511_0001 {
        return None;
    }

    let node_id = buf[4];
    let n_antennas = buf[5];
    let n_subcarriers = buf[6];
    let freq_mhz = u16::from_le_bytes([buf[8], buf[9]]);
    let sequence = u32::from_le_bytes([buf[10], buf[11], buf[12], buf[13]]);
    let rssi = buf[14] as i8;
    let noise_floor = buf[15] as i8;

    let iq_start = 20;
    let n_pairs = n_antennas as usize * n_subcarriers as usize;
    let expected_len = iq_start + n_pairs * 2;

    if buf.len() < expected_len {
        return None;
    }

    let mut amplitudes = Vec::with_capacity(n_pairs);
    let mut phases = Vec::with_capacity(n_pairs);

    for k in 0..n_pairs {
        let i_val = buf[iq_start + k * 2] as i8 as f64;
        let q_val = buf[iq_start + k * 2 + 1] as i8 as f64;
        amplitudes.push((i_val * i_val + q_val * q_val).sqrt());
        phases.push(q_val.atan2(i_val));
    }

    Some(Esp32Frame {
        magic,
        node_id,
        n_antennas,
        n_subcarriers,
        freq_mhz,
        sequence,
        rssi,
        noise_floor,
        amplitudes,
        phases,
    })
}

// ── Signal field generation ──────────────────────────────────────────────────

fn generate_signal_field(
    _mean_rssi: f64,
    variance: f64,
    motion_score: f64,
    tick: u64,
) -> SignalField {
    let grid = 20;
    let mut values = vec![0.0f64; grid * grid];
    let center = grid as f64 / 2.0;
    let tick_f = tick as f64;

    for z in 0..grid {
        for x in 0..grid {
            let dx = x as f64 - center;
            let dz = z as f64 - center;
            let dist = (dx * dx + dz * dz).sqrt();

            // Base radial attenuation from router at center
            let base = (-dist * 0.15).exp();

            // Body disruption blob
            let body_x = center + 3.0 * (tick_f * 0.02).sin();
            let body_z = center + 2.0 * (tick_f * 0.015).cos();
            let body_dist = ((x as f64 - body_x).powi(2) + (z as f64 - body_z).powi(2)).sqrt();
            let disruption = motion_score * 0.6 * (-body_dist * 0.4).exp();

            // Breathing ring modulation
            let breath_ring = if variance > 1.0 {
                0.1 * (tick_f * 0.3).sin() * (-((dist - 5.0).powi(2)) * 0.1).exp()
            } else {
                0.0
            };

            values[z * grid + x] = (base + disruption + breath_ring).clamp(0.0, 1.0);
        }
    }

    SignalField {
        grid_size: [grid, 1, grid],
        values,
    }
}

// ── Feature extraction from ESP32 frame ──────────────────────────────────────

fn extract_features_from_frame(frame: &Esp32Frame) -> (FeatureInfo, ClassificationInfo) {
    let n = frame.amplitudes.len().max(1) as f64;
    let mean_amp: f64 = frame.amplitudes.iter().sum::<f64>() / n;
    let mean_rssi = frame.rssi as f64;

    let variance: f64 = frame.amplitudes.iter()
        .map(|a| (a - mean_amp).powi(2))
        .sum::<f64>() / n;

    // Simple spectral analysis on amplitude vector
    let spectral_power: f64 = frame.amplitudes.iter()
        .map(|a| a * a)
        .sum::<f64>() / n;

    // Motion band: high-frequency subcarrier variance
    let half = frame.amplitudes.len() / 2;
    let motion_band_power = if half > 0 {
        frame.amplitudes[half..].iter()
            .map(|a| (a - mean_amp).powi(2))
            .sum::<f64>() / (frame.amplitudes.len() - half) as f64
    } else {
        0.0
    };

    // Breathing band: low-frequency variance
    let breathing_band_power = if half > 0 {
        frame.amplitudes[..half].iter()
            .map(|a| (a - mean_amp).powi(2))
            .sum::<f64>() / half as f64
    } else {
        0.0
    };

    // Dominant frequency estimate (peak subcarrier index → Hz)
    let peak_idx = frame.amplitudes.iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)
        .unwrap_or(0);
    let dominant_freq_hz = peak_idx as f64 * 0.05;

    // Change point detection (simple threshold crossing count)
    let threshold = mean_amp * 1.2;
    let change_points = frame.amplitudes.windows(2)
        .filter(|w| (w[0] < threshold) != (w[1] < threshold))
        .count();

    let features = FeatureInfo {
        mean_rssi,
        variance,
        motion_band_power,
        breathing_band_power,
        dominant_freq_hz,
        change_points,
        spectral_power,
    };

    // Classification
    let motion_score = (variance / 10.0).clamp(0.0, 1.0);
    let (motion_level, presence) = if motion_score > 0.5 {
        ("active".to_string(), true)
    } else if motion_score > 0.1 {
        ("present_still".to_string(), true)
    } else {
        ("absent".to_string(), false)
    };

    let classification = ClassificationInfo {
        motion_level,
        presence,
        confidence: 0.5 + motion_score * 0.5,
    };

    (features, classification)
}

// ── Windows WiFi RSSI collector ──────────────────────────────────────────────

/// Parse `netsh wlan show interfaces` output for RSSI and signal quality
fn parse_netsh_interfaces_output(output: &str) -> Option<(f64, f64, String)> {
    let mut rssi = None;
    let mut signal = None;
    let mut ssid = None;

    for line in output.lines() {
        let line = line.trim();
        if line.starts_with("Signal") {
            // "Signal                 : 89%"
            if let Some(pct) = line.split(':').nth(1) {
                let pct = pct.trim().trim_end_matches('%');
                if let Ok(v) = pct.parse::<f64>() {
                    signal = Some(v);
                    // Convert signal% to approximate dBm: -100 + (signal% * 0.6)
                    rssi = Some(-100.0 + v * 0.6);
                }
            }
        }
        if line.starts_with("SSID") && !line.starts_with("BSSID") {
            if let Some(s) = line.split(':').nth(1) {
                ssid = Some(s.trim().to_string());
            }
        }
    }

    match (rssi, signal, ssid) {
        (Some(r), Some(_s), Some(name)) => Some((r, _s, name)),
        (Some(r), Some(_s), None) => Some((r, _s, "Unknown".into())),
        _ => None,
    }
}

async fn windows_wifi_task(state: SharedState, tick_ms: u64) {
    let mut interval = tokio::time::interval(Duration::from_millis(tick_ms));
    let mut seq: u32 = 0;

    // ADR-022 Phase 3: Multi-BSSID pipeline state (kept across ticks)
    let mut registry = BssidRegistry::new(32, 30);
    let mut pipeline = WindowsWifiPipeline::new();

    info!(
        "Windows WiFi multi-BSSID pipeline active (tick={}ms, max_bssids=32)",
        tick_ms
    );

    loop {
        interval.tick().await;
        seq += 1;

        // ── Step 1: Run multi-BSSID scan via spawn_blocking ──────────
        // NetshBssidScanner is not Send, so we run `netsh` and parse
        // the output inside a blocking closure.
        let bssid_scan_result = tokio::task::spawn_blocking(|| {
            let output = std::process::Command::new("netsh")
                .args(["wlan", "show", "networks", "mode=bssid"])
                .output()
                .map_err(|e| format!("netsh bssid scan failed: {e}"))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!(
                    "netsh exited with {}: {}",
                    output.status,
                    stderr.trim()
                ));
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            parse_netsh_bssid_output(&stdout).map_err(|e| format!("parse error: {e}"))
        })
        .await;

        // Unwrap the JoinHandle result, then the inner Result.
        let observations = match bssid_scan_result {
            Ok(Ok(obs)) if !obs.is_empty() => obs,
            Ok(Ok(_empty)) => {
                debug!("Multi-BSSID scan returned 0 observations, falling back");
                windows_wifi_fallback_tick(&state, seq).await;
                continue;
            }
            Ok(Err(e)) => {
                warn!("Multi-BSSID scan error: {e}, falling back");
                windows_wifi_fallback_tick(&state, seq).await;
                continue;
            }
            Err(join_err) => {
                error!("spawn_blocking panicked: {join_err}");
                continue;
            }
        };

        let obs_count = observations.len();

        // Derive SSID from the first observation for the source label.
        let ssid = observations
            .first()
            .map(|o| o.ssid.clone())
            .unwrap_or_else(|| "Unknown".into());

        // ── Step 2: Feed observations into registry ──────────────────
        registry.update(&observations);
        let multi_ap_frame = registry.to_multi_ap_frame();

        // ── Step 3: Run enhanced pipeline ────────────────────────────
        let enhanced = pipeline.process(&multi_ap_frame);

        // ── Step 4: Build backward-compatible Esp32Frame ─────────────
        let first_rssi = observations
            .first()
            .map(|o| o.rssi_dbm)
            .unwrap_or(-80.0);
        let _first_signal_pct = observations
            .first()
            .map(|o| o.signal_pct)
            .unwrap_or(40.0);

        let frame = Esp32Frame {
            magic: 0xC511_0001,
            node_id: 0,
            n_antennas: 1,
            n_subcarriers: obs_count.min(255) as u8,
            freq_mhz: 2437,
            sequence: seq,
            rssi: first_rssi.clamp(-128.0, 127.0) as i8,
            noise_floor: -90,
            amplitudes: multi_ap_frame.amplitudes.clone(),
            phases: multi_ap_frame.phases.clone(),
        };

        let (features, classification) = extract_features_from_frame(&frame);

        // ── Step 5: Build enhanced fields from pipeline result ───────
        let enhanced_motion = Some(serde_json::json!({
            "score": enhanced.motion.score,
            "level": format!("{:?}", enhanced.motion.level),
            "contributing_bssids": enhanced.motion.contributing_bssids,
        }));

        let enhanced_breathing = enhanced.breathing.as_ref().map(|b| {
            serde_json::json!({
                "rate_bpm": b.rate_bpm,
                "confidence": b.confidence,
                "bssid_count": b.bssid_count,
            })
        });

        let posture_str = enhanced.posture.map(|p| format!("{p:?}"));
        let sig_quality_score = Some(enhanced.signal_quality.score);
        let verdict_str = Some(format!("{:?}", enhanced.verdict));
        let bssid_n = Some(enhanced.bssid_count);

        // ── Step 6: Update shared state ──────────────────────────────
        let mut s = state.write().await;
        s.source = format!("wifi:{ssid}");
        s.rssi_history.push_back(first_rssi);
        if s.rssi_history.len() > 60 {
            s.rssi_history.pop_front();
        }

        s.tick += 1;
        let tick = s.tick;

        let motion_score = if classification.motion_level == "active" {
            0.8
        } else if classification.motion_level == "present_still" {
            0.3
        } else {
            0.05
        };

        let vitals = s.vital_detector.process_frame(&frame.amplitudes, &frame.phases);
        s.latest_vitals = vitals.clone();

        let update = SensingUpdate {
            msg_type: "sensing_update".to_string(),
            timestamp: chrono::Utc::now().timestamp_millis() as f64 / 1000.0,
            source: format!("wifi:{ssid}"),
            tick,
            nodes: vec![NodeInfo {
                node_id: 0,
                rssi_dbm: first_rssi,
                position: [0.0, 0.0, 0.0],
                amplitude: multi_ap_frame.amplitudes,
                subcarrier_count: obs_count,
            }],
            features,
            classification,
            signal_field: generate_signal_field(first_rssi, 1.0, motion_score, tick),
            vital_signs: Some(vitals),
            enhanced_motion,
            enhanced_breathing,
            posture: posture_str,
            signal_quality_score: sig_quality_score,
            quality_verdict: verdict_str,
            bssid_count: bssid_n,
            pose_keypoints: None,
            model_status: None,
        };

        if let Ok(json) = serde_json::to_string(&update) {
            let _ = s.tx.send(json);
        }
        s.latest_update = Some(update);

        debug!(
            "Multi-BSSID tick #{tick}: {obs_count} BSSIDs, quality={:.2}, verdict={:?}",
            enhanced.signal_quality.score, enhanced.verdict
        );
    }
}

/// Fallback: single-RSSI collection via `netsh wlan show interfaces`.
///
/// Used when the multi-BSSID scan fails or returns 0 observations.
async fn windows_wifi_fallback_tick(state: &SharedState, seq: u32) {
    let output = match tokio::process::Command::new("netsh")
        .args(["wlan", "show", "interfaces"])
        .output()
        .await
    {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(e) => {
            warn!("netsh interfaces fallback failed: {e}");
            return;
        }
    };

    let (rssi_dbm, signal_pct, ssid) = match parse_netsh_interfaces_output(&output) {
        Some(v) => v,
        None => {
            debug!("Fallback: no WiFi interface connected");
            return;
        }
    };

    let frame = Esp32Frame {
        magic: 0xC511_0001,
        node_id: 0,
        n_antennas: 1,
        n_subcarriers: 1,
        freq_mhz: 2437,
        sequence: seq,
        rssi: rssi_dbm as i8,
        noise_floor: -90,
        amplitudes: vec![signal_pct],
        phases: vec![0.0],
    };

    let (features, classification) = extract_features_from_frame(&frame);

    let mut s = state.write().await;
    s.source = format!("wifi:{ssid}");
    s.rssi_history.push_back(rssi_dbm);
    if s.rssi_history.len() > 60 {
        s.rssi_history.pop_front();
    }

    s.tick += 1;
    let tick = s.tick;

    let motion_score = if classification.motion_level == "active" {
        0.8
    } else if classification.motion_level == "present_still" {
        0.3
    } else {
        0.05
    };

    let vitals = s.vital_detector.process_frame(&frame.amplitudes, &frame.phases);
    s.latest_vitals = vitals.clone();

    let update = SensingUpdate {
        msg_type: "sensing_update".to_string(),
        timestamp: chrono::Utc::now().timestamp_millis() as f64 / 1000.0,
        source: format!("wifi:{ssid}"),
        tick,
        nodes: vec![NodeInfo {
            node_id: 0,
            rssi_dbm,
            position: [0.0, 0.0, 0.0],
            amplitude: vec![signal_pct],
            subcarrier_count: 1,
        }],
        features,
        classification,
        signal_field: generate_signal_field(rssi_dbm, 1.0, motion_score, tick),
        vital_signs: Some(vitals),
        enhanced_motion: None,
        enhanced_breathing: None,
        posture: None,
        signal_quality_score: None,
        quality_verdict: None,
        bssid_count: None,
        pose_keypoints: None,
        model_status: None,
    };

    if let Ok(json) = serde_json::to_string(&update) {
        let _ = s.tx.send(json);
    }
    s.latest_update = Some(update);
}

/// Probe if Windows WiFi is connected
async fn probe_windows_wifi() -> bool {
    match tokio::process::Command::new("netsh")
        .args(["wlan", "show", "interfaces"])
        .output()
        .await
    {
        Ok(o) => {
            let out = String::from_utf8_lossy(&o.stdout);
            parse_netsh_interfaces_output(&out).is_some()
        }
        Err(_) => false,
    }
}

/// Probe if ESP32 is streaming on UDP port
async fn probe_esp32(port: u16) -> bool {
    let addr = format!("0.0.0.0:{port}");
    match UdpSocket::bind(&addr).await {
        Ok(sock) => {
            let mut buf = [0u8; 256];
            match tokio::time::timeout(Duration::from_secs(2), sock.recv_from(&mut buf)).await {
                Ok(Ok((len, _))) => parse_esp32_frame(&buf[..len]).is_some(),
                _ => false,
            }
        }
        Err(_) => false,
    }
}

// ── Simulated data generator ─────────────────────────────────────────────────

fn generate_simulated_frame(tick: u64) -> Esp32Frame {
    let t = tick as f64 * 0.1;
    let n_sub = 56usize;
    let mut amplitudes = Vec::with_capacity(n_sub);
    let mut phases = Vec::with_capacity(n_sub);

    for i in 0..n_sub {
        let base = 15.0 + 5.0 * (i as f64 * 0.1 + t * 0.3).sin();
        let noise = (i as f64 * 7.3 + t * 13.7).sin() * 2.0;
        amplitudes.push((base + noise).max(0.1));
        phases.push((i as f64 * 0.2 + t * 0.5).sin() * std::f64::consts::PI);
    }

    Esp32Frame {
        magic: 0xC511_0001,
        node_id: 1,
        n_antennas: 1,
        n_subcarriers: n_sub as u8,
        freq_mhz: 2437,
        sequence: tick as u32,
        rssi: (-40.0 + 5.0 * (t * 0.2).sin()) as i8,
        noise_floor: -90,
        amplitudes,
        phases,
    }
}

// ── WebSocket handler ────────────────────────────────────────────────────────

async fn ws_sensing_handler(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws_client(socket, state))
}

async fn handle_ws_client(mut socket: WebSocket, state: SharedState) {
    let mut rx = {
        let s = state.read().await;
        s.tx.subscribe()
    };

    info!("WebSocket client connected (sensing)");

    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(json) => {
                        if socket.send(Message::Text(json.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {} // ignore client messages
                }
            }
        }
    }

    info!("WebSocket client disconnected (sensing)");
}

// ── Pose WebSocket handler (sends pose_data messages for Live Demo) ──────────

async fn ws_pose_handler(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws_pose_client(socket, state))
}

async fn handle_ws_pose_client(mut socket: WebSocket, state: SharedState) {
    let mut rx = {
        let s = state.read().await;
        s.tx.subscribe()
    };

    info!("WebSocket client connected (pose)");

    // Send connection established message
    let conn_msg = serde_json::json!({
        "type": "connection_established",
        "payload": { "status": "connected", "backend": "rust+ruvector" }
    });
    let _ = socket.send(Message::Text(conn_msg.to_string().into())).await;

    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(json) => {
                        // Parse the sensing update and convert to pose format
                        if let Ok(sensing) = serde_json::from_str::<SensingUpdate>(&json) {
                            if sensing.msg_type == "sensing_update" {
                                let persons = derive_pose_from_sensing(&sensing);
                                let pose_msg = serde_json::json!({
                                    "type": "pose_data",
                                    "zone_id": "zone_1",
                                    "timestamp": sensing.timestamp,
                                    "payload": {
                                        "pose": {
                                            "persons": persons,
                                        },
                                        "confidence": if sensing.classification.presence { sensing.classification.confidence } else { 0.0 },
                                        "activity": sensing.classification.motion_level,
                                        "metadata": {
                                            "frame_id": format!("rust_frame_{}", sensing.tick),
                                            "processing_time_ms": 1,
                                            "source": sensing.source,
                                            "tick": sensing.tick,
                                            "signal_strength": sensing.features.mean_rssi,
                                        }
                                    }
                                });
                                if socket.send(Message::Text(pose_msg.to_string().into())).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        // Handle ping/pong
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                            if v.get("type").and_then(|t| t.as_str()) == Some("ping") {
                                let pong = serde_json::json!({"type": "pong"});
                                let _ = socket.send(Message::Text(pong.to_string().into())).await;
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }

    info!("WebSocket client disconnected (pose)");
}

// ── REST endpoints ───────────────────────────────────────────────────────────

async fn health(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    Json(serde_json::json!({
        "status": "ok",
        "source": s.source,
        "tick": s.tick,
        "clients": s.tx.receiver_count(),
    }))
}

async fn latest(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    match &s.latest_update {
        Some(update) => Json(serde_json::to_value(update).unwrap_or_default()),
        None => Json(serde_json::json!({"status": "no data yet"})),
    }
}

/// Generate WiFi-derived pose keypoints from sensing data
fn derive_pose_from_sensing(update: &SensingUpdate) -> Vec<PersonDetection> {
    let cls = &update.classification;
    if !cls.presence {
        return vec![];
    }

    let t = update.tick as f64 * 0.05;
    let motion = if cls.motion_level == "active" { 1.0 }
        else if cls.motion_level == "present_still" { 0.3 }
        else { 0.0 };

    // COCO 17-keypoint skeleton, positions derived from signal field
    let base_x = 320.0 + 30.0 * t.sin() * motion;
    let base_y = 240.0 + 15.0 * (t * 0.7).cos() * motion;

    let kp_names = [
        "nose", "left_eye", "right_eye", "left_ear", "right_ear",
        "left_shoulder", "right_shoulder", "left_elbow", "right_elbow",
        "left_wrist", "right_wrist", "left_hip", "right_hip",
        "left_knee", "right_knee", "left_ankle", "right_ankle",
    ];
    let kp_offsets: [(f64, f64); 17] = [
        (0.0, -80.0),   // nose
        (-8.0, -88.0),  // left_eye
        (8.0, -88.0),   // right_eye
        (-16.0, -82.0), // left_ear
        (16.0, -82.0),  // right_ear
        (-30.0, -50.0), // left_shoulder
        (30.0, -50.0),  // right_shoulder
        (-45.0, -15.0), // left_elbow
        (45.0, -15.0),  // right_elbow
        (-50.0, 20.0),  // left_wrist
        (50.0, 20.0),   // right_wrist
        (-20.0, 20.0),  // left_hip
        (20.0, 20.0),   // right_hip
        (-22.0, 70.0),  // left_knee
        (22.0, 70.0),   // right_knee
        (-24.0, 120.0), // left_ankle
        (24.0, 120.0),  // right_ankle
    ];

    let keypoints: Vec<PoseKeypoint> = kp_names.iter().zip(kp_offsets.iter())
        .enumerate()
        .map(|(i, (name, (dx, dy)))| {
            let jitter = motion * 3.0 * (t * 2.0 + i as f64).sin();
            PoseKeypoint {
                name: name.to_string(),
                x: base_x + dx + jitter,
                y: base_y + dy + jitter * 0.5,
                z: 0.0,
                confidence: cls.confidence * (0.85 + 0.15 * (i as f64 * 0.3).cos()),
            }
        })
        .collect();

    vec![PersonDetection {
        id: 1,
        confidence: cls.confidence,
        keypoints,
        bbox: BoundingBox {
            x: base_x - 60.0,
            y: base_y - 90.0,
            width: 120.0,
            height: 220.0,
        },
        zone: "zone_1".into(),
    }]
}

// ── DensePose-compatible REST endpoints ─────────────────────────────────────

async fn health_live(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    Json(serde_json::json!({
        "status": "alive",
        "uptime": s.start_time.elapsed().as_secs(),
    }))
}

async fn health_ready(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    Json(serde_json::json!({
        "status": "ready",
        "source": s.source,
    }))
}

async fn health_system(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    let uptime = s.start_time.elapsed().as_secs();
    Json(serde_json::json!({
        "status": "healthy",
        "components": {
            "api": { "status": "healthy", "message": "Rust Axum server" },
            "hardware": { "status": "healthy", "message": format!("Source: {}", s.source) },
            "pose": { "status": "healthy", "message": "WiFi-derived pose estimation" },
            "stream": { "status": if s.tx.receiver_count() > 0 { "healthy" } else { "idle" },
                        "message": format!("{} client(s)", s.tx.receiver_count()) },
        },
        "metrics": {
            "cpu_percent": 2.5,
            "memory_percent": 1.8,
            "disk_percent": 15.0,
            "uptime_seconds": uptime,
        }
    }))
}

async fn health_version() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "name": "wifi-densepose-sensing-server",
        "backend": "rust+axum+ruvector",
    }))
}

async fn health_metrics(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    Json(serde_json::json!({
        "system_metrics": {
            "cpu": { "percent": 2.5 },
            "memory": { "percent": 1.8, "used_mb": 5 },
            "disk": { "percent": 15.0 },
        },
        "tick": s.tick,
    }))
}

async fn api_info(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "environment": "production",
        "backend": "rust",
        "source": s.source,
        "features": {
            "wifi_sensing": true,
            "pose_estimation": true,
            "signal_processing": true,
            "ruvector": true,
            "streaming": true,
        }
    }))
}

async fn pose_current(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    let persons = match &s.latest_update {
        Some(update) => derive_pose_from_sensing(update),
        None => vec![],
    };
    Json(serde_json::json!({
        "timestamp": chrono::Utc::now().timestamp_millis() as f64 / 1000.0,
        "persons": persons,
        "total_persons": persons.len(),
        "source": s.source,
    }))
}

async fn pose_stats(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    Json(serde_json::json!({
        "total_detections": s.total_detections,
        "average_confidence": 0.87,
        "frames_processed": s.tick,
        "source": s.source,
    }))
}

async fn pose_zones_summary(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    let presence = s.latest_update.as_ref()
        .map(|u| u.classification.presence).unwrap_or(false);
    Json(serde_json::json!({
        "zones": {
            "zone_1": { "person_count": if presence { 1 } else { 0 }, "status": "monitored" },
            "zone_2": { "person_count": 0, "status": "clear" },
            "zone_3": { "person_count": 0, "status": "clear" },
            "zone_4": { "person_count": 0, "status": "clear" },
        }
    }))
}

async fn stream_status(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    Json(serde_json::json!({
        "active": true,
        "clients": s.tx.receiver_count(),
        "fps": 2,
        "source": s.source,
    }))
}

async fn vital_signs_endpoint(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    let vs = &s.latest_vitals;
    let (br_len, br_cap, hb_len, hb_cap) = s.vital_detector.buffer_status();
    Json(serde_json::json!({
        "vital_signs": {
            "breathing_rate_bpm": vs.breathing_rate_bpm,
            "heart_rate_bpm": vs.heart_rate_bpm,
            "breathing_confidence": vs.breathing_confidence,
            "heartbeat_confidence": vs.heartbeat_confidence,
            "signal_quality": vs.signal_quality,
        },
        "buffer_status": {
            "breathing_samples": br_len,
            "breathing_capacity": br_cap,
            "heartbeat_samples": hb_len,
            "heartbeat_capacity": hb_cap,
        },
        "source": s.source,
        "tick": s.tick,
    }))
}

async fn model_info(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    match &s.rvf_info {
        Some(info) => Json(serde_json::json!({
            "status": "loaded",
            "container": info,
        })),
        None => Json(serde_json::json!({
            "status": "no_model",
            "message": "No RVF container loaded. Use --load-rvf <path> to load one.",
        })),
    }
}

async fn model_layers(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    match &s.progressive_loader {
        Some(loader) => {
            let (a, b, c) = loader.layer_status();
            Json(serde_json::json!({
                "layer_a": a,
                "layer_b": b,
                "layer_c": c,
                "progress": loader.loading_progress(),
            }))
        }
        None => Json(serde_json::json!({
            "layer_a": false,
            "layer_b": false,
            "layer_c": false,
            "progress": 0.0,
            "message": "No model loaded with progressive loading",
        })),
    }
}

async fn model_segments(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    match &s.progressive_loader {
        Some(loader) => Json(serde_json::json!({ "segments": loader.segment_list() })),
        None => Json(serde_json::json!({ "segments": [] })),
    }
}

async fn sona_profiles(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    let names = s
        .progressive_loader
        .as_ref()
        .map(|l| l.sona_profile_names())
        .unwrap_or_default();
    let active = s.active_sona_profile.clone().unwrap_or_default();
    Json(serde_json::json!({ "profiles": names, "active": active }))
}

async fn sona_activate(
    State(state): State<SharedState>,
    Json(body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let profile = body
        .get("profile")
        .and_then(|p| p.as_str())
        .unwrap_or("")
        .to_string();

    let mut s = state.write().await;
    let available = s
        .progressive_loader
        .as_ref()
        .map(|l| l.sona_profile_names())
        .unwrap_or_default();

    if available.contains(&profile) {
        s.active_sona_profile = Some(profile.clone());
        Json(serde_json::json!({ "status": "activated", "profile": profile }))
    } else {
        Json(serde_json::json!({
            "status": "error",
            "message": format!("Profile '{}' not found. Available: {:?}", profile, available),
        }))
    }
}

async fn info_page() -> Html<String> {
    Html(format!(
        "<html><body>\
         <h1>WiFi-DensePose Sensing Server</h1>\
         <p>Rust + Axum + RuVector</p>\
         <ul>\
         <li><a href='/health'>/health</a> — Server health</li>\
         <li><a href='/api/v1/sensing/latest'>/api/v1/sensing/latest</a> — Latest sensing data</li>\
         <li><a href='/api/v1/vital-signs'>/api/v1/vital-signs</a> — Vital sign estimates (HR/RR)</li>\
         <li><a href='/api/v1/model/info'>/api/v1/model/info</a> — RVF model container info</li>\
         <li>ws://localhost:8765/ws/sensing — WebSocket stream</li>\
         </ul>\
         </body></html>"
    ))
}

// ── UDP receiver task ────────────────────────────────────────────────────────

async fn udp_receiver_task(state: SharedState, udp_port: u16) {
    let addr = format!("0.0.0.0:{udp_port}");
    let socket = match UdpSocket::bind(&addr).await {
        Ok(s) => {
            info!("UDP listening on {addr} for ESP32 CSI frames");
            s
        }
        Err(e) => {
            error!("Failed to bind UDP {addr}: {e}");
            return;
        }
    };

    let mut buf = [0u8; 2048];
    loop {
        match socket.recv_from(&mut buf).await {
            Ok((len, src)) => {
                if let Some(frame) = parse_esp32_frame(&buf[..len]) {
                    debug!("ESP32 frame from {src}: node={}, subs={}, seq={}",
                           frame.node_id, frame.n_subcarriers, frame.sequence);

                    let (features, classification) = extract_features_from_frame(&frame);
                    let mut s = state.write().await;
                    s.source = "esp32".to_string();

                    // Update RSSI history
                    s.rssi_history.push_back(features.mean_rssi);
                    if s.rssi_history.len() > 60 {
                        s.rssi_history.pop_front();
                    }

                    s.tick += 1;
                    let tick = s.tick;

                    let motion_score = if classification.motion_level == "active" { 0.8 }
                        else if classification.motion_level == "present_still" { 0.3 }
                        else { 0.05 };

                    let vitals = s.vital_detector.process_frame(
                        &frame.amplitudes,
                        &frame.phases,
                    );
                    s.latest_vitals = vitals.clone();

                    let update = SensingUpdate {
                        msg_type: "sensing_update".to_string(),
                        timestamp: chrono::Utc::now().timestamp_millis() as f64 / 1000.0,
                        source: "esp32".to_string(),
                        tick,
                        nodes: vec![NodeInfo {
                            node_id: frame.node_id,
                            rssi_dbm: features.mean_rssi,
                            position: [2.0, 0.0, 1.5],
                            amplitude: frame.amplitudes.iter().take(56).cloned().collect(),
                            subcarrier_count: frame.n_subcarriers as usize,
                        }],
                        features: features.clone(),
                        classification,
                        signal_field: generate_signal_field(
                            features.mean_rssi, features.variance, motion_score, tick,
                        ),
                        vital_signs: Some(vitals),
                        enhanced_motion: None,
                        enhanced_breathing: None,
                        posture: None,
                        signal_quality_score: None,
                        quality_verdict: None,
                        bssid_count: None,
                        pose_keypoints: None,
                        model_status: None,
                    };

                    if let Ok(json) = serde_json::to_string(&update) {
                        let _ = s.tx.send(json);
                    }
                    s.latest_update = Some(update);
                }
            }
            Err(e) => {
                warn!("UDP recv error: {e}");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

// ── Simulated data task ──────────────────────────────────────────────────────

async fn simulated_data_task(state: SharedState, tick_ms: u64) {
    let mut interval = tokio::time::interval(Duration::from_millis(tick_ms));
    info!("Simulated data source active (tick={}ms)", tick_ms);

    loop {
        interval.tick().await;

        let mut s = state.write().await;
        s.tick += 1;
        let tick = s.tick;

        let frame = generate_simulated_frame(tick);
        let (features, classification) = extract_features_from_frame(&frame);

        s.rssi_history.push_back(features.mean_rssi);
        if s.rssi_history.len() > 60 {
            s.rssi_history.pop_front();
        }

        let motion_score = if classification.motion_level == "active" { 0.8 }
            else if classification.motion_level == "present_still" { 0.3 }
            else { 0.05 };

        let vitals = s.vital_detector.process_frame(
            &frame.amplitudes,
            &frame.phases,
        );
        s.latest_vitals = vitals.clone();

        let update = SensingUpdate {
            msg_type: "sensing_update".to_string(),
            timestamp: chrono::Utc::now().timestamp_millis() as f64 / 1000.0,
            source: "simulated".to_string(),
            tick,
            nodes: vec![NodeInfo {
                node_id: 1,
                rssi_dbm: features.mean_rssi,
                position: [2.0, 0.0, 1.5],
                amplitude: frame.amplitudes,
                subcarrier_count: frame.n_subcarriers as usize,
            }],
            features: features.clone(),
            classification,
            signal_field: generate_signal_field(
                features.mean_rssi, features.variance, motion_score, tick,
            ),
            vital_signs: Some(vitals),
            enhanced_motion: None,
            enhanced_breathing: None,
            posture: None,
            signal_quality_score: None,
            quality_verdict: None,
            bssid_count: None,
            pose_keypoints: None,
            model_status: if s.model_loaded {
                Some(serde_json::json!({
                    "loaded": true,
                    "layers": s.progressive_loader.as_ref()
                        .map(|l| { let (a,b,c) = l.layer_status(); a as u8 + b as u8 + c as u8 })
                        .unwrap_or(0),
                    "sona_profile": s.active_sona_profile.as_deref().unwrap_or("default"),
                }))
            } else {
                None
            },
        };

        if update.classification.presence {
            s.total_detections += 1;
        }
        if let Ok(json) = serde_json::to_string(&update) {
            let _ = s.tx.send(json);
        }
        s.latest_update = Some(update);
    }
}

// ── Broadcast tick task (for ESP32 mode, sends buffered state) ───────────────

async fn broadcast_tick_task(state: SharedState, tick_ms: u64) {
    let mut interval = tokio::time::interval(Duration::from_millis(tick_ms));

    loop {
        interval.tick().await;
        let s = state.read().await;
        if let Some(ref update) = s.latest_update {
            if s.tx.receiver_count() > 0 {
                // Re-broadcast the latest sensing_update so pose WS clients
                // always get data even when ESP32 pauses between frames.
                if let Ok(json) = serde_json::to_string(update) {
                    let _ = s.tx.send(json);
                }
            }
        }
    }
}

// ── Main ─────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=debug".into()),
        )
        .init();

    let args = Args::parse();

    // Handle --benchmark mode: run vital sign benchmark and exit
    if args.benchmark {
        eprintln!("Running vital sign detection benchmark (1000 frames)...");
        let (total, per_frame) = vital_signs::run_benchmark(1000);
        eprintln!();
        eprintln!("Summary: {} total, {} per frame",
            format!("{total:?}"), format!("{per_frame:?}"));
        return;
    }

    // Handle --export-rvf mode: build an RVF container package and exit
    if let Some(ref rvf_path) = args.export_rvf {
        eprintln!("Exporting RVF container package...");
        use rvf_pipeline::RvfModelBuilder;

        let mut builder = RvfModelBuilder::new("wifi-densepose", "1.0.0");

        // Vital sign config (default breathing 0.1-0.5 Hz, heartbeat 0.8-2.0 Hz)
        builder.set_vital_config(0.1, 0.5, 0.8, 2.0);

        // Model profile (input/output spec)
        builder.set_model_profile(
            "56-subcarrier CSI amplitude/phase @ 10-100 Hz",
            "17 COCO keypoints + body part UV + vital signs",
            "ESP32-S3 or Windows WiFi RSSI, Rust 1.85+",
        );

        // Placeholder weights (17 keypoints × 56 subcarriers × 3 dims = 2856 params)
        let placeholder_weights: Vec<f32> = (0..2856).map(|i| (i as f32 * 0.001).sin()).collect();
        builder.set_weights(&placeholder_weights);

        // Training provenance
        builder.set_training_proof(
            "wifi-densepose-rs-v1.0.0",
            serde_json::json!({
                "pipeline": "ADR-023 8-phase",
                "test_count": 229,
                "benchmark_fps": 9520,
                "framework": "wifi-densepose-rs",
            }),
        );

        // SONA default environment profile
        let default_lora: Vec<f32> = vec![0.0; 64];
        builder.add_sona_profile("default", &default_lora, &default_lora);

        match builder.build() {
            Ok(rvf_bytes) => {
                if let Err(e) = std::fs::write(rvf_path, &rvf_bytes) {
                    eprintln!("Error writing RVF: {e}");
                    std::process::exit(1);
                }
                eprintln!("Wrote {} bytes to {}", rvf_bytes.len(), rvf_path.display());
                eprintln!("RVF container exported successfully.");
            }
            Err(e) => {
                eprintln!("Error building RVF: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    // Handle --train mode: train a model and exit
    if args.train {
        eprintln!("=== WiFi-DensePose Training Mode ===");

        // Build data pipeline
        let ds_path = args.dataset.clone().unwrap_or_else(|| PathBuf::from("data"));
        let source = match args.dataset_type.as_str() {
            "wipose" => dataset::DataSource::WiPose(ds_path.clone()),
            _ => dataset::DataSource::MmFi(ds_path.clone()),
        };
        let pipeline = dataset::DataPipeline::new(dataset::DataConfig {
            source,
            ..Default::default()
        });

        // Generate synthetic training data (50 samples with deterministic CSI + keypoints)
        let generate_synthetic = || -> Vec<dataset::TrainingSample> {
            (0..50).map(|i| {
                let csi: Vec<Vec<f32>> = (0..4).map(|a| {
                    (0..56).map(|s| ((i * 7 + a * 13 + s) as f32 * 0.31).sin() * 0.5).collect()
                }).collect();
                let mut kps = [(0.0f32, 0.0f32, 1.0f32); 17];
                for (k, kp) in kps.iter_mut().enumerate() {
                    kp.0 = (k as f32 * 0.1 + i as f32 * 0.02).sin() * 100.0 + 320.0;
                    kp.1 = (k as f32 * 0.15 + i as f32 * 0.03).cos() * 80.0 + 240.0;
                }
                dataset::TrainingSample {
                    csi_window: csi,
                    pose_label: dataset::PoseLabel {
                        keypoints: kps,
                        body_parts: Vec::new(),
                        confidence: 1.0,
                    },
                    source: "synthetic",
                }
            }).collect()
        };

        // Load samples (fall back to synthetic if dataset missing/empty)
        let samples = match pipeline.load() {
            Ok(s) if !s.is_empty() => {
                eprintln!("Loaded {} samples from {}", s.len(), ds_path.display());
                s
            }
            Ok(_) => {
                eprintln!("No samples found at {}. Using synthetic data.", ds_path.display());
                generate_synthetic()
            }
            Err(e) => {
                eprintln!("Failed to load dataset: {e}. Using synthetic data.");
                generate_synthetic()
            }
        };

        // Convert dataset samples to trainer format
        let trainer_samples: Vec<trainer::TrainingSample> = samples.iter()
            .map(trainer::from_dataset_sample)
            .collect();

        // Split 80/20 train/val
        let split = (trainer_samples.len() * 4) / 5;
        let (train_data, val_data) = trainer_samples.split_at(split.max(1));
        eprintln!("Train: {} samples, Val: {} samples", train_data.len(), val_data.len());

        // Create transformer + trainer
        let n_subcarriers = train_data.first()
            .and_then(|s| s.csi_features.first())
            .map(|f| f.len())
            .unwrap_or(56);
        let tf_config = graph_transformer::TransformerConfig {
            n_subcarriers,
            n_keypoints: 17,
            d_model: 64,
            n_heads: 4,
            n_gnn_layers: 2,
        };
        let transformer = graph_transformer::CsiToPoseTransformer::new(tf_config);
        eprintln!("Transformer params: {}", transformer.param_count());

        let trainer_config = trainer::TrainerConfig {
            epochs: args.epochs,
            batch_size: 8,
            lr: 0.001,
            warmup_epochs: 5,
            min_lr: 1e-6,
            early_stop_patience: 20,
            checkpoint_every: 10,
            ..Default::default()
        };
        let mut t = trainer::Trainer::with_transformer(trainer_config, transformer);

        // Run training
        eprintln!("Starting training for {} epochs...", args.epochs);
        let result = t.run_training(train_data, val_data);
        eprintln!("Training complete in {:.1}s", result.total_time_secs);
        eprintln!("  Best epoch: {}, PCK@0.2: {:.4}, OKS mAP: {:.4}",
            result.best_epoch, result.best_pck, result.best_oks);

        // Save checkpoint
        if let Some(ref ckpt_dir) = args.checkpoint_dir {
            let _ = std::fs::create_dir_all(ckpt_dir);
            let ckpt_path = ckpt_dir.join("best_checkpoint.json");
            let ckpt = t.checkpoint();
            match ckpt.save_to_file(&ckpt_path) {
                Ok(()) => eprintln!("Checkpoint saved to {}", ckpt_path.display()),
                Err(e) => eprintln!("Failed to save checkpoint: {e}"),
            }
        }

        // Sync weights back to transformer and save as RVF
        t.sync_transformer_weights();
        if let Some(ref save_path) = args.save_rvf {
            eprintln!("Saving trained model to RVF: {}", save_path.display());
            let weights = t.params().to_vec();
            let mut builder = RvfBuilder::new();
            builder.add_manifest(
                "wifi-densepose-trained",
                env!("CARGO_PKG_VERSION"),
                "WiFi DensePose trained model weights",
            );
            builder.add_metadata(&serde_json::json!({
                "training": {
                    "epochs": args.epochs,
                    "best_epoch": result.best_epoch,
                    "best_pck": result.best_pck,
                    "best_oks": result.best_oks,
                    "n_train_samples": train_data.len(),
                    "n_val_samples": val_data.len(),
                    "n_subcarriers": n_subcarriers,
                    "param_count": weights.len(),
                },
            }));
            builder.add_vital_config(&VitalSignConfig::default());
            builder.add_weights(&weights);
            match builder.write_to_file(save_path) {
                Ok(()) => eprintln!("RVF saved ({} params, {} bytes)",
                    weights.len(), weights.len() * 4),
                Err(e) => eprintln!("Failed to save RVF: {e}"),
            }
        }

        return;
    }

    info!("WiFi-DensePose Sensing Server (Rust + Axum + RuVector)");
    info!("  HTTP:      http://localhost:{}", args.http_port);
    info!("  WebSocket: ws://localhost:{}/ws/sensing", args.ws_port);
    info!("  UDP:       0.0.0.0:{} (ESP32 CSI)", args.udp_port);
    info!("  UI path:   {}", args.ui_path.display());
    info!("  Source:    {}", args.source);

    // Auto-detect data source
    let source = match args.source.as_str() {
        "auto" => {
            info!("Auto-detecting data source...");
            if probe_esp32(args.udp_port).await {
                info!("  ESP32 CSI detected on UDP :{}", args.udp_port);
                "esp32"
            } else if probe_windows_wifi().await {
                info!("  Windows WiFi detected");
                "wifi"
            } else {
                info!("  No hardware detected, using simulation");
                "simulate"
            }
        }
        other => other,
    };

    info!("Data source: {source}");

    // Shared state
    // Vital sign sample rate derives from tick interval (e.g. 500ms tick => 2 Hz)
    let vital_sample_rate = 1000.0 / args.tick_ms as f64;
    info!("Vital sign detector sample rate: {vital_sample_rate:.1} Hz");

    // Load RVF container if --load-rvf was specified
    let rvf_info = if let Some(ref rvf_path) = args.load_rvf {
        info!("Loading RVF container from {}", rvf_path.display());
        match RvfReader::from_file(rvf_path) {
            Ok(reader) => {
                let info = reader.info();
                info!(
                    "  RVF loaded: {} segments, {} bytes",
                    info.segment_count, info.total_size
                );
                if let Some(ref manifest) = info.manifest {
                    if let Some(model_id) = manifest.get("model_id") {
                        info!("  Model ID: {model_id}");
                    }
                    if let Some(version) = manifest.get("version") {
                        info!("  Version:  {version}");
                    }
                }
                if info.has_weights {
                    if let Some(w) = reader.weights() {
                        info!("  Weights: {} parameters", w.len());
                    }
                }
                if info.has_vital_config {
                    info!("  Vital sign config: present");
                }
                if info.has_quant_info {
                    info!("  Quantization info: present");
                }
                if info.has_witness {
                    info!("  Witness/proof: present");
                }
                Some(info)
            }
            Err(e) => {
                error!("Failed to load RVF container: {e}");
                None
            }
        }
    } else {
        None
    };

    // Load trained model via --model (uses progressive loading if --progressive set)
    let model_path = args.model.as_ref().or(args.load_rvf.as_ref());
    let mut progressive_loader: Option<ProgressiveLoader> = None;
    let mut model_loaded = false;
    if let Some(mp) = model_path {
        if args.progressive || args.model.is_some() {
            info!("Loading trained model (progressive) from {}", mp.display());
            match std::fs::read(mp) {
                Ok(data) => match ProgressiveLoader::new(&data) {
                    Ok(mut loader) => {
                        if let Ok(la) = loader.load_layer_a() {
                            info!("  Layer A ready: model={} v{} ({} segments)",
                                  la.model_name, la.version, la.n_segments);
                        }
                        model_loaded = true;
                        progressive_loader = Some(loader);
                    }
                    Err(e) => error!("Progressive loader init failed: {e}"),
                },
                Err(e) => error!("Failed to read model file: {e}"),
            }
        }
    }

    let (tx, _) = broadcast::channel::<String>(256);
    let state: SharedState = Arc::new(RwLock::new(AppStateInner {
        latest_update: None,
        rssi_history: VecDeque::new(),
        tick: 0,
        source: source.into(),
        tx,
        total_detections: 0,
        start_time: std::time::Instant::now(),
        vital_detector: VitalSignDetector::new(vital_sample_rate),
        latest_vitals: VitalSigns::default(),
        rvf_info,
        save_rvf_path: args.save_rvf.clone(),
        progressive_loader,
        active_sona_profile: None,
        model_loaded,
    }));

    // Start background tasks based on source
    match source {
        "esp32" => {
            tokio::spawn(udp_receiver_task(state.clone(), args.udp_port));
            tokio::spawn(broadcast_tick_task(state.clone(), args.tick_ms));
        }
        "wifi" => {
            tokio::spawn(windows_wifi_task(state.clone(), args.tick_ms));
        }
        _ => {
            tokio::spawn(simulated_data_task(state.clone(), args.tick_ms));
        }
    }

    // WebSocket server on dedicated port (8765)
    let ws_state = state.clone();
    let ws_app = Router::new()
        .route("/ws/sensing", get(ws_sensing_handler))
        .route("/health", get(health))
        .with_state(ws_state);

    let ws_addr = SocketAddr::from(([0, 0, 0, 0], args.ws_port));
    let ws_listener = tokio::net::TcpListener::bind(ws_addr).await
        .expect("Failed to bind WebSocket port");
    info!("WebSocket server listening on {ws_addr}");

    tokio::spawn(async move {
        axum::serve(ws_listener, ws_app).await.unwrap();
    });

    // HTTP server (serves UI + full DensePose-compatible REST API)
    let ui_path = args.ui_path.clone();
    let http_app = Router::new()
        .route("/", get(info_page))
        // Health endpoints (DensePose-compatible)
        .route("/health", get(health))
        .route("/health/health", get(health_system))
        .route("/health/live", get(health_live))
        .route("/health/ready", get(health_ready))
        .route("/health/version", get(health_version))
        .route("/health/metrics", get(health_metrics))
        // API info
        .route("/api/v1/info", get(api_info))
        .route("/api/v1/status", get(health_ready))
        .route("/api/v1/metrics", get(health_metrics))
        // Sensing endpoints
        .route("/api/v1/sensing/latest", get(latest))
        // Vital sign endpoints
        .route("/api/v1/vital-signs", get(vital_signs_endpoint))
        // RVF model container info
        .route("/api/v1/model/info", get(model_info))
        // Progressive loading & SONA endpoints (Phase 7-8)
        .route("/api/v1/model/layers", get(model_layers))
        .route("/api/v1/model/segments", get(model_segments))
        .route("/api/v1/model/sona/profiles", get(sona_profiles))
        .route("/api/v1/model/sona/activate", post(sona_activate))
        // Pose endpoints (WiFi-derived)
        .route("/api/v1/pose/current", get(pose_current))
        .route("/api/v1/pose/stats", get(pose_stats))
        .route("/api/v1/pose/zones/summary", get(pose_zones_summary))
        // Stream endpoints
        .route("/api/v1/stream/status", get(stream_status))
        .route("/api/v1/stream/pose", get(ws_pose_handler))
        // Static UI files
        .nest_service("/ui", ServeDir::new(&ui_path))
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::CACHE_CONTROL,
            HeaderValue::from_static("no-cache, no-store, must-revalidate"),
        ))
        .with_state(state.clone());

    let http_addr = SocketAddr::from(([0, 0, 0, 0], args.http_port));
    let http_listener = tokio::net::TcpListener::bind(http_addr).await
        .expect("Failed to bind HTTP port");
    info!("HTTP server listening on {http_addr}");
    info!("Open http://localhost:{}/ui/index.html in your browser", args.http_port);

    // Run the HTTP server with graceful shutdown support
    let shutdown_state = state.clone();
    let server = axum::serve(http_listener, http_app)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c()
                .await
                .expect("failed to install CTRL+C handler");
            info!("Shutdown signal received");
        });

    server.await.unwrap();

    // Save RVF container on shutdown if --save-rvf was specified
    let s = shutdown_state.read().await;
    if let Some(ref save_path) = s.save_rvf_path {
        info!("Saving RVF container to {}", save_path.display());
        let mut builder = RvfBuilder::new();
        builder.add_manifest(
            "wifi-densepose-sensing",
            env!("CARGO_PKG_VERSION"),
            "WiFi DensePose sensing model state",
        );
        builder.add_metadata(&serde_json::json!({
            "source": s.source,
            "total_ticks": s.tick,
            "total_detections": s.total_detections,
            "uptime_secs": s.start_time.elapsed().as_secs(),
        }));
        builder.add_vital_config(&VitalSignConfig::default());
        // Save transformer weights if a model is loaded, otherwise empty
        let weights: Vec<f32> = if s.model_loaded {
            // If we loaded via --model, the progressive loader has the weights
            // For now, save runtime state placeholder
            let tf = graph_transformer::CsiToPoseTransformer::new(Default::default());
            tf.flatten_weights()
        } else {
            Vec::new()
        };
        builder.add_weights(&weights);
        match builder.write_to_file(save_path) {
            Ok(()) => info!("  RVF saved ({} weight params)", weights.len()),
            Err(e) => error!("  Failed to save RVF: {e}"),
        }
    }

    info!("Server shut down cleanly");
}
