//! Per-node configuration constants and tuning parameters.
//!
//! Complies with ELMAS.md: explicit documentation, <= 350 lines.

use std::time::Duration;

/// Circular buffer capacity of CSI frame amplitudes for analysis.
pub const FRAME_HISTORY_CAPACITY: usize = 100;

/// Number of frames used to vote and lock onto the dominant CSI subcarrier grid.
pub const GRID_VOTE_FRAMES: usize = 64;

/// Inactivity timeout after which a node is marked offline/stale.
pub const ESP32_OFFLINE_TIMEOUT: Duration = Duration::from_secs(5);

/// Default EMA alpha for temporal keypoint smoothing (RuVector Phase 2).
/// Lower = smoother (more history, less jitter). 0.15 balances responsiveness
/// with stability for WiFi CSI where per-frame noise is high.
pub const TEMPORAL_EMA_ALPHA_DEFAULT: f64 = 0.15;

/// Reduced EMA alpha when coherence is low (trust measurements less).
pub const TEMPORAL_EMA_ALPHA_LOW_COHERENCE: f64 = 0.05;

/// Coherence threshold below which we reduce EMA alpha.
pub const COHERENCE_LOW_THRESHOLD: f64 = 0.3;

/// Maximum allowed bone-length change ratio between frames (20%).
pub const MAX_BONE_CHANGE_RATIO: f64 = 0.20;

/// Number of motion_energy frames to track for coherence scoring.
pub const COHERENCE_WINDOW: usize = 20;

/// ADR-084 Pass 3 — per-node novelty sketch dimension (56 subcarriers,
/// the dominant ESP32-S3 capture configuration).
pub const NOVELTY_VECTOR_DIM: usize = 56;

/// ADR-084 Pass 3 — number of past sketches retained per-node for
/// novelty comparison. 64 frames ≈ 6.4 s at 10 Hz.
pub const NOVELTY_HISTORY_CAPACITY: usize = 64;

/// ADR-084 Pass 3 — feature-vector schema version. Bump on changes to
/// subcarrier ordering / normalisation so banks reject stale data.
pub const NOVELTY_SKETCH_VERSION: u16 = 1;

/// Recent header-only evidence window used to choose a stable field-model
/// stream. The rate floor is 20% above the 1,000 / 600 s completion minimum.
pub const CALIBRATION_GRID_EVIDENCE_WINDOW: Duration = Duration::from_secs(20);
pub const CALIBRATION_GRID_EVIDENCE_CAPACITY: usize = 1_024;
pub const CALIBRATION_GRID_MIN_SPAN_S: f64 = 10.0;
pub const CALIBRATION_GRID_MIN_RATE_HZ: f64 = 2.0;
pub const CALIBRATION_GRID_MAX_GAP_S: f64 = 5.0;

