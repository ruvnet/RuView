//! Quantization techniques for memory compression

use crate::error::{Result, VectorDbError};
use crate::types::QuantizationType;
use serde::{Deserialize, Serialize};

/// Quantized vector representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuantizedVector {
    /// No quantization - full precision float32
    None(Vec<f32>),
    /// Scalar quantization to int8
    Scalar {
        /// Quantized values
        data: Vec<u8>,
        /// Minimum value for dequantization
        min: f32,
        /// Scale factor for dequantization
        scale: f32,
    },
    /// Product quantization
    Product {
        /// Codebook indices
        codes: Vec<u8>,
        /// Number of subspaces
        subspaces: usize,
    },
    /// Binary quantization (1 bit per dimension)
    Binary {
        /// Packed binary data
        data: Vec<u8>,
        /// Threshold value
        threshold: f32,
        /// Number of original dimensions
        dimensions: usize,
    },
}

/// Quantize a vector using specified quantization type
pub fn quantize(vector: &[f32], qtype: QuantizationType) -> Result<QuantizedVector> {
    match qtype {
        QuantizationType::None => Ok(QuantizedVector::None(vector.to_vec())),
        QuantizationType::Scalar => Ok(scalar_quantize(vector)),
        QuantizationType::Product { subspaces, k } => product_quantize(vector, subspaces, k),
        QuantizationType::Binary => Ok(binary_quantize(vector)),
    }
}

/// Dequantize a quantized vector back to float32
pub fn dequantize(quantized: &QuantizedVector) -> Vec<f32> {
    match quantized {
        QuantizedVector::None(v) => v.clone(),
        QuantizedVector::Scalar { data, min, scale } => scalar_dequantize(data, *min, *scale),
        QuantizedVector::Product { codes, subspaces } => {
            // Placeholder - would need codebooks stored separately
            vec![0.0; codes.len() * (codes.len() / subspaces)]
        }
        QuantizedVector::Binary {
            data,
            threshold,
            dimensions,
        } => binary_dequantize(data, *threshold, *dimensions),
    }
}

/// Scalar quantization to int8
fn scalar_quantize(vector: &[f32]) -> QuantizedVector {
    let min = vector.iter().copied().fold(f32::INFINITY, f32::min);
    let max = vector.iter().copied().fold(f32::NEG_INFINITY, f32::max);

    let scale = if max > min { 255.0 / (max - min) } else { 1.0 };

    let data: Vec<u8> = vector
        .iter()
        .map(|&v| ((v - min) * scale).clamp(0.0, 255.0) as u8)
        .collect();

    QuantizedVector::Scalar { data, min, scale }
}

/// Dequantize scalar quantized vector
fn scalar_dequantize(data: &[u8], min: f32, scale: f32) -> Vec<f32> {
    // CRITICAL FIX: During quantization, we compute: quantized = (value - min) * scale
    // where scale = 255.0 / (max - min)
    // Therefore, dequantization must be: value = quantized / scale + min
    // which simplifies to: value = min + quantized * (max - min) / 255.0
    // Since scale = 255.0 / (max - min), then 1/scale = (max - min) / 255.0
    // So the correct formula is: value = min + quantized / scale
    data.iter().map(|&v| min + (v as f32) / scale).collect()
}

/// Product quantization (simplified version)
fn product_quantize(vector: &[f32], subspaces: usize, _k: usize) -> Result<QuantizedVector> {
    if !vector.len().is_multiple_of(subspaces) {
        return Err(VectorDbError::Quantization(
            "Vector length must be divisible by number of subspaces".to_string(),
        ));
    }

    // Simplified: just store subspace indices
    // In production, this would involve k-means clustering per subspace
    let subspace_dim = vector.len() / subspaces;
    let codes: Vec<u8> = (0..subspaces)
        .map(|i| {
            let start = i * subspace_dim;
            let subvec = &vector[start..start + subspace_dim];
            // Placeholder: hash to a code (0-255)
            (subvec.iter().sum::<f32>() as u32 % 256) as u8
        })
        .collect();

    Ok(QuantizedVector::Product { codes, subspaces })
}

/// Binary quantization (1 bit per dimension)
fn binary_quantize(vector: &[f32]) -> QuantizedVector {
    let threshold = vector.iter().sum::<f32>() / vector.len() as f32;
    let dimensions = vector.len();

    let num_bytes = dimensions.div_ceil(8);
    let mut data = vec![0u8; num_bytes];

    for (i, &val) in vector.iter().enumerate() {
        if val > threshold {
            let byte_idx = i / 8;
            let bit_idx = i % 8;
            data[byte_idx] |= 1 << bit_idx;
        }
    }

    QuantizedVector::Binary {
        data,
        threshold,
        dimensions,
    }
}

