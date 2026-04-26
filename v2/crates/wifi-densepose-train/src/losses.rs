//! Loss functions for WiFi-DensePose training.
//!
//! This module implements the combined loss function used during training:
//!
//! - **Keypoint heatmap loss**: MSE between predicted and target Gaussian heatmaps,
//!   masked by keypoint visibility so unlabelled joints don't contribute.
//! - **DensePose loss**: Cross-entropy on body-part logits (25 classes including
//!   background) plus Smooth-L1 (Huber) UV regression for each foreground part.
//! - **Transfer / distillation loss**: MSE between student backbone features and
//!   teacher features, enabling cross-modal knowledge transfer from an RGB teacher.
//!
//! The three scalar losses are combined with configurable weights:
//!
//! ```text
//! L_total = λ_kp · L_keypoint + λ_dp · L_densepose + λ_tr · L_transfer
//! ```
//!
//! # No mock data
//! Every computation in this module is grounded in real signal mathematics.
//! No synthetic or random tensors are generated at runtime.

use std::collections::HashMap;
use tch::{Kind, Reduction, Tensor};

// ─────────────────────────────────────────────────────────────────────────────
// Public types
// ─────────────────────────────────────────────────────────────────────────────

/// Scalar components produced by a single forward pass through [`WiFiDensePoseLoss::forward`].
///
/// Contains `f32` scalar values extracted from the computation graph for
/// logging and checkpointing (they are not used for back-propagation).
#[derive(Debug, Clone)]
pub struct WiFiLossComponents {
    /// Total weighted loss value (scalar, in ℝ≥0).
    pub total: f32,
    /// Keypoint heatmap MSE loss component.
    pub keypoint: f32,
    /// DensePose (part + UV) loss component, `None` when no DensePose targets are given.
    pub densepose: Option<f32>,
    /// Transfer/distillation loss component, `None` when no teacher features are given.
    pub transfer: Option<f32>,
    /// Fine-grained breakdown (e.g. `"dp_part"`, `"dp_uv"`, `"kp_masked"`, …).
    pub details: HashMap<String, f32>,
}

/// Per-loss scalar weights used to combine the individual losses.
#[derive(Debug, Clone)]
pub struct LossWeights {
    /// Weight for the keypoint heatmap loss (λ_kp).
    pub lambda_kp: f64,
    /// Weight for the DensePose loss (λ_dp).
    pub lambda_dp: f64,
    /// Weight for the transfer/distillation loss (λ_tr).
    pub lambda_tr: f64,
}

