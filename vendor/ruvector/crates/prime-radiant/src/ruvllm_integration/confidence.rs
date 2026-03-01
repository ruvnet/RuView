//! Coherence Confidence Module
//!
//! Derives confidence scores from coherence energy using a sigmoid mapping.
//! This module bridges the gap between coherence energy (mathematical) and
//! confidence scores (interpretable probability-like values).
//!
//! # Core Principle
//!
//! **Low energy = High confidence**: When the sheaf graph has low residual energy,
//! the system is coherent and we can be confident in actions.
//!
//! **High energy = Low confidence**: When residual energy is high, there are
//! contradictions or inconsistencies that reduce our confidence.
//!
//! # Mathematical Mapping
//!
//! The sigmoid function maps energy to confidence:
//!
//! ```text
//! confidence = 1 / (1 + exp(scale * (energy - threshold)))
//! ```
//!
//! Where:
//! - `energy`: The coherence energy value (0 to infinity)
//! - `threshold`: The energy level at which confidence = 0.5
//! - `scale`: Controls the steepness of the sigmoid transition
//!
//! # References
//!
//! - ADR-CE-020: Coherence Energy to Confidence Mapping

use crate::coherence::CoherenceEnergy;
use serde::{Deserialize, Serialize};

/// Configuration for the coherence-to-confidence mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceConfidence {
    /// Scale factor for the sigmoid function (controls steepness)
    ///
    /// Higher values = sharper transition around threshold
    /// Lower values = smoother, more gradual transition
    ///
    /// Typical range: 0.5 to 5.0
    pub energy_scale: f32,

    /// Energy threshold at which confidence = 0.5
    ///
    /// This is the "decision boundary" energy level.
    /// Below this threshold, confidence > 0.5
    /// Above this threshold, confidence < 0.5
    pub threshold: f32,
}

impl Default for CoherenceConfidence {
    fn default() -> Self {
        Self {
            // Default scale provides a moderate transition slope
            energy_scale: 1.0,
            // Default threshold at 1.0 energy units
            threshold: 1.0,
        }
    }
}

impl CoherenceConfidence {
    /// Create a new coherence confidence mapper
    ///
    /// # Arguments
    ///
    /// * `energy_scale` - Scale factor controlling sigmoid steepness (0.1 to 10.0)
    /// * `threshold` - Energy level at which confidence = 0.5
    ///
    /// # Panics
    ///
    /// Panics if `energy_scale` is not positive or if `threshold` is negative.
    #[must_use]
    pub fn new(energy_scale: f32, threshold: f32) -> Self {
        assert!(
            energy_scale > 0.0,
            "energy_scale must be positive, got {energy_scale}"
        );
        assert!(
            threshold >= 0.0,
            "threshold must be non-negative, got {threshold}"
        );

        Self {
            energy_scale,
            threshold,
        }
    }

    /// Create a mapper optimized for strict coherence requirements
    ///
    /// Uses a steep sigmoid (high scale) and low threshold.
    /// Even small energy values rapidly decrease confidence.
    #[must_use]
    pub fn strict() -> Self {
        Self {
            energy_scale: 3.0,
            threshold: 0.5,
        }
    }

    /// Create a mapper optimized for lenient coherence requirements
    ///
    /// Uses a gentle sigmoid (low scale) and high threshold.
    /// Confidence decreases gradually, allowing higher energy.
    #[must_use]
    pub fn lenient() -> Self {
        Self {
            energy_scale: 0.5,
            threshold: 2.0,
        }
    }

