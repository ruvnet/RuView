//! GGUF Binary Format Parser
//!
//! This module implements the GGUF v3 binary format parser for reading
//! llama.cpp model files. The parser handles:
//!
//! - Header parsing (magic, version, counts)
//! - Metadata key-value pairs with typed values
//! - Tensor information extraction
//!
//! ## GGUF Format Structure
//!
//! ```text
//! +------------------+
//! | Header (24 bytes)|  magic, version, tensor_count, metadata_count
//! +------------------+
//! | Metadata KV      |  key-value pairs with type information
//! | ...              |
//! +------------------+
//! | Tensor Infos     |  name, shape, type, offset for each tensor
//! | ...              |
//! +------------------+
//! | Alignment Pad    |  padding to alignment boundary
//! +------------------+
//! | Tensor Data      |  raw tensor data (may be quantized)
//! | ...              |
//! +------------------+
//! ```

use std::collections::HashMap;
use std::io::{BufRead, Read};

use super::quantization::GgufQuantType;
use super::tensors::TensorInfo;
use crate::error::{Result, RuvLLMError};

// ============================================================================
// Header Structure
// ============================================================================

/// GGUF file header.
///
/// The header contains basic information about the GGUF file including
/// version, tensor count, and metadata count.
#[derive(Debug, Clone)]
pub struct GgufHeader {
    /// Magic number (should be GGUF_MAGIC)
    pub magic: u32,
    /// GGUF format version (2 or 3)
    pub version: u32,
    /// Number of tensors in the file
    pub tensor_count: u64,
    /// Number of metadata key-value pairs
    pub metadata_kv_count: u64,
}

// ============================================================================
// Metadata Value Types
// ============================================================================

/// GGUF metadata value types.
///
/// GGUF supports a variety of value types for storing model metadata,
/// from simple integers to arrays and strings.
#[derive(Debug, Clone)]
pub enum GgufValue {
    /// Unsigned 8-bit integer
    U8(u8),
    /// Signed 8-bit integer
    I8(i8),
    /// Unsigned 16-bit integer
    U16(u16),
    /// Signed 16-bit integer
    I16(i16),
    /// Unsigned 32-bit integer
    U32(u32),
    /// Signed 32-bit integer
    I32(i32),
    /// Unsigned 64-bit integer
    U64(u64),
    /// Signed 64-bit integer
    I64(i64),
    /// 32-bit floating point
    F32(f32),
    /// 64-bit floating point
    F64(f64),
    /// Boolean value
    Bool(bool),
    /// UTF-8 string
    String(String),
    /// Array of values (all same type)
    Array(Vec<GgufValue>),
}

impl GgufValue {
    /// Try to get as string reference.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            GgufValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// Try to get as u64.
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            GgufValue::U8(v) => Some(*v as u64),
            GgufValue::U16(v) => Some(*v as u64),
            GgufValue::U32(v) => Some(*v as u64),
            GgufValue::U64(v) => Some(*v),
            GgufValue::I8(v) if *v >= 0 => Some(*v as u64),
            GgufValue::I16(v) if *v >= 0 => Some(*v as u64),
            GgufValue::I32(v) if *v >= 0 => Some(*v as u64),
            GgufValue::I64(v) if *v >= 0 => Some(*v as u64),
            _ => None,
        }
    }

    /// Try to get as i64.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            GgufValue::I8(v) => Some(*v as i64),
            GgufValue::I16(v) => Some(*v as i64),
            GgufValue::I32(v) => Some(*v as i64),
            GgufValue::I64(v) => Some(*v),
            GgufValue::U8(v) => Some(*v as i64),
            GgufValue::U16(v) => Some(*v as i64),
            GgufValue::U32(v) => Some(*v as i64),
            GgufValue::U64(v) if *v <= i64::MAX as u64 => Some(*v as i64),
            _ => None,
        }
    }

    /// Try to get as f32.
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            GgufValue::F32(v) => Some(*v),
            GgufValue::F64(v) => Some(*v as f32),
            GgufValue::I8(v) => Some(*v as f32),
            GgufValue::I16(v) => Some(*v as f32),
            GgufValue::I32(v) => Some(*v as f32),
            GgufValue::U8(v) => Some(*v as f32),
            GgufValue::U16(v) => Some(*v as f32),
            GgufValue::U32(v) => Some(*v as f32),
            _ => None,
        }
    }

    /// Try to get as f64.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            GgufValue::F64(v) => Some(*v),
            GgufValue::F32(v) => Some(*v as f64),
            GgufValue::I8(v) => Some(*v as f64),
            GgufValue::I16(v) => Some(*v as f64),
            GgufValue::I32(v) => Some(*v as f64),
            GgufValue::I64(v) => Some(*v as f64),
            GgufValue::U8(v) => Some(*v as f64),
            GgufValue::U16(v) => Some(*v as f64),
            GgufValue::U32(v) => Some(*v as f64),
            GgufValue::U64(v) => Some(*v as f64),
            _ => None,
        }
    }

    /// Try to get as bool.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            GgufValue::Bool(v) => Some(*v),
            GgufValue::U8(v) => Some(*v != 0),
            GgufValue::I8(v) => Some(*v != 0),
            _ => None,
        }
    }

    /// Try to get as array.
    pub fn as_array(&self) -> Option<&[GgufValue]> {
        match self {
            GgufValue::Array(arr) => Some(arr),
            _ => None,
        }
    }
}

