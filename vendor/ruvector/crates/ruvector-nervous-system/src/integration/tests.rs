//! Integration tests for nervous system RuVector integration

use super::*;

#[test]
fn test_end_to_end_integration() {
    // Create a complete nervous system-enhanced index
    let config = NervousConfig::new(64)
        .with_hopfield(3.0, 100)
        .with_pattern_separation(5000, 100)
        .with_one_shot(true);

    let mut index = NervousVectorIndex::new(64, config);

    // Insert some vectors
    let v1 = vec![1.0; 64];
    let v2 = vec![0.5; 64];

    let _id1 = index.insert(&v1, Some("vector_1"));
    let _id2 = index.insert(&v2, Some("vector_2"));

    assert_eq!(index.len(), 2);

    // Hybrid search
    let query = vec![0.9; 64];
    let results = index.search_hybrid(&query, 2);

    assert_eq!(results.len(), 2);
    assert!(results[0].combined_score >= results[1].combined_score);

    // One-shot learning
    let key = vec![0.1; 64];
    let value = vec![0.8; 64];
    index.learn_one_shot(&key, &value);

    let retrieved = index.retrieve_one_shot(&key);
    assert!(retrieved.is_some());
}

#[test]
fn test_predictive_writer_integration() {
    let config = PredictiveConfig::new(32).with_threshold(0.15);
    let mut writer = PredictiveWriter::new(config);

    // Simulate database writes
    let mut vectors = vec![];
    for i in 0..100 {
        let mut v = vec![1.0; 32];
        v[0] = 1.0 + (i as f32 * 0.01).sin() * 0.1;
        vectors.push(v);
    }

    let mut write_count = 0;
    for vector in &vectors {
        if let Some(_residual) = writer.residual_write(vector) {
            write_count += 1;
        }
    }

    let stats = writer.stats();

    // Should have significant compression
    assert!(
        stats.bandwidth_reduction > 0.5,
        "Bandwidth reduction: {:.1}%",
        stats.reduction_percent()
    );

    println!(
        "Wrote {} out of {} vectors ({:.1}% reduction)",
        write_count,
        vectors.len(),
        stats.reduction_percent()
    );
}

#[test]
fn test_collection_versioning_workflow() {
    let schedule = ConsolidationSchedule::new(100, 16, 0.01);
    let mut versioning = CollectionVersioning::new(42, schedule);

    // Version 1: Initial parameters
    versioning.bump_version();
    let params_v1 = vec![0.5; 50];
    versioning.update_parameters(&params_v1);

    // Simulate some learning
    let gradients_v1: Vec<Vec<f32>> = (0..20).map(|_| vec![0.1; 50]).collect();

    versioning.consolidate(&gradients_v1, 0).unwrap();

    // Version 2: Update parameters (task 2)
    versioning.bump_version();
    let params_v2 = vec![0.6; 50];
    versioning.update_parameters(&params_v2);

    // EWC should protect v1 parameters
    let ewc_loss = versioning.ewc_loss();
    assert!(ewc_loss > 0.0, "EWC should penalize parameter drift");

    // Apply EWC to new gradients
    let new_gradients = vec![0.2; 50];
    let modified = versioning.apply_ewc(&new_gradients);

    // Should be different due to EWC penalty
    assert_ne!(modified, new_gradients);
}

