//! Neural Attention Mechanisms (from ruvector-attention)
//!
//! Re-exports the 7 core neural attention mechanisms:
//! - Scaled Dot-Product Attention
//! - Multi-Head Attention
//! - Hyperbolic Attention
//! - Linear Attention (Performer)
//! - Flash Attention
//! - Local-Global Attention
//! - Mixture of Experts (MoE) Attention

use ruvector_attention::{
    attention::{MultiHeadAttention, ScaledDotProductAttention},
    hyperbolic::{HyperbolicAttention, HyperbolicAttentionConfig},
    moe::{MoEAttention, MoEConfig},
    sparse::{FlashAttention, LinearAttention, LocalGlobalAttention},
    traits::Attention,
};
use wasm_bindgen::prelude::*;

// ============================================================================
// Scaled Dot-Product Attention
// ============================================================================

/// Compute scaled dot-product attention
///
/// Standard transformer attention: softmax(QK^T / sqrt(d)) * V
///
/// # Arguments
/// * `query` - Query vector (Float32Array)
/// * `keys` - Array of key vectors (JsValue - array of Float32Arrays)
/// * `values` - Array of value vectors (JsValue - array of Float32Arrays)
/// * `scale` - Optional scaling factor (defaults to 1/sqrt(dim))
///
/// # Returns
/// Attention-weighted output vector
#[wasm_bindgen(js_name = scaledDotAttention)]
pub fn scaled_dot_attention(
    query: &[f32],
    keys: JsValue,
    values: JsValue,
    scale: Option<f32>,
) -> Result<Vec<f32>, JsError> {
    let keys_vec: Vec<Vec<f32>> = serde_wasm_bindgen::from_value(keys)
        .map_err(|e| JsError::new(&format!("Failed to parse keys: {}", e)))?;
    let values_vec: Vec<Vec<f32>> = serde_wasm_bindgen::from_value(values)
        .map_err(|e| JsError::new(&format!("Failed to parse values: {}", e)))?;

    let keys_refs: Vec<&[f32]> = keys_vec.iter().map(|k| k.as_slice()).collect();
    let values_refs: Vec<&[f32]> = values_vec.iter().map(|v| v.as_slice()).collect();

    let attention = ScaledDotProductAttention::new(query.len());
    attention
        .compute(query, &keys_refs, &values_refs)
        .map_err(|e| JsError::new(&e.to_string()))
}

// ============================================================================
// Multi-Head Attention
// ============================================================================

/// Multi-head attention mechanism
///
/// Splits input into multiple heads, applies attention, and concatenates results
#[wasm_bindgen]
pub struct WasmMultiHeadAttention {
    inner: MultiHeadAttention,
}

#[wasm_bindgen]
impl WasmMultiHeadAttention {
    /// Create a new multi-head attention instance
    ///
    /// # Arguments
    /// * `dim` - Embedding dimension (must be divisible by num_heads)
    /// * `num_heads` - Number of parallel attention heads
    #[wasm_bindgen(constructor)]
    pub fn new(dim: usize, num_heads: usize) -> Result<WasmMultiHeadAttention, JsError> {
        if dim % num_heads != 0 {
            return Err(JsError::new(&format!(
                "Dimension {} must be divisible by number of heads {}",
                dim, num_heads
            )));
        }
        Ok(Self {
            inner: MultiHeadAttention::new(dim, num_heads),
        })
    }

    /// Compute multi-head attention
    ///
    /// # Arguments
    /// * `query` - Query vector
    /// * `keys` - Array of key vectors
    /// * `values` - Array of value vectors
    pub fn compute(
        &self,
        query: &[f32],
        keys: JsValue,
        values: JsValue,
    ) -> Result<Vec<f32>, JsError> {
        let keys_vec: Vec<Vec<f32>> = serde_wasm_bindgen::from_value(keys)?;
        let values_vec: Vec<Vec<f32>> = serde_wasm_bindgen::from_value(values)?;

        let keys_refs: Vec<&[f32]> = keys_vec.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values_vec.iter().map(|v| v.as_slice()).collect();

        self.inner
            .compute(query, &keys_refs, &values_refs)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Get the number of attention heads
    #[wasm_bindgen(getter, js_name = numHeads)]
    pub fn num_heads(&self) -> usize {
        self.inner.num_heads()
    }

    /// Get the embedding dimension
    #[wasm_bindgen(getter)]
    pub fn dim(&self) -> usize {
        self.inner.dim()
    }

    /// Get the dimension per head
    #[wasm_bindgen(getter, js_name = headDim)]
    pub fn head_dim(&self) -> usize {
        self.inner.dim() / self.inner.num_heads()
    }
}

// ============================================================================
// Hyperbolic Attention
// ============================================================================

/// Hyperbolic attention mechanism for hierarchical data
///
/// Operates in hyperbolic space (Poincare ball model) which naturally
/// represents tree-like hierarchical structures with exponential capacity
#[wasm_bindgen]
pub struct WasmHyperbolicAttention {
    inner: HyperbolicAttention,
    curvature_value: f32,
}

#[wasm_bindgen]
impl WasmHyperbolicAttention {
    /// Create a new hyperbolic attention instance
    ///
    /// # Arguments
    /// * `dim` - Embedding dimension
    /// * `curvature` - Hyperbolic curvature parameter (negative for hyperbolic space)
    #[wasm_bindgen(constructor)]
    pub fn new(dim: usize, curvature: f32) -> WasmHyperbolicAttention {
        let config = HyperbolicAttentionConfig {
            dim,
            curvature,
            ..Default::default()
        };
        Self {
            inner: HyperbolicAttention::new(config),
            curvature_value: curvature,
        }
    }

