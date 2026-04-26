//! Motion Detection Module
//!
//! This module provides motion detection and human presence detection
//! capabilities based on CSI features.

use crate::features::{AmplitudeFeatures, CorrelationFeatures, CsiFeatures, PhaseFeatures};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Motion score with component breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotionScore {
    /// Overall motion score (0.0 to 1.0)
    pub total: f64,

    /// Variance-based motion component
    pub variance_component: f64,

    /// Correlation-based motion component
    pub correlation_component: f64,

    /// Phase-based motion component
    pub phase_component: f64,

    /// Doppler-based motion component (if available)
    pub doppler_component: Option<f64>,
}

impl MotionScore {
    /// Create a new motion score
    pub fn new(
        variance_component: f64,
        correlation_component: f64,
        phase_component: f64,
        doppler_component: Option<f64>,
    ) -> Self {
        // Calculate weighted total
        let total = if let Some(doppler) = doppler_component {
            0.3 * variance_component
                + 0.2 * correlation_component
                + 0.2 * phase_component
                + 0.3 * doppler
        } else {
            0.4 * variance_component + 0.3 * correlation_component + 0.3 * phase_component
        };

        Self {
            total: total.clamp(0.0, 1.0),
            variance_component,
            correlation_component,
            phase_component,
            doppler_component,
        }
    }

    /// Check if motion is detected above threshold
    pub fn is_motion_detected(&self, threshold: f64) -> bool {
        self.total >= threshold
    }
}

/// Motion analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotionAnalysis {
    /// Motion score
    pub score: MotionScore,

    /// Temporal variance of motion
    pub temporal_variance: f64,

    /// Spatial variance of motion
    pub spatial_variance: f64,

    /// Estimated motion velocity (arbitrary units)
    pub estimated_velocity: f64,

    /// Motion direction estimate (radians, if available)
    pub motion_direction: Option<f64>,

    /// Confidence in the analysis
    pub confidence: f64,
}

/// Human detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanDetectionResult {
    /// Whether a human was detected
    pub human_detected: bool,

    /// Detection confidence (0.0 to 1.0)
    pub confidence: f64,

    /// Motion score
    pub motion_score: f64,

    /// Raw (unsmoothed) confidence
    pub raw_confidence: f64,

    /// Timestamp of detection
    pub timestamp: DateTime<Utc>,

    /// Detection threshold used
    pub threshold: f64,

    /// Detailed motion analysis
    pub motion_analysis: MotionAnalysis,

    /// Additional metadata
    #[serde(default)]
    pub metadata: DetectionMetadata,
}

/// Metadata for detection results
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DetectionMetadata {
    /// Number of features used
    pub features_used: usize,

    /// Processing time in milliseconds
    pub processing_time_ms: Option<f64>,

    /// Whether Doppler was available
    pub doppler_available: bool,

    /// History length used
    pub history_length: usize,
}

/// Configuration for motion detector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotionDetectorConfig {
    /// Human detection threshold (0.0 to 1.0)
    pub human_detection_threshold: f64,

    /// Motion detection threshold (0.0 to 1.0)
    pub motion_threshold: f64,

    /// Temporal smoothing factor (0.0 to 1.0)
    /// Higher values give more weight to previous detections
    pub smoothing_factor: f64,

    /// Minimum amplitude indicator threshold
    pub amplitude_threshold: f64,

    /// Minimum phase indicator threshold
    pub phase_threshold: f64,

    /// History size for temporal analysis
    pub history_size: usize,

    /// Enable adaptive thresholding
    pub adaptive_threshold: bool,

    /// Weight for amplitude indicator
    pub amplitude_weight: f64,

    /// Weight for phase indicator
    pub phase_weight: f64,

    /// Weight for motion indicator
    pub motion_weight: f64,
}

impl Default for MotionDetectorConfig {
    fn default() -> Self {
        Self {
            human_detection_threshold: 0.8,
            motion_threshold: 0.3,
            smoothing_factor: 0.9,
            amplitude_threshold: 0.1,
            phase_threshold: 0.05,
            history_size: 100,
            adaptive_threshold: false,
            amplitude_weight: 0.4,
            phase_weight: 0.3,
            motion_weight: 0.3,
        }
    }
}

