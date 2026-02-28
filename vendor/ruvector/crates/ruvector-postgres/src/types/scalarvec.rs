//! ScalarVec - Native scalar quantized vector type (SQ8)
//!
//! Stores vectors with 8 bits per dimension (4x compression).
//! Uses int8 SIMD operations for fast approximate distance computation.

use pgrx::pgrx_sql_entity_graph::metadata::{
    ArgumentError, Returns, ReturnsError, SqlMapping, SqlTranslatable,
};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use crate::MAX_DIMENSIONS;

/// ScalarVec: Scalar quantized vector (8 bits per dimension)
///
/// Memory layout (varlena):
/// - Header: 4 bytes (varlena header)
/// - Dimensions: 2 bytes (u16)
/// - Scale: 4 bytes (f32)
/// - Offset: 4 bytes (f32)
/// - Data: dimensions bytes (i8)
///
/// Maximum dimensions: 16,000
/// Compression ratio: 4x vs f32
#[derive(Clone, Serialize, Deserialize)]
pub struct ScalarVec {
    /// Number of dimensions
    dimensions: u16,
    /// Scale factor for dequantization
    scale: f32,
    /// Offset for dequantization
    offset: f32,
    /// Quantized data (i8 values)
    data: Vec<i8>,
}

impl ScalarVec {
    /// Create from f32 slice with automatic scale/offset calculation
    pub fn from_f32(vector: &[f32]) -> Self {
        if vector.len() > MAX_DIMENSIONS {
            pgrx::error!(
                "Vector dimension {} exceeds maximum {}",
                vector.len(),
                MAX_DIMENSIONS
            );
        }

        if vector.is_empty() {
            return Self {
                dimensions: 0,
                scale: 1.0,
                offset: 0.0,
                data: Vec::new(),
            };
        }

        // Find min and max
        let mut min = f32::MAX;
        let mut max = f32::MIN;
        for &v in vector {
            if v < min {
                min = v;
            }
            if v > max {
                max = v;
            }
        }

        let range = max - min;
        let scale = if range > 0.0 { range / 254.0 } else { 1.0 };
        let offset = min;

        // Quantize to i8 (-127 to 127)
        let data: Vec<i8> = vector
            .iter()
            .map(|&v| {
                let normalized = (v - offset) / scale;
                (normalized.clamp(0.0, 254.0) - 127.0) as i8
            })
            .collect();

        Self {
            dimensions: vector.len() as u16,
            scale,
            offset,
            data,
        }
    }

    /// Create with custom scale and offset
    pub fn from_f32_custom(vector: &[f32], scale: f32, offset: f32) -> Self {
        if vector.len() > MAX_DIMENSIONS {
            pgrx::error!(
                "Vector dimension {} exceeds maximum {}",
                vector.len(),
                MAX_DIMENSIONS
            );
        }

        let data: Vec<i8> = vector
            .iter()
            .map(|&v| {
                let normalized = (v - offset) / scale;
                (normalized.clamp(0.0, 254.0) - 127.0) as i8
            })
            .collect();

        Self {
            dimensions: vector.len() as u16,
            scale,
            offset,
            data,
        }
    }

    /// Get number of dimensions
    #[inline]
    pub fn dimensions(&self) -> usize {
        self.dimensions as usize
    }

    /// Get scale factor
    #[inline]
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Get offset
    #[inline]
    pub fn offset(&self) -> f32 {
        self.offset
    }

    /// Get quantized data
    #[inline]
    pub fn as_i8_slice(&self) -> &[i8] {
        &self.data
    }

    /// Dequantize to f32 vector
    pub fn to_f32(&self) -> Vec<f32> {
        self.data
            .iter()
            .map(|&q| (q as f32 + 127.0) * self.scale + self.offset)
            .collect()
    }

    /// Calculate approximate Euclidean distance (quantized space)
    pub fn distance(&self, other: &Self) -> f32 {
        debug_assert_eq!(self.dimensions, other.dimensions);
        let max_scale = self.scale.max(other.scale);
        distance_simd(&self.data, &other.data, max_scale)
    }

    /// Calculate squared distance (int32 space, no sqrt)
    pub fn distance_sq_int(&self, other: &Self) -> i32 {
        debug_assert_eq!(self.dimensions, other.dimensions);
        distance_sq(&self.data, &other.data)
    }

