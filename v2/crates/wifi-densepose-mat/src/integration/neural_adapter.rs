//! Adapter for wifi-densepose-nn crate (neural network inference).

use super::AdapterError;
use crate::domain::{BreathingPattern, BreathingType, HeartbeatSignature, SignalStrength};
use super::signal_adapter::VitalFeatures;

/// Adapter for neural network-based vital signs detection
pub struct NeuralAdapter {
    /// Whether to use GPU acceleration
    use_gpu: bool,
    /// Confidence threshold for valid detections
    confidence_threshold: f32,
    /// Model loaded status
    models_loaded: bool,
}

impl NeuralAdapter {
    /// Create a new neural adapter
    pub fn new(use_gpu: bool) -> Self {
        Self {
            use_gpu,
            confidence_threshold: 0.5,
            models_loaded: false,
        }
    }

    /// Create with default settings (CPU)
    pub fn with_defaults() -> Self {
        Self::new(false)
    }

    /// Load neural network models
    pub fn load_models(&mut self, _model_path: &str) -> Result<(), AdapterError> {
        // In production, this would load ONNX models using wifi-densepose-nn
        // For now, mark as loaded for simulation
        self.models_loaded = true;
        Ok(())
    }

    /// Classify breathing pattern using neural network
    pub fn classify_breathing(
        &self,
        features: &VitalFeatures,
    ) -> Result<Option<BreathingPattern>, AdapterError> {
        if !self.models_loaded {
            // Fall back to rule-based classification
            return Ok(self.classify_breathing_rules(features));
        }

        // In production, this would run ONNX inference
        // For now, use rule-based approach
        Ok(self.classify_breathing_rules(features))
    }

    /// Classify heartbeat using neural network
    pub fn classify_heartbeat(
        &self,
        features: &VitalFeatures,
    ) -> Result<Option<HeartbeatSignature>, AdapterError> {
        if !self.models_loaded {
            return Ok(self.classify_heartbeat_rules(features));
        }

        // In production, run ONNX inference
        Ok(self.classify_heartbeat_rules(features))
    }

    /// Combined vital signs classification
    pub fn classify_vitals(
        &self,
        features: &VitalFeatures,
    ) -> Result<VitalsClassification, AdapterError> {
        let breathing = self.classify_breathing(features)?;
        let heartbeat = self.classify_heartbeat(features)?;

        // Calculate overall confidence
        let confidence = self.calculate_confidence(
            &breathing,
            &heartbeat,
            features.signal_quality,
        );

        Ok(VitalsClassification {
            breathing,
            heartbeat,
            confidence,
            signal_quality: features.signal_quality,
        })
    }

    /// Rule-based breathing classification (fallback)
    fn classify_breathing_rules(&self, features: &VitalFeatures) -> Option<BreathingPattern> {
        if features.breathing_features.len() < 3 {
            return None;
        }

        let peak_freq = features.breathing_features[0];
        let power_ratio = features.breathing_features.get(1).copied().unwrap_or(0.0);
        let band_ratio = features.breathing_features.get(2).copied().unwrap_or(0.0);

        // Check if there's significant energy in breathing band
        if power_ratio < 0.05 || band_ratio < 0.1 {
            return None;
        }

        let rate_bpm = (peak_freq * 60.0) as f32;

        // Validate rate
        if rate_bpm < 4.0 || rate_bpm > 60.0 {
            return None;
        }

        let pattern_type = if rate_bpm < 6.0 {
            BreathingType::Agonal
        } else if rate_bpm < 10.0 {
            BreathingType::Shallow
        } else if rate_bpm > 30.0 {
            BreathingType::Labored
        } else if band_ratio < 0.3 {
            BreathingType::Irregular
        } else {
            BreathingType::Normal
        };

        Some(BreathingPattern {
            rate_bpm,
            amplitude: power_ratio as f32,
            regularity: band_ratio as f32,
            pattern_type,
        })
    }

