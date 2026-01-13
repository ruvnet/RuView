//! ONNX-based debris penetration model for material classification and depth prediction.
//!
//! This module provides neural network models for analyzing debris characteristics
//! from WiFi CSI signals. Key capabilities include:
//!
//! - Material type classification (concrete, wood, metal, etc.)
//! - Signal attenuation prediction based on material properties
//! - Penetration depth estimation with uncertainty quantification
//!
//! ## Model Architecture
//!
//! The debris model uses a multi-head architecture:
//! - Shared feature encoder (CNN-based)
//! - Material classification head (softmax output)
//! - Attenuation regression head (linear output)
//! - Depth estimation head with uncertainty (mean + variance output)

use super::{DebrisFeatures, DepthEstimate, MlError, MlResult};
use ndarray::{Array1, Array2, Array4, s};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use parking_lot::RwLock;
use thiserror::Error;
use tracing::{debug, info, instrument, warn};

#[cfg(feature = "onnx")]
use wifi_densepose_nn::{OnnxBackend, OnnxSession, InferenceOptions, Tensor, TensorShape};

/// Errors specific to debris model operations
#[derive(Debug, Error)]
pub enum DebrisModelError {
    /// Model file not found
    #[error("Model file not found: {0}")]
    FileNotFound(String),

    /// Invalid model format
    #[error("Invalid model format: {0}")]
    InvalidFormat(String),

    /// Inference error
    #[error("Inference failed: {0}")]
    InferenceFailed(String),

    /// Feature extraction error
    #[error("Feature extraction failed: {0}")]
    FeatureExtractionFailed(String),
}

/// Types of materials that can be detected in debris
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MaterialType {
    /// Reinforced concrete (high attenuation)
    Concrete,
    /// Wood/timber (moderate attenuation)
    Wood,
    /// Metal/steel (very high attenuation, reflective)
    Metal,
    /// Glass (low attenuation)
    Glass,
    /// Brick/masonry (high attenuation)
    Brick,
    /// Drywall/plasterboard (low attenuation)
    Drywall,
    /// Mixed/composite materials
    Mixed,
    /// Unknown material type
    Unknown,
}

impl MaterialType {
    /// Get typical attenuation coefficient (dB/m)
    pub fn typical_attenuation(&self) -> f32 {
        match self {
            MaterialType::Concrete => 25.0,
            MaterialType::Wood => 8.0,
            MaterialType::Metal => 50.0,
            MaterialType::Glass => 3.0,
            MaterialType::Brick => 18.0,
            MaterialType::Drywall => 4.0,
            MaterialType::Mixed => 15.0,
            MaterialType::Unknown => 12.0,
        }
    }

    /// Get typical delay spread (nanoseconds)
    pub fn typical_delay_spread(&self) -> f32 {
        match self {
            MaterialType::Concrete => 150.0,
            MaterialType::Wood => 50.0,
            MaterialType::Metal => 200.0,
            MaterialType::Glass => 20.0,
            MaterialType::Brick => 100.0,
            MaterialType::Drywall => 30.0,
            MaterialType::Mixed => 80.0,
            MaterialType::Unknown => 60.0,
        }
    }

    /// From class index
    pub fn from_index(index: usize) -> Self {
        match index {
            0 => MaterialType::Concrete,
            1 => MaterialType::Wood,
            2 => MaterialType::Metal,
            3 => MaterialType::Glass,
            4 => MaterialType::Brick,
            5 => MaterialType::Drywall,
            6 => MaterialType::Mixed,
            _ => MaterialType::Unknown,
        }
    }

    /// To class index
    pub fn to_index(&self) -> usize {
        match self {
            MaterialType::Concrete => 0,
            MaterialType::Wood => 1,
            MaterialType::Metal => 2,
            MaterialType::Glass => 3,
            MaterialType::Brick => 4,
            MaterialType::Drywall => 5,
            MaterialType::Mixed => 6,
            MaterialType::Unknown => 7,
        }
    }

    /// Number of material classes
    pub const NUM_CLASSES: usize = 8;
}

