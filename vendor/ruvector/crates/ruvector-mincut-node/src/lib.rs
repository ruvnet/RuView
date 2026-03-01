//! Node.js bindings for RuVector MinCut
//!
//! Provides native Node.js API for dynamic minimum cut operations,
//! including paper algorithms from arXiv:2512.13105.
//!
//! ## Features
//!
//! - **MinCut**: Basic dynamic minimum cut (insert/delete/query)
//! - **ThreeLevelHierarchy**: 3-level decomposition (Expander→Precluster→Cluster)
//! - **LocalKCut**: Deterministic local k-cut discovery with 4-color coding
//! - **MinCutWrapper**: Full API with connectivity curve analysis

use napi::bindgen_prelude::*;
use napi_derive::napi;
use ruvector_mincut::cluster::hierarchy::{HierarchyConfig, ThreeLevelHierarchy as RustHierarchy};
use ruvector_mincut::localkcut::deterministic::DeterministicLocalKCut;
use ruvector_mincut::{
    DynamicGraph, DynamicMinCut, MinCutBuilder, MinCutWrapper as RustMinCutWrapper,
};
use std::sync::{Arc, Mutex};

/// Edge representation for JavaScript
#[napi(object)]
pub struct JsEdge {
    pub id: u32,
    pub source: u32,
    pub target: u32,
    pub weight: f64,
}

/// Statistics about the algorithm
#[napi(object)]
pub struct JsStats {
    pub insertions: u32,
    pub deletions: u32,
    pub queries: u32,
    pub avg_update_time_us: f64,
}

/// Minimum cut result
#[napi(object)]
pub struct JsMinCutResult {
    pub value: f64,
    pub is_exact: bool,
    pub approximation_ratio: f64,
}

/// Configuration for minimum cut
#[napi(object)]
pub struct JsMinCutConfig {
    pub approximate: Option<bool>,
    pub epsilon: Option<f64>,
    pub max_exact_cut_size: Option<u32>,
}

/// Partition result
#[napi(object)]
pub struct JsPartition {
    pub s: Vec<u32>,
    pub t: Vec<u32>,
}

/// Node.js wrapper for DynamicMinCut
#[napi]
pub struct MinCut {
    inner: Arc<Mutex<DynamicMinCut>>,
}

#[napi]
impl MinCut {
    /// Create a new empty minimum cut structure
    #[napi(constructor)]
    pub fn new(config: Option<JsMinCutConfig>) -> Result<Self> {
        let mut builder = MinCutBuilder::new();

        if let Some(cfg) = config {
            if cfg.approximate.unwrap_or(false) {
                builder = builder.approximate(cfg.epsilon.unwrap_or(0.1));
            }
            if let Some(max_size) = cfg.max_exact_cut_size {
                builder = builder.max_cut_size(max_size as usize);
            }
        }

        let mincut = builder
            .build()
            .map_err(|e| Error::from_reason(format!("Failed to create MinCut: {}", e)))?;

        Ok(Self {
            inner: Arc::new(Mutex::new(mincut)),
        })
    }

    /// Create from edges array
    #[napi(factory)]
    pub fn from_edges(edges: Vec<(u32, u32, f64)>, config: Option<JsMinCutConfig>) -> Result<Self> {
        let mut builder = MinCutBuilder::new();

        if let Some(cfg) = config {
            if cfg.approximate.unwrap_or(false) {
                builder = builder.approximate(cfg.epsilon.unwrap_or(0.1));
            }
            if let Some(max_size) = cfg.max_exact_cut_size {
                builder = builder.max_cut_size(max_size as usize);
            }
        }

        // Convert edges to the expected format
        let edge_tuples: Vec<(u64, u64, f64)> = edges
            .into_iter()
            .map(|(u, v, w)| (u as u64, v as u64, w))
            .collect();

        let mincut = builder.with_edges(edge_tuples).build().map_err(|e| {
            Error::from_reason(format!("Failed to create MinCut from edges: {}", e))
        })?;

        Ok(Self {
            inner: Arc::new(Mutex::new(mincut)),
        })
    }

