//! Quantization utilities for embedding storage optimization.
//!
//! Provides F16 and INT8 quantization for reduced storage footprint
//! while maintaining acceptable precision for similarity search.
//!
//! ## Storage Comparison
//!
//! | Format | Bytes/Dim | Total (1536-D) | Precision Loss |
//! |--------|-----------|----------------|----------------|
//! | f32    | 4         | 6,144 bytes    | None (baseline)|
//! | f16    | 2         | 3,072 bytes    | ~0.1% typical  |
//! | i8     | 1         | 1,536 bytes    | ~1-2% typical  |

use half::f16;
use serde::{Deserialize, Serialize};

/// Quantize f32 embedding to f16 (half precision).
///
/// F16 provides 50% storage reduction with minimal precision loss.
/// Suitable for warm storage tier.
///
/// # Arguments
///
/// * `embedding` - The f32 embedding vector to quantize
///
/// # Returns
///
/// Vector of f16 values
///
/// # Example
///
/// ```rust
/// use sevensense_embedding::quantization::quantize_to_f16;
///
/// let embedding = vec![0.5, -0.3, 0.8];
/// let quantized = quantize_to_f16(&embedding);
/// assert_eq!(quantized.len(), embedding.len());
/// ```
#[must_use]
pub fn quantize_to_f16(embedding: &[f32]) -> Vec<f16> {
    embedding.iter().map(|&x| f16::from_f32(x)).collect()
}

/// Dequantize f16 embedding back to f32.
///
/// # Arguments
///
/// * `quantized` - The f16 quantized embedding
///
/// # Returns
///
/// Vector of f32 values
#[must_use]
pub fn dequantize_f16(quantized: &[f16]) -> Vec<f32> {
    quantized.iter().map(|&x| x.to_f32()).collect()
}

/// Quantization parameters for INT8 quantization
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct QuantizationParams {
    /// Scale factor for quantization
    pub scale: f32,
    /// Zero point for asymmetric quantization
    pub zero_point: f32,
    /// Minimum value in the original data
    pub min_val: f32,
    /// Maximum value in the original data
    pub max_val: f32,
}

impl QuantizationParams {
    /// Compute quantization parameters from data
    #[must_use]
    pub fn from_data(data: &[f32]) -> Self {
        if data.is_empty() {
            return Self {
                scale: 1.0,
                zero_point: 0.0,
                min_val: 0.0,
                max_val: 0.0,
            };
        }

        let min_val = data.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let max_val = data.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));

        // For symmetric quantization around zero (better for L2-normalized embeddings)
        let abs_max = min_val.abs().max(max_val.abs());
        let scale = if abs_max > 1e-12 {
            abs_max / 127.0
        } else {
            1.0
        };

        Self {
            scale,
            zero_point: 0.0, // Symmetric quantization
            min_val,
            max_val,
        }
    }

    /// Compute quantization parameters for asymmetric quantization
    #[must_use]
    pub fn from_data_asymmetric(data: &[f32]) -> Self {
        if data.is_empty() {
            return Self {
                scale: 1.0,
                zero_point: 0.0,
                min_val: 0.0,
                max_val: 0.0,
            };
        }

        let min_val = data.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let max_val = data.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));

        let range = max_val - min_val;
        let scale = if range > 1e-12 {
            range / 255.0
        } else {
            1.0
        };

        // Zero point maps min_val to 0 in quantized space
        let zero_point = -min_val / scale;

        Self {
            scale,
            zero_point,
            min_val,
            max_val,
        }
    }
}

/// Result of INT8 quantization including the quantized values and parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizedEmbedding {
    /// Quantized INT8 values
    pub values: Vec<i8>,
    /// Quantization parameters for dequantization
    pub params: QuantizationParams,
}

impl QuantizedEmbedding {
    /// Get the storage size in bytes
    #[must_use]
    pub fn size_bytes(&self) -> usize {
        // i8 values + params overhead
        self.values.len() + std::mem::size_of::<QuantizationParams>()
    }

