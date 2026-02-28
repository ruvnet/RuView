//! Expand Layer - Tensor Projection
//!
//! This module handles dimension adaptation when stored tensor dimensions
//! don't match the target LLM's expected input dimensions.
//!
//! For example, projecting 768-dim RoBERTa embeddings to 4096-dim LLaMA space.

use ndarray::{Array1, Array2};
use rand::Rng;
use std::collections::HashMap;
use std::time::Instant;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProjectionError {
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    #[error("Projector not found for model: {0}")]
    ProjectorNotFound(String),

    #[error("Invalid projection weights: {0}")]
    InvalidWeights(String),
}

pub type Result<T> = std::result::Result<T, ProjectionError>;

/// Linear projector: y = Wx + b
///
/// Projects from source dimension to target dimension.
#[derive(Clone)]
pub struct Projector {
    /// Weight matrix [target_dim, source_dim]
    weights: Array2<f32>,
    /// Bias vector [target_dim]
    bias: Array1<f32>,
    /// Source dimension
    source_dim: usize,
    /// Target dimension
    target_dim: usize,
    /// Model identifier
    model_id: String,
}

impl Projector {
    /// Create a new projector with random initialization
    pub fn new(source_dim: usize, target_dim: usize, model_id: impl Into<String>) -> Self {
        let mut rng = rand::thread_rng();

        // Xavier initialization
        let scale = (2.0 / (source_dim + target_dim) as f32).sqrt();
        let weights_data: Vec<f32> = (0..target_dim * source_dim)
            .map(|_| rng.gen_range(-scale..scale))
            .collect();

        Self {
            weights: Array2::from_shape_vec((target_dim, source_dim), weights_data).unwrap(),
            bias: Array1::zeros(target_dim),
            source_dim,
            target_dim,
            model_id: model_id.into(),
        }
    }

    /// Create identity projector (no transformation)
    pub fn identity(dim: usize, model_id: impl Into<String>) -> Self {
        let mut weights = Array2::zeros((dim, dim));
        for i in 0..dim {
            weights[[i, i]] = 1.0;
        }

        Self {
            weights,
            bias: Array1::zeros(dim),
            source_dim: dim,
            target_dim: dim,
            model_id: model_id.into(),
        }
    }

    /// Create with specific weights
    pub fn with_weights(
        weights: Array2<f32>,
        bias: Array1<f32>,
        model_id: impl Into<String>,
    ) -> Result<Self> {
        let (target_dim, source_dim) = weights.dim();
        if bias.len() != target_dim {
            return Err(ProjectionError::InvalidWeights(format!(
                "Bias length {} doesn't match target dim {}",
                bias.len(),
                target_dim
            )));
        }

        Ok(Self {
            weights,
            bias,
            source_dim,
            target_dim,
            model_id: model_id.into(),
        })
    }

    /// Project a vector from source to target dimension
    pub fn project(&self, input: &[f32]) -> Result<Vec<f32>> {
        if input.len() != self.source_dim {
            return Err(ProjectionError::DimensionMismatch {
                expected: self.source_dim,
                actual: input.len(),
            });
        }

        let input_arr = Array1::from_vec(input.to_vec());
        let output = self.weights.dot(&input_arr) + &self.bias;

        Ok(output.to_vec())
    }

    /// Project with timing info
    pub fn project_timed(&self, input: &[f32]) -> Result<(Vec<f32>, u64)> {
        let start = Instant::now();
        let result = self.project(input)?;
        let latency_us = start.elapsed().as_micros() as u64;
        Ok((result, latency_us))
    }

    /// Batch project multiple vectors
    pub fn project_batch(&self, inputs: &[Vec<f32>]) -> Result<Vec<Vec<f32>>> {
        inputs.iter().map(|v| self.project(v)).collect()
    }

    /// Get source dimension
    pub fn source_dim(&self) -> usize {
        self.source_dim
    }

    /// Get target dimension
    pub fn target_dim(&self) -> usize {
        self.target_dim
    }

