//! LLM inference backends for RuvLLM
//!
//! This module provides pluggable backend implementations for LLM inference.
//! Currently supported backends:
//!
//! - **Candle** (Rust-native HuggingFace): Full Rust implementation with Metal acceleration
//! - **mistral-rs**: High-performance inference with PagedAttention and X-LoRA
//!
//! ## Architecture Support
//!
//! Both backends support the following model architectures:
//! - Mistral (7B, Codestral)
//! - Llama (1B-70B, Llama 2, Llama 3)
//! - Phi (1.5, 2, 3)
//!
//! ## Quantization
//!
//! Supports GGUF quantization formats:
//! - Q4_0, Q4_1, Q4_K (4-bit quantization)
//! - Q8_0, Q8_1 (8-bit quantization)
//! - F16, F32 (full precision)
//!
//! The mistral-rs backend also supports ISQ (In-Situ Quantization) for runtime
//! quantization with AWQ, GPTQ, and SmoothQuant methods.
//!
//! ## Candle Backend Example
//!
//! ```rust,ignore
//! use ruvllm::backends::{CandleBackend, ModelConfig, GenerateParams};
//!
//! let mut backend = CandleBackend::new()?;
//!
//! let config = ModelConfig {
//!     architecture: ModelArchitecture::Mistral,
//!     quantization: Some(Quantization::Q4K),
//!     use_flash_attention: true,
//!     ..Default::default()
//! };
//!
//! backend.load_model("mistralai/Mistral-7B-v0.1", config)?;
//!
//! let params = GenerateParams::default()
//!     .with_max_tokens(256)
//!     .with_temperature(0.7);
//!
//! let response = backend.generate("Hello, world!", params)?;
//! ```
//!
//! ## mistral-rs Backend Example
//!
//! ```rust,ignore
//! use ruvllm::backends::{MistralBackend, MistralBackendConfig, ModelConfig, GenerateParams};
//! use std::path::Path;
//!
//! // Create backend with PagedAttention and X-LoRA support
//! let config = MistralBackendConfig::default()
//!     .with_paged_attention(16, 4096)
//!     .with_xlora_adapters(vec!["code", "chat"]);
//!
//! let mut backend = MistralBackend::with_config(config)?;
//! backend.load_model("mistralai/Mistral-7B-v0.3", ModelConfig::default())?;
//!
//! // Load and activate X-LoRA adapters
//! backend.load_xlora_adapter("code", Path::new("./adapters/code"))?;
//! backend.set_xlora_adapters(vec![("code", 0.7), ("chat", 0.3)])?;
//!
//! let response = backend.generate("Hello, world!", GenerateParams::default())?;
//! ```

#[cfg(feature = "candle")]
mod candle_backend;

#[cfg(feature = "candle")]
pub use candle_backend::*;

// Core ML backend for Apple Neural Engine (ANE) acceleration
mod coreml_backend;
pub use coreml_backend::{AneCapabilities, ComputeUnits, CoreMLBackend};

// Hybrid GPU+ANE pipeline coordinator
#[cfg(feature = "hybrid-ane")]
mod hybrid_pipeline;
#[cfg(feature = "hybrid-ane")]
pub use hybrid_pipeline::{
    AcceleratorMetrics, AcceleratorType, AneStrategy, DataFormat, HybridPipeline,
    HybridPipelineConfig, HybridTensor, OperationType, RoutingDecision,
};

// Model architecture implementations
pub mod gemma2;
pub mod phi3;

pub use gemma2::{
    logit_soft_cap, Gemma2Attention, Gemma2Config, Gemma2DecoderLayer, Gemma2MLP, Gemma2Model,
    ATTENTION_SOFTCAP, FINAL_LOGIT_SOFTCAP,
};
pub use phi3::{Phi3Attention, Phi3Config, Phi3DecoderLayer, Phi3MLP, Phi3Model};

// mistral-rs backend - always available, but full functionality requires the feature
mod mistral_backend;

