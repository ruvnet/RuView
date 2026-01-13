//! Detection pipeline combining all vital signs detectors.
//!
//! This module provides both traditional signal-processing-based detection
//! and optional ML-enhanced detection for improved accuracy.

use crate::domain::{ScanZone, VitalSignsReading, ConfidenceScore};
use crate::ml::{MlDetectionConfig, MlDetectionPipeline, MlDetectionResult};
use crate::{DisasterConfig, MatError};
use super::{
    BreathingDetector, BreathingDetectorConfig,
    HeartbeatDetector, HeartbeatDetectorConfig,
    MovementClassifier, MovementClassifierConfig,
};

/// Configuration for the detection pipeline
#[derive(Debug, Clone)]
pub struct DetectionConfig {
    /// Breathing detector configuration
    pub breathing: BreathingDetectorConfig,
    /// Heartbeat detector configuration
    pub heartbeat: HeartbeatDetectorConfig,
    /// Movement classifier configuration
    pub movement: MovementClassifierConfig,
    /// Sample rate of CSI data (Hz)
    pub sample_rate: f64,
    /// Whether to enable heartbeat detection (slower, more processing)
    pub enable_heartbeat: bool,
    /// Minimum overall confidence to report detection
    pub min_confidence: f64,
    /// Enable ML-enhanced detection
    pub enable_ml: bool,
    /// ML detection configuration (if enabled)
    pub ml_config: Option<MlDetectionConfig>,
}

impl Default for DetectionConfig {
    fn default() -> Self {
        Self {
            breathing: BreathingDetectorConfig::default(),
            heartbeat: HeartbeatDetectorConfig::default(),
            movement: MovementClassifierConfig::default(),
            sample_rate: 1000.0,
            enable_heartbeat: false,
            min_confidence: 0.3,
            enable_ml: false,
            ml_config: None,
        }
    }
}

impl DetectionConfig {
    /// Create configuration from disaster config
    pub fn from_disaster_config(config: &DisasterConfig) -> Self {
        let mut detection_config = Self::default();

        // Adjust sensitivity
        detection_config.breathing.confidence_threshold = (1.0 - config.sensitivity) as f32 * 0.5;
        detection_config.heartbeat.confidence_threshold = (1.0 - config.sensitivity) as f32 * 0.5;
        detection_config.min_confidence = 1.0 - config.sensitivity * 0.7;

        // Enable heartbeat for high sensitivity
        detection_config.enable_heartbeat = config.sensitivity > 0.7;

        detection_config
    }

    /// Enable ML-enhanced detection with the given configuration
    pub fn with_ml(mut self, ml_config: MlDetectionConfig) -> Self {
        self.enable_ml = true;
        self.ml_config = Some(ml_config);
        self
    }

    /// Enable ML-enhanced detection with default configuration
    pub fn with_default_ml(mut self) -> Self {
        self.enable_ml = true;
        self.ml_config = Some(MlDetectionConfig::default());
        self
    }
}

/// Trait for vital signs detection
pub trait VitalSignsDetector: Send + Sync {
    /// Process CSI data and detect vital signs
    fn detect(&self, csi_data: &CsiDataBuffer) -> Option<VitalSignsReading>;
}

/// Buffer for CSI data samples
#[derive(Debug, Default)]
pub struct CsiDataBuffer {
    /// Amplitude samples
    pub amplitudes: Vec<f64>,
    /// Phase samples (unwrapped)
    pub phases: Vec<f64>,
    /// Sample timestamps
    pub timestamps: Vec<f64>,
    /// Sample rate
    pub sample_rate: f64,
}

impl CsiDataBuffer {
    /// Create a new buffer
    pub fn new(sample_rate: f64) -> Self {
        Self {
            amplitudes: Vec::new(),
            phases: Vec::new(),
            timestamps: Vec::new(),
            sample_rate,
        }
    }

    /// Add samples to the buffer
    pub fn add_samples(&mut self, amplitudes: &[f64], phases: &[f64]) {
        self.amplitudes.extend(amplitudes);
        self.phases.extend(phases);

        // Generate timestamps
        let start = self.timestamps.last().copied().unwrap_or(0.0);
        let dt = 1.0 / self.sample_rate;
        for i in 0..amplitudes.len() {
            self.timestamps.push(start + (i + 1) as f64 * dt);
        }
    }

    /// Clear the buffer
    pub fn clear(&mut self) {
        self.amplitudes.clear();
        self.phases.clear();
        self.timestamps.clear();
    }

    /// Get the duration of data in the buffer (seconds)
    pub fn duration(&self) -> f64 {
        self.amplitudes.len() as f64 / self.sample_rate
    }

    /// Check if buffer has enough data for analysis
    pub fn has_sufficient_data(&self, min_duration: f64) -> bool {
        self.duration() >= min_duration
    }
}