    /// Get model identifier
    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    /// Export weights to binary format
    pub fn export_weights(&self) -> Vec<u8> {
        let mut data = Vec::new();

        // Header: source_dim, target_dim, model_id length
        data.extend_from_slice(&(self.source_dim as u32).to_le_bytes());
        data.extend_from_slice(&(self.target_dim as u32).to_le_bytes());
        let model_id_bytes = self.model_id.as_bytes();
        data.extend_from_slice(&(model_id_bytes.len() as u32).to_le_bytes());
        data.extend_from_slice(model_id_bytes);

        // Weights (row-major)
        for &w in self.weights.iter() {
            data.extend_from_slice(&w.to_le_bytes());
        }

        // Bias
        for &b in self.bias.iter() {
            data.extend_from_slice(&b.to_le_bytes());
        }

        data
    }

    /// Load weights from binary format
    pub fn load_weights(data: &[u8]) -> Result<Self> {
        if data.len() < 12 {
            return Err(ProjectionError::InvalidWeights("Data too short".into()));
        }

        let source_dim = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        let target_dim = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
        let model_id_len = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;

        let model_id = String::from_utf8_lossy(&data[12..12 + model_id_len]).to_string();

        let weights_start = 12 + model_id_len;
        let weights_size = target_dim * source_dim * 4;
        let bias_size = target_dim * 4;

        if data.len() < weights_start + weights_size + bias_size {
            return Err(ProjectionError::InvalidWeights(
                "Data too short for weights".into(),
            ));
        }

        let mut weights_data = Vec::with_capacity(target_dim * source_dim);
        for chunk in data[weights_start..weights_start + weights_size].chunks_exact(4) {
            let bytes: [u8; 4] = chunk.try_into().unwrap();
            weights_data.push(f32::from_le_bytes(bytes));
        }

        let mut bias_data = Vec::with_capacity(target_dim);
        for chunk in data[weights_start + weights_size..].chunks_exact(4) {
            let bytes: [u8; 4] = chunk.try_into().unwrap();
            bias_data.push(f32::from_le_bytes(bytes));
        }

        Ok(Self {
            weights: Array2::from_shape_vec((target_dim, source_dim), weights_data).unwrap(),
            bias: Array1::from_vec(bias_data),
            source_dim,
            target_dim,
            model_id,
        })
    }
}

/// Registry of projectors for different model alignments
pub struct ProjectorRegistry {
    projectors: HashMap<String, Projector>,
}

impl ProjectorRegistry {
    pub fn new() -> Self {
        Self {
            projectors: HashMap::new(),
        }
    }

    /// Register a projector for a model
    pub fn register(&mut self, projector: Projector) {
        self.projectors
            .insert(projector.model_id.clone(), projector);
    }

    /// Get projector for a model
    pub fn get(&self, model_id: &str) -> Option<&Projector> {
        self.projectors.get(model_id)
    }

    /// Project tensor to target LLM space
    pub fn project(&self, tensor: &[f32], model_id: &str) -> Result<Vec<f32>> {
        let projector = self
            .projectors
            .get(model_id)
            .ok_or_else(|| ProjectionError::ProjectorNotFound(model_id.to_string()))?;

        projector.project(tensor)
    }

    /// Check if projector exists for model
    pub fn has_projector(&self, model_id: &str) -> bool {
        self.projectors.contains_key(model_id)
    }

    /// List registered models
    pub fn models(&self) -> Vec<&str> {
        self.projectors.keys().map(|s| s.as_str()).collect()
    }

    /// Create with common LLM projectors
    pub fn with_defaults(source_dim: usize) -> Self {
        let mut registry = Self::new();

        // Common LLM configurations
        let models = [
            ("llama3-8b", 4096),
            ("llama3-70b", 8192),
            ("gpt-4", 8192),
            ("claude-3", 8192),
            ("mistral-7b", 4096),
            ("phi-3", 3072),
        ];

        for (model_id, target_dim) in models {
            if source_dim == target_dim {
                registry.register(Projector::identity(source_dim, model_id));
            } else {
                registry.register(Projector::new(source_dim, target_dim, model_id));
            }
        }

        registry
    }
}

impl Default for ProjectorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Expand layer for REFRAG pipeline
pub struct ExpandLayer {
    registry: ProjectorRegistry,
    /// Default target model
    default_model: String,
    /// Enable auto-projection
    auto_project: bool,
}

impl ExpandLayer {
    pub fn new(registry: ProjectorRegistry, default_model: impl Into<String>) -> Self {
        Self {
            registry,
            default_model: default_model.into(),
            auto_project: true,
        }
    }