pub use mistral_backend::{
    IsqConfig, IsqMethod, MistralBackend, MistralBackendConfig, MistralTokenizer,
    PagedAttentionConfigExt, XLoraConfig, XLoraManager, XLoraManagerStats, XLoraMixingMode,
};

use crate::error::{Result, RuvLLMError};
use serde::{Deserialize, Serialize};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

/// Model architecture types supported by RuvLLM.
///
/// RuvLLM supports multiple transformer architectures with varying
/// characteristics optimized for different use cases.
///
/// # Supported Architectures
///
/// | Architecture | Parameter Sizes | Best For |
/// |--------------|-----------------|----------|
/// | `Llama` | 1B-70B | General purpose, chat |
/// | `Mistral` | 7B | Code, instruction following |
/// | `Phi` | 1.5-3B | Efficient edge deployment |
/// | `Phi3` | 3B-14B | Extended context, SuRoPE |
/// | `Qwen` | 0.5B-72B | Multilingual, reasoning |
/// | `Gemma` | 2B-7B | Efficient, instruction-tuned |
/// | `Gemma2` | 2B-27B | Soft-capping, alternating attention |
///
/// # Example
///
/// ```rust,ignore
/// use ruvllm::backends::ModelArchitecture;
///
/// let arch = ModelArchitecture::Mistral;
/// assert_eq!(arch.config_name(), "mistral");
///
/// let phi3 = ModelArchitecture::Phi3;
/// assert_eq!(phi3.config_name(), "phi3");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelArchitecture {
    /// Mistral architecture (7B, Codestral)
    Mistral,
    /// Llama architecture (1B-70B)
    Llama,
    /// Phi architecture (1.5, 2)
    Phi,
    /// Phi-3 architecture (SuRoPE, SwiGLU, sliding window)
    Phi3,
    /// Qwen architecture
    Qwen,
    /// Gemma architecture (original)
    Gemma,
    /// Gemma-2 architecture (soft-capping, alternating local/global attention)
    Gemma2,
}

impl Default for ModelArchitecture {
    fn default() -> Self {
        Self::Llama
    }
}

impl ModelArchitecture {
    /// Get architecture name for HuggingFace model config
    pub fn config_name(&self) -> &'static str {
        match self {
            Self::Mistral => "mistral",
            Self::Llama => "llama",
            Self::Phi => "phi",
            Self::Phi3 => "phi3",
            Self::Qwen => "qwen2",
            Self::Gemma => "gemma",
            Self::Gemma2 => "gemma2",
        }
    }

    /// Detect architecture from model ID string
    pub fn detect_from_model_id(model_id: &str) -> Option<Self> {
        let lower = model_id.to_lowercase();
        if lower.contains("phi-3") || lower.contains("phi3") {
            Some(Self::Phi3)
        } else if lower.contains("phi") {
            Some(Self::Phi)
        } else if lower.contains("gemma-2") || lower.contains("gemma2") {
            Some(Self::Gemma2)
        } else if lower.contains("gemma") {
            Some(Self::Gemma)
        } else if lower.contains("mistral") || lower.contains("codestral") {
            Some(Self::Mistral)
        } else if lower.contains("llama") {
            Some(Self::Llama)
        } else if lower.contains("qwen") {
            Some(Self::Qwen)
        } else {
            None
        }
    }

    /// Check if this architecture uses GQA (Grouped Query Attention)
    pub fn uses_gqa(&self) -> bool {
        matches!(
            self,
            Self::Mistral | Self::Llama | Self::Gemma | Self::Gemma2 | Self::Qwen
        )
    }

    /// Check if this architecture uses sliding window attention
    pub fn uses_sliding_window(&self) -> bool {
        matches!(self, Self::Mistral | Self::Phi3 | Self::Gemma2)
    }

    /// Get default sliding window size for this architecture
    pub fn default_sliding_window(&self) -> Option<usize> {
        match self {
            Self::Mistral => Some(4096),
            Self::Phi3 => Some(2048),
            Self::Gemma2 => Some(4096), // For local attention layers
            _ => None,
        }
    }
}