    /// Insert an edge (returns new min cut value)
    #[napi]
    pub fn insert_edge(&self, u: u32, v: u32, weight: f64) -> Result<f64> {
        let mut mincut = self
            .inner
            .lock()
            .map_err(|e| Error::from_reason(format!("Lock error: {}", e)))?;

        mincut
            .insert_edge(u as u64, v as u64, weight)
            .map_err(|e| Error::from_reason(format!("Failed to insert edge: {}", e)))
    }

    /// Delete an edge (returns new min cut value)
    #[napi]
    pub fn delete_edge(&self, u: u32, v: u32) -> Result<f64> {
        let mut mincut = self
            .inner
            .lock()
            .map_err(|e| Error::from_reason(format!("Lock error: {}", e)))?;

        mincut
            .delete_edge(u as u64, v as u64)
            .map_err(|e| Error::from_reason(format!("Failed to delete edge: {}", e)))
    }

    /// Get minimum cut value
    #[napi(getter)]
    pub fn min_cut_value(&self) -> f64 {
        let mincut = self.inner.lock().unwrap();
        mincut.min_cut_value()
    }

    /// Get detailed minimum cut result
    #[napi]
    pub fn min_cut(&self) -> JsMinCutResult {
        let mincut = self.inner.lock().unwrap();
        let result = mincut.min_cut();

        JsMinCutResult {
            value: result.value,
            is_exact: result.is_exact,
            approximation_ratio: result.approximation_ratio,
        }
    }

    /// Get partition: returns { s: number[], t: number[] }
    #[napi]
    pub fn partition(&self) -> Result<JsPartition> {
        let mincut = self
            .inner
            .lock()
            .map_err(|e| Error::from_reason(format!("Lock error: {}", e)))?;

        let (s, t) = mincut.partition();

        Ok(JsPartition {
            s: s.into_iter().map(|v| v as u32).collect(),
            t: t.into_iter().map(|v| v as u32).collect(),
        })
    }

    /// Get cut edges
    #[napi]
    pub fn cut_edges(&self) -> Vec<JsEdge> {
        let mincut = self.inner.lock().unwrap();
        let edges = mincut.cut_edges();

        edges
            .into_iter()
            .map(|e| JsEdge {
                id: e.id as u32,
                source: e.source as u32,
                target: e.target as u32,
                weight: e.weight,
            })
            .collect()
    }

    /// Get number of vertices
    #[napi(getter)]
    pub fn num_vertices(&self) -> u32 {
        let mincut = self.inner.lock().unwrap();
        mincut.num_vertices() as u32
    }

    /// Get number of edges
    #[napi(getter)]
    pub fn num_edges(&self) -> u32 {
        let mincut = self.inner.lock().unwrap();
        mincut.num_edges() as u32
    }

    /// Check if graph is connected
    #[napi]
    pub fn is_connected(&self) -> bool {
        let mincut = self.inner.lock().unwrap();
        mincut.is_connected()
    }

    /// Get algorithm statistics
    #[napi(getter)]
    pub fn stats(&self) -> JsStats {
        let mincut = self.inner.lock().unwrap();
        let stats = mincut.stats();

        JsStats {
            insertions: stats.insertions as u32,
            deletions: stats.deletions as u32,
            queries: stats.queries as u32,
            avg_update_time_us: stats.avg_update_time_us,
        }
    }

    /// Reset statistics
    #[napi]
    pub fn reset_stats(&self) {
        let mut mincut = self.inner.lock().unwrap();
        mincut.reset_stats();
    }
}

// ============================================================================
// ThreeLevelHierarchy - Paper Section 3: Expander → Precluster → Cluster
// ============================================================================

/// Hierarchy statistics
#[napi(object)]
pub struct JsHierarchyStats {
    pub num_expanders: u32,
    pub num_preclusters: u32,
    pub num_clusters: u32,
    pub num_vertices: u32,
    pub num_edges: u32,
    pub global_min_cut: f64,
    pub avg_expander_size: f64,
}