// ============================================================================
// Value Type IDs
// ============================================================================

/// GGUF value type identifiers (from llama.cpp).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum GgufValueType {
    U8 = 0,
    I8 = 1,
    U16 = 2,
    I16 = 3,
    U32 = 4,
    I32 = 5,
    F32 = 6,
    Bool = 7,
    String = 8,
    Array = 9,
    U64 = 10,
    I64 = 11,
    F64 = 12,
}

impl TryFrom<u32> for GgufValueType {
    type Error = RuvLLMError;

    fn try_from(value: u32) -> Result<Self> {
        match value {
            0 => Ok(Self::U8),
            1 => Ok(Self::I8),
            2 => Ok(Self::U16),
            3 => Ok(Self::I16),
            4 => Ok(Self::U32),
            5 => Ok(Self::I32),
            6 => Ok(Self::F32),
            7 => Ok(Self::Bool),
            8 => Ok(Self::String),
            9 => Ok(Self::Array),
            10 => Ok(Self::U64),
            11 => Ok(Self::I64),
            12 => Ok(Self::F64),
            _ => Err(RuvLLMError::Model(format!(
                "Unknown GGUF value type: {}",
                value
            ))),
        }
    }
}

// ============================================================================
// Parsing Functions
// ============================================================================

/// Parse the GGUF header from a reader.
///
/// # Arguments
///
/// * `reader` - A reader positioned at the start of the file
///
/// # Returns
///
/// The parsed header structure
pub fn parse_header<R: Read>(reader: &mut R) -> Result<GgufHeader> {
    let magic = read_u32(reader)?;
    let version = read_u32(reader)?;
    let tensor_count = read_u64(reader)?;
    let metadata_kv_count = read_u64(reader)?;

    Ok(GgufHeader {
        magic,
        version,
        tensor_count,
        metadata_kv_count,
    })
}

/// Parse all metadata key-value pairs.
///
/// # Arguments
///
/// * `reader` - A reader positioned after the header
/// * `count` - Number of key-value pairs to read
///
/// # Returns
///
/// HashMap of metadata key-value pairs
pub fn parse_metadata<R: Read>(reader: &mut R, count: u64) -> Result<HashMap<String, GgufValue>> {
    let mut metadata = HashMap::with_capacity(count as usize);

    for _ in 0..count {
        let key = read_string(reader)?;
        let value = read_value(reader)?;
        metadata.insert(key, value);
    }

    Ok(metadata)
}

/// Parse all tensor information entries.
///
/// # Arguments
///
/// * `reader` - A reader positioned after metadata
/// * `count` - Number of tensors to read
///
/// # Returns
///
/// Vector of tensor information structures
pub fn parse_tensor_infos<R: Read>(reader: &mut R, count: u64) -> Result<Vec<TensorInfo>> {
    let mut tensors = Vec::with_capacity(count as usize);

    for _ in 0..count {
        let name = read_string(reader)?;
        let n_dims = read_u32(reader)? as usize;

        let mut shape = Vec::with_capacity(n_dims);
        for _ in 0..n_dims {
            shape.push(read_u64(reader)? as usize);
        }

        let dtype_id = read_u32(reader)?;
        let dtype = GgufQuantType::try_from(dtype_id)?;
        let offset = read_u64(reader)?;

        tensors.push(TensorInfo {
            name,
            shape,
            dtype,
            offset,
        });
    }

    Ok(tensors)
}

// ============================================================================
// Value Reading
// ============================================================================

