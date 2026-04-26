//! Integration tests for the RVF (RuVector Format) container module.
//!
//! These tests exercise the public RvfBuilder and RvfReader APIs through
//! the library crate's public interface. They complement the inline unit
//! tests in rvf_container.rs by testing from the perspective of an external
//! consumer.
//!
//! Test matrix:
//! - Empty builder produces valid (empty) container
//! - Full round-trip: manifest + weights + metadata -> build -> read -> verify
//! - Segment type tagging and ordering
//! - Magic byte corruption is rejected
//! - Float32 precision is preserved bit-for-bit
//! - Large payload (1M weights) round-trip
//! - Multiple metadata segments coexist
//! - File I/O round-trip
//! - Witness/proof segment verification
//! - Write/read benchmark for ~10MB container

use wifi_densepose_sensing_server::rvf_container::{
    RvfBuilder, RvfReader, VitalSignConfig,
};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_rvf_builder_empty() {
    let builder = RvfBuilder::new();
    let data = builder.build();

    // Empty builder produces zero bytes (no segments => no headers)
    assert!(
        data.is_empty(),
        "empty builder should produce empty byte vec"
    );

    // Reader should parse an empty container with zero segments
    let reader = RvfReader::from_bytes(&data).expect("should parse empty container");
    assert_eq!(reader.segment_count(), 0);
    assert_eq!(reader.total_size(), 0);
}

#[test]
fn test_rvf_round_trip() {
    let mut builder = RvfBuilder::new();

    // Add all segment types
    builder.add_manifest("vital-signs-v1", "0.1.0", "Vital sign detection model");

    let weights: Vec<f32> = (0..100).map(|i| i as f32 * 0.01).collect();
    builder.add_weights(&weights);

    let metadata = serde_json::json!({
        "training_epochs": 50,
        "loss": 0.023,
        "optimizer": "adam",
    });
    builder.add_metadata(&metadata);

    let data = builder.build();
    assert!(!data.is_empty(), "container with data should not be empty");

    // Alignment: every segment should start on a 64-byte boundary
    assert_eq!(
        data.len() % 64,
        0,
        "total size should be a multiple of 64 bytes"
    );

    // Parse back
    let reader = RvfReader::from_bytes(&data).expect("should parse container");
    assert_eq!(reader.segment_count(), 3);

    // Verify manifest
    let manifest = reader
        .manifest()
        .expect("should have manifest");
    assert_eq!(manifest["model_id"], "vital-signs-v1");
    assert_eq!(manifest["version"], "0.1.0");
    assert_eq!(manifest["description"], "Vital sign detection model");

    // Verify weights
    let decoded_weights = reader
        .weights()
        .expect("should have weights");
    assert_eq!(decoded_weights.len(), weights.len());
    for (i, (&original, &decoded)) in weights.iter().zip(decoded_weights.iter()).enumerate() {
        assert_eq!(
            original.to_bits(),
            decoded.to_bits(),
            "weight[{i}] mismatch"
        );
    }

    // Verify metadata
    let decoded_meta = reader
        .metadata()
        .expect("should have metadata");
    assert_eq!(decoded_meta["training_epochs"], 50);
    assert_eq!(decoded_meta["optimizer"], "adam");
}

#[test]
fn test_rvf_segment_types() {
    let mut builder = RvfBuilder::new();
    builder.add_manifest("test", "1.0", "test model");
    builder.add_weights(&[1.0, 2.0]);
    builder.add_metadata(&serde_json::json!({"key": "value"}));
    builder.add_witness(
        "sha256:abc123",
        &serde_json::json!({"accuracy": 0.95}),
    );

    let data = builder.build();
    let reader = RvfReader::from_bytes(&data).expect("should parse");

    assert_eq!(reader.segment_count(), 4);

    // Each segment type should be present
    assert!(reader.manifest().is_some(), "manifest should be present");
    assert!(reader.weights().is_some(), "weights should be present");
    assert!(reader.metadata().is_some(), "metadata should be present");
    assert!(reader.witness().is_some(), "witness should be present");

    // Verify segment order via segment IDs (monotonically increasing)
    let ids: Vec<u64> = reader
        .segments()
        .map(|(h, _)| h.segment_id)
        .collect();
    assert_eq!(ids, vec![0, 1, 2, 3], "segment IDs should be 0,1,2,3");
}

