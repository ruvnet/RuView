//! Quantization techniques for memory compression
//!
//! This module provides tiered quantization strategies as specified in ADR-001:
//!
//! | Quantization | Compression | Use Case |
//! |--------------|-------------|----------|
//! | Scalar (u8)  | 4x          | Warm data (40-80% access) |
//! | Int4         | 8x          | Cool data (10-40% access) |
//! | Product      | 8-16x       | Cold data (1-10% access) |
//! | Binary       | 32x         | Archive (<1% access) |
//!
//! ## Performance Optimizations v2
//!
//! - SIMD-accelerated distance calculations for scalar (int8) quantization
//! - SIMD popcnt for binary hamming distance
//! - 4x loop unrolling for better instruction-level parallelism
//! - Separate accumulator strategy to reduce data dependencies

use crate::error::Result;
use serde::{Deserialize, Serialize};

/// Trait for quantized vector representations
pub trait QuantizedVector: Send + Sync {
    /// Quantize a full-precision vector
    fn quantize(vector: &[f32]) -> Self;

    /// Calculate distance to another quantized vector
    fn distance(&self, other: &Self) -> f32;

    /// Reconstruct approximate full-precision vector
    fn reconstruct(&self) -> Vec<f32>;
}

/// Scalar quantization to int8 (4x compression)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalarQuantized {
    /// Quantized values (int8)
    pub data: Vec<u8>,
    /// Minimum value for dequantization
    pub min: f32,
    /// Scale factor for dequantization
    pub scale: f32,
}

impl QuantizedVector for ScalarQuantized {
    fn quantize(vector: &[f32]) -> Self {
        let min = vector.iter().copied().fold(f32::INFINITY, f32::min);
        let max = vector.iter().copied().fold(f32::NEG_INFINITY, f32::max);

        // Handle edge case where all values are the same (scale = 0)
        let scale = if (max - min).abs() < f32::EPSILON {
            1.0 // Arbitrary non-zero scale when all values are identical
        } else {
            (max - min) / 255.0
        };

        let data = vector
            .iter()
            .map(|&v| ((v - min) / scale).round().clamp(0.0, 255.0) as u8)
            .collect();

        Self { data, min, scale }
    }

    fn distance(&self, other: &Self) -> f32 {
        // Fast int8 distance calculation with SIMD optimization
        // Use i32 to avoid overflow: max diff is 255, and 255*255=65025 fits in i32

        // Scale handling: We use the average of both scales for balanced comparison.
        // Using max(scale) would bias toward the vector with larger range,
        // while average provides a more symmetric distance metric.
        // This ensures distance(a, b) ≈ distance(b, a) in the reconstructed space.
        let avg_scale = (self.scale + other.scale) / 2.0;

        // Use SIMD-optimized version for larger vectors
        #[cfg(target_arch = "aarch64")]
        {
            if self.data.len() >= 16 {
                return unsafe { scalar_distance_neon(&self.data, &other.data) }.sqrt() * avg_scale;
            }
        }

        #[cfg(target_arch = "x86_64")]
        {
            if self.data.len() >= 32 && is_x86_feature_detected!("avx2") {
                return unsafe { scalar_distance_avx2(&self.data, &other.data) }.sqrt() * avg_scale;
            }
        }

        // Scalar fallback with 4x loop unrolling for better ILP
        scalar_distance_scalar(&self.data, &other.data).sqrt() * avg_scale
    }

    fn reconstruct(&self) -> Vec<f32> {
        self.data
            .iter()
            .map(|&v| self.min + (v as f32) * self.scale)
            .collect()
    }
}

/// Product quantization (8-16x compression)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductQuantized {
    /// Quantized codes (one per subspace)
    pub codes: Vec<u8>,
    /// Codebooks for each subspace
    pub codebooks: Vec<Vec<Vec<f32>>>,
}

