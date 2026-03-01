#![cfg(target_arch = "wasm32")]

use ruvector_sparse_inference_wasm::*;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

/// Create a minimal mock GGUF model for testing
fn create_mock_model() -> Vec<u8> {
    // Minimal GGUF header + metadata
    // Magic: "GGUF" (0x46554747)
    // Version: 3
    // Minimal tensors and metadata
    let mut bytes = Vec::new();

    // Magic number (GGUF in little-endian)
    bytes.extend_from_slice(&[0x47, 0x47, 0x55, 0x46]); // "GGUF"

    // Version (u32)
    bytes.extend_from_slice(&3u32.to_le_bytes());

    // Tensor count (u64) - 0 tensors for minimal test
    bytes.extend_from_slice(&0u64.to_le_bytes());

    // Metadata count (u64) - minimal metadata
    bytes.extend_from_slice(&4u64.to_le_bytes());

    // Add minimal required metadata fields
    // This is a simplified version - real GGUF has complex structure

    bytes
}

fn create_test_engine() -> SparseInferenceEngine {
    let model_bytes = create_mock_model();
    let config = r#"{
        "sparsity": {
            "enabled": true,
            "threshold": 0.1
        },
        "temperature": 1.0,
        "top_k": 50
    }"#;

    SparseInferenceEngine::new(&model_bytes, config).expect("Failed to create test engine")
}

#[wasm_bindgen_test]
fn test_version() {
    let ver = version();
    assert!(!ver.is_empty());
    assert!(ver.contains('.'));
}

#[wasm_bindgen_test]
fn test_init() {
    // Just ensure init doesn't panic
    init();
}

#[wasm_bindgen_test]
fn test_engine_creation_with_invalid_config() {
    let model_bytes = create_mock_model();
    let bad_config = "not json";

    let result = SparseInferenceEngine::new(&model_bytes, bad_config);
    assert!(result.is_err());
}

#[wasm_bindgen_test]
fn test_engine_metadata() {
    let engine = create_test_engine();
    let metadata = engine.metadata();

    // Should return valid JSON string
    assert!(!metadata.is_empty());
}

#[wasm_bindgen_test]
fn test_sparsity_stats() {
    let engine = create_test_engine();
    let stats = engine.sparsity_stats();

    // Should return valid JSON string
    assert!(!stats.is_empty());
}

#[wasm_bindgen_test]
fn test_sparsity_adjustment() {
    let mut engine = create_test_engine();

    // Should not panic
    engine.set_sparsity(0.5);

    let stats = engine.sparsity_stats();
    assert!(!stats.is_empty());
}

#[wasm_bindgen_test]
fn test_embedding_model_creation() {
    let model_bytes = create_mock_model();

    let result = EmbeddingModel::new(&model_bytes);
    // May fail with mock model, but shouldn't panic
    let _ = result;
}

#[wasm_bindgen_test]
fn test_llm_model_creation() {
    let model_bytes = create_mock_model();
    let config = r#"{
        "sparsity": {"enabled": true, "threshold": 0.1},
        "temperature": 0.7,
        "top_k": 40
    }"#;

    let result = LLMModel::new(&model_bytes, config);
    // May fail with mock model, but shouldn't panic
    let _ = result;
}

#[wasm_bindgen_test]
fn test_calibrate_with_empty_samples() {
    let mut engine = create_test_engine();
    let samples: Vec<f32> = vec![];

    let result = engine.calibrate(&samples, 512);
    // Should handle gracefully
    let _ = result;
}

#[wasm_bindgen_test]
fn test_measure_inference_time() {
    let engine = create_test_engine();
    let input = vec![0.1f32; 512];

    // Should not panic even if inference fails
    let time = measure_inference_time(&engine, &input, 1);
    assert!(time >= 0.0);
}

// Integration tests with actual model would go here
// These require a real GGUF model file which we don't have in tests

#[wasm_bindgen_test]
async fn test_load_streaming_with_bad_url() {
    let config = r#"{"sparsity": {"enabled": true}}"#;
    let result =
        SparseInferenceEngine::load_streaming("https://invalid.example.com/model.gguf", config)
            .await;

    // Should fail gracefully
    assert!(result.is_err());
}
