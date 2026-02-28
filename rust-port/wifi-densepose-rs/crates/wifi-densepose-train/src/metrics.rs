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
// Per-sample PCK and OKS free functions (required by the training evaluator)
// ---------------------------------------------------------------------------

// Keypoint indices for torso-diameter PCK normalisation (COCO ordering).
const IDX_LEFT_HIP: usize = 11;
const IDX_RIGHT_SHOULDER: usize = 6;

/// Compute the torso diameter for PCK normalisation.
///
/// Torso diameter = ||left_hip − right_shoulder||₂ in normalised [0,1] space.
/// Returns 0.0 when either landmark is invisible, indicating the caller
/// should fall back to a unit normaliser.
fn torso_diameter_pck(gt_kpts: &Array2<f32>, visibility: &Array1<f32>) -> f32 {
    if visibility[IDX_LEFT_HIP] < 0.5 || visibility[IDX_RIGHT_SHOULDER] < 0.5 {
        return 0.0;
    }
    let dx = gt_kpts[[IDX_LEFT_HIP, 0]] - gt_kpts[[IDX_RIGHT_SHOULDER, 0]];
    let dy = gt_kpts[[IDX_LEFT_HIP, 1]] - gt_kpts[[IDX_RIGHT_SHOULDER, 1]];
    (dx * dx + dy * dy).sqrt()
}

/// Compute PCK (Percentage of Correct Keypoints) for a single frame.
///
/// A keypoint `j` is "correct" when its Euclidean distance to the ground
/// truth is within `threshold × torso_diameter` (left_hip ↔ right_shoulder).
/// When the torso reference joints are not visible the threshold is applied
/// directly in normalised [0,1] coordinate space (unit normaliser).
///
/// Only keypoints with `visibility[j] > 0` contribute to the count.
///
/// # Returns
/// `(correct_count, total_count, pck_value)` where `pck_value ∈ [0,1]`;
/// returns `(0, 0, 0.0)` when no keypoint is visible.
pub fn compute_pck(
    pred_kpts: &Array2<f32>,
    gt_kpts: &Array2<f32>,
    visibility: &Array1<f32>,
    threshold: f32,
) -> (usize, usize, f32) {
    let torso = torso_diameter_pck(gt_kpts, visibility);
    let norm = if torso > 1e-6 { torso } else { 1.0_f32 };
    let dist_threshold = threshold * norm;

    let mut correct = 0_usize;
    let mut total = 0_usize;

    for j in 0..17 {
        if visibility[j] < 0.5 {
            continue;
        }
        total += 1;
        let dx = pred_kpts[[j, 0]] - gt_kpts[[j, 0]];
        let dy = pred_kpts[[j, 1]] - gt_kpts[[j, 1]];
        let dist = (dx * dx + dy * dy).sqrt();
        if dist <= dist_threshold {
            correct += 1;
        }
    }

    let pck = if total > 0 {
        correct as f32 / total as f32
    } else {
        0.0
    };
    (correct, total, pck)
}

/// Compute per-joint PCK over a batch of frames.
///
/// Returns `[f32; 17]` where entry `j` is the fraction of frames in which
/// joint `j` was both visible and correctly predicted at the given threshold.
pub fn compute_per_joint_pck(
    pred_batch: &[Array2<f32>],
    gt_batch: &[Array2<f32>],
    vis_batch: &[Array1<f32>],
    threshold: f32,
) -> [f32; 17] {
    assert_eq!(pred_batch.len(), gt_batch.len());
    assert_eq!(pred_batch.len(), vis_batch.len());

    let mut correct = [0_usize; 17];
    let mut total = [0_usize; 17];

    for (pred, (gt, vis)) in pred_batch
        .iter()
        .zip(gt_batch.iter().zip(vis_batch.iter()))
    {
        let torso = torso_diameter_pck(gt, vis);
        let norm = if torso > 1e-6 { torso } else { 1.0_f32 };
        let dist_thr = threshold * norm;

        for j in 0..17 {
            if vis[j] < 0.5 {
                continue;
            }
            total[j] += 1;
            let dx = pred[[j, 0]] - gt[[j, 0]];
            let dy = pred[[j, 1]] - gt[[j, 1]];
            let dist = (dx * dx + dy * dy).sqrt();
            if dist <= dist_thr {
                correct[j] += 1;
            }
        }
    }

    let mut result = [0.0_f32; 17];
    for j in 0..17 {
        result[j] = if total[j] > 0 {
            correct[j] as f32 / total[j] as f32
        } else {
            0.0
        };
    }
    result
}

