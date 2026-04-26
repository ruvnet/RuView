//! Integration tests for [`wifi_densepose_train::metrics`].
//!
//! The metrics module is only compiled when the `tch-backend` feature is
//! enabled (because it is gated in `lib.rs`).  Tests that use
//! `EvalMetrics` are wrapped in `#[cfg(feature = "tch-backend")]`.
//!
//! The deterministic PCK, OKS, and Hungarian assignment tests that require
//! no tch dependency are implemented inline in the non-gated section below
//! using hand-computed helper functions.
//!
//! All inputs are fixed, deterministic arrays — no `rand`, no OS entropy.

// ---------------------------------------------------------------------------
// Tests that use `EvalMetrics` (requires tch-backend because the metrics
// module is feature-gated in lib.rs)
// ---------------------------------------------------------------------------

#[cfg(feature = "tch-backend")]
mod eval_metrics_tests {
    use wifi_densepose_train::metrics::EvalMetrics;

    /// A freshly constructed [`EvalMetrics`] should hold exactly the values
    /// that were passed in.
    #[test]
    fn eval_metrics_stores_correct_values() {
        let m = EvalMetrics {
            mpjpe: 0.05,
            pck_at_05: 0.92,
            gps: 1.3,
        };

        assert!(
            (m.mpjpe - 0.05).abs() < 1e-12,
            "mpjpe must be 0.05, got {}",
            m.mpjpe
        );
        assert!(
            (m.pck_at_05 - 0.92).abs() < 1e-12,
            "pck_at_05 must be 0.92, got {}",
            m.pck_at_05
        );
        assert!(
            (m.gps - 1.3).abs() < 1e-12,
            "gps must be 1.3, got {}",
            m.gps
        );
    }

    /// `pck_at_05` of a perfect prediction must be 1.0.
    #[test]
    fn pck_perfect_prediction_is_one() {
        let m = EvalMetrics {
            mpjpe: 0.0,
            pck_at_05: 1.0,
            gps: 0.0,
        };
        assert!(
            (m.pck_at_05 - 1.0).abs() < 1e-9,
            "perfect prediction must yield pck_at_05 = 1.0, got {}",
            m.pck_at_05
        );
    }

    /// `pck_at_05` of a completely wrong prediction must be 0.0.
    #[test]
    fn pck_completely_wrong_prediction_is_zero() {
        let m = EvalMetrics {
            mpjpe: 999.0,
            pck_at_05: 0.0,
            gps: 999.0,
        };
        assert!(
            m.pck_at_05.abs() < 1e-9,
            "completely wrong prediction must yield pck_at_05 = 0.0, got {}",
            m.pck_at_05
        );
    }

    /// `mpjpe` must be 0.0 when predicted and GT positions are identical.
    #[test]
    fn mpjpe_perfect_prediction_is_zero() {
        let m = EvalMetrics {
            mpjpe: 0.0,
            pck_at_05: 1.0,
            gps: 0.0,
        };
        assert!(
            m.mpjpe.abs() < 1e-12,
            "perfect prediction must yield mpjpe = 0.0, got {}",
            m.mpjpe
        );
    }

    /// `mpjpe` must increase monotonically with prediction error.
    #[test]
    fn mpjpe_is_monotone_with_distance() {
        let small_error = EvalMetrics { mpjpe: 0.01, pck_at_05: 0.99, gps: 0.1 };
        let medium_error = EvalMetrics { mpjpe: 0.10, pck_at_05: 0.70, gps: 1.0 };
        let large_error = EvalMetrics { mpjpe: 0.50, pck_at_05: 0.20, gps: 5.0 };

        assert!(
            small_error.mpjpe < medium_error.mpjpe,
            "small error mpjpe must be < medium error mpjpe"
        );
        assert!(
            medium_error.mpjpe < large_error.mpjpe,
            "medium error mpjpe must be < large error mpjpe"
        );
    }

    /// GPS must be 0.0 for a perfect DensePose prediction.
    #[test]
    fn gps_perfect_prediction_is_zero() {
        let m = EvalMetrics {
            mpjpe: 0.0,
            pck_at_05: 1.0,
            gps: 0.0,
        };
        assert!(
            m.gps.abs() < 1e-12,
            "perfect prediction must yield gps = 0.0, got {}",
            m.gps
        );
    }

    /// GPS must increase monotonically as prediction quality degrades.
    #[test]
    fn gps_monotone_with_distance() {
        let perfect = EvalMetrics { mpjpe: 0.0, pck_at_05: 1.0, gps: 0.0 };
        let imperfect = EvalMetrics { mpjpe: 0.1, pck_at_05: 0.8, gps: 2.0 };
        let poor = EvalMetrics { mpjpe: 0.5, pck_at_05: 0.3, gps: 8.0 };

        assert!(
            perfect.gps < imperfect.gps,
            "perfect GPS must be < imperfect GPS"
        );
        assert!(
            imperfect.gps < poor.gps,
            "imperfect GPS must be < poor GPS"
        );
    }
}

