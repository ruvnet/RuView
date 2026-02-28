//! WebAssembly bindings for RuVector GNN
//!
//! This module provides high-performance browser bindings for Graph Neural Network
//! operations on HNSW topology, including:
//! - GNN layer forward passes
//! - Tensor compression with adaptive level selection
//! - Differentiable search with soft attention
//! - Hierarchical forward propagation

use ruvector_gnn::{
    differentiable_search as core_differentiable_search,
    hierarchical_forward as core_hierarchical_forward, CompressedTensor, CompressionLevel,
    RuvectorLayer, TensorCompress,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// Initialize panic hook for better error messages
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

// ============================================================================
// Type Definitions for WASM
// ============================================================================

/// Query configuration for differentiable search
#[derive(Debug, Clone, Serialize, Deserialize)]
#[wasm_bindgen]
pub struct SearchConfig {
    /// Number of top results to return
    pub k: usize,
    /// Temperature for softmax (lower = sharper, higher = smoother)
    pub temperature: f32,
}

#[wasm_bindgen]
impl SearchConfig {
    /// Create a new search configuration
    #[wasm_bindgen(constructor)]
    pub fn new(k: usize, temperature: f32) -> Self {
        Self { k, temperature }
    }
}

/// Search results with indices and weights (internal)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SearchResultInternal {
    /// Indices of top-k candidates
    indices: Vec<usize>,
    /// Soft weights for each result
    weights: Vec<f32>,
}

// ============================================================================
// JsRuvectorLayer - GNN Layer Wrapper
// ============================================================================

/// Graph Neural Network layer for HNSW topology
#[wasm_bindgen]
pub struct JsRuvectorLayer {
    inner: RuvectorLayer,
    hidden_dim: usize,
}

#[wasm_bindgen]
impl JsRuvectorLayer {
    /// Create a new GNN layer
    ///
    /// # Arguments
    /// * `input_dim` - Dimension of input node embeddings
    /// * `hidden_dim` - Dimension of hidden representations
    /// * `heads` - Number of attention heads
    /// * `dropout` - Dropout rate (0.0 to 1.0)
    #[wasm_bindgen(constructor)]
    pub fn new(
        input_dim: usize,
        hidden_dim: usize,
        heads: usize,
        dropout: f32,
    ) -> Result<JsRuvectorLayer, JsValue> {
        let inner = RuvectorLayer::new(input_dim, hidden_dim, heads, dropout)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        Ok(JsRuvectorLayer { inner, hidden_dim })
    }

    /// Forward pass through the GNN layer
    ///
    /// # Arguments
    /// * `node_embedding` - Current node's embedding (Float32Array)
    /// * `neighbor_embeddings` - Embeddings of neighbor nodes (array of Float32Arrays)
    /// * `edge_weights` - Weights of edges to neighbors (Float32Array)
    ///
    /// # Returns
    /// Updated node embedding (Float32Array)
    #[wasm_bindgen]
    pub fn forward(
        &self,
        node_embedding: Vec<f32>,
        neighbor_embeddings: JsValue,
        edge_weights: Vec<f32>,
    ) -> Result<Vec<f32>, JsValue> {
        // Convert neighbor embeddings from JS value
        let neighbors: Vec<Vec<f32>> = serde_wasm_bindgen::from_value(neighbor_embeddings)
            .map_err(|e| {
                JsValue::from_str(&format!("Failed to parse neighbor embeddings: {}", e))
            })?;

        // Validate inputs
        if neighbors.len() != edge_weights.len() {
            return Err(JsValue::from_str(&format!(
                "Number of neighbors ({}) must match number of edge weights ({})",
                neighbors.len(),
                edge_weights.len()
            )));
        }

        // Call core forward
        let result = self
            .inner
            .forward(&node_embedding, &neighbors, &edge_weights);

        Ok(result)
    }

    /// Get the output dimension of this layer
    #[wasm_bindgen(getter, js_name = outputDim)]
    pub fn output_dim(&self) -> usize {
        self.hidden_dim
    }
}

// ============================================================================
// JsTensorCompress - Tensor Compression Wrapper
// ============================================================================

