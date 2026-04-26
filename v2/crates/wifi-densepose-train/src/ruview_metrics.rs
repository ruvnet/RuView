//! RuView three-metric acceptance test (ADR-031).
//!
//! Implements the tiered pass/fail acceptance criteria for multistatic fusion:
//!
//! 1. **Joint Error (PCK / OKS)**: pose estimation accuracy.
//! 2. **Multi-Person Separation (MOTA)**: tracking identity maintenance.
//! 3. **Vital Sign Accuracy**: breathing and heartbeat detection precision.
//!
//! Tiered evaluation:
//!
//! | Tier   | Requirements    | Deployment Gate        |
//! |--------|----------------|------------------------|
//! | Bronze | Metric 2       | Prototype demo         |
//! | Silver | Metrics 1 + 2  | Production candidate   |
//! | Gold   | All three      | Full deployment        |
//!
//! # No mock data
//!
//! All computations use real metric definitions from the COCO evaluation
//! protocol, MOT challenge MOTA definition, and signal-processing SNR
//! measurement. No synthetic values are introduced at runtime.

use ndarray::{Array1, Array2};

// ---------------------------------------------------------------------------
// Tier definitions
// ---------------------------------------------------------------------------

/// Deployment tier achieved by the acceptance test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuViewTier {
    /// No tier met -- system fails acceptance.
    Fail,
    /// Metric 2 (tracking) passes. Prototype demo gate.
    Bronze,
    /// Metrics 1 + 2 (pose + tracking) pass. Production candidate gate.
    Silver,
    /// All three metrics pass. Full deployment gate.
    Gold,
}

impl std::fmt::Display for RuViewTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuViewTier::Fail => write!(f, "FAIL"),
            RuViewTier::Bronze => write!(f, "BRONZE"),
            RuViewTier::Silver => write!(f, "SILVER"),
            RuViewTier::Gold => write!(f, "GOLD"),
        }
    }
}

// ---------------------------------------------------------------------------
// Metric 1: Joint Error (PCK / OKS)
// ---------------------------------------------------------------------------

/// Thresholds for Metric 1 (Joint Error).
#[derive(Debug, Clone)]
pub struct JointErrorThresholds {
    /// PCK@0.2 all 17 keypoints (>= this to pass).
    pub pck_all: f32,
    /// PCK@0.2 torso keypoints (shoulders + hips, >= this to pass).
    pub pck_torso: f32,
    /// Mean OKS (>= this to pass).
    pub oks: f32,
    /// Torso jitter RMS in metres over 10s window (< this to pass).
    pub jitter_rms_m: f32,
    /// Per-keypoint max error 95th percentile in metres (< this to pass).
    pub max_error_p95_m: f32,
}

impl Default for JointErrorThresholds {
    fn default() -> Self {
        JointErrorThresholds {
            pck_all: 0.70,
            pck_torso: 0.80,
            oks: 0.50,
            jitter_rms_m: 0.03,
            max_error_p95_m: 0.15,
        }
    }
}

/// Result of Metric 1 evaluation.
#[derive(Debug, Clone)]
pub struct JointErrorResult {
    /// PCK@0.2 over all 17 keypoints.
    pub pck_all: f32,
    /// PCK@0.2 over torso keypoints (indices 5, 6, 11, 12).
    pub pck_torso: f32,
    /// Mean OKS.
    pub oks: f32,
    /// Torso jitter RMS (metres).
    pub jitter_rms_m: f32,
    /// Per-keypoint max error 95th percentile (metres).
    pub max_error_p95_m: f32,
    /// Whether this metric passes.
    pub passes: bool,
}

/// COCO keypoint sigmas for OKS computation (17 joints).
const COCO_SIGMAS: [f32; 17] = [
    0.026, 0.025, 0.025, 0.035, 0.035, 0.079, 0.079, 0.072, 0.072,
    0.062, 0.062, 0.107, 0.107, 0.087, 0.087, 0.089, 0.089,
];

/// Torso keypoint indices (COCO ordering): left_shoulder, right_shoulder,
/// left_hip, right_hip.
const TORSO_INDICES: [usize; 4] = [5, 6, 11, 12];

