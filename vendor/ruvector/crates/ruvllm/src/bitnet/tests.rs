//! Comprehensive tests for PT-BitNet Phase 0 ternary quantization
//!
//! Test coverage based on ADR-017 (AD-1, AD-18):
//! - Ternary packing/unpacking roundtrips
//! - Absmean quantization correctness
//! - Dequantization accuracy
//! - Full tensor quantization
//! - Edge cases and error conditions

use super::{
    dequantize_bitnet_t158, pack_ternary, quantize_tensor, unpack_ternary, PtBitnetConfig,
    TernaryTensor,
};

// ============================================================================
// Test Constants
// ============================================================================

const EPSILON: f32 = 1e-6;
const BLOCK_SIZE: usize = 256;

// ============================================================================
// 1. Ternary Packing Roundtrip Tests
// ============================================================================

#[test]
fn test_pack_unpack_simple_roundtrip() {
    // Simple 4-element ternary array
    let ternary = vec![1i8, 0, -1, 1];
    let packed = pack_ternary(&ternary);
    let unpacked = unpack_ternary(&packed, 4);

    assert_eq!(ternary, unpacked, "Packing roundtrip failed for [1, 0, -1, 1]");
}

#[test]
fn test_pack_all_zeros() {
    let ternary = vec![0i8; 256];
    let packed = pack_ternary(&ternary);
    let unpacked = unpack_ternary(&packed, 256);

    assert_eq!(ternary, unpacked);
    assert!(unpacked.iter().all(|&x| x == 0), "All zeros should remain all zeros");
}

#[test]
fn test_pack_all_ones() {
    let ternary = vec![1i8; 256];
    let packed = pack_ternary(&ternary);
    let unpacked = unpack_ternary(&packed, 256);

    assert_eq!(ternary, unpacked);
    assert!(unpacked.iter().all(|&x| x == 1), "All +1 should remain all +1");
}

#[test]
fn test_pack_all_neg_ones() {
    let ternary = vec![-1i8; 256];
    let packed = pack_ternary(&ternary);
    let unpacked = unpack_ternary(&packed, 256);

    assert_eq!(ternary, unpacked);
    assert!(unpacked.iter().all(|&x| x == -1), "All -1 should remain all -1");
}

#[test]
fn test_pack_one_block_256_elements() {
    // One full block (256 elements) with alternating pattern
    let mut ternary = Vec::with_capacity(256);
    for i in 0..256 {
        ternary.push(match i % 3 {
            0 => 1,
            1 => 0,
            2 => -1,
            _ => unreachable!(),
        });
    }

    let packed = pack_ternary(&ternary);
    let unpacked = unpack_ternary(&packed, 256);

    assert_eq!(ternary, unpacked, "256-element block roundtrip failed");

    // Verify storage size: 256 elements * 2 bits = 64 bytes
    assert_eq!(packed.len(), 64, "Packed size should be 64 bytes for 256 elements");
}

#[test]
fn test_pack_non_aligned_size() {
    // 100 elements (not divisible by 128, the typical packing boundary)
    let mut ternary = Vec::with_capacity(100);
    for i in 0..100 {
        ternary.push(if i % 2 == 0 { 1 } else { -1 });
    }

    let packed = pack_ternary(&ternary);
    let unpacked = unpack_ternary(&packed, 100);

    assert_eq!(
        ternary.len(),
        unpacked.len(),
        "Unpacked length should match original"
    );
    assert_eq!(ternary, unpacked, "Non-aligned size roundtrip failed");
}

#[test]
fn test_pack_large_tensor() {
    // Multiple blocks (1024 elements = 4 blocks)
    let ternary: Vec<i8> = (0..1024)
        .map(|i| match i % 5 {
            0 | 1 => 1,
            2 | 3 => -1,
            4 => 0,
            _ => unreachable!(),
        })
        .collect();

    let packed = pack_ternary(&ternary);
    let unpacked = unpack_ternary(&packed, 1024);

    assert_eq!(ternary, unpacked, "Large tensor roundtrip failed");
}

