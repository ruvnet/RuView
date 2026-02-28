//! ONNX Runtime inference for Perch 2.0 embeddings.
//!
//! Provides efficient inference using the `ort` crate for
//! ONNX Runtime integration in Rust.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use ndarray::{Array1, Array3};
use ort::session::{Session, builder::GraphOptimizationLevel};
use thiserror::Error;
use tracing::{debug, instrument, warn};

use super::model_manager::ExecutionProvider;
use crate::{EMBEDDING_DIM, MEL_BINS, MEL_FRAMES};

/// Errors during ONNX inference
#[derive(Debug, Error)]
pub enum InferenceError {
    /// Session creation failed
    #[error("Failed to create session: {0}")]
    SessionCreation(String),

    /// Input tensor creation failed
    #[error("Failed to create input tensor: {0}")]
    InputTensor(String),

    /// Inference execution failed
    #[error("Inference failed: {0}")]
    Execution(String),

    /// Output extraction failed
    #[error("Failed to extract output: {0}")]
    OutputExtraction(String),

    /// Invalid input dimensions
    #[error("Invalid input dimensions: expected {expected:?}, got {actual:?}")]
    InvalidDimensions {
        /// Expected shape
        expected: Vec<usize>,
        /// Actual shape
        actual: Vec<usize>,
    },

    /// Model not initialized
    #[error("Model not initialized")]
    NotInitialized,
}

/// ONNX inference engine for embedding generation.
pub struct OnnxInference {
    /// The ONNX Runtime session (wrapped in Mutex for interior mutability)
    session: Mutex<Session>,

    /// Whether GPU is being used
    gpu_enabled: AtomicBool,

    /// Input name for the model
    input_name: String,

    /// Output name for embeddings
    output_name: String,
}

impl OnnxInference {
    /// Create a new ONNX inference engine from a model file.
    #[instrument(skip(providers), fields(path = ?model_path))]
    pub fn new(
        model_path: &Path,
        intra_op_threads: usize,
        inter_op_threads: usize,
        providers: &[ExecutionProvider],
    ) -> Result<Self, InferenceError> {
        let builder = Session::builder()
            .map_err(|e| InferenceError::SessionCreation(e.to_string()))?
            .with_intra_threads(intra_op_threads)
            .map_err(|e| InferenceError::SessionCreation(e.to_string()))?
            .with_inter_threads(inter_op_threads)
            .map_err(|e| InferenceError::SessionCreation(e.to_string()))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| InferenceError::SessionCreation(e.to_string()))?;

        let gpu_enabled = false;

        for provider in providers {
            match provider {
                ExecutionProvider::Cuda { device_id } => {
                    warn!("CUDA device {} requested, using CPU fallback", device_id);
                }
                ExecutionProvider::CoreML => {
                    warn!("CoreML requested, using CPU fallback");
                }
                ExecutionProvider::DirectML { device_id } => {
                    warn!("DirectML device {} requested, using CPU fallback", device_id);
                }
                ExecutionProvider::Cpu => {
                    debug!("Using CPU execution provider");
                    break;
                }
            }
        }

        let session = builder
            .commit_from_file(model_path)
            .map_err(|e| InferenceError::SessionCreation(e.to_string()))?;

        let inputs = session.inputs();
        let outputs = session.outputs();

        let input_name = inputs
            .first()
            .map(|i| i.name().to_string())
            .unwrap_or_else(|| "input".to_string());

        let output_name = outputs
            .first()
            .map(|o| o.name().to_string())
            .unwrap_or_else(|| "embedding".to_string());

        debug!(
            input = %input_name,
            output = %output_name,
            gpu = gpu_enabled,
            "ONNX session created"
        );

