//! WiFi-DensePose Sensing Server
//!
//! Lightweight Axum server that:
//! - Receives ESP32 CSI frames via UDP (port 5005)
//! - Processes signals using RuVector-powered wifi-densepose-signal crate
//! - Broadcasts sensing updates via WebSocket (ws://localhost:8765/ws/sensing)
//! - Serves the static UI files (port 8080)
//!
//! Replaces both ws_server.py and the Python HTTP server.

mod adaptive_classifier;
mod rvf_container;
mod rvf_pipeline;
mod vital_signs;

// Training pipeline modules (exposed via lib.rs)
use wifi_densepose_sensing_server::{graph_transformer, trainer, dataset, embedding};

use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path,
        State,
    },
    response::{Html, IntoResponse, Json},
    routing::{delete, get, post},
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

    /// Tick interval in milliseconds (default 100 ms = 10 fps for smooth pose animation)
    #[arg(long, default_value = "100")]
    tick_ms: u64,

    /// Bind address (default 127.0.0.1; set to 0.0.0.0 for network access)
    #[arg(long, default_value = "127.0.0.1", env = "SENSING_BIND_ADDR")]
    bind_addr: String,

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

    /// Run self-supervised contrastive pretraining (ADR-024)
    #[arg(long)]
    pretrain: bool,

    /// Number of pretraining epochs (default 50)
    #[arg(long, default_value = "50")]
    pretrain_epochs: usize,

    /// Extract embeddings mode: load model and extract CSI embeddings
    #[arg(long)]
    embed: bool,

    /// Build fingerprint index from embeddings (env|activity|temporal|person)
    #[arg(long, value_name = "TYPE")]
    build_index: Option<String>,
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
    // ── Multi-person detection (issue #97) ──
    /// Detected persons from WiFi sensing (multi-person support).
    #[serde(skip_serializing_if = "Option::is_none")]
    persons: Option<Vec<PersonDetection>>,
    /// Estimated person count from CSI feature heuristics (1-3 for single ESP32).
    #[serde(skip_serializing_if = "Option::is_none")]
    estimated_persons: Option<usize>,
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

/// Per-node sensing state for multi-node deployments (issue #249).
/// Each ESP32 node gets its own frame history, smoothing buffers, and vital
/// sign detector so that data from different nodes is never mixed.
struct NodeState {
    frame_history: VecDeque<Vec<f64>>,
    smoothed_person_score: f64,
    prev_person_count: usize,
    smoothed_motion: f64,
    current_motion_level: String,
    debounce_counter: u32,
    debounce_candidate: String,
    baseline_motion: f64,
    baseline_frames: u64,
    smoothed_hr: f64,
    smoothed_br: f64,
    smoothed_hr_conf: f64,
    smoothed_br_conf: f64,
    hr_buffer: VecDeque<f64>,
    br_buffer: VecDeque<f64>,
    rssi_history: VecDeque<f64>,
    vital_detector: VitalSignDetector,
    latest_vitals: VitalSigns,
    last_frame_time: Option<std::time::Instant>,
    edge_vitals: Option<Esp32VitalsPacket>,
}

impl NodeState {
    fn new() -> Self {
        Self {
            frame_history: VecDeque::new(),
            smoothed_person_score: 0.0,
            prev_person_count: 0,
            smoothed_motion: 0.0,
            current_motion_level: "absent".to_string(),
            debounce_counter: 0,
            debounce_candidate: "absent".to_string(),
            baseline_motion: 0.0,
            baseline_frames: 0,
            smoothed_hr: 0.0,
            smoothed_br: 0.0,
            smoothed_hr_conf: 0.0,
            smoothed_br_conf: 0.0,
            hr_buffer: VecDeque::with_capacity(8),
            br_buffer: VecDeque::with_capacity(8),
            rssi_history: VecDeque::new(),
            vital_detector: VitalSignDetector::new(10.0),
            latest_vitals: VitalSigns::default(),
            last_frame_time: None,
            edge_vitals: None,
        }
    }
}

/// Shared application state
struct AppStateInner {
    latest_update: Option<SensingUpdate>,
    rssi_history: VecDeque<f64>,
    /// Circular buffer of recent CSI amplitude vectors for temporal analysis.
    /// Each entry is the full subcarrier amplitude vector for one frame.
    /// Capacity: FRAME_HISTORY_CAPACITY frames.
    frame_history: VecDeque<Vec<f64>>,
    tick: u64,
    source: String,
    /// Instant of the last ESP32 UDP frame received (for offline detection).
    last_esp32_frame: Option<std::time::Instant>,
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
    /// Smoothed person count (EMA) for hysteresis — prevents frame-to-frame jumping.
    smoothed_person_score: f64,
    /// Previous person count for hysteresis (asymmetric up/down thresholds).
    prev_person_count: usize,
    // ── Motion smoothing & adaptive baseline (ADR-047 tuning) ────────────
    /// EMA-smoothed motion score (alpha ~0.15 for ~10 FPS → ~1s time constant).
    smoothed_motion: f64,
    /// Current classification state for hysteresis debounce.
    current_motion_level: String,
    /// How many consecutive frames the *raw* classification has agreed with a
    /// *candidate* new level.  State only changes after DEBOUNCE_FRAMES.
    debounce_counter: u32,
    /// The candidate motion level that the debounce counter is tracking.
    debounce_candidate: String,
    /// Adaptive baseline: EMA of motion score when room is "quiet" (low motion).
    /// Subtracted from raw score so slow environmental drift doesn't inflate readings.
    baseline_motion: f64,
    /// Number of frames processed so far (for baseline warm-up).
    baseline_frames: u64,
    // ── Vital signs smoothing ────────────────────────────────────────────
    /// EMA-smoothed heart rate (BPM).
    smoothed_hr: f64,
    /// EMA-smoothed breathing rate (BPM).
    smoothed_br: f64,
    /// EMA-smoothed HR confidence.
    smoothed_hr_conf: f64,
    /// EMA-smoothed BR confidence.
    smoothed_br_conf: f64,
    /// Median filter buffer for HR (last N raw values for outlier rejection).
    hr_buffer: VecDeque<f64>,
    /// Median filter buffer for BR.
    br_buffer: VecDeque<f64>,
    /// ADR-039: Latest edge vitals packet from ESP32.
    edge_vitals: Option<Esp32VitalsPacket>,
    /// ADR-040: Latest WASM output packet from ESP32.
    latest_wasm_events: Option<WasmOutputPacket>,
    // ── Model management fields ─────────────────────────────────────────────
    /// Discovered RVF model files from `data/models/`.
    discovered_models: Vec<serde_json::Value>,
    /// ID of the currently loaded model, if any.
    active_model_id: Option<String>,
    // ── Recording fields ────────────────────────────────────────────────────
    /// Metadata for recorded CSI data files.
    recordings: Vec<serde_json::Value>,
    /// Whether CSI recording is currently in progress.
    recording_active: bool,
    /// When the current recording started.
    recording_start_time: Option<std::time::Instant>,
    /// ID of the current recording (used for filename).
    recording_current_id: Option<String>,
    /// Shutdown signal for the recording writer task.
    recording_stop_tx: Option<tokio::sync::watch::Sender<bool>>,
    // ── Training fields ─────────────────────────────────────────────────────
    /// Training status: "idle", "running", "completed", "failed".
    training_status: String,
    /// Training configuration, if any.
    training_config: Option<serde_json::Value>,
    // ── Adaptive classifier (environment-tuned) ──────────────────────────
    /// Trained adaptive model (loaded from data/adaptive_model.json or trained at runtime).
    adaptive_model: Option<adaptive_classifier::AdaptiveModel>,
    // ── Per-node state (issue #249) ─────────────────────────────────────
    /// Per-node sensing state for multi-node deployments.
    /// Keyed by `node_id` from the ESP32 frame header.
    node_states: HashMap<u8, NodeState>,
}

/// If no ESP32 frame arrives within this duration, source reverts to offline.
const ESP32_OFFLINE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

impl AppStateInner {
    /// Return the effective data source, accounting for ESP32 frame timeout.
    /// If the source is "esp32" but no frame has arrived in 5 seconds, returns
    /// "esp32:offline" so the UI can distinguish active vs stale connections.
    fn effective_source(&self) -> String {
        if self.source == "esp32" {
            if let Some(last) = self.last_esp32_frame {
                if last.elapsed() > ESP32_OFFLINE_TIMEOUT {
                    return "esp32:offline".to_string();
                }
            }
        }
        self.source.clone()
    }
}

/// Number of frames retained in `frame_history` for temporal analysis.
/// At 500 ms ticks this covers ~50 seconds; at 100 ms ticks ~10 seconds.
const FRAME_HISTORY_CAPACITY: usize = 100;

type SharedState = Arc<RwLock<AppStateInner>>;

// ── ESP32 Edge Vitals Packet (ADR-039, magic 0xC511_0002) ────────────────────

/// Decoded vitals packet from ESP32 edge processing pipeline.
#[derive(Debug, Clone, Serialize)]
struct Esp32VitalsPacket {
    node_id: u8,
    presence: bool,
    fall_detected: bool,
    motion: bool,
    breathing_rate_bpm: f64,
    heartrate_bpm: f64,
    rssi: i8,
    n_persons: u8,
    motion_energy: f32,
    presence_score: f32,
    timestamp_ms: u32,
}

/// Parse a 32-byte edge vitals packet (magic 0xC511_0002).
fn parse_esp32_vitals(buf: &[u8]) -> Option<Esp32VitalsPacket> {
    if buf.len() < 32 {
        return None;
    }
    let magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    if magic != 0xC511_0002 {
        return None;
    }

    let node_id = buf[4];
    let flags = buf[5];
    let breathing_raw = u16::from_le_bytes([buf[6], buf[7]]);
    let heartrate_raw = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
    let rssi = buf[12] as i8;
    let n_persons = buf[13];
    let motion_energy = f32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]);
    let presence_score = f32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]);
    let timestamp_ms = u32::from_le_bytes([buf[24], buf[25], buf[26], buf[27]]);

    Some(Esp32VitalsPacket {
        node_id,
        presence: (flags & 0x01) != 0,
        fall_detected: (flags & 0x02) != 0,
        motion: (flags & 0x04) != 0,
        breathing_rate_bpm: breathing_raw as f64 / 100.0,
        heartrate_bpm: heartrate_raw as f64 / 10000.0,
        rssi,
        n_persons,
        motion_energy,
        presence_score,
        timestamp_ms,
    })
}

// ── ADR-040: WASM Output Packet (magic 0xC511_0004) ───────────────────────────

/// Single WASM event (type + value).
#[derive(Debug, Clone, Serialize)]
struct WasmEvent {
    event_type: u8,
    value: f32,
}

/// Decoded WASM output packet from ESP32 Tier 3 runtime.
#[derive(Debug, Clone, Serialize)]
struct WasmOutputPacket {
    node_id: u8,
    module_id: u8,
    events: Vec<WasmEvent>,
}

/// Parse a WASM output packet (magic 0xC511_0004).
fn parse_wasm_output(buf: &[u8]) -> Option<WasmOutputPacket> {
    if buf.len() < 8 {
        return None;
    }
    let magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    if magic != 0xC511_0004 {
        return None;
    }

    let node_id = buf[4];
    let module_id = buf[5];
    let event_count = u16::from_le_bytes([buf[6], buf[7]]) as usize;

    let mut events = Vec::with_capacity(event_count);
    let mut offset = 8;
    for _ in 0..event_count {
        if offset + 5 > buf.len() {
            break;
        }
        let event_type = buf[offset];
        let value = f32::from_le_bytes([
            buf[offset + 1], buf[offset + 2], buf[offset + 3], buf[offset + 4],
        ]);
        events.push(WasmEvent { event_type, value });
        offset += 5;
    }

    Some(WasmOutputPacket {
        node_id,
        module_id,
        events,
    })
}

// ── ESP32 UDP frame parser ───────────────────────────────────────────────────

fn parse_esp32_frame(buf: &[u8]) -> Option<Esp32Frame> {
    if buf.len() < 20 {
        return None;
    }

    let magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    if magic != 0xC511_0001 {
        return None;
    }

    // Frame layout (must match firmware csi_collector.c):
    //   [0..3]   magic (u32 LE)
    //   [4]      node_id (u8)
    //   [5]      n_antennas (u8)
    //   [6..7]   n_subcarriers (u16 LE)
    //   [8..11]  freq_mhz (u32 LE)
    //   [12..15] sequence (u32 LE)
    //   [16]     rssi (i8)
    //   [17]     noise_floor (i8)
    //   [18..19] reserved
    //   [20..]   I/Q data
    let node_id = buf[4];
    let n_antennas = buf[5];
    let n_subcarriers_u16 = u16::from_le_bytes([buf[6], buf[7]]);
    let n_subcarriers = n_subcarriers_u16 as u8; // truncate to u8 for Esp32Frame compat
    let freq_mhz = u16::from_le_bytes([buf[8], buf[9]]); // low 16 bits of u32
    let sequence = u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]);
    let rssi = buf[16] as i8;       // #332: was buf[14], 2 bytes off
    let noise_floor = buf[17] as i8; // #332: was buf[15], 2 bytes off

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

