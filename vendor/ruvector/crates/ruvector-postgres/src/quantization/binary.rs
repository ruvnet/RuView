//! Binary Quantization
//!
//! Compresses vectors to 1 bit per dimension, achieving 32x memory reduction.
//! Uses Hamming distance for fast comparison.

/// Quantize f32 vector to binary (1 bit per dimension)
///
/// Positive values -> 1, negative/zero values -> 0
pub fn quantize(vector: &[f32]) -> Vec<u8> {
    let n_bytes = (vector.len() + 7) / 8;
    let mut result = vec![0u8; n_bytes];

    for (i, &v) in vector.iter().enumerate() {
        if v > 0.0 {
            let byte_idx = i / 8;
            let bit_idx = i % 8;
            result[byte_idx] |= 1 << bit_idx;
        }
    }

    result
}

/// Quantize with threshold
pub fn quantize_with_threshold(vector: &[f32], threshold: f32) -> Vec<u8> {
    let n_bytes = (vector.len() + 7) / 8;
    let mut result = vec![0u8; n_bytes];

    for (i, &v) in vector.iter().enumerate() {
        if v > threshold {
            let byte_idx = i / 8;
            let bit_idx = i % 8;
            result[byte_idx] |= 1 << bit_idx;
        }
    }

    result
}

/// Calculate Hamming distance between binary vectors
pub fn hamming_distance(a: &[u8], b: &[u8]) -> u32 {
    debug_assert_eq!(a.len(), b.len());

    a.iter()
        .zip(b.iter())
        .map(|(&x, &y)| (x ^ y).count_ones())
        .sum()
}

