//! Main embedder implementation combining model, tokenizer, and pooling

use crate::config::{EmbedderConfig, ModelSource, PoolingStrategy};
use crate::model::OnnxModel;
use crate::pooling::Pooler;
use crate::tokenizer::Tokenizer;
use crate::{EmbeddingError, PretrainedModel, Result};
use std::path::Path;
use tracing::{debug, info, instrument};

#[cfg(feature = "gpu")]
use crate::gpu::{GpuAccelerator, GpuConfig};

/// High-level embedder combining tokenizer, model, and pooling
pub struct Embedder {
    /// ONNX model for inference
    model: OnnxModel,
    /// Tokenizer for text processing
    tokenizer: Tokenizer,
    /// Pooler for combining token embeddings
    pooler: Pooler,
    /// Configuration
    config: EmbedderConfig,
    /// Optional GPU accelerator for similarity operations
    #[cfg(feature = "gpu")]
    gpu: Option<GpuAccelerator>,
}

/// Embedding output with metadata
#[derive(Debug, Clone)]
pub struct EmbeddingOutput {
    /// The embedding vectors
    pub embeddings: Vec<Vec<f32>>,
    /// Original input texts
    pub texts: Vec<String>,
    /// Number of tokens per input
    pub token_counts: Vec<usize>,
    /// Embedding dimension
    pub dimension: usize,
}

impl EmbeddingOutput {
    /// Get the number of embeddings
    pub fn len(&self) -> usize {
        self.embeddings.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.embeddings.is_empty()
    }

    /// Get a single embedding by index
    pub fn get(&self, index: usize) -> Option<&Vec<f32>> {
        self.embeddings.get(index)
    }

    /// Iterate over embeddings
    pub fn iter(&self) -> impl Iterator<Item = &Vec<f32>> {
        self.embeddings.iter()
    }

    /// Convert to owned vectors
    pub fn into_vecs(self) -> Vec<Vec<f32>> {
        self.embeddings
    }
}

impl Embedder {
    /// Create a new embedder from configuration
    #[instrument(skip_all)]
    pub async fn new(config: EmbedderConfig) -> Result<Self> {
        info!("Initializing embedder");

        // Load model
        let model = OnnxModel::from_config(&config).await?;

        // Load tokenizer based on model source
        let tokenizer = match &config.model_source {
            ModelSource::Local {
                tokenizer_path, ..
            } => Tokenizer::from_file(tokenizer_path, config.max_length)?,

            ModelSource::Pretrained(pretrained) => {
                Tokenizer::from_pretrained(pretrained.model_id(), config.max_length)?
            }

            ModelSource::HuggingFace { model_id, .. } => {
                Tokenizer::from_pretrained(model_id, config.max_length)?
            }

            ModelSource::Url { tokenizer_url, .. } => {
                // Download tokenizer
                let cache_path = config.cache_dir.join("tokenizer.json");
                if !cache_path.exists() {
                    download_tokenizer(tokenizer_url, &cache_path).await?;
                }
                Tokenizer::from_file(&cache_path, config.max_length)?
            }
        };

        let pooler = Pooler::new(config.pooling, config.normalize);

        // Initialize GPU accelerator if available
        #[cfg(feature = "gpu")]
        let gpu = {
            match GpuAccelerator::new(GpuConfig::auto()).await {
                Ok(accel) => {
                    info!("GPU accelerator initialized: {}", accel.device_info().name);
                    Some(accel)
                }
                Err(e) => {
                    debug!("GPU not available, using CPU: {}", e);
                    None
                }
            }
        };

        Ok(Self {
            model,
            tokenizer,
            pooler,
            config,
            #[cfg(feature = "gpu")]
            gpu,
        })
    }

    /// Create embedder with default model (all-MiniLM-L6-v2)
    pub async fn default_model() -> Result<Self> {
        Self::new(EmbedderConfig::default()).await
    }

    /// Create embedder for a specific pretrained model
    pub async fn pretrained(model: PretrainedModel) -> Result<Self> {
        Self::new(EmbedderConfig::pretrained(model)).await
    }

    /// Embed a single text
    #[instrument(skip(self, text), fields(text_len = text.len()))]
    pub fn embed_one(&mut self, text: &str) -> Result<Vec<f32>> {
        let output = self.embed(&[text])?;
        output
            .embeddings
            .into_iter()
            .next()
            .ok_or(EmbeddingError::EmptyInput)
    }

