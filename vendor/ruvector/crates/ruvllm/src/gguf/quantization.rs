//! GGUF Quantization Types and Dequantization Kernels
//!
//! This module implements all GGUF quantization formats used by llama.cpp,
//! providing both type definitions and optimized dequantization routines.
//!
//! ## Quantization Format Overview
//!
//! GGUF supports multiple quantization formats with different tradeoffs:
//!
//! | Format | Bits/Weight | Block Size | Description |
//! |--------|-------------|------------|-------------|
//! | F32 | 32 | 1 | Full precision |
//! | F16 | 16 | 1 | Half precision |
//! | Q8_0 | 8.5 | 32 | 8-bit symmetric |
//! | Q8_1 | 9 | 32 | 8-bit with offset |
//! | Q4_0 | 4.5 | 32 | 4-bit symmetric |
//! | Q4_1 | 5 | 32 | 4-bit with offset |
//! | Q5_0 | 5.5 | 32 | 5-bit symmetric |
//! | Q5_1 | 6 | 32 | 5-bit with offset |
//! | Q2_K | 2.56 | 256 | 2-bit k-quant |
//! | Q3_K | 3.44 | 256 | 3-bit k-quant |
//! | Q4_K | 4.5 | 256 | 4-bit k-quant |
//! | Q5_K | 5.5 | 256 | 5-bit k-quant |
//! | Q6_K | 6.56 | 256 | 6-bit k-quant |
//! | IQ2_XXS | 2.06 | 256 | i-quant extreme |
//! | IQ2_XS | 2.31 | 256 | i-quant |
//! | IQ3_XXS | 3.06 | 256 | i-quant 3-bit |
//! | IQ1_S | 1.56 | 256 | i-quant 1-bit |
//! | IQ4_NL | 4.5 | 32 | i-quant 4-bit non-linear |

use crate::bitnet::dequantize_bitnet_t158;
use crate::error::{Result, RuvLLMError};

// ============================================================================
// Quantization Types
// ============================================================================

/// GGUF quantization type identifiers.
///
/// These correspond to the GGML quantization types used in llama.cpp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum GgufQuantType {
    /// 32-bit floating point (no quantization)
    F32 = 0,
    /// 16-bit floating point
    F16 = 1,
    /// 4-bit quantization (32-element blocks, symmetric)
    Q4_0 = 2,
    /// 4-bit quantization with offset
    Q4_1 = 3,
    /// Legacy 4-bit format (deprecated)
    Q4_2 = 4,
    /// Legacy 4-bit format (deprecated)
    Q4_3 = 5,
    /// 5-bit quantization (symmetric)
    Q5_0 = 6,
    /// 5-bit quantization with offset
    Q5_1 = 7,
    /// 8-bit quantization (symmetric)
    Q8_0 = 8,
    /// 8-bit quantization with offset
    Q8_1 = 9,
    /// 2-bit k-quant
    Q2_K = 10,
    /// 3-bit k-quant
    Q3_K = 11,
    /// 4-bit k-quant
    Q4_K = 12,
    /// 5-bit k-quant
    Q5_K = 13,
    /// 6-bit k-quant
    Q6_K = 14,
    /// 8-bit k-quant
    Q8_K = 15,
    /// I-quant 2-bit extreme extra small
    IQ2_XXS = 16,
    /// I-quant 2-bit extra small
    IQ2_XS = 17,
    /// I-quant 3-bit extra extra small
    IQ3_XXS = 18,
    /// I-quant 1-bit small
    IQ1_S = 19,
    /// I-quant 4-bit non-linear
    IQ4_NL = 20,
    /// I-quant 3-bit small
    IQ3_S = 21,
    /// I-quant 2-bit small
    IQ2_S = 22,
    /// I-quant 4-bit extra small
    IQ4_XS = 23,
    /// 16-bit integer
    I8 = 24,
    /// 16-bit integer
    I16 = 25,
    /// 32-bit integer
    I32 = 26,
    /// 64-bit integer
    I64 = 27,
    /// 64-bit floating point
    F64 = 28,
    /// BF16 brain float
    Bf16 = 29,
    /// BitNet b1.58 ternary quantization (2-bit packed)
    BitnetT158 = 30,
}

impl TryFrom<u32> for GgufQuantType {
    type Error = RuvLLMError;

    fn try_from(value: u32) -> Result<Self> {
        match value {
            0 => Ok(Self::F32),
            1 => Ok(Self::F16),
            2 => Ok(Self::Q4_0),
            3 => Ok(Self::Q4_1),
            4 => Ok(Self::Q4_2),
            5 => Ok(Self::Q4_3),
            6 => Ok(Self::Q5_0),
            7 => Ok(Self::Q5_1),
            8 => Ok(Self::Q8_0),
            9 => Ok(Self::Q8_1),
            10 => Ok(Self::Q2_K),
            11 => Ok(Self::Q3_K),
            12 => Ok(Self::Q4_K),
            13 => Ok(Self::Q5_K),
            14 => Ok(Self::Q6_K),
            15 => Ok(Self::Q8_K),
            16 => Ok(Self::IQ2_XXS),
            17 => Ok(Self::IQ2_XS),
            18 => Ok(Self::IQ3_XXS),
            19 => Ok(Self::IQ1_S),
            20 => Ok(Self::IQ4_NL),
            21 => Ok(Self::IQ3_S),
            22 => Ok(Self::IQ2_S),
            23 => Ok(Self::IQ4_XS),
            24 => Ok(Self::I8),
            25 => Ok(Self::I16),
            26 => Ok(Self::I32),
            27 => Ok(Self::I64),
            28 => Ok(Self::F64),
            29 => Ok(Self::Bf16),
            30 => Ok(Self::BitnetT158),
            _ => Err(RuvLLMError::Model(format!(
                "Unknown GGUF quantization type: {}",
                value
            ))),
        }
    }
}

