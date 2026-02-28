//! Groupwise symmetric quantization with f16 scales.
//!
//! For each group of `group_len` values:
//! - `scale = max(|v_i|) / qmax`
//! - `q_i = round(v_i / scale)`, clamped to `[-qmax, +qmax]`
//! - `u_i = q_i + qmax` (bias to unsigned for packing)

use crate::bitpack::qmax_from_bits;
use crate::f16;

/// Compute f16 group scales for a frame.
///
/// Returns one f16-encoded scale per group of `group_len` elements.
/// Each scale is `max(|v|) / qmax` for that group, stored as IEEE 754 half-precision.
#[inline]
pub fn compute_scales(frame: &[f32], group_len: usize, bits: u8) -> Vec<u16> {
    let qmax = qmax_from_bits(bits);
    if qmax == 0 {
        return Vec::new();
    }
    let qmax_f = qmax as f32;
    let num_groups = frame.len().div_ceil(group_len);
    let mut scales = Vec::with_capacity(num_groups);

    for chunk in frame.chunks(group_len) {
        let mut max_abs = 0.0f32;
        for &v in chunk {
            if v.is_finite() {
                let a = v.abs();
                if a > max_abs {
                    max_abs = a;
                }
            }
        }

        let scale = if max_abs == 0.0 {
            0.0
        } else {
            max_abs / qmax_f
        };
        scales.push(f16::f32_to_f16_bits(scale));
    }

    scales
}

/// Pre-convert f16 scales to f32 for hot-path use.
#[inline]
pub fn scales_to_f32(scales_f16: &[u16]) -> Vec<f32> {
    scales_f16
        .iter()
        .map(|&s| f16::f16_bits_to_f32(s))
        .collect()
}

/// Check if a frame fits within existing scales (within drift tolerance).
///
/// Uses pre-converted f32 scales to avoid repeated f16 conversion.
/// Returns `false` if any group's max absolute value exceeds
/// `scale * qmax * drift_factor`.
pub fn frame_fits_scales_f32(
    frame: &[f32],
    scales_f32: &[f32],
    group_len: usize,
    bits: u8,
    drift_factor: f32,
) -> bool {
    let qmax = qmax_from_bits(bits);
    if qmax == 0 || scales_f32.is_empty() {
        return false;
    }
    let qmax_f = qmax as f32;

    for (group_idx, chunk) in frame.chunks(group_len).enumerate() {
        if group_idx >= scales_f32.len() {
            return false;
        }
        let allowed = scales_f32[group_idx] * qmax_f * drift_factor;

        for &v in chunk {
            if v.is_finite() && v.abs() > allowed {
                return false;
            }
        }
    }

    true
}

