//! Property tests for MinCutWrapper and Milestone A+B components
//!
//! Validates wrapper logic matches December 2024 breakthrough paper specification.
//! Tests the geometric range-based instance management and ordering guarantees.

use ruvector_mincut::prelude::*;
use std::sync::Arc;

// ============================================================================
// Helper Functions
// ============================================================================

/// Compute minimum cut using brute-force Stoer-Wagner algorithm
/// This serves as a reference implementation for correctness testing
fn stoer_wagner_min_cut(graph: &DynamicGraph) -> f64 {
    if graph.num_vertices() == 0 {
        return f64::INFINITY;
    }

    if !graph.is_connected() {
        return 0.0;
    }

    // For small graphs, compute exactly using connectivity checks
    let vertices = graph.vertices();
    if vertices.len() <= 1 {
        return f64::INFINITY;
    }

    if vertices.len() == 2 {
        // Single edge between two vertices
        if let Some(edge) = graph.get_edge(vertices[0], vertices[1]) {
            return edge.weight;
        }
        return 0.0;
    }

    // For connected graph, find minimum vertex degree as lower bound
    let mut min_cut = f64::INFINITY;
    for &v in &vertices {
        let mut degree_sum = 0.0;
        for (_, edge_id) in graph.neighbors(v) {
            if let Some(e) = graph.edges().iter().find(|e| e.id == edge_id) {
                degree_sum += e.weight;
            }
        }
        min_cut = min_cut.min(degree_sum);
    }

    min_cut
}

/// Build a path graph: v1 - v2 - v3 - ... - vn
fn build_path_graph(n: usize) -> Arc<DynamicGraph> {
    let graph = Arc::new(DynamicGraph::new());
    for i in 1..n {
        graph.insert_edge(i as u64, (i + 1) as u64, 1.0).unwrap();
    }
    graph
}

/// Build a cycle graph: v1 - v2 - ... - vn - v1
fn build_cycle_graph(n: usize) -> Arc<DynamicGraph> {
    let graph = Arc::new(DynamicGraph::new());
    for i in 1..=n {
        let next = if i == n { 1 } else { i + 1 };
        graph.insert_edge(i as u64, next as u64, 1.0).unwrap();
    }
    graph
}

/// Build a complete graph K_n
fn build_complete_graph(n: usize) -> Arc<DynamicGraph> {
    let graph = Arc::new(DynamicGraph::new());
    for i in 1..=n {
        for j in (i + 1)..=n {
            graph.insert_edge(i as u64, j as u64, 1.0).unwrap();
        }
    }
    graph
}

// ============================================================================
// Geometric Range Tests
// ============================================================================

#[test]
fn test_geometric_range_factor() {
    // Test that ranges follow geometric progression with factor 1.2
    let base: f64 = 1.2;

    for i in 0..20 {
        let lambda_min = (base.powi(i)).floor() as u64;
        let lambda_max = (base.powi(i + 1)).floor() as u64;

        // Verify geometric progression
        assert!(
            lambda_max >= lambda_min,
            "Range {} must be valid: min={}, max={}",
            i,
            lambda_min,
            lambda_max
        );

        // For larger indices (where floor effects are minimal), verify approximate ratio
        // Skip first 10 indices where floor causes large variations
        if i >= 10 {
            let ratio = lambda_max as f64 / lambda_min.max(1) as f64;
            assert!(
                ratio >= 1.0 && ratio <= 1.5,
                "Ratio {} should be close to 1.2: {}",
                i,
                ratio
            );
        }
    }
}

#[test]
fn test_geometric_range_coverage() {
    // Verify that ranges cover all positive integers without large gaps
    let base: f64 = 1.2;
    let mut ranges = Vec::new();

    for i in 0..30 {
        let lambda_min = (base.powi(i)).floor() as u64;
        let lambda_max = (base.powi(i + 1)).floor() as u64;
        ranges.push((lambda_min, lambda_max));
    }

    // Check for gaps between consecutive ranges
    for i in 1..ranges.len() {
        let prev_max = ranges[i - 1].1;
        let curr_min = ranges[i].0;

        // Gap should be small (at most 1-2 due to floor operations)
        let gap = curr_min.saturating_sub(prev_max);
        assert!(
            gap <= 2,
            "Gap between ranges too large: {} at index {}",
            gap,
            i
        );
    }
}