/// Three-level hierarchy decomposition from the paper
#[napi]
pub struct ThreeLevelHierarchy {
    inner: RustHierarchy,
}

#[napi]
impl ThreeLevelHierarchy {
    /// Create with default configuration
    #[napi(constructor)]
    pub fn new() -> Self {
        ThreeLevelHierarchy {
            inner: RustHierarchy::with_defaults(),
        }
    }

    /// Create with custom expansion parameter φ
    #[napi(factory)]
    pub fn with_phi(phi: f64) -> Self {
        ThreeLevelHierarchy {
            inner: RustHierarchy::new(HierarchyConfig {
                phi,
                ..Default::default()
            }),
        }
    }

    /// Insert an edge
    #[napi]
    pub fn insert_edge(&mut self, u: u32, v: u32, weight: f64) {
        self.inner.insert_edge(u as u64, v as u64, weight);
    }

    /// Delete an edge
    #[napi]
    pub fn delete_edge(&mut self, u: u32, v: u32) {
        self.inner.delete_edge(u as u64, v as u64);
    }

    /// Build the 3-level decomposition
    #[napi]
    pub fn build(&mut self) {
        self.inner.build();
    }

    /// Get hierarchy statistics
    #[napi(getter)]
    pub fn stats(&self) -> JsHierarchyStats {
        let s = self.inner.stats();
        JsHierarchyStats {
            num_expanders: s.num_expanders as u32,
            num_preclusters: s.num_preclusters as u32,
            num_clusters: s.num_clusters as u32,
            num_vertices: s.num_vertices as u32,
            num_edges: s.num_edges as u32,
            global_min_cut: s.global_min_cut,
            avg_expander_size: s.avg_expander_size,
        }
    }

    /// Get global minimum cut estimate
    #[napi(getter)]
    pub fn global_min_cut(&self) -> f64 {
        self.inner.global_min_cut
    }

    /// Get all vertices
    #[napi]
    pub fn vertices(&self) -> Vec<u32> {
        self.inner
            .vertices()
            .into_iter()
            .map(|v| v as u32)
            .collect()
    }
}

// ============================================================================
// LocalKCut - Paper Theorem 4.1: Color-coded DFS
// ============================================================================

/// Local cut result
#[napi(object)]
pub struct JsLocalCut {
    pub cut_value: f64,
    pub vertices: Vec<u32>,
}

/// Deterministic local k-cut algorithm
#[napi]
pub struct LocalKCut {
    inner: DeterministicLocalKCut,
    num_vertices: usize,
    num_edges: usize,
}

#[napi]
impl LocalKCut {
    /// Create new LocalKCut structure
    ///
    /// # Arguments
    /// * `lambda_max` - Maximum cut value
    /// * `volume_bound` - Maximum volume (nu parameter)
    /// * `beta` - Cut depth parameter
    #[napi(constructor)]
    pub fn new(lambda_max: i64, volume_bound: u32, beta: u32) -> Self {
        LocalKCut {
            inner: DeterministicLocalKCut::new(
                lambda_max as u64,
                volume_bound as usize,
                beta as usize,
            ),
            num_vertices: 0,
            num_edges: 0,
        }
    }

    /// Insert an edge
    #[napi]
    pub fn insert_edge(&mut self, u: u32, v: u32, weight: f64) {
        self.inner.insert_edge(u as u64, v as u64, weight);
        self.num_edges += 1;
        self.num_vertices = self.num_vertices.max((u.max(v) + 1) as usize);
    }

    /// Delete an edge
    #[napi]
    pub fn delete_edge(&mut self, u: u32, v: u32) {
        self.inner.delete_edge(u as u64, v as u64);
        self.num_edges = self.num_edges.saturating_sub(1);
    }

    /// Query local cuts from a source
    #[napi]
    pub fn query(&self, source: u32) -> Vec<JsLocalCut> {
        self.inner
            .query(source as u64)
            .into_iter()
            .map(|c| JsLocalCut {
                cut_value: c.cut_value,
                vertices: c.vertices.into_iter().map(|v| v as u32).collect(),
            })
            .collect()
    }

