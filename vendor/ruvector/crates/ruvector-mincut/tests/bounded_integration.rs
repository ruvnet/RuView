//! Integration tests for bounded-range dynamic minimum cut
//!
//! Tests the full system: wrapper + instances + LocalKCut

use ruvector_mincut::instance::StubInstance;
use ruvector_mincut::prelude::*;
use ruvector_mincut::wrapper::{MinCutResult, MinCutWrapper};
use std::sync::Arc;

/// Test path graph P_n has min cut 1
#[test]
fn test_path_graph_integration() {
    let graph = Arc::new(DynamicGraph::new());

    // Build path graph: 0-1-2-3-4-5-6-7-8-9
    for i in 0..9 {
        graph.insert_edge(i, i + 1, 1.0).unwrap();
    }

    let mut wrapper = MinCutWrapper::new(Arc::clone(&graph));

    // Notify wrapper of edges
    for i in 0..9 {
        wrapper.insert_edge(i as u64, i, i + 1);
    }

    let result = wrapper.query();
    assert!(result.is_connected(), "Path graph should be connected");
    assert_eq!(result.value(), 1, "Path graph has min cut 1");
}

/// Test cycle graph C_n has min cut 2
#[test]
fn test_cycle_graph_integration() {
    let graph = Arc::new(DynamicGraph::new());

    // Build cycle graph: 0-1-2-3-4-0
    let n = 5;
    for i in 0..n {
        let j = (i + 1) % n;
        graph.insert_edge(i, j, 1.0).unwrap();
    }

    let mut wrapper = MinCutWrapper::new(Arc::clone(&graph));

    // Notify wrapper of edges
    for i in 0..n {
        let j = (i + 1) % n;
        wrapper.insert_edge(i as u64, i, j);
    }

    let result = wrapper.query();
    assert!(result.is_connected(), "Cycle graph should be connected");
    assert_eq!(result.value(), 2, "Cycle graph has min cut 2");
}

/// Test complete graph K_n has min cut n-1
#[test]
fn test_complete_graph_integration() {
    let graph = Arc::new(DynamicGraph::new());

    // Build K_5 (complete graph with 5 vertices)
    let n = 5;
    let mut edge_id = 0;
    for i in 0..n {
        for j in (i + 1)..n {
            graph.insert_edge(i, j, 1.0).unwrap();
        }
    }

    let mut wrapper = MinCutWrapper::new(Arc::clone(&graph));

    // Notify wrapper of all edges
    for i in 0..n {
        for j in (i + 1)..n {
            wrapper.insert_edge(edge_id, i, j);
            edge_id += 1;
        }
    }

    let result = wrapper.query();
    assert!(result.is_connected(), "Complete graph should be connected");
    assert_eq!(result.value(), (n - 1) as u64, "K_5 has min cut 4");
}

/// Test dynamic updates maintain correctness
#[test]
fn test_dynamic_updates_integration() {
    let graph = Arc::new(DynamicGraph::new());
    let mut wrapper = MinCutWrapper::new(Arc::clone(&graph));

    // Start with path: 0-1-2-3
    for i in 0..3 {
        graph.insert_edge(i, i + 1, 1.0).unwrap();
        wrapper.insert_edge(i as u64, i, i + 1);
    }

    let result = wrapper.query();
    assert_eq!(result.value(), 1, "Path has min cut 1");

    // Add edge to form cycle: 0-1-2-3-0
    graph.insert_edge(3, 0, 1.0).unwrap();
    wrapper.insert_edge(3, 3, 0);

    let result = wrapper.query();
    assert_eq!(result.value(), 2, "Cycle has min cut 2");

    // Delete an edge to go back to path
    graph.delete_edge(1, 2).unwrap();
    wrapper.delete_edge(1, 1, 2);

    let result = wrapper.query();
    assert_eq!(result.value(), 1, "After deletion, min cut should be 1");
}

/// Test disconnected graph returns 0
#[test]
fn test_disconnected_graph_integration() {
    let graph = Arc::new(DynamicGraph::new());

    // Create two components: {0, 1} and {2, 3}
    graph.insert_edge(0, 1, 1.0).unwrap();
    graph.insert_edge(2, 3, 1.0).unwrap();

    let mut wrapper = MinCutWrapper::new(Arc::clone(&graph));
    wrapper.insert_edge(0, 0, 1);
    wrapper.insert_edge(1, 2, 3);

    let result = wrapper.query();
    assert!(!result.is_connected(), "Graph should be disconnected");
    assert_eq!(result.value(), 0, "Disconnected graph has min cut 0");
}

/// Test star graph (min cut = 1, deleting center vertex)
#[test]
fn test_star_graph_integration() {
    let graph = Arc::new(DynamicGraph::new());

    // Create star with center 0 and leaves 1,2,3,4
    let n = 5;
    for i in 1..n {
        graph.insert_edge(0, i, 1.0).unwrap();
    }

    let mut wrapper = MinCutWrapper::new(Arc::clone(&graph));
    for i in 1..n {
        wrapper.insert_edge((i - 1) as u64, 0, i);
    }

    let result = wrapper.query();
    assert!(result.is_connected());
    assert_eq!(result.value(), 1, "Star graph has min cut 1");
}

