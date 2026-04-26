//! Coherence-Gated Update Policy (ADR-029 Section 2.6)
//!
//! Applies a threshold-based gating rule to the coherence score, producing
//! a `GateDecision` that controls downstream Kalman filter updates:
//!
//! - **Accept** (coherence > 0.85): Full measurement update with nominal noise.
//! - **PredictOnly** (0.5 < coherence < 0.85): Kalman predict step only,
//!   measurement noise inflated 3x.
//! - **Reject** (coherence < 0.5): Discard measurement entirely.
//! - **Recalibrate** (>10s continuous low coherence): Trigger SONA/AETHER
//!   recalibration pipeline.
//!
//! The gate operates on the coherence score produced by the `coherence` module
//! and the stale frame counter from `CoherenceState`.

/// Gate decision controlling Kalman filter update behavior.
#[derive(Debug, Clone, PartialEq)]
pub enum GateDecision {
    /// Coherence is high. Proceed with full Kalman measurement update.
    /// Contains the inflated measurement noise multiplier (1.0 = nominal).
    Accept {
        /// Measurement noise multiplier (1.0 for full accept).
        noise_multiplier: f32,
    },

    /// Coherence is moderate. Run Kalman predict only (no measurement update).
    /// Measurement noise would be inflated 3x if used.
    PredictOnly,

    /// Coherence is low. Reject this measurement entirely.
    Reject,

    /// Prolonged low coherence. Trigger environmental recalibration.
    /// The pipeline should freeze output at last known good pose and
    /// begin the SONA/AETHER TTT adaptation cycle.
    Recalibrate {
        /// Duration of low coherence in frames.
        stale_frames: u64,
    },
}

impl GateDecision {
    /// Returns true if this decision allows a measurement update.
    pub fn allows_update(&self) -> bool {
        matches!(self, GateDecision::Accept { .. })
    }

    /// Returns true if this is a reject or recalibrate decision.
    pub fn is_rejected(&self) -> bool {
        matches!(self, GateDecision::Reject | GateDecision::Recalibrate { .. })
    }

    /// Returns the noise multiplier for accepted decisions, or None otherwise.
    pub fn noise_multiplier(&self) -> Option<f32> {
        match self {
            GateDecision::Accept { noise_multiplier } => Some(*noise_multiplier),
            _ => None,
        }
    }
}

/// Configuration for the gate policy thresholds.
#[derive(Debug, Clone)]
pub struct GatePolicyConfig {
    /// Coherence threshold above which measurements are accepted.
    pub accept_threshold: f32,
    /// Coherence threshold below which measurements are rejected.
    pub reject_threshold: f32,
    /// Maximum stale frames before triggering recalibration.
    pub max_stale_frames: u64,
    /// Noise inflation factor for PredictOnly zone.
    pub predict_only_noise: f32,
    /// Whether to use adaptive thresholds based on drift profile.
    pub adaptive: bool,
}

impl Default for GatePolicyConfig {
    fn default() -> Self {
        Self {
            accept_threshold: 0.85,
            reject_threshold: 0.5,
            max_stale_frames: 200, // 10s at 20Hz
            predict_only_noise: 3.0,
            adaptive: false,
        }
    }
}

/// Gate policy that maps coherence scores to gate decisions.
#[derive(Debug, Clone)]
pub struct GatePolicy {
    /// Accept threshold.
    accept_threshold: f32,
    /// Reject threshold.
    reject_threshold: f32,
    /// Maximum stale frames before recalibration.
    max_stale_frames: u64,
    /// Noise inflation for predict-only zone.
    predict_only_noise: f32,
    /// Running count of consecutive rejected/predict-only frames.
    consecutive_low: u64,
    /// Last decision for tracking transitions.
    last_decision: Option<GateDecision>,
}

impl GatePolicy {
    /// Create a gate policy with the given thresholds.
    pub fn new(accept: f32, reject: f32, max_stale: u64) -> Self {
        Self {
            accept_threshold: accept,
            reject_threshold: reject,
            max_stale_frames: max_stale,
            predict_only_noise: 3.0,
            consecutive_low: 0,
            last_decision: None,
        }
    }