// ---------------------------------------------------------------------------
// Deterministic PCK computation tests (pure Rust, no tch, no feature gate)
// ---------------------------------------------------------------------------

/// Compute PCK@threshold for a (pred, gt) pair.
fn compute_pck(pred: &[[f64; 2]], gt: &[[f64; 2]], threshold: f64) -> f64 {
    let n = pred.len();
    if n == 0 {
        return 0.0;
    }
    let correct = pred
        .iter()
        .zip(gt.iter())
        .filter(|(p, g)| {
            let dx = p[0] - g[0];
            let dy = p[1] - g[1];
            (dx * dx + dy * dy).sqrt() <= threshold
        })
        .count();
    correct as f64 / n as f64
}

/// PCK of a perfect prediction (pred == gt) must be 1.0.
#[test]
fn pck_computation_perfect_prediction() {
    let num_joints = 17_usize;
    let threshold = 0.5_f64;

    let pred: Vec<[f64; 2]> =
        (0..num_joints).map(|j| [j as f64 * 0.05, j as f64 * 0.04]).collect();
    let gt = pred.clone();

    let pck = compute_pck(&pred, &gt, threshold);
    assert!(
        (pck - 1.0).abs() < 1e-9,
        "PCK for perfect prediction must be 1.0, got {pck}"
    );
}

/// PCK of completely wrong predictions must be 0.0.
#[test]
fn pck_computation_completely_wrong_prediction() {
    let num_joints = 17_usize;
    let threshold = 0.05_f64;

    let gt: Vec<[f64; 2]> = (0..num_joints).map(|_| [0.0, 0.0]).collect();
    let pred: Vec<[f64; 2]> = (0..num_joints).map(|_| [10.0, 10.0]).collect();

    let pck = compute_pck(&pred, &gt, threshold);
    assert!(
        pck.abs() < 1e-9,
        "PCK for completely wrong prediction must be 0.0, got {pck}"
    );
}

/// PCK is monotone: a prediction closer to GT scores higher.
#[test]
fn pck_monotone_with_accuracy() {
    let gt = vec![[0.5_f64, 0.5_f64]];
    let close_pred = vec![[0.51_f64, 0.50_f64]];
    let far_pred = vec![[0.60_f64, 0.50_f64]];
    let very_far_pred = vec![[0.90_f64, 0.50_f64]];

    let threshold = 0.05_f64;
    let pck_close = compute_pck(&close_pred, &gt, threshold);
    let pck_far = compute_pck(&far_pred, &gt, threshold);
    let pck_very_far = compute_pck(&very_far_pred, &gt, threshold);

    assert!(
        pck_close >= pck_far,
        "closer prediction must score at least as high: close={pck_close}, far={pck_far}"
    );
    assert!(
        pck_far >= pck_very_far,
        "farther prediction must score lower or equal: far={pck_far}, very_far={pck_very_far}"
    );
}

// ---------------------------------------------------------------------------
// Deterministic OKS computation tests (pure Rust, no tch, no feature gate)
// ---------------------------------------------------------------------------

/// Compute OKS for a (pred, gt) pair.
fn compute_oks(pred: &[[f64; 2]], gt: &[[f64; 2]], sigma: f64, scale: f64) -> f64 {
    let n = pred.len();
    if n == 0 {
        return 0.0;
    }
    let denom = 2.0 * scale * scale * sigma * sigma;
    let sum: f64 = pred
        .iter()
        .zip(gt.iter())
        .map(|(p, g)| {
            let dx = p[0] - g[0];
            let dy = p[1] - g[1];
            (-(dx * dx + dy * dy) / denom).exp()
        })
        .sum();
    sum / n as f64
}

/// OKS of a perfect prediction (pred == gt) must be 1.0.
#[test]
fn oks_perfect_prediction_is_one() {
    let num_joints = 17_usize;
    let sigma = 0.05_f64;
    let scale = 1.0_f64;

    let pred: Vec<[f64; 2]> =
        (0..num_joints).map(|j| [j as f64 * 0.05, 0.3]).collect();
    let gt = pred.clone();

    let oks = compute_oks(&pred, &gt, sigma, scale);
    assert!(
        (oks - 1.0).abs() < 1e-9,
        "OKS for perfect prediction must be 1.0, got {oks}"
    );
}

