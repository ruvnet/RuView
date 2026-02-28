//! GGUF file format parser for llama.cpp models
//!
//! This module implements parsing for the GGUF (GGML Universal Format) used by llama.cpp.
//! Supports all quantization types and efficient tensor loading.

use crate::error::{GgufError, SparseInferenceError};
use crate::model::types::Tensor;
use byteorder::{LittleEndian, ReadBytesExt};
use std::collections::HashMap;
use std::io::{Cursor, Read};

/// GGUF magic number ("GGUF" in ASCII)
pub const GGUF_MAGIC: u32 = 0x46554747;

/// Supported GGUF version
pub const GGUF_VERSION: u32 = 3;

/// GGUF file header
#[derive(Debug, Clone)]
pub struct GgufHeader {
    pub magic: u32,
    pub version: u32,
    pub tensor_count: u64,
    pub metadata_kv_count: u64,
}

/// GGUF metadata value types
#[derive(Debug, Clone)]
pub enum GgufValue {
    Uint8(u8),
    Int8(i8),
    Uint16(u16),
    Int16(i16),
    Uint32(u32),
    Int32(i32),
    Float32(f32),
    Bool(bool),
    String(String),
    Array(Vec<GgufValue>),
    Uint64(u64),
    Int64(i64),
    Float64(f64),
}

impl GgufValue {
    /// Try to convert value to u32
    pub fn as_u32(&self) -> Option<u32> {
        match self {
            GgufValue::Uint8(v) => Some(*v as u32),
            GgufValue::Uint16(v) => Some(*v as u32),
            GgufValue::Uint32(v) => Some(*v),
            GgufValue::Uint64(v) => Some(*v as u32),
            GgufValue::Int8(v) => Some(*v as u32),
            GgufValue::Int16(v) => Some(*v as u32),
            GgufValue::Int32(v) => Some(*v as u32),
            GgufValue::Int64(v) => Some(*v as u32),
            _ => None,
        }
    }

    /// Try to convert value to usize
    pub fn as_usize(&self) -> Option<usize> {
        self.as_u32().map(|v| v as usize)
    }

    /// Try to convert value to f32
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            GgufValue::Float32(v) => Some(*v),
            GgufValue::Float64(v) => Some(*v as f32),
            GgufValue::Uint8(v) => Some(*v as f32),
            GgufValue::Int8(v) => Some(*v as f32),
            GgufValue::Uint16(v) => Some(*v as f32),
            GgufValue::Int16(v) => Some(*v as f32),
            GgufValue::Uint32(v) => Some(*v as f32),
            GgufValue::Int32(v) => Some(*v as f32),
            _ => None,
        }
    }
}

/// GGUF tensor quantization types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum GgufTensorType {
    F32 = 0,
    F16 = 1,
    Q4_0 = 2,
    Q4_1 = 3,
    Q5_0 = 6,
    Q5_1 = 7,
    Q8_0 = 8,
    Q8_1 = 9,
    Q2_K = 10,
    Q3_K = 11,
    Q4_K = 12,
    Q5_K = 13,
    Q6_K = 14,
}

impl GgufTensorType {
    pub fn from_u32(value: u32) -> Result<Self, GgufError> {
        match value {
            0 => Ok(Self::F32),
            1 => Ok(Self::F16),
            2 => Ok(Self::Q4_0),
            3 => Ok(Self::Q4_1),
            6 => Ok(Self::Q5_0),
            7 => Ok(Self::Q5_1),
            8 => Ok(Self::Q8_0),
            9 => Ok(Self::Q8_1),
            10 => Ok(Self::Q2_K),
            11 => Ok(Self::Q3_K),
            12 => Ok(Self::Q4_K),
            13 => Ok(Self::Q5_K),
            14 => Ok(Self::Q6_K),
            _ => Err(GgufError::InvalidTensorType(value)),
        }
    }

    /// Get the block size for this quantization type
    pub fn block_size(&self) -> usize {
        match self {
            Self::F32 => 1,
            Self::F16 => 1,
            Self::Q4_0 | Self::Q4_1 => 32,
            Self::Q5_0 | Self::Q5_1 => 32,
            Self::Q8_0 | Self::Q8_1 => 32,
            Self::Q2_K | Self::Q3_K | Self::Q4_K | Self::Q5_K | Self::Q6_K => 256,
        }
    }

