//! Half-precision (f16) vector type implementation with zero-copy varlena storage
//!
//! HalfVec stores vectors using 16-bit floating point, reducing memory
//! usage by 50% compared to f32 with minimal accuracy loss.
//!
//! Varlena layout:
//! - VARHDRSZ (4 bytes) - PostgreSQL varlena header
//! - dimensions (2 bytes u16) - number of dimensions
//! - unused (2 bytes) - alignment padding
//! - data (2 bytes * dimensions) - f16 data as raw u16 bits

use half::f16;
use pgrx::pgrx_sql_entity_graph::metadata::{
    ArgumentError, Returns, ReturnsError, SqlMapping, SqlTranslatable,
};

use crate::MAX_DIMENSIONS;

/// Varlena layout offset constants
const VARHDRSZ: usize = 4;
const DIMENSIONS_OFFSET: usize = 0; // Offset within data portion (after VARHDRSZ)
const DATA_OFFSET: usize = 4; // Offset to f16 data (2 bytes dim + 2 bytes padding)

/// HalfVec: Zero-copy half-precision vector type
///
/// This is a wrapper around a pointer to PostgreSQL's varlena structure.
/// The actual data lives in PostgreSQL memory, enabling zero-copy operations.
#[derive(Copy, Clone)]
#[repr(C)]
pub struct HalfVec {
    ptr: *mut pgrx::pg_sys::varlena,
}

unsafe impl pgrx::datum::UnboxDatum for HalfVec {
    type As<'src> = HalfVec;

    unsafe fn unbox<'src>(datum: pgrx::datum::Datum<'src>) -> Self::As<'src>
    where
        Self: 'src,
    {
        let ptr = datum
            .sans_lifetime()
            .cast_mut_ptr::<pgrx::pg_sys::varlena>();
        HalfVec { ptr }
    }
}

impl HalfVec {
    /// Create a new HalfVec from f32 slice
    ///
    /// This allocates PostgreSQL memory and populates it with the varlena structure.
    pub fn from_f32(data: &[f32]) -> Self {
        if data.len() > MAX_DIMENSIONS {
            pgrx::error!(
                "Vector dimension {} exceeds maximum {}",
                data.len(),
                MAX_DIMENSIONS
            );
        }

        if data.len() > u16::MAX as usize {
            pgrx::error!("Vector dimension {} exceeds u16::MAX", data.len());
        }

        unsafe {
            let dimensions = data.len() as u16;
            let data_size = DATA_OFFSET + (dimensions as usize * 2);
            let total_size = VARHDRSZ + data_size;

            // Allocate PostgreSQL memory
            let ptr = pgrx::pg_sys::palloc(total_size) as *mut u8;
            let varlena = ptr as *mut pgrx::pg_sys::varlena;

            // Set varlena size
            pgrx::varlena::set_varsize_4b(varlena, total_size as i32);

            // Write dimensions (u16)
            let dim_ptr = ptr.add(VARHDRSZ) as *mut u16;
            *dim_ptr = dimensions.to_le();

            // Write padding (2 bytes of zeros)
            let padding_ptr = ptr.add(VARHDRSZ + 2) as *mut u16;
            *padding_ptr = 0;

            // Write f16 data as u16 bits
            let data_ptr = ptr.add(VARHDRSZ + DATA_OFFSET) as *mut u16;
            for (i, &val) in data.iter().enumerate() {
                let f16_val = f16::from_f32(val);
                *data_ptr.add(i) = f16_val.to_bits().to_le();
            }

            HalfVec { ptr: varlena }
        }
    }

    /// Create from f16 slice
    pub fn from_f16(data: &[f16]) -> Self {
        let f32_data: Vec<f32> = data.iter().map(|x| x.to_f32()).collect();
        Self::from_f32(&f32_data)
    }

    /// Get dimensions from the varlena structure
    #[inline]
    pub fn dimensions(&self) -> usize {
        unsafe {
            let ptr = self.ptr as *const u8;
            let dim_ptr = ptr.add(VARHDRSZ) as *const u16;
            u16::from_le(*dim_ptr) as usize
        }
    }

