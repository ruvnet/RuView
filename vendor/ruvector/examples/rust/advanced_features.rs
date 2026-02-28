//! Example demonstrating advanced features:
//! - Hypergraph structures
//! - Learned indexes
//! - Neural hashing
//! - Topological analysis

use ruvector_core::advanced::*;
use ruvector_core::types::DistanceMetric;

fn main() {
    println!("=== Ruvector Advanced Features Demo ===\n");

    demo_hypergraph();
    demo_temporal_hypergraph();
    demo_causal_memory();
    demo_learned_index();
    demo_neural_hash();
    demo_topological_analysis();
}

fn demo_hypergraph() {
    println!("--- Hypergraph for Multi-Entity Relationships ---");

    let mut index = HypergraphIndex::new(DistanceMetric::Cosine);

    // Scenario: Academic paper citation network
    // Entities: papers (represented by embeddings)
    println!("Adding papers as entities...");
    index.add_entity(1, vec![0.9, 0.1, 0.0]); // ML paper
    index.add_entity(2, vec![0.8, 0.2, 0.0]); // Similar ML paper
    index.add_entity(3, vec![0.1, 0.9, 0.0]); // NLP paper
    index.add_entity(4, vec![0.0, 0.8, 0.2]); // Similar NLP paper
    index.add_entity(5, vec![0.4, 0.4, 0.2]); // Cross-domain paper

    // Hyperedge: Papers 1, 2, 5 co-cited in review
    let edge1 = Hyperedge::new(
        vec![1, 2, 5],
        "Co-cited in ML review paper".to_string(),
        vec![0.7, 0.2, 0.1],
        0.95,
    );
    index.add_hyperedge(edge1).unwrap();

    // Hyperedge: Papers 3, 4, 5 form research thread
    let edge2 = Hyperedge::new(
        vec![3, 4, 5],
        "NLP research thread".to_string(),
        vec![0.2, 0.7, 0.1],
        0.90,
    );
    index.add_hyperedge(edge2).unwrap();

    println!("Added 2 hyperedges connecting papers");

    // Search for relationships similar to a query
    let query = vec![0.6, 0.3, 0.1]; // ML-focused query
    let results = index.search_hyperedges(&query, 2);

    println!("Searching for relationships similar to ML query:");
    for (edge_id, distance) in results {
        if let Some(edge) = index.get_hyperedge(&edge_id) {
            println!(
                "  - {} (distance: {:.3}, nodes: {:?})",
                edge.description, distance, edge.nodes
            );
        }
    }

    // Find k-hop neighbors
    let neighbors = index.k_hop_neighbors(1, 2);
    println!("Papers reachable from paper 1 (2 hops): {:?}", neighbors);

    let stats = index.stats();
    println!("Stats: {} entities, {} hyperedges, avg degree: {:.2}\n",
        stats.total_entities, stats.total_hyperedges, stats.avg_entity_degree);
}

fn demo_temporal_hypergraph() {
    println!("--- Temporal Hypergraph for Time-Series Relationships ---");

    let mut index = HypergraphIndex::new(DistanceMetric::Euclidean);

    // Scenario: User interaction patterns over time
    println!("Tracking user interactions...");

    index.add_entity(1, vec![1.0, 0.0]); // User A
    index.add_entity(2, vec![0.0, 1.0]); // User B
    index.add_entity(3, vec![0.5, 0.5]); // User C

    // Add temporal interactions
    let edge1 = Hyperedge::new(
        vec![1, 2],
        "Users A and B collaborated".to_string(),
        vec![0.5, 0.5],
        1.0,
    );
    let temporal1 = TemporalHyperedge::new(edge1, TemporalGranularity::Daily);
    index.add_temporal_hyperedge(temporal1.clone()).unwrap();

    let edge2 = Hyperedge::new(
        vec![2, 3],
        "Users B and C interacted".to_string(),
        vec![0.3, 0.7],
        0.8,
    );
    let temporal2 = TemporalHyperedge::new(edge2, TemporalGranularity::Daily);
    index.add_temporal_hyperedge(temporal2.clone()).unwrap();

    println!("Added temporal interactions");

    // Query by time bucket
    let bucket = temporal1.time_bucket();
    let results = index.query_temporal_range(bucket, bucket + 1);
    println!("Interactions in time bucket {}: {} found\n", bucket, results.len());
}

