//! CSI (Channel State Information) Processor
//!
//! This module provides functionality for preprocessing and processing CSI data
//! from WiFi signals for human pose estimation.

use chrono::{DateTime, Utc};
use ndarray::Array2;
use num_complex::Complex64;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::f64::consts::PI;
use thiserror::Error;

/// Errors that can occur during CSI processing
#[derive(Debug, Error)]
pub enum CsiProcessorError {
    /// Invalid configuration parameters
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Preprocessing failed
    #[error("Preprocessing failed: {0}")]
    PreprocessingFailed(String),

    /// Feature extraction failed
    #[error("Feature extraction failed: {0}")]
    FeatureExtractionFailed(String),

    /// Invalid input data
    #[error("Invalid input data: {0}")]
    InvalidData(String),

    /// Processing pipeline error
    #[error("Pipeline error: {0}")]
    PipelineError(String),
}

/// CSI data structure containing raw channel measurements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsiData {
    /// Timestamp of the measurement
    pub timestamp: DateTime<Utc>,

    /// Amplitude values (num_antennas x num_subcarriers)
    pub amplitude: Array2<f64>,

    /// Phase values in radians (num_antennas x num_subcarriers)
    pub phase: Array2<f64>,

    /// Center frequency in Hz
    pub frequency: f64,

    /// Bandwidth in Hz
    pub bandwidth: f64,

    /// Number of subcarriers
    pub num_subcarriers: usize,

    /// Number of antennas
    pub num_antennas: usize,

    /// Signal-to-noise ratio in dB
    pub snr: f64,

    /// Additional metadata
    #[serde(default)]
    pub metadata: CsiMetadata,
}

/// Metadata associated with CSI data
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CsiMetadata {
    /// Whether noise filtering has been applied
    pub noise_filtered: bool,

    /// Whether windowing has been applied
    pub windowed: bool,

    /// Whether normalization has been applied
    pub normalized: bool,

    /// Additional custom metadata
    #[serde(flatten)]
    pub custom: std::collections::HashMap<String, serde_json::Value>,
}

/// Builder for CsiData
#[derive(Debug, Default)]
pub struct CsiDataBuilder {
    timestamp: Option<DateTime<Utc>>,
    amplitude: Option<Array2<f64>>,
    phase: Option<Array2<f64>>,
    frequency: Option<f64>,
    bandwidth: Option<f64>,
    snr: Option<f64>,
    metadata: CsiMetadata,
}

impl CsiDataBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the timestamp
    pub fn timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.timestamp = Some(timestamp);
        self
    }

    /// Set amplitude data
    pub fn amplitude(mut self, amplitude: Array2<f64>) -> Self {
        self.amplitude = Some(amplitude);
        self
    }

    /// Set phase data
    pub fn phase(mut self, phase: Array2<f64>) -> Self {
        self.phase = Some(phase);
        self
    }

    /// Set center frequency
    pub fn frequency(mut self, frequency: f64) -> Self {
        self.frequency = Some(frequency);
        self
    }

    /// Set bandwidth
    pub fn bandwidth(mut self, bandwidth: f64) -> Self {
        self.bandwidth = Some(bandwidth);
        self
    }

    /// Set SNR
    pub fn snr(mut self, snr: f64) -> Self {
        self.snr = Some(snr);
        self
    }

    /// Set metadata
    pub fn metadata(mut self, metadata: CsiMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Build the CsiData
    pub fn build(self) -> Result<CsiData, CsiProcessorError> {
        let amplitude = self
            .amplitude
            .ok_or_else(|| CsiProcessorError::InvalidData("Amplitude data is required".into()))?;
        let phase = self
            .phase
            .ok_or_else(|| CsiProcessorError::InvalidData("Phase data is required".into()))?;

        if amplitude.shape() != phase.shape() {
            return Err(CsiProcessorError::InvalidData(
                "Amplitude and phase must have the same shape".into(),
            ));
        }

        let (num_antennas, num_subcarriers) = amplitude.dim();

        Ok(CsiData {
            timestamp: self.timestamp.unwrap_or_else(Utc::now),
            amplitude,
            phase,
            frequency: self.frequency.unwrap_or(5.0e9), // Default 5 GHz
            bandwidth: self.bandwidth.unwrap_or(20.0e6), // Default 20 MHz
            num_subcarriers,
            num_antennas,
            snr: self.snr.unwrap_or(20.0),
            metadata: self.metadata,
        })
    }
}

