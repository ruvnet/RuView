//! Basic Coherence Example
//!
//! This example demonstrates the core sheaf coherence concepts:
//! - Creating a small sheaf graph with nodes
//! - Adding edges with restriction maps
//! - Computing coherence energy
//! - Comparing coherent vs incoherent scenarios
//!
//! Run with: `cargo run --example basic_coherence`

use prime_radiant::substrate::{SheafEdgeBuilder, SheafGraph, SheafNodeBuilder, StateVector};

fn main() {
    println!("=== Prime-Radiant: Basic Coherence Example ===\n");

    // Example 1: Coherent Sheaf Graph
    // When all nodes have consistent states, energy is low
    println!("--- Example 1: Coherent Graph ---");
    run_coherent_example();

    println!();

    // Example 2: Incoherent Sheaf Graph
    // When nodes have contradictory states, energy is high
    println!("--- Example 2: Incoherent Graph ---");
    run_incoherent_example();

    println!();

    // Example 3: Mixed coherence with different edge weights
    println!("--- Example 3: Weighted Edges ---");
    run_weighted_example();
}

/// Demonstrates a coherent sheaf graph where all nodes agree
fn run_coherent_example() {
    // Create a new sheaf graph
    let graph = SheafGraph::new();

    // Create nodes with similar state vectors
    // In a coherent system, connected nodes should have consistent states
    // that satisfy the restriction map constraints

    // Node A: represents a "fact" with embedding [1.0, 0.5, 0.0, 0.2]
    let node_a = SheafNodeBuilder::new()
        .state(StateVector::new(vec![1.0, 0.5, 0.0, 0.2]))
        .label("fact_a")
        .node_type("assertion")
        .namespace("knowledge")
        .build();
    let id_a = graph.add_node(node_a);

    // Node B: represents a related "fact" with very similar embedding
    let node_b = SheafNodeBuilder::new()
        .state(StateVector::new(vec![1.0, 0.5, 0.0, 0.2])) // Same as A = coherent
        .label("fact_b")
        .node_type("assertion")
        .namespace("knowledge")
        .build();
    let id_b = graph.add_node(node_b);

    // Node C: also consistent with A and B
    let node_c = SheafNodeBuilder::new()
        .state(StateVector::new(vec![1.0, 0.5, 0.0, 0.2])) // Same state
        .label("fact_c")
        .node_type("assertion")
        .namespace("knowledge")
        .build();
    let id_c = graph.add_node(node_c);

    // Add edges with identity restriction maps
    // Identity restriction means: source state should equal target state
    let edge_ab = SheafEdgeBuilder::new(id_a, id_b)
        .identity_restrictions(4) // 4-dimensional identity map
        .weight(1.0)
        .edge_type("semantic")
        .build();
    graph.add_edge(edge_ab).expect("Failed to add edge A->B");

    let edge_bc = SheafEdgeBuilder::new(id_b, id_c)
        .identity_restrictions(4)
        .weight(1.0)
        .edge_type("semantic")
        .build();
    graph.add_edge(edge_bc).expect("Failed to add edge B->C");

    let edge_ca = SheafEdgeBuilder::new(id_c, id_a)
        .identity_restrictions(4)
        .weight(1.0)
        .edge_type("semantic")
        .build();
    graph.add_edge(edge_ca).expect("Failed to add edge C->A");

    // Compute coherence energy
    let energy = graph.compute_energy();

    println!("Graph with 3 coherent nodes and 3 edges:");
    println!("  Nodes: fact_a, fact_b, fact_c (all identical states)");
    println!("  Edges: A<->B, B<->C, C<->A (identity restrictions)");
    println!();
    println!("Coherence Results:");
    println!("  Total Energy: {:.6}", energy.total_energy);
    println!("  Node Count: {}", graph.node_count());
    println!("  Edge Count: {}", energy.edge_count);
    println!();

    // Energy should be 0 or very close to 0 for perfectly coherent system
    if energy.total_energy < 0.01 {
        println!("  Status: COHERENT (energy near zero)");
    } else {
        println!("  Status: Some incoherence detected");
    }
}