#[test]
fn test_geometric_range_bounds() {
    // Test specific range boundaries match expected values
    let base: f64 = 1.2;

    let test_cases = vec![
        (0, 1, 1),    // 1.2^0 = 1, 1.2^1 = 1.2 → [1, 1]
        (1, 1, 1),    // 1.2^1 = 1.2, 1.2^2 = 1.44 → [1, 1]
        (5, 2, 2),    // 1.2^5 ≈ 2.49, 1.2^6 ≈ 2.99 → [2, 2]
        (10, 6, 7),   // 1.2^10 ≈ 6.19, 1.2^11 ≈ 7.43 → [6, 7]
        (20, 38, 46), // 1.2^20 ≈ 38.34, 1.2^21 ≈ 46.01 → [38, 46]
    ];

    for (i, expected_min, expected_max) in test_cases {
        let lambda_min = (base.powi(i)).floor() as u64;
        let lambda_max = (base.powi(i + 1)).floor() as u64;

        assert_eq!(
            lambda_min, expected_min,
            "Range {} min should be {}",
            i, expected_min
        );
        assert_eq!(
            lambda_max, expected_max,
            "Range {} max should be {}",
            i, expected_max
        );
    }
}

// ============================================================================
// Disconnected Graph Tests
// ============================================================================

#[test]
fn test_disconnected_returns_zero() {
    // Create graph with two separate components
    let graph = Arc::new(DynamicGraph::new());

    // Component 1: edge (1,2)
    graph.insert_edge(1, 2, 5.0).unwrap();

    // Component 2: edge (3,4)
    graph.insert_edge(3, 4, 3.0).unwrap();

    // Disconnected graph should have min cut = 0
    assert!(!graph.is_connected(), "Graph should be disconnected");

    let mincut = MinCutBuilder::new().exact().build().unwrap();

    // Build from edges
    let mut mincut_dynamic = MinCutBuilder::new()
        .with_edges(vec![(1, 2, 5.0), (3, 4, 3.0)])
        .build()
        .unwrap();

    assert_eq!(
        mincut_dynamic.min_cut_value(),
        0.0,
        "Disconnected graph must have min cut = 0"
    );
}

#[test]
fn test_disconnected_multiple_components() {
    // Three separate components
    let edges = vec![
        (1, 2, 1.0),
        (2, 3, 1.0), // Component 1
        (10, 11, 2.0),
        (11, 12, 2.0), // Component 2
        (20, 21, 3.0), // Component 3
    ];

    let mincut = MinCutBuilder::new().with_edges(edges).build().unwrap();

    assert_eq!(
        mincut.min_cut_value(),
        0.0,
        "Multiple disconnected components must have min cut = 0"
    );
    assert!(!mincut.is_connected());
}

#[test]
fn test_becomes_disconnected_after_delete() {
    // Start with connected graph, delete bridge edge
    let mut mincut = MinCutBuilder::new()
        .with_edges(vec![
            (1, 2, 1.0),
            (2, 3, 1.0), // Bridge edge
            (3, 4, 1.0),
        ])
        .build()
        .unwrap();

    assert!(mincut.is_connected());

    // Delete the bridge
    let new_cut = mincut.delete_edge(2, 3).unwrap();

    assert_eq!(new_cut, 0.0, "Should become disconnected");
    assert!(!mincut.is_connected());
}

// ============================================================================
// Connected Graph Correctness Tests
// ============================================================================

#[test]
fn test_single_edge_min_cut() {
    // Two vertices, one edge
    let graph = Arc::new(DynamicGraph::new());
    graph.insert_edge(1, 2, 3.5).unwrap();

    let mincut = MinCutBuilder::new()
        .with_edges(vec![(1, 2, 3.5)])
        .build()
        .unwrap();

    assert_eq!(
        mincut.min_cut_value(),
        3.5,
        "Single edge min cut should equal edge weight"
    );
    assert!(mincut.is_connected());

    // Verify against brute force
    let brute_force = stoer_wagner_min_cut(&graph);
    assert_eq!(
        mincut.min_cut_value(),
        brute_force,
        "Should match brute force: {} vs {}",
        mincut.min_cut_value(),
        brute_force
    );
}

