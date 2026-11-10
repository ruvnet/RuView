//! Server state initialization from CLI arguments and persisted configuration.
//!
//! Complies with ELMAS.md: <= 350 lines per file, clean abstraction.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{info, warn};

use wifi_densepose_sensing_server::cli::Args;
use wifi_densepose_sensing_server::config::multistatic_guard_config_from_env;
use wifi_densepose_sensing_server::domain::{
    CalibrationGridBinding, CsiGridKey, RollingP95,
};
use wifi_densepose_sensing_server::vital_signs::{VitalSignDetector, VitalSigns};
use wifi_densepose_sensing_server::field_bridge;
use wifi_densepose_sensing_server::rufield_surface::FieldState;
use wifi_densepose_sensing_server::bootstrap_baseline;
use wifi_densepose_sensing_server::rvf_pipeline::ProgressiveLoader;
use wifi_densepose_signal::ruvsense::multistatic::{MultistaticConfig, MultistaticFuser};
use wifi_densepose_signal::ruvsense::pose_tracker::PoseTracker;

use crate::adaptive_classifier;
use crate::api::models::{models_dir, scan_model_files};
use crate::api::recordings::{recordings_dir, scan_recording_files};
use crate::training_api;
use wifi_densepose_sensing_server::cli::handlers::load_or_convert_model;
use crate::config::load_runtime_config;
use crate::engine_bridge;
use crate::mediatek_activity;
use crate::pose_physics::PosePhysicsRuntime;
use crate::rvf_container::RvfReader;
use crate::runtime::ingress::clamp_csi_ring;
use crate::api::calibration::opaque_calibration_id;
use crate::state::types::{AppStateInner, SharedState};

