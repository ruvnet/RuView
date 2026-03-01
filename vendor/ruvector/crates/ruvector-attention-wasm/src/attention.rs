use ruvector_attention::{
    attention::{MultiHeadAttention, ScaledDotProductAttention},
    hyperbolic::{HyperbolicAttention, HyperbolicAttentionConfig},
    moe::{MoEAttention, MoEConfig},
    sparse::{FlashAttention, LinearAttention, LocalGlobalAttention},
    traits::Attention,
};
use wasm_bindgen::prelude::*;

/// Compute scaled dot-product attention
///
/// # Arguments
/// * `query` - Query vector as Float32Array
/// * `keys` - Array of key vectors
/// * `values` - Array of value vectors
/// * `scale` - Optional scaling factor (defaults to 1/sqrt(dim))
#[wasm_bindgen]
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

/// Multi-head attention mechanism
#[wasm_bindgen]
pub struct WasmMultiHeadAttention {
    inner: MultiHeadAttention,
}

#[wasm_bindgen]
impl WasmMultiHeadAttention {
    /// Create a new multi-head attention instance
    ///
    /// # Arguments
    /// * `dim` - Embedding dimension
    /// * `num_heads` - Number of attention heads
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

    /// Get the number of heads
    #[wasm_bindgen(getter)]
    pub fn num_heads(&self) -> usize {
        self.inner.num_heads()
    }

    /// Get the dimension
    #[wasm_bindgen(getter)]
    pub fn dim(&self) -> usize {
        self.inner.dim()
    }
}

/// Hyperbolic attention mechanism
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
    /// * `curvature` - Hyperbolic curvature parameter
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

    /// Get the curvature
    #[wasm_bindgen(getter)]
    pub fn curvature(&self) -> f32 {
        self.curvature_value
    }
}

/// Linear attention (Performer-style)
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
    /// * `num_features` - Number of random features
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

/// Flash attention mechanism
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
    /// * `block_size` - Block size for tiling
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

/// Local-global attention mechanism
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

/// Mixture of Experts (MoE) attention
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
    /// * `top_k` - Number of experts to use per query
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
