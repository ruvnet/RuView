//! GGUF Export for BitNet b1.58 Ternary Tensors
//!
//! Serializes `TernaryTensor` data into GGUF v3 format, enabling deployment
//! of Craftsman Ultra models with mixed BitNet/FP16 tensor types.
//!
//! ## Block Format (BITNET_T158)
//!
//! Each 256-element block is encoded as 66 bytes:
//! - 64 bytes: packed 2-bit ternary data (4 values per byte, LSB-first)
//! - 2 bytes: FP16 scale factor (little-endian)

use std::collections::HashMap;
use std::io::{self, Cursor, Seek, Write};
use std::path::Path;

use super::ternary_tensor::TernaryTensor;
use crate::error::{Result, RuvLLMError};
use crate::gguf::quantization::GgufQuantType;
use crate::gguf::{self, DEFAULT_ALIGNMENT, GGUF_MAGIC, GGUF_VERSION};

// ============================================================================
// FP16 Conversion
// ============================================================================

/// Convert an f32 value to IEEE 754 half-precision bytes (little-endian).
///
/// Handles special cases: infinity, NaN, denormals, overflow, and underflow.
pub fn f32_to_f16_bytes(value: f32) -> [u8; 2] {
    let bits = value.to_bits();
    let sign = ((bits >> 31) & 1) as u16;
    let exp = ((bits >> 23) & 0xFF) as i32;
    let frac = bits & 0x007F_FFFF;

    let h: u16 = if exp == 255 {
        // Inf or NaN — preserve NaN by keeping fraction non-zero
        let h_frac = if frac != 0 { 0x0200 } else { 0 };
        (sign << 15) | 0x7C00 | h_frac
    } else if exp == 0 {
        // f32 zero or f32 denormal → f16 zero
        sign << 15
    } else {
        let unbiased = exp - 127;
        if unbiased > 15 {
            // Overflow → f16 infinity
            (sign << 15) | 0x7C00
        } else if unbiased < -24 {
            // Too small → f16 zero
            sign << 15
        } else if unbiased < -14 {
            // f16 denormal range
            let shift = (-14 - unbiased) as u32;
            let denorm = (0x0400 | (frac >> 13)) >> shift;
            (sign << 15) | denorm as u16
        } else {
            // Normal f16
            let h_exp = (unbiased + 15) as u16;
            let h_frac = (frac >> 13) as u16;
            (sign << 15) | (h_exp << 10) | h_frac
        }
    };

    h.to_le_bytes()
}

/// Convert IEEE 754 half-precision bits back to f32 (for roundtrip validation).
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
        f32::from_bits(sign | ((127 - 15 + 1 - e) << 23) | (f << 13))
    } else if exp == 31 {
        // Inf or NaN
        f32::from_bits(sign | 0x7F80_0000 | (frac << 13))
    } else {
        f32::from_bits(sign | ((exp + 127 - 15) << 23) | (frac << 13))
    }
}

// ============================================================================
// Export Tensor Types
// ============================================================================

/// A tensor prepared for GGUF export.
pub enum ExportTensor {
    /// BitNet b1.58 ternary tensor (BITNET_T158 quantization, type 30)
    Ternary(TernaryTensor),
    /// FP16 tensor with raw half-precision bytes and shape
    Fp16 {
        /// Raw FP16 data (2 bytes per element, little-endian)
        data: Vec<u8>,
        /// Tensor dimensions
        shape: Vec<usize>,
    },
}

// ============================================================================
// Tensor Serialization
// ============================================================================

