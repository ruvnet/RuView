//! Integration tests for LocalKCut algorithm
//!
//! Tests the full LocalKCut implementation including:
//! - Edge cases and boundary conditions
//! - Determinism and reproducibility
//! - Correctness on known graph structures
//! - Performance characteristics

use ruvector_mincut::prelude::*;
use std::sync::Arc;

#[test]
fn test_bridge_detection() {
    // Create a graph with a clear bridge
    let graph = Arc::new(DynamicGraph::new());

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

    let local_kcut = LocalKCut::new(graph, 5);
    let result = local_kcut.find_cut(1).expect("Should find a cut");

    // Should detect the bridge with cut value 1
    assert_eq!(result.cut_value, 1.0, "Bridge should have cut value 1");
    assert_eq!(result.cut_edges.len(), 1, "Bridge should be a single edge");
    assert!(result.cut_edges[0] == (3, 4) || result.cut_edges[0] == (4, 3));
}

#[test]
fn test_deterministic_behavior() {
    let graph = Arc::new(DynamicGraph::new());

    // Create a path graph
    for i in 1..=10 {
        graph.insert_edge(i, i + 1, 1.0).unwrap();
    }

    // Create two instances
    let lk1 = LocalKCut::new(graph.clone(), 3);
    let lk2 = LocalKCut::new(graph.clone(), 3);

    // Colors should be identical
    for edge in graph.edges() {
        assert_eq!(
            lk1.edge_color(edge.id),
            lk2.edge_color(edge.id),
            "Colors must be deterministic"
        );
    }

    // Results should be identical
    for vertex in 1..=11 {
        let result1 = lk1.find_cut(vertex);
        let result2 = lk2.find_cut(vertex);

        match (result1, result2) {
            (Some(r1), Some(r2)) => {
                assert_eq!(r1.cut_value, r2.cut_value, "Cut values must match");
                assert_eq!(r1.cut_set, r2.cut_set, "Cut sets must match");
            }
            (None, None) => {}
            _ => panic!("Results must both exist or both be None"),
        }
    }
}

#[test]
fn test_empty_graph() {
    let graph = Arc::new(DynamicGraph::new());
    let local_kcut = LocalKCut::new(graph, 5);

    // Should return None for non-existent vertex
    assert!(local_kcut.find_cut(1).is_none());
}

#[test]
fn test_single_edge() {
    let graph = Arc::new(DynamicGraph::new());
    graph.insert_edge(1, 2, 3.0).unwrap();

    let local_kcut = LocalKCut::new(graph, 5);
    let result = local_kcut.find_cut(1).expect("Should find a cut");

    // The only cut is the single edge
    assert_eq!(result.cut_value, 3.0);
    assert_eq!(result.cut_edges.len(), 1);
}

#[test]
fn test_complete_graph_k4() {
    let graph = Arc::new(DynamicGraph::new());

    // Complete graph K4 (all edges have weight 1)
    for i in 1..=4 {
        for j in i + 1..=4 {
            graph.insert_edge(i, j, 1.0).unwrap();
        }
    }

    let local_kcut = LocalKCut::new(graph.clone(), 5);

    // Find cuts from each vertex
    for vertex in 1..=4 {
        if let Some(result) = local_kcut.find_cut(vertex) {
            // In K4, minimum cut is 3 (separating one vertex from the rest)
            assert!(result.cut_value >= 3.0, "K4 minimum cut is at least 3");
        }
    }
}

#[test]
fn test_star_graph() {
    let graph = Arc::new(DynamicGraph::new());

    // Star graph: center vertex 1 connected to 5 leaves
    for i in 2..=6 {
        graph.insert_edge(1, i, 1.0).unwrap();
    }

    let local_kcut = LocalKCut::new(graph.clone(), 3);

    // From a leaf, should find cut separating that leaf
    let result = local_kcut.find_cut(2).expect("Should find a cut");
    assert_eq!(result.cut_value, 1.0, "Leaf should have cut value 1");

    // From center, harder to find small cut
    let result = local_kcut.find_cut(1);
    if let Some(r) = result {
        assert!(r.cut_value <= 3.0);
    }
}

#[test]
fn test_cycle_graph() {
    let graph = Arc::new(DynamicGraph::new());

    // Cycle graph: 1-2-3-4-5-1
    let n = 8;
    for i in 1..=n {
        graph
            .insert_edge(i, if i == n { 1 } else { i + 1 }, 1.0)
            .unwrap();
    }

    let local_kcut = LocalKCut::new(graph, 3);

    // In a cycle, any cut needs at least 2 edges
    for vertex in 1..=n {
        if let Some(result) = local_kcut.find_cut(vertex) {
            assert!(
                result.cut_value >= 2.0,
                "Cycle requires at least 2 edges to cut"
            );
        }
    }
}