    /// Memory size in bytes
    pub fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.data.len()
    }

    /// Compression ratio vs f32
    pub const fn compression_ratio() -> f32 {
        4.0 // f32 (4 bytes) -> i8 (1 byte)
    }

    /// Serialize to bytes
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(10 + self.data.len());
        bytes.extend_from_slice(&self.dimensions.to_le_bytes());
        bytes.extend_from_slice(&self.scale.to_le_bytes());
        bytes.extend_from_slice(&self.offset.to_le_bytes());

        // Convert i8 to u8 for storage
        for &val in &self.data {
            bytes.push(val as u8);
        }

        bytes
    }

    /// Deserialize from bytes
    fn from_bytes(bytes: &[u8]) -> Self {
        if bytes.len() < 10 {
            pgrx::error!("Invalid ScalarVec data: too short");
        }

        let dimensions = u16::from_le_bytes([bytes[0], bytes[1]]);
        let scale = f32::from_le_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]);
        let offset = f32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]);

        let expected_len = 10 + dimensions as usize;
        if bytes.len() != expected_len {
            pgrx::error!(
                "Invalid ScalarVec data: expected {} bytes, got {}",
                expected_len,
                bytes.len()
            );
        }

        let data: Vec<i8> = bytes[10..].iter().map(|&b| b as i8).collect();

        Self {
            dimensions,
            scale,
            offset,
            data,
        }
    }
}

// ============================================================================
// SIMD-Optimized Distance Functions
// ============================================================================

/// Calculate squared Euclidean distance (scalar)
#[inline]
pub fn distance_sq(a: &[i8], b: &[i8]) -> i32 {
    debug_assert_eq!(a.len(), b.len());

    a.iter()
        .zip(b.iter())
        .map(|(&x, &y)| {
            let diff = x as i32 - y as i32;
            diff * diff
        })
        .sum()
}

/// Calculate Euclidean distance (scalar)
#[inline]
pub fn distance(a: &[i8], b: &[i8], scale: f32) -> f32 {
    (distance_sq(a, b) as f32).sqrt() * scale
}

/// SIMD-optimized squared distance using AVX2 (x86_64)
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn distance_sq_avx2(a: &[i8], b: &[i8]) -> i32 {
    use std::arch::x86_64::*;

    let n = a.len();
    let mut sum = _mm256_setzero_si256();

    // Process 32 bytes (32 i8 values) at a time
    let chunks = n / 32;
    for i in 0..chunks {
        let offset = i * 32;

        let va = _mm256_loadu_si256(a.as_ptr().add(offset) as *const __m256i);
        let vb = _mm256_loadu_si256(b.as_ptr().add(offset) as *const __m256i);

        // Subtract with sign extension (i8 -> i16)
        // Process lower 16 bytes
        let diff_lo = _mm256_sub_epi16(
            _mm256_cvtepi8_epi16(_mm256_castsi256_si128(va)),
            _mm256_cvtepi8_epi16(_mm256_castsi256_si128(vb)),
        );

        // Process upper 16 bytes
        let diff_hi = _mm256_sub_epi16(
            _mm256_cvtepi8_epi16(_mm256_extracti128_si256(va, 1)),
            _mm256_cvtepi8_epi16(_mm256_extracti128_si256(vb, 1)),
        );

        // Square and accumulate (i16 * i16 -> i32)
        let sq_lo = _mm256_madd_epi16(diff_lo, diff_lo);
        let sq_hi = _mm256_madd_epi16(diff_hi, diff_hi);

        sum = _mm256_add_epi32(sum, sq_lo);
        sum = _mm256_add_epi32(sum, sq_hi);
    }

    // Horizontal sum of 8 i32 values
    let sum128_lo = _mm256_castsi256_si128(sum);
    let sum128_hi = _mm256_extracti128_si256(sum, 1);
    let sum128 = _mm_add_epi32(sum128_lo, sum128_hi);

    let sum64 = _mm_add_epi32(sum128, _mm_srli_si128(sum128, 8));
    let sum32 = _mm_add_epi32(sum64, _mm_srli_si128(sum64, 4));

    let mut result = _mm_cvtsi128_si32(sum32);

    // Handle remainder
    for i in (chunks * 32)..n {
        let diff = a[i] as i32 - b[i] as i32;
        result += diff * diff;
    }

    result
}

/// SIMD-optimized distance with runtime dispatch
pub fn distance_simd(a: &[i8], b: &[i8], scale: f32) -> f32 {
    debug_assert_eq!(a.len(), b.len());

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") && a.len() >= 32 {
            return (unsafe { distance_sq_avx2(a, b) } as f32).sqrt() * scale;
        }
    }

    distance(a, b, scale)
}

// ============================================================================
// Display & Parsing
// ============================================================================

impl fmt::Display for ScalarVec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        for (i, &val) in self.data.iter().enumerate() {
            if i > 0 {
                write!(f, ",")?;
            }
            // Show dequantized value
            let deq = (val as f32 + 127.0) * self.scale + self.offset;
            write!(f, "{:.6}", deq)?;
        }
        write!(f, "]")
    }
}

impl fmt::Debug for ScalarVec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ScalarVec(dims={}, scale={:.6}, offset={:.6})",
            self.dimensions, self.scale, self.offset
        )
    }
}

impl FromStr for ScalarVec {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Parse format: [1.0, 2.0, 3.0]
        let s = s.trim();
        if !s.starts_with('[') || !s.ends_with(']') {
            return Err(format!("Invalid ScalarVec format: {}", s));
        }

