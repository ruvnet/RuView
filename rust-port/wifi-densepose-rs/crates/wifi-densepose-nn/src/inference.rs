//! Inference engine abstraction for neural network backends.
//!
//! This module provides a unified interface for running inference across
//! different backends (ONNX Runtime, tch-rs, Candle).

use crate::densepose::{DensePoseConfig, DensePoseOutput};
use crate::error::{NnError, NnResult};
use crate::tensor::{Tensor, TensorShape};
use crate::translator::TranslatorConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument};

/// Options for inference execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceOptions {
    /// Batch size for inference
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    /// Whether to use GPU acceleration
    #[serde(default)]
    pub use_gpu: bool,
    /// GPU device ID (if using GPU)
    #[serde(default)]
    pub gpu_device_id: usize,
    /// Number of CPU threads for inference
    #[serde(default = "default_num_threads")]
    pub num_threads: usize,
    /// Enable model optimization/fusion
    #[serde(default = "default_optimize")]
    pub optimize: bool,
    /// Memory limit in bytes (0 = unlimited)
    #[serde(default)]
    pub memory_limit: usize,
    /// Enable profiling
    #[serde(default)]
    pub profiling: bool,
}

fn default_batch_size() -> usize {
    1
}

fn default_num_threads() -> usize {
    4
}

fn default_optimize() -> bool {
    true
}

impl Default for InferenceOptions {
    fn default() -> Self {
        Self {
            batch_size: default_batch_size(),
            use_gpu: false,
            gpu_device_id: 0,
            num_threads: default_num_threads(),
            optimize: default_optimize(),
            memory_limit: 0,
            profiling: false,
        }
    }
}

impl InferenceOptions {
    /// Create options for CPU inference
    pub fn cpu() -> Self {
        Self::default()
    }

    /// Create options for GPU inference
    pub fn gpu(device_id: usize) -> Self {
        Self {
            use_gpu: true,
            gpu_device_id: device_id,
            ..Default::default()
        }
    }

    /// Set batch size
    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size;
        self
    }

    /// Set number of threads
    pub fn with_threads(mut self, num_threads: usize) -> Self {
        self.num_threads = num_threads;
        self
    }
}

/// Backend trait for different inference engines
pub trait Backend: Send + Sync {
    /// Get the backend name
    fn name(&self) -> &str;

    /// Check if the backend is available
    fn is_available(&self) -> bool;

    /// Get input names
    fn input_names(&self) -> Vec<String>;

    /// Get output names
    fn output_names(&self) -> Vec<String>;

    /// Get input shape for a given input name
    fn input_shape(&self, name: &str) -> Option<TensorShape>;

    /// Get output shape for a given output name
    fn output_shape(&self, name: &str) -> Option<TensorShape>;

    /// Run inference
    fn run(&self, inputs: HashMap<String, Tensor>) -> NnResult<HashMap<String, Tensor>>;

    /// Run inference on a single input
    fn run_single(&self, input: &Tensor) -> NnResult<Tensor> {
        let input_names = self.input_names();
        let output_names = self.output_names();

        if input_names.is_empty() {
            return Err(NnError::inference("No input names defined"));
        }
        if output_names.is_empty() {
            return Err(NnError::inference("No output names defined"));
        }

        let mut inputs = HashMap::new();
        inputs.insert(input_names[0].clone(), input.clone());

        let outputs = self.run(inputs)?;
        outputs
            .into_iter()
            .next()
            .map(|(_, v)| v)
            .ok_or_else(|| NnError::inference("No outputs returned"))
    }

    /// Warm up the model (optional pre-run for optimization)
    fn warmup(&self) -> NnResult<()> {
        Ok(())
    }

    /// Get memory usage in bytes
    fn memory_usage(&self) -> usize {
        0
    }
}

/// Mock backend for testing
#[derive(Debug)]
pub struct MockBackend {
    name: String,
    input_shapes: HashMap<String, TensorShape>,
    output_shapes: HashMap<String, TensorShape>,
}

impl MockBackend {
    /// Create a new mock backend
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            input_shapes: HashMap::new(),
            output_shapes: HashMap::new(),
        }
    }

    /// Add an input definition
    pub fn with_input(mut self, name: impl Into<String>, shape: TensorShape) -> Self {
        self.input_shapes.insert(name.into(), shape);
        self
    }

    /// Add an output definition
    pub fn with_output(mut self, name: impl Into<String>, shape: TensorShape) -> Self {
        self.output_shapes.insert(name.into(), shape);
        self
    }
}

impl Backend for MockBackend {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_available(&self) -> bool {
        true
    }

    fn input_names(&self) -> Vec<String> {
        self.input_shapes.keys().cloned().collect()
    }

    fn output_names(&self) -> Vec<String> {
        self.output_shapes.keys().cloned().collect()
    }

    fn input_shape(&self, name: &str) -> Option<TensorShape> {
        self.input_shapes.get(name).cloned()
    }

