//! WASM bindings for RuVector MinCut
//!
//! Provides JavaScript/TypeScript API for dynamic minimum cut operations,
//! including paper algorithms from arXiv:2512.13105.
//!
//! ## Features
//!
//! - **WasmMinCut**: Basic dynamic minimum cut (insert/delete/query)
//! - **WasmThreeLevelHierarchy**: 3-level decomposition (Expander→Precluster→Cluster)
//! - **WasmLocalKCut**: Deterministic local k-cut discovery with 4-color coding
//! - **WasmMinCutWrapper**: Full API with connectivity curve analysis
//!
//! ## Example Usage
//!
//! ```javascript
//! import init, { WasmMinCut, WasmThreeLevelHierarchy, WasmLocalKCut } from './ruvector_mincut_wasm';
//!
//! await init();
//!
//! // Basic min-cut
//! const mincut = WasmMinCut.fromEdges([[0, 1, 1.0], [1, 2, 1.0], [0, 2, 1.0]]);
//! console.log(mincut.minCutValue());
//!
//! // 3-level hierarchy decomposition
//! const hierarchy = new WasmThreeLevelHierarchy();
//! hierarchy.insertEdge(0, 1, 1.0);
//! hierarchy.insertEdge(1, 2, 1.0);
//! hierarchy.build();
//! console.log(hierarchy.stats());
//!
//! // Local k-cut discovery
//! const lkcut = new WasmLocalKCut(5, 100, 2);
//! lkcut.insertEdge(0, 1, 1.0);
//! const cuts = lkcut.query(0);
//! ```

use ruvector_mincut::cluster::hierarchy::{HierarchyConfig, ThreeLevelHierarchy};
use ruvector_mincut::localkcut::deterministic::DeterministicLocalKCut;
use ruvector_mincut::{DynamicGraph, DynamicMinCut, MinCutBuilder, MinCutConfig, MinCutWrapper};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use wasm_bindgen::prelude::*;

/// WASM wrapper for DynamicMinCut
#[wasm_bindgen]
pub struct WasmMinCut {
    inner: DynamicMinCut,
}

#[derive(Serialize, Deserialize)]
struct EdgeInput {
    u: u64,
    v: u64,
    weight: f64,
}

#[derive(Serialize, Deserialize)]
struct Partition {
    s: Vec<u64>,
    t: Vec<u64>,
}

#[derive(Serialize, Deserialize)]
struct Edge {
    u: u64,
    v: u64,
    weight: f64,
}

#[derive(Serialize, Deserialize)]
struct Stats {
    num_vertices: usize,
    num_edges: usize,
    min_cut_value: f64,
    is_connected: bool,
    num_operations: usize,
}

#[wasm_bindgen]
impl WasmMinCut {
    /// Create a new empty minimum cut structure
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<WasmMinCut, JsError> {
        console_error_panic_hook::set_once();

        Ok(WasmMinCut {
            inner: DynamicMinCut::new(MinCutConfig::default()),
        })
    }

    /// Create from edges array: [[u, v, weight], ...]
    ///
    /// # Arguments
    /// * `edges` - JavaScript array of [u, v, weight] tuples
    ///
    /// # Example
    /// ```javascript
    /// const edges = [[0, 1, 1.5], [1, 2, 2.0]];
    /// const mincut = WasmMinCut.fromEdges(edges);
    /// ```
    #[wasm_bindgen(js_name = "fromEdges")]
    pub fn from_edges(edges: JsValue) -> Result<WasmMinCut, JsError> {
        console_error_panic_hook::set_once();

        // Deserialize edges from JavaScript array
        let edges_vec: Vec<Vec<f64>> = serde_wasm_bindgen::from_value(edges)
            .map_err(|e| JsError::new(&format!("Failed to parse edges: {}", e)))?;

        // Convert to tuple format expected by with_edges
        let mut edge_tuples = Vec::with_capacity(edges_vec.len());

        for edge in edges_vec {
            if edge.len() != 3 {
                return Err(JsError::new("Each edge must be [u, v, weight]"));
            }

            let u = edge[0] as u64;
            let v = edge[1] as u64;
            let weight = edge[2];

            edge_tuples.push((u, v, weight));
        }

        let inner = MinCutBuilder::new()
            .with_edges(edge_tuples)
            .build()
            .map_err(|e| JsError::new(&format!("Failed to build mincut: {}", e)))?;

        Ok(WasmMinCut { inner })
    }