/// Evaluate Metric 1: Joint Error.
///
/// # Arguments
///
/// - `pred_kpts`: per-frame predicted keypoints `[17, 2]` in normalised `[0,1]`.
/// - `gt_kpts`: per-frame ground-truth keypoints `[17, 2]`.
/// - `visibility`: per-frame visibility `[17]`, 0 = invisible.
/// - `scale`: per-frame object scale for OKS (pass 1.0 if unknown).
/// - `thresholds`: acceptance thresholds.
///
/// # Returns
///
/// `JointErrorResult` with the computed metrics and pass/fail.
pub fn evaluate_joint_error(
    pred_kpts: &[Array2<f32>],
    gt_kpts: &[Array2<f32>],
    visibility: &[Array1<f32>],
    scale: &[f32],
    thresholds: &JointErrorThresholds,
) -> JointErrorResult {
    let n = pred_kpts.len();
    if n == 0 {
        return JointErrorResult {
            pck_all: 0.0,
            pck_torso: 0.0,
            oks: 0.0,
            jitter_rms_m: f32::MAX,
            max_error_p95_m: f32::MAX,
            passes: false,
        };
    }

    // PCK@0.2 computation.
    let pck_threshold = 0.2;
    let mut all_correct = 0_usize;
    let mut all_total = 0_usize;
    let mut torso_correct = 0_usize;
    let mut torso_total = 0_usize;
    let mut oks_sum = 0.0_f64;
    let mut per_kp_errors: Vec<Vec<f32>> = vec![Vec::new(); 17];

    for i in 0..n {
        let bbox_diag = compute_bbox_diag(&gt_kpts[i], &visibility[i]);
        let safe_diag = bbox_diag.max(1e-3);
        let dist_thr = pck_threshold * safe_diag;

        for j in 0..17 {
            if visibility[i][j] < 0.5 {
                continue;
            }
            let dx = pred_kpts[i][[j, 0]] - gt_kpts[i][[j, 0]];
            let dy = pred_kpts[i][[j, 1]] - gt_kpts[i][[j, 1]];
            let dist = (dx * dx + dy * dy).sqrt();

            per_kp_errors[j].push(dist);

            all_total += 1;
            if dist <= dist_thr {
                all_correct += 1;
            }

            if TORSO_INDICES.contains(&j) {
                torso_total += 1;
                if dist <= dist_thr {
                    torso_correct += 1;
                }
            }
        }

        // OKS for this frame.
        let s = scale.get(i).copied().unwrap_or(1.0);
        let oks_frame = compute_single_oks(&pred_kpts[i], &gt_kpts[i], &visibility[i], s);
        oks_sum += oks_frame as f64;
    }

    let pck_all = if all_total > 0 { all_correct as f32 / all_total as f32 } else { 0.0 };
    let pck_torso = if torso_total > 0 { torso_correct as f32 / torso_total as f32 } else { 0.0 };
    let oks = (oks_sum / n as f64) as f32;

    // Torso jitter: RMS of frame-to-frame torso centroid displacement.
    let jitter_rms_m = compute_torso_jitter(pred_kpts, visibility);

    // 95th percentile max per-keypoint error.
    let max_error_p95_m = compute_p95_max_error(&per_kp_errors);

    let passes = pck_all >= thresholds.pck_all
        && pck_torso >= thresholds.pck_torso
        && oks >= thresholds.oks
        && jitter_rms_m < thresholds.jitter_rms_m
        && max_error_p95_m < thresholds.max_error_p95_m;

    JointErrorResult {
        pck_all,
        pck_torso,
        oks,
        jitter_rms_m,
        max_error_p95_m,
        passes,
    }
}

// ---------------------------------------------------------------------------
// Metric 2: Multi-Person Separation (MOTA)
// ---------------------------------------------------------------------------

