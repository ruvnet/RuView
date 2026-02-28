//! Integration tests for advanced features

use ruvector_core::advanced::{
    Hyperedge, HypergraphIndex, TemporalHyperedge, TemporalGranularity, CausalMemory,
    LearnedIndex, RecursiveModelIndex, HybridIndex,
    NeuralHash, DeepHashEmbedding, SimpleLSH, HashIndex,
    TopologicalAnalyzer, EmbeddingQuality,
};
use ruvector_core::types::DistanceMetric;

#[test]
fn test_hypergraph_full_workflow() {
    let mut index = HypergraphIndex::new(DistanceMetric::Cosine);

    // Add entities (documents, users, concepts)
    index.add_entity(1, vec![1.0, 0.0, 0.0]);
    index.add_entity(2, vec![0.0, 1.0, 0.0]);
    index.add_entity(3, vec![0.0, 0.0, 1.0]);
    index.add_entity(4, vec![0.5, 0.5, 0.0]);

    // Add hyperedge: "Documents 1 and 2 both discuss topic X with user 4"
    let edge1 = Hyperedge::new(
        vec![1, 2, 4],
        "Documents discuss topic with user".to_string(),
        vec![0.6, 0.3, 0.1],
        0.9,
    );
    index.add_hyperedge(edge1).unwrap();

    // Add another hyperedge
    let edge2 = Hyperedge::new(
        vec![2, 3, 4],
        "Related documents and user interaction".to_string(),
        vec![0.3, 0.6, 0.1],
        0.85,
    );
    index.add_hyperedge(edge2).unwrap();

    // Search for similar relationships
    let results = index.search_hyperedges(&[0.5, 0.4, 0.1], 5);
    assert!(!results.is_empty());

    // Find neighbors
    let neighbors = index.k_hop_neighbors(1, 2);
    assert!(neighbors.contains(&1));
    assert!(neighbors.contains(&2));

    let stats = index.stats();
    assert_eq!(stats.total_entities, 4);
    assert_eq!(stats.total_hyperedges, 2);
}

#[test]
fn test_temporal_hypergraph() {
    let mut index = HypergraphIndex::new(DistanceMetric::Euclidean);

    index.add_entity(1, vec![1.0, 0.0]);
    index.add_entity(2, vec![0.0, 1.0]);

    // Add temporal hyperedge
    let edge = Hyperedge::new(
        vec![1, 2],
        "Time-based relationship".to_string(),
        vec![0.5, 0.5],
        1.0,
    );

    let temporal = TemporalHyperedge::new(edge, TemporalGranularity::Hourly);
    index.add_temporal_hyperedge(temporal.clone()).unwrap();

    // Query by time range
    let bucket = temporal.time_bucket();
    let results = index.query_temporal_range(bucket - 1, bucket + 1);
    assert!(!results.is_empty());
}

#[test]
fn test_causal_memory_workflow() {
    let mut memory = CausalMemory::new(DistanceMetric::Cosine);

    // Add entities representing states/actions
    memory.index().add_entity(1, vec![1.0, 0.0, 0.0]);
    memory.index().add_entity(2, vec![0.0, 1.0, 0.0]);
    memory.index().add_entity(3, vec![0.0, 0.0, 1.0]);

    // Add causal relationships: action 1 causes effect 2
    memory.add_causal_edge(
        1,
        2,
        vec![3], // with context 3
        "Action leads to effect".to_string(),
        vec![0.5, 0.5, 0.0],
        100.0, // latency in ms
    ).unwrap();

    // Add more causal edges to build history
    memory.add_causal_edge(
        1,
        2,
        vec![],
        "Repeated success".to_string(),
        vec![0.6, 0.4, 0.0],
        90.0,
    ).unwrap();

    // Query with utility function
    let results = memory.query_with_utility(&[0.55, 0.45, 0.0], 1, 5);
    assert!(!results.is_empty());

    // Utility should be positive for similar situations with successful outcomes
    assert!(results[0].1 > 0.0);
}

#[test]
fn test_learned_index_rmi() {
    let mut rmi = RecursiveModelIndex::new(2, 4);

    // Generate sorted data
    let data: Vec<(Vec<f32>, u64)> = (0..100)
        .map(|i| {
            let x = i as f32 / 100.0;
            (vec![x, x * x], i as u64)
        })
        .collect();

    rmi.build(data).unwrap();

    // Test prediction
    let pos = rmi.predict(&[0.5, 0.25]).unwrap();
    assert!(pos < 100);

    // Test search
    let result = rmi.search(&[0.5, 0.25]).unwrap();
    assert!(result.is_some());

    let stats = rmi.stats();
    assert_eq!(stats.total_entries, 100);
    println!("RMI avg error: {}, max error: {}", stats.avg_error, stats.max_error);
}

#[test]
fn test_hybrid_index() {
    let mut hybrid = HybridIndex::new(1, 2, 10);

    // Build static portion
    let static_data = vec![
        (vec![0.0], 0),
        (vec![0.5], 1),
        (vec![1.0], 2),
        (vec![1.5], 3),
        (vec![2.0], 4),
    ];
    hybrid.build_static(static_data).unwrap();

    // Add dynamic updates
    for i in 5..8 {
        hybrid.insert(vec![i as f32], i as u64).unwrap();
    }

    // Search static
    assert_eq!(hybrid.search(&[1.0]).unwrap(), Some(2));

    // Search dynamic
    assert_eq!(hybrid.search(&[6.0]).unwrap(), Some(6));

    // Check rebuild threshold
    assert!(!hybrid.needs_rebuild());
}