    /// Insert an edge into the graph
    ///
    /// # Arguments
    /// * `u` - Source vertex
    /// * `v` - Target vertex
    /// * `weight` - Edge weight
    ///
    /// # Returns
    /// The new minimum cut value after insertion
    #[wasm_bindgen(js_name = "insertEdge")]
    pub fn insert_edge(&mut self, u: u64, v: u64, weight: f64) -> Result<f64, JsError> {
        self.inner
            .insert_edge(u, v, weight)
            .map_err(|e| JsError::new(&format!("Failed to insert edge: {}", e)))
    }

    /// Delete an edge from the graph
    ///
    /// # Arguments
    /// * `u` - Source vertex
    /// * `v` - Target vertex
    ///
    /// # Returns
    /// The new minimum cut value after deletion
    #[wasm_bindgen(js_name = "deleteEdge")]
    pub fn delete_edge(&mut self, u: u64, v: u64) -> Result<f64, JsError> {
        self.inner
            .delete_edge(u, v)
            .map_err(|e| JsError::new(&format!("Failed to delete edge: {}", e)))
    }

    /// Get the current minimum cut value
    ///
    /// # Returns
    /// The sum of edge weights in the minimum cut
    #[wasm_bindgen(js_name = "minCutValue")]
    pub fn min_cut_value(&self) -> f64 {
        self.inner.min_cut_value()
    }

    /// Get the partition as JSON: { "s": [...], "t": [...] }
    ///
    /// # Returns
    /// JavaScript object with two arrays: `s` and `t` containing vertex IDs
    ///
    /// # Example
    /// ```javascript
    /// const { s, t } = mincut.partition();
    /// console.log("S partition:", s);
    /// console.log("T partition:", t);
    /// ```
    #[wasm_bindgen]
    pub fn partition(&self) -> JsValue {
        let (s_set, t_set) = self.inner.partition();

        let partition = Partition {
            s: s_set.into_iter().collect(),
            t: t_set.into_iter().collect(),
        };

        serde_wasm_bindgen::to_value(&partition).unwrap_or(JsValue::NULL)
    }

    /// Get the cut edges as JSON array
    ///
    /// # Returns
    /// JavaScript array of edge objects: [{ u, v, weight }, ...]
    ///
    /// # Example
    /// ```javascript
    /// const edges = mincut.cutEdges();
    /// edges.forEach(e => console.log(`Edge ${e.u}-${e.v}: ${e.weight}`));
    /// ```
    #[wasm_bindgen(js_name = "cutEdges")]
    pub fn cut_edges(&self) -> JsValue {
        let edges = self.inner.cut_edges();

        let edge_list: Vec<Edge> = edges
            .into_iter()
            .map(|e| Edge {
                u: e.source,
                v: e.target,
                weight: e.weight,
            })
            .collect();

        serde_wasm_bindgen::to_value(&edge_list).unwrap_or(JsValue::NULL)
    }

    /// Get the number of vertices in the graph
    #[wasm_bindgen(js_name = "numVertices")]
    pub fn num_vertices(&self) -> usize {
        self.inner.num_vertices()
    }

    /// Get the number of edges in the graph
    #[wasm_bindgen(js_name = "numEdges")]
    pub fn num_edges(&self) -> usize {
        self.inner.num_edges()
    }

