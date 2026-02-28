//! BitNet Ternary Dequantization
//!
//! Converts packed 2-bit ternary weights back to FP32 for validation and testing.

use super::ternary_tensor::unpack_ternary;

/// Dequantize BITNET_T158 packed ternary data to FP32.
///
/// This function unpacks 2-bit ternary values and applies per-block scale factors
/// to reconstruct approximate FP32 weights. Used for validation and testing, not
/// for production inference (which should use native ternary kernels).
///
/// # Data Layout
///
/// The input data is organized as:
/// ```text
/// [packed_block_0][scale_0][packed_block_1][scale_1]...
/// ```
///
/// Where each block contains:
/// - 64 bytes of packed 2-bit ternary data (256 values)
/// - 2 bytes of FP16 scale factor
///
/// Total: 66 bytes per 256-element block
///
/// # Arguments
///
/// * `packed` - Raw GGUF tensor data with interleaved ternary and scales
/// * `scales` - Per-block FP32 scale factors
/// * `num_elements` - Total number of output elements
///
/// # Returns
///
/// Vector of FP32 weights approximating the original quantized tensor
///
/// # Example
///
/// ```rust,ignore
/// use ruvllm::bitnet::dequantize_bitnet_t158;
///
/// // Load from GGUF
/// let packed_data = gguf_tensor.data;  // Raw bytes
/// let scales = vec![0.542, 0.381, ...];  // One per block
/// let num_elements = 512;
///
/// let fp32_weights = dequantize_bitnet_t158(&packed_data, &scales, num_elements);
/// ```
pub fn dequantize_bitnet_t158(packed: &[u8], scales: &[f32], num_elements: usize) -> Vec<f32> {
    // Unpack ternary values
    let ternary = unpack_ternary(packed, num_elements);

    // Apply per-block scales
    let block_size = 256; // Standard BitNet block size
    let mut output = Vec::with_capacity(num_elements);

    for (block_idx, chunk) in ternary.chunks(block_size).enumerate() {
        let scale = scales.get(block_idx).copied().unwrap_or(1.0);

        for &ternary_val in chunk {
            let fp32_val = (ternary_val as f32) * scale;
            output.push(fp32_val);
        }
    }

    output
}

/// Dequantize a single BITNET_T158 block.
///
/// Helper function for block-wise dequantization in streaming scenarios.
///
/// # Arguments
///
/// * `packed_block` - 64 bytes of packed 2-bit ternary data
/// * `scale` - FP32 scale factor for this block
/// * `output` - Output buffer (must have capacity for 256 FP32 values)
///
/// # Panics
///
/// Panics if output buffer is smaller than 256 elements.
pub fn dequantize_bitnet_block(packed_block: &[u8], scale: f32, output: &mut [f32]) {
    assert!(
        output.len() >= 256,
        "Output buffer must hold at least 256 elements"
    );
    assert_eq!(
        packed_block.len(),
        64,
        "Packed block must be exactly 64 bytes"
    );

    let ternary = unpack_ternary(packed_block, 256);

    for (i, &ternary_val) in ternary.iter().enumerate() {
        output[i] = (ternary_val as f32) * scale;
    }
}