fn demo_causal_memory() {
    println!("--- Causal Hypergraph Memory for Agent Reasoning ---");

    let mut memory = CausalMemory::new(DistanceMetric::Cosine)
        .with_weights(0.7, 0.2, 0.1); // α=0.7 (similarity), β=0.2 (causal), γ=0.1 (latency)

    // Scenario: Agent learning from experience
    println!("Building causal memory from agent experiences...");

    // States/actions as embeddings
    memory.index().add_entity(1, vec![1.0, 0.0, 0.0]); // Action: fetch_data
    memory.index().add_entity(2, vec![0.0, 1.0, 0.0]); // Effect: success
    memory.index().add_entity(3, vec![0.0, 0.0, 1.0]); // Context: morning

    // Record successful causal relationship
    memory.add_causal_edge(
        1, // cause: fetch_data
        2, // effect: success
        vec![3], // context: morning
        "Fetching data in morning leads to success".to_string(),
        vec![0.5, 0.4, 0.1],
        50.0, // 50ms latency
    ).unwrap();

    // Record it again to increase causal strength
    memory.add_causal_edge(
        1, 2, vec![3],
        "Repeated success".to_string(),
        vec![0.5, 0.4, 0.1],
        45.0,
    ).unwrap();

    println!("Recorded causal relationships");

    // Query: What actions should agent take in a similar situation?
    let query = vec![0.6, 0.3, 0.1]; // Similar to morning fetch scenario
    let results = memory.query_with_utility(&query, 1, 3);

    println!("Querying causal memory for similar situation:");
    for (edge_id, utility) in results {
        if let Some(edge) = memory.index().get_hyperedge(&edge_id) {
            println!("  - {} (utility: {:.3})", edge.description, utility);
        }
    }
    println!("Utility = 0.7*similarity + 0.2*causal_uplift - 0.1*latency\n");
}

fn demo_learned_index() {
    println!("--- Recursive Model Index (RMI) ---");

    let mut rmi = RecursiveModelIndex::new(2, 4);

    // Generate data: points on a curve
    println!("Building learned index from 1000 data points...");
    let data: Vec<(Vec<f32>, u64)> = (0..1000)
        .map(|i| {
            let x = (i as f32) / 1000.0;
            let y = x * x; // Parabola
            (vec![x, y], i as u64)
        })
        .collect();

    rmi.build(data).unwrap();

    // Test predictions
    println!("Testing predictions:");
    let test_points = vec![
        (vec![0.25, 0.0625], "Point on curve"),
        (vec![0.5, 0.25], "Mid point"),
        (vec![0.75, 0.5625], "Upper point"),
    ];

    for (point, desc) in test_points {
        let predicted_pos = rmi.predict(&point).unwrap();
        let actual_idx = (point[0] * 1000.0) as usize;
        let error = (predicted_pos as i32 - actual_idx as i32).abs();
        println!("  {} - Predicted: {}, Actual: {}, Error: {}",
            desc, predicted_pos, actual_idx, error);
    }

    let stats = rmi.stats();
    println!("RMI Stats:");
    println!("  Total entries: {}", stats.total_entries);
    println!("  Model size: {} bytes", stats.model_size_bytes);
    println!("  Average error: {:.2}", stats.avg_error);
    println!("  Max error: {}\n", stats.max_error);
}

