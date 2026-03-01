//! GGUF Model Loader - Wires parsed GGUF data to model weight initialization
//!
//! This module provides the bridge between GGUF file parsing and actual model
//! weight initialization. It handles:
//!
//! - Architecture detection and configuration extraction
//! - Tensor name mapping for different model architectures
//! - Memory-mapped loading for large models
//! - Progress callbacks for monitoring load status
//! - Quantized weight handling (Q4_0, Q4_K, Q8_0, etc.)
//!
//! ## Supported Architectures
//!
//! | Architecture | Tensor Prefix | Notes |
//! |--------------|---------------|-------|
//! | Llama | `model.layers.` | Llama 1/2/3, CodeLlama |
//! | Mistral | `model.layers.` | Mistral 7B, Codestral |
//! | Phi | `transformer.h.` | Phi-1, Phi-2 |
//! | Phi3 | `model.layers.` | Phi-3 |
//! | Gemma | `model.layers.` | Gemma, Gemma-2 |
//! | Qwen | `transformer.h.` | Qwen, Qwen2 |
//!
//! ## Example
//!
//! ```rust,ignore
//! use ruvllm::gguf::{GgufLoader, LoadProgress, LoadConfig};
//! use std::path::Path;
//!
//! let config = LoadConfig::default()
//!     .with_mmap(true)
//!     .with_progress(|progress| {
//!         println!("Loading: {}%", progress.percent());
//!     });
//!
//! let loader = GgufLoader::new(Path::new("model.gguf"), config)?;
//! let weights = loader.load_weights()?;
//!
//! // Access loaded weights
//! let embed_tokens = weights.get("embed_tokens")?;
//! let layer0_q = weights.get_layer(0, "self_attn.q_proj")?;
//! ```

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use super::{GgufFile, GgufQuantType, ModelConfig as GgufConfig, QuantizedTensor, TensorInfo};
use crate::backends::ModelArchitecture;
use crate::error::{Result, RuvLLMError};

// ============================================================================
// Progress Tracking
// ============================================================================

/// Progress information during model loading.
#[derive(Debug, Clone)]
pub struct LoadProgress {
    /// Total number of tensors to load
    pub total_tensors: usize,
    /// Number of tensors loaded so far
    pub loaded_tensors: usize,
    /// Total bytes to load
    pub total_bytes: usize,
    /// Bytes loaded so far
    pub loaded_bytes: usize,
    /// Current tensor being loaded
    pub current_tensor: Option<String>,
    /// Current layer being loaded (if applicable)
    pub current_layer: Option<usize>,
    /// Estimated time remaining in seconds
    pub eta_seconds: Option<f64>,
}

impl LoadProgress {
    /// Get loading progress as a percentage (0-100).
    pub fn percent(&self) -> f32 {
        if self.total_tensors == 0 {
            return 100.0;
        }
        (self.loaded_tensors as f32 / self.total_tensors as f32) * 100.0
    }

    /// Get byte loading progress as a percentage (0-100).
    pub fn byte_percent(&self) -> f32 {
        if self.total_bytes == 0 {
            return 100.0;
        }
        (self.loaded_bytes as f32 / self.total_bytes as f32) * 100.0
    }

    /// Check if loading is complete.
    pub fn is_complete(&self) -> bool {
        self.loaded_tensors >= self.total_tensors
    }
}

/// Progress callback type.
pub type ProgressCallback = Box<dyn Fn(&LoadProgress) + Send + Sync>;

// ============================================================================
// Load Configuration
// ============================================================================

/// Configuration for GGUF model loading.
#[derive(Default)]
pub struct LoadConfig {
    /// Use memory mapping for efficient loading (recommended for large models)
    pub use_mmap: bool,
    /// Keep weights in quantized format (don't dequantize to F32)
    pub keep_quantized: bool,
    /// Only load specific tensors (empty = load all)
    pub tensor_filter: Vec<String>,
    /// Only load specific layers (empty = load all)
    pub layer_filter: Vec<usize>,
    /// Progress callback
    pub progress_callback: Option<ProgressCallback>,
    /// Number of threads for parallel loading (0 = auto)
    pub num_threads: usize,
    /// Prefetch tensor data during parsing
    pub prefetch: bool,
}

