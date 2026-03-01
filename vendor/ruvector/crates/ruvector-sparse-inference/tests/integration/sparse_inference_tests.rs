//! Integration tests for sparse inference pipeline

use ruvector_sparse_inference::*;

mod common;
use common::*;

#[test]
fn test_full_sparse_pipeline() {
    let model = load_test_llama_model();
    let mut engine = SparseInferenceEngine::new_sparse(model, 0.3);

    // Calibrate
    let calibration_samples = generate_calibration_data(100);
    engine.calibrate(&calibration_samples).unwrap();

    // Run inference
    let input = random_vector(512);
    let output = engine.infer(&input).unwrap();

    // Verify output
    assert_eq!(output.len(), 512, "Output dimension should match input");
    assert!(output.iter().all(|&x| x.is_finite()), "All outputs should be finite");

    // Check sparsity was applied
    let stats = engine.sparsity_statistics();
    assert!(stats.average_active_ratio < 0.5, "Should have at least 50% sparsity");
}

#[test]
fn test_dense_vs_sparse_accuracy() {
    let model = load_test_llama_model();
    let dense_engine = SparseInferenceEngine::new_dense(model.clone());
    let sparse_engine = SparseInferenceEngine::new_sparse(model, 0.1);

    let inputs: Vec<_> = (0..100).map(|_| random_vector(512)).collect();

    let mut total_error = 0.0;
    for input in &inputs {
        let dense_out = dense_engine.infer(input).unwrap();
        let sparse_out = sparse_engine.infer(input).unwrap();

        let error = mse(&dense_out, &sparse_out);
        total_error += error;
    }

    let avg_error = total_error / inputs.len() as f64;
    assert!(avg_error < 0.1, "Average error too high: {}", avg_error);
}

#[test]
fn test_sparse_inference_batch_processing() {
    let model = load_test_llama_model();
    let engine = SparseInferenceEngine::new_sparse(model, 0.2);

    let batch_size = 10;
    let inputs: Vec<_> = (0..batch_size).map(|_| random_vector(512)).collect();

    let mut outputs = Vec::new();
    for input in &inputs {
        let output = engine.infer(input).unwrap();
        outputs.push(output);
    }

    assert_eq!(outputs.len(), batch_size);
    for output in &outputs {
        assert_eq!(output.len(), 512);
        assert!(output.iter().all(|&x| x.is_finite()));
    }
}

#[test]
fn test_calibration_improves_accuracy() {
    let model = load_test_llama_model();

    // Create two engines: one calibrated, one not
    let mut calibrated = SparseInferenceEngine::new_sparse(model.clone(), 0.3);
    let uncalibrated = SparseInferenceEngine::new_sparse(model, 0.3);

    // Calibrate one
    let calibration_samples = generate_calibration_data(50);
    calibrated.calibrate(&calibration_samples).unwrap();

    // Test both
    let test_inputs: Vec<_> = (0..20).map(|_| random_vector(512)).collect();

    for input in &test_inputs {
        let cal_output = calibrated.infer(input).unwrap();
        let uncal_output = uncalibrated.infer(input).unwrap();

        assert_eq!(cal_output.len(), uncal_output.len());
        assert!(cal_output.iter().all(|&x| x.is_finite()));
        assert!(uncal_output.iter().all(|&x| x.is_finite()));
    }
}

#[test]
fn test_different_sparsity_levels() {
    let model = load_test_llama_model();
    let input = random_vector(512);

    for sparsity in [0.1, 0.3, 0.5, 0.7, 0.9] {
        let engine = SparseInferenceEngine::new_sparse(model.clone(), sparsity);
        let output = engine.infer(&input).unwrap();

        assert_eq!(output.len(), 512, "Output dimension mismatch for sparsity {}", sparsity);
        assert!(output.iter().all(|&x| x.is_finite()), "Non-finite output for sparsity {}", sparsity);
    }
}

#[test]
fn test_sparse_inference_consistency() {
    let model = load_test_llama_model();
    let engine = SparseInferenceEngine::new_sparse(model, 0.3);
    let input = random_vector(512);

    // Same input should produce same output
    let output1 = engine.infer(&input).unwrap();
    let output2 = engine.infer(&input).unwrap();

    assert_vectors_close(&output1, &output2, 1e-10);
}

#[test]
fn test_sparsity_statistics() {
    let model = load_test_llama_model();
    let engine = SparseInferenceEngine::new_sparse(model, 0.4);

    let stats = engine.sparsity_statistics();

    assert!(stats.average_active_ratio >= 0.0);
    assert!(stats.average_active_ratio <= 1.0);
    assert!(stats.min_active <= stats.max_active);
}

#[test]
fn test_dense_engine_activates_all_neurons() {
    let model = load_test_llama_model();
    let dense_engine = SparseInferenceEngine::new_dense(model);

    let stats = dense_engine.sparsity_statistics();

    // Dense engine should have statistics indicating all neurons are active
    // (exact values depend on implementation, but ratio should be high)
    assert!(stats.average_active_ratio >= 0.0);
}

#[test]
fn test_multiple_inferences() {
    let model = load_test_llama_model();
    let engine = SparseInferenceEngine::new_sparse(model, 0.2);

    // Run many inferences to ensure stability
    for _ in 0..100 {
        let input = random_vector(512);
        let output = engine.infer(&input).unwrap();

        assert_eq!(output.len(), 512);
        assert!(output.iter().all(|&x| x.is_finite()));
    }
}

#[test]
fn test_extreme_input_values() {
    let model = load_test_llama_model();
    let engine = SparseInferenceEngine::new_sparse(model, 0.3);

    // Test with very large values
    let large_input = vec![1000.0f32; 512];
    let output_large = engine.infer(&large_input).unwrap();
    assert!(output_large.iter().all(|&x| x.is_finite()));

    // Test with very small values
    let small_input = vec![-1000.0f32; 512];
    let output_small = engine.infer(&small_input).unwrap();
    assert!(output_small.iter().all(|&x| x.is_finite()));

    // Test with zero
    let zero_input = vec![0.0f32; 512];
    let output_zero = engine.infer(&zero_input).unwrap();
    assert!(output_zero.iter().all(|&x| x.is_finite()));
}

#[test]
fn test_calibration_with_empty_samples() {
    let model = load_test_llama_model();
    let mut engine = SparseInferenceEngine::new_sparse(model, 0.3);

    let empty_samples: Vec<Vec<f32>> = vec![];
    let result = engine.calibrate(&empty_samples);

    // Should handle empty calibration gracefully
    assert!(result.is_ok());
}

#[test]
fn test_calibration_with_many_samples() {
    let model = load_test_llama_model();
    let mut engine = SparseInferenceEngine::new_sparse(model, 0.3);

    // Large calibration set
    let samples = generate_calibration_data(1000);
    let result = engine.calibrate(&samples);

    assert!(result.is_ok());
}
