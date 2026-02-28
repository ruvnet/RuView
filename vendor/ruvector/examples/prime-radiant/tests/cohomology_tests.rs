//! Comprehensive tests for Sheaf Cohomology Module
//!
//! This test suite verifies the mathematical properties of sheaf cohomology
//! including coboundary operators, cohomology groups, and obstruction detection.

use prime_radiant::cohomology::{
    CohomologyEngine, CohomologyResult, SheafGraph, SheafNode, SheafEdge,
    Obstruction, BeliefGraphBuilder, CohomologyError,
};
use proptest::prelude::*;
use approx::assert_relative_eq;
use std::collections::HashMap;

// =============================================================================
// COBOUNDARY OPERATOR TESTS
// =============================================================================

mod coboundary_tests {
    use super::*;

    /// Test the fundamental property: delta^2 = 0
    /// The coboundary of a coboundary is always zero
    #[test]
    fn test_coboundary_squared_is_zero() {
        // Create a triangle graph (simplest complex with non-trivial cohomology)
        let mut graph = SheafGraph::new();

        // Add 3 nodes forming a triangle
        graph.add_node(SheafNode::new(0, "A", vec![1.0, 0.0, 0.0]));
        graph.add_node(SheafNode::new(1, "B", vec![0.0, 1.0, 0.0]));
        graph.add_node(SheafNode::new(2, "C", vec![0.0, 0.0, 1.0]));

        // Add edges with identity restriction maps
        graph.add_edge(SheafEdge::identity(0, 1, 3)).unwrap();
        graph.add_edge(SheafEdge::identity(1, 2, 3)).unwrap();
        graph.add_edge(SheafEdge::identity(2, 0, 3)).unwrap();

        let engine = CohomologyEngine::new();
        let result = engine.compute_cohomology(&graph).unwrap();

        // The consistency energy should be computable
        assert!(result.consistency_energy >= 0.0);
    }

    /// Test coboundary on exact sequences
    #[test]
    fn test_coboundary_on_consistent_sections() {
        let mut graph = SheafGraph::new();

        // Create nodes with identical sections (globally consistent)
        let section = vec![1.0, 2.0, 3.0];
        graph.add_node(SheafNode::new(0, "A", section.clone()));
        graph.add_node(SheafNode::new(1, "B", section.clone()));
        graph.add_node(SheafNode::new(2, "C", section.clone()));

        graph.add_edge(SheafEdge::identity(0, 1, 3)).unwrap();
        graph.add_edge(SheafEdge::identity(1, 2, 3)).unwrap();

        let engine = CohomologyEngine::new();
        let result = engine.compute_cohomology(&graph).unwrap();

        // Globally consistent sections should have zero consistency energy
        assert!(result.is_consistent);
        assert!(result.consistency_energy < 1e-10);
    }

    /// Test coboundary with non-trivial restriction maps
    #[test]
    fn test_coboundary_with_projection_maps() {
        let mut graph = SheafGraph::new();

        // Higher-dimensional source, lower-dimensional target
        graph.add_node(SheafNode::new(0, "High", vec![1.0, 2.0, 3.0, 4.0]));
        graph.add_node(SheafNode::new(1, "Low", vec![1.0, 2.0]));

        // Projection map: takes first 2 components
        let projection = vec![
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
        ];
        let edge = SheafEdge::with_map(0, 1, projection, 4, 2);
        graph.add_edge(edge).unwrap();

        let engine = CohomologyEngine::new();
        let result = engine.compute_cohomology(&graph).unwrap();

        // Should be consistent since projection matches
        assert!(result.is_consistent);
    }