impl LoadConfig {
    /// Enable memory mapping.
    pub fn with_mmap(mut self, enabled: bool) -> Self {
        self.use_mmap = enabled;
        self
    }

    /// Keep weights in quantized format.
    pub fn with_quantized(mut self, keep: bool) -> Self {
        self.keep_quantized = keep;
        self
    }

    /// Set progress callback.
    pub fn with_progress<F>(mut self, callback: F) -> Self
    where
        F: Fn(&LoadProgress) + Send + Sync + 'static,
    {
        self.progress_callback = Some(Box::new(callback));
        self
    }

    /// Filter to specific tensors.
    pub fn with_tensor_filter(mut self, tensors: Vec<String>) -> Self {
        self.tensor_filter = tensors;
        self
    }

    /// Filter to specific layers.
    pub fn with_layer_filter(mut self, layers: Vec<usize>) -> Self {
        self.layer_filter = layers;
        self
    }

    /// Set number of loading threads.
    pub fn with_threads(mut self, threads: usize) -> Self {
        self.num_threads = threads;
        self
    }
}

// ============================================================================
// Loaded Weights Container
// ============================================================================

/// Container for loaded model weights.
///
/// Provides convenient access to loaded weights organized by layer and type.
#[derive(Default)]
pub struct LoadedWeights {
    /// Raw tensor data (quantized or F32 depending on config)
    tensors: HashMap<String, LoadedTensor>,
    /// Model configuration extracted from GGUF
    config: GgufConfig,
    /// Architecture detected from GGUF
    architecture: Option<ModelArchitecture>,
    /// Number of layers
    num_layers: usize,
    /// Total memory used in bytes
    memory_bytes: usize,
}

/// A single loaded tensor.
#[derive(Clone)]
pub struct LoadedTensor {
    /// Tensor name (normalized)
    pub name: String,
    /// Original GGUF tensor name
    pub original_name: String,
    /// Data as F32 (if dequantized)
    pub data_f32: Option<Vec<f32>>,
    /// Data as quantized tensor (if kept quantized)
    pub data_quantized: Option<QuantizedTensor>,
    /// Tensor shape
    pub shape: Vec<usize>,
    /// Original quantization type
    pub quant_type: GgufQuantType,
    /// Layer index (if applicable)
    pub layer_index: Option<usize>,
    /// Tensor category
    pub category: TensorCategory,
}

/// Categories of tensors in a transformer model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TensorCategory {
    /// Token embedding layer
    Embedding,
    /// Query projection (Wq)
    AttentionQuery,
    /// Key projection (Wk)
    AttentionKey,
    /// Value projection (Wv)
    AttentionValue,
    /// Output projection (Wo)
    AttentionOutput,
    /// Attention normalization (pre-attention RMSNorm)
    AttentionNorm,
    /// Feed-forward gate projection (w1 / gate_proj)
    FfnGate,
    /// Feed-forward up projection (w3 / up_proj)
    FfnUp,
    /// Feed-forward down projection (w2 / down_proj)
    FfnDown,
    /// FFN normalization (post-attention RMSNorm)
    FfnNorm,
    /// Final layer normalization
    FinalNorm,
    /// Output/LM head projection
    OutputHead,
    /// Other/unknown
    Other,
}

impl LoadedWeights {
    /// Get a tensor by normalized name.
    pub fn get(&self, name: &str) -> Option<&LoadedTensor> {
        self.tensors.get(name)
    }

    /// Get a layer-specific tensor.
    ///
    /// # Arguments
    ///
    /// * `layer` - Layer index (0-based)
    /// * `component` - Component name (e.g., "self_attn.q_proj", "mlp.gate_proj")
    pub fn get_layer(&self, layer: usize, component: &str) -> Option<&LoadedTensor> {
        let key = format!("layers.{}.{}", layer, component);
        self.tensors.get(&key)
    }