/// SIMD-optimized Hamming distance using POPCNT
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "popcnt")]
unsafe fn hamming_distance_popcnt(a: &[u8], b: &[u8]) -> u32 {
    use std::arch::x86_64::*;

    let n = a.len();
    let mut count = 0u32;

    // Process 8 bytes at a time
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

/// AVX2-optimized Hamming distance using vpshufb popcount
///
/// Uses the SWAR (SIMD Within A Register) technique with lookup tables.
/// Processes 32 bytes per iteration, which is 4x faster than scalar POPCNT
/// for large vectors (1024+ dimensions).
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn hamming_distance_avx2(a: &[u8], b: &[u8]) -> u32 {
    use std::arch::x86_64::*;

    let n = a.len();

    // Lookup table for popcount of 4-bit values
    let lookup = _mm256_setr_epi8(
        0, 1, 1, 2, 1, 2, 2, 3, 1, 2, 2, 3, 2, 3, 3, 4, 0, 1, 1, 2, 1, 2, 2, 3, 1, 2, 2, 3, 2, 3,
        3, 4,
    );
    let low_mask = _mm256_set1_epi8(0x0F);

    let mut total = _mm256_setzero_si256();

    // Process 32 bytes at a time
    let chunks = n / 32;
    for i in 0..chunks {
        let offset = i * 32;
        let va = _mm256_loadu_si256(a.as_ptr().add(offset) as *const __m256i);
        let vb = _mm256_loadu_si256(b.as_ptr().add(offset) as *const __m256i);

        // XOR the vectors
        let xor = _mm256_xor_si256(va, vb);

        // Split into low and high nibbles
        let lo = _mm256_and_si256(xor, low_mask);
        let hi = _mm256_and_si256(_mm256_srli_epi16(xor, 4), low_mask);

        // Lookup popcount for each nibble
        let popcnt_lo = _mm256_shuffle_epi8(lookup, lo);
        let popcnt_hi = _mm256_shuffle_epi8(lookup, hi);

        // Sum nibble popcounts
        let popcnt = _mm256_add_epi8(popcnt_lo, popcnt_hi);

        // Accumulate using sad (sum of absolute differences from zero)
        let sad = _mm256_sad_epu8(popcnt, _mm256_setzero_si256());
        total = _mm256_add_epi64(total, sad);
    }

    // Horizontal sum of the 4 64-bit values
    let sum128_lo = _mm256_castsi256_si128(total);
    let sum128_hi = _mm256_extracti128_si256(total, 1);
    let sum128 = _mm_add_epi64(sum128_lo, sum128_hi);
    let sum64 = _mm_add_epi64(sum128, _mm_srli_si128(sum128, 8));
    let mut count = _mm_cvtsi128_si64(sum64) as u32;

    // Handle remainder with scalar POPCNT
    for i in (chunks * 32)..n {
        count += (a[i] ^ b[i]).count_ones();
    }

    count
}

/// Calculate Hamming distance with SIMD optimization
///
/// Automatically selects the best implementation:
/// - AVX2 vpshufb for large vectors (>= 128 bytes / 1024 bits)
/// - POPCNT for medium vectors (>= 8 bytes)
/// - Scalar for small vectors
pub fn hamming_distance_simd(a: &[u8], b: &[u8]) -> u32 {
    #[cfg(target_arch = "x86_64")]
    {
        let n = a.len();

        // For large vectors, AVX2 vpshufb is fastest
        if n >= 128 && is_x86_feature_detected!("avx2") {
            return unsafe { hamming_distance_avx2(a, b) };
        }

        // For medium vectors, use POPCNT
        if is_x86_feature_detected!("popcnt") {
            return unsafe { hamming_distance_popcnt(a, b) };
        }
    }

    hamming_distance(a, b)
}

/// Normalize Hamming distance to [0, 1] range
pub fn normalized_hamming_distance(a: &[u8], b: &[u8], dimensions: usize) -> f32 {
    let dist = hamming_distance_simd(a, b);
    dist as f32 / dimensions as f32
}

/// Binary quantized vector
#[derive(Debug, Clone)]
pub struct BinaryQuantizedVector {
    pub data: Vec<u8>,
    pub dimensions: usize,
}

impl BinaryQuantizedVector {
    /// Create from f32 vector
    pub fn from_f32(vector: &[f32]) -> Self {
        Self {
            data: quantize(vector),
            dimensions: vector.len(),
        }
    }

    /// Create from f32 vector with threshold
    pub fn from_f32_threshold(vector: &[f32], threshold: f32) -> Self {
        Self {
            data: quantize_with_threshold(vector, threshold),
            dimensions: vector.len(),
        }
    }

    /// Calculate Hamming distance to another binary vector
    pub fn hamming_distance(&self, other: &Self) -> u32 {
        debug_assert_eq!(self.dimensions, other.dimensions);
        hamming_distance_simd(&self.data, &other.data)
    }

    /// Calculate normalized distance [0, 1]
    pub fn normalized_distance(&self, other: &Self) -> f32 {
        self.hamming_distance(other) as f32 / self.dimensions as f32
    }

    /// Memory size in bytes
    pub fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.data.len()
    }

    /// Compression ratio compared to f32
    pub fn compression_ratio(&self) -> f32 {
        32.0 // f32 (32 bits) -> 1 bit
    }

    /// Get bit at position
    pub fn get_bit(&self, pos: usize) -> bool {
        debug_assert!(pos < self.dimensions);
        let byte_idx = pos / 8;
        let bit_idx = pos % 8;
        (self.data[byte_idx] >> bit_idx) & 1 == 1
    }

    /// Count number of 1 bits
    pub fn popcount(&self) -> u32 {
        self.data.iter().map(|&b| b.count_ones()).sum()
    }
}

/// Two-stage search with binary quantization
///
/// 1. Fast Hamming distance filtering using binary vectors
/// 2. Rerank top candidates with full precision distance
pub struct BinarySearcher {
    /// Binary quantized vectors
    binary_vectors: Vec<BinaryQuantizedVector>,
    /// Original vectors for reranking
    original_vectors: Vec<Vec<f32>>,
    /// Rerank factor (rerank top k * factor candidates)
    rerank_factor: usize,
}