/// Dequantize binary quantized vector
fn binary_dequantize(data: &[u8], threshold: f32, dimensions: usize) -> Vec<f32> {
    let mut result = Vec::with_capacity(dimensions);

    for (i, &byte) in data.iter().enumerate() {
        for bit_idx in 0..8 {
            if result.len() >= dimensions {
                break;
            }
            let bit = (byte >> bit_idx) & 1;
            result.push(if bit == 1 {
                threshold + 1.0
            } else {
                threshold - 1.0
            });
        }
        if result.len() >= dimensions {
            break;
        }
    }

    result
}

/// Calculate memory savings from quantization
pub fn calculate_compression_ratio(original_dims: usize, qtype: QuantizationType) -> f32 {
    let original_bytes = original_dims * 4; // float32 = 4 bytes
    let quantized_bytes = match qtype {
        QuantizationType::None => original_bytes,
        QuantizationType::Scalar => original_dims + 8, // u8 per dim + min + scale
        QuantizationType::Product { subspaces, .. } => subspaces + 4, // u8 per subspace + overhead
        QuantizationType::Binary => original_dims.div_ceil(8) + 4, // 1 bit per dim + threshold
    };

    original_bytes as f32 / quantized_bytes as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scalar_quantization() {
        let vector = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let quantized = scalar_quantize(&vector);
        let dequantized = dequantize(&quantized);

        // Check approximate equality (quantization loses precision)
        for (orig, deq) in vector.iter().zip(dequantized.iter()) {
            assert!((orig - deq).abs() < 0.1);
        }
    }

    #[test]
    fn test_binary_quantization() {
        let vector = vec![1.0, 5.0, 2.0, 8.0, 3.0];
        let quantized = binary_quantize(&vector);

        match quantized {
            QuantizedVector::Binary {
                data, dimensions, ..
            } => {
                assert!(!data.is_empty());
                assert_eq!(dimensions, 5);
            }
            _ => panic!("Expected binary quantization"),
        }
    }

    #[test]
    fn test_compression_ratio() {
        let ratio = calculate_compression_ratio(384, QuantizationType::Scalar);
        assert!(ratio > 3.0); // Should be close to 4x

        let ratio = calculate_compression_ratio(384, QuantizationType::Binary);
        assert!(ratio > 20.0); // Should be close to 32x
    }

    #[test]
    fn test_scalar_quantization_roundtrip() {
        // Test that quantize -> dequantize produces values close to original
        let test_vectors = vec![
            vec![1.0, 2.0, 3.0, 4.0, 5.0],
            vec![-10.0, -5.0, 0.0, 5.0, 10.0],
            vec![0.1, 0.2, 0.3, 0.4, 0.5],
            vec![100.0, 200.0, 300.0, 400.0, 500.0],
        ];

        for vector in test_vectors {
            let quantized = scalar_quantize(&vector);
            let dequantized = dequantize(&quantized);

            assert_eq!(vector.len(), dequantized.len());

            for (orig, deq) in vector.iter().zip(dequantized.iter()) {
                // With 8-bit quantization, max error is roughly (max-min)/255
                let max = vector.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                let min = vector.iter().copied().fold(f32::INFINITY, f32::min);
                let max_error = (max - min) / 255.0 * 2.0; // Allow 2x for rounding

                assert!(
                    (orig - deq).abs() < max_error,
                    "Roundtrip error too large: orig={}, deq={}, error={}",
                    orig,
                    deq,
                    (orig - deq).abs()
                );
            }
        }
    }

    #[test]
    fn test_scalar_quantization_edge_cases() {
        // Test with all same values
        let same_values = vec![5.0, 5.0, 5.0, 5.0];
        let quantized = scalar_quantize(&same_values);
        let dequantized = dequantize(&quantized);

        for (orig, deq) in same_values.iter().zip(dequantized.iter()) {
            assert!((orig - deq).abs() < 0.01);
        }

        // Test with extreme ranges
        let extreme = vec![f32::MIN / 1e10, 0.0, f32::MAX / 1e10];
        let quantized = scalar_quantize(&extreme);
        let dequantized = dequantize(&quantized);

        assert_eq!(extreme.len(), dequantized.len());
    }

    #[test]
    fn test_binary_quantization_roundtrip() {
        let vector = vec![1.0, -1.0, 2.0, -2.0, 0.5, -0.5];
        let quantized = binary_quantize(&vector);
        let dequantized = dequantize(&quantized);

        // Binary quantization doesn't preserve exact values,
        // but should preserve the sign relative to threshold
        assert_eq!(
            vector.len(),
            dequantized.len(),
            "Dequantized vector should have same length as original"
        );

        match quantized {
            QuantizedVector::Binary {
                threshold,
                dimensions,
                ..
            } => {
                assert_eq!(dimensions, vector.len());
                for (orig, deq) in vector.iter().zip(dequantized.iter()) {
                    // Check that both have same relationship to threshold
                    let orig_above = orig > &threshold;
                    let deq_above = deq > &threshold;
                    assert_eq!(orig_above, deq_above);
                }
            }
            _ => panic!("Expected binary quantization"),
        }
    }
}