    fn output_shape(&self, name: &str) -> Option<TensorShape> {
        self.output_shapes.get(name).cloned()
    }

    fn run(&self, inputs: HashMap<String, Tensor>) -> NnResult<HashMap<String, Tensor>> {
        let mut outputs = HashMap::new();

        for (name, shape) in &self.output_shapes {
            let dims: Vec<usize> = shape.dims().to_vec();
            if dims.len() == 4 {
                outputs.insert(
                    name.clone(),
                    Tensor::zeros_4d([dims[0], dims[1], dims[2], dims[3]]),
                );
            }
        }

        Ok(outputs)
    }
}

/// Unified inference engine that supports multiple backends
pub struct InferenceEngine<B: Backend> {
    backend: B,
    options: InferenceOptions,
    /// Inference statistics
    stats: Arc<RwLock<InferenceStats>>,
}

/// Statistics for inference performance
#[derive(Debug, Default, Clone)]
pub struct InferenceStats {
    /// Total number of inferences
    pub total_inferences: u64,
    /// Total inference time in milliseconds
    pub total_time_ms: f64,
    /// Average inference time
    pub avg_time_ms: f64,
    /// Min inference time
    pub min_time_ms: f64,
    /// Max inference time
    pub max_time_ms: f64,
    /// Last inference time
    pub last_time_ms: f64,
}

impl InferenceStats {
    /// Record a new inference timing
    pub fn record(&mut self, time_ms: f64) {
        self.total_inferences += 1;
        self.total_time_ms += time_ms;
        self.last_time_ms = time_ms;
        self.avg_time_ms = self.total_time_ms / self.total_inferences as f64;

        if self.total_inferences == 1 {
            self.min_time_ms = time_ms;
            self.max_time_ms = time_ms;
        } else {
            self.min_time_ms = self.min_time_ms.min(time_ms);
            self.max_time_ms = self.max_time_ms.max(time_ms);
        }
    }
}

impl<B: Backend> InferenceEngine<B> {
    /// Create a new inference engine with a backend
    pub fn new(backend: B, options: InferenceOptions) -> Self {
        Self {
            backend,
            options,
            stats: Arc::new(RwLock::new(InferenceStats::default())),
        }
    }

    /// Get the backend
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// Get the options
    pub fn options(&self) -> &InferenceOptions {
        &self.options
    }

    /// Check if GPU is being used
    pub fn uses_gpu(&self) -> bool {
        self.options.use_gpu && self.backend.is_available()
    }

    /// Warm up the engine
    pub fn warmup(&self) -> NnResult<()> {
        info!("Warming up inference engine: {}", self.backend.name());
        self.backend.warmup()
    }

    /// Run inference on a single input
    #[instrument(skip(self, input))]
    pub fn infer(&self, input: &Tensor) -> NnResult<Tensor> {
        let start = std::time::Instant::now();

        let result = self.backend.run_single(input)?;

        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        debug!(elapsed_ms = %elapsed_ms, "Inference completed");

        // Update stats asynchronously (best effort)
        let stats = self.stats.clone();
        tokio::spawn(async move {
            let mut stats = stats.write().await;
            stats.record(elapsed_ms);
        });

        Ok(result)
    }

    /// Run inference with named inputs
    #[instrument(skip(self, inputs))]
    pub fn infer_named(&self, inputs: HashMap<String, Tensor>) -> NnResult<HashMap<String, Tensor>> {
        let start = std::time::Instant::now();

        let result = self.backend.run(inputs)?;

        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        debug!(elapsed_ms = %elapsed_ms, "Named inference completed");

        Ok(result)
    }

    /// Run batched inference
    pub fn infer_batch(&self, inputs: &[Tensor]) -> NnResult<Vec<Tensor>> {
        inputs.iter().map(|input| self.infer(input)).collect()
    }

    /// Get inference statistics
    pub async fn stats(&self) -> InferenceStats {
        self.stats.read().await.clone()
    }

    /// Reset statistics
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = InferenceStats::default();
    }

    /// Get memory usage
    pub fn memory_usage(&self) -> usize {
        self.backend.memory_usage()
    }
}

/// Combined pipeline for WiFi-DensePose inference
pub struct WiFiDensePosePipeline<B: Backend> {
    /// Modality translator backend
    translator_backend: B,
    /// DensePose backend
    densepose_backend: B,
    /// Translator configuration
    translator_config: TranslatorConfig,
    /// DensePose configuration
    densepose_config: DensePoseConfig,
    /// Inference options
    options: InferenceOptions,
}

impl<B: Backend> WiFiDensePosePipeline<B> {
    /// Create a new pipeline
    pub fn new(
        translator_backend: B,
        densepose_backend: B,
        translator_config: TranslatorConfig,
        densepose_config: DensePoseConfig,
        options: InferenceOptions,
    ) -> Self {
        Self {
            translator_backend,
            densepose_backend,
            translator_config,
            densepose_config,
            options,
        }
    }

