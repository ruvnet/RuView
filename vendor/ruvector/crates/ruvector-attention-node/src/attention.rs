//! NAPI-RS bindings for attention mechanisms
//!
//! Provides Node.js bindings for all attention variants:
//! - Scaled dot-product attention
//! - Multi-head attention
//! - Hyperbolic attention
//! - Flash attention
//! - Linear attention
//! - Local-global attention
//! - Mixture of Experts attention

use napi::bindgen_prelude::*;
use napi_derive::napi;
use ruvector_attention::{
    attention::{MultiHeadAttention as RustMultiHead, ScaledDotProductAttention},
    hyperbolic::{HyperbolicAttention as RustHyperbolic, HyperbolicAttentionConfig},
    moe::{MoEAttention as RustMoE, MoEConfig as RustMoEConfig},
    sparse::{
        FlashAttention as RustFlash, LinearAttention as RustLinear,
        LocalGlobalAttention as RustLocalGlobal,
    },
    traits::Attention,
};

/// Attention configuration object
#[napi(object)]
pub struct AttentionConfig {
    pub dim: u32,
    pub num_heads: Option<u32>,
    pub dropout: Option<f64>,
    pub scale: Option<f64>,
    pub causal: Option<bool>,
}

/// Scaled dot-product attention
#[napi]
pub struct DotProductAttention {
    inner: ScaledDotProductAttention,
}

#[napi]
impl DotProductAttention {
    /// Create a new scaled dot-product attention instance
    ///
    /// # Arguments
    /// * `dim` - Embedding dimension
    #[napi(constructor)]
    pub fn new(dim: u32) -> Result<Self> {
        Ok(Self {
            inner: ScaledDotProductAttention::new(dim as usize),
        })
    }

    /// Compute attention output
    ///
    /// # Arguments
    /// * `query` - Query vector
    /// * `keys` - Array of key vectors
    /// * `values` - Array of value vectors
    #[napi]
    pub fn compute(
        &self,
        query: Float32Array,
        keys: Vec<Float32Array>,
        values: Vec<Float32Array>,
    ) -> Result<Float32Array> {
        let query_slice = query.as_ref();
        let keys_vec: Vec<Vec<f32>> = keys.into_iter().map(|k| k.to_vec()).collect();
        let values_vec: Vec<Vec<f32>> = values.into_iter().map(|v| v.to_vec()).collect();
        let keys_refs: Vec<&[f32]> = keys_vec.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values_vec.iter().map(|v| v.as_slice()).collect();

        let result = self
            .inner
            .compute(query_slice, &keys_refs, &values_refs)
            .map_err(|e| Error::from_reason(e.to_string()))?;

        Ok(Float32Array::new(result))
    }

    /// Compute attention with mask
    ///
    /// # Arguments
    /// * `query` - Query vector
    /// * `keys` - Array of key vectors
    /// * `values` - Array of value vectors
    /// * `mask` - Boolean mask array (true = attend, false = mask)
    #[napi]
    pub fn compute_with_mask(
        &self,
        query: Float32Array,
        keys: Vec<Float32Array>,
        values: Vec<Float32Array>,
        mask: Vec<bool>,
    ) -> Result<Float32Array> {
        let query_slice = query.as_ref();
        let keys_vec: Vec<Vec<f32>> = keys.into_iter().map(|k| k.to_vec()).collect();
        let values_vec: Vec<Vec<f32>> = values.into_iter().map(|v| v.to_vec()).collect();
        let keys_refs: Vec<&[f32]> = keys_vec.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values_vec.iter().map(|v| v.as_slice()).collect();

        let result = self
            .inner
            .compute_with_mask(query_slice, &keys_refs, &values_refs, Some(mask.as_slice()))
            .map_err(|e| Error::from_reason(e.to_string()))?;

        Ok(Float32Array::new(result))
    }

    /// Get the dimension
    #[napi(getter)]
    pub fn dim(&self) -> u32 {
        self.inner.dim() as u32
    }
}

/// Multi-head attention mechanism
#[napi]
pub struct MultiHeadAttention {
    inner: RustMultiHead,
    dim_value: usize,
    num_heads_value: usize,
}

#[napi]
impl MultiHeadAttention {
    /// Create a new multi-head attention instance
    ///
    /// # Arguments
    /// * `dim` - Embedding dimension (must be divisible by num_heads)
    /// * `num_heads` - Number of attention heads
    #[napi(constructor)]
    pub fn new(dim: u32, num_heads: u32) -> Result<Self> {
        let d = dim as usize;
        let h = num_heads as usize;

        if d % h != 0 {
            return Err(Error::from_reason(format!(
                "Dimension {} must be divisible by number of heads {}",
                d, h
            )));
        }

        Ok(Self {
            inner: RustMultiHead::new(d, h),
            dim_value: d,
            num_heads_value: h,
        })
    }