/// Demonstrates an incoherent sheaf graph where nodes contradict
fn run_incoherent_example() {
    let graph = SheafGraph::new();

    // Node A: represents one "fact"
    let node_a = SheafNodeBuilder::new()
        .state(StateVector::new(vec![1.0, 0.0, 0.0, 0.0]))
        .label("claim_positive")
        .node_type("assertion")
        .namespace("knowledge")
        .build();
    let id_a = graph.add_node(node_a);

    // Node B: represents a CONTRADICTORY "fact"
    // This embedding is opposite to Node A
    let node_b = SheafNodeBuilder::new()
        .state(StateVector::new(vec![-1.0, 0.0, 0.0, 0.0])) // Opposite!
        .label("claim_negative")
        .node_type("assertion")
        .namespace("knowledge")
        .build();
    let id_b = graph.add_node(node_b);

    // Node C: partially different
    let node_c = SheafNodeBuilder::new()
        .state(StateVector::new(vec![0.0, 1.0, 0.0, 0.0])) // Orthogonal
        .label("claim_other")
        .node_type("assertion")
        .namespace("knowledge")
        .build();
    let id_c = graph.add_node(node_c);

    // Add edges - these constrain that states should be equal
    // But they're NOT equal, so residual energy will be high
    let edge_ab = SheafEdgeBuilder::new(id_a, id_b)
        .identity_restrictions(4)
        .weight(1.0)
        .edge_type("contradiction")
        .build();
    graph.add_edge(edge_ab).expect("Failed to add edge A->B");

    let edge_bc = SheafEdgeBuilder::new(id_b, id_c)
        .identity_restrictions(4)
        .weight(1.0)
        .edge_type("mismatch")
        .build();
    graph.add_edge(edge_bc).expect("Failed to add edge B->C");

    // Compute coherence energy
    let energy = graph.compute_energy();

    println!("Graph with 3 incoherent nodes:");
    println!("  Node A: [1.0, 0.0, 0.0, 0.0] (positive claim)");
    println!("  Node B: [-1.0, 0.0, 0.0, 0.0] (contradictory)");
    println!("  Node C: [0.0, 1.0, 0.0, 0.0] (orthogonal)");
    println!();
    println!("Coherence Results:");
    println!("  Total Energy: {:.6}", energy.total_energy);
    println!("  Node Count: {}", graph.node_count());
    println!("  Edge Count: {}", energy.edge_count);
    println!();

    // Show per-edge energy breakdown
    println!("  Per-Edge Energy:");
    for (edge_id, edge_energy) in &energy.edge_energies {
        println!("    Edge {}: {:.6}", edge_id, edge_energy);
    }
    println!();

    // Energy should be high for incoherent system
    if energy.total_energy > 0.5 {
        println!("  Status: INCOHERENT (high energy indicates contradiction)");
    } else {
        println!("  Status: Mostly coherent");
    }
}

/// Demonstrates how edge weights affect coherence energy
fn run_weighted_example() {
    let graph = SheafGraph::new();

    // Create nodes with different states
    let node_a = SheafNodeBuilder::new()
        .state(StateVector::new(vec![1.0, 0.5, 0.0, 0.0]))
        .label("primary")
        .build();
    let id_a = graph.add_node(node_a);

    let node_b = SheafNodeBuilder::new()
        .state(StateVector::new(vec![0.8, 0.6, 0.1, 0.0])) // Slightly different
        .label("secondary")
        .build();
    let id_b = graph.add_node(node_b);

    let node_c = SheafNodeBuilder::new()
        .state(StateVector::new(vec![0.0, 0.0, 1.0, 0.0])) // Very different
        .label("tertiary")
        .build();
    let id_c = graph.add_node(node_c);

    // Edge A->B: LOW weight (we don't care much if they match)
    let edge_ab = SheafEdgeBuilder::new(id_a, id_b)
        .identity_restrictions(4)
        .weight(0.1) // Low weight
        .edge_type("weak_constraint")
        .build();
    graph.add_edge(edge_ab).expect("Failed to add edge A->B");

    // Edge A->C: HIGH weight (important constraint)
    let edge_ac = SheafEdgeBuilder::new(id_a, id_c)
        .identity_restrictions(4)
        .weight(5.0) // High weight
        .edge_type("strong_constraint")
        .build();
    graph.add_edge(edge_ac).expect("Failed to add edge A->C");

    let energy = graph.compute_energy();

    println!("Graph demonstrating weighted edges:");
    println!("  Node A: [1.0, 0.5, 0.0, 0.0]");
    println!("  Node B: [0.8, 0.6, 0.1, 0.0] (slightly different)");
    println!("  Node C: [0.0, 0.0, 1.0, 0.0] (very different)");
    println!();
    println!("  Edge A->B: weight 0.1 (weak constraint)");
    println!("  Edge A->C: weight 5.0 (strong constraint)");
    println!();
    println!("Coherence Results:");
    println!("  Total Energy: {:.6}", energy.total_energy);
    println!();
    println!("  Per-Edge Energy:");
    for (edge_id, edge_energy) in &energy.edge_energies {
        println!("    Edge {}: {:.6}", edge_id, edge_energy);
    }
    println!();
    println!("  Notice: The high-weight edge contributes much more to total energy,");
    println!("  even though A->B has a smaller residual (state difference).");
}