    /// Get all tensors for a specific layer.
    pub fn get_layer_tensors(&self, layer: usize) -> Vec<&LoadedTensor> {
        let prefix = format!("layers.{}.", layer);
        self.tensors
            .values()
            .filter(|t| t.name.starts_with(&prefix))
            .collect()
    }

    /// Get tensors by category.
    pub fn get_by_category(&self, category: TensorCategory) -> Vec<&LoadedTensor> {
        self.tensors
            .values()
            .filter(|t| t.category == category)
            .collect()
    }

    /// Get the model configuration.
    pub fn config(&self) -> &GgufConfig {
        &self.config
    }

    /// Get detected architecture.
    pub fn architecture(&self) -> Option<ModelArchitecture> {
        self.architecture
    }

    /// Get number of layers.
    pub fn num_layers(&self) -> usize {
        self.num_layers
    }

    /// Get total memory usage in bytes.
    pub fn memory_bytes(&self) -> usize {
        self.memory_bytes
    }

    /// Get all tensor names.
    pub fn tensor_names(&self) -> impl Iterator<Item = &str> {
        self.tensors.keys().map(|s| s.as_str())
    }

    /// Get tensor count.
    pub fn tensor_count(&self) -> usize {
        self.tensors.len()
    }
}

// ============================================================================
// Tensor Name Mapping
// ============================================================================

/// Maps GGUF tensor names to normalized internal names.
///
/// Different model architectures use different naming conventions.
/// This mapper normalizes them to a consistent format.
pub struct TensorNameMapper {
    architecture: ModelArchitecture,
}

impl TensorNameMapper {
    /// Create a new mapper for the given architecture.
    pub fn new(architecture: ModelArchitecture) -> Self {
        Self { architecture }
    }

    /// Map a GGUF tensor name to normalized form.
    ///
    /// Returns (normalized_name, layer_index, category)
    pub fn map(&self, gguf_name: &str) -> (String, Option<usize>, TensorCategory) {
        let layer = self.extract_layer_index(gguf_name);
        let category = self.categorize(gguf_name);
        let normalized = self.normalize_name(gguf_name);

        (normalized, layer, category)
    }

    /// Extract layer index from tensor name.
    fn extract_layer_index(&self, name: &str) -> Option<usize> {
        // Common patterns: "model.layers.N.", "transformer.h.N.", "blocks.N."
        for pattern in &["layers.", "h.", "blocks.", "block."] {
            if let Some(pos) = name.find(pattern) {
                let after = &name[pos + pattern.len()..];
                if let Some(end) = after.find('.') {
                    if let Ok(idx) = after[..end].parse() {
                        return Some(idx);
                    }
                }
            }
        }
        None
    }

    /// Categorize tensor by name.
    fn categorize(&self, name: &str) -> TensorCategory {
        let lower = name.to_lowercase();

        // Embedding
        if lower.contains("embed") || lower.contains("token") && lower.contains("weight") {
            if lower.contains("output") || lower.contains("lm_head") {
                return TensorCategory::OutputHead;
            }
            return TensorCategory::Embedding;
        }

        // Output head
        if lower.contains("lm_head") || (lower.contains("output") && !lower.contains("attn")) {
            return TensorCategory::OutputHead;
        }

        // Attention
        if lower.contains("attn") || lower.contains("attention") {
            if lower.contains("q_proj") || lower.contains(".wq.") || lower.contains("query") {
                return TensorCategory::AttentionQuery;
            }
            if lower.contains("k_proj") || lower.contains(".wk.") || lower.contains("key") {
                return TensorCategory::AttentionKey;
            }
            if lower.contains("v_proj") || lower.contains(".wv.") || lower.contains("value") {
                return TensorCategory::AttentionValue;
            }
            if lower.contains("o_proj") || lower.contains(".wo.") || lower.contains("out_proj") {
                return TensorCategory::AttentionOutput;
            }
        }

        // Feed-forward / MLP
        if lower.contains("mlp") || lower.contains("ffn") || lower.contains("feed_forward") {
            if lower.contains("gate") || lower.contains(".w1.") {
                return TensorCategory::FfnGate;
            }
            if lower.contains("up") || lower.contains(".w3.") {
                return TensorCategory::FfnUp;
            }
            if lower.contains("down") || lower.contains(".w2.") {
                return TensorCategory::FfnDown;
            }
        }

        // Normalization
        if lower.contains("norm") || lower.contains("ln_") || lower.contains("layer_norm") {
            if lower.contains("final") || lower.contains("model.norm") || !lower.contains("layers")
            {
                return TensorCategory::FinalNorm;
            }
            if lower.contains("input") || lower.contains("attn") || lower.contains("attention") {
                return TensorCategory::AttentionNorm;
            }
            if lower.contains("post") || lower.contains("ffn") || lower.contains("mlp") {
                return TensorCategory::FfnNorm;
            }
            // Default layer norm is usually attention norm
            if self.extract_layer_index(&lower).is_some() {
                return TensorCategory::AttentionNorm;
            }
            return TensorCategory::FinalNorm;
        }

        TensorCategory::Other
    }