    /// Check if the graph is connected
    ///
    /// # Returns
    /// `true` if there is a path between all vertex pairs
    #[wasm_bindgen(js_name = "isConnected")]
    pub fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }

    /// Get comprehensive statistics as JSON
    ///
    /// # Returns
    /// JavaScript object with:
    /// - `num_vertices`: Number of vertices
    /// - `num_edges`: Number of edges
    /// - `min_cut_value`: Current minimum cut value
    /// - `is_connected`: Whether graph is connected
    /// - `num_operations`: Total operations performed
    ///
    /// # Example
    /// ```javascript
    /// const stats = mincut.stats();
    /// console.log(`Graph has ${stats.num_vertices} vertices and ${stats.num_edges} edges`);
    /// console.log(`Minimum cut value: ${stats.min_cut_value}`);
    /// ```
    #[wasm_bindgen]
    pub fn stats(&self) -> JsValue {
        let algo_stats = self.inner.stats();
        let stats = Stats {
            num_vertices: self.inner.num_vertices(),
            num_edges: self.inner.num_edges(),
            min_cut_value: self.inner.min_cut_value(),
            is_connected: self.inner.is_connected(),
            num_operations: (algo_stats.insertions + algo_stats.deletions + algo_stats.queries)
                as usize,
        };

        serde_wasm_bindgen::to_value(&stats).unwrap_or(JsValue::NULL)
    }

    /// Update an edge weight (delete old, insert new)
    ///
    /// # Arguments
    /// * `u` - Source vertex
    /// * `v` - Target vertex
    /// * `new_weight` - New edge weight
    ///
    /// # Returns
    /// The new minimum cut value after update
    #[wasm_bindgen(js_name = "updateEdge")]
    pub fn update_edge(&mut self, u: u64, v: u64, new_weight: f64) -> Result<f64, JsError> {
        // Delete old edge (ignore error if doesn't exist)
        let _ = self.inner.delete_edge(u, v);

        // Insert with new weight
        self.inner
            .insert_edge(u, v, new_weight)
            .map_err(|e| JsError::new(&format!("Failed to update edge: {}", e)))
    }

    /// Batch insert multiple edges
    ///
    /// # Arguments
    /// * `edges` - JavaScript array of [u, v, weight] tuples
    ///
    /// # Returns
    /// The final minimum cut value
    ///
    /// # Example
    /// ```javascript
    /// const edges = [[0, 1, 1.0], [1, 2, 2.0], [2, 3, 1.5]];
    /// const cutValue = mincut.batchInsert(edges);
    /// ```
    #[wasm_bindgen(js_name = "batchInsert")]
    pub fn batch_insert(&mut self, edges: JsValue) -> Result<f64, JsError> {
        let edges_vec: Vec<Vec<f64>> = serde_wasm_bindgen::from_value(edges)
            .map_err(|e| JsError::new(&format!("Failed to parse edges: {}", e)))?;

        for edge in edges_vec {
            if edge.len() != 3 {
                return Err(JsError::new("Each edge must be [u, v, weight]"));
            }

            let u = edge[0] as u64;
            let v = edge[1] as u64;
            let weight = edge[2];

            self.inner.insert_edge(u, v, weight).map_err(|e| {
                JsError::new(&format!("Failed to insert edge [{}, {}]: {}", u, v, e))
            })?;
        }

        Ok(self.inner.min_cut_value())
    }

    /// Batch delete multiple edges
    ///
    /// # Arguments
    /// * `edges` - JavaScript array of [u, v] tuples
    ///
    /// # Returns
    /// The final minimum cut value
    ///
    /// # Example
    /// ```javascript
    /// const edges = [[0, 1], [1, 2]];
    /// const cutValue = mincut.batchDelete(edges);
    /// ```
    #[wasm_bindgen(js_name = "batchDelete")]
    pub fn batch_delete(&mut self, edges: JsValue) -> Result<f64, JsError> {
        let edges_vec: Vec<Vec<f64>> = serde_wasm_bindgen::from_value(edges)
            .map_err(|e| JsError::new(&format!("Failed to parse edges: {}", e)))?;

        for edge in edges_vec {
            if edge.len() < 2 {
                return Err(JsError::new("Each edge must be [u, v] or [u, v, weight]"));
            }

            let u = edge[0] as u64;
            let v = edge[1] as u64;

            self.inner.delete_edge(u, v).map_err(|e| {
                JsError::new(&format!("Failed to delete edge [{}, {}]: {}", u, v, e))
            })?;
        }

        Ok(self.inner.min_cut_value())
    }

    /// Clear all edges from the graph
    #[wasm_bindgen]
    pub fn clear(&mut self) {
        self.inner = DynamicMinCut::new(MinCutConfig::default());
    }
}

/// Initialize the WASM module (call once at startup)
///
/// This sets up panic hooks for better error messages in the browser console.
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

/// Get version information
#[wasm_bindgen(js_name = "getVersion")]
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

// ============================================================================
// ThreeLevelHierarchy - Paper Section 3: Expander → Precluster → Cluster
// ============================================================================

