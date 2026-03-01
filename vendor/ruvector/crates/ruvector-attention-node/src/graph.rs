//! NAPI-RS bindings for graph attention mechanisms
//!
//! Provides Node.js bindings for:
//! - Edge-featured attention (GATv2-style)
//! - Graph RoPE (Rotary Position Embeddings for graphs)
//! - Dual-space attention (Euclidean + Hyperbolic)

use napi::bindgen_prelude::*;
use napi_derive::napi;
use ruvector_attention::graph::{
    DualSpaceAttention as RustDualSpace, DualSpaceConfig as RustDualConfig,
    EdgeFeaturedAttention as RustEdgeFeatured, EdgeFeaturedConfig as RustEdgeConfig,
    GraphRoPE as RustGraphRoPE, RoPEConfig as RustRoPEConfig,
};
use ruvector_attention::traits::Attention;

// ============================================================================
// Edge-Featured Attention
// ============================================================================

/// Configuration for edge-featured attention
#[napi(object)]
pub struct EdgeFeaturedConfig {
    pub node_dim: u32,
    pub edge_dim: u32,
    pub num_heads: u32,
    pub concat_heads: Option<bool>,
    pub add_self_loops: Option<bool>,
    pub negative_slope: Option<f64>,
}

/// Edge-featured attention (GATv2-style)
#[napi]
pub struct EdgeFeaturedAttention {
    inner: RustEdgeFeatured,
    config: EdgeFeaturedConfig,
}

#[napi]
impl EdgeFeaturedAttention {
    /// Create a new edge-featured attention instance
    ///
    /// # Arguments
    /// * `config` - Edge-featured attention configuration
    #[napi(constructor)]
    pub fn new(config: EdgeFeaturedConfig) -> Self {
        let rust_config = RustEdgeConfig {
            node_dim: config.node_dim as usize,
            edge_dim: config.edge_dim as usize,
            num_heads: config.num_heads as usize,
            concat_heads: config.concat_heads.unwrap_or(true),
            add_self_loops: config.add_self_loops.unwrap_or(true),
            negative_slope: config.negative_slope.unwrap_or(0.2) as f32,
            dropout: 0.0,
        };
        Self {
            inner: RustEdgeFeatured::new(rust_config),
            config,
        }
    }

    /// Create with simple parameters
    #[napi(factory)]
    pub fn simple(node_dim: u32, edge_dim: u32, num_heads: u32) -> Self {
        Self::new(EdgeFeaturedConfig {
            node_dim,
            edge_dim,
            num_heads,
            concat_heads: Some(true),
            add_self_loops: Some(true),
            negative_slope: Some(0.2),
        })
    }

    /// Compute attention without edge features (standard attention)
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

    /// Compute attention with edge features
    ///
    /// # Arguments
    /// * `query` - Query vector
    /// * `keys` - Array of key vectors
    /// * `values` - Array of value vectors
    /// * `edge_features` - Array of edge feature vectors (same length as keys)
    #[napi]
    pub fn compute_with_edges(
        &self,
        query: Float32Array,
        keys: Vec<Float32Array>,
        values: Vec<Float32Array>,
        edge_features: Vec<Float32Array>,
    ) -> Result<Float32Array> {
        let query_slice = query.as_ref();
        let keys_vec: Vec<Vec<f32>> = keys.into_iter().map(|k| k.to_vec()).collect();
        let values_vec: Vec<Vec<f32>> = values.into_iter().map(|v| v.to_vec()).collect();
        let edge_features_vec: Vec<Vec<f32>> =
            edge_features.into_iter().map(|e| e.to_vec()).collect();

        let keys_refs: Vec<&[f32]> = keys_vec.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values_vec.iter().map(|v| v.as_slice()).collect();
        let edges_refs: Vec<&[f32]> = edge_features_vec.iter().map(|e| e.as_slice()).collect();

        let result = self
            .inner
            .compute_with_edges(query_slice, &keys_refs, &values_refs, &edges_refs)
            .map_err(|e| Error::from_reason(e.to_string()))?;

        Ok(Float32Array::new(result))
    }