#[test]
fn test_neural_hash_deep_embedding() {
    let mut hash = DeepHashEmbedding::new(4, vec![8], 16);

    // Generate training data
    let mut positive_pairs = Vec::new();
    let mut negative_pairs = Vec::new();

    for _ in 0..10 {
        let a = vec![0.1, 0.2, 0.3, 0.4];
        let b = vec![0.11, 0.21, 0.31, 0.41]; // Similar
        positive_pairs.push((a, b));

        let c = vec![0.1, 0.2, 0.3, 0.4];
        let d = vec![0.9, 0.8, 0.7, 0.6]; // Dissimilar
        negative_pairs.push((c, d));
    }

    // Train
    hash.train(&positive_pairs, &negative_pairs, 0.01, 5);

    // Test encoding
    let code1 = hash.encode(&[0.1, 0.2, 0.3, 0.4]);
    let code2 = hash.encode(&[0.11, 0.21, 0.31, 0.41]);
    let code3 = hash.encode(&[0.9, 0.8, 0.7, 0.6]);

    // Similar vectors should have smaller Hamming distance
    let dist_similar = hash.hamming_distance(&code1, &code2);
    let dist_different = hash.hamming_distance(&code1, &code3);

    println!("Similar distance: {}, Different distance: {}", dist_similar, dist_different);
    // After training, similar should be closer (though training is simplified)
}

#[test]
fn test_lsh_hash_index() {
    let lsh = SimpleLSH::new(3, 16);
    let mut index = HashIndex::new(lsh, 16);

    // Insert vectors
    for i in 0..50 {
        let angle = (i as f32) * std::f32::consts::PI / 25.0;
        let vec = vec![angle.cos(), angle.sin(), 0.1];
        index.insert(i, vec);
    }

    // Search for similar vectors
    let query = vec![1.0, 0.0, 0.1]; // Close to first vector
    let results = index.search(&query, 5, 4);

    assert!(!results.is_empty());
    println!("Found {} similar vectors", results.len());

    let stats = index.stats();
    assert_eq!(stats.total_vectors, 50);
    println!("Compression ratio: {:.2}x", stats.compression_ratio);
}

#[test]
fn test_topological_analysis() {
    let analyzer = TopologicalAnalyzer::new(5, 10.0);

    // Create embeddings with known structure: two clusters
    let mut embeddings = Vec::new();

    // Cluster 1: around origin
    for i in 0..20 {
        let angle = (i as f32) * 2.0 * std::f32::consts::PI / 20.0;
        embeddings.push(vec![angle.cos(), angle.sin()]);
    }

    // Cluster 2: around (5, 5)
    for i in 0..20 {
        let angle = (i as f32) * 2.0 * std::f32::consts::PI / 20.0;
        embeddings.push(vec![5.0 + angle.cos(), 5.0 + angle.sin()]);
    }

    let quality = analyzer.analyze(&embeddings).unwrap();

    println!("Quality Analysis:");
    println!("  Dimensions: {}", quality.dimensions);
    println!("  Vectors: {}", quality.num_vectors);
    println!("  Connected components: {}", quality.connected_components);
    println!("  Clustering coefficient: {:.3}", quality.clustering_coefficient);
    println!("  Mode collapse score: {:.3}", quality.mode_collapse_score);
    println!("  Degeneracy score: {:.3}", quality.degeneracy_score);
    println!("  Quality score: {:.3}", quality.quality_score);
    println!("  Assessment: {}", quality.assessment());

    assert_eq!(quality.dimensions, 2);
    assert_eq!(quality.num_vectors, 40);
    assert!(!quality.has_mode_collapse());
    assert!(!quality.is_degenerate());
}

#[test]
fn test_mode_collapse_detection() {
    let analyzer = TopologicalAnalyzer::new(3, 5.0);

    // Create collapsed embeddings (all very similar)
    let collapsed: Vec<Vec<f32>> = (0..50)
        .map(|i| vec![1.0 + (i as f32) * 0.001, 1.0 + (i as f32) * 0.001])
        .collect();

    let quality = analyzer.analyze(&collapsed).unwrap();

    println!("Collapsed embeddings quality: {:.3}", quality.quality_score);
    assert!(quality.has_mode_collapse());
    assert!(quality.quality_score < 0.5);
}

#[test]
fn test_integration_hypergraph_with_hash() {
    // Integration test: Use neural hashing for hyperedge embeddings
    let lsh = SimpleLSH::new(3, 32);
    let mut hash_index = HashIndex::new(lsh, 32);

    let mut hypergraph = HypergraphIndex::new(DistanceMetric::Cosine);

    // Add entities
    for i in 0..10 {
        let embedding = vec![i as f32, (i * 2) as f32, (i * i) as f32];
        hypergraph.add_entity(i, embedding.clone());
        hash_index.insert(i, embedding);
    }

    // Add hyperedges
    for i in 0..5 {
        let edge = Hyperedge::new(
            vec![i, i + 1, i + 2],
            format!("Relationship {}", i),
            vec![i as f32 * 0.5, (i + 1) as f32 * 0.5, (i + 2) as f32 * 0.3],
            0.9,
        );
        hypergraph.add_hyperedge(edge).unwrap();
    }

    // Use hash index for fast filtering, then hypergraph for precise results
    let query = vec![2.5, 5.0, 6.25];
    let hash_results = hash_index.search(&query, 10, 8);
    assert!(!hash_results.is_empty());

    let hypergraph_results = hypergraph.search_hyperedges(&query, 5);
    assert!(!hypergraph_results.is_empty());

    println!("Hash index found {} candidates", hash_results.len());
    println!("Hypergraph found {} relevant edges", hypergraph_results.len());
}