/// Quantization formats for model weights.
///
/// Quantization reduces model memory footprint and can improve inference
/// speed at the cost of some quality. RuvLLM supports multiple formats
/// with different tradeoffs.
///
/// # Memory vs Quality Tradeoff
///
/// | Format | Bytes/Weight | Memory (7B) | Quality |
/// |--------|--------------|-------------|---------|
/// | `None` (F32) | 4.0 | 28 GB | Best |
/// | `F16` | 2.0 | 14 GB | Excellent |
/// | `Q8` | 1.0 | 7 GB | Very Good |
/// | `Q4K` | 0.5 | 3.5 GB | Good |
/// | `Q4` | 0.5 | 3.5 GB | Acceptable |
/// | `Q2K` | 0.25 | 1.75 GB | Experimental |
///
/// # Example
///
/// ```rust,ignore
/// use ruvllm::backends::Quantization;
///
/// let quant = Quantization::Q4K;
/// assert_eq!(quant.bytes_per_weight(), 0.5);
/// assert!(quant.is_gguf());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Quantization {
    /// No quantization (FP32)
    None,
    /// Half precision (FP16)
    F16,
    /// Brain float (BF16)
    Bf16,
    /// 8-bit quantization
    Q8,
    /// 4-bit K-quants (higher quality)
    Q4K,
    /// 4-bit quantization (standard)
    Q4,
    /// 2-bit quantization (experimental)
    Q2K,
}

impl Default for Quantization {
    fn default() -> Self {
        Self::Q4K
    }
}

impl Quantization {
    /// Get bytes per weight element
    pub fn bytes_per_weight(&self) -> f32 {
        match self {
            Self::None => 4.0,
            Self::F16 | Self::Bf16 => 2.0,
            Self::Q8 => 1.0,
            Self::Q4K | Self::Q4 => 0.5,
            Self::Q2K => 0.25,
        }
    }

    /// Check if this is a GGUF quantization format
    pub fn is_gguf(&self) -> bool {
        matches!(self, Self::Q8 | Self::Q4K | Self::Q4 | Self::Q2K)
    }
}

/// Configuration for loading and running a model.
///
/// This struct controls all aspects of model loading including architecture,
/// quantization, attention mechanisms, and device placement.
///
/// # Example
///
/// ```rust,ignore
/// use ruvllm::backends::{ModelConfig, ModelArchitecture, Quantization, DeviceType};
///
/// let config = ModelConfig {
///     architecture: ModelArchitecture::Mistral,
///     quantization: Some(Quantization::Q4K),
///     use_flash_attention: true,
///     max_sequence_length: 8192,
///     device: DeviceType::Metal,
///     ..Default::default()
/// };
/// ```
///
/// # Architecture Detection
///
/// When loading from HuggingFace Hub, the architecture is automatically
/// detected from the model's `config.json`. The `architecture` field
/// is only used as a hint when auto-detection fails.
#[derive(Debug, Clone)]
pub struct ModelConfig {
    /// Model architecture
    pub architecture: ModelArchitecture,
    /// Quantization format
    pub quantization: Option<Quantization>,
    /// Use Flash Attention for memory efficiency
    pub use_flash_attention: bool,
    /// Maximum sequence length
    pub max_sequence_length: usize,
    /// Number of key-value heads (for GQA)
    pub num_kv_heads: Option<usize>,
    /// Hidden dimension size
    pub hidden_size: Option<usize>,
    /// Number of layers
    pub num_layers: Option<usize>,
    /// Vocabulary size
    pub vocab_size: Option<usize>,
    /// Rope theta (for rotary embeddings)
    pub rope_theta: Option<f64>,
    /// Use sliding window attention
    pub sliding_window: Option<usize>,
    /// Device to load model on (metal, cpu)
    pub device: DeviceType,
    /// Data type for inference
    pub dtype: DType,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            architecture: ModelArchitecture::default(),
            quantization: Some(Quantization::Q4K),
            use_flash_attention: true,
            max_sequence_length: 4096,
            num_kv_heads: None,
            hidden_size: None,
            num_layers: None,
            vocab_size: None,
            rope_theta: None,
            sliding_window: None,
            device: DeviceType::default(),
            dtype: DType::default(),
        }
    }
}

