//! Data types, constants, and shared state definitions.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};

use crate::adaptive_classifier;
use crate::rvf_container::RvfContainerInfo;
use crate::rvf_pipeline::ProgressiveLoader;
use crate::vital_signs::{VitalSignDetector, VitalSigns};

use wifi_densepose_signal::ruvsense::pose_tracker::PoseTracker;
use wifi_densepose_signal::ruvsense::multistatic::MultistaticFuser;
use wifi_densepose_signal::ruvsense::field_model::FieldModel;

// ── Constants ───────────────────────────────────────────────────────────────

/// Number of frames retained in `frame_history` for temporal analysis.
pub const FRAME_HISTORY_CAPACITY: usize = 100;

/// If no ESP32 frame arrives within this duration, source reverts to offline.
pub const ESP32_OFFLINE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Default EMA alpha for temporal keypoint smoothing (RuVector Phase 2).
pub const TEMPORAL_EMA_ALPHA_DEFAULT: f64 = 0.15;
/// Reduced EMA alpha when coherence is low.
pub const TEMPORAL_EMA_ALPHA_LOW_COHERENCE: f64 = 0.05;
/// Coherence threshold below which we reduce EMA alpha.
pub const COHERENCE_LOW_THRESHOLD: f64 = 0.3;
/// Maximum allowed bone-length change ratio between frames (20%).
pub const MAX_BONE_CHANGE_RATIO: f64 = 0.20;
/// Number of motion_energy frames to track for coherence scoring.
pub const COHERENCE_WINDOW: usize = 20;

/// Debounce frames required before state transition (at ~10 FPS = ~0.4s).
pub const DEBOUNCE_FRAMES: u32 = 4;
/// EMA alpha for motion smoothing (~1s time constant at 10 FPS).
pub const MOTION_EMA_ALPHA: f64 = 0.15;
/// EMA alpha for slow-adapting baseline (~30s time constant at 10 FPS).
pub const BASELINE_EMA_ALPHA: f64 = 0.003;
/// Number of warm-up frames before baseline subtraction kicks in.
pub const BASELINE_WARMUP: u64 = 50;

/// Size of the median filter window for vital signs outlier rejection.
pub const VITAL_MEDIAN_WINDOW: usize = 21;
/// EMA alpha for vital signs (~5s time constant at 10 FPS).
pub const VITAL_EMA_ALPHA: f64 = 0.02;
/// Maximum BPM jump per frame before a value is rejected as an outlier.
pub const HR_MAX_JUMP: f64 = 8.0;
pub const BR_MAX_JUMP: f64 = 2.0;
/// Minimum change from current smoothed value before EMA updates (dead-band).
pub const HR_DEAD_BAND: f64 = 2.0;
pub const BR_DEAD_BAND: f64 = 0.5;

// ── ESP32 Frame ─────────────────────────────────────────────────────────────

/// ADR-018 ESP32 CSI binary frame header (20 bytes)
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Esp32Frame {
    pub magic: u32,
    pub node_id: u8,
    pub n_antennas: u8,
    pub n_subcarriers: u8,
    pub freq_mhz: u16,
    pub sequence: u32,
    pub rssi: i8,
    pub noise_floor: i8,
    pub amplitudes: Vec<f64>,
    pub phases: Vec<f64>,
}

// ── Sensing Update ──────────────────────────────────────────────────────────