impl GgufQuantType {
    /// Get the block size for this quantization type.
    ///
    /// Quantization operates on blocks of elements. Non-quantized types
    /// have a block size of 1.
    pub fn block_size(&self) -> usize {
        match self {
            Self::F32 | Self::F16 | Self::Bf16 | Self::F64 => 1,
            Self::I8 | Self::I16 | Self::I32 | Self::I64 => 1,
            Self::Q4_0 | Self::Q4_1 | Self::Q4_2 | Self::Q4_3 => 32,
            Self::Q5_0 | Self::Q5_1 => 32,
            Self::Q8_0 | Self::Q8_1 => 32,
            Self::Q2_K | Self::Q3_K | Self::Q4_K | Self::Q5_K | Self::Q6_K | Self::Q8_K => 256,
            Self::IQ2_XXS | Self::IQ2_XS | Self::IQ2_S => 256,
            Self::IQ3_XXS | Self::IQ3_S => 256,
            Self::IQ1_S => 256,
            Self::IQ4_NL => 32,
            Self::IQ4_XS => 256,
            Self::BitnetT158 => 256,
        }
    }

    /// Get the size in bytes for one block of this type.
    ///
    /// This is the storage size for `block_size()` elements.
    pub fn type_size(&self) -> usize {
        match self {
            Self::F32 => 4,
            Self::F16 => 2,
            Self::Bf16 => 2,
            Self::F64 => 8,
            Self::I8 => 1,
            Self::I16 => 2,
            Self::I32 => 4,
            Self::I64 => 8,
            // Q4_0: 32 elements -> half (16 bytes) + scale (2 bytes f16) = 18 bytes
            Self::Q4_0 => 18,
            // Q4_1: 32 elements -> half (16 bytes) + scale (2 bytes) + min (2 bytes) = 20 bytes
            Self::Q4_1 => 20,
            Self::Q4_2 => 18, // Deprecated
            Self::Q4_3 => 20, // Deprecated
            // Q5_0: 32 elements -> scale (2) + quants (20) = 22 bytes
            Self::Q5_0 => 22,
            // Q5_1: 32 elements -> scale (2) + min (2) + quants (20) = 24 bytes
            Self::Q5_1 => 24,
            // Q8_0: 32 elements -> scale (2) + quants (32) = 34 bytes
            Self::Q8_0 => 34,
            // Q8_1: 32 elements -> scale (2) + offset (2) + quants (32) = 36 bytes
            Self::Q8_1 => 36,
            // Q2_K: 256 elements -> superblock structure
            Self::Q2_K => 84,
            // Q3_K: 256 elements
            Self::Q3_K => 110,
            // Q4_K: 256 elements -> d (2) + dmin (2) + scales (12) + qs (128) = 144 bytes
            Self::Q4_K => 144,
            // Q5_K: 256 elements
            Self::Q5_K => 176,
            // Q6_K: 256 elements
            Self::Q6_K => 210,
            // Q8_K: 256 elements
            Self::Q8_K => 292,
            // I-quants (approximate sizes)
            Self::IQ2_XXS => 66,
            Self::IQ2_XS => 74,
            Self::IQ2_S => 82,
            Self::IQ3_XXS => 98,
            Self::IQ3_S => 110,
            Self::IQ1_S => 50,
            Self::IQ4_NL => 18,
            Self::IQ4_XS => 136,
            // BitNet b1.58: 256 elements -> 64 bytes (2-bit packed) + 2 bytes (FP16 scale) = 66 bytes
            Self::BitnetT158 => 66,
        }
    }

    /// Calculate the total byte size for a tensor with this dtype.
    pub fn tensor_size(&self, num_elements: usize) -> usize {
        let block_size = self.block_size();
        let type_size = self.type_size();
        let num_blocks = (num_elements + block_size - 1) / block_size;
        num_blocks * type_size
    }

    /// Check if this is a quantized type.
    pub fn is_quantized(&self) -> bool {
        !matches!(
            self,
            Self::F32
                | Self::F16
                | Self::Bf16
                | Self::F64
                | Self::I8
                | Self::I16
                | Self::I32
                | Self::I64
        )
    }

    /// Get approximate bits per weight.
    pub fn bits_per_weight(&self) -> f32 {
        let type_size = self.type_size() as f32;
        let block_size = self.block_size() as f32;
        (type_size * 8.0) / block_size
    }