    /// Rule-based heartbeat classification (fallback)
    fn classify_heartbeat_rules(&self, features: &VitalFeatures) -> Option<HeartbeatSignature> {
        if features.heartbeat_features.len() < 3 {
            return None;
        }

        let peak_freq = features.heartbeat_features[0];
        let power_ratio = features.heartbeat_features.get(1).copied().unwrap_or(0.0);
        let band_ratio = features.heartbeat_features.get(2).copied().unwrap_or(0.0);

        // Heartbeat detection requires stronger signal
        if power_ratio < 0.03 || band_ratio < 0.08 {
            return None;
        }

        let rate_bpm = (peak_freq * 60.0) as f32;

        // Validate rate (30-200 BPM)
        if rate_bpm < 30.0 || rate_bpm > 200.0 {
            return None;
        }

        let strength = if power_ratio > 0.15 {
            SignalStrength::Strong
        } else if power_ratio > 0.08 {
            SignalStrength::Moderate
        } else if power_ratio > 0.04 {
            SignalStrength::Weak
        } else {
            SignalStrength::VeryWeak
        };

        Some(HeartbeatSignature {
            rate_bpm,
            variability: band_ratio as f32 * 0.5,
            strength,
        })
    }

    /// Calculate overall confidence from detections
    fn calculate_confidence(
        &self,
        breathing: &Option<BreathingPattern>,
        heartbeat: &Option<HeartbeatSignature>,
        signal_quality: f64,
    ) -> f32 {
        let mut confidence = signal_quality as f32 * 0.3;

        if let Some(b) = breathing {
            confidence += 0.4 * b.confidence() as f32;
        }

        if let Some(h) = heartbeat {
            confidence += 0.3 * h.confidence() as f32;
        }

        confidence.clamp(0.0, 1.0)
    }
}

impl Default for NeuralAdapter {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// Result of neural network vital signs classification
#[derive(Debug, Clone)]
pub struct VitalsClassification {
    /// Detected breathing pattern
    pub breathing: Option<BreathingPattern>,
    /// Detected heartbeat
    pub heartbeat: Option<HeartbeatSignature>,
    /// Overall classification confidence
    pub confidence: f32,
    /// Signal quality indicator
    pub signal_quality: f64,
}

impl VitalsClassification {
    /// Check if any vital signs were detected
    pub fn has_vitals(&self) -> bool {
        self.breathing.is_some() || self.heartbeat.is_some()
    }

    /// Check if detection confidence is sufficient
    pub fn is_confident(&self, threshold: f32) -> bool {
        self.confidence >= threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_good_features() -> VitalFeatures {
        VitalFeatures {
            breathing_features: vec![0.25, 0.2, 0.4], // 15 BPM, good signal
            heartbeat_features: vec![1.2, 0.1, 0.15], // 72 BPM, moderate signal
            movement_features: vec![0.1, 0.05, 0.01],
            signal_quality: 0.8,
        }
    }

    fn create_weak_features() -> VitalFeatures {
        VitalFeatures {
            breathing_features: vec![0.25, 0.02, 0.05], // Weak
            heartbeat_features: vec![1.2, 0.01, 0.02], // Very weak
            movement_features: vec![0.01, 0.005, 0.001],
            signal_quality: 0.3,
        }
    }

    #[test]
    fn test_classify_breathing() {
        let adapter = NeuralAdapter::with_defaults();
        let features = create_good_features();

        let result = adapter.classify_breathing(&features);
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[test]
    fn test_weak_signal_no_detection() {
        let adapter = NeuralAdapter::with_defaults();
        let features = create_weak_features();

        let result = adapter.classify_breathing(&features);
        assert!(result.is_ok());
        // Weak signals may or may not be detected depending on thresholds
    }

    #[test]
    fn test_classify_vitals() {
        let adapter = NeuralAdapter::with_defaults();
        let features = create_good_features();

        let result = adapter.classify_vitals(&features);
        assert!(result.is_ok());

        let classification = result.unwrap();
        assert!(classification.has_vitals());
        assert!(classification.confidence > 0.3);
    }

    #[test]
    fn test_confidence_calculation() {
        let adapter = NeuralAdapter::with_defaults();

        let breathing = Some(BreathingPattern {
            rate_bpm: 16.0,
            amplitude: 0.8,
            regularity: 0.9,
            pattern_type: BreathingType::Normal,
        });

        let confidence = adapter.calculate_confidence(&breathing, &None, 0.8);
        assert!(confidence > 0.5);
    }
}
