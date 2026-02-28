//! Application services for embedding generation.
//!
//! Provides high-level services for generating embeddings from audio
//! spectrograms using the Perch 2.0 ONNX model.

use std::sync::Arc;
use std::time::Instant;

use ndarray::Array3;
use rayon::prelude::*;
use tracing::{debug, info, instrument, warn};

use crate::domain::entities::{
    Embedding, EmbeddingBatch, EmbeddingMetadata, SegmentId, StorageTier,
};
use crate::infrastructure::model_manager::ModelManager;
use crate::normalization;
use crate::{EmbeddingError, EMBEDDING_DIM, MEL_BINS, MEL_FRAMES};

/// Input spectrogram for embedding generation.
///
/// Represents a mel spectrogram with shape [1, MEL_FRAMES, MEL_BINS] = [1, 500, 128].
#[derive(Debug, Clone)]
pub struct Spectrogram {
    /// The spectrogram data as a 3D array [batch, frames, bins]
    pub data: Array3<f32>,

    /// Associated segment ID
    pub segment_id: SegmentId,

    /// Additional metadata
    pub metadata: SpectrogramMetadata,
}

/// Metadata about the spectrogram
#[derive(Debug, Clone, Default)]
pub struct SpectrogramMetadata {
    /// Sample rate of the original audio
    pub sample_rate: Option<u32>,

    /// Duration of the audio segment in seconds
    pub duration_secs: Option<f32>,

    /// SNR of the audio segment
    pub snr: Option<f32>,
}

impl Spectrogram {
    /// Create a new spectrogram from raw data.
    ///
    /// # Arguments
    ///
    /// * `data` - 2D array of shape [MEL_FRAMES, MEL_BINS] (will be expanded to 3D)
    /// * `segment_id` - ID of the source audio segment
    ///
    /// # Errors
    ///
    /// Returns an error if the data dimensions are incorrect.
    pub fn new(
        data: ndarray::Array2<f32>,
        segment_id: SegmentId,
    ) -> Result<Self, EmbeddingError> {
        let shape = data.shape();
        if shape[0] != MEL_FRAMES || shape[1] != MEL_BINS {
            return Err(EmbeddingError::InvalidDimensions {
                expected: MEL_FRAMES * MEL_BINS,
                actual: shape[0] * shape[1],
            });
        }

        // Expand to 3D: [1, frames, bins]
        let data = data.insert_axis(ndarray::Axis(0));

        Ok(Self {
            data,
            segment_id,
            metadata: SpectrogramMetadata::default(),
        })
    }

    /// Create from a 3D array directly
    pub fn from_array3(data: Array3<f32>, segment_id: SegmentId) -> Result<Self, EmbeddingError> {
        let shape = data.shape();
        if shape[1] != MEL_FRAMES || shape[2] != MEL_BINS {
            return Err(EmbeddingError::InvalidDimensions {
                expected: MEL_FRAMES * MEL_BINS,
                actual: shape[1] * shape[2],
            });
        }

        Ok(Self {
            data,
            segment_id,
            metadata: SpectrogramMetadata::default(),
        })
    }

    /// Set metadata for the spectrogram
    pub fn with_metadata(mut self, metadata: SpectrogramMetadata) -> Self {
        self.metadata = metadata;
        self
    }
}

/// Output from the embedding service
#[derive(Debug, Clone)]
pub struct EmbeddingOutput {
    /// The generated embedding
    pub embedding: Embedding,

    /// Whether GPU was used for inference
    pub gpu_used: bool,

    /// Inference latency in milliseconds
    pub latency_ms: f32,
}

/// Configuration for the embedding service
#[derive(Debug, Clone)]
pub struct EmbeddingServiceConfig {
    /// Maximum batch size for inference
    pub batch_size: usize,

    /// Whether to L2 normalize embeddings
    pub normalize: bool,

    /// Default storage tier for new embeddings
    pub default_tier: StorageTier,

    /// Whether to validate embeddings after generation
    pub validate_embeddings: bool,

    /// Maximum allowed sparsity (fraction of near-zero values)
    pub max_sparsity: f32,
}

