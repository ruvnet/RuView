//! Common test utilities for sparse inference tests

use rand::Rng;
use ruvector_sparse_inference::*;

/// Generate a random vector of given dimension
pub fn random_vector(dim: usize) -> Vec<f32> {
    let mut rng = rand::thread_rng();
    (0..dim).map(|_| rng.gen_range(-1.0..1.0)).collect()
}

/// Generate random activations (neuron indices)
pub fn random_activations(max_neurons: usize) -> Vec<usize> {
    let mut rng = rand::thread_rng();
    let num_active = rng.gen_range(max_neurons / 4..max_neurons / 2);

    let mut activations: Vec<usize> = (0..max_neurons).collect();
    activations.truncate(num_active);
    activations
}

/// Create a test FFN with known dimensions
pub fn create_test_ffn(input_dim: usize, hidden_dim: usize) -> sparse::SparseFfn {
    sparse::SparseFfn::new(input_dim, hidden_dim, sparse::ActivationType::Silu)
}

/// Create a calibrated predictor for testing
pub fn create_calibrated_predictor() -> predictor::LowRankPredictor {
    let mut predictor = predictor::LowRankPredictor::new(512, 4096, 128, 0.1);

    // Generate some calibration data
    let samples: Vec<Vec<f32>> = (0..50)
        .map(|_| random_vector(512))
        .collect();

    let activations: Vec<Vec<usize>> = (0..50)
        .map(|_| random_activations(4096))
        .collect();

    predictor.calibrate(&samples, &activations);
    predictor
}

/// Create a quantized matrix for testing
pub fn create_quantized_matrix(rows: usize, cols: usize) -> memory::quantization::QuantizedWeights {
    let data: Vec<f32> = (0..rows * cols)
        .map(|i| (i as f32) * 0.01)
        .collect();

    memory::quantization::QuantizedWeights::quantize_int8(&data)
}

/// Create a test LLaMA model
pub fn load_test_llama_model() -> model::LlamaModel {
    model::LlamaModel::new(512, 2048, 4, 32000)
}

/// Create a test model for benchmarks
pub fn load_benchmark_model() -> model::LlamaModel {
    model::LlamaModel::new(512, 2048, 4, 32000)
}

/// Create a mock GGUF header
pub fn create_mock_gguf_header() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&0x46554747u32.to_le_bytes()); // "GGUF" magic
    data.extend_from_slice(&3u32.to_le_bytes()); // version 3
    data.extend_from_slice(&0u64.to_le_bytes()); // tensor count
    data.extend_from_slice(&0u64.to_le_bytes()); // metadata kv count
    data
}

/// Assert two vectors are close within tolerance
pub fn assert_vectors_close(a: &[f32], b: &[f32], tolerance: f32) {
    assert_eq!(a.len(), b.len(), "Vector lengths don't match");
    for (i, (&x, &y)) in a.iter().zip(b.iter()).enumerate() {
        let diff = (x - y).abs();
        assert!(
            diff < tolerance,
            "Vectors differ at index {}: {} vs {} (diff: {})",
            i, x, y, diff
        );
    }
}

/// Calculate mean squared error between two vectors
pub fn mse(a: &[f32], b: &[f32]) -> f64 {
    assert_eq!(a.len(), b.len(), "Vector lengths don't match");

    let sum: f64 = a.iter()
        .zip(b.iter())
        .map(|(&x, &y)| {
            let diff = (x - y) as f64;
            diff * diff
        })
        .sum();

    sum / a.len() as f64
}

/// Generate calibration data for testing
pub fn generate_calibration_data(num_samples: usize) -> Vec<Vec<f32>> {
    (0..num_samples)
        .map(|_| random_vector(512))
        .collect()
}