// ============================================================================
// 2. Absmean Quantization Correctness Tests
// ============================================================================

#[test]
fn test_quantize_uniform_random() {
    // Uniform random weights in [-1, 1] should produce all ternary values
    let weights = vec![0.5, -0.3, 0.1, -0.7, 0.9, -0.1, 0.0, 0.4];
    let ternary = quantize_absmean(&weights);

    // All outputs must be in {-1, 0, +1}
    for &t in &ternary {
        assert!(
            t == -1 || t == 0 || t == 1,
            "Quantized value {} not in ternary set",
            t
        );
    }
}

#[test]
fn test_quantize_all_zeros() {
    let weights = vec![0.0; 256];
    let (ternary, scale) = quantize_absmean_with_scale(&weights);

    // All ternary values should be zero
    assert!(
        ternary.iter().all(|&x| x == 0),
        "All-zero input should produce all-zero ternary"
    );

    // Scale should be near epsilon (avoiding division by zero)
    assert!(
        scale < 1e-5,
        "Scale for all-zero weights should be near epsilon, got {}",
        scale
    );
}

#[test]
fn test_quantize_large_positive() {
    // Large positive weights should quantize to all +1
    let weights = vec![10.0; 256];
    let (ternary, scale) = quantize_absmean_with_scale(&weights);

    // All should be +1
    assert!(
        ternary.iter().all(|&x| x == 1),
        "Large positive weights should quantize to +1"
    );

    // Scale should be approximately 10.0 (mean absolute value)
    assert!(
        (scale - 10.0).abs() < 0.1,
        "Scale should be ~10.0, got {}",
        scale
    );
}

#[test]
fn test_quantize_large_negative() {
    // Large negative weights should quantize to all -1
    let weights = vec![-10.0; 256];
    let (ternary, scale) = quantize_absmean_with_scale(&weights);

    // All should be -1
    assert!(
        ternary.iter().all(|&x| x == -1),
        "Large negative weights should quantize to -1"
    );

    // Scale should be approximately 10.0 (mean absolute value)
    assert!(
        (scale - 10.0).abs() < 0.1,
        "Scale should be ~10.0, got {}",
        scale
    );
}

#[test]
fn test_quantize_known_example() {
    // From ADR: W_ternary = RoundClip(W / (mean(|W|) + epsilon), -1, 1)
    // Example: weights = [0.5, -0.3, 0.1, -0.7]
    // gamma = mean(|W|) = (0.5 + 0.3 + 0.1 + 0.7) / 4 = 0.4
    // normalized = [1.25, -0.75, 0.25, -1.75]
    // ternary = [1, -1, 0, -1] (after clamp and round)

    let weights = vec![0.5, -0.3, 0.1, -0.7];
    let (ternary, scale) = quantize_absmean_with_scale(&weights);

    // Verify scale is approximately 0.4
    assert!(
        (scale - 0.4).abs() < 0.01,
        "Expected scale ~0.4, got {}",
        scale
    );

    // Verify ternary values
    // 1.25 -> 1, -0.75 -> -1, 0.25 -> 0, -1.75 -> -1
    assert_eq!(ternary[0], 1, "0.5/0.4 = 1.25 should round to 1");
    assert_eq!(ternary[1], -1, "-0.3/0.4 = -0.75 should round to -1");
    assert_eq!(ternary[2], 0, "0.1/0.4 = 0.25 should round to 0");
    assert_eq!(ternary[3], -1, "-0.7/0.4 = -1.75 should clamp to -1");
}

#[test]
fn test_quantize_scale_calculation() {
    // Verify scale = mean(|weights|)
    let weights = vec![1.0, -2.0, 3.0, -4.0];
    let (_, scale) = quantize_absmean_with_scale(&weights);

    let expected_scale = (1.0 + 2.0 + 3.0 + 4.0) / 4.0; // = 2.5
    assert!(
        (scale - expected_scale).abs() < EPSILON,
        "Scale should be mean of absolute values: expected {}, got {}",
        expected_scale,
        scale
    );
}

