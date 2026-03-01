//! Binary Quantization - 32x Memory Compression

use heapless::Vec as HVec;

pub const MAX_BINARY_SIZE: usize = 64;

/// Binary quantized vector - 1 bit per dimension
#[derive(Debug, Clone)]
pub struct BinaryVector<const N: usize> {
    pub data: HVec<u8, N>,
    pub dim: usize,
    pub threshold: i8,
}

impl<const N: usize> BinaryVector<N> {
    pub fn from_i8(values: &[i8], threshold: i8) -> crate::Result<Self> {
        let dim = values.len();
        let num_bytes = (dim + 7) / 8;
        if num_bytes > N {
            return Err(crate::Error::BufferOverflow);
        }

        let mut data = HVec::new();
        for chunk_idx in 0..num_bytes {
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

    pub fn num_bytes(&self) -> usize { self.data.len() }
    pub fn compression_ratio(&self) -> f32 { self.dim as f32 / self.data.len() as f32 }
}

/// Binary embedding table (32x smaller than INT8)
pub struct BinaryEmbedding<const VOCAB: usize, const DIM_BYTES: usize> {
    data: HVec<u8, { 32 * 1024 }>,
    vocab_size: usize,
    dim: usize,
    bytes_per_embed: usize,
}

impl<const VOCAB: usize, const DIM_BYTES: usize> BinaryEmbedding<VOCAB, DIM_BYTES> {
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

        Ok(Self { data, vocab_size, dim, bytes_per_embed })
    }

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

    pub fn memory_size(&self) -> usize { self.data.len() }
}

/// Hamming distance between binary vectors (POPCNT)
#[inline]
pub fn hamming_distance(a: &[u8], b: &[u8]) -> u32 {
    let mut distance: u32 = 0;
    let chunks = a.len() / 4;
    for i in 0..chunks {
        let idx = i * 4;
        distance += popcount8(a[idx] ^ b[idx]) + popcount8(a[idx + 1] ^ b[idx + 1])
                  + popcount8(a[idx + 2] ^ b[idx + 2]) + popcount8(a[idx + 3] ^ b[idx + 3]);
    }
    for i in (chunks * 4)..a.len() {
        distance += popcount8(a[i] ^ b[i]);
    }
    distance
}

#[inline]
pub fn hamming_similarity(a: &[u8], b: &[u8]) -> f32 {
    let total_bits = (a.len() * 8) as f32;
    1.0 - (hamming_distance(a, b) as f32 / total_bits)
}

#[inline]
pub fn popcount8(x: u8) -> u32 {
    const TABLE: [u8; 256] = [
        0,1,1,2,1,2,2,3,1,2,2,3,2,3,3,4,1,2,2,3,2,3,3,4,2,3,3,4,3,4,4,5,
        1,2,2,3,2,3,3,4,2,3,3,4,3,4,4,5,2,3,3,4,3,4,4,5,3,4,4,5,4,5,5,6,
        1,2,2,3,2,3,3,4,2,3,3,4,3,4,4,5,2,3,3,4,3,4,4,5,3,4,4,5,4,5,5,6,
        2,3,3,4,3,4,4,5,3,4,4,5,4,5,5,6,3,4,4,5,4,5,5,6,4,5,5,6,5,6,6,7,
        1,2,2,3,2,3,3,4,2,3,3,4,3,4,4,5,2,3,3,4,3,4,4,5,3,4,4,5,4,5,5,6,
        2,3,3,4,3,4,4,5,3,4,4,5,4,5,5,6,3,4,4,5,4,5,5,6,4,5,5,6,5,6,6,7,
        2,3,3,4,3,4,4,5,3,4,4,5,4,5,5,6,3,4,4,5,4,5,5,6,4,5,5,6,5,6,6,7,
        3,4,4,5,4,5,5,6,4,5,5,6,5,6,6,7,4,5,5,6,5,6,6,7,5,6,6,7,6,7,7,8,
    ];
    TABLE[x as usize] as u32
}

/// XNOR-popcount for binary neural network inference
#[inline]
pub fn xnor_popcount(a: &[u8], b: &[u8]) -> i32 {
    let total_bits = (a.len() * 8) as i32;
    let mut matching: i32 = 0;
    for (&x, &y) in a.iter().zip(b.iter()) {
        matching += popcount8(!(x ^ y)) as i32;
    }
    2 * matching - total_bits
}
