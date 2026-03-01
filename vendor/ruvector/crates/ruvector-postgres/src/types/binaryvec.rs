//! BinaryVec - Native binary quantized vector type
//!
//! Stores vectors with 1 bit per dimension (32x compression).
//! Uses Hamming distance with SIMD popcount acceleration.

use pgrx::pgrx_sql_entity_graph::metadata::{
    ArgumentError, Returns, ReturnsError, SqlMapping, SqlTranslatable,
};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use crate::MAX_DIMENSIONS;

/// BinaryVec: Binary quantized vector (1 bit per dimension)
///
/// Memory layout (varlena):
/// - Header: 4 bytes (varlena header)
/// - Dimensions: 2 bytes (u16)
/// - Data: ceil(dimensions / 8) bytes (bit-packed)
///
/// Maximum dimensions: 16,000
/// Compression ratio: 32x vs f32
#[derive(Clone, Serialize, Deserialize)]
pub struct BinaryVec {
    /// Number of dimensions
    dimensions: u16,
    /// Bit-packed data (8 bits per byte)
    data: Vec<u8>,
}

impl BinaryVec {
    /// Create from f32 slice using threshold 0.0
    pub fn from_f32(vector: &[f32]) -> Self {
        Self::from_f32_threshold(vector, 0.0)
    }

    /// Create from f32 slice with custom threshold
    pub fn from_f32_threshold(vector: &[f32], threshold: f32) -> Self {
        if vector.len() > MAX_DIMENSIONS {
            pgrx::error!(
                "Vector dimension {} exceeds maximum {}",
                vector.len(),
                MAX_DIMENSIONS
            );
        }

        let dimensions = vector.len() as u16;
        let n_bytes = (vector.len() + 7) / 8;
        let mut data = vec![0u8; n_bytes];

        for (i, &val) in vector.iter().enumerate() {
            if val > threshold {
                let byte_idx = i / 8;
                let bit_idx = i % 8;
                data[byte_idx] |= 1u8 << bit_idx;
            }
        }

        Self { dimensions, data }
    }

    /// Get number of dimensions
    #[inline]
    pub fn dimensions(&self) -> usize {
        self.dimensions as usize
    }

    /// Get bit at position
    #[inline]
    pub fn get_bit(&self, pos: usize) -> bool {
        debug_assert!(pos < self.dimensions as usize);
        let byte_idx = pos / 8;
        let bit_idx = pos % 8;
        (self.data[byte_idx] >> bit_idx) & 1 == 1
    }

    /// Set bit at position
    #[inline]
    pub fn set_bit(&mut self, pos: usize, value: bool) {
        debug_assert!(pos < self.dimensions as usize);
        let byte_idx = pos / 8;
        let bit_idx = pos % 8;
        if value {
            self.data[byte_idx] |= 1u8 << bit_idx;
        } else {
            self.data[byte_idx] &= !(1u8 << bit_idx);
        }
    }

    /// Count number of 1 bits (population count)
    pub fn popcount(&self) -> u32 {
        self.data.iter().map(|&b| b.count_ones()).sum()
    }

    /// Calculate Hamming distance to another binary vector
    pub fn hamming_distance(&self, other: &Self) -> u32 {
        debug_assert_eq!(self.dimensions, other.dimensions);
        hamming_distance_simd(&self.data, &other.data)
    }

    /// Calculate normalized Hamming distance [0, 1]
    pub fn normalized_distance(&self, other: &Self) -> f32 {
        self.hamming_distance(other) as f32 / self.dimensions as f32
    }

    /// Memory size in bytes
    pub fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.data.len()
    }

    /// Compression ratio vs f32
    pub const fn compression_ratio() -> f32 {
        32.0 // f32 (32 bits) -> 1 bit
    }

    /// Serialize to bytes (dimensions + bit data)
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(2 + self.data.len());
        bytes.extend_from_slice(&self.dimensions.to_le_bytes());
        bytes.extend_from_slice(&self.data);
        bytes
    }

    /// Deserialize from bytes
    fn from_bytes(bytes: &[u8]) -> Self {
        if bytes.len() < 2 {
            pgrx::error!("Invalid BinaryVec data: too short");
        }

        let dimensions = u16::from_le_bytes([bytes[0], bytes[1]]);
        let expected_len = 2 + ((dimensions as usize + 7) / 8);

        if bytes.len() != expected_len {
            pgrx::error!(
                "Invalid BinaryVec data: expected {} bytes, got {}",
                expected_len,
                bytes.len()
            );
        }

        let data = bytes[2..].to_vec();
        Self { dimensions, data }
    }

    /// Convert to approximate f32 vector (0.0 or 1.0)
    pub fn to_f32(&self) -> Vec<f32> {
        let mut result = Vec::with_capacity(self.dimensions as usize);
        for i in 0..self.dimensions as usize {
            result.push(if self.get_bit(i) { 1.0 } else { 0.0 });
        }
        result
    }

    /// Get raw data
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }
}

