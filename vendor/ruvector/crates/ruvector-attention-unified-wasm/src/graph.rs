//! Graph Attention Mechanisms (from ruvector-gnn)
//!
//! Re-exports graph neural network attention mechanisms:
//! - GAT (Graph Attention Networks)
//! - GCN (Graph Convolutional Networks)
//! - GraphSAGE (Sample and Aggregate)

use ruvector_gnn::{
    differentiable_search as core_differentiable_search,
    hierarchical_forward as core_hierarchical_forward, CompressedTensor, CompressionLevel,
    RuvectorLayer, TensorCompress,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

// ============================================================================
// GNN Layer (GAT-based)
// ============================================================================

/// Graph Neural Network layer with attention mechanism
///
/// Implements Graph Attention Networks (GAT) for HNSW topology.
/// Each node aggregates information from neighbors using learned attention weights.
#[wasm_bindgen]
pub struct WasmGNNLayer {
    inner: RuvectorLayer,
    hidden_dim: usize,
}

#[wasm_bindgen]
impl WasmGNNLayer {
    /// Create a new GNN layer with attention
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
    ) -> Result<WasmGNNLayer, JsError> {
        let inner = RuvectorLayer::new(input_dim, hidden_dim, heads, dropout)
            .map_err(|e| JsError::new(&e.to_string()))?;

        Ok(WasmGNNLayer { inner, hidden_dim })
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
    pub fn forward(
        &self,
        node_embedding: Vec<f32>,
        neighbor_embeddings: JsValue,
        edge_weights: Vec<f32>,
    ) -> Result<Vec<f32>, JsError> {
        let neighbors: Vec<Vec<f32>> = serde_wasm_bindgen::from_value(neighbor_embeddings)
            .map_err(|e| JsError::new(&format!("Failed to parse neighbor embeddings: {}", e)))?;

        if neighbors.len() != edge_weights.len() {
            return Err(JsError::new(&format!(
                "Number of neighbors ({}) must match number of edge weights ({})",
                neighbors.len(),
                edge_weights.len()
            )));
        }

        let result = self
            .inner
            .forward(&node_embedding, &neighbors, &edge_weights);
        Ok(result)
    }

    /// Get the output dimension
    #[wasm_bindgen(getter, js_name = outputDim)]
    pub fn output_dim(&self) -> usize {
        self.hidden_dim
    }
}

// ============================================================================
// Tensor Compression (for efficient GNN)
// ============================================================================

/// Tensor compressor with adaptive level selection
///
/// Compresses embeddings based on access frequency for memory-efficient GNN
#[wasm_bindgen]
pub struct WasmTensorCompress {
    inner: TensorCompress,
}

#[wasm_bindgen]
impl WasmTensorCompress {
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
    /// * `embedding` - The input embedding vector
    /// * `access_freq` - Access frequency in range [0.0, 1.0]
    ///   - f > 0.8: Full precision (hot data)
    ///   - f > 0.4: Half precision (warm data)
    ///   - f > 0.1: 8-bit PQ (cool data)
    ///   - f > 0.01: 4-bit PQ (cold data)
    ///   - f <= 0.01: Binary (archive)
    pub fn compress(&self, embedding: Vec<f32>, access_freq: f32) -> Result<JsValue, JsError> {
        let compressed = self
            .inner
            .compress(&embedding, access_freq)
            .map_err(|e| JsError::new(&format!("Compression failed: {}", e)))?;

        serde_wasm_bindgen::to_value(&compressed)
            .map_err(|e| JsError::new(&format!("Serialization failed: {}", e)))
    }

    /// Compress with explicit compression level
    ///
    /// # Arguments
    /// * `embedding` - The input embedding vector
    /// * `level` - Compression level: "none", "half", "pq8", "pq4", "binary"
    #[wasm_bindgen(js_name = compressWithLevel)]
    pub fn compress_with_level(
        &self,
        embedding: Vec<f32>,
        level: &str,
    ) -> Result<JsValue, JsError> {
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
                return Err(JsError::new(&format!(
                    "Unknown compression level: {}",
                    level
                )))
            }
        };

        let compressed = self
            .inner
            .compress_with_level(&embedding, &compression_level)
            .map_err(|e| JsError::new(&format!("Compression failed: {}", e)))?;

        serde_wasm_bindgen::to_value(&compressed)
            .map_err(|e| JsError::new(&format!("Serialization failed: {}", e)))
    }

    /// Decompress a compressed tensor
    pub fn decompress(&self, compressed: JsValue) -> Result<Vec<f32>, JsError> {
        let compressed_tensor: CompressedTensor = serde_wasm_bindgen::from_value(compressed)
            .map_err(|e| JsError::new(&format!("Deserialization failed: {}", e)))?;

        self.inner
            .decompress(&compressed_tensor)
            .map_err(|e| JsError::new(&format!("Decompression failed: {}", e)))
    }

    /// Get compression ratio estimate for a given access frequency
    #[wasm_bindgen(js_name = getCompressionRatio)]
    pub fn get_compression_ratio(&self, access_freq: f32) -> f32 {
        if access_freq > 0.8 {
            1.0
        } else if access_freq > 0.4 {
            2.0
        } else if access_freq > 0.1 {
            4.0
        } else if access_freq > 0.01 {
            8.0
        } else {
            32.0
        }
    }
}

