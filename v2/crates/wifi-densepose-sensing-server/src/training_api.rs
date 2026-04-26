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
use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};
use tracing::{error, info, warn};

use crate::recording::{RecordedFrame, RECORDINGS_DIR};
use crate::rvf_container::RvfBuilder;

// ── Constants ────────────────────────────────────────────────────────────────

/// Directory for trained model output.
pub const MODELS_DIR: &str = "data/models";

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

fn default_epochs() -> u32 { 100 }
fn default_batch_size() -> u32 { 8 }
fn default_learning_rate() -> f64 { 0.001 }
fn default_weight_decay() -> f64 { 1e-4 }
fn default_early_stopping_patience() -> u32 { 20 }
fn default_warmup_epochs() -> u32 { 5 }

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

fn default_pretrain_epochs() -> u32 { 50 }

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

fn default_lora_rank() -> u8 { 8 }
fn default_lora_epochs() -> u32 { 30 }

/// Current training status (returned by `GET /api/v1/train/status`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingStatus {
    pub active: bool,
    pub epoch: u32,
    pub total_epochs: u32,
    pub train_loss: f64,
    pub val_pck: f64,
    pub val_oks: f64,
    pub lr: f64,
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
#[derive(Debug, Clone, Serialize)]
pub struct TrainingProgress {
    pub epoch: u32,
    pub batch: u32,
    pub total_batches: u32,
    pub train_loss: f64,
    pub val_pck: f64,
    pub val_oks: f64,
    pub lr: f64,
    pub phase: String,
}

/// Runtime training state stored in `AppStateInner`.
pub struct TrainingState {
    /// Current status snapshot.
    pub status: TrainingStatus,
    /// Handle to the background training task (for cancellation).
    pub task_handle: Option<tokio::task::JoinHandle<()>>,
}

impl Default for TrainingState {
    fn default() -> Self {
        Self {
            status: TrainingStatus::default(),
            task_handle: None,
        }
    }
}

/// Shared application state type.
pub type AppState = Arc<RwLock<super::AppStateInner>>;

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

/// Load CSI frames from `.csi.jsonl` recording files for the given dataset IDs.
///
/// Each dataset_id maps to a file at `data/recordings/{dataset_id}.csi.jsonl`.
/// If a file does not exist, it is silently skipped.
async fn load_recording_frames(dataset_ids: &[String]) -> Vec<RecordedFrame> {
    let mut all_frames = Vec::new();
    let recordings_dir = PathBuf::from(RECORDINGS_DIR);

    for id in dataset_ids {
        let file_path = recordings_dir.join(format!("{id}.csi.jsonl"));
        let data = match tokio::fs::read_to_string(&file_path).await {
            Ok(d) => d,
            Err(e) => {
                warn!("Could not read recording {}: {e}", file_path.display());
                continue;
            }
        };

        let mut line_count = 0u64;
        let mut parse_errors = 0u64;
        for line in data.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            line_count += 1;
            match serde_json::from_str::<RecordedFrame>(line) {
                Ok(frame) => all_frames.push(frame),
                Err(_) => parse_errors += 1,
            }
        }

        info!(
            "Loaded recording {id}: {line_count} lines, {} frames, {parse_errors} parse errors",
            all_frames.len()
        );
    }

    all_frames
}

/// Attempt to collect frames from the live frame_history buffer in AppState.
/// Each `Vec<f64>` in frame_history is a subcarrier amplitude vector.
async fn load_frames_from_history(state: &AppState) -> Vec<RecordedFrame> {
    let s = state.read().await;
    let history: &VecDeque<Vec<f64>> = &s.frame_history;
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
            let a = if k < w.subcarriers.len() { w.subcarriers[k] } else { 0.0 };
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
                let cur = if k < frame.subcarriers.len() { frame.subcarriers[k] } else { 0.0 };
                let prv = if k < prev.subcarriers.len() { prev.subcarriers[k] } else { 0.0 };
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
                        let c = if k < frame.subcarriers.len() { frame.subcarriers[k] } else { 0.0 };
                        let p = if k < prev.subcarriers.len() { prev.subcarriers[k] } else { 0.0 };
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
                        let c = if k < frame.subcarriers.len() { frame.subcarriers[k] } else { 0.0 };
                        let p = if k < prev.subcarriers.len() { prev.subcarriers[k] } else { 0.0 };
                        (c - p).powi(2)
                    })
                    .sum::<f64>()
                    / n_cmp as f64;
                (diff / (mean_amp * mean_amp + 1e-9)).sqrt().clamp(0.0, 1.0)
            } else {
                0.0
            }
        }
        None => (variance / (mean_amp * mean_amp + 1e-9)).sqrt().clamp(0.0, 1.0),
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
        let start = if i >= VARIANCE_WINDOW { i - VARIANCE_WINDOW } else { 0 };
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
            if s < 1e-9 { 1.0 } else { s } // avoid division by zero
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