    /// Dequantize back to f32
    #[must_use]
    pub fn dequantize(&self) -> Vec<f32> {
        dequantize_i8(&self.values, self.params.scale, self.params.zero_point)
    }
}

/// Quantize f32 embedding to INT8 with scale and zero point.
///
/// INT8 provides 75% storage reduction but with some precision loss.
/// Suitable for cold storage tier or large-scale deployments.
///
/// # Arguments
///
/// * `embedding` - The f32 embedding vector to quantize
///
/// # Returns
///
/// Tuple of (quantized values, scale, zero_point)
///
/// # Example
///
/// ```rust
/// use sevensense_embedding::quantization::quantize_to_i8;
///
/// let embedding = vec![0.5, -0.3, 0.8, -0.1];
/// let (quantized, scale, zero_point) = quantize_to_i8(&embedding);
/// assert_eq!(quantized.len(), embedding.len());
/// ```
#[must_use]
pub fn quantize_to_i8(embedding: &[f32]) -> (Vec<i8>, f32, f32) {
    let params = QuantizationParams::from_data(embedding);

    let quantized: Vec<i8> = embedding
        .iter()
        .map(|&x| {
            let q = (x / params.scale).round();
            q.clamp(-128.0, 127.0) as i8
        })
        .collect();

    (quantized, params.scale, params.zero_point)
}

/// Quantize f32 embedding to INT8 with full quantization info.
///
/// # Arguments
///
/// * `embedding` - The f32 embedding vector to quantize
///
/// # Returns
///
/// QuantizedEmbedding containing values and parameters
#[must_use]
pub fn quantize_to_i8_full(embedding: &[f32]) -> QuantizedEmbedding {
    let params = QuantizationParams::from_data(embedding);

    let values: Vec<i8> = embedding
        .iter()
        .map(|&x| {
            let q = (x / params.scale).round();
            q.clamp(-128.0, 127.0) as i8
        })
        .collect();

    QuantizedEmbedding { values, params }
}

/// Dequantize INT8 embedding back to f32.
///
/// # Arguments
///
/// * `quantized` - The INT8 quantized values
/// * `scale` - Scale factor used during quantization
/// * `zero_point` - Zero point used during quantization
///
/// # Returns
///
/// Vector of f32 values
///
/// # Example
///
/// ```rust
/// use sevensense_embedding::quantization::{quantize_to_i8, dequantize_i8};
///
/// let embedding = vec![0.5, -0.3, 0.8, -0.1];
/// let (quantized, scale, zero_point) = quantize_to_i8(&embedding);
/// let restored = dequantize_i8(&quantized, scale, zero_point);
///
/// // Check that values are close (within quantization error)
/// for (orig, rest) in embedding.iter().zip(restored.iter()) {
///     assert!((orig - rest).abs() < 0.05);
/// }
/// ```
#[must_use]
pub fn dequantize_i8(quantized: &[i8], scale: f32, zero_point: f32) -> Vec<f32> {
    quantized
        .iter()
        .map(|&q| (q as f32 - zero_point) * scale)
        .collect()
}

/// Quantize to unsigned INT8 (0-255 range) for asymmetric quantization
#[must_use]
pub fn quantize_to_u8(embedding: &[f32]) -> (Vec<u8>, f32, f32) {
    let params = QuantizationParams::from_data_asymmetric(embedding);

    let quantized: Vec<u8> = embedding
        .iter()
        .map(|&x| {
            let q = (x / params.scale + params.zero_point).round();
            q.clamp(0.0, 255.0) as u8
        })
        .collect();

    (quantized, params.scale, params.zero_point)
}

/// Dequantize unsigned INT8 back to f32
#[must_use]
pub fn dequantize_u8(quantized: &[u8], scale: f32, zero_point: f32) -> Vec<f32> {
    quantized
        .iter()
        .map(|&q| (q as f32 - zero_point) * scale)
        .collect()
}

