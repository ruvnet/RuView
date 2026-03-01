//! ProductVec - Native product quantized vector type (PQ)
//!
//! Stores vectors using product quantization with precomputed codebooks.
//! Achieves 8-32x compression with ADC (Asymmetric Distance Computation).

use pgrx::pgrx_sql_entity_graph::metadata::{
    ArgumentError, Returns, ReturnsError, SqlMapping, SqlTranslatable,
};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use crate::MAX_DIMENSIONS;

/// ProductVec: Product quantized vector
///
/// Memory layout (varlena):
/// - Header: 4 bytes (varlena header)
/// - Original dimensions: 2 bytes (u16)
/// - Num subspaces (m): 1 byte (u8)
/// - Num centroids (k): 1 byte (u8) - typically 256
/// - Codes: m bytes (one code per subspace)
///
/// Maximum original dimensions: 16,000
/// Compression ratio: 8-32x vs f32 (depending on m)
#[derive(Clone, Serialize, Deserialize)]
pub struct ProductVec {
    /// Original vector dimensions
    original_dims: u16,
    /// Number of subspaces
    m: u8,
    /// Number of centroids per subspace (typically 256 for 8-bit codes)
    k: u8,
    /// PQ codes (one u8 per subspace)
    codes: Vec<u8>,
}

impl ProductVec {
    /// Create a new ProductVec
    pub fn new(original_dims: u16, m: u8, k: u8, codes: Vec<u8>) -> Self {
        if codes.len() != m as usize {
            pgrx::error!("ProductVec codes length {} must match m={}", codes.len(), m);
        }

        if original_dims as usize > MAX_DIMENSIONS {
            pgrx::error!(
                "Vector dimension {} exceeds maximum {}",
                original_dims,
                MAX_DIMENSIONS
            );
        }

        Self {
            original_dims,
            m,
            k,
            codes,
        }
    }

    /// Get original dimensions
    #[inline]
    pub fn original_dims(&self) -> usize {
        self.original_dims as usize
    }

    /// Get number of subspaces
    #[inline]
    pub fn m(&self) -> usize {
        self.m as usize
    }

    /// Get number of centroids per subspace
    #[inline]
    pub fn k(&self) -> usize {
        self.k as usize
    }

    /// Get PQ codes
    #[inline]
    pub fn codes(&self) -> &[u8] {
        &self.codes
    }

    /// Get dimensions per subspace
    #[inline]
    pub fn dims_per_subspace(&self) -> usize {
        self.original_dims as usize / self.m as usize
    }

    /// Calculate ADC distance using precomputed distance table
    ///
    /// Distance table format: [m][k] where m = number of subspaces, k = centroids
    /// Each entry is the squared distance from query subvector to centroid
    pub fn adc_distance(&self, distance_table: &[Vec<f32>]) -> f32 {
        debug_assert_eq!(distance_table.len(), self.m as usize);

        let mut distance_sq = 0.0f32;

        for (subspace, &code) in self.codes.iter().enumerate() {
            debug_assert!(code < self.k);
            distance_sq += distance_table[subspace][code as usize];
        }

        distance_sq.sqrt()
    }

    /// Calculate ADC distance using flat distance table
    ///
    /// Flat table format: contiguous array of m*k values
    /// More cache-friendly for SIMD operations
    pub fn adc_distance_flat(&self, distance_table: &[f32]) -> f32 {
        debug_assert_eq!(distance_table.len(), self.m as usize * self.k as usize);

        let mut distance_sq = 0.0f32;
        let k = self.k as usize;

        for (subspace, &code) in self.codes.iter().enumerate() {
            let idx = subspace * k + code as usize;
            distance_sq += distance_table[idx];
        }

        distance_sq.sqrt()
    }

    /// Calculate ADC distance with SIMD optimization
    pub fn adc_distance_simd(&self, distance_table: &[f32]) -> f32 {
        adc_distance_simd(&self.codes, distance_table, self.k as usize)
    }

    /// Memory size in bytes
    pub fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.codes.len()
    }

    /// Compression ratio vs f32
    pub fn compression_ratio(&self) -> f32 {
        (self.original_dims as f32 * 4.0) / self.m as f32
    }

    /// Serialize to bytes
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(4 + self.codes.len());
        bytes.extend_from_slice(&self.original_dims.to_le_bytes());
        bytes.push(self.m);
        bytes.push(self.k);
        bytes.extend_from_slice(&self.codes);
        bytes
    }

    /// Deserialize from bytes
    fn from_bytes(bytes: &[u8]) -> Self {
        if bytes.len() < 4 {
            pgrx::error!("Invalid ProductVec data: too short");
        }

        let original_dims = u16::from_le_bytes([bytes[0], bytes[1]]);
        let m = bytes[2];
        let k = bytes[3];

        let expected_len = 4 + m as usize;
        if bytes.len() != expected_len {
            pgrx::error!(
                "Invalid ProductVec data: expected {} bytes, got {}",
                expected_len,
                bytes.len()
            );
        }

        let codes = bytes[4..].to_vec();

        Self {
            original_dims,
            m,
            k,
            codes,
        }
    }
}