#[test]
fn test_weighted_edges() {
    let graph = Arc::new(DynamicGraph::new());

    // Graph with varying weights
    graph.insert_edge(1, 2, 5.0).unwrap();
    graph.insert_edge(2, 3, 1.0).unwrap();
    graph.insert_edge(3, 4, 5.0).unwrap();

    let local_kcut = LocalKCut::new(graph, 3);

    // Should prefer to cut the edge with weight 1
    let result = local_kcut.find_cut(2).expect("Should find a cut");

    assert!(result.cut_value <= 3.0, "Should find cut with value <= k=3");
}

#[test]
fn test_color_mask_combinations() {
    // Test various color combinations
    let test_cases = vec![
        (vec![], 0),
        (vec![EdgeColor::Red], 1),
        (vec![EdgeColor::Red, EdgeColor::Blue], 2),
        (vec![EdgeColor::Red, EdgeColor::Blue, EdgeColor::Green], 3),
        (EdgeColor::all().to_vec(), 4),
    ];

    for (colors, expected_count) in test_cases {
        let mask = ColorMask::from_colors(&colors);
        assert_eq!(mask.count(), expected_count);

        // Verify each specified color is in the mask
        for color in &colors {
            assert!(mask.contains(*color));
        }
    }

    // Test empty and all masks
    assert_eq!(ColorMask::empty().count(), 0);
    assert_eq!(ColorMask::all().count(), 4);

    for color in EdgeColor::all() {
        assert!(ColorMask::all().contains(color));
        assert!(!ColorMask::empty().contains(color));
    }
}

#[test]
fn test_forest_packing_completeness() {
    let graph = Arc::new(DynamicGraph::new());

    // Create a grid graph
    for i in 0..3 {
        for j in 0..3 {
            let v = i * 3 + j + 1;
            if j < 2 {
                graph.insert_edge(v, v + 1, 1.0).unwrap();
            }
            if i < 2 {
                graph.insert_edge(v, v + 3, 1.0).unwrap();
            }
        }
    }

    let packing = ForestPacking::greedy_packing(&*graph, 5, 0.1);

    // Should have created multiple forests
    assert!(packing.num_forests() > 0);

    // Each forest should be acyclic
    for i in 0..packing.num_forests() {
        if let Some(forest) = packing.forest(i) {
            // Forest should have at most n-1 edges for n vertices
            assert!(forest.len() <= graph.num_vertices());
        }
    }
}

#[test]
fn test_forest_packing_witness() {
    let graph = Arc::new(DynamicGraph::new());

    // Simple graph - a cycle
    graph.insert_edge(1, 2, 1.0).unwrap();
    graph.insert_edge(2, 3, 1.0).unwrap();
    graph.insert_edge(3, 4, 1.0).unwrap();
    graph.insert_edge(1, 4, 1.0).unwrap();

    let packing = ForestPacking::greedy_packing(&*graph, 3, 0.1);

    // Verify forest packing was created
    assert!(
        packing.num_forests() >= 1,
        "Should have at least one forest"
    );

    // Test witness property on single-edge cuts
    let cuts = vec![vec![(1, 2)], vec![(2, 3)]];

    // Just verify the method works without panic
    for cut in cuts {
        let _is_witnessed = packing.witnesses_cut(&cut);
        // Result depends on random forest structure
    }
}

#[test]
fn test_radius_increases_with_k() {
    let graph = Arc::new(DynamicGraph::new());
    graph.insert_edge(1, 2, 1.0).unwrap();

    // Create instances with different k values
    let lk1 = LocalKCut::new(graph.clone(), 1);
    let lk2 = LocalKCut::new(graph.clone(), 10);
    let lk3 = LocalKCut::new(graph.clone(), 100);

    // Radius should increase or stay the same as k increases
    assert!(lk1.radius() <= lk2.radius());
    assert!(lk2.radius() <= lk3.radius());
}

#[test]
fn test_enumerate_paths_diversity() {
    let graph = Arc::new(DynamicGraph::new());

    // Create a graph with multiple paths
    //     1 - 2 - 3
    //     |   |   |
    //     4 - 5 - 6

    graph.insert_edge(1, 2, 1.0).unwrap();
    graph.insert_edge(2, 3, 1.0).unwrap();
    graph.insert_edge(1, 4, 1.0).unwrap();
    graph.insert_edge(2, 5, 1.0).unwrap();
    graph.insert_edge(3, 6, 1.0).unwrap();
    graph.insert_edge(4, 5, 1.0).unwrap();
    graph.insert_edge(5, 6, 1.0).unwrap();

    let local_kcut = LocalKCut::new(graph, 5);

    let paths = local_kcut.enumerate_paths(1, 3);

    // Should find multiple different reachable sets
    assert!(paths.len() > 1, "Should find multiple paths");

    // All paths should contain the start vertex
    for path in &paths {
        assert!(path.contains(&1), "All paths should contain start vertex");
    }

    // Paths should have different sizes (due to different color masks)
    let mut sizes: Vec<_> = paths.iter().map(|p| p.len()).collect();
    sizes.sort_unstable();
    sizes.dedup();
    assert!(sizes.len() > 1, "Should have paths of different sizes");
}

