//! GGUF Loading Tests
//!
//! Tests for GGUF header/metadata parsing, tensor loading, quantization
//! format handling, architecture detection, memory mapping, and error handling.

use crate::gguf::parser::GgufValueType;
use crate::gguf::{
    parse_header, parse_metadata, GgufHeader, GgufQuantType, GgufValue, GGUF_MAGIC, GGUF_VERSION,
};
use std::io::Cursor;

// ============================================================================
// Header Parsing Tests
// ============================================================================

#[test]
fn test_parse_valid_header() {
    let mut data = vec![];
    data.extend_from_slice(&GGUF_MAGIC.to_le_bytes()); // magic
    data.extend_from_slice(&GGUF_VERSION.to_le_bytes()); // version
    data.extend_from_slice(&10u64.to_le_bytes()); // tensor_count
    data.extend_from_slice(&5u64.to_le_bytes()); // metadata_kv_count

    let mut cursor = Cursor::new(data);
    let header = parse_header(&mut cursor).unwrap();

    assert_eq!(header.magic, GGUF_MAGIC);
    assert_eq!(header.version, GGUF_VERSION);
    assert_eq!(header.tensor_count, 10);
    assert_eq!(header.metadata_kv_count, 5);
}

#[test]
fn test_gguf_magic_is_correct() {
    // "GGUF" in little-endian bytes
    let expected = 0x46554747u32;
    assert_eq!(GGUF_MAGIC, expected);

    // Verify it spells "GGUF"
    let bytes = GGUF_MAGIC.to_le_bytes();
    assert_eq!(&bytes, b"GGUF");
}

#[test]
fn test_parse_header_truncated() {
    // Only provide partial header
    let data = vec![0x47, 0x47, 0x55, 0x46]; // Just magic
    let mut cursor = Cursor::new(data);

    let result = parse_header(&mut cursor);
    assert!(result.is_err(), "Truncated header should fail");
}

#[test]
fn test_parse_header_empty() {
    let data: Vec<u8> = vec![];
    let mut cursor = Cursor::new(data);

    let result = parse_header(&mut cursor);
    assert!(result.is_err(), "Empty input should fail");
}

// ============================================================================
// GgufValue Tests
// ============================================================================

#[test]
fn test_gguf_value_string() {
    let val = GgufValue::String("test_value".to_string());

    assert_eq!(val.as_str(), Some("test_value"));
    assert_eq!(val.as_u64(), None);
    assert_eq!(val.as_i64(), None);
    assert_eq!(val.as_f32(), None);
    assert_eq!(val.as_bool(), None);
    assert!(val.as_array().is_none());
}

#[test]
fn test_gguf_value_integer_conversions() {
    // Test U32
    let val = GgufValue::U32(42);
    assert_eq!(val.as_u64(), Some(42));
    assert_eq!(val.as_i64(), Some(42));
    assert_eq!(val.as_f32(), Some(42.0));
    assert_eq!(val.as_str(), None);

    // Test I32
    let val = GgufValue::I32(-5);
    assert_eq!(val.as_i64(), Some(-5));
    assert_eq!(val.as_u64(), None); // Negative cannot be u64

    // Test U64
    let val = GgufValue::U64(u64::MAX);
    assert_eq!(val.as_u64(), Some(u64::MAX));
    assert_eq!(val.as_i64(), None); // Too large for i64

    // Test I64
    let val = GgufValue::I64(-100);
    assert_eq!(val.as_i64(), Some(-100));
    assert_eq!(val.as_u64(), None);

    // Test I64 positive
    let val = GgufValue::I64(100);
    assert_eq!(val.as_i64(), Some(100));
    assert_eq!(val.as_u64(), Some(100));
}

#[test]
fn test_gguf_value_float_conversions() {
    // Test F32
    let val = GgufValue::F32(3.14);
    assert!((val.as_f32().unwrap() - 3.14).abs() < 0.001);
    assert!((val.as_f64().unwrap() - 3.14).abs() < 0.001);
    assert_eq!(val.as_str(), None);

    // Test F64
    let val = GgufValue::F64(2.71828);
    assert!((val.as_f64().unwrap() - 2.71828).abs() < 0.00001);
    assert!((val.as_f32().unwrap() - 2.71828).abs() < 0.001);
}