    /// Get pointer to raw u16 data
    #[inline]
    pub fn data_ptr(&self) -> *const u16 {
        unsafe {
            let ptr = self.ptr as *const u8;
            ptr.add(VARHDRSZ + DATA_OFFSET) as *const u16
        }
    }

    /// Get mutable pointer to raw u16 data
    #[inline]
    pub fn data_ptr_mut(&mut self) -> *mut u16 {
        unsafe {
            let ptr = self.ptr as *mut u8;
            ptr.add(VARHDRSZ + DATA_OFFSET) as *mut u16
        }
    }

    /// Get raw u16 data as slice
    #[inline]
    pub fn as_raw(&self) -> &[u16] {
        unsafe {
            let dims = self.dimensions();
            std::slice::from_raw_parts(self.data_ptr(), dims)
        }
    }

    /// Convert to f32 Vec (allocates)
    pub fn to_f32(&self) -> Vec<f32> {
        unsafe {
            let dims = self.dimensions();
            let data_ptr = self.data_ptr();
            let mut result = Vec::with_capacity(dims);

            for i in 0..dims {
                let bits = u16::from_le(*data_ptr.add(i));
                let f16_val = f16::from_bits(bits);
                result.push(f16_val.to_f32());
            }

            result
        }
    }

    /// Convert to f16 Vec (allocates)
    pub fn to_f16(&self) -> Vec<f16> {
        unsafe {
            let dims = self.dimensions();
            let data_ptr = self.data_ptr();
            let mut result = Vec::with_capacity(dims);

            for i in 0..dims {
                let bits = u16::from_le(*data_ptr.add(i));
                result.push(f16::from_bits(bits));
            }

            result
        }
    }

    /// Calculate L2 norm
    pub fn norm(&self) -> f32 {
        unsafe {
            let dims = self.dimensions();
            let data_ptr = self.data_ptr();
            let mut sum = 0.0f32;

            for i in 0..dims {
                let bits = u16::from_le(*data_ptr.add(i));
                let val = f16::from_bits(bits).to_f32();
                sum += val * val;
            }

            sum.sqrt()
        }
    }

    /// Memory size in bytes
    pub fn memory_size(&self) -> usize {
        unsafe { pgrx::varlena::varsize_any(self.ptr) }
    }
}

// ============================================================================
// PostgreSQL I/O Functions - Internal use only
// ============================================================================
// Note: HalfVec type uses internal SIMD-optimized distance functions.
// Public SQL functions are defined via raw C calling convention or SQL.

/// Internal: Parse HalfVec from text format: [1.0, 2.0, 3.0]
pub fn halfvec_parse(input: &str) -> HalfVec {
    match parse_halfvec_string(input) {
        Ok(data) => HalfVec::from_f32(&data),
        Err(e) => pgrx::error!("Invalid halfvec format: {}", e),
    }
}

/// Internal: Format HalfVec to text format
pub fn halfvec_format(vector: &HalfVec) -> String {
    let dims = vector.dimensions();
    let data_ptr = vector.data_ptr();

    let mut result = String::from("[");
    unsafe {
        for i in 0..dims {
            if i > 0 {
                result.push(',');
            }
            let bits = u16::from_le(*data_ptr.add(i));
            let val = f16::from_bits(bits).to_f32();
            result.push_str(&format!("{}", val));
        }
    }
    result.push(']');
    result
}

// ============================================================================
// Internal Distance Functions with SIMD Optimization
// ============================================================================

/// Internal: L2 (Euclidean) distance for HalfVec
pub fn halfvec_l2(a: &HalfVec, b: &HalfVec) -> f32 {
    let dims_a = a.dimensions();
    let dims_b = b.dimensions();

    if dims_a != dims_b {
        pgrx::error!("Vector dimensions must match: {} vs {}", dims_a, dims_b);
    }

    unsafe { halfvec_euclidean_distance_dispatch(a, b) }
}

