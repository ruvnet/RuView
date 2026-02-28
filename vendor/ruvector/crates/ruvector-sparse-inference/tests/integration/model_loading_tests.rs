//! Integration tests for model loading

use ruvector_sparse_inference::model::*;

mod common;
use common::*;

#[test]
fn test_gguf_header_parsing() {
    let mock_gguf = create_mock_gguf_header();
    let header = GgufParser::parse_header(&mock_gguf).unwrap();

    assert_eq!(header.magic, 0x46554747); // "GGUF"
    assert_eq!(header.version, 3);
}

#[test]
fn test_gguf_invalid_magic() {
    let mut invalid_gguf = vec![0u8; 8];
    invalid_gguf[0..4].copy_from_slice(&0x12345678u32.to_le_bytes()); // Wrong magic
    invalid_gguf[4..8].copy_from_slice(&3u32.to_le_bytes());

    let result = GgufParser::parse_header(&invalid_gguf);
    assert!(result.is_err(), "Should fail with invalid magic number");
}

#[test]
fn test_gguf_too_small() {
    let tiny_data = vec![0u8; 4]; // Too small
    let result = GgufParser::parse_header(&tiny_data);
    assert!(result.is_err(), "Should fail with too small data");
}

#[test]
fn test_llama_model_structure() {
    let model = load_test_llama_model();

    assert!(model.metadata().hidden_size > 0);
    assert!(model.layers.len() > 0);
    assert!(model.embed_tokens.vocab_size() > 0);
}

#[test]
fn test_llama_model_dimensions() {
    let model = load_test_llama_model();

    assert_eq!(model.hidden_size(), 512);
    assert_eq!(model.intermediate_size(), 2048);
    assert_eq!(model.layers.len(), 4);
    assert_eq!(model.embed_tokens.vocab_size(), 32000);
}

#[test]
fn test_model_forward_pass() {
    let model = load_test_llama_model();
    let input = ModelInput::TokenIds(vec![1, 2, 3, 4, 5]);
    let config = InferenceConfig::default();

    let output = model.forward(&input, &config).unwrap();

    assert!(!output.logits.is_empty());
    assert_eq!(output.logits.len(), model.embed_tokens.vocab_size());
}

#[test]
fn test_model_forward_with_embeddings() {
    let model = load_test_llama_model();
    let embeddings = vec![
        random_vector(512),
        random_vector(512),
        random_vector(512),
    ];
    let input = ModelInput::Embeddings(embeddings);
    let config = InferenceConfig::default();

    let output = model.forward(&input, &config).unwrap();
    assert!(!output.logits.is_empty());
}

#[test]
fn test_inference_config_default() {
    let config = InferenceConfig::default();

    assert_eq!(config.temperature, 1.0);
    assert_eq!(config.top_k, None);
    assert_eq!(config.top_p, None);
}

#[test]
fn test_inference_config_custom() {
    let config = InferenceConfig {
        temperature: 0.8,
        top_k: Some(50),
        top_p: Some(0.95),
    };

    assert_eq!(config.temperature, 0.8);
    assert_eq!(config.top_k, Some(50));
    assert_eq!(config.top_p, Some(0.95));
}

#[test]
fn test_model_metadata_access() {
    let model = load_test_llama_model();
    let metadata = model.metadata();

    assert_eq!(metadata.hidden_size(), 512);
    assert_eq!(metadata.hidden_size, 512);
    assert_eq!(metadata.intermediate_size, 2048);
    assert_eq!(metadata.num_layers, 4);
    assert_eq!(metadata.vocab_size, 32000);
}

#[test]
fn test_embed_tokens_vocab_size() {
    let embed = EmbedTokens::new(50000, 768);
    assert_eq!(embed.vocab_size(), 50000);
}

#[test]
fn test_transformer_layer_indices() {
    let model = load_test_llama_model();

    for (i, layer) in model.layers.iter().enumerate() {
        assert_eq!(layer.layer_idx, i, "Layer index should match position");
    }
}

#[test]
fn test_model_creation_various_sizes() {
    // Test different model sizes
    let small = LlamaModel::new(256, 1024, 2, 10000);
    assert_eq!(small.hidden_size(), 256);
    assert_eq!(small.layers.len(), 2);

    let large = LlamaModel::new(2048, 8192, 32, 100000);
    assert_eq!(large.hidden_size(), 2048);
    assert_eq!(large.layers.len(), 32);
}

#[test]
fn test_gguf_header_version() {
    let mut data = create_mock_gguf_header();

    // Modify version
    data[4..8].copy_from_slice(&2u32.to_le_bytes());

    let header = GgufParser::parse_header(&data).unwrap();
    assert_eq!(header.version, 2);
}

#[test]
fn test_model_forward_deterministic() {
    let model = load_test_llama_model();
    let input = ModelInput::TokenIds(vec![1, 2, 3]);
    let config = InferenceConfig::default();

    let output1 = model.forward(&input, &config).unwrap();
    let output2 = model.forward(&input, &config).unwrap();

    // Same input should produce same output
    assert_eq!(output1.logits.len(), output2.logits.len());
    for (a, b) in output1.logits.iter().zip(output2.logits.iter()) {
        assert_eq!(a, b);
    }
}
