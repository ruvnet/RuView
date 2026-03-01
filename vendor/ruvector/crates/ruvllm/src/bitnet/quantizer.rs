//! PT-BitNet Post-Training Quantization
//!
//! Core absmean ternary quantization algorithm for converting FP32 weights
//! to BitNet b1.58 ternary format.

use super::ternary_tensor::{pack_ternary, TernaryTensor};
use crate::error::{Result, RuvLLMError};

/// Configuration for PT-BitNet post-training quantization.
///
/// Controls the quantization process behavior, including block size,
/// calibration, and layer selection.
///
/// # Example
///
/// ```rust,ignore
/// use ruvllm::bitnet::PtBitnetConfig;
///
/// let config = PtBitnetConfig {
///     calibration_samples: 1000,
///     block_size: 256,
///     optimize_scales: true,
///     layers_to_quantize: LayerMask::ExpertsOnly,
///     export_format: TernaryFormat::BitnetT158,
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone)]
pub struct PtBitnetConfig {
    /// Number of calibration samples for scale optimization
    pub calibration_samples: usize,
    /// Elements per quantization block
    pub block_size: usize,
    /// Enable scale factor optimization via calibration
    pub optimize_scales: bool,
    /// Which layers to quantize
    pub layers_to_quantize: LayerMask,
    /// Export format for GGUF serialization
    pub export_format: TernaryFormat,
    /// Precision for router and shared layers
    pub router_precision: Precision,
    /// Use memory-mapped I/O for weight loading
    pub use_mmap: bool,
    /// Use Metal GPU for calibration (Mac Studio only)
    pub use_metal_calibration: bool,
    /// Maximum memory budget in GB
    pub max_memory_gb: usize,
}

impl Default for PtBitnetConfig {
    fn default() -> Self {
        Self {
            calibration_samples: 1000,
            block_size: 256,
            optimize_scales: true,
            layers_to_quantize: LayerMask::ExpertsOnly,
            export_format: TernaryFormat::BitnetT158,
            router_precision: Precision::FP16,
            use_mmap: true,
            use_metal_calibration: cfg!(all(target_os = "macos", feature = "metal-compute")),
            max_memory_gb: 64,
        }
    }
}

/// Layer selection mask for quantization.
///
/// Determines which model layers to convert to ternary. Per ADR-017 (AD-2),
/// the MoE router, embeddings, and LM head must remain in higher precision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayerMask {
    /// Only MoE expert FFN layers (recommended for Phase 1)
    ExpertsOnly,
    /// All linear layers except router/embeddings/head
    All,
    /// Custom layer selection by name pattern
    Custom(Vec<String>),
}

/// Ternary tensor export format.
///
/// Determines the GGUF quantization type used for serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TernaryFormat {
    /// BitNet b1.58 native format (type 30)
    BitnetT158,
    /// IQ1_S compatible format (type 19)
    IQ1S,
}

/// Precision for non-quantized layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precision {
    /// 16-bit floating point
    FP16,
    /// Brain floating point 16
    BF16,
    /// 32-bit floating point
    FP32,
}