impl std::fmt::Display for MaterialType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MaterialType::Concrete => write!(f, "Concrete"),
            MaterialType::Wood => write!(f, "Wood"),
            MaterialType::Metal => write!(f, "Metal"),
            MaterialType::Glass => write!(f, "Glass"),
            MaterialType::Brick => write!(f, "Brick"),
            MaterialType::Drywall => write!(f, "Drywall"),
            MaterialType::Mixed => write!(f, "Mixed"),
            MaterialType::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Result of debris material classification
#[derive(Debug, Clone)]
pub struct DebrisClassification {
    /// Primary material type detected
    pub material_type: MaterialType,
    /// Confidence score for the classification (0.0-1.0)
    pub confidence: f32,
    /// Per-class probabilities
    pub class_probabilities: Vec<f32>,
    /// Estimated layer count
    pub estimated_layers: u8,
    /// Whether multiple materials detected
    pub is_composite: bool,
}

impl DebrisClassification {
    /// Create a new debris classification
    pub fn new(probabilities: Vec<f32>) -> Self {
        let (max_idx, &max_prob) = probabilities.iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap_or((7, &0.0));

        // Check for composite materials (multiple high probabilities)
        let high_prob_count = probabilities.iter()
            .filter(|&&p| p > 0.2)
            .count();

        let is_composite = high_prob_count > 1 && max_prob < 0.7;
        let material_type = if is_composite {
            MaterialType::Mixed
        } else {
            MaterialType::from_index(max_idx)
        };

        // Estimate layer count from delay spread characteristics
        let estimated_layers = Self::estimate_layers(&probabilities);

        Self {
            material_type,
            confidence: max_prob,
            class_probabilities: probabilities,
            estimated_layers,
            is_composite,
        }
    }

    /// Estimate number of debris layers from probability distribution
    fn estimate_layers(probabilities: &[f32]) -> u8 {
        // More uniform distribution suggests more layers
        let entropy: f32 = probabilities.iter()
            .filter(|&&p| p > 0.01)
            .map(|&p| -p * p.ln())
            .sum();

        let max_entropy = (probabilities.len() as f32).ln();
        let normalized_entropy = entropy / max_entropy;

        // Map entropy to layer count (1-5)
        (1.0 + normalized_entropy * 4.0).round() as u8
    }

    /// Get secondary material if composite
    pub fn secondary_material(&self) -> Option<MaterialType> {
        if !self.is_composite {
            return None;
        }

        let primary_idx = self.material_type.to_index();
        self.class_probabilities.iter()
            .enumerate()
            .filter(|(i, _)| *i != primary_idx)
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(i, _)| MaterialType::from_index(i))
    }
}

/// Signal attenuation prediction result
#[derive(Debug, Clone)]
pub struct AttenuationPrediction {
    /// Predicted attenuation in dB
    pub attenuation_db: f32,
    /// Attenuation per meter (dB/m)
    pub attenuation_per_meter: f32,
    /// Uncertainty in the prediction
    pub uncertainty_db: f32,
    /// Frequency-dependent attenuation profile
    pub frequency_profile: Vec<f32>,
    /// Confidence in the prediction
    pub confidence: f32,
}

impl AttenuationPrediction {
    /// Create new attenuation prediction
    pub fn new(attenuation: f32, depth: f32, uncertainty: f32) -> Self {
        let attenuation_per_meter = if depth > 0.0 {
            attenuation / depth
        } else {
            0.0
        };

        Self {
            attenuation_db: attenuation,
            attenuation_per_meter,
            uncertainty_db: uncertainty,
            frequency_profile: vec![],
            confidence: (1.0 - uncertainty / attenuation.abs().max(1.0)).max(0.0),
        }
    }

    /// Predict signal at given depth
    pub fn predict_signal_at_depth(&self, depth_m: f32) -> f32 {
        -self.attenuation_per_meter * depth_m
    }
}

/// Configuration for debris model
#[derive(Debug, Clone)]
pub struct DebrisModelConfig {
    /// Use GPU for inference
    pub use_gpu: bool,
    /// Number of inference threads
    pub num_threads: usize,
    /// Minimum confidence threshold
    pub confidence_threshold: f32,
}

impl Default for DebrisModelConfig {
    fn default() -> Self {
        Self {
            use_gpu: false,
            num_threads: 4,
            confidence_threshold: 0.5,
        }
    }
}