/// Detection pipeline that combines all detectors
pub struct DetectionPipeline {
    config: DetectionConfig,
    breathing_detector: BreathingDetector,
    heartbeat_detector: HeartbeatDetector,
    movement_classifier: MovementClassifier,
    data_buffer: parking_lot::RwLock<CsiDataBuffer>,
    /// Optional ML detection pipeline
    ml_pipeline: Option<MlDetectionPipeline>,
}

impl DetectionPipeline {
    /// Create a new detection pipeline
    pub fn new(config: DetectionConfig) -> Self {
        let ml_pipeline = if config.enable_ml {
            config.ml_config.clone().map(MlDetectionPipeline::new)
        } else {
            None
        };

        Self {
            breathing_detector: BreathingDetector::new(config.breathing.clone()),
            heartbeat_detector: HeartbeatDetector::new(config.heartbeat.clone()),
            movement_classifier: MovementClassifier::new(config.movement.clone()),
            data_buffer: parking_lot::RwLock::new(CsiDataBuffer::new(config.sample_rate)),
            ml_pipeline,
            config,
        }
    }

    /// Initialize ML models asynchronously (if enabled)
    pub async fn initialize_ml(&mut self) -> Result<(), MatError> {
        if let Some(ref mut ml) = self.ml_pipeline {
            ml.initialize().await.map_err(MatError::from)?;
        }
        Ok(())
    }

    /// Check if ML pipeline is ready
    pub fn ml_ready(&self) -> bool {
        self.ml_pipeline.as_ref().map_or(true, |ml| ml.is_ready())
    }

    /// Process a scan zone and return detected vital signs
    pub async fn process_zone(&self, zone: &ScanZone) -> Result<Option<VitalSignsReading>, MatError> {
        // In a real implementation, this would:
        // 1. Collect CSI data from sensors in the zone
        // 2. Preprocess the data
        // 3. Run detection algorithms

        // For now, check if we have buffered data
        let buffer = self.data_buffer.read();

        if !buffer.has_sufficient_data(5.0) {
            // Need at least 5 seconds of data
            return Ok(None);
        }

        // Detect vital signs using traditional pipeline
        let reading = self.detect_from_buffer(&buffer, zone)?;

        // If ML is enabled and ready, enhance with ML predictions
        let enhanced_reading = if self.config.enable_ml && self.ml_ready() {
            self.enhance_with_ml(reading, &buffer).await?
        } else {
            reading
        };

        // Check minimum confidence
        if let Some(ref r) = enhanced_reading {
            if r.confidence.value() < self.config.min_confidence {
                return Ok(None);
            }
        }

        Ok(enhanced_reading)
    }

    /// Enhance detection results with ML predictions
    async fn enhance_with_ml(
        &self,
        traditional_reading: Option<VitalSignsReading>,
        buffer: &CsiDataBuffer,
    ) -> Result<Option<VitalSignsReading>, MatError> {
        let ml_pipeline = match &self.ml_pipeline {
            Some(ml) => ml,
            None => return Ok(traditional_reading),
        };

        // Get ML predictions
        let ml_result = ml_pipeline.process(buffer).await.map_err(MatError::from)?;

        // If we have ML vital classification, use it to enhance or replace traditional
        if let Some(ref ml_vital) = ml_result.vital_classification {
            if let Some(vital_reading) = ml_vital.to_vital_signs_reading() {
                // If ML result has higher confidence, prefer it
                if let Some(ref traditional) = traditional_reading {
                    if ml_result.overall_confidence() > traditional.confidence.value() as f32 {
                        return Ok(Some(vital_reading));
                    }
                } else {
                    // No traditional reading, use ML result
                    return Ok(Some(vital_reading));
                }
            }
        }

        Ok(traditional_reading)
    }

    /// Get the latest ML detection results (if ML is enabled)
    pub async fn get_ml_results(&self) -> Option<MlDetectionResult> {
        let buffer = self.data_buffer.read();
        if let Some(ref ml) = self.ml_pipeline {
            ml.process(&buffer).await.ok()
        } else {
            None
        }
    }

    /// Add CSI data to the processing buffer
    pub fn add_data(&self, amplitudes: &[f64], phases: &[f64]) {
        let mut buffer = self.data_buffer.write();
        buffer.add_samples(amplitudes, phases);

        // Keep only recent data (last 30 seconds)
        let max_samples = (30.0 * self.config.sample_rate) as usize;
        if buffer.amplitudes.len() > max_samples {
            let drain_count = buffer.amplitudes.len() - max_samples;
            buffer.amplitudes.drain(0..drain_count);
            buffer.phases.drain(0..drain_count);
            buffer.timestamps.drain(0..drain_count);
        }
    }

    /// Clear the data buffer
    pub fn clear_buffer(&self) {
        self.data_buffer.write().clear();
    }