/// Internal: Cosine distance for HalfVec
pub fn halfvec_cosine(a: &HalfVec, b: &HalfVec) -> f32 {
    let dims_a = a.dimensions();
    let dims_b = b.dimensions();

    if dims_a != dims_b {
        pgrx::error!("Vector dimensions must match: {} vs {}", dims_a, dims_b);
    }

    unsafe { halfvec_cosine_distance_dispatch(a, b) }
}

/// Internal: Inner product distance for HalfVec
pub fn halfvec_ip(a: &HalfVec, b: &HalfVec) -> f32 {
    let dims_a = a.dimensions();
    let dims_b = b.dimensions();

    if dims_a != dims_b {
        pgrx::error!("Vector dimensions must match: {} vs {}", dims_a, dims_b);
    }

    unsafe { halfvec_inner_product_dispatch(a, b) }
}

// ============================================================================
// SIMD Distance Implementations
// ============================================================================

/// Dispatch to appropriate SIMD implementation for Euclidean distance
#[inline]
unsafe fn halfvec_euclidean_distance_dispatch(a: &HalfVec, b: &HalfVec) -> f32 {
    #[cfg(target_arch = "x86_64")]
    {
        // AVX-512 FP16 requires nightly Rust - disabled for stable builds
        // if is_x86_feature_detected!("avx512fp16") {
        //     return halfvec_euclidean_avx512fp16(a, b);
        // }
        if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("f16c") {
            return halfvec_euclidean_avx2_f16c(a, b);
        }
    }

    // Scalar fallback
    halfvec_euclidean_scalar(a, b)
}

/// Dispatch for cosine distance
#[inline]
unsafe fn halfvec_cosine_distance_dispatch(a: &HalfVec, b: &HalfVec) -> f32 {
    #[cfg(target_arch = "x86_64")]
    {
        // AVX-512 FP16 requires nightly Rust - disabled for stable builds
        // if is_x86_feature_detected!("avx512fp16") {
        //     return halfvec_cosine_avx512fp16(a, b);
        // }
        if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("f16c") {
            return halfvec_cosine_avx2_f16c(a, b);
        }
    }

    halfvec_cosine_scalar(a, b)
}

/// Dispatch for inner product
#[inline]
unsafe fn halfvec_inner_product_dispatch(a: &HalfVec, b: &HalfVec) -> f32 {
    #[cfg(target_arch = "x86_64")]
    {
        // AVX-512 FP16 requires nightly Rust - disabled for stable builds
        // if is_x86_feature_detected!("avx512fp16") {
        //     return halfvec_inner_product_avx512fp16(a, b);
        // }
        if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("f16c") {
            return halfvec_inner_product_avx2_f16c(a, b);
        }
    }

    halfvec_inner_product_scalar(a, b)
}

// ============================================================================
// AVX-512FP16 Implementations - DISABLED (requires nightly Rust)
// ============================================================================
// Native f16 operations using avx512fp16 require unstable Rust features.
// When running on CPUs with AVX-512 FP16 support (Sapphire Rapids+), we fall
// back to AVX2 + F16C which converts f16 to f32 in SIMD registers.
// To enable native AVX-512 FP16 support, use nightly Rust with:
//   #![feature(stdarch_x86_avx512_f16)]

