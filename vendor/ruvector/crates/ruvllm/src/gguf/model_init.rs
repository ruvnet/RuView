//! Model Weight Initialization from GGUF
//!
//! This module provides the actual wiring from GGUF tensors to model layer weights
//! for inference. It handles:
//!
//! - Architecture-specific weight mapping (Llama, Mistral, Phi, Gemma, Qwen)
//! - Quantized weight handling for efficient inference
//! - Layer-by-layer weight initialization
//! - Integration with the serving engine
//!
//! ## Supported Architectures
//!
//! | Architecture | Status | Notes |
//! |--------------|--------|-------|
//! | Llama | Full | Llama 1/2/3, CodeLlama |
//! | Mistral | Full | Mistral 7B, Codestral |
//! | Phi | Full | Phi-1, Phi-2, Phi-3 |
//! | Gemma | Full | Gemma, Gemma-2 |
//! | Qwen | Full | Qwen, Qwen2 |
//!
//! ## Example
//!
//! ```rust,ignore
//! use ruvllm::gguf::{GgufLoader, LoadConfig, ModelInitializer};
//! use std::path::Path;
//!
//! // Load GGUF file
//! let loader = GgufLoader::new(Path::new("model.gguf"), LoadConfig::default())?;
//! let weights = loader.load_weights()?;
//!
//! // Initialize model from weights
//! let initializer = ModelInitializer::new(weights)?;
//! let model = initializer.build_model()?;
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use super::{
    GgufQuantType, LoadedTensor, LoadedWeights, ModelConfig, QuantizedTensor, TensorCategory,
};
use crate::backends::ModelArchitecture;
use crate::error::{Result, RuvLLMError};

// ============================================================================
// Model Layer Weights
// ============================================================================

/// Weights for a single transformer layer.
#[derive(Clone)]
pub struct LayerWeights {
    /// Layer index
    pub layer_idx: usize,
    /// Query projection (Wq)
    pub q_proj: WeightTensor,
    /// Key projection (Wk)
    pub k_proj: WeightTensor,
    /// Value projection (Wv)
    pub v_proj: WeightTensor,
    /// Output projection (Wo)
    pub o_proj: WeightTensor,
    /// Attention layer normalization
    pub attn_norm: Option<WeightTensor>,
    /// FFN gate projection (gate_proj / w1)
    pub gate_proj: WeightTensor,
    /// FFN up projection (up_proj / w3)
    pub up_proj: WeightTensor,
    /// FFN down projection (down_proj / w2)
    pub down_proj: WeightTensor,
    /// FFN layer normalization
    pub ffn_norm: Option<WeightTensor>,
}

/// Full model weights container.
#[derive(Clone)]
pub struct ModelWeights {
    /// Model architecture
    pub architecture: ModelArchitecture,
    /// Model configuration
    pub config: ModelConfig,
    /// Token embedding weights
    pub embed_tokens: WeightTensor,
    /// Per-layer weights
    pub layers: Vec<LayerWeights>,
    /// Final layer normalization
    pub final_norm: Option<WeightTensor>,
    /// Output/LM head weights (may be tied to embed_tokens)
    pub lm_head: Option<WeightTensor>,
    /// Total memory usage
    pub memory_bytes: usize,
}

/// A weight tensor that can be either quantized or F32.
#[derive(Clone)]
pub enum WeightTensor {
    /// Full precision F32 weights
    F32(Arc<Vec<f32>>, Vec<usize>),
    /// Quantized weights
    Quantized(Arc<QuantizedWeight>),
}

/// Quantized weight data for efficient inference.
#[derive(Clone)]
pub struct QuantizedWeight {
    /// Raw quantized data
    pub data: Vec<u8>,
    /// Quantization type
    pub quant_type: GgufQuantType,
    /// Tensor shape
    pub shape: Vec<usize>,
    /// Number of elements
    pub num_elements: usize,
}

impl WeightTensor {
    /// Get tensor shape.
    pub fn shape(&self) -> &[usize] {
        match self {
            WeightTensor::F32(_, shape) => shape,
            WeightTensor::Quantized(q) => &q.shape,
        }
    }