// ============================================================================
// SIMD-Optimized Hamming Distance
// ============================================================================

/// Calculate Hamming distance (scalar fallback)
#[inline]
pub fn hamming_distance(a: &[u8], b: &[u8]) -> u32 {
    debug_assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b.iter())
        .map(|(&x, &y)| (x ^ y).count_ones())
        .sum()
}

/// SIMD-optimized Hamming distance using POPCNT (x86_64)
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "popcnt")]
unsafe fn hamming_distance_popcnt(a: &[u8], b: &[u8]) -> u32 {
    use std::arch::x86_64::*;

    let n = a.len();
    let mut count = 0u32;

    // Process 8 bytes (64 bits) at a time
    let chunks = n / 8;
    for i in 0..chunks {
        let offset = i * 8;
        let va = *(a.as_ptr().add(offset) as *const u64);
        let vb = *(b.as_ptr().add(offset) as *const u64);
        count += _popcnt64((va ^ vb) as i64) as u32;
    }

    // Handle remainder
    for i in (chunks * 8)..n {
        count += (a[i] ^ b[i]).count_ones();
    }

    count
}

/// SIMD-optimized Hamming distance using AVX2 (x86_64)
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn hamming_distance_avx2(a: &[u8], b: &[u8]) -> u32 {
    use std::arch::x86_64::*;

    let n = a.len();
    let mut count = 0u32;

    // Process 32 bytes at a time
    let chunks = n / 32;
    for i in 0..chunks {
        let offset = i * 32;

        let va = _mm256_loadu_si256(a.as_ptr().add(offset) as *const __m256i);
        let vb = _mm256_loadu_si256(b.as_ptr().add(offset) as *const __m256i);
        let xor = _mm256_xor_si256(va, vb);

        // Use lookup table for popcount (AVX2 doesn't have native popcount)
        let low_mask = _mm256_set1_epi8(0x0f);
        let pop_cnt_lut = _mm256_setr_epi8(
            0, 1, 1, 2, 1, 2, 2, 3, 1, 2, 2, 3, 2, 3, 3, 4, 0, 1, 1, 2, 1, 2, 2, 3, 1, 2, 2, 3, 2,
            3, 3, 4,
        );

        let lo = _mm256_and_si256(xor, low_mask);
        let hi = _mm256_and_si256(_mm256_srli_epi16(xor, 4), low_mask);

        let cnt_lo = _mm256_shuffle_epi8(pop_cnt_lut, lo);
        let cnt_hi = _mm256_shuffle_epi8(pop_cnt_lut, hi);
        let cnt = _mm256_add_epi8(cnt_lo, cnt_hi);

        // Horizontal sum
        let sum = _mm256_sad_epu8(cnt, _mm256_setzero_si256());
        let sum128_lo = _mm256_castsi256_si128(sum);
        let sum128_hi = _mm256_extracti128_si256(sum, 1);
        let total = _mm_add_epi64(sum128_lo, sum128_hi);

        count += _mm_extract_epi64(total, 0) as u32;
        count += _mm_extract_epi64(total, 1) as u32;
    }

    // Handle remainder
    for i in (chunks * 32)..n {
        count += (a[i] ^ b[i]).count_ones();
    }

    count
}

/// SIMD-optimized Hamming distance with runtime dispatch
pub fn hamming_distance_simd(a: &[u8], b: &[u8]) -> u32 {
    debug_assert_eq!(a.len(), b.len());

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") && a.len() >= 32 {
            return unsafe { hamming_distance_avx2(a, b) };
        }
        if is_x86_feature_detected!("popcnt") {
            return unsafe { hamming_distance_popcnt(a, b) };
        }
    }

    hamming_distance(a, b)
}

// ============================================================================
// Display & Parsing
// ============================================================================