/// Serialize a TernaryTensor into GGUF BITNET_T158 block format.
///
/// For each block of 256 elements:
/// - 64 bytes of packed 2-bit ternary data
/// - 2 bytes of FP16 scale factor
///
/// Total: 66 bytes per block, little-endian throughout.
pub fn serialize_bitnet_t158(tensor: &TernaryTensor) -> Vec<u8> {
    let num_blocks = tensor.num_blocks();
    let mut output = Vec::with_capacity(num_blocks * 66);

    for block_idx in 0..num_blocks {
        // Extract this block's 64 bytes of packed data
        let packed_start = block_idx * 64;
        let packed_end = (packed_start + 64).min(tensor.packed_data.len());
        let chunk = &tensor.packed_data[packed_start..packed_end];
        output.extend_from_slice(chunk);

        // Zero-pad if the last block is incomplete
        for _ in 0..(64 - chunk.len()) {
            output.push(0);
        }

        // Write FP16 scale
        let scale = tensor.scales.get(block_idx).copied().unwrap_or(0.0);
        output.extend_from_slice(&f32_to_f16_bytes(scale));
    }

    output
}

// ============================================================================
// Metadata Value
// ============================================================================

/// Metadata value types supported for GGUF export.
#[derive(Debug, Clone)]
pub enum MetadataValue {
    /// Unsigned 32-bit integer
    U32(u32),
    /// Signed 32-bit integer
    I32(i32),
    /// UTF-8 string
    String(String),
}

// ============================================================================
// GGUF Writer
// ============================================================================

/// GGUF v3 file writer for BitNet model export.
///
/// Writes a complete GGUF file with header, metadata key-value pairs,
/// tensor info entries, and aligned tensor data following the GGUF v3
/// binary layout with 32-byte alignment.
pub struct GgufBitnetWriter<W: Write + Seek> {
    writer: W,
}

impl<W: Write + Seek> GgufBitnetWriter<W> {
    /// Create a new writer wrapping the given output.
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    /// Consume the writer and return the underlying output.
    pub fn into_inner(self) -> W {
        self.writer
    }

    /// Write a complete GGUF file with the given metadata and tensors.
    pub fn write_model(
        &mut self,
        metadata: &[(&str, MetadataValue)],
        tensors: &[(&str, &ExportTensor)],
    ) -> Result<()> {
        // --- Header (24 bytes) ---
        self.write_u32(GGUF_MAGIC)?;
        self.write_u32(GGUF_VERSION)?;
        self.write_u64(tensors.len() as u64)?;
        self.write_u64(metadata.len() as u64)?;

        // --- Metadata KV pairs ---
        for &(key, ref value) in metadata {
            self.write_string(key)?;
            self.write_metadata_value(value)?;
        }

        // --- Compute tensor data sizes and aligned offsets ---
        let sizes: Vec<usize> = tensors.iter().map(|&(_, t)| tensor_data_size(t)).collect();
        let mut offsets = Vec::with_capacity(tensors.len());
        let mut cursor: u64 = 0;
        for (i, &size) in sizes.iter().enumerate() {
            offsets.push(cursor);
            cursor += size as u64;
            if i + 1 < sizes.len() {
                cursor = align_up(cursor, DEFAULT_ALIGNMENT as u64);
            }
        }

        // --- Tensor info entries ---
        for (i, &(name, tensor)) in tensors.iter().enumerate() {
            self.write_string(name)?;
            let (shape, dtype) = tensor_shape_and_type(tensor);
            self.write_u32(shape.len() as u32)?;
            for &dim in &shape {
                self.write_u64(dim as u64)?;
            }
            self.write_u32(dtype as u32)?;
            self.write_u64(offsets[i])?;
        }

        // --- Alignment padding before data section ---
        let pos = self.writer.stream_position().map_err(io_err)?;
        let aligned = align_up(pos, DEFAULT_ALIGNMENT as u64);
        if aligned > pos {
            self.writer
                .write_all(&vec![0u8; (aligned - pos) as usize])
                .map_err(io_err)?;
        }

        // --- Tensor data with inter-tensor alignment ---
        let mut data_written: u64 = 0;
        for (i, &(_, tensor)) in tensors.iter().enumerate() {
            // Pad to reach the computed offset for this tensor
            let pad = offsets[i] - data_written;
            if pad > 0 {
                self.writer
                    .write_all(&vec![0u8; pad as usize])
                    .map_err(io_err)?;
                data_written += pad;
            }
            let bytes = serialize_export_tensor(tensor);
            self.writer.write_all(&bytes).map_err(io_err)?;
            data_written += bytes.len() as u64;
        }

        self.writer.flush().map_err(io_err)?;
        Ok(())
    }

