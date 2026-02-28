//! Scalar Quantization (SQ8)
//!
//! Compresses f32 vectors to i8, achieving 4x memory reduction
//! with minimal accuracy loss.

/// Quantize f32 vector to i8
///
/// Returns (quantized_data, scale, offset)
pub fn quantize(vector: &[f32]) -> (Vec<i8>, f32, f32) {
    if vector.is_empty() {
        return (Vec::new(), 1.0, 0.0);
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
    let quantized: Vec<i8> = vector
        .iter()
        .map(|&v| {
            let normalized = (v - offset) / scale;
            (normalized.clamp(0.0, 254.0) - 127.0) as i8
        })
        .collect();

    (quantized, scale, offset)
}

/// Dequantize i8 vector back to f32
pub fn dequantize(quantized: &[i8], scale: f32, offset: f32) -> Vec<f32> {
    quantized
        .iter()
        .map(|&q| (q as f32 + 127.0) * scale + offset)
        .collect()
}

/// Calculate squared Euclidean distance between quantized vectors
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

/// Calculate Euclidean distance between quantized vectors
pub fn distance(a: &[i8], b: &[i8], scale: f32) -> f32 {
    (distance_sq(a, b) as f32).sqrt() * scale
}

/// Quantized vector with metadata
#[derive(Debug, Clone)]
pub struct ScalarQuantizedVector {
    pub data: Vec<i8>,
    pub scale: f32,
    pub offset: f32,
}

impl ScalarQuantizedVector {
    /// Create from f32 vector
    pub fn from_f32(vector: &[f32]) -> Self {
        let (data, scale, offset) = quantize(vector);
        Self {
            data,
            scale,
            offset,
        }
    }

    /// Convert back to f32
    pub fn to_f32(&self) -> Vec<f32> {
        dequantize(&self.data, self.scale, self.offset)
    }

    /// Calculate distance to another quantized vector
    pub fn distance(&self, other: &Self) -> f32 {
        let max_scale = self.scale.max(other.scale);
        distance(&self.data, &other.data, max_scale)
    }

    /// Memory size in bytes
    pub fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.data.len()
    }

    /// Compression ratio compared to f32
    pub fn compression_ratio(&self) -> f32 {
        4.0 // f32 (4 bytes) -> i8 (1 byte)
    }
}

// ============================================================================
// SIMD-optimized distance (for larger vectors)
// ============================================================================

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn distance_sq_avx2(a: &[i8], b: &[i8]) -> i32 {
    use std::arch::x86_64::*;

    let n = a.len();
    let mut sum = _mm256_setzero_si256();

    let chunks = n / 32;
    for i in 0..chunks {
        let offset = i * 32;

        let va = _mm256_loadu_si256(a.as_ptr().add(offset) as *const __m256i);
        let vb = _mm256_loadu_si256(b.as_ptr().add(offset) as *const __m256i);

        // Subtract (with sign extension trick for i8)
        let diff_lo = _mm256_sub_epi16(
            _mm256_cvtepi8_epi16(_mm256_castsi256_si128(va)),
            _mm256_cvtepi8_epi16(_mm256_castsi256_si128(vb)),
        );
        let diff_hi = _mm256_sub_epi16(
            _mm256_cvtepi8_epi16(_mm256_extracti128_si256(va, 1)),
            _mm256_cvtepi8_epi16(_mm256_extracti128_si256(vb, 1)),
        );

        // Square and accumulate
        let sq_lo = _mm256_madd_epi16(diff_lo, diff_lo);
        let sq_hi = _mm256_madd_epi16(diff_hi, diff_hi);

        sum = _mm256_add_epi32(sum, sq_lo);
        sum = _mm256_add_epi32(sum, sq_hi);
    }

    // Horizontal sum
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

/// SIMD-accelerated distance calculation
pub fn distance_simd(a: &[i8], b: &[i8], scale: f32) -> f32 {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            return (unsafe { distance_sq_avx2(a, b) } as f32).sqrt() * scale;
        }
    }

    distance(a, b, scale)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantize_dequantize() {
        let original = vec![0.1, 0.5, -0.3, 0.8, -0.9];
        let (quantized, scale, offset) = quantize(&original);
        let restored = dequantize(&quantized, scale, offset);

        for (o, r) in original.iter().zip(restored.iter()) {
            assert!((o - r).abs() < 0.02, "orig={}, restored={}", o, r);
        }
    }

    #[test]
    fn test_distance() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];

        let qa = ScalarQuantizedVector::from_f32(&a);
        let qb = ScalarQuantizedVector::from_f32(&b);

        let dist = qa.distance(&qb);
        // Euclidean distance should be sqrt(2) ≈ 1.414
        assert!((dist - 1.414).abs() < 0.2, "dist={}", dist);
    }

    #[test]
    fn test_compression_ratio() {
        let v = ScalarQuantizedVector::from_f32(&vec![0.0; 1000]);
        assert_eq!(v.compression_ratio(), 4.0);
        assert_eq!(v.data.len(), 1000); // 1000 i8 = 1000 bytes
    }

    #[test]
    fn test_simd_matches_scalar() {
        let a: Vec<i8> = (0..128).map(|i| i as i8).collect();
        let b: Vec<i8> = (0..128).map(|i| -(i as i8)).collect();

        let scalar_result = distance_sq(&a, &b);
        let simd_result = (distance_simd(&a, &b, 1.0).powi(2)) as i32;

        assert!((scalar_result - simd_result).abs() < 10);
    }
}