/// Thresholds for Metric 2 (Multi-Person Separation).
#[derive(Debug, Clone)]
pub struct TrackingThresholds {
    /// Maximum allowed identity switches (MOTA ID-switch). Must be 0 for pass.
    pub max_id_switches: usize,
    /// Maximum track fragmentation ratio (< this to pass).
    pub max_frag_ratio: f32,
    /// Maximum false track creations per minute (must be 0 for pass).
    pub max_false_tracks_per_min: f32,
}

impl Default for TrackingThresholds {
    fn default() -> Self {
        TrackingThresholds {
            max_id_switches: 0,
            max_frag_ratio: 0.05,
            max_false_tracks_per_min: 0.0,
        }
    }
}

/// A single frame of tracking data for MOTA computation.
#[derive(Debug, Clone)]
pub struct TrackingFrame {
    /// Frame index (0-based).
    pub frame_idx: usize,
    /// Ground-truth person IDs present in this frame.
    pub gt_ids: Vec<u32>,
    /// Predicted person IDs present in this frame.
    pub pred_ids: Vec<u32>,
    /// Assignment: `(pred_id, gt_id)` pairs for matched persons.
    pub assignments: Vec<(u32, u32)>,
}

/// Result of Metric 2 evaluation.
#[derive(Debug, Clone)]
pub struct TrackingResult {
    /// Number of identity switches across the sequence.
    pub id_switches: usize,
    /// Track fragmentation ratio.
    pub fragmentation_ratio: f32,
    /// False track creations per minute.
    pub false_tracks_per_min: f32,
    /// MOTA score (higher is better).
    pub mota: f32,
    /// Total number of frames evaluated.
    pub n_frames: usize,
    /// Whether this metric passes.
    pub passes: bool,
}

/// Evaluate Metric 2: Multi-Person Separation.
///
/// Computes MOTA (Multiple Object Tracking Accuracy) components:
/// identity switches, fragmentation ratio, and false track rate.
///
/// # Arguments
///
/// - `frames`: per-frame tracking data with GT and predicted IDs + assignments.
/// - `duration_minutes`: total duration of the tracking sequence in minutes.
/// - `thresholds`: acceptance thresholds.
pub fn evaluate_tracking(
    frames: &[TrackingFrame],
    duration_minutes: f32,
    thresholds: &TrackingThresholds,
) -> TrackingResult {
    let n_frames = frames.len();
    if n_frames == 0 {
        return TrackingResult {
            id_switches: 0,
            fragmentation_ratio: 0.0,
            false_tracks_per_min: 0.0,
            mota: 0.0,
            n_frames: 0,
            passes: false,
        };
    }

    // Count identity switches: a switch occurs when the predicted ID assigned
    // to a GT ID changes between consecutive frames.
    let mut id_switches = 0_usize;
    let mut prev_assignment: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    let mut total_gt = 0_usize;
    let mut total_misses = 0_usize;
    let mut total_false_positives = 0_usize;

    // Track fragmentation: count how many times a GT track is "broken"
    // (present in one frame, absent in the next, then present again).
    let mut gt_track_presence: std::collections::HashMap<u32, Vec<bool>> =
        std::collections::HashMap::new();

    for frame in frames {
        total_gt += frame.gt_ids.len();
        let n_matched = frame.assignments.len();
        total_misses += frame.gt_ids.len().saturating_sub(n_matched);
        total_false_positives += frame.pred_ids.len().saturating_sub(n_matched);

        let mut current_assignment: std::collections::HashMap<u32, u32> =
            std::collections::HashMap::new();
        for &(pred_id, gt_id) in &frame.assignments {
            current_assignment.insert(gt_id, pred_id);
            if let Some(&prev_pred) = prev_assignment.get(&gt_id) {
                if prev_pred != pred_id {
                    id_switches += 1;
                }
            }
        }

        // Track presence for fragmentation.
        for &gt_id in &frame.gt_ids {
            gt_track_presence
                .entry(gt_id)
                .or_default()
                .push(frame.assignments.iter().any(|&(_, gid)| gid == gt_id));
        }

        prev_assignment = current_assignment;
    }

    // Fragmentation ratio: fraction of GT tracks that have gaps.
    let mut n_fragmented = 0_usize;
    let mut n_tracks = 0_usize;
    for presence in gt_track_presence.values() {
        if presence.len() < 2 {
            continue;
        }
        n_tracks += 1;
        let mut has_gap = false;
        let mut was_present = false;
        let mut lost = false;
        for &present in presence {
            if was_present && !present {
                lost = true;
            }
            if lost && present {
                has_gap = true;
                break;
            }
            was_present = present;
        }
        if has_gap {
            n_fragmented += 1;
        }
    }

    let fragmentation_ratio = if n_tracks > 0 {
        n_fragmented as f32 / n_tracks as f32
    } else {
        0.0
    };

    // False tracks per minute.
    let safe_duration = duration_minutes.max(1e-6);
    let false_tracks_per_min = total_false_positives as f32 / safe_duration;

    // MOTA = 1 - (misses + false_positives + id_switches) / total_gt
    let mota = if total_gt > 0 {
        1.0 - (total_misses + total_false_positives + id_switches) as f32 / total_gt as f32
    } else {
        0.0
    };

    let passes = id_switches <= thresholds.max_id_switches
        && fragmentation_ratio < thresholds.max_frag_ratio
        && false_tracks_per_min <= thresholds.max_false_tracks_per_min;

    TrackingResult {
        id_switches,
        fragmentation_ratio,
        false_tracks_per_min,
        mota,
        n_frames,
        passes,
    }
}