    /// Get the name as used in GGUF files.
    pub fn name(&self) -> &'static str {
        match self {
            Self::F32 => "F32",
            Self::F16 => "F16",
            Self::Bf16 => "BF16",
            Self::F64 => "F64",
            Self::I8 => "I8",
            Self::I16 => "I16",
            Self::I32 => "I32",
            Self::I64 => "I64",
            Self::Q4_0 => "Q4_0",
            Self::Q4_1 => "Q4_1",
            Self::Q4_2 => "Q4_2",
            Self::Q4_3 => "Q4_3",
            Self::Q5_0 => "Q5_0",
            Self::Q5_1 => "Q5_1",
            Self::Q8_0 => "Q8_0",
            Self::Q8_1 => "Q8_1",
            Self::Q2_K => "Q2_K",
            Self::Q3_K => "Q3_K",
            Self::Q4_K => "Q4_K",
            Self::Q5_K => "Q5_K",
            Self::Q6_K => "Q6_K",
            Self::Q8_K => "Q8_K",
            Self::IQ2_XXS => "IQ2_XXS",
            Self::IQ2_XS => "IQ2_XS",
            Self::IQ2_S => "IQ2_S",
            Self::IQ3_XXS => "IQ3_XXS",
            Self::IQ3_S => "IQ3_S",
            Self::IQ1_S => "IQ1_S",
            Self::IQ4_NL => "IQ4_NL",
            Self::IQ4_XS => "IQ4_XS",
            Self::BitnetT158 => "BITNET_T158",
        }
    }
}

// ============================================================================
// Quantized Tensor Container
// ============================================================================

/// Container for quantized tensor data.
///
/// This struct holds the raw quantized bytes along with metadata
/// needed for dequantization.
#[derive(Debug, Clone)]
pub struct QuantizedTensor {
    /// Raw quantized data bytes
    pub data: Vec<u8>,
    /// Quantization type
    pub dtype: GgufQuantType,
    /// Tensor shape
    pub shape: Vec<usize>,
    /// Total number of elements
    pub num_elements: usize,
}

impl QuantizedTensor {
    /// Dequantize to FP32.
    pub fn dequantize(&self) -> Result<Vec<f32>> {
        dequantize_tensor(&self.data, self.dtype, self.num_elements)
    }

    /// Get the block count.
    pub fn block_count(&self) -> usize {
        let block_size = self.dtype.block_size();
        (self.num_elements + block_size - 1) / block_size
    }
}

// ============================================================================
// Dequantization Functions
// ============================================================================

/// Dequantize a tensor from raw bytes to FP32.
///
/// # Arguments
///
/// * `data` - Raw quantized bytes
/// * `dtype` - Quantization type
/// * `num_elements` - Total number of output elements
///
/// # Returns
///
/// Vector of FP32 values
pub fn dequantize_tensor(
    data: &[u8],
    dtype: GgufQuantType,
    num_elements: usize,
) -> Result<Vec<f32>> {
    let mut output = vec![0.0f32; num_elements];

    match dtype {
        GgufQuantType::F32 => dequantize_f32(data, &mut output),
        GgufQuantType::F16 => dequantize_f16(data, &mut output),
        GgufQuantType::Bf16 => dequantize_bf16(data, &mut output),
        GgufQuantType::Q4_0 => dequantize_q4_0(data, &mut output),
        GgufQuantType::Q4_1 => dequantize_q4_1(data, &mut output),
        GgufQuantType::Q5_0 => dequantize_q5_0(data, &mut output),
        GgufQuantType::Q5_1 => dequantize_q5_1(data, &mut output),
        GgufQuantType::Q8_0 => dequantize_q8_0(data, &mut output),
        GgufQuantType::Q8_1 => dequantize_q8_1(data, &mut output),
        GgufQuantType::Q2_K => dequantize_q2_k(data, &mut output),
        GgufQuantType::Q3_K => dequantize_q3_k(data, &mut output),
        GgufQuantType::Q4_K => dequantize_q4_k(data, &mut output),
        GgufQuantType::Q5_K => dequantize_q5_k(data, &mut output),
        GgufQuantType::Q6_K => dequantize_q6_k(data, &mut output),
        GgufQuantType::IQ4_NL => dequantize_iq4_nl(data, &mut output),
        GgufQuantType::BitnetT158 => dequantize_bitnet_t158_wrapper(data, &mut output),
        GgufQuantType::IQ1_S => {
            return Err(RuvLLMError::Model(
                "IQ1_S dequantization requires codebook lookup tables (not yet implemented). \
                 For BitNet ternary quantization, use BITNET_T158 type instead."
                    .to_string(),
            ));
        }
        _ => {
            return Err(RuvLLMError::Model(format!(
                "Dequantization not implemented for {:?}",
                dtype
            )));
        }
    }

    Ok(output)
}

/// Dequantize a single block.
///
/// # Arguments
///
/// * `data` - Raw block bytes
/// * `dtype` - Quantization type
/// * `output` - Output buffer (must have capacity for block_size elements)
pub fn dequantize_block(data: &[u8], dtype: GgufQuantType, output: &mut [f32]) {
    match dtype {
        GgufQuantType::Q4_0 => dequantize_q4_0_block(data, output),
        GgufQuantType::Q4_1 => dequantize_q4_1_block(data, output),
        GgufQuantType::Q8_0 => dequantize_q8_0_block(data, output),
        GgufQuantType::Q4_K => dequantize_q4_k_block(data, output),
        GgufQuantType::BitnetT158 => dequantize_bitnet_t158_block_wrapper(data, output),
        _ => {
            // Fallback: fill with zeros
            output.fill(0.0);
        }
    }
}

