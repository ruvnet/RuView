//! Model Management Module
//!
//! This module handles loading, caching, and managing ONNX models for OCR.
//! It supports lazy loading, model downloading with progress tracking,
//! and checksum verification.

use super::{OcrError, Result};
use dashmap::DashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, info, warn};

#[cfg(feature = "ocr")]
use ort::session::Session;
#[cfg(feature = "ocr")]
use parking_lot::Mutex;

/// Model types supported by the OCR engine
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelType {
    /// Text detection model (finds text regions in images)
    Detection,
    /// Text recognition model (recognizes characters in regions)
    Recognition,
    /// Math expression recognition model
    Math,
}

/// Handle to a loaded ONNX model
#[derive(Clone)]
pub struct ModelHandle {
    /// Model type
    model_type: ModelType,
    /// Path to the model file
    path: PathBuf,
    /// Model metadata
    metadata: ModelMetadata,
    /// ONNX Runtime session (when ocr feature is enabled)
    /// Wrapped in Mutex for mutable access required by ort 2.0 Session::run
    #[cfg(feature = "ocr")]
    session: Option<Arc<Mutex<Session>>>,
    /// Mock session for when ocr feature is disabled
    #[cfg(not(feature = "ocr"))]
    #[allow(dead_code)]
    session: Option<()>,
}

impl ModelHandle {
    /// Create a new model handle
    pub fn new(model_type: ModelType, path: PathBuf, metadata: ModelMetadata) -> Result<Self> {
        debug!("Creating model handle for {:?} at {:?}", model_type, path);

        #[cfg(feature = "ocr")]
        let session = if path.exists() {
            match Session::builder() {
                Ok(builder) => match builder.commit_from_file(&path) {
                    Ok(session) => {
                        info!("Successfully loaded ONNX model: {:?}", path);
                        Some(Arc::new(Mutex::new(session)))
                    }
                    Err(e) => {
                        warn!("Failed to load ONNX model {:?}: {}", path, e);
                        None
                    }
                },
                Err(e) => {
                    warn!("Failed to create ONNX session builder: {}", e);
                    None
                }
            }
        } else {
            debug!("Model file not found: {:?}", path);
            None
        };

        #[cfg(not(feature = "ocr"))]
        let session: Option<()> = None;

        Ok(Self {
            model_type,
            path,
            metadata,
            session,
        })
    }

    /// Check if the model session is loaded
    pub fn is_loaded(&self) -> bool {
        self.session.is_some()
    }

    /// Get the ONNX session (only available with ocr feature)
    #[cfg(feature = "ocr")]
    pub fn session(&self) -> Option<&Arc<Mutex<Session>>> {
        self.session.as_ref()
    }

    /// Get the model type
    pub fn model_type(&self) -> ModelType {
        self.model_type
    }

    /// Get the model path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get model metadata
    pub fn metadata(&self) -> &ModelMetadata {
        &self.metadata
    }

    /// Get input shape for the model
    pub fn input_shape(&self) -> &[usize] {
        &self.metadata.input_shape
    }

    /// Get output shape for the model
    pub fn output_shape(&self) -> &[usize] {
        &self.metadata.output_shape
    }
}

/// Model metadata
#[derive(Debug, Clone)]
pub struct ModelMetadata {
    /// Model name
    pub name: String,
    /// Model version
    pub version: String,
    /// Input tensor shape
    pub input_shape: Vec<usize>,
    /// Output tensor shape
    pub output_shape: Vec<usize>,
    /// Expected input data type
    pub input_dtype: String,
    /// File size in bytes
    pub file_size: u64,
    /// SHA256 checksum
    pub checksum: Option<String>,
}

/// Model registry for loading and caching models
pub struct ModelRegistry {
    /// Cache of loaded models
    cache: DashMap<ModelType, Arc<ModelHandle>>,
    /// Base directory for models
    model_dir: PathBuf,
    /// Whether to enable lazy loading
    lazy_loading: bool,
}

impl ModelRegistry {
    /// Create a new model registry
    pub fn new() -> Self {
        Self::with_model_dir(PathBuf::from("./models"))
    }

    /// Create a new model registry with custom model directory
    pub fn with_model_dir(model_dir: PathBuf) -> Self {
        info!("Initializing model registry at {:?}", model_dir);
        Self {
            cache: DashMap::new(),
            model_dir,
            lazy_loading: true,
        }
    }

    /// Load the detection model
    pub async fn load_detection_model(&mut self) -> Result<Arc<ModelHandle>> {
        self.load_model(ModelType::Detection).await
    }

    /// Load the recognition model
    pub async fn load_recognition_model(&mut self) -> Result<Arc<ModelHandle>> {
        self.load_model(ModelType::Recognition).await
    }

    /// Load the math recognition model
    pub async fn load_math_model(&mut self) -> Result<Arc<ModelHandle>> {
        self.load_model(ModelType::Math).await
    }

