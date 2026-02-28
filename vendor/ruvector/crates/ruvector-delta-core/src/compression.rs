//! Delta compression strategies
//!
//! Provides specialized compression for delta data, leveraging
//! the statistical properties of change data.

use alloc::vec::Vec;

use crate::delta::VectorDelta;
use crate::encoding::{DeltaEncoding, HybridEncoding};
use crate::error::{DeltaError, Result};

/// Compression level settings
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionLevel {
    /// No compression
    None,
    /// Fast compression (lower ratio)
    Fast,
    /// Balanced compression
    Balanced,
    /// Best compression (slower)
    Best,
}

impl Default for CompressionLevel {
    fn default() -> Self {
        Self::Balanced
    }
}

/// Compression codec types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CompressionCodec {
    /// No compression
    None = 0,
    /// LZ4 compression
    Lz4 = 1,
    /// Zstandard compression
    Zstd = 2,
    /// Delta-of-delta encoding
    DeltaOfDelta = 3,
    /// Quantization-based compression
    Quantized = 4,
}

impl TryFrom<u8> for CompressionCodec {
    type Error = DeltaError;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Lz4),
            2 => Ok(Self::Zstd),
            3 => Ok(Self::DeltaOfDelta),
            4 => Ok(Self::Quantized),
            _ => Err(DeltaError::InvalidEncoding(alloc::format!(
                "Unknown codec: {}",
                value
            ))),
        }
    }
}

/// Delta compressor configuration
#[derive(Debug, Clone)]
pub struct CompressorConfig {
    /// Compression codec
    pub codec: CompressionCodec,
    /// Compression level
    pub level: CompressionLevel,
    /// Minimum size to compress (bytes)
    pub min_size: usize,
    /// Enable checksums
    pub enable_checksum: bool,
}

impl Default for CompressorConfig {
    fn default() -> Self {
        Self {
            codec: CompressionCodec::Lz4,
            level: CompressionLevel::Balanced,
            min_size: 64,
            enable_checksum: true,
        }
    }
}

/// Compressed data header
#[derive(Debug, Clone)]
struct CompressedHeader {
    /// Compression codec used
    codec: CompressionCodec,
    /// Original uncompressed size
    original_size: u32,
    /// Compressed size
    compressed_size: u32,
    /// Optional checksum (FNV-1a)
    checksum: Option<u64>,
}

impl CompressedHeader {
    const MAGIC: u32 = 0x44454C54; // "DELT"
    const VERSION: u8 = 1;

    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(21);

        // Magic (4 bytes)
        bytes.extend_from_slice(&Self::MAGIC.to_le_bytes());

        // Version (1 byte)
        bytes.push(Self::VERSION);

        // Codec (1 byte)
        bytes.push(self.codec as u8);

        // Has checksum flag (1 byte)
        bytes.push(if self.checksum.is_some() { 1 } else { 0 });

        // Original size (4 bytes)
        bytes.extend_from_slice(&self.original_size.to_le_bytes());

        // Compressed size (4 bytes)
        bytes.extend_from_slice(&self.compressed_size.to_le_bytes());

        // Checksum (8 bytes if present)
        if let Some(cs) = self.checksum {
            bytes.extend_from_slice(&cs.to_le_bytes());
        }

        bytes
    }

    fn from_bytes(bytes: &[u8]) -> Result<(Self, usize)> {
        if bytes.len() < 15 {
            return Err(DeltaError::DecompressionError("Header too small".into()));
        }

        let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if magic != Self::MAGIC {
            return Err(DeltaError::DecompressionError(
                "Invalid magic number".into(),
            ));
        }

        let version = bytes[4];
        if version != Self::VERSION {
            return Err(DeltaError::VersionMismatch {
                expected: Self::VERSION as u32,
                actual: version as u32,
            });
        }

        let codec = CompressionCodec::try_from(bytes[5])?;
        let has_checksum = bytes[6] != 0;

        let original_size = u32::from_le_bytes([bytes[7], bytes[8], bytes[9], bytes[10]]);
        let compressed_size = u32::from_le_bytes([bytes[11], bytes[12], bytes[13], bytes[14]]);

        let (checksum, header_size) = if has_checksum {
            if bytes.len() < 23 {
                return Err(DeltaError::DecompressionError(
                    "Header too small for checksum".into(),
                ));
            }
            let cs = u64::from_le_bytes([
                bytes[15], bytes[16], bytes[17], bytes[18], bytes[19], bytes[20], bytes[21],
                bytes[22],
            ]);
            (Some(cs), 23)
        } else {
            (None, 15)
        };

        Ok((
            Self {
                codec,
                original_size,
                compressed_size,
                checksum,
            },
            header_size,
        ))
    }
}

