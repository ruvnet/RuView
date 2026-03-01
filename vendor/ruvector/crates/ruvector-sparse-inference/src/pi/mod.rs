//! π (Pi) Integration Module - Structural Constants for Low-Precision Systems
//!
//! π is irrational, non-repeating, and structure-rich. This makes it an ideal
//! reference signal in systems where precision is constrained.
//!
//! # Why π Matters
//!
//! In 3/5/7-bit math, you deliberately throw away bits. π lets you check whether
//! the system is still behaving honestly.
//!
//! # Module Components
//!
//! - **Calibration**: π-derived constants for normalization and phase encoding
//! - **Drift Detection**: Quantization honesty signals using π transforms
//! - **Angular Embeddings**: Hyperspherical embeddings with π phase encoding
//! - **Chaos Seeding**: Deterministic pseudo-randomness from π digits
//!
//! # Key Insight
//!
//! π is not about geometry here. It is about injecting infinite structure into
//! finite machines without breaking determinism.
//!
//! This pairs with:
//! - Min-cut as coherence
//! - Vectors as motion
//! - Agents as reflexes
//! - Precision as policy

pub mod angular;
pub mod chaos;
pub mod constants;
pub mod drift;

pub use angular::{AngularEmbedding, HypersphericalProjection, PhaseEncoder};
pub use chaos::{DeterministicJitter, PiChaos, PiScheduler};
pub use constants::{PiCalibration, PI_SCALE_3BIT, PI_SCALE_5BIT, PI_SCALE_7BIT};
pub use drift::{DriftDetector, DriftReport, QuantizationHonesty};

use crate::precision::PrecisionLane;

/// π-aware quantization context that tracks honesty metrics
#[derive(Debug, Clone)]
pub struct PiContext {
    /// Calibration constants
    pub calibration: PiCalibration,
    /// Drift detector for quantization honesty
    pub drift: DriftDetector,
    /// Angular embedding projector
    pub angular: AngularEmbedding,
    /// Chaos seeder for deterministic jitter
    pub chaos: PiChaos,
    /// Current precision lane
    pub lane: PrecisionLane,
}

impl PiContext {
    /// Create a new π context for a precision lane
    pub fn new(lane: PrecisionLane) -> Self {
        Self {
            calibration: PiCalibration::for_lane(lane),
            drift: DriftDetector::new(lane),
            angular: AngularEmbedding::new(lane),
            chaos: PiChaos::new(),
            lane,
        }
    }

    /// Calibrate a value using π-derived constants
    pub fn calibrate(&self, value: f32) -> f32 {
        self.calibration.normalize(value)
    }

    /// Check quantization honesty
    pub fn check_honesty(&mut self, original: &[f32], quantized: &[f32]) -> QuantizationHonesty {
        self.drift.check(original, quantized)
    }

    /// Project to angular space
    pub fn to_angular(&self, values: &[f32]) -> Vec<f32> {
        self.angular.project(values)
    }

    /// Get deterministic jitter for tie-breaking
    pub fn jitter(&self, index: usize) -> f32 {
        self.chaos.jitter(index)
    }

    /// Update drift tracking
    pub fn update_drift(&mut self, error: f32) {
        self.drift.update(error);
    }

    /// Get drift report
    pub fn drift_report(&self) -> DriftReport {
        self.drift.report()
    }

    /// Should escalate precision lane?
    pub fn should_escalate(&self) -> bool {
        self.drift.report().should_escalate
    }
}

impl Default for PiContext {
    fn default() -> Self {
        Self::new(PrecisionLane::Bit5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pi_context_creation() {
        let ctx = PiContext::new(PrecisionLane::Bit3);
        assert_eq!(ctx.lane, PrecisionLane::Bit3);
    }

    #[test]
    fn test_pi_context_calibration() {
        let ctx = PiContext::new(PrecisionLane::Bit5);
        let calibrated = ctx.calibrate(1.0);
        assert!(calibrated.is_finite());
    }

    #[test]
    fn test_pi_context_angular_projection() {
        let ctx = PiContext::new(PrecisionLane::Bit7);
        let values = vec![1.0, 2.0, 3.0, 4.0];
        let angular = ctx.to_angular(&values);
        assert_eq!(angular.len(), values.len());
    }

    #[test]
    fn test_pi_context_jitter() {
        let ctx = PiContext::new(PrecisionLane::Bit5);
        let j1 = ctx.jitter(0);
        let j2 = ctx.jitter(1);
        // Deterministic: same index = same jitter
        assert_eq!(ctx.jitter(0), j1);
        // Different indices = different jitter
        assert_ne!(j1, j2);
    }
}
