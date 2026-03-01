//! Integration tests for nervous system components with RuVector
//!
//! These tests verify that nervous system components integrate correctly
//! with RuVector's vector database functionality.

use ruvector_nervous_system::integration::*;

#[test]
fn test_complete_workflow() {
    // Step 1: Create index with all features
    let config = NervousConfig::new(128)
        .with_hopfield(4.0, 500)
        .with_pattern_separation(10000, 200)
        .with_one_shot(true);

    let mut index = NervousVectorIndex::new(128, config);

    // Step 2: Insert vectors
    for i in 0..10 {
        let vector: Vec<f32> = (0..128)
            .map(|j| ((i + j) as f32).sin())
            .collect();
        index.insert(&vector, Some(&format!("vector_{}", i)));
    }

    assert_eq!(index.len(), 10);

    // Step 3: Hybrid search
    let query: Vec<f32> = (0..128).map(|j| (j as f32).cos()).collect();
    let results = index.search_hybrid(&query, 5);

    assert_eq!(results.len(), 5);
    for result in &results {
        assert!(result.combined_score > 0.0);
        assert!(result.vector.is_some());
    }

    // Step 4: One-shot learning
    let key = vec![0.123; 128];
    let value = vec![0.789; 128];
    index.learn_one_shot(&key, &value);

    let retrieved = index.retrieve_one_shot(&key);
    assert!(retrieved.is_some());
}

#[test]
fn test_predictive_write_pipeline() {
    let config = PredictiveConfig::new(256)
        .with_threshold(0.1)
        .with_learning_rate(0.15);

    let mut writer = PredictiveWriter::new(config);

    // Simulate realistic write pattern
    let mut total_writes = 0;
    let total_attempts = 1000;

    for i in 0..total_attempts {
        // Slowly varying signal with occasional spikes
        let mut vector = vec![0.5; 256];

        // Base variation
        vector[0] = 0.5 + (i as f32 * 0.01).sin() * 0.05;

        // Occasional spike
        if i % 100 == 0 {
            vector[1] = 1.0;
        }

        if let Some(_residual) = writer.residual_write(&vector) {
            total_writes += 1;
        }
    }

    let stats = writer.stats();

    assert_eq!(stats.total_attempts, total_attempts);
    assert!(stats.bandwidth_reduction > 0.5); // At least 50% reduction

    println!(
        "Predictive writer: {}/{} writes ({:.1}% reduction)",
        total_writes,
        total_attempts,
        stats.reduction_percent()
    );
}

#[test]
fn test_collection_versioning_continual_learning() {
    let schedule = ConsolidationSchedule::new(60, 32, 0.01);
    let mut versioning = CollectionVersioning::new(123, schedule);

    // Task 1: Learn initial parameters
    versioning.bump_version();
    assert_eq!(versioning.version(), 1);

    let task1_params: Vec<f32> = (0..100).map(|i| (i as f32 * 0.01).sin()).collect();
    versioning.update_parameters(&task1_params);

    // Simulate gradient computation
    let task1_gradients: Vec<Vec<f32>> = (0..50)
        .map(|_| (0..100).map(|_| rand::random::<f32>() * 0.1).collect())
        .collect();

    versioning.consolidate(&task1_gradients, 0).unwrap();

    // Task 2: Learn new parameters
    versioning.bump_version();
    assert_eq!(versioning.version(), 2);

    let task2_params: Vec<f32> = (0..100).map(|i| (i as f32 * 0.02).cos()).collect();
    versioning.update_parameters(&task2_params);

    // EWC should prevent catastrophic forgetting
    let ewc_loss = versioning.ewc_loss();
    assert!(ewc_loss > 0.0);

    // Apply EWC to gradients
    let base_grads: Vec<f32> = (0..100).map(|_| 0.1).collect();
    let protected_grads = versioning.apply_ewc(&base_grads);

    // Gradients should be modified by EWC penalty
    let total_diff: f32 = protected_grads
        .iter()
        .zip(base_grads.iter())
        .map(|(p, b)| (p - b).abs())
        .sum();

    assert!(total_diff > 0.0, "EWC should modify gradients");

    println!("EWC loss: {:.4}, Gradient modification: {:.4}", ewc_loss, total_diff);
}

#[test]
fn test_pattern_separation_properties() {
    let config = NervousConfig::new(256)
        .with_pattern_separation(20000, 400);

    let index = NervousVectorIndex::new(256, config);

    // Test 1: Determinism
    let vector = vec![0.5; 256];
    let enc1 = index.encode_pattern(&vector).unwrap();
    let enc2 = index.encode_pattern(&vector).unwrap();
    assert_eq!(enc1, enc2, "Encoding should be deterministic");

    // Test 2: Sparsity
    let nonzero = enc1.iter().filter(|&&x| x != 0.0).count();
    assert_eq!(nonzero, 400, "Should have exactly k non-zero elements");

    // Test 3: Pattern separation
    let similar_vector: Vec<f32> = (0..256)
        .map(|i| if i < 250 { 0.5 } else { 0.6 })
        .collect();

    let enc_similar = index.encode_pattern(&similar_vector).unwrap();

    // Compute overlap
    let overlap: usize = enc1
        .iter()
        .zip(enc_similar.iter())
        .filter(|(&a, &b)| a != 0.0 && b != 0.0)
        .count();

    let overlap_ratio = overlap as f32 / 400.0;

    // High input similarity should yield low output overlap
    println!("Pattern separation overlap: {:.1}%", overlap_ratio * 100.0);
    assert!(overlap_ratio < 0.5, "Pattern separation should reduce overlap");
}