/// Compute quantization error (MSE) between original and dequantized values
#[must_use]
pub fn compute_quantization_error(original: &[f32], dequantized: &[f32]) -> f32 {
    if original.len() != dequantized.len() || original.is_empty() {
        return f32::NAN;
    }

    let mse: f32 = original
        .iter()
        .zip(dequantized.iter())
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f32>()
        / original.len() as f32;

    mse
}

/// Compute cosine similarity preservation after quantization
///
/// Returns the ratio of cosine similarities (quantized / original)
#[must_use]
pub fn compute_cosine_preservation(
    original_a: &[f32],
    original_b: &[f32],
    dequant_a: &[f32],
    dequant_b: &[f32],
) -> f32 {
    let original_sim = cosine_similarity(original_a, original_b);
    let quant_sim = cosine_similarity(dequant_a, dequant_b);

    if original_sim.abs() < 1e-12 {
        return 1.0;
    }

    quant_sim / original_sim
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a * norm_b < 1e-12 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

/// Statistics about quantization quality
#[derive(Debug, Clone)]
pub struct QuantizationStats {
    /// Mean squared error
    pub mse: f32,
    /// Root mean squared error
    pub rmse: f32,
    /// Maximum absolute error
    pub max_error: f32,
    /// Mean absolute error
    pub mean_error: f32,
    /// Compression ratio (original size / quantized size)
    pub compression_ratio: f32,
}

impl QuantizationStats {
    /// Compute statistics comparing original and dequantized embeddings
    #[must_use]
    pub fn compute(original: &[f32], dequantized: &[f32], quantized_bytes: usize) -> Self {
        let mse = compute_quantization_error(original, dequantized);
        let rmse = mse.sqrt();

        let errors: Vec<f32> = original
            .iter()
            .zip(dequantized.iter())
            .map(|(a, b)| (a - b).abs())
            .collect();

        let max_error = errors.iter().fold(0.0f32, |a, &b| a.max(b));
        let mean_error = errors.iter().sum::<f32>() / errors.len().max(1) as f32;

        let original_bytes = original.len() * std::mem::size_of::<f32>();
        let compression_ratio = original_bytes as f32 / quantized_bytes.max(1) as f32;

        Self {
            mse,
            rmse,
            max_error,
            mean_error,
            compression_ratio,
        }
    }
}

/// Batch quantization for multiple embeddings
pub struct BatchQuantizer {
    /// Whether to use symmetric quantization
    pub symmetric: bool,
    /// Target precision
    pub precision: QuantizationPrecision,
}

/// Supported quantization precisions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantizationPrecision {
    /// 16-bit floating point
    F16,
    /// 8-bit signed integer (symmetric)
    Int8,
    /// 8-bit unsigned integer (asymmetric)
    UInt8,
}

impl BatchQuantizer {
    /// Create a new batch quantizer
    #[must_use]
    pub fn new(precision: QuantizationPrecision) -> Self {
        Self {
            symmetric: matches!(precision, QuantizationPrecision::Int8),
            precision,
        }
    }

