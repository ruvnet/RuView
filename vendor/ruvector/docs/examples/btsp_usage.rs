//! # BTSP Usage Examples
//!
//! Demonstrates one-shot learning for vector database applications

use ruvector_nervous_system::plasticity::btsp::{
    BTSPAssociativeMemory, BTSPLayer, BTSPSynapse,
};

/// Example 1: Basic one-shot learning
fn example_one_shot_learning() {
    println!("=== Example 1: One-Shot Learning ===\n");

    // Create a layer with 128 inputs, 2-second time constant
    let mut layer = BTSPLayer::new(128, 2000.0);

    // Learn pattern -> target association instantly
    let pattern = vec![0.5; 128];
    let target = 0.9;

    layer.one_shot_associate(&pattern, target);
    println!("Learned: pattern -> {}", target);

    // Immediate recall (no training iterations needed)
    let output = layer.forward(&pattern);
    let error = (output - target).abs();

    println!("Recalled: {} (error: {:.4})", output, error);
    println!("One-shot learning: {}\n", if error < 0.1 { "✓" } else { "✗" });
}

/// Example 2: Vector embedding storage
fn example_embedding_storage() {
    println!("=== Example 2: Embedding Storage ===\n");

    // Create associative memory for 384-dim embeddings -> 128-dim metadata
    let mut memory = BTSPAssociativeMemory::new(384, 128);

    // Store embeddings instantly (no batch training)
    let embedding1 = vec![0.5; 384];
    let metadata1 = vec![1.0, 0.0, 0.0, 0.5, 0.8];
    metadata1.extend(vec![0.0; 123]); // Pad to 128

    let embedding2 = vec![0.3; 384];
    let metadata2 = vec![0.0, 1.0, 0.5, 0.2, 0.9];
    metadata2.extend(vec![0.0; 123]);

    memory.store_one_shot(&embedding1, &metadata1).unwrap();
    memory.store_one_shot(&embedding2, &metadata2).unwrap();

    println!("Stored 2 embeddings instantly (no iterations)");

    // Retrieve
    let retrieved = memory.retrieve(&embedding1).unwrap();
    println!("Retrieved metadata dim: {}", retrieved.len());
    println!("First 5 values: {:?}\n", &retrieved[..5]);
}

/// Example 3: Adaptive query routing
fn example_adaptive_routing() {
    println!("=== Example 3: Adaptive Query Routing ===\n");

    let mut layer = BTSPLayer::new(64, 2000.0);

    // Learn query patterns -> optimal routes
    let queries = vec![
        (vec![1.0; 64], 0.9), // High priority
        (vec![0.5; 64], 0.5), // Medium priority
        (vec![0.1; 64], 0.1), // Low priority
    ];

    for (query, route) in &queries {
        layer.one_shot_associate(query, *route);
        println!("Learned route: {:?} -> {}", &query[..3], route);
    }

    // Test routing
    let test_query = vec![1.0; 64];
    let route = layer.forward(&test_query);
    println!("\nQuery route: {:.2} (should be ~0.9)", route);
}

/// Example 4: Temporal learning with eligibility traces
fn example_eligibility_traces() {
    println!("\n=== Example 4: Eligibility Traces ===\n");

    let mut synapse = BTSPSynapse::new(0.5, 2000.0).unwrap();

    // Simulate 1 second of activity
    println!("Time\tActive\tPlateau\tTrace\tWeight");
    for t in 0..100 {
        let time = t as f32 * 10.0; // 10ms steps
        let active = t < 50; // Active for first 500ms
        let plateau = t == 60; // Plateau at 600ms

        synapse.update(active, plateau, 10.0);

        if t % 10 == 0 {
            println!(
                "{}ms\t{}\t{}\t{:.3}\t{:.3}",
                time,
                if active { "Y" } else { "N" },
                if plateau { "Y" } else { "N" },
                synapse.eligibility_trace(),
                synapse.weight()
            );
        }
    }
}

/// Example 5: Batch association storage
fn example_batch_storage() {
    println!("\n=== Example 5: Batch Storage ===\n");

    let mut memory = BTSPAssociativeMemory::new(64, 32);

    // Store multiple associations
    let pairs = vec![
        (vec![1.0; 64], vec![0.1; 32]),
        (vec![0.8; 64], vec![0.2; 32]),
        (vec![0.6; 64], vec![0.3; 32]),
        (vec![0.4; 64], vec![0.4; 32]),
        (vec![0.2; 64], vec![0.5; 32]),
    ];

    let pair_refs: Vec<_> = pairs
        .iter()
        .map(|(k, v)| (k.as_slice(), v.as_slice()))
        .collect();

    memory.store_batch(&pair_refs).unwrap();
    println!("Stored {} associations instantly", pairs.len());

    // Verify storage
    for (i, (key, expected)) in pairs.iter().enumerate() {
        let retrieved = memory.retrieve(key).unwrap();
        let error: f32 = expected
            .iter()
            .zip(retrieved.iter())
            .map(|(e, r)| (e - r).abs())
            .sum::<f32>()
            / expected.len() as f32;

        println!("Pair {}: recall error = {:.4}", i + 1, error);
    }
}

/// Example 6: Real-world vector database scenario
fn example_vector_database() {
    println!("\n=== Example 6: Vector Database Integration ===\n");

    // Scenario: Store document embeddings with instant indexing

    struct Document {
        id: String,
        embedding: Vec<f32>,
        metadata: Vec<f32>,
    }

    let documents = vec![
        Document {
            id: "doc1".into(),
            embedding: vec![0.8; 768],
            metadata: vec![1.0, 0.0, 0.5, 0.8],
        },
        Document {
            id: "doc2".into(),
            embedding: vec![0.6; 768],
            metadata: vec![0.0, 1.0, 0.3, 0.6],
        },
        Document {
            id: "doc3".into(),
            embedding: vec![0.4; 768],
            metadata: vec![0.5, 0.5, 0.7, 0.4],
        },
    ];

    // Create BTSP memory for 768-dim embeddings (common size)
    let mut db_memory = BTSPAssociativeMemory::new(768, 4);

    println!("Indexing documents with one-shot learning:");
    for doc in &documents {
        db_memory
            .store_one_shot(&doc.embedding, &doc.metadata)
            .unwrap();
        println!("  ✓ Indexed {} instantly", doc.id);
    }

    // Query
    println!("\nQuerying:");
    let query = vec![0.8; 768];
    let result = db_memory.retrieve(&query).unwrap();
    println!(
        "  Query result: {:?} (closest to doc1)",
        &result[..4]
    );

    println!("\n✓ Vector database with instant, no-iteration indexing");
}

fn main() {
    example_one_shot_learning();
    example_embedding_storage();
    example_adaptive_routing();
    example_eligibility_traces();
    example_batch_storage();
    example_vector_database();

    println!("\n═══════════════════════════════════════════");
    println!("All BTSP examples completed successfully!");
    println!("═══════════════════════════════════════════\n");
}