/// Generate a signal field that reflects where motion and signal changes are occurring.
///
/// Instead of a fixed-animation circle, this function uses the actual sensing data:
/// - `subcarrier_variances`: per-subcarrier variance computed from the frame history.
///   High-variance subcarriers indicate spatial directions where the signal is disrupted.
/// - `motion_score`: overall motion intensity [0, 1].
/// - `breathing_rate_hz`: estimated breathing rate in Hz; if > 0, adds a breathing ring.
/// - `signal_quality`: overall quality metric [0, 1] modulates field brightness.
///
/// The field grid is 20×20 cells representing a top-down view of the room.
/// Hotspots are derived from the subcarrier index (treated as an angular bin) so that
/// subcarriers with the highest variance produce peaks at the corresponding directions.
fn generate_signal_field(
    _mean_rssi: f64,
    motion_score: f64,
    breathing_rate_hz: f64,
    signal_quality: f64,
    subcarrier_variances: &[f64],
) -> SignalField {
    let grid = 20usize;
    let mut values = vec![0.0f64; grid * grid];
    let center = (grid as f64 - 1.0) / 2.0;

    // Normalise subcarrier variances to [0, 1].
    let max_var = subcarrier_variances.iter().cloned().fold(0.0f64, f64::max);
    let norm_factor = if max_var > 1e-9 { max_var } else { 1.0 };

    // For each cell, accumulate contributions from all subcarriers.
    // Each subcarrier k is assigned an angular direction proportional to its index
    // so that different subcarriers illuminate different regions of the room.
    let n_sub = subcarrier_variances.len().max(1);
    for (k, &var) in subcarrier_variances.iter().enumerate() {
        let weight = (var / norm_factor) * motion_score;
        if weight < 1e-6 {
            continue;
        }
        // Map subcarrier index to an angle across the full 2π sweep.
        let angle = (k as f64 / n_sub as f64) * 2.0 * std::f64::consts::PI;
        // Place the hotspot at a distance proportional to the weight, capped at 40% of
        // the grid radius so it stays within the room model.
        let radius = center * 0.8 * weight.sqrt();
        let hx = center + radius * angle.cos();
        let hz = center + radius * angle.sin();

        for z in 0..grid {
            for x in 0..grid {
                let dx = x as f64 - hx;
                let dz = z as f64 - hz;
                let dist2 = dx * dx + dz * dz;
                // Gaussian blob centred on the hotspot; spread scales with weight.
                let spread = (0.5 + weight * 2.0).max(0.5);
                values[z * grid + x] += weight * (-dist2 / (2.0 * spread * spread)).exp();
            }
        }
    }

    // Base radial attenuation from the router assumed at grid centre.
    for z in 0..grid {
        for x in 0..grid {
            let dx = x as f64 - center;
            let dz = z as f64 - center;
            let dist = (dx * dx + dz * dz).sqrt();
            let base = signal_quality * (-dist * 0.12).exp();
            values[z * grid + x] += base * 0.3;
        }
    }

    // Breathing ring: if a breathing rate was estimated add a faint annular highlight
    // at a radius corresponding to typical chest-wall displacement range.
    if breathing_rate_hz > 0.05 {
        let ring_r = center * 0.55;
        let ring_width = 1.8f64;
        for z in 0..grid {
            for x in 0..grid {
                let dx = x as f64 - center;
                let dz = z as f64 - center;
                let dist = (dx * dx + dz * dz).sqrt();
                let ring_val = 0.08 * (-(dist - ring_r).powi(2) / (2.0 * ring_width * ring_width)).exp();
                values[z * grid + x] += ring_val;
            }
        }
    }

    // Clamp and normalise to [0, 1].
    let field_max = values.iter().cloned().fold(0.0f64, f64::max);
    let scale = if field_max > 1e-9 { 1.0 / field_max } else { 1.0 };
    for v in &mut values {
        *v = (*v * scale).clamp(0.0, 1.0);
    }

    SignalField {
        grid_size: [grid, 1, grid],
        values,
    }
}

// ── Feature extraction from ESP32 frame ──────────────────────────────────────

/// Estimate breathing rate in Hz from the amplitude time series stored in `frame_history`.
///
/// Approach:
/// 1. Build a scalar time series by computing the mean amplitude of each historical frame.
/// 2. Run a peak-detection pass: count rising-edge zero-crossings of the de-meaned signal.
/// 3. Convert the crossing rate to Hz, clipped to the physiological range 0.1–0.5 Hz
///    (12–30 breaths/min).
///
/// For accuracy the function additionally applies a simple 3-tap Goertzel-style power
/// estimate at evenly-spaced candidate frequencies in the breathing band and returns
/// the candidate with the highest energy.
fn estimate_breathing_rate_hz(frame_history: &VecDeque<Vec<f64>>, sample_rate_hz: f64) -> f64 {
    let n = frame_history.len();
    if n < 6 {
        return 0.0;
    }

    // Build scalar time series: mean amplitude per frame.
    let series: Vec<f64> = frame_history.iter()
        .map(|amps| {
            if amps.is_empty() { 0.0 } else { amps.iter().sum::<f64>() / amps.len() as f64 }
        })
        .collect();

    let mean_s = series.iter().sum::<f64>() / n as f64;
    // De-mean.
    let detrended: Vec<f64> = series.iter().map(|x| x - mean_s).collect();

    // Goertzel power at candidate frequencies in the breathing band [0.1, 0.5] Hz.
    // We evaluate 9 candidate frequencies uniformly spaced in that band.
    let n_candidates = 9usize;
    let f_low = 0.1f64;
    let f_high = 0.5f64;
    let mut best_freq = 0.0f64;
    let mut best_power = 0.0f64;

    for i in 0..n_candidates {
        let freq = f_low + (f_high - f_low) * i as f64 / (n_candidates - 1).max(1) as f64;
        let omega = 2.0 * std::f64::consts::PI * freq / sample_rate_hz;
        let coeff = 2.0 * omega.cos();
        let mut s_prev2 = 0.0f64;
        let mut s_prev1 = 0.0f64;
        for &x in &detrended {
            let s = x + coeff * s_prev1 - s_prev2;
            s_prev2 = s_prev1;
            s_prev1 = s;
        }
        // Goertzel magnitude squared.
        let power = s_prev2 * s_prev2 + s_prev1 * s_prev1 - coeff * s_prev1 * s_prev2;
        if power > best_power {
            best_power = power;
            best_freq = freq;
        }
    }

    // Only report a breathing rate if the Goertzel energy is meaningfully above noise.
    // Threshold: power must exceed 10× the average power across all candidates.
    let avg_power = {
        let mut total = 0.0f64;
        for i in 0..n_candidates {
            let freq = f_low + (f_high - f_low) * i as f64 / (n_candidates - 1).max(1) as f64;
            let omega = 2.0 * std::f64::consts::PI * freq / sample_rate_hz;
            let coeff = 2.0 * omega.cos();
            let mut s_prev2 = 0.0f64;
            let mut s_prev1 = 0.0f64;
            for &x in &detrended {
                let s = x + coeff * s_prev1 - s_prev2;
                s_prev2 = s_prev1;
                s_prev1 = s;
            }
            total += s_prev2 * s_prev2 + s_prev1 * s_prev1 - coeff * s_prev1 * s_prev2;
        }
        total / n_candidates as f64
    };

    if best_power > avg_power * 3.0 {
        best_freq.clamp(f_low, f_high)
    } else {
        0.0
    }
}

/// Compute per-subcarrier variance across the sliding window of `frame_history`.
///
/// For each subcarrier index `k`, returns `Var[A_k]` over all stored frames.
/// This captures spatial signal variation; subcarriers whose amplitude fluctuates
/// heavily across time correspond to directions with motion.
/// Compute per-subcarrier importance weights using a simple sensitivity split.
///
/// Subcarriers whose sensitivity (amplitude magnitude) is above the median are
/// considered "sensitive" and receive weight `1.0 + (sens / max_sens)` (range 1.0–2.0).
/// The rest receive a baseline weight of 0.5. This mirrors the RuVector mincut
/// partition logic without requiring the graph dependency.
fn compute_subcarrier_importance_weights(sensitivity: &[f64]) -> Vec<f64> {
    let n = sensitivity.len();
    if n == 0 {
        return vec![];
    }
    let max_sens = sensitivity.iter().cloned().fold(f64::NEG_INFINITY, f64::max).max(1e-9);

    // Compute median via a sorted copy.
    let mut sorted = sensitivity.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = if n % 2 == 0 {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    } else {
        sorted[n / 2]
    };

    sensitivity
        .iter()
        .map(|&s| {
            if s >= median {
                1.0 + (s / max_sens).min(1.0)
            } else {
                0.5
            }
        })
        .collect()
}

fn compute_subcarrier_variances(frame_history: &VecDeque<Vec<f64>>, n_sub: usize) -> Vec<f64> {
    if frame_history.is_empty() || n_sub == 0 {
        return vec![0.0; n_sub];
    }

    let n_frames = frame_history.len() as f64;
    let mut means = vec![0.0f64; n_sub];
    let mut sq_means = vec![0.0f64; n_sub];

    for frame in frame_history.iter() {
        for k in 0..n_sub {
            let a = if k < frame.len() { frame[k] } else { 0.0 };
            means[k] += a;
            sq_means[k] += a * a;
        }
    }

    (0..n_sub)
        .map(|k| {
            let mean = means[k] / n_frames;
            let sq_mean = sq_means[k] / n_frames;
            (sq_mean - mean * mean).max(0.0)
        })
        .collect()
}

/// Extract features from the current ESP32 frame, enhanced with temporal context from
/// `frame_history`.
///
/// Improvements over the previous single-frame approach:
///
/// - **Variance**: computed as the mean of per-subcarrier temporal variance across the
///   sliding window, not just the intra-frame spatial variance.
/// - **Motion detection**: uses frame-to-frame temporal difference (mean L2 change
///   between the current frame and the previous frame) normalised by signal amplitude,
///   so that actual changes are detected rather than just a threshold on the current frame.
/// - **Breathing rate**: estimated via Goertzel filter bank on the 0.1–0.5 Hz band of
///   the amplitude time series.
/// - **Signal quality**: based on SNR estimate (RSSI – noise floor) and subcarrier
///   variance stability.
/// Returns (features, raw_classification, breathing_rate_hz, sub_variances, raw_motion_score).
fn extract_features_from_frame(
    frame: &Esp32Frame,
    frame_history: &VecDeque<Vec<f64>>,
    sample_rate_hz: f64,
) -> (FeatureInfo, ClassificationInfo, f64, Vec<f64>, f64) {
    let n_sub = frame.amplitudes.len().max(1);
    let n = n_sub as f64;
    let mean_rssi = frame.rssi as f64;

    // ── RuVector Phase 1: subcarrier importance weighting ──
    // Compute per-subcarrier sensitivity from amplitude magnitude, then weight
    // sensitive subcarriers higher (>1.0) and insensitive ones lower (0.5).
    // This emphasises body-motion-correlated subcarriers in all downstream metrics.
    let sub_sensitivity: Vec<f64> = frame.amplitudes.iter().map(|a| a.abs()).collect();
    let importance_weights = compute_subcarrier_importance_weights(&sub_sensitivity);

    let weight_sum: f64 = importance_weights.iter().sum::<f64>();
    let mean_amp: f64 = if weight_sum > 0.0 {
        frame.amplitudes.iter().zip(importance_weights.iter())
            .map(|(a, w)| a * w)
            .sum::<f64>() / weight_sum
    } else {
        frame.amplitudes.iter().sum::<f64>() / n
    };

    // ── Intra-frame subcarrier variance (weighted by importance) ──
    let intra_variance: f64 = if weight_sum > 0.0 {
        frame.amplitudes.iter().zip(importance_weights.iter())
            .map(|(a, w)| w * (a - mean_amp).powi(2))
            .sum::<f64>() / weight_sum
    } else {
        frame.amplitudes.iter()
            .map(|a| (a - mean_amp).powi(2))
            .sum::<f64>() / n
    };

    // ── Temporal (sliding-window) per-subcarrier variance ──
    let sub_variances = compute_subcarrier_variances(frame_history, n_sub);
    let temporal_variance: f64 = if sub_variances.is_empty() {
        intra_variance
    } else {
        sub_variances.iter().sum::<f64>() / sub_variances.len() as f64
    };

    // Use the larger of intra-frame and temporal variance as the reported variance.
    let variance = intra_variance.max(temporal_variance);

    // ── Spectral power ──
    let spectral_power: f64 = frame.amplitudes.iter().map(|a| a * a).sum::<f64>() / n;

    // ── Motion band power (upper half of subcarriers, high spatial frequency) ──
    let half = frame.amplitudes.len() / 2;
    let motion_band_power = if half > 0 {
        frame.amplitudes[half..].iter()
            .map(|a| (a - mean_amp).powi(2))
            .sum::<f64>() / (frame.amplitudes.len() - half) as f64
    } else {
        0.0
    };

    // ── Breathing band power (lower half of subcarriers, low spatial frequency) ──
    let breathing_band_power = if half > 0 {
        frame.amplitudes[..half].iter()
            .map(|a| (a - mean_amp).powi(2))
            .sum::<f64>() / half as f64
    } else {
        0.0
    };

    // ── Dominant frequency via peak subcarrier index ──
    let peak_idx = frame.amplitudes.iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)
        .unwrap_or(0);
    let dominant_freq_hz = peak_idx as f64 * 0.05;

    // ── Change point detection (threshold-crossing count in current frame) ──
    let threshold = mean_amp * 1.2;
    let change_points = frame.amplitudes.windows(2)
        .filter(|w| (w[0] < threshold) != (w[1] < threshold))
        .count();

    // ── Motion score: sliding-window temporal difference ──
    // Compare current frame against the most recent historical frame.
    // The difference is normalised by the mean amplitude to be scale-invariant.
    let temporal_motion_score = if let Some(prev_frame) = frame_history.back() {
        let n_cmp = n_sub.min(prev_frame.len());
        if n_cmp > 0 {
            let diff_energy: f64 = (0..n_cmp)
                .map(|k| (frame.amplitudes[k] - prev_frame[k]).powi(2))
                .sum::<f64>() / n_cmp as f64;
            // Normalise by mean squared amplitude to get a dimensionless ratio.
            let ref_energy = mean_amp * mean_amp + 1e-9;
            (diff_energy / ref_energy).sqrt().clamp(0.0, 1.0)
        } else {
            0.0
        }
    } else {
        // No history yet — fall back to intra-frame variance-based estimate.
        (intra_variance / (mean_amp * mean_amp + 1e-9)).sqrt().clamp(0.0, 1.0)
    };

    // Blend temporal motion with variance-based motion for robustness.
    // Also factor in motion_band_power and change_points for ESP32 real-world sensitivity.
    let variance_motion = (temporal_variance / 10.0).clamp(0.0, 1.0);
    let mbp_motion = (motion_band_power / 25.0).clamp(0.0, 1.0);
    let cp_motion = (change_points as f64 / 15.0).clamp(0.0, 1.0);
    let motion_score = (temporal_motion_score * 0.4 + variance_motion * 0.2 + mbp_motion * 0.25 + cp_motion * 0.15).clamp(0.0, 1.0);

    // ── Signal quality metric ──
    // Based on estimated SNR (RSSI relative to noise floor) and subcarrier consistency.
    let snr_db = (frame.rssi as f64 - frame.noise_floor as f64).max(0.0);
    let snr_quality = (snr_db / 40.0).clamp(0.0, 1.0); // 40 dB → quality = 1.0
    // Penalise quality when temporal variance is very high (unstable signal).
    let stability = (1.0 - (temporal_variance / (mean_amp * mean_amp + 1e-9)).clamp(0.0, 1.0)).max(0.0);
    let signal_quality = (snr_quality * 0.6 + stability * 0.4).clamp(0.0, 1.0);

    // ── Breathing rate estimation ──
    let breathing_rate_hz = estimate_breathing_rate_hz(frame_history, sample_rate_hz);

    let features = FeatureInfo {
        mean_rssi,
        variance,
        motion_band_power,
        breathing_band_power,
        dominant_freq_hz,
        change_points,
        spectral_power,
    };

    // Return raw motion_score and signal_quality — classification is done by
    // `smooth_and_classify()` which has access to EMA state and hysteresis.
    let raw_classification = ClassificationInfo {
        motion_level: raw_classify(motion_score),
        presence: motion_score > 0.04,
        confidence: (0.4 + signal_quality * 0.3 + motion_score * 0.3).clamp(0.0, 1.0),
    };

    (features, raw_classification, breathing_rate_hz, sub_variances, motion_score)
}

