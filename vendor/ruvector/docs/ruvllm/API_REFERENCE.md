# RuvLLM API Reference

Complete API documentation for the RuvLLM crate.

## Table of Contents

- [Core Types](#core-types)
- [Backend Trait](#backend-trait)
- [Candle Backend](#candle-backend)
- [LoRA Module](#lora-module)
- [Optimization Module](#optimization-module)
- [Kernel Functions](#kernel-functions)
- [KV Cache](#kv-cache)
- [Error Handling](#error-handling)

---

## Core Types

### `Precision`

Numeric precision for model weights and KV cache.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precision {
    /// Full 32-bit floating point
    FP32,
    /// Half precision 16-bit float
    FP16,
    /// Brain floating point (16-bit)
    BF16,
    /// 8-bit integer quantization
    Q8,
    /// 4-bit integer quantization
    Q4,
    /// 4-bit K-quant (GGML-style)
    Q4K,
}

impl Precision {
    /// Get bytes per element for this precision
    pub fn bytes_per_element(&self) -> u8;
}
```

### `ModelSize`

Model size classification for routing.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelSize {
    Tiny,   // < 1B params
    Small,  // 1-3B params
    Medium, // 3-13B params
    Large,  // > 13B params
}
```

### `DeviceType`

Compute device selection.

```rust
#[derive(Debug, Clone, Copy)]
pub enum DeviceType {
    /// CPU (fallback)
    Cpu,
    /// Apple Metal GPU
    Metal,
    /// NVIDIA CUDA GPU
    Cuda(usize),  // device index
}
```

---

## Backend Trait

### `LlmBackend`

Main trait for LLM inference backends.

```rust
pub trait LlmBackend: Send + Sync {
    /// Load a model from HuggingFace Hub or local path
    ///
    /// # Arguments
    /// * `model_id` - HuggingFace model ID or local path
    /// * `config` - Model configuration
    ///
    /// # Example
    /// ```
    /// backend.load_model("Qwen/Qwen2.5-7B-Instruct", config)?;
    /// ```
    fn load_model(&mut self, model_id: &str, config: ModelConfig) -> Result<()>;

    /// Generate text from a prompt
    ///
    /// # Arguments
    /// * `prompt` - Input text prompt
    /// * `params` - Generation parameters
    ///
    /// # Returns
    /// Generated text response
    ///
    /// # Example
    /// ```
    /// let response = backend.generate("Hello!", GenerateParams::default())?;
    /// ```
    fn generate(&self, prompt: &str, params: GenerateParams) -> Result<String>;

    /// Streaming text generation
    ///
    /// # Arguments
    /// * `prompt` - Input text prompt
    /// * `params` - Generation parameters
    /// * `callback` - Called for each generated token
    fn generate_stream<F>(&self, prompt: &str, params: GenerateParams, callback: F) -> Result<()>
    where
        F: FnMut(&str) -> bool;

    /// Get the tokenizer for this model
    fn tokenizer(&self) -> Option<&dyn Tokenizer>;

    /// Get model metadata
    fn model_info(&self) -> Option<ModelInfo>;

    /// Check if a model is loaded
    fn is_loaded(&self) -> bool;
}
```

### `ModelConfig`

Configuration for model loading.

```rust
#[derive(Debug, Clone)]
pub struct ModelConfig {
    /// Maximum context length
    pub max_context: usize,
    /// Use Flash Attention
    pub use_flash_attention: bool,
    /// Weight quantization level
    pub quantization: Precision,
    /// KV cache configuration
    pub kv_cache_config: KvCacheConfig,
    /// Device to load model on
    pub device: DeviceType,
    /// HuggingFace token for gated models
    pub hf_token: Option<String>,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            max_context: 4096,
            use_flash_attention: true,
            quantization: Precision::Q4K,
            kv_cache_config: KvCacheConfig::default(),
            device: DeviceType::Metal,
            hf_token: None,
        }
    }
}
```

### `GenerateParams`

Parameters for text generation.

```rust
#[derive(Debug, Clone)]
pub struct GenerateParams {
    /// Maximum tokens to generate
    pub max_tokens: usize,
    /// Sampling temperature (0.0 = deterministic)
    pub temperature: f32,
    /// Top-p (nucleus) sampling
    pub top_p: f32,
    /// Top-k sampling (0 = disabled)
    pub top_k: usize,
    /// Repetition penalty
    pub repetition_penalty: f32,
    /// Stop sequences
    pub stop_sequences: Vec<String>,
    /// Random seed for reproducibility
    pub seed: Option<u64>,
}

impl Default for GenerateParams {
    fn default() -> Self {
        Self {
            max_tokens: 256,
            temperature: 0.7,
            top_p: 0.9,
            top_k: 0,
            repetition_penalty: 1.1,
            stop_sequences: vec![],
            seed: None,
        }
    }
}
```

---

## Candle Backend

### `CandleBackend`

HuggingFace Candle-based inference backend.

```rust
impl CandleBackend {
    /// Create a new backend with default device
    ///
    /// # Example
    /// ```
    /// let backend = CandleBackend::new()?;
    /// ```
    pub fn new() -> Result<Self>;

    /// Create with specific device
    ///
    /// # Example
    /// ```
    /// let backend = CandleBackend::with_device(DeviceType::Metal)?;
    /// ```
    pub fn with_device(device: DeviceType) -> Result<Self>;

    /// Download model from HuggingFace Hub
    ///
    /// # Arguments
    /// * `model_id` - HuggingFace model ID
    /// * `quantization` - Target quantization
    /// * `cache_dir` - Local cache directory
    ///
    /// # Example
    /// ```
    /// let path = backend.download_model(
    ///     "Qwen/Qwen2.5-7B-Instruct",
    ///     Precision::Q4K,
    ///     "~/.cache/ruvllm"
    /// ).await?;
    /// ```
    pub async fn download_model(
        &self,
        model_id: &str,
        quantization: Precision,
        cache_dir: &str,
    ) -> Result<PathBuf>;

    /// Get current device
    pub fn device(&self) -> DeviceType;

    /// Get memory usage statistics
    pub fn memory_stats(&self) -> MemoryStats;
}
```

---

## LoRA Module

### `MicroLoRA`

Real-time per-request fine-tuning with rank 1-2 adapters.

```rust
impl MicroLoRA {
    /// Create a new MicroLoRA instance
    ///
    /// # Example
    /// ```
    /// let config = MicroLoraConfig::for_hidden_dim(4096);
    /// let lora = MicroLoRA::new(config);
    /// ```
    pub fn new(config: MicroLoraConfig) -> Self;

    /// Adapt on new input with feedback
    ///
    /// # Arguments
    /// * `input` - Input embedding vector
    /// * `feedback` - Quality feedback for learning
    ///
    /// # Example
    /// ```
    /// let feedback = AdaptFeedback::from_quality(0.9);
    /// lora.adapt(&input_embedding, feedback)?;
    /// ```
    pub fn adapt(&self, input: &[f32], feedback: AdaptFeedback) -> Result<()>;

    /// Forward pass through LoRA adapter
    ///
    /// # Arguments
    /// * `input` - Input tensor
    /// * `module` - Target module (Q, K, V, O projections)
    ///
    /// # Returns
    /// Output with LoRA contribution added
    ///
    /// # Example
    /// ```
    /// let output = lora.forward(&input, &TargetModule::QProj);
    /// ```
    pub fn forward(&self, input: &[f32], module: &TargetModule) -> Vec<f32>;

    /// Forward pass that adds to existing output (in-place)
    pub fn forward_add(&self, input: &[f32], module: &TargetModule, output: &mut [f32]);

    /// Apply accumulated gradient updates
    ///
    /// # Arguments
    /// * `learning_rate` - Learning rate for update
    pub fn apply_updates(&self, learning_rate: f32);

    /// Apply updates with EWC++ regularization
    ///
    /// # Arguments
    /// * `learning_rate` - Learning rate
    /// * `ewc_states` - EWC++ state per module
    /// * `ewc_lambda` - EWC regularization strength
    pub fn apply_updates_with_ewc(
        &self,
        learning_rate: f32,
        ewc_states: &HashMap<TargetModule, EwcState>,
        ewc_lambda: f32,
    );

    /// Reset all adapter weights
    pub fn reset(&self);

    /// Get adapter statistics
    pub fn stats(&self) -> MicroLoraStats;
}
```

### `MicroLoraConfig`

Configuration for MicroLoRA adapters.

```rust
#[derive(Debug, Clone)]
pub struct MicroLoraConfig {
    /// Input feature dimension
    pub in_features: usize,
    /// Output feature dimension
    pub out_features: usize,
    /// LoRA rank (1-2 for MicroLoRA)
    pub rank: usize,
    /// LoRA alpha scaling factor
    pub alpha: f32,
    /// Dropout probability
    pub dropout: f32,
    /// Target modules to adapt
    pub target_modules: Vec<TargetModule>,
    /// Enable gradient checkpointing
    pub gradient_checkpointing: bool,
}

impl MicroLoraConfig {
    /// Create config for a specific hidden dimension
    ///
    /// # Example
    /// ```
    /// let config = MicroLoraConfig::for_hidden_dim(4096);
    /// assert_eq!(config.in_features, 4096);
    /// assert_eq!(config.rank, 2);
    /// ```
    pub fn for_hidden_dim(hidden_dim: usize) -> Self;
}
```

### `TargetModule`

Transformer modules that can be adapted.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetModule {
    /// Query projection
    QProj,
    /// Key projection
    KProj,
    /// Value projection
    VProj,
    /// Output projection
    OProj,
    /// Gate projection (FFN)
    GateProj,
    /// Up projection (FFN)
    UpProj,
    /// Down projection (FFN)
    DownProj,
}
```

### `AdaptFeedback`

Feedback for LoRA adaptation.

```rust
#[derive(Debug, Clone)]
pub struct AdaptFeedback {
    /// Quality score (0.0 - 1.0)
    pub quality: f32,
    /// Gradient estimate from feedback
    pub gradient_estimate: Vec<f32>,
    /// Optional reward signal
    pub reward: Option<f32>,
    /// Latency in microseconds
    pub latency_us: u64,
    /// Source module (optional)
    pub source_module: Option<TargetModule>,
    /// Session identifier
    pub session_id: Option<String>,
}

impl AdaptFeedback {
    /// Create feedback from quality score
    ///
    /// # Example
    /// ```
    /// let feedback = AdaptFeedback::from_quality(0.85);
    /// ```
    pub fn from_quality(quality: f32) -> Self;
}
```

---

## Optimization Module

### `SonaLlm`

SONA learning integration for LLM inference.

```rust
impl SonaLlm {
    /// Create new SONA LLM integration
    ///
    /// # Example
    /// ```
    /// let sona = SonaLlm::new(SonaLlmConfig::default());
    /// ```
    pub fn new(config: SonaLlmConfig) -> Self;

    /// Instant loop: per-request MicroLoRA adaptation
    ///
    /// Target latency: <1ms
    ///
    /// # Arguments
    /// * `request` - User query text
    /// * `response` - Model response text
    /// * `feedback` - Quality score (0.0 - 1.0)
    ///
    /// # Returns
    /// Adaptation result with statistics
    ///
    /// # Example
    /// ```
    /// let result = sona.instant_adapt(
    ///     "What is machine learning?",
    ///     "Machine learning is...",
    ///     0.9
    /// );
    /// assert!(result.applied);
    /// assert!(result.latency_us < 1000); // <1ms
    /// ```
    pub fn instant_adapt(&self, request: &str, response: &str, feedback: f32) -> AdaptationResult;

    /// Background loop: consolidate patterns
    ///
    /// Called periodically (~100ms interval)
    ///
    /// # Example
    /// ```
    /// let result = sona.background_consolidate();
    /// println!("Consolidated {} samples", result.samples_used);
    /// ```
    pub fn background_consolidate(&self) -> AdaptationResult;

    /// Deep loop: trigger full optimization
    ///
    /// # Arguments
    /// * `dataset` - Training samples to learn from
    pub fn deep_optimize(&self, dataset: &[TrainingSample]) -> AdaptationResult;

    /// Check if background loop should run
    pub fn maybe_background(&self) -> Option<AdaptationResult>;

    /// Check if deep loop should be triggered
    pub fn should_trigger_deep(&self) -> bool;

    /// Get current statistics
    pub fn stats(&self) -> LearningLoopStats;

    /// Forward pass through MicroLoRA
    pub fn forward(&self, input: &[f32], module: &TargetModule) -> Vec<f32>;

    /// Reset all learning state
    pub fn reset(&self);
}
```

### `SonaLlmConfig`

Configuration for SONA LLM integration.

```rust
#[derive(Debug, Clone)]
pub struct SonaLlmConfig {
    /// MicroLoRA configuration
    pub micro_lora: MicroLoraConfig,
    /// Training pipeline configuration
    pub training: TrainingConfig,
    /// SONA core configuration
    pub sona: SonaConfig,
    /// Instant loop learning rate
    pub instant_lr: f32,
    /// Background loop interval (milliseconds)
    pub background_interval_ms: u64,
    /// Minimum samples for background consolidation
    pub background_min_samples: usize,
    /// Deep loop trigger threshold
    pub deep_trigger_threshold: f32,
    /// Maximum pending samples
    pub max_pending_samples: usize,
    /// Consolidation strategy
    pub consolidation_strategy: ConsolidationStrategy,
}
```

### `ConsolidationStrategy`

Strategy for consolidating learned patterns.

```rust
#[derive(Debug, Clone, Copy)]
pub enum ConsolidationStrategy {
    /// Merge with EWC++ regularization (default)
    EwcMerge,
    /// Simple averaging
    Average,
    /// Weighted by quality
    QualityWeighted,
    /// Keep best performing only
    BestOnly,
    /// Ensemble multiple adapters
    Ensemble,
}
```

---

## Kernel Functions

### Attention Kernels

```rust
/// Flash Attention 2 with NEON SIMD optimization
///
/// Memory-efficient attention with O(N) complexity.
///
/// # Arguments
/// * `query` - Query tensor (head_dim,)
/// * `key` - Key tensor (kv_len, head_dim)
/// * `value` - Value tensor (kv_len, head_dim)
/// * `scale` - Softmax scale (typically 1/sqrt(head_dim))
/// * `causal` - Apply causal masking
///
/// # Returns
/// Output tensor (head_dim,)
///
/// # Example
/// ```
/// let scale = 1.0 / (head_dim as f32).sqrt();
/// let output = flash_attention_neon(&query, &key, &value, scale, true);
/// ```
pub fn flash_attention_neon(
    query: &[f32],
    key: &[f32],
    value: &[f32],
    scale: f32,
    causal: bool,
) -> Vec<f32>;

/// Paged Attention for KV cache
///
/// # Arguments
/// * `query` - Query tensor
/// * `kv_cache` - Paged KV cache
/// * `block_tables` - Block index mapping
/// * `scale` - Softmax scale
pub fn paged_attention_neon(
    query: &[f32],
    kv_cache: &PagedKvCache,
    block_tables: &[usize],
    scale: f32,
) -> Vec<f32>;

/// Grouped-Query Attention (GQA)
///
/// KV heads shared among query head groups.
///
/// # Arguments
/// * `queries` - Query tensor (num_heads, head_dim)
/// * `keys` - Key tensor (kv_len, num_kv_heads, head_dim)
/// * `values` - Value tensor (kv_len, num_kv_heads, head_dim)
/// * `config` - Attention configuration
pub fn grouped_query_attention_neon(
    queries: &[f32],
    keys: &[f32],
    values: &[f32],
    config: &AttentionConfig,
) -> Vec<f32>;

/// Multi-Query Attention (MQA)
///
/// Single KV head shared across all query heads.
pub fn multi_query_attention_neon(
    queries: &[f32],
    key: &[f32],
    value: &[f32],
    config: &AttentionConfig,
) -> Vec<f32>;
```

### `AttentionConfig`

Configuration for attention operations.

```rust
#[derive(Debug, Clone)]
pub struct AttentionConfig {
    /// Number of query heads
    pub num_heads: usize,
    /// Number of KV heads (for GQA)
    pub num_kv_heads: usize,
    /// Dimension per head
    pub head_dim: usize,
    /// Apply causal masking
    pub causal: bool,
    /// Custom scale factor (None = 1/sqrt(head_dim))
    pub scale: Option<f32>,
}

impl AttentionConfig {
    /// Calculate GQA ratio (query heads / KV heads)
    pub fn gqa_ratio(&self) -> usize;

    /// Get effective scale factor
    pub fn effective_scale(&self) -> f32;
}
```

---

## KV Cache

### `TwoTierKvCache`

Two-tier KV cache with FP16 tail and quantized store.

```rust
impl TwoTierKvCache {
    /// Create a new two-tier KV cache
    ///
    /// # Example
    /// ```
    /// let config = KvCacheConfig {
    ///     tail_length: 256,
    ///     max_tokens: 4096,
    ///     ..Default::default()
    /// };
    /// let cache = TwoTierKvCache::new(config);
    /// ```
    pub fn new(config: KvCacheConfig) -> Self;

    /// Append new KV pairs
    ///
    /// Automatically handles:
    /// - Adding to tail
    /// - Migrating to quantized store
    /// - Evicting oldest tokens
    ///
    /// # Arguments
    /// * `keys` - Key tensor
    /// * `values` - Value tensor
    ///
    /// # Example
    /// ```
    /// cache.append(&keys, &values)?;
    /// ```
    pub fn append(&self, keys: &[f32], values: &[f32]) -> Result<()>;

    /// Get all KV pairs for attention
    ///
    /// Returns (keys, values) with cold tier dequantized.
    pub fn get_all_kv(&self) -> (Vec<f32>, Vec<f32>);

    /// Compute attention with tier-aware access
    ///
    /// # Arguments
    /// * `query` - Query tensor
    /// * `scale` - Softmax scale
    pub fn attend(&self, query: &[f32], scale: f32) -> Result<Vec<f32>>;

    /// Get current statistics
    pub fn stats(&self) -> KvCacheStats;

    /// Clear the cache
    pub fn clear(&self);

    /// Update quantization policy
    pub fn update_policy(&self, policy: CacheQuantization);
}
```

### `KvCacheConfig`

Configuration for KV cache.

```rust
#[derive(Debug, Clone)]
pub struct KvCacheConfig {
    /// Tokens to keep in high-precision tail
    pub tail_length: usize,
    /// Precision for tail storage
    pub tail_precision: Precision,
    /// Precision for quantized store
    pub store_precision: Precision,
    /// Maximum total tokens
    pub max_tokens: usize,
    /// Number of KV heads
    pub num_kv_heads: usize,
    /// Head dimension
    pub head_dim: usize,
    /// Migration batch size
    pub migration_batch: usize,
}
```

### `KvCacheStats`

Statistics for KV cache usage.

```rust
#[derive(Debug, Clone)]
pub struct KvCacheStats {
    /// Total tokens cached
    pub total_tokens: usize,
    /// Tokens in high-precision tail
    pub tail_tokens: usize,
    /// Tokens in quantized store
    pub store_tokens: usize,
    /// Bytes used by tail
    pub tail_bytes: usize,
    /// Bytes used by store
    pub store_bytes: usize,
    /// Compression ratio
    pub compression_ratio: f32,
}
```

---

## Error Handling

### `RuvLLMError`

Main error type for RuvLLM operations.

```rust
#[derive(Error, Debug)]
pub enum RuvLLMError {
    /// Storage-related errors
    #[error("Storage error: {0}")]
    Storage(String),

    /// Session management errors
    #[error("Session error: {0}")]
    Session(String),

    /// KV cache errors
    #[error("KV cache error: {0}")]
    KvCache(String),

    /// Paged attention errors
    #[error("Paged attention error: {0}")]
    PagedAttention(String),

    /// Adapter management errors
    #[error("Adapter error: {0}")]
    Adapter(String),

    /// SONA learning errors
    #[error("SONA error: {0}")]
    Sona(String),

    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// Out of memory
    #[error("Out of memory: {0}")]
    OutOfMemory(String),

    /// Invalid operation
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    /// Not found
    #[error("Not found: {0}")]
    NotFound(String),

    /// Backend inference errors
    #[error("Backend error: {0}")]
    Backend(String),

    /// Model loading errors
    #[error("Model error: {0}")]
    Model(String),

    /// Tokenization errors
    #[error("Tokenization error: {0}")]
    Tokenization(String),

    /// Generation errors
    #[error("Generation error: {0}")]
    Generation(String),

    /// IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
```

### `Result` Type Alias

```rust
/// Result type alias for RuvLLM operations
pub type Result<T> = std::result::Result<T, RuvLLMError>;
```

---

## Feature Flags Reference

| Feature | Dependencies | Description |
|---------|-------------|-------------|
| `default` | `async-runtime` | Standard async support |
| `async-runtime` | `tokio` | Tokio async runtime |
| `wasm` | - | WebAssembly support |
| `candle` | `candle-*`, `tokenizers`, `hf-hub` | Candle ML backend |
| `metal` | `candle/metal` | Apple Metal GPU |
| `cuda` | `candle/cuda` | NVIDIA CUDA GPU |
| `inference-metal` | `candle`, `metal` | Full Metal stack |
| `inference-cuda` | `candle`, `cuda` | Full CUDA stack |