    /// Check if quantized.
    pub fn is_quantized(&self) -> bool {
        matches!(self, WeightTensor::Quantized(_))
    }

    /// Get F32 data (dequantizing if necessary).
    pub fn to_f32(&self) -> Result<Vec<f32>> {
        match self {
            WeightTensor::F32(data, _) => Ok((**data).clone()),
            WeightTensor::Quantized(q) => {
                super::quantization::dequantize_tensor(&q.data, q.quant_type, q.num_elements)
            }
        }
    }

    /// Get memory size in bytes.
    pub fn memory_bytes(&self) -> usize {
        match self {
            WeightTensor::F32(data, _) => data.len() * 4,
            WeightTensor::Quantized(q) => q.data.len(),
        }
    }

    /// Create from loaded tensor.
    pub fn from_loaded(tensor: &LoadedTensor) -> Result<Self> {
        if let Some(ref data) = tensor.data_f32 {
            Ok(WeightTensor::F32(
                Arc::new(data.clone()),
                tensor.shape.clone(),
            ))
        } else if let Some(ref quantized) = tensor.data_quantized {
            Ok(WeightTensor::Quantized(Arc::new(QuantizedWeight {
                data: quantized.data.clone(),
                quant_type: quantized.dtype,
                shape: quantized.shape.clone(),
                num_elements: quantized.num_elements,
            })))
        } else {
            Err(RuvLLMError::Model("Tensor has no data".to_string()))
        }
    }
}

// ============================================================================
// Model Initializer
// ============================================================================

/// Initializes model weights from loaded GGUF data.
///
/// This struct handles the mapping of GGUF tensor names to the appropriate
/// model layer weights based on the detected architecture.
pub struct ModelInitializer {
    /// Loaded weights
    weights: LoadedWeights,
    /// Architecture
    architecture: ModelArchitecture,
    /// Tensor name mappings for the architecture
    tensor_map: TensorNameMap,
}

/// Architecture-specific tensor name mappings.
struct TensorNameMap {
    /// Embedding tensor name pattern
    embed_tokens: &'static str,
    /// Query projection pattern
    q_proj: &'static str,
    /// Key projection pattern
    k_proj: &'static str,
    /// Value projection pattern
    v_proj: &'static str,
    /// Output projection pattern
    o_proj: &'static str,
    /// Attention norm pattern
    attn_norm: &'static str,
    /// Gate projection pattern
    gate_proj: &'static str,
    /// Up projection pattern
    up_proj: &'static str,
    /// Down projection pattern
    down_proj: &'static str,
    /// FFN norm pattern
    ffn_norm: &'static str,
    /// Final norm pattern
    final_norm: &'static str,
    /// LM head pattern
    lm_head: &'static str,
}

impl TensorNameMap {
    /// Get tensor name maps for Llama architecture.
    fn llama() -> Self {
        Self {
            embed_tokens: "model.embed_tokens.weight",
            q_proj: "model.layers.{}.self_attn.q_proj.weight",
            k_proj: "model.layers.{}.self_attn.k_proj.weight",
            v_proj: "model.layers.{}.self_attn.v_proj.weight",
            o_proj: "model.layers.{}.self_attn.o_proj.weight",
            attn_norm: "model.layers.{}.input_layernorm.weight",
            gate_proj: "model.layers.{}.mlp.gate_proj.weight",
            up_proj: "model.layers.{}.mlp.up_proj.weight",
            down_proj: "model.layers.{}.mlp.down_proj.weight",
            ffn_norm: "model.layers.{}.post_attention_layernorm.weight",
            final_norm: "model.norm.weight",
            lm_head: "lm_head.weight",
        }
    }

    /// Get tensor name maps for Mistral architecture.
    fn mistral() -> Self {
        // Mistral uses same naming as Llama
        Self::llama()
    }