    /// Compute confidence from coherence energy
    ///
    /// Uses the sigmoid function: `conf = 1 / (1 + exp(scale * (energy - threshold)))`
    ///
    /// # Arguments
    ///
    /// * `energy` - The coherence energy value (non-negative)
    ///
    /// # Returns
    ///
    /// Confidence score in range [0.0, 1.0]
    /// - 1.0 = perfect confidence (energy ~ 0)
    /// - 0.5 = uncertain (energy = threshold)
    /// - 0.0 = no confidence (energy >> threshold)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use prime_radiant::ruvllm_integration::CoherenceConfidence;
    ///
    /// let mapper = CoherenceConfidence::default();
    ///
    /// // Low energy = high confidence
    /// let conf = mapper.confidence_from_energy(0.1);
    /// assert!(conf > 0.7);
    ///
    /// // At threshold, confidence = 0.5
    /// let conf = mapper.confidence_from_energy(1.0);
    /// assert!((conf - 0.5).abs() < 0.01);
    ///
    /// // High energy = low confidence
    /// let conf = mapper.confidence_from_energy(5.0);
    /// assert!(conf < 0.1);
    /// ```
    #[inline]
    #[must_use]
    pub fn confidence_from_energy(&self, energy: f32) -> f32 {
        // Sigmoid: conf = 1 / (1 + exp(scale * (energy - threshold)))
        // This maps:
        //   energy << threshold  =>  conf -> 1.0
        //   energy == threshold  =>  conf = 0.5
        //   energy >> threshold  =>  conf -> 0.0

        let exponent = self.energy_scale * (energy - self.threshold);

        // Handle numerical stability for extreme values
        if exponent > 20.0 {
            return 0.0; // exp(20) is huge, sigmoid -> 0
        }
        if exponent < -20.0 {
            return 1.0; // exp(-20) is tiny, sigmoid -> 1
        }

        1.0 / (1.0 + exponent.exp())
    }

    /// Compute a full confidence score with explanation from coherence energy
    ///
    /// # Arguments
    ///
    /// * `coherence_energy` - The full coherence energy object with per-edge breakdown
    ///
    /// # Returns
    ///
    /// A `ConfidenceScore` containing the confidence value, explanation, and witness flag.
    #[must_use]
    pub fn compute_confidence(&self, coherence_energy: &CoherenceEnergy) -> ConfidenceScore {
        let value = self.confidence_from_energy(coherence_energy.total_energy);

        // Determine witness-backed status based on whether we have edge-level breakdown
        let witness_backed = !coherence_energy.edge_energies.is_empty();

        // Build explanation
        let explanation = self.build_explanation(coherence_energy, value);

        ConfidenceScore {
            value,
            explanation,
            witness_backed,
            total_energy: coherence_energy.total_energy,
            edge_count: coherence_energy.edge_count,
            threshold_used: self.threshold,
            scale_used: self.energy_scale,
        }
    }

    /// Explain confidence by listing top energy contributors
    ///
    /// # Arguments
    ///
    /// * `coherence_energy` - The coherence energy with per-edge breakdown
    /// * `top_k` - Number of top contributors to include (default: 5)
    ///
    /// # Returns
    ///
    /// A vector of energy contributors sorted by energy (highest first)
    #[must_use]
    pub fn explain_confidence(
        &self,
        coherence_energy: &CoherenceEnergy,
        top_k: usize,
    ) -> Vec<EnergyContributor> {
        let hotspots = coherence_energy.hotspots(top_k);

        hotspots
            .into_iter()
            .map(|h| EnergyContributor {
                edge_id: h.edge_id,
                source: h.source,
                target: h.target,
                energy: h.energy,
                percentage: h.percentage,
                contribution_to_confidence_drop: self.compute_contribution_effect(h.energy),
            })
            .collect()
    }

    /// Build a human-readable explanation of the confidence score
    fn build_explanation(&self, coherence_energy: &CoherenceEnergy, confidence: f32) -> String {
        let energy = coherence_energy.total_energy;
        let edge_count = coherence_energy.edge_count;

        let confidence_level = if confidence >= 0.9 {
            "very high"
        } else if confidence >= 0.7 {
            "high"
        } else if confidence >= 0.5 {
            "moderate"
        } else if confidence >= 0.3 {
            "low"
        } else {
            "very low"
        };

        let energy_assessment = if energy < self.threshold * 0.5 {
            "well below threshold"
        } else if energy < self.threshold {
            "below threshold"
        } else if energy < self.threshold * 1.5 {
            "near threshold"
        } else if energy < self.threshold * 2.0 {
            "above threshold"
        } else {
            "significantly above threshold"
        };

        format!(
            "Confidence is {} ({:.1}%) based on total energy {:.4} ({}) \
             computed from {} edges. Threshold: {:.2}, Scale: {:.2}.",
            confidence_level,
            confidence * 100.0,
            energy,
            energy_assessment,
            edge_count,
            self.threshold,
            self.energy_scale
        )
    }