/// Feature extractor for debris classification
pub struct DebrisFeatureExtractor {
    /// Number of subcarriers to analyze
    num_subcarriers: usize,
    /// Window size for temporal analysis
    window_size: usize,
    /// Whether to use advanced features
    use_advanced_features: bool,
}

impl Default for DebrisFeatureExtractor {
    fn default() -> Self {
        Self {
            num_subcarriers: 64,
            window_size: 100,
            use_advanced_features: true,
        }
    }
}

impl DebrisFeatureExtractor {
    /// Create new feature extractor
    pub fn new(num_subcarriers: usize, window_size: usize) -> Self {
        Self {
            num_subcarriers,
            window_size,
            use_advanced_features: true,
        }
    }

    /// Extract features from debris features for model input
    pub fn extract(&self, features: &DebrisFeatures) -> MlResult<Array2<f32>> {
        let feature_vector = features.to_feature_vector();

        // Reshape to 2D for model input (batch_size=1, features)
        let arr = Array2::from_shape_vec(
            (1, feature_vector.len()),
            feature_vector,
        ).map_err(|e| MlError::FeatureExtraction(e.to_string()))?;

        Ok(arr)
    }

    /// Extract spatial-temporal features for CNN input
    pub fn extract_spatial_temporal(&self, features: &DebrisFeatures) -> MlResult<Array4<f32>> {
        let amp_len = features.amplitude_attenuation.len().min(self.num_subcarriers);
        let phase_len = features.phase_shifts.len().min(self.num_subcarriers);

        // Create 4D tensor: [batch, channels, height, width]
        // channels: amplitude, phase
        // height: subcarriers
        // width: 1 (or temporal windows if available)
        let mut tensor = Array4::<f32>::zeros((1, 2, self.num_subcarriers, 1));

        // Fill amplitude channel
        for (i, &v) in features.amplitude_attenuation.iter().take(amp_len).enumerate() {
            tensor[[0, 0, i, 0]] = v;
        }

        // Fill phase channel
        for (i, &v) in features.phase_shifts.iter().take(phase_len).enumerate() {
            tensor[[0, 1, i, 0]] = v;
        }

        Ok(tensor)
    }
}

/// ONNX-based debris penetration model
pub struct DebrisModel {
    config: DebrisModelConfig,
    feature_extractor: DebrisFeatureExtractor,
    /// Material classification model weights (for rule-based fallback)
    material_weights: MaterialClassificationWeights,
    /// Whether ONNX model is loaded
    model_loaded: bool,
    /// Cached model session
    #[cfg(feature = "onnx")]
    session: Option<Arc<RwLock<OnnxSession>>>,
}

/// Pre-computed weights for rule-based material classification
struct MaterialClassificationWeights {
    /// Weights for attenuation features
    attenuation_weights: [f32; MaterialType::NUM_CLASSES],
    /// Weights for delay spread features
    delay_weights: [f32; MaterialType::NUM_CLASSES],
    /// Weights for coherence bandwidth
    coherence_weights: [f32; MaterialType::NUM_CLASSES],
    /// Bias terms
    biases: [f32; MaterialType::NUM_CLASSES],
}

impl Default for MaterialClassificationWeights {
    fn default() -> Self {
        // Pre-computed weights based on material RF properties
        Self {
            attenuation_weights: [0.8, 0.3, 0.95, 0.1, 0.6, 0.15, 0.5, 0.4],
            delay_weights: [0.7, 0.2, 0.9, 0.1, 0.5, 0.1, 0.4, 0.3],
            coherence_weights: [0.3, 0.7, 0.1, 0.9, 0.4, 0.8, 0.5, 0.5],
            biases: [-0.5, 0.2, -0.8, 0.5, -0.3, 0.3, 0.0, 0.0],
        }
    }
}