// ---------------------------------------------------------------------------
// Metric 3: Vital Sign Accuracy
// ---------------------------------------------------------------------------

/// Thresholds for Metric 3 (Vital Sign Accuracy).
#[derive(Debug, Clone)]
pub struct VitalSignThresholds {
    /// Breathing rate accuracy tolerance (BPM).
    pub breathing_bpm_tolerance: f32,
    /// Breathing band SNR minimum (dB).
    pub breathing_snr_db: f32,
    /// Heartbeat rate accuracy tolerance (BPM, aspirational).
    pub heartbeat_bpm_tolerance: f32,
    /// Heartbeat band SNR minimum (dB, aspirational).
    pub heartbeat_snr_db: f32,
    /// Micro-motion resolution in metres.
    pub micro_motion_m: f32,
    /// Range for micro-motion test (metres).
    pub micro_motion_range_m: f32,
}

impl Default for VitalSignThresholds {
    fn default() -> Self {
        VitalSignThresholds {
            breathing_bpm_tolerance: 2.0,
            breathing_snr_db: 6.0,
            heartbeat_bpm_tolerance: 5.0,
            heartbeat_snr_db: 3.0,
            micro_motion_m: 0.001,
            micro_motion_range_m: 3.0,
        }
    }
}

/// A single vital sign measurement for evaluation.
#[derive(Debug, Clone)]
pub struct VitalSignMeasurement {
    /// Estimated breathing rate (BPM).
    pub breathing_bpm: f32,
    /// Ground-truth breathing rate (BPM).
    pub gt_breathing_bpm: f32,
    /// Breathing band SNR (dB).
    pub breathing_snr_db: f32,
    /// Estimated heartbeat rate (BPM), if available.
    pub heartbeat_bpm: Option<f32>,
    /// Ground-truth heartbeat rate (BPM), if available.
    pub gt_heartbeat_bpm: Option<f32>,
    /// Heartbeat band SNR (dB), if available.
    pub heartbeat_snr_db: Option<f32>,
}

/// Result of Metric 3 evaluation.
#[derive(Debug, Clone)]
pub struct VitalSignResult {
    /// Mean breathing rate error (BPM).
    pub breathing_error_bpm: f32,
    /// Mean breathing SNR (dB).
    pub breathing_snr_db: f32,
    /// Mean heartbeat rate error (BPM), if measured.
    pub heartbeat_error_bpm: Option<f32>,
    /// Mean heartbeat SNR (dB), if measured.
    pub heartbeat_snr_db: Option<f32>,
    /// Number of measurements evaluated.
    pub n_measurements: usize,
    /// Whether this metric passes.
    pub passes: bool,
}