#[test]
fn test_rvf_magic_validation() {
    let mut builder = RvfBuilder::new();
    builder.add_manifest("test", "1.0", "test");
    let mut data = builder.build();

    // Corrupt the magic bytes in the first segment header
    // Magic is at offset 0x00..0x04
    data[0] = 0xDE;
    data[1] = 0xAD;
    data[2] = 0xBE;
    data[3] = 0xEF;

    let result = RvfReader::from_bytes(&data);
    assert!(
        result.is_err(),
        "corrupted magic should fail to parse"
    );

    let err = result.unwrap_err();
    assert!(
        err.contains("magic"),
        "error message should mention 'magic', got: {}",
        err
    );
}

#[test]
fn test_rvf_weights_f32_precision() {
    // Test specific float32 edge cases
    let weights: Vec<f32> = vec![
        0.0,
        1.0,
        -1.0,
        f32::MIN_POSITIVE,
        f32::MAX,
        f32::MIN,
        f32::EPSILON,
        std::f32::consts::PI,
        std::f32::consts::E,
        1.0e-30,
        1.0e30,
        -0.0,
        0.123456789,
        1.0e-45, // subnormal
    ];

    let mut builder = RvfBuilder::new();
    builder.add_weights(&weights);
    let data = builder.build();

    let reader = RvfReader::from_bytes(&data).expect("should parse");
    let decoded = reader.weights().expect("should have weights");

    assert_eq!(decoded.len(), weights.len());
    for (i, (&original, &parsed)) in weights.iter().zip(decoded.iter()).enumerate() {
        assert_eq!(
            original.to_bits(),
            parsed.to_bits(),
            "weight[{i}] bit-level mismatch: original={original} (0x{:08X}), parsed={parsed} (0x{:08X})",
            original.to_bits(),
            parsed.to_bits(),
        );
    }
}

#[test]
fn test_rvf_large_payload() {
    // 1 million f32 weights = 4 MB of payload data
    let num_weights = 1_000_000;
    let weights: Vec<f32> = (0..num_weights)
        .map(|i| (i as f32 * 0.000001).sin())
        .collect();

    let mut builder = RvfBuilder::new();
    builder.add_manifest("large-test", "1.0", "Large payload test");
    builder.add_weights(&weights);
    let data = builder.build();

    // Container should be at least header + weights bytes
    assert!(
        data.len() >= 64 + num_weights * 4,
        "container should be large enough, got {} bytes",
        data.len()
    );

    let reader = RvfReader::from_bytes(&data).expect("should parse large container");
    let decoded = reader.weights().expect("should have weights");

    assert_eq!(
        decoded.len(),
        num_weights,
        "all 1M weights should round-trip"
    );

    // Spot-check several values
    for idx in [0, 1, 100, 1000, 500_000, 999_999] {
        assert_eq!(
            weights[idx].to_bits(),
            decoded[idx].to_bits(),
            "weight[{idx}] mismatch"
        );
    }
}

#[test]
fn test_rvf_multiple_metadata_segments() {
    // The current builder only stores one metadata segment, but we can add
    // multiple by adding metadata and then other segments to verify all coexist.
    let mut builder = RvfBuilder::new();
    builder.add_manifest("multi-meta", "1.0", "Multiple segment types");

    let meta1 = serde_json::json!({"training_config": {"optimizer": "adam"}});
    builder.add_metadata(&meta1);

    builder.add_vital_config(&VitalSignConfig::default());
    builder.add_quant_info("int8", 0.0078125, -128);

    let data = builder.build();
    let reader = RvfReader::from_bytes(&data).expect("should parse");

    assert_eq!(
        reader.segment_count(),
        4,
        "should have 4 segments (manifest + meta + vital_config + quant)"
    );

    assert!(reader.manifest().is_some());
    assert!(reader.metadata().is_some());
    assert!(reader.vital_config().is_some());
    assert!(reader.quant_info().is_some());

    // Verify metadata content
    let meta = reader.metadata().unwrap();
    assert_eq!(meta["training_config"]["optimizer"], "adam");
}