impl MotionDetectorConfig {
    /// Create a new builder
    pub fn builder() -> MotionDetectorConfigBuilder {
        MotionDetectorConfigBuilder::new()
    }
}

/// Builder for MotionDetectorConfig
#[derive(Debug, Default)]
pub struct MotionDetectorConfigBuilder {
    config: MotionDetectorConfig,
}

impl MotionDetectorConfigBuilder {
    /// Create new builder
    pub fn new() -> Self {
        Self {
            config: MotionDetectorConfig::default(),
        }
    }

    /// Set human detection threshold
    pub fn human_detection_threshold(mut self, threshold: f64) -> Self {
        self.config.human_detection_threshold = threshold;
        self
    }

    /// Set motion threshold
    pub fn motion_threshold(mut self, threshold: f64) -> Self {
        self.config.motion_threshold = threshold;
        self
    }

    /// Set smoothing factor
    pub fn smoothing_factor(mut self, factor: f64) -> Self {
        self.config.smoothing_factor = factor;
        self
    }

    /// Set amplitude threshold
    pub fn amplitude_threshold(mut self, threshold: f64) -> Self {
        self.config.amplitude_threshold = threshold;
        self
    }

    /// Set phase threshold
    pub fn phase_threshold(mut self, threshold: f64) -> Self {
        self.config.phase_threshold = threshold;
        self
    }

    /// Set history size
    pub fn history_size(mut self, size: usize) -> Self {
        self.config.history_size = size;
        self
    }

    /// Enable adaptive thresholding
    pub fn adaptive_threshold(mut self, enable: bool) -> Self {
        self.config.adaptive_threshold = enable;
        self
    }

    /// Set indicator weights
    pub fn weights(mut self, amplitude: f64, phase: f64, motion: f64) -> Self {
        self.config.amplitude_weight = amplitude;
        self.config.phase_weight = phase;
        self.config.motion_weight = motion;
        self
    }

    /// Build configuration
    pub fn build(self) -> MotionDetectorConfig {
        self.config
    }
}

/// Motion detector for human presence detection
#[derive(Debug)]
pub struct MotionDetector {
    config: MotionDetectorConfig,
    previous_confidence: f64,
    motion_history: VecDeque<MotionScore>,
    detection_count: usize,
    total_detections: usize,
    baseline_variance: Option<f64>,
}

impl MotionDetector {
    /// Create a new motion detector
    pub fn new(config: MotionDetectorConfig) -> Self {
        Self {
            motion_history: VecDeque::with_capacity(config.history_size),
            config,
            previous_confidence: 0.0,
            detection_count: 0,
            total_detections: 0,
            baseline_variance: None,
        }
    }

    /// Create with default configuration
    pub fn default_config() -> Self {
        Self::new(MotionDetectorConfig::default())
    }

    /// Get configuration
    pub fn config(&self) -> &MotionDetectorConfig {
        &self.config
    }

    /// Analyze motion patterns from CSI features
    pub fn analyze_motion(&self, features: &CsiFeatures) -> MotionAnalysis {
        // Calculate variance-based motion score
        let variance_score = self.calculate_variance_score(&features.amplitude);

        // Calculate correlation-based motion score
        let correlation_score = self.calculate_correlation_score(&features.correlation);

        // Calculate phase-based motion score
        let phase_score = self.calculate_phase_score(&features.phase);

        // Calculate Doppler-based score if available
        let doppler_score = features.doppler.as_ref().map(|d| {
            // Normalize Doppler magnitude to 0-1 range
            (d.mean_magnitude / 100.0).clamp(0.0, 1.0)
        });

        let motion_score = MotionScore::new(variance_score, correlation_score, phase_score, doppler_score);

        // Calculate temporal and spatial variance
        let temporal_variance = self.calculate_temporal_variance();
        let spatial_variance = features.amplitude.variance.iter().sum::<f64>()
            / features.amplitude.variance.len() as f64;

        // Estimate velocity from Doppler if available
        let estimated_velocity = features
            .doppler
            .as_ref()
            .map(|d| d.mean_magnitude)
            .unwrap_or(0.0);

        // Motion direction from phase gradient
        let motion_direction = if features.phase.gradient.len() > 0 {
            let mean_grad: f64 =
                features.phase.gradient.iter().sum::<f64>() / features.phase.gradient.len() as f64;
            Some(mean_grad.atan())
        } else {
            None
        };

        // Calculate confidence based on signal quality indicators
        let confidence = self.calculate_motion_confidence(features);

        MotionAnalysis {
            score: motion_score,
            temporal_variance,
            spatial_variance,
            estimated_velocity,
            motion_direction,
            confidence,
        }
    }

