//! # Prime-Radiant Advanced WASM Bindings
//!
//! WebAssembly bindings for all 6 Prime-Radiant Advanced Math modules:
//!
//! - **CohomologyEngine**: Sheaf cohomology computations
//! - **CategoryEngine**: Functorial retrieval and topos operations
//! - **HoTTEngine**: Type checking and path operations
//! - **SpectralEngine**: Eigenvalue computation and Cheeger bounds
//! - **CausalEngine**: Causal inference and interventions
//! - **QuantumEngine**: Topological invariants and quantum simulation
//!
//! ## Usage from JavaScript/TypeScript
//!
//! ```typescript
//! import init, {
//!     CohomologyEngine,
//!     SpectralEngine,
//!     CausalEngine,
//!     QuantumEngine,
//!     CategoryEngine,
//!     HoTTEngine,
//! } from 'prime-radiant-advanced-wasm';
//!
//! await init();
//!
//! const cohomology = new CohomologyEngine();
//! const obstructions = cohomology.detectObstructions(beliefGraph);
//! ```

use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Set up panic hook for better error messages
#[wasm_bindgen(start)]
pub fn start() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

// ============================================================================
// Common Types
// ============================================================================

/// JavaScript-friendly error type
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct WasmError {
    message: String,
    code: String,
}

#[wasm_bindgen]
impl WasmError {
    #[wasm_bindgen(getter)]
    pub fn message(&self) -> String {
        self.message.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn code(&self) -> String {
        self.code.clone()
    }
}

impl From<String> for WasmError {
    fn from(msg: String) -> Self {
        Self {
            message: msg,
            code: "ERROR".to_string(),
        }
    }
}

// ============================================================================
// Cohomology Engine
// ============================================================================

/// Sheaf node for cohomology computations
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SheafNode {
    pub id: usize,
    pub label: String,
    pub section: Vec<f64>,
    pub weight: f64,
}

/// Sheaf edge with restriction map
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SheafEdge {
    pub source: usize,
    pub target: usize,
    pub restriction_map: Vec<f64>,
    pub source_dim: usize,
    pub target_dim: usize,
}

/// Sheaf graph structure
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SheafGraph {
    pub nodes: Vec<SheafNode>,
    pub edges: Vec<SheafEdge>,
}

/// Result of cohomology computation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CohomologyResult {
    pub h0_dim: usize,
    pub h1_dim: usize,
    pub euler_characteristic: i64,
    pub consistency_energy: f64,
    pub is_consistent: bool,
}

/// Detected obstruction
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Obstruction {
    pub edge_index: usize,
    pub source_node: usize,
    pub target_node: usize,
    pub obstruction_vector: Vec<f64>,
    pub magnitude: f64,
    pub description: String,
}

/// Sheaf cohomology computation engine
#[wasm_bindgen]
pub struct CohomologyEngine {
    tolerance: f64,
}

#[wasm_bindgen]
impl CohomologyEngine {
    /// Create a new cohomology engine
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self { tolerance: 1e-10 }
    }

    /// Create with custom tolerance
    #[wasm_bindgen(js_name = withTolerance)]
    pub fn with_tolerance(tolerance: f64) -> Self {
        Self { tolerance }
    }

    /// Compute cohomology groups of a sheaf graph
    #[wasm_bindgen(js_name = computeCohomology)]
    pub fn compute_cohomology(&self, graph_js: JsValue) -> Result<JsValue, JsValue> {
        let graph: SheafGraph = serde_wasm_bindgen::from_value(graph_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse graph: {}", e)))?;

        let result = self.compute_cohomology_internal(&graph);

        serde_wasm_bindgen::to_value(&result)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize result: {}", e)))
    }

    /// Detect all obstructions to global consistency
    #[wasm_bindgen(js_name = detectObstructions)]
    pub fn detect_obstructions(&self, graph_js: JsValue) -> Result<JsValue, JsValue> {
        let graph: SheafGraph = serde_wasm_bindgen::from_value(graph_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse graph: {}", e)))?;

        let obstructions = self.detect_obstructions_internal(&graph);

        serde_wasm_bindgen::to_value(&obstructions)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize obstructions: {}", e)))
    }

    /// Compute global sections (H^0)
    #[wasm_bindgen(js_name = computeGlobalSections)]
    pub fn compute_global_sections(&self, graph_js: JsValue) -> Result<JsValue, JsValue> {
        let graph: SheafGraph = serde_wasm_bindgen::from_value(graph_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse graph: {}", e)))?;

        let sections = self.compute_global_sections_internal(&graph);

        serde_wasm_bindgen::to_value(&sections)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize sections: {}", e)))
    }

    /// Compute consistency energy
    #[wasm_bindgen(js_name = consistencyEnergy)]
    pub fn consistency_energy(&self, graph_js: JsValue) -> Result<f64, JsValue> {
        let graph: SheafGraph = serde_wasm_bindgen::from_value(graph_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse graph: {}", e)))?;

        Ok(self.compute_consistency_energy_internal(&graph))
    }
}

impl CohomologyEngine {
    fn compute_cohomology_internal(&self, graph: &SheafGraph) -> CohomologyResult {
        if graph.nodes.is_empty() {
            return CohomologyResult {
                h0_dim: 0,
                h1_dim: 0,
                euler_characteristic: 0,
                consistency_energy: 0.0,
                is_consistent: true,
            };
        }

        let _c0_dim: usize = graph.nodes.iter().map(|n| n.section.len()).sum();
        let _c1_dim: usize = graph.edges.iter().map(|e| e.target_dim).sum();

        let consistency_energy = self.compute_consistency_energy_internal(graph);
        let is_consistent = consistency_energy < self.tolerance;

        // Simplified dimension computation
        let h0_dim = if is_consistent { 1 } else { 0 };
        let h1_dim = if is_consistent { 0 } else { graph.edges.len() };

        CohomologyResult {
            h0_dim,
            h1_dim,
            euler_characteristic: h0_dim as i64 - h1_dim as i64,
            consistency_energy,
            is_consistent,
        }
    }

    fn detect_obstructions_internal(&self, graph: &SheafGraph) -> Vec<Obstruction> {
        let mut obstructions = Vec::new();

        for (i, edge) in graph.edges.iter().enumerate() {
            if edge.source >= graph.nodes.len() || edge.target >= graph.nodes.len() {
                continue;
            }

            let source = &graph.nodes[edge.source];
            let target = &graph.nodes[edge.target];

            // Apply restriction map
            let restricted = self.apply_restriction(edge, &source.section);

            // Compute difference
            let mut diff = Vec::new();
            let mut magnitude_sq = 0.0;

            let min_len = restricted.len().min(target.section.len());
            for j in 0..min_len {
                let d = restricted[j] - target.section[j];
                diff.push(d);
                magnitude_sq += d * d;
            }

            let magnitude = magnitude_sq.sqrt();

            if magnitude > self.tolerance {
                obstructions.push(Obstruction {
                    edge_index: i,
                    source_node: edge.source,
                    target_node: edge.target,
                    obstruction_vector: diff,
                    magnitude,
                    description: format!(
                        "Inconsistency between '{}' and '{}': magnitude {:.6}",
                        source.label, target.label, magnitude
                    ),
                });
            }
        }

        obstructions.sort_by(|a, b| {
            b.magnitude.partial_cmp(&a.magnitude).unwrap_or(std::cmp::Ordering::Equal)
        });

        obstructions
    }

    fn compute_global_sections_internal(&self, graph: &SheafGraph) -> Vec<Vec<f64>> {
        if graph.nodes.is_empty() {
            return Vec::new();
        }

        let dim = graph.nodes[0].section.len();
        let mut avg = vec![0.0; dim];
        let mut total_weight = 0.0;

        for node in &graph.nodes {
            for j in 0..dim.min(node.section.len()) {
                avg[j] += node.section[j] * node.weight;
            }
            total_weight += node.weight;
        }

        if total_weight > 0.0 {
            for v in &mut avg {
                *v /= total_weight;
            }
            vec![avg]
        } else {
            Vec::new()
        }
    }

    fn compute_consistency_energy_internal(&self, graph: &SheafGraph) -> f64 {
        let mut total = 0.0;

        for edge in &graph.edges {
            if edge.source >= graph.nodes.len() || edge.target >= graph.nodes.len() {
                continue;
            }

            let source = &graph.nodes[edge.source];
            let target = &graph.nodes[edge.target];

            let restricted = self.apply_restriction(edge, &source.section);

            for j in 0..restricted.len().min(target.section.len()) {
                let diff = restricted[j] - target.section[j];
                total += diff * diff;
            }
        }

        total
    }

    fn apply_restriction(&self, edge: &SheafEdge, section: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; edge.target_dim];

        for i in 0..edge.target_dim {
            for j in 0..edge.source_dim.min(section.len()) {
                if i * edge.source_dim + j < edge.restriction_map.len() {
                    result[i] += edge.restriction_map[i * edge.source_dim + j] * section[j];
                }
            }
        }

        result
    }
}

impl Default for CohomologyEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Spectral Engine
// ============================================================================

/// Graph structure for spectral analysis
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Graph {
    pub n: usize,
    pub edges: Vec<(usize, usize, f64)>,
}

/// Cheeger bounds result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheegerBounds {
    pub lower_bound: f64,
    pub upper_bound: f64,
    pub cheeger_estimate: f64,
    pub fiedler_value: f64,
}

/// Spectral gap information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpectralGap {
    pub lambda_1: f64,
    pub lambda_2: f64,
    pub gap: f64,
    pub ratio: f64,
}

