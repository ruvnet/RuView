//! Unit tests for neuron predictors

use ruvector_sparse_inference::predictor::*;

mod common;
use common::*;

#[test]
fn test_lowrank_predictor_creation() {
    let predictor = LowRankPredictor::new(512, 4096, 128, 0.1);
    assert_eq!(predictor.input_dim(), 512);
    assert_eq!(predictor.hidden_dim(), 4096);
    assert_eq!(predictor.rank(), 128);
}

#[test]
fn test_predictor_predicts_active_neurons() {
    let predictor = create_calibrated_predictor();
    let input = vec![0.1f32; 512];

    let active = predictor.predict(&input);

    // Should predict some neurons as active
    assert!(!active.is_empty(), "Predictor should activate some neurons");
    // Should predict fewer than total neurons (sparsity)
    assert!(active.len() < 4096, "Predictor should be sparse");
    // All indices should be valid
    assert!(active.iter().all(|&i| i < 4096), "All indices should be valid");
}

#[test]
fn test_predictor_top_k_mode() {
    let mut predictor = LowRankPredictor::new(512, 4096, 128, 0.0);
    predictor.set_top_k(Some(100));

    let input = vec![0.1f32; 512];
    let active = predictor.predict(&input);

    assert_eq!(active.len(), 100, "Top-K should return exactly K neurons");
}

#[test]
fn test_predictor_top_k_larger_than_hidden() {
    let mut predictor = LowRankPredictor::new(512, 100, 64, 0.0);
    predictor.set_top_k(Some(200)); // More than hidden_dim

    let input = random_vector(512);
    let active = predictor.predict(&input);

    // Should return at most hidden_dim neurons
    assert!(active.len() <= 100);
}

#[test]
fn test_predictor_calibration() {
    let mut predictor = LowRankPredictor::new(512, 4096, 128, 0.5);

    // Initial threshold
    let initial_threshold = 0.5;

    // Generate calibration data
    let samples: Vec<_> = (0..100)
        .map(|_| random_vector(512))
        .collect();

    let activations: Vec<_> = (0..100)
        .map(|_| {
            // Simulate 30% activation rate
            let num_active = (4096 as f32 * 0.3) as usize;
            (0..num_active).collect::<Vec<_>>()
        })
        .collect();

    predictor.calibrate(&samples, &activations);

    // After calibration, predictor should make better predictions
    let test_input = random_vector(512);
    let active = predictor.predict(&test_input);
    assert!(!active.is_empty(), "Calibrated predictor should activate neurons");
}

#[test]
fn test_predictor_different_inputs_different_outputs() {
    let predictor = LowRankPredictor::new(512, 4096, 128, 0.1);

    let input1 = random_vector(512);
    let input2 = random_vector(512);

    let active1 = predictor.predict(&input1);
    let active2 = predictor.predict(&input2);

    // Different inputs should generally produce different activations
    // (This test might occasionally fail due to randomness, but should pass most of the time)
    assert_ne!(active1, active2, "Different inputs should produce different activations");
}

#[test]
fn test_dense_predictor_activates_all() {
    let predictor = DensePredictor::new(4096);
    let input = random_vector(512);

    let active = predictor.predict(&input);

    assert_eq!(active.len(), 4096, "Dense predictor should activate all neurons");
    assert_eq!(active, (0..4096).collect::<Vec<_>>(), "Should be sequential indices");
}

#[test]
fn test_dense_predictor_num_neurons() {
    let predictor = DensePredictor::new(2048);
    assert_eq!(predictor.num_neurons(), 2048);
}

#[test]
#[should_panic(expected = "Input dimension mismatch")]
fn test_predictor_wrong_input_dimension() {
    let predictor = LowRankPredictor::new(512, 4096, 128, 0.1);
    let wrong_input = vec![0.1f32; 256]; // Wrong dimension

    predictor.predict(&wrong_input);
}

#[test]
fn test_predictor_zero_input() {
    let predictor = LowRankPredictor::new(512, 4096, 128, 0.1);
    let zero_input = vec![0.0f32; 512];

    let active = predictor.predict(&zero_input);

    // Zero input should still produce some output (might be threshold-dependent)
    assert!(active.len() <= 4096, "Should not exceed total neurons");
}

#[test]
fn test_predictor_extreme_values() {
    let predictor = LowRankPredictor::new(512, 4096, 128, 0.1);

    // Test with very large values
    let large_input = vec![1000.0f32; 512];
    let active_large = predictor.predict(&large_input);
    assert!(active_large.iter().all(|&i| i < 4096));

    // Test with very small values
    let small_input = vec![-1000.0f32; 512];
    let active_small = predictor.predict(&small_input);
    assert!(active_small.iter().all(|&i| i < 4096));
}

#[test]
fn test_predictor_consistent_predictions() {
    let predictor = LowRankPredictor::new(512, 4096, 128, 0.1);
    let input = random_vector(512);

    // Same input should produce same output
    let active1 = predictor.predict(&input);
    let active2 = predictor.predict(&input);

    assert_eq!(active1, active2, "Same input should produce same output");
}