impl CsiData {
    /// Create a new CsiData builder
    pub fn builder() -> CsiDataBuilder {
        CsiDataBuilder::new()
    }

    /// Get complex CSI values
    pub fn to_complex(&self) -> Array2<Complex64> {
        let mut complex = Array2::zeros(self.amplitude.dim());
        for ((i, j), amp) in self.amplitude.indexed_iter() {
            let phase = self.phase[[i, j]];
            complex[[i, j]] = Complex64::from_polar(*amp, phase);
        }
        complex
    }

    /// Create from complex values
    pub fn from_complex(
        complex: &Array2<Complex64>,
        frequency: f64,
        bandwidth: f64,
    ) -> Result<Self, CsiProcessorError> {
        let (num_antennas, num_subcarriers) = complex.dim();
        let mut amplitude = Array2::zeros(complex.dim());
        let mut phase = Array2::zeros(complex.dim());

        for ((i, j), c) in complex.indexed_iter() {
            amplitude[[i, j]] = c.norm();
            phase[[i, j]] = c.arg();
        }

        Ok(Self {
            timestamp: Utc::now(),
            amplitude,
            phase,
            frequency,
            bandwidth,
            num_subcarriers,
            num_antennas,
            snr: 20.0,
            metadata: CsiMetadata::default(),
        })
    }
}

/// Configuration for CSI processor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsiProcessorConfig {
    /// Sampling rate in Hz
    pub sampling_rate: f64,

    /// Window size for processing
    pub window_size: usize,

    /// Overlap fraction (0.0 to 1.0)
    pub overlap: f64,

    /// Noise threshold in dB
    pub noise_threshold: f64,

    /// Human detection threshold (0.0 to 1.0)
    pub human_detection_threshold: f64,

    /// Temporal smoothing factor (0.0 to 1.0)
    pub smoothing_factor: f64,

    /// Maximum history size
    pub max_history_size: usize,

    /// Enable preprocessing
    pub enable_preprocessing: bool,

    /// Enable feature extraction
    pub enable_feature_extraction: bool,

    /// Enable human detection
    pub enable_human_detection: bool,
}

impl Default for CsiProcessorConfig {
    fn default() -> Self {
        Self {
            sampling_rate: 1000.0,
            window_size: 256,
            overlap: 0.5,
            noise_threshold: -30.0,
            human_detection_threshold: 0.8,
            smoothing_factor: 0.9,
            max_history_size: 500,
            enable_preprocessing: true,
            enable_feature_extraction: true,
            enable_human_detection: true,
        }
    }
}

/// Builder for CsiProcessorConfig
#[derive(Debug, Default)]
pub struct CsiProcessorConfigBuilder {
    config: CsiProcessorConfig,
}

impl CsiProcessorConfigBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            config: CsiProcessorConfig::default(),
        }
    }

    /// Set sampling rate
    pub fn sampling_rate(mut self, rate: f64) -> Self {
        self.config.sampling_rate = rate;
        self
    }

    /// Set window size
    pub fn window_size(mut self, size: usize) -> Self {
        self.config.window_size = size;
        self
    }

    /// Set overlap fraction
    pub fn overlap(mut self, overlap: f64) -> Self {
        self.config.overlap = overlap;
        self
    }

    /// Set noise threshold
    pub fn noise_threshold(mut self, threshold: f64) -> Self {
        self.config.noise_threshold = threshold;
        self
    }

    /// Set human detection threshold
    pub fn human_detection_threshold(mut self, threshold: f64) -> Self {
        self.config.human_detection_threshold = threshold;
        self
    }

    /// Set smoothing factor
    pub fn smoothing_factor(mut self, factor: f64) -> Self {
        self.config.smoothing_factor = factor;
        self
    }

    /// Set max history size
    pub fn max_history_size(mut self, size: usize) -> Self {
        self.config.max_history_size = size;
        self
    }

    /// Enable/disable preprocessing
    pub fn enable_preprocessing(mut self, enable: bool) -> Self {
        self.config.enable_preprocessing = enable;
        self
    }

    /// Enable/disable feature extraction
    pub fn enable_feature_extraction(mut self, enable: bool) -> Self {
        self.config.enable_feature_extraction = enable;
        self
    }

    /// Enable/disable human detection
    pub fn enable_human_detection(mut self, enable: bool) -> Self {
        self.config.enable_human_detection = enable;
        self
    }

    /// Build the configuration
    pub fn build(self) -> CsiProcessorConfig {
        self.config
    }
}

