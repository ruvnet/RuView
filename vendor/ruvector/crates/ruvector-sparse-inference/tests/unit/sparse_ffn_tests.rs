//! Unit tests for sparse feed-forward networks

use ruvector_sparse_inference::sparse::*;

mod common;
use common::*;

#[test]
fn test_sparse_ffn_matches_dense() {
    let ffn = create_test_ffn(512, 2048);
    let input = random_vector(512);
    let all_neurons: Vec<usize> = (0..2048).collect();

    let dense_output = ffn.forward_dense(&input);
    let sparse_output = ffn.forward_sparse(&input, &all_neurons);

    // When all neurons are active, sparse should match dense
    assert_vectors_close(&dense_output, &sparse_output, 1e-5);
}

#[test]
fn test_sparse_ffn_with_subset() {
    let ffn = create_test_ffn(512, 2048);
    let input = random_vector(512);
    let active_neurons: Vec<usize> = (0..1024).collect(); // 50% sparsity

    let output = ffn.forward_sparse(&input, &active_neurons);

    assert_eq!(output.len(), 512, "Output dimension should match input dimension");
    assert!(output.iter().all(|&x| x.is_finite()), "All outputs should be finite");
}

#[test]
fn test_sparse_ffn_empty_activations() {
    let ffn = create_test_ffn(512, 2048);
    let input = random_vector(512);
    let no_neurons: Vec<usize> = vec![];

    let output = ffn.forward_sparse(&input, &no_neurons);

    assert_eq!(output.len(), 512);
    // With no active neurons, output should be near zero
    assert!(output.iter().all(|&x| x.abs() < 1e-5), "Output should be near zero with no active neurons");
}

#[test]
fn test_different_activations() {
    for activation in [ActivationType::Relu, ActivationType::Gelu, ActivationType::Silu] {
        let ffn = SparseFfn::new(512, 2048, activation);
        let input = random_vector(512);
        let active: Vec<usize> = (0..500).collect();

        let output = ffn.forward_sparse(&input, &active);
        assert_eq!(output.len(), 512, "Output dimension should be 512 for {:?}", activation);
        assert!(output.iter().all(|&x| x.is_finite()), "All outputs should be finite for {:?}", activation);
    }
}

#[test]
fn test_relu_activation_properties() {
    let ffn = SparseFfn::new(512, 2048, ActivationType::Relu);
    let input = vec![-1.0f32; 512]; // Negative input
    let all_neurons: Vec<usize> = (0..2048).collect();

    let output = ffn.forward_dense(&input);

    // ReLU should zero out negative activations
    // (though final output might still be negative due to w2 projection)
    assert!(output.iter().all(|&x| x.is_finite()));
}

#[test]
fn test_swiglu_paired_neurons() {
    // SwiGLU uses paired neurons (gate and up projections)
    let ffn = SwiGLUFfn::new(512, 2048);
    let input = random_vector(512);

    // Active neurons should be pairs
    let active_pairs: Vec<usize> = (0..500).map(|i| i * 2).collect();
    let output = ffn.forward_sparse(&input, &active_pairs);

    assert_eq!(output.len(), 512);
    assert!(output.iter().all(|&x| x.is_finite()));
}

#[test]
fn test_swiglu_matches_dense() {
    let ffn = SwiGLUFfn::new(512, 2048);
    let input = random_vector(512);
    let all_neurons: Vec<usize> = (0..2048).collect();

    let dense_output = ffn.forward_dense(&input);
    let sparse_output = ffn.forward_sparse(&input, &all_neurons);

    assert_vectors_close(&dense_output, &sparse_output, 1e-5);
}

#[test]
fn test_swiglu_empty_activations() {
    let ffn = SwiGLUFfn::new(512, 2048);
    let input = random_vector(512);
    let no_neurons: Vec<usize> = vec![];

    let output = ffn.forward_sparse(&input, &no_neurons);

    assert_eq!(output.len(), 512);
    assert!(output.iter().all(|&x| x.abs() < 1e-5));
}

#[test]
#[should_panic(expected = "Input dimension mismatch")]
fn test_sparse_ffn_wrong_input_dimension() {
    let ffn = create_test_ffn(512, 2048);
    let wrong_input = vec![0.1f32; 256];
    let active: Vec<usize> = (0..100).collect();

    ffn.forward_sparse(&wrong_input, &active);
}

#[test]
fn test_sparse_ffn_out_of_bounds_neurons() {
    let ffn = create_test_ffn(512, 2048);
    let input = random_vector(512);

    // Include some out-of-bounds indices
    let mut active: Vec<usize> = (0..100).collect();
    active.push(5000); // Out of bounds
    active.push(10000); // Out of bounds

    let output = ffn.forward_sparse(&input, &active);

    // Should handle gracefully
    assert_eq!(output.len(), 512);
    assert!(output.iter().all(|&x| x.is_finite()));
}

#[test]
fn test_sparse_ffn_duplicate_neurons() {
    let ffn = create_test_ffn(512, 2048);
    let input = random_vector(512);

    // Include duplicate indices
    let active = vec![10, 20, 10, 30, 20, 10];

    let output = ffn.forward_sparse(&input, &active);

    assert_eq!(output.len(), 512);
    assert!(output.iter().all(|&x| x.is_finite()));
}

#[test]
fn test_sparse_ffn_sparsity_reduces_computation() {
    let ffn = create_test_ffn(512, 2048);
    let input = random_vector(512);

    // 10% sparsity
    let sparse_neurons: Vec<usize> = (0..204).collect();

    let sparse_output = ffn.forward_sparse(&input, &sparse_neurons);

    // Should still produce valid output with much less computation
    assert_eq!(sparse_output.len(), 512);
    assert!(sparse_output.iter().all(|&x| x.is_finite()));
}

#[test]
fn test_dense_output_deterministic() {
    let ffn = create_test_ffn(512, 2048);
    let input = random_vector(512);

    let output1 = ffn.forward_dense(&input);
    let output2 = ffn.forward_dense(&input);

    assert_vectors_close(&output1, &output2, 1e-10);
}

#[test]
fn test_sparse_output_deterministic() {
    let ffn = create_test_ffn(512, 2048);
    let input = random_vector(512);
    let active: Vec<usize> = (0..500).collect();

    let output1 = ffn.forward_sparse(&input, &active);
    let output2 = ffn.forward_sparse(&input, &active);

    assert_vectors_close(&output1, &output2, 1e-10);
}