impl DebrisModel {
    /// Create a new debris model from ONNX file
    #[instrument(skip(path))]
    pub fn from_onnx<P: AsRef<Path>>(path: P, config: DebrisModelConfig) -> MlResult<Self> {
        let path_ref = path.as_ref();
        info!(?path_ref, "Loading debris model");

        #[cfg(feature = "onnx")]
        let session = if path_ref.exists() {
            let options = InferenceOptions {
                use_gpu: config.use_gpu,
                num_threads: config.num_threads,
                ..Default::default()
            };
            match OnnxSession::from_file(path_ref, &options) {
                Ok(s) => {
                    info!("ONNX debris model loaded successfully");
                    Some(Arc::new(RwLock::new(s)))
                }
                Err(e) => {
                    warn!(?e, "Failed to load ONNX model, using rule-based fallback");
                    None
                }
            }
        } else {
            warn!(?path_ref, "Model file not found, using rule-based fallback");
            None
        };

        #[cfg(feature = "onnx")]
        let model_loaded = session.is_some();

        #[cfg(not(feature = "onnx"))]
        let model_loaded = false;

        Ok(Self {
            config,
            feature_extractor: DebrisFeatureExtractor::default(),
            material_weights: MaterialClassificationWeights::default(),
            model_loaded,
            #[cfg(feature = "onnx")]
            session,
        })
    }

    /// Create with in-memory model bytes
    #[cfg(feature = "onnx")]
    pub fn from_bytes(bytes: &[u8], config: DebrisModelConfig) -> MlResult<Self> {
        let options = InferenceOptions {
            use_gpu: config.use_gpu,
            num_threads: config.num_threads,
            ..Default::default()
        };

        let session = OnnxSession::from_bytes(bytes, &options)
            .map_err(|e| MlError::ModelLoad(e.to_string()))?;

        Ok(Self {
            config,
            feature_extractor: DebrisFeatureExtractor::default(),
            material_weights: MaterialClassificationWeights::default(),
            model_loaded: true,
            session: Some(Arc::new(RwLock::new(session))),
        })
    }

    /// Create a rule-based model (no ONNX required)
    pub fn rule_based(config: DebrisModelConfig) -> Self {
        Self {
            config,
            feature_extractor: DebrisFeatureExtractor::default(),
            material_weights: MaterialClassificationWeights::default(),
            model_loaded: false,
            #[cfg(feature = "onnx")]
            session: None,
        }
    }

    /// Check if ONNX model is loaded
    pub fn is_loaded(&self) -> bool {
        self.model_loaded
    }

    /// Classify material type from debris features
    #[instrument(skip(self, features))]
    pub async fn classify(&self, features: &DebrisFeatures) -> MlResult<DebrisClassification> {
        #[cfg(feature = "onnx")]
        if let Some(ref session) = self.session {
            return self.classify_onnx(features, session).await;
        }

        // Fall back to rule-based classification
        self.classify_rules(features)
    }