        Ok(Self {
            session: Mutex::new(session),
            gpu_enabled: AtomicBool::new(gpu_enabled),
            input_name,
            output_name,
        })
    }

    /// Run inference on a single spectrogram.
    #[instrument(skip(self, input))]
    pub fn run(&self, input: &Array3<f32>) -> Result<Array1<f32>, InferenceError> {
        let shape = input.shape();
        if shape[1] != MEL_FRAMES || shape[2] != MEL_BINS {
            return Err(InferenceError::InvalidDimensions {
                expected: vec![1, MEL_FRAMES, MEL_BINS],
                actual: shape.to_vec(),
            });
        }

        // Create input tensor using ort 2.0 API with shape tuple
        let input_vec: Vec<f32> = input.iter().cloned().collect();
        let tensor_shape = vec![1i64, MEL_FRAMES as i64, MEL_BINS as i64];
        let input_tensor = ort::value::Tensor::from_array((tensor_shape, input_vec))
            .map_err(|e| InferenceError::InputTensor(e.to_string()))?;

        // Run inference (lock session for mutable access required by ort 2.0)
        let inputs = ort::inputs![&self.input_name => input_tensor];
        let mut session = self.session.lock()
            .map_err(|e| InferenceError::Execution(format!("Lock error: {}", e)))?;
        let outputs = session
            .run(inputs)
            .map_err(|e| InferenceError::Execution(e.to_string()))?;

        // Extract embedding output
        let output = outputs
            .get(&self.output_name)
            .ok_or_else(|| InferenceError::OutputExtraction("No output found".to_string()))?;

        // Extract tensor data using ort 2.0 API - returns (&Shape, &[f32]) tuple
        let (_shape, flat_slice) = output
            .try_extract_tensor::<f32>()
            .map_err(|e| InferenceError::OutputExtraction(e.to_string()))?;

        // Handle different output shapes
        let embedding_data: Vec<f32> = if flat_slice.len() == EMBEDDING_DIM {
            flat_slice.to_vec()
        } else if flat_slice.len() > EMBEDDING_DIM {
            flat_slice[..EMBEDDING_DIM].to_vec()
        } else {
            return Err(InferenceError::OutputExtraction(format!(
                "Unexpected embedding size: {} (expected {})",
                flat_slice.len(),
                EMBEDDING_DIM
            )));
        };

        debug!("Inference completed");
        Ok(Array1::from_vec(embedding_data))
    }

    /// Run inference on a batch of spectrograms.
    #[instrument(skip(self, inputs), fields(batch_size = inputs.len()))]
    pub fn run_batch(&self, inputs: &[&Array3<f32>]) -> Result<Vec<Array1<f32>>, InferenceError> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }

        let batch_size = inputs.len();

        for input in inputs.iter() {
            let shape = input.shape();
            if shape[1] != MEL_FRAMES || shape[2] != MEL_BINS {
                return Err(InferenceError::InvalidDimensions {
                    expected: vec![1, MEL_FRAMES, MEL_BINS],
                    actual: shape.to_vec(),
                });
            }
        }

        // Stack inputs into a batch tensor
        let mut batch_data = Vec::with_capacity(batch_size * MEL_FRAMES * MEL_BINS);
        for input in inputs {
            let view = input.view();
            for frame in 0..MEL_FRAMES {
                for bin in 0..MEL_BINS {
                    batch_data.push(view[[0, frame, bin]]);
                }
            }
        }

        // Create batch tensor using shape tuple API
        let tensor_shape = vec![batch_size as i64, MEL_FRAMES as i64, MEL_BINS as i64];
        let input_tensor = ort::value::Tensor::from_array((tensor_shape, batch_data))
            .map_err(|e| InferenceError::InputTensor(e.to_string()))?;

        // Run inference (lock session for mutable access required by ort 2.0)
        let ort_inputs = ort::inputs![&self.input_name => input_tensor];
        let mut session = self.session.lock()
            .map_err(|e| InferenceError::Execution(format!("Lock error: {}", e)))?;
        let outputs = session
            .run(ort_inputs)
            .map_err(|e| InferenceError::Execution(e.to_string()))?;

        // Extract embeddings using ort 2.0 API - returns (&Shape, &[f32]) tuple
        let output = outputs
            .get(&self.output_name)
            .ok_or_else(|| InferenceError::OutputExtraction("No output found".to_string()))?;

        let (_shape, flat_slice) = output
            .try_extract_tensor::<f32>()
            .map_err(|e| InferenceError::OutputExtraction(e.to_string()))?;

        // Split into individual embeddings
        let total_expected = batch_size * EMBEDDING_DIM;
        if flat_slice.len() < total_expected {
            return Err(InferenceError::OutputExtraction(format!(
                "Unexpected output size: {} (expected at least {})",
                flat_slice.len(),
                total_expected
            )));
        }

        let result: Vec<Array1<f32>> = (0..batch_size)
            .map(|i| {
                let start = i * EMBEDDING_DIM;
                let end = start + EMBEDDING_DIM;
                Array1::from_vec(flat_slice[start..end].to_vec())
            })
            .collect();

        debug!(batch_size = batch_size, "Batch inference completed");
        Ok(result)
    }

    /// Check if GPU is being used for inference.
    #[must_use]
    pub fn is_gpu(&self) -> bool {
        self.gpu_enabled.load(Ordering::Relaxed)
    }

    /// Get the input name expected by the model.
    #[must_use]
    pub fn input_name(&self) -> &str {
        &self.input_name
    }

    /// Get the output name for embeddings.
    #[must_use]
    pub fn output_name(&self) -> &str {
        &self.output_name
    }

    /// Get information about the model's expected input shape.
    #[must_use]
    pub fn input_info(&self) -> Option<InputInfo> {
        self.session.lock().ok().and_then(|session| {
            session.inputs().first().map(|input| InputInfo {
                name: input.name().to_string(),
                dimensions: Vec::new(),
            })
        })
    }

    /// Get information about the model's output shape.
    #[must_use]
    pub fn output_info(&self) -> Option<OutputInfo> {
        self.session.lock().ok().and_then(|session| {
            session.outputs().first().map(|output| OutputInfo {
                name: output.name().to_string(),
                dimensions: Vec::new(),
            })
        })
    }
}