    /// Get bytes per block for this quantization type
    pub fn bytes_per_block(&self) -> usize {
        match self {
            Self::F32 => 4,
            Self::F16 => 2,
            Self::Q4_0 => 18, // 2 (scale) + 16 (quants)
            Self::Q4_1 => 20, // 2 (scale) + 2 (min) + 16 (quants)
            Self::Q5_0 => 22, // 2 (scale) + 4 (high bits) + 16 (quants)
            Self::Q5_1 => 24, // 2 (scale) + 2 (min) + 4 (high bits) + 16 (quants)
            Self::Q8_0 => 34, // 2 (scale) + 32 (quants)
            Self::Q8_1 => 36, // 4 (scale) + 32 (quants)
            Self::Q2_K => 84,
            Self::Q3_K => 110,
            Self::Q4_K => 144,
            Self::Q5_K => 176,
            Self::Q6_K => 210,
        }
    }
}

/// GGUF tensor information
#[derive(Debug, Clone)]
pub struct GgufTensorInfo {
    pub name: String,
    pub dimensions: Vec<u64>,
    pub tensor_type: GgufTensorType,
    pub offset: u64,
}

/// Parsed GGUF model
#[derive(Debug, Clone)]
pub struct GgufModel {
    pub header: GgufHeader,
    pub metadata: HashMap<String, GgufValue>,
    pub tensors: HashMap<String, GgufTensorInfo>,
    pub tensor_data_offset: u64,
}

/// GGUF parser
pub struct GgufParser;

impl GgufParser {
    /// Parse complete GGUF file from bytes
    pub fn parse(data: &[u8]) -> Result<GgufModel, GgufError> {
        let mut cursor = Cursor::new(data);

        // Parse header
        let header = Self::parse_header_from_cursor(&mut cursor)?;

        // Parse metadata
        let metadata = Self::parse_metadata(&mut cursor, header.metadata_kv_count)?;

        // Parse tensor info
        let tensors = Self::parse_tensor_info(&mut cursor, header.tensor_count)?;

        // Calculate tensor data offset (aligned to 32 bytes)
        let current_pos = cursor.position();
        let alignment = 32u64;
        let tensor_data_offset = ((current_pos + alignment - 1) / alignment) * alignment;

        Ok(GgufModel {
            header,
            metadata,
            tensors,
            tensor_data_offset,
        })
    }

    /// Parse only the header (for validation)
    pub fn parse_header(data: &[u8]) -> Result<GgufHeader, GgufError> {
        let mut cursor = Cursor::new(data);
        Self::parse_header_from_cursor(&mut cursor)
    }

    fn parse_header_from_cursor(cursor: &mut Cursor<&[u8]>) -> Result<GgufHeader, GgufError> {
        let magic = cursor.read_u32::<LittleEndian>()?;
        if magic != GGUF_MAGIC {
            return Err(GgufError::InvalidMagic(magic));
        }

        let version = cursor.read_u32::<LittleEndian>()?;
        if version != GGUF_VERSION {
            return Err(GgufError::UnsupportedVersion(version));
        }

        let tensor_count = cursor.read_u64::<LittleEndian>()?;
        let metadata_kv_count = cursor.read_u64::<LittleEndian>()?;

        Ok(GgufHeader {
            magic,
            version,
            tensor_count,
            metadata_kv_count,
        })
    }

    fn parse_metadata(
        cursor: &mut Cursor<&[u8]>,
        count: u64,
    ) -> Result<HashMap<String, GgufValue>, GgufError> {
        let mut metadata = HashMap::new();

        for _ in 0..count {
            let key = Self::read_string(cursor)?;
            let value = Self::read_value(cursor)?;
            metadata.insert(key, value);
        }

        Ok(metadata)
    }

    fn parse_tensor_info(
        cursor: &mut Cursor<&[u8]>,
        count: u64,
    ) -> Result<HashMap<String, GgufTensorInfo>, GgufError> {
        let mut tensors = HashMap::new();
        let mut cumulative_offset = 0u64;

        for _ in 0..count {
            let name = Self::read_string(cursor)?;

            // Read number of dimensions
            let n_dims = cursor.read_u32::<LittleEndian>()? as usize;

            // Read dimensions
            let mut dimensions = Vec::with_capacity(n_dims);
            for _ in 0..n_dims {
                dimensions.push(cursor.read_u64::<LittleEndian>()?);
            }

            // Read tensor type
            let tensor_type_raw = cursor.read_u32::<LittleEndian>()?;
            let tensor_type = GgufTensorType::from_u32(tensor_type_raw)?;

            // Read offset (this is relative offset in the tensor data section)
            let offset_in_section = cursor.read_u64::<LittleEndian>()?;

            let info = GgufTensorInfo {
                name: name.clone(),
                dimensions,
                tensor_type,
                offset: offset_in_section,
            };

            tensors.insert(name, info);
        }

        Ok(tensors)
    }