// ============================================================================
// Search Configuration
// ============================================================================

/// Search configuration for differentiable search
#[wasm_bindgen]
pub struct WasmSearchConfig {
    /// Number of top results to return
    pub k: usize,
    /// Temperature for softmax
    pub temperature: f32,
}

#[wasm_bindgen]
impl WasmSearchConfig {
    /// Create a new search configuration
    #[wasm_bindgen(constructor)]
    pub fn new(k: usize, temperature: f32) -> Self {
        Self { k, temperature }
    }
}

// ============================================================================
// Differentiable Search
// ============================================================================

/// Differentiable search using soft attention mechanism
///
/// # Arguments
/// * `query` - The query vector
/// * `candidate_embeddings` - List of candidate embedding vectors
/// * `config` - Search configuration
///
/// # Returns
/// Object with indices and weights for top-k candidates
#[wasm_bindgen(js_name = graphDifferentiableSearch)]
pub fn differentiable_search(
    query: Vec<f32>,
    candidate_embeddings: JsValue,
    config: &WasmSearchConfig,
) -> Result<JsValue, JsError> {
    let candidates: Vec<Vec<f32>> = serde_wasm_bindgen::from_value(candidate_embeddings)
        .map_err(|e| JsError::new(&format!("Failed to parse candidate embeddings: {}", e)))?;

    let (indices, weights) =
        core_differentiable_search(&query, &candidates, config.k, config.temperature);

    let result = SearchResult { indices, weights };
    serde_wasm_bindgen::to_value(&result)
        .map_err(|e| JsError::new(&format!("Failed to serialize result: {}", e)))
}

#[derive(Serialize, Deserialize)]
struct SearchResult {
    indices: Vec<usize>,
    weights: Vec<f32>,
}

// ============================================================================
// Hierarchical Forward
// ============================================================================

/// Hierarchical forward pass through multiple GNN layers
///
/// # Arguments
/// * `query` - The query vector
/// * `layer_embeddings` - Embeddings organized by layer
/// * `gnn_layers` - Array of GNN layers
///
/// # Returns
/// Final embedding after hierarchical processing
#[wasm_bindgen(js_name = graphHierarchicalForward)]
pub fn hierarchical_forward(
    query: Vec<f32>,
    layer_embeddings: JsValue,
    gnn_layers: Vec<WasmGNNLayer>,
) -> Result<Vec<f32>, JsError> {
    let embeddings: Vec<Vec<Vec<f32>>> = serde_wasm_bindgen::from_value(layer_embeddings)
        .map_err(|e| JsError::new(&format!("Failed to parse layer embeddings: {}", e)))?;

    let core_layers: Vec<RuvectorLayer> = gnn_layers.iter().map(|l| l.inner.clone()).collect();

    let result = core_hierarchical_forward(&query, &embeddings, &core_layers);
    Ok(result)
}

// ============================================================================
// Graph Attention Types
// ============================================================================