    /// Create a gate policy from a configuration.
    pub fn from_config(config: &GatePolicyConfig) -> Self {
        Self {
            accept_threshold: config.accept_threshold,
            reject_threshold: config.reject_threshold,
            max_stale_frames: config.max_stale_frames,
            predict_only_noise: config.predict_only_noise,
            consecutive_low: 0,
            last_decision: None,
        }
    }

    /// Evaluate the gate decision for a given coherence score and stale count.
    pub fn evaluate(&mut self, coherence_score: f32, stale_count: u64) -> GateDecision {
        let decision = if stale_count >= self.max_stale_frames {
            GateDecision::Recalibrate {
                stale_frames: stale_count,
            }
        } else if coherence_score >= self.accept_threshold {
            self.consecutive_low = 0;
            GateDecision::Accept {
                noise_multiplier: 1.0,
            }
        } else if coherence_score >= self.reject_threshold {
            self.consecutive_low += 1;
            GateDecision::PredictOnly
        } else {
            self.consecutive_low += 1;
            GateDecision::Reject
        };

        self.last_decision = Some(decision.clone());
        decision
    }

    /// Return the last gate decision, if any.
    pub fn last_decision(&self) -> Option<&GateDecision> {
        self.last_decision.as_ref()
    }

    /// Return the current count of consecutive low-coherence frames.
    pub fn consecutive_low_count(&self) -> u64 {
        self.consecutive_low
    }

    /// Return the accept threshold.
    pub fn accept_threshold(&self) -> f32 {
        self.accept_threshold
    }

    /// Return the reject threshold.
    pub fn reject_threshold(&self) -> f32 {
        self.reject_threshold
    }

    /// Reset the policy state (e.g., after recalibration).
    pub fn reset(&mut self) {
        self.consecutive_low = 0;
        self.last_decision = None;
    }
}

impl Default for GatePolicy {
    fn default() -> Self {
        Self::from_config(&GatePolicyConfig::default())
    }
}

