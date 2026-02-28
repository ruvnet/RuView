//! Node.js bindings for Ruvector GNN via NAPI-RS
//!
//! This module provides JavaScript bindings for the Ruvector GNN library,
//! enabling graph neural network operations, tensor compression, and
//! differentiable search in Node.js applications.

#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi_derive::napi;
use ruvector_gnn::{
    compress::{
        CompressedTensor as RustCompressedTensor, CompressionLevel as RustCompressionLevel,
        TensorCompress as RustTensorCompress,
    },
    layer::RuvectorLayer as RustRuvectorLayer,
    search::{
        differentiable_search as rust_differentiable_search,
        hierarchical_forward as rust_hierarchical_forward,
    },
};

// ==================== RuvectorLayer Bindings ====================

/// Graph Neural Network layer for HNSW topology
#[napi]
pub struct RuvectorLayer {
    inner: RustRuvectorLayer,
}

#[napi]
impl RuvectorLayer {
    /// Create a new Ruvector GNN layer
    ///
    /// # Arguments
    /// * `input_dim` - Dimension of input node embeddings
    /// * `hidden_dim` - Dimension of hidden representations
    /// * `heads` - Number of attention heads
    /// * `dropout` - Dropout rate (0.0 to 1.0)
    ///
    /// # Example
    /// ```javascript
    /// const layer = new RuvectorLayer(128, 256, 4, 0.1);
    /// ```
    #[napi(constructor)]
    pub fn new(input_dim: u32, hidden_dim: u32, heads: u32, dropout: f64) -> Result<Self> {
        let inner = RustRuvectorLayer::new(
            input_dim as usize,
            hidden_dim as usize,
            heads as usize,
            dropout as f32,
        )
        .map_err(|e| Error::new(Status::InvalidArg, e.to_string()))?;

        Ok(Self { inner })
    }

    /// Forward pass through the GNN layer
    ///
    /// # Arguments
    /// * `node_embedding` - Current node's embedding (Float32Array)
    /// * `neighbor_embeddings` - Embeddings of neighbor nodes (Array of Float32Array)
    /// * `edge_weights` - Weights of edges to neighbors (Float32Array)
    ///
    /// # Returns
    /// Updated node embedding as Float32Array
    ///
    /// # Example
    /// ```javascript
    /// const node = new Float32Array([1.0, 2.0, 3.0, 4.0]);
    /// const neighbors = [new Float32Array([0.5, 1.0, 1.5, 2.0]), new Float32Array([2.0, 3.0, 4.0, 5.0])];
    /// const weights = new Float32Array([0.3, 0.7]);
    /// const output = layer.forward(node, neighbors, weights);
    /// ```
    #[napi]
    pub fn forward(
        &self,
        node_embedding: Float32Array,
        neighbor_embeddings: Vec<Float32Array>,
        edge_weights: Float32Array,
    ) -> Result<Float32Array> {
        let node_slice = node_embedding.as_ref();
        let neighbors_vec: Vec<Vec<f32>> = neighbor_embeddings
            .into_iter()
            .map(|arr| arr.to_vec())
            .collect();
        let weights_slice = edge_weights.as_ref();

        let result = self
            .inner
            .forward(node_slice, &neighbors_vec, weights_slice);

        Ok(Float32Array::new(result))
    }

    /// Serialize the layer to JSON
    #[napi]
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(&self.inner).map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Serialization error: {}", e),
            )
        })
    }

    /// Deserialize the layer from JSON
    #[napi(factory)]
    pub fn from_json(json: String) -> Result<Self> {
        let inner: RustRuvectorLayer = serde_json::from_str(&json).map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Deserialization error: {}", e),
            )
        })?;
        Ok(Self { inner })
    }
}

// ==================== TensorCompress Bindings ====================

/// Compression level for tensor compression
#[napi(object)]
pub struct CompressionLevelConfig {
    /// Type of compression: "none", "half", "pq8", "pq4", "binary"
    pub level_type: String,
    /// Scale factor (for "half" compression)
    pub scale: Option<f64>,
    /// Number of subvectors (for PQ compression)
    pub subvectors: Option<u32>,
    /// Number of centroids (for PQ8)
    pub centroids: Option<u32>,
    /// Outlier threshold (for PQ4)
    pub outlier_threshold: Option<f64>,
    /// Binary threshold (for binary compression)
    pub threshold: Option<f64>,
}

