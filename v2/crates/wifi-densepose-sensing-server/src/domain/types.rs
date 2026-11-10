use serde::{Deserialize, Serialize};

use crate::domain::csi_grid::{CalibratedPresenceEvidence, DetectorDiagnostics};
use crate::inference::{NodeInference, RoomInference};
use crate::vital_signs::VitalSigns;

/// WiFi-derived signal field heatmap (20x20 floor-plane grid).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalField {
    pub grid_size: [usize; 3],
    pub values: Vec<f64>,
}

/// Extracted physical signal features from CSI data.
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

/// Presence and motion classification results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassificationInfo {
    pub motion_level: String,
    pub presence: bool,
    pub confidence: f64,
}

/// 2D bounding box in normalized or scene coordinates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// WiFi-derived pose keypoint (17 COCO keypoints).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoseKeypoint {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub confidence: f64,
}

/// Person detection from WiFi sensing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonDetection {
    pub id: u32,
    pub confidence: f64,
    pub keypoints: Vec<PoseKeypoint>,
    pub bbox: BoundingBox,
    pub zone: String,
    #[serde(default)]
    pub position: [f64; 3],
    #[serde(default)]
    pub motion_score: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pose: Option<String>,
}

/// Diagnostics for MediaTek heuristic detector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediatekNodeDiagnostics {
    pub presence_state: String,
    pub coefficient_of_variation: f64,
    pub baseline_deviation: f64,
    pub sample_count: usize,
    pub span_ms: u64,
}

/// Per-node mesh synchronization snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSyncSnapshot {
    pub offset_us: i64,
    pub is_leader: bool,
    pub is_valid: bool,
    pub smoothed: bool,
    pub sequence: u32,
    pub csi_fps_ema: f64,
    pub csi_fps_samples: u32,
    pub staleness_ms: Option<u64>,
}

/// Node status and spatial telemetry information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub node_id: u8,
    pub rssi_dbm: f64,
    pub position: [f64; 3],
    pub amplitude: Vec<f64>,
    pub subcarrier_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync: Option<NodeSyncSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_inference: Option<NodeInference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mediatek_diagnostics: Option<MediatekNodeDiagnostics>,
}

/// Per-node feature info for multi-node WebSocket broadcasts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerNodeFeatureInfo {
    pub node_id: u8,
    pub features: FeatureInfo,
    pub classification: ClassificationInfo,
    pub rssi_dbm: f64,
    pub last_seen_ms: u64,
    pub frame_rate_hz: f64,
    pub stale: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub novelty_score: Option<f32>,
}

/// Root telemetry sensing payload broadcast to clients and recorded to datasets.
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calibrated_presence_evidence: Option<CalibratedPresenceEvidence>,
    #[serde(skip_deserializing, skip_serializing_if = "Option::is_none")]
    pub detector_diagnostics: Option<DetectorDiagnostics>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_inference: Option<RoomInference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classifier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity: Option<serde_json::Value>,
}