impl ProductQuantized {
    /// Train product quantization on a set of vectors
    pub fn train(
        vectors: &[Vec<f32>],
        num_subspaces: usize,
        codebook_size: usize,
        iterations: usize,
    ) -> Result<Self> {
        if vectors.is_empty() {
            return Err(crate::error::RuvectorError::InvalidInput(
                "Cannot train on empty vector set".into(),
            ));
        }
        if vectors[0].is_empty() {
            return Err(crate::error::RuvectorError::InvalidInput(
                "Cannot train on vectors with zero dimensions".into(),
            ));
        }
        if codebook_size > 256 {
            return Err(crate::error::RuvectorError::InvalidParameter(format!(
                "Codebook size {} exceeds u8 maximum of 256",
                codebook_size
            )));
        }
        let dimensions = vectors[0].len();
        let subspace_dim = dimensions / num_subspaces;

        let mut codebooks = Vec::with_capacity(num_subspaces);

        // Train codebook for each subspace using k-means
        for subspace_idx in 0..num_subspaces {
            let start = subspace_idx * subspace_dim;
            let end = start + subspace_dim;

            // Extract subspace vectors
            let subspace_vectors: Vec<Vec<f32>> =
                vectors.iter().map(|v| v[start..end].to_vec()).collect();

            // Run k-means
            let codebook = kmeans_clustering(&subspace_vectors, codebook_size, iterations);
            codebooks.push(codebook);
        }

        Ok(Self {
            codes: vec![],
            codebooks,
        })
    }

    /// Quantize a vector using trained codebooks
    pub fn encode(&self, vector: &[f32]) -> Vec<u8> {
        let num_subspaces = self.codebooks.len();
        let subspace_dim = vector.len() / num_subspaces;

        let mut codes = Vec::with_capacity(num_subspaces);

        for (subspace_idx, codebook) in self.codebooks.iter().enumerate() {
            let start = subspace_idx * subspace_dim;
            let end = start + subspace_dim;
            let subvector = &vector[start..end];

            // Find nearest centroid
            let code = codebook
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    let dist_a = euclidean_squared(subvector, a);
                    let dist_b = euclidean_squared(subvector, b);
                    dist_a.partial_cmp(&dist_b).unwrap()
                })
                .map(|(idx, _)| idx as u8)
                .unwrap_or(0);

            codes.push(code);
        }

        codes
    }
}

/// Int4 quantization (8x compression)
///
/// Quantizes f32 to 4-bit integers (0-15), packing 2 values per byte.
/// Provides 8x compression with better precision than binary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Int4Quantized {
    /// Packed 4-bit values (2 per byte)
    pub data: Vec<u8>,
    /// Minimum value for dequantization
    pub min: f32,
    /// Scale factor for dequantization
    pub scale: f32,
    /// Number of dimensions
    pub dimensions: usize,
}

impl Int4Quantized {
    /// Quantize a vector to 4-bit representation
    pub fn quantize(vector: &[f32]) -> Self {
        let min = vector.iter().copied().fold(f32::INFINITY, f32::min);
        let max = vector.iter().copied().fold(f32::NEG_INFINITY, f32::max);

        // Handle edge case where all values are the same
        let scale = if (max - min).abs() < f32::EPSILON {
            1.0
        } else {
            (max - min) / 15.0 // 4-bit gives 0-15 range
        };

        let dimensions = vector.len();
        let num_bytes = dimensions.div_ceil(2);
        let mut data = vec![0u8; num_bytes];

        for (i, &v) in vector.iter().enumerate() {
            let quantized = ((v - min) / scale).round().clamp(0.0, 15.0) as u8;
            let byte_idx = i / 2;
            if i % 2 == 0 {
                // Low nibble
                data[byte_idx] |= quantized;
            } else {
                // High nibble
                data[byte_idx] |= quantized << 4;
            }
        }

        Self {
            data,
            min,
            scale,
            dimensions,
        }
    }

    /// Calculate distance to another Int4 quantized vector
    pub fn distance(&self, other: &Self) -> f32 {
        assert_eq!(self.dimensions, other.dimensions);

        // Use average scale for balanced comparison
        let avg_scale = (self.scale + other.scale) / 2.0;
        let _avg_min = (self.min + other.min) / 2.0;

        let mut sum_sq = 0i32;

        for i in 0..self.dimensions {
            let byte_idx = i / 2;
            let shift = if i % 2 == 0 { 0 } else { 4 };

            let a = ((self.data[byte_idx] >> shift) & 0x0F) as i32;
            let b = ((other.data[byte_idx] >> shift) & 0x0F) as i32;
            let diff = a - b;
            sum_sq += diff * diff;
        }

        (sum_sq as f32).sqrt() * avg_scale
    }