fn read_value<R: Read>(reader: &mut R) -> Result<GgufValue> {
    let type_id = read_u32(reader)?;
    let value_type = GgufValueType::try_from(type_id)?;

    match value_type {
        GgufValueType::U8 => Ok(GgufValue::U8(read_u8(reader)?)),
        GgufValueType::I8 => Ok(GgufValue::I8(read_i8(reader)?)),
        GgufValueType::U16 => Ok(GgufValue::U16(read_u16(reader)?)),
        GgufValueType::I16 => Ok(GgufValue::I16(read_i16(reader)?)),
        GgufValueType::U32 => Ok(GgufValue::U32(read_u32(reader)?)),
        GgufValueType::I32 => Ok(GgufValue::I32(read_i32(reader)?)),
        GgufValueType::U64 => Ok(GgufValue::U64(read_u64(reader)?)),
        GgufValueType::I64 => Ok(GgufValue::I64(read_i64(reader)?)),
        GgufValueType::F32 => Ok(GgufValue::F32(read_f32(reader)?)),
        GgufValueType::F64 => Ok(GgufValue::F64(read_f64(reader)?)),
        GgufValueType::Bool => Ok(GgufValue::Bool(read_u8(reader)? != 0)),
        GgufValueType::String => Ok(GgufValue::String(read_string(reader)?)),
        GgufValueType::Array => read_array(reader),
    }
}

/// Maximum allowed array size to prevent OOM attacks from malicious GGUF files.
/// Set to 10 million elements (about 80MB for u64 arrays).
const MAX_ARRAY_SIZE: usize = 10_000_000;

fn read_array<R: Read>(reader: &mut R) -> Result<GgufValue> {
    let elem_type_id = read_u32(reader)?;
    let elem_type = GgufValueType::try_from(elem_type_id)?;
    let count = read_u64(reader)?;

    // SECURITY FIX: Prevent integer overflow and OOM attacks from malicious GGUF files
    if count > MAX_ARRAY_SIZE as u64 {
        return Err(RuvLLMError::Model(format!(
            "Array size {} exceeds maximum allowed size {}",
            count, MAX_ARRAY_SIZE
        )));
    }

    let count = count as usize;
    let mut values = Vec::with_capacity(count);

    for _ in 0..count {
        let value = match elem_type {
            GgufValueType::U8 => GgufValue::U8(read_u8(reader)?),
            GgufValueType::I8 => GgufValue::I8(read_i8(reader)?),
            GgufValueType::U16 => GgufValue::U16(read_u16(reader)?),
            GgufValueType::I16 => GgufValue::I16(read_i16(reader)?),
            GgufValueType::U32 => GgufValue::U32(read_u32(reader)?),
            GgufValueType::I32 => GgufValue::I32(read_i32(reader)?),
            GgufValueType::U64 => GgufValue::U64(read_u64(reader)?),
            GgufValueType::I64 => GgufValue::I64(read_i64(reader)?),
            GgufValueType::F32 => GgufValue::F32(read_f32(reader)?),
            GgufValueType::F64 => GgufValue::F64(read_f64(reader)?),
            GgufValueType::Bool => GgufValue::Bool(read_u8(reader)? != 0),
            GgufValueType::String => GgufValue::String(read_string(reader)?),
            GgufValueType::Array => read_array(reader)?,
        };
        values.push(value);
    }

    Ok(GgufValue::Array(values))
}

// ============================================================================
// Primitive Reading Helpers
// ============================================================================

fn read_u8<R: Read>(reader: &mut R) -> Result<u8> {
    let mut buf = [0u8; 1];
    reader.read_exact(&mut buf).map_err(read_err)?;
    Ok(buf[0])
}

fn read_i8<R: Read>(reader: &mut R) -> Result<i8> {
    Ok(read_u8(reader)? as i8)
}

fn read_u16<R: Read>(reader: &mut R) -> Result<u16> {
    let mut buf = [0u8; 2];
    reader.read_exact(&mut buf).map_err(read_err)?;
    Ok(u16::from_le_bytes(buf))
}

fn read_i16<R: Read>(reader: &mut R) -> Result<i16> {
    let mut buf = [0u8; 2];
    reader.read_exact(&mut buf).map_err(read_err)?;
    Ok(i16::from_le_bytes(buf))
}

fn read_u32<R: Read>(reader: &mut R) -> Result<u32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf).map_err(read_err)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_i32<R: Read>(reader: &mut R) -> Result<i32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf).map_err(read_err)?;
    Ok(i32::from_le_bytes(buf))
}

fn read_u64<R: Read>(reader: &mut R) -> Result<u64> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf).map_err(read_err)?;
    Ok(u64::from_le_bytes(buf))
}

fn read_i64<R: Read>(reader: &mut R) -> Result<i64> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf).map_err(read_err)?;
    Ok(i64::from_le_bytes(buf))
}

