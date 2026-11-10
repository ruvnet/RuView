//! WiFi-DensePose Sensing Server
//!
//! Lightweight Axum server that:
//! - Receives ESP32 CSI frames via UDP (port 5005)
//! - Processes signals using RuVector-powered wifi-densepose-signal crate
//! - Broadcasts sensing updates via WebSocket (ws://localhost:8765/ws/sensing)
//! - Serves the static UI files (port 8080)
//!
//! Replaces both ws_server.py and the Python HTTP server.
#![allow(dead_code)]

// Library re-exports needed by internal modules referencing crate::...
pub(crate) use wifi_densepose_sensing_server::config;
pub(crate) use wifi_densepose_sensing_server::domain::{
    self, apply_temporal_smoothing, attach_field_positions,
    BoundingBox, ClassificationInfo, FeatureInfo, MediatekNodeDiagnostics, NodeInfo,
    NodeState, NodeSyncSnapshot, PerNodeFeatureInfo, PersonDetection, PoseKeypoint, SensingUpdate,
    SignalField, edge_vitals_message_for_publication, vitals_for_publication,
    DEFAULT_NODE_POSITION, default_position_node_ids, node_is_fresh,
    node_positions_by_active_id, note_default_position_nodes,
    VITAL_PUBLICATION_MIN_CONFIDENCE, VITAL_PUBLICATION_MIN_SIGNAL_QUALITY,
};
pub(crate) use wifi_densepose_sensing_server::dsp::{
    self, compute_subcarrier_importance_weights, compute_subcarrier_variances,
    estimate_breathing_rate_hz, estimate_temporal_dominant_frequency_hz,
    extract_features_from_frame, generate_signal_field, raw_classify,
};
pub(crate) use wifi_densepose_sensing_server::ingest::{
    generate_simulated_frame, parse_netsh_interfaces_output, plan_source, probe_esp32,
    probe_wifi, SourcePlan, SOURCE_VALUES,
};
pub(crate) use wifi_densepose_sensing_server::cli::{self, Args};
pub(crate) use wifi_densepose_sensing_server::pose::{
    derive_pose_from_sensing, derive_single_person_pose,
};
pub(crate) use wifi_densepose_sensing_server::rvf_pipeline;
pub(crate) use wifi_densepose_sensing_server::vital_signs;
pub(crate) use wifi_densepose_sensing_server::{
    bootstrap_baseline, error_response, field_bridge, privacy_filter,
};
pub(crate) use wifi_densepose_sensing_server::domain::NODE_STALE_AFTER_MS;

// Binary modules
pub(crate) mod adaptive_classifier;
pub mod api;
pub(crate) use api::calibration::*;
pub mod csi;
pub(crate) mod engine_bridge;
pub(crate) mod field_localize;
pub(crate) mod model_format;
pub(crate) mod multistatic_bridge;
pub(crate) mod mediatek_activity;
pub(crate) mod mediatek_csi;
pub(crate) mod mediatek_csi_ring;
pub(crate) mod mediatek_devices;
pub(crate) mod mediatek_heuristic;
pub(crate) mod qualcomm_csi;
pub(crate) mod realtek_csi;
pub(crate) mod realtek_radar;
pub(crate) mod path_safety;
pub mod pose;
pub mod pose_physics;
pub(crate) mod rvf_container;
pub(crate) mod runtime;
pub(crate) mod state;
pub(crate) mod tracker_bridge;
pub(crate) mod training_api;
pub mod types;

// Re-exports from internal binary modules for crate:: resolution
pub(crate) use runtime::*;
pub(crate) use state::*;

use clap::Parser;
use tracing::warn;
use wifi_densepose_sensing_server::telemetry;

/// If `--ui-path` points nowhere (wrong cwd), try common repo layouts relative to cwd.
fn coalesce_ui_path(initial: std::path::PathBuf) -> std::path::PathBuf {
    if initial.is_dir() {
        return initial;
    }
    for rel in &["../ui", "./ui", "../../ui"] {
        let p = std::path::PathBuf::from(rel);
        if p.is_dir() {
            warn!(
                "UI path {} not found; using {} (set --ui-path explicitly if wrong)",
                initial.display(),
                p.display()
            );
            return p;
        }
    }
    initial
}

fn validate_startup_calibration_request(calibrate: bool) -> Result<(), &'static str> {
    if calibrate {
        Err("--calibrate cannot safely select a source and CSI grid at process start. Start the server normally, wait at least 10 seconds for live grid evidence, then POST /api/v1/calibration/start with one explicit source_node_id.")
    } else {
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let _telemetry = telemetry::init();

    let mut args = Args::parse();
    args.ui_path = coalesce_ui_path(args.ui_path);

    if let Err(error) = validate_startup_calibration_request(args.calibrate) {
        eprintln!("Error: {error}");
        std::process::exit(2);
    }

    // Dispatch early-exit CLI commands (benchmark, convert-model, export-rvf, pretrain, train, embed, build-index)
    if let Some(code) = wifi_densepose_sensing_server::cli::handle_cli_modes(&args) {
        if code != 0 {
            std::process::exit(code);
        }
        return;
    }
    if let Err(e) = runtime::run_server(args).await {
        eprintln!("Server error: {e}");
        std::process::exit(1);
    }
}