// ============================================================================
// AVX2 + F16C Implementations (Convert to f32 in SIMD registers)
// ============================================================================

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2", enable = "f16c")]
#[inline]
unsafe fn halfvec_euclidean_avx2_f16c(a: &HalfVec, b: &HalfVec) -> f32 {
    use std::arch::x86_64::*;

    let dims = a.dimensions();
    let a_ptr = a.data_ptr();
    let b_ptr = b.data_ptr();

    // Process 8 f16 values at a time (128 bits -> 256 bits f32)
    let chunks = dims / 8;
    let mut sum = _mm256_setzero_ps();

    for i in 0..chunks {
        let offset = i * 8;

        // Load 8 f16 values (128 bits)
        let a_f16 = _mm_loadu_si128(a_ptr.add(offset) as *const __m128i);
        let b_f16 = _mm_loadu_si128(b_ptr.add(offset) as *const __m128i);

        // Convert to f32 using vcvtph2ps
        let a_f32 = _mm256_cvtph_ps(a_f16);
        let b_f32 = _mm256_cvtph_ps(b_f16);

        // Compute squared difference
        let diff = _mm256_sub_ps(a_f32, b_f32);
        sum = _mm256_fmadd_ps(diff, diff, sum);
    }

    // Horizontal reduction
    let sum_high = _mm256_extractf128_ps(sum, 1);
    let sum_low = _mm256_castps256_ps128(sum);
    let sum128 = _mm_add_ps(sum_high, sum_low);
    let sum64 = _mm_add_ps(sum128, _mm_movehl_ps(sum128, sum128));
    let sum32 = _mm_add_ss(sum64, _mm_shuffle_ps(sum64, sum64, 0x1));
    let mut result = _mm_cvtss_f32(sum32);

    // Handle remainder
    for i in (chunks * 8)..dims {
        let a_bits = u16::from_le(*a_ptr.add(i));
        let b_bits = u16::from_le(*b_ptr.add(i));
        let a_val = f16::from_bits(a_bits).to_f32();
        let b_val = f16::from_bits(b_bits).to_f32();
        let diff = a_val - b_val;
        result += diff * diff;
    }

    result.sqrt()
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2", enable = "f16c")]
#[inline]
unsafe fn halfvec_cosine_avx2_f16c(a: &HalfVec, b: &HalfVec) -> f32 {
    use std::arch::x86_64::*;

    let dims = a.dimensions();
    let a_ptr = a.data_ptr();
    let b_ptr = b.data_ptr();

    let chunks = dims / 8;
    let mut dot = _mm256_setzero_ps();
    let mut norm_a = _mm256_setzero_ps();
    let mut norm_b = _mm256_setzero_ps();

    for i in 0..chunks {
        let offset = i * 8;

        let a_f16 = _mm_loadu_si128(a_ptr.add(offset) as *const __m128i);
        let b_f16 = _mm_loadu_si128(b_ptr.add(offset) as *const __m128i);

        let a_f32 = _mm256_cvtph_ps(a_f16);
        let b_f32 = _mm256_cvtph_ps(b_f16);

        dot = _mm256_fmadd_ps(a_f32, b_f32, dot);
        norm_a = _mm256_fmadd_ps(a_f32, a_f32, norm_a);
        norm_b = _mm256_fmadd_ps(b_f32, b_f32, norm_b);
    }

    // Horizontal reduction for all three accumulators
    let sum_high = _mm256_extractf128_ps(dot, 1);
    let sum_low = _mm256_castps256_ps128(dot);
    let sum128 = _mm_add_ps(sum_high, sum_low);
    let sum64 = _mm_add_ps(sum128, _mm_movehl_ps(sum128, sum128));
    let sum32 = _mm_add_ss(sum64, _mm_shuffle_ps(sum64, sum64, 0x1));
    let mut dot_sum = _mm_cvtss_f32(sum32);

    let na_high = _mm256_extractf128_ps(norm_a, 1);
    let na_low = _mm256_castps256_ps128(norm_a);
    let na128 = _mm_add_ps(na_high, na_low);
    let na64 = _mm_add_ps(na128, _mm_movehl_ps(na128, na128));
    let na32 = _mm_add_ss(na64, _mm_shuffle_ps(na64, na64, 0x1));
    let mut norm_a_sum = _mm_cvtss_f32(na32);

    let nb_high = _mm256_extractf128_ps(norm_b, 1);
    let nb_low = _mm256_castps256_ps128(norm_b);
    let nb128 = _mm_add_ps(nb_high, nb_low);
    let nb64 = _mm_add_ps(nb128, _mm_movehl_ps(nb128, nb128));
    let nb32 = _mm_add_ss(nb64, _mm_shuffle_ps(nb64, nb64, 0x1));
    let mut norm_b_sum = _mm_cvtss_f32(nb32);

    // Handle remainder
    for i in (chunks * 8)..dims {
        let a_bits = u16::from_le(*a_ptr.add(i));
        let b_bits = u16::from_le(*b_ptr.add(i));
        let a_val = f16::from_bits(a_bits).to_f32();
        let b_val = f16::from_bits(b_bits).to_f32();
        dot_sum += a_val * b_val;
        norm_a_sum += a_val * a_val;
        norm_b_sum += b_val * b_val;
    }

    let denominator = (norm_a_sum * norm_b_sum).sqrt();
    if denominator == 0.0 {
        return 1.0;
    }

    1.0 - (dot_sum / denominator)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2", enable = "f16c")]
#[inline]
unsafe fn halfvec_inner_product_avx2_f16c(a: &HalfVec, b: &HalfVec) -> f32 {
    use std::arch::x86_64::*;

    let dims = a.dimensions();
    let a_ptr = a.data_ptr();
    let b_ptr = b.data_ptr();

    let chunks = dims / 8;
    let mut sum = _mm256_setzero_ps();

    for i in 0..chunks {
        let offset = i * 8;

        let a_f16 = _mm_loadu_si128(a_ptr.add(offset) as *const __m128i);
        let b_f16 = _mm_loadu_si128(b_ptr.add(offset) as *const __m128i);

        let a_f32 = _mm256_cvtph_ps(a_f16);
        let b_f32 = _mm256_cvtph_ps(b_f16);

        sum = _mm256_fmadd_ps(a_f32, b_f32, sum);
    }

    // Horizontal reduction
    let sum_high = _mm256_extractf128_ps(sum, 1);
    let sum_low = _mm256_castps256_ps128(sum);
    let sum128 = _mm_add_ps(sum_high, sum_low);
    let sum64 = _mm_add_ps(sum128, _mm_movehl_ps(sum128, sum128));
    let sum32 = _mm_add_ss(sum64, _mm_shuffle_ps(sum64, sum64, 0x1));
    let mut result = _mm_cvtss_f32(sum32);

    // Handle remainder
    for i in (chunks * 8)..dims {
        let a_bits = u16::from_le(*a_ptr.add(i));
        let b_bits = u16::from_le(*b_ptr.add(i));
        let a_val = f16::from_bits(a_bits).to_f32();
        let b_val = f16::from_bits(b_bits).to_f32();
        result += a_val * b_val;
    }

    -result
}

// ============================================================================
// Scalar Fallback Implementations
// ============================================================================

#[inline]
unsafe fn halfvec_euclidean_scalar(a: &HalfVec, b: &HalfVec) -> f32 {
    let dims = a.dimensions();
    let a_ptr = a.data_ptr();
    let b_ptr = b.data_ptr();

    let mut sum = 0.0f32;
    for i in 0..dims {
        let a_bits = u16::from_le(*a_ptr.add(i));
        let b_bits = u16::from_le(*b_ptr.add(i));
        let a_val = f16::from_bits(a_bits).to_f32();
        let b_val = f16::from_bits(b_bits).to_f32();
        let diff = a_val - b_val;
        sum += diff * diff;
    }

    sum.sqrt()
}

#[inline]
unsafe fn halfvec_cosine_scalar(a: &HalfVec, b: &HalfVec) -> f32 {
    let dims = a.dimensions();
    let a_ptr = a.data_ptr();
    let b_ptr = b.data_ptr();

    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    for i in 0..dims {
        let a_bits = u16::from_le(*a_ptr.add(i));
        let b_bits = u16::from_le(*b_ptr.add(i));
        let a_val = f16::from_bits(a_bits).to_f32();
        let b_val = f16::from_bits(b_bits).to_f32();

        dot += a_val * b_val;
        norm_a += a_val * a_val;
        norm_b += b_val * b_val;
    }

    let denominator = (norm_a * norm_b).sqrt();
    if denominator == 0.0 {
        return 1.0;
    }

    1.0 - (dot / denominator)
}

#[inline]
unsafe fn halfvec_inner_product_scalar(a: &HalfVec, b: &HalfVec) -> f32 {
    let dims = a.dimensions();
    let a_ptr = a.data_ptr();
    let b_ptr = b.data_ptr();

    let mut sum = 0.0f32;
    for i in 0..dims {
        let a_bits = u16::from_le(*a_ptr.add(i));
        let b_bits = u16::from_le(*b_ptr.add(i));
        let a_val = f16::from_bits(a_bits).to_f32();
        let b_val = f16::from_bits(b_bits).to_f32();
        sum += a_val * b_val;
    }

    -sum
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Parse halfvec string format: [1.0, 2.0, 3.0]
fn parse_halfvec_string(s: &str) -> Result<Vec<f32>, String> {
    let s = s.trim();
    if !s.starts_with('[') || !s.ends_with(']') {
        return Err(format!(
            "Invalid halfvec format: must start with '[' and end with ']'"
        ));
    }

    let inner = &s[1..s.len() - 1];
    if inner.is_empty() {
        return Ok(Vec::new());
    }

    let values: Result<Vec<f32>, _> = inner.split(',').map(|v| v.trim().parse::<f32>()).collect();

    match values {
        Ok(data) => {
            if data.len() > MAX_DIMENSIONS {
                Err(format!(
                    "Vector dimension {} exceeds maximum {}",
                    data.len(),
                    MAX_DIMENSIONS
                ))
            } else {
                Ok(data)
            }
        }
        Err(e) => Err(format!("Invalid halfvec element: {}", e)),
    }
}

// ============================================================================
// PostgreSQL Type Integration
// ============================================================================

unsafe impl SqlTranslatable for HalfVec {
    fn argument_sql() -> Result<SqlMapping, ArgumentError> {
        Ok(SqlMapping::As(String::from("halfvec")))
    }

    fn return_sql() -> Result<Returns, ReturnsError> {
        Ok(Returns::One(SqlMapping::As(String::from("halfvec"))))
    }
}

impl pgrx::IntoDatum for HalfVec {
    fn into_datum(self) -> Option<pgrx::pg_sys::Datum> {
        Some(pgrx::pg_sys::Datum::from(self.ptr))
    }

    fn type_oid() -> pgrx::pg_sys::Oid {
        pgrx::pg_sys::Oid::INVALID
    }
}

impl pgrx::FromDatum for HalfVec {
    unsafe fn from_polymorphic_datum(
        datum: pgrx::pg_sys::Datum,
        is_null: bool,
        _typoid: pgrx::pg_sys::Oid,
    ) -> Option<Self> {
        if is_null {
            return None;
        }

        let ptr = datum.cast_mut_ptr::<pgrx::pg_sys::varlena>();
        Some(HalfVec { ptr })
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_halfvec_string() {
        let result = parse_halfvec_string("[1.0, 2.0, 3.0]").unwrap();
        assert_eq!(result, vec![1.0, 2.0, 3.0]);

        let result2 = parse_halfvec_string("[1,2,3]").unwrap();
        assert_eq!(result2, vec![1.0, 2.0, 3.0]);

        let result3 = parse_halfvec_string("[]").unwrap();
        assert_eq!(result3.len(), 0);
    }

    #[test]
    fn test_halfvec_memory_layout() {
        let data = vec![1.0f32, 2.0, 3.0];
        let hvec = HalfVec::from_f32(&data);

        // Check dimensions
        assert_eq!(hvec.dimensions(), 3);

        // Check data
        let f32_data = hvec.to_f32();
        assert!((f32_data[0] - 1.0).abs() < 0.01);
        assert!((f32_data[1] - 2.0).abs() < 0.01);
        assert!((f32_data[2] - 3.0).abs() < 0.01);

        // Check memory size: VARHDRSZ(4) + dims(2) + pad(2) + data(3*2) = 14
        assert_eq!(hvec.memory_size(), 14);
    }

    #[test]
    fn test_halfvec_precision() {
        let original = vec![0.123456, -0.654321, 0.999999, -0.000001];
        let hvec = HalfVec::from_f32(&original);
        let restored = hvec.to_f32();

        for (orig, rest) in original.iter().zip(restored.iter()) {
            // f16 has ~3 decimal digits of precision
            assert!(
                (orig - rest).abs() < 0.001,
                "orig={}, restored={}",
                orig,
                rest
            );
        }
    }
}