    /// Embed multiple texts
    #[instrument(skip(self, texts), fields(batch_size = texts.len()))]
    pub fn embed<S: AsRef<str>>(&mut self, texts: &[S]) -> Result<EmbeddingOutput> {
        if texts.is_empty() {
            return Err(EmbeddingError::EmptyInput);
        }

        let texts_owned: Vec<String> = texts.iter().map(|t| t.as_ref().to_string()).collect();

        // Process in batches
        let batch_size = self.config.batch_size;
        let mut all_embeddings = Vec::with_capacity(texts.len());
        let mut all_token_counts = Vec::with_capacity(texts.len());

        for chunk in texts.chunks(batch_size) {
            let (embeddings, token_counts) = self.embed_batch(chunk)?;
            all_embeddings.extend(embeddings);
            all_token_counts.extend(token_counts);
        }

        Ok(EmbeddingOutput {
            embeddings: all_embeddings,
            texts: texts_owned,
            token_counts: all_token_counts,
            dimension: self.model.dimension(),
        })
    }

    /// Embed a batch of texts (internal)
    fn embed_batch<S: AsRef<str>>(&mut self, texts: &[S]) -> Result<(Vec<Vec<f32>>, Vec<usize>)> {
        debug!("Embedding batch of {} texts", texts.len());

        // Tokenize
        let encoded = self.tokenizer.encode_batch(texts)?;
        let (input_ids, attention_mask, token_type_ids, shape) = encoded.to_onnx_inputs();

        // Run model
        let token_embeddings = self.model.run(
            &input_ids,
            &attention_mask,
            &token_type_ids,
            &shape,
        )?;

        let seq_length = shape[1];
        let hidden_size = self.model.dimension();

        // Pool embeddings
        let attention_masks: Vec<Vec<i64>> = encoded.attention_mask;
        let embeddings = self.pooler.pool(
            &token_embeddings,
            &attention_masks,
            seq_length,
            hidden_size,
        );

        let token_counts = encoded.original_lengths;

        Ok((embeddings, token_counts))
    }

    /// Embed texts (sequential processing)
    /// Note: For parallel processing, consider using tokio::spawn with multiple Embedder instances
    #[instrument(skip(self, texts), fields(total_texts = texts.len()))]
    pub fn embed_parallel<S: AsRef<str> + Sync>(&mut self, texts: &[S]) -> Result<EmbeddingOutput> {
        // Use sequential processing since ONNX session requires mutable access
        self.embed(texts)
    }

    /// Process texts one at a time (use embed for batch processing)
    pub fn embed_each<S: AsRef<str>>(&mut self, texts: &[S]) -> Vec<Result<Vec<f32>>> {
        texts.iter().map(|text| self.embed_one(text.as_ref())).collect()
    }

    /// Get the embedding dimension
    pub fn dimension(&self) -> usize {
        self.model.dimension()
    }

    /// Get model info
    pub fn model_info(&self) -> &crate::model::ModelInfo {
        self.model.info()
    }

    /// Get the pooling strategy
    pub fn pooling_strategy(&self) -> PoolingStrategy {
        self.config.pooling
    }

    /// Get max sequence length
    pub fn max_length(&self) -> usize {
        self.config.max_length
    }

    /// Compute similarity between two texts
    pub fn similarity(&mut self, text1: &str, text2: &str) -> Result<f32> {
        let emb1 = self.embed_one(text1)?;
        let emb2 = self.embed_one(text2)?;
        Ok(Pooler::cosine_similarity(&emb1, &emb2))
    }

    /// Find most similar texts from a corpus
    /// Uses GPU acceleration when available and corpus is large enough
    #[instrument(skip(self, query, corpus), fields(corpus_size = corpus.len()))]
    pub fn most_similar<S: AsRef<str>>(
        &mut self,
        query: &str,
        corpus: &[S],
        top_k: usize,
    ) -> Result<Vec<(usize, f32, String)>> {
        let query_emb = self.embed_one(query)?;
        let corpus_embs = self.embed(corpus)?;

        // Try GPU-accelerated similarity if available
        #[cfg(feature = "gpu")]
        if let Some(ref gpu) = self.gpu {
            if corpus.len() >= 64 {
                let candidates: Vec<&[f32]> = corpus_embs.embeddings.iter().map(|v| v.as_slice()).collect();
                if let Ok(results) = gpu.top_k_similar(&query_emb, &candidates, top_k) {
                    return Ok(results
                        .into_iter()
                        .map(|(idx, score)| (idx, score, corpus[idx].as_ref().to_string()))
                        .collect());
                }
            }
        }

        // CPU fallback
        let mut similarities: Vec<(usize, f32, String)> = corpus_embs
            .embeddings
            .iter()
            .enumerate()
            .map(|(i, emb)| {
                let sim = Pooler::cosine_similarity(&query_emb, emb);
                (i, sim, corpus[i].as_ref().to_string())
            })
            .collect();

        similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        similarities.truncate(top_k);

        Ok(similarities)
    }

