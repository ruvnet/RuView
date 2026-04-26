//! Integration tests for [`wifi_densepose_train::losses`].
//!
//! All tests are gated behind `#[cfg(feature = "tch-backend")]` because the
//! loss functions require PyTorch via `tch`.  When running without that
//! feature the entire module is compiled but skipped at test-registration
//! time.
//!
//! All input tensors are constructed from fixed, deterministic data — no
//! `rand` crate, no OS entropy.

#[cfg(feature = "tch-backend")]
mod tch_tests {
    use wifi_densepose_train::losses::{
        generate_gaussian_heatmap, generate_target_heatmaps, LossWeights, WiFiDensePoseLoss,
    };

    // -----------------------------------------------------------------------
    // Helper: CPU device
    // -----------------------------------------------------------------------

    fn cpu() -> tch::Device {
        tch::Device::Cpu
    }

    // -----------------------------------------------------------------------
    // generate_gaussian_heatmap
    // -----------------------------------------------------------------------

    /// The heatmap must have shape [heatmap_size, heatmap_size].
    #[test]
    fn gaussian_heatmap_has_correct_shape() {
        let hm = generate_gaussian_heatmap(0.5, 0.5, 56, 2.0);
        assert_eq!(
            hm.shape(),
            &[56, 56],
            "heatmap shape must be [56, 56], got {:?}",
            hm.shape()
        );
    }

    /// All values in the heatmap must lie in [0, 1].
    #[test]
    fn gaussian_heatmap_values_in_unit_interval() {
        let hm = generate_gaussian_heatmap(0.3, 0.7, 56, 2.0);
        for &v in hm.iter() {
            assert!(
                v >= 0.0 && v <= 1.0 + 1e-6,
                "heatmap value {v} is outside [0, 1]"
            );
        }
    }

    /// The peak must be at (or very close to) the keypoint pixel location.
    #[test]
    fn gaussian_heatmap_peak_at_keypoint_location() {
        let kp_x = 0.5_f32;
        let kp_y = 0.5_f32;
        let size = 56_usize;
        let sigma = 2.0_f32;

        let hm = generate_gaussian_heatmap(kp_x, kp_y, size, sigma);

        // Map normalised coordinates to pixel space.
        let s = (size - 1) as f32;
        let cx = (kp_x * s).round() as usize;
        let cy = (kp_y * s).round() as usize;

        let peak_val = hm[[cy, cx]];
        assert!(
            peak_val > 0.9,
            "peak value {peak_val} at ({cx},{cy}) must be > 0.9 for σ=2.0"
        );

        // Verify it really is the maximum.
        let global_max = hm.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        assert!(
            (global_max - peak_val).abs() < 1e-4,
            "peak at keypoint location {peak_val} must equal the global max {global_max}"
        );
    }

