//! INT4 quantization for maximum weight compression.
//!
//! Stores 2 weight values per byte, providing 2× memory reduction over INT8.
//! Uses per-row scaling for accuracy preservation.
//!
//! ## Format
//!
//! Each byte stores 2 INT4 values (range -8 to +7):
//! - High nibble: first value (bits 4-7)
//! - Low nibble: second value (bits 0-3)
//!
//! Values are stored in signed representation:
//! - 0-7 represent 0-7
//! - 8-15 represent -8 to -1
//!
//! ## Memory Savings
//!
//! | Model Size | INT8  | INT4  | Savings |
//! |------------|-------|-------|---------|
//! | 7B params  | 7 GB  | 3.5 GB| 50%     |
//! | 13B params | 13 GB | 6.5 GB| 50%     |
//! | 70B params | 70 GB | 35 GB | 50%     |
//!
//! ## Accuracy
//!
//! Per-row scaling preserves relative magnitudes within each output row.
//! Typical accuracy loss: 0.5-2% on downstream tasks vs INT8.

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;

/// INT4 quantization range (signed 4-bit)
const INT4_MIN: i8 = -8;
const INT4_MAX: i8 = 7;

/// Pack two INT4 values into a single byte.
///
/// High nibble contains first value, low nibble contains second.
#[inline]
pub fn pack_int4(v0: i8, v1: i8) -> u8 {
    let n0 = (v0.clamp(INT4_MIN, INT4_MAX) & 0x0F) as u8;
    let n1 = (v1.clamp(INT4_MIN, INT4_MAX) & 0x0F) as u8;
    (n0 << 4) | n1
}

/// Unpack a byte into two INT4 values.
///
/// Returns (high nibble, low nibble) as signed values.
#[inline]
pub fn unpack_int4(packed: u8) -> (i8, i8) {
    let n0 = (packed >> 4) as i8;
    let n1 = (packed & 0x0F) as i8;

    // Sign extend from 4-bit to 8-bit
    let v0 = if n0 > 7 { n0 - 16 } else { n0 };
    let v1 = if n1 > 7 { n1 - 16 } else { n1 };

    (v0, v1)
}

/// Quantize f32 values to INT4 with scale.
///
/// # Arguments
///
/// * `values` - Input f32 values
/// * `output` - Packed output (length = ceil(values.len() / 2))
///
/// # Returns
///
/// Scale factor for dequantization
pub fn quantize_f32_to_int4(values: &[f32], output: &mut [u8]) -> f32 {
    if values.is_empty() {
        return 1.0;
    }

    // Find max absolute value for scaling
    let max_abs = values.iter().map(|&v| v.abs()).fold(0.0f32, f32::max);
    let scale = if max_abs == 0.0 {
        1.0
    } else {
        max_abs / 7.0 // Map to [-7, 7] range
    };
    let inv_scale = 1.0 / scale;

    // Pack pairs of values
    let pairs = values.len() / 2;
    for i in 0..pairs {
        let v0 = (values[i * 2] * inv_scale).round().clamp(-8.0, 7.0) as i8;
        let v1 = (values[i * 2 + 1] * inv_scale).round().clamp(-8.0, 7.0) as i8;
        output[i] = pack_int4(v0, v1);
    }

    // Handle odd length
    if values.len() % 2 == 1 {
        let v0 = (values[values.len() - 1] * inv_scale)
            .round()
            .clamp(-8.0, 7.0) as i8;
        output[pairs] = pack_int4(v0, 0);
    }

    scale
}

/// Dequantize INT4 values to f32.
///
/// # Arguments
///
/// * `packed` - Packed INT4 values
/// * `scale` - Scale factor from quantization
/// * `count` - Number of values (may be odd)
/// * `output` - Output f32 values
pub fn dequantize_int4_to_f32(packed: &[u8], scale: f32, count: usize, output: &mut [f32]) {
    let pairs = count / 2;

    for i in 0..pairs {
        let (v0, v1) = unpack_int4(packed[i]);
        output[i * 2] = v0 as f32 * scale;
        output[i * 2 + 1] = v1 as f32 * scale;
    }

    // Handle odd length
    if count % 2 == 1 && !packed.is_empty() {
        let (v0, _) = unpack_int4(packed[pairs]);
        output[count - 1] = v0 as f32 * scale;
    }
}

