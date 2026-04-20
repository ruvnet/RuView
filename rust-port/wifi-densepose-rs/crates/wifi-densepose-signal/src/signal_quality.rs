//! Signal Quality Scoring Module (EML Improvement 5)
//!
//! Provides a learned signal quality score from raw signal characteristics.
//! Uses an EML (exp-ln) model to combine multiple quality indicators into
//! a single [0, 1] score.
//!
//! The five input features are:
//! - **SNR**: Signal-to-noise ratio (dB)
//! - **Variance**: Amplitude variance across subcarriers
//! - **Subcarrier count**: Number of usable subcarriers
//! - **Packet rate**: CSI packets per second
//! - **Multipath spread**: Delay spread from multipath propagation
//!
//! When untrained, falls back to a simple heuristic combination.
//!
//! Based on: Odrzywolel 2026, "All elementary functions from a single
//! operator" (arXiv:2603.21852v2)

use crate::eml::{EmlConfig, EmlModel};
use serde::{Deserialize, Serialize};

/// Input features for signal quality scoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalQualityInput {
    /// Signal-to-noise ratio in dB (typical range: 0-50).
    pub snr_db: f64,
    /// Amplitude variance across subcarriers (typical range: 0-10).
    pub variance: f64,
    /// Number of usable subcarriers (typical range: 0-256).
    pub subcarrier_count: usize,
    /// CSI packet reception rate in Hz (typical range: 0-1000).
    pub packet_rate_hz: f64,
    /// Multipath delay spread in nanoseconds (typical range: 0-500).
    pub multipath_spread_ns: f64,
}

impl SignalQualityInput {
    /// Normalize inputs to approximately [0, 1] for the EML model.
    fn normalize(&self) -> [f64; 5] {
        [
            (self.snr_db / 50.0).clamp(0.0, 1.0),
            (self.variance / 10.0).clamp(0.0, 1.0),
            (self.subcarrier_count as f64 / 256.0).clamp(0.0, 1.0),
            (self.packet_rate_hz / 1000.0).clamp(0.0, 1.0),
            (self.multipath_spread_ns / 500.0).clamp(0.0, 1.0),
        ]
    }
}

/// Signal quality scorer using EML learned functions.
///
/// Combines five signal characteristics into a single quality score
/// in [0, 1]. When trained on labeled signal quality data, discovers
/// non-linear relationships between signal features and quality.
///
/// # Heuristic fallback
///
/// When untrained, uses a simple weighted combination:
/// ```text
/// quality = 0.35 * snr_norm + 0.20 * (1 - variance_norm)
///         + 0.15 * subcarrier_norm + 0.15 * packet_rate_norm
///         + 0.15 * (1 - multipath_norm)
/// ```
#[derive(Debug)]
pub struct SignalQualityScorer {
    /// EML model: depth-3, 5 inputs, 1 output.
    eml_model: EmlModel,
}

impl SignalQualityScorer {
    /// Create a new signal quality scorer with an untrained EML model.
    ///
    /// The model has a depth-3 binary tree with 5 inputs and 1 output,
    /// containing approximately 50 trainable parameters.
    pub fn new() -> Self {
        Self {
            eml_model: EmlModel::new(EmlConfig {
                depth: 3,
                n_inputs: 5,
                n_outputs: 1,
            }),
        }
    }

    /// Create a scorer with a pre-trained EML model.
    pub fn with_model(model: EmlModel) -> Self {
        Self { eml_model: model }
    }

    /// Score the signal quality.
    ///
    /// Returns a value in [0, 1] where:
    /// - 1.0 = excellent signal quality
    /// - 0.7+ = good, suitable for vital sign extraction
    /// - 0.4-0.7 = degraded, motion detection only
    /// - < 0.4 = poor, unreliable measurements
    pub fn score(&self, input: &SignalQualityInput) -> f64 {
        let normalized = input.normalize();

        if self.eml_model.is_trained() {
            // Use learned EML model for non-linear quality scoring.
            let output = self.eml_model.predict(&normalized);
            output.first().copied().unwrap_or(0.0).clamp(0.0, 1.0)
        } else {
            // Heuristic fallback (backward compatible).
            self.heuristic_score(&normalized)
        }
    }

