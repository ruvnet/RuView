//! Demonstration of graph sparsification for approximate minimum cuts

use ruvector_mincut::graph::DynamicGraph;
use ruvector_mincut::sparsify::{karger_sparsify, NagamochiIbaraki, SparseGraph, SparsifyConfig};
use std::sync::Arc;

fn main() {
    println!("=== Graph Sparsification Demo ===\n");

    // Create a sample graph (complete graph on 10 vertices)
    println!("Creating complete graph with 10 vertices...");
    let graph = create_complete_graph(10);
    println!(
        "Original graph: {} vertices, {} edges\n",
        graph.num_vertices(),
        graph.num_edges()
    );

    // Demo 1: Benczúr-Karger sparsification
    println!("--- Benczúr-Karger Sparsification ---");
    demo_benczur_karger(&graph);

    // Demo 2: Karger sparsification (convenience function)
    println!("\n--- Karger Sparsification (convenience) ---");
    demo_karger(&graph);

    // Demo 3: Nagamochi-Ibaraki deterministic sparsification
    println!("\n--- Nagamochi-Ibaraki Deterministic Sparsification ---");
    demo_nagamochi_ibaraki(&graph);

    println!("\n=== Demo Complete ===");
}

fn create_complete_graph(n: usize) -> DynamicGraph {
    let g = DynamicGraph::new();
    for i in 0..n {
        for j in (i + 1)..n {
            g.insert_edge(i as u64, j as u64, 1.0).unwrap();
        }
    }
    g
}

fn demo_benczur_karger(graph: &DynamicGraph) {
    let epsilons = vec![0.1, 0.2, 0.5];

    for epsilon in epsilons {
        let config = SparsifyConfig::new(epsilon).unwrap().with_seed(42);

        let sparse = SparseGraph::from_graph(graph, config).unwrap();

        println!(
            "  ε = {:.2}: {} edges ({:.1}% of original)",
            epsilon,
            sparse.num_edges(),
            sparse.sparsification_ratio() * 100.0
        );

        let approx_cut = sparse.approximate_min_cut();
        println!("    Approximate min cut: {:.2}", approx_cut);
    }
}

fn demo_karger(graph: &DynamicGraph) {
    let epsilon = 0.15;
    let sparse = karger_sparsify(graph, epsilon, Some(123)).unwrap();

    println!(
        "  ε = {:.2}: {} edges ({:.1}% of original)",
        epsilon,
        sparse.num_edges(),
        sparse.sparsification_ratio() * 100.0
    );
}

fn demo_nagamochi_ibaraki(graph: &DynamicGraph) {
    let ni = NagamochiIbaraki::new(Arc::new(graph.clone()));

    let k_values = vec![2, 3, 5];

    for k in k_values {
        match ni.sparse_k_certificate(k) {
            Ok(sparse) => {
                let ratio = sparse.num_edges() as f64 / graph.num_edges() as f64;
                println!(
                    "  k = {}: {} edges ({:.1}% of original)",
                    k,
                    sparse.num_edges(),
                    ratio * 100.0
                );
            }
            Err(e) => {
                println!("  k = {}: Error - {}", k, e);
            }
        }
    }
}