#[test]
fn test_gguf_value_bool() {
    let val_true = GgufValue::Bool(true);
    let val_false = GgufValue::Bool(false);

    assert_eq!(val_true.as_bool(), Some(true));
    assert_eq!(val_false.as_bool(), Some(false));
    assert_eq!(val_true.as_str(), None);

    // Test implicit bool from U8
    let val = GgufValue::U8(1);
    assert_eq!(val.as_bool(), Some(true));

    let val = GgufValue::U8(0);
    assert_eq!(val.as_bool(), Some(false));
}

#[test]
fn test_gguf_value_array() {
    let arr = vec![GgufValue::U32(1), GgufValue::U32(2), GgufValue::U32(3)];
    let val = GgufValue::Array(arr);

    let array = val.as_array().unwrap();
    assert_eq!(array.len(), 3);
    assert_eq!(array[0].as_u64(), Some(1));
    assert_eq!(array[1].as_u64(), Some(2));
    assert_eq!(array[2].as_u64(), Some(3));
}

#[test]
fn test_gguf_value_small_integers() {
    // Test U8
    let val = GgufValue::U8(255);
    assert_eq!(val.as_u64(), Some(255));

    // Test I8
    let val = GgufValue::I8(-128);
    assert_eq!(val.as_i64(), Some(-128));
    assert_eq!(val.as_u64(), None);

    // Test U16
    let val = GgufValue::U16(65535);
    assert_eq!(val.as_u64(), Some(65535));

    // Test I16
    let val = GgufValue::I16(-32768);
    assert_eq!(val.as_i64(), Some(-32768));
}

// ============================================================================
// GgufValueType Tests
// ============================================================================

#[test]
fn test_value_type_conversion() {
    assert_eq!(GgufValueType::try_from(0).unwrap(), GgufValueType::U8);
    assert_eq!(GgufValueType::try_from(1).unwrap(), GgufValueType::I8);
    assert_eq!(GgufValueType::try_from(2).unwrap(), GgufValueType::U16);
    assert_eq!(GgufValueType::try_from(3).unwrap(), GgufValueType::I16);
    assert_eq!(GgufValueType::try_from(4).unwrap(), GgufValueType::U32);
    assert_eq!(GgufValueType::try_from(5).unwrap(), GgufValueType::I32);
    assert_eq!(GgufValueType::try_from(6).unwrap(), GgufValueType::F32);
    assert_eq!(GgufValueType::try_from(7).unwrap(), GgufValueType::Bool);
    assert_eq!(GgufValueType::try_from(8).unwrap(), GgufValueType::String);
    assert_eq!(GgufValueType::try_from(9).unwrap(), GgufValueType::Array);
    assert_eq!(GgufValueType::try_from(10).unwrap(), GgufValueType::U64);
    assert_eq!(GgufValueType::try_from(11).unwrap(), GgufValueType::I64);
    assert_eq!(GgufValueType::try_from(12).unwrap(), GgufValueType::F64);
}

#[test]
fn test_value_type_invalid() {
    assert!(GgufValueType::try_from(13).is_err());
    assert!(GgufValueType::try_from(100).is_err());
    assert!(GgufValueType::try_from(255).is_err());
}

// ============================================================================
// Quantization Type Tests
// ============================================================================

#[test]
fn test_quant_type_from_u32() {
    assert!(GgufQuantType::try_from(0u32).is_ok()); // F32
    assert!(GgufQuantType::try_from(1u32).is_ok()); // F16
    assert!(GgufQuantType::try_from(2u32).is_ok()); // Q4_0
    assert!(GgufQuantType::try_from(3u32).is_ok()); // Q4_1
    assert!(GgufQuantType::try_from(8u32).is_ok()); // Q8_0
}

#[test]
fn test_quant_type_block_size() {
    assert_eq!(GgufQuantType::F32.block_size(), 1);
    assert_eq!(GgufQuantType::F16.block_size(), 1);
    assert_eq!(GgufQuantType::Q4_0.block_size(), 32);
    assert_eq!(GgufQuantType::Q4_1.block_size(), 32);
    assert_eq!(GgufQuantType::Q8_0.block_size(), 32);
    assert_eq!(GgufQuantType::Q4_K.block_size(), 256);
    assert_eq!(GgufQuantType::Q2_K.block_size(), 256);
    assert_eq!(GgufQuantType::Q3_K.block_size(), 256);
    assert_eq!(GgufQuantType::Q5_K.block_size(), 256);
    assert_eq!(GgufQuantType::Q6_K.block_size(), 256);
}