    /// Get tensor name maps for Phi architecture.
    fn phi() -> Self {
        Self {
            embed_tokens: "transformer.embd.wte.weight",
            q_proj: "transformer.h.{}.mixer.Wqkv.weight", // Combined QKV
            k_proj: "transformer.h.{}.mixer.Wqkv.weight",
            v_proj: "transformer.h.{}.mixer.Wqkv.weight",
            o_proj: "transformer.h.{}.mixer.out_proj.weight",
            attn_norm: "transformer.h.{}.ln.weight",
            gate_proj: "transformer.h.{}.mlp.fc1.weight",
            up_proj: "transformer.h.{}.mlp.fc1.weight", // Combined with gate
            down_proj: "transformer.h.{}.mlp.fc2.weight",
            ffn_norm: "transformer.h.{}.ln.weight", // Same as attn_norm for Phi
            final_norm: "transformer.ln_f.weight",
            lm_head: "lm_head.weight",
        }
    }

    /// Get tensor name maps for Phi-3 architecture.
    fn phi3() -> Self {
        Self {
            embed_tokens: "model.embed_tokens.weight",
            q_proj: "model.layers.{}.self_attn.qkv_proj.weight",
            k_proj: "model.layers.{}.self_attn.qkv_proj.weight",
            v_proj: "model.layers.{}.self_attn.qkv_proj.weight",
            o_proj: "model.layers.{}.self_attn.o_proj.weight",
            attn_norm: "model.layers.{}.input_layernorm.weight",
            gate_proj: "model.layers.{}.mlp.gate_up_proj.weight",
            up_proj: "model.layers.{}.mlp.gate_up_proj.weight",
            down_proj: "model.layers.{}.mlp.down_proj.weight",
            ffn_norm: "model.layers.{}.post_attention_layernorm.weight",
            final_norm: "model.norm.weight",
            lm_head: "lm_head.weight",
        }
    }

    /// Get tensor name maps for Gemma architecture.
    fn gemma() -> Self {
        Self {
            embed_tokens: "model.embed_tokens.weight",
            q_proj: "model.layers.{}.self_attn.q_proj.weight",
            k_proj: "model.layers.{}.self_attn.k_proj.weight",
            v_proj: "model.layers.{}.self_attn.v_proj.weight",
            o_proj: "model.layers.{}.self_attn.o_proj.weight",
            attn_norm: "model.layers.{}.input_layernorm.weight",
            gate_proj: "model.layers.{}.mlp.gate_proj.weight",
            up_proj: "model.layers.{}.mlp.up_proj.weight",
            down_proj: "model.layers.{}.mlp.down_proj.weight",
            ffn_norm: "model.layers.{}.post_attention_layernorm.weight",
            final_norm: "model.norm.weight",
            lm_head: "model.embed_tokens.weight", // Tied embeddings
        }
    }

    /// Get tensor name maps for Qwen architecture.
    fn qwen() -> Self {
        Self {
            embed_tokens: "transformer.wte.weight",
            q_proj: "transformer.h.{}.attn.c_attn.weight",
            k_proj: "transformer.h.{}.attn.c_attn.weight",
            v_proj: "transformer.h.{}.attn.c_attn.weight",
            o_proj: "transformer.h.{}.attn.c_proj.weight",
            attn_norm: "transformer.h.{}.ln_1.weight",
            gate_proj: "transformer.h.{}.mlp.w1.weight",
            up_proj: "transformer.h.{}.mlp.w2.weight",
            down_proj: "transformer.h.{}.mlp.c_proj.weight",
            ffn_norm: "transformer.h.{}.ln_2.weight",
            final_norm: "transformer.ln_f.weight",
            lm_head: "lm_head.weight",
        }
    }

    /// Get tensor name with layer index substituted.
    fn layer_tensor(&self, pattern: &str, layer: usize) -> String {
        pattern.replace("{}", &layer.to_string())
    }
}

impl ModelInitializer {
    /// Create a new model initializer from loaded weights.
    pub fn new(weights: LoadedWeights) -> Result<Self> {
        let architecture = weights
            .architecture()
            .ok_or_else(|| RuvLLMError::Model("Cannot determine model architecture".to_string()))?;

        let tensor_map = match architecture {
            ModelArchitecture::Llama => TensorNameMap::llama(),
            ModelArchitecture::Mistral => TensorNameMap::mistral(),
            ModelArchitecture::Phi => TensorNameMap::phi(),
            ModelArchitecture::Phi3 => TensorNameMap::phi3(),
            ModelArchitecture::Gemma | ModelArchitecture::Gemma2 => TensorNameMap::gemma(),
            ModelArchitecture::Qwen => TensorNameMap::qwen(),
        };

        Ok(Self {
            weights,
            architecture,
            tensor_map,
        })
    }

