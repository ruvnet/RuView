//! Visualization data structures for JavaScript rendering.
//!
//! Provides types formatted for direct consumption by D3.js and Three.js
//! visualization libraries. Includes force-directed layout positioning
//! and partition coloring.

use ruv_neural_core::graph::BrainGraph;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::graph_wasm::wasm_mincut;

/// Graph data formatted for D3.js / Three.js visualization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VizGraph {
    /// Nodes with positions and visual attributes.
    pub nodes: Vec<VizNode>,
    /// Edges with visual attributes.
    pub edges: Vec<VizEdge>,
    /// Optional partition assignments (list of node-index groups).
    pub partitions: Option<Vec<Vec<usize>>>,
    /// Optional indices into `edges` that are cut edges.
    pub cut_edges: Option<Vec<usize>>,
}

/// A single node in the visualization graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VizNode {
    /// Node index.
    pub id: usize,
    /// Human-readable label.
    pub label: String,
    /// X position (layout coordinate).
    pub x: f64,
    /// Y position (layout coordinate).
    pub y: f64,
    /// Z position (layout coordinate, for 3D views).
    pub z: f64,
    /// Module/partition membership group.
    pub group: usize,
    /// Node importance (e.g., weighted degree).
    pub size: f64,
    /// Hex color string (e.g., "#ff6600").
    pub color: String,
}

/// A single edge in the visualization graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VizEdge {
    /// Source node index.
    pub source: usize,
    /// Target node index.
    pub target: usize,
    /// Edge weight.
    pub weight: f64,
    /// Whether this edge crosses a partition boundary.
    pub is_cut: bool,
    /// Hex color string.
    pub color: String,
}

/// Default color palette for partition groups.
const GROUP_COLORS: &[&str] = &[
    "#4285f4", // Blue
    "#ea4335", // Red
    "#fbbc05", // Yellow
    "#34a853", // Green
    "#ff6d01", // Orange
    "#46bdc6", // Teal
    "#7b1fa2", // Purple
    "#c2185b", // Pink
];

/// Convert a `BrainGraph` to a `VizGraph` with force-directed layout positions.
pub fn create_viz_graph(graph: &BrainGraph) -> VizGraph {
    let n = graph.num_nodes;

    // Compute partitions via mincut (if graph is small enough).
    let mincut_result = if n > 0 && n <= 500 {
        wasm_mincut(graph).ok()
    } else {
        None
    };

    // Build partition membership map.
    let mut node_group = vec![0usize; n];
    if let Some(ref mc) = mincut_result {
        for &idx in &mc.partition_b {
            if idx < n {
                node_group[idx] = 1;
            }
        }
    }

    // Compute initial layout using a simple circular arrangement
    // (JavaScript side typically re-layouts with D3 force simulation).
    let mut nodes = Vec::with_capacity(n);
    for i in 0..n {
        let angle = 2.0 * std::f64::consts::PI * (i as f64) / (n.max(1) as f64);
        let radius = 100.0;
        let group = node_group[i];
        let degree = graph.node_degree(i);

        nodes.push(VizNode {
            id: i,
            label: format!("R{}", i),
            x: radius * angle.cos(),
            y: radius * angle.sin(),
            z: 0.0,
            group,
            size: (degree + 1.0).ln(), // Log-scaled importance
            color: GROUP_COLORS[group % GROUP_COLORS.len()].to_string(),
        });
    }

    // Build cut-edge set for coloring.
    let cut_edge_set: std::collections::HashSet<(usize, usize)> = mincut_result
        .as_ref()
        .map(|mc| {
            mc.cut_edges
                .iter()
                .flat_map(|&(s, t, _)| vec![(s, t), (t, s)])
                .collect()
        })
        .unwrap_or_default();

    let mut edges = Vec::with_capacity(graph.edges.len());
    let mut cut_edge_indices = Vec::new();

    for (idx, edge) in graph.edges.iter().enumerate() {
        let is_cut = cut_edge_set.contains(&(edge.source, edge.target));
        if is_cut {
            cut_edge_indices.push(idx);
        }
        edges.push(VizEdge {
            source: edge.source,
            target: edge.target,
            weight: edge.weight,
            is_cut,
            color: if is_cut {
                "#ff0000".to_string()
            } else {
                "#999999".to_string()
            },
        });
    }

    let partitions = mincut_result.map(|mc| vec![mc.partition_a, mc.partition_b]);

    VizGraph {
        nodes,
        edges,
        partitions,
        cut_edges: if cut_edge_indices.is_empty() {
            None
        } else {
            Some(cut_edge_indices)
        },
    }
}

/// Convert a `BrainGraph` JSON string to a `VizGraph` for rendering.
#[wasm_bindgen]
pub fn to_viz_graph(json_graph: &str) -> Result<JsValue, JsError> {
    let graph: BrainGraph =
        serde_json::from_str(json_graph).map_err(|e| JsError::new(&e.to_string()))?;
    let viz = create_viz_graph(&graph);
    serde_wasm_bindgen::to_value(&viz).map_err(|e| JsError::new(&e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruv_neural_core::brain::Atlas;
    use ruv_neural_core::graph::{BrainEdge, BrainGraph};
    use ruv_neural_core::signal::FrequencyBand;

    fn make_test_graph() -> BrainGraph {
        BrainGraph {
            num_nodes: 4,
            edges: vec![
                BrainEdge {
                    source: 0,
                    target: 1,
                    weight: 5.0,
                    metric: ruv_neural_core::graph::ConnectivityMetric::Coherence,
                    frequency_band: FrequencyBand::Alpha,
                },
                BrainEdge {
                    source: 2,
                    target: 3,
                    weight: 5.0,
                    metric: ruv_neural_core::graph::ConnectivityMetric::Coherence,
                    frequency_band: FrequencyBand::Alpha,
                },
                BrainEdge {
                    source: 1,
                    target: 2,
                    weight: 0.1,
                    metric: ruv_neural_core::graph::ConnectivityMetric::Coherence,
                    frequency_band: FrequencyBand::Alpha,
                },
            ],
            timestamp: 1000.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(4),
        }
    }

    #[test]
    fn test_viz_graph_creation() {
        let graph = make_test_graph();
        let viz = create_viz_graph(&graph);
        assert_eq!(viz.nodes.len(), 4);
        assert_eq!(viz.edges.len(), 3);
        // Should have partitions from mincut.
        assert!(viz.partitions.is_some());
    }

    #[test]
    fn test_viz_graph_serializes() {
        let graph = make_test_graph();
        let viz = create_viz_graph(&graph);
        let json = serde_json::to_string(&viz).unwrap();
        assert!(json.contains("\"nodes\""));
        assert!(json.contains("\"edges\""));
    }

    #[test]
    fn test_viz_node_has_position() {
        let graph = make_test_graph();
        let viz = create_viz_graph(&graph);
        for node in &viz.nodes {
            // Nodes should have non-zero positions (circular layout).
            assert!(node.x != 0.0 || node.y != 0.0 || node.id == 0);
        }
    }

    #[test]
    fn test_cut_edges_marked() {
        let graph = make_test_graph();
        let viz = create_viz_graph(&graph);
        let cut_count = viz.edges.iter().filter(|e| e.is_cut).count();
        // Should have at least one cut edge.
        assert!(cut_count >= 1);
    }
}