    /// Reconstruct approximate full-precision vector
    pub fn reconstruct(&self) -> Vec<f32> {
        let mut result = Vec::with_capacity(self.dimensions);

        for i in 0..self.dimensions {
            let byte_idx = i / 2;
            let shift = if i % 2 == 0 { 0 } else { 4 };
            let quantized = (self.data[byte_idx] >> shift) & 0x0F;
            result.push(self.min + (quantized as f32) * self.scale);
        }

        result
    }

    /// Get compression ratio (8x for Int4)
    pub fn compression_ratio() -> f32 {
        8.0 // f32 (4 bytes) -> 4 bits (0.5 bytes)
    }
}

/// Binary quantization (32x compression)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryQuantized {
    /// Binary representation (1 bit per dimension, packed into bytes)
    pub bits: Vec<u8>,
    /// Number of dimensions
    pub dimensions: usize,
}

impl QuantizedVector for BinaryQuantized {
    fn quantize(vector: &[f32]) -> Self {
        let dimensions = vector.len();
        let num_bytes = dimensions.div_ceil(8);
        let mut bits = vec![0u8; num_bytes];

        for (i, &v) in vector.iter().enumerate() {
            if v > 0.0 {
                let byte_idx = i / 8;
                let bit_idx = i % 8;
                bits[byte_idx] |= 1 << bit_idx;
            }
        }

        Self { bits, dimensions }
    }

    fn distance(&self, other: &Self) -> f32 {
        // Hamming distance using SIMD-friendly operations
        Self::hamming_distance_fast(&self.bits, &other.bits) as f32
    }

    fn reconstruct(&self) -> Vec<f32> {
        let mut result = Vec::with_capacity(self.dimensions);

        for i in 0..self.dimensions {
            let byte_idx = i / 8;
            let bit_idx = i % 8;
            let bit = (self.bits[byte_idx] >> bit_idx) & 1;
            result.push(if bit == 1 { 1.0 } else { -1.0 });
        }

        result
    }
}

impl BinaryQuantized {
    /// Fast hamming distance using SIMD-optimized operations
    ///
    /// Uses hardware POPCNT on x86_64 or NEON vcnt on ARM64 for optimal performance.
    /// Processes 16 bytes at a time on ARM64, 8 bytes at a time on x86_64.
    /// Falls back to 64-bit operations for remainders.
    pub fn hamming_distance_fast(a: &[u8], b: &[u8]) -> u32 {
        // Use SIMD-optimized version based on architecture
        #[cfg(target_arch = "aarch64")]
        {
            if a.len() >= 16 {
                return unsafe { hamming_distance_neon(a, b) };
            }
        }

        #[cfg(target_arch = "x86_64")]
        {
            if a.len() >= 8 && is_x86_feature_detected!("popcnt") {
                return unsafe { hamming_distance_simd_x86(a, b) };
            }
        }

        // Scalar fallback using 64-bit operations
        let mut distance = 0u32;

        // Process 8 bytes at a time using u64
        let chunks_a = a.chunks_exact(8);
        let chunks_b = b.chunks_exact(8);
        let remainder_a = chunks_a.remainder();
        let remainder_b = chunks_b.remainder();

        for (chunk_a, chunk_b) in chunks_a.zip(chunks_b) {
            let a_u64 = u64::from_le_bytes(chunk_a.try_into().unwrap());
            let b_u64 = u64::from_le_bytes(chunk_b.try_into().unwrap());
            distance += (a_u64 ^ b_u64).count_ones();
        }

        // Handle remainder bytes
        for (&a_byte, &b_byte) in remainder_a.iter().zip(remainder_b) {
            distance += (a_byte ^ b_byte).count_ones();
        }

        distance
    }

    /// Compute normalized hamming similarity (0.0 to 1.0)
    pub fn similarity(&self, other: &Self) -> f32 {
        let distance = self.distance(other);
        1.0 - (distance / self.dimensions as f32)
    }

    /// Get compression ratio (32x for binary)
    pub fn compression_ratio() -> f32 {
        32.0 // f32 (4 bytes = 32 bits) -> 1 bit
    }

    /// Convert to bytes for storage
    pub fn to_bytes(&self) -> &[u8] {
        &self.bits
    }

    /// Create from bytes
    pub fn from_bytes(bits: Vec<u8>, dimensions: usize) -> Self {
        Self { bits, dimensions }
    }
}

// ============================================================================
// Helper functions for scalar quantization distance
// ============================================================================