#[test]
fn test_rvf_file_io() {
    let tmp_dir = tempfile::tempdir().expect("should create temp dir");
    let file_path = tmp_dir.path().join("test_model.rvf");

    let weights: Vec<f32> = vec![0.1, 0.2, 0.3, 0.4, 0.5];

    let mut builder = RvfBuilder::new();
    builder.add_manifest("file-io-test", "1.0.0", "File I/O test model");
    builder.add_weights(&weights);
    builder.add_metadata(&serde_json::json!({"created": "2026-02-28"}));

    // Write to file
    builder
        .write_to_file(&file_path)
        .expect("should write to file");

    // Read back from file
    let reader = RvfReader::from_file(&file_path).expect("should read from file");

    assert_eq!(reader.segment_count(), 3);

    let manifest = reader.manifest().expect("should have manifest");
    assert_eq!(manifest["model_id"], "file-io-test");

    let decoded_weights = reader.weights().expect("should have weights");
    assert_eq!(decoded_weights.len(), weights.len());
    for (a, b) in decoded_weights.iter().zip(weights.iter()) {
        assert_eq!(a.to_bits(), b.to_bits());
    }

    let meta = reader.metadata().expect("should have metadata");
    assert_eq!(meta["created"], "2026-02-28");

    // Verify file size matches in-memory serialization
    let in_memory = builder.build();
    let file_meta = std::fs::metadata(&file_path).expect("should stat file");
    assert_eq!(
        file_meta.len() as usize,
        in_memory.len(),
        "file size should match serialized size"
    );
}

#[test]
fn test_rvf_witness_proof() {
    let training_hash = "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    let metrics = serde_json::json!({
        "accuracy": 0.957,
        "loss": 0.023,
        "epochs": 200,
        "dataset_size": 50000,
    });

    let mut builder = RvfBuilder::new();
    builder.add_manifest("witnessed-model", "2.0", "Model with witness proof");
    builder.add_weights(&[1.0, 2.0, 3.0]);
    builder.add_witness(training_hash, &metrics);

    let data = builder.build();
    let reader = RvfReader::from_bytes(&data).expect("should parse");

    let witness = reader.witness().expect("should have witness segment");
    assert_eq!(
        witness["training_hash"],
        training_hash,
        "training hash should round-trip"
    );
    assert_eq!(witness["metrics"]["accuracy"], 0.957);
    assert_eq!(witness["metrics"]["epochs"], 200);
}

#[test]
fn test_rvf_benchmark_write_read() {
    // Create a container with ~10 MB of weights
    let num_weights = 2_500_000; // 10 MB of f32 data
    let weights: Vec<f32> = (0..num_weights)
        .map(|i| (i as f32 * 0.0001).sin())
        .collect();

    let mut builder = RvfBuilder::new();
    builder.add_manifest("benchmark-model", "1.0", "Benchmark test");
    builder.add_weights(&weights);
    builder.add_metadata(&serde_json::json!({"benchmark": true}));

    // Benchmark write (serialization)
    let write_start = std::time::Instant::now();
    let data = builder.build();
    let write_elapsed = write_start.elapsed();

    let size_mb = data.len() as f64 / (1024.0 * 1024.0);
    let write_speed = size_mb / write_elapsed.as_secs_f64();

    println!(
        "RVF write benchmark: {:.1} MB in {:.2}ms = {:.0} MB/s",
        size_mb,
        write_elapsed.as_secs_f64() * 1000.0,
        write_speed,
    );

    // Benchmark read (deserialization + CRC validation)
    let read_start = std::time::Instant::now();
    let reader = RvfReader::from_bytes(&data).expect("should parse benchmark container");
    let read_elapsed = read_start.elapsed();

    let read_speed = size_mb / read_elapsed.as_secs_f64();

    println!(
        "RVF read benchmark: {:.1} MB in {:.2}ms = {:.0} MB/s",
        size_mb,
        read_elapsed.as_secs_f64() * 1000.0,
        read_speed,
    );

    // Verify correctness
    let decoded_weights = reader.weights().expect("should have weights");
    assert_eq!(decoded_weights.len(), num_weights);
    assert_eq!(weights[0].to_bits(), decoded_weights[0].to_bits());
    assert_eq!(
        weights[num_weights - 1].to_bits(),
        decoded_weights[num_weights - 1].to_bits()
    );

    // Write and read should be reasonably fast
    assert!(
        write_speed > 10.0,
        "write speed {:.0} MB/s is too slow",
        write_speed
    );
    assert!(
        read_speed > 10.0,
        "read speed {:.0} MB/s is too slow",
        read_speed
    );
}

#[test]
fn test_rvf_content_hash_integrity() {
    let mut builder = RvfBuilder::new();
    builder.add_metadata(&serde_json::json!({"integrity": "test"}));
    let mut data = builder.build();

    // Corrupt one byte in the payload area (after the 64-byte header)
    if data.len() > 65 {
        data[65] ^= 0xFF;
        let result = RvfReader::from_bytes(&data);
        assert!(
            result.is_err(),
            "corrupted payload should fail CRC32 hash check"
        );
        assert!(
            result.unwrap_err().contains("hash mismatch"),
            "error should mention hash mismatch"
        );
    }
}