    /// Compute multi-head attention
    #[napi]
    pub fn compute(
        &self,
        query: Float32Array,
        keys: Vec<Float32Array>,
        values: Vec<Float32Array>,
    ) -> Result<Float32Array> {
        let query_slice = query.as_ref();
        let keys_vec: Vec<Vec<f32>> = keys.into_iter().map(|k| k.to_vec()).collect();
        let values_vec: Vec<Vec<f32>> = values.into_iter().map(|v| v.to_vec()).collect();
        let keys_refs: Vec<&[f32]> = keys_vec.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values_vec.iter().map(|v| v.as_slice()).collect();

        let result = self
            .inner
            .compute(query_slice, &keys_refs, &values_refs)
            .map_err(|e| Error::from_reason(e.to_string()))?;

        Ok(Float32Array::new(result))
    }

    /// Get the number of heads
    #[napi(getter)]
    pub fn num_heads(&self) -> u32 {
        self.num_heads_value as u32
    }

    /// Get the dimension
    #[napi(getter)]
    pub fn dim(&self) -> u32 {
        self.dim_value as u32
    }

    /// Get the head dimension
    #[napi(getter)]
    pub fn head_dim(&self) -> u32 {
        (self.dim_value / self.num_heads_value) as u32
    }
}

/// Hyperbolic attention in Poincaré ball model
#[napi]
pub struct HyperbolicAttention {
    inner: RustHyperbolic,
    curvature_value: f32,
    dim_value: usize,
}

#[napi]
impl HyperbolicAttention {
    /// Create a new hyperbolic attention instance
    ///
    /// # Arguments
    /// * `dim` - Embedding dimension
    /// * `curvature` - Hyperbolic curvature (typically 1.0)
    #[napi(constructor)]
    pub fn new(dim: u32, curvature: f64) -> Self {
        let config = HyperbolicAttentionConfig {
            dim: dim as usize,
            curvature: curvature as f32,
            ..Default::default()
        };
        Self {
            inner: RustHyperbolic::new(config),
            curvature_value: curvature as f32,
            dim_value: dim as usize,
        }
    }

    /// Create with full configuration
    ///
    /// # Arguments
    /// * `dim` - Embedding dimension
    /// * `curvature` - Hyperbolic curvature
    /// * `adaptive_curvature` - Whether to use adaptive curvature
    /// * `temperature` - Temperature for softmax
    #[napi(factory)]
    pub fn with_config(
        dim: u32,
        curvature: f64,
        adaptive_curvature: bool,
        temperature: f64,
    ) -> Self {
        let config = HyperbolicAttentionConfig {
            dim: dim as usize,
            curvature: curvature as f32,
            adaptive_curvature,
            temperature: temperature as f32,
            frechet_max_iter: 100,
            frechet_tol: 1e-6,
        };
        Self {
            inner: RustHyperbolic::new(config),
            curvature_value: curvature as f32,
            dim_value: dim as usize,
        }
    }

    /// Compute hyperbolic attention
    #[napi]
    pub fn compute(
        &self,
        query: Float32Array,
        keys: Vec<Float32Array>,
        values: Vec<Float32Array>,
    ) -> Result<Float32Array> {
        let query_slice = query.as_ref();
        let keys_vec: Vec<Vec<f32>> = keys.into_iter().map(|k| k.to_vec()).collect();
        let values_vec: Vec<Vec<f32>> = values.into_iter().map(|v| v.to_vec()).collect();
        let keys_refs: Vec<&[f32]> = keys_vec.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values_vec.iter().map(|v| v.as_slice()).collect();

        let result = self
            .inner
            .compute(query_slice, &keys_refs, &values_refs)
            .map_err(|e| Error::from_reason(e.to_string()))?;

        Ok(Float32Array::new(result))
    }

    /// Get the curvature
    #[napi(getter)]
    pub fn curvature(&self) -> f64 {
        self.curvature_value as f64
    }

    /// Get the dimension
    #[napi(getter)]
    pub fn dim(&self) -> u32 {
        self.dim_value as u32
    }
}

/// Flash attention with tiled computation
#[napi]
pub struct FlashAttention {
    inner: RustFlash,
    dim_value: usize,
    block_size_value: usize,
}

#[napi]
impl FlashAttention {
    /// Create a new flash attention instance
    ///
    /// # Arguments
    /// * `dim` - Embedding dimension
    /// * `block_size` - Block size for tiled computation
    #[napi(constructor)]
    pub fn new(dim: u32, block_size: u32) -> Self {
        Self {
            inner: RustFlash::new(dim as usize, block_size as usize),
            dim_value: dim as usize,
            block_size_value: block_size as usize,
        }
    }