/// INT4 quantized weight matrix.
///
/// Stores weights in packed INT4 format with per-row scaling.
#[derive(Clone, Debug)]
pub struct Int4Weights {
    /// Packed weight data (2 values per byte)
    pub data: Vec<u8>,
    /// Per-row scale factors
    pub row_scales: Vec<f32>,
    /// Number of rows
    pub rows: usize,
    /// Number of columns
    pub cols: usize,
}

impl Int4Weights {
    /// Create new INT4 weights from f32 matrix.
    ///
    /// # Arguments
    ///
    /// * `weights` - Row-major f32 weights [rows, cols]
    /// * `rows` - Number of rows
    /// * `cols` - Number of columns
    pub fn from_f32(weights: &[f32], rows: usize, cols: usize) -> Self {
        assert_eq!(weights.len(), rows * cols);

        let packed_cols = (cols + 1) / 2;
        let mut data = vec![0u8; rows * packed_cols];
        let mut row_scales = Vec::with_capacity(rows);

        for r in 0..rows {
            let row_start = r * cols;
            let row_end = row_start + cols;
            let row = &weights[row_start..row_end];

            let packed_start = r * packed_cols;
            let packed_end = packed_start + packed_cols;
            let packed = &mut data[packed_start..packed_end];

            let scale = quantize_f32_to_int4(row, packed);
            row_scales.push(scale);
        }

        Self {
            data,
            row_scales,
            rows,
            cols,
        }
    }

    /// Dequantize a single row to f32.
    pub fn dequantize_row(&self, row: usize, output: &mut [f32]) {
        debug_assert!(row < self.rows);
        debug_assert!(output.len() >= self.cols);

        let packed_cols = (self.cols + 1) / 2;
        let packed_start = row * packed_cols;
        let packed = &self.data[packed_start..packed_start + packed_cols];
        let scale = self.row_scales[row];

        dequantize_int4_to_f32(packed, scale, self.cols, output);
    }

    /// Get packed row data.
    #[inline]
    pub fn packed_row(&self, row: usize) -> &[u8] {
        let packed_cols = (self.cols + 1) / 2;
        let start = row * packed_cols;
        &self.data[start..start + packed_cols]
    }

    /// Get row scale.
    #[inline]
    pub fn row_scale(&self, row: usize) -> f32 {
        self.row_scales[row]
    }

    /// Memory size in bytes.
    pub fn memory_bytes(&self) -> usize {
        self.data.len() + self.row_scales.len() * 4
    }
}

/// INT4 matrix-vector multiplication.
///
/// Computes y = A * x where A is INT4 quantized.
///
/// # Arguments
///
/// * `weights` - INT4 weight matrix [n, k]
/// * `x` - Input vector [k]
/// * `x_scale` - Scale for input vector
/// * `output` - Output vector [n]
pub fn int4_gemv(weights: &Int4Weights, x: &[f32], x_scale: f32, output: &mut [f32]) {
    let n = weights.rows;
    let k = weights.cols;

    debug_assert!(x.len() >= k);
    debug_assert!(output.len() >= n);

    let packed_cols = (k + 1) / 2;

    for i in 0..n {
        let packed_row = weights.packed_row(i);
        let row_scale = weights.row_scale(i);
        let combined_scale = row_scale * x_scale;

        let mut acc = 0.0f32;

        // Process pairs
        for j in 0..packed_cols {
            let (v0, v1) = unpack_int4(packed_row[j]);
            let x_idx = j * 2;

            if x_idx < k {
                acc += v0 as f32 * x[x_idx];
            }
            if x_idx + 1 < k {
                acc += v1 as f32 * x[x_idx + 1];
            }
        }

        output[i] = acc * combined_scale;
    }
}

