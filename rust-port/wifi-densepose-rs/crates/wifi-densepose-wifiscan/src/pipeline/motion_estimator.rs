//! Stage 4: Multi-AP motion estimation.
//!
//! Combines per-BSSID residuals, attention weights, and correlation
//! features to estimate overall motion intensity and classify
//! motion level (None / Minimal / Moderate / High).

use crate::domain::result::MotionLevel;

/// Multi-AP motion estimator using weighted variance of BSSID residuals.
pub struct MultiApMotionEstimator {
    /// EMA smoothing factor for motion score.
    alpha: f32,
    /// Running EMA of motion score.
    ema_motion: f32,
    /// Motion threshold for None->Minimal transition.
    threshold_minimal: f32,
    /// Motion threshold for Minimal->Moderate transition.
    threshold_moderate: f32,
    /// Motion threshold for Moderate->High transition.
    threshold_high: f32,
}

impl MultiApMotionEstimator {
    /// Create a motion estimator with default thresholds.
    #[must_use]
    pub fn new() -> Self {
        Self {
            alpha: 0.3,
            ema_motion: 0.0,
            threshold_minimal: 0.02,
            threshold_moderate: 0.10,
            threshold_high: 0.30,
        }
    }

    /// Create with custom thresholds.
    #[must_use]
    pub fn with_thresholds(minimal: f32, moderate: f32, high: f32) -> Self {
        Self {
            alpha: 0.3,
            ema_motion: 0.0,
            threshold_minimal: minimal,
            threshold_moderate: moderate,
            threshold_high: high,
        }
    }

    /// Estimate motion from weighted residuals.
    ///
    /// - `residuals`: per-BSSID residual from `PredictiveGate`.
    /// - `weights`: per-BSSID attention weights from `AttentionWeighter`.
    /// - `diversity`: per-BSSID correlation diversity from `BssidCorrelator`.
    ///
    /// Returns `MotionEstimate` with score and level.
    pub fn estimate(
        &mut self,
        residuals: &[f32],
        weights: &[f32],
        diversity: &[f32],
    ) -> MotionEstimate {
        let n = residuals.len();
        if n == 0 {
            return MotionEstimate {
                score: 0.0,
                level: MotionLevel::None,
                weighted_variance: 0.0,
                n_contributing: 0,
            };
        }

        // Weighted variance of residuals (body-sensitive BSSIDs contribute more)
        let mut weighted_sum = 0.0f32;
        let mut weight_total = 0.0f32;
        let mut n_contributing = 0usize;

        #[allow(clippy::cast_precision_loss)]
        for (i, residual) in residuals.iter().enumerate() {
            let w = weights.get(i).copied().unwrap_or(1.0 / n as f32);
            let d = diversity.get(i).copied().unwrap_or(0.5);
            // Combine attention weight with diversity (correlated BSSIDs
            // that respond together are better indicators)
            let combined_w = w * (0.5 + 0.5 * d);
            weighted_sum += combined_w * residual.abs();
            weight_total += combined_w;

            if residual.abs() > 0.001 {
                n_contributing += 1;
            }
        }

        let weighted_variance = if weight_total > 1e-9 {
            weighted_sum / weight_total
        } else {
            0.0
        };

        // EMA smoothing
        self.ema_motion = self.alpha * weighted_variance + (1.0 - self.alpha) * self.ema_motion;

        let level = if self.ema_motion < self.threshold_minimal {
            MotionLevel::None
        } else if self.ema_motion < self.threshold_moderate {
            MotionLevel::Minimal
        } else if self.ema_motion < self.threshold_high {
            MotionLevel::Moderate
        } else {
            MotionLevel::High
        };

        MotionEstimate {
            score: self.ema_motion,
            level,
            weighted_variance,
            n_contributing,
        }
    }

    /// Reset the EMA state.
    pub fn reset(&mut self) {
        self.ema_motion = 0.0;
    }
}

impl Default for MultiApMotionEstimator {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of motion estimation.
#[derive(Debug, Clone)]
pub struct MotionEstimate {
    /// Smoothed motion score (EMA of weighted variance).
    pub score: f32,
    /// Classified motion level.
    pub level: MotionLevel,
    /// Raw weighted variance before smoothing.
    pub weighted_variance: f32,
    /// Number of BSSIDs with non-zero residuals.
    pub n_contributing: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_residuals_yields_no_motion() {
        let mut est = MultiApMotionEstimator::new();
        let result = est.estimate(&[], &[], &[]);
        assert_eq!(result.level, MotionLevel::None);
        assert!((result.score - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn zero_residuals_yield_no_motion() {
        let mut est = MultiApMotionEstimator::new();
        let residuals = vec![0.0, 0.0, 0.0];
        let weights = vec![0.33, 0.33, 0.34];
        let diversity = vec![0.5, 0.5, 0.5];
        let result = est.estimate(&residuals, &weights, &diversity);
        assert_eq!(result.level, MotionLevel::None);
    }

    #[test]
    fn large_residuals_yield_high_motion() {
        let mut est = MultiApMotionEstimator::new();
        let residuals = vec![5.0, 5.0, 5.0];
        let weights = vec![0.33, 0.33, 0.34];
        let diversity = vec![1.0, 1.0, 1.0];
        // Push several frames to overcome EMA smoothing
        for _ in 0..20 {
            est.estimate(&residuals, &weights, &diversity);
        }
        let result = est.estimate(&residuals, &weights, &diversity);
        assert_eq!(result.level, MotionLevel::High);
    }

    #[test]
    fn ema_smooths_transients() {
        let mut est = MultiApMotionEstimator::new();
        let big = vec![10.0, 10.0, 10.0];
        let zero = vec![0.0, 0.0, 0.0];
        let w = vec![0.33, 0.33, 0.34];
        let d = vec![0.5, 0.5, 0.5];

        // One big spike followed by zeros
        est.estimate(&big, &w, &d);
        let r1 = est.estimate(&zero, &w, &d);
        let r2 = est.estimate(&zero, &w, &d);
        // Score should decay
        assert!(r2.score < r1.score, "EMA should decay: {} < {}", r2.score, r1.score);
    }

    #[test]
    fn n_contributing_counts_nonzero() {
        let mut est = MultiApMotionEstimator::new();
        let residuals = vec![0.0, 1.0, 0.0, 2.0];
        let weights = vec![0.25; 4];
        let diversity = vec![0.5; 4];
        let result = est.estimate(&residuals, &weights, &diversity);
        assert_eq!(result.n_contributing, 2);
    }

    #[test]
    fn default_creates_estimator() {
        let est = MultiApMotionEstimator::default();
        assert!((est.threshold_minimal - 0.02).abs() < f32::EPSILON);
    }
}
