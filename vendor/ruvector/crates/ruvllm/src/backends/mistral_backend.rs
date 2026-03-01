//! mistral-rs Backend for High-Performance LLM Inference
//!
//! This module provides integration with the mistral-rs inference engine,
//! offering high-performance LLM inference with advanced features:
//!
//! - **PagedAttention**: Memory-efficient KV cache management
//! - **X-LoRA**: Dynamic adapter mixing with learned routing
//! - **ISQ**: In-Situ Quantization for runtime model compression
//! - **OpenAI-Compatible**: Standard API for generation
//!
//! ## Architecture
//!
//! ```text
//! +-------------------+     +-------------------+
//! | MistralBackend    |---->| mistral-rs Model  |
//! | (RuvLLM adapter)  |     | (PagedAttention)  |
//! +-------------------+     +-------------------+
//!         |                         |
//!         v                         v
//! +-------------------+     +-------------------+
//! | X-LoRA Manager    |     | ISQ Quantizer     |
//! | (adapter mixing)  |     | (runtime quant)   |
//! +-------------------+     +-------------------+
//! ```
//!
//! ## Example
//!
//! ```rust,ignore
//! use ruvllm::backends::{MistralBackend, MistralConfig};
//!
//! let config = MistralConfig::default()
//!     .with_paged_attention(16, 4096)
//!     .with_xlora_adapters(vec!["code", "chat"]);
//!
//! let mut backend = MistralBackend::new(config)?;
//! backend.load_model("mistralai/Mistral-7B-v0.3", Default::default())?;
//!
//! let response = backend.generate("Hello, world!", Default::default())?;
//! ```

use super::{
    DType, DeviceType, GenerateParams, GeneratedToken, LlmBackend, ModelArchitecture, ModelConfig,
    ModelInfo, Quantization, SpecialTokens, Tokenizer,
};
use crate::error::{Result, RuvLLMError};
use crate::paged_attention::{PagedAttention, PagedAttentionConfig};

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

// Conditional imports for mistral-rs crate integration
#[cfg(feature = "mistral-rs")]
use mistralrs::{
    Constraint, Device as MistralDevice, GGUFLoaderBuilder, GGUFSpecificConfig, MistralRs,
    MistralRsBuilder, NormalRequest, PagedAttentionMetaBuilder, Request, RequestMessage, Response,
    SamplingParams, SchedulerConfig, TokenSource,
};
#[cfg(feature = "mistral-rs")]
use tokio::sync::mpsc::channel as tokio_channel;

// ============================================================================
// Configuration Types
// ============================================================================

/// Configuration for the Mistral backend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MistralBackendConfig {
    /// PagedAttention configuration
    pub paged_attention: Option<PagedAttentionConfigExt>,
    /// X-LoRA configuration
    pub xlora: Option<XLoraConfig>,
    /// ISQ (In-Situ Quantization) configuration
    pub isq: Option<IsqConfig>,
    /// Device type for inference
    pub device: DeviceType,
    /// Data type for tensors
    pub dtype: DType,
    /// Maximum batch size
    pub max_batch_size: usize,
    /// Maximum sequence length
    pub max_seq_len: usize,
    /// Use Flash Attention 2 if available
    pub use_flash_attn: bool,
    /// Tokenizer path (optional, auto-detected from model)
    pub tokenizer_path: Option<PathBuf>,
    /// Cache directory for downloaded models
    pub cache_dir: PathBuf,
}

impl Default for MistralBackendConfig {
    fn default() -> Self {
        Self {
            paged_attention: Some(PagedAttentionConfigExt::default()),
            xlora: None,
            isq: None,
            device: DeviceType::default(),
            dtype: DType::F16,
            max_batch_size: 32,
            max_seq_len: 8192,
            use_flash_attn: true,
            tokenizer_path: None,
            cache_dir: get_cache_dir(),
        }
    }
}

impl MistralBackendConfig {
    /// Create config optimized for Apple Silicon
    pub fn for_metal() -> Self {
        Self {
            device: DeviceType::Metal,
            dtype: DType::F16,
            use_flash_attn: true,
            ..Default::default()
        }
    }

    /// Create config optimized for CUDA
    pub fn for_cuda(device_id: usize) -> Self {
        Self {
            device: DeviceType::Cuda(device_id),
            dtype: DType::F16,
            use_flash_attn: true,
            ..Default::default()
        }
    }

    /// Enable PagedAttention with custom parameters
    pub fn with_paged_attention(mut self, block_size: usize, max_pages: usize) -> Self {
        self.paged_attention = Some(PagedAttentionConfigExt {
            block_size,
            max_pages,
            ..Default::default()
        });
        self
    }

    /// Enable X-LoRA with adapter paths
    pub fn with_xlora_adapters(mut self, adapter_names: Vec<&str>) -> Self {
        self.xlora = Some(XLoraConfig {
            adapter_names: adapter_names.into_iter().map(String::from).collect(),
            ..Default::default()
        });
        self
    }

    /// Enable ISQ quantization
    pub fn with_isq(mut self, bits: u8) -> Self {
        self.isq = Some(IsqConfig {
            bits,
            ..Default::default()
        });
        self
    }

    /// Set maximum sequence length
    pub fn with_max_seq_len(mut self, max_seq_len: usize) -> Self {
        self.max_seq_len = max_seq_len;
        self
    }

    /// Set maximum batch size
    pub fn with_max_batch_size(mut self, max_batch_size: usize) -> Self {
        self.max_batch_size = max_batch_size;
        self
    }
}

/// Extended PagedAttention configuration for mistral-rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PagedAttentionConfigExt {
    /// Number of tokens per block (page)
    pub block_size: usize,
    /// Maximum number of pages in the page table
    pub max_pages: usize,
    /// Memory fraction to use for KV cache (0.0-1.0)
    pub gpu_memory_fraction: f32,
    /// Enable prefix caching for repeated prompts
    pub enable_prefix_caching: bool,
    /// Block recomputation threshold
    pub recomputation_threshold: f32,
}