/// Compute Object Keypoint Similarity (OKS) for a single person.
///
/// COCO OKS formula:
///
/// ```text
/// OKS = Σᵢ exp(-dᵢ² / (2·s²·kᵢ²)) · δ(vᵢ>0)  /  Σᵢ δ(vᵢ>0)
/// ```
///
/// - `dᵢ` – Euclidean distance between predicted and GT keypoint `i`
/// - `s` – object scale (`object_scale`; pass `1.0` when bbox is unknown)
/// - `kᵢ` – per-joint sigma from [`COCO_KP_SIGMAS`]
///
/// Returns `0.0` when no keypoints are visible.
pub fn compute_oks(
    pred_kpts: &Array2<f32>,
    gt_kpts: &Array2<f32>,
    visibility: &Array1<f32>,
    object_scale: f32,
) -> f32 {
    let s_sq = object_scale * object_scale;
    let mut numerator = 0.0_f32;
    let mut denominator = 0.0_f32;

    for j in 0..17 {
        if visibility[j] < 0.5 {
            continue;
        }
        denominator += 1.0;
        let dx = pred_kpts[[j, 0]] - gt_kpts[[j, 0]];
        let dy = pred_kpts[[j, 1]] - gt_kpts[[j, 1]];
        let d_sq = dx * dx + dy * dy;
        let k = COCO_KP_SIGMAS[j];
        let exp_arg = -d_sq / (2.0 * s_sq * k * k);
        numerator += exp_arg.exp();
    }

    if denominator > 0.0 {
        numerator / denominator
    } else {
        0.0
    }
}

/// Aggregate result type returned by [`aggregate_metrics`].
///
/// Extends the simpler [`MetricsResult`] with per-joint and per-frame details
/// needed for the full COCO-style evaluation report.
#[derive(Debug, Clone, Default)]
pub struct AggregatedMetrics {
    /// PCK@0.2 averaged over all frames.
    pub pck_02: f32,
    /// PCK@0.5 averaged over all frames.
    pub pck_05: f32,
    /// Per-joint PCK@0.2 `[17]`.
    pub per_joint_pck: [f32; 17],
    /// Mean OKS over all frames.
    pub oks: f32,
    /// Per-frame OKS values.
    pub oks_values: Vec<f32>,
    /// Number of frames evaluated.
    pub frames_evaluated: usize,
    /// Total number of visible keypoints evaluated.
    pub keypoints_evaluated: usize,
}

/// Aggregate PCK and OKS metrics over the full evaluation set.
///
/// `object_scale` is fixed at `1.0` (bounding boxes are not tracked in the
/// WiFi-DensePose CSI evaluation pipeline).
pub fn aggregate_metrics(
    pred_kpts: &[Array2<f32>],
    gt_kpts: &[Array2<f32>],
    visibility: &[Array1<f32>],
) -> AggregatedMetrics {
    assert_eq!(pred_kpts.len(), gt_kpts.len());
    assert_eq!(pred_kpts.len(), visibility.len());

    let n = pred_kpts.len();
    if n == 0 {
        return AggregatedMetrics::default();
    }

    let mut pck02_sum = 0.0_f32;
    let mut pck05_sum = 0.0_f32;
    let mut oks_values = Vec::with_capacity(n);
    let mut total_kps = 0_usize;

    for i in 0..n {
        let (_, tot, pck02) = compute_pck(&pred_kpts[i], &gt_kpts[i], &visibility[i], 0.2);
        let (_, _, pck05) = compute_pck(&pred_kpts[i], &gt_kpts[i], &visibility[i], 0.5);
        let oks = compute_oks(&pred_kpts[i], &gt_kpts[i], &visibility[i], 1.0);

        pck02_sum += pck02;
        pck05_sum += pck05;
        oks_values.push(oks);
        total_kps += tot;
    }

    let per_joint_pck = compute_per_joint_pck(pred_kpts, gt_kpts, visibility, 0.2);
    let mean_oks = oks_values.iter().copied().sum::<f32>() / n as f32;

    AggregatedMetrics {
        pck_02: pck02_sum / n as f32,
        pck_05: pck05_sum / n as f32,
        per_joint_pck,
        oks: mean_oks,
        oks_values,
        frames_evaluated: n,
        keypoints_evaluated: total_kps,
    }
}