/// Graph attention mechanism types
#[wasm_bindgen]
pub enum GraphAttentionType {
    /// Graph Attention Networks (Velickovic et al., 2018)
    GAT,
    /// Graph Convolutional Networks (Kipf & Welling, 2017)
    GCN,
    /// GraphSAGE (Hamilton et al., 2017)
    GraphSAGE,
}

/// Factory for graph attention information
#[wasm_bindgen]
pub struct GraphAttentionFactory;

#[wasm_bindgen]
impl GraphAttentionFactory {
    /// Get available graph attention types
    #[wasm_bindgen(js_name = availableTypes)]
    pub fn available_types() -> JsValue {
        let types = vec!["gat", "gcn", "graphsage"];
        serde_wasm_bindgen::to_value(&types).unwrap()
    }

    /// Get description for a graph attention type
    #[wasm_bindgen(js_name = getDescription)]
    pub fn get_description(attention_type: &str) -> String {
        match attention_type {
            "gat" => {
                "Graph Attention Networks - learns attention weights over neighbors".to_string()
            }
            "gcn" => "Graph Convolutional Networks - spectral convolution on graphs".to_string(),
            "graphsage" => "GraphSAGE - sample and aggregate neighbor features".to_string(),
            _ => "Unknown graph attention type".to_string(),
        }
    }

    /// Get recommended use cases for a graph attention type
    #[wasm_bindgen(js_name = getUseCases)]
    pub fn get_use_cases(attention_type: &str) -> JsValue {
        let cases = match attention_type {
            "gat" => vec![
                "Node classification with varying neighbor importance",
                "Link prediction in heterogeneous graphs",
                "Knowledge graph reasoning",
            ],
            "gcn" => vec![
                "Semi-supervised node classification",
                "Graph-level classification",
                "Spectral clustering",
            ],
            "graphsage" => vec![
                "Inductive learning on new nodes",
                "Large-scale graph processing",
                "Dynamic graphs with new vertices",
            ],
            _ => vec!["Unknown type"],
        };
        serde_wasm_bindgen::to_value(&cases).unwrap()
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
    fn test_gnn_layer_creation() {
        let layer = WasmGNNLayer::new(4, 8, 2, 0.1);
        assert!(layer.is_ok());
        let l = layer.unwrap();
        assert_eq!(l.output_dim(), 8);
    }

    #[wasm_bindgen_test]
    fn test_gnn_layer_invalid_dropout() {
        let layer = WasmGNNLayer::new(4, 8, 2, 1.5);
        assert!(layer.is_err());
    }

    #[wasm_bindgen_test]
    fn test_gnn_layer_invalid_heads() {
        let layer = WasmGNNLayer::new(4, 7, 3, 0.1);
        assert!(layer.is_err());
    }

    #[wasm_bindgen_test]
    fn test_tensor_compress_creation() {
        let compressor = WasmTensorCompress::new();
        assert_eq!(compressor.get_compression_ratio(1.0), 1.0);
        assert_eq!(compressor.get_compression_ratio(0.5), 2.0);
        assert_eq!(compressor.get_compression_ratio(0.2), 4.0);
        assert_eq!(compressor.get_compression_ratio(0.05), 8.0);
        assert_eq!(compressor.get_compression_ratio(0.005), 32.0);
    }

    #[wasm_bindgen_test]
    fn test_search_config() {
        let config = WasmSearchConfig::new(5, 1.0);
        assert_eq!(config.k, 5);
        assert_eq!(config.temperature, 1.0);
    }

    #[wasm_bindgen_test]
    fn test_factory_types() {
        let types_js = GraphAttentionFactory::available_types();
        assert!(!types_js.is_null());
    }

    #[wasm_bindgen_test]
    fn test_factory_descriptions() {
        let desc = GraphAttentionFactory::get_description("gat");
        assert!(desc.contains("Graph Attention"));

        let desc = GraphAttentionFactory::get_description("gcn");
        assert!(desc.contains("Graph Convolutional"));

        let desc = GraphAttentionFactory::get_description("graphsage");
        assert!(desc.contains("GraphSAGE"));
    }
}