impl CsiProcessorConfig {
    /// Create a new config builder
    pub fn builder() -> CsiProcessorConfigBuilder {
        CsiProcessorConfigBuilder::new()
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), CsiProcessorError> {
        if self.sampling_rate <= 0.0 {
            return Err(CsiProcessorError::InvalidConfig(
                "sampling_rate must be positive".into(),
            ));
        }

        if self.window_size == 0 {
            return Err(CsiProcessorError::InvalidConfig(
                "window_size must be positive".into(),
            ));
        }

        if !(0.0..1.0).contains(&self.overlap) {
            return Err(CsiProcessorError::InvalidConfig(
                "overlap must be between 0 and 1".into(),
            ));
        }

        Ok(())
    }
}

/// CSI Preprocessor for cleaning and preparing raw CSI data
#[derive(Debug)]
pub struct CsiPreprocessor {
    noise_threshold: f64,
}

impl CsiPreprocessor {
    /// Create a new preprocessor
    pub fn new(noise_threshold: f64) -> Self {
        Self { noise_threshold }
    }

    /// Remove noise from CSI data based on amplitude threshold
    pub fn remove_noise(&self, csi_data: &CsiData) -> Result<CsiData, CsiProcessorError> {
        // Convert amplitude to dB
        let amplitude_db = csi_data.amplitude.mapv(|a| 20.0 * (a + 1e-12).log10());

        // Create noise mask
        let noise_mask = amplitude_db.mapv(|db| db > self.noise_threshold);

        // Apply mask to amplitude
        let mut filtered_amplitude = csi_data.amplitude.clone();
        for ((i, j), &mask) in noise_mask.indexed_iter() {
            if !mask {
                filtered_amplitude[[i, j]] = 0.0;
            }
        }

        let mut metadata = csi_data.metadata.clone();
        metadata.noise_filtered = true;

        Ok(CsiData {
            timestamp: csi_data.timestamp,
            amplitude: filtered_amplitude,
            phase: csi_data.phase.clone(),
            frequency: csi_data.frequency,
            bandwidth: csi_data.bandwidth,
            num_subcarriers: csi_data.num_subcarriers,
            num_antennas: csi_data.num_antennas,
            snr: csi_data.snr,
            metadata,
        })
    }

    /// Apply Hamming window to reduce spectral leakage
    pub fn apply_windowing(&self, csi_data: &CsiData) -> Result<CsiData, CsiProcessorError> {
        let n = csi_data.num_subcarriers;
        let window = Self::hamming_window(n);

        // Apply window to each antenna's amplitude
        let mut windowed_amplitude = csi_data.amplitude.clone();
        for mut row in windowed_amplitude.rows_mut() {
            for (i, val) in row.iter_mut().enumerate() {
                *val *= window[i];
            }
        }

        let mut metadata = csi_data.metadata.clone();
        metadata.windowed = true;

        Ok(CsiData {
            timestamp: csi_data.timestamp,
            amplitude: windowed_amplitude,
            phase: csi_data.phase.clone(),
            frequency: csi_data.frequency,
            bandwidth: csi_data.bandwidth,
            num_subcarriers: csi_data.num_subcarriers,
            num_antennas: csi_data.num_antennas,
            snr: csi_data.snr,
            metadata,
        })
    }

    /// Normalize amplitude values to unit variance
    pub fn normalize_amplitude(&self, csi_data: &CsiData) -> Result<CsiData, CsiProcessorError> {
        let std_dev = self.calculate_std(&csi_data.amplitude);
        let normalized_amplitude = csi_data.amplitude.mapv(|a| a / (std_dev + 1e-12));

        let mut metadata = csi_data.metadata.clone();
        metadata.normalized = true;

        Ok(CsiData {
            timestamp: csi_data.timestamp,
            amplitude: normalized_amplitude,
            phase: csi_data.phase.clone(),
            frequency: csi_data.frequency,
            bandwidth: csi_data.bandwidth,
            num_subcarriers: csi_data.num_subcarriers,
            num_antennas: csi_data.num_antennas,
            snr: csi_data.snr,
            metadata,
        })
    }