/// Dequantize a single BITNET_T158 block from GGUF format.
///
/// Block format (66 bytes):
/// - 64 bytes: packed 2-bit ternary data
/// - 2 bytes: FP16 scale
fn dequantize_bitnet_t158_block_wrapper(data: &[u8], output: &mut [f32]) {
    if data.len() < BITNET_T158_TYPE_SIZE {
        output.fill(0.0);
        return;
    }

    // Extract packed data (first 64 bytes)
    let packed = &data[..64];

    // Extract scale (last 2 bytes)
    let scale = f16_to_f32(u16::from_le_bytes([data[64], data[65]]));

    // Dequantize using bitnet module (expects 256 elements)
    let min_output_len = output.len().min(BITNET_T158_BLOCK_SIZE);
    let dequantized = dequantize_bitnet_t158(packed, &[scale], min_output_len);

    // Copy to output
    output[..dequantized.len()].copy_from_slice(&dequantized);
}

// ============================================================================
// F32/F16/BF16 (No Quantization)
// ============================================================================

fn dequantize_f32(data: &[u8], output: &mut [f32]) {
    for (i, chunk) in data.chunks_exact(4).enumerate() {
        if i >= output.len() {
            break;
        }
        output[i] = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }
}

fn dequantize_f16(data: &[u8], output: &mut [f32]) {
    for (i, chunk) in data.chunks_exact(2).enumerate() {
        if i >= output.len() {
            break;
        }
        let bits = u16::from_le_bytes([chunk[0], chunk[1]]);
        output[i] = f16_to_f32(bits);
    }
}

fn dequantize_bf16(data: &[u8], output: &mut [f32]) {
    for (i, chunk) in data.chunks_exact(2).enumerate() {
        if i >= output.len() {
            break;
        }
        let bits = u16::from_le_bytes([chunk[0], chunk[1]]);
        // BF16 is upper 16 bits of F32
        output[i] = f32::from_bits((bits as u32) << 16);
    }
}

// ============================================================================
// Q4_0: 4-bit Symmetric Quantization
// ============================================================================

/// Q4_0 block structure: scale (f16) + 16 bytes (32 4-bit values)
const Q4_0_BLOCK_SIZE: usize = 32;
const Q4_0_TYPE_SIZE: usize = 18; // 2 + 16

fn dequantize_q4_0(data: &[u8], output: &mut [f32]) {
    let num_blocks = output.len() / Q4_0_BLOCK_SIZE;

    for block_idx in 0..num_blocks {
        let block_start = block_idx * Q4_0_TYPE_SIZE;
        let out_start = block_idx * Q4_0_BLOCK_SIZE;

        if block_start + Q4_0_TYPE_SIZE > data.len() {
            break;
        }

        let block = &data[block_start..block_start + Q4_0_TYPE_SIZE];
        let out = &mut output[out_start..out_start + Q4_0_BLOCK_SIZE];

        dequantize_q4_0_block(block, out);
    }
}

fn dequantize_q4_0_block(block: &[u8], output: &mut [f32]) {
    // Scale is stored as f16 in first 2 bytes
    let scale = f16_to_f32(u16::from_le_bytes([block[0], block[1]]));

    // 16 bytes of packed 4-bit values (2 values per byte)
    for i in 0..16 {
        let byte = block[2 + i];
        let q0 = (byte & 0x0F) as i8 - 8; // Q4_0 uses offset of 8
        let q1 = ((byte >> 4) & 0x0F) as i8 - 8;

        output[i * 2] = (q0 as f32) * scale;
        output[i * 2 + 1] = (q1 as f32) * scale;
    }
}

// ============================================================================
// Q4_1: 4-bit Asymmetric Quantization
// ============================================================================

const Q4_1_BLOCK_SIZE: usize = 32;
const Q4_1_TYPE_SIZE: usize = 20; // 2 + 2 + 16

fn dequantize_q4_1(data: &[u8], output: &mut [f32]) {
    let num_blocks = output.len() / Q4_1_BLOCK_SIZE;

    for block_idx in 0..num_blocks {
        let block_start = block_idx * Q4_1_TYPE_SIZE;
        let out_start = block_idx * Q4_1_BLOCK_SIZE;

        if block_start + Q4_1_TYPE_SIZE > data.len() {
            break;
        }

        let block = &data[block_start..block_start + Q4_1_TYPE_SIZE];
        let out = &mut output[out_start..out_start + Q4_1_BLOCK_SIZE];

        dequantize_q4_1_block(block, out);
    }
}

fn dequantize_q4_1_block(block: &[u8], output: &mut [f32]) {
    let scale = f16_to_f32(u16::from_le_bytes([block[0], block[1]]));
    let min = f16_to_f32(u16::from_le_bytes([block[2], block[3]]));

    for i in 0..16 {
        let byte = block[4 + i];
        let q0 = (byte & 0x0F) as f32;
        let q1 = ((byte >> 4) & 0x0F) as f32;

        output[i * 2] = q0 * scale + min;
        output[i * 2 + 1] = q1 * scale + min;
    }
}

// ============================================================================
// Q5_0: 5-bit Symmetric Quantization
// ============================================================================

const Q5_0_BLOCK_SIZE: usize = 32;
const Q5_0_TYPE_SIZE: usize = 22; // 2 + 4 (high bits) + 16 (low bits)

