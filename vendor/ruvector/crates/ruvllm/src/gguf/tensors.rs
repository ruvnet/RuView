//! GGUF Tensor Information and Utilities
//!
//! This module provides tensor-related structures and utilities for
//! working with GGUF model tensors.

use super::quantization::GgufQuantType;

// ============================================================================
// Tensor Information
// ============================================================================

/// Information about a tensor stored in a GGUF file.
///
/// This structure contains all the metadata needed to locate and
/// interpret a tensor in the GGUF file.
#[derive(Debug, Clone)]
pub struct TensorInfo {
    /// Tensor name (e.g., "model.layers.0.attention.wq.weight")
    pub name: String,
    /// Tensor shape (e.g., [4096, 4096] for a weight matrix)
    pub shape: Vec<usize>,
    /// Data type / quantization format
    pub dtype: GgufQuantType,
    /// Offset from the start of the tensor data section
    pub offset: u64,
}

impl TensorInfo {
    /// Get the total number of elements in the tensor.
    pub fn num_elements(&self) -> usize {
        self.shape.iter().product()
    }

    /// Get the byte size of the tensor data.
    pub fn byte_size(&self) -> usize {
        self.dtype.tensor_size(self.num_elements())
    }

    /// Check if this is a weight tensor.
    pub fn is_weight(&self) -> bool {
        self.name.contains("weight")
    }

    /// Check if this is a bias tensor.
    pub fn is_bias(&self) -> bool {
        self.name.contains("bias")
    }

    /// Check if this is an embedding tensor.
    pub fn is_embedding(&self) -> bool {
        self.name.contains("embed") || self.name.contains("token")
    }

    /// Check if this is an attention tensor.
    pub fn is_attention(&self) -> bool {
        self.name.contains("attn") || self.name.contains("attention")
    }

    /// Check if this is a feed-forward tensor.
    pub fn is_ffn(&self) -> bool {
        self.name.contains("ffn") || self.name.contains("feed_forward") || self.name.contains("mlp")
    }

    /// Check if this is a normalization tensor.
    pub fn is_norm(&self) -> bool {
        self.name.contains("norm") || self.name.contains("ln")
    }

    /// Get the layer number if this is a layer tensor.
    pub fn layer_index(&self) -> Option<usize> {
        // Parse patterns like "model.layers.0." or "transformer.h.0."
        for pattern in &["layers.", "h.", "block."] {
            if let Some(pos) = self.name.find(pattern) {
                let after_pattern = &self.name[pos + pattern.len()..];
                if let Some(end) = after_pattern.find('.') {
                    if let Ok(idx) = after_pattern[..end].parse() {
                        return Some(idx);
                    }
                }
            }
        }
        None
    }

    /// Get the tensor type (attention, ffn, norm, etc.).
    pub fn tensor_type(&self) -> TensorType {
        if self.is_embedding() {
            TensorType::Embedding
        } else if self.is_attention() {
            if self.name.contains("q_proj") || self.name.contains("wq") {
                TensorType::AttentionQuery
            } else if self.name.contains("k_proj") || self.name.contains("wk") {
                TensorType::AttentionKey
            } else if self.name.contains("v_proj") || self.name.contains("wv") {
                TensorType::AttentionValue
            } else if self.name.contains("o_proj") || self.name.contains("wo") {
                TensorType::AttentionOutput
            } else {
                TensorType::Attention
            }
        } else if self.is_ffn() {
            if self.name.contains("gate") || self.name.contains("w1") {
                TensorType::FfnGate
            } else if self.name.contains("up") || self.name.contains("w3") {
                TensorType::FfnUp
            } else if self.name.contains("down") || self.name.contains("w2") {
                TensorType::FfnDown
            } else {
                TensorType::Ffn
            }
        } else if self.is_norm() {
            TensorType::Norm
        } else if self.name.contains("output") || self.name.contains("lm_head") {
            TensorType::Output
        } else {
            TensorType::Other
        }
    }
}

// ============================================================================
// Tensor Type Classification
// ============================================================================

/// Classification of tensor types in a transformer model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TensorType {
    /// Token embedding layer
    Embedding,
    /// Generic attention tensor
    Attention,
    /// Query projection (Wq)
    AttentionQuery,
    /// Key projection (Wk)
    AttentionKey,
    /// Value projection (Wv)
    AttentionValue,
    /// Output projection (Wo)
    AttentionOutput,
    /// Generic feed-forward tensor
    Ffn,
    /// Feed-forward gate (SwiGLU w1)
    FfnGate,
    /// Feed-forward up projection (w3)
    FfnUp,
    /// Feed-forward down projection (w2)
    FfnDown,
    /// Normalization layer (RMSNorm, LayerNorm)
    Norm,
    /// Output/LM head
    Output,
    /// Other tensor type
    Other,
}

impl TensorType {
    /// Get a human-readable name for this tensor type.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Embedding => "embedding",
            Self::Attention => "attention",
            Self::AttentionQuery => "attention.q",
            Self::AttentionKey => "attention.k",
            Self::AttentionValue => "attention.v",
            Self::AttentionOutput => "attention.o",
            Self::Ffn => "ffn",
            Self::FfnGate => "ffn.gate",
            Self::FfnUp => "ffn.up",
            Self::FfnDown => "ffn.down",
            Self::Norm => "norm",
            Self::Output => "output",
            Self::Other => "other",
        }
    }
}

// ============================================================================
// Tensor Collection Utilities
// ============================================================================