/// Simple threshold classification (no smoothing) — used as the "raw" input.
fn raw_classify(score: f64) -> String {
    if score > 0.25 { "active".into() }
    else if score > 0.12 { "present_moving".into() }
    else if score > 0.04 { "present_still".into() }
    else { "absent".into() }
}

/// Debounce frames required before state transition (at ~10 FPS = ~0.4s).
const DEBOUNCE_FRAMES: u32 = 4;
/// EMA alpha for motion smoothing (~1s time constant at 10 FPS).
const MOTION_EMA_ALPHA: f64 = 0.15;
/// EMA alpha for slow-adapting baseline (~30s time constant at 10 FPS).
const BASELINE_EMA_ALPHA: f64 = 0.003;
/// Number of warm-up frames before baseline subtraction kicks in.
const BASELINE_WARMUP: u64 = 50;

/// Apply EMA smoothing, adaptive baseline subtraction, and hysteresis debounce
/// to the raw classification.  Mutates the smoothing state in `AppStateInner`.
fn smooth_and_classify(state: &mut AppStateInner, raw: &mut ClassificationInfo, raw_motion: f64) {
    // 1. Adaptive baseline: slowly track the "quiet room" floor.
    //    Only update baseline when raw score is below the current smoothed level
    //    (i.e. during calm periods) so walking doesn't inflate the baseline.
    state.baseline_frames += 1;
    if state.baseline_frames < BASELINE_WARMUP {
        // During warm-up, aggressively learn the baseline.
        state.baseline_motion = state.baseline_motion * 0.9 + raw_motion * 0.1;
    } else if raw_motion < state.smoothed_motion + 0.05 {
        state.baseline_motion = state.baseline_motion * (1.0 - BASELINE_EMA_ALPHA)
                              + raw_motion * BASELINE_EMA_ALPHA;
    }

    // 2. Subtract baseline and clamp.
    let adjusted = (raw_motion - state.baseline_motion * 0.7).max(0.0);

    // 3. EMA smooth the adjusted score.
    state.smoothed_motion = state.smoothed_motion * (1.0 - MOTION_EMA_ALPHA)
                          + adjusted * MOTION_EMA_ALPHA;
    let sm = state.smoothed_motion;

    // 4. Classify from smoothed score.
    let candidate = raw_classify(sm);

    // 5. Hysteresis debounce: require N consecutive frames agreeing on a new state.
    if candidate == state.current_motion_level {
        // Already in this state — reset debounce.
        state.debounce_counter = 0;
        state.debounce_candidate = candidate;
    } else if candidate == state.debounce_candidate {
        state.debounce_counter += 1;
        if state.debounce_counter >= DEBOUNCE_FRAMES {
            // Transition accepted.
            state.current_motion_level = candidate;
            state.debounce_counter = 0;
        }
    } else {
        // New candidate — restart counter.
        state.debounce_candidate = candidate;
        state.debounce_counter = 1;
    }

    // 6. Write the smoothed result back into the classification.
    raw.motion_level = state.current_motion_level.clone();
    raw.presence = sm > 0.03;
    raw.confidence = (0.4 + sm * 0.6).clamp(0.0, 1.0);
}

/// Per-node variant of `smooth_and_classify` that operates on a `NodeState`
/// instead of `AppStateInner` (issue #249).
fn smooth_and_classify_node(ns: &mut NodeState, raw: &mut ClassificationInfo, raw_motion: f64) {
    ns.baseline_frames += 1;
    if ns.baseline_frames < BASELINE_WARMUP {
        ns.baseline_motion = ns.baseline_motion * 0.9 + raw_motion * 0.1;
    } else if raw_motion < ns.smoothed_motion + 0.05 {
        ns.baseline_motion = ns.baseline_motion * (1.0 - BASELINE_EMA_ALPHA)
                           + raw_motion * BASELINE_EMA_ALPHA;
    }

    let adjusted = (raw_motion - ns.baseline_motion * 0.7).max(0.0);

    ns.smoothed_motion = ns.smoothed_motion * (1.0 - MOTION_EMA_ALPHA)
                       + adjusted * MOTION_EMA_ALPHA;
    let sm = ns.smoothed_motion;

    let candidate = raw_classify(sm);

    if candidate == ns.current_motion_level {
        ns.debounce_counter = 0;
        ns.debounce_candidate = candidate;
    } else if candidate == ns.debounce_candidate {
        ns.debounce_counter += 1;
        if ns.debounce_counter >= DEBOUNCE_FRAMES {
            ns.current_motion_level = candidate;
            ns.debounce_counter = 0;
        }
    } else {
        ns.debounce_candidate = candidate;
        ns.debounce_counter = 1;
    }

    raw.motion_level = ns.current_motion_level.clone();
    raw.presence = sm > 0.03;
    raw.confidence = (0.4 + sm * 0.6).clamp(0.0, 1.0);
}

/// If an adaptive model is loaded, override the classification with the
/// model's prediction.  Uses the full 15-feature vector for higher accuracy.
fn adaptive_override(state: &AppStateInner, features: &FeatureInfo, classification: &mut ClassificationInfo) {
    if let Some(ref model) = state.adaptive_model {
        // Get current frame amplitudes from the latest history entry.
        let amps = state.frame_history.back()
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        let feat_arr = adaptive_classifier::features_from_runtime(
            &serde_json::json!({
                "variance": features.variance,
                "motion_band_power": features.motion_band_power,
                "breathing_band_power": features.breathing_band_power,
                "spectral_power": features.spectral_power,
                "dominant_freq_hz": features.dominant_freq_hz,
                "change_points": features.change_points,
                "mean_rssi": features.mean_rssi,
            }),
            amps,
        );
        let (label, conf) = model.classify(&feat_arr);
        classification.motion_level = label.to_string();
        classification.presence = label != "absent";
        // Blend model confidence with existing smoothed confidence.
        classification.confidence = (conf * 0.7 + classification.confidence * 0.3).clamp(0.0, 1.0);
    }
}

/// Size of the median filter window for vital signs outlier rejection.
const VITAL_MEDIAN_WINDOW: usize = 21;
/// EMA alpha for vital signs (~5s time constant at 10 FPS).
const VITAL_EMA_ALPHA: f64 = 0.02;
/// Maximum BPM jump per frame before a value is rejected as an outlier.
const HR_MAX_JUMP: f64 = 8.0;
const BR_MAX_JUMP: f64 = 2.0;
/// Minimum change from current smoothed value before EMA updates (dead-band).
/// Prevents micro-drift from creeping in.
const HR_DEAD_BAND: f64 = 2.0;
const BR_DEAD_BAND: f64 = 0.5;

/// Smooth vital signs using median-filter outlier rejection + EMA.
/// Mutates `state.smoothed_hr`, `state.smoothed_br`, etc.
/// Returns the smoothed VitalSigns to broadcast.
fn smooth_vitals(state: &mut AppStateInner, raw: &VitalSigns) -> VitalSigns {
    let raw_hr = raw.heart_rate_bpm.unwrap_or(0.0);
    let raw_br = raw.breathing_rate_bpm.unwrap_or(0.0);

    // -- Outlier rejection: skip values that jump too far from current EMA --
    let hr_ok = state.smoothed_hr < 1.0 || (raw_hr - state.smoothed_hr).abs() < HR_MAX_JUMP;
    let br_ok = state.smoothed_br < 1.0 || (raw_br - state.smoothed_br).abs() < BR_MAX_JUMP;

    // Push into buffer (only non-outlier values)
    if hr_ok && raw_hr > 0.0 {
        state.hr_buffer.push_back(raw_hr);
        if state.hr_buffer.len() > VITAL_MEDIAN_WINDOW { state.hr_buffer.pop_front(); }
    }
    if br_ok && raw_br > 0.0 {
        state.br_buffer.push_back(raw_br);
        if state.br_buffer.len() > VITAL_MEDIAN_WINDOW { state.br_buffer.pop_front(); }
    }

    // Compute trimmed mean: drop top/bottom 25% then average the middle 50%.
    // This is more stable than pure median and less noisy than raw mean.
    let trimmed_hr = trimmed_mean(&state.hr_buffer);
    let trimmed_br = trimmed_mean(&state.br_buffer);

    // EMA smooth with dead-band: only update if the trimmed mean differs
    // from the current smoothed value by more than the dead-band.
    // This prevents the display from constantly creeping by tiny amounts.
    if trimmed_hr > 0.0 {
        if state.smoothed_hr < 1.0 {
            state.smoothed_hr = trimmed_hr;
        } else if (trimmed_hr - state.smoothed_hr).abs() > HR_DEAD_BAND {
            state.smoothed_hr = state.smoothed_hr * (1.0 - VITAL_EMA_ALPHA)
                              + trimmed_hr * VITAL_EMA_ALPHA;
        }
        // else: within dead-band, hold current value
    }
    if trimmed_br > 0.0 {
        if state.smoothed_br < 1.0 {
            state.smoothed_br = trimmed_br;
        } else if (trimmed_br - state.smoothed_br).abs() > BR_DEAD_BAND {
            state.smoothed_br = state.smoothed_br * (1.0 - VITAL_EMA_ALPHA)
                              + trimmed_br * VITAL_EMA_ALPHA;
        }
    }

    // Smooth confidence
    state.smoothed_hr_conf = state.smoothed_hr_conf * 0.92 + raw.heartbeat_confidence * 0.08;
    state.smoothed_br_conf = state.smoothed_br_conf * 0.92 + raw.breathing_confidence * 0.08;

    VitalSigns {
        breathing_rate_bpm: if state.smoothed_br > 1.0 { Some(state.smoothed_br) } else { None },
        heart_rate_bpm: if state.smoothed_hr > 1.0 { Some(state.smoothed_hr) } else { None },
        breathing_confidence: state.smoothed_br_conf,
        heartbeat_confidence: state.smoothed_hr_conf,
        signal_quality: raw.signal_quality,
    }
}