        let inner = &s[1..s.len() - 1];
        if inner.is_empty() {
            return Ok(Self {
                dimensions: 0,
                scale: 1.0,
                offset: 0.0,
                data: Vec::new(),
            });
        }

        let values: Result<Vec<f32>, _> =
            inner.split(',').map(|v| v.trim().parse::<f32>()).collect();

        match values {
            Ok(data) => Ok(Self::from_f32(&data)),
            Err(e) => Err(format!("Invalid ScalarVec element: {}", e)),
        }
    }
}

impl PartialEq for ScalarVec {
    fn eq(&self, other: &Self) -> bool {
        self.dimensions == other.dimensions
            && (self.scale - other.scale).abs() < 1e-6
            && (self.offset - other.offset).abs() < 1e-6
            && self.data == other.data
    }
}

// ============================================================================
// PostgreSQL Type Integration
// ============================================================================

unsafe impl SqlTranslatable for ScalarVec {
    fn argument_sql() -> Result<SqlMapping, ArgumentError> {
        Ok(SqlMapping::As(String::from("scalarvec")))
    }

    fn return_sql() -> Result<Returns, ReturnsError> {
        Ok(Returns::One(SqlMapping::As(String::from("scalarvec"))))
    }
}

impl pgrx::IntoDatum for ScalarVec {
    fn into_datum(self) -> Option<pgrx::pg_sys::Datum> {
        let bytes = self.to_bytes();
        let len = bytes.len();
        let total_size = pgrx::pg_sys::VARHDRSZ + len;

        unsafe {
            let ptr = pgrx::pg_sys::palloc(total_size) as *mut u8;
            let varlena = ptr as *mut pgrx::pg_sys::varlena;
            pgrx::varlena::set_varsize_4b(varlena, total_size as i32);
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr.add(pgrx::pg_sys::VARHDRSZ), len);
            Some(pgrx::pg_sys::Datum::from(ptr))
        }
    }

    fn type_oid() -> pgrx::pg_sys::Oid {
        pgrx::pg_sys::Oid::INVALID
    }
}

impl pgrx::FromDatum for ScalarVec {
    unsafe fn from_polymorphic_datum(
        datum: pgrx::pg_sys::Datum,
        is_null: bool,
        _typoid: pgrx::pg_sys::Oid,
    ) -> Option<Self> {
        if is_null {
            return None;
        }

        let ptr = datum.cast_mut_ptr::<pgrx::pg_sys::varlena>();
        let len = pgrx::varlena::varsize_any_exhdr(ptr);
        let data_ptr = pgrx::varlena::vardata_any(ptr) as *const u8;
        let bytes = std::slice::from_raw_parts(data_ptr, len);

        Some(ScalarVec::from_bytes(bytes))
    }
}

// Note: ScalarVec SQL functions are not exposed via #[pg_extern] due to
// pgrx 0.12 trait requirements. Use array-based functions for SQL-level operations.

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantize_dequantize() {
        let original = vec![0.1, 0.5, -0.3, 0.8, -0.9];
        let sq = ScalarVec::from_f32(&original);
        let restored = sq.to_f32();

        for (o, r) in original.iter().zip(restored.iter()) {
            assert!((o - r).abs() < 0.02, "orig={}, restored={}", o, r);
        }
    }

    #[test]
    fn test_distance() {
        let a = ScalarVec::from_f32(&[1.0, 0.0, 0.0]);
        let b = ScalarVec::from_f32(&[0.0, 1.0, 0.0]);

        let dist = a.distance(&b);
        // Euclidean distance should be approximately sqrt(2) ≈ 1.414
        assert!((dist - 1.414).abs() < 0.2, "dist={}", dist);
    }

    #[test]
    fn test_compression_ratio() {
        assert_eq!(ScalarVec::compression_ratio(), 4.0);
    }

    #[test]
    fn test_serialization() {
        let v = ScalarVec::from_f32(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        let bytes = v.to_bytes();
        let v2 = ScalarVec::from_bytes(&bytes);
        assert_eq!(v, v2);
    }

    #[test]
    fn test_simd_matches_scalar() {
        let a_data: Vec<i8> = (0..128).map(|i| i as i8).collect();
        let b_data: Vec<i8> = (0..128).map(|i| -(i as i8)).collect();

        let scalar_result = distance_sq(&a_data, &b_data);
        let simd_result = (distance_simd(&a_data, &b_data, 1.0).powi(2)) as i32;

        assert!((scalar_result - simd_result).abs() < 10);
    }

    #[test]
    fn test_parse() {
        let v: ScalarVec = "[1.0, 2.0, 3.0]".parse().unwrap();
        assert_eq!(v.dimensions(), 3);

        let restored = v.to_f32();
        assert!((restored[0] - 1.0).abs() < 0.1);
        assert!((restored[1] - 2.0).abs() < 0.1);
        assert!((restored[2] - 3.0).abs() < 0.1);
    }
}