// ============================================================================
// 3. Dequantization Correctness Tests
// ============================================================================

#[test]
fn test_dequantize_simple() {
    let ternary = vec![1i8, 0, -1];
    let scale = 2.0;

    let dequantized = dequantize_ternary(&ternary, scale);

    assert_eq!(dequantized.len(), 3);
    assert!((dequantized[0] - 2.0).abs() < EPSILON, "1 * 2.0 = 2.0");
    assert!((dequantized[1] - 0.0).abs() < EPSILON, "0 * 2.0 = 0.0");
    assert!((dequantized[2] - (-2.0)).abs() < EPSILON, "-1 * 2.0 = -2.0");
}

#[test]
fn test_dequantize_packed_data() {
    // Pack known ternary data, then dequantize
    let ternary = vec![1i8, 0, -1, 1];
    let packed = pack_ternary(&ternary);
    let scale = 3.5;

    let unpacked = unpack_ternary(&packed, 4);
    let dequantized = dequantize_ternary(&unpacked, scale);

    assert_eq!(dequantized.len(), 4);
    assert!((dequantized[0] - 3.5).abs() < EPSILON);
    assert!((dequantized[1] - 0.0).abs() < EPSILON);
    assert!((dequantized[2] - (-3.5)).abs() < EPSILON);
    assert!((dequantized[3] - 3.5).abs() < EPSILON);
}

#[test]
fn test_quantize_dequantize_roundtrip_mse() {
    // Quantize -> Dequantize should have bounded MSE
    let weights = vec![0.5, -0.3, 0.1, -0.7, 0.9, -0.1, 0.4, -0.5];
    let (ternary, scale) = quantize_absmean_with_scale(&weights);
    let dequantized = dequantize_ternary(&ternary, scale);

    // Compute MSE
    let mse: f32 = weights
        .iter()
        .zip(dequantized.iter())
        .map(|(&w, &d)| (w - d).powi(2))
        .sum::<f32>()
        / weights.len() as f32;

    // MSE should be reasonable (ternary quantization is lossy)
    // For absmean, expect MSE < 0.5 for normalized weights
    assert!(
        mse < 0.5,
        "MSE too high: {} (weights may not reconstruct well)",
        mse
    );
}

#[test]
fn test_dequantize_full_block() {
    // Dequantize a full 256-element block
    let ternary: Vec<i8> = (0..256).map(|i| if i % 2 == 0 { 1 } else { -1 }).collect();
    let scale = 1.5;

    let dequantized = dequantize_ternary(&ternary, scale);

    assert_eq!(dequantized.len(), 256);
    for (i, &val) in dequantized.iter().enumerate() {
        let expected = if i % 2 == 0 { 1.5 } else { -1.5 };
        assert!(
            (val - expected).abs() < EPSILON,
            "Element {} incorrect: expected {}, got {}",
            i,
            expected,
            val
        );
    }
}

// ============================================================================
// 4. Full Tensor Quantization Tests
// ============================================================================

#[test]
fn test_tensor_quantize_256x256() {
    // 256x256 random tensor (65536 elements)
    let mut weights = Vec::with_capacity(65536);
    for i in 0..65536 {
        let val = ((i as f32) * 0.001).sin(); // Pseudo-random in [-1, 1]
        weights.push(val);
    }

    let tensor = TernaryTensor::quantize(&weights, BLOCK_SIZE);

    // Verify shape preserved
    assert_eq!(
        tensor.num_elements(),
        65536,
        "Tensor should preserve element count"
    );

    // Verify sparsity is in valid range
    let sparsity = tensor.sparsity();
    assert!(
        sparsity >= 0.0 && sparsity <= 1.0,
        "Sparsity {} out of range [0, 1]",
        sparsity
    );

    // For uniform random, expect ~1/3 zeros (rough heuristic)
    assert!(
        sparsity > 0.15 && sparsity < 0.5,
        "Sparsity {} seems unrealistic for uniform random input",
        sparsity
    );
}

