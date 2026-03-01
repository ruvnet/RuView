//! ONNX model loading and management

use crate::config::{EmbedderConfig, ExecutionProvider, ModelSource};
use crate::{EmbeddingError, PretrainedModel, Result};
use indicatif::{ProgressBar, ProgressStyle};
use ort::session::{builder::GraphOptimizationLevel, Session};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::Path;
use tracing::{debug, info, instrument, warn};

/// Information about a loaded model
#[derive(Debug, Clone)]
pub struct ModelInfo {
    /// Model name or identifier
    pub name: String,
    /// Embedding dimension
    pub dimension: usize,
    /// Maximum sequence length
    pub max_seq_length: usize,
    /// Model file size in bytes
    pub file_size: u64,
    /// Model input names
    pub input_names: Vec<String>,
    /// Model output names
    pub output_names: Vec<String>,
}

/// ONNX model wrapper with inference capabilities
pub struct OnnxModel {
    session: Session,
    info: ModelInfo,
}

impl OnnxModel {
    /// Load model from configuration
    #[instrument(skip_all)]
    pub async fn from_config(config: &EmbedderConfig) -> Result<Self> {
        match &config.model_source {
            ModelSource::Local {
                model_path,
                tokenizer_path: _,
            } => Self::from_file(model_path, config).await,

            ModelSource::Pretrained(model) => Self::from_pretrained(*model, config).await,

            ModelSource::HuggingFace { model_id, revision } => {
                Self::from_huggingface(model_id, revision.as_deref(), config).await
            }

            ModelSource::Url {
                model_url,
                tokenizer_url: _,
            } => Self::from_url(model_url, config).await,
        }
    }

    /// Load model from a local ONNX file
    #[instrument(skip_all, fields(path = %path.as_ref().display()))]
    pub async fn from_file(path: impl AsRef<Path>, config: &EmbedderConfig) -> Result<Self> {
        let path = path.as_ref();
        info!("Loading ONNX model from file: {}", path.display());

        if !path.exists() {
            return Err(EmbeddingError::model_not_found(path.display().to_string()));
        }

        let file_size = fs::metadata(path)?.len();
        let session = Self::create_session(path, config)?;
        let info = Self::extract_model_info(&session, path, file_size)?;

        Ok(Self { session, info })
    }

    /// Load a pretrained model (downloads if not cached)
    #[instrument(skip_all, fields(model = ?model))]
    pub async fn from_pretrained(model: PretrainedModel, config: &EmbedderConfig) -> Result<Self> {
        let model_id = model.model_id();
        info!("Loading pretrained model: {}", model_id);

        // Check cache first
        let cache_path = config.cache_dir.join(sanitize_model_id(model_id));
        let model_path = cache_path.join("model.onnx");

        if model_path.exists() {
            debug!("Found cached model at {}", model_path.display());
            return Self::from_file(&model_path, config).await;
        }

        // Download from HuggingFace
        Self::from_huggingface(model_id, None, config).await
    }

    /// Load model from HuggingFace Hub
    #[instrument(skip_all, fields(model_id = %model_id))]
    pub async fn from_huggingface(
        model_id: &str,
        revision: Option<&str>,
        config: &EmbedderConfig,
    ) -> Result<Self> {
        let cache_path = config.cache_dir.join(sanitize_model_id(model_id));
        fs::create_dir_all(&cache_path)?;

        let model_path = cache_path.join("model.onnx");

        if !model_path.exists() {
            info!("Downloading model from HuggingFace: {}", model_id);
            download_from_huggingface(model_id, revision, &cache_path, config.show_progress)
                .await?;
        }

        Self::from_file(&model_path, config).await
    }

    /// Load model from a URL
    #[instrument(skip_all, fields(url = %url))]
    pub async fn from_url(url: &str, config: &EmbedderConfig) -> Result<Self> {
        let hash = hash_url(url);
        let cache_path = config.cache_dir.join(&hash);
        fs::create_dir_all(&cache_path)?;

        let model_path = cache_path.join("model.onnx");

        if !model_path.exists() {
            info!("Downloading model from URL: {}", url);
            download_file(url, &model_path, config.show_progress).await?;
        }

        Self::from_file(&model_path, config).await
    }