/// Tensor compressor with adaptive level selection
#[wasm_bindgen]
pub struct JsTensorCompress {
    inner: TensorCompress,
}

#[wasm_bindgen]
impl JsTensorCompress {
    /// Create a new tensor compressor
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: TensorCompress::new(),
        }
    }

    /// Compress an embedding based on access frequency
    ///
    /// # Arguments
    /// * `embedding` - The input embedding vector (Float32Array)
    /// * `access_freq` - Access frequency in range [0.0, 1.0]
    ///   - f > 0.8: Full precision (hot data)
    ///   - f > 0.4: Half precision (warm data)
    ///   - f > 0.1: 8-bit PQ (cool data)
    ///   - f > 0.01: 4-bit PQ (cold data)
    ///   - f <= 0.01: Binary (archive)
    ///
    /// # Returns
    /// Compressed tensor as JsValue
    #[wasm_bindgen]
    pub fn compress(&self, embedding: Vec<f32>, access_freq: f32) -> Result<JsValue, JsValue> {
        let compressed = self
            .inner
            .compress(&embedding, access_freq)
            .map_err(|e| JsValue::from_str(&format!("Compression failed: {}", e)))?;

        // Serialize using serde_wasm_bindgen
        serde_wasm_bindgen::to_value(&compressed)
            .map_err(|e| JsValue::from_str(&format!("Serialization failed: {}", e)))
    }

    /// Compress with explicit compression level
    ///
    /// # Arguments
    /// * `embedding` - The input embedding vector
    /// * `level` - Compression level ("none", "half", "pq8", "pq4", "binary")
    ///
    /// # Returns
    /// Compressed tensor as JsValue
    #[wasm_bindgen(js_name = compressWithLevel)]
    pub fn compress_with_level(
        &self,
        embedding: Vec<f32>,
        level: &str,
    ) -> Result<JsValue, JsValue> {
        let compression_level = match level {
            "none" => CompressionLevel::None,
            "half" => CompressionLevel::Half { scale: 1.0 },
            "pq8" => CompressionLevel::PQ8 {
                subvectors: 8,
                centroids: 16,
            },
            "pq4" => CompressionLevel::PQ4 {
                subvectors: 8,
                outlier_threshold: 3.0,
            },
            "binary" => CompressionLevel::Binary { threshold: 0.0 },
            _ => {
                return Err(JsValue::from_str(&format!(
                    "Unknown compression level: {}",
                    level
                )))
            }
        };

        let compressed = self
            .inner
            .compress_with_level(&embedding, &compression_level)
            .map_err(|e| JsValue::from_str(&format!("Compression failed: {}", e)))?;

        // Serialize using serde_wasm_bindgen
        serde_wasm_bindgen::to_value(&compressed)
            .map_err(|e| JsValue::from_str(&format!("Serialization failed: {}", e)))
    }

    /// Decompress a compressed tensor
    ///
    /// # Arguments
    /// * `compressed` - Serialized compressed tensor (JsValue)
    ///
    /// # Returns
    /// Decompressed embedding vector (Float32Array)
    #[wasm_bindgen]
    pub fn decompress(&self, compressed: JsValue) -> Result<Vec<f32>, JsValue> {
        let compressed_tensor: CompressedTensor = serde_wasm_bindgen::from_value(compressed)
            .map_err(|e| JsValue::from_str(&format!("Deserialization failed: {}", e)))?;

        let decompressed = self
            .inner
            .decompress(&compressed_tensor)
            .map_err(|e| JsValue::from_str(&format!("Decompression failed: {}", e)))?;

        Ok(decompressed)
    }

    /// Get compression ratio estimate for a given access frequency
    ///
    /// # Arguments
    /// * `access_freq` - Access frequency in range [0.0, 1.0]
    ///
    /// # Returns
    /// Estimated compression ratio (original_size / compressed_size)
    #[wasm_bindgen(js_name = getCompressionRatio)]
    pub fn get_compression_ratio(&self, access_freq: f32) -> f32 {
        if access_freq > 0.8 {
            1.0 // No compression
        } else if access_freq > 0.4 {
            2.0 // Half precision
        } else if access_freq > 0.1 {
            4.0 // 8-bit PQ
        } else if access_freq > 0.01 {
            8.0 // 4-bit PQ
        } else {
            32.0 // Binary
        }
    }
}