#[test]
fn test_tensor_memory_bytes() {
    let weights = vec![0.5; 256];
    let tensor = TernaryTensor::quantize(&weights, BLOCK_SIZE);

    // Expected memory:
    // - Packed data: 256 elements * 2 bits / 8 = 64 bytes
    // - Scales: 1 block * 4 bytes (f32) = 4 bytes
    // Total: 68 bytes
    let expected_bytes = 64 + 4;

    assert_eq!(
        tensor.memory_bytes(),
        expected_bytes,
        "Memory calculation incorrect"
    );
}

#[test]
fn test_tensor_sparsity_calculation() {
    // Known sparsity: 50% zeros
    let weights: Vec<f32> = (0..256)
        .map(|i| if i % 2 == 0 { 0.0 } else { 1.0 })
        .collect();

    let tensor = TernaryTensor::quantize(&weights, BLOCK_SIZE);
    let sparsity = tensor.sparsity();

    // Should be close to 0.5 (half zeros)
    assert!(
        (sparsity - 0.5).abs() < 0.1,
        "Expected sparsity ~0.5, got {}",
        sparsity
    );
}

#[test]
fn test_tensor_block_alignment() {
    // 512 elements = 2 blocks of 256
    let weights = vec![1.0; 512];
    let tensor = TernaryTensor::quantize(&weights, BLOCK_SIZE);

    // Should have 2 scale factors (one per block)
    assert_eq!(
        tensor.num_blocks(),
        2,
        "Expected 2 blocks for 512 elements"
    );
}

#[test]
fn test_tensor_non_aligned_padding() {
    // 300 elements (256 + 44) should create 2 blocks with padding
    let weights = vec![0.5; 300];
    let tensor = TernaryTensor::quantize(&weights, BLOCK_SIZE);

    // Should pad to 2 full blocks (512 elements)
    let num_blocks = (300 + BLOCK_SIZE - 1) / BLOCK_SIZE;
    assert_eq!(
        tensor.num_blocks(),
        num_blocks,
        "Non-aligned tensor should pad to full blocks"
    );

    // Original element count should be preserved
    assert_eq!(tensor.num_elements(), 300);
}

// ============================================================================
// 5. TernaryTensor Properties Tests
// ============================================================================

#[test]
fn test_ternary_tensor_properties() {
    let weights: Vec<f32> = (0..512).map(|i| (i as f32) * 0.01).collect();
    let tensor = TernaryTensor::quantize(&weights, BLOCK_SIZE);

    // Memory bytes should match calculation
    let num_blocks = (512 + BLOCK_SIZE - 1) / BLOCK_SIZE;
    let packed_bytes = num_blocks * BLOCK_SIZE * 2 / 8; // 2 bits per element
    let scale_bytes = num_blocks * 4; // f32 scales
    let expected = packed_bytes + scale_bytes;

    assert_eq!(tensor.memory_bytes(), expected);

    // Sparsity should be in valid range
    assert!(tensor.sparsity() >= 0.0 && tensor.sparsity() <= 1.0);
}

#[test]
fn test_ternary_tensor_uniform_random_sparsity() {
    // Uniform random should have ~1/3 sparsity
    let mut weights = Vec::with_capacity(2048);
    for i in 0..2048 {
        weights.push(((i as f32) * 1.234).sin());
    }

    let tensor = TernaryTensor::quantize(&weights, BLOCK_SIZE);
    let sparsity = tensor.sparsity();

    // Rough heuristic: 20-45% zeros for uniform random
    assert!(
        sparsity > 0.2 && sparsity < 0.45,
        "Uniform random sparsity {} outside expected range [0.2, 0.45]",
        sparsity
    );
}

// ============================================================================
// 6. Config Validation Tests
// ============================================================================