    /// Create with default projectors for 768-dim source
    pub fn for_roberta() -> Self {
        Self::new(ProjectorRegistry::with_defaults(768), "llama3-8b")
    }

    /// Create with default projectors for 1536-dim source (OpenAI ada-002)
    pub fn for_openai() -> Self {
        Self::new(ProjectorRegistry::with_defaults(1536), "gpt-4")
    }

    /// Set default target model
    pub fn with_default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }

    /// Enable/disable auto-projection
    pub fn with_auto_project(mut self, enabled: bool) -> Self {
        self.auto_project = enabled;
        self
    }

    /// Expand tensor to target LLM space
    pub fn expand(&self, tensor: &[f32], target_model: Option<&str>) -> Result<Vec<f32>> {
        let model = target_model.unwrap_or(&self.default_model);
        self.registry.project(tensor, model)
    }

    /// Expand with automatic model detection
    pub fn expand_auto(&self, tensor: &[f32], alignment_model: Option<&str>) -> Result<Vec<f32>> {
        if !self.auto_project {
            return Ok(tensor.to_vec());
        }

        let model = alignment_model.unwrap_or(&self.default_model);
        self.registry.project(tensor, model)
    }

    /// Check if expansion is needed
    pub fn needs_expansion(&self, tensor_dim: usize, target_model: &str) -> bool {
        if let Some(projector) = self.registry.get(target_model) {
            projector.target_dim() != tensor_dim
        } else {
            false
        }
    }

    /// Get registry for registration
    pub fn registry_mut(&mut self) -> &mut ProjectorRegistry {
        &mut self.registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_projector_dimensions() {
        let projector = Projector::new(768, 4096, "test-model");

        assert_eq!(projector.source_dim(), 768);
        assert_eq!(projector.target_dim(), 4096);
        assert_eq!(projector.model_id(), "test-model");
    }

    #[test]
    fn test_identity_projector() {
        let projector = Projector::identity(4, "identity");
        let input = vec![1.0, 2.0, 3.0, 4.0];

        let output = projector.project(&input).unwrap();
        assert_eq!(input, output);
    }

    #[test]
    fn test_projection() {
        let projector = Projector::new(4, 8, "test");
        let input = vec![1.0, 2.0, 3.0, 4.0];

        let output = projector.project(&input).unwrap();
        assert_eq!(output.len(), 8);
    }

    #[test]
    fn test_dimension_mismatch() {
        let projector = Projector::new(4, 8, "test");
        let input = vec![1.0, 2.0, 3.0]; // Wrong size

        let result = projector.project(&input);
        assert!(matches!(
            result,
            Err(ProjectionError::DimensionMismatch { .. })
        ));
    }

    #[test]
    fn test_projector_registry() {
        let mut registry = ProjectorRegistry::new();
        registry.register(Projector::new(768, 4096, "llama3-8b"));
        registry.register(Projector::new(768, 8192, "gpt-4"));

        assert!(registry.has_projector("llama3-8b"));
        assert!(registry.has_projector("gpt-4"));
        assert!(!registry.has_projector("unknown"));

        let models = registry.models();
        assert_eq!(models.len(), 2);
    }

    #[test]
    fn test_expand_layer() {
        let expand = ExpandLayer::for_roberta();

        let tensor = vec![0.1f32; 768];
        let expanded = expand.expand(&tensor, Some("llama3-8b")).unwrap();

        assert_eq!(expanded.len(), 4096);
    }

    #[test]
    fn test_weight_export_import() {
        let projector = Projector::new(4, 8, "test-model");
        let exported = projector.export_weights();

        let imported = Projector::load_weights(&exported).unwrap();

        assert_eq!(projector.source_dim(), imported.source_dim());
        assert_eq!(projector.target_dim(), imported.target_dim());
        assert_eq!(projector.model_id(), imported.model_id());

        // Verify same projection behavior
        let input = vec![1.0, 2.0, 3.0, 4.0];
        let out1 = projector.project(&input).unwrap();
        let out2 = imported.project(&input).unwrap();

        for (a, b) in out1.iter().zip(out2.iter()) {
            assert!((a - b).abs() < f32::EPSILON);
        }
    }
}