/// Quantize a frame using pre-computed f32 scales and pack into bitstream.
///
/// Appends packed bytes to `out`. Pre-reserves the expected output size
/// to avoid reallocations.
///
/// For 8-bit quantization, writes bytes directly without bit accumulation
/// since each quantized value maps 1:1 to a u8.
#[inline]
pub fn quantize_and_pack_f32(
    frame: &[f32],
    scales_f32: &[f32],
    group_len: usize,
    bits: u8,
    out: &mut Vec<u8>,
) {
    let qmax = qmax_from_bits(bits);
    if qmax == 0 {
        return;
    }

    // Fast path: 8-bit quantization writes bytes directly, no bit accumulator.
    if bits == 8 {
        out.reserve(frame.len());
        for (group_idx, chunk) in frame.chunks(group_len).enumerate() {
            let scale = if group_idx < scales_f32.len() {
                scales_f32[group_idx]
            } else {
                0.0
            };
            let inv_scale = if scale == 0.0 { 0.0 } else { 1.0 / scale };
            for &v in chunk {
                let mut q: i32 = 0;
                if v.is_finite() {
                    let scaled = v * inv_scale;
                    q = if scaled >= 0.0 {
                        (scaled + 0.5) as i32
                    } else {
                        (scaled - 0.5) as i32
                    };
                    q = q.clamp(-127, 127);
                }
                out.push((q + 127) as u8);
            }
        }
        return;
    }

    // Fast path: 5-bit quantization packs 8 values into 5 bytes.
    // 8 values * 5 bits = 40 bits = 5 bytes exactly, avoiding the bit accumulator.
    // LSB-first packing layout for 8 values in 5 bytes:
    //   byte0 = v0 | (v1 << 5)
    //   byte1 = (v1 >> 3) | (v2 << 2) | (v3 << 7)
    //   byte2 = (v3 >> 1) | (v4 << 4)
    //   byte3 = (v4 >> 4) | (v5 << 1) | (v6 << 6)
    //   byte4 = (v6 >> 2) | (v7 << 3)
    #[inline]
    fn pack_5bit_group(chunk: &[f32], inv_scale: f32, out: &mut Vec<u8>) {
        let quantize = |v: f32| -> u32 {
            let mut q: i32 = 0;
            if v.is_finite() {
                let scaled = v * inv_scale;
                q = if scaled >= 0.0 {
                    (scaled + 0.5) as i32
                } else {
                    (scaled - 0.5) as i32
                };
                q = q.clamp(-15, 15);
            }
            (q + 15) as u32
        };
        let v0 = quantize(chunk[0]);
        let v1 = quantize(chunk[1]);
        let v2 = quantize(chunk[2]);
        let v3 = quantize(chunk[3]);
        let v4 = quantize(chunk[4]);
        let v5 = quantize(chunk[5]);
        let v6 = quantize(chunk[6]);
        let v7 = quantize(chunk[7]);

        out.push((v0 | (v1 << 5)) as u8);
        out.push(((v1 >> 3) | (v2 << 2) | (v3 << 7)) as u8);
        out.push(((v3 >> 1) | (v4 << 4)) as u8);
        out.push(((v4 >> 4) | (v5 << 1) | (v6 << 6)) as u8);
        out.push(((v6 >> 2) | (v7 << 3)) as u8);
    }
    if bits == 5 {
        let needed_bytes = (frame.len() * 5).div_ceil(8);
        out.reserve(needed_bytes);

        let mut acc: u64 = 0;
        let mut acc_bits: u32 = 0;

        for (group_idx, chunk) in frame.chunks(group_len).enumerate() {
            let scale = if group_idx < scales_f32.len() {
                scales_f32[group_idx]
            } else {
                0.0
            };
            let inv_scale = if scale == 0.0 { 0.0 } else { 1.0 / scale };

            let mut i = 0;
            // Process 8 values at a time into 5 bytes when byte-aligned
            while acc_bits == 0 && i + 8 <= chunk.len() {
                pack_5bit_group(&chunk[i..i + 8], inv_scale, out);
                i += 8;
            }
            // Remainder (or misaligned) with bit accumulator
            while i < chunk.len() {
                let mut q: i32 = 0;
                if chunk[i].is_finite() {
                    let scaled = chunk[i] * inv_scale;
                    q = if scaled >= 0.0 {
                        (scaled + 0.5) as i32
                    } else {
                        (scaled - 0.5) as i32
                    };
                    q = q.clamp(-15, 15);
                }
                let u = (q + 15) as u32;
                acc |= (u as u64) << acc_bits;
                acc_bits += 5;
                while acc_bits >= 8 {
                    out.push((acc & 0xFF) as u8);
                    acc >>= 8;
                    acc_bits -= 8;
                }
                i += 1;
            }
        }

        if acc_bits > 0 {
            out.push((acc & 0xFF) as u8);
        }
        return;
    }

    // Generic path for sub-byte bit widths.
    let qmax_i = qmax;
    let bias = qmax;
    let bits_u32 = bits as u32;

    let needed_bytes = (frame.len() * bits as usize).div_ceil(8);
    out.reserve(needed_bytes);

    let mut acc: u64 = 0;
    let mut acc_bits: u32 = 0;

    for (group_idx, chunk) in frame.chunks(group_len).enumerate() {
        let scale = if group_idx < scales_f32.len() {
            scales_f32[group_idx]
        } else {
            0.0
        };
        let inv_scale = if scale == 0.0 { 0.0 } else { 1.0 / scale };

        for &v in chunk {
            let mut q: i32 = 0;
            if v.is_finite() {
                let scaled = v * inv_scale;
                q = if scaled >= 0.0 {
                    (scaled + 0.5) as i32
                } else {
                    (scaled - 0.5) as i32
                };
                q = q.clamp(-qmax_i, qmax_i);
            }

            let u = (q + bias) as u32;
            acc |= (u as u64) << acc_bits;
            acc_bits += bits_u32;

            while acc_bits >= 8 {
                out.push((acc & 0xFF) as u8);
                acc >>= 8;
                acc_bits -= 8;
            }
        }
    }

    if acc_bits > 0 {
        out.push((acc & 0xFF) as u8);
    }
}