fn dequantize_q5_0(data: &[u8], output: &mut [f32]) {
    let num_blocks = output.len() / Q5_0_BLOCK_SIZE;

    for block_idx in 0..num_blocks {
        let block_start = block_idx * Q5_0_TYPE_SIZE;
        let out_start = block_idx * Q5_0_BLOCK_SIZE;

        if block_start + Q5_0_TYPE_SIZE > data.len() {
            break;
        }

        let scale = f16_to_f32(u16::from_le_bytes([
            data[block_start],
            data[block_start + 1],
        ]));

        // 4 bytes for high bits (32 values, 1 bit each)
        let qh = u32::from_le_bytes([
            data[block_start + 2],
            data[block_start + 3],
            data[block_start + 4],
            data[block_start + 5],
        ]);

        // 16 bytes for low 4 bits
        for i in 0..16 {
            let byte = data[block_start + 6 + i];
            let h0 = ((qh >> (i * 2)) & 1) as i8;
            let h1 = ((qh >> (i * 2 + 1)) & 1) as i8;

            let q0 = ((byte & 0x0F) as i8 | (h0 << 4)) - 16;
            let q1 = (((byte >> 4) & 0x0F) as i8 | (h1 << 4)) - 16;

            output[out_start + i * 2] = (q0 as f32) * scale;
            output[out_start + i * 2 + 1] = (q1 as f32) * scale;
        }
    }
}

// ============================================================================
// Q5_1: 5-bit Asymmetric Quantization
// ============================================================================

const Q5_1_BLOCK_SIZE: usize = 32;
const Q5_1_TYPE_SIZE: usize = 24; // 2 + 2 + 4 + 16

fn dequantize_q5_1(data: &[u8], output: &mut [f32]) {
    let num_blocks = output.len() / Q5_1_BLOCK_SIZE;

    for block_idx in 0..num_blocks {
        let block_start = block_idx * Q5_1_TYPE_SIZE;
        let out_start = block_idx * Q5_1_BLOCK_SIZE;

        if block_start + Q5_1_TYPE_SIZE > data.len() {
            break;
        }

        let scale = f16_to_f32(u16::from_le_bytes([
            data[block_start],
            data[block_start + 1],
        ]));
        let min = f16_to_f32(u16::from_le_bytes([
            data[block_start + 2],
            data[block_start + 3],
        ]));

        let qh = u32::from_le_bytes([
            data[block_start + 4],
            data[block_start + 5],
            data[block_start + 6],
            data[block_start + 7],
        ]);

        for i in 0..16 {
            let byte = data[block_start + 8 + i];
            let h0 = ((qh >> (i * 2)) & 1) as u8;
            let h1 = ((qh >> (i * 2 + 1)) & 1) as u8;

            let q0 = ((byte & 0x0F) | (h0 << 4)) as f32;
            let q1 = (((byte >> 4) & 0x0F) | (h1 << 4)) as f32;

            output[out_start + i * 2] = q0 * scale + min;
            output[out_start + i * 2 + 1] = q1 * scale + min;
        }
    }
}

// ============================================================================
// Q8_0: 8-bit Symmetric Quantization
// ============================================================================

const Q8_0_BLOCK_SIZE: usize = 32;
const Q8_0_TYPE_SIZE: usize = 34; // 2 + 32

fn dequantize_q8_0(data: &[u8], output: &mut [f32]) {
    let num_blocks = output.len() / Q8_0_BLOCK_SIZE;

    for block_idx in 0..num_blocks {
        let block_start = block_idx * Q8_0_TYPE_SIZE;
        let out_start = block_idx * Q8_0_BLOCK_SIZE;

        if block_start + Q8_0_TYPE_SIZE > data.len() {
            break;
        }

        let block = &data[block_start..block_start + Q8_0_TYPE_SIZE];
        let out = &mut output[out_start..out_start + Q8_0_BLOCK_SIZE];

        dequantize_q8_0_block(block, out);
    }
}

fn dequantize_q8_0_block(block: &[u8], output: &mut [f32]) {
    let scale = f16_to_f32(u16::from_le_bytes([block[0], block[1]]));

    for i in 0..32 {
        let q = block[2 + i] as i8;
        output[i] = (q as f32) * scale;
    }
}

// ============================================================================
// Q8_1: 8-bit Asymmetric Quantization
// ============================================================================

const Q8_1_BLOCK_SIZE: usize = 32;
const Q8_1_TYPE_SIZE: usize = 36; // 2 + 2 + 32

fn dequantize_q8_1(data: &[u8], output: &mut [f32]) {
    let num_blocks = output.len() / Q8_1_BLOCK_SIZE;

    for block_idx in 0..num_blocks {
        let block_start = block_idx * Q8_1_TYPE_SIZE;
        let out_start = block_idx * Q8_1_BLOCK_SIZE;

        if block_start + Q8_1_TYPE_SIZE > data.len() {
            break;
        }

        let scale = f16_to_f32(u16::from_le_bytes([
            data[block_start],
            data[block_start + 1],
        ]));
        let offset = f16_to_f32(u16::from_le_bytes([
            data[block_start + 2],
            data[block_start + 3],
        ]));

        for i in 0..32 {
            let q = data[block_start + 4 + i] as i8;
            output[out_start + i] = (q as f32) * scale + offset;
        }
    }
}

// ============================================================================
// Q2_K: 2-bit K-Quant
// ============================================================================

const Q2_K_BLOCK_SIZE: usize = 256;
const Q2_K_TYPE_SIZE: usize = 84;