/// Construct and initialize the shared application state.
pub fn init_server_state(args: &Args, source: &str, field_surface: &FieldState) -> SharedState {
    let vital_sample_rate = 1000.0 / args.tick_ms as f64;
    info!("Vital sign detector sample rate: {vital_sample_rate:.1} Hz");

    let rvf_info = args.load_rvf.as_ref().and_then(|path| {
        info!("Loading RVF container from {}", path.display());
        match RvfReader::from_file(path) {
            Ok(reader) => {
                let info = reader.info();
                info!("  RVF loaded: {} segments, {} bytes", info.segment_count, info.total_size);
                Some(info)
            }
            Err(e) => {
                tracing::error!("Failed to load RVF container: {e}");
                None
            }
        }
    });

    let model_path = args.model.as_ref().or(args.load_rvf.as_ref());
    let mut progressive_loader: Option<ProgressiveLoader> = None;
    let mut model_loaded = false;
    if let Some(mp) = model_path {
        if args.progressive || args.model.is_some() {
            info!("Loading trained model (progressive) from {}", mp.display());
            if let Ok(data) = std::fs::read(mp) {
                match load_or_convert_model(mp, &data) {
                    Ok(mut loader) => {
                        if let Ok(la) = loader.load_layer_a() {
                            info!("  Layer A ready: model={} v{} ({} segments)", la.model_name, la.version, la.n_segments);
                        }
                        model_loaded = true;
                        progressive_loader = Some(loader);
                    }
                    Err(e) => {
                        tracing::error!("{e}");
                        tracing::error!("Model NOT loaded — falling back to signal heuristics.");
                    }
                }
            }
        }
    }

    let models_path = models_dir(&args.data_dir);
    let recordings_path = recordings_dir(&args.data_dir);
    let _ = std::fs::create_dir_all(&models_path);
    let _ = std::fs::create_dir_all(&recordings_path);

    let initial_models = scan_model_files(&models_path);
    let initial_recordings = scan_recording_files(&recordings_path);
    info!("Discovered {} model files, {} recording files", initial_models.len(), initial_recordings.len());

    let data_dir = args.data_dir.clone();
    let runtime_config = load_runtime_config(&data_dir);
    let adaptive_model_path = adaptive_classifier::model_path(&data_dir);
    wifi_densepose_sensing_server::browser_session::init_secret(&data_dir);

    let (bootstrap_field_model, bootstrap_metadata, bootstrap_baseline_active) =
        if !args.calibrate {
            args.installation_id.as_deref().map_or((None, None, false), |id| {
                let path = bootstrap_baseline::path_in(&data_dir);
                match bootstrap_baseline::load(&path, id, chrono::Utc::now().timestamp_millis() as u64) {
                    Ok((model, metadata)) => (Some(model), Some(metadata), true),
                    Err(_) => (None, None, false),
                }
            })
        } else {
            (None, None, false)
        };

    let (tx, _) = broadcast::channel::<String>(256);
    let (intro_tx, _) = broadcast::channel::<String>(256);

    let mut engine_bridge_multistatic_cfg: Option<MultistaticConfig> = None;
    let mut node_positions_config: HashMap<u8, [f32; 3]> = HashMap::new();
    let mut engine_bridge_node_positions: HashMap<u8, [f32; 3]> = HashMap::new();
    let activity_floor_seed = args.activity_state.as_ref().map(|p| mediatek_activity::load_activity_state(p)).unwrap_or_default();

    let multistatic_fuser = {
        let cfg = multistatic_guard_config_from_env();
        let mut fuser = MultistaticFuser::with_config(MultistaticConfig {
            min_nodes: 1,
            ..cfg.clone()
        });
        if let Some(ref pos_str) = args.node_positions {
            let entries = field_bridge::parse_node_position_entries(pos_str);
            if !entries.is_empty() {
                if let Some(note) = field_bridge::positional_entries_note(&entries) {
                    warn!("{note}");
                }
                node_positions_config = field_bridge::node_positions_by_id(&entries);
                fuser.set_node_positions_by_id(node_positions_config.clone());
                engine_bridge_node_positions = node_positions_config.clone();
            }
        }
        engine_bridge_multistatic_cfg = Some(MultistaticConfig { min_nodes: 1, ..cfg });
        fuser
    };

    Arc::new(RwLock::new(AppStateInner {
        latest_update: None,
        rssi_history: VecDeque::new(),
        frame_history: VecDeque::new(),
        tick: 0,
        source: source.into(),
        last_esp32_frame: None,
        latest_realtek_radar: None,
        last_realtek_frame: None,
        latest_mediatek_csi: None,
        last_mediatek_frame: None,
        mediatek_csi_by_device: HashMap::new(),
        mediatek_heuristic_by_device: HashMap::new(),
        mediatek_csi_ring_by_device: HashMap::new(),
        csi_ring_capacity: clamp_csi_ring(args.csi_ring),
        mediatek_activity_by_device: HashMap::new(),
        activity_floor_seed,
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
        tx,
        intro: wifi_densepose_sensing_server::introspection::IntrospectionState::new(),
        intro_tx,
        ws_clients: Default::default(),
        update_rate: Default::default(),
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
        discovered_models: initial_models,
        active_model_id: None,
        recordings: initial_recordings,
        recording_active: false,
        recording_start_time: None,
        recording_current_id: None,
        recording_stop_tx: None,
        privacy_mode: args.privacy_mode,
        training_state: training_api::TrainingState::default(),
        training_progress_tx: broadcast::channel::<String>(256).0,
        adaptive_model: adaptive_classifier::AdaptiveModel::load(&adaptive_model_path).ok(),
        node_states: HashMap::new(),
        room_debounced_level: "absent".to_string(),
        room_debounce_candidate: "absent".to_string(),
        room_debounce_since: None,
        pose_tracker: PoseTracker::new(),
        last_tracker_instant: None,
        multistatic_fuser,
        node_positions_config,
        default_position_noted: std::collections::HashSet::new(),
        engine_bridge: engine_bridge::EngineBridge::new(
            wifi_densepose_bfld::PrivacyMode::PrivateHome,
            1,
            "default",
            "Default Room",
            engine_bridge_multistatic_cfg,
        ).with_node_positions(engine_bridge_node_positions),
        field_model: bootstrap_field_model,
        installation_id: args.installation_id.clone(),
        bootstrap_baseline: bootstrap_metadata.clone(),
        bootstrap_baseline_active,
        calibration_model_id: None,
        calibration_boot_epoch: opaque_calibration_id("cal-boot"),
        calibration_session_id: None,
        calibration_binding_digest: None,
        calibration_source_node_ids: bootstrap_metadata
            .as_ref()
            .map(|m| m.source_node_ids.iter().copied().collect())
            .unwrap_or_default(),
        calibration_observed_source_node_ids: std::collections::BTreeSet::new(),
        calibration_model_receipt: None,
        calibration_grid_binding: bootstrap_metadata.as_ref().and_then(|m| {
            let [source_node_id] = m.source_node_ids.as_slice() else { return None; };
            Some(CalibrationGridBinding {
                source_node_id: *source_node_id,
                grid: CsiGridKey::from_bootstrap_grid(m.source_grid),
                evidence: None,
            })
        }),
        calibration_last_sequences: HashMap::new(),
        calibration_reordered_packets: HashMap::new(),
        calibration_max_reorder_depth: HashMap::new(),
        calibration_sequence_fault_node_ids: std::collections::BTreeSet::new(),
        p95_variance: RollingP95::new(600, 60),
        p95_motion_band_power: RollingP95::new(600, 60),
        p95_spectral_power: RollingP95::new(600, 60),
        dedup_factor: runtime_config.dedup_factor,
        data_dir,
        field_surface: field_surface.clone(),
        pose_physics: PosePhysicsRuntime::new(wifi_densepose_physics::PhysicsConfig::default())
            .expect("default pose physics configuration is valid"),
    }))
}