/// Device type for inference
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum DeviceType {
    /// CPU inference
    Cpu,
    /// Metal (Apple Silicon) - default on macOS
    #[default]
    Metal,
    /// CUDA (NVIDIA GPUs)
    Cuda(usize),
}

/// Data type for tensor operations
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum DType {
    /// 32-bit floating point
    F32,
    /// 16-bit floating point (default)
    #[default]
    F16,
    /// Brain float 16
    Bf16,
}

/// Parameters for text generation.
///
/// Controls the sampling strategy and output constraints for text generation.
/// Supports temperature scaling, nucleus sampling, top-k filtering, and
/// repetition penalties.
///
/// # Example
///
/// ```rust,ignore
/// use ruvllm::backends::GenerateParams;
///
/// // Creative writing (high temperature, diverse sampling)
/// let creative = GenerateParams::default()
///     .with_max_tokens(512)
///     .with_temperature(0.9)
///     .with_top_p(0.95)
///     .with_top_k(50);
///
/// // Code completion (low temperature, focused sampling)
/// let code = GenerateParams::default()
///     .with_max_tokens(256)
///     .with_temperature(0.2)
///     .with_top_p(0.9)
///     .with_repetition_penalty(1.2);
///
/// // Deterministic (greedy decoding)
/// let deterministic = GenerateParams::default()
///     .with_temperature(0.0)
///     .with_seed(42);
/// ```
///
/// # Sampling Parameters
///
/// | Parameter | Range | Effect |
/// |-----------|-------|--------|
/// | `temperature` | 0.0-2.0 | Higher = more random |
/// | `top_p` | 0.0-1.0 | Nucleus sampling threshold |
/// | `top_k` | 0-vocab_size | Limit to top K tokens |
/// | `repetition_penalty` | 1.0-2.0 | Penalize repeated tokens |
#[derive(Debug, Clone)]
pub struct GenerateParams {
    /// Maximum number of tokens to generate
    pub max_tokens: usize,
    /// Temperature for sampling (0.0 = deterministic)
    pub temperature: f32,
    /// Top-p (nucleus) sampling
    pub top_p: f32,
    /// Top-k sampling (0 = disabled)
    pub top_k: usize,
    /// Repetition penalty
    pub repetition_penalty: f32,
    /// Frequency penalty
    pub frequency_penalty: f32,
    /// Presence penalty
    pub presence_penalty: f32,
    /// Stop sequences
    pub stop_sequences: Vec<String>,
    /// Seed for reproducibility
    pub seed: Option<u64>,
}

impl Default for GenerateParams {
    fn default() -> Self {
        Self {
            max_tokens: 256,
            temperature: 0.7,
            top_p: 0.9,
            top_k: 40,
            repetition_penalty: 1.1,
            frequency_penalty: 0.0,
            presence_penalty: 0.0,
            stop_sequences: Vec::new(),
            seed: None,
        }
    }
}

impl GenerateParams {
    /// Set maximum tokens
    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// Set temperature
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature;
        self
    }

    /// Set top-p sampling
    pub fn with_top_p(mut self, top_p: f32) -> Self {
        self.top_p = top_p;
        self
    }

    /// Set top-k sampling
    pub fn with_top_k(mut self, top_k: usize) -> Self {
        self.top_k = top_k;
        self
    }

    /// Set repetition penalty
    pub fn with_repetition_penalty(mut self, penalty: f32) -> Self {
        self.repetition_penalty = penalty;
        self
    }

    /// Add stop sequence
    pub fn with_stop_sequence(mut self, stop: impl Into<String>) -> Self {
        self.stop_sequences.push(stop.into());
        self
    }

    /// Set seed for reproducibility
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }
}