/// Dequantize packed codes using f32 scales, writing f32 values.
///
/// Iterates by frame then by group to avoid per-value modulo/division
/// and caches the f32 scale per group.
///
/// For 8-bit data, reads bytes directly without bit accumulation.
#[inline]
pub fn dequantize_f32(
    data: &[u8],
    scales_f32: &[f32],
    group_len: usize,
    bits: u8,
    tensor_len: usize,
    frame_count: usize,
    out: &mut Vec<f32>,
) {
    let qmax = qmax_from_bits(bits);
    if qmax == 0 {
        return;
    }

    let total = tensor_len * frame_count;
    out.resize(total, 0.0);

    // Fast path: 8-bit dequantization reads bytes directly, no bit accumulator.
    if bits == 8 {
        let mut out_idx = 0usize;
        let mut byte_idx = 0usize;
        for _frame in 0..frame_count {
            let mut pos = 0usize;
            let mut group_idx = 0usize;
            while pos < tensor_len {
                let group_end = (pos + group_len).min(tensor_len);
                let scale = if group_idx < scales_f32.len() {
                    scales_f32[group_idx]
                } else {
                    0.0
                };
                while pos < group_end && byte_idx < data.len() {
                    let u = data[byte_idx] as i32;
                    let q = u - 127;
                    out[out_idx] = (q as f32) * scale;
                    out_idx += 1;
                    byte_idx += 1;
                    pos += 1;
                }
                group_idx += 1;
            }
        }
        return;
    }

    // Fast path: 3-bit dequantization processes 8 values from 3 bytes.
    // 8 values * 3 bits = 24 bits = 3 bytes exactly, avoiding the bit accumulator.
    // LSB-first packing layout for 8 values in 3 bytes:
    //   byte0 = v0 | (v1 << 3) | ((v2 & 0x3) << 6)
    //   byte1 = (v2 >> 2) | (v3 << 1) | (v4 << 4) | ((v5 & 0x1) << 7)
    //   byte2 = (v5 >> 1) | (v6 << 2) | (v7 << 5)
    if bits == 3 {
        let bias = 3i32; // qmax for 3-bit
        let mut out_idx = 0usize;
        let mut byte_idx = 0usize;
        for _frame in 0..frame_count {
            let mut pos = 0usize;
            let mut group_idx = 0usize;
            while pos < tensor_len {
                let group_end = (pos + group_len).min(tensor_len);
                let scale = if group_idx < scales_f32.len() {
                    scales_f32[group_idx]
                } else {
                    0.0
                };
                // Process 8 values at a time from 3 bytes
                while pos + 8 <= group_end && byte_idx + 3 <= data.len() {
                    let b0 = data[byte_idx] as u32;
                    let b1 = data[byte_idx + 1] as u32;
                    let b2 = data[byte_idx + 2] as u32;
                    byte_idx += 3;

                    out[out_idx] = ((b0 & 0x7) as i32 - bias) as f32 * scale;
                    out[out_idx + 1] = (((b0 >> 3) & 0x7) as i32 - bias) as f32 * scale;
                    out[out_idx + 2] =
                        ((((b0 >> 6) | (b1 << 2)) & 0x7) as i32 - bias) as f32 * scale;
                    out[out_idx + 3] = (((b1 >> 1) & 0x7) as i32 - bias) as f32 * scale;
                    out[out_idx + 4] = (((b1 >> 4) & 0x7) as i32 - bias) as f32 * scale;
                    out[out_idx + 5] =
                        ((((b1 >> 7) | (b2 << 1)) & 0x7) as i32 - bias) as f32 * scale;
                    out[out_idx + 6] = (((b2 >> 2) & 0x7) as i32 - bias) as f32 * scale;
                    out[out_idx + 7] = (((b2 >> 5) & 0x7) as i32 - bias) as f32 * scale;
                    out_idx += 8;
                    pos += 8;
                }
                // Handle remaining values (< 8) with a local bit accumulator
                if pos < group_end {
                    let remaining = group_end - pos;
                    let mut acc: u64 = 0;
                    let mut acc_bits: u32 = 0;
                    while acc_bits < (remaining as u32) * 3 && byte_idx < data.len() {
                        acc |= (data[byte_idx] as u64) << acc_bits;
                        acc_bits += 8;
                        byte_idx += 1;
                    }
                    for _ in 0..remaining {
                        if acc_bits < 3 {
                            break;
                        }
                        let u = (acc & 0x7) as i32;
                        acc >>= 3;
                        acc_bits -= 3;
                        out[out_idx] = (u - bias) as f32 * scale;
                        out_idx += 1;
                        pos += 1;
                    }
                }
                group_idx += 1;
            }
        }
        return;
    }

    // Fast path: 7-bit dequantization processes 8 values from 7 bytes.
    // 8 values * 7 bits = 56 bits = 7 bytes exactly, avoiding the bit accumulator.
    // LSB-first packing layout for 8 values in 7 bytes:
    //   v0 = b0 & 0x7F
    //   v1 = ((b0 >> 7) | (b1 << 1)) & 0x7F
    //   v2 = ((b1 >> 6) | (b2 << 2)) & 0x7F
    //   v3 = ((b2 >> 5) | (b3 << 3)) & 0x7F
    //   v4 = ((b3 >> 4) | (b4 << 4)) & 0x7F
    //   v5 = ((b4 >> 3) | (b5 << 5)) & 0x7F
    //   v6 = ((b5 >> 2) | (b6 << 6)) & 0x7F
    //   v7 = (b6 >> 1) & 0x7F
    if bits == 7 {
        let bias = 63i32; // qmax for 7-bit
        let mut out_idx = 0usize;
        let mut byte_idx = 0usize;
        for _frame in 0..frame_count {
            let mut pos = 0usize;
            let mut group_idx = 0usize;
            while pos < tensor_len {
                let group_end = (pos + group_len).min(tensor_len);
                let scale = if group_idx < scales_f32.len() {
                    scales_f32[group_idx]
                } else {
                    0.0
                };
                // Process 8 values at a time from 7 bytes
                #[inline]
                fn unpack_7bit(
                    out: &mut [f32],
                    out_idx: usize,
                    data: &[u8],
                    byte_idx: usize,
                    bias: i32,
                    scale: f32,
                ) {
                    let b0 = data[byte_idx] as u32;
                    let b1 = data[byte_idx + 1] as u32;
                    let b2 = data[byte_idx + 2] as u32;
                    let b3 = data[byte_idx + 3] as u32;
                    let b4 = data[byte_idx + 4] as u32;
                    let b5 = data[byte_idx + 5] as u32;
                    let b6 = data[byte_idx + 6] as u32;

                    out[out_idx] = ((b0 & 0x7F) as i32 - bias) as f32 * scale;
                    out[out_idx + 1] =
                        ((((b0 >> 7) | (b1 << 1)) & 0x7F) as i32 - bias) as f32 * scale;
                    out[out_idx + 2] =
                        ((((b1 >> 6) | (b2 << 2)) & 0x7F) as i32 - bias) as f32 * scale;
                    out[out_idx + 3] =
                        ((((b2 >> 5) | (b3 << 3)) & 0x7F) as i32 - bias) as f32 * scale;
                    out[out_idx + 4] =
                        ((((b3 >> 4) | (b4 << 4)) & 0x7F) as i32 - bias) as f32 * scale;
                    out[out_idx + 5] =
                        ((((b4 >> 3) | (b5 << 5)) & 0x7F) as i32 - bias) as f32 * scale;
                    out[out_idx + 6] =
                        ((((b5 >> 2) | (b6 << 6)) & 0x7F) as i32 - bias) as f32 * scale;
                    out[out_idx + 7] = (((b6 >> 1) & 0x7F) as i32 - bias) as f32 * scale;
                }
                while pos + 8 <= group_end && byte_idx + 7 <= data.len() {
                    unpack_7bit(out, out_idx, data, byte_idx, bias, scale);
                    byte_idx += 7;
                    out_idx += 8;
                    pos += 8;
                }
                // Handle remaining values (< 8) with a local bit accumulator
                if pos < group_end {
                    let remaining = group_end - pos;
                    let mut acc: u64 = 0;
                    let mut acc_bits: u32 = 0;
                    while acc_bits < (remaining as u32) * 7 && byte_idx < data.len() {
                        acc |= (data[byte_idx] as u64) << acc_bits;
                        acc_bits += 8;
                        byte_idx += 1;
                    }
                    for _ in 0..remaining {
                        if acc_bits < 7 {
                            break;
                        }
                        let u = (acc & 0x7F) as i32;
                        acc >>= 7;
                        acc_bits -= 7;
                        out[out_idx] = (u - bias) as f32 * scale;
                        out_idx += 1;
                        pos += 1;
                    }
                }
                group_idx += 1;
            }
        }
        return;
    }

    // Fast path: 5-bit dequantization processes 8 values from 5 bytes.
    // 8 values * 5 bits = 40 bits = 5 bytes exactly, avoiding the bit accumulator.
    // LSB-first packing layout for 8 values in 5 bytes:
    //   v0 = b0 & 0x1F
    //   v1 = ((b0 >> 5) | (b1 << 3)) & 0x1F
    //   v2 = (b1 >> 2) & 0x1F
    //   v3 = ((b1 >> 7) | (b2 << 1)) & 0x1F
    //   v4 = ((b2 >> 4) | (b3 << 4)) & 0x1F
    //   v5 = (b3 >> 1) & 0x1F
    //   v6 = ((b3 >> 6) | (b4 << 2)) & 0x1F
    //   v7 = (b4 >> 3) & 0x1F
    if bits == 5 {
        let bias = 15i32; // qmax for 5-bit
        let mut out_idx = 0usize;
        let mut byte_idx = 0usize;
        for _frame in 0..frame_count {
            let mut pos = 0usize;
            let mut group_idx = 0usize;
            while pos < tensor_len {
                let group_end = (pos + group_len).min(tensor_len);
                let scale = if group_idx < scales_f32.len() {
                    scales_f32[group_idx]
                } else {
                    0.0
                };
                // Process 8 values at a time from 5 bytes
                #[inline]
                fn unpack_5bit(
                    out: &mut [f32],
                    out_idx: usize,
                    data: &[u8],
                    byte_idx: usize,
                    bias: i32,
                    scale: f32,
                ) {
                    let b0 = data[byte_idx] as u32;
                    let b1 = data[byte_idx + 1] as u32;
                    let b2 = data[byte_idx + 2] as u32;
                    let b3 = data[byte_idx + 3] as u32;
                    let b4 = data[byte_idx + 4] as u32;

                    out[out_idx] = ((b0 & 0x1F) as i32 - bias) as f32 * scale;
                    out[out_idx + 1] =
                        ((((b0 >> 5) | (b1 << 3)) & 0x1F) as i32 - bias) as f32 * scale;
                    out[out_idx + 2] = (((b1 >> 2) & 0x1F) as i32 - bias) as f32 * scale;
                    out[out_idx + 3] =
                        ((((b1 >> 7) | (b2 << 1)) & 0x1F) as i32 - bias) as f32 * scale;
                    out[out_idx + 4] =
                        ((((b2 >> 4) | (b3 << 4)) & 0x1F) as i32 - bias) as f32 * scale;
                    out[out_idx + 5] = (((b3 >> 1) & 0x1F) as i32 - bias) as f32 * scale;
                    out[out_idx + 6] =
                        ((((b3 >> 6) | (b4 << 2)) & 0x1F) as i32 - bias) as f32 * scale;
                    out[out_idx + 7] = (((b4 >> 3) & 0x1F) as i32 - bias) as f32 * scale;
                }
                while pos + 8 <= group_end && byte_idx + 5 <= data.len() {
                    unpack_5bit(out, out_idx, data, byte_idx, bias, scale);
                    byte_idx += 5;
                    out_idx += 8;
                    pos += 8;
                }
                // Handle remaining values (< 8) with a local bit accumulator
                if pos < group_end {
                    let remaining = group_end - pos;
                    let mut acc: u64 = 0;
                    let mut acc_bits: u32 = 0;
                    while acc_bits < (remaining as u32) * 5 && byte_idx < data.len() {
                        acc |= (data[byte_idx] as u64) << acc_bits;
                        acc_bits += 8;
                        byte_idx += 1;
                    }
                    for _ in 0..remaining {
                        if acc_bits < 5 {
                            break;
                        }
                        let u = (acc & 0x1F) as i32;
                        acc >>= 5;
                        acc_bits -= 5;
                        out[out_idx] = (u - bias) as f32 * scale;
                        out_idx += 1;
                        pos += 1;
                    }
                }
                group_idx += 1;
            }
        }
        return;
    }

    // Generic path for sub-byte bit widths.
    let bias = qmax;
    let bits_u32 = bits as u32;
    let mask = (1u64 << bits_u32) - 1;

    let mut acc: u64 = 0;
    let mut acc_bits: u32 = 0;
    let mut byte_idx = 0usize;
    let mut out_idx = 0usize;

    for _frame in 0..frame_count {
        let mut pos = 0usize;
        let mut group_idx = 0usize;

        while pos < tensor_len {
            let group_end = (pos + group_len).min(tensor_len);
            let scale = if group_idx < scales_f32.len() {
                scales_f32[group_idx]
            } else {
                0.0
            };

            while pos < group_end {
                while acc_bits < bits_u32 && byte_idx < data.len() {
                    acc |= (data[byte_idx] as u64) << acc_bits;
                    acc_bits += 8;
                    byte_idx += 1;
                }
                if acc_bits < bits_u32 {
                    return;
                }

                let u = (acc & mask) as u32;
                acc >>= bits_u32;
                acc_bits -= bits_u32;

                let q = (u as i32) - bias;
                out[out_idx] = (q as f32) * scale;
                out_idx += 1;
                pos += 1;
            }

            group_idx += 1;
        }
    }
}