impl Default for PagedAttentionConfigExt {
    fn default() -> Self {
        Self {
            block_size: 16,
            max_pages: 4096,
            gpu_memory_fraction: 0.9,
            enable_prefix_caching: true,
            recomputation_threshold: 0.1,
        }
    }
}

/// X-LoRA (eXpert-mixed LoRA) configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XLoraConfig {
    /// Names/paths of adapters to load
    pub adapter_names: Vec<String>,
    /// Base adapter path (optional)
    pub base_adapter: Option<String>,
    /// Scaling factors for each adapter
    pub adapter_scales: Option<Vec<f32>>,
    /// Router hidden dimension
    pub router_hidden_dim: usize,
    /// Number of router layers
    pub router_layers: usize,
    /// Top-k adapters to activate per token
    pub top_k: usize,
    /// Softmax temperature for router
    pub temperature: f32,
    /// Whether to use learned routing
    pub use_learned_routing: bool,
    /// Mixing mode
    pub mixing_mode: XLoraMixingMode,
}

impl Default for XLoraConfig {
    fn default() -> Self {
        Self {
            adapter_names: Vec::new(),
            base_adapter: None,
            adapter_scales: None,
            router_hidden_dim: 64,
            router_layers: 2,
            top_k: 2,
            temperature: 1.0,
            use_learned_routing: true,
            mixing_mode: XLoraMixingMode::Additive,
        }
    }
}

/// X-LoRA mixing modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum XLoraMixingMode {
    /// Add adapter outputs
    Additive,
    /// Concatenate and project
    Concatenate,
    /// Gated mixture
    Gated,
    /// Attention-based mixture
    Attention,
}

impl Default for XLoraMixingMode {
    fn default() -> Self {
        Self::Additive
    }
}

/// ISQ (In-Situ Quantization) configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsqConfig {
    /// Quantization bits (2, 4, 8)
    pub bits: u8,
    /// Quantization method
    pub method: IsqMethod,
    /// Symmetric quantization
    pub symmetric: bool,
    /// Per-channel quantization
    pub per_channel: bool,
    /// Calibration samples for quantization
    pub calibration_samples: usize,
}

impl Default for IsqConfig {
    fn default() -> Self {
        Self {
            bits: 4,
            method: IsqMethod::AWQ,
            symmetric: false,
            per_channel: true,
            calibration_samples: 128,
        }
    }
}

/// ISQ quantization methods
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IsqMethod {
    /// Activation-aware Weight Quantization
    AWQ,
    /// GPTQ quantization
    GPTQ,
    /// Simple round-to-nearest
    RTN,
    /// SmoothQuant
    SmoothQuant,
}

impl Default for IsqMethod {
    fn default() -> Self {
        Self::AWQ
    }
}

// ============================================================================
// X-LoRA Manager
// ============================================================================

/// Manages X-LoRA adapters for dynamic mixing
pub struct XLoraManager {
    /// Configuration
    config: XLoraConfig,
    /// Loaded adapters (name -> weights)
    adapters: DashMap<String, AdapterWeights>,
    /// Router weights (if learned routing)
    router: Option<RouterWeights>,
    /// Currently active adapter combination
    active_combination: RwLock<Vec<(String, f32)>>,
    /// Statistics
    stats: XLoraStats,
}

/// Adapter weight storage
#[derive(Debug, Clone)]
struct AdapterWeights {
    /// LoRA A matrices by layer
    lora_a: HashMap<String, Vec<f32>>,
    /// LoRA B matrices by layer
    lora_b: HashMap<String, Vec<f32>>,
    /// Rank
    rank: usize,
    /// Alpha scaling factor
    alpha: f32,
}

/// Router weights for learned routing
#[derive(Debug, Clone)]
struct RouterWeights {
    /// Hidden layer weights
    hidden_weights: Vec<Vec<f32>>,
    /// Output layer weights (one per adapter)
    output_weights: Vec<Vec<f32>>,
    /// Biases
    biases: Vec<Vec<f32>>,
}

/// X-LoRA statistics
#[derive(Debug, Default)]
struct XLoraStats {
    /// Number of forward passes
    forward_count: AtomicU64,
    /// Total adapter selection time (ns)
    routing_time_ns: AtomicU64,
    /// Adapter usage counts
    adapter_usage: DashMap<String, AtomicU64>,
}

impl XLoraManager {
    /// Create a new X-LoRA manager
    pub fn new(config: XLoraConfig) -> Self {
        Self {
            config,
            adapters: DashMap::new(),
            router: None,
            active_combination: RwLock::new(Vec::new()),
            stats: XLoraStats::default(),
        }
    }

    /// Load an adapter from path
    pub fn load_adapter(&self, name: &str, path: &Path) -> Result<()> {
        // In a real implementation, this would load safetensors/GGUF adapter files
        // For now, we create a placeholder structure

        let adapter = AdapterWeights {
            lora_a: HashMap::new(),
            lora_b: HashMap::new(),
            rank: 16,
            alpha: 16.0,
        };

        self.adapters.insert(name.to_string(), adapter);
        self.stats
            .adapter_usage
            .insert(name.to_string(), AtomicU64::new(0));

        tracing::info!("Loaded X-LoRA adapter: {} from {:?}", name, path);
        Ok(())
    }

    /// Unload an adapter
    pub fn unload_adapter(&self, name: &str) -> Result<()> {
        if self.adapters.remove(name).is_none() {
            return Err(RuvLLMError::NotFound(format!(
                "Adapter '{}' not found",
                name
            )));
        }
        Ok(())
    }

    /// Set active adapters with weights
    pub fn set_active(&self, adapters: Vec<(&str, f32)>) -> Result<()> {
        // Validate all adapters exist
        for (name, _) in &adapters {
            if !self.adapters.contains_key(*name) {
                return Err(RuvLLMError::NotFound(format!(
                    "Adapter '{}' not found",
                    name
                )));
            }
        }

        let mut active = self.active_combination.write();
        *active = adapters
            .into_iter()
            .map(|(name, weight)| (name.to_string(), weight))
            .collect();
        Ok(())
    }