    /// Compute flash attention
    #[napi]
    pub fn compute(
        &self,
        query: Float32Array,
        keys: Vec<Float32Array>,
        values: Vec<Float32Array>,
    ) -> Result<Float32Array> {
        let query_slice = query.as_ref();
        let keys_vec: Vec<Vec<f32>> = keys.into_iter().map(|k| k.to_vec()).collect();
        let values_vec: Vec<Vec<f32>> = values.into_iter().map(|v| v.to_vec()).collect();
        let keys_refs: Vec<&[f32]> = keys_vec.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values_vec.iter().map(|v| v.as_slice()).collect();

        let result = self
            .inner
            .compute(query_slice, &keys_refs, &values_refs)
            .map_err(|e| Error::from_reason(e.to_string()))?;

        Ok(Float32Array::new(result))
    }

    /// Get the dimension
    #[napi(getter)]
    pub fn dim(&self) -> u32 {
        self.dim_value as u32
    }

    /// Get the block size
    #[napi(getter)]
    pub fn block_size(&self) -> u32 {
        self.block_size_value as u32
    }
}

/// Linear attention (Performer-style) with O(n) complexity
#[napi]
pub struct LinearAttention {
    inner: RustLinear,
    dim_value: usize,
    num_features_value: usize,
}

#[napi]
impl LinearAttention {
    /// Create a new linear attention instance
    ///
    /// # Arguments
    /// * `dim` - Embedding dimension
    /// * `num_features` - Number of random features
    #[napi(constructor)]
    pub fn new(dim: u32, num_features: u32) -> Self {
        Self {
            inner: RustLinear::new(dim as usize, num_features as usize),
            dim_value: dim as usize,
            num_features_value: num_features as usize,
        }
    }

    /// Compute linear attention
    #[napi]
    pub fn compute(
        &self,
        query: Float32Array,
        keys: Vec<Float32Array>,
        values: Vec<Float32Array>,
    ) -> Result<Float32Array> {
        let query_slice = query.as_ref();
        let keys_vec: Vec<Vec<f32>> = keys.into_iter().map(|k| k.to_vec()).collect();
        let values_vec: Vec<Vec<f32>> = values.into_iter().map(|v| v.to_vec()).collect();
        let keys_refs: Vec<&[f32]> = keys_vec.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values_vec.iter().map(|v| v.as_slice()).collect();

        let result = self
            .inner
            .compute(query_slice, &keys_refs, &values_refs)
            .map_err(|e| Error::from_reason(e.to_string()))?;

        Ok(Float32Array::new(result))
    }

    /// Get the dimension
    #[napi(getter)]
    pub fn dim(&self) -> u32 {
        self.dim_value as u32
    }

    /// Get the number of random features
    #[napi(getter)]
    pub fn num_features(&self) -> u32 {
        self.num_features_value as u32
    }
}

/// Local-global attention (Longformer-style)
#[napi]
pub struct LocalGlobalAttention {
    inner: RustLocalGlobal,
    dim_value: usize,
    local_window_value: usize,
    global_tokens_value: usize,
}

#[napi]
impl LocalGlobalAttention {
    /// Create a new local-global attention instance
    ///
    /// # Arguments
    /// * `dim` - Embedding dimension
    /// * `local_window` - Size of local attention window
    /// * `global_tokens` - Number of global attention tokens
    #[napi(constructor)]
    pub fn new(dim: u32, local_window: u32, global_tokens: u32) -> Self {
        Self {
            inner: RustLocalGlobal::new(
                dim as usize,
                local_window as usize,
                global_tokens as usize,
            ),
            dim_value: dim as usize,
            local_window_value: local_window as usize,
            global_tokens_value: global_tokens as usize,
        }
    }

    /// Compute local-global attention
    #[napi]
    pub fn compute(
        &self,
        query: Float32Array,
        keys: Vec<Float32Array>,
        values: Vec<Float32Array>,
    ) -> Result<Float32Array> {
        let query_slice = query.as_ref();
        let keys_vec: Vec<Vec<f32>> = keys.into_iter().map(|k| k.to_vec()).collect();
        let values_vec: Vec<Vec<f32>> = values.into_iter().map(|v| v.to_vec()).collect();
        let keys_refs: Vec<&[f32]> = keys_vec.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values_vec.iter().map(|v| v.as_slice()).collect();

        let result = self
            .inner
            .compute(query_slice, &keys_refs, &values_refs)
            .map_err(|e| Error::from_reason(e.to_string()))?;

        Ok(Float32Array::new(result))
    }

    /// Get the dimension
    #[napi(getter)]
    pub fn dim(&self) -> u32 {
        self.dim_value as u32
    }

    /// Get the local window size
    #[napi(getter)]
    pub fn local_window(&self) -> u32 {
        self.local_window_value as u32
    }