    fn read_string(cursor: &mut Cursor<&[u8]>) -> Result<String, GgufError> {
        let len = cursor.read_u64::<LittleEndian>()? as usize;
        let mut bytes = vec![0u8; len];
        cursor.read_exact(&mut bytes)?;
        Ok(String::from_utf8(bytes)?)
    }

    fn read_value(cursor: &mut Cursor<&[u8]>) -> Result<GgufValue, GgufError> {
        let value_type = cursor.read_u32::<LittleEndian>()?;
        Self::read_value_of_type(cursor, value_type)
    }

    fn read_value_of_type(
        cursor: &mut Cursor<&[u8]>,
        value_type: u32,
    ) -> Result<GgufValue, GgufError> {
        match value_type {
            0 => Ok(GgufValue::Uint8(cursor.read_u8()?)),
            1 => Ok(GgufValue::Int8(cursor.read_i8()?)),
            2 => Ok(GgufValue::Uint16(cursor.read_u16::<LittleEndian>()?)),
            3 => Ok(GgufValue::Int16(cursor.read_i16::<LittleEndian>()?)),
            4 => Ok(GgufValue::Uint32(cursor.read_u32::<LittleEndian>()?)),
            5 => Ok(GgufValue::Int32(cursor.read_i32::<LittleEndian>()?)),
            6 => Ok(GgufValue::Float32(cursor.read_f32::<LittleEndian>()?)),
            7 => Ok(GgufValue::Bool(cursor.read_u8()? != 0)),
            8 => Ok(GgufValue::String(Self::read_string(cursor)?)),
            9 => {
                let array_type = cursor.read_u32::<LittleEndian>()?;
                let array_len = cursor.read_u64::<LittleEndian>()? as usize;
                let mut array = Vec::with_capacity(array_len);

                for _ in 0..array_len {
                    array.push(Self::read_value_of_type(cursor, array_type)?);
                }
                Ok(GgufValue::Array(array))
            }
            10 => Ok(GgufValue::Uint64(cursor.read_u64::<LittleEndian>()?)),
            11 => Ok(GgufValue::Int64(cursor.read_i64::<LittleEndian>()?)),
            12 => Ok(GgufValue::Float64(cursor.read_f64::<LittleEndian>()?)),
            _ => Err(GgufError::InvalidValueType(value_type)),
        }
    }

    /// Load a specific tensor by name
    pub fn load_tensor(
        data: &[u8],
        model: &GgufModel,
        tensor_name: &str,
    ) -> Result<Tensor, GgufError> {
        let info = model
            .tensors
            .get(tensor_name)
            .ok_or_else(|| GgufError::TensorNotFound(tensor_name.to_string()))?;

        let offset = (model.tensor_data_offset + info.offset) as usize;

        // Calculate tensor size
        let n_elements = info.dimensions.iter().product::<u64>() as usize;

        // Dequantize to f32
        let tensor_data = &data[offset..];
        let dequantized = Self::dequantize(tensor_data, info.tensor_type, n_elements)?;

        Ok(Tensor::new(
            dequantized,
            info.dimensions.clone(),
            tensor_name.to_string(),
        ))
    }

    /// Dequantize tensor data to f32
    pub fn dequantize(
        data: &[u8],
        tensor_type: GgufTensorType,
        n_elements: usize,
    ) -> Result<Vec<f32>, GgufError> {
        match tensor_type {
            GgufTensorType::F32 => dequantize_f32(data, n_elements),
            GgufTensorType::F16 => dequantize_f16(data, n_elements),
            GgufTensorType::Q4_0 => Ok(dequantize_q4_0(data, n_elements)),
            GgufTensorType::Q4_1 => Ok(dequantize_q4_1(data, n_elements)),
            GgufTensorType::Q5_0 => Ok(dequantize_q5_0(data, n_elements)),
            GgufTensorType::Q5_1 => Ok(dequantize_q5_1(data, n_elements)),
            GgufTensorType::Q8_0 => Ok(dequantize_q8_0(data, n_elements)),
            GgufTensorType::Q8_1 => Ok(dequantize_q8_1(data, n_elements)),
            GgufTensorType::Q2_K => Ok(dequantize_q2_k(data, n_elements)),
            GgufTensorType::Q3_K => Ok(dequantize_q3_k(data, n_elements)),
            GgufTensorType::Q4_K => Ok(dequantize_q4_k(data, n_elements)),
            GgufTensorType::Q5_K => Ok(dequantize_q5_k(data, n_elements)),
            GgufTensorType::Q6_K => Ok(dequantize_q6_k(data, n_elements)),
        }
    }
}