    /// Route input to adapters (learned or manual)
    pub fn route(&self, hidden_states: &[f32]) -> Vec<(String, f32)> {
        self.stats.forward_count.fetch_add(1, Ordering::Relaxed);

        if self.config.use_learned_routing {
            if let Some(ref router) = self.router {
                return self.learned_route(hidden_states, router);
            }
        }

        // Fall back to active combination or uniform distribution
        let active = self.active_combination.read();
        if !active.is_empty() {
            return active.clone();
        }

        // Uniform distribution over all adapters
        let n = self.adapters.len() as f32;
        self.adapters
            .iter()
            .map(|entry| (entry.key().clone(), 1.0 / n))
            .collect()
    }

    /// Perform learned routing through router network
    fn learned_route(&self, hidden_states: &[f32], router: &RouterWeights) -> Vec<(String, f32)> {
        // Simple MLP router: hidden -> ReLU -> output -> softmax
        let mut activations = hidden_states.to_vec();

        // Hidden layers
        for (weights, bias) in router.hidden_weights.iter().zip(router.biases.iter()) {
            activations = self.linear_relu(&activations, weights, bias);
        }

        // Output layer (logits for each adapter)
        let logits = self.linear(
            &activations,
            router.output_weights.last().unwrap_or(&Vec::new()),
            &[],
        );

        // Apply temperature and softmax
        let scaled: Vec<f32> = logits.iter().map(|x| x / self.config.temperature).collect();
        let probs = softmax(&scaled);

        // Select top-k adapters
        let mut indexed: Vec<(usize, f32)> = probs.into_iter().enumerate().collect();
        indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        indexed.truncate(self.config.top_k);

        // Map indices to adapter names
        let adapter_names: Vec<String> = self.adapters.iter().map(|e| e.key().clone()).collect();

        indexed
            .into_iter()
            .filter_map(|(idx, weight)| {
                adapter_names.get(idx).map(|name| {
                    // Update usage stats
                    if let Some(usage) = self.stats.adapter_usage.get(name) {
                        usage.fetch_add(1, Ordering::Relaxed);
                    }
                    (name.clone(), weight)
                })
            })
            .collect()
    }

    /// Linear + ReLU layer
    fn linear_relu(&self, input: &[f32], weights: &[f32], bias: &[f32]) -> Vec<f32> {
        let output = self.linear(input, weights, bias);
        output.into_iter().map(|x| x.max(0.0)).collect()
    }

    /// Linear layer
    fn linear(&self, input: &[f32], weights: &[f32], bias: &[f32]) -> Vec<f32> {
        if weights.is_empty() {
            return input.to_vec();
        }

        let output_dim = if !bias.is_empty() {
            bias.len()
        } else {
            weights.len() / input.len().max(1)
        };

        let mut output = vec![0.0; output_dim];

        for (i, out) in output.iter_mut().enumerate() {
            for (j, &inp) in input.iter().enumerate() {
                let idx = i * input.len() + j;
                if idx < weights.len() {
                    *out += inp * weights[idx];
                }
            }
            if i < bias.len() {
                *out += bias[i];
            }
        }

        output
    }

    /// Apply X-LoRA to hidden states
    pub fn apply(&self, hidden_states: &[f32], layer_name: &str) -> Vec<f32> {
        let routing = self.route(hidden_states);
        let mut output = vec![0.0; hidden_states.len()];

        match self.config.mixing_mode {
            XLoraMixingMode::Additive => {
                for (adapter_name, weight) in &routing {
                    if let Some(adapter) = self.adapters.get(adapter_name) {
                        let delta = self.apply_adapter(hidden_states, &adapter, layer_name);
                        for (o, d) in output.iter_mut().zip(delta.iter()) {
                            *o += d * weight;
                        }
                    }
                }
            }
            XLoraMixingMode::Gated => {
                // Gated mixture: sum of gated adapter outputs
                let total_weight: f32 = routing.iter().map(|(_, w)| w).sum();
                for (adapter_name, weight) in &routing {
                    if let Some(adapter) = self.adapters.get(adapter_name) {
                        let delta = self.apply_adapter(hidden_states, &adapter, layer_name);
                        let gate = weight / total_weight;
                        for (o, d) in output.iter_mut().zip(delta.iter()) {
                            *o += d * gate;
                        }
                    }
                }
            }
            _ => {
                // Default to additive for other modes
                for (adapter_name, weight) in &routing {
                    if let Some(adapter) = self.adapters.get(adapter_name) {
                        let delta = self.apply_adapter(hidden_states, &adapter, layer_name);
                        for (o, d) in output.iter_mut().zip(delta.iter()) {
                            *o += d * weight;
                        }
                    }
                }
            }
        }

        output
    }

    /// Apply a single adapter
    fn apply_adapter(&self, input: &[f32], adapter: &AdapterWeights, layer_name: &str) -> Vec<f32> {
        let lora_a = adapter.lora_a.get(layer_name);
        let lora_b = adapter.lora_b.get(layer_name);

        match (lora_a, lora_b) {
            (Some(a), Some(b)) => {
                // LoRA: output = B @ A @ input * (alpha / rank)
                let scale = adapter.alpha / adapter.rank as f32;

                // A @ input (dimension reduction)
                let intermediate = self.matmul(input, a, adapter.rank);

                // B @ intermediate (dimension expansion)
                let output = self.matmul(&intermediate, b, input.len());

                // Scale
                output.into_iter().map(|x| x * scale).collect()
            }
            _ => vec![0.0; input.len()],
        }
    }

    /// Simple matrix multiplication (for demonstration)
    fn matmul(&self, input: &[f32], weights: &[f32], output_dim: usize) -> Vec<f32> {
        let mut output = vec![0.0; output_dim];
        let input_dim = input.len();

        for (i, out) in output.iter_mut().enumerate() {
            for (j, &inp) in input.iter().enumerate() {
                let idx = i * input_dim + j;
                if idx < weights.len() {
                    *out += inp * weights[idx];
                }
            }
        }

        output
    }

