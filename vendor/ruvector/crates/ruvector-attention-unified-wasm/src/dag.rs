//! DAG Attention Mechanisms (from ruvector-dag)
//!
//! Re-exports the 7 DAG-specific attention mechanisms:
//! - Topological Attention
//! - Causal Cone Attention
//! - Critical Path Attention
//! - MinCut-Gated Attention
//! - Hierarchical Lorentz Attention
//! - Parallel Branch Attention
//! - Temporal BTSP Attention

use ruvector_dag::{OperatorNode, QueryDag};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

// ============================================================================
// Minimal DAG for WASM
// ============================================================================

/// Minimal DAG structure for WASM attention computation
#[wasm_bindgen]
pub struct WasmQueryDag {
    inner: QueryDag,
}

#[wasm_bindgen]
impl WasmQueryDag {
    /// Create a new empty DAG
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmQueryDag {
        WasmQueryDag {
            inner: QueryDag::new(),
        }
    }

    /// Add a node with operator type and cost
    ///
    /// # Arguments
    /// * `op_type` - Operator type: "scan", "filter", "join", "aggregate", "project", "sort"
    /// * `cost` - Estimated execution cost
    ///
    /// # Returns
    /// Node ID
    #[wasm_bindgen(js_name = addNode)]
    pub fn add_node(&mut self, op_type: &str, cost: f32) -> u32 {
        let table_id = self.inner.node_count() as usize;
        let mut node = match op_type {
            "scan" => OperatorNode::seq_scan(table_id, &format!("table_{}", table_id)),
            "filter" => OperatorNode::filter(table_id, "condition"),
            "join" => OperatorNode::hash_join(table_id, "join_key"),
            "aggregate" => OperatorNode::aggregate(table_id, vec!["*".to_string()]),
            "project" => OperatorNode::project(table_id, vec!["*".to_string()]),
            "sort" => OperatorNode::sort(table_id, vec!["col".to_string()]),
            _ => OperatorNode::seq_scan(table_id, "unknown"),
        };
        node.estimated_cost = cost as f64;
        self.inner.add_node(node) as u32
    }

    /// Add an edge between nodes
    ///
    /// # Arguments
    /// * `from` - Source node ID
    /// * `to` - Target node ID
    ///
    /// # Returns
    /// True if edge was added successfully
    #[wasm_bindgen(js_name = addEdge)]
    pub fn add_edge(&mut self, from: u32, to: u32) -> bool {
        self.inner.add_edge(from as usize, to as usize).is_ok()
    }

    /// Get the number of nodes
    #[wasm_bindgen(getter, js_name = nodeCount)]
    pub fn node_count(&self) -> u32 {
        self.inner.node_count() as u32
    }

    /// Get the number of edges
    #[wasm_bindgen(getter, js_name = edgeCount)]
    pub fn edge_count(&self) -> u32 {
        self.inner.edge_count() as u32
    }

    /// Serialize to JSON
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> String {
        serde_json::to_string(&DagSummary {
            node_count: self.inner.node_count(),
            edge_count: self.inner.edge_count(),
        })
        .unwrap_or_default()
    }
}

impl WasmQueryDag {
    /// Get internal reference
    pub(crate) fn inner(&self) -> &QueryDag {
        &self.inner
    }
}

#[derive(Serialize, Deserialize)]
struct DagSummary {
    node_count: usize,
    edge_count: usize,
}

// ============================================================================
// Helper trait for converting HashMap scores to Vec
// ============================================================================

fn hashmap_to_vec(scores: &HashMap<usize, f32>, n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| scores.get(&i).copied().unwrap_or(0.0))
        .collect()
}

// ============================================================================
// Topological Attention
// ============================================================================

/// Topological attention based on DAG position
///
/// Assigns attention scores based on node position in topological order.
/// Earlier nodes (closer to sources) get higher attention.
#[wasm_bindgen]
pub struct WasmTopologicalAttention {
    decay_factor: f32,
}