/// Token generated during streaming
#[derive(Debug, Clone)]
pub struct GeneratedToken {
    /// Token ID
    pub id: u32,
    /// Token text
    pub text: String,
    /// Log probability
    pub logprob: Option<f32>,
    /// Is this a special token
    pub is_special: bool,
}

/// Stream events emitted during token generation
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// A new token has been generated
    Token(GeneratedToken),
    /// Generation is complete
    Done {
        /// Total number of tokens generated
        total_tokens: usize,
        /// Total generation duration in milliseconds
        duration_ms: u64,
        /// Tokens per second
        tokens_per_second: f64,
    },
    /// An error occurred during generation
    Error(String),
}

/// Streaming token iterator.
///
/// Provides an iterator interface over generated tokens, allowing
/// real-time processing of model output as it's generated. Includes
/// built-in metrics tracking for throughput monitoring.
///
/// # Example
///
/// ```rust,ignore
/// use ruvllm::backends::{TokenStream, StreamEvent};
///
/// let stream = backend.generate_stream_v2("Hello", params)?;
///
/// // Iterate with metrics
/// for event in stream {
///     match event? {
///         StreamEvent::Token(token) => {
///             print!("{}", token.text);
///         }
///         StreamEvent::Done { total_tokens, tokens_per_second, .. } => {
///             println!("\n\nGenerated {} tokens at {:.1} tok/s",
///                 total_tokens, tokens_per_second);
///         }
///         StreamEvent::Error(e) => {
///             eprintln!("Generation error: {}", e);
///             break;
///         }
///     }
/// }
///
/// // Check metrics during generation
/// println!("Current rate: {:.1} tok/s", stream.tokens_per_second());
/// ```
///
/// # Non-blocking Usage
///
/// ```rust,ignore
/// // Poll without blocking
/// while let Some(event) = stream.try_next() {
///     handle_event(event?);
/// }
///
/// // Poll with timeout
/// while let Some(event) = stream.recv_timeout(Duration::from_millis(100)) {
///     handle_event(event?);
/// }
/// ```
pub struct TokenStream {
    /// Channel receiver for stream events
    receiver: mpsc::Receiver<StreamEvent>,
    /// Whether the stream has completed
    finished: bool,
    /// Generation start time for metrics
    start_time: Instant,
    /// Number of tokens received so far
    token_count: usize,
}

impl TokenStream {
    /// Create a new token stream from a channel receiver
    pub fn new(receiver: mpsc::Receiver<StreamEvent>) -> Self {
        Self {
            receiver,
            finished: false,
            start_time: Instant::now(),
            token_count: 0,
        }
    }

    /// Create a channel pair for streaming
    pub fn channel() -> (mpsc::Sender<StreamEvent>, Self) {
        let (tx, rx) = mpsc::channel();
        (tx, Self::new(rx))
    }

    /// Check if the stream has finished
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Get the number of tokens received so far
    pub fn tokens_received(&self) -> usize {
        self.token_count
    }

    /// Get elapsed time since stream started
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Calculate current tokens per second
    pub fn tokens_per_second(&self) -> f64 {
        let elapsed = self.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.token_count as f64 / elapsed
        } else {
            0.0
        }
    }

    /// Try to receive the next event without blocking
    pub fn try_next(&mut self) -> Option<Result<StreamEvent>> {
        if self.finished {
            return None;
        }

        match self.receiver.try_recv() {
            Ok(event) => {
                match &event {
                    StreamEvent::Token(_) => self.token_count += 1,
                    StreamEvent::Done { .. } => self.finished = true,
                    StreamEvent::Error(_) => self.finished = true,
                }
                Some(Ok(event))
            }
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.finished = true;
                None
            }
        }
    }

    /// Receive the next event with a timeout
    pub fn recv_timeout(&mut self, timeout: Duration) -> Option<Result<StreamEvent>> {
        if self.finished {
            return None;
        }

        match self.receiver.recv_timeout(timeout) {
            Ok(event) => {
                match &event {
                    StreamEvent::Token(_) => self.token_count += 1,
                    StreamEvent::Done { .. } => self.finished = true,
                    StreamEvent::Error(_) => self.finished = true,
                }
                Some(Ok(event))
            }
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.finished = true;
                None
            }
        }
    }
}