/// Sensing update broadcast to WebSocket clients
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensingUpdate {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub timestamp: f64,
    pub source: String,
    pub tick: u64,
    pub nodes: Vec<NodeInfo>,
    pub features: FeatureInfo,
    pub classification: ClassificationInfo,
    pub signal_field: SignalField,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vital_signs: Option<VitalSigns>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enhanced_motion: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enhanced_breathing: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posture: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal_quality_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality_verdict: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bssid_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pose_keypoints: Option<Vec<[f64; 4]>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_status: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persons: Option<Vec<PersonDetection>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_persons: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_features: Option<Vec<PerNodeFeatureInfo>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub node_id: u8,
    pub rssi_dbm: f64,
    pub position: [f64; 3],
    pub amplitude: Vec<f64>,
    pub subcarrier_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureInfo {
    pub mean_rssi: f64,
    pub variance: f64,
    pub motion_band_power: f64,
    pub breathing_band_power: f64,
    pub dominant_freq_hz: f64,
    pub change_points: usize,
    pub spectral_power: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationInfo {
    pub motion_level: String,
    pub presence: bool,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalField {
    pub grid_size: [usize; 3],
    pub values: Vec<f64>,
}

/// WiFi-derived pose keypoint (17 COCO keypoints)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoseKeypoint {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub confidence: f64,
}

/// Person detection from WiFi sensing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonDetection {
    pub id: u32,
    pub confidence: f64,
    pub keypoints: Vec<PoseKeypoint>,
    pub bbox: BoundingBox,
    pub zone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Per-node feature info for WebSocket broadcasts (multi-node support).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerNodeFeatureInfo {
    pub node_id: u8,
    pub features: FeatureInfo,
    pub classification: ClassificationInfo,
    pub rssi_dbm: f64,
    pub last_seen_ms: u64,
    pub frame_rate_hz: f64,
    pub stale: bool,
}

// ── ESP32 Edge Vitals Packet (ADR-039) ──────────────────────────────────────

/// Decoded vitals packet from ESP32 edge processing pipeline.
#[derive(Debug, Clone, Serialize)]
pub struct Esp32VitalsPacket {
    pub node_id: u8,
    pub presence: bool,
    pub fall_detected: bool,
    pub motion: bool,
    pub breathing_rate_bpm: f64,
    pub heartrate_bpm: f64,
    pub rssi: i8,
    pub n_persons: u8,
    pub motion_energy: f32,
    pub presence_score: f32,
    pub timestamp_ms: u32,
}

/// Single WASM event (type + value).
#[derive(Debug, Clone, Serialize)]
pub struct WasmEvent {
    pub event_type: u8,
    pub value: f32,
}

/// Decoded WASM output packet from ESP32 Tier 3 runtime.
#[derive(Debug, Clone, Serialize)]
pub struct WasmOutputPacket {
    pub node_id: u8,
    pub module_id: u8,
    pub events: Vec<WasmEvent>,
}

// ── Per-node state ──────────────────────────────────────────────────────────

/// Per-node sensing state for multi-node deployments (issue #249).
pub struct NodeState {
    pub frame_history: VecDeque<Vec<f64>>,
    pub smoothed_person_score: f64,
    pub prev_person_count: usize,
    pub smoothed_motion: f64,
    pub current_motion_level: String,
    pub debounce_counter: u32,
    pub debounce_candidate: String,
    pub baseline_motion: f64,
    pub baseline_frames: u64,
    pub smoothed_hr: f64,
    pub smoothed_br: f64,
    pub smoothed_hr_conf: f64,
    pub smoothed_br_conf: f64,
    pub hr_buffer: VecDeque<f64>,
    pub br_buffer: VecDeque<f64>,
    pub rssi_history: VecDeque<f64>,
    pub vital_detector: VitalSignDetector,
    pub latest_vitals: VitalSigns,
    pub last_frame_time: Option<std::time::Instant>,
    pub edge_vitals: Option<Esp32VitalsPacket>,
    pub latest_features: Option<FeatureInfo>,
    pub prev_keypoints: Option<Vec<[f64; 3]>>,
    pub motion_energy_history: VecDeque<f64>,
    pub coherence_score: f64,
}

impl NodeState {
    pub fn new() -> Self {
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
            latest_features: None,
            prev_keypoints: None,
            motion_energy_history: VecDeque::with_capacity(COHERENCE_WINDOW),
            coherence_score: 1.0,
        }
    }

    /// Update the coherence score from the latest motion_energy value.
    pub fn update_coherence(&mut self, motion_energy: f64) {
        if self.motion_energy_history.len() >= COHERENCE_WINDOW {
            self.motion_energy_history.pop_front();
        }
        self.motion_energy_history.push_back(motion_energy);

        let n = self.motion_energy_history.len();
        if n < 2 {
            self.coherence_score = 1.0;
            return;
        }

        let mean: f64 = self.motion_energy_history.iter().sum::<f64>() / n as f64;
        let variance: f64 = self.motion_energy_history.iter()
            .map(|v| (v - mean) * (v - mean))
            .sum::<f64>() / (n - 1) as f64;

        self.coherence_score = (1.0 / (1.0 + variance)).clamp(0.0, 1.0);
    }

    /// Choose the EMA alpha based on current coherence score.
    pub fn ema_alpha(&self) -> f64 {
        if self.coherence_score < COHERENCE_LOW_THRESHOLD {
            TEMPORAL_EMA_ALPHA_LOW_COHERENCE
        } else {
            TEMPORAL_EMA_ALPHA_DEFAULT
        }
    }
}

// ── Shared application state ────────────────────────────────────────────────

/// Shared application state
pub struct AppStateInner {
    pub latest_update: Option<SensingUpdate>,
    pub rssi_history: VecDeque<f64>,
    pub frame_history: VecDeque<Vec<f64>>,
    pub tick: u64,
    pub source: String,
    pub last_esp32_frame: Option<std::time::Instant>,
    pub tx: broadcast::Sender<String>,
    pub total_detections: u64,
    pub start_time: std::time::Instant,
    pub vital_detector: VitalSignDetector,
    pub latest_vitals: VitalSigns,
    pub rvf_info: Option<RvfContainerInfo>,
    pub save_rvf_path: Option<PathBuf>,
    pub progressive_loader: Option<ProgressiveLoader>,
    pub active_sona_profile: Option<String>,
    pub model_loaded: bool,
    pub smoothed_person_score: f64,
    pub prev_person_count: usize,
    pub smoothed_motion: f64,
    pub current_motion_level: String,
    pub debounce_counter: u32,
    pub debounce_candidate: String,
    pub baseline_motion: f64,
    pub baseline_frames: u64,
    pub smoothed_hr: f64,
    pub smoothed_br: f64,
    pub smoothed_hr_conf: f64,
    pub smoothed_br_conf: f64,
    pub hr_buffer: VecDeque<f64>,
    pub br_buffer: VecDeque<f64>,
    pub edge_vitals: Option<Esp32VitalsPacket>,
    pub latest_wasm_events: Option<WasmOutputPacket>,
    pub discovered_models: Vec<serde_json::Value>,
    pub active_model_id: Option<String>,
    pub recordings: Vec<serde_json::Value>,
    pub recording_active: bool,
    pub recording_start_time: Option<std::time::Instant>,
    pub recording_current_id: Option<String>,
    pub recording_stop_tx: Option<tokio::sync::watch::Sender<bool>>,
    pub training_status: String,
    pub training_config: Option<serde_json::Value>,
    pub adaptive_model: Option<adaptive_classifier::AdaptiveModel>,
    pub node_states: HashMap<u8, NodeState>,
    pub pose_tracker: PoseTracker,
    pub last_tracker_instant: Option<std::time::Instant>,
    pub multistatic_fuser: MultistaticFuser,
    pub field_model: Option<FieldModel>,
}

impl AppStateInner {
    /// Return the effective data source, accounting for ESP32 frame timeout.
    pub fn effective_source(&self) -> String {
        if self.source == "esp32" {
            if let Some(last) = self.last_esp32_frame {
                if last.elapsed() > ESP32_OFFLINE_TIMEOUT {
                    return "esp32:offline".to_string();
                }
            }
        }
        self.source.clone()
    }

    /// Person count: eigenvalue-based if field model is calibrated, else heuristic.
    pub fn person_count(&self) -> usize {
        use crate::field_bridge;
        use crate::csi::score_to_person_count;
        match self.field_model.as_ref() {
            Some(fm) => {
                let history = if !self.frame_history.is_empty() {
                    &self.frame_history
                } else {
                    self.node_states.values()
                        .filter(|ns| !ns.frame_history.is_empty())
                        .max_by_key(|ns| ns.last_frame_time)
                        .map(|ns| &ns.frame_history)
                        .unwrap_or(&self.frame_history)
                };
                field_bridge::occupancy_or_fallback(
                    fm, history, self.smoothed_person_score, self.prev_person_count,
                )
            }
            None => score_to_person_count(self.smoothed_person_score, self.prev_person_count),
        }
    }
}

pub type SharedState = Arc<RwLock<AppStateInner>>;