#[wasm_bindgen]
impl WasmTopologicalAttention {
    /// Create a new topological attention instance
    ///
    /// # Arguments
    /// * `decay_factor` - Decay factor for position-based attention (0.0-1.0)
    #[wasm_bindgen(constructor)]
    pub fn new(decay_factor: f32) -> WasmTopologicalAttention {
        WasmTopologicalAttention { decay_factor }
    }

    /// Compute attention scores for the DAG
    ///
    /// # Returns
    /// Attention scores for each node
    pub fn forward(&self, dag: &WasmQueryDag) -> Result<Vec<f32>, JsError> {
        let n = dag.inner.node_count();
        if n == 0 {
            return Err(JsError::new("Empty DAG"));
        }

        let depths = dag.inner.compute_depths();
        let max_depth = depths.values().max().copied().unwrap_or(0);

        let mut scores = HashMap::new();
        let mut total = 0.0f32;

        for (&node_id, &depth) in &depths {
            let normalized_depth = depth as f32 / (max_depth.max(1) as f32);
            let score = self.decay_factor.powf(1.0 - normalized_depth);
            scores.insert(node_id, score);
            total += score;
        }

        if total > 0.0 {
            for score in scores.values_mut() {
                *score /= total;
            }
        }

        Ok(hashmap_to_vec(&scores, n))
    }
}

// ============================================================================
// Causal Cone Attention
// ============================================================================

/// Causal cone attention based on dependency lightcones
///
/// Nodes can only attend to ancestors in the DAG (causal predecessors).
/// Attention strength decays with causal distance.
#[wasm_bindgen]
pub struct WasmCausalConeAttention {
    future_discount: f32,
    ancestor_weight: f32,
}

#[wasm_bindgen]
impl WasmCausalConeAttention {
    /// Create a new causal cone attention instance
    ///
    /// # Arguments
    /// * `future_discount` - Discount for future nodes
    /// * `ancestor_weight` - Weight for ancestor influence
    #[wasm_bindgen(constructor)]
    pub fn new(future_discount: f32, ancestor_weight: f32) -> WasmCausalConeAttention {
        WasmCausalConeAttention {
            future_discount,
            ancestor_weight,
        }
    }

    /// Compute attention scores for the DAG
    pub fn forward(&self, dag: &WasmQueryDag) -> Result<Vec<f32>, JsError> {
        let n = dag.inner.node_count();
        if n == 0 {
            return Err(JsError::new("Empty DAG"));
        }

        let mut scores = HashMap::new();
        let mut total = 0.0f32;

        let depths = dag.inner.compute_depths();

        for node_id in 0..n {
            if dag.inner.get_node(node_id).is_none() {
                continue;
            }

            let ancestors = dag.inner.ancestors(node_id);
            let ancestor_count = ancestors.len();

            let mut score = 1.0 + (ancestor_count as f32 * self.ancestor_weight);

            if let Some(&depth) = depths.get(&node_id) {
                score *= self.future_discount.powi(depth as i32);
            }

            scores.insert(node_id, score);
            total += score;
        }

        if total > 0.0 {
            for score in scores.values_mut() {
                *score /= total;
            }
        }

        Ok(hashmap_to_vec(&scores, n))
    }
}

// ============================================================================
// Critical Path Attention
// ============================================================================

/// Critical path attention weighted by path criticality
///
/// Nodes on or near the critical path (longest execution path)
/// receive higher attention scores.
#[wasm_bindgen]
pub struct WasmCriticalPathAttention {
    path_weight: f32,
    branch_penalty: f32,
}

#[wasm_bindgen]
impl WasmCriticalPathAttention {
    /// Create a new critical path attention instance
    ///
    /// # Arguments
    /// * `path_weight` - Weight for critical path membership
    /// * `branch_penalty` - Penalty for branching nodes
    #[wasm_bindgen(constructor)]
    pub fn new(path_weight: f32, branch_penalty: f32) -> WasmCriticalPathAttention {
        WasmCriticalPathAttention {
            path_weight,
            branch_penalty,
        }
    }

