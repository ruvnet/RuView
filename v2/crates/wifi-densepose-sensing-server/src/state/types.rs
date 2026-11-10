//! Core state definitions and types for the sensing server.
//!
//! Complies with ELMAS.md: <= 350 lines per file, no sprawling structures.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, RwLock};

use crate::adaptive_classifier;
use crate::engine_bridge;
use crate::mediatek_activity;
use crate::mediatek_csi;
use crate::mediatek_csi_ring;
use crate::mediatek_heuristic;
use crate::pose_physics;
use crate::qualcomm_csi;
use crate::realtek_csi;
use crate::realtek_radar;
use crate::rvf_container::RvfContainerInfo;
use crate::training_api;
use wifi_densepose_sensing_server::bootstrap_baseline::BootstrapBaselineMetadata;
use wifi_densepose_sensing_server::domain::{
    CalibrationGridBinding, CalibrationModelReceipt, NodeState, RollingP95, SensingUpdate,
};
use wifi_densepose_sensing_server::introspection;
use wifi_densepose_sensing_server::rufield_surface;
use wifi_densepose_sensing_server::rvf_pipeline::ProgressiveLoader;
use wifi_densepose_sensing_server::stream_stats;
use wifi_densepose_sensing_server::vendor_rf;
use wifi_densepose_sensing_server::vital_signs::{VitalSignDetector, VitalSigns};
use wifi_densepose_sensing_server::ingest::{Esp32VitalsPacket, WasmOutputPacket};
use wifi_densepose_signal::ruvsense::field_model::FieldModel;
use wifi_densepose_signal::ruvsense::multistatic::MultistaticFuser;
use wifi_densepose_signal::ruvsense::pose_tracker::PoseTracker;

/// If no ESP32 frame arrives within this duration, source reverts to offline.
pub const ESP32_OFFLINE_TIMEOUT: Duration = Duration::from_secs(5);

/// How many recent frames per node the grid gate votes over.
pub const GRID_VOTE_FRAMES: usize = 64;

/// Number of frames retained in `frame_history` for temporal analysis.
pub const FRAME_HISTORY_CAPACITY: usize = 100;

/// Maximum duration allowed for prewarming bootstrap holdout before timing out.
pub const BOOTSTRAP_HOLDOUT_PREWARM_TIMEOUT_MS: u64 = 35_000;

/// Shared application state reference.
pub type SharedState = Arc<RwLock<AppStateInner>>;