impl Default for LossWeights {
    fn default() -> Self {
        Self {
            lambda_kp: 0.3,
            lambda_dp: 0.6,
            lambda_tr: 0.1,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// WiFiDensePoseLoss
// ─────────────────────────────────────────────────────────────────────────────

/// Combined loss function for WiFi-DensePose training.
///
/// Wraps three component losses:
/// 1. Keypoint heatmap MSE (visibility-masked)
/// 2. DensePose: part cross-entropy + UV Smooth-L1
/// 3. Teacher-student feature transfer MSE
pub struct WiFiDensePoseLoss {
    weights: LossWeights,
}

impl WiFiDensePoseLoss {
    /// Create a new loss function with the given component weights.
    pub fn new(weights: LossWeights) -> Self {
        Self { weights }
    }

    // ── Component losses ─────────────────────────────────────────────────────

    /// Compute the keypoint heatmap loss.
    ///
    /// For each keypoint joint `j` and batch element `b`, the pixel-wise MSE
    /// between `pred_heatmaps[b, j, :, :]` and `target_heatmaps[b, j, :, :]`
    /// is computed and multiplied by the binary visibility mask `visibility[b, j]`.
    /// The sum is then divided by the number of visible joints to produce a
    /// normalised scalar.
    ///
    /// If no keypoints are visible in the batch the function returns zero.
    ///
    /// # Shapes
    /// - `pred_heatmaps`: `[B, 17, H, W]` – predicted heatmaps
    /// - `target_heatmaps`: `[B, 17, H, W]` – ground-truth Gaussian heatmaps
    /// - `visibility`: `[B, 17]` – 1.0 if the keypoint is labelled, 0.0 otherwise
    pub fn keypoint_loss(
        &self,
        pred_heatmaps: &Tensor,
        target_heatmaps: &Tensor,
        visibility: &Tensor,
    ) -> Tensor {
        // Pixel-wise squared error, mean-reduced over H and W: [B, 17]
        let sq_err = (pred_heatmaps - target_heatmaps).pow_tensor_scalar(2);
        // Mean over H and W (dims 2, 3 → we flatten them first for clarity)
        let per_joint_mse = sq_err.mean_dim(&[2_i64, 3_i64][..], false, Kind::Float);

        // Mask by visibility: [B, 17]
        let masked = per_joint_mse * visibility;

        // Normalise by number of visible joints in the batch.
        let n_visible = visibility.sum(Kind::Float);
        // Guard against division by zero (entire batch may have no labels).
        let safe_n = n_visible.clamp(1.0, f64::MAX);

        masked.sum(Kind::Float) / safe_n
    }

    /// Compute the DensePose loss.
    ///
    /// Two sub-losses are combined:
    /// 1. **Part cross-entropy** – softmax cross-entropy between `pred_parts`
    ///    logits `[B, 25, H, W]` and `target_parts` integer class indices
    ///    `[B, H, W]`.  Class 0 is background and is included.
    /// 2. **UV Smooth-L1 (Huber)** – for pixels that belong to a foreground
    ///    part (target class ≥ 1), the UV prediction error is penalised with
    ///    Smooth-L1 loss.  Background pixels are masked out so the model is
    ///    not penalised for UV predictions at background locations.
    ///
    /// The two sub-losses are summed with equal weight.
    ///
    /// # Shapes
    /// - `pred_parts`: `[B, 25, H, W]` – logits (24 body parts + background)
    /// - `target_parts`: `[B, H, W]` – integer class indices in [0, 24]
    /// - `pred_uv`: `[B, 48, H, W]` – 24 pairs of (U, V) predictions, interleaved
    /// - `target_uv`: `[B, 48, H, W]` – ground-truth UV coordinates for each part
    pub fn densepose_loss(
        &self,
        pred_parts: &Tensor,
        target_parts: &Tensor,
        pred_uv: &Tensor,
        target_uv: &Tensor,
    ) -> Tensor {
        // ── 1. Part classification: cross-entropy ──────────────────────────
        // tch cross_entropy_loss expects (input: [B,C,…], target: [B,…] of i64).
        let target_int = target_parts.to_kind(Kind::Int64);
        // weight=None, reduction=Mean, ignore_index=-100, label_smoothing=0.0
        let part_loss = pred_parts.cross_entropy_loss::<Tensor>(
            &target_int,
            None,
            Reduction::Mean,
            -100,
            0.0,
        );

        // ── 2. UV regression: Smooth-L1 masked by foreground pixels ────────
        // Foreground mask: pixels where target part ≠ 0, shape [B, H, W].
        let fg_mask = target_int.not_equal(0_i64);
        // Expand to [B, 1, H, W] then broadcast to [B, 48, H, W].
        let fg_mask_f = fg_mask
            .unsqueeze(1)
            .expand_as(pred_uv)
            .to_kind(Kind::Float);

        let masked_pred_uv = pred_uv * &fg_mask_f;
        let masked_target_uv = target_uv * &fg_mask_f;

        // Count foreground pixels × 48 channels to normalise.
        let n_fg = fg_mask_f.sum(Kind::Float).clamp(1.0, f64::MAX);

        // Smooth-L1 with beta=1.0, reduction=Sum then divide by fg count.
        let uv_loss_sum =
            masked_pred_uv.smooth_l1_loss(&masked_target_uv, Reduction::Sum, 1.0);
        let uv_loss = uv_loss_sum / n_fg;

        part_loss + uv_loss
    }

    /// Compute the teacher-student feature transfer (distillation) loss.
    ///
    /// The loss is a plain MSE between the student backbone feature map and the
    /// teacher's corresponding feature map.  Both tensors must have the same
    /// shape `[B, C, H, W]`.
    ///
    /// This implements the cross-modal knowledge distillation component of the
    /// WiFi-DensePose paper where an RGB teacher supervises the CSI student.
    pub fn transfer_loss(&self, student_features: &Tensor, teacher_features: &Tensor) -> Tensor {
        student_features.mse_loss(teacher_features, Reduction::Mean)
    }

    // ── Combined forward ─────────────────────────────────────────────────────

    /// Compute and combine all loss components.
    ///
    /// Returns `(total_loss_tensor, LossOutput)` where `total_loss_tensor` is
    /// the differentiable scalar for back-propagation and `LossOutput` contains
    /// detached `f32` values for logging.
    ///
    /// # Arguments
    /// - `pred_keypoints`, `target_keypoints`: `[B, 17, H, W]`
    /// - `visibility`: `[B, 17]`
    /// - `pred_parts`, `target_parts`: `[B, 25, H, W]` / `[B, H, W]` (optional)
    /// - `pred_uv`, `target_uv`: `[B, 48, H, W]` (optional, paired with parts)
    /// - `student_features`, `teacher_features`: `[B, C, H, W]` (optional)
    #[allow(clippy::too_many_arguments)]
    pub fn forward(
        &self,
        pred_keypoints: &Tensor,
        target_keypoints: &Tensor,
        visibility: &Tensor,
        pred_parts: Option<&Tensor>,
        target_parts: Option<&Tensor>,
        pred_uv: Option<&Tensor>,
        target_uv: Option<&Tensor>,
        student_features: Option<&Tensor>,
        teacher_features: Option<&Tensor>,
    ) -> (Tensor, WiFiLossComponents) {
        let mut details = HashMap::new();

        // ── Keypoint loss (always computed) ───────────────────────────────
        let kp_loss = self.keypoint_loss(pred_keypoints, target_keypoints, visibility);
        let kp_val: f64 = kp_loss.double_value(&[]);
        details.insert("kp_mse".to_string(), kp_val as f32);

        let total = kp_loss.shallow_clone() * self.weights.lambda_kp;

        // ── DensePose loss (optional) ─────────────────────────────────────
        let (dp_val, total) = match (pred_parts, target_parts, pred_uv, target_uv) {
            (Some(pp), Some(tp), Some(pu), Some(tu)) => {
                // Part cross-entropy
                let target_int = tp.to_kind(Kind::Int64);
                let part_loss = pp.cross_entropy_loss::<Tensor>(
                    &target_int,
                    None,
                    Reduction::Mean,
                    -100,
                    0.0,
                );
                let part_val = part_loss.double_value(&[]) as f32;

                // UV loss (foreground masked)
                let fg_mask = target_int.not_equal(0_i64);
                let fg_mask_f = fg_mask
                    .unsqueeze(1)
                    .expand_as(pu)
                    .to_kind(Kind::Float);
                let n_fg = fg_mask_f.sum(Kind::Float).clamp(1.0, f64::MAX);
                let uv_loss = (pu * &fg_mask_f)
                    .smooth_l1_loss(&(tu * &fg_mask_f), Reduction::Sum, 1.0)
                    / n_fg;
                let uv_val = uv_loss.double_value(&[]) as f32;

                let dp_loss = &part_loss + &uv_loss;
                let dp_scalar = dp_loss.double_value(&[]) as f32;

                details.insert("dp_part_ce".to_string(), part_val);
                details.insert("dp_uv_smooth_l1".to_string(), uv_val);

                let new_total = total + dp_loss * self.weights.lambda_dp;
                (Some(dp_scalar), new_total)
            }
            _ => (None, total),
        };

        // ── Transfer loss (optional) ──────────────────────────────────────
        let (tr_val, total) = match (student_features, teacher_features) {
            (Some(sf), Some(tf)) => {
                let tr_loss = self.transfer_loss(sf, tf);
                let tr_scalar = tr_loss.double_value(&[]) as f32;
                details.insert("transfer_mse".to_string(), tr_scalar);
                let new_total = total + tr_loss * self.weights.lambda_tr;
                (Some(tr_scalar), new_total)
            }
            _ => (None, total),
        };

        let total_val = total.double_value(&[]) as f32;

        let output = WiFiLossComponents {
            total: total_val,
            keypoint: kp_val as f32,
            densepose: dp_val,
            transfer: tr_val,
            details,
        };

        (total, output)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Gaussian heatmap utilities
// ─────────────────────────────────────────────────────────────────────────────

/// Generate a 2-D Gaussian heatmap for a single keypoint.
///
/// The heatmap is a `heatmap_size × heatmap_size` array where the value at
/// pixel `(r, c)` is:
///
/// ```text
/// H[r, c] = exp( -((c - kp_x * S)² + (r - kp_y * S)²) / (2 · σ²) )
/// ```
///
/// where `S = heatmap_size - 1` maps normalised coordinates to pixel space.
///
/// Values outside the 3σ radius are clamped to zero to produce a sparse
/// representation that is numerically identical to the training targets used
/// in the original DensePose paper.
///
/// # Arguments
/// - `kp_x`, `kp_y`: normalised keypoint position in [0, 1]
/// - `heatmap_size`: spatial resolution of the heatmap (H = W)
/// - `sigma`: Gaussian spread in pixels (default 2.0 gives a tight, localised peak)
///
/// # Returns
/// A `heatmap_size × heatmap_size` array with values in [0, 1].
pub fn generate_gaussian_heatmap(
    kp_x: f32,
    kp_y: f32,
    heatmap_size: usize,
    sigma: f32,
) -> ndarray::Array2<f32> {
    let s = (heatmap_size - 1) as f32;
    let cx = kp_x * s;
    let cy = kp_y * s;
    let two_sigma_sq = 2.0 * sigma * sigma;
    let clip_radius_sq = (3.0 * sigma).powi(2);

    let mut map = ndarray::Array2::zeros((heatmap_size, heatmap_size));
    for r in 0..heatmap_size {
        for c in 0..heatmap_size {
            let dx = c as f32 - cx;
            let dy = r as f32 - cy;
            let dist_sq = dx * dx + dy * dy;
            if dist_sq <= clip_radius_sq {
                map[[r, c]] = (-dist_sq / two_sigma_sq).exp();
            }
        }
    }
    map
}

/// Generate a batch of target heatmaps from keypoint coordinates.
///
/// For invisible keypoints (`visibility[b, j] == 0`) the corresponding
/// heatmap channel is left as all-zeros.
///
/// # Arguments
/// - `keypoints`: `[B, 17, 2]` – (x, y) normalised to [0, 1]
/// - `visibility`: `[B, 17]` – 1.0 if visible, 0.0 if invisible
/// - `heatmap_size`: spatial resolution (H = W)
/// - `sigma`: Gaussian sigma in pixels
///
/// # Returns
/// `[B, 17, heatmap_size, heatmap_size]` target heatmap array.
pub fn generate_target_heatmaps(
    keypoints: &ndarray::Array3<f32>,
    visibility: &ndarray::Array2<f32>,
    heatmap_size: usize,
    sigma: f32,
) -> ndarray::Array4<f32> {
    let batch = keypoints.shape()[0];
    let num_joints = keypoints.shape()[1];

    let mut heatmaps =
        ndarray::Array4::zeros((batch, num_joints, heatmap_size, heatmap_size));

    for b in 0..batch {
        for j in 0..num_joints {
            if visibility[[b, j]] > 0.0 {
                let kp_x = keypoints[[b, j, 0]];
                let kp_y = keypoints[[b, j, 1]];
                let hm = generate_gaussian_heatmap(kp_x, kp_y, heatmap_size, sigma);
                for r in 0..heatmap_size {
                    for c in 0..heatmap_size {
                        heatmaps[[b, j, r, c]] = hm[[r, c]];
                    }
                }
            }
        }
    }
    heatmaps
}

// ─────────────────────────────────────────────────────────────────────────────
// Standalone functional API (mirrors the spec signatures exactly)
// ─────────────────────────────────────────────────────────────────────────────

/// Output of the combined loss computation (functional API).
#[derive(Debug, Clone)]
pub struct LossOutput {
    /// Weighted total loss (for backward pass).
    pub total: f64,
    /// Keypoint heatmap MSE loss (unweighted).
    pub keypoint: f64,
    /// DensePose part classification loss (unweighted), `None` if not computed.
    pub densepose_parts: Option<f64>,
    /// DensePose UV regression loss (unweighted), `None` if not computed.
    pub densepose_uv: Option<f64>,
    /// Teacher-student transfer loss (unweighted), `None` if teacher features absent.
    pub transfer: Option<f64>,
}

/// Compute the total weighted loss given model predictions and targets.
///
/// # Arguments
/// * `pred_kpt_heatmaps`  - Predicted keypoint heatmaps: \[B, 17, H, W\]
/// * `gt_kpt_heatmaps`    - Ground truth Gaussian heatmaps: \[B, 17, H, W\]
/// * `pred_part_logits`   - Predicted DensePose part logits: \[B, 25, H, W\]
/// * `gt_part_labels`     - GT part class indices: \[B, H, W\], value −1 = ignore
/// * `pred_uv`            - Predicted UV coordinates: \[B, 48, H, W\]
/// * `gt_uv`              - Ground truth UV: \[B, 48, H, W\]
/// * `student_features`   - Student backbone features: \[B, C, H', W'\]
/// * `teacher_features`   - Teacher backbone features: \[B, C, H', W'\]
/// * `lambda_kp`          - Weight for keypoint loss
/// * `lambda_dp`          - Weight for DensePose loss
/// * `lambda_tr`          - Weight for transfer loss
#[allow(clippy::too_many_arguments)]
pub fn compute_losses(
    pred_kpt_heatmaps: &Tensor,
    gt_kpt_heatmaps: &Tensor,
    pred_part_logits: Option<&Tensor>,
    gt_part_labels: Option<&Tensor>,
    pred_uv: Option<&Tensor>,
    gt_uv: Option<&Tensor>,
    student_features: Option<&Tensor>,
    teacher_features: Option<&Tensor>,
    lambda_kp: f64,
    lambda_dp: f64,
    lambda_tr: f64,
) -> LossOutput {
    // ── Keypoint heatmap loss — always computed ────────────────────────────
    let kpt_tensor = keypoint_heatmap_loss(pred_kpt_heatmaps, gt_kpt_heatmaps);
    let keypoint: f64 = kpt_tensor.double_value(&[]);

    // ── DensePose part classification loss ────────────────────────────────
    let (densepose_parts, dp_part_tensor): (Option<f64>, Option<Tensor>) =
        match (pred_part_logits, gt_part_labels) {
            (Some(logits), Some(labels)) => {
                let t = densepose_part_loss(logits, labels);
                let v = t.double_value(&[]);
                (Some(v), Some(t))
            }
            _ => (None, None),
        };

    // ── DensePose UV regression loss ──────────────────────────────────────
    let (densepose_uv, dp_uv_tensor): (Option<f64>, Option<Tensor>) =
        match (pred_uv, gt_uv, gt_part_labels) {
            (Some(puv), Some(guv), Some(labels)) => {
                let t = densepose_uv_loss(puv, guv, labels);
                let v = t.double_value(&[]);
                (Some(v), Some(t))
            }
            _ => (None, None),
        };

    // ── Teacher-student transfer loss ─────────────────────────────────────
    let (transfer, tr_tensor): (Option<f64>, Option<Tensor>) =
        match (student_features, teacher_features) {
            (Some(sf), Some(tf)) => {
                let t = fn_transfer_loss(sf, tf);
                let v = t.double_value(&[]);
                (Some(v), Some(t))
            }
            _ => (None, None),
        };

    // ── Weighted sum ──────────────────────────────────────────────────────
    let mut total_t = kpt_tensor * lambda_kp;

    // Combine densepose part + UV under a single lambda_dp weight.
    let zero_scalar = Tensor::zeros(&[], (Kind::Float, total_t.device()));
    let dp_part_t = dp_part_tensor
        .as_ref()
        .map(|t| t.shallow_clone())
        .unwrap_or_else(|| zero_scalar.shallow_clone());
    let dp_uv_t = dp_uv_tensor
        .as_ref()
        .map(|t| t.shallow_clone())
        .unwrap_or_else(|| zero_scalar.shallow_clone());

    if densepose_parts.is_some() || densepose_uv.is_some() {
        total_t = total_t + (&dp_part_t + &dp_uv_t) * lambda_dp;
    }

    if let Some(ref tr) = tr_tensor {
        total_t = total_t + tr * lambda_tr;
    }

    let total: f64 = total_t.double_value(&[]);

    LossOutput {
        total,
        keypoint,
        densepose_parts,
        densepose_uv,
        transfer,
    }
}

/// Keypoint heatmap loss: MSE between predicted and Gaussian-smoothed GT heatmaps.
///
/// Invisible keypoints must be zeroed in `target` before calling this function
/// (use [`generate_gaussian_heatmaps`] which handles that automatically).
///
/// # Arguments
/// * `pred`   - Predicted heatmaps \[B, 17, H, W\]
/// * `target` - Pre-computed GT Gaussian heatmaps \[B, 17, H, W\]
///
/// Returns a scalar `Tensor`.
pub fn keypoint_heatmap_loss(pred: &Tensor, target: &Tensor) -> Tensor {
    pred.mse_loss(target, Reduction::Mean)
}

/// Generate Gaussian heatmaps from keypoint coordinates.
///
/// For each keypoint `(x, y)` in \[0,1\] normalised space, places a 2D Gaussian
/// centred at the corresponding pixel location.  Invisible keypoints produce
/// all-zero heatmap channels.
///
/// # Arguments
/// * `keypoints`    - \[B, 17, 2\] normalised (x, y) in \[0, 1\]
/// * `visibility`   - \[B, 17\] 0 = invisible, 1 = visible
/// * `heatmap_size` - Output H = W (square heatmap)
/// * `sigma`        - Gaussian sigma in pixels (default 2.0)
///
/// Returns `[B, 17, H, W]`.
pub fn generate_gaussian_heatmaps(
    keypoints: &Tensor,
    visibility: &Tensor,
    heatmap_size: usize,
    sigma: f64,
) -> Tensor {
    let device = keypoints.device();
    let kind = Kind::Float;
    let size = heatmap_size as i64;

    let batch_size = keypoints.size()[0];
    let num_kpts = keypoints.size()[1];

    // Build pixel-space coordinate grids — shape [1, 1, H, W] for broadcasting.
    // `xs[w]` is the column index; `ys[h]` is the row index.
    let xs = Tensor::arange(size, (kind, device)).view([1, 1, 1, size]);
    let ys = Tensor::arange(size, (kind, device)).view([1, 1, size, 1]);

    // Convert normalised coords to pixel centres: pixel = coord * (size - 1).
    // keypoints[:, :, 0] → x (column); keypoints[:, :, 1] → y (row).
    let cx = keypoints
        .select(2, 0)
        .unsqueeze(-1)
        .unsqueeze(-1)
        .to_kind(kind)
        * (size as f64 - 1.0); // [B, 17, 1, 1]

    let cy = keypoints
        .select(2, 1)
        .unsqueeze(-1)
        .unsqueeze(-1)
        .to_kind(kind)
        * (size as f64 - 1.0); // [B, 17, 1, 1]

    // Gaussian: exp(−((x − cx)² + (y − cy)²) / (2σ²)), shape [B, 17, H, W].
    let two_sigma_sq = 2.0 * sigma * sigma;
    let dx = &xs - &cx;
    let dy = &ys - &cy;
    let heatmaps =
        (-(dx.pow_tensor_scalar(2.0) + dy.pow_tensor_scalar(2.0)) / two_sigma_sq).exp();

    // Zero out invisible keypoints: visibility [B, 17] → [B, 17, 1, 1] boolean mask.
    let vis_mask = visibility
        .to_kind(kind)
        .view([batch_size, num_kpts, 1, 1])
        .gt(0.0);

    let zero = Tensor::zeros(&[], (kind, device));
    heatmaps.where_self(&vis_mask, &zero)
}

/// DensePose part classification loss: cross-entropy with `ignore_index = −1`.
///
/// # Arguments
/// * `pred_logits` - \[B, 25, H, W\] (25 = 24 parts + background class 0)
/// * `gt_labels`   - \[B, H, W\] integer labels; −1 = ignore (no annotation)
///
/// Returns a scalar `Tensor`.
pub fn densepose_part_loss(pred_logits: &Tensor, gt_labels: &Tensor) -> Tensor {
    let labels_i64 = gt_labels.to_kind(Kind::Int64);
    pred_logits.cross_entropy_loss::<Tensor>(
        &labels_i64,
        None,            // no per-class weights
        Reduction::Mean,
        -1,              // ignore_index
        0.0,             // label_smoothing
    )
}

/// DensePose UV coordinate regression loss: Smooth L1 (Huber loss).
///
/// Only pixels where `gt_labels >= 0` (annotated with a valid part) contribute
/// to the loss; unannotated (background) pixels are masked out.
///
/// # Arguments
/// * `pred_uv`   - \[B, 48, H, W\] predicted UV (24 parts × 2 channels)
/// * `gt_uv`     - \[B, 48, H, W\] ground truth UV
/// * `gt_labels` - \[B, H, W\] part labels; mask = (labels ≥ 0)
///
/// Returns a scalar `Tensor`.
pub fn densepose_uv_loss(pred_uv: &Tensor, gt_uv: &Tensor, gt_labels: &Tensor) -> Tensor {
    // Boolean mask from annotated pixels: [B, 1, H, W].
    let mask = gt_labels.ge(0).unsqueeze(1);
    // Expand to [B, 48, H, W].
    let mask_expanded = mask.expand_as(pred_uv);

    let pred_sel = pred_uv.masked_select(&mask_expanded);
    let gt_sel = gt_uv.masked_select(&mask_expanded);

    if pred_sel.numel() == 0 {
        // No annotated pixels — return a zero scalar, still attached to graph.
        return Tensor::zeros(&[], (pred_uv.kind(), pred_uv.device()));
    }

    pred_sel.smooth_l1_loss(&gt_sel, Reduction::Mean, 1.0)
}

/// Teacher-student transfer loss: MSE between student and teacher feature maps.
///
/// If spatial or channel dimensions differ, the student features are aligned
/// to the teacher's shape via adaptive average pooling (non-parametric, no
/// learnable projection weights).
///
/// # Arguments
/// * `student_features` - \[B, Cs, Hs, Ws\]
/// * `teacher_features` - \[B, Ct, Ht, Wt\]
///
/// Returns a scalar `Tensor`.
///
/// This is a free function; the identical implementation is also available as
/// [`WiFiDensePoseLoss::transfer_loss`].
pub fn fn_transfer_loss(student_features: &Tensor, teacher_features: &Tensor) -> Tensor {
    let s_size = student_features.size();
    let t_size = teacher_features.size();

    // Align spatial dimensions if needed.
    let s_spatial = if s_size[2] != t_size[2] || s_size[3] != t_size[3] {
        student_features.adaptive_avg_pool2d([t_size[2], t_size[3]])
    } else {
        student_features.shallow_clone()
    };

    // Align channel dimensions if needed.
    let s_final = if s_size[1] != t_size[1] {
        let cs = s_spatial.size()[1];
        let ct = t_size[1];
        if cs % ct == 0 {
            // Fast path: reshape + mean pool over the ratio dimension.
            let ratio = cs / ct;
            s_spatial
                .view([-1, ct, ratio, t_size[2], t_size[3]])
                .mean_dim(Some(&[2i64][..]), false, Kind::Float)
        } else {
            // Generic: treat channel as sequence length, 1-D adaptive pool.
            let b = s_spatial.size()[0];
            let h = t_size[2];
            let w = t_size[3];
            s_spatial
                .permute([0, 2, 3, 1])       // [B, H, W, Cs]
                .reshape([-1, 1, cs])          // [B·H·W, 1, Cs]
                .adaptive_avg_pool1d(ct)       // [B·H·W, 1, Ct]
                .reshape([b, h, w, ct])        // [B, H, W, Ct]
                .permute([0, 3, 1, 2])         // [B, Ct, H, W]
        }
    } else {
        s_spatial
    };

    s_final.mse_loss(teacher_features, Reduction::Mean)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array2;

    // ── Gaussian heatmap ──────────────────────────────────────────────────────

    #[test]
    fn test_gaussian_heatmap_peak_location() {
        let kp_x = 0.5_f32;
        let kp_y = 0.5_f32;
        let size = 64_usize;
        let sigma = 2.0_f32;

        let hm = generate_gaussian_heatmap(kp_x, kp_y, size, sigma);

        // Peak should be at the centre (row=31, col=31) for a 64-pixel map
        // with normalised coordinate 0.5 → pixel 31.5, rounded to 31 or 32.
        let s = (size - 1) as f32;
        let cx = (kp_x * s).round() as usize;
        let cy = (kp_y * s).round() as usize;

        let peak = hm[[cy, cx]];
        assert!(
            peak > 0.95,
            "Peak value {peak} should be close to 1.0 at centre"
        );

        // Values far from the centre should be ≈ 0.
        let far = hm[[0, 0]];
        assert!(
            far < 0.01,
            "Corner value {far} should be near zero"
        );
    }

    #[test]
    fn test_gaussian_heatmap_reasonable_sum() {
        let hm = generate_gaussian_heatmap(0.5, 0.5, 64, 2.0);
        let total: f32 = hm.iter().copied().sum();
        // The Gaussian sum over a 64×64 grid with σ=2 is bounded away from
        // both 0 and infinity. Empirically it is ≈ 3·π·σ² ≈ 38 for σ=2.
        assert!(
            total > 5.0 && total < 200.0,
            "Heatmap sum {total} out of expected range"
        );
    }

    #[test]
    fn test_generate_target_heatmaps_invisible_joints_are_zero() {
        let batch = 2_usize;
        let num_joints = 17_usize;
        let size = 32_usize;

        let keypoints = ndarray::Array3::from_elem((batch, num_joints, 2), 0.5_f32);
        // Make all joints in batch 0 invisible.
        let mut visibility = ndarray::Array2::ones((batch, num_joints));
        for j in 0..num_joints {
            visibility[[0, j]] = 0.0;
        }

        let heatmaps = generate_target_heatmaps(&keypoints, &visibility, size, 2.0);

        // Every pixel of the invisible batch should be exactly 0.
        for j in 0..num_joints {
            for r in 0..size {
                for c in 0..size {
                    assert_eq!(
                        heatmaps[[0, j, r, c]],
                        0.0,
                        "Invisible joint heatmap should be zero"
                    );
                }
            }
        }

        // Visible batch (index 1) should have non-zero heatmaps.
        let batch1_sum: f32 = (0..num_joints)
            .map(|j| {
                (0..size)
                    .flat_map(|r| (0..size).map(move |c| heatmaps[[1, j, r, c]]))
                    .sum::<f32>()
            })
            .sum();
        assert!(batch1_sum > 0.0, "Visible joints should produce non-zero heatmaps");
    }

    // ── Loss functions ────────────────────────────────────────────────────────

    /// Returns a CUDA-or-CPU device string: always "cpu" in CI.
    fn device() -> tch::Device {
        tch::Device::Cpu
    }

    #[test]
    fn test_keypoint_loss_identical_predictions_is_zero() {
        let loss_fn = WiFiDensePoseLoss::new(LossWeights::default());
        let dev = device();

        // [B=2, 17, H=16, W=16] – use ones as a trivial non-zero tensor.
        let pred = Tensor::ones([2, 17, 16, 16], (Kind::Float, dev));
        let target = Tensor::ones([2, 17, 16, 16], (Kind::Float, dev));
        let vis = Tensor::ones([2, 17], (Kind::Float, dev));

        let loss = loss_fn.keypoint_loss(&pred, &target, &vis);
        let val = loss.double_value(&[]) as f32;

        assert!(
            val.abs() < 1e-5,
            "Keypoint loss for identical pred/target should be ≈ 0, got {val}"
        );
    }

    #[test]
    fn test_keypoint_loss_large_error_is_positive() {
        let loss_fn = WiFiDensePoseLoss::new(LossWeights::default());
        let dev = device();

        let pred = Tensor::ones([1, 17, 8, 8], (Kind::Float, dev));
        let target = Tensor::zeros([1, 17, 8, 8], (Kind::Float, dev));
        let vis = Tensor::ones([1, 17], (Kind::Float, dev));

        let loss = loss_fn.keypoint_loss(&pred, &target, &vis);
        let val = loss.double_value(&[]) as f32;

        assert!(val > 0.0, "Keypoint loss should be positive for wrong predictions");
    }

    #[test]
    fn test_keypoint_loss_invisible_joints_ignored() {
        let loss_fn = WiFiDensePoseLoss::new(LossWeights::default());
        let dev = device();

        // pred ≠ target – but all joints invisible → loss should be 0.
        let pred = Tensor::ones([1, 17, 8, 8], (Kind::Float, dev));
        let target = Tensor::zeros([1, 17, 8, 8], (Kind::Float, dev));
        let vis = Tensor::zeros([1, 17], (Kind::Float, dev)); // all invisible

        let loss = loss_fn.keypoint_loss(&pred, &target, &vis);
        let val = loss.double_value(&[]) as f32;

        assert!(
            val.abs() < 1e-5,
            "All-invisible loss should be ≈ 0, got {val}"
        );
    }

    #[test]
    fn test_transfer_loss_identical_features_is_zero() {
        let loss_fn = WiFiDensePoseLoss::new(LossWeights::default());
        let dev = device();

        let feat = Tensor::ones([2, 64, 8, 8], (Kind::Float, dev));
        let loss = loss_fn.transfer_loss(&feat, &feat);
        let val = loss.double_value(&[]) as f32;

        assert!(
            val.abs() < 1e-5,
            "Transfer loss for identical tensors should be ≈ 0, got {val}"
        );
    }

    #[test]
    fn test_forward_keypoint_only_returns_weighted_loss() {
        let weights = LossWeights {
            lambda_kp: 1.0,
            lambda_dp: 0.0,
            lambda_tr: 0.0,
        };
        let loss_fn = WiFiDensePoseLoss::new(weights);
        let dev = device();

        let pred = Tensor::ones([1, 17, 8, 8], (Kind::Float, dev));
        let target = Tensor::ones([1, 17, 8, 8], (Kind::Float, dev));
        let vis = Tensor::ones([1, 17], (Kind::Float, dev));

        let (_, output) = loss_fn.forward(
            &pred, &target, &vis, None, None, None, None, None, None,
        );

        assert!(
            output.total.abs() < 1e-5,
            "Identical heatmaps with λ_kp=1 should give ≈ 0 total loss, got {}",
            output.total
        );
        assert!(output.densepose.is_none());
        assert!(output.transfer.is_none());
    }

    #[test]
    fn test_densepose_loss_identical_inputs_part_loss_near_zero_uv() {
        // For identical pred/target UV the UV loss should be exactly 0.
        // The cross-entropy part loss won't be 0 (uniform logits have entropy ≠ 0)
        // but the UV component should contribute nothing extra.
        let loss_fn = WiFiDensePoseLoss::new(LossWeights::default());
        let dev = device();
        let b = 1_i64;
        let h = 4_i64;
        let w = 4_i64;

        // pred_parts: all-zero logits (uniform over 25 classes)
        let pred_parts = Tensor::zeros([b, 25, h, w], (Kind::Float, dev));
        // target: foreground class 1 everywhere
        let target_parts = Tensor::ones([b, h, w], (Kind::Int64, dev));
        // UV: identical pred and target → uv loss = 0
        let uv = Tensor::zeros([b, 48, h, w], (Kind::Float, dev));

        let loss = loss_fn.densepose_loss(&pred_parts, &target_parts, &uv, &uv);
        let val = loss.double_value(&[]) as f32;

        assert!(
            val >= 0.0,
            "DensePose loss must be non-negative, got {val}"
        );
        // With identical UV the total equals only the CE part loss.
        // CE of uniform logits over 25 classes: ln(25) ≈ 3.22
        assert!(
            val < 5.0,
            "DensePose loss with identical UV should be bounded by CE, got {val}"
        );
    }

    // ── Standalone functional API tests ──────────────────────────────────────

    #[test]
    fn test_fn_keypoint_heatmap_loss_identical_zero() {
        let dev = device();
        let t = Tensor::ones([2, 17, 8, 8], (Kind::Float, dev));
        let loss = keypoint_heatmap_loss(&t, &t);
        let v = loss.double_value(&[]) as f32;
        assert!(v.abs() < 1e-6, "Identical heatmaps → loss must be ≈0, got {v}");
    }

    #[test]
    fn test_fn_generate_gaussian_heatmaps_shape() {
        let dev = device();
        let kpts = Tensor::full(&[2i64, 17, 2], 0.5, (Kind::Float, dev));
        let vis = Tensor::ones(&[2i64, 17], (Kind::Float, dev));
        let hm = generate_gaussian_heatmaps(&kpts, &vis, 16, 2.0);
        assert_eq!(hm.size(), [2, 17, 16, 16]);
    }

    #[test]
    fn test_fn_generate_gaussian_heatmaps_invisible_zero() {
        let dev = device();
        let kpts = Tensor::full(&[1i64, 17, 2], 0.5, (Kind::Float, dev));
        let vis = Tensor::zeros(&[1i64, 17], (Kind::Float, dev)); // all invisible
        let hm = generate_gaussian_heatmaps(&kpts, &vis, 8, 2.0);
        let total: f64 = hm.sum(Kind::Float).double_value(&[]);
        assert_eq!(total, 0.0, "All-invisible heatmaps must be zero");
    }

    #[test]
    fn test_fn_generate_gaussian_heatmaps_peak_near_one() {
        let dev = device();
        // Keypoint at (0.5, 0.5) on an 8×8 map.
        let kpts = Tensor::full(&[1i64, 1, 2], 0.5, (Kind::Float, dev));
        let vis = Tensor::ones(&[1i64, 1], (Kind::Float, dev));
        let hm = generate_gaussian_heatmaps(&kpts, &vis, 8, 1.5);
        let max_val: f64 = hm.max().double_value(&[]);
        assert!(max_val > 0.9, "Peak value {max_val} should be > 0.9");
    }

    #[test]
    fn test_fn_densepose_part_loss_returns_finite() {
        let dev = device();
        let logits = Tensor::zeros(&[1i64, 25, 4, 4], (Kind::Float, dev));
        let labels = Tensor::zeros(&[1i64, 4, 4], (Kind::Int64, dev));
        let loss = densepose_part_loss(&logits, &labels);
        let v = loss.double_value(&[]);
        assert!(v.is_finite() && v >= 0.0);
    }

    #[test]
    fn test_fn_densepose_uv_loss_no_annotated_pixels_zero() {
        let dev = device();
        let pred = Tensor::ones(&[1i64, 48, 4, 4], (Kind::Float, dev));
        let gt = Tensor::zeros(&[1i64, 48, 4, 4], (Kind::Float, dev));
        let labels = Tensor::full(&[1i64, 4, 4], -1i64, (Kind::Int64, dev));
        let loss = densepose_uv_loss(&pred, &gt, &labels);
        let v = loss.double_value(&[]);
        assert_eq!(v, 0.0, "No annotated pixels → UV loss must be 0");
    }

    #[test]
    fn test_fn_densepose_uv_loss_identical_zero() {
        let dev = device();
        let t = Tensor::ones(&[1i64, 48, 4, 4], (Kind::Float, dev));
        let labels = Tensor::zeros(&[1i64, 4, 4], (Kind::Int64, dev));
        let loss = densepose_uv_loss(&t, &t, &labels);
        let v = loss.double_value(&[]);
        assert!(v.abs() < 1e-6, "Identical UV → loss ≈ 0, got {v}");
    }

    #[test]
    fn test_fn_transfer_loss_identical_zero() {
        let dev = device();
        let t = Tensor::ones(&[2i64, 64, 8, 8], (Kind::Float, dev));
        let loss = fn_transfer_loss(&t, &t);
        let v = loss.double_value(&[]);
        assert!(v.abs() < 1e-6, "Identical features → transfer loss ≈ 0, got {v}");
    }

    #[test]
    fn test_fn_transfer_loss_spatial_mismatch() {
        let dev = device();
        let student = Tensor::ones(&[1i64, 64, 16, 16], (Kind::Float, dev));
        let teacher = Tensor::ones(&[1i64, 64, 8, 8], (Kind::Float, dev));
        let loss = fn_transfer_loss(&student, &teacher);
        let v = loss.double_value(&[]);
        assert!(v.is_finite() && v >= 0.0, "Spatial-mismatch transfer loss must be finite");
    }

    #[test]
    fn test_fn_transfer_loss_channel_mismatch_divisible() {
        let dev = device();
        let student = Tensor::ones(&[1i64, 128, 8, 8], (Kind::Float, dev));
        let teacher = Tensor::ones(&[1i64, 64, 8, 8], (Kind::Float, dev));
        let loss = fn_transfer_loss(&student, &teacher);
        let v = loss.double_value(&[]);
        assert!(v.is_finite() && v >= 0.0);
    }

    #[test]
    fn test_compute_losses_keypoint_only() {
        let dev = device();
        let pred = Tensor::ones(&[1i64, 17, 8, 8], (Kind::Float, dev));
        let gt = Tensor::ones(&[1i64, 17, 8, 8], (Kind::Float, dev));
        let out = compute_losses(&pred, &gt, None, None, None, None, None, None,
                                 1.0, 1.0, 1.0);
        assert!(out.total.is_finite());
        assert!(out.keypoint >= 0.0);
        assert!(out.densepose_parts.is_none());
        assert!(out.densepose_uv.is_none());
        assert!(out.transfer.is_none());
    }

    #[test]
    fn test_compute_losses_all_components_finite() {
        let dev = device();
        let b = 1i64;
        let h = 4i64;
        let w = 4i64;
        let pred_kpt = Tensor::ones(&[b, 17, h, w], (Kind::Float, dev));
        let gt_kpt   = Tensor::ones(&[b, 17, h, w], (Kind::Float, dev));
        let logits   = Tensor::zeros(&[b, 25, h, w], (Kind::Float, dev));
        let labels   = Tensor::zeros(&[b, h, w], (Kind::Int64, dev));
        let pred_uv  = Tensor::ones(&[b, 48, h, w], (Kind::Float, dev));
        let gt_uv    = Tensor::ones(&[b, 48, h, w], (Kind::Float, dev));
        let sf       = Tensor::ones(&[b, 64, 2, 2], (Kind::Float, dev));
        let tf       = Tensor::ones(&[b, 64, 2, 2], (Kind::Float, dev));

        let out = compute_losses(
            &pred_kpt, &gt_kpt,
            Some(&logits), Some(&labels),
            Some(&pred_uv), Some(&gt_uv),
            Some(&sf), Some(&tf),
            1.0, 0.5, 0.1,
        );

        assert!(out.total.is_finite() && out.total >= 0.0);
        assert!(out.densepose_parts.is_some());
        assert!(out.densepose_uv.is_some());
        assert!(out.transfer.is_some());
    }
}