fn demo_neural_hash() {
    println!("--- Neural Hash Functions for Compression ---");

    // Using LSH for simplicity
    let lsh = SimpleLSH::new(128, 32);
    let mut index = HashIndex::new(lsh, 32);

    println!("Creating hash index (128D -> 32 bits)...");

    // Insert random high-dimensional vectors
    use rand::Rng;
    let mut rng = rand::thread_rng();

    for i in 0..100 {
        let vec: Vec<f32> = (0..128).map(|_| rng.gen::<f32>()).collect();
        index.insert(i, vec);
    }

    println!("Inserted 100 vectors");

    // Search with a query
    let query: Vec<f32> = (0..128).map(|_| rng.gen::<f32>()).collect();
    let results = index.search(&query, 5, 8); // Max Hamming distance: 8

    println!("Search results (top 5):");
    for (id, similarity) in results.iter().take(5) {
        println!("  Vector {} - Similarity: {:.3}", id, similarity);
    }

    let stats = index.stats();
    println!("Hash Index Stats:");
    println!("  Total vectors: {}", stats.total_vectors);
    println!("  Buckets: {}", stats.num_buckets);
    println!("  Avg bucket size: {:.2}", stats.avg_bucket_size);
    println!("  Compression ratio: {:.1}x\n", stats.compression_ratio);
}

fn demo_topological_analysis() {
    println!("--- Topological Data Analysis for Embedding Quality ---");

    let analyzer = TopologicalAnalyzer::new(5, 10.0);

    // Create embeddings with known quality issues
    println!("Analyzing three embedding sets:\n");

    // 1. Good embeddings: well-separated clusters
    println!("1. Good embeddings (two clusters):");
    let mut good_embeddings = Vec::new();
    for i in 0..30 {
        let angle = (i as f32) * 2.0 * std::f32::consts::PI / 30.0;
        good_embeddings.push(vec![angle.cos(), angle.sin()]);
    }
    for i in 0..30 {
        let angle = (i as f32) * 2.0 * std::f32::consts::PI / 30.0;
        good_embeddings.push(vec![5.0 + angle.cos(), 5.0 + angle.sin()]);
    }

    let quality1 = analyzer.analyze(&good_embeddings).unwrap();
    print_quality_report(&quality1);

    // 2. Mode collapsed embeddings
    println!("\n2. Mode collapsed embeddings:");
    let collapsed: Vec<Vec<f32>> = (0..60)
        .map(|i| vec![1.0 + (i as f32) * 0.01, 1.0 + (i as f32) * 0.01])
        .collect();

    let quality2 = analyzer.analyze(&collapsed).unwrap();
    print_quality_report(&quality2);

    // 3. Degenerate embeddings (stuck in 1D)
    println!("\n3. Degenerate embeddings (1D manifold in 2D space):");
    let degenerate: Vec<Vec<f32>> = (0..60)
        .map(|i| {
            let x = (i as f32) / 60.0;
            vec![x, 0.0] // All on x-axis
        })
        .collect();

    let quality3 = analyzer.analyze(&degenerate).unwrap();
    print_quality_report(&quality3);
}

fn print_quality_report(quality: &EmbeddingQuality) {
    println!("  Dimensions: {}", quality.dimensions);
    println!("  Vectors: {}", quality.num_vectors);
    println!("  Connected components: {}", quality.connected_components);
    println!("  Clustering coefficient: {:.3}", quality.clustering_coefficient);
    println!("  Mode collapse score: {:.3} (0=collapsed, 1=good)", quality.mode_collapse_score);
    println!("  Degeneracy score: {:.3} (0=full rank, 1=degenerate)", quality.degeneracy_score);
    println!("  Overall quality: {:.3}", quality.quality_score);
    println!("  Assessment: {}", quality.assessment());

    if quality.has_mode_collapse() {
        println!("  ⚠️  WARNING: Mode collapse detected!");
    }
    if quality.is_degenerate() {
        println!("  ⚠️  WARNING: Embeddings are degenerate!");
    }
}