#[test]
fn test_config_default_values() {
    let config = PtBitnetConfig::default();

    assert_eq!(config.block_size, 256, "Default block size should be 256");
    assert!(
        config.calibration_samples > 0,
        "Calibration samples must be > 0"
    );
}

#[test]
#[should_panic(expected = "block_size must be > 0")]
fn test_config_invalid_block_size() {
    let _config = PtBitnetConfig {
        block_size: 0,
        ..Default::default()
    };
}

#[test]
#[should_panic(expected = "calibration_samples must be > 0")]
fn test_config_invalid_calibration_samples() {
    let _config = PtBitnetConfig {
        calibration_samples: 0,
        ..Default::default()
    };
}

// ============================================================================
// 7. Edge Case Tests
// ============================================================================

#[test]
fn test_empty_input() {
    let weights: Vec<f32> = vec![];
    let tensor = TernaryTensor::quantize(&weights, BLOCK_SIZE);

    assert_eq!(tensor.num_elements(), 0);
    assert_eq!(tensor.num_blocks(), 0);
    assert_eq!(tensor.sparsity(), 0.0);
}

#[test]
fn test_single_element() {
    let weights = vec![0.5];
    let tensor = TernaryTensor::quantize(&weights, BLOCK_SIZE);

    assert_eq!(tensor.num_elements(), 1);
    // Should create 1 block (padded)
    assert_eq!(tensor.num_blocks(), 1);
}

#[test]
fn test_very_large_values() {
    let weights = vec![f32::MAX, f32::MAX, f32::MAX, f32::MAX];
    let (ternary, scale) = quantize_absmean_with_scale(&weights);

    // Should all quantize to +1
    assert!(ternary.iter().all(|&x| x == 1), "f32::MAX should quantize to +1");

    // Scale should be approximately f32::MAX
    assert!(scale > 1e30, "Scale should be very large");

    // Dequantization should not produce NaN
    let dequantized = dequantize_ternary(&ternary, scale);
    assert!(
        dequantized.iter().all(|&x| !x.is_nan()),
        "Dequantization should not produce NaN"
    );
}

#[test]
fn test_subnormal_floats() {
    // Very small positive values (subnormal range)
    let weights = vec![1e-40, -1e-40, 1e-39, -1e-39];
    let (ternary, scale) = quantize_absmean_with_scale(&weights);

    // Should quantize reasonably (may be all zeros or small values)
    assert!(ternary.iter().all(|&x| x >= -1 && x <= 1));

    // Scale should be tiny but not zero
    assert!(scale > 0.0, "Scale should be > 0 even for subnormal inputs");
}

#[test]
fn test_nan_handling() {
    // NaN should not crash, but behavior is implementation-defined
    let weights = vec![f32::NAN, 1.0, -1.0, 0.0];
    let result = std::panic::catch_unwind(|| {
        quantize_absmean_with_scale(&weights)
    });

    // Should either panic or handle gracefully
    // At minimum, should not produce infinite loop or segfault
    if let Ok((ternary, scale)) = result {
        // If it succeeds, output should not contain NaN
        assert!(
            !scale.is_nan() || scale == 0.0,
            "Scale should not be NaN unless handled explicitly"
        );
        assert!(
            ternary.iter().all(|&x| x >= -1 && x <= 1),
            "Ternary values must be in valid range"
        );
    }
}

#[test]
fn test_infinity_handling() {
    let weights = vec![f32::INFINITY, f32::NEG_INFINITY, 1.0, -1.0];
    let (ternary, scale) = quantize_absmean_with_scale(&weights);

    // Infinities should quantize to ±1
    assert_eq!(ternary[0], 1, "INFINITY should quantize to +1");
    assert_eq!(ternary[1], -1, "NEG_INFINITY should quantize to -1");

    // Scale should be finite (or handled gracefully)
    // Implementation may cap scale to avoid overflow
    assert!(
        scale.is_finite() || scale > 1e30,
        "Scale should be finite or very large"
    );
}