    /// Compute the critical path (longest path by cost)
    fn compute_critical_path(&self, dag: &QueryDag) -> Vec<usize> {
        let mut longest_path: HashMap<usize, (f64, Vec<usize>)> = HashMap::new();

        for &leaf in &dag.leaves() {
            if let Some(node) = dag.get_node(leaf) {
                longest_path.insert(leaf, (node.estimated_cost, vec![leaf]));
            }
        }

        if let Ok(topo_order) = dag.topological_sort() {
            for &node_id in topo_order.iter().rev() {
                let node = match dag.get_node(node_id) {
                    Some(n) => n,
                    None => continue,
                };

                let mut max_cost = node.estimated_cost;
                let mut max_path = vec![node_id];

                for &child in dag.children(node_id) {
                    if let Some(&(child_cost, ref child_path)) = longest_path.get(&child) {
                        let total_cost = node.estimated_cost + child_cost;
                        if total_cost > max_cost {
                            max_cost = total_cost;
                            max_path = vec![node_id];
                            max_path.extend(child_path);
                        }
                    }
                }

                longest_path.insert(node_id, (max_cost, max_path));
            }
        }

        longest_path
            .into_iter()
            .max_by(|a, b| {
                a.1 .0
                    .partial_cmp(&b.1 .0)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(_, (_, path))| path)
            .unwrap_or_default()
    }

    /// Compute attention scores for the DAG
    pub fn forward(&self, dag: &WasmQueryDag) -> Result<Vec<f32>, JsError> {
        let n = dag.inner.node_count();
        if n == 0 {
            return Err(JsError::new("Empty DAG"));
        }

        let critical = self.compute_critical_path(&dag.inner);
        let mut scores = HashMap::new();
        let mut total = 0.0f32;

        for node_id in 0..n {
            if dag.inner.get_node(node_id).is_none() {
                continue;
            }

            let is_on_critical_path = critical.contains(&node_id);
            let num_children = dag.inner.children(node_id).len();

            let mut score = if is_on_critical_path {
                self.path_weight
            } else {
                1.0
            };

            if num_children > 1 {
                score *= 1.0 + (num_children as f32 - 1.0) * self.branch_penalty;
            }

            scores.insert(node_id, score);
            total += score;
        }

        if total > 0.0 {
            for score in scores.values_mut() {
                *score /= total;
            }
        }

        Ok(hashmap_to_vec(&scores, n))
    }
}

// ============================================================================
// MinCut-Gated Attention
// ============================================================================

/// MinCut-gated attention using flow-based bottleneck detection
///
/// Uses minimum cut analysis to identify bottleneck nodes
/// and gates attention through these critical points.
#[wasm_bindgen]
pub struct WasmMinCutGatedAttention {
    gate_threshold: f32,
}

#[wasm_bindgen]
impl WasmMinCutGatedAttention {
    /// Create a new MinCut-gated attention instance
    ///
    /// # Arguments
    /// * `gate_threshold` - Threshold for gating (0.0-1.0)
    #[wasm_bindgen(constructor)]
    pub fn new(gate_threshold: f32) -> WasmMinCutGatedAttention {
        WasmMinCutGatedAttention { gate_threshold }
    }