    /// Simple heuristic quality score.
    ///
    /// Higher SNR, more subcarriers, and higher packet rate are better.
    /// Higher variance and multipath spread are worse.
    fn heuristic_score(&self, normalized: &[f64; 5]) -> f64 {
        let score = 0.35 * normalized[0]          // SNR (higher = better)
            + 0.20 * (1.0 - normalized[1])         // variance (lower = better)
            + 0.15 * normalized[2]                  // subcarrier count (more = better)
            + 0.15 * normalized[3]                  // packet rate (higher = better)
            + 0.15 * (1.0 - normalized[4]);         // multipath spread (lower = better)
        score.clamp(0.0, 1.0)
    }

    /// Train the EML model on labeled signal quality data.
    ///
    /// - `data`: pairs of (signal features, ground truth quality score).
    /// - `epochs`: number of coordinate descent iterations.
    ///
    /// Returns the final mean squared error.
    pub fn train(&mut self, data: &[(SignalQualityInput, f64)], epochs: usize) -> f64 {
        let inputs: Vec<Vec<f64>> = data
            .iter()
            .map(|(input, _)| input.normalize().to_vec())
            .collect();
        let targets: Vec<Vec<f64>> = data.iter().map(|(_, target)| vec![*target]).collect();
        self.eml_model.train(&inputs, &targets, epochs, 0.1)
    }

    /// Whether the underlying EML model has been trained.
    pub fn is_trained(&self) -> bool {
        self.eml_model.is_trained()
    }

    /// Get a reference to the underlying EML model (e.g., for serialization).
    pub fn model(&self) -> &EmlModel {
        &self.eml_model
    }

    /// Number of trainable parameters in the EML model.
    pub fn param_count(&self) -> usize {
        self.eml_model.param_count()
    }
}

impl Default for SignalQualityScorer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_input(snr: f64, variance: f64, subcarriers: usize, rate: f64, spread: f64) -> SignalQualityInput {
        SignalQualityInput {
            snr_db: snr,
            variance,
            subcarrier_count: subcarriers,
            packet_rate_hz: rate,
            multipath_spread_ns: spread,
        }
    }

    #[test]
    fn test_heuristic_score_high_quality() {
        let scorer = SignalQualityScorer::new();
        let input = make_input(40.0, 0.5, 200, 500.0, 20.0);
        let score = scorer.score(&input);
        assert!(
            score > 0.6,
            "high quality signal should score > 0.6: {score}"
        );
    }

    #[test]
    fn test_heuristic_score_low_quality() {
        let scorer = SignalQualityScorer::new();
        let input = make_input(5.0, 8.0, 20, 50.0, 400.0);
        let score = scorer.score(&input);
        assert!(
            score < 0.4,
            "low quality signal should score < 0.4: {score}"
        );
    }

    #[test]
    fn test_score_in_range() {
        let scorer = SignalQualityScorer::new();
        // Test various inputs.
        for snr in [0.0, 10.0, 25.0, 50.0] {
            for variance in [0.0, 2.0, 5.0, 10.0] {
                let input = make_input(snr, variance, 64, 100.0, 50.0);
                let score = scorer.score(&input);
                assert!(
                    (0.0..=1.0).contains(&score),
                    "score out of range: {score}"
                );
            }
        }
    }

    #[test]
    fn test_model_creation() {
        let scorer = SignalQualityScorer::new();
        assert!(!scorer.is_trained());
        assert!(scorer.param_count() > 0);
    }

    #[test]
    fn test_default() {
        let scorer = SignalQualityScorer::default();
        assert!(!scorer.is_trained());
    }

    #[test]
    fn test_normalization() {
        let input = make_input(25.0, 5.0, 128, 500.0, 250.0);
        let norm = input.normalize();
        for v in &norm {
            assert!(
                (0.0..=1.0).contains(v),
                "normalized value out of range: {v}"
            );
        }
        assert!((norm[0] - 0.5).abs() < 1e-10);
        assert!((norm[1] - 0.5).abs() < 1e-10);
        assert!((norm[2] - 0.5).abs() < 1e-10);
        assert!((norm[3] - 0.5).abs() < 1e-10);
        assert!((norm[4] - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_higher_snr_gives_higher_score() {
        let scorer = SignalQualityScorer::new();
        let low = make_input(5.0, 2.0, 64, 100.0, 50.0);
        let high = make_input(45.0, 2.0, 64, 100.0, 50.0);
        assert!(
            scorer.score(&high) > scorer.score(&low),
            "higher SNR should give higher quality score"
        );
    }
}