// --- Legacy API (delegates to f32 variants) ---

/// Check if a frame fits within existing f16 scales (within drift tolerance).
pub fn frame_fits_scales(
    frame: &[f32],
    scales: &[u16],
    group_len: usize,
    bits: u8,
    drift_factor: f32,
) -> bool {
    let scales_f32 = scales_to_f32(scales);
    frame_fits_scales_f32(frame, &scales_f32, group_len, bits, drift_factor)
}

/// Quantize a frame using pre-computed f16 scales and pack into bitstream.
pub fn quantize_and_pack(
    frame: &[f32],
    scales: &[u16],
    group_len: usize,
    bits: u8,
    out: &mut Vec<u8>,
) {
    let scales_f32 = scales_to_f32(scales);
    quantize_and_pack_f32(frame, &scales_f32, group_len, bits, out)
}

/// Dequantize packed codes using f16 scales, writing f32 values.
pub fn dequantize(
    data: &[u8],
    scales: &[u16],
    group_len: usize,
    bits: u8,
    tensor_len: usize,
    frame_count: usize,
    out: &mut Vec<f32>,
) {
    let scales_f32 = scales_to_f32(scales);
    dequantize_f32(
        data,
        &scales_f32,
        group_len,
        bits,
        tensor_len,
        frame_count,
        out,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantize_roundtrip_8bit() {
        let frame: Vec<f32> = (0..128).map(|i| (i as f32 - 64.0) * 0.1).collect();
        let scales = compute_scales(&frame, 64, 8);
        let mut packed = Vec::new();
        quantize_and_pack(&frame, &scales, 64, 8, &mut packed);

        let mut decoded = Vec::new();
        dequantize(&packed, &scales, 64, 8, frame.len(), 1, &mut decoded);

        assert_eq!(decoded.len(), frame.len());
        for (i, (&orig, &dec)) in frame.iter().zip(decoded.iter()).enumerate() {
            let err = (orig - dec).abs();
            let max_err = if orig.abs() > 0.01 {
                orig.abs() * 0.02
            } else {
                0.1
            };
            assert!(err < max_err, "i={i}, orig={orig}, dec={dec}, err={err}");
        }
    }

    #[test]
    fn test_quantize_roundtrip_3bit() {
        let frame: Vec<f32> = (0..64).map(|i| (i as f32 - 32.0) * 0.5).collect();
        let scales = compute_scales(&frame, 64, 3);
        let mut packed = Vec::new();
        quantize_and_pack(&frame, &scales, 64, 3, &mut packed);

        let mut decoded = Vec::new();
        dequantize(&packed, &scales, 64, 3, frame.len(), 1, &mut decoded);

        let max_val = frame.iter().map(|v| v.abs()).fold(0.0f32, f32::max);
        for (&orig, &dec) in frame.iter().zip(decoded.iter()) {
            let err = (orig - dec).abs();
            assert!(err < max_val * 0.35, "orig={orig}, dec={dec}, err={err}");
        }
    }

    #[test]
    fn test_quantize_roundtrip_5bit() {
        let frame: Vec<f32> = (0..256).map(|i| (i as f32 - 128.0) * 0.05).collect();
        let scales = compute_scales(&frame, 64, 5);
        let mut packed = Vec::new();
        quantize_and_pack(&frame, &scales, 64, 5, &mut packed);

        let mut decoded = Vec::new();
        dequantize(&packed, &scales, 64, 5, frame.len(), 1, &mut decoded);

        let max_val = frame.iter().map(|v| v.abs()).fold(0.0f32, f32::max);
        for (&orig, &dec) in frame.iter().zip(decoded.iter()) {
            let err = (orig - dec).abs();
            assert!(err < max_val * 0.08, "orig={orig}, dec={dec}, err={err}");
        }
    }

    #[test]
    fn test_quantize_roundtrip_7bit() {
        let frame: Vec<f32> = (0..256).map(|i| (i as f32 - 128.0) * 0.05).collect();
        let scales = compute_scales(&frame, 64, 7);
        let mut packed = Vec::new();
        quantize_and_pack(&frame, &scales, 64, 7, &mut packed);

        let mut decoded = Vec::new();
        dequantize(&packed, &scales, 64, 7, frame.len(), 1, &mut decoded);

        for (i, (&orig, &dec)) in frame.iter().zip(decoded.iter()).enumerate() {
            let err = (orig - dec).abs();
            let max_err = if orig.abs() > 0.01 {
                orig.abs() * 0.02
            } else {
                0.1
            };
            assert!(err < max_err, "i={i}, orig={orig}, dec={dec}, err={err}");
        }
    }

    #[test]
    fn test_drift_detection() {
        let frame1: Vec<f32> = vec![1.0; 64];
        let frame2: Vec<f32> = vec![1.05; 64];
        let frame3: Vec<f32> = vec![2.0; 64];

        let scales = compute_scales(&frame1, 64, 8);
        let drift_factor = 1.0 + 26.0 / 256.0;

        assert!(frame_fits_scales(&frame2, &scales, 64, 8, drift_factor));
        assert!(!frame_fits_scales(&frame3, &scales, 64, 8, drift_factor));
    }

    #[test]
    fn test_zero_frame() {
        let frame = vec![0.0f32; 128];
        let scales = compute_scales(&frame, 64, 8);
        let mut packed = Vec::new();
        quantize_and_pack(&frame, &scales, 64, 8, &mut packed);

        let mut decoded = Vec::new();
        dequantize(&packed, &scales, 64, 8, 128, 1, &mut decoded);

        for &v in &decoded {
            assert_eq!(v, 0.0);
        }
    }

    #[test]
    fn test_non_finite_values() {
        let mut frame = vec![1.0f32; 64];
        frame[10] = f32::NAN;
        frame[20] = f32::INFINITY;
        frame[30] = f32::NEG_INFINITY;

        let scales = compute_scales(&frame, 64, 8);
        let mut packed = Vec::new();
        quantize_and_pack(&frame, &scales, 64, 8, &mut packed);

        let mut decoded = Vec::new();
        dequantize(&packed, &scales, 64, 8, 64, 1, &mut decoded);

        assert_eq!(decoded[10], 0.0);
        assert_eq!(decoded[20], 0.0);
        assert_eq!(decoded[30], 0.0);
        assert!((decoded[0] - 1.0).abs() < 0.02);
    }

    #[test]
    fn test_single_element_group() {
        let frame = vec![3.14f32; 16];
        let scales = compute_scales(&frame, 1, 8);
        assert_eq!(scales.len(), 16);

        let mut packed = Vec::new();
        quantize_and_pack(&frame, &scales, 1, 8, &mut packed);

        let mut decoded = Vec::new();
        dequantize(&packed, &scales, 1, 8, 16, 1, &mut decoded);

        for (i, &v) in decoded.iter().enumerate() {
            let err = (v - 3.14).abs();
            assert!(err < 0.03, "i={i} v={v} err={err}");
        }
    }

    #[test]
    fn test_compression_ratio() {
        let frame = vec![1.0f32; 512];
        for &(bits, min_ratio) in &[(8u8, 3.5f32), (7, 4.0), (5, 5.5), (3, 8.5)] {
            let scales = compute_scales(&frame, 64, bits);
            let mut packed = Vec::new();
            quantize_and_pack(&frame, &scales, 64, bits, &mut packed);

            let raw_bytes = frame.len() * 4;
            let compressed = packed.len() + scales.len() * 2;
            let ratio = raw_bytes as f32 / compressed as f32;

            assert!(
                ratio >= min_ratio,
                "bits={bits}: ratio {ratio:.2}x < expected {min_ratio}x"
            );
        }
    }
}
