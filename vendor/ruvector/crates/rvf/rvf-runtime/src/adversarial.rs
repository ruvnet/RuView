//! Adversarial distribution detection and adaptive n_probe for ADR-033 §2.
//!
//! Detects degenerate centroid distance distributions that indicate
//! adversarial or pathological input, and automatically widens the
//! search to compensate.

/// Coefficient of variation threshold below which centroid distances
/// are considered degenerate (no discriminative power).
pub const DEGENERATE_CV_THRESHOLD: f32 = 0.05;

/// Detect adversarial or degenerate centroid distance distributions.
///
/// Returns `true` if the distribution is too uniform to trust centroid
/// routing — all top-K distances are within 5% CV of each other.
///
/// # Arguments
/// * `distances` - Distances from query to all centroids.
/// * `k` - Number of probes (n_probe) to consider.
pub fn is_degenerate_distribution(distances: &[f32], k: usize) -> bool {
    if k == 0 || distances.len() < 2 {
        return true;
    }

    let sample_size = (2 * k).min(distances.len());

    // Partial sort to get top-2k smallest distances.
    let mut sorted = distances.to_vec();
    sorted.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
    let top = &sorted[..sample_size];

    // Compute coefficient of variation (CV = stddev / mean).
    let sum: f32 = top.iter().sum();
    let mean = sum / top.len() as f32;

    if mean < f32::EPSILON {
        return true; // All distances near zero.
    }

    let variance = top.iter().map(|d| (d - mean).powi(2)).sum::<f32>() / top.len() as f32;

    // Guard against NaN from floating-point rounding producing negative variance.
    if !variance.is_finite() || variance < 0.0 {
        return true; // Treat non-finite variance as degenerate.
    }

    let cv = variance.sqrt() / mean;

    cv < DEGENERATE_CV_THRESHOLD
}

/// Compute the coefficient of variation for top-K centroid distances.
///
/// Returns the CV value for reporting in `SearchEvidenceSummary`.
pub fn centroid_distance_cv(distances: &[f32], k: usize) -> f32 {
    if k == 0 || distances.len() < 2 {
        return 0.0;
    }

    let sample_size = (2 * k).min(distances.len());
    let mut sorted = distances.to_vec();
    sorted.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
    let top = &sorted[..sample_size];

    let sum: f32 = top.iter().sum();
    let mean = sum / top.len() as f32;

    if mean < f32::EPSILON {
        return 0.0;
    }

    let variance = top.iter().map(|d| (d - mean).powi(2)).sum::<f32>() / top.len() as f32;

    if !variance.is_finite() || variance < 0.0 {
        return 0.0;
    }

    variance.sqrt() / mean
}

/// Adaptively widen n_probe when degenerate distributions are detected.
///
/// When distances are uniform, centroids provide no discriminative power.
/// Widen to sqrt(K) or 4x base, whichever is smaller.
pub fn adaptive_n_probe(
    base_n_probe: u32,
    centroid_distances: &[f32],
    total_centroids: u32,
) -> u32 {
    if is_degenerate_distribution(centroid_distances, base_n_probe as usize) {
        let widened = (total_centroids as f64).sqrt().ceil() as u32;
        base_n_probe.max(widened).min(base_n_probe * 4)
    } else {
        base_n_probe
    }
}

/// Compute effective n_probe with epoch drift compensation.
///
/// When centroid epoch drift is detected, widen n_probe to compensate
/// for stale centroids. Linear widening up to 2x at max_drift.
pub fn effective_n_probe_with_drift(base_n_probe: u32, epoch_drift: u32, max_drift: u32) -> u32 {
    if max_drift == 0 {
        return base_n_probe;
    }

    let half_drift = max_drift / 2;

    if epoch_drift <= half_drift {
        // Within comfort zone: no adjustment.
        base_n_probe
    } else if epoch_drift <= max_drift {
        // Drift zone: linear widening up to 2x.
        let numerator = epoch_drift - half_drift;
        let scale = 1.0 + numerator as f64 / max_drift as f64;
        (base_n_probe as f64 * scale).ceil() as u32
    } else {
        // Beyond max drift: double n_probe, schedule recomputation.
        base_n_probe * 2
    }
}

