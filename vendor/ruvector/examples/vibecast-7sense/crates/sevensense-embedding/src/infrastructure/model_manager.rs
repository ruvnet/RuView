//! Model management for ONNX embedding models.
//!
//! Provides thread-safe loading, caching, and hot-swapping of
//! Perch 2.0 ONNX models for embedding generation.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::RwLock;
use sha2::{Digest, Sha256};
use thiserror::Error;
use tracing::{debug, info, instrument, warn};

use super::onnx_inference::OnnxInference;
use crate::domain::entities::{EmbeddingModel, ModelVersion};

/// Errors that can occur during model management
#[derive(Debug, Error)]
pub enum ModelError {
    /// Model file not found
    #[error("Model not found: {0}")]
    NotFound(String),

    /// Failed to load model
    #[error("Failed to load model: {0}")]
    LoadFailed(String),

    /// Checksum verification failed
    #[error("Checksum mismatch for model {model}: expected {expected}, got {actual}")]
    ChecksumMismatch {
        /// Model name
        model: String,
        /// Expected checksum
        expected: String,
        /// Actual checksum
        actual: String,
    },

    /// Model initialization failed
    #[error("Model initialization failed: {0}")]
    InitializationFailed(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// ONNX Runtime error
    #[error("ONNX Runtime error: {0}")]
    OnnxRuntime(String),

    /// Model not ready
    #[error("Model not ready: {0}")]
    NotReady(String),
}

/// Configuration for the model manager
#[derive(Debug, Clone)]
pub struct ModelConfig {
    /// Directory containing model files
    pub model_dir: PathBuf,

    /// Number of threads for intra-op parallelism
    pub intra_op_threads: usize,

    /// Number of threads for inter-op parallelism
    pub inter_op_threads: usize,

    /// Whether to verify model checksums on load
    pub verify_checksums: bool,

    /// Execution providers in priority order
    pub execution_providers: Vec<ExecutionProvider>,

    /// Maximum number of cached sessions
    pub max_cached_sessions: usize,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            model_dir: PathBuf::from("models"),
            intra_op_threads: num_cpus::get().min(4),
            inter_op_threads: 1,
            verify_checksums: true,
            execution_providers: vec![
                ExecutionProvider::Cuda { device_id: 0 },
                ExecutionProvider::CoreML,
                ExecutionProvider::Cpu,
            ],
            max_cached_sessions: 4,
        }
    }
}

/// Execution provider for ONNX Runtime
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionProvider {
    /// CPU execution
    Cpu,

    /// NVIDIA CUDA execution
    Cuda {
        /// GPU device ID
        device_id: i32,
    },

    /// Apple CoreML execution
    CoreML,

    /// DirectML execution (Windows)
    DirectML {
        /// Device ID
        device_id: i32,
    },
}

/// Thread-safe model session manager with caching and hot-swap support.
///
/// Manages the lifecycle of ONNX models used for embedding generation,
/// including loading, caching, and version management.
pub struct ModelManager {
    /// Cached model sessions by version
    sessions: RwLock<HashMap<String, Arc<OnnxInference>>>,

    /// Model metadata by version
    models: RwLock<HashMap<String, EmbeddingModel>>,

    /// Currently active model version
    active_version: RwLock<ModelVersion>,

    /// Configuration
    config: ModelConfig,
}

impl ModelManager {
    /// Create a new model manager with the given configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the model directory doesn't exist and can't be created.
    pub fn new(config: ModelConfig) -> Result<Self, ModelError> {
        // Ensure model directory exists
        if !config.model_dir.exists() {
            std::fs::create_dir_all(&config.model_dir)?;
            debug!(path = ?config.model_dir, "Created model directory");
        }

        Ok(Self {
            sessions: RwLock::new(HashMap::new()),
            models: RwLock::new(HashMap::new()),
            active_version: RwLock::new(ModelVersion::perch_v2_base()),
            config,
        })
    }

    /// Create with default configuration
    pub fn with_defaults() -> Result<Self, ModelError> {
        Self::new(ModelConfig::default())
    }