    /// Create an ONNX session with the specified configuration
    fn create_session(path: &Path, config: &EmbedderConfig) -> Result<Session> {
        let mut builder = Session::builder()?;

        // Set optimization level
        if config.optimize_graph {
            builder = builder.with_optimization_level(GraphOptimizationLevel::Level3)?;
        }

        // Set number of threads
        builder = builder.with_intra_threads(config.num_threads)?;

        // Configure execution provider
        match config.execution_provider {
            ExecutionProvider::Cpu => {
                // Default CPU provider
            }
            #[cfg(feature = "cuda")]
            ExecutionProvider::Cuda { device_id } => {
                builder = builder.with_execution_providers([
                    ort::execution_providers::CUDAExecutionProvider::default()
                        .with_device_id(device_id)
                        .build(),
                ])?;
            }
            #[cfg(feature = "tensorrt")]
            ExecutionProvider::TensorRt { device_id } => {
                builder = builder.with_execution_providers([
                    ort::execution_providers::TensorRTExecutionProvider::default()
                        .with_device_id(device_id)
                        .build(),
                ])?;
            }
            #[cfg(feature = "coreml")]
            ExecutionProvider::CoreMl => {
                builder = builder.with_execution_providers([
                    ort::execution_providers::CoreMLExecutionProvider::default().build(),
                ])?;
            }
            _ => {
                warn!(
                    "Requested execution provider not available, falling back to CPU"
                );
            }
        }

        let session = builder.commit_from_file(path)?;
        Ok(session)
    }

    /// Extract model information from the session
    fn extract_model_info(session: &Session, path: &Path, file_size: u64) -> Result<ModelInfo> {
        let inputs: Vec<String> = session.inputs.iter().map(|i| i.name.clone()).collect();
        let outputs: Vec<String> = session.outputs.iter().map(|o| o.name.clone()).collect();

        // Default embedding dimension (will be determined at runtime from actual output)
        // Most sentence-transformers models output 384 dimensions
        let dimension = 384;

        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        Ok(ModelInfo {
            name,
            dimension,
            max_seq_length: 512,
            file_size,
            input_names: inputs,
            output_names: outputs,
        })
    }

    /// Run inference on encoded inputs
    #[instrument(skip_all, fields(batch_size, seq_length))]
    pub fn run(
        &mut self,
        input_ids: &[i64],
        attention_mask: &[i64],
        token_type_ids: &[i64],
        shape: &[usize],
    ) -> Result<Vec<Vec<f32>>> {
        use ort::value::Tensor;

        let batch_size = shape[0];
        let seq_length = shape[1];

        debug!(
            "Running inference: batch_size={}, seq_length={}",
            batch_size, seq_length
        );

        // Create input tensors using ort's Tensor type
        let input_ids_tensor = Tensor::from_array((
            vec![batch_size, seq_length],
            input_ids.to_vec().into_boxed_slice(),
        ))
        .map_err(|e| EmbeddingError::invalid_model(e.to_string()))?;

        let attention_mask_tensor = Tensor::from_array((
            vec![batch_size, seq_length],
            attention_mask.to_vec().into_boxed_slice(),
        ))
        .map_err(|e| EmbeddingError::invalid_model(e.to_string()))?;

        let token_type_ids_tensor = Tensor::from_array((
            vec![batch_size, seq_length],
            token_type_ids.to_vec().into_boxed_slice(),
        ))
        .map_err(|e| EmbeddingError::invalid_model(e.to_string()))?;

        // Build inputs vector
        let inputs = vec![
            ("input_ids", input_ids_tensor.into_dyn()),
            ("attention_mask", attention_mask_tensor.into_dyn()),
            ("token_type_ids", token_type_ids_tensor.into_dyn()),
        ];

        // Run inference
        let outputs = self.session.run(inputs)
            .map_err(EmbeddingError::OnnxRuntime)?;

        // Extract output tensor
        // Usually the output is [batch, seq_len, hidden_size] or [batch, hidden_size]
        let output_names = ["last_hidden_state", "output", "sentence_embedding"];

        // Find the appropriate output by name, or use the first one
        let output_iter: Vec<_> = outputs.iter().collect();
        let output = output_iter
            .iter()
            .find(|(name, _)| output_names.contains(name))
            .or_else(|| output_iter.first())
            .map(|(_, v)| v)
            .ok_or_else(|| EmbeddingError::invalid_model("No output tensor found"))?;

        // In ort 2.0, try_extract_tensor returns (&Shape, &[f32])
        let (tensor_shape, tensor_data) = output
            .try_extract_tensor::<f32>()
            .map_err(|e| EmbeddingError::invalid_model(e.to_string()))?;

        // Convert Shape to Vec<usize> - Shape yields i64
        let dims: Vec<usize> = tensor_shape.iter().map(|&d| d as usize).collect();

        // Handle different output shapes
        let embeddings = if dims.len() == 3 {
            // [batch, seq_len, hidden] - need pooling
            let hidden_size = dims[2];
            (0..batch_size)
                .map(|i| {
                    let start = i * seq_length * hidden_size;
                    let end = start + seq_length * hidden_size;
                    tensor_data[start..end].to_vec()
                })
                .collect()
        } else if dims.len() == 2 {
            // [batch, hidden] - already pooled
            let hidden_size = dims[1];
            (0..batch_size)
                .map(|i| {
                    let start = i * hidden_size;
                    let end = start + hidden_size;
                    tensor_data[start..end].to_vec()
                })
                .collect()
        } else {
            return Err(EmbeddingError::invalid_model(format!(
                "Unexpected output shape: {:?}",
                dims
            )));
        };

        Ok(embeddings)
    }

