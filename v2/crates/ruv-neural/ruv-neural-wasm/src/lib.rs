//! rUv Neural WASM — WebAssembly bindings for browser-based brain topology visualization.
//!
//! This crate provides JavaScript-callable functions for creating, analyzing, and
//! visualizing brain connectivity graphs directly in the browser. It wraps the
//! core `ruv-neural-core` types with `wasm-bindgen` bindings and provides
//! lightweight WASM-compatible implementations of graph algorithms.
//!
//! # Features
//!
//! - Parse brain graphs from JSON and return JS-compatible objects
//! - Compute minimum cut (Stoer-Wagner) on graphs up to 500 nodes
//! - Generate topology metrics (density, efficiency, modularity, Fiedler value)
//! - Spectral embedding via power iteration (no LAPACK dependency)
//! - Decode cognitive state from topology metrics
//! - RVF file format load/export
//! - Streaming data processor for WebSocket integration
//! - Visualization data structures for D3.js / Three.js

pub mod graph_wasm;
pub mod streaming;
pub mod viz_data;

use ruv_neural_core::graph::BrainGraph;
use ruv_neural_core::rvf::{RvfDataType, RvfFile};
use ruv_neural_core::topology::TopologyMetrics;
use wasm_bindgen::prelude::*;

use graph_wasm::{wasm_decode, wasm_embed, wasm_mincut, wasm_topology_metrics};

/// Initialize the WASM module.
///
/// Called automatically when the module is loaded. Sets up panic hooks
/// for better error messages in the browser console.
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// Create a brain graph from JSON data.
///
/// Parses a JSON string into a `BrainGraph` and returns it as a JS object.
///
/// # Arguments
/// * `json_data` - JSON string representing a `BrainGraph`.
///
/// # Returns
/// A JS object containing the parsed graph data.
#[wasm_bindgen]
pub fn create_brain_graph(json_data: &str) -> Result<JsValue, JsError> {
    let graph: BrainGraph =
        serde_json::from_str(json_data).map_err(|e| JsError::new(&e.to_string()))?;
    serde_wasm_bindgen::to_value(&graph).map_err(|e| JsError::new(&e.to_string()))
}

/// Compute minimum cut on a brain graph.
///
/// Uses a simplified Stoer-Wagner algorithm suitable for graphs with up to
/// 500 nodes. Returns the cut value, partitions, and cut edges.
///
/// # Arguments
/// * `json_graph` - JSON string representing a `BrainGraph`.
///
/// # Returns
/// A JS object containing the `MincutResult`.
#[wasm_bindgen]
pub fn compute_mincut(json_graph: &str) -> Result<JsValue, JsError> {
    let graph: BrainGraph =
        serde_json::from_str(json_graph).map_err(|e| JsError::new(&e.to_string()))?;
    let result = wasm_mincut(&graph)?;
    serde_wasm_bindgen::to_value(&result).map_err(|e| JsError::new(&e.to_string()))
}

/// Compute topology metrics for a brain graph.
///
/// Returns density, efficiency, modularity, Fiedler value, entropy, and
/// module count. All computations use WASM-compatible algorithms without
/// heavy linear algebra dependencies.
///
/// # Arguments
/// * `json_graph` - JSON string representing a `BrainGraph`.
///
/// # Returns
/// A JS object containing the `TopologyMetrics`.
#[wasm_bindgen]
pub fn compute_topology_metrics(json_graph: &str) -> Result<JsValue, JsError> {
    let graph: BrainGraph =
        serde_json::from_str(json_graph).map_err(|e| JsError::new(&e.to_string()))?;
    let metrics = wasm_topology_metrics(&graph)?;
    serde_wasm_bindgen::to_value(&metrics).map_err(|e| JsError::new(&e.to_string()))
}

/// Generate a spectral embedding from a brain graph.
///
/// Uses power iteration on the normalized Laplacian to compute spectral
/// coordinates. Returns a flat vector of length `num_nodes * dimension`.
///
/// # Arguments
/// * `json_graph` - JSON string representing a `BrainGraph`.
/// * `dimension` - Number of embedding dimensions.
///
/// # Returns
/// A JS object containing the `NeuralEmbedding`.
#[wasm_bindgen]
pub fn embed_graph(json_graph: &str, dimension: usize) -> Result<JsValue, JsError> {
    let graph: BrainGraph =
        serde_json::from_str(json_graph).map_err(|e| JsError::new(&e.to_string()))?;
    let embedding = wasm_embed(&graph, dimension)?;
    serde_wasm_bindgen::to_value(&embedding).map_err(|e| JsError::new(&e.to_string()))
}

/// Decode cognitive state from topology metrics.
///
/// Uses threshold-based heuristics to classify the cognitive state
/// from a set of topology metrics. For production use, the trained
/// decoder from `ruv-neural-decoder` is recommended.
///
/// # Arguments
/// * `json_metrics` - JSON string representing `TopologyMetrics`.
///
/// # Returns
/// A JS object containing the decoded `CognitiveState`.
#[wasm_bindgen]
pub fn decode_state(json_metrics: &str) -> Result<JsValue, JsError> {
    let metrics: TopologyMetrics =
        serde_json::from_str(json_metrics).map_err(|e| JsError::new(&e.to_string()))?;
    let state = wasm_decode(&metrics)?;
    serde_wasm_bindgen::to_value(&state).map_err(|e| JsError::new(&e.to_string()))
}