impl Iterator for TokenStream {
    type Item = Result<StreamEvent>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        match self.receiver.recv() {
            Ok(event) => {
                match &event {
                    StreamEvent::Token(_) => self.token_count += 1,
                    StreamEvent::Done { .. } => self.finished = true,
                    StreamEvent::Error(_) => self.finished = true,
                }
                Some(Ok(event))
            }
            Err(_) => {
                self.finished = true;
                None
            }
        }
    }
}

/// Backend trait for LLM inference.
///
/// This trait defines the interface that all inference backends must implement.
/// It provides methods for model loading, text generation, and embedding extraction.
///
/// # Implementations
///
/// - [`CandleBackend`]: Rust-native backend using HuggingFace Candle
/// - [`MistralBackend`]: High-performance backend with PagedAttention and X-LoRA
/// - [`NoopBackend`]: Placeholder when no backend is enabled
///
/// # Example
///
/// ```rust,ignore
/// use ruvllm::backends::{LlmBackend, ModelConfig, GenerateParams, create_backend};
///
/// // Create backend (auto-selects based on features)
/// let mut backend = create_backend();
///
/// // Load a model
/// let config = ModelConfig::default();
/// backend.load_model("mistralai/Mistral-7B-v0.1", config)?;
///
/// // Generate text
/// let params = GenerateParams::default().with_max_tokens(100);
/// let response = backend.generate("Hello, ", params)?;
/// println!("{}", response);
///
/// // Stream tokens
/// let stream = backend.generate_stream_v2("Hello, ", params)?;
/// for event in stream {
///     match event? {
///         StreamEvent::Token(t) => print!("{}", t.text),
///         StreamEvent::Done { tokens_per_second, .. } => {
///             println!("\n[{:.1} tok/s]", tokens_per_second);
///         }
///         StreamEvent::Error(e) => eprintln!("Error: {}", e),
///     }
/// }
/// ```
pub trait LlmBackend: Send + Sync {
    /// Load a model from path or HuggingFace Hub
    ///
    /// # Arguments
    ///
    /// * `model_id` - Path to local model or HuggingFace model ID
    /// * `config` - Model configuration
    ///
    /// # Errors
    ///
    /// Returns an error if the model cannot be loaded (not found, invalid format, etc.)
    fn load_model(&mut self, model_id: &str, config: ModelConfig) -> Result<()>;

    /// Generate text from a prompt
    ///
    /// # Arguments
    ///
    /// * `prompt` - Input text prompt
    /// * `params` - Generation parameters
    ///
    /// # Returns
    ///
    /// Generated text (excluding the input prompt)
    fn generate(&self, prompt: &str, params: GenerateParams) -> Result<String>;

    /// Generate text with streaming output (legacy interface)
    ///
    /// # Arguments
    ///
    /// * `prompt` - Input text prompt
    /// * `params` - Generation parameters
    ///
    /// # Returns
    ///
    /// Iterator over generated tokens
    fn generate_stream(
        &self,
        prompt: &str,
        params: GenerateParams,
    ) -> Result<Box<dyn Iterator<Item = Result<GeneratedToken>> + Send + '_>>;

    /// Generate text with streaming output using TokenStream
    ///
    /// This is the preferred streaming interface that provides real-time
    /// token generation with progress tracking and metrics.
    ///
    /// # Arguments
    ///
    /// * `prompt` - Input text prompt
    /// * `params` - Generation parameters
    ///
    /// # Returns
    ///
    /// A TokenStream that yields StreamEvents as tokens are generated
    fn generate_stream_v2(&self, prompt: &str, params: GenerateParams) -> Result<TokenStream>;

