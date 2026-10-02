//! Training API with WebSocket progress streaming.
//!
//! Provides REST endpoints for starting, stopping, and monitoring training runs.
//! Training runs in a background tokio task. Progress updates are broadcast via
//! a `tokio::sync::broadcast` channel that the WebSocket handler subscribes to.
//!
//! Uses a **real training pipeline** that loads recorded CSI data from `.csi.jsonl`
//! files, extracts signal features (subcarrier variance, temporal gradients, Goertzel
//! frequency-domain power), trains a regularised linear model via batch gradient
//! descent, and exports calibrated `.rvf` model containers.
//!
//! No PyTorch / `tch` dependency is required. All linear algebra is implemented
//! inline using standard Rust math.
//!
//! On completion, the best model is automatically exported as `.rvf` using `RvfBuilder`.
//!
//! REST endpoints:
//! - `POST /api/v1/train/start`    -- start a training run
//! - `POST /api/v1/train/stop`     -- stop the active training
//! - `GET  /api/v1/train/status`   -- get current training status
//! - `POST /api/v1/train/pretrain` -- start contrastive pretraining
//! - `POST /api/v1/train/lora`     -- start LoRA fine-tuning
//!
//! WebSocket:
//! - `WS /ws/train/progress`       -- streaming training progress

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use crate::rvf_container::RvfBuilder;

// ── Constants ────────────────────────────────────────────────────────────────

/// Directory for trained model output.
pub const MODELS_DIR: &str = "data/models";

/// Directory the training loop reads recorded CSI datasets from. Each
/// `dataset_id` maps to `{RECORDINGS_DIR}/{dataset_id}.csi.jsonl`.
pub const RECORDINGS_DIR: &str = "data/recordings";

/// Monotonic per-process counter appended to exported model filenames so two
/// runs that complete in the same wall-clock microsecond still get distinct
/// paths (prevents silent overwrite; keeps concurrent runs from colliding).
static MODEL_ID_SEQ: AtomicU64 = AtomicU64::new(0);

/// Build a process-unique model id `trained-{type}-{ts_micros}-{seq}`. A
/// second-resolution timestamp alone collided for runs finishing in the same
/// second (silent overwrite); microseconds + the monotonic counter guarantee
/// uniqueness even for same-microsecond concurrent completions.
fn next_model_id(training_type: &str) -> String {
    format!(
        "trained-{}-{}-{}",
        training_type,
        chrono::Utc::now().format("%Y%m%d_%H%M%S_%6f"),
        MODEL_ID_SEQ.fetch_add(1, Ordering::Relaxed)
    )
}

/// Number of COCO keypoints.
const N_KEYPOINTS: usize = 17;
/// Dimensions per keypoint in the target vector (x, y, z).
const DIMS_PER_KP: usize = 3;
/// Total target dimensionality: 17 * 3 = 51.
const N_TARGETS: usize = N_KEYPOINTS * DIMS_PER_KP;

/// Default number of subcarriers when data is unavailable.
const DEFAULT_N_SUB: usize = 56;
/// Sliding window size for computing per-subcarrier variance.
const VARIANCE_WINDOW: usize = 10;
/// Number of Goertzel frequency bands to probe.
const N_FREQ_BANDS: usize = 9;
/// Number of global scalar features (mean amplitude, std, motion score).
const N_GLOBAL_FEATURES: usize = 3;

// ── Types ────────────────────────────────────────────────────────────────────

/// A single recorded CSI frame line, as stored in the `.csi.jsonl` datasets the
/// training loop consumes.
///
/// This mirrors the on-disk JSONL schema and is intentionally self-contained so
/// the trainer does not couple to the (separate, orphaned) `recording.rs`
/// module. Only the fields the feature extractor needs are read; `rssi` /
/// `noise_floor` / `features` are carried for schema fidelity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedFrame {
    pub timestamp: f64,
    pub subcarriers: Vec<f64>,
    #[serde(default)]
    pub rssi: f64,
    #[serde(default)]
    pub noise_floor: f64,
    #[serde(default)]
    pub features: serde_json::Value,
}

/// Training configuration submitted with a start request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingConfig {
    #[serde(default = "default_epochs")]
    pub epochs: u32,
    #[serde(default = "default_batch_size")]
    pub batch_size: u32,
    #[serde(default = "default_learning_rate")]
    pub learning_rate: f64,
    #[serde(default = "default_weight_decay")]
    pub weight_decay: f64,
    #[serde(default = "default_early_stopping_patience")]
    pub early_stopping_patience: u32,
    #[serde(default = "default_warmup_epochs")]
    pub warmup_epochs: u32,
    /// Path to a pretrained RVF model to fine-tune from.
    pub pretrained_rvf: Option<String>,
    /// LoRA profile name for environment-specific fine-tuning.
    pub lora_profile: Option<String>,
}

fn default_epochs() -> u32 {
    100
}
fn default_batch_size() -> u32 {
    8
}
fn default_learning_rate() -> f64 {
    0.001
}
fn default_weight_decay() -> f64 {
    1e-4
}
fn default_early_stopping_patience() -> u32 {
    20
}
fn default_warmup_epochs() -> u32 {
    5
}

impl Default for TrainingConfig {
    fn default() -> Self {
        Self {
            epochs: default_epochs(),
            batch_size: default_batch_size(),
            learning_rate: default_learning_rate(),
            weight_decay: default_weight_decay(),
            early_stopping_patience: default_early_stopping_patience(),
            warmup_epochs: default_warmup_epochs(),
            pretrained_rvf: None,
            lora_profile: None,
        }
    }
}

/// Request body for `POST /api/v1/train/start`.
#[derive(Debug, Deserialize)]
pub struct StartTrainingRequest {
    pub dataset_ids: Vec<String>,
    pub config: TrainingConfig,
}

/// Request body for `POST /api/v1/train/pretrain`.
#[derive(Debug, Deserialize)]
pub struct PretrainRequest {
    pub dataset_ids: Vec<String>,
    #[serde(default = "default_pretrain_epochs")]
    pub epochs: u32,
    #[serde(default = "default_learning_rate")]
    pub lr: f64,
}

fn default_pretrain_epochs() -> u32 {
    50
}

/// Request body for `POST /api/v1/train/lora`.
#[derive(Debug, Deserialize)]
pub struct LoraTrainRequest {
    pub base_model_id: String,
    pub dataset_ids: Vec<String>,
    pub profile_name: String,
    #[serde(default = "default_lora_rank")]
    pub rank: u8,
    #[serde(default = "default_lora_epochs")]
    pub epochs: u32,
}

fn default_lora_rank() -> u8 {
    8
}
fn default_lora_epochs() -> u32 {
    30
}

/// Current training status (returned by `GET /api/v1/train/status`).
///
/// NOTE (ADR-155 §2.1): `val_pck` / `best_pck` carry the **torso-HEIGHT** PCK
/// proxy from [`compute_pck_torso_height`] (pixel-space, nose→hip-midpoint),
/// which is **deliberately distinct** from the canonical hip↔hip
/// `wifi_densepose_train::pck_canonical`. The wire field names are kept for
/// API/UI back-compat, but these are torso-height progress proxies, NOT the
/// canonical reported-accuracy PCK@0.2 and must not be conflated with it.
/// `val_oks` is a rough `0.88 × pck` proxy, not a COCO OKS.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingStatus {
    pub active: bool,
    pub epoch: u32,
    pub total_epochs: u32,
    pub train_loss: f64,
    /// Torso-HEIGHT PCK@0.2 proxy (NOT canonical hip↔hip PCK — see struct doc).
    pub val_pck: f64,
    /// Rough OKS proxy (`0.88 × val_pck`), NOT a COCO OKS.
    pub val_oks: f64,
    pub lr: f64,
    /// Best torso-HEIGHT PCK@0.2 proxy seen so far (NOT canonical PCK).
    pub best_pck: f64,
    pub best_epoch: u32,
    pub patience_remaining: u32,
    pub eta_secs: Option<u64>,
    pub phase: String,
}

impl Default for TrainingStatus {
    fn default() -> Self {
        Self {
            active: false,
            epoch: 0,
            total_epochs: 0,
            train_loss: 0.0,
            val_pck: 0.0,
            val_oks: 0.0,
            lr: 0.0,
            best_pck: 0.0,
            best_epoch: 0,
            patience_remaining: 0,
            eta_secs: None,
            phase: "idle".to_string(),
        }
    }
}

/// Progress update sent over WebSocket.
///
/// NOTE (ADR-155 §2.1): `val_pck`/`val_oks` are the torso-HEIGHT PCK proxy and
/// its `0.88×` OKS proxy — NOT the canonical hip↔hip `pck_canonical`/COCO OKS.
/// See [`TrainingStatus`] and [`compute_pck_torso_height`].
#[derive(Debug, Clone, Serialize)]
pub struct TrainingProgress {
    pub epoch: u32,
    pub batch: u32,
    pub total_batches: u32,
    pub train_loss: f64,
    /// Torso-HEIGHT PCK@0.2 proxy (NOT canonical hip↔hip PCK).
    pub val_pck: f64,
    /// Rough OKS proxy (`0.88 × val_pck`), NOT a COCO OKS.
    pub val_oks: f64,
    pub lr: f64,
    pub phase: String,
}

/// Runtime training state stored in `AppStateInner`.
///
/// `status` and `cancel` are shared handles (not owned snapshots) so the
/// background training job can update progress and observe stop requests
/// **without holding a reference to the full `AppStateInner`**. That decoupling
/// is what makes the training core ([`run_training_job`]) unit-testable in
/// isolation from the ~60-field server state.
pub struct TrainingState {
    /// Live status snapshot, shared with the running training job.
    pub status: Arc<Mutex<TrainingStatus>>,
    /// Cooperative stop flag; `stop_training` sets it and the job loop observes it.
    pub cancel: Arc<AtomicBool>,
    /// Handle to the background training task.
    pub task_handle: Option<tokio::task::JoinHandle<()>>,
}

impl Default for TrainingState {
    fn default() -> Self {
        Self {
            status: Arc::new(Mutex::new(TrainingStatus::default())),
            cancel: Arc::new(AtomicBool::new(false)),
            task_handle: None,
        }
    }
}

impl TrainingState {
    /// Clone of the current status snapshot.
    pub fn snapshot(&self) -> TrainingStatus {
        self.status.lock().unwrap().clone()
    }

    /// Whether a training job is currently active.
    pub fn is_active(&self) -> bool {
        self.status.lock().unwrap().active
    }
}

/// Shared application state type.
pub type AppState = Arc<tokio::sync::RwLock<super::AppStateInner>>;