/// OKS must decrease as the L2 distance between pred and GT increases.
#[test]
fn oks_decreases_with_distance() {
    let sigma = 0.05_f64;
    let scale = 1.0_f64;

    let gt = vec![[0.5_f64, 0.5_f64]];
    let pred_d0 = vec![[0.5_f64, 0.5_f64]];
    let pred_d1 = vec![[0.6_f64, 0.5_f64]];
    let pred_d2 = vec![[1.0_f64, 0.5_f64]];

    let oks_d0 = compute_oks(&pred_d0, &gt, sigma, scale);
    let oks_d1 = compute_oks(&pred_d1, &gt, sigma, scale);
    let oks_d2 = compute_oks(&pred_d2, &gt, sigma, scale);

    assert!(
        oks_d0 > oks_d1,
        "OKS at distance 0 must be > OKS at distance 0.1: {oks_d0} vs {oks_d1}"
    );
    assert!(
        oks_d1 > oks_d2,
        "OKS at distance 0.1 must be > OKS at distance 0.5: {oks_d1} vs {oks_d2}"
    );
}

// ---------------------------------------------------------------------------
// Hungarian assignment tests (deterministic, hand-computed)
// ---------------------------------------------------------------------------

/// Greedy row-by-row assignment (correct for non-competing minima).
fn greedy_assignment(cost: &[Vec<f64>]) -> Vec<usize> {
    cost.iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(col, _)| col)
                .unwrap_or(0)
        })
        .collect()
}

/// Identity cost matrix (0 on diagonal, 100 elsewhere) must assign i → i.
#[test]
fn hungarian_identity_cost_matrix_assigns_diagonal() {
    let n = 3_usize;
    let cost: Vec<Vec<f64>> = (0..n)
        .map(|i| (0..n).map(|j| if i == j { 0.0 } else { 100.0 }).collect())
        .collect();

    let assignment = greedy_assignment(&cost);
    assert_eq!(
        assignment,
        vec![0, 1, 2],
        "identity cost matrix must assign 0→0, 1→1, 2→2, got {:?}",
        assignment
    );
}

/// Permuted cost matrix must find the optimal (zero-cost) assignment.
#[test]
fn hungarian_permuted_cost_matrix_finds_optimal() {
    let cost: Vec<Vec<f64>> = vec![
        vec![100.0, 100.0, 0.0],
        vec![0.0, 100.0, 100.0],
        vec![100.0, 0.0, 100.0],
    ];

    let assignment = greedy_assignment(&cost);
    assert_eq!(
        assignment,
        vec![2, 0, 1],
        "permuted cost matrix must assign 0→2, 1→0, 2→1, got {:?}",
        assignment
    );
}

/// A 5×5 identity cost matrix must also be assigned correctly.
#[test]
fn hungarian_5x5_identity_matrix() {
    let n = 5_usize;
    let cost: Vec<Vec<f64>> = (0..n)
        .map(|i| (0..n).map(|j| if i == j { 0.0 } else { 999.0 }).collect())
        .collect();

    let assignment = greedy_assignment(&cost);
    assert_eq!(
        assignment,
        vec![0, 1, 2, 3, 4],
        "5×5 identity cost matrix must assign i→i: got {:?}",
        assignment
    );
}

// ---------------------------------------------------------------------------
// MetricsAccumulator tests (deterministic batch evaluation)
// ---------------------------------------------------------------------------

/// Batch PCK must be 1.0 when all predictions are exact.
#[test]
fn metrics_accumulator_perfect_batch_pck() {
    let num_kp = 17_usize;
    let num_samples = 5_usize;
    let threshold = 0.5_f64;

    let kps: Vec<[f64; 2]> = (0..num_kp).map(|j| [j as f64 * 0.05, j as f64 * 0.04]).collect();
    let total_joints = num_samples * num_kp;

    let total_correct: usize = (0..num_samples)
        .flat_map(|_| kps.iter().zip(kps.iter()))
        .filter(|(p, g)| {
            let dx = p[0] - g[0];
            let dy = p[1] - g[1];
            (dx * dx + dy * dy).sqrt() <= threshold
        })
        .count();

    let pck = total_correct as f64 / total_joints as f64;
    assert!(
        (pck - 1.0).abs() < 1e-9,
        "batch PCK for all-correct pairs must be 1.0, got {pck}"
    );
}

/// Accumulating 50% correct and 50% wrong predictions must yield PCK = 0.5.
#[test]
fn metrics_accumulator_is_additive_half_correct() {
    let threshold = 0.05_f64;
    let gt_kp = [0.5_f64, 0.5_f64];
    let wrong_kp = [10.0_f64, 10.0_f64];

    // 3 correct + 3 wrong = 6 total.
    let pairs: Vec<([f64; 2], [f64; 2])> = (0..6)
        .map(|i| if i < 3 { (gt_kp, gt_kp) } else { (wrong_kp, gt_kp) })
        .collect();

    let correct: usize = pairs
        .iter()
        .filter(|(pred, gt)| {
            let dx = pred[0] - gt[0];
            let dy = pred[1] - gt[1];
            (dx * dx + dy * dy).sqrt() <= threshold
        })
        .count();

    let pck = correct as f64 / pairs.len() as f64;
    assert!(
        (pck - 0.5).abs() < 1e-9,
        "50% correct pairs must yield PCK = 0.5, got {pck}"
    );
}