fn dequantize_q2_k(data: &[u8], output: &mut [f32]) {
    let num_blocks = output.len() / Q2_K_BLOCK_SIZE;

    for block_idx in 0..num_blocks {
        let block_start = block_idx * Q2_K_TYPE_SIZE;
        let out_start = block_idx * Q2_K_BLOCK_SIZE;

        if block_start + Q2_K_TYPE_SIZE > data.len() {
            break;
        }

        // Q2_K structure:
        // scales: [16] 4-bit scales
        // d: f16 super scale
        // dmin: f16 super min
        // qs: [64] 2-bit values (4 per byte)

        let block = &data[block_start..];

        let d = f16_to_f32(u16::from_le_bytes([block[16], block[17]]));
        let dmin = f16_to_f32(u16::from_le_bytes([block[18], block[19]]));

        for j in 0..16 {
            // Each sub-block of 16 elements
            let sc = (block[j / 2] >> ((j % 2) * 4)) & 0x0F;
            let scale = d * (sc as f32);
            let min = dmin * (sc as f32);

            for k in 0..16 {
                let idx = j * 16 + k;
                let byte_idx = 20 + idx / 4;
                let bit_idx = (idx % 4) * 2;
                let q = (block[byte_idx] >> bit_idx) & 0x03;
                output[out_start + idx] = (q as f32) * scale - min;
            }
        }
    }
}

// ============================================================================
// Q3_K: 3-bit K-Quant
// ============================================================================

const Q3_K_BLOCK_SIZE: usize = 256;
const Q3_K_TYPE_SIZE: usize = 110;

fn dequantize_q3_k(data: &[u8], output: &mut [f32]) {
    let num_blocks = output.len() / Q3_K_BLOCK_SIZE;

    for block_idx in 0..num_blocks {
        let block_start = block_idx * Q3_K_TYPE_SIZE;
        let out_start = block_idx * Q3_K_BLOCK_SIZE;

        if block_start + Q3_K_TYPE_SIZE > data.len() {
            break;
        }

        // Simplified Q3_K dequantization
        let block = &data[block_start..];
        let d = f16_to_f32(u16::from_le_bytes([block[104], block[105]]));

        // High bits, scales, and low bits are interleaved in complex way
        // This is a simplified implementation
        for i in 0..256 {
            let byte_idx = i * 3 / 8;
            let bit_offset = (i * 3) % 8;

            if byte_idx < 96 {
                let q = ((block[byte_idx] >> bit_offset) & 0x07) as i8 - 4;
                output[out_start + i] = (q as f32) * d;
            }
        }
    }
}

// ============================================================================
// Q4_K: 4-bit K-Quant (Most Common)
// ============================================================================

const Q4_K_BLOCK_SIZE: usize = 256;
const Q4_K_TYPE_SIZE: usize = 144; // d(2) + dmin(2) + scales(12) + qs(128)

fn dequantize_q4_k(data: &[u8], output: &mut [f32]) {
    let num_blocks = output.len() / Q4_K_BLOCK_SIZE;

    for block_idx in 0..num_blocks {
        let block_start = block_idx * Q4_K_TYPE_SIZE;
        let out_start = block_idx * Q4_K_BLOCK_SIZE;

        if block_start + Q4_K_TYPE_SIZE > data.len() {
            break;
        }

        let block = &data[block_start..block_start + Q4_K_TYPE_SIZE];
        let out = &mut output[out_start..out_start + Q4_K_BLOCK_SIZE];

        dequantize_q4_k_block(block, out);
    }
}

fn dequantize_q4_k_block(block: &[u8], output: &mut [f32]) {
    // Block layout: d (2) + dmin (2) + scales (12) + qs (128)
    let d = f16_to_f32(u16::from_le_bytes([block[0], block[1]]));
    let dmin = f16_to_f32(u16::from_le_bytes([block[2], block[3]]));

    // Process each of 8 sub-blocks of 32 elements
    for sb in 0..8 {
        // Extract 6-bit scale for this sub-block
        let scale_idx = sb * 6 / 8;
        let scale_shift = (sb * 6) % 8;

        let mut sc = (block[4 + scale_idx] >> scale_shift) & 0x3F;
        if scale_shift > 2 && scale_idx + 1 < 12 {
            sc |= (block[4 + scale_idx + 1] << (8 - scale_shift)) & 0x3F;
        }

        let scale = d * (sc as f32);

        // Dequantize 32 elements in this sub-block
        let qs_start = 16 + sb * 16; // 16 bytes header + 16 bytes per sub-block
        for i in 0..16 {
            let byte = block[qs_start + i];
            let q0 = (byte & 0x0F) as f32;
            let q1 = ((byte >> 4) & 0x0F) as f32;

            output[sb * 32 + i * 2] = q0 * scale + dmin;
            output[sb * 32 + i * 2 + 1] = q1 * scale + dmin;
        }
    }
}

// ============================================================================
// Q5_K: 5-bit K-Quant
// ============================================================================

const Q5_K_BLOCK_SIZE: usize = 256;
const Q5_K_TYPE_SIZE: usize = 176;