    /// Detect vital signs from buffered data
    fn detect_from_buffer(
        &self,
        buffer: &CsiDataBuffer,
        _zone: &ScanZone,
    ) -> Result<Option<VitalSignsReading>, MatError> {
        // Detect breathing
        let breathing = self.breathing_detector.detect(
            &buffer.amplitudes,
            buffer.sample_rate,
        );

        // Detect heartbeat (if enabled)
        let heartbeat = if self.config.enable_heartbeat {
            let breathing_rate = breathing.as_ref().map(|b| b.rate_bpm as f64);
            self.heartbeat_detector.detect(
                &buffer.phases,
                buffer.sample_rate,
                breathing_rate,
            )
        } else {
            None
        };

        // Classify movement
        let movement = self.movement_classifier.classify(
            &buffer.amplitudes,
            buffer.sample_rate,
        );

        // Check if we detected anything
        if breathing.is_none() && heartbeat.is_none() && movement.movement_type == crate::domain::MovementType::None {
            return Ok(None);
        }

        // Create vital signs reading
        let reading = VitalSignsReading::new(breathing, heartbeat, movement);

        Ok(Some(reading))
    }

    /// Get configuration
    pub fn config(&self) -> &DetectionConfig {
        &self.config
    }

    /// Update configuration
    pub fn update_config(&mut self, config: DetectionConfig) {
        self.breathing_detector = BreathingDetector::new(config.breathing.clone());
        self.heartbeat_detector = HeartbeatDetector::new(config.heartbeat.clone());
        self.movement_classifier = MovementClassifier::new(config.movement.clone());

        // Update ML pipeline if configuration changed
        if config.enable_ml != self.config.enable_ml || config.ml_config != self.config.ml_config {
            self.ml_pipeline = if config.enable_ml {
                config.ml_config.clone().map(MlDetectionPipeline::new)
            } else {
                None
            };
        }

        self.config = config;
    }

    /// Get the ML pipeline (if enabled)
    pub fn ml_pipeline(&self) -> Option<&MlDetectionPipeline> {
        self.ml_pipeline.as_ref()
    }
}

impl VitalSignsDetector for DetectionPipeline {
    fn detect(&self, csi_data: &CsiDataBuffer) -> Option<VitalSignsReading> {
        // Detect breathing from amplitude variations
        let breathing = self.breathing_detector.detect(
            &csi_data.amplitudes,
            csi_data.sample_rate,
        );

        // Detect heartbeat from phase variations
        let heartbeat = if self.config.enable_heartbeat {
            let breathing_rate = breathing.as_ref().map(|b| b.rate_bpm as f64);
            self.heartbeat_detector.detect(
                &csi_data.phases,
                csi_data.sample_rate,
                breathing_rate,
            )
        } else {
            None
        };

        // Classify movement
        let movement = self.movement_classifier.classify(
            &csi_data.amplitudes,
            csi_data.sample_rate,
        );

        // Create reading if we detected anything
        if breathing.is_some() || heartbeat.is_some()
            || movement.movement_type != crate::domain::MovementType::None
        {
            Some(VitalSignsReading::new(breathing, heartbeat, movement))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_buffer() -> CsiDataBuffer {
        let mut buffer = CsiDataBuffer::new(100.0);

        // Add 10 seconds of simulated breathing signal
        let num_samples = 1000;
        let amplitudes: Vec<f64> = (0..num_samples)
            .map(|i| {
                let t = i as f64 / 100.0;
                // 16 BPM breathing (0.267 Hz)
                (2.0 * std::f64::consts::PI * 0.267 * t).sin()
            })
            .collect();

        let phases: Vec<f64> = (0..num_samples)
            .map(|i| {
                let t = i as f64 / 100.0;
                // Phase variation from movement
                (2.0 * std::f64::consts::PI * 0.267 * t).sin() * 0.5
            })
            .collect();

        buffer.add_samples(&amplitudes, &phases);
        buffer
    }

    #[test]
    fn test_pipeline_creation() {
        let config = DetectionConfig::default();
        let pipeline = DetectionPipeline::new(config);
        assert_eq!(pipeline.config().sample_rate, 1000.0);
    }

    #[test]
    fn test_csi_buffer() {
        let mut buffer = CsiDataBuffer::new(100.0);

        assert!(!buffer.has_sufficient_data(5.0));

        let amplitudes: Vec<f64> = vec![1.0; 600];
        let phases: Vec<f64> = vec![0.0; 600];
        buffer.add_samples(&amplitudes, &phases);

        assert!(buffer.has_sufficient_data(5.0));
        assert_eq!(buffer.duration(), 6.0);
    }

    #[test]
    fn test_vital_signs_detection() {
        let config = DetectionConfig::default();
        let pipeline = DetectionPipeline::new(config);
        let buffer = create_test_buffer();

        let result = pipeline.detect(&buffer);
        assert!(result.is_some());

        let reading = result.unwrap();
        assert!(reading.has_vitals());
    }

    #[test]
    fn test_config_from_disaster_config() {
        let disaster_config = DisasterConfig::builder()
            .sensitivity(0.9)
            .build();

        let detection_config = DetectionConfig::from_disaster_config(&disaster_config);

        // High sensitivity should enable heartbeat detection
        assert!(detection_config.enable_heartbeat);
        // Low minimum confidence due to high sensitivity
        assert!(detection_config.min_confidence < 0.4);
    }
}
