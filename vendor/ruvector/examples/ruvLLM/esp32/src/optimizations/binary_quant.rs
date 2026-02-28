//! Binary Quantization - 32x Memory Compression
//!
//! Adapted from ruvector-postgres/src/quantization/binary.rs
//! Converts f32/i8 vectors to 1-bit per dimension with Hamming distance.

use heapless::Vec as HVec;

/// Maximum binary vector size in bytes (supports up to 512 dimensions)
pub const MAX_BINARY_SIZE: usize = 64;

/// Binary quantized vector - 1 bit per dimension
#[derive(Debug, Clone)]
pub struct BinaryVector<const N: usize> {
    /// Packed binary data (8 dimensions per byte)
    pub data: HVec<u8, N>,
    /// Original dimension count
    pub dim: usize,
    /// Threshold used for binarization
    pub threshold: i8,
}

impl<const N: usize> BinaryVector<N> {
    /// Create binary vector from INT8 values
    /// Values >= threshold become 1, values < threshold become 0
    pub fn from_i8(values: &[i8], threshold: i8) -> crate::Result<Self> {
        let dim = values.len();
        let num_bytes = (dim + 7) / 8;

        if num_bytes > N {
            return Err(crate::Error::BufferOverflow);
        }

        let mut data = HVec::new();

        for chunk_idx in 0..(num_bytes) {
            let mut byte = 0u8;
            for bit_idx in 0..8 {
                let val_idx = chunk_idx * 8 + bit_idx;
                if val_idx < dim && values[val_idx] >= threshold {
                    byte |= 1 << bit_idx;
                }
            }
            data.push(byte).map_err(|_| crate::Error::BufferOverflow)?;
        }

        Ok(Self { data, dim, threshold })
    }

    /// Create binary vector from f32 values (for host-side quantization)
    #[cfg(feature = "host-test")]
    pub fn from_f32(values: &[f32], threshold: f32) -> crate::Result<Self> {
        let i8_threshold = (threshold * 127.0) as i8;
        let i8_values: heapless::Vec<i8, 512> = values
            .iter()
            .map(|&v| (v * 127.0).clamp(-128.0, 127.0) as i8)
            .collect();
        Self::from_i8(&i8_values, i8_threshold)
    }

    /// Get number of packed bytes
    pub fn num_bytes(&self) -> usize {
        self.data.len()
    }

    /// Memory savings compared to INT8
    pub fn compression_ratio(&self) -> f32 {
        self.dim as f32 / self.data.len() as f32
    }
}

/// Binary embedding table for vocabulary (32x smaller than INT8)
pub struct BinaryEmbedding<const VOCAB: usize, const DIM_BYTES: usize> {
    /// Packed binary embeddings [VOCAB * DIM_BYTES]
    data: HVec<u8, { 32 * 1024 }>, // Max 32KB
    /// Vocabulary size
    vocab_size: usize,
    /// Dimensions (in bits)
    dim: usize,
    /// Bytes per embedding
    bytes_per_embed: usize,
}

impl<const VOCAB: usize, const DIM_BYTES: usize> BinaryEmbedding<VOCAB, DIM_BYTES> {
    /// Create random binary embeddings for testing
    pub fn random(vocab_size: usize, dim: usize, seed: u32) -> crate::Result<Self> {
        let bytes_per_embed = (dim + 7) / 8;
        let total_bytes = vocab_size * bytes_per_embed;

        let mut data = HVec::new();
        let mut rng_state = seed;

        for _ in 0..total_bytes {
            rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
            let byte = ((rng_state >> 16) & 0xFF) as u8;
            data.push(byte).map_err(|_| crate::Error::BufferOverflow)?;
        }

        Ok(Self {
            data,
            vocab_size,
            dim,
            bytes_per_embed,
        })
    }

    /// Look up binary embedding for a token
    pub fn lookup(&self, token_id: u16, output: &mut [u8]) -> crate::Result<()> {
        let id = token_id as usize;
        if id >= self.vocab_size {
            return Err(crate::Error::InvalidModel("Token ID out of range"));
        }

        let start = id * self.bytes_per_embed;
        let end = start + self.bytes_per_embed;

        if output.len() < self.bytes_per_embed {
            return Err(crate::Error::BufferOverflow);
        }

        output[..self.bytes_per_embed].copy_from_slice(&self.data[start..end]);
        Ok(())
    }

    /// Memory size in bytes
    pub fn memory_size(&self) -> usize {
        self.data.len()
    }

    /// Compression vs INT8 embedding of same dimensions
    pub fn compression_vs_int8(&self) -> f32 {
        8.0 // 8 bits per dimension -> 1 bit per dimension = 8x
    }
}

/// Hamming distance between two binary vectors
///
/// Counts the number of differing bits. Uses POPCNT-like operations.
/// On ESP32, this is extremely fast as it uses simple bitwise operations.
#[inline]
pub fn hamming_distance(a: &[u8], b: &[u8]) -> u32 {
    debug_assert_eq!(a.len(), b.len());

    let mut distance: u32 = 0;

    // Process 4 bytes at a time for better performance
    let chunks = a.len() / 4;
    for i in 0..chunks {
        let idx = i * 4;
        let xor0 = a[idx] ^ b[idx];
        let xor1 = a[idx + 1] ^ b[idx + 1];
        let xor2 = a[idx + 2] ^ b[idx + 2];
        let xor3 = a[idx + 3] ^ b[idx + 3];

        distance += popcount8(xor0) + popcount8(xor1) + popcount8(xor2) + popcount8(xor3);
    }

    // Handle remainder
    for i in (chunks * 4)..a.len() {
        distance += popcount8(a[i] ^ b[i]);
    }

    distance
}