    /// Compute attention scores for the DAG
    pub fn forward(&self, dag: &WasmQueryDag) -> Result<Vec<f32>, JsError> {
        let n = dag.inner.node_count();
        if n == 0 {
            return Err(JsError::new("Empty DAG"));
        }

        // Simple bottleneck detection: nodes with high in-degree and out-degree
        let mut scores = HashMap::new();
        let mut total = 0.0f32;

        for node_id in 0..n {
            if dag.inner.get_node(node_id).is_none() {
                continue;
            }

            let in_degree = dag.inner.parents(node_id).len();
            let out_degree = dag.inner.children(node_id).len();

            // Bottleneck score: higher for nodes with high connectivity
            let connectivity = (in_degree + out_degree) as f32;
            let is_bottleneck = connectivity >= self.gate_threshold * n as f32;

            let score = if is_bottleneck {
                2.0 + connectivity * 0.1
            } else {
                1.0
            };

            scores.insert(node_id, score);
            total += score;
        }

        if total > 0.0 {
            for score in scores.values_mut() {
                *score /= total;
            }
        }

        Ok(hashmap_to_vec(&scores, n))
    }
}

// ============================================================================
// Hierarchical Lorentz Attention
// ============================================================================

/// Hierarchical Lorentz attention in hyperbolic space
///
/// Combines DAG hierarchy with Lorentz (hyperboloid) geometry
/// for multi-scale hierarchical attention.
#[wasm_bindgen]
pub struct WasmHierarchicalLorentzAttention {
    curvature: f32,
    temperature: f32,
}

#[wasm_bindgen]
impl WasmHierarchicalLorentzAttention {
    /// Create a new hierarchical Lorentz attention instance
    ///
    /// # Arguments
    /// * `curvature` - Hyperbolic curvature parameter
    /// * `temperature` - Temperature for softmax
    #[wasm_bindgen(constructor)]
    pub fn new(curvature: f32, temperature: f32) -> WasmHierarchicalLorentzAttention {
        WasmHierarchicalLorentzAttention {
            curvature,
            temperature,
        }
    }

    /// Compute attention scores for the DAG
    pub fn forward(&self, dag: &WasmQueryDag) -> Result<Vec<f32>, JsError> {
        let n = dag.inner.node_count();
        if n == 0 {
            return Err(JsError::new("Empty DAG"));
        }

        let depths = dag.inner.compute_depths();
        let max_depth = depths.values().max().copied().unwrap_or(0);

        // Compute hyperbolic distances from origin
        let mut distances: Vec<f32> = Vec::with_capacity(n);
        for node_id in 0..n {
            let depth = depths.get(&node_id).copied().unwrap_or(0);
            // In hyperbolic space, distance grows exponentially with depth
            let radial = (depth as f32 * 0.5).tanh();
            let distance = (1.0 + radial).acosh() * self.curvature.abs();
            distances.push(distance);
        }

        // Convert to attention scores using softmax
        let max_neg_dist = distances
            .iter()
            .map(|&d| -d / self.temperature)
            .fold(f32::NEG_INFINITY, f32::max);
        let exp_sum: f32 = distances
            .iter()
            .map(|&d| ((-d / self.temperature) - max_neg_dist).exp())
            .sum();

        let scores: Vec<f32> = distances
            .iter()
            .map(|&d| ((-d / self.temperature) - max_neg_dist).exp() / exp_sum.max(1e-10))
            .collect();

        Ok(scores)
    }
}

// ============================================================================
// Parallel Branch Attention
// ============================================================================

/// Parallel branch attention for concurrent DAG branches
///
/// Identifies parallel branches in the DAG and applies
/// attention patterns that respect branch independence.
#[wasm_bindgen]
pub struct WasmParallelBranchAttention {
    max_branches: usize,
    sync_penalty: f32,
}

#[wasm_bindgen]
impl WasmParallelBranchAttention {
    /// Create a new parallel branch attention instance
    ///
    /// # Arguments
    /// * `max_branches` - Maximum number of branches to consider
    /// * `sync_penalty` - Penalty for synchronization between branches
    #[wasm_bindgen(constructor)]
    pub fn new(max_branches: usize, sync_penalty: f32) -> WasmParallelBranchAttention {
        WasmParallelBranchAttention {
            max_branches,
            sync_penalty,
        }
    }