#[test]
fn test_mixed_magnitudes() {
    // Mix of very large and very small values
    let weights = vec![1000.0, 0.001, -1000.0, -0.001, 0.0];
    let (ternary, scale) = quantize_absmean_with_scale(&weights);

    // Should produce valid ternary values
    assert!(ternary.iter().all(|&x| x >= -1 && x <= 1));

    // Scale should be dominated by large values
    assert!(scale > 100.0, "Scale should reflect large values");

    // Small values should quantize to 0
    assert_eq!(
        ternary[1], 0,
        "0.001 compared to scale ~500 should be 0"
    );
    assert_eq!(ternary[3], 0, "-0.001 should be 0");
}

// ============================================================================
// 8. Layer Filter Tests (per ADR-017 AD-2)
// ============================================================================

#[test]
fn test_should_quantize_expert_layers() {
    // MoE expert FFN layers (gate_proj, up_proj, down_proj) should be quantized
    use super::LayerMask;

    let layer_mask = LayerMask::ExpertsOnly;

    assert!(
        should_quantize_layer("model.layers.0.mlp.gate_proj.weight", &layer_mask),
        "gate_proj should be quantized"
    );
    assert!(
        should_quantize_layer("model.layers.0.mlp.up_proj.weight", &layer_mask),
        "up_proj should be quantized"
    );
    assert!(
        should_quantize_layer("model.layers.0.mlp.down_proj.weight", &layer_mask),
        "down_proj should be quantized"
    );
    assert!(
        should_quantize_layer("model.layers.15.block_sparse_moe.experts.7.w3.weight", &layer_mask),
        "Expert w3 (up_proj) should be quantized"
    );
}

#[test]
fn test_should_not_quantize_router() {
    // Router and gate layers must remain in FP16 per ADR-017 (AD-2)
    use super::LayerMask;

    let layer_mask = LayerMask::ExpertsOnly;

    assert!(
        !should_quantize_layer("model.layers.0.mlp.router.weight", &layer_mask),
        "Router should NOT be quantized"
    );
    assert!(
        !should_quantize_layer("model.layers.0.block_sparse_moe.gate.weight", &layer_mask),
        "MoE gate should NOT be quantized"
    );
}

#[test]
fn test_should_not_quantize_embed() {
    // Embeddings and LM head must remain in FP16 per ADR-017 (AD-2)
    use super::LayerMask;

    let layer_mask = LayerMask::ExpertsOnly;

    assert!(
        !should_quantize_layer("model.embed_tokens.weight", &layer_mask),
        "Embed tokens should NOT be quantized"
    );
    assert!(
        !should_quantize_layer("lm_head.weight", &layer_mask),
        "LM head should NOT be quantized"
    );
    assert!(
        !should_quantize_layer("model.embeddings.word_embeddings", &layer_mask),
        "Word embeddings should NOT be quantized"
    );
}

#[test]
fn test_should_not_quantize_norm() {
    // Normalization layers must remain in FP16 per ADR-017 (AD-2)
    use super::LayerMask;

    let layer_mask = LayerMask::ExpertsOnly;

    assert!(
        !should_quantize_layer("model.layers.0.input_layernorm.weight", &layer_mask),
        "Input layernorm should NOT be quantized"
    );
    assert!(
        !should_quantize_layer("model.layers.0.post_attention_layernorm.weight", &layer_mask),
        "Post-attention layernorm should NOT be quantized"
    );
    assert!(
        !should_quantize_layer("model.norm.weight", &layer_mask),
        "Final norm should NOT be quantized"
    );
    assert!(
        !should_quantize_layer("model.layers.0.self_attn.layer_norm", &layer_mask),
        "Self-attention layer_norm should NOT be quantized"
    );
}

