//! Stage 1: Predictive gating via EMA-based residual filter.
//!
//! Suppresses static BSSIDs by computing residuals between predicted
//! (EMA) and actual RSSI values. Only transmits frames where significant
//! change is detected (body interaction).
//!
//! This is a lightweight pure-Rust implementation. When `ruvector-nervous-system`
//! becomes available, the inner EMA predictor can be replaced with
//! `PredictiveLayer` for more sophisticated prediction.

/// Wrapper around an EMA predictor for multi-BSSID residual gating.
pub struct PredictiveGate {
    /// Per-BSSID EMA predictions.
    predictions: Vec<f32>,
    /// Whether a prediction has been initialised for each slot.
    initialised: Vec<bool>,
    /// EMA smoothing factor (higher = faster tracking).
    alpha: f32,
    /// Residual threshold for change detection.
    threshold: f32,
    /// Residuals from the last frame (for downstream use).
    last_residuals: Vec<f32>,
    /// Number of BSSID slots.
    n_bssids: usize,
}

impl PredictiveGate {
    /// Create a new predictive gate.
    ///
    /// - `n_bssids`: maximum number of tracked BSSIDs (subcarrier slots).
    /// - `threshold`: residual threshold for change detection (ADR-022 default: 0.05).
    #[must_use]
    pub fn new(n_bssids: usize, threshold: f32) -> Self {
        Self {
            predictions: vec![0.0; n_bssids],
            initialised: vec![false; n_bssids],
            alpha: 0.3,
            threshold,
            last_residuals: vec![0.0; n_bssids],
            n_bssids,
        }
    }

    /// Process a frame. Returns `Some(residuals)` if body-correlated change
    /// is detected, `None` if the environment is static.
    pub fn gate(&mut self, amplitudes: &[f32]) -> Option<Vec<f32>> {
        let n = amplitudes.len().min(self.n_bssids);
        let mut residuals = vec![0.0f32; n];
        let mut max_residual = 0.0f32;

        for i in 0..n {
            if self.initialised[i] {
                residuals[i] = amplitudes[i] - self.predictions[i];
                max_residual = max_residual.max(residuals[i].abs());
                // Update EMA
                self.predictions[i] =
                    self.alpha * amplitudes[i] + (1.0 - self.alpha) * self.predictions[i];
            } else {
                // First observation: seed the prediction
                self.predictions[i] = amplitudes[i];
                self.initialised[i] = true;
                residuals[i] = amplitudes[i]; // first frame always transmits
                max_residual = f32::MAX;
            }
        }

        self.last_residuals.clone_from(&residuals);

        if max_residual > self.threshold {
            Some(residuals)
        } else {
            None
        }
    }

    /// Return the residuals from the last `gate()` call.
    #[must_use]
    pub fn last_residuals(&self) -> &[f32] {
        &self.last_residuals
    }

    /// Update the threshold dynamically (e.g., from SONA adaptation).
    pub fn set_threshold(&mut self, threshold: f32) {
        self.threshold = threshold;
    }

    /// Current threshold.
    #[must_use]
    pub fn threshold(&self) -> f32 {
        self.threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_signal_is_gated() {
        let mut gate = PredictiveGate::new(4, 0.05);
        let signal = vec![1.0, 2.0, 3.0, 4.0];
        // First frame always transmits (no prediction yet)
        assert!(gate.gate(&signal).is_some());
        // After many repeated frames, EMA converges and residuals shrink
        for _ in 0..20 {
            gate.gate(&signal);
        }
        assert!(gate.gate(&signal).is_none());
    }

    #[test]
    fn changing_signal_transmits() {
        let mut gate = PredictiveGate::new(4, 0.05);
        let signal1 = vec![1.0, 2.0, 3.0, 4.0];
        gate.gate(&signal1);
        // Let EMA converge
        for _ in 0..20 {
            gate.gate(&signal1);
        }

        // Large change should be transmitted
        let signal2 = vec![1.0, 2.0, 3.0, 10.0];
        assert!(gate.gate(&signal2).is_some());
    }

    #[test]
    fn residuals_are_stored() {
        let mut gate = PredictiveGate::new(3, 0.05);
        let signal = vec![1.0, 2.0, 3.0];
        gate.gate(&signal);
        assert_eq!(gate.last_residuals().len(), 3);
    }

    #[test]
    fn threshold_can_be_updated() {
        let mut gate = PredictiveGate::new(2, 0.05);
        assert!((gate.threshold() - 0.05).abs() < f32::EPSILON);
        gate.set_threshold(0.1);
        assert!((gate.threshold() - 0.1).abs() < f32::EPSILON);
    }
}