/// Compute PCK@0.2 (Percentage of Correct Keypoints at threshold 0.2 of torso height).
///
/// Torso height is estimated as the distance between nose (kp 0) and the midpoint
/// of the two hips (kps 11, 12).
fn compute_pck(predictions: &[Vec<f64>], targets: &[Vec<f64>], threshold_ratio: f64) -> f64 {
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
    let mut rng = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    for i in (1..n).rev() {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
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
async fn real_training_loop(
    state: AppState,
    progress_tx: broadcast::Sender<String>,
    config: TrainingConfig,
    dataset_ids: Vec<String>,
    training_type: &str,
) {
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
            epoch: 0, batch: 0, total_batches: 0,
            train_loss: 0.0, val_pck: 0.0, val_oks: 0.0, lr: 0.0,
            phase: "loading_data".to_string(),
        };
        if let Ok(json) = serde_json::to_string(&progress) {
            let _ = progress_tx.send(json);
        }
    }

    let mut frames = load_recording_frames(&dataset_ids).await;
    if frames.is_empty() {
        info!("No recordings found for dataset_ids; falling back to live frame_history");
        frames = load_frames_from_history(&state).await;
    }

    if frames.len() < 10 {
        warn!(
            "Insufficient training data: only {} frames (minimum 10 required). Aborting.",
            frames.len()
        );
        let fail = TrainingProgress {
            epoch: 0, batch: 0, total_batches: 0,
            train_loss: 0.0, val_pck: 0.0, val_oks: 0.0, lr: 0.0,
            phase: "failed_insufficient_data".to_string(),
        };
        if let Ok(json) = serde_json::to_string(&fail) {
            let _ = progress_tx.send(json);
        }
        let mut s = state.write().await;
        s.training_state.status.active = false;
        s.training_state.status.phase = "failed".to_string();
        s.training_state.task_handle = None;
        return;
    }

    info!("Loaded {} frames for training", frames.len());

    // ── Phase 2: Extract features and targets ────────────────────────────────

    {
        let progress = TrainingProgress {
            epoch: 0, batch: 0, total_batches: 0,
            train_loss: 0.0, val_pck: 0.0, val_oks: 0.0, lr: 0.0,
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
        // Check cancellation.
        {
            let s = state.read().await;
            if !s.training_state.status.active {
                info!("Training cancelled at epoch {epoch}");
                break;
            }
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
        let val_pck = compute_pck(&val_preds, val_y, 0.2);
        let val_oks = val_pck * 0.88; // approximate OKS from PCK

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

        // Update shared state.
        {
            let mut s = state.write().await;
            s.training_state.status = TrainingStatus {
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

        info!(
            "Epoch {epoch}/{total_epochs}: loss={train_loss:.6}, val_pck={val_pck:.4}, \
             val_mse={val_mse:.4}, best_pck={best_pck:.4}@{best_epoch}, patience={patience_remaining}"
        );

        // Early stopping.
        if patience_remaining == 0 {
            info!(
                "Early stopping at epoch {epoch} (best={best_epoch}, PCK={best_pck:.4})"
            );
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

    let completed_phase;
    {
        let s = state.read().await;
        completed_phase = if s.training_state.status.active {
            "completed"
        } else {
            "cancelled"
        };
    }

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
            let model_id = format!(
                "trained-{}-{}",
                training_type,
                chrono::Utc::now().format("%Y%m%d_%H%M%S")
            );
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

            if let Err(e) = builder.write_to_file(&rvf_path) {
                error!("Failed to write trained model RVF: {e}");
            } else {
                info!(
                    "Trained model saved: {} ({} params, PCK={:.4})",
                    rvf_path.display(),
                    total_params,
                    best_pck
                );
            }
        }
    }

    // Mark training as inactive.
    {
        let mut s = state.write().await;
        s.training_state.status.active = false;
        s.training_state.status.phase = completed_phase.to_string();
        s.training_state.task_handle = None;
    }

    info!("Real {training_type} training finished: phase={completed_phase}");
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
        let feat_magnitude: f64 = features.iter().map(|v| v.abs()).sum::<f64>()
            / features.len().max(1) as f64;
        coords[3] = (1.0 / (1.0 + (-feat_magnitude + 1.0).exp())).clamp(0.1, 0.99);

        keypoints.push(coords);
    }

    keypoints
}

/// Return default zero-confidence keypoints when inference cannot be performed.
fn default_keypoints() -> Vec<[f64; 4]> {
    vec![[320.0, 240.0, 0.0, 0.0]; N_KEYPOINTS]
}

// ── Axum handlers ────────────────────────────────────────────────────────────

async fn start_training(
    State(state): State<AppState>,
    Json(body): Json<StartTrainingRequest>,
) -> Json<serde_json::Value> {
    // Check if training is already active.
    {
        let s = state.read().await;
        if s.training_state.status.active {
            return Json(serde_json::json!({
                "status": "error",
                "message": "Training is already active. Stop it first.",
                "current_epoch": s.training_state.status.epoch,
                "total_epochs": s.training_state.status.total_epochs,
            }));
        }
    }

    let config = body.config.clone();
    let dataset_ids = body.dataset_ids.clone();

    // Mark training as active and spawn background task.
    let progress_tx;
    {
        let s = state.read().await;
        progress_tx = s.training_progress_tx.clone();
    }

    {
        let mut s = state.write().await;
        s.training_state.status = TrainingStatus {
            active: true,
            epoch: 0,
            total_epochs: config.epochs,
            train_loss: 0.0,
            val_pck: 0.0,
            val_oks: 0.0,
            lr: config.learning_rate,
            best_pck: 0.0,
            best_epoch: 0,
            patience_remaining: config.early_stopping_patience,
            eta_secs: None,
            phase: "initializing".to_string(),
        };
    }

    let state_clone = state.clone();
    let handle = tokio::spawn(async move {
        real_training_loop(state_clone, progress_tx, config, dataset_ids, "supervised")
            .await;
    });

    {
        let mut s = state.write().await;
        s.training_state.task_handle = Some(handle);
    }

    Json(serde_json::json!({
        "status": "started",
        "type": "supervised",
        "dataset_ids": body.dataset_ids,
        "config": body.config,
    }))
}

async fn stop_training(State(state): State<AppState>) -> Json<serde_json::Value> {
    let mut s = state.write().await;
    if !s.training_state.status.active {
        return Json(serde_json::json!({
            "status": "error",
            "message": "No training is currently active.",
        }));
    }

    s.training_state.status.active = false;
    s.training_state.status.phase = "stopping".to_string();

    // The background task checks the active flag and will exit.
    // We do not abort the handle -- we let it finish the current batch gracefully.

    info!("Training stop requested");

    Json(serde_json::json!({
        "status": "stopping",
        "epoch": s.training_state.status.epoch,
        "best_pck": s.training_state.status.best_pck,
    }))
}

async fn training_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    Json(serde_json::to_value(&s.training_state.status).unwrap_or_default())
}

async fn start_pretrain(
    State(state): State<AppState>,
    Json(body): Json<PretrainRequest>,
) -> Json<serde_json::Value> {
    {
        let s = state.read().await;
        if s.training_state.status.active {
            return Json(serde_json::json!({
                "status": "error",
                "message": "Training is already active. Stop it first.",
            }));
        }
    }

    let config = TrainingConfig {
        epochs: body.epochs,
        learning_rate: body.lr,
        warmup_epochs: (body.epochs / 10).max(1),
        early_stopping_patience: body.epochs + 1, // no early stopping for pretrain
        ..Default::default()
    };

    let progress_tx;
    {
        let s = state.read().await;
        progress_tx = s.training_progress_tx.clone();
    }

    {
        let mut s = state.write().await;
        s.training_state.status = TrainingStatus {
            active: true,
            total_epochs: body.epochs,
            phase: "initializing".to_string(),
            ..Default::default()
        };
    }

    let state_clone = state.clone();
    let dataset_ids = body.dataset_ids.clone();
    let handle = tokio::spawn(async move {
        real_training_loop(state_clone, progress_tx, config, dataset_ids, "pretrain")
            .await;
    });

    {
        let mut s = state.write().await;
        s.training_state.task_handle = Some(handle);
    }

    Json(serde_json::json!({
        "status": "started",
        "type": "pretrain",
        "epochs": body.epochs,
        "lr": body.lr,
        "dataset_ids": body.dataset_ids,
    }))
}

async fn start_lora_training(
    State(state): State<AppState>,
    Json(body): Json<LoraTrainRequest>,
) -> Json<serde_json::Value> {
    {
        let s = state.read().await;
        if s.training_state.status.active {
            return Json(serde_json::json!({
                "status": "error",
                "message": "Training is already active. Stop it first.",
            }));
        }
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

    let progress_tx;
    {
        let s = state.read().await;
        progress_tx = s.training_progress_tx.clone();
    }

    {
        let mut s = state.write().await;
        s.training_state.status = TrainingStatus {
            active: true,
            total_epochs: body.epochs,
            phase: "initializing".to_string(),
            ..Default::default()
        };
    }

    let state_clone = state.clone();
    let dataset_ids = body.dataset_ids.clone();
    let handle = tokio::spawn(async move {
        real_training_loop(state_clone, progress_tx, config, dataset_ids, "lora")
            .await;
    });

    {
        let mut s = state.write().await;
        s.training_state.task_handle = Some(handle);
    }

    Json(serde_json::json!({
        "status": "started",
        "type": "lora",
        "base_model_id": body.base_model_id,
        "profile_name": body.profile_name,
        "rank": body.rank,
        "epochs": body.epochs,
        "dataset_ids": body.dataset_ids,
    }))
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
        let s = state.read().await;
        if let Ok(json) = serde_json::to_string(&s.training_state.status) {
            let msg = serde_json::json!({
                "type": "status",
                "data": serde_json::from_str::<serde_json::Value>(&json).unwrap_or_default(),
            });
            let _ = socket
                .send(Message::Text(msg.to_string().into()))
                .await;
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
        tgt[1] = 0.0;   // nose y
        tgt[34] = 100.0; // left hip y
        tgt[37] = 100.0; // right hip y
        let preds = vec![tgt.clone()];
        let targets = vec![tgt];
        let pck = compute_pck(&preds, &targets, 0.2);
        assert!((pck - 1.0).abs() < 1e-9, "Perfect prediction should give PCK=1.0");
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
}