    /// Get number of vertices
    #[napi(getter)]
    pub fn num_vertices(&self) -> u32 {
        self.num_vertices as u32
    }

    /// Get number of edges
    #[napi(getter)]
    pub fn num_edges(&self) -> u32 {
        self.num_edges as u32
    }
}

// ============================================================================
// MinCutWrapper - Full API with Connectivity Curve Analysis
// ============================================================================

/// Connectivity curve point
#[napi(object)]
pub struct JsCurvePoint {
    pub k: u32,
    pub min_cut: i64,
}

/// Elbow detection result
#[napi(object)]
pub struct JsElbowResult {
    pub k: u32,
    pub drop: i64,
}

/// Full MinCutWrapper with paper algorithms
#[napi]
pub struct MinCutWrapperNode {
    inner: RustMinCutWrapper,
}

#[napi]
impl MinCutWrapperNode {
    /// Create new wrapper
    #[napi(constructor)]
    pub fn new() -> Self {
        let graph = Arc::new(DynamicGraph::new());
        MinCutWrapperNode {
            inner: RustMinCutWrapper::new(graph),
        }
    }

    /// Insert an edge
    #[napi]
    pub fn insert_edge(&mut self, u: u32, v: u32) {
        let time = self.inner.current_time() + 1;
        self.inner.insert_edge(time, u as u64, v as u64);
    }

    /// Delete an edge
    #[napi]
    pub fn delete_edge(&mut self, u: u32, v: u32) {
        let time = self.inner.current_time() + 1;
        self.inner.delete_edge(time, u as u64, v as u64);
    }

    /// Query minimum cut
    #[napi]
    pub fn query(&mut self) -> i64 {
        self.inner.min_cut_value() as i64
    }

    /// Get number of instances
    #[napi(getter)]
    pub fn num_instances(&self) -> u32 {
        self.inner.num_instances() as u32
    }

    /// Get current time
    #[napi(getter)]
    pub fn current_time(&self) -> i64 {
        self.inner.current_time() as i64
    }

    /// Get local cuts from source
    #[napi]
    pub fn local_cuts(&self, source: u32, lambda_max: i64) -> Vec<JsLocalCut> {
        self.inner
            .local_cuts(source as u64, lambda_max as u64)
            .into_iter()
            .map(|(value, verts)| JsLocalCut {
                cut_value: value,
                vertices: verts.into_iter().map(|v| v as u32).collect(),
            })
            .collect()
    }

    /// Compute connectivity curve
    #[napi]
    pub fn connectivity_curve(
        &self,
        ranked_edges: Vec<(u32, u32, f64)>,
        k_max: u32,
    ) -> Vec<JsCurvePoint> {
        let ranked: Vec<(u64, u64, f64)> = ranked_edges
            .into_iter()
            .map(|(u, v, s)| (u as u64, v as u64, s))
            .collect();

        self.inner
            .connectivity_curve(&ranked, k_max as usize)
            .into_iter()
            .map(|(k, min_cut)| JsCurvePoint {
                k: k as u32,
                min_cut: min_cut as i64,
            })
            .collect()
    }

    /// Find elbow in curve
    #[napi]
    pub fn find_elbow(curve: Vec<JsCurvePoint>) -> Option<JsElbowResult> {
        let curve_data: Vec<(usize, u64)> = curve
            .into_iter()
            .map(|p| (p.k as usize, p.min_cut as u64))
            .collect();

        RustMinCutWrapper::find_elbow(&curve_data).map(|(k, drop)| JsElbowResult {
            k: k as u32,
            drop: drop as i64,
        })
    }

    /// Compute detector quality
    #[napi]
    pub fn detector_quality(&self, ranked_edges: Vec<(u32, u32, f64)>, true_cut_size: u32) -> f64 {
        let ranked: Vec<(u64, u64, f64)> = ranked_edges
            .into_iter()
            .map(|(u, v, s)| (u as u64, v as u64, s))
            .collect();

        self.inner.detector_quality(&ranked, true_cut_size as usize)
    }
}