// ============================================================================
// SIMD-Optimized ADC Distance
// ============================================================================

/// Calculate ADC distance using flat distance table (scalar)
#[inline]
pub fn adc_distance_scalar(codes: &[u8], distance_table: &[f32], k: usize) -> f32 {
    let mut distance_sq = 0.0f32;

    for (subspace, &code) in codes.iter().enumerate() {
        let idx = subspace * k + code as usize;
        distance_sq += distance_table[idx];
    }

    distance_sq.sqrt()
}

/// SIMD-optimized ADC distance using AVX2 (x86_64)
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn adc_distance_avx2(codes: &[u8], distance_table: &[f32], k: usize) -> f32 {
    use std::arch::x86_64::*;

    let m = codes.len();
    let mut sum = _mm256_setzero_ps();

    // Process 8 subspaces at a time
    let chunks = m / 8;
    for i in 0..chunks {
        let offset = i * 8;

        // Gather 8 distances based on codes
        let mut distances = [0.0f32; 8];
        for j in 0..8 {
            let subspace = offset + j;
            let code = codes[subspace];
            let idx = subspace * k + code as usize;
            distances[j] = distance_table[idx];
        }

        let v = _mm256_loadu_ps(distances.as_ptr());
        sum = _mm256_add_ps(sum, v);
    }

    // Horizontal sum
    let sum128_lo = _mm256_castps256_ps128(sum);
    let sum128_hi = _mm256_extractf128_ps(sum, 1);
    let sum128 = _mm_add_ps(sum128_lo, sum128_hi);

    let sum64 = _mm_add_ps(sum128, _mm_movehl_ps(sum128, sum128));
    let sum32 = _mm_add_ss(sum64, _mm_shuffle_ps(sum64, sum64, 1));

    let mut result = _mm_cvtss_f32(sum32);

    // Handle remainder
    for subspace in (chunks * 8)..m {
        let code = codes[subspace];
        let idx = subspace * k + code as usize;
        result += distance_table[idx];
    }

    result.sqrt()
}

/// SIMD-optimized ADC distance with runtime dispatch
pub fn adc_distance_simd(codes: &[u8], distance_table: &[f32], k: usize) -> f32 {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") && codes.len() >= 8 {
            return unsafe { adc_distance_avx2(codes, distance_table, k) };
        }
    }

    adc_distance_scalar(codes, distance_table, k)
}

// ============================================================================
// Display & Parsing
// ============================================================================

impl fmt::Display for ProductVec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PQ(dims={}, m={}, k={}, codes=[",
            self.original_dims, self.m, self.k
        )?;
        for (i, &code) in self.codes.iter().enumerate() {
            if i > 0 {
                write!(f, ",")?;
            }
            write!(f, "{}", code)?;
        }
        write!(f, "])")
    }
}

impl fmt::Debug for ProductVec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ProductVec(dims={}, m={}, k={}, codes={:?})",
            self.original_dims, self.m, self.k, self.codes
        )
    }
}

impl FromStr for ProductVec {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Parse format: PQ(dims=1536, m=48, k=256, codes=[1,2,3,...])
        // This is primarily for testing; normal usage would be via encoding

        if !s.starts_with("PQ(") || !s.ends_with(')') {
            return Err(format!("Invalid ProductVec format: {}", s));
        }

        let inner = &s[3..s.len() - 1];
        let parts: Vec<&str> = inner.split(", codes=").collect();

        if parts.len() != 2 {
            return Err("ProductVec must have dims/m/k and codes".to_string());
        }

        // Parse dims, m, k
        let params: Vec<&str> = parts[0].split(", ").collect();
        let mut dims = 0u16;
        let mut m = 0u8;
        let mut k = 0u8;

        for param in params {
            let kv: Vec<&str> = param.split('=').collect();
            if kv.len() != 2 {
                continue;
            }
            match kv[0] {
                "dims" => dims = kv[1].parse().map_err(|e| format!("Invalid dims: {}", e))?,
                "m" => m = kv[1].parse().map_err(|e| format!("Invalid m: {}", e))?,
                "k" => k = kv[1].parse().map_err(|e| format!("Invalid k: {}", e))?,
                _ => {}
            }
        }

        // Parse codes
        let codes_str = parts[1].trim();
        if !codes_str.starts_with('[') || !codes_str.ends_with(']') {
            return Err("Codes must be enclosed in []".to_string());
        }

        let codes_inner = &codes_str[1..codes_str.len() - 1];
        let codes: Result<Vec<u8>, _> = codes_inner
            .split(',')
            .map(|s| s.trim().parse::<u8>())
            .collect();