/// Evaluate Metric 3: Vital Sign Accuracy.
///
/// # Arguments
///
/// - `measurements`: per-epoch vital sign measurements with GT.
/// - `thresholds`: acceptance thresholds.
pub fn evaluate_vital_signs(
    measurements: &[VitalSignMeasurement],
    thresholds: &VitalSignThresholds,
) -> VitalSignResult {
    let n = measurements.len();
    if n == 0 {
        return VitalSignResult {
            breathing_error_bpm: f32::MAX,
            breathing_snr_db: 0.0,
            heartbeat_error_bpm: None,
            heartbeat_snr_db: None,
            n_measurements: 0,
            passes: false,
        };
    }

    // Breathing metrics.
    let breathing_errors: Vec<f32> = measurements
        .iter()
        .map(|m| (m.breathing_bpm - m.gt_breathing_bpm).abs())
        .collect();
    let breathing_error_mean = breathing_errors.iter().sum::<f32>() / n as f32;
    let breathing_snr_mean =
        measurements.iter().map(|m| m.breathing_snr_db).sum::<f32>() / n as f32;

    // Heartbeat metrics (optional).
    let heartbeat_pairs: Vec<(f32, f32, f32)> = measurements
        .iter()
        .filter_map(|m| {
            match (m.heartbeat_bpm, m.gt_heartbeat_bpm, m.heartbeat_snr_db) {
                (Some(hb), Some(gt), Some(snr)) => Some((hb, gt, snr)),
                _ => None,
            }
        })
        .collect();

    let (heartbeat_error, heartbeat_snr) = if heartbeat_pairs.is_empty() {
        (None, None)
    } else {
        let hb_n = heartbeat_pairs.len() as f32;
        let err = heartbeat_pairs
            .iter()
            .map(|(hb, gt, _)| (hb - gt).abs())
            .sum::<f32>()
            / hb_n;
        let snr = heartbeat_pairs.iter().map(|(_, _, s)| s).sum::<f32>() / hb_n;
        (Some(err), Some(snr))
    };

    // Pass/fail: breathing must pass; heartbeat is aspirational.
    let breathing_passes = breathing_error_mean <= thresholds.breathing_bpm_tolerance
        && breathing_snr_mean >= thresholds.breathing_snr_db;

    let heartbeat_passes = match (heartbeat_error, heartbeat_snr) {
        (Some(err), Some(snr)) => {
            err <= thresholds.heartbeat_bpm_tolerance && snr >= thresholds.heartbeat_snr_db
        }
        _ => true, // No heartbeat data: aspirational, not required.
    };

    let passes = breathing_passes && heartbeat_passes;

    VitalSignResult {
        breathing_error_bpm: breathing_error_mean,
        breathing_snr_db: breathing_snr_mean,
        heartbeat_error_bpm: heartbeat_error,
        heartbeat_snr_db: heartbeat_snr,
        n_measurements: n,
        passes,
    }
}

// ---------------------------------------------------------------------------
// Tiered acceptance
// ---------------------------------------------------------------------------

/// Combined result of all three metrics with tier determination.
#[derive(Debug, Clone)]
pub struct RuViewAcceptanceResult {
    /// Metric 1: Joint Error.
    pub joint_error: JointErrorResult,
    /// Metric 2: Tracking.
    pub tracking: TrackingResult,
    /// Metric 3: Vital Signs.
    pub vital_signs: VitalSignResult,
    /// Achieved deployment tier.
    pub tier: RuViewTier,
}

impl RuViewAcceptanceResult {
    /// A human-readable summary of the acceptance test.
    pub fn summary(&self) -> String {
        format!(
            "RuView Tier={} | PCK={:.3} OKS={:.3} | MOTA={:.3} IDsw={} | Breathing={:.1}BPM err",
            self.tier,
            self.joint_error.pck_all,
            self.joint_error.oks,
            self.tracking.mota,
            self.tracking.id_switches,
            self.vital_signs.breathing_error_bpm,
        )
    }
}