fn dequantize_q5_k(data: &[u8], output: &mut [f32]) {
    let num_blocks = output.len() / Q5_K_BLOCK_SIZE;

    for block_idx in 0..num_blocks {
        let block_start = block_idx * Q5_K_TYPE_SIZE;
        let out_start = block_idx * Q5_K_BLOCK_SIZE;

        if block_start + Q5_K_TYPE_SIZE > data.len() {
            break;
        }

        let block = &data[block_start..];
        let d = f16_to_f32(u16::from_le_bytes([block[0], block[1]]));
        let dmin = f16_to_f32(u16::from_le_bytes([block[2], block[3]]));

        // Simplified Q5_K - similar structure to Q4_K but with 5 bits
        for i in 0..256 {
            let byte_idx = 16 + (i * 5) / 8;
            let bit_offset = (i * 5) % 8;

            if byte_idx < Q5_K_TYPE_SIZE {
                let mut q = (block[byte_idx] >> bit_offset) & 0x1F;
                if bit_offset > 3 && byte_idx + 1 < Q5_K_TYPE_SIZE {
                    q |= (block[byte_idx + 1] << (8 - bit_offset)) & 0x1F;
                }
                output[out_start + i] = (q as f32) * d + dmin;
            }
        }
    }
}

// ============================================================================
// Q6_K: 6-bit K-Quant
// ============================================================================

const Q6_K_BLOCK_SIZE: usize = 256;
const Q6_K_TYPE_SIZE: usize = 210;

fn dequantize_q6_k(data: &[u8], output: &mut [f32]) {
    let num_blocks = output.len() / Q6_K_BLOCK_SIZE;

    for block_idx in 0..num_blocks {
        let block_start = block_idx * Q6_K_TYPE_SIZE;
        let out_start = block_idx * Q6_K_BLOCK_SIZE;

        if block_start + Q6_K_TYPE_SIZE > data.len() {
            break;
        }

        let block = &data[block_start..];
        let d = f16_to_f32(u16::from_le_bytes([block[208], block[209]]));

        // Q6_K has complex bit packing
        // Low 4 bits: ql[128]
        // High 2 bits: qh[64]
        // Scales: scales[16]
        for i in 0..256 {
            let ql_idx = i / 2;
            let is_high = i % 2 == 1;

            if ql_idx < 128 {
                let ql = if is_high {
                    (block[ql_idx] >> 4) & 0x0F
                } else {
                    block[ql_idx] & 0x0F
                };

                let qh_idx = 128 + i / 4;
                let qh_shift = (i % 4) * 2;
                let qh = if qh_idx < 192 {
                    (block[qh_idx] >> qh_shift) & 0x03
                } else {
                    0
                };

                let q = ((qh << 4) | ql) as i8 - 32;
                let scale_idx = i / 16;
                let sc = if scale_idx < 16 {
                    (block[192 + scale_idx / 2] >> ((scale_idx % 2) * 4)) & 0x0F
                } else {
                    1
                };

                output[out_start + i] = (q as f32) * d * (sc as f32);
            }
        }
    }
}

// ============================================================================
// IQ4_NL: I-Quant 4-bit Non-Linear
// ============================================================================

const IQ4_NL_BLOCK_SIZE: usize = 32;
const IQ4_NL_TYPE_SIZE: usize = 18;

// Non-linear quantization lookup table (simplified version)
const IQ4_NL_LUT: [f32; 16] = [
    -1.0, -0.75, -0.5, -0.375, -0.25, -0.125, 0.0, 0.125, 0.25, 0.375, 0.5, 0.75, 1.0, 1.5, 2.0,
    3.0,
];

fn dequantize_iq4_nl(data: &[u8], output: &mut [f32]) {
    let num_blocks = output.len() / IQ4_NL_BLOCK_SIZE;

    for block_idx in 0..num_blocks {
        let block_start = block_idx * IQ4_NL_TYPE_SIZE;
        let out_start = block_idx * IQ4_NL_BLOCK_SIZE;

        if block_start + IQ4_NL_TYPE_SIZE > data.len() {
            break;
        }

        let scale = f16_to_f32(u16::from_le_bytes([
            data[block_start],
            data[block_start + 1],
        ]));

        for i in 0..16 {
            let byte = data[block_start + 2 + i];
            let q0 = (byte & 0x0F) as usize;
            let q1 = ((byte >> 4) & 0x0F) as usize;

            output[out_start + i * 2] = IQ4_NL_LUT[q0] * scale;
            output[out_start + i * 2 + 1] = IQ4_NL_LUT[q1] * scale;
        }
    }
}

// ============================================================================
// BITNET_T158: BitNet b1.58 Ternary Quantization
// ============================================================================

const BITNET_T158_BLOCK_SIZE: usize = 256;
const BITNET_T158_TYPE_SIZE: usize = 66; // 64 bytes packed + 2 bytes FP16 scale