    /// Normalize tensor name to internal format.
    fn normalize_name(&self, name: &str) -> String {
        // Remove common prefixes
        let name = name
            .strip_prefix("model.")
            .unwrap_or(name)
            .strip_prefix("transformer.")
            .unwrap_or(name);

        // Normalize layer patterns
        let name = name
            .replace("h.", "layers.")
            .replace("blocks.", "layers.")
            .replace("block.", "layers.");

        // Normalize attention patterns
        let name = name
            .replace("self_attn.", "attention.")
            .replace("self_attention.", "attention.");

        // Normalize MLP patterns
        let name = name
            .replace("feed_forward.", "mlp.")
            .replace("ffn.", "mlp.");

        name.to_string()
    }
}

// ============================================================================
// GGUF Model Loader
// ============================================================================

/// GGUF Model Loader
///
/// Loads GGUF model files and maps them to model weights.
pub struct GgufLoader {
    /// Parsed GGUF file
    file: GgufFile,
    /// Load configuration
    config: LoadConfig,
    /// Tensor name mapper
    mapper: Option<TensorNameMapper>,
    /// Progress tracking
    loaded_count: AtomicUsize,
    loaded_bytes: AtomicUsize,
}

impl GgufLoader {
    /// Create a new GGUF loader.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the GGUF file
    /// * `config` - Load configuration
    pub fn new(path: &Path, config: LoadConfig) -> Result<Self> {
        let file = if config.use_mmap {
            GgufFile::open_mmap(path)?
        } else {
            GgufFile::open(path)?
        };

        let architecture = file.architecture_type();
        let mapper = architecture.map(TensorNameMapper::new);

        Ok(Self {
            file,
            config,
            mapper,
            loaded_count: AtomicUsize::new(0),
            loaded_bytes: AtomicUsize::new(0),
        })
    }

    /// Get the detected model architecture.
    pub fn architecture(&self) -> Option<ModelArchitecture> {
        self.file.architecture_type()
    }

    /// Get the model configuration extracted from GGUF.
    pub fn model_config(&self) -> GgufConfig {
        GgufConfig {
            architecture: self.file.architecture().map(|s| s.to_string()),
            context_length: self.file.context_length(),
            embedding_length: self.file.embedding_length(),
            head_count: self.file.head_count(),
            head_count_kv: self.file.head_count_kv(),
            layer_count: self.file.layer_count(),
            vocab_size: self.file.vocab_size(),
            rope_freq_base: self.file.rope_freq_base(),
            feed_forward_length: self.file.feed_forward_length(),
        }
    }

    /// Get tensor information for inspection.
    pub fn tensor_infos(&self) -> &[TensorInfo] {
        &self.file.tensors
    }