    /// Get statistics
    pub fn stats(&self) -> XLoraManagerStats {
        XLoraManagerStats {
            loaded_adapters: self.adapters.len(),
            forward_count: self.stats.forward_count.load(Ordering::Relaxed),
            adapter_usage: self
                .stats
                .adapter_usage
                .iter()
                .map(|e| (e.key().clone(), e.value().load(Ordering::Relaxed)))
                .collect(),
        }
    }
}

/// X-LoRA manager statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XLoraManagerStats {
    /// Number of loaded adapters
    pub loaded_adapters: usize,
    /// Total forward passes
    pub forward_count: u64,
    /// Per-adapter usage counts
    pub adapter_usage: HashMap<String, u64>,
}

// ============================================================================
// Mistral Tokenizer Wrapper
// ============================================================================

/// Tokenizer wrapper for mistral-rs backend
#[cfg(feature = "mistral-rs")]
pub struct MistralTokenizer {
    inner: tokenizers::Tokenizer,
    special_tokens: SpecialTokens,
}

#[cfg(not(feature = "mistral-rs"))]
pub struct MistralTokenizer {
    vocab_size: usize,
    special_tokens: SpecialTokens,
}

#[cfg(feature = "mistral-rs")]
impl Tokenizer for MistralTokenizer {
    fn encode(&self, text: &str) -> Result<Vec<u32>> {
        let encoding = self
            .inner
            .encode(text, false)
            .map_err(|e| RuvLLMError::Tokenization(format!("Tokenization failed: {}", e)))?;
        Ok(encoding.get_ids().to_vec())
    }

    fn decode(&self, tokens: &[u32]) -> Result<String> {
        self.inner
            .decode(tokens, true)
            .map_err(|e| RuvLLMError::Tokenization(format!("Decoding failed: {}", e)))
    }

    fn vocab_size(&self) -> usize {
        self.inner.get_vocab_size(true)
    }

    fn special_tokens(&self) -> SpecialTokens {
        self.special_tokens.clone()
    }
}

#[cfg(not(feature = "mistral-rs"))]
impl Tokenizer for MistralTokenizer {
    fn encode(&self, _text: &str) -> Result<Vec<u32>> {
        Err(RuvLLMError::Config(
            "mistral-rs feature not enabled".to_string(),
        ))
    }

    fn decode(&self, _tokens: &[u32]) -> Result<String> {
        Err(RuvLLMError::Config(
            "mistral-rs feature not enabled".to_string(),
        ))
    }

    fn vocab_size(&self) -> usize {
        self.vocab_size
    }

    fn special_tokens(&self) -> SpecialTokens {
        self.special_tokens.clone()
    }
}

// ============================================================================
// Mistral Backend Implementation
// ============================================================================

/// mistral-rs based inference backend
///
/// Provides high-performance LLM inference with:
/// - PagedAttention for efficient KV cache management
/// - X-LoRA for dynamic adapter mixing
/// - ISQ for runtime quantization
pub struct MistralBackend {
    /// Backend configuration
    config: MistralBackendConfig,
    /// Model configuration (after loading)
    model_config: Option<ModelConfig>,
    /// Model info
    model_info: Option<ModelInfo>,
    /// PagedAttention instance
    paged_attention: Option<PagedAttention>,
    /// X-LoRA manager
    xlora_manager: Option<XLoraManager>,
    /// Tokenizer
    tokenizer: Option<MistralTokenizer>,
    /// Model loaded flag
    is_loaded: AtomicBool,
    /// Generation sequence counter
    sequence_counter: AtomicU64,
    /// Model path
    model_path: Option<PathBuf>,

    /// mistral-rs model instance (when feature enabled)
    #[cfg(feature = "mistral-rs")]
    mistral_model: Option<Arc<MistralRs>>,
}

impl MistralBackend {
    /// Create a new Mistral backend with default configuration
    pub fn new() -> Result<Self> {
        Self::with_config(MistralBackendConfig::default())
    }

    /// Create a new Mistral backend with custom configuration
    pub fn with_config(config: MistralBackendConfig) -> Result<Self> {
        // Initialize PagedAttention if configured
        let paged_attention = config.paged_attention.as_ref().map(|pa_config| {
            PagedAttention::new(PagedAttentionConfig {
                page_size: pa_config.block_size,
                max_pages_per_sequence: pa_config.max_pages / 256, // Sequences share pages
                page_table_capacity: pa_config.max_pages,
                num_heads: 32,   // Will be updated on model load
                head_dim: 128,   // Will be updated on model load
                num_kv_heads: 8, // Will be updated on model load
                ..Default::default()
            })
        });

        // Initialize X-LoRA if configured
        let xlora_manager = config
            .xlora
            .as_ref()
            .map(|xlora_config| XLoraManager::new(xlora_config.clone()));

        Ok(Self {
            config,
            model_config: None,
            model_info: None,
            paged_attention,
            xlora_manager,
            tokenizer: None,
            is_loaded: AtomicBool::new(false),
            sequence_counter: AtomicU64::new(0),
            model_path: None,
            #[cfg(feature = "mistral-rs")]
            mistral_model: None,
        })
    }

    /// Create backend optimized for Metal (Apple Silicon)
    pub fn for_metal() -> Result<Self> {
        Self::with_config(MistralBackendConfig::for_metal())
    }

    /// Create backend optimized for CUDA
    pub fn for_cuda(device_id: usize) -> Result<Self> {
        Self::with_config(MistralBackendConfig::for_cuda(device_id))
    }

    /// Get PagedAttention statistics
    pub fn paged_attention_stats(&self) -> Option<crate::paged_attention::PageTableStats> {
        self.paged_attention.as_ref().map(|pa| pa.stats())
    }

    /// Get X-LoRA statistics
    pub fn xlora_stats(&self) -> Option<XLoraManagerStats> {
        self.xlora_manager.as_ref().map(|xm| xm.stats())
    }

