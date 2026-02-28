//! Integration tests for paper-compliant LocalKCut implementation
//!
//! Tests the integration between the paper implementation and the
//! rest of the minimum cut system.

use ruvector_mincut::{
    DeterministicFamilyGenerator, DeterministicLocalKCut, DynamicGraph, LocalKCutOracle,
    LocalKCutQuery, PaperLocalKCutResult as LocalKCutResult,
};
use std::sync::Arc;

#[test]
fn test_paper_api_basic_usage() {
    // Create a simple graph
    let graph = Arc::new(DynamicGraph::new());
    graph.insert_edge(1, 2, 1.0).unwrap();
    graph.insert_edge(2, 3, 1.0).unwrap();
    graph.insert_edge(3, 4, 1.0).unwrap();

    // Create oracle
    let oracle = DeterministicLocalKCut::new(5);

    // Create query
    let query = LocalKCutQuery {
        seed_vertices: vec![1],
        budget_k: 2,
        radius: 3,
    };

    // Execute search
    let result = oracle.search(&graph, query);

    // Should find some cut or report none
    match result {
        LocalKCutResult::Found { cut_value, witness } => {
            assert!(cut_value <= 2);
            assert!(witness.boundary_size() <= 2);
            println!("Found cut with value: {}", cut_value);
        }
        LocalKCutResult::NoneInLocality => {
            println!("No cut found in locality");
        }
    }
}

#[test]
fn test_paper_api_with_family_generator() {
    let graph = Arc::new(DynamicGraph::new());

    // Create a more complex graph
    for i in 1..=10 {
        graph.insert_edge(i, i + 1, 1.0).unwrap();
    }

    // Add some cross edges
    graph.insert_edge(1, 5, 1.0).unwrap();
    graph.insert_edge(3, 7, 1.0).unwrap();

    // Create oracle with custom family generator
    let generator = DeterministicFamilyGenerator::new(5);
    let oracle = DeterministicLocalKCut::with_family_generator(8, generator);

    let query = LocalKCutQuery {
        seed_vertices: vec![1, 2, 3],
        budget_k: 3,
        radius: 5,
    };

    let result = oracle.search(&graph, query);

    // Verify result type
    match result {
        LocalKCutResult::Found { cut_value, witness } => {
            assert!(cut_value <= 3);
            assert_eq!(witness.boundary_size(), cut_value);
        }
        LocalKCutResult::NoneInLocality => {
            // Acceptable
        }
    }
}

#[test]
fn test_witness_integration() {
    let graph = Arc::new(DynamicGraph::new());

    // Create two components connected by a single edge
    // Component 1
    graph.insert_edge(1, 2, 1.0).unwrap();
    graph.insert_edge(2, 3, 1.0).unwrap();
    graph.insert_edge(3, 1, 1.0).unwrap();

    // Bridge
    graph.insert_edge(3, 4, 1.0).unwrap();

    // Component 2
    graph.insert_edge(4, 5, 1.0).unwrap();
    graph.insert_edge(5, 6, 1.0).unwrap();
    graph.insert_edge(6, 4, 1.0).unwrap();

    let oracle = DeterministicLocalKCut::new(10);

    let query = LocalKCutQuery {
        seed_vertices: vec![1],
        budget_k: 2,
        radius: 10,
    };

    let result = oracle.search(&graph, query);

    match result {
        LocalKCutResult::Found { cut_value, witness } => {
            // Should find the bridge (cut value = 1)
            assert_eq!(cut_value, 1);

            // Witness should be consistent
            assert_eq!(witness.boundary_size(), 1);

            // Seed should be in witness
            assert!(witness.contains(witness.seed()));

            // Can materialize partition
            let (u, _v_minus_u) = witness.materialize_partition();
            assert!(!u.is_empty());

            println!("Found bridge cut with {} vertices", witness.cardinality());
        }
        LocalKCutResult::NoneInLocality => {
            panic!("Should find the bridge");
        }
    }
}