    fn write_u32(&mut self, v: u32) -> Result<()> {
        self.writer.write_all(&v.to_le_bytes()).map_err(io_err)
    }

    fn write_u64(&mut self, v: u64) -> Result<()> {
        self.writer.write_all(&v.to_le_bytes()).map_err(io_err)
    }

    fn write_string(&mut self, s: &str) -> Result<()> {
        self.write_u64(s.len() as u64)?;
        self.writer.write_all(s.as_bytes()).map_err(io_err)
    }

    fn write_metadata_value(&mut self, value: &MetadataValue) -> Result<()> {
        match value {
            MetadataValue::U32(v) => {
                self.write_u32(4)?; // GgufValueType::U32
                self.write_u32(*v)?;
            }
            MetadataValue::I32(v) => {
                self.write_u32(5)?; // GgufValueType::I32
                self.writer.write_all(&v.to_le_bytes()).map_err(io_err)?;
            }
            MetadataValue::String(s) => {
                self.write_u32(8)?; // GgufValueType::String
                self.write_string(s)?;
            }
        }
        Ok(())
    }
}

// ============================================================================
// Full Model Export
// ============================================================================

/// Export a Craftsman Ultra model to GGUF format with BitNet-specific metadata.
///
/// Identifies ternary (expert FFN) vs FP16 (router, embed, head, norms) tensors
/// and writes all data with correct quantization types. Adds standard BitNet
/// metadata including version, encoding, and block size.
///
/// # Security
///
/// Validates the output path to reject path traversal components (`..`).
pub fn export_craftsman_model(path: &Path, tensors: HashMap<String, ExportTensor>) -> Result<()> {
    // Security: reject paths containing ".." components to prevent path traversal
    for component in path.components() {
        if let std::path::Component::ParentDir = component {
            return Err(RuvLLMError::Model(format!(
                "Path traversal detected: export path must not contain '..' components, got: {:?}",
                path
            )));
        }
    }

    let file = std::fs::File::create(path)
        .map_err(|e| RuvLLMError::Model(format!("Failed to create file: {}", e)))?;
    let mut gguf = GgufBitnetWriter::new(file);

    let metadata: Vec<(&str, MetadataValue)> = vec![
        (
            "general.architecture",
            MetadataValue::String("craftsman".into()),
        ),
        ("craftsman.bitnet.version", MetadataValue::U32(1)),
        (
            "craftsman.bitnet.weight_encoding",
            MetadataValue::String("absmean_ternary".into()),
        ),
        ("craftsman.bitnet.activation_bits", MetadataValue::U32(8)),
        (
            "craftsman.bitnet.router_precision",
            MetadataValue::String("f16".into()),
        ),
        ("craftsman.bitnet.block_size", MetadataValue::U32(256)),
    ];

    // Sort tensor names for deterministic output
    let mut names: Vec<String> = tensors.keys().cloned().collect();
    names.sort();
    let refs: Vec<(&str, &ExportTensor)> = names
        .iter()
        .map(|n| (n.as_str(), tensors.get(n).unwrap()))
        .collect();

    gguf.write_model(&metadata, &refs)
}

// ============================================================================
// Validation
// ============================================================================