/// Compute an adaptive noise multiplier for the PredictOnly zone.
///
/// As coherence drops from accept to reject threshold, the noise
/// multiplier increases from 1.0 to `max_inflation`.
pub fn adaptive_noise_multiplier(
    coherence: f32,
    accept: f32,
    reject: f32,
    max_inflation: f32,
) -> f32 {
    if coherence >= accept {
        return 1.0;
    }
    if coherence <= reject {
        return max_inflation;
    }
    let range = accept - reject;
    if range < 1e-6 {
        return max_inflation;
    }
    let t = (accept - coherence) / range;
    1.0 + t * (max_inflation - 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accept_high_coherence() {
        let mut gate = GatePolicy::new(0.85, 0.5, 200);
        let decision = gate.evaluate(0.95, 0);
        assert!(matches!(decision, GateDecision::Accept { noise_multiplier } if (noise_multiplier - 1.0).abs() < f32::EPSILON));
        assert!(decision.allows_update());
        assert!(!decision.is_rejected());
    }

    #[test]
    fn predict_only_moderate_coherence() {
        let mut gate = GatePolicy::new(0.85, 0.5, 200);
        let decision = gate.evaluate(0.7, 0);
        assert!(matches!(decision, GateDecision::PredictOnly));
        assert!(!decision.allows_update());
        assert!(!decision.is_rejected());
    }

    #[test]
    fn reject_low_coherence() {
        let mut gate = GatePolicy::new(0.85, 0.5, 200);
        let decision = gate.evaluate(0.3, 0);
        assert!(matches!(decision, GateDecision::Reject));
        assert!(!decision.allows_update());
        assert!(decision.is_rejected());
    }

    #[test]
    fn recalibrate_after_stale_timeout() {
        let mut gate = GatePolicy::new(0.85, 0.5, 200);
        let decision = gate.evaluate(0.3, 200);
        assert!(matches!(decision, GateDecision::Recalibrate { stale_frames: 200 }));
        assert!(decision.is_rejected());
    }

    #[test]
    fn recalibrate_overrides_accept() {
        let mut gate = GatePolicy::new(0.85, 0.5, 100);
        // Even with high coherence, stale count triggers recalibration
        let decision = gate.evaluate(0.95, 100);
        assert!(matches!(decision, GateDecision::Recalibrate { .. }));
    }

    #[test]
    fn consecutive_low_counter() {
        let mut gate = GatePolicy::new(0.85, 0.5, 200);
        gate.evaluate(0.3, 0);
        assert_eq!(gate.consecutive_low_count(), 1);
        gate.evaluate(0.6, 0);
        assert_eq!(gate.consecutive_low_count(), 2);
        gate.evaluate(0.9, 0); // accepted -> resets
        assert_eq!(gate.consecutive_low_count(), 0);
    }

    #[test]
    fn last_decision_tracked() {
        let mut gate = GatePolicy::new(0.85, 0.5, 200);
        assert!(gate.last_decision().is_none());
        gate.evaluate(0.9, 0);
        assert!(gate.last_decision().is_some());
    }

    #[test]
    fn reset_clears_state() {
        let mut gate = GatePolicy::new(0.85, 0.5, 200);
        gate.evaluate(0.3, 0);
        gate.evaluate(0.3, 0);
        gate.reset();
        assert_eq!(gate.consecutive_low_count(), 0);
        assert!(gate.last_decision().is_none());
    }

    #[test]
    fn noise_multiplier_accessor() {
        let accept = GateDecision::Accept { noise_multiplier: 2.5 };
        assert_eq!(accept.noise_multiplier(), Some(2.5));

        let reject = GateDecision::Reject;
        assert_eq!(reject.noise_multiplier(), None);

        let predict = GateDecision::PredictOnly;
        assert_eq!(predict.noise_multiplier(), None);
    }

    #[test]
    fn adaptive_noise_at_boundaries() {
        assert!((adaptive_noise_multiplier(0.9, 0.85, 0.5, 3.0) - 1.0).abs() < f32::EPSILON);
        assert!((adaptive_noise_multiplier(0.3, 0.85, 0.5, 3.0) - 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn adaptive_noise_midpoint() {
        let mid = adaptive_noise_multiplier(0.675, 0.85, 0.5, 3.0);
        assert!((mid - 2.0).abs() < 0.01, "Midpoint noise should be ~2.0, got {}", mid);
    }

    #[test]
    fn adaptive_noise_tiny_range() {
        // When accept == reject, coherence >= accept returns 1.0
        let val = adaptive_noise_multiplier(0.5, 0.5, 0.5, 3.0);
        assert!((val - 1.0).abs() < f32::EPSILON);
        // Below both thresholds should return max_inflation
        let val2 = adaptive_noise_multiplier(0.4, 0.5, 0.5, 3.0);
        assert!((val2 - 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn default_config_values() {
        let cfg = GatePolicyConfig::default();
        assert!((cfg.accept_threshold - 0.85).abs() < f32::EPSILON);
        assert!((cfg.reject_threshold - 0.5).abs() < f32::EPSILON);
        assert_eq!(cfg.max_stale_frames, 200);
        assert!((cfg.predict_only_noise - 3.0).abs() < f32::EPSILON);
        assert!(!cfg.adaptive);
    }

    #[test]
    fn from_config_construction() {
        let cfg = GatePolicyConfig {
            accept_threshold: 0.9,
            reject_threshold: 0.4,
            max_stale_frames: 100,
            predict_only_noise: 5.0,
            adaptive: true,
        };
        let gate = GatePolicy::from_config(&cfg);
        assert!((gate.accept_threshold() - 0.9).abs() < f32::EPSILON);
        assert!((gate.reject_threshold() - 0.4).abs() < f32::EPSILON);
    }

    #[test]
    fn boundary_at_exact_accept_threshold() {
        let mut gate = GatePolicy::new(0.85, 0.5, 200);
        let decision = gate.evaluate(0.85, 0);
        assert!(matches!(decision, GateDecision::Accept { .. }));
    }

    #[test]
    fn boundary_at_exact_reject_threshold() {
        let mut gate = GatePolicy::new(0.85, 0.5, 200);
        let decision = gate.evaluate(0.5, 0);
        assert!(matches!(decision, GateDecision::PredictOnly));
    }

    #[test]
    fn boundary_just_below_reject_threshold() {
        let mut gate = GatePolicy::new(0.85, 0.5, 200);
        let decision = gate.evaluate(0.499, 0);
        assert!(matches!(decision, GateDecision::Reject));
    }
}
