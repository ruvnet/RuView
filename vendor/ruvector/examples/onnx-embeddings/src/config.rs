//! Configuration for the ONNX embedder

use crate::PretrainedModel;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Source of the ONNX model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelSource {
    /// Load from HuggingFace Hub (downloads if not cached)
    HuggingFace {
        model_id: String,
        revision: Option<String>,
    },
    /// Load from a local ONNX file
    Local {
        model_path: PathBuf,
        tokenizer_path: PathBuf,
    },
    /// Use a pre-configured model
    Pretrained(PretrainedModel),
    /// Custom URL for model download
    Url {
        model_url: String,
        tokenizer_url: String,
    },
}

impl Default for ModelSource {
    fn default() -> Self {
        Self::Pretrained(PretrainedModel::default())
    }
}

impl From<PretrainedModel> for ModelSource {
    fn from(model: PretrainedModel) -> Self {
        Self::Pretrained(model)
    }
}

/// Pooling strategy for combining token embeddings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PoolingStrategy {
    /// Mean pooling over all tokens (most common)
    #[default]
    Mean,
    /// Use [CLS] token embedding
    Cls,
    /// Max pooling over all tokens
    Max,
    /// Mean pooling with sqrt(length) scaling
    MeanSqrtLen,
    /// Last token pooling (for decoder models)
    LastToken,
    /// Weighted mean based on attention mask
    WeightedMean,
}

/// Execution provider for ONNX Runtime
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum ExecutionProvider {
    /// CPU inference (default, always available)
    #[default]
    Cpu,
    /// CUDA GPU acceleration
    Cuda { device_id: i32 },
    /// TensorRT optimization
    TensorRt { device_id: i32 },
    /// CoreML on macOS
    CoreMl,
    /// DirectML on Windows
    DirectMl,
    /// ROCm for AMD GPUs
    Rocm { device_id: i32 },
}

/// Configuration for the embedder
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedderConfig {
    /// Model source
    pub model_source: ModelSource,
    /// Pooling strategy
    pub pooling: PoolingStrategy,
    /// Whether to normalize embeddings to unit length
    pub normalize: bool,
    /// Maximum sequence length (truncation)
    pub max_length: usize,
    /// Batch size for inference
    pub batch_size: usize,
    /// Number of threads for CPU inference
    pub num_threads: usize,
    /// Execution provider
    pub execution_provider: ExecutionProvider,
    /// Cache directory for downloaded models
    pub cache_dir: PathBuf,
    /// Whether to show progress during downloads
    pub show_progress: bool,
    /// Use fp16 inference if available
    pub use_fp16: bool,
    /// Enable graph optimization
    pub optimize_graph: bool,
}

impl Default for EmbedderConfig {
    fn default() -> Self {
        Self {
            model_source: ModelSource::default(),
            pooling: PoolingStrategy::default(),
            normalize: true,
            max_length: 256,
            batch_size: 32,
            num_threads: num_cpus::get(),
            execution_provider: ExecutionProvider::default(),
            cache_dir: default_cache_dir(),
            show_progress: true,
            use_fp16: false,
            optimize_graph: true,
        }
    }
}

impl EmbedderConfig {
    /// Create a new config builder
    pub fn builder() -> EmbedderConfigBuilder {
        EmbedderConfigBuilder::default()
    }

    /// Create config for a pretrained model
    pub fn pretrained(model: PretrainedModel) -> Self {
        Self {
            model_source: ModelSource::Pretrained(model),
            max_length: model.max_seq_length(),
            normalize: model.normalize_output(),
            ..Default::default()
        }
    }

    /// Create config for a local model
    pub fn local(model_path: impl Into<PathBuf>, tokenizer_path: impl Into<PathBuf>) -> Self {
        Self {
            model_source: ModelSource::Local {
                model_path: model_path.into(),
                tokenizer_path: tokenizer_path.into(),
            },
            ..Default::default()
        }
    }

    /// Create config for a HuggingFace model
    pub fn huggingface(model_id: impl Into<String>) -> Self {
        Self {
            model_source: ModelSource::HuggingFace {
                model_id: model_id.into(),
                revision: None,
            },
            ..Default::default()
        }
    }
}

/// Builder for EmbedderConfig
#[derive(Debug, Default)]
pub struct EmbedderConfigBuilder {
    config: EmbedderConfig,
}

impl EmbedderConfigBuilder {
    pub fn model_source(mut self, source: ModelSource) -> Self {
        self.config.model_source = source;
        self
    }

    pub fn pretrained(mut self, model: PretrainedModel) -> Self {
        self.config.model_source = ModelSource::Pretrained(model);
        self.config.max_length = model.max_seq_length();
        self.config.normalize = model.normalize_output();
        self
    }

    pub fn pooling(mut self, strategy: PoolingStrategy) -> Self {
        self.config.pooling = strategy;
        self
    }

    pub fn normalize(mut self, normalize: bool) -> Self {
        self.config.normalize = normalize;
        self
    }

    pub fn max_length(mut self, length: usize) -> Self {
        self.config.max_length = length;
        self
    }

    pub fn batch_size(mut self, size: usize) -> Self {
        self.config.batch_size = size;
        self
    }

    pub fn num_threads(mut self, threads: usize) -> Self {
        self.config.num_threads = threads;
        self
    }

    pub fn execution_provider(mut self, provider: ExecutionProvider) -> Self {
        self.config.execution_provider = provider;
        self
    }

    pub fn cache_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.config.cache_dir = dir.into();
        self
    }

    pub fn show_progress(mut self, show: bool) -> Self {
        self.config.show_progress = show;
        self
    }

    pub fn use_fp16(mut self, use_fp16: bool) -> Self {
        self.config.use_fp16 = use_fp16;
        self
    }

    pub fn optimize_graph(mut self, optimize: bool) -> Self {
        self.config.optimize_graph = optimize;
        self
    }

    pub fn build(self) -> EmbedderConfig {
        self.config
    }
}

fn default_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ruvector")
        .join("onnx-models")
}

fn num_cpus_get() -> usize {
    std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(4)
}

mod num_cpus {
    pub fn get() -> usize {
        super::num_cpus_get()
    }
}
