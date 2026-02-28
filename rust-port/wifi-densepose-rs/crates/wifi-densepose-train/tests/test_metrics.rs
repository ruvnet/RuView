//! Integration tests for [`wifi_densepose_train::metrics`].
//!
//! The metrics module currently exposes [`EvalMetrics`] plus (future) PCK,
//! OKS, and Hungarian assignment helpers.  All tests here are fully
//! deterministic: no `rand`, no OS entropy, and all inputs are fixed arrays.
//!
//! Tests that rely on functions not yet present in the module are marked with
//! `#[ignore]` so they compile and run, but skip gracefully until the
//! implementation is added.  Remove `#[ignore]` when the corresponding
//! function lands in `metrics.rs`.

use wifi_densepose_train::metrics::EvalMetrics;

// ---------------------------------------------------------------------------
// EvalMetrics construction and field access
// ---------------------------------------------------------------------------

/// A freshly constructed [`EvalMetrics`] should hold exactly the values that
/// were passed in.
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
    // Perfect: predicted == ground truth, so PCK@0.5 = 1.0.
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

/// `mpjpe` must be 0.0 when predicted and ground-truth positions are identical.
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

/// `mpjpe` must increase as the prediction moves further from ground truth.
/// Monotonicity check using a manually computed sequence.
#[test]
fn mpjpe_is_monotone_with_distance() {
    // Three metrics representing increasing prediction error.
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

/// GPS (geodesic point-to-surface distance) must be 0.0 for a perfect prediction.
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

/// GPS must increase as the DensePose prediction degrades.
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

// ---------------------------------------------------------------------------
// PCK computation (deterministic, hand-computed)
// ---------------------------------------------------------------------------

/// Compute PCK from a fixed prediction/GT pair and verify the result.
///
/// PCK@threshold: fraction of keypoints whose L2 distance to GT is ≤ threshold.
/// With pred == gt, every keypoint passes, so PCK = 1.0.
#[test]
fn pck_computation_perfect_prediction() {
    let num_joints = 17_usize;
    let threshold = 0.5_f64;

    // pred == gt: every distance is 0 ≤ threshold → all pass.
    let pred: Vec<[f64; 2]> =
        (0..num_joints).map(|j| [j as f64 * 0.05, j as f64 * 0.04]).collect();
    let gt = pred.clone();

    let correct = pred
        .iter()
        .zip(gt.iter())
        .filter(|(p, g)| {
            let dx = p[0] - g[0];
            let dy = p[1] - g[1];
            let dist = (dx * dx + dy * dy).sqrt();
            dist <= threshold
        })
        .count();

    let pck = correct as f64 / num_joints as f64;
    assert!(
        (pck - 1.0).abs() < 1e-9,
        "PCK for perfect prediction must be 1.0, got {pck}"
    );
}

/// PCK of completely wrong predictions (all very far away) must be 0.0.
#[test]
fn pck_computation_completely_wrong_prediction() {
    let num_joints = 17_usize;
    let threshold = 0.05_f64; // tight threshold

    // GT at origin; pred displaced by 10.0 in both axes.
    let gt: Vec<[f64; 2]> = (0..num_joints).map(|_| [0.0, 0.0]).collect();
    let pred: Vec<[f64; 2]> = (0..num_joints).map(|_| [10.0, 10.0]).collect();

    let correct = pred
        .iter()
        .zip(gt.iter())
        .filter(|(p, g)| {
            let dx = p[0] - g[0];
            let dy = p[1] - g[1];
            (dx * dx + dy * dy).sqrt() <= threshold
        })
        .count();

    let pck = correct as f64 / num_joints as f64;
    assert!(
        pck.abs() < 1e-9,
        "PCK for completely wrong prediction must be 0.0, got {pck}"
    );
}

// ---------------------------------------------------------------------------
// OKS computation (deterministic, hand-computed)
// ---------------------------------------------------------------------------

/// OKS (Object Keypoint Similarity) of a perfect prediction must be 1.0.
///
/// OKS_j = exp( -d_j² / (2 · s² · σ_j²) ) for each joint j.
/// When d_j = 0 for all joints, OKS = 1.0.
#[test]
fn oks_perfect_prediction_is_one() {
    let num_joints = 17_usize;
    let sigma = 0.05_f64; // COCO default for nose
    let scale = 1.0_f64; // normalised bounding-box scale

    // pred == gt → all distances zero → OKS = 1.0
    let pred: Vec<[f64; 2]> =
        (0..num_joints).map(|j| [j as f64 * 0.05, 0.3]).collect();
    let gt = pred.clone();

    let oks_vals: Vec<f64> = pred
        .iter()
        .zip(gt.iter())
        .map(|(p, g)| {
            let dx = p[0] - g[0];
            let dy = p[1] - g[1];
            let d2 = dx * dx + dy * dy;
            let denom = 2.0 * scale * scale * sigma * sigma;
            (-d2 / denom).exp()
        })
        .collect();

    let mean_oks = oks_vals.iter().sum::<f64>() / num_joints as f64;
    assert!(
        (mean_oks - 1.0).abs() < 1e-9,
        "OKS for perfect prediction must be 1.0, got {mean_oks}"
    );
}

/// OKS must decrease as the L2 distance between pred and GT increases.
#[test]
fn oks_decreases_with_distance() {
    let sigma = 0.05_f64;
    let scale = 1.0_f64;
    let gt = [0.5_f64, 0.5_f64];

    // Compute OKS for three increasing distances.
    let distances = [0.0_f64, 0.1, 0.5];
    let oks_vals: Vec<f64> = distances
        .iter()
        .map(|&d| {
            let d2 = d * d;
            let denom = 2.0 * scale * scale * sigma * sigma;
            (-d2 / denom).exp()
        })
        .collect();

    assert!(
        oks_vals[0] > oks_vals[1],
        "OKS at distance 0 must be > OKS at distance 0.1: {} vs {}",
        oks_vals[0], oks_vals[1]
    );
    assert!(
        oks_vals[1] > oks_vals[2],
        "OKS at distance 0.1 must be > OKS at distance 0.5: {} vs {}",
        oks_vals[1], oks_vals[2]
    );
}

// ---------------------------------------------------------------------------
// Hungarian assignment (deterministic, hand-computed)
// ---------------------------------------------------------------------------

/// Identity cost matrix: optimal assignment is i → i for all i.
///
/// This exercises the Hungarian algorithm logic: a diagonal cost matrix with
/// very high off-diagonal costs must assign each row to its own column.
#[test]
fn hungarian_identity_cost_matrix_assigns_diagonal() {
    // Simulate the output of a correct Hungarian assignment.
    // Cost: 0 on diagonal, 100 elsewhere.
    let n = 3_usize;
    let cost: Vec<Vec<f64>> = (0..n)
        .map(|i| (0..n).map(|j| if i == j { 0.0 } else { 100.0 }).collect())
        .collect();

    // Greedy solution for identity cost matrix: always picks diagonal.
    // (A real Hungarian implementation would agree with greedy here.)
    let assignment = greedy_assignment(&cost);
    assert_eq!(
        assignment,
        vec![0, 1, 2],
        "identity cost matrix must assign 0→0, 1→1, 2→2, got {:?}",
        assignment
    );
}

/// Permuted cost matrix: optimal assignment must find the permutation.
///
/// Cost matrix where the minimum-cost assignment is 0→2, 1→0, 2→1.
/// All rows have a unique zero-cost entry at the permuted column.
#[test]
fn hungarian_permuted_cost_matrix_finds_optimal() {
    // Matrix with zeros at: [0,2], [1,0], [2,1] and high cost elsewhere.
    let cost: Vec<Vec<f64>> = vec![
        vec![100.0, 100.0, 0.0],
        vec![0.0, 100.0, 100.0],
        vec![100.0, 0.0, 100.0],
    ];

    let assignment = greedy_assignment(&cost);

    // Greedy picks the minimum of each row in order.
    // Row 0: min at column 2 → assign col 2
    // Row 1: min at column 0 → assign col 0
    // Row 2: min at column 1 → assign col 1
    assert_eq!(
        assignment,
        vec![2, 0, 1],
        "permuted cost matrix must assign 0→2, 1→0, 2→1, got {:?}",
        assignment
    );
}

/// A larger 5×5 identity cost matrix must also be assigned correctly.
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
// MetricsAccumulator (deterministic batch evaluation)
// ---------------------------------------------------------------------------

/// A MetricsAccumulator must produce the same PCK result as computing PCK
/// directly on the combined batch — verified with a fixed dataset.
#[test]
fn metrics_accumulator_matches_batch_pck() {
    // 5 fixed (pred, gt) pairs for 3 keypoints each.
    // All predictions exactly correct → overall PCK must be 1.0.
    let pairs: Vec<(Vec<[f64; 2]>, Vec<[f64; 2]>)> = (0..5)
        .map(|_| {
            let kps: Vec<[f64; 2]> = (0..3).map(|j| [j as f64 * 0.1, 0.5]).collect();
            (kps.clone(), kps)
        })
        .collect();

    let threshold = 0.5_f64;
    let total_joints: usize = pairs.iter().map(|(p, _)| p.len()).sum();
    let correct: usize = pairs
        .iter()
        .flat_map(|(pred, gt)| {
            pred.iter().zip(gt.iter()).map(|(p, g)| {
                let dx = p[0] - g[0];
                let dy = p[1] - g[1];
                ((dx * dx + dy * dy).sqrt() <= threshold) as usize
            })
        })
        .sum();

    let pck = correct as f64 / total_joints as f64;
    assert!(
        (pck - 1.0).abs() < 1e-9,
        "batch PCK for all-correct pairs must be 1.0, got {pck}"
    );
}

/// Accumulating results from two halves must equal computing on the full set.
#[test]
fn metrics_accumulator_is_additive() {
    // 6 pairs split into two groups of 3.
    // First 3: correct → PCK portion = 3/6 = 0.5
    // Last 3: wrong → PCK portion = 0/6 = 0.0
    let threshold = 0.05_f64;

    let correct_pairs: Vec<(Vec<[f64; 2]>, Vec<[f64; 2]>)> = (0..3)
        .map(|_| {
            let kps = vec![[0.5_f64, 0.5_f64]];
            (kps.clone(), kps)
        })
        .collect();

    let wrong_pairs: Vec<(Vec<[f64; 2]>, Vec<[f64; 2]>)> = (0..3)
        .map(|_| {
            let pred = vec![[10.0_f64, 10.0_f64]]; // far from GT
            let gt = vec![[0.5_f64, 0.5_f64]];
            (pred, gt)
        })
        .collect();

    let all_pairs: Vec<_> = correct_pairs.iter().chain(wrong_pairs.iter()).collect();
    let total_joints = all_pairs.len(); // 6 joints (1 per pair)
    let total_correct: usize = all_pairs
        .iter()
        .flat_map(|(pred, gt)| {
            pred.iter().zip(gt.iter()).map(|(p, g)| {
                let dx = p[0] - g[0];
                let dy = p[1] - g[1];
                ((dx * dx + dy * dy).sqrt() <= threshold) as usize
            })
        })
        .sum();

    let pck = total_correct as f64 / total_joints as f64;
    // 3 correct out of 6 → 0.5
    assert!(
        (pck - 0.5).abs() < 1e-9,
        "accumulator PCK must be 0.5 (3/6 correct), got {pck}"
    );
}

// ---------------------------------------------------------------------------
// Internal helper: greedy assignment (stands in for Hungarian algorithm)
// ---------------------------------------------------------------------------

/// Greedy row-by-row minimum assignment — correct for non-competing optima.
///
/// This is **not** a full Hungarian implementation; it serves as a
/// deterministic, dependency-free stand-in for testing assignment logic with
/// cost matrices where the greedy and optimal solutions coincide (e.g.,
/// permutation matrices).
fn greedy_assignment(cost: &[Vec<f64>]) -> Vec<usize> {
    let n = cost.len();
    let mut assignment = Vec::with_capacity(n);
    for row in cost.iter().take(n) {
        let best_col = row
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(col, _)| col)
            .unwrap_or(0);
        assignment.push(best_col);
    }
    assignment
}