/// Scalar fallback for scalar quantization distance (sum of squared differences)
fn scalar_distance_scalar(a: &[u8], b: &[u8]) -> f32 {
    let mut sum_sq = 0i32;

    // 4x loop unrolling for better ILP
    let chunks = a.len() / 4;
    for i in 0..chunks {
        let idx = i * 4;
        let d0 = (a[idx] as i32) - (b[idx] as i32);
        let d1 = (a[idx + 1] as i32) - (b[idx + 1] as i32);
        let d2 = (a[idx + 2] as i32) - (b[idx + 2] as i32);
        let d3 = (a[idx + 3] as i32) - (b[idx + 3] as i32);
        sum_sq += d0 * d0 + d1 * d1 + d2 * d2 + d3 * d3;
    }

    // Handle remainder
    for i in (chunks * 4)..a.len() {
        let diff = (a[i] as i32) - (b[i] as i32);
        sum_sq += diff * diff;
    }

    sum_sq as f32
}

/// NEON SIMD distance for scalar quantization
///
/// # Safety
/// Caller must ensure a.len() == b.len()
#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn scalar_distance_neon(a: &[u8], b: &[u8]) -> f32 {
    use std::arch::aarch64::*;

    let len = a.len();
    let a_ptr = a.as_ptr();
    let b_ptr = b.as_ptr();

    let mut sum = vdupq_n_s32(0);

    // Process 8 bytes at a time
    let chunks = len / 8;
    let mut idx = 0usize;

    for _ in 0..chunks {
        // Load 8 u8 values
        let va = vld1_u8(a_ptr.add(idx));
        let vb = vld1_u8(b_ptr.add(idx));

        // Zero-extend u8 to u16
        let va_u16 = vmovl_u8(va);
        let vb_u16 = vmovl_u8(vb);

        // Convert to signed for subtraction
        let va_s16 = vreinterpretq_s16_u16(va_u16);
        let vb_s16 = vreinterpretq_s16_u16(vb_u16);

        // Compute difference
        let diff = vsubq_s16(va_s16, vb_s16);

        // Square and accumulate
        let prod_lo = vmull_s16(vget_low_s16(diff), vget_low_s16(diff));
        let prod_hi = vmull_s16(vget_high_s16(diff), vget_high_s16(diff));

        sum = vaddq_s32(sum, prod_lo);
        sum = vaddq_s32(sum, prod_hi);

        idx += 8;
    }

    let mut total = vaddvq_s32(sum);

    // Handle remainder with bounds-check elimination
    for i in (chunks * 8)..len {
        let diff = (*a.get_unchecked(i) as i32) - (*b.get_unchecked(i) as i32);
        total += diff * diff;
    }

    total as f32
}

/// AVX2 SIMD distance for scalar quantization
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[inline]
unsafe fn scalar_distance_avx2(a: &[u8], b: &[u8]) -> f32 {
    use std::arch::x86_64::*;

    let len = a.len();
    let mut sum = _mm256_setzero_si256();

    // Process 16 bytes at a time
    let chunks = len / 16;
    for i in 0..chunks {
        let idx = i * 16;

        // Load 16 u8 values
        let va = _mm_loadu_si128(a.as_ptr().add(idx) as *const __m128i);
        let vb = _mm_loadu_si128(b.as_ptr().add(idx) as *const __m128i);

        // Zero-extend u8 to i16 (low and high halves)
        let va_lo = _mm256_cvtepu8_epi16(va);
        let vb_lo = _mm256_cvtepu8_epi16(vb);

        // Compute difference
        let diff = _mm256_sub_epi16(va_lo, vb_lo);

        // Square (multiply i16 * i16 -> i32)
        let prod = _mm256_madd_epi16(diff, diff);

        // Accumulate
        sum = _mm256_add_epi32(sum, prod);
    }

    // Horizontal sum
    let sum_lo = _mm256_castsi256_si128(sum);
    let sum_hi = _mm256_extracti128_si256(sum, 1);
    let sum_128 = _mm_add_epi32(sum_lo, sum_hi);

    let shuffle = _mm_shuffle_epi32(sum_128, 0b10_11_00_01);
    let sum_64 = _mm_add_epi32(sum_128, shuffle);

    let shuffle2 = _mm_shuffle_epi32(sum_64, 0b00_00_10_10);
    let final_sum = _mm_add_epi32(sum_64, shuffle2);

    let mut total = _mm_cvtsi128_si32(final_sum);

    // Handle remainder
    for i in (chunks * 16)..len {
        let diff = (a[i] as i32) - (b[i] as i32);
        total += diff * diff;
    }

    total as f32
}