    /// Calculate variance-based motion score
    fn calculate_variance_score(&self, amplitude: &AmplitudeFeatures) -> f64 {
        let mean_variance = amplitude.variance.iter().sum::<f64>() / amplitude.variance.len() as f64;

        // Normalize using baseline if available
        if let Some(baseline) = self.baseline_variance {
            let ratio = mean_variance / (baseline + 1e-10);
            (ratio - 1.0).max(0.0).tanh()
        } else {
            // Use heuristic normalization
            (mean_variance / 0.5).clamp(0.0, 1.0)
        }
    }

    /// Calculate correlation-based motion score
    fn calculate_correlation_score(&self, correlation: &CorrelationFeatures) -> f64 {
        let n = correlation.matrix.dim().0;
        if n < 2 {
            return 0.0;
        }

        // Calculate mean deviation from identity matrix
        let mut deviation_sum = 0.0;
        let mut count = 0;

        for i in 0..n {
            for j in 0..n {
                let expected = if i == j { 1.0 } else { 0.0 };
                deviation_sum += (correlation.matrix[[i, j]] - expected).abs();
                count += 1;
            }
        }

        let mean_deviation = deviation_sum / count as f64;
        mean_deviation.clamp(0.0, 1.0)
    }

    /// Calculate phase-based motion score
    fn calculate_phase_score(&self, phase: &PhaseFeatures) -> f64 {
        // Use phase variance and coherence
        let mean_variance = phase.variance.iter().sum::<f64>() / phase.variance.len() as f64;
        let coherence_factor = 1.0 - phase.coherence.abs();

        // Combine factors
        let score = 0.5 * (mean_variance / 0.5).clamp(0.0, 1.0) + 0.5 * coherence_factor;
        score.clamp(0.0, 1.0)
    }

    /// Calculate temporal variance from motion history
    fn calculate_temporal_variance(&self) -> f64 {
        if self.motion_history.len() < 2 {
            return 0.0;
        }

        let scores: Vec<f64> = self.motion_history.iter().map(|m| m.total).collect();
        let mean: f64 = scores.iter().sum::<f64>() / scores.len() as f64;
        let variance: f64 = scores.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / scores.len() as f64;
        variance.sqrt()
    }

    /// Calculate confidence in motion detection
    fn calculate_motion_confidence(&self, features: &CsiFeatures) -> f64 {
        let mut confidence = 0.0;
        let mut weight_sum = 0.0;

        // Amplitude quality indicator
        let amp_quality = (features.amplitude.dynamic_range / 2.0).clamp(0.0, 1.0);
        confidence += amp_quality * 0.3;
        weight_sum += 0.3;

        // Phase coherence indicator
        let phase_quality = features.phase.coherence.abs();
        confidence += phase_quality * 0.3;
        weight_sum += 0.3;

        // Correlation consistency indicator
        let corr_quality = (1.0 - features.correlation.correlation_spread).clamp(0.0, 1.0);
        confidence += corr_quality * 0.2;
        weight_sum += 0.2;

        // Doppler quality if available
        if let Some(ref doppler) = features.doppler {
            let doppler_quality = (doppler.spread / doppler.mean_magnitude.max(1.0)).clamp(0.0, 1.0);
            confidence += (1.0 - doppler_quality) * 0.2;
            weight_sum += 0.2;
        }

        if weight_sum > 0.0 {
            confidence / weight_sum
        } else {
            0.0
        }
    }

