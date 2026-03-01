#![allow(
    clippy::all,
    unused_imports,
    unused_variables,
    dead_code,
    unused_mut,
    unused_assignments,
    non_camel_case_types,
    clippy::approx_constant,
    unexpected_cfgs,
    unused_must_use,
    unused_parens
)]
//! GGUF Format Integration Tests for v2.1
//!
//! Tests GGUF file format parsing, metadata extraction, tensor loading,
//! and quantization/dequantization operations.

use std::collections::HashMap;
use std::io::{Cursor, Read, Write};

// ============================================================================
// GGUF Constants
// ============================================================================

/// GGUF magic number "GGUF" in little-endian
pub const GGUF_MAGIC: u32 = 0x46554747; // "GGUF"

/// Supported GGUF version
pub const GGUF_VERSION: u32 = 3;

/// Default alignment for tensor data
pub const GGUF_DEFAULT_ALIGNMENT: usize = 32;

// ============================================================================
// GGUF Data Types
// ============================================================================

/// GGUF metadata value types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum GgufMetadataType {
    Uint8 = 0,
    Int8 = 1,
    Uint16 = 2,
    Int16 = 3,
    Uint32 = 4,
    Int32 = 5,
    Float32 = 6,
    Bool = 7,
    String = 8,
    Array = 9,
    Uint64 = 10,
    Int64 = 11,
    Float64 = 12,
}

impl TryFrom<u32> for GgufMetadataType {
    type Error = GgufError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(GgufMetadataType::Uint8),
            1 => Ok(GgufMetadataType::Int8),
            2 => Ok(GgufMetadataType::Uint16),
            3 => Ok(GgufMetadataType::Int16),
            4 => Ok(GgufMetadataType::Uint32),
            5 => Ok(GgufMetadataType::Int32),
            6 => Ok(GgufMetadataType::Float32),
            7 => Ok(GgufMetadataType::Bool),
            8 => Ok(GgufMetadataType::String),
            9 => Ok(GgufMetadataType::Array),
            10 => Ok(GgufMetadataType::Uint64),
            11 => Ok(GgufMetadataType::Int64),
            12 => Ok(GgufMetadataType::Float64),
            _ => Err(GgufError::InvalidMetadataType(value)),
        }
    }
}

/// GGUF tensor data types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum GgmlType {
    F32 = 0,
    F16 = 1,
    Q4_0 = 2,
    Q4_1 = 3,
    Q5_0 = 6,
    Q5_1 = 7,
    Q8_0 = 8,
    Q8_1 = 9,
    Q2K = 10,
    Q3K = 11,
    Q4K = 12,
    Q5K = 13,
    Q6K = 14,
    Q8K = 15,
    Iq2Xxs = 16,
    Iq2Xs = 17,
    Iq3Xxs = 18,
    Iq1S = 19,
    Iq4Nl = 20,
    Iq3S = 21,
    Iq2S = 22,
    Iq4Xs = 23,
    I8 = 24,
    I16 = 25,
    I32 = 26,
    I64 = 27,
    F64 = 28,
    Bf16 = 29,
}

impl GgmlType {
    /// Get block size for quantized types
    pub fn block_size(&self) -> usize {
        match self {
            GgmlType::F32 | GgmlType::F16 | GgmlType::Bf16 | GgmlType::F64 => 1,
            GgmlType::I8 | GgmlType::I16 | GgmlType::I32 | GgmlType::I64 => 1,
            GgmlType::Q4_0 | GgmlType::Q4_1 => 32,
            GgmlType::Q5_0 | GgmlType::Q5_1 => 32,
            GgmlType::Q8_0 | GgmlType::Q8_1 => 32,
            GgmlType::Q2K
            | GgmlType::Q3K
            | GgmlType::Q4K
            | GgmlType::Q5K
            | GgmlType::Q6K
            | GgmlType::Q8K => 256,
            _ => 32, // Default for newer types
        }
    }

    /// Get bytes per block
    pub fn block_bytes(&self) -> usize {
        match self {
            GgmlType::F32 => 4,
            GgmlType::F16 | GgmlType::Bf16 => 2,
            GgmlType::F64 => 8,
            GgmlType::I8 => 1,
            GgmlType::I16 => 2,
            GgmlType::I32 => 4,
            GgmlType::I64 => 8,
            GgmlType::Q4_0 => 18, // 32 * 4/8 + 2 (scale)
            GgmlType::Q4_1 => 20, // 32 * 4/8 + 2 (scale) + 2 (min)
            GgmlType::Q5_0 => 22, // 32 * 5/8 + 2 (scale) (approx)
            GgmlType::Q5_1 => 24,
            GgmlType::Q8_0 => 34, // 32 * 1 + 2 (scale)
            GgmlType::Q8_1 => 36,
            GgmlType::Q4K => 144, // Complex super-block format
            _ => 32,              // Approximation
        }
    }
}