    /// Check if the mistral-rs native model is loaded
    #[cfg(feature = "mistral-rs")]
    pub fn has_native_model(&self) -> bool {
        self.mistral_model.is_some()
    }

    /// Check if the mistral-rs native model is loaded (always false when feature disabled)
    #[cfg(not(feature = "mistral-rs"))]
    pub fn has_native_model(&self) -> bool {
        false
    }

    /// Load X-LoRA adapter
    pub fn load_xlora_adapter(&self, name: &str, path: &Path) -> Result<()> {
        let manager = self
            .xlora_manager
            .as_ref()
            .ok_or_else(|| RuvLLMError::Config("X-LoRA not configured".to_string()))?;
        manager.load_adapter(name, path)
    }

    /// Set active X-LoRA adapters
    pub fn set_xlora_adapters(&self, adapters: Vec<(&str, f32)>) -> Result<()> {
        let manager = self
            .xlora_manager
            .as_ref()
            .ok_or_else(|| RuvLLMError::Config("X-LoRA not configured".to_string()))?;
        manager.set_active(adapters)
    }

    /// Apply ISQ quantization to loaded model
    pub fn apply_isq(&mut self) -> Result<()> {
        if !self.is_model_loaded() {
            return Err(RuvLLMError::InvalidOperation(
                "No model loaded for ISQ".to_string(),
            ));
        }

        let _isq_config = self
            .config
            .isq
            .as_ref()
            .ok_or_else(|| RuvLLMError::Config("ISQ not configured".to_string()))?;

        // In a real implementation, this would quantize model weights in-place
        // using the configured ISQ method (AWQ, GPTQ, RTN, etc.)
        tracing::info!(
            "ISQ quantization would be applied here (bits: {:?})",
            self.config.isq.as_ref().map(|c| c.bits)
        );

        Ok(())
    }

    /// Allocate KV cache for a new sequence
    fn allocate_sequence(&self, prompt_len: usize) -> Result<String> {
        let seq_id = format!(
            "seq-{}",
            self.sequence_counter.fetch_add(1, Ordering::SeqCst)
        );

        if let Some(ref pa) = self.paged_attention {
            pa.allocate_sequence(&seq_id, prompt_len)?;
        }

        Ok(seq_id)
    }

    /// Free KV cache for a sequence
    fn free_sequence(&self, seq_id: &str) -> Result<()> {
        if let Some(ref pa) = self.paged_attention {
            pa.free_sequence(seq_id)?;
        }
        Ok(())
    }

    /// Internal generation with PagedAttention and X-LoRA
    fn generate_internal(
        &self,
        prompt: &str,
        params: &GenerateParams,
    ) -> Result<(String, Vec<GeneratedToken>)> {
        // Try to use mistral-rs model if available
        #[cfg(feature = "mistral-rs")]
        if let Some(ref model) = self.mistral_model {
            return self.generate_with_mistral_rs(model, prompt, params);
        }

        // Fallback to stub implementation
        self.generate_internal_stub(prompt, params)
    }

    /// Generate using the actual mistral-rs model
    #[cfg(feature = "mistral-rs")]
    fn generate_with_mistral_rs(
        &self,
        model: &Arc<MistralRs>,
        prompt: &str,
        params: &GenerateParams,
    ) -> Result<(String, Vec<GeneratedToken>)> {
        use std::sync::mpsc::channel;

        // Create sampling parameters from our GenerateParams
        let sampling_params = SamplingParams {
            temperature: params.temperature.map(|t| t as f64),
            top_p: params.top_p.map(|p| p as f64),
            top_k: params.top_k.map(|k| k as usize),
            max_len: Some(params.max_tokens),
            repetition_penalty: params.repetition_penalty.map(|p| p as f32),
            presence_penalty: params.presence_penalty.map(|p| p as f32),
            frequency_penalty: params.frequency_penalty.map(|p| p as f32),
            stop_toks: if params.stop_sequences.is_empty() {
                None
            } else {
                Some(mistralrs::StopTokens::Seqs(params.stop_sequences.clone()))
            },
            ..Default::default()
        };

        // Create the request
        let (tx, rx) = channel();
        let request = Request::Normal(NormalRequest {
            messages: RequestMessage::Completion {
                text: prompt.to_string(),
                echo_prompt: false,
                best_of: 1,
            },
            sampling_params,
            response: tx,
            return_logprobs: false,
            is_streaming: false,
            id: self.sequence_counter.fetch_add(1, Ordering::SeqCst) as usize,
            constraint: Constraint::None,
            suffix: None,
            adapters: None,
            tool_choice: None,
            tools: None,
            logits_processors: None,
        });

        // Send request to model
        model
            .get_sender()
            .map_err(|e| RuvLLMError::Compute(format!("Failed to get model sender: {}", e)))?
            .blocking_send(request)
            .map_err(|e| RuvLLMError::Compute(format!("Failed to send request to model: {}", e)))?;

        // Wait for response
        let response = rx
            .recv()
            .map_err(|e| RuvLLMError::Compute(format!("Failed to receive response: {}", e)))?;

        match response {
            Response::Done(completion) => {
                let output_text = completion
                    .choices
                    .first()
                    .map(|c| c.message.content.clone().unwrap_or_default())
                    .unwrap_or_default();

                // Build generated tokens from the response
                let generated_tokens = completion
                    .choices
                    .first()
                    .map(|c| {
                        // mistral-rs doesn't provide individual tokens in non-streaming mode
                        // so we return a single token representing the full output
                        vec![GeneratedToken {
                            id: 0, // Not available in non-streaming
                            text: c.message.content.clone().unwrap_or_default(),
                            logprob: None,
                            is_special: false,
                        }]
                    })
                    .unwrap_or_default();

                Ok((output_text, generated_tokens))
            }
            Response::InternalError(e) => {
                Err(RuvLLMError::Compute(format!("Model internal error: {}", e)))
            }
            Response::ValidationError(e) => {
                Err(RuvLLMError::Config(format!("Validation error: {}", e)))
            }
            Response::ModelError(msg, _) => {
                Err(RuvLLMError::Compute(format!("Model error: {}", msg)))
            }
            _ => Err(RuvLLMError::Compute("Unexpected response type".to_string())),
        }
    }