/// Validate an exported GGUF file by re-reading header, metadata, and tensor info.
///
/// Verifies the magic number, version, tensor count, required metadata keys,
/// and that all tensor types are either BITNET_T158 or F16.
pub fn validate_export(data: &[u8], expected_tensors: usize) -> Result<()> {
    let mut cursor = Cursor::new(data);
    let header = gguf::parse_header(&mut cursor)?;

    if header.magic != GGUF_MAGIC {
        return Err(RuvLLMError::Model("Invalid GGUF magic number".into()));
    }
    if header.version != GGUF_VERSION {
        return Err(RuvLLMError::Model(format!(
            "GGUF version mismatch: expected {}, got {}",
            GGUF_VERSION, header.version
        )));
    }
    if header.tensor_count as usize != expected_tensors {
        return Err(RuvLLMError::Model(format!(
            "Tensor count mismatch: expected {}, got {}",
            expected_tensors, header.tensor_count
        )));
    }

    let metadata = gguf::parse_metadata(&mut cursor, header.metadata_kv_count)?;
    let required_keys = [
        "general.architecture",
        "craftsman.bitnet.version",
        "craftsman.bitnet.weight_encoding",
    ];
    for key in &required_keys {
        if !metadata.contains_key(*key) {
            return Err(RuvLLMError::Model(format!(
                "Missing required metadata key: {}",
                key
            )));
        }
    }

    let tensors = gguf::parse_tensor_infos(&mut cursor, header.tensor_count)?;
    for t in &tensors {
        match t.dtype {
            GgufQuantType::BitnetT158 | GgufQuantType::F16 => {}
            other => {
                return Err(RuvLLMError::Model(format!(
                    "Unexpected tensor type {:?} for {}",
                    other, t.name
                )));
            }
        }
    }

    Ok(())
}

// ============================================================================
// Internal Helpers
// ============================================================================

fn tensor_data_size(tensor: &ExportTensor) -> usize {
    match tensor {
        ExportTensor::Ternary(t) => t.num_blocks() * 66,
        ExportTensor::Fp16 { data, .. } => data.len(),
    }
}

fn tensor_shape_and_type(tensor: &ExportTensor) -> (Vec<usize>, GgufQuantType) {
    match tensor {
        ExportTensor::Ternary(t) => (vec![t.shape.0, t.shape.1], GgufQuantType::BitnetT158),
        ExportTensor::Fp16 { shape, .. } => (shape.clone(), GgufQuantType::F16),
    }
}

fn serialize_export_tensor(tensor: &ExportTensor) -> Vec<u8> {
    match tensor {
        ExportTensor::Ternary(t) => serialize_bitnet_t158(t),
        ExportTensor::Fp16 { data, .. } => data.clone(),
    }
}

#[inline]
fn align_up(offset: u64, alignment: u64) -> u64 {
    (offset + alignment - 1) / alignment * alignment
}

