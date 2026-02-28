//! Ternary Tensor Data Structure
//!
//! This module provides the `TernaryTensor` container for BitNet b1.58 ternary weights,
//! along with efficient 2-bit packing/unpacking functions.

/// Ternary tensor with 2-bit packed representation.
///
/// Stores ternary weights {-1, 0, +1} in a compact 2-bit format:
/// - 00 = -1
/// - 01 = 0
/// - 10 = +1
/// - 11 = reserved (unused)
///
/// Each block of `block_size` elements shares a single FP32 scale factor
/// derived from the absmean quantization process.
///
/// # Memory Layout
///
/// For a tensor with shape (m, n) and block_size B:
/// - `packed_data`: ceil(m*n / 4) bytes (4 ternary values per byte)
/// - `scales`: ceil(m*n / B) * 4 bytes (one FP32 scale per block)
///
/// # Example
///
/// ```rust,ignore
/// use ruvllm::bitnet::TernaryTensor;
///
/// let tensor = TernaryTensor {
///     packed_data: vec![0b10010100],  // [+1, 0, +1, 0]
///     scales: vec![0.5],
///     shape: (2, 2),
///     block_size: 256,
/// };
///
/// println!("Sparsity: {:.2}%", tensor.sparsity() * 100.0);
/// println!("Memory: {} bytes", tensor.memory_bytes());
/// ```
#[derive(Debug, Clone)]
pub struct TernaryTensor {
    /// Packed 2-bit ternary data (4 values per byte)
    pub packed_data: Vec<u8>,
    /// Per-block scale factors (FP32)
    pub scales: Vec<f32>,
    /// Tensor shape (rows, cols)
    pub shape: (usize, usize),
    /// Elements per quantization block
    pub block_size: usize,
}

impl TernaryTensor {
    /// Calculate the fraction of zero weights (sparsity).
    ///
    /// Zero weights enable feature filtering and reduce computation
    /// in ternary matrix multiplication.
    ///
    /// # Returns
    ///
    /// Fraction of weights that are exactly 0, in range [0.0, 1.0].
    /// Returns 0.0 if the tensor has zero elements.
    pub fn sparsity(&self) -> f32 {
        let total_elements = self.shape.0.saturating_mul(self.shape.1);
        if total_elements == 0 {
            return 0.0;
        }
        let unpacked = unpack_ternary(&self.packed_data, total_elements);

        let zero_count = unpacked.iter().filter(|&&x| x == 0).count();
        zero_count as f32 / total_elements as f32
    }

    /// Calculate total memory footprint in bytes.
    ///
    /// Includes both packed ternary data and per-block scales.
    ///
    /// # Returns
    ///
    /// Total bytes: packed_data.len() + scales.len() * 4
    pub fn memory_bytes(&self) -> usize {
        self.packed_data.len() + self.scales.len() * 4
    }

    /// Get the number of quantization blocks.
    ///
    /// Uses saturating arithmetic to prevent overflow for very large tensors.
    /// Returns 0 if `block_size` is zero or the tensor has no elements.
    pub fn num_blocks(&self) -> usize {
        if self.block_size == 0 {
            return 0;
        }
        let total_elements = self.shape.0.saturating_mul(self.shape.1);
        total_elements.saturating_add(self.block_size - 1) / self.block_size
    }
}

/// Pack ternary values {-1, 0, +1} into 2-bit representation.
///
/// Encoding:
/// - -1 → 00
/// - 0  → 01
/// - +1 → 10
/// - (unused) → 11
///
/// Four values are packed into each byte in LSB-first order:
/// ```text
/// byte = [v3:v2:v1:v0]
/// ```
///
/// Values outside {-1, 0, +1} are clamped: negative values map to -1,
/// positive values map to +1.
///
/// # Arguments
///
/// * `values` - Slice of i8 values, ideally in {-1, 0, +1}
///
/// # Returns
///
/// Vector of bytes, length = ceil(values.len() / 4)
///
/// # Example
///
/// ```rust,ignore
/// use ruvllm::bitnet::pack_ternary;
///
/// let values = vec![-1, 0, 1, -1];
/// let packed = pack_ternary(&values);
/// assert_eq!(packed.len(), 1);  // 4 values in 1 byte
/// ```
pub fn pack_ternary(values: &[i8]) -> Vec<u8> {
    let num_bytes = (values.len() + 3) / 4;
    let mut packed = vec![0u8; num_bytes];

    for (i, &val) in values.iter().enumerate() {
        let byte_idx = i / 4;
        let bit_offset = (i % 4) * 2;

        // Clamp out-of-range values: negative -> -1, positive -> +1, zero -> 0
        let encoded: u8 = match val {
            -1 => 0b00,
            0 => 0b01,
            1 => 0b10,
            v if v < -1 => 0b00, // clamp to -1
            _ => 0b10,           // v > 1, clamp to +1
        };

        packed[byte_idx] |= encoded << bit_offset;
    }

    packed
}

