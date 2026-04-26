//! Ensemble classifier that combines breathing, heartbeat, and movement signals
//! into a unified survivor detection confidence score.
//!
//! The ensemble uses weighted voting across the three detector signals:
//! - Breathing presence is the strongest indicator of a living survivor
//! - Heartbeat (when enabled) provides high-confidence confirmation
//! - Movement type distinguishes active vs trapped survivors
//!
//! The classifier produces a single confidence score and a recommended
//! triage status based on the combined signals.

use crate::domain::{
    BreathingType, MovementType, TriageStatus, VitalSignsReading,
};

/// Configuration for the ensemble classifier
#[derive(Debug, Clone)]
pub struct EnsembleConfig {
    /// Weight for breathing signal (0.0-1.0)
    pub breathing_weight: f64,
    /// Weight for heartbeat signal (0.0-1.0)
    pub heartbeat_weight: f64,
    /// Weight for movement signal (0.0-1.0)
    pub movement_weight: f64,
    /// Minimum combined confidence to report a detection
    pub min_ensemble_confidence: f64,
}

impl Default for EnsembleConfig {
    fn default() -> Self {
        Self {
            breathing_weight: 0.50,
            heartbeat_weight: 0.30,
            movement_weight: 0.20,
            min_ensemble_confidence: 0.3,
        }
    }
}

/// Result of ensemble classification
#[derive(Debug, Clone)]
pub struct EnsembleResult {
    /// Combined confidence score (0.0-1.0)
    pub confidence: f64,
    /// Recommended triage status based on signal analysis
    pub recommended_triage: TriageStatus,
    /// Whether breathing was detected
    pub breathing_detected: bool,
    /// Whether heartbeat was detected
    pub heartbeat_detected: bool,
    /// Whether meaningful movement was detected
    pub movement_detected: bool,
    /// Individual signal confidences
    pub signal_confidences: SignalConfidences,
}

/// Individual confidence scores for each signal type
#[derive(Debug, Clone)]
pub struct SignalConfidences {
    /// Breathing detection confidence
    pub breathing: f64,
    /// Heartbeat detection confidence
    pub heartbeat: f64,
    /// Movement detection confidence
    pub movement: f64,
}

/// Ensemble classifier combining breathing, heartbeat, and movement detectors
pub struct EnsembleClassifier {
    config: EnsembleConfig,
}

impl EnsembleClassifier {
    /// Create a new ensemble classifier
    pub fn new(config: EnsembleConfig) -> Self {
        Self { config }
    }

    /// Classify a vital signs reading using weighted ensemble voting.
    ///
    /// The ensemble combines individual detector outputs with configured weights
    /// to produce a single confidence score and triage recommendation.
    pub fn classify(&self, reading: &VitalSignsReading) -> EnsembleResult {
        // Extract individual signal confidences (using method calls)
        let breathing_conf = reading
            .breathing
            .as_ref()
            .map(|b| b.confidence())
            .unwrap_or(0.0);

        let heartbeat_conf = reading
            .heartbeat
            .as_ref()
            .map(|h| h.confidence())
            .unwrap_or(0.0);

        let movement_conf = if reading.movement.movement_type != MovementType::None {
            reading.movement.confidence()
        } else {
            0.0
        };

        // Weighted ensemble confidence
        let total_weight =
            self.config.breathing_weight + self.config.heartbeat_weight + self.config.movement_weight;

        let ensemble_confidence = if total_weight > 0.0 {
            (breathing_conf * self.config.breathing_weight
                + heartbeat_conf * self.config.heartbeat_weight
                + movement_conf * self.config.movement_weight)
                / total_weight
        } else {
            0.0
        };

        let breathing_detected = reading.breathing.is_some();
        let heartbeat_detected = reading.heartbeat.is_some();
        let movement_detected = reading.movement.movement_type != MovementType::None;

        // Determine triage status from signal combination
        let recommended_triage = self.determine_triage(reading, ensemble_confidence);

        EnsembleResult {
            confidence: ensemble_confidence,
            recommended_triage,
            breathing_detected,
            heartbeat_detected,
            movement_detected,
            signal_confidences: SignalConfidences {
                breathing: breathing_conf,
                heartbeat: heartbeat_conf,
                movement: movement_conf,
            },
        }
    }