/// Statistics about tensors in a GGUF file.
#[derive(Debug, Clone, Default)]
pub struct TensorStats {
    /// Total number of tensors
    pub count: usize,
    /// Total number of elements across all tensors
    pub total_elements: usize,
    /// Total size in bytes
    pub total_bytes: usize,
    /// Number of layers detected
    pub layer_count: usize,
    /// Quantization types used
    pub quant_types: Vec<GgufQuantType>,
}

impl TensorStats {
    /// Compute statistics from a list of tensors.
    pub fn from_tensors(tensors: &[TensorInfo]) -> Self {
        let mut stats = Self::default();
        let mut max_layer = 0usize;
        let mut quant_set = std::collections::HashSet::new();

        for tensor in tensors {
            stats.count += 1;
            stats.total_elements += tensor.num_elements();
            stats.total_bytes += tensor.byte_size();

            if let Some(layer) = tensor.layer_index() {
                max_layer = max_layer.max(layer + 1);
            }

            quant_set.insert(tensor.dtype);
        }

        stats.layer_count = max_layer;
        stats.quant_types = quant_set.into_iter().collect();

        stats
    }

    /// Get the average bits per weight.
    pub fn avg_bits_per_weight(&self) -> f32 {
        if self.total_elements == 0 {
            return 0.0;
        }
        (self.total_bytes as f32 * 8.0) / self.total_elements as f32
    }
}

// ============================================================================
// Tensor Name Parsing
// ============================================================================

/// Parse a tensor name into its components.
///
/// # Arguments
///
/// * `name` - The full tensor name
///
/// # Returns
///
/// Parsed components of the name
pub fn parse_tensor_name(name: &str) -> TensorNameParts {
    let parts: Vec<&str> = name.split('.').collect();

    TensorNameParts {
        full_name: name.to_string(),
        parts: parts.iter().map(|s| s.to_string()).collect(),
        layer_index: extract_layer_index(name),
        tensor_type: extract_tensor_type(name),
    }
}

/// Parsed components of a tensor name.
#[derive(Debug, Clone)]
pub struct TensorNameParts {
    /// The full tensor name
    pub full_name: String,
    /// Split parts of the name
    pub parts: Vec<String>,
    /// Layer index if present
    pub layer_index: Option<usize>,
    /// Inferred tensor type
    pub tensor_type: String,
}

fn extract_layer_index(name: &str) -> Option<usize> {
    for pattern in &["layers.", "h.", "block."] {
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

fn extract_tensor_type(name: &str) -> String {
    let suffixes = [
        "weight",
        "bias",
        "scale",
        "norm",
        "wq",
        "wk",
        "wv",
        "wo",
        "w1",
        "w2",
        "w3",
        "q_proj",
        "k_proj",
        "v_proj",
        "o_proj",
        "gate_proj",
        "up_proj",
        "down_proj",
    ];

    for suffix in &suffixes {
        if name.contains(suffix) {
            return suffix.to_string();
        }
    }

    "unknown".to_string()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tensor(name: &str) -> TensorInfo {
        TensorInfo {
            name: name.to_string(),
            shape: vec![4096, 4096],
            dtype: GgufQuantType::Q4_K,
            offset: 0,
        }
    }

    #[test]
    fn test_tensor_info_basic() {
        let tensor = make_tensor("model.layers.0.attention.wq.weight");

        assert_eq!(tensor.num_elements(), 4096 * 4096);
        assert!(tensor.is_weight());
        assert!(tensor.is_attention());
        assert_eq!(tensor.layer_index(), Some(0));
    }

    #[test]
    fn test_tensor_type_classification() {
        assert_eq!(
            make_tensor("model.embed_tokens.weight").tensor_type(),
            TensorType::Embedding
        );
        assert_eq!(
            make_tensor("model.layers.0.self_attn.q_proj.weight").tensor_type(),
            TensorType::AttentionQuery
        );
        assert_eq!(
            make_tensor("model.layers.0.mlp.gate_proj.weight").tensor_type(),
            TensorType::FfnGate
        );
        assert_eq!(
            make_tensor("model.layers.0.input_layernorm.weight").tensor_type(),
            TensorType::Norm
        );
    }

    #[test]
    fn test_layer_index_parsing() {
        assert_eq!(make_tensor("model.layers.0.weight").layer_index(), Some(0));
        assert_eq!(
            make_tensor("model.layers.15.weight").layer_index(),
            Some(15)
        );
        assert_eq!(make_tensor("transformer.h.7.weight").layer_index(), Some(7));
        assert_eq!(make_tensor("model.embed_tokens.weight").layer_index(), None);
    }

    #[test]
    fn test_tensor_stats() {
        let tensors = vec![
            TensorInfo {
                name: "model.layers.0.weight".to_string(),
                shape: vec![1000],
                dtype: GgufQuantType::Q4_K,
                offset: 0,
            },
            TensorInfo {
                name: "model.layers.1.weight".to_string(),
                shape: vec![1000],
                dtype: GgufQuantType::Q4_K,
                offset: 0,
            },
        ];

        let stats = TensorStats::from_tensors(&tensors);

        assert_eq!(stats.count, 2);
        assert_eq!(stats.total_elements, 2000);
        assert_eq!(stats.layer_count, 2);
    }

    #[test]
    fn test_parse_tensor_name() {
        let parts = parse_tensor_name("model.layers.5.self_attn.q_proj.weight");

        assert_eq!(parts.layer_index, Some(5));
        assert!(parts.parts.len() >= 4);
    }

    #[test]
    fn test_tensor_type_names() {
        assert_eq!(TensorType::Embedding.name(), "embedding");
        assert_eq!(TensorType::AttentionQuery.name(), "attention.q");
        assert_eq!(TensorType::FfnGate.name(), "ffn.gate");
    }
}
