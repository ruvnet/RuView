//! Application state for sensing-server
//!
//! Extracted from main.rs as part of ADR-051 Phase 1
//! 
//! NOTE: This is a transitional module. The full AppStateInner (37 fields)
//! will be decomposed further in subsequent phases:
//! - Phase 2: Extract vitals state
//! - Phase 3: Extract recording state  
//! - Phase 4: Extract training state
//! - Phase 5: Extract model management

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, RwLock};

use crate::adaptive_classifier::AdaptiveModel;
use crate::rvf_container::{RvfContainerInfo, ProgressiveLoader};
use crate::vital_signs::{VitalSignDetector, VitalSigns};

// Re-export types that will be moved in later phases
pub use super::{ClassificationInfo, FeatureInfo, SensingUpdate, Esp32VitalsPacket, WasmOutputPacket};

/// Number of frames retained in `frame_history` for temporal analysis.
/// At 500 ms ticks this covers ~50 seconds; at 100 ms ticks ~10 seconds.
pub const FRAME_HISTORY_CAPACITY: usize = 100;

/// Application state (transitional - will be decomposed further)
/// 
/// See ADR-051 for the full decomposition plan.
pub struct AppStateInner {
    // Core sensing state
    pub latest_update: Option<SensingUpdate>,
    pub rssi_history: VecDeque<f64>,
    pub frame_history: VecDeque<Vec<f64>>,
    pub tick: u64,
    pub source: String,
    pub tx: broadcast::Sender<String>,
    pub total_detections: u64,
    pub start_time: Instant,
    
    // Vital signs state (Phase 2: extract to VitalsState)
    pub vital_detector: VitalSignDetector,
    pub latest_vitals: VitalSigns,
    pub smoothed_hr: f64,
    pub smoothed_br: f64,
    pub smoothed_hr_conf: f64,
    pub smoothed_br_conf: f64,
    pub hr_buffer: VecDeque<f64>,
    pub br_buffer: VecDeque<f64>,
    pub edge_vitals: Option<Esp32VitalsPacket>,
    pub latest_wasm_events: Option<WasmOutputPacket>,
    
    // Model state (Phase 5: extract to ModelState)
    pub rvf_info: Option<RvfContainerInfo>,
    pub save_rvf_path: Option<PathBuf>,
    pub progressive_loader: Option<ProgressiveLoader>,
    pub active_sona_profile: Option<String>,
    pub model_loaded: bool,
    pub discovered_models: Vec<serde_json::Value>,
    pub active_model_id: Option<String>,
    pub adaptive_model: Option<AdaptiveModel>,
    
    // Motion classification state
    pub smoothed_person_score: f64,
    pub smoothed_motion: f64,
    pub current_motion_level: String,
    pub debounce_counter: u32,
    pub debounce_candidate: String,
    pub baseline_motion: f64,
    pub baseline_frames: u64,
    
    // Recording state (Phase 3: extract to RecordingState)
    pub recordings: Vec<serde_json::Value>,
    pub recording_active: bool,
    pub recording_start_time: Option<Instant>,
    pub recording_current_id: Option<String>,
    pub recording_stop_tx: Option<tokio::sync::watch::Sender<bool>>,
    
    // Training state (Phase 4: extract to TrainingState)
    pub training_status: String,
    pub training_config: Option<serde_json::Value>,
}

/// Shared state wrapper
pub type SharedState = Arc<RwLock<AppStateInner>>;

impl AppStateInner {
    /// Create a new state instance with default values
    pub fn new(tx: broadcast::Sender<String>, source: String) -> Self {
        Self {
            latest_update: None,
            rssi_history: VecDeque::with_capacity(1000),
            frame_history: VecDeque::with_capacity(FRAME_HISTORY_CAPACITY),
            tick: 0,
            source,
            tx,
            total_detections: 0,
            start_time: Instant::now(),
            vital_detector: VitalSignDetector::default(),
            latest_vitals: VitalSigns::default(),
            smoothed_hr: 0.0,
            smoothed_br: 0.0,
            smoothed_hr_conf: 0.0,
            smoothed_br_conf: 0.0,
            hr_buffer: VecDeque::with_capacity(15),
            br_buffer: VecDeque::with_capacity(15),
            edge_vitals: None,
            latest_wasm_events: None,
            rvf_info: None,
            save_rvf_path: None,
            progressive_loader: None,
            active_sona_profile: None,
            model_loaded: false,
            discovered_models: Vec::new(),
            active_model_id: None,
            adaptive_model: None,
            smoothed_person_score: 0.0,
            smoothed_motion: 0.0,
            current_motion_level: "unknown".to_string(),
            debounce_counter: 0,
            debounce_candidate: "unknown".to_string(),
            baseline_motion: 0.0,
            baseline_frames: 0,
            recordings: Vec::new(),
            recording_active: false,
            recording_start_time: None,
            recording_current_id: None,
            recording_stop_tx: None,
            training_status: "idle".to_string(),
            training_config: None,
        }
    }
}