#[test]
fn test_large_k_bound() {
    let graph = Arc::new(DynamicGraph::new());

    // Small graph
    graph.insert_edge(1, 2, 1.0).unwrap();
    graph.insert_edge(2, 3, 1.0).unwrap();

    // Very large k should still work
    let local_kcut = LocalKCut::new(graph, 1000);
    let result = local_kcut.find_cut(1);

    assert!(result.is_some(), "Should find a cut even with large k");
}

#[test]
fn test_disconnected_graph() {
    let graph = Arc::new(DynamicGraph::new());

    // Two disconnected components
    graph.insert_edge(1, 2, 1.0).unwrap();
    graph.insert_edge(3, 4, 1.0).unwrap();

    let local_kcut = LocalKCut::new(graph, 5);

    // From component 1, should find cut with value 0
    let result1 = local_kcut.find_cut(1);
    assert!(result1.is_some(), "Should find cut in disconnected graph");

    // From component 2
    let result2 = local_kcut.find_cut(3);
    assert!(result2.is_some(), "Should find cut in disconnected graph");
}

#[test]
fn test_local_cut_result_properties() {
    let graph = Arc::new(DynamicGraph::new());

    // Create a simple graph
    for i in 1..=5 {
        graph.insert_edge(i, i + 1, 1.0).unwrap();
    }

    let local_kcut = LocalKCut::new(graph.clone(), 3);
    let result = local_kcut.find_cut(3).expect("Should find a cut");

    // Verify result properties
    assert!(result.cut_value > 0.0);
    assert!(!result.cut_set.is_empty());
    assert!(!result.cut_edges.is_empty());
    assert!(result.iterations > 0);

    // Cut set should not include all vertices
    assert!(result.cut_set.len() < graph.num_vertices());

    // Cut edges should match the cut value
    let mut computed_value = 0.0;
    for (u, v) in &result.cut_edges {
        if let Some(weight) = graph.edge_weight(*u, *v) {
            computed_value += weight;
        }
    }
    assert!(
        (result.cut_value - computed_value).abs() < 0.001,
        "Cut value should match sum of edge weights"
    );
}

#[test]
fn test_community_structure_detection() {
    let graph = Arc::new(DynamicGraph::new());

    // Create two dense communities with weak inter-connections
    // Community 1: {1, 2, 3}
    graph.insert_edge(1, 2, 5.0).unwrap();
    graph.insert_edge(2, 3, 5.0).unwrap();
    graph.insert_edge(3, 1, 5.0).unwrap();

    // Weak connection
    graph.insert_edge(3, 4, 1.0).unwrap();

    // Community 2: {4, 5, 6}
    graph.insert_edge(4, 5, 5.0).unwrap();
    graph.insert_edge(5, 6, 5.0).unwrap();
    graph.insert_edge(6, 4, 5.0).unwrap();

    let local_kcut = LocalKCut::new(graph, 5);

    // From community 1, should find the weak connection
    let result = local_kcut.find_cut(1).expect("Should find a cut");

    // Should find the weak inter-community edge
    assert!(
        result.cut_value <= 5.0,
        "Should find cut along weak connection"
    );

    // The cut should separate the communities
    let separates_communities = result.cut_set.len() == 3
        && (result.cut_set.contains(&1)
            && result.cut_set.contains(&2)
            && result.cut_set.contains(&3));

    assert!(
        separates_communities || result.cut_set.len() < 3,
        "Cut should respect community structure"
    );
}

#[test]
fn test_performance_characteristics() {
    // Test that algorithm performs reasonably on various graph sizes
    for n in [10, 20, 50] {
        let graph = Arc::new(DynamicGraph::new());

        // Create a path graph
        for i in 1..n {
            graph.insert_edge(i, i + 1, 1.0).unwrap();
        }

        let start = std::time::Instant::now();
        let local_kcut = LocalKCut::new(graph.clone(), 5);

        // Find cuts from a few vertices
        for &v in &[1, n / 2, n - 1] {
            let _ = local_kcut.find_cut(v);
        }

        let elapsed = start.elapsed();

        // Should complete in reasonable time (< 100ms for these small graphs)
        assert!(
            elapsed.as_millis() < 100,
            "Should complete in reasonable time for n={}",
            n
        );
    }
}