impl CompressionLevelConfig {
    fn to_rust(&self) -> Result<RustCompressionLevel> {
        match self.level_type.as_str() {
            "none" => Ok(RustCompressionLevel::None),
            "half" => Ok(RustCompressionLevel::Half {
                scale: self.scale.unwrap_or(1.0) as f32,
            }),
            "pq8" => Ok(RustCompressionLevel::PQ8 {
                subvectors: self.subvectors.unwrap_or(8) as u8,
                centroids: self.centroids.unwrap_or(16) as u8,
            }),
            "pq4" => Ok(RustCompressionLevel::PQ4 {
                subvectors: self.subvectors.unwrap_or(8) as u8,
                outlier_threshold: self.outlier_threshold.unwrap_or(3.0) as f32,
            }),
            "binary" => Ok(RustCompressionLevel::Binary {
                threshold: self.threshold.unwrap_or(0.0) as f32,
            }),
            _ => Err(Error::new(
                Status::InvalidArg,
                format!("Invalid compression level: {}", self.level_type),
            )),
        }
    }
}

/// Tensor compressor with adaptive level selection
#[napi]
pub struct TensorCompress {
    inner: RustTensorCompress,
}

#[napi]
impl TensorCompress {
    /// Create a new tensor compressor
    ///
    /// # Example
    /// ```javascript
    /// const compressor = new TensorCompress();
    /// ```
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            inner: RustTensorCompress::new(),
        }
    }

    /// Compress an embedding based on access frequency
    ///
    /// # Arguments
    /// * `embedding` - The input embedding vector (Float32Array)
    /// * `access_freq` - Access frequency in range [0.0, 1.0]
    ///
    /// # Returns
    /// Compressed tensor as JSON string
    ///
    /// # Example
    /// ```javascript
    /// const embedding = new Float32Array([1.0, 2.0, 3.0, 4.0]);
    /// const compressed = compressor.compress(embedding, 0.5);
    /// ```
    #[napi]
    pub fn compress(&self, embedding: Float32Array, access_freq: f64) -> Result<String> {
        let embedding_slice = embedding.as_ref();

        let compressed = self
            .inner
            .compress(embedding_slice, access_freq as f32)
            .map_err(|e| Error::new(Status::GenericFailure, format!("Compression error: {}", e)))?;

        serde_json::to_string(&compressed).map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Serialization error: {}", e),
            )
        })
    }

    /// Compress with explicit compression level
    ///
    /// # Arguments
    /// * `embedding` - The input embedding vector (Float32Array)
    /// * `level` - Compression level configuration
    ///
    /// # Returns
    /// Compressed tensor as JSON string
    ///
    /// # Example
    /// ```javascript
    /// const embedding = new Float32Array([1.0, 2.0, 3.0, 4.0]);
    /// const level = { level_type: "half", scale: 1.0 };
    /// const compressed = compressor.compressWithLevel(embedding, level);
    /// ```
    #[napi]
    pub fn compress_with_level(
        &self,
        embedding: Float32Array,
        level: CompressionLevelConfig,
    ) -> Result<String> {
        let embedding_slice = embedding.as_ref();
        let rust_level = level.to_rust()?;

        let compressed = self
            .inner
            .compress_with_level(embedding_slice, &rust_level)
            .map_err(|e| Error::new(Status::GenericFailure, format!("Compression error: {}", e)))?;

        serde_json::to_string(&compressed).map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Serialization error: {}", e),
            )
        })
    }

    /// Decompress a compressed tensor
    ///
    /// # Arguments
    /// * `compressed_json` - Compressed tensor as JSON string
    ///
    /// # Returns
    /// Decompressed embedding vector as Float32Array
    ///
    /// # Example
    /// ```javascript
    /// const decompressed = compressor.decompress(compressed);
    /// ```
    #[napi]
    pub fn decompress(&self, compressed_json: String) -> Result<Float32Array> {
        let compressed: RustCompressedTensor =
            serde_json::from_str(&compressed_json).map_err(|e| {
                Error::new(
                    Status::GenericFailure,
                    format!("Deserialization error: {}", e),
                )
            })?;

        let result = self.inner.decompress(&compressed).map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Decompression error: {}", e),
            )
        })?;

        Ok(Float32Array::new(result))
    }
}

// ==================== Search Functions ====================