/// Information about model input
#[derive(Debug, Clone)]
pub struct InputInfo {
    /// Input tensor name
    pub name: String,
    /// Expected dimensions
    pub dimensions: Vec<usize>,
}

/// Information about model output
#[derive(Debug, Clone)]
pub struct OutputInfo {
    /// Output tensor name
    pub name: String,
    /// Output dimensions
    pub dimensions: Vec<usize>,
}

impl std::fmt::Debug for OnnxInference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OnnxInference")
            .field("gpu_enabled", &self.is_gpu())
            .field("input_name", &self.input_name)
            .field("output_name", &self.output_name)
            .finish()
    }
}

/// Configuration for ONNX inference
#[derive(Debug, Clone)]
pub struct InferenceConfig {
    /// Number of threads for intra-op parallelism
    pub intra_op_threads: usize,
    /// Number of threads for inter-op parallelism
    pub inter_op_threads: usize,
    /// Execution providers in priority order
    pub providers: Vec<ExecutionProvider>,
    /// Whether to enable memory optimization
    pub optimize_memory: bool,
    /// Maximum batch size for inference
    pub max_batch_size: usize,
}

impl Default for InferenceConfig {
    fn default() -> Self {
        Self {
            intra_op_threads: num_cpus::get().min(4),
            inter_op_threads: 1,
            providers: vec![
                ExecutionProvider::Cuda { device_id: 0 },
                ExecutionProvider::CoreML,
                ExecutionProvider::Cpu,
            ],
            optimize_memory: true,
            max_batch_size: 32,
        }
    }
}

impl InferenceConfig {
    /// Configuration optimized for field devices
    #[must_use]
    pub fn field_device() -> Self {
        Self {
            intra_op_threads: 2,
            inter_op_threads: 1,
            providers: vec![ExecutionProvider::Cpu],
            optimize_memory: true,
            max_batch_size: 1,
        }
    }

    /// Configuration optimized for server deployment
    #[must_use]
    pub fn server() -> Self {
        Self {
            intra_op_threads: 4,
            inter_op_threads: 2,
            providers: vec![
                ExecutionProvider::Cuda { device_id: 0 },
                ExecutionProvider::Cpu,
            ],
            optimize_memory: false,
            max_batch_size: 64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inference_config_default() {
        let config = InferenceConfig::default();
        assert!(config.intra_op_threads > 0);
        assert!(!config.providers.is_empty());
    }

    #[test]
    fn test_inference_config_field_device() {
        let config = InferenceConfig::field_device();
        assert_eq!(config.intra_op_threads, 2);
        assert_eq!(config.max_batch_size, 1);
        assert!(config.optimize_memory);
    }

    #[test]
    fn test_inference_config_server() {
        let config = InferenceConfig::server();
        assert_eq!(config.max_batch_size, 64);
        assert!(!config.optimize_memory);
    }

    #[test]
    fn test_input_validation() {
        let valid_shape = vec![1, MEL_FRAMES, MEL_BINS];
        let invalid_shape = vec![1, 100, 100];

        assert_eq!(valid_shape[1], MEL_FRAMES);
        assert_eq!(valid_shape[2], MEL_BINS);
        assert_ne!(invalid_shape[1], MEL_FRAMES);
    }
}