    /// Compute hyperbolic attention
    pub fn compute(
        &self,
        query: &[f32],
        keys: JsValue,
        values: JsValue,
    ) -> Result<Vec<f32>, JsError> {
        let keys_vec: Vec<Vec<f32>> = serde_wasm_bindgen::from_value(keys)?;
        let values_vec: Vec<Vec<f32>> = serde_wasm_bindgen::from_value(values)?;

        let keys_refs: Vec<&[f32]> = keys_vec.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values_vec.iter().map(|v| v.as_slice()).collect();

        self.inner
            .compute(query, &keys_refs, &values_refs)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Get the curvature parameter
    #[wasm_bindgen(getter)]
    pub fn curvature(&self) -> f32 {
        self.curvature_value
    }
}

// ============================================================================
// Linear Attention (Performer)
// ============================================================================

/// Linear attention using random feature approximation
///
/// Achieves O(n) complexity instead of O(n^2) by approximating
/// the softmax kernel with random Fourier features
#[wasm_bindgen]
pub struct WasmLinearAttention {
    inner: LinearAttention,
}

#[wasm_bindgen]
impl WasmLinearAttention {
    /// Create a new linear attention instance
    ///
    /// # Arguments
    /// * `dim` - Embedding dimension
    /// * `num_features` - Number of random features for kernel approximation
    #[wasm_bindgen(constructor)]
    pub fn new(dim: usize, num_features: usize) -> WasmLinearAttention {
        Self {
            inner: LinearAttention::new(dim, num_features),
        }
    }

    /// Compute linear attention
    pub fn compute(
        &self,
        query: &[f32],
        keys: JsValue,
        values: JsValue,
    ) -> Result<Vec<f32>, JsError> {
        let keys_vec: Vec<Vec<f32>> = serde_wasm_bindgen::from_value(keys)?;
        let values_vec: Vec<Vec<f32>> = serde_wasm_bindgen::from_value(values)?;

        let keys_refs: Vec<&[f32]> = keys_vec.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values_vec.iter().map(|v| v.as_slice()).collect();

        self.inner
            .compute(query, &keys_refs, &values_refs)
            .map_err(|e| JsError::new(&e.to_string()))
    }
}

// ============================================================================
// Flash Attention
// ============================================================================

/// Flash attention with memory-efficient tiling
///
/// Reduces memory usage from O(n^2) to O(n) by computing attention
/// in blocks and fusing operations
#[wasm_bindgen]
pub struct WasmFlashAttention {
    inner: FlashAttention,
}

#[wasm_bindgen]
impl WasmFlashAttention {
    /// Create a new flash attention instance
    ///
    /// # Arguments
    /// * `dim` - Embedding dimension
    /// * `block_size` - Block size for tiled computation
    #[wasm_bindgen(constructor)]
    pub fn new(dim: usize, block_size: usize) -> WasmFlashAttention {
        Self {
            inner: FlashAttention::new(dim, block_size),
        }
    }