/// Delta compressor for efficient storage
pub struct DeltaCompressor {
    config: CompressorConfig,
    encoding: HybridEncoding,
}

impl DeltaCompressor {
    /// Create a new compressor with default configuration
    pub fn new() -> Self {
        Self {
            config: CompressorConfig::default(),
            encoding: HybridEncoding::default(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: CompressorConfig) -> Self {
        Self {
            config,
            encoding: HybridEncoding::default(),
        }
    }

    /// Compress a delta
    pub fn compress(&self, delta: &VectorDelta) -> Result<Vec<u8>> {
        // First encode the delta
        let encoded = self.encoding.encode(delta)?;

        // Check if compression is worthwhile
        if encoded.len() < self.config.min_size || self.config.codec == CompressionCodec::None {
            // Return uncompressed with header
            let header = CompressedHeader {
                codec: CompressionCodec::None,
                original_size: encoded.len() as u32,
                compressed_size: encoded.len() as u32,
                checksum: if self.config.enable_checksum {
                    Some(fnv1a_hash(&encoded))
                } else {
                    None
                },
            };

            let mut result = header.to_bytes();
            result.extend_from_slice(&encoded);
            return Ok(result);
        }

        // Compress based on codec
        let compressed = match self.config.codec {
            CompressionCodec::None => encoded.clone(),
            #[cfg(feature = "compression")]
            CompressionCodec::Lz4 => self.compress_lz4(&encoded)?,
            #[cfg(feature = "compression")]
            CompressionCodec::Zstd => self.compress_zstd(&encoded)?,
            CompressionCodec::DeltaOfDelta => self.compress_delta_of_delta(&encoded)?,
            CompressionCodec::Quantized => self.compress_quantized(&encoded)?,
            #[cfg(not(feature = "compression"))]
            CompressionCodec::Lz4 | CompressionCodec::Zstd => {
                return Err(DeltaError::CompressionError(
                    "Compression feature not enabled".into(),
                ));
            }
        };

        // Build result
        let header = CompressedHeader {
            codec: self.config.codec,
            original_size: encoded.len() as u32,
            compressed_size: compressed.len() as u32,
            checksum: if self.config.enable_checksum {
                Some(fnv1a_hash(&encoded))
            } else {
                None
            },
        };

        let mut result = header.to_bytes();
        result.extend_from_slice(&compressed);
        Ok(result)
    }

    /// Decompress bytes to a delta
    pub fn decompress(&self, bytes: &[u8]) -> Result<VectorDelta> {
        let (header, header_size) = CompressedHeader::from_bytes(bytes)?;

        let compressed_data = &bytes[header_size..];
        if compressed_data.len() < header.compressed_size as usize {
            return Err(DeltaError::DecompressionError(alloc::format!(
                "Insufficient data: expected {}, got {}",
                header.compressed_size,
                compressed_data.len()
            )));
        }

        let compressed = &compressed_data[..header.compressed_size as usize];

        // Decompress based on codec
        let decompressed = match header.codec {
            CompressionCodec::None => compressed.to_vec(),
            #[cfg(feature = "compression")]
            CompressionCodec::Lz4 => {
                self.decompress_lz4(compressed, header.original_size as usize)?
            }
            #[cfg(feature = "compression")]
            CompressionCodec::Zstd => self.decompress_zstd(compressed)?,
            CompressionCodec::DeltaOfDelta => {
                self.decompress_delta_of_delta(compressed, header.original_size as usize)?
            }
            CompressionCodec::Quantized => {
                self.decompress_quantized(compressed, header.original_size as usize)?
            }
            #[cfg(not(feature = "compression"))]
            CompressionCodec::Lz4 | CompressionCodec::Zstd => {
                return Err(DeltaError::DecompressionError(
                    "Compression feature not enabled".into(),
                ));
            }
        };

        // Verify checksum
        if let Some(expected_checksum) = header.checksum {
            let actual_checksum = fnv1a_hash(&decompressed);
            if expected_checksum != actual_checksum {
                return Err(DeltaError::ChecksumMismatch {
                    expected: expected_checksum,
                    actual: actual_checksum,
                });
            }
        }

        // Decode
        self.encoding.decode(&decompressed)
    }

    /// Get compression ratio for a compressed buffer
    pub fn compression_ratio(&self, compressed: &[u8]) -> Result<f64> {
        let (header, _) = CompressedHeader::from_bytes(compressed)?;

        if header.compressed_size == 0 {
            return Ok(1.0);
        }

        Ok(header.original_size as f64 / header.compressed_size as f64)
    }

    #[cfg(feature = "compression")]
    fn compress_lz4(&self, data: &[u8]) -> Result<Vec<u8>> {
        lz4_flex::compress_prepend_size(data)
            .map_err(|e| DeltaError::CompressionError(alloc::format!("LZ4 error: {}", e)))
    }

    #[cfg(feature = "compression")]
    fn decompress_lz4(&self, data: &[u8], _original_size: usize) -> Result<Vec<u8>> {
        lz4_flex::decompress_size_prepended(data)
            .map_err(|e| DeltaError::DecompressionError(alloc::format!("LZ4 error: {}", e)))
    }

    #[cfg(feature = "compression")]
    fn compress_zstd(&self, data: &[u8]) -> Result<Vec<u8>> {
        let level = match self.config.level {
            CompressionLevel::None => 0,
            CompressionLevel::Fast => 1,
            CompressionLevel::Balanced => 3,
            CompressionLevel::Best => 19,
        };

        zstd::encode_all(data, level)
            .map_err(|e| DeltaError::CompressionError(alloc::format!("Zstd error: {}", e)))
    }

    #[cfg(feature = "compression")]
    fn decompress_zstd(&self, data: &[u8]) -> Result<Vec<u8>> {
        zstd::decode_all(data)
            .map_err(|e| DeltaError::DecompressionError(alloc::format!("Zstd error: {}", e)))
    }

    /// Delta-of-delta encoding for sequential data
    fn compress_delta_of_delta(&self, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() < 4 {
            return Ok(data.to_vec());
        }

        // Treat as f32 array and compute delta-of-delta
        let float_count = data.len() / 4;
        let mut result = Vec::with_capacity(data.len());

        // First value stored as-is
        result.extend_from_slice(&data[..4]);

        if float_count < 2 {
            return Ok(result);
        }

        // Second value: store delta
        let v0 = f32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let v1 = f32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let delta0 = v1 - v0;
        result.extend_from_slice(&delta0.to_le_bytes());

        // Remaining: store delta-of-delta
        let mut prev_delta = delta0;
        for i in 2..float_count {
            let offset = i * 4;
            let curr = f32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let prev_offset = (i - 1) * 4;
            let prev = f32::from_le_bytes([
                data[prev_offset],
                data[prev_offset + 1],
                data[prev_offset + 2],
                data[prev_offset + 3],
            ]);

            let curr_delta = curr - prev;
            let delta_of_delta = curr_delta - prev_delta;

            result.extend_from_slice(&delta_of_delta.to_le_bytes());
            prev_delta = curr_delta;
        }

        Ok(result)
    }

    fn decompress_delta_of_delta(&self, data: &[u8], original_size: usize) -> Result<Vec<u8>> {
        if data.len() < 4 {
            return Ok(data.to_vec());
        }

        let float_count = original_size / 4;
        let mut result = Vec::with_capacity(original_size);

        // First value
        result.extend_from_slice(&data[..4]);
        let mut prev = f32::from_le_bytes([data[0], data[1], data[2], data[3]]);

        if float_count < 2 || data.len() < 8 {
            return Ok(result);
        }

        // Second value from delta
        let delta0 = f32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let v1 = prev + delta0;
        result.extend_from_slice(&v1.to_le_bytes());

        // Remaining from delta-of-delta
        let mut prev_delta = delta0;
        prev = v1;

        for i in 2..float_count {
            let offset = i * 4;
            if offset + 4 > data.len() {
                break;
            }

            let dod = f32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);

            let curr_delta = prev_delta + dod;
            let curr = prev + curr_delta;

            result.extend_from_slice(&curr.to_le_bytes());

            prev_delta = curr_delta;
            prev = curr;
        }

        Ok(result)
    }

    /// Quantization-based compression (reduce f32 to f16)
    fn compress_quantized(&self, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() < 4 {
            return Ok(data.to_vec());
        }

        let float_count = data.len() / 4;
        let mut result = Vec::with_capacity(float_count * 2);

        for i in 0..float_count {
            let offset = i * 4;
            let value = f32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);

            // Convert to f16 representation (simplified)
            let f16_bits = f32_to_f16_bits(value);
            result.extend_from_slice(&f16_bits.to_le_bytes());
        }

        Ok(result)
    }