/// Load an RVF (RuVector File) from raw bytes.
///
/// Parses the binary RVF header, JSON metadata, and payload, returning
/// the complete file structure as a JS object.
///
/// # Arguments
/// * `data` - Raw bytes of the RVF file.
///
/// # Returns
/// A JS object containing the parsed `RvfFile`.
#[wasm_bindgen]
pub fn load_rvf(data: &[u8]) -> Result<JsValue, JsError> {
    let mut cursor = std::io::Cursor::new(data);
    let rvf = RvfFile::read_from(&mut cursor).map_err(|e| JsError::new(&e.to_string()))?;
    serde_wasm_bindgen::to_value(&rvf).map_err(|e| JsError::new(&e.to_string()))
}

/// Export a brain graph as RVF bytes.
///
/// Serializes a `BrainGraph` (provided as JSON) into the binary RVF format.
///
/// # Arguments
/// * `json_graph` - JSON string representing a `BrainGraph`.
///
/// # Returns
/// A `Vec<u8>` containing the RVF binary data.
#[wasm_bindgen]
pub fn export_rvf(json_graph: &str) -> Result<Vec<u8>, JsError> {
    let graph: BrainGraph =
        serde_json::from_str(json_graph).map_err(|e| JsError::new(&e.to_string()))?;

    let graph_json =
        serde_json::to_vec(&graph).map_err(|e| JsError::new(&e.to_string()))?;

    let mut rvf = RvfFile::new(RvfDataType::BrainGraph);
    rvf.header.num_entries = 1;
    rvf.metadata = serde_json::json!({
        "num_nodes": graph.num_nodes,
        "num_edges": graph.edges.len(),
        "timestamp": graph.timestamp,
    });
    rvf.data = graph_json;

    let mut buf = Vec::new();
    rvf.write_to(&mut buf)
        .map_err(|e| JsError::new(&e.to_string()))?;

    Ok(buf)
}

/// Get the crate version string.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruv_neural_core::brain::Atlas;
    use ruv_neural_core::graph::{BrainEdge, BrainGraph};
    use ruv_neural_core::signal::FrequencyBand;

    fn sample_graph_json() -> String {
        let graph = BrainGraph {
            num_nodes: 3,
            edges: vec![
                BrainEdge {
                    source: 0,
                    target: 1,
                    weight: 0.8,
                    metric: ruv_neural_core::graph::ConnectivityMetric::Coherence,
                    frequency_band: FrequencyBand::Alpha,
                },
                BrainEdge {
                    source: 1,
                    target: 2,
                    weight: 0.5,
                    metric: ruv_neural_core::graph::ConnectivityMetric::Coherence,
                    frequency_band: FrequencyBand::Beta,
                },
            ],
            timestamp: 1000.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(3),
        };
        serde_json::to_string(&graph).unwrap()
    }

    #[test]
    fn test_create_brain_graph_parses_valid_json() {
        let json = sample_graph_json();
        let graph: BrainGraph = serde_json::from_str(&json).unwrap();
        assert_eq!(graph.num_nodes, 3);
        assert_eq!(graph.edges.len(), 2);
    }

    #[test]
    fn test_create_brain_graph_rejects_invalid_json() {
        let result: Result<BrainGraph, _> = serde_json::from_str("not valid json");
        assert!(result.is_err());
    }

    #[test]
    fn test_compute_mincut_returns_valid_result() {
        let json = sample_graph_json();
        let graph: BrainGraph = serde_json::from_str(&json).unwrap();
        let result = wasm_mincut(&graph).unwrap();
        assert!(result.cut_value >= 0.0);
        assert_eq!(result.num_nodes(), 3);
    }

    #[test]
    fn test_rvf_round_trip() {
        let json = sample_graph_json();
        let graph: BrainGraph = serde_json::from_str(&json).unwrap();

        // Export to RVF bytes.
        let graph_bytes = serde_json::to_vec(&graph).unwrap();
        let mut rvf = RvfFile::new(RvfDataType::BrainGraph);
        rvf.header.num_entries = 1;
        rvf.metadata = serde_json::json!({"test": true});
        rvf.data = graph_bytes;

        let mut buf = Vec::new();
        rvf.write_to(&mut buf).unwrap();

        // Read back.
        let mut cursor = std::io::Cursor::new(&buf);
        let loaded = RvfFile::read_from(&mut cursor).unwrap();

        assert_eq!(loaded.header.data_type, RvfDataType::BrainGraph);
        assert_eq!(loaded.header.num_entries, 1);

        // Deserialize the payload back to a BrainGraph.
        let loaded_graph: BrainGraph = serde_json::from_slice(&loaded.data).unwrap();
        assert_eq!(loaded_graph.num_nodes, 3);
        assert_eq!(loaded_graph.edges.len(), 2);
    }

    #[test]
    fn test_version_returns_string() {
        let v = version();
        assert!(!v.is_empty());
        assert!(v.contains('.'));
    }

    #[test]
    fn test_decode_state_from_metrics() {
        let metrics = TopologyMetrics {
            global_mincut: 0.5,
            modularity: 0.6,
            global_efficiency: 0.2,
            local_efficiency: 0.3,
            graph_entropy: 1.5,
            fiedler_value: 0.3,
            num_modules: 2,
            timestamp: 0.0,
        };
        let state = wasm_decode(&metrics).unwrap();
        // High modularity + low efficiency + moderate entropy => Rest.
        assert_eq!(
            state,
            ruv_neural_core::topology::CognitiveState::Rest
        );
    }

    #[test]
    fn test_embed_graph_produces_correct_dimensions() {
        let json = sample_graph_json();
        let graph: BrainGraph = serde_json::from_str(&json).unwrap();
        let embedding = wasm_embed(&graph, 2).unwrap();
        assert_eq!(embedding.vector.len(), 6);
    }
}