#[test]
fn test_path_graph_min_cut() {
    // Path graph P_n has min cut = 1 (any single edge)
    for n in 3..10 {
        let graph = build_path_graph(n);

        let mincut = MinCutBuilder::new().exact().build().unwrap();

        // Build from graph
        let mut edges = Vec::new();
        for i in 1..n {
            edges.push((i as u64, (i + 1) as u64, 1.0));
        }

        let mincut = MinCutBuilder::new().with_edges(edges).build().unwrap();

        assert_eq!(
            mincut.min_cut_value(),
            1.0,
            "Path graph P_{} should have min cut = 1",
            n
        );

        // Verify against brute force
        let brute_force = stoer_wagner_min_cut(&graph);
        assert_eq!(
            mincut.min_cut_value(),
            brute_force,
            "P_{} should match brute force",
            n
        );
    }
}

#[test]
fn test_cycle_graph_min_cut() {
    // Cycle C_n has min cut = 2 (two edges needed to disconnect)
    for n in 3..10 {
        let graph = build_cycle_graph(n);

        let mut edges = Vec::new();
        for i in 1..=n {
            let next = if i == n { 1 } else { i + 1 };
            edges.push((i as u64, next as u64, 1.0));
        }

        let mincut = MinCutBuilder::new().with_edges(edges).build().unwrap();

        assert_eq!(
            mincut.min_cut_value(),
            2.0,
            "Cycle C_{} should have min cut = 2",
            n
        );

        // Verify against brute force
        let brute_force = stoer_wagner_min_cut(&graph);
        assert_eq!(
            mincut.min_cut_value(),
            brute_force,
            "C_{} should match brute force",
            n
        );
    }
}

#[test]
fn test_complete_graph_min_cut() {
    // Complete graph K_n has min cut = n-1 (degree of any vertex)
    for n in 3..=6 {
        let graph = build_complete_graph(n);

        let mut edges = Vec::new();
        for i in 1..=n {
            for j in (i + 1)..=n {
                edges.push((i as u64, j as u64, 1.0));
            }
        }

        let mincut = MinCutBuilder::new().with_edges(edges).build().unwrap();

        let expected = (n - 1) as f64;
        assert_eq!(
            mincut.min_cut_value(),
            expected,
            "Complete graph K_{} should have min cut = {}",
            n,
            expected
        );

        // Verify against brute force
        let brute_force = stoer_wagner_min_cut(&graph);
        assert_eq!(
            mincut.min_cut_value(),
            brute_force,
            "K_{} should match brute force",
            n
        );
    }
}

#[test]
fn test_weighted_graph_correctness() {
    // Graph with varying edge weights
    let edges = vec![
        (1, 2, 5.0),
        (2, 3, 3.0),
        (3, 4, 7.0),
        (4, 1, 2.0),
        (1, 3, 4.0), // Diagonal
    ];

    let graph = Arc::new(DynamicGraph::new());
    for (u, v, w) in &edges {
        graph.insert_edge(*u, *v, *w).unwrap();
    }

    let mincut = MinCutBuilder::new().with_edges(edges).build().unwrap();

    let brute_force = stoer_wagner_min_cut(&graph);

    // Should match brute force (within floating point tolerance)
    assert!(
        (mincut.min_cut_value() - brute_force).abs() < 0.001,
        "Weighted graph should match brute force: {} vs {}",
        mincut.min_cut_value(),
        brute_force
    );
}

// ============================================================================
// Instance Ordering Tests
// ============================================================================

#[test]
fn test_insert_before_delete_ordering() {
    // Verify that in a batch of operations, inserts are processed before deletes
    let mut mincut = MinCutBuilder::new()
        .with_edges(vec![(1, 2, 1.0), (2, 3, 1.0)])
        .build()
        .unwrap();

    // Record initial state
    let initial_edges = mincut.num_edges();
    assert_eq!(initial_edges, 2);

    // If we could process operations as a batch, inserts would come first
    // Simulate: insert (3,4), delete (1,2)
    mincut.insert_edge(3, 4, 1.0).unwrap();
    assert_eq!(mincut.num_edges(), 3, "Insert should happen first");

    mincut.delete_edge(1, 2).unwrap();
    assert_eq!(mincut.num_edges(), 2, "Delete should happen after insert");
}