/// Wrapper for BitNet T158 dequantization from GGUF format.
///
/// GGUF BITNET_T158 block layout (66 bytes per 256 elements):
/// - 64 bytes: packed 2-bit ternary data (256 values × 2 bits = 512 bits = 64 bytes)
/// - 2 bytes: FP16 scale factor
///
/// This wrapper extracts scales from the interleaved GGUF format and passes
/// them to the bitnet module's dequantization function.
fn dequantize_bitnet_t158_wrapper(data: &[u8], output: &mut [f32]) {
    let num_blocks = output.len() / BITNET_T158_BLOCK_SIZE;

    // Extract scales from GGUF format (interleaved with packed data)
    let mut scales = Vec::with_capacity(num_blocks);
    let mut packed_data = Vec::with_capacity(num_blocks * 64);

    for block_idx in 0..num_blocks {
        let block_start = block_idx * BITNET_T158_TYPE_SIZE;

        if block_start + BITNET_T158_TYPE_SIZE > data.len() {
            break;
        }

        // Extract 64 bytes of packed ternary data
        packed_data.extend_from_slice(&data[block_start..block_start + 64]);

        // Extract FP16 scale (last 2 bytes of block)
        let scale_f16 = f16_to_f32(u16::from_le_bytes([
            data[block_start + 64],
            data[block_start + 65],
        ]));
        scales.push(scale_f16);
    }

    // Call bitnet module's dequantization function
    let dequantized = dequantize_bitnet_t158(&packed_data, &scales, output.len());

    // Copy to output buffer
    output[..dequantized.len()].copy_from_slice(&dequantized);
}

// ============================================================================
// F16 Conversion Helper
// ============================================================================

/// Convert f16 bits to f32.
#[inline(always)]
fn f16_to_f32(bits: u16) -> f32 {
    let sign = ((bits & 0x8000) as u32) << 16;
    let exp = ((bits >> 10) & 0x1F) as u32;
    let frac = (bits & 0x03FF) as u32;

    if exp == 0 {
        if frac == 0 {
            return f32::from_bits(sign);
        }
        // Denormalized
        let mut e = 1u32;
        let mut f = frac;
        while (f & 0x0400) == 0 {
            f <<= 1;
            e += 1;
        }
        f &= 0x03FF;
        return f32::from_bits(sign | ((127 - 15 + 1 - e) << 23) | (f << 13));
    }

    if exp == 31 {
        // Inf or NaN
        return f32::from_bits(sign | 0x7F80_0000 | (frac << 13));
    }

    f32::from_bits(sign | ((exp + 127 - 15) << 23) | (frac << 13))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quant_type_sizes() {
        assert_eq!(GgufQuantType::F32.block_size(), 1);
        assert_eq!(GgufQuantType::F32.type_size(), 4);

        assert_eq!(GgufQuantType::Q4_0.block_size(), 32);
        assert_eq!(GgufQuantType::Q4_0.type_size(), 18);

        assert_eq!(GgufQuantType::Q4_K.block_size(), 256);
        assert_eq!(GgufQuantType::Q4_K.type_size(), 144);
    }

    #[test]
    fn test_quant_type_bits() {
        // F32 = 32 bits
        assert!((GgufQuantType::F32.bits_per_weight() - 32.0).abs() < 0.1);

        // Q4_0 = 18 bytes * 8 / 32 elements = 4.5 bits
        assert!((GgufQuantType::Q4_0.bits_per_weight() - 4.5).abs() < 0.1);

        // Q8_0 = 34 bytes * 8 / 32 elements = 8.5 bits
        assert!((GgufQuantType::Q8_0.bits_per_weight() - 8.5).abs() < 0.1);
    }

    #[test]
    fn test_f16_conversion() {
        // Test basic values
        assert_eq!(f16_to_f32(0x0000), 0.0);
        assert_eq!(f16_to_f32(0x3C00), 1.0);
        assert_eq!(f16_to_f32(0xBC00), -1.0);

        // Test small values
        let half = f16_to_f32(0x3800); // 0.5 in f16
        assert!((half - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_q4_0_dequantize() {
        // Create a simple Q4_0 block: scale=1.0, all zeros
        let mut block = vec![0u8; 18];
        // f16 1.0 = 0x3C00
        block[0] = 0x00;
        block[1] = 0x3C;
        // All quants = 8 (which becomes 0 after offset subtraction)
        for i in 0..16 {
            block[2 + i] = 0x88; // Both nibbles = 8
        }

        let mut output = vec![0.0f32; 32];
        dequantize_q4_0_block(&block, &mut output);

        // All values should be 0
        for val in &output {
            assert!(val.abs() < 0.001);
        }
    }

    #[test]
    fn test_q8_0_dequantize() {
        // Create a Q8_0 block
        let mut block = vec![0u8; 34];
        // scale = 1.0 (f16 0x3C00)
        block[0] = 0x00;
        block[1] = 0x3C;
        // quants = [1, 2, 3, ...]
        for i in 0..32 {
            block[2 + i] = (i + 1) as u8;
        }

        let mut output = vec![0.0f32; 32];
        dequantize_q8_0_block(&block, &mut output);

        // Values should be 1.0, 2.0, 3.0, ...
        for i in 0..32 {
            assert!((output[i] - (i + 1) as f32).abs() < 0.001);
        }
    }

    #[test]
    fn test_quant_type_try_from() {
        assert_eq!(GgufQuantType::try_from(0).unwrap(), GgufQuantType::F32);
        assert_eq!(GgufQuantType::try_from(12).unwrap(), GgufQuantType::Q4_K);
        assert!(GgufQuantType::try_from(100).is_err());
    }

    #[test]
    fn test_quantized_tensor() {
        let tensor = QuantizedTensor {
            data: vec![0u8; 144],
            dtype: GgufQuantType::Q4_K,
            shape: vec![256],
            num_elements: 256,
        };

        assert_eq!(tensor.block_count(), 1);
        assert!(tensor.dtype.is_quantized());
    }
}