// Helper functions

fn euclidean_squared(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(&x, &y)| {
            let diff = x - y;
            diff * diff
        })
        .sum()
}

fn kmeans_clustering(vectors: &[Vec<f32>], k: usize, iterations: usize) -> Vec<Vec<f32>> {
    use rand::seq::SliceRandom;
    use rand::thread_rng;

    let mut rng = thread_rng();

    // Initialize centroids randomly
    let mut centroids: Vec<Vec<f32>> = vectors.choose_multiple(&mut rng, k).cloned().collect();

    for _ in 0..iterations {
        // Assign vectors to nearest centroid
        let mut assignments = vec![Vec::new(); k];

        for vector in vectors {
            let nearest = centroids
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    let dist_a = euclidean_squared(vector, a);
                    let dist_b = euclidean_squared(vector, b);
                    dist_a.partial_cmp(&dist_b).unwrap()
                })
                .map(|(idx, _)| idx)
                .unwrap_or(0);

            assignments[nearest].push(vector.clone());
        }

        // Update centroids
        for (centroid, assigned) in centroids.iter_mut().zip(&assignments) {
            if !assigned.is_empty() {
                let dim = centroid.len();
                *centroid = vec![0.0; dim];

                for vector in assigned {
                    for (i, &v) in vector.iter().enumerate() {
                        centroid[i] += v;
                    }
                }

                let count = assigned.len() as f32;
                for v in centroid.iter_mut() {
                    *v /= count;
                }
            }
        }
    }

    centroids
}

// =============================================================================
// SIMD-Optimized Distance Calculations for Quantized Vectors
// =============================================================================

// NOTE: scalar_distance_scalar is already defined above (lines 404-425)
// NOTE: scalar_distance_neon is already defined above (lines 430-473)
// NOTE: scalar_distance_avx2 is already defined above (lines 479-540)
// This section uses the existing implementations for consistency

/// SIMD-optimized hamming distance using popcnt
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "popcnt")]
#[inline]
unsafe fn hamming_distance_simd_x86(a: &[u8], b: &[u8]) -> u32 {
    use std::arch::x86_64::*;

    let mut distance = 0u64;

    // Process 8 bytes at a time using u64 with hardware popcnt
    let chunks_a = a.chunks_exact(8);
    let chunks_b = b.chunks_exact(8);
    let remainder_a = chunks_a.remainder();
    let remainder_b = chunks_b.remainder();

    for (chunk_a, chunk_b) in chunks_a.zip(chunks_b) {
        let a_u64 = u64::from_le_bytes(chunk_a.try_into().unwrap());
        let b_u64 = u64::from_le_bytes(chunk_b.try_into().unwrap());
        distance += _popcnt64((a_u64 ^ b_u64) as i64) as u64;
    }

    // Handle remainder
    for (&a_byte, &b_byte) in remainder_a.iter().zip(remainder_b) {
        distance += (a_byte ^ b_byte).count_ones() as u64;
    }

    distance as u32
}

