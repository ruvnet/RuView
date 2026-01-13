//! Machine Learning module for debris penetration pattern recognition.
//!
//! This module provides ML-based models for:
//! - Debris material classification
//! - Penetration depth prediction
//! - Signal attenuation analysis
//! - Vital signs classification with uncertainty estimation
//!
//! ## Architecture
//!
//! The ML subsystem integrates with the `wifi-densepose-nn` crate for ONNX inference
//! and provides specialized models for disaster response scenarios.
//!
//! ```text
//! CSI Data -> Feature Extraction -> Model Inference -> Predictions
//!                |                        |                |
//!                v                        v                v
//!         [Debris Features]    [ONNX Models]    [Classifications]
//!         [Signal Features]    [Neural Nets]    [Confidences]
//! ```

mod debris_model;
mod vital_signs_classifier;

pub use debris_model::{
    DebrisModel, DebrisModelConfig, DebrisFeatureExtractor,
    MaterialType, DebrisClassification, AttenuationPrediction,
    DebrisModelError,
};

pub use vital_signs_classifier::{
    VitalSignsClassifier, VitalSignsClassifierConfig,
    BreathingClassification, HeartbeatClassification,
    UncertaintyEstimate, ClassifierOutput,
};

use crate::detection::CsiDataBuffer;
use crate::domain::{VitalSignsReading, BreathingPattern, HeartbeatSignature};
use async_trait::async_trait;
use std::path::Path;
use thiserror::Error;

/// Errors that can occur in ML operations
#[derive(Debug, Error)]
pub enum MlError {
    /// Model loading error
    #[error("Failed to load model: {0}")]
    ModelLoad(String),

    /// Inference error
    #[error("Inference failed: {0}")]
    Inference(String),

    /// Feature extraction error
    #[error("Feature extraction failed: {0}")]
    FeatureExtraction(String),