impl BinarySearcher {
    /// Create a new binary searcher
    pub fn new(vectors: Vec<Vec<f32>>, rerank_factor: usize) -> Self {
        let binary_vectors: Vec<_> = vectors
            .iter()
            .map(|v| BinaryQuantizedVector::from_f32(v))
            .collect();

        Self {
            binary_vectors,
            original_vectors: vectors,
            rerank_factor,
        }
    }

    /// Search for k nearest neighbors
    pub fn search(&self, query: &[f32], k: usize) -> Vec<(usize, f32)> {
        let query_binary = BinaryQuantizedVector::from_f32(query);

        // Stage 1: Fast Hamming distance search
        let mut candidates: Vec<(usize, u32)> = self
            .binary_vectors
            .iter()
            .enumerate()
            .map(|(i, bv)| (i, query_binary.hamming_distance(bv)))
            .collect();

        // Sort by Hamming distance
        candidates.sort_by_key(|(_, d)| *d);

        // Take top k * rerank_factor candidates
        let n_candidates = (k * self.rerank_factor).min(candidates.len());
        let top_candidates: Vec<usize> = candidates
            .iter()
            .take(n_candidates)
            .map(|(i, _)| *i)
            .collect();

        // Stage 2: Rerank with full precision distance
        let mut reranked: Vec<(usize, f32)> = top_candidates
            .iter()
            .map(|&i| {
                let dist: f32 = query
                    .iter()
                    .zip(self.original_vectors[i].iter())
                    .map(|(a, b)| (a - b).powi(2))
                    .sum::<f32>()
                    .sqrt();
                (i, dist)
            })
            .collect();

        reranked.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        reranked.truncate(k);
        reranked
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantize() {
        let v = vec![0.5, -0.3, 0.1, -0.8, 0.2, -0.1, 0.9, -0.5];
        let q = quantize(&v);

        assert_eq!(q.len(), 1);
        // Bits: 1, 0, 1, 0, 1, 0, 1, 0 = 0b01010101 = 85
        assert_eq!(q[0], 0b01010101);
    }

    #[test]
    fn test_hamming_distance() {
        let a = vec![0b11110000];
        let b = vec![0b10101010];
        // XOR: 0b01011010, popcount = 4
        assert_eq!(hamming_distance(&a, &b), 4);
    }

    #[test]
    fn test_compression_ratio() {
        let v = BinaryQuantizedVector::from_f32(&vec![0.0; 1024]);
        assert_eq!(v.compression_ratio(), 32.0);
        assert_eq!(v.data.len(), 128); // 1024 bits = 128 bytes
    }

    #[test]
    fn test_simd_matches_scalar() {
        let a: Vec<u8> = (0..128).collect();
        let b: Vec<u8> = (0..128).map(|i| 255 - i).collect();

        let scalar = hamming_distance(&a, &b);
        let simd = hamming_distance_simd(&a, &b);

        assert_eq!(scalar, simd);
    }

    #[test]
    fn test_binary_searcher() {
        let vectors: Vec<Vec<f32>> = (0..100)
            .map(|i| vec![i as f32 * 0.1, (100 - i) as f32 * 0.1, 0.5])
            .collect();

        let searcher = BinarySearcher::new(vectors.clone(), 4);

        let query = vec![5.0, 5.0, 0.5];
        let results = searcher.search(&query, 5);

        assert_eq!(results.len(), 5);
        // Results should be ordered by distance
        for i in 1..results.len() {
            assert!(results[i].1 >= results[i - 1].1);
        }
    }

    #[test]
    fn test_get_bit() {
        let v = vec![1.0, -1.0, 1.0, -1.0];
        let bv = BinaryQuantizedVector::from_f32(&v);

        assert!(bv.get_bit(0));
        assert!(!bv.get_bit(1));
        assert!(bv.get_bit(2));
        assert!(!bv.get_bit(3));
    }
}