    /// Compute attention scores for the DAG
    pub fn forward(&self, dag: &WasmQueryDag) -> Result<Vec<f32>, JsError> {
        let n = dag.inner.node_count();
        if n == 0 {
            return Err(JsError::new("Empty DAG"));
        }

        // Detect branch points (nodes with multiple children)
        let mut branch_starts: Vec<usize> = Vec::new();
        for node_id in 0..n {
            if dag.inner.children(node_id).len() > 1 {
                branch_starts.push(node_id);
            }
        }

        let mut scores = HashMap::new();
        let mut total = 0.0f32;

        for node_id in 0..n {
            if dag.inner.get_node(node_id).is_none() {
                continue;
            }

            // Check if node is part of a parallel branch
            let parents = dag.inner.parents(node_id);
            let is_branch_child = parents.iter().any(|&p| branch_starts.contains(&p));

            let children = dag.inner.children(node_id);
            let is_sync_point = children.len() == 0 && parents.len() > 1;

            let score = if is_branch_child {
                1.5 // Boost parallel branch nodes
            } else if is_sync_point {
                1.0 * (1.0 - self.sync_penalty) // Penalize sync points
            } else {
                1.0
            };

            scores.insert(node_id, score);
            total += score;
        }

        if total > 0.0 {
            for score in scores.values_mut() {
                *score /= total;
            }
        }

        Ok(hashmap_to_vec(&scores, n))
    }
}

// ============================================================================
// Temporal BTSP Attention
// ============================================================================

/// Temporal BTSP (Behavioral Time-Series Pattern) attention
///
/// Incorporates temporal patterns and behavioral sequences
/// for time-aware DAG attention.
#[wasm_bindgen]
pub struct WasmTemporalBTSPAttention {
    eligibility_decay: f32,
    baseline_attention: f32,
}

#[wasm_bindgen]
impl WasmTemporalBTSPAttention {
    /// Create a new temporal BTSP attention instance
    ///
    /// # Arguments
    /// * `eligibility_decay` - Decay rate for eligibility traces (0.0-1.0)
    /// * `baseline_attention` - Baseline attention for nodes without history
    #[wasm_bindgen(constructor)]
    pub fn new(eligibility_decay: f32, baseline_attention: f32) -> WasmTemporalBTSPAttention {
        WasmTemporalBTSPAttention {
            eligibility_decay,
            baseline_attention,
        }
    }

    /// Compute attention scores for the DAG
    pub fn forward(&self, dag: &WasmQueryDag) -> Result<Vec<f32>, JsError> {
        let n = dag.inner.node_count();
        if n == 0 {
            return Err(JsError::new("Empty DAG"));
        }

        let mut scores = Vec::with_capacity(n);
        let mut total = 0.0f32;

        for node_id in 0..n {
            let node = match dag.inner.get_node(node_id) {
                Some(n) => n,
                None => {
                    scores.push(0.0);
                    continue;
                }
            };

            // Base score from cost and rows
            let cost_factor = (node.estimated_cost as f32 / 100.0).min(1.0);
            let rows_factor = (node.estimated_rows as f32 / 1000.0).min(1.0);
            let score = self.baseline_attention * (0.5 * cost_factor + 0.5 * rows_factor + 0.5);

            scores.push(score);
            total += score;
        }

        // Normalize
        if total > 0.0 {
            for score in scores.iter_mut() {
                *score /= total;
            }
        }

        Ok(scores)
    }
}

// ============================================================================
// DAG Attention Factory
// ============================================================================

/// Factory for creating DAG attention mechanisms
#[wasm_bindgen]
pub struct DagAttentionFactory;

#[wasm_bindgen]
impl DagAttentionFactory {
    /// Get available DAG attention types
    #[wasm_bindgen(js_name = availableTypes)]
    pub fn available_types() -> JsValue {
        let types = vec![
            "topological",
            "causal_cone",
            "critical_path",
            "mincut_gated",
            "hierarchical_lorentz",
            "parallel_branch",
            "temporal_btsp",
        ];
        serde_wasm_bindgen::to_value(&types).unwrap()
    }