#[test]
fn test_determinism_across_calls() {
    let graph = Arc::new(DynamicGraph::new());

    // Create deterministic graph structure
    for i in 1..=8 {
        graph.insert_edge(i, i + 1, 1.0).unwrap();
    }
    graph.insert_edge(4, 6, 1.0).unwrap();

    let oracle = DeterministicLocalKCut::new(5);

    let query = LocalKCutQuery {
        seed_vertices: vec![2, 3],
        budget_k: 3,
        radius: 4,
    };

    // Run multiple times
    let mut results = Vec::new();
    for _ in 0..5 {
        results.push(oracle.search(&graph, query.clone()));
    }

    // All results should be identical
    for i in 1..results.len() {
        match (&results[0], &results[i]) {
            (
                LocalKCutResult::Found {
                    cut_value: v1,
                    witness: w1,
                },
                LocalKCutResult::Found {
                    cut_value: v2,
                    witness: w2,
                },
            ) => {
                assert_eq!(v1, v2, "Cut values should be deterministic");
                assert_eq!(w1.seed(), w2.seed(), "Seeds should match");
                assert_eq!(
                    w1.boundary_size(),
                    w2.boundary_size(),
                    "Boundary sizes should match"
                );
                assert_eq!(
                    w1.cardinality(),
                    w2.cardinality(),
                    "Cardinalities should match"
                );
            }
            (LocalKCutResult::NoneInLocality, LocalKCutResult::NoneInLocality) => {
                // Both none - consistent
            }
            _ => {
                panic!("Results are not deterministic!");
            }
        }
    }
}

#[test]
fn test_budget_boundary() {
    let graph = Arc::new(DynamicGraph::new());

    // Create a graph with known minimum cut = 2
    graph.insert_edge(1, 2, 1.0).unwrap();
    graph.insert_edge(1, 3, 1.0).unwrap();
    graph.insert_edge(2, 4, 1.0).unwrap();
    graph.insert_edge(3, 4, 1.0).unwrap();

    let oracle = DeterministicLocalKCut::new(5);

    // Try with budget = 1 (should fail or find nothing)
    let query_low = LocalKCutQuery {
        seed_vertices: vec![1],
        budget_k: 1,
        radius: 5,
    };

    let result_low = oracle.search(&graph, query_low);
    if let LocalKCutResult::Found { cut_value, .. } = result_low {
        assert!(cut_value <= 1, "Must respect budget constraint");
    }

    // Try with budget = 3 (should succeed)
    let query_high = LocalKCutQuery {
        seed_vertices: vec![1],
        budget_k: 3,
        radius: 5,
    };

    let result_high = oracle.search(&graph, query_high);
    match result_high {
        LocalKCutResult::Found { cut_value, .. } => {
            assert!(cut_value <= 3, "Must respect budget constraint");
        }
        LocalKCutResult::NoneInLocality => {
            // Acceptable based on radius
        }
    }
}

#[test]
fn test_radius_limiting() {
    let graph = Arc::new(DynamicGraph::new());

    // Create a long path
    for i in 1..=20 {
        graph.insert_edge(i, i + 1, 1.0).unwrap();
    }

    let oracle = DeterministicLocalKCut::new(3); // max_radius = 3

    // Request large radius (should be capped at max_radius)
    let query = LocalKCutQuery {
        seed_vertices: vec![1],
        budget_k: 2,
        radius: 100,
    };

    // Should not panic, should cap at max_radius = 3
    let result = oracle.search(&graph, query);

    // Result should be based on radius=3, not radius=100
    match result {
        LocalKCutResult::Found { witness, .. } => {
            // With radius=3, should find at most 4 vertices (seed + 3 layers)
            assert!(witness.cardinality() <= 4);
        }
        LocalKCutResult::NoneInLocality => {
            // Acceptable
        }
    }
}

#[test]
fn test_empty_graph() {
    let graph = Arc::new(DynamicGraph::new());
    let oracle = DeterministicLocalKCut::new(5);

    let query = LocalKCutQuery {
        seed_vertices: vec![1],
        budget_k: 10,
        radius: 5,
    };

    let result = oracle.search(&graph, query);

    // Should return NoneInLocality for empty graph
    assert!(matches!(result, LocalKCutResult::NoneInLocality));
}

#[test]
fn test_single_vertex() {
    let graph = Arc::new(DynamicGraph::new());

    // Add a single vertex (via an edge to itself would be invalid, so just query)
    graph.insert_edge(1, 2, 1.0).unwrap();

    let oracle = DeterministicLocalKCut::new(5);

    let query = LocalKCutQuery {
        seed_vertices: vec![1],
        budget_k: 10,
        radius: 0, // Don't expand
    };

    let result = oracle.search(&graph, query);

    // With radius=0, should find single vertex or none
    match result {
        LocalKCutResult::Found { witness, .. } => {
            assert_eq!(witness.cardinality(), 1);
        }
        LocalKCutResult::NoneInLocality => {
            // Also acceptable
        }
    }
}
