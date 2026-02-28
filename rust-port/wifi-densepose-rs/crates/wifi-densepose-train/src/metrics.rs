//! Evaluation metrics for WiFi-DensePose training.
//!
//! This module provides:
//!
//! - **PCK\@0.2** (Percentage of Correct Keypoints): a keypoint is considered
//!   correct when its Euclidean distance from the ground truth is within 20%
//!   of the person bounding-box diagonal.
//! - **OKS** (Object Keypoint Similarity): the COCO-style metric that uses a
//!   per-joint exponential kernel with sigmas from the COCO annotation
//!   guidelines.
//!
//! Results are accumulated over mini-batches via [`MetricsAccumulator`] and
//! finalized into a [`MetricsResult`] at the end of a validation epoch.
//!
//! # No mock data
//!
//! All computations are grounded in real geometry and follow published metric
//! definitions. No random or synthetic values are introduced at runtime.

use ndarray::{Array1, Array2};

// ---------------------------------------------------------------------------
// COCO keypoint sigmas (17 joints)
// ---------------------------------------------------------------------------

/// Per-joint sigma values from the COCO keypoint evaluation standard.
///
/// These constants control the spread of the OKS Gaussian kernel for each
/// of the 17 COCO-defined body joints.
pub const COCO_KP_SIGMAS: [f32; 17] = [
    0.026, // 0  nose
    0.025, // 1  left_eye
    0.025, // 2  right_eye
    0.035, // 3  left_ear
    0.035, // 4  right_ear
    0.079, // 5  left_shoulder
    0.079, // 6  right_shoulder
    0.072, // 7  left_elbow
    0.072, // 8  right_elbow
    0.062, // 9  left_wrist
    0.062, // 10 right_wrist
    0.107, // 11 left_hip
    0.107, // 12 right_hip
    0.087, // 13 left_knee
    0.087, // 14 right_knee
    0.089, // 15 left_ankle
    0.089, // 16 right_ankle
];

// ---------------------------------------------------------------------------
// MetricsResult
// ---------------------------------------------------------------------------

/// Aggregated evaluation metrics produced by a validation epoch.
///
/// All metrics are averaged over the full dataset passed to the evaluator.
#[derive(Debug, Clone)]
pub struct MetricsResult {
    /// Percentage of Correct Keypoints at threshold 0.2 (0-1 scale).
    ///
    /// A keypoint is "correct" when its predicted position is within
    /// 20% of the ground-truth bounding-box diagonal from the true position.
    pub pck: f32,

    /// Object Keypoint Similarity (0-1 scale, COCO standard).
    ///
    /// OKS is computed per person and averaged across the dataset.
    /// Invisible keypoints (`visibility == 0`) are excluded from both
    /// numerator and denominator.
    pub oks: f32,

    /// Total number of keypoint instances evaluated.
    pub num_keypoints: usize,

    /// Total number of samples evaluated.
    pub num_samples: usize,
}

impl MetricsResult {
    /// Returns `true` when this result is strictly better than `other` on the
    /// primary metric (PCK\@0.2).
    pub fn is_better_than(&self, other: &MetricsResult) -> bool {
        self.pck > other.pck
    }

    /// A human-readable summary line suitable for logging.
    pub fn summary(&self) -> String {
        format!(
            "PCK@0.2={:.4}  OKS={:.4}  (n_samples={}  n_kp={})",
            self.pck, self.oks, self.num_samples, self.num_keypoints
        )
    }
}