    /// Build the model weights structure.
    pub fn build_weights(&self) -> Result<ModelWeights> {
        let config = self.weights.config().clone();
        let num_layers = config.layer_count.unwrap_or(0);

        // Load embedding
        let embed_tokens = self.load_tensor(&self.tensor_map.embed_tokens)?;

        // Load layers
        let mut layers = Vec::with_capacity(num_layers);
        for layer_idx in 0..num_layers {
            let layer = self.load_layer(layer_idx)?;
            layers.push(layer);
        }

        // Load final norm
        let final_norm = self.try_load_tensor(&self.tensor_map.final_norm);

        // Load LM head (may be tied to embeddings)
        let lm_head = self.try_load_tensor(&self.tensor_map.lm_head);

        // Calculate memory
        let mut memory_bytes = embed_tokens.memory_bytes();
        for layer in &layers {
            memory_bytes += layer.q_proj.memory_bytes();
            memory_bytes += layer.k_proj.memory_bytes();
            memory_bytes += layer.v_proj.memory_bytes();
            memory_bytes += layer.o_proj.memory_bytes();
            memory_bytes += layer.gate_proj.memory_bytes();
            memory_bytes += layer.up_proj.memory_bytes();
            memory_bytes += layer.down_proj.memory_bytes();
            if let Some(ref norm) = layer.attn_norm {
                memory_bytes += norm.memory_bytes();
            }
            if let Some(ref norm) = layer.ffn_norm {
                memory_bytes += norm.memory_bytes();
            }
        }
        if let Some(ref norm) = final_norm {
            memory_bytes += norm.memory_bytes();
        }
        if let Some(ref head) = lm_head {
            memory_bytes += head.memory_bytes();
        }

        Ok(ModelWeights {
            architecture: self.architecture,
            config,
            embed_tokens,
            layers,
            final_norm,
            lm_head,
            memory_bytes,
        })
    }

    /// Load a single layer's weights.
    fn load_layer(&self, layer_idx: usize) -> Result<LayerWeights> {
        let q_proj = self.load_layer_tensor(&self.tensor_map.q_proj, layer_idx)?;
        let k_proj = self.load_layer_tensor(&self.tensor_map.k_proj, layer_idx)?;
        let v_proj = self.load_layer_tensor(&self.tensor_map.v_proj, layer_idx)?;
        let o_proj = self.load_layer_tensor(&self.tensor_map.o_proj, layer_idx)?;
        let gate_proj = self.load_layer_tensor(&self.tensor_map.gate_proj, layer_idx)?;
        let up_proj = self.load_layer_tensor(&self.tensor_map.up_proj, layer_idx)?;
        let down_proj = self.load_layer_tensor(&self.tensor_map.down_proj, layer_idx)?;

        let attn_norm = self.try_load_layer_tensor(&self.tensor_map.attn_norm, layer_idx);
        let ffn_norm = self.try_load_layer_tensor(&self.tensor_map.ffn_norm, layer_idx);

        Ok(LayerWeights {
            layer_idx,
            q_proj,
            k_proj,
            v_proj,
            o_proj,
            attn_norm,
            gate_proj,
            up_proj,
            down_proj,
            ffn_norm,
        })
    }

    /// Load a tensor by name.
    fn load_tensor(&self, name: &str) -> Result<WeightTensor> {
        // Try to find the tensor with exact name first
        if let Some(tensor) = self.weights.get(name) {
            return WeightTensor::from_loaded(tensor);
        }

        // Try normalized name
        let normalized = self.normalize_name(name);
        if let Some(tensor) = self.weights.get(&normalized) {
            return WeightTensor::from_loaded(tensor);
        }

        // Try to find by fuzzy matching
        for tensor_name in self.weights.tensor_names() {
            if tensor_name.contains(&self.extract_key_part(name)) {
                if let Some(tensor) = self.weights.get(tensor_name) {
                    return WeightTensor::from_loaded(tensor);
                }
            }
        }

        Err(RuvLLMError::NotFound(format!("Tensor not found: {}", name)))
    }