/// Shared application state inner structure.
pub struct AppStateInner {
    pub(crate) latest_update: Option<SensingUpdate>,
    pub(crate) rssi_history: VecDeque<f64>,
    pub(crate) frame_history: VecDeque<Vec<f64>>,
    pub(crate) tick: u64,
    pub(crate) source: String,
    pub(crate) last_esp32_frame: Option<Instant>,
    pub(crate) latest_realtek_radar: Option<realtek_radar::RealtekRadarSnapshot>,
    pub(crate) last_realtek_frame: Option<Instant>,
    pub(crate) latest_mediatek_csi: Option<mediatek_csi::MediatekCsiSnapshot>,
    pub(crate) last_mediatek_frame: Option<Instant>,
    pub(crate) mediatek_csi_by_device:
        HashMap<String, (mediatek_csi::MediatekCsiSnapshot, Instant)>,
    pub(crate) mediatek_heuristic_by_device: HashMap<String, mediatek_heuristic::DeviceHistory>,
    pub(crate) mediatek_csi_ring_by_device: HashMap<String, mediatek_csi_ring::DeviceRing>,
    pub(crate) csi_ring_capacity: usize,
    pub(crate) mediatek_activity_by_device: HashMap<String, mediatek_activity::ActivityTracker>,
    pub(crate) activity_floor_seed: HashMap<String, f64>,
    pub(crate) room_activity_peak_history: VecDeque<(Instant, f64)>,
    pub(crate) room_abs_activity_peak_history: VecDeque<(Instant, f64)>,
    pub(crate) activity_history: VecDeque<mediatek_activity::ActivitySample>,
    pub(crate) activity_history_last_sampled: Option<Instant>,
    pub(crate) latest_qualcomm_csi: Option<qualcomm_csi::QualcommCsiSnapshot>,
    pub(crate) last_qualcomm_frame: Option<Instant>,
    pub(crate) latest_realtek_csi: Option<realtek_csi::RealtekCsiSnapshot>,
    pub(crate) last_realtek_csi_frame: Option<Instant>,
    pub(crate) latest_vendor_rf: BTreeMap<String, vendor_rf::VendorEventSnapshot>,
    pub(crate) last_vendor_rf_frame: Option<Instant>,
    pub(crate) tx: broadcast::Sender<String>,
    pub(crate) intro: introspection::IntrospectionState,
    pub(crate) intro_tx: broadcast::Sender<String>,
    pub(crate) ws_clients: stream_stats::WsClientCounter,
    pub(crate) update_rate: stream_stats::UpdateRateMeter,
    pub(crate) total_detections: u64,
    pub(crate) start_time: Instant,
    pub(crate) vital_detector: VitalSignDetector,
    pub(crate) latest_vitals: VitalSigns,
    pub(crate) rvf_info: Option<RvfContainerInfo>,
    pub(crate) save_rvf_path: Option<PathBuf>,
    pub(crate) progressive_loader: Option<ProgressiveLoader>,
    pub(crate) active_sona_profile: Option<String>,
    pub(crate) model_loaded: bool,
    pub(crate) smoothed_person_score: f64,
    pub(crate) prev_person_count: usize,
    pub(crate) smoothed_motion: f64,
    pub(crate) current_motion_level: String,
    pub(crate) debounce_counter: u32,
    pub(crate) debounce_candidate: String,
    pub(crate) baseline_motion: f64,
    pub(crate) baseline_frames: u64,
    pub(crate) smoothed_hr: f64,
    pub(crate) smoothed_br: f64,
    pub(crate) smoothed_hr_conf: f64,
    pub(crate) smoothed_br_conf: f64,
    pub(crate) hr_buffer: VecDeque<f64>,
    pub(crate) br_buffer: VecDeque<f64>,
    pub(crate) edge_vitals: Option<Esp32VitalsPacket>,
    pub(crate) latest_wasm_events: Option<WasmOutputPacket>,
    pub(crate) discovered_models: Vec<serde_json::Value>,
    pub(crate) active_model_id: Option<String>,
    pub(crate) recordings: Vec<serde_json::Value>,
    pub(crate) recording_active: bool,
    pub(crate) recording_start_time: Option<Instant>,
    pub(crate) recording_current_id: Option<String>,
    pub(crate) recording_stop_tx: Option<tokio::sync::watch::Sender<bool>>,
    pub(crate) privacy_mode: bool,
    pub(crate) training_state: training_api::TrainingState,
    pub(crate) training_progress_tx: broadcast::Sender<String>,
    pub(crate) adaptive_model: Option<adaptive_classifier::AdaptiveModel>,
    pub(crate) node_states: HashMap<u8, NodeState>,
    pub(crate) room_debounced_level: String,
    pub(crate) room_debounce_candidate: String,
    pub(crate) room_debounce_since: Option<Instant>,
    pub(crate) pose_tracker: PoseTracker,
    pub(crate) last_tracker_instant: Option<Instant>,
    pub(crate) multistatic_fuser: MultistaticFuser,
    pub(crate) node_positions_config: HashMap<u8, [f32; 3]>,
    pub(crate) default_position_noted: std::collections::HashSet<u8>,
    pub(crate) engine_bridge: engine_bridge::EngineBridge,
    pub(crate) field_model: Option<FieldModel>,
    pub(crate) installation_id: Option<String>,
    pub(crate) bootstrap_baseline: Option<BootstrapBaselineMetadata>,
    pub(crate) bootstrap_baseline_active: bool,
    pub(crate) calibration_model_id: Option<String>,
    pub(crate) calibration_boot_epoch: String,
    pub(crate) calibration_session_id: Option<String>,
    pub(crate) calibration_binding_digest: Option<String>,
    pub(crate) calibration_source_node_ids: std::collections::BTreeSet<u8>,
    pub(crate) calibration_observed_source_node_ids: std::collections::BTreeSet<u8>,
    pub(crate) calibration_model_receipt: Option<CalibrationModelReceipt>,
    pub(crate) calibration_grid_binding: Option<CalibrationGridBinding>,
    pub(crate) calibration_last_sequences: HashMap<u8, u32>,
    pub(crate) calibration_reordered_packets: HashMap<u8, u64>,
    pub(crate) calibration_max_reorder_depth: HashMap<u8, u32>,
    pub(crate) calibration_sequence_fault_node_ids: std::collections::BTreeSet<u8>,
    pub(crate) p95_variance: RollingP95,
    pub(crate) p95_motion_band_power: RollingP95,
    pub(crate) p95_spectral_power: RollingP95,
    pub(crate) dedup_factor: f64,
    pub(crate) data_dir: PathBuf,
    pub(crate) field_surface: rufield_surface::FieldState,
    pub(crate) pose_physics: pose_physics::PosePhysicsRuntime,
}