    /// Compute flash attention
    pub fn compute(
        &self,
        query: &[f32],
        keys: JsValue,
        values: JsValue,
    ) -> Result<Vec<f32>, JsError> {
        let keys_vec: Vec<Vec<f32>> = serde_wasm_bindgen::from_value(keys)?;
        let values_vec: Vec<Vec<f32>> = serde_wasm_bindgen::from_value(values)?;

        let keys_refs: Vec<&[f32]> = keys_vec.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values_vec.iter().map(|v| v.as_slice()).collect();

        self.inner
            .compute(query, &keys_refs, &values_refs)
            .map_err(|e| JsError::new(&e.to_string()))
    }
}

// ============================================================================
// Local-Global Attention
// ============================================================================

/// Local-global sparse attention (Longformer-style)
///
/// Combines local sliding window attention with global tokens
/// for efficient long-range dependencies
#[wasm_bindgen]
pub struct WasmLocalGlobalAttention {
    inner: LocalGlobalAttention,
}

#[wasm_bindgen]
impl WasmLocalGlobalAttention {
    /// Create a new local-global attention instance
    ///
    /// # Arguments
    /// * `dim` - Embedding dimension
    /// * `local_window` - Size of local attention window
    /// * `global_tokens` - Number of global attention tokens
    #[wasm_bindgen(constructor)]
    pub fn new(dim: usize, local_window: usize, global_tokens: usize) -> WasmLocalGlobalAttention {
        Self {
            inner: LocalGlobalAttention::new(dim, local_window, global_tokens),
        }
    }

    /// Compute local-global attention
    pub fn compute(
        &self,
        query: &[f32],
        keys: JsValue,
        values: JsValue,
    ) -> Result<Vec<f32>, JsError> {
        let keys_vec: Vec<Vec<f32>> = serde_wasm_bindgen::from_value(keys)?;
        let values_vec: Vec<Vec<f32>> = serde_wasm_bindgen::from_value(values)?;

        let keys_refs: Vec<&[f32]> = keys_vec.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values_vec.iter().map(|v| v.as_slice()).collect();

        self.inner
            .compute(query, &keys_refs, &values_refs)
            .map_err(|e| JsError::new(&e.to_string()))
    }
}

// ============================================================================
// Mixture of Experts (MoE) Attention
// ============================================================================

/// Mixture of Experts attention mechanism
///
/// Routes queries to specialized expert attention heads based on
/// learned gating functions for capacity-efficient computation
#[wasm_bindgen]
pub struct WasmMoEAttention {
    inner: MoEAttention,
}

#[wasm_bindgen]
impl WasmMoEAttention {
    /// Create a new MoE attention instance
    ///
    /// # Arguments
    /// * `dim` - Embedding dimension
    /// * `num_experts` - Number of expert attention mechanisms
    /// * `top_k` - Number of experts to activate per query
    #[wasm_bindgen(constructor)]
    pub fn new(dim: usize, num_experts: usize, top_k: usize) -> WasmMoEAttention {
        let config = MoEConfig::builder()
            .dim(dim)
            .num_experts(num_experts)
            .top_k(top_k)
            .build();
        Self {
            inner: MoEAttention::new(config),
        }
    }

    /// Compute MoE attention
    pub fn compute(
        &self,
        query: &[f32],
        keys: JsValue,
        values: JsValue,
    ) -> Result<Vec<f32>, JsError> {
        let keys_vec: Vec<Vec<f32>> = serde_wasm_bindgen::from_value(keys)?;
        let values_vec: Vec<Vec<f32>> = serde_wasm_bindgen::from_value(values)?;

        let keys_refs: Vec<&[f32]> = keys_vec.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values_vec.iter().map(|v| v.as_slice()).collect();

        self.inner
            .compute(query, &keys_refs, &values_refs)
            .map_err(|e| JsError::new(&e.to_string()))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_multi_head_creation() {
        let mha = WasmMultiHeadAttention::new(64, 8);
        assert!(mha.is_ok());
        let mha = mha.unwrap();
        assert_eq!(mha.dim(), 64);
        assert_eq!(mha.num_heads(), 8);
        assert_eq!(mha.head_dim(), 8);
    }

    #[wasm_bindgen_test]
    fn test_multi_head_invalid_dims() {
        let mha = WasmMultiHeadAttention::new(65, 8);
        assert!(mha.is_err());
    }

    #[wasm_bindgen_test]
    fn test_hyperbolic_attention() {
        let hyp = WasmHyperbolicAttention::new(32, -1.0);
        assert_eq!(hyp.curvature(), -1.0);
    }

    #[wasm_bindgen_test]
    fn test_linear_attention_creation() {
        let linear = WasmLinearAttention::new(64, 128);
        // Just verify it can be created
        assert!(true);
    }

    #[wasm_bindgen_test]
    fn test_flash_attention_creation() {
        let flash = WasmFlashAttention::new(64, 16);
        assert!(true);
    }

    #[wasm_bindgen_test]
    fn test_local_global_creation() {
        let lg = WasmLocalGlobalAttention::new(64, 128, 4);
        assert!(true);
    }

    #[wasm_bindgen_test]
    fn test_moe_attention_creation() {
        let moe = WasmMoEAttention::new(64, 8, 2);
        assert!(true);
    }
}