/// Statistics for the hierarchy
#[derive(Serialize, Deserialize)]
struct HierarchyStatsJs {
    num_expanders: usize,
    num_preclusters: usize,
    num_clusters: usize,
    num_vertices: usize,
    num_edges: usize,
    global_min_cut: f64,
    avg_expander_size: f64,
}

/// WASM wrapper for ThreeLevelHierarchy
///
/// Implements the 3-level decomposition from arXiv:2512.13105:
/// - Level 0: Expanders (φ-expander subgraphs)
/// - Level 1: Preclusters (groups of expanders)
/// - Level 2: Clusters (top-level grouping with mirror cuts)
#[wasm_bindgen]
pub struct WasmThreeLevelHierarchy {
    inner: ThreeLevelHierarchy,
}

#[wasm_bindgen]
impl WasmThreeLevelHierarchy {
    /// Create a new hierarchy with default configuration
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmThreeLevelHierarchy {
        console_error_panic_hook::set_once();
        WasmThreeLevelHierarchy {
            inner: ThreeLevelHierarchy::with_defaults(),
        }
    }

    /// Create hierarchy with custom expansion parameter φ
    #[wasm_bindgen(js_name = "withPhi")]
    pub fn with_phi(phi: f64) -> WasmThreeLevelHierarchy {
        console_error_panic_hook::set_once();
        WasmThreeLevelHierarchy {
            inner: ThreeLevelHierarchy::new(HierarchyConfig {
                phi,
                ..Default::default()
            }),
        }
    }

    /// Insert an edge into the graph
    #[wasm_bindgen(js_name = "insertEdge")]
    pub fn insert_edge(&mut self, u: u64, v: u64, weight: f64) {
        self.inner.insert_edge(u, v, weight);
    }

    /// Delete an edge from the graph
    #[wasm_bindgen(js_name = "deleteEdge")]
    pub fn delete_edge(&mut self, u: u64, v: u64) {
        self.inner.delete_edge(u, v);
    }

    /// Build the complete 3-level hierarchy
    ///
    /// Must be called after inserting edges to compute the decomposition.
    #[wasm_bindgen]
    pub fn build(&mut self) {
        self.inner.build();
    }

    /// Get hierarchy statistics as JSON
    #[wasm_bindgen]
    pub fn stats(&self) -> JsValue {
        let s = self.inner.stats();
        let stats = HierarchyStatsJs {
            num_expanders: s.num_expanders,
            num_preclusters: s.num_preclusters,
            num_clusters: s.num_clusters,
            num_vertices: s.num_vertices,
            num_edges: s.num_edges,
            global_min_cut: s.global_min_cut,
            avg_expander_size: s.avg_expander_size,
        };
        serde_wasm_bindgen::to_value(&stats).unwrap_or(JsValue::NULL)
    }

    /// Get the global minimum cut estimate
    #[wasm_bindgen(js_name = "globalMinCut")]
    pub fn global_min_cut(&self) -> f64 {
        self.inner.global_min_cut
    }

    /// Get all vertices as JSON array
    #[wasm_bindgen]
    pub fn vertices(&self) -> JsValue {
        let verts: Vec<u64> = self.inner.vertices();
        serde_wasm_bindgen::to_value(&verts).unwrap_or(JsValue::NULL)
    }
}

// ============================================================================
// DeterministicLocalKCut - Paper Theorem 4.1: Color-coded DFS
// ============================================================================

/// Local cut result for JS
#[derive(Serialize, Deserialize)]
struct LocalCutJs {
    cut_value: f64,
    vertices: Vec<u64>,
}

/// WASM wrapper for DeterministicLocalKCut
///
/// Implements the deterministic local k-cut algorithm from arXiv:2512.13105:
/// - Uses 4-color coding (red-blue, green-yellow)
/// - Greedy forest packing for edge classification
/// - Color-coded DFS for cut enumeration
#[wasm_bindgen]
pub struct WasmLocalKCut {
    inner: DeterministicLocalKCut,
    num_vertices: usize,
    num_edges: usize,
}

#[wasm_bindgen]
impl WasmLocalKCut {
    /// Create a new LocalKCut structure
    ///
    /// # Arguments
    /// * `lambda_max` - Maximum cut value to consider
    /// * `volume_bound` - Maximum volume to explore (nu parameter)
    /// * `beta` - Cut depth parameter (typically 2)
    #[wasm_bindgen(constructor)]
    pub fn new(lambda_max: u64, volume_bound: usize, beta: usize) -> WasmLocalKCut {
        console_error_panic_hook::set_once();
        WasmLocalKCut {
            inner: DeterministicLocalKCut::new(lambda_max, volume_bound, beta),
            num_vertices: 0,
            num_edges: 0,
        }
    }

