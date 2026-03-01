//! Performance benchmarks for EXO-AI cognitive substrate
//!
//! Tests the performance of theoretical framework implementations

use std::time::Instant;

// EXO-AI crates
use exo_core::{Metadata, Pattern, PatternId, SubstrateTime};
use exo_federation::crypto::PostQuantumKeypair;
use exo_temporal::{ConsolidationConfig, Query, TemporalConfig, TemporalMemory};

const VECTOR_DIM: usize = 384;
const NUM_VECTORS: usize = 1_000;
const K_NEAREST: usize = 10;

fn generate_random_vector(dim: usize, seed: u64) -> Vec<f32> {
    let mut vec = Vec::with_capacity(dim);
    let mut state = seed;
    for _ in 0..dim {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        vec.push((state as f32) / (u64::MAX as f32));
    }
    vec
}

#[test]
fn benchmark_temporal_memory() {
    println!("\n=== EXO-AI Temporal Memory Performance ===\n");

    let vectors: Vec<Vec<f32>> = (0..NUM_VECTORS)
        .map(|i| generate_random_vector(VECTOR_DIM, i as u64))
        .collect();

    let config = TemporalConfig {
        consolidation: ConsolidationConfig {
            salience_threshold: 0.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let temporal = TemporalMemory::new(config);

    // Insert benchmark
    let start = Instant::now();
    for vec in vectors.iter() {
        let pattern = Pattern {
            id: PatternId::new(),
            embedding: vec.clone(),
            metadata: Metadata::default(),
            timestamp: SubstrateTime::now(),
            antecedents: Vec::new(),
            salience: 1.0,
        };
        temporal.store(pattern, &[]).unwrap();
    }
    let insert_time = start.elapsed();
    println!("Insert {} patterns: {:?}", NUM_VECTORS, insert_time);
    println!("  Per insert: {:?}", insert_time / NUM_VECTORS as u32);

    // Consolidation benchmark
    let start = Instant::now();
    let result = temporal.consolidate();
    let consolidate_time = start.elapsed();
    println!("\nConsolidate: {:?}", consolidate_time);
    println!("  Patterns consolidated: {}", result.num_consolidated);

    // Search benchmark
    let query = Query::from_embedding(generate_random_vector(VECTOR_DIM, 999999));
    let start = Instant::now();
    for _ in 0..100 {
        let _ = temporal.long_term().search(&query);
    }
    let search_time = start.elapsed();
    println!("\n100 searches: {:?}", search_time);
    println!("  Per search: {:?}", search_time / 100);
}

#[test]
fn benchmark_consciousness_metrics() {
    use exo_core::consciousness::{ConsciousnessCalculator, NodeState, SubstrateRegion};
    use std::collections::HashMap;

    println!("\n=== IIT Phi Calculation Performance ===\n");

    // Create a small reentrant network
    let nodes = vec![1, 2, 3, 4, 5];
    let mut connections = HashMap::new();
    connections.insert(1, vec![2, 3]);
    connections.insert(2, vec![4]);
    connections.insert(3, vec![4]);
    connections.insert(4, vec![5]);
    connections.insert(5, vec![1]); // Feedback loop

    let mut states = HashMap::new();
    for &node in &nodes {
        states.insert(
            node,
            NodeState {
                activation: 0.5,
                previous_activation: 0.4,
            },
        );
    }

    let region = SubstrateRegion {
        id: "test".to_string(),
        nodes,
        connections,
        states,
        has_reentrant_architecture: true,
    };

    let calculator = ConsciousnessCalculator::new(100);

    let start = Instant::now();
    let mut total_phi = 0.0;
    for _ in 0..1000 {
        let result = calculator.compute_phi(&region);
        total_phi += result.phi;
    }
    let phi_time = start.elapsed();

    println!("1000 Phi calculations: {:?}", phi_time);
    println!("  Per calculation: {:?}", phi_time / 1000);
    println!("  Average Phi: {:.4}", total_phi / 1000.0);
}

#[test]
fn benchmark_thermodynamic_tracking() {
    use exo_core::thermodynamics::{Operation, ThermodynamicTracker};

    println!("\n=== Landauer Thermodynamic Tracking Performance ===\n");

    let tracker = ThermodynamicTracker::room_temperature();

    let start = Instant::now();
    for _ in 0..100_000 {
        tracker.record_operation(Operation::VectorSimilarity { dimensions: 384 });
        tracker.record_operation(Operation::MemoryWrite { bytes: 1536 });
    }
    let track_time = start.elapsed();

    println!("200,000 operation recordings: {:?}", track_time);
    println!("  Per operation: {:?}", track_time / 200_000);

    let report = tracker.efficiency_report();
    println!("\nEfficiency Report:");
    println!("  Total bit erasures: {}", report.total_bit_erasures);
    println!(
        "  Landauer minimum: {:.2e} J",
        report.landauer_minimum_joules
    );
    println!(
        "  Estimated actual: {:.2e} J",
        report.estimated_actual_joules
    );
    println!(
        "  Efficiency ratio: {:.0}x above Landauer",
        report.efficiency_ratio
    );
    println!(
        "  Reversible savings: {:.2}%",
        (report.reversible_savings_potential / report.estimated_actual_joules) * 100.0
    );
}

#[test]
fn benchmark_post_quantum_crypto() {
    println!("\n=== Post-Quantum Cryptography Performance ===\n");

    // Key generation
    let start = Instant::now();
    let mut keypairs = Vec::new();
    for _ in 0..100 {
        keypairs.push(PostQuantumKeypair::generate());
    }
    let keygen_time = start.elapsed();
    println!("100 Kyber-1024 keypair generations: {:?}", keygen_time);
    println!("  Per keypair: {:?}", keygen_time / 100);

    // Encapsulation
    let start = Instant::now();
    for keypair in keypairs.iter().take(100) {
        let _ = PostQuantumKeypair::encapsulate(keypair.public_key()).unwrap();
    }
    let encap_time = start.elapsed();
    println!("\n100 encapsulations: {:?}", encap_time);
    println!("  Per encapsulation: {:?}", encap_time / 100);

    // Decapsulation
    let keypair = &keypairs[0];
    let (_, ciphertext) = PostQuantumKeypair::encapsulate(keypair.public_key()).unwrap();

    let start = Instant::now();
    for _ in 0..100 {
        let _ = keypair.decapsulate(&ciphertext).unwrap();
    }
    let decap_time = start.elapsed();
    println!("\n100 decapsulations: {:?}", decap_time);
    println!("  Per decapsulation: {:?}", decap_time / 100);

    println!("\nSecurity: NIST Level 5 (256-bit post-quantum)");
    println!("Public key size: 1568 bytes");
    println!("Ciphertext size: 1568 bytes");
}