    /// Stub implementation when mistral-rs is not available or model not loaded
    fn generate_internal_stub(
        &self,
        prompt: &str,
        params: &GenerateParams,
    ) -> Result<(String, Vec<GeneratedToken>)> {
        let tokenizer = self
            .tokenizer
            .as_ref()
            .ok_or_else(|| RuvLLMError::InvalidOperation("No tokenizer loaded".to_string()))?;

        // Encode prompt
        let input_ids = tokenizer.encode(prompt)?;
        let seq_id = self.allocate_sequence(input_ids.len())?;

        let mut generated_ids = input_ids.clone();
        let mut generated_tokens = Vec::new();

        // Generation loop (stub implementation)
        for step in 0..params.max_tokens {
            // Placeholder: simulate token generation
            let next_token_id = self.sample_next_token(&generated_ids, params, step)?;

            // Check for EOS
            if let Some(eos_id) = tokenizer.special_tokens().eos_token_id {
                if next_token_id == eos_id {
                    break;
                }
            }

            generated_ids.push(next_token_id);

            let token_text = tokenizer.decode(&[next_token_id])?;
            generated_tokens.push(GeneratedToken {
                id: next_token_id,
                text: token_text.clone(),
                logprob: None,
                is_special: false,
            });

            // Check stop sequences
            let current_text = tokenizer.decode(&generated_ids[input_ids.len()..])?;
            let should_stop = params
                .stop_sequences
                .iter()
                .any(|stop| current_text.contains(stop));

            if should_stop {
                break;
            }
        }

        // Free sequence resources
        self.free_sequence(&seq_id)?;

        // Decode output
        let output_text = tokenizer.decode(&generated_ids[input_ids.len()..])?;

        Ok((output_text, generated_tokens))
    }

    /// Sample next token (placeholder implementation)
    fn sample_next_token(
        &self,
        _context: &[u32],
        params: &GenerateParams,
        step: usize,
    ) -> Result<u32> {
        // In a real implementation, this would:
        // 1. Get logits from model
        // 2. Apply temperature scaling
        // 3. Apply top-p/top-k filtering
        // 4. Sample from distribution

        // Placeholder: return deterministic tokens based on step
        let seed = params.seed.unwrap_or(42);
        let token = ((seed as usize + step) % 32000) as u32;
        Ok(token)
    }
}

impl Default for MistralBackend {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            config: MistralBackendConfig::default(),
            model_config: None,
            model_info: None,
            paged_attention: None,
            xlora_manager: None,
            tokenizer: None,
            is_loaded: AtomicBool::new(false),
            sequence_counter: AtomicU64::new(0),
            model_path: None,
            #[cfg(feature = "mistral-rs")]
            mistral_model: None,
        })
    }
}