impl fmt::Display for BinaryVec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        for i in 0..self.dimensions as usize {
            if i > 0 {
                write!(f, ",")?;
            }
            write!(f, "{}", if self.get_bit(i) { 1 } else { 0 })?;
        }
        write!(f, "]")
    }
}

impl fmt::Debug for BinaryVec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BinaryVec(dims={}, bits=[", self.dimensions)?;
        for i in 0..self.dimensions.min(16) as usize {
            write!(f, "{}", if self.get_bit(i) { 1 } else { 0 })?;
        }
        if self.dimensions > 16 {
            write!(f, "...")?;
        }
        write!(f, "])")
    }
}

impl FromStr for BinaryVec {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Parse format: [1,0,1,0] or [1.0, 0.0, 1.0]
        let s = s.trim();
        if !s.starts_with('[') || !s.ends_with(']') {
            return Err(format!("Invalid BinaryVec format: {}", s));
        }

        let inner = &s[1..s.len() - 1];
        if inner.is_empty() {
            return Ok(Self {
                dimensions: 0,
                data: Vec::new(),
            });
        }

        let values: Result<Vec<f32>, _> =
            inner.split(',').map(|v| v.trim().parse::<f32>()).collect();

        match values {
            Ok(data) => Ok(Self::from_f32(&data)),
            Err(e) => Err(format!("Invalid BinaryVec element: {}", e)),
        }
    }
}

impl PartialEq for BinaryVec {
    fn eq(&self, other: &Self) -> bool {
        self.dimensions == other.dimensions && self.data == other.data
    }
}

impl Eq for BinaryVec {}

// ============================================================================
// PostgreSQL Type Integration
// ============================================================================

unsafe impl SqlTranslatable for BinaryVec {
    fn argument_sql() -> Result<SqlMapping, ArgumentError> {
        Ok(SqlMapping::As(String::from("binaryvec")))
    }

    fn return_sql() -> Result<Returns, ReturnsError> {
        Ok(Returns::One(SqlMapping::As(String::from("binaryvec"))))
    }
}

impl pgrx::IntoDatum for BinaryVec {
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

impl pgrx::FromDatum for BinaryVec {
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

        Some(BinaryVec::from_bytes(bytes))
    }
}

// Note: BinaryVec SQL functions are not exposed via #[pg_extern] due to
// pgrx 0.12 trait requirements. Use array-based functions for SQL-level operations.

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_f32() {
        let v = BinaryVec::from_f32(&[1.0, -0.5, 0.3, -0.8, 0.2, -0.1, 0.9, -0.5]);
        assert_eq!(v.dimensions(), 8);
        assert!(v.get_bit(0)); // 1.0 > 0
        assert!(!v.get_bit(1)); // -0.5 <= 0
        assert!(v.get_bit(2)); // 0.3 > 0
        assert!(!v.get_bit(3)); // -0.8 <= 0
    }

    #[test]
    fn test_hamming_distance() {
        let a = BinaryVec::from_f32(&[1.0, 0.0, 1.0, 0.0]);
        let b = BinaryVec::from_f32(&[1.0, 1.0, 0.0, 0.0]);
        // Differs in positions 1 and 2
        assert_eq!(a.hamming_distance(&b), 2);
    }

    #[test]
    fn test_compression_ratio() {
        assert_eq!(BinaryVec::compression_ratio(), 32.0);
    }

    #[test]
    fn test_serialization() {
        let v = BinaryVec::from_f32(&[1.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0]);
        let bytes = v.to_bytes();
        let v2 = BinaryVec::from_bytes(&bytes);
        assert_eq!(v, v2);
    }

    #[test]
    fn test_simd_matches_scalar() {
        let a_data = vec![0b11110000u8, 0b10101010, 0b11001100];
        let b_data = vec![0b00001111u8, 0b01010101, 0b00110011];

        let scalar = hamming_distance(&a_data, &b_data);
        let simd = hamming_distance_simd(&a_data, &b_data);

        assert_eq!(scalar, simd);
    }

    #[test]
    fn test_popcount() {
        let v = BinaryVec::from_f32(&[1.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0]);
        assert_eq!(v.popcount(), 4);
    }

    #[test]
    fn test_parse() {
        let v: BinaryVec = "[1,0,1,0]".parse().unwrap();
        assert_eq!(v.dimensions(), 4);
        assert!(v.get_bit(0));
        assert!(!v.get_bit(1));
    }
}