/// Feature normalization statistics computed from the training set.
/// Stored alongside the model weights inside the .rvf container so that
/// inference can apply the same normalization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureStats {
    /// Per-feature mean (length = n_features).
    pub mean: Vec<f64>,
    /// Per-feature standard deviation (length = n_features).
    pub std: Vec<f64>,
    /// Number of features.
    pub n_features: usize,
    /// Number of raw subcarriers used.
    pub n_subcarriers: usize,
}

// ── Data loading ─────────────────────────────────────────────────────────────

/// Parse one recording line, accepting both native training frames and the
/// `sensing_update` envelopes emitted by the dashboard recorder.
fn parse_recording_line(line: &str) -> Vec<RecordedFrame> {
    if let Ok(frame) = serde_json::from_str::<RecordedFrame>(line) {
        return vec![frame];
    }
    let value = match serde_json::from_str::<serde_json::Value>(line) {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    let timestamp = value.get("timestamp").and_then(serde_json::Value::as_f64);
    let nodes = value.get("nodes").and_then(serde_json::Value::as_array);
    match (timestamp, nodes) {
        (Some(timestamp), Some(nodes)) => nodes
            .iter()
            .filter_map(|node| {
                let subcarriers = node.get("amplitude")?.as_array()?.iter()
                    .map(serde_json::Value::as_f64).collect::<Option<Vec<_>>>()?;
                if subcarriers.is_empty() {
                    return None;
                }
                Some(RecordedFrame {
                    timestamp,
                    subcarriers,
                    rssi: node.get("rssi_dbm").and_then(serde_json::Value::as_f64).unwrap_or_default(),
                    noise_floor: node.get("noise_floor_dbm").and_then(serde_json::Value::as_f64).unwrap_or_default(),
                    features: serde_json::Value::Null,
                })
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Load CSI frames from recording files for the given dataset IDs.
///
/// Each dataset_id maps to a file at `data/recordings/{dataset_id}.csi.jsonl`.
/// If a file does not exist, it is silently skipped.
async fn load_recording_frames(dataset_ids: &[String]) -> Vec<RecordedFrame> {
    let mut all_frames = Vec::new();
    let recordings_dir = PathBuf::from(RECORDINGS_DIR);

    for id in dataset_ids {
        // Path-traversal guard (#615). Reject any dataset_id that contains
        // '/', '..', null bytes, or anything outside [A-Za-z0-9._-] BEFORE
        // building the format!() path. Otherwise an attacker could read any
        // file the server process can access via `dataset_ids: ["../../etc/passwd"]`.
        let safe = match crate::path_safety::safe_id(id) {
            Ok(s) => s,
            Err(e) => {
                warn!("Skipping invalid dataset_id {id:?}: {e}");
                continue;
            }
        };
        let candidates = [
            recordings_dir.join(format!("{safe}.csi.jsonl")),
            recordings_dir.join(format!("{safe}.jsonl")),
        ];
        let data = match tokio::fs::read_to_string(&candidates[0]).await {
            Ok(d) => d,
            Err(_) => match tokio::fs::read_to_string(&candidates[1]).await {
                Ok(d) => d,
                Err(e) => {
                    warn!("Could not read recording {}: {e}", candidates[0].display());
                    continue;
                }
            },
        };

        let mut line_count = 0u64;
        let mut parse_errors = 0u64;
        for line in data.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            line_count += 1;
            let parsed = parse_recording_line(line);
            if parsed.is_empty() {
                parse_errors += 1;
            } else {
                all_frames.extend(parsed);
            }
        }

        info!(
            "Loaded recording {id}: {line_count} lines, {} frames, {parse_errors} parse errors",
            all_frames.len()
        );
    }

    all_frames
}

/// Build fallback training frames from a snapshot of the live `frame_history`
/// buffer. Each `Vec<f64>` is one frame's subcarrier amplitude vector. Passed as
/// an owned snapshot (not a live `AppState` borrow) so the training core stays
/// state-free and independently testable.
fn frames_from_history(history: &[Vec<f64>]) -> Vec<RecordedFrame> {
    history
        .iter()
        .enumerate()
        .map(|(i, amplitudes)| RecordedFrame {
            timestamp: i as f64 * 0.1, // approximate 10 fps
            subcarriers: amplitudes.clone(),
            rssi: -50.0,
            noise_floor: -90.0,
            features: serde_json::json!({}),
        })
        .collect()
}

// ── Feature extraction ───────────────────────────────────────────────────────

/// Compute the total number of features that `extract_features_for_frame` produces
/// for a given subcarrier count.
fn feature_dim(n_sub: usize) -> usize {
    // subcarrier amplitudes + subcarrier variances + temporal gradients
    // + Goertzel freq bands + global scalars
    n_sub + n_sub + n_sub + N_FREQ_BANDS + N_GLOBAL_FEATURES
}

/// Goertzel algorithm: compute the power at a specific normalised frequency
/// from a signal buffer. `freq_norm` = target_freq_hz / sample_rate_hz.
fn goertzel_power(signal: &[f64], freq_norm: f64) -> f64 {
    let n = signal.len();
    if n == 0 {
        return 0.0;
    }
    let coeff = 2.0 * (2.0 * std::f64::consts::PI * freq_norm).cos();
    let mut s0 = 0.0f64;
    let mut s1 = 0.0f64;
    let mut s2;
    for &x in signal {
        s2 = s1;
        s1 = s0;
        s0 = x + coeff * s1 - s2;
    }
    let power = s0 * s0 + s1 * s1 - coeff * s0 * s1;
    (power / (n as f64)).max(0.0)
}

/// Extract feature vector for a single frame, given the sliding window context
/// of recent frames.
///
/// Returns a vector of length `feature_dim(n_sub)`.
fn extract_features_for_frame(
    frame: &RecordedFrame,
    window: &[&RecordedFrame],
    prev_frame: Option<&RecordedFrame>,
    sample_rate_hz: f64,
) -> Vec<f64> {
    let n_sub = frame.subcarriers.len().max(1);
    let mut features = Vec::with_capacity(feature_dim(n_sub));

    // 1. Raw subcarrier amplitudes (n_sub features).
    features.extend_from_slice(&frame.subcarriers);
    // Pad if shorter than expected.
    while features.len() < n_sub {
        features.push(0.0);
    }

    // 2. Per-subcarrier variance over the sliding window (n_sub features).
    for k in 0..n_sub {
        if window.is_empty() {
            features.push(0.0);
            continue;
        }
        let n = window.len() as f64;
        let mut sum = 0.0f64;
        let mut sq_sum = 0.0f64;
        for w in window {
            let a = if k < w.subcarriers.len() {
                w.subcarriers[k]
            } else {
                0.0
            };
            sum += a;
            sq_sum += a * a;
        }
        let mean = sum / n;
        let var = (sq_sum / n - mean * mean).max(0.0);
        features.push(var);
    }

    // 3. Temporal gradient vs previous frame (n_sub features).
    for k in 0..n_sub {
        let grad = match prev_frame {
            Some(prev) => {
                let cur = if k < frame.subcarriers.len() {
                    frame.subcarriers[k]
                } else {
                    0.0
                };
                let prv = if k < prev.subcarriers.len() {
                    prev.subcarriers[k]
                } else {
                    0.0
                };
                (cur - prv).abs()
            }
            None => 0.0,
        };
        features.push(grad);
    }

    // 4. Goertzel power at key frequency bands (N_FREQ_BANDS features).
    //    Bands: 0.1, 0.15, 0.2, 0.3, 0.4, 0.5, 1.0, 2.0, 3.0 Hz.
    let freq_bands = [0.1, 0.15, 0.2, 0.3, 0.4, 0.5, 1.0, 2.0, 3.0];
    // Build a mean-amplitude time series from the window.
    let ts: Vec<f64> = window
        .iter()
        .map(|w| {
            let n = w.subcarriers.len().max(1) as f64;
            w.subcarriers.iter().sum::<f64>() / n
        })
        .collect();
    for &freq_hz in &freq_bands {
        let freq_norm = if sample_rate_hz > 0.0 {
            freq_hz / sample_rate_hz
        } else {
            0.0
        };
        features.push(goertzel_power(&ts, freq_norm));
    }

    // 5. Global scalar features (N_GLOBAL_FEATURES = 3).
    let mean_amp = if frame.subcarriers.is_empty() {
        0.0
    } else {
        frame.subcarriers.iter().sum::<f64>() / frame.subcarriers.len() as f64
    };
    let std_amp = if frame.subcarriers.len() > 1 {
        let var = frame
            .subcarriers
            .iter()
            .map(|a| (a - mean_amp).powi(2))
            .sum::<f64>()
            / (frame.subcarriers.len() - 1) as f64;
        var.sqrt()
    } else {
        0.0
    };
    // Motion score: L2 change from previous frame, normalised.
    let motion_score = match prev_frame {
        Some(prev) => {
            let n_cmp = n_sub.min(prev.subcarriers.len());
            if n_cmp > 0 {
                let diff: f64 = (0..n_cmp)
                    .map(|k| {
                        let c = if k < frame.subcarriers.len() {
                            frame.subcarriers[k]
                        } else {
                            0.0
                        };
                        let p = if k < prev.subcarriers.len() {
                            prev.subcarriers[k]
                        } else {
                            0.0
                        };
                        (c - p).powi(2)
                    })
                    .sum::<f64>()
                    / n_cmp as f64;
                (diff / (mean_amp * mean_amp + 1e-9)).sqrt().clamp(0.0, 1.0)
            } else {
                0.0
            }
        }
        None => 0.0,
    };
    features.push(mean_amp);
    features.push(std_amp);
    features.push(motion_score);

    features
}

/// Compute teacher pose targets from a `RecordedFrame` using signal heuristics,
/// analogous to `derive_pose_from_sensing` in main.rs.
///
/// Returns a flat vector of length `N_TARGETS` (17 keypoints * 3 coordinates).
fn compute_teacher_targets(frame: &RecordedFrame, prev_frame: Option<&RecordedFrame>) -> Vec<f64> {
    let n_sub = frame.subcarriers.len().max(1);
    let mean_amp: f64 = frame.subcarriers.iter().sum::<f64>() / n_sub as f64;

    // Intra-frame variance.
    let variance: f64 = frame
        .subcarriers
        .iter()
        .map(|a| (a - mean_amp).powi(2))
        .sum::<f64>()
        / n_sub as f64;

    // Motion band power (upper half of subcarriers).
    let half = n_sub / 2;
    let motion_band_power = if half > 0 {
        frame.subcarriers[half..]
            .iter()
            .map(|a| (a - mean_amp).powi(2))
            .sum::<f64>()
            / (n_sub - half) as f64
    } else {
        0.0
    };

    // Breathing band power (lower half).
    let breathing_band_power = if half > 0 {
        frame.subcarriers[..half]
            .iter()
            .map(|a| (a - mean_amp).powi(2))
            .sum::<f64>()
            / half as f64
    } else {
        0.0
    };

    // Motion score.
    let motion_score = match prev_frame {
        Some(prev) => {
            let n_cmp = n_sub.min(prev.subcarriers.len());
            if n_cmp > 0 {
                let diff: f64 = (0..n_cmp)
                    .map(|k| {
                        let c = if k < frame.subcarriers.len() {
                            frame.subcarriers[k]
                        } else {
                            0.0
                        };
                        let p = if k < prev.subcarriers.len() {
                            prev.subcarriers[k]
                        } else {
                            0.0
                        };
                        (c - p).powi(2)
                    })
                    .sum::<f64>()
                    / n_cmp as f64;
                (diff / (mean_amp * mean_amp + 1e-9)).sqrt().clamp(0.0, 1.0)
            } else {
                0.0
            }
        }
        None => (variance / (mean_amp * mean_amp + 1e-9))
            .sqrt()
            .clamp(0.0, 1.0),
    };

    let is_walking = motion_score > 0.55;
    let breath_amp = (breathing_band_power * 4.0).clamp(0.0, 12.0);
    let breath_phase = (frame.timestamp * 0.25 * std::f64::consts::TAU).sin();

    // Dominant freq proxy.
    let peak_idx = frame
        .subcarriers
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)
        .unwrap_or(0);
    let dominant_freq_hz = peak_idx as f64 * 0.05;
    let lean_x = (dominant_freq_hz / 5.0 - 1.0).clamp(-1.0, 1.0) * 18.0;

    // Change points.
    let threshold = mean_amp * 1.2;
    let change_points = frame
        .subcarriers
        .windows(2)
        .filter(|w| (w[0] < threshold) != (w[1] < threshold))
        .count();
    let burst = (change_points as f64 / 8.0).clamp(0.0, 1.0);

    let noise_seed = variance * 31.7 + frame.timestamp * 17.3;
    let noise_val = (noise_seed.sin() * 43758.545).fract();

    // Stride.
    let stride_x = if is_walking {
        let stride_phase = (motion_band_power * 0.7 + frame.timestamp * 1.2).sin();
        stride_phase * 45.0 * motion_score
    } else {
        0.0
    };

    let snr_factor = ((variance - 0.5) / 10.0).clamp(0.0, 1.0);
    let base_confidence = (0.6 + 0.4 * snr_factor).clamp(0.0, 1.0);
    let _ = base_confidence; // used for confidence output, not target coords
    let _ = noise_val;

    // Base position on a 640x480 canvas.
    let base_x = 320.0 + stride_x + lean_x * 0.5;
    let base_y = 240.0 - motion_score * 8.0;

    // COCO 17-keypoint offsets from hip center.
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

    let mut targets = Vec::with_capacity(N_TARGETS);
    for (i, &(dx, dy)) in kp_offsets.iter().enumerate() {
        let breath_dx = if TORSO_KP.contains(&i) {
            let sign = if dx < 0.0 { -1.0 } else { 1.0 };
            sign * breath_amp * breath_phase * 0.5
        } else {
            0.0
        };
        let breath_dy = if TORSO_KP.contains(&i) {
            let sign = if dy < 0.0 { -1.0 } else { 1.0 };
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
            * variance.sqrt().clamp(0.0, 3.0)
            * motion_score;
        let kp_noise_y = ((noise_seed + i as f64 * 2.718).cos() * 31415.926).fract()
            * variance.sqrt().clamp(0.0, 3.0)
            * motion_score
            * 0.6;

        let swing_dy = if is_walking {
            let stride_phase = (motion_band_power * 0.7 + frame.timestamp * 1.2).sin();
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

        let x = base_x + dx + breath_dx + extremity_jitter.0 + kp_noise_x;
        let y = base_y + dy + breath_dy + extremity_jitter.1 + kp_noise_y + swing_dy;
        let z = 0.0; // depth placeholder

        targets.push(x);
        targets.push(y);
        targets.push(z);
    }

    targets
}

/// Build the feature matrix and target matrix from a set of recorded frames.
///
/// Returns `(feature_matrix, target_matrix, feature_stats)` where:
/// - `feature_matrix[i]` is the feature vector for frame `i`
/// - `target_matrix[i]` is the teacher target vector for frame `i`
/// - `feature_stats` contains per-feature mean/std for normalization
fn extract_features_and_targets(
    frames: &[RecordedFrame],
    sample_rate_hz: f64,
) -> (Vec<Vec<f64>>, Vec<Vec<f64>>, FeatureStats) {
    let n_sub = frames
        .first()
        .map(|f| f.subcarriers.len())
        .unwrap_or(DEFAULT_N_SUB)
        .max(1);
    let n_feat = feature_dim(n_sub);

    let mut feature_matrix: Vec<Vec<f64>> = Vec::with_capacity(frames.len());
    let mut target_matrix: Vec<Vec<f64>> = Vec::with_capacity(frames.len());

    for (i, frame) in frames.iter().enumerate() {
        // Build sliding window of up to VARIANCE_WINDOW preceding frames.
        let start = if i >= VARIANCE_WINDOW {
            i - VARIANCE_WINDOW
        } else {
            0
        };
        let window: Vec<&RecordedFrame> = frames[start..i].iter().collect();
        let prev = if i > 0 { Some(&frames[i - 1]) } else { None };

        let feats = extract_features_for_frame(frame, &window, prev, sample_rate_hz);
        let targets = compute_teacher_targets(frame, prev);

        feature_matrix.push(feats);
        target_matrix.push(targets);
    }

    // Compute feature statistics for normalization.
    let mut mean = vec![0.0f64; n_feat];
    let mut sq_mean = vec![0.0f64; n_feat];
    let n = feature_matrix.len() as f64;

    if n > 0.0 {
        for row in &feature_matrix {
            for (j, &val) in row.iter().enumerate() {
                if j < n_feat {
                    mean[j] += val;
                    sq_mean[j] += val * val;
                }
            }
        }
        for j in 0..n_feat {
            mean[j] /= n;
            sq_mean[j] /= n;
        }
    }

    let std_dev: Vec<f64> = (0..n_feat)
        .map(|j| {
            let var = (sq_mean[j] - mean[j] * mean[j]).max(0.0);
            let s = var.sqrt();
            if s < 1e-9 {
                1.0
            } else {
                s
            } // avoid division by zero
        })
        .collect();

    // Normalize feature matrix in place.
    for row in &mut feature_matrix {
        for (j, val) in row.iter_mut().enumerate() {
            if j < n_feat {
                *val = (*val - mean[j]) / std_dev[j];
            }
        }
    }

    let stats = FeatureStats {
        mean,
        std: std_dev,
        n_features: n_feat,
        n_subcarriers: n_sub,
    };

    (feature_matrix, target_matrix, stats)
}

// ── Linear algebra helpers (no external deps) ────────────────────────────────

/// Compute mean squared error between predicted and target matrices.
fn compute_mse(predictions: &[Vec<f64>], targets: &[Vec<f64>]) -> f64 {
    if predictions.is_empty() {
        return 0.0;
    }
    let n = predictions.len() as f64;
    let total: f64 = predictions
        .iter()
        .zip(targets.iter())
        .map(|(pred, tgt)| {
            pred.iter()
                .zip(tgt.iter())
                .map(|(p, t)| (p - t).powi(2))
                .sum::<f64>()
        })
        .sum();
    total / (n * predictions[0].len().max(1) as f64)
}

/// Compute **PCK_torso-height@`threshold`** — a metric DELIBERATELY DISTINCT
/// from the canonical hip↔hip PCK (`wifi_densepose_train::pck_canonical`).
///
/// # Why this is `_torso_height`, not the canonical PCK (ADR-155 §2.1 / §8 — RESOLVED)
///
/// ADR-155 unified the workspace's reported-accuracy PCK to ONE definition:
/// **hip↔hip torso WIDTH**, on `[0,1]`-normalized `[17,2]` keypoints. This
/// live-server function is **not** that metric and must never be conflated
/// with it. It is genuinely different on three load-bearing axes:
///
/// 1. **Coordinate space.** It operates on **pixel-space** teacher targets on a
///    640×480 canvas (`compute_teacher_targets`), not `[0,1]` MM-Fi coords —
///    hence the `.max(50.0)` *pixel* torso floor below.
/// 2. **Normalization axis.** It normalizes by torso **HEIGHT** (vertical
///    nose→hip-midpoint distance), not canonical torso **WIDTH** (hip↔hip).
///    Routing through `pck_canonical` would silently change which body axis
///    sets the scale, altering every live number this drives.
/// 3. **Layout.** It consumes `[17×3]`-flattened `Vec<Vec<f64>>` (x,y,z), not
///    `ndarray::Array2<f32>`; `wifi-densepose-sensing-server` does not depend on
///    `wifi-densepose-train` or `ndarray`.
///
/// Because the math is load-bearing (a running training service's progress
/// display), ADR-155 Milestone-1 resolves the label collision by **relabelling**
/// rather than forcing a false identity: the function and the metric it produces
/// are named `_torso_height` everywhere they surface (this fn, the log line),
/// and the `val_pck`/`best_pck` API fields document the divergence. The reported
/// in-loop value is a torso-HEIGHT PCK proxy on heuristic teacher targets — it is
/// NOT a claim-grade accuracy number and is NOT the canonical hip↔hip PCK@0.2.
fn compute_pck_torso_height(
    predictions: &[Vec<f64>],
    targets: &[Vec<f64>],
    threshold_ratio: f64,
) -> f64 {
    if predictions.is_empty() {
        return 0.0;
    }
    let mut correct = 0u64;
    let mut total = 0u64;

    for (pred, tgt) in predictions.iter().zip(targets.iter()) {
        // Compute torso height from target.
        // nose = kp 0 (indices 0,1,2), left_hip = kp 11 (33,34,35), right_hip = kp 12 (36,37,38)
        let torso_h = if tgt.len() >= N_TARGETS {
            let nose_y = tgt[1];
            let hip_y = (tgt[11 * 3 + 1] + tgt[12 * 3 + 1]) / 2.0;
            (hip_y - nose_y).abs().max(50.0) // minimum 50px torso height
        } else {
            100.0
        };
        let thresh = torso_h * threshold_ratio;

        for k in 0..N_KEYPOINTS {
            let px = pred.get(k * 3).copied().unwrap_or(0.0);
            let py = pred.get(k * 3 + 1).copied().unwrap_or(0.0);
            let tx = tgt.get(k * 3).copied().unwrap_or(0.0);
            let ty = tgt.get(k * 3 + 1).copied().unwrap_or(0.0);
            let dist = ((px - tx).powi(2) + (py - ty).powi(2)).sqrt();
            if dist < thresh {
                correct += 1;
            }
            total += 1;
        }
    }

    if total == 0 {
        0.0
    } else {
        correct as f64 / total as f64
    }
}

/// Forward pass: compute predictions = X @ W^T + bias for all samples.
///
/// `weights` is stored row-major: shape [n_targets, n_features].
/// `bias` has shape [n_targets].
fn forward(
    features: &[Vec<f64>],
    weights: &[f64],
    bias: &[f64],
    n_features: usize,
    n_targets: usize,
) -> Vec<Vec<f64>> {
    features
        .iter()
        .map(|x| {
            (0..n_targets)
                .map(|t| {
                    let mut sum = bias.get(t).copied().unwrap_or(0.0);
                    let row_start = t * n_features;
                    for j in 0..n_features {
                        let xj = x.get(j).copied().unwrap_or(0.0);
                        let wj = weights.get(row_start + j).copied().unwrap_or(0.0);
                        sum += wj * xj;
                    }
                    sum
                })
                .collect()
        })
        .collect()
}

/// Simple deterministic shuffle using a seed-based index permutation.
/// Uses a linear congruential generator for reproducibility without `rand`.
fn deterministic_shuffle(n: usize, seed: u64) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..n).collect();
    if n <= 1 {
        return indices;
    }
    // Fisher-Yates with LCG.
    let mut rng = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    for i in (1..n).rev() {
        rng = rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let j = (rng >> 33) as usize % (i + 1);
        indices.swap(i, j);
    }
    indices
}

// ── Real training loop ───────────────────────────────────────────────────────

/// Real training loop that trains a linear CSI-to-pose model using recorded data.
///
/// Loads CSI frames from `.csi.jsonl` recording files, extracts signal features
/// (subcarrier amplitudes, variance, temporal gradients, Goertzel frequency power),
/// computes teacher pose targets using signal heuristics, and trains a regularised
/// linear model via mini-batch gradient descent.
///
/// On completion, exports a `.rvf` container with real calibrated weights.
async fn run_training_job(
    status: Arc<Mutex<TrainingStatus>>,
    cancel: Arc<AtomicBool>,
    progress_tx: broadcast::Sender<String>,
    config: TrainingConfig,
    dataset_ids: Vec<String>,
    history_snapshot: Vec<Vec<f64>>,
    training_type: &str,
) -> Option<PathBuf> {
    let total_epochs = config.epochs;
    let patience = config.early_stopping_patience;
    let mut best_pck = 0.0f64;
    let mut best_epoch = 0u32;
    let mut patience_remaining = patience;
    let sample_rate_hz = 10.0; // default 10 fps

    info!(
        "Real {training_type} training started: {total_epochs} epochs, lr={}, lambda={}",
        config.learning_rate, config.weight_decay
    );

    // ── Phase 1: Load data ───────────────────────────────────────────────────

    {
        let progress = TrainingProgress {
            epoch: 0,
            batch: 0,
            total_batches: 0,
            train_loss: 0.0,
            val_pck: 0.0,
            val_oks: 0.0,
            lr: 0.0,
            phase: "loading_data".to_string(),
        };
        if let Ok(json) = serde_json::to_string(&progress) {
            let _ = progress_tx.send(json);
        }
    }

    let mut frames = load_recording_frames(&dataset_ids).await;
    if frames.is_empty() && dataset_ids.is_empty() {
        info!("No recordings found for dataset_ids; falling back to live frame_history");
        frames = frames_from_history(&history_snapshot);
    } else if frames.is_empty() {
        warn!("Selected recordings contained no usable CSI frames; refusing live-history fallback");
    }

    if frames.len() < 10 {
        warn!(
            "Insufficient training data: only {} frames (minimum 10 required). Aborting.",
            frames.len()
        );
        let fail = TrainingProgress {
            epoch: 0,
            batch: 0,
            total_batches: 0,
            train_loss: 0.0,
            val_pck: 0.0,
            val_oks: 0.0,
            lr: 0.0,
            phase: "failed_insufficient_data".to_string(),
        };
        if let Ok(json) = serde_json::to_string(&fail) {
            let _ = progress_tx.send(json);
        }
        {
            let mut st = status.lock().unwrap();
            st.active = false;
            st.phase = "failed".to_string();
        }
        return None;
    }

    info!("Loaded {} frames for training", frames.len());

    // ── Phase 2: Extract features and targets ────────────────────────────────

    {
        let progress = TrainingProgress {
            epoch: 0,
            batch: 0,
            total_batches: 0,
            train_loss: 0.0,
            val_pck: 0.0,
            val_oks: 0.0,
            lr: 0.0,
            phase: "extracting_features".to_string(),
        };
        if let Ok(json) = serde_json::to_string(&progress) {
            let _ = progress_tx.send(json);
        }
    }

    // Yield to avoid blocking the event loop during feature extraction.
    tokio::task::yield_now().await;

    let (feature_matrix, target_matrix, feature_stats) =
        extract_features_and_targets(&frames, sample_rate_hz);

    let n_feat = feature_stats.n_features;
    let n_samples = feature_matrix.len();

    info!(
        "Features extracted: {} samples, {} features/sample, {} targets/sample",
        n_samples, n_feat, N_TARGETS
    );

    // ── Phase 3: Train/val split (80/20) ─────────────────────────────────────

    let split_idx = (n_samples * 4) / 5;
    let (train_x, val_x) = feature_matrix.split_at(split_idx);
    let (train_y, val_y) = target_matrix.split_at(split_idx);
    let n_train = train_x.len();
    let n_val = val_x.len();

    info!("Train/val split: {n_train} train, {n_val} val");

    // ── Phase 4: Initialize weights ──────────────────────────────────────────

    // Weights: [N_TARGETS, n_feat] stored row-major.
    let n_weights = N_TARGETS * n_feat;
    let mut weights = vec![0.0f64; n_weights];
    let mut bias = vec![0.0f64; N_TARGETS];

    // Xavier initialization: scale = sqrt(2 / (n_in + n_out)).
    let xavier_scale = (2.0 / (n_feat as f64 + N_TARGETS as f64)).sqrt();
    // Deterministic pseudo-random initialization.
    for i in 0..n_weights {
        let seed = i as f64 * 1.618033988749895 + 0.5;
        weights[i] = (seed.fract() * 2.0 - 1.0) * xavier_scale;
    }

    // Best weights snapshot for early stopping.
    let mut best_weights = weights.clone();
    let mut best_bias = bias.clone();
    let mut best_val_loss = f64::MAX;

    let batch_size = config.batch_size.max(1) as usize;
    let total_batches = ((n_train + batch_size - 1) / batch_size) as u32;

    // Epoch timing for ETA.
    let training_start = std::time::Instant::now();

    // ── Phase 5: Training loop ───────────────────────────────────────────────

    for epoch in 1..=total_epochs {
        // Check cancellation (cooperative stop flag set by `stop_training`).
        if cancel.load(Ordering::Relaxed) {
            info!("Training cancelled at epoch {epoch}");
            break;
        }

        let phase = if epoch <= config.warmup_epochs {
            "warmup"
        } else {
            "training"
        };

        // Learning rate schedule: linear warmup then cosine decay.
        let lr = if epoch <= config.warmup_epochs {
            config.learning_rate * (epoch as f64 / config.warmup_epochs.max(1) as f64)
        } else {
            let progress_ratio = (epoch - config.warmup_epochs) as f64
                / (total_epochs - config.warmup_epochs).max(1) as f64;
            config.learning_rate * (1.0 + (std::f64::consts::PI * progress_ratio).cos()) / 2.0
        };

        let lambda = config.weight_decay;

        // Deterministic shuffle of training indices.
        let indices = deterministic_shuffle(n_train, epoch as u64);

        let mut epoch_loss = 0.0f64;
        let mut epoch_batches = 0u32;

        for batch_start_idx in (0..n_train).step_by(batch_size) {
            let batch_end = (batch_start_idx + batch_size).min(n_train);
            let actual_batch_size = batch_end - batch_start_idx;
            if actual_batch_size == 0 {
                continue;
            }

            // Gather batch.
            let batch_x: Vec<&Vec<f64>> = indices[batch_start_idx..batch_end]
                .iter()
                .map(|&idx| &train_x[idx])
                .collect();
            let batch_y: Vec<&Vec<f64>> = indices[batch_start_idx..batch_end]
                .iter()
                .map(|&idx| &train_y[idx])
                .collect();

            // Forward pass.
            let bs = actual_batch_size as f64;

            // Compute gradients: dW = (1/bs) * sum_i (pred_i - y_i) x_i^T + lambda * W
            //                    db = (1/bs) * sum_i (pred_i - y_i)
            let mut grad_w = vec![0.0f64; n_weights];
            let mut grad_b = vec![0.0f64; N_TARGETS];
            let mut batch_loss = 0.0f64;

            for (x, y) in batch_x.iter().zip(batch_y.iter()) {
                // Compute prediction for this sample.
                for t in 0..N_TARGETS {
                    let row_start = t * n_feat;
                    let mut pred = bias[t];
                    for j in 0..n_feat {
                        let xj = x.get(j).copied().unwrap_or(0.0);
                        pred += weights[row_start + j] * xj;
                    }
                    let tgt = y.get(t).copied().unwrap_or(0.0);
                    let error = pred - tgt;
                    batch_loss += error * error;

                    // Accumulate gradients.
                    grad_b[t] += error;
                    for j in 0..n_feat {
                        let xj = x.get(j).copied().unwrap_or(0.0);
                        grad_w[row_start + j] += error * xj;
                    }
                }
            }

            batch_loss /= bs * N_TARGETS as f64;
            epoch_loss += batch_loss;
            epoch_batches += 1;

            // Apply gradients with L2 regularization.
            for i in 0..n_weights {
                weights[i] -= lr * (grad_w[i] / bs + lambda * weights[i]);
            }
            for t in 0..N_TARGETS {
                bias[t] -= lr * grad_b[t] / bs;
            }

            // Send batch progress.
            let batch_num = epoch_batches;
            let progress = TrainingProgress {
                epoch,
                batch: batch_num,
                total_batches,
                train_loss: batch_loss,
                val_pck: 0.0,
                val_oks: 0.0,
                lr,
                phase: phase.to_string(),
            };
            if let Ok(json) = serde_json::to_string(&progress) {
                let _ = progress_tx.send(json);
            }

            // Yield periodically to keep the event loop responsive.
            if batch_num % 5 == 0 {
                tokio::task::yield_now().await;
            }
        }

        let train_loss = if epoch_batches > 0 {
            epoch_loss / epoch_batches as f64
        } else {
            0.0
        };

        // ── Validation ──────────────────────────────────────────────────

        let val_preds = forward(val_x, &weights, &bias, n_feat, N_TARGETS);
        let val_mse = compute_mse(&val_preds, val_y);
        // torso-HEIGHT PCK proxy (NOT canonical hip↔hip PCK@0.2 — see
        // compute_pck_torso_height / ADR-155 §2.1). Surfaced as `val_pck` for
        // wire-format back-compat but is a torso-height proxy, not a claim.
        let val_pck = compute_pck_torso_height(&val_preds, val_y, 0.2);
        let val_oks = val_pck * 0.88; // rough OKS proxy from torso-height PCK (NOT canonical OKS)

        let val_progress = TrainingProgress {
            epoch,
            batch: total_batches,
            total_batches,
            train_loss,
            val_pck,
            val_oks,
            lr,
            phase: "validation".to_string(),
        };
        if let Ok(json) = serde_json::to_string(&val_progress) {
            let _ = progress_tx.send(json);
        }

        // Track best model by validation loss (lower is better).
        if val_pck > best_pck {
            best_pck = val_pck;
            best_epoch = epoch;
            best_weights = weights.clone();
            best_bias = bias.clone();
            best_val_loss = val_mse;
            patience_remaining = patience;
        } else {
            patience_remaining = patience_remaining.saturating_sub(1);
        }

        // ETA estimate.
        let elapsed_secs = training_start.elapsed().as_secs();
        let secs_per_epoch = if epoch > 0 {
            elapsed_secs as f64 / epoch as f64
        } else {
            0.0
        };
        let remaining = total_epochs.saturating_sub(epoch);
        let eta_secs = (remaining as f64 * secs_per_epoch) as u64;

        // Update the shared status snapshot (read by GET /api/v1/train/status).
        {
            let mut st = status.lock().unwrap();
            *st = TrainingStatus {
                active: true,
                epoch,
                total_epochs,
                train_loss,
                val_pck,
                val_oks,
                lr,
                best_pck,
                best_epoch,
                patience_remaining,
                eta_secs: Some(eta_secs),
                phase: phase.to_string(),
            };
        }

        // Logs label this `pck_torso_h@0.2` so it is never read as the canonical
        // hip↔hip PCK@0.2 (ADR-155 §2.1). It is a torso-HEIGHT proxy on heuristic
        // teacher targets, not a claim-grade accuracy number.
        info!(
            "Epoch {epoch}/{total_epochs}: loss={train_loss:.6}, pck_torso_h@0.2={val_pck:.4}, \
             val_mse={val_mse:.4}, best_pck_torso_h={best_pck:.4}@{best_epoch}, patience={patience_remaining}"
        );

        // Early stopping.
        if patience_remaining == 0 {
            info!("Early stopping at epoch {epoch} (best={best_epoch}, pck_torso_h@0.2={best_pck:.4})");
            let stop_progress = TrainingProgress {
                epoch,
                batch: total_batches,
                total_batches,
                train_loss,
                val_pck,
                val_oks,
                lr,
                phase: "early_stopped".to_string(),
            };
            if let Ok(json) = serde_json::to_string(&stop_progress) {
                let _ = progress_tx.send(json);
            }
            break;
        }

        // Yield between epochs.
        tokio::task::yield_now().await;
    }

    // ── Phase 6: Export .rvf model ───────────────────────────────────────────

    let completed_phase = if cancel.load(Ordering::Relaxed) {
        "cancelled"
    } else {
        "completed"
    };
    let mut written_rvf: Option<PathBuf> = None;

    // Emit completion message.
    let completion = TrainingProgress {
        epoch: best_epoch,
        batch: 0,
        total_batches: 0,
        train_loss: best_val_loss,
        val_pck: best_pck,
        val_oks: best_pck * 0.88,
        lr: 0.0,
        phase: completed_phase.to_string(),
    };
    if let Ok(json) = serde_json::to_string(&completion) {
        let _ = progress_tx.send(json);
    }

    if completed_phase == "completed" || completed_phase == "early_stopped" {
        if let Err(e) = tokio::fs::create_dir_all(MODELS_DIR).await {
            error!("Failed to create models directory: {e}");
        } else {
            let model_id = next_model_id(training_type);
            let rvf_path = PathBuf::from(MODELS_DIR).join(format!("{model_id}.rvf"));

            let mut builder = RvfBuilder::new();

            // SEG_MANIFEST: model identity and configuration.
            builder.add_manifest(
                &model_id,
                env!("CARGO_PKG_VERSION"),
                &format!(
                    "WiFi DensePose {training_type} model (linear, {} features, {} targets)",
                    n_feat, N_TARGETS
                ),
            );

            // SEG_META: feature normalization stats + model config.
            builder.add_metadata(&serde_json::json!({
                "training": {
                    "type": training_type,
                    "epochs": total_epochs,
                    "best_epoch": best_epoch,
                    "best_pck": best_pck,
                    "best_oks": best_pck * 0.88,
                    "best_val_loss": best_val_loss,
                    "simulated": false,
                    "n_train_samples": n_train,
                    "n_val_samples": n_val,
                    "n_features": n_feat,
                    "n_targets": N_TARGETS,
                    "n_subcarriers": feature_stats.n_subcarriers,
                    "batch_size": config.batch_size,
                    "learning_rate": config.learning_rate,
                    "weight_decay": config.weight_decay,
                },
                "feature_stats": feature_stats,
                "model_config": {
                    "type": "linear",
                    "n_features": n_feat,
                    "n_targets": N_TARGETS,
                    "n_keypoints": N_KEYPOINTS,
                    "dims_per_keypoint": DIMS_PER_KP,
                    "n_subcarriers": feature_stats.n_subcarriers,
                }
            }));

            // SEG_VEC: real trained weights.
            // Layout: [weights (N_TARGETS * n_feat), bias (N_TARGETS)] as f32.
            let total_params = best_weights.len() + best_bias.len();
            let mut model_weights_f32: Vec<f32> = Vec::with_capacity(total_params);
            for &w in &best_weights {
                model_weights_f32.push(w as f32);
            }
            for &b in &best_bias {
                model_weights_f32.push(b as f32);
            }
            builder.add_weights(&model_weights_f32);

            // SEG_WITNESS: training attestation with metrics.
            let training_hash = format!(
                "sha256:{:016x}{:016x}",
                best_weights.len() as u64,
                (best_pck * 1e9) as u64
            );
            builder.add_witness(
                &training_hash,
                &serde_json::json!({
                    "best_pck": best_pck,
                    "best_epoch": best_epoch,
                    "val_loss": best_val_loss,
                    "n_train": n_train,
                    "n_val": n_val,
                    "n_features": n_feat,
                    "training_type": training_type,
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                }),
            );

            match builder.write_to_file(&rvf_path) {
                Err(e) => {
                    error!("Failed to write trained model RVF: {e}");
                }
                Ok(()) => {
                    info!(
                        "Trained model saved: {} ({} params, pck_torso_h@0.2={:.4})",
                        rvf_path.display(),
                        total_params,
                        best_pck
                    );
                    written_rvf = Some(rvf_path);
                }
            }
        }
    }

    // Mark training as inactive in the shared status snapshot.
    {
        let mut st = status.lock().unwrap();
        st.active = false;
        st.phase = completed_phase.to_string();
    }

    info!("Real {training_type} training finished: phase={completed_phase}");
    written_rvf
}

// ── Public inference function ────────────────────────────────────────────────

/// Apply a trained linear model to current CSI features to produce pose keypoints.
///
/// The `model_weights` slice is expected to contain the weights and bias
/// concatenated as stored in the RVF container's SEG_VEC segment:
///   `[W: N_TARGETS * n_features f32 values][bias: N_TARGETS f32 values]`
///
/// `feature_stats` provides the mean and std used during training for
/// normalization of the raw feature vector.
///
/// `raw_subcarriers` is the current frame's subcarrier amplitudes.
/// `frame_history` is the sliding window of recent frames for temporal features.
/// `prev_subcarriers` is the previous frame's amplitudes for gradient computation.
///
/// Returns 17 keypoints as `[x, y, z, confidence]`.
pub fn infer_pose_from_model(
    model_weights: &[f32],
    feature_stats: &FeatureStats,
    raw_subcarriers: &[f64],
    frame_history: &VecDeque<Vec<f64>>,
    prev_subcarriers: Option<&[f64]>,
    sample_rate_hz: f64,
) -> Vec<[f64; 4]> {
    let n_feat = feature_stats.n_features;
    let expected_params = N_TARGETS * n_feat + N_TARGETS;

    if model_weights.len() < expected_params {
        warn!(
            "Model weights too short: {} < {} expected",
            model_weights.len(),
            expected_params
        );
        return default_keypoints();
    }

    // Build a synthetic RecordedFrame for the feature extractor.
    let current_frame = RecordedFrame {
        timestamp: 0.0,
        subcarriers: raw_subcarriers.to_vec(),
        rssi: -50.0,
        noise_floor: -90.0,
        features: serde_json::json!({}),
    };

    let prev_frame = prev_subcarriers.map(|subs| RecordedFrame {
        timestamp: -0.1,
        subcarriers: subs.to_vec(),
        rssi: -50.0,
        noise_floor: -90.0,
        features: serde_json::json!({}),
    });

    // Build window from frame_history.
    let window_frames: Vec<RecordedFrame> = frame_history
        .iter()
        .rev()
        .take(VARIANCE_WINDOW)
        .rev()
        .map(|amps| RecordedFrame {
            timestamp: 0.0,
            subcarriers: amps.clone(),
            rssi: -50.0,
            noise_floor: -90.0,
            features: serde_json::json!({}),
        })
        .collect();
    let window_refs: Vec<&RecordedFrame> = window_frames.iter().collect();

    // Extract features.
    let mut features = extract_features_for_frame(
        &current_frame,
        &window_refs,
        prev_frame.as_ref(),
        sample_rate_hz,
    );

    // Normalize features.
    for (j, val) in features.iter_mut().enumerate() {
        if j < n_feat {
            let m = feature_stats.mean.get(j).copied().unwrap_or(0.0);
            let s = feature_stats.std.get(j).copied().unwrap_or(1.0);
            *val = (*val - m) / s;
        }
    }

    // Ensure feature vector length matches.
    features.resize(n_feat, 0.0);

    // Matrix multiply: for each target t, output[t] = W[t] . x + bias[t].
    let weights_end = N_TARGETS * n_feat;
    let mut keypoints = Vec::with_capacity(N_KEYPOINTS);

    for k in 0..N_KEYPOINTS {
        let mut coords = [0.0f64; 4]; // x, y, z, confidence
        for d in 0..DIMS_PER_KP {
            let t = k * DIMS_PER_KP + d;
            let row_start = t * n_feat;
            let mut sum = model_weights
                .get(weights_end + t)
                .map(|&b| b as f64)
                .unwrap_or(0.0);
            for j in 0..n_feat {
                let w = model_weights
                    .get(row_start + j)
                    .map(|&v| v as f64)
                    .unwrap_or(0.0);
                sum += w * features[j];
            }
            coords[d] = sum;
        }

        // Confidence based on feature quality: mean absolute value of normalized features.
        let feat_magnitude: f64 =
            features.iter().map(|v| v.abs()).sum::<f64>() / features.len().max(1) as f64;
        coords[3] = (1.0 / (1.0 + (-feat_magnitude + 1.0).exp())).clamp(0.1, 0.99);

        keypoints.push(coords);
    }

    keypoints
}

/// Return default zero-confidence keypoints when inference cannot be performed.
fn default_keypoints() -> Vec<[f64; 4]> {
    vec![[320.0, 240.0, 0.0, 0.0]; N_KEYPOINTS]
}

// ── Server-training enablement gate (ADR-186 P5) ─────────────────────────────

/// Env var that opts a deployment out of in-server training (e.g. the
/// lightweight appliance image without recordings). When set truthy, the start
/// endpoints return a structured `enabled:false` response pointing at the CLI —
/// never a silent `success:true` no-op.
const DISABLE_ENV: &str = "RUVIEW_DISABLE_SERVER_TRAINING";

/// Whether in-server training is enabled for this deployment.
fn server_training_enabled() -> bool {
    training_enabled_from_env(std::env::var(DISABLE_ENV).ok().as_deref())
}

/// Pure decision (unit-testable without touching process env): enabled unless
/// the flag is a truthy disable value.
fn training_enabled_from_env(flag: Option<&str>) -> bool {
    match flag {
        Some(v) => {
            let v = v.trim();
            !(v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes"))
        }
        None => true,
    }
}

/// Structured, honest "server training is off for this build — use the CLI"
/// response (HTTP 409). Guarantees no silent no-op in the disabled config.
fn disabled_response() -> Response {
    (
        StatusCode::CONFLICT,
        Json(serde_json::json!({
            "status": "error",
            "enabled": false,
            "reason": "In-server training is disabled for this deployment.",
            "cli": "wifi-densepose train-room",
            // `detail` is surfaced verbatim by the dashboard's API client.
            "detail": "In-server training is disabled on this build. Train from the CLI: wifi-densepose train-room",
        })),
    )
        .into_response()
}

// ── Axum handlers ────────────────────────────────────────────────────────────

async fn start_training(
    State(state): State<AppState>,
    Json(body): Json<StartTrainingRequest>,
) -> Response {
    if !server_training_enabled() {
        return disabled_response();
    }
    let config = body.config.clone();
    match spawn_training_job(&state, config, body.dataset_ids.clone(), "supervised").await {
        Ok(()) => Json(serde_json::json!({
            "status": "started",
            "type": "supervised",
            "dataset_ids": body.dataset_ids,
            "config": body.config,
        }))
        .into_response(),
        Err(active) => Json(active_error(&active)).into_response(),
    }
}

/// Snapshot of the already-running job returned when a start is rejected.
fn active_error(snap: &TrainingStatus) -> serde_json::Value {
    serde_json::json!({
        "status": "error",
        "message": "Training is already active. Stop it first.",
        "current_epoch": snap.epoch,
        "total_epochs": snap.total_epochs,
    })
}

/// Seed the shared status, snapshot `frame_history`, and spawn the background
/// training job. Returns `Err(current_status)` if a job is already active.
///
/// Centralises the single-job guard + spawn used by the supervised, pretrain,
/// and LoRA start handlers so they cannot diverge.
/// Atomically claim the single training slot.
///
/// Checks `active` and sets it `true` **in one `status` lock scope**, so two
/// concurrent callers cannot both observe the slot free — the first claims it,
/// the second gets `Err(current_status)`. Returns the seeded status on success.
///
/// This is the fix for a TOCTOU race: the previous code checked `is_active()`
/// under a `state` READ lock, released it, and only afterward set `active`.
/// A `tokio::RwLock` read lock is shared, so two starts could both hold it, both
/// see the slot inactive, both proceed — spawning two jobs that then share and
/// overwrite one status/cancel and orphan a task handle. The claim's atomicity
/// lives on the `status` mutex, not the coarse `state` lock, which also keeps it
/// unit-testable without a full `AppState`.
fn claim_training_slot(
    status: &Mutex<TrainingStatus>,
    config: &TrainingConfig,
) -> Result<(), TrainingStatus> {
    let mut st = status.lock().unwrap();
    if st.active {
        return Err(st.clone());
    }
    *st = TrainingStatus {
        active: true,
        total_epochs: config.epochs,
        lr: config.learning_rate,
        patience_remaining: config.early_stopping_patience,
        phase: "initializing".to_string(),
        ..Default::default()
    };
    Ok(())
}

async fn spawn_training_job(
    state: &AppState,
    config: TrainingConfig,
    dataset_ids: Vec<String>,
    training_type: &'static str,
) -> Result<(), TrainingStatus> {
    // Grab the shared handles under a read lock; the RwLock is only guarding
    // access to the Arcs, not the single-job decision.
    let (progress_tx, status, cancel, history_snapshot) = {
        let s = state.read().await;
        (
            s.training_progress_tx.clone(),
            s.training_state.status.clone(),
            s.training_state.cancel.clone(),
            s.frame_history.iter().cloned().collect::<Vec<_>>(),
        )
    };

    // Atomic check-and-set on the status mutex. This — not the read lock above —
    // is what serialises concurrent starts (see `claim_training_slot`).
    claim_training_slot(&status, &config)?;
    cancel.store(false, Ordering::Relaxed);

    let handle = tokio::spawn(async move {
        run_training_job(
            status,
            cancel,
            progress_tx,
            config,
            dataset_ids,
            history_snapshot,
            training_type,
        )
        .await;
    });

    {
        let mut s = state.write().await;
        s.training_state.task_handle = Some(handle);
    }

    Ok(())
}

async fn stop_training(State(state): State<AppState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    if !s.training_state.is_active() {
        return Json(serde_json::json!({
            "status": "error",
            "message": "No training is currently active.",
        }));
    }

    // Set the cooperative stop flag; the background job observes it between
    // epochs and exits gracefully after the current batch. We do not abort the
    // task handle.
    s.training_state.cancel.store(true, Ordering::Relaxed);
    {
        let mut st = s.training_state.status.lock().unwrap();
        st.phase = "stopping".to_string();
    }
    let snap = s.training_state.snapshot();

    info!("Training stop requested");

    Json(serde_json::json!({
        "status": "stopping",
        "epoch": snap.epoch,
        "best_pck": snap.best_pck,
    }))
}

async fn training_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    let mut value = serde_json::to_value(s.training_state.snapshot()).unwrap_or_default();
    // Surface the enablement flag so the dashboard can honestly disable the
    // Start button (with a CLI tooltip) without first firing a POST (ADR-186 P5).
    if let Some(obj) = value.as_object_mut() {
        obj.insert(
            "enabled".to_string(),
            serde_json::Value::Bool(server_training_enabled()),
        );
    }
    Json(value)
}

async fn start_pretrain(
    State(state): State<AppState>,
    Json(body): Json<PretrainRequest>,
) -> Response {
    if !server_training_enabled() {
        return disabled_response();
    }
    let config = TrainingConfig {
        epochs: body.epochs,
        learning_rate: body.lr,
        warmup_epochs: (body.epochs / 10).max(1),
        early_stopping_patience: body.epochs + 1, // no early stopping for pretrain
        ..Default::default()
    };

    match spawn_training_job(&state, config, body.dataset_ids.clone(), "pretrain").await {
        Ok(()) => Json(serde_json::json!({
            "status": "started",
            "type": "pretrain",
            "epochs": body.epochs,
            "lr": body.lr,
            "dataset_ids": body.dataset_ids,
        }))
        .into_response(),
        Err(active) => Json(active_error(&active)).into_response(),
    }
}

async fn start_lora_training(
    State(state): State<AppState>,
    Json(body): Json<LoraTrainRequest>,
) -> Response {
    if !server_training_enabled() {
        return disabled_response();
    }
    let config = TrainingConfig {
        epochs: body.epochs,
        learning_rate: 0.0005, // lower LR for LoRA
        warmup_epochs: 2,
        early_stopping_patience: 10,
        pretrained_rvf: Some(body.base_model_id.clone()),
        lora_profile: Some(body.profile_name.clone()),
        ..Default::default()
    };

    match spawn_training_job(&state, config, body.dataset_ids.clone(), "lora").await {
        Ok(()) => Json(serde_json::json!({
            "status": "started",
            "type": "lora",
            "base_model_id": body.base_model_id,
            "profile_name": body.profile_name,
            "rank": body.rank,
            "epochs": body.epochs,
            "dataset_ids": body.dataset_ids,
        }))
        .into_response(),
        Err(active) => Json(active_error(&active)).into_response(),
    }
}

// ── WebSocket handler for training progress ──────────────────────────────────

async fn ws_train_progress_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_train_ws_client(socket, state))
}

async fn handle_train_ws_client(mut socket: WebSocket, state: AppState) {
    let mut rx = {
        let s = state.read().await;
        s.training_progress_tx.subscribe()
    };

    info!("WebSocket client connected (train/progress)");

    // Send current status immediately.
    {
        let snapshot = {
            let s = state.read().await;
            s.training_state.snapshot()
        };
        if let Ok(json) = serde_json::to_string(&snapshot) {
            let msg = serde_json::json!({
                "type": "status",
                "data": serde_json::from_str::<serde_json::Value>(&json).unwrap_or_default(),
            });
            let _ = socket.send(Message::Text(msg.to_string().into())).await;
        }
    }

    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(progress_json) => {
                        let parsed = serde_json::from_str::<serde_json::Value>(&progress_json)
                            .unwrap_or_default();
                        let ws_msg = serde_json::json!({
                            "type": "progress",
                            "data": parsed,
                        });
                        if socket.send(Message::Text(ws_msg.to_string().into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("Train WS client lagged by {n} messages");
                    }
                    Err(_) => break,
                }
            }
            ws_msg = socket.recv() => {
                match ws_msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {} // ignore client messages
                }
            }
        }
    }

    info!("WebSocket client disconnected (train/progress)");
}

