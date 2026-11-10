//! Minimal test constructor for `AppStateInner`.
//!
//! Complies with ELMAS.md: <= 350 lines per file, isolated test helpers.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

use wifi_densepose_sensing_server::domain::RollingP95;
use wifi_densepose_sensing_server::rufield_surface;
use wifi_densepose_sensing_server::vital_signs::{VitalSignDetector, VitalSigns};
use wifi_densepose_signal::ruvsense::multistatic::MultistaticFuser;
use wifi_densepose_signal::ruvsense::pose_tracker::PoseTracker;

use crate::api::calibration::opaque_calibration_id;
use crate::engine_bridge;
use crate::pose_physics;
use crate::training_api;
use super::types::AppStateInner;

impl AppStateInner {
    /// Minimal, dependency-free `AppStateInner` for in-process router tests.
    pub fn minimal() -> Self {
        AppStateInner {
            latest_update: None,
            rssi_history: VecDeque::new(),
            frame_history: VecDeque::new(),
            tick: 0,
            source: "test".to_string(),
            last_esp32_frame: None,
            latest_realtek_radar: None,
            last_realtek_frame: None,
            latest_mediatek_csi: None,
            last_mediatek_frame: None,
            mediatek_csi_by_device: HashMap::new(),
            mediatek_heuristic_by_device: HashMap::new(),
            mediatek_csi_ring_by_device: HashMap::new(),
            csi_ring_capacity: 16,
            mediatek_activity_by_device: HashMap::new(),
            activity_floor_seed: HashMap::new(),
            room_activity_peak_history: VecDeque::new(),
            room_abs_activity_peak_history: VecDeque::new(),
            activity_history: VecDeque::new(),
            activity_history_last_sampled: None,
            latest_qualcomm_csi: None,
            last_qualcomm_frame: None,
            latest_realtek_csi: None,
            last_realtek_csi_frame: None,
            latest_vendor_rf: BTreeMap::new(),
            last_vendor_rf_frame: None,
            tx: broadcast::channel::<String>(16).0,
            intro: wifi_densepose_sensing_server::introspection::IntrospectionState::new(),
            intro_tx: broadcast::channel::<String>(16).0,
            ws_clients: Default::default(),
            update_rate: Default::default(),
            total_detections: 0,
            start_time: std::time::Instant::now(),
            vital_detector: VitalSignDetector::new(10.0),
            latest_vitals: VitalSigns::default(),
            rvf_info: None,
            save_rvf_path: None,
            progressive_loader: None,
            active_sona_profile: None,
            model_loaded: false,
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
            discovered_models: Vec::new(),
            active_model_id: None,
            recordings: Vec::new(),
            recording_active: false,
            recording_start_time: None,
            recording_current_id: None,
            recording_stop_tx: None,
            privacy_mode: false,
            training_state: training_api::TrainingState::default(),
            training_progress_tx: broadcast::channel::<String>(256).0,
            adaptive_model: None,
            node_states: HashMap::new(),
            room_debounced_level: "absent".to_string(),
            room_debounce_candidate: "absent".to_string(),
            room_debounce_since: None,
            pose_tracker: PoseTracker::new(),
            last_tracker_instant: None,
            multistatic_fuser: MultistaticFuser::new(),
            node_positions_config: HashMap::new(),
            default_position_noted: std::collections::HashSet::new(),
            engine_bridge: engine_bridge::EngineBridge::new(
                wifi_densepose_bfld::PrivacyMode::PrivateHome,
                1,
                "default",
                "Default Room",
                None,
            ),
            field_model: None,
            installation_id: None,
            bootstrap_baseline: None,
            bootstrap_baseline_active: false,
            calibration_model_id: None,
            calibration_boot_epoch: opaque_calibration_id("cal-boot"),
            calibration_session_id: None,
            calibration_binding_digest: None,
            calibration_source_node_ids: std::collections::BTreeSet::new(),
            calibration_observed_source_node_ids: std::collections::BTreeSet::new(),
            calibration_model_receipt: None,
            calibration_grid_binding: None,
            calibration_last_sequences: HashMap::new(),
            calibration_reordered_packets: HashMap::new(),
            calibration_max_reorder_depth: HashMap::new(),
            calibration_sequence_fault_node_ids: std::collections::BTreeSet::new(),
            p95_variance: RollingP95::new(600, 60),
            p95_motion_band_power: RollingP95::new(600, 60),
            p95_spectral_power: RollingP95::new(600, 60),
            dedup_factor: 3.0,
            data_dir: std::path::PathBuf::from("data"),
            field_surface: Arc::new(RwLock::new(rufield_surface::FieldSurface::from_env())),
            pose_physics: pose_physics::PosePhysicsRuntime::new(
                wifi_densepose_physics::PhysicsConfig::default(),
            )
            .expect("default pose physics configuration is valid"),
        }
    }
}