/// Compute dequantization error metrics.
///
/// Compares dequantized weights against original FP32 weights to measure
/// quantization quality.
///
/// # Arguments
///
/// * `original` - Original FP32 weights
/// * `dequantized` - Dequantized weights from ternary
///
/// # Returns
///
/// Tuple of (mean_absolute_error, mean_squared_error, max_error)
pub fn compute_dequant_error(original: &[f32], dequantized: &[f32]) -> (f32, f32, f32) {
    assert_eq!(
        original.len(),
        dequantized.len(),
        "Arrays must have same length"
    );

    // Guard against empty inputs to avoid division by zero
    if original.is_empty() {
        return (0.0, 0.0, 0.0);
    }

    let mut sum_abs_error = 0.0f32;
    let mut sum_sq_error = 0.0f32;
    let mut max_error = 0.0f32;

    for (orig, dequant) in original.iter().zip(dequantized.iter()) {
        let error = (orig - dequant).abs();
        sum_abs_error += error;
        sum_sq_error += error * error;
        max_error = max_error.max(error);
    }

    let n = original.len() as f32;
    let mae = sum_abs_error / n;
    let mse = sum_sq_error / n;

    (mae, mse, max_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitnet::{absmean_ternary, pack_ternary};

    #[test]
    fn test_dequantize_bitnet_t158_simple() {
        // Create simple ternary data
        let ternary = vec![-1i8, 0, 1, -1, 1, 0, 0, 1];
        let packed = pack_ternary(&ternary);
        let scales = vec![0.5f32];

        let result = dequantize_bitnet_t158(&packed, &scales, 8);

        assert_eq!(result.len(), 8);

        // Check values: ternary * scale
        assert_eq!(result[0], -0.5); // -1 * 0.5
        assert_eq!(result[1], 0.0); // 0 * 0.5
        assert_eq!(result[2], 0.5); // 1 * 0.5
        assert_eq!(result[3], -0.5); // -1 * 0.5
    }

    #[test]
    fn test_dequantize_bitnet_block() {
        // Create a full 256-element block
        let ternary = vec![1i8; 256];
        let packed = pack_ternary(&ternary);
        let scale = 2.0;

        let mut output = vec![0.0f32; 256];
        dequantize_bitnet_block(&packed, scale, &mut output);

        // All values should be 1 * 2.0 = 2.0
        assert!(output.iter().all(|&v| (v - 2.0).abs() < 1e-6));
    }

    #[test]
    fn test_dequantize_multiple_blocks() {
        // Two blocks with different scales
        let ternary1 = vec![1i8; 256];
        let ternary2 = vec![-1i8; 256];

        let mut all_ternary = ternary1.clone();
        all_ternary.extend_from_slice(&ternary2);

        let packed = pack_ternary(&all_ternary);
        let scales = vec![1.0, 2.0];

        let result = dequantize_bitnet_t158(&packed, &scales, 512);

        // First 256 should be 1.0 * 1.0 = 1.0
        assert!(result[..256].iter().all(|&v| (v - 1.0).abs() < 1e-6));

        // Next 256 should be -1.0 * 2.0 = -2.0
        assert!(result[256..512].iter().all(|&v| (v - (-2.0)).abs() < 1e-6));
    }

    #[test]
    fn test_roundtrip_quantize_dequantize() {
        // Original weights
        let original = vec![0.5, -0.3, 0.8, -0.1, 0.0, 0.4, 0.2, -0.6];

        // Quantize
        let (ternary, scale) = absmean_ternary(&original);
        let packed = pack_ternary(&ternary);

        // Dequantize
        let dequantized = dequantize_bitnet_t158(&packed, &[scale], original.len());

        // Check that we got 8 values back
        assert_eq!(dequantized.len(), 8);

        // Values should be approximate (quantization loses precision)
        // But should be close for values near the scale
        for (orig, dequant) in original.iter().zip(dequantized.iter()) {
            let error = (orig - dequant).abs();
            // Error should be bounded by the quantization step (~scale)
            assert!(error < scale * 2.0);
        }
    }

    #[test]
    fn test_compute_dequant_error() {
        let original = vec![1.0, 2.0, 3.0, 4.0];
        let dequantized = vec![1.1, 1.9, 3.2, 3.8];

        let (mae, mse, max_error) = compute_dequant_error(&original, &dequantized);

        // MAE should be (0.1 + 0.1 + 0.2 + 0.2) / 4 = 0.15
        assert!((mae - 0.15).abs() < 1e-6);

        // MSE should be (0.01 + 0.01 + 0.04 + 0.04) / 4 = 0.025
        assert!((mse - 0.025).abs() < 1e-6);

        // Max error should be 0.2
        assert!((max_error - 0.2).abs() < 1e-6);
    }

    #[test]
    #[should_panic(expected = "Output buffer must hold at least 256 elements")]
    fn test_dequantize_block_small_buffer() {
        let packed = vec![0u8; 64];
        let mut output = vec![0.0f32; 128]; // Too small
        dequantize_bitnet_block(&packed, 1.0, &mut output);
    }

    #[test]
    #[should_panic(expected = "Packed block must be exactly 64 bytes")]
    fn test_dequantize_block_wrong_size() {
        let packed = vec![0u8; 32]; // Wrong size
        let mut output = vec![0.0f32; 256];
        dequantize_bitnet_block(&packed, 1.0, &mut output);
    }

    #[test]
    fn test_dequantize_with_missing_scales() {
        // More elements than scales (should use default 1.0)
        let ternary = vec![1i8; 512];
        let packed = pack_ternary(&ternary);
        let scales = vec![2.0]; // Only one scale for two blocks

        let result = dequantize_bitnet_t158(&packed, &scales, 512);

        // First 256 use scale 2.0
        assert!(result[..256].iter().all(|&v| (v - 2.0).abs() < 1e-6));

        // Next 256 use default 1.0
        assert!(result[256..512].iter().all(|&v| (v - 1.0).abs() < 1e-6));
    }
}