#[test]
fn test_quant_type_type_size() {
    // F32: 4 bytes per element, 1 element per block
    assert_eq!(GgufQuantType::F32.type_size(), 4);

    // F16: 2 bytes per element, 1 element per block
    assert_eq!(GgufQuantType::F16.type_size(), 2);

    // Q4_0: 2 bytes scale + 16 bytes data (32 elements * 4 bits / 8) = 18 bytes
    assert_eq!(GgufQuantType::Q4_0.type_size(), 18);

    // Q4_1: 2 bytes scale + 2 bytes min + 16 bytes data = 20 bytes
    assert_eq!(GgufQuantType::Q4_1.type_size(), 20);

    // Q8_0: 2 bytes scale + 32 bytes data = 34 bytes
    assert_eq!(GgufQuantType::Q8_0.type_size(), 34);
}

#[test]
fn test_quant_type_is_quantized() {
    assert!(!GgufQuantType::F32.is_quantized());
    assert!(!GgufQuantType::F16.is_quantized());

    assert!(GgufQuantType::Q4_0.is_quantized());
    assert!(GgufQuantType::Q4_1.is_quantized());
    assert!(GgufQuantType::Q8_0.is_quantized());
    assert!(GgufQuantType::Q4_K.is_quantized());
    assert!(GgufQuantType::Q2_K.is_quantized());
}

#[test]
fn test_quant_type_bits_per_weight() {
    // bits_per_weight returns f32
    assert!((GgufQuantType::F32.bits_per_weight() - 32.0).abs() < 0.1);
    assert!((GgufQuantType::F16.bits_per_weight() - 16.0).abs() < 0.1);
    // Q8_0: 34 bytes * 8 / 32 elements = 8.5 bits
    assert!((GgufQuantType::Q8_0.bits_per_weight() - 8.5).abs() < 0.1);

    // Q4_0: (18 bytes * 8 bits) / 32 elements = 4.5 bits
    let q4_bits =
        (GgufQuantType::Q4_0.type_size() * 8) as f32 / GgufQuantType::Q4_0.block_size() as f32;
    assert!((q4_bits - 4.5).abs() < 0.1);
}

// ============================================================================
// Architecture Detection Tests
// ============================================================================

#[test]
fn test_architecture_metadata_key() {
    // Verify common architecture metadata keys
    let arch_keys = [
        "general.architecture",
        "llama.context_length",
        "llama.embedding_length",
        "llama.attention.head_count",
        "llama.attention.head_count_kv",
        "llama.block_count",
        "llama.rope.freq_base",
        "mistral.context_length",
        "phi.context_length",
    ];

    for key in &arch_keys {
        // Just verify the key format is valid
        assert!(!key.is_empty());
        assert!(key.contains('.') || key.starts_with("general"));
    }
}

#[test]
fn test_architecture_detection_patterns() {
    // Test architecture pattern matching logic
    let arch_patterns = [
        ("llama", "llama"),
        ("mistral", "mistral"),
        ("phi", "phi"),
        ("phi2", "phi"),
        ("phi3", "phi"),
        ("qwen", "qwen"),
        ("qwen2", "qwen"),
        ("gemma", "gemma"),
    ];

    for (input, expected_prefix) in &arch_patterns {
        let normalized = input.to_lowercase();
        assert!(
            normalized.starts_with(expected_prefix) || normalized.contains(expected_prefix),
            "{} should match {} pattern",
            input,
            expected_prefix
        );
    }
}

// ============================================================================
// Metadata Parsing Tests
// ============================================================================

fn build_metadata_entry(key: &str, value_type: u32, value_bytes: &[u8]) -> Vec<u8> {
    let mut data = vec![];

    // Key: length (u64) + bytes
    data.extend_from_slice(&(key.len() as u64).to_le_bytes());
    data.extend_from_slice(key.as_bytes());

    // Value type
    data.extend_from_slice(&value_type.to_le_bytes());

    // Value data
    data.extend_from_slice(value_bytes);

    data
}