    /// Check if GPU acceleration is available
    pub fn has_gpu(&self) -> bool {
        #[cfg(feature = "gpu")]
        {
            self.gpu.is_some()
        }
        #[cfg(not(feature = "gpu"))]
        {
            false
        }
    }

    /// Get GPU device info if available
    #[cfg(feature = "gpu")]
    pub fn gpu_info(&self) -> Option<crate::gpu::GpuInfo> {
        self.gpu.as_ref().map(|g| g.device_info())
    }

    /// Cluster texts by similarity (simple k-means-like approach)
    #[instrument(skip(self, texts), fields(n_texts = texts.len(), n_clusters))]
    pub fn cluster<S: AsRef<str>>(
        &mut self,
        texts: &[S],
        n_clusters: usize,
    ) -> Result<Vec<usize>> {
        let embeddings = self.embed(texts)?;
        let dim = self.dimension();

        // Initialize centroids with first k embeddings
        let mut centroids: Vec<Vec<f32>> = embeddings
            .embeddings
            .iter()
            .take(n_clusters)
            .cloned()
            .collect();

        let mut assignments = vec![0usize; texts.len()];
        let max_iterations = 100;

        for _ in 0..max_iterations {
            let old_assignments = assignments.clone();

            // Assign to nearest centroid
            for (i, emb) in embeddings.embeddings.iter().enumerate() {
                let mut min_dist = f32::MAX;
                let mut min_idx = 0;

                for (j, centroid) in centroids.iter().enumerate() {
                    let dist = Pooler::euclidean_distance(emb, centroid);
                    if dist < min_dist {
                        min_dist = dist;
                        min_idx = j;
                    }
                }

                assignments[i] = min_idx;
            }

            // Check convergence
            if assignments == old_assignments {
                break;
            }

            // Update centroids
            for (j, centroid) in centroids.iter_mut().enumerate() {
                let cluster_points: Vec<&Vec<f32>> = embeddings
                    .embeddings
                    .iter()
                    .zip(assignments.iter())
                    .filter(|(_, &a)| a == j)
                    .map(|(e, _)| e)
                    .collect();

                if !cluster_points.is_empty() {
                    *centroid = vec![0.0; dim];
                    for point in &cluster_points {
                        for (k, &val) in point.iter().enumerate() {
                            centroid[k] += val;
                        }
                    }
                    let count = cluster_points.len() as f32;
                    for val in centroid.iter_mut() {
                        *val /= count;
                    }
                }
            }
        }

        Ok(assignments)
    }
}

/// Download tokenizer from URL
async fn download_tokenizer(url: &str, path: &Path) -> Result<()> {
    use std::io::Write;

    let response = reqwest::get(url).await?;

    if !response.status().is_success() {
        return Err(EmbeddingError::download_failed(format!(
            "Failed to download tokenizer: HTTP {}",
            response.status()
        )));
    }

    let bytes = response.bytes().await?;
    let mut file = std::fs::File::create(path)?;
    file.write_all(&bytes)?;

    Ok(())
}

/// Builder for creating embedders with custom configurations
pub struct EmbedderBuilder {
    config: EmbedderConfig,
}

impl EmbedderBuilder {
    /// Start building an embedder
    pub fn new() -> Self {
        Self {
            config: EmbedderConfig::default(),
        }
    }

    /// Use a pretrained model
    pub fn pretrained(mut self, model: PretrainedModel) -> Self {
        self.config = EmbedderConfig::pretrained(model);
        self
    }

    /// Set pooling strategy
    pub fn pooling(mut self, strategy: PoolingStrategy) -> Self {
        self.config.pooling = strategy;
        self
    }

    /// Set normalization
    pub fn normalize(mut self, normalize: bool) -> Self {
        self.config.normalize = normalize;
        self
    }

    /// Set batch size
    pub fn batch_size(mut self, size: usize) -> Self {
        self.config.batch_size = size;
        self
    }

    /// Set max sequence length
    pub fn max_length(mut self, length: usize) -> Self {
        self.config.max_length = length;
        self
    }

    /// Build the embedder
    pub async fn build(self) -> Result<Embedder> {
        Embedder::new(self.config).await
    }
}

impl Default for EmbedderBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = EmbedderConfig::default();
        assert_eq!(config.pooling, PoolingStrategy::Mean);
        assert!(config.normalize);
    }
}