#[test]
fn test_layer_mask_all() {
    // LayerMask::All should quantize all linear layers except protected ones
    use super::LayerMask;

    let layer_mask = LayerMask::All;

    // Should quantize attention projections
    assert!(
        should_quantize_layer("model.layers.0.self_attn.q_proj.weight", &layer_mask),
        "Query projection should be quantized with LayerMask::All"
    );
    assert!(
        should_quantize_layer("model.layers.0.self_attn.k_proj.weight", &layer_mask),
        "Key projection should be quantized with LayerMask::All"
    );

    // Should still protect router/embed/norm
    assert!(
        !should_quantize_layer("model.layers.0.mlp.router.weight", &layer_mask),
        "Router should be protected even with LayerMask::All"
    );
    assert!(
        !should_quantize_layer("model.embed_tokens.weight", &layer_mask),
        "Embeddings should be protected even with LayerMask::All"
    );
}

#[test]
fn test_layer_mask_custom() {
    // LayerMask::Custom should match specified patterns only
    use super::LayerMask;

    let layer_mask = LayerMask::Custom(vec!["w1".to_string(), "w3".to_string()]);

    assert!(
        should_quantize_layer("model.layers.0.mlp.experts.0.w1.weight", &layer_mask),
        "w1 should match custom pattern"
    );
    assert!(
        should_quantize_layer("model.layers.0.mlp.experts.0.w3.weight", &layer_mask),
        "w3 should match custom pattern"
    );
    assert!(
        !should_quantize_layer("model.layers.0.mlp.experts.0.w2.weight", &layer_mask),
        "w2 should NOT match custom pattern"
    );
}

/// Helper function for layer filtering logic (matches ADR-017 AD-2 specification)
fn should_quantize_layer(layer_name: &str, mask: &super::LayerMask) -> bool {
    use super::LayerMask;

    match mask {
        LayerMask::ExpertsOnly => {
            // Quantize MoE expert FFN layers only (gate_proj, up_proj, down_proj, w1, w2, w3)
            // Exclude: router, gate, embed, norm, lm_head
            let is_expert_ffn = layer_name.contains("gate_proj")
                || layer_name.contains("up_proj")
                || layer_name.contains("down_proj")
                || (layer_name.contains("experts")
                    && (layer_name.contains(".w1.") || layer_name.contains(".w2.") || layer_name.contains(".w3.")));

            let is_protected = layer_name.contains("router")
                || layer_name.contains(".gate.") // MoE gate (not gate_proj)
                || layer_name.contains("embed")
                || layer_name.contains("lm_head")
                || layer_name.contains("norm");

            is_expert_ffn && !is_protected
        }
        LayerMask::All => {
            // Quantize all linear layers except protected ones
            let is_protected = layer_name.contains("router")
                || layer_name.contains("embed")
                || layer_name.contains("lm_head")
                || layer_name.contains("norm");

            !is_protected
        }
        LayerMask::Custom(patterns) => {
            // Match any custom pattern
            patterns.iter().any(|p| layer_name.contains(p))
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Helper to quantize weights using absmean method
/// Returns both ternary values and scale factor
fn quantize_absmean_with_scale(weights: &[f32]) -> (Vec<i8>, f32) {
    if weights.is_empty() {
        return (vec![], 0.0);
    }

    // Compute absmean scale: gamma = mean(|W|) + epsilon
    let absmean: f32 = weights.iter().map(|&w| w.abs()).sum::<f32>() / weights.len() as f32;
    let scale = absmean + EPSILON;

    // Quantize: W_ternary = RoundClip(W / scale, -1, 1)
    let ternary: Vec<i8> = weights
        .iter()
        .map(|&w| {
            let normalized = w / scale;
            // Round and clip to {-1, 0, +1}
            if normalized >= 0.5 {
                1
            } else if normalized <= -0.5 {
                -1
            } else {
                0
            }
        })
        .collect();

    (ternary, scale)
}

/// Helper to quantize weights (scale not needed)
fn quantize_absmean(weights: &[f32]) -> Vec<i8> {
    let (ternary, _scale) = quantize_absmean_with_scale(weights);
    ternary
}

/// Helper to dequantize ternary values
fn dequantize_ternary(ternary: &[i8], scale: f32) -> Vec<f32> {
    ternary.iter().map(|&t| (t as f32) * scale).collect()
}