    /// Extract embeddings from text
    ///
    /// Uses the model's embedding layer to generate dense vector representations.
    ///
    /// # Arguments
    ///
    /// * `text` - Input text
    ///
    /// # Returns
    ///
    /// Vector of embeddings (hidden_size dimension)
    fn get_embeddings(&self, text: &str) -> Result<Vec<f32>>;

    /// Get the tokenizer for this backend
    fn tokenizer(&self) -> Option<&dyn Tokenizer>;

    /// Check if a model is loaded
    fn is_model_loaded(&self) -> bool;

    /// Get model information
    fn model_info(&self) -> Option<ModelInfo>;

    /// Unload the current model and free memory
    fn unload_model(&mut self);
}

/// Tokenizer trait for text encoding/decoding
pub trait Tokenizer: Send + Sync {
    /// Encode text to token IDs
    fn encode(&self, text: &str) -> Result<Vec<u32>>;

    /// Decode token IDs to text
    fn decode(&self, tokens: &[u32]) -> Result<String>;

    /// Get vocabulary size
    fn vocab_size(&self) -> usize;

    /// Get special tokens
    fn special_tokens(&self) -> SpecialTokens;
}

/// Special token IDs
#[derive(Debug, Clone, Default)]
pub struct SpecialTokens {
    /// Beginning of sequence token
    pub bos_token_id: Option<u32>,
    /// End of sequence token
    pub eos_token_id: Option<u32>,
    /// Padding token
    pub pad_token_id: Option<u32>,
    /// Unknown token
    pub unk_token_id: Option<u32>,
}

/// Information about a loaded model
#[derive(Debug, Clone)]
pub struct ModelInfo {
    /// Model name/ID
    pub name: String,
    /// Model architecture
    pub architecture: ModelArchitecture,
    /// Number of parameters (approximate)
    pub num_parameters: usize,
    /// Vocabulary size
    pub vocab_size: usize,
    /// Hidden dimension
    pub hidden_size: usize,
    /// Number of layers
    pub num_layers: usize,
    /// Maximum context length
    pub max_context_length: usize,
    /// Quantization applied
    pub quantization: Option<Quantization>,
    /// Memory usage in bytes
    pub memory_usage: usize,
}

/// A placeholder backend for when no real backend is available
pub struct NoopBackend;

impl LlmBackend for NoopBackend {
    fn load_model(&mut self, _model_id: &str, _config: ModelConfig) -> Result<()> {
        Err(RuvLLMError::Config(
            "No inference backend enabled. Enable 'candle' feature.".to_string(),
        ))
    }

    fn generate(&self, _prompt: &str, _params: GenerateParams) -> Result<String> {
        Err(RuvLLMError::Config(
            "No inference backend enabled.".to_string(),
        ))
    }

    fn generate_stream(
        &self,
        _prompt: &str,
        _params: GenerateParams,
    ) -> Result<Box<dyn Iterator<Item = Result<GeneratedToken>> + Send + '_>> {
        Err(RuvLLMError::Config(
            "No inference backend enabled.".to_string(),
        ))
    }

    fn generate_stream_v2(&self, _prompt: &str, _params: GenerateParams) -> Result<TokenStream> {
        Err(RuvLLMError::Config(
            "No inference backend enabled.".to_string(),
        ))
    }

    fn get_embeddings(&self, _text: &str) -> Result<Vec<f32>> {
        Err(RuvLLMError::Config(
            "No inference backend enabled.".to_string(),
        ))
    }

    fn tokenizer(&self) -> Option<&dyn Tokenizer> {
        None
    }

    fn is_model_loaded(&self) -> bool {
        false
    }

    fn model_info(&self) -> Option<ModelInfo> {
        None
    }

    fn unload_model(&mut self) {}
}