    /// ONNX-based classification
    #[cfg(feature = "onnx")]
    async fn classify_onnx(
        &self,
        features: &DebrisFeatures,
        session: &Arc<RwLock<OnnxSession>>,
    ) -> MlResult<DebrisClassification> {
        let input_features = self.feature_extractor.extract(features)?;

        // Prepare input tensor
        let input_array = Array4::from_shape_vec(
            (1, 1, 1, input_features.len()),
            input_features.iter().cloned().collect(),
        ).map_err(|e| MlError::Inference(e.to_string()))?;

        let input_tensor = Tensor::Float4D(input_array);

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), input_tensor);

        // Run inference
        let outputs = session.write().run(inputs)
            .map_err(|e| MlError::NeuralNetwork(e))?;

        // Extract classification probabilities
        let probabilities = if let Some(output) = outputs.get("material_probs") {
            output.to_vec()
                .map_err(|e| MlError::Inference(e.to_string()))?
        } else {
            // Fallback to rule-based
            return self.classify_rules(features);
        };

        // Ensure we have enough classes
        let mut probs = vec![0.0f32; MaterialType::NUM_CLASSES];
        for (i, &p) in probabilities.iter().take(MaterialType::NUM_CLASSES).enumerate() {
            probs[i] = p;
        }

        // Apply softmax normalization
        let max_val = probs.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_sum: f32 = probs.iter().map(|&x| (x - max_val).exp()).sum();
        for p in &mut probs {
            *p = (*p - max_val).exp() / exp_sum;
        }

        Ok(DebrisClassification::new(probs))
    }

    /// Rule-based material classification (fallback)
    fn classify_rules(&self, features: &DebrisFeatures) -> MlResult<DebrisClassification> {
        let mut scores = [0.0f32; MaterialType::NUM_CLASSES];

        // Normalize input features
        let attenuation_score = (features.snr_db.abs() / 30.0).min(1.0);
        let delay_score = (features.delay_spread / 200.0).min(1.0);
        let coherence_score = (features.coherence_bandwidth / 20.0).min(1.0);
        let stability_score = features.temporal_stability;

        // Compute weighted scores for each material
        for i in 0..MaterialType::NUM_CLASSES {
            scores[i] = self.material_weights.attenuation_weights[i] * attenuation_score
                + self.material_weights.delay_weights[i] * delay_score
                + self.material_weights.coherence_weights[i] * (1.0 - coherence_score)
                + self.material_weights.biases[i]
                + 0.1 * stability_score;
        }

        // Apply softmax
        let max_score = scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_sum: f32 = scores.iter().map(|&s| (s - max_score).exp()).sum();
        let probabilities: Vec<f32> = scores.iter()
            .map(|&s| (s - max_score).exp() / exp_sum)
            .collect();

        Ok(DebrisClassification::new(probabilities))
    }

    /// Predict signal attenuation through debris
    #[instrument(skip(self, features))]
    pub async fn predict_attenuation(&self, features: &DebrisFeatures) -> MlResult<AttenuationPrediction> {
        // Get material classification first
        let classification = self.classify(features).await?;

        // Base attenuation from material type
        let base_attenuation = classification.material_type.typical_attenuation();

        // Adjust based on measured features
        let measured_factor = if features.snr_db < 0.0 {
            1.0 + (features.snr_db.abs() / 30.0).min(1.0)
        } else {
            1.0 - (features.snr_db / 30.0).min(0.5)
        };

        // Layer factor
        let layer_factor = 1.0 + 0.2 * (classification.estimated_layers as f32 - 1.0);

        // Composite factor
        let composite_factor = if classification.is_composite { 1.2 } else { 1.0 };

        let total_attenuation = base_attenuation * measured_factor * layer_factor * composite_factor;

        // Uncertainty estimation
        let uncertainty = if classification.is_composite {
            total_attenuation * 0.3  // Higher uncertainty for composite
        } else {
            total_attenuation * (1.0 - classification.confidence) * 0.5
        };

        // Estimate depth (will be refined by depth estimation)
        let estimated_depth = self.estimate_depth_internal(features, total_attenuation);

        Ok(AttenuationPrediction::new(total_attenuation, estimated_depth, uncertainty))
    }

    /// Estimate penetration depth
    #[instrument(skip(self, features))]
    pub async fn estimate_depth(&self, features: &DebrisFeatures) -> MlResult<DepthEstimate> {
        // Get attenuation prediction
        let attenuation = self.predict_attenuation(features).await?;

        // Estimate depth from attenuation and material properties
        let depth = self.estimate_depth_internal(features, attenuation.attenuation_db);

        // Calculate uncertainty
        let uncertainty = self.calculate_depth_uncertainty(
            features,
            depth,
            attenuation.confidence,
        );

        let confidence = (attenuation.confidence * features.temporal_stability).min(1.0);

        Ok(DepthEstimate::new(depth, uncertainty, confidence))
    }

    /// Internal depth estimation logic
    fn estimate_depth_internal(&self, features: &DebrisFeatures, attenuation_db: f32) -> f32 {
        // Use coherence bandwidth for depth estimation
        // Smaller coherence bandwidth suggests more multipath = deeper penetration
        let cb_depth = (20.0 - features.coherence_bandwidth) / 5.0;

        // Use delay spread
        let ds_depth = features.delay_spread / 100.0;

        // Use attenuation (assuming typical material)
        let att_depth = attenuation_db / 15.0;

        // Combine estimates with weights
        let depth = 0.3 * cb_depth + 0.3 * ds_depth + 0.4 * att_depth;

        // Clamp to reasonable range (0.1 - 10 meters)
        depth.clamp(0.1, 10.0)
    }

    /// Calculate uncertainty in depth estimate
    fn calculate_depth_uncertainty(
        &self,
        features: &DebrisFeatures,
        depth: f32,
        confidence: f32,
    ) -> f32 {
        // Base uncertainty proportional to depth
        let base_uncertainty = depth * 0.2;

        // Adjust by temporal stability (less stable = more uncertain)
        let stability_factor = 1.0 + (1.0 - features.temporal_stability) * 0.5;

        // Adjust by confidence (lower confidence = more uncertain)
        let confidence_factor = 1.0 + (1.0 - confidence) * 0.5;

        // Adjust by multipath richness (more multipath = harder to estimate)
        let multipath_factor = 1.0 + features.multipath_richness * 0.3;

        base_uncertainty * stability_factor * confidence_factor * multipath_factor
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detection::CsiDataBuffer;

    fn create_test_debris_features() -> DebrisFeatures {
        DebrisFeatures {
            amplitude_attenuation: vec![0.5; 64],
            phase_shifts: vec![0.1; 64],
            fading_profile: vec![0.8, 0.6, 0.4, 0.2, 0.1, 0.05, 0.02, 0.01],
            coherence_bandwidth: 5.0,
            delay_spread: 100.0,
            snr_db: 15.0,
            multipath_richness: 0.6,
            temporal_stability: 0.8,
        }
    }

    #[test]
    fn test_material_type() {
        assert_eq!(MaterialType::from_index(0), MaterialType::Concrete);
        assert_eq!(MaterialType::Concrete.to_index(), 0);
        assert!(MaterialType::Concrete.typical_attenuation() > MaterialType::Glass.typical_attenuation());
    }

    #[test]
    fn test_debris_classification() {
        let probs = vec![0.7, 0.1, 0.05, 0.05, 0.05, 0.02, 0.02, 0.01];
        let classification = DebrisClassification::new(probs);

        assert_eq!(classification.material_type, MaterialType::Concrete);
        assert!(classification.confidence > 0.6);
        assert!(!classification.is_composite);
    }

    #[test]
    fn test_composite_detection() {
        let probs = vec![0.4, 0.35, 0.1, 0.05, 0.05, 0.02, 0.02, 0.01];
        let classification = DebrisClassification::new(probs);

        assert!(classification.is_composite);
        assert_eq!(classification.material_type, MaterialType::Mixed);
    }

    #[test]
    fn test_attenuation_prediction() {
        let pred = AttenuationPrediction::new(25.0, 2.0, 3.0);
        assert_eq!(pred.attenuation_per_meter, 12.5);
        assert!(pred.confidence > 0.0);
    }

    #[tokio::test]
    async fn test_rule_based_classification() {
        let config = DebrisModelConfig::default();
        let model = DebrisModel::rule_based(config);

        let features = create_test_debris_features();
        let result = model.classify(&features).await;

        assert!(result.is_ok());
        let classification = result.unwrap();
        assert!(classification.confidence > 0.0);
    }

    #[tokio::test]
    async fn test_depth_estimation() {
        let config = DebrisModelConfig::default();
        let model = DebrisModel::rule_based(config);

        let features = create_test_debris_features();
        let result = model.estimate_depth(&features).await;

        assert!(result.is_ok());
        let estimate = result.unwrap();
        assert!(estimate.depth_meters > 0.0);
        assert!(estimate.depth_meters < 10.0);
        assert!(estimate.uncertainty_meters > 0.0);
    }

    #[test]
    fn test_feature_extractor() {
        let extractor = DebrisFeatureExtractor::default();
        let features = create_test_debris_features();

        let result = extractor.extract(&features);
        assert!(result.is_ok());

        let arr = result.unwrap();
        assert_eq!(arr.shape()[0], 1);
        assert_eq!(arr.shape()[1], 256);
    }

    #[test]
    fn test_spatial_temporal_extraction() {
        let extractor = DebrisFeatureExtractor::new(64, 100);
        let features = create_test_debris_features();

        let result = extractor.extract_spatial_temporal(&features);
        assert!(result.is_ok());

        let arr = result.unwrap();
        assert_eq!(arr.shape(), &[1, 2, 64, 1]);
    }
}