/// NEON-optimized hamming distance for ARM64
///
/// # Safety
/// Caller must ensure a.len() == b.len()
#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn hamming_distance_neon(a: &[u8], b: &[u8]) -> u32 {
    use std::arch::aarch64::*;

    let len = a.len();
    let a_ptr = a.as_ptr();
    let b_ptr = b.as_ptr();

    let chunks = len / 16;
    let mut idx = 0usize;

    let mut sum = vdupq_n_u8(0);

    for _ in 0..chunks {
        // Load 16 bytes
        let a_vec = vld1q_u8(a_ptr.add(idx));
        let b_vec = vld1q_u8(b_ptr.add(idx));

        // XOR and count bits using vcntq_u8 (population count)
        let xor_result = veorq_u8(a_vec, b_vec);
        let bits = vcntq_u8(xor_result);

        // Accumulate
        sum = vaddq_u8(sum, bits);

        idx += 16;
    }

    // Horizontal sum
    let sum_val = vaddvq_u8(sum) as u32;

    // Handle remainder with bounds-check elimination
    let mut remainder_sum = 0u32;
    let start = chunks * 16;
    for i in start..len {
        remainder_sum += (*a.get_unchecked(i) ^ *b.get_unchecked(i)).count_ones();
    }

    sum_val + remainder_sum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scalar_quantization() {
        let vector = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let quantized = ScalarQuantized::quantize(&vector);
        let reconstructed = quantized.reconstruct();

        // Check approximate reconstruction
        for (orig, recon) in vector.iter().zip(&reconstructed) {
            assert!((orig - recon).abs() < 0.1);
        }
    }

    #[test]
    fn test_binary_quantization() {
        let vector = vec![1.0, -1.0, 2.0, -2.0, 0.5];
        let quantized = BinaryQuantized::quantize(&vector);

        assert_eq!(quantized.dimensions, 5);
        assert_eq!(quantized.bits.len(), 1); // 5 bits fit in 1 byte
    }

    #[test]
    fn test_binary_distance() {
        let v1 = vec![1.0, 1.0, 1.0, 1.0];
        let v2 = vec![1.0, 1.0, -1.0, -1.0];

        let q1 = BinaryQuantized::quantize(&v1);
        let q2 = BinaryQuantized::quantize(&v2);

        let dist = q1.distance(&q2);
        assert_eq!(dist, 2.0); // 2 bits differ
    }

    #[test]
    fn test_scalar_quantization_roundtrip() {
        // Test that quantize -> reconstruct produces values close to original
        let test_vectors = vec![
            vec![1.0, 2.0, 3.0, 4.0, 5.0],
            vec![-10.0, -5.0, 0.0, 5.0, 10.0],
            vec![0.1, 0.2, 0.3, 0.4, 0.5],
            vec![100.0, 200.0, 300.0, 400.0, 500.0],
        ];

        for vector in test_vectors {
            let quantized = ScalarQuantized::quantize(&vector);
            let reconstructed = quantized.reconstruct();

            assert_eq!(vector.len(), reconstructed.len());

            for (orig, recon) in vector.iter().zip(reconstructed.iter()) {
                // With 8-bit quantization, max error is roughly (max-min)/255
                let max = vector.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                let min = vector.iter().copied().fold(f32::INFINITY, f32::min);
                let max_error = (max - min) / 255.0 * 2.0; // Allow 2x for rounding

                assert!(
                    (orig - recon).abs() < max_error,
                    "Roundtrip error too large: orig={}, recon={}, error={}",
                    orig,
                    recon,
                    (orig - recon).abs()
                );
            }
        }
    }

    #[test]
    fn test_scalar_distance_symmetry() {
        // Test that distance(a, b) == distance(b, a)
        let v1 = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let v2 = vec![2.0, 3.0, 4.0, 5.0, 6.0];

        let q1 = ScalarQuantized::quantize(&v1);
        let q2 = ScalarQuantized::quantize(&v2);

        let dist_ab = q1.distance(&q2);
        let dist_ba = q2.distance(&q1);

        // Distance should be symmetric (within floating point precision)
        assert!(
            (dist_ab - dist_ba).abs() < 0.01,
            "Distance is not symmetric: d(a,b)={}, d(b,a)={}",
            dist_ab,
            dist_ba
        );
    }

    #[test]
    fn test_scalar_distance_different_scales() {
        // Test distance calculation with vectors that have different scales
        let v1 = vec![1.0, 2.0, 3.0, 4.0, 5.0]; // range: 4.0
        let v2 = vec![10.0, 20.0, 30.0, 40.0, 50.0]; // range: 40.0

        let q1 = ScalarQuantized::quantize(&v1);
        let q2 = ScalarQuantized::quantize(&v2);

        let dist_ab = q1.distance(&q2);
        let dist_ba = q2.distance(&q1);

        // With average scaling, symmetry should be maintained
        assert!(
            (dist_ab - dist_ba).abs() < 0.01,
            "Distance with different scales not symmetric: d(a,b)={}, d(b,a)={}",
            dist_ab,
            dist_ba
        );
    }

    #[test]
    fn test_scalar_quantization_edge_cases() {
        // Test with all same values
        let same_values = vec![5.0, 5.0, 5.0, 5.0];
        let quantized = ScalarQuantized::quantize(&same_values);
        let reconstructed = quantized.reconstruct();

        for (orig, recon) in same_values.iter().zip(reconstructed.iter()) {
            assert!((orig - recon).abs() < 0.01);
        }

        // Test with extreme ranges
        let extreme = vec![f32::MIN / 1e10, 0.0, f32::MAX / 1e10];
        let quantized = ScalarQuantized::quantize(&extreme);
        let reconstructed = quantized.reconstruct();

        assert_eq!(extreme.len(), reconstructed.len());
    }

    #[test]
    fn test_binary_distance_symmetry() {
        // Test that binary distance is symmetric
        let v1 = vec![1.0, -1.0, 1.0, -1.0];
        let v2 = vec![1.0, 1.0, -1.0, -1.0];

        let q1 = BinaryQuantized::quantize(&v1);
        let q2 = BinaryQuantized::quantize(&v2);

        let dist_ab = q1.distance(&q2);
        let dist_ba = q2.distance(&q1);

        assert_eq!(
            dist_ab, dist_ba,
            "Binary distance not symmetric: d(a,b)={}, d(b,a)={}",
            dist_ab, dist_ba
        );
    }

    #[test]
    fn test_int4_quantization() {
        let vector = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let quantized = Int4Quantized::quantize(&vector);
        let reconstructed = quantized.reconstruct();

        assert_eq!(quantized.dimensions, 5);
        // 5 dimensions = 3 bytes (2 per byte, last byte has 1)
        assert_eq!(quantized.data.len(), 3);

        // Check approximate reconstruction
        for (orig, recon) in vector.iter().zip(&reconstructed) {
            // With 4-bit quantization, max error is roughly (max-min)/15
            let max_error = (5.0 - 1.0) / 15.0 * 2.0;
            assert!(
                (orig - recon).abs() < max_error,
                "Int4 roundtrip error too large: orig={}, recon={}",
                orig,
                recon
            );
        }
    }

    #[test]
    fn test_int4_distance() {
        // Use vectors with different quantized patterns
        // v1 spans [0.0, 15.0] -> quantizes to [0, 1, 2, ..., 15] (linear mapping)
        // v2 spans [0.0, 15.0] but with different distribution
        let v1 = vec![0.0, 5.0, 10.0, 15.0];
        let v2 = vec![0.0, 3.0, 12.0, 15.0]; // Different middle values

        let q1 = Int4Quantized::quantize(&v1);
        let q2 = Int4Quantized::quantize(&v2);

        let dist = q1.distance(&q2);
        // The quantized values differ in the middle, so distance should be positive
        assert!(
            dist > 0.0,
            "Distance should be positive, got {}. q1.data={:?}, q2.data={:?}",
            dist,
            q1.data,
            q2.data
        );
    }

    #[test]
    fn test_int4_distance_symmetry() {
        let v1 = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let v2 = vec![2.0, 3.0, 4.0, 5.0, 6.0];

        let q1 = Int4Quantized::quantize(&v1);
        let q2 = Int4Quantized::quantize(&v2);

        let dist_ab = q1.distance(&q2);
        let dist_ba = q2.distance(&q1);

        assert!(
            (dist_ab - dist_ba).abs() < 0.01,
            "Int4 distance not symmetric: d(a,b)={}, d(b,a)={}",
            dist_ab,
            dist_ba
        );
    }

    #[test]
    fn test_int4_compression_ratio() {
        assert_eq!(Int4Quantized::compression_ratio(), 8.0);
    }

    #[test]
    fn test_binary_fast_hamming() {
        // Test fast hamming distance with various sizes
        let a = vec![0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xAA];
        let b = vec![0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x55];

        let distance = BinaryQuantized::hamming_distance_fast(&a, &b);
        // All bits differ: 9 bytes * 8 bits = 72 bits
        assert_eq!(distance, 72);
    }

    #[test]
    fn test_binary_similarity() {
        let v1 = vec![1.0; 8]; // All positive
        let v2 = vec![1.0; 8]; // Same

        let q1 = BinaryQuantized::quantize(&v1);
        let q2 = BinaryQuantized::quantize(&v2);

        let sim = q1.similarity(&q2);
        assert!(
            (sim - 1.0).abs() < 0.001,
            "Same vectors should have similarity 1.0"
        );
    }

    #[test]
    fn test_binary_compression_ratio() {
        assert_eq!(BinaryQuantized::compression_ratio(), 32.0);
    }
}
