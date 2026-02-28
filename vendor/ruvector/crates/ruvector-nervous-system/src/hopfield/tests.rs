//! Integration tests for Modern Hopfield Networks

use super::*;
use approx::assert_relative_eq;
use rand::Rng;

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

fn add_noise(vector: &[f32], noise_level: f32) -> Vec<f32> {
    let mut rng = rand::thread_rng();
    vector
        .iter()
        .map(|&x| x + rng.gen_range(-noise_level..noise_level))
        .collect()
}

#[test]
fn test_perfect_retrieval() {
    let mut hopfield = ModernHopfield::new(128, 1.0);

    let pattern = vec![1.0; 128];
    hopfield.store(pattern.clone()).unwrap();

    let retrieved = hopfield.retrieve(&pattern).unwrap();

    // Should retrieve exactly the same pattern
    let similarity = cosine_similarity(&pattern, &retrieved);
    assert!(similarity > 0.999, "Similarity: {}", similarity);
}

#[test]
fn test_retrieval_with_noise() {
    let mut hopfield = ModernHopfield::new(128, 2.0);

    let pattern = vec![1.0; 128];
    hopfield.store(pattern.clone()).unwrap();

    // Add small noise
    let noisy_query = add_noise(&pattern, 0.1);
    let retrieved = hopfield.retrieve(&noisy_query).unwrap();

    // Should still retrieve similar pattern
    let similarity = cosine_similarity(&pattern, &retrieved);
    assert!(similarity > 0.95, "Similarity with noise: {}", similarity);
}

#[test]
fn test_multiple_patterns() {
    let mut hopfield = ModernHopfield::new(128, 1.0);

    // Store orthogonal patterns
    let mut pattern1 = vec![0.0; 128];
    pattern1[0] = 1.0;

    let mut pattern2 = vec![0.0; 128];
    pattern2[1] = 1.0;

    let mut pattern3 = vec![0.0; 128];
    pattern3[2] = 1.0;

    hopfield.store(pattern1.clone()).unwrap();
    hopfield.store(pattern2.clone()).unwrap();
    hopfield.store(pattern3.clone()).unwrap();

    // Retrieve each pattern
    let retrieved1 = hopfield.retrieve(&pattern1).unwrap();
    let retrieved2 = hopfield.retrieve(&pattern2).unwrap();
    let retrieved3 = hopfield.retrieve(&pattern3).unwrap();

    // Each should match its original (relaxed for softmax blending)
    assert!(
        cosine_similarity(&pattern1, &retrieved1) > 0.5,
        "pattern1 sim: {}",
        cosine_similarity(&pattern1, &retrieved1)
    );
    assert!(
        cosine_similarity(&pattern2, &retrieved2) > 0.5,
        "pattern2 sim: {}",
        cosine_similarity(&pattern2, &retrieved2)
    );
    assert!(
        cosine_similarity(&pattern3, &retrieved3) > 0.5,
        "pattern3 sim: {}",
        cosine_similarity(&pattern3, &retrieved3)
    );
}

#[test]
fn test_capacity_demonstration() {
    // Test that we can store many patterns
    let dimension = 64;
    let num_patterns = 100;
    let mut hopfield = ModernHopfield::new(dimension, 2.0);

    let mut rng = rand::thread_rng();
    let mut patterns = Vec::new();

    // Generate random patterns
    for _ in 0..num_patterns {
        let pattern: Vec<f32> = (0..dimension).map(|_| rng.gen_range(-1.0..1.0)).collect();
        patterns.push(pattern.clone());
        hopfield.store(pattern).unwrap();
    }

    assert_eq!(hopfield.num_patterns(), num_patterns);

    // Test retrieval accuracy
    let mut correct = 0;
    for (i, pattern) in patterns.iter().enumerate() {
        let retrieved = hopfield.retrieve(pattern).unwrap();
        let similarity = cosine_similarity(pattern, &retrieved);

        // Check if this pattern has highest similarity
        let mut max_sim = 0.0;
        let mut max_idx = 0;
        for (j, other) in patterns.iter().enumerate() {
            let sim = cosine_similarity(&retrieved, other);
            if sim > max_sim {
                max_sim = sim;
                max_idx = j;
            }
        }

        if max_idx == i {
            correct += 1;
        }
    }

    let accuracy = correct as f32 / num_patterns as f32;
    assert!(accuracy > 0.8, "Accuracy: {}", accuracy);
}

#[test]
fn test_beta_parameter_effect() {
    let dimension = 64;
    let mut hopfield_low = ModernHopfield::new(dimension, 0.5);
    let mut hopfield_high = ModernHopfield::new(dimension, 5.0);

    // Create two similar patterns
    let pattern1: Vec<f32> = vec![1.0; dimension];
    let mut pattern2 = pattern1.clone();
    pattern2[0] = 0.9; // Slightly different

    hopfield_low.store(pattern1.clone()).unwrap();
    hopfield_low.store(pattern2.clone()).unwrap();

    hopfield_high.store(pattern1.clone()).unwrap();
    hopfield_high.store(pattern2.clone()).unwrap();

    // Query with pattern1
    let retrieved_low = hopfield_low.retrieve(&pattern1).unwrap();
    let retrieved_high = hopfield_high.retrieve(&pattern1).unwrap();

    // High beta should give sharper retrieval (closer to pattern1)
    let sim_low = cosine_similarity(&pattern1, &retrieved_low);
    let sim_high = cosine_similarity(&pattern1, &retrieved_high);

    assert!(sim_high >= sim_low, "High beta should be sharper");
}