    /// Invalid input error
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Model not initialized
    #[error("Model not initialized: {0}")]
    NotInitialized(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Integration error with wifi-densepose-nn
    #[error("Neural network error: {0}")]
    NeuralNetwork(#[from] wifi_densepose_nn::NnError),
}

/// Result type for ML operations
pub type MlResult<T> = Result<T, MlError>;

/// Trait for debris penetration models
///
/// This trait defines the interface for models that can predict
/// material type and signal attenuation through debris layers.
#[async_trait]
pub trait DebrisPenetrationModel: Send + Sync {
    /// Classify the material type from CSI features
    async fn classify_material(&self, features: &DebrisFeatures) -> MlResult<MaterialType>;

    /// Predict signal attenuation through debris
    async fn predict_attenuation(&self, features: &DebrisFeatures) -> MlResult<AttenuationPrediction>;

    /// Estimate penetration depth in meters
    async fn estimate_depth(&self, features: &DebrisFeatures) -> MlResult<DepthEstimate>;

    /// Get model confidence for the predictions
    fn model_confidence(&self) -> f32;

    /// Check if the model is loaded and ready
    fn is_ready(&self) -> bool;
}

/// Features extracted from CSI data for debris analysis
#[derive(Debug, Clone)]
pub struct DebrisFeatures {
    /// Amplitude attenuation across subcarriers
    pub amplitude_attenuation: Vec<f32>,
    /// Phase shift patterns
    pub phase_shifts: Vec<f32>,
    /// Frequency-selective fading characteristics
    pub fading_profile: Vec<f32>,
    /// Coherence bandwidth estimate
    pub coherence_bandwidth: f32,
    /// RMS delay spread
    pub delay_spread: f32,
    /// Signal-to-noise ratio estimate
    pub snr_db: f32,
    /// Multipath richness indicator
    pub multipath_richness: f32,
    /// Temporal stability metric
    pub temporal_stability: f32,
}

impl DebrisFeatures {
    /// Create new debris features from raw CSI data
    pub fn from_csi(buffer: &CsiDataBuffer) -> MlResult<Self> {
        if buffer.amplitudes.is_empty() {
            return Err(MlError::FeatureExtraction("Empty CSI buffer".into()));
        }

        // Calculate amplitude attenuation
        let amplitude_attenuation = Self::compute_amplitude_features(&buffer.amplitudes);

        // Calculate phase shifts
        let phase_shifts = Self::compute_phase_features(&buffer.phases);

        // Compute fading profile
        let fading_profile = Self::compute_fading_profile(&buffer.amplitudes);

        // Estimate coherence bandwidth from frequency correlation
        let coherence_bandwidth = Self::estimate_coherence_bandwidth(&buffer.amplitudes);

        // Estimate delay spread
        let delay_spread = Self::estimate_delay_spread(&buffer.amplitudes);

        // Estimate SNR
        let snr_db = Self::estimate_snr(&buffer.amplitudes);

        // Multipath richness
        let multipath_richness = Self::compute_multipath_richness(&buffer.amplitudes);

        // Temporal stability
        let temporal_stability = Self::compute_temporal_stability(&buffer.amplitudes);

        Ok(Self {
            amplitude_attenuation,
            phase_shifts,
            fading_profile,
            coherence_bandwidth,
            delay_spread,
            snr_db,
            multipath_richness,
            temporal_stability,
        })
    }

    /// Compute amplitude features
    fn compute_amplitude_features(amplitudes: &[f64]) -> Vec<f32> {
        if amplitudes.is_empty() {
            return vec![];
        }

        let mean = amplitudes.iter().sum::<f64>() / amplitudes.len() as f64;
        let variance = amplitudes.iter()
            .map(|a| (a - mean).powi(2))
            .sum::<f64>() / amplitudes.len() as f64;
        let std_dev = variance.sqrt();

        // Normalize amplitudes
        amplitudes.iter()
            .map(|a| ((a - mean) / (std_dev + 1e-8)) as f32)
            .collect()
    }

    /// Compute phase features
    fn compute_phase_features(phases: &[f64]) -> Vec<f32> {
        if phases.len() < 2 {
            return vec![];
        }

        // Compute phase differences (unwrapped)
        phases.windows(2)
            .map(|w| {
                let diff = w[1] - w[0];
                // Unwrap phase
                let unwrapped = if diff > std::f64::consts::PI {
                    diff - 2.0 * std::f64::consts::PI
                } else if diff < -std::f64::consts::PI {
                    diff + 2.0 * std::f64::consts::PI
                } else {
                    diff
                };
                unwrapped as f32
            })
            .collect()
    }

    /// Compute fading profile (power spectral characteristics)
    fn compute_fading_profile(amplitudes: &[f64]) -> Vec<f32> {
        use rustfft::{FftPlanner, num_complex::Complex};

        if amplitudes.len() < 16 {
            return vec![0.0; 8];
        }

        // Take a subset for FFT
        let n = 64.min(amplitudes.len());
        let mut buffer: Vec<Complex<f64>> = amplitudes.iter()
            .take(n)
            .map(|&a| Complex::new(a, 0.0))
            .collect();

        // Pad to power of 2
        while buffer.len() < 64 {
            buffer.push(Complex::new(0.0, 0.0));
        }

        // Compute FFT
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(64);
        fft.process(&mut buffer);

        // Extract power spectrum (first half)
        buffer.iter()
            .take(8)
            .map(|c| (c.norm() / n as f64) as f32)
            .collect()
    }

    /// Estimate coherence bandwidth from frequency correlation
    fn estimate_coherence_bandwidth(amplitudes: &[f64]) -> f32 {
        if amplitudes.len() < 10 {
            return 0.0;
        }

        // Compute autocorrelation
        let n = amplitudes.len();
        let mean = amplitudes.iter().sum::<f64>() / n as f64;
        let variance: f64 = amplitudes.iter()
            .map(|a| (a - mean).powi(2))
            .sum::<f64>() / n as f64;

        if variance < 1e-10 {
            return 0.0;
        }

        // Find lag where correlation drops below 0.5
        let mut coherence_lag = n;
        for lag in 1..n / 2 {
            let correlation: f64 = amplitudes.iter()
                .take(n - lag)
                .zip(amplitudes.iter().skip(lag))
                .map(|(a, b)| (a - mean) * (b - mean))
                .sum::<f64>() / ((n - lag) as f64 * variance);

            if correlation < 0.5 {
                coherence_lag = lag;
                break;
            }
        }

        // Convert to bandwidth estimate (assuming 20 MHz channel)
        (20.0 / coherence_lag as f32).min(20.0)
    }

    /// Estimate RMS delay spread
    fn estimate_delay_spread(amplitudes: &[f64]) -> f32 {
        if amplitudes.len() < 10 {
            return 0.0;
        }

        // Use power delay profile approximation
        let power: Vec<f64> = amplitudes.iter().map(|a| a.powi(2)).collect();
        let total_power: f64 = power.iter().sum();

        if total_power < 1e-10 {
            return 0.0;
        }

        // Calculate mean delay
        let mean_delay: f64 = power.iter()
            .enumerate()
            .map(|(i, p)| i as f64 * p)
            .sum::<f64>() / total_power;

        // Calculate RMS delay spread
        let variance: f64 = power.iter()
            .enumerate()
            .map(|(i, p)| (i as f64 - mean_delay).powi(2) * p)
            .sum::<f64>() / total_power;

        // Convert to nanoseconds (assuming sample period)
        (variance.sqrt() * 50.0) as f32 // 50 ns per sample assumed
    }

    /// Estimate SNR from amplitude variance
    fn estimate_snr(amplitudes: &[f64]) -> f32 {
        if amplitudes.is_empty() {
            return 0.0;
        }

        let mean = amplitudes.iter().sum::<f64>() / amplitudes.len() as f64;
        let variance = amplitudes.iter()
            .map(|a| (a - mean).powi(2))
            .sum::<f64>() / amplitudes.len() as f64;

        if variance < 1e-10 {
            return 30.0; // High SNR assumed
        }

        // SNR estimate based on signal power to noise power ratio
        let signal_power = mean.powi(2);
        let snr_linear = signal_power / variance;

        (10.0 * snr_linear.log10()) as f32
    }

    /// Compute multipath richness indicator
    fn compute_multipath_richness(amplitudes: &[f64]) -> f32 {
        if amplitudes.len() < 10 {
            return 0.0;
        }

        // Calculate amplitude variance as multipath indicator
        let mean = amplitudes.iter().sum::<f64>() / amplitudes.len() as f64;
        let variance = amplitudes.iter()
            .map(|a| (a - mean).powi(2))
            .sum::<f64>() / amplitudes.len() as f64;

        // Normalize to 0-1 range
        let std_dev = variance.sqrt();
        let normalized = std_dev / (mean.abs() + 1e-8);

        (normalized.min(1.0)) as f32
    }

    /// Compute temporal stability metric
    fn compute_temporal_stability(amplitudes: &[f64]) -> f32 {
        if amplitudes.len() < 2 {
            return 1.0;
        }

        // Calculate coefficient of variation over time
        let differences: Vec<f64> = amplitudes.windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .collect();

        let mean_diff = differences.iter().sum::<f64>() / differences.len() as f64;
        let mean_amp = amplitudes.iter().sum::<f64>() / amplitudes.len() as f64;

        // Stability is inverse of relative variation
        let variation = mean_diff / (mean_amp.abs() + 1e-8);

        (1.0 - variation.min(1.0)) as f32
    }

    /// Convert to feature vector for model input
    pub fn to_feature_vector(&self) -> Vec<f32> {
        let mut features = Vec::with_capacity(256);

        // Add amplitude attenuation features (padded/truncated to 64)
        let amp_len = self.amplitude_attenuation.len().min(64);
        features.extend_from_slice(&self.amplitude_attenuation[..amp_len]);
        features.resize(64, 0.0);

        // Add phase shift features (padded/truncated to 64)
        let phase_len = self.phase_shifts.len().min(64);
        features.extend_from_slice(&self.phase_shifts[..phase_len]);
        features.resize(128, 0.0);

        // Add fading profile (padded to 16)
        let fading_len = self.fading_profile.len().min(16);
        features.extend_from_slice(&self.fading_profile[..fading_len]);
        features.resize(144, 0.0);

        // Add scalar features
        features.push(self.coherence_bandwidth);
        features.push(self.delay_spread);
        features.push(self.snr_db);
        features.push(self.multipath_richness);
        features.push(self.temporal_stability);

        // Pad to 256 for model input
        features.resize(256, 0.0);

        features
    }
}

/// Depth estimate with uncertainty
#[derive(Debug, Clone)]
pub struct DepthEstimate {
    /// Estimated depth in meters
    pub depth_meters: f32,
    /// Uncertainty (standard deviation) in meters
    pub uncertainty_meters: f32,
    /// Confidence in the estimate (0.0-1.0)
    pub confidence: f32,
    /// Lower bound of 95% confidence interval
    pub lower_bound: f32,
    /// Upper bound of 95% confidence interval
    pub upper_bound: f32,
}

impl DepthEstimate {
    /// Create a new depth estimate with uncertainty
    pub fn new(depth: f32, uncertainty: f32, confidence: f32) -> Self {
        Self {
            depth_meters: depth,
            uncertainty_meters: uncertainty,
            confidence,
            lower_bound: (depth - 1.96 * uncertainty).max(0.0),
            upper_bound: depth + 1.96 * uncertainty,
        }
    }

    /// Check if the estimate is reliable (high confidence, low uncertainty)
    pub fn is_reliable(&self) -> bool {
        self.confidence > 0.7 && self.uncertainty_meters < self.depth_meters * 0.3
    }
}

/// Configuration for the ML-enhanced detection pipeline
#[derive(Debug, Clone, PartialEq)]
pub struct MlDetectionConfig {
    /// Enable ML-based debris classification
    pub enable_debris_classification: bool,
    /// Enable ML-based vital signs classification
    pub enable_vital_classification: bool,
    /// Path to debris model file
    pub debris_model_path: Option<String>,
    /// Path to vital signs model file
    pub vital_model_path: Option<String>,
    /// Minimum confidence threshold for ML predictions
    pub min_confidence: f32,
    /// Use GPU for inference
    pub use_gpu: bool,
    /// Number of inference threads
    pub num_threads: usize,
}

impl Default for MlDetectionConfig {
    fn default() -> Self {
        Self {
            enable_debris_classification: false,
            enable_vital_classification: false,
            debris_model_path: None,
            vital_model_path: None,
            min_confidence: 0.5,
            use_gpu: false,
            num_threads: 4,
        }
    }
}

impl MlDetectionConfig {
    /// Create configuration for CPU inference
    pub fn cpu() -> Self {
        Self::default()
    }

    /// Create configuration for GPU inference
    pub fn gpu() -> Self {
        Self {
            use_gpu: true,
            ..Default::default()
        }
    }

    /// Enable debris classification with model path
    pub fn with_debris_model<P: Into<String>>(mut self, path: P) -> Self {
        self.debris_model_path = Some(path.into());
        self.enable_debris_classification = true;
        self
    }

    /// Enable vital signs classification with model path
    pub fn with_vital_model<P: Into<String>>(mut self, path: P) -> Self {
        self.vital_model_path = Some(path.into());
        self.enable_vital_classification = true;
        self
    }

    /// Set minimum confidence threshold
    pub fn with_min_confidence(mut self, confidence: f32) -> Self {
        self.min_confidence = confidence.clamp(0.0, 1.0);
        self
    }
}

/// ML-enhanced detection pipeline that combines traditional and ML-based detection
pub struct MlDetectionPipeline {
    config: MlDetectionConfig,
    debris_model: Option<DebrisModel>,
    vital_classifier: Option<VitalSignsClassifier>,
}

impl MlDetectionPipeline {
    /// Create a new ML detection pipeline
    pub fn new(config: MlDetectionConfig) -> Self {
        Self {
            config,
            debris_model: None,
            vital_classifier: None,
        }
    }

    /// Initialize models asynchronously
    pub async fn initialize(&mut self) -> MlResult<()> {
        if self.config.enable_debris_classification {
            if let Some(ref path) = self.config.debris_model_path {
                let debris_config = DebrisModelConfig {
                    use_gpu: self.config.use_gpu,
                    num_threads: self.config.num_threads,
                    confidence_threshold: self.config.min_confidence,
                };
                self.debris_model = Some(DebrisModel::from_onnx(path, debris_config)?);
            }
        }

        if self.config.enable_vital_classification {
            if let Some(ref path) = self.config.vital_model_path {
                let vital_config = VitalSignsClassifierConfig {
                    use_gpu: self.config.use_gpu,
                    num_threads: self.config.num_threads,
                    min_confidence: self.config.min_confidence,
                    enable_uncertainty: true,
                    mc_samples: 10,
                    dropout_rate: 0.1,
                };
                self.vital_classifier = Some(VitalSignsClassifier::from_onnx(path, vital_config)?);
            }
        }

        Ok(())
    }

    /// Process CSI data and return enhanced detection results
    pub async fn process(&self, buffer: &CsiDataBuffer) -> MlResult<MlDetectionResult> {
        let mut result = MlDetectionResult::default();

        // Extract debris features and classify if enabled
        if let Some(ref model) = self.debris_model {
            let features = DebrisFeatures::from_csi(buffer)?;
            result.debris_classification = Some(model.classify(&features).await?);
            result.depth_estimate = Some(model.estimate_depth(&features).await?);
        }

        // Classify vital signs if enabled
        if let Some(ref classifier) = self.vital_classifier {
            let features = classifier.extract_features(buffer)?;
            result.vital_classification = Some(classifier.classify(&features).await?);
        }

        Ok(result)
    }

    /// Check if the pipeline is ready for inference
    pub fn is_ready(&self) -> bool {
        let debris_ready = !self.config.enable_debris_classification
            || self.debris_model.as_ref().map_or(false, |m| m.is_loaded());
        let vital_ready = !self.config.enable_vital_classification
            || self.vital_classifier.as_ref().map_or(false, |c| c.is_loaded());

        debris_ready && vital_ready
    }

    /// Get configuration
    pub fn config(&self) -> &MlDetectionConfig {
        &self.config
    }
}

/// Combined ML detection results
#[derive(Debug, Clone, Default)]
pub struct MlDetectionResult {
    /// Debris classification result
    pub debris_classification: Option<DebrisClassification>,
    /// Depth estimate
    pub depth_estimate: Option<DepthEstimate>,
    /// Vital signs classification
    pub vital_classification: Option<ClassifierOutput>,
}

impl MlDetectionResult {
    /// Check if any ML detection was performed
    pub fn has_results(&self) -> bool {
        self.debris_classification.is_some()
            || self.depth_estimate.is_some()
            || self.vital_classification.is_some()
    }

    /// Get overall confidence
    pub fn overall_confidence(&self) -> f32 {
        let mut total = 0.0;
        let mut count = 0;

        if let Some(ref debris) = self.debris_classification {
            total += debris.confidence;
            count += 1;
        }

        if let Some(ref depth) = self.depth_estimate {
            total += depth.confidence;
            count += 1;
        }

        if let Some(ref vital) = self.vital_classification {
            total += vital.overall_confidence;
            count += 1;
        }

        if count > 0 {
            total / count as f32
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_buffer() -> CsiDataBuffer {
        let mut buffer = CsiDataBuffer::new(1000.0);
        let amplitudes: Vec<f64> = (0..1000)
            .map(|i| {
                let t = i as f64 / 1000.0;
                0.5 + 0.1 * (2.0 * std::f64::consts::PI * 0.25 * t).sin()
            })
            .collect();
        let phases: Vec<f64> = (0..1000)
            .map(|i| {
                let t = i as f64 / 1000.0;
                (2.0 * std::f64::consts::PI * 0.25 * t).sin() * 0.3
            })
            .collect();
        buffer.add_samples(&amplitudes, &phases);
        buffer
    }

    #[test]
    fn test_debris_features_extraction() {
        let buffer = create_test_buffer();
        let features = DebrisFeatures::from_csi(&buffer);
        assert!(features.is_ok());

        let features = features.unwrap();
        assert!(!features.amplitude_attenuation.is_empty());
        assert!(!features.phase_shifts.is_empty());
        assert!(features.coherence_bandwidth >= 0.0);
        assert!(features.delay_spread >= 0.0);
        assert!(features.temporal_stability >= 0.0);
    }

    #[test]
    fn test_feature_vector_size() {
        let buffer = create_test_buffer();
        let features = DebrisFeatures::from_csi(&buffer).unwrap();
        let vector = features.to_feature_vector();
        assert_eq!(vector.len(), 256);
    }

    #[test]
    fn test_depth_estimate() {
        let estimate = DepthEstimate::new(2.5, 0.3, 0.85);
        assert!(estimate.is_reliable());
        assert!(estimate.lower_bound < estimate.depth_meters);
        assert!(estimate.upper_bound > estimate.depth_meters);
    }

    #[test]
    fn test_ml_config_builder() {
        let config = MlDetectionConfig::cpu()
            .with_debris_model("models/debris.onnx")
            .with_vital_model("models/vitals.onnx")
            .with_min_confidence(0.7);

        assert!(config.enable_debris_classification);
        assert!(config.enable_vital_classification);
        assert_eq!(config.min_confidence, 0.7);
        assert!(!config.use_gpu);
    }
}