impl TryFrom<u32> for GgmlType {
    type Error = GgufError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(GgmlType::F32),
            1 => Ok(GgmlType::F16),
            2 => Ok(GgmlType::Q4_0),
            3 => Ok(GgmlType::Q4_1),
            6 => Ok(GgmlType::Q5_0),
            7 => Ok(GgmlType::Q5_1),
            8 => Ok(GgmlType::Q8_0),
            9 => Ok(GgmlType::Q8_1),
            10 => Ok(GgmlType::Q2K),
            11 => Ok(GgmlType::Q3K),
            12 => Ok(GgmlType::Q4K),
            13 => Ok(GgmlType::Q5K),
            14 => Ok(GgmlType::Q6K),
            15 => Ok(GgmlType::Q8K),
            24 => Ok(GgmlType::I8),
            25 => Ok(GgmlType::I16),
            26 => Ok(GgmlType::I32),
            27 => Ok(GgmlType::I64),
            28 => Ok(GgmlType::F64),
            29 => Ok(GgmlType::Bf16),
            _ => Err(GgufError::InvalidTensorType(value)),
        }
    }
}

// ============================================================================
// GGUF Error Types
// ============================================================================

#[derive(Debug, Clone)]
pub enum GgufError {
    InvalidMagic(u32),
    UnsupportedVersion(u32),
    InvalidMetadataType(u32),
    InvalidTensorType(u32),
    MissingMetadata(String),
    InvalidData(String),
    IoError(String),
}

impl std::fmt::Display for GgufError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GgufError::InvalidMagic(m) => write!(f, "Invalid GGUF magic: 0x{:08X}", m),
            GgufError::UnsupportedVersion(v) => write!(f, "Unsupported GGUF version: {}", v),
            GgufError::InvalidMetadataType(t) => write!(f, "Invalid metadata type: {}", t),
            GgufError::InvalidTensorType(t) => write!(f, "Invalid tensor type: {}", t),
            GgufError::MissingMetadata(k) => write!(f, "Missing metadata key: {}", k),
            GgufError::InvalidData(s) => write!(f, "Invalid data: {}", s),
            GgufError::IoError(s) => write!(f, "IO error: {}", s),
        }
    }
}

impl std::error::Error for GgufError {}

// ============================================================================
// GGUF Structures
// ============================================================================

/// GGUF file header
#[derive(Debug, Clone)]
pub struct GgufHeader {
    pub magic: u32,
    pub version: u32,
    pub tensor_count: u64,
    pub metadata_kv_count: u64,
}