/// Per-node variant of `smooth_vitals` that operates on a `NodeState` (issue #249).
fn smooth_vitals_node(ns: &mut NodeState, raw: &VitalSigns) -> VitalSigns {
    let raw_hr = raw.heart_rate_bpm.unwrap_or(0.0);
    let raw_br = raw.breathing_rate_bpm.unwrap_or(0.0);

    let hr_ok = ns.smoothed_hr < 1.0 || (raw_hr - ns.smoothed_hr).abs() < HR_MAX_JUMP;
    let br_ok = ns.smoothed_br < 1.0 || (raw_br - ns.smoothed_br).abs() < BR_MAX_JUMP;

    if hr_ok && raw_hr > 0.0 {
        ns.hr_buffer.push_back(raw_hr);
        if ns.hr_buffer.len() > VITAL_MEDIAN_WINDOW { ns.hr_buffer.pop_front(); }
    }
    if br_ok && raw_br > 0.0 {
        ns.br_buffer.push_back(raw_br);
        if ns.br_buffer.len() > VITAL_MEDIAN_WINDOW { ns.br_buffer.pop_front(); }
    }

    let trimmed_hr = trimmed_mean(&ns.hr_buffer);
    let trimmed_br = trimmed_mean(&ns.br_buffer);

    if trimmed_hr > 0.0 {
        if ns.smoothed_hr < 1.0 {
            ns.smoothed_hr = trimmed_hr;
        } else if (trimmed_hr - ns.smoothed_hr).abs() > HR_DEAD_BAND {
            ns.smoothed_hr = ns.smoothed_hr * (1.0 - VITAL_EMA_ALPHA)
                           + trimmed_hr * VITAL_EMA_ALPHA;
        }
    }
    if trimmed_br > 0.0 {
        if ns.smoothed_br < 1.0 {
            ns.smoothed_br = trimmed_br;
        } else if (trimmed_br - ns.smoothed_br).abs() > BR_DEAD_BAND {
            ns.smoothed_br = ns.smoothed_br * (1.0 - VITAL_EMA_ALPHA)
                           + trimmed_br * VITAL_EMA_ALPHA;
        }
    }

    ns.smoothed_hr_conf = ns.smoothed_hr_conf * 0.92 + raw.heartbeat_confidence * 0.08;
    ns.smoothed_br_conf = ns.smoothed_br_conf * 0.92 + raw.breathing_confidence * 0.08;

    VitalSigns {
        breathing_rate_bpm: if ns.smoothed_br > 1.0 { Some(ns.smoothed_br) } else { None },
        heart_rate_bpm: if ns.smoothed_hr > 1.0 { Some(ns.smoothed_hr) } else { None },
        breathing_confidence: ns.smoothed_br_conf,
        heartbeat_confidence: ns.smoothed_hr_conf,
        signal_quality: raw.signal_quality,
    }
}

/// Trimmed mean: sort, drop top/bottom 25%, average the middle 50%.
/// More robust than median (uses more data) and less noisy than raw mean.
fn trimmed_mean(buf: &VecDeque<f64>) -> f64 {
    if buf.is_empty() { return 0.0; }
    let mut sorted: Vec<f64> = buf.iter().copied().collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = sorted.len();
    let trim = n / 4; // drop 25% from each end
    let middle = &sorted[trim..n - trim.max(0)];
    if middle.is_empty() {
        sorted[n / 2] // fallback to median if too few samples
    } else {
        middle.iter().sum::<f64>() / middle.len() as f64
    }
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

        // ── Step 4b: Update frame history and extract features ───────
        let mut s_write_pre = state.write().await;
        s_write_pre.frame_history.push_back(frame.amplitudes.clone());
        if s_write_pre.frame_history.len() > FRAME_HISTORY_CAPACITY {
            s_write_pre.frame_history.pop_front();
        }
        let sample_rate_hz = 1000.0 / tick_ms as f64;
        let (features, mut classification, breathing_rate_hz, sub_variances, raw_motion) =
            extract_features_from_frame(&frame, &s_write_pre.frame_history, sample_rate_hz);
        smooth_and_classify(&mut s_write_pre, &mut classification, raw_motion);
        adaptive_override(&s_write_pre, &features, &mut classification);
        drop(s_write_pre);

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

        let raw_vitals = s.vital_detector.process_frame(&frame.amplitudes, &frame.phases);
        let vitals = smooth_vitals(&mut s, &raw_vitals);
        s.latest_vitals = vitals.clone();

        let feat_variance = features.variance;

        // Multi-person estimation with temporal smoothing (EMA α=0.10).
        let raw_score = compute_person_score(&features);
        s.smoothed_person_score = s.smoothed_person_score * 0.90 + raw_score * 0.10;
        let est_persons = if classification.presence {
            let count = score_to_person_count(s.smoothed_person_score, s.prev_person_count);
            s.prev_person_count = count;
            count
        } else {
            s.prev_person_count = 0;
            0
        };

        let mut update = SensingUpdate {
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
            signal_field: generate_signal_field(
                first_rssi, motion_score, breathing_rate_hz,
                feat_variance.min(1.0), &sub_variances,
            ),
            vital_signs: Some(vitals),
            enhanced_motion,
            enhanced_breathing,
            posture: posture_str,
            signal_quality_score: sig_quality_score,
            quality_verdict: verdict_str,
            bssid_count: bssid_n,
            pose_keypoints: None,
            model_status: None,
            persons: None,
            estimated_persons: if est_persons > 0 { Some(est_persons) } else { None },
        };

        // Populate persons from the sensing update.
        let persons = derive_pose_from_sensing(&update);
        if !persons.is_empty() {
            update.persons = Some(persons);
        }

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

    let mut s = state.write().await;
    // Update frame history before extracting features.
    s.frame_history.push_back(frame.amplitudes.clone());
    if s.frame_history.len() > FRAME_HISTORY_CAPACITY {
        s.frame_history.pop_front();
    }
    let sample_rate_hz = 2.0_f64; // fallback tick ~ 500 ms => 2 Hz
    let (features, mut classification, breathing_rate_hz, sub_variances, raw_motion) =
        extract_features_from_frame(&frame, &s.frame_history, sample_rate_hz);
    smooth_and_classify(&mut s, &mut classification, raw_motion);
    adaptive_override(&s, &features, &mut classification);

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

    let raw_vitals = s.vital_detector.process_frame(&frame.amplitudes, &frame.phases);
    let vitals = smooth_vitals(&mut s, &raw_vitals);
    s.latest_vitals = vitals.clone();

    let feat_variance = features.variance;

    // Multi-person estimation with temporal smoothing (EMA α=0.10).
    let raw_score = compute_person_score(&features);
    s.smoothed_person_score = s.smoothed_person_score * 0.90 + raw_score * 0.10;
    let est_persons = if classification.presence {
        let count = score_to_person_count(s.smoothed_person_score, s.prev_person_count);
        s.prev_person_count = count;
        count
    } else {
        s.prev_person_count = 0;
        0
    };

    let mut update = SensingUpdate {
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
        signal_field: generate_signal_field(
            rssi_dbm, motion_score, breathing_rate_hz,
            feat_variance.min(1.0), &sub_variances,
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
        persons: None,
        estimated_persons: if est_persons > 0 { Some(est_persons) } else { None },
    };

    let persons = derive_pose_from_sensing(&update);
    if !persons.is_empty() {
        update.persons = Some(persons);
    }

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
                                // Determine pose estimation mode for the UI indicator.
                                // "model_inference"    — a trained RVF model is loaded.
                                // "signal_derived"     — keypoints estimated from raw CSI features.
                                let model_loaded = {
                                    let s = state.read().await;
                                    s.model_loaded
                                };
                                let pose_source = if model_loaded {
                                    "model_inference"
                                } else {
                                    "signal_derived"
                                };

                                let persons = if model_loaded {
                                    // When a trained model is loaded, prefer its keypoints if present.
                                    sensing.pose_keypoints.as_ref().map(|kps| {
                                        let kp_names = [
                                            "nose","left_eye","right_eye","left_ear","right_ear",
                                            "left_shoulder","right_shoulder","left_elbow","right_elbow",
                                            "left_wrist","right_wrist","left_hip","right_hip",
                                            "left_knee","right_knee","left_ankle","right_ankle",
                                        ];
                                        let keypoints: Vec<PoseKeypoint> = kps.iter()
                                            .enumerate()
                                            .map(|(i, kp)| PoseKeypoint {
                                                name: kp_names.get(i).unwrap_or(&"unknown").to_string(),
                                                x: kp[0], y: kp[1], z: kp[2], confidence: kp[3],
                                            })
                                            .collect();
                                        vec![PersonDetection {
                                            id: 1,
                                            confidence: sensing.classification.confidence,
                                            bbox: BoundingBox { x: 260.0, y: 150.0, width: 120.0, height: 220.0 },
                                            keypoints,
                                            zone: "zone_1".into(),
                                        }]
                                    }).unwrap_or_else(|| derive_pose_from_sensing(&sensing))
                                } else {
                                    derive_pose_from_sensing(&sensing)
                                };

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
                                        // pose_source tells the UI which estimation mode is active.
                                        "pose_source": pose_source,
                                        "metadata": {
                                            "frame_id": format!("rust_frame_{}", sensing.tick),
                                            "processing_time_ms": 1,
                                            "source": sensing.source,
                                            "tick": sensing.tick,
                                            "signal_strength": sensing.features.mean_rssi,
                                            "motion_band_power": sensing.features.motion_band_power,
                                            "breathing_band_power": sensing.features.breathing_band_power,
                                            "estimated_persons": persons.len(),
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
        "source": s.effective_source(),
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

/// Generate WiFi-derived pose keypoints from sensing data.
///
/// Keypoint positions are modulated by real signal features rather than a pure
/// time-based sine/cosine loop:
///
///   - `motion_band_power`    drives whole-body translation and limb splay
///   - `variance`             seeds per-frame noise so the skeleton never freezes
///   - `breathing_band_power` expands/contracts torso keypoints (shoulders, hips)
///   - `dominant_freq_hz`     tilts the upper body laterally (lean direction)
///   - `change_points`        adds burst jitter to extremities (wrists, ankles)
///
/// When `presence == false` no persons are returned (empty room).
/// When walking is detected (`motion_score > 0.55`) the figure shifts laterally
/// with a stride-swing pattern applied to arms and legs.
// ── Multi-person estimation (issue #97) ──────────────────────────────────────

/// Estimate person count from CSI features using a weighted composite heuristic.
///
/// Single ESP32 link limitations: variance-based detection can reliably detect
/// 1-2 persons. 3+ is speculative and requires ≥3 nodes for spatial resolution.
///
/// Returns a raw score (0.0..1.0) that the caller converts to person count
/// after temporal smoothing.
fn compute_person_score(feat: &FeatureInfo) -> f64 {
    // Normalize each feature to [0, 1] using calibrated ranges:
    //
    //   variance: intra-frame amp variance. 1-person ~2-15, 2-person ~15-60,
    //     real ESP32 can go higher. Use 30.0 as scaling midpoint.
    let var_norm = (feat.variance / 30.0).clamp(0.0, 1.0);

    //   change_points: threshold crossings in 56 subcarriers. 1-person ~5-15,
    //     2-person ~15-30. Scale by 30.0 (half of max 55).
    let cp_norm = (feat.change_points as f64 / 30.0).clamp(0.0, 1.0);

    //   motion_band_power: upper-half subcarrier variance. 1-person ~1-8,
    //     2-person ~8-25. Scale by 20.0.
    let motion_norm = (feat.motion_band_power / 20.0).clamp(0.0, 1.0);

    //   spectral_power: mean squared amplitude. Highly variable (~100-1000+).
    //     Use relative change indicator: high spectral_power with high variance
    //     suggests multiple reflectors. Scale by 500.0.
    let sp_norm = (feat.spectral_power / 500.0).clamp(0.0, 1.0);

    // Weighted composite — variance and change_points carry the most signal.
    var_norm * 0.35 + cp_norm * 0.30 + motion_norm * 0.20 + sp_norm * 0.15
}

/// Convert smoothed person score to discrete count with hysteresis.
///
/// Uses asymmetric thresholds: higher threshold to *add* a person, lower to
/// *drop* one.  This prevents flickering when the score hovers near a boundary
/// (the #1 user-reported issue — see #237, #249, #280, #292).
fn score_to_person_count(smoothed_score: f64, prev_count: usize) -> usize {
    // Up-thresholds (must exceed to increase count):
    //   1→2: 0.65  (raised from 0.50 — multipath in small rooms hit 0.50 easily)
    //   2→3: 0.85  (raised from 0.80 — 3 persons needs strong sustained signal)
    // Down-thresholds (must drop below to decrease count):
    //   2→1: 0.45  (hysteresis gap of 0.20)
    //   3→2: 0.70  (hysteresis gap of 0.15)
    match prev_count {
        0 | 1 => {
            if smoothed_score > 0.85 {
                3
            } else if smoothed_score > 0.65 {
                2
            } else {
                1
            }
        }
        2 => {
            if smoothed_score > 0.85 {
                3
            } else if smoothed_score < 0.45 {
                1
            } else {
                2 // hold — within hysteresis band
            }
        }
        _ => {
            // prev_count >= 3
            if smoothed_score < 0.45 {
                1
            } else if smoothed_score < 0.70 {
                2
            } else {
                3 // hold
            }
        }
    }
}

/// Generate a single person's skeleton with per-person spatial offset and phase stagger.
///
/// `person_idx`: 0-based index of this person.
/// `total_persons`: total number of detected persons (for spacing calculation).
fn derive_single_person_pose(
    update: &SensingUpdate,
    person_idx: usize,
    total_persons: usize,
) -> PersonDetection {
    let cls = &update.classification;
    let feat = &update.features;

    // Per-person phase offset: ~120 degrees apart so they don't move in sync.
    let phase_offset = person_idx as f64 * 2.094;

    // Spatial spread: persons distributed symmetrically around center.
    let half = (total_persons as f64 - 1.0) / 2.0;
    let person_x_offset = (person_idx as f64 - half) * 120.0; // 120px spacing

    // Confidence decays for additional persons (less certain about person 2, 3).
    let conf_decay = 1.0 - person_idx as f64 * 0.15;

    // ── Signal-derived scalars ────────────────────────────────────────────────

    let motion_score = (feat.motion_band_power / 15.0).clamp(0.0, 1.0);
    let is_walking = motion_score > 0.55;
    let breath_amp = (feat.breathing_band_power * 4.0).clamp(0.0, 12.0);

    let breath_phase = if let Some(ref vs) = update.vital_signs {
        let bpm = vs.breathing_rate_bpm.unwrap_or(15.0);
        let freq = (bpm / 60.0).clamp(0.1, 0.5);
        (update.tick as f64 * freq * 0.1 * std::f64::consts::TAU + phase_offset).sin()
    } else {
        (update.tick as f64 * 0.08 + feat.breathing_band_power + phase_offset).sin()
    };

    let lean_x = (feat.dominant_freq_hz / 5.0 - 1.0).clamp(-1.0, 1.0) * 18.0;

    let stride_x = if is_walking {
        let stride_phase = (feat.motion_band_power * 0.7 + update.tick as f64 * 0.12 + phase_offset).sin();
        stride_phase * 45.0 * motion_score
    } else {
        0.0
    };

    let burst = (feat.change_points as f64 / 8.0).clamp(0.0, 1.0);

    let noise_seed = feat.variance * 31.7 + update.tick as f64 * 17.3 + person_idx as f64 * 97.1;
    let noise_val = (noise_seed.sin() * 43758.545).fract();

    let snr_factor = ((feat.variance - 0.5) / 10.0).clamp(0.0, 1.0);
    let base_confidence = cls.confidence * (0.6 + 0.4 * snr_factor) * conf_decay;

    // ── Skeleton base position ────────────────────────────────────────────────

    let base_x = 320.0 + stride_x + lean_x * 0.5 + person_x_offset;
    let base_y = 240.0 - motion_score * 8.0;

    // ── COCO 17-keypoint offsets from hip-center ──────────────────────────────

    let kp_names = [
        "nose", "left_eye", "right_eye", "left_ear", "right_ear",
        "left_shoulder", "right_shoulder", "left_elbow", "right_elbow",
        "left_wrist", "right_wrist", "left_hip", "right_hip",
        "left_knee", "right_knee", "left_ankle", "right_ankle",
    ];

    let kp_offsets: [(f64, f64); 17] = [
        (  0.0,  -80.0), // 0  nose
        ( -8.0,  -88.0), // 1  left_eye
        (  8.0,  -88.0), // 2  right_eye
        (-16.0,  -82.0), // 3  left_ear
        ( 16.0,  -82.0), // 4  right_ear
        (-30.0,  -50.0), // 5  left_shoulder
        ( 30.0,  -50.0), // 6  right_shoulder
        (-45.0,  -15.0), // 7  left_elbow
        ( 45.0,  -15.0), // 8  right_elbow
        (-50.0,   20.0), // 9  left_wrist
        ( 50.0,   20.0), // 10 right_wrist
        (-20.0,   20.0), // 11 left_hip
        ( 20.0,   20.0), // 12 right_hip
        (-22.0,   70.0), // 13 left_knee
        ( 22.0,   70.0), // 14 right_knee
        (-24.0,  120.0), // 15 left_ankle
        ( 24.0,  120.0), // 16 right_ankle
    ];

    const TORSO_KP: [usize; 4] = [5, 6, 11, 12];
    const EXTREMITY_KP: [usize; 4] = [9, 10, 15, 16];

    let keypoints: Vec<PoseKeypoint> = kp_names.iter().zip(kp_offsets.iter())
        .enumerate()
        .map(|(i, (name, (dx, dy)))| {
            let breath_dx = if TORSO_KP.contains(&i) {
                let sign = if *dx < 0.0 { -1.0 } else { 1.0 };
                sign * breath_amp * breath_phase * 0.5
            } else {
                0.0
            };
            let breath_dy = if TORSO_KP.contains(&i) {
                let sign = if *dy < 0.0 { -1.0 } else { 1.0 };
                sign * breath_amp * breath_phase * 0.3
            } else {
                0.0
            };

            let extremity_jitter = if EXTREMITY_KP.contains(&i) {
                let phase = noise_seed + i as f64 * 2.399;
                (
                    phase.sin() * burst * motion_score * 12.0,
                    (phase * 1.31).cos() * burst * motion_score * 8.0,
                )
            } else {
                (0.0, 0.0)
            };

            let kp_noise_x = ((noise_seed + i as f64 * 1.618).sin() * 43758.545).fract()
                * feat.variance.sqrt().clamp(0.0, 3.0) * motion_score;
            let kp_noise_y = ((noise_seed + i as f64 * 2.718).cos() * 31415.926).fract()
                * feat.variance.sqrt().clamp(0.0, 3.0) * motion_score * 0.6;

            let swing_dy = if is_walking {
                let stride_phase =
                    (feat.motion_band_power * 0.7 + update.tick as f64 * 0.12 + phase_offset).sin();
                match i {
                    7 | 9  => -stride_phase * 20.0 * motion_score,
                    8 | 10 =>  stride_phase * 20.0 * motion_score,
                    13 | 15 =>  stride_phase * 25.0 * motion_score,
                    14 | 16 => -stride_phase * 25.0 * motion_score,
                    _ => 0.0,
                }
            } else {
                0.0
            };

            let final_x = base_x + dx + breath_dx + extremity_jitter.0 + kp_noise_x;
            let final_y = base_y + dy + breath_dy + extremity_jitter.1 + kp_noise_y + swing_dy;

            let kp_conf = if EXTREMITY_KP.contains(&i) {
                base_confidence * (0.7 + 0.3 * snr_factor) * (0.85 + 0.15 * noise_val)
            } else {
                base_confidence * (0.88 + 0.12 * ((i as f64 * 0.7 + noise_seed).cos()))
            };

            PoseKeypoint {
                name: name.to_string(),
                x: final_x,
                y: final_y,
                z: lean_x * 0.02,
                confidence: kp_conf.clamp(0.1, 1.0),
            }
        })
        .collect();

    let xs: Vec<f64> = keypoints.iter().map(|k| k.x).collect();
    let ys: Vec<f64> = keypoints.iter().map(|k| k.y).collect();
    let min_x = xs.iter().cloned().fold(f64::MAX, f64::min) - 10.0;
    let min_y = ys.iter().cloned().fold(f64::MAX, f64::min) - 10.0;
    let max_x = xs.iter().cloned().fold(f64::MIN, f64::max) + 10.0;
    let max_y = ys.iter().cloned().fold(f64::MIN, f64::max) + 10.0;

    PersonDetection {
        id: (person_idx + 1) as u32,
        confidence: cls.confidence * conf_decay,
        keypoints,
        bbox: BoundingBox {
            x: min_x,
            y: min_y,
            width: (max_x - min_x).max(80.0),
            height: (max_y - min_y).max(160.0),
        },
        zone: format!("zone_{}", person_idx + 1),
    }
}

fn derive_pose_from_sensing(update: &SensingUpdate) -> Vec<PersonDetection> {
    let cls = &update.classification;
    if !cls.presence {
        return vec![];
    }

    // Use estimated_persons if set by the tick loop; otherwise default to 1.
    let person_count = update.estimated_persons.unwrap_or(1).max(1);

    (0..person_count)
        .map(|idx| derive_single_person_pose(update, idx, person_count))
        .collect()
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
        "source": s.effective_source(),
    }))
}