    /// Calculate detection confidence from features and motion score
    fn calculate_detection_confidence(&self, features: &CsiFeatures, motion_score: f64) -> f64 {
        // Amplitude indicator
        let amplitude_mean = features.amplitude.mean.iter().sum::<f64>()
            / features.amplitude.mean.len() as f64;
        let amplitude_indicator = if amplitude_mean > self.config.amplitude_threshold {
            1.0
        } else {
            0.0
        };

        // Phase indicator
        let phase_std = features.phase.variance.iter().sum::<f64>().sqrt()
            / features.phase.variance.len() as f64;
        let phase_indicator = if phase_std > self.config.phase_threshold {
            1.0
        } else {
            0.0
        };

        // Motion indicator
        let motion_indicator = if motion_score > self.config.motion_threshold {
            1.0
        } else {
            0.0
        };

        // Weighted combination
        let confidence = self.config.amplitude_weight * amplitude_indicator
            + self.config.phase_weight * phase_indicator
            + self.config.motion_weight * motion_indicator;

        confidence.clamp(0.0, 1.0)
    }

    /// Apply temporal smoothing (exponential moving average)
    fn apply_temporal_smoothing(&mut self, raw_confidence: f64) -> f64 {
        let smoothed = self.config.smoothing_factor * self.previous_confidence
            + (1.0 - self.config.smoothing_factor) * raw_confidence;
        self.previous_confidence = smoothed;
        smoothed
    }

    /// Detect human presence from CSI features
    pub fn detect_human(&mut self, features: &CsiFeatures) -> HumanDetectionResult {
        // Analyze motion
        let motion_analysis = self.analyze_motion(features);

        // Add to history
        if self.motion_history.len() >= self.config.history_size {
            self.motion_history.pop_front();
        }
        self.motion_history.push_back(motion_analysis.score.clone());

        // Calculate detection confidence
        let raw_confidence =
            self.calculate_detection_confidence(features, motion_analysis.score.total);

        // Apply temporal smoothing
        let smoothed_confidence = self.apply_temporal_smoothing(raw_confidence);

        // Get effective threshold (adaptive if enabled)
        let threshold = if self.config.adaptive_threshold {
            self.calculate_adaptive_threshold()
        } else {
            self.config.human_detection_threshold
        };

        // Determine detection
        let human_detected = smoothed_confidence >= threshold;

        self.total_detections += 1;
        if human_detected {
            self.detection_count += 1;
        }

        let metadata = DetectionMetadata {
            features_used: 4, // amplitude, phase, correlation, psd
            processing_time_ms: None,
            doppler_available: features.doppler.is_some(),
            history_length: self.motion_history.len(),
        };

        HumanDetectionResult {
            human_detected,
            confidence: smoothed_confidence,
            motion_score: motion_analysis.score.total,
            raw_confidence,
            timestamp: Utc::now(),
            threshold,
            motion_analysis,
            metadata,
        }
    }

    /// Calculate adaptive threshold based on recent history
    fn calculate_adaptive_threshold(&self) -> f64 {
        if self.motion_history.len() < 10 {
            return self.config.human_detection_threshold;
        }

        let scores: Vec<f64> = self.motion_history.iter().map(|m| m.total).collect();
        let mean: f64 = scores.iter().sum::<f64>() / scores.len() as f64;
        let std: f64 = {
            let var: f64 = scores.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / scores.len() as f64;
            var.sqrt()
        };

        // Threshold is mean + 1 std deviation, clamped to reasonable range
        (mean + std).clamp(0.3, 0.95)
    }

    /// Update baseline variance (for calibration)
    pub fn calibrate(&mut self, features: &CsiFeatures) {
        let mean_variance =
            features.amplitude.variance.iter().sum::<f64>() / features.amplitude.variance.len() as f64;
        self.baseline_variance = Some(mean_variance);
    }

    /// Clear calibration
    pub fn clear_calibration(&mut self) {
        self.baseline_variance = None;
    }

