//! π-based drift detection for quantization honesty
//!
//! Because π cannot be represented exactly at any finite precision, it is
//! perfect for detecting distortion. If you:
//!
//! 1. Project a signal through a π-based transform
//! 2. Quantize
//! 3. Dequantize
//! 4. Project back
//!
//! Then measure error growth over time, you get a **quantization honesty signal**.
//!
//! If error grows faster than expected:
//! - Precision is too low
//! - Accumulation is biased
//! - Or hardware is misbehaving
//!
//! This pairs beautifully with min-cut stability metrics.

use crate::precision::PrecisionLane;
use std::f32::consts::PI;

/// Expected drift rate per lane (empirically calibrated)
const DRIFT_RATE_3BIT: f32 = 0.15; // High drift expected
const DRIFT_RATE_5BIT: f32 = 0.05; // Moderate drift
const DRIFT_RATE_7BIT: f32 = 0.01; // Low drift
const DRIFT_RATE_FLOAT: f32 = 0.0001; // Minimal drift

/// Drift detector using π transforms
#[derive(Debug, Clone)]
pub struct DriftDetector {
    /// Precision lane being monitored
    lane: PrecisionLane,
    /// Accumulated error
    accumulated_error: f32,
    /// Number of samples processed
    sample_count: usize,
    /// Error history (ring buffer)
    error_history: Vec<f32>,
    /// History index
    history_idx: usize,
    /// Expected drift rate for this lane
    expected_drift_rate: f32,
    /// π reference signal
    pi_reference: f32,
    /// Escalation threshold
    escalation_threshold: f32,
}

impl DriftDetector {
    /// Create a new drift detector for a precision lane
    pub fn new(lane: PrecisionLane) -> Self {
        let expected_drift_rate = match lane {
            PrecisionLane::Bit3 => DRIFT_RATE_3BIT,
            PrecisionLane::Bit5 => DRIFT_RATE_5BIT,
            PrecisionLane::Bit7 => DRIFT_RATE_7BIT,
            PrecisionLane::Float32 => DRIFT_RATE_FLOAT,
        };

        Self {
            lane,
            accumulated_error: 0.0,
            sample_count: 0,
            error_history: vec![0.0; 64], // Rolling window
            history_idx: 0,
            expected_drift_rate,
            pi_reference: PI,
            escalation_threshold: expected_drift_rate * 3.0, // 3x expected = escalate
        }
    }

    /// Check quantization honesty between original and quantized values
    pub fn check(&mut self, original: &[f32], quantized: &[f32]) -> QuantizationHonesty {
        assert_eq!(original.len(), quantized.len());

        // Apply π transform to both
        let pi_original: Vec<f32> = original.iter().map(|&x| self.pi_transform(x)).collect();
        let pi_quantized: Vec<f32> = quantized.iter().map(|&x| self.pi_transform(x)).collect();

        // Compute error after π projection
        let error = self.compute_error(&pi_original, &pi_quantized);
        self.update(error);

        // Check if error is within expected bounds
        let ratio = error / self.expected_drift_rate.max(0.0001);
        let is_honest = ratio < 2.0;
        let should_escalate = ratio > 3.0;

        QuantizationHonesty {
            error,
            expected_error: self.expected_drift_rate,
            ratio,
            is_honest,
            should_escalate,
            sample_count: self.sample_count,
        }
    }

    /// π transform: project value through π-based trigonometric function
    fn pi_transform(&self, value: f32) -> f32 {
        // Use both sin and cos to capture full information
        let angle = value * self.pi_reference;
        angle.sin() + angle.cos() * 0.5
    }

    /// Inverse π transform (approximate)
    fn inverse_pi_transform(&self, transformed: f32) -> f32 {
        // This is lossy by design - the difference measures drift
        let angle = transformed.atan2(1.0);
        angle / self.pi_reference
    }

    /// Compute mean squared error between transformed vectors
    fn compute_error(&self, a: &[f32], b: &[f32]) -> f32 {
        if a.is_empty() {
            return 0.0;
        }

        let mse: f32 = a
            .iter()
            .zip(b.iter())
            .map(|(&x, &y)| (x - y).powi(2))
            .sum::<f32>()
            / a.len() as f32;

        mse.sqrt()
    }

    /// Update drift tracking with new error sample
    pub fn update(&mut self, error: f32) {
        self.accumulated_error += error;
        self.sample_count += 1;

        // Update rolling history
        self.error_history[self.history_idx] = error;
        self.history_idx = (self.history_idx + 1) % self.error_history.len();
    }

    /// Get drift report
    pub fn report(&self) -> DriftReport {
        let mean_error = if self.sample_count > 0 {
            self.accumulated_error / self.sample_count as f32
        } else {
            0.0
        };

        // Compute trend from history
        let trend = self.compute_trend();

        // Check if drift is accelerating
        let is_accelerating = trend > self.expected_drift_rate * 0.1;

        DriftReport {
            mean_error,
            accumulated_error: self.accumulated_error,
            sample_count: self.sample_count,
            trend,
            is_accelerating,
            should_escalate: mean_error > self.escalation_threshold,
            lane: self.lane,
        }
    }

    /// Compute error trend (slope of recent errors)
    fn compute_trend(&self) -> f32 {
        if self.sample_count < 2 {
            return 0.0;
        }

        let n = self.error_history.len().min(self.sample_count);
        if n < 2 {
            return 0.0;
        }

        // Simple linear regression on recent errors
        let mut sum_x = 0.0f32;
        let mut sum_y = 0.0f32;
        let mut sum_xy = 0.0f32;
        let mut sum_xx = 0.0f32;

        for i in 0..n {
            let x = i as f32;
            let y = self.error_history[i];
            sum_x += x;
            sum_y += y;
            sum_xy += x * y;
            sum_xx += x * x;
        }

        let n_f = n as f32;
        let denominator = n_f * sum_xx - sum_x * sum_x;
        if denominator.abs() < 1e-10 {
            return 0.0;
        }

        (n_f * sum_xy - sum_x * sum_y) / denominator
    }