    /// Insert an edge
    #[wasm_bindgen(js_name = "insertEdge")]
    pub fn insert_edge(&mut self, u: u64, v: u64, weight: f64) {
        self.inner.insert_edge(u, v, weight);
        self.num_edges += 1;
        // Rough vertex count estimate (may overcount)
        self.num_vertices = self.num_vertices.max((u.max(v) + 1) as usize);
    }

    /// Delete an edge
    #[wasm_bindgen(js_name = "deleteEdge")]
    pub fn delete_edge(&mut self, u: u64, v: u64) {
        self.inner.delete_edge(u, v);
        self.num_edges = self.num_edges.saturating_sub(1);
    }

    /// Query local cuts from a source vertex
    ///
    /// Returns array of { cut_value, vertices } objects
    #[wasm_bindgen]
    pub fn query(&self, source: u64) -> JsValue {
        let results = self.inner.query(source);
        let cuts: Vec<LocalCutJs> = results
            .into_iter()
            .map(|c| LocalCutJs {
                cut_value: c.cut_value,
                vertices: c.vertices.into_iter().collect(),
            })
            .collect();
        serde_wasm_bindgen::to_value(&cuts).unwrap_or(JsValue::NULL)
    }

    /// Get number of vertices (approximate)
    #[wasm_bindgen(js_name = "numVertices")]
    pub fn num_vertices(&self) -> usize {
        self.num_vertices
    }

    /// Get number of edges
    #[wasm_bindgen(js_name = "numEdges")]
    pub fn num_edges(&self) -> usize {
        self.num_edges
    }
}

// ============================================================================
// MinCutWrapper - Full API with Connectivity Curve Analysis
// ============================================================================

/// Connectivity curve point
#[derive(Serialize, Deserialize)]
struct CurvePoint {
    k: usize,
    min_cut: u64,
}

/// Elbow detection result
#[derive(Serialize, Deserialize)]
struct ElbowResult {
    k: usize,
    drop: u64,
}

/// WASM wrapper for MinCutWrapper
///
/// High-level API combining all paper algorithms:
/// - O(log n) instance management
/// - ThreeLevelHierarchy decomposition
/// - LocalKCut discovery
/// - Connectivity curve analysis for boundary validation
#[wasm_bindgen]
pub struct WasmMinCutWrapper {
    inner: MinCutWrapper,
}

#[wasm_bindgen]
impl WasmMinCutWrapper {
    /// Create a new MinCutWrapper
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmMinCutWrapper {
        console_error_panic_hook::set_once();
        let graph = Arc::new(DynamicGraph::new());
        WasmMinCutWrapper {
            inner: MinCutWrapper::new(graph),
        }
    }

    /// Insert an edge (timestamp auto-incremented)
    #[wasm_bindgen(js_name = "insertEdge")]
    pub fn insert_edge(&mut self, u: u64, v: u64) {
        let time = self.inner.current_time() + 1;
        self.inner.insert_edge(time, u, v);
    }

    /// Delete an edge (timestamp auto-incremented)
    #[wasm_bindgen(js_name = "deleteEdge")]
    pub fn delete_edge(&mut self, u: u64, v: u64) {
        let time = self.inner.current_time() + 1;
        self.inner.delete_edge(time, u, v);
    }

    /// Query the minimum cut value
    #[wasm_bindgen]
    pub fn query(&mut self) -> f64 {
        self.inner.min_cut_value() as f64
    }

    /// Get number of active instances
    #[wasm_bindgen(js_name = "numInstances")]
    pub fn num_instances(&self) -> usize {
        self.inner.num_instances()
    }

    /// Get current logical time
    #[wasm_bindgen(js_name = "currentTime")]
    pub fn current_time(&self) -> u64 {
        self.inner.current_time()
    }

    /// Query with LocalKCut certification
    ///
    /// Returns { cut_value, certified } object
    #[wasm_bindgen(js_name = "queryWithCertification")]
    pub fn query_with_certification(&mut self, source: u64) -> JsValue {
        let (cut_value, certified) = self.inner.query_with_local_kcut(source);
        let result = serde_json::json!({
            "cut_value": cut_value,
            "certified": certified,
        });
        serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
    }