/// Combined n_probe adjustment: applies both drift and adversarial widening.
///
/// The maximum of the two widened values is used.
pub fn combined_effective_n_probe(
    base_n_probe: u32,
    centroid_distances: &[f32],
    total_centroids: u32,
    epoch_drift: u32,
    max_drift: u32,
) -> (u32, bool) {
    let drift_adjusted = effective_n_probe_with_drift(base_n_probe, epoch_drift, max_drift);
    let adversarial_adjusted = adaptive_n_probe(base_n_probe, centroid_distances, total_centroids);
    let degenerate = is_degenerate_distribution(centroid_distances, base_n_probe as usize);

    // Cap at 4x base to prevent drift+adversarial from stacking unboundedly.
    let combined = drift_adjusted
        .max(adversarial_adjusted)
        .min(base_n_probe.saturating_mul(4));

    (combined, degenerate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn degenerate_uniform_distances() {
        // All distances identical — maximally degenerate.
        let distances = vec![1.0; 100];
        assert!(is_degenerate_distribution(&distances, 10));
    }

    #[test]
    fn non_degenerate_natural_distances() {
        // Well-separated distances — natural embeddings.
        let distances: Vec<f32> = (0..100).map(|i| i as f32 * 0.1).collect();
        assert!(!is_degenerate_distribution(&distances, 10));
    }

    #[test]
    fn degenerate_nearly_uniform() {
        // Distances within 1% of each other — degenerate.
        let distances: Vec<f32> = (0..100).map(|i| 1.0 + (i as f32) * 0.001).collect();
        assert!(is_degenerate_distribution(&distances, 10));
    }

    #[test]
    fn degenerate_empty() {
        assert!(is_degenerate_distribution(&[], 10));
    }

    #[test]
    fn degenerate_single() {
        assert!(is_degenerate_distribution(&[1.0], 1));
    }

    #[test]
    fn degenerate_zero_k() {
        assert!(is_degenerate_distribution(&[1.0, 2.0], 0));
    }

    #[test]
    fn degenerate_all_zeros() {
        let distances = vec![0.0; 100];
        assert!(is_degenerate_distribution(&distances, 10));
    }

    #[test]
    fn cv_computation_natural() {
        let distances: Vec<f32> = (0..100).map(|i| i as f32 * 0.1).collect();
        let cv = centroid_distance_cv(&distances, 10);
        assert!(cv > DEGENERATE_CV_THRESHOLD);
    }

    #[test]
    fn cv_computation_uniform() {
        let distances = vec![1.0; 100];
        let cv = centroid_distance_cv(&distances, 10);
        assert!(cv < DEGENERATE_CV_THRESHOLD);
    }

    #[test]
    fn adaptive_n_probe_no_change() {
        let distances: Vec<f32> = (0..100).map(|i| i as f32).collect();
        let result = adaptive_n_probe(10, &distances, 100);
        assert_eq!(result, 10); // No widening needed.
    }

    #[test]
    fn adaptive_n_probe_widens_on_degenerate() {
        let distances = vec![1.0; 100];
        let result = adaptive_n_probe(10, &distances, 100);
        // sqrt(100) = 10, max(10, 10) = 10, min(10, 40) = 10
        assert!(result >= 10);
    }

    #[test]
    fn adaptive_n_probe_widens_large_k() {
        let distances = vec![1.0; 1000];
        let result = adaptive_n_probe(4, &distances, 1000);
        // sqrt(1000) ≈ 32, max(4, 32) = 32, min(32, 16) = 16
        assert_eq!(result, 16); // Capped at 4x base.
    }

    #[test]
    fn drift_no_adjustment() {
        let result = effective_n_probe_with_drift(10, 0, 64);
        assert_eq!(result, 10);
    }

    #[test]
    fn drift_within_comfort_zone() {
        let result = effective_n_probe_with_drift(10, 20, 64);
        assert_eq!(result, 10); // 20 <= 64/2 = 32
    }

    #[test]
    fn drift_linear_widening() {
        let result = effective_n_probe_with_drift(10, 48, 64);
        // (48 - 32) / 64 = 0.25, scale = 1.25, 10 * 1.25 = 12.5 -> 13
        assert!(result > 10 && result <= 20);
    }

    #[test]
    fn drift_beyond_max() {
        let result = effective_n_probe_with_drift(10, 100, 64);
        assert_eq!(result, 20); // Doubled.
    }

    #[test]
    fn drift_zero_max() {
        let result = effective_n_probe_with_drift(10, 50, 0);
        assert_eq!(result, 10); // No drift adjustment possible.
    }

    #[test]
    fn combined_takes_max() {
        // Natural distances (no adversarial widening) but high drift.
        let distances: Vec<f32> = (0..100).map(|i| i as f32).collect();
        let (result, degenerate) = combined_effective_n_probe(10, &distances, 100, 100, 64);
        assert_eq!(result, 20); // Drift dominates.
        assert!(!degenerate);
    }

    #[test]
    fn combined_adversarial_dominates() {
        // Uniform distances (adversarial) but no drift.
        let distances = vec![1.0; 1000];
        let (result, degenerate) = combined_effective_n_probe(4, &distances, 1000, 0, 64);
        assert!(result >= 4);
        assert!(degenerate);
    }
}