        let codes = codes.map_err(|e| format!("Invalid code value: {}", e))?;

        Ok(Self::new(dims, m, k, codes))
    }
}

impl PartialEq for ProductVec {
    fn eq(&self, other: &Self) -> bool {
        self.original_dims == other.original_dims
            && self.m == other.m
            && self.k == other.k
            && self.codes == other.codes
    }
}

impl Eq for ProductVec {}

// ============================================================================
// PostgreSQL Type Integration
// ============================================================================

unsafe impl SqlTranslatable for ProductVec {
    fn argument_sql() -> Result<SqlMapping, ArgumentError> {
        Ok(SqlMapping::As(String::from("productvec")))
    }

    fn return_sql() -> Result<Returns, ReturnsError> {
        Ok(Returns::One(SqlMapping::As(String::from("productvec"))))
    }
}

impl pgrx::IntoDatum for ProductVec {
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

impl pgrx::FromDatum for ProductVec {
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

        Some(ProductVec::from_bytes(bytes))
    }
}

// Note: ProductVec SQL functions are not exposed via #[pg_extern] due to
// pgrx 0.12 trait requirements. Use array-based functions for SQL-level operations.

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let codes = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let pq = ProductVec::new(1536, 8, 255, codes.clone());

        assert_eq!(pq.original_dims(), 1536);
        assert_eq!(pq.m(), 8);
        assert_eq!(pq.k(), 255);
        assert_eq!(pq.codes(), &codes[..]);
    }

    #[test]
    fn test_dims_per_subspace() {
        let pq = ProductVec::new(1536, 48, 255, vec![0; 48]);
        assert_eq!(pq.dims_per_subspace(), 32); // 1536 / 48 = 32
    }

    #[test]
    fn test_compression_ratio() {
        let pq = ProductVec::new(1536, 48, 255, vec![0; 48]);
        // 1536 * 4 bytes = 6144 bytes / 48 bytes = 128x
        assert!((pq.compression_ratio() - 128.0).abs() < 0.1);
    }

    #[test]
    fn test_adc_distance() {
        let codes = vec![0, 1, 2, 3];
        let pq = ProductVec::new(64, 4, 4, codes);

        // Create a simple distance table: [4 subspaces][4 centroids]
        let table: Vec<Vec<f32>> = vec![
            vec![0.0, 1.0, 4.0, 9.0], // subspace 0
            vec![0.0, 1.0, 4.0, 9.0], // subspace 1
            vec![0.0, 1.0, 4.0, 9.0], // subspace 2
            vec![0.0, 1.0, 4.0, 9.0], // subspace 3
        ];

        let dist = pq.adc_distance(&table);
        // sqrt(0 + 1 + 4 + 9) = sqrt(14) ≈ 3.74
        assert!((dist - 3.74).abs() < 0.01);
    }

    #[test]
    fn test_adc_distance_flat() {
        let codes = vec![0, 1, 2, 3];
        let pq = ProductVec::new(64, 4, 4, codes);

        // Flat table: 4 subspaces * 4 centroids = 16 values
        let flat_table = vec![
            0.0, 1.0, 4.0, 9.0, // subspace 0
            0.0, 1.0, 4.0, 9.0, // subspace 1
            0.0, 1.0, 4.0, 9.0, // subspace 2
            0.0, 1.0, 4.0, 9.0, // subspace 3
        ];

        let dist = pq.adc_distance_flat(&flat_table);
        assert!((dist - 3.74).abs() < 0.01);
    }

    #[test]
    fn test_serialization() {
        let codes = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let pq = ProductVec::new(1536, 8, 255, codes);

        let bytes = pq.to_bytes();
        let pq2 = ProductVec::from_bytes(&bytes);

        assert_eq!(pq, pq2);
    }

    #[test]
    fn test_simd_matches_scalar() {
        let codes = vec![10, 20, 30, 40, 50, 60, 70, 80];
        let k = 256;

        // Create distance table with random-ish values
        let mut table = Vec::with_capacity(codes.len() * k);
        for i in 0..(codes.len() * k) {
            table.push((i % 100) as f32 * 0.1);
        }

        let scalar = adc_distance_scalar(&codes, &table, k);
        let simd = adc_distance_simd(&codes, &table, k);

        assert!((scalar - simd).abs() < 0.001);
    }

    #[test]
    fn test_parse() {
        let s = "PQ(dims=64, m=4, k=16, codes=[1,2,3,4])";
        let pq: ProductVec = s.parse().unwrap();

        assert_eq!(pq.original_dims(), 64);
        assert_eq!(pq.m(), 4);
        assert_eq!(pq.k(), 16);
        assert_eq!(pq.codes(), &[1, 2, 3, 4]);
    }
}