fn read_f32<R: Read>(reader: &mut R) -> Result<f32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf).map_err(read_err)?;
    Ok(f32::from_le_bytes(buf))
}

fn read_f64<R: Read>(reader: &mut R) -> Result<f64> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf).map_err(read_err)?;
    Ok(f64::from_le_bytes(buf))
}

/// Maximum allowed string size to prevent memory exhaustion attacks.
/// SECURITY FIX (H-003): Reduced from 1MB to 64KB - sufficient for metadata strings
/// while preventing memory-based DoS attacks from malicious GGUF files.
const MAX_STRING_SIZE: usize = 65536; // 64KB

fn read_string<R: Read>(reader: &mut R) -> Result<String> {
    let len = read_u64(reader)? as usize;

    if len > MAX_STRING_SIZE {
        return Err(RuvLLMError::Model(format!(
            "String too long: {} bytes (max: {} bytes)",
            len, MAX_STRING_SIZE
        )));
    }

    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).map_err(read_err)?;

    String::from_utf8(buf).map_err(|e| RuvLLMError::Model(format!("Invalid UTF-8 string: {}", e)))
}

fn read_err(e: std::io::Error) -> RuvLLMError {
    RuvLLMError::Model(format!("Failed to read: {}", e))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_read_primitives() {
        // Test u32 reading
        let data = [0x47, 0x47, 0x55, 0x46]; // "GGUF" in little-endian
        let mut cursor = Cursor::new(data);
        assert_eq!(read_u32(&mut cursor).unwrap(), 0x46554747);

        // Test u64 reading
        let data = [0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let mut cursor = Cursor::new(data);
        assert_eq!(read_u64(&mut cursor).unwrap(), 1);

        // Test f32 reading
        let data = 1.0f32.to_le_bytes();
        let mut cursor = Cursor::new(data);
        assert_eq!(read_f32(&mut cursor).unwrap(), 1.0);
    }

    #[test]
    fn test_read_string() {
        // String: length (8 bytes) + data
        let mut data = vec![];
        data.extend_from_slice(&5u64.to_le_bytes()); // length = 5
        data.extend_from_slice(b"hello");

        let mut cursor = Cursor::new(data);
        assert_eq!(read_string(&mut cursor).unwrap(), "hello");
    }

    #[test]
    fn test_parse_header() {
        let mut data = vec![];
        data.extend_from_slice(&0x46554747u32.to_le_bytes()); // magic
        data.extend_from_slice(&3u32.to_le_bytes()); // version
        data.extend_from_slice(&10u64.to_le_bytes()); // tensor_count
        data.extend_from_slice(&5u64.to_le_bytes()); // metadata_kv_count

        let mut cursor = Cursor::new(data);
        let header = parse_header(&mut cursor).unwrap();

        assert_eq!(header.magic, 0x46554747);
        assert_eq!(header.version, 3);
        assert_eq!(header.tensor_count, 10);
        assert_eq!(header.metadata_kv_count, 5);
    }

    #[test]
    fn test_gguf_value_conversions() {
        // Test string
        let val = GgufValue::String("test".to_string());
        assert_eq!(val.as_str(), Some("test"));
        assert_eq!(val.as_u64(), None);

        // Test u32
        let val = GgufValue::U32(42);
        assert_eq!(val.as_u64(), Some(42));
        assert_eq!(val.as_i64(), Some(42));
        assert_eq!(val.as_f32(), Some(42.0));
        assert_eq!(val.as_str(), None);

        // Test i32
        let val = GgufValue::I32(-5);
        assert_eq!(val.as_i64(), Some(-5));
        assert_eq!(val.as_u64(), None); // Negative can't be u64

        // Test f32
        let val = GgufValue::F32(3.14);
        assert!((val.as_f32().unwrap() - 3.14).abs() < 0.001);
        assert!((val.as_f64().unwrap() - 3.14).abs() < 0.001);

        // Test bool
        let val = GgufValue::Bool(true);
        assert_eq!(val.as_bool(), Some(true));

        // Test array
        let val = GgufValue::Array(vec![GgufValue::U32(1), GgufValue::U32(2)]);
        assert_eq!(val.as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_value_type_conversion() {
        assert_eq!(GgufValueType::try_from(0).unwrap(), GgufValueType::U8);
        assert_eq!(GgufValueType::try_from(6).unwrap(), GgufValueType::F32);
        assert_eq!(GgufValueType::try_from(8).unwrap(), GgufValueType::String);
        assert!(GgufValueType::try_from(100).is_err());
    }
}