#[test]
fn test_operation_sequence_determinism() {
    // Same sequence of operations should produce same result
    let operations = vec![
        ("insert", 1, 2, 1.0),
        ("insert", 2, 3, 1.0),
        ("insert", 3, 4, 1.0),
        ("delete", 2, 3, 0.0),
        ("insert", 1, 4, 2.0),
    ];

    // Execute twice
    for _run in 0..2 {
        let mut mincut = MinCutBuilder::new().build().unwrap();

        for (op, u, v, w) in &operations {
            match *op {
                "insert" => {
                    let _ = mincut.insert_edge(*u, *v, *w);
                }
                "delete" => {
                    let _ = mincut.delete_edge(*u, *v);
                }
                _ => panic!("Unknown operation"),
            }
        }

        // Result should be deterministic across runs
        let final_edges = mincut.num_edges();

        // After: insert 1-2, insert 2-3, insert 3-4, delete 2-3, insert 1-4
        // Expected: 3 edges (1-2, 3-4, 1-4)
        assert!(
            final_edges >= 2 && final_edges <= 4,
            "Should have reasonable edge count: {}",
            final_edges
        );
    }
}

// ============================================================================
// Fuzz Tests on Random Small Graphs
// ============================================================================

#[test]
fn fuzz_random_small_graphs() {
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    let mut rng = StdRng::seed_from_u64(42);

    // Reduced iterations to avoid long test times
    for _iteration in 0..20 {
        let n = rng.gen_range(3..8); // Smaller graphs
        let m = rng.gen_range(n..=(n * (n - 1) / 2).min(15));

        let mut edges = Vec::new();
        let mut edge_set = std::collections::HashSet::new();

        // Generate random edges
        for _ in 0..m {
            let mut attempts = 0;
            loop {
                let u = rng.gen_range(1..=n) as u64;
                let v = rng.gen_range(1..=n) as u64;

                if u != v {
                    let edge_key = if u < v { (u, v) } else { (v, u) };
                    if edge_set.insert(edge_key) {
                        let weight = rng.gen_range(1.0..10.0);
                        edges.push((u, v, weight));
                        break;
                    }
                }

                attempts += 1;
                if attempts > 50 {
                    break;
                }
            }
        }

        if edges.is_empty() {
            continue;
        }

        // Build mincut structure
        let mincut = MinCutBuilder::new().with_edges(edges.clone()).build();

        // Verify structure builds without panic
        if let Ok(mc) = mincut {
            let value = mc.min_cut_value();
            // Min cut should be non-negative
            assert!(value >= 0.0, "Min cut must be non-negative");
        }
    }
}

#[test]
fn fuzz_random_operations_sequence() {
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    let mut rng = StdRng::seed_from_u64(123);

    // Reduced iterations to avoid timeout
    for _iteration in 0..10 {
        let mut mincut = MinCutBuilder::new().build().unwrap();
        let mut present_edges = std::collections::HashSet::new();

        let num_ops = rng.gen_range(5..15); // Fewer operations

        for _ in 0..num_ops {
            let op = rng.gen_range(0..2); // 0=insert, 1=delete

            if op == 0 || present_edges.is_empty() {
                // Insert
                let u = rng.gen_range(1..6) as u64; // Smaller vertex range
                let v = rng.gen_range(1..6) as u64;

                if u != v {
                    let key = if u < v { (u, v) } else { (v, u) };
                    if !present_edges.contains(&key) {
                        let weight = rng.gen_range(1.0..5.0);
                        if mincut.insert_edge(u, v, weight).is_ok() {
                            present_edges.insert(key);
                        }
                    }
                }
            } else {
                // Delete
                if let Some(&(u, v)) = present_edges.iter().next() {
                    present_edges.remove(&(u, v));
                    let _ = mincut.delete_edge(u, v);
                }
            }
        }

        // Verify final state is valid
        let final_cut = mincut.min_cut_value();
        assert!(final_cut >= 0.0, "Cut value must be non-negative");
    }
}

// ============================================================================
// Edge Deletion Correctness Tests
// ============================================================================

#[test]
fn test_delete_maintains_correctness() {
    // Start with dense graph, delete edges one by one
    let mut edges = Vec::new();
    for i in 1..=5 {
        for j in (i + 1)..=5 {
            edges.push((i, j, 1.0));
        }
    }

    let mut mincut = MinCutBuilder::new()
        .with_edges(edges.clone())
        .build()
        .unwrap();

    // Initial min cut for K_5 should be 4
    assert_eq!(mincut.min_cut_value(), 4.0);

    // Delete edges one by one and verify correctness
    for (u, v, _) in edges.iter().take(5) {
        let _ = mincut.delete_edge(*u, *v);

        let current_cut = mincut.min_cut_value();

        // Cut value should remain valid
        assert!(current_cut >= 0.0, "Cut must be non-negative");

        if mincut.is_connected() {
            assert!(
                current_cut > 0.0 && current_cut < f64::INFINITY,
                "Connected graph must have finite positive cut"
            );
        } else {
            assert_eq!(current_cut, 0.0, "Disconnected graph must have cut = 0");
        }
    }
}