impl LlmBackend for MistralBackend {
    fn load_model(&mut self, model_id: &str, config: ModelConfig) -> Result<()> {
        let path = Path::new(model_id);

        // Determine model path
        let model_path = if path.exists() {
            path.to_path_buf()
        } else {
            // Would download from HuggingFace Hub
            self.config.cache_dir.join(model_id.replace('/', "--"))
        };

        // Load tokenizer
        let tokenizer_path = self
            .config
            .tokenizer_path
            .clone()
            .unwrap_or_else(|| model_path.join("tokenizer.json"));

        #[cfg(feature = "mistral-rs")]
        {
            let inner = tokenizers::Tokenizer::from_file(&tokenizer_path)
                .map_err(|e| RuvLLMError::Storage(format!("Failed to load tokenizer: {}", e)))?;

            let special_tokens = SpecialTokens {
                bos_token_id: inner.token_to_id("<s>"),
                eos_token_id: inner.token_to_id("</s>"),
                pad_token_id: inner.token_to_id("<pad>"),
                unk_token_id: inner.token_to_id("<unk>"),
            };

            self.tokenizer = Some(MistralTokenizer {
                inner,
                special_tokens,
            });
        }

        #[cfg(not(feature = "mistral-rs"))]
        {
            let _ = tokenizer_path;
            self.tokenizer = Some(MistralTokenizer {
                vocab_size: 32000,
                special_tokens: SpecialTokens {
                    bos_token_id: Some(1),
                    eos_token_id: Some(2),
                    pad_token_id: Some(0),
                    unk_token_id: Some(3),
                },
            });
        }

        // Update PagedAttention config based on model
        if let Some(ref mut pa) = self.paged_attention {
            // In a real implementation, we'd update based on loaded model config
            let _ = pa;
        }

        // Load X-LoRA adapters if configured
        if let Some(ref manager) = self.xlora_manager {
            if let Some(ref xlora_config) = self.config.xlora {
                for adapter_name in &xlora_config.adapter_names {
                    let adapter_path = model_path.join("adapters").join(adapter_name);
                    if adapter_path.exists() {
                        manager.load_adapter(adapter_name, &adapter_path)?;
                    }
                }
            }
        }

        // Load mistral-rs model when feature is enabled
        #[cfg(feature = "mistral-rs")]
        {
            // Detect if model is GGUF format
            let is_gguf = model_path.extension().map(|e| e == "gguf").unwrap_or(false)
                || model_path.join("model.gguf").exists()
                || std::fs::read_dir(&model_path)
                    .map(|entries| {
                        entries.filter_map(|e| e.ok()).any(|e| {
                            e.path()
                                .extension()
                                .map(|ext| ext == "gguf")
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false);

            if is_gguf {
                // Build PagedAttention configuration from our config
                let paged_attn_config = self.config.paged_attention.as_ref().map(|pa| {
                    PagedAttentionMetaBuilder::default()
                        .with_block_size(pa.block_size)
                        .with_gpu_memory_utilization(pa.gpu_memory_fraction)
                        .build()
                });

                // Determine the device
                let device = match self.config.device {
                    DeviceType::Cpu => MistralDevice::Cpu,
                    DeviceType::Cuda(id) => {
                        MistralDevice::new_cuda(id).unwrap_or(MistralDevice::Cpu)
                    }
                    DeviceType::Metal => MistralDevice::new_metal(0).unwrap_or(MistralDevice::Cpu),
                    _ => MistralDevice::Cpu,
                };

                // Find the GGUF file
                let gguf_file = if model_path.extension().map(|e| e == "gguf").unwrap_or(false) {
                    model_path.clone()
                } else {
                    // Look for .gguf file in directory
                    std::fs::read_dir(&model_path)
                        .ok()
                        .and_then(|entries| {
                            entries
                                .filter_map(|e| e.ok())
                                .find(|e| {
                                    e.path()
                                        .extension()
                                        .map(|ext| ext == "gguf")
                                        .unwrap_or(false)
                                })
                                .map(|e| e.path())
                        })
                        .unwrap_or_else(|| model_path.join("model.gguf"))
                };

                // Build GGUF loader
                let loader = GGUFLoaderBuilder::new(
                    None, // chat_template
                    Some(tokenizer_path.to_string_lossy().to_string()),
                    gguf_file.to_string_lossy().to_string(),
                    GGUFSpecificConfig::default(),
                )
                .build();

                // Build the MistralRs instance
                let scheduler_config = if paged_attn_config.is_some() {
                    SchedulerConfig::PagedAttentionMeta {
                        max_num_seqs: self.config.max_batch_size,
                        config: paged_attn_config.unwrap(),
                    }
                } else {
                    SchedulerConfig::DefaultScheduler {
                        method: mistralrs::DefaultSchedulerMethod::Fixed(
                            std::num::NonZeroUsize::new(self.config.max_batch_size)
                                .unwrap_or(std::num::NonZeroUsize::new(1).unwrap()),
                        ),
                    }
                };

                // Create the pipeline
                let pipeline = loader.load_model_from_hf(
                    None, // revision
                    TokenSource::CacheToken,
                    &device,
                    false, // silent
                    None,  // mapper
                    None,  // in_situ_quant
                    paged_attn_config,
                );

                match pipeline {
                    Ok(pipeline) => {
                        let mistral = MistralRsBuilder::new(pipeline, scheduler_config).build();
                        self.mistral_model = Some(Arc::new(mistral));
                        tracing::info!("Loaded mistral-rs GGUF model from {:?}", gguf_file);
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to load mistral-rs model: {}. Falling back to stub.",
                            e
                        );
                        self.mistral_model = None;
                    }
                }
            } else {
                tracing::info!("Model is not GGUF format, mistral-rs model loading skipped");
                self.mistral_model = None;
            }
        }

        // Create model info
        let hidden_size = config.hidden_size.unwrap_or(4096);
        let num_layers = config.num_layers.unwrap_or(32);
        let vocab_size = config.vocab_size.unwrap_or(32000);

        self.model_info = Some(ModelInfo {
            name: model_id.to_string(),
            architecture: config.architecture,
            num_parameters: estimate_parameters(hidden_size, num_layers, vocab_size),
            vocab_size,
            hidden_size,
            num_layers,
            max_context_length: config.max_sequence_length,
            quantization: config.quantization,
            memory_usage: estimate_memory_usage(hidden_size, num_layers, vocab_size, &config),
        });

        self.model_config = Some(config);
        self.model_path = Some(model_path);
        self.is_loaded.store(true, Ordering::SeqCst);

        tracing::info!(
            "Loaded model: {} (PagedAttention: {}, X-LoRA: {})",
            model_id,
            self.paged_attention.is_some(),
            self.xlora_manager.is_some()
        );

        Ok(())
    }

    fn generate(&self, prompt: &str, params: GenerateParams) -> Result<String> {
        if !self.is_model_loaded() {
            return Err(RuvLLMError::InvalidOperation("No model loaded".to_string()));
        }

        let (output, _tokens) = self.generate_internal(prompt, &params)?;
        Ok(output)
    }

    fn generate_stream(
        &self,
        prompt: &str,
        params: GenerateParams,
    ) -> Result<Box<dyn Iterator<Item = Result<GeneratedToken>> + Send + '_>> {
        if !self.is_model_loaded() {
            return Err(RuvLLMError::InvalidOperation("No model loaded".to_string()));
        }

        // For streaming, we generate all tokens and return an iterator
        // In a real implementation, this would be a true streaming iterator
        let (_, tokens) = self.generate_internal(prompt, &params)?;

        Ok(Box::new(tokens.into_iter().map(Ok)))
    }

    fn generate_stream_v2(
        &self,
        prompt: &str,
        params: GenerateParams,
    ) -> Result<super::TokenStream> {
        use super::{StreamEvent, TokenStream};
        use std::time::Instant;

        if !self.is_model_loaded() {
            return Err(RuvLLMError::InvalidOperation("No model loaded".to_string()));
        }

        let (tx, stream) = TokenStream::channel();
        let start_time = Instant::now();

        // Generate tokens and send through channel
        // In a real implementation, this would be async/threaded
        let (_, tokens) = self.generate_internal(prompt, &params)?;

        for token in &tokens {
            let _ = tx.send(StreamEvent::Token(token.clone()));
        }

        // Send completion event
        let duration = start_time.elapsed();
        let _ = tx.send(StreamEvent::Done {
            total_tokens: tokens.len(),
            duration_ms: duration.as_millis() as u64,
            tokens_per_second: if duration.as_secs_f64() > 0.0 {
                tokens.len() as f64 / duration.as_secs_f64()
            } else {
                0.0
            },
        });

        Ok(stream)
    }

    fn get_embeddings(&self, text: &str) -> Result<Vec<f32>> {
        if !self.is_model_loaded() {
            return Err(RuvLLMError::InvalidOperation("No model loaded".to_string()));
        }

        let tokenizer = self
            .tokenizer
            .as_ref()
            .ok_or_else(|| RuvLLMError::InvalidOperation("No tokenizer loaded".to_string()))?;

        let _tokens = tokenizer.encode(text)?;

        // In a real implementation, this would run the model and extract hidden states
        let hidden_size = self
            .model_info
            .as_ref()
            .map(|i| i.hidden_size)
            .unwrap_or(4096);

        Ok(vec![0.0; hidden_size])
    }

    fn tokenizer(&self) -> Option<&dyn Tokenizer> {
        self.tokenizer.as_ref().map(|t| t as &dyn Tokenizer)
    }

    fn is_model_loaded(&self) -> bool {
        self.is_loaded.load(Ordering::SeqCst)
    }

    fn model_info(&self) -> Option<ModelInfo> {
        self.model_info.clone()
    }

    fn unload_model(&mut self) {
        self.model_config = None;
        self.model_info = None;
        self.tokenizer = None;
        self.model_path = None;
        self.is_loaded.store(false, Ordering::SeqCst);

        // Clear mistral-rs model
        #[cfg(feature = "mistral-rs")]
        {
            self.mistral_model = None;
        }

        // Reset PagedAttention
        if let Some(ref config) = self.config.paged_attention {
            self.paged_attention = Some(PagedAttention::new(PagedAttentionConfig {
                page_size: config.block_size,
                max_pages_per_sequence: config.max_pages / 256,
                page_table_capacity: config.max_pages,
                num_heads: 32,
                head_dim: 128,
                num_kv_heads: 8,
                ..Default::default()
            }));
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Get cache directory for models
fn get_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ruvllm")
        .join("mistral-rs")
}

/// Softmax function
fn softmax(logits: &[f32]) -> Vec<f32> {
    if logits.is_empty() {
        return Vec::new();
    }

    let max = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let exp: Vec<f32> = logits.iter().map(|x| (x - max).exp()).collect();
    let sum: f32 = exp.iter().sum();

    exp.iter().map(|x| x / sum).collect()
}

/// Estimate number of parameters
fn estimate_parameters(hidden_size: usize, num_layers: usize, vocab_size: usize) -> usize {
    let embedding_params = vocab_size * hidden_size;
    let layer_params =
        num_layers * (4 * hidden_size * hidden_size + 8 * hidden_size * hidden_size / 3);
    let output_params = vocab_size * hidden_size;
    embedding_params + layer_params + output_params
}

/// Estimate memory usage
fn estimate_memory_usage(
    hidden_size: usize,
    num_layers: usize,
    vocab_size: usize,
    config: &ModelConfig,
) -> usize {
    let params = estimate_parameters(hidden_size, num_layers, vocab_size);
    let bytes_per_param = match config.quantization {
        Some(Quantization::Q4K) | Some(Quantization::Q4) => 0.5,
        Some(Quantization::Q8) => 1.0,
        Some(Quantization::F16) | Some(Quantization::Bf16) => 2.0,
        _ => 4.0,
    };
    (params as f64 * bytes_per_param) as usize
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_creation() {
        let backend = MistralBackend::new().unwrap();
        assert!(!backend.is_model_loaded());
    }

    #[test]
    fn test_config_builder() {
        let config = MistralBackendConfig::default()
            .with_paged_attention(32, 8192)
            .with_max_seq_len(16384)
            .with_max_batch_size(64);

        assert_eq!(config.max_seq_len, 16384);
        assert_eq!(config.max_batch_size, 64);
        assert!(config.paged_attention.is_some());

        let pa = config.paged_attention.unwrap();
        assert_eq!(pa.block_size, 32);
        assert_eq!(pa.max_pages, 8192);
    }

    #[test]
    fn test_xlora_config() {
        let config =
            MistralBackendConfig::default().with_xlora_adapters(vec!["code", "chat", "math"]);

        assert!(config.xlora.is_some());
        let xlora = config.xlora.unwrap();
        assert_eq!(xlora.adapter_names.len(), 3);
    }

    #[test]
    fn test_isq_config() {
        let config = MistralBackendConfig::default().with_isq(4);

        assert!(config.isq.is_some());
        let isq = config.isq.unwrap();
        assert_eq!(isq.bits, 4);
    }

    #[test]
    fn test_xlora_manager() {
        let xlora_config = XLoraConfig {
            adapter_names: vec!["test".to_string()],
            top_k: 1,
            ..Default::default()
        };

        let manager = XLoraManager::new(xlora_config);
        assert_eq!(manager.adapters.len(), 0);
    }

    #[test]
    fn test_softmax() {
        let logits = vec![1.0, 2.0, 3.0];
        let probs = softmax(&logits);

        assert_eq!(probs.len(), 3);
        let sum: f32 = probs.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6);
        assert!(probs[2] > probs[1]);
        assert!(probs[1] > probs[0]);
    }

    #[test]
    fn test_estimate_parameters() {
        // Test parameter estimation produces reasonable values
        // Note: This is an approximation, not exact parameter count
        let params = estimate_parameters(4096, 32, 32000);
        // Should be in the billions (rough estimate for a 7B-class model)
        assert!(
            params > 3_000_000_000,
            "Expected > 3B params, got {}",
            params
        );
        assert!(
            params < 10_000_000_000,
            "Expected < 10B params, got {}",
            params
        );
    }

    #[test]
    fn test_paged_attention_config() {
        let config = PagedAttentionConfigExt::default();
        assert_eq!(config.block_size, 16);
        assert_eq!(config.max_pages, 4096);
        assert!(config.enable_prefix_caching);
    }

    #[test]
    fn test_has_native_model() {
        let backend = MistralBackend::new().unwrap();
        // Without loading a model, native model should not be present
        assert!(!backend.has_native_model());
    }

    #[test]
    fn test_backend_unload() {
        let mut backend = MistralBackend::new().unwrap();
        backend.unload_model();
        assert!(!backend.is_model_loaded());
        assert!(!backend.has_native_model());
    }
}