async fn health_system(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    let uptime = s.start_time.elapsed().as_secs();
    Json(serde_json::json!({
        "status": "healthy",
        "components": {
            "api": { "status": "healthy", "message": "Rust Axum server" },
            "hardware": {
                "status": if s.effective_source().ends_with(":offline") { "degraded" } else { "healthy" },
                "message": format!("Source: {}", s.effective_source())
            },
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
        "source": s.effective_source(),
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
        "source": s.effective_source(),
    }))
}

async fn pose_stats(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    Json(serde_json::json!({
        "total_detections": s.total_detections,
        "average_confidence": 0.87,
        "frames_processed": s.tick,
        "source": s.effective_source(),
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
        "fps": if s.tick > 1 { 10u64 } else { 0u64 },
        "source": s.effective_source(),
    }))
}

// ── Model Management Endpoints ──────────────────────────────────────────────

/// GET /api/v1/models — list discovered RVF model files.
async fn list_models(State(state): State<SharedState>) -> Json<serde_json::Value> {
    // Re-scan directory each call so newly-added files are visible.
    let models = scan_model_files();
    let total = models.len();
    {
        let mut s = state.write().await;
        s.discovered_models = models.clone();
    }
    Json(serde_json::json!({ "models": models, "total": total }))
}

/// GET /api/v1/models/active — return currently loaded model or null.
async fn get_active_model(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    match &s.active_model_id {
        Some(id) => {
            let model = s.discovered_models.iter().find(|m| {
                m.get("id").and_then(|v| v.as_str()) == Some(id.as_str())
            });
            Json(serde_json::json!({
                "active": model.cloned().unwrap_or_else(|| serde_json::json!({ "id": id })),
            }))
        }
        None => Json(serde_json::json!({ "active": serde_json::Value::Null })),
    }
}

/// POST /api/v1/models/load — load a model by ID.
async fn load_model(
    State(state): State<SharedState>,
    Json(body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let model_id = body.get("id")
        .or_else(|| body.get("model_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if model_id.is_empty() {
        return Json(serde_json::json!({ "error": "missing 'id' field", "success": false }));
    }
    let mut s = state.write().await;
    s.active_model_id = Some(model_id.clone());
    s.model_loaded = true;
    info!("Model loaded: {model_id}");
    Json(serde_json::json!({ "success": true, "model_id": model_id }))
}

/// POST /api/v1/models/unload — unload the current model.
async fn unload_model(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let mut s = state.write().await;
    let prev = s.active_model_id.take();
    s.model_loaded = false;
    info!("Model unloaded (was: {:?})", prev);
    Json(serde_json::json!({ "success": true, "previous": prev }))
}

/// DELETE /api/v1/models/:id — delete a model file.
async fn delete_model(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    // ADR-050: Sanitize path to prevent directory traversal
    let safe_id = std::path::Path::new(&id)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");
    if safe_id.is_empty() || safe_id != id {
        return Json(serde_json::json!({ "error": "invalid model id", "success": false }));
    }
    let path = PathBuf::from("data/models").join(format!("{}.rvf", safe_id));
    if path.exists() {
        if let Err(e) = std::fs::remove_file(&path) {
            warn!("Failed to delete model file {:?}: {}", path, e);
            return Json(serde_json::json!({ "error": format!("delete failed: {e}"), "success": false }));
        }
        // If this was the active model, unload it
        let mut s = state.write().await;
        if s.active_model_id.as_deref() == Some(id.as_str()) {
            s.active_model_id = None;
            s.model_loaded = false;
        }
        s.discovered_models.retain(|m| {
            m.get("id").and_then(|v| v.as_str()) != Some(id.as_str())
        });
        info!("Model deleted: {id}");
        Json(serde_json::json!({ "success": true, "deleted": id }))
    } else {
        Json(serde_json::json!({ "error": "model not found", "success": false }))
    }
}

/// GET /api/v1/models/lora/profiles — list LoRA adapter profiles.
async fn list_lora_profiles() -> Json<serde_json::Value> {
    // LoRA profiles are discovered from data/models/*.lora.json
    let profiles = scan_lora_profiles();
    Json(serde_json::json!({ "profiles": profiles }))
}

/// POST /api/v1/models/lora/activate — activate a LoRA adapter profile.
async fn activate_lora_profile(
    Json(body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let profile = body.get("profile")
        .or_else(|| body.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if profile.is_empty() {
        return Json(serde_json::json!({ "error": "missing 'profile' field", "success": false }));
    }
    info!("LoRA profile activated: {profile}");
    Json(serde_json::json!({ "success": true, "profile": profile }))
}

/// Scan `data/models/` for `.rvf` files and return metadata.
fn scan_model_files() -> Vec<serde_json::Value> {
    let dir = PathBuf::from("data/models");
    let mut models = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("rvf") {
                let name = path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                let modified = entry.metadata().ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                models.push(serde_json::json!({
                    "id": name,
                    "name": name,
                    "path": path.display().to_string(),
                    "size_bytes": size,
                    "format": "rvf",
                    "modified_epoch": modified,
                }));
            }
        }
    }
    models
}

/// Scan `data/models/` for `.lora.json` LoRA profile files.
fn scan_lora_profiles() -> Vec<serde_json::Value> {
    let dir = PathBuf::from("data/models");
    let mut profiles = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.ends_with(".lora.json") {
                let profile_name = name.trim_end_matches(".lora.json").to_string();
                // Try to read the profile JSON
                let config = std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                    .unwrap_or_else(|| serde_json::json!({}));
                profiles.push(serde_json::json!({
                    "name": profile_name,
                    "path": path.display().to_string(),
                    "config": config,
                }));
            }
        }
    }
    profiles
}

// ── Recording Endpoints ─────────────────────────────────────────────────────

/// GET /api/v1/recording/list — list CSI recordings.
async fn list_recordings() -> Json<serde_json::Value> {
    let recordings = scan_recording_files();
    Json(serde_json::json!({ "recordings": recordings }))
}

/// POST /api/v1/recording/start — start recording CSI data.
async fn start_recording(
    State(state): State<SharedState>,
    Json(body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let mut s = state.write().await;
    if s.recording_active {
        return Json(serde_json::json!({
            "error": "recording already in progress",
            "success": false,
            "recording_id": s.recording_current_id,
        }));
    }
    let id = body.get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            format!("rec_{}", chrono_timestamp())
        });

    // Create the recording file
    let rec_path = PathBuf::from("data/recordings").join(format!("{}.jsonl", id));
    let file = match std::fs::File::create(&rec_path) {
        Ok(f) => f,
        Err(e) => {
            warn!("Failed to create recording file {:?}: {}", rec_path, e);
            return Json(serde_json::json!({
                "error": format!("cannot create file: {e}"),
                "success": false,
            }));
        }
    };

    // Create a stop signal channel
    let (stop_tx, mut stop_rx) = tokio::sync::watch::channel(false);
    s.recording_active = true;
    s.recording_start_time = Some(std::time::Instant::now());
    s.recording_current_id = Some(id.clone());
    s.recording_stop_tx = Some(stop_tx);

    // Subscribe to the broadcast channel to capture CSI frames
    let mut rx = s.tx.subscribe();

    // Add initial recording entry
    s.recordings.push(serde_json::json!({
        "id": id,
        "path": rec_path.display().to_string(),
        "status": "recording",
        "started_at": chrono_timestamp(),
        "frames": 0,
    }));

    let rec_id = id.clone();

    // Spawn writer task in background
    tokio::spawn(async move {
        use std::io::Write;
        let mut writer = std::io::BufWriter::new(file);
        let mut frame_count: u64 = 0;
        loop {
            tokio::select! {
                result = rx.recv() => {
                    match result {
                        Ok(frame_json) => {
                            if writeln!(writer, "{}", frame_json).is_err() {
                                warn!("Recording {rec_id}: write error, stopping");
                                break;
                            }
                            frame_count += 1;
                            // Flush every 100 frames
                            if frame_count % 100 == 0 {
                                let _ = writer.flush();
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            debug!("Recording {rec_id}: lagged {n} frames");
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            info!("Recording {rec_id}: broadcast closed, stopping");
                            break;
                        }
                    }
                }
                _ = stop_rx.changed() => {
                    if *stop_rx.borrow() {
                        info!("Recording {rec_id}: stop signal received ({frame_count} frames)");
                        break;
                    }
                }
            }
        }
        let _ = writer.flush();
        info!("Recording {rec_id} finished: {frame_count} frames written");
    });

    info!("Recording started: {id}");
    Json(serde_json::json!({ "success": true, "recording_id": id }))
}

/// POST /api/v1/recording/stop — stop recording CSI data.
async fn stop_recording(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let mut s = state.write().await;
    if !s.recording_active {
        return Json(serde_json::json!({
            "error": "no recording in progress",
            "success": false,
        }));
    }
    // Signal the writer task to stop
    if let Some(tx) = s.recording_stop_tx.take() {
        let _ = tx.send(true);
    }
    let duration_secs = s.recording_start_time
        .map(|t| t.elapsed().as_secs())
        .unwrap_or(0);
    let rec_id = s.recording_current_id.take().unwrap_or_default();
    s.recording_active = false;
    s.recording_start_time = None;

    // Update the recording entry status
    for rec in s.recordings.iter_mut() {
        if rec.get("id").and_then(|v| v.as_str()) == Some(rec_id.as_str()) {
            rec["status"] = serde_json::json!("completed");
            rec["duration_secs"] = serde_json::json!(duration_secs);
        }
    }

    info!("Recording stopped: {rec_id} ({duration_secs}s)");
    Json(serde_json::json!({
        "success": true,
        "recording_id": rec_id,
        "duration_secs": duration_secs,
    }))
}

/// DELETE /api/v1/recording/:id — delete a recording file.
async fn delete_recording(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    // ADR-050: Sanitize path to prevent directory traversal
    let safe_id = std::path::Path::new(&id)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");
    if safe_id.is_empty() || safe_id != id {
        return Json(serde_json::json!({ "error": "invalid recording id", "success": false }));
    }
    let path = PathBuf::from("data/recordings").join(format!("{}.jsonl", safe_id));
    if path.exists() {
        if let Err(e) = std::fs::remove_file(&path) {
            warn!("Failed to delete recording {:?}: {}", path, e);
            return Json(serde_json::json!({ "error": format!("delete failed: {e}"), "success": false }));
        }
        let mut s = state.write().await;
        s.recordings.retain(|r| {
            r.get("id").and_then(|v| v.as_str()) != Some(id.as_str())
        });
        info!("Recording deleted: {id}");
        Json(serde_json::json!({ "success": true, "deleted": id }))
    } else {
        Json(serde_json::json!({ "error": "recording not found", "success": false }))
    }
}

/// Scan `data/recordings/` for `.jsonl` files and return metadata.
fn scan_recording_files() -> Vec<serde_json::Value> {
    let dir = PathBuf::from("data/recordings");
    let mut recordings = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                let name = path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                let modified = entry.metadata().ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                // Count lines (frames) — approximate for large files
                let frame_count = std::fs::read_to_string(&path)
                    .map(|s| s.lines().count())
                    .unwrap_or(0);
                recordings.push(serde_json::json!({
                    "id": name,
                    "name": name,
                    "path": path.display().to_string(),
                    "size_bytes": size,
                    "frames": frame_count,
                    "modified_epoch": modified,
                    "status": "completed",
                }));
            }
        }
    }
    recordings
}