/// Core absmean ternary quantization algorithm.
///
/// Implements the BitNet b1.58 quantization formula:
/// ```text
/// gamma = mean(|block|) + epsilon
/// normalized = block / gamma
/// ternary = round(clamp(normalized, -1, 1))
/// ```
///
/// # Arguments
///
/// * `block` - FP32 weight block (typically 256 elements)
///
/// # Returns
///
/// Tuple of (ternary values, scale factor):
/// - `Vec<i8>`: Ternary weights in {-1, 0, +1}
/// - `f32`: Absmean scale factor (gamma)
///
/// # Example
///
/// ```rust,ignore
/// use ruvllm::bitnet::absmean_ternary;
///
/// let weights = vec![0.5, -0.3, 0.8, -0.1, 0.0, 0.4];
/// let (ternary, scale) = absmean_ternary(&weights);
///
/// println!("Scale: {}", scale);
/// println!("Ternary: {:?}", ternary);  // e.g., [1, -1, 1, 0, 0, 1]
/// ```
pub fn absmean_ternary(block: &[f32]) -> (Vec<i8>, f32) {
    // Guard: empty block returns empty ternary with epsilon scale
    if block.is_empty() {
        return (vec![], 1e-8);
    }

    // Compute absmean scale: gamma = mean(|W|)
    let sum_abs: f32 = block.iter().map(|&w| w.abs()).sum();
    let gamma = (sum_abs / block.len() as f32) + 1e-8;

    // Normalize and quantize to {-1, 0, +1}
    let ternary: Vec<i8> = block
        .iter()
        .map(|&w| {
            let normalized = w / gamma;
            let clamped = normalized.clamp(-1.0, 1.0);
            clamped.round() as i8
        })
        .collect();

    (ternary, gamma)
}