    /// Load all weights from the GGUF file.
    pub fn load_weights(&self) -> Result<LoadedWeights> {
        let total_tensors = self.file.tensors.len();
        let total_bytes: usize = self.file.tensors.iter().map(|t| t.byte_size()).sum();

        let mapper = self.mapper.as_ref().ok_or_else(|| {
            RuvLLMError::Model("Unknown architecture, cannot map tensor names".to_string())
        })?;

        let mut weights = LoadedWeights {
            config: self.model_config(),
            architecture: self.architecture(),
            num_layers: self.file.layer_count().unwrap_or(0),
            ..Default::default()
        };

        // Load each tensor
        for tensor_info in &self.file.tensors {
            // Apply filters
            if !self.should_load_tensor(tensor_info) {
                continue;
            }

            // Map tensor name
            let (normalized_name, layer_index, category) = mapper.map(&tensor_info.name);

            // Load tensor data
            let loaded =
                self.load_single_tensor(tensor_info, &normalized_name, layer_index, category)?;

            // Update memory tracking
            let tensor_bytes = loaded.data_f32.as_ref().map(|d| d.len() * 4).unwrap_or(0)
                + loaded
                    .data_quantized
                    .as_ref()
                    .map(|q| q.data.len())
                    .unwrap_or(0);
            weights.memory_bytes += tensor_bytes;

            // Store tensor
            weights.tensors.insert(normalized_name.clone(), loaded);

            // Update progress
            let count = self.loaded_count.fetch_add(1, Ordering::Relaxed) + 1;
            let bytes = self
                .loaded_bytes
                .fetch_add(tensor_info.byte_size(), Ordering::Relaxed)
                + tensor_info.byte_size();

            if let Some(ref callback) = self.config.progress_callback {
                let progress = LoadProgress {
                    total_tensors,
                    loaded_tensors: count,
                    total_bytes,
                    loaded_bytes: bytes,
                    current_tensor: Some(tensor_info.name.clone()),
                    current_layer: layer_index,
                    eta_seconds: None, // Could calculate based on rate
                };
                callback(&progress);
            }
        }

        // Send final progress
        if let Some(ref callback) = self.config.progress_callback {
            let progress = LoadProgress {
                total_tensors,
                loaded_tensors: total_tensors,
                total_bytes,
                loaded_bytes: total_bytes,
                current_tensor: None,
                current_layer: None,
                eta_seconds: Some(0.0),
            };
            callback(&progress);
        }

        Ok(weights)
    }

    /// Load weights for a specific layer only.
    pub fn load_layer(&self, layer_index: usize) -> Result<Vec<LoadedTensor>> {
        let mapper = self.mapper.as_ref().ok_or_else(|| {
            RuvLLMError::Model("Unknown architecture, cannot map tensor names".to_string())
        })?;

        let mut tensors = Vec::new();

        for tensor_info in &self.file.tensors {
            // Check if this tensor belongs to the requested layer
            if let Some(idx) = mapper.map(&tensor_info.name).1 {
                if idx != layer_index {
                    continue;
                }
            } else {
                continue;
            }

            let (normalized_name, layer_idx, category) = mapper.map(&tensor_info.name);
            let loaded =
                self.load_single_tensor(tensor_info, &normalized_name, layer_idx, category)?;
            tensors.push(loaded);
        }

        Ok(tensors)
    }

    /// Load a single tensor by name.
    pub fn load_tensor(&self, name: &str) -> Result<LoadedTensor> {
        let tensor_info = self
            .file
            .get_tensor(name)
            .ok_or_else(|| RuvLLMError::NotFound(format!("Tensor not found: {}", name)))?;

        let mapper = self.mapper.as_ref();
        let (normalized_name, layer_idx, category) = mapper
            .map(|m| m.map(&tensor_info.name))
            .unwrap_or_else(|| (name.to_string(), None, TensorCategory::Other));

        self.load_single_tensor(tensor_info, &normalized_name, layer_idx, category)
    }