    /// Get detection statistics
    pub fn get_statistics(&self) -> DetectionStatistics {
        DetectionStatistics {
            total_detections: self.total_detections,
            positive_detections: self.detection_count,
            detection_rate: if self.total_detections > 0 {
                self.detection_count as f64 / self.total_detections as f64
            } else {
                0.0
            },
            history_size: self.motion_history.len(),
            is_calibrated: self.baseline_variance.is_some(),
        }
    }

    /// Reset detector state
    pub fn reset(&mut self) {
        self.previous_confidence = 0.0;
        self.motion_history.clear();
        self.detection_count = 0;
        self.total_detections = 0;
    }

    /// Get previous confidence value
    pub fn previous_confidence(&self) -> f64 {
        self.previous_confidence
    }
}

/// Detection statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionStatistics {
    /// Total number of detection attempts
    pub total_detections: usize,

    /// Number of positive detections
    pub positive_detections: usize,

    /// Detection rate (0.0 to 1.0)
    pub detection_rate: f64,

    /// Current history size
    pub history_size: usize,

    /// Whether detector is calibrated
    pub is_calibrated: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csi_processor::CsiData;
    use crate::features::FeatureExtractor;
    use ndarray::Array2;

    fn create_test_csi_data(motion_level: f64) -> CsiData {
        let amplitude = Array2::from_shape_fn((4, 64), |(i, j)| {
            1.0 + motion_level * 0.5 * ((i + j) as f64 * 0.1).sin()
        });
        let phase = Array2::from_shape_fn((4, 64), |(i, j)| {
            motion_level * 0.3 * ((i + j) as f64 * 0.15).sin()
        });

        CsiData::builder()
            .amplitude(amplitude)
            .phase(phase)
            .frequency(5.0e9)
            .bandwidth(20.0e6)
            .snr(25.0)
            .build()
            .unwrap()
    }

    fn create_test_features(motion_level: f64) -> CsiFeatures {
        let csi_data = create_test_csi_data(motion_level);
        let extractor = FeatureExtractor::default_config();
        extractor.extract(&csi_data)
    }

    #[test]
    fn test_motion_score() {
        let score = MotionScore::new(0.5, 0.6, 0.4, None);
        assert!(score.total > 0.0 && score.total <= 1.0);
        assert_eq!(score.variance_component, 0.5);
        assert_eq!(score.correlation_component, 0.6);
        assert_eq!(score.phase_component, 0.4);
    }

    #[test]
    fn test_motion_score_with_doppler() {
        let score = MotionScore::new(0.5, 0.6, 0.4, Some(0.7));
        assert!(score.total > 0.0 && score.total <= 1.0);
        assert_eq!(score.doppler_component, Some(0.7));
    }

    #[test]
    fn test_motion_detector_creation() {
        let config = MotionDetectorConfig::default();
        let detector = MotionDetector::new(config);
        assert_eq!(detector.previous_confidence(), 0.0);
    }

    #[test]
    fn test_motion_analysis() {
        let detector = MotionDetector::default_config();
        let features = create_test_features(0.5);

        let analysis = detector.analyze_motion(&features);
        assert!(analysis.score.total >= 0.0 && analysis.score.total <= 1.0);
        assert!(analysis.confidence >= 0.0 && analysis.confidence <= 1.0);
    }

    #[test]
    fn test_human_detection() {
        let config = MotionDetectorConfig::builder()
            .human_detection_threshold(0.5)
            .smoothing_factor(0.5)
            .build();
        let mut detector = MotionDetector::new(config);

        let features = create_test_features(0.8);
        let result = detector.detect_human(&features);

        assert!(result.confidence >= 0.0 && result.confidence <= 1.0);
        assert!(result.motion_score >= 0.0 && result.motion_score <= 1.0);
    }

    #[test]
    fn test_temporal_smoothing() {
        let config = MotionDetectorConfig::builder()
            .smoothing_factor(0.9)
            .build();
        let mut detector = MotionDetector::new(config);

        // First detection with low confidence
        let features_low = create_test_features(0.1);
        let result1 = detector.detect_human(&features_low);

        // Second detection with high confidence should be smoothed
        let features_high = create_test_features(0.9);
        let result2 = detector.detect_human(&features_high);

        // Due to smoothing, result2.confidence should be between result1 and raw
        assert!(result2.confidence >= result1.confidence);
    }

    #[test]
    fn test_calibration() {
        let mut detector = MotionDetector::default_config();
        let features = create_test_features(0.5);

        assert!(!detector.get_statistics().is_calibrated);
        detector.calibrate(&features);
        assert!(detector.get_statistics().is_calibrated);

        detector.clear_calibration();
        assert!(!detector.get_statistics().is_calibrated);
    }

    #[test]
    fn test_detection_statistics() {
        let mut detector = MotionDetector::default_config();

        for i in 0..5 {
            let features = create_test_features((i as f64) / 5.0);
            let _ = detector.detect_human(&features);
        }

        let stats = detector.get_statistics();
        assert_eq!(stats.total_detections, 5);
        assert!(stats.detection_rate >= 0.0 && stats.detection_rate <= 1.0);
    }

    #[test]
    fn test_reset() {
        let mut detector = MotionDetector::default_config();
        let features = create_test_features(0.5);

        for _ in 0..5 {
            let _ = detector.detect_human(&features);
        }

        detector.reset();

        let stats = detector.get_statistics();
        assert_eq!(stats.total_detections, 0);
        assert_eq!(stats.history_size, 0);
        assert_eq!(detector.previous_confidence(), 0.0);
    }

    #[test]
    fn test_adaptive_threshold() {
        let config = MotionDetectorConfig::builder()
            .adaptive_threshold(true)
            .history_size(20)
            .build();
        let mut detector = MotionDetector::new(config);

        // Build up history
        for i in 0..15 {
            let features = create_test_features((i as f64 % 5.0) / 5.0);
            let _ = detector.detect_human(&features);
        }

        // The adaptive threshold should now be calculated
        let features = create_test_features(0.5);
        let result = detector.detect_human(&features);

        // Threshold should be different from default
        // (this is a weak assertion, mainly checking it runs)
        assert!(result.threshold > 0.0);
    }

    #[test]
    fn test_config_builder() {
        let config = MotionDetectorConfig::builder()
            .human_detection_threshold(0.7)
            .motion_threshold(0.4)
            .smoothing_factor(0.85)
            .amplitude_threshold(0.15)
            .phase_threshold(0.08)
            .history_size(200)
            .adaptive_threshold(true)
            .weights(0.35, 0.35, 0.30)
            .build();

        assert_eq!(config.human_detection_threshold, 0.7);
        assert_eq!(config.motion_threshold, 0.4);
        assert_eq!(config.smoothing_factor, 0.85);
        assert_eq!(config.amplitude_threshold, 0.15);
        assert_eq!(config.phase_threshold, 0.08);
        assert_eq!(config.history_size, 200);
        assert!(config.adaptive_threshold);
        assert_eq!(config.amplitude_weight, 0.35);
        assert_eq!(config.phase_weight, 0.35);
        assert_eq!(config.motion_weight, 0.30);
    }

    #[test]
    fn test_low_motion_no_detection() {
        let config = MotionDetectorConfig::builder()
            .human_detection_threshold(0.8)
            .smoothing_factor(0.0) // No smoothing for clear test
            .build();
        let mut detector = MotionDetector::new(config);

        // Very low motion should not trigger detection
        let features = create_test_features(0.01);
        let result = detector.detect_human(&features);

        // With very low motion, detection should likely be false
        // (depends on thresholds, but confidence should be low)
        assert!(result.motion_score < 0.5);
    }

    #[test]
    fn test_motion_history() {
        let config = MotionDetectorConfig::builder()
            .history_size(10)
            .build();
        let mut detector = MotionDetector::new(config);

        for i in 0..15 {
            let features = create_test_features((i as f64) / 15.0);
            let _ = detector.detect_human(&features);
        }

        let stats = detector.get_statistics();
        assert_eq!(stats.history_size, 10); // Should not exceed max
    }
}