    /// Generate Hamming window
    fn hamming_window(n: usize) -> Vec<f64> {
        (0..n)
            .map(|i| 0.54 - 0.46 * (2.0 * PI * i as f64 / (n - 1) as f64).cos())
            .collect()
    }

    /// Calculate standard deviation
    fn calculate_std(&self, arr: &Array2<f64>) -> f64 {
        let mean = arr.mean().unwrap_or(0.0);
        let variance = arr.mapv(|x| (x - mean).powi(2)).mean().unwrap_or(0.0);
        variance.sqrt()
    }
}

/// Statistics for CSI processing
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProcessingStatistics {
    /// Total number of samples processed
    pub total_processed: usize,

    /// Number of processing errors
    pub processing_errors: usize,

    /// Number of human detections
    pub human_detections: usize,

    /// Current history size
    pub history_size: usize,
}

impl ProcessingStatistics {
    /// Calculate error rate
    pub fn error_rate(&self) -> f64 {
        if self.total_processed > 0 {
            self.processing_errors as f64 / self.total_processed as f64
        } else {
            0.0
        }
    }

    /// Calculate detection rate
    pub fn detection_rate(&self) -> f64 {
        if self.total_processed > 0 {
            self.human_detections as f64 / self.total_processed as f64
        } else {
            0.0
        }
    }
}

/// Main CSI Processor for WiFi-DensePose
#[derive(Debug)]
pub struct CsiProcessor {
    config: CsiProcessorConfig,
    preprocessor: CsiPreprocessor,
    history: VecDeque<CsiData>,
    previous_detection_confidence: f64,
    statistics: ProcessingStatistics,
}

impl CsiProcessor {
    /// Create a new CSI processor
    pub fn new(config: CsiProcessorConfig) -> Result<Self, CsiProcessorError> {
        config.validate()?;

        let preprocessor = CsiPreprocessor::new(config.noise_threshold);

        Ok(Self {
            history: VecDeque::with_capacity(config.max_history_size),
            config,
            preprocessor,
            previous_detection_confidence: 0.0,
            statistics: ProcessingStatistics::default(),
        })
    }

    /// Get the configuration
    pub fn config(&self) -> &CsiProcessorConfig {
        &self.config
    }

    /// Preprocess CSI data
    pub fn preprocess(&self, csi_data: &CsiData) -> Result<CsiData, CsiProcessorError> {
        if !self.config.enable_preprocessing {
            return Ok(csi_data.clone());
        }

        // Remove noise
        let cleaned = self.preprocessor.remove_noise(csi_data)?;

        // Apply windowing
        let windowed = self.preprocessor.apply_windowing(&cleaned)?;

        // Normalize amplitude
        let normalized = self.preprocessor.normalize_amplitude(&windowed)?;

        Ok(normalized)
    }

    /// Add CSI data to history
    pub fn add_to_history(&mut self, csi_data: CsiData) {
        if self.history.len() >= self.config.max_history_size {
            self.history.pop_front();
        }
        self.history.push_back(csi_data);
        self.statistics.history_size = self.history.len();
    }

    /// Clear history
    pub fn clear_history(&mut self) {
        self.history.clear();
        self.statistics.history_size = 0;
    }

    /// Get recent history
    pub fn get_recent_history(&self, count: usize) -> Vec<&CsiData> {
        let len = self.history.len();
        if count >= len {
            self.history.iter().collect()
        } else {
            self.history.iter().skip(len - count).collect()
        }
    }

    /// Get history length
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Apply temporal smoothing (exponential moving average)
    pub fn apply_temporal_smoothing(&mut self, raw_confidence: f64) -> f64 {
        let smoothed = self.config.smoothing_factor * self.previous_detection_confidence
            + (1.0 - self.config.smoothing_factor) * raw_confidence;
        self.previous_detection_confidence = smoothed;
        smoothed
    }

    /// Get processing statistics
    pub fn get_statistics(&self) -> &ProcessingStatistics {
        &self.statistics
    }