    /// Get the node dimension
    #[napi(getter)]
    pub fn node_dim(&self) -> u32 {
        self.config.node_dim
    }

    /// Get the edge dimension
    #[napi(getter)]
    pub fn edge_dim(&self) -> u32 {
        self.config.edge_dim
    }

    /// Get the number of heads
    #[napi(getter)]
    pub fn num_heads(&self) -> u32 {
        self.config.num_heads
    }
}

// ============================================================================
// Graph RoPE Attention
// ============================================================================

/// Configuration for Graph RoPE attention
#[napi(object)]
pub struct RoPEConfig {
    pub dim: u32,
    pub max_position: u32,
    pub base: Option<f64>,
    pub scaling_factor: Option<f64>,
}

/// Graph RoPE attention (Rotary Position Embeddings for graphs)
#[napi]
pub struct GraphRoPEAttention {
    inner: RustGraphRoPE,
    config: RoPEConfig,
}

#[napi]
impl GraphRoPEAttention {
    /// Create a new Graph RoPE attention instance
    ///
    /// # Arguments
    /// * `config` - RoPE configuration
    #[napi(constructor)]
    pub fn new(config: RoPEConfig) -> Self {
        let rust_config = RustRoPEConfig {
            dim: config.dim as usize,
            max_position: config.max_position as usize,
            base: config.base.unwrap_or(10000.0) as f32,
            scaling_factor: config.scaling_factor.unwrap_or(1.0) as f32,
        };
        Self {
            inner: RustGraphRoPE::new(rust_config),
            config,
        }
    }

    /// Create with simple parameters
    #[napi(factory)]
    pub fn simple(dim: u32, max_position: u32) -> Self {
        Self::new(RoPEConfig {
            dim,
            max_position,
            base: Some(10000.0),
            scaling_factor: Some(1.0),
        })
    }

    /// Compute attention without positional encoding
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

    /// Compute attention with graph positions
    ///
    /// # Arguments
    /// * `query` - Query vector
    /// * `keys` - Array of key vectors
    /// * `values` - Array of value vectors
    /// * `query_position` - Position of query node
    /// * `key_positions` - Positions of key nodes (e.g., hop distances)
    #[napi]
    pub fn compute_with_positions(
        &self,
        query: Float32Array,
        keys: Vec<Float32Array>,
        values: Vec<Float32Array>,
        query_position: u32,
        key_positions: Vec<u32>,
    ) -> Result<Float32Array> {
        let query_slice = query.as_ref();
        let keys_vec: Vec<Vec<f32>> = keys.into_iter().map(|k| k.to_vec()).collect();
        let values_vec: Vec<Vec<f32>> = values.into_iter().map(|v| v.to_vec()).collect();
        let keys_refs: Vec<&[f32]> = keys_vec.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values_vec.iter().map(|v| v.as_slice()).collect();
        let positions_usize: Vec<usize> = key_positions.into_iter().map(|p| p as usize).collect();

        let result = self
            .inner
            .compute_with_positions(
                query_slice,
                &keys_refs,
                &values_refs,
                query_position as usize,
                &positions_usize,
            )
            .map_err(|e| Error::from_reason(e.to_string()))?;

        Ok(Float32Array::new(result))
    }

    /// Apply rotary embedding to a vector
    #[napi]
    pub fn apply_rotary(&self, vector: Float32Array, position: u32) -> Float32Array {
        let v = vector.as_ref();
        let result = self.inner.apply_rotary(v, position as usize);
        Float32Array::new(result)
    }

    /// Convert graph distance to position bucket
    #[napi]
    pub fn distance_to_position(distance: u32, max_distance: u32) -> u32 {
        RustGraphRoPE::distance_to_position(distance as usize, max_distance as usize) as u32
    }