    fn decompress_quantized(&self, data: &[u8], original_size: usize) -> Result<Vec<u8>> {
        let float_count = original_size / 4;
        let mut result = Vec::with_capacity(original_size);

        for i in 0..float_count {
            let offset = i * 2;
            if offset + 2 > data.len() {
                break;
            }

            let f16_bits = u16::from_le_bytes([data[offset], data[offset + 1]]);
            let value = f16_bits_to_f32(f16_bits);

            result.extend_from_slice(&value.to_le_bytes());
        }

        Ok(result)
    }
}

impl Default for DeltaCompressor {
    fn default() -> Self {
        Self::new()
    }
}

/// FNV-1a hash for checksums
fn fnv1a_hash(data: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Convert f32 to f16 bit representation
fn f32_to_f16_bits(value: f32) -> u16 {
    let bits = value.to_bits();

    let sign = (bits >> 31) as u16;
    let exp = ((bits >> 23) & 0xff) as i32;
    let frac = bits & 0x7fffff;

    if exp == 0xff {
        // Inf or NaN
        return (sign << 15) | 0x7c00 | ((frac != 0) as u16);
    }

    let new_exp = exp - 127 + 15;

    if new_exp <= 0 {
        // Subnormal or zero
        0
    } else if new_exp >= 31 {
        // Overflow to infinity
        (sign << 15) | 0x7c00
    } else {
        let new_frac = (frac >> 13) as u16;
        (sign << 15) | ((new_exp as u16) << 10) | new_frac
    }
}

/// Convert f16 bits to f32
fn f16_bits_to_f32(bits: u16) -> f32 {
    let sign = ((bits >> 15) as u32) << 31;
    let exp = ((bits >> 10) & 0x1f) as i32;
    let frac = (bits & 0x3ff) as u32;

    if exp == 0 {
        // Zero or subnormal
        if frac == 0 {
            f32::from_bits(sign)
        } else {
            // Subnormal f16 -> normalized f32
            let shift = frac.leading_zeros() - 21;
            let new_exp = (127 - 15 - shift as i32) as u32;
            let new_frac = (frac << (shift + 1)) & 0x7fffff;
            f32::from_bits(sign | (new_exp << 23) | new_frac)
        }
    } else if exp == 31 {
        // Inf or NaN
        if frac == 0 {
            f32::from_bits(sign | 0x7f800000)
        } else {
            f32::from_bits(sign | 0x7fc00000)
        }
    } else {
        let new_exp = ((exp - 15 + 127) as u32) << 23;
        let new_frac = frac << 13;
        f32::from_bits(sign | new_exp | new_frac)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compressor_roundtrip_none() {
        let config = CompressorConfig {
            codec: CompressionCodec::None,
            ..Default::default()
        };

        let compressor = DeltaCompressor::with_config(config);
        let delta = VectorDelta::from_dense(vec![1.0, 2.0, 3.0, 4.0]);

        let compressed = compressor.compress(&delta).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();

        assert_eq!(delta.dimensions, decompressed.dimensions);
    }

    #[test]
    fn test_compressor_delta_of_delta() {
        let config = CompressorConfig {
            codec: CompressionCodec::DeltaOfDelta,
            ..Default::default()
        };

        let compressor = DeltaCompressor::with_config(config);

        // Sequential data works well with delta-of-delta
        let delta = VectorDelta::from_dense(vec![1.0, 2.0, 3.0, 4.0, 5.0]);

        let compressed = compressor.compress(&delta).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();

        assert_eq!(delta.dimensions, decompressed.dimensions);
    }

    #[test]
    fn test_compressor_quantized() {
        let config = CompressorConfig {
            codec: CompressionCodec::Quantized,
            ..Default::default()
        };

        let compressor = DeltaCompressor::with_config(config);
        let delta = VectorDelta::from_dense(vec![1.0, 2.0, 3.0, 4.0]);

        let compressed = compressor.compress(&delta).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();

        // Quantization loses precision, so just check dimensions
        assert_eq!(delta.dimensions, decompressed.dimensions);
    }

    #[test]
    fn test_checksum_verification() {
        let compressor = DeltaCompressor::new();
        let delta = VectorDelta::from_dense(vec![1.0, 2.0, 3.0]);

        let mut compressed = compressor.compress(&delta).unwrap();

        // Corrupt data
        if compressed.len() > 30 {
            compressed[30] ^= 0xff;
        }

        let result = compressor.decompress(&compressed);
        assert!(result.is_err());
    }

    #[test]
    fn test_f16_conversion() {
        let values = [0.0f32, 1.0, -1.0, 0.5, 2.5, 1000.0, -0.001];

        for &original in &values {
            let bits = f32_to_f16_bits(original);
            let recovered = f16_bits_to_f32(bits);

            // f16 has limited precision
            if original != 0.0 {
                let relative_error = ((recovered - original) / original).abs();
                assert!(
                    relative_error < 0.01,
                    "Failed for {}: got {}, error {}",
                    original,
                    recovered,
                    relative_error
                );
            }
        }
    }
}