impl Default for EmbeddingServiceConfig {
    fn default() -> Self {
        Self {
            batch_size: 8,
            normalize: true,
            default_tier: StorageTier::Hot,
            validate_embeddings: true,
            max_sparsity: 0.9,
        }
    }
}

/// Service for generating embeddings from spectrograms.
///
/// This is the main application service for the embedding bounded context.
/// It coordinates between the model manager, ONNX inference, and domain entities.
pub struct EmbeddingService {
    /// Model manager for loading and caching ONNX models
    model_manager: Arc<ModelManager>,

    /// Configuration for the service
    config: EmbeddingServiceConfig,
}

impl EmbeddingService {
    /// Create a new embedding service.
    ///
    /// # Arguments
    ///
    /// * `model_manager` - The model manager for ONNX model access
    /// * `batch_size` - Maximum batch size for inference
    #[must_use]
    pub fn new(model_manager: Arc<ModelManager>, batch_size: usize) -> Self {
        Self {
            model_manager,
            config: EmbeddingServiceConfig {
                batch_size,
                ..Default::default()
            },
        }
    }

    /// Create with custom configuration
    #[must_use]
    pub fn with_config(model_manager: Arc<ModelManager>, config: EmbeddingServiceConfig) -> Self {
        Self {
            model_manager,
            config,
        }
    }

    /// Generate an embedding from a single spectrogram.
    ///
    /// # Arguments
    ///
    /// * `spectrogram` - The input spectrogram
    ///
    /// # Errors
    ///
    /// Returns an error if inference fails or the embedding is invalid.
    #[instrument(skip(self, spectrogram), fields(segment_id = %spectrogram.segment_id))]
    pub async fn embed_segment(
        &self,
        spectrogram: &Spectrogram,
    ) -> Result<EmbeddingOutput, EmbeddingError> {
        let start = Instant::now();

        // Get the inference session
        let inference = self.model_manager.get_inference().await?;
        let model_version = self.model_manager.current_version();

        // Run inference
        let raw_embedding = inference.run(&spectrogram.data)?;

        // Convert to vector
        let mut vector: Vec<f32> = raw_embedding.iter().copied().collect();

        // Calculate original norm before normalization
        let original_norm = normalization::compute_norm(&vector);

        // L2 normalize if configured
        if self.config.normalize {
            normalization::l2_normalize(&mut vector);
        }

        // Validate embedding
        if self.config.validate_embeddings {
            self.validate_embedding(&vector)?;
        }

        // Calculate sparsity
        let sparsity = normalization::compute_sparsity(&vector);

        // Create embedding entity
        let mut embedding = Embedding::new(
            spectrogram.segment_id,
            vector,
            model_version.full_version(),
        )?;

        // Set metadata
        let latency_ms = start.elapsed().as_secs_f32() * 1000.0;
        embedding.metadata = EmbeddingMetadata {
            inference_latency_ms: Some(latency_ms),
            batch_id: None,
            gpu_used: inference.is_gpu(),
            original_norm: Some(original_norm),
            sparsity: Some(sparsity),
            quality_score: Some(self.compute_quality_score(&embedding)),
        };

        embedding.tier = self.config.default_tier;

        debug!(
            latency_ms = latency_ms,
            norm = embedding.norm(),
            sparsity = sparsity,
            "Generated embedding"
        );

        Ok(EmbeddingOutput {
            embedding,
            gpu_used: inference.is_gpu(),
            latency_ms,
        })
    }

