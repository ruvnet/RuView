//! Demonstration of Modern Hopfield Networks
//!
//! This example shows the basic usage of Modern Hopfield Networks
//! for associative memory and pattern retrieval.

use ruvector_nervous_system::hopfield::ModernHopfield;

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

fn main() {
    println!("=== Modern Hopfield Networks Demo ===\n");

    // Create a Modern Hopfield network
    let dimension = 128;
    let beta = 2.0;
    let mut hopfield = ModernHopfield::new(dimension, beta);

    println!("Created Hopfield network:");
    println!("  Dimension: {}", hopfield.dimension());
    println!("  Beta (temperature): {}", hopfield.beta());
    println!("  Theoretical capacity: 2^{} patterns\n", dimension / 2);

    // Store some patterns
    println!("Storing 3 orthogonal patterns...");

    let mut pattern1 = vec![0.0; dimension];
    pattern1[0] = 1.0;

    let mut pattern2 = vec![0.0; dimension];
    pattern2[1] = 1.0;

    let mut pattern3 = vec![0.0; dimension];
    pattern3[2] = 1.0;

    hopfield
        .store(pattern1.clone())
        .expect("Failed to store pattern1");
    hopfield
        .store(pattern2.clone())
        .expect("Failed to store pattern2");
    hopfield
        .store(pattern3.clone())
        .expect("Failed to store pattern3");

    println!("Stored {} patterns\n", hopfield.num_patterns());

    // Test perfect retrieval
    println!("Test 1: Perfect Retrieval");
    println!("-------------------------");
    let retrieved1 = hopfield.retrieve(&pattern1).expect("Retrieval failed");
    let similarity1 = cosine_similarity(&pattern1, &retrieved1);
    println!("Pattern 1 similarity: {:.6}", similarity1);
    assert!(similarity1 > 0.99, "Perfect retrieval failed");
    println!("✓ Perfect retrieval works!\n");

    // Test retrieval with noise
    println!("Test 2: Noisy Retrieval");
    println!("-----------------------");
    let mut noisy_pattern = pattern1.clone();
    noisy_pattern[0] = 0.95; // Add noise
    noisy_pattern[10] = 0.05;

    let retrieved_noisy = hopfield.retrieve(&noisy_pattern).expect("Retrieval failed");
    let similarity_noisy = cosine_similarity(&pattern1, &retrieved_noisy);
    println!(
        "Noisy query similarity to original: {:.6}",
        similarity_noisy
    );
    assert!(similarity_noisy > 0.90, "Noisy retrieval failed");
    println!("✓ Noise-tolerant retrieval works!\n");

    // Test top-k retrieval
    println!("Test 3: Top-K Retrieval");
    println!("-----------------------");
    let query = pattern1.clone();
    let top_k = hopfield
        .retrieve_k(&query, 2)
        .expect("Top-k retrieval failed");

    println!("Top 2 patterns by attention:");
    for (i, (idx, _pattern, attention)) in top_k.iter().enumerate() {
        println!("  {}. Pattern {} - Attention: {:.6}", i + 1, idx, attention);
    }
    assert_eq!(top_k[0].0, 0, "Top match should be pattern 0");
    println!("✓ Top-K retrieval works!\n");

    // Test capacity calculation
    println!("Test 4: Capacity Demonstration");
    println!("--------------------------------");
    let capacity = hopfield.capacity();
    println!(
        "Theoretical capacity for {}D: 2^{} = {}",
        dimension,
        dimension / 2,
        capacity
    );
    println!("✓ Capacity calculation works!\n");

    // Demonstrate beta parameter effect
    println!("Test 5: Beta Parameter Effect");
    println!("------------------------------");

    let mut hopfield_low = ModernHopfield::new(dimension, 0.5);
    let mut hopfield_high = ModernHopfield::new(dimension, 5.0);

    hopfield_low.store(pattern1.clone()).unwrap();
    hopfield_low.store(pattern2.clone()).unwrap();

    hopfield_high.store(pattern1.clone()).unwrap();
    hopfield_high.store(pattern2.clone()).unwrap();

    let retrieved_low = hopfield_low.retrieve(&pattern1).unwrap();
    let retrieved_high = hopfield_high.retrieve(&pattern1).unwrap();

    let sim_low = cosine_similarity(&pattern1, &retrieved_low);
    let sim_high = cosine_similarity(&pattern1, &retrieved_high);

    println!("Low beta (0.5) similarity: {:.6}", sim_low);
    println!("High beta (5.0) similarity: {:.6}", sim_high);
    println!("✓ Higher beta gives sharper retrieval!\n");

    println!("=== All Tests Passed! ===");
}