    /// Get the dimension
    #[napi(getter)]
    pub fn dim(&self) -> u32 {
        self.config.dim
    }

    /// Get the max position
    #[napi(getter)]
    pub fn max_position(&self) -> u32 {
        self.config.max_position
    }
}

// ============================================================================
// Dual-Space Attention
// ============================================================================

/// Configuration for dual-space attention
#[napi(object)]
pub struct DualSpaceConfig {
    pub dim: u32,
    pub curvature: f64,
    pub euclidean_weight: f64,
    pub hyperbolic_weight: f64,
    pub temperature: Option<f64>,
}

/// Dual-space attention (Euclidean + Hyperbolic)
#[napi]
pub struct DualSpaceAttention {
    inner: RustDualSpace,
    config: DualSpaceConfig,
}

#[napi]
impl DualSpaceAttention {
    /// Create a new dual-space attention instance
    ///
    /// # Arguments
    /// * `config` - Dual-space configuration
    #[napi(constructor)]
    pub fn new(config: DualSpaceConfig) -> Self {
        let rust_config = RustDualConfig {
            dim: config.dim as usize,
            curvature: config.curvature as f32,
            euclidean_weight: config.euclidean_weight as f32,
            hyperbolic_weight: config.hyperbolic_weight as f32,
            learn_weights: false,
            temperature: config.temperature.unwrap_or(1.0) as f32,
        };
        Self {
            inner: RustDualSpace::new(rust_config),
            config,
        }
    }

    /// Create with simple parameters (equal weights)
    #[napi(factory)]
    pub fn simple(dim: u32, curvature: f64) -> Self {
        Self::new(DualSpaceConfig {
            dim,
            curvature,
            euclidean_weight: 0.5,
            hyperbolic_weight: 0.5,
            temperature: Some(1.0),
        })
    }

    /// Create with custom weights
    #[napi(factory)]
    pub fn with_weights(
        dim: u32,
        curvature: f64,
        euclidean_weight: f64,
        hyperbolic_weight: f64,
    ) -> Self {
        Self::new(DualSpaceConfig {
            dim,
            curvature,
            euclidean_weight,
            hyperbolic_weight,
            temperature: Some(1.0),
        })
    }

    /// Compute dual-space attention
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

    /// Get space contributions (Euclidean and Hyperbolic scores separately)
    #[napi]
    pub fn get_space_contributions(
        &self,
        query: Float32Array,
        keys: Vec<Float32Array>,
    ) -> SpaceContributions {
        let query_slice = query.as_ref();
        let keys_vec: Vec<Vec<f32>> = keys.into_iter().map(|k| k.to_vec()).collect();
        let keys_refs: Vec<&[f32]> = keys_vec.iter().map(|k| k.as_slice()).collect();

        let (euc_scores, hyp_scores) = self.inner.get_space_contributions(query_slice, &keys_refs);

        SpaceContributions {
            euclidean_scores: Float32Array::new(euc_scores),
            hyperbolic_scores: Float32Array::new(hyp_scores),
        }
    }

    /// Get the dimension
    #[napi(getter)]
    pub fn dim(&self) -> u32 {
        self.config.dim
    }

    /// Get the curvature
    #[napi(getter)]
    pub fn curvature(&self) -> f64 {
        self.config.curvature
    }

    /// Get the Euclidean weight
    #[napi(getter)]
    pub fn euclidean_weight(&self) -> f64 {
        self.config.euclidean_weight
    }

    /// Get the Hyperbolic weight
    #[napi(getter)]
    pub fn hyperbolic_weight(&self) -> f64 {
        self.config.hyperbolic_weight
    }
}

/// Space contribution scores
#[napi(object)]
pub struct SpaceContributions {
    pub euclidean_scores: Float32Array,
    pub hyperbolic_scores: Float32Array,
}
