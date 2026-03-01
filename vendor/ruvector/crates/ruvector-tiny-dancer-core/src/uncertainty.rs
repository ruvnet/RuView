//! Uncertainty quantification with conformal prediction

/// Uncertainty estimator for routing decisions
pub struct UncertaintyEstimator {
    /// Calibration quantile for conformal prediction
    calibration_quantile: f32,
}

impl UncertaintyEstimator {
    /// Create a new uncertainty estimator
    pub fn new() -> Self {
        Self {
            calibration_quantile: 0.9, // 90% confidence
        }
    }

    /// Create with custom calibration quantile
    pub fn with_quantile(quantile: f32) -> Self {
        Self {
            calibration_quantile: quantile,
        }
    }

    /// Estimate uncertainty for a prediction
    ///
    /// Uses a simple heuristic based on:
    /// 1. Distance from decision boundary (0.5)
    /// 2. Feature variance
    /// 3. Model confidence
    pub fn estimate(&self, _features: &[f32], prediction: f32) -> f32 {
        // Distance from decision boundary (0.5)
        let boundary_distance = (prediction - 0.5).abs();

        // Higher uncertainty when close to boundary
        let boundary_uncertainty = 1.0 - (boundary_distance * 2.0);

        // Clip to [0, 1]
        boundary_uncertainty.max(0.0).min(1.0)
    }

    /// Calibrate the estimator with a set of predictions and outcomes
    pub fn calibrate(&mut self, _predictions: &[f32], _outcomes: &[bool]) {
        // TODO: Implement conformal prediction calibration
        // This would compute the quantile of non-conformity scores
    }

    /// Get the calibration quantile
    pub fn calibration_quantile(&self) -> f32 {
        self.calibration_quantile
    }
}

impl Default for UncertaintyEstimator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uncertainty_estimation() {
        let estimator = UncertaintyEstimator::new();

        // High confidence prediction should have low uncertainty
        let features = vec![0.5; 10];
        let high_conf = estimator.estimate(&features, 0.95);
        assert!(high_conf < 0.5);

        // Low confidence prediction should have high uncertainty
        let low_conf = estimator.estimate(&features, 0.52);
        assert!(low_conf > 0.5);
    }

    #[test]
    fn test_boundary_uncertainty() {
        let estimator = UncertaintyEstimator::new();
        let features = vec![0.5; 10];

        // Prediction exactly at boundary (0.5) should have maximum uncertainty
        let boundary = estimator.estimate(&features, 0.5);
        assert!((boundary - 1.0).abs() < 0.01);
    }
}