/// Hamming similarity (inverted distance, normalized to 0-1 range)
#[inline]
pub fn hamming_similarity(a: &[u8], b: &[u8]) -> f32 {
    let total_bits = (a.len() * 8) as f32;
    let distance = hamming_distance(a, b) as f32;
    1.0 - (distance / total_bits)
}

/// Hamming similarity as fixed-point (0-255 range)
#[inline]
pub fn hamming_similarity_fixed(a: &[u8], b: &[u8]) -> u8 {
    let total_bits = (a.len() * 8) as u32;
    let matching_bits = total_bits - hamming_distance(a, b);
    ((matching_bits * 255) / total_bits) as u8
}

/// Population count for a single byte (count of 1 bits)
/// Uses lookup table for ESP32 efficiency
#[inline]
pub fn popcount8(x: u8) -> u32 {
    // Lookup table for byte population count
    const POPCOUNT_TABLE: [u8; 256] = [
        0, 1, 1, 2, 1, 2, 2, 3, 1, 2, 2, 3, 2, 3, 3, 4,
        1, 2, 2, 3, 2, 3, 3, 4, 2, 3, 3, 4, 3, 4, 4, 5,
        1, 2, 2, 3, 2, 3, 3, 4, 2, 3, 3, 4, 3, 4, 4, 5,
        2, 3, 3, 4, 3, 4, 4, 5, 3, 4, 4, 5, 4, 5, 5, 6,
        1, 2, 2, 3, 2, 3, 3, 4, 2, 3, 3, 4, 3, 4, 4, 5,
        2, 3, 3, 4, 3, 4, 4, 5, 3, 4, 4, 5, 4, 5, 5, 6,
        2, 3, 3, 4, 3, 4, 4, 5, 3, 4, 4, 5, 4, 5, 5, 6,
        3, 4, 4, 5, 4, 5, 5, 6, 4, 5, 5, 6, 5, 6, 6, 7,
        1, 2, 2, 3, 2, 3, 3, 4, 2, 3, 3, 4, 3, 4, 4, 5,
        2, 3, 3, 4, 3, 4, 4, 5, 3, 4, 4, 5, 4, 5, 5, 6,
        2, 3, 3, 4, 3, 4, 4, 5, 3, 4, 4, 5, 4, 5, 5, 6,
        3, 4, 4, 5, 4, 5, 5, 6, 4, 5, 5, 6, 5, 6, 6, 7,
        2, 3, 3, 4, 3, 4, 4, 5, 3, 4, 4, 5, 4, 5, 5, 6,
        3, 4, 4, 5, 4, 5, 5, 6, 4, 5, 5, 6, 5, 6, 6, 7,
        3, 4, 4, 5, 4, 5, 5, 6, 4, 5, 5, 6, 5, 6, 6, 7,
        4, 5, 5, 6, 5, 6, 6, 7, 5, 6, 6, 7, 6, 7, 7, 8,
    ];
    POPCOUNT_TABLE[x as usize] as u32
}

/// XNOR-popcount for binary neural network inference
/// Equivalent to computing dot product of {-1, +1} vectors
#[inline]
pub fn xnor_popcount(a: &[u8], b: &[u8]) -> i32 {
    debug_assert_eq!(a.len(), b.len());

    let total_bits = (a.len() * 8) as i32;
    let mut matching: i32 = 0;

    for (&x, &y) in a.iter().zip(b.iter()) {
        // XNOR: same bits = 1, different bits = 0
        let xnor = !(x ^ y);
        matching += popcount8(xnor) as i32;
    }

    // Convert to {-1, +1} dot product equivalent
    // matching bits contribute +1, non-matching contribute -1
    // result = 2 * matching - total_bits
    2 * matching - total_bits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binary_quantization() {
        let values = [10i8, -5, 20, -10, 0, 15, -8, 30];
        let binary = BinaryVector::<8>::from_i8(&values, 0).unwrap();

        assert_eq!(binary.dim, 8);
        assert_eq!(binary.num_bytes(), 1);

        // Expected: bits where value >= 0: positions 0, 2, 4, 5, 7
        // Binary: 10110101 = 0xB5
        assert_eq!(binary.data[0], 0b10110101);
    }

    #[test]
    fn test_hamming_distance() {
        let a = [0b11110000u8, 0b10101010];
        let b = [0b11110000u8, 0b10101010];
        assert_eq!(hamming_distance(&a, &b), 0);

        let c = [0b00001111u8, 0b01010101];
        assert_eq!(hamming_distance(&a, &c), 16); // All bits different
    }

    #[test]
    fn test_xnor_popcount() {
        let a = [0b11111111u8];
        let b = [0b11111111u8];
        // Perfect match: 8 matching bits -> 2*8 - 8 = 8
        assert_eq!(xnor_popcount(&a, &b), 8);

        let c = [0b00000000u8];
        // Complete mismatch: 0 matching bits -> 2*0 - 8 = -8
        assert_eq!(xnor_popcount(&a, &c), -8);
    }

    #[test]
    fn test_compression_ratio() {
        let values = [0i8; 64];
        let binary = BinaryVector::<8>::from_i8(&values, 0).unwrap();
        assert_eq!(binary.compression_ratio(), 8.0);
    }
}