fn io_err(e: io::Error) -> RuvLLMError {
    RuvLLMError::Model(format!("GGUF write error: {}", e))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitnet::{dequantize_bitnet_t158, pack_ternary, quantize_tensor, PtBitnetConfig};

    #[test]
    fn test_f32_to_f16_roundtrip() {
        let cases: &[(f32, f32)] = &[
            (0.0, 0.0),
            (1.0, 1.0),
            (-1.0, -1.0),
            (0.5, 0.5),
            (-0.5, -0.5),
            (65504.0, 65504.0),
            (0.00006103515625, 0.00006103515625), // smallest normal f16
        ];
        for &(input, expected) in cases {
            let bytes = f32_to_f16_bytes(input);
            let bits = u16::from_le_bytes(bytes);
            let back = f16_to_f32(bits);
            assert!(
                (back - expected).abs() < 1e-3,
                "f16 roundtrip failed for {}: got {}",
                input,
                back
            );
        }
    }

    #[test]
    fn test_f32_to_f16_special_cases() {
        // +inf
        let bytes = f32_to_f16_bytes(f32::INFINITY);
        assert_eq!(u16::from_le_bytes(bytes), 0x7C00);
        // -inf
        let bytes = f32_to_f16_bytes(f32::NEG_INFINITY);
        assert_eq!(u16::from_le_bytes(bytes), 0xFC00);
        // NaN: exponent all ones, fraction non-zero
        let bytes = f32_to_f16_bytes(f32::NAN);
        let bits = u16::from_le_bytes(bytes);
        assert_eq!(bits & 0x7C00, 0x7C00);
        assert_ne!(bits & 0x03FF, 0);
        // Overflow → inf
        let bytes = f32_to_f16_bytes(100000.0);
        assert_eq!(u16::from_le_bytes(bytes), 0x7C00);
        // Underflow → zero
        let bytes = f32_to_f16_bytes(1e-40);
        assert_eq!(u16::from_le_bytes(bytes), 0x0000);
    }

    #[test]
    fn test_serialize_bitnet_t158_single_block() {
        let ternary_vals = vec![1i8; 256];
        let packed = pack_ternary(&ternary_vals);
        let tensor = TernaryTensor {
            packed_data: packed,
            scales: vec![0.42],
            shape: (1, 256),
            block_size: 256,
        };

        let serialized = serialize_bitnet_t158(&tensor);
        assert_eq!(serialized.len(), 66);

        // Verify FP16 scale at bytes 64..66
        let scale_bits = u16::from_le_bytes([serialized[64], serialized[65]]);
        let scale_back = f16_to_f32(scale_bits);
        assert!((scale_back - 0.42).abs() < 0.01);
    }

    #[test]
    fn test_write_read_single_tensor() {
        let weights = vec![0.5f32; 256];
        let config = PtBitnetConfig::default();
        let ternary = quantize_tensor(&weights, (1, 256), &config).unwrap();
        let tensor = ExportTensor::Ternary(ternary);

        let metadata = vec![
            (
                "general.architecture",
                MetadataValue::String("craftsman".into()),
            ),
            ("craftsman.bitnet.version", MetadataValue::U32(1)),
            (
                "craftsman.bitnet.weight_encoding",
                MetadataValue::String("absmean_ternary".into()),
            ),
        ];
        let tensors = vec![("test.weight", &tensor)];

        let mut writer = GgufBitnetWriter::new(Cursor::new(Vec::new()));
        writer.write_model(&metadata, &tensors).unwrap();
        let data = writer.into_inner().into_inner();

        validate_export(&data, 1).unwrap();
    }

    #[test]
    fn test_write_read_multi_tensor() {
        let config = PtBitnetConfig::default();
        let ternary = quantize_tensor(&vec![0.3f32; 512], (2, 256), &config).unwrap();
        let t_export = ExportTensor::Ternary(ternary);

        // FP16 tensor: 4 elements
        let fp16_data: Vec<u8> = (0..4)
            .flat_map(|_| f32_to_f16_bytes(1.0).to_vec())
            .collect();
        let f_export = ExportTensor::Fp16 {
            data: fp16_data,
            shape: vec![4],
        };

        let metadata = vec![
            (
                "general.architecture",
                MetadataValue::String("craftsman".into()),
            ),
            ("craftsman.bitnet.version", MetadataValue::U32(1)),
            (
                "craftsman.bitnet.weight_encoding",
                MetadataValue::String("absmean_ternary".into()),
            ),
        ];
        let tensors = vec![("expert.weight", &t_export), ("router.weight", &f_export)];

        let mut writer = GgufBitnetWriter::new(Cursor::new(Vec::new()));
        writer.write_model(&metadata, &tensors).unwrap();
        let data = writer.into_inner().into_inner();

        validate_export(&data, 2).unwrap();
    }

    #[test]
    fn test_metadata_serialization() {
        let metadata = vec![
            ("key.string", MetadataValue::String("hello".into())),
            ("key.u32", MetadataValue::U32(42)),
            ("key.i32", MetadataValue::I32(-1)),
        ];
        let tensor = ExportTensor::Ternary(TernaryTensor {
            packed_data: vec![0u8; 64],
            scales: vec![1.0],
            shape: (1, 256),
            block_size: 256,
        });
        let tensors = vec![("t", &tensor)];

        let mut writer = GgufBitnetWriter::new(Cursor::new(Vec::new()));
        writer.write_model(&metadata, &tensors).unwrap();
        let data = writer.into_inner().into_inner();

        let mut cursor = Cursor::new(&data[..]);
        let header = gguf::parse_header(&mut cursor).unwrap();
        assert_eq!(header.metadata_kv_count, 3);

        let md = gguf::parse_metadata(&mut cursor, 3).unwrap();
        assert_eq!(md.get("key.string").unwrap().as_str(), Some("hello"));
        assert_eq!(md.get("key.u32").unwrap().as_u64(), Some(42));
        assert_eq!(md.get("key.i32").unwrap().as_i64(), Some(-1));
    }

    #[test]
    fn test_alignment_verification() {
        let config = PtBitnetConfig::default();
        let t1 = quantize_tensor(&vec![0.5f32; 256], (1, 256), &config).unwrap();
        let t2 = quantize_tensor(&vec![-0.5f32; 256], (1, 256), &config).unwrap();
        let e1 = ExportTensor::Ternary(t1);
        let e2 = ExportTensor::Ternary(t2);

        let metadata = vec![
            ("general.architecture", MetadataValue::String("test".into())),
            ("craftsman.bitnet.version", MetadataValue::U32(1)),
            (
                "craftsman.bitnet.weight_encoding",
                MetadataValue::String("absmean_ternary".into()),
            ),
        ];
        let tensors = vec![("a.weight", &e1), ("b.weight", &e2)];

        let mut writer = GgufBitnetWriter::new(Cursor::new(Vec::new()));
        writer.write_model(&metadata, &tensors).unwrap();
        let data = writer.into_inner().into_inner();

        let mut cursor = Cursor::new(&data[..]);
        let header = gguf::parse_header(&mut cursor).unwrap();
        let _ = gguf::parse_metadata(&mut cursor, header.metadata_kv_count).unwrap();
        let infos = gguf::parse_tensor_infos(&mut cursor, header.tensor_count).unwrap();

        // Data section starts at 32-byte boundary
        let info_end = cursor.position();
        let data_start = align_up(info_end, DEFAULT_ALIGNMENT as u64);
        assert_eq!(data_start % DEFAULT_ALIGNMENT as u64, 0);

        // Second tensor offset is 32-byte aligned
        assert!(infos.len() == 2);
        assert_eq!(infos[1].offset % DEFAULT_ALIGNMENT as u64, 0);
    }

    #[test]
    fn test_data_integrity_dequantize() {
        let weights = vec![0.5f32; 256];
        let config = PtBitnetConfig::default();
        let ternary = quantize_tensor(&weights, (1, 256), &config).unwrap();
        let original_scales = ternary.scales.clone();
        let original_packed = ternary.packed_data.clone();
        let tensor = ExportTensor::Ternary(ternary);

        let metadata = vec![
            (
                "general.architecture",
                MetadataValue::String("craftsman".into()),
            ),
            ("craftsman.bitnet.version", MetadataValue::U32(1)),
            (
                "craftsman.bitnet.weight_encoding",
                MetadataValue::String("absmean_ternary".into()),
            ),
        ];
        let tensors = vec![("test.weight", &tensor)];

        let mut writer = GgufBitnetWriter::new(Cursor::new(Vec::new()));
        writer.write_model(&metadata, &tensors).unwrap();
        let data = writer.into_inner().into_inner();

        // Parse to find data section offset
        let mut cursor = Cursor::new(&data[..]);
        let header = gguf::parse_header(&mut cursor).unwrap();
        let _ = gguf::parse_metadata(&mut cursor, header.metadata_kv_count).unwrap();
        let _ = gguf::parse_tensor_infos(&mut cursor, header.tensor_count).unwrap();

        let info_end = cursor.position();
        let data_start = align_up(info_end, DEFAULT_ALIGNMENT as u64) as usize;

        // Extract the single 66-byte block from the data section
        let block = &data[data_start..data_start + 66];
        let packed_read = &block[0..64];
        let scale_bits = u16::from_le_bytes([block[64], block[65]]);
        let scale_read = f16_to_f32(scale_bits);

        // Verify packed data matches original
        assert_eq!(packed_read, &original_packed[..]);

        // Verify scale within FP16 precision
        assert!(
            (scale_read - original_scales[0]).abs() < 0.01,
            "Scale mismatch: {} vs {}",
            scale_read,
            original_scales[0]
        );

        // Dequantize both and compare
        let dequant_orig = dequantize_bitnet_t158(&original_packed, &original_scales, 256);
        let dequant_read = dequantize_bitnet_t158(packed_read, &[scale_read], 256);

        for (a, b) in dequant_orig.iter().zip(dequant_read.iter()) {
            assert!((a - b).abs() < 0.01, "Dequantized mismatch: {} vs {}", a, b);
        }
    }
}