    /// Get the number of global tokens
    #[napi(getter)]
    pub fn global_tokens(&self) -> u32 {
        self.global_tokens_value as u32
    }
}

/// MoE attention configuration
#[napi(object)]
pub struct MoEConfig {
    pub dim: u32,
    pub num_experts: u32,
    pub top_k: u32,
    pub expert_capacity: Option<f64>,
}

/// Mixture of Experts attention
#[napi]
pub struct MoEAttention {
    inner: RustMoE,
    config: MoEConfig,
}

#[napi]
impl MoEAttention {
    /// Create a new MoE attention instance
    ///
    /// # Arguments
    /// * `config` - MoE configuration object
    #[napi(constructor)]
    pub fn new(config: MoEConfig) -> Self {
        let rust_config = RustMoEConfig::builder()
            .dim(config.dim as usize)
            .num_experts(config.num_experts as usize)
            .top_k(config.top_k as usize)
            .expert_capacity(config.expert_capacity.unwrap_or(1.25) as f32)
            .build();

        Self {
            inner: RustMoE::new(rust_config),
            config,
        }
    }

    /// Create with simple parameters
    ///
    /// # Arguments
    /// * `dim` - Embedding dimension
    /// * `num_experts` - Number of expert networks
    /// * `top_k` - Number of experts to route to
    #[napi(factory)]
    pub fn simple(dim: u32, num_experts: u32, top_k: u32) -> Self {
        let config = MoEConfig {
            dim,
            num_experts,
            top_k,
            expert_capacity: Some(1.25),
        };
        Self::new(config)
    }

    /// Compute MoE attention
    #[napi]
    pub fn compute(
        &self,
        query: Float32Array,
        keys: Vec<Float32Array>,
        values: Vec<Float32Array>,
    ) -> Result<Float32Array> {
        let query_slice = query.as_ref();
        let keys_vec: Vec<Vec<f32>> = keys.into_iter().map(|k| k.to_vec()).collect();
        let values_vec: Vec<Vec<f32>> = values.into_iter().map(|v| v.to_vec()).collect();
        let keys_refs: Vec<&[f32]> = keys_vec.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values_vec.iter().map(|v| v.as_slice()).collect();

        let result = self
            .inner
            .compute(query_slice, &keys_refs, &values_refs)
            .map_err(|e| Error::from_reason(e.to_string()))?;

        Ok(Float32Array::new(result))
    }

    /// Get the dimension
    #[napi(getter)]
    pub fn dim(&self) -> u32 {
        self.config.dim
    }

    /// Get the number of experts
    #[napi(getter)]
    pub fn num_experts(&self) -> u32 {
        self.config.num_experts
    }

    /// Get the top-k value
    #[napi(getter)]
    pub fn top_k(&self) -> u32 {
        self.config.top_k
    }
}

// Utility functions

/// Project a vector into the Poincaré ball
#[napi]
pub fn project_to_poincare_ball(vector: Float32Array, curvature: f64) -> Float32Array {
    let v = vector.to_vec();
    let projected = ruvector_attention::hyperbolic::project_to_ball(&v, curvature as f32, 1e-5);
    Float32Array::new(projected)
}

/// Compute hyperbolic (Poincaré) distance between two points
#[napi]
pub fn poincare_distance(a: Float32Array, b: Float32Array, curvature: f64) -> f64 {
    let a_slice = a.as_ref();
    let b_slice = b.as_ref();
    ruvector_attention::hyperbolic::poincare_distance(a_slice, b_slice, curvature as f32) as f64
}

/// Möbius addition in hyperbolic space
#[napi]
pub fn mobius_addition(a: Float32Array, b: Float32Array, curvature: f64) -> Float32Array {
    let a_slice = a.as_ref();
    let b_slice = b.as_ref();
    let result = ruvector_attention::hyperbolic::mobius_add(a_slice, b_slice, curvature as f32);
    Float32Array::new(result)
}

/// Exponential map from tangent space to hyperbolic space
#[napi]
pub fn exp_map(base: Float32Array, tangent: Float32Array, curvature: f64) -> Float32Array {
    let base_slice = base.as_ref();
    let tangent_slice = tangent.as_ref();
    let result =
        ruvector_attention::hyperbolic::exp_map(base_slice, tangent_slice, curvature as f32);
    Float32Array::new(result)
}

/// Logarithmic map from hyperbolic space to tangent space
#[napi]
pub fn log_map(base: Float32Array, point: Float32Array, curvature: f64) -> Float32Array {
    let base_slice = base.as_ref();
    let point_slice = point.as_ref();
    let result = ruvector_attention::hyperbolic::log_map(base_slice, point_slice, curvature as f32);
    Float32Array::new(result)
}