// ---------------------------------------------------------------------------
// Hungarian algorithm (min-cost bipartite matching)
// ---------------------------------------------------------------------------

/// Cost matrix entry for keypoint-based person assignment.
#[derive(Debug, Clone)]
pub struct AssignmentEntry {
    /// Index of the predicted person.
    pub pred_idx: usize,
    /// Index of the ground-truth person.
    pub gt_idx: usize,
    /// Assignment cost (lower = better match).
    pub cost: f32,
}

/// Solve the optimal linear assignment problem using the Hungarian algorithm.
///
/// Returns the minimum-cost complete matching as a list of `(pred_idx, gt_idx)`
/// pairs.  For non-square matrices exactly `min(n_pred, n_gt)` pairs are
/// returned (the shorter side is fully matched).
///
/// # Algorithm
///
/// Implements the classical O(n³) potential-based Hungarian / Kuhn-Munkres
/// algorithm:
///
/// 1. Pads non-square cost matrices to square with a large sentinel value.
/// 2. Processes each row by finding the minimum-cost augmenting path using
///    Dijkstra-style potential relaxation.
/// 3. Strips padded assignments before returning.
pub fn hungarian_assignment(cost_matrix: &[Vec<f32>]) -> Vec<(usize, usize)> {
    if cost_matrix.is_empty() {
        return vec![];
    }
    let n_rows = cost_matrix.len();
    let n_cols = cost_matrix[0].len();
    if n_cols == 0 {
        return vec![];
    }

    let n = n_rows.max(n_cols);
    let inf = f64::MAX / 2.0;

    // Build a square cost matrix padded with `inf`.
    let mut c = vec![vec![inf; n]; n];
    for i in 0..n_rows {
        for j in 0..n_cols {
            c[i][j] = cost_matrix[i][j] as f64;
        }
    }

    // u[i]: potential for row i (1-indexed; index 0 unused).
    // v[j]: potential for column j (1-indexed; index 0 = dummy source).
    let mut u = vec![0.0_f64; n + 1];
    let mut v = vec![0.0_f64; n + 1];
    // p[j]: 1-indexed row assigned to column j (0 = unassigned).
    let mut p = vec![0_usize; n + 1];
    // way[j]: predecessor column j in the current augmenting path.
    let mut way = vec![0_usize; n + 1];

    for i in 1..=n {
        // Set the dummy source (column 0) to point to the current row.
        p[0] = i;
        let mut j0 = 0_usize;

        let mut min_val = vec![inf; n + 1];
        let mut used = vec![false; n + 1];

        // Shortest augmenting path with potential updates (Dijkstra-like).
        loop {
            used[j0] = true;
            let i0 = p[j0]; // 1-indexed row currently "in" column j0
            let mut delta = inf;
            let mut j1 = 0_usize;

            for j in 1..=n {
                if !used[j] {
                    let val = c[i0 - 1][j - 1] - u[i0] - v[j];
                    if val < min_val[j] {
                        min_val[j] = val;
                        way[j] = j0;
                    }
                    if min_val[j] < delta {
                        delta = min_val[j];
                        j1 = j;
                    }
                }
            }

            // Update potentials.
            for j in 0..=n {
                if used[j] {
                    u[p[j]] += delta;
                    v[j] -= delta;
                } else {
                    min_val[j] -= delta;
                }
            }

            j0 = j1;
            if p[j0] == 0 {
                break; // free column found → augmenting path complete
            }
        }

        // Trace back and augment the matching.
        loop {
            p[j0] = p[way[j0]];
            j0 = way[j0];
            if j0 == 0 {
                break;
            }
        }
    }

    // Collect real (non-padded) assignments.
    let mut assignments = Vec::new();
    for j in 1..=n {
        if p[j] != 0 {
            let pred_idx = p[j] - 1; // back to 0-indexed
            let gt_idx = j - 1;
            if pred_idx < n_rows && gt_idx < n_cols {
                assignments.push((pred_idx, gt_idx));
            }
        }
    }
    assignments.sort_unstable_by_key(|&(pred, _)| pred);
    assignments
}