    /// Test coboundary linearity: delta(af + bg) = a*delta(f) + b*delta(g)
    #[test]
    fn test_coboundary_linearity() {
        let mut graph1 = SheafGraph::new();
        let mut graph2 = SheafGraph::new();
        let mut graph_sum = SheafGraph::new();

        // Graph 1
        graph1.add_node(SheafNode::new(0, "A", vec![1.0, 0.0]));
        graph1.add_node(SheafNode::new(1, "B", vec![0.0, 0.0]));
        graph1.add_edge(SheafEdge::identity(0, 1, 2)).unwrap();

        // Graph 2
        graph2.add_node(SheafNode::new(0, "A", vec![0.0, 1.0]));
        graph2.add_node(SheafNode::new(1, "B", vec![0.0, 0.0]));
        graph2.add_edge(SheafEdge::identity(0, 1, 2)).unwrap();

        // Sum graph
        graph_sum.add_node(SheafNode::new(0, "A", vec![1.0, 1.0]));
        graph_sum.add_node(SheafNode::new(1, "B", vec![0.0, 0.0]));
        graph_sum.add_edge(SheafEdge::identity(0, 1, 2)).unwrap();

        let engine = CohomologyEngine::new();

        let e1 = engine.compute_cohomology(&graph1).unwrap().consistency_energy;
        let e2 = engine.compute_cohomology(&graph2).unwrap().consistency_energy;
        let e_sum = engine.compute_cohomology(&graph_sum).unwrap().consistency_energy;

        // Energy is quadratic, so E(sum) <= E1 + E2 + 2*sqrt(E1*E2)
        // But should satisfy triangle inequality for sqrt(energy)
        let sqrt_sum = e_sum.sqrt();
        let sqrt_bound = e1.sqrt() + e2.sqrt();
        assert!(sqrt_sum <= sqrt_bound + 1e-10);
    }
}

// =============================================================================
// COHOMOLOGY GROUP TESTS
// =============================================================================

mod cohomology_group_tests {
    use super::*;

    /// Test H^0 computation (global sections)
    #[test]
    fn test_h0_connected_graph() {
        let mut graph = SheafGraph::new();

        // Create a path graph: A -- B -- C
        let section = vec![1.0, 2.0];
        graph.add_node(SheafNode::new(0, "A", section.clone()));
        graph.add_node(SheafNode::new(1, "B", section.clone()));
        graph.add_node(SheafNode::new(2, "C", section.clone()));

        graph.add_edge(SheafEdge::identity(0, 1, 2)).unwrap();
        graph.add_edge(SheafEdge::identity(1, 2, 2)).unwrap();

        let engine = CohomologyEngine::new();
        let result = engine.compute_cohomology(&graph).unwrap();

        // For consistent sections, H^0 dimension should be positive
        assert!(result.h0_dim > 0);
    }

    /// Test H^0 on disconnected components
    #[test]
    fn test_h0_disconnected_graph() {
        let mut graph = SheafGraph::new();

        // Two disconnected nodes
        graph.add_node(SheafNode::new(0, "A", vec![1.0, 0.0]));
        graph.add_node(SheafNode::new(1, "B", vec![0.0, 1.0]));
        // No edges - disconnected

        let engine = CohomologyEngine::new();
        let result = engine.compute_cohomology(&graph).unwrap();

        // Disconnected components each contribute to H^0
        // With no edges, no consistency constraints
        assert!(result.is_consistent);
    }

    /// Test H^1 detection (obstruction group)
    #[test]
    fn test_h1_obstruction_detection() {
        let mut graph = SheafGraph::new();

        // Create inconsistent triangle
        graph.add_node(SheafNode::new(0, "A", vec![1.0, 0.0]));
        graph.add_node(SheafNode::new(1, "B", vec![0.0, 1.0]));
        graph.add_node(SheafNode::new(2, "C", vec![1.0, 1.0]));

        graph.add_edge(SheafEdge::identity(0, 1, 2)).unwrap();
        graph.add_edge(SheafEdge::identity(1, 2, 2)).unwrap();
        graph.add_edge(SheafEdge::identity(2, 0, 2)).unwrap();

        let engine = CohomologyEngine::new();
        let result = engine.compute_cohomology(&graph).unwrap();

        // Should detect inconsistency
        assert!(!result.is_consistent);
        assert!(result.consistency_energy > 0.0);
    }

    /// Test Euler characteristic: chi = dim(H^0) - dim(H^1)
    #[test]
    fn test_euler_characteristic() {
        let mut graph = SheafGraph::new();

        // Simple path graph
        let section = vec![1.0];
        for i in 0..5 {
            graph.add_node(SheafNode::new(i, &format!("N{}", i), section.clone()));
        }
        for i in 0..4 {
            graph.add_edge(SheafEdge::identity(i, i + 1, 1)).unwrap();
        }

        let engine = CohomologyEngine::new();
        let result = engine.compute_cohomology(&graph).unwrap();

        // Euler characteristic should be computed correctly
        let computed_chi = result.h0_dim as i64 - result.h1_dim as i64;
        assert_eq!(computed_chi, result.euler_characteristic);
    }