    /// Load a model from a file.
    ///
    /// # Arguments
    ///
    /// * `name` - Model name (e.g., "perch-v2")
    ///
    /// # Errors
    ///
    /// Returns an error if the model file doesn't exist or fails to load.
    #[instrument(skip(self), fields(model = %name))]
    pub fn load_model(&self, name: &str) -> Result<Arc<OnnxInference>, ModelError> {
        let version = self.active_version.read().clone();
        let version_key = version.full_version();

        // Check cache first
        {
            let sessions = self.sessions.read();
            if let Some(session) = sessions.get(&version_key) {
                debug!("Using cached session for {}", version_key);
                return Ok(Arc::clone(session));
            }
        }

        // Resolve model path
        let model_path = self.resolve_model_path(name, &version)?;

        // Verify checksum if configured
        if self.config.verify_checksums {
            if let Some(model) = self.models.read().get(&version_key) {
                if !model.checksum.is_empty() {
                    self.verify_checksum(&model_path, &model.checksum)?;
                }
            }
        }

        // Create new session
        info!(path = ?model_path, "Loading model");
        let session = self.create_session(&model_path)?;
        let session = Arc::new(session);

        // Cache the session
        {
            let mut sessions = self.sessions.write();

            // Evict old sessions if at capacity
            while sessions.len() >= self.config.max_cached_sessions {
                if let Some(key) = sessions.keys().next().cloned() {
                    sessions.remove(&key);
                    debug!("Evicted cached session: {}", key);
                }
            }

            sessions.insert(version_key.clone(), Arc::clone(&session));
        }

        // Update model metadata
        {
            let mut models = self.models.write();
            if let Some(model) = models.get_mut(&version_key) {
                model.mark_active();
            }
        }

        info!(version = %version_key, "Model loaded successfully");
        Ok(session)
    }

    /// Verify the checksum of a model file.
    ///
    /// # Errors
    ///
    /// Returns an error if the checksum doesn't match.
    pub fn verify_checksum(&self, path: &Path, expected: &str) -> Result<bool, ModelError> {
        let actual = self.compute_checksum(path)?;

        if actual != expected {
            return Err(ModelError::ChecksumMismatch {
                model: path.display().to_string(),
                expected: expected.to_string(),
                actual,
            });
        }

        debug!(path = ?path, "Checksum verified");
        Ok(true)
    }

    /// Compute the SHA-256 checksum of a file.
    fn compute_checksum(&self, path: &Path) -> Result<String, ModelError> {
        let mut file = std::fs::File::open(path)?;
        let mut hasher = Sha256::new();
        std::io::copy(&mut file, &mut hasher)?;
        let hash = hasher.finalize();
        Ok(hex::encode(hash))
    }

    /// Hot-swap to a new model version without restart.
    ///
    /// # Arguments
    ///
    /// * `name` - Model name
    /// * `new_path` - Path to the new model file
    ///
    /// # Errors
    ///
    /// Returns an error if the new model fails to load.
    #[instrument(skip(self, new_path), fields(model = %name, path = ?new_path))]
    pub fn hot_swap(&self, name: &str, new_path: &Path) -> Result<(), ModelError> {
        // Validate the new model can be loaded
        info!("Attempting hot-swap to new model");
        let new_session = self.create_session(new_path)?;

        // Compute checksum for the new model
        let checksum = self.compute_checksum(new_path)?;

        // Create new version
        let old_version = self.active_version.read().clone();
        let new_version = ModelVersion::new(
            name,
            &old_version.version, // Keep same semantic version
            "hot-swap",
        );
        let version_key = new_version.full_version();

        // Update sessions cache
        {
            let mut sessions = self.sessions.write();
            sessions.insert(version_key.clone(), Arc::new(new_session));
        }

        // Update model metadata
        {
            let mut models = self.models.write();
            let mut model = EmbeddingModel::new(
                name.to_string(),
                new_version.clone(),
                checksum,
            );
            model.model_path = Some(new_path.to_string_lossy().to_string());
            model.mark_active();
            models.insert(version_key, model);
        }

        // Update active version
        *self.active_version.write() = new_version.clone();

        info!(
            old_version = %old_version,
            new_version = %new_version,
            "Hot-swap completed successfully"
        );

        Ok(())
    }

    /// Get the ONNX inference engine for the current model.
    ///
    /// # Errors
    ///
    /// Returns an error if no model is loaded.
    pub async fn get_inference(&self) -> Result<Arc<OnnxInference>, ModelError> {
        let version = self.active_version.read().clone();
        self.load_model(&version.name)
    }

    /// Get the currently active model version.
    #[must_use]
    pub fn current_version(&self) -> ModelVersion {
        self.active_version.read().clone()
    }

    /// Set the active model version.
    pub fn set_active_version(&self, version: ModelVersion) {
        *self.active_version.write() = version;
    }