    /// Try to load a tensor, returning None if not found.
    fn try_load_tensor(&self, name: &str) -> Option<WeightTensor> {
        self.load_tensor(name).ok()
    }

    /// Load a layer-specific tensor.
    fn load_layer_tensor(&self, pattern: &str, layer: usize) -> Result<WeightTensor> {
        let name = self.tensor_map.layer_tensor(pattern, layer);
        self.load_tensor(&name)
    }

    /// Try to load a layer-specific tensor.
    fn try_load_layer_tensor(&self, pattern: &str, layer: usize) -> Option<WeightTensor> {
        let name = self.tensor_map.layer_tensor(pattern, layer);
        self.try_load_tensor(&name)
    }

    /// Normalize tensor name for lookup.
    fn normalize_name(&self, name: &str) -> String {
        name.replace("model.", "")
            .replace("transformer.", "")
            .replace("h.", "layers.")
    }

    /// Extract the key identifying part of a tensor name.
    fn extract_key_part(&self, name: &str) -> String {
        // Extract the last meaningful part of the name
        name.split('.').last().unwrap_or(name).to_string()
    }
}

// ============================================================================
// Progress-Aware Model Building
// ============================================================================

/// Builder for constructing models with progress callbacks.
pub struct ProgressModelBuilder {
    weights: LoadedWeights,
    progress_callback: Option<Box<dyn Fn(&str, usize, usize) + Send + Sync>>,
}

impl ProgressModelBuilder {
    /// Create a new builder.
    pub fn new(weights: LoadedWeights) -> Self {
        Self {
            weights,
            progress_callback: None,
        }
    }

    /// Set progress callback.
    ///
    /// Callback receives: (stage_name, current_step, total_steps)
    pub fn with_progress<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, usize, usize) + Send + Sync + 'static,
    {
        self.progress_callback = Some(Box::new(callback));
        self
    }

    /// Build the model weights.
    pub fn build(self) -> Result<ModelWeights> {
        let initializer = ModelInitializer::new(self.weights)?;

        if let Some(ref callback) = self.progress_callback {
            callback("Initializing model", 0, 3);
        }

        let weights = initializer.build_weights()?;

        if let Some(ref callback) = self.progress_callback {
            callback("Model ready", 3, 3);
        }

        Ok(weights)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tensor_name_map_llama() {
        let map = TensorNameMap::llama();
        assert_eq!(
            map.layer_tensor(map.q_proj, 0),
            "model.layers.0.self_attn.q_proj.weight"
        );
        assert_eq!(
            map.layer_tensor(map.gate_proj, 5),
            "model.layers.5.mlp.gate_proj.weight"
        );
    }

    #[test]
    fn test_tensor_name_map_phi() {
        let map = TensorNameMap::phi();
        assert_eq!(
            map.layer_tensor(map.o_proj, 2),
            "transformer.h.2.mixer.out_proj.weight"
        );
    }

    #[test]
    fn test_weight_tensor_f32() {
        let data = vec![1.0f32, 2.0, 3.0, 4.0];
        let shape = vec![2, 2];
        let tensor = WeightTensor::F32(Arc::new(data.clone()), shape.clone());

        assert!(!tensor.is_quantized());
        assert_eq!(tensor.shape(), &[2, 2]);
        assert_eq!(tensor.memory_bytes(), 16); // 4 floats * 4 bytes
        assert_eq!(tensor.to_f32().unwrap(), data);
    }

    #[test]
    fn test_weight_tensor_quantized() {
        let data = vec![0u8; 18]; // One Q4_0 block
        let tensor = WeightTensor::Quantized(Arc::new(QuantizedWeight {
            data: data.clone(),
            quant_type: GgufQuantType::Q4_0,
            shape: vec![32],
            num_elements: 32,
        }));

        assert!(tensor.is_quantized());
        assert_eq!(tensor.shape(), &[32]);
        assert_eq!(tensor.memory_bytes(), 18);
    }
}