    /// Determine triage status based on vital signs analysis.
    ///
    /// Uses START triage protocol logic:
    /// - Immediate (Red): Breathing abnormal (agonal, apnea, too fast/slow)
    /// - Delayed (Yellow): Breathing present, limited movement
    /// - Minor (Green): Normal breathing + active movement
    /// - Deceased (Black): No vitals detected at all
    /// - Unknown: Insufficient data to classify
    ///
    /// Critical patterns (Agonal, Apnea, extreme rates) are always classified
    /// as Immediate regardless of confidence level, because in disaster response
    /// a false negative (missing a survivor in distress) is far more costly
    /// than a false positive.
    fn determine_triage(
        &self,
        reading: &VitalSignsReading,
        confidence: f64,
    ) -> TriageStatus {
        // CRITICAL PATTERNS: always classify regardless of confidence.
        // In disaster response, any sign of distress must be escalated.
        if let Some(ref breathing) = reading.breathing {
            match breathing.pattern_type {
                BreathingType::Agonal | BreathingType::Apnea => {
                    return TriageStatus::Immediate;
                }
                _ => {}
            }

            let rate = breathing.rate_bpm;
            if rate < 10.0 || rate > 30.0 {
                return TriageStatus::Immediate;
            }
        }

        // Below confidence threshold: not enough signal to classify further
        if confidence < self.config.min_ensemble_confidence {
            return TriageStatus::Unknown;
        }

        let has_breathing = reading.breathing.is_some();
        let has_movement = reading.movement.movement_type != MovementType::None;

        if !has_breathing && !has_movement {
            return TriageStatus::Deceased;
        }

        if !has_breathing && has_movement {
            return TriageStatus::Immediate;
        }

        // Has breathing above threshold - assess triage level
        if let Some(ref breathing) = reading.breathing {
            let rate = breathing.rate_bpm;

            if rate < 12.0 || rate > 24.0 {
                if has_movement {
                    return TriageStatus::Delayed;
                }
                return TriageStatus::Immediate;
            }

            // Normal breathing rate
            if has_movement {
                return TriageStatus::Minor;
            }
            return TriageStatus::Delayed;
        }

        TriageStatus::Unknown
    }

    /// Get configuration
    pub fn config(&self) -> &EnsembleConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        BreathingPattern, HeartbeatSignature, MovementProfile,
        SignalStrength, ConfidenceScore,
    };

    fn make_reading(
        breathing: Option<(f32, BreathingType)>,
        heartbeat: Option<f32>,
        movement: MovementType,
    ) -> VitalSignsReading {
        let bp = breathing.map(|(rate, pattern_type)| BreathingPattern {
            rate_bpm: rate,
            pattern_type,
            amplitude: 0.9,
            regularity: 0.9,
        });

        let hb = heartbeat.map(|rate| HeartbeatSignature {
            rate_bpm: rate,
            variability: 0.1,
            strength: SignalStrength::Moderate,
        });

        let is_moving = movement != MovementType::None;
        let mv = MovementProfile {
            movement_type: movement,
            intensity: if is_moving { 0.5 } else { 0.0 },
            frequency: 0.0,
            is_voluntary: is_moving,
        };

        VitalSignsReading::new(bp, hb, mv)
    }

    #[test]
    fn test_normal_breathing_with_movement_is_minor() {
        let classifier = EnsembleClassifier::new(EnsembleConfig::default());
        let reading = make_reading(
            Some((16.0, BreathingType::Normal)),
            None,
            MovementType::Periodic,
        );

        let result = classifier.classify(&reading);
        assert!(result.confidence > 0.0);
        assert_eq!(result.recommended_triage, TriageStatus::Minor);
        assert!(result.breathing_detected);
    }

    #[test]
    fn test_agonal_breathing_is_immediate() {
        let classifier = EnsembleClassifier::new(EnsembleConfig::default());
        let reading = make_reading(
            Some((8.0, BreathingType::Agonal)),
            None,
            MovementType::None,
        );

        let result = classifier.classify(&reading);
        assert_eq!(result.recommended_triage, TriageStatus::Immediate);
    }

    #[test]
    fn test_normal_breathing_no_movement_is_delayed() {
        let classifier = EnsembleClassifier::new(EnsembleConfig::default());
        let reading = make_reading(
            Some((16.0, BreathingType::Normal)),
            None,
            MovementType::None,
        );

        let result = classifier.classify(&reading);
        assert_eq!(result.recommended_triage, TriageStatus::Delayed);
    }

    #[test]
    fn test_no_vitals_is_deceased() {
        let mv = MovementProfile::default();
        let mut reading = VitalSignsReading::new(None, None, mv);
        reading.confidence = ConfidenceScore::new(0.5);

        let mut config = EnsembleConfig::default();
        config.min_ensemble_confidence = 0.0;
        let classifier = EnsembleClassifier::new(config);

        let result = classifier.classify(&reading);
        assert_eq!(result.recommended_triage, TriageStatus::Deceased);
    }

    #[test]
    fn test_ensemble_confidence_weighting() {
        let classifier = EnsembleClassifier::new(EnsembleConfig {
            breathing_weight: 0.6,
            heartbeat_weight: 0.3,
            movement_weight: 0.1,
            min_ensemble_confidence: 0.0,
        });

        let reading = make_reading(
            Some((16.0, BreathingType::Normal)),
            Some(72.0),
            MovementType::Periodic,
        );

        let result = classifier.classify(&reading);
        assert!(result.confidence > 0.0);
        assert!(result.breathing_detected);
        assert!(result.heartbeat_detected);
        assert!(result.movement_detected);
    }
}