/// INT4 matrix-matrix multiplication (GEMM).
///
/// Computes C = A * B^T where A is INT4 quantized.
///
/// # Arguments
///
/// * `weights` - INT4 weight matrix B [n, k]
/// * `a` - Input matrix A [m, k]
/// * `a_scale` - Scale for input matrix
/// * `m` - Rows in A
/// * `output` - Output matrix C [m, n]
pub fn int4_gemm(weights: &Int4Weights, a: &[f32], a_scale: f32, m: usize, output: &mut [f32]) {
    let n = weights.rows;
    let k = weights.cols;

    debug_assert!(a.len() >= m * k);
    debug_assert!(output.len() >= m * n);

    for i in 0..m {
        let a_row = &a[i * k..(i + 1) * k];
        let out_row = &mut output[i * n..(i + 1) * n];
        int4_gemv(weights, a_row, a_scale, out_row);
    }
}

/// Compressed block INT4 format for large matrices.
///
/// Uses block-wise scaling for better accuracy on large matrices.
/// Block size is typically 32 or 64 elements.
#[derive(Clone, Debug)]
pub struct BlockInt4Weights {
    /// Packed weight data
    pub data: Vec<u8>,
    /// Block scale factors
    pub block_scales: Vec<f32>,
    /// Block size
    pub block_size: usize,
    /// Number of rows
    pub rows: usize,
    /// Number of columns
    pub cols: usize,
}

impl BlockInt4Weights {
    /// Default block size (32 elements)
    pub const DEFAULT_BLOCK_SIZE: usize = 32;

    /// Create from f32 weights with default block size.
    pub fn from_f32(weights: &[f32], rows: usize, cols: usize) -> Self {
        Self::from_f32_with_block_size(weights, rows, cols, Self::DEFAULT_BLOCK_SIZE)
    }

    /// Create from f32 weights with specified block size.
    pub fn from_f32_with_block_size(
        weights: &[f32],
        rows: usize,
        cols: usize,
        block_size: usize,
    ) -> Self {
        assert_eq!(weights.len(), rows * cols);

        let packed_cols = (cols + 1) / 2;
        let blocks_per_row = (cols + block_size - 1) / block_size;
        let total_blocks = rows * blocks_per_row;

        let mut data = vec![0u8; rows * packed_cols];
        let mut block_scales = Vec::with_capacity(total_blocks);

        for r in 0..rows {
            let row_start = r * cols;
            let packed_row_start = r * packed_cols;

            for b in 0..blocks_per_row {
                let block_start = b * block_size;
                let block_end = (block_start + block_size).min(cols);
                let block_len = block_end - block_start;

                // Find max abs in block
                let mut max_abs = 0.0f32;
                for i in block_start..block_end {
                    max_abs = max_abs.max(weights[row_start + i].abs());
                }

                let scale = if max_abs == 0.0 { 1.0 } else { max_abs / 7.0 };
                let inv_scale = 1.0 / scale;
                block_scales.push(scale);

                // Quantize block
                let packed_block_start = packed_row_start + block_start / 2;
                for i in (0..block_len).step_by(2) {
                    let v0 = (weights[row_start + block_start + i] * inv_scale)
                        .round()
                        .clamp(-8.0, 7.0) as i8;
                    let v1 = if i + 1 < block_len {
                        (weights[row_start + block_start + i + 1] * inv_scale)
                            .round()
                            .clamp(-8.0, 7.0) as i8
                    } else {
                        0
                    };
                    data[packed_block_start + i / 2] = pack_int4(v0, v1);
                }
            }
        }

        Self {
            data,
            block_scales,
            block_size,
            rows,
            cols,
        }
    }