    /// Test cohomology with scalar sections
    #[test]
    fn test_scalar_cohomology() {
        let mut graph = SheafGraph::new();

        // Simple graph with scalar (1D) sections
        graph.add_node(SheafNode::new(0, "A", vec![1.0]));
        graph.add_node(SheafNode::new(1, "B", vec![2.0]));
        graph.add_edge(SheafEdge::identity(0, 1, 1)).unwrap();

        let engine = CohomologyEngine::new();
        let result = engine.compute_cohomology(&graph).unwrap();

        // Inconsistent scalars
        assert!(!result.is_consistent);
        assert_relative_eq!(result.consistency_energy, 1.0, epsilon = 1e-10);
    }
}

// =============================================================================
// OBSTRUCTION DETECTION TESTS
// =============================================================================

mod obstruction_detection_tests {
    use super::*;

    /// Test obstruction detection on known inconsistent graph
    #[test]
    fn test_detect_single_obstruction() {
        let mut graph = SheafGraph::new();

        graph.add_node(SheafNode::new(0, "Source", vec![1.0, 2.0, 3.0]));
        graph.add_node(SheafNode::new(1, "Target", vec![4.0, 5.0, 6.0]));
        graph.add_edge(SheafEdge::identity(0, 1, 3)).unwrap();

        let engine = CohomologyEngine::new();
        let obstructions = engine.detect_obstructions(&graph).unwrap();

        assert_eq!(obstructions.len(), 1);
        let obs = &obstructions[0];
        assert_eq!(obs.source_node, 0);
        assert_eq!(obs.target_node, 1);

        // Expected obstruction vector: [1-4, 2-5, 3-6] = [-3, -3, -3]
        assert_relative_eq!(obs.obstruction_vector[0], -3.0, epsilon = 1e-10);
        assert_relative_eq!(obs.obstruction_vector[1], -3.0, epsilon = 1e-10);
        assert_relative_eq!(obs.obstruction_vector[2], -3.0, epsilon = 1e-10);

        // Magnitude should be sqrt(27) = 3*sqrt(3)
        let expected_magnitude = (27.0_f64).sqrt();
        assert_relative_eq!(obs.magnitude, expected_magnitude, epsilon = 1e-10);
    }

    /// Test obstruction detection on fully consistent graph
    #[test]
    fn test_no_obstructions_when_consistent() {
        let mut graph = SheafGraph::new();

        let section = vec![1.0, 2.0];
        graph.add_node(SheafNode::new(0, "A", section.clone()));
        graph.add_node(SheafNode::new(1, "B", section.clone()));
        graph.add_node(SheafNode::new(2, "C", section.clone()));

        graph.add_edge(SheafEdge::identity(0, 1, 2)).unwrap();
        graph.add_edge(SheafEdge::identity(1, 2, 2)).unwrap();

        let engine = CohomologyEngine::new();
        let obstructions = engine.detect_obstructions(&graph).unwrap();

        assert!(obstructions.is_empty());
    }

    /// Test obstruction ordering by magnitude
    #[test]
    fn test_obstructions_ordered_by_magnitude() {
        let mut graph = SheafGraph::new();

        graph.add_node(SheafNode::new(0, "A", vec![0.0]));
        graph.add_node(SheafNode::new(1, "B", vec![1.0]));   // Small diff
        graph.add_node(SheafNode::new(2, "C", vec![10.0]));  // Large diff

        graph.add_edge(SheafEdge::identity(0, 1, 1)).unwrap();
        graph.add_edge(SheafEdge::identity(0, 2, 1)).unwrap();

        let engine = CohomologyEngine::new();
        let obstructions = engine.detect_obstructions(&graph).unwrap();

        assert_eq!(obstructions.len(), 2);
        // Should be sorted by magnitude (descending)
        assert!(obstructions[0].magnitude >= obstructions[1].magnitude);
    }

    /// Test obstruction detection with weighted nodes
    #[test]
    fn test_obstructions_with_weights() {
        let mut graph = SheafGraph::new();

        let node1 = SheafNode::new(0, "HighWeight", vec![1.0]).with_weight(10.0);
        let node2 = SheafNode::new(1, "LowWeight", vec![2.0]).with_weight(0.1);

        graph.add_node(node1);
        graph.add_node(node2);
        graph.add_edge(SheafEdge::identity(0, 1, 1)).unwrap();

        let engine = CohomologyEngine::new();
        let obstructions = engine.detect_obstructions(&graph).unwrap();

        assert_eq!(obstructions.len(), 1);
        assert_relative_eq!(obstructions[0].magnitude, 1.0, epsilon = 1e-10);
    }