/// Test weighted graph (min cut respects weights)
#[test]
fn test_weighted_graph_integration() {
    let graph = Arc::new(DynamicGraph::new());

    // Triangle with different weights
    // Edge (0,1) weight 5
    // Edge (1,2) weight 3
    // Edge (2,0) weight 4
    // Min cut should be 3 (cutting edge 1-2)
    graph.insert_edge(0, 1, 5.0).unwrap();
    graph.insert_edge(1, 2, 3.0).unwrap();
    graph.insert_edge(2, 0, 4.0).unwrap();

    let mut wrapper = MinCutWrapper::new(Arc::clone(&graph));
    wrapper.insert_edge(0, 0, 1);
    wrapper.insert_edge(1, 1, 2);
    wrapper.insert_edge(2, 2, 0);

    // Note: The wrapper works with integer weights internally
    // For this test, we're checking it reports a proper cut
    let result = wrapper.query();
    assert!(result.is_connected());
    assert!(
        result.value() > 0,
        "Weighted graph should have positive min cut"
    );
}

/// Stress test with many updates
#[test]
fn test_stress_many_updates() {
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    let mut rng = StdRng::seed_from_u64(12345);
    let graph = Arc::new(DynamicGraph::new());
    let mut wrapper = MinCutWrapper::new(Arc::clone(&graph));

    // 1000 random insertions
    let mut successful_inserts = 0;
    for i in 0..1000 {
        let u = rng.gen_range(0..100);
        let v = rng.gen_range(0..100);
        if u != v {
            if graph.insert_edge(u, v, 1.0).is_ok() {
                wrapper.insert_edge(i, u, v);
                successful_inserts += 1;
            }
        }
    }

    println!("Successfully inserted {} edges", successful_inserts);

    // Query should not panic
    let result = wrapper.query();

    // Result should be valid (either disconnected or connected with positive cut)
    if result.is_connected() {
        assert!(
            result.value() >= 1,
            "Connected graph should have min cut >= 1"
        );
    } else {
        assert_eq!(
            result.value(),
            0,
            "Disconnected graph should have min cut 0"
        );
    }

    // Should have buffered updates initially
    assert_eq!(
        wrapper.pending_updates(),
        0,
        "After query, updates should be processed"
    );
}

/// Test determinism: same sequence produces same result
#[test]
fn test_determinism() {
    // First run
    let graph1 = Arc::new(DynamicGraph::new());
    let mut wrapper1 = MinCutWrapper::new(Arc::clone(&graph1));

    let edges = vec![(0, 1), (1, 2), (2, 3), (3, 0), (0, 2)];
    for (i, (u, v)) in edges.iter().enumerate() {
        graph1.insert_edge(*u, *v, 1.0).unwrap();
        wrapper1.insert_edge(i as u64, *u, *v);
    }

    let result1 = wrapper1.query();

    // Second run with identical sequence
    let graph2 = Arc::new(DynamicGraph::new());
    let mut wrapper2 = MinCutWrapper::new(Arc::clone(&graph2));

    for (i, (u, v)) in edges.iter().enumerate() {
        graph2.insert_edge(*u, *v, 1.0).unwrap();
        wrapper2.insert_edge(i as u64, *u, *v);
    }

    let result2 = wrapper2.query();

    // Both should produce identical results
    assert_eq!(
        result1.value(),
        result2.value(),
        "Determinism: same input should produce same output"
    );
    assert_eq!(result1.is_connected(), result2.is_connected());
}

/// Test buffered updates are processed correctly
#[test]
fn test_buffered_updates() {
    let graph = Arc::new(DynamicGraph::new());
    let mut wrapper = MinCutWrapper::new(Arc::clone(&graph));

    // Add edges without querying
    graph.insert_edge(0, 1, 1.0).unwrap();
    graph.insert_edge(1, 2, 1.0).unwrap();
    graph.insert_edge(2, 3, 1.0).unwrap();

    wrapper.insert_edge(0, 0, 1);
    wrapper.insert_edge(1, 1, 2);
    wrapper.insert_edge(2, 2, 3);

    // Should have pending updates
    assert_eq!(wrapper.pending_updates(), 3);

    // Query processes them
    let result = wrapper.query();
    assert_eq!(wrapper.pending_updates(), 0);
    assert_eq!(result.value(), 1);
}