/// Build the OKS cost matrix for multi-person matching.
///
/// Cost between predicted person `i` and GT person `j` is `1 − OKS(pred_i, gt_j)`.
pub fn build_oks_cost_matrix(
    pred_persons: &[Array2<f32>],
    gt_persons: &[Array2<f32>],
    visibility: &[Array1<f32>],
) -> Vec<Vec<f32>> {
    let n_pred = pred_persons.len();
    let n_gt = gt_persons.len();
    assert_eq!(gt_persons.len(), visibility.len());

    let mut matrix = vec![vec![1.0_f32; n_gt]; n_pred];
    for i in 0..n_pred {
        for j in 0..n_gt {
            let oks = compute_oks(&pred_persons[i], &gt_persons[j], &visibility[j], 1.0);
            matrix[i][j] = 1.0 - oks;
        }
    }
    matrix
}

/// Find an augmenting path in the bipartite matching graph.
///
/// Used internally for unit-capacity matching checks.  In the main training
/// pipeline `hungarian_assignment` is preferred for its optimal cost guarantee.
///
/// `adj[u]` is the list of `(v, weight)` edges from left-node `u`.
/// `matching[v]` gives the current left-node matched to right-node `v`.
pub fn find_augmenting_path(
    adj: &[Vec<(usize, f32)>],
    source: usize,
    _sink: usize,
    visited: &mut Vec<bool>,
    matching: &mut Vec<Option<usize>>,
) -> bool {
    for &(v, _weight) in &adj[source] {
        if !visited[v] {
            visited[v] = true;
            if matching[v].is_none()
                || find_augmenting_path(adj, matching[v].unwrap(), _sink, visited, matching)
            {
                matching[v] = Some(source);
                return true;
            }
        }
    }
    false
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

    // ── compute_pck free function ─────────────────────────────────────────────

    fn all_visible_17() -> Array1<f32> {
        Array1::ones(17)
    }

    fn uniform_kpts_17(x: f32, y: f32) -> Array2<f32> {
        let mut arr = Array2::zeros((17, 2));
        for j in 0..17 {
            arr[[j, 0]] = x;
            arr[[j, 1]] = y;
        }
        arr
    }

    #[test]
    fn compute_pck_perfect_is_one() {
        let kpts = uniform_kpts_17(0.5, 0.5);
        let vis = all_visible_17();
        let (correct, total, pck) = compute_pck(&kpts, &kpts, &vis, 0.2);
        assert_eq!(correct, 17);
        assert_eq!(total, 17);
        assert_abs_diff_eq!(pck, 1.0_f32, epsilon = 1e-6);
    }

    #[test]
    fn compute_pck_no_visible_is_zero() {
        let kpts = uniform_kpts_17(0.5, 0.5);
        let vis = Array1::zeros(17);
        let (correct, total, pck) = compute_pck(&kpts, &kpts, &vis, 0.2);
        assert_eq!(correct, 0);
        assert_eq!(total, 0);
        assert_eq!(pck, 0.0);
    }

    // ── compute_oks free function ─────────────────────────────────────────────

    #[test]
    fn compute_oks_identical_is_one() {
        let kpts = uniform_kpts_17(0.5, 0.5);
        let vis = all_visible_17();
        let oks = compute_oks(&kpts, &kpts, &vis, 1.0);
        assert_abs_diff_eq!(oks, 1.0_f32, epsilon = 1e-5);
    }

    #[test]
    fn compute_oks_no_visible_is_zero() {
        let kpts = uniform_kpts_17(0.5, 0.5);
        let vis = Array1::zeros(17);
        let oks = compute_oks(&kpts, &kpts, &vis, 1.0);
        assert_eq!(oks, 0.0);
    }

    #[test]
    fn compute_oks_in_unit_interval() {
        let pred = uniform_kpts_17(0.4, 0.6);
        let gt = uniform_kpts_17(0.5, 0.5);
        let vis = all_visible_17();
        let oks = compute_oks(&pred, &gt, &vis, 1.0);
        assert!(oks >= 0.0 && oks <= 1.0, "OKS={oks} outside [0,1]");
    }

    // ── aggregate_metrics ────────────────────────────────────────────────────

    #[test]
    fn aggregate_metrics_perfect() {
        let kpts: Vec<Array2<f32>> = (0..4).map(|_| uniform_kpts_17(0.5, 0.5)).collect();
        let vis: Vec<Array1<f32>> = (0..4).map(|_| all_visible_17()).collect();
        let result = aggregate_metrics(&kpts, &kpts, &vis);
        assert_eq!(result.frames_evaluated, 4);
        assert_abs_diff_eq!(result.pck_02, 1.0_f32, epsilon = 1e-5);
        assert_abs_diff_eq!(result.oks, 1.0_f32, epsilon = 1e-5);
    }

    #[test]
    fn aggregate_metrics_empty_is_default() {
        let result = aggregate_metrics(&[], &[], &[]);
        assert_eq!(result.frames_evaluated, 0);
        assert_eq!(result.oks, 0.0);
    }

    // ── hungarian_assignment ─────────────────────────────────────────────────

    #[test]
    fn hungarian_identity_2x2_assigns_diagonal() {
        // [[0, 1], [1, 0]] → optimal (0→0, 1→1) with total cost 0.
        let cost = vec![vec![0.0_f32, 1.0], vec![1.0, 0.0]];
        let mut assignments = hungarian_assignment(&cost);
        assignments.sort_unstable();
        assert_eq!(assignments, vec![(0, 0), (1, 1)]);
    }

    #[test]
    fn hungarian_swapped_2x2() {
        // [[1, 0], [0, 1]] → optimal (0→1, 1→0) with total cost 0.
        let cost = vec![vec![1.0_f32, 0.0], vec![0.0, 1.0]];
        let mut assignments = hungarian_assignment(&cost);
        assignments.sort_unstable();
        assert_eq!(assignments, vec![(0, 1), (1, 0)]);
    }

    #[test]
    fn hungarian_3x3_identity() {
        let cost = vec![
            vec![0.0_f32, 10.0, 10.0],
            vec![10.0, 0.0, 10.0],
            vec![10.0, 10.0, 0.0],
        ];
        let mut assignments = hungarian_assignment(&cost);
        assignments.sort_unstable();
        assert_eq!(assignments, vec![(0, 0), (1, 1), (2, 2)]);
    }

    #[test]
    fn hungarian_empty_matrix() {
        assert!(hungarian_assignment(&[]).is_empty());
    }

    #[test]
    fn hungarian_single_element() {
        let assignments = hungarian_assignment(&[vec![0.5_f32]]);
        assert_eq!(assignments, vec![(0, 0)]);
    }

    #[test]
    fn hungarian_rectangular_fewer_gt_than_pred() {
        // 3 predicted, 2 GT → only 2 assignments.
        let cost = vec![
            vec![5.0_f32, 9.0],
            vec![4.0, 6.0],
            vec![3.0, 1.0],
        ];
        let assignments = hungarian_assignment(&cost);
        assert_eq!(assignments.len(), 2);
        // GT indices must be unique.
        let gt_set: std::collections::HashSet<usize> =
            assignments.iter().map(|&(_, g)| g).collect();
        assert_eq!(gt_set.len(), 2);
    }

    // ── OKS cost matrix ───────────────────────────────────────────────────────

    #[test]
    fn oks_cost_matrix_diagonal_near_zero() {
        let persons: Vec<Array2<f32>> = (0..3)
            .map(|i| uniform_kpts_17(i as f32 * 0.3, 0.5))
            .collect();
        let vis: Vec<Array1<f32>> = (0..3).map(|_| all_visible_17()).collect();
        let mat = build_oks_cost_matrix(&persons, &persons, &vis);
        for i in 0..3 {
            assert!(mat[i][i] < 1e-4, "cost[{i}][{i}]={} should be ≈0", mat[i][i]);
        }
    }

    // ── find_augmenting_path (helper smoke test) ──────────────────────────────

    #[test]
    fn find_augmenting_path_basic() {
        let adj: Vec<Vec<(usize, f32)>> = vec![
            vec![(0, 1.0)],
            vec![(1, 1.0)],
        ];
        let mut matching = vec![None; 2];
        let mut visited = vec![false; 2];
        let found = find_augmenting_path(&adj, 0, 2, &mut visited, &mut matching);
        assert!(found);
        assert_eq!(matching[0], Some(0));
    }
}