    /// Values outside the 3σ radius must be zero (clamped).
    #[test]
    fn gaussian_heatmap_zero_outside_3sigma_radius() {
        let size = 56_usize;
        let sigma = 2.0_f32;
        let kp_x = 0.5_f32;
        let kp_y = 0.5_f32;

        let hm = generate_gaussian_heatmap(kp_x, kp_y, size, sigma);

        let s = (size - 1) as f32;
        let cx = kp_x * s;
        let cy = kp_y * s;
        let clip_radius = 3.0 * sigma;

        for r in 0..size {
            for c in 0..size {
                let dx = c as f32 - cx;
                let dy = r as f32 - cy;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist > clip_radius + 0.5 {
                    assert_eq!(
                        hm[[r, c]],
                        0.0,
                        "pixel at ({r},{c}) with dist={dist:.2} from kp must be 0 (outside 3σ)"
                    );
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // generate_target_heatmaps (batch)
    // -----------------------------------------------------------------------

    /// Output shape must be [B, 17, H, W].
    #[test]
    fn target_heatmaps_output_shape() {
        let batch = 4_usize;
        let joints = 17_usize;
        let size = 56_usize;

        let keypoints = ndarray::Array3::from_elem((batch, joints, 2), 0.5_f32);
        let visibility = ndarray::Array2::ones((batch, joints));

        let heatmaps = generate_target_heatmaps(&keypoints, &visibility, size, 2.0);

        assert_eq!(
            heatmaps.shape(),
            &[batch, joints, size, size],
            "target heatmaps shape must be [{batch}, {joints}, {size}, {size}], \
             got {:?}",
            heatmaps.shape()
        );
    }

    /// Invisible keypoints (visibility = 0) must produce all-zero heatmap channels.
    #[test]
    fn target_heatmaps_invisible_joints_are_zero() {
        let batch = 2_usize;
        let joints = 17_usize;
        let size = 32_usize;

        let keypoints = ndarray::Array3::from_elem((batch, joints, 2), 0.5_f32);
        // Make all joints in batch 0 invisible.
        let mut visibility = ndarray::Array2::ones((batch, joints));
        for j in 0..joints {
            visibility[[0, j]] = 0.0;
        }

        let heatmaps = generate_target_heatmaps(&keypoints, &visibility, size, 2.0);

        for j in 0..joints {
            for r in 0..size {
                for c in 0..size {
                    assert_eq!(
                        heatmaps[[0, j, r, c]],
                        0.0,
                        "invisible joint heatmap at [0,{j},{r},{c}] must be zero"
                    );
                }
            }
        }
    }

    /// Visible keypoints must produce non-zero heatmaps.
    #[test]
    fn target_heatmaps_visible_joints_are_nonzero() {
        let batch = 1_usize;
        let joints = 17_usize;
        let size = 56_usize;

        let keypoints = ndarray::Array3::from_elem((batch, joints, 2), 0.5_f32);
        let visibility = ndarray::Array2::ones((batch, joints));

        let heatmaps = generate_target_heatmaps(&keypoints, &visibility, size, 2.0);

        let total_sum: f32 = heatmaps.iter().copied().sum();
        assert!(
            total_sum > 0.0,
            "visible joints must produce non-zero heatmaps, sum={total_sum}"
        );
    }

    // -----------------------------------------------------------------------
    // keypoint_heatmap_loss
    // -----------------------------------------------------------------------

    /// Loss of identical pred and target heatmaps must be ≈ 0.0.
    #[test]
    fn keypoint_heatmap_loss_identical_tensors_is_zero() {
        let loss_fn = WiFiDensePoseLoss::new(LossWeights::default());
        let dev = cpu();

        let pred = tch::Tensor::ones([2, 17, 16, 16], (tch::Kind::Float, dev));
        let target = tch::Tensor::ones([2, 17, 16, 16], (tch::Kind::Float, dev));
        let vis = tch::Tensor::ones([2, 17], (tch::Kind::Float, dev));

        let loss = loss_fn.keypoint_loss(&pred, &target, &vis);
        let val = loss.double_value(&[]) as f32;

        assert!(
            val.abs() < 1e-5,
            "keypoint loss for identical pred/target must be ≈ 0.0, got {val}"
        );
    }

    /// Loss of all-zeros pred vs all-ones target must be > 0.0.
    #[test]
    fn keypoint_heatmap_loss_zero_pred_vs_ones_target_is_positive() {
        let loss_fn = WiFiDensePoseLoss::new(LossWeights::default());
        let dev = cpu();

        let pred = tch::Tensor::zeros([1, 17, 8, 8], (tch::Kind::Float, dev));
        let target = tch::Tensor::ones([1, 17, 8, 8], (tch::Kind::Float, dev));
        let vis = tch::Tensor::ones([1, 17], (tch::Kind::Float, dev));

        let loss = loss_fn.keypoint_loss(&pred, &target, &vis);
        let val = loss.double_value(&[]) as f32;

        assert!(
            val > 0.0,
            "keypoint loss for zero vs ones must be > 0.0, got {val}"
        );
    }

    /// Invisible joints must not contribute to the loss.
    #[test]
    fn keypoint_heatmap_loss_invisible_joints_contribute_nothing() {
        let loss_fn = WiFiDensePoseLoss::new(LossWeights::default());
        let dev = cpu();

        // Large error but all visibility = 0 → loss must be ≈ 0.
        let pred = tch::Tensor::ones([1, 17, 8, 8], (tch::Kind::Float, dev));
        let target = tch::Tensor::zeros([1, 17, 8, 8], (tch::Kind::Float, dev));
        let vis = tch::Tensor::zeros([1, 17], (tch::Kind::Float, dev));

        let loss = loss_fn.keypoint_loss(&pred, &target, &vis);
        let val = loss.double_value(&[]) as f32;

        assert!(
            val.abs() < 1e-5,
            "all-invisible loss must be ≈ 0.0 (no joints contribute), got {val}"
        );
    }

    // -----------------------------------------------------------------------
    // densepose_part_loss
    // -----------------------------------------------------------------------

    /// densepose_loss must return a non-NaN, non-negative value.
    #[test]
    fn densepose_part_loss_no_nan() {
        let loss_fn = WiFiDensePoseLoss::new(LossWeights::default());
        let dev = cpu();

        let b = 1_i64;
        let h = 8_i64;
        let w = 8_i64;

        let pred_parts = tch::Tensor::zeros([b, 25, h, w], (tch::Kind::Float, dev));
        let target_parts = tch::Tensor::ones([b, h, w], (tch::Kind::Int64, dev));
        let uv = tch::Tensor::zeros([b, 48, h, w], (tch::Kind::Float, dev));

        let loss = loss_fn.densepose_loss(&pred_parts, &target_parts, &uv, &uv);
        let val = loss.double_value(&[]) as f32;

        assert!(
            !val.is_nan(),
            "densepose_loss must not produce NaN, got {val}"
        );
        assert!(
            val >= 0.0,
            "densepose_loss must be non-negative, got {val}"
        );
    }

    // -----------------------------------------------------------------------
    // compute_losses (forward)
    // -----------------------------------------------------------------------

    /// The combined forward pass must produce a total loss > 0 for non-trivial
    /// (non-identical) inputs.
    #[test]
    fn compute_losses_total_positive_for_nonzero_error() {
        let loss_fn = WiFiDensePoseLoss::new(LossWeights::default());
        let dev = cpu();

        // pred = zeros, target = ones → non-zero keypoint error.
        let pred_kp = tch::Tensor::zeros([2, 17, 8, 8], (tch::Kind::Float, dev));
        let target_kp = tch::Tensor::ones([2, 17, 8, 8], (tch::Kind::Float, dev));
        let vis = tch::Tensor::ones([2, 17], (tch::Kind::Float, dev));

        let (_, output) = loss_fn.forward(
            &pred_kp, &target_kp, &vis,
            None, None, None, None,
            None, None,
        );

        assert!(
            output.total > 0.0,
            "total loss must be > 0 for non-trivial predictions, got {}",
            output.total
        );
    }

    /// The combined forward pass with identical tensors must produce total ≈ 0.
    #[test]
    fn compute_losses_total_zero_for_perfect_prediction() {
        let weights = LossWeights {
            lambda_kp: 1.0,
            lambda_dp: 0.0,
            lambda_tr: 0.0,
        };
        let loss_fn = WiFiDensePoseLoss::new(weights);
        let dev = cpu();

        let perfect = tch::Tensor::ones([1, 17, 8, 8], (tch::Kind::Float, dev));
        let vis = tch::Tensor::ones([1, 17], (tch::Kind::Float, dev));

        let (_, output) = loss_fn.forward(
            &perfect, &perfect, &vis,
            None, None, None, None,
            None, None,
        );

        assert!(
            output.total.abs() < 1e-5,
            "perfect prediction must yield total ≈ 0.0, got {}",
            output.total
        );
    }

    /// Optional densepose and transfer outputs must be None when not supplied.
    #[test]
    fn compute_losses_optional_components_are_none() {
        let loss_fn = WiFiDensePoseLoss::new(LossWeights::default());
        let dev = cpu();

        let t = tch::Tensor::ones([1, 17, 8, 8], (tch::Kind::Float, dev));
        let vis = tch::Tensor::ones([1, 17], (tch::Kind::Float, dev));

        let (_, output) = loss_fn.forward(
            &t, &t, &vis,
            None, None, None, None,
            None, None,
        );

        assert!(
            output.densepose.is_none(),
            "densepose component must be None when not supplied"
        );
        assert!(
            output.transfer.is_none(),
            "transfer component must be None when not supplied"
        );
    }

    /// Full forward pass with all optional components must populate all fields.
    #[test]
    fn compute_losses_with_all_components_populates_all_fields() {
        let loss_fn = WiFiDensePoseLoss::new(LossWeights::default());
        let dev = cpu();

        let pred_kp = tch::Tensor::zeros([1, 17, 8, 8], (tch::Kind::Float, dev));
        let target_kp = tch::Tensor::ones([1, 17, 8, 8], (tch::Kind::Float, dev));
        let vis = tch::Tensor::ones([1, 17], (tch::Kind::Float, dev));

        let pred_parts = tch::Tensor::zeros([1, 25, 8, 8], (tch::Kind::Float, dev));
        let target_parts = tch::Tensor::ones([1, 8, 8], (tch::Kind::Int64, dev));
        let uv = tch::Tensor::zeros([1, 48, 8, 8], (tch::Kind::Float, dev));

        let student = tch::Tensor::zeros([1, 64, 4, 4], (tch::Kind::Float, dev));
        let teacher = tch::Tensor::ones([1, 64, 4, 4], (tch::Kind::Float, dev));

        let (_, output) = loss_fn.forward(
            &pred_kp, &target_kp, &vis,
            Some(&pred_parts), Some(&target_parts), Some(&uv), Some(&uv),
            Some(&student), Some(&teacher),
        );

        assert!(
            output.densepose.is_some(),
            "densepose component must be Some when all inputs provided"
        );
        assert!(
            output.transfer.is_some(),
            "transfer component must be Some when student/teacher provided"
        );
        assert!(
            output.total > 0.0,
            "total loss must be > 0 when pred ≠ target, got {}",
            output.total
        );

        // Neither component may be NaN.
        if let Some(dp) = output.densepose {
            assert!(!dp.is_nan(), "densepose component must not be NaN");
        }
        if let Some(tr) = output.transfer {
            assert!(!tr.is_nan(), "transfer component must not be NaN");
        }
    }

    // -----------------------------------------------------------------------
    // transfer_loss
    // -----------------------------------------------------------------------

    /// Transfer loss for identical tensors must be ≈ 0.0.
    #[test]
    fn transfer_loss_identical_features_is_zero() {
        let loss_fn = WiFiDensePoseLoss::new(LossWeights::default());
        let dev = cpu();

        let feat = tch::Tensor::ones([2, 64, 8, 8], (tch::Kind::Float, dev));
        let loss = loss_fn.transfer_loss(&feat, &feat);
        let val = loss.double_value(&[]) as f32;

        assert!(
            val.abs() < 1e-5,
            "transfer loss for identical tensors must be ≈ 0.0, got {val}"
        );
    }

    /// Transfer loss for different tensors must be > 0.0.
    #[test]
    fn transfer_loss_different_features_is_positive() {
        let loss_fn = WiFiDensePoseLoss::new(LossWeights::default());
        let dev = cpu();

        let student = tch::Tensor::zeros([2, 64, 8, 8], (tch::Kind::Float, dev));
        let teacher = tch::Tensor::ones([2, 64, 8, 8], (tch::Kind::Float, dev));

        let loss = loss_fn.transfer_loss(&student, &teacher);
        let val = loss.double_value(&[]) as f32;

        assert!(
            val > 0.0,
            "transfer loss for different tensors must be > 0.0, got {val}"
        );
    }
}

// When tch-backend is disabled, ensure the file still compiles cleanly.
#[cfg(not(feature = "tch-backend"))]
#[test]
fn tch_backend_not_enabled() {
    // This test passes trivially when the tch-backend feature is absent.
    // The tch_tests module above is fully skipped.
}