    /// Reset statistics
    pub fn reset_statistics(&mut self) {
        self.statistics = ProcessingStatistics::default();
    }

    /// Increment total processed count
    pub fn increment_processed(&mut self) {
        self.statistics.total_processed += 1;
    }

    /// Increment error count
    pub fn increment_errors(&mut self) {
        self.statistics.processing_errors += 1;
    }

    /// Increment human detection count
    pub fn increment_detections(&mut self) {
        self.statistics.human_detections += 1;
    }

    /// Get previous detection confidence
    pub fn previous_confidence(&self) -> f64 {
        self.previous_detection_confidence
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array2;

    fn create_test_csi_data() -> CsiData {
        let amplitude = Array2::from_shape_fn((4, 64), |(i, j)| {
            1.0 + 0.1 * ((i + j) as f64).sin()
        });
        let phase = Array2::from_shape_fn((4, 64), |(i, j)| {
            0.5 * ((i + j) as f64 * 0.1).sin()
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

    #[test]
    fn test_config_validation() {
        let config = CsiProcessorConfig::builder()
            .sampling_rate(1000.0)
            .window_size(256)
            .overlap(0.5)
            .build();

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_config() {
        let config = CsiProcessorConfig::builder()
            .sampling_rate(-100.0)
            .build();

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_csi_processor_creation() {
        let config = CsiProcessorConfig::default();
        let processor = CsiProcessor::new(config);
        assert!(processor.is_ok());
    }

    #[test]
    fn test_preprocessing() {
        let config = CsiProcessorConfig::default();
        let processor = CsiProcessor::new(config).unwrap();
        let csi_data = create_test_csi_data();

        let result = processor.preprocess(&csi_data);
        assert!(result.is_ok());

        let preprocessed = result.unwrap();
        assert!(preprocessed.metadata.noise_filtered);
        assert!(preprocessed.metadata.windowed);
        assert!(preprocessed.metadata.normalized);
    }

    #[test]
    fn test_history_management() {
        let config = CsiProcessorConfig::builder()
            .max_history_size(5)
            .build();
        let mut processor = CsiProcessor::new(config).unwrap();

        for _ in 0..10 {
            let csi_data = create_test_csi_data();
            processor.add_to_history(csi_data);
        }

        assert_eq!(processor.history_len(), 5);
    }

    #[test]
    fn test_temporal_smoothing() {
        let config = CsiProcessorConfig::builder()
            .smoothing_factor(0.9)
            .build();
        let mut processor = CsiProcessor::new(config).unwrap();

        let smoothed1 = processor.apply_temporal_smoothing(1.0);
        assert!((smoothed1 - 0.1).abs() < 1e-6);

        let smoothed2 = processor.apply_temporal_smoothing(1.0);
        assert!(smoothed2 > smoothed1);
    }

    #[test]
    fn test_csi_data_builder() {
        let amplitude = Array2::ones((4, 64));
        let phase = Array2::zeros((4, 64));

        let csi_data = CsiData::builder()
            .amplitude(amplitude)
            .phase(phase)
            .frequency(2.4e9)
            .bandwidth(40.0e6)
            .snr(30.0)
            .build();

        assert!(csi_data.is_ok());
        let data = csi_data.unwrap();
        assert_eq!(data.num_antennas, 4);
        assert_eq!(data.num_subcarriers, 64);
    }

    #[test]
    fn test_complex_conversion() {
        let csi_data = create_test_csi_data();
        let complex = csi_data.to_complex();

        assert_eq!(complex.dim(), (4, 64));

        for ((i, j), c) in complex.indexed_iter() {
            let expected_amp = csi_data.amplitude[[i, j]];
            let expected_phase = csi_data.phase[[i, j]];
            let c_val: num_complex::Complex64 = *c;
            assert!((c_val.norm() - expected_amp).abs() < 1e-10);
            assert!((c_val.arg() - expected_phase).abs() < 1e-10);
        }
    }

    #[test]
    fn test_hamming_window() {
        let window = CsiPreprocessor::hamming_window(64);
        assert_eq!(window.len(), 64);

        // Hamming window should be symmetric
        for i in 0..32 {
            assert!((window[i] - window[63 - i]).abs() < 1e-10);
        }

        // First and last values should be approximately 0.08
        assert!((window[0] - 0.08).abs() < 0.01);
    }
}