    /// Generate embeddings for multiple spectrograms in batches.
    ///
    /// This is more efficient than calling `embed_segment` multiple times
    /// as it uses batched inference.
    ///
    /// # Arguments
    ///
    /// * `spectrograms` - Slice of input spectrograms
    ///
    /// # Errors
    ///
    /// Returns an error if any inference fails. Partial results are not returned.
    #[instrument(skip(self, spectrograms), fields(count = spectrograms.len()))]
    pub async fn embed_batch(
        &self,
        spectrograms: &[Spectrogram],
    ) -> Result<Vec<EmbeddingOutput>, EmbeddingError> {
        if spectrograms.is_empty() {
            return Ok(Vec::new());
        }

        let total_start = Instant::now();
        let batch_id = uuid::Uuid::new_v4().to_string();

        info!(
            batch_id = %batch_id,
            total_segments = spectrograms.len(),
            batch_size = self.config.batch_size,
            "Starting batch embedding"
        );

        // Get the inference session
        let inference = self.model_manager.get_inference().await?;
        let model_version = self.model_manager.current_version();

        // Process in batches
        let mut all_outputs = Vec::with_capacity(spectrograms.len());

        for (batch_idx, chunk) in spectrograms.chunks(self.config.batch_size).enumerate() {
            let batch_start = Instant::now();

            // Prepare batch input
            let inputs: Vec<&Array3<f32>> = chunk.iter().map(|s| &s.data).collect();

            // Run batched inference
            let raw_embeddings = inference.run_batch(&inputs)?;

            let batch_latency_ms = batch_start.elapsed().as_secs_f32() * 1000.0;
            let per_item_latency = batch_latency_ms / chunk.len() as f32;

            // Process each embedding in the batch (parallelize normalization)
            let outputs: Vec<Result<EmbeddingOutput, EmbeddingError>> = chunk
                .par_iter()
                .zip(raw_embeddings.par_iter())
                .map(|(spectrogram, raw_emb)| {
                    let mut vector: Vec<f32> = raw_emb.iter().copied().collect();
                    let original_norm = normalization::compute_norm(&vector);

                    if self.config.normalize {
                        normalization::l2_normalize(&mut vector);
                    }

                    if self.config.validate_embeddings {
                        self.validate_embedding(&vector)?;
                    }

                    let sparsity = normalization::compute_sparsity(&vector);

                    let mut embedding = Embedding::new(
                        spectrogram.segment_id,
                        vector,
                        model_version.full_version(),
                    )?;

                    embedding.metadata = EmbeddingMetadata {
                        inference_latency_ms: Some(per_item_latency),
                        batch_id: Some(batch_id.clone()),
                        gpu_used: inference.is_gpu(),
                        original_norm: Some(original_norm),
                        sparsity: Some(sparsity),
                        quality_score: Some(self.compute_quality_score(&embedding)),
                    };

                    embedding.tier = self.config.default_tier;

                    Ok(EmbeddingOutput {
                        embedding,
                        gpu_used: inference.is_gpu(),
                        latency_ms: per_item_latency,
                    })
                })
                .collect();

            // Check for errors
            let batch_outputs: Result<Vec<_>, _> = outputs.into_iter().collect();
            all_outputs.extend(batch_outputs?);

            debug!(
                batch_idx = batch_idx,
                batch_size = chunk.len(),
                latency_ms = batch_latency_ms,
                "Completed batch"
            );
        }

        let total_latency_ms = total_start.elapsed().as_secs_f32() * 1000.0;
        let throughput = spectrograms.len() as f32 / (total_latency_ms / 1000.0);

        info!(
            batch_id = %batch_id,
            total_segments = spectrograms.len(),
            total_latency_ms = total_latency_ms,
            throughput_per_sec = throughput,
            "Completed batch embedding"
        );

        Ok(all_outputs)
    }

    /// Create a batch tracking object for monitoring progress.
    #[must_use]
    pub fn create_batch(&self, segment_ids: Vec<SegmentId>) -> EmbeddingBatch {
        EmbeddingBatch::new(segment_ids)
    }

    /// Validate an embedding vector.
    fn validate_embedding(&self, vector: &[f32]) -> Result<(), EmbeddingError> {
        // Check dimensions
        if vector.len() != EMBEDDING_DIM {
            return Err(EmbeddingError::InvalidDimensions {
                expected: EMBEDDING_DIM,
                actual: vector.len(),
            });
        }

        // Check for NaN values
        if vector.iter().any(|x| x.is_nan()) {
            return Err(EmbeddingError::Validation(
                "Embedding contains NaN values".to_string(),
            ));
        }

        // Check for infinite values
        if vector.iter().any(|x| x.is_infinite()) {
            return Err(EmbeddingError::Validation(
                "Embedding contains infinite values".to_string(),
            ));
        }

        // Check sparsity
        let sparsity = normalization::compute_sparsity(vector);
        if sparsity > self.config.max_sparsity {
            warn!(
                sparsity = sparsity,
                max_sparsity = self.config.max_sparsity,
                "Embedding has high sparsity"
            );
        }

        Ok(())
    }