    /// Reset drift tracking
    pub fn reset(&mut self) {
        self.accumulated_error = 0.0;
        self.sample_count = 0;
        self.error_history.fill(0.0);
        self.history_idx = 0;
    }

    /// Run π checksum on a signal (deterministic honesty test)
    pub fn pi_checksum(&self, signal: &[f32]) -> f32 {
        if signal.is_empty() {
            return 0.0;
        }

        // Accumulate through π transform
        let mut checksum = 0.0f32;
        for (i, &val) in signal.iter().enumerate() {
            let pi_phase = (i as f32 + 1.0) * PI / signal.len() as f32;
            checksum += val * pi_phase.sin();
        }

        checksum / signal.len() as f32
    }

    /// Verify π checksum after quantization
    pub fn verify_checksum(&self, original: &[f32], quantized: &[f32]) -> bool {
        let orig_checksum = self.pi_checksum(original);
        let quant_checksum = self.pi_checksum(quantized);

        let error = (orig_checksum - quant_checksum).abs();
        error < self.expected_drift_rate
    }
}

/// Quantization honesty result
#[derive(Debug, Clone, Copy)]
pub struct QuantizationHonesty {
    /// Actual error measured
    pub error: f32,
    /// Expected error for this precision lane
    pub expected_error: f32,
    /// Ratio of actual to expected (>1 = worse than expected)
    pub ratio: f32,
    /// Is the quantization honest (within 2x expected)?
    pub is_honest: bool,
    /// Should we escalate to higher precision?
    pub should_escalate: bool,
    /// Number of samples in this measurement
    pub sample_count: usize,
}

/// Drift report summary
#[derive(Debug, Clone)]
pub struct DriftReport {
    /// Mean error over all samples
    pub mean_error: f32,
    /// Total accumulated error
    pub accumulated_error: f32,
    /// Number of samples processed
    pub sample_count: usize,
    /// Error trend (positive = getting worse)
    pub trend: f32,
    /// Is drift accelerating?
    pub is_accelerating: bool,
    /// Should escalate precision lane?
    pub should_escalate: bool,
    /// Current precision lane
    pub lane: PrecisionLane,
}

impl DriftReport {
    /// Get severity level (0-3)
    pub fn severity(&self) -> u8 {
        if self.should_escalate {
            3
        } else if self.is_accelerating {
            2
        } else if self.mean_error > 0.05 {
            1
        } else {
            0
        }
    }

    /// Suggested next lane
    pub fn suggested_lane(&self) -> Option<PrecisionLane> {
        if self.should_escalate {
            match self.lane {
                PrecisionLane::Bit3 => Some(PrecisionLane::Bit5),
                PrecisionLane::Bit5 => Some(PrecisionLane::Bit7),
                PrecisionLane::Bit7 => Some(PrecisionLane::Float32),
                PrecisionLane::Float32 => None,
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drift_detector_creation() {
        let detector = DriftDetector::new(PrecisionLane::Bit5);
        assert_eq!(detector.sample_count, 0);
    }

    #[test]
    fn test_pi_transform_deterministic() {
        let detector = DriftDetector::new(PrecisionLane::Bit5);
        let v1 = detector.pi_transform(0.5);
        let v2 = detector.pi_transform(0.5);
        assert_eq!(v1, v2);
    }

    #[test]
    fn test_honesty_check_identical() {
        let mut detector = DriftDetector::new(PrecisionLane::Bit7);
        let values = vec![0.1, 0.2, 0.3, 0.4, 0.5];
        let honesty = detector.check(&values, &values);
        assert!(honesty.error < 0.001);
        assert!(honesty.is_honest);
    }

    #[test]
    fn test_honesty_check_with_error() {
        let mut detector = DriftDetector::new(PrecisionLane::Bit3);
        let original = vec![0.1, 0.2, 0.3, 0.4, 0.5];
        let quantized = vec![0.15, 0.25, 0.35, 0.45, 0.55]; // 0.05 error each
        let honesty = detector.check(&original, &quantized);
        assert!(honesty.error > 0.0);
    }

    #[test]
    fn test_drift_report() {
        let mut detector = DriftDetector::new(PrecisionLane::Bit5);
        detector.update(0.01);
        detector.update(0.02);
        detector.update(0.03);

        let report = detector.report();
        assert_eq!(report.sample_count, 3);
        assert!(report.mean_error > 0.0);
    }

    #[test]
    fn test_pi_checksum() {
        let detector = DriftDetector::new(PrecisionLane::Bit5);
        let signal = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let checksum = detector.pi_checksum(&signal);
        assert!(checksum.is_finite());

        // Deterministic
        assert_eq!(detector.pi_checksum(&signal), checksum);
    }

    #[test]
    fn test_verify_checksum() {
        let detector = DriftDetector::new(PrecisionLane::Bit7);
        let original = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let nearly_same = vec![1.001, 2.001, 3.001, 4.001, 5.001];
        assert!(detector.verify_checksum(&original, &nearly_same));
    }

    #[test]
    fn test_severity_levels() {
        let report = DriftReport {
            mean_error: 0.5,
            accumulated_error: 1.0,
            sample_count: 2,
            trend: 0.1,
            is_accelerating: true,
            should_escalate: true,
            lane: PrecisionLane::Bit3,
        };
        assert_eq!(report.severity(), 3);
        assert_eq!(report.suggested_lane(), Some(PrecisionLane::Bit5));
    }
}