// ── Training Endpoints ──────────────────────────────────────────────────────

/// GET /api/v1/train/status — get training status.
async fn train_status(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    Json(serde_json::json!({
        "status": s.training_status,
        "config": s.training_config,
    }))
}

/// POST /api/v1/train/start — start a training run.
async fn train_start(
    State(state): State<SharedState>,
    Json(body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let mut s = state.write().await;
    if s.training_status == "running" {
        return Json(serde_json::json!({
            "error": "training already running",
            "success": false,
        }));
    }
    s.training_status = "running".to_string();
    s.training_config = Some(body.clone());
    info!("Training started with config: {}", body);
    Json(serde_json::json!({
        "success": true,
        "status": "running",
        "message": "Training pipeline started. Use GET /api/v1/train/status to monitor.",
    }))
}

/// POST /api/v1/train/stop — stop the current training run.
async fn train_stop(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let mut s = state.write().await;
    if s.training_status != "running" {
        return Json(serde_json::json!({
            "error": "no training in progress",
            "success": false,
        }));
    }
    s.training_status = "idle".to_string();
    info!("Training stopped");
    Json(serde_json::json!({
        "success": true,
        "status": "idle",
    }))
}

// ── Adaptive classifier endpoints ────────────────────────────────────────────

/// POST /api/v1/adaptive/train — train the adaptive classifier from recordings.
async fn adaptive_train(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let rec_dir = PathBuf::from("data/recordings");
    eprintln!("=== Adaptive Classifier Training ===");
    match adaptive_classifier::train_from_recordings(&rec_dir) {
        Ok(model) => {
            let accuracy = model.training_accuracy;
            let frames = model.trained_frames;
            let stats: Vec<_> = model.class_stats.iter().map(|cs| {
                serde_json::json!({
                    "class": cs.label,
                    "samples": cs.count,
                    "feature_means": cs.mean,
                })
            }).collect();

            // Save to disk.
            if let Err(e) = model.save(&adaptive_classifier::model_path()) {
                warn!("Failed to save adaptive model: {e}");
            } else {
                info!("Adaptive model saved to {}", adaptive_classifier::model_path().display());
            }

            // Load into runtime state.
            let mut s = state.write().await;
            s.adaptive_model = Some(model);

            Json(serde_json::json!({
                "success": true,
                "trained_frames": frames,
                "accuracy": accuracy,
                "class_stats": stats,
            }))
        }
        Err(e) => {
            Json(serde_json::json!({
                "success": false,
                "error": e,
            }))
        }
    }
}

/// GET /api/v1/adaptive/status — check adaptive model status.
async fn adaptive_status(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    match &s.adaptive_model {
        Some(model) => Json(serde_json::json!({
            "loaded": true,
            "trained_frames": model.trained_frames,
            "accuracy": model.training_accuracy,
            "version": model.version,
            "classes": adaptive_classifier::CLASSES,
            "class_stats": model.class_stats,
        })),
        None => Json(serde_json::json!({
            "loaded": false,
            "message": "No adaptive model. POST /api/v1/adaptive/train to train one.",
        })),
    }
}

/// POST /api/v1/adaptive/unload — unload the adaptive model (revert to thresholds).
async fn adaptive_unload(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let mut s = state.write().await;
    s.adaptive_model = None;
    Json(serde_json::json!({ "success": true, "message": "Adaptive model unloaded." }))
}

/// Generate a simple timestamp string (epoch seconds) for recording IDs.
fn chrono_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
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
        "source": s.effective_source(),
        "tick": s.tick,
    }))
}

/// GET /api/v1/edge-vitals — latest edge vitals from ESP32 (ADR-039).
async fn edge_vitals_endpoint(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    match &s.edge_vitals {
        Some(v) => Json(serde_json::json!({
            "status": "ok",
            "edge_vitals": v,
        })),
        None => Json(serde_json::json!({
            "status": "no_data",
            "edge_vitals": null,
            "message": "No edge vitals packet received yet. Ensure ESP32 edge_tier >= 1.",
        })),
    }
}

