//! LocalKCut Algorithm Demonstration
//!
//! This example demonstrates the deterministic LocalKCut algorithm from the
//! December 2024 paper. It shows how to:
//!
//! 1. Find local minimum cuts near specific vertices
//! 2. Use deterministic edge colorings for reproducibility
//! 3. Apply forest packing for witness guarantees
//! 4. Compare with global minimum cut algorithms

use ruvector_mincut::prelude::*;
use std::sync::Arc;

fn main() {
    println!("=== LocalKCut Algorithm Demonstration ===\n");

    // Example 1: Simple graph with bridge
    println!("Example 1: Bridge Detection");
    demo_bridge_detection();
    println!();

    // Example 2: Deterministic behavior
    println!("Example 2: Deterministic Coloring");
    demo_deterministic_coloring();
    println!();

    // Example 3: Forest packing
    println!("Example 3: Forest Packing Witnesses");
    demo_forest_packing();
    println!();

    // Example 4: Comparison with global mincut
    println!("Example 4: Local vs Global Minimum Cut");
    demo_local_vs_global();
    println!();

    // Example 5: Complex graph
    println!("Example 5: Complex Graph Analysis");
    demo_complex_graph();
}

/// Demonstrates finding a bridge using LocalKCut
fn demo_bridge_detection() {
    let graph = Arc::new(DynamicGraph::new());

    // Create two components connected by a bridge
    // Component 1: triangle {1, 2, 3}
    graph.insert_edge(1, 2, 1.0).unwrap();
    graph.insert_edge(2, 3, 1.0).unwrap();
    graph.insert_edge(3, 1, 1.0).unwrap();

    // Bridge: 3 -> 4 (the minimum cut!)
    graph.insert_edge(3, 4, 1.0).unwrap();

    // Component 2: triangle {4, 5, 6}
    graph.insert_edge(4, 5, 1.0).unwrap();
    graph.insert_edge(5, 6, 1.0).unwrap();
    graph.insert_edge(6, 4, 1.0).unwrap();

    println!("Graph: Two triangles connected by a bridge");
    println!(
        "Vertices: {}, Edges: {}",
        graph.num_vertices(),
        graph.num_edges()
    );

    // Find local cut from vertex 1
    let local_kcut = LocalKCut::new(graph.clone(), 5);

    println!("\nSearching for local cut from vertex 1 with k=5...");
    if let Some(result) = local_kcut.find_cut(1) {
        println!("✓ Found local cut!");
        println!("  Cut value: {}", result.cut_value);
        println!("  Cut set size: {}", result.cut_set.len());
        println!("  Cut edges: {:?}", result.cut_edges);
        println!("  Iterations: {}", result.iterations);

        // The bridge should be found
        if result.cut_value == 1.0 {
            println!("  → Successfully detected the bridge!");
        }
    } else {
        println!("✗ No cut found within bound k=5");
    }
}

/// Demonstrates deterministic coloring behavior
fn demo_deterministic_coloring() {
    let graph = Arc::new(DynamicGraph::new());

    // Create a simple path graph: 1-2-3-4-5
    for i in 1..=4 {
        graph.insert_edge(i, i + 1, 1.0).unwrap();
    }

    println!("Graph: Path of 5 vertices");

    // Create two LocalKCut instances
    let lk1 = LocalKCut::new(graph.clone(), 3);
    let lk2 = LocalKCut::new(graph.clone(), 3);

    println!("\nEdge colorings (deterministic):");
    for edge in graph.edges() {
        let color1 = lk1.edge_color(edge.id).unwrap();
        let color2 = lk2.edge_color(edge.id).unwrap();

        println!("  Edge ({}, {}): {:?}", edge.source, edge.target, color1);

        // Verify determinism
        assert_eq!(color1, color2, "Colors should be deterministic!");
    }

    println!("\n✓ All edge colorings are deterministic!");

    // Find cuts from different vertices
    println!("\nFinding cuts from different starting vertices:");
    for start_vertex in 1..=5 {
        if let Some(result) = lk1.find_cut(start_vertex) {
            println!(
                "  Vertex {}: cut value = {}, set size = {}",
                start_vertex,
                result.cut_value,
                result.cut_set.len()
            );
        }
    }
}

/// Demonstrates forest packing and witness properties
fn demo_forest_packing() {
    let graph = Arc::new(DynamicGraph::new());

    // Create a more complex graph
    //     1 - 2
    //     |   |
    //     3 - 4 - 5
    //         |   |
    //         6 - 7

    graph.insert_edge(1, 2, 1.0).unwrap();
    graph.insert_edge(1, 3, 1.0).unwrap();
    graph.insert_edge(2, 4, 1.0).unwrap();
    graph.insert_edge(3, 4, 1.0).unwrap();
    graph.insert_edge(4, 5, 1.0).unwrap();
    graph.insert_edge(4, 6, 1.0).unwrap();
    graph.insert_edge(5, 7, 1.0).unwrap();
    graph.insert_edge(6, 7, 1.0).unwrap();

    println!("Graph: Complex grid-like structure");
    println!(
        "Vertices: {}, Edges: {}",
        graph.num_vertices(),
        graph.num_edges()
    );

    // Create forest packing
    let lambda_max = 3; // Upper bound on min cut
    let epsilon = 0.1; // Approximation parameter

    println!(
        "\nCreating forest packing with λ_max={}, ε={}...",
        lambda_max, epsilon
    );
    let packing = ForestPacking::greedy_packing(&*graph, lambda_max, epsilon);

    println!("✓ Created {} forests", packing.num_forests());

    // Show forest structures
    for i in 0..packing.num_forests().min(3) {
        if let Some(forest) = packing.forest(i) {
            println!("  Forest {}: {} edges", i, forest.len());
        }
    }

    // Find a cut and check witness property
    let local_kcut = LocalKCut::new(graph.clone(), 5);
    if let Some(result) = local_kcut.find_cut(1) {
        println!("\nFound cut with value {}", result.cut_value);

        let is_witnessed = packing.witnesses_cut(&result.cut_edges);
        println!("  Witnessed by all forests: {}", is_witnessed);

        if is_witnessed {
            println!("  ✓ Cut satisfies witness property!");
        }
    }
}