/// Create a backend instance based on available features
pub fn create_backend() -> Box<dyn LlmBackend> {
    #[cfg(feature = "candle")]
    {
        Box::new(CandleBackend::new().unwrap_or_else(|_| CandleBackend::default()))
    }

    #[cfg(not(feature = "candle"))]
    {
        Box::new(NoopBackend)
    }
}

/// Thread-safe backend wrapper
pub type SharedBackend = Arc<dyn LlmBackend>;

// ============================================================================
// Async streaming support
// ============================================================================

/// Async token stream for tokio compatibility
///
/// This provides an async-compatible wrapper around the synchronous TokenStream,
/// allowing it to be used with async/await and tokio runtime.
#[cfg(feature = "async-runtime")]
pub mod async_stream {
    use super::*;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    /// Async wrapper around TokenStream
    pub struct AsyncTokenStream {
        inner: TokenStream,
    }

    impl AsyncTokenStream {
        /// Create a new async token stream from a sync token stream
        pub fn new(inner: TokenStream) -> Self {
            Self { inner }
        }

        /// Check if the stream is finished
        pub fn is_finished(&self) -> bool {
            self.inner.is_finished()
        }

        /// Get the number of tokens received
        pub fn tokens_received(&self) -> usize {
            self.inner.tokens_received()
        }

        /// Get tokens per second
        pub fn tokens_per_second(&self) -> f64 {
            self.inner.tokens_per_second()
        }
    }

    impl futures_core::Stream for AsyncTokenStream {
        type Item = Result<StreamEvent>;

        fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            // Try to receive without blocking
            match self.inner.try_next() {
                Some(result) => Poll::Ready(Some(result)),
                None => {
                    if self.inner.is_finished() {
                        Poll::Ready(None)
                    } else {
                        // Schedule a wake-up and try again later
                        // In a real implementation, you'd want to use a proper async channel
                        cx.waker().wake_by_ref();
                        Poll::Pending
                    }
                }
            }
        }
    }

    /// Async trait for LLM backends with streaming support
    #[async_trait::async_trait]
    pub trait LlmBackendAsync: Send + Sync {
        /// Generate text with async streaming output
        ///
        /// # Arguments
        ///
        /// * `prompt` - Input text prompt
        /// * `params` - Generation parameters
        ///
        /// # Returns
        ///
        /// An async stream that yields StreamEvents as tokens are generated
        async fn generate_stream_async(
            &self,
            prompt: &str,
            params: GenerateParams,
        ) -> Result<AsyncTokenStream>;
    }

    /// Blanket implementation for any LlmBackend
    #[async_trait::async_trait]
    impl<T: LlmBackend + ?Sized> LlmBackendAsync for T {
        async fn generate_stream_async(
            &self,
            prompt: &str,
            params: GenerateParams,
        ) -> Result<AsyncTokenStream> {
            // Use the sync streaming method and wrap it
            let stream = self.generate_stream_v2(prompt, params)?;
            Ok(AsyncTokenStream::new(stream))
        }
    }
}

#[cfg(feature = "async-runtime")]
pub use async_stream::{AsyncTokenStream, LlmBackendAsync};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantization_bytes() {
        assert_eq!(Quantization::None.bytes_per_weight(), 4.0);
        assert_eq!(Quantization::F16.bytes_per_weight(), 2.0);
        assert_eq!(Quantization::Q4K.bytes_per_weight(), 0.5);
    }

    #[test]
    fn test_generate_params_builder() {
        let params = GenerateParams::default()
            .with_max_tokens(512)
            .with_temperature(0.5)
            .with_top_p(0.95)
            .with_seed(42);

        assert_eq!(params.max_tokens, 512);
        assert_eq!(params.temperature, 0.5);
        assert_eq!(params.top_p, 0.95);
        assert_eq!(params.seed, Some(42));
    }

    #[test]
    fn test_model_architecture() {
        assert_eq!(ModelArchitecture::Mistral.config_name(), "mistral");
        assert_eq!(ModelArchitecture::Llama.config_name(), "llama");
    }
}