// ============================================================================
// Standalone Functions
// ============================================================================

/// Differentiable search using soft attention mechanism
///
/// # Arguments
/// * `query` - The query vector (Float32Array)
/// * `candidate_embeddings` - List of candidate embedding vectors (array of Float32Arrays)
/// * `config` - Search configuration (k and temperature)
///
/// # Returns
/// Object with indices and weights for top-k candidates
#[wasm_bindgen(js_name = differentiableSearch)]
pub fn differentiable_search(
    query: Vec<f32>,
    candidate_embeddings: JsValue,
    config: &SearchConfig,
) -> Result<JsValue, JsValue> {
    // Convert candidate embeddings from JS value
    let candidates: Vec<Vec<f32>> = serde_wasm_bindgen::from_value(candidate_embeddings)
        .map_err(|e| JsValue::from_str(&format!("Failed to parse candidate embeddings: {}", e)))?;

    // Call core search function
    let (indices, weights) =
        core_differentiable_search(&query, &candidates, config.k, config.temperature);

    let result = SearchResultInternal { indices, weights };
    serde_wasm_bindgen::to_value(&result)
        .map_err(|e| JsValue::from_str(&format!("Failed to serialize result: {}", e)))
}

/// Hierarchical forward pass through multiple GNN layers
///
/// # Arguments
/// * `query` - The query vector (Float32Array)
/// * `layer_embeddings` - Embeddings organized by layer (array of arrays of Float32Arrays)
/// * `gnn_layers` - Array of GNN layers to process through
///
/// # Returns
/// Final embedding after hierarchical processing (Float32Array)
#[wasm_bindgen(js_name = hierarchicalForward)]
pub fn hierarchical_forward(
    query: Vec<f32>,
    layer_embeddings: JsValue,
    gnn_layers: Vec<JsRuvectorLayer>,
) -> Result<Vec<f32>, JsValue> {
    // Convert layer embeddings from JS value
    let embeddings: Vec<Vec<Vec<f32>>> = serde_wasm_bindgen::from_value(layer_embeddings)
        .map_err(|e| JsValue::from_str(&format!("Failed to parse layer embeddings: {}", e)))?;

    // Extract inner layers
    let core_layers: Vec<RuvectorLayer> = gnn_layers.iter().map(|l| l.inner.clone()).collect();

    // Call core function
    let result = core_hierarchical_forward(&query, &embeddings, &core_layers);

    Ok(result)
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Get version information
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Compute cosine similarity between two vectors
///
/// # Arguments
/// * `a` - First vector (Float32Array)
/// * `b` - Second vector (Float32Array)
///
/// # Returns
/// Cosine similarity score [-1.0, 1.0]
#[wasm_bindgen(js_name = cosineSimilarity)]
pub fn cosine_similarity(a: Vec<f32>, b: Vec<f32>) -> Result<f32, JsValue> {
    if a.len() != b.len() {
        return Err(JsValue::from_str(&format!(
            "Vector dimensions must match: {} vs {}",
            a.len(),
            b.len()
        )));
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        Ok(0.0)
    } else {
        Ok(dot_product / (norm_a * norm_b))
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
    fn test_version() {
        assert!(!version().is_empty());
    }

    #[wasm_bindgen_test]
    fn test_ruvector_layer_creation() {
        let layer = JsRuvectorLayer::new(4, 8, 2, 0.1);
        assert!(layer.is_ok());
    }

    #[wasm_bindgen_test]
    fn test_tensor_compress_creation() {
        let compressor = JsTensorCompress::new();
        assert_eq!(compressor.get_compression_ratio(1.0), 1.0);
        assert_eq!(compressor.get_compression_ratio(0.5), 2.0);
    }

    #[wasm_bindgen_test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(a, b).unwrap();
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[wasm_bindgen_test]
    fn test_search_config() {
        let config = SearchConfig::new(5, 1.0);
        assert_eq!(config.k, 5);
        assert_eq!(config.temperature, 1.0);
    }
}