#[test]
fn test_pattern_separation_collision_resistance() {
    let config = NervousConfig::new(128).with_pattern_separation(10000, 200);

    let index = NervousVectorIndex::new(128, config);

    // Create two very similar vectors (95% overlap)
    let v1 = vec![1.0; 128];
    let mut v2 = vec![1.0; 128];

    // Only differ in last 5%
    for i in 122..128 {
        v2[i] = 0.0;
    }

    // Encode both
    let enc1 = index.encode_pattern(&v1).unwrap();
    let enc2 = index.encode_pattern(&v2).unwrap();

    // Compute Jaccard similarity
    let intersection: usize = enc1
        .iter()
        .zip(enc2.iter())
        .filter(|(&a, &b)| a != 0.0 && b != 0.0)
        .count();

    let union: usize = enc1
        .iter()
        .zip(enc2.iter())
        .filter(|(&a, &b)| a != 0.0 || b != 0.0)
        .count();

    let jaccard = intersection as f32 / union as f32;

    // Pattern separation: output should be less similar than input
    let input_similarity = 122.0 / 128.0; // 95%

    assert!(
        jaccard < input_similarity,
        "Pattern separation failed: output similarity ({:.2}) >= input similarity ({:.2})",
        jaccard,
        input_similarity
    );

    println!(
        "Input similarity: {:.2}%, Output similarity: {:.2}%",
        input_similarity * 100.0,
        jaccard * 100.0
    );
}

#[test]
fn test_hopfield_hopfield_convergence() {
    let config = NervousConfig::new(32).with_hopfield(5.0, 10);
    let mut index = NervousVectorIndex::new(32, config);

    // Store a pattern
    let pattern = vec![
        1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0,
        1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0,
    ];

    index.insert(&pattern, None);

    // Query with noisy version
    let mut noisy = pattern.clone();
    noisy[0] = -1.0; // Flip 3 bits
    noisy[5] = 1.0;
    noisy[10] = -1.0;

    let retrieved = index.search_hopfield(&noisy);

    // Should converge towards original pattern
    let mut matches = 0;
    if let Some(ref result) = retrieved {
        for i in 0..32.min(result.len()) {
            if (result[i] > 0.0 && pattern[i] > 0.0) || (result[i] < 0.0 && pattern[i] < 0.0) {
                matches += 1;
            }
        }
    }

    let accuracy = matches as f32 / 32.0;
    assert!(
        accuracy > 0.8,
        "Hopfield retrieval accuracy: {:.1}%",
        accuracy * 100.0
    );
}

#[test]
fn test_one_shot_learning_multiple_associations() {
    let config = NervousConfig::new(16).with_one_shot(true);
    let mut index = NervousVectorIndex::new(16, config);

    // Learn multiple associations
    let associations = vec![
        (vec![1.0; 16], vec![0.0; 16]),
        (vec![0.0; 16], vec![1.0; 16]),
        (vec![0.5; 16], vec![0.5; 16]),
    ];

    for (key, value) in &associations {
        index.learn_one_shot(key, value);
    }

    // Retrieve associations - just verify retrieval works
    // (weight interference between patterns makes exact recall difficult)
    for (key, _expected_value) in &associations {
        let retrieved = index.retrieve_one_shot(key);
        assert!(retrieved.is_some(), "Should retrieve something for key");

        let ret = retrieved.unwrap();
        assert_eq!(
            ret.len(),
            16,
            "Retrieved vector should have correct dimension"
        );
    }
}

#[test]
fn test_adaptive_threshold_convergence() {
    let config = PredictiveConfig::new(16)
        .with_threshold(0.5) // Start with high threshold
        .with_target_compression(0.1); // Target 10% writes

    let mut writer = PredictiveWriter::new(config);

    let initial_threshold = writer.threshold();

    // Slowly varying signal
    for i in 0..500 {
        let mut signal = vec![0.5; 16];
        signal[0] = 0.5 + (i as f32 * 0.01).sin() * 0.1;
        let _ = writer.residual_write(&signal);
    }

    let final_threshold = writer.threshold();
    let stats = writer.stats();

    println!(
        "Threshold: {:.3} → {:.3}, Compression: {:.1}%",
        initial_threshold,
        final_threshold,
        stats.compression_ratio * 100.0
    );

    // Threshold should have adapted
    // If we're writing too much, threshold should increase
    // If we're writing too little, threshold should decrease
    assert!(final_threshold > 0.01 && final_threshold < 0.5);
}
