//! Threshold adjustment types.

use super::config::ThresholdConfig;
use serde::{Deserialize, Serialize};

/// Reason for a threshold adjustment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AdjustmentReason {
    /// Energy spike detected, tightening thresholds.
    EnergySpike {
        /// The spike magnitude.
        magnitude: f32,
    },
    /// Sustained incoherence, adjusting for stability.
    SustainedIncoherence {
        /// Duration in seconds.
        duration_secs: f32,
    },
    /// Success pattern detected, optimizing thresholds.
    SuccessPattern {
        /// Pattern similarity score.
        similarity: f32,
    },
    /// Manual override requested.
    ManualOverride,
    /// Background learning produced new optimal values.
    BackgroundLearning {
        /// Number of training samples.
        samples: usize,
    },
    /// Cold start initialization.
    ColdStart,
    /// Regime change detected.
    RegimeChange {
        /// Detected regime identifier.
        regime_id: String,
    },
}

/// Recommended threshold adjustment from the tuner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdAdjustment {
    /// The recommended new threshold configuration.
    pub new_thresholds: ThresholdConfig,
    /// Reason for the adjustment.
    pub reason: AdjustmentReason,
    /// Confidence in the adjustment (0.0 to 1.0).
    pub confidence: f32,
    /// Whether the adjustment is urgent (should be applied immediately).
    pub urgent: bool,
    /// Delta from current thresholds.
    pub delta: ThresholdDelta,
    /// Timestamp when adjustment was computed.
    pub timestamp_ms: u64,
}

impl ThresholdAdjustment {
    /// Create a new threshold adjustment.
    pub fn new(
        current: &ThresholdConfig,
        new_thresholds: ThresholdConfig,
        reason: AdjustmentReason,
        confidence: f32,
    ) -> Self {
        let delta = ThresholdDelta {
            reflex_delta: new_thresholds.reflex - current.reflex,
            retrieval_delta: new_thresholds.retrieval - current.retrieval,
            heavy_delta: new_thresholds.heavy - current.heavy,
        };

        let urgent = matches!(
            reason,
            AdjustmentReason::EnergySpike { magnitude } if magnitude > 0.5
        );

        Self {
            new_thresholds,
            reason,
            confidence,
            urgent,
            delta,
            timestamp_ms: current_time_ms(),
        }
    }

    /// Create an adjustment for an energy spike.
    pub fn for_energy_spike(current: &ThresholdConfig, spike_magnitude: f32) -> Self {
        // Tighten thresholds proportionally to spike
        let factor = 1.0 - (spike_magnitude * 0.5).min(0.4);
        let new = ThresholdConfig {
            reflex: current.reflex * factor,
            retrieval: current.retrieval * factor,
            heavy: current.heavy * factor,
            persistence_window_secs: current.persistence_window_secs,
        };

        Self::new(
            current,
            new,
            AdjustmentReason::EnergySpike {
                magnitude: spike_magnitude,
            },
            0.8 + spike_magnitude * 0.1,
        )
    }

    /// Create an adjustment based on a success pattern.
    pub fn from_success_pattern(
        current: &ThresholdConfig,
        pattern_thresholds: ThresholdConfig,
        similarity: f32,
    ) -> Self {
        // Interpolate toward the successful pattern based on similarity
        let new = current.lerp(&pattern_thresholds, similarity * 0.5);

        Self::new(
            current,
            new,
            AdjustmentReason::SuccessPattern { similarity },
            similarity,
        )
    }

    /// Check if this adjustment is significant enough to apply.
    pub fn is_significant(&self) -> bool {
        self.delta.max_abs_delta() > 0.01 && self.confidence > 0.5
    }

    /// Get the magnitude of the adjustment.
    pub fn magnitude(&self) -> f32 {
        self.delta.max_abs_delta()
    }
}

/// Delta between two threshold configurations.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ThresholdDelta {
    /// Change in reflex threshold.
    pub reflex_delta: f32,
    /// Change in retrieval threshold.
    pub retrieval_delta: f32,
    /// Change in heavy threshold.
    pub heavy_delta: f32,
}

impl ThresholdDelta {
    /// Get the maximum absolute delta.
    pub fn max_abs_delta(&self) -> f32 {
        self.reflex_delta
            .abs()
            .max(self.retrieval_delta.abs())
            .max(self.heavy_delta.abs())
    }

    /// Get the total magnitude of change.
    pub fn total_magnitude(&self) -> f32 {
        (self.reflex_delta.powi(2) + self.retrieval_delta.powi(2) + self.heavy_delta.powi(2)).sqrt()
    }
}

/// Get current time in milliseconds.
fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_energy_spike_adjustment() {
        let current = ThresholdConfig::default();
        let adj = ThresholdAdjustment::for_energy_spike(&current, 0.5);

        assert!(adj.new_thresholds.reflex < current.reflex);
        assert!(adj.confidence > 0.8);
        assert!(adj.urgent);
    }

    #[test]
    fn test_success_pattern_adjustment() {
        let current = ThresholdConfig::default();
        let pattern = ThresholdConfig::conservative();

        let adj = ThresholdAdjustment::from_success_pattern(&current, pattern, 0.9);

        assert!(adj.new_thresholds.reflex < current.reflex);
        assert!(adj.confidence > 0.8);
    }

    #[test]
    fn test_threshold_delta() {
        let delta = ThresholdDelta {
            reflex_delta: 0.1,
            retrieval_delta: -0.2,
            heavy_delta: 0.05,
        };

        assert!((delta.max_abs_delta() - 0.2).abs() < 0.001);
    }
}