#[test]
fn test_hybrid_search_quality() {
    let config = NervousConfig::new(64)
        .with_hopfield(3.0, 100);

    let mut index = NervousVectorIndex::new(64, config);

    // Insert orthogonal vectors
    let v1 = vec![1.0; 64];
    let v2 = vec![-1.0; 64];
    let mut v3 = vec![0.0; 64];
    for i in 0..32 {
        v3[i] = 1.0;
    }

    let id1 = index.insert(&v1, Some("all_positive"));
    let id2 = index.insert(&v2, Some("all_negative"));
    let id3 = index.insert(&v3, Some("half_half"));

    // Query close to v1
    let query = vec![0.9; 64];
    let results = index.search_hybrid(&query, 3);

    // v1 should be ranked first
    assert_eq!(results[0].id, id1, "Closest vector should rank first");

    println!("Search results:");
    for (i, result) in results.iter().enumerate() {
        println!(
            "  {}: id={}, combined={:.3}, hnsw_dist={:.3}, hopfield_sim={:.3}",
            i,
            result.id,
            result.combined_score,
            result.hnsw_distance,
            result.hopfield_similarity
        );
    }
}

#[test]
fn test_eligibility_trace_decay() {
    let mut state = EligibilityState::new(1000.0); // 1 second tau

    // Initial update
    state.update(1.0, 0);
    assert_eq!(state.trace(), 1.0);

    // After 1 tau, should decay to ~37%
    state.update(0.0, 1000);
    let trace_1tau = state.trace();
    assert!(trace_1tau > 0.35 && trace_1tau < 0.40);

    // After 2 tau, should decay to ~13.5%
    state.update(0.0, 2000);
    let trace_2tau = state.trace();
    assert!(trace_2tau > 0.10 && trace_2tau < 0.15);

    println!(
        "Eligibility decay: 0tau=1.0, 1tau={:.3}, 2tau={:.3}",
        trace_1tau, trace_2tau
    );
}

#[test]
fn test_consolidation_schedule() {
    let schedule = ConsolidationSchedule::new(3600, 64, 0.02);

    assert_eq!(schedule.replay_interval_secs, 3600);
    assert_eq!(schedule.batch_size, 64);
    assert_eq!(schedule.learning_rate, 0.02);

    // Should not consolidate initially
    assert!(!schedule.should_consolidate(0));

    // After setting last consolidation, should check interval
    let mut sched = schedule.clone();
    sched.last_consolidation = 1000;

    assert!(!sched.should_consolidate(2000)); // 1 second elapsed
    assert!(sched.should_consolidate(5000));  // 4 seconds elapsed (> 3600 not realistic in test)

    // More realistic: 1 hour = 3600 seconds
    sched.last_consolidation = 0;
    assert!(sched.should_consolidate(3600)); // Exactly 1 hour
    assert!(sched.should_consolidate(7200)); // 2 hours
}

#[test]
fn test_performance_benchmarks() {
    use std::time::Instant;

    let config = NervousConfig::new(512)
        .with_pattern_separation(40000, 800);

    let mut index = NervousVectorIndex::new(512, config);

    // Benchmark: Insert 100 vectors
    let start = Instant::now();
    for i in 0..100 {
        let vector: Vec<f32> = (0..512).map(|j| ((i + j) as f32).sin()).collect();
        index.insert(&vector, None);
    }
    let insert_time = start.elapsed();

    // Benchmark: Hybrid search
    let query: Vec<f32> = (0..512).map(|j| (j as f32).cos()).collect();
    let start = Instant::now();
    let _results = index.search_hybrid(&query, 10);
    let search_time = start.elapsed();

    // Benchmark: Pattern encoding
    let start = Instant::now();
    for _ in 0..100 {
        let _ = index.encode_pattern(&query);
    }
    let encoding_time = start.elapsed();

    println!("Performance benchmarks:");
    println!("  Insert 100 vectors: {:?}", insert_time);
    println!("  Hybrid search (k=10): {:?}", search_time);
    println!("  100 pattern encodings: {:?}", encoding_time);
    println!("  Avg encoding: {:?}", encoding_time / 100);

    // Sanity checks (not strict performance requirements)
    assert!(insert_time.as_secs() < 5, "Inserts too slow");
    assert!(search_time.as_millis() < 1000, "Search too slow");
}