    /// Internal: Load a single tensor.
    fn load_single_tensor(
        &self,
        info: &TensorInfo,
        normalized_name: &str,
        layer_index: Option<usize>,
        category: TensorCategory,
    ) -> Result<LoadedTensor> {
        let (data_f32, data_quantized) = if self.config.keep_quantized && info.dtype.is_quantized()
        {
            // Keep as quantized
            let quantized = self.file.load_tensor_quantized(&info.name)?;
            (None, Some(quantized))
        } else {
            // Dequantize to F32
            let f32_data = self.file.load_tensor_f32(&info.name)?;
            (Some(f32_data), None)
        };

        Ok(LoadedTensor {
            name: normalized_name.to_string(),
            original_name: info.name.clone(),
            data_f32,
            data_quantized,
            shape: info.shape.clone(),
            quant_type: info.dtype,
            layer_index,
            category,
        })
    }

    /// Check if a tensor should be loaded based on filters.
    fn should_load_tensor(&self, info: &TensorInfo) -> bool {
        // Check tensor filter
        if !self.config.tensor_filter.is_empty() {
            let matches = self
                .config
                .tensor_filter
                .iter()
                .any(|pattern| info.name.contains(pattern));
            if !matches {
                return false;
            }
        }

        // Check layer filter
        if !self.config.layer_filter.is_empty() {
            if let Some(ref mapper) = self.mapper {
                if let Some(layer) = mapper.map(&info.name).1 {
                    if !self.config.layer_filter.contains(&layer) {
                        return false;
                    }
                }
                // Non-layer tensors (embed, norm) are always loaded if layer filter is set
            }
        }

        true
    }
}

// ============================================================================
// Streaming Layer Loader
// ============================================================================

/// Streaming layer loader for memory-efficient loading of large models.
///
/// Instead of loading all weights at once, this loader loads one layer at a time,
/// allowing models larger than available RAM to be processed.
pub struct StreamingLoader {
    loader: GgufLoader,
    current_layer: usize,
    total_layers: usize,
}

impl StreamingLoader {
    /// Create a new streaming loader.
    pub fn new(path: &Path, config: LoadConfig) -> Result<Self> {
        let loader = GgufLoader::new(path, config)?;
        let total_layers = loader.model_config().layer_count.unwrap_or(0);

        Ok(Self {
            loader,
            current_layer: 0,
            total_layers,
        })
    }

    /// Get model configuration.
    pub fn model_config(&self) -> GgufConfig {
        self.loader.model_config()
    }

    /// Get total number of layers.
    pub fn total_layers(&self) -> usize {
        self.total_layers
    }

    /// Get current layer index.
    pub fn current_layer(&self) -> usize {
        self.current_layer
    }

    /// Check if there are more layers to load.
    pub fn has_more_layers(&self) -> bool {
        self.current_layer < self.total_layers
    }

    /// Load embedding and pre-layer normalization tensors.
    pub fn load_embeddings(&self) -> Result<Vec<LoadedTensor>> {
        let mapper = self
            .loader
            .mapper
            .as_ref()
            .ok_or_else(|| RuvLLMError::Model("Unknown architecture".to_string()))?;

        let mut tensors = Vec::new();

        for tensor_info in &self.loader.file.tensors {
            let (_, layer_idx, category) = mapper.map(&tensor_info.name);

            // Skip layer tensors
            if layer_idx.is_some() {
                continue;
            }

            // Load embedding and initial norm tensors
            if matches!(category, TensorCategory::Embedding) {
                let loaded = self.loader.load_tensor(&tensor_info.name)?;
                tensors.push(loaded);
            }
        }

        Ok(tensors)
    }

    /// Load the next layer's tensors.
    pub fn load_next_layer(&mut self) -> Result<Option<Vec<LoadedTensor>>> {
        if self.current_layer >= self.total_layers {
            return Ok(None);
        }

        let tensors = self.loader.load_layer(self.current_layer)?;
        self.current_layer += 1;

        Ok(Some(tensors))
    }

    /// Load final normalization and output head tensors.
    pub fn load_output_head(&self) -> Result<Vec<LoadedTensor>> {
        let mapper = self
            .loader
            .mapper
            .as_ref()
            .ok_or_else(|| RuvLLMError::Model("Unknown architecture".to_string()))?;

        let mut tensors = Vec::new();

        for tensor_info in &self.loader.file.tensors {
            let (_, layer_idx, category) = mapper.map(&tensor_info.name);

            // Skip layer tensors
            if layer_idx.is_some() {
                continue;
            }

            // Load output head and final norm
            if matches!(
                category,
                TensorCategory::OutputHead | TensorCategory::FinalNorm
            ) {
                let loaded = self.loader.load_tensor(&tensor_info.name)?;
                tensors.push(loaded);
            }
        }

        Ok(tensors)
    }