    /// Quantize a batch of embeddings
    pub fn quantize_batch(&self, embeddings: &[Vec<f32>]) -> Vec<QuantizedEmbedding> {
        embeddings
            .iter()
            .map(|emb| match self.precision {
                QuantizationPrecision::F16 => {
                    let f16_vals = quantize_to_f16(emb);
                    // Store f16 as i8 pairs for uniform interface
                    let bytes: Vec<i8> = f16_vals
                        .iter()
                        .flat_map(|v| {
                            let bits = v.to_bits();
                            [(bits & 0xFF) as i8, ((bits >> 8) & 0xFF) as i8]
                        })
                        .collect();
                    QuantizedEmbedding {
                        values: bytes,
                        params: QuantizationParams {
                            scale: 1.0,
                            zero_point: 0.0,
                            min_val: 0.0,
                            max_val: 0.0,
                        },
                    }
                }
                QuantizationPrecision::Int8 | QuantizationPrecision::UInt8 => {
                    quantize_to_i8_full(emb)
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_f16_roundtrip() {
        let original = vec![0.5, -0.3, 0.8, -0.1, 0.0, 1.0, -1.0];
        let quantized = quantize_to_f16(&original);
        let restored = dequantize_f16(&quantized);

        for (orig, rest) in original.iter().zip(restored.iter()) {
            assert!((orig - rest).abs() < 0.01, "F16 roundtrip error too large");
        }
    }

    #[test]
    fn test_i8_roundtrip() {
        let original = vec![0.5, -0.3, 0.8, -0.1, 0.0, 0.9, -0.9];
        let (quantized, scale, zero_point) = quantize_to_i8(&original);
        let restored = dequantize_i8(&quantized, scale, zero_point);

        for (orig, rest) in original.iter().zip(restored.iter()) {
            // INT8 has larger quantization error
            assert!((orig - rest).abs() < 0.02, "I8 roundtrip error too large");
        }
    }

    #[test]
    fn test_u8_roundtrip() {
        let original = vec![0.1, 0.3, 0.5, 0.7, 0.9];
        let (quantized, scale, zero_point) = quantize_to_u8(&original);
        let restored = dequantize_u8(&quantized, scale, zero_point);

        for (orig, rest) in original.iter().zip(restored.iter()) {
            assert!((orig - rest).abs() < 0.02, "U8 roundtrip error too large");
        }
    }

    #[test]
    fn test_quantization_params() {
        let data = vec![-0.5, 0.0, 0.5, 1.0];
        let params = QuantizationParams::from_data(&data);

        assert!(params.scale > 0.0);
        assert_eq!(params.min_val, -0.5);
        assert_eq!(params.max_val, 1.0);
    }

    #[test]
    fn test_quantization_error() {
        let original = vec![0.5, -0.3, 0.8];
        let modified = vec![0.51, -0.29, 0.79];
        let error = compute_quantization_error(&original, &modified);
        assert!(error < 0.001);
    }

    #[test]
    fn test_cosine_preservation() {
        let a = vec![0.6, 0.8, 0.0];
        let b = vec![0.0, 0.6, 0.8];

        // Slightly perturbed versions
        let a_quant = vec![0.61, 0.79, 0.01];
        let b_quant = vec![0.01, 0.59, 0.81];

        let preservation = compute_cosine_preservation(&a, &b, &a_quant, &b_quant);
        // Should be close to 1.0 if quantization preserves cosine similarity
        assert!(preservation > 0.95 && preservation < 1.05);
    }

    #[test]
    fn test_quantization_stats() {
        let original = vec![0.5, -0.3, 0.8, -0.1];
        let (quantized, scale, zero_point) = quantize_to_i8(&original);
        let restored = dequantize_i8(&quantized, scale, zero_point);

        let stats = QuantizationStats::compute(&original, &restored, quantized.len());

        assert!(stats.mse >= 0.0);
        assert!(stats.rmse >= 0.0);
        assert!(stats.compression_ratio > 1.0); // Should compress
    }

    #[test]
    fn test_batch_quantizer() {
        let embeddings = vec![
            vec![0.5, -0.3, 0.8],
            vec![-0.1, 0.2, 0.9],
        ];

        let quantizer = BatchQuantizer::new(QuantizationPrecision::Int8);
        let quantized = quantizer.quantize_batch(&embeddings);

        assert_eq!(quantized.len(), 2);
        assert_eq!(quantized[0].values.len(), 3);
    }

    #[test]
    fn test_quantized_embedding_dequantize() {
        let original = vec![0.5, -0.3, 0.8, -0.1];
        let quantized = quantize_to_i8_full(&original);
        let restored = quantized.dequantize();

        assert_eq!(restored.len(), original.len());
        for (orig, rest) in original.iter().zip(restored.iter()) {
            assert!((orig - rest).abs() < 0.02);
        }
    }

    #[test]
    fn test_empty_input() {
        let empty: Vec<f32> = vec![];

        let f16_result = quantize_to_f16(&empty);
        assert!(f16_result.is_empty());

        let (i8_result, _, _) = quantize_to_i8(&empty);
        assert!(i8_result.is_empty());

        let params = QuantizationParams::from_data(&empty);
        assert_eq!(params.scale, 1.0);
    }
}