    /// Get local cuts from a source vertex
    #[wasm_bindgen(js_name = "localCuts")]
    pub fn local_cuts(&self, source: u64, lambda_max: u64) -> JsValue {
        let cuts = self.inner.local_cuts(source, lambda_max);
        let result: Vec<LocalCutJs> = cuts
            .into_iter()
            .map(|(value, verts)| LocalCutJs {
                cut_value: value,
                vertices: verts,
            })
            .collect();
        serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
    }

    /// Compute edge-connectivity degradation curve
    ///
    /// # Arguments
    /// * `ranked_edges` - Array of [u, v, score] ranked by cut-likelihood
    /// * `k_max` - Maximum edges to remove
    ///
    /// # Returns
    /// Array of { k, min_cut } showing degradation
    #[wasm_bindgen(js_name = "connectivityCurve")]
    pub fn connectivity_curve(&self, ranked_edges: JsValue, k_max: usize) -> JsValue {
        let edges: Vec<Vec<f64>> = serde_wasm_bindgen::from_value(ranked_edges).unwrap_or_default();

        let ranked: Vec<(u64, u64, f64)> = edges
            .into_iter()
            .filter_map(|e| {
                if e.len() >= 3 {
                    Some((e[0] as u64, e[1] as u64, e[2]))
                } else {
                    None
                }
            })
            .collect();

        let curve = self.inner.connectivity_curve(&ranked, k_max);
        let result: Vec<CurvePoint> = curve
            .into_iter()
            .map(|(k, min_cut)| CurvePoint { k, min_cut })
            .collect();

        serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
    }

    /// Find elbow point in connectivity curve
    ///
    /// Returns { k, drop } or null if no elbow found
    #[wasm_bindgen(js_name = "findElbow")]
    pub fn find_elbow(curve: JsValue) -> JsValue {
        let points: Vec<CurvePoint> = serde_wasm_bindgen::from_value(curve).unwrap_or_default();

        let curve_data: Vec<(usize, u64)> = points.into_iter().map(|p| (p.k, p.min_cut)).collect();

        match MinCutWrapper::find_elbow(&curve_data) {
            Some((k, drop)) => {
                let result = ElbowResult { k, drop };
                serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
            }
            None => JsValue::NULL,
        }
    }

    /// Compute detector quality score
    ///
    /// # Arguments
    /// * `ranked_edges` - Array of [u, v, score]
    /// * `true_cut_size` - Known size of true minimum cut
    ///
    /// # Returns
    /// Quality score from 0.0 (poor) to 1.0 (perfect)
    #[wasm_bindgen(js_name = "detectorQuality")]
    pub fn detector_quality(&self, ranked_edges: JsValue, true_cut_size: usize) -> f64 {
        let edges: Vec<Vec<f64>> = serde_wasm_bindgen::from_value(ranked_edges).unwrap_or_default();

        let ranked: Vec<(u64, u64, f64)> = edges
            .into_iter()
            .filter_map(|e| {
                if e.len() >= 3 {
                    Some((e[0] as u64, e[1] as u64, e[2]))
                } else {
                    None
                }
            })
            .collect();

        self.inner.detector_quality(&ranked, true_cut_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn test_new() {
        let mincut = WasmMinCut::new().unwrap();
        assert_eq!(mincut.num_vertices(), 0);
        assert_eq!(mincut.num_edges(), 0);
    }

    #[wasm_bindgen_test]
    fn test_insert_edge() {
        let mut mincut = WasmMinCut::new().unwrap();
        let result = mincut.insert_edge(0, 1, 1.0);
        assert!(result.is_ok());
        assert_eq!(mincut.num_edges(), 1);
    }

    #[wasm_bindgen_test]
    fn test_min_cut_value() {
        let mut mincut = WasmMinCut::new().unwrap();
        mincut.insert_edge(0, 1, 1.0).unwrap();
        mincut.insert_edge(1, 2, 2.0).unwrap();
        mincut.insert_edge(0, 2, 1.5).unwrap();

        let cut_value = mincut.min_cut_value();
        assert!(cut_value > 0.0);
    }
}