// ── Router factory ───────────────────────────────────────────────────────────

/// Build the training API sub-router.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/train/start", post(start_training))
        .route("/api/v1/train/stop", post(stop_training))
        .route("/api/v1/train/status", get(training_status))
        .route("/api/v1/train/pretrain", post(start_pretrain))
        .route("/api/v1/train/lora", post(start_lora_training))
        .route("/ws/train/progress", get(ws_train_progress_handler))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn training_config_defaults() {
        let config = TrainingConfig::default();
        assert_eq!(config.epochs, 100);
        assert_eq!(config.batch_size, 8);
        assert!((config.learning_rate - 0.001).abs() < 1e-9);
        assert_eq!(config.warmup_epochs, 5);
        assert_eq!(config.early_stopping_patience, 20);
    }

    #[test]
    fn training_status_default_is_inactive() {
        let status = TrainingStatus::default();
        assert!(!status.active);
        assert_eq!(status.phase, "idle");
    }

    #[test]
    fn claim_training_slot_admits_exactly_one_concurrent_start() {
        // Regression test for the single-job TOCTOU race. Many threads race to
        // claim one slot at the same instant (a barrier maximises contention);
        // the status mutex must admit EXACTLY ONE. A split check-then-set (the
        // old shape) would let several through under load — verified by
        // temporarily reverting the atomicity, which drops this from 1.
        use std::sync::atomic::{AtomicUsize, Ordering as O};
        use std::sync::{Arc, Barrier};

        let status = Arc::new(Mutex::new(TrainingStatus::default()));
        let config = TrainingConfig::default();
        let winners = Arc::new(AtomicUsize::new(0));

        const N: usize = 32;
        let barrier = Arc::new(Barrier::new(N));
        let mut handles = Vec::with_capacity(N);
        for _ in 0..N {
            let status = status.clone();
            let config = config.clone();
            let winners = winners.clone();
            let barrier = barrier.clone();
            handles.push(std::thread::spawn(move || {
                barrier.wait();
                if claim_training_slot(&status, &config).is_ok() {
                    winners.fetch_add(1, O::SeqCst);
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(
            winners.load(O::SeqCst),
            1,
            "exactly one concurrent start may claim the single training slot"
        );
        assert!(
            status.lock().unwrap().active,
            "the slot must be marked active after a successful claim"
        );
    }

    #[test]
    fn claim_training_slot_rejects_when_already_active() {
        let status = Arc::new(Mutex::new(TrainingStatus::default()));
        let config = TrainingConfig::default();
        assert!(claim_training_slot(&status, &config).is_ok(), "first claim wins");
        let err = claim_training_slot(&status, &config)
            .expect_err("second claim must be refused while active");
        assert!(err.active, "the rejection carries the active status");
    }

    #[test]
    fn training_progress_serializes() {
        let progress = TrainingProgress {
            epoch: 10,
            batch: 25,
            total_batches: 50,
            train_loss: 0.35,
            val_pck: 0.72,
            val_oks: 0.63,
            lr: 0.0008,
            phase: "training".to_string(),
        };
        let json = serde_json::to_string(&progress).unwrap();
        assert!(json.contains("\"epoch\":10"));
        assert!(json.contains("\"phase\":\"training\""));
    }

    #[test]
    fn training_config_deserializes_with_defaults() {
        let json = r#"{"epochs": 50}"#;
        let config: TrainingConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.epochs, 50);
        assert_eq!(config.batch_size, 8); // default
        assert!((config.learning_rate - 0.001).abs() < 1e-9); // default
    }

    #[test]
    fn feature_dim_computation() {
        // 56 subs: 56 amps + 56 variances + 56 gradients + 9 freq + 3 global = 180
        assert_eq!(feature_dim(56), 56 + 56 + 56 + 9 + 3);
        assert_eq!(feature_dim(1), 1 + 1 + 1 + 9 + 3);
    }

    #[test]
    fn goertzel_dc_power() {
        // DC component (freq=0) of a constant signal should be high.
        let signal = vec![1.0; 100];
        let power = goertzel_power(&signal, 0.0);
        assert!(power > 0.5, "DC power should be significant: {power}");
    }

    #[test]
    fn goertzel_zero_on_empty() {
        assert_eq!(goertzel_power(&[], 0.1), 0.0);
    }

    #[test]
    fn extract_features_produces_correct_length() {
        let frame = RecordedFrame {
            timestamp: 1.0,
            subcarriers: vec![1.0; 56],
            rssi: -50.0,
            noise_floor: -90.0,
            features: serde_json::json!({}),
        };
        let features = extract_features_for_frame(&frame, &[], None, 10.0);
        assert_eq!(features.len(), feature_dim(56));
    }

    #[test]
    fn teacher_targets_produce_51_values() {
        let frame = RecordedFrame {
            timestamp: 1.0,
            subcarriers: vec![5.0; 56],
            rssi: -50.0,
            noise_floor: -90.0,
            features: serde_json::json!({}),
        };
        let targets = compute_teacher_targets(&frame, None);
        assert_eq!(targets.len(), N_TARGETS); // 17 * 3 = 51
    }

    #[test]
    fn deterministic_shuffle_is_stable() {
        let a = deterministic_shuffle(10, 42);
        let b = deterministic_shuffle(10, 42);
        assert_eq!(a, b);
        // Different seed should produce different order.
        let c = deterministic_shuffle(10, 99);
        assert_ne!(a, c);
    }

    #[test]
    fn deterministic_shuffle_is_permutation() {
        let perm = deterministic_shuffle(20, 12345);
        let mut sorted = perm.clone();
        sorted.sort();
        let expected: Vec<usize> = (0..20).collect();
        assert_eq!(sorted, expected);
    }

    #[test]
    fn forward_pass_zero_weights() {
        let x = vec![vec![1.0, 2.0, 3.0]];
        let weights = vec![0.0; 3 * 2]; // 2 targets, 3 features
        let bias = vec![0.0; 2];
        let preds = forward(&x, &weights, &bias, 3, 2);
        assert_eq!(preds.len(), 1);
        assert_eq!(preds[0], vec![0.0, 0.0]);
    }

    #[test]
    fn forward_pass_identity() {
        // W = identity-like: target 0 = feature 0, target 1 = feature 1.
        let x = vec![vec![3.0, 7.0]];
        let weights = vec![1.0, 0.0, 0.0, 1.0]; // 2x2 identity
        let bias = vec![0.0, 0.0];
        let preds = forward(&x, &weights, &bias, 2, 2);
        assert_eq!(preds[0], vec![3.0, 7.0]);
    }

    #[test]
    fn forward_pass_with_bias() {
        let x = vec![vec![0.0, 0.0]];
        let weights = vec![0.0; 4];
        let bias = vec![5.0, -3.0];
        let preds = forward(&x, &weights, &bias, 2, 2);
        assert_eq!(preds[0], vec![5.0, -3.0]);
    }

    #[test]
    fn compute_mse_zero_error() {
        let preds = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let targets = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        assert!((compute_mse(&preds, &targets)).abs() < 1e-9);
    }

    #[test]
    fn compute_mse_known_value() {
        let preds = vec![vec![0.0]];
        let targets = vec![vec![1.0]];
        assert!((compute_mse(&preds, &targets) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn pck_perfect_prediction() {
        // Build targets where torso height is large so threshold is generous.
        let mut tgt = vec![0.0; N_TARGETS];
        tgt[1] = 0.0; // nose y
        tgt[34] = 100.0; // left hip y
        tgt[37] = 100.0; // right hip y
        let preds = vec![tgt.clone()];
        let targets = vec![tgt];
        let pck = compute_pck_torso_height(&preds, &targets, 0.2);
        assert!(
            (pck - 1.0).abs() < 1e-9,
            "Perfect prediction should give PCK=1.0"
        );
    }

    /// ADR-155 §2.1 / §8 (RESOLVED): the live-server PCK is torso-HEIGHT
    /// normalized and is **labelled distinctly** from the canonical hip↔hip
    /// PCK. This test pins the *divergence*: the same prediction error gives a
    /// different verdict under torso-HEIGHT (nose→hip, vertical) than under an
    /// independent hip↔hip-WIDTH (horizontal) computation — proving the two are
    /// genuinely different metrics, so relabelling (not unifying) is correct.
    ///
    /// Construction (pixel-space, one keypoint of interest = left_shoulder kp5):
    /// * nose(0).y = 0,  hips(11,12).y = 100  ⇒ torso HEIGHT = 100.
    ///   ⇒ torso-height threshold @0.2 = 20 px.
    /// * hips x: left(11).x = 0, right(12).x = 10 ⇒ torso WIDTH = 10.
    ///   ⇒ a hip↔hip-WIDTH threshold @0.2 = 2 px.
    /// * Predicted kp5 is 5 px off in x from its target.
    ///   - torso-HEIGHT verdict: 5 ≤ 20 ⇒ CORRECT.
    ///   - hip↔hip-WIDTH verdict: 5 > 2  ⇒ WRONG.
    /// The two normalizers must disagree on this exact sample.
    #[test]
    fn torso_pck_is_labelled_distinctly_from_canonical() {
        // Targets: hips define both axes; kp5 is the joint under test.
        let mut tgt = vec![0.0; N_TARGETS];
        tgt[0 * 3] = 0.0; // nose x
        tgt[0 * 3 + 1] = 0.0; // nose y
        tgt[5 * 3] = 0.0; // l_shoulder x (target)
        tgt[5 * 3 + 1] = 50.0; // l_shoulder y
        tgt[11 * 3] = 0.0; // l_hip x
        tgt[11 * 3 + 1] = 100.0; // l_hip y
        tgt[12 * 3] = 10.0; // r_hip x  ⇒ hip↔hip WIDTH = 10
        tgt[12 * 3 + 1] = 100.0; // r_hip y ⇒ torso HEIGHT (nose→hip) = 100

        // Prediction: identical except kp5 x is +5 px off.
        let mut pred = tgt.clone();
        pred[5 * 3] = 5.0; // 5 px error in x on kp5

        // Live-server torso-HEIGHT PCK: error 5 ≤ 0.2×100 = 20 ⇒ kp5 counts
        // correct, so ALL 17 joints correct ⇒ PCK = 1.0.
        let pck_height = compute_pck_torso_height(&[pred.clone()], &[tgt.clone()], 0.2);
        assert!(
            (pck_height - 1.0).abs() < 1e-9,
            "torso-HEIGHT PCK should pass kp5 (5px ≤ 20px), got {pck_height}"
        );

        // Independent hip↔hip-WIDTH verdict on kp5: error 5 > 0.2×10 = 2 ⇒ kp5
        // is WRONG. This is the canonical normalization axis (width, not height).
        let hip_width = (tgt[12 * 3] - tgt[11 * 3]).abs(); // = 10
        let kp5_err = (pred[5 * 3] - tgt[5 * 3]).abs(); // = 5
        let width_threshold = 0.2 * hip_width; // = 2
        assert!(
            kp5_err > width_threshold,
            "hip↔hip-WIDTH should REJECT kp5 (5px > 2px) — the two metrics must disagree"
        );

        // Therefore torso-HEIGHT PCK (1.0) ≠ the hip↔hip-WIDTH verdict on this
        // sample: the live `val_pck` is genuinely a different metric and is
        // correctly labelled `pck_torso_h`, never conflated with canonical PCK.
    }

    #[test]
    fn infer_pose_returns_17_keypoints() {
        let n_sub = 56;
        let n_feat = feature_dim(n_sub);
        let n_params = N_TARGETS * n_feat + N_TARGETS;
        let weights: Vec<f32> = vec![0.001; n_params];
        let stats = FeatureStats {
            mean: vec![0.0; n_feat],
            std: vec![1.0; n_feat],
            n_features: n_feat,
            n_subcarriers: n_sub,
        };
        let subs = vec![5.0f64; n_sub];
        let history: VecDeque<Vec<f64>> = VecDeque::new();
        let kps = infer_pose_from_model(&weights, &stats, &subs, &history, None, 10.0);
        assert_eq!(kps.len(), N_KEYPOINTS);
        // Each keypoint has 4 values.
        for kp in &kps {
            assert_eq!(kp.len(), 4);
            // Confidence should be in (0, 1).
            assert!(kp[3] > 0.0 && kp[3] < 1.0, "confidence={}", kp[3]);
        }
    }

    #[test]
    fn infer_pose_short_weights_returns_defaults() {
        let weights: Vec<f32> = vec![0.0; 10]; // too short
        let stats = FeatureStats {
            mean: vec![0.0; 100],
            std: vec![1.0; 100],
            n_features: 100,
            n_subcarriers: 56,
        };
        let subs = vec![5.0f64; 56];
        let history: VecDeque<Vec<f64>> = VecDeque::new();
        let kps = infer_pose_from_model(&weights, &stats, &subs, &history, None, 10.0);
        assert_eq!(kps.len(), N_KEYPOINTS);
        // Default keypoints have zero confidence.
        for kp in &kps {
            assert!((kp[3]).abs() < 1e-9);
        }
    }

    #[test]
    fn feature_stats_serialization() {
        let stats = FeatureStats {
            mean: vec![1.0, 2.0],
            std: vec![0.5, 1.5],
            n_features: 2,
            n_subcarriers: 1,
        };
        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("\"n_features\":2"));
        let parsed: FeatureStats = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.n_features, 2);
        assert_eq!(parsed.mean, vec![1.0, 2.0]);
    }

    #[test]
    fn recording_parser_extracts_non_empty_sensing_update_nodes() {
        let line = r#"{"type":"sensing_update","timestamp":12.5,"nodes":[{"rssi_dbm":-42.0,"amplitude":[1.0,2.0],"subcarrier_count":2},{"amplitude":[],"subcarrier_count":0}]}"#;
        let frames = parse_recording_line(line);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].timestamp, 12.5);
        assert_eq!(frames[0].subcarriers, vec![1.0, 2.0]);
        assert_eq!(frames[0].rssi, -42.0);
    }

    #[test]
    fn recording_parser_keeps_native_training_frame_compatibility() {
        let line = r#"{"timestamp":3.0,"subcarriers":[4.0,5.0]}"#;
        let frames = parse_recording_line(line);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].subcarriers, vec![4.0, 5.0]);
    }

    /// Build a small deterministic set of synthetic CSI frames with enough
    /// variation that feature extraction is non-degenerate.
    fn synthetic_history(n: usize, n_sub: usize) -> Vec<Vec<f64>> {
        (0..n)
            .map(|i| {
                (0..n_sub)
                    .map(|k| 10.0 + ((i as f64) * 0.3 + (k as f64) * 0.1).sin() * 2.0)
                    .collect()
            })
            .collect()
    }

    /// ADR-186 P3/P6 end-to-end: the real (state-free) training core must
    /// (a) stream real progress events over the broadcast channel and
    /// (b) actually write a `.rvf` model artifact on completion — not merely
    /// flip a status flag. This is the regression guard that keeps the trainer
    /// wired (the module was previously orphaned / uncompiled — ADR-186 §1.3).
    #[tokio::test]
    async fn training_job_streams_real_progress_and_writes_model() {
        let history = synthetic_history(40, 56);

        let (tx, mut rx) = broadcast::channel::<String>(1024);
        let status = Arc::new(Mutex::new(TrainingStatus::default()));
        let cancel = Arc::new(AtomicBool::new(false));

        let config = TrainingConfig {
            epochs: 3,
            batch_size: 8,
            warmup_epochs: 1,
            early_stopping_patience: 10,
            ..Default::default()
        };

        // Empty dataset_ids → falls back to the in-memory history snapshot, so
        // this test does not depend on the recordings directory.
        let rvf = run_training_job(
            status.clone(),
            cancel,
            tx,
            config,
            Vec::new(),
            history,
            "supervised",
        )
        .await;

        // (b) A real model artifact was produced and exists on disk.
        let rvf_path = rvf.expect("training must produce an .rvf model artifact");
        assert!(
            rvf_path.exists(),
            "rvf artifact should exist at {}",
            rvf_path.display()
        );

        // (a) Real progress frames were streamed, at least one carrying an epoch.
        let mut n_frames = 0usize;
        let mut saw_epoch = false;
        let mut saw_completed = false;
        while let Ok(msg) = rx.try_recv() {
            n_frames += 1;
            let v: serde_json::Value = serde_json::from_str(&msg).unwrap();
            if v.get("epoch").and_then(|e| e.as_u64()).unwrap_or(0) >= 1 {
                saw_epoch = true;
            }
            if v.get("phase").and_then(|p| p.as_str()) == Some("completed") {
                saw_completed = true;
            }
        }
        assert!(n_frames > 0, "expected streamed progress frames, got none");
        assert!(saw_epoch, "expected at least one epoch-tagged progress frame");
        assert!(saw_completed, "expected a terminal 'completed' progress frame");

        // Final shared status reflects genuine completion, not just a flag flip:
        // real epochs ran (the loop wrote per-epoch status) and a finite loss was
        // computed from the real gradient-descent pass.
        let final_status = status.lock().unwrap().clone();
        assert!(!final_status.active, "job should be inactive when finished");
        assert_eq!(final_status.phase, "completed");
        assert!(
            final_status.epoch >= 1,
            "at least one real training epoch should have run"
        );
        assert!(
            final_status.train_loss.is_finite(),
            "a finite training loss should have been computed"
        );

        // Keep the test hermetic — remove the artifact it wrote.
        let _ = std::fs::remove_file(&rvf_path);
    }

    /// ADR-186 P4 (path safety): a `dataset_id` containing directory traversal
    /// is rejected before any file is opened, so the loader returns no frames
    /// rather than reading an arbitrary file.
    #[tokio::test]
    async fn load_recording_frames_rejects_path_traversal() {
        let frames = load_recording_frames(&["../../etc/passwd".to_string()]).await;
        assert!(
            frames.is_empty(),
            "path-traversal dataset_id must yield no frames"
        );
    }

    /// Exported model ids must be unique per call — a second-resolution
    /// timestamp alone collided for runs finishing in the same wall-clock second
    /// (silently overwriting each other's `.rvf`, which also flaked the
    /// concurrent model-writing tests on CI). Guards against regressing the
    /// filename scheme back to non-unique.
    #[test]
    fn model_ids_are_unique_per_call() {
        let ids: Vec<String> = (0..1000).map(|_| next_model_id("supervised")).collect();
        let unique: std::collections::HashSet<&String> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len(), "every model id must be distinct");
        assert!(ids[0].starts_with("trained-supervised-"));
    }

    /// ADR-186 P5: the enablement gate is enabled by default and only disabled
    /// by an explicit truthy opt-out, so a `--no-default-features` / default
    /// build always has server training ON (no silent regression to disabled).
    #[test]
    fn training_enablement_gate() {
        assert!(training_enabled_from_env(None), "default is enabled");
        assert!(training_enabled_from_env(Some("0")), "0 keeps it enabled");
        assert!(training_enabled_from_env(Some("")), "empty keeps it enabled");
        assert!(!training_enabled_from_env(Some("1")), "1 disables");
        assert!(!training_enabled_from_env(Some("true")), "true disables");
        assert!(!training_enabled_from_env(Some("YES")), "case-insensitive");
        assert!(!training_enabled_from_env(Some(" 1 ")), "trims whitespace");
    }

    /// A job that is cancelled before it starts still exits cleanly and reports
    /// the `cancelled` terminal phase (drives `stop_training`'s cooperative flag).
    #[tokio::test]
    async fn training_job_honors_cancellation() {
        let history = synthetic_history(40, 56);
        let (tx, _rx) = broadcast::channel::<String>(1024);
        let status = Arc::new(Mutex::new(TrainingStatus::default()));
        let cancel = Arc::new(AtomicBool::new(true)); // pre-cancelled

        let config = TrainingConfig {
            epochs: 50,
            batch_size: 8,
            warmup_epochs: 1,
            early_stopping_patience: 10,
            ..Default::default()
        };

        let rvf = run_training_job(
            status.clone(),
            cancel,
            tx,
            config,
            Vec::new(),
            history,
            "supervised",
        )
        .await;

        // Cancelled before the first epoch → no model, terminal phase cancelled.
        assert!(rvf.is_none(), "cancelled run should not export a model");
        let final_status = status.lock().unwrap().clone();
        assert!(!final_status.active);
        assert_eq!(final_status.phase, "cancelled");
    }
}
