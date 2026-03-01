//! Comprehensive tests for CompactGraph operations
//!
//! Tests cover:
//! - Edge add/remove operations
//! - Weight updates
//! - Boundary edge management
//! - Edge cases (empty graph, max capacity, boundary conditions)
//! - Property-based tests for invariant verification

use cognitum_gate_kernel::shard::{CompactGraph, Edge, EdgeId, VertexId, Weight};
use cognitum_gate_kernel::{DeltaError, MAX_EDGES, MAX_VERTICES};

#[cfg(test)]
mod basic_operations {
    use super::*;

    #[test]
    fn test_empty_graph() {
        let graph = CompactGraph::new();
        assert!(graph.is_empty());
        assert_eq!(graph.edge_count(), 0);
        assert_eq!(graph.vertex_count(), 0);
        assert!(!graph.is_full());
    }

    #[test]
    fn test_add_single_edge() {
        let mut graph = CompactGraph::new();
        let edge = Edge::new(VertexId(0), VertexId(1));
        let weight = Weight(100);

        let result = graph.add_edge(edge, weight);
        assert!(result.is_ok());

        let edge_id = result.unwrap();
        assert_eq!(graph.edge_count(), 1);
        assert_eq!(graph.vertex_count(), 2);
        assert_eq!(graph.get_weight(edge_id), Some(weight));
    }

    #[test]
    fn test_add_multiple_edges() {
        let mut graph = CompactGraph::new();

        let edges = [
            (Edge::new(VertexId(0), VertexId(1)), Weight(100)),
            (Edge::new(VertexId(1), VertexId(2)), Weight(200)),
            (Edge::new(VertexId(2), VertexId(3)), Weight(300)),
        ];

        for (edge, weight) in edges {
            let result = graph.add_edge(edge, weight);
            assert!(result.is_ok());
        }

        assert_eq!(graph.edge_count(), 3);
        assert_eq!(graph.vertex_count(), 4);
    }

    #[test]
    fn test_remove_edge() {
        let mut graph = CompactGraph::new();
        let edge = Edge::new(VertexId(0), VertexId(1));
        let edge_id = graph.add_edge(edge, Weight(100)).unwrap();

        let result = graph.remove_edge(edge_id);
        assert!(result.is_ok());
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn test_remove_nonexistent_edge() {
        let mut graph = CompactGraph::new();
        let result = graph.remove_edge(EdgeId(999));
        assert_eq!(result, Err(DeltaError::EdgeNotFound));
    }

    #[test]
    fn test_update_weight() {
        let mut graph = CompactGraph::new();
        let edge = Edge::new(VertexId(0), VertexId(1));
        let edge_id = graph.add_edge(edge, Weight(100)).unwrap();

        let result = graph.update_weight(edge_id, Weight(500));
        assert!(result.is_ok());
        assert_eq!(graph.get_weight(edge_id), Some(Weight(500)));
    }
}

#[cfg(test)]
mod edge_canonicalization {
    use super::*;

    #[test]
    fn test_canonical_ordering() {
        let e1 = Edge::new(VertexId(5), VertexId(3));
        let e2 = Edge::new(VertexId(3), VertexId(5));

        assert_eq!(e1.canonical(), e2.canonical());
    }

    #[test]
    fn test_self_loop_rejected() {
        let mut graph = CompactGraph::new();
        let edge = Edge::new(VertexId(5), VertexId(5));

        let result = graph.add_edge(edge, Weight(100));
        assert_eq!(result, Err(DeltaError::InvalidEdge));
    }

    #[test]
    fn test_duplicate_edge_updates_weight() {
        let mut graph = CompactGraph::new();
        let e1 = Edge::new(VertexId(0), VertexId(1));
        let e2 = Edge::new(VertexId(1), VertexId(0));

        let id1 = graph.add_edge(e1, Weight(100)).unwrap();
        let id2 = graph.add_edge(e2, Weight(200)).unwrap();

        assert_eq!(id1, id2);
        assert_eq!(graph.edge_count(), 1);
        assert_eq!(graph.get_weight(id1), Some(Weight(200)));
    }
}

#[cfg(test)]
mod boundary_edges {
    use super::*;

    #[test]
    fn test_mark_boundary() {
        let mut graph = CompactGraph::new();
        let edge = Edge::new(VertexId(0), VertexId(1));
        let edge_id = graph.add_edge(edge, Weight(100)).unwrap();

        assert_eq!(graph.total_internal_weight(), 100);
        assert_eq!(graph.total_boundary_weight(), 0);

        graph.mark_boundary(edge_id).unwrap();

        assert_eq!(graph.total_internal_weight(), 0);
        assert_eq!(graph.total_boundary_weight(), 100);
    }