    /// Run the full pipeline: CSI -> Visual Features -> DensePose
    #[instrument(skip(self, csi_input))]
    pub fn run(&self, csi_input: &Tensor) -> NnResult<DensePoseOutput> {
        // Step 1: Translate CSI to visual features
        let visual_features = self.translator_backend.run_single(csi_input)?;

        // Step 2: Run DensePose on visual features
        let mut inputs = HashMap::new();
        inputs.insert("features".to_string(), visual_features);

        let outputs = self.densepose_backend.run(inputs)?;

        // Extract outputs
        let segmentation = outputs
            .get("segmentation")
            .cloned()
            .ok_or_else(|| NnError::inference("Missing segmentation output"))?;

        let uv_coordinates = outputs
            .get("uv_coordinates")
            .cloned()
            .ok_or_else(|| NnError::inference("Missing uv_coordinates output"))?;

        Ok(DensePoseOutput {
            segmentation,
            uv_coordinates,
            confidence: None,
        })
    }

    /// Get translator config
    pub fn translator_config(&self) -> &TranslatorConfig {
        &self.translator_config
    }

    /// Get DensePose config
    pub fn densepose_config(&self) -> &DensePoseConfig {
        &self.densepose_config
    }
}

/// Builder for creating inference engines
pub struct EngineBuilder {
    options: InferenceOptions,
    model_path: Option<String>,
}

impl EngineBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            options: InferenceOptions::default(),
            model_path: None,
        }
    }

    /// Set inference options
    pub fn options(mut self, options: InferenceOptions) -> Self {
        self.options = options;
        self
    }

    /// Set model path
    pub fn model_path(mut self, path: impl Into<String>) -> Self {
        self.model_path = Some(path.into());
        self
    }

    /// Use GPU
    pub fn gpu(mut self, device_id: usize) -> Self {
        self.options.use_gpu = true;
        self.options.gpu_device_id = device_id;
        self
    }

    /// Use CPU
    pub fn cpu(mut self) -> Self {
        self.options.use_gpu = false;
        self
    }

    /// Set batch size
    pub fn batch_size(mut self, size: usize) -> Self {
        self.options.batch_size = size;
        self
    }

    /// Set number of threads
    pub fn threads(mut self, n: usize) -> Self {
        self.options.num_threads = n;
        self
    }

    /// Build with a mock backend (for testing)
    pub fn build_mock(self) -> InferenceEngine<MockBackend> {
        let backend = MockBackend::new("mock")
            .with_input("input".to_string(), TensorShape::new(vec![1, 256, 64, 64]))
            .with_output("output".to_string(), TensorShape::new(vec![1, 256, 64, 64]));

        InferenceEngine::new(backend, self.options)
    }

    /// Build with ONNX backend
    #[cfg(feature = "onnx")]
    pub fn build_onnx(self) -> NnResult<InferenceEngine<crate::onnx::OnnxBackend>> {
        let model_path = self
            .model_path
            .ok_or_else(|| NnError::config("Model path required for ONNX backend"))?;

        let backend = crate::onnx::OnnxBackend::from_file(&model_path)?;
        Ok(InferenceEngine::new(backend, self.options))
    }
}

impl Default for EngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inference_options() {
        let opts = InferenceOptions::cpu().with_batch_size(4).with_threads(8);
        assert_eq!(opts.batch_size, 4);
        assert_eq!(opts.num_threads, 8);
        assert!(!opts.use_gpu);

        let gpu_opts = InferenceOptions::gpu(0);
        assert!(gpu_opts.use_gpu);
        assert_eq!(gpu_opts.gpu_device_id, 0);
    }

    #[test]
    fn test_mock_backend() {
        let backend = MockBackend::new("test")
            .with_input("input", TensorShape::new(vec![1, 3, 224, 224]))
            .with_output("output", TensorShape::new(vec![1, 1000]));

        assert_eq!(backend.name(), "test");
        assert!(backend.is_available());
        assert_eq!(backend.input_names(), vec!["input".to_string()]);
        assert_eq!(backend.output_names(), vec!["output".to_string()]);
    }

    #[test]
    fn test_engine_builder() {
        let engine = EngineBuilder::new()
            .cpu()
            .batch_size(2)
            .threads(4)
            .build_mock();

        assert_eq!(engine.options().batch_size, 2);
        assert_eq!(engine.options().num_threads, 4);
    }

    #[test]
    fn test_inference_stats() {
        let mut stats = InferenceStats::default();
        stats.record(10.0);
        stats.record(20.0);
        stats.record(15.0);

        assert_eq!(stats.total_inferences, 3);
        assert_eq!(stats.min_time_ms, 10.0);
        assert_eq!(stats.max_time_ms, 20.0);
        assert_eq!(stats.avg_time_ms, 15.0);
    }

    #[tokio::test]
    async fn test_inference_engine() {
        let engine = EngineBuilder::new().build_mock();

        let input = Tensor::zeros_4d([1, 256, 64, 64]);
        let output = engine.infer(&input).unwrap();

        assert_eq!(output.shape().dims(), &[1, 256, 64, 64]);
    }
}
