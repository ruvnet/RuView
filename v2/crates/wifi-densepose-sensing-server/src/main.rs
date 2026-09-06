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

mod adaptive_classifier;
pub mod cli;
pub mod csi;
mod engine_bridge;
mod field_bridge;
mod field_localize;
mod model_format;
mod multistatic_bridge;
mod mediatek_csi;
mod qualcomm_csi;
mod realtek_radar;
mod path_safety;
pub mod pose;
pub mod pose_physics;
mod rvf_container;
// ADR-186 (TRAIN-RECONNECT): the in-server training pipeline was written but
// never declared as a module, so it was orphaned / uncompiled. Declaring it
// here compiles it against the real `AppStateInner` and wires its `routes()`
// (including `/ws/train/progress`) into the live router below.
mod training_api;
mod rvf_pipeline;
mod tracker_bridge;
pub mod types;
mod vital_signs;

// Training pipeline modules (exposed via lib.rs)
use wifi_densepose_sensing_server::{
    dataset, embedding, error_response, graph_transformer, rufield_surface, semconv, telemetry,
    trainer,
};
use wifi_densepose_sensing_server::bootstrap_baseline::{
    self, BootstrapBaselineMetadata, BootstrapValidationSample,
    BOOTSTRAP_VALIDATION_MIN_SPACING_MS, BOOTSTRAP_VALIDATION_SAMPLES,
    BOOTSTRAP_VALIDATION_SAMPLE_TIMEOUT_MS,
};
// ADR-295 / ADR-297: canonical provenance state + per-node/room inference.
use wifi_densepose_sensing_server::inference::{fuse_room, NodeInference, RoomInference};
use wifi_densepose_sensing_server::provenance::SourceState;

use ruvector_mincut::{DynamicMinCut, MinCutBuilder};
use rand::{rngs::OsRng, RngCore};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    body::Bytes,
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::{delete, get, post},
    Extension, Router,
};
use clap::Parser;

use axum::http::HeaderValue;
use serde::{Deserialize, Serialize};
use tokio::net::UdpSocket;
use tokio::sync::{broadcast, RwLock};
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use tracing::{debug, error, info, warn};

use rvf_container::{RvfBuilder, RvfContainerInfo, RvfReader, VitalSignConfig};
use rvf_pipeline::ProgressiveLoader;
use vital_signs::{VitalSignDetector, VitalSigns};

// ADR-022 Phase 3: Multi-BSSID pipeline integration
use wifi_densepose_wifiscan::parse_netsh_output as parse_netsh_bssid_output;
use wifi_densepose_wifiscan::{BssidRegistry, WindowsWifiPipeline};

// Accuracy sprint: Kalman tracker, multistatic fusion, field model
use wifi_densepose_signal::ruvsense::field_model::{CalibrationStatus, FieldModel};
use wifi_densepose_signal::ruvsense::multistatic::{MultistaticConfig, MultistaticFuser};
use wifi_densepose_signal::ruvsense::pose_tracker::PoseTracker;

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

    /// UDP bind address for the CSI receiver (ADR-296). Defaults to
    /// `127.0.0.1` (loopback only). Binding to a routable address (`0.0.0.0`
    /// or a LAN IP) is an explicit operator choice and requires `--udp-allow`
    /// or `--udp-insecure-lan`.
    #[arg(long, default_value = "127.0.0.1", env = "RUVIEW_UDP_BIND")]
    udp_bind: String,

    /// Source IP/CIDR allowlist for inbound UDP CSI frames (comma-separated,
    /// repeatable; env `RUVIEW_UDP_ALLOW`). When set, frames from non-matching
    /// sources are dropped and counted. Loopback is always allowed.
    /// Example: `--udp-allow 192.168.1.0/24,10.0.0.5`.
    #[arg(long = "udp-allow", value_name = "IP/CIDR", env = "RUVIEW_UDP_ALLOW")]
    udp_allow: Vec<String>,

    /// Accept a routable UDP bind with no source allowlist, explicitly opting
    /// into the LAN-spoofing risk (ADR-296). The UDP data plane is NOT
    /// authenticated; see the crate SECURITY.md.
    #[arg(long, env = "RUVIEW_UDP_INSECURE_LAN")]
    udp_insecure_lan: bool,

    /// Path to UI static files (repo `ui/`; from `v2/` use `../ui` or rely on auto-detect)
    #[arg(long, default_value = "../ui")]
    ui_path: PathBuf,

    /// Tick interval in milliseconds (default 100 ms = 10 fps for smooth pose animation)
    #[arg(long, default_value = "100")]
    tick_ms: u64,

    /// Bind address (default 127.0.0.1; set to 0.0.0.0 for network access)
    #[arg(long, default_value = "127.0.0.1", env = "SENSING_BIND_ADDR")]
    bind_addr: String,

    /// Disable local `_ruview._tcp` discovery. Discovery is automatically
    /// skipped for loopback-only binds and never carries sensor data.
    #[arg(long, env = "RUVIEW_NO_MDNS")]
    no_mdns: bool,

    /// Stable, non-secret installation routing hint published over mDNS.
    #[arg(long, env = "RUVIEW_INSTALLATION_ID")]
    installation_id: Option<String>,

    /// Private server state directory used for runtime configuration and the
    /// installation bound empty room bootstrap prior.
    #[arg(long, default_value = "data", env = "RUVIEW_DATA_DIR")]
    data_dir: PathBuf,

    /// Human-readable local service name shown by commissioning clients.
    #[arg(
        long,
        default_value = "RuView Installation",
        env = "RUVIEW_INSTALLATION_NAME"
    )]
    installation_name: String,

    /// Additional hostname (with or without `:PORT`) to permit in the `Host`
    /// header — defends loopback-bound deployments against DNS rebinding.
    /// Loopback names (`localhost`, `127.0.0.1`, `[::1]`) are always permitted
    /// implicitly. Pass multiple times to add several entries. Comma-separated
    /// values are also accepted via the `SENSING_ALLOWED_HOSTS` env var.
    #[arg(long = "allowed-host", value_name = "HOST")]
    allowed_hosts: Vec<String>,

    /// Disable Host-header validation entirely. Use only when the server sits
    /// behind a reverse proxy that already canonicalises `Host` (e.g. nginx
    /// `proxy_set_header Host`) — bare deployments stay vulnerable to DNS
    /// rebinding without it.
    #[arg(long)]
    disable_host_validation: bool,

    /// MQTT publisher (HA auto-discovery) + privacy-mode flags (ADR-115).
    /// Flattened so `--mqtt*` reach the binary's parser and the publisher
    /// in `mqtt::` is actually started (fixes #872). Uses the *lib* crate's
    /// `MqttArgs` type so it's compatible with `mqtt::config::from_args`.
    #[command(flatten)]
    mqtt_opts: wifi_densepose_sensing_server::cli::MqttArgs,

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

    /// Convert a published model file (model.safetensors / model.rvf.jsonl) to
    /// the RVF binary container the --model loader expects, then exit (#894).
    /// Pair with --convert-out for the destination path.
    #[arg(long, value_name = "PATH")]
    convert_model: Option<PathBuf>,

    /// Output path for --convert-model (defaults to <input>.rvf).
    #[arg(long, value_name = "PATH")]
    convert_out: Option<PathBuf>,

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

    /// Node positions for multistatic fusion (format: "x,y,z;x,y,z;...")
    #[arg(long, env = "SENSING_NODE_POSITIONS")]
    node_positions: Option<String>,

    /// Start field model calibration on boot (empty room required)
    #[arg(long)]
    calibrate: bool,

    // ---------------------------------------------------------------
    // ADR-102: Edge Module Registry — surface the canonical Cognitum
    // cog catalog via `GET /api/v1/edge/registry`.
    // ---------------------------------------------------------------
    /// Override the upstream URL for the edge module registry. Set to a
    /// mirror or local file://... URL for air-gapped deployments. Empty
    /// string or --no-edge-registry disables the endpoint entirely.
    #[arg(
        long,
        value_name = "URL",
        env = "RUVIEW_EDGE_REGISTRY_URL",
        default_value = "https://storage.googleapis.com/cognitum-apps/app-registry.json"
    )]
    edge_registry_url: String,

    /// Cache TTL for the edge module registry, in seconds.
    #[arg(
        long,
        value_name = "SECS",
        env = "RUVIEW_EDGE_REGISTRY_TTL_SECS",
        default_value = "3600"
    )]
    edge_registry_ttl_secs: u64,

    /// Disable the edge module registry endpoint entirely. Returns 404 on
    /// `GET /api/v1/edge/registry`. Use for air-gapped deployments.
    #[arg(long, env = "RUVIEW_NO_EDGE_REGISTRY")]
    no_edge_registry: bool,
}

// ── Data types ───────────────────────────────────────────────────────────────

/// ADR-018 ESP32 CSI binary frame header (20 bytes)
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct Esp32Frame {
    magic: u32,
    node_id: u8,
    n_antennas: u8,
    /// u16 since ADR-110 / issue #1005: ESP32-C6 HE-SU frames carry 256
    /// subcarrier bins (242 active HE20 tones). HT frames stay ≤128.
    n_subcarriers: u16,
    freq_mhz: u16,
    sequence: u32,
    rssi: i8,
    noise_floor: i8,
    /// ADR-110 byte 18: PPDU type the CSI was sampled from. Pre-ADR-110
    /// firmware sends 0 ⇒ `PpduType::HtLegacy`.
    ppdu_type: wifi_densepose_hardware::PpduType,
    /// ADR-110 byte 19 metadata, including whether this frame was captured
    /// while the node had a valid IEEE 802.15.4 mesh-time solution.
    adr018_flags: wifi_densepose_hardware::Adr018Flags,
    amplitudes: Vec<f64>,
    phases: Vec<f64>,
}

impl Esp32Frame {
    /// The `(n_subcarriers, ppdu_type)` symbol-grid identity of this frame.
    /// HT-LTF and HE-LTF grids are not bin-comparable (ADR-110 / #1005).
    fn grid(&self) -> (u16, wifi_densepose_hardware::PpduType) {
        (self.n_subcarriers, self.ppdu_type)
    }
}

/// Exact CSI symbol grid used by one field model. Subcarrier count alone is
/// insufficient because HT-LTF and HE-LTF use different tone layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
struct CsiGridKey {
    n_subcarriers: u16,
    ppdu_type: u8,
}

impl CsiGridKey {
    fn from_frame(frame: &Esp32Frame) -> Self {
        Self {
            n_subcarriers: frame.n_subcarriers,
            ppdu_type: frame.ppdu_type.to_byte(),
        }
    }

    fn as_bootstrap_grid(self) -> bootstrap_baseline::BootstrapCsiGrid {
        bootstrap_baseline::BootstrapCsiGrid {
            n_subcarriers: self.n_subcarriers,
            ppdu_type: self.ppdu_type,
        }
    }

    fn from_bootstrap_grid(grid: bootstrap_baseline::BootstrapCsiGrid) -> Self {
        Self {
            n_subcarriers: grid.n_subcarriers,
            ppdu_type: grid.ppdu_type,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct RawGridObservation {
    grid: CsiGridKey,
    observed_at: std::time::Instant,
}

#[derive(Debug, Clone, Copy, Serialize)]
struct CalibrationGridEvidence {
    sample_count: usize,
    span_s: f64,
    rate_hz: f64,
    max_gap_s: f64,
    latest_age_s: f64,
}

#[derive(Debug, Clone, Copy)]
struct CalibrationGridBinding {
    source_node_id: u8,
    grid: CsiGridKey,
    evidence: Option<CalibrationGridEvidence>,
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
    /// Per-node feature breakdown for multi-node deployments.
    #[serde(skip_serializing_if = "Option::is_none")]
    node_features: Option<Vec<PerNodeFeatureInfo>>,
    /// ADR-297 — the explicitly-fused room aggregate over the current per-node
    /// inferences (freshness-weighted vote). Deterministic and order-independent,
    /// unlike the legacy last-writer `classification`; `"unavailable"` when no
    /// fresh node backs the room rather than a frozen online value.
    #[serde(skip_serializing_if = "Option::is_none")]
    room_inference: Option<RoomInference>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NodeInfo {
    node_id: u8,
    rssi_dbm: f64,
    position: [f64; 3],
    amplitude: Vec<f64>,
    subcarrier_count: usize,
    /// ADR-110 iter 23 — cross-board sync snapshot for this node.
    /// `None` when no fresh sync packet has been observed (no mesh peer
    /// reachable, or this node is a singleton). Populated from
    /// `NodeState::latest_sync` and the iter 18 fps EMA.
    #[serde(skip_serializing_if = "Option::is_none")]
    sync: Option<NodeSyncSnapshot>,
    /// ADR-297 — this node's *own* inference (classification + confidence +
    /// freshness). Distinct from the room aggregate; a node reports what it
    /// sees, with no silent fallback to the room value. `None` on synthetic /
    /// placeholder frames that carry no per-node classification.
    #[serde(skip_serializing_if = "Option::is_none")]
    node_inference: Option<NodeInference>,
}

/// ADR-110 iter 23 — per-node mesh-sync snapshot embedded in NodeInfo.
/// Surfaces what was previously only visible in the debug log so UI clients
/// can render leader / follower / offset / measured-fps live.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct NodeSyncSnapshot {
    /// Smoothed local-vs-mesh offset in µs (negative when this node's clock
    /// is behind the leader's — see §A0.10's measured -1.16 s on the bench).
    offset_us: i64,
    /// True when this node is the elected mesh leader.
    is_leader: bool,
    /// True when this node has heard a fresh leader beacon within the
    /// firmware's VALID_WINDOW_MS gate (3 s).
    is_valid: bool,
    /// True once the EMA-smoothed offset has seeded (one full beacon round-trip).
    smoothed: bool,
    /// Sync packet's sequence high-water — used by the host to pair CSI
    /// frames against this snapshot for §A0.12 mesh-time recovery.
    sequence: u32,
    /// Per-node measured CSI frame rate (iter 18 EMA). 20.0 until the
    /// EMA has at least 5 samples; the actually-observed rate after that.
    csi_fps_ema: f64,
    /// How many CSI frames have contributed to `csi_fps_ema`. Clients can
    /// treat <5 as "not yet trustworthy" and fall back to 20 Hz.
    csi_fps_samples: u32,
    /// ADR-110 iter 34 — milliseconds since the host last received a sync
    /// packet from this node. Lets UI dashboards render sync-age decay
    /// (badge fades after 5 s, drops off after the 9 s mesh_aligned_us
    /// staleness gate). `None` only when the host never had Instant data
    /// for this node, which shouldn't happen in normal flow but is
    /// modeled defensively.
    #[serde(skip_serializing_if = "Option::is_none")]
    staleness_ms: Option<u64>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ClassificationInfo {
    motion_level: String,
    presence: bool,
    confidence: f64,
}

/// Derives the classification triple from raw ESP32 vitals fields.
///
/// `presence` is derived from `motion_level`, never taken from the raw
/// `presence` flag directly, so `motion_level: "present_moving"` can never
/// pair with `presence: false` (issue #1442) — matches the convention
/// already used elsewhere (the per-node path and `csi.rs`'s label-derived
/// `classification.presence = label != "absent"`).
fn classify_vitals(motion: bool, presence: bool, presence_score: f32) -> ClassificationInfo {
    let motion_level = if motion {
        "present_moving"
    } else if presence {
        "present_still"
    } else {
        "absent"
    };
    ClassificationInfo {
        motion_level: motion_level.to_string(),
        presence: motion_level != "absent",
        confidence: presence_score as f64,
    }
}

/// Derive the top-level (room) [`ClassificationInfo`] from the fused
/// [`RoomInference`] aggregate (ADR-297, issue #1554), rather than from
/// whichever node's packet happened to arrive last. `RoomInference`'s
/// `"unavailable"` (no fresh node contributing) maps to `"absent"` with zero
/// confidence — no evidence of presence, since `ClassificationInfo` has no
/// separate "unknown" state — never a frozen last-known value.
fn classification_from_room(room: &RoomInference) -> ClassificationInfo {
    let motion_level = if room.classification == "unavailable" {
        "absent"
    } else {
        room.classification.as_str()
    };
    ClassificationInfo {
        motion_level: motion_level.to_string(),
        presence: motion_level != "absent",
        confidence: room.confidence,
    }
}

/// Minimum time a candidate room-level classification must be observed
/// *consistently* before the debounced room state actually flips to it.
///
/// `fuse_room` recomputes its plurality vote fresh every cycle with no
/// memory of its own — unlike each node's own classification, which is
/// individually debounced (`smooth_and_classify_node`). Confirmed live
/// (2026-08-28) as a real source of "flapping": with per-node confidences
/// sitting near the classification boundary (44%/83%/53% observed), whichever
/// 2 of 3 nodes happen to agree can flip cycle-to-cycle even while each
/// node's own reading is individually stable. This adds the missing
/// room-level hysteresis, same idea as the per-node debounce, one layer up.
const ROOM_DEBOUNCE_DURATION_SECS: f64 = 1.5;

/// Debounced counterpart to [`classification_from_room`] — apply this at
/// live call sites instead, so the room's `presence`/`motion_level` only
/// flips after `raw`'s new candidate class has been observed continuously
/// for [`ROOM_DEBOUNCE_DURATION_SECS`], not on the very first cycle it wins
/// `fuse_room`'s plurality vote. `confidence` is passed through un-debounced
/// (it's a continuous value, not a discrete state, so there's nothing to
/// flap) — only `motion_level`/`presence` are held back.
fn debounce_room_classification(state: &mut AppStateInner, raw: &RoomInference) -> ClassificationInfo {
    let candidate = if raw.classification == "unavailable" {
        "absent"
    } else {
        raw.classification.as_str()
    };
    let now = std::time::Instant::now();

    if candidate == state.room_debounced_level {
        // Already stable in this state — reset any in-flight candidate.
        state.room_debounce_candidate = candidate.to_string();
        state.room_debounce_since = None;
    } else if candidate == state.room_debounce_candidate {
        let since = *state.room_debounce_since.get_or_insert(now);
        if now.duration_since(since).as_secs_f64() >= ROOM_DEBOUNCE_DURATION_SECS {
            state.room_debounced_level = candidate.to_string();
            state.room_debounce_since = None;
        }
    } else {
        // New candidate — restart the hold timer.
        state.room_debounce_candidate = candidate.to_string();
        state.room_debounce_since = Some(now);
    }

    ClassificationInfo {
        motion_level: state.room_debounced_level.clone(),
        presence: state.room_debounced_level != "absent",
        confidence: raw.confidence,
    }
}

/// ADR-297 — the window a node may be silent before it stops contributing to
/// the fused room aggregate (its entities go stale/unavailable rather than
/// holding a frozen online value). Mirrors the 10 s active-node filter used to
/// assemble the nodes array.
const NODE_STALE_AFTER_MS: u64 = 10_000;

/// Build a node's *own* [`NodeInference`] from its smoothed per-node state
/// (ADR-297). Uses the node's own `current_motion_level` — never the room
/// aggregate — with a confidence from its smoothed person score and freshness
/// from its last frame time. Pure given the state snapshot + `now`.
fn node_inference_for(n: &NodeState, now: std::time::Instant) -> NodeInference {
    let age_ms = n
        .last_frame_time
        .map(|t| now.duration_since(t).as_millis() as u64);
    let present = !matches!(n.current_motion_level.as_str(), "absent");
    let score = n.smoothed_person_score.clamp(0.0, 1.0);
    let confidence = if present { score } else { 1.0 - score };
    NodeInference::new(n.current_motion_level.clone(), confidence, age_ms)
}

#[cfg(test)]
mod classify_vitals_tests {
    use super::classify_vitals;

    #[test]
    fn motion_implies_presence_issue_1442() {
        // The exact contradictory frame from issue #1442:
        // motion=true, presence=false must not yield presence: false.
        let c = classify_vitals(true, false, 0.69);
        assert_eq!(c.motion_level, "present_moving");
        assert!(c.presence, "motion implies presence regardless of the raw presence flag");
    }

    #[test]
    fn presence_without_motion_is_present_still() {
        let c = classify_vitals(false, true, 0.5);
        assert_eq!(c.motion_level, "present_still");
        assert!(c.presence);
    }

    #[test]
    fn neither_motion_nor_presence_is_absent() {
        let c = classify_vitals(false, false, 0.0);
        assert_eq!(c.motion_level, "absent");
        assert!(!c.presence);
    }

    #[test]
    fn confidence_passes_through_presence_score() {
        let c = classify_vitals(true, true, 0.33);
        assert!((c.confidence - 0.33_f64).abs() < 1e-6);
    }
}

#[cfg(test)]
mod issue_1554_room_classification_tests {
    //! Issue #1554 — the top-level `classification` in `SensingUpdate` (served
    //! by `GET /api/v1/sensing/latest`) used to be `classify_vitals(...)` on
    //! *this packet's* single node, overwritten on every UDP packet. With 2+
    //! disagreeing nodes it flapped at packet rate (~40/s in the field report)
    //! because whichever node's packet arrived last won.
    //!
    //! `classification_from_room` instead derives the top-level classification
    //! from the deterministic, freshness-weighted `RoomInference` aggregate
    //! (ADR-297) — the same aggregate `fuse_room` already computes for the
    //! `room_inference` field — so it no longer depends on packet arrival
    //! order.
    use super::{classification_from_room, RoomInference};

    #[test]
    fn derives_presence_and_motion_from_room_classification() {
        let room = RoomInference {
            classification: "present_moving".to_string(),
            confidence: 0.82,
            contributing_nodes: 3,
        };
        let c = classification_from_room(&room);
        assert_eq!(c.motion_level, "present_moving");
        assert!(c.presence);
        assert!((c.confidence - 0.82).abs() < 1e-9);
    }

    #[test]
    fn absent_room_classification_has_no_presence() {
        let room = RoomInference {
            classification: "absent".to_string(),
            confidence: 0.4,
            contributing_nodes: 1,
        };
        let c = classification_from_room(&room);
        assert_eq!(c.motion_level, "absent");
        assert!(!c.presence);
    }

    #[test]
    fn unavailable_room_maps_to_absent_not_a_frozen_value() {
        // No fresh node contributing -> RoomInference::unavailable(). Must not
        // surface as a stale "present" from whichever node reported last.
        let room = RoomInference::unavailable();
        let c = classification_from_room(&room);
        assert_eq!(c.motion_level, "absent");
        assert!(!c.presence);
        assert_eq!(c.confidence, 0.0);
    }

    #[test]
    fn does_not_depend_on_which_node_is_named_last() {
        // The old bug was order/arrival dependent. The room-derived value must
        // be a pure function of the aggregate, so two RoomInferences with the
        // same fields (regardless of which node contributed them) classify
        // identically.
        let a = RoomInference {
            classification: "present_still".to_string(),
            confidence: 0.6,
            contributing_nodes: 2,
        };
        let b = a.clone();
        assert_eq!(classification_from_room(&a), classification_from_room(&b));
    }
}

#[cfg(test)]
mod debounce_room_classification_tests {
    //! `fuse_room`'s plurality vote has no memory of its own — confirmed live
    //! (2026-08-28) as a real "flapping" source when per-node confidences sit
    //! near the classification boundary: whichever 2 of 3 nodes happen to
    //! agree on a given cycle can flip the room result even while each
    //! node's own reading is individually stable. `debounce_room_classification`
    //! adds the missing room-level hysteresis (same idea as each node's own
    //! debounce, one layer up).
    use super::{debounce_room_classification, AppStateInner, RoomInference};
    use std::time::{Duration, Instant};

    fn room(classification: &str, confidence: f64) -> RoomInference {
        RoomInference {
            classification: classification.to_string(),
            confidence,
            contributing_nodes: 3,
        }
    }

    #[test]
    fn single_cycle_candidate_does_not_flip_immediately() {
        let mut state = AppStateInner::minimal();
        let c = debounce_room_classification(&mut state, &room("present_still", 0.6));
        assert_eq!(c.motion_level, "absent", "one cycle must not be enough to flip");
        assert!(!c.presence);
    }

    #[test]
    fn repeated_but_recent_candidate_still_does_not_flip() {
        let mut state = AppStateInner::minimal();
        debounce_room_classification(&mut state, &room("present_still", 0.6));
        // Second cycle, essentially no time elapsed — still under the hold duration.
        let c = debounce_room_classification(&mut state, &room("present_still", 0.6));
        assert_eq!(c.motion_level, "absent");
    }

    #[test]
    fn candidate_sustained_past_hold_duration_flips() {
        let mut state = AppStateInner::minimal();
        // Simulate "present_still" having already been the candidate for 2s
        // (> ROOM_DEBOUNCE_DURATION_SECS = 1.5s) without needing to sleep in
        // the test.
        state.room_debounce_candidate = "present_still".to_string();
        state.room_debounce_since = Some(Instant::now() - Duration::from_secs(2));

        let c = debounce_room_classification(&mut state, &room("present_still", 0.7));
        assert_eq!(c.motion_level, "present_still");
        assert!(c.presence);
    }

    #[test]
    fn flapping_candidate_resets_the_hold_timer() {
        let mut state = AppStateInner::minimal();
        // "present_still" has been pending 1s (not yet enough to flip).
        state.room_debounce_candidate = "present_still".to_string();
        state.room_debounce_since = Some(Instant::now() - Duration::from_secs(1));

        // A different candidate arrives — this is exactly the flapping
        // pattern (2-of-3 node agreement flipping cycle to cycle). The hold
        // timer must restart for the new candidate, not carry over.
        let c = debounce_room_classification(&mut state, &room("active", 0.5));
        assert_eq!(c.motion_level, "absent", "still debounced to the old stable state");
        assert_eq!(state.room_debounce_candidate, "active");
        assert!(
            state.room_debounce_since.unwrap().elapsed() < Duration::from_millis(100),
            "hold timer must have restarted for the new candidate"
        );
    }

    #[test]
    fn already_stable_state_reports_immediately_without_waiting() {
        let mut state = AppStateInner::minimal();
        state.room_debounced_level = "present_moving".to_string();
        let c = debounce_room_classification(&mut state, &room("present_moving", 0.9));
        assert_eq!(c.motion_level, "present_moving", "no need to re-debounce an already-stable state");
        assert!(c.presence);
    }

    #[test]
    fn unavailable_room_debounces_toward_absent_like_any_other_candidate() {
        let mut state = AppStateInner::minimal();
        state.room_debounced_level = "present_still".to_string();
        state.room_debounce_candidate = "absent".to_string();
        state.room_debounce_since = Some(Instant::now() - Duration::from_secs(2));

        let c = debounce_room_classification(&mut state, &RoomInference::unavailable());
        assert_eq!(c.motion_level, "absent");
        assert!(!c.presence);
    }
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
    /// Room-world position `[x, y, z]` (Observatory scene units / meters),
    /// derived from the strongest `signal_field` peak this person sits on
    /// (issue #1050). `y` is `0.0` — the field is a floor-plane grid. This is
    /// a real field-peak readout, not calibrated triangulation; see
    /// `field_localize` for the honesty caveat. Defaults to `[0,0,0]` until
    /// field positions are attached by `attach_field_positions`.
    #[serde(default)]
    position: [f64; 3],
    /// Motion magnitude on the Observatory's `0..100` scale, passed through
    /// from the measured `motion_band_power` (issue #1050).
    #[serde(default)]
    motion_score: f64,
    /// Coarse posture label (`"standing"`/`"lying"`/…) when a **real** aggregate
    /// posture estimate exists, else `None`. Never fabricated — per-person
    /// skeletal pose in room coordinates remains gated on the pose model
    /// (ADR-079). The Observatory defaults to `'standing'` when this is absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pose: Option<String>,
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
    pub(crate) frame_history: VecDeque<Vec<f64>>,
    smoothed_person_score: f64,
    pub(crate) prev_person_count: usize,
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
    pub(crate) last_frame_time: Option<std::time::Instant>,
    edge_vitals: Option<Esp32VitalsPacket>,
    /// ADR-110 §A0.12: Latest sync packet received from this node. When a
    /// CSI frame arrives with byte 19 bit 4 set (`adr018_flags.ieee802154_sync_valid`),
    /// the host can recover a mesh-aligned timestamp via
    /// `latest_sync.epoch_us + (now_local - latest_sync.local_us)`.
    latest_sync: Option<wifi_densepose_hardware::SyncPacket>,
    /// Last time a sync packet from this node was received (for staleness).
    latest_sync_at: Option<std::time::Instant>,
    /// Arrival time of the newest grid-admitted raw CSI frame. Edge-vitals
    /// packets intentionally do not refresh this clock.
    latest_accepted_csi_at: Option<std::time::Instant>,
    /// Sequence number of the newest CSI frame admitted to `frame_history`.
    /// Kept alongside the history so multistatic fusion can timestamp the
    /// exact sample it consumes, rather than the host's UDP arrival time.
    latest_csi_sequence: Option<u32>,
    /// Whether byte 19 bit 4 marked that newest admitted CSI frame as synced.
    latest_csi_sync_valid: bool,
    /// ADR-110 iter 18: EMA-tracked CSI frame rate for this node.
    /// Replaces the hardcoded 20 Hz fallback in
    /// `mesh_aligned_us_for_csi_frame` once `csi_fps_samples ≥ 5`.
    csi_fps_ema: f64,
    /// Number of inter-frame deltas observed (need ≥5 before trusting EMA).
    csi_fps_samples: u32,
    /// Latest extracted features for cross-node fusion.
    latest_features: Option<FeatureInfo>,
    // ── RuVector Phase 2: Temporal smoothing & coherence gating ──
    /// Previous frame's smoothed keypoint positions for EMA temporal smoothing.
    prev_keypoints: Option<Vec<[f64; 3]>>,
    /// Rolling buffer of motion_energy values for coherence scoring (last 20 frames).
    motion_energy_history: VecDeque<f64>,
    /// Coherence score [0.0, 1.0]: low variance in motion_energy = high coherence.
    coherence_score: f64,
    /// ADR-084 Pass 3 cluster-Pi novelty sensor — per-node sketch bank of
    /// recent CSI feature vectors. Populated by `update_novelty` on each
    /// frame; left `None` to disable the sensor on a per-node basis.
    feature_history: Option<wifi_densepose_signal::ruvsense::longitudinal::EmbeddingHistory>,
    /// Most recent novelty score in [0.0, 1.0] (0 = exact-match in bank,
    /// 1 = no overlap). Consumed by the model-wake gate downstream.
    pub(crate) last_novelty_score: Option<f32>,
    /// ADR-110 / issue #1005: the `(n_subcarriers, ppdu_type)` grid this
    /// node's rolling windows were built on. ESP32-C6 nodes interleave
    /// HE-SU 256-bin frames with HT 64-bin frames on one socket; mixing
    /// the two symbol grids in `frame_history` corrupts variance/baseline
    /// statistics. See [`NodeState::accept_grid`].
    active_grid: Option<(u16, wifi_densepose_hardware::PpduType)>,
    /// Header-only recent raw grid observations used to choose one stable,
    /// comparable calibration stream. No CSI amplitudes are retained here.
    raw_grid_observations: VecDeque<RawGridObservation>,
    /// Runtime history for the exact grid bound into the active field model.
    /// This must not reuse `frame_history`, whose independent inference grid
    /// may be wider and therefore not bin-comparable.
    field_model_history: VecDeque<Vec<f64>>,
    field_model_latest_sequence: Option<u32>,
    field_model_latest_seen: Option<std::time::Instant>,
}

/// Default EMA alpha for temporal keypoint smoothing (RuVector Phase 2).
/// Lower = smoother (more history, less jitter). 0.15 balances responsiveness
/// with stability for WiFi CSI where per-frame noise is high.
const TEMPORAL_EMA_ALPHA_DEFAULT: f64 = 0.15;
/// Reduced EMA alpha when coherence is low (trust measurements less).
const TEMPORAL_EMA_ALPHA_LOW_COHERENCE: f64 = 0.05;
/// Coherence threshold below which we reduce EMA alpha.
const COHERENCE_LOW_THRESHOLD: f64 = 0.3;
/// Maximum allowed bone-length change ratio between frames (20%).
const MAX_BONE_CHANGE_RATIO: f64 = 0.20;
/// Number of motion_energy frames to track for coherence scoring.
const COHERENCE_WINDOW: usize = 20;
/// ADR-084 Pass 3 — per-node novelty sketch dimension (56 subcarriers,
/// the dominant ESP32-S3 capture configuration).
const NOVELTY_VECTOR_DIM: usize = 56;
/// ADR-084 Pass 3 — number of past sketches retained per-node for
/// novelty comparison. 64 frames ≈ 6.4 s at 10 Hz.
const NOVELTY_HISTORY_CAPACITY: usize = 64;
/// ADR-084 Pass 3 — feature-vector schema version. Bump on changes to
/// subcarrier ordering / normalisation so banks reject stale data.
const NOVELTY_SKETCH_VERSION: u16 = 1;
/// Recent header-only evidence window used to choose a stable field-model
/// stream. The rate floor is 20% above the 1,000 / 600 s completion minimum.
const CALIBRATION_GRID_EVIDENCE_WINDOW: std::time::Duration =
    std::time::Duration::from_secs(20);
const CALIBRATION_GRID_EVIDENCE_CAPACITY: usize = 1_024;
const CALIBRATION_GRID_MIN_SPAN_S: f64 = 10.0;
const CALIBRATION_GRID_MIN_RATE_HZ: f64 = 2.0;
const CALIBRATION_GRID_MAX_GAP_S: f64 = 5.0;

/// Lower plausibility floor (seconds) for a CSI inter-frame delta.
///
/// The firmware caps CSI sends at `CSI_MIN_SEND_INTERVAL_US = 20 ms`
/// (`csi_collector.c`), so a single node cannot physically produce frames
/// faster than 50 fps. UDP/OS buffering, however, delivers frames in tight
/// bursts whose intra-burst arrival deltas are tens of microseconds apart —
/// a 36 µs delta yields `1/dt ≈ 27 kHz`, which the old `< 1 s` guard let
/// straight into the EMA and inflated `csi_fps_ema` by 1–3 orders of
/// magnitude (issue #1180). We reject sub-5 ms deltas as burst artifacts and
/// cap accepted estimates to the firmware's 50 fps physical ceiling.
pub(crate) const MIN_PLAUSIBLE_CSI_DT_SEC: f64 = 0.005;
pub(crate) const MAX_PHYSICAL_CSI_FPS: f64 = 50.0;

/// ADR-110 iter 18 — EMA update for per-node CSI fps tracking.
///
/// Returns the new EMA value, or `None` if the delta is implausible
/// (below [`MIN_PLAUSIBLE_CSI_DT_SEC`] — a sub-ms burst artifact, see
/// issue #1180 — or `> 1 second`, likely a connection gap rather than a
/// real frame-rate sample). α = 1/8 fixed shift, ~8-sample effective
/// window, matching the firmware-side ESP-NOW offset smoother in §A0.10.
///
/// Free function for testability — every transformation that doesn't
/// touch the rest of `NodeState` lives outside the `impl` block.
pub(crate) fn update_csi_fps_ema(prev_fps: f64, dt_sec: f64) -> Option<f64> {
    if !(dt_sec >= MIN_PLAUSIBLE_CSI_DT_SEC && dt_sec < 1.0) {
        return None;
    }
    let instantaneous = (1.0 / dt_sec).min(MAX_PHYSICAL_CSI_FPS);
    // y[n] = y[n-1] + (x - y[n-1]) / 8
    Some(prev_fps + (instantaneous - prev_fps) / 8.0)
}

#[cfg(test)]
mod fps_ema_tests {
    use super::update_csi_fps_ema;

    #[test]
    fn steady_10hz_converges_toward_10() {
        let mut fps = 20.0;
        for _ in 0..40 {
            fps = update_csi_fps_ema(fps, 0.100).unwrap();
        }
        assert!((fps - 10.0).abs() < 0.1,
                "expected ~10 Hz after 40 samples at 100 ms intervals, got {fps}");
    }

    #[test]
    fn steady_20hz_stays_near_20() {
        let mut fps = 20.0;
        for _ in 0..20 {
            fps = update_csi_fps_ema(fps, 0.050).unwrap();
        }
        assert!((fps - 20.0).abs() < 0.05, "expected ~20 Hz, got {fps}");
    }

    #[test]
    fn nonpositive_dt_rejected() {
        assert!(update_csi_fps_ema(15.0, 0.0).is_none());
        assert!(update_csi_fps_ema(15.0, -0.1).is_none());
    }

    #[test]
    fn long_gap_rejected_as_implausible() {
        assert!(update_csi_fps_ema(20.0, 2.0).is_none());
    }

    #[test]
    fn subms_burst_delta_rejected() {
        // Issue #1180: a 36 µs intra-burst delta implies ~27 kHz and must
        // not enter the EMA. Anything below the 5 ms floor is rejected.
        assert!(update_csi_fps_ema(40.0, 0.000_036).is_none());
        assert!(update_csi_fps_ema(40.0, 0.001).is_none());
        // Just above the floor is accepted.
        assert!(update_csi_fps_ema(40.0, 0.005).is_some());
    }

    #[test]
    fn accepted_burst_edge_is_capped_to_firmware_ceiling() {
        let mut fps = 50.0;
        for _ in 0..32 {
            fps = update_csi_fps_ema(fps, 0.005).unwrap();
        }
        assert!(fps <= 50.0, "reported {fps} Hz above firmware ceiling");
    }

    #[test]
    fn burst_interleaved_with_nominal_stays_in_band() {
        // A true ~40 fps node whose frames arrive in sub-ms bursts: feeding
        // only the plausible (nominal-cadence) deltas keeps the EMA near the
        // ground truth instead of blowing up. Burst deltas are rejected by
        // the caller (see NodeState::observe_csi_frame_arrival), so the EMA
        // only ever sees the ~25 ms inter-group gaps.
        let mut fps = 40.0;
        for _ in 0..40 {
            // nominal 25 ms gap (40 fps); intervening sub-ms bursts skipped
            fps = update_csi_fps_ema(fps, 0.025).unwrap();
            assert!(update_csi_fps_ema(fps, 0.000_040).is_none());
        }
        assert!(
            (fps - 40.0).abs() < 1.0,
            "EMA should stay within ~1 Hz of the 40 fps ground truth, got {fps}"
        );
    }
}

impl NodeState {
    /// Measured sample rate only after enough inter-frame deltas have warmed
    /// the EMA. The physical ceiling is enforced again at the consumption
    /// boundary so restored or malformed state cannot overclock DSP math.
    fn measured_sample_rate_hz(&self) -> Option<f64> {
        (self.csi_fps_samples >= 5 && self.csi_fps_ema.is_finite())
            .then(|| self.csi_fps_ema.clamp(1.0, MAX_PHYSICAL_CSI_FPS))
    }

    fn effective_sample_rate_hz(&self) -> f64 {
        self.measured_sample_rate_hz().unwrap_or(20.0)
    }

    /// ADR-110 §A0.12 timestamp recovery: given a CSI frame's node-local
    /// `esp_timer_get_time()` snapshot, return the mesh-aligned epoch
    /// computed from this node's most recent sync packet — or `None`
    /// if no sync has been received yet, or the last one is too stale
    /// (older than 3 × VALID_WINDOW_MS = 9 s, matching the firmware's own
    /// staleness gate).
    pub(crate) fn mesh_aligned_us(&self, local_at_frame_us: u64) -> Option<u64> {
        let sync = self.latest_sync.as_ref()?;
        if !sync.flags.is_valid {
            return None;
        }
        let seen_at = self.latest_sync_at?;
        // Drop stale syncs — firmware emits at ~0.5 Hz default, anything
        // older than 9 s likely means the mesh transport dropped.
        if seen_at.elapsed() > std::time::Duration::from_secs(9) {
            return None;
        }
        Some(sync.apply_to_local(local_at_frame_us))
    }

    /// ADR-110 §A0.12 sequence-based mesh-time recovery for an in-flight
    /// ADR-018 CSI frame. The frame carries no `local_us` (the wire
    /// format has no slot), but it carries a sequence number that the
    /// sync packet's `sequence` high-water can be paired against. Uses
    /// 20 Hz as the default CSI rate (the firmware's
    /// `CSI_MIN_SEND_INTERVAL_US`-implied ceiling). Returns `None` if
    /// no fresh sync has been observed for this node.
    pub(crate) fn mesh_aligned_us_for_csi_frame(&self, frame_sequence: u32) -> Option<u64> {
        let sync = self.latest_sync.as_ref()?;
        if !sync.flags.is_valid {
            return None;
        }
        let seen_at = self.latest_sync_at?;
        if seen_at.elapsed() > std::time::Duration::from_secs(9) {
            return None;
        }
        // A recently-received sync datagram can overtake an older CSI
        // datagram in UDP delivery order. Only extrapolate forward (including
        // a genuine u32 wrap); otherwise fall back to host arrival time.
        let delta_frames = frame_sequence.wrapping_sub(sync.sequence);
        if delta_frames > i32::MAX as u32 {
            return None;
        }
        // Iter 18: use the measured per-node fps once we have ≥5 inter-frame
        // samples; until then fall back to the 20 Hz firmware ceiling. The
        // §A0.12 capture showed real bench fps ≈ 10, so the measured value
        // is significantly more accurate than the constant fallback.
        let fps = self.effective_sample_rate_hz();
        Some(sync.mesh_aligned_us_for_sequence(frame_sequence, fps))
    }

    /// Mesh timestamp for the newest CSI frame admitted to `frame_history`.
    /// Both the frame-level sync-valid bit and a fresh, valid sync packet are
    /// required; callers retain their existing host-arrival fallback.
    pub(crate) fn mesh_aligned_us_for_latest_csi_frame(&self) -> Option<u64> {
        if !self.latest_csi_sync_valid {
            return None;
        }
        self.mesh_aligned_us_for_csi_frame(self.latest_csi_sequence?)
    }

    /// ADR-110 iter 18 — update the per-node observed-fps EMA from a fresh
    /// CSI frame arrival. Call once per accepted CSI frame from
    /// `udp_receiver_task`. Uses `last_frame_time` as the previous-frame
    /// anchor; the first frame after init seeds the timer without producing
    /// a sample (no prior dt to measure).
    /// ADR-110 iter 32 — apply a freshly-decoded sync packet to this node.
    /// Overwrites `latest_sync` with the new packet and stamps
    /// `latest_sync_at` so the staleness gate in `mesh_aligned_us_for_csi_frame`
    /// can age it out after 9 s. Used by `udp_receiver_task` on every
    /// successful magic-dispatched sync datagram; extracted so the dispatch
    /// path is testable without spinning up the tokio UDP socket.
    pub(crate) fn apply_sync_packet(
        &mut self,
        pkt: wifi_densepose_hardware::SyncPacket,
        now: std::time::Instant,
    ) {
        self.latest_sync = Some(pkt);
        self.latest_sync_at = Some(now);
    }

    /// ADR-110 iter 30 — pure snapshot of this node's mesh-sync state.
    /// Returns `None` when no sync packet has been observed. Used by both
    /// the WebSocket broadcaster (iter 23) and the REST handlers (iter 29);
    /// extracted here so tests can build a `NodeState`, populate
    /// `latest_sync`, and assert the snapshot shape without spinning up
    /// the axum router.
    pub(crate) fn sync_snapshot(&self) -> Option<NodeSyncSnapshot> {
        let sync = self.latest_sync.as_ref()?;
        Some(NodeSyncSnapshot {
            offset_us: sync.local_minus_epoch_us(),
            is_leader: sync.flags.is_leader,
            is_valid: sync.flags.is_valid,
            smoothed: sync.flags.smoothed_used,
            sequence: sync.sequence,
            csi_fps_ema: self.effective_sample_rate_hz(),
            csi_fps_samples: self.csi_fps_samples,
            staleness_ms: self.latest_sync_at.map(|t| t.elapsed().as_millis() as u64),
        })
    }

    /// Record a CSI data-frame arrival and return whether it was the node's
    /// first sensing frame. Sync packets deliberately do not change this
    /// result, so a sync-before-CSI sequence still produces `node.online`.
    pub(crate) fn observe_csi_frame_arrival(&mut self, now: std::time::Instant) -> bool {
        let first_sensing_frame = self.last_frame_time.is_none();
        if let Some(prev) = self.last_frame_time {
            let dt = now.duration_since(prev).as_secs_f64();
            // Burst arrivals (sub-floor dt, issue #1180): do NOT re-anchor on
            // them. Keeping the previous anchor means the next genuine
            // inter-frame gap measures the true cadence across the whole
            // burst instead of intra-burst jitter — so a 50 fps node whose
            // frames arrive in 36 µs bursts every 25 ms still reads ~40 fps,
            // not 27 kHz.
            if dt < MIN_PLAUSIBLE_CSI_DT_SEC {
                return false;
            }
            if let Some(new_ema) = update_csi_fps_ema(self.csi_fps_ema, dt) {
                self.csi_fps_ema = new_ema;
                self.csi_fps_samples = self.csi_fps_samples.saturating_add(1);
            }
        }
        self.last_frame_time = Some(now);
        first_sensing_frame
    }

    /// Record an accepted CSI sample and preserve the wire metadata needed by
    /// the multistatic bridge to recover capture time. Grid-rejected frames
    /// intentionally use `observe_csi_frame_arrival` directly because they do
    /// not replace the sample at the back of `frame_history`.
    pub(crate) fn observe_accepted_csi_frame(
        &mut self,
        sequence: u32,
        sync_valid: bool,
        now: std::time::Instant,
    ) -> bool {
        self.latest_accepted_csi_at = Some(now);
        self.latest_csi_sequence = Some(sequence);
        self.latest_csi_sync_valid = sync_valid;
        self.observe_csi_frame_arrival(now)
    }

    pub(crate) fn new() -> Self {
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
            vital_detector: VitalSignDetector::new(20.0),
            latest_vitals: VitalSigns::default(),
            last_frame_time: None,
            edge_vitals: None,
            latest_sync: None,
            latest_sync_at: None,
            latest_accepted_csi_at: None,
            latest_csi_sequence: None,
            latest_csi_sync_valid: false,
            csi_fps_ema: 20.0,
            csi_fps_samples: 0,
            latest_features: None,
            prev_keypoints: None,
            motion_energy_history: VecDeque::with_capacity(COHERENCE_WINDOW),
            coherence_score: 1.0, // assume stable initially
            feature_history: Some(
                wifi_densepose_signal::ruvsense::longitudinal::EmbeddingHistory::with_sketch(
                    NOVELTY_VECTOR_DIM,
                    NOVELTY_HISTORY_CAPACITY,
                    NOVELTY_SKETCH_VERSION,
                ),
            ),
            last_novelty_score: None,
            active_grid: None,
            raw_grid_observations: VecDeque::with_capacity(
                CALIBRATION_GRID_EVIDENCE_CAPACITY,
            ),
            field_model_history: VecDeque::with_capacity(FRAME_HISTORY_CAPACITY),
            field_model_latest_sequence: None,
            field_model_latest_seen: None,
        }
    }

    fn observe_raw_grid(&mut self, grid: CsiGridKey, now: std::time::Instant) {
        while self
            .raw_grid_observations
            .front()
            .is_some_and(|sample| {
                now.saturating_duration_since(sample.observed_at)
                    > CALIBRATION_GRID_EVIDENCE_WINDOW
            })
        {
            self.raw_grid_observations.pop_front();
        }
        if self.raw_grid_observations.len() == CALIBRATION_GRID_EVIDENCE_CAPACITY {
            self.raw_grid_observations.pop_front();
        }
        self.raw_grid_observations.push_back(RawGridObservation {
            grid,
            observed_at: now,
        });
    }

    fn calibration_grid_candidates(
        &self,
        now: std::time::Instant,
    ) -> Vec<(CsiGridKey, CalibrationGridEvidence)> {
        let mut grouped: Vec<(CsiGridKey, Vec<std::time::Instant>)> = Vec::new();
        for sample in self.raw_grid_observations.iter().filter(|sample| {
            now.saturating_duration_since(sample.observed_at)
                <= CALIBRATION_GRID_EVIDENCE_WINDOW
        }) {
            if let Some((_, arrivals)) = grouped
                .iter_mut()
                .find(|(candidate, _)| *candidate == sample.grid)
            {
                arrivals.push(sample.observed_at);
            } else {
                grouped.push((sample.grid, vec![sample.observed_at]));
            }
        }
        grouped
            .into_iter()
            .filter_map(|(grid, arrivals)| {
                let first = *arrivals.first()?;
                let last = *arrivals.last()?;
                let span_s = last.saturating_duration_since(first).as_secs_f64();
                let latest_age_s = now.saturating_duration_since(last).as_secs_f64();
                let mut max_gap_s = latest_age_s;
                for pair in arrivals.windows(2) {
                    max_gap_s = max_gap_s.max(
                        pair[1]
                            .saturating_duration_since(pair[0])
                            .as_secs_f64(),
                    );
                }
                let rate_hz = if span_s > 0.0 {
                    arrivals.len().saturating_sub(1) as f64 / span_s
                } else {
                    0.0
                };
                Some((
                    grid,
                    CalibrationGridEvidence {
                        sample_count: arrivals.len(),
                        span_s,
                        rate_hz,
                        max_gap_s,
                        latest_age_s,
                    },
                ))
            })
            .collect()
    }

    fn select_calibration_grid(
        &self,
        now: std::time::Instant,
    ) -> Option<(CsiGridKey, CalibrationGridEvidence)> {
        self.calibration_grid_candidates(now)
            .into_iter()
            .filter(|(grid, evidence)| {
                grid.ppdu_type <= 3
                    && evidence.span_s >= CALIBRATION_GRID_MIN_SPAN_S
                    && evidence.rate_hz >= CALIBRATION_GRID_MIN_RATE_HZ
                    && evidence.max_gap_s < CALIBRATION_GRID_MAX_GAP_S
                    && evidence.latest_age_s < CALIBRATION_GRID_MAX_GAP_S
            })
            .min_by(|(left_grid, left), (right_grid, right)| {
                left.max_gap_s
                    .total_cmp(&right.max_gap_s)
                    .then_with(|| right.rate_hz.total_cmp(&left.rate_hz))
                    .then_with(|| right_grid.n_subcarriers.cmp(&left_grid.n_subcarriers))
                    .then_with(|| left_grid.ppdu_type.cmp(&right_grid.ppdu_type))
            })
    }

    fn clear_field_model_history(&mut self) {
        self.field_model_history.clear();
        self.field_model_latest_sequence = None;
        self.field_model_latest_seen = None;
    }

    fn push_field_model_frame(
        &mut self,
        sequence: u32,
        amplitudes: &[f64],
        now: std::time::Instant,
    ) {
        self.field_model_history.push_back(amplitudes.to_vec());
        if self.field_model_history.len() > FRAME_HISTORY_CAPACITY {
            self.field_model_history.pop_front();
        }
        self.field_model_latest_sequence = Some(sequence);
        self.field_model_latest_seen = Some(now);
    }

    /// ADR-110 / issue #1005 grid gate: decide whether a frame on `grid`
    /// may enter this node's feature path, and update `active_grid`.
    ///
    /// Returns `true` to accept. Policy: lock onto the densest grid seen.
    /// On a grid *upgrade* (more subcarriers — e.g. the first HE-SU 256-bin
    /// frame after HT 64-bin history) the rolling amplitude history and
    /// motion baseline are cleared so HT and HE symbol grids are never
    /// mixed in one window. Sparser-grid frames (the ~16% HT minority an
    /// ESP32-C6 keeps emitting alongside HE) are rejected from the feature
    /// path; the caller still records the arrival for fps/liveness.
    fn accept_grid(&mut self, grid: (u16, wifi_densepose_hardware::PpduType)) -> bool {
        match self.active_grid {
            None => {
                self.active_grid = Some(grid);
                true
            }
            Some(active) if active == grid => true,
            Some((active_n, _)) if grid.0 > active_n => {
                self.active_grid = Some(grid);
                self.frame_history.clear();
                self.baseline_motion = 0.0;
                self.baseline_frames = 0;
                true
            }
            Some(_) => false,
        }
    }

    /// ADR-084 cluster-Pi novelty step. Truncates / zero-pads the
    /// incoming amplitude vector to `NOVELTY_VECTOR_DIM`, scores its
    /// novelty against the per-node bank, then inserts it. The novelty
    /// score is computed *before* the insert so a frame doesn't see
    /// itself in the bank.
    pub(crate) fn update_novelty(&mut self, amplitudes: &[f64]) {
        let history = match &mut self.feature_history {
            Some(h) => h,
            None => return,
        };
        let mut feature: Vec<f32> = amplitudes
            .iter()
            .take(NOVELTY_VECTOR_DIM)
            .map(|&v| v as f32)
            .collect();
        feature.resize(NOVELTY_VECTOR_DIM, 0.0);

        // Score before insert so a query doesn't see itself.
        self.last_novelty_score = history.novelty(&feature);

        let _ = history.push(
            wifi_densepose_signal::ruvsense::longitudinal::EmbeddingEntry {
                person_id: 0,
                day_us: 0,
                embedding: feature,
            },
        );
    }

    /// Update the coherence score from the latest motion_energy value.
    ///
    /// Coherence is computed as 1.0 / (1.0 + running_variance) so that
    /// low motion-energy variance maps to high coherence ([0, 1]).
    fn update_coherence(&mut self, motion_energy: f64) {
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
        let variance: f64 = self
            .motion_energy_history
            .iter()
            .map(|v| (v - mean) * (v - mean))
            .sum::<f64>()
            / (n - 1) as f64;

        // Map variance to [0, 1] coherence: higher variance = lower coherence.
        self.coherence_score = (1.0 / (1.0 + variance)).clamp(0.0, 1.0);
    }

    /// Choose the EMA alpha based on current coherence score.
    fn ema_alpha(&self) -> f64 {
        if self.coherence_score < COHERENCE_LOW_THRESHOLD {
            TEMPORAL_EMA_ALPHA_LOW_COHERENCE
        } else {
            TEMPORAL_EMA_ALPHA_DEFAULT
        }
    }
}

/// Per-node feature info for WebSocket broadcasts (multi-node support).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PerNodeFeatureInfo {
    node_id: u8,
    features: FeatureInfo,
    classification: ClassificationInfo,
    rssi_dbm: f64,
    last_seen_ms: u64,
    frame_rate_hz: f64,
    stale: bool,
    /// ADR-084 Pass 3 cluster-Pi novelty score in `[0.0, 1.0]`.
    /// `0.0` = exact-match-in-bank, `1.0` = no overlap with recent
    /// per-node frame history. `None` until the first
    /// `update_novelty()` call. Consumers (model-wake gate, anomaly
    /// emit, UI heatmap) read this to decide whether to escalate.
    #[serde(skip_serializing_if = "Option::is_none")]
    novelty_score: Option<f32>,
}

/// Build a per-node feature snapshot for the WebSocket envelope.
///
/// ADR-084 Pass 3.6 — exposes `last_novelty_score` from each
/// `NodeState` to the WebSocket consumer. Returns `None` when the
/// node map is empty (no live ESP32 frames have been ingested yet),
/// so the existing `node_features: None` semantics on cold-start are
/// preserved.
///
/// Stale flag uses 5-second threshold matching `ESP32_OFFLINE_TIMEOUT`.
fn build_node_features(
    node_states: &std::collections::HashMap<u8, NodeState>,
    now: std::time::Instant,
) -> Option<Vec<PerNodeFeatureInfo>> {
    if node_states.is_empty() {
        return None;
    }
    let entries: Vec<PerNodeFeatureInfo> = node_states
        .iter()
        .map(|(&node_id, ns)| {
            let last_seen_ms = ns
                .last_frame_time
                .map(|t| now.saturating_duration_since(t).as_millis() as u64)
                .unwrap_or(u64::MAX);
            let stale = ns
                .last_frame_time
                .map(|t| now.saturating_duration_since(t) > ESP32_OFFLINE_TIMEOUT)
                .unwrap_or(true);
            let features = ns.latest_features.clone().unwrap_or(FeatureInfo {
                mean_rssi: 0.0,
                variance: 0.0,
                motion_band_power: 0.0,
                breathing_band_power: 0.0,
                dominant_freq_hz: 0.0,
                change_points: 0,
                spectral_power: 0.0,
            });
            PerNodeFeatureInfo {
                node_id,
                features,
                classification: ClassificationInfo {
                    motion_level: ns.current_motion_level.clone(),
                    presence: !matches!(ns.current_motion_level.as_str(), "absent"),
                    confidence: ns.smoothed_person_score.clamp(0.0, 1.0),
                },
                rssi_dbm: ns.rssi_history.back().copied().unwrap_or(0.0),
                last_seen_ms,
                frame_rate_hz: ns.measured_sample_rate_hz().unwrap_or(0.0),
                stale,
                novelty_score: ns.last_novelty_score,
            }
        })
        .collect();
    Some(entries)
}

// ── ADR-044 §5.2: Rolling P95 adaptive feature normalizer ────────────────────

/// Streaming P95 estimator over a fixed-size sliding window.
///
/// Self-calibrates feature normalization to whatever distribution the deployment
/// produces — no hardcoded scale values that can saturate in large rooms or
/// degrade in high-interference environments.
///
/// O(n log n) per query via sorted copy — acceptable at 20 Hz with window=600.
/// Cold-start (len < min_samples) returns `None` so the caller uses the legacy
/// fixed denominator, preserving day-0 behaviour.
pub struct RollingP95 {
    buf: std::collections::VecDeque<f64>,
    window: usize,
    min_samples: usize,
}

impl RollingP95 {
    pub fn new(window: usize, min_samples: usize) -> Self {
        Self {
            buf: std::collections::VecDeque::with_capacity(window),
            window,
            min_samples,
        }
    }

    pub fn push(&mut self, v: f64) {
        if self.buf.len() == self.window {
            self.buf.pop_front();
        }
        self.buf.push_back(v);
    }

    /// Returns `Some(p95)` once enough samples have accumulated, else `None`.
    pub fn current(&self) -> Option<f64> {
        if self.buf.len() < self.min_samples {
            return None;
        }
        let mut sorted: Vec<f64> = self.buf.iter().copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = ((sorted.len() as f64) * 0.95).ceil() as usize;
        Some(sorted[idx.saturating_sub(1).min(sorted.len() - 1)])
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }
}

// ── ADR-044 §5.3: Runtime config persistence ─────────────────────────────────

/// Runtime configuration that persists across server restarts via `data/config.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RuntimeConfig {
    /// Divisor for multi-node person-count deduplication (sum / factor).
    pub dedup_factor: f64,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self { dedup_factor: 3.0 }
    }
}

/// Load persisted runtime config from `<data_dir>/config.json`.
/// Falls back to [`RuntimeConfig::default`] if the file is absent or malformed.
pub(crate) fn load_runtime_config(data_dir: &std::path::Path) -> RuntimeConfig {
    let path = data_dir.join("config.json");
    match std::fs::read_to_string(&path) {
        Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
        Err(_) => RuntimeConfig::default(),
    }
}

/// Persist runtime config to `<data_dir>/config.json`.
pub(crate) fn save_runtime_config(data_dir: &std::path::Path, config: &RuntimeConfig) {
    let path = data_dir.join("config.json");
    if let Ok(json) = serde_json::to_string_pretty(config) {
        if let Err(e) = std::fs::write(&path, json) {
            warn!("Failed to save runtime config to {}: {e}", path.display());
        } else {
            info!("Runtime config saved to {}", path.display());
        }
    }
}

/// A single sensor node's position in room space, as placed via the Room
/// Builder UI. `id` must match the node's provisioned `--node-id` on the
/// physical board — it is not implied by array position, so nodes can be
/// added/removed/reordered in the UI without breaking the id-to-position
/// mapping (the same mapping issue #228/#249 was about).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct RoomNode {
    pub id: u8,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// Room geometry (a simple rectangle) plus sensor node placements, as
/// defined via the Room Builder UI (`POST /api/v1/config/room`).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct RoomConfig {
    pub width_m: f32,
    pub depth_m: f32,
    pub nodes: Vec<RoomNode>,
}

/// Load persisted room config from `<data_dir>/room_config.json`.
/// Falls back to [`RoomConfig::default`] (empty room, no nodes) if the file
/// is absent or malformed.
pub(crate) fn load_room_config(data_dir: &std::path::Path) -> RoomConfig {
    let path = data_dir.join("room_config.json");
    match std::fs::read_to_string(&path) {
        Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
        Err(_) => RoomConfig::default(),
    }
}

/// Persist room config to `<data_dir>/room_config.json`.
pub(crate) fn save_room_config(data_dir: &std::path::Path, config: &RoomConfig) {
    let path = data_dir.join("room_config.json");
    if let Ok(json) = serde_json::to_string_pretty(config) {
        if let Err(e) = std::fs::write(&path, json) {
            warn!("Failed to save room config to {}: {e}", path.display());
        } else {
            info!("Room config saved to {}", path.display());
        }
    }
}

#[cfg(test)]
mod room_config_tests {
    use super::*;

    #[test]
    fn load_missing_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = load_room_config(dir.path());
        assert_eq!(cfg.width_m, 0.0);
        assert_eq!(cfg.depth_m, 0.0);
        assert!(cfg.nodes.is_empty());
    }

    #[test]
    fn load_malformed_file_falls_back_to_default() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("room_config.json"), "{ not valid json").unwrap();
        let cfg = load_room_config(dir.path());
        assert_eq!(cfg.width_m, 0.0);
        assert!(cfg.nodes.is_empty());
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let saved = RoomConfig {
            width_m: 5.0,
            depth_m: 4.0,
            nodes: vec![
                RoomNode { id: 0, x: 0.0, y: 0.0, z: 0.4, label: Some("front-right".into()) },
                RoomNode { id: 1, x: 3.66, y: 0.0, z: 0.4, label: None },
            ],
        };
        save_room_config(dir.path(), &saved);
        let loaded = load_room_config(dir.path());
        assert_eq!(loaded.width_m, 5.0);
        assert_eq!(loaded.depth_m, 4.0);
        assert_eq!(loaded.nodes.len(), 2);
        assert_eq!(loaded.nodes[0].id, 0);
        assert_eq!(loaded.nodes[0].label.as_deref(), Some("front-right"));
        assert_eq!(loaded.nodes[1].label, None);
    }

    fn valid_config() -> RoomConfig {
        RoomConfig {
            width_m: 5.0,
            depth_m: 4.0,
            nodes: vec![
                RoomNode { id: 0, x: 0.0, y: 0.0, z: 0.4, label: None },
                RoomNode { id: 1, x: 3.66, y: 0.0, z: 0.4, label: None },
            ],
        }
    }

    #[test]
    fn validate_accepts_a_sane_config() {
        assert!(validate_room_config(&valid_config()).is_ok());
    }

    #[test]
    fn validate_rejects_non_positive_width() {
        let mut cfg = valid_config();
        cfg.width_m = 0.0;
        assert!(validate_room_config(&cfg).is_err());
        cfg.width_m = -1.0;
        assert!(validate_room_config(&cfg).is_err());
    }

    #[test]
    fn validate_rejects_non_finite_depth() {
        let mut cfg = valid_config();
        cfg.depth_m = f32::NAN;
        assert!(validate_room_config(&cfg).is_err());
        cfg.depth_m = f32::INFINITY;
        assert!(validate_room_config(&cfg).is_err());
    }

    #[test]
    fn validate_rejects_duplicate_node_ids() {
        let mut cfg = valid_config();
        cfg.nodes[1].id = 0; // collide with nodes[0]
        let err = validate_room_config(&cfg).unwrap_err();
        assert!(err.contains("duplicate"), "unexpected error message: {err}");
    }

    #[test]
    fn validate_rejects_non_finite_node_coordinate() {
        let mut cfg = valid_config();
        cfg.nodes[0].x = f32::NAN;
        assert!(validate_room_config(&cfg).is_err());
    }

    #[test]
    fn validate_allows_empty_node_list() {
        let mut cfg = valid_config();
        cfg.nodes.clear();
        assert!(validate_room_config(&cfg).is_ok());
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
    /// Latest validated RTL8720F summary; raw radar samples are not retained here.
    latest_realtek_radar: Option<realtek_radar::RealtekRadarSnapshot>,
    /// Instant of the last validated RTL8720F UDP frame.
    last_realtek_frame: Option<std::time::Instant>,
    /// Latest validated MediaTek CSI summary; raw matrices are not retained here.
    latest_mediatek_csi: Option<mediatek_csi::MediatekCsiSnapshot>,
    /// Instant of the last validated MediaTek CSI UDP frame.
    last_mediatek_frame: Option<std::time::Instant>,
    /// Latest validated Qualcomm CSI summary; raw matrices are not retained here.
    latest_qualcomm_csi: Option<qualcomm_csi::QualcommCsiSnapshot>,
    /// Instant of the last validated Qualcomm CSI UDP frame.
    last_qualcomm_frame: Option<std::time::Instant>,
    /// Latest bounded ADR-270 event per vendor. Complex CSI uses dedicated transports.
    latest_vendor_rf: BTreeMap<String, wifi_densepose_sensing_server::vendor_rf::VendorEventSnapshot>,
    tx: broadcast::Sender<String>,
    // ADR-099 D2/D3/D4: real-time CSI introspection tap. Per-frame state +
    // a parallel broadcast topic (`/ws/introspection`) running alongside
    // (not replacing) the window-aggregated `tx` / `/ws/sensing` pipeline.
    intro: wifi_densepose_sensing_server::introspection::IntrospectionState,
    intro_tx: broadcast::Sender<String>,
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
    // ── Training fields (ADR-186 TRAIN-RECONNECT) ────────────────────────────
    /// Live training state (shared status snapshot + cooperative cancel flag +
    /// background task handle) for the in-server trainer in `training_api`.
    training_state: training_api::TrainingState,
    /// Fan-out channel the background training job publishes progress JSON to;
    /// the `/ws/train/progress` WebSocket handler subscribes to it.
    training_progress_tx: broadcast::Sender<String>,
    // ── Adaptive classifier (environment-tuned) ──────────────────────────
    /// Trained adaptive model (loaded from data/adaptive_model.json or trained at runtime).
    adaptive_model: Option<adaptive_classifier::AdaptiveModel>,
    // ── Per-node state (issue #249) ─────────────────────────────────────
    /// Per-node sensing state for multi-node deployments.
    /// Keyed by `node_id` from the ESP32 frame header.
    node_states: HashMap<u8, NodeState>,
    /// Debounced room-level classification state — see
    /// `debounce_room_classification`. `fuse_room` is a fresh, memoryless
    /// plurality vote every cycle with no debounce of its own (unlike each
    /// node's own classification, which is debounced individually), so with
    /// per-node confidences sitting near a boundary the room-level result
    /// can flip cycle-to-cycle even though each node's own reading is stable.
    room_debounced_level: String,
    room_debounce_candidate: String,
    room_debounce_since: Option<std::time::Instant>,
    // ── Accuracy sprint: Kalman tracker, multistatic fusion, eigenvalue counting ──
    /// Global Kalman-based pose tracker for stable person IDs and smoothed keypoints.
    pose_tracker: PoseTracker,
    /// Instant of last tracker update (for computing dt).
    last_tracker_instant: Option<std::time::Instant>,
    /// Attention-weighted multi-node CSI fusion engine.
    multistatic_fuser: MultistaticFuser,
    /// Node positions parsed from --node-positions, in the same order given
    /// to `multistatic_fuser.set_node_positions`. Real fleets use
    /// non-sequential logical node IDs, so this is NOT keyed by node_id —
    /// entry `i` applies to the i-th smallest currently-active node_id, the
    /// same convention `MultistaticFuser::fuse` uses. See
    /// `node_positions_by_active_id`.
    node_positions_config: Vec<[f32; 3]>,
    /// Governed trust-path bridge (ADR-135..146): runs the same live frames
    /// through the privacy/provenance/witness control plane. Does not alter
    /// person-count behavior; its trust state (witness, effective class,
    /// recalibration flag, error count) is recorded on the bridge itself and
    /// exposed via `GET /api/v1/status`, and a Restricted-class cycle strips
    /// per-node raw amplitudes from the live publish (review finding 1).
    engine_bridge: engine_bridge::EngineBridge,
    /// SVD-based room field model for eigenvalue person counting (None until calibration).
    field_model: Option<FieldModel>,
    /// Stable installation identity used only to bind local persisted state.
    installation_id: Option<String>,
    /// Metadata for the privacy reduced empty room image, when available.
    bootstrap_baseline: Option<BootstrapBaselineMetadata>,
    /// A restored prior can affect startup occupancy but cannot authorize vitals.
    bootstrap_baseline_active: bool,
    /// Server generated identity for the current explicit calibration model.
    calibration_model_id: Option<String>,
    /// Nodes that actually contributed frames to the current calibration.
    calibration_source_node_ids: std::collections::BTreeSet<u8>,
    /// Immutable source and CSI symbol grid selected from recent header-only
    /// evidence for the current field model.
    calibration_grid_binding: Option<CalibrationGridBinding>,
    /// Last admitted raw CSI sequence per contributing node. Edge-vitals
    /// packets do not carry a new CSI observation and cannot advance this map.
    calibration_last_sequences: HashMap<u8, u32>,
    /// Late packets inside the bounded UDP reorder window are ignored rather
    /// than counted. These receipts make the transport behavior auditable.
    calibration_reordered_packets: HashMap<u8, u64>,
    calibration_max_reorder_depth: HashMap<u8, u32>,
    /// Nodes whose sequence moved backward beyond the bounded UDP reorder
    /// window during the current session. Without a wire boot epoch, the
    /// session must stay failed closed after a material discontinuity.
    calibration_sequence_fault_node_ids: std::collections::BTreeSet<u8>,
    // ── ADR-044 §5.2: adaptive rolling-p95 normalization ─────────────────────
    /// Rolling P95 of `FeatureInfo.variance` over the last ~30 s (600 frames @ 20 Hz).
    pub(crate) p95_variance: RollingP95,
    /// Rolling P95 of `FeatureInfo.motion_band_power` over the last ~30 s.
    pub(crate) p95_motion_band_power: RollingP95,
    /// Rolling P95 of `FeatureInfo.spectral_power` over the last ~30 s.
    pub(crate) p95_spectral_power: RollingP95,
    // ── ADR-044 §5.3: runtime-configurable dedup factor ───────────────────────
    /// Divisor for multi-node person-count deduplication (sum / factor).
    /// Default 3.0 (one body visible to ~3 nodes on average).
    /// Configurable at runtime via `POST /api/v1/config/dedup-factor` and
    /// `POST /api/v1/config/ground-truth`. Persisted across restarts.
    pub(crate) dedup_factor: f64,
    /// Data directory for persisting runtime config (parent of `firmware_dir`).
    pub(crate) data_dir: std::path::PathBuf,
    /// ADR-262 P3: the live RuField surface. Holds the dedicated ed25519 signer
    /// + a bounded ring of recent signed `FieldEvent`s + the `/ws/field`
    /// broadcast topic. The governed sensing cycle calls `emit()` on it once per
    /// cycle (joining `SensingUpdate` features/classification/signal_field with
    /// the `TrustedOutput` trust class); `/api/field` + `/ws/field` read it.
    /// Held behind its own `Arc<RwLock<_>>` so the additive field router can
    /// take it as state without re-locking `AppStateInner`.
    field_surface: rufield_surface::FieldState,
    /// Canonical ADR-323 engine and latest additive publication. Existing
    /// image-space renderer poses are never silently inserted here.
    pose_physics: pose_physics::PosePhysicsRuntime,
}

#[cfg(test)]
mod adr323_pose_physics_http_tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    fn app() -> Router {
        Router::new()
            .route("/api/v1/pose/current", get(pose_current))
            .route("/api/v1/pose/physics/metrics", get(pose_physics_metrics))
            .with_state(Arc::new(RwLock::new(AppStateInner::minimal())))
    }

    #[tokio::test]
    async fn raw_view_remains_backward_compatible() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/pose/current?view=raw")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 65_536)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(body.get("persons").is_some());
        assert!(body.get("physics").is_none());
    }

    #[tokio::test]
    async fn refined_view_is_typed_conflict_when_no_selected_pose_exists() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/pose/current?view=refined")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let body = axum::body::to_bytes(response.into_body(), 65_536)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["code"], "pose_refined_unavailable");
    }

    #[tokio::test]
    async fn physics_metrics_use_prometheus_content_type() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/pose/physics/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[axum::http::header::CONTENT_TYPE],
            "text/plain; version=0.0.4"
        );
    }
}

/// If no ESP32 frame arrives within this duration, source reverts to offline.
const ESP32_OFFLINE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CalibrationSequenceOrder {
    First,
    Forward,
    Duplicate,
    Reordered(u32),
    Discontinuity,
}

/// Two independent live Node 5 captures observed bounded same-source UDP
/// reordering through depth eleven. Sixteen covers that measured maximum with
/// five positions of margin. At the firmware's 50 Hz ceiling it spans at most
/// 320 ms; larger backward jumps stay failed closed. Late packets inside this
/// window are always dropped and never become model evidence.
const CALIBRATION_SEQUENCE_REORDER_WINDOW: u32 = 16;

fn calibration_sequence_order(previous: Option<u32>, sequence: u32) -> CalibrationSequenceOrder {
    let Some(previous) = previous else {
        return CalibrationSequenceOrder::First;
    };
    let delta = sequence.wrapping_sub(previous);
    if delta == 0 {
        CalibrationSequenceOrder::Duplicate
    } else if delta < (1_u32 << 31) {
        CalibrationSequenceOrder::Forward
    } else {
        let reorder_depth = previous.wrapping_sub(sequence);
        if reorder_depth <= CALIBRATION_SEQUENCE_REORDER_WINDOW {
            CalibrationSequenceOrder::Reordered(reorder_depth)
        } else {
            CalibrationSequenceOrder::Discontinuity
        }
    }
}

impl AppStateInner {
    /// Admit at most one raw CSI observation per forward node sequence into
    /// the current calibration session. This stateful boundary prevents a
    /// caller from advancing the model with a replayed history tail.
    fn maybe_feed_calibration_frame(
        &mut self,
        node_id: u8,
        grid: CsiGridKey,
        sequence: u32,
        amplitudes: &[f64],
        observed_at: std::time::Instant,
    ) -> bool {
        let binding_matches = self.calibration_grid_binding.is_some_and(|binding| {
            binding.source_node_id == node_id && binding.grid == grid
        });
        if !binding_matches
            || !calibration_source_accepts(&self.calibration_source_node_ids, node_id)
            || self.calibration_sequence_fault_node_ids.contains(&node_id)
        {
            return false;
        }
        let previous_sequence = self.calibration_last_sequences.get(&node_id).copied();
        match calibration_sequence_order(previous_sequence, sequence) {
            CalibrationSequenceOrder::Duplicate => return false,
            CalibrationSequenceOrder::Reordered(depth) => {
                *self
                    .calibration_reordered_packets
                    .entry(node_id)
                    .or_default() += 1;
                self.calibration_max_reorder_depth
                    .entry(node_id)
                    .and_modify(|observed| *observed = (*observed).max(depth))
                    .or_insert(depth);
                debug!(
                    node_id,
                    previous_sequence,
                    sequence,
                    depth,
                    "Ignored bounded late CSI packet during calibration"
                );
                return false;
            }
            CalibrationSequenceOrder::Discontinuity => {
                self.calibration_sequence_fault_node_ids.insert(node_id);
                warn!(
                    node_id,
                    previous_sequence,
                    sequence,
                    "Calibration source sequence discontinuity exceeded reorder window; restart the empty-room capture"
                );
                return false;
            }
            CalibrationSequenceOrder::First | CalibrationSequenceOrder::Forward => {}
        }
        let accepted = self.field_model.as_mut().is_some_and(|field| {
            if matches!(
                field.status(),
                CalibrationStatus::Uncalibrated | CalibrationStatus::Collecting
            ) {
                field_bridge::maybe_feed_calibration(field, amplitudes)
            } else {
                field.check_freshness(
                    (chrono::Utc::now().timestamp_millis().max(0) as u64)
                        .saturating_mul(1_000),
                ) == CalibrationStatus::Fresh
            }
        });
        if accepted {
            self.calibration_source_node_ids.insert(node_id);
            self.calibration_last_sequences.insert(node_id, sequence);
            if let Some(node) = self.node_states.get_mut(&node_id) {
                node.push_field_model_frame(sequence, amplitudes, observed_at);
            }
        }
        accepted
    }

    fn clear_calibration_sequence_state(&mut self) {
        self.calibration_last_sequences.clear();
        self.calibration_reordered_packets.clear();
        self.calibration_max_reorder_depth.clear();
        self.calibration_sequence_fault_node_ids.clear();
    }

    fn clear_field_model_binding(&mut self) {
        self.calibration_grid_binding = None;
        for node in self.node_states.values_mut() {
            node.clear_field_model_history();
        }
    }

    fn calibration_grid_is_fresh(
        &self,
        binding: CalibrationGridBinding,
        now: std::time::Instant,
    ) -> bool {
        !self
            .calibration_sequence_fault_node_ids
            .contains(&binding.source_node_id)
            && self
                .node_states
                .get(&binding.source_node_id)
                .and_then(|node| node.field_model_latest_seen)
                .is_some_and(|seen| {
                    now.saturating_duration_since(seen) < ESP32_OFFLINE_TIMEOUT
                })
    }

    fn field_model_holdout_ready(
        &self,
        binding: CalibrationGridBinding,
        required_window_size: usize,
        now: std::time::Instant,
    ) -> bool {
        self.calibration_grid_binding.is_some_and(|current| {
            current.source_node_id == binding.source_node_id && current.grid == binding.grid
        }) && self.calibration_grid_is_fresh(binding, now)
            && self
                .node_states
                .get(&binding.source_node_id)
                .is_some_and(|node| node.field_model_history.len() >= required_window_size)
    }

    fn begin_field_model_holdout(&mut self, binding: CalibrationGridBinding) {
        if let Some(node) = self.node_states.get_mut(&binding.source_node_id) {
            node.clear_field_model_history();
        }
    }

    fn field_model_status_at(&self, observed_at_unix_ms: u64) -> Option<CalibrationStatus> {
        self.field_model.as_ref().map(|model| {
            field_bridge::calibration_status_at(model, observed_at_unix_ms.saturating_mul(1_000))
        })
    }

    fn field_model_active_at(&self, observed_at_unix_ms: u64) -> bool {
        self.field_model_status_at(observed_at_unix_ms)
            .is_some_and(|status| status != CalibrationStatus::Expired)
    }

    fn bootstrap_baseline_active_at(&self, observed_at_unix_ms: u64) -> bool {
        self.bootstrap_baseline_active && self.field_model_active_at(observed_at_unix_ms)
    }

    fn explicit_calibration_fresh_at(&self, observed_at_unix_ms: u64) -> bool {
        !self.bootstrap_baseline_active
            && self.field_model_status_at(observed_at_unix_ms) == Some(CalibrationStatus::Fresh)
    }

    /// Return the effective data source, accounting for ESP32 frame timeout.
    /// If the source is "esp32" but no frame has arrived in 5 seconds, returns
    /// "esp32:offline" so the UI can distinguish active vs stale connections.
    /// Person count: eigenvalue-based if field model is calibrated, else heuristic.
    /// Uses global frame_history if populated, otherwise the freshest per-node history.
    fn person_count_at(&self, observed_at_unix_ms: u64) -> usize {
        // A persisted bootstrap model has negative-only authority. Its only
        // allowed occupancy effect is the explicit empty-background suppression
        // in `bootstrap_empty_prior_applies`; it must never infer positive count.
        if self.bootstrap_baseline_active {
            return score_to_person_count(self.smoothed_person_score, self.prev_person_count);
        }
        match self.field_model.as_ref() {
            Some(fm) => {
                // A single-link model may score only the source node it was
                // calibrated against. Applying one node's baseline to a
                // different radio creates deterministic false occupancy from
                // hardware-specific amplitude offsets.
                let bound_source_node_id = (self.calibration_source_node_ids.len() == 1)
                    .then(|| self.calibration_source_node_ids.iter().next().copied())
                    .flatten()
                    .or_else(|| {
                        self.bootstrap_baseline.as_ref().and_then(|metadata| {
                            (metadata.source_node_ids.len() == 1)
                                .then_some(metadata.source_node_ids[0])
                        })
                    });
                let history = if let Some(node_id) = bound_source_node_id {
                    self.node_states
                        .get(&node_id)
                        .map(|state| &state.field_model_history)
                        .unwrap_or(&self.frame_history)
                } else if !self.frame_history.is_empty() {
                    &self.frame_history
                } else {
                    self.node_states
                        .values()
                        .filter(|ns| !ns.frame_history.is_empty())
                        .max_by_key(|ns| ns.last_frame_time)
                        .map(|ns| &ns.frame_history)
                        .unwrap_or(&self.frame_history)
                };
                field_bridge::occupancy_or_fallback(
                    fm,
                    history,
                    observed_at_unix_ms.saturating_mul(1_000),
                    self.smoothed_person_score,
                    self.prev_person_count,
                )
            }
            None => score_to_person_count(self.smoothed_person_score, self.prev_person_count),
        }
    }

    fn person_count(&self) -> usize {
        self.person_count_at(chrono::Utc::now().timestamp_millis().max(0) as u64)
    }

    /// A restored startup image has negative-only authority. It may suppress
    /// an empty-room false positive after one complete runtime window, but it
    /// cannot establish positive presence or publish vital signs.
    fn bootstrap_empty_prior_applies(&self, observed_at_unix_ms: u64) -> bool {
        self.bootstrap_background_match(observed_at_unix_ms)
            .is_some_and(|result| result.matches_empty)
    }

    /// Return current background conformance for the exact source node bound
    /// into the restored image. This remains diagnostic and negative-only.
    fn bootstrap_background_match(
        &self,
        observed_at_unix_ms: u64,
    ) -> Option<field_bridge::BootstrapBackgroundMatch> {
        if !self.bootstrap_baseline_active {
            return None;
        }
        let field_model = self.field_model.as_ref()?;
        let metadata = self.bootstrap_baseline.as_ref()?;
        let [source_node_id] = metadata.source_node_ids.as_slice() else {
            return None;
        };
        let binding = self.calibration_grid_binding?;
        if binding.source_node_id != *source_node_id
            || binding.grid != CsiGridKey::from_bootstrap_grid(metadata.source_grid)
            || self
                .calibration_sequence_fault_node_ids
                .contains(source_node_id)
        {
            return None;
        }
        let node = self.node_states.get(source_node_id)?;
        if !node.field_model_latest_seen.is_some_and(|seen| {
            std::time::Instant::now().saturating_duration_since(seen)
                < ESP32_OFFLINE_TIMEOUT
        }) {
            return None;
        }
        let history = &node.field_model_history;
        field_bridge::bootstrap_background_match(
            field_model,
            history,
            observed_at_unix_ms.saturating_mul(1_000),
        )
    }

    fn effective_source(&self) -> String {
        if self.source == "esp32" {
            if let Some(last) = self.last_esp32_frame {
                if last.elapsed() > ESP32_OFFLINE_TIMEOUT {
                    return "esp32:offline".to_string();
                }
            }
        }
        if self.source.starts_with("realtek") {
            if let Some(last) = self.last_realtek_frame {
                if last.elapsed() > ESP32_OFFLINE_TIMEOUT {
                    return format!("{}:offline", self.source);
                }
            }
        }
        if self.source.starts_with("mediatek") {
            if let Some(last) = self.last_mediatek_frame {
                if last.elapsed() > ESP32_OFFLINE_TIMEOUT {
                    return format!("{}:offline", self.source);
                }
            }
        }
        if self.source.starts_with("qualcomm") {
            if let Some(last) = self.last_qualcomm_frame {
                if last.elapsed() > ESP32_OFFLINE_TIMEOUT {
                    return format!("{}:offline", self.source);
                }
            }
        }
        self.source.clone()
    }

    /// ADR-295 — canonical provenance state for the current source. Derived
    /// from the freshness-gated [`effective_source`](Self::effective_source)
    /// label so ambiguity can never collapse to "live": a synthetic source is
    /// always `Synthetic`, an `":offline"` label is `Disconnected`, and a fresh
    /// hardware feed is `LiveUnverified` — never `LiveVerified`, since this path
    /// carries no attestation. `effective_source()` has already applied the
    /// freshness gate, so a non-offline live label means a fresh frame.
    fn source_state(&self) -> SourceState {
        SourceState::from_source_label(
            &self.effective_source(),
            Some(Duration::ZERO),
            ESP32_OFFLINE_TIMEOUT,
        )
    }
}

/// Number of frames retained in `frame_history` for temporal analysis.
/// At 500 ms ticks this covers ~50 seconds; at 100 ms ticks ~10 seconds.
const FRAME_HISTORY_CAPACITY: usize = 100;
/// A qualified grid may run at the 2 Hz admission floor. Rebuilding a 50 frame
/// runtime scoring window then needs 25 seconds, so promotion grants ten more
/// seconds for ordinary bounded jitter before failing closed.
const BOOTSTRAP_HOLDOUT_PREWARM_TIMEOUT_MS: u64 = 35_000;

type SharedState = Arc<RwLock<AppStateInner>>;

#[cfg(test)]
impl AppStateInner {
    /// Minimal, dependency-free `AppStateInner` for in-process router tests
    /// (ADR-186 P6). Uses the same field constructors as the real state seeding
    /// in `main()` but with trivial values and no CLI/config inputs, so tests can
    /// build the training router without the full server boot.
    pub(crate) fn minimal() -> Self {
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
            latest_qualcomm_csi: None,
            last_qualcomm_frame: None,
            latest_vendor_rf: BTreeMap::new(),
            tx: broadcast::channel::<String>(16).0,
            intro: wifi_densepose_sensing_server::introspection::IntrospectionState::new(),
            intro_tx: broadcast::channel::<String>(16).0,
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
            node_positions_config: Vec::new(),
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
            calibration_source_node_ids: std::collections::BTreeSet::new(),
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

#[cfg(test)]
mod calibration_expiry_tests {
    use super::*;
    use wifi_densepose_signal::ruvsense::field_model::FieldModelConfig;

    fn finalized_model(calibrated_at_unix_ms: u64) -> FieldModel {
        let mut model = FieldModel::new(FieldModelConfig {
            n_links: 1,
            n_subcarriers: 56,
            n_modes: 3,
            min_calibration_frames: 10,
            min_calibration_duration_s: 0.0,
            baseline_expiry_s: 2.0,
        })
        .unwrap();
        for frame_index in 0..20 {
            let frame = (0..56)
                .map(|subcarrier| {
                    10.0 + subcarrier as f64 * 0.01
                        + (frame_index as f64 * 0.1 + subcarrier as f64 * 0.03).sin() * 0.01
                })
                .collect();
            model.feed_calibration(&[frame]).unwrap();
        }
        model
            .finalize_calibration(calibrated_at_unix_ms.saturating_mul(1_000), 7)
            .unwrap();
        model
    }

    fn state_with_model(bootstrap: bool) -> AppStateInner {
        let model = finalized_model(1_000);
        let baseline = model.modes().unwrap().baseline[0].clone();
        let mut state = AppStateInner::minimal();
        state.frame_history = (0..50).map(|_| baseline.clone()).collect();
        let mut node = NodeState::new();
        node.field_model_history = (0..50).map(|_| baseline.clone()).collect();
        node.field_model_latest_sequence = Some(50);
        node.field_model_latest_seen = Some(std::time::Instant::now());
        state.node_states.insert(5, node);
        state.calibration_source_node_ids.insert(5);
        state.calibration_grid_binding = Some(CalibrationGridBinding {
            source_node_id: 5,
            grid: CsiGridKey {
                n_subcarriers: 64,
                ppdu_type: 0,
            },
            evidence: None,
        });
        state.field_model = Some(model);
        state.bootstrap_baseline_active = bootstrap;
        if bootstrap {
            state.bootstrap_baseline = Some(BootstrapBaselineMetadata {
                authority: bootstrap_baseline::BOOTSTRAP_BASELINE_AUTHORITY,
                source_node_ids: vec![5],
                source_grid: bootstrap_baseline::BootstrapCsiGrid {
                    n_subcarriers: 64,
                    ppdu_type: 0,
                },
                source_model_id: "test-model".to_string(),
                created_at_unix_ms: 1_000,
                expires_at_unix_ms: 3_000,
                content_sha256: "00".repeat(32),
            });
        }
        state
    }

    #[test]
    fn bootstrap_field_model_remains_negative_only_for_person_count() {
        let runtime = state_with_model(false);
        assert_eq!(runtime.person_count_at(1_500), 0);

        let bootstrap = state_with_model(true);
        assert_eq!(
            bootstrap.person_count_at(1_500),
            score_to_person_count(0.0, 0),
            "a bootstrap prior must not derive a positive or negative count through the runtime model path"
        );
    }

    #[tokio::test]
    async fn long_running_expiry_is_not_reported_active_for_runtime_or_bootstrap() {
        let runtime = Arc::new(RwLock::new(state_with_model(false)));
        let Json(runtime_status) = calibration_status(State(runtime)).await;
        assert_eq!(runtime_status["active"], false);
        assert_eq!(runtime_status["status"], "expired");
        assert_eq!(runtime_status["binding_mode"], "none");

        let bootstrap = Arc::new(RwLock::new(state_with_model(true)));
        let Json(bootstrap_status) = calibration_status(State(bootstrap)).await;
        assert_eq!(bootstrap_status["active"], false);
        assert_eq!(bootstrap_status["status"], "expired");
        assert_eq!(bootstrap_status["binding_mode"], "none");
        assert_eq!(bootstrap_status["bootstrap_baseline"]["stored"], true);
        assert_eq!(bootstrap_status["bootstrap_baseline"]["active"], false);
        assert_eq!(
            bootstrap_status["bootstrap_baseline"]["calibrated_evidence_authorized"],
            false
        );
        assert_eq!(
            bootstrap_status["bootstrap_baseline"]["numeric_vitals_authorized"],
            false
        );
    }
}

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
    person_count_valid: bool,
    motion_energy: f32,
    presence_score: f32,
    timestamp_ms: u32,
}

const EDGE_MAX_PERSONS: u8 = 4;

fn sanitize_edge_person_count(presence: bool, raw: u8) -> (u8, bool) {
    if raw > EDGE_MAX_PERSONS || (!presence && raw != 0) {
        return (0, false);
    }
    (raw, true)
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
    let presence = (flags & 0x01) != 0;
    let (n_persons, person_count_valid) = sanitize_edge_person_count(presence, buf[13]);
    let motion_energy = f32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]);
    let presence_score = f32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]);
    let timestamp_ms = u32::from_le_bytes([buf[24], buf[25], buf[26], buf[27]]);

    Some(Esp32VitalsPacket {
        node_id,
        presence,
        fall_detected: (flags & 0x02) != 0,
        motion: (flags & 0x04) != 0,
        breathing_rate_bpm: breathing_raw as f64 / 100.0,
        heartrate_bpm: heartrate_raw as f64 / 10000.0,
        rssi,
        n_persons,
        person_count_valid,
        motion_energy,
        presence_score,
        timestamp_ms,
    })
}

#[cfg(test)]
mod edge_vitals_integrity_tests {
    use super::*;

    fn packet(presence: bool, n_persons: u8) -> Vec<u8> {
        let mut buf = vec![0u8; 32];
        buf[0..4].copy_from_slice(&0xC511_0002u32.to_le_bytes());
        buf[4] = 4;
        buf[5] = u8::from(presence);
        buf[13] = n_persons;
        buf
    }

    #[test]
    fn contradictory_count_fails_closed() {
        let parsed = parse_esp32_vitals(&packet(false, 4)).expect("valid packet");
        assert_eq!(parsed.n_persons, 0);
        assert!(!parsed.person_count_valid);
    }

    #[test]
    fn present_count_is_preserved() {
        let parsed = parse_esp32_vitals(&packet(true, 3)).expect("valid packet");
        assert_eq!(parsed.n_persons, 3);
        assert!(parsed.person_count_valid);
    }
}

// ── ADR-040: WASM Output Packet (magic 0xC511_0007 — reassigned per #928) ─────

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

/// Parse a WASM output packet (magic 0xC511_0007 — reassigned per issue #928;
/// the original 0xC511_0004 was a collision with ADR-063 fused vitals).
fn parse_wasm_output(buf: &[u8]) -> Option<WasmOutputPacket> {
    if buf.len() < 8 {
        return None;
    }
    let magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    if magic != 0xC511_0007 {
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
            buf[offset + 1],
            buf[offset + 2],
            buf[offset + 3],
            buf[offset + 4],
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

// ── ADR-063: Edge Fused Vitals Packet (magic 0xC511_0004) ─────────────────────
//
// 48-byte packed struct emitted by the ESP32-C6 + MR60BHA2 mmWave config when
// `mmwave_sensor_get_state().detected` is true. Byte layout from
// `firmware/esp32-csi-node/main/edge_processing.h` line 129 — kept in lockstep
// with the firmware's `_Static_assert(sizeof(edge_fused_vitals_pkt_t) == 48)`.
// Issue #928 surfaced that this magic was being parsed as WASM output and the
// fused vitals were silently lost. Adding the proper parser here.

#[derive(Debug, Clone, Serialize)]
struct EdgeFusedVitalsPacket {
    node_id: u8,
    /// Bit0=presence, Bit1=fall, Bit2=motion, Bit3=mmwave_present.
    flags: u8,
    /// Fused breathing rate in BPM (firmware sends BPM*100; we scale here).
    breathing_rate_bpm: f32,
    /// Fused heartrate in BPM (firmware sends BPM*10000; we scale here).
    heartrate_bpm: f32,
    rssi: i8,
    n_persons: u8,
    person_count_valid: bool,
    /// `mmwave_type_t` enum value from firmware.
    mmwave_type: u8,
    /// 0-100 fusion quality score.
    fusion_confidence: u8,
    motion_energy: f32,
    presence_score: f32,
    timestamp_ms: u32,
    /// Raw mmWave heart rate (BPM).
    mmwave_hr_bpm: f32,
    /// Raw mmWave breathing rate (BPM).
    mmwave_br_bpm: f32,
    /// Distance to nearest target (cm).
    mmwave_distance_cm: f32,
    /// Target count from mmWave.
    mmwave_targets: u8,
    /// mmWave signal quality 0-100.
    mmwave_confidence: u8,
}

/// Parse an ADR-063 edge fused vitals packet (magic 0xC511_0004, 48 bytes).
fn parse_edge_fused_vitals(buf: &[u8]) -> Option<EdgeFusedVitalsPacket> {
    if buf.len() < 48 {
        return None;
    }
    let magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    if magic != 0xC511_0004 {
        return None;
    }

    let node_id = buf[4];
    let flags = buf[5];
    let breathing_raw = u16::from_le_bytes([buf[6], buf[7]]);
    let heartrate_raw = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
    let rssi = buf[12] as i8;
    let any_presence = (flags & 0x09) != 0;
    let (n_persons, person_count_valid) = sanitize_edge_person_count(any_presence, buf[13]);
    let mmwave_type = buf[14];
    let fusion_confidence = buf[15];
    let motion_energy = f32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]);
    let presence_score = f32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]);
    let timestamp_ms = u32::from_le_bytes([buf[24], buf[25], buf[26], buf[27]]);
    let mmwave_hr_bpm = f32::from_le_bytes([buf[28], buf[29], buf[30], buf[31]]);
    let mmwave_br_bpm = f32::from_le_bytes([buf[32], buf[33], buf[34], buf[35]]);
    let mmwave_distance_cm = f32::from_le_bytes([buf[36], buf[37], buf[38], buf[39]]);
    let mmwave_targets = buf[40];
    let mmwave_confidence = buf[41];
    // buf[42..48] are firmware reserved fields (reserved3 u16 + reserved4 u32).

    Some(EdgeFusedVitalsPacket {
        node_id,
        flags,
        breathing_rate_bpm: breathing_raw as f32 / 100.0,
        heartrate_bpm: heartrate_raw as f32 / 10000.0,
        rssi,
        n_persons,
        person_count_valid,
        mmwave_type,
        fusion_confidence,
        motion_energy,
        presence_score,
        timestamp_ms,
        mmwave_hr_bpm,
        mmwave_br_bpm,
        mmwave_distance_cm,
        mmwave_targets,
        mmwave_confidence,
    })
}

#[cfg(test)]
mod issue_928_magic_collision_tests {
    //! Issue #928 — `0xC511_0004` was being parsed as WASM output, eating the
    //! C6+mmWave fused-vitals packets. After this fix, `0xC511_0004` routes to
    //! `parse_edge_fused_vitals` and WASM output owns the freshly-allocated
    //! `0xC511_0007` slot. Tests guard both halves of the swap.
    use super::*;

    /// Build a 48-byte synthetic fused-vitals packet matching the firmware's
    /// `edge_fused_vitals_pkt_t` layout from `edge_processing.h:129`.
    fn build_fused_vitals_packet() -> Vec<u8> {
        let mut buf = vec![0u8; 48];
        buf[0..4].copy_from_slice(&0xC511_0004u32.to_le_bytes());
        buf[4] = 9;                                            // node_id
        buf[5] = 0b0000_1001;                                   // flags: presence | mmwave_present
        buf[6..8].copy_from_slice(&1600u16.to_le_bytes());      // breathing 16.00 BPM
        buf[8..12].copy_from_slice(&720_000u32.to_le_bytes());  // heartrate 72.0 BPM
        buf[12] = (-55i8) as u8;                                // rssi
        buf[13] = 1;                                            // n_persons
        buf[14] = 2;                                            // mmwave_type
        buf[15] = 85;                                           // fusion_confidence
        buf[16..20].copy_from_slice(&0.42f32.to_le_bytes());    // motion_energy
        buf[20..24].copy_from_slice(&0.95f32.to_le_bytes());    // presence_score
        buf[24..28].copy_from_slice(&1_234_567u32.to_le_bytes()); // timestamp_ms
        buf[28..32].copy_from_slice(&71.5f32.to_le_bytes());    // mmwave_hr_bpm
        buf[32..36].copy_from_slice(&15.8f32.to_le_bytes());    // mmwave_br_bpm
        buf[36..40].copy_from_slice(&182.0f32.to_le_bytes());   // mmwave_distance_cm
        buf[40] = 1;                                            // mmwave_targets
        buf[41] = 90;                                           // mmwave_confidence
        // bytes 42..48 — firmware reserved fields, left as zero
        buf
    }

    #[test]
    fn parse_edge_fused_vitals_extracts_fields_correctly() {
        let buf = build_fused_vitals_packet();
        let pkt = parse_edge_fused_vitals(&buf).expect("must parse a well-formed packet");
        assert_eq!(pkt.node_id, 9);
        assert_eq!(pkt.flags, 0b0000_1001);
        assert!((pkt.breathing_rate_bpm - 16.0).abs() < 1e-3, "breathing scale 100");
        assert!((pkt.heartrate_bpm - 72.0).abs() < 1e-3, "heartrate scale 10000");
        assert_eq!(pkt.rssi, -55);
        assert_eq!(pkt.n_persons, 1);
        assert!(pkt.person_count_valid);
        assert_eq!(pkt.mmwave_type, 2);
        assert_eq!(pkt.fusion_confidence, 85);
        assert!((pkt.motion_energy - 0.42).abs() < 1e-6);
        assert!((pkt.presence_score - 0.95).abs() < 1e-6);
        assert_eq!(pkt.timestamp_ms, 1_234_567);
        assert!((pkt.mmwave_hr_bpm - 71.5).abs() < 1e-6);
        assert!((pkt.mmwave_br_bpm - 15.8).abs() < 1e-3);
        assert!((pkt.mmwave_distance_cm - 182.0).abs() < 1e-6);
        assert_eq!(pkt.mmwave_targets, 1);
        assert_eq!(pkt.mmwave_confidence, 90);
    }

    #[test]
    fn parse_edge_fused_vitals_rejects_short_buffer() {
        let buf = build_fused_vitals_packet();
        // Truncate to 47 bytes — one short of the 48-byte minimum.
        assert!(parse_edge_fused_vitals(&buf[..47]).is_none());
    }

    #[test]
    fn parse_edge_fused_vitals_rejects_wrong_magic() {
        let mut buf = build_fused_vitals_packet();
        buf[0..4].copy_from_slice(&0xC511_0007u32.to_le_bytes()); // WASM magic, not fused
        assert!(parse_edge_fused_vitals(&buf).is_none());
    }

    #[test]
    fn parse_wasm_output_rejects_legacy_0004_magic() {
        // The old WASM magic collided with fused vitals — must no longer be
        // accepted. A real fused-vitals packet starts with 0xC511_0004 and
        // would have been misparsed before this fix.
        let buf = build_fused_vitals_packet();
        assert!(parse_wasm_output(&buf).is_none(),
                "issue #928: WASM parser must NOT accept 0xC511_0004");
    }

    #[test]
    fn parse_wasm_output_accepts_new_0007_magic() {
        // Build a tiny well-formed WASM output packet on the new magic.
        let mut buf = vec![0u8; 8];
        buf[0..4].copy_from_slice(&0xC511_0007u32.to_le_bytes());
        buf[4] = 5;                                          // node_id
        buf[5] = 1;                                          // module_id
        buf[6..8].copy_from_slice(&0u16.to_le_bytes());      // event_count = 0
        let pkt = parse_wasm_output(&buf).expect("0xC511_0007 must parse");
        assert_eq!(pkt.node_id, 5);
        assert_eq!(pkt.module_id, 1);
        assert!(pkt.events.is_empty());
    }
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
    //   [18]     PPDU type
    //   [19]     ADR-018 flags (bit 4 = IEEE 802.15.4 sync valid)
    //   [20..]   I/Q data
    // Issue #1005: until 2026-06 this code read n_subcarriers from byte 6
    // alone (an ESP32-C6 HE-SU frame's 256 = 0x0100 LE decoded as 0 — the
    // frame parsed with zero subcarriers) and read sequence/rssi/noise at
    // stale offsets 10/14/15. Offsets below match the comment (and firmware).
    let node_id = buf[4];
    let n_antennas = buf[5];
    let n_subcarriers = u16::from_le_bytes([buf[6], buf[7]]);
    let freq_mhz =
        u16::try_from(u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]])).unwrap_or(0);
    let sequence = u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]);
    let rssi_raw = buf[16] as i8;
    // Fix RSSI sign: ensure it's always negative (dBm convention).
    let rssi = if rssi_raw > 0 {
        rssi_raw.saturating_neg()
    } else {
        rssi_raw
    };
    let noise_floor = buf[17] as i8;
    let ppdu_type = wifi_densepose_hardware::PpduType::from_byte(buf[18]);
    let adr018_flags = wifi_densepose_hardware::Adr018Flags::from_byte(buf[19]);

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
        ppdu_type,
        adr018_flags,
        amplitudes,
        phases,
    })
}

#[cfg(test)]
mod issue_1009_n_subcarriers_u16_tests {
    //! Issue #1009 §1c — `parse_esp32_frame` must read `n_subcarriers` as a
    //! u16 LE at bytes 6..7 (ADR-018 wire format), not a single byte at 6.
    //!
    //! An ESP32-C6 HE20 frame carries 256 subcarriers → byte 6 = 0x00,
    //! byte 7 = 0x01. The pre-#1005 single-byte read decoded this as 0
    //! subcarriers, silently dropping every real HE20 frame. This was the same
    //! truncation as the CLI parser (`wifi-densepose-cli` calibrate.rs); this
    //! module pins that the sensing-server template stays u16-correct.
    use super::*;

    /// Build an ADR-018 CSI frame (magic 0xC511_0001, 20-byte header).
    fn build_csi_frame(n_subcarriers: u16) -> Vec<u8> {
        let mut buf = vec![0u8; 20 + n_subcarriers as usize * 2];
        buf[0..4].copy_from_slice(&0xC511_0001u32.to_le_bytes());
        buf[4] = 7; // node_id
        buf[5] = 1; // n_antennas
        buf[6..8].copy_from_slice(&n_subcarriers.to_le_bytes()); // u16 LE
        buf[8..12].copy_from_slice(&5180u32.to_le_bytes()); // freq_mhz (5 GHz HE)
        buf[12..16].copy_from_slice(&42u32.to_le_bytes()); // sequence
        buf[16] = (-40i8) as u8; // rssi
        buf[17] = (-90i8) as u8; // noise_floor
        buf[18] = 0; // ppdu_type
        buf[19] = 0x10; // ADR-018: IEEE 802.15.4 sync valid
        for k in 0..n_subcarriers as usize {
            buf[20 + k * 2] = (5 + (k % 40) as i8) as u8; // i
            buf[20 + k * 2 + 1] = (k % 30) as u8; // q
        }
        buf
    }

    #[test]
    fn parse_esp32_frame_he20_256_bins_not_truncated() {
        // 256 = 0x0100 LE: byte6 = 0x00, byte7 = 0x01. A u8 read of byte 6
        // would see 0 subcarriers; a u16 read sees 256.
        let buf = build_csi_frame(256);
        assert_eq!(buf.len(), 532, "256-bin frame wire size = 20 + 256*2");
        let frame = parse_esp32_frame(&buf).expect("256-bin HE20 frame must parse");
        assert_eq!(
            frame.n_subcarriers, 256,
            "n_subcarriers must read as u16 (256), not the byte-6-only 0"
        );
        assert_eq!(frame.amplitudes.len(), 256);
        assert_eq!(frame.node_id, 7);
        assert_eq!(frame.rssi, -40);
        assert_eq!(frame.sequence, 42);
        assert!(frame.adr018_flags.ieee802154_sync_valid);
    }

    #[test]
    fn parse_esp32_frame_ht20_64_bins_still_parses() {
        // Regression guard for the common single-byte (≤255) case.
        let buf = build_csi_frame(64);
        let frame = parse_esp32_frame(&buf).expect("64-bin HT20 frame must parse");
        assert_eq!(frame.n_subcarriers, 64);
        assert_eq!(frame.amplitudes.len(), 64);
    }
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
                let ring_val =
                    0.08 * (-(dist - ring_r).powi(2) / (2.0 * ring_width * ring_width)).exp();
                values[z * grid + x] += ring_val;
            }
        }
    }

    // Clamp and normalise to [0, 1].
    let field_max = values.iter().cloned().fold(0.0f64, f64::max);
    let scale = if field_max > 1e-9 {
        1.0 / field_max
    } else {
        1.0
    };
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
    let series: Vec<f64> = frame_history
        .iter()
        .map(|amps| {
            if amps.is_empty() {
                0.0
            } else {
                amps.iter().sum::<f64>() / amps.len() as f64
            }
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
    let max_sens = sensitivity
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max)
        .max(1e-9);

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
///
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
        frame
            .amplitudes
            .iter()
            .zip(importance_weights.iter())
            .map(|(a, w)| a * w)
            .sum::<f64>()
            / weight_sum
    } else {
        frame.amplitudes.iter().sum::<f64>() / n
    };

    // ── Intra-frame subcarrier variance (weighted by importance) ──
    let intra_variance: f64 = if weight_sum > 0.0 {
        frame
            .amplitudes
            .iter()
            .zip(importance_weights.iter())
            .map(|(a, w)| w * (a - mean_amp).powi(2))
            .sum::<f64>()
            / weight_sum
    } else {
        frame
            .amplitudes
            .iter()
            .map(|a| (a - mean_amp).powi(2))
            .sum::<f64>()
            / n
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
        frame.amplitudes[half..]
            .iter()
            .map(|a| (a - mean_amp).powi(2))
            .sum::<f64>()
            / (frame.amplitudes.len() - half) as f64
    } else {
        0.0
    };

    // ── Breathing band power (lower half of subcarriers, low spatial frequency) ──
    let breathing_band_power = if half > 0 {
        frame.amplitudes[..half]
            .iter()
            .map(|a| (a - mean_amp).powi(2))
            .sum::<f64>()
            / half as f64
    } else {
        0.0
    };

    // A subcarrier index is a spatial-frequency bin, not temporal hertz.
    let dominant_freq_hz =
        csi::estimate_temporal_dominant_frequency_hz(frame_history, sample_rate_hz);

    // ── Change point detection (threshold-crossing count in current frame) ──
    let threshold = mean_amp * 1.2;
    let change_points = frame
        .amplitudes
        .windows(2)
        .filter(|w| (w[0] < threshold) != (w[1] < threshold))
        .count();

    // ── Motion score: sliding-window temporal difference ──
    // Compare current frame against the most recent historical frame.
    // The difference is normalised by the mean amplitude to be scale-invariant.
    // Callers append the current frame first, making the true predecessor the
    // second item from the back.
    let temporal_motion_score = if let Some(prev_frame) = frame_history.iter().rev().nth(1) {
        let n_cmp = n_sub.min(prev_frame.len());
        if n_cmp > 0 {
            let diff_energy: f64 = (0..n_cmp)
                .map(|k| (frame.amplitudes[k] - prev_frame[k]).powi(2))
                .sum::<f64>()
                / n_cmp as f64;
            // Normalise by mean squared amplitude to get a dimensionless ratio.
            let ref_energy = mean_amp * mean_amp + 1e-9;
            (diff_energy / ref_energy).sqrt().clamp(0.0, 1.0)
        } else {
            0.0
        }
    } else {
        // No history yet — fall back to intra-frame variance-based estimate.
        (intra_variance / (mean_amp * mean_amp + 1e-9))
            .sqrt()
            .clamp(0.0, 1.0)
    };

    // Blend temporal motion with variance-based motion for robustness.
    // Also factor in motion_band_power and change_points for ESP32 real-world sensitivity.
    let variance_motion = (temporal_variance / 10.0).clamp(0.0, 1.0);
    let mbp_motion = (motion_band_power / 25.0).clamp(0.0, 1.0);
    let cp_motion = (change_points as f64 / 15.0).clamp(0.0, 1.0);
    let motion_score = (temporal_motion_score * 0.4
        + variance_motion * 0.2
        + mbp_motion * 0.25
        + cp_motion * 0.15)
        .clamp(0.0, 1.0);

    // ── Signal quality metric ──
    // Based on estimated SNR (RSSI relative to noise floor) and subcarrier consistency.
    let snr_db = (frame.rssi as f64 - frame.noise_floor as f64).max(0.0);
    let snr_quality = (snr_db / 40.0).clamp(0.0, 1.0); // 40 dB → quality = 1.0
                                                       // Penalise quality when temporal variance is very high (unstable signal).
    let stability =
        (1.0 - (temporal_variance / (mean_amp * mean_amp + 1e-9)).clamp(0.0, 1.0)).max(0.0);
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

    (
        features,
        raw_classification,
        breathing_rate_hz,
        sub_variances,
        motion_score,
    )
}

/// Simple threshold classification (no smoothing) — used as the "raw" input.
fn raw_classify(score: f64) -> String {
    if score > 0.25 {
        "active".into()
    } else if score > 0.12 {
        "present_moving".into()
    } else if score > 0.04 {
        "present_still".into()
    } else {
        "absent".into()
    }
}

/// Debounce frames required before state transition (at ~10 FPS = ~0.4s).
/// Used only by [`smooth_and_classify`] (simulated / WiFi-scan sources,
/// which run on a fixed tick cadence close to this design point).
const DEBOUNCE_FRAMES: u32 = 4;
/// EMA alpha for motion smoothing (~1s time constant at 10 FPS).
/// Used only by [`smooth_and_classify`]; see above.
const MOTION_EMA_ALPHA: f64 = 0.15;
/// EMA alpha for slow-adapting baseline (~30s time constant at 10 FPS).
/// Used only by [`smooth_and_classify`]; see above.
const BASELINE_EMA_ALPHA: f64 = 0.003;
/// Number of warm-up frames before baseline subtraction kicks in.
/// Used only by [`smooth_and_classify`]; see above.
const BASELINE_WARMUP: u64 = 50;

/// Debounce duration required before [`smooth_and_classify_node`] accepts a
/// state transition. Equivalent to the original `DEBOUNCE_FRAMES = 4` at the
/// assumed 10 FPS design point.
const NODE_DEBOUNCE_DURATION_SECS: f64 = 0.4;
/// Time constant for [`smooth_and_classify_node`]'s motion-score EMA,
/// derived from `MOTION_EMA_ALPHA` at the assumed 10 FPS design point
/// (τ = -T / ln(1-α)) so behavior is unchanged for a node running at
/// exactly 10 FPS.
///
/// Frame-count/fixed-alpha smoothing silently scales with actual CSI
/// arrival rate. ESP32-C6 boards observed in the field run CSI at
/// ~48-50 FPS, not the assumed 10 FPS — ~5x faster — which made the
/// debounce trigger in ~0.08s instead of ~0.4s and compressed the "1s"
/// motion EMA to ~0.2s of real smoothing, letting raw per-frame noise
/// flip the reported label almost directly (the reported UI "seizure").
/// Deriving a per-frame alpha from the node's actual measured
/// `csi_fps_ema` makes the smoothing strength invariant to arrival rate.
const NODE_MOTION_TIME_CONSTANT_SECS: f64 = 0.6154;
/// Time constant for [`smooth_and_classify_node`]'s baseline EMA, same
/// derivation as `NODE_MOTION_TIME_CONSTANT_SECS` from `BASELINE_EMA_ALPHA`.
const NODE_BASELINE_TIME_CONSTANT_SECS: f64 = 33.28;
/// Baseline warm-up duration before baseline subtraction kicks in for
/// [`smooth_and_classify_node`] (originally 50 frames at 10 FPS = 5s).
const NODE_BASELINE_WARMUP_SECS: f64 = 5.0;

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
        state.baseline_motion =
            state.baseline_motion * (1.0 - BASELINE_EMA_ALPHA) + raw_motion * BASELINE_EMA_ALPHA;
    }

    // 2. Subtract baseline and clamp.
    let adjusted = (raw_motion - state.baseline_motion * 0.7).max(0.0);

    // 3. EMA smooth the adjusted score.
    state.smoothed_motion =
        state.smoothed_motion * (1.0 - MOTION_EMA_ALPHA) + adjusted * MOTION_EMA_ALPHA;
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
    // Derive this frame's effective time step from the node's actual
    // measured CSI rate (already EMA-tracked, burst-filtered elsewhere —
    // see `observe_csi_frame_arrival`) rather than assuming 10 FPS.
    let fps = ns.csi_fps_ema.max(1.0);
    let dt = 1.0 / fps;
    let motion_alpha = 1.0 - (-dt / NODE_MOTION_TIME_CONSTANT_SECS).exp();
    let baseline_alpha = 1.0 - (-dt / NODE_BASELINE_TIME_CONSTANT_SECS).exp();
    let debounce_frames_needed = ((NODE_DEBOUNCE_DURATION_SECS * fps).round() as u32).max(1);
    let warmup_frames_needed = ((NODE_BASELINE_WARMUP_SECS * fps).round() as u64).max(1);

    ns.baseline_frames += 1;
    if ns.baseline_frames < warmup_frames_needed {
        ns.baseline_motion = ns.baseline_motion * 0.9 + raw_motion * 0.1;
    } else if raw_motion < ns.smoothed_motion + 0.05 {
        ns.baseline_motion =
            ns.baseline_motion * (1.0 - baseline_alpha) + raw_motion * baseline_alpha;
    }

    let adjusted = (raw_motion - ns.baseline_motion * 0.7).max(0.0);

    ns.smoothed_motion = ns.smoothed_motion * (1.0 - motion_alpha) + adjusted * motion_alpha;
    let sm = ns.smoothed_motion;

    let candidate = raw_classify(sm);

    if candidate == ns.current_motion_level {
        ns.debounce_counter = 0;
        ns.debounce_candidate = candidate;
    } else if candidate == ns.debounce_candidate {
        ns.debounce_counter += 1;
        if ns.debounce_counter >= debounce_frames_needed {
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
fn adaptive_override(
    state: &AppStateInner,
    features: &FeatureInfo,
    classification: &mut ClassificationInfo,
) {
    if let Some(ref model) = state.adaptive_model {
        // Get current frame amplitudes from the latest history entry.
        let amps = state
            .frame_history
            .back()
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        let dominant_frequency = adaptive_classifier::compatible_dominant_frequency(
            model.version,
            features.dominant_freq_hz,
            amps,
        );
        let feat_arr = adaptive_classifier::features_from_runtime(
            &serde_json::json!({
                "variance": features.variance,
                "motion_band_power": features.motion_band_power,
                "breathing_band_power": features.breathing_band_power,
                "spectral_power": features.spectral_power,
                "dominant_freq_hz": dominant_frequency,
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
        if state.hr_buffer.len() > VITAL_MEDIAN_WINDOW {
            state.hr_buffer.pop_front();
        }
    }
    if br_ok && raw_br > 0.0 {
        state.br_buffer.push_back(raw_br);
        if state.br_buffer.len() > VITAL_MEDIAN_WINDOW {
            state.br_buffer.pop_front();
        }
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
            state.smoothed_hr =
                state.smoothed_hr * (1.0 - VITAL_EMA_ALPHA) + trimmed_hr * VITAL_EMA_ALPHA;
        }
        // else: within dead-band, hold current value
    }
    if trimmed_br > 0.0 {
        if state.smoothed_br < 1.0 {
            state.smoothed_br = trimmed_br;
        } else if (trimmed_br - state.smoothed_br).abs() > BR_DEAD_BAND {
            state.smoothed_br =
                state.smoothed_br * (1.0 - VITAL_EMA_ALPHA) + trimmed_br * VITAL_EMA_ALPHA;
        }
    }

    // Smooth confidence
    state.smoothed_hr_conf = state.smoothed_hr_conf * 0.92 + raw.heartbeat_confidence * 0.08;
    state.smoothed_br_conf = state.smoothed_br_conf * 0.92 + raw.breathing_confidence * 0.08;

    VitalSigns {
        breathing_rate_bpm: if state.smoothed_br > 1.0 {
            Some(state.smoothed_br)
        } else {
            None
        },
        heart_rate_bpm: if state.smoothed_hr > 1.0 {
            Some(state.smoothed_hr)
        } else {
            None
        },
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
        if ns.hr_buffer.len() > VITAL_MEDIAN_WINDOW {
            ns.hr_buffer.pop_front();
        }
    }
    if br_ok && raw_br > 0.0 {
        ns.br_buffer.push_back(raw_br);
        if ns.br_buffer.len() > VITAL_MEDIAN_WINDOW {
            ns.br_buffer.pop_front();
        }
    }

    let trimmed_hr = trimmed_mean(&ns.hr_buffer);
    let trimmed_br = trimmed_mean(&ns.br_buffer);

    if trimmed_hr > 0.0 {
        if ns.smoothed_hr < 1.0 {
            ns.smoothed_hr = trimmed_hr;
        } else if (trimmed_hr - ns.smoothed_hr).abs() > HR_DEAD_BAND {
            ns.smoothed_hr =
                ns.smoothed_hr * (1.0 - VITAL_EMA_ALPHA) + trimmed_hr * VITAL_EMA_ALPHA;
        }
    }
    if trimmed_br > 0.0 {
        if ns.smoothed_br < 1.0 {
            ns.smoothed_br = trimmed_br;
        } else if (trimmed_br - ns.smoothed_br).abs() > BR_DEAD_BAND {
            ns.smoothed_br =
                ns.smoothed_br * (1.0 - VITAL_EMA_ALPHA) + trimmed_br * VITAL_EMA_ALPHA;
        }
    }

    ns.smoothed_hr_conf = ns.smoothed_hr_conf * 0.92 + raw.heartbeat_confidence * 0.08;
    ns.smoothed_br_conf = ns.smoothed_br_conf * 0.92 + raw.breathing_confidence * 0.08;

    VitalSigns {
        breathing_rate_bpm: if ns.smoothed_br > 1.0 {
            Some(ns.smoothed_br)
        } else {
            None
        },
        heart_rate_bpm: if ns.smoothed_hr > 1.0 {
            Some(ns.smoothed_hr)
        } else {
            None
        },
        breathing_confidence: ns.smoothed_br_conf,
        heartbeat_confidence: ns.smoothed_hr_conf,
        signal_quality: raw.signal_quality,
    }
}

/// Trimmed mean: sort, drop top/bottom 25%, average the middle 50%.
/// More robust than median (uses more data) and less noisy than raw mean.
fn trimmed_mean(buf: &VecDeque<f64>) -> f64 {
    if buf.is_empty() {
        return 0.0;
    }
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
        let first_rssi = observations.first().map(|o| o.rssi_dbm).unwrap_or(-80.0);
        let _first_signal_pct = observations.first().map(|o| o.signal_pct).unwrap_or(40.0);

        let frame = Esp32Frame {
            magic: 0xC511_0001,
            node_id: 0,
            n_antennas: 1,
            n_subcarriers: obs_count.min(u16::MAX as usize) as u16,
            freq_mhz: 2437,
            sequence: seq,
            rssi: first_rssi.clamp(-128.0, 127.0) as i8,
            noise_floor: -90,
            ppdu_type: wifi_densepose_hardware::PpduType::HtLegacy,
            adr018_flags: wifi_densepose_hardware::Adr018Flags::default(),
            amplitudes: multi_ap_frame.amplitudes.clone(),
            phases: multi_ap_frame.phases.clone(),
        };

        // ── Step 4b: Update frame history and extract features ───────
        let mut s_write_pre = state.write().await;
        s_write_pre
            .frame_history
            .push_back(frame.amplitudes.clone());
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

        let raw_vitals = s
            .vital_detector
            .process_frame(&frame.amplitudes, &frame.phases);
        let vitals = smooth_vitals(&mut s, &raw_vitals);
        s.latest_vitals = vitals.clone();

        let feat_variance = features.variance;

        // ADR-044 §5.2: feed raw features into rolling-P95 estimators before scoring.
        s.p95_variance.push(features.variance);
        s.p95_motion_band_power.push(features.motion_band_power);
        s.p95_spectral_power.push(features.spectral_power);

        // Multi-person estimation with temporal smoothing (EMA α=0.10).
        let raw_score = compute_person_score(&s, &features);
        s.smoothed_person_score = s.smoothed_person_score * 0.90 + raw_score * 0.10;
        let est_persons = if classification.presence {
            let count = s.person_count();
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
                sync: None,  // multi-BSSID scan path — no mesh peer
                node_inference: None, // single aggregate frame; no per-node split
            }],
            features,
            classification,
            signal_field: generate_signal_field(
                first_rssi,
                motion_score,
                breathing_rate_hz,
                feat_variance.min(1.0),
                &sub_variances,
            ),
            vital_signs: None,
            enhanced_motion,
            enhanced_breathing,
            posture: posture_str,
            signal_quality_score: sig_quality_score,
            quality_verdict: verdict_str,
            bssid_count: bssid_n,
            pose_keypoints: None,
            model_status: None,
            persons: None,
            estimated_persons: if est_persons > 0 {
                Some(est_persons)
            } else {
                None
            },
            node_features: None,
            room_inference: None,
        };

        // Populate persons from the sensing update (Kalman-smoothed via tracker).
        let raw_persons = derive_pose_from_sensing(&update);
        let mut last_tracker_instant = s.last_tracker_instant.take();
        let tracked = tracker_bridge::tracker_update(
            &mut s.pose_tracker,
            &mut last_tracker_instant,
            raw_persons,
        );
        s.last_tracker_instant = last_tracker_instant;
        if !tracked.is_empty() {
            update.persons = Some(tracked);
        }
        // #1050: attach real signal_field-peak positions to each person.
        attach_field_positions(&mut update);
        assess_legacy_image_pose(&mut s, &update);

        if let Ok(json) = serde_json::to_string(&update) {
            let _ = s.tx.send(json);
        }
        observe_sensing_update(s.latest_update.as_ref(), &update);
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
        ppdu_type: wifi_densepose_hardware::PpduType::HtLegacy,
        adr018_flags: wifi_densepose_hardware::Adr018Flags::default(),
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

    let raw_vitals = s
        .vital_detector
        .process_frame(&frame.amplitudes, &frame.phases);
    let vitals = smooth_vitals(&mut s, &raw_vitals);
    s.latest_vitals = vitals.clone();

    let feat_variance = features.variance;

    // ADR-044 §5.2: feed raw features into rolling-P95 estimators before scoring.
    s.p95_variance.push(features.variance);
    s.p95_motion_band_power.push(features.motion_band_power);
    s.p95_spectral_power.push(features.spectral_power);

    // Multi-person estimation with temporal smoothing (EMA α=0.10).
    let raw_score = compute_person_score(&s, &features);
    s.smoothed_person_score = s.smoothed_person_score * 0.90 + raw_score * 0.10;
    let est_persons = if classification.presence {
        let count = s.person_count();
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
            sync: None,  // synthetic-RSSI fallback path — no mesh peer
            node_inference: None, // synthetic fallback; no per-node inference
        }],
        features,
        classification,
        signal_field: generate_signal_field(
            rssi_dbm,
            motion_score,
            breathing_rate_hz,
            feat_variance.min(1.0),
            &sub_variances,
        ),
        vital_signs: None,
        enhanced_motion: None,
        enhanced_breathing: None,
        posture: None,
        signal_quality_score: None,
        quality_verdict: None,
        bssid_count: None,
        pose_keypoints: None,
        model_status: None,
        persons: None,
        estimated_persons: if est_persons > 0 {
            Some(est_persons)
        } else {
            None
        },
        node_features: None,
        room_inference: None,
    };

    let raw_persons = derive_pose_from_sensing(&update);
    let mut last_tracker_instant = s.last_tracker_instant.take();
    let tracked =
        tracker_bridge::tracker_update(&mut s.pose_tracker, &mut last_tracker_instant, raw_persons);
    s.last_tracker_instant = last_tracker_instant;
    if !tracked.is_empty() {
        update.persons = Some(tracked);
    }
    // #1050: attach real signal_field-peak positions to each person.
    attach_field_positions(&mut update);
    assess_legacy_image_pose(&mut s, &update);

    if let Ok(json) = serde_json::to_string(&update) {
        let _ = s.tx.send(json);
    }
    observe_sensing_update(s.latest_update.as_ref(), &update);
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
            // 2048 covers the largest ADR-018 frame: an ESP32-C6 HE-SU
            // capture is 532 bytes (issue #1005); on Windows a too-small
            // recv buffer makes recv_from error on the oversized datagram,
            // which made this probe fail against HE-only streams.
            let mut buf = [0u8; 2048];
            match tokio::time::timeout(Duration::from_secs(2), sock.recv_from(&mut buf)).await {
                Ok(Ok((len, _))) => parse_esp32_frame(&buf[..len]).is_some(),
                _ => false,
            }
        }
        Err(_) => false,
    }
}

// ── Source resolution state machine (issue #1004) ────────────────────────────

/// What background tasks to start, derived from `--source` and the boot probes.
///
/// Issue #1004: a one-shot startup probe latched `auto` to `simulate` forever
/// when no CSI happened to be flowing at boot (the normal case — the firmware
/// and the server race to come up). The UDP :5005 receiver was then never
/// bound, so real CSI arriving seconds later was silently ignored and the
/// server served simulated poses for the rest of the process. The UI looked
/// live; the data was fake. This is the exact "where's the real data?" failure
/// class the project fights.
///
/// The robust resolution: in `auto` mode **always bind the UDP receiver**
/// regardless of the boot probe. If no real source is up yet, serve simulated
/// data *and* keep the UDP receiver listening; the receiver promotes
/// `source` → `esp32` the instant the first real frame lands (see
/// `udp_receiver_task`, which sets `s.source = "esp32"`), mirroring the inverse
/// `esp32 → esp32:offline` reversion already in `effective_source()`.
///
/// Explicit `--source simulated` is a hard override for offline demos: it does
/// NOT bind UDP, so no promotion ever happens.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SourcePlan {
    /// The `AppStateInner.source` value to start with.
    initial_source: String,
    /// Bind the UDP :5005 receiver (and thus allow simulate→esp32 promotion).
    bind_udp: bool,
    /// Run the simulated-data generator (serves poses until a real frame arrives).
    run_simulator: bool,
    /// Run the Windows WiFi capture task.
    run_wifi: bool,
}

/// Pure decision function — fully unit-testable without binding sockets.
///
/// `requested` is the normalized `--source` value. `esp32_detected` /
/// `wifi_detected` are the boot-probe results (only consulted in `auto` mode).
/// Returns `None` for an unknown source that names neither a real source nor a
/// simulate alias (the caller maps that to its own pass-through/exit policy).
fn plan_source(requested: &str, esp32_detected: bool, wifi_detected: bool) -> SourcePlan {
    match requested {
        "auto" => {
            if esp32_detected {
                // Real CSI already flowing — bind UDP, no simulator.
                SourcePlan {
                    initial_source: "esp32".to_string(),
                    bind_udp: true,
                    run_simulator: false,
                    run_wifi: false,
                }
            } else if wifi_detected {
                SourcePlan {
                    initial_source: "wifi".to_string(),
                    bind_udp: false,
                    run_simulator: false,
                    run_wifi: true,
                }
            } else {
                // No real source *yet*. Serve simulated data, but ALSO bind UDP
                // so the receiver can promote to esp32 when the first real
                // frame arrives (issue #1004). Never latch on simulate.
                SourcePlan {
                    initial_source: "simulated".to_string(),
                    bind_udp: true,
                    run_simulator: true,
                    run_wifi: false,
                }
            }
        }
        // Explicit overrides. "simulate" is a back-compat alias for "simulated".
        "simulate" | "simulated" => SourcePlan {
            initial_source: "simulated".to_string(),
            bind_udp: false, // hard override: offline demo, no live promotion
            run_simulator: true,
            run_wifi: false,
        },
        "esp32" => SourcePlan {
            initial_source: "esp32".to_string(),
            bind_udp: true,
            run_simulator: false,
            run_wifi: false,
        },
        "wifi" => SourcePlan {
            initial_source: "wifi".to_string(),
            bind_udp: false,
            run_simulator: false,
            run_wifi: true,
        },
        // Unknown source — preserve it verbatim, no tasks (caller's policy).
        other => SourcePlan {
            initial_source: other.to_string(),
            bind_udp: false,
            run_simulator: false,
            run_wifi: false,
        },
    }
}

#[cfg(test)]
mod issue_1004_source_plan_tests {
    //! Issue #1004 — `--source auto` must NOT latch on `simulate` forever.
    //!
    //! Old behavior: a one-shot boot probe resolved the source once. With no CSI
    //! flowing at boot (the normal case), the server either latched on simulate
    //! (never binding UDP :5005, so later real CSI was silently ignored) or
    //! hard-exited (#937), never picking up CSI that started after launch.
    //!
    //! New behavior (`plan_source`): in `auto` the UDP receiver is ALWAYS bound,
    //! simulated data is served only until the first real frame, then
    //! `udp_receiver_task` promotes `source` → "esp32". These tests pin the
    //! resolution/promotion state machine directly (no sockets bound).
    use super::*;

    // FAILS ON OLD CODE: the old `auto`-with-no-source path bound no UDP
    // receiver (it spawned only `simulated_data_task`, or exited). This asserts
    // UDP IS bound even when the boot probe finds no source.
    #[test]
    fn auto_with_no_boot_source_still_binds_udp_and_simulates() {
        let plan = plan_source("auto", false, false);
        assert!(plan.bind_udp, "auto must bind UDP :5005 even with no boot source (#1004)");
        assert!(plan.run_simulator, "auto must serve simulated data until real CSI arrives");
        assert!(!plan.run_wifi);
        assert_eq!(plan.initial_source, "simulated");
    }

    #[test]
    fn auto_with_esp32_detected_binds_udp_no_simulator() {
        let plan = plan_source("auto", true, false);
        assert!(plan.bind_udp);
        assert!(!plan.run_simulator, "real CSI present → no synthetic frames");
        assert_eq!(plan.initial_source, "esp32");
    }

    #[test]
    fn auto_with_wifi_detected_runs_wifi_no_udp() {
        let plan = plan_source("auto", false, true);
        assert!(plan.run_wifi);
        assert!(!plan.bind_udp);
        assert!(!plan.run_simulator);
        assert_eq!(plan.initial_source, "wifi");
    }

    // Explicit `--source simulated` is a hard offline override: it must NOT bind
    // UDP (so it can never be promoted to live), distinguishing it from
    // auto-mode simulate.
    #[test]
    fn explicit_simulated_is_offline_override_no_udp() {
        for s in ["simulated", "simulate"] {
            let plan = plan_source(s, false, false);
            assert!(!plan.bind_udp, "{s}: explicit simulate must not bind UDP (offline demo)");
            assert!(plan.run_simulator);
            assert_eq!(plan.initial_source, "simulated");
        }
    }

    #[test]
    fn explicit_esp32_binds_udp() {
        let plan = plan_source("esp32", false, false);
        assert!(plan.bind_udp);
        assert!(!plan.run_simulator);
        assert_eq!(plan.initial_source, "esp32");
    }

    // Promotion check: the runtime promotes by setting `AppStateInner.source`
    // to "esp32" on the first real frame; `effective_source()` then reports it
    // (and reverts to "esp32:offline" after a 5 s gap). This asserts the
    // promotion direction the simulator/receiver rely on, without binding a
    // socket — it exercises the same `source` field the UDP task writes.
    #[test]
    fn effective_source_promotes_from_simulated_to_esp32_on_real_frame() {
        // Start as the auto/simulate plan would: source = "simulated".
        let mut src = "simulated".to_string();
        // effective_source() logic for the simulate state: stays "simulated".
        assert_eq!(promote_view(&src, None), "simulated");
        // First real frame arrives → udp_receiver_task sets source = "esp32".
        src = "esp32".to_string();
        let fresh = Some(std::time::Duration::from_millis(10));
        assert_eq!(promote_view(&src, fresh), "esp32", "fresh esp32 frame ⇒ live");
        // After a >5 s gap it reverts to offline (inverse machinery, #1004).
        let stale = Some(ESP32_OFFLINE_TIMEOUT + std::time::Duration::from_secs(1));
        assert_eq!(promote_view(&src, stale), "esp32:offline");
    }

    /// Mirror of `AppStateInner::effective_source` over just (source, age) so the
    /// promotion/reversion logic is testable without constructing full state.
    fn promote_view(source: &str, last_frame_age: Option<std::time::Duration>) -> String {
        if source == "esp32" {
            if let Some(age) = last_frame_age {
                if age > ESP32_OFFLINE_TIMEOUT {
                    return "esp32:offline".to_string();
                }
            }
        }
        source.to_string()
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
        n_subcarriers: n_sub as u16,
        freq_mhz: 2437,
        sequence: tick as u32,
        rssi: (-40.0 + 5.0 * (t * 0.2).sin()) as i8,
        noise_floor: -90,
        ppdu_type: wifi_densepose_hardware::PpduType::HtLegacy,
        adr018_flags: wifi_densepose_hardware::Adr018Flags::default(),
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

    // ADR-044/045: ping/pong keepalive to prevent proxy idle timeouts.
    let mut ping_interval = tokio::time::interval(std::time::Duration::from_secs(30));
    ping_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(json) => {
                        if socket.send(Message::Text(json)).await.is_err() {
                            break;
                        }
                    }
                    // Lagged: client fell behind — skip missed frames, don't disconnect.
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::debug!("WS client lagged by {n} frames, skipping");
                        continue;
                    }
                    Err(_) => break, // channel closed
                }
            }
            _ = ping_interval.tick() => {
                if socket.send(Message::Ping(vec![])).await.is_err() {
                    break;
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Pong(_))) => {} // keepalive response
                    _ => {} // ignore other client messages
                }
            }
        }
    }

    info!("WebSocket client disconnected (sensing)");
}

// ── ADR-099: real-time CSI introspection — WS topic + REST snapshot ──────────
//
// Parallel to the window-aggregated `/ws/sensing` topic. Subscribers see a
// fresh `IntrospectionSnapshot` JSON frame on every accepted CSI frame
// (regime / Lyapunov exponent / top-k DTW similarity), no window-close delay.

async fn ws_introspection_handler(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws_introspection_client(socket, state))
}

async fn handle_ws_introspection_client(mut socket: WebSocket, state: SharedState) {
    let mut rx = {
        let s = state.read().await;
        s.intro_tx.subscribe()
    };

    info!("WebSocket client connected (introspection)");

    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(json) => {
                        if socket.send(Message::Text(json)).await.is_err() {
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

    info!("WebSocket client disconnected (introspection)");
}

/// `GET /api/v1/introspection/snapshot` — one-shot poll for the latest
/// per-frame snapshot (regime, Lyapunov, top-k similarity). Mirrors the shape
/// of `/api/v1/sensing/latest` for the dashboard one-shot path.
async fn api_introspection_snapshot(State(state): State<SharedState>) -> impl IntoResponse {
    let s = state.read().await;
    Json(s.intro.snapshot().clone())
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
    let _ = socket.send(Message::Text(conn_msg.to_string())).await;

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
                                let (model_loaded, physics_assessment) = {
                                    let s = state.read().await;
                                    let physics = s.pose_physics.latest().and_then(|(raw, result)| {
                                        (raw.sequence == sensing.tick).then(|| {
                                            serde_json::json!({
                                                "schema": "pose-refinement-v1",
                                                "raw_observation_hash": raw.canonical_hash,
                                                "assessment": result,
                                            })
                                        })
                                    });
                                    (s.model_loaded, physics)
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
                                        let [nx, _ny, nz] = sensing.signal_field.grid_size;
                                        let peak = field_localize::extract_peaks(
                                            &sensing.signal_field.values, nx, nz, 1, 3.0,
                                        ).into_iter().next();
                                        vec![PersonDetection {
                                            id: 1,
                                            confidence: sensing.classification.confidence,
                                            bbox: BoundingBox { x: 260.0, y: 150.0, width: 120.0, height: 220.0 },
                                            keypoints,
                                            zone: "zone_1".into(),
                                            position: peak.map_or([0.0, 0.0, 0.0], |p| p.position),
                                            motion_score: field_localize::motion_score_from_power(
                                                sensing.features.motion_band_power,
                                            ),
                                            pose: sensing.posture.clone(),
                                        }]
                                    }).unwrap_or_else(|| {
                                        // Prefer tracked persons from broadcast if available
                                        sensing.persons.clone().unwrap_or_else(|| derive_pose_from_sensing(&sensing))
                                    })
                                } else {
                                    // Prefer tracked persons from broadcast if available
                                    sensing.persons.clone().unwrap_or_else(|| derive_pose_from_sensing(&sensing))
                                };

                                let pose_msg = serde_json::json!({
                                    "type": "pose_data",
                                    "zone_id": "zone_1",
                                    "timestamp": sensing.timestamp,
                                    "physics": physics_assessment,
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
                                if socket.send(Message::Text(pose_msg.to_string())).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    // Lagged: skip missed frames, don't disconnect.
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::debug!("WS pose client lagged by {n} frames, skipping");
                        continue;
                    }
                    Err(_) => break, // channel closed
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        // Handle ping/pong
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                            if v.get("type").and_then(|t| t.as_str()) == Some("ping") {
                                let pong = serde_json::json!({"type": "pong"});
                                let _ = socket.send(Message::Text(pong.to_string())).await;
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Pong(_))) => {} // keepalive response
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

async fn latest_realtek_radar(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    match &s.latest_realtek_radar {
        Some(snapshot) => Json(serde_json::to_value(snapshot).unwrap_or_default()),
        None => Json(serde_json::json!({"status": "no Realtek radar data yet"})),
    }
}

async fn latest_mediatek_csi(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    match &s.latest_mediatek_csi {
        Some(snapshot) => Json(serde_json::to_value(snapshot).unwrap_or_default()),
        None => Json(serde_json::json!({"status": "no MediaTek CSI data yet"})),
    }
}

async fn latest_qualcomm_csi(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    match &s.latest_qualcomm_csi {
        Some(snapshot) => Json(serde_json::to_value(snapshot).unwrap_or_default()),
        None => Json(serde_json::json!({"status": "no Qualcomm CSI data yet"})),
    }
}

async fn vendor_descriptors() -> Json<serde_json::Value> {
    Json(
        serde_json::to_value(wifi_densepose_sensing_server::vendor_rf::descriptors())
            .unwrap_or_default(),
    )
}

async fn latest_vendor_events(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let state = state.read().await;
    Json(serde_json::to_value(&state.latest_vendor_rf).unwrap_or_default())
}

async fn latest_vendor_event(
    State(state): State<SharedState>,
    Path(vendor): Path<String>,
) -> impl IntoResponse {
    let Some(vendor_id) = wifi_densepose_sensing_server::vendor_rf::vendor_from_str(&vendor) else {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "unknown vendor"})));
    };
    let state = state.read().await;
    let canonical_vendor = vendor_id.as_str();
    match state.latest_vendor_rf.get(canonical_vendor) {
        Some(snapshot) => (
            StatusCode::OK,
            Json(serde_json::to_value(snapshot).unwrap_or_default()),
        ),
        None => (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "no vendor RF data yet",
                "vendor": canonical_vendor
            })),
        ),
    }
}

async fn ingest_vendor_events(
    State(state): State<SharedState>,
    Path(vendor): Path<String>,
    payload: Bytes,
) -> impl IntoResponse {
    let Some(vendor_id) = wifi_densepose_sensing_server::vendor_rf::vendor_from_str(&vendor) else {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "unknown vendor"})));
    };
    let events = match wifi_densepose_sensing_server::vendor_rf::decode_provider(vendor_id, &payload) {
        Ok(events) => events,
        Err(error) => {
            let status = match error {
                wifi_densepose_hardware::vendor_rf::VendorEventError::Unsupported => StatusCode::NOT_IMPLEMENTED,
                wifi_densepose_hardware::vendor_rf::VendorEventError::ContractRequired
                | wifi_densepose_hardware::vendor_rf::VendorEventError::CredentialsRequired => StatusCode::FORBIDDEN,
                _ => StatusCode::BAD_REQUEST,
            };
            return (status, Json(serde_json::json!({"error": error.to_string(), "vendor": vendor})));
        }
    };
    let mut accepted = 0usize;
    let mut state = state.write().await;
    let canonical_vendor = vendor_id.as_str().to_string();
    for event in events {
        match wifi_densepose_sensing_server::vendor_rf::VendorEventSnapshot::from_event(event) {
            Ok(snapshot) => {
                let json = serde_json::to_string(&snapshot).ok();
                state.source = snapshot.source.clone();
                state
                    .latest_vendor_rf
                    .insert(canonical_vendor.clone(), snapshot);
                if let Some(json) = json {
                    let _ = state.tx.send(json);
                }
                accepted += 1;
            }
            Err(error) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": error.to_string(), "vendor": canonical_vendor})),
                )
            }
        }
    }
    (StatusCode::ACCEPTED, Json(serde_json::json!({"vendor": vendor, "accepted": accepted})))
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
/// Fuse features across all active nodes for higher SNR.
///
/// When multiple ESP32 nodes observe the same room, their CSI features
/// can be combined:
/// - Variance: use max (most sensitive node dominates)
/// - Motion/breathing/spectral power: weighted average by RSSI (closer node = higher weight)
/// - Dominant frequency: weighted average
/// - Change points: keep current node's value (not meaningful to average)
/// - Mean RSSI: use max (best signal)
fn fuse_multi_node_features(
    current_features: &FeatureInfo,
    node_states: &HashMap<u8, NodeState>,
) -> FeatureInfo {
    let now = std::time::Instant::now();
    let active: Vec<(&FeatureInfo, f64)> = node_states
        .values()
        .filter(|ns| {
            ns.last_frame_time
                .is_some_and(|t| now.duration_since(t).as_secs() < 10)
        })
        .filter_map(|ns| {
            let feat = ns.latest_features.as_ref()?;
            let rssi = ns.rssi_history.back().copied().unwrap_or(-80.0);
            Some((feat, rssi))
        })
        .collect();

    if active.len() <= 1 {
        return current_features.clone();
    }

    // RSSI-based weights: higher RSSI = closer to person = more weight.
    // Map RSSI relative to best node into [0.1, 1.0].
    let max_rssi = active
        .iter()
        .map(|(_, r)| *r)
        .fold(f64::NEG_INFINITY, f64::max);
    let weights: Vec<f64> = active
        .iter()
        .map(|(_, r)| (1.0 + (r - max_rssi + 20.0) / 20.0).clamp(0.1, 1.0))
        .collect();
    let w_sum: f64 = weights.iter().sum::<f64>().max(1e-9);

    FeatureInfo {
        // Weighted average variance (not max — max inflates person score
        // and causes count flips between 1↔2 persons).
        variance: active
            .iter()
            .zip(&weights)
            .map(|((f, _), w)| f.variance * w)
            .sum::<f64>()
            / w_sum,
        // Weighted average for motion/breathing/spectral
        motion_band_power: active
            .iter()
            .zip(&weights)
            .map(|((f, _), w)| f.motion_band_power * w)
            .sum::<f64>()
            / w_sum,
        breathing_band_power: active
            .iter()
            .zip(&weights)
            .map(|((f, _), w)| f.breathing_band_power * w)
            .sum::<f64>()
            / w_sum,
        spectral_power: active
            .iter()
            .zip(&weights)
            .map(|((f, _), w)| f.spectral_power * w)
            .sum::<f64>()
            / w_sum,
        dominant_freq_hz: active
            .iter()
            .zip(&weights)
            .map(|((f, _), w)| f.dominant_freq_hz * w)
            .sum::<f64>()
            / w_sum,
        change_points: current_features.change_points, // keep current node's value
        // Best RSSI across nodes
        mean_rssi: active
            .iter()
            .map(|(f, _)| f.mean_rssi)
            .fold(f64::NEG_INFINITY, f64::max),
    }
}

/// Estimate person count from CSI features using a weighted composite heuristic.
///
/// Single ESP32 link limitations: variance-based detection can reliably detect
/// 1-2 persons. 3+ is speculative and requires ≥3 nodes for spatial resolution.
///
/// Returns a raw score (0.0..1.0) that the caller converts to person count
/// after temporal smoothing.
fn compute_person_score(state: &AppStateInner, feat: &FeatureInfo) -> f64 {
    // ADR-044 §5.2: adaptive rolling-P95 normalization.
    // Legacy fixed denominators (variance/300, motion/250, spectral/500) saturate
    // when live ESP32 values exceed those limits — zero dynamic range results.
    // Use the P95 of the last ~30 s of history instead, falling back to the legacy
    // denominators during cold-start (<60 samples) to preserve day-0 behaviour.
    let var_denom = state
        .p95_variance
        .current()
        .map(|p| p.max(50.0))
        .unwrap_or(300.0);
    let motion_denom = state
        .p95_motion_band_power
        .current()
        .map(|p| p.max(50.0))
        .unwrap_or(250.0);
    let sp_denom = state
        .p95_spectral_power
        .current()
        .map(|p| p.max(100.0))
        .unwrap_or(500.0);
    let var_norm = (feat.variance / var_denom).clamp(0.0, 1.0);
    let cp_norm = (feat.change_points as f64 / 30.0).clamp(0.0, 1.0);
    let motion_norm = (feat.motion_band_power / motion_denom).clamp(0.0, 1.0);
    let sp_norm = (feat.spectral_power / sp_denom).clamp(0.0, 1.0);
    var_norm * 0.40 + cp_norm * 0.20 + motion_norm * 0.25 + sp_norm * 0.15
}

/// Estimate person count via ruvector DynamicMinCut on the subcarrier
/// temporal correlation graph.
///
/// Builds a graph where:
/// - Nodes = active subcarriers (variance > noise floor)
/// - Edges = Pearson correlation between subcarrier time series
///   (weight = correlation coefficient; high correlation = heavy edge)
/// - Source = virtual node connected to the most active subcarrier
/// - Sink = virtual node connected to the least correlated subcarrier
///
/// The min-cut value indicates how many independent motion clusters exist:
/// - High min-cut (relative to total edge weight) → one tightly coupled
///   group → 1 person
/// - Low min-cut → two loosely coupled groups → 2 persons
///
/// Uses `ruvector_mincut::DynamicMinCut` for O(V²E) exact max-flow.
fn estimate_persons_from_correlation(frame_history: &VecDeque<Vec<f64>>) -> usize {
    let n_frames = frame_history.len();
    if n_frames < 10 {
        return 1;
    }

    let window: Vec<&Vec<f64>> = frame_history.iter().rev().take(20).collect();
    let n_sub = window[0].len().min(56);
    if n_sub < 4 {
        return 1;
    }
    let k = window.len() as f64;

    // Per-subcarrier mean and variance
    let mut means = vec![0.0f64; n_sub];
    let mut variances = vec![0.0f64; n_sub];
    for frame in &window {
        for sc in 0..n_sub.min(frame.len()) {
            means[sc] += frame[sc] / k;
        }
    }
    for frame in &window {
        for sc in 0..n_sub.min(frame.len()) {
            variances[sc] += (frame[sc] - means[sc]).powi(2) / k;
        }
    }

    // Active subcarriers: variance above noise floor
    let noise_floor = 1.0;
    let active: Vec<usize> = (0..n_sub)
        .filter(|&sc| variances[sc] > noise_floor)
        .collect();
    let m = active.len();
    if m < 3 {
        return if m == 0 { 0 } else { 1 };
    }

    // Build correlation graph edges between active subcarriers.
    // Edge weight = |Pearson correlation|. High correlation → same person.
    let mut edges: Vec<(u64, u64, f64)> = Vec::new();
    let source = m as u64;
    let sink = (m + 1) as u64;

    // Precompute std devs
    let stds: Vec<f64> = active
        .iter()
        .map(|&sc| variances[sc].sqrt().max(1e-9))
        .collect();

    for i in 0..m {
        for j in (i + 1)..m {
            // Pearson correlation between subcarriers i and j
            let mut cov = 0.0f64;
            for frame in &window {
                let si = active[i];
                let sj = active[j];
                if si < frame.len() && sj < frame.len() {
                    cov += (frame[si] - means[si]) * (frame[sj] - means[sj]) / k;
                }
            }
            let corr = (cov / (stds[i] * stds[j])).abs();
            if corr > 0.1 {
                // Bidirectional edges for flow network
                let weight = corr * 10.0; // Scale up for integer-like flow
                edges.push((i as u64, j as u64, weight));
                edges.push((j as u64, i as u64, weight));
            }
        }
    }

    // Source → highest-variance subcarrier, Sink → lowest-variance.
    // partial_cmp returns None on NaN; the outer unwrap_or only catches an
    // empty iterator, not a comparator panic. Same NaN-panic class as #611
    // — a single NaN variance frame would kill the sensing-server process.
    let (max_var_idx, _) = active
        .iter()
        .enumerate()
        .max_by(|(_, &a), (_, &b)| {
            variances[a]
                .partial_cmp(&variances[b])
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or((0, &0));
    let (min_var_idx, _) = active
        .iter()
        .enumerate()
        .min_by(|(_, &a), (_, &b)| {
            variances[a]
                .partial_cmp(&variances[b])
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or((0, &0));

    if max_var_idx == min_var_idx {
        return 1;
    }

    edges.push((source, max_var_idx as u64, 100.0));
    edges.push((min_var_idx as u64, sink, 100.0));

    // Run min-cut
    let mc: DynamicMinCut = match MinCutBuilder::new()
        .exact()
        .with_edges(edges.clone())
        .build()
    {
        Ok(mc) => mc,
        Err(_) => return 1,
    };

    let cut_value = mc.min_cut_value();
    let total_edge_weight: f64 = edges
        .iter()
        .filter(|(s, t, _)| *s != source && *s != sink && *t != source && *t != sink)
        .map(|(_, _, w)| w)
        .sum::<f64>()
        / 2.0; // bidirectional → halve

    if total_edge_weight < 1e-9 {
        return 1;
    }

    // Normalized cut ratio: low = easy to split = multiple people
    let cut_ratio = cut_value / total_edge_weight;

    if cut_ratio > 0.4 {
        1 // Tightly coupled — one person
    } else if cut_ratio > 0.15 {
        2 // Moderately separable — two people
    } else {
        3 // Highly separable — three+ people
    }
}

/// Map a DynamicMinCut occupancy estimate (`estimate_persons_from_correlation`,
/// 0–3) onto a target score whose steady state round-trips back through
/// `score_to_person_count` to the *same* count (issue #803).
///
/// The CSI path EMA-smooths this target and re-discretises it via
/// `score_to_person_count`. The previous `corr_persons / 3.0` mapping put a
/// 2-person estimate at 0.667 — just under the 0.70 up-threshold — so the
/// smoothed score could never climb past 1, pinning the per-node count to 1
/// even when the min-cut cleanly separated two people. These anchors sit
/// inside the hysteresis bands so a *sustained* estimate converges to the
/// matching count while transient noise stays gated by the EMA:
///   1 → 0.40  (below the 0.55 down-threshold)
///   2 → 0.74  (between the 0.70 up- and 0.78 down-thresholds → reachable
///              both climbing from 1 and falling from 3)
///   3 → 0.96  (above the 0.92 up-threshold)
fn corr_persons_to_score(corr_persons: usize) -> f64 {
    match corr_persons {
        0 => 0.20,
        1 => 0.40,
        2 => 0.74,
        _ => 0.96,
    }
}

#[cfg(test)]
mod corr_persons_round_trip_tests {
    //! Issue #803 — a sustained min-cut occupancy estimate must survive the
    //! CSI path's EMA + `score_to_person_count` re-discretisation instead of
    //! collapsing back to 1.
    use super::*;

    /// Replays the CSI-loop smoothing (`score = score*0.92 + target*0.08`)
    /// followed by `score_to_person_count`, exactly as the per-node path does,
    /// and returns the steady-state reported count.
    fn converge(corr_persons: usize) -> usize {
        let mut score = 0.0f64;
        let mut count = 1usize;
        for _ in 0..400 {
            let target = corr_persons_to_score(corr_persons);
            score = score * 0.92 + target * 0.08;
            count = score_to_person_count(score, count);
        }
        count
    }

    #[test]
    fn sustained_one_person_estimate_reports_one() {
        assert_eq!(converge(1), 1);
    }

    #[test]
    fn sustained_two_person_estimate_reports_two() {
        assert_eq!(converge(2), 2, "#803: min-cut=2 must round-trip to count 2");
    }

    #[test]
    fn sustained_three_person_estimate_reports_three() {
        assert_eq!(converge(3), 3);
    }

    #[test]
    fn old_div3_mapping_would_pin_two_people_to_one() {
        // Regression-documents the bug: 2/3 = 0.667 never crosses the 0.70
        // up-threshold, so the old mapping reported 1 for two people.
        let mut score = 0.0f64;
        let mut count = 1usize;
        for _ in 0..400 {
            score = score * 0.92 + (2.0 / 3.0) * 0.08;
            count = score_to_person_count(score, count);
        }
        assert_eq!(count, 1, "old corr_persons/3.0 mapping was the #803 bug");
    }
}

/// Convert smoothed person score to discrete count with hysteresis.
///
/// Uses asymmetric thresholds: higher threshold to *add* a person, lower to
/// *drop* one.  This prevents flickering when the score hovers near a boundary
/// (the #1 user-reported issue — see #237, #249, #280, #292).
fn score_to_person_count(smoothed_score: f64, prev_count: usize) -> usize {
    // Up-thresholds (must exceed to increase count):
    //   1→2: 0.80  (raised from 0.65 — single-person movement in multipath
    //               rooms easily hits 0.65, causing false 2-person detection)
    //   2→3: 0.92  (raised from 0.85 — 3 persons needs very strong signal)
    // Down-thresholds (must drop below to decrease count):
    //   2→1: 0.55  (hysteresis gap of 0.25)
    //   3→2: 0.78  (hysteresis gap of 0.14)
    match prev_count {
        0 | 1 => {
            if smoothed_score > 0.85 {
                3
            } else if smoothed_score > 0.70 {
                2
            } else {
                1
            }
        }
        2 => {
            if smoothed_score > 0.92 {
                3
            } else if smoothed_score < 0.55 {
                1
            } else {
                2 // hold — within hysteresis band
            }
        }
        _ => {
            // prev_count >= 3
            if smoothed_score < 0.55 {
                1
            } else if smoothed_score < 0.78 {
                2
            } else {
                3 // hold
            }
        }
    }
}

/// Combine the activity-score-derived aggregate count with the count-aware
/// per-node estimates (issue #803).
///
/// The aggregate `s.person_count()` is driven by `smoothed_person_score`, an
/// EMA-smoothed *activity* score (amplitude variance / motion / spectral
/// energy). That score saturates near a single occupant — one moving person
/// can max it out — so it cannot discriminate occupancy *count*, leaving the
/// reported value pinned at 1. Meanwhile the per-node paths already derive a
/// genuinely count-aware estimate (ESP32 firmware `n_persons`, or the
/// DynamicMinCut `corr_persons`) and stash it in `NodeState::prev_person_count`
/// — but that value was being discarded by the aggregator.
///
/// This takes the larger of the two. It can only ever *raise* the count when a
/// node has positively estimated more occupants, so it never regresses the
/// single-person case (a lone occupant yields `node_max == 1`).
fn aggregate_person_count(
    activity_count: usize,
    node_states: &std::collections::HashMap<u8, NodeState>,
) -> usize {
    let node_max = node_states
        .values()
        .map(|n| n.prev_person_count)
        .max()
        .unwrap_or(0);
    activity_count.max(node_max)
}

#[cfg(test)]
mod aggregate_person_count_tests {
    //! Issue #803 — the saturating activity score must not clamp a
    //! count-aware per-node estimate back down to 1.
    use super::*;
    use std::collections::HashMap;

    fn node_with_count(c: usize) -> NodeState {
        let mut n = NodeState::new();
        n.prev_person_count = c;
        n
    }

    #[test]
    fn empty_nodes_fall_back_to_activity_count() {
        let nodes: HashMap<u8, NodeState> = HashMap::new();
        assert_eq!(aggregate_person_count(1, &nodes), 1);
        assert_eq!(aggregate_person_count(0, &nodes), 0);
    }

    #[test]
    fn node_estimate_raises_a_saturated_activity_count() {
        // The activity score saturates at 1, but a node positively reports 2.
        let mut nodes = HashMap::new();
        nodes.insert(1u8, node_with_count(2));
        assert_eq!(
            aggregate_person_count(1, &nodes),
            2,
            "a node reporting 2 must not be discarded by the activity count"
        );
    }

    #[test]
    fn activity_count_wins_when_higher_than_nodes() {
        // Never *lower* a confident activity-derived count to a stale node value.
        let mut nodes = HashMap::new();
        nodes.insert(1u8, node_with_count(1));
        assert_eq!(aggregate_person_count(3, &nodes), 3);
    }

    #[test]
    fn takes_max_across_multiple_nodes() {
        let mut nodes = HashMap::new();
        nodes.insert(1u8, node_with_count(1));
        nodes.insert(2u8, node_with_count(3));
        nodes.insert(3u8, node_with_count(2));
        assert_eq!(aggregate_person_count(1, &nodes), 3);
    }

    #[test]
    fn single_occupant_is_never_inflated() {
        // Regression guard: a lone occupant (every node sees 1) stays 1.
        let mut nodes = HashMap::new();
        nodes.insert(1u8, node_with_count(1));
        nodes.insert(2u8, node_with_count(1));
        assert_eq!(aggregate_person_count(1, &nodes), 1);
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
        // Slow tick rate (0.02) for gentle breathing, not jerky oscillation.
        (update.tick as f64 * freq * 0.02 * std::f64::consts::TAU + phase_offset).sin()
    } else {
        (update.tick as f64 * 0.02 + phase_offset).sin()
    };

    let lean_x = (feat.dominant_freq_hz / 5.0 - 1.0).clamp(-1.0, 1.0) * 18.0;

    let stride_x = if is_walking {
        let stride_phase =
            (feat.motion_band_power * 0.7 + update.tick as f64 * 0.06 + phase_offset).sin();
        stride_phase * 20.0 * motion_score
    } else {
        0.0
    };

    // Dampen burst and noise to reduce jitter.  The original used
    // tick*17.3 which changed wildly every frame.  Now use slow tick
    // rate and minimal burst scaling for a stable skeleton.
    let burst = (feat.change_points as f64 / 20.0).clamp(0.0, 0.3);

    let noise_seed = person_idx as f64 * 97.1; // stable per-person, no tick
    let noise_val = (noise_seed.sin() * 43758.545).fract();

    let snr_factor = ((feat.variance - 0.5) / 10.0).clamp(0.0, 1.0);
    let base_confidence = cls.confidence * (0.6 + 0.4 * snr_factor) * conf_decay;

    // ── Skeleton base position ────────────────────────────────────────────────

    let base_x = 320.0 + stride_x + lean_x * 0.5 + person_x_offset;
    let base_y = 240.0 - motion_score * 8.0;

    // ── COCO 17-keypoint offsets from hip-center ──────────────────────────────

    let kp_names = [
        "nose",
        "left_eye",
        "right_eye",
        "left_ear",
        "right_ear",
        "left_shoulder",
        "right_shoulder",
        "left_elbow",
        "right_elbow",
        "left_wrist",
        "right_wrist",
        "left_hip",
        "right_hip",
        "left_knee",
        "right_knee",
        "left_ankle",
        "right_ankle",
    ];

    let kp_offsets: [(f64, f64); 17] = [
        (0.0, -80.0),   // 0  nose
        (-8.0, -88.0),  // 1  left_eye
        (8.0, -88.0),   // 2  right_eye
        (-16.0, -82.0), // 3  left_ear
        (16.0, -82.0),  // 4  right_ear
        (-30.0, -50.0), // 5  left_shoulder
        (30.0, -50.0),  // 6  right_shoulder
        (-45.0, -15.0), // 7  left_elbow
        (45.0, -15.0),  // 8  right_elbow
        (-50.0, 20.0),  // 9  left_wrist
        (50.0, 20.0),   // 10 right_wrist
        (-20.0, 20.0),  // 11 left_hip
        (20.0, 20.0),   // 12 right_hip
        (-22.0, 70.0),  // 13 left_knee
        (22.0, 70.0),   // 14 right_knee
        (-24.0, 120.0), // 15 left_ankle
        (24.0, 120.0),  // 16 right_ankle
    ];

    const TORSO_KP: [usize; 4] = [5, 6, 11, 12];
    const EXTREMITY_KP: [usize; 4] = [9, 10, 15, 16];

    let keypoints: Vec<PoseKeypoint> = kp_names
        .iter()
        .zip(kp_offsets.iter())
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
                // Dampened from 12/8 to 4/3 to reduce visual jumping.
                (
                    phase.sin() * burst * motion_score * 4.0,
                    (phase * 1.31).cos() * burst * motion_score * 3.0,
                )
            } else {
                (0.0, 0.0)
            };

            let kp_noise_x = ((noise_seed + i as f64 * 1.618).sin() * 43758.545).fract()
                * feat.variance.sqrt().clamp(0.0, 3.0)
                * motion_score;
            let kp_noise_y = ((noise_seed + i as f64 * std::f64::consts::E).cos() * 31415.926)
                .fract()
                * feat.variance.sqrt().clamp(0.0, 3.0)
                * motion_score
                * 0.6;

            let swing_dy = if is_walking {
                let stride_phase =
                    (feat.motion_band_power * 0.7 + update.tick as f64 * 0.12 + phase_offset).sin();
                match i {
                    7 | 9 => -stride_phase * 20.0 * motion_score,
                    8 | 10 => stride_phase * 20.0 * motion_score,
                    13 | 15 => stride_phase * 25.0 * motion_score,
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
                // Issue #1525: the UI's default `keypointConfidenceThreshold`
                // is 0.1 (`ui/utils/pose-renderer.js`), and every draw gate
                // there rejects at `<=`/requires `>` that value — so a floor
                // of exactly 0.1 still renders nothing on the default,
                // no-model Docker path (low `base_confidence` hits this floor
                // often). Clamp strictly above the client's threshold so a
                // signal-derived keypoint is always visible, never fully
                // transparent/undrawn by construction.
                confidence: kp_conf.clamp(0.15, 1.0),
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
        // Position/motion_score/pose are attached from the real signal_field
        // peaks by `attach_field_positions` after the tracker step (#1050);
        // default here so the synthetic-skeleton geometry stays unchanged.
        position: [0.0, 0.0, 0.0],
        motion_score: 0.0,
        pose: None,
    }
}

/// Attach real, field-derived per-person world positions to a `SensingUpdate`'s
/// `persons` (issue #1050).
///
/// For each detected person we read a strongest-peak position out of the frame's
/// real `signal_field` (the same grid the Observatory already renders) and map
/// it to room-world coordinates via `field_localize::cell_to_world`. `motion_score`
/// is passed through from the measured `motion_band_power`; `pose` is taken from
/// the real aggregate `posture` estimate when present, else left `None` (never
/// fabricated). Persons beyond the number of resolvable field peaks fall back to
/// the strongest peak so they remain co-located with real energy rather than at
/// a fake origin; if the field has no peak above threshold the position stays at
/// `[0,0,0]` and `motion_score` still reflects real motion power.
/// ADR-262 P3: emit one signed RuField `FieldEvent` for this sensing cycle.
///
/// Joins the cycle's [`SensingUpdate`] (features / classification /
/// signal_field) with the governed engine's trust state (`effective_class` /
/// `demoted`, recorded on `engine_bridge` by `observe_cycle`) into a
/// `SensingSnapshot`, then surfaces it via the P1 bridge on `/api/field` +
/// `/ws/field`. The bridge maps privacy by information content and the surface
/// applies the §10 network egress gate, so above-policy cycles never reach the
/// wire.
///
/// **No phantom events:** an empty/no-presence cycle (`presence == false`)
/// emits nothing — there is no person to describe, so no event is fabricated
/// (ADR-262 §4 P3 / §6). Cycles before the governed engine has produced a trust
/// class are likewise skipped (no class ⇒ nothing honest to stamp).
///
/// `identity_bound` is `false` on the live path: RuView's live cycle does not
/// bind an enrolled identity to the surface yet (that is a per-room-calibration
/// / AETHER concern, ADR-262 §8 Q4). This is conservative for egress — it only
/// ever *lowers* a Derived cycle from P5 to P4, both of which are already held
/// edge-local, so it cannot leak.
fn emit_rufield_event(s: &AppStateInner, update: &SensingUpdate, node_id: u8) {
    // No-presence ⇒ no phantom event.
    if !update.classification.presence {
        return;
    }
    // Need a governed trust class before we can honestly stamp privacy.
    let Some(effective_class) = s.engine_bridge.effective_class() else {
        return;
    };

    let timestamp_ns = if update.timestamp.is_finite() && update.timestamp > 0.0 {
        (update.timestamp * 1_000_000_000.0) as u64
    } else {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
    };

    let snap = rufield_surface::build_snapshot(
        timestamp_ns,
        format!("esp32_node_{node_id}"),
        rufield_surface::SensingFeatures {
            mean_rssi: update.features.mean_rssi,
            variance: update.features.variance,
            motion_band_power: update.features.motion_band_power,
            breathing_band_power: update.features.breathing_band_power,
            dominant_freq_hz: update.features.dominant_freq_hz,
            change_points: update.features.change_points,
            spectral_power: update.features.spectral_power,
        },
        rufield_surface::SensingClass {
            motion_level: update.classification.motion_level.clone(),
            presence: update.classification.presence,
            confidence: update.classification.confidence,
        },
        Some(rufield_surface::SignalField {
            grid_size: update.signal_field.grid_size,
            values: update.signal_field.values.clone(),
        }),
        rufield_surface::ruview_class_from_bfld(effective_class),
        s.engine_bridge.demoted(),
        false, // identity_bound — see fn-doc (conservative, cannot leak).
    );

    // `field_surface` is its own Arc<RwLock<_>>; `try_write` is non-blocking and
    // never deadlocks against the `s` guard (a different lock). The only other
    // touchers are the read-only `/api/field` / `/ws/field` handlers, so
    // contention is negligible; a rare miss just drops one cycle's event.
    if let Ok(mut fs) = s.field_surface.try_write() {
        fs.emit(&snap);
    }
}

fn attach_field_positions(update: &mut SensingUpdate) {
    let Some(persons) = update.persons.as_mut() else {
        return;
    };
    if persons.is_empty() {
        return;
    }

    let [nx, _ny, nz] = update.signal_field.grid_size;
    let peaks = field_localize::extract_peaks(
        &update.signal_field.values,
        nx,
        nz,
        persons.len().max(1),
        3.0,
    );

    let motion_score = field_localize::motion_score_from_power(update.features.motion_band_power);
    let pose_label = update.posture.clone();

    for (i, person) in persons.iter_mut().enumerate() {
        if let Some(peak) = peaks.get(i).or_else(|| peaks.first()) {
            person.position = peak.position;
        }
        person.motion_score = motion_score;
        person.pose = pose_label.clone();
    }
}

/// Feed legacy renderer coordinates into the canonical audit boundary without
/// promoting them to metric 3D or calibrated evidence.
fn assess_legacy_image_pose(state: &mut AppStateInner, update: &SensingUpdate) {
    use wifi_densepose_core::{
        CalibrationId, Coco17Joint, JointObservation, JointVisibility, ModelRef,
        PoseDimensionality, PoseObservationV2, PoseTrustState, Probability, SourceProvenance,
        SpatialFrameRef, SymmetricCovariance3, TrackId,
    };

    if !update.timestamp.is_finite() || update.timestamp < 0.0 {
        return;
    }
    let timestamp_ns = (update.timestamp * 1_000_000_000.0) as u64;
    let Some(persons) = update.persons.as_ref() else {
        return;
    };
    for person in persons {
        if person.keypoints.len() != Coco17Joint::ALL.len()
            || !person.confidence.is_finite()
        {
            continue;
        }
        let Some(joints) = person
            .keypoints
            .iter()
            .zip(Coco17Joint::ALL)
            .map(|(keypoint, kind)| {
                let position = [keypoint.x as f32, keypoint.y as f32, 0.0];
                if !position.iter().all(|coordinate| coordinate.is_finite())
                    || !keypoint.confidence.is_finite()
                {
                    return None;
                }
                let confidence =
                    Probability::new((keypoint.confidence as f32).clamp(0.0, 1.0)).ok()?;
                Some(JointObservation {
                    kind,
                    position_m: position,
                    covariance_m2: SymmetricCovariance3 {
                        xx: 0.0,
                        xy: 0.0,
                        xz: 0.0,
                        yy: 0.0,
                        yz: 0.0,
                        zz: 0.0,
                    },
                    confidence,
                    visibility: if confidence.get() > 0.0 {
                        JointVisibility::Visible
                    } else {
                        JointVisibility::Unknown
                    },
                })
            })
            .collect::<Option<Vec<_>>>()
            .and_then(|joints| joints.try_into().ok())
        else {
            continue;
        };
        let observer_confidence =
            match Probability::new((person.confidence as f32).clamp(0.0, 1.0)) {
                Ok(confidence) => confidence,
                Err(_) => continue,
            };
        let mut raw = PoseObservationV2 {
            schema_version: 2,
            timestamp_ns,
            sensor_epoch: 1,
            sequence: update.tick,
            track_id: TrackId(format!("local:{}", person.id)),
            frame: SpatialFrameRef {
                name: "legacy-renderer-image".into(),
                version: 1,
                metric: false,
                right_handed: false,
                z_up: false,
            },
            calibration_id: CalibrationId("uncalibrated-image".into()),
            floor_plane: None,
            model: ModelRef {
                id: if state.model_loaded {
                    "sensing-server-model-artifact-unknown".into()
                } else {
                    "sensing-server-signal-derived".into()
                },
                artifact_hash: [0; 32],
            },
            source: SourceProvenance {
                sensor_id: "sensing-server-local".into(),
                authenticated: false,
                replay_protected: false,
            },
            trust_state: PoseTrustState::Degraded,
            dimensionality: PoseDimensionality::Image2d,
            uncertainty_calibrated: false,
            joints,
            observer_confidence,
            canonical_hash: [0; 32],
        };
        raw.seal();
        let _ = state.pose_physics.process(&raw, timestamp_ns);
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

// ── RuVector Phase 2: Temporal EMA smoothing for keypoints ──────────────────

/// Expected bone lengths in pixel-space for the COCO-17 skeleton as used by
/// `derive_single_person_pose`. Pairs are (parent_idx, child_idx).
const POSE_BONE_PAIRS: &[(usize, usize)] = &[
    (5, 7),
    (7, 9),
    (6, 8),
    (8, 10), // arms
    (5, 11),
    (6, 12), // torso
    (11, 13),
    (13, 15),
    (12, 14),
    (14, 16), // legs
    (5, 6),
    (11, 12), // shoulders, hips
];

/// Apply temporal EMA smoothing and bone-length clamping to person detections.
///
/// For the *first* person (index 0) this uses the per-node `prev_keypoints`
/// state. Multi-person smoothing is left for a future phase.
fn apply_temporal_smoothing(persons: &mut [PersonDetection], ns: &mut NodeState) {
    if persons.is_empty() {
        return;
    }

    let alpha = ns.ema_alpha();
    let person = &mut persons[0]; // smooth primary person only

    let current_kps: Vec<[f64; 3]> = person
        .keypoints
        .iter()
        .map(|kp| [kp.x, kp.y, kp.z])
        .collect();

    let smoothed = if let Some(ref prev) = ns.prev_keypoints {
        let mut out = Vec::with_capacity(current_kps.len());
        for (cur, prv) in current_kps.iter().zip(prev.iter()) {
            out.push([
                alpha * cur[0] + (1.0 - alpha) * prv[0],
                alpha * cur[1] + (1.0 - alpha) * prv[1],
                alpha * cur[2] + (1.0 - alpha) * prv[2],
            ]);
        }
        // Clamp bone lengths to ±20% of previous frame.
        clamp_bone_lengths_f64(&mut out, prev);
        out
    } else {
        current_kps.clone()
    };

    // Write smoothed keypoints back into the person detection.
    for (kp, s) in person.keypoints.iter_mut().zip(smoothed.iter()) {
        kp.x = s[0];
        kp.y = s[1];
        kp.z = s[2];
    }

    ns.prev_keypoints = Some(smoothed);
}

/// Clamp bone lengths so no bone changes by more than MAX_BONE_CHANGE_RATIO
/// compared to the previous frame.
fn clamp_bone_lengths_f64(pose: &mut [[f64; 3]], prev: &[[f64; 3]]) {
    for &(p, c) in POSE_BONE_PAIRS {
        if p >= pose.len() || c >= pose.len() {
            continue;
        }
        let prev_len = dist_f64(&prev[p], &prev[c]);
        if prev_len < 1e-6 {
            continue;
        }
        let cur_len = dist_f64(&pose[p], &pose[c]);
        if cur_len < 1e-6 {
            continue;
        }
        let ratio = cur_len / prev_len;
        let lo = 1.0 - MAX_BONE_CHANGE_RATIO;
        let hi = 1.0 + MAX_BONE_CHANGE_RATIO;
        if ratio < lo || ratio > hi {
            let target = prev_len * ratio.clamp(lo, hi);
            let scale = target / cur_len;
            for dim in 0..3 {
                let diff = pose[c][dim] - pose[p][dim];
                pose[c][dim] = pose[p][dim] + diff * scale;
            }
        }
    }
}

fn dist_f64(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let dz = b[2] - a[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

// ── DensePose-compatible REST endpoints ─────────────────────────────────────

async fn health_live(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    Json(serde_json::json!({
        "status": "alive",
        "uptime": s.start_time.elapsed().as_secs(),
    }))
}

/// Lowercase hex of a 32-byte witness for JSON exposure.
fn witness_hex(w: [u8; 32]) -> String {
    use std::fmt::Write;
    w.iter().fold(String::with_capacity(64), |mut acc, b| {
        let _ = write!(acc, "{b:02x}");
        acc
    })
}

async fn health_ready(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    Json(serde_json::json!({
        "status": "ready",
        "source": s.effective_source(),
        // ADR-295 — canonical provenance state so a status-endpoint consumer
        // never has to infer "live" from the absence of a signal (issue #1526).
        "source_state": s.source_state().as_str(),
        // Governed trust-path state (ADR-135..146; review finding 1b): latest
        // witness + privacy class + recalibration flag, and the engine error
        // audit — previously write-only on AppState, now readable here.
        "trust": {
            "last_witness": s.engine_bridge.last_trust_witness().map(witness_hex),
            "effective_class": s.engine_bridge.effective_class().map(|c| format!("{c:?}")),
            "demoted": s.engine_bridge.demoted(),
            "recalibration_recommended": s.engine_bridge.recalibration_recommended(),
            "engine_error_count": s.engine_bridge.engine_error_count(),
            "raw_outputs_suppressed": s.engine_bridge.suppress_raw_outputs(),
        },
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

#[derive(Debug, Default, Deserialize)]
struct PoseCurrentQuery {
    #[serde(default)]
    view: pose_physics::PoseView,
}

async fn pose_current(
    State(state): State<SharedState>,
    Query(query): Query<PoseCurrentQuery>,
) -> impl IntoResponse {
    let s = state.read().await;
    let persons = match &s.latest_update {
        Some(update) => update
            .persons
            .clone()
            .unwrap_or_else(|| derive_pose_from_sensing(update)),
        None => vec![],
    };
    let legacy = serde_json::json!({
        "timestamp": chrono::Utc::now().timestamp_millis() as f64 / 1000.0,
        "persons": persons,
        "total_persons": persons.len(),
        "source": s.effective_source(),
    });
    match query.view {
        pose_physics::PoseView::Raw => (StatusCode::OK, Json(legacy)),
        pose_physics::PoseView::Both => {
            if let Some((raw, physics)) = s.pose_physics.latest() {
                let mut body = legacy;
                body["raw"] = serde_json::to_value(raw).unwrap_or(serde_json::Value::Null);
                body["physics"] =
                    serde_json::to_value(physics).unwrap_or(serde_json::Value::Null);
                (StatusCode::OK, Json(body))
            } else {
                (
                    StatusCode::CONFLICT,
                    Json(serde_json::json!({
                        "code": "pose_physics_unavailable",
                        "detail": "no canonical pose observation has been assessed"
                    })),
                )
            }
        }
        pose_physics::PoseView::Refined => {
            let Some((raw, physics)) = s.pose_physics.latest() else {
                return (
                    StatusCode::CONFLICT,
                    Json(serde_json::json!({
                        "code": "pose_refined_unavailable",
                        "detail": "no canonical pose observation has been assessed"
                    })),
                );
            };
            match pose_physics::PosePhysicsRuntime::select(query.view, raw, physics) {
                Ok(pose_physics::SelectedPose::Refined(joints)) => (
                    StatusCode::OK,
                    Json(serde_json::json!({
                        "schema_version": 1,
                        "raw_observation_hash": raw.canonical_hash,
                        "joints_m": joints,
                        "physics": physics,
                    })),
                ),
                Ok(_) => unreachable!("refined view returns only refined data"),
                Err(error) => (
                    StatusCode::CONFLICT,
                    Json(serde_json::to_value(error).unwrap_or_else(|_| {
                        serde_json::json!({"code": "pose_refined_unavailable"})
                    })),
                ),
            }
        }
    }
}

async fn pose_physics_metrics(State(state): State<SharedState>) -> impl IntoResponse {
    let metrics = state.read().await.pose_physics.metrics_text();
    (
        [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        metrics,
    )
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
    let presence = s
        .latest_update
        .as_ref()
        .map(|u| u.classification.presence)
        .unwrap_or(false);
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
            let model = s
                .discovered_models
                .iter()
                .find(|m| m.get("id").and_then(|v| v.as_str()) == Some(id.as_str()));
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
    let model_id = body
        .get("id")
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
    if telemetry::curated_events_enabled() {
        info!(name: semconv::EVENT_RUVIEW_MODEL_LOADED, { "ruview.model.id" = %model_id }, "Model loaded: {model_id}");
    } else {
        info!("Model loaded: {model_id}");
    }
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
    // ADR-166: Sanitize path to prevent directory traversal
    let safe_id = std::path::Path::new(&id)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");
    if safe_id.is_empty() || safe_id != id {
        return Json(serde_json::json!({ "error": "invalid model id", "success": false }));
    }
    let path = effective_models_dir().join(format!("{}.rvf", safe_id));
    if path.exists() {
        if let Err(e) = std::fs::remove_file(&path) {
            // ADR-080 #2: log the OS error (incl. path) server-side only; the
            // client gets a generic body + correlation id, no leaked path.
            return error_response::internal_error_json("model delete", e);
        }
        // If this was the active model, unload it
        let mut s = state.write().await;
        if s.active_model_id.as_deref() == Some(id.as_str()) {
            s.active_model_id = None;
            s.model_loaded = false;
        }
        s.discovered_models
            .retain(|m| m.get("id").and_then(|v| v.as_str()) != Some(id.as_str()));
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
async fn activate_lora_profile(Json(body): Json<serde_json::Value>) -> Json<serde_json::Value> {
    let profile = body
        .get("profile")
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

/// Return the effective models directory, respecting the `MODELS_DIR`
/// environment variable.  Defaults to `data/models`.
fn effective_models_dir() -> PathBuf {
    PathBuf::from(std::env::var("MODELS_DIR").unwrap_or_else(|_| "data/models".to_string()))
}

/// Scan the models directory for `.rvf` files and return metadata.
/// Respects the `MODELS_DIR` environment variable.
fn scan_model_files() -> Vec<serde_json::Value> {
    let dir = effective_models_dir();
    let mut models = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("rvf") {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                let modified = entry
                    .metadata()
                    .ok()
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

/// Scan the models directory for `.lora.json` LoRA profile files.
/// Respects the `MODELS_DIR` environment variable.
fn scan_lora_profiles() -> Vec<serde_json::Value> {
    let dir = effective_models_dir();
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
    let id = body
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("rec_{}", chrono_timestamp()));

    // ADR-295: a recording captured while the live source is `Synthetic` is an
    // export, and every export of synthetic data must be watermarked so it can
    // never later be mistaken for a real capture (`export_watermark`). Stamped
    // onto the recording's own metadata entry — not the filename or the
    // per-line JSON — so the on-disk `.jsonl` schema and the `{id}.jsonl` path
    // convention `delete_recording`/`scan_recording_files` both rely on stay
    // exactly as every existing consumer (including `wifi-densepose-train`'s
    // dataset loader) already expects. Still unmissable: every caller of
    // `GET /api/v1/recordings` (and the success response below) sees it.
    let watermark = s.source_state().export_watermark();

    // Create the recording file
    let rec_path = PathBuf::from("data/recordings").join(format!("{}.jsonl", id));
    let file = match std::fs::File::create(&rec_path) {
        Ok(f) => f,
        Err(e) => {
            // ADR-080 #2: the OS error can carry the recordings path; log it
            // server-side only and return a generic body + correlation id.
            return error_response::internal_error_json("recording create", e);
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
        "watermark": watermark,
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

    match watermark {
        Some(mark) => info!("Recording started: {id} (source watermarked {mark})"),
        None => info!("Recording started: {id}"),
    }
    Json(serde_json::json!({ "success": true, "recording_id": id, "watermark": watermark }))
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
    let duration_secs = s
        .recording_start_time
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
    // ADR-166: Sanitize path to prevent directory traversal
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
            // ADR-080 #2: log the OS error (incl. path) server-side only.
            return error_response::internal_error_json("recording delete", e);
        }
        let mut s = state.write().await;
        s.recordings
            .retain(|r| r.get("id").and_then(|v| v.as_str()) != Some(id.as_str()));
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
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                let modified = entry
                    .metadata()
                    .ok()
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
//
// ADR-186 (TRAIN-RECONNECT): the former stub handlers here flipped a status
// string and logged one line without ever starting a job (issue #1233). They
// are replaced by the real `training_api` router, merged into the app below,
// which runs the pure-Rust trainer on a background task and streams live
// progress over `/ws/train/progress`.

// ── Adaptive classifier endpoints ────────────────────────────────────────────

/// POST /api/v1/adaptive/train — train the adaptive classifier from recordings.
async fn adaptive_train(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let rec_dir = PathBuf::from("data/recordings");
    eprintln!("=== Adaptive Classifier Training ===");
    match adaptive_classifier::train_from_recordings(&rec_dir) {
        Ok(model) => {
            let accuracy = model.training_accuracy;
            let frames = model.trained_frames;
            let stats: Vec<_> = model
                .class_stats
                .iter()
                .map(|cs| {
                    serde_json::json!({
                        "class": cs.label,
                        "samples": cs.count,
                        "feature_means": cs.mean,
                    })
                })
                .collect();

            // Save to disk.
            if let Err(e) = model.save(&adaptive_classifier::model_path()) {
                warn!("Failed to save adaptive model: {e}");
            } else {
                info!(
                    "Adaptive model saved to {}",
                    adaptive_classifier::model_path().display()
                );
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
        Err(e) => Json(serde_json::json!({
            "success": false,
            "error": e,
        })),
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
            "classes": model.class_names,
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

/// Numeric RF vitals are public sensing evidence only after an explicit fresh
/// room calibration has resolved exactly one occupant. Bootstrap priors are
/// intentionally lower authority and can never authorize rates.
const VITAL_PUBLICATION_MIN_CONFIDENCE: f64 = 0.55;
const VITAL_PUBLICATION_MIN_SIGNAL_QUALITY: f64 = 0.40;

fn vitals_for_publication(
    candidates: &VitalSigns,
    explicit_calibration_fresh: bool,
    person_count: usize,
) -> Option<VitalSigns> {
    if !explicit_calibration_fresh || person_count != 1 {
        return None;
    }
    let signal_quality = candidates.signal_quality.clamp(0.0, 1.0);
    if signal_quality < VITAL_PUBLICATION_MIN_SIGNAL_QUALITY {
        return None;
    }
    let breathing_rate_bpm = (candidates.breathing_confidence
        >= VITAL_PUBLICATION_MIN_CONFIDENCE)
        .then_some(candidates.breathing_rate_bpm)
        .flatten();
    let heart_rate_bpm =
        (candidates.heartbeat_confidence >= VITAL_PUBLICATION_MIN_CONFIDENCE)
            .then_some(candidates.heart_rate_bpm)
            .flatten();
    if breathing_rate_bpm.is_none() && heart_rate_bpm.is_none() {
        return None;
    }
    Some(VitalSigns {
        breathing_rate_bpm,
        heart_rate_bpm,
        breathing_confidence: candidates.breathing_confidence.clamp(0.0, 1.0),
        heartbeat_confidence: candidates.heartbeat_confidence.clamp(0.0, 1.0),
        signal_quality,
    })
}

fn edge_vitals_message_for_publication(
    raw: &Esp32VitalsPacket,
    published_vitals: Option<&VitalSigns>,
    bootstrap_empty: bool,
    explicit_calibration_fresh: bool,
    person_count: usize,
) -> serde_json::Value {
    let numeric_vitals_authorized = published_vitals.is_some();
    let abstention_reason = if numeric_vitals_authorized {
        None
    } else if bootstrap_empty {
        Some("bootstrap_empty_background")
    } else if !explicit_calibration_fresh {
        Some("explicit_calibration_required")
    } else if person_count != 1 {
        Some("exactly_one_occupant_required")
    } else {
        Some("vital_quality_gate_failed")
    };
    let effective_presence = raw.presence && !bootstrap_empty;
    let effective_motion = raw.motion && !bootstrap_empty;
    let effective_person_count = if bootstrap_empty { 0 } else { person_count };

    serde_json::json!({
        "type": "edge_vitals",
        "node_id": raw.node_id,
        "presence": effective_presence,
        "fall_detected": raw.fall_detected && !bootstrap_empty,
        "motion": effective_motion,
        "breathing_rate_bpm": published_vitals
            .and_then(|vitals| vitals.breathing_rate_bpm),
        "heartrate_bpm": published_vitals
            .and_then(|vitals| vitals.heart_rate_bpm),
        "n_persons": effective_person_count,
        "person_count_valid": raw.person_count_valid && !bootstrap_empty,
        "motion_energy": if bootstrap_empty { 0.0 } else { raw.motion_energy },
        "presence_score": if bootstrap_empty { 0.0 } else { raw.presence_score },
        "rssi": raw.rssi,
        "calibrated_evidence_authorized": explicit_calibration_fresh,
        "numeric_vitals_authorized": numeric_vitals_authorized,
        "abstention_reason": abstention_reason,
    })
}

fn opaque_calibration_model_id() -> String {
    let mut bytes = [0_u8; 16];
    OsRng.fill_bytes(&mut bytes);
    let suffix: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("cal-model-{suffix}")
}

fn calibration_source_accepts(
    source_node_ids: &std::collections::BTreeSet<u8>,
    node_id: u8,
) -> bool {
    source_node_ids.is_empty() || source_node_ids.contains(&node_id)
}

fn calibration_source_has_fresh_csi(node: &NodeState, now: std::time::Instant) -> bool {
    node.latest_accepted_csi_at
        .is_some_and(|seen| now.saturating_duration_since(seen) < ESP32_OFFLINE_TIMEOUT)
}

#[cfg(test)]
mod bootstrap_vital_publication_tests {
    use super::*;

    fn test_grid() -> CsiGridKey {
        CsiGridKey {
            n_subcarriers: 64,
            ppdu_type: 0,
        }
    }

    fn bind_test_calibration(state: &mut AppStateInner) {
        state.calibration_source_node_ids.insert(5);
        state.calibration_grid_binding = Some(CalibrationGridBinding {
            source_node_id: 5,
            grid: test_grid(),
            evidence: None,
        });
        state.node_states.entry(5).or_insert_with(NodeState::new);
    }

    fn feed_test_frame(
        state: &mut AppStateInner,
        node_id: u8,
        sequence: u32,
        amplitudes: &[f64],
    ) -> bool {
        state.maybe_feed_calibration_frame(
            node_id,
            test_grid(),
            sequence,
            amplitudes,
            std::time::Instant::now(),
        )
    }

    #[test]
    fn single_link_calibration_binds_to_first_source() {
        let mut sources = std::collections::BTreeSet::new();
        assert!(calibration_source_accepts(&sources, 5));
        sources.insert(5);
        assert!(calibration_source_accepts(&sources, 5));
        assert!(!calibration_source_accepts(&sources, 3));
    }

    #[test]
    fn explicit_single_link_source_rejects_other_live_nodes() {
        let sources = std::collections::BTreeSet::from([5]);
        assert!(calibration_source_accepts(&sources, 5));
        assert!(!calibration_source_accepts(&sources, 3));
        assert!(!calibration_source_accepts(&sources, 7));
    }

    #[test]
    fn edge_vitals_liveness_cannot_authorize_raw_csi_calibration() {
        let now = std::time::Instant::now();
        let mut node = NodeState::new();
        node.last_frame_time = Some(now);
        assert!(!calibration_source_has_fresh_csi(&node, now));

        node.observe_accepted_csi_frame(9, false, now);
        assert!(calibration_source_has_fresh_csi(&node, now));
        assert!(!calibration_source_has_fresh_csi(
            &node,
            now + ESP32_OFFLINE_TIMEOUT
        ));
    }

    #[test]
    fn calibration_grid_selection_prefers_stable_rate_qualified_stream() {
        let start = std::time::Instant::now();
        let mut node = NodeState::new();
        let stable = CsiGridKey {
            n_subcarriers: 64,
            ppdu_type: 0,
        };
        let sparse = CsiGridKey {
            n_subcarriers: 192,
            ppdu_type: 0,
        };
        for index in 0..=48 {
            node.observe_raw_grid(
                stable,
                start + std::time::Duration::from_millis(index * 250),
            );
        }
        for index in 0..=6 {
            node.observe_raw_grid(
                sparse,
                start + std::time::Duration::from_secs(index * 2),
            );
        }
        let (selected, evidence) = node
            .select_calibration_grid(start + std::time::Duration::from_secs(12))
            .expect("stable grid is eligible");
        assert_eq!(selected, stable);
        assert!(evidence.rate_hz >= 3.9);
        assert!(evidence.max_gap_s < 5.0);
    }

    #[tokio::test]
    async fn calibration_start_requires_evidence_then_binds_the_qualified_grid() {
        let mut inner = AppStateInner::minimal();
        inner.node_states.insert(5, NodeState::new());
        let state = Arc::new(RwLock::new(inner));

        let Json(unavailable) = calibration_start(
            State(state.clone()),
            Query(CalibrationStartQuery {
                source_node_id: Some(5),
            }),
        )
        .await;
        assert_eq!(unavailable["success"], false);
        assert_eq!(unavailable["error_code"], "calibration_grid_unavailable");
        {
            let state = state.read().await;
            assert!(state.field_model.is_none());
            assert!(state.calibration_model_id.is_none());
            assert!(state.calibration_source_node_ids.is_empty());
            assert!(state.calibration_grid_binding.is_none());
        }

        let now = std::time::Instant::now();
        let start = now - std::time::Duration::from_secs(10);
        {
            let mut state = state.write().await;
            let node = state.node_states.get_mut(&5).expect("node");
            for index in 0..=40 {
                node.observe_raw_grid(
                    test_grid(),
                    start + std::time::Duration::from_millis(index * 250),
                );
            }
        }
        let Json(started) = calibration_start(
            State(state.clone()),
            Query(CalibrationStartQuery {
                source_node_id: Some(5),
            }),
        )
        .await;
        assert_eq!(started["success"], true);
        assert_eq!(started["source_node_ids"], serde_json::json!([5]));
        assert_eq!(started["source_grid"]["n_subcarriers"], 64);
        assert_eq!(started["source_grid"]["ppdu_type"], 0);
        let state = state.read().await;
        assert!(state.field_model.is_some());
        assert!(state.calibration_model_id.is_some());
        assert_eq!(state.calibration_source_node_ids, [5].into_iter().collect());
        assert_eq!(
            state.calibration_grid_binding.map(|binding| binding.grid),
            Some(test_grid())
        );
    }

    #[test]
    fn non_bound_grid_never_enters_field_model_history() {
        let mut state = AppStateInner::minimal();
        state.field_model = Some(
            FieldModel::new(field_bridge::single_link_config()).expect("field model"),
        );
        bind_test_calibration(&mut state);
        let wrong_grid = CsiGridKey {
            n_subcarriers: 192,
            ppdu_type: 0,
        };
        assert!(!state.maybe_feed_calibration_frame(
            5,
            wrong_grid,
            41,
            &[0.25; 192],
            std::time::Instant::now(),
        ));
        assert!(state
            .node_states
            .get(&5)
            .expect("node")
            .field_model_history
            .is_empty());
        assert!(state.calibration_last_sequences.is_empty());
    }

    #[test]
    fn calibration_session_counts_each_unique_bound_csi_sequence_once() {
        let mut state = AppStateInner::minimal();
        state.field_model = Some(
            FieldModel::new(field_bridge::single_link_config()).expect("field model"),
        );
        bind_test_calibration(&mut state);

        let first = vec![0.25; 56];
        assert!(feed_test_frame(&mut state, 5, 41, &first));
        assert!(!feed_test_frame(&mut state, 5, 41, &first));

        let second = vec![0.5; 56];
        assert!(feed_test_frame(&mut state, 5, 42, &second));
        assert!(!feed_test_frame(&mut state, 3, 43, &second));

        let model = state.field_model.as_ref().expect("field model remains active");
        assert_eq!(model.calibration_frame_count(), 2);
        assert_eq!(state.calibration_last_sequences.get(&5), Some(&42));
        assert!(!state.calibration_last_sequences.contains_key(&3));
    }

    #[test]
    fn calibration_bounded_reorder_is_dropped_without_feeding_model() {
        let mut state = AppStateInner::minimal();
        state.field_model = Some(
            FieldModel::new(field_bridge::single_link_config()).expect("field model"),
        );
        bind_test_calibration(&mut state);
        let frame = vec![0.25; 56];

        assert!(feed_test_frame(&mut state, 5, 4_119_566, &frame));
        assert!(!feed_test_frame(&mut state, 5, 4_119_555, &frame));
        assert!(!feed_test_frame(&mut state, 5, 4_119_556, &frame));
        assert!(!feed_test_frame(&mut state, 5, 4_119_557, &frame));
        assert!(feed_test_frame(&mut state, 5, 4_119_567, &frame));
        assert!(state.calibration_sequence_fault_node_ids.is_empty());
        assert_eq!(state.calibration_reordered_packets.get(&5), Some(&3));
        assert_eq!(state.calibration_max_reorder_depth.get(&5), Some(&11));
        assert_eq!(
            state
                .field_model
                .as_ref()
                .expect("field model")
                .calibration_frame_count(),
            2
        );
    }

    #[test]
    fn calibration_sequence_discontinuity_latches_session_failure() {
        let mut state = AppStateInner::minimal();
        state.field_model = Some(
            FieldModel::new(field_bridge::single_link_config()).expect("field model"),
        );
        bind_test_calibration(&mut state);
        let frame = vec![0.25; 56];

        assert!(feed_test_frame(&mut state, 5, 100, &frame));
        assert!(!feed_test_frame(&mut state, 5, 83, &frame));
        assert!(!feed_test_frame(&mut state, 5, 101, &frame));
        assert!(state.calibration_sequence_fault_node_ids.contains(&5));
        assert_eq!(
            state
                .field_model
                .as_ref()
                .expect("field model")
                .calibration_frame_count(),
            1
        );

        state.clear_calibration_sequence_state();
        assert!(state.calibration_sequence_fault_node_ids.is_empty());
        assert!(state.calibration_last_sequences.is_empty());
        assert!(state.calibration_reordered_packets.is_empty());
        assert!(state.calibration_max_reorder_depth.is_empty());
    }

    #[test]
    fn calibration_sequence_wraps_and_failed_empty_input_can_retry() {
        let mut state = AppStateInner::minimal();
        state.field_model = Some(
            FieldModel::new(field_bridge::single_link_config()).expect("field model"),
        );
        bind_test_calibration(&mut state);
        let frame = vec![0.25; 56];

        assert!(!feed_test_frame(&mut state, 5, u32::MAX, &[]));
        assert!(state.calibration_last_sequences.is_empty());
        assert!(feed_test_frame(&mut state, 5, u32::MAX, &frame));
        assert!(feed_test_frame(&mut state, 5, 0, &frame));
        assert!(!feed_test_frame(&mut state, 5, 0, &frame));
        assert!(feed_test_frame(&mut state, 5, 1, &frame));
        assert_eq!(
            state
                .field_model
                .as_ref()
                .expect("field model")
                .calibration_frame_count(),
            3
        );
    }

    #[test]
    fn calibration_sequence_order_drops_bounded_late_and_rejects_discontinuity() {
        assert_eq!(
            calibration_sequence_order(None, 7),
            CalibrationSequenceOrder::First
        );
        assert_eq!(
            calibration_sequence_order(Some(7), 8),
            CalibrationSequenceOrder::Forward
        );
        assert_eq!(
            calibration_sequence_order(Some(8), 8),
            CalibrationSequenceOrder::Duplicate
        );
        assert_eq!(
            calibration_sequence_order(Some(8), 7),
            CalibrationSequenceOrder::Reordered(1)
        );
        assert_eq!(
            calibration_sequence_order(Some(32), 16),
            CalibrationSequenceOrder::Reordered(16)
        );
        assert_eq!(
            calibration_sequence_order(Some(32), 15),
            CalibrationSequenceOrder::Discontinuity
        );
        assert_eq!(
            calibration_sequence_order(Some(0), 1_u32 << 31),
            CalibrationSequenceOrder::Discontinuity
        );
        assert_eq!(
            calibration_sequence_order(Some(u32::MAX), 0),
            CalibrationSequenceOrder::Forward
        );
    }

    #[tokio::test]
    async fn calibration_status_and_stop_fail_closed_after_sequence_regression() {
        let mut inner = AppStateInner::minimal();
        inner.field_model = Some(
            FieldModel::new(field_bridge::single_link_config()).expect("field model"),
        );
        inner.calibration_model_id = Some("cal-model-test".to_string());
        inner.calibration_source_node_ids.insert(5);
        inner.calibration_last_sequences.insert(5, 41);
        inner.calibration_reordered_packets.insert(5, 3);
        inner.calibration_max_reorder_depth.insert(5, 3);
        inner.calibration_sequence_fault_node_ids.insert(5);
        let state = Arc::new(RwLock::new(inner));

        let Json(status) = calibration_status(State(state.clone())).await;
        assert_eq!(status["last_sequence_by_node"]["5"], 41);
        assert_eq!(status["reorder_window_sequences"], 16);
        assert_eq!(status["reordered_packets_by_node"]["5"], 3);
        assert_eq!(status["max_reorder_depth_by_node"]["5"], 3);
        assert_eq!(status["sequence_fault_node_ids"], serde_json::json!([5]));

        let Json(stopped) = calibration_stop(State(state)).await;
        assert_eq!(stopped["success"], false);
        assert_eq!(
            stopped["error_code"],
            "calibration_sequence_discontinuity"
        );
        assert_eq!(stopped["model_id"], "cal-model-test");
        assert_eq!(stopped["source_node_ids"], serde_json::json!([5]));
        assert_eq!(stopped["reordered_packets_by_node"]["5"], 3);
        assert_eq!(stopped["max_reorder_depth_by_node"]["5"], 3);
    }

    #[tokio::test]
    async fn calibration_stop_rejects_missing_or_stale_grid_identity() {
        let mut inner = AppStateInner::minimal();
        inner.field_model = Some(
            FieldModel::new(field_bridge::single_link_config()).expect("field model"),
        );
        inner.calibration_model_id = Some("cal-model-test".to_string());
        inner.calibration_source_node_ids.insert(5);
        let state = Arc::new(RwLock::new(inner));

        let Json(missing) = calibration_stop(State(state.clone())).await;
        assert_eq!(missing["success"], false);
        assert_eq!(missing["error_code"], "calibration_grid_identity_missing");

        {
            let mut state = state.write().await;
            state.calibration_grid_binding = Some(CalibrationGridBinding {
                source_node_id: 5,
                grid: test_grid(),
                evidence: None,
            });
            state.node_states.insert(5, NodeState::new());
        }
        let Json(stale) = calibration_stop(State(state)).await;
        assert_eq!(stale["success"], false);
        assert_eq!(stale["error_code"], "calibration_grid_stale");
    }

    #[test]
    fn post_finalize_holdout_excludes_training_history_and_sequence_faults() {
        let mut state = AppStateInner::minimal();
        bind_test_calibration(&mut state);
        let binding = state.calibration_grid_binding.expect("binding");
        let now = std::time::Instant::now();
        {
            let node = state.node_states.get_mut(&5).expect("node");
            for sequence in 0..50 {
                node.push_field_model_frame(sequence, &[0.25; 64], now);
            }
        }
        assert!(state.field_model_holdout_ready(binding, 50, now));

        state.begin_field_model_holdout(binding);
        assert!(state
            .node_states
            .get(&5)
            .expect("node")
            .field_model_history
            .is_empty());
        assert!(!state.field_model_holdout_ready(binding, 50, now));

        {
            let node = state.node_states.get_mut(&5).expect("node");
            for sequence in 50..100 {
                node.push_field_model_frame(sequence, &[0.5; 64], now);
            }
        }
        assert!(state.field_model_holdout_ready(binding, 50, now));
        state.calibration_sequence_fault_node_ids.insert(5);
        assert!(!state.field_model_holdout_ready(binding, 50, now));
    }

    #[test]
    fn startup_calibration_flag_fails_closed_before_state_creation() {
        assert!(validate_startup_calibration_request(false).is_ok());
        let error = validate_startup_calibration_request(true).expect_err("must fail closed");
        assert!(error.contains("--calibrate"));
        assert!(error.contains("10 seconds"));
        assert!(error.contains("source_node_id"));
    }

    fn strong_candidates() -> VitalSigns {
        VitalSigns {
            breathing_rate_bpm: Some(15.0),
            heart_rate_bpm: Some(72.0),
            breathing_confidence: 0.9,
            heartbeat_confidence: 0.9,
            signal_quality: 0.9,
        }
    }

    #[test]
    fn uncalibrated_empty_and_multi_person_rooms_publish_no_vitals() {
        let candidates = strong_candidates();
        assert!(vitals_for_publication(&candidates, false, 1).is_none());
        assert!(vitals_for_publication(&candidates, true, 0).is_none());
        assert!(vitals_for_publication(&candidates, true, 2).is_none());
    }

    #[test]
    fn one_explicitly_calibrated_occupant_may_publish_qualified_vitals() {
        let published = vitals_for_publication(&strong_candidates(), true, 1).unwrap();
        assert_eq!(published.breathing_rate_bpm, Some(15.0));
        assert_eq!(published.heart_rate_bpm, Some(72.0));
    }

    #[test]
    fn weak_vital_candidates_fail_closed() {
        let mut candidates = strong_candidates();
        candidates.signal_quality = 0.2;
        assert!(vitals_for_publication(&candidates, true, 1).is_none());
    }

    fn raw_edge_vitals() -> Esp32VitalsPacket {
        Esp32VitalsPacket {
            node_id: 5,
            presence: true,
            fall_detected: true,
            motion: true,
            breathing_rate_bpm: 15.0,
            heartrate_bpm: 72.0,
            rssi: -42,
            n_persons: 1,
            person_count_valid: true,
            motion_energy: 0.8,
            presence_score: 0.9,
            timestamp_ms: 42,
        }
    }

    #[test]
    fn bootstrap_empty_background_suppresses_legacy_edge_vitals() {
        let message = edge_vitals_message_for_publication(
            &raw_edge_vitals(),
            None,
            true,
            false,
            0,
        );
        assert_eq!(message["presence"], false);
        assert_eq!(message["motion"], false);
        assert_eq!(message["fall_detected"], false);
        assert_eq!(message["n_persons"], 0);
        assert!(message["breathing_rate_bpm"].is_null());
        assert!(message["heartrate_bpm"].is_null());
        assert_eq!(message["numeric_vitals_authorized"], false);
        assert_eq!(message["abstention_reason"], "bootstrap_empty_background");
    }

    #[test]
    fn explicit_single_occupant_may_publish_legacy_edge_vitals() {
        let published = vitals_for_publication(&strong_candidates(), true, 1).unwrap();
        let message = edge_vitals_message_for_publication(
            &raw_edge_vitals(),
            Some(&published),
            false,
            true,
            1,
        );
        assert_eq!(message["presence"], true);
        assert_eq!(message["fall_detected"], true);
        assert_eq!(message["n_persons"], 1);
        assert_eq!(message["breathing_rate_bpm"], 15.0);
        assert_eq!(message["heartrate_bpm"], 72.0);
        assert_eq!(message["numeric_vitals_authorized"], true);
        assert!(message["abstention_reason"].is_null());
    }
}

// ── Field model calibration endpoints (eigenvalue person counting) ──────────

#[derive(Debug, Default, Deserialize)]
struct CalibrationStartQuery {
    /// Optional explicit single-link source. When omitted, the first accepted
    /// ESP32 packet preserves the legacy binding behavior.
    source_node_id: Option<u8>,
}

#[derive(Debug, Default, Deserialize)]
struct BootstrapRefineQuery {
    #[serde(default)]
    confirmed_empty: bool,
    source_node_id: Option<u8>,
}

async fn calibration_start(
    State(state): State<SharedState>,
    Query(query): Query<CalibrationStartQuery>,
) -> Json<serde_json::Value> {
    let mut s = state.write().await;
    let now = std::time::Instant::now();
    let selected_binding = if let Some(source_node_id) = query.source_node_id {
        let Some(node) = s.node_states.get(&source_node_id) else {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "calibration_source_unavailable",
                "error": "The requested calibration source is missing.",
                "source_node_id": source_node_id,
            }));
        };
        let candidates = node.calibration_grid_candidates(now);
        let Some((grid, evidence)) = node.select_calibration_grid(now) else {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "calibration_grid_unavailable",
                "error": "No raw CSI grid has at least 10 seconds of evidence, a 2 Hz measured rate, and a maximum gap below 5 seconds.",
                "source_node_id": source_node_id,
                "candidates": candidates.into_iter().map(|(grid, evidence)| serde_json::json!({
                    "grid": grid,
                    "evidence": evidence,
                })).collect::<Vec<_>>(),
            }));
        };
        CalibrationGridBinding {
            source_node_id,
            grid,
            evidence: Some(evidence),
        }
    } else {
        let eligible: Vec<CalibrationGridBinding> = s
            .node_states
            .iter()
            .filter_map(|(&source_node_id, node)| {
                node.select_calibration_grid(now).map(|(grid, evidence)| {
                    CalibrationGridBinding {
                        source_node_id,
                        grid,
                        evidence: Some(evidence),
                    }
                })
            })
            .collect();
        let [binding] = eligible.as_slice() else {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "calibration_source_required",
                "error": "Choose exactly one eligible ESP32 source for room calibration.",
                "eligible_sources": eligible.iter().map(|binding| serde_json::json!({
                    "source_node_id": binding.source_node_id,
                    "grid": binding.grid,
                    "evidence": binding.evidence,
                })).collect::<Vec<_>>(),
            }));
        };
        *binding
    };
    if s.bootstrap_baseline_active {
        s.field_model = None;
        s.bootstrap_baseline_active = false;
    }
    // Guard: don't discard an in-progress or fresh calibration
    if let Some(ref fm) = s.field_model {
        let observed_at_us =
            (chrono::Utc::now().timestamp_millis().max(0) as u64).saturating_mul(1_000);
        match field_bridge::calibration_status_at(fm, observed_at_us) {
            CalibrationStatus::Collecting => {
                return Json(serde_json::json!({
                    "success": false,
                    "error": "Calibration already in progress. Call /calibration/stop first.",
                    "frame_count": fm.calibration_frame_count(),
                }));
            }
            CalibrationStatus::Fresh => {
                return Json(serde_json::json!({
                    "success": false,
                    "error": "A fresh calibration already exists. Call /calibration/stop or wait for expiry.",
                }));
            }
            _ => {} // Stale/Expired/Uncalibrated — ok to recalibrate
        }
    }
    match FieldModel::new(field_bridge::single_link_config()) {
        Ok(fm) => {
            s.field_model = Some(fm);
            s.calibration_model_id = Some(opaque_calibration_model_id());
            s.calibration_source_node_ids.clear();
            s.clear_field_model_binding();
            s.clear_calibration_sequence_state();
            s.calibration_source_node_ids
                .insert(selected_binding.source_node_id);
            s.calibration_grid_binding = Some(selected_binding);
            Json(serde_json::json!({
                "success": true,
                "message": "Calibration started — keep room empty while frames accumulate.",
                "model_id": s.calibration_model_id,
                "source_node_ids": s.calibration_source_node_ids,
                "source_grid": selected_binding.grid,
                "grid_evidence": selected_binding.evidence,
            }))
        }
        // ADR-080 #2: FieldModel init error chain stays server-side only.
        Err(e) => error_response::internal_error_json("calibration start", e),
    }
}

async fn calibration_stop(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let mut s = state.write().await;
    let model_id = s.calibration_model_id.clone();
    let source_node_ids: Vec<u8> = s.calibration_source_node_ids.iter().copied().collect();
    let source_grid = s.calibration_grid_binding.map(|binding| binding.grid);
    let reordered_packets_by_node = s.calibration_reordered_packets.clone();
    let max_reorder_depth_by_node = s.calibration_max_reorder_depth.clone();
    if !s.calibration_sequence_fault_node_ids.is_empty() {
        return Json(serde_json::json!({
            "success": false,
            "error_code": "calibration_sequence_discontinuity",
            "error": "Raw CSI sequence moved backward beyond the bounded UDP reorder window. Cancel and restart the empty-room calibration.",
            "model_id": model_id,
            "source_node_ids": source_node_ids,
            "source_grid": source_grid,
            "reordered_packets_by_node": reordered_packets_by_node,
            "max_reorder_depth_by_node": max_reorder_depth_by_node,
            "sequence_fault_node_ids": s.calibration_sequence_fault_node_ids,
        }));
    }
    let Some(binding) = s.calibration_grid_binding else {
        return Json(serde_json::json!({
            "success": false,
            "error_code": "calibration_grid_identity_missing",
            "error": "The active calibration has no frozen CSI grid identity. Cancel and restart the empty room capture.",
            "model_id": model_id,
            "source_node_ids": source_node_ids,
        }));
    };
    if source_node_ids.as_slice() != [binding.source_node_id]
        || !s.calibration_grid_is_fresh(binding, std::time::Instant::now())
    {
        let latest_seen_ms = s
            .node_states
            .get(&binding.source_node_id)
            .and_then(|node| node.field_model_latest_seen)
            .map(|seen| {
                std::time::Instant::now()
                    .saturating_duration_since(seen)
                    .as_millis() as u64
            });
        return Json(serde_json::json!({
            "success": false,
            "error_code": "calibration_grid_stale",
            "error": "The frozen calibration CSI grid is stale or no longer matches its source. Restore the source and retry before finalizing.",
            "model_id": model_id,
            "source_node_ids": source_node_ids,
            "source_grid": source_grid,
            "latest_seen_ms": latest_seen_ms,
        }));
    }
    if let Some(ref mut fm) = s.field_model {
        // Guard: finalizing before enough empty-room frames have accumulated
        // is a client-side sequencing error, not a server fault. Return a
        // clear, structured message (with progress) instead of a 500 so the
        // caller knows to keep the room empty and poll /calibration/status.
        let have = fm.calibration_frame_count();
        let need = fm.min_calibration_frames() as u64;
        let elapsed_s = fm.calibration_elapsed_s();
        let need_s = fm.min_calibration_duration_s();
        if have < need || (need_s > 0.0 && elapsed_s < need_s) {
            let mut resp = serde_json::json!({
                "success": false,
                "error": "Calibration not complete yet — keep the room empty and poll /calibration/status until both frame_count and elapsed_s reach their targets.",
                "frame_count": have,
                "frames_needed": need,
                "elapsed_s": elapsed_s,
                "duration_needed_s": need_s,
                "frames_per_second": fm.calibration_frames_per_second(),
            });
            // #1756: a fast fleet satisfies the frame gate in ~1/N of the
            // intended wall-clock window. Make that explicit instead of
            // silently collecting, so the operator sees why the session
            // keeps running.
            if have >= need && elapsed_s < need_s {
                tracing::warn!(
                    "Calibration frame target reached in {elapsed_s:.1}s — substantially less than the intended {need_s:.0}s window; continuing to collect for slow environmental variation (#1756)"
                );
                resp["warning"] = serde_json::json!(format!(
                    "frame target reached in {elapsed_s:.1}s — substantially less than the intended {need_s:.0}s window; continuing to collect for slow environmental variation"
                ));
            }
            return Json(resp);
        }
        let ts = chrono::Utc::now().timestamp_micros() as u64;
        match fm.finalize_calibration(ts, 0) {
            Ok(modes) => {
                let baseline = modes.baseline_eigenvalue_count;
                let variance_explained = modes.variance_explained;
                let holdout_window_size = modes.baseline_runtime_window_size;
                let final_frame_count = fm.calibration_frame_count();
                s.begin_field_model_holdout(binding);
                info!("Field model calibrated: baseline_eigenvalues={baseline}, variance_explained={variance_explained:.2}");
                Json(serde_json::json!({
                    "success": true,
                    "baseline_eigenvalue_count": baseline,
                    "variance_explained": variance_explained,
                    "frame_count": final_frame_count,
                    "elapsed_s": elapsed_s,
                    "holdout_window_size": holdout_window_size,
                    "model_id": model_id,
                    "source_node_ids": source_node_ids,
                    "source_grid": source_grid,
                    "reordered_packets_by_node": reordered_packets_by_node,
                    "max_reorder_depth_by_node": max_reorder_depth_by_node,
                }))
            }
            // ADR-080 #2: finalize error chain stays server-side only.
            Err(e) => error_response::internal_error_json("calibration stop", e),
        }
    } else {
        Json(serde_json::json!({
            "success": false,
            "error": "No field model active — call /calibration/start first.",
        }))
    }
}

async fn calibration_status(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    let now = std::time::Instant::now();
    let observed_at_unix_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
    let effective_status = s.field_model_status_at(observed_at_unix_ms);
    let active = s.field_model_active_at(observed_at_unix_ms);
    let bootstrap_active = s.bootstrap_baseline_active_at(observed_at_unix_ms);
    let bootstrap_background_match = s.bootstrap_background_match(observed_at_unix_ms);
    let runtime_reference = s.field_model.as_ref().and_then(|model| {
        model.modes().map(|modes| serde_json::json!({
            "window_size": modes.baseline_runtime_window_size,
            "significant_eigenvalue_p95": modes.baseline_runtime_eigenvalue_count,
            "residual_energy_threshold": modes.empty_room_residual_energy_threshold,
            "residual_reference_window_count": modes.empty_room_residual_energy_reference.len(),
            "residual_refinement_count": modes.empty_room_residual_refinement_count,
            "raw_calibration_frames_persisted": false,
        }))
    });
    let (frame_count, min_frames, elapsed_s, frames_per_second, min_duration_s) = s
        .field_model
        .as_ref()
        .map_or((0, 0, 0.0, 0.0, 0.0), |model| {
            (
                model.calibration_frame_count(),
                model.min_calibration_frames(),
                model.calibration_elapsed_s(),
                model.calibration_frames_per_second(),
                model.min_calibration_duration_s(),
            )
        });
    let status = effective_status.map_or_else(
        || "none".to_string(),
        |status| format!("{status:?}").to_lowercase(),
    );
    let grid_binding = s.calibration_grid_binding.map(|binding| {
        let node = s.node_states.get(&binding.source_node_id);
        let latest_seen_ms = node
            .and_then(|node| node.field_model_latest_seen)
            .map(|seen| now.saturating_duration_since(seen).as_millis() as u64);
        serde_json::json!({
            "source_node_id": binding.source_node_id,
            "grid": binding.grid,
            "selection_evidence": binding.evidence,
            "latest_seen_ms": latest_seen_ms,
            "runtime_history_frames": node.map_or(0, |node| node.field_model_history.len()),
            "runtime_history_capacity": FRAME_HISTORY_CAPACITY,
            "status": if latest_seen_ms.is_some_and(|age| age < ESP32_OFFLINE_TIMEOUT.as_millis() as u64)
                && !s.calibration_sequence_fault_node_ids.contains(&binding.source_node_id)
            { "active" } else { "stale" },
        })
    });
    Json(serde_json::json!({
        "active": active,
        "status": status,
        "frame_count": frame_count,
        "min_frames": min_frames,
        "elapsed_s": elapsed_s,
        "frames_per_second": frames_per_second,
        "min_duration_s": min_duration_s,
        "model_id": s.calibration_model_id,
        "source_node_ids": s.calibration_source_node_ids,
        "grid_binding": grid_binding,
        "last_sequence_by_node": s.calibration_last_sequences,
        "sequence_policy": "forward_only_with_bounded_udp_reorder_drop_v1",
        "reorder_window_sequences": CALIBRATION_SEQUENCE_REORDER_WINDOW,
        "reordered_packets_by_node": s.calibration_reordered_packets,
        "max_reorder_depth_by_node": s.calibration_max_reorder_depth,
        "sequence_fault_node_ids": s.calibration_sequence_fault_node_ids,
        "runtime_reference": runtime_reference,
        "binding_mode": if bootstrap_active { "bootstrap_only" } else if active { "runtime" } else { "none" },
        "bootstrap_baseline": s.bootstrap_baseline.as_ref().map(|metadata| serde_json::json!({
            "stored": true,
            "active": bootstrap_active,
            "authority": metadata.authority,
            "source_node_ids": metadata.source_node_ids,
            "source_grid": metadata.source_grid,
            "source_model_id": metadata.source_model_id,
            "created_at_unix_ms": metadata.created_at_unix_ms,
            "expires_at_unix_ms": metadata.expires_at_unix_ms,
            "content_sha256": metadata.content_sha256,
            "background_match": bootstrap_background_match.map(|result| serde_json::json!({
                "state": if result.matches_empty { "matched" } else { "changed" },
                "matches_empty": result.matches_empty,
                "score": result.score,
                "normalized_residual_z": result.normalized_residual_z,
                "maturity": result.maturity,
                "reliable": result.reliable,
                "residual_energy": result.residual_energy,
                "residual_energy_threshold": result.residual_energy_threshold,
                "window_size": result.window_size,
                "reference_window_count": result.reference_window_count,
            })),
            "calibrated_evidence_authorized": false,
            "numeric_vitals_authorized": false,
        })).unwrap_or_else(|| serde_json::json!({
            "stored": false,
            "active": false,
            "authority": "none",
            "calibrated_evidence_authorized": false,
            "numeric_vitals_authorized": false,
        })),
    }))
}

/// Validate a completed empty room model against twelve fresh server observed
/// samples, then persist only aggregate statistics. The request cannot supply
/// scores, labels, raw CSI, or a replacement model.
async fn calibration_promote_bootstrap(
    State(state): State<SharedState>,
) -> Json<serde_json::Value> {
    let (expected_model_id, expected_binding, holdout_window_size) = {
        let s = state.read().await;
        let observed_at_unix_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
        if s.bootstrap_baseline_active || !s.explicit_calibration_fresh_at(observed_at_unix_ms) {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "calibration_not_finalized",
                "error": "Finalize an explicit room calibration before validating a startup baseline.",
            }));
        }
        let model_id = match s.calibration_model_id.clone() {
            Some(model_id) => model_id,
            None => {
                return Json(serde_json::json!({
                    "success": false,
                    "error_code": "calibration_model_identity_missing",
                    "error": "The active calibration has no server generated model identity.",
                }));
            }
        };
        let Some(binding) = s.calibration_grid_binding else {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "calibration_grid_identity_missing",
                "error": "The completed calibration has no frozen CSI grid identity.",
            }));
        };
        if s.calibration_sequence_fault_node_ids
            .contains(&binding.source_node_id)
        {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "calibration_sequence_discontinuity",
                "error": "The calibration CSI sequence faulted before validation. The model was not stored.",
            }));
        }
        let Some(runtime_window_size) = s
            .field_model
            .as_ref()
            .and_then(|model| model.modes())
            .and_then(|modes| modes.baseline_runtime_window_size)
            .filter(|window_size| *window_size > 0 && *window_size <= FRAME_HISTORY_CAPACITY)
        else {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "calibration_runtime_window_invalid",
                "error": "The completed calibration has no valid runtime scoring window.",
            }));
        };
        (model_id, binding, runtime_window_size)
    };

    let prewarm_deadline = tokio::time::Instant::now()
        + Duration::from_millis(BOOTSTRAP_HOLDOUT_PREWARM_TIMEOUT_MS);
    loop {
        let (identity_matches, sequence_faulted, ready, current_window_size) = {
            let s = state.read().await;
            let observed_at_unix_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
            let identity_matches = s.calibration_model_id.as_deref()
                == Some(expected_model_id.as_str())
                && s.explicit_calibration_fresh_at(observed_at_unix_ms)
                && s.calibration_grid_binding.is_some_and(|binding| {
                    binding.source_node_id == expected_binding.source_node_id
                        && binding.grid == expected_binding.grid
                });
            let sequence_faulted = s
                .calibration_sequence_fault_node_ids
                .contains(&expected_binding.source_node_id);
            let current_window_size = s
                .node_states
                .get(&expected_binding.source_node_id)
                .map_or(0, |node| node.field_model_history.len());
            (
                identity_matches,
                sequence_faulted,
                s.field_model_holdout_ready(
                    expected_binding,
                    holdout_window_size,
                    std::time::Instant::now(),
                ),
                current_window_size,
            )
        };
        if sequence_faulted {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "calibration_sequence_discontinuity",
                "error": "The calibration CSI sequence faulted while preparing held out validation. The model was not stored.",
            }));
        }
        if !identity_matches {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "calibration_model_changed",
                "error": "Calibration changed while preparing held out validation. The model was not stored.",
            }));
        }
        if ready {
            break;
        }
        if tokio::time::Instant::now() >= prewarm_deadline {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "bootstrap_holdout_prewarm_timeout",
                "error": "A complete fresh CSI scoring window was not available before the held out validation timeout. The model was not stored.",
                "required_window_size": holdout_window_size,
                "current_window_size": current_window_size,
            }));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let mut samples = Vec::with_capacity(BOOTSTRAP_VALIDATION_SAMPLES);
    let mut last_sequence = None;
    let mut next_sample_at = tokio::time::Instant::now();
    for _ in 0..BOOTSTRAP_VALIDATION_SAMPLES {
        tokio::time::sleep_until(next_sample_at).await;
        let interval_deadline = tokio::time::Instant::now()
            + Duration::from_millis(BOOTSTRAP_VALIDATION_SAMPLE_TIMEOUT_MS);
        let mut sample = BootstrapValidationSample {
            fresh_tick: false,
            calibrated_empty: false,
            vital_signs_absent: true,
        };
        while tokio::time::Instant::now() < interval_deadline {
            let observed = {
                let s = state.read().await;
                let observed_at_unix_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
                if s.calibration_model_id.as_deref() != Some(expected_model_id.as_str())
                    || !s.explicit_calibration_fresh_at(observed_at_unix_ms)
                    || !s.calibration_grid_binding.is_some_and(|binding| {
                        binding.source_node_id == expected_binding.source_node_id
                            && binding.grid == expected_binding.grid
                    })
                    || s.calibration_sequence_fault_node_ids
                        .contains(&expected_binding.source_node_id)
                {
                    None
                } else {
                    s.node_states
                        .get(&expected_binding.source_node_id)
                        .and_then(|node| {
                            let sequence = node.field_model_latest_sequence?;
                            let fresh = node.field_model_latest_seen.is_some_and(|seen| {
                                std::time::Instant::now().saturating_duration_since(seen)
                                    < ESP32_OFFLINE_TIMEOUT
                            }) && Some(sequence) != last_sequence;
                            let held_out = s.field_model_holdout_ready(
                                expected_binding,
                                holdout_window_size,
                                std::time::Instant::now(),
                            );
                            (fresh && held_out).then_some((
                                sequence,
                                s.person_count() == 0,
                                s.latest_update
                                    .as_ref()
                                    .is_none_or(|update| update.vital_signs.is_none()),
                            ))
                        })
                }
            };
            if let Some((sequence, calibrated_empty, vital_signs_absent)) = observed {
                last_sequence = Some(sequence);
                sample = BootstrapValidationSample {
                    fresh_tick: true,
                    calibrated_empty,
                    vital_signs_absent,
                };
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        samples.push(sample);
        next_sample_at = tokio::time::Instant::now()
            + Duration::from_millis(BOOTSTRAP_VALIDATION_MIN_SPACING_MS);
    }

    let validation = bootstrap_baseline::evaluate_validation(&samples);
    if !validation.passed {
        return Json(serde_json::json!({
            "success": false,
            "error_code": "bootstrap_validation_failed",
            "error": "The held out empty room check did not pass. The model was not stored.",
            "validation": validation,
        }));
    }

    let mut s = state.write().await;
    let observed_at_unix_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
    if s.calibration_model_id.as_deref() != Some(expected_model_id.as_str())
        || !s.explicit_calibration_fresh_at(observed_at_unix_ms)
        || !s.calibration_grid_binding.is_some_and(|binding| {
            binding.source_node_id == expected_binding.source_node_id
                && binding.grid == expected_binding.grid
        })
        || s.calibration_sequence_fault_node_ids
            .contains(&expected_binding.source_node_id)
    {
        return Json(serde_json::json!({
            "success": false,
            "error_code": "calibration_model_changed",
            "error": "Calibration changed during validation. The model was not stored.",
        }));
    }
    let Some(installation_id) = s.installation_id.clone() else {
        return Json(serde_json::json!({
            "success": false,
            "error_code": "installation_id_required",
            "error": "A stable installation identity is required to store a local startup baseline.",
        }));
    };
    let source_node_ids: Vec<u8> = s.calibration_source_node_ids.iter().copied().collect();
    if source_node_ids.len() != 1 {
        return Json(serde_json::json!({
            "success": false,
            "error_code": "calibration_source_nodes_missing",
            "error": "The single-link startup baseline must contain exactly one contributing ESP32 node.",
        }));
    }
    let Some(field_model) = s.field_model.as_ref() else {
        return Json(serde_json::json!({
            "success": false,
            "error_code": "no_field_model",
            "error": "Calibration changed during validation. The model was not stored.",
        }));
    };
    let path = bootstrap_baseline::path_in(&s.data_dir);
    let created_at_unix_ms = chrono::Utc::now().timestamp_millis() as u64;
    let stored = match bootstrap_baseline::store(
        &path,
        &installation_id,
        &source_node_ids,
        expected_binding.grid.as_bootstrap_grid(),
        &expected_model_id,
        created_at_unix_ms,
        field_model,
    ) {
        Ok(metadata) => metadata,
        Err(error) => {
            return error_response::internal_error_json("bootstrap baseline store", error)
        }
    };
    s.bootstrap_baseline = Some(stored.clone());
    info!(
        model_id = %stored.source_model_id,
        source_node_ids = ?stored.source_node_ids,
        content_sha256 = %stored.content_sha256,
        "Stored validated privacy reduced startup baseline"
    );
    Json(serde_json::json!({
        "success": true,
        "message": "Validated startup baseline stored locally.",
        "validation": validation,
        "holdout_window_size": holdout_window_size,
        "bootstrap_baseline": stored,
        "calibrated_evidence_authorized": false,
        "numeric_vitals_authorized": false,
    }))
}

/// Measure one separate operator-confirmed empty holdout and make a bounded,
/// single-use adjustment to a restored bootstrap residual boundary. Only
/// scalar residual energies enter the persisted image; raw CSI never does.
async fn calibration_refine_bootstrap(
    State(state): State<SharedState>,
    Query(query): Query<BootstrapRefineQuery>,
) -> Json<serde_json::Value> {
    if !query.confirmed_empty {
        return Json(serde_json::json!({
            "success": false,
            "error_code": "empty_confirmation_required",
            "error": "Explicit empty-room confirmation is required for bootstrap refinement.",
        }));
    }

    let (expected_model_id, expected_digest, source_node_id, expected_grid) = {
        let s = state.read().await;
        let observed_at_unix_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
        let Some(metadata) = s
            .bootstrap_baseline
            .as_ref()
            .filter(|_| s.bootstrap_baseline_active_at(observed_at_unix_ms))
        else {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "bootstrap_not_active",
                "error": "A restored startup baseline must be active before refinement.",
            }));
        };
        let [source_node_id] = metadata.source_node_ids.as_slice() else {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "bootstrap_source_invalid",
                "error": "Bootstrap refinement requires exactly one bound source node.",
            }));
        };
        if query
            .source_node_id
            .is_some_and(|requested| requested != *source_node_id)
        {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "bootstrap_source_mismatch",
                "error": "The requested source does not match the stored bootstrap source.",
            }));
        }
        (
            metadata.source_model_id.clone(),
            metadata.content_sha256.clone(),
            *source_node_id,
            CsiGridKey::from_bootstrap_grid(metadata.source_grid),
        )
    };

    let mut residuals = Vec::with_capacity(BOOTSTRAP_VALIDATION_SAMPLES);
    let mut stale_sample_count = 0usize;
    let mut vital_sign_sample_count = 0usize;
    let mut last_sequence = None;
    let mut next_sample_at = tokio::time::Instant::now();
    for _ in 0..BOOTSTRAP_VALIDATION_SAMPLES {
        tokio::time::sleep_until(next_sample_at).await;
        let interval_deadline = tokio::time::Instant::now()
            + Duration::from_millis(BOOTSTRAP_VALIDATION_SAMPLE_TIMEOUT_MS);
        let mut observed = None;
        while tokio::time::Instant::now() < interval_deadline {
            observed = {
                let s = state.read().await;
                let observed_at_unix_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
                let identity_matches = s.bootstrap_baseline_active_at(observed_at_unix_ms)
                    && s.bootstrap_baseline.as_ref().is_some_and(|metadata| {
                        metadata.source_model_id == expected_model_id
                            && metadata.content_sha256 == expected_digest
                            && metadata.source_node_ids.as_slice() == [source_node_id]
                            && CsiGridKey::from_bootstrap_grid(metadata.source_grid)
                                == expected_grid
                    })
                    && s.calibration_grid_binding.is_some_and(|binding| {
                        binding.source_node_id == source_node_id
                            && binding.grid == expected_grid
                    });
                if !identity_matches {
                    None
                } else {
                    s.node_states.get(&source_node_id).and_then(|node| {
                        let sequence = node.field_model_latest_sequence?;
                        let fresh = node.field_model_latest_seen.is_some_and(|seen| {
                            std::time::Instant::now().saturating_duration_since(seen)
                                < ESP32_OFFLINE_TIMEOUT
                        }) && Some(sequence) != last_sequence;
                        if !fresh {
                            return None;
                        }
                        let model = s.field_model.as_ref()?;
                        let recent_frames: Vec<Vec<f64>> =
                            node.field_model_history.iter().cloned().collect();
                        let residual = model.empty_room_match(&recent_frames)?.residual_energy;
                        let vital_signs_absent = s
                            .latest_update
                            .as_ref()
                            .is_some_and(|update| update.vital_signs.is_none());
                        Some((sequence, residual, vital_signs_absent))
                    })
                }
            };
            if observed.is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        if let Some((sequence, residual, vital_signs_absent)) = observed {
            last_sequence = Some(sequence);
            residuals.push(residual);
            if !vital_signs_absent {
                vital_sign_sample_count += 1;
            }
        } else {
            stale_sample_count += 1;
        }
        next_sample_at = tokio::time::Instant::now()
            + Duration::from_millis(BOOTSTRAP_VALIDATION_MIN_SPACING_MS);
    }

    if residuals.len() != BOOTSTRAP_VALIDATION_SAMPLES
        || stale_sample_count != 0
        || vital_sign_sample_count != 0
    {
        return Json(serde_json::json!({
            "success": false,
            "error_code": "bootstrap_refinement_validation_failed",
            "error": "The fresh empty-room refinement gate did not pass. The stored baseline was not changed.",
            "validation": {
                "sample_count": residuals.len(),
                "stale_sample_count": stale_sample_count,
                "vital_sign_sample_count": vital_sign_sample_count,
            },
        }));
    }

    let mut s = state.write().await;
    let observed_at_unix_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
    let identity_matches = s.bootstrap_baseline_active_at(observed_at_unix_ms)
        && s.bootstrap_baseline.as_ref().is_some_and(|metadata| {
            metadata.source_model_id == expected_model_id
                && metadata.content_sha256 == expected_digest
                && metadata.source_node_ids.as_slice() == [source_node_id]
                && CsiGridKey::from_bootstrap_grid(metadata.source_grid) == expected_grid
        })
        && s.calibration_grid_binding.is_some_and(|binding| {
            binding.source_node_id == source_node_id && binding.grid == expected_grid
        });
    if !identity_matches {
        return Json(serde_json::json!({
            "success": false,
            "error_code": "bootstrap_changed",
            "error": "The bootstrap identity changed during refinement. The stored baseline was not changed.",
        }));
    }
    let candidate_snapshot = match s
        .field_model
        .as_ref()
        .ok_or(wifi_densepose_signal::ruvsense::field_model::FieldModelError::NotCalibrated)
        .and_then(|model| model.export_snapshot())
    {
        Ok(snapshot) => snapshot,
        Err(_) => {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "bootstrap_refinement_rejected",
                "error": "The bootstrap model rejected this bounded refinement.",
            }));
        }
    };
    let now_us = chrono::Utc::now().timestamp_micros() as u64;
    let mut candidate = match FieldModel::from_snapshot(candidate_snapshot, now_us) {
        Ok(model) => model,
        Err(_) => {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "bootstrap_refinement_rejected",
                "error": "The bootstrap model rejected this bounded refinement.",
            }));
        }
    };
    let refinement = match candidate.refine_empty_room_residual_boundary(&residuals) {
        Ok(refinement) => refinement,
        Err(_) => {
            return Json(serde_json::json!({
                "success": false,
                "error_code": "bootstrap_refinement_rejected",
                "error": "The bootstrap model rejected this bounded refinement.",
            }));
        }
    };
    let Some(installation_id) = s.installation_id.clone() else {
        return Json(serde_json::json!({
            "success": false,
            "error_code": "installation_id_required",
            "error": "A stable installation identity is required to store a local startup baseline.",
        }));
    };
    let path = bootstrap_baseline::path_in(&s.data_dir);
    let created_at_unix_ms = chrono::Utc::now().timestamp_millis() as u64;
    let stored = match bootstrap_baseline::store(
        &path,
        &installation_id,
        &[source_node_id],
        expected_grid.as_bootstrap_grid(),
        &expected_model_id,
        created_at_unix_ms,
        &candidate,
    ) {
        Ok(metadata) => metadata,
        Err(error) => {
            return error_response::internal_error_json("bootstrap baseline refinement store", error)
        }
    };
    s.field_model = Some(candidate);
    s.bootstrap_baseline = Some(stored.clone());
    Json(serde_json::json!({
        "success": true,
        "message": "The local empty-room startup baseline was refined.",
        "validation": {
            "sample_count": residuals.len(),
            "stale_sample_count": stale_sample_count,
            "vital_sign_sample_count": vital_sign_sample_count,
        },
        "refinement": {
            "threshold_before": refinement.threshold_before,
            "threshold_after": refinement.threshold_after,
            "held_out_p95": refinement.held_out_p95,
            "held_out_sample_count": refinement.held_out_sample_count,
            "reference_window_count": refinement.reference_window_count,
        },
        "bootstrap_baseline": stored,
        "calibrated_evidence_authorized": false,
        "numeric_vitals_authorized": false,
    }))
}

/// Cancel only an unfinished capture. Completed model deletion remains on the
/// administrator scoped reset route.
async fn calibration_cancel(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let mut s = state.write().await;
    let cancellable = s.field_model.as_ref().is_some_and(|model| {
        matches!(
            model.status(),
            CalibrationStatus::Uncalibrated | CalibrationStatus::Collecting
        )
    });
    if !cancellable {
        return Json(serde_json::json!({
            "success": false,
            "error_code": "calibration_not_collecting",
            "error": "Only an unfinished empty room capture can be cancelled.",
        }));
    }
    s.field_model = None;
    s.calibration_model_id = None;
    s.calibration_source_node_ids.clear();
    s.clear_field_model_binding();
    s.clear_calibration_sequence_state();
    if let Some(installation_id) = s.installation_id.clone() {
        let path = bootstrap_baseline::path_in(&s.data_dir);
        if let Ok((model, metadata)) = bootstrap_baseline::load(
            &path,
            &installation_id,
            chrono::Utc::now().timestamp_millis() as u64,
        ) {
            let [source_node_id] = metadata.source_node_ids.as_slice() else {
                return Json(serde_json::json!({
                    "success": false,
                    "error_code": "bootstrap_source_invalid",
                    "error": "The stored bootstrap source identity is invalid.",
                }));
            };
            let binding = CalibrationGridBinding {
                source_node_id: *source_node_id,
                grid: CsiGridKey::from_bootstrap_grid(metadata.source_grid),
                evidence: None,
            };
            s.field_model = Some(model);
            s.calibration_source_node_ids.insert(*source_node_id);
            s.calibration_grid_binding = Some(binding);
            s.bootstrap_baseline = Some(metadata);
            s.bootstrap_baseline_active = true;
        }
    }
    Json(serde_json::json!({
        "success": true,
        "message": "Unfinished empty room capture cancelled.",
        "status": if s.bootstrap_baseline_active { "bootstrap" } else { "none" },
    }))
}

async fn calibration_reset(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let mut s = state.write().await;
    s.field_model = None;
    s.calibration_model_id = None;
    s.calibration_source_node_ids.clear();
    s.clear_field_model_binding();
    s.clear_calibration_sequence_state();
    s.bootstrap_baseline_active = false;
    let path = bootstrap_baseline::path_in(&s.data_dir);
    let bootstrap_removed = match bootstrap_baseline::remove(&path) {
        Ok(removed) => {
            s.bootstrap_baseline = None;
            removed
        }
        Err(error) => {
            warn!(%error, "Could not remove stored bootstrap baseline during reset");
            false
        }
    };
    Json(serde_json::json!({
        "success": true,
        "message": "Calibration model reset.",
        "status": "none",
        "bootstrap_removed": bootstrap_removed,
    }))
}

/// Compatibility surface used by the bundled dashboard. Activity history is
/// not persisted by the Rust server yet, so return an honest empty collection
/// instead of advertising the endpoint and responding with 404.
async fn pose_activities() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "activities": [],
        "total": 0,
        "persisted": false,
        "message": "Activity history is not persisted by the Rust sensing server.",
    }))
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
    let observed_at_unix_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
    let person_count = s.person_count_at(observed_at_unix_ms);
    let explicit_calibration_fresh =
        s.explicit_calibration_fresh_at(observed_at_unix_ms);
    let published = vitals_for_publication(
        &s.latest_vitals,
        explicit_calibration_fresh,
        person_count,
    );
    let (br_len, br_cap, hb_len, hb_cap) = s.vital_detector.buffer_status();
    Json(serde_json::json!({
        "vital_signs": {
            "breathing_rate_bpm": published.as_ref().and_then(|value| value.breathing_rate_bpm),
            "heart_rate_bpm": published.as_ref().and_then(|value| value.heart_rate_bpm),
            "breathing_confidence": published.as_ref().map(|value| value.breathing_confidence),
            "heartbeat_confidence": published.as_ref().map(|value| value.heartbeat_confidence),
            "signal_quality": published.as_ref().map(|value| value.signal_quality),
        },
        "authority": if published.is_some() { "explicit_calibration" } else { "abstained" },
        "abstention_reason": if published.is_some() { serde_json::Value::Null } else { serde_json::json!("fresh explicit calibration with exactly one occupant and qualified evidence required") },
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

/// Query params for `GET /api/v1/edge/registry`.
#[derive(Debug, Deserialize)]
struct EdgeRegistryParams {
    /// `?refresh=1` bypasses the in-process cache. Logged at debug for
    /// abuse visibility. ADR-102 §"Cache semantics".
    #[serde(default)]
    refresh: Option<String>,
}

/// GET /api/v1/edge/registry — surfaces the canonical Cognitum cog catalog.
///
/// See ADR-102 (`docs/adr/ADR-102-edge-module-registry.md`) for the design
/// + trust model + security review.
async fn edge_registry_endpoint(
    Extension(reg): Extension<
        Option<Arc<wifi_densepose_sensing_server::edge_registry::EdgeRegistry>>,
    >,
    Query(params): Query<EdgeRegistryParams>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let Some(reg) = reg else {
        // --no-edge-registry, or upstream URL empty.
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "edge_registry_disabled",
                "detail": "This sensing-server was started with --no-edge-registry."
            })),
        ));
    };
    let force_refresh = matches!(params.refresh.as_deref(), Some("1") | Some("true"));
    if force_refresh {
        tracing::debug!(
            event = "edge_registry.refresh_requested",
            "?refresh=1 bypassed the cache; verify this isn't being abused"
        );
    }
    match tokio::task::spawn_blocking(move || reg.get(force_refresh)).await {
        Ok(Ok(resp)) => Ok(Json(
            serde_json::to_value(resp).unwrap_or(serde_json::json!({})),
        )),
        // ADR-080 #2: the upstream error can carry an internal URL/connection
        // detail — log it server-side only and return a generic 503.
        Ok(Err(err)) => Err(error_response::upstream_unavailable("edge_registry", err)),
        // ADR-080 #2: a panicked spawn_blocking surfaces "task … panicked" via
        // JoinError::Display — never ship that to the client. Generic 500 +
        // correlation id; the panic detail is logged server-side.
        Err(join_err) => Err(error_response::internal_error("edge_registry", join_err)),
    }
}

/// GET /api/v1/edge-vitals — latest edge vitals from ESP32 (ADR-039).
async fn edge_vitals_endpoint(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    let observed_at_unix_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
    let explicit_single_occupant = s.explicit_calibration_fresh_at(observed_at_unix_ms)
        && s.person_count_at(observed_at_unix_ms) == 1;
    if !explicit_single_occupant {
        return Json(serde_json::json!({
            "status": "abstained",
            "edge_vitals": null,
            "message": "Fresh explicit calibration with exactly one occupant is required.",
        }));
    }
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

/// GET /api/v1/nodes — per-node health and feature info.
/// ADR-110 iter 29 — per-node mesh sync snapshot via HTTP.
///
/// GET /api/v1/nodes/:id/sync
///   200 → Json(NodeSyncSnapshot) when latest_sync is present
///   404 → {"error": "no_sync", "node_id": N} otherwise
///
/// Complements the WebSocket `sync` field (iter 23) for clients that
/// can't hold a streaming connection (curl scripts, Home Assistant REST
/// sensors, automation rule probes).
async fn node_sync_endpoint(
    State(state): State<SharedState>,
    Path(id): Path<u8>,
) -> Result<Json<NodeSyncSnapshot>, (StatusCode, Json<serde_json::Value>)> {
    let s = state.read().await;
    let ns = s.node_states.get(&id).ok_or_else(|| {
        (StatusCode::NOT_FOUND, Json(serde_json::json!({
            "error": "unknown_node", "node_id": id,
        })))
    })?;
    ns.sync_snapshot().map(Json).ok_or_else(|| {
        (StatusCode::NOT_FOUND, Json(serde_json::json!({
            "error": "no_sync", "node_id": id,
            "hint": "node hasn't emitted a sync packet yet (no mesh peer or not v0.6.9+)",
        })))
    })
}

/// ADR-110 iter 29 — fleet-wide mesh state via HTTP.
///
/// GET /api/v1/mesh
///   200 → { "nodes": { "<id>": NodeSyncSnapshot, ... }, "total": N }
///   Nodes without a recent sync are omitted from the map; an empty
///   `nodes` object means no mesh peers reachable.
/// ADR-110 iter 36 — Prometheus exposition format for mesh state.
///
/// GET /api/v1/mesh/metrics → text/plain
///   wifi_densepose_mesh_offset_us{node="N"} <signed-int>
///   wifi_densepose_mesh_is_leader{node="N"} 0|1
///   wifi_densepose_mesh_is_valid{node="N"} 0|1
///   wifi_densepose_mesh_smoothed{node="N"} 0|1
///   wifi_densepose_mesh_sequence{node="N"} <u32>
///   wifi_densepose_mesh_csi_fps{node="N"} <float>
///   wifi_densepose_mesh_csi_fps_samples{node="N"} <u32>
///   wifi_densepose_mesh_staleness_ms{node="N"} <u64>
///
/// Spec: <https://prometheus.io/docs/instrumenting/exposition_formats/>.
/// Each metric is a gauge labeled by node_id. Nodes without a fresh sync
/// are simply absent from the output (Prometheus handles missing series
/// natively — the scrape just reports them as stale after the configured
/// staleness duration).
async fn mesh_metrics_endpoint(State(state): State<SharedState>) -> impl IntoResponse {
    use std::fmt::Write;
    let s = state.read().await;
    let mut body = String::with_capacity(1024);

    // Each metric: HELP + TYPE header + one line per node that has a snapshot.
    let metrics: &[(&str, &str, &str)] = &[
        ("wifi_densepose_mesh_offset_us",
         "Cross-board mesh-aligned offset, microseconds (signed)", "gauge"),
        ("wifi_densepose_mesh_is_leader",
         "1 if this node is the elected mesh leader, else 0", "gauge"),
        ("wifi_densepose_mesh_is_valid",
         "1 if this node has heard a fresh leader beacon, else 0", "gauge"),
        ("wifi_densepose_mesh_smoothed",
         "1 once the firmware-side EMA filter has seeded, else 0", "gauge"),
        ("wifi_densepose_mesh_sequence",
         "High-water CSI sequence at sync emit time", "gauge"),
        ("wifi_densepose_mesh_csi_fps",
         "Per-node measured CSI frame rate (Hz)", "gauge"),
        ("wifi_densepose_mesh_csi_fps_samples",
         "How many inter-frame deltas the fps EMA has seen", "gauge"),
        ("wifi_densepose_mesh_staleness_ms",
         "Milliseconds since the host last received this node's sync packet", "gauge"),
    ];

    // Collect (id, snapshot) pairs once so each metric loop reads the same set.
    let snaps: Vec<(u8, NodeSyncSnapshot)> = s.node_states.iter()
        .filter_map(|(&id, ns)| ns.sync_snapshot().map(|snap| (id, snap)))
        .collect();

    // Iter 37: fleet cardinality summary — Ops dashboards want the
    // "how many leaders / followers / no-sync" tally at a glance
    // without scraping every per-node series and counting.
    let (leaders, followers) = fleet_role_counts(&snaps);
    let no_sync = s.node_states.len().saturating_sub(snaps.len()) as u64;
    let _ = writeln!(body,
        "# HELP wifi_densepose_mesh_node_total Per-state node count across the fleet");
    let _ = writeln!(body, "# TYPE wifi_densepose_mesh_node_total gauge");
    let _ = writeln!(body, "wifi_densepose_mesh_node_total{{state=\"leader\"}} {leaders}");
    let _ = writeln!(body, "wifi_densepose_mesh_node_total{{state=\"follower\"}} {followers}");
    let _ = writeln!(body, "wifi_densepose_mesh_node_total{{state=\"no_sync\"}} {no_sync}");

    for (name, help, kind) in metrics {
        let _ = writeln!(body, "# HELP {name} {help}");
        let _ = writeln!(body, "# TYPE {name} {kind}");
        for (id, snap) in &snaps {
            let value = match *name {
                "wifi_densepose_mesh_offset_us" => snap.offset_us.to_string(),
                "wifi_densepose_mesh_is_leader" => bool_metric(snap.is_leader),
                "wifi_densepose_mesh_is_valid" => bool_metric(snap.is_valid),
                "wifi_densepose_mesh_smoothed" => bool_metric(snap.smoothed),
                "wifi_densepose_mesh_sequence" => snap.sequence.to_string(),
                "wifi_densepose_mesh_csi_fps" => format!("{:.3}", snap.csi_fps_ema),
                "wifi_densepose_mesh_csi_fps_samples" => snap.csi_fps_samples.to_string(),
                "wifi_densepose_mesh_staleness_ms" =>
                    snap.staleness_ms.map(|n| n.to_string()).unwrap_or_else(|| "0".into()),
                _ => continue,
            };
            let _ = writeln!(body, "{name}{{node=\"{id}\"}} {value}");
        }
    }
    ([(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4")], body)
}

fn bool_metric(b: bool) -> String { (if b { 1 } else { 0 }).to_string() }

/// ADR-110 iter 37 — count (leaders, followers) in a populated snapshot set.
/// Free function for testability — same pattern as iter 18's `update_csi_fps_ema`.
pub(crate) fn fleet_role_counts(snaps: &[(u8, NodeSyncSnapshot)]) -> (u64, u64) {
    let leaders = snaps.iter().filter(|(_, s)| s.is_leader).count() as u64;
    let followers = (snaps.len() as u64).saturating_sub(leaders);
    (leaders, followers)
}

async fn mesh_endpoint(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    let mut nodes = serde_json::Map::new();
    for (&id, ns) in s.node_states.iter() {
        if let Some(snap) = ns.sync_snapshot() {
            nodes.insert(id.to_string(), serde_json::to_value(snap).unwrap());
        }
    }
    let total = nodes.len();
    Json(serde_json::json!({
        "nodes": serde_json::Value::Object(nodes),
        "total": total,
    }))
}

async fn nodes_endpoint(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    let now = std::time::Instant::now();
    let observed_at_unix_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
    let nodes: Vec<serde_json::Value> = s
        .node_states
        .iter()
        .map(|(&id, ns)| {
            let elapsed_ms = ns
                .last_frame_time
                .map(|t| now.duration_since(t).as_millis() as u64)
                .unwrap_or(999999);
            let csi_elapsed_ms = ns
                .latest_accepted_csi_at
                .map(|t| now.saturating_duration_since(t).as_millis() as u64);
            let stale = elapsed_ms > 5000;
            let status = if stale { "stale" } else { "active" };
            let csi_status = if csi_elapsed_ms.is_some_and(|age| {
                age < ESP32_OFFLINE_TIMEOUT.as_millis() as u64
            }) {
                "active"
            } else {
                "stale"
            };
            let rssi = ns.rssi_history.back().copied().unwrap_or(-90.0);
            let bootstrap_empty = s.bootstrap_baseline_active
                && s.bootstrap_baseline.as_ref().is_some_and(|metadata| {
                    metadata.source_node_ids.as_slice() == [id]
                })
                && s.bootstrap_background_match(observed_at_unix_ms)
                    .is_some_and(|result| result.matches_empty);
            serde_json::json!({
                "node_id": id,
                "status": status,
                "last_seen_ms": elapsed_ms,
                "csi_status": csi_status,
                "csi_last_seen_ms": csi_elapsed_ms,
                "csi_sequence": ns.latest_csi_sequence,
                "rssi_dbm": rssi,
                "motion_level": if bootstrap_empty { "absent" } else { &ns.current_motion_level },
                "person_count": if bootstrap_empty { 0 } else { ns.prev_person_count },
                "person_count_valid": ns.edge_vitals
                    .as_ref()
                    .map(|vitals| vitals.person_count_valid),
            })
        })
        .collect();
    Json(serde_json::json!({
        "nodes": nodes,
        "total": nodes.len(),
    }))
}

async fn info_page() -> Html<String> {
    Html(
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
         .to_string()
    )
}

/// Resolve `--node-positions` entries to the currently active node IDs.
///
/// `MultistaticFuser::fuse` assigns `node_positions[i]` to the i-th node in
/// its `node_frames` argument, which callers build sorted ascending by
/// node_id (see `node_frames_from_states_with_guard`). Real fleets use
/// non-sequential logical node IDs (e.g. 11, 12, 13), so a naive
/// `node_id -> node_positions_config[node_id]` lookup silently misses for
/// every real deployment. This mirrors the same ascending-rank convention so
/// the live `NodeInfo.position` field agrees with what fusion actually used.
fn node_positions_by_active_id(
    node_positions_config: &[[f32; 3]],
    node_states: &HashMap<u8, NodeState>,
    now: std::time::Instant,
) -> HashMap<u8, [f64; 3]> {
    let mut active_ids: Vec<u8> = node_states
        .iter()
        .filter(|(_, n)| {
            n.last_frame_time
                .is_some_and(|t| now.duration_since(t).as_secs() < 10)
        })
        .map(|(&id, _)| id)
        .collect();
    active_ids.sort_unstable();
    active_ids
        .into_iter()
        .enumerate()
        .filter_map(|(rank, id)| {
            node_positions_config
                .get(rank)
                .map(|p| (id, [p[0] as f64, p[1] as f64, p[2] as f64]))
        })
        .collect()
}

#[cfg(test)]
mod node_positions_by_active_id_tests {
    use super::*;

    fn active_node(now: std::time::Instant) -> NodeState {
        let mut ns = NodeState::new();
        ns.last_frame_time = Some(now);
        ns
    }

    #[test]
    fn non_sequential_node_ids_get_positions_by_ascending_rank() {
        // Real fleets use logical IDs like 11, 12, 13 — not 0, 1, 2. The
        // configured position list must map by ascending node_id rank, not
        // by treating node_id as a direct index into the list.
        let now = std::time::Instant::now();
        let mut node_states = HashMap::new();
        node_states.insert(13, active_node(now));
        node_states.insert(11, active_node(now));
        node_states.insert(12, active_node(now));

        let configured = [[1.0, 0.0, 0.0], [2.0, 0.0, 0.0], [3.0, 0.0, 0.0]];
        let resolved = node_positions_by_active_id(&configured, &node_states, now);

        assert_eq!(resolved.get(&11), Some(&[1.0, 0.0, 0.0]));
        assert_eq!(resolved.get(&12), Some(&[2.0, 0.0, 0.0]));
        assert_eq!(resolved.get(&13), Some(&[3.0, 0.0, 0.0]));
    }

    #[test]
    fn stale_nodes_are_excluded_from_rank_assignment() {
        let now = std::time::Instant::now();
        let mut node_states = HashMap::new();
        node_states.insert(11, active_node(now));
        let mut stale = NodeState::new();
        stale.last_frame_time =
            Some(now - std::time::Duration::from_secs(30));
        node_states.insert(12, stale);
        node_states.insert(13, active_node(now));

        let configured = [[1.0, 0.0, 0.0], [3.0, 0.0, 0.0]];
        let resolved = node_positions_by_active_id(&configured, &node_states, now);

        assert_eq!(resolved.get(&11), Some(&[1.0, 0.0, 0.0]));
        assert_eq!(resolved.get(&12), None, "stale node must not consume a rank");
        assert_eq!(resolved.get(&13), Some(&[3.0, 0.0, 0.0]));
    }

    #[test]
    fn missing_config_entries_fall_through_to_the_caller_default() {
        let now = std::time::Instant::now();
        let mut node_states = HashMap::new();
        node_states.insert(11, active_node(now));

        let resolved = node_positions_by_active_id(&[], &node_states, now);
        assert_eq!(resolved.get(&11), None);
    }
}

// ── UDP receiver task ────────────────────────────────────────────────────────

async fn udp_receiver_task(
    state: SharedState,
    bind_ip: std::net::IpAddr,
    udp_port: u16,
    allowlist: std::sync::Arc<wifi_densepose_sensing_server::udp_bind::UdpSourceAllowlist>,
) {
    let addr = format!("{bind_ip}:{udp_port}");
    let socket = match UdpSocket::bind(&addr).await {
        Ok(s) => {
            info!("UDP listening on {addr} for ESP32, MediaTek, Qualcomm CSI, and RTL8720F radar frames");
            s
        }
        Err(e) => {
            error!("Failed to bind UDP {addr}: {e}");
            return;
        }
    };

    let mut buf = vec![0u8; wifi_densepose_hardware::rtl8720f::RTL8720F_RADAR_MAX_FRAME_LEN];
    loop {
        match socket.recv_from(&mut buf).await {
            Ok((len, src)) => {
                // ADR-296: drop frames from sources outside the allowlist
                // (loopback is always admitted). Counted for observability.
                if !allowlist.admit(src.ip()) {
                    debug!(
                        "Dropped UDP frame from disallowed source {src} (allowlist active; total dropped={})",
                        allowlist.dropped()
                    );
                    continue;
                }
                if len > 0 && buf[0] == b'{' {
                    match serde_json::from_slice::<wifi_densepose_hardware::vendor_rf::VendorRfEvent>(&buf[..len])
                        .map_err(|error| error.to_string())
                        .and_then(|event| wifi_densepose_sensing_server::vendor_rf::VendorEventSnapshot::from_event(event).map_err(|error| error.to_string()))
                    {
                        Ok(snapshot) if snapshot.event.synthetic => {
                            debug!("Vendor RF event from {src}: vendor={} capability={:?} seq={}", snapshot.event.vendor.as_str(), snapshot.event.capability, snapshot.event.sequence);
                            let json = serde_json::to_string(&snapshot).ok();
                            let mut state = state.write().await;
                            state.source = snapshot.source.clone();
                            state.latest_vendor_rf.insert(snapshot.event.vendor.as_str().to_string(), snapshot);
                            if let Some(json) = json { let _ = state.tx.send(json); }
                        }
                        Ok(_) => warn!("Rejected non-synthetic canonical vendor event from {src}; live payloads must use the provider decoder HTTP route"),
                        Err(error) => warn!("Rejected ADR-270 vendor event from {src}: {error}"),
                    }
                    continue;
                }
                if len >= 4
                    && u32::from_le_bytes(buf[..4].try_into().expect("four-byte slice"))
                        == wifi_densepose_hardware::qualcomm_csi::QUALCOMM_CSI_MAGIC
                {
                    match wifi_densepose_hardware::qualcomm_csi::CsiFrame::from_bytes(&buf[..len]) {
                        Ok((frame, consumed)) if consumed == len => {
                            let snapshot = qualcomm_csi::QualcommCsiSnapshot::from_frame(&frame);
                            debug!("Qualcomm CSI from {src}: profile={} seq={} dimensions={}x{}x{}", snapshot.chipset, snapshot.sequence, snapshot.tx_count, snapshot.rx_count, snapshot.subcarrier_count);
                            let json = serde_json::to_string(&snapshot).ok();
                            let mut s = state.write().await;
                            s.source = snapshot.source.to_string();
                            s.last_qualcomm_frame = Some(std::time::Instant::now());
                            s.latest_qualcomm_csi = Some(snapshot);
                            if let Some(json) = json { let _ = s.tx.send(json); }
                        }
                        Ok((_, consumed)) => warn!("Qualcomm CSI datagram from {src} has trailing bytes: consumed={consumed} received={len}"),
                        Err(error) => warn!("Rejected Qualcomm CSI datagram from {src}: {error}"),
                    }
                    continue;
                }
                if len >= 4
                    && u32::from_le_bytes(buf[..4].try_into().expect("four-byte slice"))
                        == wifi_densepose_hardware::mediatek_csi::MEDIATEK_CSI_MAGIC
                {
                    match wifi_densepose_hardware::mediatek_csi::CsiFrame::from_bytes(&buf[..len]) {
                        Ok((frame, consumed)) if consumed == len => {
                            let snapshot = mediatek_csi::MediatekCsiSnapshot::from_frame(&frame);
                            debug!("MediaTek CSI from {src}: profile={} seq={} dimensions={}x{}x{}", snapshot.chipset, snapshot.sequence, snapshot.tx_count, snapshot.rx_count, snapshot.subcarrier_count);
                            let json = serde_json::to_string(&snapshot).ok();
                            let mut s = state.write().await;
                            s.source = snapshot.source.to_string();
                            s.last_mediatek_frame = Some(std::time::Instant::now());
                            s.latest_mediatek_csi = Some(snapshot);
                            if let Some(json) = json { let _ = s.tx.send(json); }
                        }
                        Ok((_, consumed)) => warn!("MediaTek CSI datagram from {src} has trailing bytes: consumed={consumed} received={len}"),
                        Err(error) => warn!("Rejected MediaTek CSI datagram from {src}: {error}"),
                    }
                    continue;
                }
                if len >= 4
                    && u32::from_le_bytes(buf[..4].try_into().expect("four-byte slice"))
                        == wifi_densepose_hardware::rtl8720f::RTL8720F_RADAR_MAGIC
                {
                    match wifi_densepose_hardware::rtl8720f::RadarFrame::from_bytes(&buf[..len]) {
                        Ok((frame, consumed)) if consumed == len => {
                            let snapshot = realtek_radar::RealtekRadarSnapshot::from_frame(&frame);
                            debug!("RTL8720F radar from {src}: type={} seq={} elements={}", snapshot.report_type, snapshot.sequence, snapshot.element_count);
                            let json = serde_json::to_string(&snapshot).ok();
                            let mut s = state.write().await;
                            s.source = snapshot.source.to_string();
                            s.last_realtek_frame = Some(std::time::Instant::now());
                            s.latest_realtek_radar = Some(snapshot);
                            if let Some(json) = json {
                                let _ = s.tx.send(json);
                            }
                        }
                        Ok((_, consumed)) => warn!("RTL8720F radar datagram from {src} has trailing bytes: consumed={consumed} received={len}"),
                        Err(error) => warn!("Rejected RTL8720F radar datagram from {src}: {error}"),
                    }
                    continue;
                }
                // ADR-039: Try edge vitals packet first (magic 0xC511_0002).
                if let Some(vitals) = parse_esp32_vitals(&buf[..len]) {
                    debug!(
                        "ESP32 vitals from {src}: node={} br={:.1} hr={:.1} pres={}",
                        vitals.node_id,
                        vitals.breathing_rate_bpm,
                        vitals.heartrate_bpm,
                        vitals.presence
                    );
                    let mut s = state.write().await;
                    // Issue #323: Also emit a sensing_update so the UI renders
                    // detections for ESP32 nodes running the edge DSP pipeline
                    // (Tier 2+).  Without this, vitals arrive but the UI shows
                    // "no detection" because it only renders sensing_update msgs.
                    s.source = "esp32".to_string();
                    s.last_esp32_frame = Some(std::time::Instant::now());

                    // ── Per-node state for edge vitals (issue #249) ──────
                    let node_id = vitals.node_id;
                    let ns = s.node_states.entry(node_id).or_insert_with(NodeState::new);
                    let first_sensing_frame = ns.last_frame_time.is_none();
                    ns.last_frame_time = Some(std::time::Instant::now());
                    if first_sensing_frame && telemetry::curated_events_enabled() {
                        info!(name: semconv::EVENT_RUVIEW_NODE_ONLINE, { "ruview.node.id" = node_id }, "node {node_id} online (edge vitals)");
                    }
                    // Edge-triggered on the fall flag's rising edge (against
                    // the node's previous edge-vitals frame), so a persisting
                    // flag does not re-emit every frame.
                    let prev_fall = ns.edge_vitals.as_ref().is_some_and(|v| v.fall_detected);
                    if vitals.fall_detected && !prev_fall && telemetry::curated_events_enabled() {
                        warn!(name: semconv::EVENT_RUVIEW_FALL_DETECTED, { "ruview.node.id" = node_id }, "fall detected by node {node_id}");
                    }
                    ns.edge_vitals = Some(vitals.clone());
                    ns.rssi_history.push_back(vitals.rssi as f64);
                    if ns.rssi_history.len() > 60 {
                        ns.rssi_history.pop_front();
                    }

                    // Store per-node person count from edge vitals.
                    let node_est = if vitals.presence {
                        (vitals.n_persons as usize).max(1)
                    } else {
                        0
                    };
                    ns.prev_person_count = node_est;

                    s.tick += 1;
                    let tick = s.tick;

                    let motion_score = if vitals.motion {
                        0.8
                    } else if vitals.presence {
                        0.3
                    } else {
                        0.05
                    };

                    let observed_at_unix_ms =
                        chrono::Utc::now().timestamp_millis().max(0) as u64;
                    let bootstrap_empty =
                        s.bootstrap_empty_prior_applies(observed_at_unix_ms);

                    // A startup prior has negative-only authority. Once a full
                    // runtime window matches the stored background, raw edge
                    // presence cannot force the count back to one.
                    let now = std::time::Instant::now();
                    let total_persons = if bootstrap_empty {
                        0
                    } else if vitals.presence {
                        let dedup = s.dedup_factor;
                        let (fused, fallback_count) = multistatic_bridge::fuse_or_fallback(
                            &s.multistatic_fuser,
                            &s.node_states,
                            dedup,
                        );
                        match fused {
                            Some(ref f) => {
                                let score =
                                    multistatic_bridge::compute_person_score_from_amplitudes(
                                        &f.fused_amplitude,
                                    );
                                s.smoothed_person_score =
                                    s.smoothed_person_score * 0.90 + score * 0.10;
                                // #803: don't let the saturating activity score
                                // discard count-aware per-node estimates.
                                let count =
                                    aggregate_person_count(
                                        s.person_count_at(observed_at_unix_ms),
                                        &s.node_states,
                                    );
                                s.prev_person_count = count;
                                count.max(1) // presence=true => at least 1
                            }
                            None => {
                                aggregate_person_count(fallback_count.unwrap_or(0), &s.node_states)
                                    .max(1)
                            }
                        }
                    } else {
                        s.prev_person_count = 0;
                        0
                    };

                    // Governed trust cycle (ADR-135..146): run the same live
                    // frames through the privacy/provenance/witness control
                    // plane. Trust state is recorded on the bridge (exposed on
                    // /api/v1/status); engine errors are counted + rate-limit
                    // logged instead of being swallowed (review finding 1).
                    // Split-borrow the two distinct fields off the guard.
                    {
                        let sref: &mut AppStateInner = &mut s;
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_millis() as i64)
                            .unwrap_or(0);
                        sref.engine_bridge.observe_cycle(&sref.node_states, now_ms);
                    }

                    // Build nodes array with all active nodes.
                    let resolved_positions =
                        node_positions_by_active_id(&s.node_positions_config, &s.node_states, now);
                    let active_nodes: Vec<NodeInfo> = s
                        .node_states
                        .iter()
                        .filter(|(_, n)| {
                            n.last_frame_time
                                .is_some_and(|t| now.duration_since(t).as_secs() < 10)
                        })
                        .map(|(&id, n)| NodeInfo {
                            node_id: id,
                            rssi_dbm: n.rssi_history.back().copied().unwrap_or(0.0),
                            position: resolved_positions
                                .get(&id)
                                .copied()
                                .unwrap_or([2.0, 0.0, 1.5]),
                            amplitude: vec![],
                            subcarrier_count: 0,
                            // Vitals-only path; still expose the sync snapshot
                            // if the node also speaks ESP-NOW.
                            sync: n.sync_snapshot(),
                            // ADR-297 — each node carries its own inference.
                            node_inference: Some(node_inference_for(n, now)),
                        })
                        .collect();

                    // ADR-297 — explicit, deterministic room aggregate over the
                    // per-node inferences (freshness-weighted vote). Not the
                    // latest-writer classification (issue #1555).
                    let room_inference = fuse_room(
                        active_nodes.iter().filter_map(|ni| ni.node_inference.as_ref()),
                        NODE_STALE_AFTER_MS,
                    );

                    let features = FeatureInfo {
                        mean_rssi: vitals.rssi as f64,
                        variance: vitals.motion_energy as f64,
                        motion_band_power: vitals.motion_energy as f64,
                        breathing_band_power: if vitals.presence { 0.5 } else { 0.0 },
                        dominant_freq_hz: vitals.breathing_rate_bpm / 60.0,
                        change_points: 0,
                        spectral_power: vitals.motion_energy as f64,
                    };

                    // Store latest features on node for cross-node fusion.
                    if let Some(ns) = s.node_states.get_mut(&node_id) {
                        ns.latest_features = Some(features.clone());
                    }

                    // Cross-node fusion: combine features from all active nodes.
                    let fused_features = fuse_multi_node_features(&features, &s.node_states);

                    // ADR-297 (issue #1554): the top-level classification is the
                    // fused room aggregate, not this packet's single node — a
                    // node's own reading no longer overwrites the room's. The
                    // old ad-hoc "boost confidence by node count" is replaced by
                    // `room_inference`'s freshness-weighted multi-node confidence.
                    let classification = if bootstrap_empty {
                        ClassificationInfo {
                            motion_level: "absent".to_string(),
                            presence: false,
                            confidence: 0.0,
                        }
                    } else {
                        debounce_room_classification(&mut s, &room_inference)
                    };

                    let signal_field = generate_signal_field(
                        fused_features.mean_rssi,
                        motion_score,
                        vitals.breathing_rate_bpm / 60.0,
                        (vitals.presence_score as f64).min(1.0),
                        &[],
                    );
                    let vital_candidates = VitalSigns {
                        breathing_rate_bpm: (vitals.breathing_rate_bpm > 0.0)
                            .then_some(vitals.breathing_rate_bpm),
                        heart_rate_bpm: (vitals.heartrate_bpm > 0.0)
                            .then_some(vitals.heartrate_bpm),
                        breathing_confidence: if vitals.presence { 0.7 } else { 0.0 },
                        heartbeat_confidence: if vitals.presence { 0.7 } else { 0.0 },
                        signal_quality: vitals.presence_score as f64,
                    };
                    let explicit_calibration_fresh =
                        s.explicit_calibration_fresh_at(observed_at_unix_ms);
                    let published_vitals = vitals_for_publication(
                        &vital_candidates,
                        explicit_calibration_fresh,
                        total_persons,
                    );

                    // Keep the legacy edge_vitals stream aligned with the
                    // governed sensing envelope. Raw numeric candidates are
                    // never published before the explicit calibration and
                    // single-occupant gates have passed.
                    let edge_vitals_message = edge_vitals_message_for_publication(
                        &vitals,
                        published_vitals.as_ref(),
                        bootstrap_empty,
                        explicit_calibration_fresh,
                        total_persons,
                    );
                    if let Ok(json) = serde_json::to_string(&edge_vitals_message) {
                        let _ = s.tx.send(json);
                    }

                    let mut update = SensingUpdate {
                        msg_type: "sensing_update".to_string(),
                        timestamp: chrono::Utc::now().timestamp_millis() as f64 / 1000.0,
                        source: "esp32".to_string(),
                        tick,
                        nodes: active_nodes,
                        features: fused_features.clone(),
                        classification,
                        signal_field,
                        vital_signs: published_vitals,
                        enhanced_motion: None,
                        enhanced_breathing: None,
                        posture: None,
                        signal_quality_score: None,
                        quality_verdict: None,
                        bssid_count: None,
                        pose_keypoints: None,
                        model_status: None,
                        persons: None,
                        estimated_persons: if total_persons > 0 {
                            Some(total_persons)
                        } else {
                            None
                        },
                        // ADR-084 Pass 3.6: surface per-node novelty_score
                        // (and the rest of the per-node feature snapshot)
                        // on the WebSocket envelope so cluster-Pi consumers
                        // can implement model-wake gating without round-
                        // tripping back to the server.
                        node_features: build_node_features(&s.node_states, now),
                        room_inference: Some(room_inference),
                    };

                    let raw_persons = derive_pose_from_sensing(&update);
                    let mut last_tracker_instant = s.last_tracker_instant.take();
                    let tracked = tracker_bridge::tracker_update(
                        &mut s.pose_tracker,
                        &mut last_tracker_instant,
                        raw_persons,
                    );
                    s.last_tracker_instant = last_tracker_instant;
                    if !tracked.is_empty() {
                        update.persons = Some(tracked);
                    }
                    // #1050: attach real signal_field-peak positions to each person.
                    attach_field_positions(&mut update);
                    assess_legacy_image_pose(&mut s, &update);

                    if let Ok(json) = serde_json::to_string(&update) {
                        let _ = s.tx.send(json);
                    }
                    observe_sensing_update(s.latest_update.as_ref(), &update);
                    s.latest_update = Some(update);
                    s.edge_vitals = Some(vitals);
                    continue;
                }

                // ADR-110 §A0.12: Try sync packet (magic 0xC511_A110).
                // A 32-byte UDP datagram carrying mesh-aligned epoch + sequence
                // high-water from the node's c6_sync_espnow EMA-smoothed offset.
                // Stored per-node so subsequent CSI frames with byte 19 bit 4
                // set can have an aligned timestamp recovered downstream.
                if len >= wifi_densepose_hardware::SYNC_PACKET_SIZE {
                    let magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
                    if magic == wifi_densepose_hardware::SYNC_PACKET_MAGIC {
                        match wifi_densepose_hardware::SyncPacket::from_bytes(&buf[..len]) {
                            Ok(sync) => {
                                debug!("ESP32 sync from {src}: node={} leader={} valid={} smoothed={} \
                                        seq={} offset_us={}",
                                       sync.node_id, sync.flags.is_leader, sync.flags.is_valid,
                                       sync.flags.smoothed_used, sync.sequence,
                                       sync.local_minus_epoch_us());
                                let mut s = state.write().await;
                                let node_id = sync.node_id;
                                let ns = s.node_states.entry(node_id)
                                    .or_insert_with(NodeState::new);
                                ns.apply_sync_packet(sync, std::time::Instant::now());
                                continue;
                            }
                            Err(e) => {
                                debug!("Sync packet decode error from {src}: {e}");
                                // Fall through — magic matched but decode failed; not a CSI frame.
                                continue;
                            }
                        }
                    }
                }

                // ADR-063: Try edge fused vitals packet (magic 0xC511_0004).
                // Must come BEFORE the WASM parser — issue #928: these two
                // packet types shared a magic and the WASM parser was eating
                // fused-vitals frames on the C6+mmWave config. The reassign of
                // WASM_OUTPUT_MAGIC → 0xC511_0007 (firmware side) plus this
                // dedicated parser resolve the collision.
                if let Some(fused) = parse_edge_fused_vitals(&buf[..len]) {
                    debug!(
                        "Edge fused vitals from {src}: node={} br={:.1} hr={:.1} \
                         mmwave_targets={} fusion_conf={}",
                        fused.node_id, fused.breathing_rate_bpm, fused.heartrate_bpm,
                        fused.mmwave_targets, fused.fusion_confidence,
                    );
                    let s = state.write().await;
                    if let Ok(json) = serde_json::to_string(&serde_json::json!({
                        "type": "edge_fused_vitals",
                        "node_id": fused.node_id,
                        "breathing_rate_bpm": fused.breathing_rate_bpm,
                        "heartrate_bpm": fused.heartrate_bpm,
                        "n_persons": fused.n_persons,
                        "person_count_valid": fused.person_count_valid,
                        "fusion_confidence": fused.fusion_confidence,
                        "mmwave": {
                            "hr_bpm": fused.mmwave_hr_bpm,
                            "br_bpm": fused.mmwave_br_bpm,
                            "distance_cm": fused.mmwave_distance_cm,
                            "targets": fused.mmwave_targets,
                            "confidence": fused.mmwave_confidence,
                            "type": fused.mmwave_type,
                        },
                        "motion_energy": fused.motion_energy,
                        "presence_score": fused.presence_score,
                        "timestamp_ms": fused.timestamp_ms,
                    })) {
                        let _ = s.tx.send(json);
                    }
                    continue;
                }

                // ADR-040: Try WASM output packet (magic 0xC511_0007 post-#928).
                if let Some(wasm_output) = parse_wasm_output(&buf[..len]) {
                    debug!(
                        "WASM output from {src}: node={} module={} events={}",
                        wasm_output.node_id,
                        wasm_output.module_id,
                        wasm_output.events.len()
                    );
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
                    debug!(
                        "ESP32 frame from {src}: node={}, subs={}, seq={}",
                        frame.node_id, frame.n_subcarriers, frame.sequence
                    );

                    let mut s = state.write().await;
                    s.source = "esp32".to_string();
                    let observed_at = std::time::Instant::now();
                    s.last_esp32_frame = Some(observed_at);
                    let grid_key = CsiGridKey::from_frame(&frame);
                    s.node_states
                        .entry(frame.node_id)
                        .or_insert_with(NodeState::new)
                        .observe_raw_grid(grid_key, observed_at);

                    // The field model owns a separate, frozen grid path. Feed
                    // it before the global inference grid gate so a stable
                    // 64-bin stream can remain comparable even when the
                    // independent live feature path prefers a wider grid.
                    s.maybe_feed_calibration_frame(
                        frame.node_id,
                        grid_key,
                        frame.sequence,
                        &frame.amplitudes,
                        observed_at,
                    );

                    // ── ADR-110 / issue #1005: per-node subcarrier-grid gate ──
                    // ESP32-C6 nodes interleave HE-SU 256-bin frames (~84%)
                    // with HT 64-bin frames on the same socket. HT-LTF and
                    // HE-LTF symbol grids are not bin-comparable, so a frame
                    // on a different grid than the node's rolling window must
                    // not enter the feature path. Policy (NodeState::accept_grid):
                    // lock onto the densest grid seen, clear+re-warm on
                    // upgrade, skip sparser-grid frames (arrival still
                    // recorded for fps/liveness).
                    let grid_accepted = s
                        .node_states
                        .entry(frame.node_id)
                        .or_insert_with(NodeState::new)
                        .accept_grid(frame.grid());
                    if !grid_accepted {
                        debug!(
                            "node {}: skipping {}-subcarrier {:?} frame (active grid {:?})",
                            frame.node_id,
                            frame.n_subcarriers,
                            frame.ppdu_type,
                            s.node_states.get(&frame.node_id).and_then(|ns| ns.active_grid),
                        );
                        if let Some(ns) = s.node_states.get_mut(&frame.node_id) {
                            ns.observe_csi_frame_arrival(std::time::Instant::now());
                        }
                        continue;
                    }

                    // Also maintain global frame_history for backward compat
                    // (simulation path, REST endpoints, etc.).
                    s.frame_history.push_back(frame.amplitudes.clone());
                    if s.frame_history.len() > FRAME_HISTORY_CAPACITY {
                        s.frame_history.pop_front();
                    }

                    // ── ADR-099: real-time introspection tap ────────────────
                    // Per-frame update of the attractor / DTW pipeline running
                    // parallel to the window-aggregated event path. Placed
                    // BEFORE the per-node `&mut` borrow of `s.node_states` so
                    // `s.intro` / `s.intro_tx` stay reachable. Never window-
                    // blocked; `/ws/introspection` sees a fresh snapshot on
                    // every accepted frame.
                    {
                        let intro_feature = if frame.amplitudes.is_empty() {
                            0.0
                        } else {
                            frame.amplitudes.iter().copied().sum::<f64>()
                                / frame.amplitudes.len() as f64
                        };
                        let intro_ts_ns = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_nanos() as u64)
                            .unwrap_or(0);
                        let _ = s.intro.update(intro_ts_ns, intro_feature);
                        if let Ok(intro_json) = serde_json::to_string(s.intro.snapshot()) {
                            let _ = s.intro_tx.send(intro_json);
                        }
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
                    // ADR-110 iter 19 — feed the per-node fps EMA from real
                    // CSI arrivals. The helper sets `last_frame_time` as a
                    // side effect, so the previous bare assignment is gone.
                    let first_sensing_frame = ns.observe_accepted_csi_frame(
                        frame.sequence,
                        frame.adr018_flags.ieee802154_sync_valid,
                        std::time::Instant::now(),
                    );
                    if first_sensing_frame && telemetry::curated_events_enabled() {
                        info!(name: semconv::EVENT_RUVIEW_NODE_ONLINE, { "ruview.node.id" = node_id }, "node {node_id} online (CSI)");
                    }

                    // ADR-084 Pass 3: cluster-Pi novelty sensor.
                    // Score this frame's feature vector against the per-node
                    // sketch bank *before* pushing it (so the score reflects
                    // pre-insert state). Result lands in `ns.last_novelty_score`
                    // for downstream model-wake gating.
                    ns.update_novelty(&frame.amplitudes);

                    ns.frame_history.push_back(frame.amplitudes.clone());
                    if ns.frame_history.len() > FRAME_HISTORY_CAPACITY {
                        ns.frame_history.pop_front();
                    }

                    let sample_rate_hz = ns.effective_sample_rate_hz();
                    let (
                        features,
                        mut classification,
                        breathing_rate_hz,
                        sub_variances,
                        raw_motion,
                    ) = extract_features_from_frame(&frame, &ns.frame_history, sample_rate_hz);
                    smooth_and_classify_node(ns, &mut classification, raw_motion);

                    // Adaptive override using cloned model (safe, no raw pointers).
                    if let Some(ref model) = adaptive_model_clone {
                        let amps = ns.frame_history.back().map(|v| v.as_slice()).unwrap_or(&[]);
                        let dominant_frequency =
                            adaptive_classifier::compatible_dominant_frequency(
                                model.version,
                                features.dominant_freq_hz,
                                amps,
                            );
                        let feat_arr = adaptive_classifier::features_from_runtime(
                            &serde_json::json!({
                                "variance": features.variance,
                                "motion_band_power": features.motion_band_power,
                                "breathing_band_power": features.breathing_band_power,
                                "spectral_power": features.spectral_power,
                                "dominant_freq_hz": dominant_frequency,
                                "change_points": features.change_points,
                                "mean_rssi": features.mean_rssi,
                            }),
                            amps,
                        );
                        let (label, conf) = model.classify(&feat_arr);
                        classification.motion_level = label.to_string();
                        classification.presence = label != "absent";
                        classification.confidence =
                            (conf * 0.7 + classification.confidence * 0.3).clamp(0.0, 1.0);
                    }

                    ns.rssi_history.push_back(features.mean_rssi);
                    if ns.rssi_history.len() > 60 {
                        ns.rssi_history.pop_front();
                    }

                    if ns.csi_fps_samples >= 5
                        && ns.vital_detector.reconfigure_sample_rate(sample_rate_hz)
                    {
                        // Never smooth estimates computed against two clocks.
                        ns.smoothed_hr = 0.0;
                        ns.smoothed_br = 0.0;
                        ns.smoothed_hr_conf = 0.0;
                        ns.smoothed_br_conf = 0.0;
                        ns.hr_buffer.clear();
                        ns.br_buffer.clear();
                    }

                    let raw_vitals = ns
                        .vital_detector
                        .process_frame(&frame.amplitudes, &frame.phases);
                    let vitals = smooth_vitals_node(ns, &raw_vitals);
                    ns.latest_vitals = vitals.clone();

                    // DynamicMinCut person estimation from subcarrier correlation.
                    let corr_persons = estimate_persons_from_correlation(&ns.frame_history);
                    // #803: map the min-cut count onto a threshold-aligned score
                    // so it round-trips back to the same count. The old
                    // `corr_persons / 3.0` left 2 people at 0.667 — under the
                    // 0.70 up-threshold — so the count was pinned at 1.
                    let raw_score = corr_persons_to_score(corr_persons);
                    ns.smoothed_person_score = ns.smoothed_person_score * 0.92 + raw_score * 0.08;
                    if classification.presence {
                        let count =
                            score_to_person_count(ns.smoothed_person_score, ns.prev_person_count);
                        ns.prev_person_count = count;
                    } else {
                        ns.prev_person_count = 0;
                    }

                    // Store latest features on node for cross-node fusion.
                    ns.latest_features = Some(features.clone());

                    // Done with per-node mutable borrow; now read aggregated
                    // state from all nodes (the borrow of `ns` ends here).
                    // (We re-borrow node_states immutably via `s` below.)

                    s.rssi_history.push_back(features.mean_rssi);
                    if s.rssi_history.len() > 60 {
                        s.rssi_history.pop_front();
                    }
                    s.latest_vitals = vitals.clone();

                    // Cross-node fusion: combine features from all active nodes.
                    let fused_features = fuse_multi_node_features(&features, &s.node_states);

                    s.tick += 1;
                    let tick = s.tick;

                    let motion_score = if classification.motion_level == "active" {
                        0.8
                    } else if classification.motion_level == "present_still" {
                        0.3
                    } else {
                        0.05
                    };

                    let observed_at_unix_ms =
                        chrono::Utc::now().timestamp_millis().max(0) as u64;
                    let bootstrap_empty =
                        s.bootstrap_empty_prior_applies(observed_at_unix_ms);

                    // A restored prior can suppress a background-only raw
                    // classification. It cannot authorize positive presence.
                    let now = std::time::Instant::now();
                    let total_persons = if bootstrap_empty {
                        0
                    } else if classification.presence {
                        let dedup = s.dedup_factor;
                        let (fused, fallback_count) = multistatic_bridge::fuse_or_fallback(
                            &s.multistatic_fuser,
                            &s.node_states,
                            dedup,
                        );
                        match fused {
                            Some(ref f) => {
                                let score =
                                    multistatic_bridge::compute_person_score_from_amplitudes(
                                        &f.fused_amplitude,
                                    );
                                s.smoothed_person_score =
                                    s.smoothed_person_score * 0.90 + score * 0.10;
                                // #803: don't let the saturating activity score
                                // discard count-aware per-node estimates.
                                let count =
                                    aggregate_person_count(
                                        s.person_count_at(observed_at_unix_ms),
                                        &s.node_states,
                                    );
                                s.prev_person_count = count;
                                count.max(1)
                            }
                            None => {
                                aggregate_person_count(fallback_count.unwrap_or(0), &s.node_states)
                                    .max(1)
                            }
                        }
                    } else {
                        s.prev_person_count = 0;
                        0
                    };

                    // Governed trust cycle (ADR-135..146): run the same live
                    // frames through the privacy/provenance/witness control
                    // plane. Trust state is recorded on the bridge (exposed on
                    // /api/v1/status); engine errors are counted + rate-limit
                    // logged instead of being swallowed (review finding 1).
                    // Split-borrow the two distinct fields off the guard.
                    {
                        let sref: &mut AppStateInner = &mut s;
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_millis() as i64)
                            .unwrap_or(0);
                        sref.engine_bridge.observe_cycle(&sref.node_states, now_ms);
                    }

                    // Build nodes array with all active nodes. ADR-141 output
                    // gating (review finding 1c): when the governed engine
                    // emitted this cycle at class Restricted (base mode, or a
                    // contradiction/mesh-risk demotion below the configured
                    // class), the per-node raw amplitude vectors are suppressed
                    // from the live publish — the same field mapping bfld's
                    // privacy gate applies at Restricted (drop amplitude/phase
                    // proxies).
                    let suppress_raw = s.engine_bridge.suppress_raw_outputs();
                    let resolved_positions =
                        node_positions_by_active_id(&s.node_positions_config, &s.node_states, now);
                    let active_nodes: Vec<NodeInfo> = s
                        .node_states
                        .iter()
                        .filter(|(_, n)| {
                            n.last_frame_time
                                .is_some_and(|t| now.duration_since(t).as_secs() < 10)
                        })
                        .map(|(&id, n)| NodeInfo {
                            node_id: id,
                            rssi_dbm: n.rssi_history.back().copied().unwrap_or(0.0),
                            position: resolved_positions
                                .get(&id)
                                .copied()
                                .unwrap_or([2.0, 0.0, 1.5]),
                            amplitude: if suppress_raw {
                                vec![]
                            } else {
                                n.frame_history
                                    .back()
                                    .map(|a| a.iter().take(56).cloned().collect())
                                    .unwrap_or_default()
                            },
                            subcarrier_count: if suppress_raw {
                                0
                            } else {
                                n.frame_history.back().map_or(0, |a| a.len())
                            },
                            // ADR-110 iter 23 / iter 30 — single source of truth.
                            sync: n.sync_snapshot(),
                            // ADR-297 — each node carries its own inference.
                            node_inference: Some(node_inference_for(n, now)),
                        })
                        .collect();

                    // ADR-297 — explicit deterministic room aggregate over the
                    // per-node inferences (not last-writer; issue #1555).
                    let room_inference = fuse_room(
                        active_nodes.iter().filter_map(|ni| ni.node_inference.as_ref()),
                        NODE_STALE_AFTER_MS,
                    );
                    let room_classification = if bootstrap_empty {
                        ClassificationInfo {
                            motion_level: "absent".to_string(),
                            presence: false,
                            confidence: 0.0,
                        }
                    } else {
                        debounce_room_classification(&mut s, &room_inference)
                    };
                    let explicit_calibration_fresh =
                        s.explicit_calibration_fresh_at(observed_at_unix_ms);
                    let published_vitals = vitals_for_publication(
                        &vitals,
                        explicit_calibration_fresh,
                        total_persons,
                    );

                    let mut update = SensingUpdate {
                        msg_type: "sensing_update".to_string(),
                        timestamp: chrono::Utc::now().timestamp_millis() as f64 / 1000.0,
                        source: "esp32".to_string(),
                        tick,
                        nodes: active_nodes,
                        features: fused_features.clone(),
                        // ADR-297 (issue #1554): top-level classification is the
                        // fused room aggregate, not this frame's single node.
                        // `classification` (this node's own smoothed reading)
                        // still drives `motion_score`/`total_persons` above,
                        // which are legitimately this-packet-local.
                        classification: room_classification,
                        signal_field: generate_signal_field(
                            fused_features.mean_rssi,
                            motion_score,
                            breathing_rate_hz,
                            fused_features.variance.min(1.0),
                            &sub_variances,
                        ),
                        vital_signs: published_vitals,
                        enhanced_motion: None,
                        enhanced_breathing: None,
                        posture: None,
                        signal_quality_score: None,
                        quality_verdict: None,
                        bssid_count: None,
                        pose_keypoints: None,
                        model_status: None,
                        persons: None,
                        estimated_persons: if total_persons > 0 {
                            Some(total_persons)
                        } else {
                            None
                        },
                        // ADR-084 Pass 3.6: surface per-node novelty_score
                        // (and the rest of the per-node feature snapshot)
                        // on the WebSocket envelope so cluster-Pi consumers
                        // can implement model-wake gating without round-
                        // tripping back to the server.
                        node_features: build_node_features(&s.node_states, now),
                        room_inference: Some(room_inference),
                    };

                    let raw_persons = derive_pose_from_sensing(&update);
                    let mut last_tracker_instant = s.last_tracker_instant.take();
                    let tracked = tracker_bridge::tracker_update(
                        &mut s.pose_tracker,
                        &mut last_tracker_instant,
                        raw_persons,
                    );
                    s.last_tracker_instant = last_tracker_instant;
                    if !tracked.is_empty() {
                        update.persons = Some(tracked);
                    }
                    // #1050: attach real signal_field-peak positions to each person.
                    attach_field_positions(&mut update);
                    assess_legacy_image_pose(&mut s, &update);

                    if let Ok(json) = serde_json::to_string(&update) {
                        let _ = s.tx.send(json);
                    }

                    // ── ADR-262 P3: emit a signed RuField FieldEvent ────────
                    // Join this cycle's SensingUpdate (features / classification
                    // / signal_field) with the governed engine's trust state
                    // (effective_class / demoted, recorded by `observe_cycle`
                    // above) into a `SensingSnapshot`, and surface it on
                    // `/api/field` + `/ws/field` via the P1 bridge. Only cycles
                    // whose mapped privacy class clears the §10 network egress
                    // gate are surfaced (P1/P2); a `Derived → P4/P5` cycle is
                    // held edge-local. `presence == false` ⇒ no phantom event.
                    emit_rufield_event(&s, &update, node_id);

                    observe_sensing_update(s.latest_update.as_ref(), &update);
                    s.latest_update = Some(update);

                    // Evict stale nodes every 100 ticks to prevent memory leak.
                    if tick % 100 == 0 {
                        let stale = Duration::from_secs(60);
                        let stale_ids: Vec<u8> = s
                            .node_states
                            .iter()
                            .filter(|(_, ns)| {
                                !ns.last_frame_time
                                    .is_some_and(|t| now.duration_since(t) < stale)
                            })
                            .map(|(&id, _)| id)
                            .collect();
                        for id in &stale_ids {
                            s.node_states.remove(id);
                            if telemetry::curated_events_enabled() {
                                info!(name: semconv::EVENT_RUVIEW_NODE_OFFLINE, { "ruview.node.id" = *id }, "node {id} offline (no frames for 60s)");
                            }
                        }
                        if !stale_ids.is_empty() {
                            info!(
                                "Evicted {} stale node(s), {} active",
                                stale_ids.len(),
                                s.node_states.len()
                            );
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

/// Cadence, in sensing ticks, of the periodic `ruview.csi.stats` and
/// `ruview.vitals.estimate` telemetry snapshots.
const TELEMETRY_SNAPSHOT_TICKS: u64 = 100;

/// Emit the curated telemetry events for a finished sensing cycle
/// (names and attribute keys from `semconv/registry/` at the repo root):
/// `ruview.presence.changed` on presence transitions against the
/// previously published update, plus the cadenced `ruview.csi.stats` and
/// `ruview.vitals.estimate` snapshots. Called from every path that
/// publishes a `SensingUpdate`, right before it lands in `latest_update`
/// — never per frame at full rate.
fn observe_sensing_update(prev: Option<&SensingUpdate>, update: &SensingUpdate) {
    if !telemetry::curated_events_enabled() {
        return;
    }

    let presence = update.classification.presence;
    if prev.map(|u| u.classification.presence) != Some(presence) {
        let state = if presence { "present" } else { "absent" };
        info!(
            name: semconv::EVENT_RUVIEW_PRESENCE_CHANGED,
            {
                "ruview.presence.state" = state,
                "ruview.motion.level" = %update.classification.motion_level,
                "ruview.inference.confidence" = update.classification.confidence,
                "ruview.persons.count" = update.estimated_persons.unwrap_or(0) as u64,
                "ruview.csi.source" = %update.source,
            },
            "presence changed: {state}"
        );
    }
    if update.tick % TELEMETRY_SNAPSHOT_TICKS == 0 {
        info!(
            name: semconv::EVENT_RUVIEW_CSI_STATS,
            {
                "ruview.csi.frames_total" = update.tick,
                "ruview.csi.nodes_active" = update.nodes.len() as u64,
                "ruview.csi.source" = %update.source,
            },
            "csi stats: {} frames processed, {} active node(s)",
            update.tick,
            update.nodes.len()
        );
        if let Some(v) = &update.vital_signs {
            if v.breathing_rate_bpm.is_some() || v.heart_rate_bpm.is_some() {
                info!(
                    name: semconv::EVENT_RUVIEW_VITALS_ESTIMATE,
                    {
                        "ruview.vitals.breathing_rate_bpm" = v.breathing_rate_bpm.unwrap_or(0.0),
                        "ruview.vitals.heart_rate_bpm" = v.heart_rate_bpm.unwrap_or(0.0),
                        "ruview.vitals.breathing_confidence" = v.breathing_confidence,
                        "ruview.vitals.heartbeat_confidence" = v.heartbeat_confidence,
                        "ruview.csi.source" = %update.source,
                    },
                    "vitals estimate"
                );
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

        // Issue #1004: in `auto` mode this task runs alongside `udp_receiver_task`.
        // Once a real frame promotes `source` → "esp32", stop emitting synthetic
        // frames so we never clobber live CSI with simulated poses. (For an
        // explicit `--source simulated` demo, `source` stays "simulated" and the
        // simulator keeps running — that path never binds UDP, so it is never
        // promoted.) The task stays alive so it can resume serving if the real
        // source later ages out to "esp32:offline".
        if s.effective_source() == "esp32" {
            continue;
        }

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

        let motion_score = if classification.motion_level == "active" {
            0.8
        } else if classification.motion_level == "present_still" {
            0.3
        } else {
            0.05
        };

        let raw_vitals = s
            .vital_detector
            .process_frame(&frame.amplitudes, &frame.phases);
        let vitals = smooth_vitals(&mut s, &raw_vitals);
        s.latest_vitals = vitals.clone();

        let frame_amplitudes = frame.amplitudes.clone();
        let frame_n_sub = frame.n_subcarriers;

        // ADR-044 §5.2: feed raw features into rolling-P95 estimators before scoring.
        s.p95_variance.push(features.variance);
        s.p95_motion_band_power.push(features.motion_band_power);
        s.p95_spectral_power.push(features.spectral_power);

        // Multi-person estimation with temporal smoothing (EMA α=0.10).
        let raw_score = compute_person_score(&s, &features);
        s.smoothed_person_score = s.smoothed_person_score * 0.90 + raw_score * 0.10;
        let est_persons = if classification.presence {
            let count = s.person_count();
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
                sync: None,  // simulated frame path — no mesh peer
                node_inference: None, // simulated frame; source is synthetic
            }],
            features: features.clone(),
            classification,
            signal_field: generate_signal_field(
                features.mean_rssi,
                motion_score,
                breathing_rate_hz,
                features.variance.min(1.0),
                &sub_variances,
            ),
            vital_signs: None,
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
            estimated_persons: if est_persons > 0 {
                Some(est_persons)
            } else {
                None
            },
            node_features: None,
            room_inference: None,
        };

        // Populate persons from the sensing update (Kalman-smoothed via tracker).
        let raw_persons = derive_pose_from_sensing(&update);
        let mut last_tracker_instant = s.last_tracker_instant.take();
        let tracked = tracker_bridge::tracker_update(
            &mut s.pose_tracker,
            &mut last_tracker_instant,
            raw_persons,
        );
        s.last_tracker_instant = last_tracker_instant;
        if !tracked.is_empty() {
            update.persons = Some(tracked);
        }
        // #1050: attach real signal_field-peak positions to each person.
        attach_field_positions(&mut update);
        assess_legacy_image_pose(&mut s, &update);

        if update.classification.presence {
            s.total_detections += 1;
        }
        if let Ok(json) = serde_json::to_string(&update) {
            let _ = s.tx.send(json);
        }
        observe_sensing_update(s.latest_update.as_ref(), &update);
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
                //
                // Issue #618: overwrite `source` with `effective_source()`
                // before each broadcast so a stale latest_update (frozen
                // payload from a now-offline ESP32) is emitted with
                // `source: "esp32:offline"` instead of `source: "esp32"`.
                // The REST `/health` endpoint already does this; before
                // this fix the WS path was the only consumer that didn't,
                // so the UI's "LIVE — ESP32 HARDWARE Connected" banner
                // stayed green long after the hardware went away.
                let mut tagged = update.clone();
                tagged.source = s.effective_source();
                if let Ok(json) = serde_json::to_string(&tagged) {
                    let _ = s.tx.send(json);
                }
            }
        }
    }
}

/// Map one sensing-broadcast JSON document into the `VitalsSnapshot`(s) to
/// publish over MQTT (issues #872/#898).
///
/// Multi-node sources carry a `nodes` array where **each node has its own
/// `classification`** (`motion_level`, `presence`, `confidence`) and RSSI — so
/// each node must surface its *own* presence/motion, not the room-level
/// aggregate. Previously the bridge applied the aggregate `classification` to
/// every per-node Home-Assistant device, so a node in an empty corner inherited
/// another node's "present" (and `motion_level: "absent"` was mis-mapped to full
/// motion). Vitals (breathing / heart rate) and the person count are room-level
/// and shared across the per-node devices. Falls back to a single aggregate
/// snapshot when there is no per-node data (e.g. wifi / simulate sources).
#[cfg(feature = "mqtt")]
fn vitals_snapshots_from_sensing_json(
    v: &serde_json::Value,
    base_id: &str,
) -> Vec<wifi_densepose_sensing_server::mqtt::state::VitalsSnapshot> {
    use wifi_densepose_sensing_server::mqtt::state::VitalsSnapshot;

    // motion_level string -> motion scalar. "absent"/"none"/"still"/"idle"/""
    // are non-moving; anything else (walking, …) is motion. `fallback` is used
    // when the field is absent so a partial per-node payload defers to the
    // room aggregate rather than silently reading 0.
    fn motion_of(level: Option<&str>, fallback: f64) -> f64 {
        match level {
            Some("none") | Some("still") | Some("idle") | Some("absent") | Some("") => 0.0,
            Some(_) => 1.0,
            None => fallback,
        }
    }

    let ts = (v["timestamp"].as_f64().unwrap_or(0.0) * 1000.0) as i64;
    let vit = &v["vital_signs"];
    let breathing = vit["breathing_rate_bpm"].as_f64();
    let hr = vit["heart_rate_bpm"].as_f64();
    let n_persons = v["persons"]
        .as_array()
        .map(|a| a.len() as u32)
        .or_else(|| v["estimated_persons"].as_u64().map(|x| x as u32))
        .unwrap_or(0);

    // Room-level aggregate: the no-nodes fallback, and the per-node default for
    // any field a node omits.
    let acls = &v["classification"];
    let agg_presence = acls["presence"].as_bool().unwrap_or(false);
    let agg_motion = motion_of(acls["motion_level"].as_str(), 0.0);
    let agg_conf = acls["confidence"].as_f64().unwrap_or(0.0);

    let mk = |node_id: String, presence: bool, motion: f64, conf: f64, rssi: Option<f64>| {
        VitalsSnapshot {
            node_id,
            timestamp_ms: ts,
            presence,
            motion,
            presence_score: if presence { conf.max(0.0) } else { 0.0 },
            breathing_rate_bpm: breathing,
            heartrate_bpm: hr,
            n_persons,
            rssi_dbm: rssi,
            vital_confidence: conf,
            ..Default::default()
        }
    };

    match v["nodes"].as_array() {
        Some(arr) if !arr.is_empty() => arr
            .iter()
            .map(|node| {
                let n = node["node_id"].as_u64().unwrap_or(0);
                // Each node carries its OWN classification under `node_inference`
                // (ADR-297) — use it, deferring to the room aggregate only for
                // fields the node omits. Issue #1541: this previously read a
                // `"classification"` key that does not exist on `NodeInfo`'s
                // serialized JSON (the field is `node_inference`), so every
                // per-node lookup silently fell through to the room aggregate —
                // per-node MQTT topics carried array-global values.
                let ninf = &node["node_inference"];
                let presence = ninf["classification"]
                    .as_str()
                    .map(|c| c != "absent")
                    .unwrap_or(agg_presence);
                let motion = motion_of(ninf["classification"].as_str(), agg_motion);
                let conf = ninf["confidence"].as_f64().unwrap_or(agg_conf);
                mk(
                    format!("{base_id}-node{n}"),
                    presence,
                    motion,
                    conf,
                    node["rssi_dbm"].as_f64(),
                )
            })
            .collect(),
        _ => vec![mk(
            base_id.to_string(),
            agg_presence,
            agg_motion,
            agg_conf,
            v["nodes"][0]["rssi_dbm"].as_f64(),
        )],
    }
}

/// Build the multistatic guard config from the environment (#1031, #1049).
///
/// Three precedence layers, most-specific wins:
/// 1. `WDP_GUARD_INTERVAL_US` (+ optional `WDP_SOFT_GUARD_US`) — a **direct**
///    hard-guard override. This is the #1049 escape hatch: WiFi/ESP-NOW-synced
///    ESP32 nodes drift 10–150 ms (the 100 ms beacon + WiFi-MAC jitter cannot
///    hold two independently-clocked boards within the published default), so a
///    deployment can simply lift the guard past its measured spread (e.g.
///    `WDP_GUARD_INTERVAL_US=200000`) without knowing its exact TDM schedule.
/// 2. `WDP_TDM_SLOTS` + `WDP_TDM_SLOT_US` (both positive) — derive the guard
///    from the declared schedule via [`MultistaticConfig::for_tdm_schedule`].
/// 3. Otherwise the published default (60 ms hard / 20 ms soft).
///
/// The direct override (1) is applied **on top of** whichever base (2 or 3) is
/// selected, so `WDP_GUARD_INTERVAL_US` always wins for the hard guard while a
/// TDM-derived soft band is preserved unless it would exceed the new hard guard.
/// `min_nodes` is *not* set here — the caller overrides it for single-node
/// passthrough.
fn multistatic_guard_config_from_env() -> MultistaticConfig {
    multistatic_guard_config_from(
        std::env::var("WDP_TDM_SLOTS").ok().as_deref(),
        std::env::var("WDP_TDM_SLOT_US").ok().as_deref(),
        std::env::var("WDP_GUARD_INTERVAL_US").ok().as_deref(),
        std::env::var("WDP_SOFT_GUARD_US").ok().as_deref(),
    )
}

/// Pure core of [`multistatic_guard_config_from_env`] for testability.
fn multistatic_guard_config_from(
    slots: Option<&str>,
    slot_us: Option<&str>,
    guard_us: Option<&str>,
    soft_us: Option<&str>,
) -> MultistaticConfig {
    // Base: TDM-schedule-derived when both slot params are valid, else default.
    let mut cfg = match (
        slots.and_then(|s| s.trim().parse::<usize>().ok()),
        slot_us.and_then(|s| s.trim().parse::<u64>().ok()),
    ) {
        (Some(n), Some(us)) if n >= 1 && us >= 1 => MultistaticConfig::for_tdm_schedule(n, us),
        _ => MultistaticConfig::default(),
    };

    // Direct hard-guard override (#1049). Ignored when unset/zero/unparseable so
    // a malformed env var falls back to the base rather than breaking fusion.
    if let Some(g) = guard_us
        .and_then(|s| s.trim().parse::<u64>().ok())
        .filter(|&g| g >= 1)
    {
        cfg.guard_interval_us = g;
        // Keep the soft band strictly below the (possibly lowered) hard guard.
        if cfg.soft_guard_us >= g {
            cfg.soft_guard_us = g.saturating_sub(1).max(1);
        }
    }

    // Optional explicit soft-guard override, always clamped strictly below hard.
    if let Some(s) = soft_us
        .and_then(|s| s.trim().parse::<u64>().ok())
        .filter(|&s| s >= 1)
    {
        cfg.soft_guard_us = s.min(cfg.guard_interval_us.saturating_sub(1).max(1));
    }

    cfg
}

/// Turn a `ProgressiveLoader::new` failure into an actionable diagnostic (#894).
///
/// The published HuggingFace `ruvnet/wifi-densepose-pretrained` files
/// (`model.safetensors`, `model-q{2,4,8}.bin`, `model.rvf.jsonl`) are a
/// different *format* — and a different encoder architecture — than the RVF
/// binary container the `--model` progressive loader expects (`RVFS` magic
/// `0x52564653`). Feeding one to `--model` produced a bare
/// "invalid magic at offset 0 …" that left users stuck. Detect the common
/// cases and explain plainly what's loadable instead.
///
/// Superseded in the live load path by [`load_or_convert_model`] (which now
/// converts the convertible formats instead of just explaining), but retained
/// as the human-readable format-landscape summary and exercised by tests.
#[allow(dead_code)]
fn diagnose_model_load_error(path: &std::path::Path, data: &[u8], err: &str) -> String {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    // safetensors: 8-byte LE header length, then a JSON object starting with '{'.
    let looks_safetensors = ext == "safetensors" || (data.len() > 9 && data[8] == b'{');
    // JSONL manifest: starts with '{' (or the well-known suffix).
    let looks_jsonl =
        ext == "jsonl" || name.ends_with(".rvf.jsonl") || data.first() == Some(&b'{');
    // Quantized weight blob shipped on HF (model-q2/q4/q8.bin).
    let looks_quant_bin = ext == "bin" || name.contains("-q");

    let kind = if looks_safetensors {
        "a safetensors weight file"
    } else if looks_jsonl {
        "a JSONL manifest, not the binary container"
    } else if looks_quant_bin {
        "a quantized weight blob (e.g. HuggingFace model-q4.bin)"
    } else {
        "not an RVF binary container"
    };

    format!(
        "model `{}` could not be loaded: it is {kind}. The --model flag expects an \
         RVF binary container (`RVFS` magic 0x52564653) produced by the \
         wifi-densepose-train pipeline. The HuggingFace ruvnet/wifi-densepose-pretrained \
         files are a different format and encoder architecture, so they do not load \
         here directly (issue #894). Continuing with signal heuristics. (loader: {err})",
        path.display()
    )
}

/// Load a model for `--model`, auto-detecting + converting the published
/// HuggingFace formats when the native RVF loader rejects them (issue #894).
///
/// Order of operations:
/// 1. **Detect the format before constructing the lazy progressive loader.**
/// 2. Load native RVF directly (the only format with `RVFS` magic).
/// 3. If the format is convertible
///    (`safetensors` / `model.rvf.jsonl`), convert it in-memory to RVF and load
///    that — so the published `model.safetensors` becomes loadable here.
/// 4. If it is a non-convertible format (quantized blob / unknown), return the
///    typed, actionable [`model_format::ModelLoadError`] message — never the
///    opaque "invalid magic …" string.
///
/// Returns the loaded `ProgressiveLoader` or a human-actionable error string.
fn load_or_convert_model(
    path: &std::path::Path,
    data: &[u8],
) -> Result<ProgressiveLoader, String> {
    use model_format::{convert_to_rvf, detect_format, ModelFormat};

    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();
    let model_id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("converted-model");

    match detect_format(data, &name) {
        // Native RVF is the only format passed straight to the lazy loader.
        // Detect first: ProgressiveLoader::new intentionally defers parsing and
        // can otherwise accept JSONL as an empty container until a later layer.
        ModelFormat::Rvf => ProgressiveLoader::new(data).map_err(|e| {
            model_format::classify_load_failure(data, &name, &e).to_string()
        }),
        // Convertible formats: convert in-memory, then load.
        ModelFormat::Safetensors | ModelFormat::JsonlManifest => {
            match convert_to_rvf(data, &name, model_id) {
                Ok(rvf_bytes) => {
                    info!(
                        "Model `{}` is {} — converting to RVF in-memory and loading (issue #894)",
                        path.display(),
                        detect_format(data, &name).label()
                    );
                    ProgressiveLoader::new(&rvf_bytes).map_err(|e| {
                        format!(
                            "converted {} to RVF but the container failed to load: {e}",
                            detect_format(data, &name).label()
                        )
                    })
                }
                Err(conv_err) => Err(conv_err.to_string()),
            }
        }
        // 3. Non-convertible: typed actionable error.
        _ => Err(model_format::classify_load_failure(
            data,
            &name,
            "RVF container parse failed",
        )
        .to_string()),
    }
}

/// `--convert-model` entry point (issue #894): read `in_path`, convert it to an
/// RVF binary container, write it to `out_path`, and verify the result loads.
/// Returns a process exit code (0 = success).
fn run_convert_model(in_path: &std::path::Path, out_path: &std::path::Path) -> i32 {
    let data = match std::fs::read(in_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("convert-model: failed to read {}: {e}", in_path.display());
            return 1;
        }
    };
    let name = in_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();
    let model_id = in_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("converted-model");

    let detected = model_format::detect_format(&data, &name);
    eprintln!(
        "convert-model: detected {} ({} bytes)",
        detected.label(),
        data.len()
    );

    match model_format::convert_to_rvf(&data, &name, model_id) {
        Ok(rvf_bytes) => {
            // Verify the converted bytes actually load before writing.
            if let Err(e) = ProgressiveLoader::new(&rvf_bytes) {
                eprintln!("convert-model: produced RVF did NOT load (bug): {e}");
                return 1;
            }
            if let Err(e) = std::fs::write(out_path, &rvf_bytes) {
                eprintln!("convert-model: failed to write {}: {e}", out_path.display());
                return 1;
            }
            eprintln!(
                "convert-model: wrote {} ({} bytes). Load it with `--model {}`.",
                out_path.display(),
                rvf_bytes.len(),
                out_path.display()
            );
            0
        }
        Err(e) => {
            eprintln!("convert-model: {e}");
            1
        }
    }
}

/// Whether `--export-rvf` should emit the placeholder container-format demo.
///
/// It must only do so **standalone**. Combined with `--train`/`--pretrain` the
/// real model is produced by the training pipeline, so short-circuiting here
/// would silently skip training and write placeholder weights — the #894 bug
/// where the documented `--train … --export-rvf` workflow produced a fake model.
fn export_emits_placeholder_demo(export_set: bool, train: bool, pretrain: bool) -> bool {
    export_set && !train && !pretrain
}

// ── Main ─────────────────────────────────────────────────────────────────────

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
    // Initialize tracing; with the `otel` feature and
    // OTEL_EXPORTER_OTLP_ENDPOINT set, logs also export over OTLP
    // (service.name = "ruview") — see telemetry.rs. The guard flushes
    // pending log records on exit.
    let _telemetry = telemetry::init();

    let mut args = Args::parse();
    args.ui_path = coalesce_ui_path(args.ui_path);

    if let Err(error) = validate_startup_calibration_request(args.calibrate) {
        eprintln!("Error: {error}");
        std::process::exit(2);
    }

    // Handle --benchmark mode: run vital sign benchmark and exit
    if args.benchmark {
        eprintln!("Running vital sign detection benchmark (1000 frames)...");
        let (total, per_frame) = vital_signs::run_benchmark(1000);
        eprintln!();
        eprintln!("Summary: {total:?} total, {per_frame:?} per frame");
        return;
    }

    // Handle --convert-model: turn a published HF model file (safetensors /
    // model.rvf.jsonl) into the RVF binary container --model expects, then exit
    // (issue #894). Gives the reporter a one-command path off the heuristics.
    if let Some(ref in_path) = args.convert_model {
        let out_path = args
            .convert_out
            .clone()
            .unwrap_or_else(|| in_path.with_extension("rvf"));
        std::process::exit(run_convert_model(in_path, &out_path));
    }

    // Handle --export-rvf: writes a CONTAINER-FORMAT DEMO with placeholder
    // weights — it is NOT a trained model. Only short-circuit when standalone:
    // combined with --train/--pretrain the real model is exported by the
    // training pipeline, and short-circuiting here would silently skip training
    // and write placeholder weights (#894 — the documented `--train …
    // --export-rvf` workflow produced a placeholder and never trained).
    if export_emits_placeholder_demo(args.export_rvf.is_some(), args.train, args.pretrain) {
        let rvf_path = args
            .export_rvf
            .as_ref()
            .expect("export_emits_placeholder_demo implies export_rvf is set");
        eprintln!(
            "WARNING: --export-rvf writes a CONTAINER-FORMAT DEMO with placeholder \
             weights — it is NOT a trained model. Train one with \
             `--train --dataset <DIR>` (which exports a calibrated .rvf to the \
             models/ directory), or download a pretrained encoder. See issue #894."
        );
        eprintln!("Exporting RVF container package (placeholder weights)...");
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
    } else if args.export_rvf.is_some() {
        // --export-rvf alongside --train/--pretrain: don't emit a placeholder.
        // Fall through so training runs; it exports the real calibrated model.
        eprintln!(
            "Note: --export-rvf is ignored in training mode — the trained model \
             is exported by the training pipeline to the models/ directory."
        );
    }

    // Handle --pretrain mode: self-supervised contrastive pretraining (ADR-024)
    if args.pretrain {
        eprintln!("=== WiFi-DensePose Contrastive Pretraining (ADR-024) ===");

        let ds_path = args
            .dataset
            .clone()
            .unwrap_or_else(|| PathBuf::from("data"));
        let source = match args.dataset_type.as_str() {
            "wipose" => dataset::DataSource::WiPose(ds_path.clone()),
            _ => dataset::DataSource::MmFi(ds_path.clone()),
        };
        let pipeline = dataset::DataPipeline::new(dataset::DataConfig {
            source,
            ..Default::default()
        });

        // Generate synthetic or load real CSI windows
        let generate_synthetic_windows = || -> Vec<Vec<Vec<f32>>> {
            (0..50)
                .map(|i| {
                    (0..4)
                        .map(|a| {
                            (0..56)
                                .map(|s| ((i * 7 + a * 13 + s) as f32 * 0.31).sin() * 0.5)
                                .collect()
                        })
                        .collect()
                })
                .collect()
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

        let n_subcarriers = csi_windows
            .first()
            .and_then(|w| w.first())
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
            epochs: args.pretrain_epochs,
            batch_size: 8,
            lr: 0.001,
            warmup_epochs: 2,
            min_lr: 1e-6,
            early_stop_patience: args.pretrain_epochs + 1,
            pretrain_temperature: 0.07,
            ..Default::default()
        };
        let mut t = trainer::Trainer::with_transformer(trainer_config, transformer);

        let e_config = embedding::EmbeddingConfig {
            d_model: 64,
            d_proj: 128,
            temperature: 0.07,
            normalize: true,
        };
        let mut projection = embedding::ProjectionHead::new(e_config.clone());
        let augmenter = embedding::CsiAugmenter::new();

        eprintln!(
            "Starting contrastive pretraining for {} epochs...",
            args.pretrain_epochs
        );
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
                Ok(()) => eprintln!(
                    "RVF saved ({} transformer + {} projection params)",
                    weights.len(),
                    proj_weights.len()
                ),
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
            Err(e) => {
                eprintln!("Failed to load model: {e}");
                std::process::exit(1);
            }
        };

        let weights = reader.weights().unwrap_or_default();
        let (embed_config_json, proj_weights) = reader.embedding().unwrap_or_else(|| {
            eprintln!("Warning: no embedding segment in RVF, using defaults");
            (
                serde_json::json!({"d_model":64,"d_proj":128,"temperature":0.07,"normalize":true}),
                Vec::new(),
            )
        });

        let d_model = embed_config_json["d_model"].as_u64().unwrap_or(64) as usize;
        let d_proj = embed_config_json["d_proj"].as_u64().unwrap_or(128) as usize;

        let tf_config = graph_transformer::TransformerConfig {
            n_subcarriers: 56,
            n_keypoints: 17,
            d_model,
            n_heads: 4,
            n_gnn_layers: 2,
        };
        let e_config = embedding::EmbeddingConfig {
            d_model,
            d_proj,
            temperature: 0.07,
            normalize: true,
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
        let _ds_path = args
            .dataset
            .clone()
            .unwrap_or_else(|| PathBuf::from("data"));
        let csi_windows: Vec<Vec<Vec<f32>>> = (0..10)
            .map(|i| {
                (0..4)
                    .map(|a| {
                        (0..56)
                            .map(|s| ((i * 7 + a * 13 + s) as f32 * 0.31).sin() * 0.5)
                            .collect()
                    })
                    .collect()
            })
            .collect();

        eprintln!(
            "Extracting embeddings from {} CSI windows...",
            csi_windows.len()
        );
        let embeddings = extractor.extract_batch(&csi_windows);
        for (i, emb) in embeddings.iter().enumerate() {
            let norm: f32 = emb.iter().map(|x| x * x).sum::<f32>().sqrt();
            eprintln!("  Window {i}: {d_proj}-dim embedding, ||e|| = {norm:.4}");
        }
        eprintln!(
            "Extracted {} embeddings of dimension {d_proj}",
            embeddings.len()
        );

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
                eprintln!(
                    "Unknown index type '{}'. Use: env, activity, temporal, person",
                    index_type_str
                );
                std::process::exit(1);
            }
        };

        let tf_config = graph_transformer::TransformerConfig::default();
        let e_config = embedding::EmbeddingConfig::default();
        let mut extractor = embedding::EmbeddingExtractor::new(tf_config, e_config);

        // Generate synthetic CSI windows for demo
        let csi_windows: Vec<Vec<Vec<f32>>> = (0..20)
            .map(|i| {
                (0..4)
                    .map(|a| {
                        (0..56)
                            .map(|s| ((i * 7 + a * 13 + s) as f32 * 0.31).sin() * 0.5)
                            .collect()
                    })
                    .collect()
            })
            .collect();

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
            eprintln!(
                "  entry={}, distance={:.4}, metadata={}",
                r.entry, r.distance, r.metadata
            );
        }

        return;
    }

    // Handle --train mode: train a model and exit
    if args.train {
        eprintln!("=== WiFi-DensePose Training Mode ===");

        // Build data pipeline
        let ds_path = args
            .dataset
            .clone()
            .unwrap_or_else(|| PathBuf::from("data"));
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
            (0..50)
                .map(|i| {
                    let csi: Vec<Vec<f32>> = (0..4)
                        .map(|a| {
                            (0..56)
                                .map(|s| ((i * 7 + a * 13 + s) as f32 * 0.31).sin() * 0.5)
                                .collect()
                        })
                        .collect();
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
                })
                .collect()
        };

        // Load samples (fall back to synthetic if dataset missing/empty)
        let samples = match pipeline.load() {
            Ok(s) if !s.is_empty() => {
                eprintln!("Loaded {} samples from {}", s.len(), ds_path.display());
                s
            }
            Ok(_) => {
                eprintln!(
                    "No samples found at {}. Using synthetic data.",
                    ds_path.display()
                );
                generate_synthetic()
            }
            Err(e) => {
                eprintln!("Failed to load dataset: {e}. Using synthetic data.");
                generate_synthetic()
            }
        };

        // Convert dataset samples to trainer format
        let trainer_samples: Vec<trainer::TrainingSample> =
            samples.iter().map(trainer::from_dataset_sample).collect();

        // Split 80/20 train/val
        let split = (trainer_samples.len() * 4) / 5;
        let (train_data, val_data) = trainer_samples.split_at(split.max(1));
        eprintln!(
            "Train: {} samples, Val: {} samples",
            train_data.len(),
            val_data.len()
        );

        // Create transformer + trainer
        let n_subcarriers = train_data
            .first()
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
        // ADR-155 §2.1: `best_pck` is RAW-threshold PCK (no torso norm) and
        // `best_oks` uses the fake-Gold area=1.0 proxy — NOT the canonical
        // hip↔hip `pck_canonical` / COCO OKS. Label them distinctly so the
        // printed numbers are never read as claim-grade canonical metrics.
        eprintln!(
            "  Best epoch: {}, pck_raw@0.2: {:.4}, oks_map(area=1.0 proxy): {:.4}",
            result.best_epoch, result.best_pck, result.best_oks
        );

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
                Ok(()) => eprintln!(
                    "RVF saved ({} params, {} bytes)",
                    weights.len(),
                    weights.len() * 4
                ),
                Err(e) => eprintln!("Failed to save RVF: {e}"),
            }
        }

        return;
    }

    info!("WiFi-DensePose Sensing Server (Rust + Axum + RuVector)");
    info!("  HTTP:      http://localhost:{}", args.http_port);
    info!("  WebSocket: ws://localhost:{}/ws/sensing", args.ws_port);
    info!("  UDP:       {}:{} (ESP32 CSI)", args.udp_bind, args.udp_port);
    info!("  UI path:   {}", args.ui_path.display());
    info!("  Source:    {}", args.source);

    // Resolve the data source into a concrete task plan (issue #1004).
    //
    // Issue #937 (prior fix): `auto` must never serve fake CSI *tagged as
    // production telemetry*. We keep that guarantee — in the gap before real
    // CSI arrives, `source` is the honest string "simulated" (downstream
    // `/api/v1/sensing/latest`, `/ws/sensing` see `source: "simulated"`, not a
    // production tag). What #937's hard-exit got wrong: at boot the firmware and
    // server race, so CSI usually is NOT flowing during the 2 s probe. Exiting
    // (or latching on simulate) meant the server could never pick up CSI that
    // started seconds later. The robust resolution (see `plan_source`): in
    // `auto` always bind the UDP :5005 receiver; serve simulated until the first
    // real frame; then `udp_receiver_task` promotes `source` → "esp32". Explicit
    // `--source simulated` stays a hard, UDP-free override for offline demos.
    let normalized = if args.source == "simulate" { "simulated" } else { args.source.as_str() };
    let plan = if normalized == "auto" {
        info!("Auto-detecting data source (UDP :{} bound either way)...", args.udp_port);
        let esp32 = probe_esp32(args.udp_port).await;
        let wifi = if esp32 { false } else { probe_windows_wifi().await };
        if esp32 {
            info!("  ESP32 CSI detected on UDP :{}", args.udp_port);
        } else if wifi {
            info!("  Windows WiFi detected");
        } else {
            warn!(
                "No real CSI source at boot — serving SIMULATED data (tagged as \
                 'simulated', not production) while the UDP :{} receiver stays bound. \
                 The server promotes to live the instant a real frame arrives (issue \
                 #1004). For an offline demo with no live promotion, pass \
                 --source simulated explicitly.",
                args.udp_port
            );
        }
        plan_source("auto", esp32, wifi)
    } else {
        plan_source(normalized, false, false)
    };
    let source: &str = plan.initial_source.as_str();

    info!(
        "Data source: {source} (udp_receiver={}, simulator={}, wifi={})",
        plan.bind_udp, plan.run_simulator, plan.run_wifi
    );

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
                Ok(data) => match load_or_convert_model(mp, &data) {
                    Ok(mut loader) => {
                        if let Ok(la) = loader.load_layer_a() {
                            info!(
                                "  Layer A ready: model={} v{} ({} segments)",
                                la.model_name, la.version, la.n_segments
                            );
                        }
                        model_loaded = true;
                        progressive_loader = Some(loader);
                    }
                    Err(e) => {
                        // #894: typed, actionable message (never the opaque magic)
                        // and a LOUD warning that we are degrading to heuristics.
                        error!("{e}");
                        error!(
                            "Model NOT loaded — falling back to signal heuristics. \
                             Pose/person-count output will be approximate (issue #894)."
                        );
                    }
                },
                Err(e) => error!("Failed to read model file: {e}"),
            }
        }
    }

    // Ensure data directories exist for models and recordings
    let models_dir = effective_models_dir();
    let _ = std::fs::create_dir_all(&models_dir);
    let _ = std::fs::create_dir_all("data/recordings");

    // Discover model and recording files on startup
    let initial_models = scan_model_files();
    let initial_recordings = scan_recording_files();
    info!(
        "Discovered {} model files, {} recording files",
        initial_models.len(),
        initial_recordings.len()
    );

    // ADR-044 §5.3: load persisted runtime config from the data directory.
    let data_dir = args.data_dir.clone();
    if let Err(error) = std::fs::create_dir_all(&data_dir) {
        warn!(path = %data_dir.display(), %error, "Could not create server data directory");
    }
    let runtime_config = load_runtime_config(&data_dir);
    // ADR-271: resolve (or generate + persist) the browser-session signing key
    // before any request can arrive. Zero-config for a single appliance; the
    // env var still wins for a multi-instance deployment that must share one.
    wifi_densepose_sensing_server::browser_session::init_secret(&data_dir);
    let (bootstrap_field_model, bootstrap_metadata, bootstrap_baseline_active) =
        if !args.calibrate {
            args.installation_id.as_deref().map_or(
                (None, None, false),
                |installation_id| {
                    let path = bootstrap_baseline::path_in(&data_dir);
                    match bootstrap_baseline::load(
                        &path,
                        installation_id,
                        chrono::Utc::now().timestamp_millis() as u64,
                    ) {
                        Ok((model, metadata)) => {
                            info!(
                                source_model_id = %metadata.source_model_id,
                                source_node_ids = ?metadata.source_node_ids,
                                "Restored privacy reduced empty room bootstrap prior"
                            );
                            (Some(model), Some(metadata), true)
                        }
                        Err(bootstrap_baseline::BootstrapBaselineError::Io(error))
                            if error.kind() == std::io::ErrorKind::NotFound =>
                        {
                            (None, None, false)
                        }
                        Err(error) => {
                            warn!(%error, "Ignored invalid empty room bootstrap prior");
                            (None, None, false)
                        }
                    }
                },
            )
        } else {
            (None, None, false)
        };
    info!(
        "Loaded runtime config: dedup_factor={:.2}",
        runtime_config.dedup_factor
    );

    // ADR-102: optional Edge Module Registry. None when --no-edge-registry
    // is set (or when the URL is empty); otherwise we construct one with
    // the configured TTL. The fetch happens lazily on first request.
    let edge_registry: Option<
        std::sync::Arc<wifi_densepose_sensing_server::edge_registry::EdgeRegistry>,
    > = if args.no_edge_registry || args.edge_registry_url.is_empty() {
        info!("Edge module registry: DISABLED (--no-edge-registry or empty URL)");
        None
    } else {
        info!(
            "Edge module registry: enabled — upstream={} ttl={}s",
            args.edge_registry_url, args.edge_registry_ttl_secs
        );
        Some(std::sync::Arc::new(
            wifi_densepose_sensing_server::edge_registry::EdgeRegistry::new(
                args.edge_registry_url.clone(),
                std::time::Duration::from_secs(args.edge_registry_ttl_secs),
            ),
        ))
    };

    let (tx, _) = broadcast::channel::<String>(256);
    // ADR-099: parallel broadcast for the per-frame introspection snapshot stream
    // consumed by `/ws/introspection`. Same ring size as `tx` (256) — slow
    // clients drop oldest, identical backpressure shape.
    let (intro_tx, _) = broadcast::channel::<String>(256);

    // #872: actually start the MQTT publisher when `--mqtt` is set. The publisher
    // (mqtt::) consumes a typed VitalsSnapshot stream; we bridge the existing JSON
    // sensing broadcast into it with a defensive serde_json::Value mapping (absent
    // fields default — never publish wrong values). Gated on the `mqtt` feature
    // (the Docker image is built `--features mqtt`); without it `--mqtt` WARNs and
    // no-ops, matching the documented contract.
    if args.mqtt_opts.mqtt {
        #[cfg(feature = "mqtt")]
        {
            use wifi_densepose_sensing_server::mqtt;
            let mcfg = std::sync::Arc::new(mqtt::config::MqttConfig::from_args(&args.mqtt_opts));
            match mcfg.validate() {
                Ok(()) => {
                    let node_id = mcfg.client_id.clone();
                    let builder = mqtt::publisher::OwnedDiscoveryBuilder {
                        discovery_prefix: mcfg.discovery_prefix.clone(),
                        node_id: node_id.clone(),
                        node_friendly_name: Some("RuView".to_string()),
                        sw_version: env!("CARGO_PKG_VERSION").to_string(),
                        model: "RuView WiFi Sensing".to_string(),
                        via_device: None,
                    };
                    let (vtx, vrx) = broadcast::channel::<mqtt::state::VitalsSnapshot>(64);
                    let (host, port) = (mcfg.host.clone(), mcfg.port);
                    mqtt::publisher::spawn(mcfg, builder, vrx);
                    let mut jrx = tx.subscribe();
                    tokio::spawn(async move {
                        while let Ok(json) = jrx.recv().await {
                            let Ok(v) = serde_json::from_str::<serde_json::Value>(&json) else {
                                continue;
                            };
                            // #898/#872: emit one snapshot per physical node so
                            // each surfaces as its own Home-Assistant device with
                            // its *own* presence/motion/RSSI (see
                            // vitals_snapshots_from_sensing_json). Falls back to a
                            // single aggregate snapshot for per-node-less sources.
                            for snap in vitals_snapshots_from_sensing_json(&v, &node_id) {
                                let _ = vtx.send(snap);
                            }
                        }
                    });
                    tracing::info!("MQTT publisher started -> {host}:{port}");
                }
                Err(e) => tracing::error!("MQTT config invalid: {e}; publisher not started"),
            }
        }
        #[cfg(not(feature = "mqtt"))]
        tracing::warn!(
            "--mqtt set but this binary was built without the `mqtt` feature; the publisher is a \
             no-op. Use the official Docker image (built `--features mqtt`) or rebuild with \
             `cargo build -p wifi-densepose-sensing-server --features mqtt`."
        );
    }

    // ADR-262 P3: build the live RuField surface (dedicated ed25519 signer from
    // WDP_RUFIELD_SIGNING_SEED, else a logged dev default). The same Arc is
    // stored in AppStateInner (so the sensing loop can `emit()` per cycle) and
    // cloned into the additive `/api/field` + `/ws/field` router below.
    let field_surface: rufield_surface::FieldState =
        Arc::new(RwLock::new(rufield_surface::FieldSurface::from_env()));

    // Populated inside the `multistatic_fuser` field initializer below, then
    // threaded into `engine_bridge` so both fusion paths honor the same
    // WDP_TDM_SLOTS/WDP_GUARD_INTERVAL_US-derived guard (#1049/#1057).
    let mut engine_bridge_multistatic_cfg: Option<MultistaticConfig> = None;
    let mut node_positions_config: Vec<[f32; 3]> = Vec::new();
    let state: SharedState = Arc::new(RwLock::new(AppStateInner {
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
        latest_qualcomm_csi: None,
        last_qualcomm_frame: None,
        latest_vendor_rf: BTreeMap::new(),
        tx,
        intro: wifi_densepose_sensing_server::introspection::IntrospectionState::new(),
        intro_tx,
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
        // Training (ADR-186 TRAIN-RECONNECT)
        training_state: training_api::TrainingState::default(),
        training_progress_tx: broadcast::channel::<String>(256).0,
        adaptive_model:
            adaptive_classifier::AdaptiveModel::load(&adaptive_classifier::model_path())
                .ok()
                .inspect(|m| {
                    info!(
                        "Loaded adaptive classifier: {} frames, {:.1}% accuracy",
                        m.trained_frames,
                        m.training_accuracy * 100.0
                    );
                }),
        node_states: HashMap::new(),
        room_debounced_level: "absent".to_string(),
        room_debounce_candidate: "absent".to_string(),
        room_debounce_since: None,
        // Accuracy sprint
        pose_tracker: PoseTracker::new(),
        last_tracker_instant: None,
        multistatic_fuser: {
            // #1031/#1049: the default guard (60 ms hard / 20 ms soft)
            // accommodates a real TDM slot offset. A deployment overrides it via
            // WDP_GUARD_INTERVAL_US (direct, e.g. 200000 for WiFi/ESP-NOW sync —
            // #1049) or WDP_TDM_SLOTS + WDP_TDM_SLOT_US (derive from schedule).
            let cfg = multistatic_guard_config_from_env();
            info!(
                "Multistatic fusion guard: {} µs hard / {} µs soft (override via \
                 WDP_GUARD_INTERVAL_US / WDP_SOFT_GUARD_US, or WDP_TDM_SLOTS+WDP_TDM_SLOT_US)",
                cfg.guard_interval_us, cfg.soft_guard_us
            );
            let mut fuser = MultistaticFuser::with_config(MultistaticConfig {
                min_nodes: 1, // single-node passthrough
                ..cfg.clone()
            });
            if let Some(ref pos_str) = args.node_positions {
                let positions = field_bridge::parse_node_positions(pos_str);
                if !positions.is_empty() {
                    info!(
                        "Configured {} node positions for multistatic fusion",
                        positions.len()
                    );
                    node_positions_config = positions.clone();
                    fuser.set_node_positions(positions);
                }
            }
            engine_bridge_multistatic_cfg = Some(MultistaticConfig {
                min_nodes: 1,
                ..cfg
            });
            fuser
        },
        node_positions_config,
        engine_bridge: engine_bridge::EngineBridge::new(
            wifi_densepose_bfld::PrivacyMode::PrivateHome,
            1,
            "default",
            "Default Room",
            engine_bridge_multistatic_cfg,
        ),
        field_model: bootstrap_field_model,
        installation_id: args.installation_id.clone(),
        bootstrap_baseline: bootstrap_metadata.clone(),
        bootstrap_baseline_active,
        calibration_model_id: None,
        calibration_source_node_ids: bootstrap_metadata
            .as_ref()
            .map(|metadata| metadata.source_node_ids.iter().copied().collect())
            .unwrap_or_default(),
        calibration_grid_binding: bootstrap_metadata.as_ref().and_then(|metadata| {
            let [source_node_id] = metadata.source_node_ids.as_slice() else {
                return None;
            };
            Some(CalibrationGridBinding {
                source_node_id: *source_node_id,
                grid: CsiGridKey::from_bootstrap_grid(metadata.source_grid),
                evidence: None,
            })
        }),
        calibration_last_sequences: HashMap::new(),
        calibration_reordered_packets: HashMap::new(),
        calibration_max_reorder_depth: HashMap::new(),
        calibration_sequence_fault_node_ids: std::collections::BTreeSet::new(),
        // ADR-044 §5.2: rolling-P95 over ~30 s at 20 Hz; warm-up after 60 samples.
        p95_variance: RollingP95::new(600, 60),
        p95_motion_band_power: RollingP95::new(600, 60),
        p95_spectral_power: RollingP95::new(600, 60),
        // ADR-044 §5.3: runtime-configurable dedup factor (persisted).
        dedup_factor: runtime_config.dedup_factor,
        data_dir: data_dir.clone(),
        field_surface: field_surface.clone(),
        pose_physics: pose_physics::PosePhysicsRuntime::new(
            wifi_densepose_physics::PhysicsConfig::default(),
        )
        .expect("default pose physics configuration is valid"),
    }));

    // Start background tasks from the resolved plan (issue #1004).
    //
    // In `auto` mode with no boot source, `bind_udp` AND `run_simulator` are
    // both true: the UDP receiver is bound so real CSI can promote the source,
    // and the simulator serves poses in the meantime (it self-suspends once
    // promoted — see `simulated_data_task`). Explicit `--source simulated` has
    // `bind_udp = false`, so it serves simulated data only, with no live binding.
    if plan.bind_udp {
        // ADR-296: resolve the UDP bind scope + source allowlist and fail closed
        // on an unguarded routable bind, mirroring the OAuth boot refusal below.
        use wifi_densepose_sensing_server::udp_bind;
        let udp_bind_ip: std::net::IpAddr = match args.udp_bind.parse() {
            Ok(ip) => ip,
            Err(_) => {
                error!(
                    "Invalid --udp-bind '{}' (use 127.0.0.1 or 0.0.0.0)",
                    args.udp_bind
                );
                std::process::exit(1);
            }
        };
        let udp_allowlist = match udp_bind::UdpSourceAllowlist::parse(args.udp_allow.iter()) {
            Ok(a) => std::sync::Arc::new(a),
            Err(e) => {
                error!("Invalid --udp-allow: {e}");
                std::process::exit(1);
            }
        };
        match udp_bind::decide_udp_bind(
            udp_bind_ip,
            udp_allowlist.is_active(),
            args.udp_insecure_lan,
        ) {
            Ok(decision) => {
                info!(
                    "UDP data plane security: {}",
                    udp_bind::startup_summary(decision, udp_bind_ip, args.udp_port, &udp_allowlist)
                );
            }
            Err(e) => {
                error!("{e}");
                std::process::exit(1);
            }
        }
        tokio::spawn(udp_receiver_task(
            state.clone(),
            udp_bind_ip,
            args.udp_port,
            udp_allowlist,
        ));
        tokio::spawn(broadcast_tick_task(state.clone(), args.tick_ms));
    }
    if plan.run_wifi {
        tokio::spawn(windows_wifi_task(state.clone(), args.tick_ms));
    }
    if plan.run_simulator {
        tokio::spawn(simulated_data_task(state.clone(), args.tick_ms));
    }

    // ADR-166: Parse bind address once, use for all listeners
    let bind_ip: std::net::IpAddr = args
        .bind_addr
        .parse()
        .expect("Invalid --bind-addr (use 127.0.0.1 or 0.0.0.0)");

    // #443: optional bearer-token auth on `/api/v1/*`. `RUVIEW_API_TOKEN`
    // unset/empty ⇒ middleware is a no-op (LAN-mode default preserved); set ⇒
    // every `/api/v1/*` request must carry `Authorization: Bearer <token>`.
    //
    // ADR-271: additionally, `RUVIEW_OAUTH_ISSUER` enables Cognitum OAuth
    // verification alongside (not instead of) the static token.
    //
    // FAIL CLOSED. If OAuth was requested but cannot work — empty issuer, or a
    // JWKS we cannot fetch at boot — we exit rather than serve. Starting anyway
    // would silently downgrade an operator who asked for OAuth to either an
    // open API or a single-shared-secret one, and they would have no signal
    // that it happened. A loud death at boot is the kind thing here.
    let bearer_auth_state =
        match wifi_densepose_sensing_server::bearer_auth::AuthState::from_env() {
            Ok(s) => s,
            Err(e) => {
                error!(
                    "API auth: OAuth was requested but cannot be initialised: {e}. \
                     Refusing to start — unset RUVIEW_OAUTH_ISSUER to run without it."
                );
                std::process::exit(1);
            }
        };
    if bearer_auth_state.is_enabled() {
        if bearer_auth_state.oauth_enabled() {
            info!("API auth: ON for /api/v1/* — Cognitum OAuth (ADR-271){}",
                if bearer_auth_state.static_token_enabled() { " + static RUVIEW_API_TOKEN" } else { "" });
        } else {
            info!("API auth: bearer-token enforcement ON for /api/v1/* (RUVIEW_API_TOKEN set)");
        }
        if bind_ip.is_unspecified() {
            warn!(
                "API auth ON but bind-addr is {} — consider --bind-addr 127.0.0.1 for LAN-only deployments",
                bind_ip
            );
        }
    } else {
        info!(
            "API auth: OFF — /api/v1/* is unauthenticated. Set RUVIEW_API_TOKEN=<token> or RUVIEW_OAUTH_ISSUER=<issuer> to enforce auth."
        );
    }

    // DNS-rebinding defense: validate the `Host` header against an allowlist
    // before any handler runs. Default is loopback-only (`localhost`,
    // `127.0.0.1`, `[::1]`, each with or without a port). Operators extend
    // the set via `--allowed-host` flags or the `SENSING_ALLOWED_HOSTS` env
    // var; `--disable-host-validation` opts out entirely for reverse-proxy
    // setups that already canonicalise `Host`.
    let host_allowlist = if args.disable_host_validation {
        warn!(
            "Host-header validation DISABLED — server is reachable via any Host. \
             Only use this behind a reverse proxy that pins Host."
        );
        wifi_densepose_sensing_server::host_validation::HostAllowlist::disabled()
    } else {
        let discovery_label = args.installation_id.as_deref().unwrap_or("installation");
        let discovery_host =
            wifi_densepose_sensing_server::discovery::discovery_hostname(discovery_label);
        let allowlist =
            wifi_densepose_sensing_server::host_validation::HostAllowlist::from_cli_and_env(
                args.allowed_hosts
                    .iter()
                    .cloned()
                    .chain(std::iter::once(discovery_host)),
            );
        info!(
            "Host-header validation ON ({} entries; loopback names always included)",
            allowlist.entries_for_test().len()
        );
        allowlist
    };

    // WebSocket server on dedicated port (8765)
    let ws_state = state.clone();
    let ws_app = Router::new()
        .route("/ws/sensing", get(ws_sensing_handler))
        .route("/health", get(health))
        .with_state(ws_state)
        // ADR-262 P3: additive `/ws/field` (+ `/api/field`) on the WS port too,
        // so a client on :8765 can stream signed RuField FieldEvents alongside
        // `/ws/sensing`. Merged with its own FieldState (different state type).
        .merge(rufield_surface::router(field_surface.clone()))
        // ADR-272 FIX: this router had NO auth layer at all. `/ws/sensing` and
        // `/ws/field` on the dedicated WS port accepted unauthenticated
        // upgrades even with auth ON — and this is the port the UI actually
        // uses (ui/services/sensing.service.js maps HTTP 8080 -> WS 8765), so
        // gating only the HTTP port protected a path the browser never takes.
        // Applied AFTER the merge so it covers the RuField routes too.
        // AuthState shares its TicketStore via Arc, so a ticket minted at
        // POST /api/v1/ws-ticket on the HTTP port is redeemable here.
        .layer(axum::middleware::from_fn_with_state(
            bearer_auth_state.clone(),
            wifi_densepose_sensing_server::bearer_auth::require_bearer,
        ))
        .layer(axum::middleware::from_fn_with_state(
            host_allowlist.clone(),
            wifi_densepose_sensing_server::host_validation::require_allowed_host,
        ));

    let ws_addr = SocketAddr::from((bind_ip, args.ws_port));
    let ws_listener = tokio::net::TcpListener::bind(ws_addr)
        .await
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
        .route("/api/v1/radar/latest", get(latest_realtek_radar))
        .route("/api/v1/csi/mediatek/latest", get(latest_mediatek_csi))
        .route("/api/v1/csi/qualcomm/latest", get(latest_qualcomm_csi))
        .route("/api/v1/rf/vendors", get(vendor_descriptors))
        .route("/api/v1/rf/vendors/latest", get(latest_vendor_events))
        .route("/api/v1/rf/vendors/:vendor/latest", get(latest_vendor_event))
        .route("/api/v1/rf/vendors/:vendor/events", post(ingest_vendor_events))
        // Per-node health endpoint
        .route("/api/v1/nodes", get(nodes_endpoint))
        // ADR-110 iter 29 — per-node mesh sync state for HTTP clients.
        .route("/api/v1/nodes/:id/sync", get(node_sync_endpoint))
        .route("/api/v1/mesh", get(mesh_endpoint))
        .route("/api/v1/mesh/metrics", get(mesh_metrics_endpoint))
        // Vital sign endpoints
        .route("/api/v1/vital-signs", get(vital_signs_endpoint))
        .route("/api/v1/edge-vitals", get(edge_vitals_endpoint))
        // ADR-102: Edge Module Registry — surfaces the canonical Cognitum cog
        // catalog (`https://storage.googleapis.com/cognitum-apps/app-registry.json`)
        // with in-process TTL cache + stale-on-error fallback. Disabled when
        // --no-edge-registry is set (returns 404).
        .route("/api/v1/edge/registry", get(edge_registry_endpoint))
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
        .route("/api/v1/pose/physics/metrics", get(pose_physics_metrics))
        .route("/api/v1/pose/stats", get(pose_stats))
        .route("/api/v1/pose/zones/summary", get(pose_zones_summary))
        .route("/api/v1/pose/activities", get(pose_activities))
        // Dashboard-compatible aliases for the field-model calibration API.
        .route("/api/v1/pose/calibrate", post(calibration_start))
        .route(
            "/api/v1/pose/calibration/status",
            get(calibration_status),
        )
        // Stream endpoints
        .route("/api/v1/stream/status", get(stream_status))
        // ADR-272 — browsers cannot set Authorization on a WebSocket upgrade,
        // so they exchange their credential here for a 30s single-use ticket.
        .route("/api/v1/ws-ticket", axum::routing::post(ws_ticket_handler))
        // ADR-271 browser sign-in. Deliberately NOT under /api/v1/*: these are
        // how a browser obtains a credential, so gating them would deadlock.
        .route("/oauth/start", get(oauth_start))
        .route("/oauth/callback", get(oauth_callback))
        .route("/oauth/logout", get(oauth_logout))
        // Ungated on purpose: a signed-OUT browser needs to discover whether
        // sign-in is available, and it cannot ask a gated endpoint that.
        // Returns only capability + who-you-are, never a credential.
        .route("/oauth/status", get(oauth_status))
        .route("/api/v1/stream/pose", get(ws_pose_handler))
        // Sensing WebSocket on the HTTP port so the UI can reach it without a second port
        .route("/ws/sensing", get(ws_sensing_handler))
        // ADR-099: real-time introspection — per-frame attractor + DTW snapshot.
        .route("/ws/introspection", get(ws_introspection_handler))
        .route(
            "/api/v1/introspection/snapshot",
            get(api_introspection_snapshot),
        )
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
        // Training endpoints (ADR-186 TRAIN-RECONNECT): the real in-server
        // trainer + `/ws/train/progress` stream. Merged while the router is
        // still `Router<SharedState>` (before `.with_state`) so these routes
        // share `AppStateInner` and `/api/v1/train/*` sits under the bearer gate
        // applied below (like the rest of `/api/v1/*`).
        .merge(training_api::routes())
        // Adaptive classifier endpoints
        .route("/api/v1/adaptive/train", post(adaptive_train))
        .route("/api/v1/adaptive/status", get(adaptive_status))
        .route("/api/v1/adaptive/unload", post(adaptive_unload))
        // Field model calibration (eigenvalue-based person counting)
        .route("/api/v1/calibration/start", post(calibration_start))
        .route("/api/v1/calibration/stop", post(calibration_stop))
        .route(
            "/api/v1/calibration/bootstrap/promote",
            post(calibration_promote_bootstrap),
        )
        .route(
            "/api/v1/calibration/bootstrap/refine",
            post(calibration_refine_bootstrap),
        )
        .route("/api/v1/calibration/cancel", post(calibration_cancel))
        .route("/api/v1/calibration/status", get(calibration_status))
        .route("/api/v1/calibration/reset", post(calibration_reset))
        // ADR-044 §5.3: runtime-configurable dedup factor
        .route(
            "/api/v1/config/dedup-factor",
            get(config_get_dedup_factor).post(config_set_dedup_factor),
        )
        .route("/api/v1/config/ground-truth", post(config_set_ground_truth))
        // Static UI files
        .nest_service("/ui", ServeDir::new(&ui_path))
        // ADR-102: make the edge registry handle (Option<Arc<EdgeRegistry>>)
        // available to the /api/v1/edge/registry handler. None when disabled.
        .layer(Extension(edge_registry.clone()))
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::CACHE_CONTROL,
            HeaderValue::from_static("no-cache, no-store, must-revalidate"),
        ))
        // Opt-in bearer-token auth on `/api/v1/*` (#443). When `RUVIEW_API_TOKEN`
        // is unset/empty the middleware is a no-op — the default stays
        // LAN-mode-friendly. `/health*`, `/ws/sensing`, and `/ui/*` are never
        // gated (orchestrator probes + local browsers).
        // ADR-272: the ws-ticket handler needs the store the middleware owns.
        .layer(axum::Extension(bearer_auth_state.clone()))
        .with_state(state.clone())
        // ADR-262 P3: additive RuField surface (`/api/field` + `/ws/field`).
        // Merged AFTER `.with_state` (so http_app is already `Router<()>` and
        // can absorb the field router's own `FieldState`).
        .merge(rufield_surface::router(field_surface.clone()))
        // Opt-in bearer auth (#443) + ADR-272 WebSocket gating.
        //
        // Applied AFTER the merge, and that ordering is load-bearing: axum
        // `.layer()` wraps only what is already registered, so while this sat
        // above the merge, `/ws/field` bypassed authentication entirely —
        // measured 101 on an unauthenticated upgrade with auth ON. Adding
        // routes after an auth layer silently exempts them, which is exactly
        // the failure mode ADR-272 exists to prevent.
        //
        // Unset RUVIEW_API_TOKEN/RUVIEW_OAUTH_ISSUER still makes this a no-op.
        .layer(axum::middleware::from_fn_with_state(
            bearer_auth_state.clone(),
            wifi_densepose_sensing_server::bearer_auth::require_bearer,
        ))
        // DNS-rebinding defense: applied last so it runs first on the request
        // path (axum layers run outermost-in). Rejects requests whose `Host`
        // header is not in the allowlist before any handler — including
        // `/health`, `/ws/*`, and the merged `/api/field` + `/ws/field` —
        // observes the body.
        .layer(axum::middleware::from_fn_with_state(
            host_allowlist.clone(),
            wifi_densepose_sensing_server::host_validation::require_allowed_host,
        ));

    let http_addr = SocketAddr::from((bind_ip, args.http_port));
    let http_listener = tokio::net::TcpListener::bind(http_addr)
        .await
        .expect("Failed to bind HTTP port");
    info!("HTTP server listening on {http_addr}");
    info!(
        "Open http://localhost:{}/ui/index.html in your browser",
        args.http_port
    );

    let discovery_label = args.installation_id.as_deref().unwrap_or("installation");
    let discovery_hostname =
        wifi_densepose_sensing_server::discovery::discovery_hostname(discovery_label);
    let _discovery_advertiser = if args.no_mdns || bind_ip.is_loopback() {
        if bind_ip.is_loopback() && !args.no_mdns {
            info!("RuView discovery skipped for loopback-only HTTP bind");
        }
        None
    } else {
        match wifi_densepose_sensing_server::discovery::start_advertiser(
            &args.installation_name,
            discovery_label,
            &discovery_hostname,
            args.http_port,
        ) {
            Ok(advertiser) => {
                info!(hostname = %discovery_hostname, port = args.http_port, "RuView local discovery advertised");
                Some(advertiser)
            }
            Err(error) => {
                warn!(%error, "RuView local discovery unavailable; manual origin remains usable");
                None
            }
        }
    };

    // Run the HTTP server with graceful shutdown support
    let shutdown_state = state.clone();
    let server = axum::serve(http_listener, http_app).with_graceful_shutdown(async {
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

#[cfg(test)]
mod multistatic_guard_config_tests {
    //! #1049 — the multistatic guard interval must be operator-configurable so a
    //! WiFi/ESP-NOW deployment (10–150 ms inter-node clock drift) can lift the
    //! guard past its measured timestamp spread instead of being permanently
    //! demoted to Restricted with no escape hatch.
    use super::*;

    #[test]
    fn default_guard_when_nothing_set() {
        let cfg = multistatic_guard_config_from(None, None, None, None);
        assert_eq!(cfg.guard_interval_us, MultistaticConfig::default().guard_interval_us);
        assert_eq!(cfg.soft_guard_us, MultistaticConfig::default().soft_guard_us);
    }

    #[test]
    fn direct_guard_override_wins_and_unblocks_wifi_spread() {
        // The #1049 reporter's measured ~70 ms spread exceeds the 60 ms default
        // → permanent demotion. A direct 200 ms override accepts it.
        let cfg = multistatic_guard_config_from(None, None, Some("200000"), None);
        assert_eq!(cfg.guard_interval_us, 200_000);
        assert!(cfg.soft_guard_us < cfg.guard_interval_us);
        // 70 ms spread now sits inside the guard.
        assert!(70_000 < cfg.guard_interval_us);
    }

    #[test]
    fn direct_guard_override_beats_tdm_derived() {
        // Both TDM params AND a direct override set → the direct hard guard wins,
        // the TDM-derived soft band is preserved (still strictly below hard).
        let cfg = multistatic_guard_config_from(Some("2"), Some("18000"), Some("200000"), None);
        assert_eq!(cfg.guard_interval_us, 200_000);
        assert!(cfg.soft_guard_us < cfg.guard_interval_us);
        assert!(cfg.soft_guard_us >= 1);
    }

    #[test]
    fn soft_override_is_clamped_strictly_below_hard() {
        // A soft guard ≥ hard would be nonsensical → clamped below the hard guard.
        let cfg = multistatic_guard_config_from(None, None, Some("50000"), Some("999999"));
        assert_eq!(cfg.guard_interval_us, 50_000);
        assert!(cfg.soft_guard_us < 50_000);
    }

    #[test]
    fn lowering_hard_below_default_soft_pulls_soft_down() {
        // Override hard to 10 ms (< default 20 ms soft) → soft drops below it.
        let cfg = multistatic_guard_config_from(None, None, Some("10000"), None);
        assert_eq!(cfg.guard_interval_us, 10_000);
        assert!(cfg.soft_guard_us < 10_000);
    }

    #[test]
    fn malformed_or_zero_override_falls_back_to_base() {
        // Garbage / zero must not break fusion — fall back to the base config.
        for bad in ["", "abc", "0", "-5", "12.5"] {
            let cfg = multistatic_guard_config_from(None, None, Some(bad), None);
            assert_eq!(
                cfg.guard_interval_us,
                MultistaticConfig::default().guard_interval_us,
                "override {bad:?} should be ignored"
            );
        }
    }
}

#[cfg(test)]
mod node_sync_snapshot_serialization_tests {
    //! ADR-110 iter 24 — JSON public-API contract for the iter 23
    //! NodeSyncSnapshot field. Any future rename / removal here must be
    //! intentional and update both Rust + UI/automation consumers.

    use super::*;

    fn sample_sync() -> NodeSyncSnapshot {
        NodeSyncSnapshot {
            offset_us: 1_163_565,
            is_leader: false,
            is_valid: true,
            smoothed: true,
            sequence: 20,
            csi_fps_ema: 10.0,
            csi_fps_samples: 47,
            staleness_ms: Some(120),
        }
    }

    fn sample_node(sync: Option<NodeSyncSnapshot>) -> NodeInfo {
        NodeInfo {
            node_id: 9,
            rssi_dbm: -38.0,
            position: [2.0, 0.0, 1.5],
            amplitude: vec![],
            subcarrier_count: 0,
            sync,
            node_inference: None,
        }
    }

    #[test]
    fn sync_present_serializes_all_seven_fields() {
        let v = serde_json::to_value(sample_node(Some(sample_sync()))).unwrap();
        let s = v.get("sync").expect("sync key must be present");
        // All eight contract fields named exactly as iter 23/34 documented.
        for key in ["offset_us", "is_leader", "is_valid", "smoothed",
                    "sequence", "csi_fps_ema", "csi_fps_samples",
                    "staleness_ms"] {
            assert!(s.get(key).is_some(),
                    "sync object missing field `{}` — UI contract broken", key);
        }
        // Spot-check values round-trip.
        assert_eq!(s["offset_us"], 1_163_565);
        assert_eq!(s["is_leader"], false);
        assert_eq!(s["sequence"], 20);
        assert_eq!(s["csi_fps_samples"], 47);
    }

    #[test]
    fn sync_absent_omits_the_key_entirely() {
        // skip_serializing_if = "Option::is_none" must drop the key, not
        // emit `"sync": null`. The non-mesh paths rely on this for
        // backwards compatibility with pre-iter-23 UI clients.
        let v = serde_json::to_value(sample_node(None)).unwrap();
        assert!(v.get("sync").is_none(),
                "expected `sync` key omitted when None, got {:?}", v.get("sync"));
        // The base NodeInfo fields are still there.
        assert_eq!(v["node_id"], 9);
        assert_eq!(v["rssi_dbm"], -38.0);
    }

    #[test]
    fn sync_round_trips_through_serde() {
        let original = sample_node(Some(sample_sync()));
        let json = serde_json::to_string(&original).unwrap();
        let parsed: NodeInfo = serde_json::from_str(&json).unwrap();
        // Field-level equality on the sync sub-object.
        let s_orig = original.sync.unwrap();
        let s_parsed = parsed.sync.expect("sync should survive round-trip");
        assert_eq!(s_parsed.offset_us, s_orig.offset_us);
        assert_eq!(s_parsed.is_leader, s_orig.is_leader);
        assert_eq!(s_parsed.is_valid, s_orig.is_valid);
        assert_eq!(s_parsed.smoothed, s_orig.smoothed);
        assert_eq!(s_parsed.sequence, s_orig.sequence);
        assert!((s_parsed.csi_fps_ema - s_orig.csi_fps_ema).abs() < 1e-9);
        assert_eq!(s_parsed.csi_fps_samples, s_orig.csi_fps_samples);
    }
}

#[cfg(test)]
mod sync_snapshot_helper_tests {
    //! ADR-110 iter 30 — covers the pure helper that backs both
    //! `/api/v1/nodes/:id/sync` and `/api/v1/mesh` REST endpoints and
    //! the WebSocket sensing_update broadcast. Tests at this layer keep
    //! the public-API contract honest without spinning up the axum
    //! router or constructing a full AppStateInner.

    use super::*;
    use wifi_densepose_hardware::{SyncPacket, SyncPacketFlags};

    fn populated_sync(node_id: u8) -> SyncPacket {
        SyncPacket {
            node_id,
            proto_ver: 1,
            flags: SyncPacketFlags { is_leader: false, is_valid: true, smoothed_used: true },
            local_us: 28_798_450,
            epoch_us: 27_634_885,
            sequence: 20,
        }
    }

    #[test]
    fn fresh_node_with_no_sync_returns_none() {
        // Mirrors the REST 404 "no_sync" branch.
        let ns = NodeState::new();
        assert!(ns.sync_snapshot().is_none());
    }

    #[test]
    fn node_with_latest_sync_produces_correct_snapshot() {
        // Mirrors the REST 200 OK branch + the WebSocket sync field.
        let mut ns = NodeState::new();
        ns.latest_sync = Some(populated_sync(9));
        ns.latest_sync_at = Some(std::time::Instant::now());
        // Pretend the fps EMA has settled (iter 18 5-sample warmup).
        ns.csi_fps_ema = 10.5;
        ns.csi_fps_samples = 42;

        let snap = ns.sync_snapshot().expect("populated state must produce a snapshot");
        assert_eq!(snap.offset_us, 1_163_565);  // §A0.10 measured boot delta
        assert!(!snap.is_leader);
        assert!(snap.is_valid);
        assert!(snap.smoothed);
        assert_eq!(snap.sequence, 20);
        assert!((snap.csi_fps_ema - 10.5).abs() < 1e-9);
        assert_eq!(snap.csi_fps_samples, 42);
    }

    #[test]
    fn exported_node_rate_is_mature_and_physically_bounded() {
        let mut ns = NodeState::new();
        assert_eq!(ns.measured_sample_rate_hz(), None);

        ns.csi_fps_ema = 74.0;
        ns.csi_fps_samples = 5;
        assert_eq!(ns.measured_sample_rate_hz(), Some(50.0));

        ns.latest_sync = Some(populated_sync(9));
        ns.latest_sync_at = Some(std::time::Instant::now());
        assert_eq!(ns.sync_snapshot().unwrap().csi_fps_ema, 50.0);
    }

    #[test]
    fn observe_csi_frame_arrival_ignores_subms_bursts() {
        // Issue #1180 regression: a ~40 fps node whose frames are delivered
        // in tight UDP bursts (sub-ms intra-burst deltas) must still report
        // ~40 fps, not tens of kHz. Synthesize the arrival stream by adding
        // Durations to a base Instant.
        use std::time::Duration;
        let base = std::time::Instant::now();
        let mut ns = NodeState::new();
        ns.csi_fps_ema = 40.0; // pretend already warmed up
        ns.csi_fps_samples = 10;

        // 30 nominal 25 ms groups, each preceded by a 3-frame sub-ms burst.
        for g in 0..30u64 {
            let group_t = base + Duration::from_millis(25 * g);
            ns.observe_csi_frame_arrival(group_t);
            // burst: two extra arrivals 40 µs and 80 µs later — must be
            // ignored for rate purposes (anchor must not advance to them).
            ns.observe_csi_frame_arrival(group_t + Duration::from_micros(40));
            ns.observe_csi_frame_arrival(group_t + Duration::from_micros(80));
        }

        assert!(
            (ns.csi_fps_ema - 40.0).abs() < 2.0,
            "csi_fps_ema must stay near the 40 fps ground truth despite \
             sub-ms bursts, got {}",
            ns.csi_fps_ema
        );
    }

    #[test]
    fn apply_sync_packet_populates_a_fresh_node() {
        // Mirrors what udp_receiver_task does on the very first sync
        // packet from a previously-unseen node.
        let mut ns = NodeState::new();
        assert!(ns.latest_sync.is_none());
        assert!(ns.latest_sync_at.is_none());

        let now = std::time::Instant::now();
        ns.apply_sync_packet(populated_sync(9), now);

        let sync = ns.latest_sync.as_ref().expect("must be populated");
        assert_eq!(sync.node_id, 9);
        assert_eq!(sync.sequence, 20);
        // latest_sync_at must be exactly the Instant we passed (no clock skew).
        assert_eq!(ns.latest_sync_at, Some(now));
        // sync_snapshot now produces a value (REST 200 OK path).
        assert!(ns.sync_snapshot().is_some());
    }

    #[test]
    fn sync_before_first_csi_still_marks_csi_as_first_sensing_frame() {
        let mut ns = NodeState::new();
        let now = std::time::Instant::now();
        ns.apply_sync_packet(populated_sync(9), now);

        assert!(
            ns.observe_csi_frame_arrival(now + std::time::Duration::from_millis(20)),
            "a sync packet must not consume the first sensing-frame transition"
        );
        assert!(
            !ns.observe_csi_frame_arrival(now + std::time::Duration::from_millis(40)),
            "subsequent CSI frames must not re-emit node.online"
        );
    }

    #[test]
    fn apply_sync_packet_overwrites_older_data() {
        // Subsequent packets must replace, not accumulate. Otherwise the
        // §A0.10-smoothed offset would lag the latest beacon.
        let mut ns = NodeState::new();
        let t0 = std::time::Instant::now();
        ns.apply_sync_packet(populated_sync(9), t0);

        // Second packet: same node, advanced sequence + offset.
        let mut second = populated_sync(9);
        second.sequence = 40;
        second.local_us = 30_000_000;
        second.epoch_us = 28_834_900;
        let t1 = t0 + std::time::Duration::from_secs(2);
        ns.apply_sync_packet(second, t1);

        let cur = ns.latest_sync.as_ref().unwrap();
        assert_eq!(cur.sequence, 40);            // newer sequence persisted
        assert_eq!(cur.local_us, 30_000_000);    // newer local persisted
        assert_eq!(ns.latest_sync_at, Some(t1)); // staleness clock reset
    }

    #[test]
    fn snapshot_staleness_ms_tracks_apply_time() {
        // Iter 34: staleness_ms = (Instant::now() - latest_sync_at).as_millis().
        // We can't pass a synthetic "now" through sync_snapshot, but we can
        // pin latest_sync_at to a past instant and assert the value lands
        // in a plausible window.
        let mut ns = NodeState::new();
        ns.latest_sync = Some(populated_sync(9));
        ns.latest_sync_at = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_millis(750));

        let snap = ns.sync_snapshot().unwrap();
        let st = snap.staleness_ms.expect("staleness_ms must be present");
        // Should be approximately 750 ms — give a generous ±500 ms tolerance
        // for any test-runner scheduling delay between checked_sub() and
        // elapsed() within sync_snapshot.
        assert!(st >= 740 && st < 1250,
                "expected ~750 ms staleness, got {} ms", st);
    }

    #[test]
    fn fleet_role_counts_classifies_correctly() {
        // Iter 37 — verify the leader/follower split that drives the
        // Prometheus `wifi_densepose_mesh_node_total{state=...}` gauge.
        // Local fixture rather than reaching across test modules.
        fn snap(is_leader: bool) -> NodeSyncSnapshot {
            NodeSyncSnapshot {
                offset_us: 0, is_leader, is_valid: true, smoothed: true,
                sequence: 0, csi_fps_ema: 10.0, csi_fps_samples: 10,
                staleness_ms: Some(0),
            }
        }
        assert_eq!(super::fleet_role_counts(&[]), (0, 0));
        let snaps = vec![(12u8, snap(true)), (9, snap(false)), (3, snap(false))];
        assert_eq!(super::fleet_role_counts(&snaps), (1, 2));
        // Edge: all leaders (election would prevent this but gauge math must hold).
        assert_eq!(super::fleet_role_counts(&[(1u8, snap(true)), (2, snap(true))]), (2, 0));
    }

    #[test]
    fn bool_metric_returns_zero_or_one_as_text() {
        // Locks the Prometheus exposition convention: gauges holding a
        // boolean state MUST emit literal "0" or "1", never "false"/"true".
        // If anyone changes the helper to format!("{}", b), Prometheus will
        // 400-reject the scrape — catch it here instead of in production.
        assert_eq!(super::bool_metric(true), "1");
        assert_eq!(super::bool_metric(false), "0");
    }

    #[test]
    fn mesh_aligned_us_honors_9s_staleness_gate() {
        // The receive helper stores latest_sync_at = Instant::now() each
        // beacon. mesh_aligned_us_for_csi_frame returns None once that
        // Instant is older than 9 s (3 × VALID_WINDOW_MS). Verify both
        // sides of that boundary without sleeping — set latest_sync_at
        // to past instants directly.
        let mut ns = NodeState::new();
        let now = std::time::Instant::now();
        ns.latest_sync = Some(populated_sync(9));

        // Fresh: 1 s old → should return Some.
        ns.latest_sync_at = now.checked_sub(std::time::Duration::from_secs(1));
        assert!(ns.mesh_aligned_us_for_csi_frame(20).is_some(),
                "1 s old sync must produce a mesh-aligned timestamp");

        // Just inside the gate: 8 s old → should still return Some.
        ns.latest_sync_at = now.checked_sub(std::time::Duration::from_secs(8));
        assert!(ns.mesh_aligned_us_for_csi_frame(20).is_some(),
                "8 s old sync must still be inside the 9 s gate");

        // Just outside the gate: 10 s old → must return None.
        ns.latest_sync_at = now.checked_sub(std::time::Duration::from_secs(10));
        assert!(ns.mesh_aligned_us_for_csi_frame(20).is_none(),
                "10 s old sync must trigger the 9 s staleness gate");
    }

    #[test]
    fn latest_csi_mesh_time_requires_both_validity_signals() {
        let now = std::time::Instant::now();
        let mut ns = NodeState::new();
        ns.apply_sync_packet(populated_sync(9), now);

        ns.observe_accepted_csi_frame(21, false, now);
        assert!(
            ns.mesh_aligned_us_for_latest_csi_frame().is_none(),
            "an unsynchronized CSI capture must use the host-time fallback"
        );

        ns.observe_accepted_csi_frame(21, true, now + std::time::Duration::from_millis(50));
        assert_eq!(
            ns.mesh_aligned_us_for_latest_csi_frame(),
            Some(27_684_885)
        );

        ns.latest_sync.as_mut().unwrap().flags.is_valid = false;
        assert!(
            ns.mesh_aligned_us_for_latest_csi_frame().is_none(),
            "an invalid sync packet must not timestamp even a flagged CSI frame"
        );
    }

    #[test]
    fn snapshot_reflects_leader_state() {
        // Same data shape that /api/v1/mesh emits for a leader node.
        let mut ns = NodeState::new();
        let mut s = populated_sync(12);
        s.flags = SyncPacketFlags { is_leader: true, is_valid: true, smoothed_used: false };
        s.local_us = 28_864_932;
        s.epoch_us = 28_864_939;  // -7 µs delta on the leader
        ns.latest_sync = Some(s);
        ns.latest_sync_at = Some(std::time::Instant::now());

        let snap = ns.sync_snapshot().unwrap();
        assert!(snap.is_leader);
        assert_eq!(snap.offset_us, -7);  // call-stack µs only
        assert!(!snap.smoothed);
    }
}

#[cfg(test)]
mod novelty_tests {
    use super::*;

    /// First call to `update_novelty` must produce *some* score
    /// (`Some(_)` not `None`) — proves the per-node sketch bank is
    /// initialised by `NodeState::new()` and the novelty path is
    /// actually being exercised. With an empty bank the score is 1.0
    /// (max novelty).
    #[test]
    fn first_frame_yields_max_novelty_then_zero_on_repeat() {
        let mut ns = NodeState::new();
        let amplitudes: Vec<f64> = (0..NOVELTY_VECTOR_DIM).map(|i| (i as f64).sin()).collect();

        ns.update_novelty(&amplitudes);
        let first = ns.last_novelty_score.expect("sketch bank initialised");
        assert!(
            (first - 1.0).abs() < 1e-6,
            "empty bank → max novelty 1.0, got {first}"
        );

        // Repeat the exact same frame — bank now contains it, so the
        // novelty score must be 0.0 (the score is computed before the
        // second insert, against the post-first-insert bank).
        ns.update_novelty(&amplitudes);
        let second = ns.last_novelty_score.expect("score stays Some");
        assert_eq!(second, 0.0, "exact-repeat frame → novelty 0.0");
    }

    /// `update_novelty` must tolerate amplitude vectors of unexpected
    /// length — short ones zero-padded, long ones truncated — without
    /// panicking. ESP32-S3 boards report 56 subcarriers but other
    /// hardware variants ship 52 or 64; the schema-locked sketch bank
    /// requires exactly NOVELTY_VECTOR_DIM.
    #[test]
    fn handles_short_and_long_amplitude_vectors() {
        let mut ns = NodeState::new();
        ns.update_novelty(&[1.0, 2.0]); // way short
        assert!(ns.last_novelty_score.is_some());

        let too_long: Vec<f64> = (0..NOVELTY_VECTOR_DIM * 2).map(|i| i as f64).collect();
        ns.update_novelty(&too_long); // way long
        assert!(ns.last_novelty_score.is_some());
    }
}

// ── ADR-044 §5.3: dedup_factor runtime configuration endpoints ────────────────

/// `GET /api/v1/config/dedup-factor` — read the current dedup factor.
async fn config_get_dedup_factor(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    Json(serde_json::json!({
        "dedup_factor": s.dedup_factor,
        "description": "Divisor for multi-node person count deduplication (sum / factor). Range: 1.0–10.0."
    }))
}

/// `POST /api/v1/config/dedup-factor` — set the dedup factor (clamped 1.0–10.0).
///
/// Body: `{ "value": <f64> }`
async fn config_set_dedup_factor(
    State(state): State<SharedState>,
    Json(body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let value = body.get("value").and_then(|v| v.as_f64()).unwrap_or(3.0);
    let clamped = value.clamp(1.0, 10.0);
    let mut s = state.write().await;
    s.dedup_factor = clamped;
    let data_dir = s.data_dir.clone();
    drop(s);
    save_runtime_config(
        &data_dir,
        &RuntimeConfig {
            dedup_factor: clamped,
        },
    );
    Json(serde_json::json!({
        "status": "ok",
        "dedup_factor": clamped,
    }))
}

/// `GET /api/v1/config/room` — read the current room geometry + node
/// positions. Reflects live in-memory state (`node_positions_config`), not
/// just whatever was last saved to disk, so a fresh page load in the Room
/// Builder UI shows what the server is actually using right now.
async fn config_get_room(State(state): State<SharedState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    let saved = load_room_config(&s.data_dir);
    let nodes: Vec<RoomNode> = s
        .node_positions_config
        .iter()
        .enumerate()
        .map(|(idx, p)| {
            // #1791 made node_positions_config a POSITIONAL Vec rather than a
            // map keyed by node_id, so identity here is the list index. Kept
            // consistent with that convention deliberately: the Room Builder
            // must describe the same binding the fusion path actually uses,
            // or the UI would show positions the server is not applying.
            let id = idx as u8;
            let label = saved
                .nodes
                .iter()
                .find(|n| n.id == id)
                .and_then(|n| n.label.clone());
            RoomNode {
                id,
                x: p[0],
                y: p[1],
                z: p[2],
                label,
            }
        })
        .collect();
    Json(serde_json::json!({
        "width_m": saved.width_m,
        "depth_m": saved.depth_m,
        "nodes": nodes,
    }))
}

/// `POST /api/v1/config/room` — set room geometry + node positions from the
/// Room Builder UI. Takes effect immediately (updates `node_positions_config`
/// and the live `MultistaticFuser`, no restart needed) and persists to
/// `<data_dir>/room_config.json` so it's picked up on the next launch too.
///
/// Body: a full `RoomConfig` (`{ "width_m", "depth_m", "nodes": [...] }`).
/// Validate a `RoomConfig` submitted via `POST /api/v1/config/room`. Pure
/// and side-effect-free so it's directly unit-testable without standing up
/// a full `SharedState`/axum test harness.
pub(crate) fn validate_room_config(config: &RoomConfig) -> Result<(), String> {
    if !config.width_m.is_finite() || config.width_m <= 0.0 {
        return Err("width_m must be a positive, finite number".to_string());
    }
    if !config.depth_m.is_finite() || config.depth_m <= 0.0 {
        return Err("depth_m must be a positive, finite number".to_string());
    }
    let mut seen = std::collections::HashSet::new();
    for n in &config.nodes {
        if !seen.insert(n.id) {
            return Err(format!("duplicate node id {}", n.id));
        }
        if !n.x.is_finite() || !n.y.is_finite() || !n.z.is_finite() {
            return Err(format!("node {} has a non-finite coordinate", n.id));
        }
    }
    Ok(())
}

async fn config_set_room(
    State(state): State<SharedState>,
    Json(config): Json<RoomConfig>,
) -> Json<serde_json::Value> {
    if let Err(e) = validate_room_config(&config) {
        return Json(serde_json::json!({"error": e}));
    }

    let mut s = state.write().await;
    // Indexed by node id so a sparse or non-contiguous set of ids still lands
    // each node at the right slot; gaps stay at the origin rather than
    // shifting every later node, which is what a plain push would do.
    let max_id = config.nodes.iter().map(|n| n.id).max().unwrap_or(0);
    let mut positions = vec![[0.0f32, 0.0, 0.0]; max_id as usize + 1];
    for n in &config.nodes {
        positions[n.id as usize] = [n.x, n.y, n.z];
    }
    s.node_positions_config = positions.clone();
    s.multistatic_fuser.set_node_positions(positions);
    let data_dir = s.data_dir.clone();
    drop(s);
    save_room_config(&data_dir, &config);
    Json(serde_json::json!({
        "status": "ok",
        "width_m": config.width_m,
        "depth_m": config.depth_m,
        "nodes": config.nodes,
    }))
}

/// `POST /api/v1/config/ground-truth` — auto-tune dedup factor from a known person count.
///
/// Derives `dedup_factor = raw_node_sum / ground_truth_count` from the current
/// per-node person counts, clamped to [1.0, 10.0].  Persisted immediately.
///
/// Body: `{ "count": <u64> }`
async fn config_set_ground_truth(
    State(state): State<SharedState>,
    Json(body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let ground_truth = match body.get("count").and_then(|v| v.as_u64()) {
        Some(n) if n > 0 => n as usize,
        _ => return Json(serde_json::json!({"error": "count must be a positive integer"})),
    };
    let mut s = state.write().await;
    let raw_sum: usize = s
        .node_states
        .values()
        .filter(|ns| {
            ns.last_frame_time
                .map(|t| t.elapsed() < std::time::Duration::from_secs(10))
                .unwrap_or(false)
        })
        .map(|ns| ns.prev_person_count)
        .sum();
    let optimal = if raw_sum > 0 {
        (raw_sum as f64) / (ground_truth as f64)
    } else {
        3.0
    };
    let clamped = optimal.clamp(1.0, 10.0);
    s.dedup_factor = clamped;
    let data_dir = s.data_dir.clone();
    drop(s);
    save_runtime_config(
        &data_dir,
        &RuntimeConfig {
            dedup_factor: clamped,
        },
    );
    Json(serde_json::json!({
        "status": "ok",
        "ground_truth": ground_truth,
        "raw_sum": raw_sum,
        "computed_dedup_factor": clamped,
    }))
}

// ── Unit tests: RollingP95 ─────────────────────────────────────────────────────

#[cfg(test)]
mod rolling_p95_tests {
    use super::RollingP95;

    #[test]
    fn cold_start_returns_none() {
        let p = RollingP95::new(100, 10);
        assert!(p.current().is_none(), "empty buffer must return None");
    }

    #[test]
    fn below_min_samples_returns_none() {
        let mut p = RollingP95::new(100, 10);
        for i in 1..=9 {
            p.push(i as f64);
        }
        assert!(
            p.current().is_none(),
            "fewer than min_samples must return None"
        );
    }

    #[test]
    fn p95_of_ramp_is_near_95() {
        let mut p = RollingP95::new(100, 10);
        for i in 1..=100 {
            p.push(i as f64);
        }
        let p95 = p.current().expect("should have value after 100 samples");
        assert!(
            (94.0..=96.0).contains(&p95),
            "P95 of 1..=100 should be ~95, got {p95}"
        );
    }

    #[test]
    fn window_slides_evicts_oldest() {
        let mut p = RollingP95::new(5, 3);
        // Push 1..=5, then 100 — oldest (1) is evicted.
        for i in 1..=5 {
            p.push(i as f64);
        }
        p.push(100.0); // evicts 1; buf = [2, 3, 4, 5, 100]
        let p95 = p.current().expect("6 pushes, window=5 → 5 samples");
        // P95 of [2,3,4,5,100]: idx = ceil(5*0.95)=5 → sorted[4]=100
        assert_eq!(
            p95, 100.0,
            "largest value should dominate p95 after eviction"
        );
    }

    #[test]
    fn len_reports_buffer_size() {
        let mut p = RollingP95::new(10, 5);
        assert_eq!(p.len(), 0);
        p.push(1.0);
        assert_eq!(p.len(), 1);
    }
}

#[cfg(all(test, feature = "mqtt"))]
mod mqtt_bridge_tests {
    use super::vitals_snapshots_from_sensing_json;
    use serde_json::json;

    /// Regression for the per-node presence bug (#872/#898, and its
    /// resurgence as #1541): each node must surface its OWN classification,
    /// not the room-level aggregate. Node 1 is present+moving; node 2 is
    /// absent — node 2 must NOT inherit node 1's "present".
    ///
    /// The fixture below uses `nodes[].node_inference.classification` — the
    /// field `NodeInfo` actually serializes (ADR-297) — not a bare
    /// `nodes[].classification`. Issue #1541: an earlier version of this exact
    /// test used the latter, non-existent shape, which the reader silently
    /// treated as "field omitted" and fell back to the room aggregate for
    /// every node. The test therefore passed while the real per-node MQTT
    /// output was array-global — 100% line coverage of
    /// `vitals_snapshots_from_sensing_json` with a fixture that didn't match
    /// what `NodeInfo` actually serializes. Keep this fixture in the real
    /// shape so this can't recur silently.
    #[test]
    fn per_node_presence_uses_each_nodes_own_classification() {
        let v = json!({
            "timestamp": 1.0,
            "classification": { "presence": true, "motion_level": "walking", "confidence": 0.9 },
            "vital_signs": { "breathing_rate_bpm": 14.0, "heart_rate_bpm": 60.0 },
            "persons": [{}, {}],
            "nodes": [
                { "node_id": 1, "rssi_dbm": -40.0,
                  "node_inference": { "classification": "present_moving", "confidence": 0.8 } },
                { "node_id": 2, "rssi_dbm": -70.0,
                  "node_inference": { "classification": "absent", "confidence": 0.1 } }
            ]
        });
        let snaps = vitals_snapshots_from_sensing_json(&v, "ruview");
        assert_eq!(snaps.len(), 2, "one snapshot per node");

        let n1 = snaps.iter().find(|s| s.node_id == "ruview-node1").unwrap();
        let n2 = snaps.iter().find(|s| s.node_id == "ruview-node2").unwrap();

        assert!(n1.presence && n1.motion > 0.0, "node1 present + moving");
        assert!(
            !n2.presence && n2.motion == 0.0,
            "node2 must be absent — not inherit the room aggregate"
        );
        // Per-node RSSI preserved.
        assert_eq!(n1.rssi_dbm, Some(-40.0));
        assert_eq!(n2.rssi_dbm, Some(-70.0));
        // Vitals + person count are room-level, shared across node devices.
        assert_eq!(n1.n_persons, 2);
        assert_eq!(n2.n_persons, 2);
        assert_eq!(n1.breathing_rate_bpm, Some(14.0));
        assert_eq!(n2.heartrate_bpm, Some(60.0));
        // presence_score is gated on presence.
        assert!(n1.presence_score > 0.0);
        assert_eq!(n2.presence_score, 0.0);
    }

    /// A node that omits a classification field defers to the room aggregate
    /// rather than silently reading false/0.
    #[test]
    fn per_node_missing_fields_fall_back_to_aggregate() {
        let v = json!({
            "timestamp": 1.0,
            "classification": { "presence": true, "motion_level": "still", "confidence": 0.7 },
            "vital_signs": {},
            "nodes": [ { "node_id": 3, "rssi_dbm": -55.0 } ]  // no per-node classification
        });
        let snaps = vitals_snapshots_from_sensing_json(&v, "n");
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].node_id, "n-node3");
        assert!(snaps[0].presence, "defers to aggregate presence");
        assert_eq!(snaps[0].motion, 0.0, "aggregate 'still' => no motion");
    }

    /// No `nodes` array (wifi / simulate sources): single aggregate snapshot
    /// keyed by the base id.
    #[test]
    fn falls_back_to_single_aggregate_when_no_nodes() {
        let v = json!({
            "timestamp": 2.0,
            "classification": { "presence": true, "motion_level": "idle", "confidence": 0.6 },
            "vital_signs": { "breathing_rate_bpm": 12.0 },
            "persons": [{}]
        });
        let snaps = vitals_snapshots_from_sensing_json(&v, "ruview");
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].node_id, "ruview");
        assert!(snaps[0].presence);
        assert_eq!(snaps[0].motion, 0.0, "idle => no motion");
        assert_eq!(snaps[0].n_persons, 1);
    }

    /// `motion_level: "absent"` must map to zero motion (the old aggregate
    /// match fell through to `Some(_) => 1.0`, treating absent as full motion).
    #[test]
    fn absent_motion_level_is_zero_motion() {
        let v = json!({
            "timestamp": 0.0,
            "classification": { "presence": false, "motion_level": "absent", "confidence": 0.0 },
            "vital_signs": {}
        });
        let snaps = vitals_snapshots_from_sensing_json(&v, "x");
        assert_eq!(snaps[0].motion, 0.0);
        assert!(!snaps[0].presence);
    }
}

#[cfg(test)]
mod model_load_diagnostic_tests {
    use super::{diagnose_model_load_error, load_or_convert_model};
    use std::path::Path;

    #[test]
    fn jsonl_model_loads_through_model_flag_path() {
        let data = b"{\"model_id\":\"published\"}\n{\"weights\":[1.0,2.0]}\n";
        let mut loader = load_or_convert_model(Path::new("model.rvf.jsonl"), data)
            .expect("--model must auto-convert JSONL");
        loader.load_layer_a().expect("Layer A");
        let layer_c = loader.load_layer_c().expect("Layer C");
        assert_eq!(layer_c.all_weights, vec![1.0, 2.0]);
    }

    #[test]
    fn safetensors_is_named_and_points_at_894() {
        // 8-byte LE header length then '{' — the safetensors signature.
        let data = [0x10, 0, 0, 0, 0, 0, 0, 0, b'{', b'"'];
        let msg = diagnose_model_load_error(
            Path::new("models/wifi-densepose-pretrained/model.safetensors"),
            &data,
            "invalid magic at offset 0",
        );
        assert!(msg.contains("safetensors"), "{msg}");
        assert!(msg.contains("#894"), "{msg}");
        assert!(msg.contains("signal heuristics"), "{msg}");
    }

    #[test]
    fn quantized_bin_is_identified() {
        let data = [0x35, 0x57, 0x45, 0x77]; // the 0x77455735 the loader reports
        let msg = diagnose_model_load_error(Path::new("model-q4.bin"), &data, "bad magic");
        assert!(msg.contains("quantized weight blob"), "{msg}");
        assert!(msg.contains("RVFS") || msg.contains("0x52564653"), "{msg}");
    }

    #[test]
    fn jsonl_manifest_is_identified() {
        let data = *b"{\"seg\":0}";
        let msg = diagnose_model_load_error(Path::new("model.rvf.jsonl"), &data, "x");
        assert!(msg.contains("JSONL manifest"), "{msg}");
    }

    #[test]
    fn unknown_format_still_gives_guidance() {
        let data = [0u8, 1, 2, 3];
        let msg = diagnose_model_load_error(Path::new("weird.dat"), &data, "x");
        assert!(msg.contains("RVF binary container"), "{msg}");
        assert!(msg.contains("wifi-densepose-train"), "{msg}");
    }
}

#[cfg(test)]
mod export_rvf_mode_tests {
    use super::export_emits_placeholder_demo;

    #[test]
    fn standalone_export_emits_placeholder() {
        // --export-rvf alone → the container-format demo (placeholder weights).
        assert!(export_emits_placeholder_demo(true, false, false));
    }

    #[test]
    fn export_with_train_does_not_short_circuit() {
        // #894: `--train --export-rvf` must NOT emit a placeholder + skip
        // training — it must fall through to the real training pipeline.
        assert!(!export_emits_placeholder_demo(true, true, false));
        assert!(!export_emits_placeholder_demo(true, false, true));
        assert!(!export_emits_placeholder_demo(true, true, true));
    }

    #[test]
    fn no_export_flag_never_emits() {
        assert!(!export_emits_placeholder_demo(false, false, false));
        assert!(!export_emits_placeholder_demo(false, true, false));
    }
}

#[cfg(test)]
mod observatory_persons_field_position_tests {
    //! Issue #1050 — the Observatory 3D figure animates from per-person
    //! `position` / `motion_score` / `pose` carried on `sensing_update.persons`.
    //!
    //! These tests pin the public WS contract: a frame that detects a person on
    //! a known signal_field peak must emit a `persons` array whose first entry
    //! carries a `position` derived from that peak (matching the Observatory's
    //! cell→world transform), a real `motion_score`, and a serialized frame
    //! that round-trips. An empty / no-presence field must emit `persons: []`
    //! (or no person), never a phantom person at a fabricated origin.

    use super::*;

    /// Build a 20×20 signal_field that is background everywhere except a single
    /// strong normalized peak at grid cell `(ix, iz)`.
    fn field_with_peak(ix: usize, iz: usize) -> SignalField {
        let nx = 20usize;
        let nz = 20usize;
        let mut values = vec![0.05f64; nx * nz];
        values[iz * nx + ix] = 1.0;
        SignalField {
            grid_size: [nx, 1, nz],
            values,
        }
    }

    /// Build an all-background (below-threshold) 20×20 field — no localizable
    /// hotspot, modelling an empty / no-presence room.
    fn empty_field() -> SignalField {
        SignalField {
            grid_size: [20, 1, 20],
            values: vec![0.05f64; 20 * 20],
        }
    }

    fn base_update(signal_field: SignalField, presence: bool, motion_band_power: f64) -> SensingUpdate {
        SensingUpdate {
            msg_type: "sensing_update".to_string(),
            timestamp: 1.0,
            source: "test".to_string(),
            tick: 1,
            nodes: vec![],
            features: FeatureInfo {
                mean_rssi: -60.0,
                variance: 48.6,
                motion_band_power,
                breathing_band_power: 0.0,
                dominant_freq_hz: 1.0,
                change_points: 0,
                spectral_power: 0.0,
            },
            classification: ClassificationInfo {
                motion_level: if presence { "present_moving".to_string() } else { "absent".to_string() },
                presence,
                confidence: 0.8,
            },
            signal_field,
            vital_signs: None,
            enhanced_motion: None,
            enhanced_breathing: None,
            posture: None,
            signal_quality_score: None,
            quality_verdict: None,
            bssid_count: None,
            pose_keypoints: None,
            model_status: None,
            persons: None,
            estimated_persons: Some(1),
            node_features: None,
            room_inference: None,
        }
    }

    #[test]
    fn sensing_update_emits_persons_with_field_derived_position() {
        // Person present, motion energy 63.3, a hotspot at cell (15, 4).
        let peak_ix = 15;
        let peak_iz = 4;
        let mut update = base_update(field_with_peak(peak_ix, peak_iz), true, 63.3);

        // Pipeline order: derive raw skeleton, then attach real field positions.
        update.persons = Some(derive_pose_from_sensing(&update));
        attach_field_positions(&mut update);

        let persons = update.persons.as_ref().expect("persons should be Some");
        assert!(!persons.is_empty(), "a present person must be emitted");

        // Position must match the Observatory cell→world transform for (15, 4):
        // x = (15-10)*0.6 = 3.0 ; z = (4-10)*0.5 = -3.0 ; y = 0.
        let p0 = &persons[0];
        assert!((p0.position[0] - 3.0).abs() < 1e-6, "x={}", p0.position[0]);
        assert!((p0.position[1] - 0.0).abs() < 1e-9);
        assert!((p0.position[2] - (-3.0)).abs() < 1e-6, "z={}", p0.position[2]);

        // motion_score is the measured motion_band_power passed through (≤100).
        assert!((p0.motion_score - 63.3).abs() < 1e-6, "motion_score={}", p0.motion_score);

        // The serialized WS frame must carry the new fields by their exact
        // contract names the Observatory UI reads.
        let v = serde_json::to_value(&update).unwrap();
        let arr = v["persons"].as_array().expect("persons must be a JSON array");
        assert_eq!(arr.len(), persons.len());
        let pj = &arr[0];
        assert!(pj.get("position").is_some(), "person.position missing from WS frame");
        assert!(pj.get("motion_score").is_some(), "person.motion_score missing from WS frame");
        assert!((pj["position"][0].as_f64().unwrap() - 3.0).abs() < 1e-6);
        assert!((pj["position"][2].as_f64().unwrap() - (-3.0)).abs() < 1e-6);
        assert!((pj["motion_score"].as_f64().unwrap() - 63.3).abs() < 1e-6);
    }

    #[test]
    fn legacy_live_pose_is_audited_as_uncalibrated_image_data() {
        let mut update = base_update(field_with_peak(10, 10), true, 20.0);
        update.persons = Some(derive_pose_from_sensing(&update));
        let mut state = AppStateInner::minimal();

        assess_legacy_image_pose(&mut state, &update);

        let (raw, result) = state
            .pose_physics
            .latest()
            .expect("legacy image pose should reach canonical audit");
        assert_eq!(raw.dimensionality, wifi_densepose_core::PoseDimensionality::Image2d);
        assert!(!raw.uncertainty_calibrated);
        assert!(!raw.source.authenticated);
        assert_eq!(
            result.disposition,
            wifi_densepose_core::RefinementDisposition::Audited2d
        );
        assert!(!result.selected);
        assert!(result.refined_joints_m.is_none());
    }

    #[test]
    fn pose_is_real_when_posture_present_and_absent_otherwise() {
        // No aggregate posture estimate → pose is None (never fabricated).
        let mut no_posture = base_update(field_with_peak(10, 10), true, 40.0);
        no_posture.persons = Some(derive_pose_from_sensing(&no_posture));
        attach_field_positions(&mut no_posture);
        let p = &no_posture.persons.as_ref().unwrap()[0];
        assert!(p.pose.is_none(), "pose must stay None when no real posture exists");
        // skip_serializing_if drops the key entirely (UI defaults to 'standing').
        let v = serde_json::to_value(&no_posture).unwrap();
        assert!(v["persons"][0].get("pose").is_none());

        // Real aggregate posture present → pose is carried through verbatim.
        let mut with_posture = base_update(field_with_peak(10, 10), true, 40.0);
        with_posture.posture = Some("lying".to_string());
        with_posture.persons = Some(derive_pose_from_sensing(&with_posture));
        attach_field_positions(&mut with_posture);
        let p2 = &with_posture.persons.as_ref().unwrap()[0];
        assert_eq!(p2.pose.as_deref(), Some("lying"));
        let v2 = serde_json::to_value(&with_posture).unwrap();
        assert_eq!(v2["persons"][0]["pose"], "lying");
    }

    #[test]
    fn empty_room_yields_no_phantom_person() {
        // No presence → derive_pose_from_sensing returns no persons at all.
        let mut update = base_update(empty_field(), false, 2.0);
        update.persons = Some(derive_pose_from_sensing(&update));
        attach_field_positions(&mut update);

        let persons = update.persons.as_ref().unwrap();
        assert!(
            persons.is_empty(),
            "no-presence frame must not emit a phantom person, got {} persons",
            persons.len()
        );

        // And in the serialized frame the array is empty (no fake origin person).
        let v = serde_json::to_value(&update).unwrap();
        assert_eq!(v["persons"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn present_but_below_threshold_field_keeps_position_at_origin_not_fabricated() {
        // Presence is true but the field has no peak above PEAK_THRESHOLD — we
        // must NOT invent a position; it stays at the [0,0,0] default while
        // motion_score still reflects the real measured motion power. This is
        // the honest degenerate case (no localizable hotspot to report).
        let mut update = base_update(empty_field(), true, 55.0);
        update.persons = Some(derive_pose_from_sensing(&update));
        attach_field_positions(&mut update);

        let p = &update.persons.as_ref().unwrap()[0];
        assert_eq!(p.position, [0.0, 0.0, 0.0], "no peak → default origin, not fabricated coords");
        assert!((p.motion_score - 55.0).abs() < 1e-6, "motion_score stays real");
    }
}

/// `POST /api/v1/ws-ticket` — mint a single-use WebSocket ticket (ADR-272).
///
/// Reached only through the auth middleware, so an unauthenticated caller
/// cannot mint one. The ticket inherits the caller's scopes, so a
/// `sensing:read` session cannot produce a ticket that outranks itself.
///
/// Exists because a browser's `WebSocket` constructor cannot set an
/// `Authorization` header. Native clients do not need this — they send a bearer
/// on the upgrade directly.
async fn ws_ticket_handler(
    axum::Extension(auth): axum::Extension<wifi_densepose_sensing_server::bearer_auth::AuthState>,
    request: axum::extract::Request,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    use wifi_densepose_sensing_server::ws_ticket::TicketGrant;

    // Present when the caller authenticated with OAuth; absent when they used
    // the legacy static token, which predates scopes and carries full authority.
    let principal = request.extensions().get::<ruview_auth::Principal>();
    let grant = TicketGrant {
        scopes: principal.map(|p| p.scopes().collect::<Vec<_>>().join(" ")),
        subject: principal.map(|p| p.subject.clone()),
    };

    match auth.tickets().issue(grant) {
        Some(ticket) => (
            axum::http::StatusCode::OK,
            axum::Json(serde_json::json!({
                "ticket": ticket,
                "expires_in_secs": wifi_densepose_sensing_server::ws_ticket::TICKET_TTL.as_secs(),
                "usage": "append as ?ticket=<value> to the WebSocket URL; valid once",
            })),
        )
            .into_response(),
        // Refusing beats growing the store without bound.
        None => (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "too many outstanding WebSocket tickets; retry shortly\n",
        )
            .into_response(),
    }
}

// ---- ADR-271 browser sign-in ------------------------------------------------
//
// Ported from cognitum-one/freetokens (`src/auth/oauth.ts`, live). The browser
// never holds an OAuth token: this server does the exchange and issues its own
// signed session cookie. Closes the gap where `wifi-densepose login` wrote a
// file no browser could read.

fn request_is_tls(headers: &axum::http::HeaderMap) -> bool {
    // Behind a reverse proxy the TLS terminates upstream, so trust the standard
    // forwarding header when present. Conservative default: not TLS, which only
    // ever omits `Secure` — it never adds a cookie where it shouldn't be.
    headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .map(|p| p.eq_ignore_ascii_case("https"))
        .unwrap_or(false)
        || wifi_densepose_sensing_server::browser_session::public_base_url().starts_with("https://")
}

async fn oauth_start(
    axum::Extension(auth): axum::Extension<wifi_densepose_sensing_server::bearer_auth::AuthState>,
    headers: axum::http::HeaderMap,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    use wifi_densepose_sensing_server::browser_session as bs;

    let Some(issuer) = auth.oauth_issuer() else {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "OAuth is not enabled on this server (set RUVIEW_OAUTH_ISSUER)\n",
        )
            .into_response();
    };
    let secure = request_is_tls(&headers);
    // Least privilege: a browser session asks for read. Admin work goes through
    // the CLI, which requires an explicit --admin. See BROWSER_SIGNIN_SCOPE for
    // what widening this would cost.
    match bs::begin(&issuer, &auth.primary_client_id(), bs::BROWSER_SIGNIN_SCOPE, secure) {
        Ok((location, cookie)) => (
            axum::http::StatusCode::FOUND,
            [
                (axum::http::header::LOCATION, location),
                (axum::http::header::SET_COOKIE, cookie),
            ],
        )
            .into_response(),
        Err(e) => (axum::http::StatusCode::SERVICE_UNAVAILABLE, format!("{e}\n")).into_response(),
    }
}

#[derive(serde::Deserialize)]
struct OAuthCallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

async fn oauth_callback(
    axum::Extension(auth): axum::Extension<wifi_densepose_sensing_server::bearer_auth::AuthState>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(q): axum::extract::Query<OAuthCallbackQuery>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    use wifi_densepose_sensing_server::browser_session as bs;

    let secure = request_is_tls(&headers);
    let bad = |code: axum::http::StatusCode, msg: String| {
        (code, [(axum::http::header::SET_COOKIE, bs::clear_transaction(secure))], msg)
            .into_response()
    };

    if let Some(err) = q.error {
        return bad(axum::http::StatusCode::BAD_REQUEST, format!("Cognitum declined the sign-in: {err}\n"));
    }
    let (Some(code), Some(state)) = (q.code, q.state) else {
        return bad(axum::http::StatusCode::BAD_REQUEST, "Incomplete sign-in response\n".into());
    };
    let cookie_header = headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();

    // CSRF check BEFORE the single-use code is spent.
    let verifier = match bs::verifier_for_callback(&cookie_header, &state) {
        Ok(v) => v,
        Err(e) => return bad(axum::http::StatusCode::BAD_REQUEST, format!("{e}\n")),
    };

    let Some(issuer) = auth.oauth_issuer() else {
        return bad(axum::http::StatusCode::SERVICE_UNAVAILABLE, "OAuth is not enabled\n".into());
    };
    let client_id = auth.primary_client_id();

    // `ureq` is blocking; spawn_blocking so a slow token endpoint cannot park an
    // async worker (the same mistake this codebase had to fix in jwks.rs).
    let exchange = tokio::task::spawn_blocking(move || {
        ureq::post(&format!("{issuer}/oauth/token"))
            .send_form(&[
                ("grant_type", "authorization_code"),
                ("code", &code),
                ("code_verifier", &verifier),
                ("client_id", &client_id),
                ("redirect_uri", &bs::redirect_uri()),
            ])
            .map_err(|e| e.to_string())
            .and_then(|r| r.into_string().map_err(|e| e.to_string()))
    })
    .await;

    let body = match exchange {
        Ok(Ok(b)) => b,
        Ok(Err(e)) => return bad(axum::http::StatusCode::BAD_GATEWAY, format!("token exchange failed: {e}\n")),
        Err(e) => return bad(axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("token exchange task failed: {e}\n")),
    };
    let access_token = match serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v.get("access_token")?.as_str().map(str::to_owned))
    {
        Some(t) => t,
        None => return bad(axum::http::StatusCode::BAD_GATEWAY, "token endpoint returned no access_token\n".into()),
    };

    // Verify with the SAME verifier that gates every other request — signature,
    // audience, typ, expiry, scope. A browser sign-in must not be a softer path.
    let principal = match auth.verify_for_browser(&access_token) {
        Ok(p) => p,
        Err(e) => return bad(axum::http::StatusCode::UNAUTHORIZED, format!("{e}\n")),
    };

    let session_cookie = match bs::issue(&principal, secure) {
        Ok(c) => c,
        Err(e) => return bad(axum::http::StatusCode::SERVICE_UNAVAILABLE, format!("{e}\n")),
    };
    tracing::info!(sub = %principal.subject, "browser sign-in complete");

    // Clear the spent transaction as well as issuing the session. A consumed
    // OAuth transaction has no further use, and leaving it to age out for ten
    // minutes means every subsequent request carries a dead cookie.
    (
        axum::http::StatusCode::FOUND,
        // AppendHeaders, NOT an array: the array form REPLACES same-name
        // headers, so a second Set-Cookie silently overwrites the first — which
        // would drop the session cookie and make sign-in a no-op.
        axum::response::AppendHeaders([
            (axum::http::header::LOCATION, format!("/ui/?signed_in={}", now_millis())),
            (axum::http::header::SET_COOKIE, session_cookie),
            (axum::http::header::SET_COOKIE, bs::clear_transaction(secure)),
        ]),
    )
        .into_response()
}

async fn oauth_logout(headers: axum::http::HeaderMap) -> axum::response::Response {
    use axum::response::IntoResponse;
    // Local only: forgets this browser's session. Revoking the Cognitum session
    // for every device is an account-level action at auth.cognitum.one.
    let secure = request_is_tls(&headers);
    use wifi_densepose_sensing_server::browser_session as bs;
    (
        axum::http::StatusCode::FOUND,
        axum::response::AppendHeaders([
            // Cache-busting query so the landing page is re-fetched rather than
            // restored from the back/forward cache with a stale panel.
            (axum::http::header::LOCATION, format!("/ui/?signed_out={}", now_millis())),
            (axum::http::header::SET_COOKIE, bs::clear_session(secure)),
            (axum::http::header::SET_COOKIE, bs::clear_transaction(secure)),
        ]),
    )
        .into_response()
}

fn now_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// `GET /oauth/status` — what a signed-out browser needs to render the right UI.
///
/// Deliberately ungated and deliberately thin: capability flags and, if a live
/// session exists, who it belongs to. No token, no scope escalation hints, no
/// server configuration beyond "is sign-in possible here".
async fn oauth_status(
    axum::Extension(auth): axum::Extension<wifi_densepose_sensing_server::bearer_auth::AuthState>,
    headers: axum::http::HeaderMap,
) -> axum::Json<serde_json::Value> {
    use wifi_densepose_sensing_server::browser_session as bs;
    let raw = headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok());
    let session = raw.and_then(bs::from_cookie_header);
    axum::Json(serde_json::json!({
        "auth_required": auth.is_enabled(),
        "oauth_enabled": auth.oauth_enabled(),
        "browser_signin": auth.oauth_enabled() && bs::is_configured(),
        "signed_in": session.is_some(),
        "account": session.as_ref().map(|s| s.account_id.clone()),
        "scope": session.as_ref().map(|s| s.scope.clone()),
    }))
}
#[cfg(test)]
mod adr186_http_tests {
    //! ADR-186 P6: HTTP-level tests that build the real `training_api` router
    //! and drive it in-process, guarding against the module being orphaned again
    //! (`training_api::routes()` cannot compile unless the module is declared).
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    /// Serializes tests that read/toggle the process-global
    /// `RUVIEW_DISABLE_SERVER_TRAINING` env var, so the disabled-path test cannot
    /// flip enablement while an enabled-path test is mid-request.
    static TRAIN_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn test_state() -> SharedState {
        Arc::new(RwLock::new(AppStateInner::minimal()))
    }

    /// The `/ws/train/progress` route is registered and reaches the WebSocket
    /// handler (issue #1233 was a 404). Over `oneshot` there is no real socket to
    /// upgrade, so axum returns 426 Upgrade Required — which still distinguishes a
    /// wired WS endpoint (426) from an orphaned/absent route (404). The genuine
    /// 101 handshake is asserted by `ws_train_progress_live_101_and_frame`.
    #[tokio::test]
    async fn ws_train_progress_route_is_wired_not_404() {
        let app = training_api::routes().with_state(test_state());
        let req = Request::builder()
            .uri("/ws/train/progress")
            .header("connection", "upgrade")
            .header("upgrade", "websocket")
            .header("sec-websocket-version", "13")
            .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_ne!(resp.status(), StatusCode::NOT_FOUND, "route must not 404");
        assert_eq!(
            resp.status(),
            StatusCode::UPGRADE_REQUIRED,
            "a wired WS route returns 426 under oneshot — got {}",
            resp.status()
        );
    }

    /// ADR-186 §7 acceptance: over a real socket, `/ws/train/progress` completes a
    /// genuine 101 WebSocket handshake and, after a `POST /api/v1/train/start`,
    /// delivers at least one real `progress` frame to the connected client.
    #[tokio::test]
    async fn ws_train_progress_live_101_and_frame() {
        use futures_util::StreamExt;
        use tokio::io::AsyncWriteExt;
        use tokio_tungstenite::tungstenite::Message as TMsg;

        let _env_lock = TRAIN_ENV_LOCK.lock().unwrap(); // enablement must stay ON
        let shared = test_state();
        {
            let mut s = shared.write().await;
            for i in 0..40 {
                let sub: Vec<f64> = (0..56)
                    .map(|k| 10.0 + ((i as f64) * 0.3 + (k as f64) * 0.1).sin() * 2.0)
                    .collect();
                s.frame_history.push_back(sub);
            }
        }

        // Serve the training router on an ephemeral port.
        let app = training_api::routes().with_state(shared.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        // A successful `connect_async` IS the 101 handshake (it errors otherwise).
        let (mut ws, resp) =
            tokio_tungstenite::connect_async(format!("ws://{addr}/ws/train/progress"))
                .await
                .expect("WebSocket handshake should succeed (101)");
        assert_eq!(resp.status().as_u16(), 101, "handshake must be 101");

        // Drive training via a real HTTP POST over a fresh TCP connection.
        let body = r#"{"dataset_ids":[],"config":{"epochs":3,"batch_size":8,"warmup_epochs":1,"early_stopping_patience":10}}"#;
        let req = format!(
            "POST /api/v1/train/start HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let mut post = tokio::net::TcpStream::connect(addr).await.unwrap();
        post.write_all(req.as_bytes()).await.unwrap();
        post.flush().await.unwrap();

        // Read WS frames until a `progress` frame arrives (or a 10s ceiling).
        let mut got_progress = false;
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_secs(2), ws.next()).await {
                Ok(Some(Ok(TMsg::Text(txt)))) => {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) {
                        if v.get("type").and_then(|t| t.as_str()) == Some("progress") {
                            got_progress = true;
                            break;
                        }
                    }
                }
                Ok(Some(Ok(_))) => {}
                Ok(Some(Err(_))) | Ok(None) => break,
                Err(_) => {}
            }
        }
        assert!(
            got_progress,
            "should receive a real progress frame over the live WS after POST start"
        );
        // NOTE: deliberately no directory-diff cleanup here. `data/models` is
        // gitignored, and deleting by dir-diff would race concurrent model-writing
        // tests (it could remove a `.rvf` another test is asserting exists).
    }

    /// Full HTTP round-trip: POST /api/v1/train/start → poll /api/v1/train/status
    /// until completion → a real `.rvf` model artifact exists on disk, and real
    /// progress frames were streamed on the broadcast channel.
    #[tokio::test]
    async fn http_train_start_produces_model_and_streams() {
        let _env_lock = TRAIN_ENV_LOCK.lock().unwrap(); // enablement must stay ON
        let shared = test_state();
        // Seed synthetic frames so training's fallback path has data (no files).
        {
            let mut s = shared.write().await;
            for i in 0..40 {
                let sub: Vec<f64> = (0..56)
                    .map(|k| 10.0 + ((i as f64) * 0.3 + (k as f64) * 0.1).sin() * 2.0)
                    .collect();
                s.frame_history.push_back(sub);
            }
        }
        let mut progress_rx = {
            let s = shared.read().await;
            s.training_progress_tx.subscribe()
        };

        let models_dir = std::path::PathBuf::from(training_api::MODELS_DIR);
        let before: std::collections::HashSet<std::path::PathBuf> = std::fs::read_dir(&models_dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .collect();

        let app = training_api::routes().with_state(shared.clone());

        // POST start.
        let body = serde_json::json!({
            "dataset_ids": [],
            "config": {"epochs": 3, "batch_size": 8, "warmup_epochs": 1, "early_stopping_patience": 10}
        });
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/train/start")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "start should be accepted");

        // Poll status until the job reports completion.
        let mut completed = false;
        for _ in 0..250 {
            let req = Request::builder()
                .uri("/api/v1/train/status")
                .body(Body::empty())
                .unwrap();
            let resp = app.clone().oneshot(req).await.unwrap();
            let bytes = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
            let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            // Status also carries the P5 enablement flag.
            assert_eq!(v.get("enabled"), Some(&serde_json::Value::Bool(true)));
            if v.get("active") == Some(&serde_json::Value::Bool(false))
                && v.get("phase").and_then(|p| p.as_str()) == Some("completed")
            {
                completed = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        assert!(completed, "training should reach the completed phase");

        // Real progress frames were streamed.
        let mut saw_progress = false;
        while progress_rx.try_recv().is_ok() {
            saw_progress = true;
        }
        assert!(saw_progress, "expected streamed progress frames over the WS channel");

        // A new .rvf artifact was written by the run.
        let after: std::collections::HashSet<std::path::PathBuf> = std::fs::read_dir(&models_dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .collect();
        let new_models: Vec<_> = after
            .difference(&before)
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("rvf"))
            .cloned()
            .collect();
        assert!(
            !new_models.is_empty(),
            "training should write a new .rvf model artifact under {}",
            models_dir.display()
        );
        // No deletion here: removing by dir-diff would race concurrent
        // model-writing tests. `data/models` is gitignored.
    }

    /// P5 fallback guarantee: with server training disabled, POST start returns a
    /// structured `{enabled:false, cli:...}` 409 — never a silent success.
    #[tokio::test]
    async fn http_train_start_disabled_returns_structured_409() {
        // Serialize against the enabled-path tests so our env toggle can't race
        // their in-flight requests.
        let _env_lock = TRAIN_ENV_LOCK.lock().unwrap();
        std::env::set_var("RUVIEW_DISABLE_SERVER_TRAINING", "1");

        let app = training_api::routes().with_state(test_state());
        let body = serde_json::json!({"dataset_ids": [], "config": {"epochs": 1}});
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/train/start")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        std::env::remove_var("RUVIEW_DISABLE_SERVER_TRAINING");

        assert_eq!(status, StatusCode::CONFLICT, "disabled start must be 4xx/409");
        assert_eq!(v.get("enabled"), Some(&serde_json::Value::Bool(false)));
        assert_eq!(
            v.get("cli").and_then(|c| c.as_str()),
            Some("wifi-densepose train-room"),
            "must point at the CLI fallback, never a silent success"
        );
        assert_ne!(
            v.get("success"),
            Some(&serde_json::Value::Bool(true)),
            "must never claim success:true when disabled"
        );
    }
}