/// Result from differentiable search
#[napi(object)]
pub struct SearchResult {
    /// Indices of top-k candidates
    pub indices: Vec<u32>,
    /// Soft weights for top-k candidates
    pub weights: Vec<f64>,
}

/// Differentiable search using soft attention mechanism
///
/// # Arguments
/// * `query` - The query vector (Float32Array)
/// * `candidate_embeddings` - List of candidate embedding vectors (Array of Float32Array)
/// * `k` - Number of top results to return
/// * `temperature` - Temperature for softmax (lower = sharper, higher = smoother)
///
/// # Returns
/// Search result with indices and soft weights
///
/// # Example
/// ```javascript
/// const query = new Float32Array([1.0, 0.0, 0.0]);
/// const candidates = [new Float32Array([1.0, 0.0, 0.0]), new Float32Array([0.9, 0.1, 0.0]), new Float32Array([0.0, 1.0, 0.0])];
/// const result = differentiableSearch(query, candidates, 2, 1.0);
/// console.log(result.indices); // [0, 1]
/// console.log(result.weights); // [0.x, 0.y]
/// ```
#[napi]
pub fn differentiable_search(
    query: Float32Array,
    candidate_embeddings: Vec<Float32Array>,
    k: u32,
    temperature: f64,
) -> Result<SearchResult> {
    let query_slice = query.as_ref();
    let candidates_vec: Vec<Vec<f32>> = candidate_embeddings
        .into_iter()
        .map(|arr| arr.to_vec())
        .collect();

    let (indices, weights) =
        rust_differentiable_search(query_slice, &candidates_vec, k as usize, temperature as f32);

    Ok(SearchResult {
        indices: indices.iter().map(|&i| i as u32).collect(),
        weights: weights.iter().map(|&w| w as f64).collect(),
    })
}

/// Hierarchical forward pass through GNN layers
///
/// # Arguments
/// * `query` - The query vector (Float32Array)
/// * `layer_embeddings` - Embeddings organized by layer (Array of Array of Float32Array)
/// * `gnn_layers_json` - JSON array of serialized GNN layers
///
/// # Returns
/// Final embedding after hierarchical processing as Float32Array
///
/// # Example
/// ```javascript
/// const query = new Float32Array([1.0, 0.0]);
/// const layerEmbeddings = [[new Float32Array([1.0, 0.0]), new Float32Array([0.0, 1.0])]];
/// const layer1 = new RuvectorLayer(2, 2, 1, 0.0);
/// const layers = [layer1.toJson()];
/// const result = hierarchicalForward(query, layerEmbeddings, layers);
/// ```
#[napi]
pub fn hierarchical_forward(
    query: Float32Array,
    layer_embeddings: Vec<Vec<Float32Array>>,
    gnn_layers_json: Vec<String>,
) -> Result<Float32Array> {
    let query_slice = query.as_ref();

    let embeddings_f32: Vec<Vec<Vec<f32>>> = layer_embeddings
        .into_iter()
        .map(|layer| layer.into_iter().map(|arr| arr.to_vec()).collect())
        .collect();

    let gnn_layers: Vec<RustRuvectorLayer> = gnn_layers_json
        .iter()
        .map(|json| {
            serde_json::from_str(json).map_err(|e| {
                Error::new(
                    Status::GenericFailure,
                    format!("Layer deserialization error: {}", e),
                )
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let result = rust_hierarchical_forward(query_slice, &embeddings_f32, &gnn_layers);

    Ok(Float32Array::new(result))
}

// ==================== Helper Functions ====================

/// Get the compression level that would be selected for a given access frequency
///
/// # Arguments
/// * `access_freq` - Access frequency in range [0.0, 1.0]
///
/// # Returns
/// String describing the compression level: "none", "half", "pq8", "pq4", or "binary"
///
/// # Example
/// ```javascript
/// const level = getCompressionLevel(0.9); // "none" (hot data)
/// const level2 = getCompressionLevel(0.5); // "half" (warm data)
/// ```
#[napi]
pub fn get_compression_level(access_freq: f64) -> String {
    if access_freq > 0.8 {
        "none".to_string()
    } else if access_freq > 0.4 {
        "half".to_string()
    } else if access_freq > 0.1 {
        "pq8".to_string()
    } else if access_freq > 0.01 {
        "pq4".to_string()
    } else {
        "binary".to_string()
    }
}

/// Module initialization
#[napi]
pub fn init() -> String {
    "Ruvector GNN Node.js bindings initialized".to_string()
}