/// GGUF metadata value
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
    pub fn as_u32(&self) -> Option<u32> {
        match self {
            GgufValue::Uint8(v) => Some(*v as u32),
            GgufValue::Int8(v) => Some(*v as u32),
            GgufValue::Uint16(v) => Some(*v as u32),
            GgufValue::Int16(v) => Some(*v as u32),
            GgufValue::Uint32(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            GgufValue::Uint64(v) => Some(*v),
            GgufValue::Uint32(v) => Some(*v as u64),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<&str> {
        match self {
            GgufValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_f32(&self) -> Option<f32> {
        match self {
            GgufValue::Float32(v) => Some(*v),
            _ => None,
        }
    }
}

/// GGUF tensor info
#[derive(Debug, Clone)]
pub struct GgufTensorInfo {
    pub name: String,
    pub dimensions: Vec<u64>,
    pub dtype: GgmlType,
    pub offset: u64,
}

impl GgufTensorInfo {
    /// Calculate number of elements
    pub fn num_elements(&self) -> u64 {
        self.dimensions.iter().product()
    }

    /// Calculate data size in bytes
    pub fn data_size(&self) -> usize {
        let num_elements = self.num_elements() as usize;
        let block_size = self.dtype.block_size();
        let num_blocks = (num_elements + block_size - 1) / block_size;
        num_blocks * self.dtype.block_bytes()
    }
}

/// GGUF file representation
#[derive(Debug)]
pub struct GgufFile {
    pub header: GgufHeader,
    pub metadata: HashMap<String, GgufValue>,
    pub tensors: Vec<GgufTensorInfo>,
    data_offset: u64,
}

impl GgufFile {
    /// Parse GGUF from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self, GgufError> {
        let mut cursor = Cursor::new(data);
        Self::from_reader(&mut cursor)
    }

    /// Parse GGUF from reader
    pub fn from_reader<R: Read>(reader: &mut R) -> Result<Self, GgufError> {
        // Read header
        let header = Self::read_header(reader)?;

        // Read metadata
        let mut metadata = HashMap::new();
        for _ in 0..header.metadata_kv_count {
            let (key, value) = Self::read_metadata_kv(reader)?;
            metadata.insert(key, value);
        }

        // Read tensor info
        let mut tensors = Vec::new();
        for _ in 0..header.tensor_count {
            let tensor = Self::read_tensor_info(reader)?;
            tensors.push(tensor);
        }

        // Calculate data offset (simplified - in production would track exact position)
        let data_offset = 0; // Would be calculated from reader position

        Ok(Self {
            header,
            metadata,
            tensors,
            data_offset,
        })
    }

    fn read_header<R: Read>(reader: &mut R) -> Result<GgufHeader, GgufError> {
        let mut buf = [0u8; 4];

        reader
            .read_exact(&mut buf)
            .map_err(|e| GgufError::IoError(e.to_string()))?;
        let magic = u32::from_le_bytes(buf);
        if magic != GGUF_MAGIC {
            return Err(GgufError::InvalidMagic(magic));
        }

        reader
            .read_exact(&mut buf)
            .map_err(|e| GgufError::IoError(e.to_string()))?;
        let version = u32::from_le_bytes(buf);
        if version > GGUF_VERSION {
            return Err(GgufError::UnsupportedVersion(version));
        }

        let mut buf8 = [0u8; 8];
        reader
            .read_exact(&mut buf8)
            .map_err(|e| GgufError::IoError(e.to_string()))?;
        let tensor_count = u64::from_le_bytes(buf8);

        reader
            .read_exact(&mut buf8)
            .map_err(|e| GgufError::IoError(e.to_string()))?;
        let metadata_kv_count = u64::from_le_bytes(buf8);

        Ok(GgufHeader {
            magic,
            version,
            tensor_count,
            metadata_kv_count,
        })
    }

    fn read_string<R: Read>(reader: &mut R) -> Result<String, GgufError> {
        let mut buf8 = [0u8; 8];
        reader
            .read_exact(&mut buf8)
            .map_err(|e| GgufError::IoError(e.to_string()))?;
        let len = u64::from_le_bytes(buf8) as usize;

        let mut str_buf = vec![0u8; len];
        reader
            .read_exact(&mut str_buf)
            .map_err(|e| GgufError::IoError(e.to_string()))?;

        String::from_utf8(str_buf).map_err(|e| GgufError::InvalidData(e.to_string()))
    }

    fn read_metadata_kv<R: Read>(reader: &mut R) -> Result<(String, GgufValue), GgufError> {
        let key = Self::read_string(reader)?;

        let mut buf4 = [0u8; 4];
        reader
            .read_exact(&mut buf4)
            .map_err(|e| GgufError::IoError(e.to_string()))?;
        let value_type = GgufMetadataType::try_from(u32::from_le_bytes(buf4))?;

        let value = Self::read_metadata_value(reader, value_type)?;

        Ok((key, value))
    }

    fn read_metadata_value<R: Read>(
        reader: &mut R,
        value_type: GgufMetadataType,
    ) -> Result<GgufValue, GgufError> {
        let mut buf1 = [0u8; 1];
        let mut buf2 = [0u8; 2];
        let mut buf4 = [0u8; 4];
        let mut buf8 = [0u8; 8];

        match value_type {
            GgufMetadataType::Uint8 => {
                reader
                    .read_exact(&mut buf1)
                    .map_err(|e| GgufError::IoError(e.to_string()))?;
                Ok(GgufValue::Uint8(buf1[0]))
            }
            GgufMetadataType::Int8 => {
                reader
                    .read_exact(&mut buf1)
                    .map_err(|e| GgufError::IoError(e.to_string()))?;
                Ok(GgufValue::Int8(buf1[0] as i8))
            }
            GgufMetadataType::Uint16 => {
                reader
                    .read_exact(&mut buf2)
                    .map_err(|e| GgufError::IoError(e.to_string()))?;
                Ok(GgufValue::Uint16(u16::from_le_bytes(buf2)))
            }
            GgufMetadataType::Int16 => {
                reader
                    .read_exact(&mut buf2)
                    .map_err(|e| GgufError::IoError(e.to_string()))?;
                Ok(GgufValue::Int16(i16::from_le_bytes(buf2)))
            }
            GgufMetadataType::Uint32 => {
                reader
                    .read_exact(&mut buf4)
                    .map_err(|e| GgufError::IoError(e.to_string()))?;
                Ok(GgufValue::Uint32(u32::from_le_bytes(buf4)))
            }
            GgufMetadataType::Int32 => {
                reader
                    .read_exact(&mut buf4)
                    .map_err(|e| GgufError::IoError(e.to_string()))?;
                Ok(GgufValue::Int32(i32::from_le_bytes(buf4)))
            }
            GgufMetadataType::Float32 => {
                reader
                    .read_exact(&mut buf4)
                    .map_err(|e| GgufError::IoError(e.to_string()))?;
                Ok(GgufValue::Float32(f32::from_le_bytes(buf4)))
            }
            GgufMetadataType::Bool => {
                reader
                    .read_exact(&mut buf1)
                    .map_err(|e| GgufError::IoError(e.to_string()))?;
                Ok(GgufValue::Bool(buf1[0] != 0))
            }
            GgufMetadataType::String => {
                let s = Self::read_string(reader)?;
                Ok(GgufValue::String(s))
            }
            GgufMetadataType::Array => {
                // Read array type and length
                reader
                    .read_exact(&mut buf4)
                    .map_err(|e| GgufError::IoError(e.to_string()))?;
                let elem_type = GgufMetadataType::try_from(u32::from_le_bytes(buf4))?;

                reader
                    .read_exact(&mut buf8)
                    .map_err(|e| GgufError::IoError(e.to_string()))?;
                let len = u64::from_le_bytes(buf8) as usize;

                let mut arr = Vec::with_capacity(len);
                for _ in 0..len {
                    arr.push(Self::read_metadata_value(reader, elem_type)?);
                }
                Ok(GgufValue::Array(arr))
            }
            GgufMetadataType::Uint64 => {
                reader
                    .read_exact(&mut buf8)
                    .map_err(|e| GgufError::IoError(e.to_string()))?;
                Ok(GgufValue::Uint64(u64::from_le_bytes(buf8)))
            }
            GgufMetadataType::Int64 => {
                reader
                    .read_exact(&mut buf8)
                    .map_err(|e| GgufError::IoError(e.to_string()))?;
                Ok(GgufValue::Int64(i64::from_le_bytes(buf8)))
            }
            GgufMetadataType::Float64 => {
                reader
                    .read_exact(&mut buf8)
                    .map_err(|e| GgufError::IoError(e.to_string()))?;
                Ok(GgufValue::Float64(f64::from_le_bytes(buf8)))
            }
        }
    }

    fn read_tensor_info<R: Read>(reader: &mut R) -> Result<GgufTensorInfo, GgufError> {
        let name = Self::read_string(reader)?;

        // Read number of dimensions
        let mut buf4 = [0u8; 4];
        reader
            .read_exact(&mut buf4)
            .map_err(|e| GgufError::IoError(e.to_string()))?;
        let n_dims = u32::from_le_bytes(buf4) as usize;

        // Read dimensions
        let mut dimensions = Vec::with_capacity(n_dims);
        let mut buf8 = [0u8; 8];
        for _ in 0..n_dims {
            reader
                .read_exact(&mut buf8)
                .map_err(|e| GgufError::IoError(e.to_string()))?;
            dimensions.push(u64::from_le_bytes(buf8));
        }

        // Read type
        reader
            .read_exact(&mut buf4)
            .map_err(|e| GgufError::IoError(e.to_string()))?;
        let dtype = GgmlType::try_from(u32::from_le_bytes(buf4))?;

        // Read offset
        reader
            .read_exact(&mut buf8)
            .map_err(|e| GgufError::IoError(e.to_string()))?;
        let offset = u64::from_le_bytes(buf8);

        Ok(GgufTensorInfo {
            name,
            dimensions,
            dtype,
            offset,
        })
    }

    /// Get architecture from metadata
    pub fn architecture(&self) -> Option<&str> {
        self.metadata.get("general.architecture")?.as_string()
    }

    /// Get context length from metadata
    pub fn context_length(&self) -> Option<u64> {
        // Try various keys for context length
        if let Some(v) = self.metadata.get("llama.context_length") {
            return v.as_u64();
        }
        if let Some(v) = self.metadata.get("general.context_length") {
            return v.as_u64();
        }
        None
    }

    /// Get embedding length from metadata
    pub fn embedding_length(&self) -> Option<u64> {
        if let Some(v) = self.metadata.get("llama.embedding_length") {
            return v.as_u64();
        }
        None
    }

    /// Get number of attention heads
    pub fn attention_head_count(&self) -> Option<u64> {
        if let Some(v) = self.metadata.get("llama.attention.head_count") {
            return v.as_u64();
        }
        None
    }
}

// ============================================================================
// Test Helpers
// ============================================================================

/// Create a minimal valid GGUF file for testing
fn create_test_gguf() -> Vec<u8> {
    let mut data = Vec::new();

    // Magic
    data.extend_from_slice(&GGUF_MAGIC.to_le_bytes());

    // Version
    data.extend_from_slice(&GGUF_VERSION.to_le_bytes());

    // Tensor count
    data.extend_from_slice(&0u64.to_le_bytes());

    // Metadata count
    data.extend_from_slice(&0u64.to_le_bytes());

    data
}

/// Create a GGUF file with metadata
fn create_test_gguf_with_metadata() -> Vec<u8> {
    let mut data = Vec::new();

    // Magic
    data.extend_from_slice(&GGUF_MAGIC.to_le_bytes());

    // Version
    data.extend_from_slice(&GGUF_VERSION.to_le_bytes());

    // Tensor count
    data.extend_from_slice(&1u64.to_le_bytes());

    // Metadata count
    data.extend_from_slice(&3u64.to_le_bytes());

    // Metadata 1: architecture (string)
    let key = "general.architecture";
    data.extend_from_slice(&(key.len() as u64).to_le_bytes());
    data.extend_from_slice(key.as_bytes());
    data.extend_from_slice(&(GgufMetadataType::String as u32).to_le_bytes());
    let value = "llama";
    data.extend_from_slice(&(value.len() as u64).to_le_bytes());
    data.extend_from_slice(value.as_bytes());

    // Metadata 2: context length (u32)
    let key = "llama.context_length";
    data.extend_from_slice(&(key.len() as u64).to_le_bytes());
    data.extend_from_slice(key.as_bytes());
    data.extend_from_slice(&(GgufMetadataType::Uint32 as u32).to_le_bytes());
    data.extend_from_slice(&4096u32.to_le_bytes());

    // Metadata 3: embedding length (u32)
    let key = "llama.embedding_length";
    data.extend_from_slice(&(key.len() as u64).to_le_bytes());
    data.extend_from_slice(key.as_bytes());
    data.extend_from_slice(&(GgufMetadataType::Uint32 as u32).to_le_bytes());
    data.extend_from_slice(&4096u32.to_le_bytes());

    // Tensor info
    let name = "model.embed_tokens.weight";
    data.extend_from_slice(&(name.len() as u64).to_le_bytes());
    data.extend_from_slice(name.as_bytes());
    data.extend_from_slice(&2u32.to_le_bytes()); // n_dims
    data.extend_from_slice(&32000u64.to_le_bytes()); // vocab_size
    data.extend_from_slice(&4096u64.to_le_bytes()); // hidden_size
    data.extend_from_slice(&(GgmlType::Q4K as u32).to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes()); // offset

    data
}

// ============================================================================
// Quantization Helpers
// ============================================================================

/// Q4_0 block structure (32 elements)
#[repr(C, packed)]
pub struct BlockQ4_0 {
    pub d: u16,       // Scale as f16
    pub qs: [u8; 16], // Packed 4-bit values
}

/// Dequantize Q4_0 block to f32
pub fn dequantize_q4_0(quantized: &[u8], output: &mut [f32]) {
    const BLOCK_SIZE: usize = 32;

    let num_blocks = output.len() / BLOCK_SIZE;

    for block_idx in 0..num_blocks {
        let block_start = block_idx * 18; // 2 bytes scale + 16 bytes data

        if block_start + 18 > quantized.len() {
            break;
        }

        // Read scale (f16)
        let scale_bits = u16::from_le_bytes([quantized[block_start], quantized[block_start + 1]]);
        let scale = f16_to_f32(scale_bits);

        // Dequantize 32 values from 16 bytes
        for i in 0..16 {
            let byte = quantized[block_start + 2 + i];
            let q0 = (byte & 0x0F) as i8 - 8;
            let q1 = ((byte >> 4) & 0x0F) as i8 - 8;

            let out_idx = block_idx * BLOCK_SIZE + i * 2;
            if out_idx < output.len() {
                output[out_idx] = (q0 as f32) * scale;
            }
            if out_idx + 1 < output.len() {
                output[out_idx + 1] = (q1 as f32) * scale;
            }
        }
    }
}

/// Quantize f32 to Q4_0
pub fn quantize_q4_0(data: &[f32]) -> (Vec<u8>, f32, f32) {
    const BLOCK_SIZE: usize = 32;

    let num_blocks = (data.len() + BLOCK_SIZE - 1) / BLOCK_SIZE;
    let mut output = Vec::with_capacity(num_blocks * 18);

    for block_idx in 0..num_blocks {
        let start = block_idx * BLOCK_SIZE;
        let end = (start + BLOCK_SIZE).min(data.len());
        let block = &data[start..end];

        // Find max absolute value
        let max_abs = block.iter().fold(0.0f32, |acc, &x| acc.max(x.abs()));
        let scale = if max_abs > 0.0 { max_abs / 7.0 } else { 1.0 };

        // Write scale as f16
        let scale_f16 = f32_to_f16(scale);
        output.push((scale_f16 & 0xFF) as u8);
        output.push((scale_f16 >> 8) as u8);

        // Quantize and pack
        let inv_scale = if scale > 0.0 { 1.0 / scale } else { 0.0 };
        for i in (0..32).step_by(2) {
            let v0 = if i < block.len() {
                ((block[i] * inv_scale).round().clamp(-8.0, 7.0) + 8.0) as u8
            } else {
                8 // Zero = 8 in signed Q4
            };
            let v1 = if i + 1 < block.len() {
                ((block[i + 1] * inv_scale).round().clamp(-8.0, 7.0) + 8.0) as u8
            } else {
                8
            };
            output.push((v0 & 0x0F) | ((v1 & 0x0F) << 4));
        }
    }

    (output, 0.0, 0.0) // No separate zero point for Q4_0
}

/// Convert f32 to f16
fn f32_to_f16(x: f32) -> u16 {
    let bits = x.to_bits();
    let sign = (bits >> 16) & 0x8000;
    let exp = ((bits >> 23) & 0xFF) as i32;
    let frac = bits & 0x007F_FFFF;

    if exp == 0xFF {
        return (sign | 0x7C00 | ((frac != 0) as u32 * 0x0200)) as u16;
    }

    let new_exp = exp - 127 + 15;

    if new_exp >= 31 {
        return (sign | 0x7C00) as u16;
    }

    if new_exp <= 0 {
        if new_exp < -10 {
            return sign as u16;
        }
        let frac = (frac | 0x0080_0000) >> (14 - new_exp);
        return (sign | (frac >> 13)) as u16;
    }

    (sign | ((new_exp as u32) << 10) | (frac >> 13)) as u16
}

/// Convert f16 to f32
fn f16_to_f32(x: u16) -> f32 {
    let sign = ((x & 0x8000) as u32) << 16;
    let exp = ((x >> 10) & 0x1F) as u32;
    let frac = (x & 0x03FF) as u32;

    if exp == 0 {
        if frac == 0 {
            return f32::from_bits(sign);
        }
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
        return f32::from_bits(sign | 0x7F80_0000 | (frac << 13));
    }

    f32::from_bits(sign | ((exp + 127 - 15) << 23) | (frac << 13))
}

// ============================================================================
// Tests
// ============================================================================

#[test]
fn test_gguf_header_parsing() {
    let gguf_data = create_test_gguf();
    let file = GgufFile::from_bytes(&gguf_data).unwrap();

    assert_eq!(file.header.magic, GGUF_MAGIC);
    assert_eq!(file.header.version, GGUF_VERSION);
    assert_eq!(file.header.tensor_count, 0);
    assert_eq!(file.header.metadata_kv_count, 0);
}

#[test]
fn test_gguf_invalid_magic() {
    let mut data = Vec::new();
    data.extend_from_slice(&0x12345678u32.to_le_bytes()); // Wrong magic
    data.extend_from_slice(&3u32.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());

    let result = GgufFile::from_bytes(&data);
    assert!(matches!(result, Err(GgufError::InvalidMagic(0x12345678))));
}

#[test]
fn test_gguf_unsupported_version() {
    let mut data = Vec::new();
    data.extend_from_slice(&GGUF_MAGIC.to_le_bytes());
    data.extend_from_slice(&99u32.to_le_bytes()); // Future version
    data.extend_from_slice(&0u64.to_le_bytes());
    data.extend_from_slice(&0u64.to_le_bytes());

    let result = GgufFile::from_bytes(&data);
    assert!(matches!(result, Err(GgufError::UnsupportedVersion(99))));
}

#[test]
fn test_gguf_metadata_extraction() {
    let gguf_data = create_test_gguf_with_metadata();
    let file = GgufFile::from_bytes(&gguf_data).unwrap();

    assert_eq!(file.architecture(), Some("llama"));
    assert_eq!(file.context_length(), Some(4096));
    assert_eq!(file.embedding_length(), Some(4096));
}

#[test]
fn test_gguf_tensor_info() {
    let gguf_data = create_test_gguf_with_metadata();
    let file = GgufFile::from_bytes(&gguf_data).unwrap();

    assert_eq!(file.tensors.len(), 1);

    let tensor = &file.tensors[0];
    assert_eq!(tensor.name, "model.embed_tokens.weight");
    assert_eq!(tensor.dimensions, vec![32000, 4096]);
    assert_eq!(tensor.dtype, GgmlType::Q4K);
    assert_eq!(tensor.num_elements(), 32000 * 4096);
}

#[test]
fn test_quantization_dequantize_q4_0() {
    // Create test Q4_0 data: 1 block = 32 elements
    // Scale = 1.0 (encoded as f16)
    // Values: alternating pattern
    let mut quantized = Vec::new();

    // Scale as f16 (1.0 = 0x3C00)
    quantized.push(0x00);
    quantized.push(0x3C);

    // 16 bytes of packed values (all 8 = zero in signed Q4)
    for _ in 0..16 {
        quantized.push(0x88); // Two zeros (8|8)
    }

    let mut output = vec![0.0f32; 32];
    dequantize_q4_0(&quantized, &mut output);

    // All values should be ~0 (8 - 8 = 0, times scale 1.0)
    for v in &output {
        assert!(v.abs() < 1e-4, "Expected ~0, got {}", v);
    }
}

#[test]
fn test_quantization_roundtrip_accuracy() {
    // Create test data with varied values
    let original: Vec<f32> = (0..256).map(|i| (i as f32 - 128.0) / 128.0).collect();

    // Quantize
    let (quantized, _, _) = quantize_q4_0(&original);

    // Dequantize
    let mut restored = vec![0.0f32; 256];
    dequantize_q4_0(&quantized, &mut restored);

    // Check accuracy (Q4_0 should be within ~6-7% of original for most values)
    let max_error = original
        .iter()
        .zip(restored.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);

    // Q4_0 with 4-bit values can have significant error, especially for small values
    assert!(max_error < 0.2, "Max error {} exceeds 20%", max_error);
}

#[test]
fn test_quantization_extreme_values() {
    // Test with large values
    let large: Vec<f32> = vec![100.0; 32];
    let (q_large, _, _) = quantize_q4_0(&large);
    let mut d_large = vec![0.0f32; 32];
    dequantize_q4_0(&q_large, &mut d_large);

    // Values should be recoverable within quantization error
    for (orig, restored) in large.iter().zip(d_large.iter()) {
        let rel_error = (orig - restored).abs() / orig.abs().max(1e-6);
        assert!(
            rel_error < 0.2,
            "Large value error: {} vs {}",
            orig,
            restored
        );
    }

    // Test with small values
    let small: Vec<f32> = vec![0.001; 32];
    let (q_small, _, _) = quantize_q4_0(&small);
    let mut d_small = vec![0.0f32; 32];
    dequantize_q4_0(&q_small, &mut d_small);

    // Small values might not roundtrip well due to quantization
    for v in &d_small {
        assert!(v.is_finite(), "Dequantized value should be finite");
    }
}

#[test]
fn test_quantization_zeros() {
    let zeros: Vec<f32> = vec![0.0; 64];
    let (quantized, _, _) = quantize_q4_0(&zeros);
    let mut restored = vec![1.0f32; 64]; // Initialize with non-zero
    dequantize_q4_0(&quantized, &mut restored);

    for v in &restored {
        assert!(v.abs() < 1e-4, "Zero should remain zero, got {}", v);
    }
}

#[test]
fn test_f16_conversion() {
    let test_values = [0.0f32, 1.0, -1.0, 0.5, 0.125, 65504.0, -65504.0];

    for &v in &test_values {
        let h = f32_to_f16(v);
        let back = f16_to_f32(h);
        let error = (v - back).abs() / v.abs().max(1e-6);
        assert!(
            error < 0.01 || (v - back).abs() < 1e-3,
            "F16 roundtrip error for {}: {} -> {} -> {}",
            v,
            v,
            h,
            back
        );
    }
}

#[test]
fn test_f16_special_values() {
    // Zero
    let zero_h = f32_to_f16(0.0);
    assert_eq!(f16_to_f32(zero_h), 0.0);

    // Negative zero
    let neg_zero_h = f32_to_f16(-0.0);
    assert!(f16_to_f32(neg_zero_h).is_sign_negative() || f16_to_f32(neg_zero_h) == 0.0);

    // Infinity
    let inf_h = f32_to_f16(f32::INFINITY);
    assert!(f16_to_f32(inf_h).is_infinite());

    // NaN
    let nan_h = f32_to_f16(f32::NAN);
    assert!(f16_to_f32(nan_h).is_nan());
}

#[test]
fn test_ggml_type_block_sizes() {
    // F32 is element-wise
    assert_eq!(GgmlType::F32.block_size(), 1);
    assert_eq!(GgmlType::F32.block_bytes(), 4);

    // F16 is element-wise
    assert_eq!(GgmlType::F16.block_size(), 1);
    assert_eq!(GgmlType::F16.block_bytes(), 2);

    // Q4_0 uses 32-element blocks
    assert_eq!(GgmlType::Q4_0.block_size(), 32);
    assert_eq!(GgmlType::Q4_0.block_bytes(), 18); // 2 + 16

    // Q4K uses 256-element super-blocks
    assert_eq!(GgmlType::Q4K.block_size(), 256);
}

#[test]
fn test_tensor_info_calculations() {
    let tensor = GgufTensorInfo {
        name: "test.weight".to_string(),
        dimensions: vec![4096, 4096],
        dtype: GgmlType::F16,
        offset: 0,
    };

    assert_eq!(tensor.num_elements(), 4096 * 4096);
    assert_eq!(tensor.data_size(), 4096 * 4096 * 2); // F16 = 2 bytes

    let q_tensor = GgufTensorInfo {
        name: "test.q_weight".to_string(),
        dimensions: vec![4096, 4096],
        dtype: GgmlType::Q4_0,
        offset: 0,
    };

    // Q4_0: (elements / 32) * 18 bytes
    let expected_size = ((4096 * 4096 + 31) / 32) * 18;
    assert_eq!(q_tensor.data_size(), expected_size);
}

#[test]
fn test_metadata_value_types() {
    // Test all value type conversions
    let u32_val = GgufValue::Uint32(42);
    assert_eq!(u32_val.as_u32(), Some(42));
    assert_eq!(u32_val.as_string(), None);

    let string_val = GgufValue::String("test".to_string());
    assert_eq!(string_val.as_string(), Some("test"));
    assert_eq!(string_val.as_u32(), None);

    let f32_val = GgufValue::Float32(3.14);
    assert!((f32_val.as_f32().unwrap() - 3.14).abs() < 1e-6);

    let u64_val = GgufValue::Uint64(1_000_000_000_000);
    assert_eq!(u64_val.as_u64(), Some(1_000_000_000_000));
}

#[test]
fn test_gguf_alignment() {
    assert_eq!(GGUF_DEFAULT_ALIGNMENT, 32);
}

#[test]
fn test_block_q4_0_structure_size() {
    // BlockQ4_0 should be 18 bytes: 2 (d) + 16 (qs)
    assert_eq!(std::mem::size_of::<BlockQ4_0>(), 18);
}

// ============================================================================
// Multi-block Quantization Tests
// ============================================================================

#[test]
fn test_multi_block_quantization() {
    // Test with multiple blocks worth of data
    let data: Vec<f32> = (0..128).map(|i| (i as f32 - 64.0) / 64.0).collect();

    let (quantized, _, _) = quantize_q4_0(&data);

    // Should have 4 blocks (128 / 32)
    assert_eq!(quantized.len(), 4 * 18); // 4 blocks * 18 bytes

    let mut restored = vec![0.0f32; 128];
    dequantize_q4_0(&quantized, &mut restored);

    // Check that dequantization produces reasonable values
    for (i, v) in restored.iter().enumerate() {
        assert!(v.is_finite(), "Value {} at index {} should be finite", v, i);
    }
}

#[test]
fn test_non_aligned_data_length() {
    // Test with data that's not a multiple of block size
    let data: Vec<f32> = vec![1.0; 50]; // Not a multiple of 32

    let (quantized, _, _) = quantize_q4_0(&data);

    // Should have 2 blocks (ceiling division)
    assert_eq!(quantized.len(), 2 * 18);

    let mut restored = vec![0.0f32; 64]; // Pad to block boundary
    dequantize_q4_0(&quantized, &mut restored);

    // First 50 values should be close to 1.0
    for (i, &v) in restored.iter().take(50).enumerate() {
        let error = (v - 1.0).abs();
        assert!(error < 0.2, "Value {} at {} should be ~1.0", v, i);
    }
}