    /// Compute how much a single edge's energy contributes to confidence drop
    ///
    /// This estimates the marginal effect of one edge's energy on overall confidence.
    fn compute_contribution_effect(&self, edge_energy: f32) -> f32 {
        // The effect is proportional to the derivative of the sigmoid at the threshold
        // Derivative of sigmoid: f'(x) = f(x) * (1 - f(x)) * scale
        // At threshold: f(threshold) = 0.5, so f'(threshold) = 0.25 * scale
        //
        // For a single edge, the approximate confidence drop is:
        // delta_conf ≈ 0.25 * scale * edge_energy
        let max_derivative = 0.25 * self.energy_scale;
        (max_derivative * edge_energy).min(1.0)
    }

    /// Get the confidence at exactly the threshold energy
    ///
    /// By design, this should always return 0.5.
    #[inline]
    #[must_use]
    pub fn confidence_at_threshold(&self) -> f32 {
        0.5
    }

    /// Calculate the energy level for a desired confidence
    ///
    /// Inverse of the sigmoid function.
    ///
    /// # Arguments
    ///
    /// * `confidence` - Desired confidence level (0.0, 1.0)
    ///
    /// # Returns
    ///
    /// The energy level that would produce this confidence, or None if confidence
    /// is at the boundaries (0 or 1).
    #[must_use]
    pub fn energy_for_confidence(&self, confidence: f32) -> Option<f32> {
        if confidence <= 0.0 || confidence >= 1.0 {
            return None;
        }

        // Inverse sigmoid: energy = threshold + ln((1-conf)/conf) / scale
        let odds = (1.0 - confidence) / confidence;
        Some(self.threshold + odds.ln() / self.energy_scale)
    }

    /// Batch compute confidences for multiple energy values
    ///
    /// More efficient than calling `confidence_from_energy` in a loop.
    #[must_use]
    pub fn batch_confidence(&self, energies: &[f32]) -> Vec<f32> {
        energies
            .iter()
            .map(|&e| self.confidence_from_energy(e))
            .collect()
    }
}

/// A confidence score derived from coherence energy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceScore {
    /// Confidence value in range [0.0, 1.0]
    ///
    /// - 1.0 = perfect confidence (system is fully coherent)
    /// - 0.5 = uncertain (energy at threshold)
    /// - 0.0 = no confidence (high incoherence)
    pub value: f32,

    /// Human-readable explanation of the confidence score
    pub explanation: String,

    /// Whether this confidence is backed by witness records
    ///
    /// True if the confidence was computed from a CoherenceEnergy
    /// with edge-level breakdown (not just total energy).
    pub witness_backed: bool,

    /// The total coherence energy used to compute this score
    pub total_energy: f32,

    /// Number of edges contributing to the energy
    pub edge_count: usize,

    /// The threshold used for this computation
    pub threshold_used: f32,

    /// The scale factor used for this computation
    pub scale_used: f32,
}

impl ConfidenceScore {
    /// Create a confidence score from just a value (no witness)
    ///
    /// Use this for quick confidence checks without full energy breakdown.
    #[must_use]
    pub fn from_value(value: f32) -> Self {
        Self {
            value: value.clamp(0.0, 1.0),
            explanation: format!("Direct confidence value: {:.1}%", value * 100.0),
            witness_backed: false,
            total_energy: f32::NAN,
            edge_count: 0,
            threshold_used: f32::NAN,
            scale_used: f32::NAN,
        }
    }

