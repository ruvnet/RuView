//! Petgraph bridge: convert between BrainGraph and petgraph types.
//!
//! This module enables using petgraph's extensive algorithm library
//! (shortest paths, connected components, etc.) on brain connectivity graphs.

use petgraph::graph::{Graph, NodeIndex, UnGraph};
use petgraph::visit::EdgeRef;

use ruv_neural_core::brain::Atlas;
use ruv_neural_core::graph::{BrainEdge, BrainGraph, ConnectivityMetric};
use ruv_neural_core::signal::FrequencyBand;

/// Convert a BrainGraph to a petgraph undirected graph.
///
/// Node weights are the node indices (usize). Edge weights are f64 connectivity values.
/// All nodes are created even if they have no edges.
pub fn to_petgraph(graph: &BrainGraph) -> UnGraph<usize, f64> {
    let mut pg = Graph::new_undirected();
    let mut node_indices: Vec<NodeIndex> = Vec::with_capacity(graph.num_nodes);

    for i in 0..graph.num_nodes {
        node_indices.push(pg.add_node(i));
    }

    for edge in &graph.edges {
        if edge.source < graph.num_nodes && edge.target < graph.num_nodes {
            pg.add_edge(
                node_indices[edge.source],
                node_indices[edge.target],
                edge.weight,
            );
        }
    }

    pg
}

/// Convert a petgraph undirected graph back to a BrainGraph.
///
/// Node weights in the petgraph are assumed to be node indices.
/// Requires the atlas and timestamp to be provided since petgraph does not store them.
pub fn from_petgraph(
    pg: &UnGraph<usize, f64>,
    atlas: Atlas,
    timestamp: f64,
) -> BrainGraph {
    let num_nodes = pg.node_count();
    let mut edges = Vec::with_capacity(pg.edge_count());

    for edge_ref in pg.edge_references() {
        let source = pg[edge_ref.source()];
        let target = pg[edge_ref.target()];
        let weight = *edge_ref.weight();

        edges.push(BrainEdge {
            source,
            target,
            weight,
            metric: ConnectivityMetric::PhaseLockingValue,
            frequency_band: FrequencyBand::Alpha,
        });
    }

    BrainGraph {
        num_nodes,
        edges,
        timestamp,
        window_duration_s: 0.0,
        atlas,
    }
}

/// Helper: get a petgraph NodeIndex for a given brain region index.
///
/// The petgraph nodes are added in order 0..num_nodes, so the NodeIndex
/// for region `i` is simply `NodeIndex::new(i)`.
pub fn node_index(region_id: usize) -> NodeIndex {
    NodeIndex::new(region_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruv_neural_core::brain::Atlas;
    use ruv_neural_core::graph::{BrainEdge, BrainGraph, ConnectivityMetric};
    use ruv_neural_core::signal::FrequencyBand;

    fn sample_graph() -> BrainGraph {
        BrainGraph {
            num_nodes: 4,
            edges: vec![
                BrainEdge {
                    source: 0,
                    target: 1,
                    weight: 0.9,
                    metric: ConnectivityMetric::PhaseLockingValue,
                    frequency_band: FrequencyBand::Alpha,
                },
                BrainEdge {
                    source: 1,
                    target: 2,
                    weight: 0.7,
                    metric: ConnectivityMetric::PhaseLockingValue,
                    frequency_band: FrequencyBand::Alpha,
                },
                BrainEdge {
                    source: 2,
                    target: 3,
                    weight: 0.5,
                    metric: ConnectivityMetric::PhaseLockingValue,
                    frequency_band: FrequencyBand::Alpha,
                },
            ],
            timestamp: 1.0,
            window_duration_s: 0.5,
            atlas: Atlas::Custom(4),
        }
    }

    #[test]
    fn round_trip_preserves_structure() {
        let original = sample_graph();
        let pg = to_petgraph(&original);
        let restored = from_petgraph(&pg, Atlas::Custom(4), 1.0);

        assert_eq!(restored.num_nodes, original.num_nodes);
        assert_eq!(restored.edges.len(), original.edges.len());
    }

    #[test]
    fn petgraph_has_correct_node_count() {
        let graph = sample_graph();
        let pg = to_petgraph(&graph);
        assert_eq!(pg.node_count(), 4);
    }

    #[test]
    fn petgraph_has_correct_edge_count() {
        let graph = sample_graph();
        let pg = to_petgraph(&graph);
        assert_eq!(pg.edge_count(), 3);
    }

    #[test]
    fn empty_graph_round_trip() {
        let empty = BrainGraph {
            num_nodes: 10,
            edges: Vec::new(),
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(10),
        };
        let pg = to_petgraph(&empty);
        assert_eq!(pg.node_count(), 10);
        assert_eq!(pg.edge_count(), 0);

        let restored = from_petgraph(&pg, Atlas::Custom(10), 0.0);
        assert_eq!(restored.num_nodes, 10);
        assert_eq!(restored.edges.len(), 0);
    }
}