/// Quantize a full FP32 tensor to ternary representation.
///
/// Processes the input tensor in blocks of `config.block_size`, applying
/// absmean quantization to each block independently.
///
/// # Arguments
///
/// * `weights` - FP32 weight tensor (flattened)
/// * `shape` - Tensor shape (rows, cols)
/// * `config` - Quantization configuration
///
/// # Returns
///
/// `TernaryTensor` with packed 2-bit data and per-block scales
///
/// # Errors
///
/// Returns an error if the weight dimensions are invalid.
///
/// # Example
///
/// ```rust,ignore
/// use ruvllm::bitnet::{quantize_tensor, PtBitnetConfig};
///
/// let weights = vec![0.5; 512];  // 512 FP32 weights
/// let shape = (2, 256);
/// let config = PtBitnetConfig::default();
///
/// let ternary = quantize_tensor(&weights, shape, &config)?;
/// println!("Compressed to {} bytes", ternary.memory_bytes());
/// ```
pub fn quantize_tensor(
    weights: &[f32],
    shape: (usize, usize),
    config: &PtBitnetConfig,
) -> Result<TernaryTensor> {
    let (rows, cols) = shape;

    if rows == 0 || cols == 0 {
        return Err(RuvLLMError::Model(format!(
            "Invalid tensor shape: dimensions must be non-zero, got {:?}",
            shape
        )));
    }

    let block_size = config.block_size;
    if block_size == 0 {
        return Err(RuvLLMError::Model(
            "block_size must be non-zero".to_string(),
        ));
    }

    let total_elements = rows.checked_mul(cols).ok_or_else(|| {
        RuvLLMError::Model(format!(
            "Integer overflow computing total elements for shape {:?}",
            shape
        ))
    })?;

    if weights.len() != total_elements {
        return Err(RuvLLMError::Model(format!(
            "Weight size mismatch: expected {} elements for shape {:?}, got {}",
            total_elements,
            shape,
            weights.len()
        )));
    }

    // Use checked arithmetic to prevent overflow in block count
    let num_blocks = total_elements.checked_add(block_size - 1).ok_or_else(|| {
        RuvLLMError::Model("Integer overflow in block count calculation".to_string())
    })? / block_size;

    let mut all_ternary = Vec::with_capacity(total_elements);
    let mut scales = Vec::with_capacity(num_blocks);

    // Process each block
    for block_idx in 0..num_blocks {
        let start = block_idx * block_size;
        let end = (start + block_size).min(total_elements);
        let block = &weights[start..end];

        let (ternary, scale) = absmean_ternary(block);
        all_ternary.extend_from_slice(&ternary);
        scales.push(scale);
    }

    // Pack ternary values into 2-bit representation
    let packed_data = pack_ternary(&all_ternary);

    Ok(TernaryTensor {
        packed_data,
        scales,
        shape,
        block_size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_absmean_ternary_simple() {
        // Simple block with known values
        let block = vec![0.5, -0.5, 0.0, 1.0, -1.0, 0.25];
        let (ternary, scale) = absmean_ternary(&block);

        // All values should be in {-1, 0, +1}
        assert!(ternary.iter().all(|&v| v >= -1 && v <= 1));

        // Scale should be positive
        assert!(scale > 0.0);

        // Check specific values
        // gamma ≈ (0.5 + 0.5 + 0.0 + 1.0 + 1.0 + 0.25) / 6 ≈ 0.542
        // 0.5 / 0.542 ≈ 0.92 → round(0.92) = 1
        // -0.5 / 0.542 ≈ -0.92 → round(-0.92) = -1
        // 0.0 / 0.542 = 0 → round(0) = 0
        assert_eq!(ternary[0], 1);
        assert_eq!(ternary[1], -1);
        assert_eq!(ternary[2], 0);
    }

    #[test]
    fn test_absmean_ternary_all_zeros() {
        let block = vec![0.0; 256];
        let (ternary, scale) = absmean_ternary(&block);

        // All should quantize to 0
        assert!(ternary.iter().all(|&v| v == 0));

        // Scale should be epsilon (1e-8)
        assert!(scale < 1e-7 && scale > 0.0);
    }

    #[test]
    fn test_absmean_ternary_large_values() {
        let block = vec![10.0, -10.0, 5.0, -5.0];
        let (ternary, _scale) = absmean_ternary(&block);

        // All should saturate to ±1
        assert!(ternary[0] == 1 || ternary[0] == -1);
        assert!(ternary[1] == 1 || ternary[1] == -1);
    }

    #[test]
    fn test_quantize_tensor_simple() {
        let weights = vec![0.5; 512]; // 512 identical weights
        let shape = (2, 256);
        let config = PtBitnetConfig::default();

        let ternary = quantize_tensor(&weights, shape, &config).unwrap();

        assert_eq!(ternary.shape, shape);
        assert_eq!(ternary.block_size, 256);
        assert_eq!(ternary.num_blocks(), 2); // 512 / 256 = 2 blocks
        assert_eq!(ternary.scales.len(), 2);

        // 512 elements packed in 2 bits each = 128 bytes
        assert_eq!(ternary.packed_data.len(), 128);
    }

    #[test]
    fn test_quantize_tensor_size_mismatch() {
        let weights = vec![0.5; 100]; // Wrong size
        let shape = (2, 256); // Expects 512
        let config = PtBitnetConfig::default();

        let result = quantize_tensor(&weights, shape, &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_quantize_tensor_memory_savings() {
        // Quantize a 1MB FP32 tensor (256K elements)
        let weights = vec![0.5; 256 * 1024];
        let shape = (512, 512);
        let config = PtBitnetConfig::default();

        let ternary = quantize_tensor(&weights, shape, &config).unwrap();

        let original_bytes = weights.len() * 4; // FP32
        let compressed_bytes = ternary.memory_bytes();

        // Should be ~16x compression (32 bits → 2 bits + scale overhead)
        let compression_ratio = original_bytes as f32 / compressed_bytes as f32;
        assert!(compression_ratio > 10.0); // At least 10x compression
        assert!(compression_ratio < 20.0); // Less than 20x (due to scales)
    }

    #[test]
    fn test_config_default() {
        let config = PtBitnetConfig::default();
        assert_eq!(config.block_size, 256);
        assert_eq!(config.calibration_samples, 1000);
        assert!(config.optimize_scales);
        assert_eq!(config.layers_to_quantize, LayerMask::ExpertsOnly);
    }

    #[test]
    fn test_layer_mask_variants() {
        let experts = LayerMask::ExpertsOnly;
        let all = LayerMask::All;
        let custom = LayerMask::Custom(vec!["layer.0".to_string()]);

        assert_ne!(experts, all);
        assert_ne!(all, custom);
        assert_ne!(experts, custom);
    }
}