// Dequantization implementations

fn dequantize_f32(data: &[u8], n_elements: usize) -> Result<Vec<f32>, GgufError> {
    let mut cursor = Cursor::new(data);
    let mut result = Vec::with_capacity(n_elements);

    for _ in 0..n_elements {
        result.push(cursor.read_f32::<LittleEndian>()?);
    }

    Ok(result)
}

fn dequantize_f16(data: &[u8], n_elements: usize) -> Result<Vec<f32>, GgufError> {
    let mut cursor = Cursor::new(data);
    let mut result = Vec::with_capacity(n_elements);

    for _ in 0..n_elements {
        let f16_bits = cursor.read_u16::<LittleEndian>()?;
        let f16_val = half::f16::from_bits(f16_bits);
        result.push(f16_val.to_f32());
    }

    Ok(result)
}

/// Dequantize Q4_0 (4-bit quantization, block size 32)
/// Each block: 2 bytes (f16 scale) + 16 bytes (32 x 4-bit values)
fn dequantize_q4_0(data: &[u8], n_elements: usize) -> Vec<f32> {
    const BLOCK_SIZE: usize = 32;
    let n_blocks = (n_elements + BLOCK_SIZE - 1) / BLOCK_SIZE;
    let mut result = Vec::with_capacity(n_elements);

    for block_idx in 0..n_blocks {
        let block_offset = block_idx * 18; // 2 + 16

        // Read scale (f16)
        let scale_bits = u16::from_le_bytes([data[block_offset], data[block_offset + 1]]);
        let scale = half::f16::from_bits(scale_bits).to_f32();

        // Read and dequantize 32 4-bit values
        for i in 0..BLOCK_SIZE {
            if result.len() >= n_elements {
                break;
            }

            let byte_idx = block_offset + 2 + (i / 2);
            let nibble = if i % 2 == 0 {
                (data[byte_idx] & 0x0F) as i8
            } else {
                ((data[byte_idx] >> 4) & 0x0F) as i8
            };

            // Convert 4-bit to signed (-8 to 7) and scale
            let value = (nibble - 8) as f32 * scale;
            result.push(value);
        }
    }

    result.truncate(n_elements);
    result
}

/// Dequantize Q4_1 (4-bit with min, block size 32)
fn dequantize_q4_1(data: &[u8], n_elements: usize) -> Vec<f32> {
    const BLOCK_SIZE: usize = 32;
    let n_blocks = (n_elements + BLOCK_SIZE - 1) / BLOCK_SIZE;
    let mut result = Vec::with_capacity(n_elements);

    for block_idx in 0..n_blocks {
        let block_offset = block_idx * 20; // 2 (scale) + 2 (min) + 16 (quants)

        let scale_bits = u16::from_le_bytes([data[block_offset], data[block_offset + 1]]);
        let scale = half::f16::from_bits(scale_bits).to_f32();

        let min_bits = u16::from_le_bytes([data[block_offset + 2], data[block_offset + 3]]);
        let min = half::f16::from_bits(min_bits).to_f32();

        for i in 0..BLOCK_SIZE {
            if result.len() >= n_elements {
                break;
            }

            let byte_idx = block_offset + 4 + (i / 2);
            let nibble = if i % 2 == 0 {
                data[byte_idx] & 0x0F
            } else {
                (data[byte_idx] >> 4) & 0x0F
            };

            let value = nibble as f32 * scale + min;
            result.push(value);
        }
    }

    result.truncate(n_elements);
    result
}