    /// Check if confidence is above a given threshold
    #[inline]
    #[must_use]
    pub fn is_confident(&self, min_confidence: f32) -> bool {
        self.value >= min_confidence
    }

    /// Check if this score is high confidence (>= 0.7)
    #[inline]
    #[must_use]
    pub fn is_high_confidence(&self) -> bool {
        self.value >= 0.7
    }

    /// Check if this score is low confidence (< 0.3)
    #[inline]
    #[must_use]
    pub fn is_low_confidence(&self) -> bool {
        self.value < 0.3
    }

    /// Get the confidence as a percentage (0-100)
    #[inline]
    #[must_use]
    pub fn as_percentage(&self) -> f32 {
        self.value * 100.0
    }

    /// Get a categorical confidence level
    #[must_use]
    pub fn level(&self) -> ConfidenceLevel {
        if self.value >= 0.9 {
            ConfidenceLevel::VeryHigh
        } else if self.value >= 0.7 {
            ConfidenceLevel::High
        } else if self.value >= 0.5 {
            ConfidenceLevel::Moderate
        } else if self.value >= 0.3 {
            ConfidenceLevel::Low
        } else {
            ConfidenceLevel::VeryLow
        }
    }
}

impl std::fmt::Display for ConfidenceScore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.1}% confidence", self.as_percentage())
    }
}

/// Categorical confidence levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConfidenceLevel {
    /// >= 90% confidence
    VeryHigh,
    /// >= 70% confidence
    High,
    /// >= 50% confidence
    Moderate,
    /// >= 30% confidence
    Low,
    /// < 30% confidence
    VeryLow,
}

impl ConfidenceLevel {
    /// Check if this level allows action execution
    #[must_use]
    pub fn allows_action(&self) -> bool {
        matches!(self, Self::VeryHigh | Self::High | Self::Moderate)
    }

    /// Check if this level requires escalation
    #[must_use]
    pub fn requires_escalation(&self) -> bool {
        matches!(self, Self::Low | Self::VeryLow)
    }
}

impl std::fmt::Display for ConfidenceLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::VeryHigh => "Very High",
            Self::High => "High",
            Self::Moderate => "Moderate",
            Self::Low => "Low",
            Self::VeryLow => "Very Low",
        };
        write!(f, "{s}")
    }
}

/// An edge that contributes to the confidence score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyContributor {
    /// Edge identifier
    pub edge_id: String,
    /// Source node
    pub source: String,
    /// Target node
    pub target: String,
    /// Energy value for this edge
    pub energy: f32,
    /// Percentage of total energy
    pub percentage: f32,
    /// Estimated contribution to confidence drop (0 to 1)
    pub contribution_to_confidence_drop: f32,
}

impl EnergyContributor {
    /// Check if this edge is a significant contributor (>10% of total)
    #[inline]
    #[must_use]
    pub fn is_significant(&self) -> bool {
        self.percentage > 10.0
    }