impl Default for MetricsResult {
    fn default() -> Self {
        MetricsResult {
            pck: 0.0,
            oks: 0.0,
            num_keypoints: 0,
            num_samples: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// MetricsAccumulator
// ---------------------------------------------------------------------------

/// Running accumulator for keypoint metrics across a validation epoch.
///
/// Call [`MetricsAccumulator::update`] for each mini-batch. After iterating
/// the full dataset call [`MetricsAccumulator::finalize`] to obtain a
/// [`MetricsResult`].
///
/// # Thread safety
///
/// `MetricsAccumulator` is not `Sync`; create one per thread and merge if
/// running multi-threaded evaluation.
pub struct MetricsAccumulator {
    /// Cumulative sum of per-sample PCK scores.
    pck_sum: f64,
    /// Cumulative sum of per-sample OKS scores.
    oks_sum: f64,
    /// Number of individual keypoint instances that were evaluated.
    num_keypoints: usize,
    /// Number of samples seen.
    num_samples: usize,
    /// PCK threshold (fraction of bounding-box diagonal). Default: 0.2.
    pck_threshold: f32,
}

impl MetricsAccumulator {
    /// Create a new accumulator with the given PCK threshold.
    ///
    /// The COCO and many pose papers use `threshold = 0.2` (20% of the
    /// person's bounding-box diagonal).
    pub fn new(pck_threshold: f32) -> Self {
        MetricsAccumulator {
            pck_sum: 0.0,
            oks_sum: 0.0,
            num_keypoints: 0,
            num_samples: 0,
            pck_threshold,
        }
    }

    /// Default accumulator with PCK\@0.2.
    pub fn default_threshold() -> Self {
        Self::new(0.2)
    }

    /// Update the accumulator with one sample's predictions.
    ///
    /// # Arguments
    ///
    /// - `pred_kp`:    `[17, 2]` – predicted keypoint (x, y) in `[0, 1]`.
    /// - `gt_kp`:      `[17, 2]` – ground-truth keypoint (x, y) in `[0, 1]`.
    /// - `visibility`: `[17]`   – 0 = invisible, 1/2 = visible.
    ///
    /// Keypoints with `visibility == 0` are skipped.
    pub fn update(
        &mut self,
        pred_kp: &Array2<f32>,
        gt_kp: &Array2<f32>,
        visibility: &Array1<f32>,
    ) {
        let num_joints = pred_kp.shape()[0].min(gt_kp.shape()[0]).min(visibility.len());

        // Compute bounding-box diagonal from visible ground-truth keypoints.
        let bbox_diag = bounding_box_diagonal(gt_kp, visibility, num_joints);
        // Guard against degenerate (point) bounding boxes.
        let safe_diag = bbox_diag.max(1e-3);

        let mut pck_correct = 0usize;
        let mut visible_count = 0usize;
        let mut oks_num = 0.0f64;
        let mut oks_den = 0.0f64;

        for j in 0..num_joints {
            if visibility[j] < 0.5 {
                // Invisible joint: skip.
                continue;
            }
            visible_count += 1;

            let dx = pred_kp[[j, 0]] - gt_kp[[j, 0]];
            let dy = pred_kp[[j, 1]] - gt_kp[[j, 1]];
            let dist = (dx * dx + dy * dy).sqrt();

            // PCK: correct if within threshold × diagonal.
            if dist <= self.pck_threshold * safe_diag {
                pck_correct += 1;
            }

            // OKS contribution for this joint.
            let sigma = if j < COCO_KP_SIGMAS.len() {
                COCO_KP_SIGMAS[j]
            } else {
                0.07 // fallback sigma for non-standard joints
            };
            // Normalise distance by (2 × sigma)² × (area = diagonal²).
            let two_sigma_sq = 2.0 * (sigma as f64) * (sigma as f64);
            let area = (safe_diag as f64) * (safe_diag as f64);
            let exp_arg = -(dist as f64 * dist as f64) / (two_sigma_sq * area + 1e-10);
            oks_num += exp_arg.exp();
            oks_den += 1.0;
        }

        // Per-sample PCK (fraction of visible joints that were correct).
        let sample_pck = if visible_count > 0 {
            pck_correct as f64 / visible_count as f64
        } else {
            1.0 // No visible joints: trivially correct (no evidence of error).
        };

        // Per-sample OKS.
        let sample_oks = if oks_den > 0.0 {
            oks_num / oks_den
        } else {
            1.0
        };

        self.pck_sum += sample_pck;
        self.oks_sum += sample_oks;
        self.num_keypoints += visible_count;
        self.num_samples += 1;
    }

    /// Finalize and return aggregated metrics.
    ///
    /// Returns `None` if no samples have been accumulated yet.
    pub fn finalize(&self) -> Option<MetricsResult> {
        if self.num_samples == 0 {
            return None;
        }
        let n = self.num_samples as f64;
        Some(MetricsResult {
            pck: (self.pck_sum / n) as f32,
            oks: (self.oks_sum / n) as f32,
            num_keypoints: self.num_keypoints,
            num_samples: self.num_samples,
        })
    }

    /// Return the accumulated sample count.
    pub fn num_samples(&self) -> usize {
        self.num_samples
    }

    /// Reset the accumulator to the initial (empty) state.
    pub fn reset(&mut self) {
        self.pck_sum = 0.0;
        self.oks_sum = 0.0;
        self.num_keypoints = 0;
        self.num_samples = 0;
    }
}

// ---------------------------------------------------------------------------
// Geometric helpers
// ---------------------------------------------------------------------------

/// Compute the Euclidean diagonal of the bounding box of visible keypoints.
///
/// The bounding box is defined by the axis-aligned extent of all keypoints
/// that have `visibility[j] >= 0.5`.  Returns 0.0 if there are no visible
/// keypoints or all are co-located.
fn bounding_box_diagonal(
    kp: &Array2<f32>,
    visibility: &Array1<f32>,
    num_joints: usize,
) -> f32 {
    let mut x_min = f32::MAX;
    let mut x_max = f32::MIN;
    let mut y_min = f32::MAX;
    let mut y_max = f32::MIN;
    let mut any_visible = false;

    for j in 0..num_joints {
        if visibility[j] >= 0.5 {
            let x = kp[[j, 0]];
            let y = kp[[j, 1]];
            x_min = x_min.min(x);
            x_max = x_max.max(x);
            y_min = y_min.min(y);
            y_max = y_max.max(y);
            any_visible = true;
        }
    }

    if !any_visible {
        return 0.0;
    }

    let w = (x_max - x_min).max(0.0);
    let h = (y_max - y_min).max(0.0);
    (w * w + h * h).sqrt()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::{array, Array1, Array2};
    use approx::assert_abs_diff_eq;

    fn perfect_prediction(n_joints: usize) -> (Array2<f32>, Array2<f32>, Array1<f32>) {
        let gt = Array2::from_shape_fn((n_joints, 2), |(j, c)| {
            if c == 0 { j as f32 * 0.05 } else { j as f32 * 0.04 }
        });
        let vis = Array1::from_elem(n_joints, 2.0_f32);
        (gt.clone(), gt, vis)
    }

    #[test]
    fn perfect_pck_is_one() {
        let (pred, gt, vis) = perfect_prediction(17);
        let mut acc = MetricsAccumulator::default_threshold();
        acc.update(&pred, &gt, &vis);
        let result = acc.finalize().unwrap();
        assert_abs_diff_eq!(result.pck, 1.0_f32, epsilon = 1e-5);
    }

    #[test]
    fn perfect_oks_is_one() {
        let (pred, gt, vis) = perfect_prediction(17);
        let mut acc = MetricsAccumulator::default_threshold();
        acc.update(&pred, &gt, &vis);
        let result = acc.finalize().unwrap();
        assert_abs_diff_eq!(result.oks, 1.0_f32, epsilon = 1e-5);
    }

    #[test]
    fn all_invisible_gives_trivial_pck() {
        let mut acc = MetricsAccumulator::default_threshold();
        let pred = Array2::zeros((17, 2));
        let gt = Array2::zeros((17, 2));
        let vis = Array1::zeros(17);
        acc.update(&pred, &gt, &vis);
        let result = acc.finalize().unwrap();
        // No visible joints → trivially "perfect" (no errors to measure)
        assert_abs_diff_eq!(result.pck, 1.0_f32, epsilon = 1e-5);
    }

    #[test]
    fn far_predictions_reduce_pck() {
        let mut acc = MetricsAccumulator::default_threshold();
        // Ground truth: all at (0.5, 0.5)
        let gt = Array2::from_elem((17, 2), 0.5_f32);
        // Predictions: all at (0.0, 0.0) — far from ground truth
        let pred = Array2::zeros((17, 2));
        let vis = Array1::from_elem(17, 2.0_f32);
        acc.update(&pred, &gt, &vis);
        let result = acc.finalize().unwrap();
        // PCK should be well below 1.0
        assert!(result.pck < 0.5, "PCK should be low for wrong predictions, got {}", result.pck);
    }

    #[test]
    fn accumulator_averages_over_samples() {
        let mut acc = MetricsAccumulator::default_threshold();
        for _ in 0..5 {
            let (pred, gt, vis) = perfect_prediction(17);
            acc.update(&pred, &gt, &vis);
        }
        assert_eq!(acc.num_samples(), 5);
        let result = acc.finalize().unwrap();
        assert_abs_diff_eq!(result.pck, 1.0_f32, epsilon = 1e-5);
    }

    #[test]
    fn empty_accumulator_returns_none() {
        let acc = MetricsAccumulator::default_threshold();
        assert!(acc.finalize().is_none());
    }

    #[test]
    fn reset_clears_state() {
        let mut acc = MetricsAccumulator::default_threshold();
        let (pred, gt, vis) = perfect_prediction(17);
        acc.update(&pred, &gt, &vis);
        acc.reset();
        assert_eq!(acc.num_samples(), 0);
        assert!(acc.finalize().is_none());
    }

    #[test]
    fn bbox_diagonal_unit_square() {
        let kp = array![[0.0_f32, 0.0], [1.0, 1.0]];
        let vis = array![2.0_f32, 2.0];
        let diag = bounding_box_diagonal(&kp, &vis, 2);
        assert_abs_diff_eq!(diag, std::f32::consts::SQRT_2, epsilon = 1e-5);
    }

    #[test]
    fn metrics_result_is_better_than() {
        let good = MetricsResult { pck: 0.9, oks: 0.8, num_keypoints: 100, num_samples: 10 };
        let bad  = MetricsResult { pck: 0.5, oks: 0.4, num_keypoints: 100, num_samples: 10 };
        assert!(good.is_better_than(&bad));
        assert!(!bad.is_better_than(&good));
    }
}