/// Unpack 2-bit ternary values to i8.
///
/// Decoding:
/// - 00 → -1
/// - 01 → 0
/// - 10 → +1
/// - 11 → 0 (reserved, treated as zero)
///
/// # Arguments
///
/// * `packed` - Packed 2-bit data
/// * `n` - Number of elements to unpack
///
/// # Returns
///
/// Vector of i8 values in {-1, 0, +1}
///
/// # Example
///
/// ```rust,ignore
/// use ruvllm::bitnet::{pack_ternary, unpack_ternary};
///
/// let original = vec![-1, 0, 1, -1];
/// let packed = pack_ternary(&original);
/// let unpacked = unpack_ternary(&packed, 4);
/// assert_eq!(original, unpacked);
/// ```
pub fn unpack_ternary(packed: &[u8], n: usize) -> Vec<i8> {
    let mut values = Vec::with_capacity(n);

    for i in 0..n {
        let byte_idx = i / 4;
        let bit_offset = (i % 4) * 2;

        if byte_idx >= packed.len() {
            break;
        }

        let encoded = (packed[byte_idx] >> bit_offset) & 0b11;

        let val = match encoded {
            0b00 => -1,
            0b01 => 0,
            0b10 => 1,
            0b11 => 0, // Reserved, treat as zero
            _ => unreachable!(),
        };

        values.push(val);
    }

    values
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pack_unpack_ternary() {
        let values = vec![-1, 0, 1, -1, 1, 0, 0, 1];
        let packed = pack_ternary(&values);
        let unpacked = unpack_ternary(&packed, values.len());
        assert_eq!(values, unpacked);
    }

    #[test]
    fn test_pack_ternary_single_byte() {
        // 4 values fit in 1 byte
        let values = vec![-1, 0, 1, -1];
        let packed = pack_ternary(&values);
        assert_eq!(packed.len(), 1);

        // Manually verify encoding
        // -1=00, 0=01, 1=10, -1=00
        // byte = [00:10:01:00] = 0b00_10_01_00 = 0x08
        assert_eq!(packed[0], 0b00_10_01_00);
    }

    #[test]
    fn test_pack_ternary_partial_byte() {
        // 5 values need 2 bytes
        let values = vec![-1, 0, 1, -1, 1];
        let packed = pack_ternary(&values);
        assert_eq!(packed.len(), 2);
    }

    #[test]
    fn test_pack_clamps_invalid_value() {
        // Values outside {-1, 0, +1} are clamped: 2 -> +1, -5 -> -1
        let values = vec![-5, 0, 2, 3];
        let packed = pack_ternary(&values);
        let unpacked = unpack_ternary(&packed, 4);
        assert_eq!(unpacked[0], -1); // -5 clamped to -1
        assert_eq!(unpacked[1], 0);
        assert_eq!(unpacked[2], 1); // 2 clamped to +1
        assert_eq!(unpacked[3], 1); // 3 clamped to +1
    }

    #[test]
    fn test_ternary_tensor_sparsity() {
        let values = vec![0, 1, 0, -1, 0, 0, 1, 0]; // 5 zeros out of 8
        let packed = pack_ternary(&values);

        let tensor = TernaryTensor {
            packed_data: packed,
            scales: vec![1.0],
            shape: (2, 4),
            block_size: 256,
        };

        let sparsity = tensor.sparsity();
        assert!((sparsity - 0.625).abs() < 0.001); // 5/8 = 0.625
    }

    #[test]
    fn test_ternary_tensor_memory() {
        let packed = vec![0u8; 64]; // 64 bytes of packed data
        let scales = vec![0.5f32; 16]; // 16 scales * 4 bytes = 64 bytes

        let tensor = TernaryTensor {
            packed_data: packed,
            scales,
            shape: (128, 256),
            block_size: 256,
        };

        assert_eq!(tensor.memory_bytes(), 64 + 64); // 128 bytes total
    }

    #[test]
    fn test_ternary_tensor_num_blocks() {
        let tensor = TernaryTensor {
            packed_data: vec![],
            scales: vec![],
            shape: (256, 256), // 65536 elements
            block_size: 256,   // 256 elements per block
        };

        assert_eq!(tensor.num_blocks(), 256); // 65536 / 256 = 256 blocks
    }
}