    /// Memory size in bytes.
    pub fn memory_bytes(&self) -> usize {
        self.data.len() + self.block_scales.len() * 4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pack_unpack() {
        // Test positive values
        let packed = pack_int4(5, 3);
        let (v0, v1) = unpack_int4(packed);
        assert_eq!(v0, 5);
        assert_eq!(v1, 3);

        // Test negative values
        let packed = pack_int4(-3, -7);
        let (v0, v1) = unpack_int4(packed);
        assert_eq!(v0, -3);
        assert_eq!(v1, -7);

        // Test mixed
        let packed = pack_int4(-8, 7);
        let (v0, v1) = unpack_int4(packed);
        assert_eq!(v0, -8);
        assert_eq!(v1, 7);
    }

    #[test]
    fn test_pack_clamp() {
        // Values outside range should be clamped
        let packed = pack_int4(15, -20);
        let (v0, v1) = unpack_int4(packed);
        assert_eq!(v0, 7); // Clamped from 15
        assert_eq!(v1, -8); // Clamped from -20
    }

    #[test]
    fn test_quantize_dequantize() {
        let values = vec![0.5, -0.25, 1.0, -1.0, 0.0, 0.75];
        let mut packed = vec![0u8; 3];

        let scale = quantize_f32_to_int4(&values, &mut packed);
        assert!(scale > 0.0);

        let mut recovered = vec![0.0f32; 6];
        dequantize_int4_to_f32(&packed, scale, 6, &mut recovered);

        // Check approximate recovery (INT4 has low precision)
        for (orig, rec) in values.iter().zip(recovered.iter()) {
            assert!((orig - rec).abs() < 0.2, "orig={}, rec={}", orig, rec);
        }
    }

    #[test]
    fn test_quantize_odd_length() {
        let values = vec![0.5, -0.25, 1.0];
        let mut packed = vec![0u8; 2];

        let scale = quantize_f32_to_int4(&values, &mut packed);

        let mut recovered = vec![0.0f32; 3];
        dequantize_int4_to_f32(&packed, scale, 3, &mut recovered);

        assert!((values[0] - recovered[0]).abs() < 0.2);
        assert!((values[1] - recovered[1]).abs() < 0.2);
        assert!((values[2] - recovered[2]).abs() < 0.2);
    }

    #[test]
    fn test_int4_weights() {
        let weights: Vec<f32> = vec![1.0, -0.5, 0.25, -0.75, 0.0, 1.0, -1.0, 0.5];
        let int4_w = Int4Weights::from_f32(&weights, 2, 4);

        assert_eq!(int4_w.rows, 2);
        assert_eq!(int4_w.cols, 4);
        assert_eq!(int4_w.row_scales.len(), 2);

        // Verify memory savings (2 bytes per 4 weights + 4 bytes scale = 3x savings)
        let original_size = weights.len() * 4;
        let compressed_size = int4_w.memory_bytes();
        assert!(compressed_size < original_size);

        // Dequantize and verify
        let mut row0 = vec![0.0f32; 4];
        int4_w.dequantize_row(0, &mut row0);

        // INT4 precision is limited, check approximate match
        assert!((row0[0] - 1.0).abs() < 0.3);
        assert!((row0[1] - (-0.5)).abs() < 0.3);
    }

    #[test]
    fn test_int4_gemv() {
        let weights = vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        let int4_w = Int4Weights::from_f32(&weights, 3, 3);

        let x = vec![1.0, 2.0, 3.0];
        let mut y = vec![0.0f32; 3];

        int4_gemv(&int4_w, &x, 1.0, &mut y);

        // Identity matrix should return approximately the input
        // (with some quantization error)
        assert!((y[0] - 1.0).abs() < 0.5);
        assert!((y[1] - 2.0).abs() < 0.5);
        assert!((y[2] - 3.0).abs() < 0.5);
    }

    #[test]
    fn test_block_int4_weights() {
        let weights: Vec<f32> = (0..128).map(|i| (i as f32 - 64.0) / 64.0).collect();
        let block_w = BlockInt4Weights::from_f32(&weights, 4, 32);

        assert_eq!(block_w.rows, 4);
        assert_eq!(block_w.cols, 32);

        // Check memory savings
        let original = 4 * 32 * 4; // 512 bytes
        let compressed = block_w.memory_bytes();
        assert!(compressed < original);
    }

    #[test]
    fn test_int4_range() {
        // Verify all values in range map correctly
        for v in INT4_MIN..=INT4_MAX {
            let packed = pack_int4(v, 0);
            let (unpacked, _) = unpack_int4(packed);
            assert_eq!(v, unpacked, "Value {} should round-trip", v);
        }
    }
}
