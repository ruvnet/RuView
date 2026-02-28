//! Unit tests for weight quantization

use ruvector_sparse_inference::memory::quantization::*;

mod common;
use common::*;

#[test]
fn test_int8_quantization_roundtrip() {
    let original = random_vector(1024);
    let quantized = QuantizedWeights::quantize_int8(&original);
    let dequantized = quantized.dequantize_row(0);

    // Should be close after dequantization
    assert_vectors_close(&original, &dequantized, 0.01);
}

#[test]
fn test_int8_quantization_dimensions() {
    let original = random_vector(1024);
    let quantized = QuantizedWeights::quantize_int8(&original);

    assert_eq!(quantized.nrows(), 1);
    assert_eq!(quantized.ncols(), 1024);
}

#[test]
fn test_int4_quantization_compression() {
    let original: Vec<f32> = (0..1024).map(|i| (i as f32) * 0.01).collect();
    let quantized = QuantizedWeights::quantize_int4(&original, 64); // group_size=64

    // Int4 should be significantly smaller than original (4 bytes per f32)
    let original_size = original.len() * 4;
    let quantized_size = quantized.size_bytes();

    assert!(quantized_size < original_size / 4,
        "Int4 quantization should compress data (original: {}, quantized: {})",
        original_size, quantized_size);
}

#[test]
fn test_int4_quantization_roundtrip() {
    let original: Vec<f32> = (0..256).map(|i| (i as f32) * 0.01).collect();
    let quantized = QuantizedWeights::quantize_int4(&original, 32);
    let dequantized = quantized.dequantize_row(0);

    // Int4 has lower precision, so tolerance is higher
    assert_vectors_close(&original, &dequantized, 0.05);
}

#[test]
fn test_int4_different_group_sizes() {
    let original = random_vector(512);

    for group_size in [16, 32, 64, 128] {
        let quantized = QuantizedWeights::quantize_int4(&original, group_size);
        let dequantized = quantized.dequantize_row(0);

        assert_eq!(original.len(), dequantized.len(),
            "Length mismatch for group_size {}", group_size);
        assert_vectors_close(&original, &dequantized, 0.1);
    }
}

#[test]
fn test_selective_dequantization() {
    // Create a larger matrix to test selective dequantization
    let rows_data: Vec<Vec<f32>> = (0..100)
        .map(|_| random_vector(512))
        .collect();

    // For this test, we'll quantize each row separately and store them
    // (In real implementation, you'd have a multi-row quantization)
    let quantized = QuantizedWeights::quantize_int8(&rows_data[0]);

    let selected_rows = vec![0];
    let dequantized = quantized.dequantize_rows(&selected_rows);

    assert_eq!(dequantized.nrows(), selected_rows.len());
    assert_eq!(dequantized.ncols(), 512);
}

#[test]
fn test_quantization_preserves_range() {
    let original: Vec<f32> = vec![-5.0, -2.5, 0.0, 2.5, 5.0];
    let quantized = QuantizedWeights::quantize_int8(&original);
    let dequantized = quantized.dequantize_row(0);

    // Check that min and max are approximately preserved
    let orig_min = original.iter().cloned().fold(f32::INFINITY, f32::min);
    let orig_max = original.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let deq_min = dequantized.iter().cloned().fold(f32::INFINITY, f32::min);
    let deq_max = dequantized.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

    assert!((orig_min - deq_min).abs() < 0.1);
    assert!((orig_max - deq_max).abs() < 0.1);
}

#[test]
fn test_quantization_uniform_values() {
    let original = vec![3.14f32; 100];
    let quantized = QuantizedWeights::quantize_int8(&original);
    let dequantized = quantized.dequantize_row(0);

    // All values should be approximately the same
    for &val in &dequantized {
        assert!((val - 3.14).abs() < 0.1);
    }
}

#[test]
fn test_quantization_zero_values() {
    let original = vec![0.0f32; 100];
    let quantized = QuantizedWeights::quantize_int8(&original);
    let dequantized = quantized.dequantize_row(0);

    // All values should be close to zero
    for &val in &dequantized {
        assert!(val.abs() < 0.01);
    }
}

#[test]
fn test_int4_odd_length() {
    // Test with odd number of elements (tests padding)
    let original = random_vector(513); // Odd number
    let quantized = QuantizedWeights::quantize_int4(&original, 32);
    let dequantized = quantized.dequantize_row(0);

    assert_eq!(original.len(), dequantized.len());
}

#[test]
fn test_quantization_size_reduction() {
    let original = random_vector(4096);
    let original_size = original.len() * std::mem::size_of::<f32>();

    let int8_quantized = QuantizedWeights::quantize_int8(&original);
    let int8_size = int8_quantized.size_bytes();

    let int4_quantized = QuantizedWeights::quantize_int4(&original, 64);
    let int4_size = int4_quantized.size_bytes();

    // Verify compression ratios
    assert!(int8_size < original_size / 2, "Int8 should be ~4x smaller");
    assert!(int4_size < int8_size, "Int4 should be smaller than Int8");
}

#[test]
fn test_multiple_row_dequantization() {
    let quantized = create_quantized_matrix(100, 512);
    let rows = vec![10, 50, 99];

    let dequantized = quantized.dequantize_rows(&rows);

    assert_eq!(dequantized.nrows(), rows.len());
    assert_eq!(dequantized.ncols(), 512);

    // All values should be finite
    for i in 0..dequantized.nrows() {
        for j in 0..dequantized.ncols() {
            assert!(dequantized[[i, j]].is_finite());
        }
    }
}

#[test]
#[should_panic(expected = "Row index out of bounds")]
fn test_dequantize_out_of_bounds_row() {
    let quantized = QuantizedWeights::quantize_int8(&random_vector(512));
    quantized.dequantize_row(5); // Only 1 row exists
}

#[test]
fn test_quantization_large_values() {
    let original = vec![1000.0, 5000.0, -3000.0, 10000.0];
    let quantized = QuantizedWeights::quantize_int8(&original);
    let dequantized = quantized.dequantize_row(0);

    // Should handle large values reasonably
    assert_vectors_close(&original, &dequantized, 100.0); // Higher tolerance for large values
}

#[test]
fn test_int4_group_boundary() {
    // Test that group boundaries are handled correctly
    let original = random_vector(128);
    let quantized = QuantizedWeights::quantize_int4(&original, 32); // 4 groups exactly
    let dequantized = quantized.dequantize_row(0);

    assert_eq!(original.len(), dequantized.len());
    assert_vectors_close(&original, &dequantized, 0.1);
}