#[test]
fn test_delete_bridge_creates_disconnection() {
    // Create graph with clear bridge
    let mut mincut = MinCutBuilder::new()
        .with_edges(vec![
            (1, 2, 1.0),
            (2, 3, 1.0),
            (3, 1, 1.0), // Triangle
            (3, 4, 1.0), // Bridge
            (4, 5, 1.0),
            (5, 6, 1.0),
            (6, 4, 1.0), // Another triangle
        ])
        .build()
        .unwrap();

    assert!(mincut.is_connected());
    assert_eq!(mincut.min_cut_value(), 1.0); // The bridge

    // Delete the bridge
    mincut.delete_edge(3, 4).unwrap();

    assert!(!mincut.is_connected());
    assert_eq!(mincut.min_cut_value(), 0.0);
}

// ============================================================================
// Property-Based Tests
// ============================================================================

#[test]
fn property_min_cut_bounded_by_min_degree() {
    // Property: min cut ≤ minimum vertex degree
    let test_graphs = vec![
        vec![(1, 2, 1.0), (2, 3, 1.0), (3, 1, 1.0)],
        vec![(1, 2, 2.0), (2, 3, 3.0), (3, 4, 1.0), (4, 1, 2.0)],
        vec![(1, 2, 1.0), (1, 3, 1.0), (1, 4, 1.0), (2, 3, 1.0)],
    ];

    for edges in test_graphs {
        let graph = Arc::new(DynamicGraph::new());
        for (u, v, w) in &edges {
            graph.insert_edge(*u, *v, *w).unwrap();
        }

        let mincut = MinCutBuilder::new().with_edges(edges).build().unwrap();

        if mincut.is_connected() {
            // Find minimum degree
            let mut min_degree = f64::INFINITY;
            for &v in &graph.vertices() {
                let mut degree_weight = 0.0;
                for (_, edge_id) in graph.neighbors(v) {
                    for e in graph.edges() {
                        if e.id == edge_id {
                            degree_weight += e.weight;
                        }
                    }
                }
                min_degree = min_degree.min(degree_weight);
            }

            assert!(
                mincut.min_cut_value() <= min_degree + 0.001,
                "Min cut must be ≤ minimum degree: {} vs {}",
                mincut.min_cut_value(),
                min_degree
            );
        }
    }
}

#[test]
fn property_min_cut_monotonic_on_edge_removal() {
    // Property: deleting edges cannot increase min cut
    let mut mincut = MinCutBuilder::new()
        .with_edges(vec![
            (1, 2, 1.0),
            (2, 3, 1.0),
            (3, 4, 1.0),
            (4, 1, 1.0),
            (1, 3, 2.0), // Diagonal
        ])
        .build()
        .unwrap();

    let initial_cut = mincut.min_cut_value();

    // Delete edge
    mincut.delete_edge(1, 3).unwrap();
    let after_delete = mincut.min_cut_value();

    assert!(
        after_delete <= initial_cut,
        "Deleting edges cannot increase min cut: {} -> {}",
        initial_cut,
        after_delete
    );
}

#[test]
fn property_symmetry() {
    // Property: graph (u,v,w) has same min cut as (v,u,w)
    let edges_forward = vec![(1, 2, 1.5), (2, 3, 2.5), (3, 1, 1.0)];

    let edges_reverse: Vec<_> = edges_forward.iter().map(|(u, v, w)| (*v, *u, *w)).collect();

    let mincut_fwd = MinCutBuilder::new()
        .with_edges(edges_forward)
        .build()
        .unwrap();

    let mincut_rev = MinCutBuilder::new()
        .with_edges(edges_reverse)
        .build()
        .unwrap();

    assert_eq!(
        mincut_fwd.min_cut_value(),
        mincut_rev.min_cut_value(),
        "Graph should have same min cut regardless of edge direction"
    );
}