    /// Reset to beginning for another pass.
    pub fn reset(&mut self) {
        self.current_layer = 0;
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tensor_name_mapper_llama() {
        let mapper = TensorNameMapper::new(ModelArchitecture::Llama);

        // Test layer extraction
        let (name, layer, cat) = mapper.map("model.layers.5.self_attn.q_proj.weight");
        assert_eq!(layer, Some(5));
        assert_eq!(cat, TensorCategory::AttentionQuery);
        assert!(name.contains("layers.5"));

        // Test embedding
        let (_, layer, cat) = mapper.map("model.embed_tokens.weight");
        assert_eq!(layer, None);
        assert_eq!(cat, TensorCategory::Embedding);

        // Test MLP
        let (_, layer, cat) = mapper.map("model.layers.0.mlp.gate_proj.weight");
        assert_eq!(layer, Some(0));
        assert_eq!(cat, TensorCategory::FfnGate);
    }

    #[test]
    fn test_tensor_name_mapper_phi() {
        let mapper = TensorNameMapper::new(ModelArchitecture::Phi);

        // Phi uses transformer.h.N pattern
        let (_, layer, _) = mapper.map("transformer.h.3.attn.q_proj.weight");
        assert_eq!(layer, Some(3));
    }

    #[test]
    fn test_tensor_categorization() {
        let mapper = TensorNameMapper::new(ModelArchitecture::Llama);

        // Attention components
        assert_eq!(
            mapper.categorize("self_attn.q_proj"),
            TensorCategory::AttentionQuery
        );
        assert_eq!(
            mapper.categorize("attention.k_proj"),
            TensorCategory::AttentionKey
        );
        assert_eq!(
            mapper.categorize("self_attn.v_proj"),
            TensorCategory::AttentionValue
        );
        assert_eq!(
            mapper.categorize("attn.o_proj"),
            TensorCategory::AttentionOutput
        );

        // MLP components
        assert_eq!(mapper.categorize("mlp.gate_proj"), TensorCategory::FfnGate);
        assert_eq!(mapper.categorize("mlp.up_proj"), TensorCategory::FfnUp);
        assert_eq!(mapper.categorize("mlp.down_proj"), TensorCategory::FfnDown);

        // Normalization
        assert_eq!(
            mapper.categorize("model.norm.weight"),
            TensorCategory::FinalNorm
        );

        // Output
        assert_eq!(
            mapper.categorize("lm_head.weight"),
            TensorCategory::OutputHead
        );
    }

    #[test]
    fn test_load_progress_percent() {
        let progress = LoadProgress {
            total_tensors: 100,
            loaded_tensors: 25,
            total_bytes: 1000,
            loaded_bytes: 250,
            current_tensor: None,
            current_layer: None,
            eta_seconds: None,
        };

        assert!((progress.percent() - 25.0).abs() < 0.001);
        assert!((progress.byte_percent() - 25.0).abs() < 0.001);
        assert!(!progress.is_complete());

        let complete = LoadProgress {
            total_tensors: 100,
            loaded_tensors: 100,
            total_bytes: 1000,
            loaded_bytes: 1000,
            current_tensor: None,
            current_layer: None,
            eta_seconds: None,
        };

        assert!(complete.is_complete());
    }

    #[test]
    fn test_load_config_builder() {
        let config = LoadConfig::default()
            .with_mmap(true)
            .with_quantized(true)
            .with_threads(4)
            .with_layer_filter(vec![0, 1, 2]);

        assert!(config.use_mmap);
        assert!(config.keep_quantized);
        assert_eq!(config.num_threads, 4);
        assert_eq!(config.layer_filter, vec![0, 1, 2]);
    }
}