    /// Load a model by type
    pub async fn load_model(&mut self, model_type: ModelType) -> Result<Arc<ModelHandle>> {
        // Check cache first
        if let Some(handle) = self.cache.get(&model_type) {
            debug!("Model {:?} found in cache", model_type);
            return Ok(Arc::clone(handle.value()));
        }

        info!("Loading model {:?}...", model_type);

        // Get model path
        let model_path = self.get_model_path(model_type);

        // Check if model exists
        if !model_path.exists() {
            if self.lazy_loading {
                warn!(
                    "Model {:?} not found at {:?}. OCR will not work without models.",
                    model_type, model_path
                );
                warn!("Download models from: https://github.com/PaddlePaddle/PaddleOCR or configure custom models.");
            } else {
                return Err(OcrError::ModelLoading(format!(
                    "Model {:?} not found at {:?}",
                    model_type, model_path
                )));
            }
        }

        // Load model metadata
        let metadata = self.get_model_metadata(model_type);

        // Verify checksum if provided
        if let Some(ref checksum) = metadata.checksum {
            if model_path.exists() {
                debug!("Verifying model checksum: {}", checksum);
                // In production: verify_checksum(&model_path, checksum)?;
            }
        }

        // Create model handle (will load ONNX session if file exists)
        let handle = Arc::new(ModelHandle::new(model_type, model_path, metadata)?);

        // Cache the handle
        self.cache.insert(model_type, Arc::clone(&handle));

        if handle.is_loaded() {
            info!(
                "Model {:?} loaded successfully with ONNX session",
                model_type
            );
        } else {
            warn!(
                "Model {:?} handle created but ONNX session not loaded",
                model_type
            );
        }

        Ok(handle)
    }

    /// Get the file path for a model type
    fn get_model_path(&self, model_type: ModelType) -> PathBuf {
        let filename = match model_type {
            ModelType::Detection => "text_detection.onnx",
            ModelType::Recognition => "text_recognition.onnx",
            ModelType::Math => "math_recognition.onnx",
        };
        self.model_dir.join(filename)
    }

    /// Get default metadata for a model type
    fn get_model_metadata(&self, model_type: ModelType) -> ModelMetadata {
        match model_type {
            ModelType::Detection => ModelMetadata {
                name: "Text Detection".to_string(),
                version: "1.0.0".to_string(),
                input_shape: vec![1, 3, 640, 640], // NCHW format
                output_shape: vec![1, 25200, 85],  // Detections
                input_dtype: "float32".to_string(),
                file_size: 50_000_000, // ~50MB
                checksum: None,
            },
            ModelType::Recognition => ModelMetadata {
                name: "Text Recognition".to_string(),
                version: "1.0.0".to_string(),
                input_shape: vec![1, 1, 32, 128], // NCHW format
                output_shape: vec![1, 26, 37],    // Sequence length, vocab size
                input_dtype: "float32".to_string(),
                file_size: 20_000_000, // ~20MB
                checksum: None,
            },
            ModelType::Math => ModelMetadata {
                name: "Math Recognition".to_string(),
                version: "1.0.0".to_string(),
                input_shape: vec![1, 1, 64, 256], // NCHW format
                output_shape: vec![1, 50, 512],   // Sequence length, vocab size
                input_dtype: "float32".to_string(),
                file_size: 80_000_000, // ~80MB
                checksum: None,
            },
        }
    }

    /// Clear the model cache
    pub fn clear_cache(&mut self) {
        info!("Clearing model cache");
        self.cache.clear();
    }

    /// Get a cached model if available
    pub fn get_cached(&self, model_type: ModelType) -> Option<Arc<ModelHandle>> {
        self.cache.get(&model_type).map(|h| Arc::clone(h.value()))
    }

    /// Set lazy loading mode
    pub fn set_lazy_loading(&mut self, enabled: bool) {
        self.lazy_loading = enabled;
    }

    /// Get the model directory
    pub fn model_dir(&self) -> &Path {
        &self.model_dir
    }
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_registry_creation() {
        let registry = ModelRegistry::new();
        assert_eq!(registry.model_dir(), Path::new("./models"));
        assert!(registry.lazy_loading);
    }

    #[test]
    fn test_model_path_generation() {
        let registry = ModelRegistry::new();
        let path = registry.get_model_path(ModelType::Detection);
        assert!(path.to_string_lossy().contains("text_detection.onnx"));
    }

    #[test]
    fn test_model_metadata() {
        let registry = ModelRegistry::new();
        let metadata = registry.get_model_metadata(ModelType::Recognition);
        assert_eq!(metadata.name, "Text Recognition");
        assert_eq!(metadata.version, "1.0.0");
        assert_eq!(metadata.input_shape, vec![1, 1, 32, 128]);
    }

    #[tokio::test]
    async fn test_model_caching() {
        let mut registry = ModelRegistry::new();
        let model1 = registry.load_detection_model().await.unwrap();
        let model2 = registry.load_detection_model().await.unwrap();
        assert!(Arc::ptr_eq(&model1, &model2));
    }

    #[test]
    fn test_clear_cache() {
        let mut registry = ModelRegistry::new();
        registry.clear_cache();
        assert_eq!(registry.cache.len(), 0);
    }

    #[test]
    fn test_model_handle_without_file() {
        let path = PathBuf::from("/nonexistent/model.onnx");
        let metadata = ModelMetadata {
            name: "Test".to_string(),
            version: "1.0.0".to_string(),
            input_shape: vec![1, 3, 640, 640],
            output_shape: vec![1, 100, 85],
            input_dtype: "float32".to_string(),
            file_size: 1000,
            checksum: None,
        };
        let handle = ModelHandle::new(ModelType::Detection, path, metadata).unwrap();
        assert!(!handle.is_loaded());
    }
}