/// Compare local and global minimum cuts
fn demo_local_vs_global() {
    let graph = Arc::new(DynamicGraph::new());

    // Create a graph where local and global cuts differ
    //     1 - 2 - 3
    //     |   |   |
    //     4 - 5 - 6
    //     |   |   |
    //     7 - 8 - 9

    // Top row
    graph.insert_edge(1, 2, 2.0).unwrap();
    graph.insert_edge(2, 3, 2.0).unwrap();

    // Middle row
    graph.insert_edge(4, 5, 2.0).unwrap();
    graph.insert_edge(5, 6, 2.0).unwrap();

    // Bottom row
    graph.insert_edge(7, 8, 2.0).unwrap();
    graph.insert_edge(8, 9, 2.0).unwrap();

    // Vertical connections (weaker)
    graph.insert_edge(1, 4, 1.0).unwrap();
    graph.insert_edge(2, 5, 1.0).unwrap();
    graph.insert_edge(3, 6, 1.0).unwrap();
    graph.insert_edge(4, 7, 1.0).unwrap();
    graph.insert_edge(5, 8, 1.0).unwrap();
    graph.insert_edge(6, 9, 1.0).unwrap();

    println!("Graph: 3x3 grid with different edge weights");
    println!(
        "Vertices: {}, Edges: {}",
        graph.num_vertices(),
        graph.num_edges()
    );

    // Find local cuts from different vertices
    let local_kcut = LocalKCut::new(graph.clone(), 10);

    println!("\nLocal cuts from different vertices:");
    for vertex in &[1, 5, 9] {
        if let Some(result) = local_kcut.find_cut(*vertex) {
            println!(
                "  Vertex {}: cut value = {}, iterations = {}",
                vertex, result.cut_value, result.iterations
            );
        }
    }

    // Build global minimum cut (using the algorithm)
    let mut mincut = MinCutBuilder::new().exact().build().unwrap();

    // Add edges to global mincut
    for edge in graph.edges() {
        let _ = mincut.insert_edge(edge.source, edge.target, edge.weight);
    }

    let global_value = mincut.min_cut_value();
    println!("\nGlobal minimum cut value: {}", global_value);

    println!("\n✓ Local cuts provide fast approximations to global minimum cut");
}

/// Analyze a complex graph with multiple cut candidates
fn demo_complex_graph() {
    let graph = Arc::new(DynamicGraph::new());

    // Create a graph with multiple communities
    // Community 1: clique {1,2,3,4}
    for i in 1..=4 {
        for j in i + 1..=4 {
            graph.insert_edge(i, j, 2.0).unwrap();
        }
    }

    // Community 2: clique {5,6,7,8}
    for i in 5..=8 {
        for j in i + 1..=8 {
            graph.insert_edge(i, j, 2.0).unwrap();
        }
    }

    // Weak connections between communities
    graph.insert_edge(4, 5, 0.5).unwrap();
    graph.insert_edge(3, 6, 0.5).unwrap();

    println!("Graph: Two dense communities with weak connections");
    println!(
        "Vertices: {}, Edges: {}",
        graph.num_vertices(),
        graph.num_edges()
    );

    let stats = graph.stats();
    println!("Average degree: {:.2}", stats.avg_degree);
    println!("Total weight: {:.2}", stats.total_weight);

    // Find local cuts
    let local_kcut = LocalKCut::new(graph.clone(), 5);

    println!("\nSearching for cuts with k=5...");

    // Try from community 1
    if let Some(result) = local_kcut.find_cut(1) {
        println!("  From community 1:");
        println!("    Cut value: {}", result.cut_value);
        println!(
            "    Separates {} vertices from {}",
            result.cut_set.len(),
            graph.num_vertices() - result.cut_set.len()
        );
    }

    // Try from community 2
    if let Some(result) = local_kcut.find_cut(5) {
        println!("  From community 2:");
        println!("    Cut value: {}", result.cut_value);
        println!(
            "    Separates {} vertices from {}",
            result.cut_set.len(),
            graph.num_vertices() - result.cut_set.len()
        );
    }

    // Enumerate paths to understand graph structure
    println!("\nPath enumeration from vertex 1:");
    let paths = local_kcut.enumerate_paths(1, 2);
    println!("  Found {} distinct reachable sets at depth 2", paths.len());

    // Show diversity of reachable sets
    let mut sizes: Vec<_> = paths.iter().map(|p| p.len()).collect();
    sizes.sort_unstable();
    sizes.dedup();
    println!("  Reachable set sizes: {:?}", sizes);

    println!("\n✓ LocalKCut successfully analyzes community structure");
}
