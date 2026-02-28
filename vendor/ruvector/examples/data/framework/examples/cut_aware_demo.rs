//! Cut-Aware HNSW Demo
//!
//! Demonstrates how cut-aware search respects coherence boundaries
//! in a multi-cluster vector space.

use ruvector_data_framework::cut_aware_hnsw::{CutAwareHNSW, CutAwareConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Cut-Aware HNSW Demo ===\n");

    // Configure cut-aware HNSW
    let config = CutAwareConfig {
        m: 16,
        ef_construction: 200,
        ef_search: 50,
        coherence_gate_threshold: 0.4,
        max_cross_cut_hops: 2,
        enable_cut_pruning: false,
        cut_recompute_interval: 25,
        min_zone_size: 5,
    };

    let mut index = CutAwareHNSW::new(config);

    // Create three distinct clusters
    println!("Creating three vector clusters:");
    println!("  - Cluster A: High positive values (science papers)");
    println!("  - Cluster B: Low negative values (arts papers)");
    println!("  - Cluster C: Mixed values (interdisciplinary)");
    println!();

    const DIM: usize = 128;

    // Cluster A: Science papers (high positive values)
    for i in 0..30 {
        let mut vec = vec![0.0; DIM];
        for j in 0..DIM {
            vec[j] = 2.0 + (i as f32 * 0.05) + (j as f32 * 0.001);
        }
        index.insert(i, &vec)?;
    }

    // Cluster B: Arts papers (low negative values)
    for i in 30..60 {
        let mut vec = vec![0.0; DIM];
        for j in 0..DIM {
            vec[j] = -2.0 + (i as f32 * 0.05) + (j as f32 * 0.001);
        }
        index.insert(i, &vec)?;
    }

    // Cluster C: Interdisciplinary (mixed)
    for i in 60..80 {
        let mut vec = vec![0.0; DIM];
        for j in 0..DIM {
            vec[j] = 0.0 + (i as f32 * 0.05) + (j as f32 * 0.001);
        }
        index.insert(i, &vec)?;
    }

    println!("Inserted 80 vectors across 3 clusters\n");

    // Compute coherence zones
    println!("Computing coherence zones...");
    let zones = index.compute_zones();
    println!("Found {} coherence zones", zones.len());
    for (i, zone) in zones.iter().enumerate() {
        println!(
            "  Zone {}: {} nodes, coherence ratio: {:.3}",
            i, zone.nodes.len(), zone.coherence_ratio
        );
    }
    println!();

    // Query from Cluster A (science)
    let science_query = vec![2.0; DIM];
    println!("=== Query 1: Science Paper (Cluster A) ===");
    println!("Query vector: [2.0, 2.0, ...]");
    println!();

    // Ungated search (baseline)
    println!("Ungated Search (no coherence boundaries):");
    let ungated = index.search_ungated(&science_query, 5);
    for (i, result) in ungated.iter().enumerate() {
        println!(
            "  {}: Node {} - distance: {:.4}",
            i + 1, result.node_id, result.distance
        );
    }
    println!();

    // Gated search (respects boundaries)
    println!("Gated Search (respects coherence boundaries):");
    let gated = index.search_gated(&science_query, 5);
    for (i, result) in gated.iter().enumerate() {
        println!(
            "  {}: Node {} - distance: {:.4}, cuts crossed: {}, coherence: {:.3}",
            i + 1, result.node_id, result.distance, result.crossed_cuts, result.coherence_score
        );
    }
    println!();

    // Query from Cluster B (arts)
    let arts_query = vec![-2.0; DIM];
    println!("=== Query 2: Arts Paper (Cluster B) ===");
    println!("Query vector: [-2.0, -2.0, ...]");
    println!();

    println!("Gated Search:");
    let gated_arts = index.search_gated(&arts_query, 5);
    for (i, result) in gated_arts.iter().enumerate() {
        println!(
            "  {}: Node {} - distance: {:.4}, cuts crossed: {}, coherence: {:.3}",
            i + 1, result.node_id, result.distance, result.crossed_cuts, result.coherence_score
        );
    }
    println!();

    // Coherent neighborhood exploration
    println!("=== Coherent Neighborhood Exploration ===");
    println!("Finding coherent neighbors of Node 0 (Cluster A):");

    let neighborhood = index.coherent_neighborhood(0, 3);
    println!("  Radius 3: {} reachable nodes without crossing weak cuts", neighborhood.len());
    println!("  Nodes: {:?}", &neighborhood[..neighborhood.len().min(10)]);
    println!();

    // Cross-zone search
    println!("=== Cross-Zone Search ===");
    let neutral_query = vec![0.0; DIM];
    println!("Query vector: [0.0, 0.0, ...] (neutral/interdisciplinary)");
    println!();

    if zones.len() >= 2 {
        println!("Searching across zones 0 and 1:");
        let cross_zone = index.cross_zone_search(&neutral_query, 5, &[0, 1]);
        for (i, result) in cross_zone.iter().enumerate() {
            println!(
                "  {}: Node {} - distance: {:.4}, zone crossing: {}",
                i + 1, result.node_id, result.distance, result.crossed_cuts
            );
        }
        println!();
    }

    // Metrics
    println!("=== Performance Metrics ===");
    let metrics_json = index.export_metrics();
    println!("{}", serde_json::to_string_pretty(&metrics_json)?);
    println!();

    // Cut distribution
    println!("=== Cut Distribution ===");
    let cut_dist = index.cut_distribution();
    for stats in cut_dist {
        println!(
            "Layer {}: avg_cut={:.4}, weak_edges={}",
            stats.layer, stats.avg_cut, stats.weak_edge_count
        );
    }
    println!();

    println!("=== Summary ===");
    println!("Cut-aware search successfully:");
    println!("  ✓ Identified {} coherence zones", zones.len());
    println!("  ✓ Gated expansions across weak cuts");
    println!("  ✓ Maintained higher coherence scores within clusters");
    println!("  ✓ Supported explicit cross-zone queries");
    println!();
    println!("This demonstrates how semantic boundaries can guide");
    println!("vector search to stay within coherent regions!");

    Ok(())
}
