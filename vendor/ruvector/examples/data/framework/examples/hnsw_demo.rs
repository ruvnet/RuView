//! HNSW Index Demo
//!
//! Demonstrates the HNSW indexing capabilities for semantic vector search.

use chrono::Utc;
use ruvector_data_framework::hnsw::{DistanceMetric, HnswConfig, HnswIndex};
use ruvector_data_framework::ruvector_native::{Domain, SemanticVector};
use std::collections::HashMap;

fn main() {
    println!("🔍 RuVector HNSW Index Demo\n");
    println!("{}", "=".repeat(60));

    // Configure HNSW for 128-dimensional vectors
    let config = HnswConfig {
        m: 16,
        m_max_0: 32,
        ef_construction: 200,
        ef_search: 50,
        ml: 1.0 / 16.0_f64.ln(),
        dimension: 128,
        metric: DistanceMetric::Cosine,
    };

    println!("\n📊 Configuration:");
    println!("   Dimensions: {}", config.dimension);
    println!("   M (connections per layer): {}", config.m);
    println!("   M_max_0 (layer 0 connections): {}", config.m_max_0);
    println!("   ef_construction: {}", config.ef_construction);
    println!("   ef_search: {}", config.ef_search);
    println!("   Metric: {:?}", config.metric);

    let mut index = HnswIndex::with_config(config);

    // Create sample vectors
    println!("\n📝 Inserting vectors...");

    let vectors = vec![
        create_vector("climate_1", generate_random_vector(128), Domain::Climate),
        create_vector("climate_2", generate_random_vector(128), Domain::Climate),
        create_vector("finance_1", generate_random_vector(128), Domain::Finance),
        create_vector("finance_2", generate_random_vector(128), Domain::Finance),
        create_vector("research_1", generate_random_vector(128), Domain::Research),
    ];

    // Insert vectors
    for vec in vectors.clone() {
        match index.insert(vec.clone()) {
            Ok(id) => println!("   ✓ Inserted {} with node_id {}", vec.id, id),
            Err(e) => println!("   ✗ Failed to insert {}: {}", vec.id, e),
        }
    }

    // Get statistics
    let stats = index.stats();
    println!("\n📈 Index Statistics:");
    println!("   Total nodes: {}", stats.node_count);
    println!("   Layers: {}", stats.layer_count);
    println!("   Total edges: {}", stats.total_edges);
    println!("   Memory estimate: {} bytes", stats.estimated_memory_bytes);
    println!("\n   Nodes per layer:");
    for (layer, count) in stats.nodes_per_layer.iter().enumerate() {
        println!("      Layer {}: {} nodes (avg {:.2} connections)",
            layer, count, stats.avg_connections_per_layer[layer]);
    }

    // Perform k-NN search
    println!("\n🔍 K-NN Search (k=3):");
    let query = vectors[0].embedding.clone();
    println!("   Query: {}", vectors[0].id);

    match index.search_knn(&query, 3) {
        Ok(results) => {
            for (i, result) in results.iter().enumerate() {
                println!(
                    "   {}. {} (distance: {:.4}, similarity: {:.4})",
                    i + 1,
                    result.external_id,
                    result.distance,
                    result.similarity.unwrap_or(0.0)
                );
            }
        }
        Err(e) => println!("   ✗ Search failed: {}", e),
    }

    // Threshold search
    println!("\n🎯 Threshold Search (distance < 0.5):");
    match index.search_threshold(&query, 0.5, Some(10)) {
        Ok(results) => {
            println!("   Found {} vectors within threshold:", results.len());
            for result in results.iter() {
                println!(
                    "      {} (distance: {:.4})",
                    result.external_id, result.distance
                );
            }
        }
        Err(e) => println!("   ✗ Search failed: {}", e),
    }

    // Batch insertion demo
    println!("\n📦 Batch Insertion Demo:");
    let batch_vectors: Vec<SemanticVector> = (0..5)
        .map(|i| {
            create_vector(
                &format!("batch_{}", i),
                generate_random_vector(128),
                Domain::CrossDomain,
            )
        })
        .collect();

    match index.insert_batch(batch_vectors.clone()) {
        Ok(ids) => {
            println!("   ✓ Inserted {} vectors in batch", ids.len());
            println!("   Node IDs: {:?}", ids);
        }
        Err(e) => println!("   ✗ Batch insertion failed: {}", e),
    }

    // Final statistics
    let final_stats = index.stats();
    println!("\n📊 Final Statistics:");
    println!("   Total nodes: {}", final_stats.node_count);
    println!("   Total edges: {}", final_stats.total_edges);
    println!("   Memory estimate: {:.2} KB",
        final_stats.estimated_memory_bytes as f64 / 1024.0);

    println!("\n✅ Demo complete!");
    println!("{}", "=".repeat(60));
}

fn create_vector(id: &str, embedding: Vec<f32>, domain: Domain) -> SemanticVector {
    SemanticVector {
        id: id.to_string(),
        embedding,
        domain,
        timestamp: Utc::now(),
        metadata: HashMap::new(),
    }
}

fn generate_random_vector(dim: usize) -> Vec<f32> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..dim).map(|_| rng.gen::<f32>()).collect()
}