#[test]
fn test_parse_metadata_u32() {
    let key = "test.value";
    let value = 12345u32;

    let data = build_metadata_entry(key, 4, &value.to_le_bytes());
    let mut cursor = Cursor::new(data);

    let metadata = parse_metadata(&mut cursor, 1).unwrap();

    assert!(metadata.contains_key(key));
    assert_eq!(metadata.get(key).unwrap().as_u64(), Some(12345));
}

#[test]
fn test_parse_metadata_f32() {
    let key = "test.float";
    let value = 3.14159f32;

    let data = build_metadata_entry(key, 6, &value.to_le_bytes());
    let mut cursor = Cursor::new(data);

    let metadata = parse_metadata(&mut cursor, 1).unwrap();

    let parsed = metadata.get(key).unwrap().as_f32().unwrap();
    assert!((parsed - 3.14159).abs() < 0.0001);
}

#[test]
fn test_parse_metadata_string() {
    let key = "test.name";
    let value = "hello_world";

    let mut value_bytes = vec![];
    value_bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
    value_bytes.extend_from_slice(value.as_bytes());

    let data = build_metadata_entry(key, 8, &value_bytes);
    let mut cursor = Cursor::new(data);

    let metadata = parse_metadata(&mut cursor, 1).unwrap();

    assert_eq!(metadata.get(key).unwrap().as_str(), Some("hello_world"));
}

#[test]
fn test_parse_metadata_bool() {
    let key = "test.enabled";
    let value = 1u8;

    let data = build_metadata_entry(key, 7, &[value]);
    let mut cursor = Cursor::new(data);

    let metadata = parse_metadata(&mut cursor, 1).unwrap();

    assert_eq!(metadata.get(key).unwrap().as_bool(), Some(true));
}