#[test]
fn test_retrieve_k() {
    let mut hopfield = ModernHopfield::new(64, 1.0);

    // Store 5 patterns with known similarities
    let query = vec![1.0; 64];

    let pattern1 = query.clone(); // Exact match
    let mut pattern2 = query.clone();
    pattern2[0] = 0.9; // Close match

    let mut pattern3 = query.clone();
    pattern3[0] = 0.5; // Medium match

    let pattern4 = vec![0.0; 64]; // No match
    let pattern5 = vec![-1.0; 64]; // Opposite

    hopfield.store(pattern1).unwrap();
    hopfield.store(pattern2).unwrap();
    hopfield.store(pattern3).unwrap();
    hopfield.store(pattern4).unwrap();
    hopfield.store(pattern5).unwrap();

    // Retrieve top 3
    let top_k = hopfield.retrieve_k(&query, 3).unwrap();

    assert_eq!(top_k.len(), 3);

    // Check that attention weights are in descending order
    assert!(top_k[0].2 >= top_k[1].2);
    assert!(top_k[1].2 >= top_k[2].2);

    // First result should be the exact match (index 0)
    assert_eq!(top_k[0].0, 0);
}

#[test]
fn test_theoretical_capacity() {
    let hopfield = ModernHopfield::new(128, 1.0);
    let capacity = hopfield.capacity();

    // For 128 dimensions, capacity saturates to u64::MAX (exponent = 64)
    assert_eq!(capacity, u64::MAX);
}

#[test]
fn test_with_random_patterns() {
    let dimension = 128;
    let num_patterns = 50;
    let mut hopfield = ModernHopfield::new(dimension, 1.0);

    let mut rng = rand::thread_rng();
    let mut patterns = Vec::new();

    // Generate and store random patterns
    for _ in 0..num_patterns {
        let pattern: Vec<f32> = (0..dimension).map(|_| rng.gen_range(-1.0..1.0)).collect();
        patterns.push(pattern.clone());
        hopfield.store(pattern).unwrap();
    }

    // Test retrieval with noise
    for pattern in &patterns {
        let noisy = add_noise(pattern, 0.05);
        let retrieved = hopfield.retrieve(&noisy).unwrap();

        let similarity = cosine_similarity(pattern, &retrieved);
        assert!(similarity > 0.8, "Failed with similarity: {}", similarity);
    }
}

#[test]
fn test_comparison_with_baseline() {
    // Simple baseline: return closest stored pattern
    fn baseline_retrieve(patterns: &[Vec<f32>], query: &[f32]) -> Vec<f32> {
        patterns
            .iter()
            .max_by(|a, b| {
                let sim_a = cosine_similarity(a, query);
                let sim_b = cosine_similarity(b, query);
                sim_a.partial_cmp(&sim_b).unwrap()
            })
            .unwrap()
            .clone()
    }

    let dimension = 64;
    let mut hopfield = ModernHopfield::new(dimension, 2.0);

    let mut rng = rand::thread_rng();
    let mut patterns = Vec::new();

    // Generate patterns
    for _ in 0..20 {
        let pattern: Vec<f32> = (0..dimension).map(|_| rng.gen_range(-1.0..1.0)).collect();
        patterns.push(pattern.clone());
        hopfield.store(pattern).unwrap();
    }

    // Test on multiple queries
    for pattern in &patterns {
        let noisy = add_noise(pattern, 0.1);

        let hopfield_result = hopfield.retrieve(&noisy).unwrap();
        let baseline_result = baseline_retrieve(&patterns, &noisy);

        let hopfield_sim = cosine_similarity(pattern, &hopfield_result);
        let baseline_sim = cosine_similarity(pattern, &baseline_result);

        // Hopfield should be at least as good as baseline (within 5%)
        assert!(
            hopfield_sim >= baseline_sim * 0.95,
            "Hopfield: {}, Baseline: {}",
            hopfield_sim,
            baseline_sim
        );
    }
}

#[test]
fn test_performance_target() {
    use std::time::Instant;

    let dimension = 512;
    let num_patterns = 1000;
    let mut hopfield = ModernHopfield::new(dimension, 1.0);

    let mut rng = rand::thread_rng();

    // Store 1000 patterns
    for _ in 0..num_patterns {
        let pattern: Vec<f32> = (0..dimension).map(|_| rng.gen_range(-1.0..1.0)).collect();
        hopfield.store(pattern).unwrap();
    }

    // Test retrieval time
    let query: Vec<f32> = (0..dimension).map(|_| rng.gen_range(-1.0..1.0)).collect();

    let start = Instant::now();
    let _retrieved = hopfield.retrieve(&query).unwrap();
    let duration = start.elapsed();

    // Relaxed for CI environments: should be less than 100ms
    assert!(
        duration.as_millis() < 100,
        "Retrieval took {}ms, target is <100ms",
        duration.as_millis()
    );
}