    /// Check if this edge is the dominant contributor (>50% of total)
    #[inline]
    #[must_use]
    pub fn is_dominant(&self) -> bool {
        self.percentage > 50.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coherence::EdgeEnergy;
    use std::collections::HashMap;

    fn create_test_energy(total: f32, edge_count: usize) -> CoherenceEnergy {
        let mut edge_energies = HashMap::new();
        let energy_per_edge = if edge_count > 0 {
            total / edge_count as f32
        } else {
            0.0
        };

        for i in 0..edge_count {
            let edge_id = format!("e{i}");
            edge_energies.insert(
                edge_id.clone(),
                EdgeEnergy::new(
                    edge_id,
                    format!("n{i}"),
                    format!("n{}", i + 1),
                    vec![(energy_per_edge / 1.0).sqrt()], // residual that gives energy_per_edge
                    1.0,
                ),
            );
        }

        CoherenceEnergy::new(edge_energies, &HashMap::new(), edge_count + 1, "test")
    }

    #[test]
    fn test_confidence_at_threshold() {
        let mapper = CoherenceConfidence::default();

        let conf = mapper.confidence_from_energy(mapper.threshold);
        assert!(
            (conf - 0.5).abs() < 0.001,
            "Confidence at threshold should be 0.5, got {conf}"
        );
    }

    #[test]
    fn test_low_energy_high_confidence() {
        let mapper = CoherenceConfidence::default();

        // Energy much below threshold should give high confidence
        let conf = mapper.confidence_from_energy(0.1);
        assert!(
            conf > 0.7,
            "Low energy should give high confidence, got {conf}"
        );

        // Zero energy should give ~1.0 confidence
        let conf = mapper.confidence_from_energy(0.0);
        assert!(
            conf > 0.9,
            "Zero energy should give very high confidence, got {conf}"
        );
    }

    #[test]
    fn test_high_energy_low_confidence() {
        let mapper = CoherenceConfidence::default();

        // Energy above threshold should give low confidence
        let conf = mapper.confidence_from_energy(3.0);
        assert!(
            conf < 0.3,
            "High energy should give low confidence, got {conf}"
        );

        // Very high energy should give ~0 confidence
        let conf = mapper.confidence_from_energy(10.0);
        assert!(
            conf < 0.01,
            "Very high energy should give near-zero confidence, got {conf}"
        );
    }

    #[test]
    fn test_sigmoid_monotonicity() {
        let mapper = CoherenceConfidence::default();

        // Confidence should decrease as energy increases
        let energies = [0.0, 0.5, 1.0, 1.5, 2.0, 3.0, 5.0];
        let confidences: Vec<f32> = energies
            .iter()
            .map(|&e| mapper.confidence_from_energy(e))
            .collect();

        for i in 1..confidences.len() {
            assert!(
                confidences[i] < confidences[i - 1],
                "Confidence should decrease: {} should be < {}",
                confidences[i],
                confidences[i - 1]
            );
        }
    }

    #[test]
    fn test_scale_affects_steepness() {
        let steep = CoherenceConfidence::new(3.0, 1.0);
        let gentle = CoherenceConfidence::new(0.5, 1.0);

        // At threshold, both should give 0.5
        assert!((steep.confidence_from_energy(1.0) - 0.5).abs() < 0.001);
        assert!((gentle.confidence_from_energy(1.0) - 0.5).abs() < 0.001);

        // Slightly above threshold: steep should drop faster
        let steep_conf = steep.confidence_from_energy(1.5);
        let gentle_conf = gentle.confidence_from_energy(1.5);
        assert!(
            steep_conf < gentle_conf,
            "Steep scale should drop faster: {} vs {}",
            steep_conf,
            gentle_conf
        );
    }

    #[test]
    fn test_strict_vs_lenient() {
        let strict = CoherenceConfidence::strict();
        let lenient = CoherenceConfidence::lenient();

        // At moderate energy (1.0), strict should be much less confident
        let strict_conf = strict.confidence_from_energy(1.0);
        let lenient_conf = lenient.confidence_from_energy(1.0);

        assert!(
            strict_conf < lenient_conf,
            "Strict should be less confident at same energy"
        );
    }

    #[test]
    fn test_compute_confidence_full() {
        let mapper = CoherenceConfidence::default();
        let energy = create_test_energy(0.5, 3);

        let score = mapper.compute_confidence(&energy);

        assert!(score.value > 0.5, "Low energy should give >0.5 confidence");
        assert!(
            score.witness_backed,
            "Should be witness-backed with edge data"
        );
        assert_eq!(score.edge_count, 3);
        assert!(!score.explanation.is_empty());
    }

    #[test]
    fn test_explain_confidence() {
        let mapper = CoherenceConfidence::default();
        let energy = create_test_energy(2.0, 5);

        let contributors = mapper.explain_confidence(&energy, 3);

        assert!(contributors.len() <= 3);
        for contrib in &contributors {
            assert!(contrib.energy >= 0.0);
            assert!(contrib.percentage >= 0.0);
        }
    }

    #[test]
    fn test_energy_for_confidence_inverse() {
        let mapper = CoherenceConfidence::default();

        // Test round-trip: confidence -> energy -> confidence
        let original_conf = 0.75;
        if let Some(energy) = mapper.energy_for_confidence(original_conf) {
            let recovered_conf = mapper.confidence_from_energy(energy);
            assert!(
                (recovered_conf - original_conf).abs() < 0.001,
                "Round-trip failed: {} vs {}",
                original_conf,
                recovered_conf
            );
        }

        // Boundary cases should return None
        assert!(mapper.energy_for_confidence(0.0).is_none());
        assert!(mapper.energy_for_confidence(1.0).is_none());
    }

    #[test]
    fn test_confidence_score_levels() {
        assert_eq!(
            ConfidenceScore::from_value(0.95).level(),
            ConfidenceLevel::VeryHigh
        );
        assert_eq!(
            ConfidenceScore::from_value(0.75).level(),
            ConfidenceLevel::High
        );
        assert_eq!(
            ConfidenceScore::from_value(0.55).level(),
            ConfidenceLevel::Moderate
        );
        assert_eq!(
            ConfidenceScore::from_value(0.35).level(),
            ConfidenceLevel::Low
        );
        assert_eq!(
            ConfidenceScore::from_value(0.15).level(),
            ConfidenceLevel::VeryLow
        );
    }

    #[test]
    fn test_confidence_level_actions() {
        assert!(ConfidenceLevel::VeryHigh.allows_action());
        assert!(ConfidenceLevel::High.allows_action());
        assert!(ConfidenceLevel::Moderate.allows_action());
        assert!(!ConfidenceLevel::Low.allows_action());
        assert!(!ConfidenceLevel::VeryLow.allows_action());

        assert!(!ConfidenceLevel::VeryHigh.requires_escalation());
        assert!(ConfidenceLevel::Low.requires_escalation());
        assert!(ConfidenceLevel::VeryLow.requires_escalation());
    }

    #[test]
    fn test_batch_confidence() {
        let mapper = CoherenceConfidence::default();
        let energies = vec![0.0, 0.5, 1.0, 2.0, 5.0];

        let confidences = mapper.batch_confidence(&energies);

        assert_eq!(confidences.len(), energies.len());
        for (i, &conf) in confidences.iter().enumerate() {
            let expected = mapper.confidence_from_energy(energies[i]);
            assert!((conf - expected).abs() < 1e-6);
        }
    }

    #[test]
    fn test_numerical_stability() {
        let mapper = CoherenceConfidence::default();

        // Very large energy should not cause overflow
        let conf = mapper.confidence_from_energy(1000.0);
        assert!(
            conf >= 0.0 && conf <= 1.0,
            "Large energy gave invalid confidence: {conf}"
        );
        assert!(
            conf < 0.001,
            "Large energy should give near-zero confidence"
        );

        // Negative energy (shouldn't happen, but test stability)
        let conf = mapper.confidence_from_energy(-100.0);
        assert!(
            conf >= 0.0 && conf <= 1.0,
            "Negative energy gave invalid confidence: {conf}"
        );
        assert!(
            conf > 0.999,
            "Negative energy should give near-one confidence"
        );
    }

    #[test]
    fn test_energy_contributor() {
        let contrib = EnergyContributor {
            edge_id: "e1".to_string(),
            source: "a".to_string(),
            target: "b".to_string(),
            energy: 0.5,
            percentage: 25.0,
            contribution_to_confidence_drop: 0.125,
        };

        assert!(contrib.is_significant());
        assert!(!contrib.is_dominant());

        let dominant = EnergyContributor {
            edge_id: "e2".to_string(),
            source: "c".to_string(),
            target: "d".to_string(),
            energy: 1.5,
            percentage: 60.0,
            contribution_to_confidence_drop: 0.375,
        };

        assert!(dominant.is_significant());
        assert!(dominant.is_dominant());
    }
}