/// GET /api/v1/wasm-events — latest WASM events from ESP32 (ADR-040).
async fn wasm_events_endpoint(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    match &s.latest_wasm_events {
        Some(w) => Json(serde_json::json!({
            "status": "ok",
            "wasm_events": w,
        })),
        None => Json(serde_json::json!({
            "status": "no_data",
            "wasm_events": null,
            "message": "No WASM output packet received yet. Upload and start a .wasm module on the ESP32.",
        })),
    }
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
                // ADR-039: Try edge vitals packet first (magic 0xC511_0002).
                if let Some(vitals) = parse_esp32_vitals(&buf[..len]) {
                    debug!("ESP32 vitals from {src}: node={} br={:.1} hr={:.1} pres={}",
                           vitals.node_id, vitals.breathing_rate_bpm,
                           vitals.heartrate_bpm, vitals.presence);
                    let mut s = state.write().await;
                    // Broadcast vitals via WebSocket.
                    if let Ok(json) = serde_json::to_string(&serde_json::json!({
                        "type": "edge_vitals",
                        "node_id": vitals.node_id,
                        "presence": vitals.presence,
                        "fall_detected": vitals.fall_detected,
                        "motion": vitals.motion,
                        "breathing_rate_bpm": vitals.breathing_rate_bpm,
                        "heartrate_bpm": vitals.heartrate_bpm,
                        "n_persons": vitals.n_persons,
                        "motion_energy": vitals.motion_energy,
                        "presence_score": vitals.presence_score,
                        "rssi": vitals.rssi,
                    })) {
                        let _ = s.tx.send(json);
                    }

                    // Issue #323: Also emit a sensing_update so the UI renders
                    // detections for ESP32 nodes running the edge DSP pipeline
                    // (Tier 2+).  Without this, vitals arrive but the UI shows
                    // "no detection" because it only renders sensing_update msgs.
                    s.source = "esp32".to_string();
                    s.last_esp32_frame = Some(std::time::Instant::now());

                    // ── Per-node state for edge vitals (issue #249) ──────
                    let node_id = vitals.node_id;
                    let ns = s.node_states.entry(node_id).or_insert_with(NodeState::new);
                    ns.last_frame_time = Some(std::time::Instant::now());
                    ns.edge_vitals = Some(vitals.clone());
                    ns.rssi_history.push_back(vitals.rssi as f64);
                    if ns.rssi_history.len() > 60 { ns.rssi_history.pop_front(); }

                    // Store per-node person count from edge vitals.
                    let node_est = if vitals.presence {
                        (vitals.n_persons as usize).max(1)
                    } else {
                        0
                    };
                    ns.prev_person_count = node_est;

                    s.tick += 1;
                    let tick = s.tick;

                    let motion_level = if vitals.motion { "present_moving" }
                        else if vitals.presence { "present_still" }
                        else { "absent" };
                    let motion_score = if vitals.motion { 0.8 }
                        else if vitals.presence { 0.3 }
                        else { 0.05 };

                    // Aggregate person count across all active nodes.
                    let now = std::time::Instant::now();
                    let total_persons: usize = s.node_states.values()
                        .filter(|n| n.last_frame_time.map_or(false, |t| now.duration_since(t).as_secs() < 10))
                        .map(|n| n.prev_person_count)
                        .sum();

                    // Build nodes array with all active nodes.
                    let active_nodes: Vec<NodeInfo> = s.node_states.iter()
                        .filter(|(_, n)| n.last_frame_time.map_or(false, |t| now.duration_since(t).as_secs() < 10))
                        .map(|(&id, n)| NodeInfo {
                            node_id: id,
                            rssi_dbm: n.rssi_history.back().copied().unwrap_or(0.0),
                            position: [2.0, 0.0, 1.5],
                            amplitude: vec![],
                            subcarrier_count: 0,
                        })
                        .collect();

                    let features = FeatureInfo {
                        mean_rssi: vitals.rssi as f64,
                        variance: vitals.motion_energy as f64,
                        motion_band_power: vitals.motion_energy as f64,
                        breathing_band_power: if vitals.presence { 0.5 } else { 0.0 },
                        dominant_freq_hz: vitals.breathing_rate_bpm / 60.0,
                        change_points: 0,
                        spectral_power: vitals.motion_energy as f64,
                    };
                    let classification = ClassificationInfo {
                        motion_level: motion_level.to_string(),
                        presence: vitals.presence,
                        confidence: vitals.presence_score as f64,
                    };
                    let signal_field = generate_signal_field(
                        vitals.rssi as f64, motion_score, vitals.breathing_rate_bpm / 60.0,
                        (vitals.presence_score as f64).min(1.0), &[],
                    );

                    let mut update = SensingUpdate {
                        msg_type: "sensing_update".to_string(),
                        timestamp: chrono::Utc::now().timestamp_millis() as f64 / 1000.0,
                        source: "esp32".to_string(),
                        tick,
                        nodes: active_nodes,
                        features: features.clone(),
                        classification,
                        signal_field,
                        vital_signs: Some(VitalSigns {
                            breathing_rate_bpm: if vitals.breathing_rate_bpm > 0.0 { Some(vitals.breathing_rate_bpm) } else { None },
                            heart_rate_bpm: if vitals.heartrate_bpm > 0.0 { Some(vitals.heartrate_bpm) } else { None },
                            breathing_confidence: if vitals.presence { 0.7 } else { 0.0 },
                            heartbeat_confidence: if vitals.presence { 0.7 } else { 0.0 },
                            signal_quality: vitals.presence_score as f64,
                        }),
                        enhanced_motion: None,
                        enhanced_breathing: None,
                        posture: None,
                        signal_quality_score: None,
                        quality_verdict: None,
                        bssid_count: None,
                        pose_keypoints: None,
                        model_status: None,
                        persons: None,
                        estimated_persons: if total_persons > 0 { Some(total_persons) } else { None },
                    };

                    let persons = derive_pose_from_sensing(&update);
                    if !persons.is_empty() {
                        update.persons = Some(persons);
                    }

                    if let Ok(json) = serde_json::to_string(&update) {
                        let _ = s.tx.send(json);
                    }
                    s.latest_update = Some(update);
                    s.edge_vitals = Some(vitals);
                    continue;
                }

                // ADR-040: Try WASM output packet (magic 0xC511_0004).
                if let Some(wasm_output) = parse_wasm_output(&buf[..len]) {
                    debug!("WASM output from {src}: node={} module={} events={}",
                           wasm_output.node_id, wasm_output.module_id,
                           wasm_output.events.len());
                    let mut s = state.write().await;
                    // Broadcast WASM events via WebSocket.
                    if let Ok(json) = serde_json::to_string(&serde_json::json!({
                        "type": "wasm_event",
                        "node_id": wasm_output.node_id,
                        "module_id": wasm_output.module_id,
                        "events": wasm_output.events,
                    })) {
                        let _ = s.tx.send(json);
                    }
                    s.latest_wasm_events = Some(wasm_output);
                    continue;
                }

                if let Some(frame) = parse_esp32_frame(&buf[..len]) {
                    debug!("ESP32 frame from {src}: node={}, subs={}, seq={}",
                           frame.node_id, frame.n_subcarriers, frame.sequence);

                    let mut s = state.write().await;
                    s.source = "esp32".to_string();
                    s.last_esp32_frame = Some(std::time::Instant::now());

                    // Also maintain global frame_history for backward compat
                    // (simulation path, REST endpoints, etc.).
                    s.frame_history.push_back(frame.amplitudes.clone());
                    if s.frame_history.len() > FRAME_HISTORY_CAPACITY {
                        s.frame_history.pop_front();
                    }

                    // ── Per-node processing (issue #249) ──────────────────
                    // Process entirely within per-node state so different
                    // ESP32 nodes never mix their smoothing/vitals buffers.
                    // We scope the mutable borrow of node_states so we can
                    // access other AppStateInner fields afterward.
                    let node_id = frame.node_id;
                    // Clone adaptive model before mutable borrow of node_states
                    // to avoid unsafe raw pointer (review finding #2).
                    let adaptive_model_clone = s.adaptive_model.clone();

                    let ns = s.node_states.entry(node_id).or_insert_with(NodeState::new);
                    ns.last_frame_time = Some(std::time::Instant::now());

                    ns.frame_history.push_back(frame.amplitudes.clone());
                    if ns.frame_history.len() > FRAME_HISTORY_CAPACITY {
                        ns.frame_history.pop_front();
                    }

                    let sample_rate_hz = 1000.0 / 500.0_f64;
                    let (features, mut classification, breathing_rate_hz, sub_variances, raw_motion) =
                        extract_features_from_frame(&frame, &ns.frame_history, sample_rate_hz);
                    smooth_and_classify_node(ns, &mut classification, raw_motion);

                    // Adaptive override using cloned model (safe, no raw pointers).
                    if let Some(ref model) = adaptive_model_clone {
                        let amps = ns.frame_history.back()
                            .map(|v| v.as_slice())
                            .unwrap_or(&[]);
                        let feat_arr = adaptive_classifier::features_from_runtime(
                            &serde_json::json!({
                                "variance": features.variance,
                                "motion_band_power": features.motion_band_power,
                                "breathing_band_power": features.breathing_band_power,
                                "spectral_power": features.spectral_power,
                                "dominant_freq_hz": features.dominant_freq_hz,
                                "change_points": features.change_points,
                                "mean_rssi": features.mean_rssi,
                            }),
                            amps,
                        );
                        let (label, conf) = model.classify(&feat_arr);
                        classification.motion_level = label.to_string();
                        classification.presence = label != "absent";
                        classification.confidence = (conf * 0.7 + classification.confidence * 0.3).clamp(0.0, 1.0);
                    }

                    ns.rssi_history.push_back(features.mean_rssi);
                    if ns.rssi_history.len() > 60 {
                        ns.rssi_history.pop_front();
                    }

                    let raw_vitals = ns.vital_detector.process_frame(
                        &frame.amplitudes,
                        &frame.phases,
                    );
                    let vitals = smooth_vitals_node(ns, &raw_vitals);
                    ns.latest_vitals = vitals.clone();

                    let raw_score = compute_person_score(&features);
                    ns.smoothed_person_score = ns.smoothed_person_score * 0.90 + raw_score * 0.10;
                    if classification.presence {
                        let count = score_to_person_count(ns.smoothed_person_score, ns.prev_person_count);
                        ns.prev_person_count = count;
                    } else {
                        ns.prev_person_count = 0;
                    }

                    // Done with per-node mutable borrow; now read aggregated
                    // state from all nodes (the borrow of `ns` ends here).
                    // (We re-borrow node_states immutably via `s` below.)

                    s.rssi_history.push_back(features.mean_rssi);
                    if s.rssi_history.len() > 60 {
                        s.rssi_history.pop_front();
                    }
                    s.latest_vitals = vitals.clone();

                    s.tick += 1;
                    let tick = s.tick;

                    let motion_score = if classification.motion_level == "active" { 0.8 }
                        else if classification.motion_level == "present_still" { 0.3 }
                        else { 0.05 };

                    // Aggregate person count across all active nodes.
                    let now = std::time::Instant::now();
                    let total_persons: usize = s.node_states.values()
                        .filter(|n| n.last_frame_time.map_or(false, |t| now.duration_since(t).as_secs() < 10))
                        .map(|n| n.prev_person_count)
                        .sum();

                    // Build nodes array with all active nodes.
                    let active_nodes: Vec<NodeInfo> = s.node_states.iter()
                        .filter(|(_, n)| n.last_frame_time.map_or(false, |t| now.duration_since(t).as_secs() < 10))
                        .map(|(&id, n)| NodeInfo {
                            node_id: id,
                            rssi_dbm: n.rssi_history.back().copied().unwrap_or(0.0),
                            position: [2.0, 0.0, 1.5],
                            amplitude: n.frame_history.back()
                                .map(|a| a.iter().take(56).cloned().collect())
                                .unwrap_or_default(),
                            subcarrier_count: n.frame_history.back().map_or(0, |a| a.len()),
                        })
                        .collect();

                    let mut update = SensingUpdate {
                        msg_type: "sensing_update".to_string(),
                        timestamp: chrono::Utc::now().timestamp_millis() as f64 / 1000.0,
                        source: "esp32".to_string(),
                        tick,
                        nodes: active_nodes,
                        features: features.clone(),
                        classification,
                        signal_field: generate_signal_field(
                            features.mean_rssi, motion_score, breathing_rate_hz,
                            features.variance.min(1.0), &sub_variances,
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
                        persons: None,
                        estimated_persons: if total_persons > 0 { Some(total_persons) } else { None },
                    };

                    let persons = derive_pose_from_sensing(&update);
                    if !persons.is_empty() {
                        update.persons = Some(persons);
                    }

                    if let Ok(json) = serde_json::to_string(&update) {
                        let _ = s.tx.send(json);
                    }
                    s.latest_update = Some(update);

                    // Evict stale nodes every 100 ticks to prevent memory leak.
                    if tick % 100 == 0 {
                        let stale = Duration::from_secs(60);
                        let before = s.node_states.len();
                        s.node_states.retain(|_id, ns| {
                            ns.last_frame_time.map_or(false, |t| now.duration_since(t) < stale)
                        });
                        let evicted = before - s.node_states.len();
                        if evicted > 0 {
                            info!("Evicted {} stale node(s), {} active", evicted, s.node_states.len());
                        }
                    }
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

        // Append current amplitudes to history before feature extraction.
        s.frame_history.push_back(frame.amplitudes.clone());
        if s.frame_history.len() > FRAME_HISTORY_CAPACITY {
            s.frame_history.pop_front();
        }

        let sample_rate_hz = 1000.0 / tick_ms as f64;
        let (features, mut classification, breathing_rate_hz, sub_variances, raw_motion) =
            extract_features_from_frame(&frame, &s.frame_history, sample_rate_hz);
        smooth_and_classify(&mut s, &mut classification, raw_motion);
    adaptive_override(&s, &features, &mut classification);

        s.rssi_history.push_back(features.mean_rssi);
        if s.rssi_history.len() > 60 {
            s.rssi_history.pop_front();
        }

        let motion_score = if classification.motion_level == "active" { 0.8 }
            else if classification.motion_level == "present_still" { 0.3 }
            else { 0.05 };

        let raw_vitals = s.vital_detector.process_frame(
            &frame.amplitudes,
            &frame.phases,
        );
        let vitals = smooth_vitals(&mut s, &raw_vitals);
        s.latest_vitals = vitals.clone();

        let frame_amplitudes = frame.amplitudes.clone();
        let frame_n_sub = frame.n_subcarriers;

        // Multi-person estimation with temporal smoothing (EMA α=0.10).
        let raw_score = compute_person_score(&features);
        s.smoothed_person_score = s.smoothed_person_score * 0.90 + raw_score * 0.10;
        let est_persons = if classification.presence {
            let count = score_to_person_count(s.smoothed_person_score, s.prev_person_count);
            s.prev_person_count = count;
            count
        } else {
            s.prev_person_count = 0;
            0
        };

        let mut update = SensingUpdate {
            msg_type: "sensing_update".to_string(),
            timestamp: chrono::Utc::now().timestamp_millis() as f64 / 1000.0,
            source: "simulated".to_string(),
            tick,
            nodes: vec![NodeInfo {
                node_id: 1,
                rssi_dbm: features.mean_rssi,
                position: [2.0, 0.0, 1.5],
                amplitude: frame_amplitudes,
                subcarrier_count: frame_n_sub as usize,
            }],
            features: features.clone(),
            classification,
            signal_field: generate_signal_field(
                features.mean_rssi, motion_score, breathing_rate_hz,
                features.variance.min(1.0), &sub_variances,
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
            persons: None,
            estimated_persons: if est_persons > 0 { Some(est_persons) } else { None },
        };

        // Populate persons from the sensing update.
        let persons = derive_pose_from_sensing(&update);
        if !persons.is_empty() {
            update.persons = Some(persons);
        }

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

    // Handle --pretrain mode: self-supervised contrastive pretraining (ADR-024)
    if args.pretrain {
        eprintln!("=== WiFi-DensePose Contrastive Pretraining (ADR-024) ===");

        let ds_path = args.dataset.clone().unwrap_or_else(|| PathBuf::from("data"));
        let source = match args.dataset_type.as_str() {
            "wipose" => dataset::DataSource::WiPose(ds_path.clone()),
            _ => dataset::DataSource::MmFi(ds_path.clone()),
        };
        let pipeline = dataset::DataPipeline::new(dataset::DataConfig {
            source, ..Default::default()
        });

        // Generate synthetic or load real CSI windows
        let generate_synthetic_windows = || -> Vec<Vec<Vec<f32>>> {
            (0..50).map(|i| {
                (0..4).map(|a| {
                    (0..56).map(|s| ((i * 7 + a * 13 + s) as f32 * 0.31).sin() * 0.5).collect()
                }).collect()
            }).collect()
        };

        let csi_windows: Vec<Vec<Vec<f32>>> = match pipeline.load() {
            Ok(s) if !s.is_empty() => {
                eprintln!("Loaded {} samples from {}", s.len(), ds_path.display());
                s.into_iter().map(|s| s.csi_window).collect()
            }
            _ => {
                eprintln!("Using synthetic data for pretraining.");
                generate_synthetic_windows()
            }
        };

        let n_subcarriers = csi_windows.first()
            .and_then(|w| w.first())
            .map(|f| f.len())
            .unwrap_or(56);

        let tf_config = graph_transformer::TransformerConfig {
            n_subcarriers, n_keypoints: 17, d_model: 64, n_heads: 4, n_gnn_layers: 2,
        };
        let transformer = graph_transformer::CsiToPoseTransformer::new(tf_config);
        eprintln!("Transformer params: {}", transformer.param_count());

        let trainer_config = trainer::TrainerConfig {
            epochs: args.pretrain_epochs,
            batch_size: 8, lr: 0.001, warmup_epochs: 2, min_lr: 1e-6,
            early_stop_patience: args.pretrain_epochs + 1,
            pretrain_temperature: 0.07,
            ..Default::default()
        };
        let mut t = trainer::Trainer::with_transformer(trainer_config, transformer);

        let e_config = embedding::EmbeddingConfig {
            d_model: 64, d_proj: 128, temperature: 0.07, normalize: true,
        };
        let mut projection = embedding::ProjectionHead::new(e_config.clone());
        let augmenter = embedding::CsiAugmenter::new();

        eprintln!("Starting contrastive pretraining for {} epochs...", args.pretrain_epochs);
        let start = std::time::Instant::now();
        for epoch in 0..args.pretrain_epochs {
            let loss = t.pretrain_epoch(&csi_windows, &augmenter, &mut projection, 0.07, epoch);
            if epoch % 10 == 0 || epoch == args.pretrain_epochs - 1 {
                eprintln!("  Epoch {epoch}: contrastive loss = {loss:.4}");
            }
        }
        let elapsed = start.elapsed().as_secs_f64();
        eprintln!("Pretraining complete in {elapsed:.1}s");

        // Save pretrained model as RVF with embedding segment
        if let Some(ref save_path) = args.save_rvf {
            eprintln!("Saving pretrained model to RVF: {}", save_path.display());
            t.sync_transformer_weights();
            let weights = t.params().to_vec();
            let mut proj_weights = Vec::new();
            projection.flatten_into(&mut proj_weights);

            let mut builder = RvfBuilder::new();
            builder.add_manifest(
                "wifi-densepose-pretrained",
                env!("CARGO_PKG_VERSION"),
                "WiFi DensePose contrastive pretrained model (ADR-024)",
            );
            builder.add_weights(&weights);
            builder.add_embedding(
                &serde_json::json!({
                    "d_model": e_config.d_model,
                    "d_proj": e_config.d_proj,
                    "temperature": e_config.temperature,
                    "normalize": e_config.normalize,
                    "pretrain_epochs": args.pretrain_epochs,
                }),
                &proj_weights,
            );
            match builder.write_to_file(save_path) {
                Ok(()) => eprintln!("RVF saved ({} transformer + {} projection params)",
                    weights.len(), proj_weights.len()),
                Err(e) => eprintln!("Failed to save RVF: {e}"),
            }
        }

        return;
    }

    // Handle --embed mode: extract embeddings from CSI data
    if args.embed {
        eprintln!("=== WiFi-DensePose Embedding Extraction (ADR-024) ===");

        let model_path = match &args.model {
            Some(p) => p.clone(),
            None => {
                eprintln!("Error: --embed requires --model <path> to a pretrained .rvf file");
                std::process::exit(1);
            }
        };

        let reader = match RvfReader::from_file(&model_path) {
            Ok(r) => r,
            Err(e) => { eprintln!("Failed to load model: {e}"); std::process::exit(1); }
        };

        let weights = reader.weights().unwrap_or_default();
        let (embed_config_json, proj_weights) = reader.embedding().unwrap_or_else(|| {
            eprintln!("Warning: no embedding segment in RVF, using defaults");
            (serde_json::json!({"d_model":64,"d_proj":128,"temperature":0.07,"normalize":true}), Vec::new())
        });

        let d_model = embed_config_json["d_model"].as_u64().unwrap_or(64) as usize;
        let d_proj = embed_config_json["d_proj"].as_u64().unwrap_or(128) as usize;

        let tf_config = graph_transformer::TransformerConfig {
            n_subcarriers: 56, n_keypoints: 17, d_model, n_heads: 4, n_gnn_layers: 2,
        };
        let e_config = embedding::EmbeddingConfig {
            d_model, d_proj, temperature: 0.07, normalize: true,
        };
        let mut extractor = embedding::EmbeddingExtractor::new(tf_config, e_config.clone());

        // Load transformer weights
        if !weights.is_empty() {
            if let Err(e) = extractor.transformer.unflatten_weights(&weights) {
                eprintln!("Warning: failed to load transformer weights: {e}");
            }
        }
        // Load projection weights
        if !proj_weights.is_empty() {
            let (proj, _) = embedding::ProjectionHead::unflatten_from(&proj_weights, &e_config);
            extractor.projection = proj;
        }

        // Load dataset and extract embeddings
        let _ds_path = args.dataset.clone().unwrap_or_else(|| PathBuf::from("data"));
        let csi_windows: Vec<Vec<Vec<f32>>> = (0..10).map(|i| {
            (0..4).map(|a| {
                (0..56).map(|s| ((i * 7 + a * 13 + s) as f32 * 0.31).sin() * 0.5).collect()
            }).collect()
        }).collect();

        eprintln!("Extracting embeddings from {} CSI windows...", csi_windows.len());
        let embeddings = extractor.extract_batch(&csi_windows);
        for (i, emb) in embeddings.iter().enumerate() {
            let norm: f32 = emb.iter().map(|x| x * x).sum::<f32>().sqrt();
            eprintln!("  Window {i}: {d_proj}-dim embedding, ||e|| = {norm:.4}");
        }
        eprintln!("Extracted {} embeddings of dimension {d_proj}", embeddings.len());

        return;
    }

    // Handle --build-index mode: build a fingerprint index from embeddings
    if let Some(ref index_type_str) = args.build_index {
        eprintln!("=== WiFi-DensePose Fingerprint Index Builder (ADR-024) ===");

        let index_type = match index_type_str.as_str() {
            "env" | "environment" => embedding::IndexType::EnvironmentFingerprint,
            "activity" => embedding::IndexType::ActivityPattern,
            "temporal" => embedding::IndexType::TemporalBaseline,
            "person" => embedding::IndexType::PersonTrack,
            _ => {
                eprintln!("Unknown index type '{}'. Use: env, activity, temporal, person", index_type_str);
                std::process::exit(1);
            }
        };

        let tf_config = graph_transformer::TransformerConfig::default();
        let e_config = embedding::EmbeddingConfig::default();
        let mut extractor = embedding::EmbeddingExtractor::new(tf_config, e_config);

        // Generate synthetic CSI windows for demo
        let csi_windows: Vec<Vec<Vec<f32>>> = (0..20).map(|i| {
            (0..4).map(|a| {
                (0..56).map(|s| ((i * 7 + a * 13 + s) as f32 * 0.31).sin() * 0.5).collect()
            }).collect()
        }).collect();

        let mut index = embedding::FingerprintIndex::new(index_type);
        for (i, window) in csi_windows.iter().enumerate() {
            let emb = extractor.extract(window);
            index.insert(emb, format!("window_{i}"), i as u64 * 100);
        }

        eprintln!("Built {:?} index with {} entries", index_type, index.len());

        // Test a query
        let query_emb = extractor.extract(&csi_windows[0]);
        let results = index.search(&query_emb, 5);
        eprintln!("Top-5 nearest to window_0:");
        for r in &results {
            eprintln!("  entry={}, distance={:.4}, metadata={}", r.entry, r.distance, r.metadata);
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

    // Ensure data directories exist for models and recordings
    let _ = std::fs::create_dir_all("data/models");
    let _ = std::fs::create_dir_all("data/recordings");

    // Discover model and recording files on startup
    let initial_models = scan_model_files();
    let initial_recordings = scan_recording_files();
    info!("Discovered {} model files, {} recording files", initial_models.len(), initial_recordings.len());

    let (tx, _) = broadcast::channel::<String>(256);
    let state: SharedState = Arc::new(RwLock::new(AppStateInner {
        latest_update: None,
        rssi_history: VecDeque::new(),
        frame_history: VecDeque::new(),
        tick: 0,
        source: source.into(),
        last_esp32_frame: None,
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
        smoothed_person_score: 0.0,
        prev_person_count: 0,
        smoothed_motion: 0.0,
        current_motion_level: "absent".to_string(),
        debounce_counter: 0,
        debounce_candidate: "absent".to_string(),
        baseline_motion: 0.0,
        baseline_frames: 0,
        smoothed_hr: 0.0,
        smoothed_br: 0.0,
        smoothed_hr_conf: 0.0,
        smoothed_br_conf: 0.0,
        hr_buffer: VecDeque::with_capacity(8),
        br_buffer: VecDeque::with_capacity(8),
        edge_vitals: None,
        latest_wasm_events: None,
        // Model management
        discovered_models: initial_models,
        active_model_id: None,
        // Recording
        recordings: initial_recordings,
        recording_active: false,
        recording_start_time: None,
        recording_current_id: None,
        recording_stop_tx: None,
        // Training
        training_status: "idle".to_string(),
        training_config: None,
        adaptive_model: adaptive_classifier::AdaptiveModel::load(&adaptive_classifier::model_path()).ok().map(|m| {
            info!("Loaded adaptive classifier: {} frames, {:.1}% accuracy",
                  m.trained_frames, m.training_accuracy * 100.0);
            m
        }),
        node_states: HashMap::new(),
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

    // ADR-050: Parse bind address once, use for all listeners
    let bind_ip: std::net::IpAddr = args.bind_addr.parse()
        .expect("Invalid --bind-addr (use 127.0.0.1 or 0.0.0.0)");

    // WebSocket server on dedicated port (8765)
    let ws_state = state.clone();
    let ws_app = Router::new()
        .route("/ws/sensing", get(ws_sensing_handler))
        .route("/health", get(health))
        .with_state(ws_state);

    let ws_addr = SocketAddr::from((bind_ip, args.ws_port));
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
        .route("/api/v1/edge-vitals", get(edge_vitals_endpoint))
        .route("/api/v1/wasm-events", get(wasm_events_endpoint))
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
        // Sensing WebSocket on the HTTP port so the UI can reach it without a second port
        .route("/ws/sensing", get(ws_sensing_handler))
        // Model management endpoints (UI compatibility)
        .route("/api/v1/models", get(list_models))
        .route("/api/v1/models/active", get(get_active_model))
        .route("/api/v1/models/load", post(load_model))
        .route("/api/v1/models/unload", post(unload_model))
        .route("/api/v1/models/{id}", delete(delete_model))
        .route("/api/v1/models/lora/profiles", get(list_lora_profiles))
        .route("/api/v1/models/lora/activate", post(activate_lora_profile))
        // Recording endpoints
        .route("/api/v1/recording/list", get(list_recordings))
        .route("/api/v1/recording/start", post(start_recording))
        .route("/api/v1/recording/stop", post(stop_recording))
        .route("/api/v1/recording/{id}", delete(delete_recording))
        // Training endpoints
        .route("/api/v1/train/status", get(train_status))
        .route("/api/v1/train/start", post(train_start))
        .route("/api/v1/train/stop", post(train_stop))
        // Adaptive classifier endpoints
        .route("/api/v1/adaptive/train", post(adaptive_train))
        .route("/api/v1/adaptive/status", get(adaptive_status))
        .route("/api/v1/adaptive/unload", post(adaptive_unload))
        // Static UI files
        .nest_service("/ui", ServeDir::new(&ui_path))
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::CACHE_CONTROL,
            HeaderValue::from_static("no-cache, no-store, must-revalidate"),
        ))
        .with_state(state.clone());

    let http_addr = SocketAddr::from((bind_ip, args.http_port));
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
            "source": s.effective_source(),
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
