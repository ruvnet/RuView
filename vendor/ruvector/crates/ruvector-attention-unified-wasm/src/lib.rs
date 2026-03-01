//! Unified WebAssembly Attention Library
//!
//! This crate provides a unified WASM interface for 18+ attention mechanisms:
//!
//! ## Neural Attention (from ruvector-attention)
//! - **Scaled Dot-Product**: Standard transformer attention
//! - **Multi-Head**: Parallel attention heads
//! - **Hyperbolic**: Attention in hyperbolic space for hierarchical data
//! - **Linear**: O(n) Performer-style attention
//! - **Flash**: Memory-efficient blocked attention
//! - **Local-Global**: Sparse attention with global tokens
//! - **MoE**: Mixture of Experts attention
//!
//! ## DAG Attention (from ruvector-dag)
//! - **Topological**: Position-aware attention in DAG order
//! - **Causal Cone**: Lightcone-based causal attention
//! - **Critical Path**: Attention weighted by critical path distance
//! - **MinCut-Gated**: Flow-based gating attention
//! - **Hierarchical Lorentz**: Multi-scale hyperbolic DAG attention
//! - **Parallel Branch**: Attention for parallel DAG branches
//! - **Temporal BTSP**: Behavioral Time-Series Pattern attention
//!
//! ## Graph Attention (from ruvector-gnn)
//! - **GAT**: Graph Attention Networks
//! - **GCN**: Graph Convolutional Networks
//! - **GraphSAGE**: Sampling and Aggregating graph embeddings
//!
//! ## State Space Models
//! - **Mamba SSM**: Selective State Space Model attention

use wasm_bindgen::prelude::*;

// Use wee_alloc for smaller WASM binary (~10KB reduction)
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// ============================================================================
// Module declarations
// ============================================================================

pub mod mamba;

mod dag;
mod graph;
mod neural;

// ============================================================================
// Re-exports for convenient access
// ============================================================================

pub use dag::*;
pub use graph::*;
pub use mamba::*;
pub use neural::*;

// ============================================================================
// Initialization
// ============================================================================

/// Initialize the WASM module with panic hook for better error messages
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

// ============================================================================
// Version and Info
// ============================================================================

/// Get the version of the unified attention WASM crate
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Get information about all available attention mechanisms
#[wasm_bindgen(js_name = availableMechanisms)]
pub fn available_mechanisms() -> JsValue {
    let mechanisms = AttentionMechanisms {
        neural: vec![
            "scaled_dot_product".into(),
            "multi_head".into(),
            "hyperbolic".into(),
            "linear".into(),
            "flash".into(),
            "local_global".into(),
            "moe".into(),
        ],
        dag: vec![
            "topological".into(),
            "causal_cone".into(),
            "critical_path".into(),
            "mincut_gated".into(),
            "hierarchical_lorentz".into(),
            "parallel_branch".into(),
            "temporal_btsp".into(),
        ],
        graph: vec!["gat".into(), "gcn".into(), "graphsage".into()],
        ssm: vec!["mamba".into()],
    };
    serde_wasm_bindgen::to_value(&mechanisms).unwrap()
}

/// Get summary statistics about the unified attention library
#[wasm_bindgen(js_name = getStats)]
pub fn get_stats() -> JsValue {
    let stats = UnifiedStats {
        total_mechanisms: 18,
        neural_count: 7,
        dag_count: 7,
        graph_count: 3,
        ssm_count: 1,
        version: env!("CARGO_PKG_VERSION").to_string(),
    };
    serde_wasm_bindgen::to_value(&stats).unwrap()
}

// ============================================================================
// Internal Types
// ============================================================================

#[derive(serde::Serialize)]
struct AttentionMechanisms {
    neural: Vec<String>,
    dag: Vec<String>,
    graph: Vec<String>,
    ssm: Vec<String>,
}

#[derive(serde::Serialize)]
struct UnifiedStats {
    total_mechanisms: usize,
    neural_count: usize,
    dag_count: usize,
    graph_count: usize,
    ssm_count: usize,
    version: String,
}

// ============================================================================
// Unified Attention Selector
// ============================================================================

/// Unified attention mechanism selector
/// Automatically routes to the appropriate attention implementation
#[wasm_bindgen]
pub struct UnifiedAttention {
    mechanism_type: String,
}

#[wasm_bindgen]
impl UnifiedAttention {
    /// Create a new unified attention selector
    #[wasm_bindgen(constructor)]
    pub fn new(mechanism: &str) -> Result<UnifiedAttention, JsError> {
        let valid_mechanisms = [
            // Neural
            "scaled_dot_product",
            "multi_head",
            "hyperbolic",
            "linear",
            "flash",
            "local_global",
            "moe",
            // DAG
            "topological",
            "causal_cone",
            "critical_path",
            "mincut_gated",
            "hierarchical_lorentz",
            "parallel_branch",
            "temporal_btsp",
            // Graph
            "gat",
            "gcn",
            "graphsage",
            // SSM
            "mamba",
        ];

        if !valid_mechanisms.contains(&mechanism) {
            return Err(JsError::new(&format!(
                "Unknown mechanism: {}. Valid options: {:?}",
                mechanism, valid_mechanisms
            )));
        }

        Ok(Self {
            mechanism_type: mechanism.to_string(),
        })
    }