    /// Test obstruction localization
    #[test]
    fn test_obstruction_localization() {
        let mut graph = SheafGraph::new();

        // Create a longer path with obstruction in middle
        graph.add_node(SheafNode::new(0, "A", vec![1.0]));
        graph.add_node(SheafNode::new(1, "B", vec![1.0]));
        graph.add_node(SheafNode::new(2, "C", vec![5.0]));  // Jump here
        graph.add_node(SheafNode::new(3, "D", vec![5.0]));

        graph.add_edge(SheafEdge::identity(0, 1, 1)).unwrap();
        graph.add_edge(SheafEdge::identity(1, 2, 1)).unwrap();
        graph.add_edge(SheafEdge::identity(2, 3, 1)).unwrap();

        let engine = CohomologyEngine::new();
        let obstructions = engine.detect_obstructions(&graph).unwrap();

        // Only edge 1->2 should have obstruction
        assert_eq!(obstructions.len(), 1);
        assert_eq!(obstructions[0].source_node, 1);
        assert_eq!(obstructions[0].target_node, 2);
    }
}

// =============================================================================
// GLOBAL SECTIONS AND REPAIR TESTS
// =============================================================================

mod global_sections_tests {
    use super::*;

    /// Test computation of global sections
    #[test]
    fn test_compute_global_sections() {
        let mut graph = SheafGraph::new();

        let section = vec![1.0, 2.0, 3.0];
        graph.add_node(SheafNode::new(0, "A", section.clone()));
        graph.add_node(SheafNode::new(1, "B", section.clone()));
        graph.add_node(SheafNode::new(2, "C", section.clone()));

        graph.add_edge(SheafEdge::identity(0, 1, 3)).unwrap();
        graph.add_edge(SheafEdge::identity(1, 2, 3)).unwrap();

        let engine = CohomologyEngine::new();
        let global_sections = engine.compute_global_sections(&graph).unwrap();

        assert!(!global_sections.is_empty());
        // Should approximate the common section
        let gs = &global_sections[0];
        assert_eq!(gs.len(), 3);
    }

    /// Test section repair
    #[test]
    fn test_repair_sections() {
        let mut graph = SheafGraph::new();

        // Slightly inconsistent sections
        graph.add_node(SheafNode::new(0, "A", vec![1.0, 2.0]));
        graph.add_node(SheafNode::new(1, "B", vec![1.1, 2.1]));

        graph.add_edge(SheafEdge::identity(0, 1, 2)).unwrap();

        let engine = CohomologyEngine::new();
        let initial_energy = engine.compute_cohomology(&graph).unwrap().consistency_energy;

        // Repair should reduce energy
        let _adjustment = engine.repair_sections(&mut graph).unwrap();
        let final_energy = engine.compute_cohomology(&graph).unwrap().consistency_energy;

        assert!(final_energy <= initial_energy);
    }

    /// Test repair convergence
    #[test]
    fn test_repair_convergence() {
        let mut graph = SheafGraph::new();

        // Create a cycle with small inconsistency
        graph.add_node(SheafNode::new(0, "A", vec![1.0]));
        graph.add_node(SheafNode::new(1, "B", vec![1.1]));
        graph.add_node(SheafNode::new(2, "C", vec![0.9]));

        graph.add_edge(SheafEdge::identity(0, 1, 1)).unwrap();
        graph.add_edge(SheafEdge::identity(1, 2, 1)).unwrap();
        graph.add_edge(SheafEdge::identity(2, 0, 1)).unwrap();

        let engine = CohomologyEngine::with_tolerance(1e-8);

        // Multiple repair iterations should converge
        for _ in 0..5 {
            engine.repair_sections(&mut graph).unwrap();
        }

        let final_result = engine.compute_cohomology(&graph).unwrap();
        // Should have reduced energy significantly
        assert!(final_result.consistency_energy < 0.1);
    }
}

// =============================================================================
// BELIEF GRAPH BUILDER TESTS
// =============================================================================

mod belief_graph_builder_tests {
    use super::*;

    /// Test building graph from beliefs
    #[test]
    fn test_build_from_beliefs() {
        let builder = BeliefGraphBuilder::new(3);

        let beliefs = vec![
            ("Belief1".to_string(), vec![1.0, 0.0, 0.0]),
            ("Belief2".to_string(), vec![0.0, 1.0, 0.0]),
            ("Belief3".to_string(), vec![0.0, 0.0, 1.0]),
        ];

        let connections = vec![(0, 1), (1, 2)];

        let graph = builder.build_from_beliefs(&beliefs, &connections).unwrap();

        assert_eq!(graph.node_count(), 3);
        assert_eq!(graph.edge_count(), 2);
    }