#[test]
fn test_rvf_truncated_data() {
    let mut builder = RvfBuilder::new();
    builder.add_manifest("truncation-test", "1.0", "Truncation test");
    builder.add_weights(&[1.0, 2.0, 3.0, 4.0, 5.0]);
    let data = builder.build();

    // Truncating at header boundary or within payload should fail
    for truncate_at in [0, 10, 32, 63, 64, 65, 80] {
        if truncate_at < data.len() {
            let truncated = &data[..truncate_at];
            let result = RvfReader::from_bytes(truncated);
            // Empty or partial-header data: either returns empty or errors
            if truncate_at < 64 {
                // Less than one header: reader returns 0 segments (no error on empty)
                // or fails if partial header data is present
                // The reader skips if offset + HEADER_SIZE > data.len()
                if truncate_at == 0 {
                    assert!(
                        result.is_ok() && result.unwrap().segment_count() == 0,
                        "empty data should parse as 0 segments"
                    );
                }
            } else {
                // Has header but truncated payload
                assert!(
                    result.is_err(),
                    "truncated at {truncate_at} bytes should fail"
                );
            }
        }
    }
}

#[test]
fn test_rvf_empty_weights() {
    let mut builder = RvfBuilder::new();
    builder.add_weights(&[]);
    let data = builder.build();

    let reader = RvfReader::from_bytes(&data).expect("should parse");
    let weights = reader.weights().expect("should have weights segment");
    assert!(weights.is_empty(), "empty weight vector should round-trip");
}

#[test]
fn test_rvf_vital_config_round_trip() {
    let config = VitalSignConfig {
        breathing_low_hz: 0.15,
        breathing_high_hz: 0.45,
        heartrate_low_hz: 0.9,
        heartrate_high_hz: 1.8,
        min_subcarriers: 64,
        window_size: 1024,
        confidence_threshold: 0.7,
    };

    let mut builder = RvfBuilder::new();
    builder.add_vital_config(&config);
    let data = builder.build();

    let reader = RvfReader::from_bytes(&data).expect("should parse");
    let decoded = reader
        .vital_config()
        .expect("should have vital config");

    assert!(
        (decoded.breathing_low_hz - 0.15).abs() < f64::EPSILON,
        "breathing_low_hz mismatch"
    );
    assert!(
        (decoded.breathing_high_hz - 0.45).abs() < f64::EPSILON,
        "breathing_high_hz mismatch"
    );
    assert!(
        (decoded.heartrate_low_hz - 0.9).abs() < f64::EPSILON,
        "heartrate_low_hz mismatch"
    );
    assert!(
        (decoded.heartrate_high_hz - 1.8).abs() < f64::EPSILON,
        "heartrate_high_hz mismatch"
    );
    assert_eq!(decoded.min_subcarriers, 64);
    assert_eq!(decoded.window_size, 1024);
    assert!(
        (decoded.confidence_threshold - 0.7).abs() < f64::EPSILON,
        "confidence_threshold mismatch"
    );
}

#[test]
fn test_rvf_info_struct() {
    let mut builder = RvfBuilder::new();
    builder.add_manifest("info-test", "2.0", "Info struct test");
    builder.add_weights(&[1.0, 2.0, 3.0]);
    builder.add_vital_config(&VitalSignConfig::default());
    builder.add_witness("sha256:test", &serde_json::json!({"ok": true}));

    let data = builder.build();
    let reader = RvfReader::from_bytes(&data).expect("should parse");
    let info = reader.info();

    assert_eq!(info.segment_count, 4);
    assert!(info.total_size > 0);
    assert!(info.manifest.is_some());
    assert!(info.has_weights);
    assert!(info.has_vital_config);
    assert!(info.has_witness);
    assert!(!info.has_quant_info, "no quant segment was added");
}

#[test]
fn test_rvf_alignment_invariant() {
    // Every container should have total size that is a multiple of 64
    for num_weights in [0, 1, 10, 100, 255, 256, 1000] {
        let weights: Vec<f32> = (0..num_weights).map(|i| i as f32).collect();
        let mut builder = RvfBuilder::new();
        builder.add_weights(&weights);
        let data = builder.build();

        assert_eq!(
            data.len() % 64,
            0,
            "container with {num_weights} weights should be 64-byte aligned, got {} bytes",
            data.len()
        );
    }
}