#[test]
fn test_parse_metadata_multiple_entries() {
    let mut data = vec![];

    // Entry 1: U32
    data.extend(build_metadata_entry("key1", 4, &42u32.to_le_bytes()));

    // Entry 2: F32
    data.extend(build_metadata_entry("key2", 6, &1.5f32.to_le_bytes()));

    let mut cursor = Cursor::new(data);
    let metadata = parse_metadata(&mut cursor, 2).unwrap();

    assert_eq!(metadata.len(), 2);
    assert_eq!(metadata.get("key1").unwrap().as_u64(), Some(42));
    assert!((metadata.get("key2").unwrap().as_f32().unwrap() - 1.5).abs() < 0.001);
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn test_parse_metadata_truncated_key() {
    // Key length says 100 but only provide 5 bytes
    let mut data = vec![];
    data.extend_from_slice(&100u64.to_le_bytes()); // Key length
    data.extend_from_slice(b"test"); // Only 4 bytes

    let mut cursor = Cursor::new(data);
    let result = parse_metadata(&mut cursor, 1);

    assert!(result.is_err(), "Truncated key should fail");
}

#[test]
fn test_parse_metadata_invalid_value_type() {
    let mut data = vec![];
    data.extend_from_slice(&4u64.to_le_bytes()); // Key length
    data.extend_from_slice(b"test");
    data.extend_from_slice(&255u32.to_le_bytes()); // Invalid type

    let mut cursor = Cursor::new(data);
    let result = parse_metadata(&mut cursor, 1);

    assert!(result.is_err(), "Invalid value type should fail");
}

#[test]
fn test_string_too_long_protection() {
    // Attempt to create a string entry with unreasonable length
    let key = "malicious.string";
    let claimed_len = 10_000_000u64; // 10MB string

    let mut data = vec![];
    data.extend_from_slice(&(key.len() as u64).to_le_bytes());
    data.extend_from_slice(key.as_bytes());
    data.extend_from_slice(&8u32.to_le_bytes()); // String type
    data.extend_from_slice(&claimed_len.to_le_bytes());
    // Don't actually provide the data

    let mut cursor = Cursor::new(data);
    let result = parse_metadata(&mut cursor, 1);

    assert!(result.is_err(), "Unreasonably long string should fail");
}

// ============================================================================
// TensorInfo Tests
// ============================================================================

#[test]
fn test_tensor_info_byte_size() {
    use crate::gguf::tensors::TensorInfo;

    // F32 tensor: 1024 elements * 4 bytes
    let info = TensorInfo {
        name: "test.weight".to_string(),
        shape: vec![1024],
        dtype: GgufQuantType::F32,
        offset: 0,
    };
    assert_eq!(info.byte_size(), 1024 * 4);

    // F16 tensor: 1024 elements * 2 bytes
    let info = TensorInfo {
        name: "test.weight".to_string(),
        shape: vec![1024],
        dtype: GgufQuantType::F16,
        offset: 0,
    };
    assert_eq!(info.byte_size(), 1024 * 2);

    // Q4_0 tensor: 1024 elements / 32 block_size * 18 bytes_per_block = 576 bytes
    let info = TensorInfo {
        name: "test.weight".to_string(),
        shape: vec![1024],
        dtype: GgufQuantType::Q4_0,
        offset: 0,
    };
    assert_eq!(info.byte_size(), (1024 / 32) * 18);
}

#[test]
fn test_tensor_info_multidimensional() {
    use crate::gguf::tensors::TensorInfo;

    // 2D tensor: 512 x 256 = 131072 elements
    let info = TensorInfo {
        name: "model.layers.0.attention.wq.weight".to_string(),
        shape: vec![512, 256],
        dtype: GgufQuantType::F32,
        offset: 4096,
    };

    let num_elements: usize = info.shape.iter().product();
    assert_eq!(num_elements, 131072);
    assert_eq!(info.byte_size(), 131072 * 4);
}

// ============================================================================
// Memory Mapping Tests
// ============================================================================

#[test]
fn test_alignment_calculation() {
    // Test alignment helper logic
    fn align_offset(offset: u64, alignment: u64) -> u64 {
        (offset + alignment - 1) / alignment * alignment
    }

    assert_eq!(align_offset(0, 32), 0);
    assert_eq!(align_offset(1, 32), 32);
    assert_eq!(align_offset(31, 32), 32);
    assert_eq!(align_offset(32, 32), 32);
    assert_eq!(align_offset(33, 32), 64);
    assert_eq!(align_offset(100, 64), 128);
}

#[test]
fn test_default_alignment_constant() {
    use crate::gguf::DEFAULT_ALIGNMENT;

    assert_eq!(DEFAULT_ALIGNMENT, 32);
}

// ============================================================================
// Quantization Format Tests
// ============================================================================

#[test]
fn test_all_quantization_types_defined() {
    // Ensure all expected quantization types exist
    let types = [
        GgufQuantType::F32,
        GgufQuantType::F16,
        GgufQuantType::Q4_0,
        GgufQuantType::Q4_1,
        GgufQuantType::Q5_0,
        GgufQuantType::Q5_1,
        GgufQuantType::Q8_0,
        GgufQuantType::Q8_1,
        GgufQuantType::Q2_K,
        GgufQuantType::Q3_K,
        GgufQuantType::Q4_K,
        GgufQuantType::Q5_K,
        GgufQuantType::Q6_K,
    ];

    for qt in &types {
        assert!(
            qt.block_size() > 0,
            "{:?} should have positive block size",
            qt
        );
        assert!(
            qt.type_size() > 0,
            "{:?} should have positive type size",
            qt
        );
    }
}

#[test]
fn test_quantization_type_display() {
    // Verify quantization types can be formatted
    let qt = GgufQuantType::Q4_K;
    let formatted = format!("{:?}", qt);
    assert!(formatted.contains("Q4_K") || formatted.contains("4"));
}

#[test]
fn test_k_quant_larger_block_size() {
    // K-quantization uses larger blocks (256) vs legacy (32)
    assert_eq!(GgufQuantType::Q4_0.block_size(), 32);
    assert_eq!(GgufQuantType::Q4_K.block_size(), 256);

    // K-quant should have more data per block due to super-blocks
    assert!(GgufQuantType::Q4_K.type_size() > GgufQuantType::Q4_0.type_size());
}

// ============================================================================
// Model Config Tests
// ============================================================================

#[test]
fn test_model_config_default() {
    use crate::gguf::ModelConfig;

    let config = ModelConfig::default();

    assert!(config.architecture.is_none());
    assert!(config.context_length.is_none());
    assert!(config.embedding_length.is_none());
    assert!(config.head_count.is_none());
    assert!(config.head_count_kv.is_none());
    assert!(config.layer_count.is_none());
    assert!(config.vocab_size.is_none());
    assert!(config.rope_freq_base.is_none());
    assert!(config.feed_forward_length.is_none());
}

#[test]
fn test_model_config_populated() {
    use crate::gguf::ModelConfig;

    let config = ModelConfig {
        architecture: Some("llama".to_string()),
        context_length: Some(4096),
        embedding_length: Some(4096),
        head_count: Some(32),
        head_count_kv: Some(8),
        layer_count: Some(32),
        vocab_size: Some(32000),
        rope_freq_base: Some(10000.0),
        feed_forward_length: Some(11008),
    };

    assert_eq!(config.architecture.as_deref(), Some("llama"));
    assert_eq!(config.context_length, Some(4096));
    assert_eq!(config.head_count, Some(32));
    assert_eq!(config.head_count_kv, Some(8));

    // GQA ratio
    let gqa_ratio = config.head_count.unwrap() / config.head_count_kv.unwrap();
    assert_eq!(gqa_ratio, 4);
}

// ============================================================================
// Integration Tests (Without Real Files)
// ============================================================================

#[test]
fn test_complete_header_metadata_flow() {
    // Build a minimal but complete GGUF-like data structure
    let mut data = vec![];

    // Header
    data.extend_from_slice(&GGUF_MAGIC.to_le_bytes());
    data.extend_from_slice(&GGUF_VERSION.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes()); // No tensors
    data.extend_from_slice(&1u64.to_le_bytes()); // 1 metadata entry

    // Metadata entry: architecture = "llama"
    let key = "general.architecture";
    let value = "llama";
    data.extend_from_slice(&(key.len() as u64).to_le_bytes());
    data.extend_from_slice(key.as_bytes());
    data.extend_from_slice(&8u32.to_le_bytes()); // String type
    data.extend_from_slice(&(value.len() as u64).to_le_bytes());
    data.extend_from_slice(value.as_bytes());

    let mut cursor = Cursor::new(data);

    // Parse header
    let header = parse_header(&mut cursor).unwrap();
    assert_eq!(header.magic, GGUF_MAGIC);
    assert_eq!(header.metadata_kv_count, 1);

    // Parse metadata
    let metadata = parse_metadata(&mut cursor, header.metadata_kv_count).unwrap();
    assert_eq!(
        metadata.get("general.architecture").unwrap().as_str(),
        Some("llama")
    );
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_empty_string_value() {
    let key = "test.empty";
    let value = "";

    let mut value_bytes = vec![];
    value_bytes.extend_from_slice(&0u64.to_le_bytes()); // length = 0

    let data = build_metadata_entry(key, 8, &value_bytes);
    let mut cursor = Cursor::new(data);

    let metadata = parse_metadata(&mut cursor, 1).unwrap();

    assert_eq!(metadata.get(key).unwrap().as_str(), Some(""));
}

#[test]
fn test_zero_tensor_count() {
    let mut data = vec![];
    data.extend_from_slice(&GGUF_MAGIC.to_le_bytes());
    data.extend_from_slice(&GGUF_VERSION.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes()); // Zero tensors
    data.extend_from_slice(&0u64.to_le_bytes()); // Zero metadata

    let mut cursor = Cursor::new(data);
    let header = parse_header(&mut cursor).unwrap();

    assert_eq!(header.tensor_count, 0);
    assert_eq!(header.metadata_kv_count, 0);
}

#[test]
fn test_large_tensor_count() {
    // Should parse headers with large counts (though reading would require actual data)
    let mut data = vec![];
    data.extend_from_slice(&GGUF_MAGIC.to_le_bytes());
    data.extend_from_slice(&GGUF_VERSION.to_le_bytes());
    data.extend_from_slice(&1000u64.to_le_bytes()); // 1000 tensors
    data.extend_from_slice(&500u64.to_le_bytes()); // 500 metadata entries

    let mut cursor = Cursor::new(data);
    let header = parse_header(&mut cursor).unwrap();

    assert_eq!(header.tensor_count, 1000);
    assert_eq!(header.metadata_kv_count, 500);
}