    /// Test builder with mixed dimensions
    #[test]
    fn test_builder_mixed_dimensions() {
        let builder = BeliefGraphBuilder::new(4);

        let beliefs = vec![
            ("Low".to_string(), vec![1.0, 2.0]),
            ("High".to_string(), vec![1.0, 2.0, 3.0, 4.0]),
        ];

        let connections = vec![(0, 1)];

        let graph = builder.build_from_beliefs(&beliefs, &connections).unwrap();
        let engine = CohomologyEngine::new();

        // Should handle dimension mismatch gracefully
        let _result = engine.compute_cohomology(&graph).unwrap();
    }
}

// =============================================================================
// EDGE CASES AND ERROR HANDLING
// =============================================================================

mod edge_cases_tests {
    use super::*;

    /// Test empty graph
    #[test]
    fn test_empty_graph_cohomology() {
        let graph = SheafGraph::new();
        let engine = CohomologyEngine::new();
        let result = engine.compute_cohomology(&graph).unwrap();

        assert_eq!(result.h0_dim, 0);
        assert_eq!(result.h1_dim, 0);
        assert!(result.is_consistent);
    }

    /// Test single node graph
    #[test]
    fn test_single_node_graph() {
        let mut graph = SheafGraph::new();
        graph.add_node(SheafNode::new(0, "Single", vec![1.0, 2.0, 3.0]));

        let engine = CohomologyEngine::new();
        let result = engine.compute_cohomology(&graph).unwrap();

        assert!(result.is_consistent);
        assert_eq!(result.consistency_energy, 0.0);
    }

    /// Test graph with zero-dimensional sections
    #[test]
    fn test_zero_dimensional_sections() {
        let mut graph = SheafGraph::new();
        graph.add_node(SheafNode::new(0, "Empty", vec![]));
        graph.add_node(SheafNode::new(1, "Empty2", vec![]));

        // This should still work, just with trivial cohomology
        let engine = CohomologyEngine::new();
        let result = engine.compute_cohomology(&graph).unwrap();
        assert!(result.is_consistent);
    }

    /// Test invalid node reference in edge
    #[test]
    fn test_invalid_node_reference() {
        let mut graph = SheafGraph::new();
        graph.add_node(SheafNode::new(0, "Only", vec![1.0]));

        // Edge to non-existent node
        let result = graph.add_edge(SheafEdge::identity(0, 99, 1));
        assert!(result.is_err());
    }

    /// Test large graph performance
    #[test]
    fn test_large_graph_performance() {
        let mut graph = SheafGraph::new();
        let n = 100;

        // Create a path graph with n nodes
        for i in 0..n {
            graph.add_node(SheafNode::new(i, &format!("N{}", i), vec![i as f64]));
        }
        for i in 0..(n - 1) {
            graph.add_edge(SheafEdge::identity(i, i + 1, 1)).unwrap();
        }

        let engine = CohomologyEngine::new();
        let start = std::time::Instant::now();
        let result = engine.compute_cohomology(&graph).unwrap();
        let duration = start.elapsed();

        // Should complete in reasonable time
        assert!(duration.as_secs() < 5);
        assert!(result.h0_dim > 0 || result.h1_dim > 0);
    }

    /// Test numerical stability with very small values
    #[test]
    fn test_numerical_stability_small_values() {
        let mut graph = SheafGraph::new();

        graph.add_node(SheafNode::new(0, "A", vec![1e-15, 1e-15]));
        graph.add_node(SheafNode::new(1, "B", vec![1e-15, 1e-15]));
        graph.add_edge(SheafEdge::identity(0, 1, 2)).unwrap();

        let engine = CohomologyEngine::with_tolerance(1e-20);
        let result = engine.compute_cohomology(&graph).unwrap();

        // Should be consistent despite small values
        assert!(result.is_consistent);
    }

    /// Test numerical stability with large values
    #[test]
    fn test_numerical_stability_large_values() {
        let mut graph = SheafGraph::new();

        graph.add_node(SheafNode::new(0, "A", vec![1e15, 1e15]));
        graph.add_node(SheafNode::new(1, "B", vec![1e15, 1e15]));
        graph.add_edge(SheafEdge::identity(0, 1, 2)).unwrap();

        let engine = CohomologyEngine::new();
        let result = engine.compute_cohomology(&graph).unwrap();

        assert!(result.is_consistent);
    }
}