    /// Get the currently selected mechanism type
    #[wasm_bindgen(getter)]
    pub fn mechanism(&self) -> String {
        self.mechanism_type.clone()
    }

    /// Get the category of the selected mechanism
    #[wasm_bindgen(getter)]
    pub fn category(&self) -> String {
        match self.mechanism_type.as_str() {
            "scaled_dot_product" | "multi_head" | "hyperbolic" | "linear" | "flash"
            | "local_global" | "moe" => "neural".to_string(),

            "topological"
            | "causal_cone"
            | "critical_path"
            | "mincut_gated"
            | "hierarchical_lorentz"
            | "parallel_branch"
            | "temporal_btsp" => "dag".to_string(),

            "gat" | "gcn" | "graphsage" => "graph".to_string(),

            "mamba" => "ssm".to_string(),

            _ => "unknown".to_string(),
        }
    }

    /// Check if this mechanism supports sequence processing
    #[wasm_bindgen(js_name = supportsSequences)]
    pub fn supports_sequences(&self) -> bool {
        matches!(
            self.mechanism_type.as_str(),
            "scaled_dot_product" | "multi_head" | "linear" | "flash" | "local_global" | "mamba"
        )
    }

    /// Check if this mechanism supports graph/DAG structures
    #[wasm_bindgen(js_name = supportsGraphs)]
    pub fn supports_graphs(&self) -> bool {
        matches!(
            self.mechanism_type.as_str(),
            "topological"
                | "causal_cone"
                | "critical_path"
                | "mincut_gated"
                | "hierarchical_lorentz"
                | "parallel_branch"
                | "temporal_btsp"
                | "gat"
                | "gcn"
                | "graphsage"
        )
    }

    /// Check if this mechanism supports hyperbolic geometry
    #[wasm_bindgen(js_name = supportsHyperbolic)]
    pub fn supports_hyperbolic(&self) -> bool {
        matches!(
            self.mechanism_type.as_str(),
            "hyperbolic" | "hierarchical_lorentz"
        )
    }
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Compute cosine similarity between two vectors
#[wasm_bindgen(js_name = cosineSimilarity)]
pub fn cosine_similarity(a: Vec<f32>, b: Vec<f32>) -> Result<f32, JsError> {
    if a.len() != b.len() {
        return Err(JsError::new(&format!(
            "Vector dimensions must match: {} vs {}",
            a.len(),
            b.len()
        )));
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        Ok(0.0)
    } else {
        Ok(dot / (norm_a * norm_b))
    }
}

/// Softmax normalization
#[wasm_bindgen]
pub fn softmax(values: Vec<f32>) -> Vec<f32> {
    let max_val = values.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
    let exp_values: Vec<f32> = values.iter().map(|&x| (x - max_val).exp()).collect();
    let sum: f32 = exp_values.iter().sum();
    exp_values.iter().map(|&x| x / sum).collect()
}

/// Temperature-scaled softmax
#[wasm_bindgen(js_name = temperatureSoftmax)]
pub fn temperature_softmax(values: Vec<f32>, temperature: f32) -> Vec<f32> {
    if temperature <= 0.0 {
        // Return one-hot for the maximum
        let max_idx = values
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0);
        let mut result = vec![0.0; values.len()];
        result[max_idx] = 1.0;
        return result;
    }

    let scaled: Vec<f32> = values.iter().map(|&x| x / temperature).collect();
    softmax(scaled)
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
    fn test_unified_attention_creation() {
        let attention = UnifiedAttention::new("multi_head");
        assert!(attention.is_ok());

        let invalid = UnifiedAttention::new("invalid_mechanism");
        assert!(invalid.is_err());
    }

    #[wasm_bindgen_test]
    fn test_mechanism_categories() {
        let neural = UnifiedAttention::new("multi_head").unwrap();
        assert_eq!(neural.category(), "neural");

        let dag = UnifiedAttention::new("topological").unwrap();
        assert_eq!(dag.category(), "dag");

        let graph = UnifiedAttention::new("gat").unwrap();
        assert_eq!(graph.category(), "graph");

        let ssm = UnifiedAttention::new("mamba").unwrap();
        assert_eq!(ssm.category(), "ssm");
    }

    #[wasm_bindgen_test]
    fn test_softmax() {
        let input = vec![1.0, 2.0, 3.0];
        let output = softmax(input);

        // Sum should be 1.0
        let sum: f32 = output.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6);

        // Should be monotonically increasing
        assert!(output[0] < output[1]);
        assert!(output[1] < output[2]);
    }

    #[wasm_bindgen_test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(a, b).unwrap();
        assert!((sim - 1.0).abs() < 1e-6);

        let c = vec![1.0, 0.0, 0.0];
        let d = vec![0.0, 1.0, 0.0];
        let sim2 = cosine_similarity(c, d).unwrap();
        assert!(sim2.abs() < 1e-6);
    }
}