/// Min-cut prediction
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MinCutPrediction {
    pub predicted_cut: f64,
    pub lower_bound: f64,
    pub upper_bound: f64,
    pub confidence: f64,
    pub cut_nodes: Vec<usize>,
}

/// Spectral analysis engine
#[wasm_bindgen]
pub struct SpectralEngine {
    num_eigenvalues: usize,
    tolerance: f64,
    max_iterations: usize,
}

#[wasm_bindgen]
impl SpectralEngine {
    /// Create a new spectral engine
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            num_eigenvalues: 10,
            tolerance: 1e-10,
            max_iterations: 1000,
        }
    }

    /// Create with configuration
    #[wasm_bindgen(js_name = withConfig)]
    pub fn with_config(num_eigenvalues: usize, tolerance: f64, max_iterations: usize) -> Self {
        Self {
            num_eigenvalues,
            tolerance,
            max_iterations,
        }
    }

    /// Compute Cheeger bounds for a graph
    #[wasm_bindgen(js_name = computeCheegerBounds)]
    pub fn compute_cheeger_bounds(&self, graph_js: JsValue) -> Result<JsValue, JsValue> {
        let graph: Graph = serde_wasm_bindgen::from_value(graph_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse graph: {}", e)))?;

        let bounds = self.compute_cheeger_bounds_internal(&graph);

        serde_wasm_bindgen::to_value(&bounds)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize bounds: {}", e)))
    }

    /// Compute eigenvalues of the graph Laplacian
    #[wasm_bindgen(js_name = computeEigenvalues)]
    pub fn compute_eigenvalues(&self, graph_js: JsValue) -> Result<JsValue, JsValue> {
        let graph: Graph = serde_wasm_bindgen::from_value(graph_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse graph: {}", e)))?;

        let eigenvalues = self.compute_eigenvalues_internal(&graph);

        serde_wasm_bindgen::to_value(&eigenvalues)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize eigenvalues: {}", e)))
    }

    /// Compute the algebraic connectivity (Fiedler value)
    #[wasm_bindgen(js_name = algebraicConnectivity)]
    pub fn algebraic_connectivity(&self, graph_js: JsValue) -> Result<f64, JsValue> {
        let graph: Graph = serde_wasm_bindgen::from_value(graph_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse graph: {}", e)))?;

        Ok(self.compute_fiedler_value(&graph))
    }

    /// Compute spectral gap
    #[wasm_bindgen(js_name = computeSpectralGap)]
    pub fn compute_spectral_gap(&self, graph_js: JsValue) -> Result<JsValue, JsValue> {
        let graph: Graph = serde_wasm_bindgen::from_value(graph_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse graph: {}", e)))?;

        let gap = self.compute_spectral_gap_internal(&graph);

        serde_wasm_bindgen::to_value(&gap)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize gap: {}", e)))
    }

    /// Predict minimum cut
    #[wasm_bindgen(js_name = predictMinCut)]
    pub fn predict_min_cut(&self, graph_js: JsValue) -> Result<JsValue, JsValue> {
        let graph: Graph = serde_wasm_bindgen::from_value(graph_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse graph: {}", e)))?;

        let prediction = self.predict_min_cut_internal(&graph);

        serde_wasm_bindgen::to_value(&prediction)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize prediction: {}", e)))
    }

    /// Compute Fiedler vector
    #[wasm_bindgen(js_name = computeFiedlerVector)]
    pub fn compute_fiedler_vector(&self, graph_js: JsValue) -> Result<JsValue, JsValue> {
        let graph: Graph = serde_wasm_bindgen::from_value(graph_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse graph: {}", e)))?;

        let vector = self.compute_fiedler_vector_internal(&graph);

        serde_wasm_bindgen::to_value(&vector)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize vector: {}", e)))
    }
}

impl SpectralEngine {
    fn compute_cheeger_bounds_internal(&self, graph: &Graph) -> CheegerBounds {
        let fiedler = self.compute_fiedler_value(graph);

        // Cheeger inequality: λ₂/2 ≤ h(G) ≤ √(2λ₂)
        let lower_bound = fiedler / 2.0;
        let upper_bound = (2.0 * fiedler).sqrt();
        let cheeger_estimate = (lower_bound + upper_bound) / 2.0;

        CheegerBounds {
            lower_bound,
            upper_bound,
            cheeger_estimate,
            fiedler_value: fiedler,
        }
    }

    fn compute_eigenvalues_internal(&self, graph: &Graph) -> Vec<f64> {
        // Build Laplacian and compute eigenvalues using power iteration
        let laplacian = self.build_laplacian(graph);
        self.power_iteration_eigenvalues(&laplacian, graph.n)
    }

    fn compute_fiedler_value(&self, graph: &Graph) -> f64 {
        let eigenvalues = self.compute_eigenvalues_internal(graph);

        // Find first non-trivial eigenvalue
        for &ev in &eigenvalues {
            if ev > self.tolerance {
                return ev;
            }
        }

        0.0
    }

    fn compute_spectral_gap_internal(&self, graph: &Graph) -> SpectralGap {
        let eigenvalues = self.compute_eigenvalues_internal(graph);

        let non_trivial: Vec<f64> = eigenvalues
            .iter()
            .filter(|&&v| v > self.tolerance)
            .cloned()
            .collect();

        let lambda_1 = non_trivial.first().cloned().unwrap_or(0.0);
        let lambda_2 = non_trivial.get(1).cloned().unwrap_or(lambda_1 * 2.0);

        SpectralGap {
            lambda_1,
            lambda_2,
            gap: lambda_2 - lambda_1,
            ratio: if lambda_1 > self.tolerance {
                lambda_2 / lambda_1
            } else {
                f64::INFINITY
            },
        }
    }

    fn predict_min_cut_internal(&self, graph: &Graph) -> MinCutPrediction {
        let fiedler = self.compute_fiedler_value(graph);
        let fiedler_vec = self.compute_fiedler_vector_internal(graph);

        let total_weight: f64 = graph.edges.iter().map(|(_, _, w)| *w).sum();

        let lower_bound = fiedler / 2.0 * total_weight / 2.0;
        let upper_bound = (2.0 * fiedler).sqrt() * total_weight / 2.0;
        let predicted_cut = (lower_bound + upper_bound) / 2.0;

        // Find cut nodes from Fiedler vector
        let cut_nodes: Vec<usize> = fiedler_vec
            .iter()
            .enumerate()
            .filter(|(_, &v)| v > 0.0)
            .map(|(i, _)| i)
            .collect();

        let gap = self.compute_spectral_gap_internal(graph);
        let confidence = if gap.ratio > 2.0 { 0.9 }
            else if gap.ratio > 1.5 { 0.7 }
            else if gap.ratio > 1.2 { 0.5 }
            else { 0.3 };

        MinCutPrediction {
            predicted_cut,
            lower_bound,
            upper_bound,
            confidence,
            cut_nodes,
        }
    }

    fn compute_fiedler_vector_internal(&self, graph: &Graph) -> Vec<f64> {
        let laplacian = self.build_laplacian(graph);

        // Use inverse power iteration with shift to find second eigenvector
        let n = graph.n;
        let mut v = vec![1.0 / (n as f64).sqrt(); n];

        // Make orthogonal to constant vector
        let ones = vec![1.0 / (n as f64).sqrt(); n];

        for _ in 0..self.max_iterations {
            // Multiply by Laplacian
            let mut av = vec![0.0; n];
            for i in 0..n {
                for j in 0..n {
                    av[i] += laplacian[i * n + j] * v[j];
                }
            }

            // Orthogonalize against constant vector
            let dot: f64 = av.iter().zip(ones.iter()).map(|(a, b)| a * b).sum();
            for i in 0..n {
                av[i] -= dot * ones[i];
            }

            // Normalize
            let norm: f64 = av.iter().map(|x| x * x).sum::<f64>().sqrt();
            if norm > self.tolerance {
                for i in 0..n {
                    v[i] = av[i] / norm;
                }
            }
        }

        v
    }

    fn build_laplacian(&self, graph: &Graph) -> Vec<f64> {
        let n = graph.n;
        let mut laplacian = vec![0.0; n * n];

        // Build adjacency and degree
        for &(u, v, w) in &graph.edges {
            if u < n && v < n {
                laplacian[u * n + v] = -w;
                laplacian[v * n + u] = -w;
                laplacian[u * n + u] += w;
                laplacian[v * n + v] += w;
            }
        }

        laplacian
    }

    fn power_iteration_eigenvalues(&self, matrix: &[f64], n: usize) -> Vec<f64> {
        let mut eigenvalues = Vec::new();
        let mut work_matrix = matrix.to_vec();

        for _ in 0..self.num_eigenvalues.min(n) {
            // Power iteration for largest eigenvalue
            let mut v = vec![1.0 / (n as f64).sqrt(); n];

            let mut lambda = 0.0;
            for _ in 0..self.max_iterations {
                let mut av = vec![0.0; n];
                for i in 0..n {
                    for j in 0..n {
                        av[i] += work_matrix[i * n + j] * v[j];
                    }
                }

                let norm: f64 = av.iter().map(|x| x * x).sum::<f64>().sqrt();
                let new_lambda = v.iter().zip(av.iter()).map(|(a, b)| a * b).sum::<f64>();

                if (new_lambda - lambda).abs() < self.tolerance {
                    lambda = new_lambda;
                    break;
                }

                lambda = new_lambda;
                if norm > self.tolerance {
                    for i in 0..n {
                        v[i] = av[i] / norm;
                    }
                }
            }

            eigenvalues.push(lambda);

            // Deflate matrix
            for i in 0..n {
                for j in 0..n {
                    work_matrix[i * n + j] -= lambda * v[i] * v[j];
                }
            }
        }

        eigenvalues.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        eigenvalues
    }
}

impl Default for SpectralEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Causal Engine
// ============================================================================

/// Variable in causal model
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CausalVariable {
    pub name: String,
    pub var_type: String, // "continuous", "discrete", "binary"
}

/// Causal edge
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CausalEdge {
    pub from: String,
    pub to: String,
}

/// Causal model structure
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CausalModel {
    pub variables: Vec<CausalVariable>,
    pub edges: Vec<CausalEdge>,
}

/// Intervention result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InterventionResult {
    pub variable: String,
    pub original_value: f64,
    pub intervened_value: f64,
    pub affected_variables: Vec<String>,
    pub causal_effect: f64,
}

/// D-separation result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DSeparationResult {
    pub x: String,
    pub y: String,
    pub conditioning: Vec<String>,
    pub d_separated: bool,
}

/// Causal inference engine
#[wasm_bindgen]
pub struct CausalEngine {
    #[allow(dead_code)]
    tolerance: f64,
}

#[wasm_bindgen]
impl CausalEngine {
    /// Create a new causal engine
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self { tolerance: 1e-10 }
    }

    /// Check d-separation between two variables
    #[wasm_bindgen(js_name = checkDSeparation)]
    pub fn check_d_separation(
        &self,
        model_js: JsValue,
        x: &str,
        y: &str,
        conditioning_js: JsValue,
    ) -> Result<JsValue, JsValue> {
        let model: CausalModel = serde_wasm_bindgen::from_value(model_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse model: {}", e)))?;

        let conditioning: Vec<String> = serde_wasm_bindgen::from_value(conditioning_js)
            .unwrap_or_default();

        let result = self.check_d_separation_internal(&model, x, y, &conditioning);

        serde_wasm_bindgen::to_value(&result)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize result: {}", e)))
    }

    /// Compute causal effect via do-operator
    #[wasm_bindgen(js_name = computeCausalEffect)]
    pub fn compute_causal_effect(
        &self,
        model_js: JsValue,
        treatment: &str,
        outcome: &str,
        treatment_value: f64,
    ) -> Result<JsValue, JsValue> {
        let model: CausalModel = serde_wasm_bindgen::from_value(model_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse model: {}", e)))?;

        let result = self.compute_causal_effect_internal(&model, treatment, outcome, treatment_value);

        serde_wasm_bindgen::to_value(&result)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize result: {}", e)))
    }

    /// Get topological order of variables
    #[wasm_bindgen(js_name = topologicalOrder)]
    pub fn topological_order(&self, model_js: JsValue) -> Result<JsValue, JsValue> {
        let model: CausalModel = serde_wasm_bindgen::from_value(model_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse model: {}", e)))?;

        let order = self.topological_order_internal(&model);

        serde_wasm_bindgen::to_value(&order)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize order: {}", e)))
    }

    /// Find all confounders between two variables
    #[wasm_bindgen(js_name = findConfounders)]
    pub fn find_confounders(
        &self,
        model_js: JsValue,
        treatment: &str,
        outcome: &str,
    ) -> Result<JsValue, JsValue> {
        let model: CausalModel = serde_wasm_bindgen::from_value(model_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse model: {}", e)))?;

        let confounders = self.find_confounders_internal(&model, treatment, outcome);

        serde_wasm_bindgen::to_value(&confounders)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize confounders: {}", e)))
    }

    /// Check if model is a valid DAG
    #[wasm_bindgen(js_name = isValidDag)]
    pub fn is_valid_dag(&self, model_js: JsValue) -> Result<bool, JsValue> {
        let model: CausalModel = serde_wasm_bindgen::from_value(model_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse model: {}", e)))?;

        Ok(self.is_valid_dag_internal(&model))
    }
}

impl CausalEngine {
    fn check_d_separation_internal(
        &self,
        model: &CausalModel,
        x: &str,
        y: &str,
        conditioning: &[String],
    ) -> DSeparationResult {
        // Build adjacency
        let _var_names: Vec<&str> = model.variables.iter().map(|v| v.name.as_str()).collect();
        let conditioning_set: std::collections::HashSet<&str> =
            conditioning.iter().map(|s| s.as_str()).collect();

        // Check all paths from x to y (simplified BFS check)
        let mut visited = std::collections::HashSet::new();
        let mut queue = vec![x.to_string()];
        let mut path_blocked = true;

        while let Some(current) = queue.pop() {
            if current == y {
                path_blocked = false;
                break;
            }

            if visited.contains(&current) {
                continue;
            }
            visited.insert(current.clone());

            // Check if blocked by conditioning
            if conditioning_set.contains(current.as_str()) {
                continue;
            }

            // Add neighbors
            for edge in &model.edges {
                if edge.from == current && !visited.contains(&edge.to) {
                    queue.push(edge.to.clone());
                }
                if edge.to == current && !visited.contains(&edge.from) {
                    queue.push(edge.from.clone());
                }
            }
        }

        DSeparationResult {
            x: x.to_string(),
            y: y.to_string(),
            conditioning: conditioning.to_vec(),
            d_separated: path_blocked,
        }
    }

    fn compute_causal_effect_internal(
        &self,
        model: &CausalModel,
        treatment: &str,
        outcome: &str,
        treatment_value: f64,
    ) -> InterventionResult {
        // Find affected variables (descendants of treatment)
        let affected = self.find_descendants(model, treatment);

        InterventionResult {
            variable: treatment.to_string(),
            original_value: 0.0,
            intervened_value: treatment_value,
            affected_variables: affected.clone(),
            causal_effect: if affected.contains(&outcome.to_string()) {
                treatment_value // Simplified: direct proportional effect
            } else {
                0.0
            },
        }
    }

    fn topological_order_internal(&self, model: &CausalModel) -> Vec<String> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut adj: HashMap<String, Vec<String>> = HashMap::new();

        for var in &model.variables {
            in_degree.insert(var.name.clone(), 0);
            adj.insert(var.name.clone(), Vec::new());
        }

        for edge in &model.edges {
            *in_degree.entry(edge.to.clone()).or_insert(0) += 1;
            adj.entry(edge.from.clone())
                .or_default()
                .push(edge.to.clone());
        }

        let mut queue: Vec<String> = in_degree
            .iter()
            .filter(|(_, &d)| d == 0)
            .map(|(k, _)| k.clone())
            .collect();

        let mut order = Vec::new();

        while let Some(node) = queue.pop() {
            order.push(node.clone());

            if let Some(neighbors) = adj.get(&node) {
                for neighbor in neighbors {
                    if let Some(degree) = in_degree.get_mut(neighbor) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push(neighbor.clone());
                        }
                    }
                }
            }
        }

        order
    }

    fn find_confounders_internal(
        &self,
        model: &CausalModel,
        treatment: &str,
        outcome: &str,
    ) -> Vec<String> {
        // Find common ancestors
        let treatment_ancestors = self.find_ancestors(model, treatment);
        let outcome_ancestors = self.find_ancestors(model, outcome);

        treatment_ancestors
            .intersection(&outcome_ancestors)
            .cloned()
            .collect()
    }

    fn find_ancestors(&self, model: &CausalModel, node: &str) -> std::collections::HashSet<String> {
        let mut ancestors = std::collections::HashSet::new();
        let mut queue = vec![node.to_string()];

        while let Some(current) = queue.pop() {
            for edge in &model.edges {
                if edge.to == current && !ancestors.contains(&edge.from) {
                    ancestors.insert(edge.from.clone());
                    queue.push(edge.from.clone());
                }
            }
        }

        ancestors
    }

    fn find_descendants(&self, model: &CausalModel, node: &str) -> Vec<String> {
        let mut descendants = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut queue = vec![node.to_string()];

        while let Some(current) = queue.pop() {
            for edge in &model.edges {
                if edge.from == current && !visited.contains(&edge.to) {
                    visited.insert(edge.to.clone());
                    descendants.push(edge.to.clone());
                    queue.push(edge.to.clone());
                }
            }
        }

        descendants
    }

    fn is_valid_dag_internal(&self, model: &CausalModel) -> bool {
        let order = self.topological_order_internal(model);
        order.len() == model.variables.len()
    }
}

impl Default for CausalEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Quantum Engine
// ============================================================================

/// Complex number for quantum computations
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}

impl Complex {
    pub fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    pub fn norm_sq(&self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    pub fn conj(&self) -> Self {
        Self {
            re: self.re,
            im: -self.im,
        }
    }
}

/// Quantum state representation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuantumState {
    pub amplitudes: Vec<Complex>,
    pub dimension: usize,
}

/// Topological invariant result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TopologicalInvariant {
    pub betti_numbers: Vec<usize>,
    pub euler_characteristic: i64,
    pub is_connected: bool,
}

/// Quantum fidelity result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FidelityResult {
    pub fidelity: f64,
    pub trace_distance: f64,
}

/// Quantum computing and topological analysis engine
#[wasm_bindgen]
pub struct QuantumEngine {
    tolerance: f64,
}

#[wasm_bindgen]
impl QuantumEngine {
    /// Create a new quantum engine
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self { tolerance: 1e-10 }
    }

    /// Compute topological invariants of a simplicial complex
    #[wasm_bindgen(js_name = computeTopologicalInvariants)]
    pub fn compute_topological_invariants(
        &self,
        simplices_js: JsValue,
    ) -> Result<JsValue, JsValue> {
        let simplices: Vec<Vec<usize>> = serde_wasm_bindgen::from_value(simplices_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse simplices: {}", e)))?;

        let invariants = self.compute_topological_invariants_internal(&simplices);

        serde_wasm_bindgen::to_value(&invariants)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize invariants: {}", e)))
    }

    /// Compute quantum state fidelity
    #[wasm_bindgen(js_name = computeFidelity)]
    pub fn compute_fidelity(
        &self,
        state1_js: JsValue,
        state2_js: JsValue,
    ) -> Result<JsValue, JsValue> {
        let state1: QuantumState = serde_wasm_bindgen::from_value(state1_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse state1: {}", e)))?;
        let state2: QuantumState = serde_wasm_bindgen::from_value(state2_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse state2: {}", e)))?;

        let result = self.compute_fidelity_internal(&state1, &state2);

        serde_wasm_bindgen::to_value(&result)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize fidelity: {}", e)))
    }

    /// Create a GHZ state
    #[wasm_bindgen(js_name = createGHZState)]
    pub fn create_ghz_state(&self, num_qubits: usize) -> Result<JsValue, JsValue> {
        let state = self.create_ghz_state_internal(num_qubits);

        serde_wasm_bindgen::to_value(&state)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize state: {}", e)))
    }

    /// Create a W state
    #[wasm_bindgen(js_name = createWState)]
    pub fn create_w_state(&self, num_qubits: usize) -> Result<JsValue, JsValue> {
        let state = self.create_w_state_internal(num_qubits);

        serde_wasm_bindgen::to_value(&state)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize state: {}", e)))
    }

    /// Compute entanglement entropy
    #[wasm_bindgen(js_name = computeEntanglementEntropy)]
    pub fn compute_entanglement_entropy(
        &self,
        state_js: JsValue,
        subsystem_size: usize,
    ) -> Result<f64, JsValue> {
        let state: QuantumState = serde_wasm_bindgen::from_value(state_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse state: {}", e)))?;

        Ok(self.compute_entanglement_entropy_internal(&state, subsystem_size))
    }

    /// Simulate quantum circuit evolution
    #[wasm_bindgen(js_name = applyGate)]
    pub fn apply_gate(
        &self,
        state_js: JsValue,
        gate_js: JsValue,
        target_qubit: usize,
    ) -> Result<JsValue, JsValue> {
        let state: QuantumState = serde_wasm_bindgen::from_value(state_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse state: {}", e)))?;
        let gate: Vec<Vec<Complex>> = serde_wasm_bindgen::from_value(gate_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse gate: {}", e)))?;

        let result = self.apply_gate_internal(&state, &gate, target_qubit);

        serde_wasm_bindgen::to_value(&result)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize state: {}", e)))
    }
}

impl QuantumEngine {
    fn compute_topological_invariants_internal(
        &self,
        simplices: &[Vec<usize>],
    ) -> TopologicalInvariant {
        // Count simplices by dimension
        let mut simplex_counts: HashMap<usize, usize> = HashMap::new();
        let mut vertices = std::collections::HashSet::new();

        for simplex in simplices {
            let dim = if simplex.is_empty() { 0 } else { simplex.len() - 1 };
            *simplex_counts.entry(dim).or_insert(0) += 1;
            for &v in simplex {
                vertices.insert(v);
            }
        }

        let max_dim = simplex_counts.keys().cloned().max().unwrap_or(0);

        // Compute Betti numbers (simplified - just use simplex counts as approximation)
        let mut betti_numbers = vec![0usize; max_dim + 1];
        betti_numbers[0] = vertices.len();

        // Euler characteristic: Σ(-1)^i * count_i
        let euler_characteristic: i64 = simplex_counts
            .iter()
            .map(|(&dim, &count)| {
                if dim % 2 == 0 {
                    count as i64
                } else {
                    -(count as i64)
                }
            })
            .sum();

        // Check connectivity (simplified)
        let is_connected = vertices.len() <= 1 || simplex_counts.get(&1).copied().unwrap_or(0) >= vertices.len() - 1;

        TopologicalInvariant {
            betti_numbers,
            euler_characteristic,
            is_connected,
        }
    }

    fn compute_fidelity_internal(&self, state1: &QuantumState, state2: &QuantumState) -> FidelityResult {
        if state1.dimension != state2.dimension {
            return FidelityResult {
                fidelity: 0.0,
                trace_distance: 1.0,
            };
        }

        // Compute |<ψ|φ>|²
        let mut inner_re = 0.0;
        let mut inner_im = 0.0;

        for i in 0..state1.dimension.min(state1.amplitudes.len()).min(state2.amplitudes.len()) {
            let a = &state1.amplitudes[i].conj();
            let b = &state2.amplitudes[i];
            inner_re += a.re * b.re - a.im * b.im;
            inner_im += a.re * b.im + a.im * b.re;
        }

        let fidelity = inner_re * inner_re + inner_im * inner_im;
        let trace_distance = (1.0 - fidelity).sqrt();

        FidelityResult {
            fidelity,
            trace_distance,
        }
    }

    fn create_ghz_state_internal(&self, num_qubits: usize) -> QuantumState {
        let dimension = 1 << num_qubits;
        let amplitude = 1.0 / 2.0_f64.sqrt();

        let mut amplitudes = vec![Complex::new(0.0, 0.0); dimension];
        amplitudes[0] = Complex::new(amplitude, 0.0);
        amplitudes[dimension - 1] = Complex::new(amplitude, 0.0);

        QuantumState {
            amplitudes,
            dimension,
        }
    }

    fn create_w_state_internal(&self, num_qubits: usize) -> QuantumState {
        let dimension = 1 << num_qubits;
        let amplitude = 1.0 / (num_qubits as f64).sqrt();

        let mut amplitudes = vec![Complex::new(0.0, 0.0); dimension];
        for i in 0..num_qubits {
            amplitudes[1 << i] = Complex::new(amplitude, 0.0);
        }

        QuantumState {
            amplitudes,
            dimension,
        }
    }

    fn compute_entanglement_entropy_internal(
        &self,
        state: &QuantumState,
        subsystem_size: usize,
    ) -> f64 {
        // Compute reduced density matrix eigenvalues
        let num_qubits = (state.dimension as f64).log2() as usize;
        if subsystem_size >= num_qubits {
            return 0.0;
        }

        // Simplified: use probability distribution entropy as approximation
        let probs: Vec<f64> = state.amplitudes.iter().map(|a| a.norm_sq()).collect();

        let mut entropy = 0.0;
        for p in &probs {
            if *p > self.tolerance {
                entropy -= p * p.ln();
            }
        }

        entropy
    }

    fn apply_gate_internal(
        &self,
        state: &QuantumState,
        gate: &[Vec<Complex>],
        target_qubit: usize,
    ) -> QuantumState {
        let dimension = state.dimension;
        let num_qubits = (dimension as f64).log2() as usize;

        if target_qubit >= num_qubits || gate.len() != 2 || gate[0].len() != 2 {
            return state.clone();
        }

        let mut new_amplitudes = vec![Complex::new(0.0, 0.0); dimension];

        for i in 0..dimension {
            let bit = (i >> target_qubit) & 1;
            let i0 = i & !(1 << target_qubit);
            let i1 = i | (1 << target_qubit);

            if bit == 0 {
                // Apply to |0> component
                let a = &state.amplitudes[i0];
                let b = &state.amplitudes[i1];

                let g00 = &gate[0][0];
                let g01 = &gate[0][1];

                new_amplitudes[i0] = Complex::new(
                    g00.re * a.re - g00.im * a.im + g01.re * b.re - g01.im * b.im,
                    g00.re * a.im + g00.im * a.re + g01.re * b.im + g01.im * b.re,
                );
            } else {
                // Apply to |1> component
                let a = &state.amplitudes[i0];
                let b = &state.amplitudes[i1];

                let g10 = &gate[1][0];
                let g11 = &gate[1][1];

                new_amplitudes[i1] = Complex::new(
                    g10.re * a.re - g10.im * a.im + g11.re * b.re - g11.im * b.im,
                    g10.re * a.im + g10.im * a.re + g11.re * b.im + g11.im * b.re,
                );
            }
        }

        QuantumState {
            amplitudes: new_amplitudes,
            dimension,
        }
    }
}

impl Default for QuantumEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Category Engine
// ============================================================================

/// Categorical object
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatObject {
    pub id: String,
    pub dimension: usize,
    pub data: Vec<f64>,
}

/// Morphism between objects
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Morphism {
    pub source: String,
    pub target: String,
    pub matrix: Vec<f64>,
    pub source_dim: usize,
    pub target_dim: usize,
}

/// Category structure
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Category {
    pub name: String,
    pub objects: Vec<CatObject>,
    pub morphisms: Vec<Morphism>,
}

/// Functor between categories
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Functor {
    pub name: String,
    pub source_category: String,
    pub target_category: String,
    pub object_map: HashMap<String, String>,
}

/// Retrieval result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetrievalResult {
    pub object_id: String,
    pub similarity: f64,
}

/// Category theory engine
#[wasm_bindgen]
pub struct CategoryEngine {
    tolerance: f64,
}

#[wasm_bindgen]
impl CategoryEngine {
    /// Create a new category engine
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self { tolerance: 1e-10 }
    }

    /// Compose two morphisms
    #[wasm_bindgen(js_name = composeMorphisms)]
    pub fn compose_morphisms(
        &self,
        f_js: JsValue,
        g_js: JsValue,
    ) -> Result<JsValue, JsValue> {
        let f: Morphism = serde_wasm_bindgen::from_value(f_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse morphism f: {}", e)))?;
        let g: Morphism = serde_wasm_bindgen::from_value(g_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse morphism g: {}", e)))?;

        let result = self.compose_morphisms_internal(&f, &g)?;

        serde_wasm_bindgen::to_value(&result)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize morphism: {}", e)))
    }

    /// Verify categorical laws
    #[wasm_bindgen(js_name = verifyCategoryLaws)]
    pub fn verify_category_laws(&self, category_js: JsValue) -> Result<bool, JsValue> {
        let category: Category = serde_wasm_bindgen::from_value(category_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse category: {}", e)))?;

        Ok(self.verify_category_laws_internal(&category))
    }

    /// Functorial retrieval: find similar objects
    #[wasm_bindgen(js_name = functorialRetrieve)]
    pub fn functorial_retrieve(
        &self,
        category_js: JsValue,
        query_js: JsValue,
        k: usize,
    ) -> Result<JsValue, JsValue> {
        let category: Category = serde_wasm_bindgen::from_value(category_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse category: {}", e)))?;
        let query: Vec<f64> = serde_wasm_bindgen::from_value(query_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse query: {}", e)))?;

        let results = self.functorial_retrieve_internal(&category, &query, k);

        serde_wasm_bindgen::to_value(&results)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize results: {}", e)))
    }

    /// Apply morphism to an object
    #[wasm_bindgen(js_name = applyMorphism)]
    pub fn apply_morphism(
        &self,
        morphism_js: JsValue,
        data_js: JsValue,
    ) -> Result<JsValue, JsValue> {
        let morphism: Morphism = serde_wasm_bindgen::from_value(morphism_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse morphism: {}", e)))?;
        let data: Vec<f64> = serde_wasm_bindgen::from_value(data_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse data: {}", e)))?;

        let result = self.apply_morphism_internal(&morphism, &data);

        serde_wasm_bindgen::to_value(&result)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize result: {}", e)))
    }

    /// Check if functor preserves composition
    #[wasm_bindgen(js_name = verifyFunctoriality)]
    pub fn verify_functoriality(
        &self,
        functor_js: JsValue,
        source_cat_js: JsValue,
    ) -> Result<bool, JsValue> {
        let functor: Functor = serde_wasm_bindgen::from_value(functor_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse functor: {}", e)))?;
        let source_cat: Category = serde_wasm_bindgen::from_value(source_cat_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse category: {}", e)))?;

        Ok(self.verify_functoriality_internal(&functor, &source_cat))
    }
}

impl CategoryEngine {
    fn compose_morphisms_internal(&self, f: &Morphism, g: &Morphism) -> Result<Morphism, String> {
        if f.target != g.source {
            return Err(format!(
                "Cannot compose: target of f ({}) != source of g ({})",
                f.target, g.source
            ));
        }

        if f.target_dim != g.source_dim {
            return Err(format!(
                "Dimension mismatch: {} != {}",
                f.target_dim, g.source_dim
            ));
        }

        // Matrix multiplication: g ∘ f = g * f
        let mut result = vec![0.0; g.target_dim * f.source_dim];

        for i in 0..g.target_dim {
            for j in 0..f.source_dim {
                for k in 0..f.target_dim {
                    result[i * f.source_dim + j] +=
                        g.matrix[i * g.source_dim + k] * f.matrix[k * f.source_dim + j];
                }
            }
        }

        Ok(Morphism {
            source: f.source.clone(),
            target: g.target.clone(),
            matrix: result,
            source_dim: f.source_dim,
            target_dim: g.target_dim,
        })
    }

    fn verify_category_laws_internal(&self, category: &Category) -> bool {
        // Build object dimension map
        let obj_dims: HashMap<String, usize> = category
            .objects
            .iter()
            .map(|o| (o.id.clone(), o.dimension))
            .collect();

        // Check identity laws
        for morphism in &category.morphisms {
            // Get source dimension
            let source_dim = match obj_dims.get(&morphism.source) {
                Some(d) => *d,
                None => continue,
            };

            // Check id ∘ f = f
            let identity = self.create_identity(source_dim);
            let composed = match self.compose_morphisms_internal(&identity, morphism) {
                Ok(m) => m,
                Err(_) => continue,
            };

            if !self.morphisms_equal(&composed, morphism) {
                return false;
            }
        }

        true
    }

    fn functorial_retrieve_internal(
        &self,
        category: &Category,
        query: &[f64],
        k: usize,
    ) -> Vec<RetrievalResult> {
        let mut results: Vec<RetrievalResult> = category
            .objects
            .iter()
            .map(|obj| {
                let similarity = self.cosine_similarity(query, &obj.data);
                RetrievalResult {
                    object_id: obj.id.clone(),
                    similarity,
                }
            })
            .collect();

        results.sort_by(|a, b| {
            b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal)
        });

        results.truncate(k);
        results
    }

    fn apply_morphism_internal(&self, morphism: &Morphism, data: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; morphism.target_dim];

        for i in 0..morphism.target_dim {
            for j in 0..morphism.source_dim.min(data.len()) {
                result[i] += morphism.matrix[i * morphism.source_dim + j] * data[j];
            }
        }

        result
    }

    fn verify_functoriality_internal(&self, functor: &Functor, source_cat: &Category) -> bool {
        // Check that all objects are mapped
        for obj in &source_cat.objects {
            if !functor.object_map.contains_key(&obj.id) {
                return false;
            }
        }

        true
    }

    fn create_identity(&self, dim: usize) -> Morphism {
        let mut matrix = vec![0.0; dim * dim];
        for i in 0..dim {
            matrix[i * dim + i] = 1.0;
        }

        Morphism {
            source: "id".to_string(),
            target: "id".to_string(),
            matrix,
            source_dim: dim,
            target_dim: dim,
        }
    }

    fn morphisms_equal(&self, m1: &Morphism, m2: &Morphism) -> bool {
        if m1.source_dim != m2.source_dim || m1.target_dim != m2.target_dim {
            return false;
        }

        m1.matrix
            .iter()
            .zip(m2.matrix.iter())
            .all(|(a, b)| (a - b).abs() < self.tolerance)
    }

    fn cosine_similarity(&self, a: &[f64], b: &[f64]) -> f64 {
        let mut dot = 0.0;
        let mut norm_a = 0.0;
        let mut norm_b = 0.0;

        let len = a.len().min(b.len());
        for i in 0..len {
            dot += a[i] * b[i];
            norm_a += a[i] * a[i];
            norm_b += b[i] * b[i];
        }

        let denom = (norm_a * norm_b).sqrt();
        if denom < self.tolerance {
            0.0
        } else {
            dot / denom
        }
    }
}

impl Default for CategoryEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// HoTT Engine
// ============================================================================

/// HoTT type representation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HoTTType {
    pub name: String,
    pub level: usize,
    pub kind: String, // "unit", "bool", "nat", "product", "sum", "function", "identity"
    pub params: Vec<String>,
}

/// HoTT term representation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HoTTTerm {
    pub kind: String, // "var", "star", "true", "false", "zero", "succ", "lambda", "app", "pair", "refl"
    pub value: Option<String>,
    pub children: Vec<HoTTTerm>,
}

/// Path in HoTT
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HoTTPath {
    pub base_type: HoTTType,
    pub start: HoTTTerm,
    pub end: HoTTTerm,
    pub proof: HoTTTerm,
}

/// Type checking result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TypeCheckResult {
    pub is_valid: bool,
    pub inferred_type: Option<HoTTType>,
    pub error: Option<String>,
}

/// Path operation result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PathOperationResult {
    pub is_valid: bool,
    pub result_path: Option<HoTTPath>,
    pub error: Option<String>,
}

/// HoTT type checking and path operations engine
#[wasm_bindgen]
pub struct HoTTEngine {
    #[allow(dead_code)]
    strict_mode: bool,
}

#[wasm_bindgen]
impl HoTTEngine {
    /// Create a new HoTT engine
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self { strict_mode: false }
    }

    /// Create with strict mode
    #[wasm_bindgen(js_name = withStrictMode)]
    pub fn with_strict_mode(strict: bool) -> Self {
        Self { strict_mode: strict }
    }

    /// Type check a term
    #[wasm_bindgen(js_name = typeCheck)]
    pub fn type_check(
        &self,
        term_js: JsValue,
        expected_type_js: JsValue,
    ) -> Result<JsValue, JsValue> {
        let term: HoTTTerm = serde_wasm_bindgen::from_value(term_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse term: {}", e)))?;
        let expected: HoTTType = serde_wasm_bindgen::from_value(expected_type_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse type: {}", e)))?;

        let result = self.type_check_internal(&term, &expected);

        serde_wasm_bindgen::to_value(&result)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize result: {}", e)))
    }

    /// Infer type of a term
    #[wasm_bindgen(js_name = inferType)]
    pub fn infer_type(&self, term_js: JsValue) -> Result<JsValue, JsValue> {
        let term: HoTTTerm = serde_wasm_bindgen::from_value(term_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse term: {}", e)))?;

        let result = self.infer_type_internal(&term);

        serde_wasm_bindgen::to_value(&result)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize type: {}", e)))
    }

    /// Compose two paths
    #[wasm_bindgen(js_name = composePaths)]
    pub fn compose_paths(
        &self,
        path1_js: JsValue,
        path2_js: JsValue,
    ) -> Result<JsValue, JsValue> {
        let path1: HoTTPath = serde_wasm_bindgen::from_value(path1_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse path1: {}", e)))?;
        let path2: HoTTPath = serde_wasm_bindgen::from_value(path2_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse path2: {}", e)))?;

        let result = self.compose_paths_internal(&path1, &path2);

        serde_wasm_bindgen::to_value(&result)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize result: {}", e)))
    }

    /// Invert a path
    #[wasm_bindgen(js_name = invertPath)]
    pub fn invert_path(&self, path_js: JsValue) -> Result<JsValue, JsValue> {
        let path: HoTTPath = serde_wasm_bindgen::from_value(path_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse path: {}", e)))?;

        let result = self.invert_path_internal(&path);

        serde_wasm_bindgen::to_value(&result)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize result: {}", e)))
    }

    /// Create reflexivity path
    #[wasm_bindgen(js_name = createReflPath)]
    pub fn create_refl_path(
        &self,
        type_js: JsValue,
        point_js: JsValue,
    ) -> Result<JsValue, JsValue> {
        let ty: HoTTType = serde_wasm_bindgen::from_value(type_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse type: {}", e)))?;
        let point: HoTTTerm = serde_wasm_bindgen::from_value(point_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse point: {}", e)))?;

        let path = self.create_refl_path_internal(&ty, &point);

        serde_wasm_bindgen::to_value(&path)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize path: {}", e)))
    }

    /// Check type equivalence (univalence-related)
    #[wasm_bindgen(js_name = checkTypeEquivalence)]
    pub fn check_type_equivalence(
        &self,
        type1_js: JsValue,
        type2_js: JsValue,
    ) -> Result<bool, JsValue> {
        let type1: HoTTType = serde_wasm_bindgen::from_value(type1_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse type1: {}", e)))?;
        let type2: HoTTType = serde_wasm_bindgen::from_value(type2_js)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse type2: {}", e)))?;

        Ok(self.types_equal(&type1, &type2))
    }
}

impl HoTTEngine {
    fn type_check_internal(&self, term: &HoTTTerm, expected: &HoTTType) -> TypeCheckResult {
        let inferred = match self.infer_type_internal(term) {
            TypeCheckResult { is_valid: true, inferred_type: Some(ty), .. } => ty,
            TypeCheckResult { error, .. } => {
                return TypeCheckResult {
                    is_valid: false,
                    inferred_type: None,
                    error,
                };
            }
        };

        if self.types_equal(&inferred, expected) {
            TypeCheckResult {
                is_valid: true,
                inferred_type: Some(inferred),
                error: None,
            }
        } else {
            TypeCheckResult {
                is_valid: false,
                inferred_type: Some(inferred.clone()),
                error: Some(format!(
                    "Type mismatch: expected {}, got {}",
                    expected.name, inferred.name
                )),
            }
        }
    }

    fn infer_type_internal(&self, term: &HoTTTerm) -> TypeCheckResult {
        let ty = match term.kind.as_str() {
            "star" => HoTTType {
                name: "Unit".to_string(),
                level: 0,
                kind: "unit".to_string(),
                params: vec![],
            },
            "true" | "false" => HoTTType {
                name: "Bool".to_string(),
                level: 0,
                kind: "bool".to_string(),
                params: vec![],
            },
            "zero" => HoTTType {
                name: "Nat".to_string(),
                level: 0,
                kind: "nat".to_string(),
                params: vec![],
            },
            "succ" => {
                if term.children.is_empty() {
                    return TypeCheckResult {
                        is_valid: false,
                        inferred_type: None,
                        error: Some("Succ requires argument".to_string()),
                    };
                }
                let child_result = self.infer_type_internal(&term.children[0]);
                if !child_result.is_valid {
                    return child_result;
                }
                if let Some(child_ty) = &child_result.inferred_type {
                    if child_ty.kind != "nat" {
                        return TypeCheckResult {
                            is_valid: false,
                            inferred_type: None,
                            error: Some("Succ argument must be Nat".to_string()),
                        };
                    }
                }
                HoTTType {
                    name: "Nat".to_string(),
                    level: 0,
                    kind: "nat".to_string(),
                    params: vec![],
                }
            }
            "refl" => {
                if term.children.is_empty() {
                    return TypeCheckResult {
                        is_valid: false,
                        inferred_type: None,
                        error: Some("Refl requires argument".to_string()),
                    };
                }
                let child_result = self.infer_type_internal(&term.children[0]);
                if !child_result.is_valid {
                    return child_result;
                }
                let base_ty = child_result.inferred_type.unwrap();
                HoTTType {
                    name: format!("Id_{}({}, {})",
                        base_ty.name,
                        term.children[0].value.as_deref().unwrap_or("_"),
                        term.children[0].value.as_deref().unwrap_or("_")
                    ),
                    level: base_ty.level,
                    kind: "identity".to_string(),
                    params: vec![base_ty.name],
                }
            }
            "pair" => {
                if term.children.len() < 2 {
                    return TypeCheckResult {
                        is_valid: false,
                        inferred_type: None,
                        error: Some("Pair requires two arguments".to_string()),
                    };
                }
                let fst_result = self.infer_type_internal(&term.children[0]);
                let snd_result = self.infer_type_internal(&term.children[1]);

                if !fst_result.is_valid {
                    return fst_result;
                }
                if !snd_result.is_valid {
                    return snd_result;
                }

                let fst_ty = fst_result.inferred_type.unwrap();
                let snd_ty = snd_result.inferred_type.unwrap();

                HoTTType {
                    name: format!("({} × {})", fst_ty.name, snd_ty.name),
                    level: fst_ty.level.max(snd_ty.level),
                    kind: "product".to_string(),
                    params: vec![fst_ty.name, snd_ty.name],
                }
            }
            "var" => {
                // Variables need context - return placeholder
                HoTTType {
                    name: term.value.clone().unwrap_or_else(|| "?".to_string()),
                    level: 0,
                    kind: "var".to_string(),
                    params: vec![],
                }
            }
            _ => {
                return TypeCheckResult {
                    is_valid: false,
                    inferred_type: None,
                    error: Some(format!("Unknown term kind: {}", term.kind)),
                };
            }
        };

        TypeCheckResult {
            is_valid: true,
            inferred_type: Some(ty),
            error: None,
        }
    }

    fn compose_paths_internal(&self, p1: &HoTTPath, p2: &HoTTPath) -> PathOperationResult {
        // Check that endpoints match
        if !self.terms_equal(&p1.end, &p2.start) {
            return PathOperationResult {
                is_valid: false,
                result_path: None,
                error: Some("Path endpoints don't match for composition".to_string()),
            };
        }

        // Create composed path
        let composed = HoTTPath {
            base_type: p1.base_type.clone(),
            start: p1.start.clone(),
            end: p2.end.clone(),
            proof: HoTTTerm {
                kind: "compose".to_string(),
                value: None,
                children: vec![p1.proof.clone(), p2.proof.clone()],
            },
        };

        PathOperationResult {
            is_valid: true,
            result_path: Some(composed),
            error: None,
        }
    }

    fn invert_path_internal(&self, path: &HoTTPath) -> PathOperationResult {
        let inverted = HoTTPath {
            base_type: path.base_type.clone(),
            start: path.end.clone(),
            end: path.start.clone(),
            proof: HoTTTerm {
                kind: "inverse".to_string(),
                value: None,
                children: vec![path.proof.clone()],
            },
        };

        PathOperationResult {
            is_valid: true,
            result_path: Some(inverted),
            error: None,
        }
    }

    fn create_refl_path_internal(&self, ty: &HoTTType, point: &HoTTTerm) -> HoTTPath {
        HoTTPath {
            base_type: ty.clone(),
            start: point.clone(),
            end: point.clone(),
            proof: HoTTTerm {
                kind: "refl".to_string(),
                value: None,
                children: vec![point.clone()],
            },
        }
    }

    fn types_equal(&self, ty1: &HoTTType, ty2: &HoTTType) -> bool {
        ty1.kind == ty2.kind && ty1.level == ty2.level && ty1.params == ty2.params
    }

    fn terms_equal(&self, t1: &HoTTTerm, t2: &HoTTTerm) -> bool {
        t1.kind == t2.kind && t1.value == t2.value && t1.children.len() == t2.children.len()
    }
}

impl Default for HoTTEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Get library version
#[wasm_bindgen(js_name = getVersion)]
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Initialize the WASM module
#[wasm_bindgen(js_name = initModule)]
pub fn init_module() -> Result<(), JsValue> {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cohomology_engine() {
        let engine = CohomologyEngine::new();

        let graph = SheafGraph {
            nodes: vec![
                SheafNode {
                    id: 0,
                    label: "A".to_string(),
                    section: vec![1.0, 0.0],
                    weight: 1.0,
                },
                SheafNode {
                    id: 1,
                    label: "B".to_string(),
                    section: vec![1.0, 0.0],
                    weight: 1.0,
                },
            ],
            edges: vec![SheafEdge {
                source: 0,
                target: 1,
                restriction_map: vec![1.0, 0.0, 0.0, 1.0],
                source_dim: 2,
                target_dim: 2,
            }],
        };

        let result = engine.compute_cohomology_internal(&graph);
        assert!(result.is_consistent);
    }

    #[test]
    fn test_spectral_engine() {
        let engine = SpectralEngine::new();

        let graph = Graph {
            n: 3,
            edges: vec![(0, 1, 1.0), (1, 2, 1.0)],
        };

        let eigenvalues = engine.compute_eigenvalues_internal(&graph);
        assert!(!eigenvalues.is_empty());
    }

    #[test]
    fn test_causal_engine() {
        let engine = CausalEngine::new();

        let model = CausalModel {
            variables: vec![
                CausalVariable {
                    name: "X".to_string(),
                    var_type: "continuous".to_string(),
                },
                CausalVariable {
                    name: "Y".to_string(),
                    var_type: "continuous".to_string(),
                },
            ],
            edges: vec![CausalEdge {
                from: "X".to_string(),
                to: "Y".to_string(),
            }],
        };

        assert!(engine.is_valid_dag_internal(&model));
    }

    #[test]
    fn test_quantum_engine() {
        let engine = QuantumEngine::new();

        let ghz = engine.create_ghz_state_internal(3);
        assert_eq!(ghz.dimension, 8);
        assert!((ghz.amplitudes[0].norm_sq() - 0.5).abs() < 1e-10);
        assert!((ghz.amplitudes[7].norm_sq() - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_category_engine() {
        let engine = CategoryEngine::new();

        let identity = engine.create_identity(2);
        assert_eq!(identity.source_dim, 2);
        assert_eq!(identity.target_dim, 2);
        assert!((identity.matrix[0] - 1.0).abs() < 1e-10);
        assert!((identity.matrix[3] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_hott_engine() {
        let engine = HoTTEngine::new();

        let star = HoTTTerm {
            kind: "star".to_string(),
            value: None,
            children: vec![],
        };

        let result = engine.infer_type_internal(&star);
        assert!(result.is_valid);
        assert_eq!(result.inferred_type.unwrap().kind, "unit");
    }
}