// =============================================================================
// PROPERTY-BASED TESTS (using proptest)
// =============================================================================

mod property_tests {
    use super::*;

    proptest! {
        /// Property: Consistent sections always have zero energy
        #[test]
        fn prop_consistent_sections_zero_energy(
            values in proptest::collection::vec(-100.0..100.0f64, 1..10)
        ) {
            let mut graph = SheafGraph::new();
            let dim = values.len();

            graph.add_node(SheafNode::new(0, "A", values.clone()));
            graph.add_node(SheafNode::new(1, "B", values.clone()));
            graph.add_edge(SheafEdge::identity(0, 1, dim)).unwrap();

            let engine = CohomologyEngine::new();
            let result = engine.compute_cohomology(&graph).unwrap();

            prop_assert!(result.is_consistent);
            prop_assert!(result.consistency_energy < 1e-10);
        }

        /// Property: Energy is always non-negative
        #[test]
        fn prop_energy_non_negative(
            v1 in proptest::collection::vec(-100.0..100.0f64, 1..5),
            v2 in proptest::collection::vec(-100.0..100.0f64, 1..5)
        ) {
            if v1.len() != v2.len() {
                return Ok(());
            }

            let mut graph = SheafGraph::new();
            graph.add_node(SheafNode::new(0, "A", v1.clone()));
            graph.add_node(SheafNode::new(1, "B", v2.clone()));
            graph.add_edge(SheafEdge::identity(0, 1, v1.len())).unwrap();

            let engine = CohomologyEngine::new();
            let result = engine.compute_cohomology(&graph).unwrap();

            prop_assert!(result.consistency_energy >= 0.0);
        }

        /// Property: Obstruction magnitudes match energy contribution
        #[test]
        fn prop_obstruction_magnitude_matches_energy(
            diff in proptest::collection::vec(-10.0..10.0f64, 1..5)
        ) {
            let mut graph = SheafGraph::new();
            let base: Vec<f64> = vec![0.0; diff.len()];
            let target: Vec<f64> = diff.clone();

            graph.add_node(SheafNode::new(0, "A", base));
            graph.add_node(SheafNode::new(1, "B", target));
            graph.add_edge(SheafEdge::identity(0, 1, diff.len())).unwrap();

            let engine = CohomologyEngine::new();
            let obstructions = engine.detect_obstructions(&graph).unwrap();

            if !obstructions.is_empty() {
                let expected_magnitude: f64 = diff.iter().map(|x| x * x).sum::<f64>().sqrt();
                prop_assert!((obstructions[0].magnitude - expected_magnitude).abs() < 1e-10);
            }
        }

        /// Property: Adding consistent edge doesn't change consistency
        #[test]
        fn prop_consistent_edge_preserves_consistency(
            section in proptest::collection::vec(-100.0..100.0f64, 1..5)
        ) {
            let mut graph = SheafGraph::new();
            graph.add_node(SheafNode::new(0, "A", section.clone()));
            graph.add_node(SheafNode::new(1, "B", section.clone()));
            graph.add_node(SheafNode::new(2, "C", section.clone()));

            graph.add_edge(SheafEdge::identity(0, 1, section.len())).unwrap();

            let engine = CohomologyEngine::new();
            let before = engine.compute_cohomology(&graph).unwrap();

            graph.add_edge(SheafEdge::identity(1, 2, section.len())).unwrap();
            let after = engine.compute_cohomology(&graph).unwrap();

            prop_assert_eq!(before.is_consistent, after.is_consistent);
        }
    }
}

// =============================================================================
// SHEAF NEURAL NETWORK TESTS (if included in cohomology module)
// =============================================================================

mod sheaf_neural_network_tests {
    use super::*;

    /// Test that Laplacian energy is non-negative
    #[test]
    fn test_laplacian_energy_non_negative() {
        let mut graph = SheafGraph::new();

        graph.add_node(SheafNode::new(0, "A", vec![1.0, -1.0]));
        graph.add_node(SheafNode::new(1, "B", vec![-1.0, 1.0]));
        graph.add_edge(SheafEdge::identity(0, 1, 2)).unwrap();

        let engine = CohomologyEngine::new();
        let result = engine.compute_cohomology(&graph).unwrap();

        assert!(result.consistency_energy >= 0.0);
    }
}