    /// Get model info
    pub fn info(&self) -> &ModelInfo {
        &self.info
    }

    /// Get embedding dimension
    pub fn dimension(&self) -> usize {
        self.info.dimension
    }
}

/// Download model files from HuggingFace Hub
async fn download_from_huggingface(
    model_id: &str,
    revision: Option<&str>,
    cache_path: &Path,
    show_progress: bool,
) -> Result<()> {
    let revision = revision.unwrap_or("main");
    let base_url = format!(
        "https://huggingface.co/{}/resolve/{}",
        model_id, revision
    );

    let model_path = cache_path.join("model.onnx");

    // Try to download model.onnx - check multiple locations
    if !model_path.exists() {
        // Location 1: Root directory (model.onnx)
        let root_url = format!("{}/model.onnx", base_url);
        debug!("Trying to download model from root: {}", root_url);

        let root_result = download_file(&root_url, &model_path, show_progress).await;

        // Location 2: ONNX subfolder (onnx/model.onnx) - common for sentence-transformers
        if root_result.is_err() && !model_path.exists() {
            let onnx_url = format!("{}/onnx/model.onnx", base_url);
            debug!("Root download failed, trying onnx subfolder: {}", onnx_url);

            match download_file(&onnx_url, &model_path, show_progress).await {
                Ok(_) => debug!("Downloaded model.onnx from onnx/ subfolder"),
                Err(e) => {
                    // Both locations failed
                    return Err(EmbeddingError::download_failed(format!(
                        "Failed to download model.onnx from {} - tried both root and onnx/ subfolder: {}",
                        model_id, e
                    )));
                }
            }
        } else if let Err(e) = root_result {
            // Root failed but model exists (shouldn't happen, but handle gracefully)
            if !model_path.exists() {
                return Err(e);
            }
        } else {
            debug!("Downloaded model.onnx from root");
        }
    }

    // Download auxiliary files (tokenizer.json, config.json) - these are optional
    let aux_files = ["tokenizer.json", "config.json"];
    for file in aux_files {
        let path = cache_path.join(file);
        if !path.exists() {
            // Try root first, then onnx subfolder
            let root_url = format!("{}/{}", base_url, file);
            match download_file(&root_url, &path, show_progress).await {
                Ok(_) => debug!("Downloaded {}", file),
                Err(_) => {
                    // Try onnx subfolder
                    let onnx_url = format!("{}/onnx/{}", base_url, file);
                    match download_file(&onnx_url, &path, show_progress).await {
                        Ok(_) => debug!("Downloaded {} from onnx/ subfolder", file),
                        Err(e) => warn!("Failed to download {} (optional): {}", file, e),
                    }
                }
            }
        }
    }

    Ok(())
}

/// Download a file from URL with optional progress bar
async fn download_file(url: &str, path: &Path, show_progress: bool) -> Result<()> {
    let client = reqwest::Client::new();
    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(EmbeddingError::download_failed(format!(
            "HTTP {}: {}",
            response.status(),
            url
        )));
    }

    let total_size = response.content_length().unwrap_or(0);

    let pb = if show_progress && total_size > 0 {
        let pb = ProgressBar::new(total_size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .unwrap()
                .progress_chars("#>-"),
        );
        Some(pb)
    } else {
        None
    };

    let mut file = fs::File::create(path)?;
    let mut downloaded = 0u64;

    use futures_util::StreamExt;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk)?;
        downloaded += chunk.len() as u64;
        if let Some(ref pb) = pb {
            pb.set_position(downloaded);
        }
    }

    if let Some(pb) = pb {
        pb.finish_with_message("Downloaded");
    }

    Ok(())
}

/// Sanitize model ID for use as directory name
fn sanitize_model_id(model_id: &str) -> String {
    model_id.replace(['/', '\\', ':'], "_")
}

/// Create a hash of a URL for caching
fn hash_url(url: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    hex::encode(&hasher.finalize()[..8])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_model_id() {
        assert_eq!(
            sanitize_model_id("sentence-transformers/all-MiniLM-L6-v2"),
            "sentence-transformers_all-MiniLM-L6-v2"
        );
    }

    #[test]
    fn test_hash_url() {
        let hash = hash_url("https://example.com/model.onnx");
        assert_eq!(hash.len(), 16); // 8 bytes = 16 hex chars
    }
}