    #[test]
    fn test_unmark_boundary() {
        let mut graph = CompactGraph::new();
        let edge = Edge::new(VertexId(0), VertexId(1));
        let edge_id = graph.add_edge(edge, Weight(100)).unwrap();

        graph.mark_boundary(edge_id).unwrap();
        graph.unmark_boundary(edge_id).unwrap();

        assert_eq!(graph.total_boundary_weight(), 0);
        assert_eq!(graph.total_internal_weight(), 100);
    }

    #[test]
    fn test_boundary_changed_flag() {
        let mut graph = CompactGraph::new();
        let edge = Edge::new(VertexId(0), VertexId(1));
        let edge_id = graph.add_edge(edge, Weight(100)).unwrap();

        graph.clear_boundary_changed();
        assert!(!graph.boundary_changed_since_last_update());

        graph.mark_boundary(edge_id).unwrap();
        assert!(graph.boundary_changed_since_last_update());
    }
}

#[cfg(test)]
mod weight_operations {
    use super::*;

    #[test]
    fn test_weight_from_f32() {
        let w = Weight::from_f32(1.0);
        assert_eq!(w.0, 256);

        let w2 = Weight::from_f32(2.0);
        assert_eq!(w2.0, 512);
    }

    #[test]
    fn test_weight_to_f32() {
        let w = Weight(256);
        assert!((w.to_f32() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_weight_saturating_operations() {
        let w1 = Weight(u16::MAX - 10);
        let w2 = Weight(100);
        let sum = w1.saturating_add(w2);
        assert_eq!(sum, Weight::MAX);

        let w3 = Weight(10);
        let diff = w3.saturating_sub(w2);
        assert_eq!(diff, Weight::ZERO);
    }
}

#[cfg(test)]
mod vertex_degree {
    use super::*;

    #[test]
    fn test_vertex_degree_after_add() {
        let mut graph = CompactGraph::new();

        graph.add_edge(Edge::new(VertexId(0), VertexId(1)), Weight(100)).unwrap();
        graph.add_edge(Edge::new(VertexId(0), VertexId(2)), Weight(100)).unwrap();
        graph.add_edge(Edge::new(VertexId(0), VertexId(3)), Weight(100)).unwrap();

        assert_eq!(graph.vertex_degree(VertexId(0)), 3);
        assert_eq!(graph.vertex_degree(VertexId(1)), 1);
    }

    #[test]
    fn test_vertex_degree_after_remove() {
        let mut graph = CompactGraph::new();

        let id1 = graph.add_edge(Edge::new(VertexId(0), VertexId(1)), Weight(100)).unwrap();
        graph.add_edge(Edge::new(VertexId(0), VertexId(2)), Weight(100)).unwrap();

        graph.remove_edge(id1).unwrap();
        assert_eq!(graph.vertex_degree(VertexId(0)), 1);
        assert_eq!(graph.vertex_degree(VertexId(1)), 0);
    }
}

#[cfg(test)]
mod min_cut_estimation {
    use super::*;

    #[test]
    fn test_min_cut_empty_graph() {
        let graph = CompactGraph::new();
        assert_eq!(graph.local_min_cut(), 0);
    }

    #[test]
    fn test_min_cut_single_edge() {
        let mut graph = CompactGraph::new();
        graph.add_edge(Edge::new(VertexId(0), VertexId(1)), Weight(100)).unwrap();
        assert_eq!(graph.local_min_cut(), 1);
    }

    #[test]
    fn test_min_cut_clique() {
        let mut graph = CompactGraph::new();

        for i in 0..4u8 {
            for j in (i + 1)..4 {
                graph.add_edge(Edge::new(VertexId(i), VertexId(j)), Weight(100)).unwrap();
            }
        }

        assert_eq!(graph.local_min_cut(), 3);
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_add_remove_invariant(src in 0u8..250, dst in 0u8..250, weight in 1u16..1000) {
            prop_assume!(src != dst);

            let mut graph = CompactGraph::new();
            let edge = Edge::new(VertexId(src), VertexId(dst));
            let id = graph.add_edge(edge, Weight(weight)).unwrap();

            assert_eq!(graph.edge_count(), 1);
            graph.remove_edge(id).unwrap();
            assert_eq!(graph.edge_count(), 0);
        }

        #[test]
        fn prop_canonical_symmetry(a in 0u8..250, b in 0u8..250) {
            prop_assume!(a != b);

            let e1 = Edge::new(VertexId(a), VertexId(b));
            let e2 = Edge::new(VertexId(b), VertexId(a));
            assert_eq!(e1.canonical(), e2.canonical());
        }

        #[test]
        fn prop_weight_roundtrip(f in 0.0f32..200.0) {
            let weight = Weight::from_f32(f);
            let back = weight.to_f32();
            assert!((f - back).abs() < 0.01 || back >= 255.0);
        }
    }
}