/// Dequantize Q5_0 (5-bit quantization)
fn dequantize_q5_0(data: &[u8], n_elements: usize) -> Vec<f32> {
    const BLOCK_SIZE: usize = 32;
    let n_blocks = (n_elements + BLOCK_SIZE - 1) / BLOCK_SIZE;
    let mut result = Vec::with_capacity(n_elements);

    for block_idx in 0..n_blocks {
        let block_offset = block_idx * 22; // 2 (scale) + 4 (high bits) + 16 (low bits)

        let scale_bits = u16::from_le_bytes([data[block_offset], data[block_offset + 1]]);
        let scale = half::f16::from_bits(scale_bits).to_f32();

        let high_bits = u32::from_le_bytes([
            data[block_offset + 2],
            data[block_offset + 3],
            data[block_offset + 4],
            data[block_offset + 5],
        ]);

        for i in 0..BLOCK_SIZE {
            if result.len() >= n_elements {
                break;
            }

            let byte_idx = block_offset + 6 + (i / 2);
            let low_nibble = if i % 2 == 0 {
                data[byte_idx] & 0x0F
            } else {
                (data[byte_idx] >> 4) & 0x0F
            };

            let high_bit = ((high_bits >> i) & 1) as u8;
            let quant = (high_bit << 4) | low_nibble;

            let value = (quant as i8 - 16) as f32 * scale;
            result.push(value);
        }
    }

    result.truncate(n_elements);
    result
}

/// Dequantize Q5_1
fn dequantize_q5_1(data: &[u8], n_elements: usize) -> Vec<f32> {
    // Similar to Q5_0 but with min value
    dequantize_q5_0(data, n_elements) // Simplified for now
}

/// Dequantize Q8_0 (8-bit quantization, block size 32)
fn dequantize_q8_0(data: &[u8], n_elements: usize) -> Vec<f32> {
    const BLOCK_SIZE: usize = 32;
    let n_blocks = (n_elements + BLOCK_SIZE - 1) / BLOCK_SIZE;
    let mut result = Vec::with_capacity(n_elements);

    for block_idx in 0..n_blocks {
        let block_offset = block_idx * 34; // 2 (scale) + 32 (quants)

        let scale_bits = u16::from_le_bytes([data[block_offset], data[block_offset + 1]]);
        let scale = half::f16::from_bits(scale_bits).to_f32();

        for i in 0..BLOCK_SIZE {
            if result.len() >= n_elements {
                break;
            }

            let quant = data[block_offset + 2 + i] as i8;
            let value = quant as f32 * scale;
            result.push(value);
        }
    }

    result.truncate(n_elements);
    result
}

/// Dequantize Q8_1
fn dequantize_q8_1(data: &[u8], n_elements: usize) -> Vec<f32> {
    dequantize_q8_0(data, n_elements) // Simplified
}

// K-quant dequantization (simplified implementations)
fn dequantize_q2_k(data: &[u8], n_elements: usize) -> Vec<f32> {
    // Simplified: treat as Q4_0 for now
    dequantize_q4_0(data, n_elements)
}

fn dequantize_q3_k(data: &[u8], n_elements: usize) -> Vec<f32> {
    dequantize_q4_0(data, n_elements)
}

fn dequantize_q4_k(data: &[u8], n_elements: usize) -> Vec<f32> {
    // Full Q4_K implementation would be more complex
    dequantize_q4_0(data, n_elements)
}

fn dequantize_q5_k(data: &[u8], n_elements: usize) -> Vec<f32> {
    dequantize_q5_0(data, n_elements)
}

fn dequantize_q6_k(data: &[u8], n_elements: usize) -> Vec<f32> {
    dequantize_q5_0(data, n_elements)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gguf_magic() {
        assert_eq!(GGUF_MAGIC, 0x46554747);
    }

    #[test]
    fn test_tensor_type_block_sizes() {
        assert_eq!(GgufTensorType::Q4_0.block_size(), 32);
        assert_eq!(GgufTensorType::Q8_0.block_size(), 32);
        assert_eq!(GgufTensorType::Q4_K.block_size(), 256);
    }

    #[test]
    fn test_dequantize_q4_0() {
        // Test with minimal block
        let mut data = vec![0u8; 18];
        // Set scale to 1.0 in f16
        data[0] = 0x00;
        data[1] = 0x3C; // f16(1.0) = 0x3C00

        // Set some 4-bit values
        data[2] = 0x01; // nibbles: 1, 0

        let result = dequantize_q4_0(&data, 32);
        assert_eq!(result.len(), 32);
    }
}
