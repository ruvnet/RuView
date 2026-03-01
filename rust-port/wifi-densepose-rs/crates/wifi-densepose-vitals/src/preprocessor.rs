//! CSI vital sign preprocessor.
//!
//! Suppresses static subcarrier components and extracts the
//! body-modulated signal residuals for vital sign analysis.
//!
//! Uses an EMA-based predictive filter (same pattern as
//! [`PredictiveGate`](wifi_densepose_wifiscan::pipeline::PredictiveGate)
//! in the wifiscan crate) operating on per-subcarrier amplitudes.
//! The residuals represent deviations from the static environment
//! baseline, isolating physiological movements (breathing, heartbeat).

use crate::types::CsiFrame;

/// EMA-based preprocessor that extracts body-modulated residuals
/// from raw CSI subcarrier amplitudes.
pub struct CsiVitalPreprocessor {
    /// EMA predictions per subcarrier.
    predictions: Vec<f64>,
    /// Whether each subcarrier slot has been initialised.
    initialized: Vec<bool>,
    /// EMA smoothing factor (lower = slower tracking, better static suppression).
    alpha: f64,
    /// Number of subcarrier slots.
    n_subcarriers: usize,
}

impl CsiVitalPreprocessor {
    /// Create a new preprocessor.
    ///
    /// - `n_subcarriers`: number of subcarrier slots to track.
    /// - `alpha`: EMA smoothing factor in `(0, 1)`. Lower values
    ///   provide better static component suppression but slower
    ///   adaptation. Default for vital signs: `0.05`.
    #[must_use]
    pub fn new(n_subcarriers: usize, alpha: f64) -> Self {
        Self {
            predictions: vec![0.0; n_subcarriers],
            initialized: vec![false; n_subcarriers],
            alpha: alpha.clamp(0.001, 0.999),
            n_subcarriers,
        }
    }

    /// Create a preprocessor with defaults suitable for ESP32 CSI
    /// vital sign extraction (56 subcarriers, alpha = 0.05).
    #[must_use]
    pub fn esp32_default() -> Self {
        Self::new(56, 0.05)
    }

    /// Process a CSI frame and return the residual vector.
    ///
    /// The residuals represent the difference between observed and
    /// predicted (EMA) amplitudes. On the first frame for each
    /// subcarrier, the prediction is seeded and the raw amplitude
    /// is returned.
    ///
    /// Returns `None` if the frame has zero subcarriers.
    pub fn process(&mut self, frame: &CsiFrame) -> Option<Vec<f64>> {
        let n = frame.amplitudes.len().min(self.n_subcarriers);
        if n == 0 {
            return None;
        }

        let mut residuals = vec![0.0; n];

        for (i, residual) in residuals.iter_mut().enumerate().take(n) {
            if self.initialized[i] {
                // Compute residual: observed - predicted
                *residual = frame.amplitudes[i] - self.predictions[i];
                // Update EMA prediction
                self.predictions[i] =
                    self.alpha * frame.amplitudes[i] + (1.0 - self.alpha) * self.predictions[i];
            } else {
                // First observation: seed the prediction
                self.predictions[i] = frame.amplitudes[i];
                self.initialized[i] = true;
                // First-frame residual is zero (no prior to compare against)
                *residual = 0.0;
            }
        }

        Some(residuals)
    }

    /// Reset all predictions and initialisation state.
    pub fn reset(&mut self) {
        self.predictions.fill(0.0);
        self.initialized.fill(false);
    }

    /// Current EMA smoothing factor.
    #[must_use]
    pub fn alpha(&self) -> f64 {
        self.alpha
    }

    /// Update the EMA smoothing factor.
    pub fn set_alpha(&mut self, alpha: f64) {
        self.alpha = alpha.clamp(0.001, 0.999);
    }

    /// Number of subcarrier slots.
    #[must_use]
    pub fn n_subcarriers(&self) -> usize {
        self.n_subcarriers
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CsiFrame;

    fn make_frame(amplitudes: Vec<f64>, n: usize) -> CsiFrame {
        let phases = vec![0.0; n];
        CsiFrame {
            amplitudes,
            phases,
            n_subcarriers: n,
            sample_index: 0,
            sample_rate_hz: 100.0,
        }
    }

    #[test]
    fn empty_frame_returns_none() {
        let mut pp = CsiVitalPreprocessor::new(4, 0.05);
        let frame = make_frame(vec![], 0);
        assert!(pp.process(&frame).is_none());
    }

    #[test]
    fn first_frame_residuals_are_zero() {
        let mut pp = CsiVitalPreprocessor::new(3, 0.05);
        let frame = make_frame(vec![1.0, 2.0, 3.0], 3);
        let residuals = pp.process(&frame).unwrap();
        assert_eq!(residuals.len(), 3);
        for &r in &residuals {
            assert!((r - 0.0).abs() < f64::EPSILON, "first frame residual should be 0");
        }
    }

    #[test]
    fn static_signal_residuals_converge_to_zero() {
        let mut pp = CsiVitalPreprocessor::new(2, 0.1);
        let frame = make_frame(vec![5.0, 10.0], 2);

        // Seed
        pp.process(&frame);

        // After many identical frames, residuals should be near zero
        let mut last_residuals = vec![0.0; 2];
        for _ in 0..100 {
            last_residuals = pp.process(&frame).unwrap();
        }

        for &r in &last_residuals {
            assert!(r.abs() < 0.01, "residuals should converge to ~0 for static signal, got {r}");
        }
    }

    #[test]
    fn step_change_produces_large_residual() {
        let mut pp = CsiVitalPreprocessor::new(1, 0.05);
        let frame1 = make_frame(vec![10.0], 1);

        // Converge EMA
        pp.process(&frame1);
        for _ in 0..200 {
            pp.process(&frame1);
        }

        // Step change
        let frame2 = make_frame(vec![20.0], 1);
        let residuals = pp.process(&frame2).unwrap();
        assert!(residuals[0] > 5.0, "step change should produce large residual, got {}", residuals[0]);
    }

    #[test]
    fn reset_clears_state() {
        let mut pp = CsiVitalPreprocessor::new(2, 0.1);
        let frame = make_frame(vec![1.0, 2.0], 2);
        pp.process(&frame);
        pp.reset();
        // After reset, next frame is treated as first
        let residuals = pp.process(&frame).unwrap();
        for &r in &residuals {
            assert!((r - 0.0).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn alpha_clamped() {
        let pp = CsiVitalPreprocessor::new(1, -5.0);
        assert!(pp.alpha() > 0.0);
        let pp = CsiVitalPreprocessor::new(1, 100.0);
        assert!(pp.alpha() < 1.0);
    }

    #[test]
    fn esp32_default_has_correct_subcarriers() {
        let pp = CsiVitalPreprocessor::esp32_default();
        assert_eq!(pp.n_subcarriers(), 56);
    }
}