    /// Get description for a DAG attention type
    #[wasm_bindgen(js_name = getDescription)]
    pub fn get_description(attention_type: &str) -> String {
        match attention_type {
            "topological" => "Position-based attention following DAG topological order".to_string(),
            "causal_cone" => "Lightcone-based attention respecting causal dependencies".to_string(),
            "critical_path" => "Attention weighted by critical execution path distance".to_string(),
            "mincut_gated" => "Flow-based gating through bottleneck nodes".to_string(),
            "hierarchical_lorentz" => {
                "Multi-scale hyperbolic attention for DAG hierarchies".to_string()
            }
            "parallel_branch" => "Branch-aware attention for parallel DAG structures".to_string(),
            "temporal_btsp" => "Time-series pattern attention for temporal DAGs".to_string(),
            _ => "Unknown attention type".to_string(),
        }
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
    fn test_dag_creation() {
        let mut dag = WasmQueryDag::new();
        let n1 = dag.add_node("scan", 1.0);
        let n2 = dag.add_node("filter", 0.5);
        dag.add_edge(n1, n2);

        assert_eq!(dag.node_count(), 2);
        assert_eq!(dag.edge_count(), 1);
    }

    #[wasm_bindgen_test]
    fn test_topological_attention() {
        let mut dag = WasmQueryDag::new();
        dag.add_node("scan", 1.0);
        dag.add_node("filter", 0.5);
        dag.add_node("project", 0.3);
        dag.add_edge(0, 1);
        dag.add_edge(1, 2);

        let attention = WasmTopologicalAttention::new(0.9);
        let scores = attention.forward(&dag);
        assert!(scores.is_ok());
        let s = scores.unwrap();
        assert_eq!(s.len(), 3);
    }

    #[wasm_bindgen_test]
    fn test_causal_cone_attention() {
        let mut dag = WasmQueryDag::new();
        dag.add_node("scan", 1.0);
        dag.add_node("filter", 0.5);
        dag.add_edge(0, 1);

        let attention = WasmCausalConeAttention::new(0.8, 0.9);
        let scores = attention.forward(&dag);
        assert!(scores.is_ok());
    }

    #[wasm_bindgen_test]
    fn test_critical_path_attention() {
        let mut dag = WasmQueryDag::new();
        dag.add_node("scan", 1.0);
        dag.add_node("filter", 0.5);
        dag.add_edge(0, 1);

        let attention = WasmCriticalPathAttention::new(2.0, 0.5);
        let scores = attention.forward(&dag);
        assert!(scores.is_ok());
    }

    #[wasm_bindgen_test]
    fn test_mincut_gated_attention() {
        let mut dag = WasmQueryDag::new();
        dag.add_node("scan", 1.0);
        dag.add_node("filter", 0.5);
        dag.add_edge(0, 1);

        let attention = WasmMinCutGatedAttention::new(0.5);
        let scores = attention.forward(&dag);
        assert!(scores.is_ok());
    }

    #[wasm_bindgen_test]
    fn test_hierarchical_lorentz_attention() {
        let mut dag = WasmQueryDag::new();
        dag.add_node("scan", 1.0);
        dag.add_node("filter", 0.5);
        dag.add_edge(0, 1);

        let attention = WasmHierarchicalLorentzAttention::new(-1.0, 0.1);
        let scores = attention.forward(&dag);
        assert!(scores.is_ok());
    }

    #[wasm_bindgen_test]
    fn test_parallel_branch_attention() {
        let mut dag = WasmQueryDag::new();
        dag.add_node("scan", 1.0);
        dag.add_node("filter", 0.5);
        dag.add_edge(0, 1);

        let attention = WasmParallelBranchAttention::new(8, 0.2);
        let scores = attention.forward(&dag);
        assert!(scores.is_ok());
    }

    #[wasm_bindgen_test]
    fn test_temporal_btsp_attention() {
        let mut dag = WasmQueryDag::new();
        dag.add_node("scan", 1.0);
        dag.add_node("filter", 0.5);
        dag.add_edge(0, 1);

        let attention = WasmTemporalBTSPAttention::new(0.95, 0.5);
        let scores = attention.forward(&dag);
        assert!(scores.is_ok());
    }

    #[wasm_bindgen_test]
    fn test_factory_types() {
        let types_js = DagAttentionFactory::available_types();
        assert!(!types_js.is_null());
    }
}