/// Determine the deployment tier from individual metric results.
pub fn determine_tier(
    joint_error: &JointErrorResult,
    tracking: &TrackingResult,
    vital_signs: &VitalSignResult,
) -> RuViewTier {
    if !tracking.passes {
        return RuViewTier::Fail;
    }
    // Bronze: only tracking passes.
    if !joint_error.passes {
        return RuViewTier::Bronze;
    }
    // Silver: tracking + joint error pass.
    if !vital_signs.passes {
        return RuViewTier::Silver;
    }
    // Gold: all pass.
    RuViewTier::Gold
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn compute_bbox_diag(kp: &Array2<f32>, vis: &Array1<f32>) -> f32 {
    let mut x_min = f32::MAX;
    let mut x_max = f32::MIN;
    let mut y_min = f32::MAX;
    let mut y_max = f32::MIN;
    let mut any = false;

    for j in 0..17.min(kp.shape()[0]) {
        if vis[j] >= 0.5 {
            let x = kp[[j, 0]];
            let y = kp[[j, 1]];
            x_min = x_min.min(x);
            x_max = x_max.max(x);
            y_min = y_min.min(y);
            y_max = y_max.max(y);
            any = true;
        }
    }
    if !any {
        return 0.0;
    }
    let w = (x_max - x_min).max(0.0);
    let h = (y_max - y_min).max(0.0);
    (w * w + h * h).sqrt()
}

fn compute_single_oks(pred: &Array2<f32>, gt: &Array2<f32>, vis: &Array1<f32>, s: f32) -> f32 {
    let s_sq = s * s;
    let mut num = 0.0_f32;
    let mut den = 0.0_f32;
    for j in 0..17 {
        if vis[j] < 0.5 {
            continue;
        }
        den += 1.0;
        let dx = pred[[j, 0]] - gt[[j, 0]];
        let dy = pred[[j, 1]] - gt[[j, 1]];
        let d_sq = dx * dx + dy * dy;
        let k = COCO_SIGMAS[j];
        num += (-d_sq / (2.0 * s_sq * k * k)).exp();
    }
    if den > 0.0 { num / den } else { 0.0 }
}

fn compute_torso_jitter(pred_kpts: &[Array2<f32>], visibility: &[Array1<f32>]) -> f32 {
    if pred_kpts.len() < 2 {
        return 0.0;
    }

    // Compute torso centroid per frame.
    let centroids: Vec<Option<(f32, f32)>> = pred_kpts
        .iter()
        .zip(visibility.iter())
        .map(|(kp, vis)| {
            let mut cx = 0.0_f32;
            let mut cy = 0.0_f32;
            let mut count = 0_usize;
            for &idx in &TORSO_INDICES {
                if vis[idx] >= 0.5 {
                    cx += kp[[idx, 0]];
                    cy += kp[[idx, 1]];
                    count += 1;
                }
            }
            if count > 0 {
                Some((cx / count as f32, cy / count as f32))
            } else {
                None
            }
        })
        .collect();

    // Frame-to-frame displacement squared.
    let mut sum_sq = 0.0_f64;
    let mut n_pairs = 0_usize;
    for i in 1..centroids.len() {
        if let (Some((x0, y0)), Some((x1, y1))) = (centroids[i - 1], centroids[i]) {
            let dx = (x1 - x0) as f64;
            let dy = (y1 - y0) as f64;
            sum_sq += dx * dx + dy * dy;
            n_pairs += 1;
        }
    }

    if n_pairs == 0 {
        return 0.0;
    }
    (sum_sq / n_pairs as f64).sqrt() as f32
}

fn compute_p95_max_error(per_kp_errors: &[Vec<f32>]) -> f32 {
    // Collect all per-keypoint errors, find 95th percentile.
    let mut all_errors: Vec<f32> = per_kp_errors.iter().flat_map(|e| e.iter().copied()).collect();
    if all_errors.is_empty() {
        return 0.0;
    }
    all_errors.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = ((all_errors.len() as f64 * 0.95) as usize).min(all_errors.len() - 1);
    all_errors[idx]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::{Array1, Array2};

    fn make_perfect_kpts() -> (Array2<f32>, Array2<f32>, Array1<f32>) {
        let kp = Array2::from_shape_fn((17, 2), |(j, d)| {
            if d == 0 { j as f32 * 0.05 } else { j as f32 * 0.03 }
        });
        let vis = Array1::ones(17);
        (kp.clone(), kp, vis)
    }

    fn make_noisy_kpts(noise: f32) -> (Array2<f32>, Array2<f32>, Array1<f32>) {
        let gt = Array2::from_shape_fn((17, 2), |(j, d)| {
            if d == 0 { j as f32 * 0.03 } else { j as f32 * 0.02 }
        });
        let pred = Array2::from_shape_fn((17, 2), |(j, d)| {
            // Apply deterministic noise that varies per joint so some joints
            // are definitely outside the PCK threshold.
            gt[[j, d]] + noise * ((j * 7 + d * 3) as f32).sin()
        });
        let vis = Array1::ones(17);
        (pred, gt, vis)
    }

    #[test]
    fn joint_error_perfect_predictions_pass() {
        let (pred, gt, vis) = make_perfect_kpts();
        let result = evaluate_joint_error(
            &[pred],
            &[gt],
            &[vis],
            &[1.0],
            &JointErrorThresholds::default(),
        );
        assert_eq!(result.pck_all, 1.0, "perfect predictions should have PCK=1.0");
        assert!((result.oks - 1.0).abs() < 1e-3, "perfect predictions should have OKS~1.0");
    }

    #[test]
    fn joint_error_empty_returns_fail() {
        let result = evaluate_joint_error(
            &[],
            &[],
            &[],
            &[],
            &JointErrorThresholds::default(),
        );
        assert!(!result.passes);
    }

    #[test]
    fn joint_error_noisy_predictions_lower_pck() {
        let (pred, gt, vis) = make_noisy_kpts(0.5);
        let result = evaluate_joint_error(
            &[pred],
            &[gt],
            &[vis],
            &[1.0],
            &JointErrorThresholds::default(),
        );
        assert!(result.pck_all < 1.0, "noisy predictions should have PCK < 1.0");
    }

    #[test]
    fn tracking_no_id_switches_pass() {
        let frames: Vec<TrackingFrame> = (0..100)
            .map(|i| TrackingFrame {
                frame_idx: i,
                gt_ids: vec![1, 2],
                pred_ids: vec![1, 2],
                assignments: vec![(1, 1), (2, 2)],
            })
            .collect();
        let result = evaluate_tracking(&frames, 1.0, &TrackingThresholds::default());
        assert_eq!(result.id_switches, 0);
        assert!(result.passes);
    }

    #[test]
    fn tracking_id_switches_detected() {
        let mut frames: Vec<TrackingFrame> = (0..10)
            .map(|i| TrackingFrame {
                frame_idx: i,
                gt_ids: vec![1, 2],
                pred_ids: vec![1, 2],
                assignments: vec![(1, 1), (2, 2)],
            })
            .collect();
        // Swap assignments at frame 5.
        frames[5].assignments = vec![(2, 1), (1, 2)];
        let result = evaluate_tracking(&frames, 1.0, &TrackingThresholds::default());
        assert!(result.id_switches >= 1, "should detect ID switch at frame 5");
        assert!(!result.passes, "ID switches should cause failure");
    }

    #[test]
    fn tracking_empty_returns_fail() {
        let result = evaluate_tracking(&[], 1.0, &TrackingThresholds::default());
        assert!(!result.passes);
    }

    #[test]
    fn vital_signs_accurate_breathing_passes() {
        let measurements = vec![
            VitalSignMeasurement {
                breathing_bpm: 15.0,
                gt_breathing_bpm: 14.5,
                breathing_snr_db: 10.0,
                heartbeat_bpm: None,
                gt_heartbeat_bpm: None,
                heartbeat_snr_db: None,
            },
            VitalSignMeasurement {
                breathing_bpm: 16.0,
                gt_breathing_bpm: 15.5,
                breathing_snr_db: 8.0,
                heartbeat_bpm: None,
                gt_heartbeat_bpm: None,
                heartbeat_snr_db: None,
            },
        ];
        let result = evaluate_vital_signs(&measurements, &VitalSignThresholds::default());
        assert!(result.breathing_error_bpm <= 2.0);
        assert!(result.passes);
    }

    #[test]
    fn vital_signs_inaccurate_breathing_fails() {
        let measurements = vec![VitalSignMeasurement {
            breathing_bpm: 25.0,
            gt_breathing_bpm: 15.0,
            breathing_snr_db: 10.0,
            heartbeat_bpm: None,
            gt_heartbeat_bpm: None,
            heartbeat_snr_db: None,
        }];
        let result = evaluate_vital_signs(&measurements, &VitalSignThresholds::default());
        assert!(!result.passes, "10 BPM error should fail");
    }

    #[test]
    fn vital_signs_empty_returns_fail() {
        let result = evaluate_vital_signs(&[], &VitalSignThresholds::default());
        assert!(!result.passes);
    }

    #[test]
    fn tier_determination_gold() {
        let je = JointErrorResult {
            pck_all: 0.85,
            pck_torso: 0.90,
            oks: 0.65,
            jitter_rms_m: 0.01,
            max_error_p95_m: 0.10,
            passes: true,
        };
        let tr = TrackingResult {
            id_switches: 0,
            fragmentation_ratio: 0.01,
            false_tracks_per_min: 0.0,
            mota: 0.95,
            n_frames: 1000,
            passes: true,
        };
        let vs = VitalSignResult {
            breathing_error_bpm: 1.0,
            breathing_snr_db: 8.0,
            heartbeat_error_bpm: Some(3.0),
            heartbeat_snr_db: Some(4.0),
            n_measurements: 10,
            passes: true,
        };
        assert_eq!(determine_tier(&je, &tr, &vs), RuViewTier::Gold);
    }

    #[test]
    fn tier_determination_silver() {
        let je = JointErrorResult { passes: true, ..Default::default() };
        let tr = TrackingResult { passes: true, ..Default::default() };
        let vs = VitalSignResult { passes: false, ..Default::default() };
        assert_eq!(determine_tier(&je, &tr, &vs), RuViewTier::Silver);
    }

    #[test]
    fn tier_determination_bronze() {
        let je = JointErrorResult { passes: false, ..Default::default() };
        let tr = TrackingResult { passes: true, ..Default::default() };
        let vs = VitalSignResult { passes: false, ..Default::default() };
        assert_eq!(determine_tier(&je, &tr, &vs), RuViewTier::Bronze);
    }

    #[test]
    fn tier_determination_fail() {
        let je = JointErrorResult { passes: true, ..Default::default() };
        let tr = TrackingResult { passes: false, ..Default::default() };
        let vs = VitalSignResult { passes: true, ..Default::default() };
        assert_eq!(determine_tier(&je, &tr, &vs), RuViewTier::Fail);
    }

    #[test]
    fn tier_ordering() {
        assert!(RuViewTier::Gold > RuViewTier::Silver);
        assert!(RuViewTier::Silver > RuViewTier::Bronze);
        assert!(RuViewTier::Bronze > RuViewTier::Fail);
    }

    // Implement Default for test convenience.
    impl Default for JointErrorResult {
        fn default() -> Self {
            JointErrorResult {
                pck_all: 0.0,
                pck_torso: 0.0,
                oks: 0.0,
                jitter_rms_m: 0.0,
                max_error_p95_m: 0.0,
                passes: false,
            }
        }
    }

    impl Default for TrackingResult {
        fn default() -> Self {
            TrackingResult {
                id_switches: 0,
                fragmentation_ratio: 0.0,
                false_tracks_per_min: 0.0,
                mota: 0.0,
                n_frames: 0,
                passes: false,
            }
        }
    }

    impl Default for VitalSignResult {
        fn default() -> Self {
            VitalSignResult {
                breathing_error_bpm: 0.0,
                breathing_snr_db: 0.0,
                heartbeat_error_bpm: None,
                heartbeat_snr_db: None,
                n_measurements: 0,
                passes: false,
            }
        }
    }
}
