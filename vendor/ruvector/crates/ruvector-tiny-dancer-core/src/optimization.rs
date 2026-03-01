//! Model optimization techniques (quantization, pruning, knowledge distillation)

use crate::error::{Result, TinyDancerError};
use ndarray::Array2;

/// Quantization configuration
#[derive(Debug, Clone, Copy)]
pub enum QuantizationMode {
    /// No quantization (FP32)
    None,
    /// INT8 quantization
    Int8,
    /// INT16 quantization
    Int16,
}

/// Quantization parameters
#[derive(Debug, Clone)]
pub struct QuantizationParams {
    /// Scale factor
    pub scale: f32,
    /// Zero point
    pub zero_point: i32,
    /// Min value
    pub min_val: f32,
    /// Max value
    pub max_val: f32,
}

/// Quantize a weight matrix to INT8
pub fn quantize_to_int8(weights: &Array2<f32>) -> Result<(Vec<i8>, QuantizationParams)> {
    let min_val = weights.iter().fold(f32::INFINITY, |a, &b| a.min(b));
    let max_val = weights.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));

    if (max_val - min_val).abs() < f32::EPSILON {
        return Err(TinyDancerError::InvalidInput(
            "Cannot quantize constant weights".to_string(),
        ));
    }

    // Calculate scale and zero point for symmetric quantization
    let scale = (max_val - min_val) / 255.0;
    let zero_point = -128;

    let quantized: Vec<i8> = weights
        .iter()
        .map(|&w| {
            let q = ((w - min_val) / scale) as i32 + zero_point;
            q.clamp(-128, 127) as i8
        })
        .collect();

    let params = QuantizationParams {
        scale,
        zero_point,
        min_val,
        max_val,
    };

    Ok((quantized, params))
}

/// Dequantize INT8 weights back to FP32
pub fn dequantize_from_int8(
    quantized: &[i8],
    params: &QuantizationParams,
    shape: (usize, usize),
) -> Result<Array2<f32>> {
    let weights: Vec<f32> = quantized
        .iter()
        .map(|&q| {
            let dequantized = (q as i32 - params.zero_point) as f32 * params.scale + params.min_val;
            dequantized
        })
        .collect();

    Array2::from_shape_vec(shape, weights)
        .map_err(|e| TinyDancerError::InvalidInput(format!("Shape error: {}", e)))
}

/// Apply magnitude-based pruning to weights
pub fn prune_weights(weights: &mut Array2<f32>, sparsity: f32) -> Result<usize> {
    if !(0.0..=1.0).contains(&sparsity) {
        return Err(TinyDancerError::InvalidInput(
            "Sparsity must be between 0.0 and 1.0".to_string(),
        ));
    }

    let total_weights = weights.len();
    let num_to_prune = (total_weights as f32 * sparsity) as usize;

    // Get absolute values
    let mut abs_weights: Vec<(usize, f32)> = weights
        .iter()
        .enumerate()
        .map(|(i, &w)| (i, w.abs()))
        .collect();

    // Sort by magnitude
    abs_weights.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    // Zero out smallest weights
    let mut pruned_count = 0;
    for i in 0..num_to_prune {
        let idx = abs_weights[i].0;
        let (row, col) = (idx / weights.ncols(), idx % weights.ncols());
        weights[[row, col]] = 0.0;
        pruned_count += 1;
    }

    Ok(pruned_count)
}

/// Calculate model compression ratio
pub fn compression_ratio(original_size: usize, compressed_size: usize) -> f32 {
    original_size as f32 / compressed_size as f32
}

/// Calculate speedup from optimization
pub fn calculate_speedup(original_time_us: u64, optimized_time_us: u64) -> f32 {
    original_time_us as f32 / optimized_time_us as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array2;

    #[test]
    fn test_int8_quantization() {
        let weights = Array2::from_shape_vec((2, 2), vec![1.0, 2.0, 3.0, 4.0]).unwrap();
        let (quantized, params) = quantize_to_int8(&weights).unwrap();

        assert_eq!(quantized.len(), 4);
        assert!(params.scale > 0.0);
    }

    #[test]
    fn test_quantization_dequantization() {
        let weights =
            Array2::from_shape_vec((3, 3), vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0])
                .unwrap();
        let (quantized, params) = quantize_to_int8(&weights).unwrap();
        let dequantized = dequantize_from_int8(&quantized, &params, (3, 3)).unwrap();

        // Check that values are approximately preserved
        for (orig, deq) in weights.iter().zip(dequantized.iter()) {
            assert!((orig - deq).abs() < 0.1);
        }
    }

    #[test]
    fn test_pruning() {
        let mut weights = Array2::from_shape_vec((2, 2), vec![1.0, 0.1, 0.2, 2.0]).unwrap();
        let pruned = prune_weights(&mut weights, 0.5).unwrap();

        assert_eq!(pruned, 2);
        // Smallest 2 values should be zero
        let zero_count = weights.iter().filter(|&&w| w == 0.0).count();
        assert_eq!(zero_count, 2);
    }

    #[test]
    fn test_compression_ratio() {
        let ratio = compression_ratio(1000, 250);
        assert_eq!(ratio, 4.0);
    }
}