    /// Check if a model is loaded and ready.
    pub async fn is_ready(&self) -> bool {
        let version_key = self.active_version.read().full_version();
        self.sessions.read().contains_key(&version_key)
    }

    /// Get model metadata for a version.
    #[must_use]
    pub fn get_model(&self, version_key: &str) -> Option<EmbeddingModel> {
        self.models.read().get(version_key).cloned()
    }

    /// List all loaded models.
    #[must_use]
    pub fn list_models(&self) -> Vec<EmbeddingModel> {
        self.models.read().values().cloned().collect()
    }

    /// Clear all cached sessions.
    pub fn clear_cache(&self) {
        self.sessions.write().clear();
        info!("Cleared model session cache");
    }

    /// Resolve the path to a model file.
    fn resolve_model_path(&self, name: &str, version: &ModelVersion) -> Result<PathBuf, ModelError> {
        // Try various naming conventions
        let candidates = vec![
            self.config.model_dir.join(format!("{}.onnx", version.full_version())),
            self.config.model_dir.join(format!("{}_{}.onnx", name, version.version)),
            self.config.model_dir.join(format!("{}.onnx", name)),
            self.config.model_dir.join(format!("{}/{}.onnx", name, version.version)),
        ];

        for path in &candidates {
            if path.exists() {
                return Ok(path.clone());
            }
        }

        // Also check if there's a model metadata entry with a path
        let version_key = version.full_version();
        if let Some(model) = self.models.read().get(&version_key) {
            if let Some(ref path_str) = model.model_path {
                let path = PathBuf::from(path_str);
                if path.exists() {
                    return Ok(path);
                }
            }
        }

        Err(ModelError::NotFound(format!(
            "Model {} not found in {:?}. Tried: {:?}",
            name, self.config.model_dir, candidates
        )))
    }

    /// Create an ONNX inference session from a model file.
    fn create_session(&self, path: &Path) -> Result<OnnxInference, ModelError> {
        OnnxInference::new(
            path,
            self.config.intra_op_threads,
            self.config.inter_op_threads,
            &self.config.execution_providers,
        )
        .map_err(|e| ModelError::LoadFailed(e.to_string()))
    }

    /// Register a model without loading it.
    pub fn register_model(&self, model: EmbeddingModel) {
        let version_key = model.version.full_version();
        self.models.write().insert(version_key, model);
    }

    /// Unload a specific model version from cache.
    pub fn unload_model(&self, version_key: &str) -> bool {
        let removed = self.sessions.write().remove(version_key).is_some();
        if removed {
            info!(version = %version_key, "Unloaded model from cache");
        }
        removed
    }
}

impl std::fmt::Debug for ModelManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelManager")
            .field("model_dir", &self.config.model_dir)
            .field("active_version", &*self.active_version.read())
            .field("cached_sessions", &self.sessions.read().len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_model_config_default() {
        let config = ModelConfig::default();
        assert!(config.intra_op_threads > 0);
        assert!(config.verify_checksums);
    }

    #[test]
    fn test_model_manager_creation() {
        let dir = tempdir().unwrap();
        let config = ModelConfig {
            model_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        let manager = ModelManager::new(config);
        assert!(manager.is_ok());
    }

    #[test]
    fn test_checksum_computation() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.bin");

        let mut file = std::fs::File::create(&file_path).unwrap();
        file.write_all(b"test content").unwrap();

        let config = ModelConfig {
            model_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        let manager = ModelManager::new(config).unwrap();

        let checksum = manager.compute_checksum(&file_path).unwrap();
        assert!(!checksum.is_empty());
        assert_eq!(checksum.len(), 64); // SHA-256 hex length
    }

    #[test]
    fn test_model_version_key() {
        let version = ModelVersion::perch_v2_base();
        assert_eq!(version.full_version(), "perch-v2-2.0.0-base");
    }

    #[test]
    fn test_register_model() {
        let dir = tempdir().unwrap();
        let config = ModelConfig {
            model_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        let manager = ModelManager::new(config).unwrap();

        let model = EmbeddingModel::perch_v2_default();
        let version_key = model.version.full_version();

        manager.register_model(model);

        let retrieved = manager.get_model(&version_key);
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_clear_cache() {
        let dir = tempdir().unwrap();
        let config = ModelConfig {
            model_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        let manager = ModelManager::new(config).unwrap();

        manager.clear_cache();
        // Should not panic
    }
}