    /// Compute a quality score for an embedding.
    fn compute_quality_score(&self, embedding: &Embedding) -> f32 {
        let mut score = 1.0_f32;

        // Penalize deviation from unit norm
        let norm = embedding.norm();
        let norm_deviation = (norm - 1.0).abs();
        score -= norm_deviation * 0.5;

        // Penalize high sparsity
        if let Some(sparsity) = embedding.metadata.sparsity {
            score -= sparsity * 0.3;
        }

        score.clamp(0.0, 1.0)
    }

    /// Get the current model version being used.
    #[must_use]
    pub fn model_version(&self) -> String {
        self.model_manager.current_version().full_version()
    }

    /// Check if the service is ready for inference.
    pub async fn is_ready(&self) -> bool {
        self.model_manager.is_ready().await
    }
}

/// Builder for creating embedding service instances
#[derive(Debug)]
pub struct EmbeddingServiceBuilder {
    model_manager: Option<Arc<ModelManager>>,
    config: EmbeddingServiceConfig,
}

impl EmbeddingServiceBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self {
            model_manager: None,
            config: EmbeddingServiceConfig::default(),
        }
    }

    /// Set the model manager
    #[must_use]
    pub fn model_manager(mut self, manager: Arc<ModelManager>) -> Self {
        self.model_manager = Some(manager);
        self
    }

    /// Set the batch size
    #[must_use]
    pub fn batch_size(mut self, size: usize) -> Self {
        self.config.batch_size = size;
        self
    }

    /// Set whether to normalize embeddings
    #[must_use]
    pub fn normalize(mut self, normalize: bool) -> Self {
        self.config.normalize = normalize;
        self
    }

    /// Set the default storage tier
    #[must_use]
    pub fn default_tier(mut self, tier: StorageTier) -> Self {
        self.config.default_tier = tier;
        self
    }

    /// Set whether to validate embeddings
    #[must_use]
    pub fn validate_embeddings(mut self, validate: bool) -> Self {
        self.config.validate_embeddings = validate;
        self
    }

    /// Build the embedding service
    ///
    /// # Errors
    ///
    /// Returns an error if the model manager is not set.
    pub fn build(self) -> Result<EmbeddingService, EmbeddingError> {
        let model_manager = self.model_manager.ok_or_else(|| {
            EmbeddingError::Validation("Model manager is required".to_string())
        })?;

        Ok(EmbeddingService::with_config(model_manager, self.config))
    }
}

impl Default for EmbeddingServiceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array2;

    #[test]
    fn test_spectrogram_creation() {
        let data = Array2::zeros((MEL_FRAMES, MEL_BINS));
        let segment_id = SegmentId::new();
        let spec = Spectrogram::new(data, segment_id);
        assert!(spec.is_ok());
    }

    #[test]
    fn test_spectrogram_invalid_dimensions() {
        let data = Array2::zeros((100, 100)); // Wrong dimensions
        let segment_id = SegmentId::new();
        let spec = Spectrogram::new(data, segment_id);
        assert!(spec.is_err());
    }

    #[test]
    fn test_service_config_default() {
        let config = EmbeddingServiceConfig::default();
        assert_eq!(config.batch_size, 8);
        assert!(config.normalize);
        assert!(config.validate_embeddings);
    }

    #[test]
    fn test_service_builder() {
        let builder = EmbeddingServiceBuilder::new()
            .batch_size(16)
            .normalize(false)
            .default_tier(StorageTier::Warm);

        assert_eq!(builder.config.batch_size, 16);
        assert!(!builder.config.normalize);
        assert_eq!(builder.config.default_tier, StorageTier::Warm);
    }
}