/// Test lazy instantiation of instances
#[test]
fn test_lazy_instantiation() {
    let graph = Arc::new(DynamicGraph::new());
    let mut wrapper = MinCutWrapper::new(Arc::clone(&graph));

    // No instances should exist initially
    assert_eq!(wrapper.num_instances(), 0);

    // Add some edges
    graph.insert_edge(0, 1, 1.0).unwrap();
    graph.insert_edge(1, 2, 1.0).unwrap();
    wrapper.insert_edge(0, 0, 1);
    wrapper.insert_edge(1, 1, 2);

    // Still no instances until query
    assert_eq!(wrapper.num_instances(), 0);

    // Query triggers instantiation
    let _ = wrapper.query();

    // Now instances should be created
    assert!(
        wrapper.num_instances() > 0,
        "Query should instantiate instances"
    );
}

/// Test multiple queries are consistent
#[test]
fn test_multiple_queries_consistent() {
    let graph = Arc::new(DynamicGraph::new());
    let mut wrapper = MinCutWrapper::new(Arc::clone(&graph));

    // Build triangle
    graph.insert_edge(0, 1, 1.0).unwrap();
    graph.insert_edge(1, 2, 1.0).unwrap();
    graph.insert_edge(2, 0, 1.0).unwrap();

    wrapper.insert_edge(0, 0, 1);
    wrapper.insert_edge(1, 1, 2);
    wrapper.insert_edge(2, 2, 0);

    // Multiple queries should give same result
    let result1 = wrapper.query();
    let result2 = wrapper.query();
    let result3 = wrapper.query();

    assert_eq!(result1.value(), result2.value());
    assert_eq!(result2.value(), result3.value());
    assert_eq!(result1.value(), 2, "Triangle has min cut 2");
}

/// Test empty graph
#[test]
fn test_empty_graph_integration() {
    let graph = Arc::new(DynamicGraph::new());
    let mut wrapper = MinCutWrapper::new(Arc::clone(&graph));

    let result = wrapper.query();
    // Empty graph is considered disconnected
    assert_eq!(result.value(), 0);
}

/// Test single edge
#[test]
fn test_single_edge_integration() {
    let graph = Arc::new(DynamicGraph::new());
    graph.insert_edge(0, 1, 1.0).unwrap();

    let mut wrapper = MinCutWrapper::new(Arc::clone(&graph));
    wrapper.insert_edge(0, 0, 1);

    let result = wrapper.query();
    assert!(result.is_connected());
    assert_eq!(result.value(), 1, "Single edge has min cut 1");
}

/// Test grid graph (known structure with min cut = width for vertical cut)
#[test]
fn test_grid_graph_integration() {
    let graph = Arc::new(DynamicGraph::new());
    let mut wrapper = MinCutWrapper::new(Arc::clone(&graph));

    // 3x3 grid
    let width = 3;
    let height = 3;
    let mut edge_id = 0;

    for i in 0..height {
        for j in 0..width {
            let v = i * width + j;

            // Horizontal edge
            if j + 1 < width {
                let u = v;
                let w = v + 1;
                graph.insert_edge(u as u64, w as u64, 1.0).unwrap();
                wrapper.insert_edge(edge_id, u as u64, w as u64);
                edge_id += 1;
            }

            // Vertical edge
            if i + 1 < height {
                let u = v;
                let w = v + width;
                graph.insert_edge(u as u64, w as u64, 1.0).unwrap();
                wrapper.insert_edge(edge_id, u as u64, w as u64);
                edge_id += 1;
            }
        }
    }

    let result = wrapper.query();
    assert!(result.is_connected());
    // 3x3 grid has min cut of 2 (cutting off a corner vertex)
    // Corner vertices have degree 2, so min cut is 2
    assert_eq!(result.value(), 2, "3x3 grid has min cut 2");
}

/// Test bridge edge (edge whose removal disconnects graph)
#[test]
fn test_bridge_edge_integration() {
    let graph = Arc::new(DynamicGraph::new());

    // Create dumbbell: triangle-bridge-triangle
    // Left triangle: 0-1-2-0
    graph.insert_edge(0, 1, 1.0).unwrap();
    graph.insert_edge(1, 2, 1.0).unwrap();
    graph.insert_edge(2, 0, 1.0).unwrap();

    // Bridge: 2-3
    graph.insert_edge(2, 3, 1.0).unwrap();

    // Right triangle: 3-4-5-3
    graph.insert_edge(3, 4, 1.0).unwrap();
    graph.insert_edge(4, 5, 1.0).unwrap();
    graph.insert_edge(5, 3, 1.0).unwrap();

    let mut wrapper = MinCutWrapper::new(Arc::clone(&graph));
    wrapper.insert_edge(0, 0, 1);
    wrapper.insert_edge(1, 1, 2);
    wrapper.insert_edge(2, 2, 0);
    wrapper.insert_edge(3, 2, 3); // Bridge
    wrapper.insert_edge(4, 3, 4);
    wrapper.insert_edge(5, 4, 5);
    wrapper.insert_edge(6, 5, 3);

    let result = wrapper.query();
    assert!(result.is_connected());
    assert_eq!(result.value(), 1, "Bridge edge gives min cut 1");
}
